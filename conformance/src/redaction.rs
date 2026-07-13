// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Redacted-render fingerprint stability (ADR-0010/0015).
//!
//! Redacted rendering turns a statement into a stable, PII-free query fingerprint:
//! masked *content* collapses while query *shape* survives (see
//! [`RenderMode::Redacted`](squonk_ast::render::RenderMode) for the precise
//! contract). These end-to-end tests parse real SQL under the M1 superset
//! (PostgreSQL) and assert the relation directly — inputs differing only in masked
//! content share one fingerprint, inputs differing in shape do not — and pin the
//! parseability determination.

use squonk::dialect::{Ansi, Postgres};
use squonk::{Parsed, parse_with};
use squonk_ast::render::RenderMode;

/// Parse `sql` under PostgreSQL (the M1 superset) and return its redacted render.
fn fingerprint(sql: &str) -> String {
    let parsed = parse_with(sql, squonk::ParseConfig::new(Postgres))
        .unwrap_or_else(|err| panic!("expected {sql:?} to parse under PostgreSQL: {err:?}"));
    redacted(&parsed)
}

/// Render every statement of `parsed` in [`RenderMode::Redacted`], `; `-joined.
fn redacted(parsed: &Parsed) -> String {
    crate::render_statements(parsed, RenderMode::Redacted)
}

#[test]
fn fingerprint_collapses_content_only_differences() {
    // Each pair differs only along a dimension the mask erases, so both sides must
    // share one fingerprint (ADR-0010/0015).

    // Identifier spelling.
    assert_eq!(
        fingerprint("SELECT a FROM t"),
        fingerprint("SELECT b FROM u")
    );
    // Literal value (numeric and string).
    assert_eq!(fingerprint("SELECT 1"), fingerprint("SELECT 999"));
    assert_eq!(
        fingerprint("SELECT 'x'"),
        fingerprint("SELECT 'a longer secret value'"),
    );
    // Keyword casing (canonical rendering upper-cases keywords).
    assert_eq!(fingerprint("select a"), fingerprint("SELECT a"));

    // All dimensions at once, with the shared fingerprint pinned exactly.
    let lower_named = fingerprint("select a FROM t WHERE a = 1");
    let upper_named = fingerprint("SELECT x FROM y WHERE x = 42");
    assert_eq!(lower_named, upper_named);
    assert_eq!(lower_named, "SELECT id FROM id WHERE id = ?");
}

#[test]
fn fingerprint_masks_temporal_literal_values() {
    // Typed temporal literals mask to `?` like any other literal (ADR-0010/0015), so
    // neither the value string nor the type/qualifier reaches the fingerprint: two
    // dates collapse, and a `DATE - INTERVAL '90' DAY` arithmetic collapses with a
    // different `DATE - INTERVAL '5' YEAR` arithmetic.
    assert_eq!(
        fingerprint("SELECT DATE '2020-01-01'"),
        fingerprint("SELECT DATE '1999-12-31'"),
    );
    assert_eq!(fingerprint("SELECT DATE '2020-01-01'"), "SELECT ?");
    assert_eq!(
        fingerprint("SELECT d <= DATE '1998-12-01' - INTERVAL '90' DAY"),
        fingerprint("SELECT d <= DATE '2000-01-01' - INTERVAL '5' YEAR"),
    );
    assert_eq!(
        fingerprint("SELECT d <= DATE '1998-12-01' - INTERVAL '90' DAY"),
        "SELECT id <= ? - ?",
    );
}

