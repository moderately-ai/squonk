// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Proptest strategies, the `pub(crate)` AST-builder toolkit, and `render_generated`.

use proptest::prelude::*;
use squonk_ast::render::{RenderConfig, RenderCtx, RenderExt as _, RenderMode};
use squonk_ast::{
    AliasSpelling, ArgSyntax, BinaryOperator, BooleanTypeName, CaseExpr, CastSyntax,
    ColumnConstraint, ColumnDef, ColumnOption, CreateTable, CreateTableBody, CreateTableOption,
    CreateTableOptionKind, Cte, CteBody, DataType, DefaultValue, Delete, DerivedSpelling,
    DmlSelection, DmlTarget, DoubleTypeName, EqualsSpelling, Expr, FetchSpelling,
    FilterWhereSpelling, FunctionArg, FunctionCall, GeneratedColumn, GeneratedColumnSpelling,
    GeneratedColumnStorage, GroupByItem, Ident, IdentityColumn, IdentityGeneration, IdentityOption,
    IndexColumn, Insert, InsertOverriding, InsertSource, InsertTarget, InsertValue, InsertValues,
    InsertVerb, IntegerTypeName, IntervalFields, Join, JoinOperator, LikeSpelling, Limit,
    LimitSyntax, Literal, LiteralKind, Meta, ModuloSpelling, NamedWindow, NoExt, NodeId,
    NotEqSpelling, NullTestSpelling, ObjectName, OnCommitAction, OrderByExpr, Query, QuoteStyle,
    RelationInheritance, Resolver, Select, SelectDistinct, SelectItem, SelectSpelling, SetExpr,
    SetOperator, SetQuantifier, Span, Statement, Symbol, TableAlias, TableConstraint, TableElement,
    TableFactor, TableStorageParameter, TableWithJoins, TemporaryTableKind, TextTypeName, TimeZone,
    UnaryOperator, Update, UpdateAssignment, UpdateValue, Values, ValuesItem, WhenClause,
    WindowDefinition, WindowFrame, WindowFrameBound, WindowFrameExclusion, WindowFrameUnits,
    WindowSpec, With,
};
use thin_vec::{ThinVec, thin_vec};

const NAMES: [&str; 5] = ["a", "b", "c", "t", "x"];

/// The fixed table-name symbol (`NAMES[3]` == `"t"`), named so a reorder of
/// `NAMES` cannot silently retarget every generated tree's table position with
/// no compiler signal.
pub(crate) const TABLE_SYM: u32 = 4;

/// The fixed alias/window-name symbol (`NAMES[4]` == `"x"`); see [`TABLE_SYM`].
pub(crate) const ALIAS_SYM: u32 = 5;

/// Resolver for generated ASTs.
pub struct GeneratedResolver;

impl Resolver for GeneratedResolver {
    fn try_resolve(&self, sym: Symbol) -> Option<&str> {
        NAMES.get(sym.index()).copied()
    }
}

pub static GENERATED_RESOLVER: GeneratedResolver = GeneratedResolver;

/// Generate a legal statement across the supported families.
///
/// Each arm emits a tree that round-trips through render -> parse, so the property
/// oracle stays a real check. The mix is roughly uniform so DDL/DML get genuine
/// coverage instead of being a rounding error against the query body.
pub fn arb_statement() -> impl Strategy<Value = Statement<NoExt>> {
    prop_oneof![
        arb_query_statement(),
        arb_create_table().prop_map(statement_create_table),
        arb_insert().prop_map(statement_insert),
        arb_update().prop_map(statement_update),
        arb_delete().prop_map(statement_delete),
    ]
}

/// Generate a legal top-level query statement (the original M1 subset).
fn arb_query_statement() -> impl Strategy<Value = Statement<NoExt>> {
    (
        prop::option::of(arb_with()),
        arb_set_expr(),
        prop::collection::vec(arb_order_by(), 0..3),
        arb_limit(),
    )
        .prop_map(|(with, body, order_by, limit)| {
            statement_query(Query {
                with,
                body,
                order_by: order_by.into_iter().collect(),
                order_by_all: None,
                limit_by: None,
                limit,
                // Row-locking clauses are dialect-gated and exercised by dedicated
                // cases; the generic property generator leaves them empty.
                settings: ThinVec::new(),
                format: None,
                locking: ThinVec::new(),
                pipe_operators: ThinVec::new(),
                for_clause: None,
                meta: meta(),
            })
        })
}

fn arb_set_expr() -> impl Strategy<Value = SetExpr<NoExt>> {
    (
        arb_set_operand(),
        prop::collection::vec(
            (
                prop_oneof![
                    Just(SetOperator::Union),
                    Just(SetOperator::Intersect),
                    Just(SetOperator::Except),
                ],
                any::<bool>(),
                arb_set_operand(),
            ),
            0..4,
        ),
    )
        .prop_map(|(first, rest)| {
            let mut body = first;
            for (op, all, right) in rest {
                body = SetExpr::SetOperation {
                    op,
                    all,
                    // `BY NAME` is DuckDB-only; this generator round-trips through the
                    // ANSI reparse path, which rejects it, so it stays positional.
                    by_name: false,
                    left: Box::new(body),
                    right: Box::new(right),
                    meta: meta(),
                };
            }
            body
        })
}

fn arb_set_operand() -> impl Strategy<Value = SetExpr<NoExt>> {
    prop_oneof![
        arb_select().prop_map(set_select),
        arb_values().prop_map(set_values),
    ]
}

