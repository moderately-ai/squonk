// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Runtime built-in dialect selection.
//!
//! [`Dialect::Ext`](crate::parser::Dialect::Ext) makes `&dyn Dialect` non-object-safe,
//! so a config/CLI string cannot pick a compile-time dialect the way
//! `parse_with(src, Postgres)` does. This module is the designed escape: a
//! [`BuiltinDialect`] value-enum over the stock dialects — all of which share
//! `Ext = NoExt` — plus a [`parse_with_builtin`] dispatcher that matches the value to
//! the monomorphized parser and returns a plain [`Parsed`]. Each non-ANSI arm is
//! gated by its cargo feature, so a name for a disabled (or unknown) dialect resolves
//! to `None` rather than panicking.

use std::fmt;
use std::str::FromStr;

use crate::ast::dialect::FeatureSet;
use crate::dialect::Ansi;
#[cfg(feature = "bigquery")]
use crate::dialect::BigQuery;
#[cfg(feature = "clickhouse")]
use crate::dialect::ClickHouse;
#[cfg(feature = "databricks")]
use crate::dialect::Databricks;
#[cfg(feature = "duckdb")]
use crate::dialect::DuckDb;
#[cfg(feature = "hive")]
use crate::dialect::Hive;
#[cfg(feature = "lenient")]
use crate::dialect::Lenient;
#[cfg(feature = "mssql")]
use crate::dialect::Mssql;
#[cfg(feature = "mysql")]
use crate::dialect::MySql;
#[cfg(feature = "postgres")]
use crate::dialect::Postgres;
#[cfg(feature = "redshift")]
use crate::dialect::Redshift;
#[cfg(feature = "snowflake")]
use crate::dialect::Snowflake;
#[cfg(feature = "sqlite")]
use crate::dialect::Sqlite;
use crate::error::ParseResult;
use crate::parser::{
    ParseOptions, Parsed, Recovered, parse_recovering_with_options, parse_with_options,
};
use crate::render::RenderDialect;
use crate::tokenizer::{LexError, Token, TriviaIndex, tokenize_with, tokenize_with_trivia};

/// A stock dialect chosen at runtime by value or name.
///
/// The arms present in a given build are exactly the dialects compiled in: [`Ansi`]
/// always, every other dialect only with its cargo feature. It is `#[non_exhaustive]`
/// because the variant set grows with new dialects and varies by feature, so downstream
/// `match`es must carry a wildcard.
///
/// # Parsing and formatting
///
/// [`FromStr`] and [`Display`](fmt::Display) mirror the inherent
/// [`from_name`](Self::from_name)/[`name`](Self::name) pair, so the type drops straight
/// into config- and CLI-driven selection (clap's `value_parser!`, serde's string forms)
/// without a hand-written adapter. Parsing an unknown or feature-disabled name is a typed
/// [`ParseBuiltinDialectError`], never a panic; [`Default`] is [`Ansi`], the
/// always-compiled baseline `parse` itself defaults to.
///
/// ```
/// use squonk::dialect::BuiltinDialect;
///
/// let dialect: BuiltinDialect = "ansi".parse().expect("ansi is always built in");
/// assert_eq!(dialect, BuiltinDialect::Ansi);
/// assert_eq!(dialect, BuiltinDialect::default());
/// assert_eq!(dialect.to_string(), "ansi"); // Display == canonical name
/// assert!("no-such-dialect".parse::<BuiltinDialect>().is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BuiltinDialect {
    /// The ANSI/standard baseline ([`Ansi`]).
    Ansi,
    /// PostgreSQL ([`Postgres`]); present only with the `postgres` feature.
    #[cfg(feature = "postgres")]
    Postgres,
    /// MySQL ([`MySql`]); present only with the `mysql` feature.
    #[cfg(feature = "mysql")]
    MySql,
    /// SQLite ([`Sqlite`]); present only with the `sqlite` feature.
    #[cfg(feature = "sqlite")]
    Sqlite,
    /// DuckDB ([`DuckDb`]); present only with the `duckdb` feature.
    #[cfg(feature = "duckdb")]
    DuckDb,
    /// BigQuery / ZetaSQL ([`BigQuery`]); present only with the `bigquery` feature.
    #[cfg(feature = "bigquery")]
    BigQuery,
    /// Hive / HiveQL ([`Hive`]); present only with the `hive` feature.
    #[cfg(feature = "hive")]
    Hive,
    /// ClickHouse ([`ClickHouse`]); present only with the `clickhouse` feature.
    #[cfg(feature = "clickhouse")]
    ClickHouse,
    /// Databricks ([`Databricks`]); present only with the `databricks` feature.
    #[cfg(feature = "databricks")]
    Databricks,
    /// MSSQL / T-SQL ([`Mssql`]); present only with the `mssql` feature.
    #[cfg(feature = "mssql")]
    Mssql,
    /// Snowflake ([`Snowflake`]); present only with the `snowflake` feature.
    #[cfg(feature = "snowflake")]
    Snowflake,
    /// Amazon Redshift ([`Redshift`]); present only with the `redshift` feature.
    #[cfg(feature = "redshift")]
    Redshift,
    /// The permissive "parse anything" tooling union ([`Lenient`]); present only with
    /// the `lenient` feature.
    #[cfg(feature = "lenient")]
    Lenient,
}

