// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Demonstrations of the cross-tree safety contract documented on `Parsed`
//! (`define-cross-tree-node-graft-safety-contract`, ADR-0003's accepted-and-unchecked
//! record).
//!
//! `Parsed` cannot be constructed from parts publicly, so the two ways to pair
//! statements with a foreign `(source, resolver)` are splicing serde documents and
//! rendering a *detached* node against another root's resolver in the same process.
//!
//! Since `serde-untrusted-document-validation-and-depth-opt-up`, deserialize
//! **validates** a spliced document: an out-of-table symbol or an out-of-bounds
//! literal span is rejected at load. So those failure modes moved *earlier* — the
//! contract's fail-loud grading for them now fires at deserialize rather than on
//! render, which strengthens it. The render path stays deliberately graded (total for
//! a span it cannot slice, panic for an out-of-table symbol, silently wrong for an
//! in-range foreign one), but for content that came through serde that grading is now
//! reachable only in the two cases validation cannot judge: an in-range foreign symbol
//! (indistinguishable from intent) and a `SYNTHETIC` literal span (exempt by design,
//! since a rewrite-synthesized node has no source). Every other render-time grading is
//! reachable only by same-process misuse that never passes through serde — a detached
//! node rendered against a foreign resolver, shown here on the debug path.
#![cfg(feature = "serde")]

use squonk::ast::render::RenderExt as _;
use squonk::{Parsed, parse};

/// Serialize both roots and splice `donor`'s statements into `host`'s document,
/// returning the hybrid document *before* deserialization — the misuse vector every
/// serde test here shares.
fn splice_doc(host: &str, donor: &str) -> serde_json::Value {
    let host = parse(host).expect("host parses");
    let donor = parse(donor).expect("donor parses");
    let mut host_doc = serde_json::to_value(&host).expect("host serializes");
    let donor_doc = serde_json::to_value(&donor).expect("donor serializes");
    host_doc["statements"] = donor_doc["statements"].clone();
    host_doc
}

/// Set every serialized `Span` (`{"start","end"}` object) to the `SYNTHETIC` sentinel
/// (`start = u32::MAX`, `end = 0`) — the one span form validation exempts.
fn synthesize_all_spans(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if map.len() == 2 && map.contains_key("start") && map.contains_key("end") {
                map.insert("start".into(), serde_json::json!(u32::MAX));
                map.insert("end".into(), serde_json::json!(0));
                return;
            }
            for v in map.values_mut() {
                synthesize_all_spans(v);
            }
        }
        serde_json::Value::Array(items) => {
            for v in items.iter_mut() {
                synthesize_all_spans(v);
            }
        }
        _ => {}
    }
}

#[test]
fn foreign_symbol_beyond_the_table_is_rejected_at_deserialize() {
    // The donor references more dynamic symbols than the host's table holds. Before
    // document validation this deserialized and only failed (loudly) on the canonical
    // render; now the out-of-table symbol is caught at deserialize — the contract's
    // fail-loud grading moved earlier. The host's single column is deliberately long so
    // the spliced (shorter) donor spans stay in bounds, isolating the *symbol* check
    // from the span check that would otherwise fire first.
    let doc = splice_doc(
        "SELECT a_single_very_long_column_identifier_name",
        "SELECT zeta_one, zeta_two, zeta_three",
    );
    let err = serde_json::from_value::<Parsed>(doc).expect_err("out-of-table symbol rejected");
    assert!(
        err.to_string().contains("symbol"),
        "deserialize must reject the out-of-table symbol by name: {err}",
    );
}

#[test]
fn foreign_symbol_beyond_the_table_prints_a_placeholder_on_the_debug_path() {
    // The tolerant debug path exists for detached/foreign nodes reached WITHOUT serde
    // (which now rejects that graft at load). Rendering a donor statement directly
    // against a smaller host resolver — same-process misuse — marks the out-of-table
    // symbol as a placeholder instead of panicking.
    let host = parse("SELECT alpha").expect("host parses");
    let donor = parse("SELECT zeta_one, zeta_two, zeta_three").expect("donor parses");
    let debug = donor.statements()[0].debug_sql(host.resolver()).to_string();
    assert!(
        debug.contains("<unresolved>"),
        "the debug path marks the unknown symbol: {debug}"
    );
}

#[test]
fn foreign_symbol_within_the_table_silently_resolves_to_the_wrong_text() {
    // Both trees intern exactly one dynamic symbol, so the donor's symbol is in-range
    // for the host's table and resolves — to the host's text. The donor source is no
    // longer than the host's, so the spliced spans stay in bounds and validation has
    // nothing to reject: this is the ACCEPTED, unvalidatable case (ADR-0003), and
    // asserting the wrong output pins that no accidental guard appears here.
    let doc = splice_doc("SELECT alpha", "SELECT beta");
    let hybrid: Parsed =
        serde_json::from_value(doc).expect("an in-range splice still deserializes");
    assert_eq!(
        hybrid.to_sql(),
        "SELECT alpha",
        "an in-range foreign symbol resolves against the host table"
    );
}

#[test]
fn foreign_literal_span_beyond_the_source_is_rejected_at_deserialize() {
    // The donor's string literal spans bytes the short host source lacks. Before
    // validation the renderer degraded totally (kind-based fallback, no panic); now the
    // out-of-range span is rejected at deserialize, moving the fail-loud earlier.
    let doc = splice_doc(
        "SELECT a",
        "SELECT 'a considerably longer string literal than the host source has'",
    );
    let err =
        serde_json::from_value::<Parsed>(doc).expect_err("out-of-range literal span rejected");
    let message = err.to_string();
    assert!(
        message.contains("span") && message.contains("bounds"),
        "deserialize must reject the out-of-range literal span by name: {message}",
    );
}

#[test]
fn a_synthetic_literal_span_survives_validation_and_renders_by_kind() {
    // The `SYNTHETIC` sentinel is the one literal-span form deserialize does NOT reject
    // (a rewrite-synthesized node legitimately has no source), so it is the path by
    // which the canonical render fallback — spell a span-less literal by kind, stay
    // total, never panic — remains reachable through serde now that out-of-range spans
    // are rejected. This is the serde counterpart to the debug path's kind spelling in
    // `render::tests::debug_sql_resolves_known_symbols_and_spells_literals_by_kind`.
    let parsed = parse("SELECT 'x'").expect("parses");
    let mut doc = serde_json::to_value(&parsed).expect("serializes");
    synthesize_all_spans(&mut doc);

    let restored: Parsed =
        serde_json::from_value(doc).expect("synthetic spans are exempt from validation");
    // No source to slice, so the string literal falls back to its kind spelling `''`,
    // and the canonical render stays total.
    assert_eq!(restored.to_sql(), "SELECT ''");
}
