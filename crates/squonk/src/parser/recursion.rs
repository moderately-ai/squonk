// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Recursion-depth guard acceptance tests (DoS-safety).
//!
//! Proves the parser bounds recursive descent: deeply nested input yields a clean
//! [`ParseErrorKind::RecursionLimitExceeded`](crate::error::ParseErrorKind::RecursionLimitExceeded)
//! error at the configured limit instead of overflowing the stack, the limit is
//! configurable, and every audited self-recursion site is guarded — nested
//! parentheses (the Pratt core), the subquery forms (the query grammar), the
//! parenthesized join factor (the `FROM` grammar), and nested statements
//! (`EXPLAIN`/`COPY`). The from-scratch stress spike found an unguarded build
//! overflows the stack at ~721 nested parentheses / ~406 nested subqueries; these
//! tests stay fast by configuring a *low* limit and nesting just past it, never to
//! the hundreds that crash.

use crate::ast::dialect::FeatureSet;
use crate::ast::{NoExt, Statement};
use crate::error::{ParseErrorKind, ParseResult};
use crate::tokenizer::tokenize;

use super::{FeatureDialect, ParseConfig, Parser, TestDialect, parse_with};

/// `SELECT ((( … 1 … )))` with `n` nested parentheses — expression recursion
/// through [`parse_expr_bp`](Parser::parse_expr_bp); one guard entry per level.
fn nested_parens(n: usize) -> String {
    format!("SELECT {}1{}", "(".repeat(n), ")".repeat(n))
}

/// `((( … SELECT 1 … )))` with `n` nested parenthesized queries — query recursion
/// through [`parse_query`](Parser::parse_query) via the set-operation operand path;
/// one guard entry per level.
fn nested_paren_queries(n: usize) -> String {
    format!("{}SELECT 1{}", "(".repeat(n), ")".repeat(n))
}

/// `SELECT (SELECT ( … 1 … ))` with `n` nested scalar subqueries, which cross
/// *both* the expression and query recursion points (two guard entries per level).
fn nested_scalar_subqueries(n: usize) -> String {
    format!("SELECT {}1{}", "(SELECT ".repeat(n), ")".repeat(n))
}

/// `SELECT * FROM (SELECT * FROM ( … t … ))` with `n` nested derived tables —
/// the parenthesized-table-factor recursion crossing into [`parse_query`].
fn nested_derived_tables(n: usize) -> String {
    format!(
        "SELECT * FROM {}t{}",
        "(SELECT * FROM ".repeat(n),
        ")".repeat(n)
    )
}

/// `SELECT * FROM ((( t JOIN u ON TRUE )))` with `n` nested parenthesized join
/// factors — the one `FROM`-clause recursion that is *not* a query, so it relies on
/// the guard in `parse_parenthesized_table_factor` rather than `parse_query`.
fn nested_join_parens(n: usize) -> String {
    format!(
        "SELECT * FROM {}t JOIN u ON TRUE{}",
        "(".repeat(n),
        ")".repeat(n)
    )
}

/// `EXPLAIN EXPLAIN … SELECT 1` with `n` nested statements — the statement-level
/// recursion `EXPLAIN`/`COPY` drive through [`parse_statement`](Parser::parse_statement).
fn nested_explains(n: usize) -> String {
    format!("{}SELECT 1", "EXPLAIN ".repeat(n))
}

/// A low limit for the trip tests: above the handful of entries the non-nested
/// scaffolding (`parse_statement` + `parse_query` + the first `parse_expr_bp`)
/// already consumes, so a modest nesting clears it without nesting into the
/// hundreds.
const LOW_LIMIT: usize = 16;

fn limit(n: usize) -> ParseConfig {
    ParseConfig::default().recursion_limit(n)
}

#[test]
fn nested_parentheses_past_the_limit_reject_cleanly() {
    let sql = nested_parens(LOW_LIMIT + 8);
    let err = parse_with(&sql, limit(LOW_LIMIT).dialect(TestDialect))
        .expect_err("parentheses nested past the limit must be rejected, not crash");
    assert_eq!(err.kind, ParseErrorKind::RecursionLimitExceeded);
    // The error carries a real source span (the offending location), not the
    // synthetic sentinel.
    assert!(
        !err.span.is_synthetic(),
        "recursion error keeps a real span"
    );
    assert!(err.span.end() <= sql.len() as u32);
}

#[test]
fn nested_subqueries_past_the_limit_reject_cleanly() {
    // The acceptance's "subqueries" vector. Nested parenthesized queries are the
    // single-counted query-recursion path (one `parse_query` per level), so this is
    // the form that most directly exercises the query-grammar guard. Kept for the
    // module's two headline vectors (parentheses + subqueries) even though
    // `every_audited_recursion_site_rejects_past_the_limit` below also drives this
    // exact vector.
    let sql = nested_paren_queries(LOW_LIMIT + 8);
    let err = parse_with(&sql, limit(LOW_LIMIT).dialect(TestDialect))
        .expect_err("subqueries nested past the limit must be rejected, not crash");
    assert_eq!(err.kind, ParseErrorKind::RecursionLimitExceeded);
}

