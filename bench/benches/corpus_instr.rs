// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Corpus-scale instruction bench: aggregate callgrind `Ir` for parsing realistic
//! batches of statements from the vendored conformance corpora — the corpus
//! counterpart to the single-statement `perf.rs` gate.
//!
//! Valgrind-backed, so it runs only where Valgrind does: gated behind
//! `#[cfg(target_os = "linux")]` with an explicit non-Linux `main` that prints a
//! skip message (same pattern as `perf.rs`). Each preset's parseable subset is
//! built in the gungraun `setup` (an unmeasured expression — see `corpus/mod.rs`'s
//! `included_sql`), so subset selection never inflates the measured count; the
//! benchmark function then parses that whole batch in one call, and gungraun
//! reports the aggregate `Ir` over it. A soft 5% regression limit (matching
//! `perf.rs`) keeps a corpus-scale instruction regression visible in CI without
//! hard-failing on toolchain drift.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[cfg(target_os = "linux")]
mod corpus;
#[cfg(target_os = "linux")]
mod gungraun_gate;

#[cfg(target_os = "linux")]
use corpus::{Preset, included_sql, parse_subset};
#[cfg(target_os = "linux")]
use gungraun::{library_benchmark, library_benchmark_group, main};
#[cfg(target_os = "linux")]
use gungraun_gate::gate_config;
#[cfg(target_os = "linux")]
use std::hint::black_box;

// The parseable subset for each preset, built UNMEASURED in `setup` so the `Ir`
// count is the aggregate cost of parsing the batch, not of selecting it.
#[cfg(target_os = "linux")]
fn ansi_corpus_subset() -> Vec<&'static str> {
    included_sql(Preset::Ansi)
}

#[cfg(target_os = "linux")]
fn postgres_corpus_subset() -> Vec<&'static str> {
    included_sql(Preset::Postgres)
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[bench::corpus(ansi_corpus_subset())]
fn parser_ansi_corpus(subset: Vec<&'static str>) -> usize {
    black_box(parse_subset(Preset::Ansi, black_box(&subset)))
}

#[cfg(target_os = "linux")]
#[library_benchmark]
#[bench::corpus(postgres_corpus_subset())]
fn parser_postgres_corpus(subset: Vec<&'static str>) -> usize {
    black_box(parse_subset(Preset::Postgres, black_box(&subset)))
}

#[cfg(target_os = "linux")]
library_benchmark_group!(
    name = corpus_gate,
    config = gate_config(),
    benchmarks = [parser_ansi_corpus, parser_postgres_corpus]
);

#[cfg(target_os = "linux")]
main!(library_benchmark_groups = corpus_gate);

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("skipping gungraun corpus gate: Valgrind-backed benches run on Linux");
}
