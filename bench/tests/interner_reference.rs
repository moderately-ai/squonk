// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Parity + reproducibility evidence for the ADR-0003 interner comparison.
//!
//! The wall-clock ratios and the heap counts in `examples/compare_interner.rs` (its two
//! `compare-heap`-selected modes) are only meaningful if all three interners do the SAME
//! logical work over the SAME identifier stream, so this guards:
//! 1. Round-trip — every interned symbol resolves back to its exact source
//!    identifier, through the frozen resolver (ours, `lasso`) or the live interner
//!    (`string-interner`). If any interner lost or mangled text the throughput
//!    numbers would compare different work.
//! 2. Dedup parity — all three collapse the stream to exactly the same number of
//!    distinct symbols, one per distinct identifier, so the "distinct identifiers"
//!    the storage comparison turns on is agreed by construction.
//! 3. Scale — the corpus really is a large, heavily-repeated identifier stream, so
//!    the comparison is at the scale a real parse faces, not a toy input.
//!
//! The shared harness is included via `#[path]`, the same cross-target include
//! `tests/keyword_phf.rs` uses. `lasso`/`string-interner` are gated behind
//! `--features interner-compare` (the `[[test]]` is `required-features`-gated), so no
//! default-build dependency is introduced (ADR-0017).

#[path = "../benches/interner_ref/mod.rs"]
mod interner_ref;

use std::collections::HashMap;

// The panicking `resolve` on our frozen resolver comes from the AST trait.
use squonk::ast::Resolver as _;

use interner_ref::{distinct_count, identifier_corpus, lasso_ref, ours, string_interner_ref};

/// Assert an interner mapped `words` to `symbols` as a dedup bijection: equal texts
/// share a symbol, distinct texts never collide, and the distinct-symbol count equals
/// the distinct-text count. Generic over the symbol type so all three interners reuse
/// it (their symbols are all `Copy + Eq + Hash`).
fn assert_dedup_bijection<S>(name: &str, words: &[&str], symbols: &[S], distinct: usize)
where
    S: Copy + Eq + std::hash::Hash,
{
    assert_eq!(
        words.len(),
        symbols.len(),
        "{name}: one symbol per interned word",
    );
    let mut text_to_sym: HashMap<&str, S> = HashMap::new();
    let mut sym_to_text: HashMap<S, &str> = HashMap::new();
    for (&text, &sym) in words.iter().zip(symbols) {
        match text_to_sym.get(text) {
            Some(&first) => assert!(first == sym, "{name}: re-interning {text:?} must dedup"),
            None => {
                assert!(
                    sym_to_text.insert(sym, text).is_none(),
                    "{name}: {text:?} collided onto another text's symbol",
                );
                text_to_sym.insert(text, sym);
            }
        }
    }
    assert_eq!(
        text_to_sym.len(),
        distinct,
        "{name}: distinct symbols must equal distinct identifiers",
    );
}

#[test]
fn all_three_round_trip_every_identifier() {
    let words = identifier_corpus();

    // Ours: resolve through the frozen resolver shipped on a parsed tree.
    let (interner, symbols) = ours::populate_with_symbols(&words);
    let resolver = ours::freeze(interner);
    for (&word, &sym) in words.iter().zip(&symbols) {
        assert_eq!(
            resolver.resolve(sym),
            word,
            "ours lost {word:?} on round-trip"
        );
    }

    // lasso: resolve through the frozen RodeoResolver (resolve takes the key by ref).
    let (rodeo, spurs) = lasso_ref::populate_with_symbols(&words);
    let lasso_resolver = lasso_ref::freeze(rodeo);
    for (&word, spur) in words.iter().zip(&spurs) {
        assert_eq!(
            lasso_resolver.resolve(spur),
            word,
            "lasso lost {word:?} on round-trip",
        );
    }

    // string-interner: no frozen form, so resolve against the live interner.
    let (si, si_symbols) = string_interner_ref::populate_with_symbols(&words);
    for (&word, &sym) in words.iter().zip(&si_symbols) {
        assert_eq!(
            si.resolve(sym),
            Some(word),
            "string-interner lost {word:?} on round-trip",
        );
    }
}

#[test]
fn all_three_dedup_identically() {
    let words = identifier_corpus();
    let distinct = distinct_count(&words);

    let (_, ours_symbols) = ours::populate_with_symbols(&words);
    assert_dedup_bijection("ours", &words, &ours_symbols, distinct);

    let (_, lasso_symbols) = lasso_ref::populate_with_symbols(&words);
    assert_dedup_bijection("lasso", &words, &lasso_symbols, distinct);

    let (_, si_symbols) = string_interner_ref::populate_with_symbols(&words);
    assert_dedup_bijection("string-interner", &words, &si_symbols, distinct);
}

#[test]
fn corpus_is_identifier_heavy_at_scale() {
    let words = identifier_corpus();
    let distinct = distinct_count(&words);

    // Large, real identifier stream (TPC-DS + TPC-H), not a toy input.
    assert!(
        words.len() > 5_000,
        "expected a large identifier stream, got {}",
        words.len(),
    );
    assert!(
        distinct > 200,
        "expected many distinct identifiers, got {distinct}",
    );
    // Heavy repetition is the point: dedup must actually collapse the stream, or the
    // storage comparison would be over a stream with nothing to intern.
    assert!(
        words.len() > distinct * 3,
        "expected heavy repetition, got {} tokens over {distinct} distinct",
        words.len(),
    );
}
