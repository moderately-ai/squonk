# Serialized binding-schema contract

This policy covers the **serialized JSON shape** that the Python (`squonk`) and WASM/npm (`squonk-wasm`) bindings emit and accept. It is the wire-format companion to the stable Rust API policy in [`stable-api.md`](stable-api.md): that document governs the Rust type surface (`cargo xtask semver`), and this one governs the serde wire shape, which the Rust semver gate cannot see. A change that renames a serde field, alters an enum's tag/`flatten` representation, or flips an `skip_serializing_if` omission is invisible to `cargo-semver-checks` but breaks every downstream JSON/dict consumer, so the wire shape carries its own version and its own gate.

## Reviewed surface

Both bindings funnel through one shared serde module, [`crates/squonk/src/bindings.rs`](../crates/squonk/src/bindings.rs), which supplies `Serialize` views over parser-owned data; Python serializes them with `serde_json` to a JSON string, WASM with `serde-wasm-bindgen` to a native JS value. The AST node graph they embed lives in `squonk-ast` and is the only part that also deserializes (`Parsed`, in [`crates/squonk/src/parser/parsed.rs`](../crates/squonk/src/parser/parsed.rs)).

### JSON roots (the checked inventory)

Each root is the top-level value a binding entry point returns. This table is the authoritative inventory; the `release/schema/wire-schema.v{N}.json` snapshot is its machine-checked form (every root is a key, and the drift gate fails if one is added, removed, or reshaped without review).

| Root | Rust type | Emitted by | Notes |
|---|---|---|---|
| Parse result | `ParseDocument` | `parse` (both bindings) | Flattens `Parsed` (`#[serde(flatten)]`), adds `dialect`, optional `trivia`, `resolver`. |
| Recovering parse result | `RecoveredDocument` | `parse_recovering` | Flattens `ParseDocument`, adds `errors: [ParseDiagnostic]`. |
| Tokenizer output | `TokenizeDocument` | `tokenize` | `tokens: [BindingToken]`, optional `trivia`. |
| Dialect metadata | `[DialectInfo]` | `supported_dialects` | `{ name, aliases }`. |
| Diagnostic | `ParseDiagnostic` | error path (thrown as `SqlParseError`, and inside `errors`) | `message`, `kind`, optional `span`/`span_start`/`span_end`/`expected`/`found`. |
| AST root | `Parsed` | embedded in `ParseDocument` | `{ source, symbols, string_literals, statements }`; the only **deserialized** root. |

Component types reachable from those roots — and therefore part of the contract — are `Statement<X>` and the whole AST node graph (`Expr`, `Query`, `DataType`, …, in `squonk-ast`), `ResolverMetadata`, `KeywordSymbol`, `BindingToken`, `BindingTokenKind` (internally tagged on `kind`), `BindingTrivia`, `SourceSpan`, and the per-node `Span`/`Symbol`/`NodeId`/`Meta` leaves.

### Representation facts worth pinning

The AST enums use serde's **default external tagging** and verbatim Rust field names (no `rename`/`rename_all` anywhere in `squonk-ast`), so a Rust field rename is a wire rename. `BindingTokenKind` is the one **internally tagged** enum (`tag = "kind"`). `#[serde(flatten)]` merges `Parsed` into `ParseDocument` and `ParseDocument` into `RecoveredDocument`. Several diagnostic and trivia fields use `skip_serializing_if = "Option::is_none"`, so their **presence** is load-bearing: a consumer must treat an absent optional field and a `null` field identically.

### Structural view (already gated)

The exhaustive structural schema of the AST node graph is the generated TypeScript/Python view — [`crates/squonk-wasm/js/ast.generated.d.ts`](../crates/squonk-wasm/js/ast.generated.d.ts), `ast-metadata.generated.js`, [`crates/squonk-python/python/squonk/ast.py`](../crates/squonk-python/python/squonk/ast.py), and `_ast_metadata.py` — emitted by `squonk-sourcegen` from the AST type text. Any AST serde field rename, add, or removal changes those files and is caught by the sourcegen `generated_files_are_up_to_date` drift test (ADR-0013). This contract's own snapshot (below) covers the **envelope** types in `bindings.rs`, which sourcegen does not walk, plus a concrete serialized example of every root.

## Version

The single wire-schema version is `WIRE_SCHEMA_VERSION` in [`crates/squonk/src/bindings.rs`](../crates/squonk/src/bindings.rs). It is deliberately independent of the crate/package version (`CARGO_PKG_VERSION`, surfaced as `version()` / `__version__`): a patch release can change code without touching the wire, and a wire break need not coincide with a major crate bump. Both bindings expose it beside their package version — `schemaVersion()` in the WASM binding (`squonk_wasm::schema_version`), `__schema_version__` in the Python module.

