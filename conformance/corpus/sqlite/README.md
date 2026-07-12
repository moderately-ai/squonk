<!-- SPDX-License-Identifier: CC0-1.0 -->

# SQLite feature-probe corpus

This directory vendors a small, self-authored corpus of SQLite-specific SQL used by the `sqlite-dialect-100-percent-programme` phase-0 assessment sweep (`conformance/src/corpus_sqlite_verdicts.rs`). It is the deliberate complement to the broad vendored corpora: the sqllogictest corpus is SQLite-*derived* but its 373 statements are dominated by the `select1`..`select5` query templates (standard-SQL projection/`CASE`/subquery shapes), so it is thin in the SQLite-*idiomatic* families this programme must model. `features.sql` fills those families directly.

## Why self-authored (not extracted)

SQLite's source, documentation, and test suite are all public domain (<https://sqlite.org/copyright.html>), so extraction would be licence-safe. We nonetheless *author* the probes rather than copy doc examples, because a probe here has one job — isolate exactly one grammar family in a single schema-independent statement — which curated authoring does better than any excerpt, and it sidesteps having to trace provenance for each line. The constructs mirror the documented SQLite grammar (facts, not expression); the statement text is our own, dedicated to the public domain under CC0-1.0 (matching the sqllogictest corpus's elected licence and on the `cargo xtask license` permissive allowlist).

## What is vendored

`features.sql` is one statement per line (no trailing `;`); blank lines and `--` comment lines are ignored by the loader and carry the family headers. Every statement is **schema-independent**: a bare in-memory `sqlite3_prepare` accepts it with no provisioned schema, so the sweep compares each cleanly against `SqliteOracle::new()` (a `PrepareBind` oracle) with no false "no such table" divergence. Statements that *need* a table to exercise a family (the `INSERT OR REPLACE`/upsert/`RETURNING`/`CREATE INDEX` mutation surface) live instead as the setup-driver probe constants in `corpus_sqlite_verdicts.rs`, provisioned behind a fixed schema.

Families covered: quoted identifiers (backtick, `[bracket]`, the double-quoted-string fallback), the `==`/`IS`/`GLOB` operators and bitwise operators, hexadecimal integer literals, all five bind-parameter spellings (`?`, `?NNN`, `:name`, `@name`, `$name`), the `LIMIT <count>, <offset>` comma form, the `PRAGMA`/`ATTACH`/`VACUUM`/`REINDEX`/`ANALYZE` statements, and the `CREATE TABLE` decorations (`AUTOINCREMENT`, `WITHOUT ROWID`, `STRICT`, typeless/affinity columns, `COLLATE`, generated columns, column `ON CONFLICT`, parenthesized `DEFAULT`).

A construct absent here is deliberately deferred, not forgotten: `MATCH`/`REGEXP` are operators whose backing functions are unregistered in the bundled engine, so a bare `prepare` rejects them (a function-resolution artifact, not a grammar signal — recorded in the preset ticket's structural-oracle-bound note); `DETACH` needs a prior `ATTACH` in the same connection; triggers and the mutation families need a schema. The setup-driver probe list and the preset ticket track those.

## Regenerating

Hand-maintained. To add a family: append a schema-independent probe line under the matching header, then re-run the sweep

    cargo nextest run -p squonk-conformance --features oracle-engines corpus_sqlite_verdicts

which pins the statement count and re-derives the divergence inventory (a new line that our parser already accepts, or that the bundled engine rejects, fails the sweep so it can be triaged rather than silently miscounted).
