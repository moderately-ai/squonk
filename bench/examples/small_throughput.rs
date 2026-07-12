// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Small-statement throughput + concurrency-scaling testbed (spike).
//!
//! The companion to `perf_testbed.rs`, which measures only big/complex shapes
//! (`star_join`, `nested_expr`, `wide_select`) where the Pratt loop and node
//! allocation dominate. This harness measures the OPPOSITE workload — a service
//! parsing millions of SMALL statements (single-table SELECT, one-row INSERT,
//! short WHERE, short DDL) — where the PER-PARSE FIXED COSTS (the fresh interner
//! per parse, `Parser::new`, the streaming-cursor init, freezing an interner into
//! a resolver) are a much larger fraction of a tiny parse than they are of a big
//! one. It also measures THREAD-SCALING: the AST is `Send + Sync` and the interner
//! is per-parse (no shared state, no lock), so parsing *should* scale linearly —
//! but many concurrent short-lived parses each churn an interner map + a tree of
//! `Box`ed nodes, and a general-purpose allocator can contend under that churn. This is
//! the unmeasured axis (ticket
//! `spike-high-throughput-small-statement-parse-profiling-concurrency-scaling`).
//!
//! Three measurements, all release/`profiling` (never debug — the workspace builds
//! our crates at `opt-level = 0` under `dev`, so a debug number pits our
//! unoptimized parser against an optimized `sqlparser-rs` and lies):
//!
//!   `bench`     — per-case ns/parse, parses/sec, and the FIXED-cost breakdown
//!                 (scaffold floor vs lexer vs the remaining grammar+intern+build).
//!   `interner`  — the interner's own new+intern+freeze cost per tiny parse, and
//!                 the projected saving from a poolable/resettable interner that
//!                 reuses the dedup map's bucket array across parses.
//!   `threads`   — parse the small corpus across 1..N std::threads; aggregate
//!                 parses/sec and the scaling efficiency vs perfectly linear.
//!
//! Build + run (default runs all three):
//!   cargo build --profile profiling --example small_throughput -p squonk-bench
//!   ./target/profiling/examples/small_throughput
//!   ./target/profiling/examples/small_throughput threads 2000000

// Bench-only fast allocator (ADR-0017: never the production allocator). This testbed
// runs under mimalloc — the realistic allocator a high-throughput service ships, with
// per-thread heaps — so the throughput + thread-scaling numbers reflect a real
// deployment rather than the default system allocator. The allocator-contention
// attribution below (parse churns ~16 allocs/statement vs tokenize's 1-2) is
// allocator-agnostic; it now characterizes mimalloc's contention, the realistic one.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::hint::black_box;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use squonk::dialect::Postgres;
use squonk::interner::Interner;
use squonk::parse_with;
use squonk::tokenizer::{TokenKind, tokenize};

// ---------------------------------------------------------------------------
// Small-statement corpus
// ---------------------------------------------------------------------------
// The OLTP / ORM / query-router shapes the perf testbed never covered: each is a
// single short statement a service parses by the million. Kept to the parser's
// current Postgres surface so every case actually parses (the harness asserts it).

const TINY_SELECT: &str = "SELECT 1";
const PK_LOOKUP: &str = "SELECT id, name, email FROM users WHERE id = 42";
const SIMPLE_SELECT: &str = "SELECT a, b FROM t WHERE a = b";
const ONE_ROW_INSERT: &str = "INSERT INTO t (id, name) VALUES (1, 'a')";
const SHORT_UPDATE: &str = "UPDATE t SET name = 'b' WHERE id = 1";
const SHORT_DELETE: &str = "DELETE FROM t WHERE id = 1";
const SHORT_DDL: &str = "CREATE TABLE t (id INT, name TEXT)";

const CORPUS: &[(&str, &str)] = &[
    ("tiny_select", TINY_SELECT),
    ("pk_lookup", PK_LOOKUP),
    ("simple_select", SIMPLE_SELECT),
    ("one_row_insert", ONE_ROW_INSERT),
    ("short_update", SHORT_UPDATE),
    ("short_delete", SHORT_DELETE),
    ("short_ddl", SHORT_DDL),
];

/// Parse one statement under Postgres to its owned root; return the statement
/// count so the optimizer cannot elide the parse.
fn parse_one(sql: &str) -> usize {
    parse_with(sql, Postgres)
        .expect("small-corpus SQL parses under Postgres")
        .statements()
        .len()
}

/// Our tokenizer alone — the lexer's share of a parse, since the streaming parse
/// tokenizes lazily as the grammar pulls lookahead.
fn tokenize_only(sql: &str) -> usize {
    tokenize(sql).expect("small-corpus SQL tokenizes").len()
}

