<!-- SPDX-License-Identifier: MIT -->
# Conformance & differential oracles

The testing foundation for `squonk` ([ADR-0014](../docs/adr/0014-testing-strategy.md)) — the
differential harness that verifies each engine-backed dialect against the *real* engine
instead of against our own expectations. It is `publish = false`, so its test-only
dependencies (`proptest`, `bolero`, the oracle engines) never reach downstream users. The
full module map — every oracle adapter, corpus, and sweep, with the per-dialect layout
convention — is the crate-level doc in [`src/lib.rs`](src/lib.rs); this is the front door.

## The AST is the oracle, not the rendered string

The prior art asserted the re-rendered SQL *text*, so a wrong-but-identically-rendered tree
shipped green (the DIV-class precedence bug, [ADR-0014](../docs/adr/0014-testing-strategy.md)).
Downstream consumers read the *AST*, so the AST is the unit of truth here: the oracle
compares *trees*, not strings, using a structural `PartialEq` that excludes span and
`NodeId` (`Meta`, ADR-0002) and remaps both trees through one shared interner first.

## The differential-oracle model

Every engine plugs into one seam — `oracle::AcceptRejectOracle` — so a new dialect supplies
its adapter without re-wiring the harness. The engines and how they run:

| Engine | Adapter | How it runs | Gate |
|---|---|---|---|
| PostgreSQL | `pg` (`pg_query` 6.1) | in-process, **always on** | default build |
| SQLite | `m2::SqliteOracle` (rusqlite, bundled) | in-process embedded | `--features oracle-engines` |
| DuckDB | `m2::DuckDbOracle` (libduckdb-sys) | in-process, system `libduckdb` | `--features oracle-engines` |
| MySQL | `m3::MySqlOracle` | **external** `mysql:8` server over the wire (GPL stays external-process — never linked or vendored, ADR-0015) | `--features oracle-mysql` |
| ClickHouse / BigQuery | `clickhouse` / sqlglot cross-check | external-process `ParseOnly` spawn | `--features oracle-clickhouse` / `oracle-bigquery` |

Two coverage tiers sit on top: a hand-written **premium** mapper into a neutral shape
vocabulary (literal / alias / arity sensitive) and a cheap **commodity** fingerprint or
tree-equality lane any engine can add — see the tier discussion and the per-dialect homes
table in [`src/lib.rs`](src/lib.rs).

### Skip cleanly, and prove what ran

Every oracle **skips cleanly when its engine is absent** — so a green run only proves parity
for the engines that actually *ran*. The curated-corpus parity tests make that visible: each
prints `oracle-ran: <engine> (<version>)` on the ran path and `skipping <engine>
differential: …` on the skip path. `oracle-nightly.yml` greps those markers as its hard
gate; the same markers give a local ran-vs-skipped readout:

```sh
cargo nextest run -p squonk-conformance --features oracle-engines,oracle-mysql \
  -E 'test(accept_reject_parity_over_curated_corpus)' --success-output final
```

`--success-output final` is load-bearing: a skip *passes* via an early `eprintln!` +
return, and nextest suppresses passing-test output by default — so without it the run is
green and silent, the exact "believed it ran" trap this readout closes.

### Count-pinned discipline

Each dialect's verdict sweep pins the exact tally of accepts, rejects, and known divergences
(the per-engine *ledger*). A change in what the parser accepts moves a pinned count and fails
the sweep loudly — behaviour cannot drift silently, and every shift is a reviewed edit to the
pin. The pins are re-measured from a fresh run, never adjusted by arithmetic.

### Divergence allowlists — reasoned and self-arming

A genuine our-tree-is-wrong divergence is quarantined in a per-engine allowlist
(`PG_DIVERGENCE_ALLOWLIST`, `M2_DIVERGENCE_ALLOWLIST`, `M3_DIVERGENCE_ALLOWLIST`, the corpus
verdict lists). The discipline ([ADR-0015](../docs/adr/0015-source-of-truth-testing.md),
[`CONTRIBUTING.md` § Allowlist etiquette](../CONTRIBUTING.md#allowlist-etiquette)):

- **Every entry states its rationale** — never a bare silencer for a
  representation-equivalent shape the structural mapping should absorb.
- **Deleting an entry re-arms the check.** A staleness test asserts the divergence still
  exists, so a silent fix is forced to remove the entry.
- **Entries are swept, not re-pinned** — a stale line is removed at merge, never bumped to a
  new line number.

## Running without an oracle

The default `cargo nextest run` greens with **every oracle skipping cleanly** — the PostgreSQL
lane (in-process `pg_query`) is the only always-on engine, and no external server or system
library is required. `cargo xtask preflight` is exactly what per-push CI runs; it
drops only the libduckdb / MySQL lanes, which stay in the nightly oracle workflow. So a
contributor with no database installed runs the full non-oracle suite locally and matches CI.
The [`bench/`](../bench/README.md#no-oracle-no-external-engine-required) crate shares this
no-oracle path; the environment setup and the GPL-external boundary are
[`CONTRIBUTING.md` § Oracle environments](../CONTRIBUTING.md#oracle-environments) and
.

## Fuzzing

The generative side — stable `cargo test` Bolero targets and their nightly libFuzzer twins,
sharing one body per target — lives in [`fuzz/README.md`](fuzz/README.md). The OSS-Fuzz
posture is [ADR-0019](../docs/adr/0019-oss-fuzz-readiness.md).
