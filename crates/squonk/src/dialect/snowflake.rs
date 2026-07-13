// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The Snowflake dialect.
//!
//! The whole module is gated by the `snowflake` cargo feature (one `#[cfg]` on its `mod`
//! declaration), so the struct, the `Dialect` impl, and the Snowflake test cluster compile
//! only when the feature is on — no per-item gates.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;

/// The Snowflake dialect ([`FeatureSet::SNOWFLAKE`]).
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(Snowflake))`.
/// Snowflake is exposed as a deliberately conservative ANSI-derived preset (no Snowflake
/// oracle exists to fit a wider surface): it adds semi-structured path access
/// (`base:key[0].field` over `VARIANT`/`OBJECT`/`ARRAY` columns), the `QUALIFY <predicate>`
/// post-window filter (with `QUALIFY` reserved as an identifier, matching Snowflake's
/// reserved-keyword list), the `GROUP BY ALL` clause mode, and the `COPY INTO <target>
/// FROM <source>` bulk load/unload statement (table and external-location endpoints,
/// `FILE_FORMAT = (...)`/`FILES`/`PATTERN`/`VALIDATION_MODE` and the copy options).
/// Snowflake folds unquoted identifiers to uppercase and quotes with `"…"`, both already
/// the ANSI baseline. The remaining Snowflake surface (`FLATTEN`, `LATERAL` table
/// functions, `MATCH_RECOGNIZE`, `$$…$$` string literals, stage/`@`-path references —
/// including as `COPY INTO` endpoints — the `//` line comment, `ILIKE`, `CONNECT BY`, …)
/// is owned by follow-up grammar tickets and not yet accepted here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Snowflake;

impl Dialect for Snowflake {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::SNOWFLAKE
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
    fn snowflake_parses_the_full_m1_select_grammar() {
        // The representative query uses only constructs Snowflake shares with ANSI, so it
        // drives the full shared grammar identically — the structural proof.
        assert_full_grammar(Snowflake);
    }

