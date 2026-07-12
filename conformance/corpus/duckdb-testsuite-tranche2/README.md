<!-- SPDX-License-Identifier: MIT -->

# DuckDB tranche-2 spec-audit corpus

The second measurement child of the spec-level coverage-audit programme
([[spec-audit-duckdb-remaining-tranches]]), sibling to the tranche-1
`corpus/duckdb-testsuite/` group ([[spec-audit-duckdb-test-suite-corpus]]). Tranche 1
covered nine core `test/sql` directories
(select/join/subquery/aggregate/window/cte/order/limit/types); this tranche covers the
nineteen remaining directories the ticket names —
insert/update/delete/merge (DML), create/alter/index/constraints (DDL),
copy/storage/attach, function, pivot, prepared, optimizer, pragma/settings, and
tpch/tpcds — the rest of the executable spec.

It drives `tranche2_spec_audit_inventory` in
`conformance/src/corpus_duckdb_verdicts.rs` against the in-process `DuckDbOracle`. The
live artifact counts, accept/reject quadrants, and family inventories are pinned in
that module under a separate set of `TRANCHE2_*` consts that never touch the tranche-1
`CORE_*` pins. This README describes provenance and extraction only; it does not mirror
the Rust pins or the test-printed ranked inventory.

## Source + pin

DuckDB is MIT (© 2018-2026 Stichting DuckDB Foundation). Pinned to the exact upstream
commit our vendored `libduckdb` oracle links:

- tag `v1.5.4`, commit `08e34c447bae34eaee3723cac61f2878b6bdf787`
  (`duckdb --version` reports `v1.5.4 (Variegata) 08e34c447b`).

Drawn from
`test/sql/{insert,update,delete,merge,create,alter,index,constraints,copy,storage,attach,function,pivot,prepared,optimizer,pragma,settings,tpch,tpcds}`.
`LICENSE` is DuckDB's licence verbatim; each `.sql` carries an SPDX `.license`
companion; `PROVENANCE.toml` records the pin. MIT is on the `cargo xtask license`
permissive allowlist (ADR-0015).

## What is vendored

`extract_tranche2.py` is byte-for-byte the tranche-1 `extract_core.py` recipe with only
its `CORE_DIRS` list swapped to the tranche-2 directories, so the statement-class rules
(sqllogictest `statement ok`/`query`/`statement error` records), the skip filters
(`;`-free, 5–400 chars, no template/placeholder markers, no external-file DDL refs), the
per-file caps, and the setup-DDL harvesting are identical to tranche 1. It emits three
artifacts (per-file caps 12 accepts / 8 rejects, deduped, one statement per line,
`;`-free):

- `statements.sql` — accepted statements (`statement ok` + `query` bodies).
- `rejects.sql` — rejected statements (`statement error` bodies), used by the
  over-acceptance differential.
- `statements_with_schema.sql` — the same queries and rejects regrouped under their
  source `.test` file with that file's concrete `CREATE` setup DDL, so the oracle binds
  names instead of binding-rejecting `FROM t` over an empty DB (`# file:` / `# setup` /
  `# query` / `# reject`).

The `tpch`/`tpcds` directories contribute almost nothing: their benchmark queries exceed
the 400-char standalone-statement cap (the same cap tranche 1 applied), so only their
short setup/utility statements survive. This is the documented consequence of mirroring
tranche 1's filters unchanged, not a separate policy.

## Measurement method

DuckDB's oracle uses `PrepareBind`, so the sweep measures two surfaces: accepted
statements for coverage gaps and known-reject statements for syntax over-acceptances.
The Rust `TRANCHE2_*` pins in `corpus_duckdb_verdicts.rs` guard anti-vanishing counts,
grouped schema coherence, the provisioning residual, and the two quadrant tuples. A
drift fails loudly and prints the fresh family inventory for review.

The pins are a measurement baseline, not a zero gate: the orchestrator routes residual
families to child tickets, and a parser fix or corpus change re-baselines the relevant
const after the fresh oracle output is reviewed. `extract_tranche2.py` is committed so
all three artifacts are reproducible against the pinned v1.5.4 reference.
