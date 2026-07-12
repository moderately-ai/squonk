# ADR-0011: Dialect-as-data — FeatureSet, canonical AST, "generic" = ANSI

- **Status:** Accepted (2026-06-26)
- **Atoms:** A25, A26

## Context

The prior art modelled dialects as a 184-method `: Any` god-trait plus 128 `dialect_of!` TypeId checks — boolean-soup *and* TypeId-soup, gating 112 code paths unreachable to any custom dialect. Separately, its AST sometimes parsed the *same* syntax to *different* trees across dialects.

## Decision

- **A dialect is a `const` DATA value.** `FeatureSet` is a struct of typed fields (bools + small enums: `Casing`, `NullOrdering`, `IdentifierQuote`, … + the per-dialect bp/reserved/class tables), pure data in `squonk-ast`, with `const` presets (`FeatureSet::POSTGRES`, `::ANSI`, …) and a builder; a custom dialect is `BASE.with(delta)`. The parser reads `self.features.<field>` (const-folds under `Parser<D>`). The thin `Dialect` trait is `fn features() + type Ext` (ADR-0009) + a *small* set of finer typed hooks with **3-state `Handled(T) | NotHandled | Err`** returns. No `: Any`, no `dialect_of!`.
- **Self-describing feature metadata** (stable id + maturity) so the coverage matrix (ADR-0015) is exhaustively enumerable.
- **Canonical-AST policy:** one canonical *shape* per construct **+ an always-present compact surface-syntax tag** where dialects spell the same meaning differently (e.g. `Limit { offset, count, syntax: Limit|FetchFirst|Top }`). The shape is *semantics*, anchored on the SQL-standard's model where it defines a construct, the common cross-dialect shape where it is silent — **never** a single dialect's surface. `FeatureSet` gates *acceptance*, not shape; one-shape-per-syntax consistency is non-negotiable (it fixes the "same JOIN → two shapes" bug).
- **"Generic" is not a vibe-union.** It is the **ANSI/Standard** baseline (`FeatureSet::ANSI`, anchored on the SQL:2016 BNF). An optional *explicitly-defined* `FeatureSet::LENIENT` (documented union + conflict-resolutions) is the honest "parse anything" tooling mode — never called "generic".

## Consequences

- The surface tag is lossless *and* canonical *and* maintainable: engines/transpilers/rewriters see one shape per meaning; formatters read the tag for exact-spelling round-trip — serving the whole consumer spectrum from one AST.
- The anti-`dialect_of!` ban is enforced by a **local `tidy` xtask/`#[test]`** (ADR-0017), not CI-pipeline logic.

## Interconnects

- code: `crates/squonk-ast/src/dialect/ansi.rs` — `FeatureSet::ANSI`; `crates/squonk/src/dialect/builtin.rs` — `BuiltinDialect::from_name`
- invariant: a dialect is `const` `FeatureSet` data; the runtime name `"generic"` resolves to `FeatureSet::ANSI` (the strict standard baseline) with no separate `Generic` preset — `Lenient` is the only explicit permissive union.
- xtask: `cargo xtask dialect-generic`; `cargo xtask dialect` (bans `dialect_of!`/TypeId dispatch).

## References

Atoms A25–A26. Calcite `SqlConformance`, ZetaSQL `LanguageFeature`, sqlglot dialect attributes.
