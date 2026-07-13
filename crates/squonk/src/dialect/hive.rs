// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The Hive / HiveQL dialect.
//!
//! The whole module is gated by the `hive` cargo feature (one `#[cfg]` on its `mod`
//! declaration), so the struct, the `Dialect` impl, and the Hive test cluster compile only when
//! the feature is on — no per-item gates.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;

/// The Hive / HiveQL dialect ([`FeatureSet::HIVE`]).
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(Hive))`. Hive is exposed
/// as a deliberately conservative ANSI-derived preset (no Hive oracle exists to fit a wider
/// surface): it adds the sided `{LEFT|RIGHT} {SEMI|ANTI} JOIN` family (Hive originated
/// `LEFT SEMI JOIN`), backtick-quoted identifiers (`` `name` ``), and `"…"` double-quoted
/// string constants (HiveQL string literals may be written with single or double quotes,
/// reserving the backtick for identifiers). Unquoted identifiers fold case-insensitively for
/// identity. The remaining Hive surface (`LATERAL VIEW`, the `STRUCT`/`ARRAY<…>`/`MAP<…>`
/// complex types, `TRANSFORM`/`MAP`/`REDUCE` script operators,
/// `DISTRIBUTE BY`/`SORT BY`/`CLUSTER BY`, `TABLESAMPLE` bucketing, the side-less `SEMI JOIN`
/// spelling, backslash string escapes, …) is owned by follow-up grammar tickets and not yet
/// accepted here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Hive;

impl Dialect for Hive {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::HIVE
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
    fn hive_parses_the_full_m1_select_grammar() {
        // The representative query uses only constructs Hive shares with ANSI, so it drives the
        // full shared grammar identically — the structural proof.
        assert_full_grammar(Hive);
    }

    /// The five oracle-compared shipped presets, none of which must accept the given Hive-only
    /// surface (they are pinned; the boundary tests assert the reject against each so a future
    /// preset edit cannot silently move one).
    fn rejects_under_every_oracle_preset(sql: &str) {
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI must reject the Hive-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL must reject the Hive-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL must reject the Hive-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite must reject the Hive-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB must reject the Hive-only form {sql:?}",
        );
    }

    #[test]
    fn hive_parses_sided_semi_anti_joins_that_oracle_presets_reject() {
        // The capstone this preset makes real: the sided `{LEFT|RIGHT} {SEMI|ANTI} JOIN` family
        // Hive originated. `sided_semi_anti_join` is off in every oracle-compared preset —
        // DuckDB ships only the side-less `SEMI JOIN` and parse-rejects the sided spelling — so
        // each parses under Hive and rejects under all five. Hive classic documents only the
        // `LEFT SEMI` form; the atomic flag also admits the `RIGHT`/`ANTI` spellings (a known
        // conservative-direction over-acceptance deferred on the owning ticket), exercised here
        // so the flag's whole grammar surface is pinned.
        for sql in [
            "SELECT * FROM a LEFT SEMI JOIN b ON a.x = b.x",
            "SELECT * FROM a LEFT ANTI JOIN b ON a.x = b.x",
            "SELECT * FROM a RIGHT SEMI JOIN b ON a.x = b.x",
            "SELECT * FROM a RIGHT ANTI JOIN b USING (x)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Hive)).is_ok(),
                "Hive parses {sql:?}"
            );
            rejects_under_every_oracle_preset(sql);
        }
    }

    #[test]
    fn hive_quotes_identifiers_with_backticks_not_double_quotes() {
        // Hive's signature `` `name` `` backtick identifier. Among the oracle presets MySQL
        // (and SQLite, for MySQL compatibility) share the backtick quote, so the reject holds
        // only for ANSI/PostgreSQL/DuckDB; the shared half is asserted explicitly.
        let backtick = "SELECT `a b` FROM t";
        assert!(
            parse_with(backtick, crate::ParseConfig::new(Hive)).is_ok(),
            "Hive reads `` `a b` `` as an identifier",
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
    fn hive_reads_double_quotes_as_strings_not_identifiers() {
        // Hive reserves the backtick for identifiers, so `"…"` is a *string* constant, not a
        // quoted identifier. This is a meaning divergence rather than a parse boundary: the same
        // source parses under ANSI too, but there `"hi"` is a quoted *identifier* (a column
        // reference). Introspecting the projected node proves the string reading.
        let parsed = parse_with("SELECT \"hi\"", crate::ParseConfig::new(Hive))
            .expect("Hive parses `\"hi\"`");
        let SelectItem::Expr { expr, .. } = &select_projection(&parsed)[0] else {
            panic!("expected a bare projection expression");
        };
        let Expr::Literal { literal, .. } = expr else {
            panic!("Hive must read `\"hi\"` as a string literal, got {expr:?}");
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
    fn hive_features_are_the_hive_preset() {
        assert_eq!(Hive.features(), &FeatureSet::HIVE);
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
