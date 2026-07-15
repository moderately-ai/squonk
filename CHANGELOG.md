<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# Changelog

All notable changes to this project are documented here. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) as scoped by the public-API contract in [docs/stable-api.md](docs/stable-api.md).

## [Unreleased]

_No unreleased changes yet._

## [2.0.0] — 2026-07-15

This release makes the deliberately breaking interface cleanup identified after the
first stable publish and coordinates the Rust crates, Python wheel, npm facades, and
npm native platform packages on one major version.

### Added

- **QuiltDB is a first-class stable dialect.** The `quiltdb` Cargo feature,
  `QuiltDb` runtime preset, Python/WASM exposure, and `@squonk-sql/quiltdb` npm
  facade are held to QuiltDB's frozen parser-verdict corpus, integration corpus,
  curated reject set, and structural round trips.
- **Prebuilt Node-API packages cover eight Tier-1 platform triples.** Separate
  `@squonk-sql/native-*` optional packages cover arm64 and x64 on macOS, glibc and
  musl Linux, and Windows MSVC. They are built and smoke-tested independently in
  the release matrix; browser, Deno, workerd, and explicit `/wasm` paths remain
  WebAssembly-only.
- **Wire schema v2 records the 2.x AST contract.** Generated Rust, Python, and
  TypeScript views expose the same transaction spelling and mode information.

### Changed

- **Rust parse configuration is one typed combining form.** `ParseConfig<D>` now
  carries the dialect, recursion limit, trivia capture, and float classification.
  Simple ANSI calls remain `parse(sql)`, `parse_recovering(sql)`, and
  `statements(sql)`; configured calls consistently use `parse_with(sql, config)`,
  `parse_recovering_with(sql, config)`, and `statements_with(sql, config)`.
- **Rust ownership tiers are explicit.** The default parsed document uses `Arc`;
  single-threaded callers can choose `parse_rc` or `parse_rc_with` for an `Rc` root.
- **Python returns ergonomic lazy document views.** Parse results retain their
  Rust-owned representation for metadata and rendering, materializing the typed AST
  only when mapping, traversal, or JSON access requests it. The package ships
  `py.typed`, exhaustive generated AST types, structured diagnostics, and typed
  helpers for tokenization, rendering, redaction, formatting, and transpilation.
- **Node and Bun use prebuilt native bindings automatically.** Consumers call the
  synchronous API without `init()`. Unsupported platforms and `--no-addons` fall
  back to colocated WebAssembly; `runtimeInfo()` exposes the selected backend for
  diagnostics.
- **JavaScript distribution is runtime-aware.** Focused `@squonk-sql/*` packages and
  the `squonk` umbrella now provide conditional Node, Bun, Deno, workerd,
  edge-light, and browser entrypoints. Deno and edge entrypoints use static Wasm
  modules, while browsers use the explicit asynchronous `createSquonk()` factory.
- **Dialect declarations enumerate their complete grammar.** Shipped presets no
  longer inherit hidden fields through struct-update syntax; source generation
  rejects any future production preset that does not remain explicit.
- **Transaction-control grammar is dialect-specific.** PostgreSQL, DuckDB, SQLite,
  and MySQL independently model opener aliases, block words, savepoints, mode
  placement and repetition, chaining, release, and consistent snapshots according
  to their measured engine behavior.

### Fixed

- Fixed DuckDB vertical-tab/comment boundary parity and carriage-return line-comment
  handling found by raw-byte differential fuzzing.
- Fixed SQLite diagnostic classification so quoted parser-error text cannot be
  mistaken for a post-parse resolution failure.
- Fixed unquoted non-ASCII identifier parity for PostgreSQL, MySQL, and SQLite,
  including PostgreSQL's raw high-bit identifier class.
- Fixed DuckDB parse-only parity for bare `DESCRIBE`, which the engine parses before
  rejecting at a later semantic stage.

### Removed

- Removed the Rust 1.x option-specific aliases (`ParseOptions`,
  `parse_with_options`, `parse_with_trivia`, `parse_*_with_builtin_options`, and
  related variants) and the option setters on `Parser`. Migrate by constructing
  `ParseConfig::new(dialect)` and using its builder methods, then pass it to the
  appropriate `*_with` function. Runtime-selected built-ins use
  `parse_builtin_with` or `parse_recovering_builtin_with`.

## [1.0.0] — 2026-07-12 (first stable release)

First stable release of `squonk`, a lightweight, high-performance, multi-dialect SQL parser with a Rust core and Python and TypeScript/WASM bindings. `1.0.0` freezes the public API: from this tag forward, no breaking change to the surface described in [docs/stable-api.md](docs/stable-api.md) lands without a major-version bump, and the machine-checked SemVer gate (`cargo xtask semver`) enforces it against the `v1.0.0` baseline.

### Capabilities

- **13 dialect presets with tiered, documented evidence.** Every shipped preset carries an explicit support tier stating how strong its parity evidence is (see [docs/support-tiers.md](docs/support-tiers.md)):
  - **Stable — engine-differential (bar-A parity):** `postgres` (libpg_query / pg_query 6.1.1, PostgreSQL 17), `mysql` (live mysql 8.4.10 server prepare+parse), `sqlite` (rusqlite 0.40 bundled SQLite, in-process prepare), `duckdb` (libduckdb 1.5.4, in-process `extract_statements`). Each is held to its real engine over vendored regression corpora, with a hard rule that we never accept SQL the engine rejects.
  - **Stable — standard reference:** `ansi` (ISO/IEC 9075:2016 baseline), held by the structural round-trip property, the sqllogictest accept corpus, and a documented PostgreSQL nearest-engine delta ledger.
  - **Preview:** `bigquery` (sqlglot comparison cross-check), `clickhouse` (clickhouse-local 25.5.1 partial oracle, external), `lenient` (constructed permissive union).
  - **Experimental (documentation-derived):** `hive`, `databricks`, `mssql`, `snowflake`, `redshift` — conservative presets whose every enabled feature cites the engine's documentation; engine-oracle acquisition is tracked per preset.
