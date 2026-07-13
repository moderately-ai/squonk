// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The MSSQL / T-SQL dialect.
//!
//! The whole module is gated by the `mssql` cargo feature (one `#[cfg]` on its `mod`
//! declaration), so the struct, the `Dialect` impl, and the MSSQL test cluster compile only
//! when the feature is on — no per-item gates.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;

/// The MSSQL / T-SQL dialect ([`FeatureSet::MSSQL`]).
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(Mssql))`. MSSQL is
/// exposed as a deliberately conservative ANSI-derived preset (no MSSQL oracle exists to fit a
/// wider surface): it adds the `CROSS APPLY` / `OUTER APPLY` lateral-correlated join operators,
/// bracket-quoted identifiers (`[name]`), `@name` parameters / local variables, `N'…'`
/// national-character string constants, and `$1234.56` money literals. Identifiers are also
/// quoted with the standard `"…"` (T-SQL's default `QUOTED_IDENTIFIER ON`) and folded
/// case-insensitively for identity. The remaining T-SQL surface (`SELECT … INTO <table>`,
/// `TOP (n)`, table hints `WITH (NOLOCK)`, `GO` batch separators, `#temp` tables, the `OUTPUT`
/// clause, the T-SQL `MERGE` variants, …) is owned by follow-up grammar tickets and not yet
/// accepted here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mssql;

impl Dialect for Mssql {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::MSSQL
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
    fn mssql_parses_the_full_m1_select_grammar() {
        // The representative query uses only constructs MSSQL shares with ANSI, so it drives
        // the full shared grammar identically — the structural proof.
        assert_full_grammar(Mssql);
    }

    /// The five oracle-compared shipped presets, none of which must accept the given
    /// MSSQL-only surface (they are pinned; the boundary tests assert the reject against each
    /// so a future preset edit cannot silently move one).
    fn rejects_under_every_oracle_preset(sql: &str) {
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI must reject the MSSQL-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL must reject the MSSQL-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL must reject the MSSQL-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite must reject the MSSQL-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB must reject the MSSQL-only form {sql:?}",
        );
    }

    #[test]
    fn mssql_parses_cross_and_outer_apply_that_every_oracle_preset_rejects() {
        // The capstone this preset makes real: `CROSS APPLY` / `OUTER APPLY`. `apply_join` is
        // off in every oracle-compared preset, so each parses under MSSQL and rejects under
        // all five.
        for sql in [
            "SELECT * FROM a CROSS APPLY (SELECT 1) AS t",
            "SELECT * FROM a OUTER APPLY b",
            "SELECT * FROM a CROSS APPLY (SELECT b.x FROM b WHERE b.id = a.id) AS c",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Mssql)).is_ok(),
                "MSSQL parses {sql:?}"
            );
            rejects_under_every_oracle_preset(sql);
        }
    }

    #[test]
    fn mssql_parses_money_literals_that_every_oracle_preset_rejects() {
        // T-SQL `$1234.56` money literals: the `$` currency sigil prefixes a decimal.
        // `money_literals` is off in every oracle-compared preset, and the decimal form (a `.`
        // in the mantissa) is a clean reject even in the two presets that read a bare `$100` as
        // a `$`-sigil parameter (PostgreSQL `$100`, DuckDB) — so each parses under MSSQL and
        // rejects under all five.
        for sql in ["SELECT $1234.56", "SELECT $.5", "SELECT a + $9.99 FROM t"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Mssql)).is_ok(),
                "MSSQL parses {sql:?}"
            );
            rejects_under_every_oracle_preset(sql);
        }
    }

    #[test]
    fn mssql_quotes_identifiers_with_brackets_and_double_quotes() {
        // T-SQL's signature `[name]` bracket identifier. Among the oracle presets only SQLite
        // shares the bracket quote (it too models T-SQL compatibility), so the reject holds for
        // the other four; the shared half is asserted explicitly.
        let bracket = "SELECT [a b] FROM t";
        assert!(
            parse_with(bracket, crate::ParseConfig::new(Mssql)).is_ok(),
            "MSSQL reads `[a b]`"
        );
        assert!(
            parse_with(bracket, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI has no bracket identifier quote",
        );
        assert!(
            parse_with(bracket, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL has no bracket identifier quote",
        );
        assert!(
            parse_with(bracket, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL quotes with backticks, not brackets",
        );
        assert!(
            parse_with(bracket, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB has no bracket identifier quote",
        );
        // The shared half: SQLite accepts bracket identifiers too (its own T-SQL-compat quote).
        assert!(
            parse_with(bracket, crate::ParseConfig::new(Sqlite)).is_ok(),
            "SQLite shares the `[name]` bracket quote",
        );
        // `"…"` stays an identifier quote (not a string), so `double_quoted_strings` is off and
        // T-SQL's default `QUOTED_IDENTIFIER ON` behaviour holds.
        assert!(
            parse_with("SELECT \"col\" FROM t", crate::ParseConfig::new(Mssql)).is_ok(),
            "MSSQL reads a double-quoted identifier",
        );
    }

    #[test]
    fn mssql_parses_at_name_parameters() {
        // T-SQL `@name` parameters / local variables. The `@name` reading as a *parameter* is
        // MSSQL's; the `@` sigil is not a clean reject everywhere — PostgreSQL and DuckDB read a
        // prefix `@` operator (their `custom_operators` surface), and MySQL/SQLite read a
        // user-variable / `@name` parameter — so the honest reject boundary is the one preset
        // that claims `@` for nothing: ANSI.
        for sql in ["SELECT * FROM t WHERE a = @p", "SELECT @p"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Mssql)).is_ok(),
                "MSSQL parses {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI has no `@` sigil, so it rejects {sql:?}",
            );
            // DuckDB/PostgreSQL parse-accept `@p`, but as a prefix `@` operator, not a
            // parameter — the same reading engine-verified on DuckDB 1.5.4 (`SELECT @ 1`).
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB reads {sql:?} as a prefix `@` operator",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_ok(),
                "PostgreSQL reads {sql:?} as a prefix `@` operator",
            );
        }
    }

    #[test]
    fn mssql_parses_national_string_literals() {
        // T-SQL `N'…'` national-character strings lex as a single string token under MSSQL.
        // No oracle preset parse-*rejects* the surface: ANSI/PostgreSQL/MySQL/DuckDB read it as a
        // *typed string literal* (type `N`, string `'…'`), and SQLite — which has neither national
        // strings nor typed string literals but does accept a bare string alias — reads it as the
        // column `N` aliased by the string `'hi'` (engine-measured on rusqlite: `SELECT N'hi'`
        // prepares, failing only to resolve the column `N`). Each is a divergence in meaning, not
        // a parse reject, so this pins MSSQL's national-string reading against those alternatives.
        let national = "SELECT N'hi'";
        assert!(
            parse_with(national, crate::ParseConfig::new(Mssql)).is_ok(),
            "MSSQL reads `N'hi'` as a national-character string",
        );
        assert!(
            parse_with(national, crate::ParseConfig::new(Sqlite)).is_ok(),
            "SQLite reads `N'hi'` as the column `N` aliased by the bare string `'hi'`",
        );
    }

    #[test]
    fn mssql_features_are_the_mssql_preset() {
        assert_eq!(Mssql.features(), &FeatureSet::MSSQL);
    }
}