    /// The five oracle-compared shipped presets, none of which must accept the
    /// Snowflake-only surface (they are pinned; the boundary tests assert the reject
    /// against each so a future preset edit cannot silently move one).
    fn rejects_under_every_oracle_preset(sql: &str) {
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI must reject the Snowflake-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL must reject the Snowflake-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL must reject the Snowflake-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite must reject the Snowflake-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB must reject the Snowflake-only form {sql:?}",
        );
    }

    #[test]
    fn snowflake_parses_semi_structured_paths_that_oracle_presets_reject() {
        // The capstone this preset exists to expose: the `base:key[0].field` path over
        // semi-structured columns. `semi_structured_access` is off in every oracle-compared
        // preset (and its `:` trigger is claimed by SQLite's colon parameters instead), so
        // the four non-DuckDB presets reject it. DuckDB is the honest exception: with
        // `prefix_colon_alias` a leading `<ident>:` at a select-item head reads as a PREFIX
        // ALIAS (`key AS base`), not a path — the documented parser-position collision
        // between the two `:` readings — so DuckDB accepts those bytes with a *different*
        // meaning (verified on DuckDB 1.5.4: `SELECT src:customer` → COLUMN_REF aliased `src`).
        for sql in ["SELECT src:customer", "SELECT src:customer[0].name"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Snowflake)).is_ok(),
                "Snowflake parses {sql:?} as a semi-structured path"
            );
            rejects_under_the_non_duckdb_oracle_presets(sql);
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB reinterprets {sql:?} as a prefix colon alias, not a path",
            );
        }
        // A `:` *inside* an expression (`a + src:customer`) is not a select-item head, so it
        // is neither a path base nor a DuckDB prefix-alias position — every oracle preset,
        // DuckDB included, syntax-rejects it (verified on DuckDB 1.5.4).
        let mid = "SELECT a + src:customer FROM t";
        assert!(
            parse_with(mid, crate::ParseConfig::new(Snowflake)).is_ok(),
            "Snowflake parses {mid:?}"
        );
        rejects_under_every_oracle_preset(mid);
    }

    /// The four oracle presets that lack `QUALIFY` / `GROUP BY ALL`. Unlike the
    /// semi-structured surface, these two clauses are *shared with DuckDB* (which enables
    /// both flags), so the boundary here excludes DuckDB — the reject is asserted only
    /// against the presets that genuinely lack the clause.
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
    fn snowflake_parses_qualify_and_group_by_all_that_the_non_duckdb_presets_reject() {
        // `QUALIFY` and `GROUP BY ALL` parse under Snowflake and are rejected by the four
        // oracle presets that lack them; DuckDB shares both (its own flags), so it is
        // excluded from the reject set — the honest boundary for shared surface.
        for sql in [
            "SELECT a FROM t QUALIFY row_number() OVER () = 1",
            "SELECT a, count(*) FROM t GROUP BY ALL",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Snowflake)).is_ok(),
                "Snowflake parses {sql:?}"
            );
            rejects_under_the_non_duckdb_oracle_presets(sql);
            // The shared-surface half: DuckDB accepts these too (both flags on there).
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB shares {sql:?} (its own QUALIFY / GROUP BY ALL)",
            );
        }
    }

    #[test]
    fn snowflake_parses_copy_into_that_every_oracle_preset_rejects() {
        // The `copy_into`-gated Snowflake load/unload statement, in both directions with
        // the nested `FILE_FORMAT` list. No oracle preset has the `COPY INTO` grammar:
        // ANSI/MySQL/SQLite gate the leading `COPY` off entirely, and PostgreSQL/DuckDB
        // read `INTO` as the PostgreSQL COPY's table position (a reserved word there) and
        // reject — so the boundary holds against all five.
        for sql in [
            "COPY INTO t FROM 's3://bucket/data/' FILE_FORMAT = (TYPE = CSV SKIP_HEADER = 1) ON_ERROR = CONTINUE",
            "COPY INTO 's3://bucket/unload/' FROM t OVERWRITE = TRUE",
            "COPY INTO t FROM (SELECT a FROM src) PATTERN = '.*[.]csv'",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Snowflake))
                .unwrap_or_else(|err| panic!("Snowflake parses {sql:?}: {err:?}"));
            // The `KEY = VALUE` spellings render back from their spans verbatim. The
            // `Snowflake` unit dialect carries no `RenderDialect` impl (no Snowflake
            // Tier-1 spelling yet), so render through the equivalent feature set.
            let rendered = crate::render::Renderer::new(crate::parser::FeatureDialect {
                features: &FeatureSet::SNOWFLAKE,
            })
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "COPY INTO round-trips under Snowflake");
            rejects_under_every_oracle_preset(sql);
        }
    }

    #[test]
    fn snowflake_reserves_qualify_as_an_identifier() {
        // `QUALIFY` is reserved in every identifier position under Snowflake (its
        // reserved-keyword status), which is what lets the clause parse in the natural
        // `FROM t QUALIFY …` position instead of reading `QUALIFY` as a table alias.
        assert!(
            parse_with(
                "SELECT a FROM t QUALIFY row_number() OVER () = 1",
                crate::ParseConfig::new(Snowflake)
            )
            .is_ok(),
            "the bare `FROM t QUALIFY …` form parses the clause, not an alias",
        );
        // The reservation side: `qualify` is not usable as a bare column name under
        // Snowflake, while ANSI (which does not reserve it) reads it as an ordinary column.
        assert!(
            parse_with("SELECT qualify FROM t", crate::ParseConfig::new(Snowflake)).is_err(),
            "Snowflake rejects `qualify` as a column name (reserved)",
        );
        assert!(
            parse_with("SELECT qualify FROM t", crate::ParseConfig::new(Ansi)).is_ok(),
            "ANSI reads `qualify` as an ordinary column name (unreserved)",
        );
    }

    #[test]
    fn snowflake_features_are_the_snowflake_preset() {
        assert_eq!(Snowflake.features(), &FeatureSet::SNOWFLAKE);
    }

    #[test]
    fn snowflake_parses_stage_references_in_copy_into() {
        use crate::ast::{CopyIntoSource, CopyIntoTarget, Statement};

        for sql in [
            "COPY INTO t FROM @my_stage",
            "COPY INTO t FROM @my_stage/path/file.csv",
            "COPY INTO t FROM @~",
            "COPY INTO t FROM @~/data",
            "COPY INTO t FROM @%src_table",
            "COPY INTO @my_stage FROM t",
            "COPY INTO @db.schema.stage/path FROM (SELECT 1)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Snowflake))
                .unwrap_or_else(|e| panic!("must parse {sql:?}: {e:?}"));
            let Statement::CopyInto { copy, .. } = &parsed.statements()[0] else {
                panic!("expected CopyInto for {sql:?}");
            };
            let has_stage = matches!(copy.target, CopyIntoTarget::Stage { .. })
                || matches!(copy.source, CopyIntoSource::Stage { .. });
            assert!(has_stage, "expected a Stage endpoint in {sql:?}: {copy:?}");
            let rendered = crate::render::Renderer::new(crate::parser::FeatureDialect {
                features: &FeatureSet::SNOWFLAKE,
            })
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "stage COPY INTO round-trips under Snowflake");
        }
    }
}
