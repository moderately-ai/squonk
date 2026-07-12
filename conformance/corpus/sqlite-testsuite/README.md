<!-- SPDX-License-Identifier: CC0-1.0 -->

# SQLite core-tranche spec-audit corpus

The third measurement child of the spec-level coverage-audit programme
([[spec-audit-sqlite-suite-corpus]]). Where the sibling `corpus/sqlite/` group is a
small self-authored feature-probe slice, this group is a broad, conservatively
extracted slice of SQLite's public-domain TCL regression suite (`test/*.test`) — the
executable spec that curation skips.

It drives `sqlite_testsuite_spec_audit_inventory` in
`conformance/src/corpus_sqlite_verdicts.rs` against the in-process `SqliteOracle`.
The live artifact counts, accept/reject quadrants, and family inventories are pinned
in that module. This README describes provenance and extraction only; it does not
mirror the Rust pins or test-printed ranked inventory.

## Source + pin

SQLite's source tree — including the whole `test/` directory — is public domain (the
authors disclaim copyright; `LICENSE` is the upstream `LICENSE.md` verbatim). CC0-1.0
is the elected SPDX for redistributing it, matching the sibling `corpus/sqlite` group.
Pinned to the exact version our bundled `rusqlite` oracle links (libsqlite3-sys 0.38.1):

- SQLite **3.53.2** (SOURCE_ID `2026-06-03 … d6e03d8c…`), vendored from
  `https://sqlite.org/2026/sqlite-src-3530200.zip` (sha256
  `cafff764c03f6d720968f746e2f47a986bbf12bf4c18904f1eb131c0b0b592d3`); the
  archive's `manifest.uuid` matches the bundled library's `SQLITE_SOURCE_ID`.

## Extraction

Unlike the sqllogictest format, the TCL tests embed SQL inside
`execsql`/`catchsql`/`do_execsql_test`/`do_catchsql_test` brace blocks amid TCL noise
(`$vars`, `[commands]`, string maps, loops). `extract_tcl.py` is deliberately
conservative: it takes only blocks whose body is pure literal SQL — no `$`, no `[...]`,
no `\` escape, no nested TCL braces — so a noisy block is skipped and a mangled
statement is never emitted. Multi-statement bodies are split at top-level `;` with
`CREATE TRIGGER BEGIN…END` bodies and nested `CASE…END` kept intact.

## What is vendored

Three artifacts, one `extract_tcl.py` run (per-file caps, deduped, one statement per
line, no top-level `;` outside trigger bodies):

- `statements.sql` — accepted statements (`execsql` + `do_execsql_test` bodies).
- `rejects.sql` — rejected statements (`catchsql` + `do_catchsql_test` bodies), used by
  the over-acceptance differential. Many `catchsql` errors are runtime or binding errors;
  the sweep's reject classifier sorts syntax from binding/other.
- `statements_with_schema.sql` — the same queries and rejects regrouped under their
  source `.test` file with that file's pure `CREATE TABLE` setup DDL, so the oracle binds
  names instead of binding-rejecting `FROM t` over an empty DB (`# file:` / `# setup` /
  `# query` / `# reject`).

Alongside the corpus, one grammar-derived denominator from the same 3.53.2 source tree:

- `commands.txt` — the 25 canonical top-level command families of SQLite's `cmd`
  production, extracted from `src/parse.y` by `extract_cmd_productions.py` (the SQLite
  analogue of pg-regress `stmt-productions.txt`). It resolves each `cmd ::=` lemon
  alternative to a command name; `EXPLAIN` / `EXPLAIN QUERY PLAN` are excluded because the
  grammar makes them an `ecmd`-level prefix wrapping any `cmd`, not a `cmd` alternative.
  This is the independent denominator for command coverage: a command stays visible even
  when no vendored statement exercises it.

The curated families cover the statement and expression surface around SELECT, joins,
ordering, CTEs, windows, mutation, table/index/trigger/view DDL, pragmas, ALTER TABLE,
transactions, collation, functions, rowid, autoincrement/defaults, attach/vacuum/reindex,
and `WITHOUT ROWID`.

## Measurement method

SQLite's oracle uses `PrepareBind`, so the sweep measures accepted statements for
coverage gaps and known-reject statements for syntax over-acceptances while counting
binding residuals separately. The Rust pins in `corpus_sqlite_verdicts.rs` guard
anti-vanishing counts, grouped schema coherence, provisioned residuals, and the two
quadrant tuples. A drift fails loudly and prints the fresh family inventory for review.

The pins are a measurement baseline, not a zero gate: the orchestrator routes residual
families to child tickets, and a parser fix or corpus change re-baselines the relevant
const after the fresh oracle output is reviewed. `extract_tcl.py` is committed so all
three artifacts are reproducible against the pinned 3.53.2 source archive.

## Command-production coverage

`sqlite_testsuite_command_production_coverage_is_measured` maps every SQLite-accepted
corpus statement (accept + reject surfaces, behind the per-file setup driver) back to its
top-level `cmd` command family via the leading keywords, and pins the exact unexercised
set. It measures whether the executable spec reaches each top-level command production; it
does not claim coverage of the sub-productions inside a reached command. The TCL corpus
exercises 23 of the 25 command families.

`sqlite_unexercised_command_productions_have_permanent_oracle_probes` closes the corpus's
command negative space with one authored `SqliteOracle` probe per unexercised family
(`RELEASE`, `SAVEPOINT` — savepoint-family utilities the conservative extractor never
surfaced), taking combined coverage to 25/25. The probe table pins squonk acceptance
separately: engine-production reach is not a support claim (both probes happen to parse).

`sqlite_create_virtual_table_is_corpus_covered_and_parser_supported` pins the command the
coverage programme flagged as its proof-by-example gap. At this pin it is negative space in
neither axis: the accept corpus carries five `CREATE VIRTUAL TABLE` statements the oracle
accepts, and the fitted `Sqlite` preset parses it (`create_virtual_table` gate) — the probe
pins both, as separate evidence, so a regression on either surface fails loudly.
