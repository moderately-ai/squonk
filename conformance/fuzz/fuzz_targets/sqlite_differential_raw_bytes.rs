// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

#![no_main]
//! libFuzzer target: raw bytes agree with real SQLite on accept/reject + segmentation.
//!
//! Reuses the stable Bolero body
//! ([`squonk_conformance::fuzz::sqlite_differential_raw_bytes`]): arbitrary bytes
//! are decoded to UTF-8 (invalid/oversized dropped, mirroring `parse_no_panic`) and
//! fed to the SQLite parse-only accept/reject + statement-count oracle, which counts
//! via `sqlite3_prepare_v2` + `pzTail` and never executes. Searches the raw-input
//! space for the validator-correctness class (accepting SQL SQLite rejects, or vice
//! versa) and the statement-splitter class (a masked segmentation mis-count).
//!
//! Needs the conformance crate's `oracle-engines` feature (rusqlite): run with
//! `cargo +nightly fuzz run sqlite_differential_raw_bytes --features oracle-engines`.
//! Without the feature the target compiles to a no-op, so `cargo fuzz build` (which
//! builds every target) never fails for want of the linked engine.

use libfuzzer_sys::fuzz_target;

#[cfg(feature = "oracle-engines")]
fuzz_target!(|data: &[u8]| {
    squonk_conformance::fuzz::sqlite_differential_raw_bytes(data);
});

#[cfg(not(feature = "oracle-engines"))]
fuzz_target!(|_data: &[u8]| {});
