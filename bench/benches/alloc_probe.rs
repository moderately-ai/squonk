// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Allocation-isolation probe — Part 2 of the libpg-gap decomposition
//! (`spike-libpg-gap-decomposition`, `docs/performance.md`).
//!
//! Times OUR Postgres parser building + dropping its owned AST over the SAME both-accept
//! corpus the libpg wall-clock comparison uses (`benches/libpg/mod.rs`), under a
//! compile-time-selectable GLOBAL allocator, so the cross-allocator delta sizes how much
//! of the ~1.6-1.9x libpg raw-parse gap is general-purpose malloc/free overhead
//! (pursuable via an arena) versus intrinsic tree-write + interning + retention (partly
//! inherent). The PRODUCTION allocator is never touched — this is the `alloc_probe` bench
//! binary only.
//!
//! Three modes (one binary each — run all three, compare the printed ns/statement):
//!   cargo bench --bench alloc_probe                                  # system malloc (baseline)
//!   cargo bench --bench alloc_probe --features alloc-probe-mimalloc  # mimalloc (better general-purpose)
//!   cargo bench --bench alloc_probe --features alloc-probe-bump      # never-free bump (arena CEILING)
//! Optional positional args: `<corpus_passes> <case_iters>` (defaults below).
//!
//! Interpretation:
//!   - (system - mimalloc) sizes the malloc/free IMPLEMENTATION overhead recoverable by a
//!     one-line global-allocator swap (no arena, no `unsafe` in production).
//!   - (system - bump) sizes the TOTAL allocation overhead an arena could recover: the
//!     never-free bump models PostgreSQL's palloc (pointer-bump alloc, bulk free), the
//!     same model `theirs_tree_build` shows costs libpg almost nothing.
//!   - the residual `ours/theirs_tree_build` gap that REMAINS under the bump ceiling is
//!     the non-allocation part of the gap (tree-write + interning + retention + core).
//!
//! Wall-clock, so noisy by nature — read it as a sized trend, not a pinned number; capture
//! under `--profile profiling` (never debug — the documented build-profile trap). `harness
//! = false` with a manual timing loop (like `examples/perf_testbed.rs`) rather than
//! criterion, because the bump allocator's bulk-reset is unsound to share with criterion's
//! own long-lived allocations (see the `bump` module).

#![allow(dead_code)]

mod libpg;
mod upstream;

use libpg::{
    NESTED_EXPR, STAR_JOIN, libpg_complex_both_accept, libpg_subset, parse_libpg_tree_build,
    parse_ours_pg, profile_note,
};
use std::hint::black_box;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Global-allocator selection (compile-time, mutually exclusive)
// ---------------------------------------------------------------------------
//
// mimalloc takes priority over bump, so `--all-features` (which enables both) builds a
// SINGLE `#[global_allocator]` (mimalloc) — never two, which would not compile — and the
// `bump` module is then fully `cfg`'d out (no dead code under the `-D warnings` gate).

#[cfg(feature = "alloc-probe-mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(feature = "alloc-probe-bump", not(feature = "alloc-probe-mimalloc")))]
#[global_allocator]
static GLOBAL: bump::Bump = bump::Bump::new();

/// Human-readable name of the active allocator, for the report header.
fn allocator_name() -> &'static str {
    if cfg!(feature = "alloc-probe-mimalloc") {
        "mimalloc (better general-purpose malloc/free)"
    } else if cfg!(feature = "alloc-probe-bump") {
        "never-free bump (arena CEILING: pointer-bump alloc, bulk reset, no per-node free)"
    } else {
        "system (platform default malloc/free — the production allocator)"
    }
}

/// Capture the bump reset point AFTER warm-up (no-op unless the bump allocator is active).
fn mark_bump() {
    #[cfg(all(feature = "alloc-probe-bump", not(feature = "alloc-probe-mimalloc")))]
    GLOBAL.set_mark();
}

/// Bulk-reclaim everything allocated since `mark_bump` (no-op unless the bump is active).
/// Called at the top of each timed pass: this is the bump's "free" side, the palloc
/// context-delete analogue, so it is correctly inside the timed region.
fn reset_bump() {
    #[cfg(all(feature = "alloc-probe-bump", not(feature = "alloc-probe-mimalloc")))]
    GLOBAL.reset();
}

// ---------------------------------------------------------------------------
// The never-free bump allocator (compiled only when it is the active allocator)
// ---------------------------------------------------------------------------

#[cfg(all(feature = "alloc-probe-bump", not(feature = "alloc-probe-mimalloc")))]
#[allow(unsafe_code)]
mod bump {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

