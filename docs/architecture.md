# Architecture

Internal architecture reference for `squonk`: the subsystems, the parsed statement surface, and the workspace crate layout. The user-facing front door is the [workspace `README.md`](../README.md); the design rationale behind each decision lives in [`docs/adr/`](adr/) (ADR-0001 … ADR-0020).

## Subsystems

All of the following are implemented:

- **Tokenizer** — a hand-written, zero-copy tokenizer.
- **Parser engine** — the monomorphized `Parser<D>` engine: recursive descent plus a Pratt expression core over one binding-power table.
- **AST** — an owned, `'static` AST with byte-range spans and identifier interning.
- **Renderer** — a context-carrying renderer with canonical, fully parenthesized, and redacted modes, plus a Tier-2 target-dialect renderer and one-call `transpile`.
- **Dialect-as-data** — the dialect system is data, not dispatch.
- **Codegen** — schema-driven code generation for the checked-in AST walks.

## Dialects

The selectable dialects are the value list `BuiltinDialect::ALL`. `Ansi` — the always-compiled SQL-standard baseline that `parse` defaults to — is joined by the feature-gated presets `Postgres`, `MySql`, `Sqlite`, `DuckDb`, `BigQuery`, `Hive`, `ClickHouse`, `Databricks`, `Mssql`, `Snowflake`, `Redshift`, and the permissive `Lenient` "parse-anything" union; `full` turns on every feature-gated dialect.

Each preset carries a release-contract **support tier** recording how strong its parity evidence is; the authoritative per-dialect tier, source of truth, and oracle kind are generated into [`docs/support-tiers.md`](support-tiers.md) from `BuiltinDialect::support_tier` / `support_evidence`. In brief: `Ansi`, `Postgres`, `MySql`, `Sqlite`, and `DuckDb` are **stable**, each held to an authoritative source of truth (an engine differential, or the SQL standard for `Ansi`); `BigQuery`, `ClickHouse`, and `Lenient` are **preview**; and `Hive`, `Databricks`, `Mssql`, `Snowflake`, and `Redshift` are **experimental** documentation-derived presets with no differential oracle wired. The conservative ANSI-derived presets enable only the surface that already has a modelled, tested parser gate and reject unmodelled syntax cleanly.

Each stable engine-backed dialect is held to its real engine by a differential accept/reject oracle — `pg_query` in-process for PostgreSQL, in-process DuckDB and SQLite, and a live MySQL server spoken over the wire — so its accept/reject surface is corpus- and oracle-verified against the engine, not merely self-consistent. ClickHouse additionally has an external-process ParseOnly oracle (`clickhouse local`, `EXPLAIN AST`) over a partial modelled surface, not yet wired into the default gate. What the pins prove is bounded and honest: over-acceptance (we accept syntax the engine rejects) is held at **zero on every gated surface**, while the coverage-gap inventories (the engine accepts, we reject) are pinned per dialect as **un-gated measurement baselines** in the conformance suite, so a regression drifts a pin loudly but nothing is forced to zero and no gate demands a "finished" number. That corpus-relative parity is distinct from spec-production parity: grammar-production coverage — whether each dialect's own grammar productions are exercised at all — was measured under the completed `spec-level-coverage-audit-programme`, which reached its bar-A statement-coverage target for PostgreSQL, DuckDB, SQLite, and MySQL.

## Statement surface

The parsed surface spans the `Statement` enum's **40 variants** at family level:

- **Query** — `SELECT`, `WITH`/CTEs, `VALUES`, set operations, joins, window functions, and the full expression grammar (precedence, `CAST`, `IS NULL`/`BETWEEN`/`IN`, `EXISTS`, quantified comparisons, subqueries).
- **DDL** — `CREATE`/`ALTER`/`DROP` over tables, views, indexes, schemas, functions, databases, triggers, macros, and secrets, plus `TRUNCATE` and `COMMENT ON`.
- **DML** — `INSERT`/`UPDATE`/`DELETE`/`MERGE`.
- **DCL** — `GRANT`/`REVOKE`.
- **TCL** — transaction control.
- **Session config** — `SET`/`RESET`/`SHOW`.
- **Dialect utility statements** — `COPY`, `EXPLAIN`, `DESCRIBE`, `KILL`, `PRAGMA`, `ATTACH`/`DETACH`, `VACUUM`, `REINDEX`, `ANALYZE`, `USE`, `PREPARE`/`EXECUTE`/`DEALLOCATE`, `CALL`, and `PIVOT`/`UNPIVOT`.

## Bindings

- **Python** — `squonk-python` wraps the Rust parser in a maturin-built
  extension module and layers typed Python views over the shared JSON boundary.
