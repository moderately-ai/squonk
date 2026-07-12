// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

#![no_main]
//! libFuzzer target: a generated AST renders and re-parses to the same structure.
//!
//! Reuses the stable Bolero body
//! ([`squonk_conformance::fuzz::roundtrip_arbitrary_input`]): the bytes decode to
//! a legal generated statement which is rendered (fully parenthesized) and re-parsed,
//! then compared structurally through a shared interner. Run with
//! `cargo +nightly fuzz run roundtrip`.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    squonk_conformance::fuzz::roundtrip_arbitrary_input(data);
});
