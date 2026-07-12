// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Instruction-count half of the apples-to-apples comparison vs upstream
//! `sqlparser`.
//!
//! Mirrors `perf.rs`: gungraun/callgrind, which is deterministic and
//! machine-independent, so it is the primary CI regression signal. For each mapped
//! dialect pair we emit an `ours` and a `theirs` benchmark per statement; the
//! instruction ratio is `ours / theirs` read off the two, and gungraun's historical
//! diffing tracks each side's drift over time.
//!
//! Unlike `perf.rs` this group sets NO `soft_limits`: it is a *tracker*, not a hard
//! gate — our absolute instruction count is already gated by `perf.rs`, and a
//! thresholded gate on the ours/theirs ratio is a follow-up to coordinate with the
//! perf-gate work. Valgrind is Linux-only, so the whole group is `cfg`-gated like
//! `perf.rs`; the statement set is the subset confirmed (by `upstream_compare` /
//! `upstream_heap` coverage on any platform) to parse on BOTH sides under BOTH
//! pairs.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[cfg(target_os = "linux")]
mod upstream;

#[cfg(target_os = "linux")]
use gungraun::{library_benchmark, library_benchmark_group, main};
#[cfg(target_os = "linux")]
use std::hint::black_box;
#[cfg(target_os = "linux")]
use upstream::{
    DEEP_NESTED_SELECT, MULTI_STATEMENT_SELECTS, PG_CROSS_JOIN, PG_CTE, PG_JOIN_ON, PG_UNION_ALL,
    Pair, SET_SELECT, SIMPLE_SELECT, compare_config, parse_ours, parse_theirs,
};

#[cfg(target_os = "linux")]
#[library_benchmark]
#[benches::stmts(
    SIMPLE_SELECT,
    SET_SELECT,
    DEEP_NESTED_SELECT,
    MULTI_STATEMENT_SELECTS,
    PG_CTE,
    PG_UNION_ALL,
    PG_CROSS_JOIN,
    PG_JOIN_ON
)]
fn parse_ours_ansi(sql: &str) -> usize {
    black_box(parse_ours(Pair::AnsiGeneric, black_box(sql)))
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[benches::stmts(
    SIMPLE_SELECT,
    SET_SELECT,
    DEEP_NESTED_SELECT,
    MULTI_STATEMENT_SELECTS,
    PG_CTE,
    PG_UNION_ALL,
    PG_CROSS_JOIN,
    PG_JOIN_ON
)]
fn parse_theirs_generic(sql: &str) -> usize {
    black_box(parse_theirs(Pair::AnsiGeneric, black_box(sql)))
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[benches::stmts(
    SIMPLE_SELECT,
    SET_SELECT,
    DEEP_NESTED_SELECT,
    MULTI_STATEMENT_SELECTS,
    PG_CTE,
    PG_UNION_ALL,
    PG_CROSS_JOIN,
    PG_JOIN_ON
)]
fn parse_ours_postgres(sql: &str) -> usize {
    black_box(parse_ours(Pair::PostgresPostgres, black_box(sql)))
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[benches::stmts(
    SIMPLE_SELECT,
    SET_SELECT,
    DEEP_NESTED_SELECT,
    MULTI_STATEMENT_SELECTS,
    PG_CTE,
    PG_UNION_ALL,
    PG_CROSS_JOIN,
    PG_JOIN_ON
)]
fn parse_theirs_postgres(sql: &str) -> usize {
    black_box(parse_theirs(Pair::PostgresPostgres, black_box(sql)))
}

#[cfg(target_os = "linux")]
library_benchmark_group!(
    name = upstream_instr,
    config = compare_config(),
    benchmarks = [
        parse_ours_ansi,
        parse_theirs_generic,
        parse_ours_postgres,
        parse_theirs_postgres
    ]
);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = upstream_instr);

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!(
        "skipping gungraun upstream instruction comparison: Valgrind-backed benches run on Linux"
    );
}
