// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Adversarial scaling measurement: how the heap cost of parsing each width
//! (flat-list) family grows as the width grows, for OUR parser and for upstream
//! `sqlparser` on the identical inputs.
//!
//! Two sibling targets share this one module so they agree on exactly one set of
//! generators, one width ladder, one dialect pair, and one measurement path:
//!
//! - `adversarial_heap.rs`        — prints the per-family ours/theirs scaling table
//!   plus the linear-vs-super-linear verdict (runs on macOS + Linux).
//! - `tests/adversarial_scaling.rs` — asserts every core width family scales
//!   linearly (deterministic `dhat` counts), so a super-linear regression fails
//!   `cargo nextest run` locally.
//!
//! Each target consumes a different slice, so unused-per-target helpers are
//! expected; the module-level `allow(dead_code)` keeps `-D warnings` green without
//! scattering attributes (same convention as `upstream/mod.rs` and `corpus/mod.rs`).
//!
//! Both consumers install `dhat::Alloc` as the global allocator, so [`sample`]
//! reads real, deterministic counts. The comparison runs under the Postgres pair
//! (`Postgres` ↔ `PostgreSqlDialect`) — the superset both parsers accept widest —
//! exactly the pair the wall-clock testbed uses.
#![allow(dead_code)]

use crate::upstream::{HeapSample, sample};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser as UpstreamParser;
use squonk::dialect::Postgres;
use squonk::{StockParsed, parse_with};
use squonk_bench::adversarial::{ScaleSample, WIDTH_FAMILIES, WIDTH_LADDER, WidthFamily};

// ---------------------------------------------------------------------------
// Parse adapters (Postgres ↔ PostgreSqlDialect)
// ---------------------------------------------------------------------------

/// `true` iff our parser accepts `sql` under the Postgres preset.
pub fn ours_parses(sql: &str) -> bool {
    parse_with(sql, Postgres).is_ok()
}

/// `true` iff upstream accepts `sql` under `PostgreSqlDialect`. Upstream ships its
/// default features here (`recursive-protection`), so even the pathological inputs
/// reject cleanly rather than overflowing — which is exactly what lets us run them
/// in-process beside our own guard.
pub fn theirs_parses(sql: &str) -> bool {
    UpstreamParser::parse_sql(&PostgreSqlDialect {}, sql).is_ok()
}

/// Parse to our owned AST root, kept for the retained-heap read. Only called on a
/// width already proven to parse, so the `expect` cannot fire on the measured set.
pub fn parse_ours_owned(sql: &str) -> StockParsed {
    parse_with(sql, Postgres).expect("measured width parses (ours)")
}

/// Parse to upstream's owned AST (`Vec<Statement>`), kept for the retained-heap read.
pub fn parse_theirs_owned(sql: &str) -> Vec<sqlparser::ast::Statement> {
    UpstreamParser::parse_sql(&PostgreSqlDialect {}, sql).expect("measured width parses (theirs)")
}

// ---------------------------------------------------------------------------
// Heap measurement (dhat) — `HeapSample`/`sample` are imported from `upstream`
// (this module already links `sqlparser` directly for its own ours-vs-theirs
// parsing above, so mounting `upstream` alongside adds no new dependency; unlike
// `corpus/mod.rs`, which redefines its own copy specifically to avoid that link).
// ---------------------------------------------------------------------------

pub fn sample_ours(sql: &str) -> HeapSample {
    sample(|| parse_ours_owned(sql))
}

pub fn sample_theirs(sql: &str) -> HeapSample {
    sample(|| parse_theirs_owned(sql))
}

// ---------------------------------------------------------------------------
// Per-family scaling measurement
// ---------------------------------------------------------------------------

/// One width family's measured scaling curve under both parsers. Acceptance is
/// probed OUTSIDE the `sample` windows (the probe parses never land in a measured
/// count), and only widths a side actually accepts are sampled for it — the same
/// both-accept honesty the corpus/upstream harnesses use.
#[derive(Clone, Debug)]
pub struct FamilyScaling {
    pub name: &'static str,
    /// `(width, heap)` over the ladder widths OUR parser accepts.
    pub ours: Vec<(usize, HeapSample)>,
    /// `(width, heap)` over the ladder widths UPSTREAM accepts.
    pub theirs: Vec<(usize, HeapSample)>,
    /// Our parser accepts every ladder width — the precondition for gating the
    /// family's linearity (a partial curve cannot be a scaling verdict).
    pub ours_accepts_all: bool,
    /// Upstream accepts every ladder width.
    pub theirs_accepts_all: bool,
}

/// Measure one width family across the [`WIDTH_LADDER`] under both parsers.
pub fn measure_family(family: &WidthFamily) -> FamilyScaling {
    let mut ours = Vec::new();
    let mut theirs = Vec::new();
    let mut ours_accepts_all = true;
    let mut theirs_accepts_all = true;

    for &width in WIDTH_LADDER {
        let sql = (family.generate)(width);
        // Probe acceptance first, outside any sample window.
        if ours_parses(&sql) {
            ours.push((width, sample_ours(&sql)));
        } else {
            ours_accepts_all = false;
        }
        if theirs_parses(&sql) {
            theirs.push((width, sample_theirs(&sql)));
        } else {
            theirs_accepts_all = false;
        }
    }

    FamilyScaling {
        name: family.name,
        ours,
        theirs,
        ours_accepts_all,
        theirs_accepts_all,
    }
}

/// Measure every width family.
pub fn measure_all() -> Vec<FamilyScaling> {
    WIDTH_FAMILIES.iter().map(measure_family).collect()
}

/// Our transient-blocks curve for the linearity rule (allocation *count* vs width).
pub fn ours_blocks_series(fs: &FamilyScaling) -> Vec<ScaleSample> {
    fs.ours
        .iter()
        .map(|(w, h)| ScaleSample {
            width: *w,
            metric: h.transient_blocks,
        })
        .collect()
}

/// Our transient-bytes curve for the linearity rule (allocation *volume* vs width).
pub fn ours_bytes_series(fs: &FamilyScaling) -> Vec<ScaleSample> {
    fs.ours
        .iter()
        .map(|(w, h)| ScaleSample {
            width: *w,
            metric: h.transient_bytes,
        })
        .collect()
}

/// The ours/theirs transient-bytes ratio at the top ladder width both accept — the
/// "how much lighter are we on the worst case" record. `None` when no common width
/// was measured.
pub fn top_width_bytes_ratio(fs: &FamilyScaling) -> Option<(usize, f64)> {
    let ours_top = fs.ours.last()?;
    let theirs_top = fs.theirs.iter().rev().find(|(w, _)| *w == ours_top.0)?;
    if theirs_top.1.transient_bytes == 0 {
        return None;
    }
    Some((
        ours_top.0,
        ours_top.1.transient_bytes as f64 / theirs_top.1.transient_bytes as f64,
    ))
}
