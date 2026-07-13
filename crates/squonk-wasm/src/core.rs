// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Target-agnostic binding API over `squonk` for the wasm package.

use std::sync::Arc;

#[cfg(test)]
use serde::Serialize;

use squonk::bindings::{
    DialectInfo, ParseDiagnostic, ParseDocument, RecoveredDocument, TokenizeDocument,
};
use squonk::render::{RenderConfig, RenderMode, Renderer};
use squonk::{
    BuiltinDialect, ParseConfig, Parsed, Recovered, parse_builtin_with,
    parse_recovering_builtin_with, tokenize_with_builtin, tokenize_with_builtin_trivia,
};

/// Binding-layer result type. Errors use the same serializable diagnostic shape as
/// parser errors, including non-parse setup/render/serialization failures.
pub type BindingResult<T> = Result<T, ParseDiagnostic>;

/// Parse `sql` under `dialect`, emitting a binding parse document.
pub fn parse<T>(
    sql: &str,
    dialect: &str,
    emit: impl FnOnce(&ParseDocument<'_, Arc<str>>) -> BindingResult<T>,
) -> BindingResult<T> {
    parse_with(sql, dialect, None, false, false, emit)
}

/// Parse `sql` under `dialect` with an explicit recursion-depth limit.
pub fn parse_with_limit<T>(
    sql: &str,
    dialect: &str,
    limit: usize,
    emit: impl FnOnce(&ParseDocument<'_, Arc<str>>) -> BindingResult<T>,
) -> BindingResult<T> {
    parse_with(sql, dialect, Some(limit), false, false, emit)
}

/// Parse `sql` under `dialect`, honoring all binding parse options.
// One flat parameter per exposed `ParseConfig` field plus the `emit` sink: the binding
// facade mirrors the JS/wasm-bindgen call shape, so the option list grows past the
// clippy default as the parser gains knobs rather than hiding behind an options struct.
#[allow(clippy::too_many_arguments)]
pub fn parse_with<T>(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<usize>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
    emit: impl FnOnce(&ParseDocument<'_, Arc<str>>) -> BindingResult<T>,
) -> BindingResult<T> {
    let (parsed, dialect) = parse_owned_with(
        sql,
        dialect,
        recursion_limit,
        capture_trivia,
        parse_float_as_decimal,
    )?;
    emit(&ParseDocument::new(&parsed, dialect))
}

pub(crate) fn parse_owned_with(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<usize>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> BindingResult<(Parsed, BuiltinDialect)> {
    let dialect = resolve(dialect)?;
    let parsed = parse_builtin_with(
        sql,
        parse_config(
            dialect,
            recursion_limit,
            capture_trivia,
            parse_float_as_decimal,
        ),
    )
    .map_err(|error| ParseDiagnostic::from(&error))?;
    Ok((parsed, dialect))
}

/// Parse `sql` recovering past statement errors.
pub fn parse_recovering<T>(
    sql: &str,
    dialect: &str,
    emit: impl FnOnce(&RecoveredDocument<'_, Arc<str>>) -> BindingResult<T>,
) -> BindingResult<T> {
    parse_recovering_with(sql, dialect, None, false, false, emit)
}

/// Recovering parse honoring all binding parse options.
// See `parse_with`: one flat parameter per exposed `ParseConfig` field plus the
// `emit` sink mirrors the wasm-bindgen call shape.
#[allow(clippy::too_many_arguments)]
pub fn parse_recovering_with<T>(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<usize>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
    emit: impl FnOnce(&RecoveredDocument<'_, Arc<str>>) -> BindingResult<T>,
) -> BindingResult<T> {
    let (recovered, dialect) = parse_recovering_owned_with(
        sql,
        dialect,
        recursion_limit,
        capture_trivia,
        parse_float_as_decimal,
    )?;
    emit(&RecoveredDocument::new(&recovered, dialect))
}

pub(crate) fn parse_recovering_owned_with(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<usize>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> BindingResult<(Recovered, BuiltinDialect)> {
    let dialect = resolve(dialect)?;
    let recovered = parse_recovering_builtin_with(
        sql,
        parse_config(
            dialect,
            recursion_limit,
            capture_trivia,
            parse_float_as_decimal,
        ),
    )
    .map_err(|error| ParseDiagnostic::from(&error))?;
    Ok((recovered, dialect))
}

/// Built-in dialect metadata compiled into this wasm artifact.
pub fn supported_dialects<T>(
    emit: impl FnOnce(&[DialectInfo]) -> BindingResult<T>,
) -> BindingResult<T> {
    let dialects = squonk::bindings::supported_dialects();
    emit(&dialects)
}

/// Tokenize `sql` under `dialect`.
pub fn tokenize<T>(
    sql: &str,
    dialect: &str,
    include_trivia: bool,
    emit: impl FnOnce(&TokenizeDocument) -> BindingResult<T>,
) -> BindingResult<T> {
    let dialect = resolve(dialect)?;
    let document = if include_trivia {
        let (tokens, trivia) = tokenize_with_builtin_trivia(sql, dialect)
            .map_err(|error| binding_error(error.to_string(), "lex"))?;
        TokenizeDocument::with_trivia(sql, dialect, &tokens, trivia.all())
    } else {
        let tokens = tokenize_with_builtin(sql, dialect)
            .map_err(|error| binding_error(error.to_string(), "lex"))?;
        TokenizeDocument::new(sql, dialect, &tokens)
    };
    emit(&document)
}

/// Render SQL under `dialect` using Rust's renderer.
pub fn render_sql(sql: &str, dialect: &str, mode: &str) -> BindingResult<String> {
    let dialect = resolve(dialect)?;
    let parsed = parse_builtin_with(sql, ParseConfig::new(dialect))
        .map_err(|error| ParseDiagnostic::from(&error))?;
    render_parsed(&parsed, dialect, mode)
}

/// Render a deserialized parse document under `dialect`.
pub fn render_document(parsed: &Parsed, dialect: &str, mode: &str) -> BindingResult<String> {
    let dialect = resolve(dialect)?;
    render_parsed(parsed, dialect, mode)
}

/// Render a single sub-node of a deserialized parse document back to canonical SQL,
/// selected by its parser-assigned node id.
///
/// The fragment counterpart of [`render_document`]: linters, LSPs, and explainer UIs
/// hold a node handle (document + id) and want "the SQL for just this sub-tree".
/// Only standalone-renderable node kinds — a complete expression, query, statement,
/// or data type — resolve; any other id yields a `render` diagnostic rather than a
/// misleading fragment (the allowlist is enforced in the core
/// [`Parsed::render_fragment_by_id`](squonk::Parsed::render_fragment_by_id)).
/// Reconstructs a [`Parsed`] from the JS document.
pub fn render_fragment(
    parsed: &Parsed,
    node_id: u32,
    dialect: &str,
    mode: &str,
) -> BindingResult<String> {
    use squonk::ast::NodeId;

    let dialect = resolve(dialect)?;
    let node_id = NodeId::new(node_id)
        .ok_or_else(|| binding_error("node id must be a non-zero parser id", "render"))?;
    // Render for the target dialect with the requested mode, exactly as
    // `render_document` builds its config, so a fragment uses the same spellings the
    // whole document would.
    let config = RenderConfig {
        mode: render_mode(mode)?,
        ..RenderConfig::default()
    };
    let renderer = Renderer::with_config(dialect, config);
    parsed
        .render_fragment_by_id(node_id, renderer.config())
        .map_err(|error| binding_error(error.to_string(), "render"))
}

/// Pretty-print `sql` under `dialect` with the v1 style knobs.
/// `keyword_case` is `upper` / `lower` / `preserve`.
///
/// Preview, not a full-fidelity formatter: nested-expression/subquery layout and
/// comment placement carry documented limitations (see the `squonk::format`
/// module docs). Output always re-parses to the same tree and no comment is dropped.
pub fn format(
    sql: &str,
    dialect: &str,
    indent_width: usize,
    max_line_length: usize,
    keyword_case: &str,
) -> BindingResult<String> {
    use squonk::format::{FormatOptions, KeywordCase, format_sql};

    let dialect = resolve(dialect)?;
    let case = KeywordCase::from_name(keyword_case).ok_or_else(|| {
        binding_error(
            format!(
                "unknown keywordCase {keyword_case:?}; valid values are upper, lower, preserve"
            ),
            "unknown_keyword_case",
        )
    })?;
    let options = FormatOptions {
        indent_width,
        max_line_length,
        keyword_case: case,
    };
    format_sql(sql, dialect, &options).map_err(|error| ParseDiagnostic::from(&error))
}

/// Parse under `source_dialect` and render for `target_dialect`.
pub fn transpile_sql(
    sql: &str,
    source_dialect: &str,
    target_dialect: &str,
) -> BindingResult<String> {
    let source = resolve(source_dialect)?;
    let target = resolve(target_dialect)?;
    let parsed = parse_builtin_with(sql, ParseConfig::new(source))
        .map_err(|error| ParseDiagnostic::from(&error))?;
    render_parsed(&parsed, target, "canonical")
}

/// The library version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The wire-schema version of the serialized JSON surface, independent of the
/// package [`version`] (`docs/schema-contract.md`). Re-exports the single source
/// of truth in `squonk::bindings` so the JS `schemaVersion()` and Python
/// `__schema_version__` cannot drift from the Rust contract.
pub fn schema_version() -> u32 {
    squonk::bindings::WIRE_SCHEMA_VERSION
}

/// Construct a serializable non-parser diagnostic.
pub fn binding_error(message: impl Into<String>, kind: &'static str) -> ParseDiagnostic {
    ParseDiagnostic {
        message: message.into(),
        kind,
        span: None,
        span_start: None,
        span_end: None,
        expected: None,
        found: None,
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

fn resolve(dialect: &str) -> BindingResult<BuiltinDialect> {
    BuiltinDialect::from_name(dialect).ok_or_else(|| {
        binding_error(
            format!("unknown or unsupported dialect: {dialect:?}"),
            "unknown_dialect",
        )
    })
}

fn render_parsed(parsed: &Parsed, dialect: BuiltinDialect, mode: &str) -> BindingResult<String> {
    let config = RenderConfig {
        mode: render_mode(mode)?,
        ..RenderConfig::default()
    };
    Renderer::with_config(dialect, config)
        .render_parsed(parsed)
        .map_err(|error| binding_error(error.to_string(), "render"))
}

fn render_mode(mode: &str) -> BindingResult<RenderMode> {
    if mode.eq_ignore_ascii_case("canonical") {
        Ok(RenderMode::Canonical)
    } else if mode.eq_ignore_ascii_case("redacted") || mode.eq_ignore_ascii_case("redact") {
        Ok(RenderMode::Redacted)
    } else if mode.eq_ignore_ascii_case("parenthesized")
        || mode.eq_ignore_ascii_case("parenthesised")
    {
        Ok(RenderMode::Parenthesized)
    } else {
        Err(binding_error(
            format!(
                "unknown render mode {mode:?}; valid modes are canonical, redacted, parenthesized"
            ),
            "unknown_render_mode",
        ))
    }
}

#[cfg(test)]
fn to_json(value: &(impl Serialize + ?Sized)) -> BindingResult<String> {
    serde_json::to_string(value).map_err(|error| {
        binding_error(
            format!("failed to serialize binding response: {error}"),
            "serialization",
        )
    })
}

#[cfg(test)]
fn error_json(error: ParseDiagnostic) -> String {
    serde_json::to_string(&error).expect("parse diagnostics serialize")
}

#[cfg(test)]
pub fn parse_json(sql: &str, dialect: &str) -> Result<String, String> {
    parse(sql, dialect, |document| to_json(document)).map_err(error_json)
}

#[cfg(test)]
pub fn parse_with_json(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<usize>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> Result<String, String> {
    parse_with(
        sql,
        dialect,
        recursion_limit,
        capture_trivia,
        parse_float_as_decimal,
        |document| to_json(document),
    )
    .map_err(error_json)
}

#[cfg(test)]
pub fn parse_recovering_json(sql: &str, dialect: &str) -> Result<String, String> {
    parse_recovering(sql, dialect, |document| to_json(document)).map_err(error_json)
}

#[cfg(test)]
pub fn parse_recovering_with_json(
    sql: &str,
    dialect: &str,
    recursion_limit: Option<usize>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> Result<String, String> {
    parse_recovering_with(
        sql,
        dialect,
        recursion_limit,
        capture_trivia,
        parse_float_as_decimal,
        |document| to_json(document),
    )
    .map_err(error_json)
}

#[cfg(test)]
pub fn supported_dialects_json() -> String {
    supported_dialects(to_json)
        .map_err(error_json)
        .expect("supported dialect metadata serializes")
}

#[cfg(test)]
pub fn tokenize_json(sql: &str, dialect: &str, include_trivia: bool) -> Result<String, String> {
    tokenize(sql, dialect, include_trivia, to_json).map_err(error_json)
}

#[cfg(test)]
pub fn render_document_json(
    document_json: &str,
    dialect: &str,
    mode: &str,
) -> Result<String, String> {
    let parsed: Parsed = serde_json::from_str(document_json).map_err(|error| {
        error_json(binding_error(
            format!("failed to deserialize parse document: {error}"),
            "deserialize",
        ))
    })?;
    render_document_for_tests(&parsed, dialect, mode).map_err(error_json)
}

#[cfg(test)]
fn render_document_for_tests(parsed: &Parsed, dialect: &str, mode: &str) -> BindingResult<String> {
    let dialect = resolve(dialect)?;
    render_parsed(parsed, dialect, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(text: &str) -> serde_json::Value {
        serde_json::from_str(text).expect("bindings emit JSON")
    }

    #[test]
    fn parse_json_carries_resolver_metadata() {
        let json = parse_json("SELECT salary FROM employees", "ansi").expect("parses");
        let value = value(&json);
        assert_eq!(value["dialect"], "ansi");
        assert!(value["resolver"]["dynamic_base"].as_u64().is_some());
        assert!(value["resolver"]["keyword_symbols"].as_array().is_some());
    }

    #[test]
    fn all_supported_dialects_parse_a_smoke_query() {
        let dialects = supported_dialects_json();
        for dialect in value(&dialects).as_array().expect("dialect list") {
            let name = dialect["name"].as_str().expect("dialect name");
            parse_json("SELECT 1", name)
                .unwrap_or_else(|err| panic!("{name} should parse SELECT 1: {err}"));
        }
    }

    #[test]
    fn recovering_returns_structured_errors() {
        let json = parse_recovering_json("SELECT 1; FROM x; SELECT 2", "ansi").expect("recovers");
        let value = value(&json);
        assert_eq!(value["errors"][0]["kind"], "syntax");
        assert!(value["errors"][0]["span"].is_object());
    }

    #[test]
    fn recovering_rejects_boundary_errors() {
        let error =
            parse_recovering_json("SELECT 1", "klingon").expect_err("unknown dialect rejects");
        let value = value(&error);
        assert_eq!(value["kind"], "unknown_dialect");
        assert!(value["span"].is_null());
    }

    #[test]
    fn parse_options_can_expose_trivia() {
        let json = parse_with_json("/* lead */ SELECT 1", "ansi", None, true, false)
            .expect("parses with trivia");
        let value = value(&json);
        assert_eq!(value["trivia"][0]["kind"], "BlockComment");
        assert_eq!(value["trivia"][0]["text"], "/* lead */");
    }

    #[test]
    fn tokenize_json_emits_discriminated_tokens_and_trivia() {
        let json =
            tokenize_json("-- lead\nSELECT a + 1", "ansi", true).expect("tokenizes with trivia");
        let value = value(&json);
        assert_eq!(value["trivia"][0]["kind"], "LineComment");
        assert_eq!(value["tokens"][0]["kind"], "Keyword");
        assert_eq!(value["tokens"][0]["keyword"], "select");
        assert!(
            value["tokens"]
                .as_array()
                .unwrap()
                .iter()
                .any(|token| { token["kind"] == "Operator" && token["operator"] == "Plus" })
        );
    }

    #[test]
    fn render_document_and_sql_use_the_renderer() {
        let document = parse_json("select 1", "ansi").expect("parses");
        assert_eq!(
            render_document_json(&document, "ansi", "canonical").expect("renders document"),
            "SELECT 1"
        );
        assert_eq!(
            render_sql("select 1", "ansi", "parenthesized").expect("renders sql"),
            "SELECT 1"
        );
        assert_ne!(
            render_sql("select 123", "ansi", "redacted").expect("redacts sql"),
            "SELECT 123"
        );
    }

    #[test]
    fn format_pretty_prints_and_honours_keyword_case() {
        let out = format("select a,b from t where a=1", "ansi", 2, 80, "upper").expect("formats");
        assert_eq!(out, "SELECT a, b\nFROM t\nWHERE a = 1");
        let lower = format("SELECT a FROM t", "ansi", 2, 80, "lower").expect("formats");
        assert_eq!(lower, "select a\nfrom t");
        let bad = format("SELECT 1", "ansi", 2, 80, "sideways").expect_err("bad case");
        assert_eq!(
            serde_json::to_value(bad).expect("serializes")["kind"],
            "unknown_keyword_case"
        );
    }

    #[test]
    fn render_fragment_renders_a_standalone_subnode_by_id() {
        use squonk::ast::render::FragmentRender;
        use squonk::ast::{SelectItem, SetExpr, Statement};

        // Round-trip a parse document through JSON (the facade boundary), then render
        // just one sub-node by its id.
        let json = parse_json("SELECT a + 1 FROM t", "ansi").expect("parses");
        let parsed: Parsed = serde_json::from_str(&json).expect("deserializes");

        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let SelectItem::Expr { expr, .. } = &select.projection[0] else {
            panic!("expected an expression projection");
        };
        let id = expr.fragment_node_id().as_u32();

        assert_eq!(
            render_fragment(&parsed, id, "ansi", "canonical").expect("renders the sub-node"),
            "a + 1",
        );

        // An id that names no standalone-renderable node is a `render` diagnostic,
        // never a misleading fragment.
        let error =
            render_fragment(&parsed, u32::MAX, "ansi", "canonical").expect_err("no such node");
        assert_eq!(
            serde_json::to_value(error).expect("serializes")["kind"],
            "render",
        );
    }

    #[test]
    fn transpile_uses_source_and_target_dialects() {
        assert_eq!(
            transpile_sql("select 1", "ansi", "ansi").expect("transpiles"),
            "SELECT 1"
        );
        let error = transpile_sql("select 1", "klingon", "ansi").expect_err("bad source dialect");
        assert_eq!(
            serde_json::to_value(error).expect("serializes")["kind"],
            "unknown_dialect"
        );
    }
}
