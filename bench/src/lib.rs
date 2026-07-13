// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Benchmarks for `squonk` (see `benches/`). This crate is never published,
//! so perf-only dependencies stay out of the published crates' dependency graphs.

/// Deterministic adversarial / pathological SQL generators + the pure scaling
/// decision-rule, shared by the adversarial heap bench, instruction bench, scaling
/// gate, recursion gate, and wall-clock testbed.
pub mod adversarial;

/// The standardized CLI + timing helpers shared by the ours-vs-external comparison
/// examples (`examples/compare_*.rs`). Relocated here so every comparison binary
/// parses the same flags and reports the same shape; the measurement logic itself
/// stays in the per-comparison `benches/*/mod.rs` modules those examples mount.
pub mod compare;

use squonk::dialect::{Ansi, DuckDb, Lenient, Postgres, Sqlite};
use squonk::tokenizer::tokenize;
use squonk::{Parsed, parse_with};
use squonk_ast::render::{RenderConfig, RenderCtx, RenderExt as _, RenderMode};
use std::fmt::Write as _;
use std::hint::black_box;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct BenchCase {
    pub name: &'static str,
    pub sql: &'static str,
}

pub const SIMPLE_SELECT: &str = "SELECT a, b FROM t WHERE a = b";
pub const ANALYTIC_SELECT: &str = "SELECT a, b + c AS x FROM t LEFT JOIN t AS x ON a = b WHERE a > 0 GROUP BY a, b HAVING b <> c ORDER BY x DESC NULLS LAST LIMIT 0 OFFSET 0";
pub const SET_SELECT: &str = "SELECT a FROM t UNION ALL SELECT b FROM t EXCEPT SELECT c FROM t";
pub const DDL_SLOT: &str = "CREATE TABLE t (id INT, name TEXT)";
pub const DDL_INDEX: &str = "CREATE INDEX idx_t_name ON t (name)";
pub const DML_SLOT: &str = "INSERT INTO t (id, name) VALUES (1, 'a')";
pub const DML_UPDATE: &str = "UPDATE t SET name = 'b' WHERE id = 1";
pub const DML_DELETE: &str = "DELETE FROM t WHERE id = 1";
pub const DEEP_NESTED_SELECT: &str =
    "SELECT ((((((((((a + b) * c) - a) / b) + c) * a) - b) / c) + a) * b) FROM t";
pub const MULTI_STATEMENT_SELECTS: &str = "SELECT a FROM t WHERE a = 1; SELECT b FROM u WHERE b <> 2; SELECT c FROM v ORDER BY c LIMIT 10";

pub const TOKENIZER_CASES: &[BenchCase] = &[
    BenchCase {
        name: "simple_select",
        sql: SIMPLE_SELECT,
    },
    BenchCase {
        name: "analytic_select",
        sql: ANALYTIC_SELECT,
    },
    BenchCase {
        name: "set_select",
        sql: SET_SELECT,
    },
    BenchCase {
        name: "ddl_create_table",
        sql: DDL_SLOT,
    },
    BenchCase {
        name: "ddl_create_index",
        sql: DDL_INDEX,
    },
    BenchCase {
        name: "dml_insert",
        sql: DML_SLOT,
    },
    BenchCase {
        name: "dml_update",
        sql: DML_UPDATE,
    },
    BenchCase {
        name: "dml_delete",
        sql: DML_DELETE,
    },
    BenchCase {
        name: "deep_nesting",
        sql: DEEP_NESTED_SELECT,
    },
    BenchCase {
        name: "multi_statement",
        sql: MULTI_STATEMENT_SELECTS,
    },
];

// Parser-bench corpus. Mirrors `TOKENIZER_CASES` but omits families the parser
// cannot yet handle: `ddl_create_index` (CREATE INDEX) tokenizes but has no
// statement grammar, so it stays tokenizer-only until its ticket lands (ADR-0016
// keeps perf coverage tracking the actual parser surface, not aspirational SQL).
pub const PARSER_CASES: &[BenchCase] = &[
    BenchCase {
        name: "simple_select",
        sql: SIMPLE_SELECT,
    },
    BenchCase {
        name: "analytic_select",
        sql: ANALYTIC_SELECT,
    },
    BenchCase {
        name: "set_select",
        sql: SET_SELECT,
    },
    BenchCase {
        name: "ddl_create_table",
        sql: DDL_SLOT,
    },
    BenchCase {
        name: "dml_insert",
        sql: DML_SLOT,
    },
    BenchCase {
        name: "dml_update",
        sql: DML_UPDATE,
    },
    BenchCase {
        name: "dml_delete",
        sql: DML_DELETE,
    },
    BenchCase {
        name: "deep_nesting",
        sql: DEEP_NESTED_SELECT,
    },
    BenchCase {
        name: "multi_statement",
        sql: MULTI_STATEMENT_SELECTS,
    },
];

