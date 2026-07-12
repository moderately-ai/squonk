// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Arena-vs-malloc allocation microbenchmark for `prod-memory-root-arena-spike`.
//!
//! A release-profile samply profile of our parser shows ALLOCATION is ~21% of
//! parse CPU on the expression-heavy `nested_expr` case (`nanov2_free` 9.5% +
//! `nanov2_malloc` 5.8% + `bzero` 3% + `malloc_zone` 1.8% + `free` 1.3%) — the
//! single biggest remaining compute lever after the peek hot-path win. Our AST
//! builds that cost by allocating per node: every recursive `Expr` child is a
//! separate `Box` (ADR-0007), and `ThinVec` lists each take a heap block. ADR-0007
//! keeps a `Parsed`-root-owned bump arena as a *measure-first* fallback. This
//! example measures the UPPER BOUND of that arena's win WITHOUT touching the real
//! AST: it models the per-node `Box` pattern against a bump arena and reports the
//! alloc/free CPU delta.
//!
//! It models the hot node faithfully: `Expr` is 40 B and fully boxed in its
//! recursive variants (`BinaryOp { left: Box<Expr>, right: Box<Expr>, .. }`), and
//! `Meta` (Span{u32,u32} + NodeId{u32}) is 12 B on every node. The boxed tree here
//! mirrors that shape; the arena variant replaces each `Box<Node>` (an 8 B owning
//! pointer + its own heap block) with a `u32` index into one root-owned `Vec`
//! region — exactly the id-based scheme a lifetime-free `Parsed`-owned arena would
//! use (ADR-0001 forbids a public `'arena` lifetime, so an owning pointer arena is
//! out; an index arena is the only lifetime-free shape).
//!
//! What the arena changes, and what this measures:
//!   - status quo  : N nodes  -> N mallocs  + N frees  (+ per-block bzero)
//!   - bump arena  : N nodes  -> O(log N) Vec-growth reallocs + 1 bulk free
//!
//! The gap is the alloc/free CPU the arena could recover. The arena cannot recover
//! the per-parse source `Arc<str>` or the interner growth (a handful of blocks per
//! parse, not per node); those are out of its reach and out of this model. This is
//! the per-NODE allocation slice — the dominant, arena-addressable part of the 21%.
//!
//! Deterministic by construction (fixed tree shapes, fixed iteration counts, no
//! RNG, no timing-derived control flow), so a re-run on a release-class build
//! reproduces the same ratios. Build optimized — NEVER measure alloc cost in a
//! debug build (the workspace `opt-level=1` optimizes deps but leaves our code at
//! `opt-level=0`, pitting an unoptimized builder against an optimized allocator):
//!
//! ```text
//! cargo run --profile profiling --example arena_spike -p squonk-bench
//! cargo run --release          --example arena_spike -p squonk-bench
//! ```

use std::hint::black_box;
use std::time::Instant;

use squonk_bench::time_ns;

/// 12-byte node metadata, byte-for-byte the shape of the real `Meta`
/// (`Span { start: u32, end: u32 }` + `NodeId(NonZeroU32)`), so every modelled node
/// pays the same inline metadata the real AST does.
#[derive(Clone, Copy)]
struct Meta {
    span_start: u32,
    span_end: u32,
    node_id: u32,
}

impl Meta {
    fn new(i: u32) -> Self {
        Self {
            span_start: i,
            span_end: i.wrapping_add(1),
            node_id: i | 1,
        }
    }

    /// Fold every field into the traversal sink so none reads as dead, and so the
    /// optimizer cannot drop the metadata loads the real renderer/visitor would do.
    fn fold(self) -> u64 {
        u64::from(self.span_start)
            ^ (u64::from(self.span_end) << 16)
            ^ (u64::from(self.node_id) << 32)
    }
}

// ---------------------------------------------------------------------------
// Status quo: a per-node `Box`ed recursive tree (one heap block per node).
// ---------------------------------------------------------------------------
//
// Shaped like the hot `Expr` variants: `Binary` mirrors `Expr::BinaryOp` (two boxed
// children + an op + `Meta`); `Leaf` mirrors `Expr::Literal` (a payload + `Meta`).
// With the niche optimization on the non-null `Box` pointers the enum is ~32 B —
// the same small-node malloc size class as the real 40 B `Expr`.

enum BoxNode {
    Leaf {
        value: u64,
        meta: Meta,
    },
    Binary {
        left: Box<BoxNode>,
        right: Box<BoxNode>,
        op: u8,
        meta: Meta,
    },
}

/// Build a balanced boxed tree of `depth` (2^(depth+1) − 1 nodes), one `Box::new`
/// per node — the status-quo allocation pattern.
fn build_boxed(depth: u32, counter: &mut u32) -> Box<BoxNode> {
    *counter = counter.wrapping_add(1);
    let id = *counter;
    if depth == 0 {
        Box::new(BoxNode::Leaf {
            value: u64::from(id),
            meta: Meta::new(id),
        })
    } else {
        let left = build_boxed(depth - 1, counter);
        let right = build_boxed(depth - 1, counter);
        Box::new(BoxNode::Binary {
            left,
            right,
            op: (id & 0xff) as u8,
            meta: Meta::new(id),
        })
    }
}

