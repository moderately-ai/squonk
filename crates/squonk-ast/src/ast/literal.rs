// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Literal AST nodes: the `Literal` value, its `LiteralKind`s, and value-parse errors.

use super::ty::{IntervalFields, TimeZone};
use crate::dialect::StringLiteralSyntax;
use crate::vocab::{Meta, Span};
use std::borrow::Cow;
use std::fmt;
use std::iter::Peekable;
use std::str::Chars;

/// A literal token tagged by kind.
///
/// The AST stores no owned literal value. Consumers materialise the value later
/// from `source[meta.span]`, preserving exact source spelling.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Literal {
    /// The literal's semantic category; see [`LiteralKind`].
    pub kind: LiteralKind,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The semantic category of a literal token.
///
/// The temporal variants (`Date`/`Time`/`Timestamp`/`Interval`) hold only a type
/// tag, never a parsed date/time value: the source spelling round-trips from
/// `meta.span` and the value is recovered on demand via
/// [`as_temporal_text`](Literal::as_temporal_text), so the AST keeps no date/time
/// dependency. The tag carries exactly the type information a downstream
/// converter cannot recover from the value text alone — the time-zone flag and the
/// interval qualifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LiteralKind {
    /// An integer literal (`42`).
    Integer,
    /// A binary floating-point literal (`3.14`, `1e9`).
    Float,
    /// A non-integer numeric literal a consumer asked to classify as `DECIMAL` rather
    /// than [`Float`](Self::Float), via the parser's `parse_float_as_decimal` option
    /// (sqlparser-rs / BigQuery-planner parity: a planner that distinguishes exact
    /// `DECIMAL`/`NUMERIC` from binary floating point wants the decision recorded at
    /// parse time). Off by default, so the default classifier never produces this tag —
    /// a fractional or scientific literal is [`Float`](Self::Float). The distinction is
    /// pure metadata: the spelling round-trips from `meta.span` either way, so rendering
    /// is identical, and the value materialises through
    /// [`as_decimal_text`](Literal::as_decimal_text) exactly as a `Float` does.
    Decimal,
    /// A string literal (`'abc'`).
    String,
    /// A boolean literal; the flag is the `TRUE`/`FALSE` value.
    Boolean(bool),
    /// The `NULL` literal.
    Null,
    /// A `DATE '…'` typed literal.
    Date,
    /// A `TIME '…'` typed literal.
    Time {
        /// The `WITH`/`WITHOUT TIME ZONE` qualifier; see [`TimeZone`].
        time_zone: TimeZone,
    },
    /// A `TIMESTAMP '…'` typed literal.
    Timestamp {
        /// The `WITH`/`WITHOUT TIME ZONE` qualifier; see [`TimeZone`].
        time_zone: TimeZone,
    },
    /// An `INTERVAL '...' [<qualifier>]` typed literal, e.g. `INTERVAL '90' DAY` or
    /// `INTERVAL '1-2' YEAR TO MONTH`.
    Interval {
        /// The interval qualifier (`DAY`, `YEAR TO MONTH`, …); `None` for a bare
        /// `INTERVAL '...'` with no trailing field list.
        fields: Option<IntervalFields>,
        /// The interval precision, whether written as a leading `INTERVAL(p) '...'`
        /// or on the trailing field (`SECOND(p)`); the two spellings are mutually
        /// exclusive (PostgreSQL).
        precision: Option<u32>,
    },
    /// A `B'1010'` / `X'1FF'` bit-string constant (SQL `bit` type). The radix is the
    /// one thing the value digits cannot reveal — `1010` is a valid body under either
    /// radix and the two spell different bit lengths — so it rides the tag; the digits
    /// round-trip from `meta.span` and materialise via [`as_bit_text`](Literal::as_bit_text).
    BitString {
        /// The bit-string radix (binary `B'…'` or hex `X'…'`); see [`BitStringRadix`].
        radix: BitStringRadix,
    },
    /// A `$1234.56` T-SQL money literal. Distinct from [`Integer`](Self::Integer) /
    /// [`Float`](Self::Float) because the `$` currency sigil denotes the
    /// `money`/`smallmoney` type — the surface tag a downstream converter cannot
    /// recover from the bare digits. The sigil rides the span; the numeric
    /// body round-trips from `meta.span` and materialises (sigil stripped) via
    /// [`as_money_text`](Literal::as_money_text).
    Money,
}

/// The radix a [`LiteralKind::BitString`] constant is written in.
///
/// PostgreSQL spells `bit` constants two ways — binary `B'1010'` and hexadecimal
/// `X'1FF'` — and they are not interchangeable: each hex digit is four bits, so
/// `X'5'` is `B'0101'`, not `B'101'`. The radix is kept as a surface tag
/// so the canonical bit-string shape stays single.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum BitStringRadix {
    /// Binary digits, `B'1010'`.
    Binary,
    /// Hexadecimal digits, `X'1FF'`; each digit is four bits.
    Hex,
}

impl BitStringRadix {
    /// Whether `byte` is a valid digit under this radix.
    const fn is_digit(self, byte: u8) -> bool {
        match self {
            Self::Binary => matches!(byte, b'0' | b'1'),
            Self::Hex => byte.is_ascii_hexdigit(),
        }
    }
}

/// The value family a literal accessor expected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LiteralExpected {
    /// An `i64` integer value was expected.
    Integer,
    /// Decimal/float value text was expected.
    DecimalText,
    /// A string value was expected.
    String,
    /// A boolean value was expected.
    Boolean,
    /// A `NULL` was expected.
    Null,
    /// A date/time/timestamp/interval value was expected.
    Temporal,
    /// A bit-string value was expected.
    BitString,
    /// A money value was expected.
    Money,
}

/// Stable category for failed lazy literal materialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiteralValueErrorKind {
    /// The accessor was called on a literal of the wrong semantic kind.
    WrongKind {
        /// The value family the accessor expected; see [`LiteralExpected`].
        expected: LiteralExpected,
        /// The literal's actual kind; see [`LiteralKind`].
        actual: LiteralKind,
    },
    /// The literal has no backing source text, usually because it was
    /// synthesized by a rewrite or detached from its parsed root.
    MissingSource,
    /// The literal's span does not slice the supplied source.
    InvalidSourceRange,
    /// The source text is not materializable as an `i64`.
    InvalidInteger,
    /// The source text is not a supported string literal.
    InvalidString,
    /// The source text is not a well-formed bit-string constant — the body holds a
    /// digit outside the literal's radix (e.g. `X'1FG'` or `B'012'`). PostgreSQL
    /// defers this check the same way (it parses, then rejects on use).
    InvalidBitString,
    /// The source text is not a well-formed money literal — it lacks the leading `$`
    /// currency sigil a [`Money`](LiteralKind::Money) literal carries.
    InvalidMoney,
}

/// Error returned by lazy literal materialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiteralValueError {
    kind: LiteralValueErrorKind,
    span: Span,
}

impl LiteralValueError {
    /// Create a literal materialization error at `span`.
    pub const fn new(kind: LiteralValueErrorKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// The coarse failure category.
    pub const fn kind(&self) -> &LiteralValueErrorKind {
        &self.kind
    }

    /// The literal source span, or [`Span::SYNTHETIC`] for detached literals.
    pub const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for LiteralValueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            LiteralValueErrorKind::WrongKind { expected, actual } => {
                write!(
                    f,
                    "literal kind mismatch: expected {expected}, found {actual}"
                )
            }
            LiteralValueErrorKind::MissingSource => {
                f.write_str("literal has no backing source text")
            }
            LiteralValueErrorKind::InvalidSourceRange => {
                f.write_str("literal span does not slice the supplied source")
            }
            LiteralValueErrorKind::InvalidInteger => {
                f.write_str("literal source text is not a valid i64")
            }
            LiteralValueErrorKind::InvalidString => {
                f.write_str("literal source text is not a supported string literal")
            }
            LiteralValueErrorKind::InvalidBitString => {
                f.write_str("literal source text is not a valid bit-string constant")
            }
            LiteralValueErrorKind::InvalidMoney => {
                f.write_str("literal source text is not a valid money literal")
            }
        }
    }
}

impl std::error::Error for LiteralValueError {}

impl fmt::Display for LiteralExpected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Integer => "integer",
            Self::DecimalText => "decimal text",
            Self::String => "string",
            Self::Boolean => "boolean",
            Self::Null => "null",
            Self::Temporal => "temporal",
            Self::BitString => "bit string",
            Self::Money => "money",
        })
    }
}

impl fmt::Display for LiteralKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer => f.write_str("integer"),
            Self::Float => f.write_str("float"),
            Self::Decimal => f.write_str("decimal"),
            Self::String => f.write_str("string"),
            Self::Boolean(true) => f.write_str("true"),
            Self::Boolean(false) => f.write_str("false"),
            Self::Null => f.write_str("null"),
            Self::Date => f.write_str("date"),
            Self::Time { .. } => f.write_str("time"),
            Self::Timestamp { .. } => f.write_str("timestamp"),
            Self::Interval { .. } => f.write_str("interval"),
            Self::BitString { radix } => match radix {
                BitStringRadix::Binary => f.write_str("bit string (binary)"),
                BitStringRadix::Hex => f.write_str("bit string (hex)"),
            },
            Self::Money => f.write_str("money"),
        }
    }
}

