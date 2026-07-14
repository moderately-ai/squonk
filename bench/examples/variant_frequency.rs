// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Box-vs-inline evidence: AST enum `size_of` × real-corpus variant frequency.
//!
//! ADR-0007 says to box the *cold/fat* enum variants and keep the *hot small* ones
//! inline, and the compile-time size budgets (`generated/size_asserts.rs`) pin the
//! resulting widths. But a `size_of` number alone cannot say whether a fat variant
//! is worth boxing: an enum is sized to its *largest* variant, so a single fat
//! variant taxes every node of that enum — yet only if that enum is actually
//! instantiated often. The missing axis is **frequency**: how often each enum, and
//! each fat variant, occurs on real parses.
//!
//! This testbed supplies that axis. It parses the same vendored conformance corpora
//! the corpus benches measure (`benches/corpus/mod.rs`), walks every parsed tree
//! with the generated [`Visit`] traversal, and counts node and variant occurrences.
//! Cross-tabulated against `size_of`, the counts turn ADR-0007's box/inline split
//! from a guess into a measured policy (the criteria it backs are written up in
//! `crates/squonk-sourcegen/src/size_asserts.rs`).
//!
//! It is an *example*, not a bench or a test: the numbers are inputs to a one-off
//! layout decision, not a value to gate on, so it stays out of the default
//! `cargo nextest` / `cargo bench` runs and is invoked on demand:
//!
//! ```text
//! cargo run -p squonk-bench --example variant_frequency
//! ```
//!
//! Output is deterministic (fixed corpora, fixed dialect presets, `BTreeMap`
//! ordering), so a re-run reproduces the table verbatim.

#[path = "../benches/corpus/mod.rs"]
mod corpus;

use corpus::{PRESETS, Preset, parse_owned, subset};
use squonk_ast::ast::*;
use squonk_ast::generated::visit::{self, Visit};
use std::collections::BTreeMap;
use std::mem::size_of;

/// Occurrence counts keyed by a stable label: a bare type name for a node total
/// (`"Expr"`), or `"Type::Variant"` for a per-variant tally. A `BTreeMap` keeps the
/// dump ordered and so byte-reproducible.
#[derive(Default)]
struct Counter {
    counts: BTreeMap<&'static str, u64>,
}

impl Counter {
    fn bump(&mut self, key: &'static str) {
        *self.counts.entry(key).or_default() += 1;
    }

    fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }
}

// Only the nodes this analysis reasons about are overridden; every other node
// recurses through the default `walk_*`, so the counts cover the whole tree. Each
// override bumps its tally, then calls `walk_*` to descend into children.
impl<'ast> Visit<'ast> for Counter {
    fn visit_statement(&mut self, node: &'ast Statement) {
        self.bump("Statement");
        self.bump(match node {
            Statement::Query { .. } => "Statement::Query",
            Statement::CreateTable { .. } => "Statement::CreateTable",
            Statement::AlterTable { .. } => "Statement::AlterTable",
            Statement::Drop { .. } => "Statement::Drop",
            Statement::CreateSchema { .. } => "Statement::CreateSchema",
            Statement::CreateView { .. } => "Statement::CreateView",
            Statement::CreateIndex { .. } => "Statement::CreateIndex",
            Statement::Insert { .. } => "Statement::Insert",
            Statement::Update { .. } => "Statement::Update",
            Statement::Delete { .. } => "Statement::Delete",
            Statement::Transaction { .. } => "Statement::Transaction",
            Statement::Session { .. } => "Statement::Session",
            Statement::AccessControl { .. } => "Statement::AccessControl",
            Statement::Copy { .. } => "Statement::Copy",
            Statement::Explain { .. } => "Statement::Explain",
            // `Statement` is `#[non_exhaustive]`; `Other` plus any future variant.
            _ => "Statement::<other>",
        });
        visit::walk_statement(self, node);
    }

    fn visit_expr(&mut self, node: &'ast Expr) {
        self.bump("Expr");
        visit::walk_expr(self, node);
    }

    fn visit_select(&mut self, node: &'ast Select) {
        self.bump("Select");
        visit::walk_select(self, node);
    }

    fn visit_table_element(&mut self, node: &'ast TableElement) {
        self.bump("TableElement");
        self.bump(match node {
            TableElement::Column { .. } => "TableElement::Column",
            TableElement::Constraint { .. } => "TableElement::Constraint",
            TableElement::Like { .. } => "TableElement::Like",
        });
        visit::walk_table_element(self, node);
    }

