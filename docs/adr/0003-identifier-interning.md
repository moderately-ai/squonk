# ADR-0003: Identifier interning & Symbol identity

- **Status:** Accepted (2026-06-26)
- **Atoms:** A6, A7, A8

## Context

Identifiers dominate SQL and are compared constantly by a planner (catalog resolution, join keys, `GROUP BY`/`DISTINCT`). The prior art stored owned `String`s (64-byte `Ident`s), making identity a string hash/compare. Identifier *case-folding* — what counts as "the same identifier" — is dialect-dependent and a soundness minefield.

## Decision

- **Identifiers are an interned `Symbol(NonZeroU32)`** newtype in `squonk-ast` (so `Option<Symbol>` is 4 bytes); the mutable interner lives in the parser crate.
- **`Symbol` interns the EXACT original-case text** — lossless and sound. Case-folding is *not* applied at parse (folding-and-discarding is unsound for config/collation-dependent dialects like MySQL `lower_case_table_names` and loses original case for round-trip). The dialect's `Casing { Upper, Lower, Preserve }` is `const` `FeatureSet` data (ADR-0011); folding for identity is the *planner's* concern. An optional precomputed folded-`Symbol` side-table gives O(1) case-insensitive identity where wanted.
- **In-house interner** (dep-free, `unsafe`-free): mutable `{ HashMap<Box<str>, Symbol>, Vec<Box<str>> }` during parse (transient double storage, freed at freeze) → a single-storage `Resolver { Box<[Box<str>]> }` (Send+Sync) shipped on the `Parsed` root, behind a `Resolver` trait in the AST crate.

## Consequences

- `Ident` drops from 64 B to ~16/20 B; identity becomes a `u32` compare — the planner's heaviest work becomes integer ops.
- Interning the exact form keeps the AST a faithful record and serves case-sensitive-by-config dialects soundly; case-insensitivity is layered on, not baked in lossily.
- The interner is a core perf primitive we control fully (niche `Symbol`, folded side-table, keyword pre-interning, freeze lifecycle) — behind a newtype so `lasso` is a drop-in if the in-house version disappoints (benchmark backlog).
- Footguns to document loudly: a `Symbol` is comparable (by `==`) only within one interner. It **deliberately does not implement `Ord`** — a numeric order would be interning-order, not lexicographic, a silent-wrong-sort hazard — so resolve to `&str` when a sorted order is needed.
- **Cross-interner misuse is accepted-and-unchecked (recorded 2026-07-02).** A symbol resolved against a foreign interner fails loud when it is beyond the table (canonical render panics; `try_resolve` is `None`) but silently yields the wrong text when it is within it. Detecting the in-range case requires per-`Symbol` (or per-node) provenance, which this ADR's 4-byte niche decision priced out — a provenance tag doubles `Symbol` or grows every `Meta` to guard a misuse the API already prevents structurally (`Parsed` cannot be constructed from parts publicly, so trees and their resolvers are never recombined by supported paths). The contract, its graded failure modes, and the sanctioned alternatives are documented on `Parsed`'s "Cross-tree safety" section and demonstrated by `crates/squonk/tests/cross_tree_safety.rs`.

## Alternatives considered

`lasso` / `string-interner` (rejected as default — deps, and we need custom behaviour they make awkward; kept as swappable fallbacks). Interning literals (rejected — mostly unique, so interning is pure overhead; see ADR-0006).

## Interconnects

- code: `crates/squonk-ast/src/vocab/mod.rs` — `Symbol`, `Resolver`; `crates/squonk/src/interner/mod.rs` — the in-house interner
- invariant: `size_of::<Symbol>() == 4` and `size_of::<Option<Symbol>>() == 4` via the `NonZeroU32` niche; `Symbol` deliberately does not implement `Ord`; the interner is `unsafe`-free with no `lasso`/`string-interner` dependency.
- xtask: `cargo xtask deps` (keeps interning crates off the published surface); the workspace `unsafe_code = "deny"` lint.

## References

Atoms A6–A8. rustc `Symbol`, rust-analyzer interning, matklad's interner pattern.