impl Literal {
    /// Return the exact source spelling covered by this literal's span.
    ///
    /// # Errors
    ///
    /// Returns [`MissingSource`](LiteralValueErrorKind::MissingSource) if the span is
    /// synthetic (a detached or rewrite-synthesized literal with no backing source), or
    /// [`InvalidSourceRange`](LiteralValueErrorKind::InvalidSourceRange) if the span does
    /// not slice `source` — out of bounds, or not on a UTF-8 char boundary.
    pub fn source_text<'source>(
        &self,
        source: &'source str,
    ) -> Result<&'source str, LiteralValueError> {
        if self.meta.span.is_synthetic() {
            return Err(self.error(LiteralValueErrorKind::MissingSource));
        }
        source
            .get(self.meta.span.start() as usize..self.meta.span.end() as usize)
            .ok_or_else(|| self.error(LiteralValueErrorKind::InvalidSourceRange))
    }

    /// Materialize this integer literal as an `i64`.
    ///
    /// Decodes the dialect integer forms the tokenizer folds into one `Number`
    /// token: a `0x`/`0o`/`0b` radix prefix is read in base 16/8/2, and `_`
    /// digit-group separators (`1_500_000`) are stripped before conversion. Separator
    /// placement is enforced here — the lexer leaves it permissive (it ends a token at a
    /// `_` with no following digit) and defers the strict "a separator sits between two
    /// digits" rule to materialisation.
    ///
    /// # Errors
    ///
    /// Returns [`WrongKind`](LiteralValueErrorKind::WrongKind) if this literal is not an
    /// [`Integer`](LiteralKind::Integer);
    /// [`MissingSource`](LiteralValueErrorKind::MissingSource) or
    /// [`InvalidSourceRange`](LiteralValueErrorKind::InvalidSourceRange) if its span has no
    /// backing text in `source` (see [`source_text`](Self::source_text)); or
    /// [`InvalidInteger`](LiteralValueErrorKind::InvalidInteger) if the text is not a valid
    /// `i64` — a misplaced `_` separator (leading `_1`, trailing `1_`, doubled `1__2`,
    /// prefix-adjacent `0x_1`) or a value that overflows `i64`.
    pub fn as_i64(&self, source: &str) -> Result<i64, LiteralValueError> {
        self.expect_kind(LiteralExpected::Integer, |kind| {
            matches!(kind, LiteralKind::Integer)
        })?;
        materialize_i64(self.source_text(source)?)
            .map_err(|()| self.error(LiteralValueErrorKind::InvalidInteger))
    }

    /// Return caller-owned arbitrary-precision numeric text a consumer can feed into
    /// their numeric library of choice without this AST crate taking a decimal
    /// dependency.
    ///
    /// A plain decimal or scientific literal is borrowed verbatim. The dialect forms the
    /// tokenizer folds into a `Number` token are materialised: `_` digit-group
    /// separators are stripped (`1_500_000` -> `1500000`), and a `0x`/`0o`/`0b` radix
    /// integer is normalised to its base-10 value (`0xFF` -> `255`) because a radix
    /// spelling is not decimal text.
    ///
    /// # Errors
    ///
    /// Returns [`WrongKind`](LiteralValueErrorKind::WrongKind) if this literal is not an
    /// [`Integer`](LiteralKind::Integer), [`Float`](LiteralKind::Float), or
    /// [`Decimal`](LiteralKind::Decimal);
    /// [`MissingSource`](LiteralValueErrorKind::MissingSource) or
    /// [`InvalidSourceRange`](LiteralValueErrorKind::InvalidSourceRange) if its span has no
    /// backing text in `source` (see [`source_text`](Self::source_text)); or
    /// [`InvalidInteger`](LiteralValueErrorKind::InvalidInteger) if a `_` digit separator is
    /// misplaced — the same placement rule [`as_i64`](Self::as_i64) enforces.
    pub fn as_decimal_text<'source>(
        &self,
        source: &'source str,
    ) -> Result<Cow<'source, str>, LiteralValueError> {
        self.expect_kind(LiteralExpected::DecimalText, |kind| {
            matches!(
                kind,
                LiteralKind::Integer | LiteralKind::Float | LiteralKind::Decimal
            )
        })?;
        materialize_decimal_text(self.source_text(source)?)
            .map_err(|()| self.error(LiteralValueErrorKind::InvalidInteger))
    }

    /// Materialize a string literal value under standard (ANSI) string rules.
    ///
    /// Standard strings collapse a doubled delimiter quote — `''` in a `'...'` string
    /// and `""` in a `"..."` string (the latter recognised whenever the literal was
    /// lexed as a double-quoted string, e.g. MySQL without `ANSI_QUOTES`). PostgreSQL
    /// escape strings (`E'...'`) also apply C-style backslash escapes; dollar-quoted
    /// strings borrow the body verbatim because that form has no escaping.
    ///
    /// This applies **no** dialect backslash escapes inside a `'...'`/`"..."` body: a
    /// `\n` there stays the two characters `\` `n`, the ANSI/PostgreSQL reading. For a
    /// dialect that honours C-style backslash escapes in string bodies (MySQL without
    /// `NO_BACKSLASH_ESCAPES`) use [`as_str_in`](Self::as_str_in) with that dialect's
    /// [`StringLiteralSyntax`], which additionally decodes `\n`, `\t`, `\\`, `\'`, `\"`,
    /// … in the value.
    ///
    /// A literal's span may cover several quoted segments joined by SQL-standard
    /// adjacent-string concatenation — `'foo'`⏎`'bar'` is one value `foobar`.
    /// Each segment is materialized under its own form and the results
    /// concatenated; see the internal `materialize_concatenated_string`.
    ///
    /// # Errors
    ///
    /// As [`as_str_in`](Self::as_str_in).
    pub fn as_str<'source>(
        &self,
        source: &'source str,
    ) -> Result<Cow<'source, str>, LiteralValueError> {
        self.as_str_in(source, StringLiteralSyntax::ANSI)
    }

    /// Materialize a string literal value under a dialect's [`StringLiteralSyntax`].
    ///
    /// Identical to [`as_str`](Self::as_str) except that when `syntax.backslash_escapes`
    /// is set (the MySQL default) a `'...'`/`"..."` body additionally has its C-style
    /// backslash escapes decoded: `\n`/`\t`/`\r`/`\b`/`\Z`/`\0` become the control byte,
    /// `\\`/`\'`/`\"` the literal character, `\%`/`\_` keep their backslash (MySQL `LIKE`
    /// pattern escapes, literal outside a pattern), and any other `\x` drops the
    /// backslash and keeps `x`.
    ///
    /// Only `backslash_escapes` is consulted here: each string *form* is recognised from
    /// its own source prefix (the token was already lexed under the dialect that
    /// produced it), so `E'...'`, `U&'...'`, `$tag$...$tag$`, and `""` double-quote
    /// un-doubling are handled by spelling regardless of the other `syntax` flags.
    /// `backslash_escapes` is the one value-affecting bit the source spelling alone cannot
    /// disambiguate — a plain `'a\nb'` is a different value in MySQL and PostgreSQL.
    ///
    /// # Errors
    ///
    /// Returns [`WrongKind`](LiteralValueErrorKind::WrongKind) if this literal is not a
    /// [`String`](LiteralKind::String);
    /// [`MissingSource`](LiteralValueErrorKind::MissingSource) or
    /// [`InvalidSourceRange`](LiteralValueErrorKind::InvalidSourceRange) if its span has no
    /// backing text in `source` (see [`source_text`](Self::source_text)); or
    /// [`InvalidString`](LiteralValueErrorKind::InvalidString) if the text is not a
    /// well-formed string constant — an unterminated or unrecognised body, an embedded raw
    /// NUL, a malformed `E'…'` escape, or a malformed `U&'…'`/`UESCAPE` clause.
    pub fn as_str_in<'source>(
        &self,
        source: &'source str,
        syntax: StringLiteralSyntax,
    ) -> Result<Cow<'source, str>, LiteralValueError> {
        self.expect_kind(LiteralExpected::String, |kind| {
            matches!(kind, LiteralKind::String)
        })?;
        let text = self.source_text(source)?;
        materialize_concatenated_string(text, syntax.backslash_escapes)
            .map_err(|()| self.error(LiteralValueErrorKind::InvalidString))
    }

    /// The MySQL character-set introducer name on a string literal, borrowed from
    /// source: `_utf8mb4'x'` -> `Some("utf8mb4")`, and `None` for a string with no
    /// introducer (plain `'x'`, `E'x'`, `N'x'`, …).
    ///
    /// The introducer is the charset MySQL interprets the literal's bytes under — a
    /// surface tag that rides the span and is recovered here on demand,
    /// exactly as the value body is via [`as_str`](Self::as_str) (which
    /// returns the body with the introducer stripped).
    ///
    /// # Errors
    ///
    /// Returns [`WrongKind`](LiteralValueErrorKind::WrongKind) if this literal is not a
    /// [`String`](LiteralKind::String), or
    /// [`MissingSource`](LiteralValueErrorKind::MissingSource) /
    /// [`InvalidSourceRange`](LiteralValueErrorKind::InvalidSourceRange) if its span has no
    /// backing text in `source` (see [`source_text`](Self::source_text)). A `String` with
    /// no introducer is `Ok(None)`, not an error.
    pub fn charset_introducer<'source>(
        &self,
        source: &'source str,
    ) -> Result<Option<&'source str>, LiteralValueError> {
        self.expect_kind(LiteralExpected::String, |kind| {
            matches!(kind, LiteralKind::String)
        })?;
        let text = self.source_text(source)?;
        Ok(charset_introducer_quote_offset(text).map(|open| &text[1..open]))
    }

    /// Materialize the value string of a temporal literal
    /// (`DATE`/`TIME`/`TIMESTAMP`/`INTERVAL`).
    ///
    /// Temporal literals are stored as their exact source text plus a type tag, with
    /// no date/time dependency in the AST. This returns the inner string
    /// constant — the part a downstream date/time library parses — with standard and
    /// PostgreSQL string escaping already applied, exactly like [`as_str`](Self::as_str).
    /// The literal's [`kind`](Literal::kind) carries the type, time-zone flag, and
    /// interval qualifier the returned value must be interpreted under.
    ///
    /// The value string may itself be an adjacent-string concatenation
    /// (`DATE '1998'`⏎`'-12-01'`), which is materialized into the one concatenated value
    /// just as [`as_str`](Self::as_str) does.
    ///
    /// # Errors
    ///
    /// Returns [`WrongKind`](LiteralValueErrorKind::WrongKind) if this literal is not a
    /// temporal kind ([`Date`](LiteralKind::Date), [`Time`](LiteralKind::Time),
    /// [`Timestamp`](LiteralKind::Timestamp), or [`Interval`](LiteralKind::Interval));
    /// [`MissingSource`](LiteralValueErrorKind::MissingSource) or
    /// [`InvalidSourceRange`](LiteralValueErrorKind::InvalidSourceRange) if its span has no
    /// backing text in `source` (see [`source_text`](Self::source_text)); or
    /// [`InvalidString`](LiteralValueErrorKind::InvalidString) if the embedded value string
    /// is absent or malformed.
    pub fn as_temporal_text<'source>(
        &self,
        source: &'source str,
    ) -> Result<Cow<'source, str>, LiteralValueError> {
        self.expect_kind(LiteralExpected::Temporal, |kind| {
            matches!(
                kind,
                LiteralKind::Date
                    | LiteralKind::Time { .. }
                    | LiteralKind::Timestamp { .. }
                    | LiteralKind::Interval { .. }
            )
        })?;
        let token = temporal_string_token(self.source_text(source)?)
            .ok_or_else(|| self.error(LiteralValueErrorKind::InvalidString))?;
        // Temporal value strings are ANSI/PostgreSQL constants (`DATE '...'`, `E'...'`),
        // never MySQL backslash-escape strings, so materialize under standard rules.
        materialize_concatenated_string(token, false)
            .map_err(|()| self.error(LiteralValueErrorKind::InvalidString))
    }

    /// Materialise the digit body of a bit-string constant (`B'1010'` / `X'1FF'`).
    ///
    /// Returns the digits between the quotes, borrowed from source, with the `B`/`X`
    /// prefix and the quotes stripped. The radix is read from the literal's
    /// [`kind`](Literal::kind); the digits are validated against it, so a malformed
    /// body (`X'1FG'`, `B'012'`) reports [`InvalidBitString`](LiteralValueErrorKind::InvalidBitString)
    /// — the deferred check PostgreSQL also leaves until the value is used.
    ///
    /// A constant whose span covers several newline-joined segments (`B'1010'`⏎`'0101'`,
    /// SQL-standard adjacent-string concatenation) yields the concatenated digits — an
    /// owned string then, borrowed for the common single-segment case.
    ///
    /// # Errors
    ///
    /// Returns [`WrongKind`](LiteralValueErrorKind::WrongKind) if this literal is not a
    /// [`BitString`](LiteralKind::BitString);
    /// [`MissingSource`](LiteralValueErrorKind::MissingSource) or
    /// [`InvalidSourceRange`](LiteralValueErrorKind::InvalidSourceRange) if its span has no
    /// backing text in `source` (see [`source_text`](Self::source_text)); or
    /// [`InvalidBitString`](LiteralValueErrorKind::InvalidBitString) if the body holds a
    /// digit outside the literal's radix (`X'1FG'`, `B'012'`) or is not a well-formed
    /// `B'…'`/`X'…'` constant.
    pub fn as_bit_text<'source>(
        &self,
        source: &'source str,
    ) -> Result<Cow<'source, str>, LiteralValueError> {
        let radix = match self.kind {
            LiteralKind::BitString { radix } => radix,
            _ => return Err(self.wrong_kind(LiteralExpected::BitString)),
        };
        let body = concatenated_bit_body(self.source_text(source)?)
            .ok_or_else(|| self.error(LiteralValueErrorKind::InvalidBitString))?;
        if !body.bytes().all(|byte| radix.is_digit(byte)) {
            return Err(self.error(LiteralValueErrorKind::InvalidBitString));
        }
        Ok(body)
    }

    /// Materialise the numeric body of a money literal (`$1234.56` -> `1234.56`),
    /// borrowed from source with the leading `$` currency sigil stripped.
    ///
    /// The body is the exact numeric spelling the lexer captured after the `$`, ready
    /// for the consumer's decimal library — this AST keeps no decimal
    /// dependency, exactly like [`as_decimal_text`](Self::as_decimal_text) for plain
    /// numbers.
    ///
    /// # Errors
    ///
    /// Returns [`WrongKind`](LiteralValueErrorKind::WrongKind) if this literal is not a
    /// [`Money`](LiteralKind::Money) literal;
    /// [`MissingSource`](LiteralValueErrorKind::MissingSource) or
    /// [`InvalidSourceRange`](LiteralValueErrorKind::InvalidSourceRange) if its span has no
    /// backing text in `source` (see [`source_text`](Self::source_text)); or
    /// [`InvalidMoney`](LiteralValueErrorKind::InvalidMoney) if the text lacks the leading
    /// `$` currency sigil.
    pub fn as_money_text<'source>(
        &self,
        source: &'source str,
    ) -> Result<Cow<'source, str>, LiteralValueError> {
        self.expect_kind(LiteralExpected::Money, |kind| {
            matches!(kind, LiteralKind::Money)
        })?;
        let body = money_body(self.source_text(source)?)
            .ok_or_else(|| self.error(LiteralValueErrorKind::InvalidMoney))?;
        Ok(Cow::Borrowed(body))
    }

    /// Return the boolean value for a boolean literal.
    ///
    /// # Errors
    ///
    /// Returns [`WrongKind`](LiteralValueErrorKind::WrongKind) if this literal is not a
    /// [`Boolean`](LiteralKind::Boolean). This accessor reads the value from the
    /// [`kind`](Literal::kind) tag and takes no `source`, so it has no
    /// source-materialisation failure mode.
    pub fn as_bool(&self) -> Result<bool, LiteralValueError> {
        match self.kind {
            LiteralKind::Boolean(value) => Ok(value),
            _ => Err(self.wrong_kind(LiteralExpected::Boolean)),
        }
    }

    /// Validate that this literal is `NULL`.
    ///
    /// # Errors
    ///
    /// Returns [`WrongKind`](LiteralValueErrorKind::WrongKind) if this literal is not
    /// [`Null`](LiteralKind::Null). Like [`as_bool`](Self::as_bool) it inspects only the
    /// [`kind`](Literal::kind) tag and takes no `source`, so `WrongKind` is its only
    /// failure mode.
    pub fn as_null(&self) -> Result<(), LiteralValueError> {
        self.expect_kind(LiteralExpected::Null, |kind| {
            matches!(kind, LiteralKind::Null)
        })
    }

    /// Return true if this literal is `NULL`.
    pub fn is_null(&self) -> bool {
        matches!(self.kind, LiteralKind::Null)
    }

    fn expect_kind(
        &self,
        expected: LiteralExpected,
        matches: impl FnOnce(&LiteralKind) -> bool,
    ) -> Result<(), LiteralValueError> {
        if matches(&self.kind) {
            Ok(())
        } else {
            Err(self.wrong_kind(expected))
        }
    }

    fn wrong_kind(&self, expected: LiteralExpected) -> LiteralValueError {
        self.error(LiteralValueErrorKind::WrongKind {
            expected,
            actual: self.kind.clone(),
        })
    }

    fn error(&self, kind: LiteralValueErrorKind) -> LiteralValueError {
        LiteralValueError::new(kind, self.meta.span)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StringLiteralBody<'source> {
    Standard(&'source str),
    PostgresEscape(&'source str),
    DollarQuoted(&'source str),
    /// A `U&'...'` Unicode-escape body plus the active escape character (default
    /// `\`, or the `UESCAPE 'c'` override). The body still carries `''` doubled
    /// quotes; the escape sequences are unfolded by [`materialize_unicode_string`].
    Unicode {
        body: &'source str,
        escape: char,
    },
}

/// Materialise a (possibly adjacent-concatenated) string literal.
///
/// A string literal's span may cover several quoted segments joined by SQL-standard
/// adjacent-string concatenation — `'foo'`⏎`'bar'` is the one value `foobar`.
/// The parser joins segments only the way PostgreSQL's scanner does
/// (across whitespace containing a newline, every continuation an unprefixed
/// `'...'`/`"..."` character string), and
/// the tokenizer validated each segment as its own token; so each segment is
/// materialised under its own form ([`materialize_string`]) and the results
/// concatenated, which keeps the lazily materialised value in agreement with that
/// eager per-segment validation.
fn materialize_concatenated_string(text: &str, backslash: bool) -> Result<Cow<'_, str>, ()> {
    // `first_segment_end` locates the first segment's closing quote. What follows it,
    // after any whitespace, decides the shape: a `'…'` (or a `"…"` where the dialect lexes
    // double quotes as strings — MySQL folds `'a' "b"` into `'ab'`) opening quote is an
    // adjacent-string continuation (joined across a newline gap — standard — or any
    // whitespace gap under MySQL's `same_line_adjacent_concat`), so the segments are
    // materialised and joined; anything else means a lone literal whose span merely carries
    // a non-continuation tail (a `U&'…' UESCAPE '…'` clause), so it is materialised whole. A
    // newline check cannot stand in for "single segment" (same-line concat has none), and a
    // bare "first_end < len" check would misread the `UESCAPE` tail as a continuation.
    let Some(first_end) = first_segment_end(text, backslash) else {
        // A non-continuable form (a `$tag$…$tag$` dollar-quoted constant): always a lone
        // literal, never an adjacent concatenation. Materialise it whole.
        return materialize_string(text, backslash);
    };
    let tail = text[first_end..].trim_start();
    if !(tail.starts_with('\'') || tail.starts_with('"')) {
        return materialize_string(text, backslash);
    }
    let mut out = String::new();
    let mut rest = text;
    loop {
        let end = first_segment_end(rest, backslash).ok_or(())?;
        out.push_str(&materialize_string(&rest[..end], backslash)?);
        // The parser only joined whitespace gaps, so trimming reaches the next
        // segment's opening quote (or the end after the last segment).
        rest = rest[end..].trim_start();
        if rest.is_empty() {
            break;
        }
    }
    Ok(Cow::Owned(out))
}

/// The byte index just past the first string segment's closing quote in `text`, or
/// `None` if `text` does not begin with a recognised quoted string token.
///
/// Walks the segments of an adjacent-string concatenation (a single literal whose
/// span covers several newline-joined quoted constants): the leading
/// segment carries the form prefix (`E'`/`N'`/`U&'`/`B'`/`X'`/`_charset'`/plain `'` or
/// `"`), every later segment is an unprefixed `'...'`/`"..."` continuation. The terminator is located
/// under the opening quote (`'` or `"`), treating a doubled quote as embedded. Backslash
/// escaping (a `\'`/`\"` does not terminate) is honoured for an `E'`-string always and
/// for every segment when `dialect_backslash` is set (MySQL without
/// `NO_BACKSLASH_ESCAPES`) — matching how the tokenizer found the same terminator; the
/// other forms treat `\` as an ordinary byte. A dollar-quoted token is not a continuable
/// segment, so it is `None` here (the parser never joins one).
fn first_segment_end(text: &str, dialect_backslash: bool) -> Option<usize> {
    let bytes = text.as_bytes();
    let (open, prefix_backslash) = match bytes {
        [b'E' | b'e', b'\'', ..] => (1, true),
        [b'N' | b'n', b'\'', ..] => (1, false),
        [b'U' | b'u', b'&', b'\'', ..] => (2, false),
        [b'B' | b'b' | b'X' | b'x', b'\'', ..] => (1, false),
        [b'\'', ..] | [b'"', ..] => (0, false),
        // `_charset'…'` / `_charset"…"`: the introducer length is the charset name, so
        // the opening quote is measured rather than a fixed offset.
        [b'_', ..] => (charset_introducer_quote_offset(text)?, false),
        _ => return None,
    };
    let quote = bytes[open]; // the opening delimiter (`'` or `"`)
    let backslash = prefix_backslash || dialect_backslash;
    let mut i = open + 1; // first body byte, past the opening quote
    while i < bytes.len() {
        match bytes[i] {
            // An escaped byte never terminates (`\'`/`\"` is an embedded quote). The
            // delimiter is ASCII, so a `\`-escaped multi-byte char cannot hide one.
            b'\\' if backslash => i += 2,
            b if b == quote => {
                if bytes.get(i + 1) == Some(&quote) {
                    i += 2; // a doubled quote is an embedded quote, not the terminator
                } else {
                    return Some(i + 1); // the terminating quote
                }
            }
            _ => i += 1,
        }
    }
    None
}

fn materialize_string(text: &str, backslash: bool) -> Result<Cow<'_, str>, ()> {
    // PG parity: a raw NUL byte in a string literal's source is rejected in every
    // form (see [`string_literal_embeds_nul`]). The tokenizer rejects it up front, so
    // a parsed literal never reaches here carrying one; running the same check keeps a
    // detached or hand-built literal in agreement with that eager verdict (ADR-0006).
    if string_literal_embeds_nul(text) {
        return Err(());
    }
    match string_literal_body(text).ok_or(())? {
        StringLiteralBody::Standard(body) => {
            // The closing delimiter is the segment's last byte (`'` or `"`); it is the
            // quote that doubles to embed itself in a standard string. Only a standard
            // segment reaches this arm, and every such segment ends in its delimiter.
            let quote = text.as_bytes().last().copied().unwrap_or(b'\'');
            materialize_standard_string(body, quote, backslash)
        }
        StringLiteralBody::PostgresEscape(body) => materialize_postgres_escape_string(body),
        StringLiteralBody::DollarQuoted(body) => Ok(Cow::Borrowed(body)),
        StringLiteralBody::Unicode { body, escape } => materialize_unicode_string(body, escape),
    }
}

fn string_literal_body(text: &str) -> Option<StringLiteralBody<'_>> {
    if let Some(body) = text
        .strip_prefix('\'')
        .and_then(|text| text.strip_suffix('\''))
    {
        return Some(StringLiteralBody::Standard(body));
    }

    // `"..."` double-quoted string (MySQL without `ANSI_QUOTES`): its `""` doubling is
    // collapsed like `'...'`'s `''`. Recognised unconditionally — a `"..."` reaches here
    // as a `String` literal only when the tokenizer lexed it as one (its
    // `double_quoted_strings` gate); under `ANSI_QUOTES` a `"..."` is a quoted identifier,
    // interned rather than materialised through this path, so it never asks here.
    if let Some(body) = text
        .strip_prefix('"')
        .and_then(|text| text.strip_suffix('"'))
    {
        return Some(StringLiteralBody::Standard(body));
    }

    // Reuse the length-guarded `postgres_escape_body`: a truncated `E'` (a detached or
    // source-mismatched 2-byte literal) returns `None` here rather than slicing past its
    // own opening quote and panicking.
    if let Some(body) = postgres_escape_body(text) {
        return Some(StringLiteralBody::PostgresEscape(body));
    }

    // `N'...'` national strings are plain string constants in PostgreSQL (the `N`
    // is sugar), so strip the prefix and materialise the body like any standard string.
    // The `text.len() >= 3` guard keeps a truncated `N'` from slicing past its own quote,
    // the same reason `postgres_escape_body` carries it for `E'`.
    if matches!(text.as_bytes(), [b'N' | b'n', b'\'', ..])
        && text.len() >= 3
        && text.ends_with('\'')
    {
        return Some(StringLiteralBody::Standard(&text[2..text.len() - 1]));
    }

    // `_charset'...'` / `_charset"..."` MySQL charset introducer: the charset name is a
    // surface tag (like the `N` national prefix), so strip the `_charset` introducer and
    // materialise the `'...'`/`"..."` body as a standard string (the double-quoted body
    // un-doubles `""` just as the single-quoted one un-doubles `''`). The introducer rule
    // mirrors the tokenizer's so the eager lex and this lazy body agree (ADR-0006); the
    // charset *name* is separately recovered from the span by `charset_introducer`.
    if let Some(open) = charset_introducer_quote_offset(text) {
        let rest = &text[open..];
        let body = rest
            .strip_prefix('\'')
            .and_then(|body| body.strip_suffix('\''))
            .or_else(|| {
                rest.strip_prefix('"')
                    .and_then(|body| body.strip_suffix('"'))
            });
        if let Some(body) = body {
            return Some(StringLiteralBody::Standard(body));
        }
    }

    if let Some(unicode) = unicode_string_body(text) {
        return Some(unicode);
    }

    dollar_quoted_body(text).map(StringLiteralBody::DollarQuoted)
}

