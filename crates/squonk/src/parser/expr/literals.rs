// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Word, literal, and typed-literal primaries: the name/literal token class of
//! [`parse_prefix`](crate::parser::Parser::parse_prefix)'s dispatch — string/number
//! constants, parameters, session variables, column/function names, and the
//! speculative typed-string-literal forms.

use super::number_literal_kind;
use crate::ast::{
    BitStringRadix, CastSyntax, Expr, Ident, Keyword, Literal, LiteralKind, ParameterKind,
    ParameterSigil, QuoteStyle, SessionVariableKind, Span, Spanned, UnaryOperator,
    uescape_argument_is_legal, unicode_escape_string_is_valid,
};
use crate::error::ParseResult;
use crate::parser::Dialect;
use crate::parser::engine::{Checkpoint, Parser};
use crate::tokenizer::{LexError, LexErrorKind, Operator, Punctuation, Token, TokenKind};
use std::borrow::Cow;

impl<'a, D: Dialect> Parser<'a, D> {
    /// Per-class prefix dispatcher for the word/literal token class: number, string,
    /// parameter, positional-column, and session-variable constants, plus the
    /// word/quoted-identifier head that resolves to a contextual special form
    /// (`SUBSTR`, `TRY_CAST`, `LAMBDA`), a typed string literal, or a
    /// column/function name.
    ///
    /// `#[inline(never)]`: called-and-returned from the [`parse_prefix`](Self::parse_prefix)
    /// router so this class's scratch stays off the hot recursive frame (the
    /// `high_but_safe_nesting` stack canary budget).
    #[inline(never)]
    pub(super) fn parse_word_or_literal_prefix(
        &mut self,
        token: Token,
    ) -> ParseResult<Expr<D::Ext>> {
        match token.kind {
            TokenKind::Number => {
                self.advance()?;
                let kind = number_literal_kind(
                    self.span_text(token.span),
                    self.float_as_decimal_enabled(),
                );
                let literal_meta = self.make_meta(token.span);
                let literal = Literal {
                    kind,
                    meta: literal_meta,
                };
                let meta = self.make_meta(token.span);
                Ok(Expr::Literal { literal, meta })
            }
            TokenKind::String => self.parse_string_literal(token),
            TokenKind::Parameter => self.parse_parameter(token),
            TokenKind::PositionalColumn => self.parse_positional_column(token),
            TokenKind::Variable => self.parse_session_variable(token),
            // MySQL's `SUBSTR` synonym carries the same keyword grammar but is not a
            // keyword in any dialect's inventory, so the head is matched textually on
            // the unquoted word (a quoted `"substr"` lexes as `QuotedIdent`, never here).
            TokenKind::Word
                if self.features().string_func_forms.substr_from_for
                    && self.peek_nth_is_punct(1, Punctuation::LParen)?
                    && self.span_text(token.span).eq_ignore_ascii_case("substr") =>
            {
                self.parse_substring_expr(token)
            }
            // DuckDB's `TRY_CAST(<expr> AS <type>)` null-on-failure cast: the (contextual,
            // non-reserved) `TRY_CAST` word immediately followed by `(`, under the gate.
            // The `(` lookahead is the disambiguation (never a tokenizer change), so a bare
            // `try_cast` — no `(`, or the gate off — falls through to the ordinary name
            // path and stays a usable column/function name in every dialect.
            TokenKind::Word
                if self.features().call_syntax.try_cast
                    && self.peek_nth_is_punct(1, Punctuation::LParen)?
                    && self.token_is_contextual_keyword(token, "TRY_CAST") =>
            {
                self.parse_try_cast()
            }
            // BigQuery's `STRUCT(...)` value constructor: the (contextual, non-reserved)
            // `STRUCT` word opens the typeless form when immediately followed by `(` and
            // the typed `STRUCT<...>(...)` form when followed by `<`, under the gate. The
            // `(`/`<` lookahead is the disambiguation (never a tokenizer change), so a bare
            // `struct` — no `(`/`<`, or the gate off — falls through to the ordinary name
            // path and stays a usable column/function name in every non-BigQuery dialect,
            // where `struct(...)` remains an ordinary function call. Committing on `<` is a
            // single-token lookahead with no rewind: a preset that admits this form does not
            // treat `STRUCT` as a bare column, so `struct < x` is not a competing comparison.
            TokenKind::Word
                if self.features().expression_syntax.struct_constructor
                    && self.token_is_contextual_keyword(token, "STRUCT")
                    && (self.peek_nth_is_punct(1, Punctuation::LParen)?
                        || self.peek_nth_is_op(1, Operator::Lt)?) =>
            {
                self.parse_struct_constructor(token)
            }
            // DuckDB's python-style keyword lambda `lambda x, y: body` (under the gate). A
            // `lambda` word opens the production unconditionally, matching DuckDB — which
            // reserves `lambda`, so it never reads as an ordinary column here: a bare
            // `lambda` or one not followed by `<params>:` is the same parse error the engine
            // reports (`SELECT lambda`, `SELECT lambda AS x`). The deprecated single-arrow
            // form is the separate `lambda_expressions` operator gate (a different node
            // spelling, same shape).
            TokenKind::Word
                if self.features().expression_syntax.lambda_keyword
                    && self.token_is_contextual_keyword(token, "LAMBDA") =>
            {
                self.parse_keyword_lambda(token)
            }
            // A non-reserved type-name prefix followed by a string constant opens a
            // generalized typed literal (`float8 'NaN'`, `double precision '1.5'`):
            // PostgreSQL's `ConstTypename`/`func_name Sconst`, semantically a cast of the
            // string to the type (ADR-0011), detected speculatively (ADR-0005). On a
            // non-match the cursor rewinds and the word falls back to its column/function
            // reading. The temporal keywords keep priority via the keyword dispatch's
            // dedicated arm.
            TokenKind::Word | TokenKind::QuotedIdent => match self.try_parse_typed_literal()? {
                Some(expr) => Ok(expr),
                None => self.parse_word_prefix(token),
            },
            _ => unreachable!("parse_word_or_literal_prefix is routed only word/literal tokens"),
        }
    }
    /// Parse a valueless literal keyword (`NULL`, `TRUE`, `FALSE`).
    pub(super) fn parse_literal_keyword(&mut self, kind: LiteralKind) -> ParseResult<Expr<D::Ext>> {
        let token = self
            .advance()?
            .expect("parse_literal_keyword is reached only with a current token");
        let literal = Literal {
            kind,
            meta: self.make_meta(token.span),
        };
        let meta = self.make_meta(token.span);
        Ok(Expr::Literal { literal, meta })
    }
    /// Parse a `String`-class token into a literal.
    ///
    /// The lexer emits one `String` token for plain, `E'...'`, `$$…$$`, `N'...'`,
    /// `U&'...'`, and `B'...'`/`X'...'` constants; the prefix in the span selects the
    /// kind. A bit string is a distinct `bit`-typed literal, while the character forms
    /// all share `LiteralKind::String` — their spelling round-trips from the span and
    /// their value is recovered at the accessor. A `U&'...'` constant may
    /// carry a trailing `UESCAPE 'c'`, which is folded into the literal span, and its
    /// escape body is validated eagerly right here — the earliest point the active
    /// escape character (default `\`, or the `UESCAPE` override) is actually known.
    pub(crate) fn parse_string_literal(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        self.advance()?;
        let text = self.span_text(token.span);
        let kind = bit_string_kind(text).unwrap_or(LiteralKind::String);
        let unicode = is_unicode_string(text);
        // SQL-standard adjacent-string concatenation (ADR-0006): string constants
        // separated by whitespace that contains a newline are one value, so fold any
        // continuation segments into this literal's span before the optional `UESCAPE`
        // clause. A `U&'...'` constant's `UESCAPE` applies to the *whole* continued
        // constant, so the continuation runs first — mirroring PostgreSQL, which
        // assembles the continued body in its lexer and applies `UESCAPE` after.
        let span = self.consume_string_continuations(token.span, text)?;
        let span = if unicode {
            let span = self.consume_optional_uescape(span)?;
            // PG parity, at the one point it is actually decidable (see
            // `unicode_escape_string_is_valid`'s doc): the tokenizer's `U&'...'` scan
            // arm cannot validate the escape body itself, because a trailing `UESCAPE`
            // clause — a separate token it has not seen yet — can change which
            // character is the escape. Re-slicing the fully-folded span reuses the
            // exact body/escape resolution `Literal::as_str` uses lazily, so this eager
            // verdict and the materialised value can never disagree (ADR-0006).
            if !unicode_escape_string_is_valid(self.span_text(span)) {
                return Err(LexError::new(LexErrorKind::InvalidEscapeSequence, span).into());
            }
            span
        } else {
            span
        };
        let literal = Literal {
            kind,
            meta: self.make_meta(span),
        };
        let meta = self.make_meta(span);
        Ok(Expr::Literal { literal, meta })
    }
    /// Fold SQL-standard adjacent string-literal continuations into `span`.
    ///
    /// Two string constants separated by whitespace that contains a newline are one
    /// value in standard SQL and PostgreSQL (`'foo'`⏎`'bar'` ≡ `'foobar'`); the same
    /// constants separated only by same-line whitespace are an error. This extends the
    /// first literal's `span` to cover each continuation segment, so the multi-segment
    /// shape is implicit in the span — there is no concat node and the AST keeps its one
    /// canonical [`Literal`] shape. The concatenated value is recovered at
    /// the accessor ([`Literal::as_str`]), and the span renders verbatim so the exact
    /// source round-trips.
    ///
    /// Matching PostgreSQL's scanner: only an unprefixed character-string constant
    /// continues a constant (a plain `'...'`, or a plain `"..."` where the dialect lexes
    /// double quotes as strings — an `E'`/`N'`/`U&'`/`_charset'`/dollar/bit second segment
    /// is a separate constant, not a continuation), a dollar-quoted *first* segment never
    /// continues, and the joining whitespace must contain a newline — a comment in the gap
    /// does not count. Under [`StringLiteralSyntax::same_line_adjacent_concat`] (MySQL) the
    /// newline requirement is dropped: any whitespace-only gap joins, so `'a' 'b'` on one
    /// line is `'ab'` — and MySQL, lexing `"..."` as a string, likewise folds `'a' "b"`
    /// into `'ab'` (engine-measured); a comment in the gap still blocks the join.
    ///
    /// [`StringLiteralSyntax::same_line_adjacent_concat`]:
    ///     crate::ast::dialect::StringLiteralSyntax::same_line_adjacent_concat
    /// `first_text` is the first segment's source spelling, used only for the cheap
    /// dollar-quote check.
    ///
    /// Hot path: a string primary is normally followed by a non-string token, so the
    /// usual cost is the one `peek` that finds no string and returns; the source gap is
    /// inspected only when the next token is itself a string constant.
    ///
    /// Also drives a temporal literal's value string ([`try_parse_temporal_literal`](Self::try_parse_temporal_literal)),
    /// whose embedded constant PostgreSQL continues the same way.
    pub(in crate::parser) fn consume_string_continuations(
        &mut self,
        first_span: Span,
        first_text: &str,
    ) -> ParseResult<Span> {
        // A dollar-quoted constant never continues (PostgreSQL leaves the dollar-quote
        // lexer state at its close), so decline before peeking; every other string form
        // is continuable.
        if first_text.starts_with('$') {
            return Ok(first_span);
        }
        let mut span = first_span;
        loop {
            let Some(next) = self.peek()? else {
                return Ok(span);
            };
            if next.kind != TokenKind::String {
                return Ok(span);
            }
            // Only an *unprefixed* character-string constant is a continuation segment: a
            // plain `'...'`, or a plain `"..."` where the dialect lexes double quotes as
            // strings (MySQL without `ANSI_QUOTES` — `SELECT 'a' "b"` is `'ab'`, engine-
            // measured on mysql:8.4.10). A prefixed string (`E'`, `N'`, `U&'`, `_charset'`,
            // bit `X'`/`B'`) or a dollar-quote is a separate constant the engine rejects as
            // adjacent, so it is left for the caller's normal "two primaries" error rather
            // than joined. A prefix always precedes the quote, so the leading byte
            // discriminates: `"` reaches here only for an unprefixed double-quoted `String`
            // token (a `_charset"..."` introducer lexes with the leading `_`, and under
            // `ANSI_QUOTES` a `"..."` is an identifier token that never enters this string
            // path at all).
            let seg = self.span_text(next.span);
            if !(seg.starts_with('\'') || seg.starts_with('"')) {
                return Ok(span);
            }
            // Inspect the raw source gap directly (the opt-in trivia recovery is off by
            // default): it is exactly `source[span.end .. next.start]`.
            let gap = self.span_text(Span::new(span.end(), next.span.start()));
            match classify_continuation_gap(gap) {
                ContinuationGap::Newline => {
                    self.advance()?; // join the continuation segment
                    span = span.union(next.span);
                }
                ContinuationGap::SameLine => {
                    // MySQL joins adjacent literals across any whitespace, newline or not;
                    // the standard requires a newline, so same-line adjacency is otherwise
                    // an error. The materializer walks the folded span the same way either
                    // way (whitespace is trimmed between segments).
                    if self.features().string_literals.same_line_adjacent_concat {
                        self.advance()?; // join the continuation segment
                        span = span.union(next.span);
                    } else {
                        let found = self.span_text(next.span).to_owned();
                        return Err(self.error_at(
                            next.span,
                            "a newline between adjacent string literals",
                            found,
                        ));
                    }
                }
                // A comment (or other non-whitespace) in the gap is not a continuation;
                // leave the second constant for the normal adjacency error.
                ContinuationGap::NotWhitespace => return Ok(span),
            }
        }
    }
    /// Fold an optional `UESCAPE 'c'` clause that follows a `U&'...'` constant into the
    /// literal's span so the exact source round-trips; the parser only needs the
    /// clause's lexical shape here. The caller ([`Self::parse_string_literal`])
    /// validates the escape body against the resolved escape character right after
    /// this returns. The `UESCAPE` character's own legality — a single character that is not
    /// a hex digit, `+`, a single/double quote, or whitespace (PostgreSQL's
    /// `check_uescapechar`) — is validated here at parse time, so `U&'…' UESCAPE '+'` (or a
    /// multi-character `UESCAPE '!!'`) is a clean parse reject rather than a silently-accepted
    /// literal whose value only fails at the accessor.
    ///
    /// Shared with the `U&"..."` *identifier* fold in `parse_ident_admitting`:
    /// PostgreSQL's `base_yylex` wrapper looks one token past both a `U&'...'` string and a
    /// `U&"..."` identifier for the same `UESCAPE` clause, so the two surfaces share this
    /// one fold.
    pub(in crate::parser) fn consume_optional_uescape(
        &mut self,
        string_span: Span,
    ) -> ParseResult<Span> {
        let Some(uescape) = self.peek()? else {
            return Ok(string_span);
        };
        if uescape.kind != TokenKind::Keyword(Keyword::Uescape) {
            return Ok(string_span);
        }
        self.advance()?; // UESCAPE
        let Some(escape) = self.peek()? else {
            return Err(self.unexpected("a UESCAPE character string"));
        };
        if escape.kind != TokenKind::String {
            return Err(self.unexpected("a UESCAPE character string"));
        }
        if !uescape_argument_is_legal(self.span_text(escape.span)) {
            return Err(LexError::new(LexErrorKind::InvalidEscapeSequence, escape.span).into());
        }
        self.advance()?;
        Ok(string_span.union(escape.span))
    }
    /// Resolve a leading word, keyword, or quoted identifier to a prefix `NOT`, a
    /// forbidden infix keyword, or a (possibly dotted) column reference.
    ///
    /// `AND`/`OR`/`NOT` are operators, not columns: `NOT` begins a unary
    /// expression, while `AND`/`OR` are infix-only and so cannot open one. Otherwise
    /// the leading word opens a column reference or a function call — gated
    /// per-position by [`name_or_call_head_reserved`](Parser::name_or_call_head_reserved),
    /// which admits a `type_func_name` keyword (e.g. `left`) only when it is an
    /// (unqualified) call. The name may be qualified (`a.b.c` or `"a"."b"`) — the
    /// shared [`parse_object_name`](Parser::parse_object_name) collects the parts.
    pub(super) fn parse_word_prefix(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        match token.kind {
            TokenKind::Keyword(Keyword::Not) => {
                // Prefix `NOT` is a boolean operator — `a_expr`-only, so it is not a valid
                // `b_expr` head; leave it for the column-constraint context to reject.
                if self.restrict_b_expr {
                    return Err(self.unexpected("an expression"));
                }
                return self.parse_unary(UnaryOperator::Not);
            }
            TokenKind::Keyword(Keyword::And | Keyword::Or) => {
                return Err(self.unexpected("an expression"));
            }
            _ => {}
        }
        let start_cp = self.checkpoint();
        let head_reserved = self.name_or_call_head_reserved()?;
        if !self.token_admissible(token, head_reserved) {
            // A non-admissible head is not a column/function name, but a parameterized
            // type keyword (`char`, `numeric`, `bit`, `varchar`, …) still opens a typed
            // string literal `char(20) 'chars'` — PG's parameterized `ConstTypename
            // Sconst`. These keywords never form a function call (they reject here today),
            // so speculate the type reading directly when a modifier list follows; a
            // non-match rewinds to the original unexpected-token error.
            if self.features().expression_syntax.typed_string_literals
                && self.peek_nth_is_punct(1, Punctuation::LParen)?
            {
                if let Some(expr) = self.try_prefix_typed_literal(start_cp, token.span)? {
                    return Ok(expr);
                }
            }
            return Err(self.unexpected("an expression"));
        }
        let name = self.parse_object_name_with(head_reserved)?;
        if self.peek_is_punct(Punctuation::LParen)? {
            let call = self.parse_function_call(name, token.span)?;
            // A call form immediately followed by a string constant is PG's
            // `func_name '(' func_arg_list ')' Sconst` typed literal (`foo(1) 'x'`,
            // `left(1) 'x'`), not a function call. Detected only on the trailing string —
            // an ordinary call pays a single peek — then re-read from the head through the
            // type grammar (which folds any non-numeric-modifier arg back to the call).
            if self.features().expression_syntax.typed_string_literals && self.peek_is_string()? {
                let after_call = self.checkpoint();
                self.rewind(start_cp);
                if let Some(expr) = self.try_prefix_typed_literal(start_cp, token.span)? {
                    return Ok(expr);
                }
                self.rewind(after_call);
            }
            let meta = self.make_meta(call.meta.span);
            Ok(Expr::Function {
                call: Box::new(call),
                meta,
            })
        } else {
            let meta = self.make_meta(name.span());
            Ok(Expr::Column { name, meta })
        }
    }
    /// Speculatively open a generalized typed string constant: a (non-reserved)
    /// type-name prefix immediately followed by a string constant — PostgreSQL's
    /// `ConstTypename Sconst` / `func_name Sconst` (`float8 'NaN'`, `int4 '42'`,
    /// `double precision '1.5'`, `pg_catalog.float8 'NaN'`).
    ///
    /// The semantics are a cast of the string to the type, identical to
    /// `'NaN'::float8` and `CAST('NaN' AS float8)`, so the result is the one canonical
    /// [`Expr::Cast`] shape carrying a [`CastSyntax::PrefixTyped`] surface tag
    /// — never a parallel typed-literal node. The type name is parsed with
    /// [`parse_data_type`](Self::parse_data_type) so the target matches the cast forms
    /// exactly (`real` resolves to the built-in `DataType::Real`, not a user-defined
    /// name), and detection is speculative: on a non-match the cursor
    /// rewinds so the prefix falls back to its ordinary column/function reading.
    ///
    /// [`peek_opens_typed_literal`](Self::peek_opens_typed_literal) gates the
    /// speculation so the hot column/function-call path pays only a single peek; the
    /// temporal keywords (`DATE`/`TIME`/`TIMESTAMP`/`INTERVAL`) keep their dedicated
    /// reserved-keyword literal forms and never reach here.
    pub(super) fn try_parse_typed_literal(&mut self) -> ParseResult<Option<Expr<D::Ext>>> {
        if !self.features().expression_syntax.typed_string_literals {
            return Ok(None);
        }
        if !self.peek_opens_typed_literal()? {
            return Ok(None);
        }
        let checkpoint = self.checkpoint();
        let start = self.current_span()?;
        self.try_prefix_typed_literal(checkpoint, start)
    }
    /// Speculatively fold the type-name head at `start_cp` (the cursor must sit there)
    /// and a trailing string constant into one [`CastSyntax::PrefixTyped`] cast: PG's
    /// `ConstTypename Sconst` / `func_name '(' … ')' Sconst`. A non-match rewinds to
    /// `start_cp` and returns `None`; `start_span` anchors the folded node's span.
    ///
    /// The type is read with [`parse_data_type`](Self::parse_data_type) so a built-in
    /// spelling resolves to its canonical [`DataType`](crate::ast::DataType) (`char(20)` →
    /// [`DataType::Character`](crate::ast::DataType::Character), matching the bare `char 'x'` shape) — never a parallel
    /// typed-literal node. That parse's own type-name admissibility is also the
    /// `func_name` boundary for the parameterized forms: a `type_func_name` head
    /// (`left(1) 'x'`) opens the literal, a `col_name`/reserved head
    /// (`substring(a) 'x'`, `coalesce(1) 'x'`) rejects the type parse and falls back,
    /// exactly as PostgreSQL restricts the production.
    fn try_prefix_typed_literal(
        &mut self,
        start_cp: Checkpoint,
        start_span: Span,
    ) -> ParseResult<Option<Expr<D::Ext>>> {
        // A reserved keyword (or other non-type prefix) is not a valid type name; a
        // type-parse failure means "not a typed literal", so rewind to the bare prefix.
        let Ok(data_type) = self.parse_data_type() else {
            self.rewind(start_cp);
            return Ok(None);
        };
        // The value must be an `Sconst` (`peek_is_sconst`, not merely any string): PG's
        // `ConstTypename Sconst` / `func_name '(' … ')' Sconst` productions — and the
        // MySQL/DuckDB prefix-typed forms — reject a bit-string (`float8 B'1'`), national
        // (`N'x'`), or introducer (`_utf8'x'`) value here, so a non-`Sconst` string rewinds
        // to the bare prefix reading exactly as a missing value does (the trailing string
        // then surfaces as the usual adjacency parse error, matching the engines).
        if !self.peek_is_sconst()? {
            self.rewind(start_cp);
            return Ok(None);
        }
        // The value is an ordinary string constant: fold any newline-separated adjacent
        // continuations into it like a bare string primary (ADR-0006), matching
        // PostgreSQL (`float8 'x'`⏎`'y'` is the one value `xy`). A same-line second
        // string is the usual adjacency error — a malformed value is a hard error here,
        // not a rewind, since the typed literal is already committed by the leading
        // string (mirroring the temporal literal forms).
        let value = self
            .peek()?
            .expect("peek_is_string confirmed a current string token");
        let string = self.parse_string_literal(value)?;
        let span = start_span.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(Expr::Cast {
            expr: Box::new(string),
            data_type: Box::new(data_type),
            syntax: CastSyntax::PrefixTyped,
            try_cast: false,
            meta,
        }))
    }
    /// Cheap gate deciding whether the leading word *might* open a typed string
    /// constant, before the speculative [`parse_data_type`](Self::parse_data_type).
    ///
    /// A typed literal is `<type-name> <string-constant>`; the token after the leading
    /// word tells whether a type name can continue toward a string:
    /// - a string itself — a bare single-word type (`float8 'NaN'`, `real 'Infinity'`);
    /// - a `.` whose qualified name a string trails — a schema-qualified type
    ///   (`pg_catalog.float8 'NaN'`), distinguished from a qualified *column* (`t1.id`)
    ///   by [`qualified_name_precedes_string`](Self::qualified_name_precedes_string);
    /// - an unreserved keyword immediately followed by a string — a two-word built-in
    ///   (`double precision '1.5'`, `character varying 'x'`).
    ///
    /// Everything else is declined without speculating, keeping the hot non-literal
    /// followers off the allocating speculative path: an operator or punctuation ends the
    /// primary, a *reserved* keyword (`FROM`, `AND`, …) cannot continue a type name, a
    /// bare identifier follower is an implicit alias (`SELECT a b`), and `(` opens a
    /// modifier list — the parameterized `type(modifier) 'string'` spelling is handled off
    /// the call path in [`parse_word_prefix`](Self::parse_word_prefix), keyed on the
    /// trailing string, so an ordinary call never reaches the speculative type parse.
    /// Bounding the two-word case to a string at the *next* token
    /// rather than scanning an open keyword run is what keeps a clause tail like `ORDER BY
    /// b RANGE BETWEEN INTERVAL '1' …` — whose string lies several keywords past `b` — off
    /// the speculative path; the rare three-plus-word built-in (`national character
    /// varying 'x'`) is consequently not opened in prefix position.
    fn peek_opens_typed_literal(&mut self) -> ParseResult<bool> {
        let Some(next) = self.peek_nth(1)? else {
            return Ok(false);
        };
        Ok(match next.kind {
            TokenKind::String => true,
            TokenKind::Punctuation(Punctuation::Dot) => self.qualified_name_precedes_string()?,
            TokenKind::Keyword(keyword)
                if !self.features().reserved_column_name.contains(keyword) =>
            {
                self.peek_nth(2)?
                    .is_some_and(|after| after.kind == TokenKind::String)
            }
            _ => false,
        })
    }
    /// Whether a `. name` chain from the cursor's leading word is immediately followed by
    /// a string constant — the tell for a schema-qualified typed literal
    /// (`pg_catalog.float8 'NaN'`) versus an ordinary qualified column (`t1.id`). Peeks
    /// only; never advances and never allocates, and consumes only dotted name parts so it
    /// cannot run past the qualified name into a following clause.
    fn qualified_name_precedes_string(&mut self) -> ParseResult<bool> {
        // The leading word sits at offset 0; a qualified name continues as `(. name)+`.
        let mut offset = 1;
        while self.peek_nth_is_punct(offset, Punctuation::Dot)?
            && self
                .peek_nth(offset + 1)?
                .is_some_and(|part| Self::token_can_continue_qualified_name(part))
        {
            offset += 2;
        }
        Ok(self
            .peek_nth(offset)?
            .is_some_and(|token| token.kind == TokenKind::String))
    }
    /// Whether `token` can be the part of a qualified name after a `.` for the typed-
    /// literal gate: a word, a quoted identifier, or any keyword (a qualified-name
    /// continuation is a `ColLabel`, which admits every keyword). An over-broad admission
    /// only costs a speculative parse that then rewinds, so this stays a cheap kind check
    /// rather than the full name-reject set.
    fn token_can_continue_qualified_name(token: Token) -> bool {
        matches!(
            token.kind,
            TokenKind::Word | TokenKind::QuotedIdent | TokenKind::Keyword(_)
        )
    }
    /// Parse a prepared-statement parameter placeholder into [`Expr::Parameter`].
    ///
    /// The tokenizer already validated the form and the leading sigil selects it: a
    /// positional token is `$` + ASCII digits, an anonymous token is `?`, and a named
    /// token is `:name` / `@name`. The placeholder span is never empty, so the first
    /// byte is the sigil. The positional 1-based index is materialized here (like a
    /// `Number`'s value), whose only failure is a `u32` overflow — a far larger bound
    /// than any real parameter list; the named form interns its name (sigil stripped),
    /// exact-case, so it round-trips like an identifier.
    fn parse_parameter(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        let (kind, meta) = self.parse_parameter_kind(token)?;
        Ok(Expr::Parameter { kind, meta })
    }

    /// Consume and materialize a parameter token for expression and restricted-value
    /// productions that share the dialect's parameter spelling.
    pub(in crate::parser) fn parse_parameter_kind(
        &mut self,
        token: Token,
    ) -> ParseResult<(ParameterKind, crate::ast::Meta)> {
        self.advance()?; // consume the placeholder token
        let text = self.span_text(token.span);
        let bytes = text.as_bytes();
        let kind = match bytes[0] {
            // `$1` positional index vs `$name` (SQLite): the byte after `$`
            // disambiguates (the tokenizer required a digit or an identifier-start).
            b'$' if bytes.get(1).is_some_and(u8::is_ascii_digit) => {
                ParameterKind::Positional(text[1..].parse::<u32>().map_err(|_| {
                    self.error_at(
                        token.span,
                        "a positional parameter index within u32 range",
                        text.to_owned(),
                    )
                })?)
            }
            // `$name` (SQLite) / `:name` / `@name`: the sigil is one ASCII byte, so the
            // name begins at `start + 1`; record which sigil spelled it for an exact
            // round-trip.
            sigil @ (b'$' | b':' | b'@') => {
                let name_span = Span::new(token.span.start() + 1, token.span.end());
                // The name is carved out of the placeholder token, not a settled
                // Word/Keyword token, so it takes the full keyword-checking intern:
                // `:select` must still resolve to the `select` keyword slot.
                let name_text = self.span_text(name_span);
                let name = self.intern_text(name_text);
                let sigil = match sigil {
                    b'$' => ParameterSigil::Dollar,
                    b':' => ParameterSigil::Colon,
                    _ => ParameterSigil::At,
                };
                ParameterKind::Named { name, sigil }
            }
            // A `?` token: bare `?` is anonymous, `?NNN` is SQLite's numbered positional
            // (`numbered_question`). SQLite restricts the index to `1..=32766`
            // (`SQLITE_MAX_VARIABLE_NUMBER`): `?0`, `?32767`, and an overflowing digit run are
            // parse rejects ("variable number must be between ?1 and ?32766"; engine-measured),
            // enforced here where the number is materialised — the same parse-time bound the
            // DuckDB `#0` positional-column reference applies.
            b'?' if bytes.len() > 1 => {
                let index = text[1..]
                    .parse::<u32>()
                    .ok()
                    .filter(|&n| (1..=32766).contains(&n));
                match index {
                    Some(index) => ParameterKind::Numbered(index),
                    None => {
                        return Err(self.error_at(
                            token.span,
                            "a numbered parameter index between ?1 and ?32766",
                            text.to_owned(),
                        ));
                    }
                }
            }
            // The tokenizer only emits a bare anonymous parameter token for `?`.
            _ => ParameterKind::Anonymous,
        };
        let meta = self.make_meta(token.span);
        Ok((kind, meta))
    }
    /// Parse a DuckDB `#n` positional column reference into [`Expr::PositionalColumn`].
    ///
    /// The tokenizer emitted one atomic `#<digits>` token; the digits after the `#` sigil
    /// (one ASCII byte, so they begin at `start + 1`) are the 1-based output position.
    /// DuckDB rejects `#0` at parse time ("Positional reference node needs to be >= 1"),
    /// so a zero index is a clean parse error here too, as is a digit run wider than a
    /// `u32`.
    fn parse_positional_column(&mut self, token: Token) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // consume the `#n` token
        let text = self.span_text(token.span);
        let index = text[1..].parse::<u32>().map_err(|_| {
            self.error_at(
                token.span,
                "a positional column index within u32 range",
                text.to_owned(),
            )
        })?;
        if index == 0 {
            return Err(self.error_at(
                token.span,
                "a positional column index of at least 1",
                text.to_owned(),
            ));
        }
        let meta = self.make_meta(token.span);
        Ok(Expr::PositionalColumn { index, meta })
    }
    /// Parse a MySQL session variable into [`Expr::SessionVariable`].
    ///
    /// The tokenizer emitted one atomic token for the whole reference; its leading
    /// sigil selects the form. A `@@`-prefixed token is a system variable whose
    /// optional `global.`/`session.` scope was folded into the same lexeme, so the
    /// scope and name are split on the interior `.` here — an unrecognised scope word
    /// is a clean parse error. A single `@` is a user variable. Each name is interned
    /// exact-case (sigil and scope stripped) so it round-trips like an identifier.
    pub(in crate::parser) fn parse_session_variable(
        &mut self,
        token: Token,
    ) -> ParseResult<Expr<D::Ext>> {
        self.advance()?; // consume the variable token
        let text = self.span_text(token.span);
        let (kind, name_text) = if let Some(rest) = text.strip_prefix("@@") {
            // System variable `@@[scope.]name`: a `.` splits an explicit scope from the
            // name; without one the whole run is the name at the server's implicit scope
            // (so `@@global` with no dot is a system variable literally named `global`).
            match rest.split_once('.') {
                Some((scope, name)) => {
                    let kind = if scope.eq_ignore_ascii_case("global") {
                        SessionVariableKind::SystemGlobal
                    } else if scope.eq_ignore_ascii_case("session") {
                        SessionVariableKind::SystemSession
                    } else {
                        return Err(self.error_at(
                            token.span,
                            "a `global` or `session` system-variable scope",
                            text.to_owned(),
                        ));
                    };
                    (kind, name)
                }
                None => (SessionVariableKind::System, rest),
            }
        } else {
            // User variable `@name`: the sigil is one ASCII byte, so the name follows it.
            (SessionVariableKind::User, &text[1..])
        };
        let name = self.intern_text(name_text);
        let meta = self.make_meta(token.span);
        Ok(Expr::SessionVariable { kind, name, meta })
    }
    /// Parse a MySQL string-literal column alias (`SELECT 1 AS 'x'`, and `AS "x"` where
    /// MySQL's no-`ANSI_QUOTES` mode makes `"…"` a string). The string's value becomes the
    /// alias identifier and the source quote is recorded
    /// ([`QuoteStyle::Single`] / [`Double`]) so it renders
    /// back quoted and round-trips. Returns `Ok(None)` when the next token is not a plain
    /// string constant (a bare identifier or a prefixed string such as `N'…'`), so the
    /// caller falls back to the standard identifier-alias path. Gated by
    /// [`SelectSyntax::alias_string_literals`](crate::ast::dialect::SelectSyntax::alias_string_literals);
    /// consulted only in projection-alias position, since MySQL admits string aliases only
    /// for columns, not table/schema names.
    ///
    /// [`Double`]: crate::ast::QuoteStyle::Double
    pub(in crate::parser) fn parse_string_alias_ident(&mut self) -> ParseResult<Option<Ident>> {
        let Some(token) = self.peek()? else {
            return Ok(None);
        };
        if token.kind != TokenKind::String {
            return Ok(None);
        }
        let raw = self.span_text(token.span);
        // The value undoubles its own delimiter, mirroring `parse_struct_key`'s `'…'` key.
        // Backslash escapes are left verbatim (the tokenizer already located the true
        // terminator); the render doubles the delimiter the same way, so the value
        // round-trips through the parser regardless.
        let (quote, doubled, delim) = match raw.as_bytes().first() {
            Some(b'\'') => (QuoteStyle::Single, "''", "'"),
            Some(b'"') => (QuoteStyle::Double, "\"\"", "\""),
            // A prefixed string (`N'…'`, `_utf8'…'`, `X'…'`, …) is not an alias.
            _ => return Ok(None),
        };
        self.advance()?;
        let inner = &raw[1..raw.len() - 1];
        let text = if inner.contains(doubled) {
            Cow::Owned(inner.replace(doubled, delim))
        } else {
            Cow::Borrowed(inner)
        };
        let sym = self.intern_text(&text);
        let meta = self.make_meta(token.span);
        Ok(Some(Ident { sym, quote, meta }))
    }
}

