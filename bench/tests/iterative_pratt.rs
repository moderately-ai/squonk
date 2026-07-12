// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Parity gate for the iterative-vs-recursive Pratt spike (`spike-iterative-pratt`).
//!
//! The spike's perf comparison is only meaningful if the two parsers are the SAME
//! parser by behaviour, so this locks that down: identical trees on the battery,
//! identical precedence/associativity, identical non-associative rejection, and —
//! the property the whole spike exists for — the iterative parser parsing a depth
//! that would overflow the recursive one. Shares the prototype core via the same
//! `#[path]` include the driver example uses.

#[path = "../benches/iterative_pratt_ref/mod.rs"]
mod iterative_pratt_ref;

use iterative_pratt_ref::{
    BinOp, Expr, IterParser, ParseError, SHALLOW_CASES, UnOp, nested_parens, node_count,
    prefix_chain, rec_parse, tokenize,
};

/// Parse with both parsers and assert they agree; return the (shared) result.
fn both(src: &str) -> Result<Expr, ParseError> {
    let toks = tokenize(src).expect("test input tokenizes");
    let r = rec_parse(&toks);
    let i = IterParser::new().parse(&toks);
    assert_eq!(r, i, "recursive and iterative disagree on {src:?}");
    r
}

fn bin(l: Expr, op: BinOp, r: Expr) -> Expr {
    Expr::Binary(Box::new(l), op, Box::new(r))
}

fn v(id: u32) -> Expr {
    Expr::Var(id)
}

#[test]
fn battery_parity() {
    for &(name, src) in SHALLOW_CASES {
        let r = both(src);
        assert!(
            r.is_ok(),
            "battery case {name:?} ({src:?}) should parse: {r:?}"
        );
    }
}

#[test]
fn precedence_multiplicative_over_additive() {
    // a + b * c == a + (b * c)
    assert_eq!(
        both("a + b * c").unwrap(),
        bin(v(0), BinOp::Add, bin(v(1), BinOp::Mul, v(2))),
    );
}

#[test]
fn left_associative_folds_left() {
    // a - b - c == (a - b) - c  (additive is left-assoc)
    assert_eq!(
        both("a - b - c").unwrap(),
        bin(bin(v(0), BinOp::Sub, v(1)), BinOp::Sub, v(2)),
    );
}

#[test]
fn right_associative_folds_right() {
    // a ^ b ^ c == a ^ (b ^ c)  — the iterative fold's right-recursion path, the
    // one most likely to expose an explicit-stack bug.
    assert_eq!(
        both("a ^ b ^ c").unwrap(),
        bin(v(0), BinOp::Pow, bin(v(1), BinOp::Pow, v(2))),
    );
}

#[test]
fn parens_store_no_node() {
    // Redundant parens collapse: ((a)) is just the leaf, not a wrapper node.
    assert_eq!(both("((a))").unwrap(), v(0));
    // (a + b) * c == grouped sum times c (parens only regroup, never a node).
    assert_eq!(
        both("(a + b) * c").unwrap(),
        bin(bin(v(0), BinOp::Add, v(1)), BinOp::Mul, v(2)),
    );
}

#[test]
fn prefix_sign_binds_tighter_than_arithmetic() {
    // - a * b == (- a) * b  (prefix sign bp 80 > multiplicative 60)
    assert_eq!(
        both("- a * b").unwrap(),
        bin(Expr::Unary(UnOp::Neg, Box::new(v(0))), BinOp::Mul, v(1)),
    );
}

#[test]
fn prefix_not_binds_looser_than_comparison() {
    // ! a = b == ! (a = b)  (prefix not bp 30 < comparison 40), mirroring the real
    // `NOT a = b` == `NOT (a = b)`.
    assert_eq!(
        both("! a = b").unwrap(),
        Expr::Unary(UnOp::Not, Box::new(bin(v(0), BinOp::Eq, v(1)))),
    );
}

#[test]
fn non_associative_chain_rejected() {
    // a = b = c and a < b < c are clean errors in both parsers.
    assert_eq!(both("a = b = c"), Err(ParseError::NonAssocChain));
    assert_eq!(both("a < b < c"), Err(ParseError::NonAssocChain));
    // But a grouped comparison suppresses exactly the next chain check, so
    // (a = b) = c is accepted (matches the production `grouped` carve-out).
    assert!(both("(a = b) = c").is_ok());
    assert!(both("a = (b = c)").is_ok());
}

#[test]
fn errors_match() {
    assert_eq!(both("("), Err(ParseError::ExpectedExpr));
    assert_eq!(both("(a"), Err(ParseError::ExpectedRParen));
    assert_eq!(both("a b"), Err(ParseError::TrailingTokens));
    assert_eq!(both(""), Err(ParseError::ExpectedExpr));
}

#[test]
fn deep_parity_at_recursion_safe_depth() {
    // At a depth the recursive parser can still handle, the two agree exactly.
    for shape_src in [nested_parens(300), prefix_chain(300)] {
        let toks = tokenize(&shape_src).expect("tokenizes");
        assert_eq!(rec_parse(&toks), IterParser::new().parse(&toks));
    }
}

#[test]
fn iterative_parses_depth_that_would_overflow_recursive() {
    // The whole point: 50k-deep nesting parses iteratively with no stack growth.
    // (We do NOT run the recursive parser here — it would overflow and abort.)
    let mut iter = IterParser::with_capacity(1 << 16);

    let parens = tokenize(&nested_parens(50_000)).expect("tokenizes");
    let tree = iter.parse(&parens).expect("deep parens parse iteratively");
    assert_eq!(node_count(&tree), 1, "parens store no node: one leaf");

    let prefix = tokenize(&prefix_chain(50_000)).expect("tokenizes");
    let tree = iter.parse(&prefix).expect("deep prefix parse iteratively");
    assert_eq!(node_count(&tree), 50_001, "50k unary nodes + leaf");
}

#[test]
fn reused_stack_does_not_grow_across_shallow_parses() {
    // Steady-state shallow parsing must not reallocate the explicit stack.
    let mut iter = IterParser::with_capacity(64);
    let cap0 = iter.stack_capacity();
    for &(_, src) in SHALLOW_CASES {
        let toks = tokenize(src).expect("tokenizes");
        iter.parse(&toks).expect("parses");
    }
    assert_eq!(
        iter.stack_capacity(),
        cap0,
        "shallow parses stayed within reserve"
    );
}
