// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Dependency-free FxHash-style hasher for the interner's dedup maps.
//!
//! The interner's `HashMap`s key on identifier *text* (exact-case `Box<str>` for
//! the live dedup index; folded `Cow<str>` for the opt-in case-insensitive side
//! table). `std`'s default `RandomState` hashes those keys with SipHash-1-3, whose
//! per-byte mixing is cryptographically strong but slow: the perf-ceiling analysis
//! (`docs/performance.md`) measured `sip::Hasher::write` at 6.1% of
//! parse CPU *alone* on identifier-heavy `star_join` (~11% for the whole intern+hash
//! family). This module replaces that hasher with the rustc-hash / Firefox FxHash
//! construction — a `wrapping_mul` by a fixed odd constant after a `rotate_left`,
//! folding the key a machine word at a time — which is far cheaper for the short
//! keys identifiers actually are.
//!
//! ## DoS-resistance trade-off
//!
//! SipHash's value is hash-flooding resistance: an adversary who controls map keys
//! cannot force pathological collisions because the per-process seed is secret.
//! That protection buys nothing here. These keys are SQL identifier lexemes that
//! the tokenizer has *already* produced from the input, not untrusted values routed
//! straight into a long-lived map — and the parser independently bounds the work an
//! input can induce (recursion guard, single-pass interning). A non-seeded,
//! non-cryptographic hash is therefore the right tool: we trade a defence we do not
//! need for a measurable cut to the dominant interning cost. Hand-rolled (no
//! `fxhash`/`rustc-hash` dependency); kept allocation-free and
//! branch-lean, and confined to the interner's maps — `Symbol` identity, the public
//! API, and dedup/fold semantics are untouched (a `HashMap`'s `Eq`-based dedup does
//! not depend on which `Hasher` it uses).

use std::hash::{BuildHasherDefault, Hasher};

/// FxHash 64-bit multiply constant (the rustc-hash / Firefox value): an odd number
/// with well-distributed bits, so each `wrapping_mul` diffuses the freshly-folded
/// word across the whole accumulator.
const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

/// Bits to `rotate_left` before folding each word, so high bits of the running hash
/// re-enter the low end the next multiply mixes (the rustc-hash rotation).
const ROTATE: u32 = 5;

/// FxHash-style [`Hasher`] over a fixed (non-seeded) state.
///
/// Deterministic by construction: equal byte streams always yield the same hash, so
/// it upholds the `Hash`/`Eq` contract the dedup maps rely on. Folds the key a
/// 64-bit word at a time regardless of pointer width, keeping the hash stable across
/// targets.
#[derive(Default)]
pub(super) struct FxHasher {
    hash: u64,
}

impl FxHasher {
    /// Fold one machine word into the running hash: rotate the accumulator, mix in
    /// `word` with `xor`, then diffuse with one multiply by [`SEED`].
    #[inline]
    fn add_word(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(ROTATE) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, mut bytes: &[u8]) {
        // Consume the key a word at a time, then the 4/2/1-byte tail — the canonical
        // FxHash fold. The length guards make every chunk read in-bounds, so this
        // stays panic-free and unsafe-free (`unsafe_code = "deny"`, ADR-0017).
        while let Some((chunk, rest)) = bytes.split_first_chunk::<8>() {
            self.add_word(u64::from_ne_bytes(*chunk));
            bytes = rest;
        }
        if let Some((chunk, rest)) = bytes.split_first_chunk::<4>() {
            self.add_word(u64::from(u32::from_ne_bytes(*chunk)));
            bytes = rest;
        }
        if let Some((chunk, rest)) = bytes.split_first_chunk::<2>() {
            self.add_word(u64::from(u16::from_ne_bytes(*chunk)));
            bytes = rest;
        }
        if let Some(&byte) = bytes.first() {
            self.add_word(u64::from(byte));
        }
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

/// `BuildHasher` for the interner's dedup maps: builds a fresh, zero-state
/// [`FxHasher`] per key, exactly as `rustc-hash` exposes its own builder. `std`'s
/// [`BuildHasherDefault`] is dependency-free, so no `BuildHasher` impl is
/// hand-rolled — only the `Hasher` is.
pub(super) type FxBuildHasher = BuildHasherDefault<FxHasher>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    /// Hash a `str` the way a `HashMap<Box<str>, _>` key would (the `Hash for str`
    /// impl writes the bytes then a `0xff` terminator), so these exercise the real
    /// keying path, not just a bare `write`.
    fn hash_str(s: &str) -> u64 {
        let mut hasher = FxHasher::default();
        s.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn equal_keys_hash_equally_across_lengths() {
        // The `Hash`/`Eq` contract the dedup map depends on: equal text, equal hash —
        // and it must hold across the word/tail boundaries `write` switches on.
        for s in [
            "",
            "a",
            "id",
            "abc",
            "user",
            "orders",
            "customer",
            "a_long_identifier_name",
        ] {
            assert_eq!(hash_str(s), hash_str(s), "deterministic for {s:?}");
        }
    }

    #[test]
    fn distinct_keys_diffuse_to_distinct_hashes() {
        // Not a contract (collisions are always legal — `Eq` still disambiguates),
        // but a smoke test that the fold actually mixes: a pile of identifiers,
        // including near-duplicates that differ only in case or one byte, must not
        // all collapse to one bucket.
        let keys = [
            "users",
            "Users",
            "USERS",
            "user",
            "users_",
            "id",
            "id2",
            "i_item_id",
            "ss_quantity",
            "ss_list_price",
            "ss_coupon_amt",
            "store_sales",
            "date_dim",
        ];
        let mut hashes: Vec<u64> = keys.iter().map(|k| hash_str(k)).collect();
        hashes.sort_unstable();
        hashes.dedup();
        assert_eq!(hashes.len(), keys.len(), "distinct keys collided wholesale");
    }
}