/// Repetitions whose MINIMUM we keep. These statements parse in hundreds of ns, so
/// a single timed loop is swamped by scheduling/frequency noise (observed ~2x
/// run-to-run swings); the minimum over several reps is the least-perturbed
/// estimate of the true cost (it discards samples an interrupt or a frequency dip
/// inflated, never deflates).
const REPS: u32 = 9;

fn best<F: FnMut() -> f64>(mut sample: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..REPS {
        best = best.min(sample());
    }
    best
}

/// Time `f` over `iters` calls after a warm-up; return the best (min) ns per call
/// across [`REPS`] repetitions.
fn time_loop(f: impl Fn(&str) -> usize, sql: &str, iters: u64) -> f64 {
    for _ in 0..(iters / 20).max(1) {
        black_box(f(black_box(sql)));
    }
    best(|| {
        let start = Instant::now();
        let mut sink = 0usize;
        for _ in 0..iters {
            sink = sink.wrapping_add(black_box(f(black_box(sql))));
        }
        let elapsed = start.elapsed();
        black_box(sink);
        elapsed.as_nanos() as f64 / iters as f64
    })
}

// ---------------------------------------------------------------------------
// `bench`: per-case throughput + the fixed-cost breakdown
// ---------------------------------------------------------------------------
// Decomposition of a tiny parse's wall time:
//   floor = parse_with("")  — the per-call scaffold that is independent of the
//           statement: Parser::new, the streaming cursor init, a fresh empty
//           interner, freezing it, the statements Vec, and the Arc<str> root.
//   lex   = tokenize(sql)   — the lexer, which the streaming parse pays inline.
//   work  = parse - floor - lex — the residual: grammar dispatch, the token
//           cursor/peek, identifier interning, and node (Box) construction.
// `floor` is the FIXED overhead the ticket asks us to isolate; `work` scales with
// the statement. The interesting question is how big `floor` is *relative to a
// tiny parse* — on a big query it is noise, on `SELECT 1` it may dominate.

fn bench_table() {
    let iters: u64 = 200_000;
    // The scaffold floor is content-independent: measure it once on the empty
    // input, which still runs Parser::new + interner new/freeze + an Arc<str> root.
    let floor = time_loop(
        |s| {
            parse_with(s, Postgres)
                .expect("empty input parses to zero statements")
                .statements()
                .len()
        },
        "",
        iters,
    );

    println!("== small-statement throughput (release/profiling) ==");
    println!("per-call scaffold floor (parse of \"\"): {floor:.0} ns\n");
    println!(
        "{:<16} {:>6} {:>11} {:>13} {:>10} {:>7} {:>10} {:>8}",
        "case", "bytes", "parse ns", "parses/sec", "lex ns", "lex%", "floor%", "work%"
    );
    for &(name, sql) in CORPUS {
        let parse_ns = time_loop(parse_one, sql, iters);
        let lex_ns = time_loop(tokenize_only, sql, iters);
        let work_ns = (parse_ns - floor - lex_ns).max(0.0);
        let per_sec = 1e9 / parse_ns;
        println!(
            "{:<16} {:>6} {:>11.0} {:>13.0} {:>10.0} {:>6.0}% {:>9.0}% {:>7.0}%",
            name,
            sql.len(),
            parse_ns,
            per_sec,
            lex_ns,
            100.0 * lex_ns / parse_ns,
            100.0 * floor / parse_ns,
            100.0 * work_ns / parse_ns,
        );
    }
}

// ---------------------------------------------------------------------------
// `interner`: the per-parse interner cost and the poolable-interner projection
// ---------------------------------------------------------------------------

/// The identifier lexemes a statement interns — every `Word` / `QuotedIdent`
/// token's source text. Keywords short-circuit in `Interner::intern` (they own
/// fixed low slots) and never allocate, so this is the set that actually hits the
/// dynamic dedup map, mirroring what the parser interns.
fn identifiers(sql: &str) -> Vec<&str> {
    tokenize(sql)
        .expect("tokenizes")
        .iter()
        .filter(|t| matches!(t.kind, TokenKind::Word | TokenKind::QuotedIdent))
        .map(|t| &sql[t.span.start() as usize..t.span.end() as usize])
        .collect()
}

/// The REAL interner's per-parse cost: a fresh `Interner`, intern every
/// identifier, then `freeze` into the shippable resolver — exactly the lifecycle a
/// parse drives. Includes the dedup-map bucket alloc, the per-identifier `Box<str>`
/// double-store, and the freeze teardown.
fn interner_fresh(ids: &[&str]) -> usize {
    let mut interner = Interner::new();
    for &id in ids {
        black_box(interner.intern(black_box(id)));
    }
    let resolver = interner.freeze();
    black_box(&resolver);
    ids.len()
}

