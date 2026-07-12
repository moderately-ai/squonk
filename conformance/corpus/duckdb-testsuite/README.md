<!-- SPDX-License-Identifier: MIT -->

# DuckDB core-tranche spec-audit corpus

The first measurement child of the spec-level coverage-audit programme
([[spec-audit-duckdb-test-suite-corpus]]). Where the sibling `corpus/duckdb/`
group is a signature-weighted slice biased toward DuckDB's distinctive grammar, this
group is a broad slice of core `test/sql` directories — the executable spec the
signature weighting skipped.

It drives `core_tranche_spec_audit_inventory` in
`conformance/src/corpus_duckdb_verdicts.rs` against the in-process `DuckDbOracle`.
The live artifact counts, accept/reject quadrants, and family inventories are pinned
in that module. This README describes provenance and extraction only; it does not
mirror the Rust pins or test-printed ranked inventory.

## Source + pin

DuckDB is MIT (© 2018-2026 Stichting DuckDB Foundation). Pinned to the exact upstream
commit our vendored `libduckdb` oracle links:

- tag `v1.5.4`, commit `08e34c447bae34eaee3723cac61f2878b6bdf787`
  (`duckdb --version` reports `v1.5.4 (Variegata) 08e34c447b`).

Drawn from `test/sql/{select,join,subquery,aggregate,window,cte,order,limit,types}`.
`LICENSE` is DuckDB's licence verbatim; each `.sql` carries an SPDX `.license`
companion; `PROVENANCE.toml` records the pin. MIT is on the `cargo xtask license`
permissive allowlist (ADR-0015).

## What is vendored

`extract_core.py` reads the sqllogictest-style `.test` files and emits three artifacts
(per-file caps, deduped, one statement per line, `;`-free):

- `statements.sql` — accepted statements (`statement ok` + `query` bodies).
- `rejects.sql` — rejected statements (`statement error` bodies), used by the
  over-acceptance differential.
- `statements_with_schema.sql` — the same queries and rejects regrouped under their
  source `.test` file with that file's concrete `CREATE` setup DDL, so the oracle binds
  names instead of binding-rejecting `FROM t` over an empty DB (`# file:` / `# setup` /
  `# query` / `# reject`).

The extractor keeps the corpus broad while avoiding statements that would require a
multi-statement execution context or template expansion. The grouped artifact lets the
sweep distinguish syntax signal from name-resolution noise without executing the
statements under test.

## Measurement method

DuckDB's oracle uses `PrepareBind`, so the sweep measures two surfaces: accepted
statements for coverage gaps and known-reject statements for syntax over-acceptances.
The Rust pins in `corpus_duckdb_verdicts.rs` guard anti-vanishing counts, grouped
schema coherence, provisioned residuals, and the two quadrant tuples. A drift fails
loudly and prints the fresh family inventory for review.

The pins are a measurement baseline, not a zero gate: the orchestrator routes residual
families to child tickets, and a parser fix or corpus change re-baselines the relevant
const after the fresh oracle output is reviewed. `extract_core.py` is committed so all
three artifacts are reproducible against the pinned v1.5.4 reference.