impl BuiltinDialect {
    /// Every built-in dialect compiled into this build, ANSI first.
    ///
    /// A consumer enumerating selectable dialects (a CLI `--dialect` help text, a
    /// config validator) reads this rather than hard-coding names that may be gated
    /// out of the current build.
    pub const ALL: &'static [BuiltinDialect] = &[
        BuiltinDialect::Ansi,
        #[cfg(feature = "postgres")]
        BuiltinDialect::Postgres,
        #[cfg(feature = "mysql")]
        BuiltinDialect::MySql,
        #[cfg(feature = "sqlite")]
        BuiltinDialect::Sqlite,
        #[cfg(feature = "duckdb")]
        BuiltinDialect::DuckDb,
        #[cfg(feature = "bigquery")]
        BuiltinDialect::BigQuery,
        #[cfg(feature = "hive")]
        BuiltinDialect::Hive,
        #[cfg(feature = "clickhouse")]
        BuiltinDialect::ClickHouse,
        #[cfg(feature = "databricks")]
        BuiltinDialect::Databricks,
        #[cfg(feature = "mssql")]
        BuiltinDialect::Mssql,
        #[cfg(feature = "snowflake")]
        BuiltinDialect::Snowflake,
        #[cfg(feature = "redshift")]
        BuiltinDialect::Redshift,
        #[cfg(feature = "lenient")]
        BuiltinDialect::Lenient,
    ];

    /// Resolve a builtin by case-insensitive name and common aliases, returning
    /// `None` for an unknown name *or* one whose dialect is not compiled into this
    /// build (e.g. `"postgres"` without the `postgres` feature). Never panics — the
    /// clean-error contract for config/CLI-driven selection.
    ///
    /// # Migrating from `datafusion-sqlparser-rs`
    ///
    /// `"generic"` is an alias for strict [`Ansi`] (the SQL:2016
    /// standard), matching that crate's `AnsiDialect` strictness — **not** its
    /// `GenericDialect`, a permissive catch-all that accepts non-standard surface such
    /// as `COPY`. A `GenericDialect` consumer wanting that permissive behaviour should
    /// select `"lenient"` (`Lenient`, the `lenient` feature) — our documented
    /// parse-anything union and the true catch-all (see the preset spectrum on
    /// [`FeatureSet`]). Selecting `"generic"` here is
    /// therefore *stricter* than their `GenericDialect`: input it accepted (e.g. `COPY`)
    /// now rejects.
    pub fn from_name(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("ansi") || name.eq_ignore_ascii_case("generic") {
            return Some(Self::Ansi);
        }
        #[cfg(feature = "postgres")]
        if name.eq_ignore_ascii_case("postgres")
            || name.eq_ignore_ascii_case("postgresql")
            || name.eq_ignore_ascii_case("pg")
        {
            return Some(Self::Postgres);
        }
        #[cfg(feature = "mysql")]
        if name.eq_ignore_ascii_case("mysql") || name.eq_ignore_ascii_case("mariadb") {
            return Some(Self::MySql);
        }
        #[cfg(feature = "sqlite")]
        if name.eq_ignore_ascii_case("sqlite") || name.eq_ignore_ascii_case("sqlite3") {
            return Some(Self::Sqlite);
        }
        #[cfg(feature = "duckdb")]
        if name.eq_ignore_ascii_case("duckdb") || name.eq_ignore_ascii_case("duck") {
            return Some(Self::DuckDb);
        }
        #[cfg(feature = "bigquery")]
        if name.eq_ignore_ascii_case("bigquery")
            || name.eq_ignore_ascii_case("bq")
            || name.eq_ignore_ascii_case("zetasql")
        {
            return Some(Self::BigQuery);
        }
        #[cfg(feature = "hive")]
        if name.eq_ignore_ascii_case("hive") || name.eq_ignore_ascii_case("hiveql") {
            return Some(Self::Hive);
        }
        #[cfg(feature = "clickhouse")]
        if name.eq_ignore_ascii_case("clickhouse") || name.eq_ignore_ascii_case("ch") {
            return Some(Self::ClickHouse);
        }
        #[cfg(feature = "databricks")]
        if name.eq_ignore_ascii_case("databricks") || name.eq_ignore_ascii_case("dbx") {
            return Some(Self::Databricks);
        }
        #[cfg(feature = "mssql")]
        if name.eq_ignore_ascii_case("mssql")
            || name.eq_ignore_ascii_case("tsql")
            || name.eq_ignore_ascii_case("sqlserver")
        {
            return Some(Self::Mssql);
        }
        #[cfg(feature = "snowflake")]
        if name.eq_ignore_ascii_case("snowflake") || name.eq_ignore_ascii_case("sf") {
            return Some(Self::Snowflake);
        }
        // Canonical `redshift` plus the unambiguous `amazonredshift`. The bare abbreviation `rs`
        // is deliberately *not* accepted — too generic (Rust source extension, "right side", a
        // common column name) to claim as a dialect selector.
        #[cfg(feature = "redshift")]
        if name.eq_ignore_ascii_case("redshift") || name.eq_ignore_ascii_case("amazonredshift") {
            return Some(Self::Redshift);
        }
        #[cfg(feature = "lenient")]
        if name.eq_ignore_ascii_case("lenient") || name.eq_ignore_ascii_case("permissive") {
            return Some(Self::Lenient);
        }
        None
    }

    /// The canonical lower-case name of this builtin — the primary spelling
    /// [`from_name`](Self::from_name) accepts, so `from_name(d.name()) == Some(d)`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ansi => "ansi",
            #[cfg(feature = "postgres")]
            Self::Postgres => "postgres",
            #[cfg(feature = "mysql")]
            Self::MySql => "mysql",
            #[cfg(feature = "sqlite")]
            Self::Sqlite => "sqlite",
            #[cfg(feature = "duckdb")]
            Self::DuckDb => "duckdb",
            #[cfg(feature = "bigquery")]
            Self::BigQuery => "bigquery",
            #[cfg(feature = "hive")]
            Self::Hive => "hive",
            #[cfg(feature = "clickhouse")]
            Self::ClickHouse => "clickhouse",
            #[cfg(feature = "databricks")]
            Self::Databricks => "databricks",
            #[cfg(feature = "mssql")]
            Self::Mssql => "mssql",
            #[cfg(feature = "snowflake")]
            Self::Snowflake => "snowflake",
            #[cfg(feature = "redshift")]
            Self::Redshift => "redshift",
            #[cfg(feature = "lenient")]
            Self::Lenient => "lenient",
        }
    }

    /// Every case-insensitive name accepted for this builtin in this build, with the
    /// canonical spelling first.
    pub const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Ansi => &["ansi", "generic"],
            #[cfg(feature = "postgres")]
            Self::Postgres => &["postgres", "postgresql", "pg"],
            #[cfg(feature = "mysql")]
            Self::MySql => &["mysql", "mariadb"],
            #[cfg(feature = "sqlite")]
            Self::Sqlite => &["sqlite", "sqlite3"],
            #[cfg(feature = "duckdb")]
            Self::DuckDb => &["duckdb", "duck"],
            #[cfg(feature = "bigquery")]
            Self::BigQuery => &["bigquery", "bq", "zetasql"],
            #[cfg(feature = "hive")]
            Self::Hive => &["hive", "hiveql"],
            #[cfg(feature = "clickhouse")]
            Self::ClickHouse => &["clickhouse", "ch"],
            #[cfg(feature = "databricks")]
            Self::Databricks => &["databricks", "dbx"],
            #[cfg(feature = "mssql")]
            Self::Mssql => &["mssql", "tsql", "sqlserver"],
            #[cfg(feature = "snowflake")]
            Self::Snowflake => &["snowflake", "sf"],
            #[cfg(feature = "redshift")]
            Self::Redshift => &["redshift", "amazonredshift"],
            #[cfg(feature = "lenient")]
            Self::Lenient => &["lenient", "permissive"],
        }
    }

    /// The const [`FeatureSet`] preset this builtin parses and renders with — the
    /// runtime analogue of `Dialect::features` for code that needs the dialect data
    /// without constructing a `Parser`.
    pub fn features(self) -> &'static FeatureSet {
        match self {
            Self::Ansi => &FeatureSet::ANSI,
            #[cfg(feature = "postgres")]
            Self::Postgres => &FeatureSet::POSTGRES,
            #[cfg(feature = "mysql")]
            Self::MySql => &FeatureSet::MYSQL,
            #[cfg(feature = "sqlite")]
            Self::Sqlite => &FeatureSet::SQLITE,
            #[cfg(feature = "duckdb")]
            Self::DuckDb => &FeatureSet::DUCKDB,
            #[cfg(feature = "bigquery")]
            Self::BigQuery => &FeatureSet::BIGQUERY,
            #[cfg(feature = "hive")]
            Self::Hive => &FeatureSet::HIVE,
            #[cfg(feature = "clickhouse")]
            Self::ClickHouse => &FeatureSet::CLICKHOUSE,
            #[cfg(feature = "databricks")]
            Self::Databricks => &FeatureSet::DATABRICKS,
            #[cfg(feature = "mssql")]
            Self::Mssql => &FeatureSet::MSSQL,
            #[cfg(feature = "snowflake")]
            Self::Snowflake => &FeatureSet::SNOWFLAKE,
            #[cfg(feature = "redshift")]
            Self::Redshift => &FeatureSet::REDSHIFT,
            #[cfg(feature = "lenient")]
            Self::Lenient => &FeatureSet::LENIENT,
        }
    }
}

