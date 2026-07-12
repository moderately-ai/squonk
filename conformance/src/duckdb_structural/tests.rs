// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Tests for the DuckDB SELECT-surface structural oracle.
//!
//! The whole module is gated behind `oracle-engines`, so these compile only in that
//! configuration. Any test that actually serializes SQL goes through a live in-process
//! DuckDB and therefore [`oracle_or_skip`]s when `libduckdb` cannot be opened — the
//! ADR-0015 infrastructure-skip contract. The comparator/allowlist plumbing tests use
//! mock [`StructuralOracle`]s and run unconditionally.

use super::*;
use crate::oracle::OracleUnavailable;
use crate::pg::PgStructuralOracle;
use crate::shape::{
    BinaryOperatorShape, DataTypeShape, ExprShape, FunctionShape, LimitShape, LiteralShape,
    OrderByAllShape, OrderByShape, QueryShape, SelectItemShape, SelectShape, SetShape,
    StatementShape, TableFactorShape, TableWithJoinsShape, squonk_shape_result,
};
use squonk::ast::SubscriptKind;
use squonk::dialect::Postgres;
use squonk::parse_with;

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

/// The live DuckDB structural oracle, or `None` (with a printed reason) when
/// `libduckdb` is unavailable — the differential is skipped, never failed.
fn oracle_or_skip() -> Option<DuckDbStructuralOracle> {
    match DuckDbStructuralOracle::new() {
        Ok(oracle) => Some(oracle),
        Err(OracleUnavailable(reason)) => {
            eprintln!("skipping DuckDB structural test (oracle unavailable): {reason}");
            None
        }
    }
}

/// The neutral shapes DuckDB serializes `sql` into, asserting it is inside the
/// comparable subset (panics with the skip reason otherwise — a test-authoring error,
/// since these inputs are chosen to be mappable).
fn mapped(oracle: &DuckDbStructuralOracle, sql: &str) -> Vec<StatementShape> {
    match oracle.shape(sql).expect("oracle is available") {
        StructuralShape::Mapped(shape) => shape,
        StructuralShape::OutsideSubset(reason) => {
            panic!("expected {sql:?} inside the comparable subset, got OutsideSubset: {reason}")
        }
    }
}

// Compact shape constructors, so the golden literals read close to the SQL.
fn col(parts: &[&str]) -> ExprShape {
    ExprShape::Column(parts.iter().map(|s| (*s).to_owned()).collect())
}
fn int_lit(text: &str) -> ExprShape {
    ExprShape::Literal(LiteralShape::Integer(text.to_owned()))
}
fn base_table(name: &str) -> TableFactorShape {
    TableFactorShape::Table {
        name: vec![name.to_owned()],
        alias: None,
        only: false,
        sample: None,
    }
}
fn from_one(relation: TableFactorShape) -> Vec<TableWithJoinsShape> {
    vec![TableWithJoinsShape {
        relation,
        joins: Vec::new(),
    }]
}
fn no_limit() -> LimitShape {
    LimitShape {
        count: None,
        offset: None,
        with_ties: false,
    }
}
/// A one-item, clause-free `SELECT <expr>` statement shape.
fn bare_select(expr: ExprShape) -> StatementShape {
    StatementShape::Query(QueryShape {
        with: None,
        body: SetShape::Select(SelectShape {
            distinct: false,
            projection: vec![SelectItemShape::Expr { expr, alias: None }],
            from: Vec::new(),
            selection: None,
            group_by: Vec::new(),
            group_by_distinct: false,
            group_by_all: false,
            having: None,
            qualify: None,
        }),
        order_by: Vec::new(),
        order_by_all: None,
        limit: no_limit(),
        locking: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Mock oracles (comparator/allowlist plumbing — no live engine)
// ---------------------------------------------------------------------------

/// A structural oracle that always returns a fixed shape — stands in for an engine (or
/// a hypothetical mis-parsing parser) that produced a specific tree.
struct FixedOracle(Vec<StatementShape>);
impl StructuralOracle for FixedOracle {
    fn name(&self) -> &'static str {
        "fixed"
    }
    fn shape(&self, _sql: &str) -> Result<StructuralShape, OracleUnavailable> {
        Ok(StructuralShape::Mapped(self.0.clone()))
    }
}

/// A structural oracle that is always unreachable — exercises the infrastructure-skip
/// path deterministically (our real `libduckdb` is present in this environment).
struct UnavailableOracle;
impl StructuralOracle for UnavailableOracle {
    fn name(&self) -> &'static str {
        "unavailable"
    }
    fn shape(&self, _sql: &str) -> Result<StructuralShape, OracleUnavailable> {
        Err(OracleUnavailable("libduckdb absent".to_owned()))
    }
}

/// Our own neutral shapes for `sql` under the DuckDB parse dialect — used to build a
/// mock engine tree without a live engine.
fn our_shape(sql: &str) -> Vec<StatementShape> {
    let parsed = parse_with(sql, duckdb_parse_dialect()).expect("our parser accepts the SQL");
    squonk_shape_result(&parsed).expect("our neutral shape maps")
}

// ---------------------------------------------------------------------------
// Golden shapes for the three sampled trees from the ticket body
// ---------------------------------------------------------------------------

/// Sample 1 — a fully mapped SELECT: `b + 1` folds through the operator-function
/// normalization to a `BinaryOp`, `ORDER BY a` lands in `order_by`, everything else is
/// the ordinary select body.
#[test]
fn golden_ordered_select_with_operator_function() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let shape = mapped(
        &oracle,
        "SELECT a, b + 1 AS c FROM t WHERE x > 5 ORDER BY a",
    );
    let expected = vec![StatementShape::Query(QueryShape {
        with: None,
        body: SetShape::Select(SelectShape {
            distinct: false,
            projection: vec![
                SelectItemShape::Expr {
                    expr: col(&["a"]),
                    alias: None,
                },
                SelectItemShape::Expr {
                    expr: ExprShape::BinaryOp {
                        left: Box::new(col(&["b"])),
                        op: BinaryOperatorShape::Plus,
                        right: Box::new(int_lit("1")),
                    },
                    alias: Some("c".to_owned()),
                },
            ],
            from: from_one(base_table("t")),
            selection: Some(ExprShape::BinaryOp {
                left: Box::new(col(&["x"])),
                op: BinaryOperatorShape::Gt,
                right: Box::new(int_lit("5")),
            }),
            group_by: Vec::new(),
            group_by_distinct: false,
            group_by_all: false,
            having: None,
            qualify: None,
        }),
        order_by: vec![OrderByShape {
            expr: col(&["a"]),
            asc: None,
            using: None,
            nulls_first: None,
        }],
        order_by_all: None,
        limit: no_limit(),
        locking: Vec::new(),
    })];
    assert_eq!(shape, expected);
}