    /// One big `System`-backed region (lives forever): `alloc` bumps a cursor, `dealloc`
    /// is a no-op, and the harness calls `reset()` between passes to reclaim in bulk. This
    /// is the closest cheap proxy for PostgreSQL's palloc memory-context model — the model
    /// `theirs_tree_build` shows makes tree retention almost free. Per-node allocation
    /// collapses to a pointer increment; per-node free disappears.
    ///
    /// SOUND HERE ONLY because: the timing loop is single-threaded; every pass drops its
    /// parse results before the next `reset()`, so nothing live is ever reset over; and the
    /// reset point (`mark`) is captured AFTER warm-up, so all long-lived setup / lazy-static
    /// allocations sit BELOW the mark and are never reclaimed. This is a bench proxy, not a
    /// general allocator — it never frees, so it must not be shared with criterion.
    pub struct Bump {
        region: AtomicPtr<u8>,
        cursor: AtomicUsize,
        mark: AtomicUsize,
    }

    /// 1 GiB virtual region. One corpus pass is single-digit MB of never-freed transient,
    /// so this holds a pass with vast margin; the OS commits only touched pages, and
    /// `reset()` between passes keeps the touched set (hence RSS) flat.
    const REGION: usize = 1 << 30;
    /// Region base alignment — ≥ any AST node alignment, so `base + aligned_offset` is
    /// correctly aligned for every sub-allocation.
    const REGION_ALIGN: usize = 4096;

    impl Bump {
        pub const fn new() -> Self {
            Bump {
                region: AtomicPtr::new(std::ptr::null_mut()),
                cursor: AtomicUsize::new(0),
                mark: AtomicUsize::new(0),
            }
        }

        /// The region base, carved from the `System` allocator on first use (never freed).
        fn base(&self) -> *mut u8 {
            let p = self.region.load(Ordering::Acquire);
            if !p.is_null() {
                return p;
            }
            let layout =
                Layout::from_size_align(REGION, REGION_ALIGN).expect("valid region layout");
            // SAFETY: REGION > 0 and the layout is valid; `System.alloc` is the concrete
            // platform allocator, so this does not recurse through this `GlobalAlloc`.
            let fresh = unsafe { System.alloc(layout) };
            assert!(
                !fresh.is_null(),
                "bump probe: System failed to provide the region"
            );
            match self.region.compare_exchange(
                std::ptr::null_mut(),
                fresh,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => fresh,
                Err(winner) => {
                    // Lost the install race: hand our region back, use the winner's.
                    // SAFETY: `fresh`/`layout` are the matching pair from `System.alloc`.
                    unsafe { System.dealloc(fresh, layout) };
                    winner
                }
            }
        }

        /// Capture the current cursor as the reset point. Call AFTER warm-up so long-lived
        /// allocations below the mark are never reclaimed.
        pub fn set_mark(&self) {
            let _ = self.base();
            self.mark
                .store(self.cursor.load(Ordering::Acquire), Ordering::Release);
        }

        /// Reclaim everything allocated since `set_mark` (bulk free).
        pub fn reset(&self) {
            self.cursor
                .store(self.mark.load(Ordering::Acquire), Ordering::Release);
        }
    }

    // SAFETY: `alloc` only ever returns a pointer into `[base, base + REGION)` that is
    // aligned per `layout` and disjoint from concurrently-served requests (the cursor is
    // advanced with a CAS). `dealloc` is intentionally a no-op (see the type docs).
    unsafe impl GlobalAlloc for Bump {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let base = self.base();
            let align = layout.align();
            loop {
                let cur = self.cursor.load(Ordering::Acquire);
                let aligned = (cur + align - 1) & !(align - 1);
                let end = aligned + layout.size();
                if end > REGION {
                    // A single pass should never reach 1 GiB; fail loudly (the caller's
                    // `handle_alloc_error` aborts) rather than hand out OOB memory.
                    return std::ptr::null_mut();
                }
                if self
                    .cursor
                    .compare_exchange_weak(cur, end, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    // SAFETY: `aligned + size <= REGION`, so this stays inside the region.
                    return unsafe { base.add(aligned) };
                }
            }
        }

        unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
            // no-op: the whole region is reclaimed in bulk by `reset()` between passes.
        }
    }
}

// ---------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------

/// Parse the whole corpus once with `f`, dropping each result (no reset — for warm-up).
fn one_pass(f: fn(&str) -> usize, corpus: &[&str]) -> usize {
    let mut sink = 0usize;
    for &sql in corpus {
        sink = sink.wrapping_add(f(sql));
    }
    sink
}

