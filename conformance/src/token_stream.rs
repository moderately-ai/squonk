// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Token-stream differential oracle for the hand-written tokenizer (ADR-0005).
//!
//! ADR-0005 chose a hand-written byte-offset cursor over `logos` for speed and to
//! express the non-regular cases (`$tag$…$tag$` dollar-quoting, nested block
//! comments). Its own consequences section flags the cost: "a hand cursor risks
//! off-by-one/UTF-8 bugs — mitigated by proptest + fuzz + a differential
//! token-stream oracle (ADR-0014/0015)." This module is that oracle.
//!
//! ## The format
//!
//! [`render_golden`] turns one input's whole token stream into a deterministic,
//! line-oriented text the datadriven goldens pin (REWRITE=1-regenerable, the repo
//! convention). It records, per token, the byte `span`, the [`TokenKind`], and the
//! exact source slice the span covers, and it makes ADR-0005's "trivia is
//! out-of-band but offset-recoverable" claim visible by emitting the skipped
//! whitespace/comment gaps between tokens as explicit `trivia` lines. The
//! interleaving therefore reconstructs the source byte-for-byte, so a reader (and
//! `git diff`) can audit exactly how the cursor partitioned the input.
//!
//! ## The oracle
//!
//! There is no second tokenizer to diff against, so the "differential" reference
//! is a set of invariants checked *independently of the parser* and, for the
//! mechanically-checkable subset, against an independently-implemented reference:
//!
//! - **UTF-8 boundary safety** — every span endpoint lands on a `char` boundary of
//!   the source (the core cursor risk: a span that splits a codepoint panics the
//!   moment any diagnostic slices it).
//! - **Coverage modulo trivia** — spans are non-empty, monotonic, and
//!   non-overlapping, and every gap between them is *re-scanned by an independent
//!   trivia recogniser* (`is_all_trivia`) and confirmed to be pure
//!   whitespace/comments. A dropped or mis-bounded token leaves a non-trivia byte
//!   in a gap, which this catches even though both implementations read the same
//!   dialect class data (ADR-0011) — the control flow is a separate implementation.
//! - **Reconstruction** — the trivia gaps interleaved with the token slices equal
//!   the source exactly.
//! - **Kind/slice agreement** — operators, punctuation, and keywords are checked
//!   against an exhaustive, independently-written spelling table; the other kinds
//!   get a cheap structural check (a `Number` starts with a digit or `.`, a
//!   `QuotedIdent` starts with a configured quote, …). This is the "independently
//!   implemented minimal reference for a subset" ADR-0015 asks for.
//!
//! Any violation is reported as the golden's result, so it fails the datadriven
//! test independently of whether the parser would accept the input, and REWRITE
//! refuses to bake a stream that breaks an invariant. The same checks run over the
//! adversarial corpus as ordinary unit tests and over arbitrary UTF-8 in a Bolero
//! target, so a UTF-8/offset regression fails locally (ADR-0014 acceptance).

use std::fmt::Write as _;

use squonk::tokenizer::{LexError, Operator, Punctuation, Token, TokenKind, tokenize_with};
use squonk_ast::dialect::lex_class::{CLASS_IDENTIFIER_START, CLASS_WHITESPACE};
use squonk_ast::dialect::{
    CommentSyntax, FeatureDelta, FeatureSet, IdentifierQuote, StringLiteralSyntax,
};

/// MySQL-like lexical surface: `#` line comments, the full MySQL comment shape
/// (non-nesting block comments and `/*!…*/` versioned-comment regions —
/// [`CommentSyntax::MYSQL`]), `"…"` strings with backslash escapes, and the
/// backtick identifier quote. Mirrors the tokenizer's own
/// `MYSQL_STRING_FEATURES`/`HASH_COMMENT_FEATURES`/`VERSIONED_COMMENT_FEATURES`
/// test presets so the goldens (and the Bolero arm below) exercise the same
/// dialect data the unit tests do — including the stateful versioned-region
/// scanner path.
const MYSQL: FeatureSet = FeatureSet::ANSI.with(
    FeatureDelta::EMPTY
        .string_literals(StringLiteralSyntax {
            double_quoted_strings: true,
            backslash_escapes: true,
            ..StringLiteralSyntax::ANSI
        })
        .identifier_quotes(&[IdentifierQuote::Symmetric('`')])
        .comment_syntax(CommentSyntax::MYSQL),
);

