// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The Amazon Redshift dialect.
//!
//! The whole module is gated by the `redshift` cargo feature (one `#[cfg]` on its `mod`
//! declaration), so the struct, the `Dialect` impl, and the Redshift test cluster compile only
//! when the feature is on — no per-item gates.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;

/// The Amazon Redshift dialect ([`FeatureSet::REDSHIFT`]).
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(Redshift))`. Redshift is a
/// PostgreSQL-8 fork, but it is exposed here as a deliberately conservative *ANSI*-derived preset
/// (no Redshift oracle exists to fit a wider surface, and deriving from our PG-17-fitted
/// `Postgres` preset would silently over-accept features Redshift never had). Over ANSI it adds
/// two axes: (1) unquoted identifiers fold case-insensitively to lowercase (Redshift's default
/// `enable_case_sensitive_identifier` off) — an identity-only fold that never changes acceptance;
/// (2) table-position PartiQL / SUPER JSON path (`FROM src[0].a`, `table_json_path`). Its lexis
/// is standard ANSI: `"…"` quotes identifiers and `'…'` spells strings. The PostgreSQL-heritage
/// surface Redshift genuinely accepts (`ILIKE`, `SIMILAR TO`, `DISTINCT ON`, `QUALIFY`) and other
/// extensions (`DISTKEY`/`SORTKEY`/`DISTSTYLE`, `UNLOAD`/`COPY`, full SUPER DDL, window-frame
/// differences, …) remain deferred — each is a clean reject, not a silent over-accept. See
/// [`FeatureSet::REDSHIFT`](crate::ast::dialect::FeatureSet::REDSHIFT) module docs for the full
/// closed delta.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Redshift;

impl Dialect for Redshift {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::REDSHIFT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::test_support::assert_full_grammar;
    use crate::dialect::{Ansi, Postgres};
    use crate::parse_with;

    #[test]
    fn redshift_parses_the_full_m1_select_grammar() {
        // The representative query uses only constructs Redshift shares with ANSI (its lexis and
        // grammar are ANSI verbatim bar the identity-only lowercase fold), so it drives the full
        // shared grammar identically — the structural proof.
        assert_full_grammar(Redshift);
    }

    #[test]
    fn redshift_rejects_the_deferred_postgres_heritage_forms() {
        // The conservative-off decisions this preset records, pinned as tested boundaries: the
        // PostgreSQL-heritage forms Redshift genuinely accepts are deferred (our flag docs
        // attribute them to PostgreSQL, not Redshift, and no oracle can measure over-inclusion),
        // so each rejects under Redshift today exactly as under ANSI. The contrast with Postgres —
        // the fork parent, which *does* accept ILIKE / SIMILAR TO / DISTINCT ON — proves these are
        // real deferred Redshift features, not imaginary syntax, so a future edit that turns one
        // on will flip this test and demand its evidence.
        for sql in [
            "SELECT * FROM t WHERE name ILIKE 'a%'",
            "SELECT * FROM t WHERE name SIMILAR TO 'a%'",
            "SELECT DISTINCT ON (a) a, b FROM t",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Redshift)).is_err(),
                "Redshift defers the PG-heritage form {sql:?} (conservative reject)",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI also rejects {sql:?} (the base Redshift derives from)",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_ok(),
                "PostgreSQL — the fork parent — accepts {sql:?}, proving it is a real deferral",
            );
        }

        // `QUALIFY` is deferred too, but neither the ANSI base nor Postgres accepts it here (it is
        // a DuckDB/Snowflake surface), so only the Redshift reject is pinned.
        let qualify = "SELECT * FROM t QUALIFY row_number() OVER (ORDER BY a) = 1";
        assert!(
            parse_with(qualify, crate::ParseConfig::new(Redshift)).is_err(),
            "Redshift defers QUALIFY (conservative reject)",
        );
    }

    #[test]
    fn redshift_reads_double_quotes_as_identifiers_like_ansi() {
        // Redshift keeps the standard `"…"`-is-an-identifier lexis (unlike Hive/BigQuery, which
        // flip `"…"` to a string). The one delta over ANSI — the lowercase identifier fold — is
        // identity-only and never changes acceptance, so `SELECT "Col" FROM t` parses under both
        // Redshift and its ANSI base. This pins that the preset added no lexical divergence.
        let quoted = "SELECT \"Col\" FROM t";
        assert!(
            parse_with(quoted, crate::ParseConfig::new(Redshift)).is_ok(),
            "Redshift reads `\"Col\"` as a quoted identifier (standard lexis)",
        );
        assert!(
            parse_with(quoted, crate::ParseConfig::new(Ansi)).is_ok(),
            "ANSI reads `\"Col\"` as a quoted identifier too — Redshift's lexis is ANSI verbatim",
        );
    }

    #[test]
    fn redshift_features_are_the_redshift_preset() {
        assert_eq!(Redshift.features(), &FeatureSet::REDSHIFT);
    }
}
