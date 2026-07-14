// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The public dialects ([`Ansi`], the feature-gated `Postgres`, `MySql`, `Sqlite`,
//! `DuckDb`, `Lenient`, and the conservative `BigQuery`, `Hive`, `ClickHouse`, `Databricks`,
//! `Mssql`, `Snowflake`, and `Redshift` presets) and the [`parse`] convenience.
//! [`BuiltinDialect::ALL`] is the authoritative selectable list.
//!
//! A dialect is *data*: each of these is a zero-sized unit struct whose
//! [`Dialect`] impl hands back a `&'static` borrow of a const [`FeatureSet`] preset.
//! Because the borrow of an associated const is promoted to `'static`, neither
//! struct stores a field, and the parser's `self.features().<field>` reads
//! const-fold under the monomorphized `Parser<D>`.
//!
//! Each dialect lives in its own module so the packaging story is one `#[cfg]` per
//! dialect: [`Ansi`] is the always-compiled baseline [`parse`] defaults to; every other
//! dialect is gated behind its own cargo feature (the whole module — struct + impls +
//! tests — compiles only with the feature). Reach a non-default dialect with
//! [`parse_with`], e.g.
//! `parse_with(src, crate::ParseConfig::new(Postgres))`, or pick one by runtime name/value with
//! [`BuiltinDialect`] and [`parse_builtin`].
//!
//! # Choosing a dialect: the permissiveness spectrum
//!
//! The dialects run from the strict standard to a parse-anything union: [`Ansi`] is
//! the strict SQL:2016 baseline (the honest "generic"), `Postgres`, `MySql`, `Sqlite`,
//! and `DuckDb` are the oracle-compared single-dialect surfaces (each held to its real
//! engine by a differential oracle), and `Lenient` (the `lenient` feature) is the
//! permissive "parse anything" catch-all. `BigQuery`, `Hive`, `ClickHouse`, `Databricks`,
//! `Mssql`, `Snowflake`, and `Redshift` are the conservative, no-oracle presets: each derives
//! from [`Ansi`] and enables only the surface with a modelled, tested parser gate and documentary
//! evidence, so — lacking an engine oracle to measure over-acceptance — they are
//! excluded from the oracle conformance sets and reject unmodelled syntax cleanly. (`Redshift`
//! derives from `Ansi` despite being a PostgreSQL-8 fork: our `Postgres` preset is fitted to
//! PG-17 and would over-accept features Redshift never had — see its module docs.) Note
//! for a `datafusion-sqlparser-rs`
//! drop-in: the runtime name `"generic"` aliases [`Ansi`] (strict standard, like that
//! crate's `AnsiDialect`), so a `GenericDialect` consumer wanting the permissive
//! catch-all should map to `Lenient`, not `"generic"` — see
//! [`BuiltinDialect::from_name`].
//!
//! # Tier-2 dialect rendering and source-spelled literals
//!
//! The Tier-2 [`Renderer`](crate::render::Renderer) lives in this crate:
//! it validates target support with a fallible diagnostic path and then delegates
//! accepted statements to the dependency-free Tier-1 renderer in `squonk-ast`.
//! PostgreSQL accepts string literal spellings that ANSI rejects
//! ([`FeatureSet::string_literals`](crate::ast::dialect::FeatureSet::string_literals)).
//! Literal rendering is dialect-independent: if a literal has source text, Tier-1
//! emits the original span byte-for-byte. Target-specific rewriting or rejection of
//! source-spelled literal forms belongs in the Tier-2 override layer; the AST keeps
//! one semantic literal shape.
//!
//! [`Dialect`]: crate::parser::Dialect
//! [`parse_with`]: crate::parse_with
//! [`FeatureSet`]: crate::ast::dialect::FeatureSet

use crate::error::ParseResult;
use crate::parser::{Parsed, parse_with};

mod ansi;
#[cfg(feature = "bigquery")]
mod bigquery;
mod builtin;
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
mod support;

pub use ansi::Ansi;
#[cfg(feature = "bigquery")]
pub use bigquery::BigQuery;
pub use builtin::{
    BuiltinDialect, ParseBuiltinDialectError, parse_builtin, parse_builtin_with,
    parse_recovering_builtin, parse_recovering_builtin_with, tokenize_with_builtin,
    tokenize_with_builtin_trivia,
};
#[cfg(feature = "clickhouse")]
pub use clickhouse::ClickHouse;
#[cfg(feature = "databricks")]
pub use databricks::Databricks;
#[cfg(feature = "duckdb")]
pub use duckdb::DuckDb;
#[cfg(feature = "hive")]
pub use hive::Hive;
#[cfg(feature = "lenient")]
pub use lenient::Lenient;
#[cfg(feature = "mssql")]
pub use mssql::Mssql;
#[cfg(feature = "mysql")]
pub use mysql::MySql;
#[cfg(feature = "postgres")]
pub use postgres::Postgres;
#[cfg(feature = "quiltdb")]
pub use quiltdb::QuiltDb;
#[cfg(feature = "redshift")]
pub use redshift::Redshift;
#[cfg(feature = "snowflake")]
pub use snowflake::Snowflake;
#[cfg(feature = "sqlite")]
pub use sqlite::Sqlite;
pub use support::ProductSurface;

