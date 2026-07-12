<!-- SPDX-License-Identifier: CC0-1.0 -->

# MySQL top-level statement-family inventory

This directory holds a small, **self-authored** corpus naming every top-level statement family MySQL 8.4 admits, one minimal authored probe per family. It is the MySQL analogue of the PostgreSQL production-coverage denominator (`conformance/corpus/pg-regress/stmt-productions.txt`), adapted around the GPL licensing boundary, and the complement of the SQLite feature-probe corpus (`conformance/corpus/sqlite/features.sql`). The sweep that consumes it lives in `conformance/src/corpus_mysql_verdicts.rs` (`mysql_statement_family_inventory_*`).

## Licensing boundary — the headline constraint

MySQL's grammar `sql_yacc.yy` is **GPL** and is **never** vendored, copied, or extracted-from into this repository: zero grammar bytes, zero verbatim rule text. The family SET was derived from a **local, read-only** reading of the pinned GPL grammar (`github.com/mysql/mysql-server` at tag `mysql-8.4.10`, held OUTSIDE the repo) for the FACT of which top-level families the `simple_statement` production admits. Facts — *which* statement families exist — are not copyrightable expression; everything **written** here (family names, descriptions, and each probe's SQL) is **original** text authored for this corpus and dedicated to the public domain under CC0-1.0.

Unlike PostgreSQL's `stmt-productions.txt` — extracted by a committed script from the permissively-licensed `gram.y` — there is deliberately **no extraction script**. The reproducibility story is this documented fact-derivation plus the per-family live-oracle probes, never a program run over GPL text. See `PROVENANCE.toml`.

## What is vendored

`families.sql` — one `-- family: <NAME>` header line followed by that family's single authored probe line. Blank lines and every other `--` comment line are ignored by the loader (`mysql_statement_families` in `corpus_mysql_verdicts.rs`). 110 families are covered, spanning: query & DML (SELECT, TABLE, VALUES, INSERT, REPLACE, UPDATE, DELETE, DO, HANDLER, LOAD DATA/XML, LOAD INDEX/CACHE INDEX); prepared statements (PREPARE/EXECUTE/DEALLOCATE); transactions, session & locking (START TRANSACTION/COMMIT/ROLLBACK/SAVEPOINT, LOCK/UNLOCK TABLES + INSTANCE, XA, SET/SET TRANSACTION, USE); the CREATE / ALTER / DROP object matrix (database, table, index, view, trigger, procedure, function, event, user, role, server, tablespace, undo tablespace, logfile group, spatial reference system, resource group); table maintenance (TRUNCATE, RENAME TABLE/USER, ANALYZE/CHECK/CHECKSUM/OPTIMIZE/REPAIR TABLE); access control (GRANT/REVOKE, SET ROLE/RESOURCE GROUP); diagnostics & signals (SIGNAL/RESIGNAL/GET DIAGNOSTICS); server administration (FLUSH/RESET/PURGE/KILL/SHUTDOWN/RESTART/CLONE, INSTALL/UNINSTALL PLUGIN/COMPONENT, IMPORT TABLE, HELP, BINLOG); introspection (SHOW, EXPLAIN); stored routines (CALL); and replication (CHANGE REPLICATION SOURCE/FILTER, START/STOP REPLICA, GROUP REPLICATION).

`SHOW` is one family here (representative probe `SHOW DATABASES`); MySQL's grammar splits it into ~50 near-identical sub-command productions, tracked as a single top-level family for this inventory — granularizing it is a candidate follow-up.

## Two axes, tracked separately

The sweep records **engine reach** and **squonk reach** independently (mirroring the PostgreSQL probe table's engine-vs-squonk split):

- **Engine reach** — every probe is verified against the live m3 MySQL oracle. Because m3 is PREPARE-only (`COM_STMT_PREPARE`, never execute — so no `zzp_*` object is ever created), a grammar-valid family reaches one of three non-syntax wire outcomes: it PREPAREs (accept), reports `ER_UNSUPPORTED_PS` (1295 — grammar-valid but not preparable, the large administrative / stored-program surface), or binding-rejects a `zzp_*` placeholder. Only `ER_PARSE_ERROR` (1064) means "not recognized", which for an authored probe would be an inventory bug. So **MySQL 8.4.10 recognizes all 110 families** (syntax = 0). Measured split against the pinned server: 50 PREPARE, 59 ER_UNSUPPORTED_PS, 1 other (CALL → ER_SP_DOES_NOT_EXIST 1305). The server `VERSION()` is captured on every run as "oracle actually ran" evidence.

- **squonk reach** — whether the fitted `MySql` preset parses each probe, partitioned into supported vs the measured, pinned uncovered set (`MYSQL_UNCOVERED_STATEMENT_FAMILIES`). Engine reach does **not** imply squonk implements the family: the preset covers 59/110 families at this baseline (fresh-measured when `parse-mysql-lock-tables-instance` landed; the count moves with every family landing and the pinned uncovered list in `corpus_mysql_verdicts.rs` is the authoritative residual). The remaining **51 families are the measured coverage residual** — the release-blocking negative space this inventory pins, and the source of follow-up children (LOAD DATA, FLUSH/PURGE, HANDLER, XA, the object DDL matrix, replication, and more).

## Regenerating

Hand-maintained. To add a family, append a `-- family: <NAME>` header + one authored minimal probe under the matching section in `families.sql`, then re-run the sweep

    MYSQL_ORACLE_URL=mysql://root@127.0.0.1:3306 \
      cargo nextest run -p squonk-conformance --features oracle-engines,oracle-mysql \
      mysql_statement_family_inventory --no-capture

which re-pins the family count and the squonk coverage residual, verifies the new probe against the live oracle, and re-derives the engine-reach summary (a probe MySQL syntax-rejects, or a coverage-set drift, fails the sweep so it is triaged rather than silently miscounted).
