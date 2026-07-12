<!-- SPDX-License-Identifier: MIT -->

# DuckDB signature-surface corpus

This directory vendors a SQL corpus for the DuckDB dialect programme
([[duckdb-dialect-100-percent-programme]]): a license-safe slice of DuckDB's own
test suite, weighted toward the grammar surface that makes DuckDB distinctive.
It drives the accept/reject sweep in `conformance/src/corpus_duckdb_verdicts.rs`
against the in-process `DuckDbOracle` (ADR-0015 differential method).

Source material comes from the local DuckDB checkout at
`~/workspace/github.com/duckdb/duckdb` at commit
`7bacf4a248656d5a442c4fac4b6ada1e91423e5a` (`v1.5.2-6871-g7bacf4a248`).

## License

DuckDB is MIT (© 2018-2026 Stichting DuckDB Foundation). `LICENSE` is DuckDB's
licence copied verbatim. Each `.sql` file carries its SPDX marker in a companion
`<file>.license`; this README and `PROVENANCE.toml` carry it inline. MIT is on the
`cargo xtask license` permissive allowlist (ADR-0015 vendors only permissive
corpora — no GPL/BSL exposure).

## What is vendored

`statements.sql` — statements extracted from DuckDB's sqllogictest-style
`test/sql/**/*.test` files. The `.test` format interleaves SQL with expected
results and control records; `extract.py` pulls the statement body after each
`statement ok|error|maybe` record and the query SQL after each `query <types>`
header (up to its `----` results separator). Expected-result rows, hashes,
`#` comments, per-line `-- …` SQL comments, and `skipif`/`onlyif`/`loop`/`foreach`
control lines are dropped; each statement is normalized to a single line
(whitespace collapsed, trailing `;` stripped). Records containing sqllogictest
templating (`${…}`, `{DATA_DIR}`, `{type}`) or an internal `;` are skipped — the
latter because DuckDB's `prepare` executes all but the last statement of a
multi-statement string (`corpus_duckdb_verdicts` relies on one statement per line).

`statements_with_schema.sql` — the same selected queries, regrouped under
their source `.test` file with that file's concrete `CREATE` setup DDL (the
per-file **setup driver**, [[duckdb-corpus-oracle-at-scale]]). The format is
line-oriented with `# file:` / `# setup` / `# query` section markers; provisioning a
group's DDL before `prepare`ing its queries lets the DuckDB oracle bind names instead
of binding-rejecting `FROM integers` over an empty database, so the accept/reject
sweep sees real syntax signal rather than name-resolution noise. Only `CREATE`
records are captured (they establish object existence + columns, which is all
`prepare` binding needs), deduped by object identity and ordered so
schemas/types/sequences precede tables and views/indexes follow, so a whole-file
`execute_batch` provisions in one shot. Files whose DDL cannot provision on a fresh
in-memory database (`ATTACH`ed databases, extension functions, file-backed secrets)
degrade to the bare comparison — a counted residual, never a false divergence.

`docs_examples.sql` — hand-curated canonical forms, one per signature family,
spelling the textbook syntax from DuckDB's public documentation. Every line was
validated to parse under DuckDB 1.5.4 via `json_serialize_sql` before vendoring.
These anchor each family with a clean form alongside the test suite's edge cases.

## Signature weighting

The extractor tags each statement against DuckDB's signature grammar and caps each
family (and each source file) so the distinctive families stay balanced rather than
letting the large collection-literal pool crowd out the scarcer surfaces. Families
overlap (one statement can hit several), so the detector list is a weighting guide, not
a hand-maintained inventory:

| family | detector |
| --- | --- |
| collection literals (list `[…]` / struct `{k:v}` / `MAP`) | value-opening bracket/colon-brace |
| `GROUP BY ALL` / `ORDER BY ALL` | `(GROUP\|ORDER) BY ALL` |
| `PIVOT` / `UNPIVOT` | `(UN)?PIVOT` |
| lambda (`x -> …`) | arrow not `->>`/`>=` |
| FROM-first (`FROM t SELECT …`) | leading `FROM` |
| `* EXCLUDE` / `REPLACE` / `RENAME` / `COLUMNS(…)` | star modifier / `COLUMNS(` |
| `ASOF` / `POSITIONAL` join | keyword |
| `QUALIFY` | keyword |
| `UNION BY NAME` | `UNION (ALL )?BY NAME` |
| general fill | no signature match |

ASOF/positional, QUALIFY, and UNION BY NAME are genuinely scarcer in the upstream
suite; the caps keep them represented without pretending they are as common as the
larger families.

## Anti-vanishing

`corpus_duckdb_verdicts` pins the flat fixture counts, the docs-example count, the
grouped source-file/setup-DDL counts, and grouped-vs-flat coherence. A stale or
half-regenerated `statements_with_schema.sql` fails without the oracle. `extract.py` is
committed so both artifacts are reproducible against the pinned upstream reference.

## Structural note

Unlike SQLite, DuckDB exposes an in-engine AST dump: `SELECT
json_serialize_sql('SELECT …')` returns the parsed statement as JSON (SELECT-family
only — it errors `"Only SELECT statements can be serialized to json!"` on DDL/DML).
That is the lever for PG-class structural parity on the SELECT surface, mapped to
the neutral `QueryShape` the way `pg.rs` maps the PostgreSQL protobuf
([[duckdb-structural-oracle-select]]).