/// SQL-Server-like lexical surface: the asymmetric `[…]` identifier quote (doubled
/// `]]` escape) alongside `"…"`, plus `N'…'` national strings. Exercises the
/// asymmetric-quote cursor path, which is distinct from the symmetric one.
const MSSQL: FeatureSet = FeatureSet::ANSI.with(
    FeatureDelta::EMPTY
        .identifier_quotes(&[
            IdentifierQuote::Asymmetric {
                open: '[',
                close: ']',
            },
            IdentifierQuote::Symmetric('"'),
        ])
        .string_literals(StringLiteralSyntax {
            national_strings: true,
            ..StringLiteralSyntax::ANSI
        }),
);

/// Map a `tokens-<dialect>` golden directive to its lexical feature set.
fn features_for(directive: &str) -> Option<FeatureSet> {
    match directive {
        "tokens-ansi" => Some(FeatureSet::ANSI),
        "tokens-postgres" => Some(FeatureSet::POSTGRES),
        "tokens-mysql" => Some(MYSQL),
        "tokens-mssql" => Some(MSSQL),
        _ => None,
    }
}

/// Render the token stream for one golden case, or report an invariant violation.
///
/// `directive` selects the dialect feature set; `input` is the raw datadriven
/// block (trimmed, matching the existing goldens' `sql_input`, so offsets line up
/// with the visible SQL rather than the harness's trailing newline). An `Err` is
/// surfaced by the datadriven harness as a test failure, so a real tokenizer bug
/// fails the goldens directly and cannot be regenerated into a passing fixture.
pub fn render_golden(directive: &str, input: &str) -> Result<String, String> {
    let features = features_for(directive)
        .ok_or_else(|| format!("unknown token-stream dialect directive {directive:?}"))?;
    render_token_stream(input.trim(), &features)
}

/// Tokenize `src`, check the oracle invariants, and render the pinned text.
fn render_token_stream(src: &str, features: &FeatureSet) -> Result<String, String> {
    match tokenize_with(src, features) {
        Ok(tokens) => {
            check_token_stream(src, &tokens, features).map_err(|detail| {
                format!("token-stream invariant violated for {src:?}: {detail}")
            })?;
            Ok(render_tokens(src, &tokens))
        }
        Err(error) => {
            check_error_span(src, error).map_err(|detail| {
                format!("lex-error span invariant violated for {src:?}: {detail}")
            })?;
            Ok(render_error(src, error))
        }
    }
}

/// Render an accepted token stream: a `source` line, then the tokens interleaved
/// with their skipped-trivia gaps so every byte is accounted for.
///
/// Slicing here cannot panic: [`check_token_stream`] has already proven every
/// endpoint is an in-bounds char boundary.
fn render_tokens(src: &str, tokens: &[Token]) -> String {
    let mut out = String::new();
    writeln!(out, "source {src:?}").expect("writing to a String is infallible");

    let mut prev_end = 0u32;
    for token in tokens {
        let start = token.span.start();
        let end = token.span.end();
        emit_trivia_gap(&mut out, src, prev_end, start);
        let slice = &src[start as usize..end as usize];
        writeln!(out, "{start}..{end} {:?} {slice:?}", token.kind)
            .expect("writing to a String is infallible");
        prev_end = end;
    }
    emit_trivia_gap(&mut out, src, prev_end, src.len() as u32);

    out
}

/// Emit a `trivia` line for the gap `[from, to)` when it is non-empty.
fn emit_trivia_gap(out: &mut String, src: &str, from: u32, to: u32) {
    if to > from {
        let gap = &src[from as usize..to as usize];
        writeln!(out, "{from}..{to} trivia {gap:?}").expect("writing to a String is infallible");
    }
}

/// Render a fail-fast lexical error: its kind, span, and the offending slice.
fn render_error(src: &str, error: LexError) -> String {
    let (start, end) = (error.span.start(), error.span.end());
    let slice = &src[start as usize..end as usize];
    format!(
        "source {src:?}\nerror {:?} {start}..{end} {slice:?}\n",
        error.kind
    )
}

