# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Build the frozen, self-authored portable SQL performance corpus."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent
OUTPUT = ROOT / "corpus" / "portable.json"


def queries() -> list[str]:
    rows: list[str] = []
    for i in range(32):
        rows.append(f"SELECT {i} AS value_{i}")
    for i in range(32):
        rows.append(
            f"SELECT id, value FROM portable_left WHERE value >= {i} ORDER BY id"
        )
    for i in range(32):
        rows.append(
            "SELECT l.id, COUNT(r.id) AS matches "
            "FROM portable_left AS l LEFT JOIN portable_right AS r ON l.id = r.left_id "
            f"WHERE l.value >= {i} GROUP BY l.id HAVING COUNT(r.id) >= 0 ORDER BY l.id"
        )
    for i in range(32):
        rows.append(
            "WITH filtered AS ("
            f"SELECT id, value FROM portable_left WHERE value >= {i}"
            "), ranked AS ("
            "SELECT id, value, ROW_NUMBER() OVER (ORDER BY value, id) AS position FROM filtered"
            ") SELECT id, value FROM ranked WHERE position <= 10 ORDER BY position"
        )
    for i in range(16):
        rows.append(
            f"SELECT id, value FROM portable_left WHERE value = {i} "
            f"UNION ALL SELECT id, value FROM portable_left WHERE value = {i + 1} ORDER BY id"
        )
    return rows


def dml() -> list[str]:
    rows: list[str] = []
    for i in range(24):
        rows.append(
            f"INSERT INTO portable_values_{i} (id, value, label) VALUES ({i}, {i * 2}, 'row_{i}')"
        )
    for i in range(20):
        rows.append(
            f"UPDATE portable_values_{i} SET value = value + 1, label = 'updated_{i}' WHERE id = {i}"
        )
    for i in range(20):
        rows.append(f"DELETE FROM portable_values_{i} WHERE id = {i}")
    return rows


def ddl() -> list[str]:
    rows: list[str] = []
    for i in range(24):
        rows.append(
            f"CREATE TABLE portable_table_{i} (id INTEGER NOT NULL, value INTEGER, label VARCHAR(64), PRIMARY KEY (id))"
        )
    for i in range(12):
        rows.append(
            f"CREATE VIEW portable_view_{i} AS SELECT id, value FROM portable_left WHERE value >= {i}"
        )
    for i in range(12):
        rows.append(f"DROP TABLE portable_old_{i}")
    return rows


def build() -> dict[str, object]:
    groups = {
        "query": queries(),
        "dml": dml(),
        "ddl": ddl(),
    }
    assert {name: len(sql) for name, sql in groups.items()} == {
        "query": 144,
        "dml": 64,
        "ddl": 48,
    }

    statements: list[dict[str, object]] = []
    index = 0
    for family, sqls in groups.items():
        for sql in sqls:
            statements.append(
                {
                    "id": f"portable:{index:03d}",
                    "family": family,
                    "complexity": ("small", "medium", "large", "complex")[index % 4],
                    "sql": sql,
                    "bytes": len(sql.encode()),
                    "provenance": "self-authored",
                }
            )
            index += 1

    canonical = json.dumps(statements, sort_keys=True, separators=(",", ":")).encode()
    return {
        "schema": "squonk.publication-corpus/1",
        "name": "portable-full-ast-v1",
        "description": "Portable statement-only SQL used to compare complete AST parsers.",
        "license": "MIT",
        "statement_count": len(statements),
        "sha256": hashlib.sha256(canonical).hexdigest(),
        "qualification": {
            "status": "gated",
            "required_oracles": [
                "squonk-ansi",
                "libpg_query-postgresql-17",
                "mysql-8.4.10",
                "sqlite-3.53.2",
                "duckdb-1.5.4",
            ],
            "gate": "squonk-conformance publication_oracles::portable_publication_corpus_is_accepted_by_stable_engine_oracles",
            "rule": "The corpus is frozen before competitor qualification and is never intersected down.",
        },
        "statements": statements,
    }


def main() -> None:
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(build(), indent=2) + "\n")


if __name__ == "__main__":
    main()
