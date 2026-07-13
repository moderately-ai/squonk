// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Python bindings for `squonk`, packaged by maturin as `squonk._native`.
//!
//! Rust owns parsing, diagnostics, tokenization, and rendering. The Python package
//! receives JSON strings and layers typed wrapper objects on top, keeping this
//! boundary small and avoiding hand-built Python dictionaries in Rust.

use pyo3::import_exception;
use pyo3::prelude::*;

use squonk::bindings::{
    ParseDiagnostic, ParseDocument, RecoveredDocument, TokenizeDocument, WIRE_SCHEMA_VERSION,
};
use squonk::render::{RenderConfig, RenderMode, Renderer};
use squonk::{
    BuiltinDialect, ParseConfig, Parsed, Recovered, parse_builtin_with,
    parse_recovering_builtin_with, tokenize_with_builtin, tokenize_with_builtin_trivia,
};

import_exception!(squonk, SqlParseError);
import_exception!(squonk, DialectError);
import_exception!(squonk, FormatError);
import_exception!(squonk, LexError);
import_exception!(squonk, RenderError);
import_exception!(squonk, SerializationError);
import_exception!(squonk, UnsupportedNodeRenderError);

#[pyclass(frozen, module = "squonk._native")]
struct NativeDocument {
    parsed: Parsed,
    dialect: BuiltinDialect,
}

#[pymethods]
impl NativeDocument {
    #[getter]
    fn source(&self) -> &str {
        self.parsed.source()
    }

    #[getter]
    fn dialect(&self) -> &'static str {
        self.dialect.name()
    }

    fn to_json(&self) -> PyResult<String> {
        to_json(&ParseDocument::new(&self.parsed, self.dialect))
    }

    #[pyo3(signature = (dialect, mode = "canonical"))]
    fn render(&self, dialect: &str, mode: &str) -> PyResult<String> {
        render_parsed(&self.parsed, resolve_dialect(dialect)?, mode)
    }

    #[pyo3(signature = (node_id, dialect, mode = "canonical"))]
    fn render_fragment(&self, node_id: u32, dialect: &str, mode: &str) -> PyResult<String> {
        render_parsed_fragment(&self.parsed, node_id, resolve_dialect(dialect)?, mode)
    }
}

#[pyclass(frozen, module = "squonk._native")]
struct NativeRecoveredDocument {
    recovered: Recovered,
    dialect: BuiltinDialect,
}

#[pymethods]
impl NativeRecoveredDocument {
    #[getter]
    fn source(&self) -> &str {
        self.recovered.parsed().source()
    }

    #[getter]
    fn dialect(&self) -> &'static str {
        self.dialect.name()
    }

    fn to_json(&self) -> PyResult<String> {
        to_json(&RecoveredDocument::new(&self.recovered, self.dialect))
    }

    #[pyo3(signature = (dialect, mode = "canonical"))]
    fn render(&self, dialect: &str, mode: &str) -> PyResult<String> {
        render_parsed(self.recovered.parsed(), resolve_dialect(dialect)?, mode)
    }

    #[pyo3(signature = (node_id, dialect, mode = "canonical"))]
    fn render_fragment(&self, node_id: u32, dialect: &str, mode: &str) -> PyResult<String> {
        render_parsed_fragment(
            self.recovered.parsed(),
            node_id,
            resolve_dialect(dialect)?,
            mode,
        )
    }
}