/// Verify the structural invariants of an accepted token stream.
///
/// This is the oracle proper: it depends only on the token stream and the dialect
/// data, never on the parser, so a span/kind mismatch is reported independently of
/// any parse outcome (ADR-0014 acceptance).
fn check_token_stream(src: &str, tokens: &[Token], features: &FeatureSet) -> Result<(), String> {
    let len = src.len();
    let mut prev_end = 0u32;
    let mut reconstructed = String::with_capacity(len);
    // An open versioned-comment region spans gaps (its body is live tokens), so
    // the trivia re-scan carries the region flag from gap to gap, mirroring the
    // scanner's cross-token state.
    let mut in_region = false;

    for (index, token) in tokens.iter().enumerate() {
        let start = token.span.start();
        let end = token.span.end();

        if start > end {
            return Err(format!(
                "token {index} {:?} has start {start} after end {end}",
                token.kind
            ));
        }
        if end as usize > len {
            return Err(format!(
                "token {index} {:?} span {start}..{end} runs past source length {len}",
                token.kind
            ));
        }
        // The cursor stops only on byte boundaries that are char boundaries
        // (ADR-0005); a span endpoint that splits a codepoint would panic the
        // first diagnostic that slices it, so this is the headline invariant.
        if !src.is_char_boundary(start as usize) {
            return Err(format!(
                "token {index} {:?} start {start} is not a UTF-8 char boundary",
                token.kind
            ));
        }
        if !src.is_char_boundary(end as usize) {
            return Err(format!(
                "token {index} {:?} end {end} is not a UTF-8 char boundary",
                token.kind
            ));
        }
        // A real token always covers at least one byte; the fail-fast tokenizer
        // never emits an empty span or the resilience-only `Unknown` kind.
        if start == end {
            return Err(format!(
                "token {index} {:?} has an empty span {start}..{end}",
                token.kind
            ));
        }
        if start < prev_end {
            return Err(format!(
                "token {index} {:?} starts at {start}, overlapping the previous token ending at {prev_end}",
                token.kind
            ));
        }

        let gap = &src[prev_end as usize..start as usize];
        if !is_trivia_run(gap, features, &mut in_region) {
            return Err(format!(
                "gap {prev_end}..{start} before token {index} is not pure trivia: {gap:?}"
            ));
        }
        reconstructed.push_str(gap);

        let slice = &src[start as usize..end as usize];
        check_kind_slice(token.kind, slice, features)
            .map_err(|detail| format!("token {index} at {start}..{end}: {detail}"))?;
        reconstructed.push_str(slice);

        prev_end = end;
    }

    let tail = &src[prev_end as usize..];
    if !is_trivia_run(tail, features, &mut in_region) {
        return Err(format!(
            "trailing bytes {prev_end}..{len} are not pure trivia: {tail:?}"
        ));
    }
    reconstructed.push_str(tail);
    // The scanner rejects an unterminated versioned region at EOF, so an
    // *accepted* stream can never end with the region flag still set.
    if in_region {
        return Err("token stream ends inside an open versioned-comment region".to_string());
    }

    // Redundant with the per-token checks, but it ties them together cheaply: if
    // it holds, the token slices plus the trivia gaps are exactly the source.
    if reconstructed != src {
        return Err(
            "token slices interleaved with trivia gaps do not reconstruct the source".to_string(),
        );
    }

    Ok(())
}

/// Verify a fail-fast error span is an in-bounds char-boundary range.
///
/// An error span is rendered into diagnostics just like a token span, so the same
/// UTF-8/bounds safety must hold or the diagnostic path panics (ADR-0005).
fn check_error_span(src: &str, error: LexError) -> Result<(), String> {
    let (start, end) = (error.span.start(), error.span.end());
    if start > end {
        return Err(format!("error span has start {start} after end {end}"));
    }
    if end as usize > src.len() {
        return Err(format!(
            "error span {start}..{end} runs past source length {}",
            src.len()
        ));
    }
    if !src.is_char_boundary(start as usize) {
        return Err(format!(
            "error span start {start} is not a UTF-8 char boundary"
        ));
    }
    if !src.is_char_boundary(end as usize) {
        return Err(format!("error span end {end} is not a UTF-8 char boundary"));
    }
    Ok(())
}

/// True when `text` is composed entirely of trivia for `features`, as one
/// self-contained input: any versioned-comment region opened inside must also
/// close inside. The gap-by-gap path is [`is_trivia_run`], which threads the
/// region state across gaps; this wrapper serves the recogniser-agreement test.
#[cfg(test)]
fn is_all_trivia(text: &str, features: &FeatureSet) -> bool {
    let mut in_region = false;
    is_trivia_run(text, features, &mut in_region) && !in_region
}

