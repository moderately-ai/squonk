<!-- SPDX-License-Identifier: MIT -->

# Dialect docs/spec reference library

Official grammars, SQL-reference doc trees, and keyword lists per dialect, used as a guide corpus for dialect work. Owned by [[dialect-docs-reference-library]]; feeds the `oracle-parity-*` lanes, the `spec-level-coverage-audit-programme`, and the keyword-inventory diffs.

- **The manifest is the source of truth:** [`manifest.toml`](manifest.toml). Every dialect artifact is one `[[reference]]` entry with source URL, pinned version, licence verdict, storage class, destination, and status. Read its header for the storage-class policy, the citation convention, and the version/bump protocol.
- **Vendored bytes** live under [`corpora/`](corpora/) — a vendored-corpus root, so `cargo xtask license` enforces the SPDX marker + `PROVENANCE.toml` on every file, exactly like `conformance/corpus/`.

## Storage-class policy (ADR-0015, absolute)

| Class | Licences | Where it lives | Committed to repo? |
|-------|----------|----------------|--------------------|
| `vendorable` | PostgreSQL, SQLite public-domain/CC0, Apache-2.0, MIT, BSD/ISC | `docs/dialect-references/corpora/<dialect>/` | Yes — bytes + provenance |
| `local-only` | Proprietary docs (Oracle, Snowflake, BigQuery, MSSQL, Redshift, Databricks, MySQL docs) | `~/workspace/dialect-docs/<dialect>/` (+ builder machine) | Manifest entry only |
| `gpl-adjacent-local-only` | GPL (MySQL/MariaDB server grammar + test suites) | External process only | Manifest entry only |

## Vendored now

| id | dialect | artifact | licence |
|----|---------|----------|---------|
| `postgres/kwlist` | postgres | keyword categories (`kwlist.h`) | PostgreSQL |
| `postgres/sql_features` | postgres | ISO feature-conformance table | PostgreSQL |
| `duckdb/keywords` | duckdb | 4 PEG keyword-category lists | MIT |
| `trino/grammar` | trino | ANTLR4 `SqlBase.g4` | Apache-2.0 |
| `hive/grammar` | hive | ANTLR3 `HiveParser.g` + `HiveLexer.g` | Apache-2.0 |

## Acquisition list for Thomas

Two buckets. Bucket A is `local-only` (proprietary/GPL) — download to the machine-local library OUTSIDE the repo; only the manifest pin is committed. Bucket B is `vendorable` but not yet cloned locally — clone, then vendor into `corpora/<dialect>/` per the corpus pattern (add `PROVENANCE.toml` + `LICENSE` + a `.license` companion per file, then `cargo xtask license` must pass).

### Bucket A — local-only (download outside the repo, never vendor)

```bash
# Proprietary docs — freely readable, NOT redistributable. Mirror to a local library.
mkdir -p ~/workspace/dialect-docs/{oracle-db,snowflake,bigquery,mssql,redshift,databricks,mysql}

# Oracle Database 23ai SQL Language Reference
#   https://docs.oracle.com/en/database/oracle/oracle-database/23/sqlrf/   -> ~/workspace/dialect-docs/oracle-db/
# Snowflake SQL reference
#   https://docs.snowflake.com/en/sql-reference                           -> ~/workspace/dialect-docs/snowflake/
# BigQuery / GoogleSQL reference
#   https://cloud.google.com/bigquery/docs/reference/standard-sql/        -> ~/workspace/dialect-docs/bigquery/
# MSSQL / T-SQL reference
#   https://learn.microsoft.com/en-us/sql/t-sql/                          -> ~/workspace/dialect-docs/mssql/
# Redshift Database Developer Guide
#   https://docs.aws.amazon.com/redshift/latest/dg/cm_chap_SQLCommandRef.html -> ~/workspace/dialect-docs/redshift/
# Databricks SQL language manual
#   https://docs.databricks.com/en/sql/language-manual/                   -> ~/workspace/dialect-docs/databricks/
# MySQL 8.0 reference manual (docs are PROPRIETARY, not GPL)
#   https://dev.mysql.com/doc/refman/8.0/en/sql-statements.html           -> ~/workspace/dialect-docs/mysql/

# GPL — external process ONLY, never linked or vendored (run the engine as an oracle):
#   git clone https://github.com/mysql/mysql-server  (sql/sql_yacc.yy)    -> ~/workspace/dialect-docs/mysql/ (external)
```

After downloading, record the dated snapshot / commit in the matching `manifest.toml` entry's `version` (a deliberate commit) so acquisition stays reproducible.

### Bucket B — clone-then-vendor (permissive; not yet local)

```bash
# Clone upstream, then copy the named artifact into corpora/<dialect>/ with provenance.
git clone https://github.com/google/zetasql        ~/workspace/github.com/google/zetasql        # Apache-2.0 -> bigquery/grammar
git clone https://github.com/prestodb/presto       ~/workspace/github.com/prestodb/presto       # Apache-2.0 -> presto/grammar
git clone https://github.com/apache/spark          ~/workspace/github.com/apache/spark          # Apache-2.0 -> spark/grammar (SqlBaseParser.g4 + SqlBaseLexer.g4)
# SQLite keyword list + syntax reference (public domain) -> sqlite/keywords, sqlite/docs
#   https://www.sqlite.org/lang_keywords.html , https://www.sqlite.org/lang.html
```

Already-cloned permissive sources whose extra artifacts can be vendored on demand by the consuming lane (no new download needed): `postgres/grammar` (`gram.y`), `postgres/docs`, `duckdb/docs`, `trino/docs`, `clickhouse/docs`. See each entry's `verify_on_acquisition` flag — re-check the exact page/file licence before treating the bytes as vendorable.

## Citation convention

Cite a manifest entry by stable `id` + pinned `version`, never a bare URL:

```
dialect-ref: postgres/kwlist @ REL_18_BETA1-3053-g4b0bf0788b0  (docs/dialect-references/corpora/postgres/kwlist.h)
```

The wired exemplar is `crates/squonk-ast/src/dialect/ansi.rs` (the per-position reject sets cite `postgres/kwlist`). A bare URL should appear in exactly one place — the manifest entry's `source` — so there is one spot to fix when it rots.

## Version / bump protocol

Pins are deliberate. A bump = a commit that updates the entry's `version`, re-copies vendored bytes, updates the `PROVENANCE.toml` `reference`, and re-cites the new version in every consumer. Never track upstream HEAD. Keep vendored pins aligned with the matching `conformance/corpus/<dialect>` pin where one exists (postgres does today).
