// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Parser perf profiling testbed — ours vs upstream `sqlparser-rs`.
//!
//! A dedicated, reusable harness for understanding WHERE parse CPU goes. It parses
//! a representative complex query in a tight loop so a sampling profiler
//! (`samply`) attributes the time, and a `bench` mode prints apples-to-apples
//! wall-clock ratios (ours / theirs) with the tokenizer share broken out.
//!
//! The goal (ADR-0016) is 10x on BOTH compute and memory; this testbed is for the
//! compute axis. The embedded cases are all accepted by both parsers under the
//! Postgres preset (no INTERVAL/window/recursive-CTE yet), so the comparison is
//! the both-accept intersection, not a coverage artefact.
//!
//! A second `dialects` mode prints the per-dialect envelope: each shared case's parse
//! time under Sqlite/DuckDb/Lenient as a ratio to the Ansi baseline (the intra-crate,
//! load-tolerant number — a preset whose flags add hot-path work shows a ratio > 1.0
//! even on a loaded box). It surfaces a silent throughput regression from the dialect
//! presets that the Ansi/Postgres-only instruments cannot see.
//!
//! Build with symbols, then profile or measure:
//!   cargo build --profile profiling --example perf_testbed -p squonk-bench
//!   ./target/profiling/examples/perf_testbed bench                       # ours/theirs wall-clock table
//!   ./target/profiling/examples/perf_testbed dialects                    # per-dialect / Ansi ratio table
//!   samply record -- ./target/profiling/examples/perf_testbed ours star_join 3000000
//!   samply record -- ./target/profiling/examples/perf_testbed theirs star_join 3000000
//!   samply record -- ./target/profiling/examples/perf_testbed tokenize star_join 3000000

// Bench-only fast allocator (ADR-0017: never the production allocator). This profiler
// runs under mimalloc — the realistic allocator a perf-sensitive consumer ships —
// matching the COMPUTE comparison benches and separate from the dhat MEMORY suite. The
// `bench`-mode wall-clock table runs both `ours` and `theirs` (sqlparser-rs) under it:
// fair, same allocator both sides.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::hint::black_box;

use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser as UpstreamParser;
use squonk::dialect::{Ansi, DuckDb, Lenient, Postgres, Sqlite};
use squonk::parse_with;
use squonk::tokenizer::tokenize;
use squonk_bench::{
    parse_ansi_sql, parse_duckdb_sql, parse_lenient_sql, parse_postgres_sql, parse_sqlite_sql,
    time_loop,
};

/// A TPC-DS-shaped star join (implicit comma joins, aggregates, OR-predicate).
const STAR_JOIN: &str = "SELECT i_item_id, AVG(ss_quantity) AS agg1, AVG(ss_list_price) AS agg2, AVG(ss_coupon_amt) AS agg3 FROM store_sales, customer_demographics, date_dim, item, promotion WHERE ss_sold_date_sk = d_date_sk AND ss_item_sk = i_item_sk AND ss_cdemo_sk = cd_demo_sk AND ss_promo_sk = p_promo_sk AND cd_gender = 'M' AND cd_marital_status = 'S' AND cd_education_status = 'College' AND d_year = 2000 AND (p_channel_email = 'N' OR p_channel_event = 'N') GROUP BY i_item_id ORDER BY i_item_id";

/// A CTE feeding a join (WITH + aggregate + qualified columns).
const CTE_JOIN: &str = "WITH revenue AS (SELECT ss_store_sk AS store_sk, SUM(ss_ext_sales_price) AS revenue FROM store_sales, date_dim WHERE ss_sold_date_sk = d_date_sk GROUP BY ss_store_sk) SELECT s_store_name, s_store_id, r.revenue FROM store AS s, revenue AS r WHERE s.s_store_sk = r.store_sk ORDER BY s_store_name, r.revenue";

