// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Qualify every bare column with its table alias, via a `VisitMut` rewrite.
//!
//! Run with `cargo run --example rewrite_qualify`. This is the mutable-walk half of
//! the rewritable AST (ADR-0016): parse, mutate the tree in place, render the result.

use squonk::ast::generated::visit::{VisitMut, walk_expr_mut};
use squonk::ast::render::{RenderConfig, RenderCtx, RenderExt as _};
use squonk::ast::{Expr, Ident, SetExpr, Statement, TableFactor};
use squonk::parse;

/// Prepends `alias` to every unqualified column it walks.
struct Qualify {
    alias: Ident,
}

impl VisitMut for Qualify {
    fn visit_expr_mut(&mut self, node: &mut Expr) {
        // A bare column is a one-part name; qualifying it prepends the alias part.
        //
        // Graft-safety rule: the `Parsed` root's resolver is FROZEN (immutable), so a
        // rewrite may only REUSE `Symbol`s already interned in the tree — here the alias
        // symbol the FROM clause already holds. Synthesizing brand-new identifier text
        // has no symbol to point at and would need a fresh parse/interner instead.
        if let Expr::Column { name, .. } = node {
            if name.0.len() == 1 {
                name.0.insert(0, self.alias.clone());
            }
        }
        walk_expr_mut(self, node);
    }
}

/// The alias of the single table in a `SELECT ... FROM t AS a` statement.
fn table_alias(statement: &Statement) -> Ident {
    let Statement::Query { query, .. } = statement else {
        panic!("expected a query statement");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    let TableFactor::Table {
        alias: Some(alias), ..
    } = &select.from[0].relation
    else {
        panic!("expected an aliased table");
    };
    alias.name.clone()
}

fn main() {
    let parsed = parse("SELECT id, total FROM orders AS o").expect("parses");

    // The tree lives behind a shared `Parsed`; clone the statements to rewrite them
    // while the root keeps the source + resolver a render needs (`Statement: Clone`).
    let mut statements = parsed.statements().to_vec();
    let mut qualify = Qualify {
        alias: table_alias(&statements[0]),
    };
    for statement in &mut statements {
        qualify.visit_statement_mut(statement);
    }

    // Render the rewritten statement against the original root's resolver + source.
    let config = RenderConfig::default();
    let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
    let rewritten = statements[0].displayed(&ctx).to_string();

    println!("before: {parsed}");
    println!("after:  {rewritten}");
    assert_eq!(rewritten, "SELECT o.id, o.total FROM orders AS o");
}