// The std-trait surface below (`Display`/`FromStr`/`Default` plus the new public
// `ParseBuiltinDialectError`) is a purely additive extension of `BuiltinDialect`: no
// existing item changes shape, so it is semver-compatible against the
// `release/semver-baseline.toml` contract (C-COMMON-TRAITS — a string-selected config
// enum implements the std string traits directly rather than via inherent methods alone).

impl fmt::Display for BuiltinDialect {
    /// Writes the canonical [`name`](Self::name) — the spelling
    /// [`from_name`](Self::from_name) round-trips.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for BuiltinDialect {
    type Err = ParseBuiltinDialectError;

    /// Resolves a case-insensitive name or alias via [`from_name`](Self::from_name),
    /// yielding a [`ParseBuiltinDialectError`] for a name that maps to no built-in
    /// dialect in this build.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_name(s).ok_or_else(|| ParseBuiltinDialectError { name: s.into() })
    }
}

impl Default for BuiltinDialect {
    /// [`Ansi`] — the always-compiled SQL-standard baseline `parse` defaults to, so the
    /// runtime selector defaults to the same dialect as the compile-time entry point.
    fn default() -> Self {
        Self::Ansi
    }
}

/// The error [`BuiltinDialect`]'s [`FromStr`] returns for a name that resolves to no
/// built-in dialect in this build.
///
/// The name is either genuinely unrecognized or names a real dialect whose cargo feature
/// is disabled — [`BuiltinDialect::from_name`] draws exactly the same line with its
/// `None`, and this error carries the offending [`name`](Self::name) for a diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseBuiltinDialectError {
    name: Box<str>,
}

