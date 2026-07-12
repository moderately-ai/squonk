# squonk-python

Python bindings for `squonk`: a maturin-built Rust extension plus typed Python
views over the serialized AST.

## API

`parse()` returns a `Document`, which is both a mapping-compatible view of the raw
JSON and a typed helper object for common operations:

```python
import squonk

doc = squonk.parse("select salary from employees", dialect="ansi")

assert doc.source == "select salary from employees"
assert doc.to_sql() == "SELECT salary FROM employees"
assert doc.statements[0].to_sql() == "SELECT salary FROM employees"

# Documents are live views: editing the raw tree changes subsequent rendering.
doc.to_dict()["statements"].clear()
assert doc.to_sql() == ""

idents = [ident.text for ident in doc.find_all(squonk.Ident)]
assert {"salary", "employees"}.issubset(idents)
```

Use `parse_dict()` when you want the raw serde-compatible JSON shape:

```python
tree: squonk.ParseDocumentJson = squonk.parse_dict("select 1")
assert tree["statements"]
```

Dialect literals and aliases retain their canonical type: type checkers infer
`squonk.parse("select 1", "pg").dialect` as `Literal["postgres"]`. Validate
configuration strings with `validate_dialect()` before passing them to typed APIs.

Recovering parse keeps good statements and reports bad statements out of band:

```python
result = squonk.parse_recovering("select 1; from broken; select 2")

for diagnostic in result.errors:
    print(diagnostic.kind, diagnostic.source_text(), diagnostic.location())
```

Tokenization returns discriminated token dictionaries. Trivia capture is opt-in:

```python
tokens = squonk.tokenize("-- lead\nselect a + $1", "postgres", include_trivia=True)

assert tokens["tokens"][0]["kind"] == "Keyword"
assert tokens["tokens"][0]["keyword"] == "select"
assert tokens["trivia"][0]["kind"] == "LineComment"
```

Rendering and transpilation use Rust's renderer:

```python
assert squonk.render("select 1") == "SELECT 1"
assert squonk.redact("select 123") != "SELECT 123"
assert squonk.transpile("select $1", "postgres", "postgres") == "SELECT $1"
```

When rendering a `Document`, the document's dialect is used unless you pass an
override:

```python
doc = squonk.parse("select $1", dialect="postgres")
assert squonk.render(doc) == "SELECT $1"
```

## Types

The package ships `py.typed` plus stubs for the public API. The dict-returning
helpers expose `TypedDict` shapes such as `ParseDocumentJson`,
`RecoveredDocumentJson`, `TokenizeResultJson`, `TokenJson`, `TriviaJson`, and
`DiagnosticJson`.

The AST itself is represented as serde JSON. `Document`, `Node`, `Ident`,
`ObjectName`, `Diagnostic`, and `Trivia` provide ergonomic wrappers without hiding
the raw JSON: `to_dict()` returns Python structures and `to_json()` returns compact
JSON text. The generated `squonk.ast` module exhaustively types the serialized node
graph, while `squonk.__schema_version__` identifies its wire-schema version.
Generated child-node edges use a bounded JSON object type so mypy and Pyright do
not recursively expand the entire AST graph; annotate a known node with its named
type from `squonk.ast` when field-level precision is needed.

`ObjectName` is schema-aware: true qualified object-name fields wrap as
`ObjectName`, while plain `Ident` lists such as column lists remain lists of
`Ident` wrappers.

`Node.to_sql()` renders complete statements, queries, expressions, and data types.
Check `node.is_renderable` first when traversing arbitrary nodes; context-dependent
nodes raise `UnsupportedNodeRenderError`. All library failures derive from
`SquonkError`, with structured subclasses for parsing, dialects, tokenization,
rendering, formatting, and serialization.

## Examples

Runnable scripts live in [`examples/`](examples/):

- `metadata_report.py` parses SQL and reports identifiers, table names, source
  snippets, and canonical SQL.
- `recovering_diagnostics.py` shows statement-level recovery with byte-span
  diagnostics.
- `render_transpile_redact.py` shows canonical render, redaction, and
  source/target dialect rendering.

From the Python crate directory after `maturin develop`:

```sh
cd crates/squonk-python
uv run python examples/metadata_report.py
uv run python examples/recovering_diagnostics.py
uv run python examples/render_transpile_redact.py
```

## Development

From the Python crate directory:

```sh
cd crates/squonk-python
uv sync --group dev
uv run maturin develop
uv run pytest
uv run ruff check python
uv run mypy
uv run python -m mypy.stubtest squonk._ast squonk._exceptions squonk._native
```

The Rust boundary is checked by `cargo check -p squonk-python`; the Python tests
live under `crates/squonk-python/python/tests` and smoke-run the examples.
