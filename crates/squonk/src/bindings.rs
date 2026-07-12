// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared serde-facing shapes for non-Rust bindings.
//!
//! These types keep Python and WASM/JS on the same boundary contract without adding
//! `serde_json` to the published parser crate. Binding crates choose their concrete
//! format; this module only supplies `Serialize` views over parser-owned data.

use serde::Serialize;

use crate::ast::{Keyword, SourceStore, Span};

/// Wire-schema version of the serialized binding surface (`docs/schema-contract.md`).
///
/// This is the single version number for the *serialized JSON shape* that the
/// Python and WASM bindings emit — the AST node graph plus every envelope type in
/// this module (`ParseDocument`, `RecoveredDocument`, `TokenizeDocument`,
/// `ParseDiagnostic`, `DialectInfo`, `ResolverMetadata`, …). It is deliberately
/// *independent* of the crate/package version (`CARGO_PKG_VERSION`, surfaced as
/// `version()` / `__version__`): a patch release can change code without touching
/// the wire, and a wire-breaking change need not coincide with a major crate bump.
///
/// The Rust semver gate (`cargo xtask semver`, `docs/stable-api.md`) cannot see a
/// renamed serde field, a changed enum representation, or an
/// `skip_serializing_if` omission change, so this number and the checked-in
/// snapshot under `release/schema/` are the wire contract's own gate. Both
/// bindings expose it beside their package version (`schemaVersion()` in JS,
/// `__schema_version__` in Python).
///
/// # Evolution
///
/// - **Additive-optional** changes (a new `skip_serializing_if` field, a variant
///   on a `#[non_exhaustive]` enum) keep this number and regenerate the snapshot.
/// - **Breaking** changes (a renamed/removed field, a changed enum tag/`flatten`
///   representation, a changed omission behaviour) bump this number and add a new
///   `release/schema/wire-schema.v{N}.json`, keeping the prior one frozen.
///
/// The full rules and the one bump procedure shared by Rust, Python, and npm live
/// in `docs/schema-contract.md`; the `wire_schema` integration test fails loudly
/// on any unreviewed shape change and points there.
pub const WIRE_SCHEMA_VERSION: u32 = 1;
use crate::ast::dialect::{SupportEvidence, SupportTier};
use crate::dialect::BuiltinDialect;
use crate::error::{Found, ParseError, ParseErrorKind};
use crate::parser::{Parsed, Recovered};
use crate::tokenizer::{Operator, Punctuation, Token, TokenKind, TriviaKind, TriviaRange};

/// A serializable parse document for language bindings.
#[derive(Serialize)]
#[serde(bound(serialize = "Parsed<S>: Serialize"))]
pub struct ParseDocument<'a, S: SourceStore> {
    /// Canonical dialect name used for this parse.
    pub dialect: &'static str,
    /// The serde-compatible parsed root.
    #[serde(flatten)]
    pub parsed: &'a Parsed<S>,
    /// Captured whitespace/comment runs, when parse options requested trivia.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trivia: Option<Vec<BindingTrivia>>,
    /// Resolver metadata omitted from the parsed root's dynamic symbol table.
    pub resolver: ResolverMetadata,
}

impl<'a, S: SourceStore> ParseDocument<'a, S> {
    /// Borrow `parsed` as a binding document tagged with `dialect`.
    pub fn new(parsed: &'a Parsed<S>, dialect: BuiltinDialect) -> Self {
        Self {
            dialect: dialect.name(),
            parsed,
            trivia: binding_trivia(parsed.source(), parsed.trivia()),
            resolver: ResolverMetadata::new(),
        }
    }
}

/// A serializable recovering parse document for language bindings.
#[derive(Serialize)]
#[serde(bound(serialize = "ParseDocument<'a, S>: Serialize"))]
pub struct RecoveredDocument<'a, S: SourceStore> {
    /// The partial parsed root and resolver metadata.
    #[serde(flatten)]
    pub parsed: ParseDocument<'a, S>,
    /// Per-statement diagnostics collected during recovery.
    pub errors: Vec<ParseDiagnostic>,
}

