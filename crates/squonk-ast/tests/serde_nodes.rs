// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Node-level serde round-trip and the deserialization recursion-depth guard
//! (`serde` feature). The whole-tree parse -> serialize -> deserialize -> render
//! round-trip lives in the `squonk` crate, which owns the resolver.
#![cfg(feature = "serde")]

use squonk_ast::serde_depth::{DEFAULT_DESERIALIZE_DEPTH, from_deserializer};
use squonk_ast::{
    BinaryOperator, Expr, Ident, Literal, LiteralKind, Meta, NodeId, QuoteStyle, Span, Symbol,
    UnaryOperator,
};

fn meta(start: u32, end: u32, id: u32) -> Meta {
    Meta::new(Span::new(start, end), NodeId::new(id).expect("non-zero id"))
}

fn int_lit(id: u32) -> Expr {
    Expr::Literal {
        literal: Literal {
            kind: LiteralKind::Integer,
            meta: meta(0, 1, id),
        },
        meta: meta(0, 1, id),
    }
}

/// `Not (Not (… lit …))` nested `n` deep — a single-child recursion spine.
fn deep_expr(n: usize) -> Expr {
    let mut expr = int_lit(1);
    for _ in 0..n {
        expr = Expr::UnaryOp {
            op: UnaryOperator::Not,
            expr: Box::new(expr),
            meta: meta(0, 1, 2),
        };
    }
    expr
}

#[test]
fn expr_round_trips_structurally() {
    // A small mixed tree: BinaryOp over a Literal and a UnaryOp(Literal).
    let expr: Expr = Expr::BinaryOp {
        left: Box::new(int_lit(1)),
        op: BinaryOperator::Plus,
        right: Box::new(Expr::UnaryOp {
            op: UnaryOperator::Minus,
            expr: Box::new(int_lit(2)),
            meta: meta(4, 5, 3),
        }),
        meta: meta(0, 5, 4),
    };

    let json = serde_json::to_string(&expr).expect("serialize");
    let back: Expr = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(expr, back);
}

#[test]
fn ident_symbol_serializes_as_its_numeric_key() {
    // A bare node has no resolver, so `Symbol` serializes as its interner key (the
    // documented same-parse form). It round-trips exactly.
    let ident = Ident {
        sym: Symbol::new(42).expect("non-zero symbol"),
        quote: QuoteStyle::Double,
        meta: meta(0, 3, 1),
    };
    let json = serde_json::to_string(&ident).expect("serialize");
    assert!(
        json.contains("42"),
        "symbol serializes as its numeric key: {json}"
    );
    let back: Ident = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ident, back);
}

#[test]
fn span_synthetic_sentinel_round_trips() {
    // The derive rebuilds `Span` field-by-field, bypassing `new`'s `start <= end`
    // assert, so the synthetic sentinel survives instead of panicking.
    for span in [Span::new(3, 9), Span::new(7, 7), Span::SYNTHETIC] {
        let json = serde_json::to_string(&span).expect("serialize");
        let back: Span = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(span, back);
    }
    let synthetic: Span =
        serde_json::from_str(&serde_json::to_string(&Span::SYNTHETIC).unwrap()).unwrap();
    assert!(synthetic.is_synthetic());
}

#[test]
fn shallow_tree_round_trips_through_the_depth_guard() {
    let expr = deep_expr(5);
    let json = serde_json::to_string(&expr).expect("serialize");
    let mut de = serde_json::Deserializer::from_str(&json);
    let back: Expr = from_deserializer(&mut de, DEFAULT_DESERIALIZE_DEPTH).expect("within budget");
    assert_eq!(expr, back);
}

#[test]
fn depth_guard_rejects_a_tree_deeper_than_the_budget() {
    // 150 nested levels: past serde_json's own nesting limit AND our default cap.
    let json = serde_json::to_string(&deep_expr(150)).expect("serialize");

    // serde_json's built-in guard rejects the deep JSON on the ordinary path.
    assert!(
        serde_json::from_str::<Expr>(&json).is_err(),
        "serde_json's own nesting limit rejects the deep tree"
    );

    // Our format-agnostic guard rejects it too, isolated from serde_json's limit: a
    // budget below the depth fails cleanly (no panic, no stack overflow) because the
    // guard errors AT the cap without descending further — so the same holds for
    // arbitrarily deeper input.
    let mut de = serde_json::Deserializer::from_str(&json);
    de.disable_recursion_limit();
    let rejected: Result<Expr, _> = from_deserializer(&mut de, 64);
    assert!(rejected.is_err(), "budget 64 < depth 150 must be rejected");

    // The very same input deserializes under a generous budget, proving it is the
    // depth cap doing the rejecting, not malformed input.
    let mut de = serde_json::Deserializer::from_str(&json);
    de.disable_recursion_limit();
    let accepted: Result<Expr, _> = from_deserializer(&mut de, 100_000);
    assert!(accepted.is_ok(), "a generous budget admits the same tree");
}
