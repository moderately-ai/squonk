// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The scanning loop: trivia skipping and per-category lexeme recognition.
//!
//! Everything here drives a [`Cursor`] forward and emits `(kind, span)` tokens.
//! It allocates nothing per token — a token is two `u32`s and a small tag — and
//! never copies source text; literal/identifier text is recovered from the span
//! later.

use crate::ast::Span;
use crate::ast::dialect::{
    FeatureSet,
    lex_class::{
        CLASS_DIGIT, CLASS_IDENTIFIER_CONTINUE, CLASS_IDENTIFIER_START, CLASS_OPERATOR,
        CLASS_PUNCTUATION, CLASS_WHITESPACE, CLASS_WHITESPACE_BOUNDARY, CLASS_WHITESPACE_CONTINUE,
    },
    lookup_keyword,
};

use super::cursor::Cursor;
use super::error::{LexError, LexErrorKind};
use super::token::{Operator, Punctuation, Token, TokenKind};
use super::trivia::{TriviaKind, TriviaRange, TriviaSink};

/// Cross-token lexical state the scan drivers thread between [`next_token`] calls.
///
/// A MySQL versioned-comment region (`/*!40101 … */`) spans many tokens: its body
/// lexes as live input, so "inside a region" must survive from one `next_token`
/// call to the next until the closing `*/`. Deliberately a flag, not a depth —
/// MySQL regions do not nest (an inner `/*!NNNNN` is a marker, engine-verified;
/// see [`scan_versioned_marker`]) — so one open-offset is the whole state.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LexState {
    /// `Some(offset)` while a versioned-comment region is open; the offset is the
    /// region opener's start, kept for the unterminated-region error span.
    versioned_region_start: Option<u32>,
}

