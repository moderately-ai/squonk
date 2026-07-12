# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

from typing import ClassVar, Optional
from ._ast import SourceLocation

class SquonkError(Exception):
    default_kind: ClassVar[str]
    message: str
    span_start: Optional[int]
    span_end: Optional[int]
    kind: str
    expected: Optional[str]
    found: Optional[str]
    span: Optional[dict[str, int]]
    def __init__(
        self,
        message: str,
        span_start: Optional[int] = None,
        span_end: Optional[int] = None,
        kind: Optional[str] = None,
        expected: Optional[str] = None,
        found: Optional[str] = None,
    ) -> None: ...
    def source_text(self) -> Optional[str]: ...
    def location(self) -> Optional[SourceLocation]: ...

class SqlParseError(SquonkError):
    default_kind: ClassVar[str]
class DialectError(SquonkError, ValueError):
    default_kind: ClassVar[str]
class LexError(SquonkError, ValueError):
    default_kind: ClassVar[str]
class RenderError(SquonkError, ValueError):
    default_kind: ClassVar[str]
class UnsupportedNodeRenderError(RenderError):
    default_kind: ClassVar[str]
class FormatError(SquonkError, ValueError):
    default_kind: ClassVar[str]
class SerializationError(SquonkError):
    default_kind: ClassVar[str]
