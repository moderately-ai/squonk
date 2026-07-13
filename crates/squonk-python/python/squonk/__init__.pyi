# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

from __future__ import annotations

from typing import Literal, Optional, Union, overload

from ._ast import (
    Diagnostic as Diagnostic,
    Document as Document,
    Ident as Ident,
    Node as Node,
    ObjectName as ObjectName,
    RecoveredDocument as RecoveredDocument,
    SourceLocation as SourceLocation,
    Span as Span,
    Trivia as Trivia,
)
from ._exceptions import (
    DialectError as DialectError,
    FormatError as FormatError,
    LexError as LexError,
    RenderError as RenderError,
    SerializationError as SerializationError,
    SqlParseError as SqlParseError,
    SquonkError as SquonkError,
    UnsupportedNodeRenderError as UnsupportedNodeRenderError,
)
from ._types import (
    CanonicalDialectName as CanonicalDialectName,
    DiagnosticJson as DiagnosticJson,
    DialectName as DialectName,
    DialectInfoJson as DialectInfoJson,
    KeywordSymbolJson as KeywordSymbolJson,
    KeywordCase as KeywordCase,
    OperatorKind as OperatorKind,
    ParseDocumentJson as ParseDocumentJson,
    PunctuationKind as PunctuationKind,
    RecoveredDocumentJson as RecoveredDocumentJson,
    RenderMode as RenderMode,
    ResolverMetadataJson as ResolverMetadataJson,
    SourceLocationJson as SourceLocationJson,
    SpanJson as SpanJson,
    StringLiteralSyntaxJson as StringLiteralSyntaxJson,
    TokenizeResultJson as TokenizeResultJson,
    TokenJson as TokenJson,
    TriviaJson as TriviaJson,
    TriviaKind as TriviaKind,
    ValidatedDialectName as ValidatedDialectName,
)

__version__: str
__schema_version__: int

def validate_dialect(dialect: str) -> ValidatedDialectName: ...

@overload
def parse(
    sql: str,
    dialect: Literal["ansi", "generic"] = "ansi",
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["ansi"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["postgres", "postgresql", "pg"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["postgres"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["mysql", "mariadb"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["mysql"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["sqlite", "sqlite3"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["sqlite"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["duckdb", "duck"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["duckdb"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["bigquery", "bq", "zetasql"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["bigquery"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["hive", "hiveql"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["hive"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["clickhouse", "ch"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["clickhouse"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["databricks", "dbx"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["databricks"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["mssql", "tsql", "sqlserver"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["mssql"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["snowflake", "sf"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["snowflake"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["redshift", "amazonredshift"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["redshift"]]: ...
@overload
def parse(
    sql: str,
    dialect: Literal["lenient", "permissive"],
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[Literal["lenient"]]: ...
@overload
def parse(
    sql: str,
    dialect: ValidatedDialectName,
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> Document[CanonicalDialectName]: ...

def parse_with_limit(sql: str, dialect: DialectName | ValidatedDialectName = "ansi", limit: int = 128) -> Document[CanonicalDialectName]: ...

def parse_dict(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> ParseDocumentJson: ...

def parse_recovering(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> RecoveredDocument[CanonicalDialectName]: ...

def parse_recovering_dict(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    recursion_limit: Optional[int] = None,
    capture_trivia: bool = False,
    parse_float_as_decimal: bool = False,
) -> RecoveredDocumentJson: ...

def supported_dialects() -> list[DialectInfoJson]: ...

def tokenize(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    include_trivia: bool = False,
) -> TokenizeResultJson: ...

def format(
    sql: str,
    dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    indent_width: int = 2,
    max_line_length: int = 80,
    keyword_case: KeywordCase = "upper",
) -> str: ...

def render(
    sql_or_document: Union[str, Document[str]],
    dialect: Optional[DialectName | ValidatedDialectName] = None,
    *,
    mode: RenderMode = "canonical",
    recursion_limit: Optional[int] = None,
) -> str: ...

def redact(sql_or_document: Union[str, Document[str]], dialect: Optional[DialectName | ValidatedDialectName] = None) -> str: ...

def transpile(
    sql: str,
    source_dialect: DialectName | ValidatedDialectName = "ansi",
    target_dialect: DialectName | ValidatedDialectName = "ansi",
    *,
    recursion_limit: Optional[int] = None,
) -> str: ...
