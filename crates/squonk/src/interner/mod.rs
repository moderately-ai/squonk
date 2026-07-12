// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! In-house string interner and the frozen [`FrozenResolver`] it produces.
//!
//! Identifiers dominate SQL and are compared constantly downstream (catalog
//! resolution, join keys, `GROUP BY`/`DISTINCT`). Interning turns every
//! identifier into a [`Symbol`] — a `NonZeroU32` newtype — so identity becomes a
//! `u32` compare instead of a string hash/compare.
//!
//! There are two phases:
//! - [`Interner`] is mutable and used during a parse. It keeps a
//!   `HashMap<Box<str>, Symbol>` for dedup (hashed with the dependency-free
//!   FxHash-style `fast_hash::FxBuildHasher`, not SipHash — see that module) plus a
//!   `Vec<Box<str>>` in interning order for non-keyword identifiers. The two stores
//!   hold each dynamic string
//!   twice on purpose; the map copies are transient and freed at
//!   [`Interner::freeze`]. Recognized keywords occupy fixed low virtual slots
//!   backed by static canonical spellings. Keyword identifiers reuse those slots
//!   only when the source spelling is already canonical; otherwise they are
//!   dynamically interned so round-trip rendering keeps exact source case.
//! - [`FrozenResolver`] is the frozen, single-storage result shipped on the parsed
//!   root. It is `Send + Sync` and implements [`squonk_ast::Resolver`], so
//!   AST/renderer consumers resolve symbols back to text without depending on
//!   parser internals.
//!
//! For ordinary identifiers, the interner stores the EXACT original-case text.
//! Case-folding for case-insensitive identity is dialect-dependent and a soundness
//! minefield, so it stays the planner's concern, not the parser's:
//! folding-and-discarding here would be unsound for config/collation-dependent
//! dialects (e.g. MySQL `lower_case_table_names`) and would lose original case
//! for round-tripping. Recognized keyword symbols resolve to their
//! canonical lower-case spelling by design. A keyword token promoted
//! to an identifier uses the exact-spelling path instead.
//!
//! The optional, precomputed folded-`Symbol` side-table is
//! [`FoldedSymbols`]: an opt-in, O(1) case-insensitive identity built on demand
//! from a frozen [`FrozenResolver`] and a dialect [`FeatureSet`]. It is a pure side
//! table — it never changes `Symbol` identity or AST equality — so it keeps
//! case-folding the planner's concern without sacrificing the exact,
//! round-trippable default. The [`Symbol`] newtype boundary also keeps `lasso` a
//! drop-in fallback if the in-house interner ever disappoints (benchmark backlog).

use std::borrow::Cow;
use std::collections::HashMap;

use squonk_ast::dialect::FeatureSet;
use squonk_ast::{FoldedSymbol, Keyword, Symbol, lookup_keyword};

mod fast_hash;

use fast_hash::FxBuildHasher;

const KEYWORD_COUNT: u32 = Keyword::ALL.len() as u32;

/// Mutable, parse-time string interner.
///
/// `intern` deduplicates by text, so equal strings map to the same [`Symbol`] and
/// identity comparisons collapse to a `u32` compare. Symbols are only meaningful
/// within the interner — and the [`FrozenResolver`] it freezes into — that produced
/// them; the same numeric value can mean a different string elsewhere.
#[derive(Debug)]
pub struct Interner {
    /// Dedup index: interned text to its assigned symbol.
    ///
    /// Hashed with the dependency-free FxHash-style [`FxBuildHasher`] rather than
    /// `std`'s SipHash: identifier text is this parse's hottest hash input (see
    /// `fast_hash`'s rationale), and SipHash's DoS resistance buys nothing for keys
    /// already tokenized from the input. The hasher swap leaves dedup unchanged —
    /// the map still keys on text `Eq`.
    map: HashMap<Box<str>, Symbol, FxBuildHasher>,
    /// Dynamic interned text in assignment order; slot `n` holds symbol
    /// `KEYWORD_COUNT + n + 1`.
    strings: Vec<Box<str>>,
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

impl Interner {
    /// Create an interner with virtual fixed low slots reserved for keywords.
    pub fn new() -> Self {
        Self {
            // `default()` (not `new`) selects the FxHash builder; like `new`, it
            // reserves no capacity, so the allocation profile is unchanged.
            map: HashMap::default(),
            strings: Vec::new(),
        }
    }