fn arb_with() -> impl Strategy<Value = With<NoExt>> {
    (any::<bool>(), prop::option::of(any::<bool>())).prop_map(|(recursive, materialized)| With {
        recursive,
        ctes: thin_vec![Cte {
            name: ident(TABLE_SYM),
            columns: thin_vec![ident(1)],
            using_key: None,
            materialized,
            body: CteBody::Query {
                query: Box::new(Query {
                    with: None,
                    body: set_values(Values {
                        explicit_row: false,
                        rows: thin_vec![thin_vec![values_item_expr(integer_literal())]],
                        meta: meta(),
                    }),
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
            },
            search: None,
            cycle: None,
            meta: meta(),
        }],
        meta: meta(),
    })
}

fn arb_values() -> impl Strategy<Value = Values<NoExt>> {
    prop::collection::vec(prop::collection::vec(arb_values_item(), 1..3), 1..3).prop_map(|rows| {
        Values {
            explicit_row: false,
            rows: rows
                .into_iter()
                .map(|row| row.into_iter().collect())
                .collect(),
            meta: meta(),
        }
    })
}

/// A `VALUES` row item: an expression or a bare `DEFAULT`, so the round-trip
/// property exercises rendering and re-parsing `DEFAULT` in a `VALUES` row.
fn arb_values_item() -> impl Strategy<Value = ValuesItem<NoExt>> {
    prop_oneof![
        arb_scalar_expr().prop_map(values_item_expr),
        Just(ValuesItem::Default {
            default: DefaultValue { meta: meta() },
            meta: meta(),
        }),
    ]
}

pub(crate) fn values_item_expr(expr: Expr<NoExt>) -> ValuesItem<NoExt> {
    ValuesItem::Expr { expr, meta: meta() }
}

fn arb_select() -> impl Strategy<Value = Select<NoExt>> {
    (
        any::<bool>(),
        prop::collection::vec(arb_select_item(), 1..4),
        prop::collection::vec(arb_table_with_joins(), 0..3),
        prop::option::of(arb_predicate()),
        prop::collection::vec(column_expr(), 0..3),
        prop::option::of(arb_predicate()),
        arb_named_windows(),
    )
        .prop_map(
            |(distinct, projection, from, selection, group_by, having, windows)| Select {
                // Generate only "no quantifier" and `DISTINCT`; `ALL` and
                // `DISTINCT ON` have explicit unit-test coverage instead.
                distinct: distinct.then(|| SelectDistinct::Quantifier {
                    quantifier: SetQuantifier::Distinct,
                    meta: meta(),
                }),
                // STRAIGHT_JOIN is a narrow MySQL surface tag exercised by dedicated
                // cases; the generic generator leaves it unset.
                straight_join: false,
                projection: projection.into_iter().collect(),
                // `SELECT … INTO` is dialect-gated and exercised by dedicated cases;
                // the generic property generator leaves the create-table target unset.
                into: None,
                from: from.into_iter().collect(),
                // The dialect-gated Hive/Spark LATERAL VIEW clause has dedicated
                // parser/render unit-test coverage; the generic generator leaves it empty.
                lateral_views: ThinVec::new(),
                connect_by: None,
                selection,
                // The generator produces only plain-expression GROUP BY items; the
                // grouping-set constructs have dedicated round-trip and differential
                // coverage.
                group_by: group_by
                    .into_iter()
                    .map(|expr| GroupByItem::Expr { expr, meta: meta() })
                    .collect(),
                // Generate only the unquantified list; the `DISTINCT`/`ALL` grouping-set
                // quantifier has explicit parser/render unit-test coverage instead.
                group_by_quantifier: None,
                group_by_all: None,
                having,
                // A `WINDOW name AS (...)` clause; a definition need not be referenced
                // by an `OVER name` to be legal, so these stand alone.
                windows: windows.into_iter().collect(),
                // `QUALIFY` is dialect-gated (DuckDB) and exercised by dedicated
                // cases; the generic property generator leaves it unset.
                qualify: None,
                // `USING SAMPLE` is dialect-gated (DuckDB) and exercised by dedicated
                // cases; the generic property generator leaves it unset.
                sample: None,
                spelling: SelectSpelling::Select,
                meta: meta(),
            },
        )
        // Boxed to bound strategy-construction stack depth: `arb_select` is heavy and
        // embedded several times (set operands, `arb_embedded_query`), so erasing its
        // type here keeps the composed `arb_statement` strategy from overflowing.
        .boxed()
}

fn arb_select_item() -> impl Strategy<Value = SelectItem<NoExt>> {
    prop_oneof![
        Just(SelectItem::Wildcard {
            options: None,
            alias: None,
            alias_spelling: AliasSpelling::As,
            meta: meta()
        }),
        // `t.*` qualified wildcard — ANSI-standard projection form (oracle-parity-ansi).
        Just(SelectItem::QualifiedWildcard {
            name: object_name(&[TABLE_SYM]),
            options: None,
            alias: None,
            alias_spelling: AliasSpelling::As,
            meta: meta(),
        }),
        arb_scalar_expr().prop_map(|expr| SelectItem::Expr {
            expr,
            alias: None,
            alias_spelling: AliasSpelling::As,
            meta: meta(),
        }),
        arb_scalar_expr().prop_map(|expr| SelectItem::Expr {
            expr,
            alias: Some(ident(ALIAS_SYM)),
            alias_spelling: AliasSpelling::As,
            meta: meta(),
        }),
        // Window-function calls (`f(...) OVER ...`) are only legal in the projection
        // and ORDER BY, so they are seeded here rather than in `arb_scalar_expr`.
        arb_window_function_expr().prop_map(|expr| SelectItem::Expr {
            expr,
            alias: None,
            alias_spelling: AliasSpelling::As,
            meta: meta(),
        }),
        // Scalar subquery in projection (ANSI `Expr::Subquery`). Uses a *shallow*
        // select (no nested derived/subquery) so strategy construction cannot
        // blow the stack through mutual recursion with `arb_select`.
        arb_shallow_select_query().prop_map(|query| SelectItem::Expr {
            expr: Expr::Subquery {
                query: Box::new(query),
                meta: meta(),
            },
            alias: None,
            alias_spelling: AliasSpelling::As,
            meta: meta(),
        }),
    ]
}

fn arb_table_with_joins() -> impl Strategy<Value = TableWithJoins<NoExt>> {
    (arb_table_factor(), prop::collection::vec(arb_join(), 0..2)).prop_map(|(relation, joins)| {
        TableWithJoins {
            relation,
            joins: joins.into_iter().collect(),
            meta: meta(),
        }
    })
}

/// A FROM/JOIN relation: plain named table or parenthesized derived table
/// (`(SELECT …) AS x`). Unnest/function factors stay dialect-gated out of the ANSI
/// reparse path (oracle-parity-ansi).
fn arb_table_factor() -> impl Strategy<Value = TableFactor<NoExt>> {
    prop_oneof![
        prop::option::of(Just(ident(ALIAS_SYM)))
            .prop_map(|alias| table_factor(object_name(&[TABLE_SYM]), alias)),
        arb_shallow_select_query().prop_map(|subquery| TableFactor::Derived {
            lateral: false,
            subquery: Box::new(subquery),
            alias: Some(Box::new(TableAlias {
                name: ident(ALIAS_SYM),
                columns: ThinVec::new(),
                spelling: AliasSpelling::As,
                meta: meta(),
            })),
            spelling: DerivedSpelling::Parenthesized,
            meta: meta(),
        }),
    ]
}

fn arb_join() -> impl Strategy<Value = Join<NoExt>> {
    (
        prop_oneof![Just(0_u8), Just(1), Just(2), Just(3), Just(4)],
        prop::option::of(Just(ident(ALIAS_SYM))),
        arb_join_constraint(),
    )
        .prop_map(|(kind, alias, constraint)| {
            let relation = table_factor(object_name(&[TABLE_SYM]), alias);
            // The constrained operators all carry an `ON`/`USING`/`NATURAL`
            // constraint; `CROSS JOIN` takes none, so it drops the generated one.
            let operator = match kind {
                0 => JoinOperator::Inner {
                    straight: false,
                    inner: false,
                    constraint,
                    meta: meta(),
                },
                1 => JoinOperator::LeftOuter {
                    outer: false,
                    constraint,
                    meta: meta(),
                },
                2 => JoinOperator::RightOuter {
                    outer: false,
                    constraint,
                    meta: meta(),
                },
                3 => JoinOperator::FullOuter {
                    outer: false,
                    constraint,
                    meta: meta(),
                },
                _ => JoinOperator::Cross { meta: meta() },
            };
            Join {
                relation,
                operator,
                meta: meta(),
            }
        })
}

/// A join predicate for a constrained (non-cross) operator: `ON <expr>`,
/// `USING (<columns>)`, or `NATURAL`.
fn arb_join_constraint() -> impl Strategy<Value = squonk_ast::JoinConstraint<NoExt>> {
    prop_oneof![
        arb_predicate().prop_map(join_constraint_on),
        prop::collection::vec(
            prop_oneof![Just(ident(1)), Just(ident(2)), Just(ident(3))],
            1..3
        )
        .prop_map(|columns| squonk_ast::JoinConstraint::Using {
            columns: columns.into_iter().collect(),
            // `USING (...) AS alias` is a PostgreSQL extension that the ANSI
            // reparse path used by the oracle does not accept, so it stays unset.
            alias: None,
            meta: meta(),
        }),
        Just(squonk_ast::JoinConstraint::Natural { meta: meta() }),
    ]
}

fn arb_order_by() -> impl Strategy<Value = OrderByExpr<NoExt>> {
    (
        arb_scalar_expr(),
        prop::option::of(any::<bool>()),
        prop::option::of(any::<bool>()),
    )
        .prop_map(|(expr, asc, nulls_first)| OrderByExpr {
            expr,
            asc,
            using: None,
            nulls_first,
            meta: meta(),
        })
}

fn arb_limit() -> impl Strategy<Value = Option<Limit<NoExt>>> {
    prop::option::of(prop_oneof![
        Just(Limit {
            limit: Some(integer_literal()),
            offset: None,
            syntax: LimitSyntax::LimitOffset,
            with_ties: None,
            percent: None,
            fetch_spelling: FetchSpelling::FirstRows,
            meta: meta(),
        }),
        Just(Limit {
            limit: None,
            offset: Some(integer_literal()),
            syntax: LimitSyntax::LimitOffset,
            with_ties: None,
            percent: None,
            fetch_spelling: FetchSpelling::FirstRows,
            meta: meta(),
        }),
        Just(Limit {
            limit: Some(integer_literal()),
            offset: Some(integer_literal()),
            syntax: LimitSyntax::LimitOffset,
            with_ties: None,
            percent: None,
            fetch_spelling: FetchSpelling::FirstRows,
            meta: meta(),
        }),
    ])
}

fn arb_scalar_expr() -> impl Strategy<Value = Expr<NoExt>> {
    prop_oneof![
        column_expr(),
        arb_literal_expr(),
        arb_function_expr(),
        arb_cast_expr(),
        arb_case_expr(),
    ]
    .prop_recursive(4, 64, 3, |inner| {
        prop_oneof![
            (
                inner.clone(),
                prop_oneof![
                    Just(BinaryOperator::Plus),
                    Just(BinaryOperator::Minus),
                    Just(BinaryOperator::Multiply),
                    Just(BinaryOperator::Divide),
                    Just(BinaryOperator::Modulo(ModuloSpelling::Percent)),
                ],
                inner.clone(),
            )
                .prop_map(|(left, op, right)| binary(left, op, right)),
            (
                prop_oneof![Just(UnaryOperator::Plus), Just(UnaryOperator::Minus)],
                inner
            )
                .prop_map(|(op, expr)| unary(op, expr)),
        ]
    })
}

/// `CAST(<expr> AS <type>)` — ANSI call form (double-colon is dialect-gated).
fn arb_cast_expr() -> impl Strategy<Value = Expr<NoExt>> {
    (column_expr(), arb_data_type()).prop_map(|(expr, data_type)| Expr::Cast {
        expr: Box::new(expr),
        data_type: Box::new(data_type),
        syntax: CastSyntax::Call,
        try_cast: false,
        meta: meta(),
    })
}

/// Searched `CASE WHEN … THEN … [ELSE …] END` (ANSI).
///
/// Conditions are simple column/literal comparisons — not full [`arb_predicate`] —
/// so strategy construction does not recurse through `arb_scalar_expr` → CASE →
/// predicate → scalar.
fn arb_case_expr() -> impl Strategy<Value = Expr<NoExt>> {
    (
        prop::collection::vec(
            (
                (
                    column_expr(),
                    prop_oneof![
                        Just(BinaryOperator::Eq(EqualsSpelling::Single)),
                        Just(BinaryOperator::Lt),
                        Just(BinaryOperator::Gt),
                    ],
                    arb_literal_expr(),
                )
                    .prop_map(|(l, op, r)| binary(l, op, r)),
                arb_literal_expr(),
            ),
            1..3,
        ),
        prop::option::of(arb_literal_expr()),
    )
        .prop_map(|(whens, else_result)| Expr::Case {
            case: Box::new(CaseExpr {
                operand: None,
                when_clauses: whens
                    .into_iter()
                    .map(|(condition, result)| WhenClause {
                        condition,
                        result,
                        meta: meta(),
                    })
                    .collect(),
                else_result: else_result.map(Box::new),
                meta: meta(),
            }),
            meta: meta(),
        })
}

fn arb_literal_expr() -> impl Strategy<Value = Expr<NoExt>> {
    prop_oneof![
        Just(integer_literal()),
        Just(literal_expr(LiteralKind::String)),
        Just(literal_expr(LiteralKind::Boolean(true))),
        Just(literal_expr(LiteralKind::Boolean(false))),
        Just(literal_expr(LiteralKind::Null)),
        arb_temporal_literal(),
        // `LiteralKind::Float` is intentionally absent: a detached float has no
        // backing source text, so it renders as the kind placeholder `0`, which
        // reparses as `LiteralKind::Integer` — the one literal kind that cannot
        // survive the synthetic-span round-trip. Float spelling fidelity is covered
        // by the parser/render goldens that carry real source instead.
    ]
}

/// A typed temporal literal: `DATE`/`TIME`/`TIMESTAMP`/`INTERVAL`.
///
/// The renderer re-emits a placeholder value of the right type carrying the
/// time-zone flag (`TIME WITH TIME ZONE '...'`) or interval qualifier
/// (`INTERVAL '0' DAY`), so those tags round-trip. An interval *precision* is
/// omitted: the literal renderer drops it (only the type-position renderer keeps
/// it), so a `precision: Some(_)` would not survive the round-trip.
fn arb_temporal_literal() -> impl Strategy<Value = Expr<NoExt>> {
    prop_oneof![
        Just(literal_expr(LiteralKind::Date)),
        arb_time_zone().prop_map(|time_zone| literal_expr(LiteralKind::Time { time_zone })),
        arb_time_zone().prop_map(|time_zone| literal_expr(LiteralKind::Timestamp { time_zone })),
        arb_interval_fields().prop_map(|fields| literal_expr(LiteralKind::Interval {
            fields,
            precision: None,
        })),
    ]
}

fn arb_time_zone() -> impl Strategy<Value = TimeZone> {
    prop_oneof![
        Just(TimeZone::Unspecified),
        Just(TimeZone::WithTimeZone),
        Just(TimeZone::WithoutTimeZone),
    ]
}

fn arb_interval_fields() -> impl Strategy<Value = Option<IntervalFields>> {
    prop_oneof![
        Just(None),
        Just(Some(IntervalFields::Year)),
        Just(Some(IntervalFields::Month)),
        Just(Some(IntervalFields::Day)),
        Just(Some(IntervalFields::Hour)),
        Just(Some(IntervalFields::Minute)),
        Just(Some(IntervalFields::Second)),
        Just(Some(IntervalFields::YearToMonth)),
        Just(Some(IntervalFields::DayToHour)),
        Just(Some(IntervalFields::DayToMinute)),
        Just(Some(IntervalFields::DayToSecond)),
        Just(Some(IntervalFields::HourToMinute)),
        Just(Some(IntervalFields::HourToSecond)),
        Just(Some(IntervalFields::MinuteToSecond)),
    ]
}

fn arb_predicate() -> impl Strategy<Value = Expr<NoExt>> {
    prop_oneof![
        (
            arb_scalar_expr(),
            prop_oneof![
                Just(BinaryOperator::Eq(EqualsSpelling::Single)),
                Just(BinaryOperator::NotEq(NotEqSpelling::AngleBracket)),
                Just(BinaryOperator::Lt),
                Just(BinaryOperator::LtEq),
                Just(BinaryOperator::Gt),
                Just(BinaryOperator::GtEq),
            ],
            arb_scalar_expr(),
        )
            .prop_map(|(left, op, right)| binary(left, op, right)),
        // `<expr> IS [NOT] NULL` — standard spelling only (postfix forms are dialect-gated).
        (arb_scalar_expr(), prop::bool::ANY).prop_map(|(expr, negated)| Expr::IsNull {
            expr: Box::new(expr),
            negated,
            spelling: NullTestSpelling::Is,
            meta: meta(),
        }),
        // `<expr> [NOT] LIKE <pattern>`
        (arb_scalar_expr(), arb_literal_expr(), prop::bool::ANY).prop_map(
            |(expr, pattern, negated)| Expr::Like {
                expr: Box::new(expr),
                pattern: Box::new(pattern),
                escape: None,
                negated,
                spelling: LikeSpelling::Like,
                meta: meta(),
            },
        ),
        // `<expr> [NOT] BETWEEN <low> AND <high>` (asymmetric default)
        (
            arb_scalar_expr(),
            arb_literal_expr(),
            arb_literal_expr(),
            prop::bool::ANY,
        )
            .prop_map(|(expr, low, high, negated)| Expr::Between {
                expr: Box::new(expr),
                low: Box::new(low),
                high: Box::new(high),
                negated,
                symmetric: false,
                meta: meta(),
            }),
        // `<expr> [NOT] IN (<list>)`
        (
            arb_scalar_expr(),
            prop::collection::vec(arb_literal_expr(), 1..4),
            prop::bool::ANY,
        )
            .prop_map(|(expr, list, negated)| Expr::InList {
                expr: Box::new(expr),
                list: list.into_iter().collect(),
                negated,
                meta: meta(),
            }),
        // `EXISTS (<query>)` — shallow select only (stack-bounded).
        arb_shallow_select_query().prop_map(|query| Expr::Exists {
            query: Box::new(query),
            meta: meta(),
        }),
    ]
    .prop_recursive(3, 32, 2, |inner| {
        prop_oneof![
            (inner.clone(), inner.clone()).prop_map(|(left, right)| binary(
                left,
                BinaryOperator::And,
                right
            )),
            (inner.clone(), inner.clone()).prop_map(|(left, right)| binary(
                left,
                BinaryOperator::Or,
                right
            )),
            inner.prop_map(|expr| unary(UnaryOperator::Not, expr)),
        ]
    })
}

fn column_expr() -> impl Strategy<Value = Expr<NoExt>> {
    prop_oneof![
        Just(column_expr_from_name(object_name(&[1]))),
        Just(column_expr_from_name(object_name(&[2]))),
        Just(column_expr_from_name(object_name(&[3]))),
    ]
}

pub(crate) fn binary(left: Expr<NoExt>, op: BinaryOperator, right: Expr<NoExt>) -> Expr<NoExt> {
    Expr::BinaryOp {
        left: Box::new(left),
        op,
        right: Box::new(right),
        meta: meta(),
    }
}

pub(crate) fn unary(op: UnaryOperator, expr: Expr<NoExt>) -> Expr<NoExt> {
    Expr::UnaryOp {
        op,
        expr: Box::new(expr),
        meta: meta(),
    }
}

pub(crate) fn integer_literal() -> Expr<NoExt> {
    literal_expr(LiteralKind::Integer)
}

pub(crate) fn statement_query(query: Query<NoExt>) -> Statement<NoExt> {
    Statement::Query {
        query: Box::new(query),
        meta: meta(),
    }
}

pub(crate) fn set_select(select: Select<NoExt>) -> SetExpr<NoExt> {
    SetExpr::Select {
        select: Box::new(select),
        meta: meta(),
    }
}

pub(crate) fn set_values(values: Values<NoExt>) -> SetExpr<NoExt> {
    SetExpr::Values {
        values: Box::new(values),
        meta: meta(),
    }
}

pub(crate) fn table_factor(name: ObjectName, alias: Option<Ident>) -> TableFactor<NoExt> {
    TableFactor::Table {
        name,
        inheritance: RelationInheritance::Plain,
        json_path: ThinVec::new(),
        version: None,
        partition: ThinVec::new(),
        alias: alias.map(|name| {
            Box::new(TableAlias {
                name,
                columns: ThinVec::new(),
                spelling: AliasSpelling::As,
                meta: meta(),
            })
        }),
        indexed_by: None,
        index_hints: ThinVec::new(),
        sample: None,
        table_hints: ThinVec::new(),
        meta: meta(),
    }
}

fn join_constraint_on(expr: Expr<NoExt>) -> squonk_ast::JoinConstraint<NoExt> {
    squonk_ast::JoinConstraint::On { expr, meta: meta() }
}

pub(crate) fn column_expr_from_name(name: ObjectName) -> Expr<NoExt> {
    Expr::Column { name, meta: meta() }
}

/// A bare-column `IndexColumn` (no `COLLATE`/`ASC`/`DESC`) — the shape a `PRIMARY
/// KEY`/`UNIQUE` constraint column list holds for a plain name.
fn bare_index_column(name: Ident) -> IndexColumn<NoExt> {
    IndexColumn {
        expr: Expr::Column {
            name: ObjectName(thin_vec![name]),
            meta: meta(),
        },
        asc: None,
        nulls_first: None,
        meta: meta(),
    }
}

pub(crate) fn literal_expr(kind: LiteralKind) -> Expr<NoExt> {
    Expr::Literal {
        literal: Literal { kind, meta: meta() },
        meta: meta(),
    }
}

pub(crate) fn object_name(symbols: &[u32]) -> ObjectName {
    ObjectName(symbols.iter().map(|&sym| ident(sym)).collect())
}

pub(crate) fn ident(sym: u32) -> Ident {
    Ident {
        sym: Symbol::new(sym).expect("generated symbols are one-based"),
        quote: QuoteStyle::None,
        meta: meta(),
    }
}

pub(crate) fn meta() -> Meta {
    Meta::new(
        Span::SYNTHETIC,
        NodeId::new(1).expect("node ids are one-based"),
    )
}

// ---------------------------------------------------------------------------
// Statement constructors for the DDL/DML families
// ---------------------------------------------------------------------------

pub(crate) fn statement_create_table(create: CreateTable<NoExt>) -> Statement<NoExt> {
    Statement::CreateTable {
        create: Box::new(create),
        meta: meta(),
    }
}

pub(crate) fn statement_insert(insert: Insert<NoExt>) -> Statement<NoExt> {
    Statement::Insert {
        insert: Box::new(insert),
        meta: meta(),
    }
}

pub(crate) fn statement_update(update: Update<NoExt>) -> Statement<NoExt> {
    Statement::Update {
        update: Box::new(update),
        meta: meta(),
    }
}

pub(crate) fn statement_delete(delete: Delete<NoExt>) -> Statement<NoExt> {
    Statement::Delete {
        delete: Box::new(delete),
        meta: meta(),
    }
}

/// A bare query (no top-level `WITH`/`ORDER BY`/`LIMIT`) used where a statement
/// embeds a subquery: `CREATE TABLE ... AS`, `INSERT ... SELECT`, an `UPDATE`
/// tuple subquery, and so on. Kept small so embedded queries do not balloon.
pub(crate) fn query_of(body: SetExpr<NoExt>) -> Query<NoExt> {
    Query {
        with: None,
        body,
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
    }
}

/// A *shallow* SELECT query: projection of plain scalars over a plain table, no
/// nested derived tables or projection subqueries. Used as the leaf for
/// `EXISTS` / scalar-subquery / derived-table factors so the generator cannot
/// recurse unboundedly through `arb_select` → subquery → `arb_select`.
fn arb_shallow_select_query() -> impl Strategy<Value = Query<NoExt>> {
    (
        prop::collection::vec(
            prop_oneof![column_expr(), arb_literal_expr(), arb_cast_expr(),],
            1..3,
        ),
        prop::option::of(Just(ident(ALIAS_SYM))),
    )
        .prop_map(|(exprs, alias)| {
            let select = Select {
                distinct: None,
                straight_join: false,
                projection: exprs
                    .into_iter()
                    .map(|expr| SelectItem::Expr {
                        expr,
                        alias: None,
                        alias_spelling: AliasSpelling::As,
                        meta: meta(),
                    })
                    .collect(),
                into: None,
                from: thin_vec![TableWithJoins {
                    relation: table_factor(object_name(&[TABLE_SYM]), alias),
                    joins: ThinVec::new(),
                    meta: meta(),
                }],
                lateral_views: ThinVec::new(),
                selection: None,
                connect_by: None,
                group_by: ThinVec::new(),
                group_by_quantifier: None,
                group_by_all: None,
                having: None,
                windows: ThinVec::new(),
                qualify: None,
                sample: None,
                spelling: SelectSpelling::Select,
                meta: meta(),
            };
            query_of(set_select(select))
        })
        .boxed()
}

fn arb_embedded_query() -> impl Strategy<Value = Query<NoExt>> {
    prop_oneof![
        arb_select().prop_map(|select| query_of(set_select(select))),
        arb_values().prop_map(|values| query_of(set_values(values))),
    ]
    // Boxed: embedded in CTAS / INSERT ... SELECT / UPDATE tuple subqueries, so type
    // erasure keeps the composed strategy shallow.
    .boxed()
}

// ---------------------------------------------------------------------------
// Function calls and window (`OVER`) clauses
// ---------------------------------------------------------------------------

pub(crate) fn function_expr(call: FunctionCall<NoExt>) -> Expr<NoExt> {
    Expr::Function {
        call: Box::new(call),
        meta: meta(),
    }
}

/// A positional function argument wrapping `value`. The generator emits only
/// positional arguments; the named-argument surface is exercised by the parser unit
/// tests and the labelled coverage case instead.
pub(crate) fn positional_arg(value: Expr<NoExt>) -> FunctionArg<NoExt> {
    FunctionArg {
        name: None,
        variadic: false,
        syntax: ArgSyntax::Positional,
        value,
        meta: meta(),
    }
}

pub(crate) fn function_call(
    name: ObjectName,
    quantifier: Option<SetQuantifier>,
    args: ThinVec<Expr<NoExt>>,
    wildcard: bool,
    over: Option<WindowSpec<NoExt>>,
) -> FunctionCall<NoExt> {
    FunctionCall {
        name,
        quantifier,
        args: args.into_iter().map(positional_arg).collect(),
        wildcard,
        // ORDER BY / WITHIN GROUP / FILTER aggregate modifiers are left empty: they are
        // aggregate-only and add legality coupling without exercising new structure that
        // the window path below does not already cover.
        order_by: ThinVec::new(),
        separator: None,
        within_group: None,
        filter: None,
        filter_where: FilterWhereSpelling::Where,
        over,
        // `IGNORE`/`RESPECT NULLS` is dialect-gated (DuckDB) and exercised by dedicated
        // cases; the generic generator leaves it unset.
        null_treatment: None,
        // MySQL's window-function post-`)` tail is dialect-gated and exercised by
        // dedicated cases; the generic generator leaves it unset.
        window_tail: None,
        meta: meta(),
    }
}

/// A plain (non-window) scalar or aggregate call, legal anywhere an expression is.
fn arb_function_expr() -> impl Strategy<Value = Expr<NoExt>> {
    prop_oneof![
        // f(arg, ...)
        prop::collection::vec(arb_call_argument(), 1..3).prop_map(|args| function_expr(
            function_call(
                object_name(&[1]),
                None,
                args.into_iter().collect(),
                false,
                None
            )
        )),
        // f(*) — a wildcard call such as count(*).
        Just(function_expr(function_call(
            object_name(&[2]),
            None,
            ThinVec::new(),
            true,
            None,
        ))),
        // f(DISTINCT arg) — an aggregate with a set quantifier.
        arb_call_argument().prop_map(|arg| function_expr(function_call(
            object_name(&[1]),
            Some(SetQuantifier::Distinct),
            thin_vec![arg],
            false,
            None,
        ))),
    ]
}

/// Arguments to a generated call: a column or literal, kept shallow so calls do
/// not recurse into the full expression grammar.
fn arb_call_argument() -> impl Strategy<Value = Expr<NoExt>> {
    prop_oneof![column_expr(), arb_literal_expr()]
}

/// A window-function call `f(...) OVER ...`; only legal in the projection/ORDER BY.
fn arb_window_function_expr() -> impl Strategy<Value = Expr<NoExt>> {
    (
        prop_oneof![
            Just((true, ThinVec::new())),
            prop::collection::vec(arb_call_argument(), 1..3)
                .prop_map(|args| (false, args.into_iter().collect())),
        ],
        arb_window_spec(),
    )
        .prop_map(|((wildcard, args), over)| {
            function_expr(function_call(
                object_name(&[1]),
                None,
                args,
                wildcard,
                Some(over),
            ))
        })
        .boxed()
}

fn arb_window_spec() -> impl Strategy<Value = WindowSpec<NoExt>> {
    prop_oneof![
        // `OVER name` — parsing does not require the name to be a defined window.
        Just(WindowSpec::Named {
            name: window_name(),
            meta: meta(),
        }),
        arb_window_definition().prop_map(|definition| WindowSpec::Inline {
            definition: Box::new(definition),
            meta: meta(),
        }),
    ]
}

fn window_name() -> Ident {
    ident(ALIAS_SYM)
}

/// The `WINDOW name AS (...)` definitions on a SELECT. Names are kept distinct so
/// a multi-entry clause does not collide on a duplicate window name.
fn arb_named_windows() -> impl Strategy<Value = Vec<NamedWindow<NoExt>>> {
    prop_oneof![
        Just(Vec::new()),
        arb_window_definition().prop_map(|definition| vec![named_window(TABLE_SYM, definition)]),
        (arb_window_definition(), arb_window_definition())
            .prop_map(|(a, b)| vec![named_window(TABLE_SYM, a), named_window(ALIAS_SYM, b)]),
    ]
}

pub(crate) fn named_window(sym: u32, definition: WindowDefinition<NoExt>) -> NamedWindow<NoExt> {
    NamedWindow {
        name: ident(sym),
        definition,
        meta: meta(),
    }
}

pub(crate) fn window_definition(
    partition_by: Vec<Expr<NoExt>>,
    order_by: Vec<OrderByExpr<NoExt>>,
    frame: Option<WindowFrame<NoExt>>,
) -> WindowDefinition<NoExt> {
    WindowDefinition {
        // A base-window reference (`OVER (w ...)`) needs a matching WINDOW definition
        // and forbids re-specifying parts the base set, which this generator does not
        // coordinate, so it stays unset.
        existing: None,
        partition_by: partition_by.into_iter().collect(),
        order_by: order_by.into_iter().collect(),
        frame,
        meta: meta(),
    }
}

fn arb_window_definition() -> impl Strategy<Value = WindowDefinition<NoExt>> {
    prop_oneof![
        // No frame: PARTITION BY / ORDER BY are independently optional.
        (
            prop::collection::vec(column_expr(), 0..2),
            prop::collection::vec(arb_order_by(), 0..2),
        )
            .prop_map(|(partition_by, order_by)| window_definition(
                partition_by,
                order_by,
                None
            )),
        // With a frame: pair it with exactly one ORDER BY key so the RANGE/GROUPS
        // frames are legal (they require an ordering).
        (
            prop::collection::vec(column_expr(), 0..2),
            arb_order_by(),
            arb_window_frame(),
        )
            .prop_map(|(partition_by, order_key, frame)| {
                window_definition(partition_by, vec![order_key], Some(frame))
            }),
    ]
    .boxed()
}

pub(crate) fn window_frame(
    units: WindowFrameUnits,
    start: WindowFrameBound<NoExt>,
    end: Option<WindowFrameBound<NoExt>>,
    exclusion: Option<WindowFrameExclusion>,
) -> WindowFrame<NoExt> {
    WindowFrame {
        units,
        start,
        end,
        exclusion,
        meta: meta(),
    }
}

/// A curated set of unambiguously legal frames (every bound variant and all three
/// units appear), each with an optional `EXCLUDE`. Offset bounds (`<n> PRECEDING`)
/// ride `ROWS`; RANGE/GROUPS use only the unbounded/current-row extents.
fn arb_window_frame() -> impl Strategy<Value = WindowFrame<NoExt>> {
    prop_oneof![
        opt_exclusion().prop_map(|e| window_frame(
            WindowFrameUnits::Rows,
            frame_unbounded_preceding(),
            None,
            e,
        )),
        opt_exclusion().prop_map(|e| window_frame(
            WindowFrameUnits::Rows,
            frame_current_row(),
            None,
            e
        )),
        opt_exclusion().prop_map(|e| window_frame(
            WindowFrameUnits::Rows,
            frame_unbounded_preceding(),
            Some(frame_current_row()),
            e,
        )),
        opt_exclusion().prop_map(|e| window_frame(
            WindowFrameUnits::Rows,
            frame_preceding(),
            Some(frame_following()),
            e,
        )),
        opt_exclusion().prop_map(|e| window_frame(
            WindowFrameUnits::Rows,
            frame_current_row(),
            Some(frame_unbounded_following()),
            e,
        )),
        opt_exclusion().prop_map(|e| window_frame(
            WindowFrameUnits::Range,
            frame_unbounded_preceding(),
            Some(frame_unbounded_following()),
            e,
        )),
        opt_exclusion().prop_map(|e| window_frame(
            WindowFrameUnits::Groups,
            frame_unbounded_preceding(),
            Some(frame_current_row()),
            e,
        )),
    ]
}

fn opt_exclusion() -> impl Strategy<Value = Option<WindowFrameExclusion>> {
    prop::option::of(prop_oneof![
        Just(WindowFrameExclusion::CurrentRow),
        Just(WindowFrameExclusion::Group),
        Just(WindowFrameExclusion::Ties),
        Just(WindowFrameExclusion::NoOthers),
    ])
}

pub(crate) fn frame_current_row() -> WindowFrameBound<NoExt> {
    WindowFrameBound::CurrentRow { meta: meta() }
}

pub(crate) fn frame_unbounded_preceding() -> WindowFrameBound<NoExt> {
    WindowFrameBound::UnboundedPreceding { meta: meta() }
}

pub(crate) fn frame_unbounded_following() -> WindowFrameBound<NoExt> {
    WindowFrameBound::UnboundedFollowing { meta: meta() }
}

pub(crate) fn frame_preceding() -> WindowFrameBound<NoExt> {
    WindowFrameBound::Preceding {
        offset: Box::new(integer_literal()),
        meta: meta(),
    }
}

pub(crate) fn frame_following() -> WindowFrameBound<NoExt> {
    WindowFrameBound::Following {
        offset: Box::new(integer_literal()),
        meta: meta(),
    }
}

// ---------------------------------------------------------------------------
// CREATE TABLE
// ---------------------------------------------------------------------------

fn arb_create_table() -> impl Strategy<Value = CreateTable<NoExt>> {
    (
        arb_temporary_and_on_commit(),
        any::<bool>(),
        arb_create_table_body(),
        prop::option::of(arb_with_option()),
        prop::option::of(arb_tablespace_option()),
    )
        .prop_map(
            |((temporary, on_commit), if_not_exists, body, with, tablespace)| CreateTable {
                or_replace: false,
                temporary,
                unlogged: false,
                if_not_exists,
                name: object_name(&[TABLE_SYM]),
                body,
                inherits: ThinVec::new(),
                partition_by: None,
                access_method: None,
                // PostgreSQL renders these in the order WITH, ON COMMIT, TABLESPACE.
                options: [with, on_commit, tablespace]
                    .into_iter()
                    .flatten()
                    .collect(),
                meta: meta(),
            },
        )
        .boxed()
}

/// `ON COMMIT` is only legal on a temporary table, so the two are generated
/// together: a non-temporary table never carries an `ON COMMIT` option.
fn arb_temporary_and_on_commit()
-> impl Strategy<Value = (Option<TemporaryTableKind>, Option<CreateTableOption<NoExt>>)> {
    prop_oneof![
        Just((None, None)),
        arb_temporary_kind().prop_map(|kind| (Some(kind), None)),
        (arb_temporary_kind(), arb_on_commit_action())
            .prop_map(|(kind, action)| (Some(kind), Some(on_commit_option(action)))),
    ]
}

fn arb_temporary_kind() -> impl Strategy<Value = TemporaryTableKind> {
    prop_oneof![
        Just(TemporaryTableKind::Temp),
        Just(TemporaryTableKind::Temporary),
    ]
}

fn arb_create_table_body() -> impl Strategy<Value = CreateTableBody<NoExt>> {
    prop_oneof![
        prop::collection::vec(arb_table_element(), 1..4).prop_map(|elements| {
            CreateTableBody::Definition {
                elements: elements.into_iter().collect(),
                meta: meta(),
            }
        }),
        (
            prop::collection::vec(arb_column_name(), 0..3),
            arb_embedded_query(),
            prop::option::of(any::<bool>()),
        )
            .prop_map(|(columns, query, with_data)| CreateTableBody::AsQuery {
                columns: columns.into_iter().collect(),
                query: Box::new(query),
                with_data,
                meta: meta(),
            }),
    ]
    .boxed()
}

fn arb_table_element() -> impl Strategy<Value = TableElement<NoExt>> {
    prop_oneof![
        arb_column_def().prop_map(|column| TableElement::Column {
            column,
            meta: meta(),
        }),
        arb_table_constraint_def().prop_map(|constraint| TableElement::Constraint {
            constraint,
            meta: meta(),
        }),
    ]
}

fn arb_column_def() -> impl Strategy<Value = ColumnDef<NoExt>> {
    (
        arb_column_name(),
        arb_data_type(),
        // At most one column constraint: combining them (e.g. `NULL` with `PRIMARY
        // KEY`) risks conflicting clauses the reparse rejects. Multi-constraint
        // columns are exercised by the corpus instead.
        prop::collection::vec(arb_column_constraint(), 0..2),
    )
        .prop_map(|(name, data_type, constraints)| ColumnDef {
            name,
            data_type: Some(data_type),
            storage: None,
            compression: None,
            constraints: constraints.into_iter().collect(),
            meta: meta(),
        })
}

/// A small set of unambiguous, unsized type spellings. The full data-type matrix
/// (sizes, precisions, dialect spellings) has its own targeted round-trip coverage.
fn arb_data_type() -> impl Strategy<Value = DataType> {
    prop_oneof![
        Just(DataType::SmallInt {
            display_width: None,
            meta: meta(),
        }),
        Just(DataType::Integer {
            spelling: IntegerTypeName::Int,
            display_width: None,
            meta: meta(),
        }),
        Just(DataType::Integer {
            spelling: IntegerTypeName::Integer,
            display_width: None,
            meta: meta(),
        }),
        Just(DataType::BigInt {
            display_width: None,
            meta: meta(),
        }),
        Just(DataType::Boolean {
            spelling: BooleanTypeName::Boolean,
            meta: meta(),
        }),
        Just(DataType::Text {
            spelling: TextTypeName::Text,
            charset: None,
            meta: meta(),
        }),
        Just(DataType::Real { meta: meta() }),
        Just(DataType::Double {
            spelling: DoubleTypeName::DoublePrecision,
            meta: meta(),
        }),
        Just(DataType::Date { meta: meta() }),
    ]
}

fn arb_column_constraint() -> impl Strategy<Value = ColumnConstraint<NoExt>> {
    prop_oneof![
        // `DEFAULT`/`GENERATED`/`IDENTITY`/`NULL` cannot take a `CONSTRAINT name`.
        arb_unnamed_column_option().prop_map(|option| ColumnConstraint {
            name: None,
            option,
            conflict: None,
            characteristics: None,
            meta: meta(),
        }),
        (
            prop::option::of(arb_column_name()),
            arb_named_column_option()
        )
            .prop_map(|(name, option)| ColumnConstraint {
                name,
                option,
                conflict: None,
                characteristics: None,
                meta: meta(),
            }),
    ]
}

fn arb_unnamed_column_option() -> impl Strategy<Value = ColumnOption<NoExt>> {
    prop_oneof![
        Just(ColumnOption::Null { meta: meta() }),
        Just(ColumnOption::NotNull { meta: meta() }),
        // A literal default keeps `DEFAULT <expr>` (which renders without parens)
        // from colliding with a following constraint keyword.
        arb_literal_expr().prop_map(|expr| ColumnOption::Default {
            expr: Box::new(expr),
            meta: meta(),
        }),
        arb_generated_column().prop_map(|generated| ColumnOption::Generated {
            generated: Box::new(generated),
            meta: meta(),
        }),
        arb_identity_column().prop_map(|identity| ColumnOption::Identity {
            identity: Box::new(identity),
            meta: meta(),
        }),
    ]
}

fn arb_named_column_option() -> impl Strategy<Value = ColumnOption<NoExt>> {
    prop_oneof![
        Just(ColumnOption::PrimaryKey {
            ascending: None,
            index_tablespace: None,
            meta: meta(),
        }),
        Just(ColumnOption::Unique {
            nulls_not_distinct: None,
            index_tablespace: None,
            meta: meta(),
        }),
        arb_predicate().prop_map(|expr| ColumnOption::Check {
            expr: Box::new(expr),
            no_inherit: false,
            meta: meta(),
        }),
        arb_foreign_key_ref().prop_map(|reference| ColumnOption::References {
            reference: Box::new(reference),
            meta: meta(),
        }),
    ]
}

fn arb_generated_column() -> impl Strategy<Value = GeneratedColumn<NoExt>> {
    arb_scalar_expr().prop_map(|expr| GeneratedColumn {
        expr,
        // The supported surface spells generated columns `STORED`; the bare and
        // `VIRTUAL` forms are quarantined (the reparse path requires `STORED`).
        storage: Some(GeneratedColumnStorage::Stored),
        // The generic reparse path expects the standard `GENERATED ALWAYS AS` spelling;
        // the keywordless shorthand is a dialect-gated surface exercised elsewhere.
        spelling: GeneratedColumnSpelling::GeneratedAlways,
        meta: meta(),
    })
}

fn arb_identity_column() -> impl Strategy<Value = IdentityColumn<NoExt>> {
    (arb_identity_generation(), arb_identity_options()).prop_map(|(generation, options)| {
        IdentityColumn {
            generation,
            options: options.into_iter().collect(),
            meta: meta(),
        }
    })
}

fn arb_identity_generation() -> impl Strategy<Value = IdentityGeneration> {
    prop_oneof![
        Just(IdentityGeneration::Always),
        Just(IdentityGeneration::ByDefault),
    ]
}

/// Identity sequence options, at most one of each kind so a duplicate option never
/// appears, emitted in the canonical order. `MinValue`/`MaxValue` carry the
/// `NO MINVALUE` vs `MINVALUE <n>` distinction via the inner option.
fn arb_identity_options() -> impl Strategy<Value = Vec<IdentityOption<NoExt>>> {
    (
        any::<bool>(),
        any::<bool>(),
        prop::option::of(prop::option::of(Just(()))),
        prop::option::of(prop::option::of(Just(()))),
        any::<bool>(),
        prop::option::of(any::<bool>()),
    )
        .prop_map(|(start, increment, min_value, max_value, cache, cycle)| {
            let start = start.then(|| IdentityOption::StartWith {
                expr: integer_literal(),
                meta: meta(),
            });
            let increment = increment.then(|| IdentityOption::IncrementBy {
                expr: integer_literal(),
                meta: meta(),
            });
            let min_value = min_value.map(|inner| IdentityOption::MinValue {
                value: inner.map(|()| integer_literal()),
                meta: meta(),
            });
            let max_value = max_value.map(|inner| IdentityOption::MaxValue {
                value: inner.map(|()| integer_literal()),
                meta: meta(),
            });
            let cache = cache.then(|| IdentityOption::Cache {
                expr: integer_literal(),
                meta: meta(),
            });
            let cycle = cycle.map(|cycle| IdentityOption::Cycle {
                cycle,
                meta: meta(),
            });
            [start, increment, min_value, max_value, cache, cycle]
                .into_iter()
                .flatten()
                .collect()
        })
}

fn arb_table_constraint_def() -> impl Strategy<Value = squonk_ast::TableConstraintDef<NoExt>> {
    (prop::option::of(arb_column_name()), arb_table_constraint()).prop_map(|(name, constraint)| {
        squonk_ast::TableConstraintDef {
            name,
            constraint,
            no_inherit: false,
            not_valid: false,
            characteristics: None,
            meta: meta(),
        }
    })
}

fn arb_table_constraint() -> impl Strategy<Value = TableConstraint<NoExt>> {
    prop_oneof![
        arb_column_name_list().prop_map(|columns| TableConstraint::PrimaryKey {
            columns: columns.into_iter().map(bare_index_column).collect(),
            include: ThinVec::new(),
            meta: meta(),
        }),
        arb_column_name_list().prop_map(|columns| TableConstraint::Unique {
            columns: columns.into_iter().map(bare_index_column).collect(),
            nulls_not_distinct: None,
            include: ThinVec::new(),
            meta: meta(),
        }),
        arb_predicate().prop_map(|expr| TableConstraint::Check {
            expr: Box::new(expr),
            meta: meta(),
        }),
        (arb_column_name_list(), arb_foreign_key_ref()).prop_map(|(columns, references)| {
            TableConstraint::ForeignKey {
                columns: columns.into_iter().collect(),
                references: Box::new(references),
                meta: meta(),
            }
        }),
    ]
}

fn arb_foreign_key_ref() -> impl Strategy<Value = squonk_ast::ForeignKeyRef> {
    (
        prop::collection::vec(arb_column_name(), 0..3),
        arb_foreign_key_match(),
        // The `SET NULL`/`SET DEFAULT` column list is legal only on `ON DELETE`, so
        // the `ON UPDATE` generator is column-free to keep the tree reparseable.
        arb_referential_action_opt(true),
        arb_referential_action_opt(false),
    )
        .prop_map(
            |(columns, match_type, on_delete, on_update)| squonk_ast::ForeignKeyRef {
                table: object_name(&[TABLE_SYM]),
                columns: columns.into_iter().collect(),
                match_type,
                on_delete,
                on_update,
                update_before_delete: false,
                meta: meta(),
            },
        )
}

fn arb_foreign_key_match() -> impl Strategy<Value = Option<squonk_ast::ForeignKeyMatch>> {
    use squonk_ast::ForeignKeyMatch;
    // `MATCH PARTIAL` is omitted: the parser rejects it to match PostgreSQL ("MATCH
    // PARTIAL not yet implemented"), so it does not survive this render -> reparse
    // round-trip. Its rejection is exercised by the PostgreSQL accept/reject differential.
    prop_oneof![
        Just(None),
        Just(Some(ForeignKeyMatch::Full)),
        Just(Some(ForeignKeyMatch::Simple)),
    ]
}

fn arb_referential_action_opt(
    allow_columns: bool,
) -> impl Strategy<Value = Option<Box<squonk_ast::ReferentialAction>>> {
    use squonk_ast::ReferentialAction;
    // `0..=0` forces an empty list when columns are disallowed, so the same builder
    // serves both `ON DELETE` (lists allowed) and `ON UPDATE` (lists forbidden).
    let max_columns = if allow_columns { 2 } else { 0 };
    prop_oneof![
        Just(None),
        (
            0u8..5,
            prop::collection::vec(arb_column_name(), 0..=max_columns)
        )
            .prop_map(|(tag, cols)| {
                let columns = cols.into_iter().collect();
                Some(Box::new(match tag {
                    0 => ReferentialAction::NoAction { meta: meta() },
                    1 => ReferentialAction::Restrict { meta: meta() },
                    2 => ReferentialAction::Cascade { meta: meta() },
                    3 => ReferentialAction::SetNull {
                        columns,
                        meta: meta(),
                    },
                    _ => ReferentialAction::SetDefault {
                        columns,
                        meta: meta(),
                    },
                }))
            }),
    ]
}

fn arb_with_option() -> impl Strategy<Value = CreateTableOption<NoExt>> {
    prop::collection::vec(arb_storage_parameter(), 1..3).prop_map(|params| CreateTableOption {
        kind: CreateTableOptionKind::With {
            params: params.into_iter().collect(),
            meta: meta(),
        },
        meta: meta(),
    })
}

fn arb_storage_parameter() -> impl Strategy<Value = TableStorageParameter<NoExt>> {
    (arb_column_name(), prop::option::of(Just(integer_literal()))).prop_map(|(name, value)| {
        TableStorageParameter {
            name: ObjectName(thin_vec![name]),
            value,
            meta: meta(),
        }
    })
}

pub(crate) fn on_commit_option(action: OnCommitAction) -> CreateTableOption<NoExt> {
    CreateTableOption {
        kind: CreateTableOptionKind::OnCommit {
            action,
            meta: meta(),
        },
        meta: meta(),
    }
}

fn arb_on_commit_action() -> impl Strategy<Value = OnCommitAction> {
    prop_oneof![
        Just(OnCommitAction::PreserveRows),
        Just(OnCommitAction::DeleteRows),
        Just(OnCommitAction::Drop),
    ]
}

fn arb_tablespace_option() -> impl Strategy<Value = CreateTableOption<NoExt>> {
    Just(CreateTableOption {
        kind: CreateTableOptionKind::Tablespace {
            tablespace: ident(ALIAS_SYM),
            meta: meta(),
        },
        meta: meta(),
    })
}

// ---------------------------------------------------------------------------
// INSERT / UPDATE / DELETE
// ---------------------------------------------------------------------------

fn arb_insert() -> impl Strategy<Value = Insert<NoExt>> {
    (
        prop::option::of(arb_with()),
        arb_insert_target(),
        arb_insert_overriding_and_source(),
    )
        .prop_map(|(with, mut target, (overriding, source))| {
            // PostgreSQL attaches a target column list only to a `VALUES`/query source;
            // a list on `DEFAULT VALUES` is a syntax error the parser now rejects, so it
            // cannot round-trip. Drop the list for that source to keep the generated tree
            // inside the accepted subset (the rejection is covered by the differential).
            if matches!(source, InsertSource::DefaultValues { .. }) {
                target.columns = ThinVec::new();
            }
            Insert {
                // The round-trip generators only emit standard `INSERT` (the MySQL
                // `REPLACE` spelling is outside the ANSI reparse subset).
                verb: InsertVerb::Insert,
                // The SQLite `OR <action>` prefix is dialect-gated outside the ANSI
                // reparse path; quarantined as `None`.
                or_action: None,
                column_matching: None,
                with,
                target,
                overriding,
                source,
                // The MySQL row alias, upsert, and returning clauses are dialect-gated
                // outside the ANSI reparse path; quarantined here as `None`.
                row_alias: None,
                upsert: None,
                returning: None,
                meta: meta(),
            }
        })
        .boxed()
}

fn arb_insert_target() -> impl Strategy<Value = InsertTarget> {
    (
        prop::option::of(Just(ident(ALIAS_SYM))),
        prop::collection::vec(arb_column_name(), 0..3),
    )
        .prop_map(|(alias, columns)| InsertTarget {
            name: object_name(&[TABLE_SYM]),
            alias,
            alias_spelling: AliasSpelling::As,
            columns: columns.into_iter().collect(),
            meta: meta(),
        })
}

/// `OVERRIDING` only attaches to a `VALUES`/query source, so `DEFAULT VALUES` is
/// generated without it.
fn arb_insert_overriding_and_source()
-> impl Strategy<Value = (Option<InsertOverriding>, InsertSource<NoExt>)> {
    prop_oneof![
        Just((
            None,
            InsertSource::DefaultValues {
                default: DefaultValue { meta: meta() },
                meta: meta(),
            },
        )),
        (
            prop::option::of(arb_insert_overriding()),
            arb_insert_values().prop_map(|values| InsertSource::Values {
                values: Box::new(values),
                meta: meta(),
            }),
        ),
        (
            prop::option::of(arb_insert_overriding()),
            // The query source must render as a `SELECT` (not a bare `VALUES`): the
            // parser canonicalizes `INSERT ... VALUES (...)` to `InsertSource::Values`,
            // so a `Query` wrapping a `VALUES` body would not round-trip.
            arb_select().prop_map(|select| InsertSource::Query {
                query: Box::new(query_of(set_select(select))),
                meta: meta(),
            }),
        ),
    ]
}

fn arb_insert_overriding() -> impl Strategy<Value = InsertOverriding> {
    prop_oneof![
        Just(InsertOverriding::SystemValue),
        Just(InsertOverriding::UserValue),
    ]
}

fn arb_insert_values() -> impl Strategy<Value = InsertValues<NoExt>> {
    prop::collection::vec(prop::collection::vec(arb_insert_value(), 1..3), 1..3).prop_map(|rows| {
        InsertValues {
            rows: rows
                .into_iter()
                .map(|row| row.into_iter().collect())
                .collect(),
            meta: meta(),
        }
    })
}

fn arb_insert_value() -> impl Strategy<Value = InsertValue<NoExt>> {
    prop_oneof![
        arb_scalar_expr().prop_map(|expr| InsertValue::Expr { expr, meta: meta() }),
        Just(InsertValue::Default {
            default: DefaultValue { meta: meta() },
            meta: meta(),
        }),
    ]
}

fn arb_update() -> impl Strategy<Value = Update<NoExt>> {
    (
        prop::option::of(arb_with()),
        arb_dml_target(),
        prop::collection::vec(arb_update_assignment(), 1..3),
        prop::collection::vec(arb_table_with_joins(), 0..2),
        prop::option::of(arb_where_selection()),
    )
        .prop_map(|(with, target, assignments, from, selection)| Update {
            with,
            // The SQLite `OR <action>` prefix is dialect-gated outside the ANSI reparse
            // path; quarantined as `None`.
            or_action: None,
            target,
            assignments: assignments.into_iter().collect(),
            from: from.into_iter().collect(),
            selection,
            // The MySQL `ORDER BY`/`LIMIT` tails are dialect-gated outside the ANSI
            // reparse path; quarantined as empty/`None`.
            order_by: ThinVec::new(),
            limit: None,
            returning: None,
            meta: meta(),
        })
        .boxed()
}

fn arb_delete() -> impl Strategy<Value = Delete<NoExt>> {
    (
        prop::option::of(arb_with()),
        arb_dml_target(),
        prop::collection::vec(arb_table_with_joins(), 0..2),
        prop::option::of(arb_where_selection()),
    )
        .prop_map(|(with, target, using, selection)| Delete {
            with,
            target,
            using: using.into_iter().collect(),
            selection,
            // The MySQL `ORDER BY`/`LIMIT` tails are dialect-gated outside the ANSI
            // reparse path; quarantined as empty/`None`.
            order_by: ThinVec::new(),
            limit: None,
            returning: None,
            meta: meta(),
        })
        .boxed()
}

fn arb_dml_target() -> impl Strategy<Value = DmlTarget> {
    prop::option::of(Just(ident(ALIAS_SYM))).prop_map(|alias| DmlTarget {
        name: object_name(&[TABLE_SYM]),
        // The `ONLY`/`*` inheritance markers are PostgreSQL syntax gated out of the
        // ANSI reparse path; quarantined to the plain spelling.
        inheritance: RelationInheritance::Plain,
        alias,
        alias_spelling: AliasSpelling::As,
        meta: meta(),
    })
}

/// The `WHERE` filter on an `UPDATE`/`DELETE`. `WHERE CURRENT OF <cursor>` is
/// positioned-update syntax gated out of the ANSI reparse path, so only the
/// ordinary condition form is generated.
fn arb_where_selection() -> impl Strategy<Value = DmlSelection<NoExt>> {
    arb_predicate().prop_map(|condition| DmlSelection::Where {
        condition,
        meta: meta(),
    })
}

fn arb_update_assignment() -> impl Strategy<Value = UpdateAssignment<NoExt>> {
    // Only the single-column `SET col = value` form is generated. The multi-column
    // `SET (cols) = <source>` assignment (SQL feature T641, `UpdateAssignment::Tuple`)
    // is dialect-gated and the ANSI reparse path used by the oracle rejects the
    // parenthesized target list, so it is quarantined here.
    (arb_assignment_target(), arb_update_value()).prop_map(|(target, value)| {
        UpdateAssignment::Single {
            target,
            value,
            meta: meta(),
        }
    })
}

fn arb_assignment_target() -> impl Strategy<Value = ObjectName> {
    prop_oneof![
        Just(object_name(&[1])),
        Just(object_name(&[2])),
        Just(object_name(&[3])),
    ]
}

fn arb_update_value() -> impl Strategy<Value = UpdateValue<NoExt>> {
    prop_oneof![
        arb_scalar_expr().prop_map(|expr| UpdateValue::Expr { expr, meta: meta() }),
        Just(UpdateValue::Default {
            default: DefaultValue { meta: meta() },
            meta: meta(),
        }),
    ]
}

fn arb_column_name() -> impl Strategy<Value = Ident> {
    prop_oneof![Just(ident(1)), Just(ident(2)), Just(ident(3))]
}

fn arb_column_name_list() -> impl Strategy<Value = Vec<Ident>> {
    prop::collection::vec(arb_column_name(), 1..3)
}

/// Render one generated statement in `mode`.
pub fn render_generated(statement: &Statement<NoExt>, mode: RenderMode) -> String {
    let config = RenderConfig {
        mode,
        ..RenderConfig::default()
    };
    let ctx = RenderCtx::new(&GENERATED_RESOLVER, "", &config);
    statement.displayed(&ctx).to_string()
}
