// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The `Normalized*` structural type model compared by the round-trip oracle.

use squonk_ast::{
    ApplyKind, AsOfJoinKind, BinaryOperator, BinaryTypeName, BooleanTypeName, CharacterTypeName,
    DecimalTypeName, DoubleTypeName, GeneratedColumnStorage, IdentityGeneration, InsertOverriding,
    IntegerTypeName, IntervalFields, LimitPercent, LimitSyntax, LiteralKind, NullInclusion,
    OnCommitAction, PivotSpelling, Quantifier, RelationInheritance, SemiAntiSide, SetOperator,
    SetQuantifier, SpecialFunctionKeyword, TemporaryTableKind, TimeTypeName, TimeZone,
    TimestampTypeName, UnaryOperator, UnpivotSpelling, WindowFrameExclusion, WindowFrameUnits,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NormalizedStatement {
    Query(Box<NormalizedQuery>),
    CreateTable(NormalizedCreateTable),
    Insert(NormalizedInsert),
    Update(NormalizedUpdate),
    Delete(NormalizedDelete),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedInsert {
    pub(crate) with: Option<NormalizedWith>,
    pub(crate) target: NormalizedInsertTarget,
    pub(crate) overriding: Option<InsertOverriding>,
    pub(crate) source: NormalizedInsertSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedInsertTarget {
    pub(crate) name: Vec<String>,
    pub(crate) alias: Option<String>,
    pub(crate) columns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedInsertSource {
    DefaultValues,
    Values(Vec<Vec<NormalizedInsertValue>>),
    Query(Box<NormalizedQuery>),
    Set(Vec<NormalizedUpdateAssignment>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedInsertValue {
    Expr(NormalizedExpr),
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedValuesItem {
    Expr(NormalizedExpr),
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedUpdate {
    pub(crate) with: Option<NormalizedWith>,
    pub(crate) target: NormalizedDmlTarget,
    pub(crate) assignments: Vec<NormalizedUpdateAssignment>,
    pub(crate) from: Vec<NormalizedTableWithJoins>,
    pub(crate) selection: Option<NormalizedDmlSelection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedDelete {
    pub(crate) with: Option<NormalizedWith>,
    pub(crate) target: NormalizedDmlTarget,
    pub(crate) using: Vec<NormalizedTableWithJoins>,
    pub(crate) selection: Option<NormalizedDmlSelection>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedDmlTarget {
    pub(crate) name: Vec<String>,
    pub(crate) inheritance: RelationInheritance,
    pub(crate) alias: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedDmlSelection {
    Where(NormalizedExpr),
    CurrentOf(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedUpdateAssignment {
    Single {
        target: Vec<String>,
        value: NormalizedUpdateValue,
    },
    Tuple {
        targets: Vec<Vec<String>>,
        source: NormalizedUpdateTupleSource,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedUpdateValue {
    Expr(NormalizedExpr),
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedUpdateTupleSource {
    Row {
        explicit: bool,
        values: Vec<NormalizedUpdateValue>,
    },
    Subquery(Box<NormalizedQuery>),
    Default,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedCreateTable {
    pub(crate) temporary: Option<TemporaryTableKind>,
    pub(crate) if_not_exists: bool,
    pub(crate) name: Vec<String>,
    pub(crate) body: NormalizedCreateTableBody,
    pub(crate) options: Vec<NormalizedCreateTableOption>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedCreateTableBody {
    Definition(Vec<NormalizedTableElement>),
    AsQuery {
        columns: Vec<String>,
        query: Box<NormalizedQuery>,
        with_data: Option<bool>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedTableElement {
    Column(NormalizedColumnDef),
    Constraint(NormalizedTableConstraintDef),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedColumnDef {
    pub(crate) name: String,
    /// `None` for a SQLite typeless column, so the omitted type round-trips distinctly.
    pub(crate) data_type: Option<NormalizedDataType>,
    pub(crate) constraints: Vec<NormalizedColumnConstraint>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedColumnConstraint {
    pub(crate) name: Option<String>,
    pub(crate) option: NormalizedColumnOption,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedColumnOption {
    Null,
    NotNull,
    Default(NormalizedExpr),
    Generated {
        expr: NormalizedExpr,
        storage: Option<GeneratedColumnStorage>,
    },
    Identity {
        generation: IdentityGeneration,
        options: Vec<NormalizedIdentityOption>,
    },
    PrimaryKey,
    Unique,
    AutoIncrement,
    /// The column `COLLATE` clause's (possibly qualified) name parts.
    Collate(Vec<String>),
    Check(NormalizedExpr),
    References(NormalizedForeignKeyRef),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedIdentityOption {
    StartWith(NormalizedExpr),
    IncrementBy(NormalizedExpr),
    MinValue(Option<NormalizedExpr>),
    MaxValue(Option<NormalizedExpr>),
    Cache(NormalizedExpr),
    Cycle(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedTableConstraintDef {
    pub(crate) name: Option<String>,
    pub(crate) constraint: NormalizedTableConstraint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedTableConstraint {
    PrimaryKey(Vec<NormalizedIndexColumn>),
    Unique(Vec<NormalizedIndexColumn>),
    Check(NormalizedExpr),
    ForeignKey {
        columns: Vec<String>,
        references: NormalizedForeignKeyRef,
    },
}

/// A normalized `PRIMARY KEY`/`UNIQUE` key column — the `IndexColumn` shape (the
/// column expression, carrying any `COLLATE`, plus the `ASC`/`DESC` order). Non-lossy
/// so a dropped `COLLATE`/order surfaces as a round-trip inequality rather than being
/// masked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedIndexColumn {
    pub(crate) expr: NormalizedExpr,
    pub(crate) asc: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedForeignKeyRef {
    pub(crate) table: Vec<String>,
    pub(crate) columns: Vec<String>,
    pub(crate) match_type: Option<squonk_ast::ForeignKeyMatch>,
    pub(crate) on_delete: Option<NormalizedReferentialAction>,
    pub(crate) on_update: Option<NormalizedReferentialAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedReferentialAction {
    NoAction,
    Restrict,
    Cascade,
    SetNull(Vec<String>),
    SetDefault(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedCreateTableOption {
    pub(crate) kind: NormalizedCreateTableOptionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedCreateTableOptionKind {
    With(Vec<NormalizedTableStorageParameter>),
    OnCommit(OnCommitAction),
    Tablespace(String),
    KeyValue {
        name: String,
        value: NormalizedTableOptionValue,
    },
    WithoutRowid,
    Strict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedTableOptionValue {
    Word(String),
    // String/number values normalize to their literal kind, mirroring how
    // `NormalizedExpr::Literal` drops the value text (the span the literal rides).
    Literal(LiteralKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedTableStorageParameter {
    pub(crate) name: Vec<String>,
    pub(crate) value: Option<NormalizedExpr>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizedQuery {
    pub(crate) with: Option<NormalizedWith>,
    pub(crate) body: NormalizedSetExpr,
    pub(crate) order_by: Vec<NormalizedOrderBy>,
    pub(crate) limit: Option<NormalizedLimit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedSetExpr {
    Select(NormalizedSelect),
    Values(Vec<Vec<NormalizedValuesItem>>),
    Query(Box<NormalizedQuery>),
    SetOperation {
        op: SetOperator,
        all: bool,
        left: Box<NormalizedSetExpr>,
        right: Box<NormalizedSetExpr>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedWith {
    pub(crate) recursive: bool,
    pub(crate) ctes: Vec<NormalizedCte>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedCte {
    pub(crate) name: String,
    pub(crate) columns: Vec<String>,
    pub(crate) using_key: Option<Vec<String>>,
    pub(crate) materialized: Option<bool>,
    pub(crate) body: NormalizedCteBody,
}

/// The CTE body (mirroring the canonical `CteBody`): a query, or one of the
/// data-modifying arms the normalized model already covers for the top-level
/// statements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedCteBody {
    Query(Box<NormalizedQuery>),
    Insert(Box<NormalizedInsert>),
    Update(Box<NormalizedUpdate>),
    Delete(Box<NormalizedDelete>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedSelect {
    pub(crate) distinct: bool,
    pub(crate) projection: Vec<NormalizedSelectItem>,
    pub(crate) from: Vec<NormalizedTableWithJoins>,
    pub(crate) selection: Option<NormalizedExpr>,
    pub(crate) group_by: Vec<NormalizedGroupByItem>,
    pub(crate) having: Option<NormalizedExpr>,
    pub(crate) windows: Vec<NormalizedNamedWindow>,
}

/// Symbol-resolved mirror of [`GroupByItem`](squonk_ast::GroupByItem) for the round-trip structural oracle.
/// The generators only produce the `Expr` variant, but the grouping-set variants are
/// modelled so a future generator (or a real-SQL corpus entry) round-trips through
/// the oracle without a mapping gap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedGroupByItem {
    Expr(NormalizedExpr),
    Rollup(Vec<NormalizedExpr>),
    Cube(Vec<NormalizedExpr>),
    GroupingSets(Vec<NormalizedGroupByItem>),
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedNamedWindow {
    pub(crate) name: String,
    pub(crate) definition: NormalizedWindowDefinition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedWindowSpec {
    Named(String),
    Inline(NormalizedWindowDefinition),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedWindowDefinition {
    pub(crate) existing: Option<String>,
    pub(crate) partition_by: Vec<NormalizedExpr>,
    pub(crate) order_by: Vec<NormalizedOrderBy>,
    pub(crate) frame: Option<NormalizedWindowFrame>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedWindowFrame {
    pub(crate) units: WindowFrameUnits,
    pub(crate) start: NormalizedWindowFrameBound,
    pub(crate) end: Option<NormalizedWindowFrameBound>,
    pub(crate) exclusion: Option<WindowFrameExclusion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedWindowFrameBound {
    CurrentRow,
    UnboundedPreceding,
    UnboundedFollowing,
    Preceding(NormalizedExpr),
    Following(NormalizedExpr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedSelectItem {
    Wildcard,
    QualifiedWildcard(Vec<String>),
    Expr {
        expr: NormalizedExpr,
        alias: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedTableWithJoins {
    pub(crate) relation: NormalizedTableFactor,
    pub(crate) joins: Vec<NormalizedJoin>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedTableFactor {
    Table {
        name: Vec<String>,
        inheritance: RelationInheritance,
        alias: Option<NormalizedTableAlias>,
        sample: Option<NormalizedTableSample>,
    },
    Derived {
        lateral: bool,
        subquery: Box<NormalizedQuery>,
        alias: Option<NormalizedTableAlias>,
    },
    Function {
        lateral: bool,
        // Boxed to keep this the same width as the enum's other variants: an inline
        // `NormalizedFunctionCall` is the widest payload, mirroring the boxed
        // `NormalizedExpr::Function` above.
        function: Box<NormalizedFunctionCall>,
        with_ordinality: bool,
        alias: Option<NormalizedTableAlias>,
        column_defs: Vec<NormalizedTableFunctionColumn>,
    },
    RowsFrom {
        lateral: bool,
        functions: Vec<NormalizedRowsFromItem>,
        with_ordinality: bool,
        alias: Option<NormalizedTableAlias>,
    },
    Unnest {
        lateral: bool,
        array_exprs: Vec<NormalizedExpr>,
        with_ordinality: bool,
        alias: Option<NormalizedTableAlias>,
        column_defs: Vec<NormalizedTableFunctionColumn>,
        with_offset: bool,
        with_offset_alias: Option<String>,
    },
    NestedJoin {
        table: Box<NormalizedTableWithJoins>,
        alias: Option<NormalizedTableAlias>,
    },
    SpecialFunction {
        keyword: SpecialFunctionKeyword,
        precision: Option<u32>,
        alias: Option<NormalizedTableAlias>,
    },
    // The DuckDB pivot operators. The generator never synthesizes them (no arb
    // arm), but the arms normalize fully — the SpecialFunction precedent — so the
    // equality stays real if a generated surface ever grows one. The full statement
    // surface (spelling, WITH, ORDER BY, LIMIT) is kept: rendering depends on it,
    // so dropping it would hide a spelling-flip bug from the round-trip property.
    Pivot(Box<NormalizedPivot>),
    Unpivot(Box<NormalizedUnpivot>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedPivot {
    pub(crate) source: Box<NormalizedTableFactor>,
    pub(crate) aggregates: Vec<NormalizedPivotExpr>,
    pub(crate) pivot_on: Vec<NormalizedPivotColumn>,
    pub(crate) group_by: Vec<NormalizedExpr>,
    pub(crate) with: Option<NormalizedWith>,
    pub(crate) order_by: Vec<NormalizedOrderBy>,
    /// The `ORDER BY ALL` mode's `(asc, nulls_first)` modifiers.
    pub(crate) order_by_all: Option<(Option<bool>, Option<bool>)>,
    pub(crate) limit: Option<NormalizedLimit>,
    pub(crate) spelling: PivotSpelling,
    pub(crate) alias: Option<NormalizedTableAlias>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedUnpivot {
    pub(crate) source: Box<NormalizedTableFactor>,
    pub(crate) value: Vec<String>,
    pub(crate) name: Vec<String>,
    pub(crate) columns: Vec<NormalizedUnpivotColumn>,
    pub(crate) null_inclusion: Option<NullInclusion>,
    pub(crate) with: Option<NormalizedWith>,
    pub(crate) order_by: Vec<NormalizedOrderBy>,
    /// The `ORDER BY ALL` mode's `(asc, nulls_first)` modifiers.
    pub(crate) order_by_all: Option<(Option<bool>, Option<bool>)>,
    pub(crate) limit: Option<NormalizedLimit>,
    pub(crate) spelling: UnpivotSpelling,
    pub(crate) alias: Option<NormalizedTableAlias>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedPivotExpr {
    pub(crate) expr: NormalizedExpr,
    pub(crate) alias: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedPivotColumn {
    pub(crate) expr: NormalizedExpr,
    pub(crate) values: Vec<NormalizedPivotExpr>,
    pub(crate) enum_source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedUnpivotColumn {
    pub(crate) columns: Vec<NormalizedExpr>,
    pub(crate) alias: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedTableAlias {
    pub(crate) name: String,
    pub(crate) columns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedTableSample {
    pub(crate) method: Vec<String>,
    pub(crate) args: Vec<NormalizedExpr>,
    pub(crate) repeatable: Option<NormalizedExpr>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedFunctionCall {
    pub(crate) name: Vec<String>,
    pub(crate) quantifier: Option<SetQuantifier>,
    pub(crate) args: Vec<NormalizedExpr>,
    pub(crate) wildcard: bool,
    pub(crate) order_by: Vec<NormalizedOrderBy>,
    pub(crate) within_group: Option<Vec<NormalizedOrderBy>>,
    pub(crate) filter: Option<Box<NormalizedExpr>>,
    pub(crate) over: Option<NormalizedWindowSpec>,
}

/// One `ROWS FROM (...)` item: a function call and its optional per-function
/// column definition list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedRowsFromItem {
    pub(crate) function: NormalizedFunctionCall,
    pub(crate) column_defs: Vec<NormalizedTableFunctionColumn>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedTableFunctionColumn {
    pub(crate) name: String,
    pub(crate) data_type: NormalizedDataType,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedJoin {
    pub(crate) relation: NormalizedTableFactor,
    pub(crate) operator: NormalizedJoinOperator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedJoinOperator {
    Inner(NormalizedJoinConstraint),
    LeftOuter(NormalizedJoinConstraint),
    RightOuter(NormalizedJoinConstraint),
    FullOuter(NormalizedJoinConstraint),
    // The AST side enum is reused directly, like `SpecialFunctionKeyword` below.
    AsOf(AsOfJoinKind, NormalizedJoinConstraint),
    Cross,
    Positional,
    // DuckDB side-less + Spark sided SEMI/ANTI joins; the `bool` is the `ASOF`
    // composition flag (side-less only), the `SemiAntiSide` the surface side.
    Semi(bool, SemiAntiSide, NormalizedJoinConstraint),
    Anti(bool, SemiAntiSide, NormalizedJoinConstraint),
    // MSSQL CROSS/OUTER APPLY; constraint-less like `Cross`, the `kind` is the flavour.
    Apply(ApplyKind),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedJoinConstraint {
    On(NormalizedExpr),
    Using {
        columns: Vec<String>,
        alias: Option<String>,
    },
    Natural,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedOrderBy {
    pub(crate) expr: NormalizedExpr,
    pub(crate) asc: Option<bool>,
    // The schema-qualified operator name parts with the operator symbol last
    // (mirrors `pg::OrderByShape::using`), or `None` for the ordinary
    // `ASC`/`DESC` form.
    pub(crate) using: Option<Vec<String>>,
    pub(crate) nulls_first: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NormalizedLimit {
    pub(crate) limit: Option<NormalizedExpr>,
    pub(crate) offset: Option<NormalizedExpr>,
    pub(crate) syntax: LimitSyntax,
    pub(crate) with_ties: Option<bool>,
    pub(crate) percent: Option<LimitPercent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedExpr {
    Column(Vec<String>),
    Literal(LiteralKind),
    Binary {
        left: Box<NormalizedExpr>,
        op: BinaryOperator,
        right: Box<NormalizedExpr>,
    },
    Unary {
        op: UnaryOperator,
        expr: Box<NormalizedExpr>,
    },
    // Boxed to break the type cycle a window function introduces:
    // `Function` -> call `over` -> window frame bound offset -> `NormalizedExpr`.
    Function(Box<NormalizedFunctionCall>),
    Cast {
        expr: Box<NormalizedExpr>,
        data_type: NormalizedDataType,
    },
    InSubquery {
        expr: Box<NormalizedExpr>,
        subquery: Box<NormalizedQuery>,
        negated: bool,
    },
    Exists(Box<NormalizedQuery>),
    QuantifiedComparison {
        left: Box<NormalizedExpr>,
        op: BinaryOperator,
        quantifier: Quantifier,
        subquery: Box<NormalizedQuery>,
    },
    Subquery(Box<NormalizedQuery>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalizedDataType {
    Boolean(BooleanTypeName),
    SmallInt,
    Integer(IntegerTypeName),
    BigInt,
    Decimal {
        spelling: DecimalTypeName,
        precision: Option<i32>,
        scale: Option<i32>,
    },
    Float(Option<u32>),
    Real,
    Double(DoubleTypeName),
    Text,
    Character {
        spelling: CharacterTypeName,
        size: Option<u32>,
    },
    Binary {
        spelling: BinaryTypeName,
        size: Option<u32>,
    },
    Date,
    Time {
        spelling: TimeTypeName,
        precision: Option<u32>,
        time_zone: TimeZone,
    },
    Timestamp {
        spelling: TimestampTypeName,
        precision: Option<u32>,
        time_zone: TimeZone,
    },
    Interval {
        fields: Option<IntervalFields>,
        precision: Option<u32>,
    },
    Array(Box<NormalizedDataType>),
    UserDefined {
        name: Vec<String>,
        /// Each constant modifier by its [`LiteralKind`] — mirroring how
        /// [`NormalizedExpr::Literal`](super::normalized::NormalizedExpr) drops a literal to
        /// its kind. The exact spelling round-trips from the span, so kind-level equality is
        /// enough for the normalization idempotence property.
        modifiers: Vec<LiteralKind>,
    },
}