// --- A bench-local replica of the interner's dedup map, to project the win of a
// --- poolable/resettable interner. A pool can reuse the map's *bucket array*
// --- across parses (clear() retains capacity); it CANNOT reuse the `strings` Vec,
// --- which `freeze` moves into the resolver, so every parse still allocates that
// --- and every per-identifier `Box<str>`. The amortizable component is therefore
// --- exactly the map bucket array's alloc + free, which the two arms below
// --- isolate: `fresh` allocates and frees it each parse, `reuse` retains it.

/// FxHash replica of `crate::interner::fast_hash` (that module is private), so the
/// projection's map hashes identifiers exactly as the real interner does and the
/// numbers are comparable. See that module for the DoS-resistance trade-off.
#[derive(Default)]
struct FxHasher {
    hash: u64,
}

impl FxHasher {
    #[inline]
    fn add_word(&mut self, word: u64) {
        const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, mut bytes: &[u8]) {
        while let Some((chunk, rest)) = bytes.split_first_chunk::<8>() {
            self.add_word(u64::from_ne_bytes(*chunk));
            bytes = rest;
        }
        if let Some((chunk, rest)) = bytes.split_first_chunk::<4>() {
            self.add_word(u64::from(u32::from_ne_bytes(*chunk)));
            bytes = rest;
        }
        if let Some((chunk, rest)) = bytes.split_first_chunk::<2>() {
            self.add_word(u64::from(u16::from_ne_bytes(*chunk)));
            bytes = rest;
        }
        if let Some(&byte) = bytes.first() {
            self.add_word(u64::from(byte));
        }
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

type FxBuildHasher = BuildHasherDefault<FxHasher>;
type DedupMap = HashMap<Box<str>, u32, FxBuildHasher>;

/// One dedup pass into `map`: intern each identifier (skip duplicates), pushing a
/// second `Box<str>` copy into a fresh `strings` Vec exactly as the real interner's
/// transient double-store does. The `strings` Vec is fresh every call in BOTH arms
/// because `freeze` ships it out — only the `map` differs between the arms.
#[inline]
fn fill_map(map: &mut DedupMap, ids: &[&str]) {
    let mut strings: Vec<Box<str>> = Vec::new();
    for &id in ids {
        if !map.contains_key(id) {
            let boxed: Box<str> = Box::from(id);
            strings.push(boxed.clone());
            map.insert(boxed, strings.len() as u32);
        }
    }
    black_box(&strings);
}

/// Arm A — status quo: a fresh dedup map every parse (allocates its bucket array,
/// frees it on drop).
fn map_fresh(ids: &[&str]) {
    let mut map: DedupMap = HashMap::default();
    fill_map(&mut map, ids);
    black_box(&map);
}

/// Arm B — pooled: one dedup map reused across parses, `clear`ed each time (drops
/// the keys but RETAINS the bucket array, so no per-parse bucket alloc/free).
fn map_reuse(map: &mut DedupMap, ids: &[&str]) {
    map.clear();
    fill_map(map, ids);
    black_box(&map);
}

fn time_unit(iters: u64, mut f: impl FnMut()) -> f64 {
    for _ in 0..(iters / 20).max(1) {
        f();
    }
    best(|| {
        let start = Instant::now();
        for _ in 0..iters {
            f();
        }
        start.elapsed().as_nanos() as f64 / iters as f64
    })
}

fn interner_table() {
    let iters: u64 = 300_000;
    println!("== per-parse interner cost + poolable-interner projection ==");
    println!(
        "{:<16} {:>5} {:>13} {:>13} {:>13} {:>11}",
        "case", "ids", "fresh-interner", "fresh-map ns", "reuse-map ns", "pool save"
    );
    let mut total_save = 0.0;
    let mut n = 0;
    for &(name, sql) in CORPUS {
        let ids = identifiers(sql);
        let interner_ns = time_unit(iters, || {
            black_box(interner_fresh(black_box(&ids)));
        });
        let fresh_ns = time_unit(iters, || map_fresh(black_box(&ids)));
        // Pre-size the reused map with one warm fill so steady-state reuse never
        // reallocs the bucket array (a pool reaches this state after parse #1).
        let mut pool: DedupMap = HashMap::default();
        fill_map(&mut pool, &ids);
        let reuse_ns = time_unit(iters, || map_reuse(&mut pool, black_box(&ids)));
        let save = (fresh_ns - reuse_ns).max(0.0);
        total_save += save;
        n += 1;
        println!(
            "{:<16} {:>5} {:>13.0} {:>13.0} {:>13.0} {:>11.1}",
            name,
            ids.len(),
            interner_ns,
            fresh_ns,
            reuse_ns,
            save,
        );
    }
    println!(
        "\nmean per-parse map-pool saving (bucket alloc+free): {:.1} ns",
        total_save / n as f64
    );
    println!("(compare to per-case parse ns in the `bench` table — this is the projected win)");
}

// ---------------------------------------------------------------------------
// `threads`: concurrency scaling, 1..N
// ---------------------------------------------------------------------------
// Each thread parses the whole small corpus in a tight loop for a fixed iteration
// budget; all threads start together on a barrier and time their own work. The
// interner is per-parse and the AST is Send+Sync, so there is no app-level shared
// state — any sub-linearity is the global allocator (mimalloc) contending under many
// concurrent short-lived parses (each churning an interner map + a Box tree).

fn run_threads(work: fn(&str) -> usize, n_threads: usize, iters_per_thread: u64) -> f64 {
    let barrier = Arc::new(Barrier::new(n_threads));
    let handles: Vec<_> = (0..n_threads)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut sink = 0usize;
                barrier.wait();
                let start = Instant::now();
                for _ in 0..iters_per_thread {
                    for &(_, sql) in CORPUS {
                        sink = sink.wrapping_add(black_box(work(black_box(sql))));
                    }
                }
                let elapsed = start.elapsed();
                black_box(sink);
                elapsed
            })
        })
        .collect();

    // Wall time = the slowest thread (the run is done when the last finishes).
    let max_elapsed = handles
        .into_iter()
        .map(|h| h.join().expect("worker thread does not panic"))
        .max()
        .unwrap_or(Duration::ZERO);

    let units = (n_threads as u64) * iters_per_thread * (CORPUS.len() as u64);
    units as f64 / max_elapsed.as_secs_f64()
}