impl<'a, S: SourceStore> RecoveredDocument<'a, S> {
    /// Borrow `recovered` as a binding document tagged with `dialect`.
    pub fn new(recovered: &'a Recovered<S>, dialect: BuiltinDialect) -> Self {
        Self {
            parsed: ParseDocument::new(recovered.parsed(), dialect),
            errors: recovered
                .errors()
                .iter()
                .map(ParseDiagnostic::from)
                .collect(),
        }
    }
}

/// Resolver metadata needed to interpret `Symbol` ids in serialized AST nodes.
#[derive(Clone, Debug, Serialize)]
pub struct ResolverMetadata {
    /// First one-based symbol id stored in the parse root's dynamic `symbols` array.
    pub dynamic_base: u32,
    /// Fixed keyword-backed symbols, omitted from the dynamic `symbols` array.
    pub keyword_symbols: Vec<KeywordSymbol>,
}

impl ResolverMetadata {
    /// Build resolver metadata for the current keyword inventory.
    pub fn new() -> Self {
        Self {
            dynamic_base: Keyword::ALL.len() as u32 + 1,
            keyword_symbols: Keyword::ALL
                .iter()
                .copied()
                .map(|keyword| KeywordSymbol {
                    symbol: keyword.symbol().as_u32(),
                    text: keyword.as_str(),
                })
                .collect(),
        }
    }
}

impl Default for ResolverMetadata {
    fn default() -> Self {
        Self::new()
    }
}

/// One fixed keyword-backed symbol entry.
#[derive(Clone, Debug, Serialize)]
pub struct KeywordSymbol {
    /// One-based symbol id used inside serialized AST nodes.
    pub symbol: u32,
    /// Canonical keyword text for this symbol.
    pub text: &'static str,
}

/// A supported built-in dialect, its accepted aliases, and its release-contract
/// support tier with the named source of truth behind that tier.
#[derive(Clone, Debug, Serialize)]
pub struct DialectInfo {
    /// Canonical lower-case dialect name.
    pub name: &'static str,
    /// Case-insensitive names accepted by [`BuiltinDialect::from_name`].
    pub aliases: &'static [&'static str],
    /// Release-contract support tier for this preset (stable / preview /
    /// experimental) — the promise level the stable release makes for it.
    pub tier: SupportTier,
    /// The named source of truth backing `tier`; a stable tier always cites
    /// authoritative evidence (see [`SupportEvidence::is_authoritative`]).
    pub evidence: SupportEvidence,
}

/// Return every built-in dialect compiled into this build.
pub fn supported_dialects() -> Vec<DialectInfo> {
    BuiltinDialect::ALL
        .iter()
        .copied()
        .map(|dialect| DialectInfo {
            name: dialect.name(),
            aliases: dialect.aliases(),
            tier: dialect.support_tier(),
            evidence: dialect.support_evidence(),
        })
        .collect()
}

/// Serializable tokenizer output for language bindings.
#[derive(Clone, Debug, Serialize)]
pub struct TokenizeDocument {
    /// Original SQL source.
    pub source: String,
    /// Canonical dialect name used for tokenization.
    pub dialect: &'static str,
    /// Non-trivia lexical tokens in source order.
    pub tokens: Vec<BindingToken>,
    /// Captured whitespace/comment runs, when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trivia: Option<Vec<BindingTrivia>>,
}

impl TokenizeDocument {
    /// Build tokenizer output without trivia.
    pub fn new(source: &str, dialect: BuiltinDialect, tokens: &[Token]) -> Self {
        Self {
            source: source.to_owned(),
            dialect: dialect.name(),
            tokens: tokens
                .iter()
                .copied()
                .map(|token| BindingToken::new(source, token))
                .collect(),
            trivia: None,
        }
    }