### Pre-release convention (before the first published release)

`WIRE_SCHEMA_VERSION` is held at `1` and stays there until the first published release. Until then the schema is **unpublished with zero external consumers**, so the version number cannot mean anything to anyone: a pre-first-publish shape change reshapes v1 in place — regenerate the snapshot and, if the deserialized surface changed, rewrite `compat/parsed.baseline.json` — but do **not** bump the version. There is no prior wire to stay compatible with while nothing has shipped.

The Compatibility rules and the version-bump procedure below apply **from the first published release onward** — the moment a real consumer can pin `schema_version == 1`, v1 is frozen and every subsequent breaking change bumps and preserves the prior baseline as specified. In short: before first publish the bump rule is dormant and the drift gate exists only to force a deliberate, reviewed snapshot regeneration; after first publish the full versioning contract is live.

## Compatibility rules

The default direction is conservative: the wire shape is a public interface, and any change to it is reviewed against these rules before it lands.

- **Compatible (no version bump)** — additive-optional growth: a new field carrying `#[serde(skip_serializing_if = "Option::is_none")]` (or otherwise defaulted on the read side), or a new variant on a type that is already `#[non_exhaustive]`. Old consumers ignore fields they do not know and continue to parse. These regenerate the snapshot but keep `WIRE_SCHEMA_VERSION`.
- **Breaking (version bump required)** — a renamed or removed field; a retyped field; a changed enum representation (external ↔ internal tagging, adding/removing `flatten`, a changed `tag` key); a changed omission behaviour (adding or removing `skip_serializing_if`, or making a previously optional field required); or a new required field on a type a consumer constructs. These bump `WIRE_SCHEMA_VERSION` and follow the bump procedure below.

Adding a serde `rename`/`rename_all`/`untagged` attribute anywhere on an AST node or envelope type is a breaking change by definition and must bump the version — the generated TypeScript view reads Rust field names, so a `rename` silently desyncs the view from the JSON.

## Authoritative snapshot

Two artifacts live under [`release/schema/`](../release/schema), beside `release/semver-baseline.toml`:

- `wire-schema.v{N}.json` — a canonical, key-sorted serialization of every JSON root for schema version `N`. It is regenerated on every reviewed shape change and byte-drift-gated by the `wire_schema` integration test (the wire analogue of the sourcegen generated-bytes gate). The values are minimal representative instances chosen to pin field names, nesting, enum representation, and omission behaviour without embedding churny data (for example, the resolver's ~700-entry keyword table is pinned by shape with two entries, not dumped in full).
- `compat/parsed.baseline.json` — a **frozen** `Parsed` document authored against the first schema version. It is written once and never rebaselined; the compat test deserializes it under the current code, so an additive change keeps it loading (proving forward-compatibility of old documents) while a breaking representation change fails it and forces a bump.

## Drift gate

The `wire_schema` test (`crates/squonk/tests/wire_schema.rs`, `serde` feature) enforces the contract and runs in the `nextest-schema` preflight lane (`cargo nextest run -p squonk --features serde`). It has three parts: the snapshot must be byte-identical to a fresh serialization; the snapshot's recorded `schema_version` must equal `WIRE_SCHEMA_VERSION`; and the frozen baseline must still deserialize. A shape change fails the first with a message that classifies the change and points here; an out-of-date version fails the second; a breaking change to the deserialized surface fails the third.

## Version-bump procedure (shared by Rust, Python, and npm)

One procedure, one version number, for all three ecosystems. The wire shape is produced by the shared Rust `Serialize` impls, so a single bump propagates to every binding.

1. **Classify** the change against the compatibility rules above. Additive-optional changes skip to step 4 (regenerate, no bump).
2. **Bump** `WIRE_SCHEMA_VERSION` in `crates/squonk/src/bindings.rs`. This is the single source of truth; `schemaVersion()` (WASM/npm) and `__schema_version__` (Python) both read it, so both bindings advertise the new version automatically.
3. **Preserve the prior baseline.** Leave the existing `release/schema/wire-schema.v{old}.json` and `compat/parsed.baseline.json` in place as the immutable record of the superseded version; the new version writes a new `wire-schema.v{new}.json`.
4. **Regenerate** the snapshot: `UPDATE_SCHEMA_SNAPSHOT=1 cargo nextest run -p squonk --features serde wire_schema`. Review the JSON diff to confirm it matches the intended change and nothing else moved.
5. **Note the change** in the release notes with a migration path, exactly as a breaking Rust API change is called out under `stable-api.md`. Consumers branch on `schemaVersion()` / `__schema_version__` to handle both shapes across the transition.

Because Python and npm both wrap the same serialized bytes, there is nothing per-ecosystem to bump beyond their package versions on release; the wire contract itself is versioned once, here.