    fn visit_alter_table_action(&mut self, node: &'ast AlterTableAction) {
        self.bump("AlterTableAction");
        self.bump(match node {
            AlterTableAction::SetColocationGroup { .. } => "AlterTableAction::SetColocationGroup",
            AlterTableAction::DropColocationGroup { .. } => "AlterTableAction::DropColocationGroup",
            AlterTableAction::AddColumn { .. } => "AlterTableAction::AddColumn",
            AlterTableAction::DropColumn { .. } => "AlterTableAction::DropColumn",
            AlterTableAction::AlterColumn { .. } => "AlterTableAction::AlterColumn",
            AlterTableAction::AddConstraint { .. } => "AlterTableAction::AddConstraint",
            AlterTableAction::DropConstraint { .. } => "AlterTableAction::DropConstraint",
            AlterTableAction::DropPrimaryKey { .. } => "AlterTableAction::DropPrimaryKey",
            AlterTableAction::SetOptions { .. } => "AlterTableAction::SetOptions",
            AlterTableAction::RenameColumn { .. } => "AlterTableAction::RenameColumn",
            AlterTableAction::RenameConstraint { .. } => "AlterTableAction::RenameConstraint",
            AlterTableAction::RenameTable { .. } => "AlterTableAction::RenameTable",
            AlterTableAction::AttachPartition { .. } => "AlterTableAction::AttachPartition",
            AlterTableAction::DetachPartition { .. } => "AlterTableAction::DetachPartition",
        });
        visit::walk_alter_table_action(self, node);
    }

    fn visit_column_option(&mut self, node: &'ast ColumnOption) {
        self.bump("ColumnOption");
        if matches!(node, ColumnOption::References { .. }) {
            self.bump("ColumnOption::References");
        }
        visit::walk_column_option(self, node);
    }

    fn visit_table_constraint(&mut self, node: &'ast TableConstraint) {
        self.bump("TableConstraint");
        if matches!(node, TableConstraint::ForeignKey { .. }) {
            self.bump("TableConstraint::ForeignKey");
        }
        visit::walk_table_constraint(self, node);
    }

    fn visit_table_constraint_def(&mut self, node: &'ast TableConstraintDef) {
        // `box-table-constraint-def-fat-variant` closed won't-do 2026-06-29: once
        // `box-foreign-key-ref-fat-variant` shrank `TableConstraintDef` below
        // `ColumnDef`, boxing here would no longer shrink `TableElement`/
        // `AlterTableAction` (now bounded by `ColumnDef`). This count is the
        // reopen-if-`ColumnDef`-shrinks baseline, not a live proposal.
        self.bump("TableConstraintDef");
        visit::walk_table_constraint_def(self, node);
    }

    fn visit_foreign_key_ref(&mut self, node: &'ast ForeignKeyRef) {
        self.bump("ForeignKeyRef");
        visit::walk_foreign_key_ref(self, node);
    }

    fn visit_join_operator(&mut self, node: &'ast JoinOperator) {
        self.bump("JoinOperator");
        self.bump(match node {
            JoinOperator::Inner { .. } => "JoinOperator::Inner",
            JoinOperator::LeftOuter { .. } => "JoinOperator::LeftOuter",
            JoinOperator::RightOuter { .. } => "JoinOperator::RightOuter",
            JoinOperator::FullOuter { .. } => "JoinOperator::FullOuter",
            JoinOperator::AsOf { .. } => "JoinOperator::AsOf",
            JoinOperator::Cross { .. } => "JoinOperator::Cross",
            JoinOperator::Positional { .. } => "JoinOperator::Positional",
            JoinOperator::Semi { .. } => "JoinOperator::Semi",
            JoinOperator::Anti { .. } => "JoinOperator::Anti",
            JoinOperator::Apply { .. } => "JoinOperator::Apply",
        });
        visit::walk_join_operator(self, node);
    }
}

/// Count every node in the parseable subset for one preset.
fn count(preset: Preset) -> Counter {
    let mut counter = Counter::default();
    for sql in subset(preset).included {
        let parsed = parse_owned(preset, sql);
        for stmt in parsed.statements() {
            counter.visit_statement(stmt);
        }
    }
    counter
}

fn pct(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        100.0 * part as f64 / whole as f64
    }
}

/// One row of the size×frequency table: a fat-skewing enum, the variant that skews
/// it, and the inline payload that variant carries.
struct FatEnum {
    enum_name: &'static str,
    size: usize,
    total_key: &'static str,
    fat_variant_key: &'static str,
    /// The inline payload the fat variant carries (what a box would move to the heap).
    carries: &'static str,
}

