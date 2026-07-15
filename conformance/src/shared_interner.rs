// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared-symbol structural comparison for conformance round-trip oracles.
//!
//! `Symbol` values are only meaningful inside their producing resolver. The
//! conformance tests often need to compare a generated tree with a parsed tree,
//! or two independently parsed trees, so this module resolves every identifier
//! to text and interns that text into one test-only interner before using the
//! AST's ordinary derived `PartialEq`.
//!
//! Both directions — the mutating re-intern ([`SymbolRemapper`]) and the
//! read-only collection into a [`SymbolReport`] ([`SymbolCollector`]) — ride the
//! code-generated [`Visit`]/[`VisitMut`] traversal rather than a hand-written
//! walk. The AST carries a raw [`Symbol`] at exactly five leaf sites, each an
//! explicit `let _ = <field>` in the generated walker (so no hook fires):
//! `Ident.sym`, `FunctionArg.name`, `NamedOperatorExpr.op`,
//! `ParameterKind::Named.name`, and `Expr::SessionVariable.name`. Each visitor
//! overrides just those five methods and delegates every other node to the
//! generated `walk_*`, so a new symbol-bearing field is reached automatically
//! once the traversal regenerates — no parallel hand-written walk to maintain.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};

use squonk::interner::{FrozenResolver, Interner};
use squonk_ast::generated::visit::{self, Visit, VisitMut};
use squonk_ast::{
    Expr, FunctionArg, Ident, NamedOperatorExpr, NoExt, ParameterKind, Resolver, Statement, Symbol,
};

/// Two statement lists remapped through one shared test interner.
#[derive(Debug)]
pub(crate) struct SharedSymbolComparison {
    pub(crate) left: Vec<Statement<NoExt>>,
    pub(crate) right: Vec<Statement<NoExt>>,
    resolver: FrozenResolver,
    left_symbols: SymbolReport,
    right_symbols: SymbolReport,
}

impl SharedSymbolComparison {
    pub(crate) fn structurally_equal(&self) -> bool {
        self.left == self.right
    }

    pub(crate) fn failure_message(
        &self,
        reason: &str,
        context: &[(&str, &str)],
        normalized_fallback: Option<(&dyn fmt::Debug, &dyn fmt::Debug)>,
    ) -> String {
        let mut out = String::new();
        writeln!(out, "{reason}").expect("write to string");
        for (label, value) in context {
            writeln!(out, "  {label}: {value:?}").expect("write to string");
        }

        writeln!(out, "  left original symbols: {}", self.left_symbols).expect("write to string");
        writeln!(out, "  right original symbols: {}", self.right_symbols).expect("write to string");
        writeln!(
            out,
            "  shared symbols: {}",
            SymbolReport::from_statement_lists([&self.left, &self.right], &self.resolver)
        )
        .expect("write to string");

        if let Some((left, right)) = normalized_fallback {
            writeln!(out, "  normalized fallback left: {left:#?}").expect("write to string");
            writeln!(out, "  normalized fallback right: {right:#?}").expect("write to string");
        }

        writeln!(out, "  shared left AST: {:#?}", self.left).expect("write to string");
        writeln!(out, "  shared right AST: {:#?}", self.right).expect("write to string");
        out
    }
}

pub(crate) fn compare_statement_with_shared_symbols(
    left: &Statement<NoExt>,
    left_resolver: &dyn Resolver,
    right: &Statement<NoExt>,
    right_resolver: &dyn Resolver,
) -> SharedSymbolComparison {
    compare_statements_with_shared_symbols(
        std::slice::from_ref(left),
        left_resolver,
        std::slice::from_ref(right),
        right_resolver,
    )
}

pub(crate) fn compare_statements_with_shared_symbols(
    left: &[Statement<NoExt>],
    left_resolver: &dyn Resolver,
    right: &[Statement<NoExt>],
    right_resolver: &dyn Resolver,
) -> SharedSymbolComparison {
    let left_symbols = SymbolReport::from_statements(left, left_resolver);
    let right_symbols = SymbolReport::from_statements(right, right_resolver);

    let mut interner = Interner::new();
    let left = remap_statements(left, left_resolver, &mut interner);
    let right = remap_statements(right, right_resolver, &mut interner);
    let resolver = interner.freeze();

    SharedSymbolComparison {
        left,
        right,
        resolver,
        left_symbols,
        right_symbols,
    }
}