/// Time `passes` corpus passes of `f` after a warm-up; return nanoseconds PER STATEMENT.
/// Each pass resets the bump first, so the bump's bulk-free is inside the timed region (its
/// "free" side — the palloc context-delete analogue).
fn time_corpus(f: fn(&str) -> usize, corpus: &[&str], passes: u64) -> f64 {
    for _ in 0..(passes / 20).max(1) {
        reset_bump();
        black_box(one_pass(f, black_box(corpus)));
    }
    let start = Instant::now();
    for _ in 0..passes {
        reset_bump();
        black_box(one_pass(f, black_box(corpus)));
    }
    let elapsed = start.elapsed().as_nanos() as f64;
    elapsed / (passes as f64 * corpus.len() as f64)
}

/// Time `iters` parses of a single case with `f` after a warm-up; ns per parse.
fn time_case(f: fn(&str) -> usize, sql: &str, iters: u64) -> f64 {
    for _ in 0..(iters / 20).max(1) {
        reset_bump();
        black_box(f(black_box(sql)));
    }
    let start = Instant::now();
    for _ in 0..iters {
        reset_bump();
        black_box(f(black_box(sql)));
    }
    start.elapsed().as_nanos() as f64 / iters as f64
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let passes: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1_000);
    let case_iters: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(500_000);

    // Build the both-accept corpus (same bytes/cut as the libpg wall-clock comparison):
    // the curated PG-regress subset plus every complex-dataset statement both parsers
    // accept, as one flat list. Built BEFORE the bump mark so it is never reset over.
    let subset = libpg_subset();
    let mut corpus: Vec<&str> = subset.included.iter().map(|c| c.sql).collect();
    for (_, batch) in libpg_complex_both_accept() {
        corpus.extend(batch);
    }
    let n = corpus.len();

    println!("# alloc_probe — OUR Postgres parser (owned-AST build + drop), the `ours` series");
    println!("#   build profile : {}", profile_note());
    println!("#   allocator     : {}", allocator_name());
    println!(
        "#   corpus        : {n} both-accept statements (curated PG-regress + complex), same cut as libpg_compare"
    );
    println!("#   measurement   : {passes} corpus passes, {case_iters} iters per cross-check case");
    println!(
        "#   note          : run all three allocator modes; (system - mimalloc) = malloc/free \
         impl overhead, (system - bump) = the arena ceiling on allocation."
    );

    // Warm BEFORE marking, so every lazy-initialized global (interner tables, dialect
    // singletons, stdout buffer, the bump region) is allocated below the mark and survives
    // the per-pass resets. Without this, a lazy static first allocated inside a timed pass
    // would be wiped by the next reset under the bump allocator.
    for _ in 0..3 {
        black_box(one_pass(parse_ours_pg, &corpus));
        black_box(one_pass(parse_libpg_tree_build, &corpus));
        black_box(parse_ours_pg(NESTED_EXPR));
        black_box(parse_libpg_tree_build(NESTED_EXPR));
        black_box(parse_ours_pg(STAR_JOIN));
        black_box(parse_libpg_tree_build(STAR_JOIN));
    }
    mark_bump();

    // `theirs_tree_build` (libpg_query `raw_parser`, palloc, no protobuf) as a same-harness
    // reference. It allocates its tree in C (palloc), NOT through this binary's Rust global
    // allocator, so it is ~constant across the three allocator modes — a control that
    // confirms the swap only moves OUR side. `ours/theirs` shrinking from system -> mimalloc
    // -> bump is the answer to "is our core competitive with Bison once allocation is held
    // equal?" (at `bump`, our allocation is ~free and libpg's palloc is also a bump arena).
    let rows = [
        (
            "corpus (per stmt)",
            time_corpus(parse_ours_pg, &corpus, passes),
            time_corpus(parse_libpg_tree_build, &corpus, passes),
        ),
        (
            "nested_expr (alloc ~24%)",
            time_case(parse_ours_pg, NESTED_EXPR, case_iters),
            time_case(parse_libpg_tree_build, NESTED_EXPR, case_iters),
        ),
        (
            "star_join (alloc ~26%)",
            time_case(parse_ours_pg, STAR_JOIN, case_iters),
            time_case(parse_libpg_tree_build, STAR_JOIN, case_iters),
        ),
    ];
    println!(
        "{:<26} {:>12} {:>16} {:>14}",
        "case", "ours ns", "theirs_tb ns", "ours/theirs_tb"
    );
    for (name, ours_ns, theirs_ns) in rows {
        println!(
            "{name:<26} {ours_ns:>12.1} {theirs_ns:>16.1} {:>14.2}",
            ours_ns / theirs_ns
        );
    }
}
