// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

#![no_main]
//! libFuzzer target: the parser must never panic on arbitrary bytes.
//!
//! Reuses the exact body the stable Bolero check drives
//! ([`squonk_conformance::fuzz::parse_no_panic`]), so the two engines share one
//! harness. Run with `cargo +nightly fuzz run parse_no_panic` (see `fuzz/README.md`).

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    squonk_conformance::fuzz::parse_no_panic(data);
});
