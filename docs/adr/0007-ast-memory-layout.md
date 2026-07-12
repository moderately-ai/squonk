# ADR-0007: AST memory layout — owned Box tree

- **Status:** Accepted (2026-06-26)
- **Atoms:** A16, A17, A18

## Context

Given owned-root ownership (ADR-0001), the AST node layout determines allocation, traversal cost, and rewritability. The prior art had a 3400-byte `Statement` and 984-byte `Subscript` from inlined fat variants.

## Decision

- **Owned `'static`, `Box`-disciplined enum tree** for the surface AST. The two alternatives are ruled out by *constraints*, not benchmarks: a `bumpalo` arena forces the viral `'a` (already rejected, ADR-0001) and makes rewriting awkward; `rowan`/`cstree` are immutable (fighting in-place rewrites by downstream consumers) and CST-shaped (wrong for a semantic AST).
- **Enum-size discipline:** box the cold/fat variants (`Statement::CreateTable/Insert/Update/Delete/Merge`, `Subscript` bounds), keep hot small ones inline (`Identifier`, `BinaryOp`). Enforce a per-enum **compile-time size budget** — `const { assert!(size_of::<E>() <= N) }` (codegen-emitted, ADR-0013) — a zero-cost regression gate. Re-enable `clippy::large_enum_variant` as a *measured* tripwire (it is frequency-blind).
- **Compact `Ident`** (`{ sym, quote, Meta }` ≈ 20 B vs 64 B). **`ThinVec`** (leaf dep) for child sequences (8 B inline vs `Vec`'s 24 B); **`SmallVec`** (leaf dep) for short-common containers like `ObjectName`.

## Consequences

- We recover most of the arena's perf without its lifetime: Box-discipline keeps most nodes inline (few separate allocations), `ThinVec` shrinks every list-bearing node, and *heavy* bulk rewriting by a downstream consumer lowers to a **separate index-arena IR** (deferred Phase 6) where the arena's bump-alloc/bulk-free/cache-friendliness actually belong.
- The size budgets fail the build on a fat *inline* variant — a regression guard independent of the runtime perf gate (ADR-0016).
- Backlog: a `Parsed`-root-owned bump arena (arena speed, no public lifetime, via self-referential/`unsafe`) is a measure-first option if parse-time allocation shows up.

## Interconnects

- code: `crates/squonk-ast/src/generated/size_asserts.rs` — per-enum `size_of` budgets
- invariant: every AST node stays within its compile-time `size_of` budget, with child sequences boxed via `thin-vec`.
- xtask: none — enforced at compile time by the generated `const _` size asserts.

## References

Atoms A16–A18. Benchmark backlog #5 (box/inline split), #6 (container choice). oxc's `size_of::<Statement>() == 16` discipline; ruff `AtomicNodeIndex`.