    /// Build tokenizer output with captured trivia.
    pub fn with_trivia(
        source: &str,
        dialect: BuiltinDialect,
        tokens: &[Token],
        trivia: &[TriviaRange],
    ) -> Self {
        Self {
            trivia: Some(
                trivia
                    .iter()
                    .copied()
                    .map(|range| BindingTrivia::new(source, range))
                    .collect(),
            ),
            ..Self::new(source, dialect, tokens)
        }
    }
}

/// A serializable token with exact source text and typed lexical category.
#[derive(Clone, Debug, Serialize)]
pub struct BindingToken {
    /// Typed token category. Flattening gives bindings a stable `kind` discriminator.
    #[serde(flatten)]
    pub kind: BindingTokenKind,
    /// Half-open byte range in the original source.
    pub span: SourceSpan,
    /// Exact source text covered by `span`.
    pub text: String,
}

impl BindingToken {
    fn new(source: &str, token: Token) -> Self {
        Self {
            kind: BindingTokenKind::from(token.kind),
            span: SourceSpan::from_span(token.span),
            text: slice_bytes(source, token.span),
        }
    }
}

/// Token categories serialized with a `kind` discriminator.
///
/// `#[non_exhaustive]`: this is an output-only view of the tokenizer's evolving
/// vocabulary (it already carries an [`Unknown`](Self::Unknown) growth
/// placeholder), and the wire contract already treats a new variant on a
/// `#[non_exhaustive]` enum as additive-compatible (`docs/schema-contract.md`).
/// Marking it so aligns the Rust surface with that additive-growth promise —
/// downstream only ever receives and inspects it (construction is via
/// [`From<TokenKind>`], in-crate), so a future token category is a minor bump on
/// both surfaces rather than a break. Non-exhaustive does not change the serde
/// output.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind")]
#[non_exhaustive]
pub enum BindingTokenKind {
    /// An unquoted identifier or unrecognized word.
    Word,
    /// A recognized SQL keyword.
    Keyword {
        /// Canonical lower-case keyword text.
        keyword: &'static str,
    },
    /// Numeric literal token.
    Number,
    /// String literal token.
    String,
    /// Delimited identifier token.
    QuotedIdent,
    /// Prepared-statement parameter token.
    Parameter,
    /// DuckDB positional column reference token.
    PositionalColumn,
    /// MySQL-style session variable token.
    Variable,
    /// Snowflake stage-reference token (`@stage` / `@~` / `@%table`).
    StageReference,
    /// Operator token.
    Operator {
        /// Closed operator variant name.
        operator: &'static str,
    },
    /// Punctuation token.
    Punctuation {
        /// Closed punctuation variant name.
        punctuation: &'static str,
    },
    /// Unknown token placeholder for future recovering tokenizers.
    Unknown,
}

impl From<TokenKind> for BindingTokenKind {
    fn from(kind: TokenKind) -> Self {
        match kind {
            TokenKind::Word => Self::Word,
            TokenKind::Keyword(keyword) => Self::Keyword {
                keyword: keyword.as_str(),
            },
            TokenKind::Number => Self::Number,
            TokenKind::String => Self::String,
            TokenKind::QuotedIdent => Self::QuotedIdent,
            TokenKind::Parameter => Self::Parameter,
            TokenKind::PositionalColumn => Self::PositionalColumn,
            TokenKind::Variable => Self::Variable,
            TokenKind::StageReference => Self::StageReference,
            TokenKind::Operator(operator) => Self::Operator {
                operator: operator_name(operator),
            },
            TokenKind::Punctuation(punctuation) => Self::Punctuation {
                punctuation: punctuation_name(punctuation),
            },
            TokenKind::Unknown => Self::Unknown,
        }
    }
}