    /// Intern `s`, returning a stable [`Symbol`] for its exact text.
    ///
    /// Re-interning the same text returns the same symbol, so identity collapses to
    /// a `u32` compare. Source text is interned **verbatim** — case is preserved —
    /// which is what SQL identifiers need for sound, round-trippable identity
    /// (case-folding is dialect-dependent and stays the planner's concern).
    /// A keyword spelled canonically reuses its fixed low slot because it resolves
    /// to the same text; a non-reserved keyword used as an identifier in any other
    /// case (e.g. `Asc`) gets a fresh dynamic symbol that round-trips its spelling.
    ///
    /// There is deliberately no case-folding intern variant: the tokenizer already
    /// classifies keyword *tokens*, so nothing interns a keyword as text — a
    /// fold-to-canonical method would only ever be a footgun for identifier text.
    /// Opt-in case-insensitive identity is layered on after the parse via
    /// [`FoldedSymbols`], never baked into the stored text.
    pub fn intern(&mut self, s: &str) -> Symbol {
        if let Some(keyword) = lookup_keyword(s) {
            if keyword.as_str() == s {
                return keyword.symbol();
            }
        }
        self.intern_dynamic(s)
    }

    /// Intern identifier text the lexer already proved is **not** a keyword,
    /// skipping [`intern`](Self::intern)'s keyword lookup.
    ///
    /// # Proof obligation
    ///
    /// The caller must have established that `s` is not any keyword's exact
    /// canonical spelling — e.g. a `Word` token, for which the lexer's
    /// `lookup_keyword(s)` returned `None` outright. That is *strictly stronger*
    /// than the guard `intern` would skip: `intern`'s keyword fast-path fires only
    /// when `keyword.as_str() == s` (exact canonical spelling), so text that matches
    /// no keyword at all could never take it. The skip therefore changes cost, never
    /// identity — `intern_nonkeyword(s)` and `intern(s)` return the same [`Symbol`]
    /// for every such `s`.
    pub(crate) fn intern_nonkeyword(&mut self, s: &str) -> Symbol {
        self.intern_dynamic(s)
    }

    /// Intern a keyword token used in identifier position, replacing
    /// [`intern`](Self::intern)'s keyword binary search with a single equality.
    ///
    /// # Proof obligation
    ///
    /// The caller must have established `lookup_keyword(s) == Some(kw)` — e.g. a
    /// `Keyword(kw)` token, whose kind *is* that lookup's result. `intern`'s search
    /// would only rediscover `kw`, so one comparison decides identity the same way:
    /// an exact canonical spelling reuses `kw`'s fixed low slot, and any other case
    /// (e.g. `Asc`) is interned verbatim so it round-trips its source spelling.
    /// Identical to `intern(s)` in identity, cheaper in cost.
    pub(crate) fn intern_keyword_ident(&mut self, kw: Keyword, s: &str) -> Symbol {
        if s == kw.as_str() {
            return kw.symbol();
        }
        self.intern_dynamic(s)
    }

    fn intern_dynamic(&mut self, s: &str) -> Symbol {
        if let Some(&existing) = self.map.get(s) {
            return existing;
        }

        // Keywords own the first fixed slots; dynamic identifiers start after
        // them. Exhausting `u32` would take ~4 billion distinct identifiers in a
        // single parse, which is an absurd input rather than a case to recover.
        let dynamic_key =
            u32::try_from(self.strings.len() + 1).expect("interner exceeded u32::MAX");
        let key = KEYWORD_COUNT
            .checked_add(dynamic_key)
            .expect("interner exceeded u32::MAX distinct symbols");
        let symbol = Symbol::new(key).expect("interner keys are one-based and non-zero");

        // Transient double storage (ADR-0003): the map owns a copy as its dedup
        // key, `strings` owns a copy for resolution. The map's copies are dropped
        // at `freeze`, collapsing to a single backing store.
        let text: Box<str> = Box::from(s);
        self.strings.push(text.clone());
        self.map.insert(text, symbol);
        symbol
    }

