// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Adversarial-stress wall-clock testbed — ours vs upstream `sqlparser-rs`.
//!
//! The time-and-robustness companion to the deterministic gates: it prints the
//! parse-TIME scaling curve (which a gate must NOT pin — wall-clock flaps on shared
//! runners, ADR-0016) and the recursion-limit contrast, so a human can read the
//! curve and the graceful-reject-vs-overflow story the gates only enforce. The heap
//! scaling and the linear-scaling verdict are deterministic and live in the
//! `compare_adversarial` example + `tests/adversarial_scaling.rs`; this is the wall-clock
//! and policy narrative.
//!
//! Timing is meaningful ONLY in a release / `--profile profiling` build — the
//! workspace builds dev deps at `opt-level = 1`, so a debug ours-vs-theirs number
//! lies. Build with symbols, then run:
//!   cargo build --profile profiling --example adversarial_testbed -p squonk-bench
//!   ./target/profiling/examples/adversarial_testbed            # width scaling table
//!   ./target/profiling/examples/adversarial_testbed recursion  # recursion-limit contrast

use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser as UpstreamParser;
use squonk::dialect::Postgres;
use squonk::error::ParseErrorKind;
use squonk::{DEFAULT_RECURSION_LIMIT, ParseOptions, parse_with, parse_with_options};
use squonk_bench::adversarial::{DEPTH_FAMILIES, WIDTH_FAMILIES, WIDTH_LADDER, nested_parens};
use squonk_bench::{parse_postgres_sql, time_loop};

/// Parse with upstream `sqlparser` (`PostgreSqlDialect`).
fn theirs(sql: &str) -> usize {
    UpstreamParser::parse_sql(&PostgreSqlDialect {}, sql)
        .expect("theirs parses the testbed case")
        .len()
}

/// Iterations for one width: more for the cheap small widths, fewer for the
/// expensive wide ones, so every cell does roughly constant total work. Purely a
/// function of the width — no wall-clock feedback.
fn iters_for(width: usize) -> u64 {
    (2_000_000 / width as u64).max(50)
}

fn bench_table() {
    println!("# adversarial width scaling (wall-clock, --profile profiling only)");
    println!("#   pair = Postgres <-> PostgreSqlDialect; o/t = ours ns / theirs ns");
    println!(
        "{:<16} {:>7} {:>12} {:>12} {:>8}",
        "family", "width", "ours ns", "theirs ns", "o/t"
    );
    for family in WIDTH_FAMILIES {
        for &width in WIDTH_LADDER {
            let sql = (family.generate)(width);
            // Keep the harness honest if the generated surface ever drifts past
            // either parser.
            if parse_with(&sql, Postgres).is_err()
                || UpstreamParser::parse_sql(&PostgreSqlDialect {}, &sql).is_err()
            {
                println!(
                    "{:<16} {width:>7}   (skipped: one parser rejects it)",
                    family.name
                );
                continue;
            }
            let iters = iters_for(width);
            let o = time_loop(parse_postgres_sql, &sql, iters);
            let t = time_loop(theirs, &sql, iters);
            println!(
                "{:<16} {width:>7} {o:>12.0} {t:>12.0} {:>8.2}",
                family.name,
                o / t
            );
        }
    }
}

/// Our outcome on `sql`, as a label: accepted, the clean recursion rejection, or any
/// other (syntax) rejection.
fn ours_outcome(sql: &str) -> &'static str {
    match parse_with(sql, Postgres) {
        Ok(_) => "accept",
        Err(e) if e.kind == ParseErrorKind::RecursionLimitExceeded => "reject(recursion)",
        Err(_) => "reject(syntax)",
    }
}

/// Upstream's outcome on `sql`: accepted, or rejected (its default
/// `recursive-protection` rejects deep nesting cleanly rather than overflowing).
fn theirs_outcome(sql: &str) -> &'static str {
    if UpstreamParser::parse_sql(&PostgreSqlDialect {}, sql).is_ok() {
        "accept"
    } else {
        "reject"
    }
}

fn recursion_report() {
    println!("# recursion-limit contrast (ours default limit = {DEFAULT_RECURSION_LIMIT})");
    println!("#   neither parser overflows: both turn deep nesting into a clean rejection.");
    println!("#   ours admits deeper legitimate nesting (limit 128) than upstream (~50 budget).");
    let depths = [16usize, 64, DEFAULT_RECURSION_LIMIT + 16, 200];
    for family in DEPTH_FAMILIES {
        println!("#");
        println!("# [{}]", family.name);
        println!("#   {:>6}  {:>18}  {:>8}", "depth", "ours", "theirs");
        for &depth in &depths {
            let sql = (family.generate)(depth);
            println!(
                "#   {depth:>6}  {:>18}  {:>8}",
                ours_outcome(&sql),
                theirs_outcome(&sql)
            );
        }
    }

    // The configurable knob: the very same input rejects under a tight limit and
    // parses under a generous one.
    println!("#");
    println!("# configurable limit (input = SELECT with 40 nested parens):");
    let sql = nested_parens(40);
    for limit in [20usize, 200] {
        let outcome = match parse_with_options(
            &sql,
            Postgres,
            ParseOptions::default().with_recursion_limit(limit),
        ) {
            Ok(_) => "accept",
            Err(e) if e.kind == ParseErrorKind::RecursionLimitExceeded => "reject(recursion)",
            Err(_) => "reject(syntax)",
        };
        println!("#   limit {limit:>3}: {outcome}");
    }
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "recursion" => recursion_report(),
        _ => bench_table(),
    }
}
