# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""squonk — a fast, multi-dialect SQL parser with typed Python views."""

from __future__ import annotations

import json
from collections.abc import Callable
from typing import Optional, TypeVar, Union

from . import _native
from ._ast import (
    Diagnostic,
    Document,
    Ident,
    Node,
    ObjectName,
    RecoveredDocument,
    SourceLocation,
    Span,
    Trivia,
    document_from_json,
)
from ._exceptions import (
    DialectError,
    FormatError,
    LexError,
    RenderError,
    SerializationError,
    SqlParseError,
    SquonkError,
    UnsupportedNodeRenderError,
)
from ._types import (
    CanonicalDialectName,
    DiagnosticJson,
    DialectName,
    DialectInfoJson,
    KeywordSymbolJson,
    KeywordCase,
    OperatorKind,
    ParseDocumentJson,
    PunctuationKind,
    RecoveredDocumentJson,
    RenderMode,
    ResolverMetadataJson,
    SourceLocationJson,
    SpanJson,
    StringLiteralSyntaxJson,
    TokenizeResultJson,
    TokenJson,
    TriviaJson,
    TriviaKind,
    ValidatedDialectName,
)

__version__: str = _native.__version__
__schema_version__: int = _native.__schema_version__

_DIALECTS = {
    alias: canonical
    for canonical, aliases in {
        "ansi": ("ansi", "generic"), "postgres": ("postgres", "postgresql", "pg"),
        "mysql": ("mysql", "mariadb"), "sqlite": ("sqlite", "sqlite3"),
        "duckdb": ("duckdb", "duck"), "bigquery": ("bigquery", "bq", "zetasql"),
        "hive": ("hive", "hiveql"), "clickhouse": ("clickhouse", "ch"),
        "databricks": ("databricks", "dbx"),
        "mssql": ("mssql", "tsql", "sqlserver"), "snowflake": ("snowflake", "sf"),
        "redshift": ("redshift", "amazonredshift"),
        "lenient": ("lenient", "permissive"),
    }.items()
    for alias in aliases
}


def validate_dialect(dialect: str) -> ValidatedDialectName:
    """Validate and normalize a built-in dialect name or alias."""
    canonical = _DIALECTS.get(dialect.lower())
    if canonical is None:
        raise DialectError(
            f"unknown SQL dialect {dialect!r}; valid names are {', '.join(_DIALECTS)}"
        )
    return ValidatedDialectName(canonical)


T = TypeVar("T")


def _call_with_source(sql: str, call: Callable[[], T]) -> T:
    try:
        return call()
    except SquonkError as error:
        raise error._with_source(sql) from None

__all__ = [
    "Diagnostic",
    "CanonicalDialectName",
    "DialectName",
    "KeywordCase",
    "RenderMode",
    "ValidatedDialectName",
    "Document",
    "DialectError",
    "FormatError",
    "Ident",
    "Node",
    "LexError",
    "ObjectName",
    "RecoveredDocument",
    "SourceLocation",
    "Span",
    "SqlParseError",
    "SquonkError",
    "RenderError",
    "SerializationError",
    "UnsupportedNodeRenderError",
    "Trivia",
    "DiagnosticJson",
    "DialectInfoJson",
    "KeywordSymbolJson",
    "OperatorKind",
    "ParseDocumentJson",
    "PunctuationKind",
    "RecoveredDocumentJson",
    "ResolverMetadataJson",
    "SourceLocationJson",
    "SpanJson",
    "StringLiteralSyntaxJson",
    "TokenizeResultJson",
    "TokenJson",
    "TriviaJson",
    "TriviaKind",
    "__version__",
    "__schema_version__",
    "format",
    "parse",
    "parse_dict",
    "parse_recovering",
    "parse_recovering_dict",
    "parse_with_limit",
    "redact",
    "render",
    "supported_dialects",
    "tokenize",
    "transpile",
    "validate_dialect",
]


def parse(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document:
    """Parse ``sql`` and return a typed :class:`Document` view."""
    raw = _call_with_source(
        sql,
        lambda: _native.parse(
            sql, dialect, recursion_limit, capture_trivia, parse_float_as_decimal
        ),
    )
    return document_from_json(raw)


def parse_with_limit(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    limit: int = 128,
) -> Document:
    """Parse ``sql`` with an explicit recursion limit."""
    return parse(sql, dialect, recursion_limit=limit)


def parse_dict(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> ParseDocumentJson:
    """Parse ``sql`` and return the raw serde-compatible dict."""
    return parse(
        sql,
        dialect,
        recursion_limit=recursion_limit,
        capture_trivia=capture_trivia,
        parse_float_as_decimal=parse_float_as_decimal,
    ).to_dict()


def parse_recovering(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> RecoveredDocument:
    """Parse ``sql`` while collecting statement-level diagnostics."""
    document = document_from_json(
        _call_with_source(
            sql,
            lambda: _native.parse_recovering(
            sql, dialect, recursion_limit, capture_trivia, parse_float_as_decimal
            ),
        ),
        recovered=True,
    )
    assert isinstance(document, RecoveredDocument)
    return document


def parse_recovering_dict(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> RecoveredDocumentJson:
    """Recovering parse returning the raw serde-compatible dict."""
    return parse_recovering(
        sql,
        dialect,
        recursion_limit=recursion_limit,
        capture_trivia=capture_trivia,
        parse_float_as_decimal=parse_float_as_decimal,
    ).to_dict()


def supported_dialects() -> list[DialectInfoJson]:
    """Return the built-in dialects compiled into this wheel."""
    return json.loads(_native.supported_dialects())


def tokenize(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    include_trivia: bool = False,
) -> TokenizeResultJson:
    """Tokenize ``sql`` under ``dialect``."""
    return json.loads(
        _call_with_source(sql, lambda: _native.tokenize(sql, dialect, include_trivia))
    )


def format(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    indent_width: int = 2,
    max_line_length: int = 80,
    keyword_case: KeywordCase = "upper",
) -> str:
    """Pretty-print SQL with stable style controls."""
    return _call_with_source(
        sql,
        lambda: _native.format(
            sql, dialect, indent_width, max_line_length, keyword_case
        ),
    )


def render(
    sql_or_document: Union[str, Document],
    dialect: Optional[DialectName | ValidatedDialectName] = None,
    *,
    mode: RenderMode = "canonical",
    recursion_limit: Optional[int] = None,
) -> str:
    """Render SQL or a parsed :class:`Document` through Rust's renderer."""
    if isinstance(sql_or_document, Document):
        return sql_or_document.to_sql(dialect=dialect, mode=mode)
    return _call_with_source(
        sql_or_document,
        lambda: _native.render_sql(
            sql_or_document, dialect or "ansi", mode, recursion_limit
        ),
    )


def redact(
    sql_or_document: Union[str, Document],
    dialect: Optional[DialectName | ValidatedDialectName] = None,
) -> str:
    """Render a redacted SQL fingerprint."""
    return render(sql_or_document, dialect, mode="redacted")


def transpile(
    sql: str,
    source_dialect: DialectName | ValidatedDialectName = "ansi",
    target_dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    recursion_limit: Optional[int] = None,
) -> str:
    """Parse under ``source_dialect`` and render for ``target_dialect``."""
    return _call_with_source(
        sql,
        lambda: _native.transpile(
            sql, source_dialect, target_dialect, recursion_limit
        ),
    )
