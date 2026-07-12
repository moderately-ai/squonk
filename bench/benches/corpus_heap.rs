// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Corpus-scale heap bench: the aggregate heap cost of parsing realistic batches
//! of statements from the vendored conformance corpora, the corpus counterpart to
//! the single-statement `upstream_heap.rs`.
//!
//! Uses the `dhat` crate (cross-platform, deterministic) exactly as
//! `tests/allocations.rs` and `upstream_heap.rs` do, so the aggregate numbers are
//! reproducible run-to-run and can be pinned as a hard gate
//! (`tests/corpus_allocations.rs`). For each shipped preset it parses the whole
//! subset our parser accepts (selected deterministically — see `corpus/mod.rs`) and
//! reports, over that batch:
//!
//! - TRANSIENT: total bytes + blocks allocated to build the owned ASTs (everything,
//!   including freed temporaries) — `dhat`'s `total_*`.
//! - RETAINED: bytes + blocks still live while each owned AST is held — `dhat`'s
//!   `curr_*`. Reported separately from transient because our retained footprint
//!   deliberately includes the source `Arc<str>` + interner (zero-copy spans,
//!   ADR-0002/0005), so it is the footprint of a live parse result, not just nodes.
//! - PEAK: summed high-water live bytes during each parse — `dhat`'s `max_bytes`.
//!
//! The measurement lives in `corpus/mod.rs` (`measure` / `sample` / `Totals`), so
//! this bench and the pinned gate share exactly one path. Output is a per-preset
//! coverage line plus the aggregate metric table; it writes no snapshot — the
//! deterministic gate that fails on a scale regression is the `dhat::HeapStats`
//! pin in `tests/corpus_allocations.rs` (ADR-0016: one perf-gate mechanism per
//! signal).

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

mod corpus;
use corpus::{PRESETS, Subset, Totals, measure, rows};

fn print_header() {
    println!("# corpus-scale parser heap: squonk over the vendored conformance corpora");
    println!(
        "#   corpora  : sqlglot identity, sqllogictest statements, postgres regress-supported"
    );
    println!("#   measured : the subset our parser accepts under each preset (selected by parsing");
    println!(
        "#              each candidate; many corpus lines exceed the surface — that is the point)"
    );
    println!(
        "#   transient: total bytes/blocks allocated to build the ASTs (incl. freed temporaries)"
    );
    println!(
        "#   retained : bytes/blocks live while each AST is held — includes the source Arc<str>"
    );
    println!("#              + interner kept alive for zero-copy spans (ADR-0002/0005)");
    println!("#   peak     : summed high-water live bytes per parse");
    println!("#   numbers are deterministic; the failing gate is tests/corpus_allocations.rs");
}

fn print_table(s: &Subset, totals: &Totals) {
    println!("#");
    println!(
        "# [{}] heap over {} measured statements ({:.1}% of {} candidates)",
        s.preset.label(),
        s.included.len(),
        s.coverage_pct(),
        s.total_candidates,
    );
    for cov in &s.coverage {
        println!(
            "#   {:<28} {:>4}/{:<4} parsed",
            cov.corpus, cov.included, cov.candidates,
        );
    }
    println!("#   {:<18} {:>14}", "metric", "bytes/blocks");
    for (label, value) in rows(totals) {
        println!("#   {label:<18} {value:>14}");
    }
}

fn main() {
    print_header();
    for preset in PRESETS {
        let (s, totals) = measure(preset);
        print_table(&s, &totals);
    }
}
