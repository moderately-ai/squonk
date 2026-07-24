// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Const dialect data shared by the parser and renderer.
//!
//! Custom dialects are built from explicit deltas over a base preset:
//!
//! ```
//! # use squonk_ast::dialect::{Casing, FeatureDelta, FeatureSet};
//! let custom = FeatureSet::ANSI.with(
//!     FeatureDelta::EMPTY.identifier_casing(Casing::Preserve),
//! );
//! assert_eq!(custom.identifier_quotes, FeatureSet::ANSI.identifier_quotes);
//! ```
//!
//! # Flag naming: widening vs narrowing
//!
//! Every boolean feature flag names the direction ON moves acceptance:
//!
//! - **Widening** (ON accepts more): a noun phrase of the accepted surface, e.g.
//!   `distinct_on`, `limit_percent`, `identity_columns`.
//! - **Narrowing / enforcement** (ON rejects more): `<subject>_requires_<x>` or
//!   `<subject>_rejects_<x>`, e.g. `with_ties_requires_order_by`,
//!   `values_rows_require_equal_arity`, `as_alias_rejects_reserved`.
//!
//! Deliberate exceptions:
//!
//! - [`StringFuncForms::position_asymmetric_operands`] *swaps* grammar rather than widening
//!   or narrowing it — ON tightens the needle operand and widens the haystack — so neither
//!   direction word fits and it keeps a descriptive noun phrase.
//! - [`CallSyntax::restricted_cast_targets`] is grandfathered: the participle already reads
//!   as a narrowing, and the name is referenced widely enough that a rename to the
//!   `requires`/`rejects` form would churn more than it clarifies.
//! - [`QueryTailSyntax::with_ties_requires_order_by`] is named for its load-bearing
//!   `ORDER BY` guard; investigation shows it always co-varies with PostgreSQL's paired
//!   `SKIP LOCKED` + `WITH TIES` reject (only Postgres enables the flag), so both guards
//!   ride one knob rather than a second `rejects_skip_locked` field that would never
//!   diverge in shipped presets.
//!
//! # Struct header docs: category + doctrine, never member lists
//!
//! A sub-struct's header states the *category* of surface it gates and the *doctrine* its
//! flags follow (widening/narrowing per the rule above, engine-probed on/off, leading-keyword
//! dispatch, …), plus pointers to the relevant registries — never an enumeration of the
//! specific flags/statements/clauses it contains. An enumeration is stale-by-growth: every
//! field added past the ones the header happened to name turns the list into a misleading
//! subset. A statement of the struct's MECE boundary against its sibling axes is *not* an
//! enumeration, and is encouraged.

pub mod closed_delta;
pub mod keyword;
pub mod lex_class;

// Each shipped dialect owns its preset, reserved sets, and byte-class choices in
// its own module; the shared `FeatureSet`/`KeywordSet`/`ByteClasses` machinery in
// this module stays dialect-agnostic. `ansi` is the always-compiled baseline every
// dialect derives from; `postgres`/`mysql`/`sqlite`/`duckdb`/`lenient` are each gated
// behind their cargo feature so an excluded dialect's preset data is genuinely not
// compiled.
mod ansi;
#[cfg(feature = "bigquery")]
mod bigquery;
#[cfg(feature = "clickhouse")]
mod clickhouse;
#[cfg(feature = "databricks")]
mod databricks;
#[cfg(feature = "duckdb")]
mod duckdb;
#[cfg(feature = "hive")]
mod hive;
#[cfg(feature = "lenient")]
mod lenient;
#[cfg(feature = "mssql")]
mod mssql;
#[cfg(feature = "mysql")]
mod mysql;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "quiltdb")]
mod quiltdb;
#[cfg(feature = "redshift")]
mod redshift;
#[cfg(feature = "snowflake")]
mod snowflake;
#[cfg(feature = "sqlite")]
mod sqlite;

pub use ansi::{
    ANSI, MATCH_RECOGNIZE_RESERVATION, PIVOT_RESERVATION, QUALIFY_RESERVATION, RESERVED_BARE_ALIAS,
    RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME, RESERVED_SET_VALUE_WORDS, RESERVED_TYPE_NAME,
    STANDARD_IDENTIFIER_QUOTES,
};
#[cfg(feature = "bigquery")]
pub use bigquery::{BIGQUERY, BIGQUERY_IDENTIFIER_QUOTES};
#[cfg(feature = "clickhouse")]
pub use clickhouse::{CLICKHOUSE, CLICKHOUSE_IDENTIFIER_QUOTES};
#[cfg(feature = "databricks")]
pub use databricks::{
    DATABRICKS, DATABRICKS_IDENTIFIER_QUOTES, DATABRICKS_RESERVED_BARE_ALIAS,
    DATABRICKS_RESERVED_COLUMN_NAME, DATABRICKS_RESERVED_FUNCTION_NAME,
    DATABRICKS_RESERVED_TYPE_NAME,
};
#[cfg(feature = "duckdb")]
pub use duckdb::{DUCKDB, DUCKDB_BINDING_POWERS};
#[cfg(feature = "hive")]
pub use hive::{HIVE, HIVE_IDENTIFIER_QUOTES};
pub use keyword::{Keyword, KeywordSet, lookup_keyword};
#[cfg(feature = "lenient")]
pub use lenient::{LENIENT, LENIENT_IDENTIFIER_QUOTES};
pub use lex_class::{
    ByteClassTable, ByteClasses, DUCKDB_BYTE_CLASSES, MYSQL_BYTE_CLASSES, POSTGRES_BYTE_CLASSES,
    SQLITE_BYTE_CLASSES, STANDARD_BYTE_CLASSES,
};
#[cfg(feature = "mssql")]
pub use mssql::{MSSQL, MSSQL_IDENTIFIER_QUOTES};
#[cfg(feature = "mysql")]
pub use mysql::{
    MYSQL, MYSQL_IDENTIFIER_QUOTES, MYSQL_RESERVED_BARE_ALIAS, MYSQL_RESERVED_COLUMN_NAME,
    MYSQL_RESERVED_FUNCTION_NAME, MYSQL_RESERVED_TYPE_NAME, MYSQL_WINDOW_FUNCTION_KEYWORDS,
};
#[cfg(feature = "postgres")]
pub use postgres::POSTGRES;
#[cfg(feature = "quiltdb")]
pub use quiltdb::QUILTDB;
#[cfg(feature = "redshift")]
pub use redshift::REDSHIFT;
#[cfg(feature = "snowflake")]
pub use snowflake::{
    SNOWFLAKE, SNOWFLAKE_RESERVED_BARE_ALIAS, SNOWFLAKE_RESERVED_COLUMN_NAME,
    SNOWFLAKE_RESERVED_FUNCTION_NAME, SNOWFLAKE_RESERVED_TYPE_NAME,
    SNOWFLAKE_TABLE_OPERATOR_RESERVATION,
};
#[cfg(feature = "sqlite")]
pub use sqlite::{
    SQLITE, SQLITE_IDENTIFIER_QUOTES, SQLITE_RESERVED_BARE_ALIAS, SQLITE_RESERVED_COLUMN_NAME,
    SQLITE_RESERVED_FUNCTION_NAME, SQLITE_RESERVED_TYPE_NAME,
};

use std::borrow::Cow;

use crate::ast::{
    BinaryOperator, BitwiseXorSpelling, IntegerDivideSpelling, ModuloSpelling, RegexpSpelling,
    SetOperator, UnaryOperator,
};
use crate::precedence::{BindingPower, BindingPowerTable, SetOperationBindingPowerTable};

/// How unquoted identifiers fold for identity in consumers such as planners.
///
/// The parser still interns the exact source text; this value describes later
/// identity behaviour without losing render fidelity.
///
/// One tri-state models a single identity fold shared by *every* unquoted
/// identifier — table, column, and alias alike. That single-fold model is a
/// deliberate, parity-matching choice, not a gap: `datafusion-sqlparser-rs`, the
/// drop-in target, does not key case sensitivity by identifier kind either, so one
/// knob already meets parity and per-kind folding would be strictly more expressive
/// than parity requires. It also keeps the dialect model minimal.
/// [`Self::Lower`] approximates the MySQL/T-SQL column rule — "case-preserving
/// storage, case-insensitive comparison" — closely: fold lower for identity while
/// the interned text still renders exactly as written (folding is layered onto
/// the exact form, never baked into the parse).
///
/// # Known limitation: per-identifier-kind sensitivity
///
/// MySQL and T-SQL are case-insensitive for columns/aliases, but table and database
/// name sensitivity is a *deployment* setting (MySQL `lower_case_table_names`, the
/// T-SQL server/database collation). A case-insensitive column beside a
/// case-sensitive table cannot be expressed by one fold, and that deployment-config
/// knob is out of model; [`Self::Lower`] is the closest single fit and is what both
/// dialects use. Per-identifier-kind casing is a deliberate future extension, gated
/// on committing to a case-sensitive-table dialect (T-SQL): only then does the
/// table-vs-column split change parse/identity results, so until such a dialect
/// ships the added expressiveness would be unused surface (an M3 decision).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Casing {
    /// Fold unquoted identifiers to upper case (Oracle/DB2 style).
    Upper,
    /// Fold unquoted identifiers to lower case (PostgreSQL style).
    Lower,
    /// Preserve unquoted identifier case as written (MySQL/SQL Server style).
    Preserve,
}

impl Casing {
    /// Fold an unquoted identifier for dialect identity comparison.
    ///
    /// M1 performs ASCII folding only. Non-ASCII bytes are preserved, matching
    /// the tokenizer's permissive Unicode stance until a later precision pass
    /// introduces full Unicode identifier semantics.
    pub fn fold_identifier<'a>(&self, identifier: &'a str) -> Cow<'a, str> {
        match self {
            Self::Upper => fold_identifier_upper(identifier),
            Self::Lower => fold_identifier_lower(identifier),
            Self::Preserve => Cow::Borrowed(identifier),
        }
    }
}

/// Default sort position for null values when a query omits explicit ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NullOrdering {
    /// Sort null values before non-null values by default.
    NullsFirst,
    /// Sort null values after non-null values by default.
    NullsLast,
}

impl NullOrdering {
    /// Whether this default places nulls before non-null values.
    pub const fn nulls_first(&self) -> bool {
        match self {
            Self::NullsFirst => true,
            Self::NullsLast => false,
        }
    }
}

/// Which dialect's canonical surface spelling a target-dialect render emits.
///
/// When [`RenderSpelling::TargetDialect`] rendering normalizes a construct whose
/// spelling diverges across dialects — today the divergent type names (`NUMERIC` vs
/// `DECIMAL`, `TIMESTAMPTZ` vs `TIMESTAMP WITH TIME ZONE`, `BYTEA` vs `BLOB`, …) — it
/// emits this family's spelling. It is the dialect *data* the renderer reads instead
/// of recognizing a preset by identity: each preset declares its family here, so
/// PostgreSQL-vs-ANSI output spelling is a `FeatureSet` field, not a compile-time
/// `FeatureSet::POSTGRES` comparison gated on the `postgres` cargo feature.
/// `PreserveSource` rendering ignores it and keeps the AST's own syntax tag.
///
/// [`RenderSpelling::TargetDialect`]: crate::render::RenderSpelling::TargetDialect
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetSpelling {
    /// The ANSI/SQL-standard canonical spelling — the portable baseline every dialect
    /// without its own Tier-1 spelling table renders to.
    Ansi,
    /// PostgreSQL's canonical spelling (`NUMERIC`, `TIMESTAMPTZ`, `BYTEA`, `VARCHAR`,
    /// the explicit zone-suffix forms).
    Postgres,
}

/// What the `||` operator token *means* in a dialect.
///
/// ANSI and PostgreSQL concatenate; MySQL/MariaDB without `PIPES_AS_CONCAT` treat
/// `||` as a synonym for logical `OR` (sqlglot's `DPIPE_IS_STRING_CONCAT`). This
/// is dialect *meaning* data, not tree shape: both spellings map to a
/// single canonical [`BinaryOperator`], and the precedence follows automatically
/// from that operator — `OR` binds looser than concatenation, so no separate
/// binding-power override is needed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipeOperator {
    /// `||` concatenates strings ([`BinaryOperator::StringConcat`]).
    StringConcat,
    /// `||` is logical OR ([`BinaryOperator::Or`]).
    LogicalOr,
}

impl PipeOperator {
    /// The canonical binary operator `||` parses to under this dialect.
    pub const fn binary_operator(self) -> BinaryOperator {
        match self {
            Self::StringConcat => BinaryOperator::StringConcat,
            Self::LogicalOr => BinaryOperator::Or,
        }
    }
}

/// What the `&&` operator token *means* in a dialect — the operator-meaning analog
/// of [`PipeOperator`].
///
/// MySQL/MariaDB treat `&&` as a synonym for logical `AND`; DuckDB (and PostgreSQL)
/// spell array/range/geometry overlap with `&&`; ANSI has no `&&` scalar operator. This
/// is dialect *meaning* data: the `&&` lexeme always tokenizes, and this decides whether
/// it is an infix operator and which canonical [`BinaryOperator`] it maps to. The chosen
/// operator's binding power follows automatically, so there is no separate precedence
/// override.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoubleAmpersand {
    /// `&&` is not a scalar infix operator (ANSI). It still lexes, but the parser rejects
    /// it in expression position rather than mis-binding.
    Unsupported,
    /// `&&` is logical AND ([`BinaryOperator::And`], MySQL/MariaDB).
    LogicalAnd,
    /// `&&` is the overlap operator ([`BinaryOperator::Overlap`]) — array/range overlap
    /// (PostgreSQL) and bounding-box overlap over geometries (DuckDB). Binds at the "any
    /// other operator" precedence.
    Overlaps,
}

impl DoubleAmpersand {
    /// The canonical binary operator `&&` parses to, or `None` when the dialect
    /// does not treat `&&` as a scalar infix operator.
    pub const fn binary_operator(self) -> Option<BinaryOperator> {
        match self {
            Self::Unsupported => None,
            Self::LogicalAnd => Some(BinaryOperator::And),
            Self::Overlaps => Some(BinaryOperator::Overlap),
        }
    }
}

/// Whether a dialect treats MySQL's reserved-keyword infix operators — `DIV`, `MOD`,
/// `XOR`, `RLIKE`, `REGEXP` — as operators.
///
/// MySQL spells these infix operators as keywords rather than symbols; ANSI and
/// PostgreSQL have none of them (the words are ordinary identifiers there). Which
/// keywords act as operators is therefore an explicit dialect-data decision,
/// the keyword analogue of [`PipeOperator`]/[`DoubleAmpersand`]: this
/// maps a reserved keyword token to the canonical [`BinaryOperator`] it folds onto.
/// Precedence is *not* stored here — each operator's binding power is owned by the
/// [`BindingPowerTable`] (`DIV`/`MOD` multiplicative, `XOR` between `AND` and `OR`,
/// `RLIKE`/`REGEXP` comparison), so meaning and precedence cannot drift.
///
/// The `MySql` variant is named for a dialect, not the capability it grants — the
/// lone place a `FeatureSet` value is dialect-named — and that is deliberate. A
/// variant here denotes one dialect's *exact* keyword-operator set (MySQL's
/// `DIV`/`MOD`/`XOR`/`RLIKE`/`REGEXP` mapping), which is a specification, not a
/// composable capability: a future dialect with a different set gets its own
/// variant rather than reusing this one, so the dialect name *is* the spec. A
/// capability name like `DivModXorRegexp` would only restate that mapping, less
/// precisely and with no room for the next dialect's variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeywordOperators {
    /// No keyword infix operators (ANSI/PostgreSQL): `DIV`/`MOD`/`XOR`/`RLIKE`/`REGEXP`
    /// are ordinary identifiers, so in infix position they end the expression.
    Unsupported,
    /// The MySQL keyword infix operators: `DIV`, `MOD`, `XOR`, `RLIKE`, `REGEXP`.
    MySql,
    /// The SQLite keyword infix operators: `GLOB`, `MATCH`, `REGEXP`. Like
    /// [`MySql`](Self::MySql), the variant is named for the dialect whose *exact*
    /// keyword-operator set it denotes (a specification, not a composable capability),
    /// per the dialect-named-variant rationale documented on this enum. `GLOB` is a
    /// built-in the differential oracle can verify; `MATCH`/`REGEXP` are grammar hooks
    /// backed by application-defined functions the bundled engine does not register,
    /// so they parse but a bare `prepare` rejects them — grammar-only siblings.
    Sqlite,
    /// DuckDB's keyword infix set is just `GLOB` (engine-probed on 1.5.4: `MATCH` /
    /// `REGEXP` are not keyword infix operators there). Named for the dialect's exact
    /// set, same rationale as [`Sqlite`](Self::Sqlite).
    DuckDb,
}

impl KeywordOperators {
    /// The canonical binary operator `keyword` maps to as an infix operator under
    /// this dialect, or `None` when `keyword` is not a keyword operator here.
    ///
    /// The `MOD` and `RLIKE`/`REGEXP` keywords fold onto the shared modulo/regex
    /// operators with a surface tag so the exact spelling round-trips;
    /// `DIV` and `XOR` are their own operator keys.
    pub const fn binary_operator(self, keyword: Keyword) -> Option<BinaryOperator> {
        match self {
            Self::Unsupported => None,
            Self::MySql => match keyword {
                Keyword::Div => Some(BinaryOperator::IntegerDivide(IntegerDivideSpelling::Div)),
                Keyword::Mod => Some(BinaryOperator::Modulo(ModuloSpelling::Mod)),
                Keyword::Xor => Some(BinaryOperator::Xor),
                Keyword::Rlike => Some(BinaryOperator::Regexp(RegexpSpelling::Rlike)),
                Keyword::Regexp => Some(BinaryOperator::Regexp(RegexpSpelling::Regexp)),
                _ => None,
            },
            // SQLite's `GLOB`/`MATCH` get their own operator keys; `REGEXP` folds onto
            // the shared regex operator with the `Regexp` spelling tag (same round-trip
            // pattern MySQL uses for `RLIKE`/`REGEXP`).
            Self::Sqlite => match keyword {
                Keyword::Glob => Some(BinaryOperator::Glob),
                Keyword::Match => Some(BinaryOperator::Match),
                Keyword::Regexp => Some(BinaryOperator::Regexp(RegexpSpelling::Regexp)),
                _ => None,
            },
            Self::DuckDb => match keyword {
                Keyword::Glob => Some(BinaryOperator::Glob),
                _ => None,
            },
        }
    }
}

/// What the always-lexed `^` operator token *means* in a dialect — the operator-meaning
/// analog of [`PipeOperator`]/[`DoubleAmpersand`].
///
/// The `^` byte is a single-character self token that always tokenizes; this decides what
/// an infix `^` binds to. PostgreSQL/DuckDB read it as arithmetic exponentiation, MySQL as
/// bitwise XOR, and ANSI/SQLite give it no infix meaning at all. The three readings are
/// mutually exclusive per dialect — one byte, one meaning — so a single meaning-enum makes
/// the invalid "both power and XOR" state unrepresentable (conflict-exempt: the both-state is
/// unrepresentable by construction, so no [`GrammarConflict`] registry variant governs the
/// exclusion). It folds together what used to
/// be an `exponent_operator` bool and a `Caret`-XOR spelling that prose alone kept apart.
/// The precedence follows from the mapped [`BinaryOperator`] via the dialect's
/// [`BindingPowerTable`] (exponent at its own tier tighter than `*`; `Caret` XOR at the
/// bitwise-XOR rank), so there is no separate override here.
///
/// Bitwise XOR's *other* spelling `#` (PostgreSQL) rides a different byte on its own axis,
/// [`FeatureSet::hash_bitwise_xor`]; `^` and `#` never share this enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaretOperator {
    /// `^` is not an infix operator (ANSI/SQLite): it still lexes, but the parser ends the
    /// expression rather than binding it.
    Unsupported,
    /// `^` is arithmetic exponentiation ([`BinaryOperator::Exponent`], PostgreSQL/DuckDB) —
    /// its own precedence tier, tighter than `*`.
    Exponent,
    /// `^` is bitwise XOR ([`BinaryOperator::BitwiseXor`] under the
    /// [`Caret`](BitwiseXorSpelling::Caret) spelling, MySQL).
    BitwiseXor,
}

impl CaretOperator {
    /// The canonical binary operator `^` maps to under this dialect, or `None` when the
    /// dialect gives `^` no infix meaning.
    pub const fn binary_operator(self) -> Option<BinaryOperator> {
        match self {
            Self::Unsupported => None,
            Self::Exponent => Some(BinaryOperator::Exponent),
            Self::BitwiseXor => Some(BinaryOperator::BitwiseXor(BitwiseXorSpelling::Caret)),
        }
    }
}

/// Dialect-owned comment syntax extensions.
///
/// Standard `--` line comments and `/* … */` block comments are always part of
/// the baseline. These flags cover dialect-specific comment forms — and the
/// dialect-specific *shape* of the baseline forms — whose recognition is an
/// explicit dialect data decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommentSyntax {
    /// Treat `#` as a line-comment introducer (MySQL). One of several coexisting
    /// `#` claimants (see [`FeatureSet`] module docs, "Shared byte-trigger ownership:
    /// `#`"); trivia phase shadows identifier / XOR / positional readings when on.
    /// Do not also mark `#` an identifier-start in byte classes (T-SQL `#temp`), or the
    /// comment branch wins and `#temp` never lexes as a word
    /// ([`LexicalConflict::HashCommentVersusHashIdentifier`](crate::dialect::LexicalConflict)).
    pub line_comment_hash: bool,
    /// Whether a bare carriage return (`\r`, `0x0d`) ends a `--`/`#` line comment, on
    /// top of the newline (`\n`) that always ends one. PostgreSQL and DuckDB terminate a
    /// line comment at *either* `\n` or `\r` — their flex scanner's comment body is
    /// `[^\n\r]*` (engine-verified: `SELECT 1 -- c\rFROM` reads `FROM` as a live token and
    /// rejects) — while SQLite and MySQL end it at `\n` alone, treating a `\r` as ordinary
    /// comment content (the same input is one comment to end-of-line, accepted). All four
    /// engines fold `\r` as whitespace *outside* a comment, so this flag governs only
    /// whether `\r` *ends* a comment, never how it lexes elsewhere. The terminating byte is
    /// left for the whitespace scan either way (`\r` is in `CLASS_WHITESPACE` for every
    /// preset), so it never joins the comment's trivia span — matching PG, whose `[^\n\r]*`
    /// body excludes the `\r`. `0x0b`/`0x0c` (vertical tab / form feed) sit in the flex
    /// `space` set but not its newline set, so they are never terminators here.
    pub line_comment_ends_at_carriage_return: bool,
    /// Whether `/* … */` block comments nest: an inner `/*` raises a depth an
    /// inner `*/` must lower before the comment can close. PostgreSQL nests;
    /// MySQL ends every block comment at the first `*/` (engine-verified against
    /// mysql:8 — `SELECT /* a /* b */ 1` parses as `SELECT 1` there, while a
    /// nesting scanner reads it as an unterminated comment). The permissive
    /// nesting superset predates this flag, so every preset except
    /// MySQL keeps it on.
    pub nested_block_comments: bool,
    /// MySQL versioned comments (`/*! … */`, `/*!NNNNN … */`) as *conditional
    /// inclusion*: the engine executes the body, so it is not a comment. `None`
    /// (every non-MySQL dialect) keeps the whole construct an ordinary block
    /// comment. `Some(bound)` models a server whose `MYSQL_VERSION_ID` is
    /// `bound`: the body lexes as live tokens when the version is absent or
    /// `<= bound`, and the region is discarded wholesale when the version
    /// exceeds it — exactly the engine's include/skip gate.
    ///
    /// Engine-verified semantics (probed against mysql:8, 8.4.10): the version
    /// is the digit run immediately abutting the `!` (a space breaks it) —
    /// exactly five or exactly six digits form a version; 0–4 digits are not a
    /// version (the digits stay body tokens and the region is included
    /// unconditionally); from a run of ≥7 the first five are the version.
    /// Regions do not nest (a flag, not a depth): a passing inner `/*!NNNNN`
    /// marker is a no-op, a failing one discards only up to the next `*/`, and
    /// the first region-level `*/` closes the region. A `*/` inside a string
    /// literal of an *included* body does not close it (the body is lexed
    /// normally), while a *discarded* body is raw bytes — not string-aware.
    pub versioned_comments: Option<u32>,
    /// Whether an unterminated `/* … ` block comment running to end of input is silently
    /// closed (valid trailing trivia) rather than the pre-existing hard
    /// `UnterminatedBlockComment` error. SQLite's tokenizer swallows a `/*` whose body runs
    /// off the end as a `TK_SPACE` (engine-measured on rusqlite: `SELECT 1/* eof` and a
    /// whitespace-wrapped `\t\t/*\t\t` both prepare), while every other engine rejects it.
    ///
    /// The one exception, replicated here: SQLite treats a *bare* `/*` sitting exactly at
    /// end of input (no byte after the `*`, `z[2]==0`) as the `/` slash operator, not a
    /// comment — so `/*` alone and `SELECT 1 /*` still reject on both. The scanner honours
    /// this by opening a silently-EOF-closed comment only when a byte follows the `/*`. On
    /// for SQLite / Lenient, off elsewhere.
    pub unterminated_block_comment_at_eof: bool,
}

/// One accepted identifier-quoting delimiter style.
///
/// SQL identifier quotes escape an embedded close delimiter by *doubling* it
/// (`"a""b"`, `[a]]b]`, and the backtick analogue), so the escape rule is implied by the kind —
/// there is no separate escape character. A dialect accepts a *set* of these
/// ([`FeatureSet::identifier_quotes`]); the tokenizer emits a single `QuotedIdent`
/// token spanning the delimiters, and stripping/unescaping into an `Ident` is a
/// later stage that reads the style back from the span's opening byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifierQuote {
    /// Open and close are the same delimiter (`"`, `` ` ``).
    Symmetric(char),
    /// Distinct open/close (T-SQL `[a]`); only the close needs doubling to escape.
    Asymmetric {
        /// The opening delimiter character.
        open: char,
        /// The closing delimiter character.
        close: char,
    },
}

impl IdentifierQuote {
    /// The opening delimiter.
    pub const fn open(self) -> char {
        match self {
            Self::Symmetric(delim) => delim,
            Self::Asymmetric { open, .. } => open,
        }
    }

    /// The closing delimiter (doubled to escape an embedded close).
    pub const fn close(self) -> char {
        match self {
            Self::Symmetric(delim) => delim,
            Self::Asymmetric { close, .. } => close,
        }
    }
}

/// Dialect-owned string literal syntax extensions.
///
/// Standard single-quoted strings are always part of the baseline. These flags
/// cover dialect-specific forms whose recognition must be an explicit dialect
/// data decision, not a parser-side type check. Each is a lexical
/// gate: it changes which token the scanner emits, never how a value is
/// materialized (deferred).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StringLiteralSyntax {
    /// Accept PostgreSQL `E'...'` escape string constants.
    pub escape_strings: bool,
    /// Accept PostgreSQL `$tag$...$tag$` dollar-quoted string constants.
    pub dollar_quoted_strings: bool,
    /// Accept `N'...'` national-character string constants (T-SQL; PostgreSQL
    /// sugar). Lexes as a `String` token spanning the `N` prefix.
    pub national_strings: bool,
    /// Lex `"..."` as a string constant rather than a quoted identifier (MySQL
    /// without `ANSI_QUOTES`). Takes precedence over identifier quoting for `"`,
    /// so a preset enabling this must not also list `"` in `identifier_quotes`.
    pub double_quoted_strings: bool,
    /// Honour C-style backslash escapes inside `'...'` (and double-quoted) strings
    /// for termination (MySQL default; not T-SQL), so `'a\'b'` is one token. The
    /// escape is recognised lexically only; value materialization stays deferred.
    ///
    /// This bool collapses what PostgreSQL historically made a *tri-state*: legacy
    /// `standard_conforming_strings = off` made a plain `'…'` backslash-aware too — a
    /// third mode distinct from both the modern-off default and MySQL's always-on. The
    /// bool covers every *shipped* preset (modern PostgreSQL off, MySQL on); the
    /// version-varying third mode belongs to the deferred per-release version presets
    /// (`prod-dialect-release-version-presets`), which own version-varying knobs.
    pub backslash_escapes: bool,
    /// Accept `U&'...'` Unicode-escape string constants (SQL standard, PostgreSQL).
    pub unicode_strings: bool,
    /// Accept `B'...'` / `X'...'` bit-string constants (SQL standard, PostgreSQL).
    /// Lexes as a `String` token spanning the `B`/`X` prefix; the binary-vs-hex
    /// radix and the digit validation are recovered later, so a
    /// malformed body like `X'1FG'` still lexes (PostgreSQL defers the check too).
    pub bit_string_literals: bool,
    /// Accept SQLite/MySQL `x'53514C'` / `X'53514c'` hexadecimal byte-string literals
    /// (SQLite's BLOB literal; MySQL's hexadecimal literal). Only the `x`/`X` hex marker
    /// — the `B'…'` binary form is [`bit_string_literals`](Self::bit_string_literals),
    /// not this — and the quote must abut the marker (like `E'`/`B'`).
    ///
    /// Unlike a PostgreSQL/DuckDB deferred bit-string, the body is validated **eagerly at
    /// lex time**: it must be an *even* number of ASCII hex digits (each pair is one
    /// byte), so an odd-length (`x'ABC'`, `x'0'`) or non-hex (`x'XY'`) body is a
    /// tokenize-time syntax error — the rule both engines enforce (probed: SQLite
    /// "unrecognized token", MySQL `ER_PARSE_ERROR`). The empty body `x''` is a valid
    /// zero-byte blob. It lexes as a `String` token spanning the marker and classifies as
    /// a hex [`BitString`](crate::ast::LiteralKind::BitString) — the same canonical
    /// hex-digit-string shape as `X'…'`, differing only in this eager lex-time bound; the
    /// spelling round-trips from the span and a consumer reads the bytes via
    /// [`as_bit_text`](crate::ast::Literal::as_bit_text).
    ///
    /// Disjoint from `bit_string_literals` on the shared `x`/`X` marker by scan
    /// precedence: where both are on (MySQL), the eager hex arm claims `x`/`X` and the
    /// deferred bit arm keeps `B`/`b`; where only `bit_string_literals` is on
    /// (PostgreSQL) `X'…'` stays the deferred bit-string (odd-length allowed).
    pub blob_literals: bool,
    /// Accept MySQL `_charset'...'` character-set introducers — an `_`-prefixed
    /// charset name abutting a string constant (`_utf8mb4'x'`, `_latin1'x'`). Like
    /// the `N'...'` national prefix this lexes as one `String` token spanning the
    /// introducer; the charset name is a surface tag that rides the span and is
    /// recovered on demand, and the value materialises with the
    /// introducer stripped. The abutting `'` is required — a bare `_name` with no
    /// quote stays an ordinary identifier — so with this off `_utf8'x'` lexes as the
    /// identifier `_utf8` then a string (the ANSI/PostgreSQL behaviour).
    pub charset_introducers: bool,
    /// Concatenate adjacent string literals separated by whitespace with *no* newline
    /// (`'a' 'b'` on one line → `'ab'`), MySQL's rule. The SQL standard (and the default
    /// here) requires a newline in the separator, so `'a' 'b'` same-line is
    /// otherwise an adjacency error. Not a lexical gate — each literal still tokenizes
    /// separately; the parser reads this at the continuation-gap classification point and
    /// the AST materializer walks the folded span the same way it walks the newline form.
    /// A comment in the gap still blocks concatenation under either rule.
    pub same_line_adjacent_concat: bool,
}

/// Dialect-owned numeric literal syntax extensions.
///
/// Standard decimal/scientific numbers are always part of the baseline. These
/// flags cover non-ANSI numeric forms whose recognition is an explicit dialect
/// data decision. Each is lexical: it widens which numbers the scanner
/// accepts as one token; value materialization stays deferred. The
/// radix and separator forms below are PostgreSQL 14+ additions (also in T-SQL /
/// MySQL for hex), so a release-pinned PostgreSQL preset gates them by version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumericLiteralSyntax {
    /// Accept `0x..` hexadecimal integer literals (T-SQL, MySQL, PostgreSQL 14+).
    pub hex_integers: bool,
    /// Accept `0o..` octal integer literals (PostgreSQL 14+).
    pub octal_integers: bool,
    /// Accept `0b..` binary integer literals (MySQL, PostgreSQL 14+).
    pub binary_integers: bool,
    /// Accept `_` digit-group separators between digits (PostgreSQL 14+), e.g.
    /// `1_500_000`. Recognised lexically only when a digit follows the `_`. When
    /// [`reject_trailing_junk`](Self::reject_trailing_junk) is off the placement is
    /// loose (a `_` is a separator wherever a digit follows it); when it is on the
    /// placement is strict (a decimal `_` must sit *between* two digits, matching PG's
    /// `{decdigit}(_?{decdigit})*`), so a leading-in-fraction/trailing/doubled `_`
    /// stops the number and surfaces as trailing junk. Whether a `_` may additionally
    /// *lead* a radix body (`0x_1F`) is a separate axis
    /// ([`radix_leading_underscore`](Self::radix_leading_underscore)), because PG and
    /// SQLite disagree on it.
    pub underscore_separators: bool,
    /// Accept a leading `_` immediately after a radix marker, before the first radix
    /// digit (`0x_1F`, `0b_101`) — PostgreSQL's `0[xX](_?{hexdigit})+` grammar, which
    /// admits the underscore ahead of the first digit. Requires
    /// [`underscore_separators`](Self::underscore_separators); an interior radix `_`
    /// (`0x1_F`) rides that flag alone and is unaffected by this one.
    ///
    /// Its own axis rather than a rider on [`reject_trailing_junk`](Self::reject_trailing_junk)
    /// because the engines that reject trailing junk still split on it: PostgreSQL accepts
    /// `0x_1F` (probed) but SQLite rejects it — SQLite's radix grammar is
    /// `0[xX]{hexdigit}(_?{hexdigit})*`, requiring a digit before the first `_`. With this
    /// off a leading-underscore radix body does not open, so `0x_1F` falls back to the
    /// bare `0` plus a `x_1F` word/alias (loose dialects) or a trailing-junk reject
    /// (strict ones), exactly as before this axis existed.
    pub radix_leading_underscore: bool,
    /// Reject an identifier-start character abutting a numeric literal — PostgreSQL's
    /// "trailing junk after numeric literal" scanner error (`123abc`, `1x`, `0.0e`,
    /// `100_`), plus the bad-radix (`0x`, `0b0x`) and misplaced-separator (`100__000`,
    /// `1_000._5`) forms that decompose to it. A number is a maximal-munch lexeme, so
    /// with this on anything identifier-ish immediately following one is a lexer error,
    /// not a new token; it also switches [`underscore_separators`] to strict placement.
    ///
    /// Dialect-gated because the reject is not universal: PostgreSQL and SQLite reject
    /// these (both probed against their engines), but **DuckDB accepts them all** (it
    /// re-reads `123abc` as `123` aliased) and MySQL only rejects the integer/radix forms
    /// — so a dialect that lexes its numerics loosely (DuckDB/MySQL) leaves this off and
    /// the trailing text falls through to the ordinary word/alias scan, exactly as before.
    ///
    /// [`underscore_separators`]: Self::underscore_separators
    pub reject_trailing_junk: bool,
    /// Accept `$1234.56` / `$.5` T-SQL money literals (the `$` currency sigil prefixes
    /// a decimal). Off in ANSI/PostgreSQL: PostgreSQL spells `$` as a positional
    /// parameter (`$1`) or a dollar-quote (`$tag$`), never money, so the three
    /// `$`-prefix lexer forms are dialect-disjoint. Lexes as a `Number` token spanning
    /// the `$`; the money type rides the literal kind and the sigil is stripped at the
    /// accessor, like the `B`/`X` bit-string prefix.
    pub money_literals: bool,
}

/// Dialect-owned prepared-statement parameter placeholder syntax.
///
/// SQL has no single standard placeholder spelling, so which forms a dialect
/// accepts is explicit dialect data, not a parser-side type check. Each
/// flag is a lexical gate: it decides whether the scanner recognises that sigil as
/// a parameter [`TokenKind`](crate::ast) rather than a stray byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParameterSyntax {
    /// Accept PostgreSQL positional `$1`, `$2`, … (`$` + ASCII digits). Disjoint
    /// from dollar-quoting: `$` + digit is a parameter, `$` + tag-start/`$` opens a
    /// dollar-quoted string, so both forms can be enabled together.
    pub positional_dollar: bool,
    /// Preserve a `$` positional parameter whose digit run exceeds `u32`. PostgreSQL's
    /// scanner accepts these spellings and narrows them into its signed `ParamRef.number`;
    /// dialects that range-check their parameter indices leave this off.
    pub positional_dollar_large: bool,
    /// Accept anonymous positional `?` placeholders (ODBC/JDBC).
    pub anonymous_question: bool,
    /// Accept colon-named `:name` placeholders (Oracle, SQLite, JDBC/psycopg). The
    /// sigil must abut an identifier-start byte, which is what keeps `:name` free of
    /// the two other `:` meanings: `::` stays the typecast (its second `:` is not an
    /// identifier byte) and a lone `:` before a non-identifier byte stays the
    /// array-slice separator. (A dialect that *also* sliced arrays with bare
    /// identifier bounds — `a[x:y]` — or wrote semi-structured paths as `a:b` would lex
    /// `:y`/`:b` as a parameter; that pairing is the tracked
    /// [`LexicalConflict::ColonParameterVersusSliceBound`], since no real dialect combines
    /// `:name` with those spellings.)
    pub named_colon: bool,
    /// Accept at-sign-named `@name` placeholders (T-SQL parameters/local variables).
    /// The sigil must abut an identifier-start byte, so the system-variable `@@name`
    /// form is left unclaimed for `prod-token-identifier-prefix-tokens` (the second
    /// `@` is not an identifier byte, so this never grabs `@@`).
    ///
    /// Mutually exclusive with [`SessionVariableSyntax::user_variables`]: both claim
    /// the `@name` trigger (one as a placeholder, one as a user-variable read), so a
    /// feature set enabling both is a [`LexicalConflict`]. `@name`-as-parameter
    /// (T-SQL) and `@name`-as-user-variable (MySQL) are the same surface with
    /// different meaning, so a dialect picks one.
    pub named_at: bool,
    /// Accept dollar-named `$name` placeholders (SQLite). The sigil must abut an
    /// identifier-start byte — `$` + a *digit* stays the PostgreSQL positional
    /// `$1` ([`positional_dollar`](Self::positional_dollar)) and `$` + a
    /// non-identifier byte a stray/money form — so this is follow-set-disjoint from
    /// both `$`-digit forms and can be enabled alongside them.
    ///
    /// Contends only with [`StringLiteralSyntax::dollar_quoted_strings`]: a
    /// `$tag$…$tag$` opener also leads with `$` + a tag-start byte (the same class as
    /// identifier-start), so enabling both is a
    /// [`LexicalConflict::NamedDollarParameterVersusDollarQuotedString`]. SQLite has
    /// no dollar-quoting, so the two never meet in a shipped preset.
    pub named_dollar: bool,
    /// Accept SQLite numbered `?NNN` positional parameters (`?1`, `?123`) — the `?` sigil
    /// abutting an ASCII-digit run — on top of the bare anonymous `?`
    /// ([`anonymous_question`](Self::anonymous_question)). Follow-set-disjoint from the
    /// anonymous form by the digit (a `?` with no digit stays anonymous), exactly as
    /// PostgreSQL's `$1` splits from `$name`. The number is a maximal digit run, so a
    /// trailing identifier is a separate token (`?1abc` is `?1` then the alias `abc`;
    /// engine-measured). SQLite range-restricts the index to `1..=32766`
    /// (`SQLITE_MAX_VARIABLE_NUMBER`): `?0`, `?32767`, and an overflowing run are parse
    /// rejects ("variable number must be between ?1 and ?32766"; probed), enforced when the
    /// numbered token is materialised. On for SQLite / Lenient, off elsewhere.
    pub numbered_question: bool,
}

/// Dialect-owned MySQL session-variable syntax.
///
/// MySQL exposes user-defined `@name` variables and server `@@[scope.]name` system
/// variables as *value expressions* — distinct from a prepared-statement placeholder
/// ([`ParameterSyntax`]), which is a hole bound at execute time. Each flag is a
/// lexical gate: it decides whether the scanner recognises that sigil as a variable
/// token ([`Expr::SessionVariable`](crate::ast::Expr::SessionVariable)) rather than a
/// stray byte. The two forms are independent — a dialect can accept `@@sysvar` without
/// `@uservar` — and are lookahead-disjoint (`@@` needs a second `@`, `@name` an
/// identifier-start), so they never contend with each other.
///
/// [`Expr::SessionVariable`]: crate::ast
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionVariableSyntax {
    /// Accept `@name` user-defined session variables (MySQL). The sigil must abut an
    /// identifier-start byte. Mutually exclusive with
    /// [`ParameterSyntax::named_at`]: both claim `@name`, so enabling both is a
    /// [`LexicalConflict::AtNameParameterVersusUserVariable`].
    pub user_variables: bool,
    /// Accept `@@name` / `@@global.name` / `@@session.name` system variables (MySQL).
    /// The `@@` must abut an identifier-start byte; a scoped form folds the optional
    /// `global.`/`session.` prefix into the same token. Never contends with `named_at`
    /// or `user_variables` — the second `@` keeps `@@` lookahead-disjoint from the
    /// single-`@` forms.
    pub system_variables: bool,
    /// Accept the MySQL `SET` **variable-assignment** statement grammar and its `:=`
    /// assignment operator (MySQL).
    ///
    /// Two facets of one behaviour that ship together in MySQL and nowhere else:
    /// * *Parser* — a `SET` becomes a comma-separated list of heterogeneous assignments
    ///   ([`SessionStatement::SetVariables`](crate::ast::SessionStatement::SetVariables)):
    ///   `[GLOBAL|SESSION|LOCAL|PERSIST|PERSIST_ONLY] <var> {= | :=} <expr>`, the
    ///   `@@[scope.]<var>` spellings, user variables `@v {= | :=} <expr>`, and
    ///   `SET {CHARACTER SET | CHARSET} …`. Each value is a *full expression*, unlike the
    ///   generic PostgreSQL `SET`'s restricted literal/bareword value list, so the two
    ///   grammars are distinct statements rather than one widened form.
    /// * *Lexer* — `:=` lexes to the `ColonEquals` operator token (MySQL's `SET_VAR`), the
    ///   same token PostgreSQL's deprecated named-argument separator produces; the two never
    ///   coexist in a shipped preset, so the shared token is unambiguous per dialect.
    ///
    /// The two facets sit on independent axes, which is why this flag is a documented
    /// exemption from the [`feature_dependencies`](FeatureSet::feature_dependencies) registry
    /// rather than a [`FeatureDependencyViolation`] variant. The *parser* facet is unreachable
    /// without [`show_syntax.session_statements`](ShowSyntax::session_statements) — the leader
    /// that dispatches every `SET`/`RESET`/`SHOW`, so with it off no `SET` form parses at all
    /// (measured: `SET x = 1` is rejected as an unknown statement leader) — a genuine
    /// cross-axis grammar dependency. The *lexer* facet, however, fires whether or not that
    /// leader is on: with `session_statements` off, `:=` still munches to one `ColonEquals`
    /// token (measured), so the flag is **not** inert without its parser-side base. A registry
    /// whose contract is inertness cannot own a flag that keeps changing tokenization while
    /// its base is off, so the dependency is recorded here instead. See the exemption note on
    /// [`feature_dependencies`](FeatureSet::feature_dependencies).
    ///
    /// This is a *route* flag on the `SET` head — when on it dispatches the MySQL grammar before
    /// the standard `SET TIME ZONE` / `SET SESSION AUTHORIZATION` forms are read (see the parser's
    /// `parse_set`) — but unlike the
    /// [`access_control_account_grants`](AccessControlSyntax::access_control_account_grants) route
    /// it carries *no* [`GrammarConflict`] variant: the grammar it displaces is unconditional base
    /// grammar with no rival feature flag, so there is no independently-expressible both-on state,
    /// and MySQL (a shipped preset) already exercises the route deterministically. Registering it
    /// would require promoting the standard `SET` config grammar to its own flag or converting this
    /// field to an enum axis — see the [`GrammarConflict`] enum doc's route-flag discussion.
    ///
    /// **Shared `:=` claim** with [`CallSyntax::named_argument`] (PostgreSQL named args): both
    /// enable `:=` lexing; shipped presets never arm both, so the token stays unambiguous.
    pub variable_assignment: bool,
}

/// Policy for non-ASCII code points in an *unquoted* identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonAsciiIdentifierSyntax {
    /// Only Unicode letters may start an identifier; Unicode letters and numbers may
    /// continue it.
    UnicodeAlphanumeric,
    /// Every non-ASCII code point may start or continue an identifier.
    Any,
}

/// Dialect-owned policy for which characters form an *unquoted* identifier.
///
/// The identifier-start and identifier-continue classes are an explicit,
/// Unicode-aware policy, not an ad-hoc byte rule. ANSI starts with a Unicode *letter*
/// (`char::is_alphabetic`) or `_`, and continues with a letter, a Unicode *digit*
/// (`char::is_alphanumeric`), `_`, or — where [`dollar_in_identifiers`] is set — `$`.
/// PostgreSQL, MySQL, and SQLite instead admit every non-ASCII code point. Quoted
/// identifiers bypass the policy entirely (any character may be
/// quoted), and no identifier *normalization* (NFC/NFKC) is performed — characters
/// are compared as written; case folding for identity is the separate
/// [`identifier_casing`] concern.
///
/// Only the dialect-*variable* part lives here. The ASCII letter/digit/`_` classes
/// are the shared byte-class table (so a dialect can still add ASCII identifier bytes
/// like T-SQL `#`/`@` through [`byte_classes`]); [`non_ascii`](IdentifierSyntax::non_ascii)
/// selects the non-ASCII rule.
///
/// [`dollar_in_identifiers`]: IdentifierSyntax::dollar_in_identifiers
/// [`identifier_casing`]: FeatureSet::identifier_casing
/// [`byte_classes`]: FeatureSet::byte_classes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentifierSyntax {
    /// Select the non-ASCII start/continue policy. ASCII characters remain governed by
    /// [`FeatureSet::byte_classes`].
    pub non_ascii: NonAsciiIdentifierSyntax,
    /// Accept `$` as an identifier-*continue* character (`foo$bar`), a PostgreSQL /
    /// Oracle extension that strict ANSI forbids. `$` never *starts* an identifier (a
    /// leading `$` is a parameter or dollar-quote), and it is never a dollar-quote
    /// *tag* character (there `$` is the delimiter).
    pub dollar_in_identifiers: bool,
    /// Accept a single-quoted string literal in identifier positions *beyond* aliases —
    /// SQLite's misfeature where a `'name'` string is read as a name wherever the grammar
    /// wants a `nm` (identifier). Corpus-admitted for the two positions the SQLite
    /// test-suite surfaces: a DML/DDL *relation-target* name (`DELETE FROM 'table1'`, and
    /// so `CREATE TABLE 'name'` / `DROP TABLE 'n'` / `INSERT INTO 'n'` through the shared
    /// target path) and a `PRIMARY KEY`/`UNIQUE` *table-constraint column-name* list
    /// (`PRIMARY KEY('a')`, `UNIQUE('b')`). Each admitted position is *position-driven*:
    /// a bare string there is never a valid literal in standard SQL, so reading it as the
    /// name is unambiguous — no lexical or grammar conflict (the tokenizer still lexes
    /// `'x'` to a `String`; a single parser position reads it, shadowing no rival feature).
    ///
    /// SQLite's full leniency is broader (a string is also admitted as a column-def name,
    /// a `CAST` type name, a qualified column-ref qualifier, a `CREATE VIEW`/`TRIGGER`
    /// target, …); those positions carry no corpus gap and stay out of scope. A string
    /// *function* name is a SQLite syntax error (`SELECT 'f'(1)`), so the widening is
    /// deliberately confined to the two name positions rather than folded into the shared
    /// object-name grammar. The folded name records [`QuoteStyle::Single`] (or `Double`
    /// under a no-`ANSI_QUOTES` mode) so the quotes round-trip, reusing the projection
    /// alias's string round-trip. On for SQLite / Lenient, off elsewhere — every other
    /// dialect syntax-rejects the form, so the over-acceptance risk is zero (flag off).
    ///
    /// [`QuoteStyle::Single`]: crate::ast::QuoteStyle::Single
    pub string_literal_identifiers: bool,
    /// Accept a single-part Sconst spelling of a relation / table name — DuckDB's
    /// `FROM 't'` / `FROM ''` / `FROM E't'` / `FROM $$t$$` (engine-measured on
    /// libduckdb 1.5.4). The string is a *single-part* name only: a dotted string name
    /// (`FROM 'a'.'b'`) is a parser reject, matching DuckDB. Distinct from
    /// [`string_literal_identifiers`](Self::string_literal_identifiers) (SQLite's broader
    /// multi-part `'schema'.'table'` misfeature on relation *targets*). On for DuckDB and
    /// the permissive superset; off elsewhere.
    pub string_literal_table_names: bool,
    /// Accept a *zero-length* delimited (quoted) identifier — the empty backtick
    /// `` `` ``, the empty bracket `[]`, and the empty double-quote `""` — which SQL's
    /// `<delimited identifier body>` and PostgreSQL/MySQL both forbid at scan time. SQLite
    /// alone among the shipped engines admits an empty quoted identifier in every quote
    /// style (engine-measured on rusqlite: `` SELECT `` ``, `SELECT []`, and `SELECT ""`
    /// all prepare — the `""` via SQLite's double-quote-to-string fallback, the others as
    /// zero-length names). When off, the tokenizer rejects a zero-length quoted identifier
    /// the moment the close abuts the open (the pre-existing, universal
    /// `ZeroLengthDelimitedIdentifier` scan reject). On for SQLite / Lenient, off elsewhere.
    pub empty_quoted_identifiers: bool,
}

/// Dialect-owned table-expression syntax extensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableExpressionSyntax {
    /// Accept PostgreSQL `ONLY table` / `ONLY (table)` inheritance suppression.
    pub only: bool,
    /// Accept `TABLESAMPLE method(args...) [REPEATABLE (...)]`.
    pub table_sample: bool,
    /// Accept joined tables as parenthesized table factors (`FROM (a JOIN b) JOIN c`).
    /// On in every shipped preset — the standard join grouping — but stays gateable, so
    /// a stricter dialect that forbids parenthesizing a join leaves it off and the form
    /// then surfaces as a clean parse error.
    pub parenthesized_joins: bool,
    /// Accept `alias(column, ...)` derived-column lists after table factors. On in every
    /// shipped preset (the SQL-standard correlation-name column list), but stays gateable
    /// so a dialect without the form can reject it as trailing input.
    pub table_alias_column_lists: bool,
    /// Accept PostgreSQL `JOIN ... USING (...) AS alias`.
    pub join_using_alias: bool,
    /// Accept MySQL index hints (`{USE|FORCE|IGNORE} {INDEX|KEY} [FOR …] (…)`) as a
    /// table-factor tail after the alias. MySQL-only, so it rides
    /// [`TableFactor::Table::index_hints`](crate::ast::TableFactor); when off the hint
    /// keywords are left to the identifier grammar and the construct is a clean parse
    /// divergence. On for MySQL / Lenient, off elsewhere.
    pub index_hints: bool,
    /// Accept MSSQL / T-SQL `WITH ( <hint>, … )` table hints (`WITH (NOLOCK)`,
    /// `WITH (INDEX(ix), FORCESEEK)`) as a table-factor tail after the tablesample
    /// clause. T-SQL-only, so it rides
    /// [`TableFactor::Table::table_hints`](crate::ast::TableFactor); when off the
    /// trailing `WITH` is left unconsumed, so under ANSI/PostgreSQL — where `WITH`
    /// introduces only a leading CTE clause at statement start — the construct is a
    /// clean parse divergence. A separate axis from [`index_hints`](TableExpressionSyntax::index_hints):
    /// a different dialect (T-SQL vs MySQL) and a different grammar position. On for
    /// MSSQL / Lenient, off elsewhere.
    pub table_hints: bool,
    /// Accept MySQL explicit partition selection (`PARTITION (p0, p1)`) as a
    /// table-factor tail between the table name and the alias. MySQL-only, so it rides
    /// [`TableFactor::Table::partition`](crate::ast::TableFactor); when off the
    /// `PARTITION` keyword is left unconsumed and the construct is a clean parse
    /// divergence. On for MySQL / Lenient, off elsewhere.
    pub partition_selection: bool,
    /// Accept a column-list alias (`AS y(a, b)`) on a *base table* factor
    /// (`FROM t AS y(a, b)`). On for ANSI/PostgreSQL/SQLite/DuckDB/Lenient. MySQL admits a
    /// column-list alias only on a *derived* table / subquery / table function
    /// (`FROM (SELECT …) AS c(x)` parses on mysql:8, only bind-failing) and rejects one on
    /// a base table (`FROM t AS y(a, b)` is an `ER_PARSE_ERROR` on mysql:8), so it is off
    /// there; the `(` after the base-table alias name is then a clean parse error. The
    /// broader [`table_alias_column_lists`](TableExpressionSyntax::table_alias_column_lists) gate governs
    /// whether the dialect admits column-list aliases *at all* (for the derived/function
    /// positions); this one further restricts the base-table position — the base-vs-derived
    /// split MySQL draws.
    pub base_table_alias_column_lists: bool,
    /// Accept a single-quoted string literal in table-alias position — both the
    /// correlation name after an explicit `AS` (`FROM integers AS 't'`) and each entry
    /// of the alias column list (`FROM integers AS 't'('k')` / `FROM integers t('k')`),
    /// reusing the projection alias's string-literal round-trip. DuckDB admits this only
    /// after `AS`: a bare `FROM integers 't'` is an engine reject (probed on 1.5.4),
    /// preserved by the alias site's leading-string guard.
    ///
    /// Deliberately *separate* from [`SelectSyntax::alias_string_literals`], which gates
    /// the string spelling in *projection* position: the two profiles diverge. MySQL
    /// accepts a string *column* alias (`SELECT 1 AS 'x'`) but rejects a string *table*
    /// alias (`FROM t AS 't'` — engine-measured-rejected on mysql:8), so folding both
    /// onto one flag would make MySQL over-accept the table form. On for DuckDb /
    /// Lenient, off elsewhere (including MySQL).
    pub string_literal_aliases: bool,
    /// Accept a correlation alias on a *parenthesized joined table*
    /// (`FROM (a CROSS JOIN b) AS x`). On for ANSI/PostgreSQL/SQLite/DuckDB/Lenient. MySQL
    /// admits a parenthesized join ([`parenthesized_joins`](TableExpressionSyntax::parenthesized_joins)) but
    /// rejects an alias on it (`(a CROSS JOIN b) AS x` is an `ER_PARSE_ERROR` on mysql:8,
    /// while the bare `(a CROSS JOIN b)` and a derived-table `(SELECT …) AS x` both parse),
    /// so it is off there and the trailing alias surfaces as a clean parse error. Only the
    /// *joined-table* parenthesization is governed; a derived subquery's alias rides the
    /// always-accepted derived-table path.
    pub aliased_parenthesized_join: bool,
    /// Treat a bare (`AS`-less) *table* correlation alias as a `BareColLabel` — routed to
    /// [`FeatureSet::reserved_bare_alias`] — instead of the default `ColId`
    /// ([`FeatureSet::reserved_column_name`]). Off for every dialect except SQLite, where
    /// the bare alias is the narrow `ids ::= ID|STRING` grammar class (not the `nm` name
    /// class): the seven `JOIN_KW` keywords are admissible as a *table name* (`FROM cross`)
    /// yet reserved as a *bare alias*, so `FROM t cross JOIN u` must keep `cross` for the
    /// join grammar rather than read it as `t`'s alias. Routing the bare-table-alias gate
    /// to the bare-alias set (which reserves the JOIN keywords) while the table-name gate
    /// stays on the permissive `ColId` set is what makes the two positions diverge — the
    /// table-alias twin of [`SelectSyntax::as_alias_rejects_reserved`]. The explicit `AS`
    /// table alias keeps the `ColId` set (SQLite's `AS nm` admits the JOIN keywords).
    pub bare_table_alias_is_bare_label: bool,
    /// Accept a table version / time-travel modifier on a base table, written between the
    /// table name and the alias: BigQuery/MSSQL `FOR SYSTEM_TIME …`, MSSQL's five temporal
    /// forms (`AS OF`, `FROM … TO`, `BETWEEN … AND`, `CONTAINED IN`, `ALL`), and
    /// Databricks/Delta `VERSION`/`TIMESTAMP AS OF`. Rides
    /// [`TableFactor::Table::version`](crate::ast::TableFactor); when off the clause keyword
    /// is left unconsumed, so a query-level `FOR` (row locking, MSSQL `FOR XML`) still parses
    /// — the two `FOR` surfaces are position-partitioned, this one at the table factor and
    /// the query-level ones after the whole `FROM`/`WHERE`. On for BigQuery / MSSQL /
    /// Databricks / Lenient, off elsewhere.
    pub table_version: bool,
    /// Accept a PartiQL / SUPER JSON path navigating into a semi-structured column at the
    /// table-source position, attached directly to the table name (`FROM src[0].a`,
    /// `FROM src[0].a[1].b`). Redshift's SUPER navigation and Snowflake's PartiQL access
    /// (sqlparser-rs's `supports_partiql`). Rides
    /// [`TableFactor::Table::json_path`](crate::ast::TableFactor); the path is entered only
    /// by a `[` immediately after the name (a bracket index root, then `.key` / `[index]`
    /// suffixes), so a dotted `FROM src.a.b` stays a compound relation name. When off the
    /// `[` is left unconsumed and the construct is a clean parse divergence. Because the
    /// entry trigger is the `[` tokenizer trigger, this shares the
    /// [`BracketIdentifierVersusArraySyntax`](crate::dialect::LexicalConflict) hazard: a
    /// dialect with a `[` identifier quote cannot also enable it. On for Snowflake /
    /// Redshift, off elsewhere — including Lenient, whose `[` bracket identifier quote claims
    /// the trigger (the same reason Lenient keeps `subscript` / `collection_literals` off).
    pub table_json_path: bool,
    /// Accept a SQLite `INDEXED BY <index>` / `NOT INDEXED` index directive on a base table,
    /// written after the table name and its optional alias (`FROM t AS e INDEXED BY ix`,
    /// `FROM t NOT INDEXED`). Rides
    /// [`TableFactor::Table::indexed_by`](crate::ast::TableFactor). When on, a bare
    /// `INDEXED` at the base-table alias position is declined as a correlation alias so the
    /// directive is reachable (the `CONNECT BY` clause-decline precedent), which is what
    /// makes SQLite reject a bare `FROM t indexed` while still admitting `indexed` as an
    /// identifier everywhere else (`SELECT indexed`, `t AS indexed`, `indexed INT`). A
    /// separate axis from MySQL [`index_hints`](TableExpressionSyntax::index_hints): a
    /// different dialect, grammar, and cardinality. On for SQLite, off elsewhere — including
    /// Lenient, whose maximal-accept goal keeps a bare `FROM t indexed` an ordinary alias
    /// (the directive-versus-bare-alias readings are mutually exclusive given the keyword's
    /// one-position semantics, and Lenient prefers the more permissive bare-alias reading).
    pub indexed_by: bool,
    /// Accept DuckDB's **table-factor** prefix colon alias — `<alias> : <relation>` at a
    /// `FROM` head (`FROM b : a` → relation `a` aliased `b`). Projection-position twin is
    /// [`SelectSyntax::prefix_colon_alias`]. Same pure-`AS` sugar and
    /// [`GrammarConflict::PrefixColonAliasVersusSemiStructuredAccess`] hazard when either
    /// position is on with `semi_structured_access`. On for DuckDB / Lenient; off elsewhere.
    pub prefix_colon_alias: bool,
}

/// Dialect-owned join-operator and recursive-query relation-composition syntax
/// accepted by the parser.
///
/// The join-family flags — the join operators and their side/qualifier variants beyond
/// the always-available inner/left/right/cross joins — together with the recursive-query
/// structural clauses that compose relations. Split out of [`TableExpressionSyntax`] at
/// its 16-field line as the relation-composition axis, distinct from the table-factor and
/// factor-tail/alias cores. Each flag is a grammar gate: when off the keyword is left to
/// the identifier grammar or surfaces as a clean parse error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JoinSyntax {
    /// Accept *stacked* join qualifiers — the PostgreSQL right-nesting where two joins
    /// precede their `ON`/`USING` constraints and the nearest constraint closes the
    /// innermost join: `a JOIN b JOIN c ON p ON q` reads as `a JOIN (b JOIN c ON p) ON q`
    /// (PostgreSQL `table_ref: … | joined_table` right-recursion). On for ANSI/PostgreSQL/
    /// MySQL/DuckDB/Lenient. SQLite's `join-clause` is flat — each `join-operator` takes
    /// exactly one immediately-following constraint — so it is off; the right operand is
    /// not extended past the first constraint, and a second stacked `ON`/`USING`
    /// (`… USING (id) USING (id)`) is then left unconsumed and surfaces as the syntax
    /// error SQLite reports (engine-measured via rusqlite). The common `a JOIN b ON x
    /// JOIN c ON y` — each constraint right after its own join — needs no nesting and is
    /// unaffected either way.
    pub stacked_join_qualifiers: bool,
    /// Accept the `FULL [OUTER] JOIN` bilateral outer join. On for ANSI/PostgreSQL/SQLite/
    /// DuckDB/Lenient. MySQL has no `FULL` join (it offers only `LEFT`/`RIGHT` outer joins),
    /// so it is off there: the `FULL` join-side keyword is not consumed, and an
    /// already-aliased factor followed by `FULL [OUTER] JOIN` (`a x FULL OUTER JOIN b`) is
    /// then a clean parse error — exactly what MySQL reports (engine-measured-rejected on
    /// mysql:8). Because `FULL` is a non-reserved word in MySQL, a *bare* `a full JOIN b`
    /// still reads `full` as the factor's alias (grabbed before the join loop), matching the
    /// engine — this flag only governs the join-side reading, never the alias.
    pub full_outer_join: bool,
    /// Accept `NATURAL CROSS JOIN` — `NATURAL` prefixing the `CROSS` join keyword.
    /// SQLite's `join-operator` validates its up-to-three keywords post-hoc and admits
    /// `NATURAL` before any join type, `CROSS` included; PostgreSQL and DuckDB both
    /// parse-reject it (engine-probed). Because `CROSS JOIN` is the optimizer-hint
    /// spelling of `INNER JOIN` and `NATURAL` supplies the shared-column constraint,
    /// `NATURAL CROSS JOIN` is semantically a natural inner join — engine-probed on
    /// rusqlite: it yields the shared-column equijoin's row/column shape, not the cross
    /// product — so the parser normalizes it into the canonical
    /// [`JoinOperator::Inner`](crate::ast::JoinOperator) + [`JoinConstraint::Natural`](crate::ast::JoinConstraint)
    /// shape (the `NATURAL INNER` precedent, where the redundant side keyword is likewise
    /// dropped), rendering back as `NATURAL JOIN`. No new AST field is needed: the
    /// round-trip oracle compares structure, not spelling, so the elided `CROSS` word
    /// re-parses to the same AST. On for SQLite / Lenient, off elsewhere; when off,
    /// `NATURAL CROSS` fails the `NATURAL` arm's mandatory `JOIN` and rejects unchanged.
    pub natural_cross_join: bool,
    /// Accept MySQL's `STRAIGHT_JOIN` join-order hint, in both of its surfaces: the
    /// join operator `a STRAIGHT_JOIN b [ON ...]` and the `SELECT STRAIGHT_JOIN ...`
    /// modifier. One flag gates both grammar points because they are a single dialect
    /// unit — MySQL always admits the modifier wherever it admits the join keyword
    /// (mirroring how [`CreateTableClauseSyntax::table_options`] gates the trailing options
    /// and the `AUTO_INCREMENT` attribute together). Both fold into the
    /// canonical inner-join shape / `Select` flag with a surface tag, never a new node.
    /// When off, `STRAIGHT_JOIN` is left to the identifier grammar (a non-reserved word
    /// under ANSI/PostgreSQL), so the MySQL construct is a clean parse divergence.
    pub straight_join: bool,
    /// Accept DuckDB's `ASOF [INNER|LEFT|RIGHT|FULL [OUTER]] JOIN` inexact-match
    /// temporal join ([`JoinOperator::AsOf`](crate::ast::JoinOperator)). When off,
    /// `ASOF` is left to the identifier grammar (a non-reserved word under
    /// ANSI/PostgreSQL) — and because the next word is `JOIN`, the text still parses
    /// there, as an aliased *plain* join (a different-tree reading, unlike
    /// `STRAIGHT_JOIN`'s leftover-input reject). The flag alone is not enough for the
    /// DuckDB meaning on a bare table factor — the word must also be in
    /// [`FeatureSet::reserved_column_name`] or the factor's alias swallows it first
    /// (the DuckDb preset reserves it; `LENIENT` keeps the ANSI reserved model, so
    /// there the join parses only after an explicit alias). On for DuckDb / Lenient,
    /// off elsewhere.
    pub asof_join: bool,
    /// Accept DuckDB's `POSITIONAL JOIN` row-position pairing join
    /// ([`JoinOperator::Positional`](crate::ast::JoinOperator)), which takes no
    /// `ON`/`USING` constraint and no side keyword. The same reserved-word
    /// interaction as [`asof_join`](JoinSyntax::asof_join) applies. On for DuckDb /
    /// Lenient, off elsewhere.
    pub positional_join: bool,
    /// Accept DuckDB's `SEMI JOIN` / `ANTI JOIN` semi-/anti-join operators
    /// ([`JoinOperator::Semi`](crate::ast::JoinOperator)/[`Anti`](crate::ast::JoinOperator::Anti)),
    /// including their `NATURAL` and `ASOF` compositions (`NATURAL SEMI JOIN`,
    /// `ASOF ANTI JOIN`). One flag gates both because `SEMI` and `ANTI` are the same
    /// grammar production (a `join_type` keyword taking an `ON`/`USING` constraint) that
    /// DuckDB only ever ships together — the paired-flag doctrine (the
    /// [`straight_join`](JoinSyntax::straight_join) precedent), unlike the distinct-grammar
    /// [`asof_join`](JoinSyntax::asof_join)/[`positional_join`](JoinSyntax::positional_join) pair.
    /// The same reserved-word interaction as [`asof_join`](JoinSyntax::asof_join) applies: the
    /// DuckDb preset reserves both words as a `ColId`/bare-alias so a bare
    /// `l SEMI JOIN r` reads the join rather than aliasing `l`; `LENIENT` keeps the ANSI
    /// reserved model, so there the join parses only after an explicit alias. On for
    /// DuckDb / Lenient, off elsewhere.
    pub semi_anti_join: bool,
    /// Accept the Spark/Hive/Databricks *sided* semi-/anti-join spelling
    /// (`{LEFT|RIGHT} {SEMI|ANTI} JOIN`), recorded as the
    /// [`SemiAntiSide`](crate::ast::SemiAntiSide) axis on the same
    /// [`JoinOperator::Semi`](crate::ast::JoinOperator)/[`Anti`](crate::ast::JoinOperator::Anti)
    /// operators as DuckDB's side-less form. A *separate* gate from
    /// [`semi_anti_join`](JoinSyntax::semi_anti_join) because it is a different engine family:
    /// DuckDB parse-rejects `LEFT SEMI JOIN` (engine-probed), so folding the sided
    /// spelling into `semi_anti_join` would over-accept it under the DuckDb preset. The
    /// leading `LEFT`/`RIGHT` keyword is already a reserved join side, so — unlike the
    /// keyword-led [`asof_join`](JoinSyntax::asof_join) pair — no reserved-word interplay is
    /// needed: the preceding factor's alias can never swallow it, and a plain
    /// `LEFT [OUTER] JOIN` is disambiguated by the following `SEMI`/`ANTI` keyword. One
    /// flag gates the whole sided family (both sides × `SEMI`/`ANTI`) as one grammar
    /// production (the [`straight_join`](JoinSyntax::straight_join)/[`semi_anti_join`](JoinSyntax::semi_anti_join)
    /// paired-flag doctrine). The sided form always takes an `ON`/`USING` constraint and
    /// never composes with `NATURAL`/`ASOF`. On for the Databricks and Hive presets (whose
    /// engine family documents the spelling — Hive originated `LEFT SEMI JOIN`) and for
    /// Lenient (the permissive superset); off elsewhere, where the other engines
    /// parse-reject the sided spelling. The atomic flag admits the `RIGHT`-sided and `ANTI`
    /// spellings those presets do not all document — a known conservative-direction
    /// over-acceptance a future side-refinement would tighten.
    pub sided_semi_anti_join: bool,
    /// Accept MSSQL's `CROSS APPLY` / `OUTER APPLY` join operators
    /// ([`JoinOperator::Apply`](crate::ast::JoinOperator)) — a lateral-correlated join
    /// over a right table factor (derived table or table-valued function), taking no
    /// `ON`/`USING` constraint. One flag gates the whole `APPLY` family (both the
    /// `CROSS` and `OUTER` flavours) because they are a single grammar production
    /// differing only by that keyword (the [`straight_join`](JoinSyntax::straight_join)
    /// paired-flag doctrine; the [`ApplyKind`](crate::ast::ApplyKind) axis carries the
    /// flavour). No reserved-word interplay is needed — the leading `CROSS`/`OUTER`
    /// keyword already anchors the operator, so the preceding factor's alias can never
    /// swallow it (unlike the keyword-led [`asof_join`](JoinSyntax::asof_join) pair). On for
    /// the MSSQL preset and Lenient (the permissive superset), off elsewhere: the other
    /// engines parse-reject `APPLY` in join position.
    pub apply_join: bool,
    /// Accept the SQL:2023 recursive-query `SEARCH { DEPTH | BREADTH } FIRST BY … SET …`
    /// and `CYCLE … SET … [TO … DEFAULT …] USING …` clauses on a CTE
    /// ([`Cte::search`](crate::ast::Cte)/[`cycle`](crate::ast::Cte)), written after the
    /// CTE body's `)`. One flag gates both clauses because PostgreSQL ships them as a
    /// single recursive-query unit (the [`straight_join`](JoinSyntax::straight_join)
    /// paired-flag doctrine) — no dialect admits one without the other. When off, the
    /// `SEARCH`/`CYCLE` keyword after a CTE body is left unconsumed and the statement is a
    /// clean parse error. On for PostgreSQL / Lenient; off for ANSI (the conservative
    /// baseline, like [`unnest`](TableFactorSyntax::unnest)), MySQL/SQLite (no such clauses), and
    /// DuckDB — which parse-rejects both (`syntax error at or near "SEARCH"`, probed on
    /// 1.5.4), so it overrides the PostgreSQL surface it otherwise inherits (the
    /// [`MutationSyntax::data_modifying_ctes`] split precedent).
    pub recursive_search_cycle: bool,
    /// Parse-reject a top-level `ORDER BY` / `LIMIT` / `OFFSET` on a recursive CTE whose
    /// body is a `UNION [ALL]` set operation (`WITH RECURSIVE t AS (SELECT … UNION ALL
    /// SELECT … FROM t ORDER BY …)`). DuckDB special-cases the recursive term's grammar:
    /// once `WITH RECURSIVE` fronts a `UNION`-bodied CTE the query is a *recursive query*,
    /// and a result-shaping modifier on it is a parse error (`Parser Error: ORDER BY in a
    /// recursive query is not allowed` / `LIMIT or OFFSET in a recursive query is not
    /// allowed`; probed on 1.5.4). "order_limit" names the whole modifier set — `ORDER BY`,
    /// `LIMIT`, and `OFFSET` — because DuckDB forbids them under one rule (no dialect admits
    /// one but not the others), so it is one behaviour, not three.
    ///
    /// Three boundaries are load-bearing and engine-probed (1.5.4), so the check mirrors
    /// them exactly rather than firing on the bare `RECURSIVE` keyword: the body must be a
    /// `UNION` set operation (a non-set-op recursive CTE, or an `INTERSECT`/`EXCEPT` body,
    /// keeps its modifiers — those are not recursive-eligible); the modifier must sit on the
    /// set operation itself, not a parenthesized arm or a nested subquery (`((SELECT …
    /// LIMIT 1) UNION ALL …)` and `… WHERE x < (SELECT … ORDER BY 1)` both accept); and
    /// self-reference is *not* required (DuckDB rejects even a `UNION` whose right arm never
    /// names the CTE — the check is syntactic). On for DuckDB only; every other preset
    /// parse-accepts the modifier (PostgreSQL admits it outright, the rest defer the
    /// restriction to binding), so it stays off there and the modifier rides the ordinary
    /// query tail.
    pub recursive_union_rejects_order_limit: bool,
    /// Accept DuckDB's `USING KEY (col, ...)` recursive-CTE key clause
    /// ([`Cte::using_key`](crate::ast::Cte)), written between the CTE column list and `AS`
    /// (`WITH RECURSIVE t(a, b) USING KEY (a) AS (…)`). DuckDB's keyed-recursion variant
    /// (stable since 1.3; probed accepting on 1.5.4): the recurring table becomes a
    /// key-indexed dictionary whose rows the recursive term overwrites in place. A distinct
    /// gate from [`recursive_search_cycle`](Self::recursive_search_cycle) — a different
    /// engine (DuckDB, not PostgreSQL) and a different grammar slot (before `AS`, not after
    /// the body's `)`), so neither implies the other. Positioned ahead of `AS`, the leading
    /// `USING` cannot be swallowed by any prior production (a CTE otherwise expects `AS`
    /// next), so enabling it shadows no existing spelling. On for DuckDB / Lenient (the
    /// permissive superset); off elsewhere, where the clause is a parse error.
    pub recursive_using_key: bool,
}

/// Dialect-owned table-factor syntax accepted by the parser.
///
/// The `FROM`-item factor forms beyond a plain named table. Split out of
/// [`TableExpressionSyntax`] at its 16-field line as the table-factor axis, distinct from
/// the join and factor-tail/alias cores. Each flag is a grammar gate: when off the factor
/// keyword falls through to the named-table path or surfaces as a clean parse error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableFactorSyntax {
    /// Accept `LATERAL` before derived tables and table functions.
    pub lateral: bool,
    /// Accept function calls as `FROM` items.
    pub table_functions: bool,
    /// Accept PostgreSQL `ROWS FROM (...)` table functions.
    pub rows_from: bool,
    /// Accept the first-class `UNNEST(<expr>[, <expr>…])` table factor
    /// ([`TableFactor::Unnest`](crate::ast::TableFactor)) — an array/collection
    /// expression expanded into a relation, with optional `WITH ORDINALITY`, a
    /// correlation alias, and (under [`unnest_with_offset`](TableFactorSyntax::unnest_with_offset))
    /// a BigQuery `WITH OFFSET` tail. On for PostgreSQL/DuckDB/Lenient (each admits
    /// `FROM unnest(…)`); off for ANSI/MySQL/SQLite, where `UNNEST(` is left to the
    /// named-table path and — with those presets' [`table_functions`](TableFactorSyntax::table_functions)
    /// also off — surfaces as the same clean parse error as any other function-in-FROM.
    /// Distinct from [`table_functions`](TableFactorSyntax::table_functions): a dialect can admit
    /// generic table functions yet route `UNNEST` to this dedicated node.
    pub unnest: bool,
    /// Accept the BigQuery/ZetaSQL `WITH OFFSET [AS <alias>]` tail on an
    /// [`unnest`](TableFactorSyntax::unnest) table factor — a 0-based ordinal column, the BigQuery
    /// counterpart of PostgreSQL's `WITH ORDINALITY`; the dependency is
    /// [`FeatureDependencyViolation::UnnestWithOffsetWithoutUnnest`]. On for the BigQuery
    /// preset alone — the first shipped dialect to enable it; off for every oracle-compared
    /// preset (PostgreSQL and DuckDB both parse-*reject* `WITH OFFSET`, engine-probed) and
    /// off for Lenient, and BigQuery carries no differential oracle. When off, a
    /// trailing `WITH OFFSET` is left unconsumed and the statement rejects.
    pub unnest_with_offset: bool,
    /// Accept a `WITH ORDINALITY` tail on a table-valued `FROM` source — the generic
    /// function factor, an [`unnest`](TableFactorSyntax::unnest) factor, and a
    /// [`rows_from`](TableFactorSyntax::rows_from) factor — adding a trailing ordinal column. On for
    /// PostgreSQL/DuckDB/Lenient; off for SQLite, whose grammar admits generic
    /// [`table_functions`](TableFactorSyntax::table_functions) but syntax-rejects `WITH ORDINALITY`
    /// (engine-probed: `FROM pragma_table_info('t') WITH ORDINALITY` errors at
    /// `ORDINALITY`). When off, the trailing `WITH ORDINALITY` is left unconsumed and the
    /// statement rejects. Distinct from [`unnest_with_offset`](TableFactorSyntax::unnest_with_offset),
    /// the BigQuery `WITH OFFSET` counterpart.
    pub table_function_ordinality: bool,
    /// Accept a bare SQL special value function (`current_date`, `current_timestamp`,
    /// `current_user`, …) as a `FROM` table source — PostgreSQL's `func_table`
    /// promotion of a `SQLValueFunction` ([`TableFactor::SpecialFunction`](crate::ast::TableFactor)),
    /// e.g. `SELECT * FROM current_date`. On for ANSI/PostgreSQL/SQLite/DuckDB/Lenient.
    /// MySQL has no such promotion — `current_date`/`current_timestamp` are reserved and a
    /// bare one in table position is an `ER_PARSE_ERROR` on mysql:8 — so it is off there,
    /// and the special-function keyword then falls through to the named-table path where the
    /// reserved-word gate rejects it (exactly as the alias position already does).
    pub special_function_table_source: bool,
    /// Accept DuckDB's `PIVOT` operator ([`Pivot`](crate::ast::Pivot)) in both of its
    /// surfaces: the `<source> PIVOT (<aggs> FOR <col> IN (<vals>))` table factor and
    /// the leading-keyword `PIVOT <source> ON … USING …` statement. One flag gates both
    /// grammar points because they are a single dialect unit — DuckDB always admits the
    /// statement wherever it admits the table factor (the [`straight_join`](JoinSyntax::straight_join)
    /// precedent). The flag alone is not enough on a bare table factor: `PIVOT` must
    /// also be in [`FeatureSet::reserved_bare_alias`] or the source's alias swallows it
    /// first (the DuckDb preset reserves it, class `reserved` like `QUALIFY`). On for
    /// DuckDb / Lenient, off elsewhere.
    pub pivot: bool,
    /// Accept DuckDB's `UNPIVOT` operator ([`Unpivot`](crate::ast::Unpivot)) in both its
    /// table-factor and leading-keyword-statement surfaces — the [`pivot`](TableFactorSyntax::pivot)
    /// counterpart, kept a separate flag because `PIVOT` and `UNPIVOT` are distinct
    /// operators (the [`asof_join`](JoinSyntax::asof_join)/[`positional_join`](JoinSyntax::positional_join)
    /// precedent). The same reserved-word interaction applies. On for DuckDb / Lenient,
    /// off elsewhere.
    pub unpivot: bool,
    /// Accept DuckDB's `DESCRIBE`/`SHOW`/`SUMMARIZE` utility as a parenthesized `FROM`
    /// table source — DuckDB's `SHOW_REF` table reference
    /// ([`TableFactor::ShowRef`](crate::ast::TableFactor)), e.g.
    /// `FROM (DESCRIBE SELECT …)`, `FROM (SHOW databases)`. One flag gates all three
    /// keywords because DuckDB models them as a single `SHOW_REF` production (the
    /// paired-flag doctrine). When off, the leading keyword is left to the query/join
    /// grammar inside the parentheses and the construct is a clean parse divergence
    /// (`FROM (DESCRIBE …)` reads `DESCRIBE` as neither a query start nor a joined
    /// table). Only the table-source position is admitted — DuckDB parse-rejects these
    /// at CTE-body position — so, unlike [`pivot`](TableFactorSyntax::pivot), there is no query-body
    /// or leading-keyword-statement surface. On for DuckDb / Lenient, off elsewhere.
    pub show_ref: bool,
    /// Accept DuckDB's bare `FROM VALUES (<row>, …) AS <alias>` row-list table factor —
    /// a `VALUES` constructor standing directly as a table factor *without* the wrapping
    /// parentheses the standard derived table requires
    /// ([`TableFactor::Derived`](crate::ast::TableFactor) tagged
    /// [`DerivedSpelling::BareValues`](crate::ast::DerivedSpelling)). DuckDB
    /// parse-requires a table alias here (a bare `FROM VALUES (1)` is a syntax error;
    /// `FROM VALUES (1) t` accepts — probed on 1.5.4), so the parser rejects a missing
    /// one. When off, `VALUES` in table-factor position is not a table name and the
    /// construct is a clean parse divergence (the reject the other dialects give). The
    /// parenthesized `FROM (VALUES …)` derived table is separate and always accepted. On
    /// for DuckDb / Lenient, off elsewhere.
    pub from_values: bool,
    /// Accept the SQL/JSON `JSON_TABLE(context, path [AS name] [PASSING …] COLUMNS (…)
    /// [… ON ERROR])` table factor ([`TableFactor::JsonTable`](crate::ast::TableFactor)). On
    /// for PostgreSQL/Lenient. Off elsewhere: DuckDB parse-rejects it, and MySQL's `JSON_TABLE`
    /// has a *different* grammar (kept off so this PG-shaped node never fires for it). When
    /// off, `JSON_TABLE(` falls to the ordinary function/name path; reached only when the
    /// keyword is immediately followed by `(`.
    pub json_table: bool,
    /// Accept the SQL/XML `XMLTABLE([XMLNAMESPACES(…),] row PASSING doc COLUMNS …)` table
    /// factor ([`TableFactor::XmlTable`](crate::ast::TableFactor)). On for PostgreSQL/Lenient,
    /// off elsewhere (DuckDB parse-rejects it; MySQL/SQLite/ANSI have no such form). When off,
    /// `XMLTABLE(` falls to the ordinary function/name path; reached only when the keyword is
    /// immediately followed by `(`.
    pub xml_table: bool,
    /// Accept `TABLE(<expr>)` as a first-class `FROM` table factor
    /// ([`TableFactor::TableExpr`](crate::ast::TableFactor)) — sqlparser-rs's
    /// `TableFunction`. Distinct from [`table_functions`](TableFactorSyntax::table_functions) (a
    /// *named* table function, `FROM f(1)`) and from the standalone `TABLE t` query
    /// form, which is not a `FROM`-position factor at all. Snowflake and Oracle are the
    /// engines that document this shape, but neither ships a differential oracle here,
    /// so over-acceptance is unmeasurable — the conservative-preset family rule keeps
    /// this off everywhere but Lenient (the permissive superset), with no oracle-backed
    /// preset enabling it. When off, `TABLE(` in table-factor position falls through to
    /// the named-table path, where the reserved `TABLE` keyword is not an admissible
    /// relation name and the construct is a clean parse error.
    pub table_expr_factor: bool,
    /// Accept the SQL-standard PIVOT table factor's extended value sources and default —
    /// the Snowflake/BigQuery/Oracle grammar layered on the shared
    /// `<source> PIVOT (<aggs> FOR <col> IN (…))` shape: the `IN (ANY [ORDER BY …])`
    /// wildcard and `IN (<subquery>)` value sources
    /// ([`PivotValueSource`](crate::ast::PivotValueSource)) and the Snowflake
    /// `DEFAULT ON NULL (<expr>)` tail ([`Pivot::default_on_null`](crate::ast::Pivot)).
    /// Also the reachability gate for the standard single-`FOR`-column table-factor
    /// PIVOT itself where the DuckDB [`pivot`](Self::pivot) flag is off — so with `pivot`
    /// off and this on the parser reads the table-factor PIVOT but *not* the DuckDB
    /// leading-keyword statement / query-body / `IN <enum>` forms, and stops the
    /// bare-chained multi-`FOR`-column list at one head (the standard admits exactly one).
    /// On for BigQuery / Snowflake / Lenient — none oracle-compared here, so
    /// over-acceptance is unmeasurable and the conservative-preset family rule keeps it
    /// off elsewhere. Like [`pivot`](Self::pivot), a *bare*-factor standard PIVOT is
    /// reachable only where `PIVOT` is in [`FeatureSet::reserved_bare_alias`]; where it is
    /// not (BigQuery/Snowflake today), the suffix fires after an explicit alias — the
    /// `pivot`/`ASOF` reservation-dependency precedent.
    ///
    /// It doubles as the reachability gate for the standard `UNPIVOT` table factor where
    /// the DuckDB [`unpivot`](Self::unpivot) flag is off: PIVOT and UNPIVOT co-travel in
    /// these engines' grammars (every dialect with the standard PIVOT table factor also
    /// has UNPIVOT), so one flag reaches both rather than a redundant sibling that no
    /// dialect would ever toggle apart. The `UNPIVOT` table factor grammar is fully
    /// shared — the `INCLUDE`/`EXCLUDE NULLS` marker, per-column aliases, and multi-column
    /// value/name lists are all DuckDB fields BigQuery/Snowflake reuse — so this gate adds
    /// no new UNPIVOT syntax, only reachability; the same explicit-alias reachability note
    /// applies to a bare-factor standard UNPIVOT.
    pub pivot_value_sources: bool,
    /// Accept the SQL:2016 `<source> MATCH_RECOGNIZE (…)` row-pattern-recognition table
    /// factor ([`MatchRecognize`](crate::ast::MatchRecognize)) — the Snowflake / Oracle
    /// row-pattern-matching clause, with its `PARTITION BY` / `ORDER BY` / `MEASURES` /
    /// `ONE|ALL ROWS PER MATCH` / `AFTER MATCH SKIP` / `PATTERN (…)` / `SUBSET` / `DEFINE`
    /// subclauses. On for Snowflake (documented; no differential oracle, so
    /// over-acceptance is unmeasurable and the conservative-preset family rule keeps it
    /// off elsewhere) and Lenient (the permissive superset). Oracle is not a shipped
    /// preset, so it does not enable this. Like [`pivot`](Self::pivot), a *bare*-factor
    /// `MATCH_RECOGNIZE` is reachable only where the keyword is in
    /// [`FeatureSet::reserved_bare_alias`]; where it is not, the suffix fires after an
    /// explicit alias — the `pivot`/`ASOF` reservation-dependency precedent. When off,
    /// `MATCH_RECOGNIZE` falls through to the named-table/alias path and the construct is
    /// a clean parse divergence.
    pub match_recognize: bool,
    /// Accept SQL Server's `OPENJSON(<json> [, <path>]) [WITH (<col> <type> [<path>] [AS JSON],
    /// …)]` rowset-function table factor ([`OpenJson`](crate::ast::OpenJson)) — a JSON document
    /// parsed into a relation, either with the default `key`/`value`/`type` schema (no `WITH`)
    /// or an explicit column schema. On for MSSQL (documented — SQL Server is the sole engine
    /// with this exact form; no differential oracle ships, so over-acceptance is unmeasurable
    /// and the conservative-preset family rule keeps it off elsewhere) and Lenient (the
    /// permissive superset). `OPENJSON` is unreserved (a rowset function name), so this fires
    /// only when the keyword is immediately followed by `(`; a bare `OPENJSON` stays an ordinary
    /// relation name. When off, `OPENJSON(` falls to the ordinary function/name path, which
    /// rejects at the `WITH (…)` clause tail — the [`json_table`](Self::json_table) precedent.
    ///
    /// Docs: <https://learn.microsoft.com/en-us/sql/t-sql/functions/openjson-transact-sql>.
    pub open_json: bool,
}

/// Dialect-owned mutation-statement (`INSERT`/`UPDATE`/`DELETE`) syntax extensions.
///
/// These cover the non-ANSI tails the standard expresses differently (the standard
/// upsert is `MERGE`, and it has no `RETURNING`): they are explicit dialect data
/// so a parser gate never type-checks the dialect. Each flag is purely a
/// grammar gate — when off, the keyword is left unconsumed and the trailing clause
/// surfaces as a parse error, which is how ANSI rejects PostgreSQL upserts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MutationSyntax {
    /// Accept the post-verb `INSERT IGNORE` modifier.
    pub insert_ignore: bool,
    /// Accept the post-verb `INSERT OVERWRITE` modifier.
    pub insert_overwrite: bool,
    /// Accept a `RETURNING <output> [, ...]` clause on `INSERT`/`UPDATE`/`DELETE`
    /// (PostgreSQL; also Oracle/MariaDB/SQLite).
    pub returning: bool,
    /// Accept an `INSERT ... ON CONFLICT [<target>] DO {NOTHING | UPDATE ...}`
    /// upsert clause (PostgreSQL; also SQLite).
    pub on_conflict: bool,
    /// Accept an `INSERT ... ON DUPLICATE KEY UPDATE <col> = <expr> [, ...]` upsert
    /// clause (MySQL/MariaDB). MySQL infers the conflicting unique key, so there is
    /// no arbiter and no `DO NOTHING`; this also admits the `VALUES(<col>)` reference
    /// to a column's proposed insert value inside those update expressions.
    pub on_duplicate_key_update: bool,
    /// Accept `UPDATE ... SET ( col, ... ) = <source>` multiple-column assignment
    /// (PostgreSQL; SQL feature T641). Also reached through `ON CONFLICT DO UPDATE`.
    pub multi_column_assignment: bool,
    /// Reject explicit value-row RHS tuple assignments whose element count differs from
    /// the target column count (`UPDATE t SET (a, b) = (1)`). This is a parse-time
    /// DuckDB arity check; PostgreSQL parses the same surface and leaves arity to later
    /// analysis, so the behavior is intentionally split from [`multi_column_assignment`](Self::multi_column_assignment).
    pub update_tuple_value_row_arity: bool,
    /// Accept `WHERE CURRENT OF <cursor>` positioned `UPDATE`/`DELETE`
    /// (PostgreSQL; SQL feature F831).
    pub where_current_of: bool,
    /// Accept the `MERGE INTO <target> USING <source> ON <cond> WHEN [NOT] MATCHED ...`
    /// statement (SQL:2003 feature F312, the standard upsert; PostgreSQL 15+). Unlike
    /// the other flags here this gates a *leading* statement keyword, not a trailing
    /// clause: when off, `MERGE` is not dispatched and surfaces as an unknown
    /// statement, which is how MySQL (no `MERGE`) rejects it.
    pub merge: bool,
    /// Accept the MySQL `REPLACE [INTO] <table> ...` delete-then-insert statement.
    /// Like `merge` this gates a *leading* statement keyword: when off (ANSI /
    /// PostgreSQL), `REPLACE` is not dispatched and surfaces as an unknown statement.
    /// `REPLACE` reuses the `INSERT` tail grammar tagged
    /// [`InsertVerb::Replace`](crate::ast::InsertVerb), so it needs no node of its own.
    pub replace_into: bool,
    /// Accept the MySQL `INSERT`/`REPLACE ... SET <col> = <value> [, ...]`
    /// assignment-list source ([`InsertSource::Set`](crate::ast::InsertSource)). When
    /// off (ANSI / PostgreSQL), `SET` after the target is left unconsumed and surfaces
    /// as a parse error, the same reject mechanism the other trailing-clause gates use.
    pub insert_set: bool,
    /// Accept the MySQL single-table `UPDATE`/`DELETE ... [ORDER BY <keys>] [LIMIT
    /// <count>]` row-limiting tails. One flag gates both clauses on both statements
    /// because they are a single dialect unit — MySQL admits the `ORDER BY` only
    /// alongside the `LIMIT` it orders, and on `UPDATE` exactly as on `DELETE`
    /// (mirroring how `table_options` gates the trailing options and column attribute
    /// together). The multi-table `UPDATE`/`DELETE` forms take no such tail, so this is
    /// the single-table grammar only. Off in ANSI/PostgreSQL, which have neither tail:
    /// there the trailing `ORDER BY`/`LIMIT` is left unconsumed and surfaces as a clean
    /// parse error.
    pub update_delete_tails: bool,
    /// Accept joins in the target position of `UPDATE`/`DELETE` and comma-separated
    /// `DELETE FROM` targets.
    pub joined_update_delete: bool,
    /// Accept the SQLite `INSERT OR <action>` / `UPDATE OR <action>` conflict-resolution
    /// prefix on the mutation verb, where `<action>` is `REPLACE`/`IGNORE`/`ABORT`/`FAIL`/
    /// `ROLLBACK` ([`ConflictResolution`](crate::ast::ConflictResolution), the [`Insert::or_action`](crate::ast::Insert)
    /// / [`Update::or_action`](crate::ast::Update) slot). SQLite only. Distinct from the
    /// MySQL `INSERT IGNORE` surface (a bare post-verb `IGNORE`, no `OR`) — that is not
    /// absorbed here. Off in ANSI/PostgreSQL/MySQL, where the `OR` after the verb is left
    /// unconsumed and surfaces as a clean parse error.
    pub or_conflict_action: bool,
    /// Accept DuckDB `INSERT INTO t BY NAME|BY POSITION …` column-matching mode
    /// between the target and the source. When off, `BY` is left unconsumed.
    pub insert_column_matching: bool,
    /// Accept the `DELETE FROM <target> USING <from-list> …` multi-relation delete
    /// (PostgreSQL; also MySQL's multi-table delete). On for
    /// ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite has no `USING` on `DELETE`
    /// (engine-measured-rejected via rusqlite), so it is off; the `USING` keyword is then
    /// left unconsumed and the trailing relation list surfaces as a clean parse error.
    pub delete_using: bool,
    /// Accept a PostgreSQL/SQLite `UPDATE … SET … FROM <table refs>` additional-relations
    /// clause. On for ANSI/PostgreSQL/SQLite/DuckDB/Lenient. MySQL has no `UPDATE … FROM` —
    /// its multi-table update lists every table in the target position
    /// (`UPDATE t1, t2 SET …`) — so `UPDATE t SET … FROM u` is an `ER_PARSE_ERROR` on
    /// mysql:8; with the flag off the `FROM` keyword is left unconsumed and surfaces as a
    /// clean parse error.
    pub update_from: bool,
    /// Accept an alias on the *target* of a `DELETE FROM <target> USING <table_refs>`
    /// multi-table delete (`DELETE FROM t AS e USING u …`). On for ANSI/PostgreSQL/
    /// DuckDB/Lenient. MySQL's `DELETE FROM tbl … USING …` names the delete target(s) as
    /// bare table names that must match the `USING` relations, so an alias there is an
    /// `ER_PARSE_ERROR` on mysql:8 (`DELETE FROM event AS e USING sales …` rejects, while a
    /// single-table `DELETE FROM event AS e WHERE …` — no `USING` — parses); it is off for
    /// MySQL. Only the `USING`-form target is governed — the plain single-table delete's
    /// alias is unaffected. Independent of [`delete_using`](Self::delete_using), which gates
    /// whether the `USING` clause is admitted at all.
    pub delete_using_target_alias: bool,
    /// Accept a leading `WITH` CTE clause before an `INSERT` statement
    /// (`WITH a AS (…) INSERT INTO t …`). On for ANSI/PostgreSQL/DuckDB/Lenient. MySQL
    /// admits a statement-leading `WITH` before `SELECT`/`UPDATE`/`DELETE` but *not* before
    /// `INSERT` (its CTE for an insert rides the `INSERT … SELECT` source instead:
    /// `INSERT INTO t WITH … SELECT …`), so `WITH … INSERT …` is an `ER_PARSE_ERROR` on
    /// mysql:8; it is off for MySQL and the `INSERT` after the CTE list then surfaces as a
    /// clean parse error. The bare `INSERT` with no leading `WITH` is unaffected.
    pub cte_before_insert: bool,
    /// Accept a leading `WITH` CTE clause before a `MERGE` statement
    /// (`WITH a AS (…) MERGE INTO t USING a …`; PostgreSQL 15+, DuckDB — probed on
    /// 1.5.4). Off for ANSI: SQL:2016's `<merge statement>` takes no `<with clause>`
    /// (unlike an `INSERT`, whose source query carries one), so a leading `WITH`
    /// before `MERGE` is a dialect extension, not standard surface. The `MERGE`
    /// counterpart of [`cte_before_insert`](Self::cte_before_insert): when off, the
    /// `MERGE` after the CTE list surfaces as a clean parse error, and the bare
    /// `MERGE` (no leading `WITH`) is unaffected. Only reachable where
    /// [`merge`](Self::merge) dispatches `MERGE` at all (the dependency is
    /// [`FeatureDependencyViolation::CteBeforeMergeWithoutMerge`]).
    pub cte_before_merge: bool,
    /// Accept a data-modifying statement — `INSERT`/`UPDATE`/`DELETE`/`MERGE`, the
    /// `MERGE` arm PG 17+ — as a CTE body
    /// (`WITH t AS (DELETE FROM x RETURNING *) SELECT * FROM t`;
    /// [`CteBody`](crate::ast::CteBody)). PostgreSQL admits the DML body at *every*
    /// `WITH` site during raw parsing — nested subquery/scalar-subquery `WITH`s,
    /// `DECLARE CURSOR`/`CREATE TABLE AS`/`CREATE VIEW`/`EXPLAIN`/`COPY (…) TO`
    /// bodies (all probed on pg_query 17; misuse is rejected at analysis, past the
    /// parse boundary this crate models) — so the gate governs the one shared CTE
    /// grammar uniformly rather than per placement. Off everywhere else: DuckDB
    /// parse-rejects a DML CTE body (`A CTE needs a SELECT`, probed on 1.5.4), MySQL
    /// is an `ER_PARSE_ERROR` (probed on mysql:8), SQLite rejects, and the SQL
    /// standard has no data-modifying `WITH`; there the DML keyword after `AS (` is
    /// not dispatched and surfaces as the ordinary query-body parse error.
    pub data_modifying_ctes: bool,
    /// Accept the `WHEN NOT MATCHED BY {SOURCE | TARGET}` `MERGE` arms (PostgreSQL 17+,
    /// DuckDB — both probed). `NOT MATCHED BY TARGET` is the bare `NOT MATCHED` (an
    /// unpaired source row → insert); `NOT MATCHED BY SOURCE` is the new arm firing on an
    /// unpaired *target* row (→ update/delete, like `MATCHED`). Off in ANSI: SQL:2016's
    /// `<merge when clause>` is only `MATCHED`/`NOT MATCHED`, so the `BY` after `NOT
    /// MATCHED` is not standard surface — with the flag off it is left unconsumed and the
    /// clause surfaces as a clean parse error. Only reachable where [`merge`](Self::merge)
    /// dispatches `MERGE` at all (the dependency is
    /// [`FeatureDependencyViolation::MergeWhenNotMatchedByWithoutMerge`]); the bare
    /// `WHEN NOT MATCHED` is unaffected.
    pub merge_when_not_matched_by: bool,
    /// Accept the `MERGE ... WHEN NOT MATCHED THEN INSERT DEFAULT VALUES` action — a
    /// column-default row taking neither a column list nor an `OVERRIDING` clause
    /// (PostgreSQL, DuckDB — both probed). Off in ANSI: SQL:2016's
    /// `<merge insert specification>` is `INSERT [cols] [override] VALUES (...)` with no
    /// `DEFAULT VALUES` alternative, so with the flag off the `DEFAULT` after `INSERT`
    /// surfaces as a clean parse error. Distinct from the top-level `INSERT ... DEFAULT
    /// VALUES` source, which every dialect admits. Only reachable where
    /// [`merge`](Self::merge) dispatches `MERGE` at all (the dependency is
    /// [`FeatureDependencyViolation::MergeInsertDefaultValuesWithoutMerge`]).
    pub merge_insert_default_values: bool,
    /// Accept the `MERGE ... WHEN NOT MATCHED THEN INSERT ... OVERRIDING {SYSTEM | USER}
    /// VALUE` identity override on the merge insert action (SQL:2016
    /// `<merge insert specification>`'s `<override clause>`; PostgreSQL). Unlike the
    /// other two merge extensions here this is *standard* surface, so it is on for ANSI —
    /// but DuckDB rejects `OVERRIDING` inside `MERGE` (`syntax error at or near
    /// "OVERRIDING"`, probed on 1.5.4) while accepting it on a top-level `INSERT`, so its
    /// preset splits from the shared top-level [`InsertOverriding`](crate::ast::InsertOverriding)
    /// grammar in exactly this knob. Off for DuckDB/MySQL; with the flag off the
    /// `OVERRIDING` between the column list and `VALUES` is left unconsumed and surfaces
    /// as a clean parse error. Only reachable where [`merge`](Self::merge) dispatches
    /// `MERGE` at all (the dependency is
    /// [`FeatureDependencyViolation::MergeInsertOverridingWithoutMerge`]).
    pub merge_insert_overriding: bool,
    /// Accept additional comma-separated value rows in a `MERGE ... THEN INSERT` action.
    pub merge_insert_multirow: bool,
    /// Accept DuckDB `UPDATE SET *` in a MERGE WHEN MATCHED arm (column-wise copy
    /// from the source). Off in ANSI/PostgreSQL.
    pub merge_update_set_star: bool,
    /// Accept DuckDB `INSERT *` / `INSERT BY NAME [*]` merge insert spellings.
    /// Off in ANSI/PostgreSQL (only the standard column-list / VALUES form).
    pub merge_insert_star_by_name: bool,
    /// Accept DuckDB `THEN ERROR` as a MERGE action (runtime raises when the arm
    /// fires). Off in ANSI/PostgreSQL.
    pub merge_error_action: bool,
    /// Accept a multi-part (qualified) column name as an `UPDATE … SET` assignment target
    /// (`UPDATE t SET t.i = 1` / `schema.t.col = …`). On for ANSI/PostgreSQL/MySQL/SQLite/Lenient.
    /// DuckDB parse-rejects qualified SET targets ("Qualified column names in UPDATE .. SET
    /// not supported", probed on 1.5.4), so it is off there; with the flag off a target whose
    /// [`ObjectName`](crate::ast::ObjectName) has more than one part surfaces as a clean parse
    /// error. A bare single-part column remains free either way.
    pub update_set_qualified_column: bool,
}

/// Dialect-owned whole-statement DDL dispatch gates accepted by the parser.
/// Dialect-owned view/sequence *clause* syntax accepted after a DDL statement head.
///
/// Post-dispatch refinements on `CREATE VIEW` / `CREATE MATERIALIZED VIEW` / `CREATE SEQUENCE`
/// — temporary views, recursive views, view `WITH` options, matview `TO` target, sequence
/// `CACHE` — split out of [`StatementDdlGates`] so whole-statement object-kind gates stay
/// separate from clause-level grammar. Statement-head flags (`create_sequence`,
/// `materialized_views`, `or_replace`, …) remain on [`StatementDdlGates`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewSequenceClauseSyntax {
    /// Accept `CACHE <n>` in the option list of `CREATE SEQUENCE`.
    pub create_sequence_cache: bool,
    /// Accept a materialized-view storage target: `CREATE MATERIALIZED VIEW name TO target AS ...`.
    pub materialized_view_to: bool,
    /// Accept the `TEMP`/`TEMPORARY` modifier on a plain `CREATE [OR REPLACE] VIEW`
    /// (PostgreSQL/SQLite/DuckDB spell session-local views). On for ANSI/PostgreSQL/SQLite/
    /// DuckDB/Lenient. MySQL has no temporary views (only temporary *tables*), so it is off;
    /// a consumed `TEMP`/`TEMPORARY` prefix leading into `VIEW` is then the syntax error
    /// MySQL reports (engine-measured-rejected on mysql:8). Gates only the view surface — the
    /// `CREATE TEMPORARY TABLE` form is a separate, unaffected family.
    pub temporary_views: bool,
    /// Accept the `RECURSIVE` keyword before `VIEW` in `CREATE [OR REPLACE]
    /// [TEMP|TEMPORARY] RECURSIVE VIEW <name> (<columns>) AS <query>` (DuckDB,
    /// engine-measured on duckdb 1.5.4). On for DuckDB/Lenient only — although
    /// PostgreSQL spells the same form, it is gated to the measured dialect per the
    /// no-shadowing doctrine rather than widened to the PostgreSQL reference without a
    /// differential. Off elsewhere, where the `RECURSIVE` keyword is left unconsumed
    /// before the expected `VIEW` and surfaces as a clean parse error. The keyword sits
    /// between the `TEMP`/`TEMPORARY` prefix and `VIEW`, never composes with
    /// `MATERIALIZED`, and requires the explicit column list (the engine desugars a
    /// recursive view to `WITH RECURSIVE`, which names its output columns).
    pub recursive_views: bool,
    /// Accept MySQL's view definition-option surface: the `[ALGORITHM = {UNDEFINED | MERGE |
    /// TEMPTABLE}] [DEFINER = <user>] [SQL SECURITY {DEFINER | INVOKER}]` prefix (before the
    /// `VIEW` keyword) on `CREATE VIEW`, and the whole `ALTER VIEW` redefinition statement,
    /// dispatched to the [`AlterView`](crate::ast::AlterView) node. One flag gates both because
    /// they are the one MySQL view-definition behaviour — the identical [`ViewOptions`](crate::ast::ViewOptions)
    /// prefix decorates `CREATE VIEW` and heads `ALTER VIEW`, and no dialect has one without the
    /// other. On for MySQL/Lenient. Off elsewhere: the option keywords before `VIEW` are left
    /// unconsumed (a clean parse error), and `ALTER VIEW` routes only to the DuckDB
    /// [`alter_object_set_schema`](StatementDdlGates::alter_object_set_schema) `SET SCHEMA` head
    /// where that flag is on. A *separate* behaviour from that schema-relocation gate: this is
    /// the redefinition/option surface, that is the cross-schema move.
    pub view_definition_options: bool,
}

/// Dialect-owned whole-statement non-`TABLE` DDL dispatch gates accepted by the parser.
///
/// Leading-object-kind gates for `CREATE`/`DROP`/`ALTER` families other than table-body
/// clauses (those live on [`CreateTableClauseSyntax`] / [`ColumnDefinitionSyntax`] /
/// [`ConstraintSyntax`]). View/sequence *clause* refinements after dispatch live on
/// [`ViewSequenceClauseSyntax`]. Each flag is a statement-head gate unless noted:
/// when off the keyword is not dispatched and surfaces as an unknown statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatementDdlGates {
    /// Accept `CREATE`/`DROP COLOCATION GROUP` and table membership clauses.
    pub colocation_groups: bool,
    /// Accept the SQLite `CREATE [TEMP] TRIGGER [IF NOT EXISTS] <name> <timing>
    /// <event> ON <table> [FOR EACH ROW] [WHEN <expr>] BEGIN <stmt>; … END` statement.
    /// Like the other flags on this axis it gates the *whole* statement (the leading
    /// `TRIGGER` after `CREATE`), not a decoration of an already-accepted family: only
    /// SQLite's SQL-statement body form is modelled, and PostgreSQL/MySQL spell an
    /// incompatible `EXECUTE FUNCTION`/external-routine body they reject for this form,
    /// so gating to SQLite (and Lenient) is behaviour-accurate. When off, `TRIGGER`
    /// falls through to the `CREATE TABLE` expectation and surfaces as an unknown
    /// statement.
    pub create_trigger: bool,
    /// Accept the DuckDB `CREATE [OR REPLACE] [TEMP] {MACRO | FUNCTION} <name>(<params>)
    /// AS <expr> | AS TABLE <query>` macro DDL. Like [`create_trigger`](StatementDdlGates::create_trigger)
    /// this gates the *whole* statement, not a decoration of an always-accepted family:
    /// DuckDB's `MACRO` keyword and its live-body `FUNCTION` are dispatched to the
    /// [`CreateMacro`](crate::ast::CreateMacro) node only under this flag. On for
    /// DuckDB/Lenient. Off elsewhere: `MACRO` then falls through to the `CREATE TABLE`
    /// expectation (an unknown statement), and `CREATE FUNCTION` keeps routing to the
    /// string-body routine parser gated by [`routines`](StatementDdlGates::routines) — the two
    /// grammars are disjoint (a live expr/query body vs an opaque source string), so a
    /// macro is never silently reinterpreted as a routine where the flag is off.
    ///
    /// This same flag also gates the matching `DROP MACRO [TABLE] <name>` object kinds
    /// ([`DropObjectKind::Macro`](crate::ast::DropObjectKind::Macro) /
    /// [`MacroTable`](crate::ast::DropObjectKind::MacroTable)) — a dialect with `CREATE MACRO`
    /// has `DROP MACRO`. Unlike the sibling one-flag-gates-both forms (`TYPE`/`SEQUENCE`),
    /// only the `MACRO` drop spelling is gated here: the `FUNCTION` synonym DuckDB accepts on
    /// `DROP` routes to the signature routine drop gated by [`routines`](StatementDdlGates::routines),
    /// not to a macro object kind.
    pub create_macro: bool,
    /// Accept DuckDB's secrets-management statements: `CREATE [PERSISTENT] SECRET <name>
    /// (<option> <value>, …)`, dispatched to the [`CreateSecret`](crate::ast::CreateSecret)
    /// node, and its drop counterpart `DROP [PERSISTENT | TEMPORARY] SECRET [IF EXISTS] <name>
    /// [FROM <storage>]`, dispatched to [`DropSecretStmt`](crate::ast::DropSecretStmt). One
    /// flag covers both because they are the same secrets behaviour surface (the drop is
    /// meaningless without the create). Like [`create_macro`](StatementDdlGates::create_macro)
    /// this gates the *whole* statement: on for DuckDB/Lenient, off elsewhere, where the
    /// `PERSISTENT`/`SECRET` keyword falls through to the `CREATE TABLE`/`DROP` object-kind
    /// expectation and surfaces as an unknown statement.
    pub create_secret: bool,
    /// Accept DuckDB's `CREATE [OR REPLACE] [TEMP] TYPE <name> AS ENUM(…)/STRUCT(…)/<alias>`
    /// user-defined-type DDL, dispatched to the [`CreateType`](crate::ast::CreateType) node,
    /// and the matching `DROP TYPE` object kind ([`DropObjectKind::Type`](crate::ast::DropObjectKind)).
    /// Like [`create_macro`](StatementDdlGates::create_macro) this gates the *whole* statement: on for
    /// DuckDB/Lenient, off elsewhere, where the `TYPE` keyword falls through to the
    /// `CREATE TABLE` expectation (an unknown statement) and `DROP TYPE` is an unexpected
    /// object kind. One flag gates both leading forms — a dialect with `CREATE TYPE` has
    /// `DROP TYPE`.
    pub create_type: bool,
    /// Accept the SQLite `CREATE VIRTUAL TABLE [IF NOT EXISTS] <name> USING <module>
    /// [(<args>)]` statement, dispatched to the
    /// [`CreateVirtualTable`](crate::ast::CreateVirtualTable) node. Like
    /// [`create_trigger`](StatementDdlGates::create_trigger) this gates the *whole* statement (the
    /// leading `VIRTUAL` after `CREATE`): only SQLite has virtual tables, and the
    /// module-owned argument list is meaningless elsewhere, so on for SQLite/Lenient and
    /// off everywhere else — where `VIRTUAL` falls through to the `CREATE TABLE`
    /// expectation and surfaces as an unknown statement.
    pub create_virtual_table: bool,
    /// Accept the `CREATE [TEMPORARY] SEQUENCE [IF NOT EXISTS] <name> [<option> ...]`
    /// sequence-generator statement (SQL:2003 T176; PostgreSQL/DuckDB), dispatched to the
    /// [`CreateSequence`](crate::ast::CreateSequence) node, and the matching `DROP SEQUENCE`
    /// object kind ([`DropObjectKind::Sequence`](crate::ast::DropObjectKind)). Like
    /// [`create_type`](StatementDdlGates::create_type) this gates the *whole* statement: on for
    /// PostgreSQL/DuckDB/Lenient, off elsewhere, where the `SEQUENCE` keyword falls through
    /// to the `CREATE TABLE` expectation (an unknown statement) and `DROP SEQUENCE` is an
    /// unexpected object kind. One flag gates both leading forms — a dialect with
    /// `CREATE SEQUENCE` has `DROP SEQUENCE`. The modelled tail is the shared standard option
    /// core both engines' parsers accept (`START [WITH]`, `INCREMENT [BY]`, `MIN`/`MAXVALUE`,
    /// `NO MIN`/`MAXVALUE`, `CYCLE`/`NO CYCLE`). The independently gated PostgreSQL
    /// `CACHE` extension is modelled by
    /// [`ViewSequenceClauseSyntax::create_sequence_cache`];
    /// `AS` and `OWNED BY` remain unmodelled.
    pub create_sequence: bool,
    /// Accept the PostgreSQL extension-DDL statements `CREATE EXTENSION [IF NOT EXISTS]
    /// <name> [WITH] [SCHEMA s] [VERSION v] [CASCADE]` and `ALTER EXTENSION <name>
    /// {UPDATE [TO v] | ADD <member> | DROP <member>}`, dispatched to the
    /// [`CreateExtension`](crate::ast::CreateExtension) and
    /// [`AlterExtension`](crate::ast::AlterExtension) nodes. Like
    /// [`create_sequence`](StatementDdlGates::create_sequence) this gates the *whole*
    /// statement (the leading `EXTENSION` keyword after `CREATE`, and the `EXTENSION`
    /// dispatch after `ALTER`): PostgreSQL is the only shipped dialect with an extension
    /// catalogue, so on for PostgreSQL/Lenient and off everywhere else — where `EXTENSION`
    /// falls through to the `CREATE TABLE` expectation (an unknown statement) or the
    /// `ALTER TABLE` expectation. One flag gates both the `CREATE` and `ALTER` forms — a
    /// dialect with `CREATE EXTENSION` has `ALTER EXTENSION`.
    pub extension_ddl: bool,
    /// Accept the PostgreSQL `DROP TRANSFORM [IF EXISTS] FOR <type> LANGUAGE <lang>
    /// [CASCADE | RESTRICT]` statement, dispatched to the
    /// [`DropTransform`](crate::ast::DropTransform) node. Like
    /// [`extension_ddl`](StatementDdlGates::extension_ddl) this gates the *whole* statement
    /// (the `TRANSFORM` keyword after `DROP`): only PostgreSQL has the transform catalogue
    /// (`pg_transform` — a `(type, language)` conversion registered by `CREATE TRANSFORM`),
    /// so on for PostgreSQL/Lenient and off everywhere else — where `TRANSFORM` falls through
    /// to the `DROP` object-kind expectation and surfaces as a clean parse error.
    ///
    /// A *separate* behaviour from [`extension_ddl`](StatementDdlGates::extension_ddl),
    /// carrying its own gate rather than riding that one — the same split
    /// [`alter_system`](StatementDdlGates::alter_system) makes. A transform is procedural-
    /// language infrastructure (a type↔language conversion), not an extension-catalogue
    /// operation: unlike `ALTER … DEPENDS ON EXTENSION` (which names `EXTENSION` in its
    /// syntax and so rides `extension_ddl`), `DROP TRANSFORM` mutates a standalone
    /// `pg_transform` object with no extension in the grammar. It reuses the shared
    /// [`ObjectReference::Transform`](crate::ast::ObjectReference) axis for its `FOR type
    /// LANGUAGE lang` shape — the same axis `ALTER EXTENSION … ADD|DROP TRANSFORM` names a
    /// member with — but that shared *node* is not a shared *behaviour gate*.
    pub transform_ddl: bool,
    /// Accept the PostgreSQL `ALTER SYSTEM { SET <name> {= | TO} <value> | RESET <name> |
    /// RESET ALL }` server-configuration statement, dispatched to the
    /// [`AlterSystem`](crate::ast::AlterSystem) node. Like
    /// [`extension_ddl`](StatementDdlGates::extension_ddl) this gates the *whole* statement
    /// (the `SYSTEM` dispatch after `ALTER`): only PostgreSQL persists a server-wide
    /// configuration through SQL (`postgresql.auto.conf`), so on for PostgreSQL/Lenient and
    /// off everywhere else — where `SYSTEM` falls through to the `ALTER TABLE` expectation
    /// and surfaces as a clean parse error. A *separate* behaviour from
    /// [`extension_ddl`](StatementDdlGates::extension_ddl): `ALTER SYSTEM` is server
    /// configuration, unrelated to the extension catalogue, so it carries its own gate rather
    /// than riding that one. It reuses the session-`SET` value axis
    /// ([`SetValue`](crate::ast::SetValue) / [`ConfigParameter`](crate::ast::ConfigParameter))
    /// for the setting name/value grammar, but admits no `SESSION`/`LOCAL` scope and no
    /// `FROM CURRENT` — the wrapper is exactly PostgreSQL's `generic_set`/`generic_reset`.
    pub alter_system: bool,
    /// Accept MySQL's tablespace storage-DDL statements — `CREATE [UNDO] TABLESPACE <name> …`,
    /// `ALTER [UNDO] TABLESPACE <name> <action>`, and `DROP [UNDO] TABLESPACE <name> [<option>...]`
    /// — dispatched to the [`CreateTablespace`](crate::ast::CreateTablespace),
    /// [`AlterTablespace`](crate::ast::AlterTablespace), and
    /// [`DropTablespace`](crate::ast::DropTablespace) nodes. Like
    /// [`extension_ddl`](StatementDdlGates::extension_ddl) this gates the *whole* statement (the
    /// `TABLESPACE`/`UNDO` dispatch after `CREATE`/`ALTER`/`DROP`): only MySQL models an
    /// InnoDB/NDB tablespace catalogue, so on for MySQL/Lenient and off everywhere else — where
    /// the keyword falls through to the `CREATE`/`ALTER TABLE` expectation (an unknown statement)
    /// or the `DROP` object-kind expectation. One flag gates all three verbs and both the plain
    /// and `UNDO` variants — a dialect with `CREATE TABLESPACE` has the whole family. A separate
    /// behaviour from [`logfile_group_ddl`](StatementDdlGates::logfile_group_ddl): a tablespace and
    /// a logfile group are distinct storage objects with distinct leading keywords, so each
    /// carries its own gate rather than one bundling both.
    pub tablespace_ddl: bool,
    /// Accept MySQL's NDB logfile-group storage-DDL statements — `CREATE LOGFILE GROUP <name> ADD
    /// UNDOFILE '<f>' [<option>...]`, `ALTER LOGFILE GROUP <name> ADD UNDOFILE '<f>' [<option>...]`,
    /// and `DROP LOGFILE GROUP <name> [<option>...]` — dispatched to the
    /// [`CreateLogfileGroup`](crate::ast::CreateLogfileGroup),
    /// [`AlterLogfileGroup`](crate::ast::AlterLogfileGroup), and
    /// [`DropLogfileGroup`](crate::ast::DropLogfileGroup) nodes. Like
    /// [`tablespace_ddl`](StatementDdlGates::tablespace_ddl) this gates the *whole* statement (the
    /// `LOGFILE GROUP` dispatch after `CREATE`/`ALTER`/`DROP`): only MySQL (NDB) has a logfile-group
    /// catalogue, so on for MySQL/Lenient and off everywhere else — where `LOGFILE` falls through
    /// to the `CREATE`/`ALTER TABLE` expectation (an unknown statement) or the `DROP` object-kind
    /// expectation. One flag gates all three verbs — a dialect with `CREATE LOGFILE GROUP` has the
    /// whole family. A separate behaviour from
    /// [`tablespace_ddl`](StatementDdlGates::tablespace_ddl), for the reason documented there.
    pub logfile_group_ddl: bool,
    /// Accept the `CREATE SCHEMA …` and `DROP SCHEMA …` schema-object statements
    /// (SQL:2016 F771; PostgreSQL/MySQL). One flag gates both leading forms — a dialect
    /// with `CREATE SCHEMA` has `DROP SCHEMA`. On for ANSI/PostgreSQL/MySQL/DuckDB/
    /// Lenient. SQLite has no schema objects (a database *is* the schema namespace), so
    /// it is off; the `SCHEMA` keyword is then not dispatched and surfaces as an unknown
    /// statement.
    pub schemas: bool,
    /// Accept the SQL-standard embedded schema-element list on `CREATE SCHEMA`
    /// (`CREATE SCHEMA s CREATE TABLE t (...) CREATE VIEW ...`): the component objects
    /// created *inside* the new schema, parsed as children of the [`CreateSchema`](crate::ast::CreateSchema)
    /// node so the whole construct stays ONE statement. The admissible element set is
    /// closed and measured against PostgreSQL — `CREATE TABLE`/`VIEW`/`INDEX`/`SEQUENCE`/
    /// `TRIGGER` and `GRANT` — with `CREATE MATERIALIZED VIEW`/`FUNCTION`, a nested
    /// `CREATE SCHEMA`, and `DROP`/`ALTER`/`INSERT`/… all rejected as elements.
    ///
    /// On for PostgreSQL and Lenient (its superset). Off elsewhere: ANSI/MySQL/DuckDB
    /// accept the schema head but not the embedded form (MySQL/DuckDB reject the
    /// standard embedding; ANSI keeps a bare head), so a following `CREATE`/`GRANT`
    /// there is left to the top-level statement loop rather than consumed as an element.
    /// Narrower than [`schemas`](StatementDdlGates::schemas) on purpose — the head is
    /// widely accepted, the embedding is not.
    pub schema_elements: bool,
    /// Accept the `CREATE DATABASE …` statement (PostgreSQL/MySQL). On for
    /// ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite has no `CREATE DATABASE` (databases
    /// are files, reached via `ATTACH`), so it is off; the `DATABASE` keyword is then not
    /// dispatched and surfaces as an unknown statement.
    pub databases: bool,
    /// Accept MySQL's `DROP {DATABASE | SCHEMA} [IF EXISTS] <name>` — a single-database drop
    /// where `DATABASE` and `SCHEMA` are exact synonyms (the lexer folds them onto one
    /// grammar), naming exactly one unqualified database with no `CASCADE`/`RESTRICT` and no
    /// comma list (server-measured on mysql:8: `DROP DATABASE a, b`, `DROP DATABASE db.x`, and
    /// `DROP DATABASE a CASCADE` are each `ER_PARSE_ERROR`). Distinct from the shared
    /// name-list `DROP SCHEMA <name> [, …] [CASCADE | RESTRICT]` gated by
    /// [`schemas`](StatementDdlGates::schemas): where this flag is on, both the `DATABASE` and
    /// `SCHEMA` keywords are intercepted for the single-name form *before* the shared
    /// name-list path, so the two cannot both fire. On for MySQL only. Off elsewhere,
    /// including Lenient: because enabling it would recast `DROP SCHEMA` as the single-name
    /// form and forfeit the more permissive PostgreSQL/DuckDB name-list-plus-`CASCADE`
    /// `DROP SCHEMA` — a documented conflict resolution — Lenient keeps the name-list path and
    /// forgoes the MySQL `DROP DATABASE` spelling. With the flag off the `DATABASE` keyword is
    /// not dispatched as a drop and surfaces as an unknown drop object kind.
    pub drop_database: bool,
    /// Accept the `CREATE MATERIALIZED VIEW …` and `DROP MATERIALIZED VIEW …` statements
    /// (PostgreSQL; DuckDB). One flag gates both — a dialect with the create form has the
    /// drop form. On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite has no materialized
    /// views, so it is off; `MATERIALIZED` is then not dispatched and surfaces as an
    /// unknown statement (a plain `CREATE VIEW` is unaffected — a separate always-accepted
    /// family).
    pub materialized_views: bool,
    /// Accept the stored-routine DDL — `CREATE FUNCTION …`, `DROP FUNCTION …`, and
    /// `DROP PROCEDURE …` (PostgreSQL/MySQL SQL/PSM). One flag gates the routine family as
    /// a unit. On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite has no stored routines
    /// (its functions are C-registered, not SQL-declared), so it is off; the `FUNCTION`/
    /// `PROCEDURE` keyword is then not dispatched and surfaces as an unknown statement.
    pub routines: bool,
    /// Accept the `CREATE OR REPLACE …` object-replacement modifier on `VIEW`/`FUNCTION`
    /// (PostgreSQL; MySQL `OR REPLACE VIEW`). On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient.
    /// SQLite has no `OR REPLACE` (it spells the intent `DROP` then `CREATE`), so it is
    /// off; the `OR` after `CREATE` is then left unconsumed and surfaces as a clean parse
    /// error.
    pub or_replace: bool,
    /// Accept DuckDB's `CREATE OR REPLACE TABLE` — the `OR REPLACE` object-replacement
    /// modifier on `TABLE` (threaded onto the [`CreateTable`](crate::ast::CreateTable) node).
    /// **Statement-head** gate: consulted during `CREATE` dispatch when the object kind is
    /// `TABLE`, not a table-body clause (moved off [`CreateTableClauseSyntax`] for MECE).
    /// On for DuckDB/Lenient. Off elsewhere: other dialects take `OR REPLACE` only on
    /// `VIEW`/`FUNCTION` (gated by [`or_replace`](Self::or_replace)), so with this flag off a
    /// `TABLE` after `OR REPLACE` is left for the `VIEW` expectation and surfaces as a clean
    /// parse error.
    pub create_or_replace_table: bool,
    /// Parse a routine/trigger/event body as a MySQL SQL/PSM *compound statement* — the
    /// `[<label>:] BEGIN [<declarations>] … END` block with its `DECLARE` prefix, the
    /// flow-control statements (`IF`/`CASE`/`LOOP`/`WHILE`/`REPEAT`/`LEAVE`/`ITERATE`/
    /// `RETURN`) and the cursor operations (`OPEN`/`FETCH`/`CLOSE`). This is a
    /// *body-context* behaviour, not a top-level statement gate: it governs the separate
    /// `parse_body_statement` dispatcher the routine/trigger wrappers invoke, never the
    /// top-level one (a bare top-level `BEGIN … END` stays transaction-start regardless).
    /// On for MySQL (and Lenient). Off elsewhere: PostgreSQL/ANSI spell a routine body as
    /// an opaque `$$…$$`/string routine definition (a different, unaffected grammar), and
    /// SQLite has no stored programs — so where it is off the body dispatcher rejects the
    /// compound grammar and the existing string/opaque body paths are untouched.
    pub compound_statements: bool,
    /// Accept DuckDB's `ALTER DATABASE [IF EXISTS] <name> SET ALIAS TO <alias>` statement
    /// (`AlterDatabaseStmt`), dispatched to the [`AlterDatabase`](crate::ast::AlterDatabase)
    /// node. Like [`alter_system`](StatementDdlGates::alter_system) this gates the *whole*
    /// statement (the `DATABASE` dispatch after `ALTER`): DuckDB's sole `ALTER DATABASE` form
    /// re-aliases an attached database. On for DuckDB/Lenient, off elsewhere — where
    /// `DATABASE` falls through to the `ALTER TABLE` expectation and surfaces as a clean parse
    /// error. Named for the object it alters, not the DuckDB-specific `SET ALIAS TO` spelling.
    /// MySQL's `ALTER DATABASE` (the charset/collation/encryption/read-only option list) is a
    /// *disjoint* behaviour on its own
    /// [`alter_database_options`](StatementDdlGates::alter_database_options) gate: the two
    /// grammars share only the `ALTER DATABASE` head and cannot be unioned under one gate (DuckDB
    /// rejects MySQL's options and vice versa), so each is its own flag and node per the MECE
    /// doctrine. Although PostgreSQL also has an `ALTER DATABASE` grammar, this stays gated to the
    /// measured dialect (DuckDB) per the no-shadowing doctrine rather than widened to the
    /// PostgreSQL reference without a differential.
    pub alter_database: bool,
    /// Accept MySQL's `ALTER {DATABASE | SCHEMA} [<name>] <option> …` schema-option change
    /// (`alter_database_stmt`), dispatched to the
    /// [`AlterDatabaseOptions`](crate::ast::AlterDatabaseOptions) node. A *disjoint* behaviour
    /// from DuckDB's [`alter_database`](StatementDdlGates::alter_database) `SET ALIAS`
    /// relocation: the two share only the `ALTER DATABASE` head, but MySQL adds the `SCHEMA`
    /// synonym and an optional name and takes a non-empty, repeatable list of charset/collation/
    /// encryption/read-only options — so it is its own gate and its own node rather than a union
    /// (which would make each dialect over-accept the other's grammar). On for MySQL/Lenient, off
    /// elsewhere — where `DATABASE`/`SCHEMA` falls through to the `ALTER TABLE` expectation and
    /// surfaces as a clean parse error (the DuckDB `SET ALIAS` head is intercepted first where
    /// its gate is on, and the two are disambiguated by lookahead under Lenient, which enables
    /// both).
    pub alter_database_options: bool,
    /// Accept MySQL's federated-server DDL — `CREATE SERVER <name> FOREIGN DATA WRAPPER
    /// <wrapper> OPTIONS ( … )`, `ALTER SERVER <name> OPTIONS ( … )`, and `DROP SERVER
    /// [IF EXISTS] <name>` — dispatched to the [`CreateServer`](crate::ast::CreateServer),
    /// [`AlterServer`](crate::ast::AlterServer), and [`DropServer`](crate::ast::DropServer)
    /// nodes. One flag gates all three leading dispatches because they are one cohesive
    /// server-object behaviour: `CREATE`/`ALTER` share the
    /// [`ServerOption`](crate::ast::ServerOption) axis (the `server_options_list` grammar) and
    /// `DROP` disposes of the same object — the `extension_ddl` (CREATE + ALTER extension) and
    /// `view_definition_options` (CREATE VIEW prefix + ALTER VIEW) one-object-one-gate precedent.
    /// On for MySQL/Lenient, off elsewhere, where the `SERVER` head falls through and surfaces as
    /// a clean parse error.
    pub server_definition: bool,
    /// Accept MySQL's `ALTER INSTANCE <action>` server-instance administration statement
    /// (`alter_instance_stmt`), dispatched to the [`AlterInstance`](crate::ast::AlterInstance)
    /// node. A whole-statement gate like [`alter_system`](StatementDdlGates::alter_system) — the
    /// `INSTANCE` dispatch after `ALTER`: a single instance-wide maintenance action (rotate a
    /// master key, reload TLS/the keyring, toggle the InnoDB redo log). A *separate* behaviour
    /// from the server and database DDL: it names no object and touches the running instance, not
    /// a catalogue object. On for MySQL/Lenient, off elsewhere, where `INSTANCE` surfaces as the
    /// "expected TABLE" parse error.
    pub alter_instance: bool,
    /// Accept MySQL's spatial-reference-system DDL — `CREATE [OR REPLACE] SPATIAL REFERENCE
    /// SYSTEM [IF NOT EXISTS] <srid> <attributes>` and `DROP SPATIAL REFERENCE SYSTEM
    /// [IF EXISTS] <srid>` — dispatched to the
    /// [`CreateSpatialReferenceSystem`](crate::ast::CreateSpatialReferenceSystem) and
    /// [`DropSpatialReferenceSystem`](crate::ast::DropSpatialReferenceSystem) nodes. One flag
    /// gates both dispatches because they are one catalogue-object behaviour (the
    /// [`server_definition`](StatementDdlGates::server_definition) one-object-one-gate
    /// precedent). On for MySQL/Lenient, off elsewhere — where the `SPATIAL` head falls through
    /// to the `TABLE` expectation and surfaces as a clean parse error.
    pub spatial_reference_system: bool,
    /// Accept MySQL's resource-group DDL — `CREATE RESOURCE GROUP <name> TYPE [=] {SYSTEM |
    /// USER} [VCPU …] [THREAD_PRIORITY …] [ENABLE | DISABLE]`, `ALTER RESOURCE GROUP <name>
    /// [VCPU …] [THREAD_PRIORITY …] [ENABLE | DISABLE] [FORCE]`, `DROP RESOURCE GROUP <name>
    /// [FORCE]`, and the session-statement `SET RESOURCE GROUP <name> [FOR <thread_ids>]` —
    /// dispatched to the [`CreateResourceGroup`](crate::ast::CreateResourceGroup) /
    /// [`AlterResourceGroup`](crate::ast::AlterResourceGroup) /
    /// [`DropResourceGroup`](crate::ast::DropResourceGroup) statement nodes and the
    /// [`SetResourceGroup`](crate::ast::SessionStatement::SetResourceGroup) session variant. One
    /// flag gates all four because they are one resource-group behaviour sharing the
    /// `VCPU`/`THREAD_PRIORITY`/`ENABLE|DISABLE` axes (the
    /// [`server_definition`](StatementDdlGates::server_definition) precedent); the `SET` member
    /// rides the same flag from inside the `SET`-statement dispatch, claimed off the
    /// `RESOURCE GROUP` two-word lookahead before the variable-assignment fallback. On for
    /// MySQL/Lenient, off elsewhere — where `RESOURCE` falls through and surfaces as a clean
    /// parse error (and `SET RESOURCE GROUP g` stays a variable-assignment parse error).
    pub resource_group: bool,
    /// Accept DuckDB's `ALTER SEQUENCE [IF EXISTS] <name> <option>...` statement
    /// (`AlterSeqStmt`), dispatched to the [`AlterSequence`](crate::ast::AlterSequence) node.
    /// Like [`alter_database`](StatementDdlGates::alter_database) this gates the *whole*
    /// statement (the `SEQUENCE` dispatch after `ALTER`): the option-list form changes a
    /// sequence generator's options, reusing the shared
    /// [`IdentityOption`](crate::ast::IdentityOption) axis for the core `CREATE SEQUENCE` also
    /// accepts. On for DuckDB/Lenient, off elsewhere — where `SEQUENCE` falls through to the
    /// `ALTER TABLE` expectation and surfaces as a clean parse error. A *separate* behaviour
    /// from [`alter_object_set_schema`](StatementDdlGates::alter_object_set_schema): `ALTER
    /// SEQUENCE … SET SCHEMA` is a schema relocation (that gate), not a sequence-option change.
    /// Although PostgreSQL also has `ALTER SEQUENCE`, this stays gated to DuckDB per the
    /// no-shadowing doctrine.
    pub alter_sequence: bool,
    /// Accept DuckDB's `ALTER {TABLE | VIEW | SEQUENCE} [IF EXISTS] <name> SET SCHEMA
    /// <schema>` statement (`AlterObjectSchemaStmt`), dispatched to the
    /// [`AlterObjectSchema`](crate::ast::AlterObjectSchema) node. Like
    /// [`alter_database`](StatementDdlGates::alter_database) this gates the *whole* statement
    /// (the `SET SCHEMA` tail after the object head): it relocates a relocatable object to
    /// another schema. DuckDB 1.5.4's binder rejects this as `Not implemented`, but the
    /// production is parse-reachable and PARSE-level parity is the modelled bar (analogous to
    /// PostgreSQL's grammar-present, engine-unimplemented `CREATE ASSERTION`). On for
    /// DuckDB/Lenient, off elsewhere — where a `SET SCHEMA` tail is left to the `ALTER TABLE`
    /// command parser (a clean parse error for the `VIEW`/`SEQUENCE`/`DATABASE` heads). Gated
    /// to DuckDB per the no-shadowing doctrine, though PostgreSQL shares the grammar.
    pub alter_object_set_schema: bool,
}

/// Dialect-owned `CREATE TABLE` table-level clause syntax accepted by the parser.
///
/// The table-level decorations on a `CREATE TABLE` — the clauses that attach to the table
/// as a whole rather than to a single column or constraint. Split out of the retired
/// `SchemaChangeSyntax` at its 16-field line as the table-clause axis, distinct from the
/// column-definition and constraint axes. Each flag is a grammar gate: when off the clause
/// keyword is left unconsumed and surfaces as a clean parse error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreateTableClauseSyntax {
    /// Accept the MySQL `CREATE TABLE` storage decorations: the trailing table-option
    /// list (`ENGINE = InnoDB`, `AUTO_INCREMENT = 100`, `DEFAULT CHARSET = utf8mb4`,
    /// `COMMENT = '...'`, `ROW_FORMAT = ...`, `COLLATE = ...`) **and** the column-level
    /// underscored `AUTO_INCREMENT` attribute.
    ///
    /// **Deliberate dual-position unit (not a MECE bug):** both grammar points are one
    /// MySQL dialect unit — MySQL always admits the column attribute wherever it admits
    /// the table options (mirroring how `existence_guards.if_exists` co-gates related
    /// sites). Splitting them would invent independent axes no shipped engine separates.
    /// The SQLite joined `AUTOINCREMENT` spelling is a separate flag on
    /// [`ColumnDefinitionSyntax`]. Not ANSI/PostgreSQL, which reject both surfaces as
    /// leftover input.
    pub table_options: bool,
    /// Accept the SQLite trailing `WITHOUT ROWID` table option on `CREATE TABLE`
    /// (`CREATE TABLE t (a INTEGER PRIMARY KEY) WITHOUT ROWID`), recorded as
    /// [`CreateTableOptionKind::WithoutRowid`](crate::ast::CreateTableOptionKind::WithoutRowid).
    /// SQLite-only; off in every other preset, where the trailing `WITHOUT ROWID` is left
    /// unconsumed and surfaces as a clean parse error. Split out of
    /// the retired `sqlite_table_decorations` bundle because the rowid-storage
    /// table option is an independent grammar point from the trailing `STRICT` option, the
    /// typeless column, `AUTOINCREMENT`, the column `COLLATE`, and the inline-`PRIMARY KEY`
    /// ordering.
    pub without_rowid_table_option: bool,
    /// Accept the SQLite trailing `STRICT` table option on `CREATE TABLE`
    /// (`CREATE TABLE t (a INTEGER) STRICT`), recorded as
    /// [`CreateTableOptionKind::Strict`](crate::ast::CreateTableOptionKind::Strict); the table
    /// then enforces its declared column types instead of SQLite's default flexible typing.
    /// SQLite-only; off in every other preset, where the trailing `STRICT` is left unconsumed
    /// and surfaces as a clean parse error. Split out of
    /// the retired `sqlite_table_decorations` bundle because the strict-typing
    /// table option is an independent grammar point from the trailing `WITHOUT ROWID` option,
    /// the typeless column, `AUTOINCREMENT`, the column `COLLATE`, and the inline-`PRIMARY KEY`
    /// ordering.
    pub strict_table_option: bool,
    /// Accept the `CREATE TABLE … WITH ( <name> = <value>, … )` storage-parameter list
    /// (PostgreSQL `WITH (fillfactor=…)`; Trino/Spark `WITH (format=…)`). On for
    /// ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite has no `WITH (…)` table clause, so it
    /// is off; the `WITH` keyword is then not read as a table option and surfaces as a
    /// clean parse error.
    pub storage_parameters: bool,
    /// Accept the temporary-table `ON COMMIT { PRESERVE ROWS | DELETE ROWS | DROP }`
    /// action (SQL:1999; PostgreSQL). On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite
    /// has no `ON COMMIT` table clause, so it is off; the `ON` keyword is then left
    /// unconsumed and surfaces as a clean parse error.
    pub on_commit: bool,
    /// Accept the trailing `WITH [NO] DATA` populate clause on `CREATE TABLE … AS <query>`
    /// (and `CREATE MATERIALIZED VIEW`). On for ANSI/PostgreSQL/SQLite/DuckDB/Lenient. MySQL's
    /// `CREATE TABLE … AS SELECT` has no `WITH [NO] DATA` clause (`ER_PARSE_ERROR` on
    /// mysql:8), so it is off there; the `WITH` keyword after the query is then left as
    /// leftover input and surfaces as a clean parse error.
    pub create_table_as_with_data: bool,
    /// Accept the PostgreSQL `CREATE TABLE t [(cols)] AS EXECUTE <prepared> [(args)] [WITH [NO]
    /// DATA]` form — a CTAS whose rows come from running a prepared statement. On for
    /// PostgreSQL/Lenient. Off for ANSI/MySQL/SQLite/DuckDB (DuckDB rejects `AS EXECUTE`), where
    /// the `EXECUTE` keyword after `AS` is left unconsumed and the inline-query CTAS path rejects
    /// it as a clean parse error.
    pub create_table_as_execute: bool,
    /// Accept PostgreSQL declarative partitioning: the parent `CREATE TABLE … PARTITION BY
    /// {LIST | RANGE | HASH} (<key>, …)` clause, the child `CREATE TABLE … PARTITION OF <parent>
    /// [(<augmentation>, …)] {FOR VALUES … | DEFAULT}` body, and the `ALTER TABLE … {ATTACH |
    /// DETACH} PARTITION` actions. On for PostgreSQL/Lenient. Off for ANSI/MySQL/SQLite/DuckDB:
    /// none spell this grammar (MySQL's `PARTITION BY HASH(c) PARTITIONS n` and DuckDB's
    /// COPY-level `PARTITION_BY` are unrelated surfaces), so the `PARTITION` / `ATTACH` /
    /// `DETACH` keyword is left unconsumed and surfaces as a clean parse error. One flag gates
    /// the whole family — the parent spec, the child body, and the two alter actions travel
    /// together as a single dialect unit.
    pub declarative_partitioning: bool,
    /// Accept the PostgreSQL `INHERITS (<parent>, ...)` legacy table-inheritance clause
    /// (`CREATE TABLE t (…) INHERITS (parent)`). On for PostgreSQL/Lenient. Off for
    /// ANSI/MySQL/SQLite/DuckDB — none have table inheritance (DuckDB, which otherwise inherits
    /// the PostgreSQL schema surface, rejects it), so the `INHERITS` keyword is left unconsumed
    /// and surfaces as a clean parse error.
    pub table_inheritance: bool,
    /// Accept the PostgreSQL `LIKE <source> [{INCLUDING | EXCLUDING} <feature> …]` source-table
    /// copy *element* inside the parenthesized `CREATE TABLE` definition list (`CREATE TABLE t
    /// (LIKE src INCLUDING ALL)`). On for PostgreSQL/Lenient. Off for ANSI/MySQL/SQLite/DuckDB:
    /// DuckDB rejects the element form, and MySQL's `CREATE TABLE t LIKE src` is a distinct
    /// *statement-level* production (no parentheses), not this element — so when off, a `LIKE` at
    /// an element position surfaces as a clean parse error.
    pub like_source_table: bool,
    /// Accept MySQL's statement-level `CREATE TABLE t LIKE <source>` table-clone body and its
    /// parenthesized twin `CREATE TABLE t (LIKE <source>)` — a whole-statement production that
    /// copies an existing table's definition, distinct from the PostgreSQL copy *element* gated
    /// by [`like_source_table`](CreateTableClauseSyntax::like_source_table). The source is a single bare (qualified)
    /// name: no `{INCLUDING | EXCLUDING} <feature>` options, no co-element, no trailing table
    /// options (`LIKE src ENGINE=…`, `(LIKE src, x INT)`, `(LIKE src INCLUDING ALL)` are all
    /// `ER_PARSE_ERROR` on mysql:8.4). On for MySQL/Lenient. Off for ANSI/PostgreSQL/SQLite/
    /// DuckDB, where a `LIKE` after the table name (or as the first token inside `(`) is left
    /// unconsumed and surfaces as a clean parse error — PostgreSQL rejects the bare form at raw
    /// parse, and only reads `(LIKE src …)` as the element form above. When both this and
    /// [`like_source_table`](CreateTableClauseSyntax::like_source_table) are on (Lenient), the parenthesized `(LIKE
    /// …)` reads as the more general PostgreSQL element (a superset that also admits the feature
    /// options), so this flag governs only the bare form there; MySQL, with the element flag off,
    /// takes both spellings onto this body.
    pub statement_level_table_like: bool,
    /// Accept `CREATE UNLOGGED TABLE` — the non-WAL-logged persistence keyword. On for
    /// PostgreSQL/DuckDB/Lenient (DuckDB parses it as a no-op). Off for ANSI/MySQL/SQLite, where
    /// `UNLOGGED` after `CREATE` is left unconsumed and surfaces as a clean parse error. In
    /// PostgreSQL's grammar `UNLOGGED` is a peer of `TEMP`/`TEMPORARY`, so the two are mutually
    /// exclusive; the parser rejects `CREATE TEMP UNLOGGED TABLE` accordingly.
    pub unlogged_tables: bool,
    /// Accept the trailing `USING <access_method>` table access-method clause (PostgreSQL
    /// `CREATE TABLE … USING heap`). On for PostgreSQL/Lenient. Off for ANSI/MySQL/SQLite/DuckDB
    /// (DuckDB has no pluggable table access methods), where the `USING` keyword after the table
    /// body is left unconsumed and surfaces as a clean parse error. Distinct from the CREATE
    /// INDEX `USING <method>` clause gated by [`index_using_method`](IndexAlterSyntax::index_using_method).
    pub table_access_method: bool,
    /// Accept the legacy `WITHOUT OIDS` trailing option (PostgreSQL) — kept as an accepted
    /// no-op. On for PostgreSQL/Lenient. Off for ANSI/MySQL/SQLite/DuckDB, where the `WITHOUT`
    /// keyword (absent SQLite's `WITHOUT ROWID`, a separate
    /// [`without_rowid_table_option`](CreateTableClauseSyntax::without_rowid_table_option) option) is left unconsumed
    /// and surfaces as a clean parse error.
    pub without_oids: bool,
    /// Accept the `CREATE TABLE t OF <type> [(…)]` typed-table form (PostgreSQL): the table's
    /// column shape is drawn from a composite type. On for PostgreSQL/Lenient. Off for
    /// ANSI/MySQL/SQLite/DuckDB, where `OF` after the table name is left unconsumed and surfaces
    /// as a clean parse error.
    pub typed_tables: bool,
}

/// Dialect-owned `CREATE TABLE` column-definition syntax accepted by the parser.
///
/// The column-level decorations inside a `CREATE TABLE` definition — the attributes and
/// restrictions that attach to a single column. Split out of the retired
/// `SchemaChangeSyntax` at its 16-field line as the column-definition axis, distinct from
/// the table-clause and constraint axes. Each flag is a grammar gate: when off the
/// decoration is left unconsumed and rejects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnDefinitionSyntax {
    /// Accept the keywordless generated-column shorthand `<col> <type> AS (<expr>)
    /// [STORED|VIRTUAL]`, written without the leading `GENERATED ALWAYS` (MySQL,
    /// SQLite). It folds onto the one [`GeneratedColumn`](crate::ast::GeneratedColumn)
    /// shape tagged
    /// [`GeneratedColumnSpelling::Shorthand`](crate::ast::GeneratedColumnSpelling),
    /// never a new node. PostgreSQL requires the `GENERATED ALWAYS`
    /// keywords, so this is off there and the bare `AS (…)` after a column type is left
    /// unconsumed and surfaces as a clean parse error.
    pub generated_column_shorthand: bool,
    /// Accept the SQLite column-level `ON CONFLICT <resolution>` clause on an inline
    /// `NOT NULL` / `UNIQUE` / `PRIMARY KEY` / `CHECK` constraint
    /// (`a INTEGER UNIQUE ON CONFLICT REPLACE`), recording the resolution algorithm in
    /// [`ColumnConstraint::conflict`](crate::ast::ColumnConstraint). SQLite-only; off in
    /// every other preset, where the trailing `ON` after the constraint is left unconsumed
    /// and surfaces as a clean parse error. Split out of
    /// the retired `sqlite_table_decorations` bundle because column-level
    /// conflict resolution is an independent grammar point from the trailing table options,
    /// the typeless column, `AUTOINCREMENT`, and the inline-`PRIMARY KEY` ordering.
    pub column_conflict_resolution_clause: bool,
    /// Accept a SQLite *typeless* column definition — a column named with no data type
    /// (`CREATE TABLE t (a, b)`), leaving [`ColumnDef::data_type`](crate::ast::ColumnDef)
    /// unset. SQLite-only; off in every other preset, where a column with no type falls
    /// through to `parse_data_type` and surfaces as a clean parse error. Split out of
    /// the retired `sqlite_table_decorations` bundle because the typeless
    /// column is an independent grammar point from the trailing table options,
    /// `AUTOINCREMENT`, the column `COLLATE`, and the inline-`PRIMARY KEY` ordering.
    pub typeless_column_definitions: bool,
    /// Accept a column that omits its data type *only* when the column is a generated column —
    /// DuckDB's narrowing of the SQLite typeless rule. DuckDB requires a data type on every
    /// column except a generated one: both the `GENERATED { ALWAYS | BY DEFAULT } AS …` form
    /// and the keywordless `AS (<expr>)` shorthand may drop the type
    /// (`CREATE TABLE t (x INT, gen_x AS (x + 5))`, engine-measured on libduckdb 1.5.4), while
    /// a plain typeless column (`x`) or a typeless non-generated constraint (`y DEFAULT 5`) is
    /// a parse error. On for DuckDB/Lenient. Off for ANSI/PostgreSQL/MySQL — a generated column
    /// with no type falls through to `parse_data_type` and surfaces as a clean parse error — and
    /// off for SQLite, which instead accepts *any* typeless column via the strictly wider
    /// [`typeless_column_definitions`](ColumnDefinitionSyntax::typeless_column_definitions), so
    /// this narrow gate stays off there.
    pub typeless_generated_columns: bool,
    /// Accept the SQLite joined `AUTOINCREMENT` column attribute — the bare one-word keyword
    /// on an inline `PRIMARY KEY` column (`a INTEGER PRIMARY KEY AUTOINCREMENT`), recorded as
    /// [`AutoIncrementSpelling::Joined`](crate::ast::AutoIncrementSpelling::Joined) so it
    /// round-trips as one word. SQLite-only; off in every other preset, where the trailing
    /// `AUTOINCREMENT` is left unconsumed and surfaces as a clean parse error. This gates
    /// *only* the joined spelling: the underscored MySQL `AUTO_INCREMENT` attribute is a
    /// separate surface gated by [`table_options`](CreateTableClauseSyntax::table_options), so the two spellings
    /// toggle independently and neither preset admits the other's word. Split out of
    /// the retired `sqlite_table_decorations` bundle because the auto-increment
    /// attribute is an independent grammar point from the typeless column, the column
    /// `COLLATE`, and the inline-`PRIMARY KEY` ordering.
    pub joined_autoincrement_attribute: bool,
    /// Accept the underscored MySQL `AUTO_INCREMENT` column attribute
    /// (`a INT AUTO_INCREMENT`), recorded as
    /// [`AutoIncrementSpelling::Underscored`](crate::ast::AutoIncrementSpelling::Underscored)
    /// so it round-trips with the underscore. Its own gate — one behaviour = one flag —
    /// rather than a rider on [`table_options`](CreateTableClauseSyntax::table_options),
    /// so a preset can admit the column attribute without MySQL's whole trailing
    /// table-option vocabulary (QuiltDB does exactly that: the attribute is
    /// SERIAL-equivalent there while `ENGINE = …` options stay parse errors). On for
    /// MySQL and Lenient (whose `table_options` previously implied it — same accepted
    /// surface) and QuiltDB; off elsewhere, where the trailing `AUTO_INCREMENT` is left
    /// unconsumed and surfaces as a clean parse error. The joined SQLite spelling stays
    /// separately gated by
    /// [`joined_autoincrement_attribute`](Self::joined_autoincrement_attribute), so the
    /// two spellings toggle independently.
    pub underscored_autoincrement_attribute: bool,
    /// Accept an `ASC`/`DESC` sort-order qualifier on an inline `PRIMARY KEY` column
    /// constraint (`CREATE TABLE t (a INTEGER PRIMARY KEY DESC)`), recorded in the
    /// [`ColumnOption::PrimaryKey`](crate::ast::ColumnOption) `ascending` field (`ASC` →
    /// `Some(true)`, `DESC` → `Some(false)`, absent → `None`). SQLite-only; off in every other
    /// preset, where the trailing `ASC`/`DESC` is left unconsumed and surfaces as a clean parse
    /// error. Split out of the retired `sqlite_table_decorations` bundle because
    /// the inline primary-key ordering is an independent grammar point from the trailing table
    /// options, the typeless column, `AUTOINCREMENT`, and the column `COLLATE`.
    pub inline_primary_key_ordering: bool,
    /// Accept a `CONSTRAINT <name>` prefix on a column `COLLATE` clause
    /// (`CREATE TABLE t (a TEXT CONSTRAINT c COLLATE nocase)`): SQLite's grammar makes `COLLATE`
    /// an ordinary nameable column constraint, so a `CONSTRAINT <name>` symbol binds to it.
    /// SQLite-only; off in every other preset (PostgreSQL and DuckDB engine-measured reject the
    /// named form, where `COLLATE any_name` is a constraint alternative parallel to — not under —
    /// the nameable constraint element), leaving the `CONSTRAINT <name>` prefix on a `COLLATE`
    /// clause as a clean parse error. This gates *only* the named wrapper: the bare column
    /// `COLLATE` surface ([`ColumnOption::Collate`](crate::ast::ColumnOption)) is a separate
    /// cross-dialect shape gated by [`column_collation`](ColumnDefinitionSyntax::column_collation), so the accepting
    /// case needs both flags on. Split out of
    /// the retired `sqlite_table_decorations` bundle because the named-`COLLATE`
    /// wrapper is an independent grammar point from the inline-`PRIMARY KEY` ordering.
    pub named_column_collate_constraint: bool,
    /// Accept the `<col> <type> GENERATED { ALWAYS | BY DEFAULT } AS IDENTITY [(…)]`
    /// identity column (SQL:2003; PostgreSQL). On for ANSI/PostgreSQL/MySQL/DuckDB/
    /// Lenient. SQLite has no `IDENTITY` (its auto-key is `INTEGER PRIMARY KEY
    /// [AUTOINCREMENT]`), so it is off; the `IDENTITY` keyword is then left unconsumed and
    /// surfaces as a clean parse error. Gates only the `AS IDENTITY` reading — the
    /// `GENERATED ALWAYS AS (<expr>)` computed column (which SQLite has) is unaffected.
    pub identity_columns: bool,
    /// Accept the compact identity-column forms `<col> <type> IDENTITY` and
    /// `<col> <type> IDENTITY(<seed>, <increment>)`. The two numeric arguments map to the
    /// same start/increment identity options as the standard generated form.
    pub compact_identity_columns: bool,
    /// Require a *parenthesized* `DEFAULT (expr)` for a functional / general-expression
    /// column default. On for MySQL: a column `DEFAULT` there admits only a literal, a
    /// signed literal, the `CURRENT_TIMESTAMP`/`NOW()`/`LOCALTIME`/`LOCALTIMESTAMP` temporal
    /// family, or a parenthesized expression — a bare function call or operator expression
    /// (`DEFAULT UUID()`, `DEFAULT 1 + 2`) is an `ER_PARSE_ERROR` on mysql:8, while
    /// `DEFAULT (UUID())` parses. Off for ANSI/PostgreSQL/SQLite/DuckDB/Lenient, which accept
    /// a bare expression default. When off, the default expression is read whole with no
    /// wrapping requirement.
    pub default_expression_requires_parens: bool,
    /// Restrict a *column-constraint* `DEFAULT` to PostgreSQL's `b_expr` grammar class
    /// (`ColConstraintElem: DEFAULT b_expr`), rather than the full `a_expr` used everywhere
    /// else. `b_expr` is the "boolean-and-predicate-free" expression production: it keeps
    /// arithmetic, comparison, `||`, `OPERATOR(...)`, `::`, subscripts, `COLLATE`, and the
    /// `IS [NOT] DISTINCT FROM` / `IS [NOT] DOCUMENT` tests, but excludes the `a_expr`-only
    /// forms — `AND`/`OR`/`NOT`, `IN`, `BETWEEN`, `LIKE`/`ILIKE`/`SIMILAR TO`, `IS [NOT]
    /// NULL`/`TRUE`/`FALSE`/`UNKNOWN`, a quantified `= ANY(...)`, and `AT TIME ZONE`. So
    /// `... DEFAULT 1 IN (1, 2)` is a syntax error on PostgreSQL (the `IN` predicate is
    /// `a_expr`-level), while the parenthesized `DEFAULT (1 IN (1, 2))` — a `c_expr` reset to
    /// `a_expr` — parses. On for PostgreSQL only; every other dialect (including DuckDB, which
    /// otherwise inherits the PostgreSQL schema surface) reads the default as a full `a_expr`.
    /// Note the asymmetry: `ALTER COLUMN ... SET DEFAULT` is `a_expr` even on PostgreSQL, so
    /// this gates only the inline column-constraint site.
    pub column_default_requires_b_expr: bool,
    /// Accept the column-definition `COLLATE <collation>` clause (`a text COLLATE "C"`). On for
    /// PostgreSQL/SQLite/DuckDB/Lenient — all three spell a per-column collation inside the
    /// column definition (PostgreSQL takes a qualified `any_name`, SQLite/DuckDB a single bare
    /// identifier; the parser narrows the name grammar per dialect). Off for ANSI/MySQL, where a
    /// column-level `COLLATE` is left unconsumed and surfaces as a clean parse error. Distinct
    /// from the expression-level `COLLATE` gated by
    /// [`ExpressionSyntax::collate`] on a different sub-struct — this one
    /// qualifies a column, not an expression. (MySQL *does* spell a column `COLLATE` in its own
    /// `CHARACTER SET … COLLATE …` attribute grammar, a distinct surface left to a follow-up.)
    pub column_collation: bool,
    /// Accept the per-column `STORAGE {PLAIN | EXTERNAL | EXTENDED | MAIN | DEFAULT}` and
    /// `COMPRESSION <method>` physical-storage clauses (PostgreSQL). The two travel together as a
    /// single dialect unit — both are fixed-position clauses between a column's type and its
    /// constraint list. On for PostgreSQL/Lenient. Off for ANSI/MySQL/SQLite/DuckDB (DuckDB,
    /// which otherwise inherits the PostgreSQL schema surface, rejects both), where the
    /// `STORAGE`/`COMPRESSION` keyword is left unconsumed and surfaces as a clean parse error.
    pub column_storage: bool,
}

/// Dialect-owned table/column-constraint syntax accepted by the parser.
///
/// The constraint forms and their decorations. Split out of the retired
/// `SchemaChangeSyntax` at its 16-field line as the constraint axis, distinct from the
/// table-clause and column-definition axes. Each flag is a grammar gate: when off the
/// constraint keyword is left unconsumed and rejects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstraintSyntax {
    /// Accept a trailing `<constraint characteristics>` clause (`[NOT] DEFERRABLE`,
    /// `INITIALLY {DEFERRED | IMMEDIATE}`) on a table/column constraint, in both `CREATE
    /// TABLE` and `ALTER TABLE ADD CONSTRAINT`. On for PostgreSQL/SQLite/DuckDB/Lenient.
    /// MySQL has no deferrable constraints — `… REFERENCES t (c) DEFERRABLE` / `INITIALLY
    /// DEFERRED` are `ER_PARSE_ERROR` on mysql:8 — so it is off there and the
    /// `DEFERRABLE`/`INITIALLY` keyword surfaces as a clean parse error.
    pub deferrable_constraints: bool,
    /// Accept a `CONSTRAINT <symbol>` name prefix on a *non-CHECK* inline column constraint
    /// (`a INT CONSTRAINT c REFERENCES b`, `… CONSTRAINT c UNIQUE`). On for ANSI/PostgreSQL/
    /// SQLite/DuckDB/Lenient. MySQL admits a named inline constraint only for `CHECK`
    /// (`a INT CONSTRAINT c CHECK (…)` parses on mysql:8), so a `CONSTRAINT <symbol>` before
    /// any other inline column constraint — `REFERENCES`, `UNIQUE`, `PRIMARY KEY`,
    /// `NOT NULL` — is an `ER_PARSE_ERROR`; it is off there. The bare, unnamed inline
    /// constraints and the named `CHECK` are unaffected either way.
    pub named_inline_non_check_constraints: bool,
    /// Accept a trailing bodyless `CONSTRAINT <name>` — a nameable constraint marker with no
    /// constraint element after it — in a column definition (`a INT CONSTRAINT cn`) or as a
    /// standalone table constraint (`CREATE TABLE t (a INT, CONSTRAINT cn)`), recorded as
    /// [`ColumnOption::Bare`](crate::ast::ColumnOption)/[`TableConstraint::Bare`](crate::ast::TableConstraint).
    /// SQLite's grammar makes the constraint element optional after `CONSTRAINT <name>`, and (in
    /// the table-constraint list only) lets the separating comma before such a bare marker be
    /// omitted too — `UNIQUE(a) CONSTRAINT c` and `CONSTRAINT a UNIQUE(x) CONSTRAINT b` both
    /// engine-measured accept, any number of bare/named-with-body constraints chaining freely.
    /// Parsed only when nothing but the element terminator (`,`/`)`) follows the name — a
    /// `CONSTRAINT <name>` immediately followed by a constraint element (`CHECK`, `UNIQUE`, …)
    /// still takes that element as its body, unaffected by this flag. On for SQLite/Lenient. Off
    /// elsewhere (PostgreSQL engine-measured rejects a bodyless `CONSTRAINT <name>`, requiring a
    /// constraint element), where a `CONSTRAINT <name>` with nothing following is a clean parse
    /// error.
    pub bare_constraint_name: bool,
    /// Accept the PostgreSQL `EXCLUDE [USING <method>] (<element> WITH <operator> [, ...]) [tail]`
    /// exclusion constraint as a table element (`CREATE TABLE t (c circle, EXCLUDE USING gist (c
    /// WITH &&))`). On for PostgreSQL/Lenient. Off for ANSI/MySQL/SQLite/DuckDB (DuckDB, which
    /// otherwise inherits the PostgreSQL schema surface, rejects it), where `EXCLUDE` at a
    /// constraint position is left unconsumed and surfaces as a clean parse error.
    pub exclusion_constraints: bool,
    /// Accept the `NO INHERIT` / `NOT VALID` constraint markers on `CHECK` (and `NOT VALID` on
    /// `FOREIGN KEY`) constraints — PostgreSQL's shared `ConstraintAttributeSpec` slot, also
    /// accepted by DuckDB. On for PostgreSQL/DuckDB/Lenient. Off for ANSI/MySQL/SQLite, where the
    /// `NO INHERIT` / `NOT VALID` keywords after a constraint are left unconsumed and surface as a
    /// clean parse error. The parser enforces which constraint kinds admit which marker
    /// (PostgreSQL rejects `NOT VALID` on `PRIMARY KEY`/`UNIQUE`/`EXCLUDE` and `NO INHERIT` on
    /// everything but `CHECK`, in the grammar action — reproduced at parse), so the flag only
    /// opens the keywords, not every combination.
    pub constraint_no_inherit_not_valid: bool,
    /// Accept the PostgreSQL index-backed-constraint parameters on `UNIQUE`/`PRIMARY KEY`: the
    /// covering `INCLUDE (<col>, ...)` list, the `NULLS [NOT] DISTINCT` null-treatment (PG 15+),
    /// and the `USING INDEX TABLESPACE <name>` index tablespace. The three travel together as one
    /// PostgreSQL `ConstraintElem` index-parameter unit (the same "single dialect unit" rationale
    /// as [`column_storage`](ColumnDefinitionSyntax::column_storage)). On for PostgreSQL/Lenient. Off for
    /// ANSI/MySQL/SQLite/DuckDB (DuckDB rejects all three), where the `INCLUDE`/`NULLS`/`USING`
    /// keyword is left unconsumed and surfaces as a clean parse error.
    pub index_constraint_parameters: bool,
    /// Accept a per-column `COLLATE <collation>` and `ASC`/`DESC` sort order inside a
    /// `PRIMARY KEY (...)` / `UNIQUE (...)` table-constraint column list — SQLite's
    /// "indexed-column" spelling in constraint position (`PRIMARY KEY (a COLLATE nocase)`,
    /// `UNIQUE ('b' COLLATE nocase DESC)`). On for SQLite/Lenient.
    ///
    /// Engine-measured scope (constraint position, *not* `CREATE INDEX`): SQLite admits a bare
    /// column name optionally decorated with `COLLATE`/`ASC`/`DESC`, but prohibits general
    /// expressions (`UNIQUE (a+b)` / `(lower(a))` → "expressions prohibited in PRIMARY KEY and
    /// UNIQUE constraints") and `NULLS FIRST`/`LAST` ("unsupported use of NULLS FIRST"), so the
    /// widened parser stays column-name + `COLLATE` + `ASC`/`DESC` and never fills
    /// [`IndexColumn::nulls_first`](crate::ast::IndexColumn). Off for ANSI/PostgreSQL/DuckDB
    /// (all engine-measured reject `COLLATE`/`ASC`/`DESC` here — the decoration belongs to their
    /// `CREATE INDEX` grammar, not the table constraint), where the keyword is left unconsumed
    /// and surfaces as a clean parse error. Also off for MySQL: its `key_part` admits `ASC`/`DESC`
    /// and length prefixes / functional `(expr)` parts but not `COLLATE`, a differently-shaped
    /// surface with no corpus demand — left off and scoped out rather than modelled as this
    /// SQLite-shaped gate.
    pub constraint_column_collate_order: bool,
    /// Accept the cascading referential actions `ON {DELETE|UPDATE} {CASCADE | SET NULL |
    /// SET DEFAULT}` on a foreign-key constraint. On for ANSI/PostgreSQL/MySQL/SQLite/Lenient.
    /// DuckDB parse-rejects those three actions ("FOREIGN KEY constraints cannot use CASCADE,
    /// SET NULL or SET DEFAULT", probed on 1.5.4) while still admitting `RESTRICT` and
    /// `NO ACTION`, so it is off there; with the flag off those three keywords after
    /// `ON DELETE`/`ON UPDATE` surface as a clean parse error. Distinct from admitting the
    /// `ON DELETE`/`ON UPDATE` clause itself — `RESTRICT`/`NO ACTION` remain free either way.
    pub referential_action_cascade_set: bool,
    /// Accept a subquery (or `EXISTS` / `IN (SELECT …)`) inside a `CHECK` constraint
    /// expression. On for ANSI/PostgreSQL/MySQL/Lenient. Off for SQLite and DuckDB, both of
    /// which parse-reject subqueries in `CHECK` ("subqueries prohibited in CHECK constraints",
    /// engine-measured); with the flag off a subquery in the CHECK body is a clean parse error.
    pub check_constraint_subqueries: bool,
}

/// Dialect-owned `CREATE INDEX` / `ALTER TABLE` / `DROP` syntax accepted by the parser.
///
/// The clause-level gates on the index, alter-table, and drop statements — the object-DDL
/// surface that decorates an already-dispatched statement rather than deciding its leading
/// keyword. Split out of the retired `SchemaChangeSyntax` at its 16-field line as the
/// index/alter/drop axis, distinct from the whole-statement-dispatch axis. Each flag is a
/// grammar gate: when off the keyword is left unconsumed and rejects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexAlterSyntax {
    /// Accept PostgreSQL `ALTER TABLE … RENAME CONSTRAINT <old> TO <new>`.
    pub rename_constraint: bool,
    /// Accept `ALTER TABLE … SET (<name> = <value>, …)` table options.
    pub alter_table_set_options: bool,
    /// Accept the unnamed `ALTER TABLE … DROP PRIMARY KEY` action.
    pub drop_primary_key: bool,
    /// Accept `ALTER TABLE … ALTER COLUMN <name> ADD GENERATED {ALWAYS | BY DEFAULT}
    /// AS IDENTITY [(…)]`.
    pub alter_column_add_identity: bool,
    /// Accept `CREATE INDEX … (<keys>) WITH (<name> = <value>, …)` storage parameters.
    pub index_storage_parameters: bool,
    /// Accept a trailing `CASCADE` / `RESTRICT` drop behaviour on `DROP` statements
    /// and `ALTER TABLE ... DROP` actions (the SQL standard `<drop behavior>`;
    /// PostgreSQL). A dialect that does not model dependency behaviour leaves it off.
    pub drop_behavior: bool,
    /// Accept MySQL's `DROP INDEX <name> ON <table> [ALGORITHM [=] {DEFAULT | INPLACE |
    /// INSTANT | COPY}] [LOCK [=] {DEFAULT | NONE | SHARED | EXCLUSIVE}]` — the index drop
    /// that names its owning table with a mandatory `ON` and carries the online-DDL
    /// `ALGORITHM`/`LOCK` execution hints (`drop_index_stmt`/`opt_index_lock_and_algorithm`,
    /// `sql_yacc.yy`). Server-measured on mysql:8: the `ON <table>` is mandatory (`DROP INDEX
    /// i` with no `ON` is `ER_PARSE_ERROR`), the tail admits at most one `ALGORITHM` and one
    /// `LOCK` in either order, and no trailing `CASCADE`/`RESTRICT`. On for MySQL only. Off
    /// elsewhere, where `DROP INDEX <name> [, …]` stays the shared name-list drop; enabling it
    /// would make the mandatory-`ON` form displace that name-list drop, so Lenient forgoes it
    /// to keep the more permissive bare-name form (a documented conflict resolution). With the
    /// flag off a `DROP INDEX i ON t` leaves `ON` unconsumed and surfaces as a clean parse
    /// error.
    pub index_drop_on_table: bool,
    /// Accept the MySQL `ALTER TABLE … DROP {INDEX | KEY} <name>` action (an index drop
    /// nested inside `ALTER TABLE`, distinct from the standalone `DROP INDEX` governed by
    /// [`index_drop_on_table`](Self::index_drop_on_table)). No preset models the *parse* of
    /// this action yet, so it is `false` everywhere; with the flag off, a `DROP INDEX`/`DROP
    /// KEY` followed by a name is a clean parse error naming the keyword rather than
    /// swallowing it as the dropped column's name. A dialect that adds the parse flips this
    /// to `true` and handles the action.
    pub alter_table_drop_index: bool,
    /// Accept `CREATE INDEX CONCURRENTLY` (PostgreSQL builds the index without an
    /// exclusive table lock). Not ANSI.
    pub index_concurrently: bool,
    /// Accept the `CREATE INDEX … USING <method>` access-method clause (PostgreSQL
    /// `btree`/`hash`/`gin`/`gist`/…; MySQL `USING BTREE`). Not ANSI.
    pub index_using_method: bool,
    /// Accept a trailing `WHERE <predicate>` partial-index clause on `CREATE INDEX`
    /// (PostgreSQL, SQLite). Not ANSI.
    pub partial_index: bool,
    /// Accept the `IF NOT EXISTS` guard on `CREATE INDEX` (PostgreSQL/SQLite/DuckDB). On for
    /// ANSI/PostgreSQL/SQLite/DuckDB/Lenient. MySQL has no `CREATE INDEX IF NOT EXISTS`
    /// (engine-measured `ER_PARSE_ERROR` on mysql:8), so it is off; the `IF NOT EXISTS` is
    /// then left unconsumed and the following index name surfaces as a clean parse error.
    /// Distinct from the `CREATE TABLE`/`CREATE DATABASE` guards, which MySQL *does* admit.
    pub index_if_not_exists: bool,
    /// Accept the per-key `NULLS FIRST` / `NULLS LAST` null-ordering modifier on a
    /// `CREATE INDEX` column (PostgreSQL/SQLite 3.30+). On for ANSI/PostgreSQL/SQLite/DuckDB/
    /// Lenient. MySQL has no index-key `NULLS` ordering (engine-measured `ER_PARSE_ERROR` on
    /// mysql:8; it orders NULLs implicitly), so it is off; the `NULLS` keyword is then left
    /// unconsumed and surfaces as a clean parse error. Independent of the `ORDER BY` null
    /// ordering, which is a separate grammar position.
    pub index_nulls_order: bool,
    /// Accept the extended `ALTER TABLE` surface beyond SQLite's lenient action set: the
    /// table-level `IF EXISTS`, comma-separated multiple actions, `ALTER COLUMN …`, the
    /// `ADD PRIMARY KEY`/`UNIQUE`/`FOREIGN KEY` table constraints, and the `IF [NOT]
    /// EXISTS` guard on `ADD`/`DROP COLUMN` (PostgreSQL/MySQL). On for ANSI/PostgreSQL/
    /// MySQL/DuckDB/Lenient. Off for SQLite, whose `ALTER TABLE` (engine-measured via
    /// rusqlite) admits `RENAME TO`/`RENAME COLUMN`, a single `ADD [COLUMN] <def>` / `ADD
    /// [CONSTRAINT …] CHECK (…)`, and a single `DROP [COLUMN]` / `DROP CONSTRAINT` — those
    /// stay accepted with the flag off; every other form is left unconsumed and surfaces
    /// as a clean parse error. Distinct from [`ExistenceGuards::if_exists`], which stays
    /// on for SQLite (its `DROP TABLE IF EXISTS` is valid) — only the *`ALTER`* existence
    /// guard is gated here.
    pub alter_table_extended: bool,
    /// Accept a comma-separated multi-action `ALTER TABLE` list
    /// (`ALTER TABLE t ADD COLUMN a INT, DROP COLUMN b`). On for ANSI/PostgreSQL/MySQL/Lenient.
    /// Off for DuckDB (parse-rejects with "Only one ALTER command per statement is supported",
    /// probed on 1.5.4) and moot for SQLite (its
    /// [`alter_table_extended`](IndexAlterSyntax::alter_table_extended) is already off, so the
    /// multi-action loop is never entered). Only reachable where `alter_table_extended` is on;
    /// with the flag off the first action parses and a trailing comma surfaces as a clean parse
    /// error.
    pub alter_table_multiple_actions: bool,
    /// Accept an `IF EXISTS` / `IF NOT EXISTS` existence guard *inside* `ALTER TABLE` — the
    /// table-level `ALTER TABLE IF EXISTS t …` and the per-action `ADD COLUMN IF NOT EXISTS`
    /// / `DROP [COLUMN|CONSTRAINT] IF EXISTS` guards. On for PostgreSQL/DuckDB/Lenient (and
    /// unused by SQLite, whose non-[`alter_table_extended`](IndexAlterSyntax::alter_table_extended) path
    /// parses no guard, so it rides that gate —
    /// [`FeatureDependencyViolation::AlterExistenceGuardsWithoutAlterTableExtended`]). MySQL
    /// supports the extended `ALTER TABLE` surface (multi-action
    /// lists, `ADD`/`DROP CONSTRAINT`, `ALTER COLUMN`) but *not* these guards
    /// (`ALTER TABLE IF EXISTS`, `ADD COLUMN IF NOT EXISTS`, `DROP COLUMN IF EXISTS` are each
    /// `ER_PARSE_ERROR` on mysql:8), so it is off there; the `IF` keyword is then read as a
    /// name and surfaces as a clean parse error. Distinct from
    /// [`ExistenceGuards::if_exists`](ExistenceGuards::if_exists), which MySQL keeps on for
    /// `DROP TABLE IF EXISTS`.
    pub alter_existence_guards: bool,
    /// Accept dotted nested-column paths in DuckDB `ALTER TABLE` column targets for the
    /// actions the engine parses (`ADD COLUMN s.k`, `DROP COLUMN s.k`, and old-side
    /// `RENAME COLUMN s.k TO k2`). The gate does not affect `ALTER COLUMN` or the rename
    /// destination: DuckDB 1.5.4 parse-rejects dotted paths in those positions.
    pub alter_nested_column_paths: bool,
    /// Accept the PostgreSQL `ALTER TABLE … ALTER COLUMN` actions beyond `SET`/`DROP
    /// DEFAULT` — `SET DATA TYPE <type>` (and its bare `TYPE <type>` synonym, with an
    /// optional `USING <expr>`) plus `SET`/`DROP NOT NULL`. On for PostgreSQL/DuckDB/Lenient.
    /// MySQL's `ALTER COLUMN` admits only `SET`/`DROP DEFAULT` — it changes a column's type
    /// with `MODIFY`/`CHANGE` — so `ALTER COLUMN i SET DATA TYPE …`/`SET NOT NULL`/`DROP NOT
    /// NULL` are `ER_PARSE_ERROR` on mysql:8; with the flag off those actions surface as a
    /// clean parse error. SQLite never reaches the action (its
    /// [`alter_table_extended`](IndexAlterSyntax::alter_table_extended) is off), so this is moot there —
    /// it rides that gate
    /// ([`FeatureDependencyViolation::AlterColumnSetDataTypeWithoutAlterTableExtended`]).
    pub alter_column_set_data_type: bool,
    /// Accept a routine reference's parenthesized argument-type list — `DROP FUNCTION f(INT)`,
    /// `GRANT EXECUTE ON FUNCTION f(INT) …` (PostgreSQL overload-disambiguation). On for
    /// ANSI/PostgreSQL/DuckDB/Lenient. MySQL identifies a routine by name alone, so the arg
    /// list is a syntax error there (`DROP FUNCTION f(INT)` → engine-measured `ER_PARSE_ERROR`
    /// on mysql:8); with the flag off the `(` is left unconsumed and surfaces as a clean parse
    /// error. Off for SQLite too (it has no stored routines — [`routines`](StatementDdlGates::routines) is
    /// already off, so this is moot there).
    pub routine_arg_types: bool,
    /// Accept a `CREATE FUNCTION` parameter default — `func_arg DEFAULT <expr>` / `func_arg =
    /// <expr>` (PostgreSQL `func_arg_with_default`). On for ANSI/PostgreSQL/DuckDB/Lenient.
    /// MySQL's routine parameters are `[IN|OUT|INOUT] name type` with no default, so a
    /// `DEFAULT`/`=` after the type is a syntax error there (`ER_PARSE_ERROR` on mysql:8); with
    /// the flag off the `DEFAULT`/`=` is left unconsumed and the parameter-list close surfaces
    /// as a clean parse error. Distinct from [`routine_arg_types`](Self::routine_arg_types)
    /// (which gates the *reference*-site arg-type list on `DROP`/`GRANT`): this gates the
    /// *definition*-site default on `CREATE FUNCTION`. Off for SQLite too (no stored routines —
    /// [`routines`](StatementDdlGates::routines) is already off, so this is moot there).
    pub routine_arg_defaults: bool,
    /// Accept a `CREATE FUNCTION` parameter argument mode — the `arg_class` prefix
    /// `IN`/`OUT`/`INOUT`/`VARIADIC` before the parameter (PostgreSQL `func_arg`). On for
    /// ANSI/PostgreSQL/DuckDB/Lenient. MySQL's `CREATE FUNCTION` parameters are `name type`
    /// with no mode (the `IN`/`OUT`/`INOUT` modes are a stored-*procedure* form, a distinct
    /// statement), so a mode keyword before a `CREATE FUNCTION` parameter is a syntax error
    /// there; with the flag off the keyword is left for the name/type parse (a reserved mode
    /// keyword like `IN`/`VARIADIC` then surfaces as a clean parse error). A sibling of
    /// [`routine_arg_defaults`](Self::routine_arg_defaults): both gate independent
    /// *definition*-site `CREATE FUNCTION` parameter facets that MySQL's grammar omits. Off
    /// for SQLite too (no stored routines — [`routines`](StatementDdlGates::routines) is
    /// already off, so this is moot there).
    pub routine_arg_modes: bool,
    /// Accept a string-constant (`Sconst`) spelling of the routine `LANGUAGE` name —
    /// `LANGUAGE 'sql'`/`E'sql'`/`$$sql$$` — alongside the bare word, PostgreSQL's
    /// `NonReservedWord_or_Sconst` operand (the same shape the `DO … LANGUAGE` argument
    /// spells). On for PostgreSQL/Lenient. MySQL's routine `LANGUAGE` admits only the bare
    /// word `SQL` — `LANGUAGE 'SQL'` is engine-measured `ER_PARSE_ERROR` (1064) on mysql:8 for
    /// both `CREATE FUNCTION` and `CREATE PROCEDURE` — so off there; with the flag off a string
    /// in the `LANGUAGE` position falls through to the bare-word parse, which rejects the string
    /// as MySQL does. Off for ANSI too: the SQL-standard `<language name>` is a bare identifier
    /// (`SQL`/`C`/`ADA`/…), not a string. A bit-string (`b'…'`/`x'…'`) or national (`N'…'`)
    /// constant is never an `Sconst`, so it stays a reject even where the flag is on (matching
    /// PostgreSQL). Off for SQLite too (no stored routines — [`routines`](StatementDdlGates::routines)
    /// is already off, so this is moot there).
    pub routine_language_string: bool,
}

/// Dialect-owned `IF [NOT] EXISTS` existence guards on DDL statements.
///
/// The SQL existence guards — `IF EXISTS` on `DROP`/`ALTER`, `IF NOT EXISTS` on
/// `CREATE` — are dialect data: each flag decides whether the parser admits
/// the guard at one statement site, and when off the `IF [NOT] EXISTS` is left
/// unconsumed and surfaces as a clean parse error (the same reject mechanism the other
/// grammar gates use). They live in their own sub-struct rather than scattered through
/// the retired `SchemaChangeSyntax` because their bundle membership differs per dialect: leaving a
/// coarse `if_exists` bundle in `SchemaChangeSyntax` made every new dialect bolt on a
/// separate exception flag (`view_if_not_exists`, `create_database_if_not_exists`), so a
/// dedicated per-site table is the shape that does not accrete one exception per dialect.
///
/// Two-level spelling convention (uniform across the dialect-data knobs): a top-level
/// [`FeatureSet`] assembly spells every field explicitly, while a *sub-preset* const of a
/// knob like this one may struct-update-derive from a sibling with `..Self::OTHER` (the
/// [`SelectSyntax::DUCKDB`] precedent) — the base preset stays the exhaustive source of
/// truth and the derived one records only its deltas.
///
/// [`SelectSyntax::DUCKDB`]: crate::dialect::SelectSyntax
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExistenceGuards {
    /// Accept `IF EXISTS` on `DROP`/`ALTER TABLE` (and on their column/constraint
    /// sub-actions) and `IF NOT EXISTS` on `ADD COLUMN` (PostgreSQL, MySQL, SQLite;
    /// not ANSI, whose `DROP`/`ALTER` has no existence guard). One flag gates the
    /// drop/alter/add-column existence guards together because a dialect that spells one
    /// spells them all; the `ALTER TABLE`/`ADD COLUMN` sites additionally require
    /// [`IndexAlterSyntax::alter_table_extended`] (SQLite keeps this guard on for its
    /// `DROP … IF EXISTS` while its plain-`ALTER` surface stays off).
    pub if_exists: bool,
    /// Accept `IF NOT EXISTS` on a *plain* (non-materialized) `CREATE VIEW` (SQLite).
    /// PostgreSQL admits `IF NOT EXISTS` only on a `CREATE MATERIALIZED VIEW` (that
    /// form is always accepted, independent of this flag), and MySQL has no view
    /// existence guard at all, so off there; SQLite spells `CREATE VIEW IF NOT EXISTS`
    /// over a regular view. When off, the `IF NOT EXISTS` is left unconsumed on a plain
    /// view and surfaces as a clean parse error.
    pub view_if_not_exists: bool,
    /// Accept the `CREATE DATABASE IF NOT EXISTS <name>` existence guard (MySQL; also
    /// SQLite has no `CREATE DATABASE` at all, so off there). PostgreSQL's `CREATE
    /// DATABASE` has no `IF NOT EXISTS`, so this is a dedicated site rather than a reuse
    /// of [`if_exists`](Self::if_exists) — that one is on in PostgreSQL, which must still
    /// reject `CREATE DATABASE IF NOT EXISTS`. When off, the `IF NOT EXISTS` is left
    /// unconsumed and surfaces as a clean parse error.
    pub create_database_if_not_exists: bool,
}

/// Dialect-owned SELECT-core syntax extensions accepted by the parser.
///
/// The SELECT-body forms — the projection / select-list, the set-operation-operand, and
/// the `VALUES`-constructor surface — after the row-limiting/locking query tail and the
/// grouping/ordering clauses split out into [`QueryTailSyntax`] and [`GroupingSyntax`] at
/// this struct's 16-field line. Acceptance is explicit dialect data, not a parser-side type
/// check: most flags are widening grammar gates (when off, the introducing keyword is left
/// unconsumed and the clause surfaces as a clean parse error), and a few are narrowing
/// well-formedness enforcements (per the flag-naming rule above).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectSyntax {
    /// Accept PostgreSQL `SELECT DISTINCT ON (<expr>, ...)`.
    pub distinct_on: bool,
    /// Accept PostgreSQL's `SELECT … INTO [TEMP] <table>` create-table form (the
    /// `INTO` target sits between the projection and `FROM`, materializing the result
    /// into a new relation). When off, `INTO` is left unconsumed and surfaces as a
    /// clean parse error. This gates *only* the create-table form: bare standard
    /// `SELECT … INTO <variable>` is PSM host/local-variable assignment — a different
    /// construct — so ANSI leaves this off, and MySQL has no `SELECT INTO <table>`.
    pub select_into: bool,
    /// Accept an empty SELECT target list — a projection with zero items (`SELECT`,
    /// `SELECT;`, `SELECT FROM t`, `SELECT WHERE …`). libpg_query's raw grammar makes
    /// the projection optional before any clause (PostgreSQL rejects it later, at
    /// parse-analysis, which is past our parse-level parity contract), so the honest
    /// closure accepts it under the PostgreSQL preset only. When off (ANSI/MySQL, which
    /// both require ≥1 select item), the projection's first-item requirement stands and
    /// a bare `SELECT` is a clean parse error.
    pub empty_target_list: bool,
    /// Accept DuckDB's `QUALIFY <predicate>` post-window filter clause, written after
    /// the `WINDOW` clause (DuckDB's grammar order: `… HAVING … WINDOW … QUALIFY …`;
    /// verified against DuckDB 1.5.4). When off (ANSI/PostgreSQL/MySQL/SQLite, none of
    /// which have the clause), the `QUALIFY` keyword is left unconsumed and surfaces
    /// as a clean parse error — the same reject mechanism the other SELECT gates use.
    /// This flag gates only the *clause*; whether `QUALIFY` is usable as a plain
    /// identifier is the orthogonal reservation data (the `reserved_*` keyword sets:
    /// DuckDB reserves it like `HAVING`, every other shipped dialect leaves it a free
    /// identifier). On for DuckDB / Lenient, off elsewhere.
    pub qualify: bool,
    /// Accept a string literal as a column alias (`SELECT 1 AS 'x'`, and MySQL's
    /// `SELECT 1 AS "x"` where `"…"` is a string), MySQL's rule. The alias parser admits
    /// a `String` token after `AS`, materialises its value as the alias identifier, and
    /// records the source quote ([`QuoteStyle::Single`](crate::ast::QuoteStyle::Single) /
    /// [`Double`](crate::ast::QuoteStyle::Double)) so it renders back quoted. When off,
    /// a string in alias position is left unconsumed and surfaces as a clean parse error
    /// (the standard requires an identifier). On for MySQL / Lenient, off elsewhere.
    pub alias_string_literals: bool,
    /// Accept a string literal as a *bare* (`AS`-less) column alias (`SELECT 1 'x'`), on
    /// top of the `AS`-introduced form [`alias_string_literals`](Self::alias_string_literals)
    /// gates. SQLite and MySQL read a string in bare-alias position as the column name
    /// (engine-measured: `SELECT 1 'x'` prepares on both, naming the column `x`), while
    /// **DuckDB accepts only the `AS 'x'` form and rejects the bare `SELECT 1 'x'`** (probed
    /// on 1.5.4) — so the bare position is its own axis rather than a rider on
    /// [`alias_string_literals`](Self::alias_string_literals). A separate axis rather than
    /// widening that flag because the two enablers split: DuckDB arms the `AS` form here but
    /// not the bare one. When off, a string in bare-alias position is left unconsumed and
    /// surfaces as a clean parse error. On for SQLite / MySQL / Lenient, off elsewhere.
    ///
    /// MySQL's bare string alias overlaps same-line adjacent-string concatenation
    /// ([`same_line_adjacent_concat`](StringLiteralSyntax::same_line_adjacent_concat)):
    /// `SELECT 'a' 'b'` is the single value `'ab'`, while `SELECT 1 'x'` is a bare alias
    /// (both engine-measured on mysql:8.4.10). No carve-out flag is needed — parse ordering
    /// resolves it: a string primary greedily folds every following unprefixed string
    /// continuation into its own value before the alias parser runs, so a trailing string
    /// reaches the bare-alias branch only when the preceding expression was not itself a
    /// string. A prefixed string (`N'…'`, `_charset'…'`, bit) is never a bare alias
    /// (rejected as an identifier), matching the engine's rejects (`SELECT 'a' _utf8'b'`).
    pub bare_alias_string_literals: bool,
    /// Accept DuckDB's FROM-first SELECT order: a query primary may lead with the
    /// `FROM` clause (`FROM <tables> [SELECT [DISTINCT] <projection>] …`), the projection
    /// written after it, or omitted entirely — the bare `FROM <tables>` is an implicit
    /// `SELECT *`. Semantically the ordinary SELECT (DuckDB serializes `FROM t SELECT x`
    /// and `SELECT x FROM t` to the same tree; probed on 1.5.4), so it parses to the
    /// canonical [`Select`](crate::ast::Select) tagged
    /// [`SelectSpelling::FromFirst`](crate::ast::SelectSpelling). The projection, when
    /// present, must sit immediately after the `FROM` clause (DuckDB syntax-errors on
    /// `FROM t WHERE x SELECT y` / `FROM t GROUP BY a SELECT a`), so it is parsed only in
    /// that position and every following clause parses in its ordinary place. When off
    /// (ANSI/PostgreSQL/MySQL/SQLite, none of which admit a statement-position `FROM`), a
    /// leading `FROM` is never a query start, so it surfaces as a clean parse error — the
    /// over-acceptance guard the differential oracle relies on. The gate is read wherever
    /// a query primary may begin (statement, set operand, scalar/`IN` subquery, CTE body,
    /// derived table), so the one flag composes everywhere. On for DuckDB / Lenient, off
    /// elsewhere.
    pub from_first: bool,
    /// Accept SQL's `<explicit table>` form `TABLE <name>` (equivalent to
    /// `SELECT * FROM <name>`). On for ANSI / PostgreSQL / DuckDB / MySQL / Lenient
    /// (engine-measured: libpg_query and libduckdb accept; mysql:8 accepts). Off for
    /// SQLite, which syntax-rejects a leading `TABLE` (engine-measured on rusqlite:
    /// `near "TABLE": syntax error`). When off, a leading `TABLE` is left undispatched
    /// and surfaces as an unknown statement.
    pub explicit_table: bool,
    /// Accept DuckDB's `UNION [ALL] BY NAME` name-matched set operation: pair the two
    /// inputs' columns by name (padding missing columns with NULL) instead of by
    /// position ([`SetExpr::SetOperation::by_name`](crate::ast::SetExpr) — a flag on
    /// the set-op node, orthogonal to `all`). DuckDB restricts `BY NAME` to `UNION`
    /// (`INTERSECT BY NAME` / `EXCEPT BY NAME` are syntax errors; probed on 1.5.4), so
    /// the parser consumes the modifier only after `UNION`; after `INTERSECT`/`EXCEPT`
    /// the `BY` keyword is left unconsumed and surfaces as the usual operand reject.
    /// When off (ANSI/PostgreSQL/MySQL/SQLite, none of which have the modifier), the
    /// `BY` after a set operator is likewise left unconsumed and a bare `UNION BY NAME`
    /// is a clean parse error — the same reject mechanism the other SELECT gates use.
    /// On for DuckDB / Lenient, off elsewhere.
    pub union_by_name: bool,
    /// Accept DuckDB's `*` / `t.*` wildcard modifiers — the `EXCLUDE (…)`,
    /// `REPLACE (expr AS col)`, and `RENAME (col AS new)` tail that rewrites which
    /// columns the wildcard expands to ([`WildcardOptions`](crate::ast::WildcardOptions)
    /// on the wildcard select item). DuckDB fixes their surface order (`EXCLUDE`, then
    /// `REPLACE`, then `RENAME`, each at most once; any other order is a syntax error,
    /// probed on 1.5.4), which the parser enforces by parsing them in that sequence.
    /// One flag, not three: DuckDB ships the trio as a single grammar production and no
    /// shipped dialect adopts a subset (the paired-flag doctrine). This gates the
    /// select-list/`RETURNING` wildcard tail; the sibling `COLUMNS(…)` expression form
    /// is [`CallSyntax::columns_expression`](CallSyntax::columns_expression),
    /// a separate grammar position (expression vs projection item). When off
    /// (ANSI/PostgreSQL/MySQL/SQLite), the `EXCLUDE`/`REPLACE`/`RENAME` keyword after a
    /// `*` is left unconsumed and surfaces as a clean parse error — the over-acceptance
    /// guard the differential oracle relies on. On for DuckDB / Lenient, off elsewhere.
    pub wildcard_modifiers: bool,
    /// Accept the `REPLACE (<expr> AS <column>, ...)` wildcard tail independently of
    /// the `EXCLUDE` and `RENAME` modifier family.
    pub wildcard_replace: bool,
    /// Accept `INTERSECT ALL`.
    pub intersect_all: bool,
    /// Accept `EXCEPT ALL`.
    pub except_all: bool,
    /// Accept a trailing `[AS] alias` on a *qualified* wildcard select item (`SELECT t.* x`,
    /// `SELECT t.* AS x`, `SELECT s.t.* x`), folding it onto the
    /// [`QualifiedWildcard`](crate::ast::SelectItem::QualifiedWildcard) item's `alias` slot
    /// with the source [`AliasSpelling`](crate::ast::AliasSpelling) (bare vs `AS`).
    ///
    /// PostgreSQL treats `t.*` as an ordinary column-reference expression (`columnref`, an
    /// `a_expr`), so it flows through the very same `target_el: a_expr [AS] label` projection
    /// alias an ordinary value takes: the bare form admits the `BareColLabel` reserved-word
    /// set (`SELECT t.* select` parses, `SELECT t.* from`/`order` reject) and the `AS` form the
    /// full `ColLabel` set (`SELECT t.* AS from` parses) — measured against libpg_query, which
    /// matches the parser's ordinary projection-alias boundary exactly, so the alias is parsed
    /// by reusing it. The bare `*` wildcard is the *separate* non-aliasable `target_el:
    /// '*'` production, so this gate never touches it (a bare-`*` alias is DuckDB's rename-all,
    /// which rides [`wildcard_modifiers`](Self::wildcard_modifiers)).
    ///
    /// Deliberately its own axis, *not* folded into [`wildcard_modifiers`](Self::wildcard_modifiers):
    /// the two behaviours have different measured boundaries — the `EXCLUDE`/`REPLACE`/`RENAME`
    /// modifier tail and the bare-`*` rename-all alias are DuckDB-only, whereas a qualified
    /// wildcard's plain alias is accepted by **PostgreSQL and DuckDB** (engine-probed: PG's
    /// libpg_query and DuckDB 1.5.4 accept `t.* x`/`t.* AS x`, while SQLite/MySQL and the SQL
    /// standard special-case `t.*` as a non-aliasable wildcard production and reject it —
    /// measured Reject on rusqlite/mysql:8 with the table provisioned). One behaviour = one flag.
    /// When off (ANSI/MySQL/SQLite and the ANSI-derived presets), the alias word is left
    /// unconsumed after `t.*` and surfaces as a clean parse error. On for PostgreSQL / DuckDB /
    /// Lenient, off elsewhere.
    pub qualified_wildcard_alias: bool,
    /// Accept a *parenthesized query* as a set-operation / statement / CTE-body / CTAS /
    /// `INSERT`-source operand — `(SELECT …) UNION (SELECT …)`, `((SELECT …)) LIMIT 1`,
    /// `CREATE TABLE t AS (SELECT …)` (PostgreSQL `select_with_parens`; the canonical
    /// set-op AST). On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite has no
    /// parenthesized compound operand — a `select-core` is `SELECT …`/`VALUES …`, never
    /// `( … )` — so it is off; a leading `(` in operand position is then a syntax error
    /// (engine-measured via rusqlite). The parenthesized query that SQLite *does* admit —
    /// a `FROM` table-or-subquery grouping (`FROM ((SELECT 1))`) and an expression-position
    /// scalar subquery (`SELECT ((SELECT 1))`) — is a complete standalone primary, not an
    /// operand, and stays accepted with the flag off (the parser threads a grouping
    /// context through those two positions); a paren-query there that is *extended* by a
    /// set operator or an `ORDER BY`/`LIMIT` tail (`FROM ((SELECT 1) UNION (SELECT 2))`,
    /// `FROM ((SELECT 1) LIMIT 1)`) is the syntax error SQLite reports.
    pub parenthesized_query_operands: bool,
    /// Reject a `VALUES` table-value constructor whose rows differ in width
    /// (`VALUES (1, 2), (3)`) at *parse* time. Unlike the additive gates above, this is a
    /// well-formedness *enforcement*: on rejects the ragged constructor, off accepts it.
    ///
    /// Equal row degree is a universal SQL rule, but engines check it at different phases,
    /// and this validator models per-dialect *parse* acceptance. DuckDB checks at parse —
    /// `Parser Error: VALUES lists must all be the same length`, in every VALUES position
    /// (standalone, derived table, `INSERT`; measured on 1.5.4) — so the DuckDb preset
    /// turns it on. PostgreSQL's raw grammar (libpg_query) and MySQL accept a ragged
    /// constructor and defer the arity check to parse-analysis / bind, past our parse-level
    /// parity contract, so it stays off there (their presets keep accepting it, exactly as
    /// `empty_target_list` accepts a bare `SELECT` under the PostgreSQL preset). `LENIENT`
    /// is a pure-acceptance superset, so it also leaves this off. Both counts the check
    /// compares — the two rows' arities — are present in the parse tree, so this is a
    /// shape-level gate, not a semantic one. On for DuckDB only; off elsewhere.
    pub values_rows_require_equal_arity: bool,
    /// Accept a bare-parenthesized row (`VALUES (1), (2)`) as a `VALUES` table-value
    /// constructor in *query* position — a top-level query body, a set-operation operand,
    /// a CTE body, or a derived table. On for ANSI/PostgreSQL/SQLite/DuckDB/Lenient. MySQL
    /// spells the query-position constructor `VALUES ROW(1), ROW(2)` (the `TABLE`/`VALUES`
    /// row-constructor grammar): a bare `(…)` row there is the syntax error MySQL reports
    /// (engine-measured on mysql:8 — `VALUES (1)` / `SELECT … FROM (VALUES (1)) …` /
    /// `VALUES (1) UNION …` all `ER_PARSE_ERROR`), so it is off there. Scoped to query
    /// position only: the `INSERT … VALUES (…)` source list is a distinct grammar that
    /// admits bare rows on every dialect (MySQL included), so it is parsed on a separate
    /// path this gate does not touch.
    pub values_row_constructor: bool,
    /// Reject a reserved word as an `AS`-introduced *projection* alias
    /// (`SELECT 1 AS <label>`), routing the position to the stricter
    /// [`reserved_bare_alias`](FeatureSet::reserved_bare_alias) set instead of the
    /// permissive [`reserved_as_label`](FeatureSet::reserved_as_label). Off for
    /// ANSI/PostgreSQL/SQLite/DuckDB/Lenient, whose projection `AS` alias is a
    /// PostgreSQL-style `ColLabel`: PostgreSQL admits every keyword there
    /// (`SELECT a AS select` parses) via its empty `reserved_as_label`, and SQLite draws
    /// no `ColId`/`ColLabel` split so its non-empty `reserved_as_label` already rejects
    /// them — neither needs this gate. MySQL has no `ColLabel` relaxation: an `AS` alias
    /// rejects exactly the words a *bare* alias does (`SELECT 1 AS range`/`AS left`/`AS
    /// delete` are `ER_PARSE_ERROR` on mysql:8, while the non-reserved `SELECT 1 AS any`
    /// parses), so it is on there. Scoped to the projection `AS` alias only: the
    /// dotted-name continuation (`t.range`, a qualified-name label MySQL admits) is a
    /// separate position that keeps the permissive `reserved_as_label` set.
    pub as_alias_rejects_reserved: bool,
    /// Accept a single trailing comma before a list's closing delimiter in the list
    /// positions DuckDB tolerates it: the `SELECT` projection list (`SELECT a, b, FROM
    /// t`), the query-position and `INSERT` `VALUES` row lists and each parenthesized
    /// row (`VALUES (1), (2),` / `VALUES (1, 2,)`), the `[…]` / `ARRAY[…]` list and
    /// `{…}` struct and `MAP {…}` collection literals, the `IN (…)` list, the `GROUP BY`
    /// key list and its `ROLLUP(…)` / `CUBE(…)` / `GROUPING SETS (…)` sub-lists
    /// (`GROUP BY a, b,` / `GROUP BY ROLLUP(a, b,)`), the wildcard-modifier lists
    /// (`* EXCLUDE (a,)`, `* REPLACE (e AS c,)`, `* RENAME (a AS b,)`), the
    /// `COALESCE` special-form argument list (`coalesce(1, 2,)`), and the
    /// `CREATE TABLE` table-element list (`CREATE TABLE t (a INT, b INT,)`), after a
    /// column or a constraint element alike. The comma is
    /// discarded — the list shape is unchanged, so no AST node carries it and the render
    /// drops it (canonical form, a lossy-spelling trade); it is not semantically
    /// meaningful.
    ///
    /// Scoped to those list sites only, because DuckDB is *not* uniform: engine-probed
    /// (1.5.4), an ordinary function-argument list (`greatest(1, 2,)`), a bare
    /// parenthesized / row constructor (`(1, 2,)`), an `ORDER BY` / `PARTITION BY` list,
    /// and an `INSERT` *column* list (`INSERT INTO t (a, b,) …`) all reject the trailing
    /// comma, so this gate is applied per accepting list site rather than in the shared
    /// `parse_comma_separated`. `COALESCE` is the lone function-call exception —
    /// `coalesce(1, 2,)` accepts because DuckDB parses it as a grammar special form, not
    /// a `func_application`, whereas every sibling (`greatest`/`least`/`nullif`/`concat`)
    /// keeps rejecting the comma. Only a *single* trailing comma is admitted (`[1, 2, ,]`
    /// and a leading `[,]` stay parse errors). When off (ANSI/PostgreSQL/MySQL/SQLite,
    /// none of which admit a trailing comma), the dangling comma is left for the item
    /// parser to reject — the same clean parse error those dialects report. On for DuckDB
    /// / Lenient, off elsewhere.
    pub trailing_comma: bool,
    /// Accept DuckDB's **projection** prefix colon alias — an alias written *before* its
    /// value as `<alias> : <value>` on a select-item head only (`SELECT j : 42` → alias `j`
    /// on value `42`). Pure sugar for the standard trailing `AS` alias: DuckDB records it
    /// in the ordinary alias field and canonically re-emits `AS` (json round-trip on 1.5.4:
    /// `SELECT j : 42` → `SELECT 42 AS j`). The FROM-position twin lives on
    /// [`TableExpressionSyntax::prefix_colon_alias`] so a dialect can enable one position
    /// without the other.
    ///
    /// Grammar-position gate (not lexical): lone `:` is always `Colon` punctuation. Collides
    /// at a value head with [`ExpressionSyntax::semi_structured_access`]
    /// ([`GrammarConflict::PrefixColonAliasVersusSemiStructuredAccess`]) when either this
    /// flag or the table-position twin is on. On for DuckDB / Lenient; off elsewhere.
    pub prefix_colon_alias: bool,
    /// Accept the Hive/Spark `LATERAL VIEW [OUTER] <generator>(args) <alias>
    /// [AS <col> [, …]]` table-generating clauses, written after the whole `FROM`
    /// clause and before `WHERE` and repeatable (Hive LanguageManual LateralView:
    /// `fromClause: FROM baseTable (lateralView)*`; Spark `SqlBaseParser.g4` places
    /// `lateralView*` at the same fromClause tail), modelled as
    /// [`Select::lateral_views`](crate::ast::Select) (see
    /// [`LateralView`](crate::ast::LateralView) for the typed shape, the sqlparser-rs
    /// parity reshapings, and the recorded acceptance bound — there is no Hive/Spark
    /// oracle, so the two engines' published grammars are the acceptance evidence).
    ///
    /// **Leading-keyword dispatch, position- and follow-token-partitioned against
    /// LATERAL derived tables.** `LATERAL` also introduces the standard derived-table /
    /// function factor ([`TableFactorSyntax::lateral`]). The two occupy disjoint
    /// grammar positions — a table-factor head (after `FROM`/`,`/a join keyword) versus
    /// after the *complete* FROM relation list — and split on the follow token (`VIEW`
    /// here; `(` or a function/subquery head there), so the dispatch is unambiguous
    /// under every preset combination and needs no [`GrammarConflict`] entry: this
    /// clause claims a `LATERAL` only when `VIEW` follows and only once the FROM list
    /// is complete, a cursor position the factor grammar never holds.
    ///
    /// **On for Hive, Databricks, and Lenient.** Hive and Databricks have no
    /// differential oracle, so this is a no-oracle acceptance addition carried by the
    /// two conservative presets and the permissive union: every oracle-compared dialect
    /// (ANSI/PostgreSQL/MySQL/SQLite/DuckDB) leaves it off — they parse-reject a
    /// post-FROM `LATERAL` as unconsumed input — so conformance sweeps see zero
    /// movement.
    pub lateral_view_clause: bool,
    /// Accept the Oracle-style `[START WITH <cond>] CONNECT BY [NOCYCLE] <cond>`
    /// hierarchical query clause, written **after `WHERE` and before `GROUP BY`** with
    /// `START WITH` and `CONNECT BY` in either order, modelled as
    /// [`Select::connect_by`](crate::ast::Select) (see
    /// [`HierarchicalClause`](crate::ast::HierarchicalClause) for the typed shape, the
    /// either-order fidelity tag, and the recorded acceptance bounds). The clause also
    /// enables the [`UnaryOperator::Prior`](crate::ast::UnaryOperator) operator, but only
    /// inside the `CONNECT BY` condition — the global expression grammar is unchanged, so
    /// a bare `prior` stays an ordinary column name.
    ///
    /// **Grammar evidence (no Oracle/Snowflake oracle).** Snowflake's public docs are the
    /// citable grammar (there is no Oracle preset): they document
    /// `START WITH … CONNECT BY [PRIOR] col = [PRIOR] col`. Oracle contributes the
    /// either-order rule, the after-`WHERE` position, and `NOCYCLE` (which Snowflake's
    /// docs explicitly omit) — modelling the Oracle superset is a documented
    /// conservative-direction over-acceptance, captured on the owning ticket.
    ///
    /// **On for Snowflake and Lenient.** Snowflake has no differential oracle, so this is
    /// a no-oracle acceptance addition carried by the Snowflake preset and the permissive
    /// union. Databricks/Spark does *not* get it: Databricks documents recursive CTEs
    /// (`WITH RECURSIVE`) instead of `CONNECT BY` and does not support the Oracle
    /// `CONNECT BY … START WITH` syntax (Databricks SQL reference), so it stays off there
    /// alongside every oracle-compared dialect (ANSI/PostgreSQL/MySQL/SQLite/DuckDB),
    /// which parse-reject a post-`WHERE` `CONNECT BY`/`START WITH` as unconsumed input —
    /// so conformance sweeps see zero movement.
    pub connect_by_clause: bool,
}

/// Dialect-owned query-tail syntax accepted by the parser.
///
/// The clauses parsed by the query-tail sequence that runs after the SELECT body. Split
/// out of [`SelectSyntax`] at its 16-field line as the query-tail axis, distinct from the
/// SELECT-core and grouping axes. Each flag is a grammar gate: when off the introducing
/// keyword is left unconsumed and the clause surfaces as a clean parse error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryTailSyntax {
    /// Accept the standard `OFFSET <n> { ROW | ROWS }` and `FETCH { FIRST | NEXT }
    /// <count> { ROW | ROWS } ONLY` row-limiting spelling.
    pub fetch_first: bool,
    /// Accept the MySQL/MariaDB/SQLite `LIMIT <offset>, <count>` two-argument
    /// comma form. It is a *spelling* of the same row limit as
    /// `LIMIT <count> OFFSET <offset>`, so it folds into the one canonical
    /// [`Limit`](crate::ast::Limit) shape tagged
    /// [`LimitSyntax::LimitOffset`](crate::ast::LimitSyntax) — never a
    /// new node. The comma binds the offset *first*, the count second (the reverse
    /// of `LIMIT <count> OFFSET <offset>`), which is exactly why it must be dialect
    /// data: reading the arguments in the wrong order silently swaps them.
    pub limit_offset_comma: bool,
    /// Accept a trailing row-locking clause (`FOR UPDATE`/`FOR SHARE [OF …]
    /// [NOWAIT|SKIP LOCKED]`, plus MySQL's legacy `LOCK IN SHARE MODE`), written after
    /// `LIMIT`. PostgreSQL and MySQL share this modern surface, so it folds into one
    /// canonical [`LockingClause`](crate::ast::LockingClause) on the query,
    /// gated here. When off (ANSI/SQLite/DuckDB, none of which admit a query-tail lock
    /// clause), the `FOR`/`LOCK` keyword is left unconsumed and surfaces as a clean
    /// parse error — the same reject mechanism the other query-tail gates use. On for
    /// PostgreSQL / MySQL / Lenient, off elsewhere. The gate covers the shared modern
    /// surface — a single `FOR UPDATE`/`FOR SHARE` clause with an `OF <table>, …` list
    /// and a wait tail. PostgreSQL's `NO KEY UPDATE`/`KEY SHARE` strengths and its
    /// stacked clauses ride the further [`key_lock_strengths`](QueryTailSyntax::key_lock_strengths)
    /// and [`stacked_locking_clauses`](QueryTailSyntax::stacked_locking_clauses) gates; the clause
    /// *before* `LIMIT` is still deferred.
    pub locking_clauses: bool,
    /// Accept PostgreSQL's `FOR NO KEY UPDATE` / `FOR KEY SHARE` row-locking strengths —
    /// the two levels between `FOR UPDATE` and `FOR SHARE`
    /// ([`LockStrength::NoKeyUpdate`](crate::ast::LockStrength) /
    /// [`LockStrength::KeyShare`](crate::ast::LockStrength)). Requires
    /// [`locking_clauses`](QueryTailSyntax::locking_clauses) (it refines the strength keyword after
    /// `FOR`; the dependency is [`FeatureDependencyViolation::KeyLockStrengthsWithoutLockingClauses`]);
    /// when off, the `NO`/`KEY` after `FOR` is never a strength lead, so
    /// `FOR NO KEY UPDATE` / `FOR KEY SHARE` surface as clean parse errors — the
    /// over-acceptance guard MySQL relies on (its grammar has only `UPDATE`/`SHARE`,
    /// engine-verified). PostgreSQL alone spells the `KEY`/`NO KEY` refinements
    /// (libpg_query `for_locking_strength`), so this is on for PostgreSQL / Lenient, off
    /// elsewhere. `FOR KEY UPDATE` and `FOR NO KEY SHARE` stay rejected under both
    /// settings — PostgreSQL pairs `NO KEY` only with `UPDATE` and `KEY` only with
    /// `SHARE` (probed on libpg_query, pg-locking-clause-strengths-and-stacking).
    pub key_lock_strengths: bool,
    /// Accept several *stacked* row-locking clauses on one query
    /// (`FOR UPDATE OF a FOR SHARE OF b`) — PostgreSQL applies a distinct lock per table
    /// group, so [`Query::locking`](crate::ast::Query) holds a list. Requires
    /// [`locking_clauses`](QueryTailSyntax::locking_clauses) (it repeats the shared clause; the
    /// dependency is [`FeatureDependencyViolation::StackedLockingClausesWithoutLockingClauses`]);
    /// when off, exactly one clause is parsed and any following `FOR`/`LOCK` is left
    /// unconsumed, surfacing as a trailing-input parse error — the over-acceptance guard
    /// MySQL relies on (its grammar admits exactly one locking clause, engine-verified by
    /// `mysql-select-tails-locking-hints-partition`). On for PostgreSQL / Lenient, off
    /// elsewhere.
    pub stacked_locking_clauses: bool,
    /// Accept DuckDB's `USING SAMPLE <entry>` query-level sample clause
    /// ([`Select::sample`](crate::ast::Select::sample)), written after `QUALIFY` and before
    /// the enclosing query's `ORDER BY` (`SELECT … USING SAMPLE 3 ORDER BY …`). The entry is
    /// DuckDB's `tablesample_entry`: a count-first `<size> [ROWS|PERCENT|%] [ '(' method
    /// [',' seed] ')' ]` or a method-first `method '(' <size> ')' [REPEATABLE '(' seed ')']`.
    /// On for DuckDB / Lenient, off elsewhere; when off the `USING` keyword in that position
    /// is left to fail as an unexpected statement token (matching the engines that lack the
    /// clause). Distinct from the table-factor `TABLESAMPLE`
    /// ([`TableExpressionSyntax::table_sample`](TableExpressionSyntax)).
    pub using_sample: bool,
    /// Accept a *leading* `OFFSET <count> [LIMIT <count>]` row-skip written without a
    /// preceding `LIMIT` (PostgreSQL's `[LIMIT …] [OFFSET …]` where either may come
    /// first). On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite spells the skip only as
    /// `LIMIT <count> OFFSET <count>` / `LIMIT <offset>, <count>` — a bare `OFFSET` with no
    /// `LIMIT` is a syntax error there — so it is off; the `OFFSET` keyword is then left
    /// unconsumed and surfaces as a clean parse error. The `OFFSET` that trails a `LIMIT`
    /// is unaffected (parsed by the `LIMIT` branch).
    pub leading_offset: bool,
    /// Accept an arbitrary *expression* as a `LIMIT`/`OFFSET` row count. On for
    /// ANSI/PostgreSQL/SQLite/DuckDB/Lenient, whose row limits admit a general expression
    /// (`LIMIT 1 + 1`, `LIMIT (SELECT n FROM cfg)`). MySQL restricts the count to an
    /// unsigned integer literal or a `?` placeholder (its `limit_clause` grammar), so it is
    /// off there: an operand that parses to anything else is the syntax error MySQL reports
    /// (engine-measured-rejected on mysql:8 — `LIMIT 1 + 1` / `LIMIT (SELECT 1)`). The whole
    /// operand is still parsed, then rejected on shape, so the diagnostic points at it. The
    /// plain `LIMIT <int>` / `LIMIT ?` / comma / `OFFSET <int>` forms are unaffected.
    pub limit_expressions: bool,
    /// Accept DuckDB's percentage `LIMIT` — a numeric-literal count directly followed by
    /// a `%` operator or the `PERCENT` keyword (`LIMIT 40 PERCENT`, `LIMIT 35%`), which
    /// returns that fraction of the result rows rather than a row number. On for
    /// DuckDB/Lenient only; off for ANSI/PostgreSQL/MySQL/SQLite, whose `LIMIT` has no
    /// percentage form. With the flag off the marker is left unconsumed — a `%` folds as
    /// modulo (needing a right operand) and a trailing `PERCENT` is leftover input — so
    /// both surface as the parse error those dialects report. DuckDB folds the marker
    /// only onto a bare numeric literal at a clause boundary: `LIMIT 10 % 3` stays
    /// ordinary modulo, and `LIMIT a PERCENT` / `LIMIT (1 + 1) PERCENT` are DuckDB syntax
    /// errors (verified on 1.5.4), so the gate does not over-accept them.
    pub limit_percent: bool,
    /// Enforce PostgreSQL's `gram.y` validity checks on `FETCH FIRST … WITH TIES` — both
    /// semantic guards `insertSelectOptions` raises during raw parsing (so `pg_query`, a
    /// parse-only oracle, rejects them):
    ///
    /// 1. **`ORDER BY` required** — `WITH TIES` needs a governing `ORDER BY` at the same
    ///    query level (`WITH TIES cannot be specified without ORDER BY clause`).
    /// 2. **`SKIP LOCKED` rejected** — `WITH TIES` cannot combine with a `SKIP LOCKED`
    ///    locking clause (`SKIP LOCKED and WITH TIES options cannot be used together`).
    ///
    /// On for PostgreSQL only; other dialects with `fetch_first` keep accepting both forms
    /// unchanged. When off, neither guard fires. The two rules always co-vary in shipped
    /// presets (only Postgres enables this flag), so they share one knob rather than a
    /// second independently toggled field — see the module-level naming-exception note.
    pub with_ties_requires_order_by: bool,
    /// Accept BigQuery/ZetaSQL **query pipe syntax**: a trailing chain of `|>` operators
    /// that post-process a query result one step at a time
    /// (`FROM t |> WHERE x |> SELECT a`), modelled as
    /// [`Query::pipe_operators`](crate::ast::Query::pipe_operators). This gate does double
    /// duty — it is read by the *tokenizer* to munch `|>` (pipe-arrow) as a single token
    /// (off, the two bytes stay `|` then `>`, so no dialect's lexing shifts) and by the
    /// *parser* to admit the trailing `|>`-operator chain. When off (every shipped preset),
    /// a `|>` after a query is never a pipe separator and surfaces as a clean parse error,
    /// exactly as today.
    ///
    /// **Off for every shipped preset.** This surface belongs to no engine we oracle
    /// against — the BigQuery preset that would home it deliberately defers it (a
    /// considered judgment; see that preset's module docs) — so with no differential
    /// oracle to verify it, every shipped dialect (including DuckDB, which inherits the
    /// PostgreSQL value) leaves it off, and conformance sweeps see zero movement. `LENIENT`
    /// leaves it off *for now* as well: although the lenient charter admits any
    /// conflict-free pure-acceptance form (and `|>` *is* conflict-free — its munch is
    /// feature-gated, shadowing nothing), the framework ships only the reference `WHERE`
    /// operator, so enabling it in `LENIENT` today would make the "parse anything" preset
    /// accept `|> WHERE` while rejecting every other pipe operator — a fragment a reader of
    /// the lenient module could not predict, violating that module's honesty bar. Once the
    /// pipe-operator surface is coherent (the `planner-parity-pipe-*` tickets land),
    /// `LENIENT` should flip this on as a pure-acceptance addition.
    pub pipe_syntax: bool,
    /// Accept ClickHouse `LIMIT n [OFFSET m] BY expr, …` — per-group row limiting,
    /// written after `ORDER BY` and before the ordinary `LIMIT` tail (both may appear
    /// in one query), modelled as [`Query::limit_by`](crate::ast::Query::limit_by). The
    /// parser reads a leading `LIMIT` speculatively and treats it as `LIMIT BY` only
    /// when a `BY` follows the count and optional `OFFSET`; otherwise it rewinds and the
    /// token is the ordinary `LIMIT`, so a plain `LIMIT n` / `LIMIT n OFFSET m` parses
    /// identically whether this gate is on or off.
    ///
    /// **On for the ClickHouse preset and Lenient.** ClickHouse has no differential oracle,
    /// so this is a no-oracle acceptance addition carried by the ClickHouse preset and the
    /// permissive union (the `apply_join` precedent): every oracle-compared dialect
    /// (ANSI/PostgreSQL/MySQL/SQLite/DuckDB) leaves it off — they parse-reject a `BY`
    /// after `LIMIT` — so conformance sweeps see zero movement.
    pub limit_by_clause: bool,
    /// Accept ClickHouse `SETTINGS name = value, …` — query-level setting overrides
    /// written after the ordinary `LIMIT` tail, modelled as
    /// [`Query::settings`](crate::ast::Query::settings). `SETTINGS` is matched as a
    /// contextual keyword (it stays an ordinary identifier elsewhere); each pair is an
    /// identifier `=` value, the value a general expression.
    ///
    /// **On for the ClickHouse preset and Lenient.** ClickHouse has no differential oracle,
    /// so this is a no-oracle acceptance addition carried by the ClickHouse preset and the
    /// permissive union (the `limit_by_clause` precedent): every oracle-compared dialect
    /// (ANSI/PostgreSQL/MySQL/SQLite/DuckDB) leaves it off — they parse-reject a trailing
    /// `SETTINGS …` as unconsumed input — so conformance sweeps see zero movement.
    pub settings_clause: bool,
    /// Accept ClickHouse `FORMAT <name>` — the output-format clause that closes the
    /// query, modelled as [`Query::format`](crate::ast::Query::format). `FORMAT` is
    /// matched as a contextual keyword (it stays an ordinary identifier elsewhere); the
    /// format name is a bare, case-sensitive identifier (`JSON`, `TabSeparated`, `Null`),
    /// not a string literal.
    ///
    /// **On for the ClickHouse preset and Lenient.** ClickHouse has no differential oracle,
    /// so this is a no-oracle acceptance addition carried by the ClickHouse preset and the
    /// permissive union (the `settings_clause` precedent): every oracle-compared dialect
    /// (ANSI/PostgreSQL/MySQL/SQLite/DuckDB) leaves it off — they parse-reject a trailing
    /// `FORMAT …` as unconsumed input — so conformance sweeps see zero movement.
    pub format_clause: bool,
    /// Accept MSSQL's `FOR XML {RAW|AUTO|EXPLICIT|PATH} [, …]` and
    /// `FOR JSON {AUTO|PATH} [, …]` result-shaping tails, which serialize the result
    /// set as XML/JSON rather than a rowset, modelled as
    /// [`Query::for_clause`](crate::ast::Query::for_clause) (see [`ForClause`](crate::ast::ForClause)).
    /// The modes and their directives (`ELEMENTS [XSINIL|ABSENT]`, `BINARY BASE64`,
    /// `TYPE`, `ROOT ['name']`, `INCLUDE_NULL_VALUES`, `WITHOUT_ARRAY_WRAPPER`) are typed
    /// data; the accepted grammar follows the MSSQL `FOR XML` / `FOR JSON`
    /// documentation (no MSSQL oracle, so that is the recorded acceptance bound).
    ///
    /// **Leading-keyword dispatch, follow-token partitioned against locking.** `FOR`
    /// also introduces the row-locking clauses ([`locking_clauses`](QueryTailSyntax::locking_clauses)).
    /// The two share the `FOR` lead but split on the *follow* token — `XML`/`JSON` here
    /// versus `UPDATE`/`SHARE`/`NO`/`KEY` for locking — so the dispatch is unambiguous
    /// under every preset combination and needs no
    /// [`GrammarConflict`] entry: the locking parser
    /// declines a `FOR` whose follow token is `XML`/`JSON` (so a stacked
    /// `FOR UPDATE … FOR XML` still reaches this clause), and this clause declines a
    /// `FOR` whose follow token is anything else.
    ///
    /// **On for MSSQL and Lenient.** MSSQL has no differential oracle, so this is a
    /// no-oracle acceptance addition carried by the MSSQL preset and the permissive
    /// union: every oracle-compared dialect (ANSI/PostgreSQL/MySQL/SQLite/DuckDB) leaves
    /// it off — they parse-reject a trailing `FOR XML`/`FOR JSON` as unconsumed input
    /// (or, where `locking_clauses` is on, reject `XML`/`JSON` after `FOR`) — so
    /// conformance sweeps see zero movement.
    pub for_xml_json_clause: bool,
}

/// Dialect-owned `GROUP BY` / `ORDER BY` grouping-and-ordering syntax accepted by the
/// parser.
///
/// The grouping-set constructs and the clause-level grouping/ordering modes and
/// quantifiers. Split out of [`SelectSyntax`] at its 16-field line as the
/// grouping/ordering axis, distinct from the SELECT-core and query-tail axes. Each
/// flag is a grammar gate: when off the keyword falls through to the ordinary
/// expression/grouping-item grammar or surfaces as a clean parse error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupingSyntax {
    /// Accept the SQL:1999 (feature T431) grouping-set constructs as `GROUP BY`
    /// items: `ROLLUP (…)`, `CUBE (…)`, `GROUPING SETS (…)`, and the empty grouping
    /// set `()`. PostgreSQL lowers these in GROUP BY item position for *any* case
    /// spelling — an unquoted `rollup (a, b)` is the grouping construct, never a
    /// call to a user function `rollup` (quote it, `"rollup"(a, b)`, to call the
    /// function) — so the parser models them as [`GroupByItem`](crate::ast::GroupByItem)
    /// nodes, not [`FunctionCall`](crate::ast::FunctionCall) expressions. When off,
    /// the keywords fall through to the expression grammar (an ordinary function
    /// call), which is how MySQL reads them; MySQL's own grouping surface is the
    /// distinct trailing `WITH ROLLUP`, not modelled here. On for ANSI (the T431
    /// standard) / PostgreSQL / Lenient, off for MySQL.
    pub grouping_sets: bool,
    /// Accept MySQL's trailing `GROUP BY <keys> WITH ROLLUP` modifier — MySQL's only
    /// grouping-set surface (it has no SQL:1999 `ROLLUP (…)` item form). It is a
    /// *spelling* of the same super-aggregate as `ROLLUP (…)`, so it canonicalizes
    /// into the one [`GroupByItem::Rollup`](crate::ast::GroupByItem) shape tagged
    /// [`RollupSpelling::WithRollup`](crate::ast::RollupSpelling) — never a
    /// new node. When off, the trailing `WITH ROLLUP` is left unconsumed and surfaces
    /// as a clean parse error; PostgreSQL/ANSI spell the construct `ROLLUP (…)`, so
    /// accepting `WITH ROLLUP` there would be an over-acceptance. On for MySQL /
    /// Lenient, off elsewhere. (MySQL 8.0.1+ also permits `WITH ROLLUP` alongside
    /// `ORDER BY`; older versions did not — a version wrinkle we do not model.)
    pub with_rollup: bool,
    /// Accept PostgreSQL's `ORDER BY <expr> USING <operator>` sort form (`gram.y`
    /// `sortby: a_expr USING qual_all_Op opt_nulls_order`), which sorts by a named
    /// ordering operator (`USING <`, `USING OPERATOR(schema.op)`) instead of
    /// `ASC`/`DESC`. PostgreSQL-only; ANSI and MySQL have only `ASC`/`DESC`, so there
    /// the `USING` keyword is left unconsumed and surfaces as a trailing-input parse
    /// error — the same reject mechanism the other unsupported clauses use.
    pub order_by_using: bool,
    /// Accept DuckDB's `GROUP BY ALL` clause mode: group by every non-aggregated
    /// projection column, resolved at bind time
    /// ([`Select::group_by_all`](crate::ast::Select) — a mode with an empty key
    /// list, never a [`GroupByItem`](crate::ast::GroupByItem)). `ALL` cannot mix
    /// with explicit keys or grouping sets (DuckDB syntax-errors on `GROUP BY ALL,
    /// x` and `GROUP BY ROLLUP(x), ALL`; probed on 1.5.4), so the branch consumes
    /// exactly the one keyword. When off, `ALL` after `GROUP BY` falls through to
    /// the expression grammar, where every shipped dialect reserves it — a clean
    /// parse error. A *separate* flag from [`order_by_all`](GroupingSyntax::order_by_all),
    /// not one paired gate: the paired-flag doctrine (the `straight_join` /
    /// `table_options` precedent) covers grammar points a dialect only ever ships
    /// together, but these are two independent constructs on two clauses that real
    /// engines adopt separately (Snowflake ships `GROUP BY ALL` with no
    /// `ORDER BY ALL`), so pairing them would bake a DuckDB coincidence into the
    /// vocabulary. On for DuckDB / Lenient, off elsewhere.
    pub group_by_all: bool,
    /// Accept PostgreSQL's `GROUP BY {DISTINCT | ALL} <grouping items>` set-quantifier
    /// (SQL:2016 feature T434): a `DISTINCT`/`ALL` prefix on the whole grouping clause
    /// that governs deduplication of the generated grouping sets
    /// ([`Select::group_by_quantifier`](crate::ast::Select)). The quantifier requires a
    /// non-empty grouping list — PostgreSQL rejects a bare `GROUP BY ALL` /
    /// `GROUP BY DISTINCT` (probed on pg_query PG-17) — which is what keeps it MECE with
    /// [`group_by_all`](GroupingSyntax::group_by_all), DuckDB's mode where `ALL` *is* the entire
    /// clause. A *separate* flag from `group_by_all` for that reason: the two constructs
    /// spell an overlapping keyword (`ALL`) but mean opposite things (a modifier on a
    /// list vs. a standalone mode), and no shipped dialect but Lenient enables both.
    /// Under Lenient (both on) the forms stay disambiguated by lookahead — a bare
    /// `GROUP BY ALL` is the DuckDB mode, `GROUP BY ALL <items>` is the PostgreSQL
    /// quantifier — so the widening is conflict-free. When off, the `DISTINCT`/`ALL`
    /// keyword after `GROUP BY` falls through to the grouping-item grammar, where every
    /// shipped dialect reserves it — a clean parse error. On for PostgreSQL / Lenient,
    /// off elsewhere.
    pub group_by_set_quantifier: bool,
    /// Accept DuckDB's `ORDER BY ALL [ASC | DESC] [NULLS FIRST | LAST]` clause
    /// mode: sort by every projection column, left to right
    /// ([`Query::order_by_all`](crate::ast::Query) — a mode of the whole clause,
    /// never a sort-key expression). `ALL` cannot mix with explicit keys
    /// (`ORDER BY ALL, x` / `ORDER BY x, ALL` syntax-error; probed on 1.5.4) and
    /// takes no `USING` tail. The gate covers only the *query-level* clause, the
    /// one position DuckDB gives mode semantics: window `ORDER BY ALL` is a
    /// dedicated DuckDB parse error ("Cannot ORDER BY ALL in a window
    /// expression"), and the aggregate-internal `agg(x ORDER BY ALL)` form —
    /// which DuckDB reads as a `COLUMNS(*)` star expansion producing one output
    /// per column — is a different grammar position: a sort *key*
    /// ([`Expr::Columns`](crate::ast::Expr::Columns)), gated with the node's own
    /// [`CallSyntax::columns_expression`](CallSyntax::columns_expression).
    /// When off, `ALL` after `ORDER BY` falls through to
    /// the expression grammar, where every shipped dialect reserves it — a clean
    /// parse error. On for DuckDB / Lenient, off elsewhere; see
    /// [`group_by_all`](GroupingSyntax::group_by_all) for why the two are separate flags.
    pub order_by_all: bool,
}

/// Dialect-owned transaction-control (TCL) syntax accepted by the parser.
///
/// Transaction openers, completers, savepoints, mode lists, and XA distributed-transaction
/// forms — the SQL transaction-control family. Split out of [`UtilitySyntax`] so utility
/// statement-head gates (COPY/PRAGMA/SHOW siblings) are not mixed with TCL grammar and
/// narrowing validators. Statement-head flags that open TCL statements
/// (`start_transaction`, `set_transaction`, `xa_transactions`, …) are still consumed by
/// `parser::statement_dispatch` / `parser::tcl`; mode-list validators are read by the TCL
/// body parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransactionSyntax {
    /// Accept the standard `START TRANSACTION` transaction opener. Some engines,
    /// notably SQLite, expose only `BEGIN` and reject the `START` spelling.
    pub start_transaction: bool,
    /// Accept `START [TRANSACTION | WORK]` with the transaction block word omitted or
    /// spelled `WORK`. When off, the standard `START TRANSACTION` spelling remains
    /// mandatory. DuckDB accepts all three spellings; PostgreSQL requires
    /// `TRANSACTION` after `START`.
    pub start_transaction_block_optional: bool,
    /// Accept `WORK` as the optional transaction block word on `BEGIN`, `COMMIT`, and
    /// `ROLLBACK`. `START WORK` is controlled by
    /// [`start_transaction_block_optional`](Self::start_transaction_block_optional).
    pub transaction_work_keyword: bool,
    /// Accept `TRANSACTION` as the optional block word after `BEGIN`.
    pub begin_transaction_keyword: bool,
    /// Accept `TRANSACTION` as the optional block word after `COMMIT`.
    pub commit_transaction_keyword: bool,
    /// Accept `TRANSACTION` as the optional block word after `ROLLBACK`.
    pub rollback_transaction_keyword: bool,
    /// Accept SQLite's optional transaction name immediately after an explicit
    /// `TRANSACTION` block word (`BEGIN TRANSACTION name`, `COMMIT TRANSACTION name`,
    /// or `ROLLBACK TRANSACTION name`).
    pub transaction_name: bool,
    /// Accept a standard transaction-mode list after `BEGIN [WORK | TRANSACTION]`.
    pub begin_transaction_modes: bool,
    /// Accept transaction savepoint statements: `SAVEPOINT`, `RELEASE`, and
    /// `ROLLBACK TO`. This controls the complete savepoint statement family.
    pub transaction_savepoints: bool,
    /// Accept `SET TRANSACTION <mode> [, ...]`.
    pub set_transaction: bool,
    /// Accept `ISOLATION LEVEL <level>` in a transaction mode list.
    pub transaction_isolation_mode: bool,
    /// Accept `READ ONLY` / `READ WRITE` in a transaction mode list.
    pub transaction_access_mode: bool,
    /// Accept `DEFERRABLE` / `NOT DEFERRABLE` in a transaction mode list.
    pub transaction_deferrable_mode: bool,
    /// Accept `ISOLATION LEVEL <level>` after `START TRANSACTION`. When off, the
    /// isolation form can remain available to `SET TRANSACTION`.
    pub start_transaction_isolation_mode: bool,
    /// Accept `[NOT] DEFERRABLE` after `START TRANSACTION`. When off, the form can
    /// remain available to `SET TRANSACTION`.
    pub start_transaction_deferrable_mode: bool,
    /// Accept MySQL's `WITH CONSISTENT SNAPSHOT` `START TRANSACTION` characteristic.
    pub start_transaction_consistent_snapshot: bool,
    /// Accept more than one transaction mode, separated by an optional comma.
    pub transaction_multiple_modes: bool,
    /// ON rejects a missing comma between adjacent transaction modes (MySQL).
    /// PostgreSQL permits bare whitespace between modes when this is off.
    pub transaction_modes_require_commas: bool,
    /// ON rejects a repeated transaction-mode kind in one list (MySQL: each
    /// characteristic category at most once).
    pub transaction_modes_reject_duplicates: bool,
    /// Accept `ABORT` as an exact `ROLLBACK` synonym.
    pub abort_transaction_alias: bool,
    /// Accept `END` as an exact `COMMIT` synonym.
    pub end_transaction_alias: bool,
    /// Accept MySQL's `COMMIT|ROLLBACK [NO] RELEASE` completion modifier.
    pub transaction_release: bool,
    /// Accept `COMMIT AND [NO] CHAIN` and whole-transaction `ROLLBACK AND [NO] CHAIN`.
    /// PostgreSQL and SQL-standard transaction chaining; off where the engine grammar lacks it.
    pub transaction_chain: bool,
    /// Accept `RELEASE <name>` without the `SAVEPOINT` keyword. When off,
    /// `RELEASE SAVEPOINT <name>` remains available with transaction savepoints.
    pub release_savepoint_keyword_optional: bool,
    /// Accept SQLite's `{DEFERRED | IMMEDIATE | EXCLUSIVE}` transaction-mode modifier
    /// between `BEGIN` and the optional `TRANSACTION` keyword (stored on
    /// [`TransactionStatement::Begin`](crate::ast::TransactionStatement::Begin)'s `mode`
    /// field). On for SQLite/Lenient; off elsewhere. PostgreSQL's `BEGIN` takes its own,
    /// differently-shaped modifier set (`ISOLATION LEVEL …` / `READ ONLY|WRITE` / `[NOT]
    /// DEFERRABLE`, the existing [`TransactionMode`](crate::ast::TransactionMode) list),
    /// deliberately not modelled here, so it stays off there and the leading modifier
    /// keyword falls through to today's error (engine-probed: `pg_query` rejects `BEGIN
    /// DEFERRED`/`BEGIN IMMEDIATE`/`BEGIN EXCLUSIVE`).
    pub begin_transaction_mode: bool,
    /// Accept the MySQL `XA` distributed-transaction family — `XA {START | BEGIN} xid [JOIN |
    /// RESUME]`, `XA END xid [SUSPEND [FOR MIGRATE]]`, `XA PREPARE xid`, `XA COMMIT xid [ONE
    /// PHASE]`, `XA ROLLBACK xid`, and `XA RECOVER [CONVERT XID]` (the X/Open two-phase-commit
    /// verbs; `sql_yacc.yy` `xa:`). One flag gates the whole family because it is a single
    /// dialect unit reached through one unique leading `XA` keyword (the
    /// [`kill`](UtilitySyntax::kill) leading-keyword-gate precedent, not a keyword shared with
    /// another dialect). On for MySQL and the Lenient superset (a pure addition there — no other
    /// dialect claims `XA`); off elsewhere, where the leading `XA` keyword is not dispatched and
    /// surfaces as an unknown statement. Live mysql:8.4.10: every grammar-valid form answers
    /// `ER_UNSUPPORTED_PS` 1295 (recognized, not preparable over the wire). See
    /// [`XaStatement`](crate::ast::XaStatement).
    pub xa_transactions: bool,
}

/// Dialect-owned utility-statement syntax extensions accepted by the parser.
///
/// Non-TCL utility statements whose leading keyword a dialect dispatches — COPY, PRAGMA,
/// ATTACH, prepared statements, locks, replication, … — the statement-keyword analogue of
/// [`MutationSyntax::merge`]. Transaction-control grammar lives in [`TransactionSyntax`].
/// Whether each statement is dispatched is explicit dialect data: when a flag is off the
/// keyword is left undispatched and surfaces as an unknown statement.
/// **Statement-head flags on this axis are consumed by the parser's statement-head router**
/// (`parser::statement_dispatch`); parse bodies live in family modules (`util`, …).
/// `EXPLAIN` is not gated here (accepted dialect-agnostically for now). Introspection,
/// physical-maintenance, and access-control statements live in [`ShowSyntax`],
/// [`MaintenanceSyntax`], and [`AccessControlSyntax`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UtilitySyntax {
    /// Accept the PostgreSQL `COPY <table>|(<query>) {FROM|TO} <endpoint> ...` bulk
    /// data-transfer statement. PostgreSQL-only (also its generic/permissive supersets);
    /// off in ANSI and MySQL, which have no `COPY`, so there the leading `COPY` keyword
    /// is not dispatched and surfaces as an unknown statement.
    pub copy: bool,
    /// Accept the Snowflake `COPY INTO <target> FROM <source> [<opt> = <val> ...]` bulk
    /// load/unload statement — a distinct grammar from the PostgreSQL `COPY` gated by
    /// [`copy`](Self::copy): fixed `INTO … FROM …` direction, `KEY = VALUE` options with a
    /// nested `FILE_FORMAT = (…)` list, and the `FILES`/`PATTERN`/`VALIDATION_MODE` clauses.
    /// The two share only the leading `COPY` keyword; the dispatcher branches on the `INTO`
    /// that follows. On for Snowflake and Lenient; off elsewhere (including PostgreSQL and
    /// DuckDB, whose `COPY` is the `copy`-gated `{FROM | TO}` transfer), where a `COPY INTO`
    /// surfaces as an unknown statement.
    pub copy_into: bool,
    /// Accept Snowflake stage references `@stage` / `@~` / `@%table` (with optional
    /// `/path` segments) as a dedicated token and as a `COPY INTO` endpoint.
    /// On for Snowflake and Lenient; off elsewhere so `@` keeps its other claimants.
    pub stage_references: bool,
    /// Accept the PostgreSQL `COMMENT ON <object> IS '<text>' | NULL` object-metadata
    /// statement. PostgreSQL-only (and its permissive superset); off in ANSI and MySQL,
    /// which have no `COMMENT ON`, so there the leading `COMMENT` keyword is not
    /// dispatched and surfaces as an unknown statement.
    pub comment_on: bool,
    /// Accept the front-position `COMMENT IF EXISTS ON ...` guard.
    pub comment_if_exists: bool,
    /// Accept the SQLite `PRAGMA [<schema> .] <name> [= <value> | (<value>)]`
    /// configuration statement. SQLite-only (and the permissive superset); off in
    /// ANSI, PostgreSQL, and MySQL, which have no `PRAGMA`, so there the leading
    /// `PRAGMA` keyword is not dispatched and surfaces as an unknown statement.
    pub pragma: bool,
    /// Accept the SQLite `ATTACH [DATABASE] <expr> AS <schema>` statement *and* its
    /// `DETACH [DATABASE] <schema>` inverse. One flag gates both leading keywords
    /// because they are a single dialect unit — a dialect with `ATTACH` has `DETACH`
    /// (mirroring how `existence_guards.if_exists` gates its paired grammar
    /// points together). SQLite-only (and the permissive superset); off in ANSI,
    /// PostgreSQL, and MySQL, so there the leading keywords are not dispatched and
    /// surface as unknown statements.
    pub attach: bool,
    /// Accept the MySQL `KILL [CONNECTION | QUERY] <id>` thread/query-termination
    /// statement. MySQL-only (and the permissive superset); off in ANSI, PostgreSQL, and
    /// SQLite, which have no `KILL`, so there the leading keyword is not dispatched and
    /// surfaces as an unknown statement — the `copy`/`comment_on` leading-keyword-gate
    /// precedent.
    pub kill: bool,
    /// Accept the MySQL `HANDLER` low-level cursor family — `HANDLER <t> OPEN [[AS] alias]`,
    /// `HANDLER <t> READ …`, and `HANDLER <t> CLOSE` — direct index-level storage-engine
    /// access that bypasses the optimizer. One flag gates the leading `HANDLER` keyword (all
    /// three verbs follow the opened table) — the `copy`/`kill` leading-keyword-gate
    /// precedent. MySQL-only among the shipped presets, and a pure addition in the permissive
    /// superset (the leading `HANDLER` collides with no other statement); off in ANSI,
    /// PostgreSQL, and SQLite, which have no `HANDLER`, so there it is not dispatched and
    /// surfaces as an unknown statement. See
    /// [`HandlerStatement`](crate::ast::HandlerStatement).
    pub handler_statements: bool,
    /// Accept the MySQL plugin/component install-management family — `INSTALL PLUGIN <name>
    /// SONAME <lib>`, `INSTALL COMPONENT <urn> … [SET …]`, `UNINSTALL PLUGIN <name>`, and
    /// `UNINSTALL COMPONENT <urn> …`. One flag gates the leading `INSTALL` and `UNINSTALL`
    /// keywords together, an install/uninstall pair kept as one dialect unit like
    /// `attach`/`detach`. MySQL-only among the shipped presets, and a pure addition in the
    /// permissive superset (the leading `INSTALL`/`UNINSTALL` collide with no other statement);
    /// off in ANSI, PostgreSQL, and SQLite, which have neither, so there they are not dispatched
    /// and surface as an unknown statement. See
    /// [`InstallStatement`](crate::ast::InstallStatement) and
    /// [`UninstallStatement`](crate::ast::UninstallStatement).
    pub plugin_component_statements: bool,
    /// Accept the MySQL `SHUTDOWN` server-shutdown statement — a nullary leading keyword. A
    /// leading-keyword gate like `kill`; MySQL-only among the shipped presets and a pure
    /// addition in the permissive superset (the leading `SHUTDOWN` collides with no other
    /// statement); off in ANSI, PostgreSQL, and SQLite, where it is not dispatched and surfaces
    /// as an unknown statement. Separate from [`restart`](UtilitySyntax::restart): `SHUTDOWN`
    /// and `RESTART` are distinct statements, not an inverse pair, so each takes its own flag
    /// (the `vacuum`/`reindex` precedent). See [`Statement::Shutdown`](crate::ast::Statement).
    pub shutdown: bool,
    /// Accept the MySQL `RESTART` server-restart statement — a nullary leading keyword. A
    /// leading-keyword gate like `kill`; MySQL-only among the shipped presets and a pure
    /// addition in the permissive superset; off in ANSI, PostgreSQL, and SQLite, where it
    /// surfaces as an unknown statement. Separate from [`shutdown`](UtilitySyntax::shutdown) —
    /// distinct behaviours, distinct flags. See [`Statement::Restart`](crate::ast::Statement).
    pub restart: bool,
    /// Accept the MySQL `CLONE` data-directory provisioning statement — `CLONE LOCAL DATA
    /// DIRECTORY [=] '<dir>'` and `CLONE INSTANCE FROM <user>[@<host>]:<port> IDENTIFIED BY
    /// '<pw>' [DATA DIRECTORY [=] '<dir>'] [REQUIRE [NO] SSL]`. One flag gates the leading
    /// `CLONE` keyword (both `LOCAL`/`INSTANCE` forms follow it — one statement, two forms, like
    /// `flush`); MySQL-only and a pure addition in the permissive superset; off in ANSI,
    /// PostgreSQL, and SQLite. See [`CloneStatement`](crate::ast::CloneStatement).
    pub clone: bool,
    /// Accept the MySQL `IMPORT TABLE FROM '<file>' [, …]` tablespace-import statement. A
    /// leading-keyword gate on `IMPORT` distinguished from DuckDB's `IMPORT DATABASE`
    /// ([`export_import_database`](UtilitySyntax::export_import_database)) by the second keyword
    /// (`TABLE` vs `DATABASE`): both may be on in the permissive superset without colliding.
    /// MySQL-only among the shipped presets; off in ANSI, PostgreSQL, SQLite, and DuckDB. See
    /// [`ImportTableStatement`](crate::ast::ImportTableStatement).
    pub import_table: bool,
    /// Accept the MySQL `HELP '<topic>'` help-lookup statement — a leading-keyword gate like
    /// `kill`. MySQL-only among the shipped presets and a pure addition in the permissive
    /// superset (the leading `HELP` collides with no other statement); off in ANSI, PostgreSQL,
    /// and SQLite. See [`HelpStatement`](crate::ast::HelpStatement).
    pub help_statement: bool,
    /// Accept the MySQL `BINLOG '<base64-event>'` binary-log-event replay statement — a
    /// leading-keyword gate like `kill`. MySQL-only among the shipped presets and a pure
    /// addition in the permissive superset; off in ANSI, PostgreSQL, and SQLite. See
    /// [`BinlogStatement`](crate::ast::BinlogStatement).
    pub binlog: bool,
    /// Accept the MySQL MyISAM key-cache statement pair — `CACHE INDEX <t> [<keys>][, ...]
    /// [PARTITION (...)] IN <cache>` and `LOAD INDEX INTO CACHE <t> [PARTITION (...)] [<keys>]
    /// [IGNORE LEAVES][, ...]`. One flag gates both leading keywords (`CACHE` and the
    /// `LOAD INDEX` lookahead) because they are a single dialect unit — key-cache assignment
    /// and preload travel together (the `attach`/`detach`, `prepared_statements` single-flag
    /// precedent). MySQL-only among the shipped presets, and a pure addition in the permissive
    /// superset; off in ANSI, PostgreSQL, and SQLite, where the leading keywords are not
    /// dispatched (a leading `LOAD` without `INDEX` still reaches the `load_extension` gate).
    /// See [`CacheIndexStatement`](crate::ast::CacheIndexStatement) and
    /// [`LoadIndexStatement`](crate::ast::LoadIndexStatement).
    pub key_cache_statements: bool,
    /// Accept the `USE` catalog/schema-switch statement — DuckDB `USE <catalog> [. <schema>]`
    /// and MySQL `USE <schema>`. A leading-keyword gate like `pragma`: on for DuckDB, MySQL,
    /// and the permissive superset; off in ANSI, PostgreSQL, and SQLite, which have no `USE`
    /// statement, so there the leading `USE` keyword is not dispatched and surfaces as an
    /// unknown statement. (A *non-leading* `USE` is the MySQL index-hint keyword, consumed by
    /// the `FROM` grammar, so it never reaches this statement-leading position.) The accepted
    /// name arity rides [`use_qualified_name`](UtilitySyntax::use_qualified_name).
    pub use_statement: bool,
    /// Accept a dotted `USE <catalog> . <schema>` name (DuckDB) rather than the single
    /// unqualified `USE <schema>` (MySQL). Refines the name grammar of the base `USE`
    /// statement, so it requires [`use_statement`](UtilitySyntax::use_statement): without it
    /// the leading `USE` is not dispatched and this flag is inert, the dependency the
    /// [`UseQualifiedNameWithoutUseStatement`](crate::dialect::FeatureDependencyViolation::UseQualifiedNameWithoutUseStatement)
    /// registry variant records. On for DuckDB and the permissive superset; off for MySQL,
    /// whose `USE ident` grammar `ER_PARSE_ERROR`s any dotted name (engine-measured on
    /// mysql:8), and off (vacuously) wherever `use_statement` is off. DuckDB still rejects a
    /// three-part `USE a.b.c` even with this on — that arity bound is enforced in the parser.
    pub use_qualified_name: bool,
    /// Accept a string-constant (`Sconst`) spelling of the `USE` target name — DuckDB's
    /// `USE 'n'` / `USE E'n'` / `USE $$n$$` single-part form (engine-measured on
    /// libduckdb 1.5.4 via `duckdb_extract_statements`). The string is a *single-part*
    /// name only: a dotted string name (`USE 'a'.'b'`, `USE a.'b'`) is a parser reject,
    /// matching DuckDB. Refines the name grammar of the base `USE` statement, so it
    /// requires [`use_statement`](UtilitySyntax::use_statement): without it the leading
    /// `USE` is not dispatched and this flag is inert, the dependency the
    /// [`UseStringLiteralNameWithoutUseStatement`](crate::dialect::FeatureDependencyViolation::UseStringLiteralNameWithoutUseStatement)
    /// registry variant records. On for DuckDB and the permissive superset; off for MySQL,
    /// whose `USE ident` grammar `ER_PARSE_ERROR`s a string name (engine-measured on
    /// mysql:8), and off (vacuously) wherever `use_statement` is off.
    pub use_string_literal_name: bool,
    /// Accept the prepared-statement lifecycle: `PREPARE <name> [(<types>)] AS
    /// <statement>`, `EXECUTE <name> [(<args>)]`, and `DEALLOCATE [PREPARE] <name>`. One
    /// flag gates the three leading keywords because they are a single dialect unit — a
    /// dialect that prepares statements executes and frees them too (the
    /// `attach`/`detach` single-flag precedent). On for DuckDB, PostgreSQL, and Lenient;
    /// off elsewhere, where the leading keywords surface as unknown statements.
    /// (`EXECUTE` is only a *leading* keyword here; the `EXECUTE` privilege in `GRANT
    /// EXECUTE` is unaffected.) The parenthesized `PREPARE` type list is a separate,
    /// differently-shaped surface gated by
    /// [`prepare_typed_parameters`](UtilitySyntax::prepare_typed_parameters): this flag alone only
    /// admits the bare `PREPARE <name> AS <statement>` form. Never both on with MySQL's
    /// [`prepared_statements_from`](UtilitySyntax::prepared_statements_from) — the pair is the
    /// registered [`GrammarConflict::PreparedStatementsVersusPreparedStatementsFrom`].
    pub prepared_statements: bool,
    /// Accept the PostgreSQL `PREPARE name ( <type> [, ...] ) AS <statement>`
    /// parenthesized parameter-type list between the name and `AS` — full type names
    /// (parameterized like `numeric(10,2)`, arrayed like `int[]`), at least one,
    /// PostgreSQL rejects an empty `()`. Depends on
    /// [`prepared_statements`](UtilitySyntax::prepared_statements) being on too (the type list is
    /// a widening of that grammar's name position, not a standalone surface); the
    /// dependency is
    /// [`FeatureDependencyViolation::PrepareTypedParametersWithoutPreparedStatements`].
    /// On for PostgreSQL and Lenient; off for DuckDB, which structurally rejects the
    /// whole typed-parameter-list form ("Prepared statement argument types are not
    /// supported, use CAST") — so it keeps `prepared_statements` for the bare form only.
    pub prepare_typed_parameters: bool,
    /// Accept MySQL's prepared-statement lifecycle: `PREPARE <name> FROM {'<text>' | @<var>}`,
    /// `EXECUTE <name> [USING @<var>, ...]`, and `{DEALLOCATE | DROP} PREPARE <name>`. One
    /// flag gates the three leading keywords (plus the `DROP PREPARE` synonym) because they
    /// are a single dialect unit — the `prepared_statements` single-flag precedent.
    ///
    /// A *different grammar on the same three keywords* from DuckDB's typed-`AS`
    /// [`prepared_statements`](UtilitySyntax::prepared_statements): MySQL prepares from a
    /// statement *source* (a string literal or a `@`-variable, never an inline-parsed
    /// statement), executes with a `USING` clause of `@`-variable references (never
    /// parenthesized expressions), and requires the `PREPARE` keyword after `{DEALLOCATE |
    /// DROP}`. The two are mutually exclusive per preset (each arms at most one), the split
    /// mirroring [`do_statement`](UtilitySyntax::do_statement) vs
    /// [`do_expression_list`](UtilitySyntax::do_expression_list) on the `DO` keyword; a preset
    /// never arms both, so the shared leading keywords dispatch unambiguously. MySQL-only among
    /// the shipped presets; off elsewhere (the permissive superset keeps the DuckDB typed-`AS`
    /// reading, since the `FROM`/`USING` surfaces have no positional-argument spelling). Never
    /// both on with [`prepared_statements`](UtilitySyntax::prepared_statements) — the pair is the
    /// registered [`GrammarConflict::PreparedStatementsVersusPreparedStatementsFrom`], whose
    /// `DEALLOCATE` tail (mandatory `PREPARE` under this flag) is incoherent with the DuckDB-first
    /// dispatch of the `PREPARE`/`EXECUTE` heads. See
    /// [`PrepareFromStatement`](crate::ast::PrepareFromStatement) and
    /// [`ExecuteUsingStatement`](crate::ast::ExecuteUsingStatement).
    pub prepared_statements_from: bool,
    /// Accept the DuckDB `CALL <name>(<args>)` routine/table-function invocation
    /// statement. Its own flag rather than sharing
    /// [`prepared_statements`](UtilitySyntax::prepared_statements): `CALL` is an independent
    /// procedure-call statement, not part of the prepare/execute/deallocate lifecycle
    /// (the `vacuum`/`reindex`/`analyze` separate-flags precedent). DuckDB-only among the
    /// shipped fitted presets (and the permissive superset); off elsewhere, where a
    /// leading `CALL` surfaces as an unknown statement.
    pub call: bool,
    /// Accept MySQL's bare `CALL <name>` form — a routine invocation with no parenthesized
    /// argument list at all (`CALL_SYM sp_name opt_paren_expr_list`, the `opt_paren_expr_list`
    /// %empty alternative) — on top of the base [`call`](UtilitySyntax::call) statement (the
    /// dependency is [`FeatureDependencyViolation::CallBareNameWithoutCall`]). MySQL-only among
    /// the shipped presets (and Lenient); DuckDB's parentheses are mandatory (a bare
    /// `CALL pragma_version` is a syntax error), so it is off there and a `CALL name` with no
    /// following `(` rejects.
    pub call_bare_name: bool,
    /// Accept the `LOAD <string>` extension/shared-library load statement. On for
    /// PostgreSQL/DuckDB/Lenient (PostgreSQL `LOAD 'plpgsql'`, DuckDB `LOAD 'tpch'` —
    /// both accept a string argument); off in ANSI/MySQL/SQLite, where the leading `LOAD`
    /// surfaces as an unknown statement. The DuckDB bare-identifier argument rides the
    /// separate [`load_bare_name`](UtilitySyntax::load_bare_name) gate.
    pub load_extension: bool,
    /// Accept DuckDB's bare-identifier `LOAD <name>` argument (`LOAD tpch`) on top of the
    /// base [`load_extension`](UtilitySyntax::load_extension) statement (the dependency is
    /// [`FeatureDependencyViolation::LoadBareNameWithoutLoadExtension`]). DuckDB-only among the
    /// shipped presets (and Lenient); PostgreSQL's `LOAD` requires a string
    /// (`LOAD tpch` is a pg_query parser error), so it is off there.
    pub load_bare_name: bool,
    /// Accept MySQL's `LOAD {DATA | XML} … INFILE … INTO TABLE …` bulk-import statement — a
    /// DIFFERENT behaviour on the leading `LOAD` keyword from the PostgreSQL/DuckDB
    /// [`load_extension`](UtilitySyntax::load_extension) statement, dispatched on the two-word
    /// `LOAD DATA`/`LOAD XML` lookahead so the two never collide even where both gates are on
    /// (the `do_statement`/`do_expression_list` split precedent). MySQL-only among the shipped
    /// presets (and Lenient); off elsewhere, where a leading `LOAD DATA` surfaces as an unknown
    /// statement. Covers the classic documented clause train (`PARTITION`, `CHARACTER SET`,
    /// `FIELDS`/`LINES`, `IGNORE n LINES`, the column/`@var` list, `SET`, and — grammar-shared —
    /// `ROWS IDENTIFIED BY`); the MySQL 8.4 secondary-engine bulk-load extension clauses (`URL`/
    /// `S3` sources, `COUNT`, `COMPRESSION`, `PARALLEL`, `MEMORY`, `ALGORITHM`) are a separate
    /// feature not covered by this gate.
    pub load_data: bool,
    /// Accept a `SESSION | LOCAL | GLOBAL` scope qualifier before a `RESET <name>` target
    /// (DuckDB `RESET SESSION x`, `RESET GLOBAL x`; DuckDB parse-accepts all three, though
    /// `RESET LOCAL` is a runtime not-implemented). DuckDB-only among the shipped presets
    /// (and Lenient); PostgreSQL's `RESET` takes no scope prefix (`RESET SESSION x` is a
    /// pg_query parser error — its `RESET SESSION AUTHORIZATION` is a distinct special
    /// form, not modelled), so it is off there and a scope keyword after `RESET` rejects.
    pub reset_scope: bool,
    /// Accept a DuckDB `IF EXISTS` guard on `DETACH DATABASE IF EXISTS <name>` (on top of
    /// the [`attach`](UtilitySyntax::attach) `DETACH` statement; the dependency is
    /// [`FeatureDependencyViolation::DetachIfExistsWithoutAttach`]). DuckDB-only among the shipped
    /// presets (and Lenient); DuckDB admits the guard only after the `DATABASE` keyword
    /// (`DETACH IF EXISTS x` is a parser error, `DETACH DATABASE IF EXISTS x` parses —
    /// probed on 1.5.4). SQLite's `DETACH` has no `IF EXISTS`, so it is off there.
    pub detach_if_exists: bool,
    /// Accept the PostgreSQL `DO [LANGUAGE <lang>] '<body>'` anonymous code block. Its own
    /// leading-keyword gate, like [`copy`](UtilitySyntax::copy): PostgreSQL-only among the shipped
    /// presets (and Lenient); DuckDB has no `DO` statement (probed on 1.5.4: `DO $$...$$`
    /// is a parser error), and MySQL/SQLite have none, so there the leading `DO` keyword is
    /// not dispatched and surfaces as an unknown statement. The block body is an opaque
    /// procedural-language string, not re-parsed here (see
    /// [`DoStatement`](crate::ast::DoStatement)). Never both on with
    /// [`do_expression_list`](UtilitySyntax::do_expression_list) — the pair is the registered
    /// [`GrammarConflict::DoStatementVersusDoExpressionList`].
    pub do_statement: bool,
    /// Accept the MySQL `DO <expr> [, <expr> ...]` evaluate-and-discard statement — a
    /// *different behaviour on the same `DO` keyword* from the PostgreSQL anonymous code block
    /// gated by [`do_statement`](UtilitySyntax::do_statement). The two are mutually exclusive
    /// per dialect (each preset arms at most one), the split mirroring transaction-`BEGIN`
    /// vs compound-block-`BEGIN`; a preset never arms both, so the shared leading `DO` keyword
    /// dispatches unambiguously. MySQL-only among the shipped presets; off elsewhere (the
    /// permissive superset keeps the PostgreSQL code-block reading, since the `DO LANGUAGE`
    /// form has no expression-list spelling). Enabling both at once is the registered
    /// [`GrammarConflict::DoStatementVersusDoExpressionList`] — the code-block branch shadows the
    /// expression list, so `DO 'x'` mis-parses and `DO 1, 2` over-rejects. See
    /// [`DoExpressionsStatement`](crate::ast::DoExpressionsStatement).
    pub do_expression_list: bool,
    /// Accept the MySQL `LOCK {TABLES | TABLE} <tbl> [[AS] <alias>] {READ [LOCAL] | WRITE}
    /// [, ...]` explicit table-locking statement and its `UNLOCK {TABLES | TABLE}` release
    /// counterpart (one gate for the pair, the [`rename_statement`](Self::rename_statement)
    /// precedent: MySQL's `lock`/`unlock` grammar rules carry both — a dialect with one has
    /// both). This is the *per-table lock-kind* reading of the leading `LOCK` keyword —
    /// a [`do_statement`](Self::do_statement)/[`do_expression_list`](Self::do_expression_list)-style
    /// behaviour split: PostgreSQL's `LOCK [TABLE] <rel>, … [IN <mode> MODE] [NOWAIT]`
    /// statement-level mode-list reading is a different behaviour on the same keyword and,
    /// when implemented, takes its own gate — a preset would arm at most one, so the shared
    /// leading `LOCK`/`UNLOCK` keywords dispatch unambiguously. MySQL-only among the shipped
    /// presets (plus the Lenient superset, where it is a pure addition *today* — no other
    /// `LOCK`-keyword statement exists; the union will owe a reading decision when the
    /// PostgreSQL form lands). Off elsewhere, where the leading keyword is not dispatched and
    /// surfaces as an unknown statement. See
    /// [`LockTablesStatement`](crate::ast::LockTablesStatement) /
    /// [`UnlockTablesStatement`](crate::ast::UnlockTablesStatement).
    pub lock_tables: bool,
    /// Accept the MySQL `LOCK INSTANCE FOR BACKUP` / `UNLOCK INSTANCE` instance-wide
    /// backup-lock pair (one gate for both, as with [`lock_tables`](Self::lock_tables) —
    /// the same `lock`/`unlock` grammar rules carry them). A separate flag from
    /// `lock_tables` because the two surfaces are independent behaviours that only happen to
    /// share MySQL: the backup lock is a MySQL-8-specific administrative statement with no
    /// table list (MariaDB, for one, spells it `BACKUP LOCK` instead while sharing `LOCK
    /// TABLES`), and it is additionally collision-free with the future PostgreSQL mode-list
    /// reading (`LOCK instance …` continues with `IN`/`NOWAIT`/end there, never `FOR`).
    /// MySQL-only among the shipped presets (plus the Lenient superset). See
    /// [`InstanceLockStatement`](crate::ast::InstanceLockStatement).
    pub lock_instance: bool,
    /// Accept the MySQL standalone `RENAME TABLE <a> TO <b>[, ...]` and `RENAME USER <u>
    /// TO <v>[, ...]` object-rename statements (both →
    /// [`Statement::Rename`](crate::ast::Statement::Rename)). A leading-keyword gate like
    /// [`kill`](Self::kill): off outside MySQL (and the Lenient superset), where the
    /// leading `RENAME` keyword is not dispatched and surfaces as an unknown statement. The
    /// two forms share one gate because MySQL's single `rename` grammar rule carries both —
    /// a dialect with one has both. Distinct from the `ALTER TABLE ... RENAME TO`
    /// sub-clause, which is consumed by the `ALTER TABLE` grammar and never reaches this
    /// leading position.
    pub rename_statement: bool,
    /// Accept the MySQL diagnostics-area family — `SIGNAL`, `RESIGNAL`, and
    /// `GET [CURRENT | STACKED] DIAGNOSTICS` — as top-level statements (a leading-keyword gate
    /// like [`kill`](UtilitySyntax::kill)). One flag for all three: they are the single
    /// cohesive behaviour of manipulating the diagnostics area (raise / re-raise / read).
    ///
    /// Its own axis, NOT the body-context
    /// [`compound_statements`](StatementDdlGates::compound_statements) flag: these three attach
    /// to MySQL's top-level `simple_statement` production and are engine-recognized at top
    /// level (measured `1295`/`ER_UNSUPPORTED_PS` over the PREPARE oracle — grammar-valid,
    /// merely not preparable), exactly where a compound `BEGIN … END` block is NOT (a bare
    /// top-level `BEGIN` is transaction-start). Because the body dispatcher falls through to
    /// the top-level one for non-compound keywords, this single gate serves both surfaces. On
    /// for MySQL (and Lenient); off elsewhere, where the leading `SIGNAL`/`RESIGNAL`/`GET`
    /// keyword is left unconsumed and surfaces as an unknown statement.
    pub signal_diagnostics: bool,
    /// Accept the DuckDB `EXPORT DATABASE ['<db>' TO] '<path>' [<copy-options>]` catalogue
    /// dump *and* its `IMPORT DATABASE '<path>'` inverse. One flag gates both leading
    /// keywords because they are a single dialect unit — the two halves of one
    /// export/import round-trip, a dialect that dumps a database replays it (the same
    /// pairing reasoning [`attach`](Self::attach) uses for `ATTACH`/`DETACH`). DuckDB-only
    /// (and the permissive superset); off in ANSI, PostgreSQL, MySQL, and SQLite, which have
    /// no `EXPORT`/`IMPORT DATABASE`, so there the leading keywords are not dispatched and
    /// surface as unknown statements — the `copy`/`attach` leading-keyword-gate precedent.
    pub export_import_database: bool,
    /// Accept the DuckDB `UPDATE EXTENSIONS [( <name>, ... )]` extension-refresh statement
    /// ([`Statement::UpdateExtensions`](crate::ast::Statement::UpdateExtensions)). A
    /// *refinement* of the leading `UPDATE`, not a bare leading-keyword gate: a top-level
    /// `UPDATE` whose next word is the DuckDB-unreserved `EXTENSIONS` keyword is this
    /// statement only when that word is followed by the parenthesized name list or the
    /// statement end; an `UPDATE extensions SET …` (a table literally named `extensions`,
    /// or `… AS e SET …`) still routes to the DML `UPDATE`, exactly as DuckDB's own grammar
    /// resolves the shared prefix (engine-probed on 1.5.4). Off for every non-DuckDB preset
    /// (bar the Lenient superset), where the `EXTENSIONS` lookahead is never taken and every
    /// `UPDATE` reaches the DML parser unchanged.
    pub update_extensions: bool,
    /// Accept the MySQL `FLUSH [NO_WRITE_TO_BINLOG | LOCAL] <target>` server-administration
    /// statement ([`Statement::Flush`](crate::ast::Statement::Flush)) — the `{TABLE | TABLES}
    /// [<list>] [WITH READ LOCK | FOR EXPORT]` form and the comma-separated keyword-target
    /// list (`PRIVILEGES`, `LOGS`, `STATUS`, `RELAY LOGS FOR CHANNEL …`, …). A leading-keyword
    /// gate like [`kill`](Self::kill): on for MySQL (and the Lenient superset), off elsewhere,
    /// where the leading `FLUSH` keyword is not dispatched and surfaces as an unknown
    /// statement.
    pub flush: bool,
    /// Accept the MySQL `PURGE BINARY LOGS {TO '<log>' | BEFORE <datetime>}` binary-log purge
    /// statement ([`Statement::Purge`](crate::ast::Statement::Purge)). A leading-keyword gate
    /// like [`kill`](Self::kill): on for MySQL (and the Lenient superset), off elsewhere,
    /// where the leading `PURGE` keyword is not dispatched and surfaces as an unknown
    /// statement. Named for the only surviving 8.4 form — the deprecated `MASTER` synonym was
    /// removed, so this gate carries `BINARY LOGS` alone.
    pub purge_binary_logs: bool,
    /// Accept the MySQL replication-administration family
    /// ([`Statement::Replication`](crate::ast::Statement::Replication)) — `CHANGE REPLICATION
    /// SOURCE TO <options>`, `CHANGE REPLICATION FILTER <rules>`, `START`/`STOP REPLICA`, and
    /// `START`/`STOP GROUP_REPLICATION`. One gate for all five measured families because they
    /// are one cohesive dialect unit: MySQL's replication-control surface, reached through the
    /// replication-specific leading-keyword sequences (`CHANGE REPLICATION`, `START`/`STOP
    /// REPLICA`, `START`/`STOP GROUP_REPLICATION`). A dialect either implements MySQL
    /// replication administration or it does not — Group Replication rides the same unit as
    /// classic asynchronous replication (both are the server's replication control, not a
    /// severable syntax axis). A *refinement* of the shared `START`/`STOP`/`CHANGE` leading
    /// keywords (not a bare leading-keyword gate like [`kill`](Self::kill)): the dispatch
    /// claims `START`/`STOP` only when the next word is `REPLICA`/`GROUP_REPLICATION` and
    /// `CHANGE` only before `REPLICATION`, so `START TRANSACTION` and every other use of those
    /// keywords is untouched. On for MySQL (and the Lenient superset); off elsewhere, where
    /// the sequences are not dispatched and surface as unknown statements. MySQL 8.4 removed
    /// the legacy `MASTER`/`SLAVE` spellings, so only the `REPLICATION`/`REPLICA` grammar is
    /// accepted.
    pub replication_statements: bool,
}

/// Dialect-owned SHOW/DESCRIBE introspection-statement syntax accepted by the parser.
///
/// The session `SHOW`/`SET`/`RESET` reader, the typed `SHOW <object>` catalogue
/// listings, and the MySQL/DuckDB `DESCRIBE`/`SUMMARIZE` introspection statements. Split
/// out of [`UtilitySyntax`] at its 16-field line as the introspection axis. Each flag is a
/// leading-keyword dispatch gate: when off the keyword is not dispatched and surfaces as
/// an unknown statement (or the typed-`SHOW` lookahead falls through to the session reader).
/// **Statement-head gates are consumed by `parser::statement_dispatch`**; parse bodies live
/// in the SHOW/session family modules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShowSyntax {
    /// Accept MySQL's `DESCRIBE`/`DESC` keywords as EXPLAIN synonyms *and* the MySQL
    /// `{DESCRIBE | DESC | EXPLAIN} <table> [<column> | '<pattern>']` table-metadata
    /// overload. MySQL-only (and the permissive superset). When on: the leading `DESCRIBE`
    /// and `DESC` keywords are dispatched into the EXPLAIN grammar (spelling recorded on
    /// [`ExplainStatement`](crate::ast::ExplainStatement)), and all three EXPLAIN-family
    /// keywords additionally accept a table name in place of an explainable statement
    /// (yielding a [`DescribeStatement`](crate::ast::DescribeStatement)). When off
    /// (ANSI/PostgreSQL/SQLite): `DESCRIBE`/`DESC` are not statement leaders and `EXPLAIN`
    /// keeps its plain query-plan-only grammar, so a table after `EXPLAIN` is rejected as
    /// PostgreSQL does. `EXPLAIN` itself stays ungated (accepted everywhere).
    pub describe: bool,
    /// Accept DuckDB's leading-keyword `{DESCRIBE | SUMMARIZE} <query> | <table>`
    /// introspection statement ([`Statement::ShowRef`](crate::ast::Statement::ShowRef)).
    /// DuckDB desugars it to `SELECT * FROM (<SHOW_REF>)`, so it reuses the same `SHOW_REF`
    /// core ([`ShowRef`](crate::ast::ShowRef)) as the parenthesized table factor, only at
    /// statement-leading position. DuckDB-only (and the permissive superset).
    ///
    /// Distinct from [`describe`](ShowSyntax::describe): that flag is MySQL's overload of the
    /// EXPLAIN keyword (`DESCRIBE` as a query-plan synonym, plus the `<table> [<column>]`
    /// metadata form) and never touches `SUMMARIZE`; this one is DuckDB's `SHOW_REF`
    /// utility, whose `DESCRIBE`/`SUMMARIZE` return the target's schema / summary statistics
    /// as a relation. The two share the `DESCRIBE` leader but are *not* a
    /// [`GrammarConflict`]: every real-dialect preset enables at most one, and Lenient — the
    /// one preset with both on — resolves the shared leader deterministically by dispatch
    /// order (`DESCRIBE`/`DESC` route to the MySQL EXPLAIN synonym above; only `SUMMARIZE`
    /// reaches this `SHOW_REF` utility), a conflict-free permissive union rather than a mutual
    /// exclusion, so no registry variant governs it. Boundary with
    /// [`TableFactorSyntax::show_ref`](crate::dialect::TableFactorSyntax::show_ref):
    /// that flag owns the same core inside a `FROM (…)` table factor; this one owns the
    /// statement-leading spelling — one production, two grammar positions, one flag each.
    pub describe_summarize: bool,
    /// Accept the session statements `SET <var> …` / `RESET …` / `SHOW …` (PostgreSQL/
    /// MySQL; the standard `SET CONSTRAINTS`/`SET SESSION CHARACTERISTICS` also route
    /// here). On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite has no session-variable
    /// statement, so it is off; the leading keyword is then not dispatched and surfaces as
    /// an unknown statement. `SET TRANSACTION` is transaction control (claimed earlier in
    /// dispatch), so it is unaffected by this gate.
    pub session_statements: bool,
    /// Keywords rejected as unquoted values in the generic
    /// `SET <parameter> {= | TO} <value> [, ...]` grammar. String and numeric literals,
    /// ordinary identifiers, and non-reserved keywords remain accepted. PostgreSQL's
    /// `var_value` production and DuckDB's corresponding grammar both enforce this
    /// boundary; permissive compatibility dialects may accept any keyword.
    pub set_value_reserved_words: KeywordSet,
    /// Accept the otherwise-reserved `ON` keyword as a generic `SET` value.
    /// PostgreSQL names `ON` explicitly in `opt_boolean_or_string`; DuckDB does not.
    /// `TRUE` and `FALSE` are accepted independently as boolean values whenever reserved
    /// words are rejected.
    pub set_value_on_keyword: bool,
    /// Accept the otherwise-reserved `NULL` keyword as a generic `SET` value. DuckDB
    /// accepts this spelling; PostgreSQL's `var_value` production does not.
    pub set_value_null_keyword: bool,
    /// Accept the typed `SHOW [EXTENDED] [FULL] [ALL] TABLES [{FROM | IN} <db>] [LIKE
    /// '<pat>' | WHERE <expr>]` catalogue-listing statement
    /// ([`Statement::Show`](crate::ast::Statement::Show)). On for MySQL/DuckDB/Lenient;
    /// off for ANSI/PostgreSQL/SQLite.
    ///
    /// This is a *refinement* of the generic-`SHOW` dispatch, so its boundary with the
    /// two other `SHOW` seams is MECE:
    /// - [`session_statements`](ShowSyntax::session_statements) owns the generic top-level
    ///   `SHOW <var>` that reads one configuration parameter. When `show_tables` is on,
    ///   a top-level `SHOW` whose next word (past the optional `EXTENDED`/`FULL`/`ALL`
    ///   modifiers) is `TABLES` is claimed here instead; every other `SHOW <var>` still
    ///   falls through to the session statement. So PostgreSQL (this flag off) keeps
    ///   parsing `SHOW tables` as a generic session `SHOW` — the parse it already accepts
    ///   — and only MySQL/DuckDB reinterpret the `TABLES` keyword as the catalogue listing.
    /// - [`TableFactorSyntax::show_ref`](crate::dialect::TableFactorSyntax::show_ref)
    ///   owns the parenthesized `(SHOW <name>)` / `(DESCRIBE …)` *table factor* that only
    ///   appears inside a `FROM (…)` and yields a relation; it never reaches the
    ///   statement-leading position this flag governs.
    ///
    /// One flag per typed-`SHOW` subform (the sibling `SHOW COLUMNS`/`SHOW CREATE TABLE`/
    /// `SHOW FUNCTIONS` tickets add their own), because their per-dialect availability
    /// differs — the `copy`/`comment_on` separate-flags precedent. Once on, the single
    /// flag admits the whole modifier union permissively (MySQL's `FULL`/`LIKE`/`WHERE`,
    /// DuckDB's `ALL`), the DESCRIBE/PRAGMA single-flag-utility precedent — the corpus
    /// verdict gate catches only corpus-present over-acceptance.
    pub show_tables: bool,
    /// Accept the typed `SHOW [EXTENDED] [FULL] {COLUMNS | FIELDS} {FROM | IN} <tbl>
    /// [{FROM | IN} <db>] [LIKE '<pat>' | WHERE <expr>]` column-listing statement
    /// ([`Statement::Show`](crate::ast::Statement::Show) with
    /// [`ShowTarget::Columns`](crate::ast::ShowTarget)). On for MySQL/Lenient only; off for
    /// ANSI/PostgreSQL/SQLite/DuckDB.
    ///
    /// A separate gate from [`show_tables`](ShowSyntax::show_tables) because its per-dialect
    /// availability differs: DuckDB accepts `SHOW [ALL] TABLES` but has *no* `SHOW COLUMNS`
    /// grammar at all (every `SHOW {COLUMNS | FIELDS} …` form is `ER_PARSE_ERROR` on DuckDB
    /// 1.5.4, engine-probed — it uses `DESCRIBE` / `SHOW <table>` instead), so a shared flag
    /// would over-accept there. Same MECE refinement of the generic-`SHOW` dispatch as
    /// `show_tables`: a top-level `SHOW` whose next word past the optional `EXTENDED`/`FULL`
    /// modifiers is `COLUMNS` or the `FIELDS` synonym is claimed here; every other
    /// `SHOW <var>` still falls through to [`session_statements`](ShowSyntax::session_statements),
    /// and the parenthesized `(SHOW <name>)`
    /// [`show_ref`](crate::dialect::TableFactorSyntax::show_ref) table factor is
    /// untouched. Once on, the single flag admits the whole MySQL modifier/filter union
    /// permissively (the DESCRIBE/PRAGMA single-flag-utility precedent).
    pub show_columns: bool,
    /// Accept the typed `SHOW CREATE TABLE <tbl>` statement — the `CREATE TABLE` DDL that
    /// would recreate the named table ([`Statement::Show`](crate::ast::Statement::Show) with
    /// [`ShowTarget::Create`](crate::ast::ShowTarget) and
    /// [`ShowCreateKind::Table`](crate::ast::ShowCreateKind)). On for MySQL/Lenient only; off
    /// for ANSI/PostgreSQL/SQLite/DuckDB. The other `SHOW CREATE <kind>` object kinds ride
    /// [`show_admin`](ShowSyntax::show_admin).
    ///
    /// A separate gate from [`show_tables`](ShowSyntax::show_tables)/[`show_columns`](ShowSyntax::show_columns)
    /// because its per-dialect availability differs: MySQL has `SHOW CREATE TABLE` but DuckDB
    /// has no such grammar (it uses `SELECT sql FROM duckdb_tables` / `.schema`), so a shared
    /// flag would over-accept there. Same MECE refinement of the generic-`SHOW` dispatch: a
    /// top-level `SHOW` whose next two words are `CREATE TABLE` is claimed here; a bare
    /// `SHOW create` (the two-keyword lookahead requires `TABLE` to follow) and every other
    /// `SHOW <var>` still fall through to [`session_statements`](ShowSyntax::session_statements),
    /// so PostgreSQL's generic `SHOW <var>` reading `create` as the variable name is
    /// undisturbed. There are no `EXTENDED`/`FULL` modifiers on this subform (MySQL docs).
    /// Only the `TABLE` object kind is modelled; `SHOW CREATE {DATABASE | VIEW | …}` is
    /// deferred to sibling tickets.
    pub show_create_table: bool,
    /// Accept the typed `SHOW [{USER | SYSTEM | ALL}] FUNCTIONS [{FROM | IN} <schema>]
    /// [[LIKE] {<function_name> | '<regex>'}]` function-listing statement
    /// ([`Statement::Show`](crate::ast::Statement::Show) with
    /// [`ShowTarget::Functions`](crate::ast::ShowTarget)). On for Databricks/Lenient only;
    /// off for ANSI/PostgreSQL/MySQL/SQLite/DuckDB.
    ///
    /// A separate gate from the other `show_*` subforms because its per-dialect
    /// availability differs — and it is the first typed-`SHOW` flag on under Databricks,
    /// where the other three are off. Spark/Databricks are the only shipped engines with a
    /// bare `SHOW FUNCTIONS` listing (doc-cited grammar); MySQL's `SHOW FUNCTION STATUS` is
    /// a *different* routine-catalogue statement (deferred to its own ticket), and DuckDB
    /// has no `SHOW FUNCTIONS` grammar at all — `SHOW <name>` there is a `DESCRIBE` alias,
    /// so `SHOW functions` describes a table named `functions` (engine-probed on 1.5.4), a
    /// generic-`SHOW` reinterpretation a shared flag would corrupt. Same MECE refinement of
    /// the generic-`SHOW` dispatch: a top-level `SHOW` whose next word (past the optional
    /// `USER`/`SYSTEM`/`ALL` scope) is `FUNCTIONS` is claimed here; a bare `SHOW <var>`
    /// (including `SHOW ALL` with no `FUNCTIONS`) still falls through to
    /// [`session_statements`](ShowSyntax::session_statements). Once on, the single flag admits the
    /// whole modifier/filter union permissively (the DESCRIBE/PRAGMA single-flag-utility
    /// precedent).
    pub show_functions: bool,
    /// Accept the typed `SHOW {FUNCTION | PROCEDURE} STATUS [LIKE '<pat>' | WHERE <expr>]`
    /// stored-routine catalogue listing
    /// ([`Statement::Show`](crate::ast::Statement::Show) with
    /// [`ShowTarget::RoutineStatus`](crate::ast::ShowTarget)). On for MySQL/Lenient only;
    /// off for ANSI/PostgreSQL/SQLite/DuckDB/Databricks.
    ///
    /// A separate gate from [`show_functions`](ShowSyntax::show_functions) because it is a
    /// *different statement*, not the same one under another dialect: MySQL's routine
    /// catalogue takes the singular `FUNCTION`/`PROCEDURE` object keyword plus a mandatory
    /// `STATUS` and lists a row per stored routine, where the Spark/Databricks
    /// `show_functions` listing takes the bare plural `FUNCTIONS` and lists function names.
    /// Their per-dialect availability is disjoint (MySQL rejects `SHOW FUNCTIONS`,
    /// Databricks rejects `SHOW FUNCTION STATUS`), so a shared flag would over-accept in
    /// both directions — the one-flag-per-typed-`SHOW`-subform precedent. Same MECE
    /// refinement of the generic-`SHOW` dispatch as the sibling gates: a top-level `SHOW`
    /// whose next two words are `FUNCTION STATUS` or `PROCEDURE STATUS` is claimed here. The
    /// two-keyword lookahead steals only that full prefix; `FUNCTION`/`PROCEDURE` are reserved
    /// keywords, so a bare `SHOW FUNCTION` cannot be a generic session `SHOW <var>` (like
    /// `SHOW CREATE`, it is a parse error both ways), and every other `SHOW <var>` still falls
    /// through to [`session_statements`](ShowSyntax::session_statements). There is no scope keyword and
    /// no `{FROM | IN}` qualifier — `SHOW FUNCTION STATUS FROM db` is `ER_PARSE_ERROR` on
    /// mysql:8 (engine-probed) — only the optional `LIKE`/`WHERE` narrowing, which reuses the
    /// shared [`ShowFilter`](crate::ast::ShowFilter).
    pub show_routine_status: bool,
    /// Accept the trailing `VERBOSE` on a generic session `SHOW` — `SHOW ALL VERBOSE`,
    /// `SHOW <setting> VERBOSE` — carried on
    /// [`SessionStatement::Show::verbose`](crate::ast::SessionStatement). On for Lenient
    /// only; off for every oracle-backed dialect.
    ///
    /// `VERBOSE` here is the sqlparser-rs/DataFusion planner spelling, *not* a database
    /// grammar: `pg_query` and DuckDB both reject `SHOW ALL VERBOSE` and
    /// `SHOW <setting> VERBOSE` (engine-probed), so no dialect with a real oracle can turn
    /// it on without diverging. It refines only the session `SHOW` seam and never the
    /// typed-`SHOW` dispatch: the `TABLES`/`COLUMNS`/`CREATE TABLE`/`FUNCTIONS` lookaheads
    /// each insist on their keyword after the modifiers, so `SHOW ALL VERBOSE` (no
    /// `TABLES`) falls through to the session branch and `SHOW ALL TABLES` stays typed —
    /// the seams remain MECE. A behaviour-named tail flag, not gated on
    /// [`session_statements`](ShowSyntax::session_statements): where session `SHOW` is off
    /// (SQLite) the keyword is never dispatched, so this flag is inert there regardless.
    pub show_verbose: bool,
    /// Accept the MySQL server-administration / catalogue-introspection `SHOW` sub-command
    /// family — the ~40 `SHOW` productions beyond the individually-gated
    /// `TABLES`/`COLUMNS`/`CREATE TABLE`/`{FUNCTION|PROCEDURE} STATUS` subforms:
    /// `SHOW DATABASES`, `SHOW [GLOBAL|SESSION] {STATUS|VARIABLES}`, `SHOW PLUGINS`,
    /// `SHOW [STORAGE] ENGINES`, `SHOW ENGINE <e> {STATUS|MUTEX|LOGS}`, `SHOW PRIVILEGES`,
    /// `SHOW {CHARACTER SET|CHARSET}`, `SHOW COLLATION`, `SHOW EVENTS`, `SHOW TABLE STATUS`,
    /// `SHOW OPEN TABLES`, `SHOW [FULL] TRIGGERS`, `SHOW [FULL] PROCESSLIST`,
    /// `SHOW CREATE {VIEW|DATABASE|EVENT|PROCEDURE|FUNCTION|TRIGGER} <name>`,
    /// `SHOW [EXTENDED] {INDEX|INDEXES|KEYS} FROM <t>`, `SHOW GRANTS`,
    /// `SHOW {WARNINGS|ERRORS} [LIMIT …]` / `SHOW COUNT(*) {WARNINGS|ERRORS}`,
    /// `SHOW BINARY LOGS`, `SHOW REPLICAS`, `SHOW BINARY LOG STATUS`,
    /// `SHOW REPLICA STATUS [FOR CHANNEL …]`, `SHOW PROFILES`, and
    /// `SHOW {PROCEDURE|FUNCTION} CODE <name>` (all → [`Statement::Show`](crate::ast::Statement::Show)).
    /// On for MySQL/Lenient only; off for every other preset.
    ///
    /// This is a *single* behaviour flag for the whole family rather than one flag per
    /// sub-command (the `show_tables`/`show_columns` precedent) because, unlike those, this
    /// family's per-dialect availability does not vary — every member is MySQL-only and they
    /// travel together; the sub-command identity is DATA (the [`ShowTarget`](crate::ast::ShowTarget)
    /// `Listing`/`Bare`/`Create`/`Index`/`Engine`/… axis), not a separate behaviour axis.
    /// The parser reaches it through one table-driven dispatch (`parse_show_admin_statement`),
    /// not one arm per keyword. Same MECE refinement of the generic-`SHOW` dispatch as the
    /// sibling gates: the lookahead insists on one of the family's lead keywords, so any
    /// other `SHOW <var>` still falls through to
    /// [`session_statements`](ShowSyntax::session_statements). The gate also covers the
    /// grammatically heavier subforms with operands (`SHOW GRANTS FOR <user> [USING …]`,
    /// `SHOW CREATE USER`, `SHOW PROFILE`, `SHOW {BINLOG | RELAYLOG} EVENTS`); when off, every
    /// family keyword falls through unclaimed.
    pub show_admin: bool,
}

/// Dialect-owned physical-maintenance-statement syntax accepted by the parser.
///
/// The storage-maintenance statements and their operands. Split out of [`UtilitySyntax`]
/// at its 16-field line as the maintenance axis, distinct from the introspection and
/// access-control axes. Each flag is a leading-keyword dispatch gate: when off the keyword
/// is not dispatched and surfaces as an unknown statement. **Statement-head gates are
/// consumed by `parser::statement_dispatch`**; parse bodies live in the maintenance family
/// modules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaintenanceSyntax {
    /// Accept the SQLite `VACUUM [<schema>] [INTO <expr>]` database-compaction
    /// statement. SQLite-only (and the permissive superset). It takes its own flag
    /// rather than sharing one with `reindex`/`analyze` because the three are
    /// independent maintenance statements, not an inverse pair like `ATTACH`/`DETACH`
    /// — the `copy`/`comment_on` precedent (separate flags even though every shipped
    /// dialect toggles them together). PostgreSQL also has a (differently-shaped)
    /// `VACUUM`, which is not modelled, so this stays off there; when off the leading
    /// keyword is not dispatched and surfaces as an unknown statement.
    pub vacuum: bool,
    /// Accept DuckDB's `VACUUM [ANALYZE] [<table> [(<col>, …)]]` statistics/compaction
    /// statement — a *separate* leading-`VACUUM` base gate from SQLite's
    /// [`vacuum`](MaintenanceSyntax::vacuum) (the two dialects' `VACUUM` operand grammars
    /// are disjoint: SQLite's `[<schema>] INTO <expr>` versus DuckDB's `[ANALYZE] <table>
    /// (<cols>)`), so the leading keyword dispatches when *either* is on and the parser
    /// reads whichever tail its gate admits. On for DuckDB/Lenient; off elsewhere. DuckDB's
    /// `VACUUM` admits only the `ANALYZE` option — 1.5.4's transform throws
    /// `NotImplementedException` on `FULL`/`FREEZE`/`VERBOSE`/`disable_page_skipping`, so
    /// those never parse and this gate never over-accepts them. Independent of
    /// [`vacuum`](MaintenanceSyntax::vacuum): a dialect can admit one `VACUUM` grammar
    /// without the other.
    ///
    /// The column list is bundled here rather than split into a
    /// `vacuum_analyze_columns` sibling of
    /// [`analyze_columns`](MaintenanceSyntax::analyze_columns) — a deliberate
    /// granularity asymmetry. The `ANALYZE` split is measured necessity: two engines
    /// share the leading `ANALYZE` but differ on the column list (SQLite has
    /// [`analyze`](MaintenanceSyntax::analyze) without
    /// [`analyze_columns`](MaintenanceSyntax::analyze_columns); DuckDB has both). No
    /// second engine shares this `VACUUM` grammar, and DuckDB's grammar ties the column
    /// list to the table operand inseparably — engine-measured on 1.5.4, `VACUUM (a)`
    /// without a table reads the parens as the PG-legacy *options* list
    /// (`Parser Error: unrecognized VACUUM option "a"`), not columns — so a split flag
    /// would have no independent surface to gate. Split only when a second dialect's
    /// measured grammar demands it.
    pub vacuum_analyze: bool,
    /// Accept the SQLite `REINDEX [<name>]` index-rebuild statement. SQLite-only (and
    /// the permissive superset); its own flag for the same reason as
    /// [`vacuum`](MaintenanceSyntax::vacuum). Off elsewhere.
    pub reindex: bool,
    /// Accept the SQLite `ANALYZE [<name>]` / DuckDB `ANALYZE [<table>]` statistics
    /// statement (a *leading* `ANALYZE`; the `ANALYZE` option inside `EXPLAIN` is
    /// unaffected). On for SQLite/DuckDB (and the permissive superset); its own flag for
    /// the same reason as [`vacuum`](MaintenanceSyntax::vacuum). Off elsewhere.
    pub analyze: bool,
    /// Accept DuckDB's optional parenthesized column list on top of the base
    /// [`analyze`](MaintenanceSyntax::analyze) statement (`ANALYZE <table> (<col>, …)`; the
    /// dependency is [`FeatureDependencyViolation::AnalyzeColumnsWithoutAnalyze`]). DuckDB-only
    /// among the shipped presets (and Lenient); SQLite's `ANALYZE` takes no column list, so
    /// it is off there and the trailing `(` surfaces as a parser error.
    ///
    /// This split exists because two engines share the base
    /// [`analyze`](MaintenanceSyntax::analyze) statement and differ only on the column
    /// list; the DuckDB `VACUUM` column list has no such second consumer, so it stays
    /// bundled inside [`vacuum_analyze`](MaintenanceSyntax::vacuum_analyze) — see that
    /// flag's doc for the measured justification of the granularity asymmetry.
    pub analyze_columns: bool,
    /// Accept the bare `CHECKPOINT` write-ahead-log flush statement. On for
    /// PostgreSQL/DuckDB/Lenient (both engines accept a bare `CHECKPOINT` — measured on
    /// pg_query PG-17 and DuckDB 1.5.4); off in ANSI/MySQL/SQLite, where the leading
    /// keyword is not dispatched and surfaces as an unknown statement. The DuckDB `FORCE`
    /// modifier and database operand ride the separate
    /// [`checkpoint_database`](MaintenanceSyntax::checkpoint_database) gate (PostgreSQL rejects both).
    pub checkpoint: bool,
    /// Accept DuckDB's `[FORCE] CHECKPOINT [<database>]` operands on top of the base
    /// [`checkpoint`](MaintenanceSyntax::checkpoint) statement (the dependency is
    /// [`FeatureDependencyViolation::CheckpointDatabaseWithoutCheckpoint`]): the optional
    /// `FORCE` modifier and the
    /// optional single database name. DuckDB-only among the shipped presets (and Lenient);
    /// PostgreSQL's `CHECKPOINT` takes no operands (`FORCE CHECKPOINT` and `CHECKPOINT db`
    /// are pg_query parser errors), so it is off there and both forms reject.
    pub checkpoint_database: bool,
    /// Accept the MySQL admin-table maintenance verbs `{ANALYZE | CHECK | CHECKSUM |
    /// OPTIMIZE | REPAIR} {TABLE | TABLES} <table-list> [options]` (all →
    /// [`Statement::TableMaintenance`](crate::ast::Statement::TableMaintenance)). One
    /// behaviour gate for the whole five-verb family rather than one flag per verb: the
    /// verb is DATA on the [`TableMaintenanceKind`](crate::ast::TableMaintenanceKind) axis,
    /// not a separate behaviour axis — every member is MySQL-only and they travel together
    /// (the `show_admin` precedent). On for MySQL/Lenient only; off in every other preset,
    /// where the leading verb is not dispatched and surfaces as an unknown statement.
    ///
    /// The dispatch is MECE with the SQLite/DuckDB leading-`ANALYZE`
    /// [`analyze`](MaintenanceSyntax::analyze) gate: MySQL's `ANALYZE` always takes
    /// `{TABLE | TABLES}` (optionally after the `NO_WRITE_TO_BINLOG | LOCAL` prefix), so
    /// the lookahead insists on that follow-set before claiming the keyword; a bare
    /// `ANALYZE` still falls through to the sibling gate.
    pub table_maintenance: bool,
}

/// Dialect-owned access-control-statement syntax accepted by the parser.
///
/// The `GRANT`/`REVOKE` permission statements and their extended object/prefix grammar.
/// Split out of [`UtilitySyntax`] at its 16-field line as the access-control axis. Each
/// flag is a grammar gate: when off the keyword is not dispatched, or the extended object
/// grammar rejects, as the dialect requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccessControlSyntax {
    /// Accept PostgreSQL `ALTER ROLE <name> RENAME TO <new_name>`.
    pub alter_role_rename: bool,
    /// Accept the access-control statements `GRANT …` / `REVOKE …` (SQL:2016 E081;
    /// PostgreSQL/MySQL). On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite has no
    /// permission system, so it is off; the leading keyword is then not dispatched and
    /// surfaces as an unknown statement.
    pub access_control: bool,
    /// Accept the PostgreSQL/standard *extended* `GRANT`/`REVOKE` object and prefix
    /// grammar on top of the base [`access_control`](AccessControlSyntax::access_control) statements (the
    /// dependency is [`FeatureDependencyViolation::AccessControlExtendedObjectsWithoutAccessControl`]): a
    /// schema-scoped object (`ON SCHEMA s`, `ON DATABASE d`), the `ON ALL <kind> IN
    /// SCHEMA s` bulk form, and the `{GRANT | ADMIN} OPTION FOR` `REVOKE` prefix. On for
    /// ANSI/PostgreSQL/DuckDB/Lenient. MySQL admits `GRANT`/`REVOKE` but not these forms —
    /// its object grammar is `ON [TABLE | FUNCTION | PROCEDURE] priv_level`, and `SCHEMA` /
    /// `DATABASE` are reserved words that cannot introduce a priv_level, so those objects
    /// and the `OPTION FOR` prefix are the syntax error MySQL reports (engine-measured on
    /// mysql:8 — `GRANT … ON SCHEMA s`, `REVOKE GRANT OPTION FOR … `, `REVOKE … ON SCHEMA
    /// s` all `ER_PARSE_ERROR`), so it is off there. The bare/`TABLE`/`FUNCTION`/`PROCEDURE`
    /// objects, `WITH GRANT OPTION`, and the role-membership `GRANT r TO u` / `REVOKE r
    /// FROM u` forms are unaffected (MySQL accepts them), so the gate never over-rejects
    /// MySQL's supported surface. (SQLite has no permission system, so
    /// [`access_control`](AccessControlSyntax::access_control) is already off there and this is moot.)
    /// Never both on with the MySQL account route
    /// [`access_control_account_grants`](AccessControlSyntax::access_control_account_grants) — the
    /// pair is the registered [`GrammarConflict::AccountGrantsVersusExtendedObjects`], where the
    /// route dispatches its grammar before this extended-object reading is consulted.
    pub access_control_extended_objects: bool,
    /// Accept the MySQL account-management DDL family — `CREATE`/`ALTER`/`DROP USER`,
    /// `CREATE`/`DROP ROLE` — with its shared account-name (`user@host` / `CURRENT_USER`),
    /// authentication (`IDENTIFIED BY`/`WITH`), TLS (`REQUIRE`), resource (`WITH …`), and
    /// password/lock option surface. On for MySQL/Lenient. Off elsewhere (no other dialect
    /// models MySQL accounts): the leading `USER`/`ROLE` after `CREATE`/`ALTER`/`DROP` is then
    /// not dispatched and surfaces as the ordinary `TABLE`-expectation parse error. Independent
    /// of [`access_control`](AccessControlSyntax::access_control) (GRANT/REVOKE): a dialect can
    /// admit privilege statements without the account-management DDL, and vice versa.
    pub user_role_management: bool,
    /// Dispatch `GRANT`/`REVOKE` through the MySQL account-based grammar rather than the
    /// standard/PostgreSQL one (the dependency is
    /// [`FeatureDependencyViolation::AccountGrantsWithoutAccessControl`]): the object is a
    /// `priv_level` (`*`, `*.*`, `db.*`, `db.tbl`) rather than a typed object with a name list,
    /// every grantee/role is a `user@host` account rather than a role spec, and the grammar adds
    /// `PROXY` grants, the `AS <user> [WITH ROLE …]` grantor context, the
    /// `[IF EXISTS] … [IGNORE UNKNOWN USER]` `REVOKE` guards, and the `REVOKE ALL PRIVILEGES,
    /// GRANT OPTION` form — while dropping `GRANTED BY`, `CASCADE`/`RESTRICT`, and the
    /// `{GRANT | ADMIN} OPTION FOR` prefix (all engine-measured `ER_PARSE_ERROR` on mysql:8.4.10).
    ///
    /// On for MySQL only. It is a *route*, not an additive layer: the MySQL `priv_level`/account
    /// object and grantee grammar structurally conflicts with the PostgreSQL typed-object/role-spec
    /// grammar (one input, one AST — they cannot both be represented), so a dialect cannot enable
    /// both grant grammars at once — enabling this route alongside
    /// [`access_control_extended_objects`](Self::access_control_extended_objects) is the registered
    /// [`GrammarConflict::AccountGrantsVersusExtendedObjects`], the route deadening the
    /// extended-object reading. That is why the Lenient permissive superset keeps this *off* —
    /// it retains the richer PostgreSQL-extended grant grammar (schema objects, `GRANTED BY`,
    /// `CASCADE`, routine signatures) that
    /// [`access_control_extended_objects`](Self::access_control_extended_objects) governs, which a
    /// route to the MySQL grammar would forfeit. Requires
    /// [`access_control`](Self::access_control): the MySQL forms are still `GRANT`/`REVOKE`
    /// statements, unreachable without the base dispatch.
    pub access_control_account_grants: bool,
}

/// Dialect-owned type-name vocabulary extensions accepted by the parser.
///
/// The standard/PostgreSQL scalar type names are always recognized; these flags
/// gate type-name surfaces which the shared vocabulary does not cover.
/// Each is a recognition gate, not a parser-side dialect check: when off,
/// a name like `TINYINT` is not matched as a built-in and falls through to the
/// user-defined-type path (so ANSI/PostgreSQL read it as an ordinary type name),
/// while a structural form like `ENUM('a','b')` surfaces as a clean parse error
/// (its value list is not a numeric type modifier). The modelling — new
/// [`DataType`](crate::ast::DataType) variants, spelling tags, and value-list/wrapper
/// shapes — lives with the AST; this struct only decides *which dialects recognize it*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeNameSyntax {
    /// Recognize extended scalar type names: the `TINYINT`/`MEDIUMINT`
    /// integer widths, bare `DOUBLE`, `DATETIME`, the `TINYTEXT`/`MEDIUMTEXT`/
    /// `LONGTEXT` character-LOB family, and the `TINYBLOB`/`BLOB`/`MEDIUMBLOB`/
    /// `LONGBLOB` binary-LOB family.
    pub extended_scalar_type_names: bool,
    /// Recognize the `ENUM(...)` value-list type in data-type position — MySQL's column
    /// type and DuckDB's `x::ENUM('a', 'b')` cast target (which rides the same
    /// [`DataType::Enum`](crate::ast::DataType) shape; DuckDB's *`CREATE TYPE ... AS ENUM`*
    /// statement uses a separate dedicated production, see
    /// [`CreateTypeDefinition`](crate::ast::CreateTypeDefinition)). Split from
    /// [`set_type`](Self::set_type) because DuckDB has `ENUM` but no `SET` type
    /// (`x::SET('a','b')` is an unknown-type error there, not a value-list type).
    pub enum_type: bool,
    /// Recognize the `SET(...)` value-list type in data-type position (MySQL only). Kept
    /// distinct from [`enum_type`](Self::enum_type): the two share one value-list shape but
    /// `SET` is MySQL-specific, so DuckDB enables only `ENUM`.
    pub set_type: bool,
    /// Recognize the `SIGNED`/`UNSIGNED`/`ZEROFILL` numeric modifiers (as a postfix
    /// on a numeric type) and the standalone `SIGNED`/`UNSIGNED` integer cast
    /// targets, e.g. `CAST(x AS UNSIGNED)` (MySQL).
    pub numeric_modifiers: bool,
    /// Recognize an optional display width `(M)` on a built-in integer type name —
    /// `INT(11)`, `TINYINT(1)`, `BIGINT(20)` — stored on the integer
    /// [`DataType`](crate::ast::DataType) variant's `display_width` field. Canonical
    /// to MySQL (deprecated in 8.0.17+ but ubiquitous in dumps); SQLite accepts it
    /// through affinity type-name absorption. When off, the trailing `(` on a
    /// built-in integer is not consumed and surfaces as a clean parse error, so
    /// ANSI/PostgreSQL reject `INT(11)` (verified against `pg_query`). Independent of
    /// [`extended_scalar_type_names`](Self::extended_scalar_type_names) (SQLite wants the width
    /// without the `TINYINT`/`MEDIUMINT` scalar names) and of
    /// [`numeric_modifiers`](Self::numeric_modifiers) (`(M)` is a prefix arg on the
    /// type name; `UNSIGNED`/`ZEROFILL` are a separate postfix).
    pub integer_display_width: bool,
    /// Recognize DuckDB's anonymous composite / nested type constructors in type
    /// position: `STRUCT(a INT, ...)` and the standard `ROW(...)` spelling of the same
    /// shape, the tagged `UNION(tag T, ...)`, and `MAP(K, V)`. A grammar-position gate
    /// keyed on the keyword immediately followed by `(`; when off, the leading word falls
    /// through to the user-defined-type path (a bare `struct`/`map` name still resolves),
    /// so ANSI/PostgreSQL reject the anonymous form — PostgreSQL has only *named*
    /// composite types and spells `ROW` as a value constructor, never a type (verified
    /// against a live server: `x::STRUCT(a int)` / `x::ROW(a int)` / `x::MAP(int,text)`
    /// each syntax-error). On for DuckDb/Lenient. The array-type suffixes (`T[]`/`T[n]`/
    /// `T ARRAY[n]`) are *not* gated here — PostgreSQL accepts them too — they ride the
    /// always-parsed array-suffix grammar.
    pub composite_types: bool,
    /// Accept BigQuery angle-bracket type forms `STRUCT<field TYPE, …>` and `ARRAY<T>`
    /// in type position (`CAST(x AS STRUCT<a INT64>)`, column definitions). On for
    /// BigQuery/Lenient. Distinct from [`composite_types`](Self::composite_types)
    /// (DuckDB paren form `STRUCT(a INT)`) and from expression-position
    /// [`struct_constructor`](crate::dialect::ExpressionSyntax::struct_constructor).
    pub angle_bracket_types: bool,
    /// Require an explicit length parameter on `VARCHAR` and `VARBINARY`
    /// (`VARCHAR(255)`, `VARBINARY(16)`). On for MySQL, whose `VARCHAR`/`VARBINARY` are a
    /// syntax error without a length (`CREATE TABLE t (a VARCHAR)` is an `ER_PARSE_ERROR` on
    /// mysql:8, while the fixed-width `CHAR`/`BINARY` default to length 1 and stay valid).
    /// Off for ANSI/PostgreSQL/SQLite/DuckDB/Lenient, where a length-less `VARCHAR` is
    /// accepted; when off the missing size is simply left `None`.
    pub varchar_requires_length: bool,
    /// Accept time-zone-aware temporal type names — `TIMESTAMPTZ` / `TIMESTAMP WITH TIME
    /// ZONE`, `TIMETZ` / `TIME WITH TIME ZONE`. On for ANSI/PostgreSQL/SQLite/DuckDB/Lenient.
    /// MySQL has no zoned temporal type (its `TIMESTAMP` stores UTC but the *type* carries no
    /// zone qualifier), so `TIMESTAMPTZ` and the `WITH TIME ZONE` spellings are an
    /// `ER_PARSE_ERROR` on mysql:8; it is off there and a parsed temporal type carrying a
    /// zone qualifier is rejected. The zone-less `TIMESTAMP`/`TIME`/`DATETIME` forms are
    /// unaffected.
    pub zoned_temporal_types: bool,
    /// Accept an EMPTY type-parameter parenthesis list on the `DECIMAL`/`DEC`/`NUMERIC`
    /// type names — `DECIMAL()`, `DEC()`, `NUMERIC()` — meaning the default precision/scale.
    /// DuckDB normalizes `DECIMAL()` to `DECIMAL(18,3)`, byte-identical to a bare `DECIMAL`
    /// (probed on 1.5.4: `typeof(x::DECIMAL())` == `typeof(x::DECIMAL)` == `DECIMAL(18,3)`),
    /// so the empty form carries no information and folds onto the same `precision: None,
    /// scale: None` [`DataType::Decimal`](crate::ast::DataType) shape — the canonical render
    /// drops the parens (an ADR-0011 spelling trade; the verbatim `()` survives on the node
    /// span). On for DuckDb/Lenient. Off elsewhere, where the empty `(` on a `DECIMAL` needs
    /// a precision and the missing modifier surfaces as a clean parse error (verified against
    /// `pg_query`: PostgreSQL rejects `DECIMAL()`).
    ///
    /// Scoped to the DECIMAL family (the surface the core-tranche corpus exercises). DuckDB
    /// also admits empty parens on its generic/user-resolved type names and a handful of
    /// dedicated built-ins (`DOUBLE()`, `TEXT()`, `DATE()`, `JSON()`, `TIMESTAMPTZ()`,
    /// `UUID()`, `HUGEINT()`, …) via the same `opt_type_modifiers` grammar, while the other
    /// hard-coded keyword types keep rejecting it (`VARCHAR()`/`TIMESTAMP()`/`FLOAT()` need a
    /// value; `INT()`/`REAL()`/`BOOLEAN()` admit no parens at all) — an asymmetric,
    /// separately-testable extension deferred (probe matrix on `duckdb-empty-type-parens`).
    pub empty_type_parens: bool,
    /// Recognize MySQL's character-set annotation on a char-family type — the grammar's
    /// `opt_charset_with_opt_binary` production: `CHARACTER SET <name>`, the `CHARSET`
    /// synonym, the `ASCII`/`UNICODE`/`BYTE` shortcuts, and/or the `BINARY` binary-collation
    /// modifier, in either order (`CHAR CHARACTER SET x BINARY`, `CHAR BINARY ASCII`). Stored
    /// on the [`DataType::Character`](crate::ast::DataType) node's `charset` field because it
    /// is part of the *type* — it must immediately follow the type and its length, and is an
    /// `ER_PARSE_ERROR` on mysql:8 once a column attribute intervenes (`CHAR(5) NOT NULL
    /// CHARACTER SET x`), unlike the free-floating `COLLATE` column attribute. Admitted in
    /// both the cast target (`CAST(x AS CHAR(5) CHARACTER SET utf8mb4)`) and column-definition
    /// positions that funnel through the shared type grammar, on the non-national spellings
    /// only (`CHAR`/`CHARACTER`/`VARCHAR`; the `NCHAR`/`NATIONAL` forms fix their own charset
    /// and reject it). On for MySQL/Lenient. Off elsewhere — PostgreSQL rejects `CHARACTER
    /// SET` in its modern grammar (verified against `pg_query`: `CHAR(5) CHARACTER SET utf8`
    /// is a syntax error), so when off the annotation keyword is left unconsumed and surfaces
    /// as a clean parse error.
    pub character_set_annotation: bool,
    /// Accept a leading sign on a `numeric`/`decimal` precision/scale type modifier
    /// (`numeric(5, -2)`, `numeric(-3, 6)`). PostgreSQL parses the modifier arguments as a
    /// general expression list at raw-parse time, so a signed integer is accepted and only
    /// validated later. On for PostgreSQL/Lenient. Off elsewhere (ANSI/MySQL/SQLite/DuckDB
    /// require an unsigned modifier), where the leading `-` on a modifier surfaces as a clean
    /// parse error.
    pub signed_type_modifier: bool,
    /// Recognize ClickHouse's `Nullable(T)` parametric type combinator in type position —
    /// the inner type extended with a `NULL` value, carried on the
    /// [`DataType::Wrapped`](crate::ast::DataType) shape. A grammar-position gate keyed on
    /// the keyword immediately followed by `(` (the composite-type precedent), so a bare
    /// `Nullable` with no `(` stays an ordinary type/column name and, when off, the whole
    /// `Nullable(...)` head falls through to the user-defined-type path. The inner type is a
    /// full recursive type, so `Nullable(DECIMAL(10, 2))` / `Nullable(String)[]` parse;
    /// ClickHouse's `Nullable(Nullable(T))` / `Nullable(Array(T))` composability rejects are
    /// a bind-time `DB::Exception`, not a grammar error, so they parse-accept here.
    ///
    /// **On for the ClickHouse preset and Lenient.** ClickHouse has no differential oracle, so
    /// the no-oracle acceptance addition belongs to the ClickHouse preset and Lenient (the
    /// `composite_types` / `format_clause` precedent): off for every oracle-compared preset,
    /// which parse-reject `Nullable(...)` (its head resolves to a user-defined type name whose
    /// `(String)` modifier list then fails to parse).
    pub nullable_type: bool,
    /// Recognize ClickHouse's `LowCardinality(T)` parametric type combinator in type
    /// position — a dictionary-encoding wrapper transparent to query semantics, carried on
    /// the [`DataType::Wrapped`](crate::ast::DataType) shape (the same single-inner-type
    /// wrapper as `nullable_type`, its own flag per one-behaviour-one-flag). A
    /// grammar-position gate keyed on the keyword immediately followed by `(`, so a bare
    /// `LowCardinality` with no `(` stays an ordinary type/column name and, when off, the
    /// whole `LowCardinality(...)` head falls through to the user-defined-type path. The
    /// inner type is a full recursive type, so the canonical `LowCardinality(Nullable(String))`
    /// composition and `LowCardinality(DECIMAL(10, 2))` parse; ClickHouse constrains which
    /// inner `T` is valid at type resolution (a bind-time `DB::Exception`, not a grammar
    /// error), so any single inner type parse-accepts here.
    ///
    /// **On for the ClickHouse preset and Lenient.** ClickHouse has no differential oracle, so
    /// the no-oracle acceptance addition belongs to the ClickHouse preset and Lenient (the
    /// `composite_types` / `nullable_type` precedent): off for every oracle-compared preset,
    /// which parse-reject `LowCardinality(...)` (its head resolves to a user-defined type
    /// name whose `(String)` modifier list then fails to parse).
    pub low_cardinality_type: bool,
    /// Recognize ClickHouse's `FixedString(N)` type constructor in type position — a
    /// fixed-length byte string of exactly `N` bytes, carried on the
    /// [`DataType::FixedString`](crate::ast::DataType) shape. Unlike the `Nullable`/
    /// `LowCardinality` wrappers its argument is a scalar length, not an inner type, so it
    /// is its own variant, not a [`WrappedTypeKind`](crate::ast::WrappedTypeKind) arm; its
    /// own flag per one-behaviour-one-flag. A grammar-position gate keyed on the keyword
    /// immediately followed by `(` (the composite/wrapper precedent), so a bare
    /// `FixedString` with no `(` stays an ordinary type/column name and, when off, the whole
    /// `FixedString(...)` head falls through to the user-defined-type path. `N` is mandatory
    /// (a bare `FixedString` is an invalid ClickHouse spelling) and parsed as any `u32`
    /// literal; ClickHouse's positive-length requirement (`FixedString(0)` reject) is a
    /// bind-time `DB::Exception`, not a grammar error, so it parse-accepts here.
    ///
    /// **On for the ClickHouse preset and Lenient.** ClickHouse has no differential oracle, so
    /// the no-oracle acceptance addition belongs to the ClickHouse preset and Lenient (the
    /// `nullable_type` / `low_cardinality_type` precedent): off for every oracle-compared
    /// preset, which parse-reject `FixedString(...)` (its head resolves to a user-defined
    /// type name whose `(N)` modifier list then fails to parse).
    pub fixed_string_type: bool,
    /// Recognize ClickHouse's `DateTime64(P[, 'timezone'])` type constructor in type
    /// position — a sub-second timestamp carried on the
    /// [`DataType::DateTime64`](crate::ast::DataType) shape. Its own flag per
    /// one-behaviour-one-flag; like [`fixed_string_type`](Self::fixed_string_type) its
    /// leading argument is a mandatory scalar (the precision `P`), not an inner type, so it
    /// is a dedicated variant, not a [`WrappedTypeKind`](crate::ast::WrappedTypeKind) arm.
    /// The optional second argument is a single-quoted time-zone string literal, not the
    /// ANSI `WITH TIME ZONE` flag. A grammar-position gate keyed on the keyword immediately
    /// followed by `(` (the composite/wrapper precedent), so a bare `DateTime64` with no `(`
    /// stays an ordinary type/column name and, when off, the whole `DateTime64(...)` head
    /// falls through to the user-defined-type path. `P` is parsed as any `u32` literal;
    /// ClickHouse's documented `0..=9` range is a bind-time reject, not a grammar error, so
    /// it parse-accepts here.
    ///
    /// **On for the ClickHouse preset and Lenient.** ClickHouse has no differential oracle, so
    /// the no-oracle acceptance addition belongs to the ClickHouse preset and Lenient (the
    /// `fixed_string_type` precedent): off for every oracle-compared preset. Off-gate the
    /// boundary is asymmetric — `DateTime64(3)` still parse-accepts as a user-defined type
    /// name with a `(3)` numeric-modifier list, but `DateTime64(3, 'UTC')` parse-*rejects*,
    /// because the string second argument does not fit the `u32`-only modifier grammar.
    pub datetime64_type: bool,
    /// Recognize ClickHouse's `Nested(name1 Type1, name2 Type2, ...)` named-field composite
    /// type in type position — a repeated group carried on the
    /// [`DataType::Nested`](crate::ast::DataType) shape (the named-field
    /// [`StructTypeField`](crate::ast::StructTypeField) list of `composite_types`, but a
    /// distinct variant and its own flag per one-behaviour-one-flag: `Nested` round-trips as
    /// `Nested`, never `STRUCT`, and its semantics are a repeated structure, not a product).
    /// A grammar-position gate keyed on the keyword immediately followed by `(` (the
    /// composite/wrapper precedent), so a bare `Nested` with no `(` stays an ordinary
    /// type/column name and, when off, the whole `Nested(...)` head falls through to the
    /// user-defined-type path. A field type is a full recursive type, so `Nested(x
    /// Nested(...))` parses; ClickHouse's nesting-level limit is a `flatten_nested` setting /
    /// bind concern, not a grammar error, so it parse-accepts here.
    ///
    /// **On for the ClickHouse preset and Lenient.** ClickHouse has no differential oracle, so
    /// the no-oracle acceptance addition belongs to the ClickHouse preset and Lenient (the
    /// `datetime64_type` precedent): off for every oracle-compared preset, which parse-reject
    /// `Nested(a UInt8)` — its head resolves to a user-defined type name whose modifier list
    /// is `u32`-only, so the two-word `a UInt8` field has no grammar to fit (the wrapper
    /// off-gate reject, not the asymmetric `DateTime64(3)` accept).
    pub nested_type: bool,
    /// Recognize ClickHouse's fixed-bit-width integer type names — the signed
    /// `Int8`/`Int16`/`Int32`/`Int64`/`Int128`/`Int256` family and their unsigned
    /// `UInt8`…`UInt256` siblings — carried on the
    /// [`DataType::FixedWidthInt`](crate::ast::DataType) shape. One flag for the whole
    /// bit-width family: the names always travel together in a dialect, exactly as MySQL's
    /// `TINYINT`/`MEDIUMINT`/… ride one [`extended_scalar_type_names`](Self::extended_scalar_type_names)
    /// gate, so this is one behaviour, not one flag per width. Unlike the
    /// `Nullable`/`FixedString` constructors these names take no arguments, so recognition is a
    /// bare-name gate (the `TINYINT`/`extended_scalar` precedent, not the keyword-then-`(`
    /// lookahead): when off, a bare `Int256` simply falls through to the user-defined-type path
    /// (its trivial off-gate boundary, like a bare `Nullable`).
    ///
    /// **On for the ClickHouse preset and Lenient.** ClickHouse has no differential oracle, so the
    /// no-oracle acceptance addition belongs to the ClickHouse preset and Lenient (the `nullable_type`
    /// / `fixed_string_type` / `datetime64_type` precedent): off for every oracle-compared
    /// preset, which read `Int256` as an ordinary user-defined type name.
    pub bit_width_integer_names: bool,
    /// Widen the type-name grammar to SQLite's liberal affinity form: a column/cast type is
    /// any run of one-or-more space-separated words (`UNSIGNED BIG INT`, `LONG INTEGER`, the
    /// misspelled `INTEGEB PRIMARI KEY`) with an optional *two*-argument parenthesized
    /// modifier (`VARCHAR(123,456)`, `FLOATING POINT(5,10)`), carried on the
    /// [`DataType::Liberal`](crate::ast::DataType) shape. SQLite has no closed type
    /// vocabulary — every declared type is affinity text — so its grammar's `typename`
    /// accepts an arbitrary `ids ...` token run terminated by a column-constraint keyword, a
    /// comma, or a close paren (engine-probed on rusqlite/sqlite3 3.53.2 & 3.43.2).
    ///
    /// A strict FALLBACK: the typed variants and the single-word user-defined path win
    /// wherever they can faithfully represent the input, so a bare `INT`, `DOUBLE PRECISION`,
    /// `VARCHAR(255)`, `NATIONAL CHARACTER(15)`, or single-word affinity `BANANA` keep their
    /// existing shapes with the flag on; only a trailing type-word or a two-argument paren
    /// list that no typed / user-defined parse can hold falls to `DataType::Liberal`. The
    /// word run terminates at a column-constraint keyword (`PRIMARY`/`NOT`/`NULL`/`UNIQUE`/
    /// `CHECK`/`DEFAULT`/`COLLATE`/`REFERENCES`/`CONSTRAINT`/`AS`/`GENERATED`), so
    /// `GENERATED ALWAYS AS` generated columns are unaffected.
    ///
    /// **On for SQLite and Lenient.** Off elsewhere, where a multi-word type name or a
    /// two-argument built-in modifier surfaces as a clean parse error (the standard/
    /// PostgreSQL/MySQL/DuckDB have a closed type vocabulary; `pg_query` rejects
    /// `LONG INTEGER` and `VARCHAR(123,456)`).
    pub liberal_type_names: bool,
    /// Admit a string-literal argument in a user-defined type name's modifier list —
    /// DuckDB's `GEOMETRY('OGC:CRS84')` coordinate-system annotation, and more generally
    /// any `type_name('constant', ...)` where DuckDB's grammar accepts constants (string or
    /// numeric) as type modifiers (engine-measured on DuckDB 1.5.4: `MYTYPE('abc')` reaches
    /// the binder — a parse-accept — while a non-constant like `(['abc'])` stays a parser
    /// error). The modifiers ride the [`DataType::UserDefined`](crate::ast::DataType)
    /// shape's `modifiers` list as [`Literal`](crate::ast::Literal)s.
    ///
    /// When off, only unsigned-integer modifiers parse (`FOO(3)`), so a string modifier
    /// surfaces as a clean parse error — the standard/PostgreSQL/MySQL user-type grammar
    /// admits no string modifier there. **On for DuckDB only.** Numeric modifiers parse
    /// under every dialect regardless of this flag; it gates *only* the string form.
    pub string_type_modifiers: bool,
}

/// Dialect-owned expression *postfix and constructor* syntax accepted by the parser.
///
/// The postfix operators that navigate a value and the constructor / typed-literal forms
/// that build one, each gated by dialect data: a flag decides whether the parser *admits*
/// the form, and when off the leading punctuation/keyword surfaces as a clean parse error.
/// The infix/prefix operator spellings and the function-call-tail forms
/// split out into [`OperatorSyntax`] and [`CallSyntax`] once this struct crossed its
/// documented 16-field line; this half keeps only the shapes that build or navigate a
/// single expression value. Most gates are over lexemes that always tokenize (a lone `:`
/// and `::` are structural punctuation); a flag whose form needs a dialect-gated *lexeme*
/// says "also gates the tokenizer" in its first paragraph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpressionSyntax {
    /// Accept the `expr::type` typecast operator.
    pub typecast_operator: bool,
    /// Accept `base[index]` element access and `base[lower:upper]` slicing.
    pub subscript: bool,
    /// Accept DuckDB's three-bound slice `base[lower:upper:step]` on top of the two-bound
    /// slice [`subscript`](Self::subscript) admits. A pure parser gate (the `:` separators
    /// and the `-` placeholder always tokenize): reachable only once
    /// [`subscript`](Self::subscript) has opened the brackets (the dependency is
    /// [`FeatureDependencyViolation::SliceStepWithoutSubscript`]), it admits the second `:` and,
    /// as the middle bound, DuckDB's bare `-`
    /// open-upper placeholder (`base[lower:-:step]`) — an empty middle `base[lower::step]`
    /// stays a parse error. On for DuckDB only; PostgreSQL slices are two-bound, so it keeps
    /// this off despite `subscript`, and a `base[a:b:c]` there is a clean parse error at the
    /// second `:`.
    pub slice_step: bool,
    /// Accept `expr COLLATE collation`.
    pub collate: bool,
    /// Accept `expr AT TIME ZONE zone`.
    pub at_time_zone: bool,
    /// Accept semi-structured value paths written as `base:key[0].field`.
    ///
    /// A postfix grammar gate over `:` followed by an identifier-like key, then optional
    /// `.` and `[...]` path suffixes. It contends with
    /// [`ParameterSyntax::named_colon`] on the same `:`+identifier trigger, so enabling
    /// both is a [`LexicalConflict::ColonParameterVersusSliceBound`] conflict: the scanner
    /// would otherwise turn `:key` into a parameter before the postfix parser could read
    /// the path. It also contends one layer up, at the *grammar* head, with
    /// [`SelectSyntax::prefix_colon_alias`] — whose alias-before-value form reads the same
    /// leading `<ident> :` — so enabling both is a
    /// [`GrammarConflict::PrefixColonAliasVersusSemiStructuredAccess`] conflict (no shipped
    /// preset pairs them).
    pub semi_structured_access: bool,
    /// Accept the `ARRAY[...]` / `ARRAY(<query>)` array constructors.
    pub array_constructor: bool,
    /// Accept PostgreSQL multidimensional array literals — a bare-bracket sub-row
    /// `[...]` as an element inside an [`array_constructor`](Self::array_constructor),
    /// as in `ARRAY[[1,2],[3,4]]` and deeper nestings.
    ///
    /// A pure grammar gate on the array-constructor element position (its dependency on
    /// [`array_constructor`](Self::array_constructor) is
    /// [`FeatureDependencyViolation::MultidimArrayLiteralsWithoutArrayConstructor`]), independent of
    /// [`collection_literals`](Self::collection_literals): the bare-bracket row is only
    /// a value in an array context (a top-level `[1,2]` is still rejected under
    /// PostgreSQL), and PostgreSQL enforces that each bracket level is uniform — every
    /// element is a sub-row or every element is a scalar, never a mix
    /// (`ARRAY[[1,2],3]`, `ARRAY[1,[2,3]]`, and `ARRAY[[1,2],ARRAY[3,4]]` are parse
    /// errors, matching PG's `expr_list` / `array_expr_list` split). Ragged nestings
    /// (`ARRAY[[1,2],[3]]`) parse-accept — PostgreSQL rejects them at bind time, not in
    /// the grammar. A sub-row is represented as an ordinary bracket-spelled
    /// [`ArrayExpr::Elements`](crate::ast::ArrayExpr), so it renders and shapes exactly
    /// like a DuckDB list level. DuckDB reaches the same surface through
    /// [`collection_literals`](Self::collection_literals) (a top-level list *is* a value
    /// there, and levels may mix), so it keeps this off and overrides the POSTGRES
    /// spread.
    pub multidim_array_literals: bool,
    /// Accept the DuckDB collection literals: the bare-bracket list `[a, b, …]`, the
    /// struct `{'k': v, …}`, and the map `MAP {k: v, …}` / `MAP(<keys>, <values>)`.
    ///
    /// A pure grammar/lexical gate on the primary-expression `[`, `{`, and `MAP`
    /// leads. The bracket list contends for the same `[` trigger as an
    /// [`identifier_quotes`](FeatureSet::identifier_quotes) style opening with `[`
    /// (T-SQL/SQLite/Lenient bracket identifiers), so enabling both is the
    /// [`LexicalConflict::BracketIdentifierVersusArraySyntax`] conflict — a dialect
    /// picks bracket identifiers *or* `[` collection/array syntax. The `key: value`
    /// separator likewise contends with [`ParameterSyntax::named_colon`] on the
    /// `:`+identifier trigger
    /// ([`LexicalConflict::ColonParameterVersusSliceBound`], shared with the slice
    /// bound). When off, `[`/`{` in primary position surface as a clean parse error
    /// (`MAP` falls back to an ordinary name).
    pub collection_literals: bool,
    /// Accept explicit `ROW(...)` and implicit `(a, b, …)` row constructors.
    pub row_constructor: bool,
    /// Accept BigQuery's `STRUCT(...)` value constructor — the typeless `STRUCT(1, 2)`
    /// and named `STRUCT(x AS a, y AS b)` forms and the typed
    /// `STRUCT<a INT64, b STRING>(1, 'x')` form — on the canonical
    /// [`StructConstructorExpr`](crate::ast::StructConstructorExpr) shape.
    ///
    /// A pure parser lookahead gate, the sibling of
    /// [`row_constructor`](Self::row_constructor)/[`array_constructor`](Self::array_constructor):
    /// the (contextual, non-reserved) `STRUCT` word opens the constructor only when
    /// immediately followed by `(` (typeless) or `<` (typed), mirroring the `ROW(`/`ARRAY[`
    /// disambiguation — the tokenizer is untouched, and the `<` opener is committed on a
    /// single-token lookahead (no rewind), sound because in a preset that admits this form
    /// `STRUCT` is not a bare column so `struct < x` is not a competing comparison. When
    /// off, `STRUCT(...)` is left to the ordinary call path and stays an
    /// [`Expr::Function`](crate::ast::Expr::Function) catalog-function call — the
    /// non-interference boundary every non-BigQuery preset (PostgreSQL included) keeps.
    ///
    /// **On for BigQuery and Lenient.** BigQuery documents the form and has no differential
    /// oracle here (self-consistency + gate-off rejection are the tests); Lenient unions it
    /// in. Off for every other preset: DuckDB builds structs with the `{...}` literal /
    /// `struct_pack()` / `row()` rather than a `STRUCT(...)` keyword form, and the
    /// Spark-family `struct(...)` builtin is not verifiable without an engine, so those
    /// presets keep `struct(...)` an ordinary call rather than risk reshaping it. The typed
    /// field list here is parsed inline; a bare `STRUCT<...>` in *type* position
    /// (`CAST(x AS STRUCT<...>)`) is a separate type-name surface not gated by this flag.
    pub struct_constructor: bool,
    /// Accept `(expr).field` composite field selection.
    pub field_selection: bool,
    /// Accept the `.*` composite/whole-row star selector in a *value* position: the
    /// composite expansion `(expr).*` off a parenthesized primary, and a whole-row
    /// `tbl.*` used as a value (inside a `ROW(...)` field, a function argument, a
    /// comparison, or a `tbl.*::type` cast) rather than as a select-list projection
    /// target. PostgreSQL admits this `.*` indirection wherever an `a_expr` is, at the
    /// same tight precedence as [`field_selection`](Self::field_selection) (so
    /// `(a).* + 1` groups as `((a).*) + 1`); engine-probed on pg_query PG-17. Gated
    /// apart from `field_selection` because DuckDB parse-accepts `(struct).field` but
    /// parse-rejects every `.*` value expansion (engine-probed on DuckDB 1.5.4), so it
    /// is off there and on for PostgreSQL/Lenient. When off, a `.` followed by `*` is
    /// left unconsumed and surfaces as the usual downstream parse error. This governs
    /// only the *value*-position star; a plain select-list/`RETURNING` `tbl.*` remains
    /// a [`SelectItem::QualifiedWildcard`](crate::ast::SelectItem::QualifiedWildcard)
    /// under every preset.
    pub field_wildcard: bool,
    /// Accept the prefix-typed string literal — a type name whose reading becomes a
    /// literal when a string constant follows the type prefix: the standard temporal
    /// forms `DATE '…'` / `TIME '…'` / `TIMESTAMP '…'` and the PostgreSQL generalized
    /// `<type> '…'` (`float8 '1.5'`, folded onto a
    /// [`CastSyntax::PrefixTyped`](crate::ast::CastSyntax) cast). One flag covers both,
    /// because the two travel together — a dialect with the temporal forms has the
    /// generalized one. On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite has neither
    /// (`date '…'` juxtaposes a name and a string, a clean parse error there), so it is
    /// off; the type keyword then falls back to its ordinary column/function reading and
    /// the trailing string surfaces as the usual parse error.
    ///
    /// The prefix-typed `INTERVAL '…' <fields>` literal is split onto its own
    /// [`typed_interval_literal`](Self::typed_interval_literal) refinement, because MySQL
    /// has the other temporal literals but no first-class interval literal.
    pub typed_string_literals: bool,
    /// Arm the prefix-typed `INTERVAL '<amount>' <fields>` literal — the ANSI/PostgreSQL
    /// interval literal that folds onto
    /// [`LiteralKind::Interval`](crate::ast::LiteralKind::Interval), including the ANSI
    /// `HOUR TO SECOND` composite and the `SECOND(p)` unit precision. A refinement of
    /// [`typed_string_literals`](Self::typed_string_literals) (the interval literal is only
    /// reached when that is on), split out because MySQL admits the other prefix-typed
    /// temporal literals (`DATE`/`TIME`/`TIMESTAMP`) yet has no interval literal at all:
    /// every typed `INTERVAL '…'` form — standalone *or* in a `+`/`-` operand — is
    /// `ER_PARSE_ERROR` (1064) on mysql:8.4.10 (engine-measured), the only valid MySQL
    /// interval being the operator-position `INTERVAL <expr> <unit>` modelled by
    /// [`mysql_interval_operator`](Self::mysql_interval_operator).
    ///
    /// On for ANSI/PostgreSQL/DuckDB/Lenient. Off for MySQL (the operator reader owns its
    /// valid unit-bearing forms; every form that reaches this literal path there — the ANSI
    /// `TO`/precision spellings the operator reader declines, and the unit-less
    /// `INTERVAL '1'` — is one MySQL rejects, so the literal path declines and the
    /// `INTERVAL` keyword falls back to its ordinary reading, a parse error on the trailing
    /// string). Off for SQLite, where [`typed_string_literals`](Self::typed_string_literals)
    /// is already off and no prefix-typed literal is reached.
    pub typed_interval_literal: bool,
    /// Accept DuckDB's relaxed `INTERVAL` literal spellings on top of the standard
    /// quoted `INTERVAL '1' DAY` form: an unquoted integer amount (`INTERVAL 3 DAY`),
    /// a parenthesized-expression amount (`INTERVAL (days) DAY`), and plural unit
    /// spellings (`INTERVAL 3 DAYS`, `INTERVAL '1' hours`). All three fold onto the one
    /// [`LiteralKind::Interval`](crate::ast::LiteralKind::Interval) shape: the
    /// amount round-trips from the literal's span exactly as the quoted string
    /// does, so only the unit qualifier lands on the tag, and a plural unit folds
    /// onto its singular [`IntervalFields`](crate::ast::IntervalFields) — the plural `s`
    /// round-trips from the span (the documented spelling trade). The unquoted/parenthesized
    /// amount forms require a trailing unit (a bare `INTERVAL 3` is a DuckDB *binding*
    /// error, not a syntax one). On for DuckDB and Lenient; off elsewhere, where the
    /// non-standard spellings surface as the usual parse error.
    pub relaxed_interval_syntax: bool,
    /// Accept the MySQL operator-position interval quantity `INTERVAL <expr> <unit>` —
    /// the `INTERVAL 3 DAY` operand of MySQL date arithmetic — as an
    /// [`Expr::Interval`](crate::ast::Expr::Interval) node.
    ///
    /// A behaviour distinct from the ANSI/PostgreSQL/DuckDB typed-string interval *literal*
    /// ([`typed_string_literals`](Self::typed_string_literals) /
    /// [`relaxed_interval_syntax`](Self::relaxed_interval_syntax), both folding onto
    /// [`LiteralKind::Interval`](crate::ast::LiteralKind::Interval)): MySQL's `INTERVAL` is
    /// not a first-class value but the second argument of the `Item_date_add_interval`
    /// production, so it carries an arbitrary amount *expression* (integer, decimal, string,
    /// `?`, `@var`, `n + 1`, `(expr)`, negative) and a **mandatory** unit keyword drawn from
    /// MySQL's underscore vocabulary — the simple units `MICROSECOND`…`YEAR` (plus `WEEK`,
    /// `QUARTER`) and the composites `SECOND_MICROSECOND`, `MINUTE_SECOND`, `DAY_HOUR`,
    /// `YEAR_MONTH`, … (engine-measured on mysql:8.4.10). It admits **no** ANSI `TO`
    /// composite and **no** `(p)` unit precision (`INTERVAL '1' HOUR TO SECOND` and
    /// `INTERVAL '1' SECOND(3)` are `ER_PARSE_ERROR` there); a `TO`/`(` after the unit makes
    /// the operator reader decline so the typed-string interval literal path
    /// ([`typed_interval_literal`](Self::typed_interval_literal)) owns those spellings under
    /// Lenient/PostgreSQL. Under MySQL that literal path is off, so the declined ANSI
    /// spellings reject (matching the engine).
    ///
    /// **On for MySQL and Lenient.** When on, an `INTERVAL` in expression-prefix position is
    /// read as this node before the typed-string literal path (MySQL has no first-class
    /// interval literal); a form that is not a valid MySQL operator interval — unit-less, an
    /// ANSI `TO`/precision spelling, a bare `INTERVAL(a, b)` index function — rewinds and
    /// falls through, so under Lenient the DuckDB relaxed amounts, plural units, unit-less
    /// PostgreSQL literals, and the ANSI `TO`/precision interval literals still parse (under
    /// MySQL they reject, [`typed_interval_literal`](Self::typed_interval_literal) being off).
    /// The node is modelled as a primary (highest binding power), so MySQL's *position*
    /// restriction is deliberately not enforced: a standalone `SELECT INTERVAL 3 DAY`, a
    /// leading `INTERVAL 3 DAY - x` (only `+` leads on mysql:8), and `INTERVAL 3 DAY IS NULL`
    /// over-accept rather than over-reject the valid operand/`DATE_ADD`/frame positions a
    /// general grammar cannot distinguish. Off elsewhere, where `INTERVAL` keeps its
    /// literal-or-column reading.
    pub mysql_interval_operator: bool,
    /// Accept DuckDB's `#n` positional column reference — a select-list column named by
    /// its 1-based output position ([`Expr::PositionalColumn`](crate::ast::Expr::PositionalColumn)),
    /// used mainly in `ORDER BY #1` / `GROUP BY #2` but valid wherever a value expression
    /// is. Also gates the tokenizer: the `#<digits>` lexeme is scanned only under a
    /// dialect that sets this (DuckDB), so elsewhere `#` stays a stray byte, a MySQL line
    /// comment, or PostgreSQL's XOR operator per that dialect's data. Because it claims
    /// the `#` trigger, it is mutually exclusive with the two other `#` claimants — the
    /// [`hash_bitwise_xor`](FeatureSet::hash_bitwise_xor) XOR operator
    /// ([`LexicalConflict::HashXorOperatorVersusPositionalColumn`]) and a
    /// [`CommentSyntax::line_comment_hash`] line comment
    /// ([`LexicalConflict::HashCommentVersusPositionalColumn`]); a `#`-led identifier byte
    /// class instead resolves by scan order (the identifier scan precedes this arm), like
    /// the XOR case. On for DuckDB only. Lenient, which already commits `#` to a MySQL
    /// line comment, cannot also enable it. When off, `#1` surfaces per the dialect's
    /// other `#` reading (a clean parse error in ANSI/SQLite).
    pub positional_column: bool,
    /// Accept DuckDB's python-style keyword lambda `lambda x, y: body` — a prefix
    /// production, distinct from the [`OperatorSyntax::lambda_expressions`] single-arrow
    /// form and folded onto the same [`Expr::Lambda`](crate::ast::Expr::Lambda) node with
    /// a [`LambdaParamSpelling::Keyword`](crate::ast::LambdaParamSpelling) spelling tag.
    /// DuckDB 1.3.0 introduced it and prefers it over the deprecated arrow; the two
    /// spellings are separate flags because DuckDB's roadmap keeps the keyword while
    /// dropping the arrow. When on, a `lambda` word in expression-prefix position opens
    /// the production unconditionally rather than reading as an ordinary column — matching
    /// DuckDB, which reserves `lambda` — so a bare `lambda`, or one not followed by
    /// `<params>:`, is a parse error. On for DuckDB and Lenient.
    pub lambda_keyword: bool,
}

/// Dialect-owned infix/prefix *operator* syntax accepted by the parser.
///
/// The operator-acceptance and operator-spelling family, split out of [`ExpressionSyntax`]
/// when it crossed its 16-field line. Each flag decides whether the parser *admits* the
/// operator, while the binding powers that order them live in [`BindingPowerTable`]; a
/// spelling that folds onto an existing operator carries a spelling tag so it round-trips.
/// A flag whose form needs a dialect-gated *lexeme* says "also gates the tokenizer" in its
/// first paragraph — a flag crossing the lexer/parser boundary must declare it there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperatorSyntax {
    /// Accept the PostgreSQL explicit-operator infix form `a OPERATOR(schema.op) b`.
    pub operator_construct: bool,
    /// Accept PostgreSQL's containment operators — infix `@>` (contains) and `<@`
    /// (contained by). Also gates the tokenizer: the `@>`/`<@` lexemes are recognised
    /// only under a dialect that sets this (PostgreSQL), so elsewhere `@>` stays a stray
    /// `@` then `>`, and `<@` a `<` then a stray `@`. The `@>` munch requires the
    /// following `>`, so a bare `@` is always a stray byte here — the prefix `@`
    /// absolute-value operator is a scoped follow-up, since its bare-`@` lexeme contends
    /// with the T-SQL/MySQL `@name` sigils and needs a tracked conflict. The `<@` munch,
    /// by contrast, shadows an abutting `@name` sigil (`a<@x` meaning `a < @x`) whenever
    /// this is on together with a single-`@` form — the tracked
    /// [`LexicalConflict::ContainmentOperatorVersusAtName`]. The operators bind at
    /// PostgreSQL's "any other operator" precedence ([`BindingPowerTable::any_operator`]).
    ///
    /// [`BindingPowerTable::any_operator`]: crate::precedence::BindingPowerTable::any_operator
    pub containment_operators: bool,
    /// Accept PostgreSQL's JSON access operators — infix `->` (field/element as
    /// `json`) and `->>` (as `text`). Also gates the tokenizer: the `->`/`->>`
    /// lexemes are munched only under a dialect that sets this, so elsewhere `a->b`
    /// stays a `-` then a `>`. MySQL spells the same JSON accessors and can enable
    /// this independently of [`containment_operators`](Self::containment_operators),
    /// which is why the two are separate flags. Same "any other operator" precedence.
    pub json_arrow_operators: bool,
    /// Accept PostgreSQL's `jsonb` existence / path / search operators as one family: `?`
    /// (key exists), `?|` (any key exists), `?&` (all keys exist), `@?` (`jsonpath` returns
    /// any item), `@@` (`jsonpath` predicate match, also the `tsvector @@ tsquery` full-text
    /// match), `#>` (extract at path), `#>>` (extract at path as `text`), and `#-` (delete at
    /// path). Also gates the tokenizer: these lexemes are munched only under a dialect that
    /// sets this. One flag for the whole family (the family-level granularity of
    /// [`json_arrow_operators`](Self::json_arrow_operators) / [`containment_operators`](Self::containment_operators)):
    /// PostgreSQL enables the set as a unit and every other dialect leaves it off. All eight
    /// bind at the "any other operator" precedence ([`BindingPowerTable::any_operator`]),
    /// left-associative — engine-measured on pg_query (tighter than comparison, looser than
    /// additive). The operators fold onto the dedicated
    /// [`BinaryOperator`] `Json…` keys.
    ///
    /// Three lead bytes are shared triggers the tokenizer partitions by follow byte:
    /// - `?` is otherwise the anonymous placeholder ([`ParameterSyntax::anonymous_question`]);
    ///   the two never co-enable in a shipped preset (PostgreSQL has no `?` parameter), and a
    ///   feature set enabling both is the tracked
    ///   [`LexicalConflict::JsonbKeyExistsVersusAnonymousParameter`].
    /// - `@@` is otherwise the MySQL system-variable sigil
    ///   ([`SessionVariableSyntax::system_variables`]); the tracked
    ///   [`LexicalConflict::JsonbSearchOperatorVersusSystemVariable`]. `@?` is disjoint from
    ///   every other `@` claimant by its second byte, so it adds no conflict.
    /// - `#>`/`#>>`/`#-` ride the `#` byte, which reaches the operator scanner only under
    ///   PostgreSQL's `#` bitwise-XOR ([`hash_bitwise_xor`](FeatureSet::hash_bitwise_xor), on together with this in the
    ///   PostgreSQL preset); they are munched ahead of the bare `#` and stay disjoint from
    ///   DuckDB's `#n` positional column (`#`+digit) by follow byte.
    ///
    /// The sibling regex/geometric/network operator surface builds on the same `?`/`@`/`#`
    /// lexeme foundation under its own flag(s): bare prefix `@` (absolute value) stays a
    /// stray byte here (the `@?`/`@@` munch requires a follow byte), left for that work.
    ///
    /// [`BindingPowerTable::any_operator`]: crate::precedence::BindingPowerTable::any_operator
    pub jsonb_operators: bool,
    /// Accept SQLite's `==` equality spelling as a synonym for `=`. Also gates the
    /// tokenizer: the doubled-`=` lexeme is munched to the equality operator only
    /// under a dialect that sets this, so elsewhere `a == b` stays `a` `=` `=` `b`
    /// and surfaces as a clean parse error. The two spellings fold onto the one
    /// canonical [`BinaryOperator::Eq`] operator; the
    /// [`EqualsSpelling`](crate::ast::EqualsSpelling) tag records
    /// which the source used so `==`/`=` round-trip exactly, the same pattern as
    /// `MOD`/`RLIKE`/`REGEXP`.
    pub double_equals: bool,
    /// Accept DuckDB's `//` integer-division spelling. Also gates the tokenizer: the
    /// doubled-`/` lexeme is munched to the integer-division operator only under a dialect
    /// that sets this, so elsewhere `a // b` stays `a` `/` `/` `b` and surfaces as a clean
    /// parse error. No shipped preset lexes `//` as a line comment, so the doubled munch
    /// never shadows a comment mode. The symbol folds onto the one canonical
    /// [`BinaryOperator::IntegerDivide`] operator; the [`IntegerDivideSpelling`]
    /// tag records the `//` spelling so it round-trips (this is load-bearing for validity,
    /// not only fidelity: DuckDB has no `DIV` keyword and MySQL no `//` operator, so the
    /// spelling cannot be normalized away). Distinct from MySQL's `DIV`, which rides
    /// [`KeywordOperators::MySql`].
    pub integer_divide_slash: bool,
    /// Accept DuckDB's `^@` "starts with" infix operator (`'hello' ^@ 'he'`). Also
    /// gates the tokenizer: `^@` is munched only when this is on; elsewhere `^` then
    /// `@` stay separate (and `@` is often a stray byte).
    pub starts_with_operator: bool,
    /// Accept SQLite's `IS` / `IS NOT` as a *general* null-safe equality over
    /// arbitrary operands (`1 IS 1`, `1 IS NOT 2`), not just `IS NULL` /
    /// `IS [NOT] DISTINCT FROM`. When on, an `IS [NOT] <expr>` whose right operand is
    /// neither `NULL` nor `DISTINCT FROM …` folds onto the existing null-safe
    /// [`IsNotDistinctFrom`](crate::ast::BinaryOperator::IsNotDistinctFrom) /
    /// [`IsDistinctFrom`](crate::ast::BinaryOperator::IsDistinctFrom) operators —
    /// SQLite's `IS` is exactly `IS NOT DISTINCT FROM`. When off (ANSI/PostgreSQL/
    /// MySQL), `IS` requires `NULL` or `DISTINCT FROM`, so `1 IS 1` is a clean parse
    /// error.
    pub is_general_equality: bool,
    /// Accept the SQL:2016 truth-value tests `<expr> IS [NOT] {TRUE | FALSE | UNKNOWN}`
    /// (F571), parsed to the postfix [`Expr::IsTruth`](crate::ast::Expr::IsTruth) predicate.
    /// On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient, all of which accept the three-valued
    /// `UNKNOWN` (engine-measured on pg_query, MySQL 8, DuckDB). Off for SQLite, whose `IS`
    /// is a general null-safe equality ([`is_general_equality`](Self::is_general_equality)):
    /// there `IS TRUE`/`IS FALSE` fold onto the boolean literal and `IS UNKNOWN` reads as
    /// equality against an identifier `unknown`, so SQLite has no truth-value predicate and
    /// this stays off (turning it on would over-accept `IS UNKNOWN`, which SQLite rejects
    /// unless `unknown` is a bound column). Checked ahead of the general-equality reading so
    /// that a dialect enabling both keeps the standard truth predicate.
    pub truth_value_tests: bool,
    /// Accept MySQL's `<=>` null-safe equality operator (`a <=> b` ≡
    /// `a IS NOT DISTINCT FROM b`). Also gates the tokenizer: the `<=>` lexeme is munched
    /// (ahead of `<=`) only under a dialect that sets this, so elsewhere `a <=> b` stays
    /// `a` `<=` `>` `b` and surfaces as a clean parse error. It folds onto the canonical
    /// [`BinaryOperator::IsNotDistinctFrom`] operator; the
    /// [`IsNotDistinctFromSpelling`](crate::ast::IsNotDistinctFromSpelling) tag records the
    /// `<=>` spelling so it round-trips (MySQL rejects the keyword `IS NOT DISTINCT FROM`,
    /// so the spelling cannot be normalized away). Binds at comparison precedence, riding
    /// the shared comparison row like the other comparison operators.
    pub null_safe_equals: bool,
    /// Read an infix `->` whose left operand is a lambda-parameter list — a bare
    /// unqualified name, a parenthesized name list `(x, y)`, or the equivalent
    /// `ROW(x, y)` — as a DuckDB single-arrow lambda
    /// ([`Expr::Lambda`](crate::ast::Expr::Lambda)) instead of the JSON-arrow binary
    /// operator; any other left operand keeps the
    /// [`JsonGet`](crate::ast::BinaryOperator::JsonGet) reading.
    ///
    /// A grammar-position gate over a token another flag lexes: the `->` lexeme is
    /// munched only under [`json_arrow_operators`](Self::json_arrow_operators), so
    /// this flag is inert without it (the dependency is
    /// [`FeatureDependencyViolation::LambdaExpressionsWithoutJsonArrowOperators`]; no lexical
    /// trigger of its own, hence no [`LexicalConflict`] entry — the lexical registry covers
    /// shared *tokenizer* triggers, this one covers grammar dependencies). The node split mirrors the
    /// engine exactly (probed on DuckDB 1.5.4): DuckDB parses *every* `->` as a
    /// `LAMBDA` tree node — even `1 -> 2` or `t.a -> 'k'` — and defers the
    /// lambda-vs-JSON decision to bind time, where a lambda-consuming argument
    /// requires exactly the parameter shape above ("Parameters must be unqualified
    /// comma-separated names like x or (x, y)") and any other `->` is re-read as JSON
    /// extraction. Applying that bind-time shape test at parse time is a pure
    /// node-label choice: lambda `->` and JSON `->` share one token, one binding
    /// power, and one associativity, so acceptance is unchanged either way and the
    /// text round-trips identically. Position-independent, like the engine: a lambda
    /// parses anywhere an expression does (`SELECT x -> x + 1` parses in DuckDB; only
    /// the *binder* rejects an unconsumed lambda). Multi-parameter lists ride the
    /// implicit-row parse, so they additionally need
    /// [`ExpressionSyntax::row_constructor`] (on in every preset that sets this). On
    /// for DuckDB only.
    pub lambda_expressions: bool,
    /// Accept the shared bitwise operators — binary `|` (OR), `&` (AND), `<<`/`>>`
    /// (shift), and prefix `~` (complement) — over integers. On in PostgreSQL, MySQL,
    /// SQLite, and DuckDB (the family is cross-dialect); off in ANSI. A parser-acceptance
    /// gate over lexemes that always tokenize (`|`/`&`/`~` are operator-class bytes and
    /// `<<`/`>>` are maximal-munched unconditionally), so when off the operator ends the
    /// expression and the trailing operand surfaces as a clean parse error. Each operator's
    /// binding power lives in [`BindingPowerTable`], where the ranks diverge per dialect
    /// (MySQL splits `|` < `&` < `<<`/`>>`; PostgreSQL/SQLite/DuckDB share one rank).
    /// Bitwise *XOR* is a separate pair of knobs — [`FeatureSet::caret_operator`] for the
    /// `^` spelling and [`FeatureSet::hash_bitwise_xor`] for `#` — because XOR's spelling,
    /// lexing, and precedence all diverge (`#` vs `^`).
    pub bitwise_operators: bool,
    /// Accept the quantified subquery comparison `<expr> <cmp> {ANY | ALL | SOME}
    /// (<subquery>)` (SQL-92 F291), as in `a = ANY (SELECT …)` / `a > ALL (SELECT …)`.
    /// On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient. SQLite has no quantified comparison
    /// (it spells the same intent with `IN`/`EXISTS`), so it is off; the `ANY`/`ALL`/
    /// `SOME` keyword is then not read as a quantifier and surfaces as a clean parse
    /// error. Gates only the subquery-quantifier reading; `ALL`/`DISTINCT` as an
    /// aggregate-argument quantifier is the separate always-parsed set-quantifier.
    pub quantified_comparisons: bool,
    /// Accept the *list*-operand form of a quantified comparison — `<expr> <cmp>
    /// {ANY | ALL | SOME} (<value>)` where the parenthesized operand is a scalar
    /// list/array value rather than a subquery, as in DuckDB `ax = ANY (b)` /
    /// `x = ANY ([1, 2, 3])` and PostgreSQL `x = ANY (ARRAY[…])`. On for
    /// PostgreSQL/DuckDB/Lenient (which model it as PostgreSQL's `ScalarArrayOpExpr`,
    /// parsed to [`Expr::QuantifiedList`](crate::ast::Expr::QuantifiedList)); off for
    /// ANSI/MySQL, which admit only the subquery quantifier, and vacuously off for
    /// SQLite, which has no quantified comparison at all. Rides on top of
    /// [`quantified_comparisons`](Self::quantified_comparisons): when that gate leaves
    /// `ANY`/`ALL`/`SOME` unread this flag is unreachable, so it is only meaningfully
    /// set alongside it (the dependency is
    /// [`FeatureDependencyViolation::QuantifiedComparisonListsWithoutQuantifiedComparisons`]).
    /// When off, a non-subquery operand surfaces as the same clean
    /// "a subquery" parse error the standard form has always produced.
    pub quantified_comparison_lists: bool,
    /// Extend the quantifier `{ANY | ALL | SOME} (…)` past the six comparison
    /// operators to *any* infix operator — `<expr> <op> {ANY | ALL | SOME} (…)`
    /// where `<op>` is arithmetic (`+ - * / % ^`), string concatenation (`||`),
    /// bitwise (`& | # << >>`), or a comparison. PostgreSQL's grammar admits every
    /// operator in `MathOp`/`Op` here, only the boolean keywords `AND`/`OR` are
    /// excluded (engine-probed); the standard and the other dialects restrict the
    /// quantifier to the comparison operators. On for PostgreSQL/Lenient; off for
    /// ANSI/MySQL/DuckDB/SQLite. Rides on
    /// [`quantified_comparisons`](Self::quantified_comparisons) (and
    /// [`quantified_comparison_lists`](Self::quantified_comparison_lists) for the
    /// array-operand form): when the quantifier is unread this flag is unreachable (the
    /// dependency is [`FeatureDependencyViolation::QuantifiedArbitraryOperatorWithoutQuantifiedComparisons`]).
    /// When off, a non-comparison operator before `ANY`/`ALL`/`SOME` folds as an
    /// ordinary binary operator and the quantifier keyword then rejects as usual.
    pub quantified_arbitrary_operator: bool,
    /// Accept the general symbolic-operator surface — ANY operator drawn from the `Op`
    /// character class (`~ ! @ # ^ & | ? + - * / % < > =`), in both infix and prefix
    /// position, over a user-extensible operator set. A dialect-neutral capability any preset
    /// can enable; PostgreSQL (its `pg_operator`) is the current enabler, and the model
    /// follows its grammar. This is the ONE model for the whole
    /// tail (regex `~`/`!~`/`~*`/`!~*`; geometric/network/text-search `&&`/`&<`/`&>`/`<->`/
    /// `<<|`/`|>>`/`^@`/`##`/`<^`/`<%`/`@-@`; negator spellings `*<>`/`*>=`; the prefix
    /// `@`/`@@`/`|/`/`||/`/`!!`; and a fully user-defined `@#@`) rather than an enumerated
    /// per-lexeme set: the grammar admits every one as the same `Op`/`qual_Op`
    /// production, so a bare `a ~ b` is exactly `a OPERATOR(~) b`
    /// ([`Expr::NamedOperator`](crate::ast::Expr::NamedOperator) with the
    /// [`Bare`](crate::ast::NamedOperatorSpelling::Bare) spelling), and a prefix `@ x` is
    /// [`Expr::PrefixOperator`](crate::ast::Expr::PrefixOperator). Known operators still
    /// fold onto their dedicated [`BinaryOperator`] keys; only
    /// the remainder becomes a named operator.
    ///
    /// **Also gates the tokenizer.** Under this flag the operator scanner switches to
    /// PostgreSQL's maximal-munch lexer rule: a run of `Op` characters is one operator,
    /// truncated at an embedded `--`/`/*` comment start and with a trailing `+`/`-` stripped
    /// unless the run holds one of `~ ! @ # ^ & | ? %` (engine-measured: `a +- b` is
    /// `a + (- b)` but `a @- b` is one `@-` operator; `a <-- b` is `a <` then a `--`
    /// comment). The run that matches no built-in operator becomes an
    /// `Operator::Custom` token (the parser crate's tokenizer) carrying its span. Off leaves
    /// the fixed-form lexer untouched, so every other dialect's operator lexing is
    /// unchanged. The bare operators bind at the "any other operator" rank
    /// ([`BindingPowerTable::any_operator`](crate::precedence::BindingPowerTable::any_operator)),
    /// left-associative. On for PostgreSQL and DuckDB.
    ///
    /// The `Op` character class is not identical across enablers: DuckDB drops `#` and `?`
    /// (its positional-column `#1` and anonymous-parameter `?` sigils), so its operator runs
    /// stop at those bytes where PostgreSQL's do not — the lexer's `is_operator_char` reads
    /// the [`ExpressionSyntax::positional_column`] / [`ParameterSyntax::anonymous_question`]
    /// flags rather than hard-coding one charset. DuckDB *postfix* symbolic operators (`1 !`,
    /// removed from PostgreSQL in 14) are a separate axis this flag does not carry.
    ///
    /// The bare `@` operator shares its lead byte with the T-SQL `@name` / MySQL
    /// `@var` / `@@sysvar` sigils; the sigil arms win where a dialect enables them (the
    /// tracked [`LexicalConflict::CustomOperatorVersusAtName`] /
    /// [`LexicalConflict::CustomOperatorVersusSystemVariable`]), and no shipped preset
    /// enables both, so under PostgreSQL a bare `@` is always the operator.
    pub custom_operators: bool,
    /// Accept PostgreSQL/SQLite's one-word postfix null-test synonyms `<expr> ISNULL` and
    /// `<expr> NOTNULL` (for `IS NULL` / `IS NOT NULL`), folded onto
    /// [`Expr::IsNull`](crate::ast::Expr::IsNull) with a
    /// [`NullTestSpelling::Postfix`](crate::ast::NullTestSpelling) tag so they round-trip.
    /// A pure grammar gate at comparison precedence (the same non-associative rank as `IS
    /// NULL`): when off, the trailing `ISNULL`/`NOTNULL` keyword is left unconsumed and
    /// surfaces as a clean parse error (MySQL, which has no such synonym — `ISNULL(x)` is a
    /// *function* there, unaffected by this postfix gate). On for
    /// PostgreSQL/DuckDB/SQLite/Lenient.
    pub null_test_postfix: bool,
    /// Read a trailing symbolic operator with no following operand as a postfix operator
    /// application ([`Expr::PostfixOperator`](crate::ast::Expr::PostfixOperator)): `10!`
    /// (factorial), `1 ~`, `1 <->`, `1 &`. DuckDB keeps the generalized postfix reading
    /// PostgreSQL removed in version 14 (`a_expr Op %prec POSTFIXOP`), so it is the only
    /// enabler; PostgreSQL 14+ and every other preset reject the trailing operator.
    ///
    /// A pure parser-position gate, MECE against [`custom_operators`](Self::custom_operators):
    /// that flag owns the tokenizer's maximal-munch lexer and the *infix*/*prefix* general
    /// operator surface, while this flag owns only the *postfix* reduction of an already-lexed
    /// `Op`-class token. The infix reading still wins whenever an operand follows the operator
    /// (`1 ! + 2` is the infix `1 ! (+2)`); the postfix reduction fires only in the
    /// operand-absent position (`1 ! < 2`, `10!`, `1 ! FROM t`), engine-measured on DuckDB
    /// 1.5.4 via `duckdb_extract_statements` (parse-accept) — the unknown postfix operators
    /// then bind-reject (`Scalar Function !~__postfix does not exist`), an under-acceptance our
    /// parse-only parser closes by accepting the parse. The postfix binds at the "any other
    /// operator" left rank (looser than the arithmetic operators: `2 * 3 !` is `(2 * 3)!`).
    /// The postfix-eligible tokens are the general symbolic operators — the `Custom` residue,
    /// the lone `~`/`!`/`&&`, and the dedicated `& | << >> || <@ @> ^@`; the JSON arrows
    /// `->`/`->>` are excluded (DuckDB rejects them postfix). On for DuckDB, and for the
    /// permissive Lenient union (a pure additive parser position with no contended trigger,
    /// though only the always-lexed `Op` tokens reach it there — `custom_operators` is off
    /// under Lenient for the `@`-sigil conflict).
    pub postfix_operators: bool,
}

/// Dialect-owned function-*call* syntax accepted by the parser.
///
/// The call-tail and special-call-form family, split out of [`ExpressionSyntax`] when it
/// crossed its 16-field line; the aggregate/window call forms and the `StringFunc` keyword
/// special forms later split out again into [`AggregateCallSyntax`] and [`StringFuncForms`]
/// at their own 16-field lines. Each flag decides whether the parser *admits* the form at
/// its call-grammar position, and when off the introducing keyword/arrow is left unconsumed
/// and surfaces as a clean parse error (`named_argument` additionally gates the `=>`/`:=`
/// tokenizer lexemes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallSyntax {
    /// Accept PostgreSQL named function arguments `f(name => value)` and the
    /// deprecated `f(name := value)`. Also gates the tokenizer for the `=>` / `:=`
    /// arrow lexemes. **Shared `:=` claim:** MySQL's
    /// [`SessionVariableSyntax::variable_assignment`] also enables `:=` lexing for
    /// `SET @v := …`; the two never coexist in a shipped preset, so one
    /// `Operator::ColonEquals` token stays unambiguous per
    /// dialect (see tokenizer `:=` munch comments).
    pub named_argument: bool,
    /// Accept MySQL's `UTC_DATE` / `UTC_TIME` / `UTC_TIMESTAMP` niladic date/time
    /// functions — the UTC-clock analogues of the `CURRENT_*` special value functions,
    /// sharing the same nullary (plus optional precision on the time forms) grammar
    /// production. A parser-acceptance gate only: the keywords always tokenize (they are
    /// non-reserved outside MySQL), so when off they stay ordinary column/function names.
    /// Expression position only — MySQL has no PostgreSQL-style `func_table` promotion.
    pub utc_special_functions: bool,
    /// Read `COLUMNS(<selector>)` as DuckDB's star-expression column selector
    /// ([`Expr::Columns`](crate::ast::Expr::Columns)) rather than an ordinary call to a
    /// function named `columns`. A grammar-position gate keyed on the (non-reserved)
    /// `COLUMNS` keyword immediately followed by `(`: DuckDB has no user function of
    /// that spelling, so the form is unambiguous once the flag is on (the tokenizer is
    /// untouched — the disambiguation is a parser lookahead, mirroring `ARRAY[`/`ROW(`).
    /// A bare `columns` with no `(` stays an identifier in every dialect. Separate from
    /// [`SelectSyntax::wildcard_modifiers`](SelectSyntax::wildcard_modifiers) because
    /// the surfaces sit in different grammar positions — `COLUMNS(…)` is an expression
    /// (it nests in `sum(COLUMNS(*))`, `COLUMNS(*)::JSON`), the `*` modifiers are a
    /// projection-item tail — and are read by different parser code (expression vs
    /// select-item). When off, `COLUMNS(x)` parses as a plain function call, matching
    /// every non-DuckDB dialect. On for DuckDB / Lenient, off elsewhere.
    pub columns_expression: bool,
    /// Accept the `EXTRACT(<field> FROM <source>)` datetime-field extraction special
    /// form (SQL-92 F052; PostgreSQL). On for ANSI/PostgreSQL/MySQL/DuckDB/Lenient.
    /// SQLite has no `EXTRACT` (its date functions are `strftime`/`date`/…), so it is
    /// off; `EXTRACT` then reads as an ordinary identifier/function name and the `FROM`
    /// inside its parentheses surfaces as a clean parse error. Gates only the
    /// `FROM`-separated form — an ordinary `extract(a, b)` call is unaffected.
    pub extract_from_syntax: bool,
    /// Read `TRY_CAST(<expr> AS <type>)` as DuckDB's null-on-failure cast — the same
    /// [`Expr::Cast`](crate::ast::Expr::Cast) shape as `CAST`, distinguished by the
    /// canonical `try` flag (DuckDB's own serialized tree carries `try_cast: true`), not
    /// a spelling tag: null-on-failure is different *semantics*, not a different spelling
    /// of `CAST`. A grammar-position gate keyed on the (DuckDB-only) `TRY_CAST`
    /// keyword immediately followed by `(`; when off, `TRY_CAST` reads as an ordinary
    /// function name and the `AS` inside its parentheses surfaces as a clean parse error,
    /// matching every non-DuckDB dialect (verified: PostgreSQL syntax-errors at `AS`). On
    /// for DuckDb/Lenient.
    pub try_cast: bool,
    /// Restrict the `CAST(<expr> AS <target>)` target to MySQL's narrow `cast_type`
    /// grammar rather than the full column-type vocabulary. Off for
    /// ANSI/PostgreSQL/SQLite/DuckDB/Lenient, whose casts admit any type name. On for
    /// MySQL, whose `CAST`/`CONVERT` target is a closed set —
    /// `SIGNED`/`UNSIGNED [INTEGER|INT]`, `CHAR`/`NCHAR`/`CHARACTER`/`NATIONAL CHAR`,
    /// `BINARY`, `DATE`, `DATETIME`, `TIME`, `DECIMAL`/`DEC`, `DOUBLE`/`DOUBLE PRECISION`,
    /// `FLOAT`, `REAL`, `JSON`, `YEAR`, and the spatial types (`POINT`, `LINESTRING`,
    /// `POLYGON`, `MULTIPOINT`, `MULTILINESTRING`, `MULTIPOLYGON`, `GEOMETRYCOLLECTION`, and
    /// the `GEOMCOLLECTION` alias) — so the common column types `INT`/`INTEGER`/
    /// `SMALLINT`/`BIGINT`/`TINYINT`, `VARCHAR`/`TEXT`, `TIMESTAMP`, `NUMERIC`, `BOOLEAN`,
    /// `VARBINARY`/`BLOB`/`BIT`, bare `GEOMETRY`, and any user-defined name are the syntax
    /// error MySQL reports in cast position (while remaining valid as column types). When on,
    /// the parser parses the target type as usual, then rejects it unless it is one of the
    /// MySQL cast targets (the whole set engine-measured on mysql:8 —
    /// `mysql-faithful-cast-type-production`).
    ///
    /// `YEAR` and the spatial cast targets parse as user-defined names, so the faithful
    /// production layers a name allowlist over the shape check (the parser's
    /// `is_mysql_cast_target`); the inert trailing `INTEGER`/`INT` of `SIGNED`/`UNSIGNED` is
    /// folded onto the standalone numeric modifier. The one engine-measured residual is the
    /// `CHAR` charset annotation
    /// (`CAST(x AS CHAR CHARACTER SET utf8mb4)`, and the `ASCII`/`UNICODE`/trailing-`BINARY`
    /// shorthands): MySQL accepts these, but type-name charset/collation is a general MySQL
    /// feature the shared type grammar does not model at all (columns take the same
    /// annotation), so it is over-rejected here — a separate type-grammar feature, not a
    /// cast-target-membership gap, and invisible to every corpus.
    pub restricted_cast_targets: bool,
    /// Accept a string literal as the `EXTRACT('<field>' FROM <source>)` field, as DuckDB
    /// admits (`extract('year' FROM x)`), storing the quoted field as an
    /// [`Ident`](crate::ast::Ident) with [`QuoteStyle::Single`](crate::ast::QuoteStyle) so
    /// it round-trips. On for Postgres/DuckDb/Lenient. When off the field is a bare
    /// identifier only, so a leading string surfaces as a clean parse error (the standard
    /// `EXTRACT` field is an identifier). PostgreSQL admits the same quoted field (`Sconst`
    /// in its `extract_arg`) — engine-verified against pg_query, including the reject
    /// boundary (a non-string non-identifier field rejects on both) — so the flag ships on
    /// under the `Postgres` preset too.
    pub extract_string_field: bool,
    /// Accept DuckDB's dot-method call chaining on a value — a postfix `.<method>(<args>)`
    /// on a non-name receiver (`list(forecast).list_transform(x -> x + 10)`) that desugars
    /// to the ordinary function call `<method>(<receiver>, <args>)`. On for DuckDb/Lenient.
    /// A plain `name.method(args)` is already the schema-qualified call the object-name
    /// grammar reads, so the postfix fires only on a receiver that is not a bare name (a
    /// function-call result, a parenthesized expression, …); there is no ambiguity with a
    /// qualified call. When off, a `.method(` after a value surfaces as a clean parse error.
    pub method_chaining: bool,
    /// Reject an empty argument list on PostgreSQL's SQL/JSON constructor keywords — the
    /// closed set `PG_SQLJSON_EMPTY_REJECTING_CONSTRUCTORS` in the parser crate (`JSON`,
    /// `JSON_SCALAR`, `JSON_SERIALIZE`). PostgreSQL parses these through dedicated `gram.y`
    /// productions that require the context-item / value argument, so `JSON()` /
    /// `JSON_SCALAR()` / `JSON_SERIALIZE()` is a syntax error, whereas we would otherwise
    /// admit them as ordinary niladic calls. Keyed on a single *unquoted* name, so a quoted
    /// `"json"()` stays an ordinary call PostgreSQL accepts (rejected only at name
    /// resolution). On for PostgreSQL only. The set is deliberately narrow and extension-
    /// shaped: the JSON_VALUE/JSON_QUERY grammar ([[pg-sqljson-expression-functions]])
    /// carries the same arity floor via [`sqljson_expression_functions`](CallSyntax::sqljson_expression_functions).
    pub sqljson_constructors_require_argument: bool,
    /// Parse the SQL:2016/2023 SQL/JSON expression functions as dedicated special forms:
    /// the `JSON_VALUE`/`JSON_QUERY`/`JSON_EXISTS` query functions with their
    /// `PASSING`/`RETURNING`/`FORMAT JSON`/wrapper/quotes/`ON EMPTY`/`ON ERROR` clause
    /// tails, the `JSON_OBJECT`/`JSON_ARRAY` constructors and their `JSON_OBJECTAGG`/
    /// `JSON_ARRAYAGG` aggregates (with `[ABSENT|NULL] ON NULL` and `[WITH|WITHOUT]
    /// UNIQUE [KEYS]`), the bare `JSON`/`JSON_SCALAR`/`JSON_SERIALIZE` constructors, and
    /// the `IS [NOT] JSON [VALUE|ARRAY|OBJECT|SCALAR] [WITH|WITHOUT UNIQUE [KEYS]]`
    /// predicate. On for PostgreSQL/Lenient (engine-verified against `pg_query`); the
    /// clause grammar is PostgreSQL's raw-parse surface (per-function legality such as
    /// `JSON_VALUE` rejecting a wrapper is enforced, while the shared behaviour set that
    /// PostgreSQL only narrows during parse *analysis* is admitted uniformly). Off for
    /// MySQL/DuckDB/SQLite/ANSI: MySQL has its own JSON functions with a *different*
    /// grammar, DuckDB/SQLite have no SQL/JSON standard special forms, and those
    /// dialects keep the keywords as ordinary function/column names. When off, the
    /// keyword heads fall through to the ordinary call/name path exactly as before.
    /// Composes with [`sqljson_constructors_require_argument`](CallSyntax::sqljson_constructors_require_argument):
    /// the empty `JSON()`/`JSON_SCALAR()`/`JSON_SERIALIZE()` reject is enforced by these
    /// special forms requiring their argument (PostgreSQL), while a dialect with the
    /// arity floor *off* (Lenient) falls the empty form back to an ordinary niladic call.
    pub sqljson_expression_functions: bool,
    /// Parse the SQL:2006 SQL/XML expression functions as dedicated special forms: the
    /// `xmlelement`/`xmlforest`/`xmlconcat`/`xmlparse`/`xmlpi`/`xmlroot`/`xmlserialize`/
    /// `xmlexists` constructors — with their keyword-clause grammar inside the parens
    /// (`NAME <label>`, `xmlattributes(…)`, `{DOCUMENT|CONTENT}`, `{PRESERVE|STRIP}
    /// WHITESPACE`, `VERSION {…|NO VALUE}`, `STANDALONE {YES|NO|NO VALUE}`, `AS <type>
    /// [[NO] INDENT]`, `PASSING [BY {REF|VALUE}] …`) — and the `IS [NOT] DOCUMENT`
    /// predicate. On for PostgreSQL/Lenient (engine-verified against `pg_query`); the
    /// clause grammar is PostgreSQL's raw-parse surface. Off for MySQL/DuckDB/SQLite/ANSI:
    /// none of those dialects have the SQL/XML standard special forms, and they keep the
    /// `xml*` keywords as ordinary function/column names. When off, the keyword heads fall
    /// through to the ordinary call/name path exactly as before. The `xmlagg` aggregate is
    /// *not* gated here — it is an ordinary keyword-free aggregate name that already parses
    /// through the ordinary aggregate call path in every dialect.
    pub xml_expression_functions: bool,
    /// Accept the call-site `VARIADIC` argument marker that spreads an array over a
    /// variadic parameter (`f(a, VARIADIC arr)`, `f(VARIADIC name => arr)`), riding the
    /// shared [`FunctionArg`](crate::ast::FunctionArg)'s
    /// [`variadic`](crate::ast::FunctionArg::variadic) flag. On for PostgreSQL/DuckDb/
    /// Lenient (both engines parse-accept it with identical rules — engine-probed on
    /// pg_query PG-17 and DuckDB 1.5.4). A parse-layer gate: the parser admits the
    /// `VARIADIC` prefix only on the *last* argument of the list and rejects it alongside
    /// an `ALL`/`DISTINCT` quantifier, mirroring both engines' `gram.y` productions
    /// (`func_application: … ',' VARIADIC func_arg_expr`), which carry no quantifier and
    /// place `VARIADIC` last. When off (ANSI/MySQL/SQLite), the `VARIADIC` keyword is left
    /// unconsumed and surfaces as a clean parse error. The argument-type check (the spread
    /// value must be an array) is a binding concern neither parser enforces.
    pub variadic_argument: bool,
    /// Accept PostgreSQL's `merge_action()` — the zero-argument special function that
    /// reports which `MERGE` branch produced a row (`'INSERT'`/`'UPDATE'`/`'DELETE'`),
    /// valid only in a `MERGE ... RETURNING` list. PostgreSQL gives it a dedicated
    /// `func_expr_common_subexpr` production (`MERGE_ACTION '(' ')'`), so at raw parse it
    /// is accepted *anywhere* an expression is (the MERGE-RETURNING-only restriction is a
    /// parse-*analysis* check, engine-verified against `pg_query`: `SELECT merge_action()`
    /// raw-parse-accepts) but takes strictly empty parens — `merge_action(1)` and
    /// `merge_action() OVER ()` are both syntax errors (probed). The keyword is reserved
    /// against ordinary calls (`POSTGRES_NON_GENERIC_FUNCTION_KEYWORDS`), so when off it
    /// stays the "no call form" reject it already was; when on, the strictly niladic form
    /// parses to the canonical [`Expr::Function`](crate::ast::Expr::Function) shape (name
    /// `merge_action`, no arguments). On for PostgreSQL/Lenient; off elsewhere (no other
    /// shipped dialect has the form). A bare `merge_action` with no `(` is untouched.
    pub merge_action_function: bool,
    /// Accept MySQL's `CONVERT` special-form function — both the comma-form cast
    /// `CONVERT(<expr>, <type>)` and the transcoding `CONVERT(<expr> USING <charset>)`
    /// form (grammar's one `CONVERT '(' … ')'` production, two shapes). Only `CONVERT`
    /// immediately followed by `(` opens it; when off, `CONVERT` keeps its ordinary
    /// function-name reading (PostgreSQL's plain `convert(bytea, name, name)` call is
    /// unaffected — engine-verified against `pg_query`, which parses `CONVERT('x', 'a', 'b')`
    /// as an ordinary call and rejects the `USING` form). The comma form folds onto the
    /// [`Expr::Cast`](crate::ast::Expr::Cast) node as [`CastSyntax::Convert`](crate::ast::CastSyntax)
    /// and shares [`restricted_cast_targets`](CallSyntax::restricted_cast_targets)' `cast_type`
    /// gate (so `CONVERT(1, INT)` rejects wherever `CAST(1 AS INT)` does); the USING form
    /// is a [`StringFunc::ConvertUsing`](crate::ast::StringFunc::ConvertUsing) whose charset
    /// operand is a MySQL `charset_name` (`ident_or_text` or the `BINARY` transcoding name).
    /// On for MySQL/Lenient; off elsewhere (no other shipped dialect has the special form).
    pub convert_function: bool,
}

/// Dialect-owned keyword string/scalar special-form syntax accepted by the parser.
///
/// The keyword special forms that parse to the [`StringFunc`](crate::ast::StringFunc)
/// AST family — the SQL-standard string special forms and the sibling scalar keyword forms
/// sharing that node. Split out of [`CallSyntax`] at its 16-field line on the grammar
/// boundary "the flag's sole AST product is a `StringFunc` variant" — so the dual
/// cast/transcode `CONVERT` stays with the cast core. Each flag is a grammar gate: when off
/// the keyword head falls through to the ordinary call path or the trailing keyword
/// surfaces as a clean parse error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StringFuncForms {
    /// Accept the `SUBSTRING(<expr> FROM <start> [FOR <count>])` keyword special form
    /// (SQL-92 E021-06). On for ANSI/PostgreSQL/DuckDB/MySQL/Lenient (each engine
    /// probed accepting); off for SQLite, which has no keyword string forms at all
    /// (probed: `near "FROM": syntax error`) — there the head falls through to the
    /// ordinary call path and the inner `FROM` is a clean parse error. The comma
    /// plain-call spelling `substring(x, 1, 2)` is untouched by this flag: it keeps
    /// parsing as an ordinary [`FunctionCall`](crate::ast::FunctionCall) everywhere
    /// (every probed engine accepts it). MySQL additionally requires the `(` adjacent
    /// to the head for the keyword form — the same `IGNORE_SPACE`-off demotion the
    /// aggregates and `EXTRACT` follow, composed via
    /// [`aggregate_args_require_adjacent_paren`](AggregateCallSyntax::aggregate_args_require_adjacent_paren)
    /// (probed: spaced `SUBSTRING ('a' FROM 2)` is 1064 while spaced
    /// `SUBSTRING ('a', 2)` parse-accepts through the demoted generic path).
    pub substring_from_for: bool,
    /// Accept the `FOR`-leading `SUBSTRING` spellings: the bare
    /// `SUBSTRING(<expr> FOR <count>)` and the reversed
    /// `SUBSTRING(<expr> FOR <count> FROM <start>)` order. On for
    /// PostgreSQL/DuckDB/Lenient (both engines probed accepting both orders); off for
    /// MySQL (probed 1064 on both — its grammar is strictly `FROM`-first), ANSI
    /// (SQL-92's `<character substring function>` is `FROM`-first only), and SQLite.
    pub substring_leading_for: bool,
    /// Accept PostgreSQL's `SUBSTRING(<expr> SIMILAR <pattern> ESCAPE <escape>)`
    /// regex form (SQL:1999's regular-expression substring function). On for
    /// PostgreSQL/Lenient; off for DuckDB (probed: parser error — its PG-fork grammar
    /// dropped the production), MySQL, SQLite, and ANSI (an optional-feature form only
    /// PostgreSQL ships). The `ESCAPE` operand is mandatory, matching pg_query
    /// (`SUBSTRING(x SIMILAR p)` rejects, probed).
    pub substring_similar: bool,
    /// Reject a `substring(…)`/`substr(…)` plain (comma) call whose argument count is
    /// not 2 or 3 — MySQL's dedicated grammar admits exactly `(str, pos)`,
    /// `(str, pos, len)`, and the `FROM`/`FOR` keyword forms, so `SUBSTRING('a')`,
    /// `SUBSTRING()`, and `SUBSTRING('a', 2, 3, 4)` are `ER_PARSE_ERROR` (1064) on
    /// mysql:8.4 (all probed) while PostgreSQL parse-accepts any arity
    /// (`SUBSTRING()` probed accepted — arity is a catalog lookup there, not
    /// grammar). On for MySQL only. Only the *adjacent-paren* call is checked: a
    /// spaced `SUBSTRING ('a')` is MySQL's demoted stored-function path, which
    /// parse-accepts any arity (probed 1046, a binding-class reject).
    pub substring_plain_call_requires_2_or_3_args: bool,
    /// Accept the `SUBSTR` head for the same `FROM`/`FOR` keyword forms
    /// (`SUBSTR(str FROM 2 FOR 3)`). On for MySQL/Lenient: MySQL's `SUBSTR` is a
    /// full synonym of `SUBSTRING` including the keyword grammar (probed accepted),
    /// while PostgreSQL and DuckDB have no `SUBSTR` keyword at all — their `substr`
    /// is an ordinary catalog function, so the keyword form parse-rejects (both
    /// probed) and the flag stays off. `substr` is not a keyword in any dialect's
    /// inventory, so the head is matched textually on an unquoted call only — a
    /// quoted `"substr"(…)` stays an ordinary call, and every plain `substr(a, b)`
    /// call is untouched.
    pub substr_from_for: bool,
    /// Accept the `POSITION(<substr> IN <string>)` keyword special form (SQL-92
    /// E021-11). On for ANSI/PostgreSQL/DuckDB/MySQL/Lenient; off for SQLite (no
    /// keyword form; its `position(a, b)` stays an ordinary call that fails only at
    /// binding). There is NO plain-call fallback where the flag is on:
    /// `position('b', 'abc')` is a parse error on PostgreSQL, DuckDB, *and* MySQL
    /// (all probed), so a comma after the first operand surfaces as a clean parse
    /// error rather than re-reading as a generic call. The operands are the
    /// restricted `b_expr` (PostgreSQL's `position_list`, DuckDB inheriting it —
    /// `POSITION('a' = 'b' IN 'c')` parse-accepts on both while
    /// `POSITION(1 IN 2 OR 3)` rejects, both probed); MySQL's asymmetric grammar is
    /// [`position_asymmetric_operands`](StringFuncForms::position_asymmetric_operands).
    pub position_in: bool,
    /// Use MySQL's asymmetric `POSITION` operand grammar — `bit_expr IN expr` — in
    /// place of the standard symmetric `b_expr IN b_expr`. The needle tightens to
    /// MySQL's `bit_expr` (arithmetic/bit operators only — no comparisons:
    /// `POSITION('a' = 'b' IN 'c')` is 1064, probed) and the haystack widens to a
    /// full expression (`POSITION(1 IN 2 OR 3)` accepts, probed). On for MySQL only.
    pub position_asymmetric_operands: bool,
    /// Accept the `OVERLAY(<target> PLACING <replacement> FROM <start> [FOR <count>])`
    /// keyword special form (SQL:1999 T312). On for ANSI/PostgreSQL/DuckDB/Lenient;
    /// off for MySQL (no `OVERLAY` at all — the keyword form is 1064 and a plain
    /// `overlay(…)` call parses as a stored-function reference, probed) and SQLite.
    /// `FROM <start>` is mandatory: `OVERLAY(x PLACING y)` and
    /// `OVERLAY(x PLACING y FOR 1)` parse-reject on PostgreSQL and DuckDB (probed).
    pub overlay_placing: bool,
    /// Require the `PLACING` form after `OVERLAY(` — i.e. drop the plain-call
    /// fallback. DuckDB's grammar has *only* the `PLACING` production:
    /// `overlay('abc', 'X', 2, 1)`, `overlay('abc')`, and `overlay()` are all parser
    /// errors there (probed), while PostgreSQL keeps its `func_arg_list_opt`
    /// alternative so the same spellings parse-accept (probed; arity is a catalog
    /// concern there). On for DuckDB and ANSI (the standard defines no plain
    /// `overlay` call); off for PostgreSQL/Lenient, where a non-`PLACING` argument
    /// list falls back to the ordinary call path.
    pub overlay_requires_placing: bool,
    /// Accept the restricted `TRIM([{BOTH | LEADING | TRAILING}] [<chars>] FROM
    /// <source>)` keyword special form (SQL-92 E021-09): a side and/or a
    /// trim-character expression, then `FROM` and exactly one source. On for
    /// ANSI/PostgreSQL/DuckDB/MySQL/Lenient; off for SQLite (probed: syntax error —
    /// its two-argument `trim(x, y)` plain call is the only spelling). The bare
    /// single-argument `TRIM(x)` stays an ordinary call everywhere, and `TRIM()`
    /// rejects wherever the flag is on (PostgreSQL/DuckDB/MySQL all probed rejecting
    /// the empty form). MySQL requires at least one of side/chars before `FROM`
    /// (`TRIM(FROM 'x')` is 1064, probed) and holds the keyword form to the adjacent
    /// `(` like `SUBSTRING` (spaced `TRIM (LEADING …)` is 1064 while spaced
    /// `TRIM ('abc')` parse-accepts through the demoted generic path, both probed);
    /// the looser PostgreSQL tails are [`trim_list_syntax`](StringFuncForms::trim_list_syntax).
    pub trim_from: bool,
    /// Accept PostgreSQL's loose `trim_list` tails on the `TRIM` special form: the
    /// bare `TRIM(FROM <list>)` (no side, no chars), a side without `FROM`
    /// (`TRIM(TRAILING ' foo ')`, `TRIM(LEADING 'x', 'y')`), a multi-expression
    /// source list (`TRIM('a' FROM 'b', 'c')`, `TRIM(BOTH FROM 'a', 'b')`), and the
    /// comma plain-call spelling `trim('a', 'b')` (which PostgreSQL parses through
    /// the same production and we keep as an ordinary call). On for
    /// PostgreSQL/DuckDB/Lenient (every listed form probed parse-accepting on both
    /// engines — DuckDB's rejects here are binder arity, not grammar); off for MySQL
    /// (each probed 1064), ANSI (the standard's trim operand takes one source), and
    /// SQLite. Where this is off but [`trim_from`](StringFuncForms::trim_from) is on, a comma
    /// after the first `TRIM` operand is a clean parse error — matching MySQL, whose
    /// `trim('a', 'b')` is 1064 (probed).
    pub trim_list_syntax: bool,
    /// Accept PostgreSQL's `COLLATION FOR (<expr>)` common-subexpr — the special form that
    /// reports the collation name derived for its operand. PostgreSQL gives it a dedicated
    /// `COLLATION FOR '(' a_expr ')'` production (lowered to a
    /// `pg_catalog.pg_collation_for(<expr>)` call), so only `COLLATION` immediately trailed
    /// by `FOR (` opens it; the parentheses and single `a_expr` operand are mandatory —
    /// `COLLATION FOR 'x'`, `COLLATION FOR ()`, and a two-argument list all reject
    /// (engine-verified against `pg_query`). When off, `COLLATION` keeps its ordinary
    /// `type_func_name` reading (a plain `collation(x)` call is unaffected either way). On
    /// for PostgreSQL/Lenient; off elsewhere (no other shipped dialect has the form —
    /// DuckDB overrides PostgreSQL's `true` back to `false`, its `COLLATION FOR` surface
    /// unprobed). The parsed form keeps its keyword shape as
    /// [`StringFunc::CollationFor`](crate::ast::StringFunc::CollationFor) so it round-trips
    /// as written rather than as the lowered call.
    pub collation_for_expression: bool,
    /// Accept the `CEIL`/`CEILING` rounding-field keyword form: `CEIL(<expr> TO <field>)`
    /// (and the `CEILING` spelling). No probed oracle grammar admits the `TO` tail —
    /// engine-verified against `pg_query` (`syntax error at or near "TO"`), DuckDB, and
    /// mysql:8.4.10 (all reject) — so this is sqlparser-rs-parity surface only, not a
    /// real-engine grammar; on for Lenient, off for every shipped engine preset. Only
    /// `CEIL`/`CEILING` immediately followed by `(` opens the speculative read (mirroring
    /// [`substring_from_for`](StringFuncForms::substring_from_for)'s shape): the first operand parses
    /// as an ordinary expression, and only a following `TO` commits to the special form —
    /// a first operand with no `TO` tail rewinds to the ordinary call path, so the comma
    /// scale spelling `CEIL(<expr>, <scale>)` is untouched and keeps parsing as a plain
    /// [`FunctionCall`](crate::ast::FunctionCall) in every dialect regardless of this flag.
    /// When off, `CEIL(x TO DAY)` is the same clean parse error it is today (an
    /// unexpected `TO` where a `,` or `)` is expected). The field (`DAY`, `HOUR`, …) is
    /// stored as a written [`Ident`](crate::ast::Ident), validated (if at all) by the
    /// consuming engine at analysis time, not parse. The parsed form is
    /// [`StringFunc::CeilTo`](crate::ast::StringFunc::CeilTo).
    pub ceil_to_field: bool,
    /// Accept the `FLOOR` rounding-field keyword form: `FLOOR(<expr> TO <field>)`. No
    /// probed oracle grammar admits the `TO` tail — engine-verified against `pg_query`
    /// (`syntax error at or near "TO"`), DuckDB, and mysql:8.4.10 (all reject) — so this
    /// is sqlparser-rs-parity surface only, not a real-engine grammar; on for Lenient,
    /// off for every shipped engine preset. Unlike `CEIL`/`CEILING`, `FLOOR` has no
    /// synonym spelling to track. Only `FLOOR` immediately followed by `(` opens the
    /// speculative read (mirroring [`ceil_to_field`](StringFuncForms::ceil_to_field)'s shape): the
    /// first operand parses as an ordinary expression, and only a following `TO` commits
    /// to the special form — a first operand with no `TO` tail rewinds to the ordinary
    /// call path, so the comma scale spelling `FLOOR(<expr>, <scale>)` is untouched and
    /// keeps parsing as a plain [`FunctionCall`](crate::ast::FunctionCall) in every
    /// dialect regardless of this flag. When off, `FLOOR(x TO DAY)` is the same clean
    /// parse error it is today (an unexpected `TO` where a `,` or `)` is expected). The
    /// field (`DAY`, `HOUR`, …) is stored as a written [`Ident`](crate::ast::Ident),
    /// validated (if at all) by the consuming engine at analysis time, not parse. The
    /// parsed form is [`StringFunc::FloorTo`](crate::ast::StringFunc::FloorTo).
    pub floor_to_field: bool,
    /// Accept MySQL's full-text `MATCH (<col>, …) AGAINST (<expr> [<modifier>])` special-form
    /// expression (grammar's `MATCH ident_list_arg AGAINST '(' bit_expr fulltext_options ')'`).
    /// Only `MATCH` immediately followed by `(` opens it; the column list is comma-separated
    /// column references (a general expression, literal, function call, or empty list all
    /// parse-reject), the `AGAINST` operand is a `bit_expr` (so a trailing `IN`/`WITH` opens the
    /// modifier rather than an `IN` predicate), and the optional modifier is exactly one of
    /// `IN NATURAL LANGUAGE MODE`, `IN NATURAL LANGUAGE MODE WITH QUERY EXPANSION`,
    /// `IN BOOLEAN MODE`, or `WITH QUERY EXPANSION` (all engine-verified on mysql:8.4.10; the
    /// non-reserved `AGAINST`/`QUERY`/`EXPANSION` words are matched contextually, MySQL's
    /// reserved-only inventory design). The parsed form is
    /// [`StringFunc::MatchAgainst`](crate::ast::StringFunc::MatchAgainst). Distinct from SQLite's
    /// infix `<expr> MATCH <expr>` operator (a binding-power table entry, not this gate). On for
    /// MySQL/Lenient; off elsewhere (no other shipped dialect has the special form).
    pub match_against: bool,
}

/// Dialect-owned aggregate/window function-call syntax accepted by the parser.
///
/// The call-grammar forms specific to aggregate and window function calls — the
/// in-parenthesis and post-`)` tails, the argument-shape and arity restrictions, and
/// the `OVER`-eligibility gate. Split out of [`CallSyntax`] at its 16-field line as
/// the aggregate/window axis, distinct from the scalar special-form and general
/// call-tail cores. Each flag is a grammar gate: when off the tail keyword is left
/// unconsumed and surfaces as a clean parse error, or the restriction does not fire.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AggregateCallSyntax {
    /// Accept the MySQL `GROUP_CONCAT(<args> [ORDER BY …] SEPARATOR <string>)` delimiter
    /// tail — the trailing `SEPARATOR '<sep>'` inside an aggregate call's parentheses,
    /// after any in-parenthesis `ORDER BY`. It rides the shared
    /// [`FunctionCall`](crate::ast::FunctionCall) shape's new
    /// [`separator`](crate::ast::FunctionCall::separator) field, gated by
    /// grammar position like the always-parsed in-parenthesis `ORDER BY`. When off
    /// (ANSI/PostgreSQL, which write the delimiter as an ordinary `string_agg` argument),
    /// the `SEPARATOR` keyword is left unconsumed and the unmatched `)` surfaces as a
    /// clean parse error.
    pub group_concat_separator: bool,
    /// Accept the `WITHIN GROUP (ORDER BY …)` ordered-set-aggregate tail (SQL:2008
    /// T612/T614), as in `percentile_cont(0.5) WITHIN GROUP (ORDER BY x)`. On for
    /// ANSI/PostgreSQL/DuckDB/Lenient. Neither SQLite nor MySQL has ordered-set
    /// aggregates (both engine-measured-rejected), so it is off for both; the `WITHIN`
    /// keyword is then left unconsumed and surfaces as a clean parse error. Only the
    /// `WITHIN GROUP` pair opens the clause, so a bare `within` stays a usable alias
    /// regardless.
    pub within_group: bool,
    /// Accept the `FILTER (WHERE <predicate>)` aggregate-filter tail (SQL:2003 T612), as
    /// in `sum(x) FILTER (WHERE x > 1)`. On for ANSI/PostgreSQL/SQLite (3.30+)/DuckDB/
    /// Lenient. MySQL has no aggregate `FILTER` clause (engine-measured-rejected on
    /// mysql:8), so it is off there; the `FILTER` keyword is then left unconsumed and
    /// surfaces as a clean parse error. Only `FILTER` immediately followed by `(` opens the
    /// clause, so a bare `filter` after a call stays a usable alias.
    pub aggregate_filter: bool,
    /// Accept an aggregate `FILTER (…)` tail whose predicate is *not* preceded by the
    /// SQL-standard `WHERE` keyword, as in DuckDB's `sum(x) FILTER (x > 1)` (probed on
    /// 1.5.4). On for DuckDb/Lenient; the presence of the keyword round-trips via
    /// [`FunctionCall::filter_where`](crate::ast::FunctionCall::filter_where). Off for
    /// ANSI/PostgreSQL/SQLite, which require `FILTER (WHERE …)` — a keyword-less body then
    /// surfaces as a clean "expected `WHERE`" parse error. Independent of
    /// [`aggregate_filter`](Self::aggregate_filter): this only widens the *body* of a
    /// clause that gate already admits, so it is inert when the filter clause itself is off
    /// (MySQL).
    pub filter_optional_where: bool,
    /// Require a built-in aggregate's argument parentheses to be *adjacent* to its name
    /// for the aggregate-only argument forms — a leading `*`, a `DISTINCT`/`ALL` quantifier,
    /// the in-parenthesis `ORDER BY`, or the `SEPARATOR` tail. On for MySQL, off everywhere
    /// else. MySQL's default (`IGNORE_SPACE` off) tokenizer treats a space before the `(`
    /// as demoting a built-in aggregate to an ordinary/stored-function reference, where that
    /// aggregate-only argument grammar is illegal: `COUNT ( * )` / `MAX ( ALL 1 )` /
    /// `COUNT ( DISTINCT 1 )` are engine-measured `ER_PARSE_ERROR` (1064) on mysql:8, while
    /// the adjacent `COUNT(*)` accepts. A *normal*-argument spaced call is unaffected —
    /// `count (1)` still parses (it fails only at name resolution, a binding not a syntax
    /// error), so this narrowly rejects the aggregate-only forms rather than blanket-forbidding
    /// a space (which would over-reject the valid general-call form). When off, the name/paren
    /// gap is irrelevant and every dialect admits the aggregate forms with or without the space.
    /// Only the default `IGNORE_SPACE`-off mode is modelled; the runtime `sql_mode` toggle to
    /// `IGNORE_SPACE` on (which would accept the spaced aggregate forms) is out of scope.
    pub aggregate_args_require_adjacent_paren: bool,
    /// Accept the `IGNORE NULLS` / `RESPECT NULLS` null-treatment written *inside* a
    /// window/aggregate call's parentheses (DuckDB's `last(s IGNORE NULLS) OVER (…)`),
    /// riding the shared [`FunctionCall`](crate::ast::FunctionCall)'s
    /// [`null_treatment`](crate::ast::FunctionCall::null_treatment) field. On for
    /// DuckDb/Lenient. DuckDB spells it inside the parentheses (the SQL:2016 post-`)`
    /// position engine-rejects on 1.5.4), so it is parsed at the in-parenthesis tail after
    /// any `ORDER BY`. When off, `IGNORE`/`RESPECT` is left unconsumed and the unmatched `)`
    /// surfaces as a clean parse error. PostgreSQL has no null-treatment clause, so this
    /// stays a DuckDB extension rather than a shared PG form.
    pub null_treatment: bool,
    /// Reject an empty argument list `f()` on a MySQL built-in *aggregate* function —
    /// the closed set `MYSQL_AGGREGATE_FUNCTIONS` in the parser crate (`COUNT`, `SUM`,
    /// `AVG`, `MIN`, `MAX`, the `BIT_*`/`STD*`/`VAR*`/`VARIANCE` family, `GROUP_CONCAT`,
    /// `JSON_ARRAYAGG`/`JSON_OBJECTAGG`). MySQL's dedicated aggregate grammar requires at
    /// least one argument (or the `COUNT(*)` wildcard), so `COUNT()` is `ER_PARSE_ERROR`
    /// (1064) on mysql:8, while a niladic *non*-aggregate built-in (`NOW()`, `UUID()`,
    /// `PI()`) and an empty user-function call are accepted (the latter fails only at
    /// name resolution — a binding, not a syntax error — and a `CONCAT()`/`ABS()` empty
    /// call is a `wrong-parameter-count` semantic reject, also not a syntax error). On for
    /// MySQL, off elsewhere. When off, an empty aggregate call parses as an ordinary empty
    /// function call (every non-MySQL dialect admits it). Only a single *unquoted* name in
    /// the set matches — a backtick-quoted `` `count`() `` is a general call MySQL rejects
    /// at binding, not a syntax error, so it stays accepted — and the `COUNT(*)` wildcard
    /// and any argumented/quantified call are unaffected.
    pub aggregate_calls_reject_empty_arguments: bool,
    /// Restrict the `OVER (…)` window clause to MySQL's *windowable* functions — the
    /// built-in aggregates (`MYSQL_AGGREGATE_FUNCTIONS`) ∪ the dedicated window functions
    /// (`MYSQL_WINDOW_FUNCTIONS`: `ROW_NUMBER`, `RANK`, `DENSE_RANK`, `PERCENT_RANK`,
    /// `CUME_DIST`, `NTILE`, `LEAD`, `LAG`, `FIRST_VALUE`, `LAST_VALUE`, `NTH_VALUE`), both
    /// in the parser crate. MySQL grammatically admits `OVER` only on this set:
    /// `OVER` on an ordinary scalar built-in or a user function (`ABS(x) OVER ()`,
    /// `PERCENTILE_CONT(x, 0.5) OVER ()`, `ANY_VALUE(x) OVER ()`) is `ER_PARSE_ERROR`
    /// (1064) on mysql:8, while `SUM(x) OVER ()` / `ROW_NUMBER() OVER ()` /
    /// `GROUP_CONCAT(x) OVER ()` parse (they fail only at binding or a not-supported-yet
    /// semantic reject). On for MySQL, off elsewhere.
    ///
    /// The vocabulary must stay *complete*: an omission would over-*reject* a valid
    /// windowed call (the worse failure — no coverage-gap pin catches an over-rejection),
    /// so it is the engine-verified full MySQL 8.0 aggregate + window function list. A
    /// qualified name (`db.f(x) OVER ()`, engine-rejected too) is not a single-part member
    /// and is likewise rejected. When off, `OVER` attaches to any call, matching every
    /// non-MySQL dialect.
    ///
    /// This flag also gates the *converse* half of MySQL's dedicated window-function
    /// grammar — the requirements the pure window functions (`MYSQL_WINDOW_FUNCTIONS`,
    /// admitted as call heads by carving them out of `MYSQL_RESERVED_FUNCTION_NAME`) carry
    /// once admitted. Unlike the aggregates (whose `OVER` is optional), each window
    /// function *requires* an `OVER` clause, takes a *fixed* positional argument arity
    /// (`ROW_NUMBER`/`RANK`/`DENSE_RANK`/`PERCENT_RANK`/`CUME_DIST` exactly 0, `NTILE`/
    /// `FIRST_VALUE`/`LAST_VALUE` exactly 1, `LEAD`/`LAG` 1–3, `NTH_VALUE` exactly 2), and
    /// rejects the aggregate-only argument forms (`*`, a `DISTINCT`/`ALL` quantifier, an
    /// in-parenthesis `ORDER BY`, a `SEPARATOR`) — each violation an `ER_PARSE_ERROR`
    /// (1064) on mysql:8 (`ROW_NUMBER()` without `OVER`, `ROW_NUMBER(1) OVER ()`,
    /// `NTILE() OVER ()`, `RANK(DISTINCT a) OVER ()`). These are one indivisible dialect
    /// grammar, so they share the flag rather than a second knob that would have to
    /// co-vary with it. The parser enforces them in `parse_function_call`.
    pub over_requires_windowable_function: bool,
    /// Accept MySQL's window-function post-`)` tail — the SQL:2016
    /// `[FROM {FIRST | LAST}] [{RESPECT | IGNORE} NULLS]` clauses written *between* a
    /// null-treatment window function's argument `)` and its `OVER` clause, riding the
    /// shared [`FunctionCall`](crate::ast::FunctionCall)'s
    /// [`window_tail`](crate::ast::FunctionCall::window_tail) field. On for MySQL, off
    /// everywhere else. Engine-verified on mysql:8, the accepted surface is narrow: the
    /// null treatment is admitted only on `LEAD`/`LAG`/`FIRST_VALUE`/`LAST_VALUE`/
    /// `NTH_VALUE` and only as `RESPECT NULLS` (`IGNORE NULLS` grammar-admits but
    /// feature-rejects, `ER_NOT_SUPPORTED_YET` 1235); `FROM {FIRST | LAST}` is admitted
    /// only on `NTH_VALUE` and only as `FROM FIRST` (`FROM LAST` likewise 1235); the two
    /// clauses appear in that fixed order (the reverse is `ER_PARSE_ERROR`, 1064), and
    /// both sit strictly after the `)` (the in-paren spelling MySQL rejects, unlike
    /// DuckDB's [`null_treatment`](AggregateCallSyntax::null_treatment)). Keyed on a single
    /// *unquoted* window-function name — a quoted `` `nth_value` `` or qualified
    /// `db.nth_value` takes the general-call path (rejected there), matching the engine.
    /// When off, the tail keywords are left unconsumed and the trailing text surfaces as
    /// a clean parse error. The parser enforces the per-function admission in
    /// `parse_window_function_tail`.
    pub window_function_tail: bool,
    /// Accept a bare in-parenthesis `ORDER BY` as the sole content of a call's argument
    /// list — no positional argument preceding it (`rank(ORDER BY x) OVER w`,
    /// `cume_dist(ORDER BY x DESC) OVER w`). DuckDB lets a window/rank function carry its
    /// ordering inside the call parentheses instead of the `OVER (ORDER BY …)` clause; the
    /// [`order_by`](crate::ast::FunctionCall::order_by) list is the same field the
    /// arguments-then-`ORDER BY` form (`array_agg(x ORDER BY y)`) already fills, so the
    /// call shape is unchanged — only the empty *positional* list is new. On for
    /// DuckDb/Lenient, off elsewhere: standard SQL / PostgreSQL / MySQL require at least
    /// one argument before an aggregate `ORDER BY` (engine-verified), so when off the
    /// leading `ORDER` keyword falls into the argument-expression grammar where the
    /// reserved word surfaces as a clean parse error.
    ///
    /// A parse-level gate only. The per-function validity of the standalone form is a
    /// binding concern DuckDB enforces after parsing — `sum`/`array_agg`/`string_agg` and
    /// a bare `rank(ORDER BY x)` with no `OVER` are DuckDB *binder*/*catalog* rejects (the
    /// standalone form parses), so a parse-only parser correctly accepts them here; the one
    /// exception is DuckDB's parser-level "ORDER BY is not supported for the window function
    /// `dense_rank`", a fine-grained per-function restriction this gate deliberately does not
    /// model (see `duckdb-order-by-in-agg-args-trailing-comma`).
    pub standalone_argument_order_by: bool,
}

/// Dialect-owned predicate-form syntax accepted by the parser.
///
/// The dialect-gated predicate forms beyond the always-available comparison / `IS` / `IN`
/// baseline. They are predicates (non-chaining, comparison precedence), distinct from the
/// [`ExpressionSyntax`] postfix/constructor forms, and — unlike those — not all PostgreSQL
/// extensions: some are SQL Core surface on in every shipped dialect (`LIKE`, SQL-92 /
/// SQL:2016 **Core E021-08**), while the rest are gated per dialect. Each is a pure grammar
/// gate: when off, the keyword is left unconsumed and the trailing operand surfaces as a
/// clean parse error — the same reject mechanism the other syntax gates use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PredicateSyntax {
    /// Accept `<expr> IS [NOT] DISTINCT FROM <expr>`.
    pub is_distinct_from: bool,
    /// Accept `<expr> [NOT] LIKE <pattern> [ESCAPE <c>]` (SQL-92 core E021-08). On in
    /// every dialect — this is the standard pattern-match predicate.
    pub like: bool,
    /// Accept `<expr> [NOT] ILIKE <pattern> [ESCAPE <c>]` case-insensitive matching
    /// (PostgreSQL).
    pub ilike: bool,
    /// Accept `<expr> [NOT] SIMILAR TO <pattern> [ESCAPE <c>]` regex matching
    /// (SQL:1999 F841; PostgreSQL).
    pub similar_to: bool,
    /// Accept the SQL-standard `(s1, e1) OVERLAPS (s2, e2)` period predicate (SQL:2016
    /// F251) — the `row OVERLAPS row` form yielding a boolean, both operands
    /// exactly-two-element rows (a bare parenthesized pair or `ROW(...)`). A pure grammar
    /// gate: when off, the `OVERLAPS` keyword is left unconsumed and surfaces as a clean
    /// parse error. The parser enforces the two-element-row operand shape on both sides
    /// (a scalar, a single-element grouping `(a)`, or a three-element row is rejected,
    /// matching PostgreSQL's grammar-level wrong-arity error), so a value stored here only
    /// opens the predicate — validity of the operands is still checked. PostgreSQL-only
    /// among the shipped presets (DuckDB, MySQL, and SQLite all reject the form,
    /// engine-probed); the Lenient union carries it too.
    pub overlaps_period_predicate: bool,
    /// Accept DuckDB's unparenthesized `<expr> [NOT] IN <value>` list-membership operator
    /// ([`Expr::InExpr`](crate::ast::Expr::InExpr)) — `z IN y`, distinct from the standard
    /// parenthesized `IN (list)` / `IN (subquery)`. DuckDB-only: the right operand is a
    /// restricted `c_expr` that may not begin with a constant or unary sign (`IN 4` / `IN
    /// -5` are DuckDB parser errors), so the parser gates on the leading token. Off in
    /// every non-DuckDB preset (PostgreSQL and the standard require the parentheses).
    pub unparenthesized_in_list: bool,
    /// Accept a pattern-match predicate quantified over an array operand: `<expr>
    /// [NOT] LIKE|ILIKE {ANY | ALL | SOME} (<array>)`
    /// ([`Expr::QuantifiedLike`](crate::ast::Expr::QuantifiedLike)) — PostgreSQL's
    /// `ScalarArrayOpExpr` over the `~~`/`~~*` operator. `SIMILAR TO` has no
    /// quantified form (PostgreSQL rejects it, engine-probed), so only `LIKE`/`ILIKE`
    /// open it. A pure grammar gate: when off, the `ANY`/`ALL`/`SOME` head after a
    /// pattern operator is left unconsumed and surfaces as the usual reject at the
    /// reserved quantifier keyword. PostgreSQL-only among the shipped presets; the
    /// Lenient union carries it too.
    pub pattern_match_quantifier: bool,
    /// Accept the SQL-standard `SYMMETRIC`/`ASYMMETRIC` modifier on the range predicate:
    /// `<expr> [NOT] BETWEEN {SYMMETRIC | ASYMMETRIC} <low> AND <high>` (SQL:2016 T461).
    /// `SYMMETRIC` is load-bearing — it permits `low > high` by testing against the ordered
    /// pair — and is kept on [`Expr::Between`](crate::ast::Expr::Between)'s `symmetric` flag;
    /// the default `ASYMMETRIC` is a noise word dropped on parse. A pure grammar gate: when
    /// off, the modifier keyword after `BETWEEN` is left unconsumed and surfaces as a clean
    /// parse error. `SYMMETRIC` is an *optional* standard feature (T461), so it stays off in
    /// the strict ANSI baseline (and thus in MySQL/SQLite/ClickHouse/Snowflake, which reuse
    /// `PredicateSyntax::ANSI` and reject the modifier); on for PostgreSQL (engine-probed on
    /// pg_query) and the Lenient union.
    pub between_symmetric: bool,
    /// Accept the SQL-standard Unicode-normalization test `<expr> IS [NOT]
    /// [NFC|NFD|NFKC|NFKD] NORMALIZED` (SQL:2016 T061), parsed to the postfix
    /// [`Expr::IsNormalized`](crate::ast::Expr::IsNormalized) predicate. A pure grammar gate:
    /// when off, the `NORMALIZED` continuation after `IS [NOT]` is left unconsumed and the
    /// null/truth reading rejects it. An optional standard feature, so off in the strict ANSI
    /// baseline (and thus in MySQL/SQLite/ClickHouse/Snowflake, which reuse
    /// `PredicateSyntax::ANSI` and reject it); on for PostgreSQL (engine-probed on pg_query)
    /// and the Lenient union.
    pub is_normalized: bool,
    /// Accept an empty parenthesized `IN` list — `<expr> [NOT] IN ()` — with no elements,
    /// parsed to an [`Expr::InList`](crate::ast::Expr::InList) whose `list` is empty. SQLite
    /// evaluates `x IN ()` to false and `x NOT IN ()` to true (engine-measured via rusqlite
    /// 3.53.2, where both `prepare`-accept). A pure grammar gate: when off, the closing `)` in
    /// list position is left unconsumed and the required-first-element reject stands — the
    /// standard `IN` predicate demands at least one element, so ANSI/PostgreSQL/MySQL/DuckDB
    /// syntax-reject the empty list. On for SQLite and the Lenient union, off elsewhere.
    pub empty_in_list: bool,
    /// Accept the two-word postfix null test `<expr> NOT NULL` (a synonym for `IS NOT NULL`),
    /// folded onto [`Expr::IsNull`](crate::ast::Expr::IsNull) with `negated: true` and a
    /// [`NullTestSpelling::PostfixNotNull`](crate::ast::NullTestSpelling) tag so it round-trips.
    /// A `NOT`-led predicate spelling, parsed at comparison precedence alongside the other
    /// `NOT`-led predicates (`NOT IN`/`NOT LIKE`/`NOT BETWEEN`); when off, the `NOT NULL` run is
    /// left unconsumed and the `NOT` surfaces as the ordinary prefix operator (or a clean parse
    /// error), so the two-word form is rejected.
    ///
    /// Distinct from the one-word [`OperatorSyntax::null_test_postfix`](OperatorSyntax) gate
    /// because the surfaces diverge (engine-measured): SQLite and DuckDB accept both spellings,
    /// but PostgreSQL — despite accepting the one-word `ISNULL`/`NOTNULL` — rejects the two-word
    /// `NOT NULL` postfix. On for SQLite and the Lenient union; off elsewhere (including
    /// PostgreSQL; DuckDB's acceptance is tracked separately).
    pub null_test_two_word_postfix: bool,
}

/// A frozen SQL-standard edition, used to anchor feature availability to a release
/// without enumerating features (ZetaSQL's `LanguageVersion` checkpoints).
///
/// These are the feature-ID-era editions: the `Ennn`/`Fnnn`/`Tnnn` feature taxonomy
/// was introduced in SQL:1999, so it is the earliest anchor. Declaration order is
/// chronological, so `Ord` reads as "released no later than".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StandardVersion {
    /// SQL:1999 — first edition with the named-feature taxonomy.
    Sql1999,
    /// SQL:2003 — adds windowed table functions (OLAP), MERGE, sequences.
    Sql2003,
    /// SQL:2008.
    Sql2008,
    /// SQL:2011.
    Sql2011,
    /// SQL:2016 — the edition "generic"/ANSI is anchored on.
    Sql2016,
}

/// Small, typed dialect data consumed by parser and renderer code.
///
/// # Preset permissiveness spectrum
///
/// The shipped presets run from the strict standard to a parse-anything union; a
/// caller picks by how much non-standard SQL it must accept. The `squonk` crate's
/// `BuiltinDialect::ALL` is the authoritative selectable list — this doc groups those
/// presets by how they are held honest:
///
/// - [`ANSI`](Self::ANSI) — the strict SQL:2016 standard baseline: the
///   principled neutral default, accepting only standard surface.
/// - `POSTGRES` / `MYSQL` / `SQLITE` / `DUCKDB` — one real dialect's surface, strict in
///   its own idiom (each gated behind its cargo feature). These, with `ANSI`, are the
///   oracle-compared presets: each is held to its real engine by a differential
///   accept/reject oracle, so over-acceptance is a measured, gated zero.
/// - `LENIENT` — the permissive "parse anything" catch-all: the documented maximal
///   union tooling reaches for on SQL of unknown origin (gated behind the `lenient`
///   feature).
/// - The conservative, no-oracle presets — `BIGQUERY`, `HIVE`, `CLICKHOUSE`, `DATABRICKS`,
///   `MSSQL`, `SNOWFLAKE`, `REDSHIFT` (each behind its cargo feature) — derive from `ANSI`
///   and enable only the
///   surface that already has a modelled, tested parser gate and documentary evidence
///   (their `ON`s are evidence-cited, not oracle-proven). This workspace ships no oracle
///   for these engines, so their over-acceptance cannot be measured; conservatism is the
///   honesty bar, and they are deliberately excluded from the oracle conformance sets —
///   unsupported syntax is a clean reject routed to a follow-up ticket, never a silent
///   over-accept. Each preset's module doc spells out exactly what it adds over `ANSI`.
///
/// There is deliberately **no** permissive "generic" preset (a vibe-union is
/// banned): `ANSI` *is* the generic/standard baseline, and `LENIENT` is the
/// honest, spelled-out permissive end. This differs from `datafusion-sqlparser-rs`,
/// whose `GenericDialect` is a permissive catch-all — the equivalent here is
/// `LENIENT`, not the standard baseline. The runtime name `"generic"` aliases `ANSI`;
/// see `BuiltinDialect::from_name` in the `squonk` crate for the migration mapping.
///
/// # Shared byte-trigger ownership: `#`
///
/// A single-meaning byte folds into one meaning-enum (the `^` axis is
/// [`caret_operator`](Self::caret_operator): exactly one of exponent / bitwise-XOR / none
/// per dialect). The `#` byte is **not** such a byte, and this is why its claimants stay
/// separate flags on their own sub-structs rather than a single `HashMeaning` enum: `#` is
/// a shared *lead* byte whose readings **coexist** in one dialect, partitioned by scan
/// phase and follow byte, not mutually exclusive. PostgreSQL enables bare-`#` XOR **and**
/// the `#>`/`#>>`/`#-` jsonb operators at once; a single-valued enum could not represent
/// both. Its five claimants, in resolution order:
///
/// 1. `#` line comment ([`CommentSyntax::line_comment_hash`], MySQL) — consumed as trivia
///    before tokenizing, so when on it shadows every reading below.
/// 2. `#`+identifier-start word (a [`byte_classes`](Self::byte_classes) table marking `#`
///    [`CLASS_IDENTIFIER_START`](lex_class::CLASS_IDENTIFIER_START), T-SQL `#temp`) — the
///    identifier scan precedes the operator/positional arms, so it wins on an identifier
///    follow byte.
/// 3. `#`+digit positional column ([`ExpressionSyntax::positional_column`], DuckDB `#1`).
/// 4. `#`+`>`/`-` jsonb path operators ([`OperatorSyntax::jsonb_operators`], PostgreSQL) —
///    maximal-munched ahead of a bare `#`, so disjoint from the readings below by follow
///    byte; reaches the operator scanner only when `#` is routed there by claimant 5.
/// 5. bare `#` bitwise-XOR operator ([`hash_bitwise_xor`](Self::hash_bitwise_xor),
///    PostgreSQL) — the fallthrough when no earlier partition claims the byte.
///
/// Claimants 3/4/5 partition by follow byte and coexist freely; the *genuine* collisions
/// are only where a claimant is silently shadowed — the trivia phase (1 vs 2/3/5) and the
/// scan-order overlap (positional 3 vs bare-XOR 5). Those are the tracked pairwise
/// [`LexicalConflict`]s ([`HashCommentVersusHashIdentifier`](LexicalConflict::HashCommentVersusHashIdentifier),
/// [`HashXorOperatorVersusHashComment`](LexicalConflict::HashXorOperatorVersusHashComment),
/// [`HashXorOperatorVersusPositionalColumn`](LexicalConflict::HashXorOperatorVersusPositionalColumn),
/// [`HashCommentVersusPositionalColumn`](LexicalConflict::HashCommentVersusPositionalColumn)),
/// asserted absent on every shipped preset. That registry — one flag per behaviour, plus a
/// conflict entry per silently-shadowing pair — is the correct model for a coexisting-lead
/// byte; a `HashMeaning` meaning-enum would falsely impose mutual exclusion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureSet {
    /// Identity fold for unquoted identifiers; exact text remains interned.
    pub identifier_casing: Casing,
    /// Identifier quote styles the dialect accepts (one or more; symmetric or
    /// asymmetric). The tokenizer matches an opening delimiter against this set.
    pub identifier_quotes: &'static [IdentifierQuote],
    /// Dialect default when `NULLS FIRST`/`NULLS LAST` is omitted.
    pub default_null_ordering: NullOrdering,
    /// Keywords rejected as an unquoted column/table name or `ColId` alias
    /// (PostgreSQL `type_func_name ∪ reserved`). Per-position reservation
    /// (prod-keyword-position-reserved-sets): a keyword's admissibility depends on
    /// the grammatical position, so the single reserved set is replaced by four.
    pub reserved_column_name: KeywordSet,
    /// Keywords rejected as a function name (PostgreSQL `reserved`).
    pub reserved_function_name: KeywordSet,
    /// Keywords rejected as a (user-defined) type name (PostgreSQL
    /// `col_name ∪ reserved`).
    pub reserved_type_name: KeywordSet,
    /// Keywords rejected as a bare column alias — one written without `AS`
    /// (PostgreSQL `AS_LABEL`). `AS`-introduced aliases (`ColLabel`) admit every
    /// keyword, so they need no reject set.
    pub reserved_bare_alias: KeywordSet,
    /// Keywords rejected in `ColLabel` position — an `AS`-introduced alias
    /// (`SELECT 1 AS <label>`) and a dotted-name continuation part (`schema.<part>`,
    /// `x.<part>`). PostgreSQL admits *every* keyword there (`SELECT a AS select` is
    /// valid), so its set is [`KeywordSet::EMPTY`]; the same holds for every
    /// PostgreSQL-family preset. SQLite draws no `ColId`/`ColLabel` split — a reserved
    /// word is rejected in every name position uniformly — so it reuses its single
    /// reserved set here, making `SELECT 1 AS delete` / `SELECT x.update` /
    /// `FROM schema.case` the parse errors SQLite reports (engine-measured via rusqlite).
    pub reserved_as_label: KeywordSet,
    /// Whether a relation (table / index / view) name admits a *catalog* qualifier —
    /// the third dotted part, `catalog.schema.table`. On for ANSI/PostgreSQL/MySQL/
    /// DuckDB/Lenient, which cap a relation at three parts (a fourth is rejected). Off
    /// for SQLite, whose relation names are `schema.table` at most (a database *is* the
    /// schema namespace), so a three-part `a.b.c` in table/index position is the syntax
    /// error SQLite reports (engine-measured via rusqlite). Column references reach one
    /// part deeper through composite-field selection — a different grammar position —
    /// and are never bounded by this flag.
    pub catalog_qualified_names: bool,
    /// Byte classes used by tokenizer dispatch.
    pub byte_classes: ByteClasses,
    /// Binary and prefix operator binding powers used by parser and renderer.
    pub binding_powers: BindingPowerTable,
    /// Set-operation binding powers used by parser and renderer.
    pub set_operation_powers: SetOperationBindingPowerTable,
    /// String literal syntax extensions accepted by the tokenizer/parser.
    pub string_literals: StringLiteralSyntax,
    /// Numeric literal syntax extensions accepted by the tokenizer.
    pub numeric_literals: NumericLiteralSyntax,
    /// Prepared-statement parameter placeholder forms accepted by the tokenizer.
    pub parameters: ParameterSyntax,
    /// MySQL session-variable forms (`@name` user variables, `@@[scope.]name` system
    /// variables) accepted by the tokenizer.
    pub session_variables: SessionVariableSyntax,
    /// Unquoted-identifier character policy (the dialect-variable `$`) accepted by the
    /// tokenizer.
    pub identifier_syntax: IdentifierSyntax,
    /// Table-expression syntax forms accepted by the parser.
    pub table_expressions: TableExpressionSyntax,
    /// Join-operator and recursive-query relation-composition syntax, gathered from
    /// [`table_expressions`](Self::table_expressions).
    pub join_syntax: JoinSyntax,
    /// Table-factor `FROM`-item forms beyond a plain named table, gathered from
    /// [`table_expressions`](Self::table_expressions).
    pub table_factor_syntax: TableFactorSyntax,
    /// Expression postfix and constructor forms (typecast, subscript, COLLATE, AT TIME
    /// ZONE, array/row constructors, field selection, typed literals) accepted by the
    /// parser. The operator spellings and call-tail forms are separate dimensions —
    /// see [`operator_syntax`](Self::operator_syntax) and [`call_syntax`](Self::call_syntax).
    pub expression_syntax: ExpressionSyntax,
    /// Infix/prefix operator forms (`OPERATOR(…)`, `@>`/`<@`, `->`/`->>`, `==`, general
    /// `IS`, `<=>`, the DuckDB lambda, the bitwise family, quantified comparisons)
    /// accepted by the parser.
    pub operator_syntax: OperatorSyntax,
    /// Function-call forms accepted by the parser (named arguments, cast/convert shape,
    /// `UTC_*`/`COLUMNS(…)`/`EXTRACT(… FROM …)` special calls, …).
    /// Aggregate tails (`SEPARATOR`/`WITHIN GROUP`/`FILTER`/`OVER` eligibility) live in
    /// [`aggregate_call_syntax`](Self::aggregate_call_syntax); keyword string specials live
    /// in [`string_func_forms`](Self::string_func_forms).
    pub call_syntax: CallSyntax,
    /// Keyword string/scalar special-form syntax — the flags whose sole AST product
    /// is a [`StringFunc`](crate::ast::StringFunc).
    pub string_func_forms: StringFuncForms,
    /// Aggregate/window function-call syntax — the in-parenthesis and post-`)` tails,
    /// the argument-shape/arity restrictions, and the `OVER`-eligibility gate.
    pub aggregate_call_syntax: AggregateCallSyntax,
    /// Pattern-match predicate forms (`LIKE`/`ILIKE`/`SIMILAR TO`) accepted by the
    /// parser. `LIKE` is SQL core (on everywhere); `ILIKE`/`SIMILAR TO` are gated.
    pub predicate_syntax: PredicateSyntax,
    /// What the `||` operator token means: string concatenation or logical OR.
    pub pipe_operator: PipeOperator,
    /// What the `&&` operator token means: logical AND, or not an operator.
    pub double_ampersand: DoubleAmpersand,
    /// Which dialect's keyword infix operators are recognized (MySQL's
    /// `DIV`/`MOD`/`XOR`/`RLIKE`/`REGEXP`, SQLite's `GLOB`/`MATCH`/`REGEXP`, or none) —
    /// each variant names one dialect's exact set; see [`KeywordOperators`].
    pub keyword_operators: KeywordOperators,
    /// What the `^` operator token means: arithmetic exponentiation, bitwise XOR, or no
    /// infix meaning. The `^` byte always lexes; this is the sole owner of its infix
    /// reading — the former split across an `exponent_operator` bool and a `Caret`-XOR
    /// spelling is folded here, so the "both power and XOR" state is unrepresentable. See
    /// [`CaretOperator`].
    pub caret_operator: CaretOperator,
    /// Whether a bare `#` lexes and parses as the bitwise-XOR operator
    /// ([`BinaryOperator::BitwiseXor`] under the [`Hash`](BitwiseXorSpelling::Hash)
    /// spelling, PostgreSQL). A different byte from [`caret_operator`](Self::caret_operator)'s
    /// `^`: PostgreSQL spells XOR `#`, MySQL spells it `^`, and neither accepts the other's
    /// spelling. Also gates the tokenizer — `#` reaches the operator scanner only under this
    /// flag — so it is one of the `#`-trigger claimants tracked in [`LexicalConflict`]
    /// (against a `#` line comment and DuckDB's `#n` positional column). Off for every
    /// shipped dialect but PostgreSQL.
    pub hash_bitwise_xor: bool,
    /// Dialect comment syntax extensions accepted by the tokenizer.
    pub comment_syntax: CommentSyntax,
    /// Mutation-statement (`INSERT`/`UPDATE`/`DELETE`) syntax forms accepted by the
    /// parser (`RETURNING`, `ON CONFLICT`).
    pub mutation_syntax: MutationSyntax,
    /// Whole-statement DDL dispatch gates (non-`TABLE` `CREATE`/`DROP` object dispatch),
    /// split from the retired `SchemaChangeSyntax`.
    pub statement_ddl_gates: StatementDdlGates,
    /// View/sequence clause syntax after a DDL statement head has been chosen —
    /// temporary/recursive views, view options, matview targets, sequence `CACHE`.
    pub view_sequence_clause_syntax: ViewSequenceClauseSyntax,
    /// `CREATE TABLE` table-level clause syntax (storage/CTAS/partitioning/inheritance/
    /// persistence clauses), split from the retired `SchemaChangeSyntax`.
    pub create_table_clause_syntax: CreateTableClauseSyntax,
    /// `CREATE TABLE` column-definition syntax (generated/identity columns, SQLite column
    /// attributes, `DEFAULT`/`COLLATE`/`STORAGE` clauses), split from `SchemaChangeSyntax`.
    pub column_definition_syntax: ColumnDefinitionSyntax,
    /// Table/column-constraint syntax (deferrable/named/bare constraints, `EXCLUDE`, the
    /// `NO INHERIT`/`NOT VALID` markers, index parameters), split from `SchemaChangeSyntax`.
    pub constraint_syntax: ConstraintSyntax,
    /// `CREATE INDEX`/`ALTER TABLE`/`DROP` syntax (index clauses, the extended `ALTER`
    /// surface, drop behaviour, routine arg types), split from `SchemaChangeSyntax`.
    pub index_alter_syntax: IndexAlterSyntax,
    /// `IF [NOT] EXISTS` existence guards on DDL statements (`DROP`/`ALTER IF EXISTS`,
    /// `CREATE VIEW`/`CREATE DATABASE IF NOT EXISTS`), gathered from the schema-change
    /// surface into one per-site table.
    pub existence_guards: ExistenceGuards,
    /// SELECT-body syntax forms accepted by the parser (`DISTINCT ON`, `QUALIFY`,
    /// projection aliases, set-op quantifiers, …). Row-limiting / locking / pipe tails
    /// live in [`query_tail_syntax`](Self::query_tail_syntax); `GROUP BY`/`ORDER BY`
    /// modes live in [`grouping_syntax`](Self::grouping_syntax).
    pub select_syntax: SelectSyntax,
    /// Query-tail syntax — the row-limiting/locking family and the trailing
    /// ClickHouse/pipe clauses parsed after the SELECT body.
    pub query_tail_syntax: QueryTailSyntax,
    /// `GROUP BY`/`ORDER BY` grouping-and-ordering syntax (grouping sets, clause
    /// modes, and quantifiers).
    pub grouping_syntax: GroupingSyntax,
    /// Utility-statement syntax forms the parser dispatches: the
    /// PostgreSQL `COPY`/`COMMENT ON` and SQLite `PRAGMA`/`ATTACH`/`DETACH`
    /// statement gates.
    pub utility_syntax: UtilitySyntax,
    /// Transaction-control (TCL) syntax — openers, completers, savepoints, mode lists,
    /// and XA forms — split from [`utility_syntax`](Self::utility_syntax).
    pub transaction_syntax: TransactionSyntax,
    /// SHOW/DESCRIBE introspection-statement syntax (the session `SHOW` reader, the typed
    /// `SHOW <object>` listings, and `DESCRIBE`/`SUMMARIZE`), gathered from
    /// [`utility_syntax`](Self::utility_syntax).
    pub show_syntax: ShowSyntax,
    /// Physical-maintenance-statement syntax (`VACUUM`/`REINDEX`/`ANALYZE`/`CHECKPOINT`),
    /// gathered from [`utility_syntax`](Self::utility_syntax).
    pub maintenance_syntax: MaintenanceSyntax,
    /// Access-control-statement syntax (`GRANT`/`REVOKE` and the extended object grammar),
    /// gathered from [`utility_syntax`](Self::utility_syntax).
    pub access_control_syntax: AccessControlSyntax,
    /// Type-name vocabulary the parser recognizes beyond the shared standard set
    /// (the MySQL `TINYINT`/`ENUM`/`UNSIGNED`/… surface).
    pub type_name_syntax: TypeNameSyntax,
    /// Which dialect's canonical surface spelling a [`RenderSpelling::TargetDialect`]
    /// render emits for constructs whose spelling diverges across dialects (today the
    /// type names). Read by the renderer in place of recognizing a preset by identity,
    /// so PostgreSQL-vs-ANSI output spelling is pure dialect data, not a cfg-gated
    /// `FeatureSet::POSTGRES` comparison.
    ///
    /// [`RenderSpelling::TargetDialect`]: crate::render::RenderSpelling::TargetDialect
    pub target_spelling: TargetSpelling,
}

impl FeatureSet {
    /// The ANSI/standard config baseline as of SQL edition `version`.
    ///
    /// The M1 dialect-data surface (identifier quoting, casing, concatenation, set
    /// operations, …) is invariant across feature-ID-era editions — every standard
    /// feature we implement is SQL:1999 Core — so this returns [`FeatureSet::ANSI`]
    /// for every edition today. It is the stable anchor consumers pin against; the
    /// exhaustive `match` forces a decision here once edition-varying dialect data
    /// is added. For the set of *standard features* available as of an edition
    /// (which does vary), query [`standard_features_as_of`].
    ///
    /// Per-dialect *release* pinning (e.g. PostgreSQL 14 vs 16) is a distinct axis
    /// that needs multi-version dialect data; it arrives with the later milestones.
    pub const fn as_of(version: StandardVersion) -> Self {
        match version {
            StandardVersion::Sql1999
            | StandardVersion::Sql2003
            | StandardVersion::Sql2008
            | StandardVersion::Sql2011
            | StandardVersion::Sql2016 => Self::ANSI,
        }
    }

    /// Return all lexical classes for `byte` under this dialect.
    pub const fn byte_class(&self, byte: u8) -> u8 {
        self.byte_classes.byte_class(byte)
    }

    /// Return true if `byte` has any lexical class in `mask` under this dialect.
    pub const fn has_byte_class(&self, byte: u8, mask: u8) -> bool {
        self.byte_classes.has_class(byte, mask)
    }

    /// Return the binary binding power for `op` under this dialect.
    pub const fn binding_power(&self, op: &BinaryOperator) -> BindingPower {
        self.binding_powers.binary(op)
    }

    /// Return the prefix binding power for `op` under this dialect.
    pub const fn prefix_binding_power(&self, op: &UnaryOperator) -> u8 {
        self.binding_powers.prefix(op)
    }

    /// Return the set-operation binding power for `op` under this dialect.
    pub const fn set_operation_binding_power(&self, op: &SetOperator) -> BindingPower {
        self.set_operation_powers.set_operation(op)
    }

    /// Fold an unquoted identifier according to this dialect's identity rules.
    pub fn fold_unquoted_identifier<'a>(&self, identifier: &'a str) -> Cow<'a, str> {
        self.identifier_casing.fold_identifier(identifier)
    }

    /// Whether omitted `NULLS FIRST`/`NULLS LAST` sorts nulls first.
    pub const fn default_nulls_first(&self) -> bool {
        self.default_null_ordering.nulls_first()
    }
}

fn fold_identifier_upper(identifier: &str) -> Cow<'_, str> {
    if identifier.bytes().any(|byte| byte.is_ascii_lowercase()) {
        Cow::Owned(identifier.to_ascii_uppercase())
    } else {
        Cow::Borrowed(identifier)
    }
}

fn fold_identifier_lower(identifier: &str) -> Cow<'_, str> {
    if identifier.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(identifier.to_ascii_lowercase())
    } else {
        Cow::Borrowed(identifier)
    }
}

// `FeatureDelta`, `FeatureSet::with`, and the
// `Feature` / `FeatureMetadata` registry below are the per-field boilerplate that
// drifted each time a `FeatureSet` dimension was added (the delta mirror, the
// builder setters, and the enum/`id`/`ALL`/metadata registry). The thin include
// below pulls in that generated module: it is derived from the `FeatureSet` struct
// plus an annotation table (ADR-0013 drift gate; ADR-0011 self-describing dialect
// data) and re-exported here, so the public dialect API is unchanged. To add a
// dimension: add the struct field above and its `FEATURE_FIELDS` annotation in
// `crates/squonk-sourcegen/src/feature_set.rs`, then run
// `cargo run -p squonk-sourcegen`.
mod feature_set_generated;
pub use feature_set_generated::{FEATURE_METADATA, FEATURES, Feature, FeatureDelta};

mod conflict;
mod head_contention;
mod standard_catalog;
mod support_tier;

pub use conflict::{FeatureDependencyViolation, GrammarConflict, LexicalConflict};
pub use head_contention::{
    BASE_VS_FEATURE_STATEMENT_HEADS, BaseVsFeatureStatementHead, FeatureGatePredicate,
    HeadResolution, MULTI_CLAIMANT_STATEMENT_HEADS, MultiClaimantHead, NamedFeatureGate,
};
pub use standard_catalog::{
    Conformance, FeatureMetadata, Maturity, STANDARD_FEATURE_CATALOG, StandardFeature,
    max_feature_metadata, standard_feature, standard_features_as_of, unsupported_standard_features,
};
pub use support_tier::{SupportEvidence, SupportTier};

#[cfg(test)]
mod tests {
    use super::*;
    use lex_class::{
        CLASS_DIGIT, CLASS_IDENTIFIER_CONTINUE, CLASS_IDENTIFIER_START, CLASS_OPERATOR,
        CLASS_PUNCTUATION, CLASS_WHITESPACE, CLASS_WHITESPACE_BOUNDARY, CLASS_WHITESPACE_CONTINUE,
        byte_class, has_class,
    };

    #[test]
    fn ansi_and_postgres_differ_in_identifier_casing() {
        assert_eq!(FeatureSet::ANSI.identifier_casing, Casing::Upper);
        assert_eq!(FeatureSet::POSTGRES.identifier_casing, Casing::Lower);
        assert_ne!(
            FeatureSet::ANSI.identifier_casing,
            FeatureSet::POSTGRES.identifier_casing
        );
    }

    #[test]
    fn identifier_casing_folds_unquoted_identifier_identity() {
        assert_eq!(FeatureSet::ANSI.fold_unquoted_identifier("MiXeD"), "MIXED");
        assert_eq!(
            FeatureSet::POSTGRES.fold_unquoted_identifier("MiXeD"),
            "mixed",
        );
        assert_eq!(
            FeatureSet::ANSI
                .with(FeatureDelta::EMPTY.identifier_casing(Casing::Preserve))
                .fold_unquoted_identifier("MiXeD"),
            "MiXeD",
        );
        assert_eq!(
            FeatureSet::ANSI.fold_unquoted_identifier("CAFE"),
            "CAFE",
            "already-folded ASCII identifiers stay borrowed-equivalent",
        );
    }

    #[test]
    fn default_null_ordering_exposes_sort_behavior() {
        assert!(!FeatureSet::ANSI.default_nulls_first());
        assert!(
            FeatureSet::ANSI
                .with(FeatureDelta::EMPTY.default_null_ordering(NullOrdering::NullsFirst))
                .default_nulls_first()
        );
    }

    #[test]
    fn delta_customizes_from_base_preset() {
        let custom = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .identifier_casing(Casing::Preserve)
                .default_null_ordering(NullOrdering::NullsFirst),
        );

        assert_eq!(custom.identifier_casing, Casing::Preserve);
        assert_eq!(custom.default_null_ordering, NullOrdering::NullsFirst);
        assert_eq!(custom.identifier_quotes, FeatureSet::ANSI.identifier_quotes);
        assert_eq!(
            custom.reserved_column_name,
            FeatureSet::ANSI.reserved_column_name
        );
        assert_eq!(
            custom.reserved_bare_alias,
            FeatureSet::ANSI.reserved_bare_alias
        );
        assert_eq!(custom.byte_classes, FeatureSet::ANSI.byte_classes);
        assert_eq!(custom.binding_powers, FeatureSet::ANSI.binding_powers);
        assert_eq!(
            custom.set_operation_powers,
            FeatureSet::ANSI.set_operation_powers
        );
        assert_eq!(custom.string_literals, FeatureSet::ANSI.string_literals);
        assert_eq!(custom.numeric_literals, FeatureSet::ANSI.numeric_literals);
        assert_eq!(custom.parameters, FeatureSet::ANSI.parameters);
        assert_eq!(custom.table_expressions, FeatureSet::ANSI.table_expressions);
        assert_eq!(custom.double_ampersand, FeatureSet::ANSI.double_ampersand);
        assert_eq!(custom.comment_syntax, FeatureSet::ANSI.comment_syntax);
        assert_eq!(custom.pipe_operator, FeatureSet::ANSI.pipe_operator);
    }

    #[test]
    fn identifier_quote_styles_expose_open_and_close() {
        assert_eq!(
            FeatureSet::ANSI.identifier_quotes,
            STANDARD_IDENTIFIER_QUOTES
        );
        assert_eq!(
            FeatureSet::POSTGRES.identifier_quotes,
            STANDARD_IDENTIFIER_QUOTES
        );

        let double = IdentifierQuote::Symmetric('"');
        assert_eq!((double.open(), double.close()), ('"', '"'));

        let bracket = IdentifierQuote::Asymmetric {
            open: '[',
            close: ']',
        };
        assert_eq!((bracket.open(), bracket.close()), ('[', ']'));
    }

    #[test]
    fn string_literal_syntax_is_explicit_dialect_data() {
        assert_eq!(FeatureSet::ANSI.string_literals, StringLiteralSyntax::ANSI);
        assert_eq!(
            FeatureSet::POSTGRES.string_literals,
            StringLiteralSyntax::POSTGRES,
        );
        assert_ne!(
            FeatureSet::ANSI.string_literals,
            FeatureSet::POSTGRES.string_literals,
        );
    }

    #[test]
    fn numeric_literal_syntax_is_explicit_dialect_data() {
        // The radix/separator forms are off in both baselines (PostgreSQL gates
        // them by release), so the dimension is exercised via an explicit delta.
        assert_eq!(
            FeatureSet::ANSI.numeric_literals,
            NumericLiteralSyntax::ANSI
        );
        let hex =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.numeric_literals(NumericLiteralSyntax {
                hex_integers: true,
                ..NumericLiteralSyntax::ANSI
            }));
        assert!(hex.numeric_literals.hex_integers);
        assert_ne!(hex.numeric_literals, FeatureSet::ANSI.numeric_literals);
    }

    #[test]
    fn parameter_syntax_is_explicit_dialect_data() {
        // ANSI recognises no placeholder forms; PostgreSQL enables positional `$n`
        // but not `?`. Bind to a local so the field checks are not const-folded into
        // a clippy `assertions_on_constants` lint.
        assert_eq!(FeatureSet::ANSI.parameters, ParameterSyntax::ANSI);
        assert_eq!(FeatureSet::POSTGRES.parameters, ParameterSyntax::POSTGRES);
        let postgres = FeatureSet::POSTGRES.parameters;
        assert!(postgres.positional_dollar);
        assert!(!postgres.anonymous_question);
        assert_ne!(FeatureSet::ANSI.parameters, FeatureSet::POSTGRES.parameters);

        let anon = FeatureSet::ANSI.with(FeatureDelta::EMPTY.parameters(ParameterSyntax {
            anonymous_question: true,
            ..ParameterSyntax::ANSI
        }));
        assert!(anon.parameters.anonymous_question);
        assert_ne!(anon.parameters, FeatureSet::ANSI.parameters);
    }

    #[test]
    fn identifier_syntax_is_explicit_dialect_data() {
        // The `$`-in-identifier policy is the dialect-variable part of the identifier
        // rule: ANSI forbids `$`, PostgreSQL accepts it mid-identifier. Bind to locals
        // so the field checks are not const-folded into a clippy
        // `assertions_on_constants` lint.
        assert_eq!(FeatureSet::ANSI.identifier_syntax, IdentifierSyntax::ANSI);
        assert_eq!(
            FeatureSet::POSTGRES.identifier_syntax,
            IdentifierSyntax::POSTGRES,
        );
        let ansi = FeatureSet::ANSI.identifier_syntax;
        let postgres = FeatureSet::POSTGRES.identifier_syntax;
        assert!(!ansi.dollar_in_identifiers);
        assert!(postgres.dollar_in_identifiers);
        assert_ne!(ansi, postgres);

        // The SQLite string-literal identifier misfeature is SQLite/Lenient-only; every
        // other shipped preset syntax-rejects a string in a name position, so it stays off.
        // Bound to locals so the check is a runtime `assert!`, not the `assertions_on_constants`
        // lint's const-value form.
        let sqlite = FeatureSet::SQLITE.identifier_syntax;
        let lenient = FeatureSet::LENIENT.identifier_syntax;
        let mysql = FeatureSet::MYSQL.identifier_syntax;
        assert!(sqlite.string_literal_identifiers);
        assert!(lenient.string_literal_identifiers);
        assert!(!ansi.string_literal_identifiers);
        assert!(!postgres.string_literal_identifiers);
        assert!(!mysql.string_literal_identifiers);

        // The policy toggles independently of the base preset.
        let dollars =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.identifier_syntax(IdentifierSyntax {
                dollar_in_identifiers: true,
                ..IdentifierSyntax::ANSI
            }));
        assert!(dollars.identifier_syntax.dollar_in_identifiers);
        assert_ne!(
            dollars.identifier_syntax,
            FeatureSet::ANSI.identifier_syntax
        );
    }

    #[test]
    fn table_expression_syntax_is_explicit_dialect_data() {
        assert_eq!(
            FeatureSet::ANSI.table_expressions,
            TableExpressionSyntax::ANSI
        );
        assert_eq!(
            FeatureSet::POSTGRES.table_expressions,
            TableExpressionSyntax::POSTGRES
        );
        assert_ne!(
            FeatureSet::ANSI.table_expressions,
            FeatureSet::POSTGRES.table_expressions
        );
    }

    #[test]
    fn pipe_operator_is_explicit_dialect_data() {
        assert_eq!(FeatureSet::ANSI.pipe_operator, PipeOperator::StringConcat);
        assert_eq!(
            FeatureSet::POSTGRES.pipe_operator,
            PipeOperator::StringConcat
        );

        let mysql_like =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.pipe_operator(PipeOperator::LogicalOr));
        assert_eq!(mysql_like.pipe_operator, PipeOperator::LogicalOr);

        // `||` maps to one canonical operator; `OR`'s looser binding power follows.
        assert_eq!(
            PipeOperator::StringConcat.binary_operator(),
            BinaryOperator::StringConcat,
        );
        assert_eq!(
            PipeOperator::LogicalOr.binary_operator(),
            BinaryOperator::Or
        );
    }

    #[test]
    fn double_ampersand_is_explicit_dialect_data() {
        // ANSI/PostgreSQL do not treat `&&` as a scalar operator.
        assert_eq!(
            FeatureSet::ANSI.double_ampersand,
            DoubleAmpersand::Unsupported
        );
        assert_eq!(DoubleAmpersand::Unsupported.binary_operator(), None);

        // A MySQL-like dialect maps `&&` to the canonical `AND` operator, so its
        // `AND` binding power follows automatically (no precedence override).
        let mysql_like = FeatureSet::ANSI
            .with(FeatureDelta::EMPTY.double_ampersand(DoubleAmpersand::LogicalAnd));
        assert_eq!(mysql_like.double_ampersand, DoubleAmpersand::LogicalAnd);
        assert_eq!(
            DoubleAmpersand::LogicalAnd.binary_operator(),
            Some(BinaryOperator::And),
        );
    }

    #[test]
    fn comment_syntax_is_explicit_dialect_data() {
        assert_eq!(FeatureSet::ANSI.comment_syntax, CommentSyntax::ANSI);

        let mysql_like = FeatureSet::ANSI.with(FeatureDelta::EMPTY.comment_syntax(CommentSyntax {
            line_comment_hash: true,
            ..CommentSyntax::ANSI
        }));
        assert!(mysql_like.comment_syntax.line_comment_hash);
        assert_ne!(mysql_like.comment_syntax, FeatureSet::ANSI.comment_syntax);

        // The MySQL comment shape is data, not tokenizer hard-coding: block comments
        // stop nesting and `/*!…*/` becomes conditional inclusion, while the ANSI
        // baseline keeps the permissive nesting and no versioned form.
        let ansi = FeatureSet::ANSI.comment_syntax;
        let mysql = FeatureSet::MYSQL.comment_syntax;
        assert!(ansi.nested_block_comments);
        assert_eq!(ansi.versioned_comments, None);
        assert!(!mysql.nested_block_comments);
        assert_eq!(
            mysql.versioned_comments,
            Some(CommentSyntax::MYSQL_8_VERSION_BOUND)
        );
    }

    #[test]
    fn select_syntax_is_explicit_dialect_data() {
        // `DISTINCT ON` is PostgreSQL-only; the fetch-first row-limiting spelling is
        // standard and on in both presets. Bind to locals so the field checks are not
        // const-folded into a clippy `assertions_on_constants` lint.
        assert_eq!(FeatureSet::ANSI.select_syntax, SelectSyntax::ANSI);
        assert_eq!(FeatureSet::POSTGRES.select_syntax, SelectSyntax::POSTGRES);
        let ansi = FeatureSet::ANSI.select_syntax;
        let postgres = FeatureSet::POSTGRES.select_syntax;
        let ansi_qt = FeatureSet::ANSI.query_tail_syntax;
        assert!(!ansi.distinct_on);
        assert!(postgres.distinct_on);
        assert!(ansi_qt.fetch_first);
        assert_ne!(ansi, postgres);

        // The fetch-first gate can be turned off independently of the base preset.
        let no_fetch =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                fetch_first: false,
                ..QueryTailSyntax::ANSI
            }));
        assert!(!no_fetch.query_tail_syntax.fetch_first);
        assert_ne!(
            no_fetch.query_tail_syntax,
            FeatureSet::ANSI.query_tail_syntax
        );
    }

    #[test]
    fn utility_syntax_is_explicit_dialect_data() {
        // `COPY` and `COMMENT ON` are PostgreSQL-only: the ANSI baseline leaves both off,
        // PostgreSQL turns both on, and MySQL reuses the ANSI off value. Bind to locals so
        // the field checks are not const-folded into a clippy `assertions_on_constants`
        // lint.
        assert_eq!(FeatureSet::ANSI.utility_syntax, UtilitySyntax::ANSI);
        assert_eq!(FeatureSet::POSTGRES.utility_syntax, UtilitySyntax::POSTGRES);
        let ansi = FeatureSet::ANSI.utility_syntax;
        let postgres = FeatureSet::POSTGRES.utility_syntax;
        assert!(!ansi.copy);
        assert!(postgres.copy);
        assert!(!ansi.comment_on);
        assert!(postgres.comment_on);
        assert_ne!(ansi, postgres);

        // The gates toggle independently of the base preset.
        let copy_on = FeatureSet::ANSI.with(FeatureDelta::EMPTY.utility_syntax(UtilitySyntax {
            copy: true,
            ..UtilitySyntax::ANSI
        }));
        assert!(copy_on.utility_syntax.copy);
        assert_ne!(copy_on.utility_syntax, FeatureSet::ANSI.utility_syntax);

        // The SQLite statement gates ride the same struct: SQLITE dispatches
        // `PRAGMA`/`ATTACH` and the `VACUUM`/`REINDEX`/`ANALYZE` maintenance trio while
        // keeping the PostgreSQL statements off.
        let sqlite = FeatureSet::SQLITE.utility_syntax;
        let sqlite_maint = FeatureSet::SQLITE.maintenance_syntax;
        let ansi_maint = FeatureSet::ANSI.maintenance_syntax;
        let postgres_maint = FeatureSet::POSTGRES.maintenance_syntax;
        assert_eq!(sqlite, UtilitySyntax::SQLITE);
        assert!(sqlite.pragma);
        assert!(sqlite.attach);
        assert!(sqlite_maint.vacuum);
        assert!(sqlite_maint.reindex);
        assert!(sqlite_maint.analyze);
        assert!(!sqlite.copy);
        assert!(!ansi.pragma);
        assert!(!ansi.attach);
        assert!(!ansi_maint.vacuum);
        assert!(!ansi_maint.reindex);
        assert!(!ansi_maint.analyze);
        assert!(!postgres.pragma);
        assert!(!postgres.attach);
        // PostgreSQL has its own `VACUUM`/`ANALYZE`/`REINDEX`, but only SQLite's forms
        // are modelled, so the gates stay off under the PostgreSQL preset.
        assert!(!postgres_maint.vacuum);
        assert!(!postgres_maint.reindex);
        assert!(!postgres_maint.analyze);

        // `create_trigger` is the whole-statement DDL gate: on for SQLite, off for the
        // standard/PostgreSQL baselines whose trigger body form is not modelled. Bound
        // to locals like the checks above, for the same const-folding reason.
        let sqlite_schema_change = FeatureSet::SQLITE.statement_ddl_gates;
        let ansi_schema_change = FeatureSet::ANSI.statement_ddl_gates;
        let postgres_schema_change = FeatureSet::POSTGRES.statement_ddl_gates;
        assert!(sqlite_schema_change.create_trigger);
        assert!(!ansi_schema_change.create_trigger);
        assert!(!postgres_schema_change.create_trigger);

        // `create_macro` is the parallel whole-statement DDL gate for DuckDB's macro DDL:
        // on for DuckDB, off for every non-DuckDB baseline (PostgreSQL's `CREATE FUNCTION`
        // is the string-body routine, not a live-body macro). Bound to a local like the
        // checks above, both for const-folding and to keep clippy off the constant assert.
        let duckdb_schema_change = FeatureSet::DUCKDB.statement_ddl_gates;
        assert!(duckdb_schema_change.create_macro);
        assert!(!ansi_schema_change.create_macro);
        assert!(!postgres_schema_change.create_macro);
        assert!(!sqlite_schema_change.create_macro);
    }

    #[test]
    fn mysql_preset_composes_existing_knobs_as_data() {
        // The third dialect is a re-composition of foundation knobs, not new fields:
        // every distinctive MySQL lexical choice is a value the preset selects.
        let mysql = FeatureSet::MYSQL;

        // Backtick-only identifier quoting; `"` is a string, not an identifier.
        assert_eq!(mysql.identifier_quotes, MYSQL_IDENTIFIER_QUOTES);
        assert!(mysql.string_literals.double_quoted_strings);
        assert!(mysql.string_literals.backslash_escapes);
        // Operator-meaning divergences: `||` is OR and `&&` is AND in MySQL.
        assert_eq!(mysql.pipe_operator, PipeOperator::LogicalOr);
        assert_eq!(mysql.double_ampersand, DoubleAmpersand::LogicalAnd);
        // `#` line comments, `0x`/`0b` numbers, `?` placeholders, `$` in identifiers.
        assert!(mysql.comment_syntax.line_comment_hash);
        assert!(mysql.numeric_literals.hex_integers && mysql.numeric_literals.binary_integers);
        assert!(mysql.parameters.anonymous_question);
        assert!(mysql.identifier_syntax.dollar_in_identifiers);
        // The one new grammar gate: the `LIMIT a, b` comma form.
        assert!(mysql.query_tail_syntax.limit_offset_comma);

        // It is genuinely distinct from both shipped presets.
        assert_ne!(mysql, FeatureSet::ANSI);
        assert_ne!(mysql, FeatureSet::POSTGRES);

        // Adding the preset did not perturb the existing ones (no new field default
        // leaked a behaviour change): ANSI/PostgreSQL still reject the comma form.
        // Bind to locals so the const field reads are not flagged by clippy's
        // `assertions_on_constants`.
        let ansi_qt = FeatureSet::ANSI.query_tail_syntax;
        let postgres_qt = FeatureSet::POSTGRES.query_tail_syntax;
        assert!(!ansi_qt.limit_offset_comma);
        assert!(!postgres_qt.limit_offset_comma);
    }

    #[test]
    fn limit_by_clause_off_for_oracle_compared_presets() {
        // ClickHouse `LIMIT n BY …` has no differential oracle, so the no-oracle
        // acceptance addition belongs to the ClickHouse preset and Lenient (the `apply_join`
        // precedent): on for Lenient, off for every oracle-compared preset. Bind to a
        // local so the const read is not flagged by clippy's `assertions_on_constants`.
        let lenient_qt = FeatureSet::LENIENT.query_tail_syntax;
        assert!(lenient_qt.limit_by_clause);
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.query_tail_syntax.limit_by_clause);
        }
    }

    #[test]
    fn format_clause_off_for_oracle_compared_presets() {
        // ClickHouse `FORMAT <name>` has no differential oracle, so the no-oracle
        // acceptance addition belongs to the ClickHouse preset and Lenient (the `settings_clause`
        // precedent): on for Lenient, off for every oracle-compared preset. Bind to a
        // local so the const read is not flagged by clippy's `assertions_on_constants`.
        let lenient_qt = FeatureSet::LENIENT.query_tail_syntax;
        assert!(lenient_qt.format_clause);
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.query_tail_syntax.format_clause);
        }
    }

    #[test]
    fn for_xml_json_clause_on_for_mssql_and_lenient_off_elsewhere() {
        // MSSQL `FOR XML`/`FOR JSON` has no differential oracle, so the no-oracle
        // acceptance addition belongs to the MSSQL preset and Lenient (the permissive
        // union): on for both, off for every oracle-compared preset. Bind to a local so
        // the const read is not flagged by clippy's `assertions_on_constants`.
        let lenient_qt = FeatureSet::LENIENT.query_tail_syntax;
        assert!(lenient_qt.for_xml_json_clause);
        #[cfg(feature = "mssql")]
        {
            let mssql_qt = FeatureSet::MSSQL.query_tail_syntax;
            assert!(mssql_qt.for_xml_json_clause);
            // MSSQL spells result-shaping with `FOR`, but has no query-tail locking
            // clause — the two `FOR`-led clauses never coexist under this preset.
            assert!(!mssql_qt.locking_clauses);
        }
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.query_tail_syntax.for_xml_json_clause);
        }
    }

    #[test]
    fn lateral_view_clause_on_for_hive_databricks_and_lenient_off_elsewhere() {
        // Hive/Spark `LATERAL VIEW` has no differential oracle, so the no-oracle
        // acceptance addition belongs to the Hive and Databricks presets and Lenient
        // (the permissive union): on for all three, off for every oracle-compared
        // preset. Bind to a local so the const read is not flagged by clippy's
        // `assertions_on_constants`.
        let lenient_select = FeatureSet::LENIENT.select_syntax;
        assert!(lenient_select.lateral_view_clause);
        // Lenient also enables the LATERAL derived-table factor — the one preset with
        // both `LATERAL`-led gates on, which the position/follow-token partition keeps
        // conflict-free (see the flag doc).
        let lenient_factors = FeatureSet::LENIENT.table_factor_syntax;
        assert!(lenient_factors.lateral);
        #[cfg(feature = "hive")]
        {
            let hive = FeatureSet::HIVE;
            assert!(hive.select_syntax.lateral_view_clause);
            // Hive has no LATERAL derived-table factor gate on, so under its preset
            // `LATERAL` leads only the view clause.
            assert!(!hive.table_factor_syntax.lateral);
        }
        #[cfg(feature = "databricks")]
        {
            let dbx = FeatureSet::DATABRICKS;
            assert!(dbx.select_syntax.lateral_view_clause);
            assert!(!dbx.table_factor_syntax.lateral);
        }
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.select_syntax.lateral_view_clause);
        }
    }

    #[test]
    fn connect_by_clause_on_for_snowflake_and_lenient_off_elsewhere() {
        // The Oracle-style `START WITH`/`CONNECT BY` hierarchical query clause has no
        // differential oracle, so the no-oracle acceptance addition belongs to the
        // Snowflake preset (whose public docs are the citable grammar) and Lenient (the
        // permissive union): on for both, off for every oracle-compared preset. Bind to a
        // local so the const read is not flagged by clippy's `assertions_on_constants`.
        let lenient_select = FeatureSet::LENIENT.select_syntax;
        assert!(lenient_select.connect_by_clause);
        #[cfg(feature = "snowflake")]
        {
            let snowflake = FeatureSet::SNOWFLAKE;
            assert!(snowflake.select_syntax.connect_by_clause);
        }
        // Databricks/Spark documents recursive CTEs instead of `CONNECT BY` and does not
        // support the Oracle `CONNECT BY … START WITH` syntax, so it stays off — unlike
        // the LATERAL VIEW clause, which Databricks *does* inherit from Spark.
        #[cfg(feature = "databricks")]
        {
            let dbx = FeatureSet::DATABRICKS;
            assert!(!dbx.select_syntax.connect_by_clause);
        }
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.select_syntax.connect_by_clause);
        }
    }

    #[test]
    fn nullable_type_off_for_oracle_compared_presets() {
        // ClickHouse `Nullable(T)` has no differential oracle, so the no-oracle
        // acceptance addition belongs to the ClickHouse preset and Lenient (the `composite_types`
        // precedent): on for Lenient, off for every oracle-compared preset. Bind to a local
        // so the const read is not flagged by clippy's `assertions_on_constants`.
        let lenient_types = FeatureSet::LENIENT.type_name_syntax;
        assert!(lenient_types.nullable_type);
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.type_name_syntax.nullable_type);
        }
    }

    #[test]
    fn low_cardinality_type_off_for_oracle_compared_presets() {
        // ClickHouse `LowCardinality(T)` has no differential oracle, so the no-oracle
        // acceptance addition belongs to the ClickHouse preset and Lenient (the `nullable_type`
        // precedent): on for Lenient, off for every oracle-compared preset. Bind to a local
        // so the const read is not flagged by clippy's `assertions_on_constants`.
        let lenient_types = FeatureSet::LENIENT.type_name_syntax;
        assert!(lenient_types.low_cardinality_type);
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.type_name_syntax.low_cardinality_type);
        }
    }

    #[test]
    fn fixed_string_type_off_for_oracle_compared_presets() {
        // ClickHouse `FixedString(N)` has no differential oracle, so the no-oracle
        // acceptance addition belongs to the ClickHouse preset and Lenient (the `nullable_type` /
        // `low_cardinality_type` precedent): on for Lenient, off for every oracle-compared
        // preset. Bind to a local so the const read is not flagged by clippy's
        // `assertions_on_constants`.
        let lenient_types = FeatureSet::LENIENT.type_name_syntax;
        assert!(lenient_types.fixed_string_type);
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.type_name_syntax.fixed_string_type);
        }
    }

    #[test]
    fn datetime64_type_off_for_oracle_compared_presets() {
        // ClickHouse `DateTime64(P[, 'tz'])` has no differential oracle, so the
        // no-oracle acceptance addition belongs to the ClickHouse preset and Lenient (the
        // `fixed_string_type` precedent): on for Lenient, off for every oracle-compared
        // preset. Bind to a local so the const read is not flagged by clippy's
        // `assertions_on_constants`.
        let lenient_types = FeatureSet::LENIENT.type_name_syntax;
        assert!(lenient_types.datetime64_type);
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.type_name_syntax.datetime64_type);
        }
    }

    #[test]
    fn nested_type_off_for_oracle_compared_presets() {
        // ClickHouse `Nested(name Type, ...)` has no differential oracle, so the
        // no-oracle acceptance addition belongs to the ClickHouse preset and Lenient (the
        // `datetime64_type` precedent): on for Lenient, off for every oracle-compared
        // preset. Bind to a local so the const read is not flagged by clippy's
        // `assertions_on_constants`.
        let lenient_types = FeatureSet::LENIENT.type_name_syntax;
        assert!(lenient_types.nested_type);
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.type_name_syntax.nested_type);
        }
    }

    #[test]
    fn bit_width_integer_names_off_for_oracle_compared_presets() {
        // ClickHouse's `Int8`…`Int256`/`UInt*` fixed-bit-width integer names have no
        // differential oracle, so the no-oracle acceptance addition belongs to the ClickHouse
        // preset and Lenient (the `datetime64_type` precedent): on for Lenient, off for every
        // oracle-compared preset. Bind to a local so the const read is not flagged by clippy's
        // `assertions_on_constants`.
        let lenient_types = FeatureSet::LENIENT.type_name_syntax;
        assert!(lenient_types.bit_width_integer_names);
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.type_name_syntax.bit_width_integer_names);
        }
    }

    #[test]
    fn settings_clause_off_for_oracle_compared_presets() {
        // ClickHouse `SETTINGS name = value, …` has no differential oracle, so the
        // no-oracle acceptance addition belongs to the ClickHouse preset and Lenient (the
        // `limit_by_clause` precedent): on for Lenient, off for every oracle-compared
        // preset. Bind to a local so the const read is not flagged by clippy's
        // `assertions_on_constants`.
        let lenient_qt = FeatureSet::LENIENT.query_tail_syntax;
        assert!(lenient_qt.settings_clause);
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(!preset.query_tail_syntax.settings_clause);
        }
    }

    #[test]
    fn feature_set_as_of_is_a_deterministic_total_anchor() {
        // Invariant across feature-ID-era editions at M1 (every implemented standard
        // feature is SQL:1999 Core), but a stable, total pin point.
        for version in [
            StandardVersion::Sql1999,
            StandardVersion::Sql2003,
            StandardVersion::Sql2008,
            StandardVersion::Sql2011,
            StandardVersion::Sql2016,
        ] {
            assert_eq!(FeatureSet::as_of(version), FeatureSet::ANSI);
        }
    }

    #[test]
    fn byte_classes_cover_m1_lexer_needs() {
        assert!(has_class(b' ', CLASS_WHITESPACE));
        assert!(has_class(b'\n', CLASS_WHITESPACE));

        assert!(has_class(b'a', CLASS_IDENTIFIER_START));
        assert!(has_class(b'Z', CLASS_IDENTIFIER_START));
        assert!(has_class(b'_', CLASS_IDENTIFIER_START));
        assert!(has_class(b'a', CLASS_IDENTIFIER_CONTINUE));

        assert!(has_class(b'7', CLASS_DIGIT));
        assert!(has_class(b'7', CLASS_IDENTIFIER_CONTINUE));
        assert!(!has_class(b'7', CLASS_IDENTIFIER_START));

        assert!(has_class(b'+', CLASS_OPERATOR));
        assert!(has_class(b'|', CLASS_OPERATOR));
        assert!(has_class(b'(', CLASS_PUNCTUATION));
        assert!(has_class(b'.', CLASS_PUNCTUATION));

        assert!(has_class(0x80, CLASS_IDENTIFIER_CONTINUE));
        assert_eq!(byte_class(0), 0);
    }

    #[test]
    fn vertical_tab_is_dialect_specific_whitespace() {
        // The vertical tab (`0x0b`) is the one member of the flex `space` set `[ \t\n\r\f\v]`
        // that Rust's `is_ascii_whitespace` — and hence `STANDARD_BYTE_CLASSES` — omits, and
        // every engine treats it differently. Engine-measured:
        //
        // - PostgreSQL / MySQL fold it as *ordinary* whitespace everywhere (a lone `0x0b`
        //   parses as an empty statement, `SELECT\x0b1` as `SELECT 1`) — full `CLASS_WHITESPACE`.
        assert!(FeatureSet::POSTGRES.has_byte_class(0x0b, CLASS_WHITESPACE));
        assert!(FeatureSet::MYSQL.has_byte_class(0x0b, CLASS_WHITESPACE));
        // - SQLite folds it only as a whitespace-run *continuation*: it rides an open run
        //   (`"\x20\x0b"` accepts) but cannot start one (lone `"\x0b"` rejects) — so it carries
        //   `CLASS_WHITESPACE_CONTINUE`, never `CLASS_WHITESPACE`.
        assert!(FeatureSet::SQLITE.has_byte_class(0x0b, CLASS_WHITESPACE_CONTINUE));
        assert!(!FeatureSet::SQLITE.has_byte_class(0x0b, CLASS_WHITESPACE));
        // - DuckDB folds it as statement-boundary *trim*: whitespace at a `;`-segment's edges
        //   (lone `"\x0b"`, `"SELECT 1\x0b"` accept) but a hard error interior to a statement
        //   (`"SELECT\x0b1"` rejects) — so it carries both `CLASS_WHITESPACE` and the
        //   `CLASS_WHITESPACE_BOUNDARY` marker the tokenizer's interior guard reads.
        assert!(FeatureSet::DUCKDB.has_byte_class(0x0b, CLASS_WHITESPACE));
        assert!(FeatureSet::DUCKDB.has_byte_class(0x0b, CLASS_WHITESPACE_BOUNDARY));
        assert!(FeatureSet::DUCKDB.byte_classes.has_boundary_whitespace());
        // - ANSI (and every other preset) keeps it strict: not whitespace in any form.
        for preset in [FeatureSet::ANSI, FeatureSet::POSTGRES, FeatureSet::MYSQL] {
            assert!(!preset.has_byte_class(0x0b, CLASS_WHITESPACE_BOUNDARY));
        }
        assert!(!FeatureSet::ANSI.has_byte_class(
            0x0b,
            CLASS_WHITESPACE | CLASS_WHITESPACE_CONTINUE | CLASS_WHITESPACE_BOUNDARY,
        ));
        // Only DuckDB flags the boundary class, so only its tokenizer pays the guard.
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
        ] {
            assert!(!preset.byte_classes.has_boundary_whitespace());
        }
        // The form feed (`0x0c`) is shared whitespace: every probed engine folds it, and it
        // already rides `STANDARD_BYTE_CLASSES` via `is_ascii_whitespace`.
        for preset in [
            FeatureSet::ANSI,
            FeatureSet::POSTGRES,
            FeatureSet::MYSQL,
            FeatureSet::SQLITE,
            FeatureSet::DUCKDB,
        ] {
            assert!(preset.has_byte_class(0x0c, CLASS_WHITESPACE));
        }
    }

    #[test]
    fn dialect_data_owns_byte_classes_and_binding_powers() {
        let byte_classes = FeatureSet::ANSI
            .byte_classes
            .with_class(b'@', CLASS_IDENTIFIER_START | CLASS_IDENTIFIER_CONTINUE);
        let binding_powers = FeatureSet::ANSI.binding_powers.with_binary(
            &BinaryOperator::StringConcat,
            BindingPower {
                left: 70,
                right: 71,
                assoc: crate::precedence::Assoc::Left,
            },
        );
        let custom = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .byte_classes(byte_classes)
                .binding_powers(binding_powers),
        );

        assert!(custom.has_byte_class(b'@', CLASS_IDENTIFIER_START));
        assert_eq!(custom.binding_power(&BinaryOperator::StringConcat).left, 70);
        assert_eq!(
            custom.binding_power(&BinaryOperator::Plus),
            FeatureSet::ANSI.binding_power(&BinaryOperator::Plus),
        );
    }

    #[test]
    fn dialect_data_owns_set_operation_binding_powers() {
        let custom_set_powers = FeatureSet::ANSI.set_operation_powers.with_set_operator(
            &SetOperator::Union,
            BindingPower {
                left: 30,
                right: 31,
                assoc: crate::precedence::Assoc::Left,
            },
        );
        let custom =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.set_operation_powers(custom_set_powers));

        assert_eq!(
            custom.set_operation_binding_power(&SetOperator::Union).left,
            30,
        );
        assert_eq!(
            custom
                .set_operation_binding_power(&SetOperator::Except)
                .left,
            30,
        );
        assert_eq!(
            custom.set_operation_binding_power(&SetOperator::Intersect),
            FeatureSet::ANSI.set_operation_binding_power(&SetOperator::Intersect),
        );
    }
}
