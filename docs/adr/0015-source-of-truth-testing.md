# ADR-0015: Source-of-truth differential testing & dialect milestones

- **Status:** Accepted (2026-06-26)
- **Atoms:** A32, A33, A34, A35

## Context

Round-trip self-consistency cannot catch three classes: accepting SQL a dialect rejects, rejecting SQL it accepts, and a wrong-but-round-trippable AST. The fix is *external ground truth* — and we should generate test data *from the real engines* rather than author our opinion of "correct."

## Decision

- **The real engine is the accept/reject source of truth**, elevated from a discovery tool to the *primary* correctness method for reproducible dialects. `pg_query` (the real PostgreSQL-17 parser, in-process) is highest-value: accept/reject *and* **structural** goldens via a protobuf→canonical mapping. `rusqlite` + `duckdb` `prepare()` (in-process, empty in-memory DB, never execute) give accept/reject; the MySQL/MariaDB server gives accept/reject; `sqlglot` is a wide-but-noisy breadth net. The differential runs *inside* the bolero harness (fuzz + differential = one loop). Accept/reject is a clean objective gate; structural needs a shape-mapping (PG, targeted, incremental).
- **Goldens (three tiers):** (1) engine-generated accept/reject (+ PG protobuf structure) — *objective, regenerable*; (2) our own output snapshotted in CockroachDB-`datadriven` format (canonical + fully-parenthesized + redacted columns) + a few `insta` Debug-AST snapshots — *regression*, kept honest by the round-trip/differential oracles; (3) a *thin* hand-authored edge layer. Input SQL comes from the engines' regression suites + sqlsmith/sqlancer.
- **`supports_*` coverage matrix:** a meta-test asserting every `FeatureSet` flag has a positive (a dialect enables it → accepts) *and* a negative (a dialect lacks it → rejects/diverges) case; the build fails on a flag with zero coverage. Enumerable because the `FeatureSet` is self-describing (ADR-0011).
- **Licensing:** vendor only permissive corpora (SQLite public-domain, DuckDB MIT, PostgreSQL regress, sqlglot/ZetaSQL/Calcite). MySQL `mysql-test` is GPL → **run the engine, don't vendor**. TPC/ClickBench/JOB → fetch/generate. CockroachDB testdata (BSL) → copy the *format*, not files. A REUSE/SPDX check (a local xtask/`#[test]`) guards the vendor subtree.

## Dialect milestones

| Milestone | Dialects | Oracle |
|---|---|---|
| **M1** | PostgreSQL + ANSI | `pg_query` (structural + accept/reject) · SQL:2016 BNF |
| **M2** | SQLite, DuckDB | `rusqlite` / `duckdb` `prepare()` (accept/reject) |
| **M3** | MySQL/MariaDB | server `PREPARE` (accept/reject; GPL → run-don't-vendor) |
| **M4+** | BigQuery (ZetaSQL), then MSSQL/Spark/ClickHouse (containers), then proprietary (corpus-only) | as oracles become available |

## Amendment (2026-06-27): the structural oracle normalizes representation-equivalent forms

Comparing two *independently built* parse trees (ours vs the `pg_query` protobuf) surfaces two categories that the protobuf→canonical mapping must treat differently — conflating them makes the oracle either noisy or blind:

1. **Representation-equivalent differences** — same semantics, different tree shape, an artefact of two parsers making different (both valid) choices. Example: a signed numeric literal is `UnaryOp(±, Literal)` for us but a folded signed constant for PostgreSQL (ADR-0006 amendment). The mapping **must normalize** these to a common shape; left alone they read as false divergences and drown the signal.
2. **Real divergences** — our tree is genuinely wrong. Example: mixed set-operation precedence, `a UNION b INTERSECT c` mis-bound to `(a UNION b) INTERSECT c` (ADR-0008 amendment). These must be **fixed**; until fixed, an explicit allowlist entry with a concrete rationale keeps the loop honest, and a replay test asserts the divergence still exists so a silent fix forces the allowlist entry's removal.

The divergence allowlist is for category (2) only — never reach for it to silence a (1) the mapping should absorb. The fuzz+differential loop is the mechanism that forces this distinction to be made explicitly; it found both categories on the M1 SELECT surface (`prod-fuzz-differential-loop`): signed literals → mapping normalization (`prod-pg-map-expressions`), set-op precedence → parser fix (`prod-adr-precedence-and-setops`).

A corollary on the allowlist's granularity: it is keyed by exact SQL string, which fits a finite triaged set but cannot suppress an open *class* of divergence. Where a known-divergent class is still unbounded (e.g. every mixed set-op), the differential loop restricts its **structural** generator to the comparable subset (with the exclusion documented and guarded by a replay that asserts the class still diverges), rather than weakening the assertion. Accept/reject parity, which has no such class gaps on the M1 surface, runs over the full generated surface.

## Interconnects

- code: `conformance/src/fuzz.rs` — the `pg_query` differential oracle; `xtask/src/lib.rs` — `check_corpus_licenses`
- invariant: PostgreSQL differential testing runs `pg_query` as a dev-only oracle; vendored corpora stay SPDX + provenance clean, never GPL/BSL.
- xtask: `cargo xtask license`.

## References

Atoms A32–A35. `pg_query`/libpg_query; CockroachDB datadriven; sqllogictest. Structural-normalization amendment driven by `prod-fuzz-differential-loop` findings.
