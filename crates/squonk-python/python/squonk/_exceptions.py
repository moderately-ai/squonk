# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Structured exceptions raised by Squonk's Python interface."""

from __future__ import annotations

from typing import TYPE_CHECKING, Optional

if TYPE_CHECKING:
    from ._ast import SourceLocation


class SquonkError(Exception):
    """Base class for failures reported by Squonk."""

    default_kind = "binding"

    def __init__(
        self,
        message: str,
        span_start: Optional[int] = None,
        span_end: Optional[int] = None,
        kind: Optional[str] = None,
        expected: Optional[str] = None,
        found: Optional[str] = None,
    ) -> None:
        super().__init__(message)
        self.message = message
        self.span_start = span_start
        self.span_end = span_end
        self.kind = kind or self.default_kind
        self.expected = expected
        self.found = found
        self.span = (
            {"start": span_start, "end": span_end}
            if span_start is not None and span_end is not None
            else None
        )
        self._source: Optional[str] = None

    def _with_source(self, source: str) -> SquonkError:
        self._source = source
        return self

    def source_text(self) -> Optional[str]:
        """Return the exact source text covered by this error."""
        if self._source is None or self.span_start is None or self.span_end is None:
            return None
        return self._source.encode("utf-8")[self.span_start : self.span_end].decode("utf-8")

    def location(self) -> Optional[SourceLocation]:
        """Resolve the error start into source coordinates."""
        if self._source is None or self.span_start is None:
            return None
        from ._ast import Document

        return Document({"source": self._source}).location(self.span_start)


class SqlParseError(SquonkError):
    """SQL source failed to parse."""

    default_kind = "syntax"


class DialectError(SquonkError, ValueError):
    """A dialect name is unknown or unavailable."""

    default_kind = "unknown_dialect"


class LexError(SquonkError, ValueError):
    """SQL source failed to tokenize."""

    default_kind = "lex"


class RenderError(SquonkError, ValueError):
    """A document or fragment could not be rendered."""

    default_kind = "render"


class UnsupportedNodeRenderError(RenderError):
    """A context-dependent node cannot render as standalone SQL."""

    default_kind = "unsupported_node_render"


class FormatError(SquonkError, ValueError):
    """SQL source could not be formatted with the requested options."""

    default_kind = "format"


class SerializationError(SquonkError):
    """The binding could not serialize or deserialize an AST payload."""

    default_kind = "serialization"
