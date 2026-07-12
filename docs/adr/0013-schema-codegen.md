# ADR-0013: Schema-driven codegen — an xtask, not a proc-macro

- **Status:** Accepted (2026-06-26); amended 2026-06-30 — node classification is convention-based; the `#[ast]`/`#[span]`/`#[sql]` markers were not implemented (see Amendment).
- **Atoms:** A28

## Context

The prior art walked every AST node *three* times — `Visit` was derived, but `Display` (hundreds of impls) and `Spanned` (with 179 empty-span holes) were hand-written and drifted. A field added to a node could silently drop from `Display` (round-trip corruption) or `Spanned` (lost span). This is the **linchpin**: the `Other(X)` seam (ADR-0009) and both renderers (ADR-0010) hard-depend on generated walks.

## Decision

An external **`crates/squonk-sourcegen` xtask** (`syn`/`quote`/`proc-macro2`, dev-only) parses the *hand-written, annotated* AST (`#[ast]`/`#[span]`/`#[sql]`) into one schema and emits **checked-in `.rs`**. **Hybrid division:** stock `#[derive]`s for uniform per-type traits (`Clone`/`Debug`/`PartialEq`+`Eq`+`Hash`/serde/`Arbitrary`, feature-gated); the **xtask for cross-type / large / inspectability-critical walks** — `Spanned` (union over *all* fields → holes impossible), `Visit`/`VisitMut` (the master trait), the `Render` skeleton + `<X>` threading, and per-enum size-assert blocks. Drift gate: a plain `ensure_file_contents` `#[test]` that regenerates in-memory and fails under `cargo nextest run` if the checked-in file is stale.

## Consequences

- **Why xtask over a proc-macro:** derives can't see cross-type facts (a master visitor, the render-skeleton's exhaustive dispatch); checked-in output is inspectable/reviewable/debuggable; generation runs occasionally (not every build); and the published crate ships **no** `syn`/`quote` in its dep tree (ADR-0017). The proc-macro's one edge — no-drift-by-construction — is neutralized by the drift test.
- **Why not `ungrammar`:** it generates the AST *types* from a grammar DSL, forfeiting the layout control (boxing, `#[repr]`, `Ident` shape) secured in ADR-0007. We keep the types hand-written and generate the *walks*.
- This is the **#1 schedule risk**: it must exist *before* the AST grows past its first real node — built early (Phase 0), starting tiny (`Spanned` + `Visit`) and growing to the render skeleton.
- Convergent prior art: rust-analyzer, oxc, swc all use checked-in external codegen for the bulk.

## Interconnects

- code: `crates/squonk-sourcegen/src/schema.rs` — `struct_has_meta`/`enum_has_meta` convention; `crates/squonk-ast/src/generated/size_asserts.rs`
- invariant: node classification is convention-based on a `meta: Meta` field, with no `#[ast]`/`#[span]`/`#[sql]` markers; the generated files are byte-identical to a fresh regeneration.
- xtask: none — enforced by the `generated_files_are_up_to_date` drift test; `cargo xtask deps` keeps `syn`/`quote` off the published surface.

## References

Atom A28. rust-analyzer `sourcegen`/`ensure_file_contents`; oxc `ast_tools`; swc generated code.

## Amendment (2026-06-30): node classification is convention-based; the `#[ast]`/`#[span]`/`#[sql]` markers were not built

The Decision above describes the generator reading a *hand-written, annotated* AST via `#[ast]`/`#[span]`/`#[sql]` markers. Those attributes were never implemented, and a conformance audit (`prod-sourcegen-ast-annotations`) surfaced the gap. After strategic review the **convention-based classification the generator actually uses is recorded here as the ACCEPTED mechanism** — the markers are deliberately *not* added. The signals below are what `crates/squonk-sourcegen/src/schema.rs` reads from the node text, so this section is self-contained against the code.

**What the markers would have made declarative, and what the convention reads instead:**

- **Node membership (`#[ast]`).** A type is a *spanned node* — and gets a generated `Spanned` impl — when it is a struct with a `meta: Meta` field, or an enum whose *every* variant carries a `meta: Meta` field. The single hand-seed is `ObjectName`, the one node that wraps `ThinVec<Ident>` rather than carrying `meta` (its span is the union of its identifier parts). The wider *node graph* that receives the `Visit`/`VisitMut`, node-id, and render-skeleton walks is the spanned set plus each generic's extension default (`NoExt`, the stock `X` — a real part of the AST the `Other(X)` seam walks through the generic), closed under field references by fixpoint so the tag/leaf enums a node field names (`BinaryOperator`, `DataType`, `LiteralKind`, …) are pulled in. Types never reached this way — e.g. the `LiteralValueError` accessor-error types — are *not* AST nodes and get no generated walk. The `meta: Meta` field is the same fact that already defines a node's identity (ADR-0002), so classification reuses an existing invariant instead of maintaining a parallel annotation.

- **Span fields (`#[span]`).** A field contributes to a node's span by *type*, not by annotation: its core type (seen through a transparent `Box`) must be in the spanned set or be an `Extension`/`Spanned`-bound generic parameter. A `Vec`/`ThinVec`/`Option` of such a type folds with `Span::union`; a scalar of one is taken directly. Operator/tag enums, `bool`, `Symbol`, and the like carry no span and are skipped.

