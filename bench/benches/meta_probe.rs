// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Per-node metadata-cost probe — Part 1 of
//! `spike-per-node-metadata-cost-and-optionality` (`docs/performance.md`).
//!
//! Every parsed AST node carries a `Meta { span: Span, node_id: NodeId }` — 12 bytes
//! (`Span` 8 + `NodeId` 4), `Copy`, written by the parser's single `make_meta`
//! chokepoint. The libpg gap-decomposition (`docs/performance.md`)
//! left this cost unmeasured, lumped into the "AST richness" residual. This probe
//! isolates it on two axes, with NO production change (this is a bench binary only —
//! the production AST and the default parse are untouched):
//!
//!   1. SIZE — the bytes/node `Meta` adds (a constant 12) and the total over the
//!      both-accept corpus (× the measured node count), plus `Meta` as a fraction of
//!      each node type's `size_of`. This is the dominant axis: 12 B on a 40 B `Expr`
//!      is ~30%, on a 20 B `Ident` ~60%. It feeds allocation volume + cache density,
//!      but that allocation effect OVERLAPS the gap-decomposition's arena/fast-malloc
//!      slice (measured at current node sizes) — see the notes for the de-overlap.
//!
//!   2. COMPUTE — the parse-time cost of POPULATING the metadata: per node, a span
//!      union (the node's start span ∪ its last token's span) plus a node-id counter
//!      bump. We do NOT expect this to be large — it is a couple of integer ops and a
//!      12-byte copy. It is measured as an isolated micro-A/B in the same machine
//!      state: the real `make_meta` op sequence in a loop (`META` arm) vs a
//!      constant-`Meta` baseline (`BASE` arm), the delta × the corpus node count → a
//!      parse-time share. This is a tight BOUND, not an in-situ feature-gated A/B (the
//!      metadata population lives in the `parser` crate and a stub feature would touch
//!      `crates/squonk/Cargo.toml`, outside this spike's bench scope; the cost is
//!      small enough that an isolated op-cost × node-count is both sufficient and
//!      crisper than a noise-dominated in-situ delta). Cross-checked against the
//!      samply self-time tables in `docs/performance.md`, where
//!      `make_meta`/`next_node_id` do not surface as a category ⇒ below ~2% of parse.
//!
//! Wall-clock, so noisy by nature — read it as a sized trend, not a pinned number;
//! capture under `--profile profiling` (never debug — the documented build-profile
//! trap). `harness = false` with a manual timing loop, like `alloc_probe.rs` and
//! `examples/perf_testbed.rs`, because the micro-A/B is not a criterion-shaped bench.
//!
//!   cargo bench --profile profiling --bench meta_probe                  # full report
//!   cargo bench --profile profiling --bench meta_probe -- <passes> <micro_iters>

#![allow(dead_code)]

mod libpg;
mod upstream;

use libpg::{
    NESTED_EXPR, OURS_PAIR, STAR_JOIN, libpg_complex_both_accept, libpg_subset, parse_ours_pg,
    profile_note,
};
use squonk_ast::generated::{NodeIdWalk, Visit};
use squonk_ast::{Expr, Ident, Literal, Meta, NodeId, Span, Statement};
use std::hint::black_box;
use std::mem::size_of;
use std::time::Instant;
use upstream::parse_ours_owned;

// ---------------------------------------------------------------------------
// SIZE axis
// ---------------------------------------------------------------------------

/// Count the id-bearing (`Meta`-carrying) nodes in `sql`'s parsed tree by walking it
/// with the sourcegen `NodeIdWalk` (one recorded `Meta` per node). This is the
/// RETAINED node count — exactly the nodes that occupy memory, so it is the right
/// multiplier for the per-node `Meta` byte cost. (The parser's id counter high-water
/// can exceed this when speculative parsing mints ids for nodes it later discards, so
/// the parse-time COMPUTE count is a touch higher; noted in the report.)
fn count_nodes(sql: &str) -> usize {
    let parsed = parse_ours_owned(OURS_PAIR, sql);
    let mut walk = NodeIdWalk::default();
    for statement in parsed.statements() {
        walk.visit_statement(statement);
    }
    walk.metas.len()
}

/// `Meta` as a percentage of a node type's total `size_of`.
fn meta_pct(node_size: usize) -> f64 {
    100.0 * size_of::<Meta>() as f64 / node_size as f64
}

fn report_sizes() {
    println!("## SIZE — `Meta` bytes per node (size_of, exact)");
    println!(
        "#   Span = {} B, NodeId = {} B  =>  Meta = {} B per node (Copy)",
        size_of::<Span>(),
        size_of::<NodeId>(),
        size_of::<Meta>(),
    );
    println!("#   Meta as a fraction of representative node types (NoExt layout):");
    println!(
        "{:<14} {:>10} {:>12} {:>10}",
        "node", "size_of", "of which Meta", "Meta %"
    );
    for (name, size) in [
        ("Expr", size_of::<Expr>()),
        ("Statement", size_of::<Statement>()),
        ("Ident", size_of::<Ident>()),
        ("Literal", size_of::<Literal>()),
    ] {
        println!(
            "{:<14} {:>9} B {:>11} B {:>9.0}%",
            name,
            size,
            size_of::<Meta>(),
            meta_pct(size),
        );
    }
}

