// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! `Fuzz*` builder vocabulary: the `arbitrary`-generated legal-AST subset and its
//! lowering into `squonk_ast` nodes. The fuzz entry points at the module root
//! decode an arbitrary buffer into a [`FuzzStatement`] and drive it through the sole
//! root->builders seam, [`FuzzStatement::into_statement`].

use arbitrary::Arbitrary;
use squonk_ast::{
    AliasSpelling, BinaryOperator, BooleanTypeName, ColumnConstraint, ColumnDef, ColumnOption,
    CreateTable, CreateTableBody, CreateTableOption, CreateTableOptionKind, Cte, CteBody, DataType,
    DefaultValue, Delete, DmlSelection, DmlTarget, DoubleTypeName, EqualsSpelling, Expr,
    FetchSpelling, ForeignKeyMatch, ForeignKeyRef, GeneratedColumn, GeneratedColumnSpelling,
    GeneratedColumnStorage, GroupByItem, Ident, IdentityColumn, IdentityGeneration, IdentityOption,
    IndexColumn, Insert, InsertOverriding, InsertSource, InsertTarget, InsertValue, InsertValues,
    InsertVerb, IntegerTypeName, IntervalFields, Join, JoinConstraint, JoinOperator, Limit,
    LimitSyntax, LiteralKind, ModuloSpelling, NamedWindow, NoExt, NotEqSpelling, ObjectName,
    OnCommitAction, OrderByExpr, Query, ReferentialAction, RelationInheritance, Select,
    SelectDistinct, SelectItem, SelectSpelling, SetExpr, SetOperator, SetQuantifier, Statement,
    TableConstraint, TableConstraintDef, TableElement, TableStorageParameter, TableWithJoins,
    TemporaryTableKind, TextTypeName, TimeZone, UnaryOperator, Update, UpdateAssignment,
    UpdateValue, Values, ValuesItem, WindowDefinition, WindowFrame, WindowFrameBound,
    WindowFrameExclusion, WindowFrameUnits, WindowSpec, With,
};
use thin_vec::{ThinVec, thin_vec};

use crate::properties::{
    binary, frame_current_row, frame_following, frame_preceding, frame_unbounded_following,
    frame_unbounded_preceding, function_call, function_expr, ident as ident_sym,
    literal_expr as literal, meta, object_name as object_name_sym, on_commit_option, query_of,
    set_select, set_values, table_factor, unary, values_item_expr,
};

/// One generated statement across the families the proptest oracle covers
/// (`conformance/src/properties.rs::arb_statement`). The query arm is the original
/// M1 subset; the DDL/DML arms mirror the proptest legal-subset growth so the bolero
/// round-trip exercises the same surface (ADR-0014). The non-query families are
/// outside the M1 structural corpus, so `structurally_pg_comparable` excludes them
/// from the PostgreSQL structural differential — accept/reject parity still covers
/// them, and every shape here stays legal PostgreSQL (it mirrors the parity-tested
/// `pg.rs` DDL/DML corpus).
// These generator enums carry a large `FuzzSelect` arm beside much smaller ones. Their
// in-memory size does not matter — one value is built per fuzz iteration, lowered to an
// AST, and dropped — and boxing the large arm would force the whole hand-rolled harness
// off its uniform `Copy` + derived-`Arbitrary` shape, so the size-difference lint is
// allowed here and on the sibling source/operand enums below.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzStatement {
    Query(FuzzQuery),
    CreateTable(FuzzCreateTable),
    Insert(FuzzInsert),
    Update(FuzzUpdate),
    Delete(FuzzDelete),
}

impl FuzzStatement {
    pub(crate) fn into_statement(self) -> Statement<NoExt> {
        match self {
            Self::Query(query) => query.into_statement(),
            Self::CreateTable(create) => Statement::CreateTable {
                create: Box::new(create.into_create_table()),
                meta: meta(),
            },
            Self::Insert(insert) => Statement::Insert {
                insert: Box::new(insert.into_insert()),
                meta: meta(),
            },
            Self::Update(update) => Statement::Update {
                update: Box::new(update.into_update()),
                meta: meta(),
            },
            Self::Delete(delete) => Statement::Delete {
                delete: Box::new(delete.into_delete()),
                meta: meta(),
            },
        }
    }
}

/// A top-level query statement: an optional `WITH`, a set expression over
/// SELECT/VALUES operands, `ORDER BY`, and `LIMIT` — the `arb_query_statement` shape.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzQuery {
    pub(crate) with: Option<FuzzWith>,
    pub(crate) first: FuzzSetOperand,
    pub(crate) set0: Option<FuzzSetTail>,
    pub(crate) set1: Option<FuzzSetTail>,
    pub(crate) order0: Option<FuzzOrderBy>,
    pub(crate) order1: Option<FuzzOrderBy>,
    pub(crate) limit: FuzzLimit,
}

impl FuzzQuery {
    pub(crate) fn into_statement(self) -> Statement<NoExt> {
        Statement::Query {
            query: Box::new(self.into_query()),
            meta: meta(),
        }
    }