/// Split a `U&'...'` literal's source text into its escape body and active escape
/// character, recognising an optional trailing `UESCAPE 'c'` clause. The clause is
/// part of the literal's span (the parser extends it) so the value is fully
/// recoverable here without any parser-stored state.
fn unicode_string_body(text: &str) -> Option<StringLiteralBody<'_>> {
    let (body, escape) = unicode_lexeme_body(text, b'\'')?;
    Some(StringLiteralBody::Unicode { body, escape })
}

/// Split a `U&`-prefixed unicode-escape lexeme into its escape body and active escape
/// character. `quote` is the delimiter of the escaped lexeme — `'` for a `U&'...'`
/// string constant, `"` for a `U&"..."` delimited identifier — the one axis on which
/// the two surfaces of the single `U&` escape facility differ (SQL:2016 introduces both
/// off one `<Unicode escape prefix>`). An optional trailing `UESCAPE 'c'` clause folded
/// into the span overrides the default `\` escape; the clause's own quoting is always
/// `'…'`, independent of `quote`.
fn unicode_lexeme_body(text: &str, quote: u8) -> Option<(&str, char)> {
    let rest = text
        .strip_prefix("U&")
        .or_else(|| text.strip_prefix("u&"))?;
    let bytes = rest.as_bytes();
    if bytes.first() != Some(&quote) {
        return None;
    }
    let close = terminating_quote(bytes, quote)?;
    let body = &rest[1..close];
    let tail = rest.get(close + 1..)?.trim_start();
    let escape = if tail.is_empty() {
        '\\'
    } else {
        uescape_char(tail)?
    };
    Some((body, escape))
}

/// Index of the single `quote` that terminates a `quote`-delimited body, treating a
/// doubled delimiter as an embedded copy rather than the terminator. `bytes[0]` is the
/// opener.
fn terminating_quote(bytes: &[u8], quote: u8) -> Option<usize> {
    let mut index = 1;
    while index < bytes.len() {
        if bytes[index] == quote {
            if bytes.get(index + 1) == Some(&quote) {
                index += 2;
                continue;
            }
            return Some(index);
        }
        index += 1;
    }
    None
}

/// Parse a trailing `UESCAPE 'c'` clause into its single escape character, applying
/// PostgreSQL's restriction that the character is not a hex digit, `+`, a quote, or
/// whitespace. `None` (an `InvalidString` at the accessor) for any other shape.
fn uescape_char(tail: &str) -> Option<char> {
    tail.get(..7)
        .filter(|kw| kw.eq_ignore_ascii_case("UESCAPE"))?;
    let arg = tail[7..].trim_start();
    parse_uescape_argument(arg)
}

/// The single escape character of a `UESCAPE '<c>'` argument string `arg` — the `'+'` in
/// `U&'…' UESCAPE '+'` — applying PostgreSQL's `check_uescapechar` restriction (not a hex
/// digit, `+`, a single/double quote, or whitespace). `None` for a multi-character or
/// illegal delimiter, or a non-string argument. Shared by the lazy accessor
/// ([`uescape_char`]) and the parser's eager parse-time gate ([`uescape_argument_is_legal`]),
/// so the two can never disagree.
fn parse_uescape_argument(arg: &str) -> Option<char> {
    let inner = arg.strip_prefix('\'')?.strip_suffix('\'')?;
    // A `UESCAPE 'c'` argument is a plain single-quoted ANSI string (no backslash escapes).
    let value = materialize_standard_string(inner, b'\'', false).ok()?;
    let mut chars = value.chars();
    let escape = chars.next()?;
    if chars.next().is_some() || !is_legal_uescape_char(escape) {
        return None;
    }
    Some(escape)
}

/// Whether `arg` — the source spelling of a `UESCAPE '<c>'` argument's string token — is
/// PostgreSQL's legal single escape character. The parser calls this the moment it consumes
/// the clause so an illegal delimiter (`UESCAPE '+'`, a multi-character `UESCAPE '!!'`, a hex
/// digit, a quote, or whitespace) is rejected at parse time (`invalid Unicode escape
/// character`) rather than silently accepted and deferred to the value accessor.
pub fn uescape_argument_is_legal(arg: &str) -> bool {
    parse_uescape_argument(arg).is_some()
}

/// PostgreSQL's legal `UESCAPE` characters: anything but a hex digit, `+`, a single
/// or double quote, or whitespace.
fn is_legal_uescape_char(ch: char) -> bool {
    !(ch.is_ascii_hexdigit() || ch == '+' || ch == '\'' || ch == '"' || ch.is_whitespace())
}

/// Slice a bit-string constant's digit body out of its source text: strip the
/// leading `B`/`X` radix marker and the surrounding quotes. Radix validation is the
/// caller's (it holds the [`BitStringRadix`] tag).
fn bit_string_body(text: &str) -> Option<&str> {
    let rest = match text.as_bytes() {
        [b'B' | b'b' | b'X' | b'x', b'\'', ..] => &text[1..],
        _ => return None,
    };
    rest.strip_prefix('\'')?.strip_suffix('\'')
}

/// The digit body of a (possibly adjacent-concatenated) bit-string constant.
///
/// A single `B'1010'` / `X'1FF'` borrows its body verbatim; a newline-joined
/// constant (`B'1010'`⏎`'0101'`, SQL-standard adjacent-string concatenation)
/// concatenates each segment's digits — the leading segment carries the
/// `B`/`X` marker, every continuation is a plain `'...'`. Radix and digit validation
/// stay with the caller.
fn concatenated_bit_body(text: &str) -> Option<Cow<'_, str>> {
    // Bit-string bodies treat `\` as an ordinary byte (backslashes are inert), so the
    // segment terminator is located without backslash escaping. Reaching the span end on
    // the first segment means a lone constant — borrow its body (the common case);
    // otherwise the parser joined continuations (newline or, under MySQL, same-line) and
    // each segment is appended below.
    let first_end = first_segment_end(text, false)?;
    if first_end == text.len() {
        return bit_string_body(text).map(Cow::Borrowed);
    }
    // The leading segment carries the `B`/`X` marker; each continuation is a plain
    // `'...'` whose digit body is the inter-quote text.
    let mut out = String::from(bit_string_body(&text[..first_end])?);
    let mut rest = text[first_end..].trim_start();
    while !rest.is_empty() {
        let end = first_segment_end(rest, false)?;
        out.push_str(plain_quoted_inner(&rest[..end])?);
        rest = rest[end..].trim_start();
    }
    Some(Cow::Owned(out))
}

/// The text between the quotes of a plain `'...'` constant (no prefix). Used for the
/// continuation segments of a concatenated bit-string constant, whose digit bodies
/// carry no doubled `''` quotes.
fn plain_quoted_inner(segment: &str) -> Option<&str> {
    segment.strip_prefix('\'')?.strip_suffix('\'')
}

/// Strip the leading `$` currency sigil from a money literal's source text, leaving
/// the numeric body (`$1234.56` -> `1234.56`). `None` when the `$` is absent, which
/// the accessor reports as [`InvalidMoney`](LiteralValueErrorKind::InvalidMoney).
fn money_body(text: &str) -> Option<&str> {
    text.strip_prefix('$')
}

// --- numeric literal materialisation ---------------------------------------
//
// The tokenizer folds the dialect numeric forms — `0x`/`0o`/`0b` radix integers and `_`
// digit-group separators — into one `Number` token but defers value materialisation and
// strict separator placement to here (ADR-0006). These helpers do that decoding for
// `as_i64` and `as_decimal_text`. They are deliberately free functions, not a `Radix`
// enum: every struct and enum in this crate feeds the generated render-shape fingerprint
// (ADR-0013), and a pure value-materialisation helper must not perturb the AST's render
// shape — the same reason `Utf8Tail` is a bare tuple.

/// Split an optional folded sign off a numeric literal's text. A `+`/`-` rides the span
/// only in SET-value position (PostgreSQL's `NumericOnly`); an
/// expression-position sign is a separate operator token. Returned apart from the body
/// so the magnitude can be decoded under its radix and the sign reapplied.
fn split_sign(text: &str) -> (&str, &str) {
    match text.as_bytes().first() {
        Some(b'+' | b'-') => text.split_at(1),
        _ => ("", text),
    }
}

/// Split a `0x`/`0o`/`0b` radix prefix off an unsigned numeric body: the base it
/// introduces (16/8/2) and the body past the two-byte prefix, or base 10 and the body
/// unchanged when there is no prefix. The marker is matched case-insensitively.
///
/// The single `&str`-domain radix-prefix recognizer, shared so classification
/// and materialization cannot drift: the AST value materializers decode the base through
/// it ([`Literal::as_i64`](Literal::as_i64),
/// [`Literal::as_decimal_text`](Literal::as_decimal_text)), and the parser's
/// number-literal classifier reads `base != 10` to tag a radix integer apart from a
/// decimal float. The tokenizer keeps a deliberately separate recognizer in the
/// streaming-cursor domain (it gates lexing on a byte `Cursor`, not a `&str`), which is
/// not consolidated here.
pub fn split_radix_prefix(unsigned: &str) -> (u32, &str) {
    match unsigned.as_bytes() {
        [b'0', b'x' | b'X', ..] => (16, &unsigned[2..]),
        [b'0', b'o' | b'O', ..] => (8, &unsigned[2..]),
        [b'0', b'b' | b'B', ..] => (2, &unsigned[2..]),
        _ => (10, unsigned),
    }
}

/// Whether `byte` is a digit in `base` (2/8/10/16) — the set a `_` separator must sit
/// between. Mirrors the tokenizer's per-radix digit classes.
fn is_radix_digit(byte: u8, base: u32) -> bool {
    match base {
        16 => byte.is_ascii_hexdigit(),
        8 => matches!(byte, b'0'..=b'7'),
        2 => matches!(byte, b'0' | b'1'),
        _ => byte.is_ascii_digit(),
    }
}

