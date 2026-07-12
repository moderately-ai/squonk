# ADR-0004: Keyword recognition

- **Status:** Accepted (2026-06-26)
- **Atoms:** A10

## Context

Every word token must be classified as a keyword (and which) or an identifier. The prior art's real cost was *allocating* the word `String` before a binary search over ~1092 keywords — and we have already eliminated that with zero-copy tokens (ADR-0005). Reserved-vs-unreserved keyword status is dialect-dependent.

## Decision

- **Recognition is dep-free, codegen-generated** (by the ADR-0013 xtask): a case-insensitive, no-alloc lookup on the borrowed `&str` slice — starting with a sorted-table binary search or a length-bucketed `match`. A generated perfect-hash is a backlog upgrade if profiling flags it; `phf` is the fallback dep (it pulls `phf_shared`+`siphasher`).
- **Reserved-ness is a per-dialect `const` bitset** indexed by the `Keyword` discriminant — compile-time `FeatureSet` data; `is_reserved(kw)` is one bit-test.
- **Keyword-as-identifier:** the tokenizer recognizes the `Keyword`; the parser accepts a *non-reserved* keyword as an identifier via the bitset. Keywords are **pre-interned** into fixed low `Symbol`s at interner construction, so a keyword-used-as-identifier already has its `Symbol`.

## Consequences

- O(1)-ish, zero-allocation keyword lookup; one shared keyword table serves all dialects, with reserved-ness as cheap per-dialect data.
- Generating the table via our own codegen keeps the published crate dependency-free (per ADR-0017) while leaving `phf` as a measured option.

## Interconnects

- code: `crates/squonk-ast/src/dialect/keyword/generated.rs` — generated keyword lookup; `crates/squonk-ast/src/dialect/keyword.rs` — reserved-word data
- invariant: the keyword table is codegen-generated and drift-tested; no `phf` in the published surface.
- xtask: `cargo xtask deps`; the `generated_files_are_up_to_date` drift test.

## References

Atom A10. Benchmark backlog #3 (dep-free generated vs `phf`). PostgreSQL's 2019 binary-search→perfect-hash move; Servo `cssparser`.