/// Parse `src` under the default [`Ansi`] dialect into an owned [`Parsed`] tree.
///
/// To parse under a specific dialect, call [`parse_with`]
/// directly, e.g. `parse_with(src, crate::ParseConfig::new(Postgres))`, or select one by runtime name with
/// [`BuiltinDialect`]/[`parse_builtin`].
///
/// For high-throughput or alloc-heavy parsing, see the crate-level [Performance note](crate#performance) on choosing a fast global allocator.
///
/// # Errors
///
/// Returns the first [`ParseError`] — a lexical fault surfaced through the parse
/// channel, or a grammar error — exactly as [`parse_with`] does.
///
/// ```
/// let parsed = squonk::dialect::parse("SELECT 1").expect("`SELECT 1` parses");
/// assert_eq!(parsed.statements().len(), 1);
/// ```
///
/// [`ParseError`]: crate::error::ParseError
pub fn parse(src: &str) -> ParseResult<Parsed> {
    parse_with(src, crate::parser::ParseConfig::default())
}

/// Shared scaffolding for the per-dialect test modules: the representative query and
/// the generic grammar/type assertions each shipped dialect drives (every arm of
/// [`BuiltinDialect::ALL`], including the conservative no-oracle presets).
///
/// It lives in the parent module (rather than being duplicated per dialect) so each
/// per-dialect test module — gated with its dialect — can drive the same generic
/// helper. `pub(crate)` + `#[cfg(test)]` keep it test-only and crate-private.
#[cfg(test)]
pub(crate) mod test_support {
    use crate::ast::NoExt;
    use crate::ast::{
        CreateTableBody, DataType, SelectItem, SetExpr, SetOperator, Statement, TableElement,
    };
    use crate::parser::{Dialect, parse_with};

    /// A representative spread of the full M1 SELECT grammar in one query:
    /// projection with an alias, `FROM … JOIN … ON`, `WHERE`, `GROUP BY`,
    /// `HAVING`, a `UNION ALL` set operation, and query-level `ORDER BY` / `LIMIT`.
    /// Written in canonical form so the Tier-1 renderer round-trips it verbatim.
    pub(crate) const REPRESENTATIVE: &str = "SELECT a, b AS x \
        FROM t1 JOIN t2 ON t1.id = t2.id \
        WHERE a > 1 GROUP BY a HAVING a < 9 \
        UNION ALL SELECT c, d FROM t3 \
        ORDER BY x LIMIT 10 OFFSET 5";

    /// Parse [`REPRESENTATIVE`] under `dialect` and assert every listed construct is
    /// present. Shared by the per-dialect cases so each public dialect is shown
    /// driving the same full grammar (the exhaustive per-clause shape checks live with
    /// the engine in `parser::query`).
    pub(crate) fn assert_full_grammar<D: Dialect<Ext = NoExt>>(dialect: D) {
        let parsed = parse_with(REPRESENTATIVE, crate::ParseConfig::new(dialect))
            .expect("the representative query parses");
        assert_eq!(parsed.statements().len(), 1, "exactly one statement");

        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        // Query-level ORDER BY and LIMIT bind the whole set operation.
        assert_eq!(query.order_by.len(), 1, "one ORDER BY key");
        assert!(query.limit.is_some(), "a LIMIT/OFFSET tail");

        // The body is a `UNION ALL` of two SELECTs.
        let SetExpr::SetOperation {
            op: SetOperator::Union,
            all,
            left,
            ..
        } = &query.body
        else {
            panic!("expected a UNION set operation");
        };
        assert!(*all, "UNION ALL");

        // The left operand carries the per-SELECT clauses.
        let SetExpr::Select { select, .. } = &**left else {
            panic!("the left operand is a SELECT");
        };
        assert_eq!(select.projection.len(), 2, "two projection items");
        assert!(
            matches!(
                select.projection[1],
                SelectItem::Expr { alias: Some(_), .. }
            ),
            "the second item is aliased `b AS x`",
        );
        assert_eq!(select.from.len(), 1, "one FROM relation");
        assert_eq!(select.from[0].joins.len(), 1, "one JOIN … ON");
        assert!(select.selection.is_some(), "a WHERE predicate");
        assert_eq!(select.group_by.len(), 1, "one GROUP BY key");
        assert!(select.having.is_some(), "a HAVING predicate");
    }

    /// The declared type of the first column in `CREATE TABLE t (c <type>)` parsed
    /// under `dialect`. The column type grammar is shared with `CAST`, so this is the
    /// precise lens onto `parse_data_type`'s dialect-gated recognition.
    pub(crate) fn first_column_type<D: Dialect<Ext = NoExt>>(dialect: D, sql: &str) -> DataType {
        let parsed = parse_with(sql, crate::ParseConfig::new(dialect))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let Statement::CreateTable { create, .. } = &parsed.statements()[0] else {
            panic!("{sql:?}: expected a CREATE TABLE statement");
        };
        let CreateTableBody::Definition { elements, .. } = &create.body else {
            panic!("{sql:?}: expected a column-definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("{sql:?}: expected a column element");
        };
        column
            .data_type
            .clone()
            .unwrap_or_else(|| panic!("{sql:?}: the probed column has an explicit type"))
    }
}
