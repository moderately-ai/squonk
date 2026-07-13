// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! First render-path allocation pin (prod-perf-render-presize-output-buffer).
//!
//! `tests/allocations.rs` pins the parse path; this is the render-side counterpart,
//! guarding `Parsed::to_sql`'s pre-sized buffer so a future change that reintroduces
//! `String::new()`'s empty-buffer doubling chain — or adds a stray allocation
//! anywhere in the shared render loop — fails here. Mirrors that file's dhat idiom:
//! binary-wide allocator, `testing()` profiler scope, arch-gated byte pins.

use squonk::dialect::Ansi;
use squonk::parse_with;
use squonk_bench::ANALYTIC_SELECT;
use std::hint::black_box;

#[derive(Clone, Copy)]
struct ExpectedAllocations {
    total_blocks: u64,
    total_bytes: u64,
    max_blocks: usize,
    max_bytes: usize,
}

fn assert_allocations(name: &str, expected: ExpectedAllocations, f: impl FnOnce()) {
    let _profiler = dhat::Profiler::builder().testing().build();

    f();

    let stats = dhat::HeapStats::get();
    // Same arch-gating convention as `tests/allocations.rs`: block counts are the
    // allocation-COUNT signal (architecture-independent — this pin's whole point is
    // "exactly one allocation", which holds on every arch), while byte totals are
    // gated to the aarch64 dev reference (see `allocation-pins-are-architecture-specific`).
    dhat::assert_eq!(
        stats.total_blocks,
        expected.total_blocks,
        "{name} total blocks"
    );
    dhat::assert_eq!(stats.max_blocks, expected.max_blocks, "{name} max blocks");
    if cfg!(target_arch = "aarch64") {
        dhat::assert_eq!(
            stats.total_bytes,
            expected.total_bytes,
            "{name} total bytes"
        );
        dhat::assert_eq!(stats.max_bytes, expected.max_bytes, "{name} max bytes");
    }
}

#[test]
fn to_sql_renders_the_analytic_select_in_one_allocation() {
    // Parsed OUTSIDE the profiler scope: this pin isolates `to_sql`'s own
    // allocations from the parse that builds the tree it renders.
    let parsed =
        parse_with(ANALYTIC_SELECT, squonk::ParseConfig::new(Ansi)).expect("fixture parses");

    // `ANALYTIC_SELECT` is already canonical (its round-trip is byte-identical), so
    // `with_capacity(source().len())` fits the render exactly: one allocation for
    // the output `String`, never grown. `total_bytes`/`max_bytes` are the fixture's
    // own byte length (137), independent of architecture — no AST node layout is on
    // this path, only a single `Vec<u8>` buffer request — but pinned behind the same
    // `aarch64`-only gate as every other byte pin in this suite for consistency.
    assert_allocations(
        "to_sql_analytic_select",
        ExpectedAllocations {
            total_blocks: 1,
            total_bytes: ANALYTIC_SELECT.len() as u64,
            max_blocks: 1,
            max_bytes: ANALYTIC_SELECT.len(),
        },
        || {
            black_box(parsed.to_sql());
        },
    );
}