/// Validate `_` digit-group separator placement and return `body` with the separators
/// removed (borrowed when it carries none).
///
/// The tokenizer accepts a `_` only when a digit follows it and otherwise ends the token
/// there, leaving the strict placement rule to materialisation. This is that
/// rule: every separator must sit directly between two `base` digits, so a leading
/// (`_1`), trailing (`1_`), doubled (`1__2`), radix-prefix-adjacent (`0x_1`), or
/// decimal-point-adjacent (`1_.5`) `_` is rejected — the numeric grammar of the dialects
/// that accept the separator (PostgreSQL 14+).
fn strip_digit_separators(body: &str, base: u32) -> Result<Cow<'_, str>, ()> {
    let bytes = body.as_bytes();
    if !bytes.contains(&b'_') {
        return Ok(Cow::Borrowed(body));
    }
    for (index, &byte) in bytes.iter().enumerate() {
        if byte != b'_' {
            continue;
        }
        let after_digit = index
            .checked_sub(1)
            .is_some_and(|prev| is_radix_digit(bytes[prev], base));
        let before_digit = bytes
            .get(index + 1)
            .is_some_and(|&next| is_radix_digit(next, base));
        if !(after_digit && before_digit) {
            return Err(());
        }
    }
    Ok(Cow::Owned(body.chars().filter(|&ch| ch != '_').collect()))
}

/// Materialise an integer literal's text as an `i64`, decoding a `0x`/`0o`/`0b` radix and
/// stripping validated `_` separators.
fn materialize_i64(text: &str) -> Result<i64, ()> {
    let (sign, unsigned) = split_sign(text);
    let (base, body) = split_radix_prefix(unsigned);
    let digits = strip_digit_separators(body, base)?;
    if base == 10 {
        // No prefix was stripped, so a folded sign still leads the (separator-free)
        // digits; parse the spelling whole so i64::MIN — whose magnitude overflows i64 —
        // still decodes.
        return match digits {
            Cow::Borrowed(_) => text.parse::<i64>(),
            Cow::Owned(stripped) => format!("{sign}{stripped}").parse::<i64>(),
        }
        .map_err(|_| ());
    }
    let value = i64::from_str_radix(&digits, base).map_err(|_| ())?;
    if sign == "-" {
        value.checked_neg().ok_or(())
    } else {
        Ok(value)
    }
}

/// Materialise a numeric literal's text as decimal text. Decimal/scientific text passes
/// through (separators stripped, borrowed when none), and a `0x`/`0o`/`0b` radix integer
/// is normalised to its base-10 value through `i128`, which covers every realistic radix
/// literal (a binary form is at most 127 significant digits).
fn materialize_decimal_text(text: &str) -> Result<Cow<'_, str>, ()> {
    let (sign, unsigned) = split_sign(text);
    let (base, body) = split_radix_prefix(unsigned);
    let digits = strip_digit_separators(body, base)?;
    if base == 10 {
        // Decimal/scientific text already parses in a numeric library; with no prefix
        // stripped, the original spelling stands as-is (borrowed, sign included) —
        // a digit separator in the text is the only thing that forces the owned,
        // re-joined copy below.
        return Ok(match digits {
            Cow::Borrowed(_) => Cow::Borrowed(text),
            Cow::Owned(stripped) => Cow::Owned(format!("{sign}{stripped}")),
        });
    }
    let magnitude = i128::from_str_radix(&digits, base).map_err(|_| ())?;
    let value = if sign == "-" { -magnitude } else { magnitude };
    Ok(Cow::Owned(value.to_string()))
}

/// The byte length of a dollar-quote opening delimiter (`$$` or `$tag$`) at the start of
/// `bytes` — the offset just past its closing `$` — or `None` when `bytes` does not open
/// with a well-formed `$`-delimiter. The delimiter grammar (a bare `$$`, or a `$tag$`
/// whose tag is an identifier run) lives here alone so the body slicer
/// ([`dollar_quoted_body`]) and the token-length scanner ([`dollar_quoted_token_len`])
/// cannot drift on it, the same single-recognizer discipline the radix and
/// charset scanners keep.
fn dollar_open_delim_len(bytes: &[u8]) -> Option<usize> {
    if bytes.first() != Some(&b'$') {
        return None;
    }
    let mut index = 1;
    match bytes.get(index).copied()? {
        b'$' => {}
        byte if is_dollar_tag_start(byte) => {
            index += 1;
            while bytes
                .get(index)
                .is_some_and(|&byte| is_dollar_tag_continue(byte))
            {
                index += 1;
            }
            if bytes.get(index) != Some(&b'$') {
                return None;
            }
        }
        _ => return None,
    }
    Some(index + 1)
}

fn dollar_quoted_body(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let delim_len = dollar_open_delim_len(bytes)?;
    if text.len() < delim_len * 2 {
        return None;
    }
    let close_start = text.len() - delim_len;
    if bytes[..delim_len] != bytes[close_start..] {
        return None;
    }
    Some(&text[delim_len..close_start])
}

fn is_dollar_tag_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic() || byte >= 0x80
}

fn is_dollar_tag_continue(byte: u8) -> bool {
    is_dollar_tag_start(byte) || byte.is_ascii_digit()
}

/// The byte offset of the opening quote in a MySQL charset-introduced string's source
/// text — i.e. the length of the `_charset` introducer (`_utf8mb4'x'` -> `8`,
/// `_latin1"x"` -> `7`). `None` when `text` does not open with a `_<name>'`/`_<name>"`
/// introducer.
///
/// The introducer is `_` then a non-empty ASCII identifier run (the charset name), then
/// the quote opening the string — the exact shape the tokenizer's
/// `charset_introducer_prefix` recognises, so the eager lex and this lazy split never
/// disagree. The quote is `'` or `"`: a `"` reaches here only when the
/// tokenizer already lexed `_name"…"` as one string (its `double_quoted_strings` gate,
/// MySQL `ANSI_QUOTES` off), so recognising it unconditionally is safe — under
/// `ANSI_QUOTES` on, `_name"…"` is never a single string literal and this is never
/// asked about it. Charset names are ASCII (`utf8mb4`, `latin1`, …); a non-ASCII byte
/// ends the run, matching the tokenizer, so the `_…` would there lex as an ordinary
/// identifier rather than an introducer.
fn charset_introducer_quote_offset(text: &str) -> Option<usize> {
    let name_len = text
        .strip_prefix('_')?
        .bytes()
        .take_while(|&byte| is_charset_name_byte(byte))
        .count();
    if name_len == 0 {
        return None;
    }
    let open = 1 + name_len; // the `_` plus the charset name
    matches!(text.as_bytes().get(open), Some(b'\'' | b'"')).then_some(open)
}

/// Whether `byte` may appear in a charset-introducer name: the ASCII
/// identifier-continue set (`[A-Za-z0-9_]`). This hardcodes the standard rule the
/// tokenizer applies via its byte-class table for the dialect that enables charset
/// introducers (MySQL), keeping the two introducer scanners in agreement,
/// just as `is_dollar_tag_start` mirrors the tokenizer's dollar-tag rule.
fn is_charset_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Slice the embedded value-string token out of a temporal literal's source text.
///
/// A temporal literal is a type prefix, a string constant, and (for intervals) a
/// trailing field qualifier; only the value is quoted or dollar-quoted, so the
/// prefix and trailer contain neither `'` nor `$`. The returned slice is the whole
/// string token (`'...'`, `E'...'`, or `$tag$...$tag$`) ready for [`materialize_string`].
fn temporal_string_token(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let first_quote = bytes.iter().position(|&byte| byte == b'\'');
    let first_dollar = bytes.iter().position(|&byte| byte == b'$');
    let dollar_first = match (first_quote, first_dollar) {
        (Some(quote), Some(dollar)) => dollar < quote,
        (None, Some(_)) => true,
        _ => false,
    };
    if dollar_first {
        let start = first_dollar?;
        let len = dollar_quoted_token_len(&bytes[start..])?;
        return text.get(start..start + len);
    }

    let quote = first_quote?;
    // Include a leading escape-string `E`/`e` marker when it stands alone — i.e. it
    // is not the final letter of a preceding keyword word (`... ZONE 'x'`), so that
    // `materialize_string` applies PostgreSQL escapes to an `E'...'` value.
    let start = if quote >= 1
        && matches!(bytes[quote - 1], b'E' | b'e')
        && (quote < 2 || !is_dollar_tag_continue(bytes[quote - 2]))
    {
        quote - 1
    } else {
        quote
    };
    let end = bytes.iter().rposition(|&byte| byte == b'\'')?;
    text.get(start..=end)
}

/// The byte length of a dollar-quoted string token starting at `bytes[0] == b'$'`,
/// or `None` if `bytes` does not open a complete dollar-quoted string.
fn dollar_quoted_token_len(bytes: &[u8]) -> Option<usize> {
    let delim_len = dollar_open_delim_len(bytes)?;
    let delim = &bytes[..delim_len];
    let mut close = delim_len;
    while close + delim_len <= bytes.len() {
        if &bytes[close..close + delim_len] == delim {
            return Some(close + delim_len);
        }
        close += 1;
    }
    None
}

/// Materialise a standard-quoted string body: collapse the doubled delimiter quote
/// (`''` in a `'...'` string, `""` in a `"..."` string) and, when `backslash` is set (a
/// MySQL-family dialect without `NO_BACKSLASH_ESCAPES`), decode C-style backslash escapes
/// in the body.
///
/// `quote` is the delimiter byte (`'` or `"`). A lone undoubled delimiter is rejected,
/// mirroring the tokenizer, which would have closed the string there. With `backslash`
/// off this is the ANSI/PostgreSQL reading — `\` is an ordinary byte — so a single-quoted
/// body decodes byte-for-byte as it did before double-quote/backslash support.
fn materialize_standard_string(body: &str, quote: u8, backslash: bool) -> Result<Cow<'_, str>, ()> {
    let bytes = body.as_bytes();
    let has_backslash = backslash && bytes.contains(&b'\\');
    if !bytes.contains(&quote) && !has_backslash {
        return Ok(Cow::Borrowed(body));
    }

    let mut out = String::with_capacity(body.len());
    let mut segment_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if backslash => {
                out.push_str(&body[segment_start..i]);
                i += 1;
                consume_mysql_escape(body, &mut i, &mut out);
                segment_start = i;
            }
            b if b == quote => {
                if bytes.get(i + 1) != Some(&quote) {
                    return Err(());
                }
                out.push_str(&body[segment_start..i]);
                out.push(quote as char); // `quote` is the ASCII `'` or `"`
                i += 2;
                segment_start = i;
            }
            _ => i += 1,
        }
    }
    out.push_str(&body[segment_start..]);
    Ok(Cow::Owned(out))
}

/// Decode one MySQL C-style backslash escape. `index` points just past the `\`; on
/// return it points past the consumed escape.
///
/// MySQL's escapes never fail: `\0`/`\b`/`\n`/`\r`/`\t`/`\Z` decode to their control
/// bytes, `\'`/`\"`/`\\` to the literal character, and `\%`/`\_` keep the backslash —
/// they are `LIKE` pattern escapes whose value outside a pattern is the two characters
/// `\%` / `\_` (MySQL). Every other `\x` drops the backslash and keeps the (possibly
/// multi-byte) character. A dangling `\` at the end of the body is kept verbatim.
fn consume_mysql_escape(body: &str, index: &mut usize, out: &mut String) {
    let Some(byte) = body.as_bytes().get(*index).copied() else {
        out.push('\\'); // dangling backslash at end of body
        return;
    };
    match byte {
        b'0' => out.push('\0'),
        b'\'' => out.push('\''),
        b'"' => out.push('"'),
        b'b' => out.push('\u{08}'),
        b'n' => out.push('\n'),
        b'r' => out.push('\r'),
        b't' => out.push('\t'),
        b'Z' => out.push('\u{1a}'),
        b'\\' => out.push('\\'),
        b'%' => out.push_str("\\%"),
        b'_' => out.push_str("\\_"),
        _ => {
            let ch = body[*index..]
                .chars()
                .next()
                .expect("index is within the body");
            out.push(ch);
            *index += ch.len_utf8();
            return;
        }
    }
    *index += 1;
}

fn materialize_postgres_escape_string(body: &str) -> Result<Cow<'_, str>, ()> {
    let bytes = body.as_bytes();
    if !bytes.contains(&b'\\') && !bytes.contains(&b'\'') {
        return Ok(Cow::Borrowed(body));
    }

    let mut out = Vec::with_capacity(body.len());
    scan_postgres_escape_string(body, &mut out)?;
    // The scan already rejected a NUL and a short/out-of-range Unicode escape; the
    // final UTF-8 check catches byte escapes (`\xNN`, octal) that do not assemble
    // into valid UTF-8 (`\xff`, a lone `\xc3`). `from_utf8` is the same RFC-3629
    // rule the no-allocation parse-time check applies incrementally via `Utf8Tail`.
    String::from_utf8(out).map(Cow::Owned).map_err(|_| ())
}

/// Whether a PostgreSQL escape-string literal (`E'...'`) carries only escapes the
/// real parser accepts — checked without materialising the value.
///
/// PostgreSQL rejects a malformed escape while *parsing*: a short or out-of-range
/// Unicode escape (`\u12`, `\uD800`, `\U00110000`), an escape that yields a NUL
/// (`\0`, `\x00`, `\U00000000`), or a byte escape that breaks UTF-8 (`\xff`, a lone
/// `\xc3`). An *unknown* escape such as `\q`, or a bare `\x` with no hex digit, is
/// kept as the literal following character and accepted — the boundary the real
/// libpg_query parser draws.
///
/// This is the eager, parse-time half of [`Literal::as_str`]'s lazy
/// materialisation: both run the one escape walk (`scan_postgres_escape_string`), so
/// the parse-time verdict and the lazily materialised value can never disagree on
/// what counts as a valid escape. The scan allocates nothing —
/// it never builds the unescaped string — so rejecting at parse time keeps the
/// no-eager-materialisation contract intact. A bool keeps the AST crate's public
/// surface (and its generated render-shape fingerprint) free of a new
/// error type for what is a pure yes/no check.
///
/// `text` is the full literal spelling (`E'...'`); a literal that is not an escape
/// string carries no C-style escapes and is vacuously valid.
pub fn postgres_escape_string_is_valid(text: &str) -> bool {
    let Some(body) = postgres_escape_body(text) else {
        return true;
    };
    let bytes = body.as_bytes();
    // Fast path mirrors `materialize_postgres_escape_string`: a body with no escape
    // and no quote decodes to itself and is already valid UTF-8 (it is a slice of
    // the source), so there is nothing to reject.
    if !bytes.contains(&b'\\') && !bytes.contains(&b'\'') {
        return true;
    }
    let mut tail = UTF8_TAIL_START;
    // `Ok` means every escape decoded and the bytes stayed valid UTF-8; the final
    // `tail == START` rejects a trailing partial sequence (a lone `\xc3`).
    scan_postgres_escape_string(body, &mut tail).is_ok() && tail == UTF8_TAIL_START
}