/// Sample 2 — `SELECT * EXCLUDE (...)` parses on our side
/// (`duckdb-select-star-modifiers`), but the modifier lists have no neutral-shape
/// fields yet (`duckdb-structural-oracle-select` owns that parity), so a
/// modifier-bearing wildcard stays a counted skip carrying its ticket — never a
/// mis-map onto a plain wildcard. This is exactly the structural check that would
/// catch a mis-parsed `EXCLUDE`.
#[test]
fn golden_star_exclude_is_a_deferred_skip() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    match oracle.shape("SELECT * EXCLUDE (i, j) FROM integers") {
        Ok(StructuralShape::OutsideSubset(reason)) => {
            assert!(
                reason.contains("duckdb-structural-oracle-select"),
                "reason should name the shape-parity owner, got: {reason}",
            );
        }
        other => panic!("expected OutsideSubset for a star modifier, got {other:?}"),
    }
}

/// The `COLUMNS(...)` star expression maps on both sides (`duckdb-select-star-modifiers`):
/// each selector form — the regex string, the whole-projection `*`, the qualified
/// `t.*`, the lambda (whose engine-side `list_filter(<bare *>, λ)` desugaring the
/// mapping unwraps), the name list — compares structurally in projection and
/// expression position alike, instead of skipping.
#[test]
fn golden_columns_selector_compares_on_both_sides() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    for sql in [
        "SELECT COLUMNS('re') FROM t",
        "SELECT COLUMNS(*) FROM t",
        "SELECT COLUMNS(t.*) FROM t",
        "SELECT COLUMNS(c -> c LIKE 'x%') FROM t",
        "SELECT COLUMNS(['a', 'b']) FROM t",
        "SELECT sum(COLUMNS(*)) FROM t",
        "SELECT COLUMNS('a|c') + 42 FROM t",
        "SELECT * FROM t ORDER BY COLUMNS('re')",
    ] {
        assert!(
            matches!(
                structural_comparison(sql, duckdb_parse_dialect(), &oracle),
                Comparison::Match,
            ),
            "expected a structural match for {sql:?}",
        );
    }
    // The qualifier is part of the shape: a qualified star must not unify with the
    // bare one (the discriminating direction).
    assert_ne!(
        mapped(&oracle, "SELECT COLUMNS(t.*) FROM t"),
        mapped(&oracle, "SELECT COLUMNS(*) FROM t"),
    );
}

/// `ORDER BY COLUMNS(*)` and `ORDER BY ALL` serialize to the identical tree (sole
/// whole-projection star order entry; probed on 1.5.4), so both mappings lift the
/// sole bare `COLUMNS(*)` order key to the neutral order-by-all mode — the shape
/// agreement that retired this exact statement's `DUCKDB_STRUCTURAL_ALLOWLIST` entry
/// (`duckdb-select-star-modifiers`).
#[test]
fn golden_order_by_columns_star_lifts_to_the_all_mode_on_both_sides() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let sql = "SELECT * FROM grouped_table ORDER BY COLUMNS(*)";
    let engine = mapped(&oracle, sql);
    let StatementShape::Query(query) = &engine[0] else {
        panic!("expected a query shape");
    };
    assert!(
        query.order_by_all.is_some() && query.order_by.is_empty(),
        "the engine side lifts the sole star order key to the all mode",
    );
    assert!(
        matches!(
            structural_comparison(sql, duckdb_parse_dialect(), &oracle),
            Comparison::Match,
        ),
        "our parse of ORDER BY COLUMNS(*) lifts to the same mode",
    );
    // A qualified or pattern-carrying COLUMNS order key is *not* the mode — it stays
    // an ordinary order entry on both sides (the engine only serializes the bare
    // whole-projection star identically to ALL).
    let qualified = mapped(&oracle, "SELECT * FROM t ORDER BY COLUMNS(t.*)");
    let StatementShape::Query(query) = &qualified[0] else {
        panic!("expected a query shape");
    };
    assert!(
        query.order_by_all.is_none() && query.order_by.len() == 1,
        "a qualified star order key is an ordinary entry",
    );
}

/// Sample 3 — `[1, 2, 3]` desugars to `list_value(...)`, which the mapping normalizes
/// back to the real `Array` shape (`duckdb-collection-literals` upgraded it from the
/// former `Unmapped` gap).
#[test]
fn golden_list_literal_maps_to_array_shape() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let shape = mapped(&oracle, "SELECT [1, 2, 3]");
    let expected = vec![bare_select(ExprShape::Array(vec![
        int_lit("1"),
        int_lit("2"),
        int_lit("3"),
    ]))];
    assert_eq!(shape, expected);
}