/// Deeply nested arithmetic + boolean — stresses the Pratt expression climb.
const NESTED_EXPR: &str = "SELECT ((((a + b) * (c - d)) / ((e + f) * (g - h))) + (((i * j) - (k / l)) + ((m + n) * (o - p)))) AS x FROM t WHERE (a > b AND c < d) OR (e = f AND g <> h) OR (i >= j AND k <= l) OR (m > n AND o < p)";

/// Wide projection + explicit joins + a long AND chain.
const WIDE_SELECT: &str = "SELECT c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14, c15, c16, c17, c18, c19, c20 FROM t1 JOIN t2 ON t1.id = t2.id JOIN t3 ON t2.id = t3.id JOIN t4 ON t3.id = t4.id WHERE c1 = 1 AND c2 = 2 AND c3 = 3 AND c4 = 4 AND c5 = 5 AND c6 = 6 AND c7 = 7 AND c8 = 8 AND c9 = 9 AND c10 = 10";

/// A minimal SELECT — the fixed-overhead baseline.
const SIMPLE: &str = "SELECT a, b FROM t WHERE a = b";

const CASES: &[(&str, &str)] = &[
    ("star_join", STAR_JOIN),
    ("cte_join", CTE_JOIN),
    ("nested_expr", NESTED_EXPR),
    ("wide_select", WIDE_SELECT),
    ("simple", SIMPLE),
];

fn case_sql(name: &str) -> &'static str {
    CASES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, sql)| *sql)
        .unwrap_or(STAR_JOIN)
}

/// Parse with upstream sqlparser-rs (PostgreSqlDialect).
fn theirs(sql: &str) -> usize {
    UpstreamParser::parse_sql(&PostgreSqlDialect {}, sql)
        .expect("theirs parses the testbed case")
        .len()
}

/// Our tokenizer alone (no parse) — isolates the lexer's share of our cost.
fn tokenize_only(sql: &str) -> usize {
    tokenize(sql).expect("tokenizes").len()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("bench");
    let case = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "star_join".to_string());

    match mode {
        "sizes" => print_sizes(),
        // Tight single-case loops, one parser only — for `samply record`.
        "ours" => loop_mode(parse_postgres_sql, &case, &args, "ours"),
        "theirs" => loop_mode(theirs, &case, &args, "theirs"),
        "tokenize" => loop_mode(tokenize_only, &case, &args, "tokenize"),
        // Per-dialect / Ansi wall-clock ratio table over every case.
        "dialects" => bench_dialects(),
        // Wall-clock comparison table over every case.
        _ => bench_table(),
    }
}

fn loop_mode(f: impl Fn(&str) -> usize, case: &str, args: &[String], label: &str) {
    let iters: u64 = args
        .get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3_000_000);
    let sql = case_sql(case);
    let mut sink = 0usize;
    for _ in 0..iters {
        sink = sink.wrapping_add(black_box(f(black_box(sql))));
    }
    eprintln!("[{label}] case={case} iters={iters} sink={sink}");
}

/// Print the sizes that govern the per-peek return cost: the peek hot path
/// (`Parser::peek`/`peek_nth`/`advance`) returns the 16-byte
/// `Result<Option<Token>, LexError>` by value on every peek — a lexical fault is the
/// only way a peek fails — while a grammar method's `?` widens that to the 64-byte
/// `Result<Option<Token>, ParseError>` on the error path only, so the wide result is
/// never the per-peek cost.
fn print_sizes() {
    use std::mem::size_of;
    type Tok = squonk::tokenizer::Token;
    type PErr = squonk::error::ParseError;
    type LErr = squonk::tokenizer::LexError;
    println!(
        "ours  Token                                = {} B",
        size_of::<Tok>()
    );
    println!(
        "ours  Span                                 = {} B",
        size_of::<squonk_ast::Span>()
    );
    println!(
        "ours  ParseError                           = {} B",
        size_of::<PErr>()
    );
    println!(
        "ours  LexError                             = {} B",
        size_of::<LErr>()
    );
    println!(
        "ours  Option<Token>                        = {} B",
        size_of::<Option<Tok>>()
    );
    println!(
        "ours  Result<Option<Token>, ParseError>    = {} B   <- grammar result (LexError widened via ?)",
        size_of::<Result<Option<Tok>, PErr>>()
    );
    println!(
        "ours  Result<Option<Token>, LexError>      = {} B   <- returned per Parser::peek/peek_nth/advance",
        size_of::<Result<Option<Tok>, LErr>>()
    );
    println!(
        "theirs sqlparser::tokenizer::Token         = {} B   (upstream; owned data, not Copy)",
        size_of::<sqlparser::tokenizer::Token>()
    );
}

