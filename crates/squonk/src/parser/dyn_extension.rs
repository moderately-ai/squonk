// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The dynamic extension hatch end-to-end: a dialect whose `Ext` is the
//! type-erased [`DynExt`] rather than a single typed enum; tests only.
//!
//! This is the runtime-composition counterpart to `extension_operators`, and its
//! point is twofold. First, it proves the parser's generic seam needs **no** changes
//! to drive the dynamic hatch: `Parser` is generic over `Dialect::Ext: Extension`,
//! and `DynExt` is an `Extension`, so the *same* precedence-climbing hooks that
//! build a typed `Expr::Other` build an erased one. Second, it composes **two
//! unrelated** concrete node types — an infix `^` and a prefix `~`, defined as
//! separate structs, not variants of one enum — behind a single `DynExt`, the thing
//! the typed path cannot do without a hand-written sum type naming both up front.
//! The custom operators round-trip through the renderer by the same binding-power
//! rule as the typed ones, and a visitor recovers each concrete type by downcast.

use std::fmt;
use std::sync::Arc;

use crate::ast::generated::visit::Visit;
use crate::ast::precedence::{Assoc, BindingPower};
use crate::ast::render::{
    DynExt, Render, RenderConfig, RenderCtx, RenderExt as _, RenderMode, render_extension_infix,
    render_extension_prefix,
};
use crate::ast::{Expr, SelectItem, SetExpr, Span, Spanned, Statement};
use crate::error::ParseResult;
use crate::parser::{Dialect, Parsed, Parser, parse_with};
use crate::tokenizer::{Operator, Token, TokenKind};

/// Binding power of the infix `^`: tighter than `+`/`*`/comparison. Left-associative.
const MATCH_BP: BindingPower = BindingPower {
    left: 64,
    right: 65,
    assoc: Assoc::Left,
};

/// Prefix binding power of `~`: tighter than the infix `^`.
const NEG_PREFIX_BP: u8 = 82;

/// The custom infix `^` node — its *own* concrete type, erased into `DynExt`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DynMatch {
    left: Box<Expr<DynExt>>,
    right: Box<Expr<DynExt>>,
    span: Span,
}

impl Spanned for DynMatch {
    fn span(&self) -> Span {
        self.span
    }
}

impl Render for DynMatch {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        render_extension_infix(ctx, f, MATCH_BP, (&self.left, &self.right), |f| {
            f.write_str(" ^ ")
        })
    }

    fn operand_binding_power(&self) -> Option<BindingPower> {
        Some(MATCH_BP)
    }
}

/// The custom prefix `~` node — a *separate* concrete type, also erased into `DynExt`,
/// so one tree holds two unrelated extension kinds at once.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DynNeg {
    operand: Box<Expr<DynExt>>,
    span: Span,
}

impl Spanned for DynNeg {
    fn span(&self) -> Span {
        self.span
    }
}

impl Render for DynNeg {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        render_extension_prefix(ctx, f, NEG_PREFIX_BP, |f| f.write_str("~"), &self.operand)
    }

    fn operand_binding_power(&self) -> Option<BindingPower> {
        // A prefix operator closes its left edge; modelled as a tight, right-leaning
        // operand so `~a ^ b` does not parenthesize but a looser operand would.
        Some(BindingPower {
            left: NEG_PREFIX_BP,
            right: NEG_PREFIX_BP,
            assoc: Assoc::Right,
        })
    }
}

/// A dialect that builds *erased* extension nodes: `type Ext = DynExt`.
#[derive(Clone, Copy)]
struct DynDialect;

impl Dialect for DynDialect {
    type Ext = DynExt;

    fn features(&self) -> &crate::ast::dialect::FeatureSet {
        &crate::ast::dialect::FeatureSet::ANSI
    }

