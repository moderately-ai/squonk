# Stable Rust API policy

This policy covers the two published Rust crates, `squonk-ast` and
`squonk`. It starts at the `v1.0.0` tag. Each major line uses its `.0.0` release
tag as the immutable compatibility baseline; the checked configuration in
`release/semver-baseline.toml` names the current baseline. Package paths are used
there so crate-name changes do not invalidate the gate.

## Reviewed surface

The stable Rust surface includes every public item reachable from either crate,
the names and dependency relationships of Cargo features, and implementations
of public traits. The compatibility check builds all features because dialect
presets, serde support, and document rendering all expose public API. The
default ANSI-only build remains a supported subset, so a change that only
breaks default or another partial feature combination is still a breaking
change even when the all-feature check misses it.

The AST is designed for direct inspection and rewriting. Its enums are
therefore exhaustive by default: downstream consumers may use complete matches,
and adding or changing a variant requires a major release. Only types explicitly
marked `#[non_exhaustive]` reserve additive growth for a minor release. The
inventory is exactly nine. Six are AST/dialect enums: `Statement`,
`CommentTarget`, `BuiltinDialect`, `LexicalConflict`,
`FeatureDependencyViolation`, and `GrammarConflict`. Three are error/diagnostic
kind enums that form the deliberate pre-1.0 error-evolvability seam (ADR-0005):
`ParseErrorKind` (a growth axis for robustness guards and carried lexical
categories), `LexErrorKind` (new dialect scanners add lexical faults over time,
and it is now carried onto `ParseErrorKind::Lexical` and mapped to a bindings
machine kind), and `BindingTokenKind` (an output-only serialize view of the
evolving tokenizer vocabulary, so a new token category stays additive on both the
Rust and wire surfaces). The six typed `Other(X)` extension seams do not make
their containing enums implicitly non-exhaustive.

Public fields are part of the construction and matching API. Renaming a field,
changing its type, making it private, adding a field to an exhaustive struct
that downstream code can construct, or tightening a public generic bound is a
breaking change. `ParseError` is the deliberate exception: it carries a private
`hint` field (ADR-0005), which closes downstream struct-literal construction to
the crate's own constructors and forces downstream matches to end in `..`. That
one private field is the diagnostics-growth seam — any later field (a hint, a
structured label) is absorbed by the rest pattern, so the struct can grow in a
minor release, while its public `span`/`kind`/`expected`/`found` fields stay
directly readable and matchable. Public traits require the same care: adding a
required item or tightening a supertrait bound is breaking. In particular,
ADR-0009 permanently defines `Extension` as a blanket-implemented bound alias, so
it can never gain a required item or a specific implementation.

The serde wire representation is a separate compatibility surface governed by
the serialized-AST schema contract in [`schema-contract.md`](schema-contract.md),
which carries its own `WIRE_SCHEMA_VERSION` and `release/schema/` baseline.
Passing this Rust API gate does not prove wire compatibility.

## Evolution rules

- Patch releases fix behavior and documentation without removing, renaming, or
  changing public API.
- Minor releases may add functions, methods with default implementations,
  optional Cargo features, and variants or fields only where the type is
  explicitly non-exhaustive. They may deprecate existing API.
- Major releases may make breaking changes. Each intentional break must be
  called out in the changelog with a migration path.
- A deprecated item remains available for at least one minor release. Removal
  requires the next major release; a soundness or security exception must be
  documented in the release notes.
- Removing or renaming a Cargo feature is breaking. Adding an optional feature
  is minor. Changing default features is reviewed as a breaking change because
  it changes downstream builds even when Rust names remain available.
- The workspace `rust-version` is compatibility metadata and a supported floor,
  not just a declaration. Raising it is a **minor-release event**: it requires a
  minor version bump and a changelog entry, because it can break a downstream
  build that pins the old floor. CI enforces the promise — the `msrv` job in
  `.github/workflows/ci.yml` compiles both published crates at exactly the
  declared `rust-version` on every PR (reading the floor from `Cargo.toml`, so
  there is one source of truth), and fails if any code uses an API or language
  feature newer than that floor. This is a compile-time backstop for
  `clippy::incompatible_msrv`, which only partially covers stdlib APIs. Note this
  is distinct from the API-compatibility gate below, which runs on the local
  stable toolchain and does not install or rebuild under historical Rust
  toolchains.

## Compatibility gate

Run:

```console
cargo xtask semver
```

While the workspace is 0.x, the command validates the checked baseline manifest
and reports that comparison is pending. At major `N >= 1`, it requires the
configured `vN.0.0` Git tag and an installed `cargo-semver-checks`, then compares
both published crates against that tag with all features enabled. The xtask also
requires the configured tag to match the workspace major, so retaining an older
major's baseline cannot turn the checks into a vacuous major-version comparison.
Install the external tool with `cargo install cargo-semver-checks --locked`; it is
release tooling, not a workspace dependency.

`cargo-semver-checks` is a guard, not the policy. Review its report together
with this document because automated analysis does not cover every generic,
type-signature, partial-feature, behavioral, or serialized-schema break. An
intentional failure is resolved by choosing the required version bump, not by
silencing the lint without a written exception.
