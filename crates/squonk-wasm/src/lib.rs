// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! `squonk-wasm` — parse SQL to JS values in the browser/edge.
//!
//! A `publish = false` bindings crate exposing a tiny JS/wasm surface over the
//! pure-Rust [`squonk`] parser: parse, recovery, tokenization, rendering,
//! transpilation, dialect metadata, and `version`. Parse/tokenize payloads serialize
//! directly to native JS values with `serde-wasm-bindgen`, and parse documents can
//! be passed back into Rust for raw-AST rendering.
//!
//! The implementation lives in the target-agnostic [`core`] module — plain Rust
//! functions over serializable binding views, unit-tested by the normal native CI
//! gate. The `#[wasm_bindgen]` layer is cfg'd to `wasm32` (a `run_smoke` /
//! `smoke` split), so a native build never pulls the
//! macro crate and CI stays green without a wasm runtime. Build/test/size recipes
//! and the untrusted-input recursion note are in the crate README.

pub mod core;

/// The `#[wasm_bindgen]` JS-callable exports, re-exported at the crate root. Only
/// compiled for `wasm32`, where `wasm-bindgen` is a dependency.
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::{
    parse, parse_recovering, parse_recovering_with_options, parse_with_limit, parse_with_options,
    render_sql, schema_version, supported_dialects, tokenize, transpile, version,
};

#[cfg(target_arch = "wasm32")]
pub use wasm::{render_document, render_fragment};