fn remap_statements(
    statements: &[Statement<NoExt>],
    resolver: &dyn Resolver,
    interner: &mut Interner,
) -> Vec<Statement<NoExt>> {
    let mut remapped = statements.to_vec();
    let mut remapper = SymbolRemapper { resolver, interner };
    for statement in &mut remapped {
        remapper.visit_statement_mut(statement);
    }
    remapped
}

/// Re-interns every [`Symbol`] a statement carries from its producing `resolver`
/// into the shared `interner`, walking with the generated [`VisitMut`] and
/// overriding only the five raw-`Symbol` leaf sites (see the module note).
///
/// This is [`VisitMut`]'s first production consumer.
struct SymbolRemapper<'a> {
    resolver: &'a dyn Resolver,
    interner: &'a mut Interner,
}

impl SymbolRemapper<'_> {
    fn remap(&mut self, sym: &mut Symbol) {
        let text = self
            .resolver
            .try_resolve(*sym)
            .unwrap_or_else(|| panic!("cannot remap unknown symbol {}", sym.as_u32()));
        *sym = self.interner.intern(text);
    }
}

impl VisitMut<NoExt> for SymbolRemapper<'_> {
    fn visit_ident_mut(&mut self, node: &mut Ident) {
        self.remap(&mut node.sym);
        visit::walk_ident_mut(self, node);
    }

    fn visit_function_arg_mut(&mut self, node: &mut FunctionArg<NoExt>) {
        if let Some(name) = &mut node.name {
            self.remap(name);
        }
        visit::walk_function_arg_mut(self, node);
    }

    fn visit_named_operator_expr_mut(&mut self, node: &mut NamedOperatorExpr<NoExt>) {
        self.remap(&mut node.op);
        visit::walk_named_operator_expr_mut(self, node);
    }

    fn visit_parameter_kind_mut(&mut self, node: &mut ParameterKind) {
        match node {
            ParameterKind::Named { name, .. } => self.remap(name),
            ParameterKind::PositionalLarge { digits } => self.remap(digits),
            _ => {}
        }
        visit::walk_parameter_kind_mut(self, node);
    }

    fn visit_expr_mut(&mut self, node: &mut Expr<NoExt>) {
        if let Expr::SessionVariable { name, .. } = node {
            self.remap(name);
        }
        visit::walk_expr_mut(self, node);
    }
}

#[derive(Debug)]
struct SymbolReport {
    entries: BTreeMap<u32, String>,
}

impl SymbolReport {
    fn from_statements(statements: &[Statement<NoExt>], resolver: &dyn Resolver) -> Self {
        let mut collector = SymbolCollector::new(resolver);
        for statement in statements {
            collector.visit_statement(statement);
        }
        collector.report
    }

    fn from_statement_lists<'a>(
        statements: impl IntoIterator<Item = &'a Vec<Statement<NoExt>>>,
        resolver: &dyn Resolver,
    ) -> Self {
        let mut collector = SymbolCollector::new(resolver);
        for list in statements {
            for statement in list {
                collector.visit_statement(statement);
            }
        }
        collector.report
    }

    fn record(&mut self, sym: Symbol, resolver: &dyn Resolver) {
        let text = resolver
            .try_resolve(sym)
            .map(|text| format!("{text:?}"))
            .unwrap_or_else(|| "<unresolved>".to_string());
        self.entries.entry(sym.as_u32()).or_insert(text);
    }
}

impl fmt::Display for SymbolReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.entries.is_empty() {
            return f.write_str("<none>");
        }

        for (i, (symbol, text)) in self.entries.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "#{symbol}={text}")?;
        }
        Ok(())
    }
}

/// Resolves every [`Symbol`] a statement carries into a [`SymbolReport`], walking
/// with the generated [`Visit`] and overriding only the five raw-`Symbol` leaf
/// sites (see the module note).
struct SymbolCollector<'a> {
    report: SymbolReport,
    resolver: &'a dyn Resolver,
}

impl<'a> SymbolCollector<'a> {
    fn new(resolver: &'a dyn Resolver) -> Self {
        Self {
            report: SymbolReport {
                entries: BTreeMap::new(),
            },
            resolver,
        }
    }

    fn record(&mut self, sym: Symbol) {
        self.report.record(sym, self.resolver);
    }
}

