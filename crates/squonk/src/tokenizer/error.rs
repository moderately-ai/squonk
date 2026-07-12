// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Self-contained lexical errors.
//!
//! This type is intentionally independent of `squonk::error`: that module is
//! being built concurrently by `m1-parse-error`, and the two error types are
//! reconciled in `m1-engine`. Keeping a local error here lets the tokenizer land
//! and be tested without taking a dependency on an in-flight sibling ticket.

use std::fmt;

use crate::ast::Span;

/// A lexical error: the offending source range plus what went wrong.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LexError {
    /// The source range the error concerns. For unterminated constructs this is
    /// the whole construct, from its opening delimiter to end of input.
    pub span: Span,
    /// What kind of lexical error occurred.
    pub kind: LexErrorKind,
}

impl LexError {
    /// Build a lexical error for `span`.
    pub const fn new(kind: LexErrorKind, span: Span) -> Self {
        Self { span, kind }
    }
}

/// The lexical error conditions the tokenizer can raise.
///
/// `#[non_exhaustive]`: new dialect scanners add lexical faults over time, and
/// this kind is now carried onto [`ParseErrorKind::Lexical`](crate::error::ParseErrorKind::Lexical)
/// and mapped to a bindings machine kind, so it is a growth axis that reserves
/// additive variants for a minor release rather than freezing at 1.0. In-crate
/// matches ([`message`](Self::message), [`machine_kind`](Self::machine_kind))
/// stay exhaustive with no wildcard; only downstream matches must add a `_` arm.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[non_exhaustive]
pub enum LexErrorKind {
    /// A single-quoted string literal with no closing `'`.
    UnterminatedString,
    /// A double-quoted identifier with no closing `"`.
    UnterminatedQuotedIdent,
    /// A `/*`-opened construct with no matching close: a `/* … */` block comment
    /// (nesting per dialect data) or a MySQL `/*!…` versioned-comment region.
    UnterminatedBlockComment,
    /// A `$tag$ … $tag$` dollar-quoted string with no closing tag.
    UnterminatedDollarQuote,
    /// A byte that begins no known token (e.g. `@`, `?`, a control byte), or a
    /// `$` that does not open a valid dollar-quote.
    StrayByte,
    /// A PostgreSQL escape string (`E'…'`) carrying a malformed escape that the real
    /// parser rejects while lexing: a short or out-of-range Unicode escape (`\u12`,
    /// `\uD800`, `\U00110000`), an escape decoding to NUL (`\0`, `\x00`), or a byte
    /// escape that does not form valid UTF-8 (`\xff`). An unknown escape (`\q`) or a
    /// bare `\x` is *not* malformed — it is the literal following character. The
    /// escape grammar lives in the AST crate and is shared with lazy
    /// materialisation; the lexer invokes it so the malformed value is rejected up front
    /// rather than deferred to a value accessor.
    InvalidEscapeSequence,
    /// A string literal whose source text embeds a raw NUL byte (`0x00`) — in any
    /// form (ordinary `'…'`, `E'…'`, `N'…'`, `U&'…'`, bit-string, or dollar-quoted).
    /// PostgreSQL cannot carry a NUL inside any value: a query reaches the server as
    /// a NUL-terminated C string, so libpg_query rejects a literal `0x00` byte while
    /// parsing (confirmed against the oracle). This is distinct from an *escape* that
    /// decodes to NUL (`\0`, `\x00`), which is an [`InvalidEscapeSequence`]; here the
    /// NUL is a raw byte in the body, independent of escape decoding. The predicate
    /// lives in the AST crate and is shared with lazy `as_str` materialisation, so the
    /// eager lexer verdict and the lazily materialised value cannot disagree.
    ///
    /// [`InvalidEscapeSequence`]: Self::InvalidEscapeSequence
    NulByteInString,
    /// A quoted identifier whose source text embeds a raw NUL byte (`0x00`) — in any
    /// configured form (`"…"`, T-SQL `[…]`, or backtick `` `…` ``). PostgreSQL rejects a
    /// NUL in *all* lexable text via the same C-string boundary that powers
    /// [`NulByteInString`]: a query reaches the server as a NUL-terminated C string, so
    /// libpg_query rejects a literal `0x00` byte in a quoted identifier while parsing
    /// (confirmed against the oracle). The check reuses the AST crate's content-agnostic
    /// NUL predicate (a single `memchr`, no allocation) — only the scan site and this
    /// error kind distinguish it from the string case. Unlike a string literal, a quoted
    /// identifier is interned rather than materialised through `Literal::as_str`, so
    /// there is no eager-vs-lazy agreement contract to uphold here; this is purely the
    /// up-front PG-parity gate.
    ///
    /// [`NulByteInString`]: Self::NulByteInString
    NulByteInIdentifier,
    /// A comment — a `--`/`#` line comment or a `/* … */` block comment — whose source
    /// text embeds a raw NUL byte (`0x00`). PostgreSQL rejects a NUL in *all* lexable
    /// text via the same C-string boundary that powers [`NulByteInString`] and
    /// [`NulByteInIdentifier`]: a query reaches the server as a NUL-terminated C string,
    /// so libpg_query rejects any interior `0x00` (confirmed against the oracle — it
    /// surfaces as a `NulError` conversion). A comment is *skipped* as trivia — the
    /// scanner consumes it to end-of-line / `*/` without inspecting the bytes — so it is
    /// the one lexable context the value-bearing [`NulByteInString`]/[`NulByteInIdentifier`]
    /// gates cannot see; this kind closes that gap so the whole-input NUL rejection is
    /// complete. The span covers the comment run.
    ///
    /// [`NulByteInString`]: Self::NulByteInString
    /// [`NulByteInIdentifier`]: Self::NulByteInIdentifier
    NulByteInComment,
    /// A delimited identifier (`"…"`, backtick, asymmetric `[…]`) whose body has
    /// zero length: the closing delimiter follows the opening one immediately, with
    /// no character — not even a doubled-close escape — between them (`""`,
    /// `` `` ``, `[]`). SQL's `<delimited identifier body>` requires at least one
    /// character; PostgreSQL rejects this while scanning ("zero-length delimited
    /// identifier", `scan.l`) and MySQL rejects an empty backtick identifier the
    /// same way. The rejection is unconditional — not gated by dialect feature data
    /// — because no shipped dialect legitimately accepts an empty delimited
    /// identifier. Distinct from an empty *string* literal (`''`, or MySQL `""`
    /// under `ANSI_QUOTES` off), which is ordinary valid SQL and never raises this:
    /// the scan keys the check on the token kind, so only an identifier-kind scan is
    /// covered.
    ZeroLengthDelimitedIdentifier,
    /// An identifier-start character immediately follows a numeric literal with no
    /// separator (`123abc`, `1x`, `0.0e`, `100_`) — PostgreSQL's "trailing junk after
    /// numeric literal" scanner error. A number is a maximal-munch lexeme, so anything
    /// identifier-ish abutting it is malformed input, not a new token; the bad-radix
    /// (`0x`, `0b0x`) and misplaced-`_` (`100__000`, `1_000._5`) forms all decompose to
    /// this — the number ends at the last well-formed digit and the remainder is junk.
    /// Gated by [`NumericLiteralSyntax::reject_trailing_junk`]: PostgreSQL/SQLite reject
    /// it, DuckDB/MySQL lex loosely and let the trailing text be an aliased identifier, so
    /// under a loose dialect no number ever raises this. The span covers the whole
    /// malformed lexeme (the number plus the abutting identifier run).
    ///
    /// [`NumericLiteralSyntax::reject_trailing_junk`]: crate::ast::dialect::NumericLiteralSyntax::reject_trailing_junk
    TrailingJunkAfterNumber,
    /// A SQLite/MySQL `x'…'`/`X'…'` hexadecimal blob literal whose body is not an even
    /// count of ASCII hex digits — an odd length (`x'ABC'`, `x'0'`) or a non-hex byte
    /// (`x'XY'`). Both engines reject it at tokenize time as a syntax error (SQLite
    /// "unrecognized token", MySQL `ER_PARSE_ERROR` — probed), so the eager
    /// [`blob_literals`](crate::ast::dialect::StringLiteralSyntax::blob_literals) scan
    /// raises it up front rather than deferring the digit check the way a
    /// [`bit_string_literals`] `X'…'` does. The span covers the whole malformed lexeme.
    /// The empty body `x''` is valid (a zero-byte blob) and never raises this.
    ///
    /// [`bit_string_literals`]: crate::ast::dialect::StringLiteralSyntax::bit_string_literals
    MalformedBlobLiteral,
    /// The source is longer than `u32::MAX` bytes, so its offsets do not fit the
    /// `u32` spans this tokenizer uses. The span is not meaningful here; the
    /// whole input is rejected. Huge inputs are served by statement streaming, not by
    /// widening spans.
    SourceTooLong,
}

