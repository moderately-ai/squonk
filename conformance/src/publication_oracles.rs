// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

use crate::m2::{DuckDbOracle, SqliteOracle};
use crate::m3::{MySqlOracle, WireVerdict};
use crate::oracle::{AcceptRejectOracle, OracleVerdict};

struct Case {
    id: String,
    sql: String,
}

fn corpus() -> Vec<Case> {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../bench/publication/corpus/portable.json"))
            .expect("portable publication corpus");
    corpus["statements"]
        .as_array()
        .expect("statement array")
        .iter()
        .map(|case| Case {
            id: case["id"].as_str().expect("case id").to_owned(),
            sql: case["sql"].as_str().expect("case SQL").to_owned(),
        })
        .collect()
}

fn setup_sql() -> String {
    let mut statements = vec![
        "CREATE TABLE portable_left(id INTEGER, value INTEGER)".to_owned(),
        "CREATE TABLE portable_right(id INTEGER, left_id INTEGER)".to_owned(),
    ];
    statements.extend((0..24).map(|index| {
        format!(
            "CREATE TABLE portable_values_{index}(id INTEGER, value INTEGER, label VARCHAR(64))"
        )
    }));
    statements
        .extend((0..12).map(|index| format!("CREATE TABLE portable_old_{index}(id INTEGER)")));
    statements.join(";")
}

#[test]
fn portable_publication_corpus_is_accepted_by_stable_engine_oracles() {
    let corpus = corpus();
    let setup = setup_sql();
    let sqlite = SqliteOracle::with_schema(&setup).expect("SQLite oracle");
    let duckdb = DuckDbOracle::with_schema(&setup).expect("DuckDB oracle");
    let mysql = MySqlOracle::with_schema(&format!(
        "CREATE DATABASE IF NOT EXISTS squonk_oracle; USE squonk_oracle; {setup}"
    ))
    .expect("MySQL oracle");

    for case in corpus {
        pg_query::parse(&case.sql)
            .unwrap_or_else(|error| panic!("{} rejected by PostgreSQL: {error}", case.id));
        assert_eq!(
            sqlite.verdict(&case.sql).expect("SQLite verdict"),
            OracleVerdict::Accept,
            "{} rejected by SQLite",
            case.id
        );
        assert_eq!(
            duckdb.verdict(&case.sql).expect("DuckDB verdict"),
            OracleVerdict::Accept,
            "{} rejected by DuckDB",
            case.id
        );
        match mysql.wire_verdict(&case.sql).expect("MySQL wire verdict") {
            WireVerdict::Accept => {}
            WireVerdict::Reject(1064) => panic!("{} rejected as invalid syntax by MySQL", case.id),
            WireVerdict::Reject(_) => {}
        }
    }
}
