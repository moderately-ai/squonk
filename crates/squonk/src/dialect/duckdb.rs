// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The DuckDB dialect.
//!
//! The whole module is gated by the `duckdb` cargo feature (one `#[cfg]` on its `mod`
//! declaration), so the struct, the `Dialect` impl, and the DuckDB test cluster are
//! compiled only when the feature is on — no per-item gates.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;

/// The DuckDB dialect ([`FeatureSet::DUCKDB`]).
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(DuckDb))`. DuckDB
/// is PostgreSQL-dialect-compatible, so this preset is [`Postgres`](super::Postgres)'s
/// with the measured deltas: the `0x`/`0o`/`0b` radix integer forms and `_` digit
/// separators, the `QUALIFY` clause (and its keyword reservation), the collection
/// literals (`[1, 2]`, `{'a': 1}`, `MAP {k: v}`), the single-arrow lambdas
/// (`x -> x + 1`, `(x, y) -> x + y`), and the star-expression family — the `*`/`t.*`
/// wildcard modifiers `EXCLUDE`/`REPLACE`/`RENAME`, the `COLUMNS(…)` column-set selector,
/// and the `PIVOT`/`UNPIVOT` operators (both the
/// leading-keyword statement and the `FROM t PIVOT (…)` table factor, with their
/// keyword reservations) are accepted, while the empty (bare `SELECT`) target list is
/// rejected where PostgreSQL accepts it. `COLUMNS(…)` and `*COLUMNS(…)` unpack forms are
/// part of the shipped surface (see dialect tests). Other DuckDB-specific grammar still
/// deferred is listed in [`FeatureSet::DUCKDB`] module docs — not claimed “unsupported”
/// when a gate is already on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct DuckDb;

impl Dialect for DuckDb {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::DUCKDB
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::test_support::{assert_full_grammar, first_column_type};
    use crate::dialect::{Ansi, Postgres};
    use crate::parse_with;

    #[test]
    fn duckdb_parses_the_full_m1_select_grammar() {
        // The representative query uses only constructs DuckDB shares with ANSI/Postgres,
        // so it drives the full shared grammar identically — the structural proof.
        assert_full_grammar(DuckDb);
    }

    #[test]
    fn duckdb_accepts_the_radix_integer_and_digit_separator_forms() {
        // The additive numeric delta over PostgreSQL, each accepted by DuckDB's engine
        // (verified against DuckDB 1.5.4). The trailing `+ 1` forces the radix reading:
        // under ANSI (radix off) the leading digit + rest tokenizes as `0 AS xFF`, so the
        // `+ 1` is trailing garbage and the statement rejects — the discriminator the
        // SQLite preset uses for the same `hex_integers` knob.
        for sql in [
            "SELECT 0xFF + 1",        // hex integer
            "SELECT 0o17 + 1",        // octal integer
            "SELECT 0b101 + 1",       // binary integer
            "SELECT 1_000_000 + 1",   // underscore digit separators
            "SELECT 0xDEAD_BEEF + 1", // radix + separators together
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects the DuckDB-only radix form {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_shares_the_postgres_lexical_surface() {
        // The inherited PostgreSQL forms: positional `$1`, dollar-quoted strings, and the
        // escape-string spelling all parse under DuckDB because it derives from Postgres.
        for sql in [
            "SELECT $1",
            "SELECT $tag$dollar quoted$tag$",
            "SELECT E'\\n'",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }
    }

    #[test]
    fn duckdb_parses_optional_transaction_block_word_after_start() {
        for (sql, rendered) in [
            ("START", "START"),
            ("START WORK", "START WORK"),
            ("START TRANSACTION", "START TRANSACTION"),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|err| panic!("DuckDB should parse {sql:?}: {err:?}"));
            assert_eq!(parsed.to_sql(), rendered);
        }

        for sql in ["ABORT", "ABORT WORK", "END", "END TRANSACTION"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|err| panic!("DuckDB should parse {sql:?}: {err:?}"));
            assert_eq!(parsed.to_sql(), sql);
        }

        for sql in [
            "SAVEPOINT s",
            "RELEASE SAVEPOINT s",
            "ROLLBACK TO SAVEPOINT s",
            "SET TRANSACTION READ ONLY",
            "START TRANSACTION ISOLATION LEVEL SERIALIZABLE",
            "START TRANSACTION DEFERRABLE",
            "START TRANSACTION READ ONLY, READ WRITE",
            "COMMIT AND CHAIN",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB rejects {sql:?}",
            );
        }

        for sql in ["START", "START WORK"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL requires START TRANSACTION for {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_rejects_the_empty_target_list_where_postgres_accepts_it() {
        // The one subtractive delta: DuckDB's parser rejects a bare `SELECT` (`SELECT
        // clause without selection list`) that PostgreSQL's raw grammar accepts. This is
        // the divergence the fitted preset closes — proven by the direct Postgres/DuckDb
        // contrast so the tightening cannot silently regress.
        assert!(
            parse_with("SELECT", crate::ParseConfig::new(DuckDb)).is_err(),
            "DuckDB rejects the empty target list",
        );
        assert!(
            parse_with("SELECT", crate::ParseConfig::new(Postgres)).is_ok(),
            "PostgreSQL accepts the empty target list (the divergence DuckDB tightens)",
        );
    }

    /// The `Select` body of a single-query statement.
    fn select_of(parsed: &crate::parser::Parsed) -> &crate::ast::Select<NoExt> {
        use crate::ast::{SetExpr, Statement};
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a plain SELECT body");
        };
        select
    }

    #[test]
    fn duckdb_parses_the_qualify_clause() {
        // The flagship post-window filter: the predicate lands in its own `qualify`
        // slot (never `having`), directly after a FROM relation — which relies on the
        // preset reserving `QUALIFY` so the relation cannot absorb it as an alias.
        let parsed = parse_with(
            "SELECT a FROM t QUALIFY row_number() OVER (PARTITION BY id ORDER BY ts DESC) = 1",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDB parses QUALIFY");
        let select = select_of(&parsed);
        assert!(select.qualify.is_some(), "the predicate is captured");
        assert!(select.having.is_none(), "QUALIFY is not folded into HAVING");
    }

    #[test]
    fn duckdb_qualify_clause_order_matches_the_engine() {
        // Verified against DuckDB 1.5.4: QUALIFY comes after GROUP BY/HAVING and the
        // WINDOW clause, and before ORDER BY/LIMIT; `QUALIFY … WINDOW …` is a syntax
        // error there and here.
        for sql in [
            "SELECT a, count(*) FROM t GROUP BY a HAVING count(*) > 1 \
             QUALIFY row_number() OVER () = 1 ORDER BY a LIMIT 2",
            "SELECT a FROM t WINDOW w AS (PARTITION BY id) QUALIFY row_number() OVER w = 1",
            // A predicate with no window function parses; DuckDB rejects it at BIND
            // time ("at least one window function must appear…"), past the
            // parse-level contract.
            "SELECT a FROM t QUALIFY a > 1",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }
        assert!(
            parse_with(
                "SELECT a FROM t QUALIFY row_number() OVER w = 1 WINDOW w AS (PARTITION BY id)",
                crate::ParseConfig::new(DuckDb),
            )
            .is_err(),
            "QUALIFY cannot precede the WINDOW clause (engine-verified order)",
        );
    }

    #[test]
    fn duckdb_qualify_round_trips_byte_identically() {
        use crate::dialect::Lenient;
        use crate::render::Renderer;

        // DuckDb is not itself a Tier-2 render target yet (the preset defers
        // `TargetSpelling::DuckDb` to a later ticket), so the round-trip renders under
        // Lenient — the permissive superset — proving the `qualify` slot, not the
        // target dialect, drives the emitted clause (the `WITH ROLLUP` precedent).
        for sql in [
            "SELECT a FROM t QUALIFY row_number() OVER (PARTITION BY id ORDER BY ts DESC) = 1",
            "SELECT a FROM t GROUP BY a QUALIFY row_number() OVER () = 1 ORDER BY a LIMIT 2",
            "SELECT a FROM t WINDOW w AS (PARTITION BY id) QUALIFY row_number() OVER w = 1",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb)).expect("QUALIFY parses");
            assert_eq!(
                Renderer::new(Lenient)
                    .render_parsed(&parsed)
                    .expect("QUALIFY renders"),
                sql,
            );
        }
    }

    #[test]
    fn qualify_clause_is_rejected_where_the_gate_is_off() {
        // ANSI and PostgreSQL have no QUALIFY clause: the word is read as an ordinary
        // identifier (a table alias here), leaving the predicate as trailing input —
        // a clean parse error, not an over-acceptance.
        for sql in [
            "SELECT a FROM t QUALIFY x = 1",
            "SELECT a FROM t QUALIFY row_number() OVER (PARTITION BY id ORDER BY ts DESC) = 1",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects the QUALIFY clause: {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects the QUALIFY clause: {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_reserves_qualify_like_the_engine() {
        // `duckdb_keywords()` classes QUALIFY `reserved` (like HAVING): never a plain
        // identifier — each probe below syntax-errors on DuckDB 1.5.4 — while the
        // `AS`-label and quoted spellings stay legal.
        for sql in [
            "SELECT qualify FROM t",     // column name
            "SELECT 1 qualify",          // bare projection alias
            "SELECT * FROM qualify",     // table name
            "SELECT * FROM t qualify",   // bare table alias
            "SELECT qualify(1)",         // function name
            "SELECT CAST(1 AS qualify)", // type name
            "CREATE TABLE qualify (i INT)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB reserves QUALIFY: {sql:?}",
            );
        }
        assert!(
            parse_with("SELECT 1 AS qualify", crate::ParseConfig::new(DuckDb)).is_ok(),
            "a reserved keyword is still a valid AS label",
        );
        assert!(
            parse_with("SELECT \"qualify\" FROM t", crate::ParseConfig::new(DuckDb)).is_ok(),
            "the quoted spelling is an ordinary identifier",
        );
    }

    #[test]
    fn duckdb_unreserved_words_follow_engine_positions() {
        for word in ["grant", "user"] {
            for template in [
                "SELECT {} FROM t",
                "SELECT * FROM {}",
                "SELECT {}(1)",
                "SELECT CAST(1 AS {})",
                "SET x = {}",
                "SELECT 1 AS {}",
            ] {
                let sql = template.replace("{}", word);
                assert!(
                    parse_with(&sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                    "DuckDB admits its unreserved word in {sql:?}",
                );
            }
            let sql = format!("SELECT 1 {word}");
            assert!(
                parse_with(&sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB rejects its unreserved word as a bare alias in {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_special_value_names_are_callable_and_unreserved() {
        for word in [
            "current_catalog",
            "current_date",
            "current_role",
            "current_schema",
            "current_time",
            "current_timestamp",
            "current_user",
            "localtime",
            "localtimestamp",
            "session_user",
            "system_user",
        ] {
            for template in [
                "SELECT {} FROM t",
                "SELECT * FROM {}",
                "SELECT {}(1)",
                "SELECT CAST(1 AS {})",
                "SELECT 1 {}",
                "SET x = {}",
            ] {
                let sql = template.replace("{}", word);
                assert!(
                    parse_with(&sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                    "DuckDB admits its ordinary identifier in {sql:?}",
                );
            }
        }
    }

    #[test]
    fn qualify_stays_an_ordinary_identifier_elsewhere() {
        // The keyword-inventory addition must not reserve the word anywhere else:
        // ANSI and PostgreSQL keep accepting `qualify` in every identifier position
        // it was legal in before (the reserved-word regression guard).
        for sql in [
            "SELECT qualify FROM t",
            "SELECT 1 qualify",
            "SELECT 1 AS qualify",
            "SELECT * FROM qualify",
            "SELECT * FROM t qualify",
            "CREATE TABLE qualify (i INT)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_ok(),
                "ANSI leaves qualify a free identifier: {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_ok(),
                "PostgreSQL leaves qualify a free identifier: {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_recognizes_a_declared_column_type() {
        // DuckDB shares PostgreSQL's type vocabulary; the column grammar is shared with
        // `CAST`, so this is the lens onto type recognition.
        use crate::ast::DataType;
        assert!(matches!(
            first_column_type(DuckDb, "CREATE TABLE t (c INTEGER)"),
            DataType::Integer { .. }
        ));
    }

    #[test]
    fn duckdb_folds_empty_decimal_parens_onto_the_bare_shape() {
        // `duckdb-empty-type-parens`: the empty `DECIMAL()`/`DEC()`/`NUMERIC()` parens mean
        // the default precision/scale, which DuckDB normalizes to the same `DECIMAL(18,3)`
        // as a bare `DECIMAL` (probed on 1.5.4). The empty form parses onto the bare
        // `precision: None, scale: None` shape (per spelling), so the render drops the parens
        // — an ADR-0011 spelling trade matching the engine's own normalization.
        use crate::ast::{DataType, DecimalTypeName};
        use crate::dialect::Lenient;
        use crate::render::Renderer;

        assert!(matches!(
            first_column_type(DuckDb, "CREATE TABLE t (c DECIMAL())"),
            DataType::Decimal {
                precision: None,
                scale: None,
                spelling: DecimalTypeName::Decimal,
                ..
            }
        ));
        assert!(matches!(
            first_column_type(DuckDb, "CREATE TABLE t (c DEC())"),
            DataType::Decimal {
                precision: None,
                scale: None,
                spelling: DecimalTypeName::Dec,
                ..
            }
        ));
        assert!(matches!(
            first_column_type(DuckDb, "CREATE TABLE t (c NUMERIC())"),
            DataType::Decimal {
                precision: None,
                scale: None,
                spelling: DecimalTypeName::Numeric,
                ..
            }
        ));

        // The render fold: `DECIMAL()` renders as bare `DECIMAL`, while a genuine
        // `DECIMAL(18, 3)` still round-trips its modifier.
        let empty = parse_with(
            "SELECT CAST(x AS DECIMAL())",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DECIMAL() parses");
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&empty)
                .expect("renders"),
            "SELECT CAST(x AS DECIMAL)",
        );
        let sized = parse_with(
            "SELECT CAST(x AS DECIMAL(18, 3))",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DECIMAL(18,3) parses");
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&sized)
                .expect("renders"),
            "SELECT CAST(x AS DECIMAL(18, 3))",
        );
    }

    #[test]
    fn empty_decimal_parens_are_rejected_where_the_gate_is_off() {
        // ANSI/PostgreSQL require a precision inside `DECIMAL(...)`; the empty `()` is a clean
        // parse error (verified against pg_query), never an over-acceptance.
        for sql in [
            "CREATE TABLE t (c DECIMAL())",
            "SELECT CAST(x AS DEC())",
            "SELECT CAST(x AS NUMERIC())",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_parses_the_sweep_lambdas_as_lambda_nodes() {
        // The corpus-sweep list-function shapes (each executes on DuckDB 1.5.4): the
        // lambda argument is the dedicated node, not a `JsonGet` fold. The node split
        // rides the whole fitted preset here — collection literals for the `[…]`
        // argument plus the lambda gate — the composition the corpus actually uses.
        use crate::ast::{Expr, FunctionArg};
        for (sql, params) in [
            ("SELECT list_transform([1, 2, 3], x -> x + 1)", 1),
            ("SELECT list_filter([1, 2, 3, 4], x -> x % 2 = 0)", 1),
            ("SELECT list_reduce([1, 2, 3], (x, y) -> x + y)", 2),
        ] {
            let parsed =
                parse_with(sql, crate::ParseConfig::new(DuckDb)).expect("the sweep lambda parses");
            let select = select_of(&parsed);
            let crate::ast::SelectItem::Expr { expr, .. } = &select.projection[0] else {
                panic!("expected an expression item for {sql:?}");
            };
            let Expr::Function { call, .. } = expr else {
                panic!("expected the list-function call for {sql:?}");
            };
            let FunctionArg { value, .. } = &call.args[1];
            let Expr::Lambda { lambda, .. } = value else {
                panic!("expected the lambda argument for {sql:?}, got {value:?}");
            };
            assert_eq!(lambda.params.len(), params, "params for {sql:?}");
        }
    }

    #[test]
    fn duckdb_lambda_parses_position_independently_like_the_engine() {
        // DuckDB parses a lambda anywhere an expression sits — `SELECT x -> x + 1`
        // serializes as a LAMBDA node (probed via json_serialize_sql; only the BINDER
        // later rejects an unconsumed lambda) — so the parse-level gate is
        // position-independent too, never restricted to function arguments.
        use crate::ast::Expr;
        let parsed = parse_with("SELECT x -> x + 1", crate::ParseConfig::new(DuckDb))
            .expect("a bare lambda parses");
        let select = select_of(&parsed);
        let crate::ast::SelectItem::Expr { expr, .. } = &select.projection[0] else {
            panic!("expected an expression item");
        };
        assert!(matches!(expr, Expr::Lambda { .. }));
    }

    #[test]
    fn duckdb_accepts_the_probed_pivot_surface() {
        // The engine-probed acceptance matrix (each verified against DuckDB 1.5.4):
        // the statement forms with every optional-clause combination, the table-factor
        // forms with aliases and NULLS markers, and the parenthesized statement in
        // FROM. Shape assertions live in `parser::pivot`; this is the dialect gate.
        for sql in [
            // statement forms
            "PIVOT Cities ON Year USING sum(Population)",
            "PIVOT Cities ON Year USING sum(Population) GROUP BY Country",
            "PIVOT Cities ON Year",
            "PIVOT Cities USING sum(Population)",
            "PIVOT Cities GROUP BY Country",
            "PIVOT Cities ON Year IN (2000, 2010) USING sum(Population)",
            "PIVOT Cities ON Year USING sum(Population) AS total",
            // the entry production admits operator/call/grouped forms (probed)…
            "PIVOT Cities ON Year || Country USING sum(Population)",
            "PIVOT Cities ON 1 + Year USING sum(Population)",
            "PIVOT Cities ON lower(Country) USING sum(Population)",
            // …a subquery IN source, and the ALL order mode (both probed)
            "PIVOT Cities ON Year IN (SELECT Year FROM Cities) USING sum(Population)",
            "PIVOT Cities ON Year USING sum(Population) GROUP BY Country ORDER BY ALL",
            // …and UNPIVOT entries are parse-unrestricted (bind-time rejects only),
            // including cast/expression entries and the VALUES spelling
            "UNPIVOT t ON 'jan', 42 INTO NAME n VALUE v",
            "UNPIVOT t ON (a + 100)::VARCHAR, b INTO NAME n VALUE v",
            "UNPIVOT t ON (a, jan), (b, feb) INTO NAME n VALUES v1, v2",
            // the ENUM-typed and multi-head IN sources (both probed)
            "SELECT * FROM s PIVOT (sum(amount) FOR m IN month_enum) AS p",
            "SELECT * FROM s PIVOT (sum(amount) FOR y IN (2020, 2021) m IN ('JAN')) AS p",
            "PIVOT (SELECT Country, Year FROM Cities) ON Year",
            "PIVOT Cities ON Year USING sum(Population) GROUP BY Country ORDER BY Country LIMIT 5",
            "UNPIVOT monthly_sales ON jan, feb, mar INTO NAME month VALUE sales",
            "UNPIVOT monthly_sales ON jan, feb, mar",
            "WITH c AS (SELECT 1) PIVOT c ON x USING sum(y)",
            // table-factor forms
            "SELECT * FROM Cities PIVOT (sum(Population) FOR Year IN (2000, 2010))",
            "SELECT * FROM Cities PIVOT (sum(Population) FOR Year IN (2000 AS y2000, 2010 AS y2010))",
            "SELECT * FROM Cities PIVOT (sum(Population) FOR Year IN (2000) GROUP BY Country)",
            "SELECT * FROM Cities PIVOT (sum(Population) AS total, count(*) AS n FOR Year IN (2000)) AS p",
            "SELECT * FROM monthly_sales UNPIVOT (sales FOR month IN (jan, feb, mar))",
            "SELECT * FROM t UNPIVOT INCLUDE NULLS (v FOR n IN (a, b))",
            "SELECT * FROM t UNPIVOT EXCLUDE NULLS (v FOR n IN (a, b))",
            "SELECT * FROM t UNPIVOT ((v1, v2) FOR n IN ((a, b), (c, d)))",
            "SELECT * FROM Cities c PIVOT (sum(Population) FOR Year IN (2000))",
            "SELECT * FROM (SELECT * FROM Cities) PIVOT (sum(Population) FOR Year IN (2000))",
            "SELECT * FROM t PIVOT (sum(x) FOR y IN (1)) JOIN u ON true",
            // the statement form parenthesized into FROM
            "SELECT * FROM (PIVOT Cities ON Year USING sum(Population))",
            "SELECT * FROM (UNPIVOT monthly_sales ON jan, feb)",
            "WITH x AS (SELECT 1) SELECT * FROM (WITH c AS (SELECT 2) PIVOT c ON a USING sum(b))",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }
    }

    #[test]
    fn duckdb_rejects_the_engine_rejected_pivot_forms() {
        // Probed rejections (DuckDB 1.5.4): the table factor requires both the
        // aggregate list and the single `FOR … IN (…)` head, and the statement form
        // admits no `INCLUDE NULLS` marker.
        for sql in [
            // no FOR clause in the parenthesized form
            "SELECT * FROM Cities PIVOT (sum(Population))",
            // empty aggregate list
            "SELECT * FROM t PIVOT (FOR y IN (1, 2))",
            // a second FOR clause
            "SELECT * FROM t PIVOT (sum(x) FOR y IN (1) FOR z IN (2))",
            // the statement form rejects the NULLS marker
            "UNPIVOT INCLUDE NULLS t ON a, b",
            // the statement ON entry excludes bare constants, subqueries, and
            // boolean/predicate tops (the engine's b_expr-like production; probed —
            // these were the sweep's three over-acceptances plus siblings)
            "PIVOT Cities ON NULL USING sum(Population) GROUP BY Year",
            "PIVOT Cities ON 'hello world' USING sum(Population) GROUP BY Year",
            "PIVOT Cities ON (SELECT Country) USING sum(Population) GROUP BY Year",
            "PIVOT Cities ON NOT Country USING sum(Population)",
            "PIVOT Cities ON Year IS NULL USING sum(Population)",
            "PIVOT Cities ON Year NOT IN (2000) USING sum(Population)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB rejects {sql:?}"
            );
        }
    }

    #[test]
    fn pivot_forms_are_rejected_where_the_gate_is_off() {
        // ANSI and PostgreSQL have no PIVOT grammar: the leading keyword is an unknown
        // statement, and the factor suffix is unreachable (the word reads as an
        // ordinary alias whose `(…)` column list cannot hold `sum(x)`) — clean parse
        // errors, never an over-acceptance.
        for sql in [
            "PIVOT Cities ON Year USING sum(Population)",
            "UNPIVOT monthly_sales ON jan, feb, mar INTO NAME month VALUE sales",
            "SELECT * FROM Cities PIVOT (sum(Population) FOR Year IN (2000, 2010))",
            "SELECT * FROM monthly_sales UNPIVOT (sales FOR month IN (jan, feb))",
            "SELECT * FROM (PIVOT Cities ON Year USING sum(Population))",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_reserves_pivot_and_unpivot_like_the_engine() {
        // `duckdb_keywords()` classes PIVOT/UNPIVOT `reserved` (like QUALIFY): never a
        // plain identifier — each probe below syntax-errors on DuckDB 1.5.4 — while
        // the `AS`-label and quoted spellings stay legal. The bare-alias reservation
        // is the load-bearing one: it is what lets `FROM t PIVOT (…)` read the
        // operator instead of an alias.
        for word in ["pivot", "unpivot"] {
            for template in [
                "SELECT {} FROM t",     // column name
                "SELECT 1 {}",          // bare projection alias
                "SELECT * FROM {}",     // table name
                "SELECT {}(1)",         // function name
                "SELECT CAST(1 AS {})", // type name
                "CREATE TABLE {} (i INT)",
            ] {
                let sql = template.replace("{}", word);
                assert!(
                    parse_with(&sql, crate::ParseConfig::new(DuckDb)).is_err(),
                    "DuckDB reserves {word}: {sql:?}",
                );
            }
            let label = format!("SELECT 1 AS {word}");
            assert!(
                parse_with(&label, crate::ParseConfig::new(DuckDb)).is_ok(),
                "a reserved keyword is still a valid AS label",
            );
            let quoted = format!("SELECT \"{word}\" FROM t");
            assert!(
                parse_with(&quoted, crate::ParseConfig::new(DuckDb)).is_ok(),
                "the quoted spelling is an ordinary identifier",
            );
        }
    }

    #[test]
    fn pivot_stays_an_ordinary_identifier_elsewhere() {
        // The keyword-inventory addition must not reserve the words anywhere else:
        // ANSI and PostgreSQL keep accepting them in every identifier position (the
        // reserved-word regression guard, mirroring `qualify`'s).
        for word in ["pivot", "unpivot"] {
            for template in [
                "SELECT {} FROM t",
                "SELECT 1 {}",
                "SELECT 1 AS {}",
                "SELECT * FROM {}",
                "SELECT * FROM t {}",
                "CREATE TABLE {} (i INT)",
            ] {
                let sql = template.replace("{}", word);
                assert!(
                    parse_with(&sql, crate::ParseConfig::new(Ansi)).is_ok(),
                    "ANSI leaves {word} a free identifier: {sql:?}",
                );
                assert!(
                    parse_with(&sql, crate::ParseConfig::new(Postgres)).is_ok(),
                    "PostgreSQL leaves {word} a free identifier: {sql:?}",
                );
            }
        }
    }

    #[test]
    fn duckdb_pivot_round_trips_byte_identically() {
        use crate::dialect::Lenient;
        use crate::render::Renderer;

        // DuckDb is not itself a Tier-2 render target yet, so the round-trip renders
        // under Lenient — the QUALIFY precedent — proving the node and spelling tag,
        // not the target dialect, drive the emitted surface.
        for sql in [
            "PIVOT Cities ON Year USING sum(Population) GROUP BY Country",
            "PIVOT Cities ON Year IN (2000, 2010) USING sum(Population) AS total ORDER BY Country LIMIT 5",
            // A grouped comparison as the ON column: the unfolded IN keeps the
            // grouping parens the re-parse needs (the corpus regression case).
            "PIVOT Cities ON (Country = 'NL') IN (false, true) USING avg(Population) GROUP BY Name",
            "PIVOT (SELECT Country, Year FROM Cities) ON Year",
            "WITH c AS (SELECT 1) PIVOT c ON x USING sum(y)",
            "UNPIVOT monthly_sales ON jan, feb, mar INTO NAME month VALUE sales",
            "SELECT * FROM Cities PIVOT (sum(Population) FOR Year IN (2000 AS y2000, 2010))",
            "SELECT * FROM Cities PIVOT (sum(Population) AS total, count(*) AS n FOR Year IN (2000) GROUP BY Country) AS p",
            "SELECT * FROM t UNPIVOT INCLUDE NULLS (v FOR n IN (a, b))",
            "SELECT * FROM t UNPIVOT ((v1, v2) FOR n IN ((a, b) AS 'g1', (c, d)))",
            "SELECT * FROM (PIVOT Cities ON Year USING sum(Population)) AS p",
            "SELECT * FROM test PIVOT (sum(x) FOR y IN ('z', 'q')) UNPIVOT (x FOR y IN (z, q)) AS x",
            "PIVOT Cities ON Year IN (SELECT Year FROM Cities) USING sum(Population) ORDER BY ALL",
            "SELECT * FROM s PIVOT (sum(amount) FOR m IN month_enum) AS p",
            "SELECT * FROM s PIVOT (sum(amount) FOR y IN (2020, 2021) m IN ('JAN')) AS p",
            // an INTEGER cast: Lenient's target spelling table rewrites `VARCHAR` to
            // `CHARACTER VARYING`, so the byte-identity case uses the stable name
            // (the expression-entry acceptance itself is covered above).
            "UNPIVOT t ON (a + 100)::INTEGER, b INTO NAME n VALUE v",
        ] {
            let parsed =
                parse_with(sql, crate::ParseConfig::new(DuckDb)).expect("the pivot form parses");
            assert_eq!(
                Renderer::new(Lenient)
                    .render_parsed(&parsed)
                    .expect("the pivot form renders"),
                sql,
            );
        }
    }

    #[test]
    fn lambda_syntax_is_rejected_or_rereads_where_the_gate_is_off() {
        // ANSI never lexes `->` (its JSON-arrow gate is off), so the lambda spelling
        // is a clean parse error there; PostgreSQL lexes `->` as its JSON accessor
        // and must keep that reading — same text, `JsonGet` node — proving the lambda
        // gate cannot bleed through the inherited surface.
        use crate::ast::{BinaryOperator, Expr, SetExpr, Statement};
        assert!(parse_with("SELECT x -> x + 1", crate::ParseConfig::new(Ansi)).is_err());
        assert!(parse_with("SELECT (x, y) -> x + y", crate::ParseConfig::new(Ansi)).is_err());

        let parsed = parse_with("SELECT x -> x + 1", crate::ParseConfig::new(Postgres))
            .expect("PG reads the JSON arrow");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a plain SELECT body");
        };
        let crate::ast::SelectItem::Expr { expr, .. } = &select.projection[0] else {
            panic!("expected an expression item");
        };
        assert!(matches!(
            expr,
            Expr::BinaryOp {
                op: BinaryOperator::JsonGet,
                ..
            }
        ));
    }

    #[test]
    fn duckdb_parses_the_wildcard_modifiers_onto_the_wildcard_item() {
        // The EXCLUDE/REPLACE/RENAME tail lands in the wildcard's `options` (probed on
        // 1.5.4): EXCLUDE entries may be qualified (`t.a`), REPLACE pairs an expression
        // with its output column, RENAME pairs a (possibly qualified) source column
        // with its new name — all three stack in DuckDB's fixed order.
        use crate::ast::{Resolver as _, SelectItem};
        let parsed = parse_with(
            "SELECT * EXCLUDE (a, t.b) REPLACE (c / 1000 AS c) RENAME (d AS e) FROM t",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the stacked modifiers parse");
        let select = select_of(&parsed);
        let SelectItem::Wildcard {
            options: Some(options),
            ..
        } = &select.projection[0]
        else {
            panic!("expected a modifier-bearing wildcard");
        };
        assert_eq!(options.exclude.len(), 2);
        assert_eq!(options.exclude[1].0.len(), 2, "`t.b` keeps its qualifier");
        assert_eq!(options.replace.len(), 1);
        assert_eq!(
            parsed.resolver().resolve(options.replace[0].column.sym),
            "c",
        );
        assert_eq!(options.rename.len(), 1);
        assert_eq!(parsed.resolver().resolve(options.rename[0].alias.sym), "e");
    }

    #[test]
    fn duckdb_parses_the_bare_single_item_modifier_forms() {
        // Parens are optional for exactly one item (`EXCLUDE a`, `REPLACE x/2 AS x`,
        // `RENAME a AS b`; probed on 1.5.4). The bare EXCLUDE takes a single column
        // only: `* EXCLUDE a, b` reads `b` as a *second projection item* (the engine
        // parses it that way, not as a two-column exclude).
        use crate::ast::SelectItem;
        for sql in [
            "SELECT * EXCLUDE a FROM t",
            "SELECT * EXCLUDE t.a FROM t",
            "SELECT * REPLACE x / 2 AS x FROM t",
            "SELECT * RENAME a AS b FROM t",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }
        let parsed = parse_with(
            "SELECT * EXCLUDE a, b FROM t",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the bare form parses");
        let select = select_of(&parsed);
        assert_eq!(
            select.projection.len(),
            2,
            "`b` is a second projection item"
        );
        let SelectItem::Wildcard {
            options: Some(options),
            ..
        } = &select.projection[0]
        else {
            panic!("expected the modifier-bearing wildcard first");
        };
        assert_eq!(options.exclude.len(), 1);
    }

    #[test]
    fn duckdb_wildcard_modifiers_keep_the_engines_fixed_order() {
        // DuckDB fixes EXCLUDE -> REPLACE -> RENAME, each at most once (any other
        // order or a repeat syntax-errors; probed on 1.5.4). Parsing them in sequence
        // reproduces the reject: the out-of-place keyword is left unconsumed.
        for sql in [
            "SELECT * REPLACE (b + 1 AS b) EXCLUDE (a) FROM t",
            "SELECT * RENAME (c AS d) REPLACE (b + 1 AS b) FROM t",
            "SELECT * RENAME (c AS d) EXCLUDE (a) FROM t",
            "SELECT * EXCLUDE (a) EXCLUDE (b) FROM t",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB rejects the out-of-order modifiers: {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_parses_qualified_wildcard_modifiers() {
        use crate::ast::SelectItem;
        let parsed = parse_with(
            "SELECT t.* EXCLUDE (a) FROM t",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("qualified wildcard modifiers parse");
        let select = select_of(&parsed);
        let SelectItem::QualifiedWildcard {
            options: Some(options),
            ..
        } = &select.projection[0]
        else {
            panic!("expected a modifier-bearing qualified wildcard");
        };
        assert_eq!(options.exclude.len(), 1);
    }

    #[test]
    fn duckdb_parses_returning_wildcard_modifiers_and_columns() {
        // The RETURNING output list shares the projection-item grammar, so the
        // wildcard modifiers and the COLUMNS selector compose there too (corpus
        // statements: `RETURNING * EXCLUDE c1`, `RETURNING COLUMNS('a|c') + 42`).
        for sql in [
            "INSERT INTO v0 VALUES (1), (2) RETURNING * EXCLUDE c1",
            "DELETE FROM v0 WHERE c1 = 0 RETURNING * EXCLUDE c1",
            "UPDATE v0 SET c1 = 0 WHERE true RETURNING v0.* EXCLUDE c1",
            "INSERT INTO table1 VALUES (1, 2, 3) RETURNING COLUMNS('a|c')",
            "INSERT INTO table1 VALUES (1, 2, 3) RETURNING COLUMNS('a|c') + 42",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }
    }

    #[test]
    fn duckdb_parses_the_star_alias() {
        // DuckDB admits an alias on a star projection: `SELECT * AS idx` renames *every*
        // star-expanded column `idx` (a rename-all, not a struct pack; engine-probed on
        // 1.5.4). It lands in the wildcard item's new `alias`/`alias_spelling` slots via
        // the reused projection-alias machinery, so the spelling tag rides for free.
        use crate::ast::{AliasSpelling, Resolver as _, SelectItem};
        let parsed = parse_with("SELECT * AS idx FROM t", crate::ParseConfig::new(DuckDb))
            .expect("`* AS idx` parses");
        let select = select_of(&parsed);
        let SelectItem::Wildcard {
            alias: Some(alias),
            alias_spelling,
            ..
        } = &select.projection[0]
        else {
            panic!("expected an aliased wildcard");
        };
        assert_eq!(parsed.resolver().resolve(alias.sym), "idx");
        assert_eq!(*alias_spelling, AliasSpelling::As);

        // The bare (AS-less) form keeps its own spelling tag.
        let parsed = parse_with("SELECT * idx FROM t", crate::ParseConfig::new(DuckDb))
            .expect("`* idx` parses");
        let select = select_of(&parsed);
        let SelectItem::Wildcard {
            alias: Some(alias),
            alias_spelling,
            ..
        } = &select.projection[0]
        else {
            panic!("expected a bare-aliased wildcard");
        };
        assert_eq!(parsed.resolver().resolve(alias.sym), "idx");
        assert_eq!(*alias_spelling, AliasSpelling::Bare);

        // The qualified star aliases too, and the alias co-travels with the modifier
        // tail (written after it).
        let parsed = parse_with("SELECT t.* AS x FROM t", crate::ParseConfig::new(DuckDb))
            .expect("`t.* AS x` parses");
        let select = select_of(&parsed);
        let SelectItem::QualifiedWildcard {
            alias: Some(alias), ..
        } = &select.projection[0]
        else {
            panic!("expected an aliased qualified wildcard");
        };
        assert_eq!(parsed.resolver().resolve(alias.sym), "x");

        for sql in [
            "SELECT * EXCLUDE (a) AS x FROM t",
            "SELECT * REPLACE (a + 1 AS a) AS x FROM t",
            "SELECT * EXCLUDE (a) x FROM t",
            "INSERT INTO t VALUES (1) RETURNING * AS x",
            "INSERT INTO t VALUES (1) RETURNING * EXCLUDE (a) AS x",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }
    }

    #[test]
    fn duckdb_star_alias_is_gated_and_ordered() {
        // The star alias rides the `wildcard_modifiers` gate: a non-DuckDB dialect leaves
        // the word unconsumed, so both the `AS` and bare forms reject (sqlite/ANSI reject
        // the star alias entirely; probed on 1.5.4).
        assert!(
            parse_with("SELECT * AS x FROM t", crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects the star `AS` alias",
        );
        assert!(
            parse_with("SELECT * x FROM t", crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects the bare star alias",
        );
        // The alias is written *after* the modifier tail, which is consumed first, so an
        // alias placed before it leaves the trailing modifier keyword unconsumed — a
        // syntax error there and here.
        assert!(
            parse_with(
                "SELECT * AS x EXCLUDE (a) FROM t",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err(),
            "an alias before the modifier tail rejects",
        );
    }

    #[test]
    fn duckdb_star_alias_round_trips_byte_identically() {
        use crate::dialect::Lenient;
        use crate::render::Renderer;

        // The canonical `AS` spellings render back byte-identically under Lenient (the
        // star-family precedent: DuckDb has no Tier-1 render target yet).
        for sql in [
            "SELECT * AS idx FROM t",
            "SELECT t.* AS x FROM t",
            "SELECT * EXCLUDE (a) AS x FROM t",
        ] {
            let parsed =
                parse_with(sql, crate::ParseConfig::new(DuckDb)).expect("the star alias parses");
            assert_eq!(
                Renderer::new(Lenient)
                    .render_parsed(&parsed)
                    .expect("the star alias renders"),
                sql,
            );
        }
        // The bare spelling is source-fidelity data: a `PreserveSource` render (`to_sql`)
        // keeps `* idx`, while a normalizing Lenient render canonicalizes it to the
        // trailing `AS` — the bare→`AS` alias precedent.
        let parsed = parse_with("SELECT * idx FROM t", crate::ParseConfig::new(DuckDb))
            .expect("the bare star alias parses");
        assert_eq!(parsed.to_sql(), "SELECT * idx FROM t");
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&parsed)
                .expect("the bare star alias renders"),
            "SELECT * AS idx FROM t",
        );
    }

    #[test]
    fn duckdb_parses_columns_selector_forms_as_the_dedicated_node() {
        // Every COLUMNS argument form lands on `Expr::Columns` (probed on 1.5.4): the
        // regex string, the whole-projection `*` (optionally with modifiers inside),
        // the lambda (composing with the landed lambda node), and the name list.
        use crate::ast::{Expr, SelectItem};
        for (sql, has_qualifier, has_pattern, has_options) in [
            ("SELECT COLUMNS('number\\d+') FROM tbl", false, true, false),
            ("SELECT COLUMNS(*) FROM t", false, false, false),
            ("SELECT COLUMNS(* EXCLUDE (i)) FROM t", false, false, true),
            (
                "SELECT COLUMNS(c -> c LIKE '%num%') FROM tbl",
                false,
                true,
                false,
            ),
            ("SELECT COLUMNS(['a', 'b']) FROM t", false, true, false),
            // The qualified star form rides the same node's `qualifier` (the engine's
            // single `relation_name` slot); modifiers compose inside it.
            ("SELECT COLUMNS(df1.*) FROM df1", true, false, false),
            (
                "SELECT sin(COLUMNS(df1.* EXCLUDE (key))) FROM df1",
                true,
                false,
                true,
            ),
        ] {
            let parsed =
                parse_with(sql, crate::ParseConfig::new(DuckDb)).expect("the COLUMNS form parses");
            let select = select_of(&parsed);
            let SelectItem::Expr { expr, .. } = &select.projection[0] else {
                panic!("expected an expression item for {sql:?}");
            };
            // The qualified-EXCLUDE case wraps its COLUMNS in `sin(…)` (the corpus
            // spelling); unwrap one call level to reach the node.
            let expr = match expr {
                Expr::Function { call, .. } => &call.args[0].value,
                expr => expr,
            };
            let Expr::Columns {
                qualifier,
                pattern,
                options,
                ..
            } = expr
            else {
                panic!("expected the COLUMNS node for {sql:?}");
            };
            assert_eq!(qualifier.is_some(), has_qualifier, "qualifier for {sql:?}");
            assert_eq!(pattern.is_some(), has_pattern, "pattern for {sql:?}");
            assert_eq!(options.is_some(), has_options, "options for {sql:?}");
        }
        // The engine takes exactly one qualifier part: `COLUMNS(s.t.*)` syntax-errors
        // (probed on 1.5.4), so the multi-part spelling stays a parse error here too.
        assert!(
            parse_with(
                "SELECT COLUMNS(s.t.*) FROM s.t",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
        let parsed = parse_with(
            "SELECT COLUMNS(c -> c LIKE '%num%') FROM tbl",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("parses");
        let select = select_of(&parsed);
        let SelectItem::Expr {
            expr:
                Expr::Columns {
                    pattern: Some(pattern),
                    ..
                },
            ..
        } = &select.projection[0]
        else {
            panic!("expected the COLUMNS node");
        };
        assert!(
            matches!(pattern.as_ref(), Expr::Lambda { .. }),
            "the lambda selector composes with the lambda node",
        );
    }

    #[test]
    fn duckdb_parses_columns_in_expression_position() {
        // COLUMNS is a star *expression*: it nests in calls, operators, and casts
        // (`sum(COLUMNS(*))`, `COLUMNS('a|c') + 42`, `COLUMNS(*)::VARCHAR`; each
        // executes on 1.5.4). The sweep's DISTINCT case is the coverage-gap example.
        for sql in [
            "SELECT sum(COLUMNS(*)) FROM t",
            "SELECT COLUMNS('a|c') + 42 FROM t",
            "SELECT COLUMNS(*)::VARCHAR FROM t",
            "SELECT DISTINCT COLUMNS('key') FROM (VALUES(1,1,2),(1,1,3)) AS t(key1,key2,v)",
            "SELECT * FROM grouped_table ORDER BY COLUMNS(*)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }
        // DuckDB takes exactly one COLUMNS argument (`COLUMNS(a, b)` syntax-errors).
        assert!(
            parse_with(
                "SELECT COLUMNS(a, b) FROM t",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
    }

    #[test]
    fn duckdb_parses_aggregate_internal_order_by_all_as_columns_star() {
        // `agg(x ORDER BY ALL)` — including WITHIN GROUP — is DuckDB's `COLUMNS(*)`
        // star expansion in the sort key (serializes identically to `ORDER BY
        // COLUMNS(*)`; probed on 1.5.4), with the direction/nulls modifiers on the
        // key. No sibling key is admitted, and the window position keeps its
        // dedicated engine reject.
        use crate::ast::{Expr, SelectItem};
        let parsed = parse_with(
            "SELECT string_agg(x ORDER BY ALL DESC NULLS FIRST) FROM t",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the aggregate-internal ALL parses");
        let select = select_of(&parsed);
        let SelectItem::Expr {
            expr: Expr::Function { call, .. },
            ..
        } = &select.projection[0]
        else {
            panic!("expected the aggregate call");
        };
        let [sole] = call.order_by.as_slice() else {
            panic!("expected the sole ALL key");
        };
        assert!(matches!(
            sole.expr,
            Expr::Columns {
                pattern: None,
                options: None,
                ..
            }
        ));
        assert_eq!(sole.asc, Some(false));
        assert_eq!(sole.nulls_first, Some(true));

        assert!(
            parse_with(
                "SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY ALL) FROM t",
                crate::ParseConfig::new(DuckDb),
            )
            .is_ok(),
            "WITHIN GROUP admits the ALL key too",
        );
        for sql in [
            "SELECT string_agg(x ORDER BY ALL, y) FROM t",
            "SELECT string_agg(x ORDER BY y, ALL) FROM t",
            "SELECT row_number() OVER (ORDER BY ALL) FROM t",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB rejects {sql:?} (engine-verified)",
            );
        }
    }

    #[test]
    fn duckdb_star_family_round_trips() {
        use crate::dialect::Lenient;
        use crate::render::Renderer;

        // Canonical spellings render back byte-identically under Lenient (the
        // `qualify` precedent: DuckDb has no Tier-1 render target yet).
        for sql in [
            "SELECT * EXCLUDE (a, t.b) REPLACE (c / 1000 AS c) RENAME (d AS e) FROM t",
            "SELECT t.* EXCLUDE (a) FROM t",
            "SELECT COLUMNS('number\\d+') FROM tbl",
            "SELECT COLUMNS(*) FROM t",
            "SELECT COLUMNS(* EXCLUDE (i)) FROM t",
            "SELECT COLUMNS(df1.* EXCLUDE (key)) FROM df1",
            "SELECT sum(COLUMNS(*)) FROM t",
            // `::INTEGER` rather than the corpus's `::VARCHAR`: Lenient's Tier-1 type
            // table spells VARCHAR `CHARACTER VARYING`, which would fail the *byte*
            // check on the type name — orthogonal to the star family under test.
            "SELECT COLUMNS(*)::INTEGER FROM t",
        ] {
            let parsed =
                parse_with(sql, crate::ParseConfig::new(DuckDb)).expect("the star form parses");
            assert_eq!(
                Renderer::new(Lenient)
                    .render_parsed(&parsed)
                    .expect("the star form renders"),
                sql,
            );
        }
        // The bare single-item forms canonicalize to the parenthesized spelling —
        // the same construct, so the render re-parses to a structurally equal tree.
        let parsed = parse_with("SELECT * EXCLUDE a FROM t", crate::ParseConfig::new(DuckDb))
            .expect("bare form parses");
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&parsed)
                .expect("bare form renders"),
            "SELECT * EXCLUDE (a) FROM t",
        );
        // The aggregate-internal ALL canonicalizes to its engine-identical
        // `COLUMNS(*)` spelling.
        let parsed = parse_with(
            "SELECT string_agg(x ORDER BY ALL) FROM t",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("ALL parses");
        assert_eq!(
            Renderer::new(Lenient)
                .render_parsed(&parsed)
                .expect("ALL renders"),
            "SELECT string_agg(x ORDER BY COLUMNS(*)) FROM t",
        );
    }

    #[test]
    fn duckdb_parses_star_columns_unpack_prefix() {
        // `*COLUMNS(...)` spreads the selected columns into the enclosing call / `IN`-list
        // argument list (each executes on 1.5.4). The prefix `*` is unpack only in primary
        // position, so it lands on `Expr::Columns` with the `Unpack` spelling.
        use crate::ast::{ColumnsSpelling, Expr, SelectItem};
        let parsed = parse_with(
            "SELECT struct_pack(*COLUMNS(*)) FROM integers",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the unpack prefix parses");
        let select = select_of(&parsed);
        let SelectItem::Expr {
            expr: Expr::Function { call, .. },
            ..
        } = &select.projection[0]
        else {
            panic!("expected the struct_pack call");
        };
        assert!(matches!(
            call.args[0].value,
            Expr::Columns {
                spelling: ColumnsSpelling::Unpack,
                pattern: None,
                ..
            },
        ));
        for sql in [
            "SELECT COALESCE(*COLUMNS('id')) FROM integers",
            "SELECT CONCAT(*COLUMNS(*), *COLUMNS(*)) FROM integers",
            "SELECT 2 IN (*COLUMNS(*)) FROM integers",
            "SELECT struct_pack(*COLUMNS(* EXCLUDE (id))) FROM integers",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }
        // A *leading* `*COLUMNS(...)` in a select projection is the unpack expression, not
        // the bare `SELECT *` wildcard: `SELECT *COLUMNS('a') + 42` reads as an ordinary
        // value expression (`*COLUMNS('a')` folded into the `+ 42`), so the projection item
        // is an `Expr` whose head is `Expr::Columns`/`Unpack`, never `SelectItem::Wildcard`.
        let parsed = parse_with(
            "SELECT *COLUMNS('a') + 42 FROM integers",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the leading unpack prefix parses in projection position");
        let select = select_of(&parsed);
        let SelectItem::Expr {
            expr: Expr::BinaryOp { left, .. },
            ..
        } = &select.projection[0]
        else {
            panic!("expected a `*COLUMNS('a') + 42` binary expression, not a wildcard");
        };
        assert!(matches!(
            **left,
            Expr::Columns {
                spelling: ColumnsSpelling::Unpack,
                pattern: Some(_),
                ..
            },
        ));
        // The infix `*` never collides: `id * COLUMNS(*)` is multiplication (the climb
        // loop consumes the star after its left operand), not unpack.
        let parsed = parse_with(
            "SELECT id * COLUMNS(*) FROM integers",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the multiplication parses");
        let select = select_of(&parsed);
        let SelectItem::Expr { expr, .. } = &select.projection[0] else {
            panic!("expected an expression item");
        };
        assert!(
            matches!(expr, Expr::BinaryOp { .. }),
            "infix `*` is a binary op"
        );
    }

    #[test]
    fn duckdb_double_equals_binds_as_a_generic_operator() {
        // DuckDB lexes `==` as a generic `%left Op`, not the `%nonassoc '='` comparison, so
        // it binds *tighter than* the comparisons and looser than additive, left-associative
        // (measured on 1.5.4: `1 = 2 == 3` is `1 = (2 = 3)`, `1 == 2 == 3` is `((1 = 2) = 3)`,
        // `1 < 2 == 3` is `1 < (2 = 3)`). `1 = 2 == 3` therefore parses — the `==` is the
        // right operand of `=`, never a second comparison in a chain.
        use crate::ast::{BinaryOperator, EqualsSpelling, Expr, SelectItem};
        let parsed = parse_with("SELECT 1 = 2 == 3", crate::ParseConfig::new(DuckDb))
            .expect("`=` then `==` parses");
        let select = select_of(&parsed);
        let SelectItem::Expr {
            expr:
                Expr::BinaryOp {
                    op: BinaryOperator::Eq(EqualsSpelling::Single),
                    right,
                    ..
                },
            ..
        } = &select.projection[0]
        else {
            panic!("expected the outer `=` with a `==` right operand");
        };
        assert!(
            matches!(
                **right,
                Expr::BinaryOp {
                    op: BinaryOperator::Eq(EqualsSpelling::Double),
                    ..
                },
            ),
            "`==` binds tighter, so it is the right operand of `=`",
        );
        for sql in [
            "SELECT 1 == 2 == 3",
            "SELECT 1 < 2 == 3",
            "SELECT 1 + 1 == 2",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }
        // The bare `=`/`<` comparisons stay non-associative: a genuine chain is still a
        // clean reject (only `==`, the generic operator, may chain).
        for sql in ["SELECT 1 = 2 = 3", "SELECT 1 < 2 < 3"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB rejects the non-associative chain {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_parses_bare_star_columns_in_order_by_and_unpivot() {
        // The bare `*` / `t.*` all-columns star (DuckDB's `columns:false` STAR node) is
        // admitted in the query-tail `ORDER BY` sort key and the `UNPIVOT` `ON`/`IN`
        // column positions, each carrying the optional wildcard modifiers (probed on
        // 1.5.4). It lands on `Expr::Columns` with the pattern-free `Star` spelling.
        use crate::ast::{ColumnsSpelling, Expr, Statement};
        let parsed = parse_with(
            "SELECT * FROM integers ORDER BY * DESC NULLS LAST",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("ORDER BY * parses");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let [sole] = query.order_by.as_slice() else {
            panic!("expected the sole bare-star sort key");
        };
        assert!(matches!(
            sole.expr,
            Expr::Columns {
                spelling: ColumnsSpelling::Star,
                pattern: None,
                qualifier: None,
                ..
            },
        ));
        assert_eq!(sole.asc, Some(false));
        assert_eq!(sole.nulls_first, Some(false));

        for sql in [
            "SELECT * FROM integers ORDER BY *",
            "SELECT * FROM integers ORDER BY * EXCLUDE (id)",
            "SELECT * FROM integers ORDER BY id, *",
            "SELECT * FROM integers t ORDER BY t.*",
            "SELECT * FROM t1 UNPIVOT (val FOR col IN (*))",
            "SELECT * FROM t1 UNPIVOT (val FOR col IN (* EXCLUDE (id)))",
            "UNPIVOT t1 ON * INTO NAME col VALUE val",
            "UNPIVOT t1 ON * EXCLUDE (id) INTO NAME col VALUE val",
            "UNPIVOT t1 ON *, id INTO NAME c VALUE v",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }

        // The bare star is a query-tail / aggregate sort key only: DuckDB syntax-rejects
        // it in a window `ORDER BY` ("Cannot ORDER BY ALL in a window expression"), so the
        // window position keeps its plain expression grammar and rejects the lone `*`.
        assert!(
            parse_with(
                "SELECT row_number() OVER (ORDER BY *) FROM integers",
                crate::ParseConfig::new(DuckDb),
            )
            .is_err(),
            "the window ORDER BY rejects the bare star",
        );
    }

    #[test]
    fn duckdb_columns_star_expansion_round_trips() {
        use crate::dialect::Lenient;
        use crate::render::Renderer;

        // Canonical spellings render back byte-identically under Lenient (DuckDb has no
        // Tier-1 render target yet — the `qualify` precedent).
        for sql in [
            "SELECT struct_pack(*COLUMNS(*)) FROM integers",
            "SELECT COALESCE(*COLUMNS('id')) FROM integers",
            "SELECT struct_pack(*COLUMNS(* EXCLUDE (id))) FROM integers",
            "SELECT * FROM integers ORDER BY * DESC NULLS LAST",
            "SELECT * FROM integers ORDER BY * EXCLUDE (id)",
            "SELECT * FROM integers AS t ORDER BY t.*",
            "SELECT * FROM t1 UNPIVOT (val FOR col IN (*))",
            "UNPIVOT t1 ON * EXCLUDE (id) INTO NAME col VALUE val",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .expect("the star-expansion form parses");
            assert_eq!(
                Renderer::new(Lenient)
                    .render_parsed(&parsed)
                    .expect("the star-expansion form renders"),
                sql,
            );
        }
    }

    #[test]
    fn star_columns_expansion_rejected_where_the_gate_is_off() {
        // Off the `columns_expression` gate none of the star-expansion surfaces exist:
        // the unpack `*` and the bare-`*` sort key are unexpected input, a clean parse
        // error rather than an over-acceptance (the differential-oracle guard).
        for sql in [
            "SELECT struct_pack(*COLUMNS(*)) FROM integers",
            "SELECT * FROM integers ORDER BY *",
            "SELECT * FROM integers ORDER BY * DESC",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects the star expansion: {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects the star expansion: {sql:?}",
            );
        }
    }

    #[test]
    fn wildcard_modifiers_are_rejected_where_the_gate_is_off() {
        // ANSI and PostgreSQL have no wildcard tail: a `*` item is complete, so the
        // trailing EXCLUDE/REPLACE/RENAME keyword is unconsumed input — a clean parse
        // error, not an over-acceptance (the differential-oracle guard).
        for sql in [
            "SELECT * EXCLUDE (a) FROM t",
            "SELECT * EXCLUDE a FROM t",
            "SELECT t.* EXCLUDE (a) FROM t",
            "SELECT * REPLACE (a + 1 AS a) FROM t",
            "SELECT * RENAME (a AS b) FROM t",
            "INSERT INTO v0 VALUES (1) RETURNING * EXCLUDE c1",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects the wildcard modifiers: {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects the wildcard modifiers: {sql:?}",
            );
        }
    }

    #[test]
    fn columns_stays_an_ordinary_function_call_where_the_gate_is_off() {
        // `COLUMNS` is a non-reserved word, so with `columns_expression` off the same
        // text keeps its function-call reading (the lambda no-bleed-through
        // precedent): same acceptance, different node.
        use crate::ast::{Expr, SelectItem, SetExpr, Statement};
        let parsed = parse_with(
            "SELECT COLUMNS('re') FROM t",
            crate::ParseConfig::new(Postgres),
        )
        .expect("PG reads a plain call");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a plain SELECT body");
        };
        let SelectItem::Expr { expr, .. } = &select.projection[0] else {
            panic!("expected an expression item");
        };
        assert!(
            matches!(expr, Expr::Function { .. }),
            "the gate-off reading is an ordinary call",
        );
    }

    #[test]
    fn duckdb_parses_the_python_keyword_lambda_as_a_lambda_node() {
        // DuckDB 1.3.0's preferred `lambda x: body` spelling folds onto the same
        // `Expr::Lambda` node as the arrow, tagged `Keyword` (each executes on 1.5.4).
        // These are the list-function coverage-gap surfaces from the corpus.
        use crate::ast::{Expr, FunctionArg, LambdaParamSpelling, SelectItem};
        for (sql, params) in [
            ("SELECT list_filter(NULL, lambda x: x)", 1),
            ("SELECT list_transform(bb, lambda x: [x, b])", 1),
            ("SELECT list_reduce(a, lambda x, y: x + y)", 2),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .expect("the keyword lambda parses");
            let select = select_of(&parsed);
            let SelectItem::Expr {
                expr: Expr::Function { call, .. },
                ..
            } = &select.projection[0]
            else {
                panic!("expected the list-function call for {sql:?}");
            };
            let FunctionArg { value, .. } = &call.args[1];
            let Expr::Lambda { lambda, .. } = value else {
                panic!("expected the lambda argument for {sql:?}, got {value:?}");
            };
            assert_eq!(lambda.params.len(), params, "params for {sql:?}");
            assert_eq!(
                lambda.spelling,
                LambdaParamSpelling::Keyword,
                "spelling for {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_keyword_lambda_composes_with_columns() {
        // The corpus COLUMNS-hosted keyword lambdas — the selector predicate and the
        // ORDER BY sort key (each executes on 1.5.4). The lambda body ranges over the
        // full expression grammar (the subscript `x[-1]`, the `IN` list).
        for sql in [
            "SELECT COLUMNS(lambda x: x <> 'i') FROM integers",
            "SELECT * FROM tbl ORDER BY COLUMNS(lambda x: x[-1] IN ('2', '3'))",
            "SELECT COLUMNS(lambda x: x LIKE 'col%') FROM integers",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }
    }

    #[test]
    fn duckdb_reserves_lambda_in_expression_position_like_the_engine() {
        // A `lambda` word opens the production unconditionally under the gate, matching
        // DuckDB, which reserves `lambda`: a bare `lambda`, or one not followed by
        // `<params>:`, is the same syntax error the engine reports (probed on 1.5.4). The
        // `AS`-alias spelling `SELECT 1 AS lambda` stays legal there and here (`lambda` is
        // not added to the keyword inventory, so other dialects keep it a free name).
        for sql in [
            "SELECT lambda",
            "SELECT lambda FROM t",
            "SELECT lambda AS x",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB rejects {sql:?}"
            );
        }
        assert!(
            parse_with("SELECT 1 AS lambda", crate::ParseConfig::new(DuckDb)).is_ok(),
            "`lambda` is still a valid AS label",
        );
        assert!(
            parse_with("SELECT lambda FROM t", crate::ParseConfig::new(Ansi)).is_ok(),
            "ANSI leaves `lambda` a free column name (not reserved)",
        );
    }

    #[test]
    fn keyword_lambda_is_rejected_where_the_gate_is_off() {
        // ANSI/PostgreSQL have no keyword lambda: `lambda` reads as an ordinary column, so
        // the trailing param leaves `x` as unconsumed input — a clean parse error, never
        // an over-acceptance (the differential-oracle guard).
        for sql in [
            "SELECT list_filter(NULL, lambda x: x)",
            "SELECT COLUMNS(lambda x: x <> 'i') FROM integers",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_parses_list_comprehensions() {
        // The python-style list comprehension and its DuckDB column-star sources — the
        // corpus coverage-gap surfaces (each executes on 1.5.4 inside `COLUMNS(…)`), plus
        // a general list-valued source (parse-accepted; bind-time-only outside `COLUMNS`).
        for sql in [
            "SELECT COLUMNS([x for x in *]) FROM integers",
            "SELECT COLUMNS([x for x in (*) if x <> 'i']) FROM integers",
            "SELECT COLUMNS([x for x in (* EXCLUDE (i))]) FROM integers",
            "SELECT COLUMNS([x for x in (*) if x LIKE 'i']) FROM integers",
            "SELECT COLUMNS([x for x in (* REPLACE (i AS i))]) FROM integers",
            "SELECT [x + 1 for x in [1, 2, 3] if x > 1]",
            "SELECT [upper(x) for x in ['a', 'b']]",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB parses {sql:?}"
            );
        }
    }

    #[test]
    fn duckdb_list_comprehension_shape() {
        // The general form carries element/var/source/filter; the column-star source
        // keeps its parenthesization and wildcard modifiers.
        use crate::ast::{ArrayExpr, ComprehensionSource, Expr, SelectItem};
        let parsed = parse_with(
            "SELECT [x + 1 for x in [1, 2, 3] if x > 1]",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("parses");
        let select = select_of(&parsed);
        let SelectItem::Expr {
            expr: Expr::Array { array, .. },
            ..
        } = &select.projection[0]
        else {
            panic!("expected the array expr");
        };
        let ArrayExpr::Comprehension { comprehension, .. } = &**array else {
            panic!("expected a comprehension");
        };
        assert!(comprehension.filter.is_some(), "the if-filter is captured");
        assert!(
            matches!(comprehension.source, ComprehensionSource::Expr { .. }),
            "a general list source",
        );

        let parsed = parse_with(
            "SELECT [x for x in (* EXCLUDE (i))]",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("star source parses");
        let select = select_of(&parsed);
        let SelectItem::Expr {
            expr: Expr::Array { array, .. },
            ..
        } = &select.projection[0]
        else {
            panic!("expected the array expr");
        };
        let ArrayExpr::Comprehension { comprehension, .. } = &**array else {
            panic!("expected a comprehension");
        };
        let ComprehensionSource::Star {
            parenthesized,
            options,
            ..
        } = &comprehension.source
        else {
            panic!("expected a column-star source");
        };
        assert!(*parenthesized, "the `(*)` spelling is recorded");
        assert!(options.is_some(), "the EXCLUDE modifiers are captured");
    }

    #[test]
    fn list_comprehension_is_rejected_where_the_gate_is_off() {
        // ANSI/PostgreSQL have no bracket list grammar, so `[` in expression position is a
        // clean parse error — the comprehension cannot bleed through.
        for sql in [
            "SELECT [x for x in [1, 2, 3]]",
            "SELECT [x + 1 for x in [1, 2, 3] if x > 1]",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_python_style_round_trips_byte_identically() {
        use crate::dialect::Lenient;
        use crate::render::Renderer;

        // DuckDb has no Tier-2 render target yet, so the round-trip renders under Lenient
        // — the QUALIFY/PIVOT precedent — proving the node and spelling tags, not the
        // target dialect, drive the emitted surface. The bare `*` / `(* …)` star sources
        // keep their spelling (they cannot canonicalize to `COLUMNS(*)`, which DuckDB
        // rejects nested).
        for sql in [
            "SELECT list_filter(NULL, lambda x: x)",
            "SELECT list_reduce(a, lambda x, y: x + y)",
            "SELECT list_transform(bb, lambda x: [x, b])",
            "SELECT COLUMNS(lambda x: x <> 'i') FROM integers",
            "SELECT [x + 1 for x in [1, 2, 3] if x > 1]",
            "SELECT COLUMNS([x for x in *]) FROM integers",
            "SELECT COLUMNS([x for x in (*) if x <> 'i']) FROM integers",
            "SELECT COLUMNS([x for x in (* EXCLUDE (i))]) FROM integers",
            "SELECT COLUMNS([x for x in (* REPLACE (i AS i))]) FROM integers",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .expect("the python-style form parses");
            assert_eq!(
                Renderer::new(Lenient)
                    .render_parsed(&parsed)
                    .expect("the python-style form renders"),
                sql,
            );
        }
    }
}