- **Dialects are data, not code.** One parser engine reads a per-dialect feature set of documented flags and enums with machine-checked consistency rules; a custom dialect is a delta on a preset, not a fork of parser code.
- **Owned, `'static`, allocation-lean AST** with byte spans and stable node ids on every node; structured errors with recovery that keeps the good statements and locates the bad ones.
- **Error-type evolvability seam frozen into the surface.** In the pre-publish window (a free change — no consumer can yet pin the API), `ParseError`'s kind enums (`ParseErrorKind`, `LexErrorKind`, `BindingTokenKind`) are `#[non_exhaustive]` growth axes; a widened lexical fault carries its tokenizer category through `ParseErrorKind::Lexical(LexErrorKind)` and both bindings' `ParseDiagnostic.kind` (distinct from `syntax`) instead of collapsing to a syntax error; and `ParseError` reserves a private hint channel (`with_hint`/`hint()`) for post-1.0 diagnostics growth without a major bump. See [docs/adr/0005-tokenizer.md](docs/adr/0005-tokenizer.md) and the exhaustiveness inventory in [docs/stable-api.md](docs/stable-api.md).
- **Rendering surfaces:** canonical, fully-parenthesized, and PII-redacted output; render-for-a-target-dialect; one-call transpile between two dialects.

### Distribution surfaces

- **Rust crates** (crates.io): `squonk` (parser) and `squonk-ast` (dialect-agnostic AST). Default build is ANSI-only with a single micro-dependency (`thin-vec`); every other dialect and `serde` are opt-in features.
- **Python wheel** (`squonk` on PyPI, via `maturin`): `cp311-abi3`, one stable-ABI wheel per platform on CPython >= 3.11. v1 wheel matrix is Linux `x86_64` (manylinux2014), macOS `arm64` (macos-14), macOS `x86_64` (macos-15-intel), Windows `x86_64`, plus an sdist — all built and install-smoked on native runners.
- **TypeScript / WASM packages** (npm): six focused `@squonk-sql/*` dialect packages plus the batteries-included `squonk` umbrella. Node imports initialize synchronously; browser subpaths expose the explicit async factory. Each package is pure Rust/WASM and requires no C toolchain.
- **`serde` AST wire schema v1** (`release/schema/wire-schema.v1.json`): the stable, drift-gated serialization contract. **The first publish freezes it** — per the documented pre-release convention, the schema stayed `v1` through development and crystallizes as immutable at this release; a breaking wire change becomes `v2`.

### Performance and leanness (see [docs/performance.md](docs/performance.md))

- Added deterministic instruction/allocation regression gates, adversarial scaling suites, and direct `datafusion-sqlparser-rs`/`libpg_query` engineering comparisons.
- Added a frozen, oracle-qualified publication workload and public-package adapters. Current results, comparison boundaries, uncertainty, and limitations are maintained in the performance report rather than copied into release notes.

### Platform support

- **Windows `x86_64` promoted to Tier 1 (tested).** The `cross-os` lane in `platform.yml` now runs the full core-crate `cargo nextest` suite on `windows-latest` on every landing, alongside macOS `arm64` and Linux `x86_64`; a Windows test regression blocks the release. This is a release-notes-worthy tier promotion per the [platform-support](docs/platform-support.md) Evolution rule. See that document for the full Tier 1/2/3 matrix across all three published surfaces.

[Unreleased]: https://github.com/moderately-ai/squonk/compare/v2.0.0...HEAD
[2.0.0]: https://github.com/moderately-ai/squonk/compare/v1.0.0...v2.0.0
[1.0.0]: https://github.com/moderately-ai/squonk/releases/tag/v1.0.0

---

## Changelog policy

- **Format.** [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/). Entries are grouped under a version heading with `Added` / `Changed` / `Deprecated` / `Removed` / `Fixed` / `Security` subsections as needed; an `[Unreleased]` section at the top accumulates changes between releases.
- **Versioning.** [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html), scoped by [docs/stable-api.md](docs/stable-api.md): the public API surface named there is what the major version protects. Additive dialect coverage and newly-accepted syntax are minor or patch changes; a breaking change to the public API or the `serde` wire schema is a major bump. The `serde` wire schema carries its own `v1`/`v2` marker inside the major line.
- **Cadence.** Release-driven, not calendar-driven. A release is cut when a coherent batch of user-facing change has landed and the full gate stack plus a rehearsed release-candidate is green; there is no fixed schedule. Security fixes are released as soon as a fix is verified (see [SECURITY.md](SECURITY.md)).
- **Ownership.** The maintainer (Moderately AI) owns release notes and version choice. Each release entry is written from the merged changes at cut time and reviewed against the diff before the tag is created. Tier promotions/demotions and any wire-schema change are always called out explicitly, because downstream builds depend on them.
- **When.** Update the `[Unreleased]` section in the same change that lands a user-facing behaviour, surface, tier, or dependency change. Internal refactors with no user-visible effect need no entry.
