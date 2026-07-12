# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Public JSON shape types for squonk' Python API."""

from __future__ import annotations

from typing import TYPE_CHECKING, Literal, NewType, Optional, TypedDict, Union

if TYPE_CHECKING:
    from .ast import StatementJson
else:
    StatementJson = dict[str, object]

CanonicalDialectName = Literal[
    "ansi", "postgres", "mysql", "sqlite", "duckdb", "bigquery", "hive",
    "clickhouse", "databricks", "mssql", "snowflake", "redshift", "lenient",
]
DialectAlias = Literal[
    "generic", "postgresql", "pg", "mariadb", "sqlite3", "duck", "bq",
    "zetasql", "hiveql", "ch", "dbx", "tsql", "sqlserver", "sf",
    "amazonredshift", "permissive",
]
DialectName = Union[CanonicalDialectName, DialectAlias]
ValidatedDialectName = NewType("ValidatedDialectName", str)
RenderMode = Literal["canonical", "redacted", "parenthesized", "parenthesised"]
KeywordCase = Literal["upper", "lower", "preserve"]


class SpanJson(TypedDict):
    """A half-open byte range in the original SQL source."""

    start: int
    end: int


class SourceLocationJson(TypedDict):
    """A byte offset resolved into source coordinates."""

    line: int
    byte_column: int
    char_column: int
    utf16_column: int


class StringLiteralSyntaxJson(TypedDict):
    escape_strings: bool
    dollar_quoted_strings: bool
    national_strings: bool
    double_quoted_strings: bool
    backslash_escapes: bool
    unicode_strings: bool
    bit_string_literals: bool
    charset_introducers: bool
    same_line_adjacent_concat: bool


class KeywordSymbolJson(TypedDict):
    symbol: int
    text: str


class ResolverMetadataJson(TypedDict):
    dynamic_base: int
    keyword_symbols: list[KeywordSymbolJson]


class _DiagnosticRequiredJson(TypedDict):
    message: str
    kind: str
    span: Optional[SpanJson]


class DiagnosticJson(_DiagnosticRequiredJson, total=False):
    span_start: int
    span_end: int
    expected: str
    found: str


OperatorKind = Literal[
    "Plus",
    "Minus",
    "Star",
    "Slash",
    "SlashSlash",
    "Percent",
    "Eq",
    "EqEq",
    "Lt",
    "LtEq",
    "Gt",
    "GtEq",
    "NotEq",
    "LtEqGt",
    "Concat",
    "AmpAmp",
    "Bang",
    "Pipe",
    "Amp",
    "Caret",
    "Tilde",
    "ShiftLeft",
    "ShiftRight",
    "Hash",
    "Arrow",
    "ColonEquals",
    "AtGt",
    "LtAt",
    "MinusGt",
    "MinusGtGt",
]

PunctuationKind = Literal[
    "LParen",
    "RParen",
    "Comma",
    "Semicolon",
    "Dot",
    "LBracket",
    "RBracket",
    "LBrace",
    "RBrace",
    "Colon",
    "DoubleColon",
]

TriviaKind = Literal["LineComment", "BlockComment", "Whitespace"]


class _TokenBaseJson(TypedDict):
    span: SpanJson
    text: str


class WordTokenJson(_TokenBaseJson):
    kind: Literal["Word"]


class KeywordTokenJson(_TokenBaseJson):
    kind: Literal["Keyword"]
    keyword: str


class NumberTokenJson(_TokenBaseJson):
    kind: Literal["Number"]


class StringTokenJson(_TokenBaseJson):
    kind: Literal["String"]


class QuotedIdentTokenJson(_TokenBaseJson):
    kind: Literal["QuotedIdent"]


class ParameterTokenJson(_TokenBaseJson):
    kind: Literal["Parameter"]


class PositionalColumnTokenJson(_TokenBaseJson):
    kind: Literal["PositionalColumn"]


class VariableTokenJson(_TokenBaseJson):
    kind: Literal["Variable"]


class OperatorTokenJson(_TokenBaseJson):
    kind: Literal["Operator"]
    operator: OperatorKind


class PunctuationTokenJson(_TokenBaseJson):
    kind: Literal["Punctuation"]
    punctuation: PunctuationKind


class UnknownTokenJson(_TokenBaseJson):
    kind: Literal["Unknown"]


TokenJson = Union[
    WordTokenJson,
    KeywordTokenJson,
    NumberTokenJson,
    StringTokenJson,
    QuotedIdentTokenJson,
    ParameterTokenJson,
    PositionalColumnTokenJson,
    VariableTokenJson,
    OperatorTokenJson,
    PunctuationTokenJson,
    UnknownTokenJson,
]


class TriviaJson(TypedDict):
    kind: TriviaKind
    span: SpanJson
    text: str


class _ParseDocumentRequiredJson(TypedDict):
    dialect: str
    source: str
    symbols: list[str]
    resolver: ResolverMetadataJson
    string_literals: StringLiteralSyntaxJson
    statements: list[StatementJson]


class ParseDocumentJson(_ParseDocumentRequiredJson, total=False):
    trivia: list[TriviaJson]


class RecoveredDocumentJson(ParseDocumentJson):
    errors: list[DiagnosticJson]


class DialectInfoJson(TypedDict):
    name: str
    aliases: list[str]


class _TokenizeResultRequiredJson(TypedDict):
    source: str
    dialect: str
    tokens: list[TokenJson]


class TokenizeResultJson(_TokenizeResultRequiredJson, total=False):
    trivia: list[TriviaJson]