    fn peek_infix_operator_hook<'a>(
        parser: &mut Parser<'a, Self>,
    ) -> ParseResult<Option<BindingPower>> {
        match parser.peek()? {
            Some(token) if token.kind == TokenKind::Operator(Operator::Caret) => Ok(Some(MATCH_BP)),
            _ => Ok(None),
        }
    }

    fn build_infix_operator<'a>(
        parser: &mut Parser<'a, Self>,
        op: Token,
        left: Expr<DynExt>,
        right: Expr<DynExt>,
    ) -> ParseResult<Expr<DynExt>> {
        debug_assert_eq!(op.kind, TokenKind::Operator(Operator::Caret));
        let span = left.span().union(right.span());
        // The same hook shape as the typed dialect — only the node is erased here.
        let ext = DynExt::new(DynMatch {
            left: Box::new(left),
            right: Box::new(right),
            span,
        });
        let meta = parser.make_meta(span);
        Ok(Expr::Other { ext, meta })
    }

    fn peek_prefix_operator_hook<'a>(parser: &mut Parser<'a, Self>) -> ParseResult<Option<u8>> {
        match parser.peek()? {
            Some(token) if token.kind == TokenKind::Operator(Operator::Tilde) => {
                Ok(Some(NEG_PREFIX_BP))
            }
            _ => Ok(None),
        }
    }

    fn build_prefix_operator<'a>(
        parser: &mut Parser<'a, Self>,
        op: Token,
        operand: Expr<DynExt>,
    ) -> ParseResult<Expr<DynExt>> {
        debug_assert_eq!(op.kind, TokenKind::Operator(Operator::Tilde));
        let span = op.span.union(operand.span());
        let ext = DynExt::new(DynNeg {
            operand: Box::new(operand),
            span,
        });
        let meta = parser.make_meta(span);
        Ok(Expr::Other { ext, meta })
    }
}

/// The single projection expression of a parsed `SELECT <expr>`.
fn projection_expr(parsed: &Parsed<Arc<str>, DynExt>) -> &Expr<DynExt> {
    let Statement::Query { query, .. } = &parsed.statements()[0] else {
        panic!("expected a query statement");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    let SelectItem::Expr { expr, .. } = &select.projection[0] else {
        panic!("expected a bare projection expression");
    };
    expr
}

/// Canonically render a parsed `SELECT` and return its SQL text.
fn canonical(src: &str) -> String {
    parse_with(src, DynDialect)
        .expect("DynDialect parses the test expression")
        .to_string()
}

#[test]
fn dynamic_dialect_parses_and_renders_with_no_parser_changes() {
    // Parse → erased `Expr::Other` → render, all through the stock generic seam. The
    // custom operators bind and parenthesize by the same binding-power table as the
    // typed dialect, so the erased trees round-trip identically.
    assert_eq!(canonical("SELECT ~ a ^ b"), "SELECT ~a ^ b");
    assert_eq!(canonical("SELECT a ^ b + c"), "SELECT a ^ b + c");
    assert_eq!(canonical("SELECT (a + b) ^ c"), "SELECT (a + b) ^ c");
    assert_eq!(canonical("SELECT a ^ (b ^ c)"), "SELECT a ^ (b ^ c)");
}

#[test]
fn dynamic_custom_operator_trees_round_trip() {
    // The canonical render is a fixpoint, just as for typed extensions (ADR-0014).
    for src in [
        "SELECT ~ a ^ b",
        "SELECT a ^ b + c",
        "SELECT a ^ (b ^ c)",
        "SELECT (a + b) ^ c",
        "SELECT ~ (a ^ b)",
    ] {
        let once = canonical(src);
        let twice = canonical(&once);
        assert_eq!(once, twice, "canonical render of `{src}` is not a fixpoint");
    }
}

#[test]
fn parenthesized_mode_wraps_erased_operators_like_built_ins() {
    let parsed = parse_with("SELECT ~ a ^ b", DynDialect).expect("parses");
    let config = RenderConfig {
        mode: RenderMode::Parenthesized,
        ..RenderConfig::default()
    };
    let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
    assert_eq!(
        parsed.statements()[0].displayed(&ctx).to_string(),
        "SELECT ((~a) ^ b)",
    );
}

#[test]
fn visitor_downcasts_heterogeneous_erased_nodes() {
    // `~ a ^ b` parses to `Other(DynMatch { left: Other(DynNeg { a }), right: b })`:
    // two *different* concrete extension types in one tree. A visitor recovers each
    // by downcast — the payoff of the erased hatch. The generated walker hands the
    // top `DynExt` to `visit_extension`; the extensions' own children are not part of
    // the generated shape, so the visitor recurses into them by hand.
    #[derive(Default)]
    struct Count {
        matches: usize,
        negs: usize,
    }

    impl<'ast> Visit<'ast, DynExt> for Count {
        fn visit_extension(&mut self, node: &'ast DynExt) {
            if let Some(m) = node.downcast_ref::<DynMatch>() {
                self.matches += 1;
                self.visit_expr(&m.left);
                self.visit_expr(&m.right);
            } else if let Some(n) = node.downcast_ref::<DynNeg>() {
                self.negs += 1;
                self.visit_expr(&n.operand);
            }
        }
    }

    let parsed = parse_with("SELECT ~ a ^ b", DynDialect).expect("parses");
    let mut count = Count::default();
    count.visit_expr(projection_expr(&parsed));

    assert_eq!(count.matches, 1, "one infix `^` (DynMatch)");
    assert_eq!(count.negs, 1, "one prefix `~` (DynNeg)");
}