impl ParseBuiltinDialectError {
    /// The unrecognized name as supplied to [`FromStr`].
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for ParseBuiltinDialectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown built-in dialect {:?}: not a recognized name or alias, or its dialect \
             is not compiled into this build",
            self.name,
        )
    }
}

impl std::error::Error for ParseBuiltinDialectError {}

/// Parse `src` under a runtime-selected built-in `dialect` into an owned [`Parsed`].
///
/// Dispatches by value to the monomorphized [`parse_with`](crate::parse_with): every
/// builtin's `Ext` is [`NoExt`](crate::ast::NoExt), so all arms share the single
/// [`Parsed`] return type — the designed object-safe runtime path, without a
/// `dyn Dialect`. To select the dialect from a string, resolve it with
/// [`BuiltinDialect::from_name`] first; an unknown or disabled name is a clean `None`
/// there, never a panic here.
///
/// # Errors
///
/// Returns the first [`ParseError`](crate::error::ParseError), exactly as
/// [`parse_with`](crate::parse_with) does.
///
/// ```
/// use squonk::dialect::{BuiltinDialect, parse_with_builtin};
///
/// let dialect = BuiltinDialect::from_name("ansi").expect("ansi is always built in");
/// let parsed = parse_with_builtin("SELECT 1", dialect).expect("`SELECT 1` parses");
/// assert_eq!(parsed.statements().len(), 1);
///
/// assert!(BuiltinDialect::from_name("nope").is_none());
/// ```
pub fn parse_with_builtin(src: &str, dialect: BuiltinDialect) -> ParseResult<Parsed> {
    parse_with_builtin_options(src, dialect, ParseOptions::default())
}

/// Parse `src` under a runtime-selected built-in `dialect`, honouring
/// [`ParseOptions`].
///
/// This is the optioned counterpart to [`parse_with_builtin`]. It keeps runtime
/// dialect dispatch centralized so language bindings do not have to hand-copy the
/// same feature-gated match.
pub fn parse_with_builtin_options(
    src: &str,
    dialect: BuiltinDialect,
    options: ParseOptions,
) -> ParseResult<Parsed> {
    match dialect {
        BuiltinDialect::Ansi => parse_with_options(src, Ansi, options),
        #[cfg(feature = "postgres")]
        BuiltinDialect::Postgres => parse_with_options(src, Postgres, options),
        #[cfg(feature = "mysql")]
        BuiltinDialect::MySql => parse_with_options(src, MySql, options),
        #[cfg(feature = "sqlite")]
        BuiltinDialect::Sqlite => parse_with_options(src, Sqlite, options),
        #[cfg(feature = "duckdb")]
        BuiltinDialect::DuckDb => parse_with_options(src, DuckDb, options),
        #[cfg(feature = "bigquery")]
        BuiltinDialect::BigQuery => parse_with_options(src, BigQuery, options),
        #[cfg(feature = "hive")]
        BuiltinDialect::Hive => parse_with_options(src, Hive, options),
        #[cfg(feature = "clickhouse")]
        BuiltinDialect::ClickHouse => parse_with_options(src, ClickHouse, options),
        #[cfg(feature = "databricks")]
        BuiltinDialect::Databricks => parse_with_options(src, Databricks, options),
        #[cfg(feature = "mssql")]
        BuiltinDialect::Mssql => parse_with_options(src, Mssql, options),
        #[cfg(feature = "snowflake")]
        BuiltinDialect::Snowflake => parse_with_options(src, Snowflake, options),
        #[cfg(feature = "redshift")]
        BuiltinDialect::Redshift => parse_with_options(src, Redshift, options),
        #[cfg(feature = "lenient")]
        BuiltinDialect::Lenient => parse_with_options(src, Lenient, options),
    }
}

