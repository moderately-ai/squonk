// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Iterative-vs-recursive Pratt parsing spike driver (`spike-iterative-pratt`).
//!
//! Measures whether an explicit-heap-stack expression parser can replace the
//! recursive-descent core (`crates/squonk/src/parser/expr.rs`) to remove the
//! stack-overflow ceiling on deep nesting, and at what cost to the common shallow
//! case. The two parsers and the battery live in the shared core; this is the
//! measurement harness around them.
//!
//! The number that decides it is the SHALLOW-case iterative/recursive delta: a
//! regression there taxes every parse. The deep cases only confirm the iterative
//! form has no stack growth.
//!
//! Build OPTIMIZED — never measure parse CPU in a debug build (the workspace
//! `opt-level=1` optimizes deps but leaves our code at `opt-level=0`):
//!
//! ```text
//! cargo run --profile profiling --example iterative_pratt -p squonk-bench            # shallow table (the decider)
//! cargo run --profile profiling --example iterative_pratt -p squonk-bench -- scaling # deep 1k/10k/100k, no overflow
//! cargo run --profile profiling --example iterative_pratt -p squonk-bench -- deep iter prefix 100000
//! cargo run --profile profiling --example iterative_pratt -p squonk-bench -- ceiling parens  # INTENTIONALLY overflows recursive
//! ```

#[path = "../benches/iterative_pratt_ref/mod.rs"]
mod iterative_pratt_ref;

use std::hint::black_box;
use std::time::Instant;

use iterative_pratt_ref::{
    Expr, IterParser, SHALLOW_CASES, nested_parens, node_count, prefix_chain, rec_parse, tokenize,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("shallow");
    match mode {
        "shallow" => shallow_table(),
        "scaling" => scaling(),
        "deep" => deep_once(&args),
        "ceiling" => ceiling(&args),
        "verify" => verify(),
        other => {
            eprintln!("unknown mode {other:?}; modes: shallow | scaling | deep | ceiling | verify");
            std::process::exit(2);
        }
    }
}

