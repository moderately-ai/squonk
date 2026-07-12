// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared SQL keyword table and per-position reservation bitsets.
//!
//! The [`Keyword`] inventory, its `ALL`/`as_str` tables, the allocation-free
//! [`lookup_keyword`], and the per-category reservation bitsets are *generated*
//! from the objective keyword data (`keyword_data/*.csv`; PostgreSQL `kwlist.h`
//! and SQL:2016 Part 2) into a `generated` submodule. This hand-written
//! parent owns the [`KeywordSet`] representation, the keyword-symbol bridge, and
//! the *composition* of the generated categories into the per-position reject sets
//! the parser's identifier gates consult.

use crate::vocab::Symbol;

mod generated;

pub use generated::{
    Keyword, MYSQL_FUNCTION_ONLY_KEYWORDS, MYSQL_RESERVED_KEYWORDS, MYSQL_TYPE_FUNC_NAME_KEYWORDS,
    POSTGRES_AS_LABEL_KEYWORDS, POSTGRES_COL_NAME_KEYWORDS, POSTGRES_RESERVED_KEYWORDS,
    POSTGRES_TYPE_FUNC_NAME_KEYWORDS, lookup_keyword,
};

impl Keyword {
    /// The fixed low symbol occupied by this keyword's canonical spelling.
    pub fn symbol(self) -> Symbol {
        Symbol::new(self as u32 + 1).expect("keyword symbols are one-based")
    }
}

// Fail the build loudly if the discriminant-order invariant the keyword
// machinery relies on is ever broken: `symbol()` (`self as u32 + 1`), the
// resolver's reverse lookup (`Keyword::ALL[sym - 1]`), and the [`KeywordSet`]
// bitset all index by discriminant, so the generated `ALL` must list every
// variant in discriminant order or symbols silently mis-resolve. The bitset is
// `[u64; KEYWORD_WORDS]`, which widens with the inventory, and `#[repr(u16)]`
// admits up to 65_535 keywords.
const _: () = {
    let mut index = 0;
    while index < Keyword::ALL.len() {
        assert!(
            Keyword::ALL[index] as usize == index,
            "Keyword::ALL must list every variant in discriminant order; symbol() and \
             the resolver's reverse lookup index by `self as u32`",
        );
        index += 1;
    }
};

/// Number of 64-bit words the keyword bitset needs to give every [`Keyword`] its
/// own slot, derived from the keyword count so it widens automatically with the
/// inventory.
const KEYWORD_WORDS: usize = Keyword::ALL.len().div_ceil(u64::BITS as usize);

/// A const bitset giving every [`Keyword`] discriminant its own bit.
///
/// Backed by `[u64; KEYWORD_WORDS]` rather than a single `u64`, so reservation
/// stays a one-bit-test per-position lookup while scaling past 64
/// keywords with no representation change. Membership and [`union`](Self::union)
/// are `O(1)` and `const`, so the per-position sets fold at compile time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeywordSet([u64; KEYWORD_WORDS]);

impl KeywordSet {
    /// The empty keyword set.
    pub const EMPTY: Self = Self([0; KEYWORD_WORDS]);

    /// Build a set from keywords.
    pub const fn from_keywords(keywords: &[Keyword]) -> Self {
        let mut words = [0; KEYWORD_WORDS];
        let mut index = 0;
        while index < keywords.len() {
            let (word, bit) = keyword_slot(keywords[index] as usize);
            words[word] |= bit;
            index += 1;
        }
        Self(words)
    }

    /// The union of two sets — the per-position reject sets are built by unioning
    /// the generated `kwlist.h` category bitsets (e.g. a ColId rejects
    /// `type_func_name ∪ reserved`), so this must be `const`.
    pub const fn union(self, other: Self) -> Self {
        let mut words = self.0;
        let mut index = 0;
        while index < KEYWORD_WORDS {
            words[index] |= other.0[index];
            index += 1;
        }
        Self(words)
    }

