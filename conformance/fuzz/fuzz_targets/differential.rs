// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

#![no_main]
//! libFuzzer target: a generated statement agrees with the real PostgreSQL parser.
//!
//! Reuses the stable Bolero body
//! ([`squonk_conformance::fuzz::differential_arbitrary_input`]): the bytes decode
//! to a legal generated statement, rendered to SQL and compared against `pg_query`
//! for accept/reject parity (full surface) and structural parity (the comparable
//! subset). Run with `cargo +nightly fuzz run differential`.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    squonk_conformance::fuzz::differential_arbitrary_input(data);
});
