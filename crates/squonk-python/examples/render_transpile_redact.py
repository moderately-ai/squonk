# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Render canonical SQL, redact literal values, and transpile between dialects."""

from __future__ import annotations

import squonk


SQL = "select id, email from users where id = $1 and status = 'active'"


def main() -> None:
    document = squonk.parse(SQL, dialect="postgres")

    print(f"canonical: {document.to_sql()}")
    print(f"redacted: {squonk.redact(document)}")
    print(
        "postgres: "
        + squonk.transpile(
            SQL,
            source_dialect="postgres",
            target_dialect="postgres",
        )
    )


if __name__ == "__main__":
    main()
