// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Regenerable golden corpus for M1 conformance.
//!
//! The on-disk corpus uses the CockroachDB/datadriven shape from ADR-0015. Running
//! the tests with `REWRITE=1` rewrites the expected blocks from the current
//! engines and renderer. PostgreSQL cases record `pg_query` accept/reject plus
//! the mapped protobuf shape where available; ANSI cases are the thin
//! spec-reproducible BNF layer, because ANSI has no in-process engine oracle.

use std::fmt::{Debug, Write as _};
use std::path::{Component, Path, PathBuf};

use squonk::dialect::{Ansi, Postgres};
use squonk::render::{RenderDialect, Renderer};
use squonk::{Parsed, parse_with};
use squonk_ast::render::RenderMode;

use crate::{pg, shape};

/// Generate the PostgreSQL golden output for one SQL string.
pub fn postgres_golden(sql: &str) -> String {
    let sql = sql_input(sql);
    let pg_result = pg_query::parse(sql);
    let ours_result = parse_with(sql, squonk::ParseConfig::new(Postgres));

    let mut out = String::new();
    writeln!(out, "pg_query: {}", verdict(pg_result.is_ok())).expect("write to string");
    writeln!(out, "squonk: {}", verdict(ours_result.is_ok())).expect("write to string");

    match &pg_result {
        Ok(parsed) => match pg::pg_shape(&parsed.protobuf) {
            Ok(shape) => write_debug_block(&mut out, "pg_shape", &shape),
            Err(err) => writeln!(out, "pg_shape_error: {err}").expect("write to string"),
        },
        Err(err) => writeln!(out, "pg_error: {err}").expect("write to string"),
    }

    match &ours_result {
        Ok(parsed) => {
            write_debug_block(&mut out, "squonk_shape", &shape::squonk_shape(parsed));
            write_render_modes(&mut out, parsed);
        }
        Err(err) => writeln!(out, "squonk_error: {err}").expect("write to string"),
    }

    out
}

/// Generate the ANSI golden output for one SQL string.
pub fn ansi_golden(sql: &str) -> String {
    let sql = sql_input(sql);
    let parsed = parse_with(sql, squonk::ParseConfig::new(Ansi));

    let mut out = String::new();
    writeln!(out, "squonk_ansi: {}", verdict(parsed.is_ok())).expect("write to string");

    match &parsed {
        Ok(parsed) => {
            write_debug_block(&mut out, "squonk_shape", &shape::squonk_shape(parsed));
            write_render_modes(&mut out, parsed);
        }
        Err(err) => writeln!(out, "squonk_error: {err}").expect("write to string"),
    }

    out
}

/// Generate the identifier-redaction golden for one SQL string.
///
/// Parses under the M1 superset (PostgreSQL) and renders the statements under both
/// Canonical and Redacted modes. The two blocks side by side prove the ADR-0010
/// guarantee directly: every identifier and literal spelled out in the canonical
/// block is replaced by a placeholder (`id` for names, `?` for values) in the
/// redacted block, while keywords, operators, and qualified-name arity are kept.
pub fn redacted_golden(sql: &str) -> String {
    let sql = sql_input(sql);
    let parsed = parse_with(sql, squonk::ParseConfig::new(Postgres));

    let mut out = String::new();
    writeln!(out, "parse: {}", verdict(parsed.is_ok())).expect("write to string");
    match &parsed {
        Ok(parsed) => {
            write_block(
                &mut out,
                "canonical",
                &crate::render_statements(parsed, RenderMode::Canonical),
            );
            write_block(
                &mut out,
                "redacted",
                &crate::render_statements(parsed, RenderMode::Redacted),
            );
        }
        Err(err) => writeln!(out, "squonk_error: {err}").expect("write to string"),
    }

    out
}

/// Generate dialect-target (Tier-2) render output for one SQL string.
///
/// Parses under PostgreSQL — the M1 superset — then spells the one AST for the
/// ANSI and PostgreSQL targets through the fallible Tier-2 [`Renderer`]. This is
/// the ADR-0010 transpilation path: shared constructs render for both targets
/// (each in its preferred spelling), while a construct a target cannot express is
/// rejected with an unsupported-construct diagnostic instead of being mis-rendered.
pub fn target_render_golden(sql: &str) -> String {
    let sql = sql_input(sql);
    let parsed = parse_with(sql, squonk::ParseConfig::new(Postgres));

    let mut out = String::new();
    writeln!(out, "parse (postgres): {}", verdict(parsed.is_ok())).expect("write to string");
    match &parsed {
        Ok(parsed) => {
            write_target_render(&mut out, "ansi", &Renderer::new(Ansi), parsed);
            write_target_render(&mut out, "postgres", &Renderer::new(Postgres), parsed);
        }
        Err(err) => writeln!(out, "squonk_error: {err}").expect("write to string"),
    }

    out
}

