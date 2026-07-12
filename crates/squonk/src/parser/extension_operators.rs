// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Extension operator precedence through typed hooks; tests only.
//!
//! A custom dialect (`OpDialect`) defines three operators over lexemes the core
//! grammar leaves free — a left-associative infix `^` ("match"), a non-associative
//! infix `&` ("compare"), and a prefix `~` ("neg") — through the typed [`Dialect`]
//! operator hooks. The hooks only *recognize* an operator and report its binding
//! power; the parser owns the precedence climb, so a custom operator cannot ignore
//! its own right binding power the way the prior art's `parse_infix` hooks did. These
//! tests pin that the operators bind at their reported precedence, that the renderer
//! derives parentheses from the *same* binding power, that custom-operator trees
//! round-trip, and that the non-associative `&` rejects an unparenthesized chain
//! (`a & b & c`) the way `a < b < c` is rejected.

use std::fmt;
use std::sync::Arc;

use crate::ast::precedence::{Assoc, BindingPower};
use crate::ast::render::{
    Render, RenderConfig, RenderCtx, RenderExt as _, RenderMode, render_extension_infix,
    render_extension_prefix,
};
use crate::ast::{BinaryOperator, Expr, SelectItem, SetExpr, Span, Spanned, Statement};
use crate::error::ParseResult;
use crate::parser::{Dialect, Parsed, Parser, parse_with};
use crate::tokenizer::{Operator, Token, TokenKind};

/// Binding power of the infix `^` operator: tighter than `+`/`*`/comparison, looser
/// than the PostgreSQL postfix operators. Left-associative.
const MATCH_BP: BindingPower = BindingPower {
    left: 64,
    right: 65,
    assoc: Assoc::Left,
};

/// Binding power of the non-associative infix `&` ("compare"): deliberately the
/// built-in comparison rank (`40`/`41`), so it neither chains with itself
/// (`a & b & c`) nor with a built-in comparison at the same rank (`a & b < c`) — the
/// level-based, not identity-based, chain check. Its `left < right` encoding is what
/// routes a second `&` back to the enclosing precedence climb, where the chain check
/// runs.
const CMP_BP: BindingPower = BindingPower {
    left: 40,
    right: 41,
    assoc: Assoc::NonAssoc,
};

/// Prefix binding power of `~`: tighter than the infix `^` and the built-in sign.
const NEG_PREFIX_BP: u8 = 82;

/// The custom AST nodes `OpDialect` produces in `Expr::Other`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum OpExt {
    /// `left ^ right` — the custom left-associative infix operator.
    Match {
        left: Box<Expr<OpExt>>,
        right: Box<Expr<OpExt>>,
        span: Span,
    },
    /// `left & right` — the custom non-associative infix operator.
    Cmp {
        left: Box<Expr<OpExt>>,
        right: Box<Expr<OpExt>>,
        span: Span,
    },
    /// `~ operand` — the custom prefix operator.
    Neg {
        operand: Box<Expr<OpExt>>,
        span: Span,
    },
}

impl Spanned for OpExt {
    fn span(&self) -> Span {
        match self {
            OpExt::Match { span, .. } | OpExt::Cmp { span, .. } | OpExt::Neg { span, .. } => *span,
        }
    }
}

impl Render for OpExt {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpExt::Match { left, right, .. } => {
                render_extension_infix(ctx, f, MATCH_BP, (left, right), |f| f.write_str(" ^ "))
            }
            OpExt::Cmp { left, right, .. } => {
                render_extension_infix(ctx, f, CMP_BP, (left, right), |f| f.write_str(" & "))
            }
            OpExt::Neg { operand, .. } => {
                render_extension_prefix(ctx, f, NEG_PREFIX_BP, |f| f.write_str("~"), operand)
            }
        }
    }

    fn operand_binding_power(&self) -> Option<BindingPower> {
        Some(match self {
            OpExt::Match { .. } => MATCH_BP,
            OpExt::Cmp { .. } => CMP_BP,
            // A prefix operator closes its left edge; modelled as a tight,
            // right-leaning operand so `(~a)::int` keeps its parens but `~a ^ b`
            // does not (the prefix binds tighter than the infix).
            OpExt::Neg { .. } => BindingPower {
                left: NEG_PREFIX_BP,
                right: NEG_PREFIX_BP,
                assoc: Assoc::Right,
            },
        })
    }
}