fn main() {
    // Hot anchors and the inline payloads a box would move off the stack, with their
    // measured `size_of` so the table reads without cross-referencing.
    println!("# AST box/inline evidence (ADR-0007)\n");
    println!("Reference sizes (size_of, stock NoExt layout):");
    println!(
        "  Expr               = {:>3} B  (hot, fully boxed)",
        size_of::<Expr>()
    );
    println!(
        "  Statement          = {:>3} B  (hot, fully boxed)",
        size_of::<Statement>()
    );
    println!("  ColumnDef          = {:>3} B", size_of::<ColumnDef>());
    println!(
        "  TableConstraintDef = {:>3} B",
        size_of::<TableConstraintDef>()
    );
    println!("  ForeignKeyRef      = {:>3} B", size_of::<ForeignKeyRef>());
    println!(
        "  JoinConstraint     = {:>3} B",
        size_of::<JoinConstraint>()
    );

    // Both `carries` groups below are RESOLVED ADR-0007 decisions, not open
    // candidates: `ColumnOption`/`TableConstraint` (carries: ForeignKeyRef) shipped
    // via `box-foreign-key-ref-fat-variant` (`ForeignKeyRef` is `Box`ed in
    // ddl.rs:113/244); `TableElement`/`AlterTableAction` (carries:
    // TableConstraintDef) closed won't-do via `box-table-constraint-def-fat-variant`
    // on 2026-06-29 (see the comment on `visit_table_constraint_def` above). The
    // table stays as the measured baseline both decisions were made against, and the
    // one a future `ColumnDef` shrink would reopen.
    let fat_enums = [
        FatEnum {
            enum_name: "TableElement",
            size: size_of::<TableElement>(),
            total_key: "TableElement",
            fat_variant_key: "TableElement::Constraint",
            carries: "TableConstraintDef",
        },
        FatEnum {
            enum_name: "AlterTableAction",
            size: size_of::<AlterTableAction>(),
            total_key: "AlterTableAction",
            fat_variant_key: "AlterTableAction::AddConstraint",
            carries: "TableConstraintDef",
        },
        FatEnum {
            enum_name: "ColumnOption",
            size: size_of::<ColumnOption>(),
            total_key: "ColumnOption",
            fat_variant_key: "ColumnOption::References",
            carries: "ForeignKeyRef",
        },
        FatEnum {
            enum_name: "TableConstraint",
            size: size_of::<TableConstraint>(),
            total_key: "TableConstraint",
            fat_variant_key: "TableConstraint::ForeignKey",
            carries: "ForeignKeyRef",
        },
    ];

    for preset in PRESETS {
        let c = count(preset);
        let exprs = c.get("Expr");
        let stmts = c.get("Statement");
        let selects = c.get("Select");

        println!("\n========================================================");
        println!("Preset: {}", preset.label());
        println!("========================================================");
        println!(
            "Hot anchors:  Statement = {stmts:>6}   Select = {selects:>6}   Expr = {exprs:>6}"
        );

        println!("\nStatement frequency (by kind):");
        let mut kinds: Vec<(&&'static str, &u64)> = c
            .counts
            .iter()
            .filter(|(k, _)| k.starts_with("Statement::"))
            .collect();
        kinds.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (kind, n) in kinds {
            println!(
                "  {:<28} {:>6}  ({:>5.1}% of statements)",
                kind,
                n,
                pct(*n, stmts)
            );
        }

        println!("\nFat-skewing enums (size_of × frequency):");
        println!(
            "  {:<18} {:>5}  {:>8}  {:>10}  {:>22}",
            "enum", "size", "nodes", "fat-var", "fat-variant share"
        );
        for fe in &fat_enums {
            let total = c.get(fe.total_key);
            let fat = c.get(fe.fat_variant_key);
            println!(
                "  {:<18} {:>4}B  {:>8}  {:>10}  {:>20.1}%   (fat carries {} inline)",
                fe.enum_name,
                fe.size,
                total,
                fat,
                pct(fat, total),
                fe.carries,
            );
        }

        // Allocations corpus-wide: ForeignKeyRef is the shipped box's actual cost
        // (box-foreign-key-ref-fat-variant); TableConstraintDef is what boxing would
        // have cost the closed won't-do decision (box-table-constraint-def-fat-variant).
        println!(
            "\nBoxing allocations corpus-wide:  TableConstraintDef = {}   ForeignKeyRef = {}",
            c.get("TableConstraintDef"),
            c.get("ForeignKeyRef"),
        );

        // JoinOperator is fat (72B) but NOT skewed: four of five variants carry the
        // same JoinConstraint, so no single variant is the outlier a box would fix.
        println!("\nJoinOperator variants (no single fat variant — 4/5 carry JoinConstraint):");
        for v in [
            "JoinOperator::Inner",
            "JoinOperator::LeftOuter",
            "JoinOperator::RightOuter",
            "JoinOperator::FullOuter",
            "JoinOperator::Cross",
        ] {
            println!("  {:<28} {:>6}", v, c.get(v));
        }
    }
}