/// `{'a': 1, 'b': 2}` desugars to `struct_pack(…)` with each written key preserved in
/// its child's `alias`; the mapping normalizes it back to the `Struct` shape with the
/// keys' exact text.
#[test]
fn golden_struct_literal_maps_to_struct_shape() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let shape = mapped(&oracle, "SELECT {'a': 1, 'B': 2}");
    let expected = vec![bare_select(ExprShape::Struct(vec![
        ("a".to_owned(), int_lit("1")),
        ("B".to_owned(), int_lit("2")),
    ]))];
    assert_eq!(shape, expected);
}

/// `a[1]` / `a[1:3]` / `a[2:]` / `a[1:3:2]` / `a[1:-:2]` serialize as
/// `ARRAY_EXTRACT`/`ARRAY_SLICE` operator nodes (an omitted bound — including the stepped
/// `-` open-upper placeholder — as the empty-LIST constant sentinel; the stepped slice a
/// four-child `ARRAY_SLICE`); each normalizes to the `Subscript` shape.
#[test]
fn golden_subscript_and_slice_map_to_subscript_shape() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let subscript = |lower: Option<ExprShape>,
                     upper: Option<ExprShape>,
                     step: Option<ExprShape>,
                     kind: SubscriptKind| {
        vec![StatementShape::Query(QueryShape {
            with: None,
            body: SetShape::Select(SelectShape {
                distinct: false,
                projection: vec![SelectItemShape::Expr {
                    expr: ExprShape::Subscript {
                        base: Box::new(col(&["a"])),
                        lower: lower.map(Box::new),
                        upper: upper.map(Box::new),
                        step: step.map(Box::new),
                        kind,
                    },
                    alias: None,
                }],
                from: from_one(base_table("t")),
                selection: None,
                group_by: Vec::new(),
                group_by_distinct: false,
                group_by_all: false,
                having: None,
                qualify: None,
            }),
            order_by: Vec::new(),
            order_by_all: None,
            limit: no_limit(),
            locking: Vec::new(),
        })]
    };
    assert_eq!(
        mapped(&oracle, "SELECT a[1] FROM t"),
        subscript(Some(int_lit("1")), None, None, SubscriptKind::Index),
    );
    assert_eq!(
        mapped(&oracle, "SELECT a[1:3] FROM t"),
        subscript(
            Some(int_lit("1")),
            Some(int_lit("3")),
            None,
            SubscriptKind::Slice
        ),
    );
    assert_eq!(
        mapped(&oracle, "SELECT a[2:] FROM t"),
        subscript(Some(int_lit("2")), None, None, SubscriptKind::Slice),
    );
    // The three-bound stepped slice (four-child `ARRAY_SLICE`).
    assert_eq!(
        mapped(&oracle, "SELECT a[1:3:2] FROM t"),
        subscript(
            Some(int_lit("1")),
            Some(int_lit("3")),
            Some(int_lit("2")),
            SubscriptKind::SliceWithStep,
        ),
    );
    // The `-` open-upper placeholder serializes its upper as the omitted-bound sentinel.
    assert_eq!(
        mapped(&oracle, "SELECT a[1:-:2] FROM t"),
        subscript(
            Some(int_lit("1")),
            None,
            Some(int_lit("2")),
            SubscriptKind::SliceWithStep,
        ),
    );
}

/// `MAP {'a': 1}` serializes as an ordinary `map(list_value('a'), list_value(1))`
/// call — the exact desugaring our side's map shape emits — so the generic function
/// path matches it without a dedicated arm.
#[test]
fn golden_map_literal_maps_to_the_map_call_desugaring() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let shape = mapped(&oracle, "SELECT MAP {'a': 1}");
    let expected = vec![bare_select(ExprShape::Function(FunctionShape {
        name: vec!["map".to_owned()],
        args: vec![
            ExprShape::Array(vec![ExprShape::Literal(LiteralShape::String(
                "a".to_owned(),
            ))]),
            ExprShape::Array(vec![int_lit("1")]),
        ],
        wildcard: false,
    }))];
    assert_eq!(shape, expected);
}

/// The sweep lambda: DuckDB's `LAMBDA` class maps onto [`ExprShape::Lambda`] when its
/// left side is a parameter name list — the same split our parser applies
/// (`duckdb-lambda-expressions`) — while a non-parameter left side (a qualified name)
/// normalizes to the `JsonGet` binary op our fall-through parse produces.
#[test]
fn golden_lambda_maps_by_the_parameter_shape_split() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let shape = mapped(&oracle, "SELECT list_transform([1, 2, 3], x -> x + 1)");
    let expected = vec![bare_select(ExprShape::Function(FunctionShape {
        name: vec!["list_transform".to_owned()],
        args: vec![
            ExprShape::Array(vec![int_lit("1"), int_lit("2"), int_lit("3")]),
            ExprShape::Lambda {
                params: vec!["x".to_owned()],
                body: Box::new(ExprShape::BinaryOp {
                    left: Box::new(col(&["x"])),
                    op: BinaryOperatorShape::Plus,
                    right: Box::new(int_lit("1")),
                }),
            },
        ],
        wildcard: false,
    }))];
    assert_eq!(shape, expected);

    // The JSON half of the split: a qualified left side is no parameter list, so the
    // same serialized class lands on the `JsonGet` fold instead.
    let shape = mapped(&oracle, "SELECT t.a -> 'k' FROM t");
    let StatementShape::Query(query) = &shape[0] else {
        panic!("expected a query shape");
    };
    let SetShape::Select(select) = &query.body else {
        panic!("expected a select body");
    };
    let SelectItemShape::Expr { expr, .. } = &select.projection[0] else {
        panic!("expected an expression item");
    };
    assert_eq!(
        *expr,
        ExprShape::BinaryOp {
            left: Box::new(col(&["t", "a"])),
            op: BinaryOperatorShape::JsonGet,
            right: Box::new(ExprShape::Literal(LiteralShape::String("k".to_owned()))),
        },
    );
}