#[derive(Clone, Copy)]
struct OpDialect;

impl Dialect for OpDialect {
    type Ext = OpExt;

    fn features(&self) -> &crate::ast::dialect::FeatureSet {
        &crate::ast::dialect::FeatureSet::ANSI
    }

    fn peek_infix_operator_hook<'a>(
        parser: &mut Parser<'a, Self>,
    ) -> ParseResult<Option<BindingPower>> {
        Ok(match parser.peek()? {
            Some(token) if token.kind == TokenKind::Operator(Operator::Caret) => Some(MATCH_BP),
            Some(token) if token.kind == TokenKind::Operator(Operator::Amp) => Some(CMP_BP),
            _ => None,
        })
    }

    fn build_infix_operator<'a>(
        parser: &mut Parser<'a, Self>,
        op: Token,
        left: Expr<OpExt>,
        right: Expr<OpExt>,
    ) -> ParseResult<Expr<OpExt>> {
        let span = left.span().union(right.span());
        let (left, right) = (Box::new(left), Box::new(right));
        let ext = match op.kind {
            TokenKind::Operator(Operator::Caret) => OpExt::Match { left, right, span },
            TokenKind::Operator(Operator::Amp) => OpExt::Cmp { left, right, span },
            other => {
                unreachable!("peek_infix_operator_hook only recognizes `^` and `&`, got {other:?}")
            }
        };
        let meta = parser.make_meta(span);
        Ok(Expr::Other { ext, meta })
    }

    fn extension_operand_binding_power(ext: &OpExt) -> Option<BindingPower> {
        // Parse-time chain rejection reads the *same* source as render-time grouping:
        // a real dialect whose `Ext: Render` can delegate straight to the render
        // accessor, so the two binding powers cannot drift.
        ext.operand_binding_power()
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
        operand: Expr<OpExt>,
    ) -> ParseResult<Expr<OpExt>> {
        debug_assert_eq!(op.kind, TokenKind::Operator(Operator::Tilde));
        let span = op.span.union(operand.span());
        let ext = OpExt::Neg {
            operand: Box::new(operand),
            span,
        };
        let meta = parser.make_meta(span);
        Ok(Expr::Other { ext, meta })
    }
}

/// Parse one `SELECT <expr>` under `OpDialect`.
fn parse(src: &str) -> Parsed<Arc<str>, OpExt> {
    parse_with(src, OpDialect).expect("OpDialect parses the test expression")
}

/// The single projection expression of a parsed `SELECT <expr>`.
fn projection_expr(parsed: &Parsed<Arc<str>, OpExt>) -> &Expr<OpExt> {
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
    parse_with(src, OpDialect)
        .expect("OpDialect parses the test expression")
        .to_string()
}

/// Fully-parenthesized render (the precedence oracle).
fn parenthesized(src: &str) -> String {
    let parsed = parse_with(src, OpDialect).expect("OpDialect parses the test expression");
    let config = RenderConfig {
        mode: RenderMode::Parenthesized,
        ..RenderConfig::default()
    };
    let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
    parsed.statements()[0].displayed(&ctx).to_string()
}

#[test]
fn infix_hook_binds_at_its_reported_right_power() {
    // `a ^ b + c` must bind as `(a ^ b) + c`: `^` (64) binds tighter than `+` (50),
    // and the parser climbs the operator's right operand at the hook's reported
    // right power (65). A hook that ignored that power and greedily consumed the
    // rest of the expression would instead yield `a ^ (b + c)` — the exact mis-bind
    // ADR-0008's typed-hook discipline makes structurally impossible.
    let parsed = parse("SELECT a ^ b + c");
    let expr = projection_expr(&parsed);
    let Expr::BinaryOp { left, op, .. } = expr else {
        panic!("expected a top-level `+`, got {expr:?}");
    };
    assert_eq!(*op, BinaryOperator::Plus);
    assert!(
        matches!(
            &**left,
            Expr::Other {
                ext: OpExt::Match { .. },
                ..
            }
        ),
        "the `^` must have bound only `a ^ b`, leaving `+ c` to the outer climb",
    );

    // The mirror: `a + b ^ c` binds as `a + (b ^ c)`.
    let mirror = parse("SELECT a + b ^ c");
    let Expr::BinaryOp { right, op, .. } = projection_expr(&mirror) else {
        panic!("expected a top-level `+`");
    };
    assert_eq!(*op, BinaryOperator::Plus);
    assert!(matches!(
        &**right,
        Expr::Other {
            ext: OpExt::Match { .. },
            ..
        }
    ));
}

