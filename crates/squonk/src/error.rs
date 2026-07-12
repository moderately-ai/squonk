// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Structured parse errors and the error-sink seam.
//!
//! A [`ParseError`] is a self-contained diagnostic: it carries a byte [`Span`]
//! into the source plus a description of what the parser *expected* and what it
//! *found*. It deliberately does not borrow from the source or carry a
//! `LineIndex`, keeping it lifetime-free and cheap to move, store, and return —
//! line/column rendering is layered on later from the source + [`Span`].
//!
//! Errors reach policy through one seam, the [`ErrorSink`]. Parser call sites
//! only ever build a `ParseError` and hand it to `report`; they never decide
//! what happens next. v1 ships a fail-fast policy ([`FailFastSink`]); a future
//! resilient parser can collect many errors and keep going by dropping in a
//! different sink, and because the *only* coupling is the `report` call, no
//! parser call site has to change for that swap.

use std::borrow::Cow;
use std::fmt;

use squonk_ast::Span;

use crate::tokenizer::LexErrorKind;

/// A structured parse error: a source location plus expected/found context.
///
/// The byte [`Span`] is mandatory — it is the anchor every downstream
/// diagnostic (line/column resolution, squiggle rendering, LSP ranges) builds
/// on. `expected`/`found` are human-readable descriptions rather than token
/// enums so this type stays decoupled from the tokenizer's evolving token
/// vocabulary; the inner representation can grow into structured expectation
/// sets later without changing this public shape.
///
/// # Evolvability seam
///
/// The private `hint` field is deliberate (ADR-0005): it closes downstream
/// struct-literal construction to this crate's own constructors, which reserves
/// room to grow a `ParseError` — a `help`/hint channel, structured labels — as a
/// non-breaking *minor* change instead of an API break. Because a struct with a
/// private field already forces downstream matches to end in `..`, any later
/// field (public or private) is absorbed by that rest pattern, so this one
/// private field future-proofs the whole struct's *API* without committing the
/// hint's representation to the public surface (it is reached only through
/// [`hint`](Self::hint), so its storage type can change freely). The public
/// `span`/`kind`/`expected`/`found` fields stay directly readable and matchable.
///
/// The hint is stored *boxed* because a `ParseError` is returned by value
/// through every recursive-descent frame (`ParseResult<T>`), so its size is on
/// the parser's hot stack path — the drift sentinel in `parser::recursion`
/// guards it. A rare, cold hint therefore costs one pointer inline and allocates
/// only when actually attached, keeping the common error lean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Byte range of the offending text in the original source.
    pub span: Span,
    /// Which category of failure this is. Almost every error is a
    /// [`ParseErrorKind::Syntax`] mismatch whose specifics live in
    /// `expected`/`found`; the resource-limit and [`Lexical`] kinds are
    /// programmatically distinguishable so a caller can tell "the SQL is
    /// malformed" from "the SQL is too deeply nested to parse safely" — or from a
    /// specific lexical fault — without string-matching the message.
    ///
    /// [`Lexical`]: ParseErrorKind::Lexical
    pub kind: ParseErrorKind,
    /// What the parser was looking for at this position.
    pub expected: Expected,
    /// What the parser actually encountered.
    pub found: Found,
    /// Optional actionable hint, reached only through [`hint`](Self::hint). This
    /// is the diagnostics-growth channel the private-field seam reserves; v1
    /// ships it empty and a later dialect-aware hinting pass populates it. Boxed
    /// behind one thin pointer (see the type's evolvability-seam note) so the cold
    /// hint costs a single word inline and the hot error stays lean.
    hint: Option<Box<Cow<'static, str>>>,
}

