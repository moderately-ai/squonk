// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The `#[wasm_bindgen]` JS-callable surface over [`crate::core`].
//!
//! wasm-only (`cfg(target_arch = "wasm32")`, applied at the module declaration in
//! `lib.rs`) so a native build never pulls the `wasm-bindgen` macro crate.

use serde::Serialize;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

use crate::core;

/// Parse `sql` under the named `dialect`, returning the parsed tree as a native JS
/// object. On failure the JS call throws the same diagnostic object shape.
///
/// `dialect` is a built-in name compiled into this artifact; use
/// [`supported_dialects`] to inspect the active variant. Uses the default
/// recursion limit — see [`parse_with_limit`] for untrusted input.
#[wasm_bindgen]
pub fn parse(sql: &str, dialect: &str) -> Result<JsValue, JsValue> {
    core::parse(sql, dialect, |document| to_js_value(document)).map_err(to_js_error)
}

/// [`parse`] with an explicit recursion-depth `limit` — the DoS guard for
/// untrusted browser/edge SQL (a hostile deeply-nested statement fails cleanly at
/// `limit` instead of overflowing the wasm stack).
#[wasm_bindgen]
pub fn parse_with_limit(sql: &str, dialect: &str, limit: u32) -> Result<JsValue, JsValue> {
    core::parse_with_limit(sql, dialect, limit as usize, |document| {
        to_js_value(document)
    })
    .map_err(to_js_error)
}

/// Parse with explicit recursion-depth, trivia-capture, and float-as-decimal options.
#[wasm_bindgen]
pub fn parse_with_options(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<u32>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> Result<JsValue, JsValue> {
    core::parse_with_options(
        sql,
        dialect,
        recursion_limit.map(|limit| limit as usize),
        capture_trivia,
        parse_float_as_decimal,
        |document| to_js_value(document),
    )
    .map_err(to_js_error)
}

/// Parse `sql` recovering past errors: one JS object carrying both the partial
/// tree's `statements` and every diagnostic in an `errors` array. SQL syntax
/// errors are data in the returned document; binding/setup errors such as an
/// unknown dialect still throw like the fail-fast parse APIs.
#[wasm_bindgen]
pub fn parse_recovering(sql: &str, dialect: &str) -> Result<JsValue, JsValue> {
    core::parse_recovering(sql, dialect, |document| to_js_value(document)).map_err(to_js_error)
}

/// Recovering parse with explicit recursion-depth, trivia-capture, and
/// float-as-decimal options.
#[wasm_bindgen]
pub fn parse_recovering_with_options(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<u32>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> Result<JsValue, JsValue> {
    core::parse_recovering_with_options(
        sql,
        dialect,
        recursion_limit.map(|limit| limit as usize),
        capture_trivia,
        parse_float_as_decimal,
        |document| to_js_value(document),
    )
    .map_err(to_js_error)
}

/// Supported dialect metadata as a native JS array.
#[wasm_bindgen]
pub fn supported_dialects() -> Result<JsValue, JsValue> {
    core::supported_dialects(|dialects| to_js_value(dialects)).map_err(to_js_error)
}

/// Tokenize `sql` under `dialect`, returning a native JS object.
#[wasm_bindgen]
pub fn tokenize(sql: &str, dialect: &str, include_trivia: bool) -> Result<JsValue, JsValue> {
    core::tokenize(sql, dialect, include_trivia, |document| {
        to_js_value(document)
    })
    .map_err(to_js_error)
}

/// Render SQL under `dialect`.
#[wasm_bindgen]
pub fn render_sql(sql: &str, dialect: &str, mode: &str) -> Result<String, JsValue> {
    core::render_sql(sql, dialect, mode).map_err(to_js_error)
}

/// Render a serialized parse document under `dialect`.
#[wasm_bindgen]
pub fn render_document(document: JsValue, dialect: &str, mode: &str) -> Result<String, JsValue> {
    let parsed = serde_wasm_bindgen::from_value(document).map_err(|error| {
        to_js_error(core::binding_error(
            format!("failed to deserialize parse document: {error}"),
            "deserialize",
        ))
    })?;
    core::render_document(&parsed, dialect, mode).map_err(to_js_error)
}

/// Render a single sub-node of a serialized parse document, selected by node id.
#[wasm_bindgen]
pub fn render_fragment(
    document: JsValue,
    node_id: u32,
    dialect: &str,
    mode: &str,
) -> Result<String, JsValue> {
    let parsed = serde_wasm_bindgen::from_value(document).map_err(|error| {
        to_js_error(core::binding_error(
            format!("failed to deserialize parse document: {error}"),
            "deserialize",
        ))
    })?;
    core::render_fragment(&parsed, node_id, dialect, mode).map_err(to_js_error)
}

/// Parse under `source_dialect` and render under `target_dialect`.
#[wasm_bindgen]
pub fn transpile(sql: &str, source_dialect: &str, target_dialect: &str) -> Result<String, JsValue> {
    core::transpile_sql(sql, source_dialect, target_dialect).map_err(to_js_error)
}

/// Pretty-print `sql` under `dialect`. `indent_width`
/// spaces per level, `max_line_length` columns before groups break, and
/// `keyword_case` one of `upper` / `lower` / `preserve`.
///
/// A documented preview rather than a full-fidelity formatter: nested-expression /
/// subquery layout and comment placement carry known limitations (see the
/// `squonk::format` module docs). Output always re-parses and no comment is dropped.
#[wasm_bindgen]
pub fn format(
    sql: &str,
    dialect: &str,
    indent_width: u32,
    max_line_length: u32,
    keyword_case: &str,
) -> Result<String, JsValue> {
    core::format(
        sql,
        dialect,
        indent_width as usize,
        max_line_length as usize,
        keyword_case,
    )
    .map_err(to_js_error)
}

/// The library version string.
#[wasm_bindgen]
pub fn version() -> String {
    core::version().to_owned()
}

/// The wire-schema version of the serialized JSON surface (`docs/schema-contract.md`),
/// independent of the package [`version`]. Lets a JS consumer branch on the shape
/// contract it was built against. Exported snake-case like every other raw binding;
/// the TypeScript facade maps it to `schemaVersion` alongside `version`.
#[wasm_bindgen]
pub fn schema_version() -> u32 {
    core::schema_version()
}

fn to_js_value(value: &(impl Serialize + ?Sized)) -> core::BindingResult<JsValue> {
    let serializer = serde_wasm_bindgen::Serializer::json_compatible();
    value.serialize(&serializer).map_err(|error| {
        core::binding_error(
            format!("failed to serialize binding response: {error}"),
            "serialization",
        )
    })
}

fn to_js_error(error: squonk::bindings::ParseDiagnostic) -> JsValue {
    to_js_value(&error).unwrap_or_else(|_| JsValue::from_str(&error.message))
}
