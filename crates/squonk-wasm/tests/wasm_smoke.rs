// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Runtime proof that the bindings RUN — not just link — on `wasm32`.
//!
//! Executed in Node by the wasm-bindgen test runner (`wasm-pack test --node`, or
//! `cargo build --target wasm32-unknown-unknown --tests` + `wasm-bindgen-test-runner`).
//! The exhaustive behaviour coverage lives in the native `core` unit tests (the
//! normal `cargo nextest` gate); this file is the small on-target smoke that proves
//! the parser and the `#[wasm_bindgen]` `JsValue` boundary work under wasm.
//!
//! The whole file is `wasm32`-only, so `cargo nextest run --workspace` on a native
//! host compiles it to an empty test binary and the wasm-only dev-dependency is
//! never pulled there.
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::wasm_bindgen_test;

use squonk_wasm::core;

// Node is the default wasm-bindgen-test environment; no configure! call is needed.

/// The target-agnostic entry runs on wasm and emits a serializable representation.
#[wasm_bindgen_test]
fn core_parse_runs_on_wasm() {
    let value = core::parse("SELECT 1, name FROM users", "ansi", |document| {
        serde_json::to_value(document).map_err(|error| {
            core::binding_error(
                format!("failed to serialize test document: {error}"),
                "serialization",
            )
        })
    })
    .expect("valid ANSI SQL parses on wasm");
    assert!(value.get("statements").is_some(), "{value}");
    assert!(
        value.get("symbols").is_some(),
        "carries the resolver table: {value}"
    );
}

/// The `#[wasm_bindgen]` `parse` export returns a JS object across the boundary.
#[wasm_bindgen_test]
fn wasm_export_parse_returns_a_js_object() {
    let value = squonk_wasm::parse("SELECT 1", "ansi")
        .expect("the wasm parse export succeeds for valid SQL");
    let document: serde_json::Value =
        serde_wasm_bindgen::from_value(value).expect("the export returns a JS object");
    assert!(document.get("statements").is_some(), "{document}");
}

/// The recovering export returns a single JS value carrying statements and errors.
#[wasm_bindgen_test]
fn wasm_export_parse_recovering_carries_errors() {
    let value = squonk_wasm::parse_recovering("SELECT alpha; FROM x; SELECT beta", "ansi")
        .expect("recovering succeeds for a supported dialect");
    let document: serde_json::Value =
        serde_wasm_bindgen::from_value(value).expect("recovering returns a JS object");
    assert!(document.get("errors").is_some(), "{document}");
    assert!(document.get("statements").is_some(), "{document}");
}

/// The `version` export resolves across the boundary.
#[wasm_bindgen_test]
fn wasm_export_version_is_nonempty() {
    assert!(!squonk_wasm::version().is_empty());
}
