// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! PostgreSQL differential test suite for the M1 parser surface.
//!
//! The accept/reject + structural parity corpora and their oracles, split out of
//! `pg.rs` (the module root) under the file+dir idiom. Reaches the oracle, the parity
//! helpers, the divergence allowlist, the re-exported [`pg_shape`](super::pg_shape), and
//! the PG-specific regress-guide fixture parser via `use super::*`.

use super::*;
// AST-node types the parse-tree assertions match on. They used to reach the tests
// via `super::*` from the module-level `squonk_ast` import, which moved to
// `crate::shape` with the mapper; the tests keep their own narrow set.
use squonk_ast::{JoinConstraint, JoinOperator, SetExpr, Statement, TableFactor};

/// The neutral shape of `sql`'s first statement as our parser maps it — the
/// single-statement projection the structural-oracle tests compare.
fn first_ours_shape(sql: &str) -> StatementShape {
    squonk_shape(&parse_with(sql, Postgres).expect("squonk parses"))
        .into_iter()
        .next()
        .expect("one mapped statement")
}

/// The neutral shape of `sql`'s first statement as PostgreSQL maps it.
fn first_pg_shape(sql: &str) -> StatementShape {
    pg_shape(&pg_query::parse(sql).expect("pg_query parses").protobuf)
        .expect("mapped PostgreSQL shape")
        .into_iter()
        .next()
        .expect("one mapped statement")
}

const ACCEPT_CORPUS: &[&str] = &[
    "SELECT 1",
    "SELECT TRUE, FALSE, NULL",
    "SELECT a, b, *",
    "SELECT a AS x FROM t",
    "SELECT * FROM t1 JOIN t2 ON t1.id = t2.id",
    "SELECT a FROM t WHERE a > 1 GROUP BY a HAVING a < 9 ORDER BY a LIMIT 10 OFFSET 5",
    "SELECT 1 UNION ALL SELECT 2",
    "SELECT 1 INTERSECT SELECT 1",
    "SELECT 1 EXCEPT SELECT 2",
    "SELECT 1 UNION SELECT 2 INTERSECT SELECT 3",
    "SELECT 1 INTERSECT SELECT 2 UNION SELECT 3",
    "SELECT 1 UNION (SELECT 2 UNION ALL SELECT 2) ORDER BY 1",
    "(SELECT 1 UNION SELECT 2) EXCEPT SELECT 3",
    "(SELECT 1 EXCEPT SELECT 2) INTERSECT SELECT 3",
    "VALUES (1, 2), (3, 4)",
    // `DEFAULT` as a VALUES row element (prod-sql-values-default): PostgreSQL
    // parses it to `SetToDefault`, accepted standalone and in mixed rows.
    "VALUES (DEFAULT)",
    "VALUES (1, DEFAULT), (DEFAULT, 2)",
    "WITH x AS (VALUES (1, DEFAULT)) SELECT * FROM x",
    "WITH x AS (SELECT 1) SELECT * FROM x",
    "WITH RECURSIVE x(a) AS NOT MATERIALIZED (VALUES (1)) SELECT a FROM x",
    "SELECT CAST(a AS int), CAST(b AS varchar(5))",
    "SELECT CAST(a AS public.geometry(4326))",
    "SELECT CAST(a AS timestamp(3) with time zone)",
    "SELECT (a < b) < c",
    "SELECT a < (b < c)",
    "SELECT 'it''s'",
    "SELECT E'line\\nquote\\''",
    "SELECT e'\\141\\x62\\u0063\\U00000064'",
    "SELECT $$a\\n'b$$",
    "SELECT $tag$a$inner$b$tag$",
];

/// The `TABLE name` command and the empty SELECT target list
/// (parse-pg-table-command-and-empty-select). PostgreSQL lowers `TABLE t` to the
/// same star-projection `SelectStmt` as `SELECT * FROM t`, and an empty projection
/// maps to an empty target list on both sides, so every entry holds *structural*
/// parity (not just accept/reject) — the canonicalize-into-`Select` design is what
/// makes the neutral shapes coincide with no mapping change.
const PG_TABLE_COMMAND_CORPUS: &[&str] = &[
    "TABLE t",
    "TABLE s.t",
    // The PostgreSQL `ONLY`/`*` inheritance markers ride the FROM-relation gate;
    // the neutral shape records `ONLY` suppression identically to pg's `!inh`.
    "TABLE ONLY t",
    "TABLE ONLY (t)",
    "TABLE t *",
    // Set operations compose (each operand is a star-projection Select), and the
    // `TABLE` form mixes with `SELECT`/`VALUES` operands.
    "TABLE t UNION TABLE b",
    "TABLE a UNION ALL SELECT 1",
    "TABLE t INTERSECT TABLE b",
    // `ORDER BY`/`LIMIT` bind the whole query, outside the `TABLE` primary.
    "TABLE t ORDER BY 1 LIMIT 2",
    // `TABLE` as a query primary: a CTE body, a derived table, an `IN` subquery.
    "WITH c AS (SELECT 1) TABLE c",
    "SELECT * FROM (TABLE t) x",
    "SELECT * FROM t WHERE a IN (TABLE b)",
];

/// The empty SELECT target list before each clause that may follow it, plus the
/// explicit-`ALL` head (which, like a bare `SELECT`, admits an empty list).
const PG_EMPTY_SELECT_CORPUS: &[&str] = &[
    "SELECT",
    "SELECT FROM t",
    "SELECT WHERE a = 1",
    "SELECT GROUP BY a",
    "SELECT HAVING a",
    "SELECT ORDER BY 1",
    "SELECT LIMIT 1",
    "SELECT ALL",
    "SELECT ALL FROM t",
];

/// The reject boundary both parsers enforce: `TABLE` takes only a bare
/// `relation_expr` (no alias, no `WHERE`, no subquery, ≤3 name parts), and a
/// `DISTINCT`/`DISTINCT ON` head requires a non-empty target list.
const PG_TABLE_EMPTY_SELECT_REJECT_CORPUS: &[&str] = &[
    "TABLE",
    "TABLE t x",
    "TABLE t AS x",
    "TABLE t WHERE a = 1",
    "TABLE (SELECT 1)",
    "TABLE a.b.c.d",
    "SELECT DISTINCT",
    "SELECT DISTINCT FROM t",
    "SELECT DISTINCT ON (a) FROM t",
];

