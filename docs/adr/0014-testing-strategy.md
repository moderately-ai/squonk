# ADR-0014: Testing — AST-as-oracle, structural equality, proptest, fuzz

- **Status:** Accepted (2026-06-26)
- **Atoms:** A29, A30, A31

## Context

The prior art's dominant idiom asserts the *re-rendered SQL string*, not the AST — so any wrong-but-identically-rendered tree ships green (the DIV-class bug). It has no generative testing and a single never-run, oracle-less fuzz target. A downstream rewriter consumes the *AST* directly, so the AST must be the test oracle.

## Decision

- **Structural equality (the make-or-break)** = the derived `PartialEq` (the `Meta` wrapper ignores span + `NodeId`, ADR-0002) **+ a shared test interner** so `Symbol`s compare correctly across the two parses of a round-trip; trivia is out-of-band so excluded by construction. The round-trip render uses the **independent, structure-derived parenthesized mode** (ADR-0008/0010) so it is a real oracle, not a tautology (a `normalize()`/symbol-resolving comparison is the cross-interner fallback).
- **Property layer (`proptest`)** in `conformance/`: hand-written `prop_recursive` (depth 4–6) over a deliberately-grown **legal subset** (derive only leaf nodes), committing `proptest-regressions/`. Ship order: P1 `render` never panics → **P2 (flagship) generated-AST structural round-trip** `parse(render(ast)) ≅ normalize(ast)` (oracle = the generator, not the parser) → P3 corpus idempotence/stability → P4 differential (ADR-0015). Illegal trees are triaged/quarantined, never blanket-`prop_assume!`-d.
- **Fuzz layer:** a single `bolero`-authored, `arbitrary`-driven harness runs as a stable Rust test property, replays the committed crash corpus as stable tests, *and* drives libFuzzer. Targets: `parse_no_panic` (weaponizes the unwrap/panic sites) and `roundtrip` (the structural oracle the prior art lacked). Seed from sqllogictest/sqlglot fixtures; `cmin`/`tmin`; commit crashes.

## Consequences

- The AST, not the rendered string, is the unit of truth — closing the prior art's central blind spot.
- All dev-deps (free, per ADR-0017). The libFuzzer engine needs nightly; bolero's stable test mode runs without nightly. OSS-Fuzz is deferred (needs a user base) — see [ADR-0019](0019-oss-fuzz-readiness.md) for the go/no-go decision and its revisit trigger.

## Interconnects

- code: `conformance/src/fuzz.rs` — bolero harness; `conformance/fuzz/fuzz_targets/parse_no_panic.rs`, `conformance/fuzz/fuzz_targets/differential.rs`, `conformance/fuzz/fuzz_targets/roundtrip.rs`
- invariant: three fuzz targets exist over a shared-interner harness; node equality is structural via derived `PartialEq` plus the always-equal `Meta`.
- xtask: none — the target files are present and exercised by the fuzz/proptest suites.

## References

Atoms A29–A31. proptest (integrated shrinking + committed seeds); Black's AST-equivalence check; ReadySet's query-generator.
