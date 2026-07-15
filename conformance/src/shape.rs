// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The engine-neutral structural-comparison vocabulary and the `squonk`-AST mapper.
//!
//! PostgreSQL was the first structural engine, so this neutral model grew inside
//! [`pg`](crate::pg). It is not PostgreSQL-specific: it is the incremental
//! protobuf-to-canonical shape ADR-0015 calls for — enough structure to compare the
//! constructs both parsers support while avoiding a second `squonk` interner. Every
//! structural engine maps *into* this vocabulary (`crate::pg` maps the PostgreSQL
//! protobuf, `crate::duckdb_structural` maps DuckDB's `json_serialize_sql` tree), and
//! this module owns the neutral types plus the `squonk`-side mapper
//! ([`squonk_shape`] / the crate-private `squonk_shape_result` and their
//! `*_shape` family). The per-engine protobuf/JSON mappers stay with their engine
//! module.
//!
//! # The canonical data-type identity space is PostgreSQL's `pg_catalog`
//!
//! [`DataTypeShape`] names a type by its PostgreSQL `pg_catalog` spelling — `int4`,
//! `bpchar`, `timestamptz` — not the surface keyword. `squonk_data_type_shape`
//! folds our AST to those names, and the PostgreSQL side's `normalize_pg_type_name`
//! (in [`pg`](crate::pg)) folds the protobuf to the same ones, so both mappers land on
//! one identity space. This is a deliberate shared choice, not a PostgreSQL bias baked
//! into the "neutral" model; changing it (e.g. to a dialect-agnostic spelling) is out
//! of scope for the extraction and tracked as an open question for
//! `conformance-eval-mediated-roundtrip-structural`.
//!
//! # PostgreSQL-populated-only variants
//!
//! A few neutral members are only ever produced by the PostgreSQL mapper on today's
//! corpus — they exist so the vocabulary stays exhaustive over the canonical AST, and
//! another engine may populate them later:
//! - [`ReferentialActionShape::SetNull`] (and `SetDefault`) carry the PostgreSQL `ON
//!   DELETE` column list (`fk_del_set_cols`); empty for the bare form.
//! - [`RoleSpecShape::CurrentRole`] — PostgreSQL's `CURRENT_ROLE` grantee spelling.

use squonk::Parsed;
use squonk_ast::{
    AccessControlStatement, AlterColumnAction, AlterColumnTarget, AlterTable, AlterTableAction,
    ApplyKind, ArrayExpr, AsOfJoinKind, BinaryOperator, BitwiseXorSpelling, CharacterTypeName,
    ColumnDef, ColumnOption, CommentOnStatement, CommentTarget, ConfigParameter, ConflictAction,
    ConflictTarget, CreateIndex, CreateSchema, CreateTable, CreateTableBody, CreateTableOption,
    CreateTableOptionKind, CreateView, Cte, CteBody, DataType, Delete, DmlSelection, DmlTarget,
    DropBehavior, DropObjectKind, DropStatement, ExplainFormat, ExplainOption, ExplainStatement,
    Expr, ForeignKeyMatch, FunctionCall, GeneratedColumnStorage, GrantObject, Grantee, GroupByItem,
    Ident, IdentityGeneration, IdentityOption, IndexColumn, Insert, InsertOverriding, InsertSource,
    InsertValue, IsDistinctFromSpelling, IsNotDistinctFromSpelling, IsolationLevel, JoinConstraint,
    JoinOperator, Limit, Literal, LiteralKind, LockStrength, LockWait, LockingClause, Merge,
    MergeAction, MergeMatchKind, NamedObjectKind, NoExt, ObjectName, OnCommitAction, OnConflict,
    OrderByExpr, ParameterKind, PivotExpr, Privilege, PrivilegeKind, Privileges, Quantifier, Query,
    QuoteStyle, ReferentialAction, RelationInheritance, Resolver as _, Returning, RoleSpec,
    RoutineObjectKind, RoutineSignature, SchemaObjectKind, Select, SelectDistinct, SelectItem,
    SemiAntiSide, SessionStatement, SetExpr, SetOperator, SetParameterValue, SetQuantifier,
    SetScope, SetValue, SpecialFunctionKeyword, Statement, SubscriptKind, TableAlias,
    TableConstraint, TableConstraintDef, TableElement, TableFactor, TableFunctionColumn,
    TableSample, TableWithJoins, TimeTypeName, TimeZone, TimestampTypeName, TransactionAccessMode,
    TransactionMode, TransactionStart, TransactionStatement, UnaryOperator, Update,
    UpdateAssignment, UpdateTupleSource, UpdateValue, Upsert, Values, ValuesItem, ViewCheckOption,
    With,
};

/// Neutral query structure for cross-parser structural comparison.
///
/// This is not a replacement AST. It is the incremental protobuf-to-canonical
/// mapping ADR-0015 calls for: enough structure to compare the M1 SELECT/query
/// constructs currently supported by both parsers while avoiding symbol/interner
/// coupling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryShape {
    pub with: Option<WithShape>,
    pub body: SetShape,
    pub order_by: Vec<OrderByShape>,
    /// DuckDB's `ORDER BY ALL` clause mode (`duckdb-group-order-by-all`). One
    /// neutral member per ADR-0011: our side maps `Query::order_by_all`, the DuckDB
    /// structural oracle maps its serialized whole-projection star order entry, and
    /// PostgreSQL — whose grammar has no such mode — always maps `None`. Mutually
    /// exclusive with a non-empty `order_by` (the engine rejects mixing).
    pub order_by_all: Option<OrderByAllShape>,
    pub limit: LimitShape,
    /// Row-locking clauses (`FOR UPDATE`/`FOR SHARE …`). One neutral member per
    /// ADR-0011: our side maps `Query::locking`, PostgreSQL maps the `SelectStmt`
    /// `locking_clause` list. The surface spelling (`LOCK IN SHARE MODE` vs
    /// `FOR SHARE`) is dropped — only the semantic strength/targets/wait compare, like
    /// [`LimitShape`] ignoring `LimitSyntax`.
    pub locking: Vec<LockingClauseShape>,
}