- **Render fragments (`#[sql]`).** There is nothing to declare. The generated render *skeleton* binds every field and variant exhaustively and pins a render-shape fingerprint, but the render text stays hand-written (ADR-0010). The skeleton's only job is to fail to compile — or fail the drift test — when an AST shape changes under a stale renderer, so no per-node render fragment is annotated. The fingerprint is **partitioned one const per AST source file** — `CURRENT_RENDER_SHAPE_<FAMILY>` for each `crates/squonk-ast/src/ast/<family>.rs` (`_EXPR`, `_QUERY`, `_DDL`, `_TY`, `_LITERAL`, …) — keyed by the file each node type is defined in, the AST author's own module boundary rather than a hand-maintained grouping map that could drift. Every struct/enum in a file feeds that file's hash, so the union still fingerprints the whole AST shape; the split only changes *which* const moves when a shape changes, so a landing that touches only `ast/expr.rs` moves `CURRENT_RENDER_SHAPE_EXPR` alone. Two agents editing disjoint files move disjoint pin lines in `render/nodes.rs` and merge without conflict, while a same-file collision still conflicts loudly on that file's single shared pin — the property that unblocks parallel AST work (see the Amendment below). The skeleton module and its fingerprint pins are `#[cfg(test)]`-gated (in the generated `generated/mod.rs` and the hand-written `render/nodes.rs`), so the compile-time fingerprint check runs in test builds while product builds omit the skeleton's thousands of dead lines; the `generated_files_are_up_to_date` drift test enforces regen either way.

**Why accept the convention rather than build the markers:**

- **The convention is already fail-loud, so markers would guard nothing new.** The `generated_files_are_up_to_date` drift test (the `ensure_file_contents` gate this ADR mandates) regenerates in memory and fails `cargo nextest` the moment a checked-in walk is stale, and the generator *panics at generation time* for the exact holes the markers were meant to close: a spanned enum whose variants do not all carry `meta` (an enum node must be addressable directly, not reconstruct its span from children), and a field that reaches an AST node through a container shape the classifier does not emit — a tuple/array/map — which is the "empty-span / un-visited-node hole" the Decision calls impossible. The markers' one edge, declarativeness at the node, buys no guarantee the drift test and these panics do not already provide.

- **Markers would add a redundant source of truth and the machinery to police it.** `#[ast]`/`#[span]`/`#[sql]` would each have to *agree* with the `meta` field and the field types the generator already reads, so they introduce a second fact that can itself drift (an `#[span]` on a non-spanned field, a missing `#[ast]`) plus the validation code to catch that drift. That is machinery for declarativeness the project does not need, and it cuts against the minimal, compact-codebase value proposition (ADR-0017) — the same reasoning that kept the extension seam coarse in ADR-0009's 2026-06-30 amendment. The two hand-seeds the convention carries (`ObjectName`, the `NoExt` extension default) sit in one place in `schema.rs` and are far cheaper than an annotation system spanning every node.

Net: the `#[ast]`/`#[span]`/`#[sql]` markers in the Decision are **withdrawn**; the convention in `crates/squonk-sourcegen/src/schema.rs`, enforced by the drift test and the generation-time panics, is the accepted node-classification mechanism.

## Amendment (2026-07-06): the render-shape fingerprint is split one const per AST source file

The render-shape fingerprint began as a single `CURRENT_RENDER_SHAPE` const covering the whole AST. That made the const a global serialization point for parallel work: two agents adding *unrelated* nodes (a new `Expr` variant vs. a new `CreateTable` option) both had to re-pin the same one line in `render/nodes.rs`, so their branches conflicted on the pin even though their AST edits were disjoint — forcing one-fingerprint-owner-at-a-time scheduling, the throughput ceiling for the AST-heavy planner-parity era.

**Decision:** sourcegen emits one fingerprint const per **AST source file** — `CURRENT_RENDER_SHAPE_<FAMILY>` for each `crates/squonk-ast/src/ast/<family>.rs` — and `render/nodes.rs` pins each. The partition is the file a node type is defined in (`schema.rs` records the file stem per parsed item in `Schema::families`); `render_skeleton` buckets `schema.items` by family and hashes each bucket with the same FNV feed as before. Choosing the *file* boundary — the author's own module partition — over a hand-curated grouping keeps the partition mechanically derived and drift-free: a new node lands in whatever file the author writes it in and is fingerprinted there automatically, with no grouping map to maintain. The current partition is the fourteen node-bearing files: `dcl`, `ddl`, `dml`, `expr`, `ext`, `ident`, `literal`, `pivot`, `query`, `stmt`, `tcl`, `ty`, `util`, `window`.

**What is preserved.** Every struct/enum still feeds exactly one family hash (reachable render node or not), so the union of the family hashes fingerprints the whole AST shape exactly as the single hash did — no shape change can escape all pins. The compile-error-carries-two-hashes property is per-family: a shape change regenerates only its file's const, so only that family's `const _` pin fails to compile, carrying that family's old and new hash. The regenerate-audit-copy procedure at the pin site (rerun sourcegen → audit that family's hand-written `Render` impls against the regenerated skeleton → copy only that family's hash) is unchanged except that it now names one family instead of the whole AST.

**What it buys.** A landing that edits one `ast/*.rs` file moves one pin line; two agents editing disjoint files move disjoint pin lines and merge cleanly, while two agents landing shapes in the *same* file still collide loudly on that file's single shared pin (the drift alarm is not weakened, only localized). This removes the fingerprint as a global serialization point without adding a maintained partition artifact.