    /// Freeze into a compact, shareable [`FrozenResolver`].
    ///
    /// Drops the dedup map — collapsing the transient double storage to a single
    /// backing slice — because a frozen resolver never interns again.
    pub fn freeze(self) -> FrozenResolver {
        FrozenResolver {
            strings: self.strings.into_boxed_slice(),
        }
    }
}

/// Frozen, single-storage resolver shipped on the parsed root.
///
/// Holds interned text in symbol order behind a `Box<[Box<str>]>`. Every field is
/// `Send + Sync`, so the resolver auto-derives both and a parsed tree can cross
/// threads with the resolver that gives its symbols meaning. It implements
/// [`squonk_ast::Resolver`] so AST/renderer code resolves symbols without
/// depending on this crate's internals.
#[derive(Debug)]
pub struct FrozenResolver {
    /// Interned text indexed by zero-based [`Symbol::index`].
    strings: Box<[Box<str>]>,
}

impl squonk_ast::Resolver for FrozenResolver {
    fn try_resolve(&self, sym: Symbol) -> Option<&str> {
        resolve_symbol(&self.strings, sym)
    }
}

impl FrozenResolver {
    /// The dynamically-interned strings in `Symbol` assignment order (the slice a
    /// dynamic `Symbol` at index `KEYWORD_COUNT + n + 1` reads from).
    ///
    /// Keywords are omitted: they occupy fixed low slots backed by static canonical
    /// spellings ([`Keyword::ALL`]), stable across every parse and build, so only the
    /// dynamic tail needs carrying to reconstruct this resolver. The serde parse-root
    /// serialization re-interns exactly this slice (via [`Interner::intern_nonkeyword`])
    /// to rebuild an identical resolver on load, so the tree's numeric symbols keep
    /// resolving to the same text across processes.
    #[cfg(any(feature = "serde-serialize", feature = "serde-deserialize"))]
    pub(crate) fn dynamic_strings(&self) -> &[Box<str>] {
        &self.strings
    }

    /// Build the opt-in folded-symbol side table for `features`' identifier casing.
    ///
    /// The result is a *derived view*, not part of the resolver: it borrows `self`
    /// and leaves `Symbol` identity, resolution, and round-trip text untouched.
    /// Consumers that want dialect case-insensitive identity (planners matching
    /// catalog names, join keys, `GROUP BY` keys) build one alongside the resolver;
    /// everyone else pays nothing.
    pub fn fold_symbols(&self, features: &FeatureSet) -> FoldedSymbols {
        FoldedSymbols::build(&self.strings, features)
    }
}

/// Opt-in, precomputed dialect case-insensitive ("folded") symbol identity.
///
/// The default identity is the exact-case [`Symbol`], which round-trips source
/// spelling. Some consumers instead need *dialect* case-insensitive identity:
/// under `Casing::Lower`, `Users` and `users` name the same column. This side
/// table precomputes, for every symbol its [`FrozenResolver`] knows, a [`FoldedSymbol`]
/// such that two symbols share a `FoldedSymbol` exactly when their text folds
/// equal under [`FeatureSet::fold_unquoted_identifier`]. Lookups are an O(1) array
/// index.
///
/// # Identity invariant
///
/// This is a pure *side table*: it is built from a frozen resolver and
/// never mutates `Symbol` identity or AST equality, so exact symbols stay the
/// canonical identity and round-trip fidelity is preserved. Folding is layered on
/// here, never baked into the parse.
///
/// # Quoted identifiers
///
/// Folding describes *unquoted* identity only. Quoted identifiers are
/// case-sensitive, so a consumer keeps using the exact [`Symbol`] for them and
/// folds only the unquoted ones — this table never collapses a distinction the
/// consumer relies on, because the consumer chooses per identifier which identity
/// to read. Leaving the quoted/unquoted policy with the consumer is deliberate:
/// identity policy is the planner's concern, not the parser's.
#[derive(Debug)]
pub struct FoldedSymbols {
    /// Folded identity for each symbol, indexed by zero-based [`Symbol::index`]
    /// across the full keyword + dynamic symbol space.
    folded: Box<[FoldedSymbol]>,
}

impl FoldedSymbols {
    /// Precompute the folded identity of every symbol backed by `strings`.
    ///
    /// Mirrors the interner's transient double storage: a build-time
    /// `folded text -> id` map assigns one [`FoldedSymbol`] per distinct folded
    /// spelling, then is dropped once the compact `Box<[FoldedSymbol]>` is built.
    fn build(strings: &[Box<str>], features: &FeatureSet) -> Self {
        let dynamic = u32::try_from(strings.len()).expect("dynamic symbol count fits u32");
        let total = KEYWORD_COUNT
            .checked_add(dynamic)
            .expect("folded symbol space exceeds u32::MAX");

        let mut folded = Vec::with_capacity(total as usize);
        // Build-time dedup of folded spellings to ids; dropped once `folded` is
        // materialized, collapsing to the single backing slice (cf. `freeze`). Uses
        // the same FxHash builder as the live interner map: the keys are folded
        // identifier text, so the SipHash → FxHash rationale applies identically.
        let mut ids: HashMap<Cow<'_, str>, FoldedSymbol, FxBuildHasher> = HashMap::default();
        let mut next_id: u32 = 1;

        for raw in 1..=total {
            let sym = Symbol::new(raw).expect("folded keys are one-based and non-zero");
            let text = resolve_symbol(strings, sym).expect("every symbol in range resolves");
            // Apply the dialect's own folding rule (ADR-0011); don't reinvent it.
            let key = features.fold_unquoted_identifier(text);
            let id = *ids.entry(key).or_insert_with(|| {
                let id = FoldedSymbol::new(next_id).expect("folded ids are one-based and non-zero");
                next_id += 1;
                id
            });
            folded.push(id);
        }

        Self {
            folded: folded.into_boxed_slice(),
        }
    }