/// True when `text` is composed entirely of trivia for `features`: ASCII
/// whitespace, `--` and (when enabled) `#` line comments, `/* … */` block
/// comments (nesting per dialect data), and — under `versioned_comments` — the
/// markers and discarded bodies of MySQL `/*!…*/` regions. `in_region` carries
/// the open-region flag across successive gaps: an *included* region's body is
/// live tokens, so its opener and its closing `*/` land in different gaps.
///
/// This is an *independent* re-implementation of [`skip_trivia`]'s recognition (it
/// shares only the dialect class data, ADR-0011), so it is a genuine cross-check on
/// where the cursor placed token boundaries: a token the tokenizer dropped or
/// over-/under-sized leaves a non-trivia byte in a gap, which returns `false` here.
///
/// [`skip_trivia`]: https://docs.rs/squonk (tokenizer::scan::skip_trivia)
fn is_trivia_run(text: &str, features: &FeatureSet, in_region: &mut bool) -> bool {
    let bytes = text.as_bytes();
    let nested = features.comment_syntax.nested_block_comments;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if features.has_byte_class(byte, CLASS_WHITESPACE) {
            index += 1;
        } else if byte == b'-' && bytes.get(index + 1) == Some(&b'-') {
            index = consume_line_comment(bytes, index + 2);
        } else if byte == b'#' && features.comment_syntax.line_comment_hash {
            index = consume_line_comment(bytes, index + 1);
        } else if *in_region && byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
            *in_region = false;
            index += 2;
        } else if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            let versioned = features
                .comment_syntax
                .versioned_comments
                .filter(|_| bytes.get(index + 2) == Some(&b'!'));
            let consumed = match versioned {
                Some(bound) => consume_versioned_marker(bytes, index, bound, nested, in_region),
                None => consume_block_comment(bytes, index, nested),
            };
            match consumed {
                Some(next) => index = next,
                // An unbalanced `/*` cannot occur in a real gap (the tokenizer
                // would have failed the whole scan), so treat it as non-trivia.
                None => return false,
            }
        } else {
            return false;
        }
    }
    true
}

/// Advance past a line comment body: everything up to the next `\n` (left for the
/// whitespace branch) or the end of the slice. Mirrors [`skip_line_comment`].
///
/// [`skip_line_comment`]: https://docs.rs/squonk (tokenizer::scan::skip_line_comment)
fn consume_line_comment(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

/// Advance past a `/* … */` block comment starting at `index`, nesting only when
/// `nested` (dialect data: MySQL closes at the first `*/`). Returns the index
/// just past the close, or `None` if it never closes.
fn consume_block_comment(bytes: &[u8], mut index: usize, nested: bool) -> Option<usize> {
    index += 2; // opening `/*`
    let mut depth = 1u32;
    while index < bytes.len() {
        if nested && bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            depth += 1;
            index += 2;
        } else if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return Some(index);
            }
        } else {
            index += 1;
        }
    }
    None
}

/// Advance past a `/*!` versioned-comment marker starting at `index`, mirroring
/// the scanner's engine-verified rules: read the abutting version digits
/// (exactly five or six form a version; 0–4 are body tokens; from ≥7 the first
/// five count); a version above `bound` discards raw bytes — honouring inner
/// comments — up to the region-level `*/` (which must land in this same gap: a
/// discarded region contains no tokens); otherwise the region opens (or stays
/// open — a flag, not a depth) and `in_region` carries it to the next gap.
fn consume_versioned_marker(
    bytes: &[u8],
    index: usize,
    bound: u32,
    nested: bool,
    in_region: &mut bool,
) -> Option<usize> {
    let mut index = index + 3; // `/*!`
    let mut digits = 0usize;
    while digits < 7 && bytes.get(index + digits).is_some_and(u8::is_ascii_digit) {
        digits += 1;
    }
    let version_len = match digits {
        0..=4 => 0,
        6 => 6,
        _ => 5,
    };
    let mut version: u32 = 0;
    for offset in 0..version_len {
        version = version * 10 + u32::from(bytes[index + offset] - b'0');
    }
    index += version_len;

    if version_len > 0 && version > bound {
        loop {
            match bytes.get(index) {
                Some(b'*') if bytes.get(index + 1) == Some(&b'/') => return Some(index + 2),
                Some(b'/') if bytes.get(index + 1) == Some(&b'*') => {
                    index = consume_block_comment(bytes, index, nested)?;
                }
                Some(_) => index += 1,
                None => return None,
            }
        }
    }
    *in_region = true;
    Some(index)
}