/// `GROUP BY ALL` serializes as `aggregate_handling: FORCE_AGGREGATES` with an
/// empty key list (probed on 1.5.4); the mapping lifts it to the neutral
/// `group_by_all` mode rather than skipping on the non-standard handling.
#[test]
fn golden_group_by_all_maps_to_the_clause_mode() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let shape = mapped(&oracle, "SELECT i FROM t GROUP BY ALL");
    let expected = vec![StatementShape::Query(QueryShape {
        with: None,
        body: SetShape::Select(SelectShape {
            distinct: false,
            projection: vec![SelectItemShape::Expr {
                expr: col(&["i"]),
                alias: None,
            }],
            from: from_one(base_table("t")),
            selection: None,
            group_by: Vec::new(),
            group_by_distinct: false,
            group_by_all: true,
            having: None,
            qualify: None,
        }),
        order_by: Vec::new(),
        order_by_all: None,
        limit: no_limit(),
        locking: Vec::new(),
    })];
    assert_eq!(shape, expected);
}

/// `ORDER BY ALL DESC NULLS LAST` serializes as a single order entry whose
/// expression is the whole-projection `COLUMNS(*)` star with the direction and
/// nulls on the order node (probed on 1.5.4); the mapping lifts the sole
/// star-entry to the neutral `order_by_all` mode with its modifiers.
#[test]
fn golden_order_by_all_maps_to_the_clause_mode() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let shape = mapped(&oracle, "SELECT * FROM t ORDER BY ALL DESC NULLS LAST");
    let expected = vec![StatementShape::Query(QueryShape {
        with: None,
        body: SetShape::Select(SelectShape {
            distinct: false,
            projection: vec![SelectItemShape::Wildcard],
            from: from_one(base_table("t")),
            selection: None,
            group_by: Vec::new(),
            group_by_distinct: false,
            group_by_all: false,
            having: None,
            qualify: None,
        }),
        order_by: Vec::new(),
        order_by_all: Some(OrderByAllShape {
            asc: Some(false),
            nulls_first: Some(false),
        }),
        limit: no_limit(),
        locking: Vec::new(),
    })];
    assert_eq!(shape, expected);
}

/// FROM-first parity — the engine evidence the canonicalize-with-a-spelling-tag decision
/// rests on (`duckdb-from-first-select`). DuckDB serializes `FROM t SELECT a` to the
/// identical `SELECT_NODE` as `SELECT a FROM t` (only the source `query_location` differs,
/// which the mapping drops), and the bare `FROM t` to the same tree as `SELECT * FROM t`
/// (DuckDB materializes the implicit `SELECT *`), so all four map to the two neutral
/// shapes below — one meaning, the surface order carried only by our parser's tag.
#[test]
fn golden_from_first_serializes_like_select_first() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let projected = vec![StatementShape::Query(QueryShape {
        with: None,
        body: SetShape::Select(SelectShape {
            distinct: false,
            projection: vec![SelectItemShape::Expr {
                expr: col(&["a"]),
                alias: None,
            }],
            from: from_one(base_table("t")),
            selection: None,
            group_by: Vec::new(),
            group_by_distinct: false,
            group_by_all: false,
            having: None,
            qualify: None,
        }),
        order_by: Vec::new(),
        order_by_all: None,
        limit: no_limit(),
        locking: Vec::new(),
    })];
    assert_eq!(mapped(&oracle, "FROM t SELECT a"), projected);
    assert_eq!(mapped(&oracle, "SELECT a FROM t"), projected);

    let star = vec![StatementShape::Query(QueryShape {
        with: None,
        body: SetShape::Select(SelectShape {
            distinct: false,
            projection: vec![SelectItemShape::Wildcard],
            from: from_one(base_table("t")),
            selection: None,
            group_by: Vec::new(),
            group_by_distinct: false,
            group_by_all: false,
            having: None,
            qualify: None,
        }),
        order_by: Vec::new(),
        order_by_all: None,
        limit: no_limit(),
        locking: Vec::new(),
    })];
    assert_eq!(mapped(&oracle, "FROM t"), star);
    assert_eq!(mapped(&oracle, "SELECT * FROM t"), star);
}

/// Our own parse maps both FROM-first spellings to the *same* neutral shape as their
/// SELECT-first equivalents — the neutral `SelectShape` carries no spelling tag, so the
/// canonicalize-with-a-tag design collapses them here with no live oracle needed.
#[test]
fn from_first_and_select_first_share_our_neutral_shape() {
    assert_eq!(our_shape("FROM t SELECT a"), our_shape("SELECT a FROM t"));
    assert_eq!(our_shape("FROM t"), our_shape("SELECT * FROM t"));
    assert_eq!(
        our_shape("FROM t SELECT a WHERE a > 1"),
        our_shape("SELECT a FROM t WHERE a > 1"),
    );
    assert_eq!(
        our_shape("FROM a SELECT x UNION FROM b SELECT y"),
        our_shape("SELECT x FROM a UNION SELECT y FROM b"),
    );
}