    /// Return the folded identity of `sym`, or `None` if `sym` was not produced by
    /// the resolver this table was built from.
    ///
    /// Mirrors `FrozenResolver::try_resolve`: a `Symbol` from a different interner whose
    /// index is out of range yields `None` rather than panicking.
    pub fn fold(&self, sym: Symbol) -> Option<FoldedSymbol> {
        self.folded.get(sym.index()).copied()
    }
}

/// Resolve `sym` against the fixed keyword slots and a dynamic-string store.
///
/// Shared by the frozen [`FrozenResolver`] and the live [`Interner`]: keyword symbols map
/// to their canonical spelling and dynamic symbols index `strings` in assignment
/// order. A `Symbol` minted by a different interner whose dynamic index is out of
/// range resolves to `None` rather than panicking.
fn resolve_symbol(strings: &[Box<str>], sym: Symbol) -> Option<&str> {
    let raw = sym.as_u32();
    if raw <= KEYWORD_COUNT {
        let index = usize::try_from(raw - 1).expect("keyword symbol index fits usize");
        return Some(Keyword::ALL[index].as_str());
    }
    let dynamic_index =
        usize::try_from(raw - KEYWORD_COUNT - 1).expect("dynamic symbol index fits usize");
    strings.get(dynamic_index).map(|stored| &**stored)
}

/// Live resolution over the still-mutating interner.
///
/// Interning is append-only and never reassigns a [`Symbol`], so a symbol minted
/// while parsing one statement resolves to the same text even as later statements
/// intern more. This lets the streaming parser resolve each statement before the
/// interner is frozen — the bounded-memory `statements()` path.
impl squonk_ast::Resolver for Interner {
    fn try_resolve(&self, sym: Symbol) -> Option<&str> {
        resolve_symbol(&self.strings, sym)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Bring the AST trait's methods (`try_resolve`, `resolve`) into scope for
    // method resolution; `as _` imports the trait for its methods alone.
    use squonk_ast::Resolver as _;
    // `Casing`/`FeatureDelta` build a case-preserving dialect for folding tests;
    // `FeatureSet` is already in scope via `super::*`.
    use squonk_ast::dialect::{Casing, FeatureDelta};

    #[test]
    fn interning_same_text_returns_same_symbol() {
        let mut interner = Interner::new();

        let first = interner.intern("users");
        let second = interner.intern("users");

        assert_eq!(first, second);
        // Identity is a `u32` compare (ADR-0003).
        assert_eq!(first.as_u32(), second.as_u32());
    }

    #[test]
    fn fast_hash_map_dedups_and_folds_a_pile_of_identifiers() {
        use std::collections::HashMap;

        // Regression guard for the SipHash → FxHash swap (ADR-0003): the dedup map's
        // hasher changed, so prove identity and folding are byte-for-byte unchanged
        // across a realistic pile of identifiers — case variants, near-duplicates,
        // and the same names re-interned out of order to stress the map under many
        // distinct keys (where a weak hash would surface as wrong dedup, not just
        // slow lookups). A `HashMap`'s dedup is `Eq`-based, so a correct hasher
        // cannot change which texts share a `Symbol`; this pins that invariant.
        let pile = [
            "users",
            "Users",
            "USERS",
            "id",
            "id",
            "users",
            "i_item_id",
            "ss_quantity",
            "ss_list_price",
            "ss_coupon_amt",
            "store_sales",
            "customer_demographics",
            "date_dim",
            "item",
            "promotion",
            "ss_sold_date_sk",
            "d_date_sk",
            "ss_item_sk",
            "i_item_sk",
            "cd_gender",
            "Cd_Gender",
            "CD_GENDER",
            "id",
            "agg1",
            "agg2",
            "agg3",
        ];

        // Exact dedup: a text seen before must return its first `Symbol`, and two
        // distinct texts must never share one. Tracking text -> first symbol in a
        // std-hashed map (independent of the interner's own hasher) cross-checks both
        // directions in one pass.
        let mut interner = Interner::new();
        let mut seen: HashMap<&str, Symbol> = HashMap::new();
        let mut symbol_to_text: HashMap<Symbol, &str> = HashMap::new();
        for &text in &pile {
            let sym = interner.intern(text);
            match seen.get(&text) {
                Some(&first) => assert_eq!(sym, first, "re-interning {text:?} must dedup"),
                None => {
                    assert!(
                        symbol_to_text.insert(sym, text).is_none(),
                        "{text:?} collided onto another text's symbol"
                    );
                    seen.insert(text, sym);
                }
            }
        }
        // One `Symbol` per *distinct* text — no more, no fewer.
        let distinct_texts: std::collections::HashSet<&str> = pile.iter().copied().collect();
        assert_eq!(seen.len(), distinct_texts.len());

        // Folding still collapses exactly the case-insensitive groups (the build-time
        // folded map shares the same FxHash builder). Under Postgres' lower-folding,
        // the three `users` spellings share one folded id, as do the three
        // `cd_gender` spellings, while the two groups stay distinct from each other.
        let resolver = interner.freeze();
        let folded = resolver.fold_symbols(&FeatureSet::POSTGRES);
        let fold = |text| folded.fold(seen[text]).expect("interned symbol folds");
        assert_eq!(fold("users"), fold("Users"));
        assert_eq!(fold("users"), fold("USERS"));
        assert_eq!(fold("cd_gender"), fold("Cd_Gender"));
        assert_eq!(fold("cd_gender"), fold("CD_GENDER"));
        assert_ne!(fold("users"), fold("cd_gender"));
        assert_ne!(fold("users"), fold("id"));
    }

    #[test]
    fn keywords_are_preinterned_in_fixed_low_slots() {
        let mut interner = Interner::new();

        for keyword in Keyword::ALL {
            assert_eq!(interner.intern(keyword.as_str()), keyword.symbol());
        }
        // A non-canonical keyword spelling is interned verbatim, not folded to the
        // canonical slot.
        assert_ne!(interner.intern("SELECT"), Keyword::Select.symbol());
    }

    #[test]
    fn keyword_identifiers_preserve_non_canonical_source_case() {
        let mut interner = Interner::new();

        let canonical = interner.intern("asc");
        let mixed = interner.intern("Asc");
        let upper = interner.intern("ASC");

        assert_eq!(canonical, Keyword::Asc.symbol());
        assert_ne!(mixed, Keyword::Asc.symbol());
        assert_ne!(upper, Keyword::Asc.symbol());
        assert_ne!(mixed, upper);

        let resolver = interner.freeze();
        assert_eq!(resolver.resolve(canonical), "asc");
        assert_eq!(resolver.resolve(mixed), "Asc");
        assert_eq!(resolver.resolve(upper), "ASC");
    }

    #[test]
    fn nonkeyword_skip_matches_full_intern_identity() {
        // The Word-token fast path (`intern_nonkeyword`, which skips the keyword
        // lookup on text the lexer already classified as a non-keyword) must assign
        // the SAME symbol as the full `intern` — the skip changes cost, never
        // identity. Cover plain identifiers and keyword-*prefixed* non-keywords
        // (`selecting`, `fromage`, `order_by`), the cases where a careless skip could
        // plausibly diverge from the full path.
        for text in ["users", "id", "_c1", "selecting", "fromage", "order_by"] {
            // Precondition the callers guarantee: a `Word` token means the lexer's
            // `lookup_keyword` found nothing at all.
            assert_eq!(lookup_keyword(text), None, "test precondition: {text:?}");
            let skipped = Interner::new().intern_nonkeyword(text);
            let full = Interner::new().intern(text);
            assert_eq!(
                skipped, full,
                "intern_nonkeyword({text:?}) must match intern({text:?})",
            );
        }
    }

    #[test]
    fn keyword_ident_skip_matches_full_intern_identity() {
        // The Keyword-token fast path (`intern_keyword_ident`) replaces `intern`'s
        // binary search with one comparison; it must land on the same symbol. A
        // canonical spelling reuses the fixed slot; any other case is interned
        // verbatim. Both must equal what the full `intern` returns for that text.
        let cases = [
            (Keyword::Select, "select"), // canonical -> fixed keyword slot
            (Keyword::Asc, "asc"),       // canonical -> fixed keyword slot
            (Keyword::Asc, "Asc"),       // mixed case -> verbatim dynamic symbol
            (Keyword::Asc, "ASC"),       // upper case -> verbatim dynamic symbol
        ];
        for (kw, text) in cases {
            // Precondition the callers guarantee: the lexer classified `text` as `kw`.
            assert_eq!(
                lookup_keyword(text),
                Some(kw),
                "test precondition: {text:?}"
            );
            let skipped = Interner::new().intern_keyword_ident(kw, text);
            let full = Interner::new().intern(text);
            assert_eq!(
                skipped, full,
                "intern_keyword_ident({kw:?}, {text:?}) must match intern({text:?})",
            );
        }
        // A canonical spelling specifically lands on the fixed keyword slot.
        assert_eq!(
            Interner::new().intern_keyword_ident(Keyword::Select, "select"),
            Keyword::Select.symbol(),
        );
    }

    #[test]
    fn distinct_text_returns_distinct_symbols() {
        let mut interner = Interner::new();

        let users = interner.intern("users");
        let id = interner.intern("id");

        assert_ne!(users, id);
    }

    #[test]
    fn freeze_then_resolve_returns_exact_original_case() {
        let mut interner = Interner::new();

        // Case is preserved, so these three spellings are three distinct symbols.
        let lower = interner.intern("users");
        let upper = interner.intern("USERS");
        let mixed = interner.intern("Users");
        assert_ne!(lower, upper);
        assert_ne!(lower, mixed);
        assert_ne!(upper, mixed);

        let resolver = interner.freeze();

        assert_eq!(resolver.try_resolve(lower), Some("users"));
        assert_eq!(resolver.try_resolve(upper), Some("USERS"));
        assert_eq!(resolver.try_resolve(mixed), Some("Users"));
        // The panicking convenience method round-trips too.
        assert_eq!(resolver.resolve(mixed), "Users");
    }

    #[test]
    fn resolving_out_of_range_symbol_returns_none() {
        let mut interner = Interner::new();
        let only = interner.intern("solo");
        let resolver = interner.freeze();

        // The one interned slot still resolves...
        assert_eq!(resolver.try_resolve(only), Some("solo"));
        // ...but one past the last slot belongs to no string here.
        let beyond = Symbol::new(only.as_u32() + 1).expect("non-zero symbol");
        assert_eq!(resolver.try_resolve(beyond), None);

        // A fresh parser interner still contains only the preinterned keywords.
        let fresh = Interner::new().freeze();
        assert_eq!(fresh.try_resolve(Keyword::All.symbol()), Some("all"));
        let beyond_keywords = Symbol::new(Keyword::ALL.len() as u32 + 1).expect("non-zero symbol");
        assert_eq!(fresh.try_resolve(beyond_keywords), None);
    }

    #[test]
    fn frozen_resolver_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<FrozenResolver>();
    }