/// Scan the next token, skipping any leading trivia (whitespace and comments).
///
/// Returns `Ok(None)` at end of input, `Ok(Some(token))` for a lexeme, or `Err`
/// for the first lexical error (fail-fast). On error the cursor is left at the
/// offending position.
///
/// `trivia` is the opt-in out-of-band capture sink: the leading trivia
/// is skipped from the token stream either way, but a recording sink keeps its
/// spans for tooling. The default [`NoTrivia`](super::trivia::NoTrivia) sink folds
/// that capture away at compile time, so the common path is unchanged — see
/// [`skip_trivia`].
///
/// `state` is the driver-owned cross-token lexical state ([`LexState`]); each
/// eager/streaming driver owns one per scan so a versioned-comment region can
/// span the tokens between its opener and its closing `*/`.
pub(crate) fn next_token<S: TriviaSink>(
    cursor: &mut Cursor,
    features: &FeatureSet,
    trivia: &mut S,
    state: &mut LexState,
) -> Result<Option<Token>, LexError> {
    skip_trivia(cursor, features, trivia, state)?;

    let start = cursor.pos();
    let Some(byte) = cursor.peek() else {
        // End of input inside an open versioned region: the engine rejects an
        // unterminated `/*!…` construct exactly like an unterminated `/* …`.
        if let Some(open) = state.versioned_region_start {
            return Err(LexError::new(
                LexErrorKind::UnterminatedBlockComment,
                Span::new(open, cursor.pos()),
            ));
        }
        return Ok(None);
    };

    let token = match byte {
        b'E' | b'e'
            if features.string_literals.escape_strings && cursor.peek_nth(1) == Some(b'\'') =>
        {
            // PostgreSQL `E'...'` always processes backslash escapes for termination.
            let token = scan_prefixed_string(cursor, 1, b'\'', true)?;
            // Reject a malformed escape (short/invalid Unicode, an escape decoding to
            // NUL, or a byte escape that breaks UTF-8) here, as the real parser does
            // in its scanner — an unknown escape like `\q` stays a literal character.
            // The escape grammar is the AST crate's, shared with lazy `as_str`
            // materialisation, so the eager verdict and the materialised value never
            // disagree (ADR-0006: the check is a no-allocation scan, not a decode).
            let text = cursor
                .src()
                .get(token.span.start() as usize..token.span.end() as usize);
            if text.is_some_and(|text| !crate::ast::postgres_escape_string_is_valid(text)) {
                return Err(LexError::new(
                    LexErrorKind::InvalidEscapeSequence,
                    token.span,
                ));
            }
            token
        }
        b'N' | b'n'
            if features.string_literals.national_strings && cursor.peek_nth(1) == Some(b'\'') =>
        {
            scan_prefixed_string(cursor, 1, b'\'', features.string_literals.backslash_escapes)?
        }
        b'U' | b'u'
            if features.string_literals.unicode_strings
                && cursor.peek_nth(1) == Some(b'&')
                && cursor.peek_nth(2) == Some(b'\'') =>
        {
            // `U&'...'` reserves `\` for Unicode escapes, not quote termination. Unlike
            // the `E'...'` arm above, this one does not eagerly validate the escape
            // body: a trailing `UESCAPE 'c'` clause can override which character is
            // the escape, and that clause is a separate token this scan has not seen
            // yet, so a check keyed to the default `\` here could misreject a body
            // that a later `UESCAPE` override makes legal. PostgreSQL's own scanner
            // has the identical constraint and resolves it the same way: it never
            // decodes a `U&'...'` string itself — a wrapper between its scanner and
            // grammar looks one token ahead for `UESCAPE` before decoding exactly once
            // with whichever escape character that resolves to. Our parser plays that
            // wrapper's role (`Parser::parse_string_literal`, which validates via
            // `unicode_escape_string_is_valid` right after folding any `UESCAPE`
            // clause into the literal's span) — so the eager check still lands before
            // any value is materialised, just one layer up from where `E'...'`
            // manages it.
            scan_prefixed_string(cursor, 2, b'\'', false)?
        }
        // `U&"..."` Unicode-escaped *identifier* (PostgreSQL / SQL standard), the
        // delimited-identifier twin of the `U&'...'` string arm above — one `U&` escape
        // facility with two quote surfaces (SQL:2016's `<Unicode escape prefix>`), gated
        // by the same `unicode_strings`. The three-byte `U`/`u` + `&` + `"` trigger is
        // what distinguishes it from a plain `U`-led identifier: `U` is identifier-start,
        // so `SELECT U&1` must stay a `U` word, a `&` operator, and `1` (engine-probed),
        // and only the full `U&"` lead yields to this arm — it never steals a bare `u`
        // followed by an `&` operator. Disjoint from the `U&'...'` string arm by the
        // third byte (`"` vs `'`). Emits a `QuotedIdent` so the zero-length body check
        // (`U&""`) fires exactly as it does for a plain `""`, covering the whole `U&""`
        // span (PG rejects it there too). Like the string arm, the escape body is *not*
        // validated here — a trailing `UESCAPE 'c'` clause, a token this scan has not
        // seen, can change the active escape character; the parser folds that clause and
        // decodes/validates once (`Parser::parse_ident_admitting` →
        // `materialize_unicode_ident`), playing the role of PostgreSQL's `base_yylex`
        // wrapper, which looks one token past a `UIDENT` for `UESCAPE` before decoding.
        b'U' | b'u'
            if features.string_literals.unicode_strings
                && cursor.peek_nth(1) == Some(b'&')
                && cursor.peek_nth(2) == Some(b'"') =>
        {
            scan_unicode_ident(cursor, features)?
        }
        // SQLite/MySQL `x'53514C'` / `X'53514c'` hexadecimal blob literal — an even count
        // of hex digits, validated *eagerly* here (odd length or a non-hex body is a
        // tokenize-time syntax error in both engines, unlike a deferred `X'…'` bit-string;
        // probed). Only the `x`/`X` hex marker, so this arm precedes the bit-string arm:
        // where both flags are on (MySQL), `x`/`X` lexes as the eager blob and `B`/`b`
        // falls through to the deferred bit-string below; where only bit-strings are on
        // (PostgreSQL), this arm's guard is false and `X'…'` stays the deferred form.
        b'X' | b'x'
            if features.string_literals.blob_literals && cursor.peek_nth(1) == Some(b'\'') =>
        {
            scan_hex_blob(cursor)?
        }
        // `B'1010'` / `X'1FF'` bit-string constants. The quote must abut the marker
        // (the prod-token disambiguation, like `E'`/`N'`): with a space, `B` / `X` is
        // an ordinary identifier. Backslashes are inert in a bit string, so the body
        // scans with standard `''`-only quoting; radix and digit checks are deferred.
        b'B' | b'b' | b'X' | b'x'
            if features.string_literals.bit_string_literals
                && cursor.peek_nth(1) == Some(b'\'') =>
        {
            scan_prefixed_string(cursor, 1, b'\'', false)?
        }
        // MySQL `_charset'…'` / `_charset"…"` character-set introducer: an `_`-prefixed
        // charset name abutting a string literal (`_utf8mb4'x'`, `_latin1"x"`). Gated by
        // dialect data; the name rides the span and the body materialises with the
        // introducer stripped (ADR-0006), mirroring the `N'…'` national prefix — but the
        // prefix length is variable (the charset name), so it is measured first, along
        // with the opening quote byte. The abutting quote is required: a bare `_name`
        // with no quote makes [`charset_introducer_prefix`] return `None`, so the arm
        // lexes it as an ordinary identifier — `_` is an identifier-start byte — and never
        // steals a leading-`_` identifier. A `"` counts only when the dialect
        // treats `"…"` as a string (`double_quoted_strings`); under `ANSI_QUOTES` on,
        // `"` quotes an identifier and the introducer does not apply — see
        // [`charset_introducer_prefix`].
        b'_' if features.string_literals.charset_introducers => {
            match charset_introducer_prefix(cursor, features) {
                Some((prefix_len, close)) => scan_prefixed_string(
                    cursor,
                    prefix_len,
                    close,
                    features.string_literals.backslash_escapes,
                )?,
                // No charset name, or no abutting string quote: an ordinary `_name`
                // identifier. `_` is an identifier-start byte, so this is the same
                // fallthrough the byte-class dispatch below would take.
                None => scan_word(cursor, features),
            }
        }
        b'\'' => scan_quoted(
            cursor,
            QuoteScan {
                close: b'\'',
                kind: TokenKind::String,
                unterminated: LexErrorKind::UnterminatedString,
                backslash: features.string_literals.backslash_escapes,
                // A `String` body has no zero-length reject (an empty string is valid SQL),
                // so this flag is inert for the string kinds; pass `false` uniformly.
                allow_zero_length: false,
            },
        )?,
        b'"' if features.string_literals.double_quoted_strings => scan_quoted(
            cursor,
            QuoteScan {
                close: b'"',
                kind: TokenKind::String,
                unterminated: LexErrorKind::UnterminatedString,
                backslash: features.string_literals.backslash_escapes,
                allow_zero_length: false,
            },
        )?,
        byte if opening_identifier_quote(features, byte).is_some() => {
            let close = opening_identifier_quote(features, byte)
                .expect("guard matched an opening identifier quote");
            scan_quoted(
                cursor,
                QuoteScan {
                    close,
                    kind: TokenKind::QuotedIdent,
                    unterminated: LexErrorKind::UnterminatedQuotedIdent,
                    backslash: false,
                    allow_zero_length: features.identifier_syntax.empty_quoted_identifiers,
                },
            )?
        }
        // The `$` byte fans out to three dialect-disjoint forms, in this priority:
        //   1. money `$1234.56` / `$.5`   (T-SQL, `numeric_literals.money_literals`)
        //   2. positional `$1` parameter  (PostgreSQL, `parameters.positional_dollar`)
        //   3. dollar-quoted `$tag$…$tag$` (PostgreSQL, `string_literals.dollar_quoted_strings`)
        // The follow-sets keep these unambiguous: a dollar-quote opener needs a
        // tag-start or `$` after the `$` (never a digit or `.`), so it never competes
        // with money or a parameter; `$.5` starts with `.`, which is neither a digit, a
        // tag-start, nor `$`, so only money can claim it. Money and the positional
        // parameter both want `$`+digit, but no shipped dialect enables both — T-SQL has
        // money and neither PG `$` form; PostgreSQL has the two `$` forms and no money —
        // so the overlap is theoretical. Money is tried first to resolve it
        // deterministically all the same.
        b'$' if features.numeric_literals.money_literals && money_follows(cursor, features) => {
            scan_money(cursor, features)
        }
        b'$' if features.parameters.positional_dollar
            && cursor.peek_nth(1).is_some_and(|b| b.is_ascii_digit()) =>
        {
            scan_parameter(cursor, features)
        }
        // SQLite `$name` parameter, gated by dialect data. Follow-set-disjoint from the
        // positional `$`+digit arm above (this needs `$`+identifier-start), and tried
        // before dollar-quoting: the two share the `$`+identifier-start trigger (a
        // `LexicalConflict`), so this fixed order resolves it — but SQLite has no
        // dollar-quoting, so no shipped preset enables both.
        b'$' if features.parameters.named_dollar
            && cursor
                .char_at(1)
                .is_some_and(|ch| is_identifier_start(ch, features)) =>
        {
            scan_parameter(cursor, features)
        }
        b'$' if features.string_literals.dollar_quoted_strings => {
            scan_dollar_quote(cursor, features)?
        }
        b'$' => {
            return Err(LexError::new(
                LexErrorKind::StrayByte,
                Span::new(start, start + 1),
            ));
        }
        // Anonymous `?` parameter (ODBC/JDBC), gated by dialect data; otherwise `?`
        // is a stray byte (it is in no lexical class).
        b'?' if features.parameters.anonymous_question => scan_parameter(cursor, features),
        // SQLite numbered `?NNN` parameter, gated by dialect data. Reached only when the
        // anonymous `?` arm above declines (a dialect with numbered but not anonymous `?`);
        // requires a following digit, so it never claims a bare `?`. Follow-set-disjoint from
        // the `jsonb` `?`/`?|`/`?&` operators below (none is `?`+digit), exactly as
        // `positional_dollar` (`$`+digit) is disjoint from dollar-quoting — placed before the
        // operator arm so the digit form always routes here.
        b'?' if features.parameters.numbered_question
            && cursor.peek_nth(1).is_some_and(|b| b.is_ascii_digit()) =>
        {
            scan_parameter(cursor, features)
        }
        // PostgreSQL `jsonb` existence operators `?` / `?|` / `?&`, gated by dialect data.
        // The `?` byte is not in the operator class, so it reaches the operator scanner
        // through this dispatch arm (like `@>`/`#`). Placed after the anonymous-placeholder
        // arm above, so a feature set enabling both (the tracked
        // `LexicalConflict::JsonbKeyExistsVersusAnonymousParameter`) lexes `?` as the
        // placeholder; PostgreSQL has no `?` parameter, so the two never both fire here.
        b'?' if features.operator_syntax.jsonb_operators => scan_operator(cursor, features),
        // Named `:name` parameter (Oracle/SQLite/JDBC), gated by dialect data. The
        // sigil must abut an identifier-start byte, which keeps the other two `:`
        // meanings intact: `::` is the typecast (its second `:` is not an identifier
        // byte, so this arm never fires on it) and a lone `:` before a digit/`]`/space
        // is the array-slice separator. So no `::` handling belongs here — that stays
        // with `scan_punctuation`.
        b':' if features.parameters.named_colon
            && cursor
                .char_at(1)
                .is_some_and(|ch| is_identifier_start(ch, features)) =>
        {
            scan_parameter(cursor, features)
        }
        // Snowflake stage reference `@stage` / `@~` / `@%table` with optional `/path`
        // segments. Gated by `stage_references`. Placed before the MySQL/T-SQL `@name`
        // arms so a dialect that enables stage refs (and keeps those off — Snowflake)
        // claims `@ident` as a stage endpoint rather than a variable/parameter.
        b'@' if features.utility_syntax.stage_references
            && (matches!(cursor.peek_nth(1), Some(b'~') | Some(b'%'))
                || cursor
                    .char_at(1)
                    .is_some_and(|ch| is_identifier_start(ch, features))) =>
        {
            scan_stage_reference(cursor, features)
        }
        // MySQL `@@[scope.]name` system variable, gated by dialect data. The second
        // `@` keeps this disjoint from the single-`@` user-variable and named-at
        // parameter forms below; an identifier-start must follow the `@@`.
        b'@' if features.session_variables.system_variables
            && cursor.peek_nth(1) == Some(b'@')
            && cursor
                .char_at(2)
                .is_some_and(|ch| is_identifier_start(ch, features)) =>
        {
            scan_session_variable(cursor, features)
        }
        // MySQL `@name` user variable, gated by dialect data. Shares the
        // `@`+identifier-start trigger with the named-at parameter below; a feature
        // set enabling both is a `LexicalConflict`, and this arm wins (a user-variable
        // read shadows the placeholder), mirroring the money-vs-positional precedence.
        b'@' if features.session_variables.user_variables
            && cursor
                .char_at(1)
                .is_some_and(|ch| is_identifier_start(ch, features)) =>
        {
            scan_session_variable(cursor, features)
        }
        // Named `@name` parameter (T-SQL), gated by dialect data. Requiring an
        // identifier-start after the `@` deliberately leaves `@@name` (system
        // variables) for the system-variable arm above: a second `@` is not an
        // identifier byte, so this never claims `@@`, and a bare `@` stays a stray byte.
        b'@' if features.parameters.named_at
            && cursor
                .char_at(1)
                .is_some_and(|ch| is_identifier_start(ch, features)) =>
        {
            scan_parameter(cursor, features)
        }
        // PostgreSQL `@>` containment operator, gated by dialect data. The `>` lookahead
        // is required so a bare `@` (not opening `@>`) stays a stray byte — the prefix
        // `@` absolute-value operator is a scoped follow-up, because a bare `@name`
        // contends with the T-SQL/MySQL `@name` sigils and needs a tracked conflict. This
        // arm is reached only after the MySQL `@@`/`@name` and T-SQL `@name` arms above
        // decline (they are all off in PostgreSQL), so a PostgreSQL `@>` routes here into
        // the operator scanner. The sibling `<@` / `->` / `->>` operators lead with `<` /
        // `-` (already operator-class bytes), so they are recognised inside
        // `scan_operator` itself rather than through a dispatch arm.
        b'@' if features.operator_syntax.containment_operators
            && cursor.peek_nth(1) == Some(b'>') =>
        {
            scan_operator(cursor, features)
        }
        // PostgreSQL `jsonb` operators `@?` and `@@`, gated by dialect data. Placed after the
        // MySQL `@@`/`@name`, T-SQL `@name`, and `@>` containment arms above, so a feature set
        // that also enables the `@@name` system-variable form keeps that reading (the tracked
        // `LexicalConflict::JsonbSearchOperatorVersusSystemVariable`), which is why the second
        // byte is checked here rather than left to the operator scanner. `@?` is disjoint from
        // every other `@` claimant by its `?`, so it always routes here.
        b'@' if features.operator_syntax.jsonb_operators
            && matches!(cursor.peek_nth(1), Some(b'?') | Some(b'@')) =>
        {
            scan_operator(cursor, features)
        }
        // The general `@`-lead operators under `custom_operators` — a bare `@` (absolute
        // value / user-defined prefix) or any `@`-led operator not caught above (`@-`, `@-@`,
        // `@#@`, `@@@`). The `@` byte is not in the operator class table, so it reaches the
        // scanner through this dispatch arm (like `@>`). Placed after the MySQL/T-SQL
        // `@`-sigil arms (which win where a dialect enables them — the tracked
        // `LexicalConflict::CustomOperatorVersusAtName` /
        // `LexicalConflict::CustomOperatorVersusSystemVariable`) and after the `@>`/`@?`/`@@`
        // arms; the general operator scanner re-derives the exact operator from the maximal
        // `Op` run, so those known forms classify identically whichever `@` arm routes them.
        b'@' if features.operator_syntax.custom_operators => scan_operator(cursor, features),
        // Standalone `@` account-name separator (MySQL `user@host` with a quoted/backtick
        // host: `u@'h'`, `u@"h"`, `` u@`h` ``). Reached only after the `@@`/`@name` variable
        // arms and every `@`-led operator arm above decline — i.e. the `@` is followed by a
        // string/identifier-quote delimiter (or nothing), never an identifier-start (that
        // folds into a `@name` variable) or a second `@`. Placed after the operator arms so a
        // dialect that reads a bare `@` as an operator keeps that reading; MySQL/Lenient leave
        // those operators off, so the `@` reaches here. The account-name parser reads the
        // following quoted `ident_or_text` as the host; an `@` in any other position surfaces
        // as an unexpected token at parse time (the same net reject as the stray byte it
        // replaces). Gated on `user_variables` so only the MySQL-family `@` surface grows it.
        b'@' if features.session_variables.user_variables => {
            let start = cursor.pos();
            cursor.bump(); // the lone `@`
            Token::new(
                TokenKind::Punctuation(Punctuation::At),
                Span::new(start, cursor.pos()),
            )
        }
        // Unquoted identifier or keyword. ASCII identifier-start bytes (letters, `_`,
        // and any the dialect adds via `byte_classes`) take the byte-class fast path.
        _ if features.has_byte_class(byte, CLASS_IDENTIFIER_START) => scan_word(cursor, features),
        // A non-ASCII lead byte is decoded to one whole character and admitted only
        // if it is a Unicode letter (the identifier-start policy). Any other code
        // point — emoji, symbol, lone combining mark — is a lexical stray rather than
        // a silent word byte; its span covers the whole character so the error offset
        // stays on a char boundary.
        _ if byte >= 0x80 => {
            let ch = cursor
                .char_at(0)
                .expect("a non-ASCII lead byte begins a valid UTF-8 char in &str source");
            if is_identifier_start(ch, features) {
                scan_word(cursor, features)
            } else {
                return Err(LexError::new(
                    LexErrorKind::StrayByte,
                    Span::new(start, start + ch.len_utf8() as u32),
                ));
            }
        }
        _ if is_digit(byte, features) => scan_number(cursor, features)?,
        // A leading `.` is part of a numeric literal only when a digit follows
        // (`.5`); otherwise it is the member/qualifier dot, handled as punctuation.
        b'.' if cursor.peek_nth(1).is_some_and(|b| is_digit(b, features)) => {
            scan_number(cursor, features)?
        }
        // DuckDB `#n` positional column reference, gated by dialect data. A digit must
        // follow — a bare `#` stays a stray byte, matching DuckDB's own "syntax error at
        // or near #". Placed after the identifier-start arm above so a custom byte-class
        // table that marks `#` an identifier byte keeps its `#name` word (the identifier
        // scan wins by order), and before the XOR arm below: no shipped preset enables
        // both (the tracked `LexicalConflict::HashXorOperatorVersusPositionalColumn`), and
        // the digit-led form is the more specific claimant of the `#` trigger.
        b'#' if features.expression_syntax.positional_column
            && cursor.peek_nth(1).is_some_and(|b| b.is_ascii_digit()) =>
        {
            scan_positional_column(cursor)
        }
        // PostgreSQL `#` bitwise-XOR, gated by dialect data. The `#` byte is not in the
        // operator class, so it reaches the operator scanner through this dispatch arm
        // (like `@>`). Placed after the identifier-start arm above so a custom byte-class
        // table that marks `#` an identifier byte keeps its `#name` word (the identifier
        // scan wins by order) — under `hash_bitwise_xor` with a `#` line comment, the
        // comment already won in `skip_trivia`, the tracked conflict.
        b'#' if features.hash_bitwise_xor => scan_operator(cursor, features),
        // Backtick-led general operator under `custom_operators`: `` ` `` is an `Op`-class byte
        // in PostgreSQL's `scan.l` (and DuckDB's fork), so a lead `` ` `` opens a symbolic
        // operator (`` `= ``, `` `` ``) there. Reached only after the identifier-quote arm
        // above declines — a dialect that spells identifiers with backticks (MySQL/Lenient)
        // claims `` `…` `` as a quoted identifier first (and leaves `custom_operators` off);
        // PostgreSQL/DuckDB do not quote with backticks, so their lead `` ` `` reaches the
        // operator scanner here (backtick is not [`CLASS_OPERATOR`], so it needs its own arm,
        // like `@`). Engine-measured on DuckDB 1.5.4: `p` `= q` is one `` `= `` operator.
        b'`' if features.operator_syntax.custom_operators => scan_operator(cursor, features),
        _ if features.has_byte_class(byte, CLASS_OPERATOR) => scan_operator(cursor, features),
        _ if features.has_byte_class(byte, CLASS_PUNCTUATION) => scan_punctuation(cursor, features),
        _ => {
            return Err(LexError::new(
                LexErrorKind::StrayByte,
                Span::new(start, start + 1),
            ));
        }
    };

    // PG parity: reject a raw NUL byte (`0x00`) anywhere in a quoted lexeme's source —
    // a string literal or a quoted identifier. PostgreSQL cannot carry a NUL in a query
    // (its query strings are NUL-terminated C strings), so libpg_query rejects any
    // string literal — ordinary, `E'…'`, `N'…'`, `U&'…'`, bit-string, or dollar-quoted —
    // and any quoted identifier (`"…"`, `[…]`, backtick) that embeds one. Both lexeme
    // families funnel through here carrying their delimiters and body verbatim (escapes
    // are recognised for termination only, never unfolded — ADR-0006), and every quote
    // delimiter is NUL-free ASCII, so one byte scan over the token text covers every
    // form (an escape that *decodes* to NUL is the orthogonal `InvalidEscapeSequence`,
    // already rejected in the `E'…'` arm). The predicate is the AST crate's
    // content-agnostic NUL scan — a single `memchr`, no allocation — which for a string
    // literal is also shared with lazy `as_str` materialisation so the eager verdict and
    // the materialised value never disagree (ADR-0006); a quoted identifier is interned,
    // not materialised through that accessor, so the check is purely the eager gate there.
    let nul_byte_error = match token.kind {
        TokenKind::String => Some(LexErrorKind::NulByteInString),
        TokenKind::QuotedIdent => Some(LexErrorKind::NulByteInIdentifier),
        _ => None,
    };
    if let Some(error_kind) = nul_byte_error {
        let text = &cursor.src()[token.span.start() as usize..token.span.end() as usize];
        if crate::ast::string_literal_embeds_nul(text) {
            return Err(LexError::new(error_kind, token.span));
        }
    }

    Ok(Some(token))
}