#[test]
fn high_but_safe_nesting_is_accepted_at_the_default_limit() {
    // 64 levels is far past anything legitimate yet well within the default 128, so
    // both the parenthesis and subquery vectors must parse — the guard rejects DoS
    // input without clipping ordinary (even pathological-but-bounded) queries.
    parse_with(&nested_parens(64), crate::ParseConfig::new(TestDialect))
        .expect("64 nested parens parse under the default");
    parse_with(
        &nested_paren_queries(64),
        crate::ParseConfig::new(TestDialect),
    )
    .expect("64 nested subqueries parse under the default");
}

#[test]
fn the_recursion_limit_is_configurable() {
    // The very same input is rejected under a tight limit and accepted under a
    // generous one — proving the knob, not just a hard-coded constant, is in force.
    let sql = nested_parens(40);
    let err = parse_with(&sql, limit(20).dialect(TestDialect))
        .expect_err("40 nested parens exceed a limit of 20");
    assert_eq!(err.kind, ParseErrorKind::RecursionLimitExceeded);
    parse_with(&sql, limit(200).dialect(TestDialect))
        .expect("40 nested parens fit comfortably under a limit of 200");
}

#[test]
fn the_limit_is_configurable_through_the_engine_builder() {
    // The underlying parser/engine option (`Parser::recursion_limit`) that the
    // public `ParseConfig` wraps, exercised directly.
    let sql = nested_parens(40);
    let tokens = tokenize(&sql).expect("the input lexes cleanly");
    let mut parser = Parser::new(&sql, &tokens, TestDialect).recursion_limit(20);
    let err = parser
        .parse_next_statement()
        .expect_err("the engine option bounds depth just like ParseConfig");
    assert_eq!(err.kind, ParseErrorKind::RecursionLimitExceeded);
}

#[test]
fn every_audited_recursion_site_rejects_past_the_limit() {
    // Each genuine self-recursion entry is guarded: nesting any of
    // them past the limit must produce the clean error, never a stack overflow. A
    // gap here (a site left unguarded) would crash the process instead of failing
    // this assertion.
    let deep = LOW_LIMIT + 8;
    let vectors = [
        ("nested parentheses (parse_expr_bp)", nested_parens(deep)),
        (
            "nested parenthesized queries (parse_query)",
            nested_paren_queries(deep),
        ),
        (
            "nested scalar subqueries (expr + query)",
            nested_scalar_subqueries(deep),
        ),
        (
            "nested derived tables (parse_parenthesized_table_factor + query)",
            nested_derived_tables(deep),
        ),
        (
            "nested join factors (parse_parenthesized_table_factor)",
            nested_join_parens(deep),
        ),
        (
            "nested EXPLAIN statements (parse_statement)",
            nested_explains(deep),
        ),
    ];
    for (label, sql) in vectors {
        let err = parse_with(&sql, limit(LOW_LIMIT).dialect(TestDialect))
            .err()
            .unwrap_or_else(|| panic!("{label}: expected a clean rejection, but the input parsed"));
        assert_eq!(err.kind, ParseErrorKind::RecursionLimitExceeded, "{label}");
    }
}

/// `CREATE TRIGGER … BEGIN SELECT (SELECT ( … 1 … )); END` with `n` nested scalar
/// subqueries inside the body statement. The trigger body reuses the recursion-guarded
/// [`parse_statement`](Parser::parse_statement), so it introduces no new self-recursion
/// site and needs no separate budget — a nested body statement is bounded exactly like
/// a top-level one.
fn nested_trigger_body(n: usize) -> String {
    format!(
        "CREATE TRIGGER trg AFTER INSERT ON t BEGIN SELECT {}1{}; END",
        "(SELECT ".repeat(n),
        ")".repeat(n),
    )
}

#[test]
fn nested_trigger_body_past_the_limit_rejects_cleanly() {
    // The trigger body is a statement list routed through the shared guard (ADR-0012),
    // so a body statement nested past the limit rejects cleanly rather than crashing —
    // the ticket's DoS-safety check that the body budget does not escape the guard.
    // Uses the SQLite preset because `CREATE TRIGGER` is gated to it.
    use crate::dialect::Sqlite;
    let sql = nested_trigger_body(LOW_LIMIT + 8);
    let err = parse_with(&sql, limit(LOW_LIMIT).dialect(Sqlite))
        .expect_err("a trigger body nested past the limit must be rejected, not crash");
    assert_eq!(err.kind, ParseErrorKind::RecursionLimitExceeded);
    // A shallow trigger body still parses comfortably under the default limit.
    parse_with(
        "CREATE TRIGGER trg AFTER INSERT ON t BEGIN SELECT 1; END",
        crate::ParseConfig::new(Sqlite),
    )
    .expect("a shallow trigger body parses under the default limit");
}

#[test]
fn sibling_subtrees_do_not_accumulate_depth() {
    // The guard is RAII: depth is restored when each guarded production returns, so
    // many *siblings* at a shallow depth never trip the limit even though their
    // total count dwarfs it. A guard that leaked on the success path (forgot to
    // decrement) would instead trip after a few items. Fifty shallow groups under a
    // limit of 8 — each peaks at depth ~4 — must all parse.
    let mut sql = String::from("SELECT ");
    for i in 0..50 {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str("(1)");
    }
    parse_with(&sql, limit(8).dialect(TestDialect))
        .expect("independent sibling groups must not accumulate recursion depth");
}

