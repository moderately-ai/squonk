// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Whole-tree serde round-trip through the parse root (`serde` feature): parse ->
//! serialize -> deserialize -> render byte-identical + structurally equal, plus the
//! parse-root deserialization depth bound.
#![cfg(feature = "serde")]

use serde::Deserialize;
use squonk::{Parsed, parse};

#[test]
fn parse_serialize_deserialize_renders_byte_identical() {
    let sql = "SELECT a + 1 AS n, b FROM t WHERE a IN (1, 2) ORDER BY b";
    let parsed = parse(sql).expect("parses");
    let before = parsed.to_sql();

    let json = serde_json::to_string(&parsed).expect("serializes");
    let restored: Parsed = serde_json::from_str(&json).expect("deserializes");

    // The headline requirement: rendering the round-tripped tree is byte-identical.
    assert_eq!(
        restored.to_sql(),
        before,
        "render must be byte-identical after a serde round-trip"
    );
    // Structural equality holds too: the numeric symbols survive verbatim and the
    // resolver is rebuilt to give them the same text (re-intern on load).
    assert_eq!(restored.statements(), parsed.statements());
    assert_eq!(restored.source(), parsed.source());
}

#[test]
fn round_trip_preserves_case_sensitive_identifier_text() {
    // Mixed-case + keyword-spelled identifiers exercise the resolver's dynamic string
    // table: they must resolve to the exact same text after re-interning on load.
    let sql = r#"SELECT "MixedCase", other FROM "Tbl""#;
    let parsed = parse(sql).expect("parses");
    let json = serde_json::to_string(&parsed).expect("serializes");
    let restored: Parsed = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(restored.to_sql(), parsed.to_sql());
}

#[test]
fn parsed_deserialize_bounds_recursion_depth() {
    // Left-associative `+` builds a ~200-deep BinaryOp spine. Chained binary
    // operators are parsed by iteration, not recursion, so this clears the parser's
    // recursion guard yet is deeper than the deserialize depth cap — the exact tree a
    // hostile serialized payload could carry.
    let mut sql = String::from("SELECT 1");
    for _ in 0..200 {
        sql.push_str(" + 1");
    }
    let parsed = parse(&sql).expect("addition chain parses");
    let json = serde_json::to_string(&parsed).expect("serializes");

    // Isolate our guard from serde_json's own limit: the parse root routes its
    // statement tree through the depth guard, so a >cap tree is rejected cleanly
    // rather than built (and later overflowing the stack on drop/render/visit).
    let mut de = serde_json::Deserializer::from_str(&json);
    de.disable_recursion_limit();
    let rejected: Result<Parsed, _> = Parsed::deserialize(&mut de);
    assert!(
        rejected.is_err(),
        "an over-deep statement tree must be rejected on deserialize, not built"
    );

    // serde_json's built-in nesting limit rejects it on the ordinary path too.
    assert!(serde_json::from_str::<Parsed>(&json).is_err());

    // Opt-up: the *same* legitimately deep document loads once the caller raises the
    // budget, and round-trips byte-identically. serde_json's own nesting limit is
    // disabled so this exercises our threaded budget alone.
    let mut de = serde_json::Deserializer::from_str(&json);
    de.disable_recursion_limit();
    let restored: Parsed = Parsed::deserialize_with_depth(&mut de, 4096)
        .expect("the deep tree loads once the deserialize cap is raised");
    assert_eq!(
        restored.to_sql(),
        parsed.to_sql(),
        "the opt-up deserialize must render byte-identically"
    );
}

/// Depth-first, mutate the first serialized `Span` (`{"start","end"}` object) found;
/// returns whether one was hit. Spans are the only two-field `start`/`end` objects a
/// `Parsed` document carries.
fn corrupt_first_span(
    value: &mut serde_json::Value,
    mutate: &mut dyn FnMut(&mut serde_json::Map<String, serde_json::Value>),
) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            if map.len() == 2 && map.contains_key("start") && map.contains_key("end") {
                mutate(map);
                return true;
            }
            map.values_mut().any(|v| corrupt_first_span(v, mutate))
        }
        serde_json::Value::Array(items) => items.iter_mut().any(|v| corrupt_first_span(v, mutate)),
        _ => false,
    }
}

