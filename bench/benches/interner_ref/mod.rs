// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared harness for the ADR-0003 / ADR-0017 interner comparison: our in-house
//! [`Interner`] vs `lasso` (`Rodeo`) vs `string-interner` (`StringInterner`) on the
//! identifier-heavy token stream of the vendored TPC-DS / TPC-H analytic corpora.
//! Shared by the wall-clock bench (`interner_compare.rs`), the heap bench
//! (`interner_heap.rs`), and the parity test (`tests/interner_reference.rs`),
//! mirroring how `keyword_lookup_ref/` serves the phf comparison and `logos_ref/`
//! the logos one.
//!
//! # Why this exists
//!
//! ADR-0003 chose a dependency-free in-house interner (a `Symbol` = `NonZeroU32`
//! newtype over `Vec<Box<str>>` + an FxHash dedup map) and kept `lasso` +
//! `string-interner` only as *measured* fallbacks — the `Symbol` boundary was
//! designed to keep `lasso` a drop-in replacement "if the in-house interner ever
//! disappoints (benchmark backlog)". ADR-0017 keeps both rejected crates off the
//! published dependency graph. This harness is the concrete measurement behind those
//! decisions: over a realistic identifier-heavy SQL workload, does either crate beat
//! the in-house interner on parse-time allocation, freeze cost, or lookup throughput
//! by enough to justify a dependency the published crates otherwise avoid?
//!
//! # Workload — the identifier (`Word`) token stream
//!
//! The interner the parser drives only ever *dynamically* interns non-keyword
//! identifiers: the tokenizer classifies keywords as `TokenKind::Keyword` before
//! they reach the interner, and `Interner::intern` shortcuts the rare
//! canonical-keyword case to a fixed slot without allocating. So the fair,
//! apples-to-apples drop-in workload — exactly what a `lasso` swap would actually
//! intern — is the `TokenKind::Word` stream, extracted here from the real corpus via
//! the production tokenizer, in source order, with the natural repetition an interner
//! dedups (a TPC-DS query names the same columns/tables dozens of times).
//!
//! Keywords are excluded on purpose. Feeding them in would credit our design with the
//! ADR-0004 keyword table (keywords cost us zero allocations), which is orthogonal to
//! the interner data structure under test; the note records that keyword shortcut as
//! a separate production advantage rather than letting it skew the head-to-head.
//!
//! # Measured directly, not through a full parse
//!
//! `lasso`/`string-interner` are not wired into the parser, so the only head-to-head
//! is to drive each interner's public API over the same identifier stream — our
//! `Interner` is reached through its ordinary public surface
//! (`squonk::interner::{Interner, FrozenResolver}` + the `ast::Resolver` trait), with
//! no bench-only accessor. The *input* is realistic (the real tokenizer over the real
//! corpus); the *driver* is the interner in isolation, which is what isolates the
//! data structure the way `keyword_lookup_ref` isolates keyword recognition.
//!
//! # Fairness caveats (stated so a raw log explains itself — ADR-0017)
//!
//! - **Hashers differ by each crate's DEFAULT** (what `cargo add` gives you, i.e. the
//!   cost a real swap would carry): ours uses the dependency-free FxHash builder;
//!   `lasso::Rodeo` defaults to std `RandomState` (SipHash — DoS-resistant, slower on
//!   short keys); `string-interner` defaults to hashbrown's fast hasher. Only
//!   *intern-time* throughput is hasher-sensitive; freeze and frozen lookup are
//!   index-based and hasher-independent.
//! - **Our `intern()` also runs a keyword-recognition probe** per identifier (it must,
//!   to catch canonical keywords); the alternatives do not. That is real work our
//!   interner does in production, so it stays in the intern-throughput arm rather than
//!   being hidden.
//! - **Storage models differ**: ours is `Vec<Box<str>>` — one allocation per distinct
//!   identifier; `lasso`/`string-interner` pack strings into a growing arena (a few
//!   large allocations). The heap bench surfaces this directly.
//! - **`string-interner` has no frozen form**: it stays its own resolver and never
//!   sheds its dedup map. That is a zero freeze cost but a permanently higher retained
//!   footprint; both sides of that trade show up in the heap bench.
//!
//! Each consumer uses a different slice of this module (the heap bench never resolves,
//! the wall-clock bench never reads `dhat`, the parity test never times), so a
//! module-level `allow(dead_code, unused_imports)` keeps `-D warnings` green — the
//! same convention `keyword_lookup_ref` / `logos_ref` use for their shared surface.
#![allow(dead_code, unused_imports)]

use std::collections::HashSet;