/// Best (max) aggregate throughput for `n` threads over a handful of runs — the
/// min-time estimator of [`time_loop`], applied to the whole parallel run so a
/// single scheduler hiccup on one thread does not understate scaling.
fn best_threads(work: fn(&str) -> usize, n: usize, iters_per_thread: u64) -> f64 {
    const THREAD_REPS: u32 = 5;
    (0..THREAD_REPS)
        .map(|_| run_threads(work, n, iters_per_thread))
        .fold(0.0_f64, f64::max)
}

// Two scaling curves on the SAME corpus, to attribute any sub-linearity:
//   parse    — the full pipeline: ~16 allocations / statement (interner map + the
//              Box tree + the Arc<str> root), so it leans hard on the global
//              allocator under concurrency.
//   tokenize — the lexer alone: 1-2 allocations / statement (just the token Vec).
// Both pay identical scheduler/heterogeneous-core effects, so where parse's
// efficiency falls BELOW tokenize's, the gap is global-allocator contention under
// concurrent short-lived parses; where they fall together, it is core saturation /
// scheduling, not the allocator.
fn thread_scaling(iters_per_thread: u64) {
    let max_threads = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);
    println!("== thread-scaling: small corpus across 1..{max_threads} threads ==");
    println!("(release/profiling; {iters_per_thread} corpus-iterations/thread, best of 5)");
    println!("(no SMT on Apple Silicon; heterogeneous P+E cores make the low-N region noisy)\n");
    println!(
        "{:>7} | {:>13} {:>11} {:>9} | {:>13} {:>11} {:>9}",
        "threads", "parse/sec", "per-thread", "scale", "tok/sec", "per-thread", "scale"
    );

    // Warm caches/allocator before the timed single-thread baselines.
    let _ = run_threads(parse_one, 1, (iters_per_thread / 10).max(1));
    let parse_base = best_threads(parse_one, 1, iters_per_thread);
    let tok_base = best_threads(tokenize_only, 1, iters_per_thread);
    for n in 1..=max_threads {
        let parse_tp = if n == 1 {
            parse_base
        } else {
            best_threads(parse_one, n, iters_per_thread)
        };
        let tok_tp = if n == 1 {
            tok_base
        } else {
            best_threads(tokenize_only, n, iters_per_thread)
        };
        println!(
            "{n:>7} | {parse_tp:>13.0} {:>11.0} {:>8.1}% | {tok_tp:>13.0} {:>11.0} {:>8.1}%",
            parse_tp / n as f64,
            100.0 * parse_tp / (parse_base * n as f64),
            tok_tp / n as f64,
            100.0 * tok_tp / (tok_base * n as f64),
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("all");
    let arg2: Option<u64> = args.get(2).and_then(|s| s.parse().ok());

    // Fail loudly if a corpus entry ever drifts off the parser surface.
    for &(name, sql) in CORPUS {
        assert!(
            parse_with(sql, Postgres).is_ok(),
            "corpus case {name} no longer parses",
        );
    }

    match mode {
        "bench" => bench_table(),
        "interner" => interner_table(),
        "threads" => thread_scaling(arg2.unwrap_or(40_000)),
        _ => {
            bench_table();
            println!();
            interner_table();
            println!();
            thread_scaling(arg2.unwrap_or(40_000));
        }
    }
}
