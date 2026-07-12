// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Representative allocation gate (ADR-0016): exact `dhat::HeapStats` pins for a
//! handful of tokenize/parse micro fixtures, so a per-statement allocation regression
//! fails `cargo nextest run` locally.
//!
//! The pins are insta snapshots, not hand-written numbers: `dhat`'s counts are
//! deterministic run-to-run, so the snapshot is still an exact pin — an intentional
//! layout change re-baselines by reviewing the `.snap` diff (`cargo insta review`, or
//! move the generated `.snap.new` over the `.snap`), with the why recorded in the
//! commit rather than an ever-growing comment ledger here.
//!
//! Block counts are the allocation-COUNT signal — architecture-independent, so their
//! snapshot is asserted on every arch. Byte totals track AST node SIZES, which differ
//! across architectures (enum layout / niche / alignment: aarch64 vs x86-64), so the
//! byte snapshot is gated to the aarch64 dev reference — a size regression shows there
//! (same code) and the block counts guard every arch. See
//! `allocation-pins-are-architecture-specific`.

use squonk_bench::{
    ANALYTIC_SELECT, DDL_SLOT, DML_DELETE, DML_SLOT, DML_UPDATE, MULTI_STATEMENT_SELECTS,
    parse_ansi_sql, parse_postgres_sql, tokenize_sql,
};
use std::fmt::Write as _;
use std::hint::black_box;

/// Run `f` inside its own dhat profiler window and return the measured stats. The
/// profiler is dropped before the caller formats/asserts, so snapshot bookkeeping
/// never counts toward a later window.
fn measure(f: impl FnOnce()) -> dhat::HeapStats {
    let profiler = dhat::Profiler::builder().testing().build();
    f();
    let stats = dhat::HeapStats::get();
    drop(profiler);
    stats
}

#[test]
fn representative_allocation_counts_are_pinned() {
    // The tokenizer cases guard lexing; the parser cases guard the statement grammars
    // (SELECT, CREATE TABLE, INSERT, UPDATE, DELETE), so a regression in either
    // layer's allocation profile trips the build.
    let cases: &[(&str, fn())] = &[
        ("tokenize_analytic_select", || {
            black_box(tokenize_sql(black_box(ANALYTIC_SELECT)));
        }),
        ("tokenize_ddl_create_table", || {
            black_box(tokenize_sql(black_box(DDL_SLOT)));
        }),
        ("tokenize_dml_insert", || {
            black_box(tokenize_sql(black_box(DML_SLOT)));
        }),
        ("parse_ansi_analytic_select", || {
            black_box(parse_ansi_sql(black_box(ANALYTIC_SELECT)));
        }),
        ("parse_ansi_multi_statement", || {
            black_box(parse_ansi_sql(black_box(MULTI_STATEMENT_SELECTS)));
        }),
        ("parse_postgres_analytic_select", || {
            black_box(parse_postgres_sql(black_box(ANALYTIC_SELECT)));
        }),
        ("parse_ansi_ddl_create_table", || {
            black_box(parse_ansi_sql(black_box(DDL_SLOT)));
        }),
        ("parse_ansi_dml_insert", || {
            black_box(parse_ansi_sql(black_box(DML_SLOT)));
        }),
        ("parse_ansi_dml_update", || {
            black_box(parse_ansi_sql(black_box(DML_UPDATE)));
        }),
        ("parse_ansi_dml_delete", || {
            black_box(parse_ansi_sql(black_box(DML_DELETE)));
        }),
    ];

    let mut blocks = String::new();
    let mut bytes = String::new();
    for (name, f) in cases {
        let stats = measure(f);
        writeln!(
            blocks,
            "{name}: total_blocks={} max_blocks={}",
            stats.total_blocks, stats.max_blocks
        )
        .expect("writing to a String cannot fail");
        writeln!(
            bytes,
            "{name}: total_bytes={} max_bytes={}",
            stats.total_bytes, stats.max_bytes
        )
        .expect("writing to a String cannot fail");
    }

    insta::with_settings!({ prepend_module_to_snapshot => false }, {
        insta::assert_snapshot!("allocations__representative_blocks", blocks);
    });
    if cfg!(target_arch = "aarch64") {
        insta::with_settings!({ prepend_module_to_snapshot => false }, {
            insta::assert_snapshot!("allocations__representative_bytes_aarch64", bytes);
        });
    }
}