fn bench_table() {
    // ~20ms-of-work per cell, scaled cheaply by query length so short cases get
    // more iterations. Deterministic count, no timing-derived control flow.
    println!(
        "{:<13} {:>11} {:>11} {:>8} {:>11} {:>8}",
        "case", "ours ns", "theirs ns", "o/t", "tok ns", "tok%"
    );
    for &(name, sql) in CASES {
        // Skip a case only if a parser genuinely rejects it (keeps the harness
        // honest if the embedded SQL drifts past either surface).
        if parse_with(sql, Postgres).is_err()
            || UpstreamParser::parse_sql(&PostgreSqlDialect {}, sql).is_err()
        {
            println!("{name:<13}  (skipped: one parser rejects it)");
            continue;
        }
        let iters = 60_000;
        let o = time_loop(parse_postgres_sql, sql, iters);
        let t = time_loop(theirs, sql, iters);
        let tok = time_loop(tokenize_only, sql, iters);
        println!(
            "{name:<13} {o:>11.0} {t:>11.0} {:>8.2} {tok:>11.0} {:>7.0}%",
            o / t,
            100.0 * tok / o
        );
    }
}

/// Per-dialect wall-clock envelope over the shared cases: each measured dialect's parse
/// time (Sqlite / DuckDb / Lenient) as a ratio to the Ansi baseline.
///
/// The RATIO, not the absolute ns, is the deliverable — the same intra-crate relative
/// framing as the ours/theirs table, load-tolerant enough to read on a busy box. A
/// preset whose flags add hot-path work (new scan-dispatch arms, a larger
/// keyword/reserved table, Lenient's multi-quote scan) surfaces as a ratio > 1.0 that
/// the Ansi/Postgres-only instruments cannot see. A case is timed only when EVERY series
/// dialect (the Ansi baseline included) accepts it; one that does not is skipped with the
/// rejecting dialect named, so the harness stays honest if the embedded SQL or a preset
/// drifts past a dialect's surface (rather than turning a reject into an `.expect` panic).
fn bench_dialects() {
    println!(
        "{:<13} {:>10} {:>10} {:>6} {:>10} {:>6} {:>10} {:>6}",
        "case", "ansi ns", "sqlite ns", "sq/an", "duckdb ns", "dk/an", "lenient ns", "ln/an"
    );
    for &(name, sql) in CASES {
        let checks = [
            ("ansi", parse_with(sql, Ansi).is_ok()),
            ("sqlite", parse_with(sql, Sqlite).is_ok()),
            ("duckdb", parse_with(sql, DuckDb).is_ok()),
            ("lenient", parse_with(sql, Lenient).is_ok()),
        ];
        if let Some((rejecter, _)) = checks.iter().find(|(_, accepts)| !accepts) {
            println!("{name:<13}  (skipped: {rejecter} rejects it)");
            continue;
        }
        let iters = 60_000;
        let a = time_loop(parse_ansi_sql, sql, iters);
        let s = time_loop(parse_sqlite_sql, sql, iters);
        let d = time_loop(parse_duckdb_sql, sql, iters);
        let l = time_loop(parse_lenient_sql, sql, iters);
        println!(
            "{name:<13} {a:>10.0} {s:>10.0} {:>6.2} {d:>10.0} {:>6.2} {l:>10.0} {:>6.2}",
            s / a,
            d / a,
            l / a,
        );
    }
}