/// Cross-check a token's kind against the source slice it spans.
///
/// Operators, punctuation, and keywords are matched against an exhaustive,
/// independently-written spelling reference (the match is total, so a new variant
/// forces an entry here — the point of an independent reference). The remaining
/// kinds get a cheap structural check on the slice's opener.
fn check_kind_slice(kind: TokenKind, slice: &str, features: &FeatureSet) -> Result<(), String> {
    // Safe: `check_token_stream` rejects empty spans before calling this.
    let first = slice.as_bytes()[0];
    match kind {
        TokenKind::Punctuation(punctuation) => {
            let expected = punctuation_spelling(punctuation);
            if slice != expected {
                return Err(format!(
                    "Punctuation({punctuation:?}) span text {slice:?} is not the expected {expected:?}"
                ));
            }
        }
        TokenKind::Operator(Operator::Custom) => {
            // A general custom operator has no fixed spelling — its span is the exact
            // `Op`-class run the maximal-munch scanner captured. Verify it is a non-empty run
            // of `Op`-class bytes (`~ ! @ # ^ & | ` ? + - * / % < > =`).
            if slice.is_empty() || !slice.bytes().all(is_operator_char) {
                return Err(format!(
                    "Operator(Custom) span text {slice:?} is not a run of Op-class bytes"
                ));
            }
        }
        TokenKind::Operator(operator) => {
            if !operator_spellings(operator).contains(&slice) {
                return Err(format!(
                    "Operator({operator:?}) span text {slice:?} is not a recognised spelling"
                ));
            }
        }
        TokenKind::Keyword(keyword) => {
            if !slice.eq_ignore_ascii_case(keyword.as_str()) {
                return Err(format!(
                    "Keyword({keyword:?}) span text {slice:?} does not fold to canonical {:?}",
                    keyword.as_str()
                ));
            }
        }
        TokenKind::Number => {
            if !(first.is_ascii_digit() || first == b'.') {
                return Err(format!(
                    "Number span text {slice:?} does not begin with a digit or '.'"
                ));
            }
        }
        TokenKind::Parameter => {
            if first != b'$' && first != b'?' {
                return Err(format!(
                    "Parameter span text {slice:?} does not begin with '$' or '?'"
                ));
            }
        }
        TokenKind::PositionalColumn => {
            // `#n`: the `#` sigil then a non-empty all-digit run.
            let digits_ok =
                slice.len() >= 2 && slice.as_bytes()[1..].iter().all(|b| b.is_ascii_digit());
            if first != b'#' || !digits_ok {
                return Err(format!(
                    "PositionalColumn span text {slice:?} is not '#' followed by digits"
                ));
            }
        }
        TokenKind::Variable | TokenKind::StageReference => {
            if first != b'@' {
                return Err(format!(
                    "Variable span text {slice:?} does not begin with '@'"
                ));
            }
        }
        TokenKind::String => {
            // A delimited literal is at least two bytes and opens with a quote, a
            // recognised string prefix (`E`/`N`/`U`), or `$` (dollar-quoting).
            if slice.len() < 2 {
                return Err(format!(
                    "String span text {slice:?} is too short to be a delimited literal"
                ));
            }
            if !matches!(
                first,
                b'\'' | b'"' | b'$' | b'E' | b'e' | b'N' | b'n' | b'U' | b'u'
            ) {
                return Err(format!(
                    "String span text {slice:?} does not begin with a string opener"
                ));
            }
        }
        TokenKind::QuotedIdent => {
            if !is_opening_identifier_quote(features, first) {
                return Err(format!(
                    "QuotedIdent span text {slice:?} does not begin with a configured identifier quote"
                ));
            }
        }
        TokenKind::Word => {
            if !(features.has_byte_class(first, CLASS_IDENTIFIER_START) || first >= 0x80) {
                return Err(format!(
                    "Word span text {slice:?} does not begin with an identifier-start byte"
                ));
            }
        }
        // Reserved for a future resilient driver; the eager fail-fast path reports a
        // stray byte as a `LexError` rather than ever emitting this kind (ADR-0005).
        TokenKind::Unknown => {
            return Err(
                "Unknown token kind must not appear in an eager fail-fast token stream".to_string(),
            );
        }
    }
    Ok(())
}