/// Traverse the boxed tree, folding payload + metadata into a sink. Pointer-chasing
/// across N independent heap blocks — the read-locality the arena variant improves.
fn sum_boxed(node: &BoxNode) -> u64 {
    match node {
        BoxNode::Leaf { value, meta } => value.wrapping_add(meta.fold()),
        BoxNode::Binary {
            left,
            right,
            op,
            meta,
        } => sum_boxed(left)
            .wrapping_add(sum_boxed(right))
            .wrapping_add(u64::from(*op))
            .wrapping_add(meta.fold()),
    }
}

// ---------------------------------------------------------------------------
// Candidate: a bump arena (one growing `Vec` region; children by `u32` index).
// ---------------------------------------------------------------------------
//
// The lifetime-free arena shape ADR-0001 permits: nodes live in one root-owned
// region and reference children by index, not by an owning `Box` or a borrowed
// `&'arena` pointer. Building bump-allocates (a `Vec::push`, amortized); the whole
// region frees in ONE deallocation when the root drops.

enum ArenaNode {
    Leaf {
        value: u64,
        meta: Meta,
    },
    Binary {
        left: u32,
        right: u32,
        op: u8,
        meta: Meta,
    },
}

/// Build the same balanced tree into `arena` post-order, returning the root index.
/// Each node is one `push` (a bump into the region), no per-node heap block.
fn build_arena(depth: u32, arena: &mut Vec<ArenaNode>, counter: &mut u32) -> u32 {
    *counter = counter.wrapping_add(1);
    let id = *counter;
    if depth == 0 {
        let idx = arena.len() as u32;
        arena.push(ArenaNode::Leaf {
            value: u64::from(id),
            meta: Meta::new(id),
        });
        idx
    } else {
        let left = build_arena(depth - 1, arena, counter);
        let right = build_arena(depth - 1, arena, counter);
        let idx = arena.len() as u32;
        arena.push(ArenaNode::Binary {
            left,
            right,
            op: (id & 0xff) as u8,
            meta: Meta::new(id),
        });
        idx
    }
}

/// Traverse the arena tree from `idx`, folding payload + metadata into a sink. The
/// region is contiguous, so children sit near their parent — better cache locality
/// than chasing N independent boxes.
fn sum_arena(arena: &[ArenaNode], idx: u32) -> u64 {
    match &arena[idx as usize] {
        ArenaNode::Leaf { value, meta } => value.wrapping_add(meta.fold()),
        ArenaNode::Binary {
            left,
            right,
            op,
            meta,
        } => sum_arena(arena, *left)
            .wrapping_add(sum_arena(arena, *right))
            .wrapping_add(u64::from(*op))
            .wrapping_add(meta.fold()),
    }
}

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

fn node_count(depth: u32) -> u64 {
    (1u64 << (depth + 1)) - 1
}

/// One row of the build+drop comparison for a given tree size.
fn bench_build_drop(depth: u32, iters: u64) {
    let nodes = node_count(depth);

    // Status quo: build a boxed tree and drop it — N mallocs + N frees + per-block
    // zeroing, every iteration.
    let boxed = time_ns(iters, || {
        let mut counter = 0u32;
        let tree = build_boxed(depth, &mut counter);
        let s = sum_boxed(&tree); // touch it so the build cannot be elided
        drop(tree);
        s
    });

    // Arena, pre-reserved: the upper bound — one allocation up front, N bumps, one
    // free. Models a root-owned arena that knows (or over-estimates) its size.
    let arena_reserved = time_ns(iters, || {
        let mut arena = Vec::with_capacity(nodes as usize);
        let mut counter = 0u32;
        let root = build_arena(depth, &mut arena, &mut counter);
        let s = sum_arena(&arena, root);
        drop(arena);
        s
    });

    // Arena, no reserve: the realistic case — the region grows by amortized
    // doubling (O(log N) reallocs), still one free. Shows growth cost is negligible
    // next to N individual mallocs.
    let arena_grow = time_ns(iters, || {
        let mut arena = Vec::new();
        let mut counter = 0u32;
        let root = build_arena(depth, &mut arena, &mut counter);
        let s = sum_arena(&arena, root);
        drop(arena);
        s
    });

    println!(
        "  {depth:>5} {nodes:>9} {boxed:>12.1} {arena_reserved:>12.1} {arena_grow:>12.1} {:>9.2}x {:>9.2}x",
        boxed / arena_reserved,
        boxed / arena_grow,
    );
}