/// Skip whitespace, `--` line comments, and `/* … */` block comments (nesting
/// per dialect data) until the cursor sits on the start of a real token or at
/// EOF. Under
/// [`CommentSyntax::versioned_comments`](crate::ast::dialect::CommentSyntax::versioned_comments)
/// this dispatch also owns the MySQL `/*!…` region markers: the opener/closer are
/// consumed as trivia while the body between them lexes as live tokens (`state`
/// carries the open region across `next_token` calls).
///
/// Trivia is consumed, never emitted: the parser sees no whitespace tokens, but
/// every token's span still slices back to the exact source text. An unclosed
/// block comment (or versioned region) is the only trivia that can fail.
///
/// Each skipped run is offered to `trivia` so tooling can recover comments and
/// whitespace out-of-band. The capture is gated on
/// [`TriviaSink::RECORDING`]: the default [`NoTrivia`](super::trivia::NoTrivia)
/// sink sets it `false`, so the `if` below — and the `start` read feeding it — are
/// dropped at compile time, leaving this loop byte-identical to a trivia-free
/// build (no branch, no `Vec`, no push). A recording sink pays only for what it
/// captures. The cursor advances monotonically, so records arrive sorted by
/// offset, which the [`TriviaIndex`](super::trivia::TriviaIndex) relies on.
fn skip_trivia<S: TriviaSink>(
    cursor: &mut Cursor,
    features: &FeatureSet,
    trivia: &mut S,
    state: &mut LexState,
) -> Result<(), LexError> {
    // Statement-boundary "trim" whitespace (`CLASS_WHITESPACE_BOUNDARY`, DuckDB's
    // vertical tab): folded as whitespace below, but legal only as leading/trailing
    // statement trivia. `sweep_start` is where this trivia sweep began — the byte just
    // before it is the previous token's last byte (a `;` there means we are at a
    // statement's leading edge). Comments count as content for this narrow rule: DuckDB
    // accepts a boundary byte on either side of one comment, but rejects one between two
    // comments. `pending_boundary` records a boundary byte with content on its left; a
    // later comment or content token supplies the rejecting right side.
    // The whole mechanism is gated off for every dialect but DuckDB.
    let track_boundary = features.byte_classes.has_boundary_whitespace();
    let sweep_start = cursor.pos();
    let content_before_sweep =
        sweep_start != 0 && cursor.src().as_bytes()[sweep_start as usize - 1] != b';';
    let mut comment_seen = false;
    let mut pending_boundary: Option<u32> = None;
    loop {
        let start = cursor.pos();
        let kind = match cursor.peek() {
            Some(byte) if features.has_byte_class(byte, CLASS_WHITESPACE) => {
                // A whitespace run may absorb `CLASS_WHITESPACE_CONTINUE` bytes (SQLite's
                // vertical tab) as run *extensions* — they ride an open run here but,
                // lacking `CLASS_WHITESPACE`, cannot enter this arm to start one.
                cursor.eat_while(|b| {
                    features.has_byte_class(b, CLASS_WHITESPACE | CLASS_WHITESPACE_CONTINUE)
                });
                if track_boundary {
                    let boundary_at = cursor.src().as_bytes()
                        [start as usize..cursor.pos() as usize]
                        .iter()
                        .position(|&b| features.has_byte_class(b, CLASS_WHITESPACE_BOUNDARY))
                        .map(|i| start + i as u32);
                    if pending_boundary.is_none() && (content_before_sweep || comment_seen) {
                        pending_boundary = boundary_at;
                    }
                }
                TriviaKind::Whitespace
            }
            Some(b'-') if cursor.peek_nth(1) == Some(b'-') => {
                skip_line_comment(
                    cursor,
                    2,
                    features.comment_syntax.line_comment_ends_at_carriage_return,
                );
                TriviaKind::LineComment
            }
            // MySQL `#` line comment, gated by dialect data. This branch claims `#`
            // before any byte-class use of it as an identifier byte, so a dialect
            // cannot both hash-comment and `#temp`-prefix (the documented either/or).
            Some(b'#') if features.comment_syntax.line_comment_hash => {
                skip_line_comment(
                    cursor,
                    1,
                    features.comment_syntax.line_comment_ends_at_carriage_return,
                );
                TriviaKind::LineComment
            }
            // The close of an open versioned region: `*/` where a token would
            // start. Checking only at token boundaries is exactly the engine's
            // string protection — a `*/` inside a string literal of the body is
            // consumed by the string scan and never reaches this dispatch.
            // Outside a region `*/` stays the two operator tokens `*` `/`.
            Some(b'*')
                if state.versioned_region_start.is_some() && cursor.peek_nth(1) == Some(b'/') =>
            {
                cursor.advance_bytes(2);
                state.versioned_region_start = None;
                TriviaKind::BlockComment
            }
            // A bare `/*` sitting exactly at end of input is NOT a comment opener under
            // `unterminated_block_comment_at_eof` (SQLite): the `*` is the last byte, so
            // SQLite lexes the `/` as the slash operator (`z[2]==0`; engine-measured). The
            // extra `peek_nth(2).is_some()` term declines the arm there, letting `/` fall
            // through to the operator scan — every `/*` with a following byte still opens a
            // (silently-EOF-closed) comment.
            Some(b'/')
                if cursor.peek_nth(1) == Some(b'*')
                    && !(features.comment_syntax.unterminated_block_comment_at_eof
                        && cursor.peek_nth(2).is_none()) =>
            {
                // `/*!` is lookahead-disjoint from a plain block comment by its
                // third byte; under `versioned_comments` it is conditional
                // inclusion (MySQL executes the body), never a comment skip.
                match features.comment_syntax.versioned_comments {
                    Some(bound) if cursor.peek_nth(2) == Some(b'!') => {
                        scan_versioned_marker(
                            cursor,
                            bound,
                            features.comment_syntax.nested_block_comments,
                            state,
                        )?;
                    }
                    _ => {
                        skip_block_comment(
                            cursor,
                            features.comment_syntax.nested_block_comments,
                            features.comment_syntax.unterminated_block_comment_at_eof,
                        )?;
                    }
                }
                TriviaKind::BlockComment
            }
            _ => {
                // End of the trivia sweep: the cursor is at the next content byte, `;`,
                // or end of input. A statement-boundary "trim" byte (DuckDB's `\v`) is an
                // error only when a content token both precedes and follows it in the same
                // statement — a leading edge (sweep at input start, or just past a `;`) or
                // a trailing edge (sweep ends at `;` or end of input) is legal.
                if let Some(off) = pending_boundary {
                    let content_after = cursor.peek().is_some_and(|b| b != b';');
                    if content_after {
                        return Err(LexError::new(
                            LexErrorKind::StrayByte,
                            Span::new(off, off + 1),
                        ));
                    }
                }
                return Ok(());
            }
        };
        if matches!(kind, TriviaKind::LineComment | TriviaKind::BlockComment) {
            if let Some(off) = pending_boundary {
                return Err(LexError::new(
                    LexErrorKind::StrayByte,
                    Span::new(off, off + 1),
                ));
            }
            comment_seen = true;
        }
        // PG parity: reject a raw NUL byte (`0x00`) embedded in a comment. PostgreSQL
        // rejects a NUL anywhere in a query — a query reaches the server as a
        // NUL-terminated C string, so libpg_query's `CString::new` fails on any interior
        // NUL before parsing — which the `next_token` gate already enforces for the
        // value-bearing lexemes (string literals, quoted identifiers). A comment is skipped
        // here as trivia, its bytes consumed to end-of-line / `*/` without inspection, so
        // this is the one lexable context that gate cannot see; without this check a
        // `--`/`/* */` comment embedding a NUL is silently accepted while PostgreSQL rejects
        // it (fuzz-pg-differential-crash-2b8d66f9). Whitespace runs are NUL-free by
        // construction (NUL is not in the whitespace class), so only comment runs are scanned.
        if matches!(kind, TriviaKind::LineComment | TriviaKind::BlockComment)
            && cursor.src().as_bytes()[start as usize..cursor.pos() as usize].contains(&0)
        {
            return Err(LexError::new(
                LexErrorKind::NulByteInComment,
                Span::new(start, cursor.pos()),
            ));
        }
        if S::RECORDING {
            trivia.record(TriviaRange::new(kind, Span::new(start, cursor.pos())));
        }
    }
}

/// Consume a line-comment introducer (`marker_len` bytes: 2 for `--`, 1 for `#`)
/// and the rest of the line. A `\n` always ends the line; when `cr_terminates` is set —
/// PostgreSQL/DuckDB dialect data
/// ([`CommentSyntax::line_comment_ends_at_carriage_return`](crate::ast::dialect::CommentSyntax::line_comment_ends_at_carriage_return)),
/// whose flex scanner's comment body is `[^\n\r]*` — a `\r` ends it too, while SQLite/MySQL
/// (flag off) read a `\r` as ordinary comment content and end only at `\n`. The terminating
/// byte is left for the whitespace branch either way (both `\n` and `\r` are whitespace), so
/// end-of-line handling — and the terminator's trivia span — stays in one place.
fn skip_line_comment(cursor: &mut Cursor, marker_len: u32, cr_terminates: bool) {
    cursor.advance_bytes(marker_len); // the dispatch confirmed these marker bytes
    // Hoist the dialect branch out of the per-byte loop: choose the stop predicate once.
    if cr_terminates {
        cursor.eat_while(|b| b != b'\n' && b != b'\r');
    } else {
        cursor.eat_while(|b| b != b'\n');
    }
}