/// Whether a string literal's source text embeds a raw NUL byte (`0x00`).
///
/// PostgreSQL cannot represent a NUL inside any value — a query reaches the server
/// as a NUL-terminated C string, so libpg_query rejects a literal `0x00` byte in the
/// source of *any* string form (ordinary `'…'`, `E'…'`, `N'…'`, `U&'…'`, bit-string,
/// dollar-quoted) while parsing (confirmed against the oracle). This is the broader
/// companion to [`postgres_escape_string_is_valid`], which rejects an *escape* that
/// decodes to NUL (`\0`, `\x00`); here the NUL is a raw byte in the body, independent
/// of escape decoding, so the check is a plain byte scan rather than an escape walk.
///
/// The tokenizer calls this to reject up front, and [`Literal::as_str`]'s
/// materialisation runs the identical check (via `materialize_string`), so the
/// eager lexer verdict and the lazily materialised value can never
/// disagree. The scan allocates nothing — a single `memchr` over the
/// source bytes — keeping the no-eager-materialisation contract intact.
///
/// `text` is the full literal spelling; the delimiters and prefixes of every form are
/// NUL-free ASCII, so scanning the whole token is equivalent to scanning its body.
pub fn string_literal_embeds_nul(text: &str) -> bool {
    text.as_bytes().contains(&0)
}

/// Walk a PostgreSQL escape-string body once, decoding each `\`-escape and doubled
/// `''` quote and handing the resulting bytes to `out`.
///
/// This is the single definition of the escape grammar — which sequences are
/// recognised, how many hex/octal digits each consumes, that a NUL or a
/// short/invalid Unicode escape is rejected, and that an unknown escape collapses to
/// its literal character. The value accessor drives it with a [`Vec<u8>`] sink to
/// build the string; the parse-time check drives it with a [`Utf8Tail`] sink that
/// keeps nothing. Sharing one walk is what keeps the eager and lazy paths from
/// drifting into two divergent escape grammars.
fn scan_postgres_escape_string<S: EscapeSink>(body: &str, out: &mut S) -> Result<(), ()> {
    let bytes = body.as_bytes();
    let mut segment_start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' => {
                if bytes.get(i + 1) != Some(&b'\'') {
                    return Err(());
                }
                out.utf8_run(&bytes[segment_start..i])?;
                push_escape_byte(out, b'\'')?;
                i += 2;
                segment_start = i;
            }
            b'\\' => {
                out.utf8_run(&bytes[segment_start..i])?;
                i += 1;
                consume_postgres_escape(body, &mut i, out)?;
                segment_start = i;
            }
            _ => i += 1,
        }
    }
    out.utf8_run(&bytes[segment_start..])
}

/// Sink for the bytes a PostgreSQL escape string decodes to, so one escape walk
/// ([`scan_postgres_escape_string`]) serves both callers: the value accessor
/// collects the bytes into a buffer, while the parse-time validator feeds them
/// through an incremental UTF-8 check and stores nothing.
trait EscapeSink {
    /// Append a run of bytes that are valid UTF-8 on their own — a verbatim body
    /// segment, a decoded scalar's encoding, or a doubled-quote byte.
    fn utf8_run(&mut self, bytes: &[u8]) -> Result<(), ()>;
    /// Append one raw decoded byte (`\xNN` / octal), which may be a UTF-8 fragment.
    fn raw_byte(&mut self, byte: u8) -> Result<(), ()>;
}

impl EscapeSink for Vec<u8> {
    fn utf8_run(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.extend_from_slice(bytes);
        Ok(())
    }

    fn raw_byte(&mut self, byte: u8) -> Result<(), ()> {
        self.push(byte);
        Ok(())
    }
}

/// Incremental UTF-8 validator state, carried as a bare `(remaining, lo, hi)` tuple
/// rather than a named struct on purpose: the generated render-shape
/// fingerprint hashes every struct and enum in this crate, and a pure parse-time
/// validation helper must not perturb the AST's render shape.
///
/// - `remaining`: continuation bytes still expected for the in-flight sequence
///   (`0` at a character boundary).
/// - `lo` / `hi`: the inclusive range the next continuation byte must fall in. A
///   sequence's leading byte narrows it for the first continuation byte to reject
///   overlong encodings, surrogates, and code points above U+10FFFF (Unicode
///   Table 3-7); it is `0x80..=0xBF` for every continuation byte after the first.
///
/// The check accepts the decoded bytes one at a time, failing the moment the stream
/// cannot extend to valid UTF-8, storing nothing beyond these three bytes — so the
/// parse-time check honours the contract (validate, never materialise) and agrees with
/// the `String::from_utf8` the value accessor applies.
type Utf8Tail = (u8, u8, u8);

/// The character-boundary state: nothing pending. A completed scan must end here, or
/// a trailing partial sequence (a lone `\xc3`) slipped through.
const UTF8_TAIL_START: Utf8Tail = (0, 0x80, 0xBF);

/// Feed one decoded byte to the UTF-8 validator carried in `tail`.
fn utf8_tail_push(tail: &mut Utf8Tail, byte: u8) -> Result<(), ()> {
    let (remaining, lo, hi) = *tail;
    if remaining > 0 {
        if !(lo..=hi).contains(&byte) {
            return Err(());
        }
        *tail = (remaining - 1, 0x80, 0xBF);
        return Ok(());
    }
    *tail = match byte {
        0x00..=0x7F => (0, 0x80, 0xBF), // ASCII; no continuation
        0xC2..=0xDF => (1, 0x80, 0xBF), // 2-byte sequence
        0xE0 => (2, 0xA0, 0xBF),        // 3-byte; reject overlong
        0xED => (2, 0x80, 0x9F),        // 3-byte; reject surrogates
        0xE1..=0xEC | 0xEE..=0xEF => (2, 0x80, 0xBF), // 3-byte
        0xF0 => (3, 0x90, 0xBF),        // 4-byte; reject overlong
        0xF4 => (3, 0x80, 0x8F),        // 4-byte; reject > U+10FFFF
        0xF1..=0xF3 => (3, 0x80, 0xBF), // 4-byte
        // A continuation byte at a boundary, a `0xC0`/`0xC1` overlong lead, or a
        // `0xF5..` out-of-range lead can never start valid UTF-8.
        _ => return Err(()),
    };
    Ok(())
}

impl EscapeSink for Utf8Tail {
    fn utf8_run(&mut self, bytes: &[u8]) -> Result<(), ()> {
        bytes
            .iter()
            .try_for_each(|&byte| utf8_tail_push(self, byte))
    }

    fn raw_byte(&mut self, byte: u8) -> Result<(), ()> {
        utf8_tail_push(self, byte)
    }
}

/// The escape-string body between the `E'`/`e'` prefix and the closing `'`, or
/// `None` when `text` is not an escape string. Shared by [`string_literal_body`]
/// (the lazy accessor) and [`postgres_escape_string_is_valid`] (the parse-time
/// check) so both slice the identical body. The length guard keeps a truncated `E'`
/// from slicing past its own opening quote.
fn postgres_escape_body(text: &str) -> Option<&str> {
    (matches!(text.as_bytes(), [b'E' | b'e', b'\'', ..]) && text.len() >= 3 && text.ends_with('\''))
        .then(|| &text[2..text.len() - 1])
}

fn consume_postgres_escape<S: EscapeSink>(
    body: &str,
    index: &mut usize,
    out: &mut S,
) -> Result<(), ()> {
    let bytes = body.as_bytes();
    let Some(byte) = bytes.get(*index).copied() else {
        return Err(());
    };

    match byte {
        b'b' => {
            push_escape_byte(out, 0x08)?;
            *index += 1;
        }
        b'f' => {
            push_escape_byte(out, 0x0c)?;
            *index += 1;
        }
        b'n' => {
            push_escape_byte(out, b'\n')?;
            *index += 1;
        }
        b'r' => {
            push_escape_byte(out, b'\r')?;
            *index += 1;
        }
        b't' => {
            push_escape_byte(out, b'\t')?;
            *index += 1;
        }
        b'0'..=b'7' => consume_octal_escape(bytes, index, out)?,
        b'x' => consume_hex_escape(body, index, out)?,
        b'u' => consume_unicode_escape(bytes, index, out, 4)?,
        b'U' => consume_unicode_escape(bytes, index, out, 8)?,
        _ => consume_escaped_char(body, index, out)?,
    }

    Ok(())
}

fn consume_octal_escape<S: EscapeSink>(
    bytes: &[u8],
    index: &mut usize,
    out: &mut S,
) -> Result<(), ()> {
    let mut value = 0_u32;
    let mut count = 0;
    while count < 3 {
        let Some(byte @ b'0'..=b'7') = bytes.get(*index).copied() else {
            break;
        };
        value = value * 8 + u32::from(byte - b'0');
        *index += 1;
        count += 1;
    }
    push_escape_byte(out, value as u8)
}

fn consume_hex_escape<S: EscapeSink>(body: &str, index: &mut usize, out: &mut S) -> Result<(), ()> {
    let bytes = body.as_bytes();
    let start = *index + 1;
    let Some(first) = bytes.get(start).and_then(|byte| hex_value(*byte)) else {
        return consume_escaped_char(body, index, out);
    };

    let mut value = first;
    *index = start + 1;
    if let Some(second) = bytes.get(*index).and_then(|byte| hex_value(*byte)) {
        value = value * 16 + second;
        *index += 1;
    }
    push_escape_byte(out, value as u8)
}

fn consume_unicode_escape<S: EscapeSink>(
    bytes: &[u8],
    index: &mut usize,
    out: &mut S,
    width: usize,
) -> Result<(), ()> {
    let mut value = 0_u32;
    let start = *index + 1;
    let end = start.checked_add(width).ok_or(())?;
    for byte in bytes.get(start..end).ok_or(())? {
        let digit = hex_value(*byte).ok_or(())?;
        value = value * 16 + digit;
    }
    let ch = char::from_u32(value).ok_or(())?;
    if ch == '\0' {
        return Err(());
    }
    let mut buf = [0; 4];
    out.utf8_run(ch.encode_utf8(&mut buf).as_bytes())?;
    *index = end;
    Ok(())
}

fn consume_escaped_char<S: EscapeSink>(
    body: &str,
    index: &mut usize,
    out: &mut S,
) -> Result<(), ()> {
    let ch = body[*index..].chars().next().ok_or(())?;
    if ch == '\0' {
        return Err(());
    }
    let mut buf = [0; 4];
    out.utf8_run(ch.encode_utf8(&mut buf).as_bytes())?;
    *index += ch.len_utf8();
    Ok(())
}

/// Push one decoded escape byte, rejecting a NUL. PostgreSQL forbids an escape
/// that decodes to NUL (`\0`, `\x00`, or a NUL-valued `\u`/`\U` escape) because a
/// value cannot embed a NUL, whereas an *unknown* escape like `\q` is kept as a
/// literal character -- so the NUL check is on the decoded byte, not the letter.
fn push_escape_byte<S: EscapeSink>(out: &mut S, byte: u8) -> Result<(), ()> {
    if byte == 0 {
        return Err(());
    }
    out.raw_byte(byte)
}

fn hex_value(byte: u8) -> Option<u32> {
    match byte {
        b'0'..=b'9' => Some(u32::from(byte - b'0')),
        b'a'..=b'f' => Some(u32::from(byte - b'a' + 10)),
        b'A'..=b'F' => Some(u32::from(byte - b'A' + 10)),
        _ => None,
    }
}

/// Unfold a `U&'...'` body: `<esc>XXXX` and `<esc>+XXXXXX` Unicode escapes (with
/// surrogate pairs for code points above U+FFFF), `<esc><esc>` for a literal escape
/// character, and `''` for an embedded quote. The accept set mirrors PostgreSQL's:
/// invalid hex, a lone surrogate, `U+0000`, and a code point above `U+10FFFF` all
/// fail — which the accessor reports as `InvalidString`.
///
/// PostgreSQL rejects these at parse time, and so does the parser's eager check
/// ([`unicode_escape_string_is_valid`]): it shares this function's escape walk
/// ([`scan_unicode_escape_string`]), so the two can never disagree. This
/// materialiser stays the check of record for what the eager one cannot reach — a
/// detached or hand-built [`Literal`] with no parser behind it, and a body joined from
/// several adjacent-concatenated segments, which the eager check does not reconstruct
/// (it validates the one leading segment it can resolve, the same scope `E'...'`'s
/// eager check keeps for its own continuations).
fn materialize_unicode_string(body: &str, escape: char) -> Result<Cow<'_, str>, ()> {
    if !body.contains(escape) && !body.contains('\'') {
        return Ok(Cow::Borrowed(body));
    }
    let mut out = String::with_capacity(body.len());
    scan_unicode_escape_string(body, escape, '\'', &mut out)?;
    Ok(Cow::Owned(out))
}

/// Decode a `U&"..."` delimited identifier's source text (any `UESCAPE 'c'` clause
/// folded into the span) to its identifier value — the double-quoted-identifier twin of
/// `materialize_unicode_string`. A doubled `""` collapses to one `"`, and
/// `<esc>XXXX` / `<esc>+XXXXXX` escapes decode against the resolved escape character.
///
/// `None` for a malformed escape (invalid hex, a lone surrogate, `U+0000`, a code point
/// above `U+10FFFF`) or an illegal `UESCAPE` delimiter — the same inputs PostgreSQL
/// rejects while parsing a `U&"..."` identifier, so the parser's eager check (which calls
/// this the moment it folds the identifier) and this value walk can never disagree. Text
/// that is not a `U&"..."` lexeme at all is likewise `None`; the caller routes only genuine
/// `U&"` identifiers here (see [`is_unicode_ident`]).
pub fn materialize_unicode_ident(text: &str) -> Option<Cow<'_, str>> {
    let (body, escape) = unicode_lexeme_body(text, b'"')?;
    if !body.contains(escape) && !body.contains('"') {
        return Some(Cow::Borrowed(body));
    }
    let mut out = String::with_capacity(body.len());
    scan_unicode_escape_string(body, escape, '"', &mut out).ok()?;
    Some(Cow::Owned(out))
}

/// Whether a delimited-identifier lexeme is a `U&"..."` Unicode-escape identifier — the
/// only quoted-identifier form that decodes escapes and takes a trailing `UESCAPE`
/// clause. The identifier twin of the parser's `is_unicode_string`; the tokenizer only
/// emits this shape where [`StringLiteralSyntax::unicode_strings`] is on, so the parser
/// routes such a token to [`materialize_unicode_ident`] rather than the plain
/// quoted-identifier path.
pub fn is_unicode_ident(text: &str) -> bool {
    matches!(text.as_bytes(), [b'U' | b'u', b'&', b'"', ..])
}