// ---------------------------------------------------------------------------
// The mapping agrees with our parser across the core SELECT surface
// ---------------------------------------------------------------------------

/// The crux: over a spread of both-accept SELECT constructs, our parse and DuckDB's
/// `json_serialize_sql` tree map to the *same* neutral shape — every category-1
/// normalization (operator-functions, `count(*)`, boolean/decimal literals, `VALUES`
/// desugaring, IN-subquery, n-ary conjunction, casts) absorbed, no residual.
#[test]
fn mapping_agrees_with_our_parser_on_core_selects() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    for sql in [
        "SELECT a, b + 1 AS c FROM t WHERE x > 5 ORDER BY a",
        "SELECT 1.5",
        "SELECT -1.5",
        "SELECT count(*) FROM t",
        "SELECT TRUE",
        "SELECT FALSE",
        "SELECT NULL",
        "SELECT 'hello'",
        "SELECT a FROM t GROUP BY a",
        "SELECT DISTINCT a FROM t",
        "SELECT a FROM t LIMIT 5 OFFSET 2",
        "SELECT a FROM t ORDER BY a DESC NULLS FIRST",
        "SELECT * FROM a JOIN b ON a.i = b.i",
        "SELECT * FROM a LEFT JOIN b ON a.i = b.i",
        "SELECT 1 UNION ALL SELECT 2",
        "SELECT 1 UNION SELECT 2",
        "SELECT 1 INTERSECT SELECT 2",
        "SELECT 1 EXCEPT SELECT 2",
        "WITH x AS (SELECT 1 AS a) SELECT * FROM x",
        "SELECT CAST(x AS INTEGER) FROM t",
        "SELECT x FROM t WHERE x IS NULL",
        "SELECT x FROM t WHERE x IS NOT NULL",
        "SELECT x FROM t WHERE x IN (SELECT y FROM u)",
        "SELECT x FROM t WHERE x NOT IN (SELECT y FROM u)",
        "SELECT x FROM t WHERE EXISTS (SELECT 1)",
        "SELECT (SELECT 1) AS s",
        "SELECT CASE WHEN x > 1 THEN 1 ELSE 2 END FROM t",
        "SELECT * FROM (VALUES (1, 2), (3, 4)) v",
        "SELECT -x FROM t",
        "SELECT x FROM t WHERE a AND b AND c",
        "SELECT x FROM t WHERE a OR b",
        "SELECT * FROM (SELECT 1 AS a) sub",
        // QUALIFY (duckdb-qualify-clause): the clause slot and its comparison
        // structure agree; the window call inside is the shared `Unmapped` gap.
        "SELECT a FROM t QUALIFY row_number() OVER () = 1",
        "SELECT a FROM t GROUP BY a HAVING count(*) > 1 QUALIFY row_number() OVER () = 1",
        // Collection literals + subscript/slicing (duckdb-collection-literals): the
        // list/struct/subscript desugar-normalizations and the map-call desugaring
        // agree with our parse, including the ticket's nested sweep example and both
        // key spellings a struct admits.
        "SELECT [1, 2, 3]",
        "SELECT []",
        "SELECT ['a', 'b']",
        "SELECT {'a': 1, 'b': 2}",
        "SELECT {a: 1}",
        "SELECT [{'a': 42}, {'b': 84}]",
        "SELECT {'a': 2.0, 'b': 'hello', 'c': [1, 2]}",
        "SELECT MAP {'a': 1, 'b': 2}",
        "SELECT MAP {}",
        "SELECT map(['a'], [1])",
        "SELECT MAP([1], [2])",
        "SELECT a[1] FROM t",
        "SELECT a[-1] FROM t",
        "SELECT a[1:3] FROM t",
        "SELECT a[2:] FROM t",
        "SELECT a[:3] FROM t",
        "SELECT a[:] FROM t",
        "SELECT [1, 2, 3][2]",
        "SELECT ARRAY[1, 2]",
        // Nonstandard joins (duckdb-nonstandard-joins): every ASOF side (the INNER
        // spelling normalizes onto the bare form on both sides), the USING form, the
        // constraint-less POSITIONAL pairing, and the sweep's derived-table example.
        "SELECT * FROM a ASOF JOIN b ON a.i >= b.i",
        "SELECT * FROM a ASOF INNER JOIN b ON a.i >= b.i",
        "SELECT * FROM a ASOF LEFT JOIN b ON a.i >= b.i",
        "SELECT * FROM a ASOF LEFT OUTER JOIN b ON a.i >= b.i",
        "SELECT * FROM a ASOF RIGHT JOIN b ON a.i >= b.i",
        "SELECT * FROM a ASOF FULL JOIN b ON a.i >= b.i",
        "SELECT * FROM a ASOF JOIN b USING (i)",
        "SELECT a.x FROM a POSITIONAL JOIN b",
        "SELECT * FROM a ASOF LEFT JOIN (SELECT i FROM u) b ON a.i >= b.i",
        // SEMI / ANTI joins (duckdb-semi-anti-join): the bare join_type (never a side)
        // with ON and USING, the NATURAL match (no ON/USING), and the ASOF composition —
        // every ref-type that pairs with SEMI/ANTI on the engine.
        "SELECT * FROM a SEMI JOIN b ON a.i = b.i",
        "SELECT * FROM a ANTI JOIN b ON a.i = b.i",
        "SELECT * FROM a SEMI JOIN b USING (i)",
        "SELECT * FROM a NATURAL SEMI JOIN b",
        "SELECT * FROM a NATURAL ANTI JOIN b",
        "SELECT * FROM a ASOF SEMI JOIN b ON a.i >= b.i",
        "SELECT * FROM a ASOF ANTI JOIN b ON a.i >= b.i",
        "SELECT * FROM a ASOF SEMI JOIN b USING (i)",
        // Lambdas + the `->` split (duckdb-lambda-expressions): every parameter
        // spelling, the DuckDB-measured loose precedence (the comparison lives in
        // the body), the JSON fall-throughs (qualified / constant / chained left
        // sides), and the sweep's list-function shapes.
        "SELECT list_transform([1, 2, 3], x -> x + 1)",
        "SELECT list_filter([1, 2, 3, 4], x -> x % 2 = 0)",
        "SELECT list_reduce([1, 2, 3], (x, y) -> x + y)",
        "SELECT list_transform([1], (x) -> x + 1)",
        "SELECT list_reduce([1, 2, 3], ROW(x, y) -> x + y)",
        "SELECT x -> x + 1",
        "SELECT x -> x OR y",
        "SELECT t.a -> 'k' FROM t",
        "SELECT 1 -> 2",
        "SELECT x -> y -> z",
        "SELECT list_transform(l, x -> list_transform(x, y -> y + 1)) FROM t",
        // GROUP BY ALL / ORDER BY ALL (duckdb-group-order-by-all): the clause modes
        // and their modifier/LIMIT/set-operation compositions agree with our parse.
        "SELECT i FROM t GROUP BY ALL",
        "SELECT i, count(*) FROM t GROUP BY ALL HAVING count(*) > 1",
        "SELECT * FROM t ORDER BY ALL",
        "SELECT * FROM t ORDER BY ALL ASC",
        "SELECT * FROM t ORDER BY ALL DESC NULLS LAST",
        "SELECT i FROM t GROUP BY ALL ORDER BY ALL LIMIT 3",
        "SELECT i FROM t UNION ALL SELECT j FROM t ORDER BY ALL",
        // FROM-first SELECT (duckdb-from-first-select): DuckDB serializes the FROM-first
        // order — and the SELECT-less bare `FROM t` — to the same SELECT tree as the
        // SELECT-first spelling, so our canonicalize-with-a-tag parse maps to the identical
        // neutral shape, including the composed tail, query-level clauses, set-op, and CTE
        // forms.
        "FROM t SELECT a",
        "FROM t",
        "FROM t SELECT a WHERE a > 1",
        "FROM t SELECT i, count(*) GROUP BY i HAVING count(*) > 1",
        "FROM t GROUP BY ALL",
        "FROM t ORDER BY a",
        "FROM t ORDER BY ALL LIMIT 3",
        "FROM a SELECT x UNION FROM b SELECT y",
        "WITH c AS (FROM t SELECT a) FROM c",
        // PIVOT/UNPIVOT table factors (duckdb-pivot-unpivot): the engine serializes
        // a first-class `PIVOT` from_table node for the FROM-suffix spelling, so
        // these compare as real tree parity, not a desugar match — aggregates with
        // aliases (incl. the count_star fold), IN-value aliases, GROUP BY inside the
        // parens, both NULLS markers, the multi-column unpivot, a derived source,
        // chaining, and the trailing alias. The *statement* spelling stays outside
        // the bound (the serializer refuses it; see the non-SELECT test).
        "SELECT * FROM t PIVOT (sum(x) FOR y IN (1, 2))",
        "SELECT * FROM t PIVOT (sum(x) AS total, count(*) FOR y IN (1 AS one, 2)) AS p",
        "SELECT * FROM t PIVOT (sum(x) FOR y IN ('a') GROUP BY z)",
        "SELECT * FROM (SELECT a, b FROM t) PIVOT (sum(a) FOR b IN (1))",
        "SELECT * FROM t UNPIVOT (v FOR n IN (a, b))",
        "SELECT * FROM t UNPIVOT INCLUDE NULLS (v FOR n IN (a, b))",
        "SELECT * FROM t UNPIVOT ((v1, v2) FOR n IN ((a, b), (c, d)))",
        "SELECT * FROM t PIVOT (sum(x) FOR y IN (1)) UNPIVOT (v FOR n IN (a)) AS u",
    ] {
        assert_eq!(
            duckdb_structural_divergence(&oracle, sql),
            None,
            "unexpected structural divergence for {sql:?}",
        );
    }
}

