# ADR-0010: Rendering — ctx-carrying renderer & render modes

- **Status:** Accepted (2026-06-26); amended 2026-06-30 — the debug-SQL mitigation shipped as an opt-in, explicit-resolver `debug_sql` helper, not the floated thread-local resolver (see Amendment).
- **Atoms:** A23, A24

## Context

Because nodes are non-self-rendering (an `Ident` is a `Symbol`, a literal is a `Span` — ADR-0003/0006), the obvious `impl Display for Ident` cannot work: `Display::fmt` takes no extra arguments and cannot reach the resolver or source. The renderer signature must be fixed *before* any render body, or retrofitting a context into every impl is the prior-art-style rework we are avoiding.

## Decision

- **A ctx-carrying `Render` trait** (`fn render(&self, ctx: &RenderCtx, f: &mut Formatter)`) plus a **`Displayed<'a>(node, &ctx)` newtype that `impl std::fmt::Display`** for ergonomics (`format!("{}", node.displayed(&ctx))`). `RenderCtx = { Resolver, source: &str, RenderConfig }`. The `Resolver` *trait* lives in `squonk-ast` so the renderer is dependency-free of the interner impl. `std::Display` is reserved for the `Parsed` root bundle (`format!("{parsed}")` works — it owns source + resolver).
- **`RenderMode { Canonical | Parenthesized | Redacted }`** + a target `FeatureSet` in the config. Canonical = round-trip (minimal parens from the bp table); Parenthesized = the precedence oracle (ADR-0014); Redacted = literal/identifier masking → query fingerprinting, PII-free logging, the datadriven golden oracle (ADR-0015).
- **Tier split:** a Tier-1 canonical `Render` (the three modes) in `squonk-ast`; a Tier-2 `Renderer<D>` (dialect overrides + **fallible** emission — loudly reject an unsupported construct, never panic) in the parser crate. Render bodies are codegen-generated (ADR-0013).

## Consequences

- **Cost:** a *bare* node isn't directly `Display`/`to_string()`-able — a detached subtree needs the ctx, and `dbg!(expr)` shows `Debug` (symbols as `u32`), not SQL. This is *intrinsic* to interning + byte-range literals, not a flaw of this signature; rustc/rust-analyzer accept the same. Mitigations: `Debug` works (+ an opt-in thread-local resolver for debug-SQL), `Parsed` is directly `Display`, the `Displayed` wrapper re-enables `Display` at any boundary, and codegen threads the ctx so we write no boilerplate.
- Gains naive `Display` cannot: multi-mode + dialect-target rendering (transpilation), exact literal round-trip, codegen anti-drift.
- The deep-research (ADR-0018) validated this: multiple render modes are *separate printers over one AST*, not a single bidirectional spec; and the reflective-`put` law explains why exact round-trip needs the source (carried on the root).

## Interconnects

- code: `crates/squonk-ast/src/render/mod.rs` — `Render`, `RenderMode`, `debug_sql`
- invariant: `RenderMode` has exactly the three variants (`Canonical`, `Parenthesized`, `Redacted`); the renderer takes its resolver by explicit argument, with no thread-local.
- xtask: none — the render-shape fingerprint is pinned by the render tests.

## References

Atoms A23–A24. rustc/rust-analyzer wrapper-display pattern; sqlglot transpile; Calcite `SqlDialect`.

## Amendment (2026-06-30): the debug-SQL mitigation shipped as an explicit-resolver helper; the floated thread-local was rejected

The Consequences above list, among the mitigations for a bare node not being directly `Display`-able, "an opt-in thread-local resolver for debug-SQL". That was *floated*, not decided. When the mitigation shipped (`prod-render-debug-sql-helper`; `crates/squonk-ast/src/render/mod.rs`) it deliberately took the opposite shape — an explicit, required resolver — and the thread-local was rejected. This section records what was built and why.

**What shipped — an opt-in, explicit-resolver debug adapter.** `RenderExt::debug_sql(&self, resolver: &dyn Resolver)` returns a `DebugSql` `Display` adapter over `RenderCtx::debug(resolver, …)`. It is a strict *addition* beside the canonical path, not a change to it:

- **The resolver is an explicit argument** — never a thread-local, global, or default — so debug rendering can never *silently* pick a resolver; the choice is always written at the call site.
- **A private `ContextKind::Debug` branch** confines the entire behavioural difference to the two resolve chokepoints (`RenderCtx::resolve` and `RenderCtx::slice`); every `Render` body is written once and is unaware of it. The canonical `ContextKind::Canonical` path is byte-for-byte unchanged — still strict (a foreign symbol panics) and still slicing exact source spelling.
- **Tolerant resolution:** a symbol the resolver does not know renders the `<unresolved>` placeholder instead of panicking, so a partially-detached tree prints to completion. The placeholder is deliberately not a valid bare identifier, so it reads as an obvious debug artifact, not a plausible-but-wrong name.
- **No source argument:** the debug path never slices source (a detached node's spans may index a *different* string), so literals fall back to their kind-based spelling and can never emit unrelated bytes.
- **The docs steer to the canonical path first:** `debug_sql` / `RenderCtx::debug` are documented as the *detached-node* fallback; the `Parsed` root's `Display` and the `Displayed` wrapper — which travel with the matched source and resolver — remain the first choice.

**Why the thread-local was rejected.** A thread-local resolver is exactly the hidden global the helper's requirement forbids: it renders a node against whatever resolver the thread last installed, which — for a symbol whose numeric id collides between two parses — is the wrong parse's identifier text, with no signal at the call site. The explicit argument eliminates that class of *silent* mismatch at the API level: choosing a resolver becomes a visible, deliberate act. (An explicit argument still cannot *detect* a resolver from a different parse — a colliding id resolves to the other parse's text — but only the canonical path, which owns its own matched resolver, rules that out entirely; the helper's contract is scoped to detached debugging, not exact rendering.)

Net: the floated thread-local resolver is **withdrawn**; the shipped debug-SQL mitigation is the opt-in, explicit-resolver `debug_sql` / `RenderCtx::debug` adapter in `crates/squonk-ast/src/render/mod.rs`, with the canonical path unchanged.