/// Whether a `U&'...'` Unicode-escape string carries only escapes the real parser
/// accepts — checked without materialising the value. The `U&'...'` twin of
/// [`postgres_escape_string_is_valid`]: both share their form's
/// one escape walk with the lazy accessor, so the eager verdict and the materialised
/// value can never disagree.
///
/// Unlike `E'...'`, a `U&'...'` body's escape character is not fixed: a trailing
/// `UESCAPE 'c'` clause can override the default `\`, and that clause is a separate
/// token the tokenizer has not seen when it scans the string body — so, unlike the
/// `E'...'` check, this one cannot run from the tokenizer. PostgreSQL's own scanner
/// carries the identical constraint and resolves it the same way: it never decodes a
/// `U&'...'` string itself, and a wrapper between its scanner and grammar looks one
/// token ahead for `UESCAPE` before decoding exactly once with whichever escape
/// character that lookahead resolves to. `text` must already be that resolution: the
/// literal's full spelling, with the `UESCAPE` clause folded in when the parser found
/// one — `unicode_string_body`, which the lazy accessor also drives from the same
/// fully-resolved span text, is what recovers the active escape character from it.
///
/// `text` this cannot resolve to a `U&'...'` body — including one joined from several
/// adjacent-concatenated segments, which this does not reconstruct — is
/// vacuously valid here, same as `E'...'`'s check on non-`E'` text: left for
/// [`Literal::as_str`] to validate lazily, unchanged from before this check existed.
pub fn unicode_escape_string_is_valid(text: &str) -> bool {
    let Some(StringLiteralBody::Unicode { body, escape }) = unicode_string_body(text) else {
        return true;
    };
    if !body.contains(escape) && !body.contains('\'') {
        return true;
    }
    scan_unicode_escape_string(body, escape, '\'', &mut ()).is_ok()
}

/// Walk a `U&…` escape body once, decoding each `<esc>XXXX` / `<esc>+XXXXXX`
/// Unicode escape (surrogate-pairing a high surrogate with an immediately following
/// low one), `<esc><esc>` for a literal escape character, and a doubled `quote` for an
/// embedded delimiter, handing each resulting character to `out`.
///
/// `quote` is the lexeme's delimiter — `'` for a `U&'...'` string body, `"` for a
/// `U&"..."` identifier body — the only difference between the two surfaces: an embedded
/// delimiter is doubled and collapses here, while the *other* quote character is an
/// ordinary body byte (a `'` inside a `U&"..."` identifier, or a `"` inside a `U&'...'`
/// string, is passed through untouched).
///
/// This is the single definition of the Unicode-escape grammar — which escapes are
/// recognised, how many hex digits each consumes, and that a NUL, a lone surrogate,
/// and a code point above U+10FFFF are all rejected — mirroring
/// [`scan_postgres_escape_string`]'s role for `E'...'`. The value accessors drive it
/// with a `String` sink that builds the value; the parse-time checks
/// ([`unicode_escape_string_is_valid`] / [`materialize_unicode_ident`]) drive it to
/// validate. Sharing one walk is what keeps the eager and lazy paths from drifting into
/// two divergent escape grammars.
fn scan_unicode_escape_string<S: UnicodeEscapeSink>(
    body: &str,
    escape: char,
    quote: char,
    out: &mut S,
) -> Result<(), ()> {
    let mut chars = body.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == quote {
            // The lexer keeps embedded delimiters doubled; collapse `''`/`""` back.
            if chars.next() != Some(quote) {
                return Err(());
            }
            out.emit(quote)?;
        } else if ch == escape {
            match chars.peek().copied() {
                Some(next) if next == escape => {
                    chars.next();
                    out.emit(escape)?;
                }
                Some('+') => {
                    chars.next();
                    let code_point = read_unicode_hex(&mut chars, 6)?;
                    push_code_point(out, code_point)?;
                }
                Some(_) => {
                    let code_point = read_unicode_hex(&mut chars, 4)?;
                    if is_high_surrogate(code_point) {
                        // A high surrogate pairs with an immediately-following
                        // `<esc>XXXX` low surrogate to encode a supplementary char.
                        if chars.next() != Some(escape) {
                            return Err(());
                        }
                        let low = read_unicode_hex(&mut chars, 4)?;
                        if !is_low_surrogate(low) {
                            return Err(());
                        }
                        let combined = 0x1_0000 + ((code_point - 0xD800) << 10) + (low - 0xDC00);
                        push_code_point(out, combined)?;
                    } else {
                        push_code_point(out, code_point)?;
                    }
                }
                None => return Err(()),
            }
        } else {
            out.emit(ch)?;
        }
    }
    Ok(())
}

/// Sink for the characters a `U&'...'` body decodes to, so one escape walk
/// ([`scan_unicode_escape_string`]) serves both callers: the value accessor collects
/// characters into the materialised `String`, while the parse-time validator counts
/// nothing and keeps only pass/fail.
trait UnicodeEscapeSink {
    fn emit(&mut self, ch: char) -> Result<(), ()>;
}

impl UnicodeEscapeSink for String {
    fn emit(&mut self, ch: char) -> Result<(), ()> {
        self.push(ch);
        Ok(())
    }
}

// The validate-only `UnicodeEscapeSink`: `()` rather than a named unit struct, on
// purpose — the generated render-shape fingerprint (ADR-0013) hashes every struct
// and enum the AST source defines (see `Utf8Tail`'s identical reasoning above for
// `E'...'`), and a pure parse-time validation helper must not perturb it.
// [`push_code_point`] and the doubled-escape/doubled-quote arms above already reject
// anything invalid before a character reaches `emit`, so there is nothing left to do
// with it here.
impl UnicodeEscapeSink for () {
    fn emit(&mut self, _ch: char) -> Result<(), ()> {
        Ok(())
    }
}

/// Read exactly `width` hexadecimal digits from `chars` into a code-point value.
fn read_unicode_hex(chars: &mut Peekable<Chars<'_>>, width: usize) -> Result<u32, ()> {
    let mut value = 0_u32;
    for _ in 0..width {
        let digit = chars.next().and_then(|ch| ch.to_digit(16)).ok_or(())?;
        value = value * 16 + digit;
    }
    Ok(value)
}

/// Push a validated Unicode code point, rejecting NUL (which PostgreSQL forbids in
/// Unicode-escape strings) and — via [`char::from_u32`] — surrogates and out-of-range
/// values.
fn push_code_point<S: UnicodeEscapeSink>(out: &mut S, code_point: u32) -> Result<(), ()> {
    if code_point == 0 {
        return Err(());
    }
    out.emit(char::from_u32(code_point).ok_or(())?)
}

fn is_high_surrogate(code_point: u32) -> bool {
    (0xD800..=0xDBFF).contains(&code_point)
}

