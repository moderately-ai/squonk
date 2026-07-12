<!-- SPDX-License-Identifier: CC0-1.0 -->

# sqllogictest SQL Corpus

This directory vendors SQL extracted from the sqllogictest suite as a second broad, growing round-trip target for the local conformance tests, alongside the sqlglot identity corpus. Source material comes from the upstream sqllogictest repository at https://github.com/gregrahn/sqllogictest, at commit `c67f97bf3ca7e590d12e073408bcacaf2ff0f3a0`.

## License

Upstream ships no `LICENSE` file; its `COPYRIGHT.md` (© 2008 D. Richard Hipp) places the suite under a choice of the GPL, BSD, MIT, or CC0 licenses with "no attribution required". `LICENSE` in this directory is that `COPYRIGHT.md` copied verbatim. We elect **CC0-1.0** as the most faithful to the upstream "no attribution required" grant, and CC0-1.0 is on the `cargo xtask license` permissive allowlist. The companion `statements.sql.license` carries the SPDX marker; `supported.sql` and this README carry it inline.

## What is vendored

Unlike the sqlglot corpus — whose `identity.sql` is a byte-for-byte upstream fixture — sqllogictest ships no plain-SQL file. Its `.test` files interleave SQL with expected results and control records (`statement ok`, `query <types> <sort>`, result rows and hashes, `skipif`/`onlyif` guards, `hash-threshold`). `statements.sql` is the SQL *extracted* from that format: the statement bodies after each `statement ok` / `statement error`, and the query SQL after each `query <types> <sort>` header up to its `----` results separator. Expected-result rows, hashes, and `skipif`/`onlyif`/`hash-threshold`/`halt` control lines are dropped; each statement is normalized to a single line (per-line whitespace only, so in-line string-literal content is preserved). The extraction is the vendoring step here, so `statements.sql` is derived rather than byte-identical to any one upstream file.

To keep the subset representative rather than exhaustive, it takes the first N distinct statements (source order) from a fixed set of `.test` files spanning the `select`, `index`, and `random` families, then deduplicates globally (first occurrence wins). The select files share a query template, so their early statements overlap heavily; the "new" column is each file's distinct contribution after that global dedup:

| source `.test` file | first N | distinct contributed |
| --- | --- | --- |
| `test/select1.test` | 60 | 60 |
| `test/select2.test` | 60 | 42 |
| `test/select3.test` | 60 | 26 |
| `test/select4.test` | 50 | 50 |
| `test/select5.test` | 50 | 50 |
| `test/index/between/1/slt_good_0.test` | 35 | 35 |
| `test/index/orderby/10/slt_good_0.test` | 35 | 22 |
| `test/random/expr/slt_good_0.test` | 50 | 50 |
| `test/random/aggregates/slt_good_0.test` | 50 | 38 |
| **total** | | **373** |

This spans `CREATE TABLE`/`CREATE INDEX` DDL, bulk `INSERT ... VALUES` and `INSERT ... SELECT`, and a wide variety of `SELECT` shapes (arithmetic, `CASE`, correlated subqueries, `BETWEEN`, aggregates, `ORDER BY`, and dialect-specific operators such as MySQL `DIV`). As with the sqlglot corpus, much of it falls outside the M1 ANSI surface today — that is the point; it is a coverage target that grows as the parser grows.

## Supported vs guide

The split is decided by running every line through `squonk::parse_with`, never by hand. `supported.sql` holds the subset that parses and round-trips under the `Ansi` dialect in both the canonical and fully-parenthesized oracles; it is a regenerable, source-ordered, verbatim copy of those `statements.sql` lines, exercised through `assert_roundtrips` and `assert_roundtrips_parenthesized`.

There is no separate `guide.sql`. The PostgreSQL corpus needs a guide file because it vendors no full upstream file, so un-promoted statements would otherwise be lost; here `statements.sql` is the full extracted corpus, so every not-yet-supported statement is already tracked there and a `guide.sql` would only duplicate the unsupported majority. The conformance test instead machine-checks that every one of the 373 lines lands in exactly one class, so nothing is silently dropped, and that `supported.sql` matches the live classification so it can never drift.

Two small classes are pinned in the test (`conformance/src/corpus_sqllogictest.rs`) rather than in a file, mirroring how the PostgreSQL guide keeps unsupported cases ticketed instead of silently dropped: Postgres-only statements (which need the `Postgres` preset to parse and round-trip) and round-trip defects (which parse but fail a round-trip oracle, tracked under `prod-corpus-idempotence-stability`). At the vendored commit both classes are empty: everything that parses under `Ansi` also round-trips cleanly.

## Coverage

At the vendored commit, 313 of 373 statements (83.91%) are validated, all round-tripping under `Ansi`. The remaining 60 are outside the current surface — chiefly `CREATE INDEX`/`CREATE UNIQUE INDEX` DDL, the `SELECT ALL` and aggregate `ALL` quantifiers (`COUNT(ALL x)`), parenthesized join factors, and MySQL-only operators (`DIV`, `CAST(x AS SIGNED)`). The conformance test reports this breakdown on every run.

## Regenerating

`supported.sql` is regenerable. Running the conformance tests with `REWRITE=1` — the same convention as the datadriven goldens — rewrites it from the current classification and prints the suggested `POSTGRES_ONLY_SUPPORTED` and `ANSI_ROUNDTRIP_DEFECTS` lists for the test consts:

    REWRITE=1 cargo nextest run -p squonk-conformance corpus_sqllogictest

Because the supported set is a checked-in cache, coverage changes surface as a reviewable diff rather than silently. Re-extracting `statements.sql` itself is a manual vendoring step (from the source files and caps above) and pinned by `STATEMENTS_TOTAL`, so a statement vanishing from the fixture fails the test.
