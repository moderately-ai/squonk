<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# Changelog

All notable changes to this project are documented here. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html) as scoped by the public-API contract in [docs/stable-api.md](docs/stable-api.md).

## [Unreleased]

_No unreleased changes yet._

## [3.0.0] — 2026-07-31

A dialect-surface release: QuiltDB's grammar closes several gaps against its engine contract, `COMMENT ON` grows two typed targets, and the project gains a documentation site with an in-browser playground. The major version is forced by the stable-API contract, not by churn: three dialect-data structs gained a public field, and two per-dialect reservation constants were replaced by shared ones. All coordinated packages (crates, wheel, npm family) move to 3.0.0 together, and `v3.0.0` becomes the 3.x semver baseline.

### Breaking (Rust API)

- **Three exhaustive dialect-data structs gained a field**:
  `ColumnDefinitionSyntax::underscored_autoincrement_attribute`,
  `StatementDdlGates::create_or_replace_materialized_view`, and
  `IndexAlterSyntax::alter_table_drop_index`. *Migration:* struct literals must
  name the new field; literals built with functional update from a preset
  (`ColumnDefinitionSyntax { …, ..ColumnDefinitionSyntax::ANSI }`) compile
  unchanged.
- **`DATABRICKS_QUALIFY_RESERVATION` and `SNOWFLAKE_QUALIFY_RESERVATION` are
  removed.** The QUALIFY / PIVOT+UNPIVOT / MATCH_RECOGNIZE reservation keyword
  lists are now role-named shared constants — `QUALIFY_RESERVATION`,
  `PIVOT_RESERVATION`, `MATCH_RECOGNIZE_RESERVATION` — exported from
  `squonk_ast::dialect`. *Migration:* replace either removed constant with
  `QUALIFY_RESERVATION`; the composed per-dialect reject sets
  (`*_RESERVED_COLUMN_NAME` etc.) are value-identical.
- **`CommentTarget` gained `Schema` and `Sequence` mid-enum.** The enum is
  `#[non_exhaustive]` so matches are unaffected, but the implicit discriminants
  of later variants shifted; code casting variants to integers must not rely on
  the old ordering.

### Added

- **`COMMENT ON SCHEMA` and `COMMENT ON SEQUENCE` parse to their own
  `CommentTarget` variants** in every dialect with `COMMENT ON` (previously a
  parse error), matching PostgreSQL's `comment_text` object kinds; render,
  visit, and the generated Python/TypeScript views follow.
- **QuiltDB grammar closes engine-contract gaps.** The preset now accepts
  `ILIKE`, `DESCRIBE`/`DESC`, typed `SHOW TABLES` / `SHOW COLUMNS FROM`
  (via its own `ShowSyntax::QUILTDB`; the unserved typed-`SHOW` family stays
  rejected), both auto-increment spellings (`AUTOINCREMENT` /
  `AUTO_INCREMENT`, SERIAL-equivalent in the engine), and
  `CREATE OR REPLACE MATERIALIZED VIEW` (new statement-head gate, on for
  QuiltDB and Lenient).
- **A documentation and marketing site** (`website/`, Astro + Starlight) with
  getting-started and dialect/parsing/transpiling guides, plus an entirely
  client-side SQL playground running the WebAssembly engine; deployed to GitHub
  Pages by `deploy-site.yml`.
- **Registry metadata polish** for search facets: crates.io
  keywords/categories/description, PyPI Topic/Typing trove classifiers, and
  author/homepage/bugs/keywords on the npm native platform packages.

### Changed

- **QuiltDB narrows two identifier surfaces deliberately**: `qualify`, `pivot`,
  and `unpivot` are now reserved (column/table-name and bare-alias positions),
  so a use of the unsupported construct names it in the reject
  (`found QUALIFY`) instead of being swallowed as an alias — quote the words to
  keep using them as identifiers. Per-column `STORAGE`/`COMPRESSION` attributes
  are now rejected with a named construct (the engine owns its on-disk layout).
- **Reject-message quality**: `IS [NOT] DISTINCT FROM` with the predicate off,
  and `ALTER TABLE … DROP {INDEX|KEY} <name>` (unmodelled in every preset), now
  name the construct instead of failing on a nearby token; a column literally
  named `index`/`key` still drops through the column path.
- **SQLite engine fidelity**: `AUTOINCREMENT` is excluded from liberal
  type-name word runs exactly as the engine excludes it from `typename`
  (measured on sqlite3 3.54.0), so previously over-accepted forms like
  `a INTEGER AUTOINCREMENT (5)` now reject; every engine-valid input keeps its
  exact prior AST.
- **`CREATE … VIEW` rejects `OR REPLACE` combined with `IF NOT EXISTS`** — the
  two state opposite intents for the same name collision; each alone is
  unchanged.
