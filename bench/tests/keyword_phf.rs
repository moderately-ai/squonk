// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Parity + reproducibility evidence for the ADR-0004 generated-vs-phf comparison.
//!
//! The wall-clock ratios in `examples/compare_keyword_lookup.rs` are only meaningful if both
//! lookups compute the SAME function over the SAME inventory, so this guards:
//! 1. Parity on hits — the phf map and the generated lookup agree on every keyword,
//!    case-insensitively.
//! 2. Parity on misses — they agree (both `None`) on the identifier corpus, which
//!    also keeps that "pure miss" scenario honest.
//! 3. Scale — the inventory really is the full ANSI/PostgreSQL union, not the M1
//!    subset, so the comparison is at the scale the ADR's "if profiling flags it"
//!    upgrade would face.
//!
//! The shared harness is included via `#[path]`, the same cross-target include
//! `tests/logos_reference.rs` uses. `phf` is gated behind `--features phf-compare`
//! (the `[[test]]` is `required-features`-gated), so no default-build dependency is
//! introduced (ADR-0017).

#[path = "../benches/keyword_lookup_ref/mod.rs"]
mod keyword_lookup_ref;

use keyword_lookup_ref::{IDENTIFIERS, Keyword, lookup_keyword, lookup_keyword_phf};

#[test]
fn phf_and_generated_agree_on_every_keyword() {
    for keyword in Keyword::ALL {
        let spelling = keyword.as_str();
        assert_eq!(
            lookup_keyword(spelling),
            Some(keyword),
            "generated lookup must recognize its own spelling {spelling:?}",
        );
        assert_eq!(
            lookup_keyword_phf(spelling),
            Some(keyword),
            "phf and generated disagree on {spelling:?}; the bench would compare different work",
        );
    }
}

#[test]
fn phf_and_generated_agree_case_insensitively() {
    // Upper- and mixed-case spellings must resolve identically through both paths.
    for keyword in Keyword::ALL {
        let upper = keyword.as_str().to_ascii_uppercase();
        assert_eq!(lookup_keyword(&upper), Some(keyword));
        assert_eq!(
            lookup_keyword_phf(&upper),
            Some(keyword),
            "phf lost case-insensitivity on {upper:?}",
        );
    }
}

#[test]
fn identifier_corpus_misses_in_both_lookups() {
    // Keeps the bench's "pure miss" scenario honest: any entry that is secretly a
    // keyword would make the scenario measure hits, not misses.
    for identifier in IDENTIFIERS {
        assert_eq!(
            lookup_keyword(identifier),
            None,
            "{identifier:?} is unexpectedly a keyword in the generated lookup",
        );
        assert_eq!(
            lookup_keyword_phf(identifier),
            None,
            "phf and generated disagree on the non-keyword {identifier:?}",
        );
    }
}

#[test]
fn inventory_is_the_full_ansi_postgres_union() {
    // The whole point of this ticket: measure at full scale, far past the M1 subset.
    assert!(
        Keyword::ALL.len() > 700,
        "expected the full ANSI/PostgreSQL union, got {} keywords",
        Keyword::ALL.len(),
    );
}