// ---------------------------------------------------------------------------
// The differential catches a mis-parse
// ---------------------------------------------------------------------------

/// A mis-parse that changes tree shape (here: a dropped projection item, the signature
/// of a mis-bound select list) must surface as a `Divergence`, never a silent `Match`.
/// Engine-free: the mock engine reports the shape of a *different* statement.
#[test]
fn differential_reports_a_shape_divergence() {
    let faithful = our_shape("SELECT a, b FROM t");
    let mis_parsed = our_shape("SELECT a FROM t");
    assert_ne!(
        faithful, mis_parsed,
        "the two statements must have distinct shapes for this to be a real test",
    );

    // Our parse of the two-column select vs an engine that reports one column -> caught.
    assert!(
        matches!(
            structural_comparison(
                "SELECT a, b FROM t",
                duckdb_parse_dialect(),
                &FixedOracle(mis_parsed)
            ),
            Comparison::Divergence(_),
        ),
        "a differing engine tree must be reported as a divergence",
    );
    // Faithful engine tree -> Match (no false positive).
    assert!(
        matches!(
            structural_comparison(
                "SELECT a, b FROM t",
                duckdb_parse_dialect(),
                &FixedOracle(faithful)
            ),
            Comparison::Match,
        ),
        "an identical engine tree must match",
    );
}