impl LexErrorKind {
    /// A short, stable human-readable description.
    pub const fn message(&self) -> &'static str {
        match self {
            Self::UnterminatedString => "unterminated string literal",
            Self::UnterminatedQuotedIdent => "unterminated quoted identifier",
            Self::UnterminatedBlockComment => "unterminated block comment",
            Self::UnterminatedDollarQuote => "unterminated dollar-quoted string",
            Self::StrayByte => "stray byte",
            Self::InvalidEscapeSequence => "invalid escape sequence in string literal",
            Self::NulByteInString => "NUL byte in string literal",
            Self::NulByteInIdentifier => "NUL byte in quoted identifier",
            Self::NulByteInComment => "NUL byte in comment",
            Self::ZeroLengthDelimitedIdentifier => "zero-length delimited identifier",
            Self::TrailingJunkAfterNumber => "trailing junk after numeric literal",
            Self::MalformedBlobLiteral => "malformed hexadecimal blob literal",
            Self::SourceTooLong => "source exceeds u32::MAX bytes",
        }
    }

    /// A stable snake_case machine kind for the bindings' `ParseDiagnostic.kind`.
    ///
    /// When a lexical fault widens into a
    /// [`ParseError`](crate::error::ParseError) (via
    /// [`ParseErrorKind::Lexical`](crate::error::ParseErrorKind::Lexical)), this
    /// is the wire string an editor branches on to tell one lexical category from
    /// another — distinct from the `"syntax"` a grammar mismatch carries. Each
    /// string is part of the serialized binding surface
    /// (`docs/schema-contract.md`); the `kind` field is a `&'static str`, so new
    /// variants add new strings additively without a wire-shape change. Kept
    /// separate from [`message`](Self::message) so the human wording can change
    /// without moving the machine contract.
    pub const fn machine_kind(&self) -> &'static str {
        match self {
            Self::UnterminatedString => "unterminated_string",
            Self::UnterminatedQuotedIdent => "unterminated_quoted_identifier",
            Self::UnterminatedBlockComment => "unterminated_block_comment",
            Self::UnterminatedDollarQuote => "unterminated_dollar_quote",
            Self::StrayByte => "stray_byte",
            Self::InvalidEscapeSequence => "invalid_escape_sequence",
            Self::NulByteInString => "nul_byte_in_string",
            Self::NulByteInIdentifier => "nul_byte_in_identifier",
            Self::NulByteInComment => "nul_byte_in_comment",
            Self::ZeroLengthDelimitedIdentifier => "zero_length_delimited_identifier",
            Self::TrailingJunkAfterNumber => "trailing_junk_after_number",
            Self::MalformedBlobLiteral => "malformed_blob_literal",
            Self::SourceTooLong => "source_too_long",
        }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at bytes {}..{}",
            self.kind.message(),
            self.span.start(),
            self.span.end()
        )
    }
}