    #[test]
    fn mixed_case_unquoted_identifiers_share_folded_identity() {
        let mut interner = Interner::new();
        let lower = interner.intern("users");
        let mixed = interner.intern("Users");
        let upper = interner.intern("USERS");
        let other = interner.intern("id");

        // Exact identity stays case-sensitive — the canonical AST identity.
        assert_ne!(lower, mixed);
        assert_ne!(lower, upper);

        let resolver = interner.freeze();
        // PostgreSQL folds unquoted identifiers to lower case.
        let folded = resolver.fold_symbols(&FeatureSet::POSTGRES);

        let lower_id = folded.fold(lower).expect("folded id for interned symbol");
        assert_eq!(folded.fold(mixed), Some(lower_id));
        assert_eq!(folded.fold(upper), Some(lower_id));
        // A genuinely different identifier gets a different folded identity.
        assert_ne!(folded.fold(other), Some(lower_id));

        // The side table left exact resolution / round-trip text untouched.
        assert_eq!(resolver.resolve(mixed), "Users");
        assert_eq!(resolver.resolve(upper), "USERS");
    }

    #[test]
    fn quoted_identifiers_keep_case_sensitive_exact_identity() {
        // A quoted identifier interns its exact content; a consumer honouring
        // quoting reads identity from the exact `Symbol`, never the fold table.
        let mut interner = Interner::new();
        let upper = interner.intern("Col");
        let lower = interner.intern("col");

        // Case-sensitive: the two quoted spellings are distinct identities, and
        // stay distinct regardless of any folding the table could offer.
        assert_ne!(upper, lower);

        let resolver = interner.freeze();
        let folded = resolver.fold_symbols(&FeatureSet::POSTGRES);

        // The *unquoted* view would collapse them, but folding is opt-in: the
        // distinction the quoted consumer relies on is never lost.
        assert_eq!(folded.fold(upper), folded.fold(lower));
        assert_ne!(upper, lower);
        assert_eq!(resolver.resolve(upper), "Col");
        assert_eq!(resolver.resolve(lower), "col");
    }

