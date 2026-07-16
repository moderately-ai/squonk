// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The Databricks dialect.
//!
//! The whole module is gated by the `databricks` cargo feature (one `#[cfg]` on its `mod`
//! declaration), so the struct, the `Dialect` impl, and the Databricks test cluster compile
//! only when the feature is on — no per-item gates.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;

/// The Databricks dialect ([`FeatureSet::DATABRICKS`]).
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(Databricks))`.
/// Databricks is exposed as a deliberately conservative ANSI-derived preset (no Databricks
/// oracle exists to fit a wider surface). Over ANSI it currently enables (non-exhaustive
/// headline list — full closed delta in [`FeatureSet::DATABRICKS`]): the sided
/// `{LEFT|RIGHT} {SEMI|ANTI} JOIN` family, semi-structured colon path access
/// (`base:key[0].field`), `QUALIFY` (reserved as an identifier), `GROUP BY ALL` /
/// `ORDER BY ALL`, Spark-style `LATERAL VIEW`, `VERSION`/`TIMESTAMP AS OF` table time
/// travel, and `SHOW FUNCTIONS`. Identifiers accept MySQL-style backticks and standard
/// `"…"`; unquoted case is preserved at parse time. Deferred surfaces (side-less
/// `SEMI JOIN`, stage/`@` paths, `$` templates, broader `MERGE` extensions, …) remain
/// clean rejects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Databricks;

impl Dialect for Databricks {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::DATABRICKS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::test_support::assert_full_grammar;
    use crate::dialect::{Ansi, DuckDb, MySql, Postgres, Sqlite};
    use crate::parse_with;
    use crate::parser::Dialect;

    #[test]
    fn databricks_parses_the_full_m1_select_grammar() {
        // The representative query uses only constructs Databricks shares with ANSI, so it
        // drives the full shared grammar identically — the structural proof.
        assert_full_grammar(Databricks);
    }

    /// The five oracle-compared shipped presets, none of which must accept the given
    /// Databricks-only surface (they are pinned; the boundary tests assert the reject
    /// against each so a future preset edit cannot silently move one).
    fn rejects_under_every_oracle_preset(sql: &str) {
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI must reject the Databricks-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL must reject the Databricks-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL must reject the Databricks-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite must reject the Databricks-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB must reject the Databricks-only form {sql:?}",
        );
    }

    /// The four oracle presets that lack `QUALIFY` / `GROUP BY ALL` / `ORDER BY ALL`.
    /// Unlike the semi-structured and sided-join surface, these three clauses are *shared
    /// with DuckDB* (which enables all three flags), so the boundary here excludes DuckDB —
    /// the reject is asserted only against the presets that genuinely lack the clause.
    fn rejects_under_the_non_duckdb_oracle_presets(sql: &str) {
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI must reject {sql:?}"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL must reject {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL must reject {sql:?}"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite must reject {sql:?}"
        );
    }

    #[test]
    fn databricks_parses_sided_semi_anti_joins_that_oracle_presets_reject() {
        // The capstone this preset makes real: the sided `{LEFT|RIGHT} {SEMI|ANTI} JOIN`
        // family. `sided_semi_anti_join` is off in every oracle-compared preset — DuckDB
        // ships only the side-less `SEMI JOIN` and parse-rejects the sided spelling — so
        // each parses under Databricks and rejects under all five.
        for sql in [
            "SELECT * FROM a LEFT SEMI JOIN b ON a.x = b.x",
            "SELECT * FROM a LEFT ANTI JOIN b ON a.x = b.x",
            "SELECT * FROM a RIGHT SEMI JOIN b ON a.x = b.x",
            "SELECT * FROM a RIGHT ANTI JOIN b USING (x)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Databricks)).is_ok(),
                "Databricks parses {sql:?}"
            );
            rejects_under_every_oracle_preset(sql);
        }
    }

    #[test]
    fn databricks_parses_semi_structured_paths_that_oracle_presets_reject() {
        // The `base:key[0].field` colon path over semi-structured columns.
        // `semi_structured_access` is off in every oracle-compared preset (and its `:`
        // trigger is claimed by SQLite's colon parameters instead), so the four non-DuckDB
        // presets reject it. DuckDB is the honest exception: with `prefix_colon_alias` a
        // leading `<ident>:` at a select-item head reads as a PREFIX ALIAS (`key AS base`),
        // not a path — the documented parser-position collision between the two `:` readings
        // (semi-structured path vs prefix alias) — so DuckDB accepts those bytes with a
        // *different* meaning. Verified against DuckDB 1.5.4: `SELECT src:customer` yields a
        // COLUMN_REF `customer` aliased `src`.
        for sql in ["SELECT src:customer", "SELECT src:customer[0].name"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Databricks)).is_ok(),
                "Databricks parses {sql:?} as a semi-structured path"
            );
            rejects_under_the_non_duckdb_oracle_presets(sql);
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB reinterprets {sql:?} as a prefix colon alias, not a path",
            );
        }
        // A `:` *inside* an expression (`a + src:customer`) is not a select-item head, so it
        // is neither a Databricks path base nor a DuckDB prefix-alias position — every oracle
        // preset, DuckDB included, syntax-rejects it (verified on DuckDB 1.5.4). Databricks
        // still reads the `src:customer` operand as a path.
        let mid = "SELECT a + src:customer FROM t";
        assert!(
            parse_with(mid, crate::ParseConfig::new(Databricks)).is_ok(),
            "Databricks parses {mid:?}"
        );
        rejects_under_every_oracle_preset(mid);
    }

    #[test]
    fn databricks_parses_qualify_group_and_order_by_all_that_the_non_duckdb_presets_reject() {
        // `QUALIFY`, `GROUP BY ALL`, and `ORDER BY ALL` parse under Databricks and are
        // rejected by the four oracle presets that lack them; DuckDB shares all three (its
        // own flags), so it is excluded from the reject set — the honest boundary for
        // shared surface.
        for sql in [
            "SELECT a FROM t QUALIFY row_number() OVER () = 1",
            "SELECT a, count(*) FROM t GROUP BY ALL",
            "SELECT a FROM t ORDER BY ALL",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Databricks)).is_ok(),
                "Databricks parses {sql:?}"
            );
            rejects_under_the_non_duckdb_oracle_presets(sql);
            // The shared-surface half: DuckDB accepts these too (all three flags on there).
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB shares {sql:?} (its own QUALIFY / GROUP BY ALL / ORDER BY ALL)",
            );
        }
    }

    #[test]
    fn databricks_reserves_qualify_as_an_identifier() {
        // `QUALIFY` is reserved in every identifier position under Databricks (matching
        // Spark's ANSI reserved-keyword parser), which is what lets the clause parse in the
        // natural `FROM t QUALIFY …` position instead of reading `QUALIFY` as a table alias.
        assert!(
            parse_with(
                "SELECT a FROM t QUALIFY row_number() OVER () = 1",
                crate::ParseConfig::new(Databricks)
            )
            .is_ok(),
            "the bare `FROM t QUALIFY …` form parses the clause, not an alias",
        );
        // The reservation side: `qualify` is not usable as a bare column name under
        // Databricks, while ANSI (which does not reserve it) reads it as an ordinary column.
        assert!(
            parse_with("SELECT qualify FROM t", crate::ParseConfig::new(Databricks)).is_err(),
            "Databricks rejects `qualify` as a column name (reserved)",
        );
        assert!(
            parse_with("SELECT qualify FROM t", crate::ParseConfig::new(Ansi)).is_ok(),
            "ANSI reads `qualify` as an ordinary column name (unreserved)",
        );
    }

    #[test]
    fn databricks_quotes_identifiers_with_backticks() {
        // Databricks quotes identifiers with the MySQL-style backtick and the standard
        // `"…"`. The backtick form is the signature Spark/Databricks spelling; ANSI (which
        // has no backtick quote) rejects it.
        assert!(
            parse_with("SELECT `a b` FROM t", crate::ParseConfig::new(Databricks)).is_ok(),
            "Databricks reads a backtick-quoted identifier",
        );
        assert!(
            parse_with("SELECT `a b` FROM t", crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI has no backtick identifier quote",
        );
        // `"…"` stays an identifier quote (not a string), so `double_quoted_strings` is off.
        assert!(
            parse_with("SELECT \"col\" FROM t", crate::ParseConfig::new(Databricks)).is_ok(),
            "Databricks reads a double-quoted identifier",
        );
    }

    #[test]
    fn databricks_features_are_the_databricks_preset() {
        assert_eq!(Databricks.features(), &FeatureSet::DATABRICKS);
    }
}
