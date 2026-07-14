// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The MySQL dialect.
//!
//! The whole module is gated by the `mysql` cargo feature (one `#[cfg]` on its
//! `mod` declaration), so the struct, the `Dialect` impl, and the MySQL test cluster
//! are compiled only when the feature is on — no per-item gates.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;

/// The MySQL dialect ([`FeatureSet::MYSQL`]) — the third shipped dialect, added to
/// validate that dialect-as-data scales to a maximally-different dialect.
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(MySql))`. It
/// diverges from [`Ansi`](super::Ansi) across many *data* dimensions (backtick
/// identifier quotes, `#` comments, `&&`-as-`AND`, `||`-as-`OR`, `"..."` strings with
/// backslash escapes, `0x`/`0b` numbers, `?` placeholders) plus one new gated grammar
/// production — the `LIMIT <offset>, <count>` comma form. The per-dimension "how clean
/// was dialect #3" verdict lives on [`FeatureSet::MYSQL`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MySql;

impl Dialect for MySql {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::MYSQL
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        BinaryOperator, DataType, DoubleTypeName, Expr, LimitSyntax, SelectItem, SetExpr,
        Signedness, Statement, TextTypeName, TimeZone, TimestampTypeName,
    };
    use crate::dialect::test_support::{assert_full_grammar, first_column_type};
    use crate::parse_with;
    use crate::parser::Parsed;

    /// Tier-1 canonical render of the first statement, parsed under MySQL. Canonical
    /// mode keeps source spellings (PreserveSource), so a MySQL-spelled type
    /// round-trips verbatim. `target: FeatureSet::MYSQL` (not the `RenderConfig`
    /// default, which is ANSI) so binding-power-driven parenthesization reads the
    /// same dialect the statement was parsed under — load-bearing now that MySQL's
    /// comparison row is `Left`, not `STANDARD`'s `NonAssoc`
    /// (mysql-comparison-operators-are-left-associative).
    fn mysql_render(sql: &str) -> String {
        use squonk_ast::render::{RenderConfig, RenderCtx, RenderExt, RenderMode};

        let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let config = RenderConfig {
            mode: RenderMode::Canonical,
            target: FeatureSet::MYSQL,
            ..RenderConfig::default()
        };
        let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
        parsed.statements()[0].displayed(&ctx).to_string()
    }

    #[test]
    fn mysql_parses_the_full_m1_select_grammar() {
        // The representative query uses only constructs MySQL shares with ANSI, so it
        // drives the full shared grammar identically — the "parses the same where the
        // semantics match" structural proof for dialect #3.
        assert_full_grammar(MySql);
    }

    #[test]
    fn mysql_parses_its_distinctive_surface() {
        // Each line is a MySQL-only lexical/grammar choice the preset selects as data.
        for sql in [
            "SELECT `id`, `name` FROM `users`",  // backtick identifier quotes
            "SELECT 1 # a MySQL line comment\n", // `#` line comment
            "SELECT a FROM t WHERE a = 1 && b = 2", // `&&` as logical AND
            "SELECT a FROM t LIMIT 5, 10",       // `LIMIT <offset>, <count>`
            "SELECT a FROM t LIMIT 10",          // the plain form still parses
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_ok(),
                "MySQL parses {sql:?}"
            );
        }
    }

    #[test]
    fn mysql_rejects_the_over_acceptances_the_tightening_closed() {
        // One representative per family the over-acceptance tightening gated off — each a
        // statement the fitted preset used to accept that live mysql:8 syntax-rejects
        // (engine-measured via the corpus_mysql_verdicts sweep).
        for sql in [
            "SELECT * FROM t FETCH FIRST 1 ROWS ONLY", // FETCH FIRST (MySQL uses LIMIT)
            "SELECT listagg(x) WITHIN GROUP (ORDER BY x)", // WITHIN GROUP ordered-set aggregate
            "SELECT sum(x) FILTER (WHERE x > 1)",      // FILTER (WHERE …) aggregate filter
            "CREATE TABLE p (x INT GENERATED ALWAYS AS IDENTITY)", // GENERATED … AS IDENTITY
            "CREATE MATERIALIZED VIEW mv AS SELECT 1", // materialized views
            "CREATE TABLE z WITH (FORMAT='parquet') AS SELECT 1", // WITH (storage params)
            "SELECT * FROM a.b.c",                     // three-part relation name
            "SELECT CAST(a AS INT)",                   // MySQL CAST target is narrow
            "SELECT CAST(a AS VARCHAR)", // (VARCHAR is a column type, not a cast target)
            "SELECT CAST(a AS TIMESTAMP)", // (DATETIME is the cast target, not TIMESTAMP)
            "SELECT CAST(a AS GEOMETRY)", // bare GEOMETRY is a column type, not a `cast_type`
            "SELECT ARRAY(1, 2, 3)",     // ARRAY is reserved in MySQL 8
            // The residual families closed by `mysql-preset-over-acceptance-residual`.
            "SELECT 1 FROM x AS a FULL OUTER JOIN y AS b ON a.c = b.c", // MySQL has no FULL join
            "SELECT * FROM t LIMIT 1 + 1", // LIMIT admits only an integer literal or `?`
            "SELECT * FROM t LIMIT (SELECT 1)", // (a subquery LIMIT is a syntax error)
            "CREATE TEMPORARY VIEW v AS SELECT 1", // MySQL has temporary tables, not views
            "CREATE OR REPLACE TEMPORARY VIEW v AS SELECT 1",
            "CREATE TABLE a (b INT) ON COMMIT PRESERVE ROWS", // ON COMMIT is not MySQL (non-temp)
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "MySQL should reject the tightened over-acceptance {sql:?}",
            );
        }

        // The valid MySQL forms the same gates must keep accepting (the other direction) —
        // including the two forms deliberately left tolerant because their tightening needs
        // a position-aware split MySQL admits in one position only (a derived-table
        // column-list alias, and a reserved word in dotted-name-continuation position).
        for sql in [
            "SELECT a FROM t LIMIT 10",         // LIMIT is MySQL's row limit
            "SELECT * FROM a.b",                // schema.table (two parts) is fine
            "SELECT 1 AS my_alias",             // a non-reserved alias
            "SELECT * FROM (SELECT 1) AS c(x)", // derived-table column-list alias (MySQL accepts)
            "SELECT t.select FROM t", // reserved word in dotted continuation (MySQL parses)
            "SELECT CAST(a AS SIGNED)", // the valid MySQL cast targets
            "SELECT CAST(a AS UNSIGNED)",
            "SELECT CAST(a AS CHAR)",
            "SELECT CAST(a AS DECIMAL(5, 2))",
            "SELECT CAST(a AS DATETIME)",
            "SELECT CAST(a AS JSON)",
            "SELECT CAST(a AS YEAR)", // YEAR is a MySQL cast target (8.0.22+)
            "SELECT CAST(a AS POINT)", // the spatial cast targets (8.0.17+)
            "SELECT CAST(a AS MULTIPOLYGON)",
            "SELECT CAST(a AS GEOMETRYCOLLECTION)",
            "SELECT CAST(a AS geomcollection)", // the GEOMCOLLECTION alias, case-insensitive
            // The other direction of the residual gates: the valid forms must still parse.
            "SELECT 1 FROM x AS a LEFT OUTER JOIN y AS b ON a.c = b.c", // LEFT/RIGHT remain
            "SELECT 1 FROM x FULL JOIN y ON x.c = y.c", // bare `FULL` reads as `x`'s alias
            "SELECT a FROM t LIMIT 10",                 // an integer LIMIT
            "SELECT a FROM t LIMIT ?",                  // a `?` placeholder LIMIT
            "SELECT a FROM t LIMIT 5, 10",              // the comma LIMIT form
            "CREATE VIEW v AS SELECT a FROM t",         // a plain (non-temporary) view
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_ok(),
                "MySQL should still accept the valid form {sql:?}",
            );
        }
    }

    #[test]
    fn mysql_faithful_cast_type_matrix() {
        // The faithful MySQL `cast_type` production (mysql-faithful-cast-type-production),
        // every case engine-measured on live mysql:8.4 via the corpus_mysql_verdicts /
        // m3 PREPARE oracle: syntax-accepted (1054/binding or a clean prepare) vs
        // syntax-rejected (1064). The parser must mirror the engine's *syntax* verdict.
        for sql in [
            // The spatial cast targets (8.0.17+) — syntax-accepted (their bare-connection
            // reject is a binding error, not 1064).
            "SELECT CAST(a AS POINT)",
            "SELECT CAST(a AS LINESTRING)",
            "SELECT CAST(a AS POLYGON)",
            "SELECT CAST(a AS MULTIPOINT)",
            "SELECT CAST(a AS MULTILINESTRING)",
            "SELECT CAST(a AS MULTIPOLYGON)",
            "SELECT CAST(a AS GEOMETRYCOLLECTION)",
            "SELECT CAST(a AS geomcollection)", // the GEOMCOLLECTION alias, case-insensitive
            // YEAR (8.0.22+), no tail.
            "SELECT CAST(a AS YEAR)",
            // `SIGNED`/`UNSIGNED` with the inert, optional trailing `INTEGER`/`INT`.
            "SELECT CAST(a AS SIGNED)",
            "SELECT CAST(a AS UNSIGNED)",
            "SELECT CAST(a AS SIGNED INTEGER)",
            "SELECT CAST(a AS UNSIGNED INTEGER)",
            "SELECT CAST(a AS SIGNED INT)",
            "SELECT CAST(a AS UNSIGNED INT)",
            // The scalar cast targets, with their precision/length tails.
            "SELECT CAST(a AS DECIMAL(10, 2))",
            "SELECT CAST(a AS DEC(10, 2))",
            "SELECT CAST(a AS CHAR(5))",
            "SELECT CAST(a AS NCHAR(5))",
            "SELECT CAST(a AS TIME(3))",
            "SELECT CAST(a AS DATETIME(6))",
            "SELECT CAST(a AS BINARY(8))",
            "SELECT CAST(a AS FLOAT(10))",
            "SELECT CAST(a AS DOUBLE)",
            "SELECT CAST(a AS DOUBLE PRECISION)",
            "SELECT CAST(a AS REAL)",
            "SELECT CAST(a AS JSON)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_ok(),
                "MySQL should accept the cast target {sql:?}",
            );
        }

        for sql in [
            // Bare `GEOMETRY` is a column type but not a `cast_type` (1064), unlike its
            // `GEOMETRYCOLLECTION` sibling.
            "SELECT CAST(a AS GEOMETRY)",
            // A spatial or `YEAR` cast target takes no argument/tail (1064).
            "SELECT CAST(a AS POINT(4))",
            "SELECT CAST(a AS YEAR(4))",
            // `NUMERIC` is not a cast target (only `DECIMAL`/`DEC` is); `FLOAT(M,D)` is a
            // column-only form; both 1064 in cast position.
            "SELECT CAST(a AS NUMERIC)",
            "SELECT CAST(a AS NUMERIC(10, 2))",
            "SELECT CAST(a AS FLOAT(10, 2))",
            // The common column types that are cast-position syntax errors.
            "SELECT CAST(a AS INT)",
            "SELECT CAST(a AS INTEGER)",
            "SELECT CAST(a AS VARCHAR(10))",
            "SELECT CAST(a AS TEXT)",
            "SELECT CAST(a AS TIMESTAMP)",
            "SELECT CAST(a AS BOOLEAN)",
            "SELECT CAST(a AS BLOB)",
            // `UUID` is a first-class `DataType::Uuid`, but MySQL has no such type: it is
            // absent from the `cast_type` set, so the target gate rejects it (1064) exactly
            // as it did the former user-defined `UUID` name — first-classing changed the
            // identity, not the acceptance.
            "SELECT CAST(a AS UUID)",
            "SELECT CAST(a AS mytype)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "MySQL should reject the non-cast-target {sql:?}",
            );
        }
    }

    #[test]
    fn mysql_rejects_the_position_aware_and_extended_families_of_round_five() {
        // One representative per family `mysql-preset-over-acceptance-residual` round-5
        // gated off — each engine-measured `ER_PARSE_ERROR` on live mysql:8.
        for sql in [
            // Reserved word as an `AS` projection alias (routed to `reserved_bare_alias`).
            "SELECT 1 AS range",
            "SELECT 1 AS delete",
            "SELECT a AS all FROM t", // `ALL` is reserved
            "SELECT 1 AS left",       // `LEFT` is a type_func_name built-in (bare alias set)
            "SELECT 1 AS numeric",
            // Base-table column-list alias (the base-vs-derived split).
            "SELECT * FROM x AS y(a, b)",
            "SELECT * FROM x y(a, b)",
            // Bare `OFFSET` with no preceding `LIMIT` (leading_offset off).
            "SELECT 1 OFFSET 1",
            "(SELECT 1) UNION (SELECT 2) OFFSET 2",
            "SELECT 1 FROM t ORDER BY 1 OFFSET 1",
            // Extended `ALTER TABLE` existence guards.
            "ALTER TABLE t ADD COLUMN IF NOT EXISTS k INT",
            "ALTER TABLE IF EXISTS t ADD COLUMN k INT",
            "ALTER TABLE t DROP COLUMN IF EXISTS k",
            "ALTER TABLE t DROP CONSTRAINT IF EXISTS c",
            // PostgreSQL `ALTER COLUMN` type / nullability actions (MySQL uses MODIFY/CHANGE).
            "ALTER TABLE t ALTER COLUMN i SET DATA TYPE VARCHAR(5)",
            "ALTER TABLE t ALTER COLUMN i TYPE INT",
            "ALTER TABLE t ALTER COLUMN i SET NOT NULL",
            "ALTER TABLE t ALTER COLUMN i DROP NOT NULL",
            // Deferrable constraint characteristics (both CREATE and ALTER positions).
            "CREATE TABLE foo (a INT REFERENCES b (id) DEFERRABLE)",
            "ALTER TABLE t ADD CONSTRAINT c FOREIGN KEY (a) REFERENCES b (id) INITIALLY DEFERRED",
            // `CREATE TABLE … AS SELECT … WITH [NO] DATA`.
            "CREATE TABLE t AS SELECT 1 WITH NO DATA",
            "CREATE TABLE t AS SELECT 1 WITH DATA",
            // `UPDATE … FROM`.
            "UPDATE t SET a = 1 FROM u",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "MySQL should reject the round-5 over-acceptance {sql:?}",
            );
        }

        // The other direction: valid MySQL the same gates must keep accepting — the
        // position/base-vs-derived split, and every base ALTER/constraint/CTAS form.
        for sql in [
            "SELECT 1 AS any",  // ANY is non-reserved in MySQL — a valid alias
            "SELECT 1 AS some", // SOME too
            "SELECT 1 AS my_alias",
            "SELECT t.range FROM t", // reserved word in dotted continuation (parses)
            "SELECT * FROM (SELECT 1) AS c(x)", // derived-table column-list alias
            "SELECT 1 LIMIT 10 OFFSET 5", // trailing OFFSET after LIMIT stays valid
            "SELECT 1 LIMIT 5, 10",  // the comma LIMIT form
            "ALTER TABLE t ADD COLUMN k INT",
            "ALTER TABLE t DROP COLUMN k",
            "ALTER TABLE t ALTER COLUMN i SET DEFAULT 1",
            "ALTER TABLE t ALTER COLUMN i DROP DEFAULT",
            "ALTER TABLE t DROP CONSTRAINT c",
            "ALTER TABLE t ADD CONSTRAINT c FOREIGN KEY (a) REFERENCES b (id)",
            "CREATE TABLE t AS SELECT 1", // CTAS without the WITH DATA clause
            "UPDATE t SET a = 1",         // UPDATE without a FROM clause
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_ok(),
                "MySQL should still accept the valid form {sql:?}",
            );
        }
    }

    #[test]
    fn mysql_rejects_the_type_grammar_and_statement_families_of_round_six() {
        // One representative per family `mysql-preset-over-acceptance-residual` round-6
        // gated off — each engine-measured `ER_PARSE_ERROR` on live mysql:8.
        for sql in [
            // Length-less `VARCHAR`/`VARBINARY` (varchar_requires_length).
            "CREATE TABLE t (a VARCHAR)",
            "CREATE TABLE t (a VARBINARY)",
            // Zoned temporal types MySQL lacks (zoned_temporal_types off).
            "CREATE TABLE t (a TIMESTAMPTZ)",
            "CREATE TABLE t (a TIMESTAMP WITH TIME ZONE)",
            "ALTER TABLE t ADD COLUMN mtime TIMESTAMPTZ",
            // A functional / operator default without wrapping parens
            // (default_expression_requires_parens).
            "CREATE TABLE t (a INT DEFAULT UUID())",
            "CREATE TABLE t (a INT DEFAULT 1 + 2)",
            // A `CONSTRAINT <name>` prefix on a non-CHECK inline constraint
            // (named_inline_non_check_constraints off).
            "CREATE TABLE t (a INT CONSTRAINT c REFERENCES b)",
            "CREATE TABLE t (a INT CONSTRAINT c UNIQUE)",
            // An alias on a parenthesized joined table (aliased_parenthesized_join off).
            "SELECT * FROM (a CROSS JOIN b) AS x",
            "SELECT * FROM (a JOIN b ON a.c = b.c) x",
            // An alias on a `DELETE … USING` target (delete_using_target_alias off).
            "DELETE FROM t AS e USING u WHERE e.a = 1",
            // A leading `WITH` before `INSERT` (cte_before_insert off).
            "WITH a AS (SELECT 1) INSERT INTO b SELECT * FROM a",
            // A bare special value function as a `FROM` source
            // (special_function_table_source off).
            "SELECT * FROM current_date",
            "SELECT * FROM current_timestamp",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "MySQL should reject the round-6 over-acceptance {sql:?}",
            );
        }

        // The other direction: valid MySQL the same gates must keep accepting — the
        // length-bearing / zone-less types, the parenthesized and literal defaults, the
        // named inline CHECK, the unaliased parenthesized join and derived-table alias, the
        // plain single-table delete alias, and the leading `WITH` before SELECT.
        for sql in [
            "CREATE TABLE t (a VARCHAR(255), b CHAR, c BINARY, d VARBINARY(16))",
            "CREATE TABLE t (a TIMESTAMP, b DATETIME, c DATE, d TIME)",
            "CREATE TABLE t (a INT DEFAULT 5, b INT DEFAULT (UUID()))",
            "CREATE TABLE t (a TIMESTAMP DEFAULT NOW())",
            "CREATE TABLE t (a TIMESTAMP DEFAULT CURRENT_TIMESTAMP)",
            "CREATE TABLE t (a INT CONSTRAINT c CHECK (a > 0))",
            "CREATE TABLE t (a INT REFERENCES b (id))",
            "SELECT * FROM (a CROSS JOIN b)",
            "SELECT * FROM (SELECT 1) AS x",
            "DELETE FROM t AS e WHERE e.a = 1",
            "DELETE FROM t USING u WHERE t.a = u.a",
            "WITH a AS (SELECT 1) SELECT * FROM a",
            "INSERT INTO b WITH a AS (SELECT 1) SELECT * FROM a",
            "SELECT CURRENT_DATE",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_ok(),
                "MySQL should still accept the valid form {sql:?}",
            );
        }
    }

    #[test]
    fn mysql_limit_comma_folds_into_the_canonical_limit_offset_shape() {
        // `LIMIT 5, 10` is the *same* canonical row limit as `LIMIT 10 OFFSET 5`
        // (offset 5, count 10): one `Limit` shape, never a new node (ADR-0011). It
        // carries the `CommaOffset` spelling tag so a source-fidelity render replays the
        // comma form; the counts are literals whose value rides the source span (Meta is
        // equality-neutral), so they compare by value across the two parses.
        let limit_of = |parsed: &Parsed| {
            let Statement::Query { query, .. } = &parsed.statements()[0] else {
                panic!("expected a query statement");
            };
            query.limit.clone().expect("a row-limiting clause")
        };

        let comma = limit_of(
            &parse_with(
                "SELECT a FROM t LIMIT 5, 10",
                crate::ParseConfig::new(MySql),
            )
            .expect("comma form"),
        );
        let explicit = limit_of(
            &parse_with(
                "SELECT a FROM t LIMIT 10 OFFSET 5",
                crate::ParseConfig::new(MySql),
            )
            .expect("offset form"),
        );

        assert_eq!(comma.syntax, LimitSyntax::CommaOffset);
        assert_eq!(explicit.syntax, LimitSyntax::LimitOffset);
        assert_eq!(
            comma.limit, explicit.limit,
            "the count is the second comma argument",
        );
        assert_eq!(
            comma.offset, explicit.offset,
            "the offset is the first comma argument",
        );
    }

    #[test]
    fn mysql_recognizes_its_extended_scalar_type_names() {
        // Each MySQL-only scalar name resolves to its built-in `DataType` variant or
        // spelling tag (data-gated recognition), never the user-defined fallback.
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c TINYINT)"),
            DataType::TinyInt { .. }
        ));
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c MEDIUMINT)"),
            DataType::MediumInt { .. }
        ));
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c MEDIUMTEXT)"),
            DataType::Text {
                spelling: TextTypeName::MediumText,
                ..
            }
        ));
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c LONGBLOB)"),
            DataType::Blob { .. }
        ));
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c DOUBLE)"),
            DataType::Double {
                spelling: DoubleTypeName::Double,
                ..
            }
        ));
        // `DATETIME` reuses the timestamp shape with a zone-less spelling.
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c DATETIME)"),
            DataType::Timestamp {
                spelling: TimestampTypeName::Datetime,
                time_zone: TimeZone::Unspecified,
                ..
            }
        ));
    }

    #[test]
    fn mysql_enum_and_set_carry_their_value_lists() {
        let enum_ty = first_column_type(MySql, "CREATE TABLE t (c ENUM('a', 'b'))");
        let DataType::Enum { values, .. } = &enum_ty else {
            panic!("expected an ENUM type, got {enum_ty:?}");
        };
        assert_eq!(values.len(), 2, "ENUM keeps its two members");

        let set_ty = first_column_type(MySql, "CREATE TABLE t (c SET('x', 'y', 'z'))");
        let DataType::Set { values, .. } = &set_ty else {
            panic!("expected a SET type, got {set_ty:?}");
        };
        assert_eq!(values.len(), 3, "SET keeps its three members");
    }

    #[test]
    fn mysql_charset_annotation_rides_the_string_typed_columns_and_cast_targets() {
        use crate::ast::Charset;

        // The `opt_charset_with_opt_binary` selector kinds on a CHAR column, each shape
        // engine-verified on mysql:8.4 (mysql-char-charset-annotation).
        for (sql, want_charset, want_binary) in [
            (
                "CREATE TABLE t (c CHAR(5) CHARACTER SET utf8mb4)",
                Some(Charset::Named),
                false,
            ),
            // The `CHARSET` synonym folds to the canonical `Named` selector.
            (
                "CREATE TABLE t (c CHAR(5) CHARSET utf8mb4)",
                Some(Charset::Named),
                false,
            ),
            (
                "CREATE TABLE t (c CHAR(5) ASCII)",
                Some(Charset::Ascii),
                false,
            ),
            (
                "CREATE TABLE t (c CHAR(5) UNICODE)",
                Some(Charset::Unicode),
                false,
            ),
            (
                "CREATE TABLE t (c CHAR(5) BYTE)",
                Some(Charset::Byte),
                false,
            ),
            // Bare `BINARY` names no charset.
            ("CREATE TABLE t (c CHAR(5) BINARY)", None, true),
            // `BINARY` composes with a selector in either written order.
            (
                "CREATE TABLE t (c CHAR(5) BINARY ASCII)",
                Some(Charset::Ascii),
                true,
            ),
            (
                "CREATE TABLE t (c CHAR(5) CHARACTER SET utf8mb4 BINARY)",
                Some(Charset::Named),
                true,
            ),
        ] {
            let ty = first_column_type(MySql, sql);
            let DataType::Character {
                charset: Some(annotation),
                ..
            } = &ty
            else {
                panic!("{sql:?}: expected an annotated CHAR, got {ty:?}");
            };
            assert_eq!(annotation.charset, want_charset, "{sql:?}");
            assert_eq!(annotation.binary, want_binary, "{sql:?}");
            assert_eq!(
                annotation.name.is_some(),
                want_charset == Some(Charset::Named),
                "{sql:?}: only the Named selector carries a charset name",
            );
        }

        // The other string-typed carriers the probes proved admit the annotation.
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c TEXT CHARACTER SET utf8mb4)"),
            DataType::Text {
                charset: Some(_),
                ..
            }
        ));
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c TINYTEXT ASCII)"),
            DataType::Text {
                spelling: TextTypeName::TinyText,
                charset: Some(_),
                ..
            }
        ));
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c ENUM('a') CHARACTER SET utf8mb4)"),
            DataType::Enum {
                charset: Some(_),
                ..
            }
        ));
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c SET('a') BYTE)"),
            DataType::Set {
                charset: Some(_),
                ..
            }
        ));

        // Cast position: the annotated CHAR stays an admissible MySQL cast target (the
        // shape predicate ignores the annotation), engine-verified accepts.
        for sql in [
            "SELECT CAST(NULL AS CHAR(5) CHARACTER SET utf8mb4)",
            "SELECT CAST(NULL AS CHAR ASCII)",
            "SELECT CAST(NULL AS CHAR CHARACTER SET utf8mb4 BINARY)",
            // MySQL's charset_name is `ident_or_text`: quoted spellings parse too.
            "SELECT CAST(NULL AS CHAR CHARACTER SET 'utf8mb4')",
            "SELECT CAST(NULL AS CHAR CHARACTER SET `utf8mb4`)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_ok(),
                "MySQL parses {sql:?}"
            );
        }
    }

    #[test]
    fn mysql_charset_annotation_folds_to_the_canonical_render_order() {
        // The reversed `BINARY CHARSET x` spelling and the `CHARSET` synonym fold onto the
        // canonical `CHARACTER SET x BINARY` order (the documented ADR-0011 spelling trade;
        // the written order stays recoverable from the node span).
        assert_eq!(
            mysql_render("CREATE TABLE t (c CHAR(5) BINARY CHARSET utf8mb4)"),
            "CREATE TABLE t (c CHAR(5) CHARACTER SET utf8mb4 BINARY)",
        );
        // A quoted charset name round-trips its quote style.
        assert_eq!(
            mysql_render("SELECT CAST(NULL AS CHAR CHARACTER SET 'utf8mb4')"),
            "SELECT CAST(NULL AS CHAR CHARACTER SET 'utf8mb4')",
        );
    }

    #[test]
    fn mysql_charset_annotation_rejects_the_engine_measured_boundaries() {
        use crate::dialect::Postgres;

        // Each line is `ER_PARSE_ERROR` (1064) on mysql:8.4 — the annotation grammar's
        // measured edges — so the fitted preset must reject it too.
        for sql in [
            // The national forms fix their own charset.
            "CREATE TABLE t (c NCHAR(5) CHARACTER SET utf8mb4)",
            "SELECT CAST(NULL AS NCHAR CHARACTER SET utf8mb4)",
            // The annotation is part of the type: a column attribute may not intervene.
            "CREATE TABLE t (c CHAR(5) NOT NULL CHARACTER SET utf8mb4)",
            // `BYTE` composes with nothing.
            "CREATE TABLE t (c CHAR(5) BYTE BINARY)",
            "SELECT CAST(NULL AS CHAR BINARY BYTE)",
            // At most one selector, one BINARY.
            "SELECT CAST(NULL AS CHAR ASCII UNICODE)",
            "SELECT CAST(NULL AS CHAR CHARACTER SET utf8mb4 ASCII)",
            "SELECT CAST(NULL AS CHAR BINARY BINARY)",
            // `VARCHAR` admits the annotation as a column type but is still not a
            // `cast_type`, annotated or not.
            "SELECT CAST(NULL AS VARCHAR(5) CHARACTER SET utf8mb4)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "MySQL should reject {sql:?}",
            );
        }

        // The gate is dialect data: PostgreSQL leaves `character_set_annotation` off and
        // rejects the annotation (pg_query-verified syntax error), so the same column
        // definition fails there.
        assert!(
            parse_with(
                "CREATE TABLE t (c CHAR(5) CHARACTER SET utf8mb4)",
                crate::ParseConfig::new(Postgres)
            )
            .is_err(),
            "PostgreSQL rejects the MySQL charset annotation",
        );
    }

    #[test]
    fn mysql_numeric_modifiers_wrap_the_inner_numeric_type() {
        // `TINYINT UNSIGNED ZEROFILL` wraps the inner numeric in one modifier node,
        // preserving both written attributes.
        let ty = first_column_type(MySql, "CREATE TABLE t (c TINYINT UNSIGNED ZEROFILL)");
        let DataType::NumericModifier {
            element,
            signedness,
            zerofill,
            ..
        } = &ty
        else {
            panic!("expected a NumericModifier, got {ty:?}");
        };
        assert!(matches!(element.as_deref(), Some(DataType::TinyInt { .. })));
        assert_eq!(*signedness, Signedness::Unsigned);
        assert!(*zerofill, "ZEROFILL is recorded");

        // A bare `ZEROFILL` keeps `Unspecified` signedness (the written sign survives).
        let zerofill_only = first_column_type(MySql, "CREATE TABLE t (c INT ZEROFILL)");
        assert!(matches!(
            zerofill_only,
            DataType::NumericModifier {
                signedness: Signedness::Unspecified,
                zerofill: true,
                ..
            }
        ));
    }

    #[test]
    fn mysql_integer_display_width_rides_the_type_name() {
        // The `(M)` after a built-in integer is a display width stored on the integer
        // variant itself (display metadata, never precision), across the shared
        // spellings and the MySQL-only `TINYINT`/`MEDIUMINT`.
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c INT(11))"),
            DataType::Integer {
                display_width: Some(11),
                ..
            }
        ));
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c TINYINT(1))"),
            DataType::TinyInt {
                display_width: Some(1),
                ..
            }
        ));
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c BIGINT(20))"),
            DataType::BigInt {
                display_width: Some(20),
                ..
            }
        ));

        // The width binds to the inner integer; `UNSIGNED`/`ZEROFILL` wrap the widthed
        // type via the existing numeric-modifier node (extends, not parallels, the
        // modifier modelling).
        let ty = first_column_type(MySql, "CREATE TABLE t (c INT(10) UNSIGNED ZEROFILL)");
        let DataType::NumericModifier {
            element,
            signedness,
            zerofill,
            ..
        } = &ty
        else {
            panic!("expected a NumericModifier, got {ty:?}");
        };
        assert!(matches!(
            element.as_deref(),
            Some(DataType::Integer {
                display_width: Some(10),
                ..
            })
        ));
        assert_eq!(*signedness, Signedness::Unsigned);
        assert!(*zerofill, "ZEROFILL survives alongside the width");

        // A bare integer keeps `None` — the width is genuinely optional.
        assert!(matches!(
            first_column_type(MySql, "CREATE TABLE t (c INT)"),
            DataType::Integer {
                display_width: None,
                ..
            }
        ));
    }

    #[test]
    fn mysql_type_names_round_trip() {
        // parse -> Tier-1 canonical render -> identical source, across the scalar
        // names, the `ENUM`/`SET` value lists, the numeric modifiers, and the
        // standalone `CAST(... AS UNSIGNED)` / `SIGNED` cast targets.
        for sql in [
            "SELECT CAST(a AS UNSIGNED) FROM t",
            "SELECT CAST(a AS SIGNED) FROM t",
            "CREATE TABLE t (c TINYINT UNSIGNED ZEROFILL)",
            "CREATE TABLE t (c INT UNSIGNED)",
            "CREATE TABLE t (c BIGINT ZEROFILL)",
            "CREATE TABLE t (c INT(11))",
            "CREATE TABLE t (c TINYINT(1))",
            "CREATE TABLE t (c BIGINT(20))",
            "CREATE TABLE t (c INT(10) UNSIGNED ZEROFILL)",
            "CREATE TABLE t (c ENUM('a', 'b'))",
            "CREATE TABLE t (c SET('x', 'y'))",
            "CREATE TABLE t (c MEDIUMTEXT)",
            "CREATE TABLE t (c LONGBLOB)",
            "CREATE TABLE t (c DATETIME)",
            "CREATE TABLE t (c DATETIME(6))",
            "CREATE TABLE t (c DOUBLE)",
        ] {
            assert_eq!(mysql_render(sql), sql, "{sql:?} round-trips verbatim");
        }
    }

    #[test]
    fn mysql_comparison_chain_render_round_trips_by_associativity() {
        // Canonical mode emits the minimal parens the binding-power table demands
        // (ADR-0008): under MySQL's now-`Left` comparison row
        // (mysql-comparison-operators-are-left-associative), the left-nested chain
        // from `a < b < c` needs none and reparses to the identical shape; the
        // right-nested shape from explicit source parens (`a < (b < c)`) still
        // needs them regardless of associativity (a left-assoc operator's right
        // side never absorbs an equal-precedence child without parens), so both
        // directions round-trip through the renderer picking up the dialect's
        // binding powers automatically.
        fn project(parsed: &Parsed) -> &Expr<NoExt> {
            let Statement::Query { query, .. } = &parsed.statements()[0] else {
                panic!("expected a query statement");
            };
            let SetExpr::Select { select, .. } = &query.body else {
                panic!("expected a SELECT body");
            };
            let SelectItem::Expr { expr, .. } = &select.projection[0] else {
                panic!("expected a bare projection expression");
            };
            expr
        }

        let chain = "SELECT a < b < c";
        let rendered = mysql_render(chain);
        assert_eq!(
            rendered, chain,
            "the left-nested chain renders without parens"
        );
        let reparsed = parse_with(&rendered, crate::ParseConfig::new(MySql))
            .expect("the bare render reparses");
        let Expr::BinaryOp {
            op: BinaryOperator::Lt,
            left,
            ..
        } = project(&reparsed)
        else {
            panic!("expected the outer `<`");
        };
        assert!(
            matches!(
                **left,
                Expr::BinaryOp {
                    op: BinaryOperator::Lt,
                    ..
                }
            ),
            "reparses to the same left-nested `(a < b) < c` shape",
        );

        let explicit = "SELECT a < (b < c)";
        let rendered = mysql_render(explicit);
        assert_eq!(
            rendered, explicit,
            "the right-nested shape keeps its parens"
        );
        let reparsed = parse_with(&rendered, crate::ParseConfig::new(MySql))
            .expect("the parenthesized render reparses");
        let Expr::BinaryOp {
            op: BinaryOperator::Lt,
            right,
            ..
        } = project(&reparsed)
        else {
            panic!("expected the outer `<`");
        };
        assert!(
            matches!(
                **right,
                Expr::BinaryOp {
                    op: BinaryOperator::Lt,
                    ..
                }
            ),
            "reparses to the same right-nested `a < (b < c)` shape",
        );
    }

    #[test]
    fn mysql_operator_and_literal_gap_families_fold_and_round_trip() {
        use crate::ast::{IsNotDistinctFromSpelling, QuoteStyle, SpecialFunctionKeyword};

        // The sole SELECT item of a single-statement MySQL parse.
        fn item(parsed: &Parsed) -> &SelectItem<NoExt> {
            let Statement::Query { query, .. } = &parsed.statements()[0] else {
                panic!("expected a query statement");
            };
            let SetExpr::Select { select, .. } = &query.body else {
                panic!("expected a SELECT body");
            };
            &select.projection[0]
        }
        fn expr(item: &SelectItem<NoExt>) -> &Expr<NoExt> {
            let SelectItem::Expr { expr, .. } = item else {
                panic!("expected a bare projection expression");
            };
            expr
        }

        // Family 1 — `<=>` folds onto the canonical null-safe operator with the
        // `NullSafeEq` spelling and renders back to `<=>`, never the keyword form (which
        // MySQL rejects), so the spelling tag is load-bearing for a valid re-parse.
        let parsed =
            parse_with("SELECT 1 <=> NULL", crate::ParseConfig::new(MySql)).expect("`<=>` parses");
        assert!(matches!(
            expr(item(&parsed)),
            Expr::BinaryOp {
                op: BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::NullSafeEq),
                ..
            }
        ));
        assert_eq!(mysql_render("SELECT 1 <=> NULL"), "SELECT 1 <=> NULL");

        // Family 2 — same-line adjacent string literals concatenate to one value, and the
        // source spelling round-trips verbatim (the span renders unchanged, ADR-0006).
        let parsed = parse_with("SELECT 'a' 'b'", crate::ParseConfig::new(MySql))
            .expect("adjacent strings parse");
        let Expr::Literal { literal, .. } = expr(item(&parsed)) else {
            panic!("expected a string literal");
        };
        assert_eq!(literal.as_str(parsed.source()).expect("string value"), "ab");
        assert_eq!(mysql_render("SELECT 'a' 'b'"), "SELECT 'a' 'b'");

        // Family 3 — a string-literal alias interns its value and records the source quote,
        // round-tripping to the same quoting (both the single- and double-quoted forms).
        let parsed = parse_with("SELECT 1 AS 'x'", crate::ParseConfig::new(MySql))
            .expect("single-quoted string alias");
        let SelectItem::Expr {
            alias: Some(alias), ..
        } = item(&parsed)
        else {
            panic!("expected an aliased projection");
        };
        assert_eq!(alias.quote, QuoteStyle::Single);
        assert_eq!(mysql_render("SELECT 1 AS 'x'"), "SELECT 1 AS 'x'");
        assert_eq!(mysql_render("SELECT 1 AS \"x\""), "SELECT 1 AS \"x\"");

        // Family 4 — the UTC_* niladic date/time functions parse to the dedicated special
        // value function, and the precision form round-trips.
        let parsed = parse_with("SELECT UTC_DATE", crate::ParseConfig::new(MySql))
            .expect("`UTC_DATE` parses");
        assert!(matches!(
            expr(item(&parsed)),
            Expr::SpecialFunction {
                keyword: SpecialFunctionKeyword::UtcDate,
                ..
            }
        ));
        assert_eq!(
            mysql_render("SELECT UTC_TIMESTAMP(6)"),
            "SELECT UTC_TIMESTAMP(6)"
        );
    }

    #[test]
    fn mysql_transaction_control_matches_server_grammar() {
        // Measured against mysql:8. The block words and mode positions differ by
        // statement head, and START/SET mode lists require comma separators.
        for sql in [
            "START TRANSACTION",
            "START TRANSACTION READ ONLY",
            "START TRANSACTION WITH CONSISTENT SNAPSHOT",
            "START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY",
            "BEGIN",
            "BEGIN WORK",
            "COMMIT WORK AND CHAIN",
            "COMMIT NO RELEASE",
            "ROLLBACK WORK AND CHAIN",
            "ROLLBACK TO s",
            "RELEASE SAVEPOINT s",
            "SET TRANSACTION READ ONLY",
            "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
            "SET TRANSACTION READ ONLY, ISOLATION LEVEL READ COMMITTED",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_ok(),
                "MySQL accepts {sql:?}",
            );
            assert_eq!(mysql_render(sql), sql, "{sql:?} round-trips");
        }

        for sql in [
            "START",
            "START WORK",
            "START TRANSACTION ISOLATION LEVEL READ COMMITTED",
            "START TRANSACTION DEFERRABLE",
            "START TRANSACTION READ ONLY READ WRITE",
            "START TRANSACTION READ ONLY, READ WRITE",
            "BEGIN TRANSACTION",
            "BEGIN READ ONLY",
            "COMMIT TRANSACTION",
            "ROLLBACK TRANSACTION",
            "ABORT",
            "END",
            "RELEASE s",
            "SET TRANSACTION DEFERRABLE",
            "SET TRANSACTION READ ONLY ISOLATION LEVEL READ COMMITTED",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "MySQL rejects {sql:?}",
            );
        }
    }
}