/// Consume a `/* … */` block comment. With `nested` (the PostgreSQL-style
/// permissive superset chosen as the baseline) an inner `/*` raises the
/// depth, `*/` lowers it, and the comment ends only when depth returns to zero;
/// without it (MySQL, engine-verified) the first `*/` closes the comment.
///
/// `silent_eof` is SQLite's
/// [`CommentSyntax::unterminated_block_comment_at_eof`](crate::ast::dialect::CommentSyntax::unterminated_block_comment_at_eof):
/// a body that runs off the end returns `Ok` (the comment is silently closed as trailing
/// trivia) instead of the [`LexErrorKind::UnterminatedBlockComment`] every other dialect
/// raises. The dispatch has already guaranteed a byte follows the opening `/*` under this
/// flag, so a bare `/*` at EOF never reaches here as a comment.
fn skip_block_comment(cursor: &mut Cursor, nested: bool, silent_eof: bool) -> Result<(), LexError> {
    let start = cursor.pos();
    cursor.bump(); // '/'
    cursor.bump(); // '*'

    let mut depth: u32 = 1;
    loop {
        match (cursor.peek(), cursor.peek_nth(1)) {
            (Some(b'/'), Some(b'*')) if nested => {
                cursor.bump();
                cursor.bump();
                // Each `/*` is two bytes and the source is <= u32::MAX, so depth
                // is bounded well below u32::MAX and this cannot overflow. Assert
                // the invariant loudly instead of silently wrapping in release.
                depth = depth
                    .checked_add(1)
                    .expect("block-comment nesting depth exceeds u32::MAX");
            }
            (Some(b'*'), Some(b'/')) => {
                cursor.bump();
                cursor.bump();
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            (Some(_), _) => {
                cursor.bump();
            }
            (None, _) => {
                // SQLite silently closes an unterminated `/* …` at EOF as trailing trivia;
                // every other dialect errors.
                if silent_eof {
                    return Ok(());
                }
                return Err(LexError::new(
                    LexErrorKind::UnterminatedBlockComment,
                    Span::new(start, cursor.pos()),
                ));
            }
        }
    }
}

/// Handle a `/*!` versioned-comment introducer (MySQL conditional inclusion).
///
/// Consumes the `/*!` marker and its abutting version digits, then either OPENS
/// the region — the body lexes as live tokens until the next region-level `*/`,
/// which [`skip_trivia`] consumes as the close — or DISCARDS the region wholesale
/// (the engine's skip for a version above the modelled `bound`). Inside an
/// already-open region the same scan runs again, giving the engine-verified flag
/// semantics: a passing inner marker is a no-op (no depth is tracked), while a
/// failing inner marker discards only up to the next `*/`, leaving the outer
/// region open.
///
/// The version-digit rule is the engine's (probed against mysql:8, 8.4.10): the
/// run must abut the `!`; exactly five or exactly six digits form the version;
/// 0–4 digits are not a version (they stay body tokens and the region is
/// included unconditionally); from a run of ≥7 the first five are the version
/// and the rest stay body tokens.
fn scan_versioned_marker(
    cursor: &mut Cursor,
    bound: u32,
    nested: bool,
    state: &mut LexState,
) -> Result<(), LexError> {
    let start = cursor.pos();
    cursor.advance_bytes(3); // the dispatch confirmed `/*!`

    let mut digits: u32 = 0;
    while digits < 7 && cursor.peek_nth(digits).is_some_and(|b| b.is_ascii_digit()) {
        digits += 1;
    }
    let version_len = match digits {
        0..=4 => 0,
        6 => 6,
        _ => 5, // exactly five, or the first five of a ≥7 run
    };
    let mut version: u32 = 0;
    for _ in 0..version_len {
        let digit = cursor.bump().expect("the digit run was just measured") - b'0';
        // At most six decimal digits, so the accumulator tops out at 999_999.
        version = version * 10 + u32::from(digit);
    }

    if version_len > 0 && version > bound {
        return skip_discarded_region(cursor, start, nested);
    }
    // Include: open the region, or keep the outer one open (inner markers are
    // no-ops — the engine tracks a flag, not a depth).
    state.versioned_region_start.get_or_insert(start);
    Ok(())
}

/// Discard a version-gated region whose gate failed: raw bytes up to and
/// including the next region-level `*/`, honouring inner `/* … */` comments
/// (each inner comment — a plain one or an inner `/*!…` marker — consumes its
/// own terminator, so its `*/` cannot close the region; engine-verified). The
/// scan is NOT string-aware: a quote is just a byte here, exactly as in the
/// engine's discard path (probed: an unbalanced `'` inside a skipped region is
/// harmless, and a `*/` inside a would-be string does close it).
fn skip_discarded_region(
    cursor: &mut Cursor,
    region_start: u32,
    nested: bool,
) -> Result<(), LexError> {
    loop {
        match (cursor.peek(), cursor.peek_nth(1)) {
            (Some(b'*'), Some(b'/')) => {
                cursor.advance_bytes(2);
                return Ok(());
            }
            (Some(b'/'), Some(b'*')) => {
                // A discarded versioned region (MySQL) never sets `silent_eof`: an
                // unterminated inner `/* …` here is still the unterminated-region error.
                skip_block_comment(cursor, nested, false)?;
            }
            (Some(_), _) => {
                cursor.bump();
            }
            (None, _) => {
                return Err(LexError::new(
                    LexErrorKind::UnterminatedBlockComment,
                    Span::new(region_start, cursor.pos()),
                ));
            }
        }
    }
}

/// Scan an unquoted identifier-or-keyword word.
///
/// The dispatch has already validated the start character against the Unicode
/// identifier policy ([`is_identifier_start`]); this consumes it and the
/// identifier-continue run. Start and continue characters are consumed whole (a
/// multi-byte character is never split), so the resulting span always lands on char
/// boundaries — token text is recovered by slicing the span, which must stay
/// valid UTF-8.
fn scan_word(cursor: &mut Cursor, features: &FeatureSet) -> Token {
    let start = cursor.pos();
    cursor
        .bump_char()
        .expect("dispatch routes only identifier-start characters to scan_word");
    eat_identifier_continue(cursor, features);
    let span = Span::new(start, cursor.pos());
    let text = &cursor.src()[span.start() as usize..span.end() as usize];
    let kind = lookup_keyword(text).map_or(TokenKind::Word, TokenKind::Keyword);
    Token::new(kind, span)
}

/// Scan an integer or float: `[0-9]+`, `[0-9]*.[0-9]*`, leading-dot `.[0-9]+`,
/// each with an optional `[eE][+-]?[0-9]+` exponent, plus the dialect radix and `_`
/// separator forms. The caller guarantees the cursor is on a digit, or on a `.`
/// immediately followed by a digit.
///
/// Fails with [`LexErrorKind::TrailingJunkAfterNumber`] when the dialect sets
/// [`reject_trailing_junk`](crate::ast::dialect::NumericLiteralSyntax::reject_trailing_junk)
/// and an identifier-start character abuts the literal (`123abc`, `0x`, `100_`): PG
/// treats a numeric literal as maximal munch, so anything identifier-ish immediately
/// after is the scanner error, not a new token. Under a loose dialect the check is off
/// and that trailing text lexes as an ordinary word/alias next.
fn scan_number(cursor: &mut Cursor, features: &FeatureSet) -> Result<Token, LexError> {
    let start = cursor.pos();
    let num = &features.numeric_literals;
    let separators = num.underscore_separators;
    // Strict placement (`_` must sit between digits) rides the same knob as trailing-junk
    // rejection: both are PostgreSQL's precise numeric scanner, so a misplaced `_` stops
    // the number and is then caught as junk below. A loose dialect keeps the deferred
    // placement (a `_` is a separator wherever a digit follows).
    let strict = num.reject_trailing_junk;

    // Radix-prefixed integer (`0x`/`0o`/`0b`) when the dialect enables it and a valid
    // radix body follows (a radix digit, or — under the strict scanner — a `_`-led group);
    // otherwise the leading `0` is an ordinary decimal (`0xZ` -> `0` then `xZ`).
    if let Some(radix) = radix_prefix(cursor, features) {
        cursor.bump(); // '0'
        cursor.bump(); // radix marker (x/o/b)
        eat_radix_digits(cursor, radix, separators);
    } else {
        eat_fixed_point(cursor, features, separators, strict);

        if matches!(cursor.peek(), Some(b'e' | b'E')) && exponent_follows(cursor, features) {
            cursor.bump(); // 'e' / 'E'
            if matches!(cursor.peek(), Some(b'+' | b'-')) {
                cursor.bump();
            }
            eat_decimal_digits(cursor, features, separators, strict);
        }
    }

    // PG parity (`reject_trailing_junk`): an identifier-start character abutting the
    // literal is "trailing junk after numeric literal" — a lexer error. The whole
    // malformed lexeme (number plus the trailing identifier run) is spanned, mirroring
    // PG's `{decinteger}{identifier}` munch, so the diagnostic points at `123abc`, not
    // just `123`. A trailing/doubled/leading-in-fraction `_` already stopped the number
    // above (strict placement) and surfaces here as its own junk `_…` run.
    if strict
        && cursor
            .char_at(0)
            .is_some_and(|ch| is_identifier_start(ch, features))
    {
        eat_identifier_continue(cursor, features);
        return Err(LexError::new(
            LexErrorKind::TrailingJunkAfterNumber,
            Span::new(start, cursor.pos()),
        ));
    }

    Ok(Token::new(
        TokenKind::Number,
        Span::new(start, cursor.pos()),
    ))
}

/// Whether the `$` at the cursor opens a money literal: `$` + a digit (`$100`) or
/// `$` + `.` + a digit (`$.5`). The dispatch guard — the `$` is `peek()`, so the body
/// begins at `peek_nth(1)`. A `$.` with no following digit is *not* money (it falls
/// through to the dollar-quote / stray-byte arms).
fn money_follows(cursor: &Cursor, features: &FeatureSet) -> bool {
    match cursor.peek_nth(1) {
        Some(byte) if is_digit(byte, features) => true,
        Some(b'.') => cursor
            .peek_nth(2)
            .is_some_and(|byte| is_digit(byte, features)),
        _ => false,
    }
}

/// Scan a T-SQL money literal: `$` then a fixed-point decimal (`$1234.56`, `$100`,
/// `$.5`). The caller's guard ([`money_follows`]) has confirmed a digit follows the
/// `$` or the `$.`. T-SQL money is plain fixed-point — no exponent, radix, or `_`
/// separator — so the body is an integer part with an optional fraction, or a
/// leading-dot fraction (the decimal half of [`scan_number`], without the exponent).
///
/// Emits a [`Number`](TokenKind::Number) spanning the `$`: money shares the numeric
/// token family and its `money` type is recovered from the `$` prefix at parse
/// time, mirroring how `B'…'`/`X'…'` reuse the String token. A leading `-`/`+`
/// stays a separate operator (signs are never folded into a literal), so
/// `-$1000` lexes as `Minus` then this money token.
fn scan_money(cursor: &mut Cursor, features: &FeatureSet) -> Token {
    let start = cursor.pos();
    cursor.bump(); // '$'
    // Money is plain fixed-point: no `_` separators (so `strict` is moot) and no
    // trailing-junk gate (a T-SQL form, not part of PG's strict numeric scanner).
    eat_fixed_point(cursor, features, false, false);
    Token::new(TokenKind::Number, Span::new(start, cursor.pos()))
}

/// Eat a fixed-point decimal: an integer part with an optional `.fraction`, or a
/// leading-dot `.fraction` (`.5`). Shared by [`scan_number`] — the decimal half,
/// before any exponent — and [`scan_money`], whose whole body is exactly this.
/// `separators` toggles `_` digit grouping (dialect-gated for numbers, always off for
/// money); `strict` requires each `_` to sit between two digits (see
/// [`eat_decimal_digits`]). A trailing `1.` with no fraction is left as written and
/// validated later.
fn eat_fixed_point(cursor: &mut Cursor, features: &FeatureSet, separators: bool, strict: bool) {
    if cursor.peek() == Some(b'.') {
        cursor.bump(); // leading '.'
        eat_decimal_digits(cursor, features, separators, strict);
    } else {
        eat_decimal_digits(cursor, features, separators, strict); // integer part
        if cursor.peek() == Some(b'.') {
            cursor.bump(); // decimal point
            eat_decimal_digits(cursor, features, separators, strict); // fractional part (may be empty, e.g. `1.`)
        }
    }
}

/// Eat a run of decimal digits, treating `_` as a digit-group separator when
/// `separators` is set and a digit follows it — so `1_000` is one run, but a trailing
/// `1_` stops at `1` (the `_` then begins a word). With `strict` (PG's precise numeric
/// scanner, `{decdigit}(_?{decdigit})*`) a `_` must additionally follow a digit *within
/// this run*, so a leading `_` — the fraction of `1_000._5`, or a `_` opening the
/// integer part — is not a separator and stops the run, leaving the `_` to be caught as
/// trailing junk. Without `strict` the placement is loose (the deferred rule).
fn eat_decimal_digits(cursor: &mut Cursor, features: &FeatureSet, separators: bool, strict: bool) {
    let mut prev_was_digit = false;
    loop {
        match cursor.peek() {
            Some(byte) if is_digit(byte, features) => {
                cursor.bump();
                prev_was_digit = true;
            }
            Some(b'_')
                if separators
                    && (!strict || prev_was_digit)
                    && cursor.peek_nth(1).is_some_and(|b| is_digit(b, features)) =>
            {
                cursor.bump(); // separator; the following digit is eaten next iteration
                prev_was_digit = false; // the '_' is not itself a digit
            }
            _ => return,
        }
    }
}

/// A non-decimal integer radix introduced by a `0x`/`0o`/`0b` prefix.
#[derive(Clone, Copy)]
enum Radix {
    Hex,
    Octal,
    Binary,
}

impl Radix {
    /// Whether `byte` is a digit in this radix.
    fn is_digit(self, byte: u8) -> bool {
        match self {
            Self::Hex => byte.is_ascii_hexdigit(),
            Self::Octal => matches!(byte, b'0'..=b'7'),
            Self::Binary => matches!(byte, b'0' | b'1'),
        }
    }
}

/// The radix at the cursor's `0x`/`0o`/`0b` prefix, when the matching dialect flag is
/// set and a valid radix body opens after the marker. `None` otherwise, so the leading
/// `0` falls through to ordinary decimal scanning (e.g. `0xZ` -> `0` + `xZ`; a bare
/// `0x` -> `0` + `x`, then caught as trailing junk where the dialect rejects it).
///
/// The body opens with a radix digit (`0x1F`), or — where the dialect sets
/// [`radix_leading_underscore`](crate::ast::dialect::NumericLiteralSyntax::radix_leading_underscore),
/// PostgreSQL's `0[xX](_?{hexdigit})+` grammar — with a `_` leading the first digit
/// (`0x_1F`). That opener rides its own axis (also requiring
/// [`underscore_separators`](crate::ast::dialect::NumericLiteralSyntax::underscore_separators))
/// because the dialects split on it: PostgreSQL admits the leading `_`, SQLite does not
/// (`0[xX]{hexdigit}(_?{hexdigit})*`), and a loose radix dialect (DuckDB) keeps its
/// pre-existing `0` + word split. A trailing or doubled `_` is never a valid opener here;
/// it is left for [`eat_radix_digits`] to stop on and the junk check to flag.
fn radix_prefix(cursor: &Cursor, features: &FeatureSet) -> Option<Radix> {
    if cursor.peek() != Some(b'0') {
        return None;
    }
    let num = &features.numeric_literals;
    let radix = match cursor.peek_nth(1)? {
        b'x' | b'X' if num.hex_integers => Radix::Hex,
        b'o' | b'O' if num.octal_integers => Radix::Octal,
        b'b' | b'B' if num.binary_integers => Radix::Binary,
        _ => return None,
    };
    let first = cursor.peek_nth(2)?;
    let opens_body = radix.is_digit(first)
        || (num.underscore_separators
            && num.radix_leading_underscore
            && first == b'_'
            && cursor.peek_nth(3).is_some_and(|b| radix.is_digit(b)));
    opens_body.then_some(radix)
}

/// Eat a run of radix digits, honouring `_` separators like [`eat_decimal_digits`].
fn eat_radix_digits(cursor: &mut Cursor, radix: Radix, separators: bool) {
    loop {
        match cursor.peek() {
            Some(byte) if radix.is_digit(byte) => {
                cursor.bump();
            }
            Some(b'_') if separators && cursor.peek_nth(1).is_some_and(|b| radix.is_digit(b)) => {
                cursor.bump();
            }
            _ => return,
        }
    }
}

/// True when the `e`/`E` at the cursor begins a well-formed exponent: an
/// optional sign and at least one digit. If not, the `e` belongs to a following
/// word rather than the number.
fn exponent_follows(cursor: &Cursor, features: &FeatureSet) -> bool {
    let mut ahead = 1;
    if matches!(cursor.peek_nth(ahead), Some(b'+' | b'-')) {
        ahead += 1;
    }
    cursor
        .peek_nth(ahead)
        .is_some_and(|b| is_digit(b, features))
}

/// Scan a parameter placeholder: positional `$1` (`$` + ASCII digits), anonymous
/// `?`, or named `:name` / `@name` (sigil + identifier run). The dispatch guards
/// already confirmed the dialect enables the form and that the required next byte
/// (a digit after `$`, an identifier-start after `:`/`@`) follows; the leading byte
/// selects which run to eat. The span covers the whole placeholder, and the
/// form/name split is recovered from it at parse time, so nothing is
/// materialized here.
fn scan_parameter(cursor: &mut Cursor, features: &FeatureSet) -> Token {
    let start = cursor.pos();
    match cursor.peek() {
        Some(b'$') => {
            cursor.bump(); // '$'
            // `$1` positional (PostgreSQL, digits) vs `$name` (SQLite, identifier): the
            // dispatch guard already selected one by the follow byte, so eat the run it
            // matched — digits for the positional index, an identifier for the name.
            if cursor.peek().is_some_and(|b| b.is_ascii_digit()) {
                cursor.eat_while(|b| b.is_ascii_digit());
            } else {
                eat_identifier_continue(cursor, features);
            }
        }
        // `:name` / `@name`: consume the sigil, then the identifier-continue run —
        // the same Unicode policy as an unquoted identifier, so a parameter name is
        // exactly an identifier (non-ASCII letters kept, non-letter code points not).
        Some(b':' | b'@') => {
            cursor.bump(); // ':' or '@'
            eat_identifier_continue(cursor, features);
        }
        // The remaining dispatch arm is the anonymous `?`, or a SQLite numbered `?NNN`.
        _ => {
            cursor.bump(); // '?'
            // SQLite numbered `?NNN` (`numbered_question`): the `?` abutting an ASCII-digit
            // run, a maximal munch so `?1abc` is `?1` then `abc` (engine-measured). A bare
            // `?` with no following digit stays anonymous. The index's 1..=32766 range is
            // validated at parse-time materialisation (`parse_parameter`), not here.
            if features.parameters.numbered_question
                && cursor.peek().is_some_and(|b| b.is_ascii_digit())
            {
                cursor.eat_while(|b| b.is_ascii_digit());
            }
        }
    }
    Token::new(TokenKind::Parameter, Span::new(start, cursor.pos()))
}

/// Scan a DuckDB `#n` positional column reference: the `#` sigil then its ASCII-digit
/// run. The dispatch guard already confirmed the dialect enables the form and that a
/// digit follows; the 1-based index is recovered from the span at parse time,
/// so nothing is materialized here.
fn scan_positional_column(cursor: &mut Cursor) -> Token {
    let start = cursor.pos();
    cursor.bump(); // '#'
    cursor.eat_while(|b| b.is_ascii_digit());
    Token::new(TokenKind::PositionalColumn, Span::new(start, cursor.pos()))
}

/// Scan a Snowflake stage reference: `@stage`, `@db.schema.stage`, `@~`, `@%table`,
/// each optionally followed by `/path/segments`. The dispatch guard confirmed the
/// form is enabled and that a stage body follows `@`. Path segments stop at
/// whitespace or structural punctuation so `FROM @stage/path,` and `INTO @~/x)`
/// leave the delimiter for the grammar.
fn scan_stage_reference(cursor: &mut Cursor, features: &FeatureSet) -> Token {
    let start = cursor.pos();
    cursor.bump(); // '@'
    match cursor.peek() {
        Some(b'~') => {
            cursor.bump();
        }
        Some(b'%') => {
            cursor.bump();
            // `@%table` — table name after the percent.
            if cursor
                .char_at(0)
                .is_some_and(|ch| is_identifier_start(ch, features))
            {
                eat_identifier_continue(cursor, features);
            }
        }
        _ => {
            // `@name` or `@db.schema.stage` — identifier runs joined by dots.
            eat_identifier_continue(cursor, features);
            while cursor.peek() == Some(b'.')
                && cursor
                    .char_at(1)
                    .is_some_and(|ch| is_identifier_start(ch, features))
            {
                cursor.bump(); // '.'
                eat_identifier_continue(cursor, features);
            }
        }
    }
    // Optional `/path/segments` — path bytes until whitespace or structural stop.
    while cursor.peek() == Some(b'/') {
        cursor.bump(); // '/'
        cursor.eat_while(|b| {
            !b.is_ascii_whitespace()
                && !matches!(b, b',' | b')' | b'(' | b';' | b'=' | b'<' | b'>' | b'!')
        });
    }
    Token::new(TokenKind::StageReference, Span::new(start, cursor.pos()))
}

/// Scan a MySQL session variable: a user variable `@name`, or a system variable
/// `@@name` / `@@global.name` / `@@session.name`. The dispatch guards confirmed the
/// dialect enables the form and that an identifier-start follows the sigil (`@` or
/// `@@`); the leading bytes select which run to eat. A `@@` scan additionally folds
/// an optional `scope.` prefix into the token, so a scoped system variable stays one
/// atomic lexeme and its inner `.` never reaches the punctuation grammar (mirroring
/// how a whole `@name` parameter is one token). The span covers the entire reference;
/// the sigil/scope/name split is recovered from it at parse time, so
/// nothing is materialized here.
fn scan_session_variable(cursor: &mut Cursor, features: &FeatureSet) -> Token {
    let start = cursor.pos();
    cursor.bump(); // first '@'
    if cursor.peek() == Some(b'@') {
        cursor.bump(); // second '@' — a system variable
        eat_identifier_continue(cursor, features); // scope word or the bare name
        // An abutting `.name` makes the first run a `global`/`session` scope and this
        // the variable name (the parser validates the scope word). Consume the `.` only
        // when an identifier-start follows, so a trailing `.` stays punctuation.
        if cursor.peek() == Some(b'.')
            && cursor
                .char_at(1)
                .is_some_and(|ch| is_identifier_start(ch, features))
        {
            cursor.bump(); // '.'
            eat_identifier_continue(cursor, features);
        }
    } else {
        eat_identifier_continue(cursor, features); // user variable '@name'
    }
    Token::new(TokenKind::Variable, Span::new(start, cursor.pos()))
}

/// Scan a delimiter-quoted lexeme: a `'…'`/`"…"` symmetric quote, or an asymmetric
/// identifier quote such as T-SQL `[…]`.
///
/// The cursor sits on the opening delimiter; the [`scan_quoted_body`] loop consumes
/// it. Only the *close* matters: a symmetric quote passes its delimiter, an
/// asymmetric quote passes its close byte (the open is then an ordinary inner byte).
fn scan_quoted(cursor: &mut Cursor, scan: QuoteScan) -> Result<Token, LexError> {
    let start = cursor.pos();
    scan_quoted_body(cursor, start, scan)
}

/// What [`scan_quoted_body`] is scanning: the closing delimiter, the token kind to
/// emit, the error kind if it never closes, and whether `\` escapes the next byte.
struct QuoteScan {
    close: u8,
    kind: TokenKind,
    unterminated: LexErrorKind,
    backslash: bool,
    /// Whether a zero-length `QuotedIdent` body is *accepted* rather than rejected —
    /// SQLite's empty quoted identifier
    /// ([`IdentifierSyntax::empty_quoted_identifiers`](crate::ast::dialect::IdentifierSyntax::empty_quoted_identifiers)).
    /// Inert for `String` kinds (an empty string is always valid).
    allow_zero_length: bool,
}

/// Scan a quote-delimited string introduced by a prefix (`E'…'`, `N'…'`, `U&'…'`, or a
/// MySQL `_charset'…'` / `_charset"…"` introducer).
///
/// `prefix_len` bytes precede the opening quote; the span still starts at the prefix so
/// the literal's exact spelling is recovered later. The fixed forms pass a
/// constant length and the `'` delimiter; a charset introducer passes its measured
/// length and the abutting quote byte ([`charset_introducer_prefix`]) — `'`, or `"`
/// when the dialect strings double quotes.
fn scan_prefixed_string(
    cursor: &mut Cursor,
    prefix_len: u32,
    close: u8,
    backslash: bool,
) -> Result<Token, LexError> {
    let start = cursor.pos();
    cursor.advance_bytes(prefix_len); // prefix byte(s): `E`, `N`, `U&`, or `_charset`
    scan_quoted_body(
        cursor,
        start,
        QuoteScan {
            close,
            kind: TokenKind::String,
            unterminated: LexErrorKind::UnterminatedString,
            backslash,
            allow_zero_length: false,
        },
    )
}

/// Scan a `U&"..."` Unicode-escaped identifier, emitting a [`QuotedIdent`] token that
/// spans the `U&` prefix and the `"…"` body so the exact spelling is recovered later.
///
/// The `U&` prefix (2 bytes) precedes the opening `"`; the shared [`scan_quoted_body`]
/// loop handles the doubled-`""` escape and, because the kind is [`QuotedIdent`], the
/// zero-length-body rejection — so `U&""` fails as a zero-length delimited identifier
/// over the whole `U&""` span (PostgreSQL parity), while `U&""""` is one valid identifier
/// carrying a literal `"`. No escape decoding happens here; the parser folds any trailing
/// `UESCAPE` clause and decodes once (see the dispatch arm's rationale).
///
/// [`QuotedIdent`]: TokenKind::QuotedIdent
fn scan_unicode_ident(cursor: &mut Cursor, features: &FeatureSet) -> Result<Token, LexError> {
    let start = cursor.pos();
    cursor.advance_bytes(2); // the `U&` prefix
    scan_quoted_body(
        cursor,
        start,
        QuoteScan {
            close: b'"',
            kind: TokenKind::QuotedIdent,
            unterminated: LexErrorKind::UnterminatedQuotedIdent,
            backslash: false,
            // `U&""` stays a zero-length reject unless the dialect admits empty quoted
            // identifiers (no shipped `unicode_strings` dialect does, so this is off there).
            allow_zero_length: features.identifier_syntax.empty_quoted_identifiers,
        },
    )
}

/// Scan a SQLite/MySQL `x'53514C'` / `X'53514c'` hexadecimal blob literal, validating
/// eagerly that the body is an *even* number of ASCII hex digits (each pair is one byte)
/// — the tokenize-time rule both engines enforce, where an odd length or a non-hex byte
/// is a syntax error (`x'ABC'`, `x'0'`, `x'XY'`; probed), the opposite of a PG/DuckDB
/// deferred bit-string. The empty body `x''` is a valid zero-byte blob.
///
/// The cursor is on the `x`/`X` marker, whose abutting `'` the dispatch guard confirmed.
/// The body carries no `''` doubling (a blob holds only hex digits), so — unlike
/// [`scan_prefixed_string`] — the first `'` after the hex run terminates it: the scan
/// takes the maximal hex-digit run, then the next byte decides the verdict. The token is
/// a `String` spanning the marker so the parser classifies it as a hex `BitString` and
/// the exact spelling round-trips from the span.
fn scan_hex_blob(cursor: &mut Cursor) -> Result<Token, LexError> {
    let start = cursor.pos();
    cursor.advance_bytes(2); // the `x`/`X` marker and the opening `'`
    let digits_start = cursor.pos();
    cursor.eat_while(|byte| byte.is_ascii_hexdigit());
    let even = (cursor.pos() - digits_start) % 2 == 0;
    if even && cursor.peek() == Some(b'\'') {
        cursor.bump(); // closing quote
        return Ok(Token::new(
            TokenKind::String,
            Span::new(start, cursor.pos()),
        ));
    }
    // Malformed: an odd hex-digit count, a non-hex byte in the body, or no closing quote.
    // Extend the span to the closing quote (if any) so the error covers the whole lexeme,
    // as SQLite's tokenizer recovers by scanning to the matching `'`.
    cursor.eat_while(|byte| byte != b'\'');
    let kind = if cursor.peek() == Some(b'\'') {
        cursor.bump();
        LexErrorKind::MalformedBlobLiteral
    } else {
        LexErrorKind::UnterminatedString
    };
    Err(LexError::new(kind, Span::new(start, cursor.pos())))
}

/// The MySQL charset introducer at the cursor — its byte length (the `_` plus the
/// charset-name run) and the opening quote byte that follows — when the name abuts a
/// quote opening a string (`_utf8mb4'…'` measures `(8, b'\'')`, `_latin1"…"` measures
/// `(7, b'"')`). `None` when the `_` is an ordinary identifier (no name, or no abutting
/// string quote), so the dispatch falls through to [`scan_word`].
///
/// The abutting quote is `'` (always a string), or `"` — but `"` opens a string only
/// where the dialect treats double quotes as string literals (`double_quoted_strings`,
/// MySQL `ANSI_QUOTES` off). Under `ANSI_QUOTES` on, `"` quotes an *identifier*, so the
/// introducer does not apply: this returns `None`, the guard falls false, and `_name`
/// then `"…"` lex as an identifier and a quoted identifier — mirroring the string-vs-
/// identifier decision the top-level `b'"'` dispatch arm makes on the same knob.
///
/// The charset name is a non-empty *ASCII* identifier-continue run. MySQL charset
/// names are ASCII (`utf8mb4`, `latin1`, …), so a non-ASCII byte ends the run and the
/// `_…` is left to lex as an ordinary identifier. This ASCII rule matches the AST
/// crate's `charset_introducer_quote_offset`, so the eager lex and the lazy body split
/// recognise the same introducer and can never disagree — the same eager/
/// lazy contract the dollar-quote tag predicates keep.
fn charset_introducer_prefix(cursor: &Cursor, features: &FeatureSet) -> Option<(u32, u8)> {
    // The dispatch guard placed the cursor on `_`; the charset name runs over the
    // ASCII identifier-continue bytes after it.
    let mut len = 1; // the leading `_`
    while cursor
        .peek_nth(len)
        .is_some_and(|byte| byte < 0x80 && is_identifier_continue(byte as char, features))
    {
        len += 1;
    }
    // The name must be non-empty (len > 1) and abut a string-opening quote.
    if len == 1 {
        return None;
    }
    match cursor.peek_nth(len) {
        Some(b'\'') => Some((len, b'\'')),
        Some(b'"') if features.string_literals.double_quoted_strings => Some((len, b'"')),
        _ => None,
    }
}

/// The shared quote-scanning loop. The cursor is on the opening delimiter (consumed
/// here); `start` is where the whole lexeme began, before any prefix.
///
/// A doubled close (`''` / `""` / `]]`) is an in-literal escape, not the terminator;
/// with `backslash`, a `\` likewise escapes the following byte (MySQL string
/// semantics). Escapes are recognised for *termination* only — never unfolded here
/// (deferred to lazy materialization), so the span covers the delimiters
/// and any raw escapes verbatim.
///
/// A `QuotedIdent` scan additionally rejects a zero-length body: SQL's `<delimited
/// identifier body>` requires at least one character, and PostgreSQL/MySQL both
/// reject an empty delimited identifier at scan time — the same for every configured
/// style (`"…"`, backtick, asymmetric `[…]`), *unless* the dialect sets
/// [`IdentifierSyntax::empty_quoted_identifiers`](crate::ast::dialect::IdentifierSyntax::empty_quoted_identifiers)
/// (SQLite, which admits every empty quoted style), carried on `scan.allow_zero_length`.
/// The check is keyed on `scan.kind`, not on which caller
/// reached this loop, because this same loop also scans `String` bodies (`''`, or
/// `"…"` under MySQL `double_quoted_strings`) — an empty *string* is ordinary valid
/// SQL and must never trip it. `body_start` marks the position right after the
/// opening delimiter; it is what tells a truly-empty body (`""`) apart from the
/// doubled-close escape (`""""`, one literal `"`): the doubled-close arm below always
/// advances the cursor past `body_start` before the terminator arm can be reached, so
/// landing on the terminator with the cursor still *at* `body_start` can only mean
/// the close followed the open with nothing in between.
fn scan_quoted_body(cursor: &mut Cursor, start: u32, scan: QuoteScan) -> Result<Token, LexError> {
    cursor.bump(); // opening delimiter
    let body_start = cursor.pos();

    loop {
        match cursor.peek() {
            None => {
                return Err(LexError::new(
                    scan.unterminated,
                    Span::new(start, cursor.pos()),
                ));
            }
            Some(b'\\') if scan.backslash => {
                cursor.bump(); // backslash
                if cursor.bump().is_none() {
                    return Err(LexError::new(
                        scan.unterminated,
                        Span::new(start, cursor.pos()),
                    ));
                }
            }
            Some(byte) if byte == scan.close => {
                if cursor.peek_nth(1) == Some(scan.close) {
                    cursor.bump(); // doubled close: an escaped literal delimiter
                    cursor.bump();
                } else {
                    // Decide before consuming the close: an empty body means the
                    // cursor never moved past `body_start` (see the function doc).
                    let zero_length_ident = scan.kind == TokenKind::QuotedIdent
                        && !scan.allow_zero_length
                        && cursor.pos() == body_start;
                    cursor.bump(); // closing delimiter
                    let span = Span::new(start, cursor.pos());
                    return if zero_length_ident {
                        Err(LexError::new(
                            LexErrorKind::ZeroLengthDelimitedIdentifier,
                            span,
                        ))
                    } else {
                        Ok(Token::new(scan.kind, span))
                    };
                }
            }
            Some(_) => {
                cursor.bump();
            }
        }
    }
}

/// Scan a PostgreSQL dollar-quoted string, `$tag$ … $tag$` (the tag may be
/// empty, `$$ … $$`). The body is taken verbatim up to the next occurrence of
/// the exact opening delimiter — there is no escape processing, which is the
/// whole point of dollar-quoting. A `$` that does not open a valid delimiter is
/// a stray byte (M1 does not lex `$n` positional parameters).
fn scan_dollar_quote(cursor: &mut Cursor, features: &FeatureSet) -> Result<Token, LexError> {
    let start = cursor.pos();
    // `src()` hands back the `&'a str` itself, so `delim` below borrows the
    // source, not the cursor, and the cursor stays mutable while we scan.
    let bytes = cursor.src().as_bytes();
    let opener_start = start as usize; // the '$'

    // Tag: empty, or a tag-name with no embedded '$' (PostgreSQL's rule). The tag
    // keeps the byte-oriented rule (any high byte accepted), not the Unicode
    // identifier policy — see [`is_dollar_tag_start`].
    let mut index = opener_start + 1;
    if bytes
        .get(index)
        .is_some_and(|&b| is_dollar_tag_start(b, features))
    {
        index += 1;
        while bytes
            .get(index)
            .is_some_and(|&b| is_dollar_tag_continue(b, features))
        {
            index += 1;
        }
    }

    // A valid opener closes the tag with '$'; otherwise the '$' opens nothing.
    if bytes.get(index) != Some(&b'$') {
        return Err(LexError::new(
            LexErrorKind::StrayByte,
            Span::new(start, start + 1),
        ));
    }
    let delim = &bytes[opener_start..=index]; // `$tag$`, including the trailing '$'

    cursor.advance_bytes(delim.len() as u32); // consume the opening delimiter

    // Scan to the next `$` candidate first (a tight single-byte position scan,
    // the same shape `scan_quoted_body` uses for its close byte) instead of
    // re-running `starts_with(delim)` — an up-to-`delim.len()`-byte comparison —
    // at every body position; that turned this scan into O(body_len * tag_len).
    // Only a `$` can start the closing delimiter, so every non-`$` byte is
    // skippable in bulk with no per-byte delimiter check at all.
    loop {
        let rest = cursor.rest();
        match rest.iter().position(|&b| b == b'$') {
            Some(offset) => {
                cursor.advance_bytes(offset as u32); // land on the candidate '$'
                if cursor.rest().starts_with(delim) {
                    cursor.advance_bytes(delim.len() as u32); // consume the closing delimiter
                    return Ok(Token::new(
                        TokenKind::String,
                        Span::new(start, cursor.pos()),
                    ));
                }
                // Not a close. Resume just past *this* '$', not past the whole
                // failed `delim.len()`-byte match: the true terminator can start
                // inside a near-miss candidate's tail (e.g. body `...$ta` ahead of
                // the real `$tag$`), so skipping the full match width could step
                // over it.
                cursor.bump();
            }
            None => {
                // No '$' anywhere in the rest of the source: the body runs off
                // the end unterminated. Land the cursor at EOF so the error span
                // matches the byte-at-a-time scan this replaces.
                cursor.advance_bytes(rest.len() as u32);
                return Err(LexError::new(
                    LexErrorKind::UnterminatedDollarQuote,
                    Span::new(start, cursor.pos()),
                ));
            }
        }
    }
}

/// Scan an operator. Two-byte operators (`<=`, `>=`, `<>`, `!=`, `||`) take
/// priority over their one-byte prefixes; every other operator-class byte is a
/// one-byte operator. Maximal munch beyond these fixed forms is intentionally
/// not attempted in M1.
///
/// `=>` is the one operator gated by dialect data: it is munched as the
/// named-argument [`Arrow`](Operator::Arrow) only when the dialect enables named
/// arguments, so under other dialects `=>` stays a lone `=` followed by `>`.
fn scan_operator(cursor: &mut Cursor, features: &FeatureSet) -> Token {
    use Operator::{
        Amp, AmpAmp, Arrow, AtAt, AtGt, AtQuestion, Bang, Caret, CaretAt, Concat, Eq, EqEq, Gt,
        GtEq, Hash, HashGt, HashGtGt, HashMinus, Lt, LtAt, LtEq, LtEqGt, Minus, MinusGt, MinusGtGt,
        NotEq, Percent, Pipe, PipeArrow, Plus, Question, QuestionAmp, QuestionPipe, ShiftLeft,
        ShiftRight, Slash, SlashSlash, Star, Tilde,
    };

    // The general symbolic-operator surface (`custom_operators`): one maximal-munch scan
    // over the `Op` character class, classifying a run that matches no built-in spelling as
    // `Operator::Custom`. A dialect-neutral capability driven by the flag (PostgreSQL is the
    // current enabler, and the maximal-munch rule follows its `scan.l`); gated so every
    // dialect that leaves it off keeps the fixed-form scanner below untouched — a token
    // reaching the fixed arms means `custom_operators` is off.
    if features.operator_syntax.custom_operators {
        return scan_custom_operator(cursor, features);
    }

    let start = cursor.pos();
    let lead = cursor
        .peek()
        .expect("dispatch routes only operator-class bytes to scan_operator");

    let (operator, width): (Operator, u32) = match lead {
        b'+' => (Plus, 1),
        // PostgreSQL JSON access: `->>` (as text) before `->` (as json) before a bare
        // `-`, gated by dialect data. The maximal munch is over contiguous bytes only,
        // so `a - > b` (spaced) stays `-` then `>`.
        b'-' if features.operator_syntax.json_arrow_operators
            && cursor.peek_nth(1) == Some(b'>') =>
        {
            match cursor.peek_nth(2) {
                Some(b'>') => (MinusGtGt, 3),
                _ => (MinusGt, 2),
            }
        }
        b'-' => (Minus, 1),
        b'*' => (Star, 1),
        // DuckDB's `//` integer-division spelling, gated by dialect data. The maximal
        // munch takes the doubled `/` ahead of a lone `/`; elsewhere `a // b` stays `/`
        // then `/`. No preset lexes `//` as a line comment, so the doubled munch never
        // shadows a comment mode.
        b'/' if features.operator_syntax.integer_divide_slash
            && cursor.peek_nth(1) == Some(b'/') =>
        {
            (SlashSlash, 2)
        }
        b'/' => (Slash, 1),
        b'%' => (Percent, 1),
        b'=' if features.call_syntax.named_argument && cursor.peek_nth(1) == Some(b'>') => {
            (Arrow, 2)
        }
        // SQLite's `==` equality spelling, gated by dialect data. The maximal munch
        // takes the doubled `=` ahead of a lone `=`; elsewhere `a == b` stays `=` then
        // `=` and surfaces as a clean parse error (a bare `=` cannot follow another).
        b'=' if features.operator_syntax.double_equals && cursor.peek_nth(1) == Some(b'=') => {
            (EqEq, 2)
        }
        b'=' => (Eq, 1),
        b'<' => match cursor.peek_nth(1) {
            // MySQL `<=>` null-safe equality, gated by dialect data. The 3-byte munch
            // precedes `<=` so `<=>` wins over `<=` then `>`; elsewhere `<=>` stays `<=`
            // then `>` (maximal munch over contiguous bytes only, so `< = >` spaced does
            // not combine).
            Some(b'=')
                if features.operator_syntax.null_safe_equals
                    && cursor.peek_nth(2) == Some(b'>') =>
            {
                (LtEqGt, 3)
            }
            Some(b'=') => (LtEq, 2),
            Some(b'>') => (NotEq, 2),
            // Bitwise left shift `<<`, munched unconditionally (no dialect spells two
            // adjacent `<` any other way); the parser gate decides if it is an operator.
            Some(b'<') => (ShiftLeft, 2),
            // PostgreSQL `<@` "contained by", gated by dialect data; otherwise `<` then
            // whatever `@` lexes as (a stray byte outside PostgreSQL).
            Some(b'@') if features.operator_syntax.containment_operators => (LtAt, 2),
            _ => (Lt, 1),
        },
        b'>' => match cursor.peek_nth(1) {
            Some(b'=') => (GtEq, 2),
            // Bitwise right shift `>>`, munched unconditionally like `<<` above.
            Some(b'>') => (ShiftRight, 2),
            _ => (Gt, 1),
        },
        // PostgreSQL `@`-lead operators. `@` reaches the scanner only through a dispatch arm
        // that already validated the second byte: containment routes `@>`, and the `jsonb`
        // arm routes `@?` / `@@` — so the follow byte selects the operator and no other value
        // is reachable.
        b'@' => match cursor.peek_nth(1) {
            Some(b'>') => (AtGt, 2),
            Some(b'?') => (AtQuestion, 2),
            Some(b'@') => (AtAt, 2),
            _ => unreachable!("the `@` dispatch arms route only `@>`/`@?`/`@@` to scan_operator"),
        },
        // PostgreSQL's `#`-lead operators. `#` reaches the scanner only under `hash_bitwise_xor`
        // (the dispatch arm), so a bare `#` is always the XOR operator. Under the `jsonb`
        // operators the contiguous `#>` / `#>>` / `#-` are munched ahead of the bare `#`
        // (engine-verified maximal munch: `5#-3` is `5 #- 3`, but a space splits `#` from `-`);
        // the `#`+digit positional column split by follow byte in the dispatch, not here. The
        // sibling geometric `##` would extend this same match.
        b'#' if features.operator_syntax.jsonb_operators => match cursor.peek_nth(1) {
            Some(b'>') => match cursor.peek_nth(2) {
                Some(b'>') => (HashGtGt, 3),
                _ => (HashGt, 2),
            },
            Some(b'-') => (HashMinus, 2),
            _ => (Hash, 1),
        },
        b'#' => (Hash, 1),
        // PostgreSQL's `jsonb` existence operators. `?` reaches the scanner only through the
        // `jsonb`-gated dispatch arm, so no feature re-check is needed; the follow byte selects
        // `?|` (any-key) or `?&` (all-keys) ahead of the bare `?` (key), maximal munch over
        // contiguous bytes.
        b'?' => match cursor.peek_nth(1) {
            Some(b'|') => (QuestionPipe, 2),
            Some(b'&') => (QuestionAmp, 2),
            _ => (Question, 1),
        },
        b'!' => match cursor.peek_nth(1) {
            Some(b'=') => (NotEq, 2),
            _ => (Bang, 1),
        },
        // BigQuery/ZetaSQL `|>` query pipe separator, gated by dialect data. Munched over
        // the two contiguous bytes ahead of a lone `|`, beside `||`; with the gate off the
        // bytes stay `|` then `>` exactly as before, so no dialect's `|`/`||` lexing shifts
        // (the feature-gated maximal-munch idiom `->`/`<=>`/`//` use). The PG geometric
        // `|>>` operator is not lexed by this parser, so there is no munch to contend with.
        b'|' => match cursor.peek_nth(1) {
            Some(b'|') => (Concat, 2),
            Some(b'>') if features.query_tail_syntax.pipe_syntax => (PipeArrow, 2),
            _ => (Pipe, 1),
        },
        b'&' => match cursor.peek_nth(1) {
            Some(b'&') => (AmpAmp, 2),
            _ => (Amp, 1),
        },
        b'^' => match cursor.peek_nth(1) {
            Some(b'@') if features.operator_syntax.starts_with_operator => (CaretAt, 2),
            _ => (Caret, 1),
        },
        b'~' => (Tilde, 1),
        _ => unreachable!("dispatch routes only operator-class bytes to scan_operator"),
    };

    cursor.advance_bytes(width); // the matched arm confirmed all `width` bytes
    Token::new(
        TokenKind::Operator(operator),
        Span::new(start, cursor.pos()),
    )
}

/// Scan one general symbolic operator (`custom_operators`): the maximal-munch run of
/// `Op`-class bytes at the cursor (per [`custom_operator_run_len`]), classified onto its
/// built-in [`Operator`] spelling if it has one, else the general
/// [`Operator::Custom`] carrying its span.
///
/// The dialect-neutral maximal-munch operator rule (any dialect enabling
/// [`OperatorSyntax::custom_operators`](crate::ast::dialect::OperatorSyntax::custom_operators)
/// gets it; PostgreSQL is the current enabler and the rule follows its `scan.l`): any run of
/// the `Op` character class is one operator, so `&<`, `<->`, `<<|`, `|>>`, `^@`, `##`, `*<>`,
/// `@#@`, and the regex `!~`/`~*`/`!~*` all lex as a single token, while `a & b` (bitwise)
/// and `a <-> b` (distance) split by the run boundary exactly as the engine does. Every
/// built-in operator still resolves to its own variant (so acceptance and shape are unchanged
/// for currently-parsed input); only the residue — an operator with no dedicated key —
/// becomes `Custom`.
fn scan_custom_operator(cursor: &mut Cursor, features: &FeatureSet) -> Token {
    let start = cursor.pos();
    let n = custom_operator_run_len(cursor, features);
    let run = &cursor.src()[start as usize..(start + n) as usize];
    let (operator, width) = match known_operator_token(run, features) {
        Some(operator) => (operator, n),
        // `=>` is a RESERVED lexeme — the named-argument arrow — never a general operator
        // (PostgreSQL forbids `=>` as a user operator and rejects it in infix position). With
        // named arguments off it is therefore NOT a `Custom` operator; it splits into `=` then
        // `>` (the comparison bytes), exactly as the fixed scanner leaves it, so the trailing
        // `>` re-lexes on its own. Every other unrecognized run is a genuine custom operator.
        None if run == "=>" => (Operator::Eq, 1),
        None => (Operator::Custom, n),
    };
    cursor.advance_bytes(width); // confirmed above (a subset of the `n` munched bytes)
    Token::new(
        TokenKind::Operator(operator),
        Span::new(start, cursor.pos()),
    )
}

/// A byte of the symbolic-operator character class (the `Op`-class of PostgreSQL's `scan.l`,
/// the reference for the general operator rule): the characters a symbolic operator is
/// built from.
///
/// `#` and `?` are `Op`-class bytes in PostgreSQL's `scan.l`, but a DuckDB-shaped dialect
/// repurposes them — `#` as the positional-column sigil (`#1`,
/// [`ExpressionSyntax::positional_column`](crate::ast::dialect::ExpressionSyntax)) and `?`
/// as the anonymous parameter placeholder
/// ([`ParameterSyntax::anonymous_question`](crate::ast::dialect::ParameterSyntax)) — so they
/// leave the operator class there. A run stops at such a byte, matching DuckDB 1.5.4:
/// `1 @#@ 2` is `@` then a stray `#` (reject), not one `@#@` operator, and `1 &#& 2`
/// likewise. The exclusion rides the sigil flag that claims the byte, so PostgreSQL (which
/// sets neither) keeps `#`/`?` in its operator runs (`1 @#@ 2` parses on pg_query).
fn is_operator_char(byte: u8, features: &FeatureSet) -> bool {
    match byte {
        b'#' => !features.expression_syntax.positional_column,
        b'?' => !features.parameters.anonymous_question,
        b'~' | b'!' | b'@' | b'^' | b'&' | b'|' | b'`' | b'+' | b'-' | b'*' | b'/' | b'%'
        | b'<' | b'>' | b'=' => true,
        _ => false,
    }
}

/// A byte whose presence lets a multi-character operator END in `+`/`-` (the general
/// operator rule, per PostgreSQL's `scan.l`): if a run holds none of these, its trailing
/// `+`/`-` are stripped.
fn operator_char_allows_trailing_sign(byte: u8) -> bool {
    matches!(
        byte,
        b'~' | b'!' | b'@' | b'#' | b'^' | b'&' | b'|' | b'`' | b'?' | b'%'
    )
}

/// The byte length of the maximal symbolic operator at the cursor, applying the general
/// operator rule's two truncations (as PostgreSQL's `scan.l` specifies them). The lead byte
/// is always taken (the dispatch routes only an op-class, non-comment-start lead), so the
/// result is at least 1.
///
/// 1. **Embedded comment start:** an operator stops before an embedded `--` line comment or
///    `/*` block comment (`a <-- b` is `a <` then a `--` comment, not a `<--` operator).
/// 2. **Trailing sign:** a multi-character operator may end in `+`/`-` only if it holds one
///    of `~ ! @ # ^ & | ` ? %`; otherwise the trailing `+`/`-` are stripped and re-lexed
///    (`a +- b` is `a + (- b)`, but `a @- b` is one `@-` operator — engine-measured).
fn custom_operator_run_len(cursor: &Cursor, features: &FeatureSet) -> u32 {
    // The lead byte is guaranteed op-class and not a comment start (skip_trivia already
    // consumed any `--`/`/*` beginning at the cursor), so it is always part of the operator.
    let mut n: u32 = 1;
    while let Some(byte) = cursor.peek_nth(n) {
        if !is_operator_char(byte, features) {
            break;
        }
        // A `--`/`/*` beginning at this byte starts a comment, ending the operator here.
        let next = cursor.peek_nth(n + 1);
        if (byte == b'-' && next == Some(b'-')) || (byte == b'/' && next == Some(b'*')) {
            break;
        }
        n += 1;
    }
    let has_sign_char = (0..n).any(|i| {
        cursor
            .peek_nth(i)
            .is_some_and(operator_char_allows_trailing_sign)
    });
    if !has_sign_char {
        while n > 1 && matches!(cursor.peek_nth(n - 1), Some(b'+') | Some(b'-')) {
            n -= 1;
        }
    }
    n
}

/// Map an operator run to its built-in [`Operator`] spelling, or `None` for a general
/// custom operator. Every fixed form the ordinary [`scan_operator`] arms produce is
/// mirrored here (single source of truth for the maximal-munch path): the always-lexed
/// comparison/shift/concat/`&&` forms unconditionally, the dialect-gated forms (`@>`, `->`,
/// the `jsonb` family, `=>`, `|>`) only when their gate is on — matching the fixed scanner,
/// so a run reaching this under the wrong gate falls through to `Custom` exactly as the
/// fixed arms would leave the bytes unrecognised. A run matching nothing here is the
/// residue (`&<`, `<->`, `!~`, `@#@`, …) the parser reads as a named operator.
fn known_operator_token(run: &str, features: &FeatureSet) -> Option<Operator> {
    use Operator::{
        Amp, AmpAmp, Arrow, AtAt, AtGt, AtQuestion, Bang, Caret, CaretAt, Concat, Eq, EqEq, Gt,
        GtEq, Hash, HashGt, HashGtGt, HashMinus, Lt, LtAt, LtEq, LtEqGt, Minus, MinusGt, MinusGtGt,
        NotEq, Percent, Pipe, PipeArrow, Plus, Question, QuestionAmp, QuestionPipe, ShiftLeft,
        ShiftRight, Slash, SlashSlash, Star, Tilde,
    };
    let op = match run.as_bytes() {
        // Single-char "self" and generic-`Op` bytes with a dedicated token. `~`/`!`/`#`/`&`/
        // `|` carry a token whose *infix* meaning the parser decides (a lone `~` is prefix
        // bitwise-NOT or infix regex; `#`/`&`/`|` are bitwise). `^` is exponentiation here.
        b"+" => Plus,
        b"-" => Minus,
        b"*" => Star,
        b"/" => Slash,
        b"%" => Percent,
        b"^" => Caret,
        b"<" => Lt,
        b">" => Gt,
        b"=" => Eq,
        b"~" => Tilde,
        b"!" => Bang,
        b"&" => Amp,
        b"|" => Pipe,
        b"#" if features.hash_bitwise_xor => Hash,
        // Always-lexed multi-character forms (no dialect spells these any other way).
        b"<=" => LtEq,
        b">=" => GtEq,
        b"<>" | b"!=" => NotEq,
        b"<<" => ShiftLeft,
        b">>" => ShiftRight,
        b"||" => Concat,
        b"&&" => AmpAmp,
        // Dialect-gated forms — the dedicated token only under the gate that lexes it in the
        // fixed scanner, else `Custom` (never over-recognising).
        b"?" if features.operator_syntax.jsonb_operators => Question,
        b"?|" if features.operator_syntax.jsonb_operators => QuestionPipe,
        b"?&" if features.operator_syntax.jsonb_operators => QuestionAmp,
        b"@?" if features.operator_syntax.jsonb_operators => AtQuestion,
        b"@@" if features.operator_syntax.jsonb_operators => AtAt,
        b"#>" if features.operator_syntax.jsonb_operators => HashGt,
        b"#>>" if features.operator_syntax.jsonb_operators => HashGtGt,
        b"#-" if features.operator_syntax.jsonb_operators => HashMinus,
        b"@>" if features.operator_syntax.containment_operators => AtGt,
        b"<@" if features.operator_syntax.containment_operators => LtAt,
        b"->" if features.operator_syntax.json_arrow_operators => MinusGt,
        b"->>" if features.operator_syntax.json_arrow_operators => MinusGtGt,
        b"=>" if features.call_syntax.named_argument => Arrow,
        b"|>" if features.query_tail_syntax.pipe_syntax => PipeArrow,
        // The remaining dialect-gated symbol spellings. None is enabled together with
        // `custom_operators` in a shipped preset today (PostgreSQL has none of them), so these
        // arms are dormant under the current presets — but they keep this classifier complete
        // for any dialect that later pairs the general operator surface with these operators
        // (so `==`/`//`/`<=>` stay their dedicated tokens rather than becoming `Custom`).
        b"==" if features.operator_syntax.double_equals => EqEq,
        b"//" if features.operator_syntax.integer_divide_slash => SlashSlash,
        b"<=>" if features.operator_syntax.null_safe_equals => LtEqGt,
        b"^@" if features.operator_syntax.starts_with_operator => CaretAt,
        _ => return None,
    };
    Some(op)
}

/// Scan a structural punctuation lexeme.
///
/// Every punctuation byte is one token except `:`, which munches `::` (the
/// PostgreSQL typecast operator) ahead of a lone `:` (the array-slice separator),
/// mirroring the two-byte-over-one-byte priority [`scan_operator`] applies. Under a
/// dialect that enables named arguments, `:=` munches ahead of a lone `:` into the
/// deprecated named-argument separator [`ColonEquals`](Operator::ColonEquals) — an
/// operator lexeme reached through the punctuation scanner because its `:` lead byte
/// is punctuation-class (the mirror of `::`, a typecast operator scanned here too).
fn scan_punctuation(cursor: &mut Cursor, features: &FeatureSet) -> Token {
    use Punctuation::{
        Colon, Comma, Dot, DoubleColon, LBrace, LBracket, LParen, RBrace, RBracket, RParen,
        Semicolon,
    };

    let start = cursor.pos();
    let byte = cursor
        .peek()
        .expect("dispatch routes only punctuation-class bytes to scan_punctuation");

    // `::` is the one two-byte punctuation; take it before consuming a lone `:`.
    if byte == b':' && cursor.peek_nth(1) == Some(b':') {
        cursor.bump();
        cursor.bump();
        return Token::new(
            TokenKind::Punctuation(DoubleColon),
            Span::new(start, cursor.pos()),
        );
    }

    // `:=` munches ahead of a lone `:` (like `::` above) under either behaviour that owns
    // it: PostgreSQL's deprecated named-argument separator (`call_syntax.named_argument`) or
    // MySQL's `SET_VAR` variable-assignment operator (`session_variables.variable_assignment`).
    // The two never coexist in a shipped preset, so the shared [`Operator::ColonEquals`] token
    // is unambiguous per dialect.
    if byte == b':'
        && (features.call_syntax.named_argument || features.session_variables.variable_assignment)
        && cursor.peek_nth(1) == Some(b'=')
    {
        cursor.bump();
        cursor.bump();
        return Token::new(
            TokenKind::Operator(Operator::ColonEquals),
            Span::new(start, cursor.pos()),
        );
    }

    cursor.bump();
    let punctuation = match byte {
        b'(' => LParen,
        b')' => RParen,
        b',' => Comma,
        b';' => Semicolon,
        b'.' => Dot,
        b'[' => LBracket,
        b']' => RBracket,
        b'{' => LBrace,
        b'}' => RBrace,
        b':' => Colon,
        _ => unreachable!("dispatch routes only punctuation-class bytes to scan_punctuation"),
    };
    Token::new(
        TokenKind::Punctuation(punctuation),
        Span::new(start, cursor.pos()),
    )
}

/// Whether `ch` may **begin** an unquoted identifier under this dialect.
///
/// The explicit, Unicode-aware identifier policy, mirroring PostgreSQL —
/// the M1 reference dialect: a Unicode *letter* (`char::is_alphabetic`) or `_`. The
/// ASCII range is decided by the dialect byte-class table, which already encodes
/// "letter or `_`" and is where a dialect adds extra ASCII identifier bytes (T-SQL
/// `#`/`@`); only a non-ASCII lead character pays for the Unicode property test, so
/// the hot ASCII path stays a single table lookup. `$` never *starts* an identifier
/// (a leading `$` opens a parameter or dollar-quote).
///
/// This is deliberately the Unicode *letter* property, not PostgreSQL's raw "any
/// high byte" lexer rule: a Unicode-aware tokenizer should reject a non-letter code
/// point (an emoji, a symbol, a lone combining mark) as an identifier start rather
/// than silently folding it into a word. We do **not** XID-classify or
/// NF-normalize: characters are taken as written, and case folding for identity is a
/// separate concern ([`Casing`](crate::ast::dialect::Casing)).
fn is_identifier_start(ch: char, features: &FeatureSet) -> bool {
    if ch.is_ascii() {
        features.has_byte_class(ch as u8, CLASS_IDENTIFIER_START)
    } else {
        ch.is_alphabetic()
    }
}

/// Whether `ch` may **continue** an unquoted identifier, *excluding* the
/// dialect-variable `$` (applied by [`eat_identifier_continue`]).
///
/// Identifier-start characters plus a Unicode *digit*: for non-ASCII this is
/// `char::is_alphanumeric` (letter or number) and for ASCII the byte-class table's
/// continue set (letters, digits, `_`, and any byte the dialect adds).
fn is_identifier_continue(ch: char, features: &FeatureSet) -> bool {
    if ch.is_ascii() {
        features.has_byte_class(ch as u8, CLASS_IDENTIFIER_CONTINUE)
    } else {
        ch.is_alphanumeric()
    }
}

/// Advance the cursor over the identifier-continue run tailing an unquoted
/// identifier or a `:name`/`@name` parameter name.
///
/// ASCII bytes take the byte-class table fast path with no char decode; a non-ASCII
/// byte is decoded to one whole `char` and classified by Unicode property, so the
/// cursor always stops on a char boundary. `$` is accepted only where the dialect
/// sets `identifier_syntax.dollar_in_identifiers` (PostgreSQL/Oracle `foo$bar`; off
/// in strict ANSI) — the one part of the policy that genuinely varies by dialect.
/// Dollar-quote *tags* deliberately do not use this: there `$` is the delimiter, not
/// an identifier character (see [`is_dollar_tag_continue`]).
fn eat_identifier_continue(cursor: &mut Cursor, features: &FeatureSet) {
    let dollar = features.identifier_syntax.dollar_in_identifiers;
    while let Some(byte) = cursor.peek() {
        if byte < 0x80 {
            // ASCII fast path: byte-class table, plus the dialect-variable `$`. No
            // char decode on the common path.
            if !(is_identifier_continue(byte as char, features) || (dollar && byte == b'$')) {
                return;
            }
            cursor.bump();
        } else {
            // Non-ASCII: decode the whole char and classify by Unicode property.
            let ch = cursor
                .char_at(0)
                .expect("a non-ASCII lead byte begins a valid UTF-8 char in &str source");
            if !is_identifier_continue(ch, features) {
                return;
            }
            cursor.advance_bytes(ch.len_utf8() as u32); // the whole decoded char
        }
    }
}

/// Whether `byte` may **begin** a `$tag$` dollar-quote tag.
///
/// Dollar-quote tags keep PostgreSQL's *byte-oriented* rule (`[A-Za-z\200-\377_]`):
/// any high byte is a tag character verbatim, with no Unicode letter classification.
/// This is intentionally not the Unicode identifier policy above — it matches
/// PostgreSQL (which accepts any high byte in a tag) and, crucially, keeps the tag
/// boundary this tokenizer finds identical to the one the AST materializer re-derives
/// from the same token text ([`literal::is_dollar_tag_start`]), so the eager lex and
/// the lazy body extraction can never disagree. `$` is excluded — it
/// closes the tag.
///
/// [`literal::is_dollar_tag_start`]: crate::ast
fn is_dollar_tag_start(byte: u8, features: &FeatureSet) -> bool {
    features.has_byte_class(byte, CLASS_IDENTIFIER_START) || byte >= 0x80
}

/// Whether `byte` may **continue** a `$tag$` dollar-quote tag: the byte rule of
/// [`is_dollar_tag_start`] plus ASCII digits (the shared table's continue set is
/// exactly that union, and excludes `$`).
fn is_dollar_tag_continue(byte: u8, features: &FeatureSet) -> bool {
    features.has_byte_class(byte, CLASS_IDENTIFIER_CONTINUE)
}

/// ASCII decimal digit, via the shared class table.
fn is_digit(byte: u8, features: &FeatureSet) -> bool {
    features.has_byte_class(byte, CLASS_DIGIT)
}

/// The close byte for the identifier quote style whose opening delimiter is `byte`,
/// or `None` if `byte` opens no configured style. The first matching style wins, so
/// dialects order `identifier_quotes` by precedence. Only ASCII delimiters match
/// (every SQL quote delimiter is ASCII); a non-ASCII configured delimiter is inert.
fn opening_identifier_quote(features: &FeatureSet, byte: u8) -> Option<u8> {
    features.identifier_quotes.iter().find_map(|style| {
        let (open, close) = (style.open(), style.close());
        (open.is_ascii() && open as u8 == byte && close.is_ascii()).then_some(close as u8)
    })
}