- The MySQL `AUTO_INCREMENT` column attribute rides its own dialect flag
  instead of the trailing table-options gate (same accepted surface for every
  existing preset).

### Internal

- GitHub Actions pinned-SHA group bump across the CI and release workflows.
- `cargo xtask oracle-mysql` runs the oracle-featured conformance suite with the
  external MySQL dependency provisioned automatically — reusing a reachable
  server, otherwise starting (and always removing) the same `mysql:8` container
  CI provisions as a service.
- `release/semver-baseline.toml` rotates to the `v3.0.0` baseline per the
  stable-API policy.

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
- **Dialect organization freeze gate.** `cargo xtask knob-org` (also part of
  `cargo xtask tidy` / preflight) holds FeatureSet packaging bijection, synopsis
  honesty, head-contention ledger completeness, and residual-bool labeling
  discipline so organization drift fails CI rather than accumulating silently.
- **Closed-delta helpers for satellite presets.** ANSI satellites and DuckDB assert
  their exact top-level axis divergence from the documented base, keeping preset
  deltas MECE and reviewable.

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
- **Dialect FeatureSet organization is MECE and flip-proven.** Composite knobs are
  split along real grammar axes rather than packaging convenience:
  - New `TransactionSyntax` and `ViewSequenceClauseSyntax` composites (fields moved
    out of `UtilitySyntax` / `StatementDdlGates`).
  - `create_or_replace_table` lives on `StatementDdlGates`.
  - `prefix_colon_alias` is split by grammar position (`SelectSyntax` vs
    `TableExpressionSyntax`).
  - Transaction mode flag renames for polarity honesty:
    `transaction_mode_comma_required` → `transaction_modes_require_commas`,
    `transaction_modes_unique` → `transaction_modes_reject_duplicates`.
  - Statement dispatch is extracted; multi-claimant heads use dual-require companion
    gates and a complete head-contention ledger.
  - Every former residual dialect bool is a `ToggleableFeature` required by a
    flip-verified `LabeledCase` (accept, reject, or structural shape), closing the
    DP6 inventory without an AXIS_COVERED escape hatch.

### Fixed

- Fixed DuckDB vertical-tab/comment boundary parity and carriage-return line-comment
  handling found by raw-byte differential fuzzing.
- Fixed SQLite diagnostic classification so quoted parser-error text cannot be
  mistaken for a post-parse resolution failure.
- Fixed unquoted non-ASCII identifier parity for PostgreSQL, MySQL, and SQLite,
  including PostgreSQL's raw high-bit identifier class.
- Fixed DuckDB parse-only parity for bare `DESCRIBE`, which the engine parses before
  rejecting at a later semantic stage.
- Fixed DuckDB parse-only parity for the `DESC` utility spelling while preserving it
  distinctly from `DESCRIBE` in the AST and renderer.
- Fixed generic `SET` value parsing so PostgreSQL and DuckDB reject fully reserved
  keywords, while retaining each engine's measured special-keyword spellings.

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

[Unreleased]: https://github.com/moderately-ai/squonk/compare/v3.0.0...HEAD
[3.0.0]: https://github.com/moderately-ai/squonk/compare/v2.0.0...v3.0.0
[2.0.0]: https://github.com/moderately-ai/squonk/compare/v1.0.0...v2.0.0
[1.0.0]: https://github.com/moderately-ai/squonk/releases/tag/v1.0.0

---

## Changelog policy

- **Format.** [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/). Entries are grouped under a version heading with `Added` / `Changed` / `Deprecated` / `Removed` / `Fixed` / `Security` subsections as needed; an `[Unreleased]` section at the top accumulates changes between releases.
- **Versioning.** [SemVer 2.0.0](https://semver.org/spec/v2.0.0.html), scoped by [docs/stable-api.md](docs/stable-api.md): the public API surface named there is what the major version protects. Additive dialect coverage and newly-accepted syntax are minor or patch changes; a breaking change to the public API or the `serde` wire schema is a major bump. The `serde` wire schema carries its own `v1`/`v2` marker inside the major line.
- **Cadence.** Release-driven, not calendar-driven. A release is cut when a coherent batch of user-facing change has landed and the full gate stack plus a rehearsed release-candidate is green; there is no fixed schedule. Security fixes are released as soon as a fix is verified (see [SECURITY.md](SECURITY.md)).
- **Ownership.** The maintainer (Moderately AI) owns release notes and version choice. Each release entry is written from the merged changes at cut time and reviewed against the diff before the tag is created. Tier promotions/demotions and any wire-schema change are always called out explicitly, because downstream builds depend on them.
- **When.** Update the `[Unreleased]` section in the same change that lands a user-facing behaviour, surface, tier, or dependency change. Internal refactors with no user-visible effect need no entry.