/// Depth-first, set the first raw `Symbol` field (`"sym"`, serialized as a bare
/// number) to `to`; returns whether one was hit.
fn corrupt_first_symbol(value: &mut serde_json::Value, to: u32) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(sym) = map.get_mut("sym") {
                *sym = serde_json::json!(to);
                return true;
            }
            map.values_mut().any(|v| corrupt_first_symbol(v, to))
        }
        serde_json::Value::Array(items) => items.iter_mut().any(|v| corrupt_first_symbol(v, to)),
        _ => false,
    }
}

#[test]
fn deserialize_rejects_a_span_past_the_source_bounds() {
    // `SELECT abc` is 10 bytes; pushing a span's end far past that is content the
    // parser would never emit and the field-by-field rebuild would otherwise admit.
    let parsed = parse("SELECT abc").expect("parses");
    let mut doc = serde_json::to_value(&parsed).expect("serializes");
    assert!(
        corrupt_first_span(&mut doc, &mut |span| span["end"] = serde_json::json!(9999)),
        "the document must carry a span to corrupt",
    );

    let err = serde_json::from_value::<Parsed>(doc).expect_err("out-of-bounds span rejected");
    let message = err.to_string();
    assert!(
        message.contains("span") && message.contains("bounds"),
        "the error must name the out-of-bounds span: {message}",
    );
}

#[test]
fn deserialize_rejects_a_span_with_start_after_end() {
    // Both offsets are within the 10-byte source, isolating the `start <= end`
    // invariant that `Span::new` normally enforces but the rebuild bypasses.
    let parsed = parse("SELECT abc").expect("parses");
    let mut doc = serde_json::to_value(&parsed).expect("serializes");
    assert!(
        corrupt_first_span(&mut doc, &mut |span| {
            span["start"] = serde_json::json!(9);
            span["end"] = serde_json::json!(1);
        }),
        "the document must carry a span to corrupt",
    );

    let err = serde_json::from_value::<Parsed>(doc).expect_err("start>end span rejected");
    assert!(
        err.to_string().contains("span"),
        "the error must name the malformed span: {err}",
    );
}

#[test]
fn deserialize_rejects_a_symbol_past_the_table() {
    // A symbol index beyond the rebuilt table is the real clean-input-in/panic-out
    // vector: unresolved, it would panic on the first canonical render.
    let parsed = parse("SELECT abc").expect("parses");
    let mut doc = serde_json::to_value(&parsed).expect("serializes");
    assert!(
        corrupt_first_symbol(&mut doc, 9999),
        "the document must carry a raw symbol to corrupt",
    );

    let err = serde_json::from_value::<Parsed>(doc).expect_err("out-of-table symbol rejected");
    assert!(
        err.to_string().contains("symbol"),
        "the error must name the unresolvable symbol: {err}",
    );
}

#[test]
fn deserialize_rejects_a_duplicate_symbol_table_entry() {
    // The interner dedupes on re-intern, so a duplicated entry collapses two slots and
    // shifts every later symbol's resolution — silent misresolution unless rejected.
    let parsed = parse("SELECT alpha, beta").expect("parses");
    let mut doc = serde_json::to_value(&parsed).expect("serializes");
    assert_eq!(doc["symbols"], serde_json::json!(["alpha", "beta"]));
    doc["symbols"] = serde_json::json!(["alpha", "alpha"]);

    let err = serde_json::from_value::<Parsed>(doc).expect_err("duplicate table entry rejected");
    assert!(
        err.to_string().contains("duplicate"),
        "the error must name the duplicate table entry: {err}",
    );
}

#[test]
fn deserialize_accepts_an_uncorrupted_document() {
    // The positive control: validation must not false-positive on a genuine document
    // spanning literals, identifiers, functions, and an IN-list, so the round-trip is
    // untouched by the new checks.
    let parsed = parse("SELECT a + 1 AS n, count(b) FROM t WHERE a IN (1, 2)").expect("parses");
    let doc = serde_json::to_value(&parsed).expect("serializes");
    let restored: Parsed = serde_json::from_value(doc).expect("a valid document passes validation");
    assert_eq!(restored.to_sql(), parsed.to_sql());
}