/// A serializable trivia run with exact source text.
#[derive(Clone, Debug, Serialize)]
pub struct BindingTrivia {
    /// Closed trivia category name.
    pub kind: &'static str,
    /// Half-open byte range in the original source.
    pub span: SourceSpan,
    /// Exact source text covered by `span`.
    pub text: String,
}

impl BindingTrivia {
    fn new(source: &str, trivia: TriviaRange) -> Self {
        Self {
            kind: trivia_kind_name(trivia.kind()),
            span: SourceSpan::from_span(trivia.span()),
            text: slice_bytes(source, trivia.span()),
        }
    }
}

/// A language-binding diagnostic with byte spans and structured parser context.
#[derive(Clone, Debug, Serialize)]
pub struct ParseDiagnostic {
    /// Human-readable diagnostic text.
    pub message: String,
    /// Stable snake-case diagnostic category.
    pub kind: &'static str,
    /// Byte span for the diagnostic, or `None` for synthetic/no-source errors.
    pub span: Option<SourceSpan>,
    /// Transitional aliases for the original Python binding contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_start: Option<u32>,
    /// Transitional alias for `span.end`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_end: Option<u32>,
    /// Parser expectation text, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// Text describing what the parser found, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub found: Option<String>,
}

impl From<&ParseError> for ParseDiagnostic {
    fn from(error: &ParseError) -> Self {
        let span = SourceSpan::from_diagnostic_span(error.span);
        Self {
            message: error.to_string(),
            kind: parse_error_kind(error.kind),
            span,
            span_start: span.map(|span| span.start),
            span_end: span.map(|span| span.end),
            expected: Some(error.expected.to_string()),
            found: found_text(&error.found),
        }
    }
}

/// A half-open byte range in the original SQL source.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct SourceSpan {
    /// Inclusive start byte offset.
    pub start: u32,
    /// Exclusive end byte offset.
    pub end: u32,
}

impl SourceSpan {
    /// Convert a real source span to a binding span.
    pub fn from_span(span: Span) -> Self {
        Self {
            start: span.start(),
            end: span.end(),
        }
    }

    fn from_diagnostic_span(span: Span) -> Option<Self> {
        if span.is_synthetic() {
            None
        } else {
            Some(Self::from_span(span))
        }
    }
}

/// Backwards-compatible Rust type alias for diagnostic spans.
pub type DiagnosticSpan = SourceSpan;

fn parse_error_kind(kind: ParseErrorKind) -> &'static str {
    match kind {
        ParseErrorKind::Syntax => "syntax",
        ParseErrorKind::RecursionLimitExceeded => "recursion_limit_exceeded",
        // A widened lexical fault surfaces its own tokenizer category so an editor
        // can tell an unterminated string from a grammar mismatch (kind=Syntax).
        ParseErrorKind::Lexical(lex_kind) => lex_kind.machine_kind(),
    }
}

fn found_text(found: &Found) -> Option<String> {
    match found {
        Found::EndOfInput => Some("end of input".to_owned()),
        Found::Text(text) => Some(text.to_string()),
    }
}

fn binding_trivia(source: &str, trivia: &[TriviaRange]) -> Option<Vec<BindingTrivia>> {
    if trivia.is_empty() {
        None
    } else {
        Some(
            trivia
                .iter()
                .copied()
                .map(|range| BindingTrivia::new(source, range))
                .collect(),
        )
    }
}

