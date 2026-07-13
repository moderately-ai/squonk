# ADR-0020: Lossless CST — deferred (no-go for now)

- **Status:** Accepted (2026-06-29) — *records a deliberate deferral with an explicit revisit trigger*
- **Atoms:** — (resolves the lossless-CST spike `prod-lossless-cst-spike`, deferred to "Phase 6" by [ADR-0005](0005-tokenizer.md); informs [ADR-0008](0008-operator-precedence.md) and the open rejected-alternative [ADR-0018](0018-rejected-bidirectional-parsing.md))

## Context

A **lossless CST** (concrete syntax tree, à la rust-analyzer's [Rowan](https://github.com/rust-analyzer/rowan) green/red trees) is a parallel tree in which *every* token is a node — keywords, punctuation, and trivia included — carrying parent pointers, exact layout, and structural sharing so an editor can reuse unchanged subtrees across edits (incremental reparse). It is the canonical substrate for IDEs, whitespace-exact formatters, and refactoring tools.

[ADR-0005](0005-tokenizer.md) deliberately deferred it — *"A lossless CST / resilient parser is deferred (Phase 6) but not foreclosed"* — and [ADR-0018](0018-rejected-bidirectional-parsing.md) rejected bidirectional/lens parsing while leaving the lossless-CST question explicitly open. That deferral has to remain a **decision**, not an oversight. The trigger to re-check it has now fired: `prod-token-trivia-recovery` landed (status `done`), which means the parser is most of the way to "lossless" *without* a separate tree. This ADR is the recorded go/no-go.

### What is already in place (the near-lossless surface)

The question is not "lossless or nothing" — it is "does a *second* concrete tree buy anything over the facilities the AST already carries?" Those facilities are:

- **A span on every node ([ADR-0002](0002-node-metadata-spans.md)).** Every node carries `Meta { span: Span, node_id: NodeId }`; `Span` is an 8-byte half-open byte range, populated *by construction* with no synthetic holes (`crates/squonk-ast/src/vocab/mod.rs`, generated `Spanned` in `crates/squonk-ast/src/generated/spanned.rs`). So every node maps back to its exact source bytes (`&source[span]`), and `NodeId` is a stable per-parse key for out-of-tree side tables (analysis, diagnostics, rename maps). `LineIndex` recovers `(line, column)` lazily.
- **Exact-text round-trip ([ADR-0006](0006-literals.md), [ADR-0010](0010-rendering.md)).** Literals are a 1-byte tag plus `Meta.span`, materialized lazily from `source[span]`, so `0x1F`, `1_000`, `1.5e3`, and exact escape forms render **byte-for-byte** (`literal_renders_source_slice_verbatim`, `crates/squonk-ast/src/render/tests.rs`); quoted identifiers preserve their delimiter style (`quoted_identifiers_round_trip_their_delimiters`). The `Canonical` render mode is round-trip-*by-construction*: `parse(render(x)) == x`, with the minimal parentheses the one binding-power table requires ([ADR-0008](0008-operator-precedence.md)).
- **Out-of-band trivia recovery ([ADR-0005](0005-tokenizer.md); `prod-token-trivia-recovery`, done).** Comments and whitespace are kept *out* of the token stream but recorded as a sorted, non-overlapping `TriviaIndex` that travels on the `Parsed` root (`crates/squonk/src/tokenizer/trivia.rs`, `crates/squonk/src/parser/parsed.rs`). Capture is enabled with `ParseConfig::capture_trivia` and is zero-cost when off (a `const RECORDING` sink folds it away). Queries are binary searches: `trivia_in(span)` (runs fully inside a node) and `trivia_before(offset)` (the leading comment/whitespace chain abutting a token). The text of each run slices back zero-copy from source.
- **The streaming token stream.** A `BufferedTokenCursor` drives the parser with random-access speculation/backtracking and a `statements()` iterator for bounded memory across a script; it is trivia-capable (`streaming_with_trivia`).
- **An owned, `'static`, structurally-comparable AST ([ADR-0001](0001-owned-root-ast.md)).** `Send + 'static`; the always-equal `Meta` makes ordinary derives structural (spans/ids excluded), so nodes are correct `HashMap` keys.

Together: **byte-faithful slicing, byte-faithful rendering, and offset-addressable trivia** — most of what "lossless" is *for*, carried by the AST + root rather than a parallel tree.

## Decision

**No-go: do not build a separate lossless CST now. Defer, with the explicit revisit trigger below.**

Per-node spans + exact-text render + out-of-band trivia already cover every consumer on the roadmap. The genuine representational gaps a CST would close — trivia/positions bound to *non-AST* tokens, byte-exact redundant parentheses, and incremental green-tree reuse — have **no consumer that needs them today** (the sole downstream is a batch *rewrite* engine, not an interactive editor). A parallel concrete tree is a Phase-6-scale investment that would roughly **double node count and memory** and add a second public tree to render, test, and fuzz, paid against the lean AST identity the project deliberately bought ([ADR-0007](0007-ast-memory-layout.md)) — for benefit nothing currently consumes. Two cheaper, narrower seams already exist to absorb the near-term pressure (`prod-render-byte-fidelity-marker-spike`, `prod-parser-recovery-mode`) and must be exhausted first.

## Use-case-by-use-case verdict

| Consumer | What it needs | Existing facility | Verdict |
|----------|---------------|-------------------|---------|
| **Formatter / pretty-printer** (reproduce-then-reformat) | every token's text, attached comments, re-emit with new layout | walk AST; per node `trivia_before(span.start())` + `trivia_in(span)` recover comments; `Canonical` render re-emits; literal/ident spelling is exact | **Covered** for a *reformatter* (normalize layout, keep comments). The *byte-exact, keep-every-space* formatter is the residual gap → owned by `prod-render-byte-fidelity-marker-spike`. |
| **IDE / LSP navigation, hover, go-to-def, rename, code actions** | map cursor offset ↔ node; stable identity for edits; ranges | `span` per node (offset→node, node→range); `NodeId` side table for rename/analysis maps; `LineIndex` for `(line, col)` | **Covered.** Selection, hover, rename, and structural edits are span + side-table queries; no concrete tree required. |
| **IDE incremental reparse** (sub-frame keystroke latency, green-tree subtree reuse) | structural sharing of unchanged subtrees; full token-as-node cursor with parent pointers | none — reparse is a full (streamed) statement reparse; the AST has no parent pointers and no nodes for keywords/punctuation | **Not covered.** No consumer needs it: the downstream consumer is a batch rewrite engine, not an editor. |
| **Error recovery / partial trees** (multiple diagnostics over broken input) | continue past errors; emit a usable partial tree | the error-sink seam ([ADR-0005](0005-tokenizer.md)); `prod-parser-recovery-mode` delivers multi-diagnostic + partial AST *without* a CST (its acceptance says so) | **Covered** by the planned recovery mode, independent of a CST. |

### What AST + spans + trivia genuinely *cannot* reconstruct

Stated concretely, so a future GO is judged on real gaps and not a vibe:

1. **Trivia bound to a non-AST token.** A comment between `(` and the first argument, or the exact spaces around a `,`, is *recoverable by offset* but is **not tree-addressable** — there is no node for `(` or `,` to hang it on. A consumer can still find it (`trivia_in` / `trivia_before` over the surrounding node's span); it just is not a child of a punctuation node, because punctuation is not a node.
2. **Redundant / explicit parentheses.** Parens are **derived at render, not stored** — there is no `Expr::Nested` ([ADR-0008](0008-operator-precedence.md)). `((a + b))` and `a + b` parse to the *same* AST; `Canonical` render reintroduces only the *required* parens. Byte-exact paren preservation is precisely the **off-by-default fidelity-marker** question — a far cheaper, narrower mechanism than a whole tree (`prod-render-byte-fidelity-marker-spike`, which depends on this spike).
3. **Incremental green-tree reuse.** No structural sharing of unchanged subtrees across edits; an edit reparses the whole (streamed) statement. This is the one capability with *no* cheaper substitute — and the one with no consumer.

A worked illustration of the covered case — a comment-preserving reformatter needs no CST:

```text
for stmt in parsed.statements() {
    // leading comments of the statement: the trivia chain abutting its first byte
    for run in parsed.trivia_before(span_of(stmt).start()) { emit(&parsed.source()[run.span()]); }
    emit(format!("{}", stmt.displayed(&ctx)));   // exact literals/idents, canonical layout
}
```

## Cost of a GO (what a lossless CST would entail)

- **~2× the nodes and memory.** Every keyword, every punctuation mark, and every trivia run becomes a node — a parallel tree roughly doubling node count against the compact-AST constraint established by the then-current native-Rust heap baseline, the borrow-free **12-byte `Token`** (`{ kind, span }`), and the compile-time per-enum size budgets ([ADR-0007](0007-ast-memory-layout.md)). Current publication measurements live separately in [`docs/performance.md`](../performance.md).
- **Its own builder, threaded through the parser.** Either every hand-RD + Pratt production emits CST nodes alongside the AST, or a green/red split is adopted — but [ADR-0007](0007-ast-memory-layout.md) already **rejected `rowan`/`cstree`** as immutable (fighting in-place rewrites by downstream consumers) and CST-shaped (wrong for a semantic AST). So a GO is likely a *bespoke* green/red layer, not an off-the-shelf crate.
- **A second public tree to maintain.** Render, structural-equality, round-trip, proptest, and fuzz surfaces all roughly double, against the dependency-and-surface minimalism of [ADR-0017](0017-engineering-policies.md).
- **Phase-6 scale.** [ADR-0005](0005-tokenizer.md) already placed it there; nothing in the current roadmap pulls it earlier.

This is the same architectural conclusion [ADR-0018](0018-rejected-bidirectional-parsing.md) reached from the other direction: the precedent tools we follow — rust-analyzer (`ungrammar` generates tree *types* only, with a separate hand-RD parser) and datafusion-sqlparser-rs (hand-RD + Pratt + round-trip tests) — keep one semantic tree plus printers, not a parse-print engine over a concrete tree.

## Revisit trigger

Re-open this decision (flip to a GO assessment) when **all** of the following hold:

1. **A concrete consumer materializes that the existing facilities cannot serve** — specifically an *interactive editor / LSP server* (not a batch rewrite engine; the current downstream consumer does not qualify) **or** a published formatter that must preserve *every byte* of original layout; **and**
2. **The cheaper seams are demonstrably insufficient** — the off-by-default fidelity marker (`prod-render-byte-fidelity-marker-spike`) does **not** cover the formatter's byte-fidelity needs, **and** offset-based trivia queries (`trivia_in` / `trivia_before`) do **not** cover the editor's tree-navigation needs; **and**
3. **Incremental performance is an actual, measured requirement** — full-statement reparse latency is measured against the target editor's keystroke-interaction budget and is too slow, justifying green-tree subtree reuse (the one gap with no cheaper substitute).

Intermediate steps (each de-risks and likely *obviates* the eventual GO; **none** requires the CST), in increasing cost:

- **`prod-parser-recovery-mode`** — multiple diagnostics + a partial tree over the error-sink seam. Covers the "usable tree over broken input" half of the IDE story without a CST.
- **`prod-render-byte-fidelity-marker-spike`** — off-by-default parenthesis/layout markers (excluded from structural equality) for byte-faithful round-trip. Covers the formatter half without a CST. This spike *depends on* the present one, so its prototype is the next probe of the byte-fidelity gap.

## Consequences

- [ADR-0005](0005-tokenizer.md)'s Phase-6 deferral is now a recorded decision with a concrete, conjunctive revisit trigger — it cannot be silently forgotten or silently adopted.
- **No CST type, no second tree, no parser-builder change now.** `crates/squonk*/src/` and `generated/` are untouched; the lean AST identity ([ADR-0007](0007-ast-memory-layout.md)) and the 12-byte token are preserved.
- `prod-render-byte-fidelity-marker-spike` and `prod-parser-recovery-mode` remain the cheaper intermediate seams and **must be exhausted before** a CST is reconsidered.
- **No implementation work is scheduled** (no GO) — the two intermediate spikes above already capture the cheaper alternatives, and scheduling a CST implementation now would contradict this decision.

## Interconnects

- code: `crates/squonk/src/tokenizer/trivia.rs` — `TriviaIndex`; `crates/squonk/src/parser/parsed.rs`
- invariant: trivia is offset-recoverable via `TriviaIndex` with no second CST tree and no `rowan`/`cstree` dependency.
- xtask: none — absence guard; `cargo xtask deps` keeps a CST crate off the published surface.

## References

ADRs: [0001](0001-owned-root-ast.md) (owned-root `'static` AST), [0002](0002-node-metadata-spans.md) (spans / `Meta` / `NodeId` / `LineIndex`), [0005](0005-tokenizer.md) (tokenizer; out-of-band trivia; the Phase-6 CST deferral), [0006](0006-literals.md) (lazy literal materialization → exact round-trip), [0007](0007-ast-memory-layout.md) (Box-tree; `rowan`/`cstree` rejected; size budgets), [0008](0008-operator-precedence.md) (parens derived at render; the optional fidelity marker), [0010](0010-rendering.md) (render modes = N printers over one AST), [0018](0018-rejected-bidirectional-parsing.md) (bidirectional parsing rejected; the `ungrammar` precedent).

The related trivia-recovery and byte-fidelity experiments remain the cheaper paths if this decision is revisited.

Code: `crates/squonk-ast/src/vocab/mod.rs` (`Span`/`Meta`/`NodeId`/`LineIndex`), `crates/squonk/src/tokenizer/trivia.rs` + `crates/squonk/src/parser/parsed.rs` (`TriviaIndex` + root queries), `crates/squonk-ast/src/render/` (exact-text render). External: rust-analyzer Rowan green/red trees; rust-analyzer `ungrammar`.
