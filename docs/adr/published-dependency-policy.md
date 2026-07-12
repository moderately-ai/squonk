# Published dependency policy

Companion to [ADR-0017](0017-engineering-policies.md) (dependency minimalism). It records exactly what the *published* crates — `squonk` and `squonk-ast` — are allowed to ship at runtime, why each dependency is justified, and the local gate that keeps the surface from drifting. Every other workspace crate (`squonk-sourcegen`, `squonk-bench`, `squonk-conformance`, `squonk-conformance-fuzz`, `xtask`) is `publish = false` and may depend on whatever it needs — those deps never reach a downstream embedder.

## The published runtime surface

`cargo tree --edges normal` (runtime edges only — it excludes dev-dependencies) on each published crate:

```
$ cargo tree -p squonk-ast --edges normal
squonk-ast v0.1.0
└── thin-vec v0.2.18

$ cargo tree -p squonk --edges normal
squonk v0.1.0
├── squonk-ast v0.1.0
│   └── thin-vec v0.2.18
└── thin-vec v0.2.18
```

The entire external runtime surface is a single leaf crate, `thin-vec`. The published crates declare no build-dependencies and no dev-dependencies of their own, so `cargo tree --edges normal,build,dev` adds nothing to the trees above.

## Each published dependency

| Dependency | Direct purpose | Transitive cost | ADR-0017 justification |
|---|---|---|---|
| `thin-vec` 0.2 (external) | One-word (8 B) child-sequence container used for AST node child lists, versus `Vec`'s three words / 24 B (ADR-0007). | **Zero** transitive dependencies — a true leaf (its `Cargo.lock` entry has no `dependencies`). Adds one small crate, no subtree. | A sanctioned leaf, exactly the case ADR-0017 permits (`memchr`/`thin-vec`/`smallvec`/`unicode-ident`) over a subtree-dragging crate (`phf` → `phf_shared` + `siphasher`). Its `unsafe` is encapsulated inside the crate, so `squonk`/`squonk-ast` honour the workspace `unsafe_code = "deny"` lint and stay `unsafe`-free. |
| `squonk-ast` (internal path crate) | The shared, dialect-agnostic AST that `squonk` parses into and renders from. | First-party path + version dep; transitively contributes only `thin-vec`. | A published workspace crate, not an external dependency — it carries the same policy and is itself audited by this gate. |

## Dev / test / sourcegen isolation (leak check)

The intentionally broad dev/test/sourcegen surface is confined to `publish = false` crates and `[dev-dependencies]`, and none of it appears in the published normal-edge trees above:

- `squonk-sourcegen` — the codegen toolchain (`syn`, `quote`, `proc-macro2`, `prettyplease`); `publish = false`.
- `squonk-conformance` — the differential/oracle stack (`pg_query`, `proptest`, `datadriven`, `insta`, `arbitrary`, and `bolero` as a dev-dependency); `publish = false`.
- `squonk-bench` — the measurement stack (`codspeed-criterion-compat`, `dhat`, `gungraun`, plus the measured rejected alternatives below); `publish = false`.
- `squonk-conformance-fuzz` — the libFuzzer targets (`libfuzzer-sys`); a standalone workspace, `publish = false`, never pulled into the top-level build.
- The rejected dependencies kept as measured backlog alternatives (ADR-0004/0005/0017) — `phf` (+ `phf_shared`/`siphasher`/`phf_codegen`), `logos`, and upstream `sqlparser` — resolve **only** under `squonk-bench`, and `phf` only behind its off-by-default `phf-compare` feature. `lasso` and `text-size` are absent from the resolved graph entirely.

**Verdict: compliant.** No dev/test/sourcegen dependency leaks into the published runtime surface; the published surface is `thin-vec`-only.

## The regression gate

`cargo xtask deps` enforces the allowlist locally and fails if the published surface drifts. It also runs inside `cargo xtask tidy` and as the `published_deps_*` `#[test]`s under `cargo nextest run` (a live `#[test]` re-checks the real workspace), keeping the gate local-runnable per ADR-0017. Two layers prove `published runtime surface ⊆ allowlist` without a full graph engine:

1. **Direct (manifests).** Each published crate's `[dependencies]` / `[build-dependencies]` may name only another published workspace crate or a crate on the allowlist (`thin-vec`). `[dev-dependencies]` are ignored — they never ship. A stray `regex = "1"` on `squonk` fails here.
2. **Closed-set (`Cargo.lock`).** Every allowlisted external crate must itself pull only allowlisted crates (its lock `dependencies` ⊆ the allowlist), so an allowlisted leaf that sprouts a subtree upstream (a future `thin-vec` that grew a dependency) also fails.

The allowlist is `PUBLISHED_DEP_ALLOWLIST` in [`xtask/src/lib.rs`](../../xtask/src/lib.rs). Adding a name to it is a deliberate ADR-0017 decision — weigh the transitive tree, prefer a leaf over a subtree-dragging crate, and record the purpose + transitive cost + justification in the table above.

### Why not cargo-deny?

`deny.toml` (cargo-deny) owns the whole-graph supply-chain checks — advisories, licences, sources, and duplicate versions across every crate, dev-deps included. It cannot express the published-surface allowlist: its bans apply graph-wide and cannot be scoped to one crate's normal-edge subtree, so a graph-wide allowlist would also have to enumerate every dev/bench dependency. The two tools are complementary — cargo-deny for supply-chain across everything, `cargo xtask deps` for the published-surface allowlist.

### How to run

```
cargo xtask deps            # the published-surface allowlist gate
cargo xtask tidy            # all local gates, including the above
cargo nextest run -p xtask  # the same checks, exercised as unit tests
```

To re-audit the surface by hand, regenerate the trees at the top of this document:

```
cargo tree -p squonk --edges normal
cargo tree -p squonk-ast --edges normal
```