/// The category of a [`ParseError`].
///
/// The parser's failures are overwhelmingly ordinary syntax mismatches, so
/// [`Syntax`](Self::Syntax) is the default the [`ParseError::new`] constructor
/// stamps. The distinct [`RecursionLimitExceeded`](Self::RecursionLimitExceeded)
/// kind exists because that condition is a *robustness* outcome rather than a
/// grammar one — a caller feeding untrusted SQL wants to recognize it (and, e.g.,
/// surface it differently or raise the limit) without parsing the human-readable
/// message. [`Lexical`](Self::Lexical) carries the tokenizer's own
/// [`LexErrorKind`] across the widening into a `ParseError`, so a fault that
/// began in the lexer stays machine-distinguishable (an unterminated string vs a
/// stray byte) instead of collapsing into `Syntax`.
///
/// `#[non_exhaustive]`: this enum is a growth axis (new robustness guards, new
/// carried lexical categories), so it reserves additive variants for a minor
/// release rather than freezing the set at 1.0. In-crate matches stay exhaustive
/// with no wildcard; only downstream matches must add a `_` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ParseErrorKind {
    /// A grammar mismatch: the parser expected one production and found another.
    /// The [`expected`](ParseError::expected)/[`found`](ParseError::found) fields
    /// carry the specifics.
    Syntax,
    /// The input nested deeper than the parser's configured recursion limit, so
    /// parsing was stopped with this clean error instead of recursing further and
    /// overflowing the stack. A deliberate DoS guard for untrusted SQL;
    /// the limit is configurable via
    /// [`Parser::with_recursion_limit`](crate::parser::Parser::with_recursion_limit).
    RecursionLimitExceeded,
    /// A lexical fault that widened into a `ParseError` at a `?` boundary,
    /// carrying the tokenizer's machine-matchable [`LexErrorKind`]. The span and a
    /// faithful message are preserved; the carried kind lets a caller (or a
    /// binding) tell one lexical category from another without string-matching.
    Lexical(LexErrorKind),
}

impl ParseError {
    /// Build a [`Syntax`](ParseErrorKind::Syntax) error at `span`.
    ///
    /// `expected` and `found` take `impl Into<…>` so the common call sites stay
    /// terse: a fixed description is a string literal (zero allocation), a
    /// dynamic lexeme is an owned `String`, and end-of-input is [`Found::EndOfInput`].
    ///
    /// ```
    /// use squonk::error::{Found, ParseError, ParseErrorKind};
    /// use squonk::ast::Span;
    ///
    /// let err = ParseError::new(Span::new(7, 11), "expression", Found::EndOfInput);
    /// assert_eq!(err.span, Span::new(7, 11));
    /// assert_eq!(err.kind, ParseErrorKind::Syntax);
    /// ```
    pub fn new(span: Span, expected: impl Into<Expected>, found: impl Into<Found>) -> Self {
        Self {
            span,
            kind: ParseErrorKind::Syntax,
            expected: expected.into(),
            found: found.into(),
            hint: None,
        }
    }

    /// Build a [`RecursionLimitExceeded`](ParseErrorKind::RecursionLimitExceeded)
    /// error at `span` — the source location where the over-limit nesting was
    /// detected (the token that would have opened one level too many).
    ///
    /// The `expected`/`found` text is descriptive only; the load-bearing signal is
    /// the [`kind`](ParseError::kind), so a caller branches on that rather than the
    /// message.
    ///
    /// ```
    /// use squonk::error::{ParseError, ParseErrorKind};
    /// use squonk::ast::Span;
    ///
    /// let err = ParseError::recursion_limit_exceeded(Span::new(120, 121));
    /// assert_eq!(err.kind, ParseErrorKind::RecursionLimitExceeded);
    /// ```
    pub fn recursion_limit_exceeded(span: Span) -> Self {
        Self {
            span,
            kind: ParseErrorKind::RecursionLimitExceeded,
            expected: Expected::from("input within the configured recursion-depth limit"),
            found: Found::from("input nested past the recursion-depth limit"),
            hint: None,
        }
    }

    /// Build a [`Lexical`](ParseErrorKind::Lexical) error at `span`, carrying the
    /// tokenizer's [`LexErrorKind`] so the lexical category survives the widening
    /// into a `ParseError`.
    ///
    /// This is the sole widening site's constructor: the `kind`'s stable
    /// [`message`](LexErrorKind::message) becomes the `found`, against the generic
    /// expectation a well-formed token would have met. The precise byte span is
    /// preserved; the machine-matchable kind rides on
    /// [`ParseErrorKind::Lexical`] so downstream (and both bindings) can tell an
    /// unterminated string from a stray byte without parsing the message.
    ///
    /// ```
    /// use squonk::error::{ParseError, ParseErrorKind};
    /// use squonk::tokenizer::LexErrorKind;
    /// use squonk::ast::Span;
    ///
    /// let err = ParseError::lexical(Span::new(0, 5), LexErrorKind::UnterminatedString);
    /// assert_eq!(err.kind, ParseErrorKind::Lexical(LexErrorKind::UnterminatedString));
    /// ```
    pub fn lexical(span: Span, kind: LexErrorKind) -> Self {
        Self {
            span,
            kind: ParseErrorKind::Lexical(kind),
            expected: Expected::from("a well-formed SQL token"),
            found: Found::from(kind.message()),
            hint: None,
        }
    }

