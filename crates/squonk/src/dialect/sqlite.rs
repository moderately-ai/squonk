// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The SQLite dialect.
//!
//! The whole module is gated by the `sqlite` cargo feature (one `#[cfg]` on its
//! `mod` declaration), so the struct, the `Dialect` impl, and the SQLite test cluster
//! are compiled only when the feature is on — no per-item gates.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;

/// The SQLite dialect ([`FeatureSet::SQLITE`]).
///
/// Reached via [`parse_with`](crate::parse_with), e.g. `parse_with(src, crate::ParseConfig::new(Sqlite))`. It
/// diverges from [`Ansi`](super::Ansi) across the pure-data families the phase-0
/// sweep proved needed — backtick / `[bracket]` identifier quotes, hex integers, the
/// `?`/`:name`/`@name`/`$name` placeholders, the `LIMIT <offset>, <count>` comma
/// form, the JSON `->`/`->>` accessors, and the `ON CONFLICT`/`RETURNING`/`REPLACE
/// INTO` + `IF [NOT] EXISTS`/partial-index surface — plus the small new grammar the
/// sweep surfaced: the `==` equality spelling, general-equality `IS` / `IS NOT`, and
/// the `GLOB` / `MATCH` / `REGEXP` keyword operators.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Sqlite;

impl Dialect for Sqlite {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::SQLITE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        BinaryOperator, DataType, EqualsSpelling, Expr, IsDistinctFromSpelling,
        IsNotDistinctFromSpelling, ParameterKind, ParameterSigil, Statement, TransactionModeKind,
        TransactionStatement, UnaryOperator,
    };
    use crate::dialect::test_support::{assert_full_grammar, first_column_type};
    use crate::parse_with;
    use crate::parser::Parsed;

    /// Tier-1 canonical render of the first statement, parsed under SQLite.
    /// `target: FeatureSet::SQLITE` (not the `RenderConfig` default, which is ANSI) so
    /// binding-power-driven parenthesization reads the same left-associative
    /// comparison row the statement was parsed under.
    fn sqlite_render(sql: &str) -> String {
        use squonk_ast::render::{RenderConfig, RenderCtx, RenderExt, RenderMode};

        let parsed = parse_with(sql, crate::ParseConfig::new(Sqlite))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let config = RenderConfig {
            mode: RenderMode::Canonical,
            target: FeatureSet::SQLITE,
            ..RenderConfig::default()
        };
        let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
        parsed.statements()[0].displayed(&ctx).to_string()
    }

    fn project_expr(parsed: &Parsed) -> &Expr<NoExt> {
        use crate::ast::{SelectItem, SetExpr, Statement};
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

    #[test]
    fn sqlite_parses_the_full_m1_select_grammar() {
        // The representative query uses only constructs SQLite shares with ANSI, so it
        // drives the full shared grammar identically — the structural proof.
        assert_full_grammar(Sqlite);
    }

    #[test]
    fn sqlite_parses_its_distinctive_surface() {
        // Each line is a SQLite-only lexical/grammar choice the preset selects as data
        // (the schema-independent probes from the feature-probe corpus).
        for sql in [
            "SELECT 1 AS `back_ticked`",   // backtick identifier quotes
            "SELECT 1 AS [bracketed]",     // `[bracket]` identifier quotes
            "SELECT 1 == 1",               // `==` equality spelling
            "SELECT 1 IS 1",               // general-equality `IS`
            "SELECT 1 IS NOT 2",           // general-equality `IS NOT`
            "SELECT 'abc' GLOB 'a*'",      // GLOB keyword operator
            "SELECT 'abc' NOT GLOB 'a*'",  // negated GLOB
            "SELECT 'abc' REGEXP 'a'",     // REGEXP keyword operator (grammar-only)
            "SELECT 'abc' MATCH 'a'",      // MATCH keyword operator (grammar-only)
            "SELECT 0xFF + 1",             // hex integer
            "SELECT ?",                    // anonymous placeholder
            "SELECT :name",                // colon-named placeholder
            "SELECT @name",                // at-named placeholder
            "SELECT $value",               // dollar-named placeholder (SQLite)
            "SELECT 1 LIMIT 2, 3",         // `LIMIT <offset>, <count>` comma form
            "SELECT '{\"a\":1}' -> '$.a'", // JSON `->` accessor
            "SELECT 1 IS NULL",            // the plain null test still parses
            "SELECT 1 IS NOT NULL",        // ... and its negation
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_ok(),
                "SQLite parses {sql:?}"
            );
        }
    }

    #[test]
    fn sqlite_accepts_empty_in_list_and_round_trips() {
        // SQLite accepts an empty `IN ()` list (`x IN ()` is false, `x NOT IN ()` true) —
        // engine-measured via rusqlite 3.53.2. The standard requires a non-empty list, so
        // ANSI rejects both. Gated by `PredicateSyntax::empty_in_list`.
        for sql in ["SELECT 1 WHERE 1 IN ()", "SELECT 1 WHERE 1 NOT IN ()"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_ok(),
                "SQLite parses {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(crate::dialect::Ansi)).is_err(),
                "ANSI rejects the empty `IN ()` list {sql:?}",
            );
        }
        assert_eq!(
            sqlite_render("SELECT 1 WHERE 1 IN ()"),
            "SELECT 1 WHERE 1 IN ()"
        );
        assert_eq!(
            sqlite_render("SELECT 1 WHERE 1 NOT IN ()"),
            "SELECT 1 WHERE 1 NOT IN ()",
        );
    }

    #[test]
    fn sqlite_accepts_string_literal_projection_alias_and_round_trips() {
        // SQLite accepts a single-quoted string as an `AS` projection alias — engine-measured
        // via rusqlite 3.53.2, where `'x'` becomes the result-column name. Reuses the
        // MySQL/DuckDB `alias_string_literals` round-trip machinery; ANSI (no such flag) still
        // rejects it. (The bare, `AS`-less form `SELECT 1 'x'` is a separate parser surface
        // SQLite also accepts, left to its own follow-up.)
        let sql = "SELECT 1 AS 'x'";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_ok(),
            "SQLite parses {sql:?}"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(crate::dialect::Ansi)).is_err(),
            "ANSI rejects the string-literal alias {sql:?}",
        );
        assert_eq!(sqlite_render(sql), sql);
    }

    #[test]
    fn sqlite_accepts_indexed_by_clause_and_round_trips() {
        use crate::ast::{IndexedBy, SetExpr, TableFactor};

        // The three indexedby.test gaps: `INDEXED BY <name>` and `NOT INDEXED`, on a bare
        // table and after an explicit alias — engine-measured accepts on rusqlite 3.53.2.
        // Gated by `TableExpressionSyntax::indexed_by`; ANSI (no such flag) rejects each.
        let cases = [
            "SELECT * FROM t INDEXED BY ix",
            "SELECT * FROM t NOT INDEXED",
        ];
        for sql in cases {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_ok(),
                "SQLite parses {sql:?}"
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(crate::dialect::Ansi)).is_err(),
                "ANSI rejects the `INDEXED BY` directive {sql:?}",
            );
            assert_eq!(sqlite_render(sql), sql, "{sql:?} round-trips exactly");
        }

        // The directive trails the alias (`FROM t NOT INDEXED AS e` is an engine reject —
        // the alias must lead), so it renders after it.
        assert_eq!(
            sqlite_render("SELECT * FROM t AS e INDEXED BY ix"),
            "SELECT * FROM t AS e INDEXED BY ix",
        );
        assert_eq!(
            sqlite_render("SELECT * FROM t AS e NOT INDEXED"),
            "SELECT * FROM t AS e NOT INDEXED",
        );

        // The typed field carries the directive, so a planner reads it without string
        // inspection: `INDEXED BY ix` is `Named`, `NOT INDEXED` is `NotIndexed`.
        let parsed = parse_with(
            "SELECT * FROM t INDEXED BY ix",
            crate::ParseConfig::new(Sqlite),
        )
        .unwrap();
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let TableFactor::Table { indexed_by, .. } = &select.from[0].relation else {
            panic!("expected a base-table factor");
        };
        assert!(
            matches!(indexed_by.as_deref(), Some(IndexedBy::Named { .. })),
            "the `INDEXED BY ix` directive is typed as `Named`",
        );
    }

    #[test]
    fn sqlite_indexed_is_a_keyword_only_in_bare_post_table_position() {
        // `INDEXED` is special ONLY as a bare directive head after a table reference. A bare
        // `FROM t indexed` with no trailing `BY` is an engine reject (SQLite commits to the
        // directive on the keyword) — the alias-decline plus the mandatory `BY` reproduce it.
        assert!(
            parse_with("SELECT * FROM t indexed", crate::ParseConfig::new(Sqlite)).is_err(),
            "bare `FROM t indexed` (no `BY`) is a parse error, matching SQLite",
        );
        // `NOT INDEXED AS e` is a reject — the directive must trail the alias, not lead it.
        assert!(
            parse_with(
                "SELECT * FROM t NOT INDEXED AS e",
                crate::ParseConfig::new(Sqlite)
            )
            .is_err(),
            "`NOT INDEXED` before the alias is rejected",
        );
        // Everywhere else `indexed` stays a plain identifier (it is in no reserved set):
        // an explicit `AS` alias, a column reference, and a column name all parse.
        for sql in [
            "SELECT * FROM t AS indexed",
            "SELECT indexed FROM t",
            "CREATE TABLE t (indexed INT)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_ok(),
                "`indexed` is a plain identifier in {sql:?}",
            );
        }
    }

    #[test]
    fn sqlite_typed_type_names_still_win_under_the_liberal_gate() {
        // The FALLBACK ordering: with `liberal_type_names` on, a bare built-in or a
        // single-word user-defined affinity name keeps its existing shape — the liberal
        // variant is reached only when a trailing word or a two-argument paren list exceeds
        // what the typed / user-defined parse can hold.
        assert!(matches!(
            first_column_type(Sqlite, "CREATE TABLE t (c INT)"),
            DataType::Integer { .. }
        ));
        assert!(matches!(
            first_column_type(Sqlite, "CREATE TABLE t (c INTEGER)"),
            DataType::Integer { .. }
        ));
        assert!(matches!(
            first_column_type(Sqlite, "CREATE TABLE t (c DOUBLE PRECISION)"),
            DataType::Double { .. }
        ));
        assert!(matches!(
            first_column_type(Sqlite, "CREATE TABLE t (c VARCHAR(255))"),
            DataType::Character { .. }
        ));
        // A typed two-word national-char name with a single arg stays typed (a constraint
        // keyword terminates the run, so no liberal reparse fires).
        assert!(matches!(
            first_column_type(Sqlite, "CREATE TABLE t (c NATIONAL CHARACTER(15) NOT NULL)"),
            DataType::Character { .. }
        ));
        // A single-word affinity name still resolves to `UserDefined`, not `Liberal`.
        assert!(matches!(
            first_column_type(Sqlite, "CREATE TABLE t (c BANANA)"),
            DataType::UserDefined { .. }
        ));
    }

    #[test]
    fn sqlite_accepts_liberal_type_names_and_round_trips() {
        // SQLite's `typename` is a free `ids ...` token run: an arbitrary multi-word affinity
        // name (with an optional two-argument modifier) terminated by a column-constraint
        // keyword / comma / close paren — engine-measured on rusqlite/sqlite3 3.53.2 & 3.43.2.
        // ANSI (closed type vocabulary) rejects each; gated by `TypeNameSyntax::liberal_type_names`.
        // The two corpus statements' distinctive types plus the exact-boundary probes. The
        // liberal shape and arg count are read off the `DataType`; the word count and text are
        // asserted via the token-exact render (the word symbols are per-parse, so comparing
        // through the render is cleaner than reaching into a dropped parse's interner).
        for (sql, word_count, args) in [
            ("CREATE TABLE t (c LONG INTEGER)", 2usize, &[][..]),
            ("CREATE TABLE t (c UNSIGNED BIG INT)", 3, &[][..]),
            ("CREATE TABLE t (c VARCHAR(123,456))", 1, &[123u32, 456][..]),
            (
                "CREATE TABLE t (c FLOATING POINT(5,10))",
                2,
                &[5u32, 10][..],
            ),
            ("CREATE TABLE t (c INTEGEB PRIMARI KEY)", 3, &[][..]),
        ] {
            let ty = first_column_type(Sqlite, sql);
            let DataType::Liberal {
                words: got_words,
                args: got_args,
                ..
            } = &ty
            else {
                panic!("{sql:?}: expected DataType::Liberal, got {ty:?}");
            };
            assert_eq!(got_words.len(), word_count, "{sql:?} word count");
            assert_eq!(got_args.as_slice(), args, "{sql:?} args");
            assert!(
                parse_with(sql, crate::ParseConfig::new(crate::dialect::Ansi)).is_err(),
                "ANSI rejects the liberal type name {sql:?}",
            );
        }

        // Token-exact round-trip (the render re-spaces the modifier list, which the
        // token-level fidelity harness treats as identical).
        assert_eq!(
            sqlite_render("CREATE TABLE t (c LONG INTEGER)"),
            "CREATE TABLE t (c LONG INTEGER)",
        );
        assert_eq!(
            sqlite_render("CREATE TABLE t (c INTEGEB PRIMARI KEY)"),
            "CREATE TABLE t (c INTEGEB PRIMARI KEY)",
        );

        // Boundary: a bare `PRIMARY` (not `PRIMARY KEY`) terminates the run and is a reject;
        // a three-argument paren list on a multi-word liberal name is a reject (SQLite's
        // `typetoken` caps the modifier list at two). (`FOO(1,2,3)` on a lone word rides the
        // pre-existing user-defined-type modifier list, a separate, wider surface.)
        assert!(
            parse_with(
                "CREATE TABLE t (c MY PRIMARY)",
                crate::ParseConfig::new(Sqlite)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "CREATE TABLE t (c FLOATING POINT(1,2,3))",
                crate::ParseConfig::new(Sqlite)
            )
            .is_err()
        );
        // A `GENERATED ALWAYS AS` generated column is unaffected — `GENERATED` terminates the
        // type run rather than being absorbed as a liberal word.
        assert!(
            parse_with(
                "CREATE TABLE t (a INT, c INT GENERATED ALWAYS AS (a) STORED)",
                crate::ParseConfig::new(Sqlite)
            )
            .is_ok(),
        );
    }

    #[test]
    fn sqlite_accepts_begin_transaction_mode_and_round_trips() {
        // SQLite's `BEGIN {DEFERRED|IMMEDIATE|EXCLUSIVE} [TRANSACTION]` transaction-mode
        // modifier (`sqlite-begin-transaction-modifiers`) — engine-measured via rusqlite
        // 3.53.2: all three accept, with and without the trailing `TRANSACTION`, and
        // doubling the modifier rejects (`BEGIN DEFERRED IMMEDIATE`). `pg_query` rejects
        // all three (PostgreSQL's `BEGIN` takes its own, differently-shaped
        // `ISOLATION LEVEL …`/`READ ONLY|WRITE`/`[NOT] DEFERRABLE` modifier vocabulary,
        // deliberately not modelled here), so ANSI/PostgreSQL keep rejecting the modifier
        // keyword under the new gate too.
        for (sql, expected) in [
            ("BEGIN DEFERRED", TransactionModeKind::Deferred),
            ("BEGIN IMMEDIATE", TransactionModeKind::Immediate),
            ("BEGIN EXCLUSIVE", TransactionModeKind::Exclusive),
            ("BEGIN DEFERRED TRANSACTION", TransactionModeKind::Deferred),
            (
                "BEGIN IMMEDIATE TRANSACTION",
                TransactionModeKind::Immediate,
            ),
            (
                "BEGIN EXCLUSIVE TRANSACTION",
                TransactionModeKind::Exclusive,
            ),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Sqlite))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let [Statement::Transaction { transaction, .. }] = parsed.statements() else {
                panic!("{sql:?} did not parse to one transaction statement");
            };
            let TransactionStatement::Begin { mode, .. } = &**transaction else {
                panic!("{sql:?} should be a Begin statement");
            };
            assert_eq!(*mode, Some(expected), "{sql:?}");
        }

        // The default source-fidelity render replays the optional `TRANSACTION` block
        // word (via the `TransactionBlockKeyword` tag) alongside the mode word, and drops
        // it when the source did (spelling-tags-keyword-operator-batch).
        assert_eq!(sqlite_render("BEGIN DEFERRED"), "BEGIN DEFERRED");
        assert_eq!(
            sqlite_render("BEGIN IMMEDIATE TRANSACTION"),
            "BEGIN IMMEDIATE TRANSACTION"
        );
        assert_eq!(sqlite_render("BEGIN EXCLUSIVE"), "BEGIN EXCLUSIVE");

        // Only one modifier word is admitted.
        assert!(
            parse_with("BEGIN DEFERRED IMMEDIATE", crate::ParseConfig::new(Sqlite)).is_err(),
            "doubling the transaction-mode modifier must reject",
        );

        // Flag off: the modifier keyword is not recognized, so it falls through to the
        // existing trailing-token error under ANSI and PostgreSQL.
        for sql in ["BEGIN DEFERRED", "BEGIN IMMEDIATE", "BEGIN EXCLUSIVE"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(crate::dialect::Ansi)).is_err(),
                "ANSI rejects the SQLite transaction-mode modifier {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres)).is_err(),
                "PostgreSQL rejects the SQLite transaction-mode modifier {sql:?}",
            );
        }
    }

    #[test]
    fn sqlite_transaction_control_matches_its_statement_vocabulary() {
        for sql in [
            "BEGIN",
            "BEGIN TRANSACTION",
            "BEGIN TRANSACTION tx",
            "COMMIT",
            "COMMIT TRANSACTION",
            "COMMIT TRANSACTION tx",
            "END",
            "END TRANSACTION",
            "END TRANSACTION tx",
            "ROLLBACK",
            "ROLLBACK TRANSACTION",
            "ROLLBACK TRANSACTION tx",
            "SAVEPOINT s",
            "RELEASE s",
            "RELEASE SAVEPOINT s",
            "ROLLBACK TO s",
            "ROLLBACK TO SAVEPOINT s",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_ok(),
                "SQLite parses {sql:?}",
            );
        }

        for sql in [
            "START TRANSACTION",
            "BEGIN WORK",
            "COMMIT WORK",
            "ROLLBACK WORK",
            "SET TRANSACTION READ ONLY",
            "BEGIN READ ONLY",
            "ABORT",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
                "SQLite rejects {sql:?}",
            );
        }

        assert_eq!(sqlite_render("END TRANSACTION"), "END TRANSACTION");
        assert_eq!(sqlite_render("EnD\ntrAnsaction--\nE"), "END TRANSACTION E");
        assert!(
            parse_with("END TRANSACTION E F", crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite admits at most one transaction name",
        );
    }

    #[test]
    fn sqlite_equality_spellings_round_trip_exactly() {
        // `==` and `=` fold onto the one canonical equality operator with an
        // `EqualsSpelling` tag (ADR-0011), so each spelling round-trips verbatim.
        assert_eq!(sqlite_render("SELECT 1 == 1"), "SELECT 1 == 1");
        assert_eq!(sqlite_render("SELECT 1 = 1"), "SELECT 1 = 1");
        // The GLOB operator and the `$name` placeholder round-trip their exact spelling.
        assert_eq!(
            sqlite_render("SELECT 'abc' GLOB 'a*'"),
            "SELECT 'abc' GLOB 'a*'"
        );
        assert_eq!(sqlite_render("SELECT $value"), "SELECT $value");
    }

    #[test]
    fn sqlite_pragma_attach_and_detach_round_trip_exactly() {
        // The phase-0 evidence set, round-trip-exact: PRAGMA's bare / `= <value>` /
        // `(<value>)` forms (the `parenthesized` tag and the reused SET value shape
        // preserve keyword, signed-number — sign folded into the literal — string,
        // and hex spellings verbatim), schema-qualified names, ATTACH with and
        // without the `DATABASE` keyword, and DETACH — which the accept/reject
        // oracle cannot gate (a bare `prepare` needs a prior ATTACH), so round-trip
        // is its guard, per the ticket.
        for sql in [
            "PRAGMA user_version",
            "PRAGMA optimize",
            "PRAGMA foreign_keys = ON",
            "PRAGMA journal_mode = WAL",
            "PRAGMA synchronous = FULL",
            "PRAGMA synchronous = 2",
            "PRAGMA cache_size = -2000",
            "PRAGMA memory_limit = '1GB'",
            "PRAGMA table_info(sqlite_master)",
            "PRAGMA QUICK_CHECK(0)",
            "PRAGMA QUICK_CHECK('sqlite_master')",
            "PRAGMA cache_size(-500)",
            "PRAGMA optimize(0x10002)",
            "PRAGMA schema.quick_check",
            "PRAGMA schema.synchronous = FULL",
            "ATTACH DATABASE ':memory:' AS aux",
            "ATTACH ':memory:' AS aux2",
            "DETACH DATABASE aux",
            "DETACH aux",
        ] {
            assert_eq!(sqlite_render(sql), sql, "round-trip-exact for {sql:?}");
        }
    }

    #[test]
    fn sqlite_maintenance_and_trigger_statements_round_trip_exactly() {
        // The `sqlite-utility-and-trigger-statements` evidence set, round-trip-exact
        // under the fitted preset (each form engine-verified against bundled SQLite):
        // the three maintenance statements with their optional (single, non-dotted for
        // VACUUM) name and the `VACUUM INTO <expr>` target, and the trigger envelope's
        // timing/event/`FOR EACH ROW`/`WHEN` matrix over a reused-statement body.
        for sql in [
            "VACUUM",
            "VACUUM main",
            "VACUUM INTO 'backup.db'",
            "VACUUM main INTO 'a' || '.db'",
            "REINDEX",
            "REINDEX nocase",
            "REINDEX main.t",
            "ANALYZE",
            "ANALYZE main",
            "ANALYZE main.t",
            "CREATE TRIGGER trg AFTER INSERT ON t BEGIN UPDATE t SET c = c + 1; END",
            "CREATE TEMP TRIGGER trg BEFORE UPDATE OF a, b ON t BEGIN SELECT 1; END",
            "CREATE TRIGGER IF NOT EXISTS trg INSTEAD OF DELETE ON v FOR EACH ROW WHEN old.a > 0 BEGIN INSERT INTO log VALUES (1); DELETE FROM t WHERE a = old.a; END",
        ] {
            assert_eq!(sqlite_render(sql), sql, "round-trip-exact for {sql:?}");
        }
    }

    #[test]
    fn sqlite_double_equals_folds_onto_canonical_equality_with_its_spelling() {
        // A consumer matching `BinaryOperator::Eq(_)` sees both spellings — the fold
        // is what keeps rewriters/analyzers from silently missing `==` comparisons.
        let parsed =
            parse_with("SELECT 1 == 1", crate::ParseConfig::new(Sqlite)).expect("`==` parses");
        assert!(matches!(
            project_expr(&parsed),
            Expr::BinaryOp {
                op: BinaryOperator::Eq(EqualsSpelling::Double),
                ..
            }
        ));
        let single =
            parse_with("SELECT 1 = 1", crate::ParseConfig::new(Sqlite)).expect("`=` parses");
        assert!(matches!(
            project_expr(&single),
            Expr::BinaryOp {
                op: BinaryOperator::Eq(EqualsSpelling::Single),
                ..
            }
        ));
    }

    #[test]
    fn sqlite_general_is_folds_onto_the_null_safe_operators() {
        // `1 IS 1` is null-safe equality — `IS NOT DISTINCT FROM` — and `1 IS NOT 2`
        // is `IS DISTINCT FROM`. Both fold onto the existing operators (no new node),
        // tagged with the SQLite bare-`IS` spelling so they render back as `IS`/`IS NOT`.
        let is = parse_with("SELECT 1 IS 1", crate::ParseConfig::new(Sqlite)).expect("`IS` parses");
        assert!(matches!(
            project_expr(&is),
            Expr::BinaryOp {
                op: BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::Is),
                ..
            }
        ));
        let is_not = parse_with("SELECT 1 IS NOT 2", crate::ParseConfig::new(Sqlite))
            .expect("`IS NOT` parses");
        assert!(matches!(
            project_expr(&is_not),
            Expr::BinaryOp {
                op: BinaryOperator::IsDistinctFrom(IsDistinctFromSpelling::Is),
                ..
            }
        ));
        // The plain null test keeps its dedicated shape, unaffected by general equality.
        let is_null = parse_with("SELECT 1 IS NULL", crate::ParseConfig::new(Sqlite))
            .expect("`IS NULL` parses");
        assert!(matches!(project_expr(&is_null), Expr::IsNull { .. }));
    }

    #[test]
    fn sqlite_bare_is_round_trips_as_bare_is() {
        // The bare `IS`/`IS NOT` spelling round-trips rather than re-spelling as the
        // explicit `IS [NOT] DISTINCT FROM` — the `IsNotDistinctFromSpelling::Is` /
        // `IsDistinctFromSpelling::Is` tags carry the surface form
        // (sqlite-spelling-fidelity-parser-fixes). The explicit keyword forms, also valid
        // under SQLite, keep their own spelling.
        assert_eq!(sqlite_render("SELECT 1 IS 2"), "SELECT 1 IS 2");
        assert_eq!(sqlite_render("SELECT 1 IS NOT 2"), "SELECT 1 IS NOT 2");
        assert_eq!(
            sqlite_render("SELECT 1 IS DISTINCT FROM 2"),
            "SELECT 1 IS DISTINCT FROM 2"
        );
        assert_eq!(
            sqlite_render("SELECT 1 IS NOT DISTINCT FROM 2"),
            "SELECT 1 IS NOT DISTINCT FROM 2"
        );
    }

    #[test]
    fn sqlite_not_glob_folds_to_the_negation_of_glob() {
        // `a NOT GLOB b` has no negated surface of its own, so it is `NOT (a GLOB b)`.
        let parsed = parse_with(
            "SELECT 'abc' NOT GLOB 'a*'",
            crate::ParseConfig::new(Sqlite),
        )
        .expect("`NOT GLOB` parses");
        let Expr::UnaryOp {
            op: UnaryOperator::Not,
            expr,
            ..
        } = project_expr(&parsed)
        else {
            panic!("expected `NOT (...)`");
        };
        assert!(matches!(
            **expr,
            Expr::BinaryOp {
                op: BinaryOperator::Glob,
                ..
            }
        ));
    }

    #[test]
    fn sqlite_dollar_name_parameter_carries_the_dollar_sigil() {
        let parsed =
            parse_with("SELECT $value", crate::ParseConfig::new(Sqlite)).expect("`$name` parses");
        assert!(matches!(
            project_expr(&parsed),
            Expr::Parameter {
                kind: ParameterKind::Named {
                    sigil: ParameterSigil::Dollar,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn sqlite_recognizes_a_declared_column_type() {
        // SQLite's flexible typing still records a written type name; the column
        // grammar is shared with `CAST`, so this is the lens onto type recognition.
        assert!(matches!(
            first_column_type(Sqlite, "CREATE TABLE t (c INTEGER)"),
            DataType::Integer { .. }
        ));
    }

    #[test]
    fn sqlite_absorbs_integer_display_width_but_ansi_rejects_it() {
        use crate::dialect::Ansi;

        // SQLite accepts a display width on a built-in integer (`INT(11)`) — the
        // affinity absorption the ticket models — storing it on the integer variant and
        // round-tripping byte-exact. Engine-measured on bundled rusqlite: `INT(11)` /
        // `BIGINT(20)` prepare.
        assert!(matches!(
            first_column_type(Sqlite, "CREATE TABLE t (c INT(11))"),
            DataType::Integer {
                display_width: Some(11),
                ..
            }
        ));
        for sql in [
            "CREATE TABLE t (c INT(11))",
            "CREATE TABLE t (c BIGINT(20))",
        ] {
            assert_eq!(sqlite_render(sql), sql, "{sql:?} round-trips verbatim");
        }

        // The gate is dialect data, not a universal accept: ANSI (display width off)
        // rejects the parenthesized form on a built-in integer, matching `pg_query`'s
        // reject for `INT(11)`.
        for sql in [
            "CREATE TABLE t (c INT(11))",
            "CREATE TABLE t (c BIGINT(20))",
            "CREATE TABLE t (c SMALLINT(5))",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_ok(),
                "SQLite absorbs the display width {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
                "ANSI rejects the display width on a built-in integer {sql:?}",
            );
        }

        // The width is a prefix arg; SQLite's `numeric_modifiers` stays off, so a
        // trailing `UNSIGNED` after the parens is rejected — matching rusqlite, whose
        // type grammar forbids a keyword once the `(M)` closes.
        assert!(
            parse_with(
                "CREATE TABLE t (c INT(11) UNSIGNED)",
                crate::ParseConfig::new(Sqlite)
            )
            .is_err(),
            "SQLite rejects a keyword modifier after the display-width parens",
        );
    }

    #[test]
    fn sqlite_frees_keywords_ansi_reserves_as_identifiers() {
        // SQLite's small reserved set lets `END`/`DESC`/`ASC` serve as bare column
        // names, where the ANSI/PostgreSQL model rejects them.
        for sql in [
            "CREATE TABLE z (end INT)",
            "SELECT desc FROM (SELECT 1 AS desc) AS s",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_ok(),
                "SQLite accepts {sql:?}"
            );
        }
    }

    #[test]
    fn sqlite_rejects_the_multi_dialect_syntax_the_gate_baselined() {
        // The fitted preset sheds the multi-dialect over-acceptances the at-scale gate
        // (`conformance::corpus_sqlite_verdicts`) surfaced — each is engine-measured-
        // rejected by rusqlite. Proven as dialect data: `Ansi` (which sets each gate on)
        // accepts every one, so the tightening is the SQLite preset's, not a universal
        // reject. One representative per closed family.
        use crate::dialect::Ansi;
        for sql in [
            // NB: `SELECT date '1998-12-01'` is NOT here — SQLite reads it as the column `date`
            // aliased by the bare string `'1998-12-01'` (`bare_alias_string_literals`), a
            // resolution-only reject that reads as a parse *accept*, matching rusqlite. The
            // interval form keeps a genuine syntax reject (its trailing `day` cannot be a second
            // alias): `SELECT interval '90' day` is `near "day": syntax error` on rusqlite.
            "SELECT interval '90' day", // interval literal
            "SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY x)", // ordered-set aggregate
            "SELECT a FROM t WHERE a = ANY (SELECT 1)", // quantified comparison
            "SELECT * FROM x AS y(a, b)", // table-alias column list
            "SELECT extract(year FROM d) FROM t", // EXTRACT(f FROM src)
            "SELECT 1 OFFSET 3",        // bare leading OFFSET
            "SET x = 1",                // session SET
            "GRANT SELECT ON tbl TO usr", // GRANT
            "REVOKE SELECT ON tbl FROM usr", // REVOKE
            "DELETE FROM t USING s WHERE t.a = s.a", // DELETE ... USING
            "CREATE SCHEMA s",          // CREATE SCHEMA
            "CREATE DATABASE d",        // CREATE DATABASE
            "CREATE MATERIALIZED VIEW v AS SELECT 1", // MATERIALIZED VIEW
            "CREATE FUNCTION f() LANGUAGE sql", // CREATE FUNCTION
            "CREATE OR REPLACE VIEW v AS SELECT 1", // OR REPLACE
            "DROP FUNCTION f",          // DROP FUNCTION
            "DROP MATERIALIZED VIEW v", // DROP MATERIALIZED VIEW
            "CREATE TABLE t (x INT GENERATED ALWAYS AS IDENTITY)", // IDENTITY column
            "CREATE TABLE t (a INT) WITH (fillfactor = 70)", // storage parameters
            "CREATE TABLE t (a INT) ON COMMIT PRESERVE ROWS", // ON COMMIT action
            "ALTER TABLE t ALTER COLUMN a SET DEFAULT 1", // ALTER COLUMN
            "ALTER TABLE t ADD CONSTRAINT pk PRIMARY KEY (a)", // ADD PRIMARY KEY
            "ALTER TABLE t ADD FOREIGN KEY (a) REFERENCES u", // ADD FOREIGN KEY
            "ALTER TABLE t ADD COLUMN a INT, ADD COLUMN b INT", // multiple ALTER actions
            // The position-aware query-structure + name-grammar residual, closed by
            // `sqlite-preset-over-acceptance-query-and-name-grammar` (one representative
            // per family; the position-aware boundary is exercised in
            // `parser::query`/`parser::from`).
            "(SELECT 1) UNION (SELECT 2)", // parenthesized compound operand
            "SELECT * FROM a.b.c",         // three-part relation name
            "SELECT 1 AS delete",          // reserved word as a ColLabel
            "SELECT 1 FROM a JOIN b JOIN c ON b.id = c.id ON a.id = b.id", // stacked join qualifiers
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
                "SQLite must reject the multi-dialect form {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(Ansi)).is_ok(),
                "the override dialect (Ansi) still accepts {sql:?}",
            );
        }
        // The `IF [NOT] EXISTS` guard on `ALTER`'s column actions is gated by
        // `index_alter_syntax.alter_table_extended` *and* `existence_guards.if_exists`;
        // ANSI leaves `if_exists` off, so use the SQLite-only reject direction here (the
        // accept side is the sweep's job).
        for sql in [
            "ALTER TABLE t DROP COLUMN IF EXISTS a",
            "ALTER TABLE IF EXISTS t ADD COLUMN k INT",
            "ALTER TABLE t ADD COLUMN IF NOT EXISTS k INT",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
                "SQLite must reject the ALTER existence guard {sql:?}",
            );
        }
    }

    #[test]
    fn sqlite_keeps_its_lenient_alter_and_computed_columns() {
        // The tightening is surgical: SQLite's own lenient `ALTER TABLE` surface (a
        // `CHECK` table constraint via `ADD`, `DROP CONSTRAINT`) and its computed columns
        // (`GENERATED ALWAYS AS (<expr>)`, unlike the rejected `AS IDENTITY`) stay
        // accepted — each engine-measured-accepted by rusqlite.
        for sql in [
            "ALTER TABLE t ADD CHECK (a < 20)",
            "ALTER TABLE t ADD CONSTRAINT c CHECK (a < 20)",
            "ALTER TABLE t DROP CONSTRAINT c",
            "ALTER TABLE t ADD COLUMN k INT",
            "ALTER TABLE t DROP COLUMN a",
            "ALTER TABLE t RENAME TO t2",
            "CREATE TABLE t (x INT, y INT GENERATED ALWAYS AS (x + 1))",
            "CREATE TABLE t (x INT, y INT AS (x + 1) STORED)",
            "SELECT 1 LIMIT 2 OFFSET 3",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Sqlite)).is_ok(),
                "SQLite keeps {sql:?}"
            );
        }
    }
}