use squonk::ast::Resolver as _;
use squonk::ast::Symbol;
use squonk::ast::dialect::FeatureSet;
use squonk::interner::{FrozenResolver, Interner};
use squonk::tokenizer::{TokenKind, tokenize_with};

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------
//
// The two most identifier-dense vendored corpora: sqlglot's TPC-DS and TPC-H
// analytic queries (dozens of column/table references each, heavily repeated). They
// are `include_str!`'d straight from the conformance crate's `corpus/` tree — the
// EXACT bytes conformance already pins (SPDX-clean, MIT) — so there is no second copy
// to drift, and touching only these read-only includes keeps the work inside
// `bench/**` (no write into the separately-owned conformance crate). Paths are
// relative to THIS file; `include_str!` resolves from a file's own directory even
// when the module is `#[path]`-mounted from `tests/`, so the wall-clock bench, the
// heap bench, and the parity test all read the same fixtures.

/// sqlglot complex-query corpus: TPC-DS input statements, `;`-delimited.
const TPC_DS: &str = include_str!("../../../conformance/corpus/sqlglot-complex/tpc-ds.sql");

/// sqlglot complex-query corpus: TPC-H input statements, `;`-delimited.
const TPC_H: &str = include_str!("../../../conformance/corpus/sqlglot-complex/tpc-h.sql");

/// The corpora, in a fixed order so every derived count is deterministic and
/// git-diffable (ADR-0016/0017).
const CORPORA: &[(&str, &str)] = &[("tpc-ds", TPC_DS), ("tpc-h", TPC_H)];

/// The identifier (`Word`) token stream of the corpus, in source order, ready to
/// feed every interner.
///
/// Each entry borrows the `'static` corpus text (a sub-slice, no copy), so building
/// the stream allocates only the returned `Vec`'s backing array — harness bookkeeping
/// that both measurement harnesses construct *outside* their measured region, exactly
/// as `corpus/mod.rs` builds its subset `Vec` before opening the `dhat` profiler.
///
/// Statements are cut on `;` and tokenized independently so one un-lexable chunk is
/// skipped rather than dropping the whole file (no `;` appears inside a string literal
/// in these generated fixtures, so the split is exact); keywords, punctuation,
/// operators, numbers, and string literals are filtered out, leaving exactly the
/// identifiers the interner would dynamically intern.
pub fn identifier_corpus() -> Vec<&'static str> {
    let mut words = Vec::new();
    for &(_, text) in CORPORA {
        for statement in text.split(';') {
            // The permissive Postgres feature set maximizes what lexes (dollar quotes,
            // `::`, params); a chunk that still fails to tokenize is skipped.
            let Ok(tokens) = tokenize_with(statement, &FeatureSet::POSTGRES) else {
                continue;
            };
            for token in tokens {
                if token.kind == TokenKind::Word {
                    let start = token.span.start() as usize;
                    let end = token.span.end() as usize;
                    words.push(&statement[start..end]);
                }
            }
        }
    }
    words
}

/// Distinct-identifier count of a stream — the number of `Box<str>` slots our
/// interner mints and the number of arena entries `lasso`/`string-interner` create.
pub fn distinct_count(words: &[&str]) -> usize {
    words.iter().copied().collect::<HashSet<_>>().len()
}

// ---------------------------------------------------------------------------
// Per-interner drivers
// ---------------------------------------------------------------------------
//
// The three interners have unrelated `Symbol`, resolver, and freeze types (and
// `string-interner` has no freeze at all), so they cannot share a trait object; each
// gets a parallel driver module, the same way the phf harness exposes two free
// `fn`s. Every driver feeds the SAME `&[&str]` and performs the SAME logical work, so
// the benches and the parity test compare like for like.

/// Our in-house interner, reached through its ordinary public API.
pub mod ours {
    use super::{FrozenResolver, Interner, Symbol};
    use squonk::ast::Resolver as _;

    /// Intern every word, discarding the symbols — for the heap bench, which measures
    /// only the interner's own storage (no symbol `Vec` to pollute the retained read).
    pub fn populate(words: &[&str]) -> Interner {
        let mut interner = Interner::new();
        for &word in words {
            let _ = interner.intern(word);
        }
        interner
    }

    /// Intern every word, keeping the symbols in source order — for the lookup bench
    /// and the parity test, which resolve them back.
    pub fn populate_with_symbols(words: &[&str]) -> (Interner, Vec<Symbol>) {
        let mut interner = Interner::new();
        let symbols = words.iter().map(|&word| interner.intern(word)).collect();
        (interner, symbols)
    }