#[pyfunction]
#[pyo3(signature = (sql, dialect = "ansi", recursion_limit = None, capture_trivia = false, parse_float_as_decimal = false))]
fn parse_document(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<usize>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> PyResult<NativeDocument> {
    let dialect = resolve_dialect(dialect)?;
    let config = parse_config(
        dialect,
        recursion_limit,
        capture_trivia,
        parse_float_as_decimal,
    );
    let parsed = Python::attach(|py| py.detach(|| parse_builtin_with(sql, config)))
        .map_err(|error| parse_error_to_pyerr(&error))?;
    Ok(NativeDocument { parsed, dialect })
}

#[pyfunction]
#[pyo3(signature = (sql, dialect = "ansi", recursion_limit = None, capture_trivia = false, parse_float_as_decimal = false))]
fn parse_recovering_document(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<usize>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> PyResult<NativeRecoveredDocument> {
    let dialect = resolve_dialect(dialect)?;
    let config = parse_config(
        dialect,
        recursion_limit,
        capture_trivia,
        parse_float_as_decimal,
    );
    let recovered = Python::attach(|py| py.detach(|| parse_recovering_builtin_with(sql, config)))
        .map_err(|error| parse_error_to_pyerr(&error))?;
    Ok(NativeRecoveredDocument { recovered, dialect })
}

#[pyfunction]
#[pyo3(signature = (sql, dialect = "ansi", recursion_limit = None, capture_trivia = false, parse_float_as_decimal = false))]
fn parse(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<usize>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> PyResult<String> {
    let builtin = resolve_dialect(dialect)?;
    let config = parse_config(
        builtin,
        recursion_limit,
        capture_trivia,
        parse_float_as_decimal,
    );
    match parse_builtin_with(sql, config) {
        Ok(parsed) => to_json(&ParseDocument::new(&parsed, builtin)),
        Err(error) => Err(parse_error_to_pyerr(&error)),
    }
}

#[pyfunction]
#[pyo3(signature = (sql, dialect = "ansi", recursion_limit = None, capture_trivia = false, parse_float_as_decimal = false))]
fn parse_recovering(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<usize>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> PyResult<String> {
    let builtin = resolve_dialect(dialect)?;
    let config = parse_config(
        builtin,
        recursion_limit,
        capture_trivia,
        parse_float_as_decimal,
    );
    let recovered =
        parse_recovering_builtin_with(sql, config).map_err(|error| parse_error_to_pyerr(&error))?;
    to_json(&RecoveredDocument::new(&recovered, builtin))
}

#[pyfunction]
fn supported_dialects() -> PyResult<String> {
    to_json(&squonk::bindings::supported_dialects())
}

#[pyfunction]
#[pyo3(signature = (sql, dialect = "ansi", include_trivia = false))]
fn tokenize(sql: &str, dialect: &str, include_trivia: bool) -> PyResult<String> {
    let builtin = resolve_dialect(dialect)?;
    let tokens = if include_trivia {
        let (tokens, trivia) =
            tokenize_with_builtin_trivia(sql, builtin).map_err(lex_error_to_pyerr)?;
        to_json(&TokenizeDocument::with_trivia(
            sql,
            builtin,
            &tokens,
            trivia.all(),
        ))?
    } else {
        let tokens = tokenize_with_builtin(sql, builtin).map_err(lex_error_to_pyerr)?;
        to_json(&TokenizeDocument::new(sql, builtin, &tokens))?
    };
    Ok(tokens)
}

#[pyfunction]
#[pyo3(signature = (document_json, dialect = "ansi", mode = "canonical"))]
fn render_document(document_json: &str, dialect: &str, mode: &str) -> PyResult<String> {
    let builtin = resolve_dialect(dialect)?;
    let parsed: Parsed = serde_json::from_str(document_json).map_err(|error| {
        SerializationError::new_err(format!("failed to deserialize parse document: {error}"))
    })?;
    render_parsed(&parsed, builtin, mode)
}

/// Render a single sub-node of a serialized parse document, selected by node id.
///
/// The fragment counterpart of [`render_document`]: only standalone-renderable node
/// kinds (a complete expression, query, statement, or data type) resolve; any other
/// id raises `ValueError` rather than emitting misleading SQL from a context-dependent
/// node. Ungated like `render_document` — the Python extension always carries serde.
#[pyfunction]
#[pyo3(signature = (document_json, node_id, dialect = "ansi", mode = "canonical"))]
fn render_fragment(
    document_json: &str,
    node_id: u32,
    dialect: &str,
    mode: &str,
) -> PyResult<String> {
    let builtin = resolve_dialect(dialect)?;
    let parsed: Parsed = serde_json::from_str(document_json).map_err(|error| {
        SerializationError::new_err(format!("failed to deserialize parse document: {error}"))
    })?;
    render_parsed_fragment(&parsed, node_id, builtin, mode)
}

fn render_parsed_fragment(
    parsed: &Parsed,
    node_id: u32,
    dialect: BuiltinDialect,
    mode: &str,
) -> PyResult<String> {
    use squonk::ast::NodeId;

    let node_id = NodeId::new(node_id)
        .ok_or_else(|| UnsupportedNodeRenderError::new_err("node id must be non-zero"))?;
    // Render for the target dialect with the requested mode, matching `render_document`.
    let config = RenderConfig {
        mode: render_mode(mode)?,
        ..RenderConfig::default()
    };
    let renderer = Renderer::with_config(dialect, config);
    parsed
        .render_fragment_by_id(node_id, renderer.config())
        .map_err(|error| UnsupportedNodeRenderError::new_err(error.to_string()))
}

#[pyfunction]
#[pyo3(signature = (sql, dialect = "ansi", mode = "canonical", recursion_limit = None))]
fn render_sql(
    sql: &str,
    dialect: &str,
    mode: &str,
    recursion_limit: Option<usize>,
) -> PyResult<String> {
    let builtin = resolve_dialect(dialect)?;
    let parsed = parse_builtin_with(sql, parse_config(builtin, recursion_limit, false, false))
        .map_err(|error| parse_error_to_pyerr(&error))?;
    render_parsed(&parsed, builtin, mode)
}

#[pyfunction]
#[pyo3(signature = (sql, source_dialect = "ansi", target_dialect = "ansi", recursion_limit = None))]
fn transpile(
    sql: &str,
    source_dialect: &str,
    target_dialect: &str,
    recursion_limit: Option<usize>,
) -> PyResult<String> {
    let source = resolve_dialect(source_dialect)?;
    let target = resolve_dialect(target_dialect)?;
    let parsed = parse_builtin_with(sql, parse_config(source, recursion_limit, false, false))
        .map_err(|error| parse_error_to_pyerr(&error))?;
    Renderer::new(target)
        .render_parsed(&parsed)
        .map_err(|error| RenderError::new_err(error.to_string()))
}

/// Pretty-print `sql` under `dialect` with the v1 style knobs (feature
/// `document-render`). `keyword_case` is `upper` / `lower` / `preserve`.
///
/// Preview, not a full-fidelity formatter: nested-expression/subquery layout and
/// comment placement carry documented limitations (see the `squonk::format`
/// module docs). Output always re-parses to the same tree and no comment is dropped.
#[pyfunction]
#[pyo3(signature = (sql, dialect = "ansi", indent_width = 2, max_line_length = 80, keyword_case = "upper"))]
fn format(
    sql: &str,
    dialect: &str,
    indent_width: usize,
    max_line_length: usize,
    keyword_case: &str,
) -> PyResult<String> {
    use squonk::format::{FormatOptions, KeywordCase, format_sql};

    let builtin = resolve_dialect(dialect)?;
    let case = KeywordCase::from_name(keyword_case).ok_or_else(|| {
        FormatError::new_err(format!(
            "unknown keyword_case {keyword_case:?}; valid values are upper, lower, preserve"
        ))
    })?;
    let options = FormatOptions {
        indent_width,
        max_line_length,
        keyword_case: case,
    };
    format_sql(sql, builtin, &options).map_err(|error| parse_error_to_pyerr(&error))
}

fn render_parsed(parsed: &Parsed, dialect: BuiltinDialect, mode: &str) -> PyResult<String> {
    let config = RenderConfig {
        mode: render_mode(mode)?,
        ..RenderConfig::default()
    };
    Renderer::with_config(dialect, config)
        .render_parsed(parsed)
        .map_err(|error| RenderError::new_err(error.to_string()))
}

fn render_mode(mode: &str) -> PyResult<RenderMode> {
    if mode.eq_ignore_ascii_case("canonical") {
        Ok(RenderMode::Canonical)
    } else if mode.eq_ignore_ascii_case("redacted") || mode.eq_ignore_ascii_case("redact") {
        Ok(RenderMode::Redacted)
    } else if mode.eq_ignore_ascii_case("parenthesized")
        || mode.eq_ignore_ascii_case("parenthesised")
    {
        Ok(RenderMode::Parenthesized)
    } else {
        Err(RenderError::new_err(format!(
            "unknown render mode {mode:?}; valid modes are canonical, redacted, parenthesized"
        )))
    }
}

fn parse_config(
    dialect: BuiltinDialect,
    recursion_limit: Option<usize>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> ParseConfig<BuiltinDialect> {
    let mut config = ParseConfig::new(dialect)
        .capture_trivia(capture_trivia)
        .parse_float_as_decimal(parse_float_as_decimal);
    if let Some(limit) = recursion_limit {
        config = config.recursion_limit(limit);
    }
    config
}

fn resolve_dialect(name: &str) -> PyResult<BuiltinDialect> {
    BuiltinDialect::from_name(name).ok_or_else(|| {
        let valid = squonk::bindings::supported_dialects()
            .iter()
            .map(|dialect| dialect.aliases.join("/"))
            .collect::<Vec<_>>()
            .join(", ");
        DialectError::new_err(format!(
            "unknown SQL dialect {name:?}; valid names are {valid}"
        ))
    })
}

fn parse_error_to_pyerr(error: &squonk::error::ParseError) -> PyErr {
    let diagnostic = ParseDiagnostic::from(error);
    SqlParseError::new_err((
        diagnostic.message,
        diagnostic.span_start,
        diagnostic.span_end,
        diagnostic.kind,
        diagnostic.expected,
        diagnostic.found,
    ))
}

fn lex_error_to_pyerr(error: squonk::tokenizer::LexError) -> PyErr {
    LexError::new_err(error.to_string())
}

fn serialization_error(error: serde_json::Error) -> PyErr {
    SerializationError::new_err(format!(
        "failed to serialize the parse result to JSON: {error}"
    ))
}

fn to_json(value: &impl serde::Serialize) -> PyResult<String> {
    serde_json::to_string(value).map_err(serialization_error)
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    // Wire-schema version of the serialized JSON surface, independent of the
    // package version (docs/schema-contract.md). Mirrors `schemaVersion()` in the
    // WASM/npm binding.
    module.add("__schema_version__", WIRE_SCHEMA_VERSION)?;
    module.add_class::<NativeDocument>()?;
    module.add_class::<NativeRecoveredDocument>()?;
    module.add_function(wrap_pyfunction!(parse, module)?)?;
    module.add_function(wrap_pyfunction!(parse_document, module)?)?;
    module.add_function(wrap_pyfunction!(parse_recovering, module)?)?;
    module.add_function(wrap_pyfunction!(parse_recovering_document, module)?)?;
    module.add_function(wrap_pyfunction!(supported_dialects, module)?)?;
    module.add_function(wrap_pyfunction!(tokenize, module)?)?;
    module.add_function(wrap_pyfunction!(render_document, module)?)?;
    module.add_function(wrap_pyfunction!(render_fragment, module)?)?;
    module.add_function(wrap_pyfunction!(render_sql, module)?)?;
    module.add_function(wrap_pyfunction!(transpile, module)?)?;
    module.add_function(wrap_pyfunction!(format, module)?)?;
    Ok(())
}