pub fn tokenize_sql(sql: &str) -> usize {
    tokenize(sql).expect("benchmark SQL tokenizes").len()
}

pub fn parse_ansi_sql(sql: &str) -> usize {
    parse_ansi(sql).statements().len()
}

pub fn parse_postgres_sql(sql: &str) -> usize {
    parse_postgres(sql).statements().len()
}

// The three post-Ansi/Postgres dialects (ADR-0009/0011 presets). Each mirrors
// `parse_postgres_sql` so the per-dialect perf series (`perf_testbed dialects`, the
// future gungraun Ir gate) drives the same monomorphized `Parser<D>` a consumer would,
// with no extra passes — the intra-crate dialect/Ansi ratio is the load-tolerant number.
pub fn parse_sqlite_sql(sql: &str) -> usize {
    parse_sqlite(sql).statements().len()
}

pub fn parse_duckdb_sql(sql: &str) -> usize {
    parse_duckdb(sql).statements().len()
}

pub fn parse_lenient_sql(sql: &str) -> usize {
    parse_lenient(sql).statements().len()
}

pub fn render_canonical_sql(sql: &str) -> usize {
    render_sql(&parse_ansi(sql), RenderMode::Canonical).len()
}

pub fn render_parenthesized_sql(sql: &str) -> usize {
    render_sql(&parse_ansi(sql), RenderMode::Parenthesized).len()
}

fn parse_ansi(sql: &str) -> Parsed {
    parse_with(sql, squonk::ParseConfig::new(Ansi)).expect("benchmark SQL parses")
}

fn parse_postgres(sql: &str) -> Parsed {
    parse_with(sql, squonk::ParseConfig::new(Postgres)).expect("benchmark SQL parses")
}

fn parse_sqlite(sql: &str) -> Parsed {
    parse_with(sql, squonk::ParseConfig::new(Sqlite)).expect("benchmark SQL parses")
}

fn parse_duckdb(sql: &str) -> Parsed {
    parse_with(sql, squonk::ParseConfig::new(DuckDb)).expect("benchmark SQL parses")
}

fn parse_lenient(sql: &str) -> Parsed {
    parse_with(sql, squonk::ParseConfig::new(Lenient)).expect("benchmark SQL parses")
}

fn render_sql(parsed: &Parsed, mode: RenderMode) -> String {
    let config = RenderConfig {
        mode,
        ..RenderConfig::default()
    };
    let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
    let mut out = String::new();
    for (i, statement) in parsed.statements().iter().enumerate() {
        if i > 0 {
            out.push_str("; ");
        }
        write!(out, "{}", statement.displayed(&ctx)).expect("rendering to String cannot fail");
    }
    out
}

// ---------------------------------------------------------------------------
// Shared wall-clock timing helpers — the warm-up + timed-loop microbenchmark shape
// every hand-timed example/spike needs. Needs only `Instant` + `black_box`, so it
// lives here rather than re-declared per file (perf_testbed / adversarial_testbed /
// arena_spike / lexing_spike were all byte-identical copies of one of the two).
// ---------------------------------------------------------------------------

/// Time `f` over `iters` iterations after a warm-up; return nanoseconds per call.
/// The iteration count is parameter-derived (never timing-derived), so the work is
/// deterministic even though the resulting nanoseconds are not.
pub fn time_loop(f: impl Fn(&str) -> usize, sql: &str, iters: u64) -> f64 {
    for _ in 0..(iters / 20).max(1) {
        black_box(f(black_box(sql)));
    }
    let start = Instant::now();
    for _ in 0..iters {
        black_box(f(black_box(sql)));
    }
    start.elapsed().as_nanos() as f64 / iters as f64
}

/// Time `f` over `iters` iterations after a warm-up; return nanoseconds per call.
/// Unlike [`time_loop`], `f` takes no argument and returns the `u64` sink directly —
/// the shape a closure-driven microbenchmark over a pre-built input needs.
pub fn time_ns(iters: u64, mut f: impl FnMut() -> u64) -> f64 {
    for _ in 0..(iters / 20).max(1) {
        black_box(f());
    }
    let start = Instant::now();
    let mut sink = 0u64;
    for _ in 0..iters {
        sink = sink.wrapping_add(black_box(f()));
    }
    let elapsed = start.elapsed().as_nanos() as f64;
    black_box(sink);
    elapsed / iters as f64
}
