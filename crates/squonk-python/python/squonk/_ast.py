# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""Typed Python views over squonk' JSON parse documents."""

from __future__ import annotations

from collections.abc import Iterator, Mapping
from dataclasses import dataclass
import json
from typing import Any, Generic, Optional, TypeVar, Union, cast

import squonk._native as _native
from ._ast_metadata import AST_FIELD_TYPES


JsonDict = dict[str, Any]
NativeDocument = Union[_native.NativeDocument, _native.NativeRecoveredDocument]
_TDialect = TypeVar("_TDialect", bound=str, covariant=True)


@dataclass(frozen=True)
class SourceLocation:
    """A byte offset resolved into source coordinates."""

    line: int
    byte_column: int
    char_column: int
    utf16_column: int


@dataclass(frozen=True)
class Span:
    """A half-open byte span in the original source."""

    start: int
    end: int


class Document(Mapping[str, Any], Generic[_TDialect]):
    """A live typed view over a parsed SQL document."""

    def __init__(
        self,
        raw: Optional[JsonDict] = None,
        *,
        native: Optional[NativeDocument] = None,
    ):
        if (raw is None) == (native is None):
            raise TypeError("Document requires exactly one raw mapping or native document")
        self._raw = raw
        self._native = native
        self._keyword_symbols: Optional[dict[int, str]] = None
        self._line_starts: Optional[list[int]] = None

    @property
    def raw(self) -> JsonDict:
        """The serde-compatible parse document backing this view."""
        return self._materialize()

    @property
    def source(self) -> str:
        if self._raw is None:
            assert self._native is not None
            return self._native.source
        return cast(str, self._raw["source"])

    @property
    def dialect(self) -> _TDialect:
        if self._raw is None:
            assert self._native is not None
            return cast(_TDialect, self._native.dialect)
        return cast(_TDialect, self._raw.get("dialect", "ansi"))

    @property
    def statements(self) -> list[Node]:
        return [
            _wrap_node(value, self, "Statement")
            for value in self.raw.get("statements", [])
        ]

    @property
    def trivia(self) -> list[Trivia]:
        return [Trivia(value, self) for value in self.raw.get("trivia", [])]

    @property
    def errors(self) -> list[Diagnostic]:
        return [Diagnostic(error, self) for error in self.raw.get("errors", [])]

    def resolve_symbol(self, symbol: int) -> str:
        raw = self.raw
        resolver = raw.get("resolver", {})
        dynamic_base = int(resolver.get("dynamic_base", 1))
        if symbol < dynamic_base:
            if self._keyword_symbols is None:
                self._keyword_symbols = {
                    int(entry["symbol"]): str(entry["text"])
                    for entry in resolver.get("keyword_symbols", [])
                }
            try:
                return self._keyword_symbols[symbol]
            except KeyError as error:
                raise KeyError(f"unknown keyword-backed symbol {symbol}") from error

        index = symbol - dynamic_base
        try:
            return str(raw["symbols"][index])
        except IndexError as error:
            raise KeyError(f"unknown dynamic symbol {symbol}") from error

    def source_text(self, span: Union[Span, Mapping[str, Any]]) -> str:
        actual = _span(span)
        return self.source.encode("utf-8")[actual.start : actual.end].decode("utf-8")

    def location(self, offset: int) -> SourceLocation:
        starts = self._line_start_bytes()
        line = 0
        lo, hi = 0, len(starts)
        while lo < hi:
            mid = (lo + hi) // 2
            if starts[mid] <= offset:
                line = mid
                lo = mid + 1
            else:
                hi = mid
        line_start = starts[line]
        prefix = self.source.encode("utf-8")[line_start:offset].decode("utf-8")
        return SourceLocation(
            line=line,
            byte_column=offset - line_start,
            char_column=len(prefix),
            utf16_column=sum(2 if ord(ch) > 0xFFFF else 1 for ch in prefix),
        )

    def walk(self) -> Iterator[Node]:
        stack = list(reversed(self.statements))
        while stack:
            node = stack.pop()
            yield node
            stack.extend(reversed(node.children()))

    def find_all(self, kind: Union[str, type[Node]]) -> Iterator[Node]:
        for node in self.walk():
            if isinstance(kind, str):
                if node.kind == kind:
                    yield node
            elif isinstance(node, kind):
                yield node

    def to_dict(self) -> JsonDict:
        return self.raw

    def to_json(self) -> str:
        if self._raw is None:
            assert self._native is not None
            return self._native.to_json()
        return json.dumps(self._raw, separators=(",", ":"))

    def to_sql(self, *, dialect: Optional[str] = None, mode: str = "canonical") -> str:
        from ._exceptions import SquonkError

        try:
            if self._raw is None:
                assert self._native is not None
                return self._native.render(dialect or self.dialect, mode)
            return _native.render_document(
                self.to_json(),
                dialect or self.dialect,
                mode,
            )
        except SquonkError as error:
            raise error._with_source(self.source) from None

    def __getitem__(self, key: str) -> Any:
        return self.raw[key]

    def __iter__(self) -> Iterator[str]:
        return iter(self.raw)

    def __len__(self) -> int:
        return len(self.raw)

    def __eq__(self, other: object) -> bool:
        if isinstance(other, Document):
            return self.raw == other.raw
        return self.raw == other

    def _materialize(self) -> JsonDict:
        if self._raw is None:
            assert self._native is not None
            self._raw = cast(JsonDict, json.loads(self._native.to_json()))
            self._native = None
        return self._raw

    def _line_start_bytes(self) -> list[int]:
        if self._line_starts is None:
            starts = [0]
            for index, byte in enumerate(self.source.encode("utf-8")):
                if byte == 0x0A:
                    starts.append(index + 1)
            self._line_starts = starts
        return self._line_starts


