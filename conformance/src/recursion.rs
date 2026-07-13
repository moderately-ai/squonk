// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Public-API DoS-safety: deeply nested untrusted SQL is rejected cleanly rather
//! than overflowing the stack (ADR-0012).
//!
//! Black-box over the published entry points (`parse_with` / `parse_with`)
//! and the `Ansi` dialect — the surface a downstream consumer fronting untrusted
//! SQL actually uses. The from-scratch stress spike crashed an *unguarded* build at
//! ~721 nested parentheses / ~406 nested subqueries; here each vector is nested far
//! past both that crash depth and the default limit, and must come back as a clean
//! [`ParseErrorKind::RecursionLimitExceeded`]. If the guard regressed, these would
//! abort the test process with a stack overflow instead of failing an assertion.
//!
//! Each parse runs on an explicitly sized worker thread so the assertion is
//! independent of the ambient test-runner stack size: a generous 4 MiB proves the
//! guard trips (capping recursion at the default 128) well within it, and — for the
//! shipped release build — a 2 MiB stack (the size Rust gives a spawned thread by
//! default) proves the chosen default is genuinely safe on the smallest commonly
//! supported stack. Debug frames are ~10x larger (unoptimized), so the 2 MiB bound is
//! a release property; `default_limit_is_safe_on_a_2_mib_stack` asserts it there and
//! gives the guard a generous stack in debug (measuring the guard, not frame size).

use squonk::dialect::Ansi;
use squonk::error::ParseErrorKind;
use squonk::{ParseConfig, parse_with};

/// Far past both the empirical crash depth and the default limit.
const ADVERSARIAL_DEPTH: usize = 1500;

/// `SELECT ((( … 1 … )))` — expression-recursion vector.
fn nested_parens(n: usize) -> String {
    format!("SELECT {}1{}", "(".repeat(n), ")".repeat(n))
}

/// `((( … SELECT 1 … )))` — query-recursion vector (the single-counted, heaviest
/// per-level path).
fn nested_subqueries(n: usize) -> String {
    format!("{}SELECT 1{}", "(".repeat(n), ")".repeat(n))
}

/// `SELECT (SELECT ( … 1 … ))` — scalar subqueries, crossing both recursion points.
fn nested_scalar_subqueries(n: usize) -> String {
    format!("SELECT {}1{}", "(SELECT ".repeat(n), ")".repeat(n))
}

/// `SELECT * FROM (SELECT * FROM ( … t … ))` — derived-table vector.
fn nested_derived_tables(n: usize) -> String {
    format!(
        "SELECT * FROM {}t{}",
        "(SELECT * FROM ".repeat(n),
        ")".repeat(n)
    )
}

/// Run `body` on a worker thread of `stack_bytes`, propagating its panic — so a
/// failed assertion still fails the test, and only a genuine stack overflow aborts
/// the process (which is itself the regression these tests guard against).
fn on_stack(stack_bytes: usize, body: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(stack_bytes)
        .spawn(body)
        .expect("spawn worker thread")
        .join()
        .expect("worker thread parsed without overflowing the stack");
}

fn assert_clean_recursion_rejection(sql: String) {
    let err = parse_with(&sql, squonk::ParseConfig::new(Ansi))
        .expect_err("deeply nested input must be rejected, never accepted or crashed");
    assert_eq!(err.kind, ParseErrorKind::RecursionLimitExceeded);
}

#[test]
fn adversarial_nesting_rejects_cleanly_on_a_generous_stack() {
    // 4 MiB is comfortably above the ~2 MiB the default limit needs; every vector
    // must come back as a clean error rather than overflowing.
    on_stack(4 * 1024 * 1024, || {
        assert_clean_recursion_rejection(nested_parens(ADVERSARIAL_DEPTH));
        assert_clean_recursion_rejection(nested_subqueries(ADVERSARIAL_DEPTH));
        assert_clean_recursion_rejection(nested_scalar_subqueries(ADVERSARIAL_DEPTH));
        assert_clean_recursion_rejection(nested_derived_tables(ADVERSARIAL_DEPTH));
    });
}

#[test]
fn default_limit_is_safe_on_a_2_mib_stack() {
    // The headline guarantee: at the default limit the worst-case vectors trip the
    // guard *within* a 2 MiB stack — Rust's default spawned-thread size — so the
    // default protects untrusted callers without assuming a large main-thread stack.
    //
    // This is a property of the SHIPPED (release) build: an optimized 128-deep descent
    // needs ~200 KiB of stack (measured), leaving ~10x headroom under 2 MiB. An
    // unoptimized debug build's frames are ~10x larger, so 128-deep sits right at 2 MiB
    // there — a debug 2-MiB assertion would pin frame size rather than prove safety, and
    // would flake on any benign frame change. So the 2-MiB bound is asserted for the
    // shipped build; in debug the guard gets a generous stack and is still verified to
    // trip cleanly (the invariant that actually matters: reject, never overflow).
    let stack_bytes = if cfg!(debug_assertions) {
        8 * 1024 * 1024
    } else {
        2 * 1024 * 1024
    };
    on_stack(stack_bytes, || {
        assert_clean_recursion_rejection(nested_parens(ADVERSARIAL_DEPTH));
        assert_clean_recursion_rejection(nested_subqueries(ADVERSARIAL_DEPTH));
        assert_clean_recursion_rejection(nested_scalar_subqueries(ADVERSARIAL_DEPTH));
        assert_clean_recursion_rejection(nested_derived_tables(ADVERSARIAL_DEPTH));
    });
}

#[test]
fn a_raised_limit_still_bounds_recursion() {
    // Opting into deeper nesting still caps it: 1500 levels exceed even a doubled
    // limit of 256, so the guard stays in force when reconfigured. Run on an 8 MiB
    // stack with comfortable margin (256 levels is well under it even in this debug
    // build), since the point is that the *limit* is enforced, not how deep it sits.
    on_stack(8 * 1024 * 1024, || {
        let sql = nested_parens(ADVERSARIAL_DEPTH);
        let err = parse_with(&sql, ParseConfig::new(Ansi).recursion_limit(256))
            .expect_err("1500 levels exceed a raised limit of 256");
        assert_eq!(err.kind, ParseErrorKind::RecursionLimitExceeded);
    });
}

#[test]
fn ordinary_depth_still_parses() {
    // The guard must not clip legitimate queries: a handful of nesting levels parses
    // normally under the default limit.
    parse_with(&nested_parens(8), squonk::ParseConfig::new(Ansi)).expect("ordinary nesting parses");
    parse_with(&nested_subqueries(8), squonk::ParseConfig::new(Ansi))
        .expect("ordinary subquery nesting parses");
    parse_with(&nested_scalar_subqueries(8), squonk::ParseConfig::new(Ansi))
        .expect("ordinary scalar subqueries parse");
    parse_with(&nested_derived_tables(8), squonk::ParseConfig::new(Ansi))
        .expect("ordinary derived tables parse");
}
