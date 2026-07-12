// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Pure decision-rule tests for the upstream ratio gate (ADR-0016).
//!
//! The live gate in `tests/upstream_gate.rs` only proves the tree is currently
//! clean; it cannot prove the gate would CATCH a regression, because today there
//! is none. These tests exercise `detect_regressions` directly with synthetic
//! rows, so the threshold rule is verified independently of live measurement.
//!
//! No `dhat` profiler window here on purpose: this file tests the decision rule
//! without live allocation measurement.

use crate::upstream;
use upstream::{DEFAULT_RATIO_SLACK, detect_regressions};

/// Build the five canonical metric rows (same order `rows`/`baseline_rows` use)
/// from `ours` and `theirs` arrays; only the numbers matter for these tests.
fn rows(ours: [u64; 5], theirs: [u64; 5]) -> [(&'static str, u64, u64); 5] {
    [
        ("transient_bytes", ours[0], theirs[0]),
        ("transient_blocks", ours[1], theirs[1]),
        ("retained_bytes", ours[2], theirs[2]),
        ("retained_blocks", ours[3], theirs[3]),
        ("peak_bytes", ours[4], theirs[4]),
    ]
}

#[test]
fn within_slack_and_improvements_pass() {
    let theirs = [1000; 5];
    let base = rows([100; 5], theirs);
    // transient_bytes +4% (under the 5% slack), retained_bytes improved (-10%),
    // the rest unchanged: nothing should be flagged.
    let current = rows([104, 100, 90, 100, 100], theirs);
    let found = detect_regressions("p", &base, &current, DEFAULT_RATIO_SLACK);
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn equal_ratios_pass() {
    let theirs = [1000, 2000, 3000, 4000, 5000];
    let rows_eq = rows([100, 200, 300, 400, 500], theirs);
    let found = detect_regressions("p", &rows_eq, &rows_eq, DEFAULT_RATIO_SLACK);
    assert!(
        found.is_empty(),
        "identical ratios must not regress: {found:#?}"
    );
}

#[test]
fn beyond_slack_is_flagged() {
    let theirs = [1000; 5];
    let base = rows([100; 5], theirs);
    // transient_bytes +6% (over the 5% slack); everything else unchanged.
    let current = rows([106, 100, 100, 100, 100], theirs);
    let found = detect_regressions("p", &base, &current, DEFAULT_RATIO_SLACK);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].metric, "transient_bytes");
    assert!(found[0].current_ratio > found[0].allowed_ratio);
    assert!((found[0].baseline_ratio - 0.100).abs() < 1e-9);
}

#[test]
fn tighter_slack_flags_more() {
    let theirs = [1000; 5];
    let base = rows([100; 5], theirs);
    // +4% and +3% on two metrics; passes at 5% slack, both trip at 1%.
    let current = rows([104, 103, 100, 100, 100], theirs);
    assert!(detect_regressions("p", &base, &current, 0.05).is_empty());
    let strict = detect_regressions("p", &base, &current, 0.01);
    assert_eq!(strict.len(), 2, "{strict:#?}");
}
