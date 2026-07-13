// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Adversarial recursion-limit policy gate (ADR-0012 / ADR-0016).
//!
//! The robustness invariant: deeply nested untrusted SQL must never abort the
//! process. Our engine's recursion guard turns unbounded recursive descent into a
//! clean [`ParseErrorKind::RecursionLimitExceeded`] at the configured limit (the
//! guard itself, and its parser-side acceptance tests, live in
//! `crates/squonk/src/parser/`); this bench-side gate pins the *policy* from the
//! outside — through the public parse API and beside upstream `sqlparser` on the
//! identical inputs — so the adversarial-stress story has one place that records
//! "ours and theirs both graceful-reject, neither overflows".
//!
//! It deliberately pins the POLICY, never a raw overflow depth: the absolute depth
//! at which an *unguarded* recursive descent would smash the stack is stack-size and
//! build dependent (the spike measured ~721 nested parens on an 8 MB dev stack; it
//! would be far lower on a 1–2 MB thread), so it is no kind of stable gate value.
//! What is stable, and what these tests fix, is the contract: reject cleanly past the
//! configured limit, accept at a realistic safe depth, and do it for every guarded
//! nesting site. The reach over the limit is small on purpose — a handful past it,
//! never into the hundreds — so the gate is fast and can never itself approach a real
//! stack-overflow depth.

use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser as UpstreamParser;
use squonk::dialect::Postgres;
use squonk::error::ParseErrorKind;
use squonk::{DEFAULT_RECURSION_LIMIT, ParseConfig, parse_with};
use squonk_bench::adversarial::{nested_parens, nested_subqueries};

/// A safe depth far past anything legitimate (real SQL rarely nests beyond ~20) yet
/// well within the shipped default limit, so our parser must accept it.
const SAFE_DEPTH: usize = 64;

/// A tight configured limit for the trip tests, with a small over-reach above it.
const TIGHT_LIMIT: usize = 32;
const OVER_REACH: usize = 8;

/// Past upstream's default `recursive-protection` budget (~50) for either nesting
/// form, so upstream must reject — used only to confirm upstream graceful-rejects
/// (never overflows) on the same input, not to pin its exact budget.
const PAST_THEIRS_BUDGET: usize = 200;

fn ours_under_limit(sql: &str, limit: usize) -> Result<(), ParseErrorKind> {
    parse_with(sql, ParseConfig::new(Postgres).recursion_limit(limit))
        .map(|_| ())
        .map_err(|e| e.kind)
}

fn theirs_accepts(sql: &str) -> bool {
    UpstreamParser::parse_sql(&PostgreSqlDialect {}, sql).is_ok()
}

/// Run `body` on a thread with a stack generous enough for the (unoptimized) debug
/// recursion frames to reach the default limit, so this gate measures the guard
/// tripping rather than the ambient test-thread stack size. The shipped release build
/// reaches the limit in ~200 KiB — this larger stack is a debug-frame concession, not
/// a shipped requirement (the smallest-stack *safety* bound lives in the conformance
/// crate's `recursion` gate). A genuine overflow still aborts; a failed assertion
/// propagates unchanged.
fn on_generous_stack(body: impl FnOnce() + Send + 'static) {
    match std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(body)
        .expect("spawn worker thread")
        .join()
    {
        Ok(()) => {}
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[test]
fn ours_rejects_cleanly_past_the_configured_limit() {
    // The configurable knob is in force: the very same depth that clears a generous
    // limit trips a tight one, with the programmatic recursion kind (not a generic
    // syntax error and — load-bearing — not a crash).
    let depth = TIGHT_LIMIT + OVER_REACH;
    for (label, sql) in [
        ("nested_parens", nested_parens(depth)),
        ("nested_subqueries", nested_subqueries(depth)),
    ] {
        assert_eq!(
            ours_under_limit(&sql, TIGHT_LIMIT),
            Err(ParseErrorKind::RecursionLimitExceeded),
            "{label}: depth {depth} must reject cleanly under a limit of {TIGHT_LIMIT}"
        );
        assert!(
            ours_under_limit(&sql, depth * 4).is_ok(),
            "{label}: depth {depth} must parse under a generous limit (proving the knob, not a constant)"
        );
    }
}

#[test]
fn ours_rejects_cleanly_past_the_shipped_default_limit() {
    // Not just a tight test limit: the DEFAULT the argument-free `parse_with` ships
    // also rejects past its bound, so untrusted SQL hitting the default API is safe.
    // Run on a generous stack so the guard (at 128) — not the ambient debug test
    // thread — is what stops the descent; the shipped release build reaches the limit
    // in ~200 KiB, but unoptimized debug frames need ~2 MiB for 128 levels.
    on_generous_stack(|| {
        let depth = DEFAULT_RECURSION_LIMIT + 16;
        for (label, sql) in [
            ("nested_parens", nested_parens(depth)),
            ("nested_subqueries", nested_subqueries(depth)),
        ] {
            let err = parse_with(&sql, squonk::ParseConfig::new(Postgres)).expect_err(&format!(
                "{label}: depth {depth} must reject under the default limit"
            ));
            assert_eq!(
                err.kind,
                ParseErrorKind::RecursionLimitExceeded,
                "{label}: the default-limit rejection carries the recursion kind"
            );
        }
    });
}

#[test]
fn ours_accepts_a_safe_depth_under_the_default() {
    // The guard rejects DoS input without clipping ordinary (even pathological but
    // bounded) queries: both guarded nesting forms parse at the safe depth.
    for (label, sql) in [
        ("nested_parens", nested_parens(SAFE_DEPTH)),
        ("nested_subqueries", nested_subqueries(SAFE_DEPTH)),
    ] {
        assert!(
            parse_with(&sql, squonk::ParseConfig::new(Postgres)).is_ok(),
            "{label}: depth {SAFE_DEPTH} is safe and must parse under the default limit"
        );
    }
}

#[test]
fn upstream_graceful_rejects_the_same_deep_input_without_overflowing() {
    // The recorded ours-vs-theirs robustness contrast: upstream `sqlparser`, with its
    // default `recursive-protection`, ALSO rejects deeply nested input rather than
    // overflowing — the test reaching this assertion at all is the proof it did not
    // abort the process. (Without our recursion guard OUR parser would overflow here; with
    // it, both graceful-reject.)
    for (label, sql) in [
        ("nested_parens", nested_parens(PAST_THEIRS_BUDGET)),
        ("nested_subqueries", nested_subqueries(PAST_THEIRS_BUDGET)),
    ] {
        assert!(
            !theirs_accepts(&sql),
            "{label}: upstream must reject depth {PAST_THEIRS_BUDGET} (its recursive-protection budget is ~50)"
        );
    }
}

#[test]
fn ours_has_more_headroom_than_upstream() {
    // The relative-policy contrast: our default limit (128) is more generous than
    // upstream's protection budget (~50), so there is a band — the safe depth sits in
    // it — where OURS accepts and THEIRS rejects. Both are still safe (neither
    // overflows); ours simply admits deeper legitimate nesting before guarding.
    for (label, sql) in [
        ("nested_parens", nested_parens(SAFE_DEPTH)),
        ("nested_subqueries", nested_subqueries(SAFE_DEPTH)),
    ] {
        assert!(
            parse_with(&sql, squonk::ParseConfig::new(Postgres)).is_ok(),
            "{label}: ours accepts the safe depth {SAFE_DEPTH}"
        );
        assert!(
            !theirs_accepts(&sql),
            "{label}: theirs rejects the safe depth {SAFE_DEPTH} (past its ~50 budget)"
        );
    }
}