/// The single canonical spelling of a punctuation sub-kind.
fn punctuation_spelling(punctuation: Punctuation) -> &'static str {
    match punctuation {
        Punctuation::LParen => "(",
        Punctuation::RParen => ")",
        Punctuation::Comma => ",",
        Punctuation::Semicolon => ";",
        Punctuation::Dot => ".",
        Punctuation::LBracket => "[",
        Punctuation::RBracket => "]",
        Punctuation::LBrace => "{",
        Punctuation::RBrace => "}",
        Punctuation::Colon => ":",
        Punctuation::DoubleColon => "::",
        Punctuation::At => "@",
    }
}

/// The accepted spelling(s) of an operator sub-kind. `NotEq` has two
/// (`<>` and `!=`); every other operator is spelled exactly one way.
fn operator_spellings(operator: Operator) -> &'static [&'static str] {
    match operator {
        Operator::Plus => &["+"],
        Operator::Minus => &["-"],
        Operator::Star => &["*"],
        Operator::Slash => &["/"],
        Operator::SlashSlash => &["//"],
        Operator::Percent => &["%"],
        Operator::Eq => &["="],
        Operator::EqEq => &["=="],
        Operator::Lt => &["<"],
        Operator::LtEq => &["<="],
        Operator::Gt => &[">"],
        Operator::GtEq => &[">="],
        Operator::NotEq => &["<>", "!="],
        Operator::LtEqGt => &["<=>"],
        Operator::Concat => &["||"],
        Operator::AmpAmp => &["&&"],
        Operator::Bang => &["!"],
        Operator::Pipe => &["|"],
        Operator::Amp => &["&"],
        Operator::Caret => &["^"],
        Operator::CaretAt => &["^@"],
        Operator::Tilde => &["~"],
        Operator::Arrow => &["=>"],
        Operator::ColonEquals => &[":="],
        Operator::AtGt => &["@>"],
        Operator::LtAt => &["<@"],
        Operator::MinusGt => &["->"],
        Operator::MinusGtGt => &["->>"],
        Operator::ShiftLeft => &["<<"],
        Operator::ShiftRight => &[">>"],
        Operator::Hash => &["#"],
        Operator::PipeArrow => &["|>"],
        Operator::Question => &["?"],
        Operator::QuestionPipe => &["?|"],
        Operator::QuestionAmp => &["?&"],
        Operator::AtQuestion => &["@?"],
        Operator::AtAt => &["@@"],
        Operator::HashGt => &["#>"],
        Operator::HashGtGt => &["#>>"],
        Operator::HashMinus => &["#-"],
        // A custom operator has no fixed spelling; its span is validated as an `Op`-class
        // run by the dedicated arm in `check_kind_slice`, so it never reaches here.
        Operator::Custom => &[],
    }
}

/// A byte of the symbolic-operator character class (the `Op`-class of PostgreSQL's `scan.l`,
/// the reference for the general operator rule) — the bytes a general custom operator
/// ([`Operator::Custom`]) is built from.
fn is_operator_char(byte: u8) -> bool {
    matches!(
        byte,
        b'~' | b'!'
            | b'@'
            | b'#'
            | b'^'
            | b'&'
            | b'|'
            | b'`'
            | b'?'
            | b'+'
            | b'-'
            | b'*'
            | b'/'
            | b'%'
            | b'<'
            | b'>'
            | b'='
    )
}