/// The real mapping is discriminating in the dimension a mis-parse corrupts: a
/// one-operator difference (`>` vs `<`) yields different neutral shapes, so the live
/// differential would flag it.
#[test]
fn real_mapping_discriminates_a_flipped_operator() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let gt = mapped(&oracle, "SELECT a FROM t WHERE x > 5");
    let lt = mapped(&oracle, "SELECT a FROM t WHERE x < 5");
    assert_ne!(
        gt, lt,
        "a flipped comparison operator must change the shape"
    );
}

// ---------------------------------------------------------------------------
// The SELECT-only bound: a non-SELECT is a skip, never a divergence
// ---------------------------------------------------------------------------

/// Every [`DUCKDB_STRUCTURAL_ALLOWLIST`] entry still genuinely diverges — the
/// staleness contract the entry type documents: a silently-fixed residual (a mapping
/// improvement, a grammar landing) must force its entry's removal rather than rot in
/// the list.
#[test]
fn structural_allowlist_entries_still_diverge() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    for entry in DUCKDB_STRUCTURAL_ALLOWLIST {
        assert!(
            matches!(
                structural_comparison(entry.sql, duckdb_parse_dialect(), &oracle),
                Comparison::Divergence(_),
            ),
            "stale structural allowlist entry {:?} ({}): no longer a divergence; remove it",
            entry.sql,
            entry.ticket,
        );
    }
}

#[test]
fn non_select_is_outside_the_bound_not_a_divergence() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    // json_serialize_sql refuses every non-SELECT in-band via the error flag.
    match oracle.shape("CREATE TABLE t (a INTEGER)") {
        Ok(StructuralShape::OutsideSubset(reason)) => assert!(
            reason.to_ascii_lowercase().contains("select"),
            "the reason should quote DuckDB's SELECT-only message, got: {reason}",
        ),
        other => panic!("expected OutsideSubset for a CREATE, got {other:?}"),
    }
    // A CREATE is never a structural divergence.
    assert!(duckdb_structural_divergence(&oracle, "CREATE TABLE t (a INTEGER)").is_none());
    // The leading-keyword PIVOT statement is the same bound: DuckDB models it as a
    // PivotStatement the serializer refuses, and our side maps `Statement::Pivot` to
    // the not-implemented statement skip — accept-parity + round-trip only, the measured
    // bound for the PIVOT statement (`duckdb-pivot-unpivot`).
    let pivot_statement = "PIVOT t ON y USING sum(x) GROUP BY z";
    assert!(
        matches!(
            structural_comparison(pivot_statement, duckdb_parse_dialect(), &oracle),
            Comparison::EngineOutside(_) | Comparison::OursOutside(_),
        ),
        "a pivot statement both sides accept must be a skip, not a divergence",
    );
    assert!(duckdb_structural_divergence(&oracle, pivot_statement).is_none());

    // The meaningful skip: a statement OUR parser accepts but DuckDB will not serialize
    // (INSERT) resolves to EngineOutside, not a false divergence.
    let insert = "INSERT INTO t VALUES (1)";
    assert!(
        matches!(
            structural_comparison(insert, duckdb_parse_dialect(), &oracle),
            Comparison::EngineOutside(_) | Comparison::OursReject | Comparison::OursOutside(_),
        ),
        "a non-SELECT our side accepts must be a skip, not a divergence",
    );
    assert!(duckdb_structural_divergence(&oracle, insert).is_none());
}

// ---------------------------------------------------------------------------
// Infrastructure-skip and seam-generality
// ---------------------------------------------------------------------------

/// An unreachable engine is a skip (`Unavailable`), never a divergence — the ADR-0015
/// contract, exercised deterministically with a mock (real `libduckdb` is present here).
#[test]
fn unavailable_engine_is_skipped() {
    assert!(matches!(
        structural_comparison("SELECT 1", duckdb_parse_dialect(), &UnavailableOracle),
        Comparison::Unavailable(_),
    ));
}

/// The `StructuralOracle` seam is general: the PostgreSQL oracle (wrapping `pg_shape`)
/// runs through the *same* comparator and matches our Postgres parse — proving the
/// abstraction is not DuckDB-only (the second-source justification from the ticket).
#[test]
fn pg_structural_oracle_drives_the_same_comparator() {
    assert!(
        matches!(
            structural_comparison(
                "SELECT a, b FROM t WHERE x > 5",
                Postgres,
                &PgStructuralOracle
            ),
            Comparison::Match,
        ),
        "the PG oracle must match our parse through the shared comparator",
    );
    // And its own SELECT-only-irrelevant bound: PG serializes DDL too, but a construct
    // our neutral model does not cover is still a clean skip.
    assert!(matches!(
        structural_comparison("SELECT 1", Postgres, &PgStructuralOracle),
        Comparison::Match,
    ));
}

// ---------------------------------------------------------------------------
// decimal_text reconstruction (fiddly scaled-integer padding)
// ---------------------------------------------------------------------------

#[test]
fn decimal_text_reconstructs_scaled_integers() {
    assert_eq!(decimal_text(15, 1), "1.5");
    assert_eq!(decimal_text(5, 2), "0.05");
    assert_eq!(decimal_text(-15, 1), "-1.5");
    assert_eq!(decimal_text(1234, 2), "12.34");
    assert_eq!(decimal_text(100, 0), "100");
    assert_eq!(decimal_text(0, 2), "0.00");
}

// ---------------------------------------------------------------------------
// The corpus differential — the gate
// ---------------------------------------------------------------------------