/// Isolate the FREE side: pre-build a batch of trees (untimed), then time dropping
/// the whole batch. Boxed drop = N frees per tree; arena drop = 1 free per tree.
/// This is the `nanov2_free` (9.5%) slice the arena collapses hardest.
fn bench_free_isolation(depth: u32, batch: usize) {
    let nodes = node_count(depth);

    // Boxed: build `batch` trees, hold them, then time their destruction.
    let boxed_free = {
        let mut counter = 0u32;
        let mut held: Vec<Box<BoxNode>> = (0..batch)
            .map(|_| build_boxed(depth, &mut counter))
            .collect();
        black_box(&held);
        let start = Instant::now();
        held.clear(); // N frees per tree, `batch` times
        let elapsed = start.elapsed().as_nanos() as f64;
        black_box(&held);
        elapsed / batch as f64
    };

    // Arena: build `batch` regions, hold them, then time their destruction.
    let arena_free = {
        let mut counter = 0u32;
        let mut held: Vec<Vec<ArenaNode>> = (0..batch)
            .map(|_| {
                let mut arena = Vec::with_capacity(nodes as usize);
                build_arena(depth, &mut arena, &mut counter);
                arena
            })
            .collect();
        black_box(&held);
        let start = Instant::now();
        held.clear(); // 1 free per region, `batch` times
        let elapsed = start.elapsed().as_nanos() as f64;
        black_box(&held);
        elapsed / batch as f64
    };

    println!(
        "  {depth:>5} {nodes:>9} {boxed_free:>14.1} {arena_free:>14.1} {:>11.2}x",
        boxed_free / arena_free,
    );
}

/// Read-side parity: build once (untimed), then time repeated traversals. Shows the
/// index indirection does not cost reads — the contiguous region is friendlier to
/// the cache than chasing N independent boxes.
fn bench_traverse(depth: u32, iters: u64) {
    let nodes = node_count(depth);

    let mut counter = 0u32;
    let boxed_tree = build_boxed(depth, &mut counter);
    let boxed = time_ns(iters, || sum_boxed(&boxed_tree));

    let mut counter = 0u32;
    let mut arena = Vec::with_capacity(nodes as usize);
    let root = build_arena(depth, &mut arena, &mut counter);
    let arena_t = time_ns(iters, || sum_arena(&arena, root));

    println!(
        "  {depth:>5} {nodes:>9} {boxed:>12.1} {arena_t:>12.1} {:>9.2}x",
        boxed / arena_t,
    );
}

fn main() {
    use std::mem::size_of;
    println!("# arena-vs-malloc allocation spike (prod-memory-root-arena-spike)\n");
    println!("Modelled node sizes (size_of):");
    println!(
        "  BoxNode (per-node Box, status quo) = {:>2} B   (real Expr = 40 B, fully boxed)",
        size_of::<BoxNode>()
    );
    println!(
        "  ArenaNode (u32-indexed, candidate) = {:>2} B   (Box<Node> 8 B -> u32 index 4 B)",
        size_of::<ArenaNode>()
    );
    println!(
        "  Meta (inline on every node)        = {:>2} B",
        size_of::<Meta>()
    );

    // Representative tree sizes. A single complex statement parses to hundreds of
    // nodes; depth 8..=14 spans ~0.5k..~32k nodes, bracketing a statement up to a
    // whole corpus batch. Iteration counts are fixed (deterministic) and sized so
    // each cell does tens of ms of work on a release build.
    println!("\n## build + drop  (alloc + free, the realistic parse-shaped cost), ns/parse");
    println!(
        "  {:>5} {:>9} {:>12} {:>12} {:>12} {:>10} {:>10}",
        "depth", "nodes", "boxed ns", "arena ns", "arena+grow", "rsv speedup", "grow speedup"
    );
    for &(depth, iters) in &[(8u32, 40_000u64), (10, 12_000), (12, 3_000), (14, 800)] {
        bench_build_drop(depth, iters);
    }

    println!("\n## free isolation  (drop only — boxed: N frees/tree; arena: 1 free/tree), ns/tree");
    println!(
        "  {:>5} {:>9} {:>14} {:>14} {:>11}",
        "depth", "nodes", "boxed free ns", "arena free ns", "speedup"
    );
    for &(depth, batch) in &[(8u32, 4_000usize), (10, 1_500), (12, 400), (14, 120)] {
        bench_free_isolation(depth, batch);
    }

    println!("\n## traverse  (read-side parity — index indirection vs pointer chase), ns/walk");
    println!(
        "  {:>5} {:>9} {:>12} {:>12} {:>9}",
        "depth", "nodes", "boxed ns", "arena ns", "speedup"
    );
    for &(depth, iters) in &[(8u32, 40_000u64), (10, 12_000), (12, 3_000), (14, 800)] {
        bench_traverse(depth, iters);
    }

    println!(
        "\nReading the table: 'rsv speedup' = boxed / arena(reserved) for build+drop; >1 means\n\
         the arena is faster. The free-isolation speedup is the upper bound on the `nanov2_free`\n\
         9.5% slice; the build+drop speedup is the bound on the whole ~21% allocation share."
    );
}
