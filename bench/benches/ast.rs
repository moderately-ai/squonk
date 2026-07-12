// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Ours-only wall-clock micro-benchmarks for the three production code paths —
//! tokenizer, parser, renderer — over fixed single-statement fixtures. These are the
//! CodSpeed-tracked (codspeed-criterion-compat) counterpart to the deterministic
//! callgrind `perf.rs` gate: CodSpeed's historical diffing tracks each path's
//! wall-clock drift, while `perf.rs` is the hard instruction-count gate (wall-clock
//! flaps, so it tracks but never fails — ADR-0016). No external parser is measured;
//! every function here exercises only `squonk`.

// Bench-only fast allocator (ADR-0017: never the production allocator). These
// micro-benchmarks run under mimalloc — the realistic allocator a perf-sensitive
// consumer ships — matching the rest of the COMPUTE suite and separate from the dhat
// MEMORY benches.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use codspeed_criterion_compat::{Criterion, criterion_group, criterion_main};
use squonk_bench::{
    ANALYTIC_SELECT, DEEP_NESTED_SELECT, PARSER_CASES, TOKENIZER_CASES, parse_ansi_sql,
    parse_postgres_sql, render_canonical_sql, render_parenthesized_sql, tokenize_sql,
};
use std::hint::black_box;

/// Guards tokenizer throughput: a regression here means lexing a statement got slower.
fn tokenizer(c: &mut Criterion) {
    let mut group = c.benchmark_group("tokenizer");
    for case in TOKENIZER_CASES {
        group.bench_function(case.name, |b| {
            b.iter(|| black_box(tokenize_sql(black_box(case.sql))));
        });
    }
    group.finish();
}

/// Guards end-to-end parse throughput under both the ANSI and Postgres dialects: a
/// regression means building the owned AST for a statement got slower on that dialect.
fn parser(c: &mut Criterion) {
    let mut ansi = c.benchmark_group("parser/ansi");
    for case in PARSER_CASES {
        ansi.bench_function(case.name, |b| {
            b.iter(|| black_box(parse_ansi_sql(black_box(case.sql))));
        });
    }
    ansi.finish();

    let mut postgres = c.benchmark_group("parser/postgres");
    for case in PARSER_CASES {
        postgres.bench_function(case.name, |b| {
            b.iter(|| black_box(parse_postgres_sql(black_box(case.sql))));
        });
    }
    postgres.finish();
}

/// Guards render (AST -> SQL) throughput in both the canonical and fully-parenthesized
/// modes: a regression means serializing a parsed statement back to SQL got slower.
fn renderer(c: &mut Criterion) {
    let mut group = c.benchmark_group("renderer");
    group.bench_function("canonical", |b| {
        b.iter(|| black_box(render_canonical_sql(black_box(ANALYTIC_SELECT))));
    });
    group.bench_function("parenthesized", |b| {
        b.iter(|| black_box(render_parenthesized_sql(black_box(DEEP_NESTED_SELECT))));
    });
    group.finish();
}

criterion_group!(benches, tokenizer, parser, renderer);
criterion_main!(benches);
