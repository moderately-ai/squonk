// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Adversarial linear-scaling gate (ADR-0016 / ADR-0017).
//!
//! The robustness invariant this pins: parsing a flat-list construct (a long
//! operator chain, an M-way join, a long `IN`/`VALUES` list, a CTE chain) must cost
//! at most LINEARLY in its width — a parser that went quadratic on adversarial input
//! is a denial-of-service vector even without overflowing the stack. The `dhat`
//! allocation counts are deterministic run-to-run (like `corpus_allocations.rs`), so
//! measuring the curve across a doubling width ladder and asserting no step grows
//! faster than ~proportionally to the width is a real local gate, not a flaky
//! wall-clock check: a super-linear regression fails `cargo nextest run` here.
//!
//! The verdict is on the SCALING (the per-step growth factor), not on any absolute
//! byte count, so it is invariant under AST-layout changes — an extra field on a
//! node scales every width identically and the ratios are unchanged. That is the
//! deliberate difference from the exact-pin gates (`allocations.rs`,
//! `corpus_allocations.rs`): there is nothing to re-baseline on an ordinary layout
//! change, only on a genuine algorithmic-complexity regression. The cross-platform
//! `dhat` measurement is the linearity gate; the Linux `adversarial_instr` bench
//! adds the callgrind `Ir` compute-regression signal (Valgrind-only, hence its own
//! non-Linux skip), and the wall-clock testbed example reports the time curve a gate
//! must not (wall-clock flaps — ADR-0016).
//!
//! This module holds exactly ONE measured test on purpose: `dhat::Alloc` is the
//! binary-wide global allocator and counts process-wide allocations, so this gate
//! relies on nextest's per-test process isolation. The pure decision-rule's own unit
//! tests live in `src/adversarial.rs`, exactly as `ratio_gate_logic.rs` separates the
//! rule from the measurement.

#[path = "../benches/adversarial/mod.rs"]
mod adversarial;

use adversarial::{
    FamilyScaling, measure_all, ours_blocks_series, ours_bytes_series, top_width_bytes_ratio,
};
use squonk_bench::adversarial::{
    CORE_WIDTH_FAMILIES, DEFAULT_SCALING_SLACK, ScaleSample, WIDTH_LADDER, superlinear_steps,
};

/// Render the flagged super-linear steps for a failure message, so the assertion
/// itself reports the offending growth (the "read the failure to see the numbers"
/// workflow the other deterministic gates use).
fn describe_superlinear(samples: &[ScaleSample]) -> String {
    superlinear_steps(samples, DEFAULT_SCALING_SLACK)
        .iter()
        .map(|step| {
            format!(
                "width {}->{}: metric x{:.3} for width x{:.3} (allowed <= x{:.3})",
                step.from_width,
                step.to_width,
                step.metric_factor(),
                step.width_factor(),
                step.allowed_factor(DEFAULT_SCALING_SLACK),
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Assert one measured metric curve covers the whole ladder and is linear.
fn assert_linear(family: &str, metric: &str, samples: &[ScaleSample]) {
    assert_eq!(
        samples.len(),
        WIDTH_LADDER.len(),
        "{family}/{metric}: measured {} of {} ladder widths (incomplete curve)",
        samples.len(),
        WIDTH_LADDER.len(),
    );
    assert!(
        superlinear_steps(samples, DEFAULT_SCALING_SLACK).is_empty(),
        "{family}/{metric}: super-linear scaling (a complexity regression): {}",
        describe_superlinear(samples),
    );
}

#[test]
fn adversarial_width_families_scale_linearly() {
    let families = measure_all();

    // Anti-vanishing: every core family must still be present and accepted across the
    // whole ladder. A core flat-list family dropping out of our surface is a coverage
    // regression that would otherwise silently shrink the gate.
    for &core in CORE_WIDTH_FAMILIES {
        let fs = families
            .iter()
            .find(|f: &&FamilyScaling| f.name == core)
            .unwrap_or_else(|| panic!("core width family `{core}` missing from the registry"));
        assert!(
            fs.ours_accepts_all,
            "core width family `{core}`: our parser stopped accepting some ladder width \
             (a surface regression on a flat-list construct)"
        );
    }

    // Every both-accept family must scale linearly on both deterministic metrics.
    // Core families are mandatory (asserted above); a non-core family (cte_chain) is
    // gated opportunistically — exercised when both parsers accept the whole ladder,
    // logged and skipped if a future surface change drops it.
    for fs in &families {
        if !fs.ours_accepts_all {
            assert!(
                !CORE_WIDTH_FAMILIES.contains(&fs.name),
                "core family `{}` must accept every ladder width",
                fs.name
            );
            eprintln!(
                "note: skipping non-core family `{}` (our parser rejects some ladder width)",
                fs.name
            );
            continue;
        }
        assert_linear(fs.name, "transient_bytes", &ours_bytes_series(fs));
        assert_linear(fs.name, "transient_blocks", &ours_blocks_series(fs));

        // Record the ours-vs-theirs contrast at the worst case: how much lighter our
        // owned AST is to build than upstream's on the same pathological width. This
        // is the relative-scaling signal the ticket asks be recorded; the durable
        // table lives in the `compare_adversarial` example output.
        if let Some((width, ratio)) = top_width_bytes_ratio(fs) {
            eprintln!(
                "[{}] ours/theirs transient bytes at width {width}: {ratio:.3}",
                fs.name
            );
        }
    }
}