#[test]
fn infix_hook_left_associates() {
    // `a ^ b ^ c` folds left into `(a ^ b) ^ c`.
    let parsed = parse("SELECT a ^ b ^ c");
    let Expr::Other {
        ext: OpExt::Match { left, right, .. },
        ..
    } = projection_expr(&parsed)
    else {
        panic!("expected a top-level `^`");
    };
    assert!(
        matches!(
            &**left,
            Expr::Other {
                ext: OpExt::Match { .. },
                ..
            }
        ),
        "left operand should be the nested `a ^ b`",
    );
    assert!(
        matches!(&**right, Expr::Column { .. }),
        "right operand should be the bare column `c`",
    );
}

#[test]
fn prefix_hook_binds_tighter_than_infix() {
    // `~ a ^ b` binds as `(~a) ^ b`: the prefix `~` (82) climbs only `a` (the infix
    // `^` at 64 is looser than the prefix's operand power), then `^` folds in `b`.
    let parsed = parse("SELECT ~ a ^ b");
    let Expr::Other {
        ext: OpExt::Match { left, .. },
        ..
    } = projection_expr(&parsed)
    else {
        panic!("expected a top-level `^`");
    };
    assert!(
        matches!(
            &**left,
            Expr::Other {
                ext: OpExt::Neg { .. },
                ..
            }
        ),
        "the `^` left operand should be the prefix `~a`",
    );
}

#[test]
fn renders_minimal_parens_from_the_same_binding_power() {
    // Render derives parentheses from the same table the parser climbed, so each
    // tree round-trips with exactly the parentheses precedence demands.
    assert_eq!(canonical("SELECT a ^ b + c"), "SELECT a ^ b + c");
    assert_eq!(canonical("SELECT a + b ^ c"), "SELECT a + b ^ c");
    assert_eq!(canonical("SELECT a * b ^ c"), "SELECT a * b ^ c");
    assert_eq!(canonical("SELECT a ^ b * c"), "SELECT a ^ b * c");
    assert_eq!(canonical("SELECT a ^ b = c"), "SELECT a ^ b = c");
    assert_eq!(canonical("SELECT a ^ b ^ c"), "SELECT a ^ b ^ c");
    assert_eq!(canonical("SELECT ~ a ^ b"), "SELECT ~a ^ b");
    assert_eq!(canonical("SELECT ~ a + b"), "SELECT ~a + b");

    // A right-nested tree reached only through explicit source parentheses: the
    // left-associative `^` must reintroduce the parens its grouping requires, or the
    // re-parse would left-associate into a different tree.
    assert_eq!(canonical("SELECT a ^ (b ^ c)"), "SELECT a ^ (b ^ c)");
    // And a custom-operator operand of a built-in operator that binds looser keeps
    // its grouping (`(a + b)` under the tighter `^`).
    assert_eq!(canonical("SELECT (a + b) ^ c"), "SELECT (a + b) ^ c");
}

#[test]
fn custom_operator_trees_round_trip() {
    // The canonical render is a fixpoint: re-parsing it yields the same canonical
    // text, so the parentheses the renderer adds reproduce the parsed structure
    // (the ADR-0014 round-trip oracle, extended to custom-operator trees).
    for src in [
        "SELECT a ^ b + c",
        "SELECT a + b ^ c",
        "SELECT a ^ b ^ c",
        "SELECT a ^ (b ^ c)",
        "SELECT (a + b) ^ c",
        "SELECT ~ a ^ b",
        "SELECT ~ (a ^ b)",
        "SELECT a ^ b = c AND d",
    ] {
        let once = canonical(src);
        let twice = canonical(&once);
        assert_eq!(once, twice, "canonical render of `{src}` is not a fixpoint");
    }
}

