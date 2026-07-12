# ADR-0002: Node metadata — byte-range spans, NodeId, and the Meta wrapper

- **Status:** Accepted (2026-06-26)
- **Atoms:** A2, A3, A4, A5, A9

## Context

The prior art stored line/column on every node, which (a) bloated each span to 32 bytes and (b) foreclosed zero-copy, because without a byte offset you cannot slice the source. Every node also needs identity (for side-tables) and must participate correctly in structural equality.

## Decision

- **`Span { start: u32, end: u32 }`** — an 8-byte, half-open byte range, an **opaque newtype** owned in `squonk-ast` (no `text-size` dependency). `u32` caps a source at 4 GiB; the newtype makes the representation swappable in one place if a real >4 GiB workload appears.
- **Line/col is never stored.** It is recovered lazily from a `LineIndex` (a sorted newline-offset array, `partition_point`), built on first request via a single dependency-free newline scan (`squonk-ast` takes no `memchr` dep) and cached on the `Parsed` root. The common (no-diagnostic) path pays zero.
- **`NodeId(NonZeroU32)`** on every node, for out-of-tree side-tables (analysis, diagnostics, caching) — *not* structural identity. Assigned from a per-parse counter.
- **The `Meta` wrapper** carries `span` + `NodeId` and is **always-equal**: `eq → true`, `hash → no-op` (and `Ord → Equal` if derived). So a plain `#[derive(PartialEq, Eq, Hash)]` yields *structural* equality with span/id excluded — zero per-type hand impls, and AST nodes work as correct `HashMap` keys for CSE.

## Consequences

- Spans are 4× smaller and cheap enough to populate *by construction*, eliminating the prior art's 179 empty-span holes.
- The always-equal wrapper centralizes exclusion across `Eq`/`Hash`/`Ord` in one type, avoiding the std-contract trap (excluding from `Eq` but forgetting `Hash`, which corrupts `HashMap`s).
- `u64` spans were rejected: they double the most-replicated field for a 4 GiB cap that the *materialized AST* (multi-TB for such input) breaks long before. Huge inputs are a **streaming** problem (ADR-0005), not a span-width one.
- `NodeId` is semantically inert (in `Meta`, excluded from eq), so it can be removed later by deleting one field — lower-regret than retrofitting.

## Interconnects

- code: `crates/squonk-ast/src/vocab/mod.rs` — `Span`, `NodeId`, `Meta`, `LineIndex`
- invariant: `size_of::<Span>() == 8`, `size_of::<NodeId>() == 4`, `size_of::<Meta>() == 12`; `Meta`'s `PartialEq`/`Hash`/`Ord` are the always-equal impls, so metadata is identity-transparent and node equality stays structural.
- xtask: none — pinned by the `vocab` size test and the `Meta` structural-equality/ordering contract test.

## References

Atoms A2–A5, A9. rustc `BytePos`/`SourceMap`, rust-analyzer `text-size`/`line-index`. The 179-empty-span-hole failure is closed here + by generated `Spanned` (ADR-0013).
