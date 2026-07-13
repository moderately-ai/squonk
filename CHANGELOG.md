<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# Changelog

All notable changes to this project are documented here. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) as scoped by the public-API contract in [docs/stable-api.md](docs/stable-api.md).

## [Unreleased]

_No unreleased changes yet._

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

[Unreleased]: https://github.com/moderately-ai/squonk/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/moderately-ai/squonk/releases/tag/v1.0.0

---

## Changelog policy

- **Format.** [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/). Entries are grouped under a version heading with `Added` / `Changed` / `Deprecated` / `Removed` / `Fixed` / `Security` subsections as needed; an `[Unreleased]` section at the top accumulates changes between releases.
- **Versioning.** [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html), scoped by [docs/stable-api.md](docs/stable-api.md): the public API surface named there is what the major version protects. Additive dialect coverage and newly-accepted syntax are minor or patch changes; a breaking change to the public API or the `serde` wire schema is a major bump. The `serde` wire schema carries its own `v1`/`v2` marker inside the major line.
- **Cadence.** Release-driven, not calendar-driven. A release is cut when a coherent batch of user-facing change has landed and the full gate stack plus a rehearsed release-candidate is green; there is no fixed schedule. Security fixes are released as soon as a fix is verified (see [SECURITY.md](SECURITY.md)).
- **Ownership.** The maintainer (Moderately AI) owns release notes and version choice. Each release entry is written from the merged changes at cut time and reviewed against the diff before the tag is created. Tier promotions/demotions and any wire-schema change are always called out explicitly, because downstream builds depend on them.
- **When.** Update the `[Unreleased]` section in the same change that lands a user-facing behaviour, surface, tier, or dependency change. Internal refactors with no user-visible effect need no entry.
