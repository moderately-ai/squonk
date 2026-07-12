# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Moderately AI Inc.

"""The binding contract for squonk' Python API.

These tests pin the public Python boundary: typed document wrappers, raw JSON helpers,
diagnostics, tokenizer output, dialect selection, rendering, transpilation, recursion
limits, and byte-accurate spans over unicode source. A regression in the Rust boundary
or the JSON schema fails here.
"""

from __future__ import annotations

from pathlib import Path
import runpy

import pytest

import squonk


def _slice_bytes(sql: str, start: int, end: int) -> str:
    """The source text at a byte span — the interpretation spans carry (byte offsets)."""
    return sql.encode("utf-8")[start:end].decode("utf-8")


# --- parse: the happy path -------------------------------------------------------


def test_parse_returns_a_document_with_mapping_compatibility() -> None:
    tree = squonk.parse("SELECT 1")
    assert isinstance(tree, squonk.Document)
    assert tree["source"] == "SELECT 1"
    assert tree.source == "SELECT 1"
    assert tree.dialect == "ansi"
    assert len(tree["statements"]) == 1
    assert len(tree.statements) == 1
    # The recovering-only key is absent on the fail-fast path.
    assert "errors" not in tree


def test_parse_dict_returns_raw_json_shape() -> None:
    tree = squonk.parse_dict("SELECT 1")
    assert isinstance(tree, dict)
    assert tree["source"] == "SELECT 1"
    assert tree["dialect"] == "ansi"
    assert tree["resolver"]["dynamic_base"] > 1
    assert "errors" not in tree


def test_parse_multiple_statements() -> None:
    tree = squonk.parse("SELECT 1; SELECT 2")
    assert len(tree["statements"]) == 2


def test_parse_carries_the_symbol_table_for_identifiers() -> None:
    # Identifiers are interned to numeric symbols in the statement tree; their text
    # travels in the `symbols` table so the tree stays resolvable across the boundary.
    # Keyword-spelled identifiers (e.g. `name`) resolve via the static keyword table
    # and are deliberately omitted here, so this uses plainly non-keyword names.
    tree = squonk.parse("SELECT salary FROM employees")
    assert "salary" in tree["symbols"]
    assert "employees" in tree["symbols"]
    assert {"salary", "employees"}.issubset(
        {ident.text for ident in tree.find_all(squonk.Ident)}
    )


def test_schema_aware_wrappers_distinguish_object_names_from_ident_lists() -> None:
    tree = squonk.parse("CREATE TABLE public.employees (id INT, salary INT)")
    create = next(
        node
        for node in tree.find_all("CreateTable")
        if isinstance(getattr(node, "name", None), squonk.ObjectName)
    )
    assert isinstance(create.name, squonk.ObjectName)
    assert create.name.text == "public.employees"

    columns = list(tree.find_all("ColumnDef"))
    assert [column.name.text for column in columns] == ["id", "salary"]
    assert all(isinstance(column.name, squonk.Ident) for column in columns)
    assert "employees" in {ident.text for ident in tree.find_all(squonk.Ident)}


def test_document_helpers_walk_source_text_locations_and_sql() -> None:
    tree = squonk.parse("SELECT salary\nFROM employees")
    statement = tree.statements[0]
    assert next(tree.walk()) is not None
    assert statement.source_text() == tree.source
    assert statement.location() == squonk.SourceLocation(
        line=0,
        byte_column=0,
        char_column=0,
        utf16_column=0,
    )
    assert tree.to_dict() is tree.raw
    assert isinstance(tree.to_json(), str)
    assert tree.to_sql() == "SELECT salary FROM employees"


def test_parse_can_capture_trivia_on_the_document_root() -> None:
    tree = squonk.parse("/* lead */ SELECT 1", capture_trivia=True)
    assert tree.trivia
    assert tree.trivia[0].kind == "BlockComment"
    assert tree.trivia[0].text == "/* lead */"
    assert tree.trivia[0].location().line == 0
    raw = squonk.parse_dict("-- lead\nSELECT 1", capture_trivia=True)
    assert raw["trivia"][0]["kind"] == "LineComment"
    assert raw["trivia"][0]["text"] == "-- lead"


# --- parse: the error path, with byte spans --------------------------------------


def test_parse_error_raises_sql_parse_error_with_span() -> None:
    sql = "SELECT FROM t"
    with pytest.raises(squonk.SqlParseError) as excinfo:
        squonk.parse(sql)
    exc = excinfo.value
    assert exc.message
    assert str(exc) == exc.message
    # The span brackets the offending `FROM` keyword, as byte offsets into the source.
    assert isinstance(exc.span_start, int) and isinstance(exc.span_end, int)
    assert exc.span_start < exc.span_end
    assert _slice_bytes(sql, exc.span_start, exc.span_end) == "FROM"
    assert (exc.span_start, exc.span_end) == (7, 11)
    assert exc.source_text() == "FROM"
    assert exc.location() == squonk.SourceLocation(0, 7, 7, 7)


