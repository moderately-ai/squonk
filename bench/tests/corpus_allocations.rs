// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Corpus-scale allocation gate (ADR-0016 / ADR-0017).
//!
//! `tests/allocations.rs` pins the allocation profile of a handful of micro
//! fixtures; this pins the AGGREGATE profile of parsing whole vendored conformance
//! corpora, so a *scale* regression — a per-node allocation creeping up across
//! hundreds of statements — fails `cargo nextest run` locally even when no single
//! micro fixture moves enough to notice. The numbers come from the same
//! `corpus::measure` path the `corpus_heap` bench prints, so the bench and the gate
//! can never disagree. `dhat`'s counts are deterministic run-to-run, so the
//! aggregate totals are exact pins, snapshotted with insta (like `allocations.rs`):
//! an intentional change re-baselines by reviewing the `.snap` diff, with the why
//! recorded in the commit. The subset-size coverage pins stay direct asserts — they
//! guard fixture drift, not allocation, and their failure messages name the corpus
//! that moved.
//!
//! This module holds exactly ONE measured test on purpose: `dhat::Alloc` is the
//! binary-wide global allocator and counts process-wide allocations, so this gate
//! relies on nextest's per-test process isolation. The subset-size pins therefore
//! live INSIDE this one test rather than in a sibling.

#[path = "../benches/corpus/mod.rs"]
mod corpus;

use corpus::{Preset, Subset, Totals, measure};
use std::fmt::Write as _;

/// Pin the per-corpus candidate/included counts (and the totals), so a statement
/// silently vanishing from a fixture — or quietly dropping out of the parseable
/// surface — fails here with a precise message instead of only nudging the
/// allocation totals.
fn assert_coverage(
    s: &Subset,
    total_candidates: usize,
    included: usize,
    per_corpus: &[(&str, usize, usize)],
) {
    let label = s.preset.label();
    assert_eq!(
        s.coverage.len(),
        per_corpus.len(),
        "{label}: corpus count changed; update the coverage pins"
    );
    for (cov, &(name, candidates, inc)) in s.coverage.iter().zip(per_corpus) {
        assert_eq!(cov.corpus, name, "{label}: corpus order changed");
        assert_eq!(
            cov.candidates, candidates,
            "{label}: {name} candidate count changed (fixture drifted?)"
        );
        assert_eq!(cov.included, inc, "{label}: {name} parseable count changed");
    }
    assert_eq!(
        s.total_candidates, total_candidates,
        "{label}: total candidate count changed"
    );
    assert_eq!(
        s.included.len(),
        included,
        "{label}: measured subset size changed"
    );
}

/// Append one preset's aggregate heap metrics to the two snapshot tables. Block
/// counts are architecture-independent (the allocation-COUNT signal) and snapshotted
/// on every arch; byte totals track arch-dependent AST node sizes (aarch64 vs x86-64
/// layout), so their snapshot is gated to the aarch64 dev reference. See
/// `allocation-pins-are-architecture-specific`.
fn record_totals(label: &str, totals: &Totals, blocks: &mut String, bytes: &mut String) {
    writeln!(
        blocks,
        "{label}: transient_blocks={} retained_blocks={}",
        totals.transient_blocks, totals.retained_blocks
    )
    .expect("writing to a String cannot fail");
    writeln!(
        bytes,
        "{label}: transient_bytes={} retained_bytes={} peak_bytes={}",
        totals.transient_bytes, totals.retained_bytes, totals.peak_bytes
    )
    .expect("writing to a String cannot fail");
}

#[test]
fn corpus_scale_aggregate_allocations_are_pinned() {
    // Measure both presets first (each `measure` closes its sample windows before
    // returning, so the two are independent), then assert — subset sizes before
    // totals, so a vanished statement is reported as a coverage drift, not as an
    // opaque totals mismatch.
    let (ansi_subset, ansi_totals) = measure(Preset::Ansi);
    let (postgres_subset, postgres_totals) = measure(Preset::Postgres);

    // Subset-size pins (anti-vanishing): sqlglot identity (955), sqllogictest
    // statements (373), postgres regress-supported (32). A newly parseable statement
    // moves the `included` counts (a correctness gain — update the pin); a changed
    // `candidates` count means the vendored fixture itself drifted.
    assert_coverage(
        &ansi_subset,
        1360,
        866,
        &[
            ("sqlglot_identity", 955, 473),
            ("sqllogictest_statements", 373, 366),
            ("postgres_regress_supported", 32, 27),
        ],
    );
    // sqlglot_identity 514: the `-|-` range-adjacency line joins the Postgres-parseable
    // subset under the general operator surface (pg-operator-surface-regex-geometric-network).
    assert_coverage(
        &postgres_subset,
        1360,
        911,
        &[
            ("sqlglot_identity", 955, 514),
            ("sqllogictest_statements", 373, 366),
            ("postgres_regress_supported", 32, 31),
        ],
    );

    // Aggregate allocation pins (measured `dhat::HeapStats`, summed over the subset).
    let mut blocks = String::new();
    let mut bytes = String::new();
    record_totals("Ansi", &ansi_totals, &mut blocks, &mut bytes);
    record_totals("Postgres", &postgres_totals, &mut blocks, &mut bytes);

    insta::with_settings!({ prepend_module_to_snapshot => false }, {
        insta::assert_snapshot!("corpus_allocations__corpus_aggregate_blocks", blocks);
    });
    if cfg!(target_arch = "aarch64") {
        insta::with_settings!({ prepend_module_to_snapshot => false }, {
            insta::assert_snapshot!(
                "corpus_allocations__corpus_aggregate_bytes_aarch64",
                bytes
            );
        });
    }
}
