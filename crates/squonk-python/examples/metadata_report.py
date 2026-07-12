# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Parse SQL and report common metadata from the typed AST wrappers."""

from __future__ import annotations

import squonk


SQL = "SELECT u.id, u.email FROM public.users AS u WHERE u.id = $1"


def main() -> None:
    document = squonk.parse(SQL, dialect="postgres", capture_trivia=True)

    print(f"dialect: {document.dialect}")
    print(f"statements: {len(document.statements)}")
    print(f"canonical: {document.to_sql()}")

    identifiers = sorted({ident.text for ident in document.find_all(squonk.Ident)})
    print(f"identifiers: {', '.join(identifiers)}")

    tables = []
    for node in document.find_all("Table"):
        name = getattr(node, "name", None)
        if isinstance(name, squonk.ObjectName):
            tables.append(name.text)
    print(f"tables: {', '.join(tables)}")

    for ident in document.find_all(squonk.Ident):
        location = ident.location()
        if location is not None:
            print(f"{ident.text}: line {location.line + 1}, source={ident.source_text()!r}")


if __name__ == "__main__":
    main()
