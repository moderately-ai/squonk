---
title: SQL dialects
description: squonk supports 14 SQL dialects as data — ANSI, Postgres, MySQL, SQLite, DuckDB, BigQuery, Snowflake, Redshift, Databricks, ClickHouse, Hive, MSSQL, QuiltDB, and a lenient union — each a small delta over a shared specification.
---

Most SQL parsers encode each dialect as a subclass full of overrides, which makes
dialects hard to inspect and compare. squonk treats a **dialect as data**: a small
set of feature deltas over a shared specification. You can inspect a dialect,
diff two presets field by field, or define a custom dialect as a delta instead of
forking parser code.

## Built-in dialects

Fourteen dialects ship today:

| Dialect | Notes |
| --- | --- |
| `ansi` | Standard SQL; the default. |
| `postgres` | Conformance-tested against `libpg_query`. |
| `mysql` | Conformance-tested against a live server. |
| `sqlite` | Tested against the embedded engine. |
| `duckdb` | Tested against the embedded engine. |
| `bigquery` | Cross-checked against sqlglot. |
| `clickhouse` | Checked with `clickhouse local`. |
| `snowflake`, `redshift`, `databricks` | Conservative, documentation-backed presets. |
| `hive`, `mssql` | Documentation-backed presets. |
| `quiltdb` | Held to a frozen parser-verdict corpus. |
| `lenient` | A permissive union dialect for parse-almost-anything workloads. |

Each dialect's support tier and source of truth is listed in
[support-tiers.md](https://github.com/moderately-ai/squonk/blob/main/docs/support-tiers.md).

## Selecting a dialect

<code>parse</code> takes the dialect explicitly, so the same input can be read under
different grammars:

```python
import squonk as sql

pg = sql.parse("SELECT id FROM t WHERE ts > now() - interval '1 day'", dialect="postgres")
```

In Rust, native dialects are enabled through Cargo features (`postgres`, `mysql`,
`sqlite`, `duckdb`, …, or `full` for all of them); the default build is ANSI-only
and depends on a single micro-crate. In Python and TypeScript the unscoped
package bundles every dialect, while a focused `@squonk-sql/<dialect>` npm package
ships only one for a smaller artifact.

## Comparing and converting

Because dialects are data, two presets can be compared directly — and because the
AST is dialect-agnostic, a query parsed under one dialect can be rendered under
another. That is [transpilation](/guides/transpiling/).