fn operator_name(operator: Operator) -> &'static str {
    match operator {
        Operator::Plus => "Plus",
        Operator::Minus => "Minus",
        Operator::Star => "Star",
        Operator::Slash => "Slash",
        Operator::SlashSlash => "SlashSlash",
        Operator::Percent => "Percent",
        Operator::Eq => "Eq",
        Operator::EqEq => "EqEq",
        Operator::Lt => "Lt",
        Operator::LtEq => "LtEq",
        Operator::Gt => "Gt",
        Operator::GtEq => "GtEq",
        Operator::NotEq => "NotEq",
        Operator::LtEqGt => "LtEqGt",
        Operator::Concat => "Concat",
        Operator::AmpAmp => "AmpAmp",
        Operator::Bang => "Bang",
        Operator::Pipe => "Pipe",
        Operator::Amp => "Amp",
        Operator::Caret => "Caret",
        Operator::CaretAt => "CaretAt",
        Operator::Tilde => "Tilde",
        Operator::ShiftLeft => "ShiftLeft",
        Operator::ShiftRight => "ShiftRight",
        Operator::Hash => "Hash",
        Operator::Arrow => "Arrow",
        Operator::ColonEquals => "ColonEquals",
        Operator::AtGt => "AtGt",
        Operator::LtAt => "LtAt",
        Operator::MinusGt => "MinusGt",
        Operator::MinusGtGt => "MinusGtGt",
        Operator::PipeArrow => "PipeArrow",
        Operator::Question => "Question",
        Operator::QuestionPipe => "QuestionPipe",
        Operator::QuestionAmp => "QuestionAmp",
        Operator::AtQuestion => "AtQuestion",
        Operator::AtAt => "AtAt",
        Operator::HashGt => "HashGt",
        Operator::HashGtGt => "HashGtGt",
        Operator::HashMinus => "HashMinus",
        Operator::Custom => "Custom",
    }
}

fn punctuation_name(punctuation: Punctuation) -> &'static str {
    match punctuation {
        Punctuation::LParen => "LParen",
        Punctuation::RParen => "RParen",
        Punctuation::Comma => "Comma",
        Punctuation::Semicolon => "Semicolon",
        Punctuation::Dot => "Dot",
        Punctuation::LBracket => "LBracket",
        Punctuation::RBracket => "RBracket",
        Punctuation::LBrace => "LBrace",
        Punctuation::RBrace => "RBrace",
        Punctuation::Colon => "Colon",
        Punctuation::DoubleColon => "DoubleColon",
        Punctuation::At => "At",
    }
}

fn trivia_kind_name(kind: TriviaKind) -> &'static str {
    match kind {
        TriviaKind::LineComment => "LineComment",
        TriviaKind::BlockComment => "BlockComment",
        TriviaKind::Whitespace => "Whitespace",
    }
}

fn slice_bytes(source: &str, span: Span) -> String {
    String::from_utf8_lossy(&source.as_bytes()[span.start() as usize..span.end() as usize])
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::{LexError, LexErrorKind};

    /// Every lexical kind, so the widening round-trip covers the whole set.
    const ALL_LEX_KINDS: &[LexErrorKind] = &[
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
    fn widened_lexical_faults_surface_distinct_diagnostic_kinds() {
        let mut seen = std::collections::HashSet::new();
        for &lex_kind in ALL_LEX_KINDS {
            // The real widening path: LexError -> ParseError -> ParseDiagnostic.
            let parse_error = ParseError::from(LexError::new(lex_kind, Span::new(1, 4)));
            let diagnostic = ParseDiagnostic::from(&parse_error);

            assert_eq!(
                diagnostic.kind,
                lex_kind.machine_kind(),
                "diagnostic kind must be the lexical machine kind for {lex_kind:?}"
            );
            // The seam's contract: a lexical fault carries its own machine kind,
            // distinct from the `syntax` a grammar mismatch reports.
            assert_ne!(diagnostic.kind, "syntax", "for {lex_kind:?}");
            assert!(
                seen.insert(diagnostic.kind),
                "duplicate diagnostic kind {:?}",
                diagnostic.kind
            );
            // Span survives the widening.
            let span = diagnostic.span.expect("real span");
            assert_eq!((span.start, span.end), (1, 4));
        }
    }

    #[test]
    fn ordinary_syntax_error_still_maps_to_syntax() {
        let err = ParseError::new(Span::new(0, 1), "expression", "`,`");
        assert_eq!(ParseDiagnostic::from(&err).kind, "syntax");
    }
}
