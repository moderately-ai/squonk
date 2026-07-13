// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Publication benchmark over the frozen portable full-AST corpus.

use std::hint::black_box;

use codspeed_criterion_compat::{Criterion, Throughput, criterion_group, criterion_main};
use serde::Deserialize;
use sqlparser::{dialect::AnsiDialect, parser::Parser};
use squonk::{dialect::Ansi, parse_with};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Deserialize)]
struct Corpus {
    statement_count: usize,
    statements: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    sql: String,
}

fn corpus() -> Corpus {
    let corpus: Corpus = serde_json::from_str(include_str!("../publication/corpus/portable.json"))
        .expect("checked-in portable corpus is valid JSON");
    assert_eq!(corpus.statements.len(), corpus.statement_count);
    corpus
}

fn parse_squonk(sql: &[String]) -> Vec<squonk::StockParsed> {
    sql.iter()
        .map(|statement| {
            parse_with(
                black_box(statement.as_str()),
                squonk::ParseConfig::new(Ansi),
            )
            .expect("publication corpus was qualified for Squonk ANSI")
        })
        .collect()
}

fn parse_sqlparser(sql: &[String]) -> Vec<Vec<sqlparser::ast::Statement>> {
    sql.iter()
        .map(|statement| {
            Parser::parse_sql(&AnsiDialect {}, black_box(statement.as_str()))
                .expect("publication corpus was qualified for sqlparser ANSI")
        })
        .collect()
}

fn publication(c: &mut Criterion) {
    let corpus = corpus();
    let sql: Vec<_> = corpus.statements.into_iter().map(|case| case.sql).collect();
    let bytes = sql.iter().map(String::len).sum::<usize>();
    let mut group = c.benchmark_group("publication/portable-full-ast");
    group.throughput(Throughput::Bytes(bytes as u64));
    group.bench_function("squonk", |b| {
        b.iter(|| black_box(parse_squonk(black_box(&sql))));
    });
    group.bench_function("datafusion-sqlparser-rs", |b| {
        b.iter(|| black_box(parse_sqlparser(black_box(&sql))));
    });
    group.finish();
}

criterion_group!(benches, publication);
criterion_main!(benches);
