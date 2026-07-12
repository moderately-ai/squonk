<!-- SPDX-License-Identifier: PostgreSQL -->

# PostgreSQL regression spec-audit corpus

The second measurement child of the spec-level coverage-audit programme
([[spec-audit-pg-regress-corpus]], sibling of
[[spec-audit-duckdb-test-suite-corpus]]). PostgreSQL's own regression suite is its
*executable spec*. Where the hand-curated `corpus/postgres/` group is a small
structural-oracle slice and the three vendored multi-dialect corpora
(`corpus/sqlglot` / `sqllogictest` / `sqlglot-complex`) are breadth-but-not-PG, this
is the full statement set extracted from `src/test/regress/sql/*.sql`.

It drives `pg_regress_spec_audit_inventory` and
`pg_regress_corpus_is_pinned_and_parses_without_panicking` in
`conformance/src/corpus_pg_verdicts.rs` against the in-process `pg_query` oracle.
The live statement counts, accept/reject quadrant, and family inventory are pinned
there (`PG_REGRESS_QUADRANT`, `PG_REGRESS_GAP_FAMILIES`, and
`PG_REGRESS_OVERACCEPT_FAMILIES`). The README describes the corpus and extraction
method only; the Rust consts and test output are the authority for measured counts.

## Source + pin

PostgreSQL is under the PostgreSQL Licence (© 1996-2026 PostgreSQL Global Development
Group; verbatim in `LICENSE`). Pinned to the newest REL_17_* tag so the corpus aligns
with our oracle — `pg_query` 6.1.1 (libpg_query) is the **PostgreSQL 17** parser:

- tag `REL_17_10`, commit `25c49f3a4a742ba283f5cc43cc7f1d361552e917` (PostgreSQL 17.10).

PostgreSQL freezes its grammar at each major's `.0`, so 17.10 and 17.0 share one
grammar; the newest 17.x tag maximizes the vendored corpus while staying oracle-aligned
(REL_18 would out-run the PG-17 oracle). `statements.sql` carries an SPDX `.license`
companion; `PROVENANCE.toml` records the pin; the PostgreSQL licence is on the
`cargo xtask license` permissive allowlist (ADR-0015).

## What is vendored

`extract_pg_regress.py` is a psql-aware statement splitter (the regress `.sql` files are
psql scripts, not plain SQL). It splits on top-level `;` while honouring dollar-quoting
(`$$`/`$tag$`, never `$1` params), single/double quotes (`''` doubling + E-string `\`
escapes), line comments and nested block comments; strips psql meta-commands (a
backslash mid-statement — `\gset`/`\gexec`/`\g`/… — flushes the accumulated SQL as a
statement and drops the rest of the line); skips `COPY … FROM STDIN` inline data (keeps
the terminable `COPY` head, drops the rows to the lone `\.`); and drops statements
carrying psql `:'var'`/`:"var"` interpolation. Output is grouped under `# file:` markers
(provenance) and deduped globally.

- `statements.sql` — the flat statement corpus. Every non-`#` non-blank line is a
  statement; the `# file:` grouping traces each divergence family back to its source.
- `stmt-productions.txt` — the sorted 124 direct alternatives of PostgreSQL 17's
  top-level `stmt` production, extracted by `extract_stmt_productions.py`. This is the
  independent denominator for production coverage: a production remains visible even
  when the regress suite contains no statement that exercises it.

Extraction was clean at day-scale (no scope-guard subset needed): zero leftover
backslash junk, COPY data stripped, dollar-quoted function bodies intact.

## Measurement method

`pg_query` is `ParseOnly` and in-process, so — unlike the DuckDB/SQLite `PrepareBind`
sweeps — there is no schema provisioning and no binding/syntax split: every statement
gets a clean two-engine verdict. The inventory test pins the four accept/reject cells
and the rolled family maps, then prints the current ranked inventory on drift for
triage.

The pins are a measurement baseline, not a gate: nothing is forced to zero and no
ticket is required here. A parser fix or corpus change re-baselines the relevant Rust
const after the fresh oracle output is reviewed. `extract_pg_regress.py` is committed
so `statements.sql` is reproducible against the pinned REL_17_10 reference.

`pg_regress_statement_production_coverage_is_measured` maps every PostgreSQL-accepted
corpus statement's raw parse node back to its `stmt` alternative. It pins the exact
unexercised set and prints both halves. This measures whether the executable spec reaches
each top-level statement production; it does not claim coverage of the sub-productions
inside an exercised statement family.

`pg_unexercised_statement_productions_have_permanent_oracle_probes` closes the
regression corpus's negative space with six minimal authored `pg_query` probes. Together,
the regress suite and probes exercise 123/124 alternatives. The remaining
`CreateAssertionStmt` is kept in the grammar inventory, but PostgreSQL 17 raises its
"not yet implemented" parser error instead of producing a raw node. The probe table also
pins squonk acceptance separately; production reach does not imply that squonk
implements these object-DDL statements.
