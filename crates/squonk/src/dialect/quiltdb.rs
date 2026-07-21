// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! QuiltDB SQL syntax backed by the frozen parser contract in `squonk-conformance`.

use crate::ast::NoExt;
use crate::ast::dialect::FeatureSet;
use crate::parser::Dialect;

/// QuiltDB SQL, backed by [`FeatureSet::QUILTDB`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct QuiltDb;

impl Dialect for QuiltDb {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        &FeatureSet::QUILTDB
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ParseConfig, parse_with};

    #[test]
    fn composite_contract_is_enabled() {
        for sql in [
            "CREATE TABLE t (a ARRAY<INT>, s STRUCT<x INT>, m MAP(TEXT, INT))",
            "SELECT MAP {'a': 1}",
            "SELECT $1",
        ] {
            parse_with(sql, ParseConfig::new(QuiltDb))
                .unwrap_or_else(|error| panic!("QuiltDB should parse {sql:?}: {error}"));
        }
    }

    #[test]
    fn native_colocation_grammar_enforces_structure() {
        for sql in [
            "CREATE COLOCATION GROUP IF NOT EXISTS g PARTITION BY HASH (id) SHARDS 4",
            "CREATE COLOCATION GROUP r PARTITION BY RANGE (ts) SHARDS 3",
            "DROP COLOCATION GROUP IF EXISTS g",
            "CREATE TABLE t (id BIGINT) COLOCATE WITH anchor ON (id)",
            "CREATE TABLE t (id BIGINT) IN COLOCATION GROUP g ON (id)",
            "CREATE TABLE t (id BIGINT) WITH (range_min = 1) IN COLOCATION GROUP g",
            "ALTER TABLE t SET COLOCATION GROUP g",
            "ALTER TABLE t DROP COLOCATION GROUP",
        ] {
            parse_with(sql, ParseConfig::new(QuiltDb))
                .unwrap_or_else(|error| panic!("QuiltDB should parse {sql:?}: {error}"));
        }
        for sql in [
            "CREATE TABLE t (id BIGINT) COLOCATE WITH anchor",
            "CREATE TABLE t (id BIGINT) WITH (x = 1) COLOCATE WITH anchor ON (id)",
            "CREATE TABLE t (id BIGINT) COLOCATE WITH anchor ON (id) IN COLOCATION GROUP g",
            "CREATE TABLE t (id BIGINT) IN COLOCATION GROUP g",
            "CREATE TABLE t (id BIGINT) WITH (x = 1) IN COLOCATION GROUP g ON (id)",
            "ALTER TABLE t SET COLOCATION GROUP g ON (id)",
            "ALTER TABLE t SET COLOCATION GROUP g, ADD COLUMN x INT",
        ] {
            assert!(
                parse_with(sql, ParseConfig::new(QuiltDb)).is_err(),
                "QuiltDB must reject structurally invalid {sql:?}"
            );
        }
    }

    #[test]
    fn comment_targets_and_front_guard_preserve_structure() {
        for sql in [
            "COMMENT IF EXISTS ON TABLE t IS 'table'",
            "COMMENT ON COLUMN t.c IS NULL",
            "COMMENT ON VIEW v IS 'view'",
            "COMMENT ON MATERIALIZED VIEW mv IS 'materialized'",
            "COMMENT ON INDEX idx IS 'index'",
            "COMMENT ON CONSTRAINT uq ON t IS 'constraint'",
        ] {
            let parsed = parse_with(sql, ParseConfig::new(QuiltDb))
                .unwrap_or_else(|error| panic!("failed to parse {sql:?}: {error}"));
            let rendered = parsed.to_sql();
            parse_with(&rendered, ParseConfig::new(QuiltDb))
                .unwrap_or_else(|error| panic!("failed to reparse {rendered:?}: {error}"));
        }
    }

    #[test]
    fn narrowed_query_spellings_are_independently_gated() {
        for sql in [
            "SELECT * REPLACE (1 AS id) FROM t",
            "SELECT * FROM a NATURAL LEFT JOIN b",
            // The engine executes ILIKE (DataFusion's case-insensitive
            // match), so the grammar accepts it.
            "SELECT 1 WHERE 'a' ILIKE 'A'",
        ] {
            parse_with(sql, ParseConfig::new(QuiltDb))
                .unwrap_or_else(|error| panic!("failed to parse {sql:?}: {error}"));
        }
        for sql in [
            "SELECT * EXCLUDE (id) FROM t",
            "SELECT * RENAME (id AS other) FROM t",
            "SELECT * FROM a NATURAL CROSS JOIN b",
            "SELECT * FROM a INTERSECT ALL SELECT * FROM b",
            "SELECT * FROM a EXCEPT ALL SELECT * FROM b",
            "SELECT 1 WHERE 1 IS DISTINCT FROM 2",
        ] {
            assert!(
                parse_with(sql, ParseConfig::new(QuiltDb)).is_err(),
                "unexpectedly accepted {sql:?}"
            );
        }
    }

    #[test]
    fn introspection_statements_parse_and_the_rest_stay_rejected() {
        for sql in [
            "DESCRIBE t",
            "DESC t",
            "SHOW TABLES",
            "SHOW COLUMNS FROM t",
            "SHOW columns FROM db.t",
            "EXPLAIN SELECT 1",
            "EXPLAIN ANALYZE SELECT 1",
            "SHOW search_path",
            "SHOW ALL",
            "SET x = 1",
            "RESET x",
        ] {
            let parsed = parse_with(sql, ParseConfig::new(QuiltDb))
                .unwrap_or_else(|error| panic!("QuiltDB should parse {sql:?}: {error}"));
            let rendered = parsed.to_sql();
            parse_with(&rendered, ParseConfig::new(QuiltDb))
                .unwrap_or_else(|error| panic!("failed to reparse {rendered:?}: {error}"));
        }
        // With the typed listing off, `SHOW FUNCTIONS` falls through to the
        // generic session `SHOW <var>` (a variable named `functions`) —
        // PostgreSQL's exact parse, per the `ShowSyntax` MECE contract. The
        // unknown variable is the downstream planner's rejection, not a
        // grammar one; pin the fall-through shape so a future typed-listing
        // flag flip shows up here.
        {
            let parsed = parse_with("SHOW FUNCTIONS", ParseConfig::new(QuiltDb))
                .expect("QuiltDB should parse SHOW FUNCTIONS as a session SHOW");
            let statement = parsed.statements().iter().next().expect("one statement");
            assert!(
                matches!(
                    statement,
                    crate::ast::Statement::Session { session, .. }
                        if matches!(**session, crate::ast::SessionStatement::Show { .. })
                ),
                "SHOW FUNCTIONS must be the generic session SHOW, got: {statement:?}"
            );
        }
        // Reserved keywords cannot be session-variable names, so the off
        // typed forms are parse errors rather than fall-throughs.
        for sql in ["SHOW CREATE TABLE t", "SHOW FUNCTION STATUS", "SUMMARIZE t"] {
            assert!(
                parse_with(sql, ParseConfig::new(QuiltDb)).is_err(),
                "unexpectedly accepted {sql:?}"
            );
        }
    }

    #[test]
    fn extended_statement_shapes_parse_and_round_trip() {
        for sql in [
            "ALTER TABLE t SET (append_only = true)",
            "ALTER TABLE t RENAME CONSTRAINT old TO new",
            "ALTER TABLE t DROP PRIMARY KEY",
            "CREATE INDEX idx ON t USING btree (id) WITH (fillfactor = 70)",
            "REFRESH MATERIALIZED VIEW CONCURRENTLY mv WITH NO DATA",
            "COMMIT AND NO CHAIN",
            "INSERT IGNORE INTO t VALUES (1)",
            "INSERT OVERWRITE INTO t VALUES (1)",
            "UPDATE t JOIN u ON t.id = u.id SET x = 1",
            "DELETE FROM t JOIN u ON t.id = u.id",
            "MERGE INTO t USING s ON t.id = s.id WHEN NOT MATCHED THEN INSERT VALUES (1), (2)",
            "CREATE MATERIALIZED VIEW mv TO target AS SELECT 1",
            "CREATE TABLE t (id INT IDENTITY, v TEXT)",
            "CREATE TABLE t (id INT IDENTITY(5, 2), v TEXT)",
            "ALTER TABLE t ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY",
            "ALTER TABLE t ALTER COLUMN id ADD GENERATED BY DEFAULT AS IDENTITY (START WITH 5 INCREMENT BY 2)",
            "CREATE SEQUENCE s CACHE 10 START WITH 5",
        ] {
            let parsed = parse_with(sql, ParseConfig::new(QuiltDb))
                .unwrap_or_else(|error| panic!("failed to parse {sql:?}: {error}"));
            let rendered = parsed.to_sql();
            parse_with(&rendered, ParseConfig::new(QuiltDb))
                .unwrap_or_else(|error| panic!("failed to reparse {rendered:?}: {error}"));
        }
    }
}