class RecoveredDocument(Document[_TDialect], Generic[_TDialect]):
    """A parsed document plus statement-level recovery diagnostics."""


class Node:
    """A typed view over one AST node or enum variant."""

    def __init__(self, raw: JsonDict, document: Document[str], type_name: Optional[str] = None):
        self._raw = raw
        self.document = document
        self.type_name = type_name
        kind, data, is_variant = _node_kind_and_data(raw)
        self.kind = type_name if type_name is not None and not is_variant else kind
        self._data = data

    @property
    def raw(self) -> JsonDict:
        return self._raw

    @property
    def span(self) -> Optional[Span]:
        meta = self._data.get("meta") if isinstance(self._data, dict) else None
        if not isinstance(meta, dict) or "span" not in meta:
            return None
        return _span(meta["span"])

    @property
    def node_id(self) -> Optional[int]:
        meta = self._data.get("meta") if isinstance(self._data, dict) else None
        if not isinstance(meta, dict):
            return None
        value = meta.get("node_id")
        return int(value) if value is not None else None

    @property
    def is_renderable(self) -> bool:
        """Whether this node is a standalone statement, query, expression, or type."""
        return self.type_name in {"Statement", "Query", "Expr", "DataType"}

    def source_text(self) -> Optional[str]:
        if self.span is None:
            return None
        return self.document.source_text(self.span)

    def location(self) -> Optional[SourceLocation]:
        if self.span is None:
            return None
        return self.document.location(self.span.start)

    def children(self) -> list[Node]:
        children: list[Node] = []
        for field, value in _iter_child_entries(self._data):
            wrapped = _wrap(
                value,
                self.document,
                _field_type(self.type_name, self.kind, field),
            )
            _collect_nodes(wrapped, children)
        return children

    def walk(self) -> Iterator[Node]:
        stack = [self]
        while stack:
            node = stack.pop()
            yield node
            stack.extend(reversed(node.children()))

    def find_all(self, kind: Union[str, type[Node]]) -> Iterator[Node]:
        for node in self.walk():
            if isinstance(kind, str):
                if node.kind == kind:
                    yield node
            elif isinstance(node, kind):
                yield node

    def to_dict(self) -> JsonDict:
        return self._raw

    def to_sql(self, *, dialect: Optional[str] = None, mode: str = "canonical") -> str:
        """Render just this node as a canonical SQL fragment (not its owning document).

        Only standalone-renderable nodes -- a complete expression, query, statement, or
        data type -- are supported; a context-dependent node (a join constraint, select
        item, order-by term) raises ``ValueError`` rather than emitting misleading SQL.
        The node must carry a parser-assigned id.
        """
        from ._exceptions import SquonkError

        if not self.is_renderable:
            from ._exceptions import UnsupportedNodeRenderError

            span = self.span
            raise UnsupportedNodeRenderError(
                f"{self.kind} requires surrounding SQL context",
                span.start if span is not None else None,
                span.end if span is not None else None,
            )._with_source(self.document.source)
        node_id = self.node_id
        if node_id is None:
            from ._exceptions import UnsupportedNodeRenderError

            raise UnsupportedNodeRenderError(
                "to_sql() requires a parser-assigned node id"
            )._with_source(self.document.source)
        try:
            return _native.render_fragment(
                self.document.to_json(),
                node_id,
                dialect or self.document.dialect,
                mode,
            )
        except SquonkError as error:
            raise error._with_source(self.document.source) from None

    def __getattr__(self, name: str) -> Any:
        if isinstance(self._data, dict) and name in self._data:
            return _wrap(
                self._data[name],
                self.document,
                _field_type(self.type_name, self.kind, name),
            )
        raise AttributeError(name)

    def __getitem__(self, key: str) -> Any:
        if isinstance(self._data, dict):
            return self._data[key]
        raise TypeError(f"{self.kind} does not expose mapping fields")

    def __repr__(self) -> str:
        return f"{type(self).__name__}(kind={self.kind!r})"


