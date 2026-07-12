# ADR-0017: Engineering policies — dependency minimalism & local-runnable checks

- **Status:** Accepted (2026-06-26)
- **Atoms:** cross-cutting (governs A3, A4, A10, A18, A25, A28, A34, A36)

## Context

Two cross-cutting policies recur across the design and deserve a standing record so future decisions apply them consistently.

## Decision

**1. Dependency minimalism.** Keep the *published* crates (`squonk`, `squonk-ast`) dependency-free where possible; add a dependency only on real need *and* when its complexity/benefit outweighs maintaining the equivalent ourselves.
- A dependency's **transitive tree** matters more than the direct crate: a leaf crate (e.g. `memchr` = 0 deps, `thin-vec`, `smallvec`, `unicode-ident`) is far more acceptable than one dragging a subtree (e.g. `phf` → `phf_shared`+`siphasher`).
- **Dev-deps are the explicit exception** — deps in `conformance`/`bench`/`fuzz` (`publish = false`) are brought in freely (proptest, bolero, arbitrary, pg_query, rusqlite, duckdb, datadriven, gungraun, dhat, …); they never reach downstream users.
- Consequences in practice: in-house interner over `lasso` (ADR-0003), codegen-generated keyword lookup over `phf` (ADR-0004), own `Span` over `text-size` (ADR-0002), xtask over a published proc-macro (ADR-0013) — each with the rejected dep kept as a measured backlog alternative.

**2. Checks are local-runnable, not CI-coupled.** Every gate is a `cargo xtask <cmd>` or a plain `#[test]` (rust-analyzer `tidy` / `ensure_file_contents` style) that runs locally through `cargo nextest run` or `cargo xtask`. CI merely *invokes* them. This governs the codegen drift gate (ADR-0013), the anti-`dialect_of!` ban (ADR-0011), the REUSE/SPDX corpus-license check (ADR-0015), and the perf/alloc gate (ADR-0016).

## Consequences

- The published crates stay lean for downstream embedders; the dev/test surface is rich.
- Gates are reproducible and debuggable locally, not pipeline-locked. The project stays local-first — CI merely invokes the same local gates; the nightly `cargo-fuzz` soak (`.github/workflows/fuzz-nightly.yml`) is the first such workflow.

## Interconnects

- code: `xtask/src/lib.rs` — `PUBLISHED_DEP_ALLOWLIST`, `CHECKS`
- invariant: the published crates depend only on the allowlisted surface (`thin-vec`) and carry zero `unsafe`; every gate is local-runnable and CI merely invokes it.
- xtask: `cargo xtask deps`; `cargo xtask tidy` (the local aggregate over every gate).

## References

Working-preference memories: dependency-minimalism, prefer-local-runnable-checks.
