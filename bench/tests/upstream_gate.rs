// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Thresholded upstream ours/theirs ratio gate (ADR-0016 / ADR-0017).
//!
//! Re-measures the heap ratios with the SAME shared harness (`benches/upstream/mod.rs`)
//! that `examples/compare_upstream.rs` heap mode uses to write `upstream-baseline.json`,
//! reads the committed snapshot back, and fails (default) — or warns, if
//! `SQUONK_RATIO_GATE_WARN` is set — when any ours/theirs ratio has regressed past
//! the baseline by more than the slack (`SQUONK_RATIO_GATE_SLACK`, default 5%). The
//! dhat counts are deterministic, so this is a real local gate, not a flaky wall-clock
//! check (ADR-0017: local-runnable). On an intentional change, refresh the baseline with
//! `cargo run -p squonk-bench --release --example compare_upstream --features
//! compare-heap -- --update-baseline` and review the diff. The fail-vs-warn CI policy and
//! CI invocation are deferred to `prod-adr-perf-production-gate` so there is one perf-gate
//! policy, not two.
//!
//! This module holds exactly ONE measured test on purpose: `dhat::Alloc` is the
//! binary-wide global allocator and counts process-wide allocations, so this gate
//! relies on nextest's per-test process isolation. The pure decision-rule tests live
//! in `tests/ratio_gate_logic.rs`, which opens no dhat profiler window.

use crate::upstream;

#[test]
fn upstream_ours_theirs_ratio_gate() {
    // Coverage is exact (deterministic, no slack): a complex-corpus query dropping
    // out of — or newly entering — either parser's surface is always a reviewable
    // change. Checked before the ratios, since a silently shrinking both-accept
    // subset could otherwise mask a ratio regression. The same fail-vs-warn switch
    // applies, so a deliberate surface change can be acknowledged the same way.
    let coverage = upstream::verify_complex_coverage();

    let slack = upstream::ratio_slack();
    let regressions = upstream::measure_ratio_regressions(slack)
        .unwrap_or_else(|err| panic!("upstream ratio gate could not run: {err}"));

    let mut report = String::new();
    if let Err(drift) = &coverage {
        report.push_str(drift);
        report.push('\n');
    }
    if !regressions.is_empty() {
        report.push_str(&upstream::format_regressions(&regressions, slack));
    }
    if report.is_empty() {
        return;
    }
    if upstream::gate_is_warn_only() {
        eprintln!("warning ({} set): {report}", upstream::RATIO_WARN_ENV);
    } else {
        panic!("{report}");
    }
}
