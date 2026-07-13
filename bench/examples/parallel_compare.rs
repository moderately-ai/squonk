// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Parallel-parse comparison (measured, not adopted) — `spike-parallel-processing-feasibility`.
//!
//! Feasibility measurement for an opt-in `parallel` feature: is there a batch/bulk
//! workload where parallelizing our parse pipeline beats the sequential path, and
//! what does the interner-merge cost (ADR-0003) do to the break-even point?
//!
//! This is a measurement scaffold in `bench/` (publish = false), NOT a shipped
//! feature and NOT a change to any production parse path. It uses only the public
//! API plus the same `VisitMut` symbol-remap machinery as
//! `conformance/src/shared_interner.rs` (the merge prior art the ticket points at).
//!
//! Two candidate sites are measured:
//!   * BULK — many independent SQL strings parsed to independent `Parsed` trees. Embarrassingly parallel: NO interner merge (each tree keeps its own resolver). This is the parallelism CEILING.
//!   * BATCH — one logical multi-statement input (`a; b; c`) parsed to ONE tree with ONE shared interner + ONE NodeId space, as `parse_with` does. Requires a deterministic MERGE (symbol remap + NodeId/span renumber) to stay byte-identical to the sequential parse. This is the REALISTIC number for the top candidate site — bulk minus the merge tax.
//!
//! Dep policy (ADR-0017): the std::thread path is DEP-FREE and always built. The
//! rayon variant is behind the `parallel-compare` bench feature (a measured-not-adopted
//! dep, exactly like `logos`/`phf`/`lasso` in this crate).
//!
//! Run (dev box is limited-core — see the note in the doc):
//!   cargo run --release --example parallel_compare                 # std-thread sweep
//!   cargo run --release --example parallel_compare -- verify       # byte-identity checks
//!   cargo run --release --example parallel_compare -- sweep 32     # force 32 workers
//!   cargo run --release --features parallel-compare --example parallel_compare -- sweep
//!
//! On the 32-thread Linux builder the orchestrator wants the last form (rayon on)
//! with the default sweep + a `scale` run; see `docs/performance.md`.

// Fair, realistic allocator — matches the other COMPUTE comparison binaries
// (perf_testbed/compare_upstream). Bench-only, never the production allocator.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::hint::black_box;
use std::thread;
use std::time::Instant;

use squonk::dialect::Postgres;
use squonk::interner::{FrozenResolver, Interner};
use squonk::parse_with;
use squonk::tokenizer::tokenize;
use squonk_ast::generated::NodeIdWalk;
use squonk_ast::generated::Visit as _;
use squonk_ast::generated::visit::{self, VisitMut};
use squonk_ast::render::{RenderConfig, RenderCtx, RenderExt as _};
use squonk_ast::{
    Expr, FunctionArg, Ident, NamedOperatorExpr, NoExt, ParameterKind, Resolver, Statement, Symbol,
};

// The vendored sqllogictest corpus: one statement per line, no blanks/comments —
// the cleanest shape for building N independent statements and a `;`-joined batch.
// Same bytes conformance pins (`conformance/src/corpus_sqllogictest.rs`).
const SQLLOGICTEST: &str = include_str!("../../conformance/corpus/sqllogictest/statements.sql");
// A second, heavier corpus for a richer identifier/expression mix.
const SQLGLOT: &str = include_str!("../../conformance/corpus/sqlglot/identity.sql");

type StockParsed = squonk::Parsed<std::sync::Arc<str>, NoExt>;

/// Every corpus line that is EXACTLY one statement AND terminates cleanly at a
/// following `; ` — so a `;`-join round-trips 1:1 and `merged[i]` lines up with
/// `lines[i]`. The sandwich test (`<line> ; SELECT 1` must yield 2 statements)
/// drops lines whose grammar would bleed past the batch separator into the next
/// statement (some corpus DDL over-consumes a trailing `;`), which would otherwise
/// make the joined batch un-parseable.
fn parseable_lines() -> Vec<&'static str> {
    let mut out = Vec::new();
    for line in SQLLOGICTEST.lines().chain(SQLGLOT.lines()) {
        let line = line.trim();
        if line.is_empty() || line.starts_with("--") {
            continue;
        }
        let standalone_ok = matches!(parse_with(line, squonk::ParseConfig::new(Postgres)), Ok(p) if p.statements().len() == 1);
        let joins_cleanly = matches!(
            parse_with(&format!("{line} ; SELECT 1"), squonk::ParseConfig::new(Postgres)),
            Ok(p) if p.statements().len() == 2
        );
        if standalone_ok && joins_cleanly {
            out.push(line);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The MERGE (ADR-0003 crux) — per-thread interner + deterministic remap.
//
// Mirrors `conformance/src/shared_interner.rs`: re-intern every `Symbol` a worker's
// tree carries, from its own resolver into ONE shared interner, riding the generated
// `VisitMut` and overriding only the five raw-`Symbol` leaf sites. Re-interning in
// source order reproduces the sequential interner's dedup + assignment.
// ---------------------------------------------------------------------------

struct SymbolRemapper<'a> {
    resolver: &'a dyn Resolver,
    interner: &'a mut Interner,
}

impl SymbolRemapper<'_> {
    fn remap(&mut self, sym: &mut Symbol) {
        let text = self
            .resolver
            .try_resolve(*sym)
            .expect("worker symbol resolves in its own resolver");
        *sym = self.interner.intern(text);
    }
}