    /// Attach an actionable hint, returning the error for chaining.
    ///
    /// The hint is the additive diagnostics-growth channel the private-field seam
    /// reserves (ADR-0005). It is boxed on attach — a cold, rare path — so the
    /// common error keeps its lean inline footprint on the parser's hot stack.
    pub fn with_hint(mut self, hint: impl Into<Cow<'static, str>>) -> Self {
        self.hint = Some(Box::new(hint.into()));
        self
    }

    /// The attached hint, if any.
    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref().map(|hint| hint.as_ref())
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            // A recursion-limit failure is not an "expected X, found Y" mismatch —
            // the offending thing is the *depth*, not a token — so it reads as its
            // own message rather than through the expected/found template.
            ParseErrorKind::RecursionLimitExceeded => {
                f.write_str("input nested too deeply: parser recursion limit exceeded")?;
            }
            // A widened lexical fault carries meaningful expected/found text (a
            // well-formed token vs the specific lexical message), so it renders
            // through the same template as an ordinary syntax mismatch — the
            // machine-matchable distinction lives in `kind`, not the message.
            ParseErrorKind::Syntax | ParseErrorKind::Lexical(_) => {
                let expected = &self.expected;
                let found = &self.found;
                write!(f, "expected {expected}, found {found}")?;
            }
        }

        // Positional info is byte offsets, not line/column: the error does not
        // carry the source, and spans are byte ranges by design (ADR-0002). A
        // synthetic span is not a real source range, so render it as such
        // rather than leaking the `u32::MAX..0` sentinel into a message.
        if self.span.is_synthetic() {
            f.write_str(" at an unknown position")
        } else {
            let start = self.span.start();
            let end = self.span.end();
            write!(f, " at bytes {start}..{end}")
        }
    }
}

impl std::error::Error for ParseError {}

/// What the parser was looking for at the error site.
///
/// A short description such as `"expression"` or ``"`)`"``. The inner storage is
/// `Cow<'static, str>` so fixed descriptions (the overwhelming majority) cost no
/// allocation, while a dynamically composed message (e.g. ``"`,` or `)`"``) can
/// still be carried as an owned `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected(Cow<'static, str>);

impl Expected {
    /// Borrow the description text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Expected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&'static str> for Expected {
    fn from(description: &'static str) -> Self {
        Self(Cow::Borrowed(description))
    }
}

impl From<String> for Expected {
    fn from(description: String) -> Self {
        Self(Cow::Owned(description))
    }
}

impl From<Cow<'static, str>> for Expected {
    fn from(description: Cow<'static, str>) -> Self {
        Self(description)
    }
}

/// What the parser actually encountered at the error site.
///
/// End-of-input is a distinct, common case that has no source text to quote, so
/// it is a variant of its own rather than a magic string — this keeps both the
/// rendered message ("found end of input") and any future programmatic handling
/// honest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// A concrete piece of source text (a lexeme or a description of one).
    Text(Cow<'static, str>),
    /// The parser reached the end of the input.
    EndOfInput,
}

impl fmt::Display for Found {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => write!(f, "{text}"),
            Self::EndOfInput => f.write_str("end of input"),
        }
    }
}

impl From<&'static str> for Found {
    fn from(text: &'static str) -> Self {
        Self::Text(Cow::Borrowed(text))
    }
}

impl From<String> for Found {
    fn from(text: String) -> Self {
        Self::Text(Cow::Owned(text))
    }
}

impl From<Cow<'static, str>> for Found {
    fn from(text: Cow<'static, str>) -> Self {
        Self::Text(text)
    }
}

/// Result of a parse step. `Err` is the fail-fast short-circuit; the same
/// [`ParseError`] is what a parser hands to its [`ErrorSink`].
pub type ParseResult<T> = Result<T, ParseError>;

/// The single seam between error *production* and error *policy*.
///
/// Parser call sites only ever call [`report`](ErrorSink::report). What happens
/// to a reported error — abort now, or record it and recover — is entirely the
/// sink's concern. That separation is what lets a resilient, multi-error sink
/// replace the fail-fast one (below) with no change to any call site.
pub trait ErrorSink {
    /// Hand a freshly built error to the sink.
    fn report(&mut self, error: ParseError);
}

// Threading a sink through deep recursive descent means passing it by mutable
// reference. Implementing the trait for `&mut T` lets a function written
// generically over `S: ErrorSink` accept a borrowed sink and forward it onward
// without explicit reborrows — the same ergonomics `std::io::Write` gives. The
// `?Sized` bound also covers `&mut dyn ErrorSink` for dynamic dispatch.
impl<T: ErrorSink + ?Sized> ErrorSink for &mut T {
    fn report(&mut self, error: ParseError) {
        (**self).report(error);
    }
}