#[test]
fn frame_growth_trips_the_drift_sentinel_before_the_canary() {
    // Stack-frame drift sentinel for the hot recursive expression frame
    // (`parse_expr_bp_inner` + `parse_prefix` + `parse_grouped`; see primary.rs's
    // STACK-FRAME RULE). Measured 2026-07-08 at the prefix-router split
    // (RUST_MIN_STACK bisection of `high_but_safe_nesting_is_accepted_at_the_default_limit`,
    // debug build): both 64-deep vectors need ~1.30 MiB, i.e. ~20.3 KB per nesting
    // level including the non-nested scaffolding — down from ~1.94 MiB (~31.1 KB per
    // level) before the split, when the canary sat ~60 KB from its 2 MiB cliff.
    // This thread's 1.6 MB stack sits ~300 KB above the measured need and ~485 KB
    // below the canary's 2 MiB default, so per-level frame growth (an arm body
    // re-inlined into the hot frames, a fat new local) SIGABRTs *this* named test
    // first, while the canary — which is NEVER weakened from 64 levels / 2 MiB —
    // still passes and pinpoints the regression as drift rather than exhaustion.
    let sentinel = std::thread::Builder::new()
        .name("expr-frame-drift-sentinel".into())
        .stack_size(1_600_000)
        .spawn(|| {
            parse_with(&nested_parens(64), crate::ParseConfig::new(TestDialect))
                .expect("64 nested parens parse within the 1.6 MB sentinel stack");
            parse_with(
                &nested_paren_queries(64),
                crate::ParseConfig::new(TestDialect),
            )
            .expect("64 nested subqueries parse within the 1.6 MB sentinel stack");
        })
        .expect("the sentinel thread spawns");
    sentinel
        .join()
        .expect("the sentinel thread completes cleanly");
}

// --- Compound-statement body nesting (a distinct recursion axis) ---------------

/// The MySQL feature preset (`compound_statements` on) as a data-only test dialect,
/// so the body dispatcher's grammar is reachable.
const COMPOUND_DIALECT: FeatureDialect = FeatureDialect {
    features: &FeatureSet::MYSQL,
};

/// `BEGIN BEGIN … SELECT 1; … END; END` with `n` nested compound blocks — the body
/// dispatcher's own self-recursion axis. Each block re-enters the recursion guard
/// (a DIFFERENT axis from the expression hot frames), so a block nested past the
/// limit rejects cleanly rather than crashing.
fn nested_compound(n: usize) -> String {
    let mut sql = "BEGIN ".repeat(n);
    sql.push_str("SELECT 1;");
    sql.push_str(&" END;".repeat(n - 1));
    sql.push_str(" END");
    sql
}

/// Drive the `pub(super)` body dispatcher directly (the routine/trigger wrappers'
/// seam), at the given recursion limit.
fn parse_nested_compound(sql: &str, limit: usize) -> ParseResult<Statement<NoExt>> {
    let tokens = tokenize(sql)?;
    let mut parser = Parser::new(sql, &tokens, COMPOUND_DIALECT).recursion_limit(limit);
    parser.parse_body_statement()
}

#[test]
fn nested_compound_body_past_the_limit_rejects_cleanly() {
    // The compound-block body is a statement list routed through the body dispatcher's
    // own recursion guard (ADR-0012), so a block nested past the limit rejects cleanly
    // rather than overflowing the stack — the compound-nesting canary the spike calls
    // for. The body-nesting axis is distinct from the expression hot frames, and the
    // `high_but_safe_nesting` expression canary must stay green independently.
    let sql = nested_compound(LOW_LIMIT + 8);
    let err = parse_nested_compound(&sql, LOW_LIMIT)
        .expect_err("a compound block nested past the limit must be rejected, not crash");
    assert_eq!(err.kind, ParseErrorKind::RecursionLimitExceeded);
    // A shallow block still parses comfortably under a generous limit.
    parse_nested_compound(&nested_compound(4), 200).expect("a shallow block parses");
}

#[test]
fn compound_body_nesting_stays_within_the_frame_budget() {
    // Per-level frame budget for the cold body dispatcher, mirroring the expression
    // drift sentinel above: the `#[inline(never)]` body frames sit off the expression
    // hot path, so 64 nested blocks fit a bounded 1.6 MB stack. A body frame
    // re-inlined into the expression frames (inflating their per-level budget) would
    // SIGABRT this test first, pinpointing the regression as body-frame drift.
    let sentinel = std::thread::Builder::new()
        .name("compound-frame-budget-sentinel".into())
        .stack_size(1_600_000)
        .spawn(|| {
            parse_nested_compound(&nested_compound(64), 128)
                .expect("64 nested compound blocks parse within the 1.6 MB sentinel stack");
        })
        .expect("the sentinel thread spawns");
    sentinel
        .join()
        .expect("the sentinel thread completes cleanly");
}