- **WASM and TypeScript** — `squonk-wasm` exposes the same parser to
  browser, Node, worker, and edge runtimes through wasm-bindgen plus a typed
  TypeScript facade.

## Workspace layout

| Path | Crate | Published | Purpose |
| --- | --- | --- | --- |
| `crates/squonk-ast` | `squonk-ast` | yes | Dialect-agnostic SQL AST: node types, byte-range `Span`s, the `Meta`/`NodeId` wrapper, interned `Symbol`s, dialect data (`FeatureSet`), the one binding-power table, and the context-carrying `Render` trait. Its only non-optional dependency is the `thin-vec` micro-leaf (serde is opt-in), so downstream tooling (rewriters, linters, formatters) builds on it directly. |
| `crates/squonk` | `squonk` | yes | Tokenizer + `Parser<D>` engine + dialect implementations + the interner. Re-exports the AST as `squonk::ast`, so most users only need this crate. Depends only on `squonk-ast`. |
| `crates/squonk-wasm` | `squonk-wasm` | no | `wasm32` bindings exposing the pure-Rust parser to JS / the browser / the edge. The low-level wasm exports return JSON strings; the `js/` facade provides strict TypeScript declarations, typed `Document` / `Node` / `Ident` / `ObjectName` / `Diagnostic` wrappers, raw `parseJson` helpers, generated AST field metadata, discriminated token/trivia output, rendering, redaction, transpilation, and Node/browser examples. Built on the pure-Rust published crates (ADR-0017), it compiles to `wasm32-unknown-unknown` with no C toolchain and a tiny artifact — for in-browser linters/formatters, LSP-in-the-browser, and edge SQL validation. |
| `crates/squonk-python` | `squonk-python` | no | Python bindings — a maturin-packaged `squonk._native` extension module plus pure-Python typed views. The public surface includes `Document` / `Node` / `Ident` / `ObjectName` / `Diagnostic` / `Trivia` wrappers, raw `parse_dict` / `parse_recovering_dict` helpers, `TypedDict` JSON shapes, generated AST field metadata, discriminated token/trivia output, rendering, redaction, transpilation, and runnable examples. The AST still crosses the native boundary as shared `serde` JSON, so Rust stays PyO3 + `serde_json` without hand-mapped Python node classes. |
| `crates/squonk-sourcegen` | `squonk-sourcegen` | no | Dev-only code generator (an xtask, not a proc-macro — ADR-0013). Parses the hand-written AST and emits the checked-in `Spanned` / `Visit` / `VisitMut` / render-skeleton / size-assert walks under `crates/squonk-ast/src/generated/`. Run `cargo run -p squonk-sourcegen` after changing a node; a drift test fails the build if the checked-in output is stale. |
| `xtask/` | `xtask` | no | Local-runnable gate runner (ADR-0017). `cargo xtask preflight` is the headline command — the whole ordered stack in one run (`fmt → tidy → clippy → nextest → doc → guard`; `--matrix` adds the feature-combination gate). It composes the dependency-free tidy gates, runnable together as `cargo xtask tidy` or by name: `license` (vendored-corpus SPDX + provenance), `dialect` (the anti-`dialect_of!`/`TypeId` dispatch ban), `deps` (published-crate dependency allowlist), `traceability` + `adr` (ADR anti-drift), `precedence`, `extension-seam`, `dialect-generic`, and `comments`. |
| `bench/` | `squonk-bench` | no | Deterministic perf/allocation gates (Valgrind instruction counts via `gungraun`/callgrind, plus `dhat::assert_eq!` exact-allocation pins) alongside Criterion/CodSpeed wall-clock benches and ours-vs-`sqlparser`/`libpg_query` comparisons. Kept separate so bench dependencies never enter the published crates. |
| `conformance/` | `squonk-conformance` | no | The AST-as-oracle test suite (ADR-0014): structural round-trip properties (`proptest`), a `bolero` fuzz harness, per-dialect engine-differential accept/reject oracles behind a pluggable seam — `pg_query` in-process (PostgreSQL, always on), in-process DuckDB + SQLite (`oracle-engines`), and a live MySQL server (`oracle-mysql`) — the PostgreSQL and DuckDB structural-parity mappers, a tokenizer differential, and the cross-dialect feature-coverage matrix. |

The two published crates (`squonk`, `squonk-ast`) are held to a strict dependency-minimalism policy (ADR-0017): their only non-optional runtime dependency is the `thin-vec` micro-leaf (`squonk-ast` pulls in just `thin-vec`; `squonk` adds only `squonk-ast`), with `serde` support opt-in behind its feature. The `deps` tidy gate holds the published runtime surface to that allowlist. The rich dev/test toolchain lives only in the `publish = false` crates above.
