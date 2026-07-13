// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

use std::collections::BTreeSet;

use serde::Deserialize;
use sqlparser::{dialect::AnsiDialect, parser::Parser};
use squonk::{dialect::Ansi, parse_with};

#[derive(Deserialize)]
struct Corpus {
    schema: String,
    statement_count: usize,
    sha256: String,
    statements: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    family: String,
    complexity: String,
    bytes: usize,
    sql: String,
}

#[test]
fn portable_publication_corpus_is_balanced_and_qualified() {
    let corpus: Corpus = serde_json::from_str(include_str!("../publication/corpus/portable.json"))
        .expect("portable corpus JSON");
    assert_eq!(corpus.schema, "squonk.publication-corpus/1");
    assert_eq!(corpus.statement_count, 256);
    assert_eq!(corpus.statements.len(), 256);
    assert_eq!(corpus.sha256.len(), 64);

    let ids: BTreeSet<_> = corpus.statements.iter().map(|case| &case.id).collect();
    assert_eq!(ids.len(), 256);
    for (family, expected) in [("query", 144), ("dml", 64), ("ddl", 48)] {
        assert_eq!(
            corpus
                .statements
                .iter()
                .filter(|case| case.family == family)
                .count(),
            expected
        );
    }
    for complexity in ["small", "medium", "large", "complex"] {
        assert_eq!(
            corpus
                .statements
                .iter()
                .filter(|case| case.complexity == complexity)
                .count(),
            64
        );
    }

    for case in &corpus.statements {
        assert_eq!(case.bytes, case.sql.len());
        parse_with(&case.sql, squonk::ParseConfig::new(Ansi))
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        Parser::parse_sql(&AnsiDialect {}, &case.sql)
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
    }
}
