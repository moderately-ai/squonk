// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Instruction-count half of the apples-to-apples COMPUTE comparison vs
//! `libpg_query` — the real PostgreSQL parser, in-process.
//!
//! Mirrors `upstream_instr.rs`: gungraun/callgrind, which is deterministic and
//! machine-independent, so it is the primary compute regression signal. Callgrind
//! instruments at the machine-instruction level, so it counts the C parser's
//! instructions too — making `ours / theirs` here a genuine "are we faster than the
//! reference C parser" number, not a Rust-only one. For the fixed PostgreSQL-valid
//! statement set we emit an `ours` (our Postgres parser) and a `theirs`
//! (libpg_query) benchmark per statement.
//!
//! Like `upstream_instr.rs` this group sets NO `soft_limits`: it is a *tracker*, not
//! a hard gate — our absolute instruction count is already gated by `perf.rs`, and a
//! thresholded ours/theirs ratio gate is follow-up perf-gate work. Valgrind is
//! Linux-only, so the whole group is `cfg`-gated like `upstream_instr.rs`; the
//! statement set is standard SQL the PostgreSQL parser accepts (and our parser
//! accepts under the Postgres dialect — the same fixtures `upstream_instr.rs` runs).
//!
//! The `theirs` cost is `pg_query::parse` (C parse + protobuf round-trip + metadata
//! walk), so it overstates libpg_query's pure-parse instructions — a conservative
//! bias documented in `libpg/mod.rs`. The memory axis is intentionally absent (C
//! allocations are invisible to dhat; see `libpg/mod.rs`).

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[cfg(target_os = "linux")]
mod libpg;
#[cfg(target_os = "linux")]
mod upstream;

#[cfg(target_os = "linux")]
use gungraun::{library_benchmark, library_benchmark_group, main};
#[cfg(target_os = "linux")]
use libpg::{parse_libpg, parse_libpg_parse_only, parse_libpg_tree_build, parse_ours_pg};
#[cfg(target_os = "linux")]
use std::hint::black_box;
#[cfg(target_os = "linux")]
use upstream::{
    DEEP_NESTED_SELECT, MULTI_STATEMENT_SELECTS, PG_CROSS_JOIN, PG_CTE, PG_JOIN_ON, PG_UNION_ALL,
    SET_SELECT, SIMPLE_SELECT, compare_config,
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
fn parse_ours_postgres(sql: &str) -> usize {
    black_box(parse_ours_pg(black_box(sql)))
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
fn parse_libpg_postgres(sql: &str) -> usize {
    black_box(parse_libpg(black_box(sql)))
}

// The pure-parse lower bound (tree discarded — no protobuf, no metadata walk), so
// the instruction comparison brackets libpg_query's cost the same way the wall-clock
// bench does and the protobuf/metadata overhead cannot read as a parser-speed win.
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
fn parse_libpg_parse_only_postgres(sql: &str) -> usize {
    black_box(parse_libpg_parse_only(black_box(sql)))
}

// The FAIR owned-tree series: libpg_query BUILDS its palloc `List*`/`Node*` tree (internal
// `pg_query_raw_parse`) then bulk-frees it, NO protobuf — the honest middle between the
// discard lower bound and the protobuf upper bound (see `libpg/mod.rs`), and the
// deterministic Ir counterpart of the wall-clock `theirs_tree_build` series.
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
fn parse_libpg_tree_build_postgres(sql: &str) -> usize {
    black_box(parse_libpg_tree_build(black_box(sql)))
}

#[cfg(target_os = "linux")]
library_benchmark_group!(
    name = libpg_instr,
    config = compare_config(),
    benchmarks = [
        parse_ours_postgres,
        parse_libpg_postgres,
        parse_libpg_tree_build_postgres,
        parse_libpg_parse_only_postgres
    ]
);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = libpg_instr);

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!(
        "skipping gungraun libpg_query instruction comparison: Valgrind-backed benches run on Linux"
    );
}