/// Time `f` over `iters` calls after a warm-up; nanoseconds per call.
fn time_loop(mut f: impl FnMut(), iters: u64) -> f64 {
    for _ in 0..(iters / 20).max(1) {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    start.elapsed().as_nanos() as f64 / iters as f64
}

// ---------------------------------------------------------------------------
// THE deciding measurement: shallow recursive vs iterative
// ---------------------------------------------------------------------------

fn shallow_table() {
    // Parity first: the perf numbers only mean something if both parsers build the
    // same tree for every case.
    assert_parity();

    println!("Shallow case: recursive vs iterative explicit-stack (ns/parse).");
    println!(
        "Tokenized once per case; only the PARSE is timed (lexer is shared, not under test).\n"
    );
    println!(
        "{:<18} {:>6} {:>11} {:>11} {:>9}",
        "case", "nodes", "recursive", "iterative", "iter/rec"
    );

    let mut iter = IterParser::with_capacity(64);
    let mut tot_rec = 0.0;
    let mut tot_iter = 0.0;
    for &(name, src) in SHALLOW_CASES {
        let toks = tokenize(src).expect("battery tokenizes");
        let tree = rec_parse(&toks).expect("battery parses");
        let nodes = node_count(&tree);

        let iters = 400_000;
        let rec_ns = time_loop(
            || {
                black_box(rec_parse(black_box(&toks))).ok();
            },
            iters,
        );
        let iter_ns = time_loop(
            || {
                black_box(iter.parse(black_box(&toks))).ok();
            },
            iters,
        );
        tot_rec += rec_ns;
        tot_iter += iter_ns;
        println!(
            "{name:<18} {nodes:>6} {rec_ns:>11.1} {iter_ns:>11.1} {:>9.3}",
            iter_ns / rec_ns
        );
    }
    println!(
        "\n{:<18} {:>6} {:>11.1} {:>11.1} {:>9.3}   <- aggregate",
        "ALL",
        "",
        tot_rec,
        tot_iter,
        tot_iter / tot_rec
    );
    println!(
        "\nReused iterative stack capacity after the run: {} frames (no per-parse alloc).",
        iter.stack_capacity()
    );
}

// ---------------------------------------------------------------------------
// Deep scaling: iterative handles arbitrary depth with no stack growth
// ---------------------------------------------------------------------------

fn scaling() {
    println!("Deep nesting — iterative parser, in-process (it never overflows).\n");
    println!(
        "{:<8} {:>9} {:>14} {:>10} {:>8}",
        "shape", "depth", "ns/parse", "ns/level", "nodes"
    );
    let mut iter = IterParser::with_capacity(1 << 20);
    for shape in ["parens", "prefix"] {
        for depth in [1_000usize, 10_000, 100_000] {
            let src = build(shape, depth);
            let toks = tokenize(&src).expect("deep input tokenizes");
            // A handful of iterations: enough to time, few enough that a 100k tree
            // build+teardown stays quick.
            let runs = if depth >= 100_000 { 60 } else { 400 };
            let ns = time_loop(
                || {
                    black_box(iter.parse(black_box(&toks))).ok();
                },
                runs,
            );
            let tree = iter.parse(&toks).expect("deep input parses iteratively");
            let nodes = node_count(&tree);
            println!(
                "{shape:<8} {depth:>9} {ns:>14.0} {:>10.2} {nodes:>8}",
                ns / depth as f64
            );
        }
    }
    println!(
        "\nNo overflow at any depth; ns/level is ~flat => O(depth), linear in nesting.\nReused stack capacity: {} frames.",
        iter.stack_capacity()
    );
}

/// Parse one deep input once with the chosen parser and report it.
/// `deep <rec|iter> <parens|prefix> <depth>`. `rec` at large depth WILL overflow
/// (that is the ceiling this spike removes) — run it only deliberately.
fn deep_once(args: &[String]) {
    let kind = args.get(2).map(String::as_str).unwrap_or("iter");
    let shape = args.get(3).map(String::as_str).unwrap_or("parens");
    let depth: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(100_000);
    let src = build(shape, depth);
    let toks = tokenize(&src).expect("deep input tokenizes");

    let start = Instant::now();
    let result: Result<Expr, _> = match kind {
        "rec" => rec_parse(&toks),
        "iter" => IterParser::new().parse(&toks),
        other => {
            eprintln!("deep kind must be rec|iter, got {other:?}");
            std::process::exit(2);
        }
    };
    let ns = start.elapsed().as_nanos();
    match result {
        Ok(tree) => println!(
            "ok {kind} {shape} depth={depth} nodes={} ns={ns}",
            node_count(&tree)
        ),
        Err(e) => println!("err {kind} {shape} depth={depth} {e:?}"),
    }
}

/// Drive the RECURSIVE parser to its stack-overflow ceiling on a fixed-size
/// thread stack, printing each depth it survives. This INTENTIONALLY aborts the
/// process at the ceiling — the last `ok ...` line printed is the recursive
/// limit. The iterative parser run for contrast never aborts.
fn ceiling(args: &[String]) {
    // Owned so the probe thread (which must be `'static`) can capture it.
    let shape = args
        .get(2)
        .map(String::as_str)
        .unwrap_or("parens")
        .to_string();
    // A small, documented stack so the ceiling is reproducible and quick to hit;
    // the real parser's ceiling scales the same way with stack size / frame size.
    const STACK_BYTES: usize = 1 << 20; // 1 MiB
    println!("Recursive ceiling probe, shape={shape}, thread stack={STACK_BYTES} bytes.");
    println!("(iterative sails past this; recursive aborts at the limit below)\n");

    let handle = std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(move || {
            // Show the iterative parser clearing a depth far beyond any recursive
            // ceiling on this same small stack, then find where recursion dies.
            let probe = build(&shape, 200_000);
            let toks = tokenize(&probe).expect("tokenizes");
            let ok = IterParser::with_capacity(1 << 20).parse(&toks).is_ok();
            println!("iterative parsed depth=200000 on a {STACK_BYTES}-byte stack: ok={ok}");

            let mut depth = 64usize;
            loop {
                let src = build(&shape, depth);
                let toks = tokenize(&src).expect("tokenizes");
                match rec_parse(&toks) {
                    Ok(_) => println!("ok recursive {shape} depth={depth}"),
                    Err(e) => {
                        println!("err recursive {shape} depth={depth} {e:?}");
                        break;
                    }
                }
                use std::io::Write;
                std::io::stdout().flush().ok();
                depth += 64;
            }
        })
        .expect("spawn ceiling probe thread");
    let _ = handle.join();
}

fn verify() {
    assert_parity();
    println!(
        "parity ok: recursive and iterative agree on the battery + deep (safe depth) + non-assoc rejection."
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build(shape: &str, depth: usize) -> String {
    match shape {
        "parens" => nested_parens(depth),
        "prefix" => prefix_chain(depth),
        other => {
            eprintln!("shape must be parens|prefix, got {other:?}");
            std::process::exit(2);
        }
    }
}

/// Both parsers agree on the shallow battery and on a (recursion-safe) deep depth.
fn assert_parity() {
    let mut iter = IterParser::new();
    for &(name, src) in SHALLOW_CASES {
        let toks = tokenize(src).expect("battery tokenizes");
        let r = rec_parse(&toks);
        let i = iter.parse(&toks);
        assert_eq!(r, i, "rec/iter disagree on {name:?} ({src:?})");
        assert!(r.is_ok(), "battery case {name:?} should parse");
    }
    // Non-associative chains reject the same way in both (`a = b = c`).
    let toks = tokenize("a = b = c").expect("tokenizes");
    assert_eq!(rec_parse(&toks), iter.parse(&toks));
    assert!(rec_parse(&toks).is_err(), "a = b = c must be rejected");
    // Deep, but shallow enough that the recursive comparison itself is safe.
    for shape in ["parens", "prefix"] {
        let toks = tokenize(&build(shape, 200)).expect("tokenizes");
        assert_eq!(rec_parse(&toks), iter.parse(&toks), "deep {shape} parity");
    }
}
