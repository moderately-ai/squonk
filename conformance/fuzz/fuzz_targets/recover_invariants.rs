// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

#![no_main]
//! libFuzzer target: `parse_recovering` never panics and its partial tree holds the
//! whole-tree invariants.
//!
//! Reuses the stable body
//! ([`squonk_conformance::recovery_invariants::recover_invariants`]): arbitrary
//! bytes are decoded to UTF-8 and fed to the recovering parser under every built-in
//! dialect; the recovered tree is swept for panic-freedom, unique nonzero NodeIds,
//! non-synthetic in-bounds spans, and symbol resolvability across the resync
//! boundaries the resilient path introduces. Run with
//! `cargo +nightly fuzz run recover_invariants`.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    squonk_conformance::recovery_invariants::recover_invariants(data);
});