/// How the source gap between two adjacent string constants classifies for
/// SQL-standard string continuation (see [`Parser::consume_string_continuations`]).
enum ContinuationGap {
    /// Whitespace containing a newline: the two constants concatenate into one value.
    Newline,
    /// Whitespace with no newline (`'a' 'b'` on one line): an adjacency error.
    SameLine,
    /// A non-whitespace byte (a comment) sits in the gap: not a continuation.
    NotWhitespace,
}

/// The bit-string [`LiteralKind`] for a `B'...'` / `X'...'` lexeme, or `None` for any
/// other string spelling. The radix is the binary/hex marker the lexer kept in the span.
fn bit_string_kind(text: &str) -> Option<LiteralKind> {
    match text.as_bytes() {
        [b'B' | b'b', b'\'', ..] => Some(LiteralKind::BitString {
            radix: BitStringRadix::Binary,
        }),
        [b'X' | b'x', b'\'', ..] => Some(LiteralKind::BitString {
            radix: BitStringRadix::Hex,
        }),
        _ => None,
    }
}

/// Whether a `String` lexeme is a `U&'...'` Unicode-escape constant — the only string
/// form that takes a trailing `UESCAPE` clause.
fn is_unicode_string(text: &str) -> bool {
    matches!(text.as_bytes(), [b'U' | b'u', b'&', b'\'', ..])
}