#[test]
fn parenthesized_mode_fully_wraps_custom_operators() {
    // The oracle mode wraps every custom operator like a built-in one, proving the
    // extension nodes route through the same self-wrapping path.
    assert_eq!(parenthesized("SELECT a ^ b ^ c"), "SELECT ((a ^ b) ^ c)");
    assert_eq!(parenthesized("SELECT a ^ b + c"), "SELECT ((a ^ b) + c)");
    assert_eq!(parenthesized("SELECT ~ a ^ b"), "SELECT ((~a) ^ b)");
}

#[test]
fn nonassoc_infix_hook_rejects_chains() {
    // `a & b & c` for the non-associative `&` is a clean parse error at the second
    // operator, exactly like `a < b < c` for the built-in comparisons — never a
    // silently associated parse (ADR-0008). The chain check reads the left operand's
    // precedence back through `Dialect::extension_operand_binding_power`.
    let err = parse_with("SELECT a & b & c", OpDialect)
        .expect_err("the non-associative `&` does not chain");
    assert_eq!(err.expected.as_str(), "the end of the operator chain");
    // `SELECT a & b & c`: the second `&` sits at bytes 13..14.
    assert_eq!(err.span, Span::new(13, 14));

    // A single `&` is fine, and binds a plain `Cmp` node.
    let parsed = parse_with("SELECT a & b", OpDialect).expect("a single `&` parses");
    assert!(matches!(
        projection_expr(&parsed),
        Expr::Other {
            ext: OpExt::Cmp { .. },
            ..
        }
    ));
}

#[test]
fn parenthesized_nonassoc_extension_resets_chain_detection() {
    // Explicit grouping suppresses the next chain check (the parser's transient
    // `grouped` barrier), so both hand-groupings of the non-associative `&` parse, and
    // each round-trips with the parentheses its grouping requires — the bare chain can
    // never be re-parsed, so the renderer must reintroduce the parens.
    for (src, expected) in [
        ("SELECT (a & b) & c", "SELECT (a & b) & c"),
        ("SELECT a & (b & c)", "SELECT a & (b & c)"),
    ] {
        let canon = canonical(src);
        assert_eq!(canon, expected);
        assert_eq!(
            canonical(&canon),
            canon,
            "grouped `&` render is not a fixpoint"
        );
    }

    // The left grouping is genuinely `(a & b) & c`: a `Cmp` whose left operand is a
    // nested `Cmp`.
    let parsed = parse_with("SELECT (a & b) & c", OpDialect).expect("left grouping parses");
    let Expr::Other {
        ext: OpExt::Cmp { left, .. },
        ..
    } = projection_expr(&parsed)
    else {
        panic!("expected the outer `&`");
    };
    assert!(
        matches!(
            &**left,
            Expr::Other {
                ext: OpExt::Cmp { .. },
                ..
            }
        ),
        "the parenthesized `a & b` is the left operand",
    );
}

#[test]
fn nonassoc_extension_and_builtin_comparison_do_not_chain_across_the_boundary() {
    // `&` sits at the built-in comparison rank, and the chain check is level-based, not
    // operator-identity based: a non-associative extension operator and a built-in
    // comparison at the same rank reject each other in either order. This exercises
    // both consult paths — the built-in loop reading an `Expr::Other` left operand, and
    // the extension branch reading a built-in `Expr::BinaryOp` left operand.
    for src in ["SELECT a & b < c", "SELECT a < b & c"] {
        parse_with(src, OpDialect)
            .expect_err("a non-associative extension operator does not chain with a comparison");
    }

    // Grouping still resets it across the boundary.
    assert_eq!(canonical("SELECT (a & b) < c"), "SELECT (a & b) < c");
    assert_eq!(canonical("SELECT a & (b < c)"), "SELECT a & (b < c)");
}