impl VisitMut<NoExt> for SymbolRemapper<'_> {
    fn visit_ident_mut(&mut self, node: &mut Ident) {
        self.remap(&mut node.sym);
        visit::walk_ident_mut(self, node);
    }
    fn visit_function_arg_mut(&mut self, node: &mut FunctionArg<NoExt>) {
        if let Some(name) = &mut node.name {
            self.remap(name);
        }
        visit::walk_function_arg_mut(self, node);
    }
    fn visit_named_operator_expr_mut(&mut self, node: &mut NamedOperatorExpr<NoExt>) {
        self.remap(&mut node.op);
        visit::walk_named_operator_expr_mut(self, node);
    }
    fn visit_parameter_kind_mut(&mut self, node: &mut ParameterKind) {
        if let ParameterKind::Named { name, .. } = node {
            self.remap(name);
        }
        visit::walk_parameter_kind_mut(self, node);
    }
    fn visit_expr_mut(&mut self, node: &mut Expr<NoExt>) {
        if let Expr::SessionVariable { name, .. } = node {
            self.remap(name);
        }
        visit::walk_expr_mut(self, node);
    }
}

/// Merge already-parsed worker trees into ONE shared resolver + statement list, in
/// source order — the sequential tail of a batch-parallel parse.
///
/// This does the REAL, dominant merge work: clone each statement out (a production
/// merge would OWN the worker's statements and skip this clone — so the number here
/// slightly over-counts), symbol-remap it into the shared interner, and freeze.
/// The NodeId/span renumber is folded in as one extra whole-tree traversal via the
/// read-only `NodeIdWalk` proxy (a production `NodeIdWalkMut` sibling would add only
/// a constant `+= offset` per node, riding this same walk — see the note).
fn merge_batch(parseds: &[StockParsed]) -> (Vec<Statement<NoExt>>, FrozenResolver) {
    let mut interner = Interner::new();
    let mut merged: Vec<Statement<NoExt>> = Vec::new();
    for parsed in parseds {
        let resolver: &dyn Resolver = parsed.resolver();
        for stmt in parsed.statements() {
            let mut cloned = stmt.clone();
            let mut remapper = SymbolRemapper {
                resolver,
                interner: &mut interner,
            };
            remapper.visit_statement_mut(&mut cloned);
            merged.push(cloned);
        }
    }
    // NodeId/span renumber proxy: one whole-tree traversal (the arithmetic add per
    // node is below timing resolution; what costs is the traversal, measured here).
    let mut walk = NodeIdWalk::default();
    for stmt in &merged {
        walk.visit_statement(stmt);
    }
    black_box(walk.metas.len());
    (merged, interner.freeze())
}

// ---------------------------------------------------------------------------
// Workloads
// ---------------------------------------------------------------------------

/// BULK sequential: parse each string to its own tree; sum statement counts so the
/// full parse + drop happens inside the timed region.
fn bulk_seq(lines: &[&str]) -> usize {
    let mut n = 0;
    for &s in lines {
        n += parse_with(s, squonk::ParseConfig::new(Postgres))
            .expect("parses")
            .statements()
            .len();
    }
    n
}

/// BULK std::thread: fresh scoped worker threads per call (a stateless
/// `parse_bulk_parallel` API spawns per call — no persistent pool), chunked evenly.
fn bulk_threads(lines: &[&str], workers: usize) -> usize {
    if workers <= 1 || lines.len() < 2 {
        return bulk_seq(lines);
    }
    let chunk = lines.len().div_ceil(workers);
    thread::scope(|scope| {
        let handles: Vec<_> = lines
            .chunks(chunk)
            .map(|c| scope.spawn(move || bulk_seq(c)))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).sum()
    })
}

#[cfg(feature = "parallel-compare")]
fn bulk_rayon(lines: &[&str]) -> usize {
    use rayon::prelude::*;
    lines
        .par_iter()
        .map(|&s| {
            parse_with(s, squonk::ParseConfig::new(Postgres))
                .expect("parses")
                .statements()
                .len()
        })
        .sum()
}