/// Whether a `String` lexeme is an `Sconst` — PostgreSQL's character-string constant, as
/// opposed to a bit-string (`b'...'`/`x'...'`, a `bit`-typed `BCONST`/`XCONST`) or a
/// national (`N'...'`) or charset-introducer (`_utf8'...'`) constant. The lexer emits one
/// `String` token for every prefixed spelling, distinguished only by the prefix kept in
/// the span, so the discrimination is a parse-site span-prefix check.
///
/// `Sconst` covers the plain `'...'`, escape `E'...'`, Unicode-escape `U&'...'`, and
/// dollar-quoted `$$...$$` forms — the constants PostgreSQL's grammar admits wherever it
/// writes `Sconst` (a `DO` code block, a `NonReservedWord_or_Sconst` operand). Callers in
/// those positions reject the non-`Sconst` string kinds, matching libpg_query (`DO b'0'`,
/// `DO x'ab'`, `DO N'x'` are all syntax errors).
pub(in crate::parser) fn string_literal_is_sconst(text: &str) -> bool {
    match text.as_bytes() {
        // BCONST / XCONST bit-string constants (a distinct `bit`-typed literal).
        [b'B' | b'b' | b'X' | b'x', b'\'', ..] => false,
        // A national `N'...'` constant; PostgreSQL has no national-string prefix, so this
        // is never an `Sconst` there.
        [b'N' | b'n', b'\'', ..] => false,
        // A MySQL `_charset'...'` character-set introducer.
        [b'_', ..] => false,
        // Plain `'...'`, escape `E'...'`, Unicode-escape `U&'...'`, and dollar-quoted
        // `$$...$$` constants all lower to `Sconst`.
        _ => true,
    }
}

/// Classify the source gap between two adjacent string constants. PostgreSQL joins
/// them only across whitespace that contains a newline; a comment in the gap (even
/// one that itself contains a newline) does not continue the string, so any
/// non-whitespace byte disqualifies the gap. Both `\n` and `\r` count as newlines,
/// matching the scanner's `newline` class.
fn classify_continuation_gap(gap: &str) -> ContinuationGap {
    let mut newline = false;
    for byte in gap.bytes() {
        match byte {
            b'\n' | b'\r' => newline = true,
            // The horizontal-whitespace bytes the scanner allows around the newline:
            // space, tab, vertical tab, form feed.
            b' ' | b'\t' | 0x0b | 0x0c => {}
            _ => return ContinuationGap::NotWhitespace,
        }
    }
    if newline {
        ContinuationGap::Newline
    } else {
        ContinuationGap::SameLine
    }
}