/// Parse `src` under a runtime-selected built-in `dialect`, recovering past
/// statement errors to return the partial tree plus diagnostics.
pub fn parse_recovering_with_builtin(src: &str, dialect: BuiltinDialect) -> ParseResult<Recovered> {
    parse_recovering_with_builtin_options(src, dialect, ParseOptions::default())
}

/// [`parse_recovering_with_builtin`] honouring [`ParseOptions`].
pub fn parse_recovering_with_builtin_options(
    src: &str,
    dialect: BuiltinDialect,
    options: ParseOptions,
) -> ParseResult<Recovered> {
    match dialect {
        BuiltinDialect::Ansi => parse_recovering_with_options(src, Ansi, options),
        #[cfg(feature = "postgres")]
        BuiltinDialect::Postgres => parse_recovering_with_options(src, Postgres, options),
        #[cfg(feature = "mysql")]
        BuiltinDialect::MySql => parse_recovering_with_options(src, MySql, options),
        #[cfg(feature = "sqlite")]
        BuiltinDialect::Sqlite => parse_recovering_with_options(src, Sqlite, options),
        #[cfg(feature = "duckdb")]
        BuiltinDialect::DuckDb => parse_recovering_with_options(src, DuckDb, options),
        #[cfg(feature = "bigquery")]
        BuiltinDialect::BigQuery => parse_recovering_with_options(src, BigQuery, options),
        #[cfg(feature = "hive")]
        BuiltinDialect::Hive => parse_recovering_with_options(src, Hive, options),
        #[cfg(feature = "clickhouse")]
        BuiltinDialect::ClickHouse => parse_recovering_with_options(src, ClickHouse, options),
        #[cfg(feature = "databricks")]
        BuiltinDialect::Databricks => parse_recovering_with_options(src, Databricks, options),
        #[cfg(feature = "mssql")]
        BuiltinDialect::Mssql => parse_recovering_with_options(src, Mssql, options),
        #[cfg(feature = "snowflake")]
        BuiltinDialect::Snowflake => parse_recovering_with_options(src, Snowflake, options),
        #[cfg(feature = "redshift")]
        BuiltinDialect::Redshift => parse_recovering_with_options(src, Redshift, options),
        #[cfg(feature = "lenient")]
        BuiltinDialect::Lenient => parse_recovering_with_options(src, Lenient, options),
    }
}

/// Tokenize `src` under a runtime-selected built-in `dialect`.
pub fn tokenize_with_builtin(src: &str, dialect: BuiltinDialect) -> Result<Vec<Token>, LexError> {
    tokenize_with(src, dialect.features())
}

/// Tokenize `src` under a runtime-selected built-in `dialect`, capturing trivia.
pub fn tokenize_with_builtin_trivia(
    src: &str,
    dialect: BuiltinDialect,
) -> Result<(Vec<Token>, TriviaIndex), LexError> {
    tokenize_with_trivia(src, dialect.features())
}