    /// Freeze into the compact, map-free [`FrozenResolver`] shipped on a parsed tree.
    pub fn freeze(interner: Interner) -> FrozenResolver {
        interner.freeze()
    }

    /// Resolve every symbol back to text, summing lengths so the reads are observed.
    pub fn resolve_all(resolver: &FrozenResolver, symbols: &[Symbol]) -> usize {
        symbols.iter().map(|&sym| resolver.resolve(sym).len()).sum()
    }
}

/// `lasso`'s single-threaded `Rodeo`, in its default (`Spur` key, `RandomState`).
pub mod lasso_ref {
    use lasso::{Rodeo, RodeoResolver, Spur};

    pub fn populate(words: &[&str]) -> Rodeo {
        let mut rodeo = Rodeo::default();
        for &word in words {
            let _ = rodeo.get_or_intern(word);
        }
        rodeo
    }

    pub fn populate_with_symbols(words: &[&str]) -> (Rodeo, Vec<Spur>) {
        let mut rodeo = Rodeo::default();
        let symbols = words
            .iter()
            .map(|&word| rodeo.get_or_intern(word))
            .collect();
        (rodeo, symbols)
    }

    /// `into_resolver` drops the dedup map, yielding the compact read-only
    /// `RodeoResolver` — the direct analog of our `Interner::freeze`.
    pub fn freeze(rodeo: Rodeo) -> RodeoResolver {
        rodeo.into_resolver()
    }

    pub fn resolve_all(resolver: &RodeoResolver, symbols: &[Spur]) -> usize {
        symbols.iter().map(|sym| resolver.resolve(sym).len()).sum()
    }
}

/// `string-interner`'s `StringInterner` in its default backend + hasher.
pub mod string_interner_ref {
    use string_interner::{DefaultStringInterner, DefaultSymbol};

    pub fn populate(words: &[&str]) -> DefaultStringInterner {
        let mut interner = DefaultStringInterner::new();
        for &word in words {
            let _ = interner.get_or_intern(word);
        }
        interner
    }

    pub fn populate_with_symbols(words: &[&str]) -> (DefaultStringInterner, Vec<DefaultSymbol>) {
        let mut interner = DefaultStringInterner::new();
        let symbols = words
            .iter()
            .map(|&word| interner.get_or_intern(word))
            .collect();
        (interner, symbols)
    }

    // No `freeze`: `string-interner` has no separate frozen resolver, so it keeps its
    // dedup map for the life of the interner (see the heap bench's retained rows).

    pub fn resolve_all(interner: &DefaultStringInterner, symbols: &[DefaultSymbol]) -> usize {
        symbols
            .iter()
            .map(|&sym| interner.resolve(sym).map_or(0, str::len))
            .sum()
    }
}

// ---------------------------------------------------------------------------
// Report header (shared by both measurement harnesses)
// ---------------------------------------------------------------------------

/// The self-describing context block both benches print, so a raw log states the
/// tradeoff on its own (ADR-0017: name the caveat, never leave it silent).
pub fn report_header(words: &[&str]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# interner comparison: in-house (ADR-0003) vs lasso vs string-interner"
    );
    let _ = writeln!(
        out,
        "#   ours            : Vec<Box<str>> + FxHash dedup map; freeze drops the map."
    );
    let _ = writeln!(
        out,
        "#   lasso           : Rodeo (default Spur key + std RandomState/SipHash);"
    );
    let _ = writeln!(
        out,
        "#                     arena-packed strings; into_resolver() drops the map."
    );
    let _ = writeln!(
        out,
        "#   string-interner : StringInterner (default backend + hashbrown hasher);"
    );
    let _ = writeln!(
        out,
        "#                     arena-packed strings; NO frozen form (keeps its map)."
    );
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "# Workload: the TokenKind::Word stream of the vendored TPC-DS + TPC-H corpora,"
    );
    let _ = writeln!(
        out,
        "# in source order — the non-keyword identifiers our interner dynamically"
    );
    let _ = writeln!(
        out,
        "# interns (keywords are shortcut to fixed slots and never reach it)."
    );
    let _ = writeln!(
        out,
        "#   identifier tokens : {} total, {} distinct",
        words.len(),
        distinct_count(words),
    );
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "# Caveats (see interner_ref/mod.rs): hashers differ by each crate's default;"
    );
    let _ = writeln!(
        out,
        "# our intern() also probes the keyword table per identifier; the published"
    );
    let _ = writeln!(
        out,
        "# crates stay dependency-free either way (ADR-0017) — adopt only on compelling"
    );
    let _ = writeln!(out, "# data, via a follow-up ticket.");
    out
}
