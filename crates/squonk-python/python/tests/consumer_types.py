# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Cold-consumer typing contract shared by mypy and Pyright."""

from typing import Literal, assert_type

from squonk import (
    DialectName,
    Document,
    Node,
    ParseDocumentJson,
    RenderMode,
    SquonkError,
    parse,
    parse_dict,
    render,
)
from squonk.ast import StatementJson

document = parse("select 1", "pg")
assert_type(document, Document[Literal["postgres"]])
assert_type(document.dialect, Literal["postgres"])
assert_type(document.statements[0], Node[StatementJson])

raw = parse_dict("select 1")
assert_type(raw, ParseDocumentJson)
raw["statements"]

dialect: DialectName = "postgresql"
mode: RenderMode = "redacted"
assert_type(render("select 1", dialect, mode=mode), str)

try:
    parse("select from")
except SquonkError as error:
    assert_type(error.kind, str)