    fn into_query(self) -> Query<NoExt> {
        let mut body = self.first.into_set_expr();
        for tail in [self.set0, self.set1].into_iter().flatten() {
            body = SetExpr::SetOperation {
                op: tail.op.into_operator(),
                all: tail.all,
                // `BY NAME` is DuckDB-only; this harness reparses under ANSI, which
                // rejects it, so generated set operations stay positional.
                by_name: false,
                left: Box::new(body),
                right: Box::new(tail.operand.into_set_expr()),
                meta: meta(),
            };
        }

        let mut order_by = ThinVec::new();
        for item in [self.order0, self.order1].into_iter().flatten() {
            order_by.push(item.into_order_by());
        }

        Query {
            with: self.with.map(FuzzWith::into_with),
            body,
            order_by,
            order_by_all: None,
            limit_by: None,
            limit: self.limit.into_limit(),
            // Row-locking clauses are a narrow dialect-gated tail with dedicated
            // coverage; the fuzzer leaves them empty.
            settings: ThinVec::new(),
            format: None,
            locking: ThinVec::new(),
            pipe_operators: ThinVec::new(),
            for_clause: None,
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzSetTail {
    pub(crate) op: FuzzSetOperator,
    pub(crate) all: bool,
    pub(crate) operand: FuzzSetOperand,
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzSelect {
    pub(crate) distinct: bool,
    pub(crate) projection0: FuzzSelectItem,
    pub(crate) projection1: Option<FuzzSelectItem>,
    pub(crate) projection2: Option<FuzzSelectItem>,
    pub(crate) from0: Option<FuzzTableWithJoins>,
    pub(crate) from1: Option<FuzzTableWithJoins>,
    pub(crate) selection: Option<FuzzPredicate>,
    pub(crate) group0: Option<FuzzScalar>,
    pub(crate) group1: Option<FuzzScalar>,
    pub(crate) having: Option<FuzzPredicate>,
    pub(crate) windows: FuzzNamedWindows,
}

impl FuzzSelect {
    fn into_select(self) -> Select<NoExt> {
        let mut projection = thin_vec![self.projection0.into_select_item()];
        for item in [self.projection1, self.projection2].into_iter().flatten() {
            projection.push(item.into_select_item());
        }

        let mut from = ThinVec::new();
        for table in [self.from0, self.from1].into_iter().flatten() {
            from.push(table.into_table_with_joins());
        }

        let mut group_by = ThinVec::new();
        for expr in [self.group0, self.group1].into_iter().flatten() {
            // The fuzzer synthesizes only plain-expression GROUP BY items; the
            // grouping-set constructs (`ROLLUP`/`CUBE`/`GROUPING SETS`/`()`) have
            // dedicated differential and round-trip coverage.
            group_by.push(GroupByItem::Expr {
                expr: expr.into_expr(),
                meta: meta(),
            });
        }

        Select {
            // Generate only "no quantifier" and `DISTINCT`; the explicit-`ALL` and
            // `DISTINCT ON` forms are covered by parser/render unit tests.
            distinct: self.distinct.then(|| SelectDistinct::Quantifier {
                quantifier: SetQuantifier::Distinct,
                meta: meta(),
            }),
            // STRAIGHT_JOIN is a narrow MySQL surface tag with dedicated coverage; the
            // fuzzer leaves it unset.
            straight_join: false,
            projection,
            // The fuzzer does not synthesize the dialect-gated `SELECT … INTO`
            // create-table target; round-trip for it has dedicated coverage.
            into: None,
            from,
            // The fuzzer does not synthesize the dialect-gated Hive/Spark LATERAL VIEW
            // clause (the `QUALIFY` precedent); its round-trip has dedicated coverage.
            lateral_views: ThinVec::new(),
            selection: self.selection.map(FuzzPredicate::into_expr),
            // The fuzzer does not synthesize the dialect-gated Oracle-style CONNECT BY
            // hierarchical clause; its round-trip has dedicated coverage.
            connect_by: None,
            group_by,
            group_by_quantifier: None,
            group_by_all: None,
            having: self.having.map(FuzzPredicate::into_expr),
            // A `WINDOW name AS (...)` clause; a windowed select is excluded from the
            // PostgreSQL structural differential by `select.windows.is_empty()` in
            // `structurally_pg_comparable`, so generating it stays differential-safe
            // (revisit if the PG structural mapping later covers windows).
            windows: self.windows.into_named_windows(),
            // The fuzzer does not synthesize the dialect-gated `QUALIFY` clause (the
            // `SELECT … INTO` precedent); its round-trip has dedicated coverage.
            qualify: None,
            // The fuzzer does not synthesize the dialect-gated `USING SAMPLE` clause
            // (the `QUALIFY` precedent); its round-trip has dedicated coverage.
            sample: None,
            // The generator emits ordinary `SELECT` bodies; the `TABLE`-command
            // spelling has dedicated parser/render/differential coverage.
            spelling: SelectSpelling::Select,
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzSelectItem {
    Wildcard,
    QualifiedWildcard(FuzzObjectName),
    Expr {
        expr: FuzzScalar,
        alias: Option<FuzzName>,
    },
    // A window-function call (`f(...) OVER ...`) is legal only in the projection and
    // `ORDER BY`, so it is seeded here rather than inside `FuzzScalar`.
    Window(FuzzWindowFunction),
}

impl FuzzSelectItem {
    fn into_select_item(self) -> SelectItem<NoExt> {
        match self {
            Self::Wildcard => SelectItem::Wildcard {
                options: None,
                alias: None,
                alias_spelling: AliasSpelling::As,
                meta: meta(),
            },
            Self::QualifiedWildcard(name) => SelectItem::QualifiedWildcard {
                name: name.into_object_name(),
                options: None,
                alias: None,
                alias_spelling: AliasSpelling::As,
                meta: meta(),
            },
            Self::Expr { expr, alias } => SelectItem::Expr {
                expr: expr.into_expr(),
                alias: alias.map(ident),
                alias_spelling: AliasSpelling::As,
                meta: meta(),
            },
            Self::Window(call) => SelectItem::Expr {
                expr: call.into_expr(),
                alias: None,
                alias_spelling: AliasSpelling::As,
                meta: meta(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzTableWithJoins {
    pub(crate) table: FuzzObjectName,
    pub(crate) alias: Option<FuzzName>,
    pub(crate) join0: Option<FuzzJoin>,
    pub(crate) join1: Option<FuzzJoin>,
}

impl FuzzTableWithJoins {
    fn into_table_with_joins(self) -> TableWithJoins<NoExt> {
        let mut joins = ThinVec::new();
        for join in [self.join0, self.join1].into_iter().flatten() {
            joins.push(join.into_join());
        }

        TableWithJoins {
            relation: table_factor(self.table.into_object_name(), self.alias.map(ident)),
            joins,
            meta: meta(),
        }
    }
}

/// A join: a table factor plus an operator. The constrained operators
/// (`INNER`/`LEFT`/`RIGHT`/`FULL`) each carry an `ON`/`USING`/`NATURAL` constraint
/// independent of the operator (so `LEFT JOIN ... USING (a)` and `FULL JOIN ...
/// NATURAL` are generated, not just the inner forms); `CROSS JOIN` takes none.
/// Mirrors `arb_join` over `arb_join_constraint`.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzJoin {
    pub(crate) operator: FuzzJoinOperator,
    pub(crate) table: FuzzObjectName,
    pub(crate) alias: Option<FuzzName>,
}

impl FuzzJoin {
    fn into_join(self) -> Join<NoExt> {
        Join {
            relation: table_factor(self.table.into_object_name(), self.alias.map(ident)),
            operator: self.operator.into_operator(),
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzJoinOperator {
    Inner(FuzzJoinConstraint),
    LeftOuter(FuzzJoinConstraint),
    RightOuter(FuzzJoinConstraint),
    FullOuter(FuzzJoinConstraint),
    Cross,
}

impl FuzzJoinOperator {
    fn into_operator(self) -> JoinOperator<NoExt> {
        match self {
            Self::Inner(constraint) => JoinOperator::Inner {
                straight: false,
                inner: false,
                constraint: constraint.into_constraint(),
                meta: meta(),
            },
            Self::LeftOuter(constraint) => JoinOperator::LeftOuter {
                outer: false,
                constraint: constraint.into_constraint(),
                meta: meta(),
            },
            Self::RightOuter(constraint) => JoinOperator::RightOuter {
                outer: false,
                constraint: constraint.into_constraint(),
                meta: meta(),
            },
            Self::FullOuter(constraint) => JoinOperator::FullOuter {
                outer: false,
                constraint: constraint.into_constraint(),
                meta: meta(),
            },
            Self::Cross => JoinOperator::Cross { meta: meta() },
        }
    }
}

/// A constraint on a non-cross join: `ON <predicate>`, `USING (<columns>)`, or
/// `NATURAL` (`arb_join_constraint`). `USING (...) AS alias` is a PostgreSQL extension
/// the ANSI reparse path used by the oracle does not accept, so the alias stays unset.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzJoinConstraint {
    On(FuzzPredicate),
    Using {
        col0: FuzzColumnName,
        col1: Option<FuzzColumnName>,
    },
    Natural,
}

impl FuzzJoinConstraint {
    fn into_constraint(self) -> JoinConstraint<NoExt> {
        match self {
            Self::On(predicate) => JoinConstraint::On {
                expr: predicate.into_expr(),
                meta: meta(),
            },
            Self::Using { col0, col1 } => {
                let mut columns = thin_vec![ident_sym(col0.symbol())];
                if let Some(col) = col1 {
                    columns.push(ident_sym(col.symbol()));
                }
                JoinConstraint::Using {
                    columns,
                    alias: None,
                    meta: meta(),
                }
            }
            Self::Natural => JoinConstraint::Natural { meta: meta() },
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzOrderBy {
    pub(crate) expr: FuzzScalar,
    pub(crate) asc: Option<bool>,
    pub(crate) nulls_first: Option<bool>,
}

impl FuzzOrderBy {
    fn into_order_by(self) -> OrderByExpr<NoExt> {
        OrderByExpr {
            expr: self.expr.into_expr(),
            asc: self.asc,
            using: None,
            nulls_first: self.nulls_first,
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzLimit {
    None,
    Limit(FuzzScalar),
    Offset(FuzzScalar),
    LimitOffset {
        limit: FuzzScalar,
        offset: FuzzScalar,
    },
}

impl FuzzLimit {
    fn into_limit(self) -> Option<Limit<NoExt>> {
        match self {
            Self::None => None,
            Self::Limit(limit) => Some(limit_clause(Some(limit.into_expr()), None)),
            Self::Offset(offset) => Some(limit_clause(None, Some(offset.into_expr()))),
            Self::LimitOffset { limit, offset } => Some(limit_clause(
                Some(limit.into_expr()),
                Some(offset.into_expr()),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzPredicate {
    pub(crate) first: FuzzComparison,
    pub(crate) step0: Option<FuzzLogicStep>,
    pub(crate) step1: Option<FuzzLogicStep>,
    pub(crate) negated: bool,
}

impl FuzzPredicate {
    fn into_expr(self) -> Expr<NoExt> {
        let mut expr = self.first.into_expr();
        for step in [self.step0, self.step1].into_iter().flatten() {
            expr = binary(expr, step.op.into_operator(), step.rhs.into_expr());
        }
        if self.negated {
            unary(UnaryOperator::Not, expr)
        } else {
            expr
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzLogicStep {
    pub(crate) op: FuzzLogicOperator,
    pub(crate) rhs: FuzzComparison,
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzComparison {
    pub(crate) left: FuzzScalar,
    pub(crate) op: FuzzComparisonOperator,
    pub(crate) right: FuzzScalar,
}

impl FuzzComparison {
    fn into_expr(self) -> Expr<NoExt> {
        binary(
            self.left.into_expr(),
            self.op.into_operator(),
            self.right.into_expr(),
        )
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzScalar {
    pub(crate) first: FuzzAtom,
    pub(crate) step0: Option<FuzzScalarStep>,
    pub(crate) step1: Option<FuzzScalarStep>,
    pub(crate) step2: Option<FuzzScalarStep>,
    pub(crate) unary: Option<FuzzSignOperator>,
}

impl FuzzScalar {
    fn into_expr(self) -> Expr<NoExt> {
        let mut expr = self.first.into_expr();
        for step in [self.step0, self.step1, self.step2].into_iter().flatten() {
            expr = binary(expr, step.op.into_operator(), step.rhs.into_expr());
        }
        match self.unary {
            Some(op) => unary(op.into_operator(), expr),
            None => expr,
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzScalarStep {
    pub(crate) op: FuzzScalarOperator,
    pub(crate) rhs: FuzzAtom,
}

/// The base of a scalar expression: a column, a literal, or a plain (non-window)
/// function call (`arb_scalar_expr`'s base set). A plain call's arguments are columns
/// or literals, kept shallow so the scalar grammar does not recurse through calls.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzAtom {
    Column(FuzzObjectName),
    Literal(FuzzLiteral),
    Function(FuzzPlainFunction),
}

impl FuzzAtom {
    fn into_expr(self) -> Expr<NoExt> {
        match self {
            Self::Column(name) => Expr::Column {
                name: name.into_object_name(),
                meta: meta(),
            },
            Self::Literal(literal) => literal.into_expr(),
            Self::Function(call) => call.into_expr(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzObjectName {
    pub(crate) head: FuzzName,
    pub(crate) tail: Option<FuzzName>,
}

impl FuzzObjectName {
    fn into_object_name(self) -> ObjectName {
        let mut parts = thin_vec![ident(self.head)];
        if let Some(tail) = self.tail {
            parts.push(ident(tail));
        }
        ObjectName(parts)
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzName {
    A,
    B,
    C,
    T,
    X,
}

impl FuzzName {
    const fn symbol(self) -> u32 {
        match self {
            Self::A => 1,
            Self::B => 2,
            Self::C => 3,
            Self::T => 4,
            Self::X => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzSetOperator {
    Union,
    Intersect,
    Except,
}

impl FuzzSetOperator {
    const fn into_operator(self) -> SetOperator {
        match self {
            Self::Union => SetOperator::Union,
            Self::Intersect => SetOperator::Intersect,
            Self::Except => SetOperator::Except,
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzScalarOperator {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    StringConcat,
}

impl FuzzScalarOperator {
    const fn into_operator(self) -> BinaryOperator {
        match self {
            Self::Plus => BinaryOperator::Plus,
            Self::Minus => BinaryOperator::Minus,
            Self::Multiply => BinaryOperator::Multiply,
            Self::Divide => BinaryOperator::Divide,
            Self::Modulo => BinaryOperator::Modulo(ModuloSpelling::Percent),
            Self::StringConcat => BinaryOperator::StringConcat,
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzComparisonOperator {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

impl FuzzComparisonOperator {
    const fn into_operator(self) -> BinaryOperator {
        match self {
            Self::Eq => BinaryOperator::Eq(EqualsSpelling::Single),
            Self::NotEq => BinaryOperator::NotEq(NotEqSpelling::AngleBracket),
            Self::Lt => BinaryOperator::Lt,
            Self::LtEq => BinaryOperator::LtEq,
            Self::Gt => BinaryOperator::Gt,
            Self::GtEq => BinaryOperator::GtEq,
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzLogicOperator {
    And,
    Or,
}

impl FuzzLogicOperator {
    const fn into_operator(self) -> BinaryOperator {
        match self {
            Self::And => BinaryOperator::And,
            Self::Or => BinaryOperator::Or,
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzSignOperator {
    Plus,
    Minus,
}

impl FuzzSignOperator {
    const fn into_operator(self) -> UnaryOperator {
        match self {
            Self::Plus => UnaryOperator::Plus,
            Self::Minus => UnaryOperator::Minus,
        }
    }
}

// ---------------------------------------------------------------------------
// `WITH`, set operands, and `VALUES`
// ---------------------------------------------------------------------------

/// The statement-level `WITH` clause. The CTE shape is fixed — `t(a) AS [NOT]
/// MATERIALIZED (VALUES (1))`, mirroring `arb_with` — so only the `RECURSIVE` flag
/// and the materialization hint vary. A query carrying a `WITH` clause is excluded
/// from the PostgreSQL structural differential (`query_pg_comparable` requires
/// `with.is_none()`), leaving accept/reject parity to cover it.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzWith {
    pub(crate) recursive: bool,
    pub(crate) materialized: Option<bool>,
}

impl FuzzWith {
    fn into_with(self) -> With<NoExt> {
        With {
            recursive: self.recursive,
            ctes: thin_vec![Cte {
                name: ident_sym(FuzzName::T.symbol()),
                columns: thin_vec![ident_sym(1)],
                using_key: None,
                materialized: self.materialized,
                body: CteBody::Query {
                    query: Box::new(query_of(set_values(Values {
                        explicit_row: false,
                        rows: thin_vec![thin_vec![values_item_expr(integer_literal())]],
                        meta: meta(),
                    }))),
                    meta: meta(),
                },
                search: None,
                cycle: None,
                meta: meta(),
            }],
            meta: meta(),
        }
    }
}

/// A set-expression operand: a `SELECT` or a `VALUES` body (`arb_set_operand`).
#[allow(clippy::large_enum_variant)] // see the FuzzStatement note above
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzSetOperand {
    Select(FuzzSelect),
    Values(FuzzValues),
}

impl FuzzSetOperand {
    fn into_set_expr(self) -> SetExpr<NoExt> {
        match self {
            Self::Select(select) => set_select(select.into_select()),
            Self::Values(values) => set_values(values.into_values()),
        }
    }
}

/// A `VALUES` body: one or two rows, each of one or two items. Row widths vary
/// independently — the parser does not enforce a uniform width (that is a binder
/// check) — matching `arb_values`.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzValues {
    pub(crate) row0: FuzzValuesRow,
    pub(crate) row1: Option<FuzzValuesRow>,
}

impl FuzzValues {
    fn into_values(self) -> Values<NoExt> {
        let mut rows = thin_vec![self.row0.into_row()];
        if let Some(row) = self.row1 {
            rows.push(row.into_row());
        }
        Values {
            explicit_row: false,
            rows,
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzValuesRow {
    pub(crate) item0: FuzzValuesItem,
    pub(crate) item1: Option<FuzzValuesItem>,
}

impl FuzzValuesRow {
    fn into_row(self) -> ThinVec<ValuesItem<NoExt>> {
        let mut items = thin_vec![self.item0.into_values_item()];
        if let Some(item) = self.item1 {
            items.push(item.into_values_item());
        }
        items
    }
}

/// A `VALUES` row item: an expression or a bare `DEFAULT` (`arb_values_item`).
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzValuesItem {
    Expr(FuzzScalar),
    Default,
}

impl FuzzValuesItem {
    fn into_values_item(self) -> ValuesItem<NoExt> {
        match self {
            Self::Expr(expr) => values_item_expr(expr.into_expr()),
            Self::Default => ValuesItem::Default {
                default: DefaultValue { meta: meta() },
                meta: meta(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Literal forms
// ---------------------------------------------------------------------------

/// A literal expression (`arb_literal_expr` + `arb_temporal_literal`): the
/// integer/string/boolean/null forms plus the typed temporal literals. `FLOAT` is
/// omitted on purpose — a detached float has no backing source, so it renders as the
/// placeholder `0` and reparses as an integer, the one literal kind that cannot
/// survive the synthetic-span round-trip (the same quarantine the proptest generator
/// documents).
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzLiteral {
    Integer,
    String,
    BooleanTrue,
    BooleanFalse,
    Null,
    Date,
    Time(FuzzTimeZone),
    Timestamp(FuzzTimeZone),
    // The interval precision is omitted: the literal renderer drops it (only the
    // type-position renderer keeps it), so a `precision: Some(_)` would not survive the
    // round-trip (mirrors `arb_temporal_literal`).
    Interval(Option<FuzzIntervalFields>),
}

impl FuzzLiteral {
    fn into_expr(self) -> Expr<NoExt> {
        literal(self.into_kind())
    }

    fn into_kind(self) -> LiteralKind {
        match self {
            Self::Integer => LiteralKind::Integer,
            Self::String => LiteralKind::String,
            Self::BooleanTrue => LiteralKind::Boolean(true),
            Self::BooleanFalse => LiteralKind::Boolean(false),
            Self::Null => LiteralKind::Null,
            Self::Date => LiteralKind::Date,
            Self::Time(time_zone) => LiteralKind::Time {
                time_zone: time_zone.into_time_zone(),
            },
            Self::Timestamp(time_zone) => LiteralKind::Timestamp {
                time_zone: time_zone.into_time_zone(),
            },
            Self::Interval(fields) => LiteralKind::Interval {
                fields: fields.map(FuzzIntervalFields::into_fields),
                precision: None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzTimeZone {
    Unspecified,
    WithTimeZone,
    WithoutTimeZone,
}

impl FuzzTimeZone {
    const fn into_time_zone(self) -> TimeZone {
        match self {
            Self::Unspecified => TimeZone::Unspecified,
            Self::WithTimeZone => TimeZone::WithTimeZone,
            Self::WithoutTimeZone => TimeZone::WithoutTimeZone,
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzIntervalFields {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    YearToMonth,
    DayToHour,
    DayToMinute,
    DayToSecond,
    HourToMinute,
    HourToSecond,
    MinuteToSecond,
}

impl FuzzIntervalFields {
    const fn into_fields(self) -> IntervalFields {
        match self {
            Self::Year => IntervalFields::Year,
            Self::Month => IntervalFields::Month,
            Self::Day => IntervalFields::Day,
            Self::Hour => IntervalFields::Hour,
            Self::Minute => IntervalFields::Minute,
            Self::Second => IntervalFields::Second,
            Self::YearToMonth => IntervalFields::YearToMonth,
            Self::DayToHour => IntervalFields::DayToHour,
            Self::DayToMinute => IntervalFields::DayToMinute,
            Self::DayToSecond => IntervalFields::DayToSecond,
            Self::HourToMinute => IntervalFields::HourToMinute,
            Self::HourToSecond => IntervalFields::HourToSecond,
            Self::MinuteToSecond => IntervalFields::MinuteToSecond,
        }
    }
}

// ---------------------------------------------------------------------------
// Function calls and window (`OVER`) clauses
// ---------------------------------------------------------------------------

/// A plain (non-window) function call, legal anywhere an expression is. Mirrors the
/// three `arb_function_expr` shapes: `f(arg[, arg])`, the wildcard `f(*)`, and the
/// `f(DISTINCT arg)` aggregate.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzPlainFunction {
    Plain {
        arg0: FuzzCallArgument,
        arg1: Option<FuzzCallArgument>,
    },
    Wildcard,
    Distinct(FuzzCallArgument),
}

impl FuzzPlainFunction {
    fn into_expr(self) -> Expr<NoExt> {
        let call = match self {
            Self::Plain { arg0, arg1 } => function_call(
                object_name_sym(&[1]),
                None,
                call_arguments(arg0, arg1),
                false,
                None,
            ),
            Self::Wildcard => {
                function_call(object_name_sym(&[2]), None, ThinVec::new(), true, None)
            }
            Self::Distinct(arg) => function_call(
                object_name_sym(&[1]),
                Some(SetQuantifier::Distinct),
                thin_vec![arg.into_expr()],
                false,
                None,
            ),
        };
        function_expr(call)
    }
}

/// An argument to a generated call: a column or a literal (`arb_call_argument`).
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzCallArgument {
    Column(FuzzColumnName),
    Literal(FuzzLiteral),
}

impl FuzzCallArgument {
    fn into_expr(self) -> Expr<NoExt> {
        match self {
            Self::Column(col) => column_expr(col),
            Self::Literal(literal) => literal.into_expr(),
        }
    }
}

fn call_arguments(arg0: FuzzCallArgument, arg1: Option<FuzzCallArgument>) -> ThinVec<Expr<NoExt>> {
    let mut args = thin_vec![arg0.into_expr()];
    if let Some(arg) = arg1 {
        args.push(arg.into_expr());
    }
    args
}

/// A window-function call `f(...) OVER ...` — only legal in the projection / ORDER
/// BY. Mirrors `arb_window_function_expr`.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzWindowFunction {
    pub(crate) call: FuzzWindowCall,
    pub(crate) over: FuzzWindowSpec,
}

impl FuzzWindowFunction {
    fn into_expr(self) -> Expr<NoExt> {
        let (args, wildcard) = self.call.into_args();
        function_expr(function_call(
            object_name_sym(&[1]),
            None,
            args,
            wildcard,
            Some(self.over.into_window_spec()),
        ))
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzWindowCall {
    Wildcard,
    Args {
        arg0: FuzzCallArgument,
        arg1: Option<FuzzCallArgument>,
    },
}

impl FuzzWindowCall {
    fn into_args(self) -> (ThinVec<Expr<NoExt>>, bool) {
        match self {
            Self::Wildcard => (ThinVec::new(), true),
            Self::Args { arg0, arg1 } => (call_arguments(arg0, arg1), false),
        }
    }
}

/// An `OVER` clause: a named-window reference or an inline definition
/// (`arb_window_spec`). Parsing does not require the name to be a defined window.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzWindowSpec {
    Named,
    Inline(FuzzWindowDefinition),
}

impl FuzzWindowSpec {
    fn into_window_spec(self) -> WindowSpec<NoExt> {
        match self {
            Self::Named => WindowSpec::Named {
                name: ident_sym(FuzzName::X.symbol()),
                meta: meta(),
            },
            Self::Inline(definition) => WindowSpec::Inline {
                definition: Box::new(definition.into_window_definition()),
                meta: meta(),
            },
        }
    }
}

/// A window definition (`arb_window_definition`). Either no frame — with independently
/// optional `PARTITION BY` / `ORDER BY` — or a frame paired with exactly one ORDER BY
/// key so the RANGE/GROUPS frames stay legal (they require an ordering). A base-window
/// reference (`OVER (w ...)`) needs a coordinating WINDOW definition, so it stays unset.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzWindowDefinition {
    NoFrame {
        partition: Option<FuzzColumnName>,
        order: Option<FuzzOrderBy>,
    },
    Framed {
        partition: Option<FuzzColumnName>,
        order: FuzzOrderBy,
        frame: FuzzWindowFrame,
    },
}

impl FuzzWindowDefinition {
    fn into_window_definition(self) -> WindowDefinition<NoExt> {
        match self {
            Self::NoFrame { partition, order } => {
                window_definition(partition, order.map(FuzzOrderBy::into_order_by), None)
            }
            Self::Framed {
                partition,
                order,
                frame,
            } => window_definition(
                partition,
                Some(order.into_order_by()),
                Some(frame.into_window_frame()),
            ),
        }
    }
}

fn window_definition(
    partition: Option<FuzzColumnName>,
    order: Option<OrderByExpr<NoExt>>,
    frame: Option<WindowFrame<NoExt>>,
) -> WindowDefinition<NoExt> {
    let mut partition_by = ThinVec::new();
    if let Some(col) = partition {
        partition_by.push(column_expr(col));
    }
    let mut order_by = ThinVec::new();
    if let Some(item) = order {
        order_by.push(item);
    }
    WindowDefinition {
        existing: None,
        partition_by,
        order_by,
        frame,
        meta: meta(),
    }
}

/// A curated set of unambiguously legal frames (every bound variant and all three
/// units appear), each with an optional `EXCLUDE` (`arb_window_frame`). Offset bounds
/// (`<n> PRECEDING`/`FOLLOWING`) ride `ROWS`; RANGE/GROUPS use only the
/// unbounded/current-row extents.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzWindowFrame {
    pub(crate) shape: FuzzFrameShape,
    pub(crate) exclusion: Option<FuzzFrameExclusion>,
}

impl FuzzWindowFrame {
    fn into_window_frame(self) -> WindowFrame<NoExt> {
        let (units, start, end) = self.shape.into_parts();
        WindowFrame {
            units,
            start,
            end,
            exclusion: self.exclusion.map(FuzzFrameExclusion::into_exclusion),
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzFrameShape {
    RowsUnboundedPreceding,
    RowsCurrentRow,
    RowsUnboundedPrecedingToCurrentRow,
    RowsPrecedingToFollowing,
    RowsCurrentRowToUnboundedFollowing,
    RangeUnboundedPrecedingToUnboundedFollowing,
    GroupsUnboundedPrecedingToCurrentRow,
}

impl FuzzFrameShape {
    fn into_parts(
        self,
    ) -> (
        WindowFrameUnits,
        WindowFrameBound<NoExt>,
        Option<WindowFrameBound<NoExt>>,
    ) {
        match self {
            Self::RowsUnboundedPreceding => {
                (WindowFrameUnits::Rows, frame_unbounded_preceding(), None)
            }
            Self::RowsCurrentRow => (WindowFrameUnits::Rows, frame_current_row(), None),
            Self::RowsUnboundedPrecedingToCurrentRow => (
                WindowFrameUnits::Rows,
                frame_unbounded_preceding(),
                Some(frame_current_row()),
            ),
            Self::RowsPrecedingToFollowing => (
                WindowFrameUnits::Rows,
                frame_preceding(),
                Some(frame_following()),
            ),
            Self::RowsCurrentRowToUnboundedFollowing => (
                WindowFrameUnits::Rows,
                frame_current_row(),
                Some(frame_unbounded_following()),
            ),
            Self::RangeUnboundedPrecedingToUnboundedFollowing => (
                WindowFrameUnits::Range,
                frame_unbounded_preceding(),
                Some(frame_unbounded_following()),
            ),
            Self::GroupsUnboundedPrecedingToCurrentRow => (
                WindowFrameUnits::Groups,
                frame_unbounded_preceding(),
                Some(frame_current_row()),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzFrameExclusion {
    CurrentRow,
    Group,
    Ties,
    NoOthers,
}

impl FuzzFrameExclusion {
    const fn into_exclusion(self) -> WindowFrameExclusion {
        match self {
            Self::CurrentRow => WindowFrameExclusion::CurrentRow,
            Self::Group => WindowFrameExclusion::Group,
            Self::Ties => WindowFrameExclusion::Ties,
            Self::NoOthers => WindowFrameExclusion::NoOthers,
        }
    }
}

/// The SELECT-level `WINDOW name AS (...)` clause (`arb_named_windows`): empty, one
/// entry, or two with distinct names so a multi-entry clause does not collide.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzNamedWindows {
    None,
    One(FuzzWindowDefinition),
    Two(FuzzWindowDefinition, FuzzWindowDefinition),
}

impl FuzzNamedWindows {
    fn into_named_windows(self) -> ThinVec<NamedWindow<NoExt>> {
        match self {
            Self::None => ThinVec::new(),
            Self::One(definition) => thin_vec![named_window(4, definition)],
            Self::Two(a, b) => thin_vec![named_window(4, a), named_window(5, b)],
        }
    }
}

/// A column name drawn from `a`/`b`/`c` (`arb_column_name`).
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzColumnName {
    A,
    B,
    C,
}

impl FuzzColumnName {
    const fn symbol(self) -> u32 {
        match self {
            Self::A => 1,
            Self::B => 2,
            Self::C => 3,
        }
    }
}

/// A non-empty list of column names (`arb_column_name_list`): one or two of `a`/`b`/`c`.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzColumnList {
    pub(crate) head: FuzzColumnName,
    pub(crate) tail: Option<FuzzColumnName>,
}

impl FuzzColumnList {
    fn into_idents(self) -> ThinVec<Ident> {
        let mut idents = thin_vec![ident_sym(self.head.symbol())];
        if let Some(col) = self.tail {
            idents.push(ident_sym(col.symbol()));
        }
        idents
    }

    /// The `PRIMARY KEY`/`UNIQUE` constraint-column shape: each name as a bare-column
    /// [`IndexColumn`] (no `COLLATE`/`ASC`/`DESC`).
    fn into_index_columns(self) -> ThinVec<IndexColumn<NoExt>> {
        self.into_idents()
            .into_iter()
            .map(|name| IndexColumn {
                expr: Expr::Column {
                    name: ObjectName(thin_vec![name]),
                    meta: meta(),
                },
                asc: None,
                nulls_first: None,
                meta: meta(),
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// CREATE TABLE
// ---------------------------------------------------------------------------

/// `CREATE TABLE` (`arb_create_table`). PostgreSQL renders the post-definition
/// options in the order `WITH`, `ON COMMIT`, `TABLESPACE`, so they are emitted in that
/// order; the table name is the fixed `t`.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzCreateTable {
    pub(crate) temporary: FuzzTemporaryAndOnCommit,
    pub(crate) if_not_exists: bool,
    pub(crate) body: FuzzCreateTableBody,
    pub(crate) with_option: Option<FuzzWithOption>,
    pub(crate) tablespace: bool,
}

impl FuzzCreateTable {
    fn into_create_table(self) -> CreateTable<NoExt> {
        let (temporary, on_commit) = self.temporary.into_parts();
        let mut options = ThinVec::new();
        if let Some(with) = self.with_option {
            options.push(with.into_option());
        }
        if let Some(action) = on_commit {
            options.push(on_commit_option(action));
        }
        if self.tablespace {
            options.push(tablespace_option());
        }
        CreateTable {
            or_replace: false,
            temporary,
            unlogged: false,
            if_not_exists: self.if_not_exists,
            name: object_name_sym(&[FuzzName::T.symbol()]),
            body: self.body.into_body(),
            inherits: ThinVec::new(),
            partition_by: None,
            access_method: None,
            options,
            meta: meta(),
        }
    }
}

/// `ON COMMIT` is legal only on a temporary table, so the two are generated together
/// (`arb_temporary_and_on_commit`): a non-temporary table never carries `ON COMMIT`.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzTemporaryAndOnCommit {
    NotTemporary,
    Temporary(FuzzTemporaryKind),
    TemporaryOnCommit(FuzzTemporaryKind, FuzzOnCommitAction),
}

impl FuzzTemporaryAndOnCommit {
    fn into_parts(self) -> (Option<TemporaryTableKind>, Option<OnCommitAction>) {
        match self {
            Self::NotTemporary => (None, None),
            Self::Temporary(kind) => (Some(kind.into_kind()), None),
            Self::TemporaryOnCommit(kind, action) => {
                (Some(kind.into_kind()), Some(action.into_action()))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzTemporaryKind {
    Temp,
    Temporary,
}

impl FuzzTemporaryKind {
    const fn into_kind(self) -> TemporaryTableKind {
        match self {
            Self::Temp => TemporaryTableKind::Temp,
            Self::Temporary => TemporaryTableKind::Temporary,
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzOnCommitAction {
    PreserveRows,
    DeleteRows,
    Drop,
}

impl FuzzOnCommitAction {
    const fn into_action(self) -> OnCommitAction {
        match self {
            Self::PreserveRows => OnCommitAction::PreserveRows,
            Self::DeleteRows => OnCommitAction::DeleteRows,
            Self::Drop => OnCommitAction::Drop,
        }
    }
}

/// The two `CREATE TABLE` bodies (`arb_create_table_body`): a parenthesized element
/// list (one to three elements) or `AS <query>` (with an optional column list and
/// `WITH [NO] DATA`).
#[allow(clippy::large_enum_variant)] // see the FuzzStatement note above
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzCreateTableBody {
    Definition {
        element0: FuzzTableElement,
        element1: Option<FuzzTableElement>,
        element2: Option<FuzzTableElement>,
    },
    AsQuery {
        col0: Option<FuzzColumnName>,
        col1: Option<FuzzColumnName>,
        query: FuzzEmbeddedQuery,
        with_data: Option<bool>,
    },
}

impl FuzzCreateTableBody {
    fn into_body(self) -> CreateTableBody<NoExt> {
        match self {
            Self::Definition {
                element0,
                element1,
                element2,
            } => {
                let mut elements = thin_vec![element0.into_element()];
                for element in [element1, element2].into_iter().flatten() {
                    elements.push(element.into_element());
                }
                CreateTableBody::Definition {
                    elements,
                    meta: meta(),
                }
            }
            Self::AsQuery {
                col0,
                col1,
                query,
                with_data,
            } => {
                let mut columns = ThinVec::new();
                for col in [col0, col1].into_iter().flatten() {
                    columns.push(ident_sym(col.symbol()));
                }
                CreateTableBody::AsQuery {
                    columns,
                    query: Box::new(query.into_query()),
                    with_data,
                    meta: meta(),
                }
            }
        }
    }
}

/// A bare embedded query for `CREATE TABLE ... AS` (`arb_embedded_query`).
#[allow(clippy::large_enum_variant)] // see the FuzzStatement note above
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzEmbeddedQuery {
    Select(FuzzSelect),
    Values(FuzzValues),
}

impl FuzzEmbeddedQuery {
    fn into_query(self) -> Query<NoExt> {
        match self {
            Self::Select(select) => query_of(set_select(select.into_select())),
            Self::Values(values) => query_of(set_values(values.into_values())),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzTableElement {
    Column(FuzzColumnDef),
    Constraint(FuzzTableConstraintDef),
}

impl FuzzTableElement {
    fn into_element(self) -> TableElement<NoExt> {
        match self {
            Self::Column(column) => TableElement::Column {
                column: column.into_column_def(),
                meta: meta(),
            },
            Self::Constraint(constraint) => TableElement::Constraint {
                constraint: constraint.into_constraint_def(),
                meta: meta(),
            },
        }
    }
}

/// A column definition (`arb_column_def`). At most one column constraint is attached:
/// combining them (e.g. `NULL` with `PRIMARY KEY`) risks conflicting clauses the
/// reparse rejects; multi-constraint columns are exercised by the corpus instead.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzColumnDef {
    pub(crate) name: FuzzColumnName,
    pub(crate) data_type: FuzzDataType,
    pub(crate) constraint: Option<FuzzColumnConstraint>,
}

impl FuzzColumnDef {
    fn into_column_def(self) -> ColumnDef<NoExt> {
        let mut constraints = ThinVec::new();
        if let Some(constraint) = self.constraint {
            constraints.push(constraint.into_constraint());
        }
        ColumnDef {
            name: ident_sym(self.name.symbol()),
            data_type: Some(self.data_type.into_data_type()),
            storage: None,
            compression: None,
            constraints,
            meta: meta(),
        }
    }
}

/// A small set of unambiguous, unsized type spellings (`arb_data_type`). The full
/// data-type matrix has its own targeted round-trip coverage.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzDataType {
    SmallInt,
    Int,
    Integer,
    BigInt,
    Boolean,
    Text,
    Real,
    DoublePrecision,
    Date,
}

impl FuzzDataType {
    fn into_data_type(self) -> DataType {
        match self {
            // `display_width: None`: the structured oracle reparses under ANSI, which
            // rejects a display width on a built-in integer, so the generator never emits
            // one (the width's round-trip is covered by the MySQL/SQLite dialect tests).
            Self::SmallInt => DataType::SmallInt {
                display_width: None,
                meta: meta(),
            },
            Self::Int => DataType::Integer {
                spelling: IntegerTypeName::Int,
                display_width: None,
                meta: meta(),
            },
            Self::Integer => DataType::Integer {
                spelling: IntegerTypeName::Integer,
                display_width: None,
                meta: meta(),
            },
            Self::BigInt => DataType::BigInt {
                display_width: None,
                meta: meta(),
            },
            Self::Boolean => DataType::Boolean {
                spelling: BooleanTypeName::Boolean,
                meta: meta(),
            },
            Self::Text => DataType::Text {
                spelling: TextTypeName::Text,
                charset: None,
                meta: meta(),
            },
            Self::Real => DataType::Real { meta: meta() },
            Self::DoublePrecision => DataType::Double {
                spelling: DoubleTypeName::DoublePrecision,
                meta: meta(),
            },
            Self::Date => DataType::Date { meta: meta() },
        }
    }
}

/// A column constraint (`arb_column_constraint`). `DEFAULT`/`GENERATED`/`IDENTITY`/
/// `NULL`/`NOT NULL` cannot take a `CONSTRAINT name`, so the unnamed and named option
/// families are split.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzColumnConstraint {
    Unnamed(FuzzUnnamedColumnOption),
    Named {
        name: Option<FuzzColumnName>,
        option: FuzzNamedColumnOption,
    },
}

impl FuzzColumnConstraint {
    fn into_constraint(self) -> ColumnConstraint<NoExt> {
        match self {
            Self::Unnamed(option) => ColumnConstraint {
                name: None,
                option: option.into_option(),
                conflict: None,
                characteristics: None,
                meta: meta(),
            },
            Self::Named { name, option } => ColumnConstraint {
                name: name.map(|name| ident_sym(name.symbol())),
                option: option.into_option(),
                conflict: None,
                characteristics: None,
                meta: meta(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzUnnamedColumnOption {
    Null,
    NotNull,
    // A literal default keeps `DEFAULT <expr>` (which renders without parens) from
    // colliding with a following constraint keyword (mirrors the proptest choice).
    Default(FuzzLiteral),
    Generated(FuzzScalar),
    Identity(FuzzIdentityColumn),
}

impl FuzzUnnamedColumnOption {
    fn into_option(self) -> ColumnOption<NoExt> {
        match self {
            Self::Null => ColumnOption::Null { meta: meta() },
            Self::NotNull => ColumnOption::NotNull { meta: meta() },
            Self::Default(literal) => ColumnOption::Default {
                expr: Box::new(literal.into_expr()),
                meta: meta(),
            },
            Self::Generated(expr) => ColumnOption::Generated {
                generated: Box::new(GeneratedColumn {
                    expr: expr.into_expr(),
                    // The supported surface spells generated columns `STORED`; the bare
                    // and `VIRTUAL` forms are quarantined (the reparse requires `STORED`).
                    storage: Some(GeneratedColumnStorage::Stored),
                    // The fuzz reparse path emits the standard `GENERATED ALWAYS AS`
                    // spelling; the keywordless shorthand is exercised elsewhere.
                    spelling: GeneratedColumnSpelling::GeneratedAlways,
                    meta: meta(),
                }),
                meta: meta(),
            },
            Self::Identity(identity) => ColumnOption::Identity {
                identity: Box::new(identity.into_identity_column()),
                meta: meta(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzNamedColumnOption {
    PrimaryKey,
    Unique,
    Check(FuzzPredicate),
    References(FuzzForeignKeyRef),
}

impl FuzzNamedColumnOption {
    fn into_option(self) -> ColumnOption<NoExt> {
        match self {
            Self::PrimaryKey => ColumnOption::PrimaryKey {
                ascending: None,
                index_tablespace: None,
                meta: meta(),
            },
            Self::Unique => ColumnOption::Unique {
                nulls_not_distinct: None,
                index_tablespace: None,
                meta: meta(),
            },
            Self::Check(predicate) => ColumnOption::Check {
                expr: Box::new(predicate.into_expr()),
                no_inherit: false,
                meta: meta(),
            },
            Self::References(reference) => ColumnOption::References {
                reference: Box::new(reference.into_foreign_key_ref()),
                meta: meta(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzIdentityColumn {
    pub(crate) generation: FuzzIdentityGeneration,
    pub(crate) options: FuzzIdentityOptions,
}

impl FuzzIdentityColumn {
    fn into_identity_column(self) -> IdentityColumn<NoExt> {
        IdentityColumn {
            generation: self.generation.into_generation(),
            options: self.options.into_options(),
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzIdentityGeneration {
    Always,
    ByDefault,
}

impl FuzzIdentityGeneration {
    const fn into_generation(self) -> IdentityGeneration {
        match self {
            Self::Always => IdentityGeneration::Always,
            Self::ByDefault => IdentityGeneration::ByDefault,
        }
    }
}

/// Identity sequence options, at most one of each kind so a duplicate never appears,
/// emitted in the canonical order (`arb_identity_options`). `MinValue`/`MaxValue`
/// carry the `NO MINVALUE` vs `MINVALUE <n>` distinction via the inner option.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzIdentityOptions {
    pub(crate) start: bool,
    pub(crate) increment: bool,
    pub(crate) min_value: Option<Option<()>>,
    pub(crate) max_value: Option<Option<()>>,
    pub(crate) cache: bool,
    pub(crate) cycle: Option<bool>,
}

impl FuzzIdentityOptions {
    fn into_options(self) -> ThinVec<IdentityOption<NoExt>> {
        let mut options = ThinVec::new();
        if self.start {
            options.push(IdentityOption::StartWith {
                expr: integer_literal(),
                meta: meta(),
            });
        }
        if self.increment {
            options.push(IdentityOption::IncrementBy {
                expr: integer_literal(),
                meta: meta(),
            });
        }
        if let Some(inner) = self.min_value {
            options.push(IdentityOption::MinValue {
                value: inner.map(|()| integer_literal()),
                meta: meta(),
            });
        }
        if let Some(inner) = self.max_value {
            options.push(IdentityOption::MaxValue {
                value: inner.map(|()| integer_literal()),
                meta: meta(),
            });
        }
        if self.cache {
            options.push(IdentityOption::Cache {
                expr: integer_literal(),
                meta: meta(),
            });
        }
        if let Some(cycle) = self.cycle {
            options.push(IdentityOption::Cycle {
                cycle,
                meta: meta(),
            });
        }
        options
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzTableConstraintDef {
    pub(crate) name: Option<FuzzColumnName>,
    pub(crate) constraint: FuzzTableConstraint,
}

impl FuzzTableConstraintDef {
    fn into_constraint_def(self) -> TableConstraintDef<NoExt> {
        TableConstraintDef {
            name: self.name.map(|name| ident_sym(name.symbol())),
            constraint: self.constraint.into_constraint(),
            no_inherit: false,
            not_valid: false,
            characteristics: None,
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzTableConstraint {
    PrimaryKey(FuzzColumnList),
    Unique(FuzzColumnList),
    Check(FuzzPredicate),
    ForeignKey {
        columns: FuzzColumnList,
        references: FuzzForeignKeyRef,
    },
}

impl FuzzTableConstraint {
    fn into_constraint(self) -> TableConstraint<NoExt> {
        match self {
            Self::PrimaryKey(columns) => TableConstraint::PrimaryKey {
                columns: columns.into_index_columns(),
                include: ThinVec::new(),
                meta: meta(),
            },
            Self::Unique(columns) => TableConstraint::Unique {
                columns: columns.into_index_columns(),
                nulls_not_distinct: None,
                include: ThinVec::new(),
                meta: meta(),
            },
            Self::Check(predicate) => TableConstraint::Check {
                expr: Box::new(predicate.into_expr()),
                meta: meta(),
            },
            Self::ForeignKey {
                columns,
                references,
            } => TableConstraint::ForeignKey {
                columns: columns.into_idents(),
                references: Box::new(references.into_foreign_key_ref()),
                meta: meta(),
            },
        }
    }
}

/// A foreign-key reference (`arb_foreign_key_ref`). The `SET NULL`/`SET DEFAULT`
/// column list is legal only on `ON DELETE`, so the `ON UPDATE` action is generated
/// column-free to keep the tree reparseable.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzForeignKeyRef {
    pub(crate) col0: Option<FuzzColumnName>,
    pub(crate) col1: Option<FuzzColumnName>,
    pub(crate) match_type: Option<FuzzForeignKeyMatch>,
    pub(crate) on_delete: Option<FuzzReferentialAction>,
    pub(crate) on_update: Option<FuzzReferentialActionNoColumns>,
}

impl FuzzForeignKeyRef {
    fn into_foreign_key_ref(self) -> ForeignKeyRef {
        let mut columns = ThinVec::new();
        for col in [self.col0, self.col1].into_iter().flatten() {
            columns.push(ident_sym(col.symbol()));
        }
        ForeignKeyRef {
            table: object_name_sym(&[FuzzName::T.symbol()]),
            columns,
            match_type: self.match_type.map(FuzzForeignKeyMatch::into_match),
            on_delete: self.on_delete.map(|action| Box::new(action.into_action())),
            on_update: self.on_update.map(|action| Box::new(action.into_action())),
            update_before_delete: false,
            meta: meta(),
        }
    }
}

/// The `MATCH` mode of a foreign key. `MATCH PARTIAL` is included on purpose: the
/// parser rejects it to match PostgreSQL ("MATCH PARTIAL not yet implemented"), so it
/// exercises the accept/reject differential's now-correct rejection. It is outside the
/// render round-trip oracle's accepted subset — [`roundtrip_statement`](super::roundtrip_statement) skips a
/// generated statement carrying it (see [`statement_outside_roundtrip_subset`](super::statement_outside_roundtrip_subset)).
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzForeignKeyMatch {
    Full,
    Partial,
    Simple,
}

impl FuzzForeignKeyMatch {
    const fn into_match(self) -> ForeignKeyMatch {
        match self {
            Self::Full => ForeignKeyMatch::Full,
            Self::Partial => ForeignKeyMatch::Partial,
            Self::Simple => ForeignKeyMatch::Simple,
        }
    }
}

/// An `ON DELETE` referential action, where `SET NULL`/`SET DEFAULT` may carry a
/// PostgreSQL column list.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzReferentialAction {
    NoAction,
    Restrict,
    Cascade,
    SetNull(FuzzActionColumns),
    SetDefault(FuzzActionColumns),
}

impl FuzzReferentialAction {
    fn into_action(self) -> ReferentialAction {
        match self {
            Self::NoAction => ReferentialAction::NoAction { meta: meta() },
            Self::Restrict => ReferentialAction::Restrict { meta: meta() },
            Self::Cascade => ReferentialAction::Cascade { meta: meta() },
            Self::SetNull(columns) => ReferentialAction::SetNull {
                columns: columns.into_idents(),
                meta: meta(),
            },
            Self::SetDefault(columns) => ReferentialAction::SetDefault {
                columns: columns.into_idents(),
                meta: meta(),
            },
        }
    }
}

/// An `ON UPDATE` referential action: the column list is forbidden here, so
/// `SET NULL`/`SET DEFAULT` are column-free.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzReferentialActionNoColumns {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

impl FuzzReferentialActionNoColumns {
    fn into_action(self) -> ReferentialAction {
        match self {
            Self::NoAction => ReferentialAction::NoAction { meta: meta() },
            Self::Restrict => ReferentialAction::Restrict { meta: meta() },
            Self::Cascade => ReferentialAction::Cascade { meta: meta() },
            Self::SetNull => ReferentialAction::SetNull {
                columns: ThinVec::new(),
                meta: meta(),
            },
            Self::SetDefault => ReferentialAction::SetDefault {
                columns: ThinVec::new(),
                meta: meta(),
            },
        }
    }
}

/// Zero to two columns for a `SET NULL (..)` / `SET DEFAULT (..)` action.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzActionColumns {
    pub(crate) col0: Option<FuzzColumnName>,
    pub(crate) col1: Option<FuzzColumnName>,
}

impl FuzzActionColumns {
    fn into_idents(self) -> ThinVec<Ident> {
        let mut idents = ThinVec::new();
        for col in [self.col0, self.col1].into_iter().flatten() {
            idents.push(ident_sym(col.symbol()));
        }
        idents
    }
}

/// `WITH (<storage parameter>, ...)` — one or two parameters (`arb_with_option`).
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzWithOption {
    pub(crate) param0: FuzzStorageParameter,
    pub(crate) param1: Option<FuzzStorageParameter>,
}

impl FuzzWithOption {
    fn into_option(self) -> CreateTableOption<NoExt> {
        let mut params = thin_vec![self.param0.into_parameter()];
        if let Some(param) = self.param1 {
            params.push(param.into_parameter());
        }
        CreateTableOption {
            kind: CreateTableOptionKind::With {
                params,
                meta: meta(),
            },
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzStorageParameter {
    pub(crate) name: FuzzColumnName,
    pub(crate) value: bool,
}

impl FuzzStorageParameter {
    fn into_parameter(self) -> TableStorageParameter<NoExt> {
        TableStorageParameter {
            name: object_name_sym(&[self.name.symbol()]),
            value: self.value.then(integer_literal),
            meta: meta(),
        }
    }
}

// ---------------------------------------------------------------------------
// INSERT / UPDATE / DELETE
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzInsert {
    pub(crate) with: Option<FuzzWith>,
    pub(crate) target: FuzzInsertTarget,
    pub(crate) source: FuzzInsertOverridingAndSource,
}

impl FuzzInsert {
    fn into_insert(self) -> Insert<NoExt> {
        let (overriding, source) = self.source.into_parts();
        Insert {
            // The fuzzer only generates the standard `INSERT` spelling; the MySQL
            // `REPLACE` verb is outside the ANSI reparse round-trip subset.
            verb: InsertVerb::Insert,
            // The SQLite `OR <action>` prefix is dialect-gated outside the ANSI reparse
            // path; quarantined as `None`.
            or_action: None,
            column_matching: None,
            with: self.with.map(FuzzWith::into_with),
            // A target column list on a `DEFAULT VALUES` source is PostgreSQL-illegal
            // (`INSERT INTO t (a) DEFAULT VALUES` is a syntax error), and the parser now
            // rejects it. The list is generated unconditionally so that combo reaches the
            // accept/reject differential's now-correct rejection; the render round-trip
            // oracle skips it instead (see `statement_outside_roundtrip_subset`).
            target: self.target.into_target(),
            overriding,
            source,
            // The MySQL row alias, upsert, and returning clauses are dialect-gated
            // outside the ANSI reparse path; quarantined.
            row_alias: None,
            upsert: None,
            returning: None,
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzInsertTarget {
    pub(crate) alias: bool,
    pub(crate) col0: Option<FuzzColumnName>,
    pub(crate) col1: Option<FuzzColumnName>,
}

impl FuzzInsertTarget {
    fn into_target(self) -> InsertTarget {
        let mut columns = ThinVec::new();
        for col in [self.col0, self.col1].into_iter().flatten() {
            columns.push(ident_sym(col.symbol()));
        }
        InsertTarget {
            name: object_name_sym(&[FuzzName::T.symbol()]),
            alias: self.alias.then(|| ident_sym(FuzzName::X.symbol())),
            alias_spelling: AliasSpelling::As,
            columns,
            meta: meta(),
        }
    }
}

/// `OVERRIDING` only attaches to a `VALUES`/query source, so `DEFAULT VALUES` is
/// generated without it (`arb_insert_overriding_and_source`).
#[allow(clippy::large_enum_variant)] // see the FuzzStatement note above
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzInsertOverridingAndSource {
    DefaultValues,
    Values {
        overriding: Option<FuzzInsertOverriding>,
        values: FuzzInsertValues,
    },
    Query {
        overriding: Option<FuzzInsertOverriding>,
        // The query source must render as a `SELECT` (not a bare `VALUES`): the parser
        // canonicalizes `INSERT ... VALUES (...)` to `InsertSource::Values`, so a
        // `Query` wrapping a `VALUES` body would not round-trip.
        select: FuzzSelect,
    },
}

impl FuzzInsertOverridingAndSource {
    fn into_parts(self) -> (Option<InsertOverriding>, InsertSource<NoExt>) {
        match self {
            Self::DefaultValues => (
                None,
                InsertSource::DefaultValues {
                    default: DefaultValue { meta: meta() },
                    meta: meta(),
                },
            ),
            Self::Values { overriding, values } => (
                overriding.map(FuzzInsertOverriding::into_overriding),
                InsertSource::Values {
                    values: Box::new(values.into_values()),
                    meta: meta(),
                },
            ),
            Self::Query { overriding, select } => (
                overriding.map(FuzzInsertOverriding::into_overriding),
                InsertSource::Query {
                    query: Box::new(query_of(set_select(select.into_select()))),
                    meta: meta(),
                },
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzInsertOverriding {
    SystemValue,
    UserValue,
}

impl FuzzInsertOverriding {
    const fn into_overriding(self) -> InsertOverriding {
        match self {
            Self::SystemValue => InsertOverriding::SystemValue,
            Self::UserValue => InsertOverriding::UserValue,
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzInsertValues {
    pub(crate) row0: FuzzInsertRow,
    pub(crate) row1: Option<FuzzInsertRow>,
}

impl FuzzInsertValues {
    fn into_values(self) -> InsertValues<NoExt> {
        let mut rows = thin_vec![self.row0.into_row()];
        if let Some(row) = self.row1 {
            rows.push(row.into_row());
        }
        InsertValues { rows, meta: meta() }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzInsertRow {
    pub(crate) item0: FuzzInsertValue,
    pub(crate) item1: Option<FuzzInsertValue>,
}

impl FuzzInsertRow {
    fn into_row(self) -> ThinVec<InsertValue<NoExt>> {
        let mut items = thin_vec![self.item0.into_insert_value()];
        if let Some(item) = self.item1 {
            items.push(item.into_insert_value());
        }
        items
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzInsertValue {
    Expr(FuzzScalar),
    Default,
}

impl FuzzInsertValue {
    fn into_insert_value(self) -> InsertValue<NoExt> {
        match self {
            Self::Expr(expr) => InsertValue::Expr {
                expr: expr.into_expr(),
                meta: meta(),
            },
            Self::Default => InsertValue::Default {
                default: DefaultValue { meta: meta() },
                meta: meta(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzUpdate {
    pub(crate) with: Option<FuzzWith>,
    pub(crate) target: FuzzDmlTarget,
    pub(crate) assignment0: FuzzUpdateAssignment,
    pub(crate) assignment1: Option<FuzzUpdateAssignment>,
    pub(crate) from0: Option<FuzzTableWithJoins>,
    pub(crate) selection: Option<FuzzWhereSelection>,
}

impl FuzzUpdate {
    fn into_update(self) -> Update<NoExt> {
        let mut assignments = thin_vec![self.assignment0.into_assignment()];
        if let Some(assignment) = self.assignment1 {
            assignments.push(assignment.into_assignment());
        }
        let mut from = ThinVec::new();
        if let Some(table) = self.from0 {
            from.push(table.into_table_with_joins());
        }
        Update {
            with: self.with.map(FuzzWith::into_with),
            // The SQLite `OR <action>` prefix is dialect-gated outside the ANSI reparse
            // path; quarantined as `None`.
            or_action: None,
            target: self.target.into_target(),
            assignments,
            from,
            selection: self.selection.map(FuzzWhereSelection::into_selection),
            // The MySQL `ORDER BY`/`LIMIT` tails are dialect-gated outside the ANSI
            // reparse path; quarantined.
            order_by: ThinVec::new(),
            limit: None,
            returning: None,
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzDelete {
    pub(crate) with: Option<FuzzWith>,
    pub(crate) target: FuzzDmlTarget,
    pub(crate) using0: Option<FuzzTableWithJoins>,
    pub(crate) selection: Option<FuzzWhereSelection>,
}

impl FuzzDelete {
    fn into_delete(self) -> Delete<NoExt> {
        let mut using = ThinVec::new();
        if let Some(table) = self.using0 {
            using.push(table.into_table_with_joins());
        }
        Delete {
            with: self.with.map(FuzzWith::into_with),
            target: self.target.into_target(),
            using,
            selection: self.selection.map(FuzzWhereSelection::into_selection),
            // The MySQL `ORDER BY`/`LIMIT` tails are dialect-gated outside the ANSI
            // reparse path; quarantined.
            order_by: ThinVec::new(),
            limit: None,
            returning: None,
            meta: meta(),
        }
    }
}

/// The `UPDATE`/`DELETE` target relation (`arb_dml_target`). The `ONLY`/`*`
/// inheritance markers are PostgreSQL syntax gated out of the ANSI reparse path, so
/// the plain spelling is the only one generated.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzDmlTarget {
    pub(crate) alias: bool,
}

impl FuzzDmlTarget {
    fn into_target(self) -> DmlTarget {
        DmlTarget {
            name: object_name_sym(&[FuzzName::T.symbol()]),
            inheritance: RelationInheritance::Plain,
            alias: self.alias.then(|| ident_sym(FuzzName::X.symbol())),
            alias_spelling: AliasSpelling::As,
            meta: meta(),
        }
    }
}

/// The `WHERE` filter on an `UPDATE`/`DELETE` (`arb_where_selection`). `WHERE CURRENT
/// OF <cursor>` is positioned-update syntax gated out of the ANSI reparse path, so
/// only the ordinary condition form is generated.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzWhereSelection {
    pub(crate) condition: FuzzPredicate,
}

impl FuzzWhereSelection {
    fn into_selection(self) -> DmlSelection<NoExt> {
        DmlSelection::Where {
            condition: self.condition.into_expr(),
            meta: meta(),
        }
    }
}

/// Only the single-column `SET col = value` form is generated (`arb_update_assignment`).
/// The multiple-column `SET (cols) = <source>` assignment (SQL feature T641,
/// `UpdateAssignment::Tuple`) is dialect-gated and the ANSI reparse path rejects the
/// parenthesized target list, so it is quarantined.
#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) struct FuzzUpdateAssignment {
    pub(crate) target: FuzzColumnName,
    pub(crate) value: FuzzUpdateValue,
}

impl FuzzUpdateAssignment {
    fn into_assignment(self) -> UpdateAssignment<NoExt> {
        UpdateAssignment::Single {
            target: object_name_sym(&[self.target.symbol()]),
            value: self.value.into_value(),
            meta: meta(),
        }
    }
}

#[derive(Clone, Copy, Debug, Arbitrary)]
pub(crate) enum FuzzUpdateValue {
    Expr(FuzzScalar),
    Default,
}

impl FuzzUpdateValue {
    fn into_value(self) -> UpdateValue<NoExt> {
        match self {
            Self::Expr(expr) => UpdateValue::Expr {
                expr: expr.into_expr(),
                meta: meta(),
            },
            Self::Default => UpdateValue::Default {
                default: DefaultValue { meta: meta() },
                meta: meta(),
            },
        }
    }
}

fn limit_clause(limit: Option<Expr<NoExt>>, offset: Option<Expr<NoExt>>) -> Limit<NoExt> {
    Limit {
        limit,
        offset,
        syntax: LimitSyntax::LimitOffset,
        with_ties: None,
        percent: None,
        fetch_spelling: FetchSpelling::FirstRows,
        meta: meta(),
    }
}

fn ident(name: FuzzName) -> Ident {
    ident_sym(name.symbol())
}

fn integer_literal() -> Expr<NoExt> {
    literal(LiteralKind::Integer)
}

fn column_expr(col: FuzzColumnName) -> Expr<NoExt> {
    Expr::Column {
        name: object_name_sym(&[col.symbol()]),
        meta: meta(),
    }
}

fn named_window(sym: u32, definition: FuzzWindowDefinition) -> NamedWindow<NoExt> {
    NamedWindow {
        name: ident_sym(sym),
        definition: definition.into_window_definition(),
        meta: meta(),
    }
}

fn tablespace_option() -> CreateTableOption<NoExt> {
    CreateTableOption {
        kind: CreateTableOptionKind::Tablespace {
            tablespace: ident_sym(FuzzName::X.symbol()),
            meta: meta(),
        },
        meta: meta(),
    }
}