#[test]
fn pg_parity_for_table_command_and_empty_select() {
    for sql in PG_TABLE_COMMAND_CORPUS.iter().chain(PG_EMPTY_SELECT_CORPUS) {
        assert_accept_reject_parity(sql);
        assert_structural_parity(sql);
    }
    for sql in PG_TABLE_EMPTY_SELECT_REJECT_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

/// `TABLE t` and `SELECT * FROM t` are the same star-projection `SelectStmt` in
/// PostgreSQL; the spelling tag is the only thing distinguishing them, so their
/// neutral shapes must coincide (the parity property the canonicalization buys).
#[test]
fn table_command_maps_like_explicit_select_star() {
    assert_eq!(
        first_ours_shape("TABLE t"),
        first_ours_shape("SELECT * FROM t")
    );
    assert_eq!(first_pg_shape("TABLE t"), first_pg_shape("SELECT * FROM t"));
    assert_eq!(
        first_ours_shape("TABLE t"),
        first_pg_shape("SELECT * FROM t")
    );
}

const SUBQUERY_PREDICATE_CORPUS: &[&str] = &[
    "SELECT (SELECT 1)",
    "SELECT * FROM t WHERE EXISTS (SELECT 1)",
    "SELECT * FROM t WHERE a IN (SELECT b FROM u)",
    "SELECT * FROM t WHERE a NOT IN (SELECT b FROM u)",
    "SELECT * FROM t WHERE a = ANY (SELECT b FROM u)",
    "SELECT * FROM t WHERE a < ALL (SELECT b FROM u)",
];

/// Expression constructs mapped by this ticket: plain function calls, NULL
/// tests, CASE (searched and simple), and signed numeric literals. Drives both
/// accept/reject and structural parity, so every entry must map on both sides.
const EXPRESSION_ACCEPT_CORPUS: &[&str] = &[
    // Function calls — plain `FuncCall` nodes (user functions and bare
    // aggregates); the SQL-syntax functions (COALESCE/NULLIF/GREATEST/LEAST)
    // are a documented gap, see `pg_special_form_function_mapping_gaps`.
    "SELECT lower(a)",
    "SELECT f(a, b)",
    "SELECT count(*)",
    "SELECT sum(a) FROM t",
    "SELECT abs(a) FROM t WHERE f(a) > g(b, c)",
    // NULL tests, alone and combined with boolean operators.
    "SELECT a IS NULL",
    "SELECT a IS NOT NULL",
    "SELECT a FROM t WHERE a IS NULL AND b IS NOT NULL",
    // CASE — searched and simple forms (the simple form keeps its operand and
    // bare branch values, matching the PostgreSQL raw tree).
    "SELECT CASE WHEN a > 1 THEN 'big' WHEN a > 0 THEN 'small' ELSE 'np' END",
    "SELECT CASE a WHEN 1 THEN 'one' WHEN 2 THEN 'two' END",
    // Signed numeric literals (ADR-0015 representation-equivalence: PostgreSQL
    // folds the sign into the constant; the mapping unfolds it).
    "SELECT -1",
    "SELECT -1.5",
    "SELECT a + -2 FROM t",
];

/// Operator precedence/associativity cases. The neutral shape is a binding
/// tree, so structural parity over these asserts our parser binds mixed
/// operators exactly as PostgreSQL does (ADR-0008); the deliberate mis-bindings
/// are exercised by `pg_structural_oracle_catches_expression_misbinding`.
const PRECEDENCE_ACCEPT_CORPUS: &[&str] = &[
    "SELECT a = b AND c",
    "SELECT a OR b AND c",
    "SELECT NOT a AND b",
    "SELECT a + b * c",
    "SELECT a - b - c",
    "SELECT a AND b OR c = d",
    "SELECT a * b + c / d",
];

const ADVANCED_TABLE_ACCEPT_CORPUS: &[&str] = &[
    "SELECT * FROM LATERAL (SELECT 1) AS s(a)",
    "SELECT * FROM LATERAL generate_series(1, 3) WITH ORDINALITY AS g(x, ord)",
    "SELECT * FROM ROWS FROM (generate_series(1, 2), generate_series(3, 4)) WITH ORDINALITY AS r(a, b, ord)",
    "SELECT * FROM json_to_record('{}') AS x(a INTEGER, b TEXT)",
    "SELECT * FROM json_to_record('{}') AS (a INTEGER, b TEXT)",
    "SELECT * FROM ROWS FROM (json_to_record('{}') AS (a INTEGER), generate_series(1, 2) AS (b INTEGER)) AS r",
    "SELECT * FROM ONLY (t) AS x TABLESAMPLE BERNOULLI (10) REPEATABLE (42)",
    // Legacy `relation_expr` descendant star (`inh = true`, like a bare name).
    "SELECT * FROM t *",
    "SELECT * FROM t * AS x",
    "SELECT * FROM s.t *",
    "SELECT * FROM (t JOIN u ON t.id = u.id) AS j",
    "SELECT * FROM ((t JOIN u ON TRUE)) AS j",
    "SELECT * FROM t JOIN (u JOIN v ON u.id = v.id) ON t.id = u.id",
    "SELECT * FROM t JOIN u USING (id) AS merged",
];

const CREATE_TABLE_ACCEPT_CORPUS: &[&str] = &[
    "CREATE TABLE t (id INT PRIMARY KEY, name TEXT NOT NULL DEFAULT 'x', n INT GENERATED ALWAYS AS (id + 1) STORED, ident BIGINT GENERATED BY DEFAULT AS IDENTITY, CONSTRAINT u UNIQUE (name), CHECK (id > 0))",
    "CREATE TEMP TABLE IF NOT EXISTS t (id) ON COMMIT DROP AS SELECT 1 WITH NO DATA",
    "CREATE TABLE t (id INT) WITH (fillfactor = 70) TABLESPACE pg_default",
    "CREATE TABLE t (id BIGINT GENERATED ALWAYS AS IDENTITY (START WITH 10 INCREMENT BY 2 NO MINVALUE MAXVALUE 100 CACHE 5 CYCLE))",
    // Foreign-key referential actions (parse-foreign-key-referential-actions-*).
    // `MATCH PARTIAL` is deliberately absent here: PostgreSQL rejects it ("not yet
    // implemented") at parse time and the parser matches that verdict, so it lives in
    // `CREATE_TABLE_REJECT_CORPUS` rather than this accept corpus.
    "CREATE TABLE t (a INT REFERENCES p ON DELETE CASCADE)",
    "CREATE TABLE t (a INT REFERENCES p (id) ON DELETE SET NULL ON UPDATE CASCADE)",
    "CREATE TABLE t (a INT REFERENCES p MATCH FULL ON UPDATE RESTRICT ON DELETE NO ACTION)",
    "CREATE TABLE t (a INT, b INT, FOREIGN KEY (a, b) REFERENCES p (x, y) ON DELETE SET NULL (a, b) ON UPDATE SET DEFAULT)",
    "CREATE TABLE t (a INT, CONSTRAINT fk FOREIGN KEY (a) REFERENCES p MATCH SIMPLE ON DELETE SET DEFAULT)",
];

const INSERT_ACCEPT_CORPUS: &[&str] = &[
    "INSERT INTO t VALUES (1, 'a')",
    "INSERT INTO t (id, name) VALUES (1, DEFAULT), (2, 'b')",
    "INSERT INTO t DEFAULT VALUES",
    "INSERT INTO t SELECT 1, 'a'",
    "INSERT INTO t AS target (id) OVERRIDING SYSTEM VALUE VALUES (1)",
    "INSERT INTO t AS target (id) OVERRIDING USER VALUE VALUES (1)",
    "WITH src AS (SELECT 1) INSERT INTO t SELECT * FROM src",
    "INSERT INTO t WITH src AS (SELECT 1) SELECT * FROM src",
];
const UPDATE_DELETE_ACCEPT_CORPUS: &[&str] = &[
    "UPDATE t SET a = 1",
    "UPDATE t AS target SET a = 1",
    "UPDATE t target SET a = 1",
    "UPDATE t SET a = DEFAULT",
    "UPDATE t SET a = 1, b = b + 1 FROM u WHERE t.id = u.id",
    "WITH src AS (SELECT 1) UPDATE t SET a = 1 WHERE EXISTS (SELECT 1)",
    "DELETE FROM t",
    "DELETE FROM t AS target",
    "DELETE FROM t target",
    "DELETE FROM t USING u WHERE t.id = u.id",
    "WITH src AS (SELECT 1) DELETE FROM t WHERE EXISTS (SELECT 1)",
];
// Advanced UPDATE/DELETE forms gated by dialect data: `ONLY` targets,
// multiple-column (tuple) SET assignments, and positioned `WHERE CURRENT OF`.
const UPDATE_DELETE_ADVANCED_ACCEPT_CORPUS: &[&str] = &[
    "UPDATE ONLY t SET a = 1",
    "UPDATE ONLY (t) SET a = 1",
    "UPDATE ONLY t AS x SET a = 1",
    "DELETE FROM ONLY t",
    "DELETE FROM ONLY (t) AS d WHERE d.id = 1",
    "DELETE FROM ONLY t USING u WHERE t.id = u.id",
    // Legacy `relation_expr` descendant star on a target (`inh = true`).
    "UPDATE t * SET a = 1",
    "UPDATE t * AS x SET a = 1",
    "DELETE FROM t *",
    "DELETE FROM t * WHERE id = 1",
    "UPDATE t SET (a, b) = (1, 2)",
    "UPDATE t SET (a, b) = (1, DEFAULT)",
    "UPDATE t SET (a, b) = ROW(1, 2)",
    "UPDATE t SET (a, b) = (SELECT x, y FROM u)",
    "UPDATE t SET (a, b) = DEFAULT",
    "UPDATE t SET a = 1, (b, c) = (2, 3)",
    "UPDATE t AS x SET (a, b) = (1, 2) FROM u WHERE x.id = u.id",
    "UPDATE t SET a = 1 WHERE CURRENT OF c",
    "DELETE FROM t WHERE CURRENT OF c",
];
// PostgreSQL mutation extensions (RETURNING / ON CONFLICT), gated by dialect data.
const RETURNING_CONFLICT_ACCEPT_CORPUS: &[&str] = &[
    "INSERT INTO t VALUES (1) RETURNING *",
    "INSERT INTO t (id, name) VALUES (1, 'a') RETURNING id, name",
    "INSERT INTO t (id) VALUES (1) RETURNING id AS new_id",
    "INSERT INTO t DEFAULT VALUES RETURNING *",
    "INSERT INTO t SELECT 1 RETURNING *",
    "INSERT INTO t VALUES (1) ON CONFLICT DO NOTHING",
    "INSERT INTO t VALUES (1) ON CONFLICT (id) DO NOTHING",
    "INSERT INTO t VALUES (1) ON CONFLICT (id, name) DO NOTHING",
    "INSERT INTO t VALUES (1) ON CONFLICT (id) WHERE id > 0 DO NOTHING",
    "INSERT INTO t VALUES (1) ON CONFLICT ON CONSTRAINT t_pkey DO NOTHING",
    "INSERT INTO t (id, n) VALUES (1, 2) ON CONFLICT (id) DO UPDATE SET n = 5",
    "INSERT INTO t (id, n) VALUES (1, 2) ON CONFLICT (id) DO UPDATE SET n = excluded.n WHERE t.n < excluded.n",
    "INSERT INTO t (a, b) VALUES (1, 2) ON CONFLICT (id) DO UPDATE SET (a, b) = (excluded.a, excluded.b)",
    "INSERT INTO t VALUES (1) ON CONFLICT (id) DO NOTHING RETURNING *",
    "UPDATE t SET a = 1 RETURNING *",
    "UPDATE t SET a = 1 WHERE id = 2 RETURNING a, id",
    "UPDATE t SET a = 1 WHERE CURRENT OF c RETURNING *",
    "DELETE FROM t RETURNING *",
    "DELETE FROM t WHERE id = 1 RETURNING id",
    "DELETE FROM t WHERE CURRENT OF c RETURNING *",
];
const RETURNING_CONFLICT_REJECT_CORPUS: &[&str] = &[
    // RETURNING needs at least one output expression.
    "INSERT INTO t VALUES (1) RETURNING",
    // ON CONFLICT needs a DO action.
    "INSERT INTO t VALUES (1) ON CONFLICT",
    "INSERT INTO t VALUES (1) ON CONFLICT (id)",
];

const REJECT_CORPUS: &[&str] = &[
    "SELECT FROM",
    "SELECT 1 +",
    "SELECT * FROM",
    "SELECT (",
    "SELECT a < b < c",
    "SELECT * FROM LATERAL t",
    "SELECT * FROM LATERAL ONLY t",
    "SELECT * FROM LATERAL ONLY (t)",
    "SELECT * FROM (t) AS x",
    "SELECT * FROM ((t)) AS x",
    "SELECT * FROM (t) JOIN u ON TRUE",
    "SELECT * FROM ((t JOIN u ON TRUE) AS inner_j)",
    "SELECT * FROM ((t JOIN u ON TRUE) AS inner_j) AS outer_j",
    // A subscript indirection needs a parenthesized `c_expr`; a bare `CASE … END` is
    // not one, so PostgreSQL rejects `CASE … END[…]` (tighten-pg-overacceptance-trio).
    "SELECT CASE 1 WHEN 1 THEN MAP('a', 'b') ELSE MAP('b', 'c') END['a']",
    // A relation name is capped at catalog.schema.table; PostgreSQL rejects a fourth
    // part (tighten-pg-overacceptance-trio).
    "SELECT * FROM project.dataset.INFORMATION_SCHEMA.TABLES",
];
const ALTER_DROP_ACCEPT_CORPUS: &[&str] = &[
    "ALTER TABLE t ADD COLUMN a INT",
    "ALTER TABLE IF EXISTS t ADD COLUMN IF NOT EXISTS a INT NOT NULL DEFAULT 0",
    "ALTER TABLE t ADD b TEXT, DROP COLUMN c CASCADE",
    "ALTER TABLE t ADD CONSTRAINT u UNIQUE (a)",
    "ALTER TABLE t ADD PRIMARY KEY (a)",
    "ALTER TABLE t DROP CONSTRAINT IF EXISTS pk CASCADE",
    "ALTER TABLE t ALTER COLUMN a SET DEFAULT 0, ALTER COLUMN a DROP DEFAULT",
    "ALTER TABLE t ALTER COLUMN a SET NOT NULL",
    "ALTER TABLE t ALTER COLUMN a DROP NOT NULL",
    "ALTER TABLE t ALTER COLUMN a SET DATA TYPE BIGINT",
    "ALTER TABLE t ALTER COLUMN a TYPE TEXT",
    "DROP TABLE t",
    "DROP TABLE IF EXISTS a, b CASCADE",
    "DROP VIEW v RESTRICT",
    "DROP INDEX i",
    "DROP SCHEMA s CASCADE",
];
const ALTER_DROP_REJECT_CORPUS: &[&str] = &[
    "ALTER TABLE t",             // no action
    "ALTER TABLE t ADD",         // ADD with nothing to add
    "ALTER TABLE t DROP COLUMN", // missing column name
    "DROP",                      // missing object kind
    "DROP TABLE",                // missing name
    "DROP TABLE a,",             // dangling comma
    // A foreign-key `MATCH` type must precede the `ON UPDATE` / `ON DELETE` actions;
    // PostgreSQL rejects a trailing `MATCH` (tighten-pg-overacceptance-trio).
    "ALTER TABLE baa ADD CONSTRAINT boo FOREIGN KEY (x, y) REFERENCES persons ON UPDATE NO ACTION ON DELETE NO ACTION MATCH FULL",
    // The `RETURN <expr>` routine body is the trailing `opt_routine_body`; an option cannot
    // follow it, so PostgreSQL rejects a `LANGUAGE` after the body (proving the slot is
    // grammatically after — not merely interleaved with — the order-independent options).
    "CREATE FUNCTION a() RETURNS INT RETURN 1 LANGUAGE sql",
];
// Schema / view / index DDL (prod-sql-ddl-schema-view-index). `OR REPLACE`,
// `MATERIALIZED`, and `WITH CHECK OPTION` are PostgreSQL/ANSI view spellings; the
// `CREATE INDEX` `CONCURRENTLY` / `USING` / partial-`WHERE` clauses are gated by
// dialect data and enabled in the PostgreSQL preset.
const CREATE_SCHEMA_VIEW_INDEX_ACCEPT_CORPUS: &[&str] = &[
    "CREATE SCHEMA s",
    "CREATE SCHEMA IF NOT EXISTS s",
    "CREATE SCHEMA AUTHORIZATION joe",
    "CREATE SCHEMA s AUTHORIZATION joe",
    "CREATE VIEW v AS SELECT 1",
    "CREATE OR REPLACE VIEW v AS SELECT a FROM t",
    "CREATE VIEW v (a, b) AS SELECT 1, 2",
    "CREATE TEMP VIEW v AS SELECT 1",
    "CREATE VIEW v AS SELECT a FROM t WITH CHECK OPTION",
    "CREATE VIEW v AS SELECT a FROM t WITH CASCADED CHECK OPTION",
    "CREATE VIEW v AS SELECT a FROM t WITH LOCAL CHECK OPTION",
    "CREATE MATERIALIZED VIEW m AS SELECT 1",
    "CREATE MATERIALIZED VIEW IF NOT EXISTS m AS SELECT a FROM t WITH NO DATA",
    "CREATE MATERIALIZED VIEW m (a) AS SELECT 1 WITH DATA",
    "CREATE INDEX i ON t (a)",
    "CREATE INDEX ON t (a)",
    "CREATE UNIQUE INDEX i ON t (a, b)",
    "CREATE INDEX CONCURRENTLY i ON t (a)",
    "CREATE INDEX IF NOT EXISTS i ON t (a)",
    "CREATE INDEX i ON t USING btree (a)",
    "CREATE INDEX i ON t (lower(a))",
    "CREATE INDEX i ON t ((a + b))",
    "CREATE INDEX i ON t (a DESC NULLS LAST, b)",
    "CREATE INDEX i ON t (a) WHERE a IS NOT NULL",
    "CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS i ON s.t USING btree (a, lower(b) DESC) WHERE a IS NOT NULL",
];
const CREATE_SCHEMA_VIEW_INDEX_REJECT_CORPUS: &[&str] = &[
    "CREATE SCHEMA",                                     // no name or authorization
    "CREATE VIEW v",                                     // missing AS query
    "CREATE OR REPLACE MATERIALIZED VIEW m AS SELECT 1", // OR REPLACE is not a matview spelling
    "CREATE UNIQUE i ON t (a)",                          // UNIQUE without INDEX
    "CREATE INDEX i ON t",                               // missing column list
    "CREATE INDEX i ON t ()",                            // empty column list
];
const CREATE_TABLE_REJECT_CORPUS: &[&str] = &[
    "CREATE TABLE t (id INT) AS SELECT 1",
    // `MATCH PARTIAL` parses in standard SQL but PostgreSQL rejects it ("MATCH PARTIAL
    // not yet implemented"); the parser matches that verdict. `MATCH FULL`/`SIMPLE`
    // stay in the accept corpus above.
    "CREATE TABLE t (a INT REFERENCES p MATCH PARTIAL)",
];
const INSERT_REJECT_CORPUS: &[&str] = &[
    "INSERT INTO t target VALUES (1)",
    // A target column list attaches only to a `VALUES`/query source; PostgreSQL
    // rejects it on `DEFAULT VALUES`, and the parser now matches that verdict.
    "INSERT INTO t (a, b) DEFAULT VALUES",
];
const UPDATE_DELETE_REJECT_CORPUS: &[&str] = &[
    "DELETE t",
    "UPDATE t a = 1",
    "DELETE FROM t USING",
    // Empty target/value lists are rejected by both parsers.
    "UPDATE t SET () = ()",
];

const PG_REGRESS_SUPPORTED_SQL: &str = include_str!("../../corpus/postgres/regress-supported.sql");
const PG_REGRESS_GUIDE_SQL: &str = include_str!("../../corpus/postgres/regress-guide.sql");

#[test]
fn pg_accept_reject_parity_over_m1_corpus() {
    for sql in ACCEPT_CORPUS
        .iter()
        .chain(SUBQUERY_PREDICATE_CORPUS)
        .chain(EXPRESSION_ACCEPT_CORPUS)
        .chain(PRECEDENCE_ACCEPT_CORPUS)
        .chain(ADVANCED_TABLE_ACCEPT_CORPUS)
        .chain(PG_GROUPING_SETS_CORPUS)
        .chain(CREATE_TABLE_ACCEPT_CORPUS)
        .chain(ALTER_DROP_ACCEPT_CORPUS)
        .chain(CREATE_SCHEMA_VIEW_INDEX_ACCEPT_CORPUS)
        .chain(CREATE_SCHEMA_VIEW_INDEX_REJECT_CORPUS)
        .chain(INSERT_ACCEPT_CORPUS)
        .chain(UPDATE_DELETE_ACCEPT_CORPUS)
        .chain(UPDATE_DELETE_ADVANCED_ACCEPT_CORPUS)
        .chain(RETURNING_CONFLICT_ACCEPT_CORPUS)
        .chain(REJECT_CORPUS)
        .chain(CREATE_TABLE_REJECT_CORPUS)
        .chain(ALTER_DROP_REJECT_CORPUS)
        .chain(INSERT_REJECT_CORPUS)
        .chain(UPDATE_DELETE_REJECT_CORPUS)
        .chain(RETURNING_CONFLICT_REJECT_CORPUS)
    {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_query_oracle_matches_the_direct_accept_reject_path() {
    // Dogfood the AcceptRejectOracle seam: the M1 PostgreSQL oracle must produce
    // the same verdicts and divergences as the legacy direct pg_query checks, so
    // routing the differential through the seam changed nothing observable.
    assert_eq!(PgQueryOracle.semantics(), OracleSemantics::ParseOnly);
    for sql in ACCEPT_CORPUS
        .iter()
        .chain(SUBQUERY_PREDICATE_CORPUS)
        .chain(ADVANCED_TABLE_ACCEPT_CORPUS)
        .chain(REJECT_CORPUS)
    {
        let oracle_verdict = PgQueryOracle
            .verdict(sql)
            .expect("the in-process oracle is always available");
        assert_eq!(
            oracle_verdict,
            OracleVerdict::from_accepts(pg_query::parse(sql).is_ok()),
            "PgQueryOracle verdict diverges from direct pg_query for {sql:?}",
        );
        assert_eq!(
            crate::oracle::accept_reject_divergence(sql, Postgres, &PgQueryOracle),
            pg_accept_reject_divergence(sql),
            "seam divergence differs from the direct path for {sql:?}",
        );
    }
}

#[test]
fn pg_accepts_some_quantified_comparison_like_squonk() {
    assert_accept_reject_parity("SELECT * FROM t WHERE a = SOME (SELECT b FROM u)");
}

#[test]
fn pg_quantified_comparison_over_array_operand_matches_squonk() {
    // pg-quantified-comparison-array-operand: PostgreSQL quantifies pattern-match and
    // arbitrary operators over an array operand, and accepts a cast-of-subquery operand.
    // All map to `ScalarArrayOpExpr` and are outside the neutral structural corpus, so
    // this stays an accept/reject parity claim (engine-probed via pg_query).
    for sql in [
        // LIKE / ILIKE / NOT LIKE quantified over an array value.
        "SELECT 'foo' LIKE ANY (ARRAY['%a', '%o'])",
        "SELECT 'foo' LIKE ALL (ARRAY['f%', '%o'])",
        "SELECT 'foo' NOT LIKE ANY (ARRAY['%a', '%b'])",
        "SELECT 'foo' NOT LIKE ALL (ARRAY['%a', '%o'])",
        "SELECT 'foo' ILIKE ANY (ARRAY['%A', '%O'])",
        "SELECT 'foo' NOT ILIKE ALL (ARRAY['F%', '%O'])",
        // Any operator, not only the comparisons (PostgreSQL's `MathOp`/`Op`).
        "SELECT 3 * ANY ('{1,2,3}')",
        "SELECT 3 + ANY (ARRAY[1, 2, 3])",
        // A cast of a scalar subquery is an expression operand, not a bare subquery.
        "SELECT 'foo'::text = ANY ((SELECT ARRAY['abc','def','foo']::text[])::text[])",
        // `SIMILAR TO` has no quantified form — a shared reject.
        "SELECT 'foo' SIMILAR TO ANY (ARRAY['%a'])",
        // The boolean keywords are not quantifiable — a shared reject.
        "SELECT 3 AND ANY ('{1,2,3}')",
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_within_group_ordered_set_aggregates_match_squonk() {
    // WITHIN GROUP ordered-set aggregates (SQL:2008 T612/T614), closing
    // parse-within-group-ordered-set-aggregates. Kept out of `EXPRESSION_ACCEPT_CORPUS`
    // so this stays an accept/reject parity claim: PostgreSQL folds the WITHIN GROUP
    // sort key into `agg_order` and sets `agg_within_group`, which the neutral
    // `FunctionShape` gate rejects (like FILTER/OVER), so the pair is not structurally
    // comparable.
    for sql in [
        "SELECT LISTAGG(x) WITHIN GROUP (ORDER BY x) AS y",
        "SELECT LISTAGG(x) WITHIN GROUP (ORDER BY x DESC)",
        "SELECT PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY x)",
        "SELECT PERCENTILE_DISC(0.5) WITHIN GROUP (ORDER BY x)",
        // WITHIN GROUP precedes FILTER and OVER, matching PostgreSQL's grammar order.
        "SELECT PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY x) FILTER (WHERE x > 0) OVER w",
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_rejects_within_group_conflicts_like_squonk() {
    // PostgreSQL rejects these at parse time (gram.y `func_expr`): WITHIN GROUP
    // shares the aggregate ORDER BY slot and an ordered-set aggregate is never
    // DISTINCT. The parser matches those rejects.
    for sql in [
        "SELECT array_agg(x ORDER BY y) WITHIN GROUP (ORDER BY z)",
        "SELECT count(DISTINCT x) WITHIN GROUP (ORDER BY y)",
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_qualified_wildcard_alias_matches_squonk() {
    // parse-qualified-wildcard-bare-alias. PostgreSQL reads `t.*` as an ordinary columnref,
    // so it takes the standard `[AS] label` projection alias; the Postgres preset ships
    // `qualified_wildcard_alias` on. Both directions of the surface, all engine-probed
    // against libpg_query and asserted to agree:
    for sql in [
        // Accepts: bare and `AS` aliases, a multi-part prefix, and the minimized fuzz
        // reproducer (`hEE.*` then the bare label `LC`).
        "SELECT t.* a FROM t",
        "SELECT t.* AS a FROM t",
        "SELECT s.t.* a FROM s.t",
        "SELECT hEE.*LC",
        // Reserved-word boundary — identical to an ordinary projection alias: the bare form
        // is a `BareColLabel` (admits `select`, rejects the fully-reserved `from`/`order`),
        // the `AS` form the full `ColLabel` (admits `from`).
        "SELECT t.* select FROM t",
        "SELECT t.* AS from FROM t",
        "SELECT t.* from FROM t",
        "SELECT t.* order FROM t",
        // Asymmetry: a bare `*` is the non-aliasable `target_el: '*'` production, so a
        // trailing word rejects on both sides.
        "SELECT * a FROM t",
        "SELECT * AS a FROM t",
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_accepts_extract_string_field_like_squonk() {
    // pg-extract-string-field-widening: PostgreSQL admits a single-quoted string as the
    // `EXTRACT(<field> FROM x)` field (`Sconst` in gram.y `extract_arg`), so the Postgres
    // preset ships `extract_string_field` on. The quoted field interns as a
    // `QuoteStyle::Single` identifier and round-trips. The bare-identifier and
    // double-quoted-identifier fields (the standard form) are unaffected and stay accepted.
    // Kept out of ACCEPT_CORPUS so this is a focused accept/reject parity claim — the
    // structural mapper does not need an `EXTRACT`-field shape to make the point.
    for sql in [
        "SELECT EXTRACT('year' FROM x)",
        "SELECT EXTRACT('month' FROM x)",
        "SELECT EXTRACT('epoch' FROM now())",
        "SELECT EXTRACT('doy' FROM TIMESTAMP '2020-01-01')",
        "SELECT EXTRACT('hour' FROM INTERVAL '1 day')",
        "SELECT EXTRACT(year FROM x)",
        "SELECT EXTRACT(\"year\" FROM x)",
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_rejects_non_string_non_identifier_extract_field_like_squonk() {
    // The reject boundary of the widening: `extract_arg` admits only an identifier, a
    // small set of unreserved field keywords, or a single `Sconst` — so a numeric literal,
    // a compound expression, two adjacent strings, or an empty field are all parse errors
    // in PostgreSQL, and ours rejects the same four with the flag on (the string-alias
    // reader falls through to `parse_ident`, which fails on a non-identifier field).
    for sql in [
        "SELECT EXTRACT(123 FROM x)",
        "SELECT EXTRACT('year' + 1 FROM x)",
        "SELECT EXTRACT(1.5 FROM x)",
        "SELECT EXTRACT('year' 'x' FROM x)",
        "SELECT EXTRACT(FROM x)",
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_accepts_double_quoted_identifiers_like_squonk() {
    // PostgreSQL enables only the standard `"..."` identifier quote
    // (prod-sql-quoted-identifiers). The quoted column ref, quoted table name,
    // doubled-quote escape, and a fully-quoted qualified name are accepted by
    // both the real parser and ours. Kept out of ACCEPT_CORPUS so this stays a
    // focused accept/reject parity claim, independent of structural shape.
    for sql in [
        "SELECT \"x\"",
        "SELECT \"x\" FROM \"t\"",
        "SELECT \"a\"\"b\"",
        "SELECT \"s\".\"t\".\"c\"",
        "SELECT 1 AS \"from\"",
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_rejects_non_postgres_identifier_quotes_like_squonk() {
    // PostgreSQL does not enable backtick or bracket identifier quoting, so those
    // delimiters are not valid identifiers there — and ours rejects them under the
    // PostgreSQL preset too, since the tokenizer only emits a quoted-identifier
    // token for a configured style.
    for sql in ["SELECT `x`", "SELECT [x]"] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_rejects_zero_length_delimited_identifiers_like_squonk() {
    // reject-zero-length-delimited-identifier-pg-mysql-parity: SQL's `<delimited
    // identifier body>` requires at least one character. PostgreSQL rejects a
    // zero-length delimited identifier while scanning ("zero-length delimited
    // identifier", scan.l) in every position an identifier can appear — a
    // projection item, a qualified column reference, an `AS` alias, and a table
    // name — and ours rejects the same four at the tokenizer layer. `U&""` is
    // scanned by our dedicated `U&"..."` Unicode-escaped-identifier arm, which
    // rejects the zero-length body over the whole `U&""` lexeme exactly as
    // PostgreSQL's own `U&"..."` scanner arm does.
    for sql in [
        "SELECT \"\" FROM t",
        "SELECT x.\"\" FROM t",
        "SELECT a AS \"\" FROM t",
        "SELECT * FROM \"\"",
        "SELECT U&\"\" FROM t",
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_unicode_escaped_identifiers_match_squonk() {
    // pg-unicode-escaped-identifiers: PostgreSQL's `U&"..."` delimited-identifier form
    // (the `U&'...'` string's identifier twin) and its trailing `UESCAPE 'c'` clause are
    // accepted by both parsers — a column ref, an `AS` alias, and a table name — and the
    // eager parse-time rejects (malformed escape, illegal `UESCAPE` delimiter, a greedy
    // `UESCAPE` keyword with no following string) match too, so no case over-accepts. Kept
    // out of ACCEPT_CORPUS: the neutral structural mapping has no U&-identifier shape, so
    // only accept/reject parity is asserted here.
    for sql in [
        // Accepts (the exact PG regress corpus lines this ticket closes, plus positions).
        r#"SELECT U&"real\00A7_name" FROM (select 1) AS x(real_name)"#,
        r#"SELECT U&'d\0061t\+000061' AS U&"d\0061t\+000061""#,
        r#"SELECT U&'d!0061t\+000061' UESCAPE '!' AS U&"d*0061t\+000061" UESCAPE '*'"#,
        r#"SELECT 'tricky' AS U&"\" UESCAPE '!'"#,
        r#"SELECT * FROM U&"my\0074able""#,
        r#"SELECT U&"d0061" UESCAPE '-'"#,
        // Rejects, matching PostgreSQL's parse-time boundary (no over-acceptance).
        r#"SELECT U&"d0061" UESCAPE '+'"#,
        r#"SELECT U&"\ZZZZ""#,
        r#"SELECT U&"\d800""#,
        r#"SELECT U&"x" uescape FROM t"#,
        r#"SELECT "x" UESCAPE '!'"#,
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_accepts_positional_parameter_like_squonk() {
    // PostgreSQL's `$1` placeholder is accepted by both the real parser and ours
    // under the PostgreSQL preset. It is kept out of ACCEPT_CORPUS because that
    // corpus also drives structural parity, which has no parameter shape mapping.
    assert_accept_reject_parity("SELECT $1");
}

/// Parenthesized query expressions in derived-table (`FROM`) position
/// (parse-parenthesized-set-operation-operand-in-derived-table-from-position):
/// PostgreSQL's `select_with_parens` admits a parenthesized query as a set-op
/// operand, so a 3+-arm set-op derived table round-trips through the Parenthesized
/// render — `FROM ((SELECT …) UNION …) x` — and re-parses. Kept out of ACCEPT_CORPUS
/// because the neutral structural mapping does not cover the derived-table set-op
/// shape, so only accept/reject parity and the render round-trip are asserted.
const PG_DERIVED_TABLE_SET_OP_CORPUS: &[&str] = &[
    // The canonical 3-arm set-op derived table and its fully-parenthesized render
    // (the regression: the latter must re-parse as a derived table, not a join).
    "SELECT * FROM (SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3) AS x",
    "SELECT * FROM ((SELECT 1 UNION ALL SELECT 2) UNION ALL SELECT 3) AS x",
    // A parenthesized leading operand of other forms: a VALUES operand, both arms
    // parenthesized under EXCEPT, and an unaliased group.
    "SELECT * FROM ((VALUES (1)) UNION ALL SELECT 2) AS x",
    "SELECT * FROM ((SELECT 1) EXCEPT (SELECT 2)) AS x",
    "SELECT * FROM ((SELECT 1) UNION ALL SELECT 2)",
    // Deeper nesting, and redundant query parens (`select_with_parens` nesting).
    "SELECT * FROM (((SELECT 1) UNION SELECT 2) UNION SELECT 3) AS x",
    "SELECT * FROM ((SELECT 1)) AS x",
    // A parenthesized *join* whose first factor is a derived table: the trailing
    // `JOIN` (not a set-op keyword) keeps it a joined table, not a query — the
    // speculative query reading must rewind here.
    "SELECT * FROM ((SELECT 1) AS a JOIN (SELECT 2) AS b ON TRUE) AS j",
];

/// Parenthesized `FROM`-position forms PostgreSQL rejects and so must we: extra
/// parens around a plain table reference, and around a single (aliased) derived
/// table — neither is a `joined_table`, the only thing `'(' … ')'` wraps in
/// `table_ref`, nor a `select_with_parens`.
const PG_DERIVED_TABLE_REJECT_CORPUS: &[&str] = &[
    "SELECT * FROM ((t)) AS x",
    "SELECT * FROM ((SELECT 1) AS a)",
];

#[test]
fn pg_derived_table_set_op_accepts_like_squonk() {
    for sql in PG_DERIVED_TABLE_SET_OP_CORPUS
        .iter()
        .chain(PG_DERIVED_TABLE_REJECT_CORPUS)
    {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_derived_table_set_op_render_round_trips() {
    // The Parenthesized mode `roundtrip` also checks is the precedence oracle that
    // originally exposed the parser gap here: it wraps the left set-op arm,
    // producing `FROM ((SELECT …) UNION …) x`.
    for sql in PG_DERIVED_TABLE_SET_OP_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the derived-table set-op form {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("derived-table set-op round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

/// PostgreSQL `SELECT … INTO <table>` create-table form (select-into ticket): the
/// new-table target sits between the projection and `FROM`, with the optional
/// `TEMP` / `TEMPORARY` markers. Only the PostgreSQL preset gates this form, so it
/// round-trips under Postgres; this is the materialize-into-a-new-table construct,
/// distinct from the SQL-standard `SELECT … INTO <variable>` assignment.
const PG_SELECT_INTO_CORPUS: &[&str] = &[
    "SELECT a INTO t FROM s",
    "SELECT a INTO TEMP t FROM s",
    "SELECT a INTO TEMPORARY t FROM s",
    "SELECT a, b INTO public.t FROM s WHERE a > 1",
];

#[test]
fn pg_select_into_render_round_trips() {
    for sql in PG_SELECT_INTO_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the SELECT INTO form {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("SELECT INTO round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

/// The SQL:1999 grouping-set GROUP BY items
/// (model-group-by-grouping-sets-rollup-cube): `ROLLUP`/`CUBE`/`GROUPING SETS`/
/// empty `()`, plus mixed and nested forms and the quoted-`"rollup"` counter-case
/// that stays a function call. PostgreSQL emits a `GroupingSet` node for each
/// construct, so `StatementShape` now compares the grouping-set tree from both
/// parsers — the structural check that would have caught the original mis-parse.
/// Every member is bare-column (no parenthesized multi-column set), so both the
/// leaves and the tree map into the neutral shape rather than `ExprShape::Unmapped`.
const PG_GROUPING_SETS_CORPUS: &[&str] = &[
    "SELECT a FROM t GROUP BY ROLLUP (a, b)",
    "SELECT a FROM t GROUP BY CUBE (a, b)",
    "SELECT a FROM t GROUP BY ()",
    "SELECT a FROM t GROUP BY a, ROLLUP (b, c), CUBE (d)",
    "SELECT a FROM t GROUP BY GROUPING SETS (a, b, ())",
    "SELECT a FROM t GROUP BY GROUPING SETS (ROLLUP (a, b), c)",
    // A quoted `"rollup"` is a delimited identifier, so it stays a function call on
    // both sides — the counter-case proving only the bare keyword is lowered.
    "SELECT a FROM t GROUP BY \"rollup\" (a, b)",
];

#[test]
fn pg_structural_parity_for_grouping_sets() {
    // Closes the oracle hole: the grouping-set tree shape is now compared, so a
    // regression to the function-call mis-parse would fail structural parity.
    for sql in PG_GROUPING_SETS_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_grouping_sets_render_round_trips() {
    for sql in PG_GROUPING_SETS_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the grouping-set form {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("grouping-set round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

/// Named arguments on a table-valued function in FROM position
/// (planner-parity-table-factor-named-table-function-args). PostgreSQL's `func_table`
/// reuses `func_application`, so the `name => value` / deprecated `name := value`
/// argument arrows are admissible in FROM exactly as in a scalar call, including a
/// mixed positional-then-named list. Our parser reaches the same shared call-argument
/// grammar from the FROM factor, so these ride the existing grammar with no dedicated
/// FROM-position handling — this corpus locks the real-oracle accept parity and the
/// round-trip of both arrow spellings.
const PG_TABLE_FUNCTION_NAMED_ARG_CORPUS: &[&str] = &[
    "SELECT * FROM f(x => 1)",
    "SELECT * FROM f(x := 1)",
    "SELECT * FROM generate_series(1, 3, step => 1)",
    "SELECT * FROM f(1, y => 2)",
];

#[test]
fn pg_table_function_named_args_accept_like_squonk() {
    for sql in PG_TABLE_FUNCTION_NAMED_ARG_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_table_function_named_args_render_round_trip() {
    for sql in PG_TABLE_FUNCTION_NAMED_ARG_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the named table-function arg form {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("named table-function arg round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

/// PostgreSQL's `GROUP BY {DISTINCT | ALL} <grouping items>` set-quantifier
/// (pg-group-by-distinct-grouping-sets, SQL:2016 T434). PostgreSQL records the choice
/// as `SelectStmt.group_distinct` (explicit `ALL` and an unwritten quantifier both map
/// `false`), so `StatementShape::group_by_distinct` compares the flag from both parsers
/// — the structural check proving we read `DISTINCT` where PostgreSQL does. Bare-column
/// grouping items keep the leaves and tree in the neutral shape.
const PG_GROUP_BY_QUANTIFIER_CORPUS: &[&str] = &[
    "SELECT a FROM t GROUP BY DISTINCT a, b",
    "SELECT a FROM t GROUP BY ALL a, b",
    "SELECT a FROM t GROUP BY DISTINCT ROLLUP (a, b)",
    "SELECT a FROM t GROUP BY ALL CUBE (a, b)",
    "SELECT a FROM t GROUP BY DISTINCT GROUPING SETS (a, b, ())",
    // The unquantified default maps `group_distinct = false`, same as explicit `ALL`.
    "SELECT a FROM t GROUP BY a, b",
];

#[test]
fn pg_structural_parity_for_group_by_quantifier() {
    // The grouping-set quantifier is recorded in the neutral shape, so a regression that
    // dropped or mis-read `DISTINCT` would fail structural parity against PostgreSQL.
    for sql in PG_GROUP_BY_QUANTIFIER_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_group_by_quantifier_bare_forms_reject_like_pg() {
    // The quantifier is a prefix on a non-empty item list, never a standalone clause —
    // PostgreSQL rejects a bare `GROUP BY {ALL | DISTINCT}` (the MECE boundary against
    // DuckDB's standalone `GROUP BY ALL` mode). Accept/reject parity, engine-probed.
    for sql in [
        "SELECT a FROM t GROUP BY DISTINCT",
        "SELECT a FROM t GROUP BY ALL",
        "SELECT a FROM t GROUP BY a DISTINCT",
        "SELECT a FROM t GROUP BY DISTINCT ALL a",
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_group_by_quantifier_render_round_trips() {
    for sql in PG_GROUP_BY_QUANTIFIER_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the group-by quantifier form {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("group-by quantifier round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

// --- IS [NOT] DISTINCT FROM (close-p0-datafusion-parity-coverage-gaps) -------
// The null-safe comparison predicate, routed through
// `BinaryOperator::Is[Not]DistinctFrom` at comparison precedence. PostgreSQL lowers
// it to a DISTINCT-kind `AExpr`, mapped to the same `BinaryOp` shape, so both leaves
// and the operator compare structurally.
const PG_IS_DISTINCT_FROM_CORPUS: &[&str] = &[
    "SELECT a IS DISTINCT FROM b",
    "SELECT a IS NOT DISTINCT FROM b",
    "SELECT NULL IS DISTINCT FROM a",
    "SELECT (a + 1) IS DISTINCT FROM (b * 2)",
    "SELECT a IS DISTINCT FROM b AND c IS NOT DISTINCT FROM d",
];

#[test]
fn pg_structural_parity_for_is_distinct_from() {
    for sql in PG_IS_DISTINCT_FROM_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_is_distinct_from_render_round_trips() {
    for sql in PG_IS_DISTINCT_FROM_CORPUS {
        expect_round_trip(sql, "IS DISTINCT FROM");
    }
}

#[test]
fn pg_is_distinct_from_chains_reject_like_postgres() {
    // `%prec IS` is non-associative (gram.y), so a chained predicate is a syntax
    // error on both sides — accept/reject parity confirms we reject it too.
    for sql in [
        "SELECT a IS DISTINCT FROM b IS DISTINCT FROM c",
        "SELECT a IS NOT DISTINCT FROM b IS NOT DISTINCT FROM c",
    ] {
        assert_accept_reject_parity(sql);
    }
}

// --- Row-locking clauses (mysql-select-tails-locking-hints-partition) ---------
// The modern `FOR UPDATE`/`FOR SHARE [OF …] [NOWAIT|SKIP LOCKED]` surface MySQL and
// PostgreSQL share, modelled once as the canonical `Query.locking` clause (ADR-0011)
// and gated `query_tail_syntax.locking_clauses`. PostgreSQL records the clause on the
// `SelectStmt.locking_clause` list, so `QueryShape.locking` now compares the
// semantic strength / `OF` targets / wait policy from both parsers (the surface
// spelling is dropped). PostgreSQL's richer forms now parse too
// (pg-locking-clause-strengths-and-stacking): the `NO KEY UPDATE`/`KEY SHARE`
// strengths (`query_tail_syntax.key_lock_strengths`), the multi-table `OF a, b` list (the
// shared comma form), and stacked clauses (`query_tail_syntax.stacked_locking_clauses`) —
// each engine-verified accept, mapping to the same stacked/strength shape.
const PG_LOCKING_CLAUSE_CORPUS: &[&str] = &[
    "SELECT a FROM t1 FOR UPDATE",
    "SELECT a FROM t1 FOR SHARE",
    "SELECT a FROM t1 FOR UPDATE OF t1",
    "SELECT a FROM t1 FOR SHARE OF t1",
    "SELECT a FROM t1 FOR UPDATE NOWAIT",
    "SELECT a FROM t1 FOR UPDATE SKIP LOCKED",
    "SELECT a FROM t1 FOR SHARE OF t1 NOWAIT",
    // The clause also trails `ORDER BY`/`LIMIT` (PostgreSQL accepts it after the
    // limit, matching MySQL's fixed position).
    "SELECT a FROM t1 ORDER BY a LIMIT 5 FOR UPDATE",
    // PostgreSQL-only strengths (`key_lock_strengths`): the two levels between
    // `FOR UPDATE` and `FOR SHARE`, with and without `OF`/wait tails.
    "SELECT a FROM t1 FOR NO KEY UPDATE",
    "SELECT a FROM t1 FOR KEY SHARE",
    "SELECT a FROM t1 FOR NO KEY UPDATE OF t1",
    "SELECT a FROM t1 FOR KEY SHARE NOWAIT",
    "SELECT a FROM t1 FOR NO KEY UPDATE OF t1 SKIP LOCKED",
    // The multi-table `OF a, b` restriction list.
    "SELECT a FROM t1, t2 FOR UPDATE OF t1, t2",
    // Stacked clauses (`stacked_locking_clauses`): one lock per table group, with the
    // new strengths and per-clause wait tails, and after `ORDER BY`/`LIMIT`.
    "SELECT a FROM t1 FOR UPDATE FOR SHARE",
    "SELECT a FROM t1, t2 FOR UPDATE OF t1 FOR SHARE OF t2",
    "SELECT a FROM t1, t2 FOR NO KEY UPDATE OF t1 FOR KEY SHARE OF t2",
    "SELECT a FROM t1, t2 FOR UPDATE OF t1 NOWAIT FOR SHARE OF t2 SKIP LOCKED",
    "SELECT a FROM t1 ORDER BY a LIMIT 5 FOR UPDATE FOR KEY SHARE",
];

#[test]
fn pg_structural_parity_for_locking_clauses() {
    // The neutral `QueryShape.locking` now compares the clause on both sides, so a
    // regression that drops or mis-maps the strength / OF / wait would fail here.
    for sql in PG_LOCKING_CLAUSE_CORPUS {
        assert_accept_reject_parity(sql);
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_locking_clause_render_round_trips() {
    for sql in PG_LOCKING_CLAUSE_CORPUS {
        expect_round_trip(sql, "locking clause");
    }
}

// The strength boundary PostgreSQL rejects: `NO KEY` pairs only with `UPDATE` and `KEY`
// only with `SHARE`, and a bare `FOR NO`/`FOR KEY` is incomplete (probed on libpg_query,
// pg-locking-clause-strengths-and-stacking). Our parser must reject the same set — the
// `expect_keyword` after each strength lead enforces the pairing — so neither over-accepts.
const PG_LOCKING_REJECT_BOUNDARY: &[&str] = &[
    "SELECT a FROM t1 FOR KEY UPDATE",
    "SELECT a FROM t1 FOR NO KEY SHARE",
    "SELECT a FROM t1 FOR NO UPDATE",
    "SELECT a FROM t1 FOR KEY",
    "SELECT a FROM t1 FOR NO KEY",
];

#[test]
fn pg_rejects_malformed_lock_strengths_like_squonk() {
    for sql in PG_LOCKING_REJECT_BOUNDARY {
        assert!(!postgres_accepts(sql), "PostgreSQL rejects {sql:?}");
        assert!(!squonk_accepts(sql), "squonk rejects {sql:?}");
        assert_accept_reject_parity(sql);
    }
}

// --- TRUNCATE (close-p0-datafusion-parity-coverage-gaps) ---------------------
const PG_TRUNCATE_CORPUS: &[&str] = &[
    "TRUNCATE TABLE t",
    // The `TABLE` keyword is optional sugar; the shape and the canonical render are
    // identical to the spelled form.
    "TRUNCATE t",
    "TRUNCATE TABLE a, b",
    "TRUNCATE TABLE t RESTART IDENTITY",
    "TRUNCATE TABLE t CONTINUE IDENTITY",
    "TRUNCATE TABLE t CASCADE",
    "TRUNCATE TABLE t RESTART IDENTITY CASCADE",
];

#[test]
fn pg_structural_parity_for_truncate() {
    for sql in PG_TRUNCATE_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_truncate_render_round_trips() {
    for sql in PG_TRUNCATE_CORPUS {
        expect_round_trip(sql, "TRUNCATE");
    }
}

// --- COMMENT ON (close-p0-datafusion-parity-coverage-gaps) -------------------
// TABLE/COLUMN/DATABASE map structurally; PROCEDURE is accept/reject + round-trip
// only (its argument types canonicalize on the PostgreSQL side — see
// `CommentOnShape`).
const PG_COMMENT_ON_STRUCTURAL_CORPUS: &[&str] = &[
    "COMMENT ON TABLE t IS 'a table'",
    "COMMENT ON TABLE my_schema.my_table IS 'Employee Information'",
    "COMMENT ON COLUMN my_schema.my_table.my_column IS 'Employee ID number'",
    "COMMENT ON DATABASE my_database IS 'Development Database'",
    "COMMENT ON TABLE t IS NULL",
];
const PG_COMMENT_ON_ACCEPT_CORPUS: &[&str] = &[
    "COMMENT ON PROCEDURE my_proc(integer, integer) IS 'Runs a report'",
    "COMMENT ON PROCEDURE my_proc IS 'unspecified signature'",
];

#[test]
fn pg_structural_parity_for_comment_on() {
    for sql in PG_COMMENT_ON_STRUCTURAL_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_comment_on_accept_parity() {
    for sql in PG_COMMENT_ON_ACCEPT_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_comment_on_render_round_trips() {
    for sql in PG_COMMENT_ON_STRUCTURAL_CORPUS
        .iter()
        .chain(PG_COMMENT_ON_ACCEPT_CORPUS)
    {
        expect_round_trip(sql, "COMMENT ON");
    }
}

/// Assert `sql` parses under PostgreSQL and survives render -> re-parse in both
/// render modes (shared by the P0-gap round-trip tests).
fn expect_round_trip(sql: &str, label: &str) {
    match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
        crate::corpus_roundtrip::Roundtrip::Ok => {}
        crate::corpus_roundtrip::Roundtrip::Unparsable => {
            panic!("Postgres should parse the {label} form {sql:?}")
        }
        crate::corpus_roundtrip::Roundtrip::Failed(message) => {
            panic!("{label} round-trip failed for {sql:?}: {message}")
        }
    }
}

#[test]
fn postgres_order_by_using_operator_matches_pg() {
    use squonk::dialect::{Ansi, MySql};

    // PostgreSQL's operator-driven `ORDER BY <expr> USING <operator>` sort (gram.y
    // `sortby: a_expr USING qual_all_Op opt_nulls_order`): the bare symbolic
    // operator, a schema-qualified `OPERATOR(schema.op)`, and a trailing NULLS
    // order. Each parses in both engines and maps to the *same* neutral shape — the
    // operator lands in `using`, not `asc` — and round-trips through both render
    // modes, so this is structural parity, not mere acceptance.
    for sql in [
        "SELECT a FROM t ORDER BY a USING <",
        "SELECT a FROM t ORDER BY a USING > NULLS LAST",
        "SELECT a FROM t ORDER BY a USING OPERATOR(pg_catalog.<)",
        "SELECT a, b FROM t ORDER BY a USING <, b DESC",
    ] {
        assert_structural_parity(sql);
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the ORDER BY USING statement {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("ORDER BY USING round-trip failed for {sql:?}: {message}")
            }
        }
    }

    // The `USING` sort is PostgreSQL-only: ANSI and MySQL have only `ASC`/`DESC`, so
    // the `USING` keyword is left unconsumed and surfaces as a trailing-input parse
    // error (`order_by_using` gate off).
    let using_sql = "SELECT a FROM t ORDER BY a USING <";
    assert!(
        parse_with(using_sql, Ansi).is_err(),
        "ANSI has no ORDER BY USING sort operator",
    );
    assert!(
        parse_with(using_sql, MySql).is_err(),
        "MySQL has no ORDER BY USING sort operator",
    );
}

#[test]
fn postgres_fetch_first_tail_forms_match_pg() {
    // The `FETCH { FIRST | NEXT } ...` tail admits an optional row count
    // (PostgreSQL defaults the omitted count to 1, `gram.y`: `FETCH
    // first_or_next row_or_rows ONLY`) and a `WITH TIES` choice
    // (`LIMIT_OPTION_WITH_TIES`) alongside the default `ONLY`. Every `WITH TIES`
    // member carries an `ORDER BY`: PostgreSQL's `insertSelectOptions` raw-parse
    // guard rejects `WITH TIES` without one, and the `Postgres` preset enforces the
    // same (`QueryTailSyntax::with_ties_requires_order_by`).
    for sql in [
        "SELECT * FROM test FETCH FIRST ROWS ONLY",
        "SELECT * FROM test ORDER BY id DESC FETCH FIRST 10 ROWS WITH TIES",
        "SELECT * FROM test FETCH FIRST 5 ROWS ONLY",
        "SELECT * FROM test ORDER BY id FETCH FIRST 5 ROWS WITH TIES",
    ] {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the FETCH FIRST statement {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("FETCH FIRST round-trip failed for {sql:?}: {message}")
            }
        }
    }
    // The un-ordered `WITH TIES` form is the guard's reject on both sides — real
    // PostgreSQL refuses it at raw parse, and so does the preset.
    let unordered = "SELECT * FROM test FETCH FIRST 5 ROWS WITH TIES";
    assert!(
        !squonk_accepts(unordered),
        "WITH TIES without ORDER BY rejects under the Postgres preset",
    );
    assert!(
        !postgres_accepts(unordered),
        "PostgreSQL rejects WITH TIES without ORDER BY at raw parse",
    );
    // Structural parity needs an explicit count: PostgreSQL's raw tree
    // synthesizes an implicit `AConst(1)` (`location: -1`, not from source
    // text) for the count-omitted form, while `Limit::limit` stays `None`
    // per its "unwritten stays `None`" convention — an explicit count
    // sidesteps that synthesized-vs-absent mismatch entirely.
    assert_structural_parity("SELECT * FROM test FETCH FIRST 5 ROWS ONLY");
    assert_structural_parity("SELECT * FROM test ORDER BY id DESC FETCH FIRST 10 ROWS WITH TIES");
}

#[test]
fn postgres_parenthesized_insert_source_matches_pg() {
    // A parenthesized query source (`SelectStmt: select_with_parens`) is
    // pure grouping around the same INSERT source the unparenthesized
    // spelling parses: `parse_query` already resolves a leading `(`
    // recursively (the set-operation climb, a nested `WITH`), so these
    // structurally match the identical unparenthesized form and round-trip.
    for sql in [
        "INSERT INTO x (SELECT * FROM y)",
        "INSERT INTO y (SELECT 1) UNION (SELECT 2)",
        "INSERT INTO result_table (WITH test AS (SELECT * FROM source_table) SELECT * FROM test)",
    ] {
        assert_structural_parity(sql);
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the parenthesized INSERT source {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("parenthesized INSERT source round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

#[test]
fn postgres_set_operand_with_own_clauses_matches_pg() {
    // A parenthesized set operand can carry its own ORDER BY/LIMIT bound to the
    // operand (not the whole set operation): PostgreSQL pins them on the operand
    // `SelectStmt`, squonk wraps the operand in `SetExpr::Query`. Both now map
    // to `SetShape::Query`, so the clauses are compared instead of silently dropped
    // on both sides (the set-shape-drops-nested-query-clauses gap). The clause-free
    // operand stays the flat shape via the unwrap normalization, keeping parity
    // green.
    for sql in [
        "(SELECT 1) UNION (SELECT 2 ORDER BY 1)",
        "SELECT 1 UNION (SELECT 2 ORDER BY 1 LIMIT 3)",
        "(SELECT 1) UNION (SELECT 2)",
    ] {
        assert_structural_parity(sql);
    }

    // Guard the capture directly: parity alone would also pass if both sides
    // regressed to dropping the operand clauses, as they did before this fix.
    let StatementShape::Query(query) =
        first_ours_shape("SELECT 1 UNION (SELECT 2 ORDER BY 1 LIMIT 3)")
    else {
        panic!("expected a query shape");
    };
    let SetShape::SetOperation { right, .. } = &query.body else {
        panic!("expected a set-operation body, got {:?}", query.body);
    };
    let SetShape::Query(inner) = right.as_ref() else {
        panic!("expected the clause-carrying operand to map to SetShape::Query, got {right:?}");
    };
    assert_eq!(inner.order_by.len(), 1, "operand ORDER BY must be captured");
    assert!(
        inner.limit.count.is_some(),
        "operand LIMIT must be captured, got {:?}",
        inner.limit,
    );
}

#[test]
fn postgres_parenthesized_set_operand_in_expression_position_matches_pg() {
    // `select_with_parens` recurses directly in PostgreSQL's grammar (`'('
    // select_with_parens ')'`), so a parenthesized set operation can open a
    // scalar subquery or an `IN` operand the same way it already can in
    // `FROM` position — `parse_grouped`'s speculative read
    // (checkpoint/rewind over the same `query` set-op climb) resolves it
    // the same way `from::try_parenthesized_query_factor` already does for
    // derived tables.
    for sql in [
        "SELECT * FROM x WHERE y IN ((SELECT 1) EXCEPT (SELECT 2))",
        "SELECT ((SELECT 0) UNION (SELECT 1) ORDER BY 1 OFFSET 1)",
        "SELECT * FROM x WHERE y IN ((SELECT 1) UNION (SELECT 2) OFFSET 2)",
    ] {
        assert_structural_parity(sql);
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the parenthesized set-op operand {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("parenthesized set-op operand round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

#[test]
fn postgres_stacked_joins_right_nest_like_pg() {
    // PostgreSQL's `joined_table` grammar is right-recursive (`table_ref:
    // ... | joined_table`): an unqualified `a JOIN b` cannot reduce while a
    // `JOIN` keyword still follows (its `join_qual` is mandatory), so the
    // parser is forced to keep extending the right operand. `a JOIN b JOIN
    // c ON e1 ON e2` therefore right-nests as `a JOIN (b JOIN c ON e1) ON
    // e2` — the *nearest* `ON`/`USING` closes the innermost,
    // most-recently-opened join. Confirmed against `pg_query`'s raw
    // `JoinExpr` tree directly: `larg: a, rarg: JoinExpr { larg: b, rarg:
    // c, quals: e1 }, quals: e2` (not just accept/reject).
    for sql in [
        "SELECT 1 FROM a JOIN b JOIN c ON b.id = c.id ON a.id = b.id",
        "SELECT * FROM a JOIN b JOIN c USING (id) USING (id)",
    ] {
        assert_structural_parity(sql);
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the stacked join {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("stacked join round-trip failed for {sql:?}: {message}")
            }
        }
    }

    // The tree-shape proof itself: assert the right-nested structure
    // directly, independent of the `pg_query` cross-check above — a
    // `NestedJoin` factor wrapping `b JOIN c`, not a flat 2-join list on `a`.
    let sql = "SELECT 1 FROM a JOIN b JOIN c ON b.id = c.id ON a.id = b.id";
    let parsed = parse_with(sql, Postgres).expect("stacked join parses");
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query statement");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a plain SELECT body");
    };
    assert_eq!(select.from.len(), 1, "one comma-free FROM item");
    let table = &select.from[0];
    assert!(
        matches!(table.relation, TableFactor::Table { .. }),
        "the outermost relation is the bare table `a`",
    );
    assert_eq!(
        table.joins.len(),
        1,
        "the outer JOIN is the only top-level join",
    );
    let JoinOperator::Inner {
        constraint: JoinConstraint::On { .. },
        ..
    } = &table.joins[0].operator
    else {
        panic!("expected the outer join to carry `ON a.id = b.id`");
    };
    let TableFactor::NestedJoin {
        table: inner,
        alias,
        ..
    } = &table.joins[0].relation
    else {
        panic!("expected the outer join's relation to be a NestedJoin wrapping `b JOIN c`");
    };
    assert!(alias.is_none(), "the synthesized nesting carries no alias");
    assert!(
        matches!(inner.relation, TableFactor::Table { .. }),
        "the nested relation is the bare table `b`",
    );
    assert_eq!(inner.joins.len(), 1, "exactly one join nested inside");
    assert!(
        matches!(
            inner.joins[0].operator,
            JoinOperator::Inner {
                constraint: JoinConstraint::On { .. },
                ..
            },
        ),
        "the inner join carries `ON b.id = c.id`",
    );
    assert!(
        matches!(inner.joins[0].relation, TableFactor::Table { .. }),
        "the innermost relation is the bare table `c`",
    );
}

#[test]
fn postgres_special_function_from_clause_matches_pg() {
    // A bare special value function as a FROM table reference (PostgreSQL
    // `func_table: func_expr_windowless`, lowered to a `RangeFunction`
    // wrapping a `SqlValueFunction`) — structurally distinct from an
    // ordinary call (`TableFactor::Function` wraps a `FunctionCall`, which
    // this construct has neither the name nor the parenthesized argument
    // list of), so only accept/reject parity and the render round-trip are
    // asserted, mirroring `Expr::SpecialFunction`'s existing
    // `ExprShape::Unmapped` treatment at the expression-position sibling
    // production.
    for sql in [
        "SELECT * FROM current_date",
        "SELECT * FROM current_timestamp",
        "SELECT * FROM current_user",
    ] {
        assert_accept_reject_parity(sql);
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the special-function FROM item {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("special-function FROM round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

/// PostgreSQL expression extensions (prod-sql-expr-postgres): typecast,
/// subscript/slice, COLLATE, AT TIME ZONE, array/row constructors, and field
/// selection. Kept out of ACCEPT_CORPUS because the neutral structural mapping
/// does not cover these forms yet (they map to `ExprShape::Unmapped`), so only
/// accept/reject parity and the render round-trip are asserted over them.
const PG_EXPRESSION_FORM_CORPUS: &[&str] = &[
    "SELECT a::int",
    "SELECT a::numeric(10, 2)",
    "SELECT a::int + b",
    "SELECT - a::int",
    "SELECT a::int::text",
    "SELECT a[1]",
    "SELECT a[1:2]",
    "SELECT a[1:]",
    "SELECT a[:2]",
    "SELECT a[1][2]",
    "SELECT a COLLATE \"C\"",
    "SELECT a AT TIME ZONE 'UTC'",
    "SELECT a AT TIME ZONE 'UTC' AT TIME ZONE 'Asia/Tokyo'",
    "SELECT ARRAY[1, 2, 3]",
    "SELECT ARRAY[]",
    "SELECT ARRAY(SELECT 1)",
    "SELECT ROW(1, 2)",
    "SELECT ROW()",
    "SELECT (a, b)",
    "SELECT (a).b",
    "SELECT (f(a)).b",
    // Mixed precedence: the cast and subscript bind tighter than COLLATE, which
    // binds tighter than the comparison.
    "SELECT a::text COLLATE \"C\" = b",
    // Parenthesization stress: a looser-binding operand must be wrapped when it
    // is the operand of a tighter postfix operator, so these round-trip only if
    // the renderer derives the parens from the binding-power table (ADR-0008).
    "SELECT (a + b)::int",
    "SELECT (a OR b) COLLATE \"C\"",
    "SELECT (a + b)[1]",
    "SELECT (a + b) AT TIME ZONE 'UTC'",
    "SELECT ((a, b)).f1",
];

#[test]
fn pg_expression_forms_accept_like_squonk() {
    for sql in PG_EXPRESSION_FORM_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_expression_forms_render_round_trip() {
    // The Parenthesized mode `roundtrip` also checks is an independent precedence
    // oracle over the new postfix binding powers (ADR-0008/0014).
    for sql in PG_EXPRESSION_FORM_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the expression form {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("expression-form round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

/// Typed temporal literals (prod-literal-date-time-interval): `DATE`/`TIME`/
/// `TIMESTAMP` constants with optional precision and time zone, and `INTERVAL`
/// constants with the qualifier forms. Like [`PG_EXPRESSION_FORM_CORPUS`], these
/// stay out of the structural corpus — PostgreSQL lowers each to a `TypeCast`,
/// not an `A_Const`, so the neutral mapping reports them as `Unmapped` — so only
/// accept/reject parity and the render round-trip are asserted over them.
const PG_TEMPORAL_LITERAL_CORPUS: &[&str] = &[
    "SELECT DATE '1998-12-01'",
    "SELECT date '1998-12-01'",
    "SELECT TIME '12:00:00'",
    "SELECT TIME WITH TIME ZONE '12:00:00+00'",
    "SELECT TIMESTAMP '2020-01-01 12:00:00'",
    "SELECT TIMESTAMP WITH TIME ZONE '2020-01-01 12:00:00+00'",
    "SELECT TIMESTAMP WITHOUT TIME ZONE '2020-01-01 12:00:00'",
    "SELECT TIMESTAMP(6) '2020-01-01 12:00:00'",
    "SELECT INTERVAL '1 day'",
    "SELECT INTERVAL '90' DAY",
    "SELECT INTERVAL '1-2' YEAR TO MONTH",
    "SELECT INTERVAL '1' DAY TO SECOND",
    "SELECT INTERVAL '1' SECOND(3)",
    // The TPC-H Q1 date arithmetic and its `WHERE`-clause context.
    "SELECT date '1998-12-01' - interval '90' day",
    "SELECT * FROM t WHERE d <= DATE '1998-12-01' - INTERVAL '90' DAY",
    // The value string itself may be an adjacent-string concatenation
    // (prod-literal-adjacent-string-concat): PostgreSQL continues the embedded
    // constant across a newline just like a bare string primary.
    "SELECT DATE '1998'\n'-12-01'",
    "SELECT INTERVAL '1'\n' day'",
    "SELECT TIMESTAMP '2020-01-01'\n' 12:00:00'",
];

#[test]
fn pg_temporal_literals_accept_like_squonk() {
    for sql in PG_TEMPORAL_LITERAL_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_temporal_literals_render_round_trip() {
    // The literal round-trips byte-for-byte from its source span (keyword casing,
    // time zone, and interval qualifier survive), so both render modes `roundtrip`
    // checks re-parse to the same tree (ADR-0008/0014).
    for sql in PG_TEMPORAL_LITERAL_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the temporal literal {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("temporal-literal round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

/// Generalized typed string constants (prod-literal-generic-typed): the prefix
/// `type 'string'` form for arbitrary, non-temporal types — bare aliases
/// (`float8`/`int4`/`bool`), built-in spellings (`real`), a multi-word built-in
/// (`double precision`), and schema-qualified names. PostgreSQL lowers each to a
/// `TypeCast` (identical to the `'..'::type` / `CAST(..)` spellings), but it
/// canonicalizes the type name to its internal spelling (`real` → `float4`,
/// `double precision` → `float8`), which the neutral structural mapping does not
/// reproduce — exactly why the `::`/`CAST` forms also stay out of the structural
/// corpus — so accept/reject parity and the render round-trip are the oracles here.
const PG_TYPED_LITERAL_CORPUS: &[&str] = &[
    "SELECT float8 'NaN'",
    "SELECT float8 '-Infinity'",
    "SELECT real 'Infinity'",
    "SELECT int4 '42'",
    "SELECT bool 'true'",
    "SELECT double precision '1.5'",
    "SELECT pg_catalog.float8 'NaN'",
    // The value string itself may be an adjacent-string concatenation
    // (prod-literal-adjacent-string-concat): PostgreSQL continues the embedded
    // constant across a newline just like a bare string primary.
    "SELECT float8 'N'\n'aN'",
    // In realistic predicate / projection contexts.
    "SELECT * FROM t WHERE x > float8 'NaN'",
    "SELECT int4 '1' AS one, bool 'f' AS flag",
    // `N'x'` is a typed literal here, not a national string: PostgreSQL has no `N'…'`
    // constant (pg-national-strings-lexing-divergence) — its scanner rewrites a
    // quote-adjacent `[nN]'` to the identifier `nchar`, so `N'x'` is the typed literal
    // `nchar 'x'` (engine-probed against pg_query 6.1.1: a `TypeCast` to `bpchar`). Our
    // parser reads the generalized typed literal `N '…'` (a `Cast` to the type named `N`),
    // which — exactly like `real`→`float4` above — PostgreSQL canonicalizes to a different
    // internal type-name spelling (`nchar`→`bpchar`), so this stays an accept/reject +
    // round-trip case rather than a structural-parity one. Both cases (`N`/`n`) and a
    // predicate position are covered.
    "SELECT N'x'",
    "SELECT n'x'",
    "SELECT * FROM t WHERE a = N'1'",
    // The value position accepts every `Sconst` spelling, not just the plain `'…'`: the
    // escape (`E'…'`), Unicode-escape (`U&'…'`), and dollar-quoted forms all fold to the
    // typed literal (typed-literal-value-sconst-per-engine), pinning that the value gate
    // rejects only the non-`Sconst` bit/national/introducer kinds and never a valid Sconst.
    r"SELECT float8 E'1.5'",
    r"SELECT float8 U&'1.5'",
    "SELECT float8 $$1.5$$",
];

#[test]
fn pg_typed_literals_accept_like_squonk() {
    for sql in PG_TYPED_LITERAL_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

/// The accept/reject boundary both parsers share: a non-string after the type name
/// is not a typed constant (PostgreSQL's `func_name Sconst` requires a *string*),
/// and a same-line second string is the usual adjacency error once the leading
/// string has committed the typed literal.
#[test]
fn pg_typed_literal_rejects_match_postgres() {
    for sql in ["SELECT float8 42", "SELECT float8 'x' 'y'"] {
        assert_accept_reject_parity(sql);
    }
}

/// The typed-literal *value* is an `Sconst`, so a bit-string (`B'…'`/`X'…'`, a
/// `bit`-typed `BCONST`/`XCONST`) in that position is rejected across every head —
/// generalized (`float8`), parameterized (`char(1)`), func-name (`left(1)`),
/// schema-qualified (`pg_catalog.float8`), and the temporal
/// `DATE`/`TIMESTAMP`/`TIME`/`INTERVAL` constants (typed-literal-value-sconst-per-engine).
/// Measured against pg_query 6.1.1: the 18-combo matrix (9 heads × `B'1'`/`X'ab'`) is
/// reject on the engine; our preset over-accepted before the value gate and now agrees.
#[test]
fn pg_typed_literal_value_bit_string_rejects_match_postgres() {
    for sql in [
        "SELECT float8 B'1'",
        "SELECT float8 X'ab'",
        "SELECT char(1) B'1'",
        "SELECT left(1) X'ab'",
        "SELECT pg_catalog.float8 B'1'",
        "SELECT DATE X'ab'",
        "SELECT TIMESTAMP B'1'",
        "SELECT TIME X'ab'",
        "SELECT INTERVAL X'ab'",
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_typed_literals_render_round_trip() {
    // The type name and the string constant round-trip from their spans, so both
    // render modes `roundtrip` checks re-parse to the same tree (the typed constant
    // is a primary and never self-wraps).
    for sql in PG_TYPED_LITERAL_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the typed literal {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("typed-literal round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

/// PostgreSQL special string-literal forms (prod-literal-pg-special-forms):
/// bit-string constants (`B'...'`/`X'...'`), `N'...'` national strings, and
/// `U&'...'` Unicode-escape strings with the optional `UESCAPE 'c'` override. Like
/// the temporal corpus these stay out of the structural comparison (bit strings
/// lower to a `bit`-typed `A_Const` the neutral mapping does not model; the
/// character forms collapse to a plain string whose exact spelling structural
/// equality cannot recover), so accept/reject parity and the render round-trip are
/// the oracles. Only well-formed escapes appear here — malformed `U&'...'` escape
/// bodies are covered separately in
/// [`PG_MALFORMED_UNICODE_ESCAPE_STRING_CORPUS`], and the one still-deferred
/// `UESCAPE` character-legality gap in
/// [`pg_defers_unicode_escape_validation_unlike_postgres`].
const PG_SPECIAL_STRING_LITERAL_CORPUS: &[&str] = &[
    "SELECT B'1010'",
    "SELECT b'1010'",
    "SELECT X'1FF'",
    "SELECT x'1ff'",
    "SELECT N'naive'",
    "SELECT n'naive'",
    r"SELECT U&'\0041'",
    r"SELECT u&'\0041'",
    r"SELECT U&'d\0061t\+000061'",
    r"SELECT U&'\D800\DC00'",
    "SELECT U&'d!0061t!+000061' UESCAPE '!'",
    // In realistic predicate / projection contexts.
    "SELECT * FROM t WHERE flags = B'1010'",
    r"SELECT U&'\0041' AS letter, N'x' AS tag",
];

/// Numeric-literal edge cases PostgreSQL parses as plain numeric constants:
/// exponent/scientific forms, a trailing decimal point, and signed numerics (the
/// sign is a unary operator over the constant, folded identically by both parsers).
const PG_NUMERIC_LITERAL_CORPUS: &[&str] = &[
    "SELECT 1e10",
    "SELECT 1.5e-3",
    "SELECT .5e3",
    "SELECT 1E3",
    "SELECT 1.",
    "SELECT -5",
    "SELECT +5",
];

#[test]
fn pg_special_string_literals_accept_like_squonk() {
    for sql in PG_SPECIAL_STRING_LITERAL_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_numeric_edge_case_literals_accept_like_squonk() {
    for sql in PG_NUMERIC_LITERAL_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_special_literals_render_round_trip() {
    // Each form round-trips byte-for-byte from its span (marker casing, the value
    // body, and the `UESCAPE` clause survive), so both render modes `roundtrip`
    // checks re-parse to the same tree (ADR-0008/0014).
    let corpus = PG_SPECIAL_STRING_LITERAL_CORPUS
        .iter()
        .chain(PG_NUMERIC_LITERAL_CORPUS);
    for sql in corpus {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the special literal {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("special-literal round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

/// SQL-standard adjacent-string concatenation (prod-literal-adjacent-string-concat):
/// string constants separated by whitespace containing a newline are one value in
/// standard SQL and PostgreSQL. Confirmed against the libpg_query oracle: the gap
/// must contain a newline (`\n` or `\r`), every continuation segment must be a plain
/// `'...'` (an `E'`/`U&'`/dollar second segment, or a dollar-quoted first segment, is
/// not a continuation), and a comment in the gap does not continue the string. Like
/// the other special string forms these stay out of the structural corpus (the
/// concatenated value is a plain string whose exact spelling structural equality
/// cannot recover), so accept/reject parity and the render round-trip are the oracles.
const PG_ADJACENT_STRING_CONCAT_CORPUS: &[&str] = &[
    "SELECT 'foo'\n'bar'",
    "SELECT 'foo'\n'bar'\n'baz'", // three segments
    "SELECT E'a'\n'b'",           // a leading escape-string segment
    "SELECT 'a'  \n  'b'",        // spaces around the newline
    "SELECT 'a'\r\n'b'",          // CRLF
    "SELECT 'a'\n\n'b'",          // a blank line between
    "SELECT U&'a'\n'b'",          // a leading Unicode-escape segment
    "SELECT B'1010'\n'0101'",     // bit-string constants continue too
    "SELECT X'1F'\n'2a'",
    "SELECT 'a''b'\n'c'\n'd''e'", // doubled quotes within the segments
    // In a realistic predicate context.
    "SELECT * FROM t WHERE name = 'foo'\n'bar'",
];

/// Adjacent string constants PostgreSQL rejects (confirmed against the oracle), which
/// `squonk` rejects too: same-line adjacency (no newline), a comment in the gap
/// (even one containing a newline), a non-plain continuation segment, and a
/// dollar-quoted first segment.
const PG_ADJACENT_STRING_REJECT_CORPUS: &[&str] = &[
    "SELECT 'foo' 'bar'",          // same line, space only
    "SELECT 'foo'\t'bar'",         // tab only, no newline
    "SELECT 'a'\nE'b'",            // a non-plain continuation segment
    "SELECT 'a'\nU&'b'",           // ditto
    "SELECT 'a'/* c */'b'",        // a comment in the gap
    "SELECT 'a'/* \n */'b'",       // a newline inside a comment does not count
    "SELECT 'a' -- c\n'b'",        // a line comment before the newline
    "SELECT $$a$$\n'b'",           // a dollar-quoted first segment never continues
    "SELECT DATE '1998' '-12-01'", // a temporal value string, same line: also rejected
];

#[test]
fn pg_adjacent_string_concat_accepts_like_squonk() {
    for sql in PG_ADJACENT_STRING_CONCAT_CORPUS {
        assert!(postgres_accepts(sql), "PostgreSQL accepts {sql:?}");
        assert!(squonk_accepts(sql), "squonk accepts {sql:?}");
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_adjacent_string_concat_rejects_like_squonk() {
    for sql in PG_ADJACENT_STRING_REJECT_CORPUS {
        assert!(!postgres_accepts(sql), "PostgreSQL rejects {sql:?}");
        assert!(!squonk_accepts(sql), "squonk rejects {sql:?}");
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_adjacent_string_concat_render_round_trip() {
    // The literal renders its span verbatim — newline and all — so the concatenated
    // constant re-parses to the same multi-segment literal under both render modes
    // `roundtrip` checks (ADR-0006/0008).
    for sql in PG_ADJACENT_STRING_CONCAT_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the adjacent-string concat {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("adjacent-string-concat round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

#[test]
fn pg_validates_uescape_character_eagerly_like_postgres() {
    // eager-validate-unicode-escape-strings-for-oracle-parity closed the escape
    // *body* half of the old unicode-escape divergence (NUL, lone surrogates,
    // out-of-range code points, malformed digits — see
    // [`PG_MALFORMED_UNICODE_ESCAPE_STRING_CORPUS`]), and pg-regress-small-over-accepts
    // closed the remaining half: the `UESCAPE` clause's own character legality
    // (PostgreSQL's `check_uescapechar` — not a hex digit, `+`, a quote, or whitespace)
    // is now validated when the clause is consumed, so both engines reject an illegal
    // delimiter at parse time and accept/reject parity holds for the whole surface.
    let sql = "SELECT U&'x' UESCAPE 'a'"; // a hex digit is an illegal escape character
    assert!(
        !squonk_accepts(sql),
        "squonk rejects the illegal UESCAPE delimiter in {sql:?} at parse time",
    );
    assert!(
        !postgres_accepts(sql),
        "PostgreSQL rejects {sql:?} at parse time",
    );
}

/// Malformed `U&'...'` Unicode-escape strings PostgreSQL rejects while parsing
/// (confirmed against the libpg_query oracle): an escape decoding to NUL, an
/// unpaired high or low surrogate, a code point above U+10FFFF, non-hex escape
/// digits, and a dangling trailing escape. `squonk` rejects these at parse time
/// too (eager-validate-unicode-escape-strings-for-oracle-parity), and the `UESCAPE`
/// delimiter's own legality is likewise eager now
/// ([`pg_validates_uescape_character_eagerly_like_postgres`]), so accept/reject
/// parity holds for the whole unicode-escape surface.
const PG_MALFORMED_UNICODE_ESCAPE_STRING_CORPUS: &[&str] = &[
    r"SELECT U&'\0000'",    // escape decoding to NUL
    r"SELECT U&'\D800'",    // lone (unpaired) high surrogate
    r"SELECT U&'\DC00'",    // lone (unpaired) low surrogate
    r"SELECT U&'\+110000'", // code point above U+10FFFF
    r"SELECT U&'\XYZW'",    // non-hex escape digits
    r"SELECT U&'\'",        // dangling trailing escape
];

#[test]
fn pg_rejects_malformed_unicode_escape_strings_like_squonk() {
    for sql in PG_MALFORMED_UNICODE_ESCAPE_STRING_CORPUS {
        assert!(!postgres_accepts(sql), "PostgreSQL rejects {sql:?}");
        assert!(!squonk_accepts(sql), "squonk rejects {sql:?}");
        assert_accept_reject_parity(sql);
    }
}

/// Malformed PostgreSQL escape strings (`E'...'`) the real parser rejects while
/// lexing (confirmed against the libpg_query oracle): a short or out-of-range
/// Unicode escape, an escape decoding to NUL, and a byte escape that does not form
/// valid UTF-8. Unlike the still-deferred `U&'...'` forms above,
/// `squonk` now rejects these at parse time too
/// (prod-literal-pg-escape-validation), so accept/reject parity holds.
const PG_MALFORMED_ESCAPE_STRING_CORPUS: &[&str] = &[
    r"SELECT E'\u12'",       // \u with too few hex digits
    r"SELECT E'\u'",         // \u with no hex digits
    r"SELECT E'\u006'",      // \u with three hex digits
    r"SELECT E'\U0000006'",  // \U with seven hex digits
    r"SELECT E'\uD800'",     // lone surrogate code point
    r"SELECT E'\U00110000'", // code point above U+10FFFF
    r"SELECT E'\0'",         // octal escape decoding to NUL
    r"SELECT E'\x00'",       // hex escape decoding to NUL
    r"SELECT E'\U00000000'", // Unicode escape decoding to NUL
    r"SELECT E'\377'",       // octal 0xFF: not valid UTF-8
    r"SELECT E'\xff'",       // hex 0xFF: not valid UTF-8
    r"SELECT E'\xc3'",       // truncated two-byte UTF-8 sequence
    r"SELECT E'\xc3a'",      // 0xC3 then a non-continuation byte
];

/// Escape strings the real parser accepts: an unknown escape collapses to its
/// literal character, a bare `\x` is a literal `x`, and well-formed byte/Unicode
/// escapes decode normally — the accept side of the same parse-time boundary.
const PG_ACCEPTED_ESCAPE_STRING_CORPUS: &[&str] = &[
    r"SELECT E'\q'",
    r"SELECT E'\x'",
    r"SELECT E'\xg'",
    r"SELECT E'\q\x'",
    r"SELECT E'\xc3\xa9'",
    r"SELECT E'\141\x62c\U00000064'",
    r"SELECT E'\b\f\n\r\t'",
];

#[test]
fn pg_rejects_malformed_escape_strings_like_squonk() {
    for sql in PG_MALFORMED_ESCAPE_STRING_CORPUS {
        assert!(!postgres_accepts(sql), "PostgreSQL rejects {sql:?}");
        assert!(!squonk_accepts(sql), "squonk rejects {sql:?}");
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_accepts_unknown_and_valid_escape_strings_like_squonk() {
    for sql in PG_ACCEPTED_ESCAPE_STRING_CORPUS {
        assert!(postgres_accepts(sql), "PostgreSQL accepts {sql:?}");
        assert!(squonk_accepts(sql), "squonk accepts {sql:?}");
        assert_accept_reject_parity(sql);
    }
}

/// String literals embedding a raw NUL byte (0x00) in their source, in every form
/// (ordinary, `E'…'`, `N'…'`, `U&'…'`, dollar-quoted). PostgreSQL rejects all of
/// these while parsing: a query reaches the server as a NUL-terminated C string, so
/// libpg_query rejects any interior `0x00` (confirmed against the oracle — it
/// surfaces as a `NulError` conversion). This is the raw-byte case, distinct from
/// an *escape* that decodes to NUL above (`E'\x00'`); `squonk` now rejects it at
/// parse time too (reject-literal-nul-byte-in-string-literals-...), so accept/reject
/// parity holds. These cannot be raw string literals — a raw NUL byte is written
/// with `\0`.
const PG_NUL_STRING_REJECT_CORPUS: &[&str] = &[
    "SELECT 'a\0b'",         // ordinary
    "SELECT '\0'",           // ordinary, lone NUL
    "SELECT E'a\0b'",        // escape string, raw NUL byte (not a `\x00` escape)
    "SELECT N'a\0b'",        // national string
    "SELECT U&'a\0b'",       // unicode-escape string
    "SELECT $$a\0b$$",       // dollar-quoted
    "SELECT $tag$a\0b$tag$", // tagged dollar-quote
];

#[test]
fn pg_rejects_nul_byte_string_literals_like_squonk() {
    for sql in PG_NUL_STRING_REJECT_CORPUS {
        assert!(!postgres_accepts(sql), "PostgreSQL rejects {sql:?}");
        assert!(!squonk_accepts(sql), "squonk rejects {sql:?}");
        assert_accept_reject_parity(sql);
    }
}

/// Quoted identifiers embedding a raw NUL byte (0x00) in their source. PostgreSQL
/// enables only the standard `"…"` identifier quote and rejects a NUL there the same
/// way it rejects one in a string literal: a query reaches the server as a
/// NUL-terminated C string, so libpg_query rejects any interior `0x00` (confirmed
/// against the oracle — it surfaces as a `NulError` conversion). `squonk` now
/// rejects it at lex time too (reject-raw-nul-byte-in-quoted-identifiers-...), as the
/// dedicated `NulByteInIdentifier` lexical error, so accept/reject parity holds. The
/// dialect `[…]`/backtick quote forms are not PostgreSQL identifier quotes — it
/// rejects them for unrelated syntax reasons — so they are exercised by
/// tokenizer-level tests under presets that enable them, not here. A raw NUL byte
/// cannot be written in a Rust raw string, so these use `\0` escapes.
const PG_NUL_QUOTED_IDENT_REJECT_CORPUS: &[&str] = &[
    "SELECT \"a\0b\"", // standard double-quoted identifier
    "SELECT \"\0\"",   // lone NUL
];

#[test]
fn pg_rejects_nul_byte_quoted_identifiers_like_squonk() {
    for sql in PG_NUL_QUOTED_IDENT_REJECT_CORPUS {
        assert!(!postgres_accepts(sql), "PostgreSQL rejects {sql:?}");
        assert!(!squonk_accepts(sql), "squonk rejects {sql:?}");
        assert_accept_reject_parity(sql);
    }
}

/// Comments embedding a raw NUL byte (0x00). PostgreSQL rejects a NUL *anywhere* in a
/// query — a `--`/`/* */` comment included — via the same NUL-terminated-C-string
/// boundary that rejects one in a string literal or quoted identifier (confirmed against
/// the oracle: the NUL surfaces as a `NulError` conversion before libpg_query parses).
/// `squonk` now rejects it at lex time too, as the dedicated `NulByteInComment`
/// lexical error (fuzz-pg-differential-crash-2b8d66f9), completing the per-lexeme NUL gate
/// that previously covered only the value-bearing lexemes and leaked a NUL inside a
/// skipped comment — so accept/reject parity holds. The bare `--\0-` case is the minimized
/// raw-byte differential fuzz reproducer. A raw NUL cannot be written in a Rust raw string,
/// so these use `\0` escapes.
const PG_NUL_COMMENT_REJECT_CORPUS: &[&str] = &[
    "--\0-",               // the minimized fuzz reproducer: a line comment to EOF
    "-- a\0b",             // `--` line comment, NUL mid-body
    "-- a\0b\nSELECT 1",   // NUL before the comment-ending newline
    "SELECT 1 /* a\0b */", // `/* … */` block comment after a statement
    "/* a /* \0 */ b */",  // NUL in a nested block comment
];

#[test]
fn pg_rejects_nul_byte_comments_like_squonk() {
    for sql in PG_NUL_COMMENT_REJECT_CORPUS {
        assert!(!postgres_accepts(sql), "PostgreSQL rejects {sql:?}");
        assert!(!squonk_accepts(sql), "squonk rejects {sql:?}");
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_create_ddl_render_round_trips() {
    // The Parenthesized mode `roundtrip` also checks is an independent precedence
    // oracle over the query body and the index key/partial-predicate expressions.
    for sql in CREATE_SCHEMA_VIEW_INDEX_ACCEPT_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the schema/view/index DDL {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("create-DDL round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

#[test]
fn pg_accepts_transaction_and_dcl_statements_like_squonk() {
    // Transaction-control and DCL statements parse identically under the real
    // PostgreSQL-17 parser and ours. Kept out of ACCEPT_CORPUS because it predates
    // their structural mapping; the shaped subset now also has dedicated structural
    // parity (`pg_structural_parity_for_transaction_and_session` /
    // `_for_access_control`), while the special `SET` subforms here stay accept-only.
    for sql in [
        "BEGIN",
        "BEGIN TRANSACTION",
        "START TRANSACTION ISOLATION LEVEL SERIALIZABLE READ ONLY",
        "COMMIT",
        "ROLLBACK",
        "SAVEPOINT sp1",
        "ROLLBACK TO SAVEPOINT sp1",
        "RELEASE SAVEPOINT sp1",
        "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
        // The `[NOT] DEFERRABLE` transaction mode on START / SET TRANSACTION.
        "START TRANSACTION READ ONLY, DEFERRABLE",
        "SET TRANSACTION NOT DEFERRABLE",
        "SET search_path TO public",
        "SET search_path = public",
        "SET LOCAL statement_timeout TO 100",
        // A signed numeric SET value (PostgreSQL `NumericOnly`).
        "SET x = -1",
        // Special-cased SET subforms PostgreSQL shares (its `SET NAMES` takes
        // only a string/DEFAULT, so the MySQL bareword/COLLATE form is excluded).
        "SET TIME ZONE 'UTC'",
        "SET TIME ZONE LOCAL",
        "SET LOCAL TIME ZONE DEFAULT",
        "SET ROLE admin",
        "SET ROLE NONE",
        "SET SESSION AUTHORIZATION admin",
        "SET SESSION AUTHORIZATION DEFAULT",
        "SET CONSTRAINTS ALL DEFERRED",
        "SET CONSTRAINTS a, b IMMEDIATE",
        "SET NAMES 'utf8'",
        "SET NAMES DEFAULT",
        "SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL SERIALIZABLE, READ ONLY",
        "RESET ALL",
        "RESET search_path",
        "SHOW ALL",
        "SHOW search_path",
        "GRANT SELECT, INSERT ON t TO alice",
        "GRANT ALL PRIVILEGES ON TABLE t TO alice WITH GRANT OPTION",
        "GRANT SELECT (a, b) ON t TO alice",
        "REVOKE SELECT ON t FROM alice",
        "REVOKE GRANT OPTION FOR INSERT ON t FROM alice",
        // The trailing `<drop behavior>` on a REVOKE, including on a non-table
        // object (close-pg-verdict-ddl-tail-gaps).
        "REVOKE DELETE ON SCHEMA finance FROM bob CASCADE",
        "REVOKE SELECT ON t FROM alice RESTRICT",
        // Non-table privilege kinds and the arbitrary-identifier privilege escape
        // (PostgreSQL's parser accepts any identifier in privilege position).
        "GRANT USAGE, EXECUTE ON SCHEMA s TO alice",
        "GRANT TEMPORARY ON DATABASE d TO alice",
        "GRANT mypriv ON t TO alice",
        // Object-type matrix beyond TABLE.
        "GRANT USAGE ON SEQUENCE s TO alice",
        "GRANT EXECUTE ON FUNCTION f(integer, text) TO alice",
        "GRANT EXECUTE ON PROCEDURE p TO alice",
        "GRANT USAGE ON FOREIGN DATA WRAPPER w TO alice",
        "GRANT USAGE ON FOREIGN SERVER srv TO alice",
        "GRANT SELECT ON ALL TABLES IN SCHEMA s TO alice",
        "GRANT ALL ON DOMAIN d TO alice",
        // Grantee kinds and the GRANTED BY / WITH GRANT OPTION trailers.
        "GRANT SELECT ON t TO PUBLIC, GROUP admins GRANTED BY CURRENT_USER",
        // Role-membership grants, WITH ADMIN OPTION, ADMIN OPTION FOR.
        "GRANT admin, staff TO alice, bob WITH ADMIN OPTION",
        "REVOKE admin FROM alice",
        "REVOKE ADMIN OPTION FOR admin FROM bob",
        "REVOKE admin FROM alice CASCADE",
        // PostgreSQL parses `GRANT SELECT TO alice` (no `ON`) as a role-membership
        // grant whose granted role is spelled like a privilege keyword; ours now
        // agrees rather than diverging.
        "GRANT SELECT TO alice",
    ] {
        assert_accept_reject_parity(sql);
    }
}

/// The DDL-tail cluster the vendored-corpus verdict oracle surfaced
/// (close-pg-verdict-ddl-tail-gaps): `CREATE FUNCTION`/`DATABASE`, the routine and
/// materialized-view `DROP` sub-forms, `ALTER TABLE RENAME`, and the `DEFERRABLE`
/// constraint characteristics. Accept-only, like the DCL statements above:
/// `CREATE FUNCTION`/`DATABASE`/`DROP ROUTINE` have no structural shape mapping,
/// and PostgreSQL lowers `ALTER … RENAME` to a distinct `RenameStmt`, so these
/// stay out of the structure-comparing corpora.
#[test]
fn pg_accepts_ddl_tail_statements_like_squonk() {
    for sql in DDL_TAIL_ACCEPT_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

/// Every [`DDL_TAIL_ACCEPT_CORPUS`] statement re-parses to the same tree after a
/// render, pinning the round-trip of the new nodes and their clauses.
#[test]
fn pg_ddl_tail_render_round_trips() {
    for sql in DDL_TAIL_ACCEPT_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the DDL-tail statement {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("DDL-tail round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

const DDL_TAIL_ACCEPT_CORPUS: &[&str] = &[
    // CREATE FUNCTION: the four corpus forms plus the OR REPLACE, STRICT, and
    // RETURNS-NULL-ON-NULL-INPUT option paths.
    "CREATE FUNCTION a(b INT, c VARCHAR) AS 'SELECT 1'",
    "CREATE FUNCTION a() LANGUAGE sql",
    "CREATE FUNCTION a(x INT) RETURNS INT LANGUAGE SQL CALLED ON NULL INPUT AS 'SELECT 1'",
    "CREATE FUNCTION a.b.c()",
    "CREATE OR REPLACE FUNCTION a() LANGUAGE sql",
    "CREATE FUNCTION a(x INT) RETURNS INT LANGUAGE sql STRICT AS 'SELECT 1'",
    "CREATE FUNCTION a() LANGUAGE sql RETURNS NULL ON NULL INPUT AS 'SELECT 1'",
    // The dollar-quoted body: the `AS` option carries a `FunctionBody::Definition` holding
    // the source `Literal` (the tokenizer already lexes `$$...$$`/`$tag$...$tag$` as a string
    // token), so the delimiter tag and verbatim body text round-trip from the span rather
    // than through a bespoke single-quoted-only body. The `AS`-before-`LANGUAGE` case pins
    // that the body stays an order-independent option (its written position is preserved).
    "CREATE FUNCTION a() LANGUAGE sql AS $$SELECT 1$$",
    "CREATE FUNCTION a() AS $$SELECT 1$$ LANGUAGE sql",
    "CREATE FUNCTION a() LANGUAGE plpgsql AS $body$ BEGIN RETURN 1; END; $body$",
    // Parameter defaults (`func_arg_with_default`): both the `DEFAULT` keyword and the `=`
    // spellings of PostgreSQL's one production. The `FunctionParamDefaultSpelling` tag records
    // which the source used, so a round-trip preserves `DEFAULT` vs `=` verbatim rather than
    // normalizing to one form (sqlparser-rs collapses both to `=`). The default value is a live
    // `Expr`, so a compound default (`1 + 2`) and a string default round-trip through the
    // expression grammar. These ride the `routine_arg_defaults` gate (off for MySQL, which has
    // no parameter defaults).
    "CREATE FUNCTION a(b INT DEFAULT 0) LANGUAGE sql",
    "CREATE FUNCTION a(b INT = 0) LANGUAGE sql",
    "CREATE FUNCTION a(b INT DEFAULT 1 + 2, c TEXT DEFAULT 'x') RETURNS INT LANGUAGE sql",
    "CREATE FUNCTION a(b INT, c INT DEFAULT 5) RETURNS INT LANGUAGE sql AS 'SELECT 1'",
    // SQL-standard `RETURN <expr>` body (`opt_routine_body`, PostgreSQL 14+): a live SQL
    // expression on the trailing `CreateFunction::body` slot, disjoint from the order-independent
    // `AS` string body. The trailing slot strictly follows the whole option list — `LANGUAGE sql
    // RETURN 1` accepts (option then body) but `RETURN 1 LANGUAGE sql` does not (pinned as a
    // reject below), so the position is grammatical, not just a written-order preference.
    "CREATE FUNCTION a() RETURNS INT RETURN 1",
    "CREATE FUNCTION a(b INT) RETURNS INT RETURN b + 1",
    "CREATE FUNCTION a() RETURNS INT LANGUAGE sql RETURN 1",
    "CREATE OR REPLACE FUNCTION a() RETURNS INT RETURN 42",
    // CREATE DATABASE.
    "CREATE DATABASE x",
    // Routine and materialized-view DROP sub-forms, plus IF EXISTS / behaviour /
    // ROUTINE spellings.
    "DROP FUNCTION a.b.c (INT)",
    "DROP PROCEDURE a.b.c (INT)",
    "DROP MATERIALIZED VIEW x.y.z",
    "DROP FUNCTION IF EXISTS f (INT, TEXT) CASCADE",
    "DROP ROUTINE r",
    // ALTER TABLE RENAME.
    "ALTER TABLE table1 RENAME TO table2",
    "ALTER TABLE table1 RENAME COLUMN c1 TO c2",
    // DEFERRABLE / INITIALLY constraint characteristics.
    "CREATE TABLE foo (baz_id INT REFERENCES baz (id) DEFERRABLE)",
    "ALTER TABLE ct ADD CONSTRAINT ct_id_fk FOREIGN KEY (id) REFERENCES et (fid) DEFERRABLE INITIALLY DEFERRED",
    "CREATE TABLE t (a INT UNIQUE NOT DEFERRABLE INITIALLY IMMEDIATE)",
];

#[test]
fn pg_rejects_malformed_transaction_and_dcl_statements_like_squonk() {
    // `GRANT SELECT TO alice` now parses for both as a role-membership grant (see
    // the accept corpus); the historical divergence is gone.
    for sql in [
        "SAVEPOINT",                                // missing savepoint name
        "RELEASE",                                  // missing savepoint name
        "GRANT SELECT ON t",                        // missing TO grantees
        "REVOKE SELECT ON t",                       // missing FROM grantees
        "GRANT ALL TO alice",                       // `ALL` requires `ON <object>`
        "REVOKE GRANT OPTION FOR admin FROM alice", // GRANT OPTION FOR needs `ON`
        "SET TIME ZONE",                            // TIME ZONE needs a value
        "SET CONSTRAINTS ALL",                      // missing DEFERRED / IMMEDIATE
    ] {
        assert_accept_reject_parity(sql);
    }
}

const COPY_EXPLAIN_ACCEPT_CORPUS: &[&str] = &[
    // COPY: table form, both directions, every endpoint, with/without options.
    "COPY t TO '/tmp/out.csv'",
    "COPY t FROM '/tmp/in.csv'",
    "COPY t (a, b) TO '/tmp/out.csv'",
    "COPY t FROM STDIN",
    "COPY t TO STDOUT",
    // PostgreSQL's grammar admits STDIN/STDOUT with either direction (the
    // mismatch is a later semantic check), so both cross pairings parse.
    "COPY t TO STDIN",
    "COPY t FROM STDOUT",
    "COPY t TO PROGRAM 'gzip > /tmp/out.gz'",
    "COPY t TO '/tmp/out.csv' WITH (FORMAT csv)",
    "COPY t TO '/tmp/out.csv' (FORMAT csv, HEADER)",
    "COPY t FROM STDIN WITH (FORMAT csv, HEADER true, DELIMITER ',')",
    // Generic parenthesized option-value shapes beyond the bareword/string forms
    // (PostgreSQL `copy_generic_opt_arg`): a numeric argument (incl. sign-folded and
    // decimal), the bare `*`, and a parenthesized argument list. These are the
    // DuckDB/Snowflake-parity file-format/option surfaces (`ROW_GROUP_SIZE 100000`,
    // `FORCE_QUOTE (a, b)`), all accepted by pg_query under the plain COPY gate.
    "COPY t TO '/tmp/out.csv' (FORMAT csv, HEADER 1)",
    "COPY t TO '/tmp/out.csv' (FORMAT csv, HEADER -1)",
    "COPY t TO '/tmp/out.csv' (FORMAT csv, FOO 1.5)",
    "COPY t TO '/tmp/out.csv' (FORMAT csv, ROW_GROUP_SIZE 100000)",
    "COPY t TO '/tmp/out.csv' (FORMAT csv, FORCE_QUOTE (a, b))",
    "COPY t TO '/tmp/out.csv' (FORMAT csv, FORCE_QUOTE *)",
    "COPY t FROM STDIN (FORMAT csv, FORCE_NOT_NULL (a, b))",
    "COPY t TO '/tmp/out.csv' (FORMAT csv, COMPRESSION 'zstd', PARTITION_BY (a, b))",
    "COPY public.t (a) FROM STDIN",
    // COPY query form: a parenthesized PreparableStmt, TO-only. SELECT/VALUES/
    // WITH plus DML-with-RETURNING are all valid inner sources, with every
    // endpoint and the option spellings.
    "COPY (SELECT 1) TO STDOUT",
    "COPY (SELECT a, b FROM t WHERE a > 0) TO '/tmp/out.csv'",
    "COPY (VALUES (1), (2)) TO STDOUT",
    "COPY (WITH x AS (SELECT 1) SELECT * FROM x) TO STDOUT",
    "COPY (INSERT INTO t VALUES (1) RETURNING *) TO STDOUT",
    "COPY (UPDATE t SET a = 1 RETURNING *) TO STDOUT",
    "COPY (DELETE FROM t RETURNING *) TO STDOUT",
    "COPY (SELECT 1) TO PROGRAM 'cat'",
    "COPY (SELECT 1) TO STDOUT WITH (FORMAT csv)",
    "COPY (SELECT 1) TO STDOUT CSV",
    // COPY legacy un-parenthesized options: bare keywords, the `[AS] '<str>'`
    // string forms, `ENCODING`, with and without the optional `WITH`, and on
    // both directions and the column-list/query sources.
    "COPY t TO '/tmp/out.csv' CSV",
    "COPY t TO '/tmp/out.csv' WITH CSV",
    "COPY t TO '/tmp/out.csv' BINARY",
    "COPY t TO '/tmp/out.csv' CSV HEADER",
    "COPY t FROM '/tmp/in.csv' DELIMITER ','",
    "COPY t FROM '/tmp/in.csv' WITH DELIMITER ','",
    "COPY t FROM '/tmp/in.csv' DELIMITER AS ','",
    "COPY t TO '/tmp/out.csv' NULL ''",
    "COPY t TO '/tmp/out.csv' QUOTE '\"' ESCAPE '\\'",
    "COPY t TO '/tmp/out.csv' ENCODING 'UTF8'",
    "COPY t TO '/tmp/out.csv' FREEZE",
    "COPY t (a, b) TO '/tmp/out.csv' CSV HEADER",
    "COPY t TO '/tmp/out.csv' CSV HEADER DELIMITER ',' NULL ''",
    "COPY (SELECT 1) TO STDOUT WITH CSV",
    // COPY remaining legacy surfaces: the compound-keyword FORCE options with a
    // column list or `*`, the `opt_binary` prefix, the `[USING] DELIMITERS`
    // clause, and the COPY FROM ... WHERE filter.
    "COPY t TO '/tmp/out.csv' CSV FORCE QUOTE a",
    "COPY t TO '/tmp/out.csv' CSV FORCE QUOTE a, b",
    "COPY t TO '/tmp/out.csv' CSV FORCE QUOTE *",
    "COPY t FROM '/tmp/in.csv' CSV FORCE NOT NULL a, b",
    "COPY t FROM '/tmp/in.csv' CSV FORCE NOT NULL *",
    "COPY t FROM '/tmp/in.csv' CSV FORCE NULL a",
    "COPY t FROM '/tmp/in.csv' CSV FORCE NULL *",
    "COPY BINARY t TO STDOUT",
    "COPY BINARY t (a, b) FROM STDIN",
    "COPY t FROM '/tmp/in.csv' USING DELIMITERS ','",
    "COPY t FROM '/tmp/in.csv' DELIMITERS ','",
    "COPY t FROM '/tmp/in.csv' DELIMITERS ',' CSV HEADER",
    "COPY t FROM STDIN WHERE a > 1",
    "COPY t FROM '/tmp/in.csv' CSV WHERE a > 0 AND b < 10",
    "COPY BINARY t FROM STDIN USING DELIMITERS ',' WHERE a > 1",
    // EXPLAIN: bare, the legacy ANALYZE/VERBOSE prefix, and the option list.
    "EXPLAIN SELECT 1",
    "EXPLAIN ANALYZE SELECT 1",
    "EXPLAIN VERBOSE SELECT 1",
    "EXPLAIN ANALYZE VERBOSE SELECT 1",
    "EXPLAIN (ANALYZE) SELECT 1",
    "EXPLAIN (ANALYZE, VERBOSE) SELECT 1",
    "EXPLAIN (FORMAT JSON) SELECT 1",
    "EXPLAIN (ANALYZE, BUFFERS) SELECT 1",
    "EXPLAIN (COSTS off, TIMING off) SELECT 1",
    "EXPLAIN INSERT INTO t VALUES (1)",
    "EXPLAIN UPDATE t SET a = 1",
    "EXPLAIN DELETE FROM t",
];

#[test]
fn pg_accepts_copy_and_explain_statements_like_squonk() {
    // COPY/EXPLAIN parse identically under the real PostgreSQL-17 parser and ours.
    // Kept out of ACCEPT_CORPUS because it predates their mapping: EXPLAIN now has
    // dedicated structural parity (`pg_structural_parity_for_explain`), while COPY
    // stays accept-only by decision — its option soup maps to an explicit "not
    // implemented" divergence rather than silent parity (see `StatementShape`).
    for sql in COPY_EXPLAIN_ACCEPT_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_rejects_malformed_copy_and_explain_statements_like_squonk() {
    for sql in [
        "COPY",                                    // missing table
        "COPY t",                                  // missing FROM/TO
        "COPY t FROM",                             // missing source
        "COPY t TO",                               // missing target
        "COPY (SELECT 1) FROM STDIN",              // the query form is TO-only
        "COPY (CREATE TABLE x (a int)) TO STDOUT", // inner must be a preparable query
        "COPY t TO 'f' BOGUS",                     // unknown legacy option keyword
        "COPY t TO 'f' CSV BOGUS",                 // trailing unknown legacy option
        "COPY t TO 'f' ENCODING AS 'UTF8'",        // ENCODING admits no `AS`
        "COPY t TO STDOUT WHERE a > 1",            // WHERE not allowed with COPY TO
        "EXPLAIN",                                 // missing inner statement
        "EXPLAIN () SELECT 1",                     // empty option list
    ] {
        assert_accept_reject_parity(sql);
    }
}

#[test]
fn pg_copy_and_explain_render_round_trips() {
    // Both render modes `roundtrip` checks re-parse to the same tree: the option
    // lists and inner statement round-trip, and the canonicalized `WITH (...)` /
    // parenthesized option spelling re-parses to the same options (the surface
    // `WITH` is not load-bearing once the list is captured).
    for sql in COPY_EXPLAIN_ACCEPT_CORPUS {
        match crate::corpus_roundtrip::roundtrip(sql, Postgres) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("Postgres should parse the COPY/EXPLAIN statement {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(message) => {
                panic!("COPY/EXPLAIN round-trip failed for {sql:?}: {message}")
            }
        }
    }
}

// ---- structural parity for the transaction / session / DCL / EXPLAIN families
// (pg-structural-oracle-for-dcl-tcl-utility). These families reach structural
// parity, not merely the accept/reject + round-trip of the corpora above, closing
// the same oracle-hole class that hid the ROLLUP/CUBE mis-parse but for whole
// statement families. `COPY` and the special `SET` subforms stay accept-only by
// decision (see `StatementShape`), so they are absent here.

/// Transaction-control and the generic `SET`/`RESET`/`SHOW` session forms.
const TCL_SHAPE_CORPUS: &[&str] = &[
    "BEGIN",
    "BEGIN TRANSACTION",
    "START TRANSACTION",
    "START TRANSACTION ISOLATION LEVEL SERIALIZABLE READ ONLY",
    "START TRANSACTION READ ONLY, DEFERRABLE",
    "START TRANSACTION READ WRITE",
    "START TRANSACTION ISOLATION LEVEL READ UNCOMMITTED",
    "START TRANSACTION ISOLATION LEVEL REPEATABLE READ",
    "COMMIT",
    "ROLLBACK",
    "SAVEPOINT sp1",
    "ROLLBACK TO SAVEPOINT sp1",
    "RELEASE SAVEPOINT sp1",
    "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
    "SET TRANSACTION READ ONLY",
    "SET TRANSACTION NOT DEFERRABLE",
    // Generic SET / RESET / SHOW: the string, bareword (both fold to one string
    // constant), signed-integer, float, multi-value, and LOCAL-scope forms.
    "SET search_path TO public",
    "SET search_path = public",
    "SET search_path TO public, pg_catalog",
    "SET LOCAL statement_timeout TO 100",
    "SET x = -1",
    "SET x = 1.5",
    "SET x = 'foo'",
    "SET x = foo",
    "RESET ALL",
    "RESET search_path",
    "SHOW ALL",
    "SHOW search_path",
];

/// `GRANT`/`REVOKE`: the privilege/role directions, the object-type matrix, the
/// column and routine-signature scopes, the grantee kinds, and the option/behaviour
/// trailers.
const DCL_SHAPE_CORPUS: &[&str] = &[
    "GRANT SELECT, INSERT ON t TO alice",
    "GRANT ALL PRIVILEGES ON TABLE t TO alice WITH GRANT OPTION",
    "GRANT SELECT (a, b) ON t TO alice",
    "GRANT SELECT ON t, u TO alice",
    "REVOKE SELECT ON t FROM alice",
    "REVOKE GRANT OPTION FOR INSERT ON t FROM alice",
    "REVOKE DELETE ON SCHEMA finance FROM bob CASCADE",
    "REVOKE SELECT ON t FROM alice RESTRICT",
    "GRANT USAGE, EXECUTE ON SCHEMA s TO alice",
    "GRANT TEMPORARY ON DATABASE d TO alice",
    "GRANT mypriv ON t TO alice",
    "GRANT USAGE ON SEQUENCE s TO alice",
    "GRANT EXECUTE ON FUNCTION f(integer, text) TO alice",
    "GRANT EXECUTE ON PROCEDURE p TO alice",
    "GRANT EXECUTE ON ROUTINE r TO alice",
    "GRANT USAGE ON FOREIGN DATA WRAPPER w TO alice",
    "GRANT USAGE ON FOREIGN SERVER srv TO alice",
    "GRANT USAGE ON LANGUAGE plpgsql TO alice",
    "GRANT ALL ON TABLESPACE ts TO alice",
    "GRANT USAGE ON TYPE ty TO alice",
    "GRANT ALL ON DOMAIN d TO alice",
    "GRANT SELECT ON ALL TABLES IN SCHEMA s TO alice",
    "GRANT SELECT ON ALL SEQUENCES IN SCHEMA s TO alice",
    "GRANT SELECT ON t TO PUBLIC, GROUP admins GRANTED BY CURRENT_USER",
    "GRANT admin, staff TO alice, bob WITH ADMIN OPTION",
    "REVOKE admin FROM alice",
    "REVOKE ADMIN OPTION FOR admin FROM bob",
    "REVOKE admin FROM alice CASCADE",
    // The privilege-keyword-spelled role grant PostgreSQL lowers to a role grant.
    "GRANT SELECT TO alice",
];

/// `EXPLAIN`: the legacy `ANALYZE`/`VERBOSE` prefix, the parenthesized option list,
/// and the DML inner statements (each recursed into its own shape).
const EXPLAIN_SHAPE_CORPUS: &[&str] = &[
    "EXPLAIN SELECT 1",
    "EXPLAIN ANALYZE SELECT 1",
    "EXPLAIN VERBOSE SELECT 1",
    "EXPLAIN ANALYZE VERBOSE SELECT 1",
    "EXPLAIN (ANALYZE) SELECT 1",
    "EXPLAIN (ANALYZE, VERBOSE) SELECT 1",
    "EXPLAIN (FORMAT JSON) SELECT 1",
    "EXPLAIN (ANALYZE, BUFFERS) SELECT 1",
    "EXPLAIN (COSTS off, TIMING off) SELECT 1",
    "EXPLAIN SELECT a FROM t WHERE a > 1",
    "EXPLAIN INSERT INTO t VALUES (1)",
    "EXPLAIN UPDATE t SET a = 1",
    "EXPLAIN DELETE FROM t",
];

#[test]
fn pg_structural_parity_for_transaction_and_session() {
    for sql in TCL_SHAPE_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_parity_for_access_control() {
    for sql in DCL_SHAPE_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_parity_for_explain() {
    for sql in EXPLAIN_SHAPE_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_oracle_catches_dcl_tcl_utility_differences() {
    // The point of the mapping (ADR-0015): a wrong-but-round-trip-stable
    // GRANT/SET/EXPLAIN/transaction tree now maps to a *different* shape than
    // PostgreSQL's, so it can no longer ship green. Each `assert_ne` is a content
    // difference the shape distinguishes; the `assert_eq` against PostgreSQL anchors
    // that our correct parse matches the oracle (so the differing tree fails parity).
    let ours = first_ours_shape;
    let pg = first_pg_shape;

    // GRANT vs REVOKE, the privilege set, the WITH GRANT OPTION trailer, the
    // grantee, the drop behaviour, and the object class are each structural.
    assert_ne!(
        ours("GRANT SELECT ON t TO alice"),
        ours("REVOKE SELECT ON t FROM alice"),
        "GRANT vs REVOKE is structural",
    );
    assert_ne!(
        ours("GRANT SELECT ON t TO alice"),
        ours("GRANT INSERT ON t TO alice"),
        "the privilege is structural",
    );
    assert_ne!(
        ours("GRANT SELECT ON t TO alice"),
        ours("GRANT SELECT ON t TO alice WITH GRANT OPTION"),
        "WITH GRANT OPTION is structural",
    );
    assert_ne!(
        ours("GRANT SELECT ON t TO alice"),
        ours("GRANT SELECT ON t TO bob"),
        "the grantee is structural",
    );
    assert_ne!(
        ours("REVOKE SELECT ON t FROM alice"),
        ours("REVOKE SELECT ON t FROM alice CASCADE"),
        "REVOKE CASCADE is structural",
    );
    assert_ne!(
        ours("GRANT USAGE ON SCHEMA s TO alice"),
        ours("GRANT USAGE ON SEQUENCE s TO alice"),
        "the GRANT object class is structural",
    );
    assert_eq!(
        ours("GRANT SELECT (a, b) ON t TO alice"),
        pg("GRANT SELECT (a, b) ON t TO alice"),
        "our column-scoped GRANT matches PostgreSQL",
    );

    // The SET value and its LOCAL scope, and the transaction isolation level and
    // BEGIN-vs-START spelling, are each structural and match PostgreSQL.
    assert_ne!(
        ours("SET search_path TO public"),
        ours("SET search_path TO other"),
        "the SET value is structural",
    );
    assert_ne!(
        ours("SET statement_timeout TO 100"),
        ours("SET LOCAL statement_timeout TO 100"),
        "the SET LOCAL scope is structural",
    );
    assert_eq!(
        ours("SET search_path TO public"),
        pg("SET search_path TO public"),
        "our SET matches PostgreSQL",
    );
    assert_ne!(
        ours("SET TRANSACTION ISOLATION LEVEL READ COMMITTED"),
        ours("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE"),
        "the transaction isolation level is structural",
    );
    assert_ne!(
        ours("BEGIN"),
        ours("START TRANSACTION"),
        "BEGIN vs START TRANSACTION is structural",
    );

    // The EXPLAIN option set and inner statement are structural and match PostgreSQL.
    assert_ne!(
        ours("EXPLAIN SELECT 1"),
        ours("EXPLAIN ANALYZE SELECT 1"),
        "the EXPLAIN option set is structural",
    );
    assert_ne!(
        ours("EXPLAIN SELECT 1"),
        ours("EXPLAIN SELECT 2"),
        "the EXPLAIN inner statement is structural",
    );
    assert_eq!(
        ours("EXPLAIN ANALYZE SELECT 1"),
        pg("EXPLAIN ANALYZE SELECT 1"),
        "our EXPLAIN matches PostgreSQL",
    );
}

#[test]
fn pg_structural_oracle_catches_explain_inner_misbinding() {
    // EXPLAIN recurses into its inner statement's shape, so a precedence mis-bind
    // inside the explained query maps to a shape different from PostgreSQL's — the
    // oracle sees through the wrapper (ADR-0008/0015).
    let pg = first_pg_shape;
    let ours = first_ours_shape;
    let correct = "EXPLAIN SELECT a + b * c FROM t";
    let misbinding = "EXPLAIN SELECT (a + b) * c FROM t";
    assert_ne!(
        pg(correct),
        pg(misbinding),
        "the mis-bound inner query must map to a different shape",
    );
    assert_eq!(
        ours(correct),
        pg(correct),
        "structural parity on {correct:?}"
    );
    assert_ne!(
        ours(correct),
        pg(misbinding),
        "our parse must not match the mis-binding",
    );
}

#[test]
fn pg_divergence_allowlist_entries_name_existing_tickets_and_still_diverge() {
    for entry in PG_DIVERGENCE_ALLOWLIST {
        assert!(!entry.sql.trim().is_empty(), "allowlist SQL is required");
        assert!(
            !entry.reason.trim().is_empty(),
            "allowlist reason is required for {:?}",
            entry.sql,
        );
        assert!(
            !entry.ticket.trim().is_empty(),
            "allowlist ticket is required for {:?}",
            entry.sql,
        );

        match entry.kind {
            PgDivergenceKind::AcceptReject => assert_ne!(
                postgres_accepts(entry.sql),
                squonk_accepts(entry.sql),
                "stale accept/reject allowlist entry for {:?}: pg_query and squonk now agree, \
                 so the divergence is fixed — SWEEP this entry (delete it from PG_DIVERGENCE_ALLOWLIST), \
                 never re-pin or edit it to keep the parity assert silenced (ADR-0015: a fix forces removal)",
                entry.sql,
            ),
            PgDivergenceKind::Structural => {
                assert!(
                    postgres_accepts(entry.sql) && squonk_accepts(entry.sql),
                    "structural allowlist entry {:?} should be accepted by both parsers",
                    entry.sql,
                );
                assert!(
                    pg_structural_divergence(entry.sql).is_some(),
                    "stale structural allowlist entry for {:?}: the neutral shapes now match, \
                     so the divergence is fixed — SWEEP this entry (delete it from PG_DIVERGENCE_ALLOWLIST), \
                     never re-pin or edit it to keep the structural assert silenced (ADR-0015: a fix forces removal)",
                    entry.sql,
                );
            }
        }
    }
}

#[test]
fn untriaged_accept_reject_divergence_panics() {
    // `LISTEN foo` is accepted by PostgreSQL but has no `squonk` statement (only the
    // `LISTEN` keyword exists for the lexer), so it stays an untriaged accept/reject
    // divergence — exactly what this test needs to prove the parity assert panics rather
    // than passing silently (verified: pg accepts, we reject, not allowlisted, absent
    // from every corpus). The specimen has migrated as each prior gap was closed: the
    // bare `TABLE foo` parses under parse-pg-table-command-and-empty-select, then the
    // plain `FOR UPDATE` under mysql-select-tails-locking-hints-partition, then
    // `SELECT * FROM t FOR NO KEY UPDATE` under this ticket
    // (pg-locking-clause-strengths-and-stacking) — so it moved on to `LISTEN foo`.
    let panic = std::panic::catch_unwind(|| assert_accept_reject_parity("LISTEN foo"))
        .expect_err("untriaged accept/reject divergence should panic");
    let message = panic_message(panic);
    assert!(message.contains("untriaged PostgreSQL accept/reject divergence"));
}

#[test]
fn untriaged_structural_divergence_panics() {
    // A `COPY` statement parses in both engines but is deliberately outside the
    // structural corpus (its option soup stays accept-only, see `StatementShape`),
    // so the mapping reports an explicit "not implemented" divergence — exactly the
    // untriaged case `assert_structural_parity` must turn into a panic. The
    // transaction/session/DCL/EXPLAIN families it once stood in for are now mapped.
    let panic = std::panic::catch_unwind(|| assert_structural_parity("COPY t TO STDOUT"))
        .expect_err("untriaged structural divergence should panic");
    let message = panic_message(panic);
    assert!(message.contains("untriaged PostgreSQL structural"));
}

#[test]
fn pg_structural_parity_for_mapped_select_constructs() {
    for sql in ACCEPT_CORPUS.iter().chain(SUBQUERY_PREDICATE_CORPUS) {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_parity_for_some_quantifier() {
    // `= SOME (subquery)` now reaches *structural* parity, not merely the
    // accept/reject parity of `pg_accepts_some_quantified_comparison_like_squonk`.
    // SOME and ANY are exact SQL synonyms, so PostgreSQL lowers both to one
    // `AnySublink` (`Quantifier::Any`) and `squonk_shape` collapses our
    // render-spelling `Quantifier::Some` to that same `Any` — the two spellings
    // land on a single canonical shape (ADR-0011) instead of diverging.
    let some = "SELECT * FROM t WHERE a = SOME (SELECT b FROM u)";
    let any = "SELECT * FROM t WHERE a = ANY (SELECT b FROM u)";
    assert_structural_parity(some);
    assert_eq!(
        squonk_shape(&parse_with(some, Postgres).expect("squonk parses SOME")),
        squonk_shape(&parse_with(any, Postgres).expect("squonk parses ANY")),
        "SOME and ANY must collapse to one canonical shape",
    );
}

#[test]
fn pg_structural_parity_for_expression_constructs() {
    for sql in EXPRESSION_ACCEPT_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_parity_for_operator_precedence() {
    for sql in PRECEDENCE_ACCEPT_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_oracle_catches_expression_misbinding() {
    // ADR-0008: operator precedence/associativity is the whole point of the
    // structural oracle (ADR-0015). The neutral shape is a binding tree, so a
    // mis-bound parse necessarily maps to a *different* shape than
    // PostgreSQL's — which is what makes the oracle a real precedence check
    // rather than round-trip-blind. Each pair is the correctly-bound SQL vs.
    // the parenthesized form of the *wrong* binding; PostgreSQL's two shapes
    // must differ, and ours must match the correct one (so a regression to the
    // mis-binding would fail structural parity).
    let pg = first_pg_shape;
    let ours = first_ours_shape;

    for (correct, misbinding) in [
        ("SELECT a = b AND c", "SELECT a = (b AND c)"), // `=` binds tighter than AND
        ("SELECT a OR b AND c", "SELECT (a OR b) AND c"), // AND binds tighter than OR
        ("SELECT a + b * c", "SELECT (a + b) * c"),     // `*` binds tighter than `+`
        ("SELECT a - b - c", "SELECT a - (b - c)"),     // `-` is left-associative
        ("SELECT NOT a AND b", "SELECT NOT (a AND b)"), // NOT binds tighter than AND
    ] {
        let correct_shape = pg(correct);
        let misbound_shape = pg(misbinding);
        assert_ne!(
            correct_shape, misbound_shape,
            "the mis-bound grouping must map to a different shape than {correct:?}",
        );
        assert_eq!(
            ours(correct),
            correct_shape,
            "structural parity on {correct:?}"
        );
        assert_ne!(
            ours(correct),
            misbound_shape,
            "our parse of {correct:?} must not match the mis-binding {misbinding:?}",
        );
    }
}

#[test]
fn pg_special_form_function_mapping_gaps_are_explicit() {
    // PostgreSQL lowers these SQL-syntax "functions" to dedicated parse nodes
    // (CoalesceExpr / MinMaxExpr / AEXPR_NULLIF), not the generic FuncCall our
    // parser produces, so this incremental mapping does not yet normalize them.
    // Until it does they must surface as an explicit structural divergence,
    // never silent parity (ADR-0015); plain `FuncCall` functions are mapped and
    // covered by `pg_structural_parity_for_expression_constructs`.
    for sql in [
        "SELECT coalesce(a, b)",
        "SELECT greatest(a, b)",
        "SELECT least(a, b)",
        "SELECT nullif(a, b)",
    ] {
        assert!(
            postgres_accepts(sql) && squonk_accepts(sql),
            "{sql} should parse in both engines for the gap to be structural",
        );
        let divergence = pg_structural_divergence(sql)
            .unwrap_or_else(|| panic!("{sql} should be an explicit structural gap"));
        assert!(
            divergence.contains("unsupported PostgreSQL expression"),
            "{sql} divergence should name the unmapped node, got: {divergence}",
        );
    }
}

#[test]
fn pg_structural_parity_for_advanced_table_expressions() {
    for sql in ADVANCED_TABLE_ACCEPT_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_parity_for_regress_supported_subset() {
    assert_structural_parity(PG_REGRESS_SUPPORTED_SQL);
}

#[test]
fn pg_structural_parity_for_create_table() {
    for sql in CREATE_TABLE_ACCEPT_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_parity_for_alter_and_drop() {
    for sql in ALTER_DROP_ACCEPT_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_parity_for_schema_view_index() {
    for sql in CREATE_SCHEMA_VIEW_INDEX_ACCEPT_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_parity_for_insert() {
    for sql in INSERT_ACCEPT_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_parity_for_update_and_delete() {
    for sql in UPDATE_DELETE_ACCEPT_CORPUS
        .iter()
        .chain(UPDATE_DELETE_ADVANCED_ACCEPT_CORPUS)
    {
        assert_structural_parity(sql);
    }
}

#[test]
fn pg_structural_parity_for_returning_and_on_conflict() {
    for sql in RETURNING_CONFLICT_ACCEPT_CORPUS {
        assert_structural_parity(sql);
    }
}

#[test]
fn structural_shape_detects_ddl_dml_content_differences() {
    let ours = first_ours_shape;

    // A `CHECK` predicate's binding is structural: `id > 0 AND id < 9` must not
    // collapse to the same shape as the mis-bound `id > (0 AND id < 9)`.
    let correct_check = ours("CREATE TABLE t (id INT, CHECK (id > 0 AND id < 9))");
    let pg_check = pg_shape(
        &pg_query::parse("CREATE TABLE t (id INT, CHECK (id > 0 AND id < 9))")
            .expect("pg parses")
            .protobuf,
    )
    .expect("mapped PostgreSQL shape")
    .into_iter()
    .next()
    .expect("one mapped statement");
    assert_eq!(
        correct_check, pg_check,
        "CHECK predicate binds as PostgreSQL"
    );
    assert_ne!(
        correct_check,
        ours("CREATE TABLE t (id INT, CHECK ((id > 0 AND id) < 9))"),
        "a CHECK predicate's binding is structural",
    );

    // Column name, type, and a single constraint flag are each structural.
    assert_ne!(
        ours("CREATE TABLE t (a INT)"),
        ours("CREATE TABLE t (b INT)"),
        "column names are structural",
    );
    assert_ne!(
        ours("CREATE TABLE t (a INT)"),
        ours("CREATE TABLE t (a TEXT)"),
        "column types are structural",
    );
    assert_ne!(
        ours("CREATE TABLE t (a INT)"),
        ours("CREATE TABLE t (a INT NOT NULL)"),
        "column constraints are structural",
    );

    // The INSERT source kind, a per-row DEFAULT, and the override clause differ.
    assert_ne!(
        ours("INSERT INTO t VALUES (1)"),
        ours("INSERT INTO t SELECT 1"),
        "the INSERT source kind is structural",
    );
    assert_ne!(
        ours("INSERT INTO t VALUES (1)"),
        ours("INSERT INTO t VALUES (DEFAULT)"),
        "a per-row DEFAULT is structural",
    );
    assert_ne!(
        ours("INSERT INTO t OVERRIDING SYSTEM VALUE VALUES (1)"),
        ours("INSERT INTO t OVERRIDING USER VALUE VALUES (1)"),
        "the override kind is structural",
    );

    // UPDATE single vs. tuple assignment, and `WHERE` vs. `WHERE CURRENT OF`.
    assert_ne!(
        ours("UPDATE t SET a = 1, b = 2"),
        ours("UPDATE t SET (a, b) = (1, 2)"),
        "a tuple assignment is not a pair of single assignments",
    );
    assert_ne!(
        ours("UPDATE t SET a = 1 WHERE b = 2"),
        ours("UPDATE t SET a = 1 WHERE CURRENT OF c"),
        "WHERE CURRENT OF is structurally distinct from a WHERE predicate",
    );

    // ON CONFLICT arbiter kind and DROP cascade behaviour are structural.
    assert_ne!(
        ours("INSERT INTO t VALUES (1) ON CONFLICT (id) DO NOTHING"),
        ours("INSERT INTO t VALUES (1) ON CONFLICT ON CONSTRAINT c DO NOTHING"),
        "the ON CONFLICT arbiter kind is structural",
    );
    assert_ne!(
        ours("DROP TABLE t"),
        ours("DROP TABLE t CASCADE"),
        "DROP CASCADE is structural",
    );
}

#[test]
fn pg_structural_oracle_catches_ddl_dml_misbinding() {
    // The DDL/DML shapes carry their embedded expressions as binding trees, so a
    // precedence mis-bind inside a CHECK predicate, an index key, or a DML
    // predicate maps to a shape different from PostgreSQL's (ADR-0008/0015).
    let pg = first_pg_shape;
    let ours = first_ours_shape;

    for (correct, misbinding) in [
        (
            "CREATE TABLE t (id INT, CHECK (id = 1 OR id = 2 AND id = 3))",
            "CREATE TABLE t (id INT, CHECK ((id = 1 OR id = 2) AND id = 3))",
        ),
        (
            "CREATE INDEX i ON t ((a + b * c))",
            "CREATE INDEX i ON t (((a + b) * c))",
        ),
        (
            "UPDATE t SET a = 1 WHERE b + c * d > 0",
            "UPDATE t SET a = 1 WHERE (b + c) * d > 0",
        ),
        (
            "DELETE FROM t WHERE a OR b AND c",
            "DELETE FROM t WHERE (a OR b) AND c",
        ),
    ] {
        let correct_shape = pg(correct);
        assert_ne!(
            correct_shape,
            pg(misbinding),
            "the mis-bound grouping must map to a different shape than {correct:?}",
        );
        assert_eq!(
            ours(correct),
            correct_shape,
            "structural parity on {correct:?}"
        );
        assert_ne!(
            ours(correct),
            pg(misbinding),
            "our parse of {correct:?} must not match the mis-binding {misbinding:?}",
        );
    }
}

#[test]
fn pg_regress_guide_cases_remain_ticketed_gaps() {
    let cases = pg_regress_guide_cases(PG_REGRESS_GUIDE_SQL);
    assert!(
        !cases.is_empty(),
        "guide corpus should contain at least one case"
    );

    for case in cases {
        assert!(
            !case.ticket.trim().is_empty(),
            "guide case {} needs a provenance label",
            case.id
        );
        pg_query::parse(&case.sql).unwrap_or_else(|err| {
            panic!(
                "guide case {} from {} should be accepted by PostgreSQL: {err}",
                case.id, case.source,
            )
        });

        let Some(divergence) = pg_structural_divergence(&case.sql) else {
            panic!(
                "guide case {} from {} now has PostgreSQL structural parity; \
                     move it into regress-supported.sql under {}",
                case.id, case.source, case.ticket,
            );
        };
        assert!(
            !divergence.trim().is_empty(),
            "guide case {} produced an empty divergence reason",
            case.id,
        );
    }
}

#[test]
fn structural_shape_detects_clause_differences() {
    let without_where =
        squonk_shape(&parse_with("SELECT a FROM t", Postgres).expect("query parses"));
    let with_where =
        squonk_shape(&parse_with("SELECT a FROM t WHERE a > 1", Postgres).expect("query parses"));

    assert_ne!(without_where, with_where);
}

#[test]
fn structural_shape_detects_select_core_content_differences() {
    let column_a = squonk_shape(&parse_with("SELECT a FROM t", Postgres).expect("query parses"));
    let column_b = squonk_shape(&parse_with("SELECT b FROM t", Postgres).expect("query parses"));
    assert_ne!(column_a, column_b, "projection names are structural");

    let greater =
        squonk_shape(&parse_with("SELECT a FROM t WHERE a > 1", Postgres).expect("query parses"));
    let less =
        squonk_shape(&parse_with("SELECT a FROM t WHERE a < 1", Postgres).expect("query parses"));
    assert_ne!(greater, less, "predicate operators are structural");

    let order_default =
        squonk_shape(&parse_with("SELECT a FROM t ORDER BY a", Postgres).expect("query parses"));
    let order_desc = squonk_shape(
        &parse_with("SELECT a FROM t ORDER BY a DESC", Postgres).expect("query parses"),
    );
    assert_ne!(
        order_default, order_desc,
        "ORDER BY direction is structural"
    );
}

#[test]
fn structural_shape_detects_expression_content_differences() {
    let ours = |sql: &str| squonk_shape(&parse_with(sql, Postgres).expect("query parses"));

    // NULL-test negation, function name, and function arity/args are structural.
    assert_ne!(ours("SELECT a IS NULL"), ours("SELECT a IS NOT NULL"));
    assert_ne!(ours("SELECT f(a)"), ours("SELECT g(a)"));
    assert_ne!(ours("SELECT f(a)"), ours("SELECT f(b)"));
    assert_ne!(ours("SELECT f(a)"), ours("SELECT f(a, b)"));

    // CASE operand presence and branch contents are structural.
    assert_ne!(
        ours("SELECT CASE WHEN a THEN 1 END"),
        ours("SELECT CASE a WHEN b THEN 1 END"),
    );
    assert_ne!(
        ours("SELECT CASE WHEN a THEN 1 END"),
        ours("SELECT CASE WHEN a THEN 1 ELSE 2 END"),
    );

    // The signed-literal normalization unfolds the sign rather than collapsing
    // it: `-1` and `1` stay distinct, and `-1` matches PostgreSQL's folded form.
    assert_ne!(ours("SELECT -1"), ours("SELECT 1"));
    let pg_neg = pg_shape(&pg_query::parse("SELECT -1").expect("pg parses").protobuf)
        .expect("mapped PostgreSQL shape");
    assert_eq!(ours("SELECT -1"), pg_neg);
}

#[test]
fn postgres_shape_detects_select_core_content_differences() {
    let shape = |sql: &str| {
        pg_shape(&pg_query::parse(sql).expect("pg_query parses").protobuf)
            .expect("mapped PostgreSQL shape")
    };

    assert_ne!(shape("SELECT a FROM t"), shape("SELECT b FROM t"));
    assert_ne!(
        shape("SELECT a FROM t WHERE a > 1"),
        shape("SELECT a FROM t WHERE a < 1")
    );
    assert_ne!(
        shape("SELECT a FROM t LIMIT 1"),
        shape("SELECT a FROM t LIMIT 2")
    );
}

fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = panic.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = panic.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else {
        "<non-string panic>".to_owned()
    }
}

// --- feature-labelled differential corpus (prod-coverage-labels-differential-corpus)
//
// ZetaSQL tags every compliance case with `[required_features=...]` and *skips* (never
// fails) a case whose features the engine under test lacks. This applies that model to
// the PostgreSQL differential: each entry is PostgreSQL SQL the real engine accepts,
// tagged with the `FeatureSet` sub-flags a dialect must enable to parse it. At M1 the
// differential runs under one engine (PostgreSQL, all features on) so every entry
// applies and the labels add no selectivity; they pay off at M2+ (SQLite/DuckDB/MySQL),
// where a partial dialect skips the constructs it lacks instead of failing.
//
// The labels reuse the *same* executable toggles the coverage cases use
// (`crate::coverage`), and are kept honest two ways: against ground truth —
// `assert_accept_reject_parity` checks the real libpg_query parser accepts each entry,
// which the coverage cases (squonk-only) do not — and against decoration —
// `differential_labels_are_genuine` turns each required feature off and asserts our
// parser then rejects the entry (the falsely-required flip).
//
// Only features PostgreSQL genuinely accepts appear here. Dialect-only extensions the
// real engine rejects (hex/octal/binary numeric literals, `#` line comments, `?`
// placeholders) and structural-only flags (`double_quoted_strings`, whose `||`-style
// meaning shift never changes accept/reject) carry no differential entry — they are
// covered by the coverage cases, including the structural `forbidden_features` case.
const LABELLED_DIFFERENTIAL_CORPUS: &[(&str, &[&str])] = &[
    // String-literal extensions.
    ("SELECT E'x'", &["escape_strings"]),
    ("SELECT $$x$$", &["dollar_quoted_strings"]),
    // `N'x'` carries no differential entry: PostgreSQL has no national-string constant
    // (pg-national-strings-lexing-divergence), so `national_strings` is off in the PG preset
    // and cannot be a required PG label. PG lexes `N'x'` as the typed literal `nchar 'x'`;
    // our parser reads the generalized typed literal `N '…'` — the same
    // `typed_string_literals` class as `PG_TYPED_LITERAL_CORPUS`'s `float8 'NaN'`, where it
    // is accept/reject + round-trip verified (PG's type-name canonicalization keeps the
    // exact shape out of the structural corpus).
    ("SELECT B'1010'", &["bit_string_literals"]),
    ("SELECT U&'x'", &["unicode_strings"]),
    // Parameter placeholder.
    ("SELECT $1", &["positional_dollar"]),
    // PostgreSQL expression forms.
    ("SELECT a::int", &["typecast_operator"]),
    ("SELECT a[1]", &["subscript"]),
    ("SELECT a COLLATE \"C\"", &["collate"]),
    ("SELECT a AT TIME ZONE 'UTC'", &["at_time_zone"]),
    ("SELECT ARRAY[1, 2]", &["array_constructor"]),
    ("SELECT (a, b)", &["row_constructor"]),
    ("SELECT (a).b", &["field_selection"]),
    ("SELECT f(a => 1)", &["named_argument"]),
    ("SELECT a OPERATOR(+) b", &["operator_construct"]),
    // Advanced table expressions.
    ("SELECT * FROM LATERAL (SELECT 1) AS s", &["lateral"]),
    ("SELECT * FROM generate_series(1, 3)", &["table_functions"]),
    (
        "SELECT * FROM ROWS FROM (generate_series(1, 3))",
        &["rows_from"],
    ),
    ("SELECT * FROM ONLY t", &["only"]),
    (
        "SELECT * FROM t TABLESAMPLE BERNOULLI (10)",
        &["table_sample"],
    ),
    (
        "SELECT * FROM (t JOIN u ON t.id = u.id) AS j",
        &["parenthesized_joins"],
    ),
    ("SELECT * FROM t AS a(c)", &["table_alias_column_lists"]),
    (
        "SELECT * FROM t JOIN u USING (a) AS x",
        &["join_using_alias"],
    ),
    // SELECT-clause extensions.
    ("SELECT DISTINCT ON (a) a FROM t", &["distinct_on"]),
    ("SELECT 1 FETCH FIRST 2 ROWS ONLY", &["fetch_first"]),
    // DML extensions.
    ("INSERT INTO t VALUES (1) RETURNING id", &["returning"]),
    (
        "INSERT INTO t VALUES (1) ON CONFLICT DO NOTHING",
        &["on_conflict"],
    ),
    ("UPDATE t SET (a, b) = (1, 2)", &["multi_column_assignment"]),
    (
        "UPDATE t SET a = 1 WHERE CURRENT OF c",
        &["where_current_of"],
    ),
    // Schema-change extensions.
    ("DROP TABLE IF EXISTS t", &["if_exists"]),
    ("DROP TABLE t CASCADE", &["drop_behavior"]),
    (
        "CREATE INDEX CONCURRENTLY i ON t (a)",
        &["index_concurrently"],
    ),
    (
        "CREATE INDEX i ON t USING btree (a)",
        &["index_using_method"],
    ),
    (
        "CREATE INDEX i ON t (a) WHERE a IS NOT NULL",
        &["partial_index"],
    ),
];

#[test]
fn differential_corpus_is_selectable_per_dialect() {
    // Skip-vs-fail over the differential corpus. Under PostgreSQL (all features on)
    // every labelled entry applies and the real engine accepts it — the differential
    // ground truth. Under ANSI — a partial dialect — only the SQL-standard entries it
    // also enables apply (parenthesized joins, table alias column lists, FETCH FIRST,
    // and the `CASCADE` drop behaviour); the PostgreSQL-only entries are *skipped*, not
    // failed. That partition is the ZetaSQL property the labels buy for the M2+ oracles.
    use squonk::ast::dialect::FeatureSet;

    let mut ansi_applied = 0;
    let mut ansi_skipped = 0;
    for &(sql, required) in LABELLED_DIFFERENTIAL_CORPUS {
        assert!(
            crate::coverage::required_features_satisfied(required, &FeatureSet::POSTGRES),
            "PostgreSQL should satisfy every differential label for {sql:?}",
        );
        // Ground truth: the real PostgreSQL parser accepts the entry, and ours agrees.
        assert_accept_reject_parity(sql);

        if crate::coverage::required_features_satisfied(required, &FeatureSet::ANSI) {
            ansi_applied += 1;
            assert!(
                crate::coverage::accepts_under(sql, &FeatureSet::ANSI),
                "an entry whose features ANSI enables must parse under ANSI: {sql:?}",
            );
        } else {
            ansi_skipped += 1;
        }
    }
    assert!(
        ansi_skipped > 0,
        "ANSI must skip the PostgreSQL-only differential entries, not fail them",
    );
    assert!(
        ansi_applied > 0,
        "ANSI must still run the SQL-standard differential entries it satisfies",
    );
}

#[test]
fn differential_labels_are_genuine() {
    // The falsely-required flip applied to the differential corpus: each entry parses
    // under PostgreSQL (all features on), and turning OFF any single required feature
    // must change how our parser reads it — either it rejects (an accept/reject flip),
    // or it still parses but with a different tree (a *shape* feature, e.g. a
    // string-prefix marker re-read as the generalized typed literal `marker 'string'`,
    // prod-literal-generic-typed). Either way the label names features the construct
    // genuinely needs, not decoration. (Real-engine acceptance is asserted above.)
    use squonk::ast::dialect::FeatureSet;

    for &(sql, required) in LABELLED_DIFFERENTIAL_CORPUS {
        assert!(
            crate::coverage::accepts_under(sql, &FeatureSet::POSTGRES),
            "{sql:?} should parse under the full PostgreSQL feature set",
        );
        assert!(
            !required.is_empty(),
            "a labelled differential entry must declare at least one feature: {sql:?}",
        );
        for &name in required {
            assert!(
                crate::coverage::feature_flip_changes_parse(sql, name),
                "turning off `{name}` should change the parse of {sql:?} (reject or shape) \
                     — otherwise the label is not genuinely required",
            );
        }
    }
}

/// `CREATE EXTENSION` / `ALTER EXTENSION` forms both parsers accept
/// (parse-pg-extension-ddl): the create-option axis, the `UPDATE` version bump, and
/// the full `ADD`/`DROP <member>` object-reference axis (every signature shape pg_query
/// admits — named, routine, aggregate, operator, operator class/family, cast, type,
/// transform). Every entry round-trips.
const PG_EXTENSION_DDL_ACCEPT_CORPUS: &[&str] = &[
    // CreateExtensionStmt option forms.
    "CREATE EXTENSION ext",
    "CREATE EXTENSION IF NOT EXISTS ext",
    "CREATE EXTENSION ext WITH",
    "CREATE EXTENSION ext WITH SCHEMA s",
    "CREATE EXTENSION ext SCHEMA s",
    "CREATE EXTENSION ext VERSION '1.0'",
    "CREATE EXTENSION ext VERSION v1",
    "CREATE EXTENSION ext CASCADE",
    "CREATE EXTENSION ext WITH SCHEMA s VERSION '1.0' CASCADE",
    "CREATE EXTENSION ext CASCADE SCHEMA s VERSION '1.0'",
    "CREATE EXTENSION ext SCHEMA s SCHEMA s2",
    "CREATE EXTENSION IF NOT EXISTS ext WITH SCHEMA s VERSION '2' CASCADE",
    "CREATE EXTENSION \"quoted ext\"",
    // AlterExtensionStmt UPDATE.
    "ALTER EXTENSION ext UPDATE",
    "ALTER EXTENSION ext UPDATE TO '2.0'",
    "ALTER EXTENSION ext UPDATE TO v2",
    // AlterExtensionContentsStmt — object_type_any_name (schema-qualifiable) members.
    "ALTER EXTENSION ext ADD TABLE t",
    "ALTER EXTENSION ext DROP TABLE t",
    "ALTER EXTENSION ext ADD TABLE s.t",
    "ALTER EXTENSION ext ADD SEQUENCE q",
    "ALTER EXTENSION ext ADD VIEW v",
    "ALTER EXTENSION ext ADD MATERIALIZED VIEW mv",
    "ALTER EXTENSION ext ADD INDEX i",
    "ALTER EXTENSION ext ADD FOREIGN TABLE ft",
    "ALTER EXTENSION ext ADD COLLATION c",
    "ALTER EXTENSION ext ADD CONVERSION c",
    "ALTER EXTENSION ext ADD STATISTICS st",
    "ALTER EXTENSION ext ADD TEXT SEARCH PARSER p",
    "ALTER EXTENSION ext ADD TEXT SEARCH DICTIONARY d",
    "ALTER EXTENSION ext ADD TEXT SEARCH TEMPLATE tp",
    "ALTER EXTENSION ext ADD TEXT SEARCH CONFIGURATION cf",
    // object_type_name (single-name) members.
    "ALTER EXTENSION ext ADD ACCESS METHOD am",
    "ALTER EXTENSION ext ADD EVENT TRIGGER et",
    "ALTER EXTENSION ext ADD FOREIGN DATA WRAPPER fdw",
    "ALTER EXTENSION ext ADD LANGUAGE plpgsql",
    "ALTER EXTENSION ext ADD PROCEDURAL LANGUAGE plpgsql",
    "ALTER EXTENSION ext ADD PUBLICATION pub",
    "ALTER EXTENSION ext ADD SCHEMA sch",
    "ALTER EXTENSION ext ADD SERVER srv",
    "ALTER EXTENSION ext ADD DATABASE db",
    "ALTER EXTENSION ext ADD ROLE r",
    "ALTER EXTENSION ext ADD TABLESPACE ts",
    // Signature/typed members.
    "ALTER EXTENSION ext ADD AGGREGATE agg(int)",
    "ALTER EXTENSION ext ADD AGGREGATE agg(*)",
    "ALTER EXTENSION ext ADD AGGREGATE agg(int, text)",
    "ALTER EXTENSION ext ADD AGGREGATE agg(int ORDER BY text)",
    "ALTER EXTENSION ext ADD AGGREGATE agg(ORDER BY text)",
    "ALTER EXTENSION ext ADD CAST (int AS text)",
    "ALTER EXTENSION ext ADD DOMAIN dom",
    "ALTER EXTENSION ext ADD DOMAIN s.dom",
    "ALTER EXTENSION ext ADD FUNCTION f",
    "ALTER EXTENSION ext ADD FUNCTION f(int)",
    "ALTER EXTENSION ext ADD FUNCTION f(int, text)",
    "ALTER EXTENSION ext ADD FUNCTION s.f(int)",
    "ALTER EXTENSION ext ADD OPERATOR + (int, int)",
    "ALTER EXTENSION ext ADD OPERATOR + (NONE, int)",
    "ALTER EXTENSION ext ADD OPERATOR + (int, NONE)",
    "ALTER EXTENSION ext ADD OPERATOR CLASS oc USING btree",
    "ALTER EXTENSION ext ADD OPERATOR FAMILY of USING btree",
    "ALTER EXTENSION ext ADD PROCEDURE p(int)",
    "ALTER EXTENSION ext ADD ROUTINE r(int)",
    "ALTER EXTENSION ext ADD TRANSFORM FOR int LANGUAGE sql",
    "ALTER EXTENSION ext ADD TYPE ty",
    "ALTER EXTENSION ext ADD TYPE s.ty",
];

/// Extension-DDL forms both parsers reject, guarding the grammar boundary: the retired
/// `CREATE EXTENSION ... FROM` item, the `object_type_name_on_any_name` members that are
/// not extension-membership object kinds (`TRIGGER`/`RULE`/`POLICY`), a schema-qualified
/// name where the single-name grammar forbids one, and a fully-`NONE` operator signature.
///
/// `ALTER EXTENSION <name> SET SCHEMA <schema>` is deliberately absent: pg_query maps it
/// to `AlterObjectSchemaStmt` (the relocatable-object production shared with every other
/// `SET SCHEMA`), not `AlterExtensionStmt`, so it belongs to that separate head, not this
/// ticket.
const PG_EXTENSION_DDL_REJECT_CORPUS: &[&str] = &[
    "CREATE EXTENSION ext FROM 'unpackaged'",
    "ALTER EXTENSION ext ADD TRIGGER tg",
    "ALTER EXTENSION ext ADD RULE ru",
    "ALTER EXTENSION ext ADD POLICY po",
    "ALTER EXTENSION ext ADD COLUMN c",
    "ALTER EXTENSION ext ADD SCHEMA s.bad",
    "ALTER EXTENSION ext ADD OPERATOR + (NONE, NONE)",
];

#[test]
fn pg_parity_and_roundtrip_for_extension_ddl() {
    use crate::corpus_roundtrip::{Roundtrip, roundtrip};
    use squonk::dialect::Postgres;

    for sql in PG_EXTENSION_DDL_ACCEPT_CORPUS {
        assert_accept_reject_parity(sql);
        match roundtrip(sql, Postgres) {
            Roundtrip::Ok => {}
            Roundtrip::Unparsable => panic!("expected {sql:?} to parse under Postgres"),
            Roundtrip::Failed(message) => panic!("extension DDL round-trip failed: {message}"),
        }
    }
    for sql in PG_EXTENSION_DDL_REJECT_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

/// `DROP TRANSFORM [IF EXISTS] FOR <type> LANGUAGE <lang> [CASCADE | RESTRICT]` forms both
/// parsers accept (parse-pg-drop-transform): the `IF EXISTS` guard (only between `TRANSFORM`
/// and `FOR`), the `CASCADE`/`RESTRICT` behaviour, the full `Typename` axis for the `FOR`
/// type (bare/extension type, schema-qualified, array, parameterized, multi-word), and a
/// `ColId` language name — bare or a quoted reserved word. Every entry round-trips.
const PG_DROP_TRANSFORM_ACCEPT_CORPUS: &[&str] = &[
    "DROP TRANSFORM FOR hstore LANGUAGE plpython3u",
    "DROP TRANSFORM IF EXISTS FOR hstore LANGUAGE plpython3u",
    "DROP TRANSFORM FOR hstore LANGUAGE plpython3u CASCADE",
    "DROP TRANSFORM FOR hstore LANGUAGE plpython3u RESTRICT",
    "DROP TRANSFORM IF EXISTS FOR int LANGUAGE sql CASCADE",
    "DROP TRANSFORM FOR pg_catalog.int4 LANGUAGE c",
    // A bare `C` is a reserved-ish language spelling only as a quoted identifier.
    "DROP TRANSFORM FOR integer LANGUAGE \"C\"",
    "DROP TRANSFORM FOR int[] LANGUAGE sql",
    "DROP TRANSFORM FOR varchar(10) LANGUAGE sql",
    // `default` is a reserved keyword: rejected bare (see the reject corpus), accepted quoted.
    "DROP TRANSFORM FOR int LANGUAGE \"default\"",
    "DROP TRANSFORM FOR double precision LANGUAGE sql",
    "DROP TRANSFORM FOR timestamp with time zone LANGUAGE sql",
];

/// `DROP TRANSFORM` forms both parsers reject, guarding the grammar boundary: a bare
/// reserved keyword in the `ColId` language position (`default`, `for`), the `IF EXISTS`
/// guard in any position other than between `TRANSFORM` and `FOR`, the missing `FOR`/type/
/// `LANGUAGE`/name pieces, and a comma list (PostgreSQL admits exactly one transform).
const PG_DROP_TRANSFORM_REJECT_CORPUS: &[&str] = &[
    "DROP TRANSFORM FOR int LANGUAGE default",
    "DROP TRANSFORM FOR int LANGUAGE for",
    "DROP TRANSFORM FOR hstore IF EXISTS LANGUAGE plpython3u",
    "DROP TRANSFORM FOR hstore",
    "DROP TRANSFORM LANGUAGE sql",
    "DROP TRANSFORM hstore LANGUAGE sql",
    "DROP TRANSFORM FOR int LANGUAGE sql, FOR text LANGUAGE sql",
];

#[test]
fn pg_parity_and_roundtrip_for_drop_transform() {
    use crate::corpus_roundtrip::{Roundtrip, roundtrip};
    use squonk::dialect::Postgres;

    for sql in PG_DROP_TRANSFORM_ACCEPT_CORPUS {
        assert_accept_reject_parity(sql);
        match roundtrip(sql, Postgres) {
            Roundtrip::Ok => {}
            Roundtrip::Unparsable => panic!("expected {sql:?} to parse under Postgres"),
            Roundtrip::Failed(message) => panic!("DROP TRANSFORM round-trip failed: {message}"),
        }
    }
    for sql in PG_DROP_TRANSFORM_REJECT_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

/// `ALTER <object> [NO] DEPENDS ON EXTENSION <ext>` forms both parsers accept
/// (parse-pg-alter-object-depends): the four object heads PostgreSQL's
/// `AlterObjectDependsStmt` admits — `FUNCTION`/`PROCEDURE`/`ROUTINE`
/// (`function_with_argtypes`, with and without an arg list, schema-qualified), `TRIGGER
/// <name> ON <table>` (the table up to a three-part `qualified_name`), `MATERIALIZED VIEW`,
/// and `INDEX` — across the `NO` negation, quoted/qualified object names, and a quoted
/// extension name. Every entry round-trips.
const PG_ALTER_OBJECT_DEPENDS_ACCEPT_CORPUS: &[&str] = &[
    // FUNCTION/PROCEDURE/ROUTINE — function_with_argtypes.
    "ALTER FUNCTION f(integer) DEPENDS ON EXTENSION ext",
    "ALTER FUNCTION f DEPENDS ON EXTENSION ext",
    "ALTER FUNCTION f() DEPENDS ON EXTENSION ext",
    "ALTER FUNCTION f(integer, text) DEPENDS ON EXTENSION ext",
    "ALTER FUNCTION s.f(integer) DEPENDS ON EXTENSION ext",
    "ALTER FUNCTION f(integer) NO DEPENDS ON EXTENSION ext",
    "ALTER PROCEDURE p(integer) DEPENDS ON EXTENSION ext",
    "ALTER PROCEDURE p DEPENDS ON EXTENSION ext",
    "ALTER ROUTINE r(integer) DEPENDS ON EXTENSION ext",
    "ALTER ROUTINE r NO DEPENDS ON EXTENSION ext",
    // TRIGGER name ON qualified_name.
    "ALTER TRIGGER tg ON tbl DEPENDS ON EXTENSION ext",
    "ALTER TRIGGER tg ON s.tbl DEPENDS ON EXTENSION ext",
    "ALTER TRIGGER tg ON cat.s.tbl DEPENDS ON EXTENSION ext",
    "ALTER TRIGGER tg ON tbl NO DEPENDS ON EXTENSION ext",
    // MATERIALIZED VIEW qualified_name.
    "ALTER MATERIALIZED VIEW mv DEPENDS ON EXTENSION ext",
    "ALTER MATERIALIZED VIEW s.mv DEPENDS ON EXTENSION ext",
    "ALTER MATERIALIZED VIEW mv NO DEPENDS ON EXTENSION ext",
    // INDEX qualified_name.
    "ALTER INDEX i DEPENDS ON EXTENSION ext",
    "ALTER INDEX s.i DEPENDS ON EXTENSION ext",
    "ALTER INDEX cat.s.i DEPENDS ON EXTENSION ext",
    "ALTER INDEX i NO DEPENDS ON EXTENSION ext",
    // Quoted names.
    "ALTER INDEX \"My Index\" DEPENDS ON EXTENSION \"My Ext\"",
];

/// `ALTER <object> … DEPENDS ON EXTENSION` forms both parsers reject, guarding the object
/// boundary: object kinds outside the four the grammar admits (`TABLE`, `VIEW` (plain),
/// `SEQUENCE`, `FOREIGN TABLE`), a schema-qualified extension name where PostgreSQL's
/// `name` forbids qualification, and a reserved word (`select`) where a `ColId` is
/// required.
const PG_ALTER_OBJECT_DEPENDS_REJECT_CORPUS: &[&str] = &[
    "ALTER TABLE t DEPENDS ON EXTENSION ext",
    "ALTER VIEW v DEPENDS ON EXTENSION ext",
    "ALTER SEQUENCE q DEPENDS ON EXTENSION ext",
    "ALTER FOREIGN TABLE ft DEPENDS ON EXTENSION ext",
    "ALTER FUNCTION f(integer) DEPENDS ON EXTENSION s.ext",
    "ALTER INDEX i DEPENDS ON EXTENSION select",
];

#[test]
fn pg_parity_and_roundtrip_for_alter_object_depends() {
    use crate::corpus_roundtrip::{Roundtrip, roundtrip};
    use squonk::dialect::Postgres;

    for sql in PG_ALTER_OBJECT_DEPENDS_ACCEPT_CORPUS {
        assert_accept_reject_parity(sql);
        match roundtrip(sql, Postgres) {
            Roundtrip::Ok => {}
            Roundtrip::Unparsable => panic!("expected {sql:?} to parse under Postgres"),
            Roundtrip::Failed(message) => {
                panic!("alter-object-depends round-trip failed: {message}")
            }
        }
    }
    for sql in PG_ALTER_OBJECT_DEPENDS_REJECT_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

/// `ALTER SYSTEM { SET … | RESET … }` forms both parsers accept (parse-pg-alter-system):
/// the `generic_set` value axis reused from the session `SET` — `=` / `TO` separators, a
/// string / numeric / bareword / `ON`-`OFF` value, a comma value list, a signed numeric,
/// a dotted (multi-part) `var_name`, a quoted parameter name, and `DEFAULT` — plus the
/// `generic_reset` targets (`RESET <name>`, `RESET ALL`, dotted). Every entry round-trips.
const PG_ALTER_SYSTEM_ACCEPT_CORPUS: &[&str] = &[
    // generic_set — value shapes.
    "ALTER SYSTEM SET work_mem = '64MB'",
    "ALTER SYSTEM SET work_mem TO '64MB'",
    "ALTER SYSTEM SET max_connections = 100",
    "ALTER SYSTEM SET max_connections TO 100",
    "ALTER SYSTEM SET search_path = public",
    "ALTER SYSTEM SET search_path TO public, other",
    "ALTER SYSTEM SET search_path = public, other, third",
    "ALTER SYSTEM SET ssl = on",
    "ALTER SYSTEM SET ssl = off",
    "ALTER SYSTEM SET ssl TO on",
    "ALTER SYSTEM SET seq_page_cost = -1",
    "ALTER SYSTEM SET seq_page_cost = 1.5",
    "ALTER SYSTEM SET seq_page_cost TO 2",
    // Dotted (custom) var_name and quoted parameter name.
    "ALTER SYSTEM SET myapp.foo = 'bar'",
    "ALTER SYSTEM SET myapp.foo TO 'bar'",
    "ALTER SYSTEM SET a.b.c = 1",
    "ALTER SYSTEM SET \"quoted param\" = 1",
    // DEFAULT sentinel.
    "ALTER SYSTEM SET work_mem = DEFAULT",
    "ALTER SYSTEM SET work_mem TO DEFAULT",
    // generic_reset.
    "ALTER SYSTEM RESET work_mem",
    "ALTER SYSTEM RESET ALL",
    "ALTER SYSTEM RESET myapp.foo",
    "ALTER SYSTEM RESET a.b.c",
];

/// `ALTER SYSTEM …` forms both parsers reject, guarding the grammar boundary: a `SET`
/// with no value, `SET ALL` (only `RESET ALL` exists), the `FROM CURRENT` form
/// `generic_set` does not admit (unlike the session `SET`), a parenthesized value, a
/// trailing value after `RESET`, and the `SESSION` / `LOCAL` scope keyword `ALTER SYSTEM`
/// forbids (the wrapper is exactly `generic_set` / `generic_reset`).
const PG_ALTER_SYSTEM_REJECT_CORPUS: &[&str] = &[
    "ALTER SYSTEM SET x",
    "ALTER SYSTEM SET ALL",
    "ALTER SYSTEM SET x FROM CURRENT",
    "ALTER SYSTEM SET x = ('a')",
    "ALTER SYSTEM RESET x = 1",
    "ALTER SYSTEM SET LOCAL x = 1",
    "ALTER SYSTEM SET SESSION x = 1",
];

#[test]
fn pg_parity_and_roundtrip_for_alter_system() {
    use crate::corpus_roundtrip::{Roundtrip, roundtrip};
    use squonk::dialect::Postgres;

    for sql in PG_ALTER_SYSTEM_ACCEPT_CORPUS {
        assert_accept_reject_parity(sql);
        match roundtrip(sql, Postgres) {
            Roundtrip::Ok => {}
            Roundtrip::Unparsable => panic!("expected {sql:?} to parse under Postgres"),
            Roundtrip::Failed(message) => panic!("alter-system round-trip failed: {message}"),
        }
    }
    for sql in PG_ALTER_SYSTEM_REJECT_CORPUS {
        assert_accept_reject_parity(sql);
    }
}

/// The differential is segmentation-aware: when both parsers accept, their top-level
/// statement counts must agree (pg-do-statement-separator-divergence's masking class —
/// boolean agreement hid a splitter mis-split for months). The corpus walks the
/// segmentation-sensitive shapes: empty statements, doubled separators, trailing
/// comments, interior `;` inside dollar-quoted bodies and string literals — every
/// entry must produce NO divergence, i.e. identical accept/reject AND identical counts.
#[test]
fn pg_statement_segmentation_matches_pg_query() {
    const SEGMENTATION_PARITY_CORPUS: &[&str] = &[
        "SELECT 1",
        "SELECT 1;",
        "SELECT 1;;",
        ";;SELECT 1",
        "SELECT 1;;SELECT 2",
        "SELECT 1; SELECT 2;",
        ";",
        "",
        "SELECT 1 -- trailing comment",
        "SELECT 1; -- c\nSELECT 2",
        "DO $$ SELECT 1; SELECT 2; $$",
        "SELECT ';'; SELECT 1",
    ];
    for sql in SEGMENTATION_PARITY_CORPUS {
        assert_eq!(
            pg_accept_reject_divergence(sql),
            None,
            "segmentation parity broke for {sql:?}",
        );
    }
}
