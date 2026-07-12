// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Collect every table a query references, with a read-only `Visit`.
//!
//! Run with `cargo run --example analyze_tables`. This is the immutable-walk half of
//! the traversal API, paired with `Resolver::resolve` (interned `Symbol` -> text).

use squonk::ast::generated::visit::{Visit, walk_table_factor};
use squonk::ast::{Resolver, TableFactor};
use squonk::parse;

/// Gathers the dotted name of every `TableFactor::Table` it walks.
struct TableCollector<'a> {
    resolver: &'a dyn Resolver,
    tables: Vec<String>,
}

impl<'a, 'ast> Visit<'ast> for TableCollector<'a> {
    fn visit_table_factor(&mut self, node: &'ast TableFactor) {
        if let TableFactor::Table { name, .. } = node {
            // A `Symbol` is a bare interner key; the tree's resolver gives it text back.
            let dotted: Vec<&str> = name
                .0
                .iter()
                .map(|part| self.resolver.resolve(part.sym))
                .collect();
            self.tables.push(dotted.join("."));
        }
        // Recurse so tables nested in joins and subqueries are collected too.
        walk_table_factor(self, node);
    }
}

fn main() {
    let parsed = parse("SELECT * FROM orders o JOIN customers c ON o.cust = c.id").expect("parses");

    // A read-only walk needs no clone: it borrows the statements the root owns and
    // resolves their symbols through the root's own resolver.
    let mut collector = TableCollector {
        resolver: parsed.resolver(),
        tables: Vec::new(),
    };
    for statement in parsed.statements() {
        collector.visit_statement(statement);
    }

    println!("tables: {:?}", collector.tables);
    assert_eq!(collector.tables, ["orders", "customers"]);
}
