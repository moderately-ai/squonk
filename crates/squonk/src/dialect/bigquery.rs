// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The BigQuery / ZetaSQL dialect.
//!
//! The whole module is gated by the `bigquery` cargo feature (one `#[cfg]` on its `mod`
//! declaration), so the struct, the `Dialect` impl, and the BigQuery test cluster compile only
//! when the feature is on — no per-item gates.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;

/// The BigQuery / ZetaSQL dialect ([`FeatureSet::BIGQUERY`]).
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(BigQuery))`. BigQuery is
/// exposed as a deliberately conservative ANSI-derived preset (no BigQuery oracle exists to fit
/// a wider surface): it adds the first-class `UNNEST(<expr>)` table factor with its
/// `WITH OFFSET [AS <alias>]` tail, the `STRUCT(...)` value constructor (`STRUCT(1, 2)`,
/// `STRUCT(x AS a)`, `STRUCT<a INT64>(1)`), backtick-quoted identifiers (`` `name` ``), and `"…"`
/// double-quoted string constants (BigQuery quotes strings with both `'…'` and `"…"`, reserving
/// the backtick for identifiers). Unquoted identifiers fold case-insensitively for identity. The
/// remaining BigQuery surface (query pipe syntax `|>`, the `STRUCT<…>`/`ARRAY<…>` *type-position*
/// surface (`CAST(x AS STRUCT<…>)`, column types), `SELECT AS STRUCT/VALUE`,
/// `EXCEPT`/`REPLACE` in `SELECT *`, `QUALIFY`, the `SAFE.` prefix, …)
/// is owned by follow-up grammar tickets and not yet accepted here — `|>` pipe syntax
/// deliberately so, until the pipe-operator surface is coherent (see the preset module docs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BigQuery;

impl Dialect for BigQuery {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::BIGQUERY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, SelectItem};
    use crate::dialect::test_support::assert_full_grammar;
    use crate::dialect::{Ansi, DuckDb, MySql, Postgres, Sqlite};
    use crate::parse_with;
    use crate::parser::{Dialect, Parsed};

    #[test]
    fn bigquery_parses_the_full_m1_select_grammar() {
        // The representative query uses only constructs BigQuery shares with ANSI, so it drives
        // the full shared grammar identically — the structural proof.
        assert_full_grammar(BigQuery);
    }

    #[test]
    fn bigquery_parses_unnest_with_offset_that_every_oracle_preset_rejects() {
        // The capstone this preset makes real: the BigQuery `WITH OFFSET [AS <alias>]` tail on a
        // first-class `UNNEST` factor. `unnest_with_offset` is off in every oracle-compared
        // preset — ANSI/MySQL/SQLite lack the `UNNEST` factor outright, and PostgreSQL/DuckDB
        // parse `UNNEST(…)` but parse-*reject* the `WITH OFFSET` tail (engine-probed, per the
        // flag doc) — so each full statement parses under BigQuery and rejects under all five.
        // The argument is a bare column reference, not `ARRAY[…]`: this preset keeps ANSI's
        // `array_constructor` off, so the array-literal form is itself unsupported here.
        for sql in [
            "SELECT * FROM UNNEST(arr) WITH OFFSET",
            "SELECT * FROM UNNEST(arr) WITH OFFSET AS pos",
            "SELECT * FROM t CROSS JOIN UNNEST(t.items) WITH OFFSET AS ord",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(BigQuery)).is_ok(),
                "BigQuery parses {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI has no UNNEST factor, so it rejects {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL parses UNNEST but rejects the WITH OFFSET tail in {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "MySQL has no UNNEST factor, so it rejects {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
                "SQLite has no UNNEST factor, so it rejects {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB parses UNNEST but rejects the WITH OFFSET tail in {sql:?}",
            );
        }
    }

    #[test]
    fn bigquery_parses_bare_unnest_without_offset() {
        // The base `unnest` gate: `FROM UNNEST(<expr>)` with no tail parses on its own (the
        // BigQuery FROM surface the WITH OFFSET tail rides). PostgreSQL/DuckDB share the bare
        // factor; the three ANSI-string presets reject it — the boundary that matters here is
        // simply that BigQuery accepts the base form.
        for sql in ["SELECT * FROM UNNEST(arr)", "SELECT * FROM UNNEST(a, b)"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(BigQuery)).is_ok(),
                "BigQuery parses {sql:?}"
            );
        }
    }

    #[test]
    fn bigquery_quotes_identifiers_with_backticks_not_double_quotes() {
        // BigQuery's signature `` `name` `` backtick identifier. Among the oracle presets MySQL
        // (and SQLite, for MySQL/T-SQL compatibility) share the backtick quote, so the reject
        // holds only for ANSI/PostgreSQL/DuckDB; the shared half is asserted explicitly.
        let backtick = "SELECT `a b` FROM t";
        assert!(
            parse_with(backtick, crate::ParseConfig::new(BigQuery)).is_ok(),
            "BigQuery reads `` `a b` `` as an identifier",
        );
        assert!(
            parse_with(backtick, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI has no backtick identifier quote",
        );
        assert!(
            parse_with(backtick, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL has no backtick identifier quote",
        );
        assert!(
            parse_with(backtick, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB has no backtick identifier quote",
        );
        // The shared half: MySQL and SQLite accept backtick identifiers too.
        assert!(
            parse_with(backtick, crate::ParseConfig::new(MySql)).is_ok(),
            "MySQL shares the backtick identifier quote",
        );
        assert!(
            parse_with(backtick, crate::ParseConfig::new(Sqlite)).is_ok(),
            "SQLite shares the backtick identifier quote (MySQL compat)",
        );
    }

    #[test]
    fn bigquery_reads_double_quotes_as_strings_not_identifiers() {
        // BigQuery reserves the backtick for identifiers, so `"…"` is a *string* constant, not a
        // quoted identifier. This is a meaning divergence rather than a parse boundary: the same
        // source parses under ANSI too, but there `"hi"` is a quoted *identifier* (a column
        // reference). Introspecting the projected node proves the string reading.
        let parsed = parse_with("SELECT \"hi\"", crate::ParseConfig::new(BigQuery))
            .expect("BigQuery parses `\"hi\"`");
        let SelectItem::Expr { expr, .. } = &select_projection(&parsed)[0] else {
            panic!("expected a bare projection expression");
        };
        let Expr::Literal { literal, .. } = expr else {
            panic!("BigQuery must read `\"hi\"` as a string literal, got {expr:?}");
        };
        assert_eq!(literal.as_str(parsed.source()).expect("string value"), "hi",);

        // Under ANSI the identical source reads `"hi"` as a quoted identifier — a column
        // reference, never a literal — which is the divergence this preset flips.
        let ansi_parsed = parse_with("SELECT \"hi\"", crate::ParseConfig::new(Ansi))
            .expect("ANSI parses `\"hi\"`");
        let SelectItem::Expr { expr, .. } = &select_projection(&ansi_parsed)[0] else {
            panic!("expected a bare projection expression");
        };
        assert!(
            !matches!(expr, Expr::Literal { .. }),
            "ANSI must read `\"hi\"` as an identifier, not a string literal",
        );
    }

    #[test]
    fn bigquery_features_are_the_bigquery_preset() {
        assert_eq!(BigQuery.features(), &FeatureSet::BIGQUERY);
    }

    /// The projection list of the sole SELECT statement in `parsed`.
    fn select_projection(parsed: &Parsed) -> &[SelectItem<NoExt>] {
        use crate::ast::{SetExpr, Statement};
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a plain SELECT body");
        };
        &select.projection
    }
}
