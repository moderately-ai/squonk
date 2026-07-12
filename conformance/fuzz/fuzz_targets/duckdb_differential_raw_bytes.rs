// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

#![no_main]
//! libFuzzer target: raw bytes agree with real DuckDB on accept/reject + segmentation.
//!
//! Reuses the stable Bolero body
//! ([`squonk_conformance::fuzz::duckdb_differential_raw_bytes`]): arbitrary bytes
//! are decoded to UTF-8 (invalid/oversized dropped) and fed to the DuckDB parse-only
//! accept/reject + statement-count oracle, which counts via `duckdb_extract_statements`
//! (the parser, not the preparer) and so never executes — sidestepping DuckDB's
//! "prepare executes all but the last statement" hazard. Searches the raw-input space
//! for the validator-correctness and statement-splitter classes.
//!
//! Needs the conformance crate's `oracle-engines` feature (system libduckdb): run with
//! `cargo +nightly fuzz run duckdb_differential_raw_bytes --features oracle-engines`.
//! Without the feature the target compiles to a no-op, so `cargo fuzz build` never
//! fails for want of the linked engine.

use libfuzzer_sys::fuzz_target;

#[cfg(feature = "oracle-engines")]
fuzz_target!(|data: &[u8]| {
    squonk_conformance::fuzz::duckdb_differential_raw_bytes(data);
});

#[cfg(not(feature = "oracle-engines"))]
fuzz_target!(|_data: &[u8]| {});