/// Render `parsed` through one Tier-2 target, recording accepted SQL under
/// `<label>_render` or the unsupported-construct rejection under `<label>_render_error`.
fn write_target_render<D: RenderDialect>(
    out: &mut String,
    label: &str,
    renderer: &Renderer<D>,
    parsed: &Parsed,
) {
    match renderer.render_parsed(parsed) {
        Ok(rendered) => write_block(out, &format!("{label}_render"), &rendered),
        Err(err) => writeln!(out, "{label}_render_error: {err}").expect("write to string"),
    }
}

/// Generate PostgreSQL golden output for a checked-in corpus file.
pub fn postgres_corpus_golden(name: &str) -> Result<String, String> {
    let path = postgres_corpus_path(name)?;
    let sql = std::fs::read_to_string(&path)
        .map_err(|err| format!("read PostgreSQL corpus file {}: {err}", path.display()))?;
    Ok(postgres_golden(&sql))
}

fn postgres_corpus_path(name: &str) -> Result<PathBuf, String> {
    let path = Path::new(name.trim());
    if path.as_os_str().is_empty() {
        return Err("PostgreSQL corpus file name is required".to_string());
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!(
            "PostgreSQL corpus file name must be relative and flat, got {name:?}"
        ));
    }
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("corpus/postgres")
        .join(path))
}

fn sql_input(input: &str) -> &str {
    input.trim()
}

fn verdict(accepted: bool) -> &'static str {
    if accepted { "accept" } else { "reject" }
}

fn write_render_modes(out: &mut String, parsed: &Parsed) {
    for (label, mode) in [
        ("canonical", RenderMode::Canonical),
        ("parenthesized", RenderMode::Parenthesized),
        ("redacted", RenderMode::Redacted),
    ] {
        write_block(out, label, &crate::render_statements(parsed, mode));
    }
}

fn write_debug_block<T: Debug>(out: &mut String, label: &str, value: &T) {
    write_block(out, label, &format!("{value:#?}"));
}

fn write_block(out: &mut String, label: &str, value: &str) {
    writeln!(out, "{label}:").expect("write to string");
    if value.is_empty() {
        writeln!(out, "  <empty>").expect("write to string");
        return;
    }
    for line in value.lines() {
        writeln!(out, "  {line}").expect("write to string");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use squonk_ast::Statement;

    const GOLDEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/goldens");

    #[test]
    fn datadriven_golden_corpus() {
        datadriven::walk(GOLDEN_DIR, |file| {
            file.run(|case| -> Result<String, String> {
                case.expect_empty().map_err(|err| err.to_string())?;
                match case.directive.as_str() {
                    "postgres" => Ok(postgres_golden(&case.input)),
                    "postgres-corpus" => postgres_corpus_golden(&case.input),
                    "ansi-bnf" => Ok(ansi_golden(&case.input)),
                    "redacted" => Ok(redacted_golden(&case.input)),
                    "target-render" => Ok(target_render_golden(&case.input)),
                    // Tokenizer differential oracle (ADR-0005); the dialect rides in
                    // the directive and the module owns the format + invariants.
                    directive @ ("tokens-ansi" | "tokens-postgres" | "tokens-mysql"
                    | "tokens-mssql") => crate::token_stream::render_golden(directive, &case.input),
                    other => Err(format!("unknown golden directive {other:?}")),
                }
            });
        });
    }

    #[test]
    fn debug_ast_snapshots() {
        let parsed = parse_with(
            "SELECT a + b * c FROM t WHERE a = b ORDER BY a",
            squonk::ParseConfig::new(Postgres),
        )
        .expect("debug snapshot query parses");
        insta::assert_debug_snapshot!("m1_select_debug_ast", parsed.statements());

        let parsed = parse_with(
            "SELECT 1 UNION ALL SELECT 2",
            squonk::ParseConfig::new(Postgres),
        )
        .expect("debug snapshot set operation parses");
        let [Statement::Query { query, .. }] = parsed.statements() else {
            panic!("expected one query statement");
        };
        insta::assert_debug_snapshot!("m1_set_operation_debug_ast", &query.body);
    }
}