class Ident(Node):
    """An identifier node with resolved text."""

    def __init__(self, raw: JsonDict, document: Document[str]):
        super().__init__(raw, document, "Ident")
        self.kind = "Ident"
        self._data = raw

    @property
    def symbol(self) -> int:
        return int(self._data["sym"])

    @property
    def text(self) -> str:
        return self.document.resolve_symbol(self.symbol)

    @property
    def quote(self) -> str:
        return str(self._data["quote"])


class ObjectName:
    """A qualified object name backed by one or more identifiers."""

    def __init__(self, raw: list[Any], document: Document[str]):
        self._raw = raw
        self.document = document
        self.parts = [Ident(part, document) for part in raw]

    @property
    def text(self) -> str:
        return ".".join(part.text for part in self.parts)

    def to_dict(self) -> list[Any]:
        return self._raw

    def __iter__(self) -> Iterator[Ident]:
        return iter(self.parts)

    def __len__(self) -> int:
        return len(self.parts)

    def __getitem__(self, index: int) -> Ident:
        return self.parts[index]

    def __repr__(self) -> str:
        return f"ObjectName({self.text!r})"


class Diagnostic(Mapping[str, Any]):
    """A parse diagnostic with byte-span helpers."""

    def __init__(self, raw: JsonDict, document: Document[str]):
        self.raw = raw
        self.document = document

    @property
    def message(self) -> str:
        return str(self.raw["message"])

    @property
    def kind(self) -> str:
        return str(self.raw.get("kind", "syntax"))

    @property
    def span(self) -> Optional[Span]:
        span = self.raw.get("span")
        return _span(span) if isinstance(span, Mapping) else None

    def source_text(self) -> Optional[str]:
        if self.span is None:
            return None
        return self.document.source_text(self.span)

    def location(self) -> Optional[SourceLocation]:
        if self.span is None:
            return None
        return self.document.location(self.span.start)

    def __getitem__(self, key: str) -> Any:
        return self.raw[key]

    def __iter__(self) -> Iterator[str]:
        return iter(self.raw)

    def __len__(self) -> int:
        return len(self.raw)


