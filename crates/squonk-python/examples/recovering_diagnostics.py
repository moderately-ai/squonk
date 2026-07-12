# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Recover around statement-level SQL errors and print source diagnostics."""

from __future__ import annotations

import squonk


SQL = "SELECT alpha; FROM broken; SELECT gamma"


def main() -> None:
    document = squonk.parse_recovering(SQL, dialect="ansi")

    print(f"valid statements: {len(document.statements)}")
    print(f"diagnostics: {len(document.errors)}")
    for diagnostic in document.errors:
        location = diagnostic.location()
        where = f"line {location.line + 1}" if location is not None else "unknown location"
        print(f"{diagnostic.kind}: {diagnostic.message} at {where}")
        print(f"source: {diagnostic.source_text()!r}")


if __name__ == "__main__":
    main()