fn is_low_surrogate(code_point: u32) -> bool {
    (0xDC00..=0xDFFF).contains(&code_point)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{RenderConfig, RenderCtx, RenderExt};
    use crate::vocab::{NodeId, Resolver, Symbol};

    #[test]
    fn integer_accessor_materializes_i64() {
        let literal = literal(LiteralKind::Integer, 7, 10);

        assert_eq!(literal.as_i64("SELECT 123"), Ok(123));
    }

    #[test]
    fn integer_accessor_reports_invalid_text() {
        let literal = literal(LiteralKind::Integer, 0, 19);

        assert_eq!(
            literal
                .as_i64("9223372036854775808")
                .expect_err("overflows i64")
                .kind(),
            &LiteralValueErrorKind::InvalidInteger,
        );
    }

    #[test]
    fn decimal_text_accessor_borrows_exact_numeric_spelling() {
        let literal = literal(LiteralKind::Float, 7, 12);
        let text = literal
            .as_decimal_text("SELECT 1.5e3")
            .expect("float text materializes");

        assert!(matches!(text, Cow::Borrowed("1.5e3")));
    }

    #[test]
    fn decimal_kind_materializes_as_decimal_text_like_float() {
        // A `Decimal` literal (the parse_float_as_decimal classification) carries the
        // same numeric spelling as a `Float` and materialises through the identical
        // accessor — the kind is metadata, not a different value form.
        let literal = literal(LiteralKind::Decimal, 7, 12);
        let text = literal
            .as_decimal_text("SELECT 1.5e3")
            .expect("decimal text materializes");

        assert!(matches!(text, Cow::Borrowed("1.5e3")));
        assert_eq!(LiteralKind::Decimal.to_string(), "decimal");
    }

    #[test]
    fn string_accessor_borrows_unescaped_body() {
        let literal = literal(LiteralKind::String, 7, 13);
        let text = literal
            .as_str("SELECT 'cafe'")
            .expect("string text materializes");

        assert!(matches!(text, Cow::Borrowed("cafe")));
    }

    #[test]
    fn string_accessor_allocates_only_for_doubled_quote_unescape() {
        let literal = literal(LiteralKind::String, 7, 14);
        let text = literal
            .as_str("SELECT 'it''s'")
            .expect("escaped string materializes");

        assert!(matches!(text, Cow::Owned(_)));
        assert_eq!(text, "it's");
    }

    #[test]
    fn string_accessor_strips_mysql_charset_introducer() {
        // `_utf8mb4'cafe'`: the value is the body, with the `_charset` introducer
        // stripped (like the `N` national prefix), borrowed from source.
        let src = "SELECT _utf8mb4'cafe'";
        let literal = literal(LiteralKind::String, 7, src.len() as u32);
        let text = literal.as_str(src).expect("charset string materializes");

        assert!(matches!(text, Cow::Borrowed("cafe")));
    }

    #[test]
    fn charset_introducer_accessor_recovers_the_name_from_the_span() {
        // The introducer name rides the span and is recovered on demand.
        let src = "SELECT _latin1'x'";
        let introduced = literal(LiteralKind::String, 7, src.len() as u32);
        assert_eq!(
            introduced.charset_introducer(src).expect("string literal"),
            Some("latin1"),
        );

        // A plain string carries no introducer.
        let plain = literal(LiteralKind::String, 7, 10);
        assert_eq!(
            plain.charset_introducer("SELECT 'x'").expect("string"),
            None
        );

        // A non-string literal is the wrong kind.
        let integer = literal(LiteralKind::Integer, 7, 8);
        assert!(integer.charset_introducer("SELECT 1").is_err());
    }

    #[test]
    fn charset_introducer_accessor_recovers_the_name_from_a_double_quoted_span() {
        // A `_charset"..."` introducer (MySQL `ANSI_QUOTES` off, so `"..."` is a string)
        // is modelled exactly like the single-quote form: the charset name rides the span
        // and is recovered on demand (ADR-0006). The tokenizer only hands this accessor a
        // `_name"..."` string when it already lexed one under `double_quoted_strings`, so
        // recognising the `"` introducer here keeps the eager lex and this lazy split in
        // agreement.
        let src = r#"SELECT _latin1"x""#;
        let introduced = literal(LiteralKind::String, 7, src.len() as u32);
        assert_eq!(
            introduced.charset_introducer(src).expect("string literal"),
            Some("latin1"),
        );

        // The *value* body of a double-quoted string materialises too: a plain `"x"`
        // yields `x`, and the `_charset"x"` introducer materialises the same body (with
        // the introducer stripped), symmetric with the single-quote forms.
        let plain_dq = literal(LiteralKind::String, 7, 10); // `"x"` in `SELECT "x"`
        assert_eq!(
            plain_dq
                .as_str(r#"SELECT "x""#)
                .expect("double-quoted value"),
            "x",
        );
        assert_eq!(
            introduced.as_str(src).expect("charset double-quoted value"),
            "x",
        );
    }

    #[test]
    fn string_accessor_materializes_double_quoted_values() {
        // A `"..."` string (MySQL without `ANSI_QUOTES`) materialises like a single-quoted
        // one: the body borrows, and a doubled `""` collapses to one `"`. This holds under
        // the ANSI-default `as_str` — double-quote un-doubling is dialect-independent (the
        // token was already lexed as a string, so `""` is its doubling escape).
        let src = r#"SELECT "abc""#;
        let plain = literal(LiteralKind::String, 7, src.len() as u32);
        let value = plain.as_str(src).expect("double-quoted value materializes");
        assert!(matches!(value, Cow::Borrowed("abc")));

        let src = r#"SELECT "a""b""#;
        let doubled = literal(LiteralKind::String, 7, src.len() as u32);
        let value = doubled.as_str(src).expect("doubled quote un-doubles");
        assert!(matches!(value, Cow::Owned(_)));
        assert_eq!(value, r#"a"b"#);
    }

    #[test]
    fn string_accessor_applies_mysql_backslash_escapes_in_both_quote_forms() {
        // Under MySQL (`backslash_escapes` on) a `'...'` and a `"..."` body both decode
        // C-style escapes, so `\n` becomes a real newline. The ANSI-default `as_str`
        // leaves the backslash literal (the PostgreSQL reading), so the same source is a
        // different value.
        for src in [r"'a\nb'", r#""a\nb""#] {
            let literal = literal(LiteralKind::String, 0, src.len() as u32);
            assert_eq!(
                literal
                    .as_str_in(src, StringLiteralSyntax::MYSQL)
                    .expect("escape materializes"),
                "a\nb",
                "escape for {src:?}",
            );
            assert_eq!(
                literal.as_str(src).expect("literal backslash"),
                r"a\nb",
                "ansi for {src:?}",
            );
        }
    }

    #[test]
    fn string_accessor_mysql_pattern_and_unknown_escapes() {
        // `\%` / `\_` keep the backslash (MySQL `LIKE` pattern escapes, literal outside a
        // pattern); an unknown escape drops the backslash; `\\`/`\'` decode normally; and
        // the doubled `''` quote still collapses alongside backslash escapes.
        for (src, value) in [
            (r"'a\%b'", r"a\%b"),
            (r"'a\_b'", r"a\_b"),
            (r"'a\qb'", "aqb"),
            (r"'a\\b'", r"a\b"),
            (r"'a\'b'", "a'b"),
            (r"'it''s'", "it's"),
        ] {
            let literal = literal(LiteralKind::String, 0, src.len() as u32);
            assert_eq!(
                literal
                    .as_str_in(src, StringLiteralSyntax::MYSQL)
                    .expect("materializes"),
                value,
                "for {src:?}",
            );
        }
    }

    #[test]
    fn string_accessor_mysql_control_escapes_decode_to_bytes() {
        // The named control escapes decode to their bytes, including `\0` -> NUL: MySQL
        // permits an embedded NUL via the *escape* (only a raw NUL byte in source is
        // rejected, by the shared PG-parity check).
        let src = r"'\0\b\t\r\Z'";
        let literal = literal(LiteralKind::String, 0, src.len() as u32);
        assert_eq!(
            literal
                .as_str_in(src, StringLiteralSyntax::MYSQL)
                .expect("control escapes"),
            "\0\u{08}\t\r\u{1a}",
        );
    }

    #[test]
    fn single_quote_doubling_path_is_unchanged_by_escape_support() {
        // Regression: the plain `''` single-quote path decodes byte-for-byte as before —
        // borrowing when there is nothing to unescape, allocating only for a doubled quote
        // — under both the ANSI default and MySQL (whose backslash support must not
        // perturb a body carrying no backslash).
        let borrow = literal(LiteralKind::String, 0, "'plain'".len() as u32);
        assert!(matches!(
            borrow.as_str("'plain'").expect("borrow"),
            Cow::Borrowed("plain"),
        ));
        assert!(matches!(
            borrow
                .as_str_in("'plain'", StringLiteralSyntax::MYSQL)
                .expect("borrow"),
            Cow::Borrowed("plain"),
        ));

        let doubled = literal(LiteralKind::String, 0, "'it''s'".len() as u32);
        assert_eq!(doubled.as_str("'it''s'").expect("unescape"), "it's");
    }

    #[test]
    fn double_quoted_and_escaped_strings_render_exact_source() {
        // Materialisation reads source[span] but never mutates it, so the renderer still
        // emits the verbatim spelling (ADR-0006) after the value has materialised.
        for src in [r#""a""b""#, r"'a\nb'"] {
            let literal = literal(LiteralKind::String, 0, src.len() as u32);
            let _ = literal
                .as_str_in(src, StringLiteralSyntax::MYSQL)
                .expect("materializes");
            assert_eq!(rendered(&literal, src), *src, "round-trip for {src:?}");
        }
    }

    #[test]
    fn mysql_multi_segment_and_multiline_double_quoted_strings_materialize() {
        // A MySQL adjacent-string concatenation whose leading `'...'` segment ends in a
        // `\'` escaped quote: locating each segment terminator must honour backslash
        // escapes (as the tokenizer did) before materialising the segment values.
        let concat = literal(LiteralKind::String, 0, "'a\\'b'\n'c'".len() as u32);
        assert_eq!(
            concat
                .as_str_in("'a\\'b'\n'c'", StringLiteralSyntax::MYSQL)
                .expect("concatenation materializes"),
            "a'bc",
        );

        // A double-quoted string with a raw newline in its body is one segment (the parser
        // only continues `'...'`); its terminator is located under the `"` delimiter.
        let multiline = literal(LiteralKind::String, 0, "\"a\nb\"".len() as u32);
        assert_eq!(
            multiline
                .as_str_in("\"a\nb\"", StringLiteralSyntax::MYSQL)
                .expect("multiline double-quoted"),
            "a\nb",
        );
    }

    #[test]
    fn string_accessor_borrows_plain_postgres_escape_string_body() {
        let literal = literal(LiteralKind::String, 7, 15);
        let text = literal
            .as_str("SELECT E'plain'")
            .expect("escape string materializes");

        assert!(matches!(text, Cow::Borrowed("plain")));
    }

    #[test]
    fn string_accessor_materializes_postgres_c_style_escapes() {
        let src = "SELECT E'line\\nquote\\''";
        let literal = literal(LiteralKind::String, 7, src.len() as u32);
        let text = literal.as_str(src).expect("escape string materializes");

        assert!(matches!(text, Cow::Owned(_)));
        assert_eq!(text, "line\nquote'");
    }

    #[test]
    fn string_accessor_materializes_postgres_numeric_and_unicode_escapes() {
        let src = "SELECT e'\\141\\x62\\u0063\\U00000064'";
        let literal = literal(LiteralKind::String, 7, src.len() as u32);
        let text = literal.as_str(src).expect("escape string materializes");

        assert_eq!(text, "abcd");
    }

    #[test]
    fn string_accessor_validates_postgres_escape_output_as_utf8() {
        let src = "SELECT E'\\xc3\\xa9'";
        let valid = literal(LiteralKind::String, 7, src.len() as u32);
        let text = valid.as_str(src).expect("UTF-8 byte escapes materialize");

        assert_eq!(text, "é");

        let src = "SELECT E'\\xff'";
        let invalid = literal(LiteralKind::String, 7, src.len() as u32);
        let error = invalid.as_str(src).expect_err("invalid UTF-8 byte escape");
        assert_eq!(error.kind(), &LiteralValueErrorKind::InvalidString);
    }

    #[test]
    fn string_accessor_rejects_invalid_postgres_escape_values() {
        for src in ["SELECT E'\\u12'", "SELECT E'\\0'", "SELECT E'\\U00110000'"] {
            let literal = literal(LiteralKind::String, 7, src.len() as u32);
            let error = literal.as_str(src).expect_err("invalid escape value");
            assert_eq!(error.kind(), &LiteralValueErrorKind::InvalidString);
        }
    }

    #[test]
    fn string_accessor_treats_unknown_postgres_escapes_as_literal_characters() {
        let src = "SELECT E'\\q\\x'";
        let literal = literal(LiteralKind::String, 7, src.len() as u32);
        let text = literal.as_str(src).expect("unknown escapes materialize");

        assert_eq!(text, "qx");
    }

    /// The escape strings PostgreSQL rejects while *parsing* (confirmed against the
    /// libpg_query oracle): a short or out-of-range Unicode escape, an escape that
    /// decodes to NUL, and a byte escape that does not form valid UTF-8. The
    /// parse-time validator must reject exactly these.
    const POSTGRES_ESCAPE_REJECTS: &[&str] = &[
        "E'\\u12'",       // \u with too few hex digits
        "E'\\u'",         // \u with no hex digits
        "E'\\u006'",      // \u with three hex digits
        "E'\\U0000006'",  // \U with seven hex digits
        "E'\\uD800'",     // lone surrogate code point
        "E'\\U00110000'", // code point above U+10FFFF
        "E'\\0'",         // octal escape decoding to NUL
        "E'\\x00'",       // hex escape decoding to NUL
        "E'\\U00000000'", // Unicode escape decoding to NUL
        "E'\\377'",       // octal 0xFF: not valid UTF-8 on its own
        "E'\\xff'",       // hex 0xFF: not valid UTF-8 on its own
        "E'\\xc3'",       // truncated two-byte UTF-8 sequence
        "E'\\xc3a'",      // 0xC3 followed by a non-continuation byte
    ];

    /// The escape strings PostgreSQL *accepts*: an unknown escape collapses to its
    /// literal character, a bare `\x` is a literal `x`, and well-formed byte/Unicode
    /// escapes decode normally.
    const POSTGRES_ESCAPE_ACCEPTS: &[&str] = &[
        "E''",                       // empty body
        "E'plain'",                  // no escapes
        "E'\\q'",                    // unknown escape -> literal q
        "E'\\x'",                    // bare \x -> literal x
        "E'\\xg'",                   // \x with no hex digit -> literal x then g
        "E'\\q\\x'",                 // both collapse to literal characters
        "E'\\b\\f\\n\\r\\t'",        // the named control escapes
        "E'\\xc3\\xa9'",             // two byte escapes forming valid UTF-8 (e-acute)
        "E'\\141\\x62c\\U00000064'", // mixed octal/hex/unicode forming "abcd"
        "E'it''s'",                  // a doubled quote, no backslash escape
    ];

    #[test]
    fn parse_time_check_rejects_what_postgres_rejects() {
        for text in POSTGRES_ESCAPE_REJECTS {
            assert!(
                !postgres_escape_string_is_valid(text),
                "should reject {text:?}",
            );
        }
    }

    #[test]
    fn parse_time_check_accepts_what_postgres_accepts() {
        for text in POSTGRES_ESCAPE_ACCEPTS {
            assert!(
                postgres_escape_string_is_valid(text),
                "should accept {text:?}",
            );
        }
    }

    #[test]
    fn parse_time_check_skips_non_escape_string_literals() {
        // A non-`E'` literal carries no C-style escapes, so the parse-time check is a
        // no-op there (standard / dollar / unicode strings are validated elsewhere).
        for text in ["'plain'", "$$a\\n$$", "U&'\\D800'", "N'x'"] {
            assert!(postgres_escape_string_is_valid(text), "for {text:?}");
        }
    }

    #[test]
    fn parse_time_check_and_as_str_agree() {
        // The eager parse-time check and the lazy `as_str` materialisation share one
        // escape walk, so they must agree on validity for every body — the property
        // that lets the parser reject up front without changing what a value means.
        for text in POSTGRES_ESCAPE_REJECTS
            .iter()
            .chain(POSTGRES_ESCAPE_ACCEPTS)
        {
            let literal = literal(LiteralKind::String, 0, text.len() as u32);
            let as_str_ok = literal.as_str(text).is_ok();
            let valid = postgres_escape_string_is_valid(text);
            assert_eq!(
                valid, as_str_ok,
                "parse-time check ({valid}) and as_str ({as_str_ok}) disagree for {text:?}",
            );
        }
    }

    /// String literals whose source embeds a raw NUL byte (0x00), one per form. PG
    /// rejects all of these while parsing (confirmed against the libpg_query oracle);
    /// the parse-time check and `as_str` must both reject them.
    const NUL_STRING_REJECTS: &[&str] = &[
        "'a\0b'",         // ordinary
        "'\0'",           // ordinary, lone NUL
        "E'a\0b'",        // escape string, raw NUL byte (not a `\x00` escape)
        "N'a\0b'",        // national string
        "$$a\0b$$",       // dollar-quoted
        "$tag$a\0b$tag$", // tagged dollar-quote
        "U&'a\0b'",       // unicode-escape string
    ];

    /// The same forms with no NUL, each one `as_str` accepts — so a NUL byte is the
    /// only variable the agreement test turns. A non-NUL control byte (0x01) is kept.
    const NUL_STRING_ACCEPTS: &[&str] = &["'ab'", "'a\x01b'", "E'ab'", "N'ab'", "$$ab$$", "U&'ab'"];

    #[test]
    fn parse_time_nul_check_detects_only_a_raw_nul() {
        for text in NUL_STRING_REJECTS {
            assert!(
                string_literal_embeds_nul(text),
                "should detect a NUL in {text:?}",
            );
        }
        for text in NUL_STRING_ACCEPTS {
            assert!(!string_literal_embeds_nul(text), "should accept {text:?}");
        }
    }

    #[test]
    fn parse_time_nul_check_and_as_str_agree() {
        // The eager NUL check (the tokenizer's parse-time gate) and the lazy `as_str`
        // materialisation share one predicate, so they must agree for every body: a NUL
        // makes both reject, its absence lets both accept. This is the property that
        // lets the lexer reject up front without changing what a value means (ADR-0006).
        for text in NUL_STRING_REJECTS.iter().chain(NUL_STRING_ACCEPTS) {
            let literal = literal(LiteralKind::String, 0, text.len() as u32);
            let as_str_ok = literal.as_str(text).is_ok();
            let embeds_nul = string_literal_embeds_nul(text);
            assert_eq!(
                embeds_nul, !as_str_ok,
                "nul-check ({embeds_nul}) and as_str ok ({as_str_ok}) disagree for {text:?}",
            );
        }
    }

    #[test]
    fn string_accessor_borrows_dollar_quoted_body_verbatim() {
        let src = "SELECT $tag$a\\n'b$tag$";
        let literal = literal(LiteralKind::String, 7, src.len() as u32);
        let text = literal
            .as_str(src)
            .expect("dollar-quoted string materializes");

        assert!(matches!(text, Cow::Borrowed("a\\n'b")));
    }

    #[test]
    fn string_accessor_strips_national_string_prefix() {
        // `N'...'` is a plain string constant in PostgreSQL; doubled quotes still collapse.
        let src = "N'caf''e'";
        let literal = literal(LiteralKind::String, 0, src.len() as u32);
        assert_eq!(
            literal.as_str(src).expect("national string materializes"),
            "caf'e",
        );
    }

    #[test]
    fn string_accessor_applies_unicode_escapes_with_default_backslash() {
        // `\0061` and `\+000061` both encode `a`.
        let src = r"U&'d\0061t\+000061'";
        let literal = literal(LiteralKind::String, 0, src.len() as u32);
        assert_eq!(
            literal.as_str(src).expect("unicode string materializes"),
            "data",
        );
    }

    #[test]
    fn string_accessor_borrows_unicode_string_without_escapes() {
        let src = "U&'plain'";
        let literal = literal(LiteralKind::String, 0, src.len() as u32);
        let text = literal.as_str(src).expect("unicode string materializes");
        assert!(matches!(text, Cow::Borrowed("plain")));
    }

    #[test]
    fn string_accessor_applies_unicode_uescape_override() {
        let src = "U&'d!0061t!+000061' UESCAPE '!'";
        let literal = literal(LiteralKind::String, 0, src.len() as u32);
        assert_eq!(
            literal.as_str(src).expect("uescape override materializes"),
            "data",
        );
    }

    #[test]
    fn unicode_ident_decodes_the_double_quoted_identifier_surface() {
        // The `U&"..."` identifier twin of the `U&'...'` string decode: `\XXXX`/`\+XXXXXX`
        // escapes apply, a doubled `""` collapses to one `"`, a single quote is an ordinary
        // body byte (not doubled, unlike the string surface), and a trailing `UESCAPE`
        // overrides the escape character. Values probed against pg_query.
        for (src, value) in [
            (r#"U&"d\0061ta""#, "data"),
            (r#"U&"d0061ta""#, "d0061ta"), // no `\`, no decoding
            (r#"U&"real\00A7_name""#, "real\u{00A7}_name"),
            (r#"U&"""""#, "\""),   // doubled close is one literal `"`
            (r#"U&"a'b""#, "a'b"), // a `'` is an ordinary identifier byte
            (r#"U&"d!0061ta" UESCAPE '!'"#, "data"),
            (r#"U&"\ZZZZ" UESCAPE '!'"#, "\\ZZZZ"), // `!` makes `\` inert
        ] {
            assert_eq!(
                materialize_unicode_ident(src).expect("valid U&\"...\" identifier"),
                value,
                "decoded value for {src:?}",
            );
            assert!(is_unicode_ident(src), "prefix recognised for {src:?}");
        }
    }

    #[test]
    fn unicode_ident_rejects_the_escapes_postgres_rejects() {
        // Malformed escapes against the resolved escape character are `Err` — the value walk
        // and the parser's eager parse-time reject share this, so they can never disagree.
        for src in [
            r#"U&"\ZZZZ""#,    // non-hex escape digits
            r#"U&"\d800""#,    // a lone (unpaired) surrogate
            r#"U&"\0000""#,    // an escape decoding to NUL
            r#"U&"\+110000""#, // a code point above U+10FFFF
        ] {
            assert!(
                materialize_unicode_ident(src).is_none(),
                "{src:?} must be rejected",
            );
        }
    }

    #[test]
    fn string_accessor_combines_unicode_surrogate_pairs() {
        let src = r"U&'\D800\DC00'";
        let literal = literal(LiteralKind::String, 0, src.len() as u32);
        assert_eq!(
            literal.as_str(src).expect("surrogate pair materializes"),
            "\u{10000}",
        );
    }

    #[test]
    fn string_accessor_unfolds_doubled_escape_and_quote_in_unicode_string() {
        // `\\` is a literal backslash; `''` is an embedded quote.
        let src = r"U&'a\\b''c'";
        let literal = literal(LiteralKind::String, 0, src.len() as u32);
        assert_eq!(literal.as_str(src).expect("materializes"), "a\\b'c");
    }

    #[test]
    fn string_accessor_rejects_malformed_unicode_escapes() {
        // PostgreSQL rejects these at parse; per ADR-0006 the lexical form still parses
        // and the accessor is where the value check lands.
        for src in [
            r"U&'\XYZW'",          // non-hex digits
            r"U&'\D800'",          // lone high surrogate
            r"U&'\+110000'",       // beyond U+10FFFF
            r"U&'\0000'",          // NUL
            r"U&'\'",              // dangling escape
            "U&'ab' UESCAPE 'a'",  // a hex digit is an illegal escape character
            "U&'ab' UESCAPE 'xy'", // not a single character
        ] {
            let literal = literal(LiteralKind::String, 0, src.len() as u32);
            let error = literal.as_str(src).expect_err("malformed unicode escape");
            assert_eq!(
                error.kind(),
                &LiteralValueErrorKind::InvalidString,
                "for {src:?}",
            );
        }
    }

    #[test]
    fn bit_string_accessor_borrows_validated_digits() {
        let binary = literal(
            LiteralKind::BitString {
                radix: BitStringRadix::Binary,
            },
            0,
            "B'1010'".len() as u32,
        );
        assert_eq!(binary.as_bit_text("B'1010'").expect("binary body"), "1010");

        let hex = literal(
            LiteralKind::BitString {
                radix: BitStringRadix::Hex,
            },
            0,
            "X'1FF'".len() as u32,
        );
        assert_eq!(hex.as_bit_text("X'1FF'").expect("hex body"), "1FF");
    }

    #[test]
    fn bit_string_accessor_rejects_out_of_radix_digits() {
        let hex = literal(
            LiteralKind::BitString {
                radix: BitStringRadix::Hex,
            },
            0,
            "X'1FG'".len() as u32,
        );
        assert_eq!(
            hex.as_bit_text("X'1FG'")
                .expect_err("G is not a hex digit")
                .kind(),
            &LiteralValueErrorKind::InvalidBitString,
        );

        let binary = literal(
            LiteralKind::BitString {
                radix: BitStringRadix::Binary,
            },
            0,
            "B'012'".len() as u32,
        );
        assert_eq!(
            binary
                .as_bit_text("B'012'")
                .expect_err("2 is not a binary digit")
                .kind(),
            &LiteralValueErrorKind::InvalidBitString,
        );
    }

    #[test]
    fn bit_string_accessor_rejects_non_bit_literal() {
        let literal = literal(LiteralKind::String, 0, 5);
        assert_eq!(
            literal
                .as_bit_text("'abc'")
                .expect_err("string is not a bit string")
                .kind(),
            &LiteralValueErrorKind::WrongKind {
                expected: LiteralExpected::BitString,
                actual: LiteralKind::String,
            },
        );
    }

    #[test]
    fn money_accessor_borrows_numeric_body() {
        // The `$` currency sigil is stripped; the numeric body borrows from source.
        for (src, body) in [("$1234.56", "1234.56"), ("$100", "100"), ("$.5", ".5")] {
            let literal = literal(LiteralKind::Money, 0, src.len() as u32);
            let text = literal.as_money_text(src).expect("money body materializes");
            assert!(matches!(text, Cow::Borrowed(_)), "borrowed for {src:?}");
            assert_eq!(text, body, "body for {src:?}");
        }
    }

    #[test]
    fn money_accessor_rejects_non_money_literal() {
        let literal = literal(LiteralKind::Integer, 0, 3);
        assert_eq!(
            literal
                .as_money_text("100")
                .expect_err("integer is not a money literal")
                .kind(),
            &LiteralValueErrorKind::WrongKind {
                expected: LiteralExpected::Money,
                actual: LiteralKind::Integer,
            },
        );
    }

    #[test]
    fn money_accessor_reports_missing_currency_sigil() {
        // A Money literal whose span text lacks the `$` sigil (e.g. hand-built or
        // detached onto the wrong source) is reported rather than silently mis-sliced.
        let literal = literal(LiteralKind::Money, 0, 3);
        assert_eq!(
            literal
                .as_money_text("100")
                .expect_err("money text without `$`")
                .kind(),
            &LiteralValueErrorKind::InvalidMoney,
        );
    }

    #[test]
    fn temporal_accessor_materializes_value_string_for_each_family() {
        // The literal span covers the whole typed literal; the accessor recovers just
        // the inner value string, with the type recoverable from the kind tag.
        let cases: &[(&str, LiteralKind, &str)] = &[
            ("DATE '1998-12-01'", LiteralKind::Date, "1998-12-01"),
            (
                "TIME WITH TIME ZONE '12:00:00+00'",
                LiteralKind::Time {
                    time_zone: TimeZone::WithTimeZone,
                },
                "12:00:00+00",
            ),
            (
                "TIMESTAMP '2020-01-01 00:00:00'",
                LiteralKind::Timestamp {
                    time_zone: TimeZone::Unspecified,
                },
                "2020-01-01 00:00:00",
            ),
            (
                "INTERVAL '90' DAY",
                LiteralKind::Interval {
                    fields: Some(IntervalFields::Day),
                    precision: None,
                },
                "90",
            ),
            (
                "INTERVAL '1-2' YEAR TO MONTH",
                LiteralKind::Interval {
                    fields: Some(IntervalFields::YearToMonth),
                    precision: None,
                },
                "1-2",
            ),
        ];

        for (src, kind, expected) in cases {
            let literal = literal(kind.clone(), 0, src.len() as u32);
            assert_eq!(
                literal
                    .as_temporal_text(src)
                    .expect("temporal value materializes"),
                *expected,
                "value string for {src:?}",
            );
        }
    }

    #[test]
    fn temporal_accessor_applies_escaping_to_dollar_and_escape_strings() {
        // A trailing interval qualifier still leaves the dollar-quoted value the only
        // `$`-delimited token, so the locator slices it correctly.
        let dollar = literal(
            LiteralKind::Interval {
                fields: Some(IntervalFields::Day),
                precision: None,
            },
            0,
            "INTERVAL $$90$$ DAY".len() as u32,
        );
        assert_eq!(
            dollar
                .as_temporal_text("INTERVAL $$90$$ DAY")
                .expect("dollar-quoted value materializes"),
            "90",
        );

        // A leading `E` escape marker after a keyword word must apply C-style escapes.
        let src = "TIMESTAMP WITH TIME ZONE E'2020-01-01\\t00:00:00'";
        let escape = literal(
            LiteralKind::Timestamp {
                time_zone: TimeZone::WithTimeZone,
            },
            0,
            src.len() as u32,
        );
        assert_eq!(
            escape
                .as_temporal_text(src)
                .expect("escape value materializes"),
            "2020-01-01\t00:00:00",
        );
    }

    #[test]
    fn temporal_accessor_rejects_non_temporal_literal() {
        let literal = literal(LiteralKind::String, 0, 4);
        assert_eq!(
            literal
                .as_temporal_text("'ab'")
                .expect_err("string is not a temporal literal")
                .kind(),
            &LiteralValueErrorKind::WrongKind {
                expected: LiteralExpected::Temporal,
                actual: LiteralKind::String,
            },
        );
    }

    #[test]
    fn boolean_and_null_accessors_do_not_need_source() {
        assert_eq!(synthetic(LiteralKind::Boolean(true)).as_bool(), Ok(true));
        assert_eq!(synthetic(LiteralKind::Null).as_null(), Ok(()));
        assert!(synthetic(LiteralKind::Null).is_null());
    }

    #[test]
    fn wrong_kind_errors_name_expected_and_actual_kinds() {
        let literal = synthetic(LiteralKind::String);
        let error = literal.as_bool().expect_err("string is not boolean");

        assert_eq!(
            error.kind(),
            &LiteralValueErrorKind::WrongKind {
                expected: LiteralExpected::Boolean,
                actual: LiteralKind::String,
            },
        );
    }

    #[test]
    fn detached_literal_reports_missing_source() {
        let literal = synthetic(LiteralKind::Integer);
        let error = literal
            .as_i64("1")
            .expect_err("synthetic span has no slice");

        assert_eq!(error.kind(), &LiteralValueErrorKind::MissingSource);
        assert_eq!(error.span(), Span::SYNTHETIC);
    }

    #[test]
    fn invalid_span_reports_source_range_error() {
        let literal = literal(LiteralKind::String, 0, 20);
        let error = literal.as_str("'x'").expect_err("span is out of range");

        assert_eq!(error.kind(), &LiteralValueErrorKind::InvalidSourceRange);
    }

    #[test]
    fn malformed_string_reports_invalid_string() {
        let literal = literal(LiteralKind::String, 0, 4);
        let error = literal
            .as_str("oops")
            .expect_err("not a single-quoted string literal");

        assert_eq!(error.kind(), &LiteralValueErrorKind::InvalidString);
    }

    #[test]
    fn string_accessor_rejects_truncated_escape_and_national_prefixes() {
        // A 2-byte `E'`/`e'`/`N'`/`n'` span carries a prefix and an opening quote but no
        // body or closing quote: slicing past the opening quote must report `InvalidString`,
        // never panic. Reachable via a detached or source-mismatched literal, which the
        // accessors contract to reject rather than crash on (ADR-0006).
        for src in ["E'", "e'", "N'", "n'"] {
            let literal = literal(LiteralKind::String, 0, src.len() as u32);
            assert_eq!(
                literal
                    .as_str(src)
                    .expect_err("truncated prefix is not a valid string")
                    .kind(),
                &LiteralValueErrorKind::InvalidString,
                "for {src:?}",
            );
        }
    }

    #[test]
    fn string_accessor_concatenates_newline_separated_segments() {
        // SQL-standard adjacent-string concatenation: a span covering two quoted
        // segments joined across a newline is one value. The result is owned because
        // concatenation allocates (ADR-0006: the value is recovered, joined, lazily).
        let src = "'foo'\n'bar'";
        let literal = literal(LiteralKind::String, 0, src.len() as u32);
        let value = literal.as_str(src).expect("adjacent strings concatenate");

        assert!(matches!(value, Cow::Owned(_)));
        assert_eq!(value, "foobar");
    }

    #[test]
    fn string_accessor_concatenates_three_segments_and_collapses_doubled_quotes() {
        // Three segments, each materialised under standard rules (doubled `''` -> `'`)
        // before concatenation.
        let src = "'a''b'\n'c'\n'd''e'";
        let literal = literal(LiteralKind::String, 0, src.len() as u32);

        assert_eq!(literal.as_str(src).expect("three segments"), "a'bcd'e");
    }

    #[test]
    fn string_accessor_concatenates_mixed_escape_and_standard_segments() {
        // A leading `E'...'` escape segment then a plain continuation: each segment is
        // materialised under its own form, so the escape applies only within its
        // segment (the `\x4` does not greedily consume the next segment's `1`).
        let src = "E'\\x4'\n'1'";
        let literal = literal(LiteralKind::String, 0, src.len() as u32);
        let value = literal.as_str(src).expect("mixed segments concatenate");

        assert_eq!(value, "\u{4}1");
    }

    #[test]
    fn string_accessor_single_segment_with_body_newline_is_not_concatenation() {
        // A newline *inside* a single string body is an ordinary character, not a
        // segment separator: the span is one segment and round-trips as such.
        let src = "'line1\nline2'";
        let literal = literal(LiteralKind::String, 0, src.len() as u32);
        let value = literal.as_str(src).expect("multi-line string body");

        assert!(matches!(value, Cow::Borrowed("line1\nline2")));
    }

    #[test]
    fn bit_string_accessor_concatenates_newline_separated_segments() {
        // Bit-string constants concatenate the same way (PostgreSQL continues `<xb>`
        // too); the digits join and validate against the radix as one body.
        let src = "B'1010'\n'0101'";
        let literal = literal(
            LiteralKind::BitString {
                radix: BitStringRadix::Binary,
            },
            0,
            src.len() as u32,
        );
        let value = literal
            .as_bit_text(src)
            .expect("adjacent bit strings concatenate");

        assert!(matches!(value, Cow::Owned(_)));
        assert_eq!(value, "10100101");
    }

    #[test]
    fn bit_string_accessor_concatenation_still_validates_radix() {
        // A digit illegal under the radix is rejected after joining, the deferred check
        // PostgreSQL also leaves until use.
        let src = "B'10'\n'21'";
        let literal = literal(
            LiteralKind::BitString {
                radix: BitStringRadix::Binary,
            },
            0,
            src.len() as u32,
        );
        let error = literal
            .as_bit_text(src)
            .expect_err("`2` is not a binary digit");

        assert_eq!(error.kind(), &LiteralValueErrorKind::InvalidBitString);
    }

    #[test]
    fn integer_accessor_materializes_radix_and_separator_forms() {
        // The radix prefix is decoded in its base and `_` separators drop out; each form
        // the tokenizer folds into a `Number` token now materialises to its value.
        for (src, value) in [
            ("0x1F", 31),
            ("0b1010", 10),
            ("0o17", 15),
            ("1_500_000", 1_500_000),
        ] {
            let literal = literal(LiteralKind::Integer, 0, src.len() as u32);
            assert_eq!(literal.as_i64(src), Ok(value), "value for {src:?}");
        }
    }

    #[test]
    fn integer_accessor_rejects_misplaced_separators() {
        // The lexer keeps `_` placement permissive and defers the strict between-digits
        // rule to materialisation (ADR-0006); each malformed form fails here, including a
        // radix body whose separator hugs the prefix (`0x_1F`) or trails the digits.
        for src in ["_1", "1_", "1__2", "0x_1F", "0x1F_"] {
            let literal = literal(LiteralKind::Integer, 0, src.len() as u32);
            assert_eq!(
                literal.as_i64(src).expect_err("misplaced separator").kind(),
                &LiteralValueErrorKind::InvalidInteger,
                "rejecting {src:?}",
            );
        }
    }

    #[test]
    fn decimal_text_accessor_strips_separators_and_normalizes_radix() {
        // Separators drop so the text feeds a decimal library cleanly; a radix integer,
        // which has no base-10 spelling, is normalised to its decimal value.
        let underscores = literal(LiteralKind::Integer, 0, "1_500_000".len() as u32);
        assert_eq!(
            underscores
                .as_decimal_text("1_500_000")
                .expect("separators strip"),
            "1500000",
        );

        let hex = literal(LiteralKind::Integer, 0, "0xFF".len() as u32);
        assert_eq!(hex.as_decimal_text("0xFF").expect("hex normalises"), "255");
    }

    #[test]
    fn radix_and_separator_literals_render_their_exact_source_spelling() {
        // Materialisation reads source[span] but never mutates the span, so the renderer
        // — which slices the same span — still emits the verbatim dialect spelling, even
        // after the value has been materialised (ADR-0006).
        for src in ["0x1F", "0b1010", "0o17", "1_500_000"] {
            let literal = literal(LiteralKind::Integer, 0, src.len() as u32);
            let _ = literal.as_i64(src).expect("materialises");
            assert_eq!(rendered(&literal, src), *src, "round-trip for {src:?}");
        }
    }

    /// A literal slices its value from `source` by span and never consults the resolver,
    /// so one that resolves nothing suffices to drive the renderer here.
    struct NoSymbols;

    impl Resolver for NoSymbols {
        fn try_resolve(&self, _sym: Symbol) -> Option<&str> {
            None
        }
    }

    fn rendered(literal: &Literal, source: &str) -> String {
        let resolver = NoSymbols;
        let config = RenderConfig::default();
        let ctx = RenderCtx::new(&resolver, source, &config);
        literal.displayed(&ctx).to_string()
    }

    fn literal(kind: LiteralKind, start: u32, end: u32) -> Literal {
        Literal {
            kind,
            meta: Meta::new(
                Span::new(start, end),
                NodeId::new(1).expect("non-zero node id"),
            ),
        }
    }

    fn synthetic(kind: LiteralKind) -> Literal {
        Literal {
            kind,
            meta: Meta::new(Span::SYNTHETIC, NodeId::new(1).expect("non-zero node id")),
        }
    }
}