/// Fail-fast error policy: keep the first reported error, ignore the rest.
///
/// In v1 the parser unwinds on the first error (via `Err`), so this sink
/// normally observes exactly one error. Keeping the *first* rather than the
/// last is deliberate: if a recovery path is ever added that reports more than
/// once before unwinding, the earliest — and usually most actionable —
/// diagnostic is the one that survives. A resilient sink that accumulates every
/// error is a future drop-in replacement and does not live here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FailFastSink {
    first: Option<ParseError>,
}

impl FailFastSink {
    /// Create an empty sink.
    pub const fn new() -> Self {
        Self { first: None }
    }
}

impl ErrorSink for FailFastSink {
    fn report(&mut self, error: ParseError) {
        // First write wins; later reports are dropped so the earliest diagnostic
        // is preserved.
        if self.first.is_none() {
            self.first = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_preserves_its_span() {
        let span = Span::new(7, 11);
        let err = ParseError::new(span, "expression", "keyword `FROM`");

        assert_eq!(err.span, span);
        assert_eq!(err.span.start(), 7);
        assert_eq!(err.span.end(), 11);
    }

    #[test]
    fn display_is_readable_and_includes_position() {
        let err = ParseError::new(Span::new(7, 11), "expression", "keyword `FROM`");
        let msg = err.to_string();

        assert!(msg.contains("expected expression"), "{msg}");
        assert!(msg.contains("found keyword `FROM`"), "{msg}");
        // Positional info: the byte offsets of the offending span.
        assert!(msg.contains("7..11"), "{msg}");
    }

    #[test]
    fn found_end_of_input_renders_readably() {
        let err = ParseError::new(Span::new(12, 12), "`)`", Found::EndOfInput);
        assert!(err.to_string().contains("found end of input"), "{err}");
    }

    #[test]
    fn synthetic_span_does_not_leak_the_sentinel() {
        let err = ParseError::new(Span::SYNTHETIC, "expression", "`,`");
        let msg = err.to_string();

        assert!(msg.contains("unknown position"), "{msg}");
        assert!(!msg.contains(&u32::MAX.to_string()), "{msg}");
    }

    #[test]
    fn parse_error_is_a_std_error() {
        let err = ParseError::new(Span::new(0, 1), "expression", "`,`");
        // Exercise the `std::error::Error` impl through a trait object.
        let boxed: Box<dyn std::error::Error> = Box::new(err.clone());

        assert_eq!(boxed.to_string(), err.to_string());
    }

    #[test]
    fn hint_seam_round_trips_and_defaults_empty() {
        let plain = ParseError::new(Span::new(0, 1), "expression", "`,`");
        assert_eq!(
            plain.hint(),
            None,
            "constructors leave the hint channel empty"
        );

        let hinted = plain.with_hint("did you mean `SELECT`?");
        assert_eq!(hinted.hint(), Some("did you mean `SELECT`?"));
        // The public fields survive the builder untouched.
        assert_eq!(hinted.kind, ParseErrorKind::Syntax);
        assert_eq!(hinted.span, Span::new(0, 1));
    }

    #[test]
    fn lexical_constructor_carries_the_lex_kind_and_stays_readable() {
        let err = ParseError::lexical(Span::new(3, 8), LexErrorKind::UnterminatedString);

        assert_eq!(
            err.kind,
            ParseErrorKind::Lexical(LexErrorKind::UnterminatedString)
        );
        // The widened error still renders through the expected/found template.
        let msg = err.to_string();
        assert!(msg.contains("expected a well-formed SQL token"), "{msg}");
        assert!(msg.contains("unterminated string literal"), "{msg}");
        assert!(msg.contains("3..8"), "{msg}");
    }

    #[test]
    fn fail_fast_sink_captures_the_first_error() {
        let mut sink = FailFastSink::new();
        assert!(sink.first.is_none());

        let first = ParseError::new(Span::new(0, 3), "`SELECT`", "`slect`");
        let second = ParseError::new(Span::new(10, 14), "`)`", Found::EndOfInput);
        sink.report(first.clone());
        sink.report(second);

        // The second report must not displace the first.
        assert_eq!(sink.first, Some(first));
    }

    #[test]
    fn error_sink_is_implemented_for_mutable_references() {
        // A function generic over `impl ErrorSink` must accept a borrowed sink:
        // this is the property that keeps call sites stable across sink swaps.
        fn report_once(mut sink: impl ErrorSink, error: ParseError) {
            sink.report(error);
        }

        let mut sink = FailFastSink::new();
        report_once(&mut sink, ParseError::new(Span::new(0, 1), "x", "y"));

        assert!(sink.first.is_some());
    }
}