class Trivia(Mapping[str, Any]):
    """A captured whitespace/comment run with byte-span helpers."""

    def __init__(self, raw: JsonDict, document: Document[str]):
        self.raw = raw
        self.document = document

    @property
    def kind(self) -> str:
        return str(self.raw["kind"])

    @property
    def span(self) -> Span:
        return _span(self.raw["span"])

    @property
    def text(self) -> str:
        return str(self.raw["text"])

    def location(self) -> SourceLocation:
        return self.document.location(self.span.start)

    def __getitem__(self, key: str) -> Any:
        return self.raw[key]

    def __iter__(self) -> Iterator[str]:
        return iter(self.raw)

    def __len__(self) -> int:
        return len(self.raw)


def document_from_json(text: str, *, recovered: bool = False) -> Document[str]:
    raw = json.loads(text)
    cls = RecoveredDocument if recovered else Document
    return cls(raw)


def document_from_native(
    native: NativeDocument, *, recovered: bool = False
) -> Document[str]:
    cls = RecoveredDocument if recovered else Document
    return cls(native=native)


def _wrap(value: Any, document: Document[str], type_spec: Optional[str] = None) -> Any:
    if value is None:
        return None
    if type_spec == "ObjectName":
        return ObjectName(value, document)
    if type_spec == "Ident":
        return Ident(value, document)
    if isinstance(value, dict):
        if _is_ident(value):
            return Ident(value, document)
        return Node(value, document, _node_type(type_spec))
    if isinstance(value, list):
        return [_wrap(item, document, _array_element_type(type_spec)) for item in value]
    return value


def _wrap_node(value: Any, document: Document[str], type_spec: Optional[str] = None) -> Node:
    wrapped = _wrap(value, document, type_spec)
    if isinstance(wrapped, Node):
        return wrapped
    raise TypeError(f"expected AST node object, got {type(value).__name__}")


def _node_kind_and_data(raw: JsonDict) -> tuple[str, Any, bool]:
    if len(raw) == 1:
        key, value = next(iter(raw.items()))
        if key[:1].isupper():
            return key, value, True
    return type(raw).__name__, raw, False


def _is_ident(value: JsonDict) -> bool:
    return {"sym", "quote", "meta"}.issubset(value)


def _iter_child_entries(value: Any) -> Iterator[tuple[str, Any]]:
    if isinstance(value, dict):
        for key, child in value.items():
            if key == "meta":
                continue
            yield key, child
    elif isinstance(value, list):
        for index, item in enumerate(value):
            yield str(index), item


def _field_type(type_name: Optional[str], kind: str, field: str) -> Optional[str]:
    if type_name is not None:
        variant_fields = AST_FIELD_TYPES.get(f"{type_name}.{kind}")
        if variant_fields is not None and field in variant_fields:
            return cast(str, variant_fields[field])
        fields = AST_FIELD_TYPES.get(type_name)
        if fields is not None and field in fields:
            return cast(str, fields[field])
    return None


def _array_element_type(type_spec: Optional[str]) -> Optional[str]:
    if isinstance(type_spec, str) and type_spec.endswith("[]"):
        return type_spec[:-2]
    return None


def _node_type(type_spec: Optional[str]) -> Optional[str]:
    if not isinstance(type_spec, str) or type_spec == "NoExt" or type_spec.endswith("[]"):
        return None
    return type_spec


def _collect_nodes(value: Any, out: list[Node]) -> None:
    if isinstance(value, Node):
        out.append(value)
    elif isinstance(value, ObjectName):
        out.extend(value.parts)
    elif isinstance(value, list):
        for item in value:
            _collect_nodes(item, out)


def _span(value: Union[Span, Mapping[str, Any]]) -> Span:
    if isinstance(value, Span):
        return value
    return Span(start=int(value["start"]), end=int(value["end"]))