    #[test]
    fn dialect_casing_changes_folded_identity() {
        let mut interner = Interner::new();
        let lower = interner.intern("users");
        let upper = interner.intern("USERS");
        let resolver = interner.freeze();

        // Upper- and lower-folding dialects both collapse the two spellings...
        let upper_fold = resolver.fold_symbols(&FeatureSet::ANSI);
        assert_eq!(upper_fold.fold(lower), upper_fold.fold(upper));

        let lower_fold = resolver.fold_symbols(&FeatureSet::POSTGRES);
        assert_eq!(lower_fold.fold(lower), lower_fold.fold(upper));

        // ...while a case-preserving dialect keeps them distinct.
        let preserve =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.identifier_casing(Casing::Preserve));
        let preserve_fold = resolver.fold_symbols(&preserve);
        assert_ne!(preserve_fold.fold(lower), preserve_fold.fold(upper));
    }

    #[test]
    fn keyword_slots_fold_alongside_dynamic_identifiers() {
        let mut interner = Interner::new();
        // `SELECT` is a non-canonical keyword spelling, so it is interned verbatim
        // as a dynamic identifier rather than reusing the keyword slot.
        let upper_select = interner.intern("SELECT");
        assert_ne!(upper_select, Keyword::Select.symbol());

        let resolver = interner.freeze();

        // The table spans the fixed keyword slots too: under either folding the
        // canonical keyword `select` and the verbatim `SELECT` share one identity.
        let upper_fold = resolver.fold_symbols(&FeatureSet::ANSI);
        assert_eq!(
            upper_fold.fold(Keyword::Select.symbol()),
            upper_fold.fold(upper_select)
        );

        let lower_fold = resolver.fold_symbols(&FeatureSet::POSTGRES);
        assert_eq!(
            lower_fold.fold(Keyword::Select.symbol()),
            lower_fold.fold(upper_select)
        );
    }

    #[test]
    fn fold_returns_none_for_foreign_symbol() {
        let mut interner = Interner::new();
        let only = interner.intern("solo");
        let resolver = interner.freeze();
        let folded = resolver.fold_symbols(&FeatureSet::POSTGRES);

        // The one interned slot folds...
        assert!(folded.fold(only).is_some());
        // ...but one past the last slot belongs to no symbol in this table.
        let beyond = Symbol::new(only.as_u32() + 1).expect("non-zero symbol");
        assert_eq!(folded.fold(beyond), None);
    }
}
