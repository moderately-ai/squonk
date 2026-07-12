// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

#![no_main]
//! libFuzzer target: raw bytes agree with the real PostgreSQL parser on accept/reject.
//!
//! Reuses the stable Bolero body
//! ([`squonk_conformance::fuzz::pg_differential_raw_bytes`]): arbitrary bytes are
//! decoded to UTF-8 (invalid/oversized dropped, mirroring `parse_no_panic`) and fed
//! straight to the `pg_query` accept/reject oracle. Unlike the generated
//! `differential` target — which only renders legal-by-construction trees — this
//! searches the raw-input space for the validator-correctness class (accepting SQL
//! PostgreSQL rejects, or vice versa). Run with
//! `cargo +nightly fuzz run pg_differential_raw_bytes`.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    squonk_conformance::fuzz::pg_differential_raw_bytes(data);
});
