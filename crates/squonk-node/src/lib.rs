// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Native Node-API bindings for Squonk.
//!
//! The exported names deliberately match the low-level WebAssembly binding contract.
//! The TypeScript facade can therefore select the native Node backend without changing
//! its public API, type declarations, or lazy-document semantics.

use napi::{Error, Result, Status};
use napi_derive::napi;
use serde::Serialize;
use serde_json::Value;
use squonk::bindings::{ParseDiagnostic, ParseDocument, RecoveredDocument, TokenizeDocument};
use squonk::error::ParseError;
use squonk::render::{RenderConfig, RenderMode, Renderer};
use squonk::{
    BuiltinDialect, ParseConfig, Parsed, Recovered, parse_builtin_with,
    parse_recovering_builtin_with, tokenize_with_builtin, tokenize_with_builtin_trivia,
};

#[napi(js_name = "NativeDocument")]
pub struct NativeDocument {
    parsed: Parsed,
    dialect: BuiltinDialect,
}

#[napi]
impl NativeDocument {
    #[napi(getter)]
    pub fn source(&self) -> String {
        self.parsed.source().to_owned()
    }

    #[napi(getter)]
    pub fn dialect(&self) -> String {
        self.dialect.name().to_owned()
    }

    #[napi(js_name = "to_value")]
    pub fn to_value(&self) -> Result<Value> {
        to_value(&ParseDocument::new(&self.parsed, self.dialect))
    }

    #[napi]
    pub fn render(&self, dialect: String, mode: String) -> Result<String> {
        render_parsed(&self.parsed, &dialect, &mode)
    }

    #[napi(js_name = "render_fragment")]
    pub fn render_fragment(&self, node_id: u32, dialect: String, mode: String) -> Result<String> {
        render_fragment_parsed(&self.parsed, node_id, &dialect, &mode)
    }
}

#[napi(js_name = "NativeRecoveredDocument")]
pub struct NativeRecoveredDocument {
    recovered: Recovered,
    dialect: BuiltinDialect,
}

#[napi]
impl NativeRecoveredDocument {
    #[napi(getter)]
    pub fn source(&self) -> String {
        self.recovered.parsed().source().to_owned()
    }

    #[napi(getter)]
    pub fn dialect(&self) -> String {
        self.dialect.name().to_owned()
    }

    #[napi(js_name = "to_value")]
    pub fn to_value(&self) -> Result<Value> {
        to_value(&RecoveredDocument::new(&self.recovered, self.dialect))
    }

    #[napi]
    pub fn render(&self, dialect: String, mode: String) -> Result<String> {
        render_parsed(self.recovered.parsed(), &dialect, &mode)
    }

    #[napi(js_name = "render_fragment")]
    pub fn render_fragment(&self, node_id: u32, dialect: String, mode: String) -> Result<String> {
        render_fragment_parsed(self.recovered.parsed(), node_id, &dialect, &mode)
    }
}