// ---------------------------------------------------------------------------
// COMPUTE axis — the isolated micro-A/B
// ---------------------------------------------------------------------------

/// A constant `Meta` for the BASE arm (the metadata field still has to exist + be
/// copied into the node; only the COMPUTATION — span union + id bump — is removed).
fn const_meta() -> Meta {
    Meta::new(Span::new(0, 1), NodeId::new(1).expect("nonzero"))
}

/// META arm: per iteration, do exactly what `make_meta` + its call site do — read
/// two token spans, union them, mint the next id from a counter, build the `Meta` —
/// and fold the result into a sink so nothing is optimized away. `ns` per node.
fn time_meta_arm(spans: &[Span], iters: u64) -> f64 {
    let n = spans.len();
    let start = Instant::now();
    let mut sink = 0u64;
    for i in 0..iters {
        let a = black_box(spans[(i as usize) % n]);
        let b = black_box(spans[(i as usize).wrapping_mul(2_654_435_761) % n]);
        // The node's start span ∪ its last token's span (the call-site pattern).
        let span = a.union(b);
        // The id-counter bump: a fresh non-zero id per node.
        let id = NodeId::new(((i as u32) & 0x7FFF_FFFF) + 1).expect("nonzero");
        let meta = Meta::new(span, id);
        sink ^= u64::from(meta.span.start()) ^ u64::from(meta.node_id.as_u32());
    }
    black_box(sink);
    start.elapsed().as_nanos() as f64 / iters as f64
}

/// BASE arm: the same loop shape and the same sink-fold, but the `Meta` is a constant
/// (no union, no counter). The META−BASE delta is the metadata-COMPUTE cost per node.
fn time_base_arm(spans: &[Span], iters: u64) -> f64 {
    let n = spans.len();
    let start = Instant::now();
    let mut sink = 0u64;
    let meta = const_meta();
    for i in 0..iters {
        // Touch the same memory the META arm touches, so the delta is the compute,
        // not a difference in array traffic.
        let _ = black_box(spans[(i as usize) % n]);
        let _ = black_box(spans[(i as usize).wrapping_mul(2_654_435_761) % n]);
        let meta = black_box(meta);
        sink ^= u64::from(meta.span.start()) ^ u64::from(meta.node_id.as_u32());
    }
    black_box(sink);
    start.elapsed().as_nanos() as f64 / iters as f64
}

/// Median of `rounds` interleaved (META, BASE) measurements, read WITHIN a round so
/// both arms share one thermal/frequency state (the alloc_probe method-note rule).
fn meta_compute_ns_per_node(spans: &[Span], iters: u64, rounds: usize) -> (f64, f64, f64) {
    // Warm.
    black_box(time_meta_arm(spans, iters / 10 + 1));
    black_box(time_base_arm(spans, iters / 10 + 1));
    let mut metas = Vec::with_capacity(rounds);
    let mut bases = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        metas.push(time_meta_arm(spans, iters));
        bases.push(time_base_arm(spans, iters));
    }
    let meta = median(&mut metas);
    let base = median(&mut bases);
    (meta, base, (meta - base).max(0.0))
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).expect("no NaN timings"));
    v[v.len() / 2]
}

// ---------------------------------------------------------------------------
// Real-parse timing (for the % denominator), same shape as alloc_probe
// ---------------------------------------------------------------------------

fn time_corpus(corpus: &[&str], passes: u64) -> f64 {
    for _ in 0..(passes / 20).max(1) {
        black_box(one_pass(corpus));
    }
    let start = Instant::now();
    for _ in 0..passes {
        black_box(one_pass(corpus));
    }
    start.elapsed().as_nanos() as f64 / (passes as f64 * corpus.len() as f64)
}

fn one_pass(corpus: &[&str]) -> usize {
    let mut sink = 0usize;
    for &sql in corpus {
        sink = sink.wrapping_add(parse_ours_pg(sql));
    }
    sink
}

fn time_case(sql: &str, iters: u64) -> f64 {
    for _ in 0..(iters / 20).max(1) {
        black_box(parse_ours_pg(black_box(sql)));
    }
    let start = Instant::now();
    for _ in 0..iters {
        black_box(parse_ours_pg(black_box(sql)));
    }
    start.elapsed().as_nanos() as f64 / iters as f64
}

// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let passes: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1_000);
    let micro_iters: u64 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(50_000_000);
    let case_iters: u64 = (passes * 500).max(200_000);

    // Same both-accept corpus + cut as the libpg wall-clock comparison + alloc_probe.
    let subset = libpg_subset();
    let mut corpus: Vec<&str> = subset.included.iter().map(|c| c.sql).collect();
    for (_, batch) in libpg_complex_both_accept() {
        corpus.extend(batch);
    }
    let n_stmts = corpus.len();

    println!("# meta_probe — per-node Meta (span + NodeId) cost, the `ours` parser");
    println!("#   build profile : {}", profile_note());
    println!(
        "#   corpus        : {n_stmts} both-accept statements (curated PG-regress + complex), \
         same cut as libpg_compare / alloc_probe"
    );
    println!(
        "#   measurement   : {passes} corpus passes; {micro_iters} micro-A/B iters; \
         {case_iters} case iters"
    );
    println!(
        "#   NOTE          : bench binary only — the production AST + default parse are untouched."
    );
    println!();

    report_sizes();
    println!();

    // Node counts (retained, Meta-bearing) → bytes attributable to Meta.
    let corpus_nodes: usize = corpus.iter().map(|s| count_nodes(s)).sum();
    let meta_bytes_corpus = corpus_nodes * size_of::<Meta>();
    let nodes_per_stmt = corpus_nodes as f64 / n_stmts as f64;
    println!("## SIZE — total over the corpus");
    println!(
        "#   {corpus_nodes} Meta-bearing nodes over {n_stmts} statements \
         ({nodes_per_stmt:.1} nodes/stmt)"
    );
    println!(
        "#   bytes attributable to Meta = {} B/node x {corpus_nodes} = {meta_bytes_corpus} B \
         ({:.1} KB) of retained AST",
        size_of::<Meta>(),
        meta_bytes_corpus as f64 / 1024.0,
    );
    println!();

    // Build a representative span pool from the corpus's real node spans, so the
    // micro-A/B unions realistic (not constant) inputs.
    let spans = harvest_spans(&corpus);

    // COMPUTE micro-A/B.
    let (meta_ns, base_ns, delta_ns) = meta_compute_ns_per_node(&spans, micro_iters, 7);
    println!("## COMPUTE — isolated micro-A/B (median of 7 interleaved rounds)");
    println!(
        "#   META arm = {meta_ns:.3} ns/node, BASE arm = {base_ns:.3} ns/node  =>  \
         metadata-compute = {delta_ns:.3} ns/node"
    );
    println!();

    // Real parse times + the metadata-compute share.
    let corpus_parse_ns = time_corpus(&corpus, passes);
    let meta_per_stmt = delta_ns * nodes_per_stmt;
    println!("## COMPUTE — share of parse time");
    println!(
        "{:<26} {:>12} {:>12} {:>14} {:>12}",
        "case", "parse ns", "nodes", "meta ns", "meta % parse"
    );
    print_share(
        "corpus (per stmt)",
        corpus_parse_ns,
        nodes_per_stmt,
        delta_ns,
    );
    for (name, sql) in [("nested_expr", NESTED_EXPR), ("star_join", STAR_JOIN)] {
        let nodes = count_nodes(sql) as f64;
        let parse_ns = time_case(sql, case_iters);
        print_share(name, parse_ns, nodes, delta_ns);
    }
    let _ = meta_per_stmt;
    println!();
    println!(
        "# READ-THROUGH: metadata-COMPUTE is the small axis (a span union + an id bump \
         per node);"
    );
    println!(
        "#   the metadata WEIGHT is SIZE ({} B/node), whose parse-time effect is allocation \
         volume +",
        size_of::<Meta>(),
    );
    println!(
        "#   cache density — and the allocation part OVERLAPS the arena/fast-malloc slice \
         already"
    );
    println!("#   measured in libpg-gap-decomposition.md. See docs/performance.md.");
}

fn print_share(name: &str, parse_ns: f64, nodes: f64, delta_ns: f64) {
    let meta_ns = delta_ns * nodes;
    println!(
        "{name:<26} {parse_ns:>12.1} {nodes:>12.0} {meta_ns:>14.2} {:>11.2}%",
        100.0 * meta_ns / parse_ns,
    );
}

/// Collect a pool of real node spans from the corpus, to feed the micro-A/B realistic
/// union inputs (a constant span would let the optimizer fold the union).
fn harvest_spans(corpus: &[&str]) -> Vec<Span> {
    let mut spans = Vec::new();
    for &sql in corpus.iter().take(40) {
        let parsed = parse_ours_owned(OURS_PAIR, sql);
        let mut walk = NodeIdWalk::default();
        for statement in parsed.statements() {
            walk.visit_statement(statement);
        }
        spans.extend(walk.metas.iter().map(|m| m.span));
    }
    if spans.is_empty() {
        spans.push(Span::new(0, 1));
    }
    spans
}

const _: () = {
    // Pin the cost this probe is built around: Meta is 12 bytes (Span 8 + NodeId 4).
    assert!(size_of::<Meta>() == 12);
    assert!(size_of::<Span>() == 8);
    assert!(size_of::<NodeId>() == 4);
};