impl<'ast> Visit<'ast, NoExt> for SymbolCollector<'_> {
    fn visit_ident(&mut self, node: &'ast Ident) {
        self.record(node.sym);
        visit::walk_ident(self, node);
    }

    fn visit_function_arg(&mut self, node: &'ast FunctionArg<NoExt>) {
        if let Some(name) = node.name {
            self.record(name);
        }
        visit::walk_function_arg(self, node);
    }

    fn visit_named_operator_expr(&mut self, node: &'ast NamedOperatorExpr<NoExt>) {
        self.record(node.op);
        visit::walk_named_operator_expr(self, node);
    }

    fn visit_parameter_kind(&mut self, node: &'ast ParameterKind) {
        match node {
            ParameterKind::Named { name, .. } => self.record(*name),
            ParameterKind::PositionalLarge { digits } => self.record(*digits),
            _ => {}
        }
        visit::walk_parameter_kind(self, node);
    }

    fn visit_expr(&mut self, node: &'ast Expr<NoExt>) {
        if let Expr::SessionVariable { name, .. } = node {
            self.record(*name);
        }
        visit::walk_expr(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use squonk_ast::{
        AliasSpelling, Meta, NodeId, ObjectName, Query, QuoteStyle, Select, SelectItem,
        SelectSpelling, SetExpr, Span,
    };
    use thin_vec::{ThinVec, thin_vec};

    struct TinyResolver(&'static [&'static str]);

    impl Resolver for TinyResolver {
        fn try_resolve(&self, sym: Symbol) -> Option<&str> {
            self.0.get(sym.index()).copied()
        }
    }

    const LEFT_RESOLVER: TinyResolver = TinyResolver(&["a", "b"]);
    const RIGHT_RESOLVER: TinyResolver = TinyResolver(&["x", "a"]);

    #[test]
    fn shared_symbols_allow_raw_symbol_ids_to_differ() {
        let left = select_column(1);
        let right = select_column(2);
        let comparison =
            compare_statement_with_shared_symbols(&left, &LEFT_RESOLVER, &right, &RIGHT_RESOLVER);

        assert_ne!(
            left, right,
            "the control comparison must fail before shared-symbol remapping",
        );
        assert!(comparison.structurally_equal());
    }

    #[test]
    fn failure_diagnostics_include_original_and_shared_symbols() {
        let comparison = compare_statement_with_shared_symbols(
            &select_column(1),
            &LEFT_RESOLVER,
            &select_column(2),
            &LEFT_RESOLVER,
        );

        let message = comparison.failure_message("test mismatch", &[("sql", "SELECT a")], None);

        assert!(message.contains("test mismatch"));
        assert!(message.contains("left original symbols: #1=\"a\""));
        assert!(message.contains("right original symbols: #2=\"b\""));
        assert!(message.contains("shared symbols:"));
        assert!(message.contains("#"));
    }

    fn select_column(sym: u32) -> Statement<NoExt> {
        Statement::Query {
            query: Box::new(Query {
                with: None,
                body: SetExpr::Select {
                    select: Box::new(Select {
                        distinct: None,
                        straight_join: false,
                        projection: thin_vec![SelectItem::Expr {
                            expr: Expr::Column {
                                name: ObjectName(thin_vec![Ident {
                                    sym: Symbol::new(sym).expect("symbol is one-based"),
                                    quote: QuoteStyle::None,
                                    meta: meta(),
                                }]),
                                meta: meta(),
                            },
                            alias: None,
                            alias_spelling: AliasSpelling::As,
                            meta: meta(),
                        }],
                        into: None,
                        from: ThinVec::new(),
                        lateral_views: ThinVec::new(),
                        connect_by: None,
                        selection: None,
                        group_by: ThinVec::new(),
                        group_by_quantifier: None,
                        group_by_all: None,
                        having: None,
                        windows: ThinVec::new(),
                        qualify: None,
                        sample: None,
                        spelling: SelectSpelling::Select,
                        meta: meta(),
                    }),
                    meta: meta(),
                },
                order_by: ThinVec::new(),
                order_by_all: None,
                limit_by: None,
                limit: None,
                settings: ThinVec::new(),
                format: None,
                locking: ThinVec::new(),
                pipe_operators: ThinVec::new(),
                for_clause: None,
                meta: meta(),
            }),
            meta: meta(),
        }
    }

    fn meta() -> Meta {
        Meta::new(
            Span::SYNTHETIC,
            NodeId::new(1).expect("node ids are one-based"),
        )
    }
}
