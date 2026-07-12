# ADR-0018: Considered & rejected — bidirectional / lens parsing

- **Status:** Accepted (2026-06-26) — *records a rejected alternative*
- **Atoms:** deep-research outcome (informs ADR-0013, ADR-0014)

## Context

Our worst parser/printer-drift bugs (operator-precedence mis-binding; a `Display` that drops a field; hand-written span impls drifting from the types) are, formally, *lens-law violations* — so we investigated whether "lenses" / bidirectional transformations (BX) / invertible syntax should be the foundation for keeping parse and print consistent. A dedicated, adversarially-verified deep-research pass examined the literature and Rust ecosystem.

## Decision

**Do not adopt a true bidirectional/invertible spec.** Keep hand-written RD + Pratt (ADR-0012) + single-schema codegen (ADR-0013) + round-trip property tests (ADR-0014). Optionally generate the *render skeleton* from the same schema so a field added to a node forces a printer fragment by construction — this captures the "printer can't drift" guarantee *without* a BX engine.

## Rationale (cited)

- The lens **PutGet** law *is* our round-trip property (`parse(render(ast)) == ast`), and "Display drops a field" is the foundational paper's own canonical PutGet counterexample — so the framing is validated, and our round-trip test checks a named law.
- But **every law-guaranteed BX system forces a combinator DSL or a generator**, and the forward parser is *generated* (FliPpr emits a CFG; BiYacc runs GLR) — structurally incompatible with hand-written RD + Pratt. FliPpr's authors decline to embed optimized parsing in a bidirectional program.
- The performance cost is *unquantified* (no surviving benchmark) — so the rejection is **architectural** (forced generator), not measured.
- BX hits hard theoretical limits on our needs: compositional **totality** holds only for recursion-free grammars (SQL is deeply recursive); and a single lens pairs *one* get with *one* put, so multiple render modes (canonical/parenthesized/redacted) and per-dialect rendering are *not* expressible as one lens — they are N printers sharing a parse, which is exactly our design (ADR-0010).
- **No production tool — and nothing in Rust — uses BX for parse-print.** The closest precedents are *our* plan: rust-analyzer's `ungrammar` (generates tree types only, separate hand-RD parser) and datafusion-sqlparser-rs (hand-RD + Pratt + round-trip tests).

## Open questions (deferred)

Optics for AST *rewriting* in Rust (lens-rs vs visitor/arena) was unanswered by the research — a future microbenchmark dig for a downstream rewrite layer. BX × error-recovery compatibility is unaddressed in the literature (and would have fought our error-sink/resilience seam).

## Interconnects

- code: `crates/squonk-ast/src/render/mod.rs` — the one-way `Render` path chosen instead of a bidirectional/lens engine
- invariant: no bidirectional-transformation / lens engine or dependency exists in the workspace; rendering stays the one-way `Render` path (ADR-0010).
- xtask: none — absence guard; `cargo xtask deps` keeps a BX/lens crate off the published surface.

## References

Deep-research task `wvll2s48q` (full record in the design notes). Foster et al. TOPLAS 2007; Rendel & Ostermann (Haskell 2010); Matsuda & Wang FliPpr (ESOP 2013); Xie/Schrijvers/Hu biparsers (POPL 2025).