def test_parse_error_span_is_a_byte_offset_over_unicode() -> None:
    # A multibyte char before the fault shifts byte offsets past char offsets; the span
    # must still slice the source by BYTES to recover the offending token.
    sql = "SELECT 'café', FROM t"
    with pytest.raises(squonk.SqlParseError) as excinfo:
        squonk.parse(sql)
    exc = excinfo.value
    assert _slice_bytes(sql, exc.span_start, exc.span_end) == "FROM"


# --- dialect selection -----------------------------------------------------------


def test_unknown_dialect_raises_value_error_not_parse_error() -> None:
    # A bad dialect name is a caller mistake with no source span — a ValueError, not a
    # SqlParseError.
    with pytest.raises(squonk.DialectError):
        squonk.parse("SELECT 1", "klingon")


def test_generic_is_an_alias_for_ansi() -> None:
    assert squonk.parse("SELECT 1", "generic") == squonk.parse("SELECT 1", "ansi")


def test_dialect_name_is_case_insensitive() -> None:
    assert squonk.parse("SELECT 1", "ANSI") == squonk.parse("SELECT 1", "ansi")


def test_postgres_only_syntax_selected_by_dialect() -> None:
    # `$1` positional parameters are PostgreSQL-only: they parse under `postgres` and
    # are rejected under the strict `ansi` baseline.
    assert squonk.parse("SELECT $1", "postgres")["statements"]
    with pytest.raises(squonk.SqlParseError):
        squonk.parse("SELECT $1", "ansi")


# --- parse_recovering ------------------------------------------------------------


def test_recovering_collects_errors_and_keeps_good_statements() -> None:
    # Two well-formed statements around a broken one: recovery resynchronizes at each
    # `;`, so both good statements survive and the broken one is reported out of band.
    result = squonk.parse_recovering("SELECT alpha; ); SELECT gamma")
    assert isinstance(result, squonk.RecoveredDocument)
    assert len(result["statements"]) == 2
    assert len(result.errors) >= 1
    for error in result.errors:
        assert isinstance(error, squonk.Diagnostic)
        assert error.source_text()
        assert error.location() is not None
    for error in result["errors"]:
        assert error["message"]
        assert isinstance(error["span_start"], int)
        assert isinstance(error["span_end"], int)


def test_recovering_shares_the_parse_root_shape() -> None:
    result = squonk.parse_recovering("SELECT salary FROM employees")
    # A superset of the `parse` dict: same keys, plus `errors`.
    assert result["source"] == "SELECT salary FROM employees"
    assert "salary" in result["symbols"]
    assert "errors" in result


def test_recovering_clean_script_reports_no_errors() -> None:
    result = squonk.parse_recovering("SELECT 1; SELECT 2")
    assert len(result["statements"]) == 2
    assert result["errors"] == []
    assert result.errors == []


def test_parse_recovering_dict_returns_raw_json_shape() -> None:
    result = squonk.parse_recovering_dict("SELECT 1; FROM x")
    assert isinstance(result, dict)
    assert result["statements"]
    assert result["errors"]


# --- tokenize -------------------------------------------------------------------


def test_supported_dialects_exposes_names_and_aliases() -> None:
    dialects = squonk.supported_dialects()
    assert {"ansi", "postgres", "mysql", "sqlite", "duckdb"}.issubset(
        {dialect["name"] for dialect in dialects}
    )
    ansi = next(dialect for dialect in dialects if dialect["name"] == "ansi")
    assert "generic" in ansi["aliases"]
    assert squonk.validate_dialect("PG") == "postgres"
    with pytest.raises(ValueError):
        squonk.validate_dialect("klingon")


def test_schema_version_and_formatter_are_public() -> None:
    assert squonk.__schema_version__ >= 1
    assert squonk.format("select 1", keyword_case="lower").startswith("select")


def test_tokenize_returns_discriminated_token_kinds() -> None:
    result = squonk.tokenize("SELECT a + $1", "postgres")
    assert "trivia" not in result
    assert result["tokens"][0] == {
        "kind": "Keyword",
        "keyword": "select",
        "span": {"start": 0, "end": 6},
        "text": "SELECT",
    }
    assert any(
        token["kind"] == "Operator" and token["operator"] == "Plus"
        for token in result["tokens"]
    )
    assert any(token["kind"] == "Parameter" for token in result["tokens"])


