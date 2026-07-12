// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Ours-only absolute-instruction regression gate for the three production code paths
//! — tokenizer, parser (ANSI + Postgres), renderer — over fixed single-statement
//! fixtures. `perf_gate` is the hard signal: gungraun/callgrind `Ir` is deterministic
//! and machine-independent, so a soft 5% regression on any statement's instruction
//! count is caught in CI (ADR-0016 — the wall-clock `ast.rs` tracks the same paths but
//! never fails). Valgrind is Linux-only, so the whole group is `cfg`-gated with a
//! non-Linux skip `main`. No external parser is measured; every arm runs only
//! `squonk` (the ours-vs-external compute comparison is `upstream_instr` /
//! `libpg_instr`).

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[cfg(target_os = "linux")]
mod gungraun_gate;

#[cfg(target_os = "linux")]
use gungraun::{library_benchmark, library_benchmark_group, main};
#[cfg(target_os = "linux")]
use gungraun_gate::gate_config;
#[cfg(target_os = "linux")]
use squonk_bench::{
    ANALYTIC_SELECT, DDL_INDEX, DDL_SLOT, DEEP_NESTED_SELECT, DML_DELETE, DML_SLOT, DML_UPDATE,
    MULTI_STATEMENT_SELECTS, SET_SELECT, SIMPLE_SELECT, parse_ansi_sql, parse_postgres_sql,
    render_canonical_sql, render_parenthesized_sql, tokenize_sql,
};
#[cfg(target_os = "linux")]
use std::hint::black_box;

#[cfg(target_os = "linux")]
#[library_benchmark]
#[benches::queries(
    SIMPLE_SELECT,
    ANALYTIC_SELECT,
    SET_SELECT,
    DDL_SLOT,
    DDL_INDEX,
    DML_SLOT,
    DML_UPDATE,
    DEEP_NESTED_SELECT,
    MULTI_STATEMENT_SELECTS
)]
fn tokenizer_gate(sql: &str) -> usize {
    black_box(tokenize_sql(black_box(sql)))
}

#[cfg(target_os = "linux")]
#[library_benchmark]
// DDL/DML join the SELECT cases here so the parser surface is gated end to end,
// not just at the tokenizer (ADR-0016). CREATE INDEX (DDL_INDEX) is absent: it
// has no statement grammar yet, so it stays in `tokenizer_gate` only.
#[benches::queries(
    SIMPLE_SELECT,
    ANALYTIC_SELECT,
    SET_SELECT,
    DDL_SLOT,
    DML_SLOT,
    DML_UPDATE,
    DML_DELETE,
    DEEP_NESTED_SELECT,
    MULTI_STATEMENT_SELECTS
)]
fn parser_ansi_gate(sql: &str) -> usize {
    black_box(parse_ansi_sql(black_box(sql)))
}

#[cfg(target_os = "linux")]
#[library_benchmark]
// Mirrors `parser_ansi_gate`: the Postgres dialect walks the same DDL/DML paths,
// so it is gated over the same corpus (ADR-0016).
#[benches::queries(
    SIMPLE_SELECT,
    ANALYTIC_SELECT,
    SET_SELECT,
    DDL_SLOT,
    DML_SLOT,
    DML_UPDATE,
    DML_DELETE,
    DEEP_NESTED_SELECT,
    MULTI_STATEMENT_SELECTS
)]
fn parser_postgres_gate(sql: &str) -> usize {
    black_box(parse_postgres_sql(black_box(sql)))
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[benches::queries(ANALYTIC_SELECT, MULTI_STATEMENT_SELECTS)]
fn renderer_canonical_gate(sql: &str) -> usize {
    black_box(render_canonical_sql(black_box(sql)))
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[bench::deep_nesting(DEEP_NESTED_SELECT)]
fn renderer_parenthesized_gate(sql: &str) -> usize {
    black_box(render_parenthesized_sql(black_box(sql)))
}

#[cfg(target_os = "linux")]
library_benchmark_group!(
    name = perf_gate,
    config = gate_config(),
    benchmarks = [
        tokenizer_gate,
        parser_ansi_gate,
        parser_postgres_gate,
        renderer_canonical_gate,
        renderer_parenthesized_gate
    ]
);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = perf_gate);

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("skipping gungraun perf gate: Valgrind-backed benches run on Linux");
}
