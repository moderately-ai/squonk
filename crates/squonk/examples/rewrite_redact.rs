// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Strip aliases with a `VisitMut`, then render in redacted mode — showing a render
//! MODE composing on top of a structural rewrite.
//!
//! Run with `cargo run --example rewrite_redact`.

use squonk::ast::generated::visit::{VisitMut, walk_select_item_mut, walk_table_factor_mut};
use squonk::ast::render::{RenderConfig, RenderCtx, RenderExt as _, RenderMode};
use squonk::ast::{SelectItem, TableFactor};
use squonk::parse;

/// Drops every `AS alias` — on select items and on tables — that it walks.
struct StripAliases;

impl VisitMut for StripAliases {
    fn visit_select_item_mut(&mut self, node: &mut SelectItem) {
        if let SelectItem::Expr { alias, .. } = node {
            *alias = None;
        }
        walk_select_item_mut(self, node);
    }

    fn visit_table_factor_mut(&mut self, node: &mut TableFactor) {
        if let TableFactor::Table { alias, .. } = node {
            *alias = None;
        }
        walk_table_factor_mut(self, node);
    }
}

fn main() {
    let parsed = parse("SELECT secret AS s, 42 AS answer FROM users AS u").expect("parses");

    // Clone-and-rewrite while the root keeps source + resolver for rendering.
    let mut statements = parsed.statements().to_vec();
    let mut strip = StripAliases;
    for statement in &mut statements {
        strip.visit_statement_mut(statement);
    }

    // The structural rewrite (alias stripping) and the render MODE (redaction)
    // compose independently: `RenderMode::Redacted` masks identifier/literal CONTENT
    // to a stable, PII-free fingerprint, on top of whatever the `VisitMut` changed.
    let config = RenderConfig {
        mode: RenderMode::Redacted,
        ..RenderConfig::default()
    };
    let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
    let redacted = statements[0].displayed(&ctx).to_string();

    println!("before:            {parsed}");
    println!("stripped+redacted: {redacted}");
    assert_eq!(redacted, "SELECT id, ? FROM id");
}
