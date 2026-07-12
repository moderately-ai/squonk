// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Adversarial scaling instruction bench: callgrind `Ir` for parsing the worst-case
//! (top ladder width) of each width family under our parser — the deterministic
//! compute counterpart to the cross-platform `dhat` linearity gate.
//!
//! Valgrind-backed, so it runs only where Valgrind does: gated behind
//! `#[cfg(target_os = "linux")]` with an explicit non-Linux `main` that prints a
//! skip message (the same pattern as `perf.rs` / `corpus_instr.rs`). Each family's
//! worst-case SQL is generated in the gungraun `setup` (an unmeasured expression),
//! so building the string never inflates the measured `Ir`; the benchmark function
//! then parses it. A soft 5% regression limit (matching `perf.rs`) keeps a
//! compute-scaling regression on the pathological inputs visible in CI without
//! hard-failing on toolchain drift. The cross-width *linearity* verdict itself is
//! asserted deterministically and cross-platform by `tests/adversarial_scaling.rs`
//! (callgrind `Ir` is unavailable inside a plain test); this bench is the absolute
//! compute-regression signal at the worst case.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[cfg(target_os = "linux")]
mod adversarial;
#[cfg(target_os = "linux")]
mod gungraun_gate;
// `adversarial/mod.rs` imports `HeapSample`/`sample` from this module (it already
// links `sqlparser` directly, so mounting `upstream` alongside adds no dependency).
#[cfg(target_os = "linux")]
mod upstream;

#[cfg(target_os = "linux")]
use adversarial::parse_ours_owned;
#[cfg(target_os = "linux")]
use gungraun::{library_benchmark, library_benchmark_group, main};
#[cfg(target_os = "linux")]
use gungraun_gate::gate_config;
#[cfg(target_os = "linux")]
use squonk_bench::adversarial::{
    WIDTH_LADDER, cte_chain, in_list, many_way_join, operator_chain, values_rows,
};
#[cfg(target_os = "linux")]
use std::hint::black_box;

/// The worst case the dhat ladder reaches — the width where any super-linearity is
/// most visible.
#[cfg(target_os = "linux")]
fn top_width() -> usize {
    *WIDTH_LADDER.last().expect("the width ladder is non-empty")
}

// Each family's worst-case SQL, generated UNMEASURED in `setup`.
#[cfg(target_os = "linux")]
fn operator_chain_top() -> String {
    operator_chain(top_width())
}
#[cfg(target_os = "linux")]
fn many_way_join_top() -> String {
    many_way_join(top_width())
}
#[cfg(target_os = "linux")]
fn in_list_top() -> String {
    in_list(top_width())
}
#[cfg(target_os = "linux")]
fn values_rows_top() -> String {
    values_rows(top_width())
}
#[cfg(target_os = "linux")]
fn cte_chain_top() -> String {
    cte_chain(top_width())
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[bench::operator_chain(operator_chain_top())]
#[bench::many_way_join(many_way_join_top())]
#[bench::in_list(in_list_top())]
#[bench::values_rows(values_rows_top())]
#[bench::cte_chain(cte_chain_top())]
fn parser_width_family(sql: String) -> usize {
    black_box(parse_ours_owned(black_box(&sql)).statements().len())
}

#[cfg(target_os = "linux")]
library_benchmark_group!(
    name = adversarial_gate,
    config = gate_config(),
    benchmarks = [parser_width_family]
);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = adversarial_gate);

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("skipping gungraun adversarial gate: Valgrind-backed benches run on Linux");
}