/// Neutral shape of one row-locking clause: the semantic strength, the `OF` target
/// relations (as object-name parts), and the wait policy. No surface spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockingClauseShape {
    pub strength: LockStrengthShape,
    pub of: Vec<Vec<String>>,
    pub wait: Option<LockWaitShape>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockStrengthShape {
    Update,
    NoKeyUpdate,
    Share,
    KeyShare,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockWaitShape {
    NoWait,
    SkipLocked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetShape {
    Select(SelectShape),
    Values(Vec<Vec<ValuesItemShape>>),
    SetOperation {
        op: SetOpShape,
        all: bool,
        /// DuckDB's `UNION [ALL] BY NAME` name-matched modifier (`duckdb-union-by-name`).
        /// One neutral member per ADR-0011: our side maps
        /// [`SetExpr::SetOperation::by_name`](squonk_ast::ast::SetExpr), the DuckDB
        /// structural oracle maps its `setop_type: UNION_BY_NAME` (distinct from
        /// `UNION`), and PostgreSQL — whose grammar has no name-matched set operation —
        /// always maps `false`. Orthogonal to `all` (DuckDB records `setop_all`
        /// separately), and only ever `true` for [`SetOpShape::Union`].
        by_name: bool,
        left: Box<SetShape>,
        right: Box<SetShape>,
    },
    /// A set operand that carries its own query-level clauses
    /// (`X UNION (Y ORDER BY 1 LIMIT 3)`). Only constructed when the operand has a
    /// `with`/`order_by`/`limit` of its own — a pure-grouping operand collapses to
    /// its inner body instead, so this variant never wraps a clause-free query
    /// (keeping the two spellings of the same operand a single canonical shape).
    Query(Box<QueryShape>),
}

/// One item in a `VALUES` query row: an expression or a bare `DEFAULT`.
///
/// Mirrors [`InsertItemShape`]: PostgreSQL maps a `DEFAULT` row element to a
/// `SetToDefault` node, so the neutral shape distinguishes it from an expression
/// rather than forcing `DEFAULT` through [`ExprShape`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValuesItemShape {
    Expr(ExprShape),
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithShape {
    pub recursive: bool,
    pub ctes: Vec<CteShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CteShape {
    pub name: String,
    pub columns: Vec<String>,
    pub materialized: Option<bool>,
    pub body: CteBodyShape,
}

/// The CTE body: a query, or one of PostgreSQL's data-modifying CTE statements
/// (mirroring the canonical [`CteBody`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CteBodyShape {
    Query(Box<QueryShape>),
    Insert(Box<InsertShape>),
    Update(Box<UpdateShape>),
    Delete(Box<DeleteShape>),
    /// `pg_shape` treats `MergeStmt` as outside the structural subset (exactly as
    /// the top-level statement mapper does), so this arm currently maps only on the
    /// `squonk` side; it stays a full shape so two of our shapes never falsely
    /// compare equal (the [`InsertSourceShape::Set`] exhaustiveness idiom).
    Merge(Box<MergeShape>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectShape {
    pub distinct: bool,
    pub projection: Vec<SelectItemShape>,
    pub from: Vec<TableWithJoinsShape>,
    pub selection: Option<ExprShape>,
    pub group_by: Vec<GroupByItemShape>,
    /// PostgreSQL's `GROUP BY DISTINCT` grouping-set quantifier
    /// (`pg-group-by-distinct-grouping-sets`). One neutral member per ADR-0011: our side
    /// maps `Select::group_by_quantifier == Some(Distinct)`, PostgreSQL maps its
    /// `SelectStmt.group_distinct` flag. PostgreSQL collapses explicit `ALL` and an
    /// unwritten quantifier to `false`, so this is a bool (`DISTINCT` vs. not), and our
    /// mapping matches that collapse. The DuckDB structural oracle — whose grammar has no
    /// such quantifier — always maps `false`.
    pub group_by_distinct: bool,
    /// DuckDB's `GROUP BY ALL` clause mode (`duckdb-group-order-by-all`). One
    /// neutral member per ADR-0011: our side maps `Select::group_by_all`, the
    /// DuckDB structural oracle maps its `FORCE_AGGREGATES` aggregate handling, and
    /// PostgreSQL — whose grammar has no such mode — always maps `false`. Mutually
    /// exclusive with a non-empty `group_by` (the engine rejects mixing).
    pub group_by_all: bool,
    pub having: Option<ExprShape>,
    /// DuckDB's `QUALIFY` post-window filter (`duckdb-qualify-clause`). One neutral
    /// member per ADR-0011: the DuckDB structural oracle maps its first-class
    /// `SELECT_NODE` `qualify` field here, our side maps `Select::qualify`, and
    /// PostgreSQL — whose grammar has no such clause — always maps `None`. Boxed like
    /// the AST field: the clause is rare, and inline it tips `SetShape::Select` over
    /// clippy's large-variant threshold.
    pub qualify: Option<Box<ExprShape>>,
}

/// Neutral shape of one `GROUP BY` item, comparing the grouping-set tree structure
/// from both parsers.
///
/// This closes the oracle hole that hid the `ROLLUP`/`CUBE` mis-parse: previously
/// `group_by` was a `Vec<ExprShape>`, so a grouping construct could only ever map to
/// an expression, making a function-call mis-parse indistinguishable from the correct
/// grouping-set node. PostgreSQL emits a `GroupingSet` node (not a function call) for
/// each construct, so the two sides now diverge unless squonk models them too.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroupByItemShape {
    Expr(ExprShape),
    Rollup(Vec<ExprShape>),
    Cube(Vec<ExprShape>),
    GroupingSets(Vec<GroupByItemShape>),
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetOpShape {
    Union,
    Intersect,
    Except,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectItemShape {
    Wildcard,
    QualifiedWildcard(Vec<String>),
    Expr {
        expr: ExprShape,
        alias: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableWithJoinsShape {
    pub relation: TableFactorShape,
    pub joins: Vec<JoinShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableFactorShape {
    Table {
        name: Vec<String>,
        alias: Option<AliasShape>,
        only: bool,
        sample: Option<TableSampleShape>,
    },
    Derived {
        lateral: bool,
        subquery: Box<QueryShape>,
        alias: Option<AliasShape>,
    },
    Function {
        lateral: bool,
        function: FunctionShape,
        with_ordinality: bool,
        alias: Option<AliasShape>,
        /// The function-level `func_alias_clause` column definition list
        /// (`func(...) AS x(a int, b text)`); empty for an untyped function.
        column_defs: Vec<ColumnDefShape>,
    },
    RowsFrom {
        lateral: bool,
        functions: Vec<RowsFromItemShape>,
        with_ordinality: bool,
        alias: Option<AliasShape>,
    },
    NestedJoin {
        table: Box<TableWithJoinsShape>,
        alias: Option<AliasShape>,
    },
    /// A bare special value function used as a table reference (`SELECT * FROM
    /// current_date`). Mapped on the `squonk` side only — the PostgreSQL
    /// side (`RangeFunction` wrapping `SqlValueFunction`) is not unwrapped, so a
    /// structural comparison against it stays an honest "not implemented" gap
    /// rather than a guessed mapping (mirrors `ExprShape::Unmapped` for the same
    /// construct in expression position; this item's allowlist entry is
    /// `AcceptReject`, which does not need structural parity).
    SpecialFunction {
        keyword: SpecialFunctionKeyword,
        precision: Option<u32>,
        alias: Option<AliasShape>,
    },
    /// A DuckDB `PIVOT` table factor (`t PIVOT (sum(x) FOR y IN (…))`). PostgreSQL
    /// has no pivot grammar, so only the DuckDB structural oracle produces this
    /// shape on the engine side (`json_serialize_sql` emits a first-class `PIVOT`
    /// `from_table` node for the table-factor spelling — probed on 1.5.4 — so the
    /// comparison is real tree parity, not a desugar match). The canonical shape
    /// carries the operator fields only: the statement-spelled surfaces (top-level
    /// or parenthesized into FROM) never reach a comparison — the serializer
    /// refuses the statement (`Only SELECT statements can be serialized`), skipping
    /// them engine-side — so the statement-only `WITH`/`ORDER BY`/`LIMIT` members
    /// are deliberately not part of the shape.
    Pivot(Box<PivotShape>),
    /// A DuckDB `UNPIVOT` table factor — the [`Pivot`](Self::Pivot) counterpart
    /// (the engine serializes both operators as the one `PIVOT` node class,
    /// discriminated by its `unpivot_names`).
    Unpivot(Box<UnpivotShape>),
}

/// The neutral shape of a DuckDB `PIVOT` table factor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PivotShape {
    pub source: Box<TableFactorShape>,
    pub aggregates: Vec<PivotExprShape>,
    pub pivot_on: Vec<PivotColumnShape>,
    pub group_by: Vec<ExprShape>,
    pub alias: Option<AliasShape>,
}

/// The neutral shape of a DuckDB `UNPIVOT` table factor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnpivotShape {
    pub source: Box<TableFactorShape>,
    pub value: Vec<String>,
    pub name: Vec<String>,
    pub columns: Vec<UnpivotColumnShape>,
    pub include_nulls: bool,
    pub alias: Option<AliasShape>,
}

/// An aliased pivot expression — a `USING`/paren-list aggregate or an `IN` value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PivotExprShape {
    pub expr: ExprShape,
    pub alias: Option<String>,
}

/// One pivot column with its `IN` source — a written value list or a named ENUM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PivotColumnShape {
    pub expr: ExprShape,
    pub values: Vec<PivotExprShape>,
    /// The `IN <enum>` form's type name; mutually exclusive with `values`.
    pub enum_source: Option<String>,
}

/// One `UNPIVOT` column entry (a column or a grouped `(a, b)`, with its alias).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnpivotColumnShape {
    pub columns: Vec<ExprShape>,
    pub alias: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AliasShape {
    pub name: String,
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionShape {
    pub name: Vec<String>,
    pub args: Vec<ExprShape>,
    pub wildcard: bool,
}

/// One `ROWS FROM (...)` item: a function call and its optional per-function
/// column definition list (`func(...) AS (a int)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowsFromItemShape {
    pub function: FunctionShape,
    pub column_defs: Vec<ColumnDefShape>,
}

/// A typed column definition (`name type`) inside a table function's column
/// definition list (PostgreSQL `TableFuncElement`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnDefShape {
    pub name: String,
    pub data_type: DataTypeShape,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableSampleShape {
    pub method: Vec<String>,
    pub args: Vec<ExprShape>,
    pub repeatable: Option<ExprShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinShape {
    pub relation: TableFactorShape,
    pub operator: JoinOperatorShape,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JoinOperatorShape {
    Inner(JoinConstraintShape),
    LeftOuter(JoinConstraintShape),
    RightOuter(JoinConstraintShape),
    FullOuter(JoinConstraintShape),
    /// DuckDB `ASOF [side] JOIN` — never produced by a PostgreSQL parse; the DuckDB
    /// structural oracle maps `ref_type: ASOF` here (the side enum is the AST leaf
    /// reused directly, like `SpecialFunctionKeyword` above).
    AsOf(AsOfJoinKind, JoinConstraintShape),
    Cross,
    /// DuckDB `POSITIONAL JOIN` — never produced by a PostgreSQL parse; the DuckDB
    /// structural oracle maps `ref_type: POSITIONAL` here. Constraint-less like
    /// `Cross`.
    Positional,
    /// DuckDB `[ASOF|NATURAL] SEMI JOIN` or Spark `[LEFT|RIGHT] SEMI JOIN` — never
    /// produced by a PostgreSQL parse; the DuckDB structural oracle maps `join_type:
    /// SEMI` here. The `bool` is the `ASOF` composition flag (`ref_type: ASOF`, side-less
    /// only) and the [`SemiAntiSide`] the surface side, mirroring the AST's
    /// [`JoinOperator::Semi`](squonk::ast::JoinOperator)`::asof`/`::side`.
    Semi(bool, SemiAntiSide, JoinConstraintShape),
    /// DuckDB `[ASOF|NATURAL] ANTI JOIN` or Spark `[LEFT|RIGHT] ANTI JOIN` — the
    /// [`Semi`](Self::Semi) counterpart (`join_type: ANTI`); same `ASOF` flag and side.
    Anti(bool, SemiAntiSide, JoinConstraintShape),
    /// MSSQL `CROSS`/`OUTER APPLY` — never produced by a PostgreSQL parse and covered
    /// by no oracle, so this shape only ever compares against itself. Constraint-less
    /// like `Cross`; the `kind` is the AST leaf reused directly.
    Apply(ApplyKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JoinConstraintShape {
    On(ExprShape),
    Using {
        columns: Vec<String>,
        alias: Option<String>,
    },
    Natural,
    None,
}

/// Neutral shape of DuckDB's `ORDER BY ALL [ASC | DESC] [NULLS FIRST | LAST]`
/// clause mode: the direction/nulls modifiers only — the mode has no key
/// expression (the sort keys are bind-time projection columns), so it is not an
/// [`OrderByShape`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OrderByAllShape {
    pub asc: Option<bool>,
    pub nulls_first: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderByShape {
    pub expr: ExprShape,
    pub asc: Option<bool>,
    /// PostgreSQL `USING <operator>` sort operator, as the schema-qualified name
    /// parts with the operator symbol last (`["<"]`, `["pg_catalog", "<"]`); `None`
    /// for the ordinary `ASC`/`DESC` form. Mutually exclusive with a set `asc`.
    pub using: Option<Vec<String>>,
    pub nulls_first: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LimitShape {
    pub count: Option<ExprShape>,
    pub offset: Option<ExprShape>,
    /// Whether a `FETCH` tail chose `WITH TIES`. A plain `bool`, not a tri-state
    /// mirroring `Limit::with_ties`: PostgreSQL's raw tree cannot distinguish an
    /// ordinary `LIMIT n` from a plain `FETCH n ROWS ONLY` (both set
    /// `limit_option = LIMIT_OPTION_COUNT`), so — like this shape already
    /// ignoring `LimitSyntax` entirely — only the genuinely semantic `WITH TIES`
    /// bit is compared, not which spelling was used to reach "not with ties".
    pub with_ties: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprShape {
    Column(Vec<String>),
    Literal(LiteralShape),
    BinaryOp {
        left: Box<ExprShape>,
        op: BinaryOperatorShape,
        right: Box<ExprShape>,
    },
    UnaryOp {
        op: UnaryOperatorShape,
        expr: Box<ExprShape>,
    },
    Cast {
        expr: Box<ExprShape>,
        data_type: DataTypeShape,
    },
    InSubquery {
        expr: Box<ExprShape>,
        subquery: Box<QueryShape>,
        negated: bool,
    },
    Exists(Box<QueryShape>),
    QuantifiedComparison {
        left: Box<ExprShape>,
        op: BinaryOperatorShape,
        quantifier: Quantifier,
        subquery: Box<QueryShape>,
    },
    Subquery(Box<QueryShape>),
    Function(FunctionShape),
    IsNull {
        expr: Box<ExprShape>,
        negated: bool,
    },
    Case {
        operand: Option<Box<ExprShape>>,
        when_clauses: Vec<WhenClauseShape>,
        else_result: Option<Box<ExprShape>>,
    },
    /// An array constructor's element list — PostgreSQL `ARRAY[…]` and the DuckDB
    /// bare-bracket `[…]` fold onto this one shape (the spelling tag is a surface
    /// concern, ADR-0011). DuckDB desugars both to a `list_value(…)` call in its
    /// serialized tree; the DuckDB mapping normalizes that back here (ADR-0015
    /// representation equivalence). The `ARRAY(<query>)` subquery form stays
    /// [`Unmapped`](Self::Unmapped).
    Array(Vec<ExprShape>),
    /// A DuckDB struct literal's fields, in source order. Keys compare by exact
    /// text: the engine's `struct_pack` desugaring preserves the written key
    /// (quoted or bare) verbatim in each child's alias, and the three key
    /// spellings all name the same field, so only the text is the shape.
    Struct(Vec<(String, ExprShape)>),
    /// An array element access `base[index]` ([`SubscriptKind::Index`], index in `lower`),
    /// a two-bound slice `base[lower:upper]` ([`SubscriptKind::Slice`], either bound
    /// optional), or a DuckDB three-bound slice `base[lower:upper:step]`
    /// ([`SubscriptKind::SliceWithStep`], `step` present when spelled) — the
    /// PostgreSQL/DuckDB subscript surface. DuckDB serializes these as
    /// `ARRAY_EXTRACT`/`ARRAY_SLICE` operator nodes (the stepped slice is a four-child
    /// `ARRAY_SLICE`); the PostgreSQL protobuf (`A_Indirection`) is not mapped yet, so
    /// subscripts only reach comparison through the DuckDB oracle. An omitted bound (the
    /// two-bound `[lower:]`/`[:upper]`, the stepped `-` open-upper placeholder, or an
    /// omitted trailing step) is `None`, matching DuckDB's omitted-bound sentinel.
    Subscript {
        base: Box<ExprShape>,
        lower: Option<Box<ExprShape>>,
        upper: Option<Box<ExprShape>>,
        step: Option<Box<ExprShape>>,
        kind: SubscriptKind,
    },
    /// A DuckDB single-arrow lambda: parameter names (compared by exact text, like
    /// [`Struct`](Self::Struct) keys — the spelling tag is a surface concern,
    /// ADR-0011) and a body. DuckDB serializes every `->` as a `LAMBDA` node; the
    /// DuckDB mapping applies the same parameter-shape split our parser does
    /// (`duckdb-lambda-expressions`), so a non-parameter `->` normalizes to the
    /// [`JsonGet`](BinaryOperatorShape::JsonGet) [`BinaryOp`](Self::BinaryOp)
    /// instead. The PostgreSQL side never produces this (its `->` is always the
    /// JSON operator), so lambdas only reach comparison through the DuckDB oracle.
    Lambda {
        params: Vec<String>,
        body: Box<ExprShape>,
    },
    /// DuckDB's `COLUMNS(<selector>)` star expression ([`Expr::Columns`]). `pattern`
    /// is the selector argument — `None` for the star form `COLUMNS(*)` /
    /// `COLUMNS(t.*)`, `Some` for `COLUMNS(<expr>)` (the regex string, a lambda, a
    /// name list, or a column); `qualifier` is the qualified star's relation
    /// (DuckDB's single `relation_name` slot). DuckDB serializes the whole construct
    /// as a `columns: true` STAR node; the DuckDB mapping unwraps its lambda
    /// desugaring (`COLUMNS(λ)` → `list_filter(*, λ)`) back to the lambda (ADR-0015
    /// representation equivalence). A star form carrying `EXCLUDE`/`REPLACE`/`RENAME`
    /// modifiers (`COLUMNS(* EXCLUDE …)`) has no neutral shape here yet and is
    /// skipped on the DuckDB side, so it never reaches this shape.
    Columns {
        qualifier: Option<Vec<String>>,
        pattern: Option<Box<ExprShape>>,
    },
    /// A `squonk` expression construct outside the neutral structural corpus
    /// (the PostgreSQL postfix/constructor forms). The PostgreSQL mapping never
    /// produces this, so any statement carrying one is reported as a structural
    /// divergence — an explicit gap (ADR-0015) rather than a mapping panic — until
    /// the construct is wired into the differential.
    Unmapped,
}

/// One `WHEN <condition> THEN <result>` branch of a [`ExprShape::Case`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhenClauseShape {
    pub condition: ExprShape,
    pub result: ExprShape,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataTypeShape {
    Named {
        name: Vec<String>,
        modifiers: Vec<i64>,
        array_depth: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiteralShape {
    Integer(String),
    Float(String),
    String(String),
    Boolean(bool),
    Null,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOperatorShape {
    Plus,
    Minus,
    Multiply,
    Divide,
    Modulo,
    StringConcat,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    IsDistinctFrom,
    IsNotDistinctFrom,
    And,
    Or,
    /// The SQL-standard `OVERLAPS` period predicate. A real PostgreSQL operator, but its
    /// two-element-row operands are outside the neutral structural corpus
    /// (`fuzz::pg_comparable` excludes `Expr::Row`), so a mapped `OVERLAPS` tree is never
    /// diffed against `pg_query`'s representation (which lowers it to an `overlaps`
    /// function call) — the shape exists for the mapper's totality and the golden snapshots.
    Overlaps,
    /// The `&&` overlap operator (array/range overlap in PostgreSQL, geometry
    /// bounding-box overlap in DuckDB). PostgreSQL lowers `&&` to a function call and the
    /// operator is outside the neutral structural corpus, so a mapped tree is never diffed
    /// against `pg_query`; the shape exists for the mapper's totality and golden snapshots.
    Overlap,
    Contains,
    ContainedBy,
    JsonGet,
    JsonGetText,
    /// The PostgreSQL `jsonb` existence/path/search operators (`?`/`?|`/`?&`/`@?`/`@@`/
    /// `#>`/`#>>`/`#-`), in the PostgreSQL shape vocabulary so the differential compares
    /// them structurally.
    JsonExists,
    JsonExistsAny,
    JsonExistsAll,
    JsonPathExists,
    JsonPathMatch,
    JsonExtractPath,
    JsonExtractPathText,
    JsonDeletePath,
    BitwiseOr,
    BitwiseAnd,
    BitwiseShiftLeft,
    BitwiseShiftRight,
    /// PostgreSQL bitwise XOR (`#`). The spelling tag is dropped: the structural
    /// comparison is representation-equivalent (ADR-0015), and only PostgreSQL's `#` ever
    /// reaches this PostgreSQL oracle.
    BitwiseXor,
    /// PostgreSQL exponentiation (`^`). Distinct from [`BitwiseXor`](Self::BitwiseXor):
    /// PostgreSQL's `^` is arithmetic power (`#` is its XOR), so the two never collide here.
    Exponent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOperatorShape {
    Not,
    Minus,
    Plus,
    BitwiseNot,
    Prior,
}

/// Neutral statement structure spanning the query and DDL/DML families the oracle
/// compares.
///
/// [`QueryShape`] stays the unit for plain `SELECT`/`VALUES`; the DDL/DML families
/// added by `prod-pg-map-ddl-dml` get their own statement-shape variants so the
/// protobuf-to-canonical mapping (ADR-0015) covers more than queries. The
/// transaction-control ([`Transaction`](Self::Transaction)), session
/// ([`Session`](Self::Session)), access-control ([`AccessControl`](Self::AccessControl)),
/// and `EXPLAIN` ([`Explain`](Self::Explain)) families are mapped too
/// (`pg-structural-oracle-for-dcl-tcl-utility`). Two utility surfaces stay an
/// explicit "not implemented" divergence rather than silent parity, by decision:
/// `COPY` (PostgreSQL canonicalizes its legacy/parenthesized/`FORCE`/`DELIMITERS`
/// option soup into one `DefElem` list, disproportionate to the low consumer risk)
/// and the special `SET` subforms (`TIME ZONE`/`ROLE`/`SESSION AUTHORIZATION`/
/// `CONSTRAINTS`/`NAMES`/`SESSION CHARACTERISTICS`), which PostgreSQL lowers to a
/// stringly-named `VariableSetStmt`/`ConstraintsSetStmt` whose recovery is fragile.
/// Both keep accept/reject parity plus render round-trip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatementShape {
    Query(QueryShape),
    CreateTable(CreateTableShape),
    AlterTable(AlterTableShape),
    Drop(DropShape),
    CreateSchema(CreateSchemaShape),
    CreateView(CreateViewShape),
    CreateIndex(CreateIndexShape),
    Insert(InsertShape),
    Update(UpdateShape),
    Delete(DeleteShape),
    Transaction(TransactionShape),
    Session(SessionShape),
    AccessControl(AccessControlShape),
    Explain(ExplainShape),
    Truncate(TruncateShape),
    CommentOn(CommentOnShape),
}

// ---- CREATE TABLE ----------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateTableShape {
    pub temporary: bool,
    /// `CREATE UNLOGGED TABLE`. PostgreSQL reports it as `relpersistence == 'u'` (a peer of
    /// the temporary `'t'`), on both the `CreateStmt` and `CreateTableAsStmt` relations.
    pub unlogged: bool,
    pub if_not_exists: bool,
    pub name: Vec<String>,
    pub body: CreateTableBodyShape,
    /// The `USING <access_method>` table access method (PostgreSQL `CreateStmt.accessMethod`);
    /// `None` when unwritten (PostgreSQL reports an empty string).
    pub access_method: Option<String>,
    pub options: TableOptionsShape,
}

/// The two `CREATE TABLE` bodies. PostgreSQL parses `CREATE TABLE ... (cols)` to a
/// `CreateStmt` but `CREATE TABLE ... AS <query>` to a `CreateTableAsStmt`; our
/// single `CreateTable` node distinguishes them by body, so both sides converge
/// here on one shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CreateTableBodyShape {
    Definition(Vec<TableElementShape>),
    AsQuery {
        columns: Vec<String>,
        query: Box<QueryShape>,
        /// `true` for `WITH NO DATA`. PostgreSQL always materializes a populate flag
        /// (`skip_data`), so the unwritten default and an explicit `WITH DATA` both
        /// map to `false` (ADR-0015 representation-equivalence).
        no_data: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableElementShape {
    Column(TableColumnShape),
    Constraint(TableConstraintShape),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableColumnShape {
    pub name: String,
    pub data_type: DataTypeShape,
    /// The column `COLLATE <name>` clause as its qualified name parts. PostgreSQL hangs the
    /// clause *on the column* (`ColumnDef.collClause`), not in the constraint list, so our
    /// side lifts the parsed collate constraint up here to compare position-independently;
    /// `None` when unwritten.
    pub collation: Option<Vec<String>>,
    /// The `STORAGE <strategy>` clause (PostgreSQL `ColumnDef.storage_name`, a downcased
    /// `ColId` or `default` for the `DEFAULT` keyword); `None` when unwritten.
    pub storage: Option<String>,
    /// The `COMPRESSION <method>` clause (PostgreSQL `ColumnDef.compression`); `None` when
    /// unwritten.
    pub compression: Option<String>,
    pub constraints: Vec<ColumnConstraintShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnConstraintShape {
    pub name: Option<String>,
    pub option: ColumnOptionShape,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColumnOptionShape {
    Null,
    NotNull,
    Default(ExprShape),
    /// `GENERATED ALWAYS AS (<expr>) STORED`. PostgreSQL only supports stored
    /// generated columns and does not encode the keyword separately, so `stored` is
    /// always `true` on the PostgreSQL side; a parsed `VIRTUAL` (which PostgreSQL
    /// rejects) would surface as a structural difference.
    Generated {
        expr: ExprShape,
        stored: bool,
    },
    Identity {
        generation: IdentityGenerationShape,
        options: Vec<IdentityOptionShape>,
    },
    PrimaryKey,
    Unique,
    /// MySQL `AUTO_INCREMENT`; never produced by the PostgreSQL oracle (libpg_query
    /// rejects it), so it only appears on our side of a non-PostgreSQL comparison.
    AutoIncrement,
    /// A column `COLLATE <name>` kept *in the constraint list*: a named (`CONSTRAINT c
    /// COLLATE …`, SQLite-only) or repeated collate that the first-collate lift into
    /// [`TableColumnShape::collation`] leaves behind. Never produced by the PostgreSQL oracle
    /// (libpg_query hangs the one legal collate on the column and rejects the named/repeated
    /// forms), so like `AutoIncrement` it only appears on our side of a non-PostgreSQL
    /// comparison.
    Collate(Vec<String>),
    Check(ExprShape),
    References {
        table: Vec<String>,
        columns: Vec<String>,
        actions: ForeignKeyActionsShape,
    },
}

/// The foreign-key `MATCH` / `ON DELETE` / `ON UPDATE` clauses, normalized to the
/// effective action PostgreSQL records.
///
/// PostgreSQL always materializes these (defaulting `MATCH SIMPLE` / `NO ACTION`)
/// and cannot distinguish an omitted clause from its explicit default, so each
/// field is non-optional and an absent clause maps to the default — the same
/// representation-equivalence normalization [`DropShape`] applies to drop
/// behaviour (ADR-0015).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignKeyActionsShape {
    pub match_type: ForeignKeyMatchShape,
    pub on_delete: ReferentialActionShape,
    pub on_update: ReferentialActionShape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForeignKeyMatchShape {
    Full,
    Partial,
    Simple,
}

/// A `<referential action>`. `SetNull`/`SetDefault` carry the PostgreSQL `ON
/// DELETE` column list (`fk_del_set_cols`), empty for the bare form and for every
/// `ON UPDATE` action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferentialActionShape {
    NoAction,
    Restrict,
    Cascade,
    SetNull { columns: Vec<String> },
    SetDefault { columns: Vec<String> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityGenerationShape {
    Always,
    ByDefault,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityOptionShape {
    StartWith(ExprShape),
    IncrementBy(ExprShape),
    /// `None` is `NO MINVALUE` (PostgreSQL `minvalue` `DefElem` with no argument).
    MinValue(Option<ExprShape>),
    /// `None` is `NO MAXVALUE`.
    MaxValue(Option<ExprShape>),
    Cache(ExprShape),
    Cycle(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableConstraintShape {
    pub name: Option<String>,
    pub kind: TableConstraintKindShape,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TableConstraintKindShape {
    PrimaryKey(Vec<String>),
    Unique(Vec<String>),
    Check(ExprShape),
    ForeignKey {
        columns: Vec<String>,
        table: Vec<String>,
        ref_columns: Vec<String>,
        actions: ForeignKeyActionsShape,
    },
}

/// The post-definition table options, compared as PostgreSQL stores them — three
/// independent slots rather than an ordered list.
///
/// PostgreSQL splits `WITH (...)`, `ON COMMIT`, and `TABLESPACE` across separate
/// protobuf fields (`options` / `oncommit` / `tablespacename`), losing the source
/// order our AST keeps in one list, so the shape compares each slot rather than a
/// positional list (ADR-0015 representation-equivalence).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TableOptionsShape {
    pub with_params: Vec<StorageParamShape>,
    pub on_commit: Option<OnCommitShape>,
    pub tablespace: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageParamShape {
    pub name: Vec<String>,
    pub value: Option<ExprShape>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnCommitShape {
    PreserveRows,
    DeleteRows,
    Drop,
}

// ---- ALTER TABLE -----------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlterTableShape {
    pub if_exists: bool,
    pub name: Vec<String>,
    pub actions: Vec<AlterTableActionShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlterTableActionShape {
    AddColumn {
        if_not_exists: bool,
        column: TableColumnShape,
    },
    DropColumn {
        if_exists: bool,
        name: String,
        cascade: bool,
    },
    AlterColumn {
        name: String,
        change: AlterColumnActionShape,
    },
    AddConstraint(TableConstraintShape),
    DropConstraint {
        if_exists: bool,
        name: String,
        cascade: bool,
    },
    SetOptions(Vec<StorageParamShape>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlterColumnActionShape {
    SetDefault(ExprShape),
    DropDefault,
    SetNotNull,
    DropNotNull,
    AddIdentity {
        generation: IdentityGenerationShape,
        options: Vec<IdentityOptionShape>,
    },
    SetDataType {
        data_type: DataTypeShape,
        using: Option<ExprShape>,
    },
}

// ---- DROP ------------------------------------------------------------------

/// `DROP {TABLE|VIEW|INDEX|SCHEMA} ...`. PostgreSQL always materializes a drop
/// behaviour (defaulting to `RESTRICT`), so the explicit-vs-implicit `RESTRICT`
/// distinction our AST keeps is not observable there; the shape normalizes to a
/// `cascade` flag (ADR-0015 representation-equivalence).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropShape {
    pub object_kind: DropObjectKindShape,
    pub if_exists: bool,
    pub names: Vec<Vec<String>>,
    pub cascade: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropObjectKindShape {
    Table,
    View,
    Index,
    Schema,
}

// ---- TRUNCATE --------------------------------------------------------------

/// `TRUNCATE [TABLE] <name> [, ...] [RESTART|CONTINUE IDENTITY] [CASCADE|RESTRICT]`.
/// PostgreSQL materializes only a `restart_seqs` bool (no "unspecified" state), so our
/// absent and `CONTINUE IDENTITY` forms both normalize to `restart_identity: false`,
/// the same representation-equivalence (ADR-0015) the `cascade` flag uses for DROP.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TruncateShape {
    pub names: Vec<Vec<String>>,
    pub restart_identity: bool,
    pub cascade: bool,
}

// ---- COMMENT ON ------------------------------------------------------------

/// `COMMENT ON <object> IS '<text>' | NULL`. Only TABLE/COLUMN/DATABASE map here;
/// `PROCEDURE` stays an explicit not-implemented divergence because its
/// `ObjectWithArgs` signature canonicalizes argument types on the PostgreSQL side
/// (`integer` -> `int4`), which the neutral shape does not model. PostgreSQL cannot
/// distinguish `IS NULL` from `IS ''` (both lower to an empty comment string), so an
/// empty comment normalizes to `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentOnShape {
    pub target: CommentTargetShape,
    pub name: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentTargetShape {
    Table,
    Column,
    Database,
}

// ---- CREATE SCHEMA ---------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateSchemaShape {
    pub if_not_exists: bool,
    pub name: Option<Vec<String>>,
    pub authorization: Option<String>,
}

// ---- CREATE [MATERIALIZED] VIEW --------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateViewShape {
    pub or_replace: bool,
    pub materialized: bool,
    pub temporary: bool,
    pub if_not_exists: bool,
    pub name: Vec<String>,
    pub columns: Vec<String>,
    pub query: Box<QueryShape>,
    pub check_option: Option<ViewCheckOptionShape>,
    /// `true` for `WITH NO DATA` on a materialized view; see
    /// [`CreateTableBodyShape::AsQuery`].
    pub no_data: bool,
}

/// `WITH CHECK OPTION` level. The bare `WITH CHECK OPTION` (`ViewCheckOption::
/// Unspecified` on our side) means `CASCADED` and PostgreSQL reports it as
/// `CascadedCheckOption`, so the shape has no `Unspecified` variant — both
/// spellings normalize to [`Cascaded`](Self::Cascaded). `None` is no check option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewCheckOptionShape {
    Cascaded,
    Local,
}

// ---- CREATE INDEX ----------------------------------------------------------

/// `CREATE [UNIQUE] INDEX ...`. PostgreSQL always materializes the access method
/// (defaulting to `btree`), so an omitted `USING` and an explicit `USING btree` are
/// indistinguishable there; the shape normalizes `method` to the resolved
/// lower-cased name (ADR-0015 representation-equivalence).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateIndexShape {
    pub unique: bool,
    pub concurrently: bool,
    pub if_not_exists: bool,
    pub name: Option<String>,
    pub table: Vec<String>,
    pub method: String,
    pub columns: Vec<IndexColumnShape>,
    pub predicate: Option<ExprShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexColumnShape {
    pub expr: ExprShape,
    pub asc: Option<bool>,
    pub nulls_first: Option<bool>,
}

// ---- INSERT ----------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertShape {
    pub with: Option<WithShape>,
    pub target: InsertTargetShape,
    pub overriding: Option<InsertOverriding>,
    pub source: InsertSourceShape,
    pub on_conflict: Option<OnConflictShape>,
    /// `RETURNING` output list (a projection); empty when absent.
    pub returning: Vec<SelectItemShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertTargetShape {
    pub name: Vec<String>,
    pub alias: Option<String>,
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InsertSourceShape {
    DefaultValues,
    Values(Vec<Vec<InsertItemShape>>),
    Query(Box<QueryShape>),
    /// The MySQL `SET <col> = <value>` assignment-list source. PostgreSQL has no such
    /// form, so this shape never arises on the PostgreSQL differential path; it exists
    /// to keep the conversion exhaustive over the canonical [`InsertSource`].
    Set(Vec<UpdateAssignmentShape>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InsertItemShape {
    Expr(ExprShape),
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnConflictShape {
    pub target: Option<ConflictTargetShape>,
    pub action: ConflictActionShape,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictTargetShape {
    Index {
        columns: Vec<ExprShape>,
        predicate: Option<ExprShape>,
    },
    Constraint(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictActionShape {
    Nothing,
    Update {
        assignments: Vec<UpdateAssignmentShape>,
        selection: Option<ExprShape>,
    },
}

// ---- UPDATE / DELETE -------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateShape {
    pub with: Option<WithShape>,
    pub target: DmlTargetShape,
    pub assignments: Vec<UpdateAssignmentShape>,
    pub from: Vec<TableWithJoinsShape>,
    pub selection: Option<DmlSelectionShape>,
    pub returning: Vec<SelectItemShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeleteShape {
    pub with: Option<WithShape>,
    pub target: DmlTargetShape,
    pub using: Vec<TableWithJoinsShape>,
    pub selection: Option<DmlSelectionShape>,
    pub returning: Vec<SelectItemShape>,
}

/// A `MERGE INTO … USING … ON … WHEN …` statement (currently reached only as a
/// [`CteBodyShape::Merge`] body; the top-level `Statement::Merge` stays an explicit
/// not-implemented divergence on both sides).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeShape {
    pub with: Option<WithShape>,
    pub target: DmlTargetShape,
    pub using: TableWithJoinsShape,
    pub on: ExprShape,
    pub clauses: Vec<MergeWhenShape>,
    pub returning: Vec<SelectItemShape>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeWhenShape {
    pub match_kind: MergeMatchShape,
    pub condition: Option<ExprShape>,
    pub action: MergeActionShape,
}

/// Which `WHEN` production a merge arm is. A bare `NOT MATCHED` and `NOT MATCHED BY
/// TARGET` are one production on both sides (pg_query's `matchKind` folds them), so
/// both map to [`NotMatchedByTarget`](Self::NotMatchedByTarget).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MergeMatchShape {
    Matched,
    NotMatchedByTarget,
    NotMatchedBySource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MergeActionShape {
    Insert {
        columns: Vec<String>,
        overriding: Option<InsertOverriding>,
        values: Vec<InsertItemShape>,
    },
    /// `INSERT DEFAULT VALUES` — no column list, no override, no value row.
    InsertDefault,
    Update {
        assignments: Vec<UpdateAssignmentShape>,
    },
    Delete,
    DoNothing,
    UpdateStar,
    InsertStar,
    InsertByName {
        star: bool,
    },
    Error,
}

/// The `UPDATE`/`DELETE` target relation. PostgreSQL only records inheritance
/// suppression as `!inh`, so the bare `ONLY t` and parenthesized `ONLY (t)`
/// spellings our AST distinguishes both map to `only: true`, while the bare `t`
/// and explicit-descendants `t *` spellings both map to `only: false`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DmlTargetShape {
    pub name: Vec<String>,
    pub only: bool,
    pub alias: Option<String>,
}

/// One `SET` assignment. PostgreSQL flattens a multiple-column `(a, b) = source`
/// into one `MultiAssignRef`-bearing target per column; `pg_update_assignments`
/// re-groups those back into a [`Tuple`](Self::Tuple) so both sides share this
/// grouped shape (ADR-0015 representation-equivalence).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateAssignmentShape {
    Single {
        target: Vec<String>,
        value: UpdateValueShape,
    },
    Tuple {
        targets: Vec<Vec<String>>,
        source: UpdateTupleSourceShape,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateValueShape {
    Expr(ExprShape),
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateTupleSourceShape {
    /// `( ... )`, or `ROW( ... )` when `explicit` (PostgreSQL `RowExpr` with an
    /// explicit-call coercion form).
    Row {
        explicit: bool,
        values: Vec<UpdateValueShape>,
    },
    Subquery(Box<QueryShape>),
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DmlSelectionShape {
    Where(ExprShape),
    CurrentOf(String),
}

// ---- TRANSACTION CONTROL ---------------------------------------------------

/// A transaction-control statement. PostgreSQL splits the operational verbs across a
/// `TransactionStmt` (`BEGIN`/`START`/`COMMIT`/`ROLLBACK`/`SAVEPOINT`/`RELEASE`) but
/// lowers `SET TRANSACTION` to a `VariableSetStmt` (name `TRANSACTION`); both
/// converge here. The interchangeable `WORK`/`TRANSACTION` noise words carry no
/// meaning and are not represented, matching PostgreSQL (ADR-0011).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransactionShape {
    /// `BEGIN` / `START TRANSACTION [<mode> ...]`. `start` distinguishes the `START
    /// TRANSACTION` spelling (PostgreSQL `TransStmtStart`) from the `BEGIN` spelling
    /// (`TransStmtBegin`); both accept the same mode list.
    Begin {
        start: bool,
        modes: Vec<TransactionModeShape>,
    },
    Commit,
    /// `ROLLBACK [TO [SAVEPOINT] <name>]`; `to_savepoint` is `Some` for the rewind.
    Rollback {
        to_savepoint: Option<String>,
    },
    Savepoint(String),
    Release(String),
    /// `SET TRANSACTION <mode> ...`.
    SetCharacteristics {
        modes: Vec<TransactionModeShape>,
    },
}

/// One transaction mode, mirroring PostgreSQL's `DefElem` encoding: an isolation
/// level (`transaction_isolation`), the read/write flag (`transaction_read_only`),
/// the deferrable flag (`transaction_deferrable`), or MySQL's consistent-snapshot
/// start characteristic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransactionModeShape {
    IsolationLevel(IsolationLevelShape),
    /// `true` = `READ ONLY`, `false` = `READ WRITE`.
    ReadOnly(bool),
    /// `true` = `DEFERRABLE`, `false` = `NOT DEFERRABLE`.
    Deferrable(bool),
    ConsistentSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsolationLevelShape {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

// ---- SESSION (SET / RESET / SHOW) ------------------------------------------

/// A run-time-configuration statement. Only the generic `SET <name> {= | TO}
/// <value>`, `RESET`, and `SHOW` forms are shaped; the special `SET` subforms stay
/// accept-only (see [`StatementShape`]). PostgreSQL collapses a bareword and a
/// string value to one string constant, so both map to [`LiteralShape::String`]
/// (representation-equivalence, ADR-0015).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionShape {
    Set {
        local: bool,
        name: String,
        value: SetValueShape,
    },
    /// `RESET <name>` / `RESET ALL`; `None` is `ALL` (PostgreSQL `VarResetAll`).
    Reset { name: Option<String> },
    /// `SHOW <name>` / `SHOW ALL`; `ALL` maps to `name = "all"` (PostgreSQL lowers it
    /// to the pseudo-parameter name, unlike `RESET ALL`).
    Show { name: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetValueShape {
    Default,
    Values(Vec<SetParameterValueShape>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SetParameterValueShape {
    Literal(LiteralShape),
    PositionalParameter(i32),
}

// ---- ACCESS CONTROL (GRANT / REVOKE) ---------------------------------------

/// A `GRANT`/`REVOKE` statement. PostgreSQL folds both directions into one
/// `GrantStmt` (privilege grants) or `GrantRoleStmt` (role-membership grants) with an
/// `is_grant` flag, so our separate `Grant`/`Revoke`/`GrantRole`/`RevokeRole`
/// variants converge onto these two shapes. PostgreSQL always materializes the
/// `<drop behavior>` (defaulting `RESTRICT`) and drops both the legacy `GROUP`
/// grantee prefix and the redundant `TABLE` object keyword, so the shape normalizes
/// drop behaviour to a `cascade` flag and does not carry those surface tags
/// (ADR-0015).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessControlShape {
    RoleRename {
        name: String,
        new_name: String,
    },
    Privilege {
        is_grant: bool,
        /// `WITH GRANT OPTION` (grant) / `GRANT OPTION FOR` (revoke).
        grant_option: bool,
        privileges: PrivilegesShape,
        object: GrantObjectShape,
        grantees: Vec<RoleSpecShape>,
        granted_by: Option<RoleSpecShape>,
        /// `REVOKE ... CASCADE`; always `false` for `GRANT`.
        cascade: bool,
    },
    Role {
        is_grant: bool,
        /// `WITH ADMIN OPTION` (grant) / `ADMIN OPTION FOR` (revoke).
        admin_option: bool,
        roles: Vec<String>,
        grantees: Vec<RoleSpecShape>,
        granted_by: Option<RoleSpecShape>,
        cascade: bool,
    },
}

/// The privileges of a privilege `GRANT`/`REVOKE`: `ALL` (PostgreSQL's empty list) or
/// a specific list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrivilegesShape {
    All,
    List(Vec<PrivilegeShape>),
}

/// One privilege: a lower-cased name and its optional column scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivilegeShape {
    pub name: String,
    pub columns: Vec<String>,
}

/// The object a privilege applies to, keyed by the object class PostgreSQL records in
/// `objtype`/`targtype`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrantObjectShape {
    /// A plain-name object list (`[TABLE] ...`, `SEQUENCE ...`, `DATABASE ...`, ...).
    Objects {
        kind: GrantObjectKindShape,
        names: Vec<Vec<String>>,
    },
    /// `{FUNCTION | PROCEDURE | ROUTINE} <signature> ...`.
    Routines {
        kind: RoutineKindShape,
        routines: Vec<RoutineSignatureShape>,
    },
    /// `ALL {TABLES | SEQUENCES | ...} IN SCHEMA <schema> ...`.
    AllInSchema {
        class: SchemaClassShape,
        schemas: Vec<Vec<String>>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantObjectKindShape {
    Table,
    Sequence,
    Database,
    Schema,
    Domain,
    Type,
    Language,
    Tablespace,
    ForeignDataWrapper,
    ForeignServer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutineKindShape {
    Function,
    Procedure,
    Routine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaClassShape {
    Tables,
    Sequences,
    Functions,
    Procedures,
    Routines,
}

/// A routine reference: a (possibly qualified) name and an optional argument-type
/// list. `arg_types` is `None` when no parentheses were written (`FUNCTION foo`,
/// PostgreSQL `args_unspecified`) and `Some` — possibly empty — when they were,
/// preserving the `foo` vs `foo()` distinction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineSignatureShape {
    pub name: Vec<String>,
    pub arg_types: Option<Vec<DataTypeShape>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoleSpecShape {
    Public,
    CurrentRole,
    CurrentUser,
    SessionUser,
    Name(String),
}

// ---- EXPLAIN ---------------------------------------------------------------

/// An `EXPLAIN <statement>`. PostgreSQL lowers the legacy `ANALYZE`/`VERBOSE` keyword
/// prefix and the parenthesized option list to one `DefElem` list and does not record
/// which spelling was written, so the shape drops the `parenthesized` surface tag and
/// compares the option list plus the inner statement's own shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplainShape {
    pub options: Vec<ExplainOptionShape>,
    pub statement: Box<StatementShape>,
}

/// One `EXPLAIN` option: a lower-cased name and its optional lower-cased argument
/// (`ANALYZE`, `ANALYZE true`, `FORMAT json`, `COSTS off`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplainOptionShape {
    pub name: String,
    pub value: Option<String>,
}

/// Extract the neutral shape from a `squonk` parse.
pub fn squonk_shape(parsed: &Parsed) -> Vec<StatementShape> {
    squonk_shape_result(parsed)
        .unwrap_or_else(|err| panic!("unsupported squonk structural shape: {err}"))
}

// `pub(crate)`: the DuckDB structural oracle (`crate::duckdb_structural`) builds our
// side of its differential from the same neutral shapes, and needs the fallible form
// to skip (never panic on) a statement kind the neutral model does not cover — exactly
// as `pg_structural_divergence` above uses it. Visibility only; no behaviour change.
pub(crate) fn squonk_shape_result(parsed: &Parsed) -> Result<Vec<StatementShape>, String> {
    parsed
        .statements()
        .iter()
        .map(|stmt| squonk_statement_shape(parsed, stmt))
        .collect()
}

/// Map one `squonk` statement to its neutral shape. Split out from
/// [`squonk_shape_result`] so [`EXPLAIN`](StatementShape::Explain) can recurse
/// into its inner statement.
fn squonk_statement_shape(
    parsed: &Parsed,
    stmt: &Statement<NoExt>,
) -> Result<StatementShape, String> {
    match stmt {
        Statement::Query { query, .. } => Ok(StatementShape::Query(query_shape(parsed, query))),
        Statement::CreateTable { create, .. } => {
            squonk_create_table_shape(parsed, create).map(StatementShape::CreateTable)
        }
        Statement::AlterTable { alter, .. } => {
            squonk_alter_table_shape(parsed, alter).map(StatementShape::AlterTable)
        }
        Statement::Drop { drop, .. } => squonk_drop_shape(parsed, drop).map(StatementShape::Drop),
        Statement::CreateSchema { schema, .. } => Ok(StatementShape::CreateSchema(
            squonk_create_schema_shape(parsed, schema),
        )),
        Statement::CreateView { view, .. } => Ok(StatementShape::CreateView(
            squonk_create_view_shape(parsed, view),
        )),
        Statement::CreateIndex { index, .. } => Ok(StatementShape::CreateIndex(
            squonk_create_index_shape(parsed, index),
        )),
        Statement::Insert { insert, .. } => {
            Ok(StatementShape::Insert(squonk_insert_shape(parsed, insert)))
        }
        Statement::Update { update, .. } => {
            Ok(StatementShape::Update(squonk_update_shape(parsed, update)))
        }
        Statement::Delete { delete, .. } => {
            Ok(StatementShape::Delete(squonk_delete_shape(parsed, delete)))
        }
        Statement::Transaction { transaction, .. } => Ok(StatementShape::Transaction(
            squonk_transaction_shape(parsed, transaction),
        )),
        Statement::Session { session, .. } => {
            squonk_session_shape(parsed, session).map(StatementShape::Session)
        }
        Statement::AccessControl { access, .. } => Ok(StatementShape::AccessControl(
            squonk_access_control_shape(parsed, access),
        )),
        Statement::Explain { explain, .. } => {
            squonk_explain_shape(parsed, explain).map(StatementShape::Explain)
        }
        Statement::Truncate {
            tables,
            restart_identity,
            behavior,
            ..
        } => Ok(StatementShape::Truncate(TruncateShape {
            names: tables
                .iter()
                .map(|name| object_name_shape(parsed, name))
                .collect(),
            restart_identity: matches!(restart_identity, Some(true)),
            cascade: matches!(behavior, Some(DropBehavior::Cascade)),
        })),
        Statement::CommentOn { comment, .. } => {
            squonk_comment_on_shape(parsed, comment).map(StatementShape::CommentOn)
        }
        Statement::Other { ext, .. } => match *ext {},
        // `COPY` and the special `SET` subforms stay an explicit "not implemented"
        // divergence rather than silent parity (ADR-0015); see [`StatementShape`].
        _ => Err(
            "PostgreSQL structural shape mapping for this statement kind is not implemented"
                .to_owned(),
        ),
    }
}

// ---- our AST -> statement shape --------------------------------------------

fn squonk_create_table_shape(
    parsed: &Parsed,
    create: &CreateTable<NoExt>,
) -> Result<CreateTableShape, String> {
    // Declarative partitioning has no neutral-shape mapping yet (the `PartitionSpec` /
    // `PartitionBound` grammar is unmodelled on the PG protobuf side too), so a partitioned
    // parent or a `PARTITION OF` child surfaces as an explicit not-implemented divergence
    // (ADR-0015), covered by accept/reject and the fingerprint-mediated lane instead — mirroring
    // the `ALTER TABLE RENAME` precedent below.
    if create.partition_by.is_some() {
        return Err(
            "PostgreSQL structural shape mapping for PARTITION BY is not implemented".to_owned(),
        );
    }
    // The legacy `INHERITS (parents)` clause and the `(LIKE src …)` source-table copy element
    // are likewise unmodelled on the neutral-shape side, so a table carrying either surfaces as
    // an explicit not-implemented divergence (ADR-0015), covered by accept/reject parity instead.
    if !create.inherits.is_empty() {
        return Err(
            "PostgreSQL structural shape mapping for INHERITS is not implemented".to_owned(),
        );
    }
    if let CreateTableBody::Definition { elements, .. } = &create.body {
        if elements
            .iter()
            .any(|element| matches!(element, TableElement::Like { .. }))
        {
            return Err(
                "PostgreSQL structural shape mapping for the LIKE element is not implemented"
                    .to_owned(),
            );
        }
        // An `EXCLUDE` exclusion constraint has no neutral-shape mapping (the PG protobuf side
        // errors on `ConstrExclusion` too), so a table carrying one surfaces as an explicit
        // not-implemented divergence before any element is mapped — the `LIKE`/`PARTITION`
        // precedent — covered by accept/reject and the fingerprint-mediated lane instead.
        if elements.iter().any(|element| {
            matches!(
                element,
                TableElement::Constraint { constraint, .. }
                    if matches!(constraint.constraint, TableConstraint::Exclude { .. })
            )
        }) {
            return Err(
                "PostgreSQL structural shape mapping for the EXCLUDE constraint is not implemented"
                    .to_owned(),
            );
        }
    }
    let body = match &create.body {
        CreateTableBody::Definition { elements, .. } => CreateTableBodyShape::Definition(
            elements
                .iter()
                .map(|element| squonk_table_element_shape(parsed, element))
                .collect(),
        ),
        CreateTableBody::AsQuery {
            columns,
            query,
            with_data,
            ..
        } => CreateTableBodyShape::AsQuery {
            columns: idents_shape(parsed, columns),
            query: Box::new(query_shape(parsed, query)),
            no_data: matches!(with_data, Some(false)),
        },
        CreateTableBody::PartitionOf { .. } => {
            return Err(
                "PostgreSQL structural shape mapping for PARTITION OF is not implemented"
                    .to_owned(),
            );
        }
        // A typed table's augmentation body carries typeless column overrides, unmodelled on
        // the neutral-shape side (the PG mapper gates on `of_typename` the same way), so it
        // surfaces as an explicit not-implemented divergence (ADR-0015), covered by
        // accept/reject and the fingerprint-mediated lane instead — the `PARTITION OF`
        // precedent above.
        CreateTableBody::OfType { .. } => {
            return Err(
                "PostgreSQL structural shape mapping for typed (OF) tables is not implemented"
                    .to_owned(),
            );
        }
        // `AS EXECUTE <prepared>` is a prepared-statement CTAS source, unmodelled on the
        // neutral-shape side (like `PARTITION OF`/`OF`), so it surfaces as an explicit
        // not-implemented divergence — covered by accept/reject and the fingerprint-mediated lane.
        CreateTableBody::AsExecute { .. } => {
            return Err(
                "PostgreSQL structural shape mapping for AS EXECUTE is not implemented".to_owned(),
            );
        }
        // MySQL's statement-level `LIKE <source>` clone body is a MySQL-only surface (PostgreSQL
        // spells only the copy element), unmodelled on the neutral-shape side like the
        // `PARTITION OF`/`OF`/`AS EXECUTE` precedents above — covered by accept/reject instead.
        CreateTableBody::LikeSource { .. } => {
            return Err(
                "PostgreSQL structural shape mapping for statement-level LIKE is not implemented"
                    .to_owned(),
            );
        }
    };
    Ok(CreateTableShape {
        temporary: create.temporary.is_some(),
        unlogged: create.unlogged,
        if_not_exists: create.if_not_exists,
        name: object_name_shape(parsed, &create.name),
        body,
        access_method: create
            .access_method
            .as_deref()
            .map(|method| ident_shape(parsed, method)),
        options: squonk_table_options_shape(parsed, &create.options),
    })
}

fn squonk_table_element_shape(parsed: &Parsed, element: &TableElement<NoExt>) -> TableElementShape {
    match element {
        TableElement::Column { column, .. } => {
            TableElementShape::Column(squonk_table_column_shape(parsed, column))
        }
        TableElement::Constraint { constraint, .. } => {
            TableElementShape::Constraint(squonk_table_constraint_shape(parsed, constraint))
        }
        // A `LIKE src …` copy element has no neutral-shape mapping; `squonk_create_table_shape`
        // returns an explicit not-implemented divergence before mapping any element, so this arm
        // is never reached from a table carrying one.
        TableElement::Like { .. } => {
            unreachable!("a LIKE element short-circuits create-table shape mapping")
        }
    }
}

fn squonk_table_column_shape(parsed: &Parsed, column: &ColumnDef<NoExt>) -> TableColumnShape {
    // PostgreSQL hangs the column collation on `ColumnDef.collClause`, not in the constraint
    // list, so the first *unnamed* parsed collate constraint lifts into the column-level slot
    // to compare position-independently; any further collate (a repeat, or a named
    // `CONSTRAINT c COLLATE` — both SQLite-only, unreachable on the PostgreSQL differential
    // path) stays in the constraint list and diverges loudly rather than silently matching.
    let mut collation = None;
    let mut constraints = Vec::new();
    for constraint in &column.constraints {
        if let ColumnOption::Collate {
            collation: name, ..
        } = &constraint.option
        {
            if constraint.name.is_none() && collation.is_none() {
                collation = Some(object_name_shape(parsed, name));
                continue;
            }
        }
        constraints.push(ColumnConstraintShape {
            name: opt_ident_shape(parsed, &constraint.name),
            option: squonk_column_option_shape(parsed, &constraint.option),
        });
    }
    TableColumnShape {
        name: ident_shape(parsed, &column.name),
        // PostgreSQL columns always declare a type (the typeless form is SQLite-only),
        // so the differential path never sees a `None` here.
        data_type: squonk_data_type_shape(
            parsed,
            column
                .data_type
                .as_ref()
                .expect("PostgreSQL columns declare a type"),
        ),
        collation,
        storage: column
            .storage
            .as_deref()
            .map(|storage| ident_shape(parsed, storage)),
        compression: column
            .compression
            .as_deref()
            .map(|compression| ident_shape(parsed, compression)),
        constraints,
    }
}

fn squonk_column_option_shape(parsed: &Parsed, option: &ColumnOption<NoExt>) -> ColumnOptionShape {
    match option {
        ColumnOption::Null { .. } => ColumnOptionShape::Null,
        ColumnOption::NotNull { .. } => ColumnOptionShape::NotNull,
        ColumnOption::Default { expr, .. } => ColumnOptionShape::Default(expr_shape(parsed, expr)),
        ColumnOption::Generated { generated, .. } => ColumnOptionShape::Generated {
            expr: expr_shape(parsed, &generated.expr),
            // PostgreSQL only has stored generated columns; a parsed `VIRTUAL`
            // (which PostgreSQL rejects) deliberately maps to `stored: false` so it
            // would diverge from the PostgreSQL side rather than silently match.
            stored: !matches!(generated.storage, Some(GeneratedColumnStorage::Virtual)),
        },
        ColumnOption::Identity { identity, .. } => ColumnOptionShape::Identity {
            generation: match identity.generation {
                IdentityGeneration::Always => IdentityGenerationShape::Always,
                IdentityGeneration::ByDefault => IdentityGenerationShape::ByDefault,
            },
            options: identity
                .options
                .iter()
                .map(|option| squonk_identity_option_shape(parsed, option))
                .collect(),
        },
        ColumnOption::PrimaryKey { .. } => ColumnOptionShape::PrimaryKey,
        ColumnOption::Unique { .. } => ColumnOptionShape::Unique,
        ColumnOption::AutoIncrement { .. } => ColumnOptionShape::AutoIncrement,
        ColumnOption::Collate { collation, .. } => {
            ColumnOptionShape::Collate(object_name_shape(parsed, collation))
        }
        ColumnOption::Check { expr, .. } => ColumnOptionShape::Check(expr_shape(parsed, expr)),
        ColumnOption::References { reference, .. } => ColumnOptionShape::References {
            table: object_name_shape(parsed, &reference.table),
            columns: idents_shape(parsed, &reference.columns),
            actions: squonk_foreign_key_actions_shape(parsed, reference),
        },
        ColumnOption::Bare { .. } => unreachable!(
            "a bare `CONSTRAINT <name>` is a SQLite/Lenient-only form, gated off for the \
             PostgreSQL/DuckDB dialects this shape mapper serves"
        ),
        ColumnOption::Other { ext, .. } => match *ext {},
    }
}

fn squonk_identity_option_shape(
    parsed: &Parsed,
    option: &IdentityOption<NoExt>,
) -> IdentityOptionShape {
    match option {
        IdentityOption::StartWith { expr, .. } => {
            IdentityOptionShape::StartWith(expr_shape(parsed, expr))
        }
        IdentityOption::IncrementBy { expr, .. } => {
            IdentityOptionShape::IncrementBy(expr_shape(parsed, expr))
        }
        IdentityOption::MinValue { value, .. } => {
            IdentityOptionShape::MinValue(value.as_ref().map(|expr| expr_shape(parsed, expr)))
        }
        IdentityOption::MaxValue { value, .. } => {
            IdentityOptionShape::MaxValue(value.as_ref().map(|expr| expr_shape(parsed, expr)))
        }
        IdentityOption::Cache { expr, .. } => IdentityOptionShape::Cache(expr_shape(parsed, expr)),
        IdentityOption::Cycle { cycle, .. } => IdentityOptionShape::Cycle(*cycle),
    }
}

fn squonk_table_constraint_shape(
    parsed: &Parsed,
    def: &TableConstraintDef<NoExt>,
) -> TableConstraintShape {
    TableConstraintShape {
        name: opt_ident_shape(parsed, &def.name),
        kind: match &def.constraint {
            TableConstraint::PrimaryKey { columns, .. } => {
                TableConstraintKindShape::PrimaryKey(constraint_columns_shape(parsed, columns))
            }
            TableConstraint::Unique { columns, .. } => {
                TableConstraintKindShape::Unique(constraint_columns_shape(parsed, columns))
            }
            TableConstraint::Check { expr, .. } => {
                TableConstraintKindShape::Check(expr_shape(parsed, expr))
            }
            TableConstraint::ForeignKey {
                columns,
                references,
                ..
            } => TableConstraintKindShape::ForeignKey {
                columns: idents_shape(parsed, columns),
                table: object_name_shape(parsed, &references.table),
                ref_columns: idents_shape(parsed, &references.columns),
                actions: squonk_foreign_key_actions_shape(parsed, references),
            },
            // An EXCLUDE constraint short-circuits create-table shape mapping (the pre-scan in
            // `squonk_create_table_shape` returns a not-implemented divergence), so this arm is
            // never reached from a table carrying one.
            TableConstraint::Exclude { .. } => {
                unreachable!("an EXCLUDE constraint short-circuits create-table shape mapping")
            }
            TableConstraint::Bare { .. } => unreachable!(
                "a bare `CONSTRAINT <name>` is a SQLite/Lenient-only form, gated off for the \
                 PostgreSQL/DuckDB dialects this shape mapper serves"
            ),
            TableConstraint::Other { ext, .. } => match *ext {},
        },
    }
}

/// Normalize the foreign-key action clauses of a parsed [`ForeignKeyRef`](squonk_ast::ForeignKeyRef). An
/// absent clause folds into PostgreSQL's effective default (`MATCH SIMPLE` /
/// `NO ACTION`), matching how the protobuf side reports them (ADR-0015).
fn squonk_foreign_key_actions_shape(
    parsed: &Parsed,
    reference: &squonk_ast::ForeignKeyRef,
) -> ForeignKeyActionsShape {
    ForeignKeyActionsShape {
        match_type: match reference.match_type {
            None | Some(ForeignKeyMatch::Simple) => ForeignKeyMatchShape::Simple,
            Some(ForeignKeyMatch::Full) => ForeignKeyMatchShape::Full,
            Some(ForeignKeyMatch::Partial) => ForeignKeyMatchShape::Partial,
        },
        on_delete: squonk_referential_action_shape(parsed, reference.on_delete.as_deref()),
        on_update: squonk_referential_action_shape(parsed, reference.on_update.as_deref()),
    }
}

fn squonk_referential_action_shape(
    parsed: &Parsed,
    action: Option<&ReferentialAction>,
) -> ReferentialActionShape {
    match action {
        None | Some(ReferentialAction::NoAction { .. }) => ReferentialActionShape::NoAction,
        Some(ReferentialAction::Restrict { .. }) => ReferentialActionShape::Restrict,
        Some(ReferentialAction::Cascade { .. }) => ReferentialActionShape::Cascade,
        Some(ReferentialAction::SetNull { columns, .. }) => ReferentialActionShape::SetNull {
            columns: idents_shape(parsed, columns),
        },
        Some(ReferentialAction::SetDefault { columns, .. }) => ReferentialActionShape::SetDefault {
            columns: idents_shape(parsed, columns),
        },
    }
}

fn squonk_table_options_shape(
    parsed: &Parsed,
    options: &[CreateTableOption<NoExt>],
) -> TableOptionsShape {
    let mut shape = TableOptionsShape::default();
    for option in options {
        match &option.kind {
            CreateTableOptionKind::With { params, .. } => {
                shape
                    .with_params
                    .extend(params.iter().map(|param| StorageParamShape {
                        name: object_name_shape(parsed, &param.name),
                        value: param.value.as_ref().map(|expr| expr_shape(parsed, expr)),
                    }));
            }
            CreateTableOptionKind::OnCommit { action, .. } => {
                shape.on_commit = Some(match action {
                    OnCommitAction::PreserveRows => OnCommitShape::PreserveRows,
                    OnCommitAction::DeleteRows => OnCommitShape::DeleteRows,
                    OnCommitAction::Drop => OnCommitShape::Drop,
                });
            }
            CreateTableOptionKind::Tablespace { tablespace, .. } => {
                shape.tablespace = Some(ident_shape(parsed, tablespace));
            }
            // MySQL-only (`KeyValue`) and SQLite-only (`WithoutRowid`/`Strict`) trailing
            // options; libpg_query (the PostgreSQL oracle) rejects them, so these arms are
            // unreachable on the differential path and record no shape — a PostgreSQL parse
            // never yields one.
            CreateTableOptionKind::KeyValue { .. }
            | CreateTableOptionKind::WithoutRowid { .. }
            | CreateTableOptionKind::Strict { .. } => {}
            // The legacy `WITHOUT OIDS` no-op records no shape *by fidelity*: PostgreSQL's
            // grammar drops it at raw parse (`OptWith: WITHOUT OIDS { NIL }`, engine-measured —
            // the protobuf carries no trace), so both sides erase it and the written and
            // omitted forms are representation-equivalent (ADR-0015).
            CreateTableOptionKind::WithoutOids { .. } => {}
            // These productions are outside PostgreSQL grammar and cannot occur after
            // both parsers accept the same input.
            CreateTableOptionKind::ColocateWith { .. }
            | CreateTableOptionKind::InColocationGroup { .. } => {}
        }
    }
    shape
}

fn squonk_alter_table_shape(
    parsed: &Parsed,
    alter: &AlterTable<NoExt>,
) -> Result<AlterTableShape, String> {
    Ok(AlterTableShape {
        if_exists: alter.if_exists,
        name: object_name_shape(parsed, &alter.name),
        actions: alter
            .actions
            .iter()
            .map(|action| squonk_alter_table_action_shape(parsed, action))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn alter_column_target_shape(parsed: &Parsed, target: &AlterColumnTarget) -> String {
    target
        .parts
        .iter()
        .map(|part| ident_shape(parsed, part))
        .collect::<Vec<_>>()
        .join(".")
}

fn squonk_alter_table_action_shape(
    parsed: &Parsed,
    action: &AlterTableAction<NoExt>,
) -> Result<AlterTableActionShape, String> {
    Ok(match action {
        AlterTableAction::AddColumn {
            if_not_exists,
            column,
            ..
        } => AlterTableActionShape::AddColumn {
            if_not_exists: *if_not_exists,
            column: squonk_table_column_shape(parsed, column),
        },
        AlterTableAction::DropColumn {
            if_exists,
            name,
            behavior,
            ..
        } => AlterTableActionShape::DropColumn {
            if_exists: *if_exists,
            name: alter_column_target_shape(parsed, name),
            cascade: matches!(behavior, Some(DropBehavior::Cascade)),
        },
        AlterTableAction::AlterColumn { name, change, .. } => AlterTableActionShape::AlterColumn {
            name: ident_shape(parsed, name),
            change: match change {
                AlterColumnAction::SetDefault { expr, .. } => {
                    AlterColumnActionShape::SetDefault(expr_shape(parsed, expr))
                }
                AlterColumnAction::DropDefault { .. } => AlterColumnActionShape::DropDefault,
                AlterColumnAction::SetNotNull { .. } => AlterColumnActionShape::SetNotNull,
                AlterColumnAction::DropNotNull { .. } => AlterColumnActionShape::DropNotNull,
                AlterColumnAction::AddIdentity { identity, .. } => {
                    AlterColumnActionShape::AddIdentity {
                        generation: match identity.generation {
                            IdentityGeneration::Always => IdentityGenerationShape::Always,
                            IdentityGeneration::ByDefault => IdentityGenerationShape::ByDefault,
                        },
                        options: identity
                            .options
                            .iter()
                            .map(|option| squonk_identity_option_shape(parsed, option))
                            .collect(),
                    }
                }
                AlterColumnAction::SetDataType {
                    data_type, using, ..
                } => AlterColumnActionShape::SetDataType {
                    data_type: squonk_data_type_shape(parsed, data_type),
                    using: using.as_ref().map(|expr| expr_shape(parsed, expr)),
                },
            },
        },
        AlterTableAction::AddConstraint { constraint, .. } => {
            AlterTableActionShape::AddConstraint(squonk_table_constraint_shape(parsed, constraint))
        }
        AlterTableAction::DropConstraint {
            if_exists,
            name,
            behavior,
            ..
        } => AlterTableActionShape::DropConstraint {
            if_exists: *if_exists,
            name: ident_shape(parsed, name),
            cascade: matches!(behavior, Some(DropBehavior::Cascade)),
        },
        // PostgreSQL parses the rename forms as a `RenameStmt`, a different top-level
        // node than the `AlterTableStmt` our AST folds them into, so they cannot reach
        // structural parity here; they surface as an explicit not-implemented
        // divergence (ADR-0015), covered by accept/reject parity instead.
        AlterTableAction::RenameColumn { .. }
        | AlterTableAction::RenameConstraint { .. }
        | AlterTableAction::RenameTable { .. } => {
            return Err(
                "PostgreSQL structural shape mapping for ALTER TABLE RENAME is not implemented"
                    .to_owned(),
            );
        }
        // Declarative-partitioning actions have no neutral-shape mapping yet (see the
        // `PARTITION BY` note on `squonk_create_table_shape`); covered by accept/reject
        // parity instead.
        AlterTableAction::AttachPartition { .. } | AlterTableAction::DetachPartition { .. } => {
            return Err(
                "PostgreSQL structural shape mapping for ALTER TABLE {ATTACH|DETACH} PARTITION is not implemented"
                    .to_owned(),
            );
        }
        AlterTableAction::SetOptions { params, .. } => AlterTableActionShape::SetOptions(
            params
                .iter()
                .map(|param| StorageParamShape {
                    name: object_name_shape(parsed, &param.name),
                    // PostgreSQL's raw tree stores option words, including TRUE/FALSE,
                    // as strings. Normalize the typed literal to that raw-tree shape.
                    value: param
                        .value
                        .as_ref()
                        .map(|expr| match expr_shape(parsed, expr) {
                            ExprShape::Literal(LiteralShape::Boolean(value)) => {
                                ExprShape::Literal(LiteralShape::String(value.to_string()))
                            }
                            value => value,
                        }),
                })
                .collect(),
        ),
        AlterTableAction::DropPrimaryKey { .. } => {
            return Err("DROP PRIMARY KEY is not PostgreSQL grammar".to_owned());
        }
        AlterTableAction::SetColocationGroup { .. }
        | AlterTableAction::DropColocationGroup { .. } => {
            return Err("colocation actions are not PostgreSQL grammar".to_owned());
        }
    })
}

fn squonk_drop_shape(parsed: &Parsed, drop: &DropStatement) -> Result<DropShape, String> {
    let object_kind = match drop.object_kind {
        DropObjectKind::Table => DropObjectKindShape::Table,
        DropObjectKind::View => DropObjectKindShape::View,
        DropObjectKind::Index => DropObjectKindShape::Index,
        DropObjectKind::Schema => DropObjectKindShape::Schema,
        // `DROP MATERIALIZED VIEW` parses (close-pg-verdict-ddl-tail-gaps) but is not
        // yet in the structural corpus, so it surfaces as an explicit not-implemented
        // divergence rather than silent parity (ADR-0015).
        DropObjectKind::MaterializedView => {
            return Err(
                "PostgreSQL structural shape mapping for DROP MATERIALIZED VIEW is not implemented"
                    .to_owned(),
            );
        }
        // `DROP TYPE` is DuckDB user-defined-type DDL (create-drop-type-udt-ddl); not in the
        // PG structural corpus, so it surfaces as an explicit not-implemented divergence
        // rather than silent parity (ADR-0015), like `DROP MATERIALIZED VIEW` above.
        DropObjectKind::Type => {
            return Err("structural shape mapping for DROP TYPE is not implemented".to_owned());
        }
        // `DROP SEQUENCE` is the T176 generator DDL (duckdb-create-sequence); not in the PG
        // structural corpus, so it surfaces as an explicit not-implemented divergence rather
        // than silent parity (ADR-0015), like `DROP TYPE` above.
        DropObjectKind::Sequence => {
            return Err("structural shape mapping for DROP SEQUENCE is not implemented".to_owned());
        }
        // `DROP MACRO [TABLE]` is DuckDB macro DDL (duckdb-drop-macro); not in the PG
        // structural corpus, so it surfaces as an explicit not-implemented divergence rather
        // than silent parity (ADR-0015), like `DROP SEQUENCE` above.
        DropObjectKind::Macro | DropObjectKind::MacroTable => {
            return Err("structural shape mapping for DROP MACRO is not implemented".to_owned());
        }
        // `DROP TRIGGER` is MySQL/SQLite trigger DDL (parse-mysql-trigger-ddl); not in the PG
        // structural corpus, so it surfaces as an explicit not-implemented divergence rather
        // than silent parity (ADR-0015), like `DROP MACRO` above.
        DropObjectKind::Trigger => {
            return Err("structural shape mapping for DROP TRIGGER is not implemented".to_owned());
        }
    };
    Ok(DropShape {
        object_kind,
        if_exists: drop.if_exists,
        names: drop
            .names
            .iter()
            .map(|name| object_name_shape(parsed, name))
            .collect(),
        cascade: matches!(drop.behavior, Some(DropBehavior::Cascade)),
    })
}

fn squonk_comment_on_shape(
    parsed: &Parsed,
    comment: &CommentOnStatement,
) -> Result<CommentOnShape, String> {
    let target = match &comment.target {
        CommentTarget::Table => CommentTargetShape::Table,
        CommentTarget::Column => CommentTargetShape::Column,
        CommentTarget::Database => CommentTargetShape::Database,
        // `PROCEDURE` argument types canonicalize on the PostgreSQL side, so it stays an
        // explicit not-implemented divergence (accept/reject parity still holds); see
        // [`CommentOnShape`]. `CommentTarget` is `#[non_exhaustive]`, so a future object
        // kind falls through the same way until it is mapped.
        CommentTarget::Procedure { .. } => {
            return Err(
                "PostgreSQL structural shape mapping for COMMENT ON PROCEDURE is not implemented"
                    .to_owned(),
            );
        }
        _ => {
            return Err(
                "PostgreSQL structural shape mapping for this COMMENT ON target is not implemented"
                    .to_owned(),
            );
        }
    };
    Ok(CommentOnShape {
        target,
        name: object_name_shape(parsed, &comment.name),
        comment: comment.comment.as_ref().map(|literal| {
            literal
                .as_str(parsed.source())
                .expect("parsed comment string should materialize")
                .into_owned()
        }),
    })
}

fn squonk_create_schema_shape(parsed: &Parsed, schema: &CreateSchema) -> CreateSchemaShape {
    CreateSchemaShape {
        if_not_exists: schema.if_not_exists,
        name: schema
            .name
            .as_ref()
            .map(|name| object_name_shape(parsed, name)),
        authorization: opt_ident_shape(parsed, &schema.authorization),
    }
}

fn squonk_create_view_shape(parsed: &Parsed, view: &CreateView<NoExt>) -> CreateViewShape {
    CreateViewShape {
        or_replace: view.or_replace,
        materialized: view.materialized,
        temporary: view.temporary.is_some(),
        if_not_exists: view.if_not_exists,
        name: object_name_shape(parsed, &view.name),
        columns: idents_shape(parsed, &view.columns),
        query: Box::new(query_shape(parsed, &view.query)),
        check_option: view.check_option.map(|option| match option {
            // PostgreSQL reports the bare `WITH CHECK OPTION` as CASCADED.
            ViewCheckOption::Unspecified | ViewCheckOption::Cascaded => {
                ViewCheckOptionShape::Cascaded
            }
            ViewCheckOption::Local => ViewCheckOptionShape::Local,
        }),
        no_data: matches!(view.with_data, Some(false)),
    }
}

fn squonk_create_index_shape(parsed: &Parsed, index: &CreateIndex<NoExt>) -> CreateIndexShape {
    CreateIndexShape {
        unique: index.unique,
        concurrently: index.concurrently,
        if_not_exists: index.if_not_exists,
        name: opt_ident_shape(parsed, &index.name),
        table: object_name_shape(parsed, &index.table),
        method: index
            .using
            .as_ref()
            .map(|ident| ident_shape(parsed, ident).to_ascii_lowercase())
            .unwrap_or_else(|| "btree".to_string()),
        columns: index
            .columns
            .iter()
            .map(|column| IndexColumnShape {
                expr: expr_shape(parsed, &column.expr),
                asc: column.asc,
                nulls_first: column.nulls_first,
            })
            .collect(),
        predicate: index
            .predicate
            .as_ref()
            .map(|expr| expr_shape(parsed, expr)),
    }
}

fn squonk_insert_shape(parsed: &Parsed, insert: &Insert<NoExt>) -> InsertShape {
    InsertShape {
        with: insert.with.as_ref().map(|with| with_shape(parsed, with)),
        target: InsertTargetShape {
            name: object_name_shape(parsed, &insert.target.name),
            alias: opt_ident_shape(parsed, &insert.target.alias),
            columns: idents_shape(parsed, &insert.target.columns),
        },
        overriding: insert.overriding,
        source: squonk_insert_source_shape(parsed, &insert.source),
        on_conflict: insert.upsert.as_deref().and_then(|upsert| match upsert {
            Upsert::OnConflict { conflict, .. } => Some(squonk_on_conflict_shape(parsed, conflict)),
            // The PostgreSQL oracle never yields a MySQL `ON DUPLICATE KEY UPDATE`
            // (the Postgres dialect cannot parse one), so this arm is unreachable on
            // this path; the shape carries only the PostgreSQL `ON CONFLICT` clause.
            Upsert::OnDuplicateKeyUpdate { .. } => None,
        }),
        returning: squonk_returning_shape(parsed, &insert.returning),
    }
}

fn squonk_insert_source_shape(parsed: &Parsed, source: &InsertSource<NoExt>) -> InsertSourceShape {
    match source {
        InsertSource::DefaultValues { .. } => InsertSourceShape::DefaultValues,
        InsertSource::Values { values, .. } => InsertSourceShape::Values(
            values
                .rows
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|item| squonk_insert_item_shape(parsed, item))
                        .collect()
                })
                .collect(),
        ),
        InsertSource::Query { query, .. } => {
            InsertSourceShape::Query(Box::new(query_shape(parsed, query)))
        }
        InsertSource::Set { assignments, .. } => InsertSourceShape::Set(
            assignments
                .iter()
                .map(|assignment| squonk_update_assignment_shape(parsed, assignment))
                .collect(),
        ),
    }
}

fn squonk_insert_item_shape(parsed: &Parsed, item: &InsertValue<NoExt>) -> InsertItemShape {
    match item {
        InsertValue::Expr { expr, .. } => InsertItemShape::Expr(expr_shape(parsed, expr)),
        InsertValue::Default { .. } => InsertItemShape::Default,
    }
}

fn squonk_on_conflict_shape(parsed: &Parsed, conflict: &OnConflict<NoExt>) -> OnConflictShape {
    OnConflictShape {
        target: conflict.target.as_ref().map(|target| match target {
            ConflictTarget::Index {
                columns, predicate, ..
            } => ConflictTargetShape::Index {
                columns: columns
                    .iter()
                    .map(|expr| expr_shape(parsed, expr))
                    .collect(),
                predicate: predicate.as_ref().map(|expr| expr_shape(parsed, expr)),
            },
            ConflictTarget::Constraint { name, .. } => {
                ConflictTargetShape::Constraint(ident_shape(parsed, name))
            }
        }),
        action: match &conflict.action {
            ConflictAction::Nothing { .. } => ConflictActionShape::Nothing,
            ConflictAction::Update {
                assignments,
                selection,
                ..
            } => ConflictActionShape::Update {
                assignments: assignments
                    .iter()
                    .map(|assignment| squonk_update_assignment_shape(parsed, assignment))
                    .collect(),
                selection: selection.as_ref().map(|expr| expr_shape(parsed, expr)),
            },
        },
    }
}

fn squonk_update_shape(parsed: &Parsed, update: &Update<NoExt>) -> UpdateShape {
    UpdateShape {
        with: update.with.as_ref().map(|with| with_shape(parsed, with)),
        target: squonk_dml_target_shape(parsed, &update.target),
        assignments: update
            .assignments
            .iter()
            .map(|assignment| squonk_update_assignment_shape(parsed, assignment))
            .collect(),
        from: update
            .from
            .iter()
            .map(|table| table_with_joins_shape(parsed, table))
            .collect(),
        selection: update
            .selection
            .as_ref()
            .map(|selection| squonk_dml_selection_shape(parsed, selection)),
        returning: squonk_returning_shape(parsed, &update.returning),
    }
}

fn squonk_delete_shape(parsed: &Parsed, delete: &Delete<NoExt>) -> DeleteShape {
    DeleteShape {
        with: delete.with.as_ref().map(|with| with_shape(parsed, with)),
        target: squonk_dml_target_shape(parsed, &delete.target),
        using: delete
            .using
            .iter()
            .map(|table| table_with_joins_shape(parsed, table))
            .collect(),
        selection: delete
            .selection
            .as_ref()
            .map(|selection| squonk_dml_selection_shape(parsed, selection)),
        returning: squonk_returning_shape(parsed, &delete.returning),
    }
}

fn squonk_dml_target_shape(parsed: &Parsed, target: &DmlTarget) -> DmlTargetShape {
    DmlTargetShape {
        name: object_name_shape(parsed, &target.name),
        // `t` and `t *` both leave `inh = true` in PostgreSQL, so only the `ONLY`
        // spellings count as inheritance suppression in the structural comparison.
        only: matches!(target.inheritance, RelationInheritance::Only(_)),
        alias: opt_ident_shape(parsed, &target.alias),
    }
}

fn squonk_update_assignment_shape(
    parsed: &Parsed,
    assignment: &UpdateAssignment<NoExt>,
) -> UpdateAssignmentShape {
    match assignment {
        UpdateAssignment::Single { target, value, .. } => UpdateAssignmentShape::Single {
            target: object_name_shape(parsed, target),
            value: squonk_update_value_shape(parsed, value),
        },
        UpdateAssignment::Tuple {
            targets, source, ..
        } => UpdateAssignmentShape::Tuple {
            targets: targets
                .iter()
                .map(|target| object_name_shape(parsed, target))
                .collect(),
            source: match source {
                UpdateTupleSource::Row {
                    explicit, values, ..
                } => UpdateTupleSourceShape::Row {
                    explicit: *explicit,
                    values: values
                        .iter()
                        .map(|value| squonk_update_value_shape(parsed, value))
                        .collect(),
                },
                UpdateTupleSource::Subquery { query, .. } => {
                    UpdateTupleSourceShape::Subquery(Box::new(query_shape(parsed, query)))
                }
                UpdateTupleSource::Default { .. } => UpdateTupleSourceShape::Default,
            },
        },
    }
}

fn squonk_update_value_shape(parsed: &Parsed, value: &UpdateValue<NoExt>) -> UpdateValueShape {
    match value {
        UpdateValue::Expr { expr, .. } => UpdateValueShape::Expr(expr_shape(parsed, expr)),
        UpdateValue::Default { .. } => UpdateValueShape::Default,
    }
}

fn squonk_dml_selection_shape(
    parsed: &Parsed,
    selection: &DmlSelection<NoExt>,
) -> DmlSelectionShape {
    match selection {
        DmlSelection::Where { condition, .. } => {
            DmlSelectionShape::Where(expr_shape(parsed, condition))
        }
        DmlSelection::CurrentOf { cursor, .. } => {
            DmlSelectionShape::CurrentOf(ident_shape(parsed, cursor))
        }
    }
}

fn squonk_returning_shape(
    parsed: &Parsed,
    returning: &Option<Returning<NoExt>>,
) -> Vec<SelectItemShape> {
    returning
        .as_ref()
        .map(|returning| {
            returning
                .items
                .iter()
                .map(|item| select_item_shape(parsed, item))
                .collect()
        })
        .unwrap_or_default()
}

// ---- our AST -> transaction / session / DCL / EXPLAIN shape ----------------

fn squonk_transaction_shape(
    parsed: &Parsed,
    transaction: &TransactionStatement,
) -> TransactionShape {
    match transaction {
        TransactionStatement::Begin { syntax, modes, .. } => TransactionShape::Begin {
            start: matches!(syntax, TransactionStart::Start),
            modes: modes.iter().map(squonk_transaction_mode).collect(),
        },
        TransactionStatement::Commit { .. } => TransactionShape::Commit,
        TransactionStatement::Rollback { to_savepoint, .. } => TransactionShape::Rollback {
            to_savepoint: opt_ident_shape(parsed, to_savepoint),
        },
        TransactionStatement::Savepoint { name, .. } => {
            TransactionShape::Savepoint(ident_shape(parsed, name))
        }
        TransactionStatement::Release { savepoint, .. } => {
            TransactionShape::Release(ident_shape(parsed, savepoint))
        }
        TransactionStatement::SetCharacteristics { modes, .. } => {
            TransactionShape::SetCharacteristics {
                modes: modes.iter().map(squonk_transaction_mode).collect(),
            }
        }
    }
}

fn squonk_transaction_mode(mode: &TransactionMode) -> TransactionModeShape {
    match mode {
        TransactionMode::IsolationLevel { level, .. } => {
            TransactionModeShape::IsolationLevel(isolation_level_shape(*level))
        }
        TransactionMode::AccessMode { access, .. } => {
            TransactionModeShape::ReadOnly(matches!(access, TransactionAccessMode::ReadOnly))
        }
        TransactionMode::Deferrable { deferrable, .. } => {
            TransactionModeShape::Deferrable(*deferrable)
        }
        TransactionMode::ConsistentSnapshot { .. } => TransactionModeShape::ConsistentSnapshot,
    }
}

fn isolation_level_shape(level: IsolationLevel) -> IsolationLevelShape {
    match level {
        IsolationLevel::ReadUncommitted => IsolationLevelShape::ReadUncommitted,
        IsolationLevel::ReadCommitted => IsolationLevelShape::ReadCommitted,
        IsolationLevel::RepeatableRead => IsolationLevelShape::RepeatableRead,
        IsolationLevel::Serializable => IsolationLevelShape::Serializable,
    }
}

fn squonk_session_shape(
    parsed: &Parsed,
    session: &SessionStatement,
) -> Result<SessionShape, String> {
    match session {
        SessionStatement::Set {
            scope, name, value, ..
        } => Ok(SessionShape::Set {
            local: matches!(scope, Some(SetScope::Local)),
            name: config_name_shape(parsed, name),
            value: squonk_set_value_shape(parsed, value),
        }),
        SessionStatement::Reset { target, .. } => Ok(SessionShape::Reset {
            name: match target {
                ConfigParameter::All { .. } => None,
                ConfigParameter::Named { name, .. } => Some(config_name_shape(parsed, name)),
            },
        }),
        SessionStatement::Show { target, .. } => Ok(SessionShape::Show {
            name: match target {
                ConfigParameter::All { .. } => "all".to_owned(),
                ConfigParameter::Named { name, .. } => config_name_shape(parsed, name),
            },
        }),
        // The special SET subforms lower to a stringly-named `VariableSetStmt` /
        // `ConstraintsSetStmt` on the PostgreSQL side; their structural mapping is
        // disproportionate, so they stay accept-only (see [`StatementShape`]).
        SessionStatement::SetTimeZone { .. }
        | SessionStatement::SetRole { .. }
        | SessionStatement::SetSessionAuthorization { .. }
        | SessionStatement::SetConstraints { .. }
        | SessionStatement::SetNames { .. }
        | SessionStatement::SetSessionCharacteristics { .. }
        // The MySQL variable-assignment SET, `CHARACTER SET`, and `RESOURCE GROUP` forms have
        // no PostgreSQL structural analogue, so they stay accept-only on the PG differential.
        | SessionStatement::SetVariables { .. }
        | SessionStatement::SetCharacterSet { .. }
        | SessionStatement::SetResourceGroup { .. } => Err(
            "PostgreSQL structural shape mapping for this special SET subform is not implemented"
                .to_owned(),
        ),
    }
}

/// A run-time-configuration parameter name — the dotted, lower-cased spelling
/// PostgreSQL stores (a GUC name is a `ColId`, so unquoted parts are folded).
fn config_name_shape(parsed: &Parsed, name: &ObjectName) -> String {
    object_name_shape(parsed, name)
        .join(".")
        .to_ascii_lowercase()
}

fn squonk_set_value_shape(parsed: &Parsed, value: &SetValue) -> SetValueShape {
    match value {
        SetValue::Default { .. } => SetValueShape::Default,
        SetValue::Values { values, .. } => SetValueShape::Values(
            values
                .iter()
                .map(|value| set_parameter_value_shape(parsed, value))
                .collect(),
        ),
    }
}

fn set_parameter_value_shape(parsed: &Parsed, value: &SetParameterValue) -> SetParameterValueShape {
    match value {
        // PostgreSQL collapses a bareword name and a string literal to one string
        // constant (`A_Const` `Sval`), so both map to `String` (ADR-0015).
        SetParameterValue::Name { name, .. } => {
            SetParameterValueShape::Literal(LiteralShape::String(ident_shape(parsed, name)))
        }
        SetParameterValue::Literal { literal, .. } => {
            SetParameterValueShape::Literal(literal_scalar_shape(parsed, literal))
        }
        SetParameterValue::Parameter {
            kind: ParameterKind::Positional(index),
            ..
        } => SetParameterValueShape::PositionalParameter(*index as i32),
        SetParameterValue::Parameter {
            kind: ParameterKind::PositionalLarge { digits },
            ..
        } => SetParameterValueShape::PositionalParameter(postgres_parameter_number(
            parsed.resolver().resolve(*digits),
        )),
        SetParameterValue::Parameter { .. } => unreachable!(
            "only positional-dollar parameters are enabled under the PostgreSQL preset"
        ),
        // The bracketed list value is DuckDB-only (gated by `collection_literals`, off in
        // the PostgreSQL preset this differential parses under), so it never reaches this
        // PostgreSQL shape mapping.
        SetParameterValue::List { .. } => {
            unreachable!("a bracketed SET list value is a DuckDB-only form absent under PostgreSQL")
        }
    }
}

/// Mirror PostgreSQL's scanner materialization for an oversized `$<digits>` token:
/// `atol` saturates at signed-long max on the 64-bit oracle builds, then assignment
/// to the parser's `int` field retains the low 32 bits.
fn postgres_parameter_number(digits: &str) -> i32 {
    digits.parse::<i64>().unwrap_or(i64::MAX) as i32
}

/// The neutral shape of a scalar literal used as a `SET` value — the same mapping
/// [`expr_shape`] applies to a literal expression. Only the numeric/string/boolean
/// kinds appear as a `SET` value under the PostgreSQL preset.
fn literal_scalar_shape(parsed: &Parsed, literal: &Literal) -> LiteralShape {
    match &literal.kind {
        LiteralKind::Integer => LiteralShape::Integer(
            literal
                .as_decimal_text(parsed.source())
                .expect("parsed integer literal should have source text")
                .into_owned(),
        ),
        LiteralKind::Float => LiteralShape::Float(
            literal
                .as_decimal_text(parsed.source())
                .expect("parsed float literal should have source text")
                .into_owned(),
        ),
        LiteralKind::Boolean(value) => LiteralShape::Boolean(*value),
        LiteralKind::Null => LiteralShape::Null,
        // A string constant, and the non-scalar kinds that never reach a `SET` value
        // under the PostgreSQL preset, materialize to their string text.
        _ => LiteralShape::String(
            literal
                .as_str(parsed.source())
                .map(|text| text.into_owned())
                .unwrap_or_default(),
        ),
    }
}

fn squonk_access_control_shape(
    parsed: &Parsed,
    access: &AccessControlStatement,
) -> AccessControlShape {
    match access {
        AccessControlStatement::AlterRoleRename { name, new_name, .. } => {
            AccessControlShape::RoleRename {
                name: ident_shape(parsed, name),
                new_name: ident_shape(parsed, new_name),
            }
        }
        AccessControlStatement::Grant {
            privileges,
            object,
            grantees,
            with_grant_option,
            granted_by,
            ..
        } => AccessControlShape::Privilege {
            is_grant: true,
            grant_option: *with_grant_option,
            privileges: squonk_privileges_shape(parsed, privileges),
            object: squonk_grant_object_shape(parsed, object),
            grantees: squonk_grantees_shape(parsed, grantees),
            granted_by: granted_by
                .as_ref()
                .map(|spec| role_spec_shape(parsed, spec)),
            cascade: false,
        },
        AccessControlStatement::Revoke {
            grant_option_for,
            privileges,
            object,
            grantees,
            granted_by,
            behavior,
            ..
        } => AccessControlShape::Privilege {
            is_grant: false,
            grant_option: *grant_option_for,
            privileges: squonk_privileges_shape(parsed, privileges),
            object: squonk_grant_object_shape(parsed, object),
            grantees: squonk_grantees_shape(parsed, grantees),
            granted_by: granted_by
                .as_ref()
                .map(|spec| role_spec_shape(parsed, spec)),
            cascade: matches!(behavior, Some(DropBehavior::Cascade)),
        },
        AccessControlStatement::GrantRole {
            roles,
            grantees,
            with_admin_option,
            granted_by,
            ..
        } => AccessControlShape::Role {
            is_grant: true,
            admin_option: *with_admin_option,
            roles: roles
                .iter()
                .map(|role| fold_ident_shape(parsed, role))
                .collect(),
            grantees: squonk_grantees_shape(parsed, grantees),
            granted_by: granted_by
                .as_ref()
                .map(|spec| role_spec_shape(parsed, spec)),
            cascade: false,
        },
        AccessControlStatement::RevokeRole {
            admin_option_for,
            roles,
            grantees,
            granted_by,
            behavior,
            ..
        } => AccessControlShape::Role {
            is_grant: false,
            admin_option: *admin_option_for,
            roles: roles
                .iter()
                .map(|role| fold_ident_shape(parsed, role))
                .collect(),
            grantees: squonk_grantees_shape(parsed, grantees),
            granted_by: granted_by
                .as_ref()
                .map(|spec| role_spec_shape(parsed, spec)),
            cascade: matches!(behavior, Some(DropBehavior::Cascade)),
        },
        AccessControlStatement::AccountGrantPrivilege { .. }
        | AccessControlStatement::AccountGrantProxy { .. }
        | AccessControlStatement::AccountGrantRole { .. }
        | AccessControlStatement::AccountRevokePrivilege { .. }
        | AccessControlStatement::AccountRevokeAll { .. }
        | AccessControlStatement::AccountRevokeProxy { .. }
        | AccessControlStatement::AccountRevokeRole { .. } => unreachable!(
            "MySQL account-based GRANT/REVOKE is absent from the PostgreSQL structural corpus"
        ),
    }
}

fn squonk_grantees_shape(parsed: &Parsed, grantees: &[Grantee]) -> Vec<RoleSpecShape> {
    // The legacy `GROUP` grantee prefix is noise PostgreSQL does not record, so the
    // `Grantee::group` tag is dropped and only the role spec is compared (ADR-0015).
    grantees
        .iter()
        .map(|grantee| role_spec_shape(parsed, &grantee.spec))
        .collect()
}

fn squonk_privileges_shape(parsed: &Parsed, privileges: &Privileges) -> PrivilegesShape {
    match privileges {
        Privileges::All { .. } => PrivilegesShape::All,
        Privileges::List { privileges, .. } => PrivilegesShape::List(
            privileges
                .iter()
                .map(|privilege| squonk_privilege_shape(parsed, privilege))
                .collect(),
        ),
    }
}

fn squonk_privilege_shape(parsed: &Parsed, privilege: &Privilege) -> PrivilegeShape {
    match privilege {
        Privilege::Known { kind, columns, .. } => PrivilegeShape {
            name: privilege_kind_name(*kind).to_owned(),
            columns: idents_shape(parsed, columns),
        },
        Privilege::Other { name, columns, .. } => PrivilegeShape {
            name: ident_shape(parsed, name).to_ascii_lowercase(),
            columns: idents_shape(parsed, columns),
        },
    }
}

fn privilege_kind_name(kind: PrivilegeKind) -> &'static str {
    match kind {
        PrivilegeKind::Select => "select",
        PrivilegeKind::Insert => "insert",
        PrivilegeKind::Update => "update",
        PrivilegeKind::Delete => "delete",
        PrivilegeKind::Truncate => "truncate",
        PrivilegeKind::References => "references",
        PrivilegeKind::Trigger => "trigger",
        PrivilegeKind::Usage => "usage",
        PrivilegeKind::Execute => "execute",
        PrivilegeKind::Create => "create",
        PrivilegeKind::Connect => "connect",
        PrivilegeKind::Temporary => "temporary",
        PrivilegeKind::Temp => "temp",
        PrivilegeKind::Maintain => "maintain",
        PrivilegeKind::Index
        | PrivilegeKind::Alter
        | PrivilegeKind::Drop
        | PrivilegeKind::Reload
        | PrivilegeKind::Shutdown
        | PrivilegeKind::Process
        | PrivilegeKind::File
        | PrivilegeKind::Super
        | PrivilegeKind::Event
        | PrivilegeKind::GrantOption
        | PrivilegeKind::ShowDatabases
        | PrivilegeKind::CreateTemporaryTables
        | PrivilegeKind::LockTables
        | PrivilegeKind::ReplicationSlave
        | PrivilegeKind::ReplicationClient
        | PrivilegeKind::CreateView
        | PrivilegeKind::ShowView
        | PrivilegeKind::CreateRoutine
        | PrivilegeKind::AlterRoutine
        | PrivilegeKind::CreateUser
        | PrivilegeKind::CreateTablespace
        | PrivilegeKind::CreateRole
        | PrivilegeKind::DropRole => {
            unreachable!("MySQL static privileges are absent from the PostgreSQL structural corpus")
        }
    }
}

fn squonk_grant_object_shape(parsed: &Parsed, object: &GrantObject) -> GrantObjectShape {
    match object {
        // The redundant `TABLE` object keyword is not observable on the PostgreSQL
        // side (`objtype` is `ObjectTable` either way), so `explicit` is dropped.
        GrantObject::Table { names, .. } => GrantObjectShape::Objects {
            kind: GrantObjectKindShape::Table,
            names: names
                .iter()
                .map(|name| object_name_shape(parsed, name))
                .collect(),
        },
        GrantObject::Named { kind, names, .. } => GrantObjectShape::Objects {
            kind: named_object_kind_shape(*kind),
            names: names
                .iter()
                .map(|name| object_name_shape(parsed, name))
                .collect(),
        },
        GrantObject::Routines { kind, routines, .. } => GrantObjectShape::Routines {
            kind: routine_object_kind_shape(*kind),
            routines: routines
                .iter()
                .map(|routine| squonk_routine_signature_shape(parsed, routine))
                .collect(),
        },
        GrantObject::AllInSchema { kind, schemas, .. } => GrantObjectShape::AllInSchema {
            class: schema_object_kind_shape(*kind),
            schemas: schemas
                .iter()
                .map(|schema| object_name_shape(parsed, schema))
                .collect(),
        },
    }
}

fn squonk_routine_signature_shape(
    parsed: &Parsed,
    routine: &RoutineSignature,
) -> RoutineSignatureShape {
    RoutineSignatureShape {
        name: object_name_shape(parsed, &routine.name),
        arg_types: routine.arg_types.as_ref().map(|types| {
            types
                .iter()
                .map(|data_type| squonk_data_type_shape(parsed, data_type))
                .collect()
        }),
    }
}

fn named_object_kind_shape(kind: NamedObjectKind) -> GrantObjectKindShape {
    match kind {
        NamedObjectKind::Sequence => GrantObjectKindShape::Sequence,
        NamedObjectKind::Database => GrantObjectKindShape::Database,
        NamedObjectKind::Schema => GrantObjectKindShape::Schema,
        NamedObjectKind::Domain => GrantObjectKindShape::Domain,
        NamedObjectKind::Type => GrantObjectKindShape::Type,
        NamedObjectKind::Language => GrantObjectKindShape::Language,
        NamedObjectKind::Tablespace => GrantObjectKindShape::Tablespace,
        NamedObjectKind::ForeignDataWrapper => GrantObjectKindShape::ForeignDataWrapper,
        NamedObjectKind::ForeignServer => GrantObjectKindShape::ForeignServer,
    }
}

fn routine_object_kind_shape(kind: RoutineObjectKind) -> RoutineKindShape {
    match kind {
        RoutineObjectKind::Function => RoutineKindShape::Function,
        RoutineObjectKind::Procedure => RoutineKindShape::Procedure,
        RoutineObjectKind::Routine => RoutineKindShape::Routine,
    }
}

fn schema_object_kind_shape(kind: SchemaObjectKind) -> SchemaClassShape {
    match kind {
        SchemaObjectKind::Tables => SchemaClassShape::Tables,
        SchemaObjectKind::Sequences => SchemaClassShape::Sequences,
        SchemaObjectKind::Functions => SchemaClassShape::Functions,
        SchemaObjectKind::Procedures => SchemaClassShape::Procedures,
        SchemaObjectKind::Routines => SchemaClassShape::Routines,
    }
}

fn role_spec_shape(parsed: &Parsed, spec: &RoleSpec) -> RoleSpecShape {
    match spec {
        RoleSpec::Public { .. } => RoleSpecShape::Public,
        RoleSpec::CurrentRole { .. } => RoleSpecShape::CurrentRole,
        RoleSpec::CurrentUser { .. } => RoleSpecShape::CurrentUser,
        RoleSpec::SessionUser { .. } => RoleSpecShape::SessionUser,
        RoleSpec::Name { name, .. } => RoleSpecShape::Name(fold_ident_shape(parsed, name)),
    }
}

/// Resolve a role/identifier to the text PostgreSQL records: an unquoted name is a
/// `ColId`, so it is folded to lower case, while a quoted name keeps its case. This
/// lets a keyword-spelled role (`GRANT SELECT TO alice`) compare equal.
fn fold_ident_shape(parsed: &Parsed, ident: &Ident) -> String {
    let text = ident_shape(parsed, ident);
    match ident.quote {
        QuoteStyle::None => text.to_ascii_lowercase(),
        // A `U&"..."` Unicode-escaped identifier is a case-sensitive delimited identifier
        // like `Double`; `ident_shape` resolves its *decoded* value, which pg_query records
        // the same way, so the folded shape keeps its case.
        QuoteStyle::Single
        | QuoteStyle::Double
        | QuoteStyle::UnicodeDouble
        | QuoteStyle::Backtick
        | QuoteStyle::Bracket => text,
    }
}

/// The quote-aware folded form of a multi-part name: each unquoted part lowered
/// (PostgreSQL `ColId` folding, per [`fold_ident_shape`]), each quoted part kept
/// verbatim. Used for a *function* name, which is a case-insensitive identifier: this
/// lets `COUNT(*)` and `count(*)` — and any engine that canonicalizes to lower case,
/// like PostgreSQL's already-folded protobuf and DuckDB's `json_serialize_sql` — compare
/// equal, while a genuinely quoted `"Count"()` still keeps its case on every side.
fn fold_object_name_shape(parsed: &Parsed, name: &ObjectName) -> Vec<String> {
    name.0
        .iter()
        .map(|ident| fold_ident_shape(parsed, ident))
        .collect()
}

fn squonk_explain_shape(
    parsed: &Parsed,
    explain: &ExplainStatement<NoExt>,
) -> Result<ExplainShape, String> {
    Ok(ExplainShape {
        options: explain
            .options
            .iter()
            .map(|option| squonk_explain_option_shape(parsed, option))
            .collect(),
        statement: Box::new(squonk_statement_shape(parsed, &explain.statement)?),
    })
}

fn squonk_explain_option_shape(parsed: &Parsed, option: &ExplainOption) -> ExplainOptionShape {
    match option {
        ExplainOption::Analyze { value, .. } => ExplainOptionShape {
            name: "analyze".to_owned(),
            value: opt_ident_lowercase(parsed, value),
        },
        ExplainOption::Verbose { value, .. } => ExplainOptionShape {
            name: "verbose".to_owned(),
            value: opt_ident_lowercase(parsed, value),
        },
        ExplainOption::Format { format, .. } => ExplainOptionShape {
            name: "format".to_owned(),
            value: Some(explain_format_name(*format).to_owned()),
        },
        ExplainOption::Other { name, value, .. } => ExplainOptionShape {
            name: ident_shape(parsed, name).to_ascii_lowercase(),
            value: opt_ident_lowercase(parsed, value),
        },
    }
}

fn opt_ident_lowercase(parsed: &Parsed, ident: &Option<Ident>) -> Option<String> {
    ident
        .as_ref()
        .map(|ident| ident_shape(parsed, ident).to_ascii_lowercase())
}

fn explain_format_name(format: ExplainFormat) -> &'static str {
    match format {
        ExplainFormat::Text => "text",
        ExplainFormat::Xml => "xml",
        ExplainFormat::Json => "json",
        ExplainFormat::Yaml => "yaml",
    }
}

fn query_shape(parsed: &Parsed, query: &Query<NoExt>) -> QueryShape {
    // A parenthesized query used as a whole operand/source/subquery (`(<inner>)`)
    // parses as a pure-grouping wrapper: an outer `Query` with no clauses of its own
    // whose body is the inner `Query`. PostgreSQL has no grouping node — parens are
    // discarded and the inner query's `with`/`order_by`/`limit` land on one
    // `SelectStmt` — so collapse to the inner query here to compare at the same level
    // (otherwise the inner clauses would be pinned one level too deep and diverge).
    if query.with.is_none()
        && query.order_by.is_empty()
        && query.order_by_all.is_none()
        && query.limit_by.is_none()
        && query.limit.is_none()
        && query.settings.is_empty()
        && query.format.is_none()
        && query.locking.is_empty()
    {
        if let SetExpr::Query { query: inner, .. } = &query.body {
            return query_shape(parsed, inner);
        }
    }
    // `ORDER BY COLUMNS(*)`: DuckDB serializes it and `ORDER BY ALL` to the identical
    // tree (a sole order entry whose expression is the whole-projection star; probed
    // on 1.5.4), and the DuckDB engine mapping lifts that tree to the neutral
    // order-by-all mode (`is_order_by_all_star`). Mirror the lift for our parse — a
    // sole order key that is the bare `COLUMNS(*)` (no pattern, no wildcard
    // modifiers, no `USING`) is the engine-identical spelling of the mode.
    // `Expr::Columns` is reachable only under the DuckDB/Lenient gate, so the
    // PostgreSQL differential (which parses under Postgres) never takes this branch.
    let sole_columns_star = query.order_by_all.is_none()
        && match query.order_by.as_slice() {
            [sole] => {
                sole.using.is_none()
                    && matches!(
                        &sole.expr,
                        Expr::Columns {
                            qualifier: None,
                            pattern: None,
                            options: None,
                            ..
                        }
                    )
            }
            _ => false,
        };
    QueryShape {
        with: query.with.as_ref().map(|with| with_shape(parsed, with)),
        body: set_shape(parsed, &query.body),
        order_by: if sole_columns_star {
            Vec::new()
        } else {
            query
                .order_by
                .iter()
                .map(|item| order_by_shape(parsed, item))
                .collect()
        },
        order_by_all: if sole_columns_star {
            let sole = &query.order_by[0];
            Some(OrderByAllShape {
                asc: sole.asc,
                nulls_first: sole.nulls_first,
            })
        } else {
            query.order_by_all.as_deref().map(|all| OrderByAllShape {
                asc: all.asc,
                nulls_first: all.nulls_first,
            })
        },
        limit: query
            .limit
            .as_ref()
            .map(|limit| limit_shape(parsed, limit))
            .unwrap_or_else(|| LimitShape {
                count: None,
                offset: None,
                with_ties: false,
            }),
        locking: query
            .locking
            .iter()
            .map(|clause| squonk_locking_clause_shape(parsed, clause))
            .collect(),
    }
}

/// Map one of our [`LockingClause`]s to the neutral shape. The surface spelling
/// (`LOCK IN SHARE MODE` vs `FOR SHARE`) is dropped — only the semantic
/// strength/targets/wait compare.
fn squonk_locking_clause_shape(parsed: &Parsed, clause: &LockingClause) -> LockingClauseShape {
    LockingClauseShape {
        strength: match clause.strength {
            LockStrength::Update => LockStrengthShape::Update,
            LockStrength::NoKeyUpdate => LockStrengthShape::NoKeyUpdate,
            LockStrength::Share => LockStrengthShape::Share,
            LockStrength::KeyShare => LockStrengthShape::KeyShare,
        },
        of: clause
            .of
            .iter()
            .map(|name| object_name_shape(parsed, name))
            .collect(),
        wait: clause.wait.map(|wait| match wait {
            LockWait::NoWait => LockWaitShape::NoWait,
            LockWait::SkipLocked => LockWaitShape::SkipLocked,
        }),
    }
}

fn with_shape(parsed: &Parsed, with: &With<NoExt>) -> WithShape {
    WithShape {
        recursive: with.recursive,
        ctes: with.ctes.iter().map(|cte| cte_shape(parsed, cte)).collect(),
    }
}

fn cte_shape(parsed: &Parsed, cte: &Cte<NoExt>) -> CteShape {
    CteShape {
        name: ident_shape(parsed, &cte.name),
        columns: idents_shape(parsed, &cte.columns),
        materialized: cte.materialized,
        body: match &cte.body {
            CteBody::Query { query, .. } => {
                CteBodyShape::Query(Box::new(query_shape(parsed, query)))
            }
            CteBody::Insert { insert, .. } => {
                CteBodyShape::Insert(Box::new(squonk_insert_shape(parsed, insert)))
            }
            CteBody::Update { update, .. } => {
                CteBodyShape::Update(Box::new(squonk_update_shape(parsed, update)))
            }
            CteBody::Delete { delete, .. } => {
                CteBodyShape::Delete(Box::new(squonk_delete_shape(parsed, delete)))
            }
            CteBody::Merge { merge, .. } => {
                CteBodyShape::Merge(Box::new(squonk_merge_shape(parsed, merge)))
            }
        },
    }
}

fn squonk_merge_shape(parsed: &Parsed, merge: &Merge<NoExt>) -> MergeShape {
    MergeShape {
        with: merge.with.as_ref().map(|with| with_shape(parsed, with)),
        target: squonk_dml_target_shape(parsed, &merge.target),
        using: table_with_joins_shape(parsed, &merge.using),
        on: expr_shape(parsed, &merge.on),
        clauses: merge
            .clauses
            .iter()
            .map(|clause| MergeWhenShape {
                match_kind: match clause.match_kind {
                    MergeMatchKind::Matched => MergeMatchShape::Matched,
                    MergeMatchKind::NotMatchedByTarget => MergeMatchShape::NotMatchedByTarget,
                    MergeMatchKind::NotMatchedBySource => MergeMatchShape::NotMatchedBySource,
                },
                condition: clause
                    .condition
                    .as_ref()
                    .map(|expr| expr_shape(parsed, expr)),
                action: match &clause.action {
                    MergeAction::Insert {
                        columns,
                        overriding,
                        values,
                        ..
                    } => MergeActionShape::Insert {
                        columns: idents_shape(parsed, columns),
                        overriding: *overriding,
                        values: values
                            .iter()
                            .map(|item| squonk_insert_item_shape(parsed, item))
                            .collect(),
                    },
                    MergeAction::InsertDefault { .. } => MergeActionShape::InsertDefault,
                    MergeAction::Update { assignments, .. } => MergeActionShape::Update {
                        assignments: assignments
                            .iter()
                            .map(|assignment| squonk_update_assignment_shape(parsed, assignment))
                            .collect(),
                    },
                    MergeAction::UpdateStar { .. } => MergeActionShape::UpdateStar,
                    MergeAction::InsertStar { .. } => MergeActionShape::InsertStar,
                    MergeAction::InsertByName { star, .. } => {
                        MergeActionShape::InsertByName { star: *star }
                    }
                    MergeAction::Error { .. } => MergeActionShape::Error,
                    MergeAction::Delete { .. } => MergeActionShape::Delete,
                    MergeAction::DoNothing { .. } => MergeActionShape::DoNothing,
                },
            })
            .collect(),
        returning: squonk_returning_shape(parsed, &merge.returning),
    }
}

fn limit_shape(parsed: &Parsed, limit: &Limit<NoExt>) -> LimitShape {
    LimitShape {
        count: limit.limit.as_ref().map(|expr| expr_shape(parsed, expr)),
        offset: limit.offset.as_ref().map(|expr| expr_shape(parsed, expr)),
        with_ties: matches!(limit.with_ties, Some(true)),
    }
}

fn order_by_shape(parsed: &Parsed, item: &OrderByExpr<NoExt>) -> OrderByShape {
    OrderByShape {
        expr: expr_shape(parsed, &item.expr),
        asc: item.asc,
        using: item.using.as_ref().map(|using| {
            let mut parts = using
                .schema
                .as_ref()
                .map(|schema| object_name_shape(parsed, schema))
                .unwrap_or_default();
            parts.push(parsed.resolver().resolve(using.op).to_owned());
            parts
        }),
        nulls_first: item.nulls_first,
    }
}

fn set_shape(parsed: &Parsed, set: &SetExpr<NoExt>) -> SetShape {
    match set {
        SetExpr::Select { select, .. } => SetShape::Select(select_shape(parsed, select)),
        SetExpr::Values { values, .. } => SetShape::Values(values_shape(parsed, values)),
        // A parenthesized set operand carrying its own query-level clauses
        // (`X UNION (Y ORDER BY 1)`) keeps them under a `Query` wrapper so the
        // comparison sees them; a pure-grouping operand (no clauses) is transparent
        // and unwraps to its inner body — matching PostgreSQL, which pins an operand's
        // own `ORDER BY`/`LIMIT`/`WITH` on the operand `SelectStmt` and collapses bare
        // grouping.
        SetExpr::Query { query, .. } => {
            if query.with.is_none()
                && query.order_by.is_empty()
                && query.order_by_all.is_none()
                && query.limit.is_none()
                && query.locking.is_empty()
            {
                set_shape(parsed, &query.body)
            } else {
                SetShape::Query(Box::new(query_shape(parsed, query)))
            }
        }
        SetExpr::SetOperation {
            op,
            all,
            by_name,
            left,
            right,
            ..
        } => SetShape::SetOperation {
            op: match op {
                SetOperator::Union => SetOpShape::Union,
                SetOperator::Intersect => SetOpShape::Intersect,
                SetOperator::Except => SetOpShape::Except,
            },
            all: *all,
            by_name: *by_name,
            left: Box::new(set_shape(parsed, left)),
            right: Box::new(set_shape(parsed, right)),
        },
        // DuckDB admits PIVOT/UNPIVOT as a query body, but its `json_serialize_sql`
        // refuses the pivot statement outright ("Only SELECT statements…"), so a
        // pivot-bodied statement is always `EngineOutside` before this maps — the
        // structural differential never reaches it (the DuckDB-only-form pattern).
        SetExpr::Pivot { .. } | SetExpr::Unpivot { .. } => {
            unreachable!(
                "PIVOT/UNPIVOT query bodies are a DuckDB-only form absent under PostgreSQL"
            )
        }
    }
}

fn values_shape(parsed: &Parsed, values: &Values<NoExt>) -> Vec<Vec<ValuesItemShape>> {
    values
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|item| squonk_values_item_shape(parsed, item))
                .collect()
        })
        .collect()
}

fn squonk_values_item_shape(parsed: &Parsed, item: &ValuesItem<NoExt>) -> ValuesItemShape {
    match item {
        ValuesItem::Expr { expr, .. } => ValuesItemShape::Expr(expr_shape(parsed, expr)),
        ValuesItem::Default { .. } => ValuesItemShape::Default,
    }
}

fn select_shape(parsed: &Parsed, select: &Select<NoExt>) -> SelectShape {
    SelectShape {
        // PostgreSQL's distinctClause is empty for both no quantifier and explicit
        // `ALL`, so the comparable flag is true only for `DISTINCT` / `DISTINCT ON`.
        distinct: matches!(
            select.distinct,
            Some(
                SelectDistinct::Quantifier {
                    quantifier: SetQuantifier::Distinct,
                    ..
                } | SelectDistinct::On { .. }
            )
        ),
        projection: select
            .projection
            .iter()
            .map(|item| select_item_shape(parsed, item))
            .collect(),
        from: select
            .from
            .iter()
            .map(|table| table_with_joins_shape(parsed, table))
            .collect(),
        selection: select
            .selection
            .as_ref()
            .map(|expr| expr_shape(parsed, expr)),
        group_by: select
            .group_by
            .iter()
            .map(|item| group_by_item_shape(parsed, item))
            .collect(),
        group_by_distinct: matches!(
            select.group_by_quantifier,
            Some(squonk_ast::SetQuantifier::Distinct)
        ),
        group_by_all: select.group_by_all.is_some(),
        having: select.having.as_ref().map(|expr| expr_shape(parsed, expr)),
        qualify: select
            .qualify
            .as_deref()
            .map(|expr| Box::new(expr_shape(parsed, expr))),
    }
}

fn group_by_item_shape(parsed: &Parsed, item: &GroupByItem<NoExt>) -> GroupByItemShape {
    match item {
        GroupByItem::Expr { expr, .. } => GroupByItemShape::Expr(expr_shape(parsed, expr)),
        GroupByItem::Rollup { exprs, .. } => {
            GroupByItemShape::Rollup(exprs.iter().map(|e| expr_shape(parsed, e)).collect())
        }
        GroupByItem::Cube { exprs, .. } => {
            GroupByItemShape::Cube(exprs.iter().map(|e| expr_shape(parsed, e)).collect())
        }
        GroupByItem::GroupingSets { sets, .. } => GroupByItemShape::GroupingSets(
            sets.iter()
                .map(|s| group_by_item_shape(parsed, s))
                .collect(),
        ),
        GroupByItem::Empty { .. } => GroupByItemShape::Empty,
    }
}

fn select_item_shape(parsed: &Parsed, item: &SelectItem<NoExt>) -> SelectItemShape {
    match item {
        // A wildcard's `EXCLUDE`/`REPLACE`/`RENAME` modifiers are dropped: they have
        // no neutral shape yet, and the DuckDB engine mapping skips any statement
        // whose star carries them (`has_star_modifiers`), so the drop can never
        // manufacture a false match — the pair never reaches comparison. The
        // PostgreSQL differential parses under Postgres, where the modifiers are
        // unreachable (gate off).
        SelectItem::Wildcard { .. } => SelectItemShape::Wildcard,
        SelectItem::QualifiedWildcard { name, .. } => {
            SelectItemShape::QualifiedWildcard(object_name_shape(parsed, name))
        }
        SelectItem::Expr { expr, alias, .. } => SelectItemShape::Expr {
            expr: expr_shape(parsed, expr),
            alias: opt_ident_shape(parsed, alias),
        },
    }
}

fn table_with_joins_shape(parsed: &Parsed, table: &TableWithJoins<NoExt>) -> TableWithJoinsShape {
    TableWithJoinsShape {
        relation: table_factor_shape(parsed, &table.relation),
        joins: table
            .joins
            .iter()
            .map(|join| JoinShape {
                relation: table_factor_shape(parsed, &join.relation),
                operator: join_operator_shape(parsed, &join.operator),
            })
            .collect(),
    }
}

fn table_factor_shape(parsed: &Parsed, factor: &TableFactor<NoExt>) -> TableFactorShape {
    match factor {
        TableFactor::Table {
            name,
            inheritance,
            alias,
            sample,
            ..
        } => TableFactorShape::Table {
            name: object_name_shape(parsed, name),
            alias: alias.as_ref().map(|alias| alias_shape(parsed, alias)),
            // `t` and `t *` are inheritance-equivalent (`inh = true`); only `ONLY`
            // suppresses descendants, matching PostgreSQL's `!inh`.
            only: matches!(inheritance, RelationInheritance::Only(_)),
            sample: sample
                .as_ref()
                .map(|sample| table_sample_shape(parsed, sample)),
        },
        TableFactor::Derived {
            lateral,
            subquery,
            alias,
            ..
        } => TableFactorShape::Derived {
            lateral: *lateral,
            subquery: Box::new(query_shape(parsed, subquery)),
            alias: alias.as_ref().map(|alias| alias_shape(parsed, alias)),
        },
        TableFactor::Function {
            lateral,
            function,
            with_ordinality,
            alias,
            column_defs,
            ..
        } => TableFactorShape::Function {
            lateral: *lateral,
            function: function_shape(parsed, function),
            with_ordinality: *with_ordinality,
            alias: alias.as_ref().map(|alias| alias_shape(parsed, alias)),
            column_defs: column_defs
                .iter()
                .map(|column| column_def_shape(parsed, column))
                .collect(),
        },
        TableFactor::RowsFrom {
            lateral,
            functions,
            with_ordinality,
            alias,
            ..
        } => TableFactorShape::RowsFrom {
            lateral: *lateral,
            functions: functions
                .iter()
                .map(|item| RowsFromItemShape {
                    function: function_shape(parsed, &item.function),
                    column_defs: item
                        .column_defs
                        .iter()
                        .map(|column| column_def_shape(parsed, column))
                        .collect(),
                })
                .collect(),
            with_ordinality: *with_ordinality,
            alias: alias.as_ref().map(|alias| alias_shape(parsed, alias)),
        },
        // PostgreSQL lowers `FROM unnest(…)` to the same `RangeFunction` as any other
        // set-returning function ([`pg_range_function_shape`] maps it to
        // [`TableFactorShape::Function`]), so the first-class `TableFactor::Unnest` maps to
        // that identical neutral Function shape — name `unnest`, the array expressions as
        // arguments — keeping the structural differential parity-equal. The BigQuery-only
        // `WITH OFFSET` fields have no PostgreSQL counterpart and drop out here (a
        // `WITH OFFSET` factor never reaches an agree-accept comparison — PostgreSQL
        // parse-rejects the tail).
        TableFactor::Unnest {
            lateral,
            array_exprs,
            with_ordinality,
            alias,
            column_defs,
            ..
        } => TableFactorShape::Function {
            lateral: *lateral,
            function: FunctionShape {
                name: vec!["unnest".to_string()],
                args: array_exprs
                    .iter()
                    .map(|expr| expr_shape(parsed, expr))
                    .collect(),
                wildcard: false,
            },
            with_ordinality: *with_ordinality,
            alias: alias.as_ref().map(|alias| alias_shape(parsed, alias)),
            column_defs: column_defs
                .iter()
                .map(|column| column_def_shape(parsed, column))
                .collect(),
        },
        TableFactor::NestedJoin { table, alias, .. } => TableFactorShape::NestedJoin {
            table: Box::new(table_with_joins_shape(parsed, table)),
            alias: alias.as_ref().map(|alias| alias_shape(parsed, alias)),
        },
        TableFactor::SpecialFunction {
            keyword,
            precision,
            alias,
            ..
        } => TableFactorShape::SpecialFunction {
            keyword: *keyword,
            precision: *precision,
            alias: alias.as_ref().map(|alias| alias_shape(parsed, alias)),
        },
        // Both pivot spellings canonicalize onto the one operator shape (ADR-0011);
        // the statement-only `with`/`order_by`/`limit` members are not part of it —
        // the texts that carry them never reach an engine-side comparison (the
        // serializer refuses the statement spelling), so the shape stays the
        // operator core the engine's `PIVOT` node mirrors.
        TableFactor::Pivot { pivot, alias, .. } => TableFactorShape::Pivot(Box::new(PivotShape {
            source: Box::new(table_factor_shape(parsed, &pivot.source)),
            aggregates: pivot
                .aggregates
                .iter()
                .map(|aggregate| pivot_expr_shape(parsed, aggregate))
                .collect(),
            pivot_on: pivot
                .pivot_on
                .iter()
                .map(|column| PivotColumnShape {
                    expr: expr_shape(parsed, &column.expr),
                    values: column
                        .values
                        .iter()
                        .map(|value| pivot_expr_shape(parsed, value))
                        .collect(),
                    enum_source: column
                        .enum_source
                        .as_ref()
                        .map(|name| ident_shape(parsed, name)),
                })
                .collect(),
            group_by: pivot
                .group_by
                .iter()
                .map(|expr| expr_shape(parsed, expr))
                .collect(),
            alias: alias.as_ref().map(|alias| alias_shape(parsed, alias)),
        })),
        TableFactor::Unpivot { unpivot, alias, .. } => {
            TableFactorShape::Unpivot(Box::new(UnpivotShape {
                source: Box::new(table_factor_shape(parsed, &unpivot.source)),
                value: idents_shape(parsed, &unpivot.value),
                name: idents_shape(parsed, &unpivot.name),
                columns: unpivot
                    .columns
                    .iter()
                    .map(|column| UnpivotColumnShape {
                        columns: column
                            .columns
                            .iter()
                            .map(|expr| expr_shape(parsed, expr))
                            .collect(),
                        alias: column
                            .alias
                            .as_ref()
                            .map(|alias| ident_shape(parsed, alias)),
                    })
                    .collect(),
                // Semantic projection for the DuckDB structural differential: DuckDB's
                // engine tree records a bool, so an explicit `EXCLUDE NULLS` and the
                // omitted default both collapse to `false` (nulls excluded).
                include_nulls: matches!(
                    unpivot.null_inclusion,
                    Some(squonk_ast::ast::NullInclusion::IncludeNulls)
                ),
                alias: alias.as_ref().map(|alias| alias_shape(parsed, alias)),
            }))
        }
        // DuckDB's DESCRIBE/SHOW/SUMMARIZE table source desugars to a `SUBQUERY`
        // (`SELECT * FROM <SHOW_REF>`) in the engine tree — an unmappable `SHOW_REF`
        // node — so the structural differential skips it (`EngineOutside`) before this
        // maps; it carries no neutral shape (the DuckDB-only-form pattern).
        TableFactor::ShowRef { .. } => {
            unreachable!(
                "DESCRIBE/SHOW/SUMMARIZE table sources carry no neutral shape (DuckDB desugars them to a SUBQUERY over an unmappable SHOW_REF)"
            )
        }
        // PostgreSQL lowers `JSON_TABLE`/`XMLTABLE` to dedicated `JsonTable`/`RangeTableFunc`
        // from-clause items that `pg_shape` reports as `OutsideSubset` (an unmapped FROM item),
        // so the structural differential skips these before this maps them — the `ShowRef`
        // Unmapped precedent. They carry no neutral shape.
        TableFactor::JsonTable { .. } => unreachable!(
            "JSON_TABLE carries no neutral shape (PostgreSQL's JsonTable FROM item is unmapped)"
        ),
        TableFactor::XmlTable { .. } => unreachable!(
            "XMLTABLE carries no neutral shape (PostgreSQL's RangeTableFunc FROM item is unmapped)"
        ),
        // `TABLE(<expr>)` is a Lenient-only factor no oracle-backed preset ships, so it
        // never reaches an engine-side comparison — the `ShowRef`/`JsonTable` precedent.
        TableFactor::TableExpr { .. } => {
            unreachable!(
                "TABLE(<expr>) carries no neutral shape (no oracle-backed preset ships it)"
            )
        }
        // MATCH_RECOGNIZE is a Snowflake/Lenient-only factor no oracle-backed preset ships,
        // so it never reaches an engine-side comparison — the `TableExpr` precedent.
        TableFactor::MatchRecognize { .. } => unreachable!(
            "MATCH_RECOGNIZE carries no neutral shape (no oracle-backed preset ships it)"
        ),
        // OPENJSON is an MSSQL/Lenient-only factor no oracle-backed preset ships, so it never
        // reaches an engine-side comparison — the `TableExpr` precedent.
        TableFactor::OpenJson { .. } => {
            unreachable!("OPENJSON carries no neutral shape (no oracle-backed preset ships it)")
        }
        TableFactor::Other { ext, .. } => match *ext {},
    }
}

/// The shape of an aliased pivot expression (a `USING` aggregate or an `IN` value).
fn pivot_expr_shape(parsed: &Parsed, expr: &PivotExpr<NoExt>) -> PivotExprShape {
    PivotExprShape {
        expr: expr_shape(parsed, &expr.expr),
        alias: expr.alias.as_ref().map(|alias| ident_shape(parsed, alias)),
    }
}

fn alias_shape(parsed: &Parsed, alias: &TableAlias) -> AliasShape {
    AliasShape {
        name: ident_shape(parsed, &alias.name),
        columns: idents_shape(parsed, &alias.columns),
    }
}

fn function_shape(parsed: &Parsed, function: &FunctionCall<NoExt>) -> FunctionShape {
    FunctionShape {
        // Fold the function name (a case-insensitive identifier) so the neutral shape is
        // case-canonical — matching PostgreSQL's lower-cased protobuf and DuckDB's
        // `json_serialize_sql`, both of which lower an unquoted call.
        name: fold_object_name_shape(parsed, &function.name),
        // The neutral shape compares argument *values*; the named-argument name is a
        // surface concern PostgreSQL carries as a `NamedArgExpr` wrapper, which the
        // mapping unwraps to the same inner value (see `pg_expr_shape`), so a named
        // and a positional argument with equal value compare equal here.
        args: function
            .args
            .iter()
            .map(|arg| expr_shape(parsed, &arg.value))
            .collect(),
        wildcard: function.wildcard,
    }
}

fn column_def_shape(parsed: &Parsed, column: &TableFunctionColumn) -> ColumnDefShape {
    ColumnDefShape {
        name: ident_shape(parsed, &column.name),
        data_type: squonk_data_type_shape(parsed, &column.data_type),
    }
}

fn table_sample_shape(parsed: &Parsed, sample: &TableSample<NoExt>) -> TableSampleShape {
    TableSampleShape {
        method: normalize_shape_name(object_name_shape(parsed, &sample.method)),
        args: sample
            .args
            .iter()
            .map(|expr| expr_shape(parsed, expr))
            .collect(),
        repeatable: sample
            .repeatable
            .as_ref()
            .map(|expr| expr_shape(parsed, expr)),
    }
}

fn normalize_shape_name(name: Vec<String>) -> Vec<String> {
    name.into_iter()
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn join_operator_shape(parsed: &Parsed, operator: &JoinOperator<NoExt>) -> JoinOperatorShape {
    match operator {
        JoinOperator::Inner { constraint, .. } => {
            JoinOperatorShape::Inner(join_constraint_shape(parsed, constraint))
        }
        JoinOperator::LeftOuter { constraint, .. } => {
            JoinOperatorShape::LeftOuter(join_constraint_shape(parsed, constraint))
        }
        JoinOperator::RightOuter { constraint, .. } => {
            JoinOperatorShape::RightOuter(join_constraint_shape(parsed, constraint))
        }
        JoinOperator::FullOuter { constraint, .. } => {
            JoinOperatorShape::FullOuter(join_constraint_shape(parsed, constraint))
        }
        // PostgreSQL's raw parse tree normalizes `CROSS JOIN` to an inner join
        // with no quals/USING clause. Keep the AST surface tag for rendering, but
        // compare the engine-facing neutral shape semantically here.
        JoinOperator::Cross { .. } => JoinOperatorShape::Inner(JoinConstraintShape::None),
        // The DuckDB-only joins map one-to-one; PostgreSQL never parses them, so
        // these shapes only ever meet the DuckDB structural oracle's.
        JoinOperator::AsOf {
            kind, constraint, ..
        } => JoinOperatorShape::AsOf(*kind, join_constraint_shape(parsed, constraint)),
        JoinOperator::Positional { .. } => JoinOperatorShape::Positional,
        JoinOperator::Semi {
            asof,
            side,
            constraint,
            ..
        } => JoinOperatorShape::Semi(*asof, *side, join_constraint_shape(parsed, constraint)),
        JoinOperator::Anti {
            asof,
            side,
            constraint,
            ..
        } => JoinOperatorShape::Anti(*asof, *side, join_constraint_shape(parsed, constraint)),
        JoinOperator::Apply { kind, .. } => JoinOperatorShape::Apply(*kind),
    }
}

fn join_constraint_shape(
    parsed: &Parsed,
    constraint: &JoinConstraint<NoExt>,
) -> JoinConstraintShape {
    match constraint {
        JoinConstraint::On { expr, .. } => JoinConstraintShape::On(expr_shape(parsed, expr)),
        JoinConstraint::Using { columns, alias, .. } => JoinConstraintShape::Using {
            columns: idents_shape(parsed, columns),
            alias: opt_ident_shape(parsed, alias),
        },
        JoinConstraint::Natural { .. } => JoinConstraintShape::Natural,
        JoinConstraint::None { .. } => JoinConstraintShape::None,
    }
}

fn expr_shape(parsed: &Parsed, expr: &Expr<NoExt>) -> ExprShape {
    match expr {
        Expr::Column { name, .. } => ExprShape::Column(object_name_shape(parsed, name)),
        Expr::Literal { literal, .. } => match &literal.kind {
            LiteralKind::Integer => ExprShape::Literal(LiteralShape::Integer(
                literal
                    .as_decimal_text(parsed.source())
                    .expect("parsed integer literal should have source text")
                    .into_owned(),
            )),
            // `Decimal` is the same non-integer numeric value as `Float`, differing only
            // in the parse-time `parse_float_as_decimal` classification request. That
            // request is never set on the differential-oracle parse path (it always uses
            // default options), so this arm is unreachable there; it maps alongside
            // `Float` for totality and because the two share a value materialisation.
            LiteralKind::Float | LiteralKind::Decimal => ExprShape::Literal(LiteralShape::Float(
                literal
                    .as_decimal_text(parsed.source())
                    .expect("parsed float literal should have source text")
                    .into_owned(),
            )),
            LiteralKind::String => ExprShape::Literal(LiteralShape::String(
                literal
                    .as_str(parsed.source())
                    .expect("parsed string literal should materialize")
                    .into_owned(),
            )),
            LiteralKind::Boolean(value) => ExprShape::Literal(LiteralShape::Boolean(*value)),
            LiteralKind::Null => ExprShape::Literal(LiteralShape::Null),
            // PostgreSQL lowers typed temporal literals to a `TypeCast`, not an
            // `A_Const`, so they fall outside the neutral literal corpus; map them to
            // `Unmapped` (like the PostgreSQL postfix/constructor forms) so a
            // structural comparison reports an explicit gap rather than a mis-mapping
            // (ADR-0015). Accept/reject parity and render round-trip cover them.
            LiteralKind::Date
            | LiteralKind::Time { .. }
            | LiteralKind::Timestamp { .. }
            | LiteralKind::Interval { .. } => ExprShape::Unmapped,
            // Bit-string constants (`B'1010'`/`X'1FF'`) lower to a `bit`-typed
            // `A_Const`, which the neutral `LiteralShape` does not model; like the
            // temporal forms they are covered by accept/reject parity and render
            // round-trip rather than structural comparison.
            LiteralKind::BitString { .. } => ExprShape::Unmapped,
            // Money (`$1234.56`) is a T-SQL form; PostgreSQL has no money literal and
            // the differential oracle runs under the PostgreSQL preset (money off), so
            // this arm is never reached in practice — map it to `Unmapped` for totality,
            // consistent with the other non-PostgreSQL literal kinds.
            LiteralKind::Money => ExprShape::Unmapped,
        },
        Expr::BinaryOp {
            left, op, right, ..
        } => ExprShape::BinaryOp {
            left: Box::new(expr_shape(parsed, left)),
            op: binary_operator_shape(op),
            right: Box::new(expr_shape(parsed, right)),
        },
        Expr::UnaryOp { op, expr, .. } => {
            fold_unary(unary_operator_shape(op), expr_shape(parsed, expr))
        }
        Expr::Cast {
            expr, data_type, ..
        } => ExprShape::Cast {
            expr: Box::new(expr_shape(parsed, expr)),
            data_type: squonk_data_type_shape(parsed, data_type),
        },
        Expr::InSubquery {
            expr,
            subquery,
            negated,
            ..
        } => ExprShape::InSubquery {
            expr: Box::new(expr_shape(parsed, expr)),
            subquery: Box::new(query_shape(parsed, subquery)),
            negated: *negated,
        },
        Expr::Exists { query, .. } => ExprShape::Exists(Box::new(query_shape(parsed, query))),
        Expr::QuantifiedComparison {
            left,
            op,
            quantifier,
            subquery,
            ..
        } => ExprShape::QuantifiedComparison {
            left: Box::new(expr_shape(parsed, left)),
            op: binary_operator_shape(op),
            // `SOME` and `ANY` are exact SQL synonyms (`x = SOME (s)` ≡ `x = ANY (s)`):
            // PostgreSQL has no distinct "some" sublink, so pg_query lowers both to one
            // `AnySublink` => `Quantifier::Any` (see `pg_sub_link_shape`). Our AST keeps
            // `Quantifier::Some` only as a render-spelling tag (ADR-0011), so collapse it
            // to `Any` here to land on that single canonical shape rather than a false
            // structural divergence.
            quantifier: match *quantifier {
                Quantifier::Some => Quantifier::Any,
                other => other,
            },
            subquery: Box::new(query_shape(parsed, subquery)),
        },
        Expr::Subquery { query, .. } => ExprShape::Subquery(Box::new(query_shape(parsed, query))),
        Expr::IsNull { expr, negated, .. } => ExprShape::IsNull {
            expr: Box::new(expr_shape(parsed, expr)),
            negated: *negated,
        },
        // The truth-value tests (`IS [NOT] {TRUE|FALSE|UNKNOWN}`) map to the explicit
        // `Unmapped` gap: neither structural mapper recognizes the engine's truth-test node
        // yet (PostgreSQL's `BooleanTest`, DuckDB's serialized form), so a neutral
        // `IsTruth` shape would compare our tree against nothing on the engine side. Wiring
        // both mappers is a follow-up (`planner-parity-expr-is-truth-predicates` deferral),
        // like the windowed-call gap below.
        Expr::IsTruth { .. } => ExprShape::Unmapped,
        // The Unicode-normalization test maps to the explicit `Unmapped` gap alongside the
        // sibling `IsTruth` predicate.
        Expr::IsNormalized { .. } => ExprShape::Unmapped,
        // A windowed call (`OVER …`) maps to the explicit `Unmapped` gap: the neutral
        // `FunctionShape` has no window member, and both engine sides agree this is a
        // gap rather than a shape — PostgreSQL rejects the statement outright
        // ([`pg_func_call_shape`]), and the DuckDB oracle maps its `WINDOW` class to
        // `Unmapped` — so mapping ours to `Function` here would claim silent parity
        // on the dropped window spec against DuckDB (load-bearing since QUALIFY
        // predicates put windowed calls in the compared subset,
        // `duckdb-qualify-clause`).
        Expr::Function { call, .. } if call.over.is_some() => ExprShape::Unmapped,
        // Only name/args/wildcard are compared; the remaining aggregate modifiers our
        // `FunctionCall` can carry (DISTINCT/ORDER BY/FILTER) are not in the
        // shape, and both engine sides reject/skip any call bearing them
        // ([`pg_func_call_shape`] errors; the DuckDB mapping errors on
        // filter/order/distinct) — so a modifier-bearing call surfaces as an explicit
        // divergence or skip rather than silent parity (ADR-0015).
        Expr::Function { call, .. } => ExprShape::Function(function_shape(parsed, call)),
        Expr::Case { case, .. } => ExprShape::Case {
            operand: case
                .operand
                .as_ref()
                .map(|operand| Box::new(expr_shape(parsed, operand))),
            when_clauses: case
                .when_clauses
                .iter()
                .map(|clause| WhenClauseShape {
                    condition: expr_shape(parsed, &clause.condition),
                    result: expr_shape(parsed, &clause.result),
                })
                .collect(),
            else_result: case
                .else_result
                .as_ref()
                .map(|else_result| Box::new(expr_shape(parsed, else_result))),
        },
        Expr::SessionVariable { .. } => {
            unreachable!("session variables are a MySQL construct, absent from the PostgreSQL corpus")
        }
        // The collection constructors and the subscript map to their neutral shapes
        // (`duckdb-collection-literals`): the DuckDB structural oracle normalizes its
        // `list_value`/`struct_pack`/`ARRAY_EXTRACT`/`ARRAY_SLICE` desugarings back to
        // the same shapes, and the PostgreSQL side maps its own `A_ArrayExpr`. The
        // `ARRAY(<query>)` subquery form stays an explicit gap.
        Expr::Array { array, .. } => match &**array {
            ArrayExpr::Elements { elements, .. } => ExprShape::Array(
                elements
                    .iter()
                    .map(|element| expr_shape(parsed, element))
                    .collect(),
            ),
            ArrayExpr::Subquery { .. } => ExprShape::Unmapped,
            // The DuckDB list comprehension desugars to a `list_apply`/`list_filter`
            // call tree in the engine; we keep the surface node and do not model that
            // structural expansion, so it stays an explicit gap (like `Subquery`).
            ArrayExpr::Comprehension { .. } => ExprShape::Unmapped,
        },
        Expr::Struct { r#struct, .. } => ExprShape::Struct(
            r#struct
                .fields
                .iter()
                .map(|field| {
                    (
                        parsed.resolver().resolve(field.key).to_owned(),
                        expr_shape(parsed, &field.value),
                    )
                })
                .collect(),
        ),
        // DuckDB defines `MAP {k: v, …}` as sugar for `map([keys], [values])` and its
        // serialized tree carries exactly that call, with the surface form discarded —
        // so the map literal's *neutral* shape is its documented desugaring (ADR-0015
        // representation equivalence, category 1), keeping `MAP {'a': 1}` and
        // `map(['a'], [1])` comparable the same way the engine sees them. The AST
        // keeps the surface distinction for round-trip; only the shape folds.
        Expr::Map { map, .. } => {
            let (keys, values) = map
                .entries
                .iter()
                .map(|entry| {
                    (
                        expr_shape(parsed, &entry.key),
                        expr_shape(parsed, &entry.value),
                    )
                })
                .unzip();
            ExprShape::Function(FunctionShape {
                name: vec!["map".to_owned()],
                args: vec![ExprShape::Array(keys), ExprShape::Array(values)],
                wildcard: false,
            })
        }
        Expr::Subscript { subscript, .. } => ExprShape::Subscript {
            base: Box::new(expr_shape(parsed, &subscript.base)),
            lower: subscript
                .lower
                .as_ref()
                .map(|lower| Box::new(expr_shape(parsed, lower))),
            upper: subscript
                .upper
                .as_ref()
                .map(|upper| Box::new(expr_shape(parsed, upper))),
            step: subscript
                .step
                .as_ref()
                .map(|step| Box::new(expr_shape(parsed, step))),
            kind: subscript.kind,
        },
        // The DuckDB lambda maps its parameter names by exact text (like `Struct`
        // keys) and recurses the body; the parameter-list spelling is exactly what
        // the neutral shape ignores (ADR-0011). The DuckDB oracle produces the same
        // shape from its `LAMBDA` class (`duckdb-lambda-expressions`).
        Expr::Lambda { lambda, .. } => ExprShape::Lambda {
            params: lambda
                .params
                .iter()
                .map(|param| parsed.resolver().resolve(param.sym).to_owned())
                .collect(),
            body: Box::new(expr_shape(parsed, &lambda.body)),
        },
        // `COLUMNS(<pattern>)` / `COLUMNS(*)` / `COLUMNS(t.*)`. The
        // `EXCLUDE`/`REPLACE`/`RENAME` modifiers on the star form carry no neutral
        // shape yet; the DuckDB side skips any such statement, so dropping `options`
        // here cannot manufacture a false match (the pair never reaches comparison).
        Expr::Columns {
            qualifier, pattern, ..
        } => ExprShape::Columns {
            qualifier: qualifier
                .as_ref()
                .map(|qualifier| object_name_shape(parsed, qualifier)),
            pattern: pattern
                .as_ref()
                .map(|pattern| Box::new(expr_shape(parsed, pattern))),
        },
        // The remaining PostgreSQL postfix/constructor forms are not in the neutral
        // corpus yet; mapping them to `Unmapped` makes a statement that carries one
        // surface as an explicit structural divergence instead of crashing the
        // mapping (ADR-0015), so a parseable-but-unmapped construct stays a gap.
        // `LIKE`/`ILIKE`/`SIMILAR TO` are not in the neutral structural corpus yet, so
        // map them to `Unmapped` (an explicit gap, ADR-0015) rather than crashing.
        // `BETWEEN`, the `IN`-list, `EXTRACT`, and prepared-statement parameters
        // (`$1`) belong here too: each parses under the PostgreSQL preset (they are
        // standard forms, not dialect-gated), so a future promotion from
        // regress-guide.sql takes this clean divergence path instead of a hard panic.
        Expr::Between { .. }
        | Expr::InList { .. }
        // DuckDB's unparenthesized `x IN y` list-membership: dialect-gated and not in the
        // neutral structural corpus yet, so it stays an explicit gap (ADR-0015).
        | Expr::InExpr { .. }
        | Expr::Extract { .. }
        | Expr::Parameter { .. }
        // DuckDB `#n` positional column reference: dialect-gated and not in the neutral
        // structural corpus yet, so it stays an explicit gap (ADR-0015) like `$1` above.
        | Expr::PositionalColumn { .. }
        // DuckDB/PostgreSQL `= ANY (<list>)` scalar-array comparison (PG's
        // `ScalarArrayOpExpr`): dialect-gated and not in the neutral structural corpus
        // yet, so it stays an explicit gap (ADR-0015) like the forms above.
        | Expr::QuantifiedList { .. }
        | Expr::QuantifiedLike { .. }
        | Expr::SemiStructuredAccess { .. }
        | Expr::Like { .. }
        | Expr::Collate { .. }
        | Expr::AtTimeZone { .. }
        // MySQL's operator-position interval `INTERVAL <expr> <unit>`: gated to MySQL/Lenient,
        // neither of which has a structural oracle, so it stays an explicit gap (ADR-0015) like
        // the dialect-gated forms above.
        | Expr::Interval { .. }
        | Expr::Row { .. }
        // The BigQuery `STRUCT(...)` value constructor: gated to BigQuery/Lenient,
        // neither of which has a structural oracle, so it stays an explicit gap
        // (ADR-0015) like the dialect-gated forms above.
        | Expr::StructConstructor { .. }
        | Expr::FieldSelection { .. }
        // `a OPERATOR(schema.op) b`: PostgreSQL lowers the explicit-operator form to a
        // plain operator `A_Expr` (identical to the bare operator), which the neutral
        // corpus does not yet model, so it stays an explicit gap (ADR-0015) rather
        // than being normalized to a `BinaryOp` shape it cannot fully carry (the
        // schema qualification has no neutral representation).
        | Expr::NamedOperator { .. }
        // A PostgreSQL prefix symbolic operator (`@ x`, `|/ x`, `@#@ x`): PostgreSQL lowers
        // it to a prefix operator `A_Expr` (lexpr = NULL), which the neutral corpus does not
        // yet model, so — like the bare/`OPERATOR(...)` infix form above — it stays an
        // explicit gap (ADR-0015) rather than a shape it cannot fully carry.
        | Expr::PrefixOperator { .. }
        // A DuckDB postfix symbolic operator (`10!`, `1 ~`, `1 <->`): DuckDB lowers it to a
        // `__postfix` function call, which the neutral corpus does not yet model, so — like
        // the prefix/infix general-operator forms above — it stays an explicit gap (ADR-0015).
        | Expr::PostfixOperator { .. }
        | Expr::SpecialFunction { .. }
        // The SQL/JSON expression functions (JSON_VALUE/JSON_QUERY/…, JSON_OBJECT/ARRAY
        // constructors + aggregates, JSON()/JSON_SCALAR()/JSON_SERIALIZE(), IS JSON): a large
        // special-form family PostgreSQL lowers to dedicated `JsonFuncExpr`/`JsonConstructor`/
        // `JsonIsPredicate` nodes not in the neutral structural corpus yet, so they stay an
        // explicit gap (ADR-0015) like the forms above.
        | Expr::JsonFunc { .. }
        | Expr::JsonObject { .. }
        | Expr::JsonArray { .. }
        | Expr::JsonAggregate { .. }
        | Expr::JsonConstructor { .. }
        | Expr::IsJson { .. }
        // The SQL/XML expression functions (xmlelement/xmlserialize/…, IS DOCUMENT):
        // PostgreSQL lowers these to `XmlExpr`/`XmlSerialize` nodes not in the neutral
        // structural corpus yet, so they stay an explicit gap (ADR-0015) like the JSON
        // special forms above.
        | Expr::XmlFunc { .. }
        | Expr::IsDocument { .. }
        // The standard string special forms (SUBSTRING/POSITION/OVERLAY/TRIM keyword
        // grammar): PostgreSQL lowers them to `COERCE_SQL_SYNTAX` FuncCalls whose
        // argument order the neutral corpus does not model, so they stay an explicit
        // gap (ADR-0015) like the special-form families above.
        | Expr::StringFunc { .. } => ExprShape::Unmapped,
        Expr::Other { ext, .. } => match *ext {},
    }
}

fn squonk_data_type_shape(parsed: &Parsed, data_type: &DataType) -> DataTypeShape {
    match data_type {
        DataType::Boolean { .. } => data_type_shape(["bool"], std::iter::empty::<i64>(), 0),
        DataType::SmallInt { .. } => data_type_shape(["int2"], std::iter::empty::<i64>(), 0),
        DataType::Integer { .. } => data_type_shape(["int4"], std::iter::empty::<i64>(), 0),
        DataType::BigInt { .. } => data_type_shape(["int8"], std::iter::empty::<i64>(), 0),
        DataType::Decimal {
            precision, scale, ..
        } => data_type_shape(["numeric"], numeric_modifiers(*precision, *scale), 0),
        DataType::Float { precision, .. } => {
            data_type_shape(["float8"], optional_modifier(*precision), 0)
        }
        DataType::Real { .. } => data_type_shape(["float4"], std::iter::empty::<i64>(), 0),
        DataType::Double { .. } => data_type_shape(["float8"], std::iter::empty::<i64>(), 0),
        DataType::Text { .. } => data_type_shape(["text"], std::iter::empty::<i64>(), 0),
        DataType::Character { spelling, size, .. } => match spelling {
            CharacterTypeName::Char
            | CharacterTypeName::Character
            | CharacterTypeName::Nchar
            | CharacterTypeName::NationalChar
            | CharacterTypeName::NationalCharacter => {
                data_type_shape(["bpchar"], optional_modifier(*size), 0)
            }
            CharacterTypeName::CharVarying
            | CharacterTypeName::CharacterVarying
            | CharacterTypeName::Varchar
            | CharacterTypeName::NcharVarying
            | CharacterTypeName::NationalCharVarying
            | CharacterTypeName::NationalCharacterVarying => {
                data_type_shape(["varchar"], optional_modifier(*size), 0)
            }
        },
        DataType::Binary { .. } => data_type_shape(["bytea"], std::iter::empty::<i64>(), 0),
        DataType::Bit { varying, size, .. } => data_type_shape(
            [if *varying { "varbit" } else { "bit" }],
            optional_modifier(*size),
            0,
        ),
        DataType::Json { .. } => data_type_shape(["json"], std::iter::empty::<i64>(), 0),
        DataType::Uuid { .. } => data_type_shape(["uuid"], std::iter::empty::<i64>(), 0),
        DataType::Date { .. } => data_type_shape(["date"], std::iter::empty::<i64>(), 0),
        DataType::Time {
            spelling,
            precision,
            time_zone,
            ..
        } => match (spelling, time_zone) {
            (TimeTypeName::Timetz, TimeZone::Unspecified)
            | (TimeTypeName::Timetz, TimeZone::WithTimeZone)
            | (TimeTypeName::Time, TimeZone::WithTimeZone) => {
                data_type_shape(["timetz"], optional_modifier(*precision), 0)
            }
            (TimeTypeName::Time, TimeZone::Unspecified)
            | (TimeTypeName::Time, TimeZone::WithoutTimeZone)
            | (TimeTypeName::Timetz, TimeZone::WithoutTimeZone) => {
                data_type_shape(["time"], optional_modifier(*precision), 0)
            }
        },
        DataType::Timestamp {
            spelling,
            precision,
            time_zone,
            ..
        } => match (spelling, time_zone) {
            (TimestampTypeName::Timestamptz, TimeZone::Unspecified)
            | (TimestampTypeName::Timestamptz, TimeZone::WithTimeZone)
            | (TimestampTypeName::Timestamp, TimeZone::WithTimeZone) => {
                data_type_shape(["timestamptz"], optional_modifier(*precision), 0)
            }
            (TimestampTypeName::Timestamp, TimeZone::Unspecified)
            | (TimestampTypeName::Timestamp, TimeZone::WithoutTimeZone)
            | (TimestampTypeName::Timestamptz, TimeZone::WithoutTimeZone) => {
                data_type_shape(["timestamp"], optional_modifier(*precision), 0)
            }
            // MySQL `DATETIME` is gated off for PostgreSQL, so it never reaches here.
            (TimestampTypeName::Datetime, _) => {
                unreachable!("MySQL DATETIME is not in the PostgreSQL structural corpus")
            }
        },
        DataType::Interval { precision, .. } => {
            data_type_shape(["interval"], optional_modifier(*precision), 0)
        }
        DataType::Array { element, .. } => {
            increment_array_depth(squonk_data_type_shape(parsed, element))
        }
        // The MySQL-only type names, the DuckDB-only anonymous composite / nested types
        // (`STRUCT`/`ROW`/`UNION`/`MAP`), and the ClickHouse-only `Nullable(T)` combinator /
        // `FixedString(N)` / `DateTime64(P)` / `Nested(...)` constructors are gated off under
        // the PostgreSQL preset, so a
        // PostgreSQL-parsed tree can never carry them (like the non-PG `Expr` forms mapped to
        // `unreachable!` above).
        DataType::TinyInt { .. }
        | DataType::MediumInt { .. }
        | DataType::Blob { .. }
        | DataType::Enum { .. }
        | DataType::Set { .. }
        | DataType::NumericModifier { .. }
        | DataType::Struct { .. }
        | DataType::Union { .. }
        | DataType::Map { .. }
        | DataType::Wrapped { .. }
        | DataType::FixedString { .. }
        | DataType::DateTime64 { .. }
        | DataType::Nested { .. }
        | DataType::FixedWidthInt { .. }
        | DataType::Liberal { .. } => {
            unreachable!(
                "MySQL-, DuckDB-, SQLite-, and ClickHouse-only data types are not in the \
                 PostgreSQL structural corpus"
            )
        }
        DataType::UserDefined {
            name, modifiers, ..
        } => DataTypeShape::Named {
            name: object_name_shape(parsed, name),
            // Type modifiers in the PostgreSQL structural corpus are integer typmods; a
            // string modifier is DuckDB's `GEOMETRY('OGC:CRS84')`, which rides the separate
            // `json_serialize_sql` lane and never reaches this PostgreSQL mapper. Materialize
            // the integer value; a non-integer defaults to 0 (unreached here).
            modifiers: modifiers
                .iter()
                .map(|modifier| modifier.as_i64(parsed.source()).unwrap_or(0))
                .collect(),
            array_depth: 0,
        },
        // Uninhabited under the builtin `NoExt`, like the other seams above.
        DataType::Other { ext, .. } => match *ext {},
    }
}

fn data_type_shape<const N: usize, M>(
    name: [&'static str; N],
    modifiers: M,
    array_depth: usize,
) -> DataTypeShape
where
    M: IntoIterator<Item = i64>,
{
    DataTypeShape::Named {
        name: name.into_iter().map(str::to_owned).collect(),
        modifiers: modifiers.into_iter().collect(),
        array_depth,
    }
}

fn optional_modifier(modifier: Option<u32>) -> Vec<i64> {
    modifier.into_iter().map(i64::from).collect::<Vec<i64>>()
}

fn numeric_modifiers(precision: Option<i32>, scale: Option<i32>) -> Vec<i64> {
    match (precision, scale) {
        (Some(precision), Some(scale)) => vec![i64::from(precision), i64::from(scale)],
        (Some(precision), None) => vec![i64::from(precision)],
        (None, Some(scale)) => vec![i64::from(scale)],
        (None, None) => Vec::new(),
    }
}

fn increment_array_depth(data_type: DataTypeShape) -> DataTypeShape {
    match data_type {
        DataTypeShape::Named {
            name,
            modifiers,
            array_depth,
        } => DataTypeShape::Named {
            name,
            modifiers,
            array_depth: array_depth + 1,
        },
    }
}

fn binary_operator_shape(op: &BinaryOperator) -> BinaryOperatorShape {
    match op {
        BinaryOperator::Plus => BinaryOperatorShape::Plus,
        BinaryOperator::Minus => BinaryOperatorShape::Minus,
        BinaryOperator::Multiply => BinaryOperatorShape::Multiply,
        BinaryOperator::Divide => BinaryOperatorShape::Divide,
        BinaryOperator::Modulo(_) => BinaryOperatorShape::Modulo,
        BinaryOperator::Exponent => BinaryOperatorShape::Exponent,
        BinaryOperator::StringConcat => BinaryOperatorShape::StringConcat,
        // Both equality spellings (`=` and the SQLite `==`) fold onto one shape: the
        // spelling tag is a surface detail the structural comparison ignores (ADR-0015
        // representation-equivalence), exactly like `Modulo(_)` above.
        BinaryOperator::Eq(_) => BinaryOperatorShape::Eq,
        BinaryOperator::NotEq(_) => BinaryOperatorShape::NotEq,
        BinaryOperator::Lt => BinaryOperatorShape::Lt,
        BinaryOperator::LtEq => BinaryOperatorShape::LtEq,
        BinaryOperator::Gt => BinaryOperatorShape::Gt,
        BinaryOperator::GtEq => BinaryOperatorShape::GtEq,
        // `IS [NOT] DISTINCT FROM` is a PostgreSQL operator (a DISTINCT-kind `AExpr`),
        // so it maps to a real shape rather than the MySQL-only unreachable arm below.
        BinaryOperator::IsDistinctFrom(IsDistinctFromSpelling::Keyword) => {
            BinaryOperatorShape::IsDistinctFrom
        }
        BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::Keyword) => {
            BinaryOperatorShape::IsNotDistinctFrom
        }
        BinaryOperator::And => BinaryOperatorShape::And,
        BinaryOperator::Or => BinaryOperatorShape::Or,
        // The SQL-standard `OVERLAPS` period predicate is a real PostgreSQL operator (a
        // `row OVERLAPS row` form), so it maps to a real shape rather than the MySQL/SQLite
        // unreachable arm below. Its `Expr::Row` operands keep it out of the neutral
        // structural comparison (`fuzz::pg_comparable`), so this never diffs against
        // `pg_query`'s `overlaps` function-call lowering.
        BinaryOperator::Overlaps => BinaryOperatorShape::Overlaps,
        BinaryOperator::Overlap => BinaryOperatorShape::Overlap,
        // The PostgreSQL `@>`/`<@` containment and `->`/`->>` JSON operators, in the
        // PostgreSQL shape vocabulary so the differential compares them structurally.
        BinaryOperator::Contains => BinaryOperatorShape::Contains,
        BinaryOperator::ContainedBy => BinaryOperatorShape::ContainedBy,
        BinaryOperator::JsonGet => BinaryOperatorShape::JsonGet,
        BinaryOperator::JsonGetText => BinaryOperatorShape::JsonGetText,
        // The PostgreSQL `jsonb` existence/path/search operators, in the PostgreSQL shape
        // vocabulary so the differential compares them structurally.
        BinaryOperator::JsonExists => BinaryOperatorShape::JsonExists,
        BinaryOperator::JsonExistsAny => BinaryOperatorShape::JsonExistsAny,
        BinaryOperator::JsonExistsAll => BinaryOperatorShape::JsonExistsAll,
        BinaryOperator::JsonPathExists => BinaryOperatorShape::JsonPathExists,
        BinaryOperator::JsonPathMatch => BinaryOperatorShape::JsonPathMatch,
        BinaryOperator::JsonExtractPath => BinaryOperatorShape::JsonExtractPath,
        BinaryOperator::JsonExtractPathText => BinaryOperatorShape::JsonExtractPathText,
        BinaryOperator::JsonDeletePath => BinaryOperatorShape::JsonDeletePath,
        // The bitwise operators PostgreSQL parses (`| & << >>` and the `#` XOR); the
        // structural oracle compares them by shape so the per-dialect precedence is
        // engine-checked. MySQL's `^` XOR spelling never reaches this PostgreSQL oracle.
        BinaryOperator::BitwiseOr => BinaryOperatorShape::BitwiseOr,
        BinaryOperator::BitwiseAnd => BinaryOperatorShape::BitwiseAnd,
        BinaryOperator::BitwiseShiftLeft => BinaryOperatorShape::BitwiseShiftLeft,
        BinaryOperator::BitwiseShiftRight => BinaryOperatorShape::BitwiseShiftRight,
        BinaryOperator::BitwiseXor(BitwiseXorSpelling::Hash) => BinaryOperatorShape::BitwiseXor,
        // `DIV`/`XOR`/`RLIKE`/`REGEXP`/`<=>`/`^`-XOR (MySQL) and `GLOB`/`MATCH` (SQLite) are
        // operators outside the PostgreSQL grammar, so a tree parsed by this PostgreSQL
        // oracle can never carry them — `BinaryOperatorShape` stays the PostgreSQL shape
        // vocabulary. The `<=>` spelling of the null-safe operator is MySQL-only; its
        // keyword spelling (`IS NOT DISTINCT FROM`) is a real PostgreSQL shape above. The
        // bare-`IS` spellings (`a IS b` / `a IS NOT b`) are SQLite-only surface for the same
        // two null-safe operators; PostgreSQL never parses them.
        BinaryOperator::IntegerDivide(_)
        | BinaryOperator::Xor
        | BinaryOperator::Regexp(_)
        | BinaryOperator::Glob
        | BinaryOperator::StartsWith
        | BinaryOperator::Match
        | BinaryOperator::BitwiseXor(BitwiseXorSpelling::Caret)
        | BinaryOperator::IsDistinctFrom(IsDistinctFromSpelling::Is)
        | BinaryOperator::IsNotDistinctFrom(
            IsNotDistinctFromSpelling::NullSafeEq | IsNotDistinctFromSpelling::Is,
        ) => {
            unreachable!("PostgreSQL does not parse the MySQL/SQLite keyword operators")
        }
    }
}

fn unary_operator_shape(op: &UnaryOperator) -> UnaryOperatorShape {
    match op {
        UnaryOperator::Not => UnaryOperatorShape::Not,
        UnaryOperator::Minus => UnaryOperatorShape::Minus,
        UnaryOperator::Plus => UnaryOperatorShape::Plus,
        UnaryOperator::BitwiseNot => UnaryOperatorShape::BitwiseNot,
        UnaryOperator::Prior => UnaryOperatorShape::Prior,
    }
}

/// Build a unary-operator shape, folding a unary minus over a numeric literal into
/// the signed literal PostgreSQL produces.
///
/// ADR-0015 (representation-equivalence, category 1; owned by this ticket): our
/// tokenizer keeps `-` a separate operator, so `-1` parses as `UnaryOp(Minus, 1)`,
/// while PostgreSQL's grammar folds the sign into the constant (`doNegate`). This
/// mirrors `doNegate` exactly — it folds *only* a direct numeric-literal operand
/// (so `-(a + b)` and `-col` keep their `UnaryOp`, and operator precedence stays
/// structural), drops the sign for integer zero (`-0` is `0`), and flips the
/// leading sign for floats (preserving `-0.0`). Because the inner operand is mapped
/// first, nested signs (`- -1`) fold correctly too.
fn fold_unary(op: UnaryOperatorShape, operand: ExprShape) -> ExprShape {
    match (op, operand) {
        (UnaryOperatorShape::Minus, ExprShape::Literal(LiteralShape::Integer(text))) => {
            ExprShape::Literal(LiteralShape::Integer(fold_negate_integer(&text)))
        }
        (UnaryOperatorShape::Minus, ExprShape::Literal(LiteralShape::Float(text))) => {
            ExprShape::Literal(LiteralShape::Float(fold_negate_float(&text)))
        }
        (op, operand) => ExprShape::UnaryOp {
            op,
            expr: Box::new(operand),
        },
    }
}

fn fold_negate_integer(text: &str) -> String {
    let magnitude = text.strip_prefix('-').unwrap_or(text);
    // Both `-(-n)` (already negative) and integer zero (`-0` is `0`, unsigned per
    // `doNegate`) collapse to the bare magnitude; any other value gains a sign.
    if text.starts_with('-') || magnitude.bytes().all(|byte| byte == b'0') {
        magnitude.to_owned()
    } else {
        format!("-{text}")
    }
}

fn fold_negate_float(text: &str) -> String {
    match text.strip_prefix('-') {
        Some(magnitude) => magnitude.to_owned(),
        None => format!("-{text}"),
    }
}

fn object_name_shape(parsed: &Parsed, name: &ObjectName) -> Vec<String> {
    idents_shape(parsed, &name.0)
}

fn ident_shape(parsed: &Parsed, ident: &Ident) -> String {
    parsed.resolver().resolve(ident.sym).to_owned()
}

/// Resolve every identifier in a list to its text — the neutral shape of a
/// `Vec<Ident>` / `ThinVec<Ident>` field.
fn idents_shape(parsed: &Parsed, idents: &[Ident]) -> Vec<String> {
    idents
        .iter()
        .map(|ident| ident_shape(parsed, ident))
        .collect()
}

/// The neutral name shape of a `PRIMARY KEY`/`UNIQUE` constraint column list. The
/// structural oracles (PostgreSQL/DuckDB) accept only bare column names here — no
/// `COLLATE`/`ASC`/`DESC` — so each [`IndexColumn`] is an [`Expr::Column`] and this
/// projects its dotted name, matching the engines' bare-name constraint trees.
fn constraint_columns_shape(parsed: &Parsed, columns: &[IndexColumn<NoExt>]) -> Vec<String> {
    columns
        .iter()
        .map(|column| match &column.expr {
            Expr::Column { name, .. } => object_name_shape(parsed, name).join("."),
            other => format!("{:?}", expr_shape(parsed, other)),
        })
        .collect()
}

/// Resolve an optional identifier to its text — the neutral shape of an
/// `Option<Ident>` field.
fn opt_ident_shape(parsed: &Parsed, ident: &Option<Ident>) -> Option<String> {
    ident.as_ref().map(|ident| ident_shape(parsed, ident))
}