def test_tokenize_can_include_trivia_with_byte_spans() -> None:
    sql = "-- café\nSELECT 1"
    result = squonk.tokenize(sql, include_trivia=True)
    assert result["trivia"][0]["kind"] == "LineComment"
    assert result["trivia"][0]["text"] == "-- café"
    span = result["trivia"][0]["span"]
    assert _slice_bytes(sql, span["start"], span["end"]) == "-- café"


def test_tokenize_unknown_dialect_and_lex_errors_are_value_errors() -> None:
    with pytest.raises(ValueError):
        squonk.tokenize("SELECT 1", "klingon")
    with pytest.raises(ValueError):
        squonk.tokenize("SELECT 'unterminated")


# --- render / transpile ----------------------------------------------------------


def test_render_accepts_sql_or_document_and_redacts_literals() -> None:
    assert squonk.render("select 1") == "SELECT 1"
    document = squonk.parse("select 1")
    assert squonk.render(document) == "SELECT 1"
    assert document.to_sql(mode="parenthesised") == "SELECT 1"
    assert squonk.redact("select 123") != "SELECT 123"


def test_render_errors_are_structured() -> None:
    with pytest.raises(squonk.SqlParseError):
        squonk.render("SELECT FROM t")
    with pytest.raises(ValueError):
        squonk.render("SELECT 1", mode="unknown")


def test_node_to_sql_renders_a_standalone_subnode() -> None:
    document = squonk.parse("SELECT a + 1 FROM t")
    binary = next(document.find_all("BinaryOp"))
    assert binary.is_renderable
    # Just the sub-tree, canonically -- not the owning SELECT.
    assert binary.to_sql() == "a + 1"
    assert binary.to_sql(mode="redacted") == "id + ?"

    # A context-dependent node (a bare identifier is not one of the four
    # standalone-renderable kinds) declines rather than emitting misleading SQL.
    ident = next(document.find_all("Ident"))
    assert not ident.is_renderable
    with pytest.raises(squonk.UnsupportedNodeRenderError) as excinfo:
        ident.to_sql()
    assert excinfo.value.kind == "unsupported_node_render"
    assert excinfo.value.source_text() == "a"


def test_structured_operation_errors_keep_source_context() -> None:
    with pytest.raises(squonk.LexError) as lex:
        squonk.tokenize("SELECT 'unterminated")
    assert lex.value.kind == "lex"

    with pytest.raises(squonk.RenderError) as render_error:
        squonk.render("SELECT 1", mode="unknown")  # type: ignore[arg-type]
    assert render_error.value.kind == "render"


def test_transpile_parses_source_and_renders_target() -> None:
    assert squonk.transpile("select 1") == "SELECT 1"
    assert squonk.transpile("select $1", "postgres", "postgres") == "SELECT $1"
    with pytest.raises(squonk.SqlParseError):
        squonk.transpile("SELECT FROM t")
    with pytest.raises(ValueError):
        squonk.transpile("SELECT 1", "klingon", "ansi")


# --- robustness: the recursion-depth guard ---------------------------------------


def test_deeply_nested_input_raises_cleanly_at_the_recursion_limit() -> None:
    # Hostile depth: the parser's recursion guard must stop with a catchable error
    # rather than overflowing the stack. The test process surviving to assert is the
    # proof there was no crash (the whole point of the DoS guard for untrusted SQL).
    depth = 300
    sql = "SELECT " + "(" * depth + "1" + ")" * depth
    with pytest.raises(squonk.SqlParseError) as excinfo:
        squonk.parse(sql)
    assert "nested" in str(excinfo.value).lower()


# --- unicode ---------------------------------------------------------------------


def test_unicode_sql_parses_and_source_round_trips() -> None:
    sql = "SELECT '日本語' AS 名前"
    tree = squonk.parse(sql)
    assert tree["source"] == sql
    assert len(tree["statements"]) == 1


# --- version ---------------------------------------------------------------------


def test_version_is_exposed() -> None:
    assert isinstance(squonk.__version__, str)
    assert squonk.__version__


# --- examples --------------------------------------------------------------------


@pytest.mark.parametrize(
    "name",
    [
        "metadata_report.py",
        "recovering_diagnostics.py",
        "render_transpile_redact.py",
    ],
)
def test_python_examples_run(name: str, capsys: pytest.CaptureFixture[str]) -> None:
    examples_dir = Path(__file__).resolve().parents[2] / "examples"
    runpy.run_path(str(examples_dir / name), run_name="__main__")
    assert capsys.readouterr().out
