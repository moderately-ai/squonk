# ADR-0012: Parser engine — Parser<D>, hand-RD + Pratt

- **Status:** Accepted (2026-06-26)
- **Atoms:** A27

## Context

The engine assembles the earlier decisions. The prior art was one 21k-line `impl Parser` block (608 methods) with half-finished module extractions (ALTER moved out, its dispatcher left behind).

## Decision

- **`Parser<D: Dialect>`** — monomorphized, reading `FeatureSet` (const-folds for a compile-time dialect; `BuiltinDialect` for runtime). **Hand-written recursive descent** for statements + a **Pratt core** for expressions over the one binding-power table (ADR-0008). A token-index cursor with speculation/backtracking (ADR-0005). The interner lives here, frozen to a resolver on the `Parsed` root. Structured `ParseError` through the error-sink seam. Typed 3-state dialect hooks + `Other(X)` returning `D::Ext`. Output is `Parsed { source, resolver, statements }` (or the `statements()` iterator), in canonical-shape-plus-tag form (ADR-0011).
- **Module organization:** split the parser by *grammar family* — `parser/{expr, query, ddl, dml, dcl}.rs` — each an `impl<D> Parser<D>` block, with **each family's dispatcher co-located with its helpers** (the prior art's split failed because the dispatcher stayed in `mod.rs`).

## Consequences

- Kills, by construction, the DIV mis-bind (Pratt own-rbp), the dual-precedence-match sync panic (one table), the `dialect_of!` god-trait (FeatureSet), and the 21k-line monolith (grammar-family modules with co-located dispatch — restoring unit-testable seams).
- Hand-written RD + Pratt is the universal production choice (and the deep-research, ADR-0018, found no production tool deriving the parser from a spec); parser generators lose on perf and on per-dialect node extensibility.

## Interconnects

- code: `crates/squonk/src/parser/mod.rs` — `Parser`, `Dialect`; `crates/squonk/src/parser/query.rs` — a grammar-family `impl` block
- invariant: one monomorphized `Parser<D>` over a `Dialect` trait; grammar families are `impl<D> Parser<D>` blocks with no `dyn Dialect` in the hot path.
- xtask: none — enforced by the `Parser<D>` shape and the `cargo xtask dialect` `dialect_of!` ban.

## References

Atom A27. matklad Pratt; datafusion-sqlparser-rs / Clang / Ruff / Materialize all hand-written RD.
