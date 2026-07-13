// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The ClickHouse dialect.
//!
//! The whole module is gated by the `clickhouse` cargo feature (one `#[cfg]` on its
//! `mod` declaration), so the struct, the `Dialect` impl, and the ClickHouse test
//! cluster compile only when the feature is on — no per-item gates.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;

/// The ClickHouse dialect ([`FeatureSet::CLICKHOUSE`]).
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(ClickHouse))`.
/// ClickHouse is exposed as a deliberately conservative ANSI-derived preset (no
/// ClickHouse oracle exists to fit a wider surface): it adds the three ClickHouse query
/// tails — `LIMIT n [OFFSET m] BY …`, `SETTINGS name = value, …`, and `FORMAT <name>` —
/// and the six ClickHouse type constructors — `Nullable(T)`, `LowCardinality(T)`,
/// `FixedString(N)`, `DateTime64(P[, 'tz'])`, `Nested(name Type, …)`, and the
/// `Int8`…`Int256`/`UInt*` fixed-bit-width integer names — plus backtick-or-double-quote
/// identifier quoting and case-sensitive identifiers. The remaining ClickHouse surface
/// (`Array`/`Map`/`Tuple` types, `ARRAY JOIN`, `WITH FILL`, the `LIMIT n, m BY`
/// comma-offset form, …) is owned by follow-up grammar tickets and not yet accepted here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ClickHouse;

impl Dialect for ClickHouse {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::CLICKHOUSE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{DataType, WrappedTypeKind};
    use crate::dialect::test_support::{assert_full_grammar, first_column_type};
    use crate::dialect::{Ansi, DuckDb, MySql, Postgres, Sqlite};
    use crate::parse_with;
    use crate::parser::Dialect;

    #[test]
    fn clickhouse_parses_the_full_m1_select_grammar() {
        // The representative query uses only constructs ClickHouse shares with ANSI, so
        // it drives the full shared grammar identically — the structural proof.
        assert_full_grammar(ClickHouse);
    }

    /// The five oracle-compared shipped presets, none of which must accept the
    /// ClickHouse-only surface (they are pinned; the boundary tests assert the reject
    /// against each so a future preset edit cannot silently move one).
    fn rejects_under_every_oracle_preset(sql: &str) {
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI must reject the ClickHouse-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL must reject the ClickHouse-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL must reject the ClickHouse-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite must reject the ClickHouse-only form {sql:?}",
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB must reject the ClickHouse-only form {sql:?}",
        );
    }

    #[test]
    fn clickhouse_parses_the_query_tails_that_oracle_presets_reject() {
        // The three ClickHouse SELECT tails: each parses under ClickHouse and is rejected
        // by every oracle-compared preset (the tail keyword is contextual/unreserved
        // there, so it dangles as trailing input and the statement rejects).
        for sql in [
            "SELECT a FROM t LIMIT 5 BY a",             // LIMIT BY
            "SELECT a FROM t LIMIT 5 OFFSET 2 BY a",    // LIMIT … OFFSET … BY
            "SELECT a FROM t SETTINGS max_threads = 8", // SETTINGS tail
            "SELECT a FROM t FORMAT JSON",              // FORMAT tail
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(ClickHouse)).is_ok(),
                "ClickHouse parses {sql:?}"
            );
            rejects_under_every_oracle_preset(sql);
        }
    }

    #[test]
    fn clickhouse_type_constructors_with_type_bodies_reject_under_oracle_presets() {
        // The ClickHouse type constructors whose body is itself a type or column list, not
        // a generic numeric modifier: an oracle preset reads the head as a user-defined
        // type name and then rejects the body (e.g. `Nullable ( INT )` — `INT` is not a
        // u32 modifier — and `Nested ( a INT, … )` — column defs are not modifiers). Each
        // parses under ClickHouse and rejects under every oracle-compared preset.
        for sql in [
            "CREATE TABLE t (c Nullable(INT))",
            "CREATE TABLE t (c LowCardinality(FixedString(16)))",
            "CREATE TABLE t (c Nested(a INT, b INT))",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(ClickHouse)).is_ok(),
                "ClickHouse parses {sql:?}"
            );
            rejects_under_every_oracle_preset(sql);
        }
    }

    #[test]
    fn clickhouse_type_constructors_with_scalar_bodies_route_to_clickhouse_nodes() {
        // The remaining ClickHouse type constructors have a scalar/absent body that an
        // oracle preset happily reads as a *generic* parameterized user-defined type
        // (`FixedString(16)`/`DateTime64(3)` are a name plus a numeric modifier, `Int256`
        // a bare name), so the boundary here is AST *shape*, not accept/reject: under
        // ClickHouse each routes to its dedicated node, while ANSI produces the generic
        // `UserDefined` type. Proving the shape split is what shows the preset gate fires.
        assert!(matches!(
            first_column_type(ClickHouse, "CREATE TABLE t (c FixedString(16))"),
            DataType::FixedString { .. }
        ));
        assert!(matches!(
            first_column_type(ClickHouse, "CREATE TABLE t (c DateTime64(3))"),
            DataType::DateTime64 { .. }
        ));
        assert!(matches!(
            first_column_type(ClickHouse, "CREATE TABLE t (c Int256)"),
            DataType::FixedWidthInt { signed: true, .. }
        ));
        // The same three read as a generic user-defined type under the ANSI baseline —
        // the off-gate shape the ClickHouse preset diverges from.
        for sql in [
            "CREATE TABLE t (c FixedString(16))",
            "CREATE TABLE t (c DateTime64(3))",
            "CREATE TABLE t (c Int256)",
        ] {
            assert!(
                matches!(first_column_type(Ansi, sql), DataType::UserDefined { .. }),
                "ANSI reads {sql:?} as a generic user-defined type",
            );
        }
    }

    #[test]
    fn clickhouse_wrapper_type_kinds_are_the_clickhouse_combinators() {
        // The two wrapper combinators land on the shared `Wrapped` node with the correct
        // `WrappedTypeKind` axis — the AST proof that `Nullable`/`LowCardinality` route to
        // their combinators (not a user-defined name) under the preset.
        assert!(matches!(
            first_column_type(ClickHouse, "CREATE TABLE t (c Nullable(INT))"),
            DataType::Wrapped {
                kind: WrappedTypeKind::Nullable,
                ..
            }
        ));
        assert!(matches!(
            first_column_type(ClickHouse, "CREATE TABLE t (c LowCardinality(INT))"),
            DataType::Wrapped {
                kind: WrappedTypeKind::LowCardinality,
                ..
            }
        ));
    }

    #[test]
    fn clickhouse_quotes_identifiers_with_backticks_and_double_quotes() {
        // The dual identifier-quote fact: both spellings parse under ClickHouse; ANSI (a
        // `"`-only quoter) rejects the backtick form. `"a"` reads as an identifier, never
        // a string (`double_quoted_strings` stays off).
        assert!(
            parse_with(
                r#"SELECT "a", `b` FROM t"#,
                crate::ParseConfig::new(ClickHouse)
            )
            .is_ok(),
            "ClickHouse accepts both `\"a\"` and `` `b` `` identifiers",
        );
        assert!(
            parse_with("SELECT `b` FROM t", crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects the backtick identifier ClickHouse accepts",
        );
    }

    #[test]
    fn clickhouse_features_are_the_clickhouse_preset() {
        assert_eq!(ClickHouse.features(), &FeatureSet::CLICKHOUSE);
    }
}