#[napi(js_name = "parse_document_with")]
pub fn parse_document_with(
    sql: String,
    dialect: String,
    recursion_limit: Option<u32>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> Result<NativeDocument> {
    let dialect = resolve(&dialect)?;
    let parsed = parse_builtin_with(
        &sql,
        parse_config(
            dialect,
            recursion_limit,
            capture_trivia,
            parse_float_as_decimal,
        ),
    )
    .map_err(parse_error)?;
    Ok(NativeDocument { parsed, dialect })
}

#[napi(js_name = "parse_recovering_document_with")]
pub fn parse_recovering_document_with(
    sql: String,
    dialect: String,
    recursion_limit: Option<u32>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> Result<NativeRecoveredDocument> {
    let dialect = resolve(&dialect)?;
    let recovered = parse_recovering_builtin_with(
        &sql,
        parse_config(
            dialect,
            recursion_limit,
            capture_trivia,
            parse_float_as_decimal,
        ),
    )
    .map_err(parse_error)?;
    Ok(NativeRecoveredDocument { recovered, dialect })
}

#[napi(js_name = "parse_with")]
pub fn parse_with(
    sql: String,
    dialect: String,
    recursion_limit: Option<u32>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> Result<Value> {
    let document = parse_document_with(
        sql,
        dialect,
        recursion_limit,
        capture_trivia,
        parse_float_as_decimal,
    )?;
    document.to_value()
}

#[napi(js_name = "parse_recovering_with")]
pub fn parse_recovering_with(
    sql: String,
    dialect: String,
    recursion_limit: Option<u32>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> Result<Value> {
    let document = parse_recovering_document_with(
        sql,
        dialect,
        recursion_limit,
        capture_trivia,
        parse_float_as_decimal,
    )?;
    document.to_value()
}

#[napi(js_name = "render_sql")]
pub fn render_sql(sql: String, dialect: String, mode: String) -> Result<String> {
    let dialect = resolve(&dialect)?;
    let parsed = parse_builtin_with(&sql, ParseConfig::new(dialect)).map_err(parse_error)?;
    render_parsed(&parsed, dialect.name(), &mode)
}

#[napi(js_name = "render_document")]
pub fn render_document(document: Value, dialect: String, mode: String) -> Result<String> {
    let parsed = serde_json::from_value::<Parsed>(document).map_err(|error| {
        diagnostic_error(binding_error(
            format!("failed to deserialize binding document: {error}"),
            "deserialize",
        ))
    })?;
    render_parsed(&parsed, &dialect, &mode)
}

#[napi(js_name = "render_fragment")]
pub fn render_fragment(
    document: Value,
    node_id: u32,
    dialect: String,
    mode: String,
) -> Result<String> {
    let parsed = serde_json::from_value::<Parsed>(document).map_err(|error| {
        diagnostic_error(binding_error(
            format!("failed to deserialize binding document: {error}"),
            "deserialize",
        ))
    })?;
    render_fragment_parsed(&parsed, node_id, &dialect, &mode)
}

#[napi]
pub fn format(
    sql: String,
    dialect: String,
    indent_width: u32,
    max_line_length: u32,
    keyword_case: String,
) -> Result<String> {
    use squonk::format::{FormatOptions, KeywordCase, format_sql};

    let dialect = resolve(&dialect)?;
    let keyword_case = KeywordCase::from_name(&keyword_case).ok_or_else(|| {
        diagnostic_error(binding_error(
            format!(
                "unknown keywordCase {keyword_case:?}; valid values are upper, lower, preserve"
            ),
            "unknown_keyword_case",
        ))
    })?;
    format_sql(
        &sql,
        dialect,
        &FormatOptions {
            indent_width: indent_width as usize,
            max_line_length: max_line_length as usize,
            keyword_case,
        },
    )
    .map_err(parse_error)
}

#[napi(js_name = "supported_dialects")]
pub fn supported_dialects() -> Result<Value> {
    to_value(&squonk::bindings::supported_dialects())
}

#[napi]
pub fn tokenize(sql: String, dialect: String, include_trivia: bool) -> Result<Value> {
    let dialect = resolve(&dialect)?;
    let document = if include_trivia {
        let (tokens, trivia) = tokenize_with_builtin_trivia(&sql, dialect)
            .map_err(|error| diagnostic_error(binding_error(error.to_string(), "lex")))?;
        TokenizeDocument::with_trivia(&sql, dialect, &tokens, trivia.all())
    } else {
        let tokens = tokenize_with_builtin(&sql, dialect)
            .map_err(|error| diagnostic_error(binding_error(error.to_string(), "lex")))?;
        TokenizeDocument::new(&sql, dialect, &tokens)
    };
    to_value(&document)
}

#[napi]
pub fn transpile(sql: String, source_dialect: String, target_dialect: String) -> Result<String> {
    let source = resolve(&source_dialect)?;
    let target = resolve(&target_dialect)?;
    let parsed = parse_builtin_with(&sql, ParseConfig::new(source)).map_err(parse_error)?;
    render_parsed(&parsed, target.name(), "canonical")
}

#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

#[napi(js_name = "schema_version")]
pub fn schema_version() -> u32 {
    squonk::bindings::WIRE_SCHEMA_VERSION
}

fn parse_config(
    dialect: BuiltinDialect,
    recursion_limit: Option<u32>,
    capture_trivia: bool,
    parse_float_as_decimal: bool,
) -> ParseConfig<BuiltinDialect> {
    let mut config = ParseConfig::new(dialect)
        .capture_trivia(capture_trivia)
        .parse_float_as_decimal(parse_float_as_decimal);
    if let Some(limit) = recursion_limit {
        config = config.recursion_limit(limit as usize);
    }
    config
}

fn resolve(dialect: &str) -> Result<BuiltinDialect> {
    BuiltinDialect::from_name(dialect).ok_or_else(|| {
        diagnostic_error(binding_error(
            format!("unknown or unsupported dialect: {dialect:?}"),
            "unknown_dialect",
        ))
    })
}

fn render_parsed(parsed: &Parsed, dialect: &str, mode: &str) -> Result<String> {
    let dialect = resolve(dialect)?;
    Renderer::with_config(
        dialect,
        RenderConfig {
            mode: render_mode(mode)?,
            ..RenderConfig::default()
        },
    )
    .render_parsed(parsed)
    .map_err(|error| diagnostic_error(binding_error(error.to_string(), "render")))
}

fn render_fragment_parsed(
    parsed: &Parsed,
    node_id: u32,
    dialect: &str,
    mode: &str,
) -> Result<String> {
    use squonk::ast::NodeId;

    let dialect = resolve(dialect)?;
    let node_id = NodeId::new(node_id).ok_or_else(|| {
        diagnostic_error(binding_error(
            "node id must be a non-zero parser id",
            "render",
        ))
    })?;
    let config = RenderConfig {
        mode: render_mode(mode)?,
        ..RenderConfig::default()
    };
    let renderer = Renderer::with_config(dialect, config);
    parsed
        .render_fragment_by_id(node_id, renderer.config())
        .map_err(|error| diagnostic_error(binding_error(error.to_string(), "render")))
}

fn render_mode(mode: &str) -> Result<RenderMode> {
    if mode.eq_ignore_ascii_case("canonical") {
        Ok(RenderMode::Canonical)
    } else if mode.eq_ignore_ascii_case("redacted") || mode.eq_ignore_ascii_case("redact") {
        Ok(RenderMode::Redacted)
    } else if mode.eq_ignore_ascii_case("parenthesized")
        || mode.eq_ignore_ascii_case("parenthesised")
    {
        Ok(RenderMode::Parenthesized)
    } else {
        Err(diagnostic_error(binding_error(
            format!(
                "unknown render mode {mode:?}; valid modes are canonical, redacted, parenthesized"
            ),
            "unknown_render_mode",
        )))
    }
}

fn to_value(value: &(impl Serialize + ?Sized)) -> Result<Value> {
    serde_json::to_value(value).map_err(|error| {
        diagnostic_error(binding_error(
            format!("failed to serialize binding response: {error}"),
            "serialization",
        ))
    })
}

fn parse_error(error: ParseError) -> Error {
    diagnostic_error(ParseDiagnostic::from(&error))
}

fn binding_error(message: impl Into<String>, kind: &'static str) -> ParseDiagnostic {
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

fn diagnostic_error(diagnostic: ParseDiagnostic) -> Error {
    let message = serde_json::to_string(&diagnostic).unwrap_or(diagnostic.message);
    Error::new(Status::GenericFailure, message)
}
