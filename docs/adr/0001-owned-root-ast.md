# ADR-0001: Owned-root AST & source ownership

- **Status:** Accepted (2026-06-26)
- **Atoms:** A1

## Context

An embeddable SQL parser's AST is held and pattern-matched by many *independent* consumers (query engines, planners, rewriters, linters, formatters), and a downstream rewriter mutates it in bulk. How the AST owns or borrows the source text is the foundational decision — it determines whether a lifetime parameter infects every public type.

## Decision

**Owned-root, `'static` AST.** The parse result `Parsed` is the *sole* holder of the source, behind a `SourceStore: Clone + Deref<Target = str>` trait, shipped with both `Arc<str>` (default) and `Rc<str>` impls. The generic is confined to `Parsed<S = Arc<str>>` — the ~hundreds of node types are **non-generic**, store only `Span` + interned `Symbol`, and never carry a lifetime. The tokenizer works with `&'a str` slices internally, but that lifetime never escapes into the AST.

## Consequences

- No viral lifetime on every type; the AST is `Send` + `'static` (with `Arc`), so it crosses threads and caches cleanly.
- **Rewriting works**: a synthesized node has no text in the source, which a borrowed AST literally cannot represent.
- A node *detached* from its root needs a render context to render (see ADR-0010) — the accepted cost of interning + byte-range literals.
- Three consumption tiers fall out for free: `Parsed<Arc>` (Send/shared), `Parsed<Rc>` (single-thread, cheapest refcount), bare `Vec<Statement>` (Send, structure-only, no source).

## Alternatives considered

- **Lifetime-threaded `Ast<'a>` (bumpalo/oxc model)** — rejected: the viral `'a` taxes every consumer, and a borrowed AST can't represent rewritten nodes. oxc tolerates it only because it owns the entire toolchain.
- **Feature-gating the ownership model** — rejected: lifetimes are not an additive feature axis; a feature that adds/removes `<'a>` forks the public API into two incompatible universes.

## Interconnects

- code: `crates/squonk-ast/src/vocab/mod.rs` — `SourceStore`, `Resolver`; `crates/squonk/src/parser/parsed.rs` — `Parsed`
- invariant: AST node types carry no lifetime parameter, and `Parsed` defaults its store to `Arc<str>`, so the owned tree is `'static` and never threads a transient borrow into a node.
- xtask: none — enforced by the no-lifetime AST shape and the `SourceStore: 'static` supertrait bound.

## References

Atom A1. Wave 2 performance brief; precedent: rustc, rust-analyzer, swc, ruff (all owned-`'static` + byte-range spans + interning).