impl std::error::Error for LexError {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every lexical kind, enumerated so the round-trip and distinctness checks
    /// see the whole set. A new variant forces the exhaustive `message`/
    /// `machine_kind` matches to grow; adding it here keeps this coverage honest.
    const ALL: &[LexErrorKind] = &[
        LexErrorKind::UnterminatedString,
        LexErrorKind::UnterminatedQuotedIdent,
        LexErrorKind::UnterminatedBlockComment,
        LexErrorKind::UnterminatedDollarQuote,
        LexErrorKind::StrayByte,
        LexErrorKind::InvalidEscapeSequence,
        LexErrorKind::NulByteInString,
        LexErrorKind::NulByteInIdentifier,
        LexErrorKind::NulByteInComment,
        LexErrorKind::ZeroLengthDelimitedIdentifier,
        LexErrorKind::TrailingJunkAfterNumber,
        LexErrorKind::MalformedBlobLiteral,
        LexErrorKind::SourceTooLong,
    ];

    #[test]
    fn machine_kinds_are_distinct_stable_snake_case() {
        let mut seen = std::collections::HashSet::new();
        for kind in ALL {
            let machine = kind.machine_kind();
            assert!(!machine.is_empty(), "{kind:?} has an empty machine kind");
            assert!(
                machine.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
                "{kind:?} machine kind {machine:?} is not snake_case"
            );
            // Must never collide with the non-lexical parser kinds.
            assert_ne!(machine, "syntax");
            assert_ne!(machine, "recursion_limit_exceeded");
            assert!(seen.insert(machine), "duplicate machine kind {machine:?}");
        }
    }
}