#[test]
fn fingerprint_masks_pg_special_literal_values() {
    // Bit-string and Unicode-escape constants mask to `?` like any other literal, so
    // neither the digits nor the Unicode escapes (or the UESCAPE override) reach the
    // fingerprint (ADR-0010/0015).
    for sql in [
        "SELECT B'1010'",
        "SELECT X'1FF'",
        r"SELECT U&'\0041'",
        "SELECT U&'d!0061ta' UESCAPE '!'",
    ] {
        assert_eq!(fingerprint(sql), "SELECT ?", "masking {sql:?}");
    }

    // `N'secret'` is NOT a national string under PostgreSQL (the preset leaves
    // `national_strings` off, pg-national-strings-lexing-divergence): it reads as the
    // generalized typed literal `N '…'`, so the type name masks as an identifier and the
    // value as `?` — the same fingerprint as any other prefix-typed literal.
    assert_eq!(fingerprint("SELECT N'secret'"), "SELECT id ?");
    assert_eq!(fingerprint("SELECT float8 'NaN'"), "SELECT id ?");

    // The genuine national-string constant masks whole under a dialect that arms it
    // (MySQL), keeping the value out of the fingerprint.
    let mysql_national = parse_with(
        "SELECT N'secret'",
        squonk::ParseConfig::new(squonk::dialect::MySql),
    )
    .expect("MySQL lexes the national string");
    assert_eq!(redacted(&mysql_national), "SELECT ?");

    // Two bit strings with different radix and digits collapse to one fingerprint.
    assert_eq!(fingerprint("SELECT B'1010'"), fingerprint("SELECT X'ff'"));
    // A bit string in a predicate keeps the query shape but masks the constant.
    assert_eq!(
        fingerprint("SELECT * FROM t WHERE flags = B'1010'"),
        "SELECT * FROM id WHERE id = ?",
    );
}

#[test]
fn fingerprint_preserves_query_shape() {
    // Differences in shape the fingerprint must keep, so distinct queries do not
    // collide (ADR-0010/0015).

    // An added clause.
    assert_ne!(
        fingerprint("SELECT a FROM t"),
        fingerprint("SELECT a FROM t WHERE a = 1"),
    );
    // Projection arity.
    assert_ne!(fingerprint("SELECT a"), fingerprint("SELECT a, b"));
    // Qualified-name arity.
    assert_ne!(fingerprint("SELECT a"), fingerprint("SELECT t.a"));
    // Operator.
    assert_ne!(fingerprint("SELECT a = b"), fingerprint("SELECT a < b"));
    // Logical operator.
    assert_ne!(fingerprint("SELECT a AND b"), fingerprint("SELECT a OR b"));
    // A shape-bearing keyword.
    assert_ne!(
        fingerprint("SELECT a FROM t"),
        fingerprint("SELECT DISTINCT a FROM t"),
    );
}

#[test]
fn redacted_output_is_a_fingerprint_not_guaranteed_reparseable() {
    // A masked literal renders as `?`, the anonymous-parameter sigil, which neither
    // ANSI nor PostgreSQL enables lexically (`ParameterSyntax::{ANSI, POSTGRES}`
    // both leave `anonymous_question` off). So a redacted statement carrying any
    // literal does not re-parse under the dialect it came from: redaction is a
    // fingerprint, not a round-trip (only Canonical round-trips, ADR-0010).
    let with_literal = fingerprint("SELECT 1");
    assert_eq!(with_literal, "SELECT ?");
    assert!(
        parse_with(with_literal.as_str(), squonk::ParseConfig::new(Postgres)).is_err(),
        "redacted `?` must not re-parse under PostgreSQL",
    );
    assert!(
        parse_with(with_literal.as_str(), squonk::ParseConfig::new(Ansi)).is_err(),
        "redacted `?` must not re-parse under ANSI",
    );

    // The `id` mask is itself a valid bare identifier, so an identifier-only
    // redaction happens to re-parse — but that is incidental, not a guarantee, and
    // the re-parse carries none of the original names.
    let names_only = fingerprint("SELECT a FROM t");
    assert_eq!(names_only, "SELECT id FROM id");
    assert!(
        parse_with(names_only.as_str(), squonk::ParseConfig::new(Postgres)).is_ok(),
        "the identifier-only redaction is incidentally parseable",
    );
}