/// BATCH sequential baseline: ONE `parse_with` over the `;`-joined input — the
/// production batch API, one interner + one NodeId space.
fn batch_seq(joined: &str) -> usize {
    parse_with(joined, squonk::ParseConfig::new(Postgres))
        .expect("batch parses")
        .statements()
        .len()
}

/// BATCH std::thread + merge: parse chunks in parallel to worker trees, then the
/// sequential deterministic merge (symbol remap + renumber). This is the honest
/// end-to-end batch-parallel cost, INCLUDING the interner-merge tax.
fn batch_threads_merge(lines: &[&str], workers: usize) -> usize {
    let parseds = parse_workers(lines, workers);
    let (merged, resolver) = merge_batch(&parseds);
    black_box(&resolver);
    merged.len()
}

/// Parse `lines` into per-line worker trees, in source order, across `workers`
/// scoped threads (or sequentially when workers <= 1).
fn parse_workers(lines: &[&str], workers: usize) -> Vec<StockParsed> {
    if workers <= 1 || lines.len() < 2 {
        return lines
            .iter()
            .map(|&s| parse_with(s, squonk::ParseConfig::new(Postgres)).expect("parses"))
            .collect();
    }
    let chunk = lines.len().div_ceil(workers);
    thread::scope(|scope| {
        let handles: Vec<_> = lines
            .chunks(chunk)
            .map(|c| {
                scope.spawn(move || {
                    c.iter()
                        .map(|&s| {
                            parse_with(s, squonk::ParseConfig::new(Postgres)).expect("parses")
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        // Chunks are contiguous and joined in order → source order preserved.
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    })
}

// ---------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------

/// Median-of-`rounds` ns for one call of `f` over the whole batch, after a warm-up.
fn bench<F: FnMut() -> usize>(mut f: F, rounds: usize) -> f64 {
    for _ in 0..3 {
        black_box(f());
    }
    let mut samples: Vec<f64> = (0..rounds)
        .map(|_| {
            let start = Instant::now();
            let out = f();
            let ns = start.elapsed().as_nanos() as f64;
            black_box(out);
            ns
        })
        .collect();
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[rounds / 2]
}

// ---------------------------------------------------------------------------
// Correctness — byte-identity of the parallel decomposition + the merge
// ---------------------------------------------------------------------------

fn verify(lines: &[&str]) {
    // (1) Decomposition preserves RENDER byte-for-byte: rendering each statement
    // independently and joining by "; " equals the single sequential batch render.
    // Render resolves symbols->text and slices each statement's OWN source, so it is
    // independent of raw Symbol/NodeId values — the robust identity guarantee.
    let sample = &lines[..lines.len().min(200)];
    let joined = sample.join("; ");
    let seq_render = parse_with(&joined, squonk::ParseConfig::new(Postgres))
        .expect("batch parses")
        .to_string();
    let par_render = sample
        .iter()
        .map(|&s| {
            parse_with(s, squonk::ParseConfig::new(Postgres))
                .expect("parses")
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("; ");
    assert_eq!(
        seq_render, par_render,
        "parallel decomposition changed the render"
    );

    // (2) The symbol MERGE is correct: after remapping worker symbols into one shared
    // interner, each merged statement still resolves + renders to its original text.
    let parseds = parse_workers(sample, 4);
    let (merged, resolver) = merge_batch(&parseds);
    assert_eq!(
        merged.len(),
        sample.len(),
        "merge dropped/duplicated a statement"
    );
    for (i, stmt) in merged.iter().enumerate() {
        let src = parseds[i].source();
        let config = RenderConfig::default();
        let ctx = RenderCtx::new(&resolver, src, &config);
        let via_shared = stmt.displayed(&ctx).to_string();
        let standalone = parseds[i].to_string();
        assert_eq!(
            via_shared, standalone,
            "merged statement {i} rendered differently through the shared resolver",
        );
    }
    println!("verify: PASS  ({} statements)", sample.len());
    println!("  - render byte-identity: sequential batch == parallel-decomposed join");
    println!(
        "  - symbol merge: all {} statements resolve through the shared interner",
        merged.len()
    );
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("sweep");
    let forced_workers: Option<usize> = args.get(2).and_then(|s| s.parse().ok());
    let hw = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let workers = forced_workers.unwrap_or(hw);

    let lines = parseable_lines();
    println!(
        "corpus: {} parseable single-statement lines | hw parallelism = {} | workers = {}{}",
        lines.len(),
        hw,
        workers,
        if cfg!(feature = "parallel-compare") {
            " | rayon: ON"
        } else {
            " | rayon: OFF (dep-free std::thread only)"
        },
    );

    match mode {
        "verify" => verify(&lines),
        "scale" => scale(&lines),
        _ => sweep(&lines, workers),
    }
}

/// The money table: for a ladder of batch sizes, sequential vs parallel for BOTH
/// sites, with speedups and the merge tax. Finds the break-even batch size.
fn sweep(lines: &[&str], workers: usize) {
    let sizes = [1usize, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024];
    println!();
    println!("=== SWEEP  (median ns per whole-batch call, {workers} workers) ===");
    println!(
        "{:>6} {:>12} {:>12} {:>7} {:>12} {:>7} {:>12} {:>7}",
        "N", "seq(bulk)", "thr(bulk)", "x", "seq(batch)", "x", "thr+merge", "x",
    );
    for &n in &sizes {
        if n > lines.len() {
            break;
        }
        let batch: Vec<&str> = lines.iter().take(n).copied().collect();
        let joined = batch.join("; ");
        // A big batch is cheap per call, a small one very cheap — scale rounds so
        // every row gets a stable median without a fixed wall-clock budget.
        let rounds = if n <= 16 {
            200
        } else if n <= 128 {
            60
        } else {
            25
        };

        let seq_bulk = bench(|| bulk_seq(&batch), rounds);
        let thr_bulk = bench(|| bulk_threads(&batch, workers), rounds);
        let seq_batch = bench(|| batch_seq(&joined), rounds);
        let thr_merge = bench(|| batch_threads_merge(&batch, workers), rounds);

        println!(
            "{n:>6} {seq_bulk:>12.0} {thr_bulk:>12.0} {:>7.2} {seq_batch:>12.0} {:>7.2} {thr_merge:>12.0} {:>7.2}",
            seq_bulk / thr_bulk,
            1.0, // batch seq is the batch baseline, shown as 1.00 reference
            seq_batch / thr_merge,
        );

        #[cfg(feature = "parallel-compare")]
        {
            let ray = bench(|| bulk_rayon(&batch), rounds);
            println!("       rayon(bulk) {ray:>12.0}  x {:.2}", seq_bulk / ray);
        }
    }

    // Merge-tax breakdown at the largest batch: what fraction of the batch-parallel
    // path is the inherently-sequential merge (the Amdahl serial bottleneck).
    let n = *sizes.iter().rev().find(|&&n| n <= lines.len()).unwrap();
    let batch: Vec<&str> = lines.iter().take(n).copied().collect();
    let joined = batch.join("; ");
    println!();
    println!("=== MERGE-TAX BREAKDOWN  (N = {n}) ===");
    let parse_only = bench(
        || {
            let p = parse_workers(&batch, workers);
            black_box(p.len())
        },
        25,
    );
    let full = bench(|| batch_threads_merge(&batch, workers), 25);
    let split_ub = bench(|| tokenize(&joined).expect("tokenizes").len(), 60);
    println!("  parallel parse only (no merge)   : {parse_only:>10.0} ns");
    println!("  parallel parse + merge (total)   : {full:>10.0} ns");
    println!(
        "  => sequential merge tax           : {:>10.0} ns  ({:.0}% of total)",
        full - parse_only,
        100.0 * (full - parse_only) / full
    );
    println!(
        "  boundary split upper bound (tok) : {split_ub:>10.0} ns  (sequential, add to batch path)"
    );
    println!();
    println!("Note: `thr+merge` spawns worker threads per call (stateless API model).");
    println!("      NodeId/span renumber is one folded traversal; symbol remap dominates the tax.");
}

/// Thread-count scaling at a fixed large batch — the run the orchestrator wants on
/// the 32-thread builder to see where bulk speedup saturates.
fn scale(lines: &[&str]) {
    let n = lines.len();
    let batch: Vec<&str> = lines.to_vec();
    let joined = batch.join("; ");
    let hw = thread::available_parallelism()
        .map(|x| x.get())
        .unwrap_or(1);
    println!();
    println!("=== THREAD SCALING  (N = {n}, hw = {hw}) ===");
    let seq_bulk = bench(|| bulk_seq(&batch), 40);
    let seq_batch = bench(|| batch_seq(&joined), 40);
    println!("  bulk  sequential : {seq_bulk:>10.0} ns");
    println!("  batch sequential : {seq_batch:>10.0} ns");
    println!(
        "{:>8} {:>12} {:>7} {:>12} {:>7}",
        "workers", "bulk", "x", "batch+merge", "x"
    );
    let mut w = 1;
    while w <= (hw * 2).max(2) {
        let b = bench(|| bulk_threads(&batch, w), 40);
        let m = bench(|| batch_threads_merge(&batch, w), 30);
        println!(
            "{w:>8} {b:>12.0} {:>7.2} {m:>12.0} {:>7.2}",
            seq_bulk / b,
            seq_batch / m
        );
        w *= 2;
    }
    #[cfg(feature = "parallel-compare")]
    {
        let ray = bench(|| bulk_rayon(&batch), 40);
        println!("   rayon bulk    : {ray:>10.0} ns  x {:.2}", seq_bulk / ray);
    }
}