/// True when `byte` is the opening delimiter of any configured identifier quote.
/// Only ASCII delimiters can match a single byte, matching the tokenizer's rule.
fn is_opening_identifier_quote(features: &FeatureSet, byte: u8) -> bool {
    features.identifier_quotes.iter().any(|quote| {
        let open = match quote {
            IdentifierQuote::Symmetric(delim) => *delim,
            IdentifierQuote::Asymmetric { open, .. } => *open,
        };
        open.is_ascii() && open as u8 == byte
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use squonk_ast::Span;

    /// Every adversarial fixture that the goldens pin, also run through the oracle
    /// directly so an invariant failure is attributable here, not only in the
    /// datadriven diff. Pairs each input with the directive whose dialect it needs.
    const ADVERSARIAL_CORPUS: &[(&str, &str)] = &[
        // Multibyte UTF-8 across words, strings, and quoted identifiers — the
        // headline char-boundary risk (ADR-0005).
        ("tokens-ansi", "café = δ"),
        ("tokens-ansi", "δ=1"),
        ("tokens-ansi", "'naïve''s'"),
        ("tokens-ansi", "'🎉x'"),
        ("tokens-ansi", "\"café\""),
        ("tokens-postgres", "$$café δ$$"),
        // Escaped / doubled quoting.
        ("tokens-ansi", "\"Odd\"\"Name\""),
        ("tokens-ansi", "'it''s'"),
        ("tokens-mysql", "`a``b`"),
        ("tokens-mysql", "\"a\"\"b\""),
        ("tokens-mssql", "[a]]b]"),
        ("tokens-mssql", "N'x'"),
        // Dollar-quoting, tagged and edge.
        ("tokens-postgres", "$$x$$"),
        ("tokens-postgres", "$tag$y$tag$"),
        ("tokens-postgres", "$$$$"),
        ("tokens-postgres", "$a$ inner $b$ still body $a$"),
        ("tokens-postgres", "$1"),
        // Comments: nested block, line, hash, and trivia-only.
        ("tokens-ansi", "/* a /* b */ c */x"),
        ("tokens-ansi", "a /* c */ b"),
        ("tokens-mysql", "a # c\nb"),
        ("tokens-ansi", "/* only */"),
        ("tokens-ansi", "--eof comment"),
        ("tokens-ansi", "/**/x"),
        // MySQL comment shape: versioned-comment regions (included body = live
        // tokens with the markers as trivia; future version = one discarded run;
        // string-protected close) and non-nesting block comments.
        ("tokens-mysql", "SELECT /*!40101 1 */"),
        ("tokens-mysql", "SELECT /*!99999 1 */ x"),
        ("tokens-mysql", "a /*!40101 '*/' */ b"),
        ("tokens-mysql", "/*!012345 9 */"),
        ("tokens-mysql", "/* a /* b */ x"),
        ("tokens-mysql", "SELECT /*!40101 1"),
        // Errors: stray bytes and unterminated constructs.
        ("tokens-ansi", "a @ b"),
        ("tokens-ansi", "$ x"),
        ("tokens-ansi", "?"),
        ("tokens-ansi", "a \\ b"),
        ("tokens-ansi", "'abc"),
        ("tokens-ansi", "\"abc"),
        ("tokens-ansi", "/* open"),
        ("tokens-ansi", "/* a /* b */"),
        ("tokens-postgres", "$tag$ body"),
        // Plain controls and empties.
        ("tokens-ansi", "SELECT a, b + 1 FROM t WHERE x <> 'y'"),
        ("tokens-ansi", ""),
        ("tokens-ansi", "   \t  "),
    ];

    #[test]
    fn oracle_accepts_every_adversarial_fixture() {
        for (directive, input) in ADVERSARIAL_CORPUS {
            render_golden(directive, input)
                .unwrap_or_else(|err| panic!("oracle rejected fixture {input:?}: {err}"));
        }
    }

    #[test]
    fn format_records_span_kind_and_slice_with_trivia() {
        let rendered = render_golden("tokens-ansi", "SELECT café <> 'x' -- note\n")
            .expect("clean tokenization");
        assert_eq!(
            rendered,
            "source \"SELECT café <> 'x' -- note\"\n\
             0..6 Keyword(Select) \"SELECT\"\n\
             6..7 trivia \" \"\n\
             7..12 Word \"café\"\n\
             12..13 trivia \" \"\n\
             13..15 Operator(NotEq) \"<>\"\n\
             15..16 trivia \" \"\n\
             16..19 String \"'x'\"\n\
             19..27 trivia \" -- note\"\n",
        );
    }

    #[test]
    fn format_records_lex_errors() {
        let rendered = render_golden("tokens-ansi", "SELECT 'it''s").expect("error renders");
        assert_eq!(
            rendered,
            "source \"SELECT 'it''s\"\nerror UnterminatedString 7..13 \"'it''s\"\n",
        );
    }

    /// The oracle is only useful if it *rejects* a broken stream; drive each
    /// invariant with a hand-built bad token vector and assert it is caught.
    #[test]
    fn checker_rejects_a_span_split_mid_codepoint() {
        // "café": é is bytes 3..5, so an endpoint at 4 splits the codepoint.
        let src = "café";
        let tokens = [Token::new(TokenKind::Word, Span::new(0, 4))];
        let err = check_token_stream(src, &tokens, &FeatureSet::ANSI)
            .expect_err("a mid-codepoint endpoint must be rejected");
        assert!(err.contains("char boundary"), "{err}");
    }

    #[test]
    fn checker_rejects_a_non_trivia_gap() {
        // Two words with a real byte (`b`) left unaccounted between them.
        let src = "abc";
        let tokens = [
            Token::new(TokenKind::Word, Span::new(0, 1)),
            Token::new(TokenKind::Word, Span::new(2, 3)),
        ];
        let err = check_token_stream(src, &tokens, &FeatureSet::ANSI)
            .expect_err("a non-trivia gap must be rejected");
        assert!(err.contains("not pure trivia"), "{err}");
    }

    #[test]
    fn checker_rejects_overlapping_spans() {
        let src = "ab";
        let tokens = [
            Token::new(TokenKind::Word, Span::new(0, 2)),
            Token::new(TokenKind::Word, Span::new(1, 2)),
        ];
        let err = check_token_stream(src, &tokens, &FeatureSet::ANSI)
            .expect_err("overlapping spans must be rejected");
        assert!(err.contains("overlapping"), "{err}");
    }

    #[test]
    fn checker_rejects_a_span_past_end_of_source() {
        let src = "a";
        let tokens = [Token::new(TokenKind::Word, Span::new(0, 2))];
        let err = check_token_stream(src, &tokens, &FeatureSet::ANSI)
            .expect_err("an out-of-bounds span must be rejected");
        assert!(err.contains("past source length"), "{err}");
    }

    #[test]
    fn checker_rejects_a_kind_slice_mismatch() {
        // A comma kind over the text "a" is a kind/slice disagreement.
        let src = "a";
        let tokens = [Token::new(
            TokenKind::Punctuation(Punctuation::Comma),
            Span::new(0, 1),
        )];
        let err = check_token_stream(src, &tokens, &FeatureSet::ANSI)
            .expect_err("a kind/slice mismatch must be rejected");
        assert!(err.contains("Punctuation(Comma)"), "{err}");
    }

    #[test]
    fn independent_trivia_recogniser_agrees_with_the_tokenizer() {
        // Pure-trivia strings tokenize to nothing AND read as all-trivia here; a
        // string with a real token does neither. This pins the two implementations
        // against each other on the recognition boundary.
        for trivia in ["", "   ", "-- x", "/* a /* b */ c */", "\t\n"] {
            assert!(is_all_trivia(trivia, &FeatureSet::ANSI), "{trivia:?}");
            assert!(
                tokenize_with(trivia, &FeatureSet::ANSI)
                    .expect("trivia lexes")
                    .is_empty(),
            );
        }
        assert!(!is_all_trivia("-- x\ny", &FeatureSet::ANSI));
        // `#` is trivia only where the dialect enables it.
        assert!(is_all_trivia("# c", &MYSQL));
        assert!(!is_all_trivia("# c", &FeatureSet::ANSI));

        // The MySQL comment shape, same agreement contract: a discarded or empty
        // versioned region is pure trivia; an included body is live tokens; and
        // under MySQL the first `*/` closes a block comment, so the
        // balanced-nesting spelling leaves a non-trivia tail.
        for trivia in ["/*!99999 x */", "/*!40101*/", "/* a /* b */"] {
            assert!(is_all_trivia(trivia, &MYSQL), "{trivia:?}");
            assert!(
                tokenize_with(trivia, &MYSQL)
                    .expect("trivia lexes")
                    .is_empty(),
            );
        }
        assert!(!is_all_trivia("/*!40101 x */", &MYSQL));
        assert!(!is_all_trivia("/* a /* b */ c */", &MYSQL));
        // An opener alone is not *self-contained* trivia (the region never closes).
        assert!(!is_all_trivia("/*!40101 ", &MYSQL));
    }

    #[test]
    fn bolero_token_stream_invariants_hold_under_cargo_test() {
        // The fuzz arm of the ADR-0005 mitigation: arbitrary UTF-8 must never
        // produce a token stream that breaks an invariant, across every preset.
        bolero::check!()
            .with_iterations(256)
            .with_max_len(64)
            .for_each(|input: &[u8]| {
                let Ok(src) = std::str::from_utf8(input) else {
                    return;
                };
                for features in [&FeatureSet::ANSI, &FeatureSet::POSTGRES, &MYSQL, &MSSQL] {
                    if let Ok(tokens) = tokenize_with(src, features) {
                        check_token_stream(src, &tokens, features).unwrap_or_else(|err| {
                            panic!("token-stream invariant violated for {src:?}: {err}")
                        });
                    }
                }
            });
    }
}