/// Run the structural differential over the both-accept SELECT subset of the vendored
/// DuckDB corpus and assert no untriaged divergence. The comparable subset is narrow by
/// design (the corpus is signature-weighted DuckDB syntax Ansi mostly rejects, and
/// `json_serialize_sql` covers only SELECT), so most statements are counted skips; the
/// gate is that whatever *does* compare agrees, modulo the ticketed
/// [`DUCKDB_STRUCTURAL_ALLOWLIST`].
///
/// The corpus fixtures are owned/pinned by [`corpus_duckdb_verdicts`](crate::corpus_duckdb_verdicts);
/// this test only reads them, without re-pinning their (sibling-evolving) line count.
#[test]
fn structural_differential_over_the_duckdb_corpus() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    const STATEMENTS: &str = include_str!("../../corpus/duckdb/statements.sql");
    const DOCS: &str = include_str!("../../corpus/duckdb/docs_examples.sql");

    let mut compared = 0usize;
    let mut matched = 0usize;
    let mut ours_reject = 0usize;
    let mut ours_outside = 0usize;
    let mut engine_outside = 0usize;
    let mut allowlisted = 0usize;
    let mut untriaged: Vec<String> = Vec::new();

    for sql in STATEMENTS
        .lines()
        .chain(DOCS.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        match structural_comparison(sql, duckdb_parse_dialect(), &oracle) {
            Comparison::Match => {
                compared += 1;
                matched += 1;
            }
            Comparison::Divergence(detail) => {
                compared += 1;
                if duckdb_structural_allowlisted(sql) {
                    allowlisted += 1;
                } else {
                    untriaged.push(format!("  SQL: {sql}\n  {detail}"));
                }
            }
            Comparison::OursReject => ours_reject += 1,
            Comparison::OursOutside(_) => ours_outside += 1,
            Comparison::EngineOutside(_) => engine_outside += 1,
            // The oracle was constructed, so a per-statement Unavailable would be a bug.
            Comparison::Unavailable(reason) => {
                panic!("oracle became unavailable mid-sweep for {sql:?}: {reason}")
            }
        }
    }

    eprintln!("\n=== DuckDB structural differential (DuckDb vs json_serialize_sql) ===");
    eprintln!("  compared (both mapped)    {compared}  (match {matched})");
    eprintln!("  allowlisted divergences   {allowlisted}");
    eprintln!("  skip: our parser rejects  {ours_reject}");
    eprintln!("  skip: our model uncovered {ours_outside}");
    eprintln!("  skip: engine outside SEL  {engine_outside}");

    assert!(
        untriaged.is_empty(),
        "untriaged DuckDB structural divergences (fix the mapping if category-1, else \
         add a DUCKDB_STRUCTURAL_ALLOWLIST entry with its ticket):\n{}",
        untriaged.join("\n---\n"),
    );
    assert!(
        compared > 0,
        "the differential compared zero statements — the comparability predicate or \
         corpus wiring is broken",
    );
}

/// Every allowlisted structural residual names a real ticket and still diverges —
/// the ledger-staleness contract [`DuckDbStructuralAllowlistEntry`] documents, so a
/// silently fixed residual forces its entry's removal.
#[test]
fn structural_allowlist_entries_are_live_and_ticketed() {
    for entry in DUCKDB_STRUCTURAL_ALLOWLIST {
        assert!(
            !entry.ticket.trim().is_empty(),
            "structural allowlist entry {:?} needs a provenance label ({})",
            entry.sql,
            entry.ticket,
        );
        assert!(
            !entry.reason.trim().is_empty(),
            "structural allowlist entry {:?} needs a reason",
            entry.sql,
        );
    }
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    for entry in DUCKDB_STRUCTURAL_ALLOWLIST {
        assert!(
            duckdb_structural_divergence(&oracle, entry.sql).is_some(),
            "stale structural allowlist entry {:?}: no longer diverges; remove it",
            entry.sql,
        );
    }
}

/// A `CAST(x AS UUID)` target is the same single-keyword scalar on both sides: DuckDB
/// serializes the cast type as the `UUID` id (probed on 1.5.4), which
/// [`duckdb_data_type_shape`] maps to the canonical `["uuid"]` name that
/// [`squonk_data_type_shape`](crate::shape) also emits for the first-class
/// `DataType::Uuid` — so the neutral shapes unify and the statement is a verified pass,
/// not a `duckdb_data_type_shape` `unsupported cast target` skip. The vendored corpus
/// carries no UUID cast, so this golden is the sole exercise of the promotion.
#[test]
fn golden_uuid_cast_compares_on_both_sides() {
    let Some(oracle) = oracle_or_skip() else {
        return;
    };
    let uuid_cast = |inner: ExprShape| {
        bare_select(ExprShape::Cast {
            expr: Box::new(inner),
            data_type: DataTypeShape::Named {
                name: vec!["uuid".to_owned()],
                modifiers: Vec::new(),
                array_depth: 0,
            },
        })
    };
    // The engine side maps to the same `["uuid"]` cast shape our parser produces, for the
    // `CAST(… AS UUID)` and `::UUID` spellings alike.
    assert_eq!(
        mapped(
            &oracle,
            "SELECT CAST('00000000-0000-0000-0000-000000000000' AS UUID)",
        ),
        vec![uuid_cast(ExprShape::Literal(LiteralShape::String(
            "00000000-0000-0000-0000-000000000000".to_owned(),
        )))],
    );
    for sql in [
        "SELECT CAST('00000000-0000-0000-0000-000000000000' AS UUID)",
        "SELECT '00000000-0000-0000-0000-000000000000'::UUID",
        "SELECT CAST(x AS UUID) FROM t",
    ] {
        assert!(
            matches!(
                structural_comparison(sql, duckdb_parse_dialect(), &oracle),
                Comparison::Match,
            ),
            "expected a structural match for {sql:?}",
        );
    }
}