    /// The set difference `self \ other` — every keyword in `self` that is not in
    /// `other`. Carves a small allowlist out of a broad reserved set (MySQL admits its
    /// reserved window-function names — `ROW_NUMBER`, `RANK`, … — as call heads via a
    /// dedicated grammar, so its function-name reject set is the reserved set minus those),
    /// so it must be `const` to fold at compile time alongside [`union`](Self::union).
    pub const fn difference(self, other: Self) -> Self {
        let mut words = self.0;
        let mut index = 0;
        while index < KEYWORD_WORDS {
            words[index] &= !other.0[index];
            index += 1;
        }
        Self(words)
    }

    /// True if `keyword` is present.
    pub const fn contains(self, keyword: Keyword) -> bool {
        let (word, bit) = keyword_slot(keyword as usize);
        self.0[word] & bit != 0
    }
}

/// The `(word index, bit mask)` addressing a keyword `discriminant` in the bitset.
///
/// `discriminant / 64` selects the word and `discriminant % 64` the bit, so a
/// keyword's slot stays fixed as the bitset widens. The discriminant-order
/// invariant asserted above keeps these in lockstep with `symbol()`.
const fn keyword_slot(discriminant: usize) -> (usize, u64) {
    let bits = u64::BITS as usize;
    (discriminant / bits, 1_u64 << (discriminant % bits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_keyword_is_case_insensitive_and_alloc_free() {
        assert_eq!(lookup_keyword("select"), Some(Keyword::Select));
        assert_eq!(lookup_keyword("SeLeCt"), Some(Keyword::Select));
        assert_eq!(lookup_keyword("selector"), None);
        assert_eq!(lookup_keyword("café"), None);
    }

    #[test]
    fn lookup_table_covers_every_keyword() {
        for keyword in Keyword::ALL {
            assert_eq!(lookup_keyword(keyword.as_str()), Some(keyword));
        }
    }

    /// The obvious-correct keyword lookup: ASCII-lower-case the word and linear-scan
    /// the inventory. The shipped `lookup_keyword` is a length-bucketed search that
    /// compares the lower-cased bytes as a packed integer (`u64`/`u128`); this is the
    /// independent oracle it must match, so a packing, endianness, bucket-dispatch, or
    /// table-sort bug surfaces as a disagreement.
    fn reference_lookup(word: &str) -> Option<Keyword> {
        let lowered = word.to_ascii_lowercase();
        Keyword::ALL
            .into_iter()
            .find(|&keyword| keyword.as_str() == lowered)
    }

    /// Upper/lower alternating copy of an ASCII spelling (`select` -> `SeLeCt`), so
    /// the agreement test exercises mixed case across the whole inventory.
    fn alternating_case(spelling: &str) -> String {
        spelling
            .bytes()
            .enumerate()
            .map(|(index, byte)| {
                if index % 2 == 0 {
                    byte.to_ascii_uppercase() as char
                } else {
                    byte.to_ascii_lowercase() as char
                }
            })
            .collect()
    }

    #[test]
    fn packed_lookup_agrees_with_reference_over_keywords_and_non_keywords() {
        // Non-keywords that must all MISS — snake_case identifiers (the real hot-path
        // input is overwhelmingly identifiers, not keywords) plus adversarial edges:
        // empty, lone `_`, keyword-with-affix, near-misses, and non-ASCII words.
        const NON_KEYWORDS: &[&str] = &[
            "user_id",
            "created_at",
            "customer_email",
            "order_total",
            "line_item",
            "txn_ref",
            "qty_on_hand",
            "unit_price_usd",
            "is_active",
            "uuid_pk",
            "foo",
            "bar",
            "baz",
            "xyzzy",
            "col_1",
            "",
            "_",
            "select_",
            "_select",
            "selectt",
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            "café",
            "naïve",
            "señor",
        ];

        let mut probes: Vec<String> = Vec::new();
        for keyword in Keyword::ALL {
            let spelling = keyword.as_str();
            // Canonical, all-upper, and alternating case across the whole inventory.
            probes.push(spelling.to_string());
            probes.push(spelling.to_ascii_uppercase());
            probes.push(alternating_case(spelling));
            // Near-misses probe the length-bucket boundaries and the packing: one byte
            // longer, one byte shorter, a prefixed copy. Spellings are ASCII, so the
            // byte slice is a char boundary. Comparing against the reference (not a flat
            // `None`) keeps it correct when a truncation IS a shorter keyword (`asc` ->
            // `as`).
            probes.push(format!("{spelling}z"));
            probes.push(format!("z{spelling}"));
            if spelling.len() > 1 {
                probes.push(spelling[..spelling.len() - 1].to_string());
            }
        }
        probes.extend(NON_KEYWORDS.iter().map(|word| (*word).to_string()));

        for probe in &probes {
            assert_eq!(
                lookup_keyword(probe),
                reference_lookup(probe),
                "packed lookup disagrees with the reference scan on {probe:?}",
            );
        }
    }

    #[test]
    fn keyword_symbols_are_fixed_low_slots() {
        // The inventory is alphabetical, so `A` is the first variant; symbols are
        // one-based discriminants, and the last keyword fills the `ALL.len()` slot.
        assert_eq!(Keyword::A.symbol().as_u32(), 1);
        assert_eq!(Keyword::A as usize, 0);
        let last = *Keyword::ALL.last().expect("non-empty inventory");
        assert_eq!(last.symbol().as_u32(), Keyword::ALL.len() as u32);
        // The symbol is exactly the discriminant plus one, for every keyword.
        for keyword in Keyword::ALL {
            assert_eq!(keyword.symbol().as_u32(), keyword as u32 + 1);
        }
    }

    #[test]
    fn keyword_set_round_trips_every_keyword() {
        let all = KeywordSet::from_keywords(&Keyword::ALL);
        for keyword in Keyword::ALL {
            assert!(all.contains(keyword), "{keyword:?} should be present");
            assert!(
                !KeywordSet::EMPTY.contains(keyword),
                "{keyword:?} should be absent from the empty set",
            );
        }
    }

    #[test]
    fn keyword_set_union_is_membership_union() {
        let left = KeywordSet::from_keywords(&[Keyword::Select, Keyword::From]);
        let right = KeywordSet::from_keywords(&[Keyword::From, Keyword::Where]);
        let union = left.union(right);
        for keyword in [Keyword::Select, Keyword::From, Keyword::Where] {
            assert!(
                union.contains(keyword),
                "{keyword:?} should be in the union"
            );
        }
        assert!(
            !union.contains(Keyword::Join),
            "Join was in neither operand"
        );
    }

    #[test]
    fn keyword_bitset_addresses_slots_across_word_boundaries() {
        // The bitset packs discriminants into 64-bit words; prove the addressing
        // is correct across word boundaries so the representation scales with the
        // full inventory (which already spans many words).
        assert_eq!(keyword_slot(0), (0, 1));
        assert_eq!(keyword_slot(63), (0, 1 << 63));
        assert_eq!(keyword_slot(64), (1, 1));
        assert_eq!(keyword_slot(127), (1, 1 << 63));
        assert_eq!(keyword_slot(128), (2, 1));

        // Round-trip set/get over a synthetic 130-slot space spanning three words.
        let mut words = [0_u64; 3];
        let present = [0_usize, 1, 63, 64, 65, 127, 129];
        for &discriminant in &present {
            let (word, bit) = keyword_slot(discriminant);
            words[word] |= bit;
        }
        for discriminant in 0..130_usize {
            let (word, bit) = keyword_slot(discriminant);
            assert_eq!(
                words[word] & bit != 0,
                present.contains(&discriminant),
                "slot {discriminant}",
            );
        }
    }
}
