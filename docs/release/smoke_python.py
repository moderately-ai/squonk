# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Release acceptance smoke test for the `squonk` wheel.

Runs against an *installed* `squonk` distribution in a clean virtualenv — it imports
the published package, never the source tree — and exercises the public Python API end
to end: version exposure, parse → typed document, render / transpile / redact round
trips, dialect selection, recovering parse, and tokenization. It has NO third-party
dependencies (no pytest), so a runbook can invoke it in a bare venv that holds only the
wheel under test:

    python -m venv /tmp/squonk-smoke
    /tmp/squonk-smoke/bin/pip install <wheel-or-'squonk'>
    /tmp/squonk-smoke/bin/python docs/release/smoke_python.py

Exits 0 on success, non-zero (with a printed reason) on the first failed check. This is
the gate both the pre-publish local-wheel test and the post-publish `pip install squonk`
test in `docs/release/python-distribution.md` run.
"""

from __future__ import annotations

import sys
from pathlib import Path


def _fail(reason: str) -> None:
    print(f"SMOKE FAIL: {reason}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    import squonk

    # The import must resolve to an installed distribution, not a checkout on sys.path —
    # a release smoke that accidentally imports the source tree proves nothing about the
    # wheel. Reject only the checkout's importable package directory; a virtualenv may
    # deliberately live under the repository while still containing the installed wheel.
    module_path = Path(squonk.__file__).resolve()
    repo_root = Path(__file__).resolve().parents[2]
    source_package = (repo_root / "crates" / "squonk-python" / "python" / "squonk").resolve()
    if module_path == source_package or source_package in module_path.parents:
        _fail(
            f"imported squonk from the source tree ({module_path}); "
            "run this from a clean venv with only the wheel installed"
        )

    if not isinstance(squonk.__version__, str) or not squonk.__version__:
        _fail(f"__version__ is not a non-empty string: {squonk.__version__!r}")
    if not isinstance(squonk.__schema_version__, int) or squonk.__schema_version__ < 1:
        _fail(f"invalid __schema_version__: {squonk.__schema_version__!r}")

    # parse → typed document, mapping + attribute views agree.
    doc = squonk.parse("select salary from employees", dialect="ansi")
    if doc.source != "select salary from employees":
        _fail(f"unexpected doc.source: {doc.source!r}")
    if len(doc.statements) != 1:
        _fail(f"expected 1 statement, got {len(doc.statements)}")
    if doc.statements[0].to_sql() != "SELECT salary FROM employees":
        _fail("node-local SQL rendering failed")
    if not isinstance(doc.to_dict(), dict) or not isinstance(doc.to_json(), str):
        _fail("document serialization helpers have the wrong shape")
    if squonk.validate_dialect("PG") != "postgres":
        _fail("dialect validation did not normalize an alias")
    if "SELECT" not in squonk.format("select 1"):
        _fail("formatter is unavailable or returned unexpected output")

    # Render round trip: parse then canonical render normalizes the source.
    rendered = squonk.render(doc)
    if rendered != "SELECT salary FROM employees":
        _fail(f"unexpected canonical render: {rendered!r}")
    # render(str) and render(Document) must agree.
    if squonk.render("select salary from employees") != rendered:
        _fail("render(str) disagrees with render(Document)")

    # Full parse → render → re-parse → render fixed point.
    reparsed = squonk.render(squonk.parse(rendered))
    if reparsed != rendered:
        _fail(f"render is not idempotent: {rendered!r} -> {reparsed!r}")

    # Redaction changes literals; transpile parses source and renders target.
    if squonk.redact("select 123") == "SELECT 123":
        _fail("redact did not rewrite the literal")
    if squonk.transpile("select $1", "postgres", "postgres") != "SELECT $1":
        _fail("postgres positional-parameter transpile failed")

    # Dialect selection: a postgres-only construct is rejected under ansi.
    if not squonk.parse("SELECT $1", "postgres")["statements"]:
        _fail("postgres $1 did not parse under postgres")
    try:
        squonk.parse("SELECT $1", "ansi")
    except squonk.SqlParseError:
        pass
    else:
        _fail("ansi accepted postgres-only $1")

    # Recovering parse keeps the good statements and reports the bad one out of band.
    recovered = squonk.parse_recovering("SELECT alpha; ); SELECT gamma")
    if len(recovered["statements"]) != 2 or len(recovered.errors) < 1:
        _fail(f"recovering parse shape wrong: {recovered['statements']}, {recovered.errors}")

    # The dialects compiled into this wheel are the promised full set.
    names = {d["name"] for d in squonk.supported_dialects()}
    missing = {"ansi", "postgres", "mysql", "sqlite", "duckdb"} - names
    if missing:
        _fail(f"wheel is missing promised dialects: {sorted(missing)}")

    # Tokenizer boundary.
    tokens = squonk.tokenize("SELECT 1")["tokens"]
    if not tokens or tokens[0]["kind"] != "Keyword":
        _fail(f"unexpected first token: {tokens[:1]}")

    print(f"SMOKE OK: squonk {squonk.__version__} from {module_path}")
    print(f"  dialects: {len(names)} compiled; render/transpile/recover/tokenize all green")


if __name__ == "__main__":
    main()