impl RenderDialect for BuiltinDialect {
    fn render_features(&self) -> FeatureSet {
        self.features().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_is_always_built_in_and_round_trips_its_name() {
        let ansi = BuiltinDialect::from_name("ansi").expect("ansi resolves");
        assert_eq!(ansi, BuiltinDialect::Ansi);
        // Case-insensitive, with the `generic` alias.
        assert_eq!(
            BuiltinDialect::from_name("ANSI"),
            Some(BuiltinDialect::Ansi)
        );
        assert_eq!(
            BuiltinDialect::from_name("Generic"),
            Some(BuiltinDialect::Ansi)
        );
        // name() is the inverse of the canonical from_name spelling.
        for dialect in BuiltinDialect::ALL {
            assert_eq!(BuiltinDialect::from_name(dialect.name()), Some(*dialect));
        }
        assert_eq!(BuiltinDialect::Ansi.features(), &FeatureSet::ANSI);
    }

    #[test]
    fn unknown_name_is_a_clean_none() {
        // The clean-error contract: never a panic for an unrecognized name. `duckdb`,
        // `snowflake`, `databricks`, and `mssql`/`tsql` are real dialects behind their cargo
        // features, so they are deliberately absent from this unknown-name set — their positive
        // resolution is proven by `duckdb_builtin_resolves_and_parses_its_surface` /
        // `snowflake_builtin_resolves_and_parses_its_surface` /
        // `databricks_builtin_resolves_and_parses_its_surface` /
        // `mssql_builtin_resolves_and_parses_its_surface`.
        for name in ["", "oracle", "postgre", "my sql"] {
            assert_eq!(BuiltinDialect::from_name(name), None, "{name:?}");
        }
    }

    #[test]
    fn display_and_fromstr_round_trip_over_all_including_aliases_and_case() {
        for &dialect in BuiltinDialect::ALL {
            // Display == canonical name, and FromStr inverts both the canonical name and
            // the `Display` string.
            assert_eq!(dialect.to_string(), dialect.name());
            assert_eq!(dialect.name().parse::<BuiltinDialect>(), Ok(dialect));
            assert_eq!(dialect.to_string().parse::<BuiltinDialect>(), Ok(dialect));
            // Every accepted alias resolves through `FromStr`, case-insensitively.
            for alias in dialect.aliases() {
                assert_eq!(alias.parse::<BuiltinDialect>(), Ok(dialect), "{alias:?}");
                assert_eq!(
                    alias.to_uppercase().parse::<BuiltinDialect>(),
                    Ok(dialect),
                    "{alias:?} upper",
                );
            }
        }
        assert_eq!(BuiltinDialect::default(), BuiltinDialect::Ansi);
    }

    #[test]
    fn fromstr_error_carries_the_name_and_is_a_std_error() {
        let err = "no-such-dialect".parse::<BuiltinDialect>().unwrap_err();
        assert_eq!(err.name(), "no-such-dialect");
        // C-GOOD-ERR: a real `Error`, not `()`, with a non-empty diagnostic Display.
        let as_error: &dyn std::error::Error = &err;
        assert!(as_error.to_string().contains("no-such-dialect"));
    }

    #[test]
    fn parse_with_builtin_parses_under_ansi() {
        let parsed = parse_with_builtin("SELECT 1", BuiltinDialect::Ansi).expect("parses");
        assert_eq!(parsed.statements().len(), 1);
    }

    #[cfg(feature = "postgres")]
    #[test]
    fn postgres_builtin_resolves_and_parses_its_surface() {
        assert_eq!(
            BuiltinDialect::from_name("postgresql"),
            Some(BuiltinDialect::Postgres)
        );
        assert_eq!(
            BuiltinDialect::from_name("PG"),
            Some(BuiltinDialect::Postgres)
        );
        assert_eq!(BuiltinDialect::Postgres.features(), &FeatureSet::POSTGRES);
        // `$1` positional parameters are a PostgreSQL-only lexical form: it parses
        // under the runtime-selected Postgres builtin and is rejected under ANSI.
        assert!(parse_with_builtin("SELECT $1", BuiltinDialect::Postgres).is_ok());
        assert!(parse_with_builtin("SELECT $1", BuiltinDialect::Ansi).is_err());
    }

    #[cfg(feature = "mysql")]
    #[test]
    fn mysql_builtin_resolves_and_parses_its_surface() {
        assert_eq!(
            BuiltinDialect::from_name("MySQL"),
            Some(BuiltinDialect::MySql)
        );
        assert_eq!(BuiltinDialect::MySql.features(), &FeatureSet::MYSQL);
        // The `LIMIT <offset>, <count>` comma form is MySQL-only: parses under the
        // runtime-selected MySQL builtin, rejected under ANSI.
        assert!(parse_with_builtin("SELECT a FROM t LIMIT 5, 10", BuiltinDialect::MySql).is_ok());
        assert!(parse_with_builtin("SELECT a FROM t LIMIT 5, 10", BuiltinDialect::Ansi).is_err());
    }

    #[cfg(feature = "sqlite")]
    #[test]
    fn sqlite_builtin_resolves_and_parses_its_surface() {
        assert_eq!(
            BuiltinDialect::from_name("SQLite"),
            Some(BuiltinDialect::Sqlite)
        );
        assert_eq!(
            BuiltinDialect::from_name("sqlite3"),
            Some(BuiltinDialect::Sqlite)
        );
        assert_eq!(BuiltinDialect::Sqlite.features(), &FeatureSet::SQLITE);
        // The `==` equality spelling and `$name` placeholder are SQLite-only lexical
        // forms: they parse under the runtime-selected SQLite builtin, rejected under ANSI.
        assert!(parse_with_builtin("SELECT 1 == 1", BuiltinDialect::Sqlite).is_ok());
        assert!(parse_with_builtin("SELECT 1 == 1", BuiltinDialect::Ansi).is_err());
        assert!(parse_with_builtin("SELECT $value", BuiltinDialect::Sqlite).is_ok());
    }

    #[cfg(feature = "duckdb")]
    #[test]
    fn duckdb_builtin_resolves_and_parses_its_surface() {
        assert_eq!(
            BuiltinDialect::from_name("DuckDB"),
            Some(BuiltinDialect::DuckDb)
        );
        assert_eq!(
            BuiltinDialect::from_name("duck"),
            Some(BuiltinDialect::DuckDb)
        );
        assert_eq!(BuiltinDialect::DuckDb.features(), &FeatureSet::DUCKDB);
        // The `0x` radix integer form is a DuckDB-only lexical widening over ANSI: with
        // the trailing `+ 1` forcing the hex reading it parses under the runtime-selected
        // DuckDB builtin and rejects under ANSI (which lexes `0 AS xFF` then trailing `+`).
        assert!(parse_with_builtin("SELECT 0xFF + 1", BuiltinDialect::DuckDb).is_ok());
        assert!(parse_with_builtin("SELECT 0xFF + 1", BuiltinDialect::Ansi).is_err());
        // The bare `SELECT` empty-target list is the DuckDB tightening: rejected under
        // DuckDB, accepted under the runtime-selected Postgres builtin.
        assert!(parse_with_builtin("SELECT", BuiltinDialect::DuckDb).is_err());
    }

    #[cfg(feature = "bigquery")]
    #[test]
    fn bigquery_builtin_resolves_and_parses_its_surface() {
        assert_eq!(
            BuiltinDialect::from_name("BigQuery"),
            Some(BuiltinDialect::BigQuery)
        );
        // Both alternate names resolve — `bq` and `zetasql` — alongside canonical `bigquery`.
        assert_eq!(
            BuiltinDialect::from_name("bq"),
            Some(BuiltinDialect::BigQuery)
        );
        assert_eq!(
            BuiltinDialect::from_name("ZetaSQL"),
            Some(BuiltinDialect::BigQuery)
        );
        assert_eq!(BuiltinDialect::BigQuery.features(), &FeatureSet::BIGQUERY);
        // `UNNEST(...) WITH OFFSET` is the BigQuery-only surface this preset makes real: it
        // parses under the runtime-selected BigQuery builtin and is rejected under ANSI (which
        // has no first-class UNNEST factor at all).
        assert!(
            parse_with_builtin(
                "SELECT * FROM UNNEST(arr) WITH OFFSET AS pos",
                BuiltinDialect::BigQuery
            )
            .is_ok()
        );
        assert!(
            parse_with_builtin(
                "SELECT * FROM UNNEST(arr) WITH OFFSET AS pos",
                BuiltinDialect::Ansi
            )
            .is_err()
        );
    }

    #[cfg(feature = "hive")]
    #[test]
    fn hive_builtin_resolves_and_parses_its_surface() {
        assert_eq!(
            BuiltinDialect::from_name("Hive"),
            Some(BuiltinDialect::Hive)
        );
        // The alternate name resolves — `hiveql` — alongside canonical `hive`.
        assert_eq!(
            BuiltinDialect::from_name("HiveQL"),
            Some(BuiltinDialect::Hive)
        );
        assert_eq!(BuiltinDialect::Hive.features(), &FeatureSet::HIVE);
        // The sided `LEFT SEMI JOIN` is the Hive-originated surface this preset makes real: it
        // parses under the runtime-selected Hive builtin and is rejected under ANSI (which has
        // no sided semi-/anti-join spelling).
        assert!(
            parse_with_builtin(
                "SELECT * FROM a LEFT SEMI JOIN b ON a.x = b.x",
                BuiltinDialect::Hive
            )
            .is_ok()
        );
        assert!(
            parse_with_builtin(
                "SELECT * FROM a LEFT SEMI JOIN b ON a.x = b.x",
                BuiltinDialect::Ansi
            )
            .is_err()
        );
    }

    #[cfg(feature = "clickhouse")]
    #[test]
    fn clickhouse_builtin_resolves_and_parses_its_surface() {
        assert_eq!(
            BuiltinDialect::from_name("ClickHouse"),
            Some(BuiltinDialect::ClickHouse)
        );
        assert_eq!(
            BuiltinDialect::from_name("ch"),
            Some(BuiltinDialect::ClickHouse)
        );
        assert_eq!(
            BuiltinDialect::ClickHouse.features(),
            &FeatureSet::CLICKHOUSE
        );
        // The ClickHouse `SETTINGS` query tail is a ClickHouse-only clause: it parses
        // under the runtime-selected ClickHouse builtin and is rejected under ANSI (which
        // leaves the trailing `SETTINGS …` unconsumed).
        assert!(
            parse_with_builtin(
                "SELECT a FROM t SETTINGS max_threads = 8",
                BuiltinDialect::ClickHouse
            )
            .is_ok()
        );
        assert!(
            parse_with_builtin(
                "SELECT a FROM t SETTINGS max_threads = 8",
                BuiltinDialect::Ansi
            )
            .is_err()
        );
    }

    #[cfg(feature = "snowflake")]
    #[test]
    fn snowflake_builtin_resolves_and_parses_its_surface() {
        assert_eq!(
            BuiltinDialect::from_name("Snowflake"),
            Some(BuiltinDialect::Snowflake)
        );
        assert_eq!(
            BuiltinDialect::from_name("sf"),
            Some(BuiltinDialect::Snowflake)
        );
        assert_eq!(BuiltinDialect::Snowflake.features(), &FeatureSet::SNOWFLAKE);
        // The `base:key` semi-structured path is the Snowflake-only accessor this preset
        // exposes: it parses under the runtime-selected Snowflake builtin and is rejected
        // under ANSI (where `:` after an identifier is unexpected).
        assert!(
            parse_with_builtin("SELECT src:customer[0].name", BuiltinDialect::Snowflake).is_ok()
        );
        assert!(parse_with_builtin("SELECT src:customer[0].name", BuiltinDialect::Ansi).is_err());
    }

    #[cfg(feature = "redshift")]
    #[test]
    fn redshift_builtin_resolves_and_defers_its_pg_heritage_surface() {
        assert_eq!(
            BuiltinDialect::from_name("Redshift"),
            Some(BuiltinDialect::Redshift)
        );
        // The alternate name resolves — `amazonredshift` — alongside canonical `redshift`.
        assert_eq!(
            BuiltinDialect::from_name("AmazonRedshift"),
            Some(BuiltinDialect::Redshift)
        );
        // `rs` is deliberately not a Redshift alias (too generic).
        assert_eq!(BuiltinDialect::from_name("rs"), None);
        assert_eq!(BuiltinDialect::Redshift.features(), &FeatureSet::REDSHIFT);
        // Redshift's ANSI-verbatim grammar parses the shared surface, and its lexis is standard —
        // `"…"` is a quoted identifier just like ANSI (unlike Hive/BigQuery).
        assert!(parse_with_builtin("SELECT \"Col\" FROM t", BuiltinDialect::Redshift).is_ok());
        // The conservative-off deferral, pinned at the runtime layer: the PostgreSQL-heritage
        // `ILIKE` Redshift genuinely accepts is deferred here, so it rejects under the
        // runtime-selected Redshift builtin exactly as under ANSI — while Postgres accepts it.
        assert!(
            parse_with_builtin(
                "SELECT * FROM t WHERE name ILIKE 'a%'",
                BuiltinDialect::Redshift
            )
            .is_err()
        );
    }

    #[cfg(feature = "databricks")]
    #[test]
    fn databricks_builtin_resolves_and_parses_its_surface() {
        assert_eq!(
            BuiltinDialect::from_name("Databricks"),
            Some(BuiltinDialect::Databricks)
        );
        assert_eq!(
            BuiltinDialect::from_name("dbx"),
            Some(BuiltinDialect::Databricks)
        );
        assert_eq!(
            BuiltinDialect::Databricks.features(),
            &FeatureSet::DATABRICKS
        );
        // The sided `LEFT SEMI JOIN` is the Databricks-only join family this preset makes
        // real: it parses under the runtime-selected Databricks builtin and is rejected
        // under ANSI (where `SEMI` after `LEFT` is unexpected).
        assert!(
            parse_with_builtin(
                "SELECT * FROM a LEFT SEMI JOIN b ON a.x = b.x",
                BuiltinDialect::Databricks
            )
            .is_ok()
        );
        assert!(
            parse_with_builtin(
                "SELECT * FROM a LEFT SEMI JOIN b ON a.x = b.x",
                BuiltinDialect::Ansi
            )
            .is_err()
        );
    }

    #[cfg(feature = "mssql")]
    #[test]
    fn mssql_builtin_resolves_and_parses_its_surface() {
        assert_eq!(
            BuiltinDialect::from_name("MSSQL"),
            Some(BuiltinDialect::Mssql)
        );
        // Both alternate names resolve — `tsql` and `sqlserver` — alongside canonical `mssql`.
        assert_eq!(
            BuiltinDialect::from_name("tsql"),
            Some(BuiltinDialect::Mssql)
        );
        assert_eq!(
            BuiltinDialect::from_name("SqlServer"),
            Some(BuiltinDialect::Mssql)
        );
        assert_eq!(BuiltinDialect::Mssql.features(), &FeatureSet::MSSQL);
        // `CROSS APPLY` is the MSSQL-only join family this preset makes real: it parses under
        // the runtime-selected MSSQL builtin and is rejected under ANSI (where `APPLY` in join
        // position is unexpected).
        assert!(
            parse_with_builtin(
                "SELECT * FROM a CROSS APPLY (SELECT 1) AS t",
                BuiltinDialect::Mssql
            )
            .is_ok()
        );
        assert!(
            parse_with_builtin(
                "SELECT * FROM a CROSS APPLY (SELECT 1) AS t",
                BuiltinDialect::Ansi
            )
            .is_err()
        );
    }

    #[cfg(feature = "lenient")]
    #[test]
    fn lenient_builtin_resolves_and_parses_its_union() {
        assert_eq!(
            BuiltinDialect::from_name("Lenient"),
            Some(BuiltinDialect::Lenient)
        );
        assert_eq!(
            BuiltinDialect::from_name("permissive"),
            Some(BuiltinDialect::Lenient)
        );
        assert_eq!(BuiltinDialect::Lenient.features(), &FeatureSet::LENIENT);
        // The multi-quote union is the distinctive capability: all three identifier
        // styles parse under the runtime-selected Lenient builtin, where ANSI rejects the
        // non-standard ones.
        assert!(
            parse_with_builtin(r#"SELECT "a", `b`, [c] FROM t"#, BuiltinDialect::Lenient).is_ok()
        );
        assert!(parse_with_builtin("SELECT `b` FROM t", BuiltinDialect::Ansi).is_err());
    }
}
