// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! DDL statement AST nodes: table, column, and constraint definitions.

use super::{
    DataType, ExecuteStatement, Expr, Extension, Ident, IntervalFields, LanguageName, Literal,
    NamedOperatorSpelling, NoExt, ObjectName, Query, RoutineObjectKind, RoutineSignature,
    Statement,
};
use crate::vocab::{Meta, Symbol};
use thin_vec::ThinVec;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL create table.
pub struct CreateTable<X: Extension = NoExt> {
    /// `CREATE OR REPLACE TABLE` (DuckDB): atomically replaces an existing table.
    /// Gated by [`CreateTableClauseSyntax::create_or_replace_table`](crate::dialect::CreateTableClauseSyntax::create_or_replace_table) —
    /// off elsewhere, where `OR REPLACE` before `TABLE` surfaces as a clean parse error
    /// (only `VIEW`/`FUNCTION` take `OR REPLACE` in the other dialects).
    pub or_replace: bool,
    /// Optional temporary for this syntax.
    pub temporary: Option<TemporaryTableKind>,
    /// `CREATE UNLOGGED TABLE` (PostgreSQL; also a DuckDB no-op): the table's data is not
    /// written to the write-ahead log. PostgreSQL's `OptTemp` grammar makes `UNLOGGED` a
    /// *peer* of `TEMP`/`TEMPORARY` — the two are mutually exclusive, so a set `unlogged`
    /// never co-occurs with a `Some(temporary)` (`CREATE TEMP UNLOGGED TABLE` is a raw-parse
    /// error, reproduced here). Gated for acceptance by
    /// [`CreateTableClauseSyntax::unlogged_tables`](crate::dialect::CreateTableClauseSyntax::unlogged_tables) — off
    /// elsewhere (ANSI/MySQL/SQLite), where `UNLOGGED` after `CREATE` surfaces as a clean
    /// parse error.
    pub unlogged: bool,
    /// Whether the if not exists form was present in the source.
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Statement or query body governed by this node.
    pub body: CreateTableBody<X>,
    /// The `INHERITS (<parent>, ...)` legacy table-inheritance clause (PostgreSQL), naming the
    /// parent tables this table inherits columns and constraints from. It follows the
    /// [`Definition`](CreateTableBody::Definition) `(elements)` body and precedes both
    /// [`partition_by`](Self::partition_by) and the trailing [`options`](Self::options)
    /// (PostgreSQL grammar order: `(…) INHERITS (…) PARTITION BY … WITH … ON COMMIT … TABLESPACE
    /// …`). Empty when no clause is written — an `INHERITS ()` with no parent is a raw-parse
    /// error, so a non-empty list always corresponds to a written clause and no separate
    /// `Option` is needed. Only the `(elements)` definition body carries it (the `PARTITION OF`,
    /// `OF <type>`, and `AS <query>` bodies do not), so it is empty for those. Gated for
    /// acceptance by
    /// [`CreateTableClauseSyntax::table_inheritance`](crate::dialect::CreateTableClauseSyntax::table_inheritance) — off
    /// elsewhere, where the `INHERITS` keyword is left unconsumed and surfaces as a clean parse
    /// error.
    pub inherits: ThinVec<ObjectName>,
    /// The trailing `PARTITION BY {LIST | RANGE | HASH} (<key>, ...)` declarative-partitioning
    /// clause (PostgreSQL), marking this table as a partitioned *parent*. It follows the
    /// [`Definition`](CreateTableBody::Definition) body (`CREATE TABLE t (...) PARTITION BY
    /// LIST (a)`) and the [`PartitionOf`](CreateTableBody::PartitionOf) child body (a
    /// sub-partitioned child, `... FOR VALUES IN (1) PARTITION BY RANGE (b)`); it never rides
    /// an `AS <query>` body. Boxed as a cold clause so a non-partitioned table pays only a
    /// null pointer. `None` when the table is not a partition parent. Gated for acceptance by
    /// [`CreateTableClauseSyntax::declarative_partitioning`](crate::dialect::CreateTableClauseSyntax::declarative_partitioning) —
    /// off elsewhere, where the `PARTITION` keyword is left unconsumed and surfaces as a clean
    /// parse error.
    pub partition_by: Option<Box<PartitionSpec<X>>>,
    /// The `USING <access_method>` table access-method clause (PostgreSQL): the storage
    /// engine backing the table (`heap`, an extension-provided method). In PostgreSQL's
    /// `CreateStmt` grammar it sits *after* the body, [`inherits`](Self::inherits), and
    /// [`partition_by`](Self::partition_by) but *before* the trailing
    /// [`options`](Self::options) (`(…) INHERITS (…) PARTITION BY … USING heap WITH (…) ON
    /// COMMIT … TABLESPACE …`) — a `WITH (…)` before `USING` is a raw-parse error, so parsing
    /// it here reproduces that order. On an `AS <query>` body the slot instead precedes the
    /// `AS` (`CreateTableAsStmt`: `CREATE TABLE t [(cols)] USING m [WITH (…)] AS query`; a
    /// `USING` *after* the query is a raw-parse error). The method is a single (optionally
    /// quoted) identifier — a qualified `schema.method` is a raw-parse error. `None` when the
    /// clause is unwritten. Boxed as a cold clause (rarely written) so a plain `CREATE TABLE`
    /// pays only a null pointer, keeping this node within its size budget (the box/inline
    /// budget). Gated for acceptance by
    /// [`CreateTableClauseSyntax::table_access_method`](crate::dialect::CreateTableClauseSyntax::table_access_method) — off
    /// elsewhere, where the `USING` keyword is left unconsumed and surfaces as a clean parse
    /// error.
    pub access_method: Option<Box<Ident>>,
    /// Options supplied in source order.
    pub options: ThinVec<CreateTableOption<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Surface spelling for temporary table creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TemporaryTableKind {
    /// `TEMP` — a session-local temporary table (the short spelling).
    Temp,
    /// `TEMPORARY` — a session-local temporary table.
    Temporary,
}

/// The `CREATE TABLE` body: a `(elements)` definition, an `AS <query>`, a `PARTITION OF`
/// declarative-partitioning child, or an `OF <type>` typed table.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CreateTableBody<X: Extension = NoExt> {
    /// A parenthesized `(col …, constraint …)` column/constraint definition list.
    Definition {
        /// elements in source order.
        elements: ThinVec<TableElement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `CREATE TABLE … AS <query>` — populate the table from a query.
    AsQuery {
        /// Columns in source order.
        columns: ThinVec<Ident>,
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Whether the with data form was present in the source.
        with_data: Option<bool>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `AS EXECUTE <prepared> [(<arg>, ...)] [WITH [NO] DATA]` — a CTAS whose source rows come
    /// from running a prepared statement rather than an inline query (PostgreSQL's
    /// `CreateTableAsStmt` over an `ExecuteStmt`, `CREATE TABLE t AS EXECUTE plan(1, 2)`). The
    /// optional [`columns`](Self::AsExecute::columns) rename the result columns (empty when the
    /// `(...)` list is absent) exactly as on [`AsQuery`](Self::AsQuery), and the same trailing
    /// `WITH [NO] DATA` populate flag rides here. The prepared-statement source is a reused
    /// [`ExecuteStatement`] (name + positional args), boxed to keep this variant within the enum's
    /// size budget. Gated for acceptance by
    /// [`CreateTableClauseSyntax::create_table_as_execute`](crate::dialect::CreateTableClauseSyntax::create_table_as_execute); off
    /// elsewhere, where `EXECUTE` after `AS` is left unconsumed and surfaces as a clean parse
    /// error (the inline-query CTAS path takes over and rejects the `EXECUTE` keyword).
    AsExecute {
        /// Columns in source order.
        columns: ThinVec<Ident>,
        /// The prepared statement executed to populate the table; see [`ExecuteStatement`].
        execute: Box<ExecuteStatement<X>>,
        /// Whether the with data form was present in the source.
        with_data: Option<bool>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PARTITION OF <parent> [(<augmentation>, ...)] <bound>` — a declarative-partitioning
    /// *child*, whose column shape is inherited from `parent` (PostgreSQL). The optional
    /// parenthesized augmentation list carries per-column overrides (a bare `ColId` with a
    /// `[WITH OPTIONS]` constraint list — *no* type, so each [`ColumnDef`] leaves
    /// [`data_type`](ColumnDef::data_type) `None`) and table constraints; it reuses
    /// [`TableElement`] and stays empty when the `(...)` is absent (PostgreSQL rejects an empty
    /// `()`). The `bound` is the mandatory `FOR VALUES …` / `DEFAULT` partition-bound spec.
    /// Gated for acceptance by
    /// [`CreateTableClauseSyntax::declarative_partitioning`](crate::dialect::CreateTableClauseSyntax::declarative_partitioning); off
    /// elsewhere, `PARTITION` after the table name surfaces as a clean parse error.
    PartitionOf {
        /// The parent partitioned table (`PARTITION OF <parent>`).
        parent: ObjectName,
        /// elements in source order.
        elements: ThinVec<TableElement<X>>,
        // Boxed as the fat child (a `PartitionBound` is ~48 B, the widest field here): inlining
        // it would set `CreateTableBody`'s width and tax the warm `Definition`/`AsQuery` paths,
        // so a partition child pays one cold allocation instead — the same reason `AsQuery` boxes
        // its `Query` (ADR-0007, box/inline budget).
        /// The partition bound (`FOR VALUES …` / `DEFAULT`); see [`PartitionBound`].
        bound: Box<PartitionBound<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `OF <type_name> [(<augmentation>, ...)]` — a *typed table* whose column shape is drawn
    /// from a composite type (PostgreSQL). Like [`PartitionOf`](Self::PartitionOf) the optional
    /// parenthesized list *augments* rather than declares: each element is a bare `ColId` with an
    /// optional `WITH OPTIONS` noise phrase and a constraint list (*no* type — the column's type
    /// comes from `type_name`, so each [`ColumnDef`] leaves [`data_type`](ColumnDef::data_type)
    /// `None`) or a table constraint. It reuses [`TableElement`] and stays empty when the `(...)`
    /// is absent (PostgreSQL rejects an empty `()`). Unlike the `(elements)`
    /// [`Definition`](Self::Definition) body, the `OF` form takes no `INHERITS` clause (`CREATE
    /// TABLE t OF ty INHERITS (p)` is a raw-parse error), so [`CreateTable::inherits`] is always
    /// empty for it. Gated for acceptance by
    /// [`CreateTableClauseSyntax::typed_tables`](crate::dialect::CreateTableClauseSyntax::typed_tables); off elsewhere,
    /// `OF` after the table name surfaces as a clean parse error.
    OfType {
        /// The composite type the table is defined `OF` (`CREATE TABLE t OF <type>`).
        type_name: ObjectName,
        /// elements in source order.
        elements: ThinVec<TableElement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `LIKE <source>` / `(LIKE <source>)` — MySQL's statement-level table-clone body, which
    /// copies the whole definition of an existing table (`CREATE TABLE t LIKE src`). Unlike
    /// PostgreSQL's [`TableElement::Like`] copy *element* (which sits inside a `(elements)`
    /// [`Definition`](Self::Definition) list, carries `{INCLUDING | EXCLUDING} <feature>`
    /// options, and can co-occur with other elements — gated by
    /// [`CreateTableClauseSyntax::like_source_table`](crate::dialect::CreateTableClauseSyntax::like_source_table)), this is a
    /// whole-statement production that replaces the entire body: exactly one bare source name,
    /// no feature-copy options, no co-element, no trailing table options — MySQL rejects `LIKE
    /// src ENGINE=…`, `(LIKE src, x INT)`, and `(LIKE src INCLUDING ALL)` all as
    /// `ER_PARSE_ERROR`. MySQL spells it two ways, differing only by optional parentheses
    /// ([`parenthesized`](Self::LikeSource::parenthesized) records which so the source form
    /// round-trips); both admit a qualified source name and compose freely with `IF NOT EXISTS`
    /// and `TEMPORARY`. Gated for acceptance by
    /// [`CreateTableClauseSyntax::statement_level_table_like`](crate::dialect::CreateTableClauseSyntax::statement_level_table_like);
    /// off elsewhere, `LIKE` after the table name (or as the first token inside `(`) is left
    /// unconsumed and surfaces as a clean parse error.
    LikeSource {
        /// Input source for this syntax.
        source: ObjectName,
        /// Whether the source was written parenthesized (`(LIKE src)`) rather than bare
        /// (`LIKE src`). Both are the same MySQL production; this preserves the spelling.
        parenthesized: bool,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The `PARTITION BY {LIST | RANGE | HASH} (<key>, ...)` declarative-partitioning spec on a
/// partitioned parent (or a sub-partitioned child). PostgreSQL-only; see
/// [`CreateTable::partition_by`].
///
/// PostgreSQL 17 validates the strategy word in the grammar action (`parsePartitionStrategy`),
/// so an unrecognized strategy (`PARTITION BY foo (a)`) is a raw-parse error — hence
/// [`strategy`](Self::strategy) is a closed [`PartitionStrategy`] enum, not a free identifier.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PartitionSpec<X: Extension = NoExt> {
    /// The partitioning strategy (`LIST`/`RANGE`/`HASH`); see [`PartitionStrategy`].
    pub strategy: PartitionStrategy,
    /// The partition key: one or more [`PartitionElem`]s (the grammar requires ≥ 1 — an empty
    /// `()` is a raw-parse error).
    pub columns: ThinVec<PartitionElem<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The partitioning strategy keyword. A `Copy` leaf enum whose span rides the owning
/// [`PartitionSpec`], like [`DropBehavior`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PartitionStrategy {
    /// `PARTITION BY LIST` — assign rows by a discrete key value.
    List,
    /// `PARTITION BY RANGE` — assign rows by a key range.
    Range,
    /// `PARTITION BY HASH` — assign rows by a hash of the key.
    Hash,
}

/// One partition-key element: a key expression plus its optional `COLLATE` and operator-class
/// tails (PostgreSQL `part_elem`).
///
/// The grammar admits three key forms — a bare column (`a`), a bare `func_expr_windowless`
/// (`lower(a)`), or a parenthesized expression (`(a + b)`) — the first two rendered bare and
/// the last parenthesized. [`parenthesized`](Self::parenthesized) records which so the source
/// form round-trips; the `COLLATE` and operator-class clauses are *separate* `part_elem`
/// tails (not expression postfixes), so a bare-column key keeps them out of `expr`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PartitionElem<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Expr<X>,
    /// Whether the key was written parenthesized (`(a_expr)`). A bare column / function call is
    /// unparenthesized; every other expression form must be parenthesized (the grammar's
    /// `'(' a_expr ')'` production), and rendering re-adds the parentheses from this flag.
    pub parenthesized: bool,
    /// Optional collation for this syntax.
    pub collation: Option<ObjectName>,
    /// The trailing operator-class name (`opt_qualified_name`), when written —
    /// `a part_test_int4_ops`, `a myschema.text_ops`.
    pub opclass: Option<ObjectName>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The partition-bound spec of a [`PartitionOf`](CreateTableBody::PartitionOf) child (or an
/// [`AttachPartition`](AlterTableAction::AttachPartition) action): `FOR VALUES {IN | FROM…TO |
/// WITH} (…)` or `DEFAULT` (PostgreSQL).
///
/// The bound datums are full `a_expr`s (PostgreSQL parses the `minvalue`/`maxvalue` range
/// sentinels as ordinary column references at raw-parse time, so they need no dedicated node).
/// The hash bound's `MODULUS m, REMAINDER r` are unsigned integer literals validated in the
/// grammar action — exactly one of each, either order — so they are modelled as a resolved
/// pair rather than a raw option list (a missing/duplicate/non-integer value is a raw-parse
/// error PostgreSQL raises, matched at the same layer).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PartitionBound<X: Extension = NoExt> {
    /// `FOR VALUES IN (<expr>, ...)` — a list partition (≥ 1 value).
    List {
        /// Values in source order.
        values: ThinVec<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `FOR VALUES FROM (<expr>, ...) TO (<expr>, ...)` — a range partition (each side ≥ 1).
    Range {
        /// from in source order.
        from: ThinVec<Expr<X>>,
        /// to in source order.
        to: ThinVec<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `FOR VALUES WITH (MODULUS <n>, REMAINDER <n>)` — a hash partition.
    Hash {
        /// The `MODULUS` of a `FOR VALUES WITH (MODULUS m, REMAINDER r)` hash bound.
        modulus: Literal,
        /// The `REMAINDER` of a hash partition bound.
        remainder: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DEFAULT` — the catch-all partition.
    Default {
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL table element forms represented by the AST.
pub enum TableElement<X: Extension = NoExt> {
    /// A column definition.
    Column {
        /// Column referenced by this syntax.
        column: ColumnDef<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A table-level constraint.
    Constraint {
        /// The table constraint; see [`TableConstraintDef`].
        constraint: TableConstraintDef<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `LIKE <source_table> [{INCLUDING | EXCLUDING} <feature> ...]` — the source-table copy
    /// element (PostgreSQL). Appears *inside* the parenthesized definition list (`CREATE TABLE t
    /// (a int, LIKE src INCLUDING ALL, b int)`), copying the named table's structure. The
    /// [`options`](Self::Like::options) are the repeatable, order-free
    /// `{INCLUDING | EXCLUDING} <feature>` selectors, preserved as written (PostgreSQL folds them
    /// into a bitmask, but the source sequence round-trips faithfully here); the list is empty
    /// for a bare `LIKE src`. Gated for acceptance by
    /// [`CreateTableClauseSyntax::like_source_table`](crate::dialect::CreateTableClauseSyntax::like_source_table) — off
    /// elsewhere, where `LIKE` at an element position surfaces as a clean parse error. Distinct
    /// from MySQL's statement-level `CREATE TABLE t LIKE src` (no parentheses), a separate
    /// production.
    Like {
        /// Input source for this syntax.
        source: ObjectName,
        /// Options supplied in source order.
        options: ThinVec<TableLikeOption>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL table like option.
pub struct TableLikeOption {
    /// Whether the feature is `INCLUDING` or `EXCLUDING`; see [`TableLikeAction`].
    pub action: TableLikeAction,
    /// Which feature is copied/omitted; see [`TableLikeFeature`].
    pub feature: TableLikeFeature,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Whether a [`TableLikeOption`] includes or excludes its feature. A `Copy` leaf enum whose span
/// rides the owning [`TableLikeOption`], like [`PartitionStrategy`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TableLikeAction {
    /// `INCLUDING <feature>` — copy the feature from the source table.
    Including,
    /// `EXCLUDING <feature>` — do not copy the feature.
    Excluding,
}

/// The feature a [`TableLikeOption`] copies (or omits) from the source table (PostgreSQL's
/// `TableLikeOption` set). A `Copy` leaf enum whose span rides the owning [`TableLikeOption`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TableLikeFeature {
    /// `COMMENTS` — column and constraint comments.
    Comments,
    /// `COMPRESSION` — per-column compression methods.
    Compression,
    /// `CONSTRAINTS` — `CHECK` constraints.
    Constraints,
    /// `DEFAULTS` — column default expressions.
    Defaults,
    /// `GENERATED` — generated-column expressions.
    Generated,
    /// `IDENTITY` — identity/serial column specifications.
    Identity,
    /// `INDEXES` — indexes plus primary-key and unique constraints.
    Indexes,
    /// `STATISTICS` — extended planner statistics.
    Statistics,
    /// `STORAGE` — per-column storage settings.
    Storage,
    /// `ALL` — every feature (PostgreSQL's `CREATE_TABLE_LIKE_ALL`).
    All,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL column def.
pub struct ColumnDef<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// The column's declared type, or `None` for a SQLite *typeless* column
    /// (`CREATE TABLE t (a, b)`): SQLite's flexible typing lets a column omit its
    /// type entirely (the column then takes BLOB affinity). Every other dialect
    /// requires a type, so the omission is gated for acceptance by
    /// [`ColumnDefinitionSyntax::typeless_column_definitions`](crate::dialect::ColumnDefinitionSyntax::typeless_column_definitions)
    /// — off elsewhere, where a missing type surfaces as a clean parse error. The
    /// omitted and written forms round-trip distinctly (an `Option`, not a synthesized
    /// default), so a downstream converter sees exactly what the source declared.
    pub data_type: Option<DataType<X>>,
    /// The per-column `STORAGE {<strategy> | DEFAULT}` clause (PostgreSQL): the column's
    /// on-disk storage strategy (`PLAIN` / `EXTERNAL` / `EXTENDED` / `MAIN`, or the `DEFAULT`
    /// keyword). It is a *fixed-position* clause in PostgreSQL's `columnDef` grammar — after the
    /// type and before both [`compression`](Self::compression) and the
    /// [`constraints`](Self::constraints) (`ColId Typename [STORAGE …] [COMPRESSION …]
    /// ColQualList`) — so a `STORAGE` *after* a constraint, or after `COMPRESSION`, is a
    /// raw-parse error. The value is an *open* name, not a closed enum: PostgreSQL's grammar is
    /// `STORAGE {ColId | DEFAULT}` and validates the strategy word at analysis, out of this
    /// parser's layer (`STORAGE bogus` is engine-measured accepted at raw parse), so any single
    /// (optionally quoted) identifier — or the `DEFAULT` keyword, interned as written — rides
    /// here as an [`Ident`], boxed as the cold clause it is (see
    /// [`compression`](Self::compression)). `None` when unwritten. Gated for acceptance by
    /// [`ColumnDefinitionSyntax::column_storage`](crate::dialect::ColumnDefinitionSyntax::column_storage) — off
    /// elsewhere, where the `STORAGE` keyword surfaces as a clean parse error.
    pub storage: Option<Box<Ident>>,
    /// The per-column `COMPRESSION <method>` clause (PostgreSQL): the compression method for the
    /// column's TOAST-able values (`pglz`, `lz4`, or the `DEFAULT` keyword). Like
    /// [`storage`](Self::storage) a fixed-position clause with the same open
    /// `{ColId | DEFAULT}` value grammar — after the type and [`storage`](Self::storage), before
    /// the [`constraints`](Self::constraints) — so a `COMPRESSION` after a constraint or before
    /// `STORAGE` is a raw-parse error, while a qualified `schema.method` is one too (the value
    /// is a single identifier). PostgreSQL validates the specific method name at a later stage,
    /// out of this parser's layer. `None` when unwritten. The name is boxed to keep this cold
    /// clause from widening the warm, vec-embedded [`ColumnDef`] (the box/inline budget — a wide
    /// table holds many `ColumnDef`s and `COMPRESSION` is rarely written). Gated for acceptance
    /// by the same [`ColumnDefinitionSyntax::column_storage`](crate::dialect::ColumnDefinitionSyntax::column_storage)
    /// flag as `STORAGE` (the two physical-storage attributes travel together).
    pub compression: Option<Box<Ident>>,
    /// constraints in source order.
    pub constraints: ThinVec<ColumnConstraint<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL column constraint.
pub struct ColumnConstraint<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Option<Ident>,
    /// Optional option for this syntax.
    pub option: ColumnOption<X>,
    /// The SQLite `ON CONFLICT <resolution>` clause attached to this constraint —
    /// `a INTEGER UNIQUE ON CONFLICT REPLACE`. Modelled on the constraint wrapper (like
    /// [`characteristics`](Self::characteristics)) rather than on the individual
    /// [`ColumnOption`] variants because the conflict clause qualifies a *constraint*
    /// (SQLite admits it after `NOT NULL` / `UNIQUE` / `PRIMARY KEY` / `CHECK`), and the
    /// parser only reads it after one of those. `None` when unwritten. Gated for
    /// acceptance by
    /// [`ColumnDefinitionSyntax::column_conflict_resolution_clause`](crate::dialect::ColumnDefinitionSyntax::column_conflict_resolution_clause);
    /// off elsewhere, where `ON CONFLICT` on a column constraint is a clean parse error.
    pub conflict: Option<ConflictResolution>,
    /// The trailing `DEFERRABLE`/`INITIALLY` characteristics, when written. Modelled
    /// on the constraint wrapper rather than on [`ColumnOption::References`] because
    /// the SQL characteristic qualifies a *constraint* generally (unique / primary
    /// key / foreign key), not the reference clause specifically. Boxed because it is
    /// a cold, rarely written clause: the common column constraint then pays only a
    /// null pointer, keeping this warm node lean (the box/inline budget).
    pub characteristics: Option<Box<ConstraintCharacteristics>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `<constraint characteristics>` of a constraint: its deferral mode.
///
/// `deferrable` records `DEFERRABLE` (`Some(true)`) / `NOT DEFERRABLE`
/// (`Some(false)`); `initially_deferred` records `INITIALLY DEFERRED` (`Some(true)`)
/// / `INITIALLY IMMEDIATE` (`Some(false)`). Each stays `None` when its clause is
/// unwritten so the omitted form and the explicit default round-trip distinctly —
/// PostgreSQL materializes both to `NOT DEFERRABLE INITIALLY IMMEDIATE`, so the
/// omitted and default-explicit spellings are representation-equivalent.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ConstraintCharacteristics {
    /// Whether the deferrable form was present in the source.
    pub deferrable: Option<bool>,
    /// Whether the initially deferred form was present in the source.
    pub initially_deferred: Option<bool>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL column option forms represented by the AST.
pub enum ColumnOption<X: Extension = NoExt> {
    /// An explicit `NULL` — the column is allowed to contain nulls.
    Null {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `NOT NULL` constraint.
    NotNull {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `DEFAULT <expr>` value clause.
    Default {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `GENERATED ALWAYS AS (…)` computed column.
    Generated {
        /// The generated-column specification; see [`GeneratedColumn`].
        generated: Box<GeneratedColumn<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `GENERATED … AS IDENTITY` identity column.
    Identity {
        /// The identity-column specification; see [`IdentityColumn`].
        identity: Box<IdentityColumn<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An inline `PRIMARY KEY` constraint.
    PrimaryKey {
        /// The SQLite `ASC` / `DESC` sort-order qualifier on an inline primary key
        /// (`id INT PRIMARY KEY DESC`): `Some(true)` for `ASC`, `Some(false)` for
        /// `DESC`, `None` when unwritten. Gated for acceptance by
        /// [`ColumnDefinitionSyntax::inline_primary_key_ordering`](crate::dialect::ColumnDefinitionSyntax::inline_primary_key_ordering)
        /// — off elsewhere, where the trailing `ASC`/`DESC` is a clean parse error.
        ascending: Option<bool>,
        /// The `USING INDEX TABLESPACE <name>` index-parameter clause (PostgreSQL): the
        /// tablespace of the implicit index backing this inline primary key (`a int PRIMARY
        /// KEY USING INDEX TABLESPACE ts`). `None` when unwritten. Boxed as the cold clause it
        /// is (the box/inline budget — a wide table holds many `ColumnDef`s and this is rarely
        /// written), matching [`Collate`](Self::Collate). Gated for acceptance by
        /// [`ConstraintSyntax::index_constraint_parameters`](crate::dialect::ConstraintSyntax::index_constraint_parameters);
        /// off elsewhere, where the `USING` keyword after `PRIMARY KEY` is a clean parse error.
        index_tablespace: Option<Box<Ident>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An inline `UNIQUE` constraint.
    Unique {
        /// The `NULLS [NOT] DISTINCT` null-treatment (PostgreSQL 15+): `Some(false)` for
        /// `NULLS NOT DISTINCT` (nulls collide, so at most one null row), `Some(true)` for the
        /// explicit `NULLS DISTINCT` default, `None` when unwritten. Gated for acceptance by
        /// [`ConstraintSyntax::index_constraint_parameters`](crate::dialect::ConstraintSyntax::index_constraint_parameters);
        /// off elsewhere, where the `NULLS` keyword after `UNIQUE` is a clean parse error.
        nulls_not_distinct: Option<bool>,
        /// The `USING INDEX TABLESPACE <name>` index-parameter clause (PostgreSQL), as on
        /// [`PrimaryKey`](Self::PrimaryKey::index_tablespace). `None` when unwritten. Boxed and
        /// gated identically.
        index_tablespace: Option<Box<Ident>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The MySQL `AUTO_INCREMENT` / SQLite `AUTOINCREMENT` column attribute: the
    /// column's value defaults to the next sequence number. The one canonical shape
    /// carries a [`spelling`](AutoIncrementSpelling) tag so the two surface forms
    /// round-trip. Each spelling gates on its own flag: the underscored `AUTO_INCREMENT`
    /// under `create_table_clause_syntax.table_options` (MySQL) and the joined `AUTOINCREMENT`
    /// under `column_definition_syntax.joined_autoincrement_attribute` (SQLite). Neither preset
    /// admits the other's spelling.
    AutoIncrement {
        /// Exact source spelling retained for faithful rendering.
        spelling: AutoIncrementSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The column-definition `COLLATE <collation>` clause: the column's default collating
    /// sequence. One shape spans three dialects that each spell it in a column definition —
    /// PostgreSQL (`a text COLLATE "C"` / `COLLATE pg_catalog."default"`), SQLite (`a TEXT
    /// COLLATE NOCASE`), and DuckDB (`a VARCHAR COLLATE nocase`) — interleaving freely with the
    /// other [`constraints`](ColumnDef::constraints) in PostgreSQL's `ColQualList` order. The
    /// name is a (possibly qualified, possibly quoted) [`ObjectName`], matching the
    /// expression-level `COLLATE` ([`ExpressionSyntax::collate`](crate::dialect::ExpressionSyntax));
    /// SQLite/DuckDB write a single bare identifier (a one-part name), PostgreSQL a full
    /// `any_name`. Distinct from that expression-level `COLLATE`, which qualifies an expression,
    /// not a column. Gated for acceptance by
    /// [`ColumnDefinitionSyntax::column_collation`](crate::dialect::ColumnDefinitionSyntax::column_collation); off
    /// elsewhere (ANSI/MySQL), where a column-level `COLLATE` is a clean parse error. The
    /// collation name is boxed to keep this cold clause from widening the warm `ColumnOption`
    /// enum (the box/inline budget): every other fat variant here boxes its payload, so
    /// `Collate` matches them and the common unit constraints pay only a null pointer.
    Collate {
        /// The collation name.
        collation: Box<ObjectName>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An inline `CHECK (<predicate>)` constraint.
    Check {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// The `NO INHERIT` marker (PostgreSQL/DuckDB): the check is not inherited by child
        /// tables (`a int CHECK (a > 0) NO INHERIT`). At column level PostgreSQL bakes
        /// `opt_no_inherit` directly into the `CHECK` production (unlike the table-level
        /// [`TableConstraintDef`] marker, which rides the shared constraint-attribute slot), and
        /// no column-level `NOT VALID` exists. Gated for acceptance by
        /// [`ConstraintSyntax::constraint_no_inherit_not_valid`](crate::dialect::ConstraintSyntax::constraint_no_inherit_not_valid);
        /// off elsewhere, where `NO INHERIT` after a column `CHECK` is a clean parse error.
        no_inherit: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An inline `REFERENCES <table> (<cols>)` foreign-key constraint.
    References {
        // Boxed to keep `ColumnOption` lean: a `ForeignKeyRef` (48 B) is the only
        // fat payload among these variants (every other is boxed or a unit), so
        // inlining it set the whole enum's width. `REFERENCES` is a cold DDL path, so
        // one allocation is a cheap price for the per-node saving — the fat-variant
        // skew ADR-0007 calls out, data-backed by the box/inline budget benchmark.
        /// The foreign-key target table, columns, and options; see [`ForeignKeyRef`].
        reference: Box<ForeignKeyRef>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A trailing bodyless `CONSTRAINT <name>` (SQLite): the [`ColumnConstraint::name`] is
    /// written but no constraint element follows it before the column/table-element
    /// terminator. Gated for acceptance by
    /// [`ConstraintSyntax::bare_constraint_name`](crate::dialect::ConstraintSyntax::bare_constraint_name); off
    /// elsewhere, where a `CONSTRAINT <name>` with nothing following is a clean parse error.
    Bare {
        /// Source location and node identity.
        meta: Meta,
    },
    /// Dialect extension node supplied by the extension type.
    Other {
        /// The dialect extension node value.
        ext: X,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Surface spelling for the auto-increment column attribute ([`ColumnOption::AutoIncrement`]).
///
/// The same canonical attribute is written `AUTO_INCREMENT` (MySQL, an underscore) or
/// `AUTOINCREMENT` (SQLite, one solid word); this tag records which the source used so
/// rendering round-trips exactly, mirroring the [`GeneratedColumnSpelling`] tag. A leaf
/// `Copy` enum whose span lives on the owning [`ColumnOption`], like [`DropBehavior`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AutoIncrementSpelling {
    /// The MySQL `AUTO_INCREMENT` spelling (with the underscore).
    Underscored,
    /// The SQLite `AUTOINCREMENT` spelling (one solid word).
    Joined,
}

use super::ConflictResolution;

/// A foreign-key reference: `REFERENCES <table> [(<column>, ...)]` plus the
/// optional `<referential triggered action>` clauses.
///
/// `match_type`, `on_delete`, and `on_update` stay `None` when their clause is
/// unwritten — the canonical shape keeps one node for both the column-level
/// `REFERENCES` form and the table-level `FOREIGN KEY` form. The clauses
/// are order-independent in the standard, so the parser accepts them in any order
/// and renders them in a fixed canonical order (`ON DELETE` before `ON UPDATE`); the
/// [`update_before_delete`](Self::update_before_delete) tag records the one bit of
/// order the two separate fields otherwise lose, so a source-fidelity render replays
/// the written order. PostgreSQL always materializes these (defaulting `MATCH SIMPLE`
/// / `NO ACTION`), so an unwritten clause and the explicit default are
/// representation-equivalent.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ForeignKeyRef {
    /// Table referenced by this syntax.
    pub table: ObjectName,
    /// Columns in source order.
    pub columns: ThinVec<Ident>,
    /// Optional match type for this syntax.
    pub match_type: Option<ForeignKeyMatch>,
    /// The referential actions are boxed because they are usually absent and a
    /// [`ReferentialAction`] is comparatively large: a bare `REFERENCES` then pays
    /// only a null pointer, keeping the inline node small where this type is
    /// embedded in [`ColumnOption`]/[`TableConstraint`] (the same reason those
    /// enums box their generated-column and identity payloads).
    pub on_delete: Option<Box<ReferentialAction>>,
    /// Optional on update for this syntax.
    pub on_update: Option<Box<ReferentialAction>>,
    /// Whether the source wrote `ON UPDATE` before `ON DELETE`. Meaningful only when
    /// both actions are present; the canonical render emits `ON DELETE` first, and a
    /// source-fidelity render swaps to the written order. Exact-order fidelity — the
    /// clauses are order-independent, so a target re-spell and the redacted fingerprint
    /// keep the canonical order.
    pub update_before_delete: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `MATCH` mode of a foreign key, governing how a composite key containing
/// NULLs matches. A bare `REFERENCES` omits it; PostgreSQL reports the omitted
/// form as `SIMPLE`. A leaf vocabulary enum (no `meta`): like [`DropBehavior`] it
/// holds no spanned children, so its span lives on the owning [`ForeignKeyRef`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ForeignKeyMatch {
    /// `MATCH FULL` — a partially-null referencing key is rejected.
    Full,
    /// `MATCH PARTIAL` — reserved by the standard; unimplemented by most engines.
    Partial,
    /// `MATCH SIMPLE` — the default; a null in any referencing column skips the check.
    Simple,
}

/// A foreign-key `<referential action>`: what a delete (`ON DELETE`) or update
/// (`ON UPDATE`) of the referenced row does to the referencing rows.
///
/// `SetNull`/`SetDefault` carry the optional PostgreSQL column list (`SET NULL
/// (col, ...)`), which restricts the action to a subset of the referencing
/// columns; the grammar accepts the list only on `ON DELETE`, so it is always
/// empty for an `ON UPDATE` action and for the bare keyword forms. Each variant
/// carries `meta` because the action is a span-bearing node — the `SET NULL (col)`
/// form references spanned [`Ident`]s — mirroring [`IdentityOption`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ReferentialAction {
    /// `NO ACTION` — reject the change if references remain (deferrable).
    NoAction {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RESTRICT` — reject the change immediately if references remain.
    Restrict {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CASCADE` — propagate the delete/update to the referencing rows.
    Cascade {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET NULL` — null out the referencing columns (optionally a subset).
    SetNull {
        /// Columns in source order.
        columns: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET DEFAULT` — reset the referencing columns to their defaults.
    SetDefault {
        /// Columns in source order.
        columns: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL generated column.
pub struct GeneratedColumn<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Expr<X>,
    /// Optional storage for this syntax.
    pub storage: Option<GeneratedColumnStorage>,
    /// Which surface spelled the generation clause: the standard `GENERATED ALWAYS AS`
    /// or the MySQL/SQLite keywordless `AS` shorthand (see [`GeneratedColumnSpelling`]).
    pub spelling: GeneratedColumnSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL generated column storage forms represented by the AST.
pub enum GeneratedColumnStorage {
    /// `STORED` — the generated value is materialized on write.
    Stored,
    /// `VIRTUAL` — the generated value is computed on read.
    Virtual,
}

/// Surface spelling for a [`GeneratedColumn`]'s generation clause.
///
/// A stored/virtual generated column is written two interchangeable-shape ways: the
/// standard `GENERATED ALWAYS AS (<expr>)` and the keywordless `AS (<expr>)` shorthand
/// (MySQL, SQLite). The canonical AST keeps one [`GeneratedColumn`] node and
/// this tag records which the source used so rendering round-trips exactly, mirroring
/// the [`LimitSyntax`](super::LimitSyntax) surface tag. The shorthand is gated for
/// acceptance by
/// [`ColumnDefinitionSyntax::generated_column_shorthand`](crate::dialect::ColumnDefinitionSyntax::generated_column_shorthand).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum GeneratedColumnSpelling {
    /// The standard `GENERATED ALWAYS AS (<expr>)` spelling.
    GeneratedAlways,
    /// The keywordless `AS (<expr>)` shorthand (MySQL, SQLite).
    Shorthand,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL identity column.
pub struct IdentityColumn<X: Extension = NoExt> {
    /// Whether the identity is `GENERATED ALWAYS` or `BY DEFAULT`; see [`IdentityGeneration`].
    pub generation: IdentityGeneration,
    /// Options supplied in source order.
    pub options: ThinVec<IdentityOption<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL identity generation forms represented by the AST.
pub enum IdentityGeneration {
    /// `GENERATED ALWAYS` — the identity value cannot be overridden on insert.
    Always,
    /// `GENERATED BY DEFAULT` — an explicit insert value overrides the sequence.
    ByDefault,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL identity option forms represented by the AST.
pub enum IdentityOption<X: Extension = NoExt> {
    /// `START WITH <n>` — the sequence's initial value.
    StartWith {
        /// Expression evaluated by this syntax.
        expr: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `INCREMENT BY <n>` — the step between successive values.
    IncrementBy {
        /// Expression evaluated by this syntax.
        expr: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `MINVALUE <n>` / `NO MINVALUE` — the sequence's lower bound.
    MinValue {
        /// Value supplied by this syntax.
        value: Option<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `MAXVALUE <n>` / `NO MAXVALUE` — the sequence's upper bound.
    MaxValue {
        /// Value supplied by this syntax.
        value: Option<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CACHE <n>` — how many values to preallocate per session.
    Cache {
        /// Expression evaluated by this syntax.
        expr: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CYCLE` / `NO CYCLE` — whether the sequence wraps around at its bound.
    Cycle {
        /// Whether cycling is enabled.
        cycle: bool,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL table constraint def.
pub struct TableConstraintDef<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Option<Ident>,
    /// The constraint kind and its data; see [`TableConstraint`].
    pub constraint: TableConstraint<X>,
    /// The `NO INHERIT` marker (PostgreSQL/DuckDB): the constraint is not inherited by child
    /// tables. It rides PostgreSQL's shared `ConstraintAttributeSpec` slot alongside
    /// [`characteristics`](Self::characteristics), order-free with `NOT VALID` and
    /// `DEFERRABLE`. PostgreSQL admits it only on `CHECK` constraints (`FOREIGN KEY` /
    /// `PRIMARY KEY` / `UNIQUE` / `EXCLUDE` reject it in the grammar action — reproduced at
    /// parse), so a set flag always pairs with a [`TableConstraint::Check`]. Gated for
    /// acceptance by
    /// [`ConstraintSyntax::constraint_no_inherit_not_valid`](crate::dialect::ConstraintSyntax::constraint_no_inherit_not_valid).
    pub no_inherit: bool,
    /// The `NOT VALID` marker (PostgreSQL/DuckDB): the constraint is recorded but existing rows
    /// are not checked. Shares the `ConstraintAttributeSpec` slot with
    /// [`no_inherit`](Self::no_inherit) and [`characteristics`](Self::characteristics).
    /// PostgreSQL admits it only on `CHECK` and `FOREIGN KEY` constraints (`PRIMARY KEY` /
    /// `UNIQUE` / `EXCLUDE` reject it — reproduced at parse), and there is no column-level
    /// form. Gated for acceptance by the same
    /// [`ConstraintSyntax::constraint_no_inherit_not_valid`](crate::dialect::ConstraintSyntax::constraint_no_inherit_not_valid)
    /// flag as [`no_inherit`](Self::no_inherit).
    pub not_valid: bool,
    /// The trailing `DEFERRABLE`/`INITIALLY` characteristics, when written; boxed as
    /// the cold clause it is (see [`ColumnConstraint::characteristics`]).
    pub characteristics: Option<Box<ConstraintCharacteristics>>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL table constraint forms represented by the AST.
pub enum TableConstraint<X: Extension = NoExt> {
    /// A `PRIMARY KEY (col, …)` table constraint.
    PrimaryKey {
        /// The key columns. Each is an [`IndexColumn`] — the same "indexed-column" shape
        /// [`CreateIndex`] uses — so a column may carry a `COLLATE <collation>` postfix
        /// (folded into [`IndexColumn::expr`] as an [`Expr::Collate`](super::Expr)) and an
        /// `ASC`/`DESC` sort order. The bare-column case is an
        /// [`Expr::Column`](super::Expr) with `asc`/`nulls_first` unset — the shape every
        /// dialect produces. The COLLATE/ordering decoration is gated for *acceptance* by
        /// [`ConstraintSyntax::constraint_column_collate_order`](crate::dialect::ConstraintSyntax::constraint_column_collate_order)
        /// (SQLite): off elsewhere, where a bare name is the only accepted form and a
        /// trailing `COLLATE`/`ASC`/`DESC` is a clean parse error. SQLite prohibits general
        /// expressions and `NULLS FIRST`/`LAST` in this position, so the parser never fills
        /// `nulls_first` nor produces a non-column/non-collate `expr` here.
        columns: ThinVec<IndexColumn<X>>,
        /// The `INCLUDE (<col>, ...)` covering-index columns (PostgreSQL): non-key payload
        /// columns stored in the primary key's implicit index (`PRIMARY KEY (a) INCLUDE (b)`).
        /// Empty when no `INCLUDE` clause is written. Gated for acceptance by
        /// [`ConstraintSyntax::index_constraint_parameters`](crate::dialect::ConstraintSyntax::index_constraint_parameters);
        /// off elsewhere, where `INCLUDE` after the key list is a clean parse error.
        include: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `UNIQUE (col, …)` table constraint.
    Unique {
        /// The key columns, as on [`PrimaryKey`](Self::PrimaryKey::columns): [`IndexColumn`]s
        /// admitting a per-column `COLLATE <collation>` and `ASC`/`DESC` under the same
        /// [`ConstraintSyntax::constraint_column_collate_order`](crate::dialect::ConstraintSyntax::constraint_column_collate_order)
        /// gate.
        columns: ThinVec<IndexColumn<X>>,
        /// The `NULLS [NOT] DISTINCT` null-treatment (PostgreSQL 15+), as on the column-level
        /// [`ColumnOption::Unique`]. `Some(false)` = `NULLS NOT DISTINCT`, `Some(true)` = the
        /// explicit `NULLS DISTINCT` default, `None` = unwritten. Gated for acceptance by
        /// [`ConstraintSyntax::index_constraint_parameters`](crate::dialect::ConstraintSyntax::index_constraint_parameters).
        nulls_not_distinct: Option<bool>,
        /// The `INCLUDE (<col>, ...)` covering-index columns (PostgreSQL), as on
        /// [`PrimaryKey`](Self::PrimaryKey::include). Empty when unwritten. Same gate.
        include: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `CHECK (<predicate>)` table constraint.
    Check {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL `EXCLUDE [USING <method>] (<element> WITH <operator> [, ...]) [INCLUDE
    /// (...)] [WITH (...)] [USING INDEX TABLESPACE ...] [WHERE (...)]` exclusion constraint —
    /// no two rows may hold values that all pairwise satisfy the named operators. The whole
    /// clause is boxed into [`ExcludeConstraint`] so this cold, rarely-written variant does not
    /// widen the warm `TableConstraint` enum (the box/inline budget — the same reason
    /// [`ForeignKey`](Self::ForeignKey) boxes its `ForeignKeyRef`). Gated for acceptance by
    /// [`ConstraintSyntax::exclusion_constraints`](crate::dialect::ConstraintSyntax::exclusion_constraints); off
    /// elsewhere (DuckDB included, which rejects it), where `EXCLUDE` at a constraint position is
    /// a clean parse error.
    Exclude {
        /// The exclusion-constraint specification; see [`ExcludeConstraint`].
        exclude: Box<ExcludeConstraint<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `FOREIGN KEY (col, …) REFERENCES …` table constraint.
    ForeignKey {
        /// Columns in source order.
        columns: ThinVec<Ident>,
        // Boxed for the same fat-variant reason as `ColumnOption::References`: the
        // inline `ForeignKeyRef` (48 B) alone set `TableConstraint`'s width — and
        // transitively that of the `TableConstraintDef` containing it. Cold DDL path,
        // so the lone allocation is cheap (ADR-0007, box/inline budget benchmark).
        /// The foreign-key target table, columns, and options; see [`ForeignKeyRef`].
        references: Box<ForeignKeyRef>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A trailing bodyless `CONSTRAINT <name>` (SQLite), standalone in the table-element list
    /// (`CREATE TABLE t (a INT, CONSTRAINT cn)`) — the [`TableConstraintDef::name`] is written
    /// but no constraint element follows it. SQLite also lets the comma separating this from a
    /// preceding table constraint be omitted (`UNIQUE(a) CONSTRAINT cn`, engine-measured
    /// accepted); the parser's table-element loop handles that comma elision, not this variant.
    /// Gated for acceptance by
    /// [`ConstraintSyntax::bare_constraint_name`](crate::dialect::ConstraintSyntax::bare_constraint_name); off
    /// elsewhere, where a `CONSTRAINT <name>` with nothing following is a clean parse error.
    Bare {
        /// Source location and node identity.
        meta: Meta,
    },
    /// Dialect extension node supplied by the extension type.
    Other {
        /// The dialect extension node value.
        ext: X,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The body of a PostgreSQL [`EXCLUDE`](TableConstraint::Exclude) exclusion constraint.
///
/// `EXCLUDE [USING <method>] (<element> [, ...]) [INCLUDE (<col>, ...)] [WITH (<param>, ...)]
/// [USING INDEX TABLESPACE <name>] [WHERE (<predicate>)]` — the tail clauses appear in that
/// fixed PostgreSQL grammar order. Boxed inside [`TableConstraint::Exclude`] so the cold clause
/// does not widen the warm constraint enum.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ExcludeConstraint<X: Extension = NoExt> {
    /// The `USING <method>` index access method (`gist`, `btree`, ...); `None` for the bare
    /// `EXCLUDE (...)` form, which PostgreSQL defaults to `btree`. A single identifier — a
    /// qualified method is a raw-parse error.
    pub method: Option<Ident>,
    /// The exclusion elements, each an index element paired with an operator (`c WITH &&`);
    /// non-empty (an empty `()` is a raw-parse error).
    pub elements: ThinVec<ExcludeElement<X>>,
    /// include in source order.
    pub include: ThinVec<Ident>,
    /// The `WITH (<param> [= <value>], ...)` index storage parameters (reloptions); empty when
    /// unwritten. Reuses [`TableStorageParameter`], the same element the `CREATE TABLE … WITH
    /// (…)` option list holds.
    pub with_params: ThinVec<TableStorageParameter<X>>,
    /// Optional index tablespace for this syntax.
    pub index_tablespace: Option<Ident>,
    /// The `WHERE (<predicate>)` partial-constraint predicate; `None` when unwritten. Boxed as a
    /// cold clause so an exclusion constraint without a predicate pays only a null pointer.
    pub predicate: Option<Box<Expr<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `<index_element> WITH <operator>` element of an [`ExcludeConstraint`] (PostgreSQL
/// `ExclusionConstraintElem`).
///
/// The index element is PostgreSQL's `index_elem`: a key (a bare column, a bare function call,
/// or a parenthesized `(a_expr)` — the [`parenthesized`](Self::parenthesized) flag records which
/// so the source form round-trips), then optional `COLLATE`, an operator-class name with
/// optional reloptions, and `ASC`/`DESC` + `NULLS FIRST`/`LAST` sort modifiers — exactly the
/// [`CreateIndex`] key grammar. The `WITH <operator>` names the exclusion operator.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ExcludeElement<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Expr<X>,
    /// Whether the key was written parenthesized (`(a + b)`); a bare column / function call is
    /// unparenthesized. Mirrors [`PartitionElem::parenthesized`].
    pub parenthesized: bool,
    /// Optional collation for this syntax.
    pub collation: Option<ObjectName>,
    /// The operator-class name (`index_elem` `opt_qualified_name`), when written — `a int4_ops`,
    /// `a myschema.text_ops`.
    pub opclass: Option<ObjectName>,
    /// The operator-class reloptions `(<param> [= <value>], ...)` (`index_elem` `reloptions`),
    /// present only with an [`opclass`](Self::opclass); empty otherwise.
    pub opclass_params: ThinVec<TableStorageParameter<X>>,
    /// `ASC` (`Some(true)`) / `DESC` (`Some(false)`) sort order; `None` when unwritten.
    pub asc: Option<bool>,
    /// `NULLS FIRST` (`Some(true)`) / `NULLS LAST` (`Some(false)`); `None` when unwritten.
    pub nulls_first: Option<bool>,
    /// Operator applied by this expression.
    pub operator: ExcludeOperator,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `WITH <operator>` operator of an [`ExcludeElement`] — PostgreSQL `any_operator`, a bare
/// symbolic operator (`&&`, `=`, `-|-`) or the explicit `OPERATOR(<schema>.<op>)` keyword form.
///
/// Models the operator the same way [`NamedOperatorExpr`](super::NamedOperatorExpr) does — an
/// optional schema qualification, the interned symbolic [`op`](Self::op), and a
/// [`spelling`](Self::spelling) tag recording the bare-vs-`OPERATOR(...)` surface — so the source
/// form round-trips. A leaf node without `meta`: its span is subsumed by the owning
/// [`ExcludeElement`], like [`DropBehavior`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ExcludeOperator {
    /// The optional schema qualification (`pg_catalog` in `OPERATOR(pg_catalog.=)`); an empty
    /// name for a bare or unqualified operator.
    pub schema: ObjectName,
    /// The symbolic operator spelling (`&&`, `=`, `-|-`), interned exact-case.
    pub op: Symbol,
    /// Which surface spelled the operator — a bare `&&` or the explicit `OPERATOR(...)` keyword.
    pub spelling: NamedOperatorSpelling,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL create table option.
pub struct CreateTableOption<X: Extension = NoExt> {
    /// Which option kind (`WITH`/`ON COMMIT`/`TABLESPACE`/…); see [`CreateTableOptionKind`].
    pub kind: CreateTableOptionKind<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL create table option kind forms represented by the AST.
pub enum CreateTableOptionKind<X: Extension = NoExt> {
    /// A `WITH (<param> = <value>, …)` storage-parameter clause.
    With {
        /// params in source order.
        params: ThinVec<TableStorageParameter<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An `ON COMMIT {PRESERVE ROWS | DELETE ROWS | DROP}` temporary-table clause.
    OnCommit {
        /// The on-commit action; see [`OnCommitAction`].
        action: OnCommitAction,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `TABLESPACE <name>` clause.
    Tablespace {
        /// The tablespace name.
        tablespace: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL trailing table option written as an open `<name> [=] <value>` pair —
    /// `ENGINE = InnoDB`, `AUTO_INCREMENT = 100`, `DEFAULT CHARSET = utf8mb4`,
    /// `COMMENT = '...'`, `ROW_FORMAT = DYNAMIC`, `COLLATE = utf8mb4_general_ci`.
    ///
    /// Modelled as one canonical name/value pair rather than a variant (or a
    /// `CreateTable` field) per option keyword: the MySQL option
    /// vocabulary is large and server-version-dependent, so a single open shape
    /// round-trips an arbitrary option list — the same reasoning the [`CopyOption`]
    /// list follows. The optional `DEFAULT` noise word MySQL accepts before
    /// `CHARSET`/`COLLATE` is normalized away by the parser, and the `=` is optional.
    /// Gated by `create_table_clause_syntax.table_options`.
    ///
    /// [`CopyOption`]: super::CopyOption
    KeyValue {
        // The whole `<name> = <value>` payload is boxed (ADR-0007): a `TableOption`
        // (name + a 40 B value + meta) is the only fat payload among these variants, so
        // inlining it would set the enum's width and tax the lean `With`/`OnCommit`/
        // `Tablespace` variants — and, transitively, every `CreateTableOption` an
        // ANSI/PostgreSQL parse stores in its options `ThinVec`. Table options are a
        // cold DDL path, so the lone allocation per MySQL option is cheap, the same
        // box call `ColumnOption::Identity` makes for its `IdentityColumn`.
        /// The `<name> = <value>` option pair; see [`TableOption`].
        option: Box<TableOption>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The SQLite `WITHOUT ROWID` trailing table option: the table is stored as a
    /// clustered index on its primary key rather than the default implicit `rowid`.
    /// A bare keyword-style option (no `= value`), so a variant of its own rather than
    /// the MySQL [`KeyValue`](Self::KeyValue) name/value catch-all. SQLite
    /// comma-separates these trailing options (`... STRICT, WITHOUT ROWID`). Gated for
    /// acceptance by
    /// [`CreateTableClauseSyntax::without_rowid_table_option`](crate::dialect::CreateTableClauseSyntax::without_rowid_table_option).
    WithoutRowid {
        /// Source location and node identity.
        meta: Meta,
    },
    /// The SQLite `STRICT` trailing table option: the table enforces its declared
    /// column types instead of SQLite's default flexible typing. Like
    /// [`WithoutRowid`](Self::WithoutRowid) a bare keyword-style option, gated for
    /// acceptance by
    /// [`CreateTableClauseSyntax::strict_table_option`](crate::dialect::CreateTableClauseSyntax::strict_table_option).
    Strict {
        /// Source location and node identity.
        meta: Meta,
    },
    /// The legacy PostgreSQL `WITHOUT OIDS` trailing option: historically suppressed the
    /// system `oid` column. Modern PostgreSQL keeps it in the grammar as an accepted no-op,
    /// while the affirmative `WITH OIDS` has no production and *rejects* (both
    /// engine-measured). A bare keyword-style option, so a
    /// variant of its own rather than the MySQL [`KeyValue`](Self::KeyValue) catch-all. It
    /// occupies PostgreSQL's `OptWith` slot — mutually exclusive with a `WITH (…)` storage-
    /// parameter list, and it precedes `ON COMMIT` / `TABLESPACE`. Gated for acceptance by
    /// [`CreateTableClauseSyntax::without_oids`](crate::dialect::CreateTableClauseSyntax::without_oids).
    WithoutOids {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One MySQL trailing table option: a `<name> = <value>` pair.
///
/// The canonical "options as data" shape: `name` is the option keyword as
/// written (`ENGINE`, `AUTO_INCREMENT`, `CHARSET`, ...) and `value` is its argument.
/// Boxed inside [`CreateTableOptionKind::KeyValue`] to keep that enum lean; mirrors
/// the [`CopyOption`](super::CopyOption) list element.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct TableOption {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Value supplied by this syntax.
    pub value: TableOptionValue,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The value of a [`TableOption`].
///
/// Three surface forms a downstream converter cannot otherwise recover:
/// a bareword/keyword (`InnoDB`, `DYNAMIC`), a string (`COMMENT = '...'`), or a
/// number (`AUTO_INCREMENT = 100`). String and numeric values ride their
/// [`Literal`]'s `meta.span` and materialise lazily; a bareword keeps its
/// source spelling on the [`Ident`]. Mirrors [`CopyOptionValue`](super::CopyOptionValue),
/// which omits the numeric form COPY options never take.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TableOptionValue {
    /// A bareword/keyword value: `ENGINE = InnoDB`, `ROW_FORMAT = DYNAMIC`.
    Word {
        /// Identifier-form value.
        word: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A string value: `COMMENT = 'text'`.
    String {
        /// Value supplied by this syntax.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A numeric value: `AUTO_INCREMENT = 100`, `KEY_BLOCK_SIZE = 8`.
    Number {
        /// Value supplied by this syntax.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL on commit action forms represented by the AST.
pub enum OnCommitAction {
    /// `ON COMMIT PRESERVE ROWS` — keep the temp table's rows after commit.
    PreserveRows,
    /// `ON COMMIT DELETE ROWS` — empty the temp table on each commit.
    DeleteRows,
    /// `ON COMMIT DROP` — drop the temp table at the end of the transaction.
    Drop,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL table storage parameter.
pub struct TableStorageParameter<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Value supplied by this syntax.
    pub value: Option<Expr<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// `ALTER TABLE [IF EXISTS] <name> <action> [, <action>]...`.
///
/// The schema-evolution counterpart of [`CreateTable`]. A single statement carries
/// one or more comma-separated [`AlterTableAction`]s, matching PostgreSQL's
/// multi-action form (`ALTER TABLE t ADD COLUMN a INT, DROP COLUMN b`). The `IF
/// EXISTS` prefix is gated by `existence_guards.if_exists` dialect data:
/// off under ANSI, on under PostgreSQL. The PostgreSQL-only `ONLY`
/// inheritance qualifier is a deliberate follow-up, tracked separately.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterTable<X: Extension = NoExt> {
    /// Whether the if exists form was present in the source.
    pub if_exists: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// actions in source order.
    pub actions: ThinVec<AlterTableAction<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A column target inside an `ALTER TABLE` action.
///
/// Most dialects name a top-level column (`c`), while DuckDB also admits dotted paths into
/// nested STRUCT fields for selected actions (`s.s2.k`). Kept separate from [`ObjectName`]
/// so ALTER-specific nested paths do not imply that table columns generally use relation-name
/// semantics.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterColumnTarget {
    /// parts in source order.
    pub parts: ThinVec<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One action in an `ALTER TABLE` statement.
///
/// `AddColumn`/`AddConstraint` reuse the `CREATE TABLE` element nodes ([`ColumnDef`]
/// and [`TableConstraintDef`]) rather than inventing alter-specific copies, so the
/// `Other(X)` extension seams already on [`ColumnOption`]/[`TableConstraint`] cover
/// custom DDL attached through an alter too. The optional `COLUMN` noise
/// word is not represented: rendering normalizes to the canonical `ADD COLUMN`
/// spelling.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AlterTableAction<X: Extension = NoExt> {
    /// `ADD [COLUMN] <col> <type> …` — add a column to the table.
    AddColumn {
        /// Whether the if not exists form was present in the source.
        if_not_exists: bool,
        /// Whether the optional `COLUMN` noise word was written (`ADD COLUMN c` vs the
        /// bare `ADD c`). Exact-synonym fidelity; the canonical render emits `COLUMN`.
        column_keyword: bool,
        /// Object targeted by this syntax.
        target: Option<AlterColumnTarget>,
        /// Column referenced by this syntax.
        column: ColumnDef<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DROP [COLUMN] <name>` — remove a column from the table.
    DropColumn {
        /// Whether the if exists form was present in the source.
        if_exists: bool,
        /// Whether the optional `COLUMN` noise word was written (`DROP COLUMN c` vs the
        /// bare `DROP c`). Exact-synonym fidelity; the canonical render emits `COLUMN`.
        column_keyword: bool,
        /// Name referenced by this syntax.
        name: AlterColumnTarget,
        /// Optional behavior for this syntax.
        behavior: Option<DropBehavior>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ALTER [COLUMN] <name> …` — change an existing column's definition.
    AlterColumn {
        /// Whether the optional `COLUMN` noise word was written (`ALTER COLUMN c` vs the
        /// bare `ALTER c`). Exact-synonym fidelity; the canonical render emits `COLUMN`.
        column_keyword: bool,
        /// Name referenced by this syntax.
        name: Ident,
        /// The per-column change; see [`AlterColumnAction`].
        change: AlterColumnAction<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ADD CONSTRAINT …` — add a table constraint.
    AddConstraint {
        /// The constraint being added; see [`TableConstraintDef`].
        constraint: TableConstraintDef<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DROP CONSTRAINT <name>` — remove a table constraint.
    DropConstraint {
        /// Whether the if exists form was present in the source.
        if_exists: bool,
        /// Name referenced by this syntax.
        name: Ident,
        /// Optional behavior for this syntax.
        behavior: Option<DropBehavior>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RENAME [COLUMN] <name> TO <new_name>`.
    ///
    /// PostgreSQL parses the rename forms as a distinct `RenameStmt`, not the
    /// `AlterTableStmt` the other actions take; the canonical AST keeps one
    /// [`AlterTable`] node for every `ALTER TABLE` spelling, so the rename
    /// is a further action variant here. The optional `COLUMN` noise word is
    /// recorded, mirroring [`AddColumn`](Self::AddColumn).
    RenameColumn {
        /// Whether the optional `COLUMN` noise word was written (`RENAME COLUMN c` vs
        /// the bare `RENAME c`). Exact-synonym fidelity; the canonical render emits
        /// `COLUMN`.
        column_keyword: bool,
        /// Name referenced by this syntax.
        name: AlterColumnTarget,
        /// The column's new name.
        new_name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RENAME TO <new_name>` — rename the table.
    RenameTable {
        /// The table's new name.
        new_name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ATTACH PARTITION <name> <bound>` — attach an existing table as a partition
    /// (PostgreSQL declarative partitioning). A *standalone* action: PostgreSQL parses
    /// `ATTACH`/`DETACH PARTITION` as their own `AlterTableStmt` productions, so they never
    /// combine with other actions in a comma list (`ALTER TABLE p ADD COLUMN x, ATTACH …` is a
    /// syntax error). Gated by
    /// [`CreateTableClauseSyntax::declarative_partitioning`](crate::dialect::CreateTableClauseSyntax::declarative_partitioning).
    AttachPartition {
        /// The partition (child) table name.
        partition: ObjectName,
        // Boxed as the fat child (matching the `PartitionOf` body): a `PartitionBound` is the
        // widest payload here, so a partition attach pays one cold allocation rather than setting
        // `AlterTableAction`'s width (ADR-0007, box/inline budget).
        /// The partition bound (`FOR VALUES …` / `DEFAULT`); see [`PartitionBound`].
        bound: Box<PartitionBound<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DETACH PARTITION <name> [CONCURRENTLY | FINALIZE]` — detach a partition (PostgreSQL).
    /// Like [`AttachPartition`](Self::AttachPartition) a standalone action. The optional
    /// [`mode`](Self::DetachPartition::mode) records the `CONCURRENTLY` / `FINALIZE`
    /// qualifier; `None` for the bare form.
    DetachPartition {
        /// The partition (child) table name.
        partition: ObjectName,
        /// Mode selected by this syntax.
        mode: Option<DetachPartitionMode>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The qualifier on a `DETACH PARTITION` action. A `Copy` leaf enum whose span rides the owning
/// [`AlterTableAction::DetachPartition`], like [`DropBehavior`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DetachPartitionMode {
    /// `CONCURRENTLY` — detach without a long-held lock (PostgreSQL).
    Concurrently,
    /// `FINALIZE` — complete an interrupted concurrent detach.
    Finalize,
}

/// A single-column alteration inside `ALTER TABLE ... ALTER COLUMN <name> ...`.
///
/// The ANSI `SET DATA TYPE` and PostgreSQL `TYPE` spellings map to one canonical
/// [`SetDataType`](Self::SetDataType) variant; the optional PostgreSQL
/// `USING <expr>` conversion expression rides along when present.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AlterColumnAction<X: Extension = NoExt> {
    /// `SET DEFAULT <expr>` — set the column's default value.
    SetDefault {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DROP DEFAULT` — remove the column's default value.
    DropDefault {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET NOT NULL` — add a not-null constraint to the column.
    SetNotNull {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DROP NOT NULL` — remove the column's not-null constraint.
    DropNotNull {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET DATA TYPE <t>` / `TYPE <t> [USING …]` — change the column's data type.
    SetDataType {
        /// Whether the ANSI `SET DATA` prefix was written (`SET DATA TYPE <t>`) versus
        /// the bare PostgreSQL `TYPE <t>`. Exact-synonym fidelity; the canonical render
        /// emits `SET DATA TYPE`.
        set_data: bool,
        /// Data type named by this syntax.
        data_type: DataType<X>,
        /// Optional using for this syntax.
        using: Option<Box<Expr<X>>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// `DROP {TABLE | VIEW | INDEX | SCHEMA} [IF EXISTS] <name> [, ...] [CASCADE | RESTRICT]`.
///
/// Non-generic: a drop names objects and a behaviour only — it carries no
/// expressions or extension nodes — so it parallels the other leaf statement
/// payloads ([`super::TransactionStatement`], [`super::SessionStatement`]). The `IF
/// EXISTS` prefix and the `CASCADE`/`RESTRICT` drop behaviour are each gated by
/// `schema_change_syntax` dialect data. Materialized-view and concurrent-index drop
/// surfaces are not modelled here.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DropStatement {
    /// Which object kind is dropped (`TABLE`/`VIEW`/…); see [`DropObjectKind`].
    pub object_kind: DropObjectKind,
    /// Whether the if exists form was present in the source.
    pub if_exists: bool,
    /// Names in source order.
    pub names: ThinVec<ObjectName>,
    /// Optional behavior for this syntax.
    pub behavior: Option<DropBehavior>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The kind of object a `DROP` statement removes.
///
/// The routine kinds (`FUNCTION`/`PROCEDURE`) take an argument-type signature, so
/// they are a separate [`Statement::DropRoutine`]
/// statement rather than a variant here;
/// this covers the object kinds a plain name list can drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DropObjectKind {
    /// `DROP TABLE`.
    Table,
    /// `DROP VIEW`.
    View,
    /// `DROP MATERIALIZED VIEW`.
    MaterializedView,
    /// `DROP INDEX`.
    Index,
    /// `DROP SCHEMA`.
    Schema,
    /// `DROP TYPE <name> [, …]` — a user-defined type (DuckDB/PostgreSQL), gated by
    /// [`StatementDdlGates::create_type`](crate::dialect::StatementDdlGates::create_type) (the same
    /// flag that admits [`CreateType`]). The comma list and `CASCADE`/`RESTRICT` behaviour
    /// ride the shared [`DropStatement`] grammar; DuckDB parse-accepts a multi-name list and
    /// only rejects it at plan time ("can only drop one object at a time"), so the shape is
    /// the general one.
    Type,
    /// `DROP SEQUENCE <name> [, …]` — a sequence generator (DuckDB/PostgreSQL), gated by
    /// [`StatementDdlGates::create_sequence`](crate::dialect::StatementDdlGates::create_sequence) (the same
    /// flag that admits [`CreateSequence`]). The comma list and `CASCADE`/`RESTRICT` ride the
    /// shared [`DropStatement`] grammar. DuckDB's parser accepts `DROP SEQUENCE` (the
    /// `IF EXISTS` form binds; the bare/list/`CASCADE` forms parse but bind-reject a missing
    /// object — engine-measured), so the modelled shape is the general one.
    Sequence,
    /// `DROP MACRO <name> [, …]` — a scalar macro (DuckDB), gated by
    /// [`StatementDdlGates::create_macro`](crate::dialect::StatementDdlGates::create_macro) (the same
    /// flag that admits [`CreateMacro`]). Unlike PostgreSQL's `DROP FUNCTION`, the macro drop
    /// takes *no* argument-type signature (`DROP MACRO m(int)` is a DuckDB syntax error), so it
    /// rides the shared [`DropStatement`] name-list grammar rather than the signature
    /// [`Statement::DropRoutine`] path — the `FUNCTION` spelling
    /// (which DuckDB accepts as a synonym) still routes to that routine drop, so the two DROP
    /// spellings land on distinct nodes and no [`MacroSpelling`] tag is needed here. The comma
    /// list and `CASCADE`/`RESTRICT` ride the shared grammar; DuckDB parse-accepts a multi-name
    /// list and only bind-rejects it ("can only drop one object at a time" — engine-measured).
    Macro,
    /// `DROP MACRO TABLE <name> [, …]` — a *table* macro (DuckDB), the drop counterpart of a
    /// `CREATE MACRO … AS TABLE`. Separate from [`Macro`](DropObjectKind::Macro) because DuckDB
    /// keeps scalar and table macros in distinct namespaces (`DROP MACRO m` drops a "Macro
    /// Function", `DROP MACRO TABLE m` a "Table Macro Function" — engine-measured) and the
    /// `TABLE` keyword must round-trip verbatim. Gated by the same
    /// [`StatementDdlGates::create_macro`](crate::dialect::StatementDdlGates::create_macro) flag;
    /// only `MACRO TABLE` (not `TABLE MACRO` or `FUNCTION TABLE`) is accepted — engine-measured.
    MacroTable,
    /// `DROP TRIGGER [IF EXISTS] [<schema> .] <name>` — a trigger (MySQL / SQLite), gated by
    /// either trigger-modelling flag (SQLite's
    /// [`StatementDdlGates::create_trigger`](crate::dialect::StatementDdlGates::create_trigger) or
    /// MySQL's
    /// [`StatementDdlGates::compound_statements`](crate::dialect::StatementDdlGates::compound_statements)).
    /// Both dialects spell the same name-only drop (no `ON <table>`, which is PostgreSQL's
    /// separate trigger-drop shape), so it rides the shared [`DropStatement`] name-list grammar.
    Trigger,
}

/// The dependency behaviour of a `DROP` or `ALTER TABLE ... DROP`: whether dependent
/// objects are dropped too (`CASCADE`) or block the drop (`RESTRICT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DropBehavior {
    /// `CASCADE` — also drop objects that depend on the target.
    Cascade,
    /// `RESTRICT` — refuse the drop if any object depends on the target.
    Restrict,
}

/// A PostgreSQL `DROP TRANSFORM [IF EXISTS] FOR <type> LANGUAGE <lang>
/// [CASCADE | RESTRICT]` statement (`DropTransformStmt`, gram.y).
///
/// Kept apart from [`DropStatement`] because a transform is not named by a plain
/// name list: it is identified by the `(type, language)` pair a `CREATE TRANSFORM`
/// registers, which is exactly the [`ObjectReference::Transform`] shape the shared
/// object-reference axis already models (also reached by `ALTER EXTENSION … ADD|DROP
/// TRANSFORM`). Reusing that variant keeps every transform reference on one node, so
/// an AST walk finds transforms uniformly regardless of the parent statement, rather
/// than re-deriving the `FOR type LANGUAGE lang` shape here (the reason [`DropRoutine`]
/// inlines its signature does not apply — a transform has a single fixed shape, not a
/// per-kind one).
///
/// PostgreSQL admits exactly one transform per statement (no comma list — engine-measured)
/// and places `IF EXISTS` between the `TRANSFORM` keyword and `FOR` (`DROP TRANSFORM IF
/// EXISTS FOR …`; `FOR type IF EXISTS LANGUAGE` is a syntax error), which is why the guard
/// rides on this node rather than on the shared reference render.
///
/// [`DropRoutine`]: super::Statement::DropRoutine
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DropTransform<X: Extension = NoExt> {
    /// The dropped transform, always an [`ObjectReference::Transform`] (`FOR <type>
    /// LANGUAGE <lang>`).
    pub object: ObjectReference<X>,
    /// Whether the `IF EXISTS` guard was present in the source.
    pub if_exists: bool,
    /// Optional `CASCADE`/`RESTRICT` behaviour for this syntax.
    pub behavior: Option<DropBehavior>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The object kind a [`Statement::CommentOn`] targets.
///
/// PostgreSQL's `COMMENT ON` object list is large (tables, columns, functions, types,
/// operators, extensions, roles, …). This models only the deliberate subset the
/// datafusion-parity corpus needs — `TABLE`, `COLUMN`, `DATABASE`, and `PROCEDURE` —
/// and is `#[non_exhaustive]` so the remaining object kinds can be added later without
/// a breaking change. The object's name rides
/// [`Statement::CommentOn::name`](super::Statement::CommentOn); only a procedure adds
/// per-target data (its argument-type signature).
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CommentTarget<X: Extension = NoExt> {
    /// `COMMENT ON TABLE <name>`.
    Table,
    /// `COMMENT ON COLUMN <name>`.
    Column,
    /// `COMMENT ON DATABASE <name>`.
    Database,
    /// `COMMENT ON PROCEDURE <name>[(<arg types>)]`. The argument-type list
    /// disambiguates overloaded procedures; `None` is an unspecified signature
    /// (bare `PROCEDURE foo`), `Some` a written list — possibly empty (`foo()`) —
    /// mirroring [`RoutineSignature::arg_types`](super::RoutineSignature).
    Procedure {
        /// Optional arg types for this syntax.
        arg_types: Option<ThinVec<DataType<X>>>,
    },
}

/// The payload of a [`Statement::CommentOn`], boxed off the
/// statement enum to keep it within its size budget (like [`DropStatement`]).
///
/// `name` is the object's (possibly qualified) name; `target` records its kind (and, for
/// a procedure, its argument-type signature). `comment` is `None` for `IS NULL` — which
/// clears the object's comment — and `Some` for a string literal.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CommentOnStatement<X: Extension = NoExt> {
    /// Object targeted by this syntax.
    pub target: CommentTarget<X>,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Optional comment for this syntax.
    pub comment: Option<Literal>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// `CREATE SCHEMA [IF NOT EXISTS] [<name>] [AUTHORIZATION <role>] [<schema element> ...]`.
///
/// At least one of `name` / `authorization` is present — PostgreSQL's `CREATE SCHEMA
/// AUTHORIZATION joe` omits the schema name and derives it from the owning role.
///
/// The SQL-standard form embeds a list of component objects created *inside* the new
/// schema (`CREATE SCHEMA s CREATE TABLE t (...)`); [`elements`](Self::elements) holds
/// them as children so the whole construct stays ONE statement, mirroring PostgreSQL's
/// `CreateSchemaStmt.schemaElts` (which likewise carries the components as raw
/// statement parsenodes). Rendering them embedded preserves the statement COUNT that
/// downstream consumers observe — the earlier model split them into separate
/// `;`-joined statements, a statement-level rewrite no source-fidelity render can undo.
///
/// The element list is a *closed* admissible set enforced by the parser (measured
/// against PostgreSQL: `CREATE TABLE`/`VIEW`/`INDEX`/`SEQUENCE`/`TRIGGER` and `GRANT`
/// — and PostgreSQL rejects `CREATE MATERIALIZED VIEW`/`FUNCTION`, a nested `CREATE
/// SCHEMA`, `DROP`/`ALTER`/`INSERT`/… as elements). Modelled as a general
/// [`Statement`] list rather than a bespoke element enum — the same idiom as
/// [`CreateTrigger`]'s body — because the members ARE statements and reusing the
/// statement shape keeps the differential/structural oracles element-agnostic; the
/// closed set is the recorded acceptance bound, enforced at parse time. Non-empty only
/// under PostgreSQL (`schema_elements` gate); every other dialect parses a bare schema
/// head. Generic over `X` because the elements carry the extension node.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateSchema<X: Extension = NoExt> {
    /// Whether the if not exists form was present in the source.
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: Option<ObjectName>,
    /// Optional authorization for this syntax.
    pub authorization: Option<Ident>,
    /// The embedded schema-element statements, in source order. Empty for a bare
    /// `CREATE SCHEMA` head (the common case) and for every non-PostgreSQL dialect.
    pub elements: ThinVec<Statement<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// `CREATE [OR REPLACE] [TEMP|TEMPORARY] VIEW` and `CREATE MATERIALIZED VIEW`,
/// with the `AS <query>` body that every view shares (a view body reuses [`Query`]
/// rather than re-deriving the SELECT grammar).
///
/// One node covers both spellings, distinguished by `materialized`. The divergent
/// tails are mutually exclusive by construction: the parser fills `check_option`
/// only for a regular view and `with_data` only for a materialized one. `OR
/// REPLACE` and the `MATERIALIZED` keyword (with its `WITH [NO] DATA` populate
/// clause) are PostgreSQL extensions gated by `schema_change_syntax` dialect data;
/// the ANSI `WITH [CASCADED|LOCAL] CHECK OPTION` is always accepted.
///
/// `recursive` records the `CREATE [OR REPLACE] [TEMP|TEMPORARY] RECURSIVE VIEW`
/// spelling (DuckDB, gated by
/// [`StatementDdlGates::recursive_views`](crate::dialect::StatementDdlGates::recursive_views)):
/// the keyword
/// sits between the `TEMP`/`TEMPORARY` prefix and `VIEW`, never composes with
/// `MATERIALIZED` (engine-rejected), and — mirroring the engine, which desugars a
/// recursive view to `WITH RECURSIVE` — requires the explicit `columns` list, so
/// the parser rejects the bare `RECURSIVE VIEW v AS …` form.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateView<X: Extension = NoExt> {
    /// Whether the or replace form was present in the source.
    pub or_replace: bool,
    /// The MySQL `[ALGORITHM = …] [DEFINER = …] [SQL SECURITY …]` definition-option prefix
    /// (see [`ViewOptions`]); all-`None` for every non-MySQL view and a bare MySQL view. The
    /// options sit between `OR REPLACE` and the `VIEW` keyword.
    pub options: ViewOptions,
    /// Whether the materialized form was present in the source.
    pub materialized: bool,
    /// Whether the recursive form was present in the source.
    pub recursive: bool,
    /// Optional temporary for this syntax.
    pub temporary: Option<TemporaryTableKind>,
    /// Whether the if not exists form was present in the source.
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Columns in source order.
    pub columns: ThinVec<Ident>,
    /// Query governed by this node.
    pub query: Box<Query<X>>,
    /// Optional check option for this syntax.
    pub check_option: Option<ViewCheckOption>,
    /// Whether the with data form was present in the source.
    pub with_data: Option<bool>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// `WITH [ CASCADED | LOCAL ] CHECK OPTION` on a regular (non-materialized) view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ViewCheckOption {
    /// Bare `WITH CHECK OPTION`; PostgreSQL treats the unqualified form as `CASCADED`.
    Unspecified,
    /// `WITH CASCADED CHECK OPTION` — enforce the check on this and all underlying views.
    Cascaded,
    /// `WITH LOCAL CHECK OPTION` — enforce the check on this view only.
    Local,
}

/// The MySQL `ALGORITHM = { UNDEFINED | MERGE | TEMPTABLE }` view-processing algorithm, the
/// first of the [`ViewOptions`] definition-option prefix. A `Copy` tag whose span rides the
/// owning [`ViewOptions`], like [`SqlSecurityContext`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ViewAlgorithm {
    /// `ALGORITHM = UNDEFINED` — the server chooses between `MERGE` and `TEMPTABLE` (the default).
    Undefined,
    /// `ALGORITHM = MERGE` — the view's text is merged into the referencing statement.
    Merge,
    /// `ALGORITHM = TEMPTABLE` — the view result is materialized into a temporary table.
    TempTable,
}

/// The MySQL view definition-option prefix `[ALGORITHM = …] [DEFINER = …] [SQL SECURITY …]`,
/// shared verbatim by [`CreateView`] and [`AlterView`] — the two statements name the same
/// options in the same fixed source order, immediately before the `VIEW` keyword.
///
/// Each option is independently optional; the source order (algorithm, then definer, then SQL
/// security) is engine-required — a permutation (`DEFINER` before `ALGORITHM`, `SQL SECURITY`
/// before either) is `ER_PARSE_ERROR`. All-`None` is the ordinary view with no prefix (every
/// non-MySQL dialect, and a bare MySQL `CREATE/ALTER VIEW`). The [`Definer`] reuses the shared
/// routine/event account reference; the [`SqlSecurityContext`] reuses the routine security tag.
/// This is a non-spanned bundle — [`Definer`] carries its own span, and the two `Copy` tags
/// ride the owning statement's span, so the prefix needs no `meta` of its own.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ViewOptions {
    /// The optional `ALGORITHM = …` processing algorithm; `None` when omitted.
    pub algorithm: Option<ViewAlgorithm>,
    /// The optional `DEFINER = <user>` account; `None` when omitted. Boxed like the routine
    /// definer (a rare fat field paying only a null pointer when absent).
    pub definer: Option<Box<Definer>>,
    /// The optional `SQL SECURITY { DEFINER | INVOKER }` privilege context; `None` when omitted.
    pub sql_security: Option<SqlSecurityContext>,
}

/// `ALTER [ALGORITHM = …] [DEFINER = …] [SQL SECURITY …] VIEW <name> [(<columns>)] AS <query>
/// [WITH [CASCADED | LOCAL] CHECK OPTION]` — the MySQL view redefinition (gated by
/// [`StatementDdlGates::view_definition_options`](crate::dialect::StatementDdlGates::view_definition_options)).
///
/// MySQL's `ALTER VIEW` re-specifies the whole view body; it is the [`CreateView`] grammar
/// minus `OR REPLACE`/`IF NOT EXISTS` (server-measured: both `ER_PARSE_ERROR` after `ALTER`)
/// and the `TEMP`/`MATERIALIZED`/`RECURSIVE` prefixes MySQL has no views for. The shared
/// [`ViewOptions`] prefix and the [`ViewCheckOption`] tail are reused verbatim, and the body
/// reuses [`Query`] like every view.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterView<X: Extension = NoExt> {
    /// The `[ALGORITHM = …] [DEFINER = …] [SQL SECURITY …]` definition-option prefix; all-`None`
    /// for a bare `ALTER VIEW`.
    pub options: ViewOptions,
    /// The view name (`db.view` or bare `view`).
    pub name: ObjectName,
    /// The optional explicit output-column list; empty when omitted.
    pub columns: ThinVec<Ident>,
    /// The redefining `AS <query>` body.
    pub query: Box<Query<X>>,
    /// The optional `WITH [CASCADED | LOCAL] CHECK OPTION` constraint.
    pub check_option: Option<ViewCheckOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A DuckDB `CREATE [PERSISTENT] SECRET <name> ( <option> <value> [, ...] )`
/// secrets-management statement (DuckDB-specific; gated by
/// [`StatementDdlGates::create_secret`](crate::dialect::StatementDdlGates::create_secret)).
///
/// DuckDB stores credentials (S3/HTTP/… access) as named secrets. `persistent`
/// records the `PERSISTENT` keyword (a persistent secret survives the session; the
/// bare form is session-temporary). The parenthesized [`options`](Self::options) are
/// `<name> <value>` pairs — the required `TYPE <provider>` plus provider-specific
/// settings (`KEY_ID`, `REGION`, …). Only the bare/`PERSISTENT` name-then-options form
/// the corpus exercises is modelled; the explicit `TEMPORARY` keyword, `OR REPLACE`,
/// `IF NOT EXISTS`, the anonymous (unnamed) secret, and the `IN <storage>` clause are
/// not part of this shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateSecret<X: Extension = NoExt> {
    /// Whether the persistent form was present in the source.
    pub persistent: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Options supplied in source order.
    pub options: ThinVec<SecretOption<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `<name> <value>` option inside a [`CreateSecret`] option list (`TYPE S3`,
/// `KEY_ID '...'`). The value is modelled as a general [`Expr`] — DuckDB's own option
/// value is a bare word, string, or list — so the corpus `TYPE <provider>` form
/// round-trips; the wider expression grammar is the recorded acceptance bound.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct SecretOption<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Value supplied by this syntax.
    pub value: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A DuckDB `DROP [PERSISTENT | TEMPORARY] SECRET [IF EXISTS] <name> [FROM <storage>]`
/// secrets-management statement (DuckDB-specific; gated by
/// [`StatementDdlGates::create_secret`](crate::dialect::StatementDdlGates::create_secret) —
/// the same whole-statement gate that admits [`CreateSecret`], because `DROP SECRET` is the
/// same secrets behaviour surface and rides the one flag rather than a second gate).
///
/// The drop counterpart of [`CreateSecret`]. In DuckDB's grammar `drop_secret.y` is the
/// *only* `DROP` with its own top-level `stmt` production (not a [`DropStatement`] object
/// kind), because it carries the `opt_persist` persistence modifier and a `FROM <storage>`
/// backend selector — neither of which the shared name-list DROP grammar has.
/// [`persistence`](Self::persistence) records which of the three `opt_persist` spellings
/// preceded `SECRET` (absent → [`SecretPersistence::Default`]);
/// [`storage`](Self::storage) is the optional `FROM <backend>` secret-storage name. The
/// dropped secret is a single identifier (grammar `ColId`), never a qualified object name.
///
/// Non-generic: a secret drop names a secret, an optional storage backend, and two flags —
/// it carries no expressions or extension nodes — so it parallels the leaf
/// [`DropStatement`]/[`DropTransform`] payloads.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DropSecretStmt {
    /// Which `opt_persist` spelling preceded `SECRET`; see [`SecretPersistence`].
    pub persistence: SecretPersistence,
    /// Whether the `IF EXISTS` existence guard was present in the source.
    pub if_exists: bool,
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Optional `FROM <storage>` secret-storage backend selector.
    pub storage: Option<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `opt_persist` persistence modifier on a DuckDB `DROP SECRET`: which of the three
/// storage scopes the statement names. (The wider `CREATE SECRET` grammar shares the same
/// `opt_persist` production; [`CreateSecret`] models only its `PERSISTENT`/absent forms as
/// a `bool`, so this three-valued modifier lives with the drop that needs all three.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SecretPersistence {
    /// No modifier written — DuckDB's `default` secret scope (`DROP SECRET s`).
    Default,
    /// `TEMPORARY` — the session-scoped, in-memory secret store.
    Temporary,
    /// `PERSISTENT` — the on-disk persistent secret store.
    Persistent,
}

/// A SQLite `CREATE VIRTUAL TABLE [IF NOT EXISTS] [<schema> .] <name> USING <module>
/// [( <arg> [, <arg>] * )]` statement (SQLite-specific; gated by
/// [`StatementDdlGates::create_virtual_table`](crate::dialect::StatementDdlGates::create_virtual_table)).
///
/// A virtual table delegates its storage and query implementation to a *module*
/// (`fts5`, `rtree`, `csv`, …). The module — not SQLite's core grammar — owns the
/// argument syntax: `fts5` reads column names and `tokenize = …` options, `rtree`
/// reads dimension columns, and a bespoke module reads whatever it likes. SQLite's
/// own parser therefore imposes almost no structure on the argument list — it splits
/// the parenthesized text on the *top-level* commas (parentheses nest and quoted
/// strings are transparent) and hands each raw slice to the module verbatim, even
/// tolerating empty members (`USING m(a,,b)`). Module resolution and any argument
/// validation happen at execution time, so the parse layer accepts an unknown module
/// and any balanced-parenthesis token soup.
///
/// Each argument is consequently modelled as an OPAQUE verbatim [`ModuleArg`] (the
/// interned source text of one top-level slice), never a parsed sub-grammar — imposing
/// column/option structure would invent constraints SQLite does not enforce.
///
/// [`args`](Self::args) is `None` for the bare `USING m` form and `Some` (possibly
/// empty) for the parenthesized `USING m (…)` form, so the two round-trip distinctly.
/// The name admits at most two parts (`schema.table`); SQLite has no `TEMP` virtual
/// table, so no temporary modifier is modelled. Non-generic: the arguments hold no
/// expressions or extension nodes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateVirtualTable {
    /// Whether the if not exists form was present in the source.
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// The virtual-table module name (`USING <module>`).
    pub module: Ident,
    /// Arguments in source order.
    pub args: Option<ThinVec<ModuleArg>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One opaque, verbatim argument of a [`CreateVirtualTable`] module argument list.
///
/// [`text`](Self::text) is the interned source text of a single top-level
/// comma-delimited slice, preserved exactly (internal spacing and all) because the
/// module owns its grammar — the parser never interprets it. An empty slice (from
/// `USING m(a,,b)` or a trailing comma) is a legal empty argument.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ModuleArg {
    /// The comment text.
    pub text: Symbol,
    /// Source location and node identity.
    pub meta: Meta,
}

/// `CREATE [UNIQUE] INDEX [CONCURRENTLY] [IF NOT EXISTS] [<name>] ON <table>
/// [USING <method>] (<column> [, ...]) [WHERE <predicate>]`.
///
/// `UNIQUE` is portable; `CONCURRENTLY`, the `USING <method>` access-method clause,
/// and the trailing `WHERE` partial-index predicate are PostgreSQL extensions gated
/// by `schema_change_syntax.index_extensions` dialect data. The index
/// name is optional — PostgreSQL derives one from the table and columns when it is
/// omitted (`CREATE INDEX ON t (a)`). COLLATE / operator-class / `INCLUDE` column
/// decorations are a deliberate follow-up.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateIndex<X: Extension = NoExt> {
    /// Whether the unique form was present in the source.
    pub unique: bool,
    /// Whether the concurrently form was present in the source.
    pub concurrently: bool,
    /// Whether the if not exists form was present in the source.
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: Option<Ident>,
    /// Table referenced by this syntax.
    pub table: ObjectName,
    /// Optional using for this syntax.
    pub using: Option<Ident>,
    /// Columns in source order.
    pub columns: ThinVec<IndexColumn<X>>,
    /// Predicate that controls this clause.
    pub predicate: Option<Box<Expr<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One key column of a [`CreateIndex`]: an expression with optional sort modifiers.
///
/// A bare column is the common case (`expr` is an [`Expr::Column`](super::Expr)); a
/// parenthesized expression indexes a computed value. `asc` / `nulls_first` mirror
/// [`OrderByExpr`](super::OrderByExpr) and stay `None` when the modifier is
/// unwritten, leaving the dialect default to the consumer.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct IndexColumn<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Expr<X>,
    /// Whether the asc form was present in the source.
    pub asc: Option<bool>,
    /// Whether the nulls first form was present in the source.
    pub nulls_first: Option<bool>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A SQLite `CREATE [TEMP|TEMPORARY] TRIGGER [IF NOT EXISTS] [<schema> .] <name>
/// [BEFORE | AFTER | INSTEAD OF] <event> ON <table> [FOR EACH ROW] [WHEN <expr>]
/// BEGIN <stmt>; ... END` statement (SQLite-specific; gated by
/// [`StatementDdlGates::create_trigger`](crate::dialect::StatementDdlGates::create_trigger)).
///
/// Unlike the other `CREATE` families — which are always accepted because their body
/// grammar is standard — the trigger gate is real: only SQLite's `BEGIN … END`
/// SQL-statement-body form is modelled, and PostgreSQL/MySQL spell an incompatible
/// body (`EXECUTE FUNCTION f()` / an external routine), which they genuinely reject
/// for this form. Gating to SQLite (and Lenient) is therefore behaviour-accurate, not
/// only a modelling limitation.
///
/// The [`body`](Self::body) is a non-empty sequence of `INSERT`/`UPDATE`/`DELETE`/
/// `SELECT` statements — a statement list inside a statement (the structurally
/// heaviest SQLite shape). It reuses the existing [`Statement`] nodes rather than
/// minting trigger-body variants (the node stays generic over `X`); the
/// parser gates the body kinds and routes each through the recursion-guarded
/// statement dispatcher, and the enclosing [`Statement::CreateTrigger`]
/// boxes this payload to keep the enum within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateTrigger<X: Extension = NoExt> {
    /// Optional temporary for this syntax.
    pub temporary: Option<TemporaryTableKind>,
    /// Whether the if not exists form was present in the source.
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// The fire time; `None` when unwritten (SQLite defaults it to `BEFORE`, but the
    /// absent form is preserved so it round-trips).
    pub timing: Option<TriggerTiming>,
    /// Which DML event fires the trigger; see [`TriggerEvent`].
    pub event: TriggerEvent,
    /// Table referenced by this syntax.
    pub table: ObjectName,
    /// Whether `FOR EACH ROW` was written. SQLite parses (and ignores) it but rejects
    /// `FOR EACH STATEMENT`, so only the `ROW` form is modelled as a surface tag.
    pub for_each_row: bool,
    /// Optional when for this syntax.
    pub when: Option<Expr<X>>,
    /// The trigger-body statement list, in source order (always non-empty — SQLite
    /// rejects an empty `BEGIN END`).
    pub body: ThinVec<Statement<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// When a [`CreateTrigger`] fires relative to the event.
///
/// A tag (no `meta`): the timing keyword's span is subsumed by the enclosing
/// [`CreateTrigger`], exactly as [`TemporaryTableKind`] rides its parent's span.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TriggerTiming {
    /// `BEFORE` — fire before the event is applied.
    Before,
    /// `AFTER` — fire after the event is applied.
    After,
    /// `INSTEAD OF` (valid on a view).
    InsteadOf,
}

/// The DML event that fires a [`CreateTrigger`].
///
/// `UPDATE` alone fires on any column; `UPDATE OF a, b` restricts it to the listed
/// columns ([`columns`](Self::Update::columns) empty for the bare `UPDATE`). `INSERT`
/// and `DELETE` take no column list.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TriggerEvent {
    /// `DELETE` — fire on row deletion.
    Delete {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `INSERT` — fire on row insertion.
    Insert {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `UPDATE [OF col, …]` — fire on row update, optionally restricted to columns.
    Update {
        /// Columns in source order.
        columns: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// `CREATE [DEFINER = <user>] TRIGGER [IF NOT EXISTS] <name> {BEFORE | AFTER}
/// {INSERT | UPDATE | DELETE} ON <table> FOR EACH ROW [{FOLLOWS | PRECEDES} <other>]
/// <sp_proc_stmt>` — the MySQL SQL/PSM trigger (gated by
/// [`StatementDdlGates::compound_statements`](crate::dialect::StatementDdlGates::compound_statements),
/// the same stored-program gate the routine wrappers ride).
///
/// A DISTINCT node from the SQLite [`CreateTrigger`], not an extension of it, because the two
/// shapes do not genuinely unify — the decisive split is the body: SQLite's is a
/// `BEGIN <stmt>; … END` *list* of plain SQL statements ([`CreateTrigger::body`], a
/// [`ThinVec`]), MySQL's is a *single* `sp_proc_stmt` — usually a
/// [`Statement::Compound`] block, but any one body statement (a `SET`,
/// a flow-control construct) — parsed through the shared `parse_body_statement` seam and boxed
/// because [`Statement`] is large. Their decorating axes are disjoint too: SQLite carries
/// `TEMP` / `WHEN` / `INSTEAD OF` / `UPDATE OF <cols>`, MySQL carries [`DEFINER`](Self::definer),
/// mandatory `BEFORE`/`AFTER` timing, mandatory `FOR EACH ROW`, and the
/// [`FOLLOWS`/`PRECEDES` ordering](Self::ordering) anchor. Folding both into one node would
/// leave half its fields dialect-dead — the [`CreateProcedure`] precedent (a MySQL
/// stored-program object gets its own node rather than overloading a cross-dialect one) applies.
///
/// The [`timing`](Self::timing) and [`event`](Self::event) axes reuse the shared
/// [`TriggerTiming`] / [`TriggerEvent`] vocabulary — MySQL simply never emits the SQLite-only
/// [`TriggerTiming::InsteadOf`] or a non-empty [`TriggerEvent::Update`] column list (the parser
/// enforces the bare forms), so the reuse is safe and keeps one trigger-axis vocabulary.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateStoredTrigger<X: Extension = NoExt> {
    /// The optional `DEFINER = <user>` account prefix; `None` when omitted. Boxed (the rare
    /// fat field pays only a null pointer when absent), like [`CreateProcedure::definer`].
    pub definer: Option<Box<Definer>>,
    /// Whether the `IF NOT EXISTS` guard was written (MySQL 8.0.29+).
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// The fire time (`BEFORE` / `AFTER`) — mandatory in MySQL (no defaulted/absent form, and
    /// never [`TriggerTiming::InsteadOf`]).
    pub timing: TriggerTiming,
    /// The DML event that fires the trigger (`INSERT` / `UPDATE` / `DELETE`) — always the bare
    /// form; MySQL has no `UPDATE OF <cols>`, so a [`TriggerEvent::Update`] carries no columns.
    pub event: TriggerEvent,
    /// Table referenced by this syntax.
    pub table: ObjectName,
    /// The optional `{FOLLOWS | PRECEDES} <other>` ordering anchor relative to another trigger on
    /// the same table and event; `None` for the unordered form.
    pub ordering: Option<TriggerOrder>,
    /// The trigger body: one `sp_proc_stmt`, boxed. Usually a
    /// [`Statement::Compound`] `BEGIN … END` block, but any single
    /// body statement is admitted, parsed through the `parse_body_statement` seam.
    pub body: Box<Statement<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The MySQL `{FOLLOWS | PRECEDES} <other_trigger>` ordering clause of a
/// [`CreateStoredTrigger`] (`trigger_follows_precedes_clause`): where the new trigger fires
/// relative to an existing one on the same table and event.
///
/// The anchor is an [`Ident`] (MySQL's `ident_or_text` — a bare or quoted trigger name) so both
/// source spellings round-trip from its span. The keyword direction is the enum variant.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TriggerOrder {
    /// `FOLLOWS <other>` — fire immediately after the named trigger.
    Follows {
        /// The anchor trigger this one follows.
        anchor: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PRECEDES <other>` — fire immediately before the named trigger.
    Precedes {
        /// The anchor trigger this one precedes.
        anchor: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// `CREATE DATABASE [IF NOT EXISTS] <name>`.
///
/// Non-generic, like [`CreateSchema`]: a database is named by identifiers only. The
/// trailing `[WITH] <option> ...` list (`OWNER`, `TEMPLATE`, `ENCODING`, …) is a
/// deliberate follow-up — real migrations rarely spell it and it carries no
/// expressions the shape must compare.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateDatabase {
    /// The MySQL `IF NOT EXISTS` existence guard (the [`CreateTable`] precedent). Gated
    /// for acceptance by
    /// [`ExistenceGuards::create_database_if_not_exists`](crate::dialect::ExistenceGuards):
    /// PostgreSQL has no `CREATE DATABASE IF NOT EXISTS`, so the flag is off there and
    /// the guard surfaces as a clean parse error.
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Source location and node identity.
    pub meta: Meta,
}

/// `CREATE [OR REPLACE] FUNCTION <name> ( [<param> [, ...]] ) [RETURNS <type>]
/// [<option> ...]`.
///
/// The routine body has two disjoint grammatical homes on the [`FunctionBody`] axis. An
/// opaque source *string* is introduced by the order-independent `AS` option, spelled either
/// single-quoted (`AS 'SELECT 1'`) or dollar-quoted (`AS $$ … $$` / `AS $tag$ … $tag$`) where
/// the dialect enables [`dollar_quoted_strings`](crate::dialect::StringLiteralSyntax); the
/// delimiter and verbatim body text round-trip from the body [`Literal`]'s span. A *live* SQL
/// body — a SQL-standard `RETURN <expr>` (PostgreSQL 14+ / standard `opt_routine_body`) — is
/// the trailing [`body`](Self::body) slot, which strictly follows the whole option list (proven
/// against the PG oracle: `LANGUAGE sql RETURN 1` accepts, `RETURN 1 LANGUAGE sql` rejects), so
/// it is a distinct parse position from `AS`, not another order-independent option. The two
/// homes share the [`FunctionBody`] *type* (the axis vocabulary the dollar-body sibling staged)
/// but not a grammatical slot.
///
/// The node is generic over `X` because a parameter or `RETURNS` type can be a host-owned
/// [`DataType::Other`] under a custom dialect, and the live
/// [`body`](Self::body) carries an [`Expr`]`<X>` (stock builtins pin `X = NoExt`). The M1
/// surface is the parameter list, an optional `RETURNS <type>`, the [`FunctionOption`] cluster
/// the corpus exercises (`LANGUAGE` / `AS` / null-call behaviour), and the trailing `RETURN`
/// body; the fuller volatility, security, parallelism, and `SET` option matrix — and the
/// statement-list `BEGIN ATOMIC … END` body that shares the trailing slot — are deliberate
/// follow-ups the open option list and the [`FunctionBody`] axis extend without a shape change.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateFunction<X: Extension = NoExt> {
    /// Whether the or replace form was present in the source.
    pub or_replace: bool,
    /// The optional MySQL `DEFINER = <user>` clause; `None` when omitted (always `None`
    /// for the PostgreSQL string-body routine, which has no definer prefix). Boxed because a
    /// [`Definer`] is comparatively large and the clause is rare, so a definer-less routine
    /// pays only a null pointer (the [`FunctionParam::default`] precedent).
    pub definer: Option<Box<Definer>>,
    /// Whether the MySQL `IF NOT EXISTS` guard was written (MySQL 8.0.29+); always `false`
    /// for PostgreSQL, which spells the intent `CREATE OR REPLACE`.
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// params in source order.
    pub params: ThinVec<FunctionParam<X>>,
    /// Optional returns for this syntax.
    pub returns: Option<DataType<X>>,
    /// Options supplied in source order.
    pub options: ThinVec<FunctionOption<X>>,
    /// The trailing SQL-standard routine body (`opt_routine_body`) — a `RETURN <expr>` live
    /// SQL expression that follows the entire option list. Boxed and optional because it is
    /// usually absent (an `AS`-string routine leaves it unset, paying one null pointer — the
    /// [`FunctionParam::default`] precedent); a [`FunctionBody::Definition`] string body never
    /// lands here (it rides the `AS` option instead).
    pub body: Option<Box<FunctionBody<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One routine parameter: an optional argument [`mode`](Self::mode)
/// (`IN`/`OUT`/`INOUT`/`VARIADIC`), an optional name, its type (`b INT`), and an
/// optional default value (`b INT DEFAULT 0` / `b INT = 0`).
///
/// The three lead-in facets are independent: a bare type (`f(int)`) leaves both
/// `mode` and `name` unset, a mode may precede an unnamed type (`f(OUT int)`), and a
/// named parameter may carry a default. The name is PostgreSQL's `param_name` =
/// `type_function_name` production (parsed with the [type-name reservation
/// set](crate::dialect::FeatureSet::reserved_type_name)): a `type_func_name` keyword
/// like `left` is a legal parameter name, a `col_name` keyword like `int` is not — so
/// a lone `int` is unambiguously the type, mirroring how PostgreSQL resolves the
/// name-vs-type ambiguity. The default is the PostgreSQL `func_arg_with_default`
/// tail — boxed because it is usually absent and a [`FunctionParamDefault`] (an
/// [`Expr`]) is comparatively large, so a default-less parameter pays only a null
/// pointer (the [`ForeignKeyRef`] referential-action precedent). It is *parameter
/// metadata*: an ordinary function-**call** argument is a
/// [`FunctionArg`](crate::ast::FunctionArg) on the unrelated call path and is
/// unaffected.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct FunctionParam<X: Extension = NoExt> {
    /// Mode selected by this syntax.
    pub mode: Option<FunctionParamMode>,
    /// Name referenced by this syntax.
    pub name: Option<Ident>,
    /// Data type named by this syntax.
    pub data_type: DataType<X>,
    /// Optional default for this syntax.
    pub default: Option<Box<FunctionParamDefault<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A routine parameter's argument mode — PostgreSQL's `arg_class` prefix
/// (`func_arg: arg_class param_name func_type | …`). `IN` is the default and may be
/// omitted; `OUT`/`INOUT` mark output parameters; `VARIADIC` marks a trailing
/// array-spread parameter. A fieldless [`Copy`] tag whose span rides the owning
/// [`FunctionParam`] (the [`FunctionParamDefaultSpelling`] precedent): it records only
/// the written mode so it round-trips.
///
/// PostgreSQL admits the mode either before *or* after the name (`IN a int` and
/// `a IN int` both parse); this models the documented, canonical mode-first spelling
/// (which sqlparser-rs's `ArgMode` also targets) and adds [`Variadic`](Self::Variadic),
/// which sqlparser-rs omits but real PostgreSQL requires. The rarer name-first spelling
/// is a deliberate parse-surface boundary — no corpus exercises it and PostgreSQL itself
/// normalizes the two orders to one AST.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum FunctionParamMode {
    /// `IN` — an input parameter (the default when omitted).
    In,
    /// `OUT` — an output parameter.
    Out,
    /// `INOUT` — an input/output parameter.
    InOut,
    /// `VARIADIC` — a trailing array-spread parameter.
    Variadic,
}

/// A routine parameter's default-value clause: the [`spelling`](Self::spelling)
/// (`DEFAULT` keyword vs `=`) and the value [`Expr`]. PostgreSQL's grammar spells
/// one production two ways — `func_arg_with_default: func_arg DEFAULT a_expr |
/// func_arg '=' a_expr` — so the tag records which the source used and the value is
/// a live SQL expression reusing [`Expr`] rather than a re-derived literal grammar.
///
/// sqlparser-rs models the same clause as a bare `default_expr: Option<Expr>` and
/// always re-renders it as `= <expr>`, normalizing the `DEFAULT` spelling away; this
/// AST instead carries the fieldless [`FunctionParamDefaultSpelling`] tag so both
/// forms round-trip verbatim (the spelling-fidelity doctrine, as for
/// [`EqualsSpelling`](crate::ast::EqualsSpelling)).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct FunctionParamDefault<X: Extension = NoExt> {
    /// Exact source spelling retained for faithful rendering.
    pub spelling: FunctionParamDefaultSpelling,
    /// Value supplied by this syntax.
    pub value: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Surface spelling for a [`FunctionParamDefault`]: PostgreSQL admits both the
/// `DEFAULT` keyword and a bare `=` to introduce a routine parameter's default, two
/// spellings of the one `func_arg_with_default` production. A `Copy` tag whose span
/// rides the owning [`FunctionParamDefault`], like [`MacroSpelling`]; it records the
/// source form only so rendering round-trips exactly (a fidelity tag, not a validity
/// one — the parser accepts both spellings wherever defaults are gated on).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum FunctionParamDefaultSpelling {
    /// The `DEFAULT <expr>` keyword form.
    Default,
    /// The `= <expr>` operator form.
    Equals,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL function option forms represented by the AST.
pub enum FunctionOption<X: Extension = NoExt> {
    /// A `LANGUAGE <name>` clause naming the routine's implementation language. The name is a
    /// `NonReservedWord_or_Sconst` ([`LanguageName`]) — a bare word or, on the PostgreSQL
    /// surface, an `Sconst` string (`LANGUAGE 'sql'`/`E'sql'`/`$$sql$$`) — shared with the
    /// [`DoArg::Language`](crate::ast::DoArg) argument.
    Language {
        /// Name referenced by this syntax.
        name: LanguageName,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `AS <body>` — the *string* routine body, carried on the [`FunctionBody`] axis. The `AS`
    /// option only ever homes an opaque source string (plain `'…'` or dollar-quoted
    /// `$tag$…$tag$`), i.e. [`FunctionBody::Definition`]; the live `RETURN <expr>` body rides
    /// the trailing [`CreateFunction::body`] slot, a disjoint grammatical position.
    As {
        /// The routine's string body; see [`FunctionBody`].
        body: FunctionBody<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The null-call behaviour (`CALLED ON NULL INPUT` / `RETURNS NULL ON NULL INPUT`
    /// / `STRICT`).
    NullBehavior {
        /// The null-input behaviour; see [`FunctionNullBehavior`].
        behavior: FunctionNullBehavior,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The MySQL `[NOT] DETERMINISTIC` routine characteristic — whether the routine
    /// always produces the same result for the same inputs. The `not` field records the negated
    /// spelling so `NOT DETERMINISTIC` round-trips distinctly from a
    /// routine that simply omits the characteristic. A stored-routine option (MySQL SQL/PSM),
    /// not a PostgreSQL `CREATE FUNCTION` clause.
    Deterministic {
        /// Whether the `NOT` prefix was written (`NOT DETERMINISTIC`).
        not: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The MySQL SQL-data-access routine characteristic (`CONTAINS SQL` / `NO SQL` /
    /// `READS SQL DATA` / `MODIFIES SQL DATA`) — the advisory declaration of how the
    /// routine body touches data.
    DataAccess {
        /// Which data-access class was written; see [`SqlDataAccess`].
        access: SqlDataAccess,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The MySQL `SQL SECURITY {DEFINER | INVOKER}` routine characteristic — the privilege
    /// context the routine executes under.
    SqlSecurity {
        /// The security context; see [`SqlSecurityContext`].
        context: SqlSecurityContext,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The MySQL `COMMENT '<string>'` routine characteristic — a stored descriptive comment.
    /// The PostgreSQL comment surface is the separate `COMMENT ON FUNCTION` statement, so
    /// this inline option is MySQL-only.
    Comment {
        /// The comment string literal.
        comment: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The MySQL routine SQL-data-access characteristic — how the routine body is declared to
/// touch data. Advisory (the server does not enforce it), recorded so it round-trips. A
/// `Copy` tag whose span rides the owning [`FunctionOption::DataAccess`], like
/// [`FunctionNullBehavior`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SqlDataAccess {
    /// `CONTAINS SQL` — the body contains SQL but neither reads nor writes data (the default).
    ContainsSql,
    /// `NO SQL` — the body contains no SQL.
    NoSql,
    /// `READS SQL DATA` — the body reads but does not modify data.
    ReadsSqlData,
    /// `MODIFIES SQL DATA` — the body may modify data.
    ModifiesSqlData,
}

/// The MySQL `SQL SECURITY` context — whose privileges a routine runs under. A `Copy` tag
/// whose span rides the owning [`FunctionOption::SqlSecurity`], like [`SqlDataAccess`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SqlSecurityContext {
    /// `SQL SECURITY DEFINER` — run under the routine definer's privileges (the default).
    Definer,
    /// `SQL SECURITY INVOKER` — run under the calling user's privileges.
    Invoker,
}

/// The executable body of a [`CreateFunction`] routine — the body-representation **axis**.
///
/// Two body *kinds* are modelled today. [`Definition`](Self::Definition) is an opaque source
/// string introduced by the order-independent `AS` option (`AS 'SELECT 1'`, `AS $$ … $$`,
/// `AS $body$ … $body$`). [`Return`](Self::Return) is the SQL-standard *live* body
/// `RETURN <expr>` (PostgreSQL 14+ / standard `opt_routine_body`), a real SQL [`Expr`] that
/// rides the trailing [`CreateFunction::body`] slot — a *distinct* parse position from `AS`
/// (the `RETURN` body strictly follows the whole option list, oracle-proven). This is a
/// distinct enum — not a bare [`Literal`] field on [`FunctionOption::As`] — precisely so the
/// live body reuses one body vocabulary across both slots rather than reshaping the option.
///
/// The axis is body *kind* (opaque string vs. live SQL), **not** quote style: a plain `'…'`
/// and a dollar-quoted `$tag$…$tag$` body are both
/// [`LiteralKind::String`](crate::ast::LiteralKind) and differ only in source spelling. That
/// spelling — the delimiter tag and the verbatim body text — round-trips from the
/// [`Literal`]'s span; it is recovered from source, never normalized, so a dollar body
/// re-renders byte-for-byte.
///
/// The enum is generic over `X` because [`Return`](Self::Return) carries an [`Expr`]`<X>`; the
/// [`Definition`](Self::Definition) string variant is generic-free. A statement-list
/// `BEGIN ATOMIC … END` body (PostgreSQL, oracle-accepted) shares the trailing slot and slots
/// in here as a further variant carrying a routine-body statement list — deliberately deferred:
/// modelling it honestly needs a routine-body statement grammar (its inner `RETURN` is not a
/// top-level [`Statement`]), which is a separate surface from this single-`Expr` body.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum FunctionBody<X: Extension = NoExt> {
    /// `AS <string>` — an opaque routine body in the target language, kept as the source
    /// [`Literal`] (plain-quoted or dollar-quoted) rather than re-parsed, because the body's
    /// grammar is the target language, not SQL.
    Definition {
        /// Definition text supplied by this syntax.
        definition: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RETURN <expr>` — the SQL-standard live expression body (`opt_routine_body`'s
    /// `ReturnStmt`). The [`Expr`] is boxed (the [`MacroBody`] precedent) so the common `AS`
    /// string body pays no [`Expr`] footprint on the shared [`FunctionOption::As`] path.
    Return {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL SQL/PSM routine body — one `routine_body` statement, usually the
    /// `BEGIN … END` compound block ([`Statement::Compound`])
    /// but any single body statement (a bare `SET`, a flow-control construct, a `RETURN`).
    /// This is the live statement-list body the [`FunctionBody`] axis doc anticipated; it
    /// rides the trailing [`CreateFunction::body`] slot exactly as [`Return`](Self::Return)
    /// does — a distinct grammatical position after the whole characteristic list. Boxed
    /// ([`Statement`] is large) so the common `AS`-string path pays no footprint.
    Block {
        /// The routine body statement (typically a compound block); parsed through the
        /// `parse_body_statement` seam so its inner grammar is the MySQL body sub-language.
        body: Box<Statement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// How a routine handles a NULL argument. `Strict` is the exact shorthand for
/// `ReturnsNullOnNull`, kept distinct so the written spelling round-trips. A `Copy`
/// leaf enum whose span lives on the owning [`FunctionOption`], like [`DropBehavior`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum FunctionNullBehavior {
    /// `CALLED ON NULL INPUT` — run the routine even when an argument is null.
    CalledOnNull,
    /// `RETURNS NULL ON NULL INPUT` — return null without running when any argument is null.
    ReturnsNullOnNull,
    /// `STRICT` — shorthand for `RETURNS NULL ON NULL INPUT`.
    Strict,
}

/// The MySQL `DEFINER = <user>` clause prefixing a routine (and, on other tickets, a
/// view/trigger/event) definition — the account the object's `SQL SECURITY DEFINER` context
/// resolves to.
///
/// This is the *measured minimal* account surface the routine family needs: the bare
/// `<user>[@<host>]` account name and the `CURRENT_USER [()]` self-reference, the two forms
/// the corpus and the oracle exercise. The full MySQL account-name axis — the quoting-nuance
/// matrix, role grants, and the `IDENTIFIED BY` authentication tail — is owned by
/// `parse-mysql-user-role-ddl`; this node deliberately stops at the account *reference* a
/// routine header carries, and that ticket widens it (or lifts it to a shared account node)
/// without reshaping the routine statements that hold it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum Definer {
    /// `DEFINER = <user>[@<host>]` — a named account. The `user` and optional `host` are
    /// each an [`Ident`] so both the bare (`root`) and quoted (`'root'@'localhost'`) source
    /// spellings round-trip from their spans.
    Account {
        /// The account user name.
        user: Ident,
        /// The optional `@<host>` part; `None` for a bare user with no host.
        host: Option<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DEFINER = CURRENT_USER [()]` — the current user at definition time.
    CurrentUser {
        /// Whether the empty `()` call-style parentheses were written.
        parens: bool,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// `CREATE [DEFINER = <user>] PROCEDURE [IF NOT EXISTS] <name> ( [<param> [, …]] )
/// [<characteristic> …] <routine_body>` — the MySQL stored-procedure definition (SQL/PSM).
///
/// A distinct node from [`CreateFunction`]: a procedure has no `RETURNS` type, its body may
/// not contain `RETURN` (the routine family enforces `ER_SP_BADRETURN`), and it is invoked by
/// `CALL` rather than in an expression. It shares the routine *vocabulary* — the parameter
/// list reuses [`FunctionParam`] (with its `IN`/`OUT`/`INOUT` [`FunctionParamMode`]) and the
/// characteristic list reuses [`FunctionOption`] (`LANGUAGE`/`COMMENT`/`DETERMINISTIC`/data
/// access/`SQL SECURITY`) — but not a statement shape. The [`body`](Self::body) is a single
/// MySQL body statement (typically a [`Statement::Compound`]
/// block), parsed through the `parse_body_statement` seam and boxed because [`Statement`] is
/// large.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateProcedure<X: Extension = NoExt> {
    /// The optional `DEFINER = <user>` clause; `None` when omitted. Boxed (the rare fat
    /// field pays only a null pointer when absent), like [`CreateFunction::definer`].
    pub definer: Option<Box<Definer>>,
    /// Whether the `IF NOT EXISTS` guard was written (MySQL 8.0.29+).
    pub if_not_exists: bool,
    /// The procedure name.
    pub name: ObjectName,
    /// The parameter list, in source order (always parenthesized, possibly empty).
    pub params: ThinVec<FunctionParam<X>>,
    /// The routine characteristics, in source order (`LANGUAGE`/`COMMENT`/`DETERMINISTIC`/
    /// data-access/`SQL SECURITY`), carried on the shared [`FunctionOption`] axis.
    pub characteristics: ThinVec<FunctionOption<X>>,
    /// The routine body — one MySQL body statement, usually a compound block.
    pub body: Box<Statement<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Which routine kind an [`AlterRoutine`] targets — the `PROCEDURE`/`FUNCTION` object keyword.
/// A `Copy` surface tag whose span rides the owning [`AlterRoutine`], like
/// [`ShowRoutineKind`](crate::ast::ShowRoutineKind).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum RoutineKind {
    /// `PROCEDURE`.
    Procedure,
    /// `FUNCTION`.
    Function,
}

/// `ALTER {PROCEDURE | FUNCTION} <name> [<characteristic> …]` — the MySQL routine-characteristics
/// alteration (SQL/PSM). Characteristics only: unlike [`CreateProcedure`]/[`CreateFunction`]
/// there is no body and no parameter list — `ALTER` re-declares only the mutable characteristic
/// subset (`COMMENT` / `LANGUAGE` / data access / `SQL SECURITY`; the server rejects
/// `DETERMINISTIC` here, which the parser enforces). One node spans both routine kinds via
/// [`kind`](Self::kind); boxed by [`Statement::AlterRoutine`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterRoutine<X: Extension = NoExt> {
    /// Whether this alters a `PROCEDURE` or a `FUNCTION`.
    pub kind: RoutineKind,
    /// The routine name.
    pub name: ObjectName,
    /// The re-declared characteristics, in source order (the `ALTER`-legal subset).
    pub characteristics: ThinVec<FunctionOption<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `ON SCHEDULE <when>` clause of a MySQL `CREATE`/`ALTER EVENT` — the one-shot
/// `AT <timestamp>` form or the recurring `EVERY <interval> [STARTS …] [ENDS …]` form.
///
/// Both timestamp positions ([`At::at`](Self::At::at), [`Every::starts`](Self::Every::starts),
/// [`Every::ends`](Self::Every::ends)) are ordinary [`Expr`]s, so a `NOW() + INTERVAL 1 DAY`
/// offset rides the expression grammar with no special schedule handling. The recurring
/// interval is `<value> <unit>` where the unit reuses the shared [`IntervalFields`] vocabulary
/// (MySQL's `interval` production, rendered in underscore spelling — `DAY_HOUR`, `MINUTE_SECOND`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum EventSchedule<X: Extension = NoExt> {
    /// `AT <timestamp>` — a one-shot event fired once at the given time. The [`Expr`] is
    /// boxed to keep the schedule (and the enclosing event node) lean.
    At {
        /// The one-shot execution timestamp (any expression, e.g. `NOW() + INTERVAL 1 DAY`).
        at: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `EVERY <value> <unit> [STARTS <ts>] [ENDS <ts>]` — a recurring event. The `<value>
    /// <unit>` interval reuses the shared [`IntervalFields`] vocabulary; `STARTS`/`ENDS` are
    /// optional window bounds (each an ordinary expression).
    Every {
        /// The interval count (an expression, typically an integer literal).
        value: Box<Expr<X>>,
        /// The interval unit, in the shared [`IntervalFields`] vocabulary (MySQL `interval`).
        unit: IntervalFields,
        /// The optional `STARTS <ts>` window start.
        starts: Option<Box<Expr<X>>>,
        /// The optional `ENDS <ts>` window end.
        ends: Option<Box<Expr<X>>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// `ON COMPLETION [NOT] PRESERVE` — whether a MySQL event is retained after its last
/// execution. A `Copy` surface tag whose span rides the owning event node, like
/// [`RoutineKind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum EventOnCompletion {
    /// `ON COMPLETION PRESERVE` — keep the event definition after its final run.
    Preserve,
    /// `ON COMPLETION NOT PRESERVE` — drop the event after its final run (the default).
    NotPreserve,
}

/// The `ENABLE | DISABLE [ON SLAVE|REPLICA]` activation state of a MySQL event. A `Copy`
/// surface tag whose span rides the owning event node.
///
/// [`DisableOnReplica`](Self::DisableOnReplica) carries a [`ReplicaSpelling`] because MySQL
/// 8.4 still admits BOTH the deprecated `DISABLE ON SLAVE` and the current `DISABLE ON
/// REPLICA` (server-measured: both accept, `SLAVE` warns) and the source spelling must
/// round-trip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum EventStatus {
    /// `ENABLE` — the event is active.
    Enable,
    /// `DISABLE` — the event is inactive on every node.
    Disable,
    /// `DISABLE ON SLAVE` / `DISABLE ON REPLICA` — the event is active only on the source,
    /// disabled on replicas. The [`ReplicaSpelling`] records which keyword was written.
    DisableOnReplica(ReplicaSpelling),
}

/// Which of MySQL's two interchangeable replica keywords a source used — the deprecated
/// `SLAVE` or the current `REPLICA`. Both parse on MySQL 8.4 (`SLAVE` with a deprecation
/// warning), so the spelling is a round-trip tag, not a semantic difference. A `Copy` leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ReplicaSpelling {
    /// The deprecated `SLAVE` keyword (still admitted on MySQL 8.4 with a warning).
    Slave,
    /// The current `REPLICA` keyword.
    Replica,
}

/// `CREATE [DEFINER = <user>] EVENT [IF NOT EXISTS] <name> ON SCHEDULE <schedule>
/// [ON COMPLETION [NOT] PRESERVE] [ENABLE | DISABLE [ON SLAVE|REPLICA]] [COMMENT '…']
/// DO <body>` — the MySQL scheduled-event definition (SQL/PSM, gated by
/// [`StatementDdlGates::compound_statements`](crate::dialect::StatementDdlGates::compound_statements),
/// the stored-program body surface).
///
/// The clause order after the name is fixed by the grammar (schedule, completion, status,
/// comment, `DO`) — server-measured: a comment before the status is a syntax error. The
/// [`body`](Self::body) is a single MySQL body statement (usually a [`Statement::Compound`]
/// block), parsed through the `parse_body_statement` seam and boxed because [`Statement`] is
/// large; an event body carries no return value, so `RETURN` inside it is rejected (as in a
/// procedure body — server `ER_SP_BADRETURN`). The [`Definer`] prefix reuses the shared
/// routine account reference.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateEvent<X: Extension = NoExt> {
    /// The optional `DEFINER = <user>` clause; `None` when omitted. Boxed like the routine
    /// definer (a rare fat field paying only a null pointer when absent).
    pub definer: Option<Box<Definer>>,
    /// Whether the `IF NOT EXISTS` guard was written.
    pub if_not_exists: bool,
    /// The event name (`db.event` or bare `event`).
    pub name: ObjectName,
    /// The `ON SCHEDULE` clause — the `AT` one-shot or `EVERY` recurring form.
    pub schedule: EventSchedule<X>,
    /// The optional `ON COMPLETION [NOT] PRESERVE` clause.
    pub on_completion: Option<EventOnCompletion>,
    /// The optional `ENABLE | DISABLE [ON SLAVE|REPLICA]` activation state.
    pub status: Option<EventStatus>,
    /// The optional `COMMENT '…'` description.
    pub comment: Option<Literal>,
    /// The event body — one MySQL body statement (the `DO <stmt>` clause).
    pub body: Box<Statement<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// `ALTER [DEFINER = <user>] EVENT <name> [ON SCHEDULE <schedule>] [ON COMPLETION [NOT]
/// PRESERVE] [RENAME TO <name>] [ENABLE | DISABLE [ON SLAVE|REPLICA]] [COMMENT '…']
/// [DO <body>]` — the MySQL event alteration.
///
/// Every clause is optional, but the grammar requires **at least one** (server-measured: a
/// bare `ALTER EVENT e` is a syntax error), which the parser enforces. The clause order is
/// the same fixed order as [`CreateEvent`], with `RENAME TO` between the schedule/completion
/// and the status. Unlike [`CreateEvent`] the schedule and body are optional, and
/// [`on_completion`](Self::on_completion) may appear with or without a schedule.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterEvent<X: Extension = NoExt> {
    /// The optional `DEFINER = <user>` clause; `None` when omitted.
    pub definer: Option<Box<Definer>>,
    /// The event name.
    pub name: ObjectName,
    /// The optional `ON SCHEDULE` clause.
    pub schedule: Option<EventSchedule<X>>,
    /// The optional `ON COMPLETION [NOT] PRESERVE` clause.
    pub on_completion: Option<EventOnCompletion>,
    /// The optional `RENAME TO <name>` clause.
    pub rename_to: Option<ObjectName>,
    /// The optional `ENABLE | DISABLE [ON SLAVE|REPLICA]` activation state.
    pub status: Option<EventStatus>,
    /// The optional `COMMENT '…'` description.
    pub comment: Option<Literal>,
    /// The optional `DO <body>` re-definition (one MySQL body statement).
    pub body: Option<Box<Statement<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// `DROP EVENT [IF EXISTS] <name>` — the MySQL scheduled-event drop. Kept apart from the
/// generic [`Statement::Drop`] because MySQL's event drop names exactly ONE event
/// (server-measured: `DROP EVENT a, b` is a syntax error) and takes no `CASCADE`/`RESTRICT`
/// behaviour, unlike the shared name-list [`DropStatement`] grammar.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DropEvent {
    /// Whether the `IF EXISTS` guard was written.
    pub if_exists: bool,
    /// The single event name (`db.event` or bare `event`).
    pub name: ObjectName,
    /// Source location and node identity.
    pub meta: Meta,
}

/// `DROP {DATABASE | SCHEMA} [IF EXISTS] <name>` — MySQL's single-database drop, where
/// `DATABASE` and `SCHEMA` are exact synonyms (the lexer folds `SCHEMA` onto `DATABASE`).
/// Kept apart from the generic name-list [`Statement::Drop`] because the form names exactly
/// ONE unqualified database — server-measured on mysql:8, `DROP DATABASE a, b` is
/// `ER_PARSE_ERROR`, `DROP DATABASE db.x` is `ER_PARSE_ERROR`, and it takes no
/// `CASCADE`/`RESTRICT` (both syntax errors) — unlike PostgreSQL/DuckDB's `DROP SCHEMA
/// <name> [, …] [CASCADE | RESTRICT]`, which keeps riding the shared [`DropStatement`]
/// grammar there. Gated by
/// [`StatementDdlGates::drop_database`](crate::dialect::StatementDdlGates::drop_database).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DropDatabase {
    /// Which of the two synonymous keywords was written; recorded so the source spelling
    /// round-trips verbatim (the tag pattern of [`TemporaryTableKind`], not a semantic
    /// difference — MySQL treats the two identically).
    pub spelling: DatabaseKeyword,
    /// Whether the `IF EXISTS` guard was written.
    pub if_exists: bool,
    /// The single database name — a bare unqualified identifier (`ident`; a dotted
    /// `db.x` is a server syntax error).
    pub name: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The keyword spelling of a database object — `DATABASE` or its exact synonym `SCHEMA`
/// (MySQL folds the two onto one grammar). Recorded on [`DropDatabase`] so the written
/// keyword round-trips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DatabaseKeyword {
    /// The `DATABASE` spelling.
    Database,
    /// The `SCHEMA` spelling (a MySQL synonym for `DATABASE`).
    Schema,
}

/// `DROP INDEX <name> ON <table> [ALGORITHM [=] {DEFAULT | INPLACE | INSTANT | COPY}]
/// [LOCK [=] {DEFAULT | NONE | SHARED | EXCLUSIVE}]` — MySQL's index drop (`drop_index_stmt`,
/// `sql_yacc.yy`). Kept apart from the generic name-list [`Statement::Drop`] because it names
/// the owning table with a mandatory `ON <table>` (server-measured: `DROP INDEX i` with no
/// `ON` is `ER_PARSE_ERROR` on mysql:8) and carries the online-DDL `ALGORITHM`/`LOCK`
/// execution hints, neither of which the shared grammar has. The index name is a bare
/// identifier (`ident`; a dotted `i.j` is a syntax error) while the table is a possibly-dotted
/// [`ObjectName`] (`db.t` binds). Gated by
/// [`IndexAlterSyntax::index_drop_on_table`](crate::dialect::IndexAlterSyntax::index_drop_on_table).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DropIndexOnTable {
    /// The dropped index — a bare unqualified identifier.
    pub name: Ident,
    /// The table the index belongs to (`ON <table>`); may be schema-qualified (`db.t`).
    pub table: ObjectName,
    /// The trailing `ALGORITHM`/`LOCK` execution hints in source order (`opt_index_lock_and_algorithm`).
    /// The grammar admits at most one of each and both orderings (`ALGORITHM … LOCK …` or
    /// `LOCK … ALGORITHM …`, server-measured: a repeated `ALGORITHM`/`LOCK` is `ER_PARSE_ERROR`),
    /// so this holds zero, one, or two entries and preserves the written order for round-trip.
    pub options: ThinVec<IndexLockAlgorithmOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One entry of a `DROP INDEX … ON …` (or online `ALTER TABLE`) `opt_index_lock_and_algorithm`
/// tail — an `ALGORITHM` or a `LOCK` execution hint. Each records whether the optional `=`
/// (`opt_equal`) was written so the surface form round-trips verbatim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IndexLockAlgorithmOption {
    /// `ALGORITHM [=] {DEFAULT | INPLACE | INSTANT | COPY}`.
    Algorithm {
        /// Whether the optional `=` was written between the keyword and the value.
        equals: bool,
        /// The chosen algorithm.
        value: IndexAlgorithm,
    },
    /// `LOCK [=] {DEFAULT | NONE | SHARED | EXCLUSIVE}`.
    Lock {
        /// Whether the optional `=` was written between the keyword and the value.
        equals: bool,
        /// The chosen lock level.
        value: IndexLock,
    },
}

/// The online-DDL `ALGORITHM` value (`alter_algorithm_option_value`). `DEFAULT` is a keyword;
/// the rest are matched case-insensitively as identifiers, and an unknown value is a *binding*
/// reject (`ER_UNKNOWN_ALTER_ALGORITHM` 1800), not a syntax error — so only these four bind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IndexAlgorithm {
    /// `DEFAULT` — let the server choose.
    Default,
    /// `INPLACE` — rebuild in place where supported.
    Inplace,
    /// `INSTANT` — metadata-only change where supported.
    Instant,
    /// `COPY` — rebuild via a table copy.
    Copy,
}

/// The online-DDL `LOCK` value (`alter_lock_option_value`). `DEFAULT` is a keyword; the rest
/// are matched case-insensitively as identifiers, and an unknown value is a *binding* reject
/// (`ER_UNKNOWN_ALTER_LOCK` 1801), not a syntax error — so only these four bind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IndexLock {
    /// `DEFAULT` — the server's default concurrency.
    Default,
    /// `NONE` — permit concurrent reads and writes.
    None,
    /// `SHARED` — permit concurrent reads only.
    Shared,
    /// `EXCLUSIVE` — permit no concurrent access.
    Exclusive,
}

/// `CREATE [OR REPLACE] [TEMP|TEMPORARY] {MACRO | FUNCTION} [IF NOT EXISTS] <name>
/// (<param> [, ...]) AS <body>` — DuckDB's macro DDL (gated by
/// [`StatementDdlGates::create_macro`](crate::dialect::StatementDdlGates::create_macro)).
///
/// This is a distinct node from [`CreateFunction`], not a variant of it: a DuckDB
/// macro body is a *live* SQL expression or query (`AS x + 1` / `AS TABLE SELECT …`),
/// whereas a PostgreSQL/MySQL `CREATE FUNCTION` body is an opaque source *string* in a
/// target language (`AS 'RETURN 1'`). The two share only the `CREATE … (params) AS`
/// prefix; their bodies, parameter grammars (a macro parameter is a bare untyped name,
/// a routine parameter is `[name] <type>`), and option tails are disjoint.
///
/// DuckDB spells the same feature with either the `MACRO` keyword or `FUNCTION` as an
/// exact synonym; [`spelling`](Self::spelling) records which so the source keyword
/// round-trips (the tag pattern of [`TemporaryTableKind`], not a semantic difference).
/// Whole-statement gated to DuckDB (and Lenient): every other dialect either has no
/// macro grammar (`MACRO` is left unconsumed and surfaces as an unknown statement) or
/// spells `CREATE FUNCTION` as the incompatible string-body routine, so the gate is
/// behaviour-accurate, not merely a modelling limitation. Boxed by the enclosing
/// [`Statement::CreateMacro`] to keep the enum within its size budget.
///
/// The single-body surface — one `(params) AS body` per statement — is modelled; DuckDB
/// additionally accepts a comma-separated *overload* list (`… AS a, (x, y) AS b`), a
/// deliberate follow-up (no vendored corpus statement spells it) the shape would extend
/// by lifting `params`/`body` into an overload sequence.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateMacro<X: Extension = NoExt> {
    /// Whether the or replace form was present in the source.
    pub or_replace: bool,
    /// Optional temporary for this syntax.
    pub temporary: Option<TemporaryTableKind>,
    /// Exact source spelling retained for faithful rendering.
    pub spelling: MacroSpelling,
    /// Whether the if not exists form was present in the source.
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// params in source order.
    pub params: ThinVec<MacroParam<X>>,
    /// Statement or query body governed by this node.
    pub body: MacroBody<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Which keyword introduced a [`CreateMacro`] — DuckDB accepts `MACRO` and `FUNCTION`
/// as exact synonyms. A `Copy` tag whose span rides the owning [`CreateMacro`], like
/// [`TemporaryTableKind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum MacroSpelling {
    /// Source used the `MACRO` spelling.
    Macro,
    /// Source used the `FUNCTION` spelling.
    Function,
}

/// One [`CreateMacro`] parameter: a bare untyped name with an optional `:= <expr>`
/// default (`CREATE MACRO m(a, b := 10) AS …`). Unlike a [`FunctionParam`], a macro
/// parameter carries no type — DuckDB macros are template-substituted, not typed — so
/// the name is mandatory and no `DataType` rides the node.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct MacroParam<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Optional default for this syntax.
    pub default: Option<Expr<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A [`CreateMacro`] body: a scalar expression (`AS <expr>`) or a table-producing
/// query (`AS TABLE <query>`). The `TABLE` keyword is the DuckDB discriminant between a
/// scalar macro (returns one value) and a table macro (returns a relation); the body
/// grammar reuses [`Expr`] / [`Query`] rather than re-deriving either.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum MacroBody<X: Extension = NoExt> {
    /// A scalar macro body — an expression returning a single value.
    Scalar {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A table macro body — a query returning a relation.
    Table {
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// `CREATE [OR REPLACE] [TEMP|TEMPORARY] TYPE [IF NOT EXISTS] <name> AS <definition>` —
/// DuckDB's user-defined-type DDL (gated by
/// [`StatementDdlGates::create_type`](crate::dialect::StatementDdlGates::create_type)).
///
/// The name is optionally schema-qualified (`CREATE TYPE s1.mood AS …`). Its
/// [`definition`](Self::definition) is either the dedicated `ENUM` production (a label
/// list or a label-supplying query) or an alias to any other (possibly composite/nested)
/// data type — see [`CreateTypeDefinition`].
///
/// `OR REPLACE` and `IF NOT EXISTS` are mutually exclusive in DuckDB's grammar (under `OR
/// REPLACE` the parser reads `IF` as the type name), so the parser fills `if_not_exists`
/// only on the plain form; the two flags are never both set. Whole-statement gated to
/// DuckDB (and Lenient): every other dialect leaves `TYPE` unconsumed after `CREATE`,
/// where it surfaces as the `TABLE` expectation (an unknown statement). Boxed by the
/// enclosing [`Statement::CreateType`] to keep the enum
/// within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateType<X: Extension = NoExt> {
    /// Whether the or replace form was present in the source.
    pub or_replace: bool,
    /// Optional temporary for this syntax.
    pub temporary: Option<TemporaryTableKind>,
    /// Whether the if not exists form was present in the source.
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// The type definition — an `ENUM` label list or a type alias; see [`CreateTypeDefinition`].
    pub definition: CreateTypeDefinition<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `AS <definition>` body of a [`CreateType`]. DuckDB's `CREATE TYPE` grammar has a
/// dedicated `ENUM` rule distinct from the general data-type grammar — its labels are
/// parser-restricted to string constants (`ENUM(1, 2)` is a *parse* error, unlike the
/// data-type-position `x::ENUM(...)` cast, which parses any modifier and only bind-rejects
/// a non-string) — so an enum definition is its own variant rather than a
/// [`DataType::Enum`](crate::ast::DataType). Every non-`ENUM` spelling (`STRUCT`/`MAP`/
/// `UNION`, an alias to a named type, arrays, `DECIMAL(p, s)`, …) reuses the shared data
/// type via [`Alias`](Self::Alias).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CreateTypeDefinition<X: Extension = NoExt> {
    /// `AS ENUM ('a', 'b', …)` — a labelled enumeration. The label list may be empty
    /// (`ENUM ()`, which DuckDB accepts) and each label is a string literal.
    Enum {
        /// labels in source order.
        labels: ThinVec<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `AS ENUM (<query>)` — the enum labels are drawn from a query's single column
    /// (`ENUM (SELECT DISTINCT month FROM …)`); the parenthesized query reuses [`Query`].
    EnumFromQuery {
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `AS <type>` — an alias to another type: a scalar (`AS VARCHAR`), a composite/nested
    /// constructor (`AS STRUCT(…)` / `AS MAP(…)` / `AS UNION(…)`), an array (`AS my_int[]`),
    /// or a named user type. Reuses the shared [`DataType`] grammar.
    Alias {
        /// Data type named by this syntax.
        data_type: DataType<X>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A `CREATE [TEMPORARY] SEQUENCE [IF NOT EXISTS] <name> [<option> ...]` sequence-generator
/// statement (SQL:2003 T176; PostgreSQL and DuckDB), gated by
/// [`StatementDdlGates::create_sequence`](crate::dialect::StatementDdlGates::create_sequence).
///
/// The trailing options are the SQL-standard sequence-generator core — `START [WITH]`,
/// `INCREMENT [BY]`, `MINVALUE`/`NO MINVALUE`, `MAXVALUE`/`NO MAXVALUE`, `CYCLE`/`NO CYCLE` —
/// which both engines' parsers accept in any order, so this single node is gated
/// per-dialect by the one flag rather than split into parallel PostgreSQL/DuckDB nodes
/// (ADR-0011). They reuse [`IdentityOption`], the identical option vocabulary the
/// `GENERATED … AS IDENTITY` column form already carries; the [`Cache`](IdentityOption::Cache)
/// variant is never produced here — DuckDB's `CREATE SEQUENCE` grammar rejects `CACHE`
/// (engine-measured), so admitting it would over-accept, and it is a PostgreSQL-only
/// extension left to a follow-up.
///
/// PostgreSQL's extended tails (`AS <type>`, `CACHE`, `OWNED BY`, `RESTART`) and DuckDB's
/// `OR REPLACE SEQUENCE` are deliberately unmodelled here: each is one engine's extension,
/// not the shared standard core, and none is exercised by the DuckDB corpus.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateSequence<X: Extension = NoExt> {
    /// Optional temporary for this syntax.
    pub temporary: Option<TemporaryTableKind>,
    /// Whether the if not exists form was present in the source.
    pub if_not_exists: bool,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Options supplied in source order.
    pub options: ThinVec<IdentityOption<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `CREATE EXTENSION [IF NOT EXISTS] <name> [WITH] [SCHEMA s] [VERSION v] [CASCADE]`
/// statement (PostgreSQL; gated by
/// [`StatementDdlGates::extension_ddl`](crate::dialect::StatementDdlGates::extension_ddl)).
///
/// Non-generic: an extension is named by a bare identifier and its options carry only
/// names and version strings — no expressions or extension nodes. The `WITH` keyword is
/// optional sugar (PostgreSQL's `opt_with`); [`with`](Self::with) records whether it was
/// written so it round-trips. The options are order-independent and repeatable in the
/// grammar (`create_extension_opt_list`), kept here in source order.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateExtension {
    /// Whether the if not exists form was present in the source.
    pub if_not_exists: bool,
    /// The extension name.
    pub name: Ident,
    /// Whether the optional `WITH` keyword preceded the option list.
    pub with: bool,
    /// Options supplied in source order.
    pub options: ThinVec<CreateExtensionOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `CREATE EXTENSION` option (`create_extension_opt_item`). PostgreSQL's grammar
/// carries a `FROM <old_version>` item whose action is a parse-time
/// `FEATURE_NOT_SUPPORTED` reject, so that form is not modelled here.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CreateExtensionOption {
    /// `SCHEMA <name>` — the schema the extension's objects are installed into.
    Schema {
        /// The target schema name.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `VERSION <version>` — the version to install.
    Version {
        /// Version value supplied by this syntax.
        version: ExtensionVersion,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CASCADE` — install missing required extensions too.
    Cascade {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// An extension version value (PostgreSQL's `NonReservedWord_or_Sconst`): a bare word or
/// a string constant. PostgreSQL folds both to the same string internally, but the
/// surface spelling is kept distinct so it round-trips.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ExtensionVersion {
    /// A bare non-reserved word (`VERSION v1`).
    Word {
        /// Identifier-form version value.
        word: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A string constant (`VERSION '1.0'`).
    String {
        /// Value supplied by this syntax.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// An `ALTER EXTENSION <name> ...` statement, gated by
/// [`StatementDdlGates::extension_ddl`](crate::dialect::StatementDdlGates::extension_ddl).
///
/// Unifies PostgreSQL's two `ALTER EXTENSION` productions under one node, since both
/// share the `ALTER EXTENSION <name>` head: the `UPDATE [TO <version>]` version bump
/// (`AlterExtensionStmt`) and the `ADD`/`DROP <member>` membership change
/// (`AlterExtensionContentsStmt`). PostgreSQL's `ALTER EXTENSION <name> SET SCHEMA
/// <schema>` is a separate `AlterObjectSchemaStmt` production (shared with every other
/// relocatable object), not an `AlterExtensionStmt`, so it is out of this node's scope.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterExtension<X: Extension = NoExt> {
    /// The extension name.
    pub name: Ident,
    /// The change applied to the extension.
    pub action: AlterExtensionAction<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The change an [`AlterExtension`] applies.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AlterExtensionAction<X: Extension = NoExt> {
    /// `UPDATE [TO <version>]` — bump the installed version (`AlterExtensionStmt`).
    Update {
        /// The target version, when a `TO` clause was written.
        version: Option<ExtensionVersion>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ADD <member>` (`add` is `true`) or `DROP <member>` — change extension membership
    /// (`AlterExtensionContentsStmt`).
    Change {
        /// `true` for `ADD`, `false` for `DROP`.
        add: bool,
        /// The member object added to or dropped from the extension.
        member: ObjectReference<X>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// An `ALTER DATABASE [IF EXISTS] <name> SET ALIAS TO <alias>` statement (DuckDB), gated by
/// [`StatementDdlGates::alter_database`](crate::dialect::StatementDdlGates::alter_database).
///
/// DuckDB's sole `AlterDatabaseStmt` production re-aliases an attached database
/// (`third_party/libpg_query/grammar/statements/alter_database.y` at the pinned v1.5.4
/// commit `08e34c447b`). The grammar reduces `SET <ident> TO <name>`, but its action rejects
/// any keyword but `ALIAS`, so only `SET ALIAS TO` is a parse-accept (engine-measured: `SET
/// FOO TO` is a syntax error, and `ALTER DATABASE d RENAME TO e` is likewise a syntax error —
/// DuckDB has no `RENAME` form for a database). The change is held in an
/// [`AlterDatabaseAction`] enum so a sibling dialect (MySQL's charset/collation `ALTER
/// DATABASE`) can add its own action variants without reshaping this node.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterDatabase {
    /// Whether the `IF EXISTS` existence guard was present in the source.
    pub if_exists: bool,
    /// The database name.
    pub name: Ident,
    /// The change applied to the database.
    pub action: AlterDatabaseAction,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The change an [`AlterDatabase`] applies.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AlterDatabaseAction {
    /// `SET ALIAS TO <alias>` — re-alias the attached database.
    SetAlias {
        /// The new alias.
        new_name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// An `ALTER SEQUENCE [IF EXISTS] <name> <option>...` statement (DuckDB), gated by
/// [`StatementDdlGates::alter_sequence`](crate::dialect::StatementDdlGates::alter_sequence).
///
/// DuckDB's `AlterSeqStmt` production changes a sequence generator's options
/// (`third_party/libpg_query/grammar/statements/alter_sequence.y` at the pinned v1.5.4
/// commit). Its `SeqOptList` is the same option grammar `CREATE SEQUENCE` reduces, so the
/// shared core is carried through the reused [`IdentityOption`] axis
/// ([`AlterSequenceOption::Common`]); the ALTER-only leads DuckDB additionally parses
/// (`RESTART`, `AS <type>`, `OWNED BY`, `SEQUENCE NAME`) are their own variants. Options
/// are kept in source order. `SET SCHEMA` is *not* an option here — it is DuckDB's separate
/// [`AlterObjectSchema`] production, dispatched before this one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterSequence<X: Extension = NoExt> {
    /// Whether the `IF EXISTS` existence guard was present in the source.
    pub if_exists: bool,
    /// The sequence name.
    pub name: ObjectName,
    /// Options supplied in source order.
    pub options: ThinVec<AlterSequenceOption<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `ALTER SEQUENCE` option (DuckDB's `SeqOptElem`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AlterSequenceOption<X: Extension = NoExt> {
    /// A shared sequence-generator option `CREATE SEQUENCE` also accepts (`START [WITH]`,
    /// `INCREMENT [BY]`, `MIN`/`MAXVALUE`, `NO MIN`/`MAXVALUE`, `CYCLE`/`NO CYCLE`, `CACHE`),
    /// reusing the [`IdentityOption`] axis.
    Common {
        /// The reused shared option.
        option: IdentityOption<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RESTART [[WITH] <n>]` — reset the sequence's current value (bare = restart to its
    /// start value).
    Restart {
        /// The restart value, when a `[WITH] <n>` tail was written.
        value: Option<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `AS <type>` — change the sequence's integer type.
    As {
        /// The sequence's new integer type.
        data_type: DataType<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `OWNED BY {<column> | NONE}` — tie the sequence's lifetime to a column, or detach it
    /// (`None` renders `NONE`).
    OwnedBy {
        /// The owning column, or `None` for `OWNED BY NONE`.
        owner: Option<ObjectName>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SEQUENCE NAME <name>` — the internal rename form libpg_query documents as pg_dump-only.
    SequenceName {
        /// The new internal name.
        name: ObjectName,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// An `ALTER {TABLE | VIEW | SEQUENCE} [IF EXISTS] <name> SET SCHEMA <schema>` statement
/// (DuckDB's `AlterObjectSchemaStmt`), gated by
/// [`StatementDdlGates::alter_object_set_schema`](crate::dialect::StatementDdlGates::alter_object_set_schema).
///
/// Relocates a relocatable object to another schema
/// (`third_party/libpg_query/grammar/statements/alter_schema.y` at the pinned v1.5.4 commit).
/// DuckDB 1.5.4's binder rejects this with `Not implemented: T_AlterObjectSchemaStmt`, but the
/// production is parse-reachable (engine-measured via `json_serialize_sql`), and PARSE-level
/// parity is the modelled bar — analogous to PostgreSQL's grammar-present, engine-unimplemented
/// `CreateAssertionStmt`. The object kind is one of [`SchemaRelocationObject`]'s three.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterObjectSchema {
    /// Which relocatable object kind was named.
    pub object_type: SchemaRelocationObject,
    /// Whether the `IF EXISTS` existence guard was present in the source.
    pub if_exists: bool,
    /// The object's current (schema-qualified) name.
    pub name: ObjectName,
    /// The destination schema.
    pub new_schema: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The relocatable object kind an [`AlterObjectSchema`] moves — DuckDB's `AlterObjectSchemaStmt`
/// admits exactly these three object heads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SchemaRelocationObject {
    /// `ALTER TABLE … SET SCHEMA`.
    Table,
    /// `ALTER VIEW … SET SCHEMA`.
    View,
    /// `ALTER SEQUENCE … SET SCHEMA`.
    Sequence,
}

/// A reference to a database object by kind and kind-appropriate signature — the shared
/// object-reference axis for PostgreSQL object-membership DDL.
///
/// PostgreSQL's `ALTER EXTENSION … ADD|DROP <member>` grammar
/// (`AlterExtensionContentsStmt`) names a member object by a kind keyword plus a
/// signature whose shape depends on the kind: a bare or qualified name, a routine
/// argument-type signature, an aggregate/operator signature, a cast type pair, a type
/// name, or a transform. The sibling object-DDL heads name objects the same way — the
/// `parse-pg-alter-object-depends` head (`ALTER … DEPENDS ON EXTENSION`) reuses
/// [`Routine`](Self::Routine) and [`Named`](Self::Named) for its `FUNCTION`/`PROCEDURE`/
/// `ROUTINE`, `MATERIALIZED VIEW`, and `INDEX` objects, and `parse-pg-drop-transform`
/// reuses [`Transform`](Self::Transform) — so the axis lives here once rather than being
/// re-derived per head.
///
/// [`Trigger`](Self::Trigger) is the one shape no `ALTER EXTENSION` member takes: its
/// `TRIGGER <name> ON <table>` form is exclusive to the `DEPENDS ON EXTENSION` head
/// (`ALTER EXTENSION … ADD TRIGGER` is a reject), so it was added to the axis additively
/// when that head's measured grammar demanded it, keeping every object-DDL head on one
/// shared reference type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ObjectReference<X: Extension = NoExt> {
    /// A kind keyword and a name. [`kind`](ObjectRefKind) records whether the name may be
    /// schema-qualified: PostgreSQL's `object_type_any_name` kinds (`TABLE`, `VIEW`, …)
    /// accept a dotted `any_name`, its `object_type_name` kinds (`SCHEMA`, `ROLE`, …)
    /// only a single `name`.
    Named {
        /// The object kind; see [`ObjectRefKind`].
        kind: ObjectRefKind,
        /// The referenced name.
        name: ObjectName,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `FUNCTION`/`PROCEDURE`/`ROUTINE <name>[(<argtypes>)]`
    /// (PostgreSQL's `function_with_argtypes`; the argument list is optional).
    Routine {
        /// Which routine kind; see [`RoutineObjectKind`].
        kind: RoutineObjectKind,
        /// The routine name and optional argument-type signature.
        signature: RoutineSignature<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `AGGREGATE <name>(<aggr_args>)`.
    Aggregate {
        /// The aggregate name.
        name: ObjectName,
        /// The aggregate argument signature.
        args: AggregateArgs<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `OPERATOR [<schema>.]<sym>(<left>, <right>)` (PostgreSQL's `operator_with_argtypes`).
    Operator {
        /// The optional schema qualification (empty for an unqualified operator).
        schema: ObjectName,
        /// The symbolic operator spelling, interned exact-case.
        op: Symbol,
        /// The operand types.
        args: OperatorArgs<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `OPERATOR CLASS`/`OPERATOR FAMILY <name> USING <access_method>`.
    OperatorClass {
        /// `true` for `OPERATOR FAMILY`, `false` for `OPERATOR CLASS`.
        family: bool,
        /// The operator class/family name.
        name: ObjectName,
        /// The `USING <access_method>` index access method.
        access_method: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CAST (<from> AS <to>)`.
    Cast {
        /// The cast's source type.
        from: DataType<X>,
        /// The cast's target type.
        to: DataType<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DOMAIN`/`TYPE <typename>` — a type-named object.
    Type {
        /// `true` for the `DOMAIN` keyword, `false` for `TYPE` (both name a type via a
        /// `Typename`).
        domain: bool,
        /// The type name (a `Typename`, possibly schema-qualified).
        name: DataType<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `TRANSFORM FOR <typename> LANGUAGE <language>`.
    Transform {
        /// The type the transform is `FOR`.
        type_name: DataType<X>,
        /// The transform's procedural language name.
        language: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `TRIGGER <name> ON <table>` — a trigger named by its bare name plus the table it
    /// is defined on (PostgreSQL's `ALTER TRIGGER name ON qualified_name`). Unlike the
    /// other object kinds, a trigger is not schema-qualifiable on its own name (it is a
    /// `name`, a single `ColId`); the `table` carries the qualification.
    ///
    /// This variant is reached only by the `DEPENDS ON EXTENSION` head — no `ALTER
    /// EXTENSION` member is a trigger — so `parse_object_reference` never constructs it.
    Trigger {
        /// The trigger's bare name.
        name: Ident,
        /// The table the trigger is defined on (a `qualified_name`).
        table: ObjectName,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A member-object kind naming an object by a bare or qualified name — the
/// name-only shapes of [`ObjectReference::Named`], covering PostgreSQL's
/// `object_type_any_name` and `object_type_name` productions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ObjectRefKind {
    /// `TABLE`.
    Table,
    /// `SEQUENCE`.
    Sequence,
    /// `VIEW`.
    View,
    /// `MATERIALIZED VIEW`.
    MaterializedView,
    /// `INDEX`.
    Index,
    /// `FOREIGN TABLE`.
    ForeignTable,
    /// `COLLATION`.
    Collation,
    /// `CONVERSION`.
    Conversion,
    /// `STATISTICS`.
    Statistics,
    /// `TEXT SEARCH PARSER`.
    TextSearchParser,
    /// `TEXT SEARCH DICTIONARY`.
    TextSearchDictionary,
    /// `TEXT SEARCH TEMPLATE`.
    TextSearchTemplate,
    /// `TEXT SEARCH CONFIGURATION`.
    TextSearchConfiguration,
    /// `ACCESS METHOD`.
    AccessMethod,
    /// `EVENT TRIGGER`.
    EventTrigger,
    /// `EXTENSION`.
    Extension,
    /// `FOREIGN DATA WRAPPER`.
    ForeignDataWrapper,
    /// `[PROCEDURAL] LANGUAGE`. The optional `PROCEDURAL` keyword is exact-synonym sugar
    /// (PostgreSQL's `opt_procedural`); the canonical render emits `LANGUAGE`.
    Language,
    /// `PUBLICATION`.
    Publication,
    /// `SCHEMA`.
    Schema,
    /// `SERVER` (a foreign server).
    Server,
    /// `DATABASE`.
    Database,
    /// `ROLE`.
    Role,
    /// `TABLESPACE`.
    Tablespace,
}

impl ObjectRefKind {
    /// Whether this kind names a schema-qualifiable object (PostgreSQL's
    /// `object_type_any_name` group), so its name may be a dotted `any_name`. The
    /// remaining kinds are `object_type_name`, which take only a single `name`.
    pub fn schema_qualifiable(self) -> bool {
        matches!(
            self,
            Self::Table
                | Self::Sequence
                | Self::View
                | Self::MaterializedView
                | Self::Index
                | Self::ForeignTable
                | Self::Collation
                | Self::Conversion
                | Self::Statistics
                | Self::TextSearchParser
                | Self::TextSearchDictionary
                | Self::TextSearchTemplate
                | Self::TextSearchConfiguration
        )
    }
}

/// The argument signature of an `AGGREGATE` member (PostgreSQL's `aggr_args`).
///
/// Argument names and modes are dropped, keeping only the type list — the same
/// simplification [`RoutineSignature`] applies to `function_with_argtypes`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AggregateArgs<X: Extension = NoExt> {
    /// `(*)` — the zero-argument wildcard.
    Star {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A direct argument-type list, optionally followed by an ordered-set `ORDER BY`
    /// list: `(<types>)`, `(ORDER BY <types>)` (empty `direct`), or
    /// `(<types> ORDER BY <types>)`.
    Types {
        /// Direct argument types (empty for the leading `ORDER BY` form).
        direct: ThinVec<DataType<X>>,
        /// Ordered-set `ORDER BY` argument types, when the clause is present.
        order_by: Option<ThinVec<DataType<X>>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The operand types of an `OPERATOR` member (PostgreSQL's `oper_argtypes`): a binary or
/// unary operator's left and right operand types, where a `NONE` operand (a unary
/// operator's missing side) is `None`. At least one side is always present.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct OperatorArgs<X: Extension = NoExt> {
    /// The left operand type, or `None` for a left-unary operator's `NONE`.
    pub left: Option<DataType<X>>,
    /// The right operand type, or `None` for a right-unary operator's `NONE`.
    pub right: Option<DataType<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// An `ALTER <object> [NO] DEPENDS ON EXTENSION <extension>` statement (PostgreSQL's
/// `AlterObjectDependsStmt`), gated by
/// [`StatementDdlGates::extension_ddl`](crate::dialect::StatementDdlGates::extension_ddl).
///
/// Records or clears a dependency of a database object on an extension, so that dropping
/// the extension cascades to the object (and `pg_dump` skips it). PostgreSQL admits only
/// four object heads here — `FUNCTION`/`PROCEDURE`/`ROUTINE` (a `function_with_argtypes`),
/// `TRIGGER <name> ON <table>`, `MATERIALIZED VIEW`, and `INDEX` — captured by the shared
/// [`ObjectReference`] axis, so the wider `object_type` keyword set (`TABLE`, `VIEW`,
/// `SEQUENCE`, …) that `object` could otherwise spell is out of the parsed grammar.
///
/// The construct references an extension but operates on a non-extension object; it shares
/// the [`extension_ddl`](crate::dialect::StatementDdlGates::extension_ddl) gate rather than
/// carrying its own, because it is meaningless without the extension system and its dialect
/// boundary (PostgreSQL on, everything else off) is identical to extension DDL's — a
/// dialect that has extensions has this, one that does not (DuckDB's `INSTALL`/`LOAD`) has
/// neither.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterObjectDepends<X: Extension = NoExt> {
    /// The object whose extension dependency is being set — one of the four
    /// [`ObjectReference`] shapes the `DEPENDS` grammar admits.
    pub object: ObjectReference<X>,
    /// Whether the `NO DEPENDS` negation form was written (PostgreSQL's `opt_no`), which
    /// removes the recorded dependency instead of adding it.
    pub no: bool,
    /// The extension the object depends on (a bare `name`, not schema-qualifiable).
    pub extension: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `CREATE SERVER <name> FOREIGN DATA WRAPPER <wrapper> OPTIONS ( <option> [, ...] )`
/// federated-server definition (`sql_yacc.yy` `CREATE SERVER_SYM …`), gated by
/// [`StatementDdlGates::server_definition`](crate::dialect::StatementDdlGates::server_definition).
///
/// Registers a remote server for the `FEDERATED` storage engine. The server name and the
/// `FOREIGN DATA WRAPPER` name are each an `ident_or_text` (a bare/backtick identifier or a
/// quoted string, folded to an [`Ident`] whose quote style round-trips). The `OPTIONS` list is
/// the fixed [`ServerOption`] keyword set and is non-empty — an empty `OPTIONS ()` is
/// `ER_PARSE_ERROR` on mysql:8.4.10. Shares its gate and the [`ServerOption`] axis with
/// [`AlterServer`] (the two differ only in whether a wrapper is named); [`DropServer`] disposes
/// of the same object.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateServer {
    /// The server name (`ident_or_text`).
    pub name: Ident,
    /// The `FOREIGN DATA WRAPPER` name (`ident_or_text`) — the storage-engine wrapper the
    /// server uses.
    pub wrapper: Ident,
    /// The `OPTIONS` list, in source order; non-empty (`server_options_list`).
    pub options: ThinVec<ServerOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `ALTER SERVER <name> OPTIONS ( <option> [, ...] )` federated-server change
/// (`sql_yacc.yy` `ALTER SERVER_SYM …`), gated by
/// [`StatementDdlGates::server_definition`](crate::dialect::StatementDdlGates::server_definition).
///
/// Re-sets connection options on an existing server. Unlike [`CreateServer`] it names no
/// `FOREIGN DATA WRAPPER` (the wrapper is fixed at creation), but the `OPTIONS` list is the
/// same non-empty [`ServerOption`] grammar.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterServer {
    /// The server name (`ident_or_text`).
    pub name: Ident,
    /// The `OPTIONS` list, in source order; non-empty (`server_options_list`).
    pub options: ThinVec<ServerOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `DROP SERVER [IF EXISTS] <name>` federated-server drop (`sql_yacc.yy`
/// `drop_server_stmt`), gated by
/// [`StatementDdlGates::server_definition`](crate::dialect::StatementDdlGates::server_definition).
///
/// Removes a single server; no comma list (`DROP SERVER a, b` is `ER_PARSE_ERROR` on
/// mysql:8.4.10). The name is an `ident_or_text`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DropServer {
    /// Whether the `IF EXISTS` guard was written (`drop_server_stmt`'s `if_exists`).
    pub if_exists: bool,
    /// The server name (`ident_or_text`).
    pub name: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `CREATE`/`ALTER SERVER` option — a fixed keyword ([`ServerOptionKind`]) and its value
/// (`sql_yacc.yy` `server_option`). Every option carries exactly one value: the string-valued
/// keywords (`HOST`/`DATABASE`/`USER`/`PASSWORD`/`SOCKET`/`OWNER`) take a `TEXT_STRING_sys`
/// string literal, and `PORT` takes a `ulong_num` unsigned-integer literal — the parser holds
/// each to its measured value type (`HOST 123` and `PORT '3306'` are both `ER_PARSE_ERROR` on
/// mysql:8.4.10). The value is a [`Literal`] whose [`kind`](Literal::kind) records which.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ServerOption {
    /// Which fixed option keyword this is.
    pub kind: ServerOptionKind,
    /// The option value — a string literal for every kind but [`ServerOptionKind::Port`],
    /// which carries an unsigned-integer literal.
    pub value: Literal,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The fixed keyword of a [`ServerOption`] (`sql_yacc.yy` `server_option`). A surface tag (no
/// `meta` — its span rides the owning [`ServerOption`]). Only these seven keywords are
/// options; any other (`FOO 'bar'`) is `ER_PARSE_ERROR` on mysql:8.4.10.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ServerOptionKind {
    /// `HOST '<host>'` — the remote host name.
    Host,
    /// `DATABASE '<db>'` — the remote default database.
    Database,
    /// `USER '<user>'` — the connection user.
    User,
    /// `PASSWORD '<password>'` — the connection password.
    Password,
    /// `SOCKET '<socket>'` — the remote unix socket path.
    Socket,
    /// `OWNER '<owner>'` — the server owner.
    Owner,
    /// `PORT <n>` — the remote TCP port (an unsigned-integer value, not a string).
    Port,
}

/// A MySQL `ALTER INSTANCE <action>` server-instance administration statement (`sql_yacc.yy`
/// `alter_instance_stmt`), gated by
/// [`StatementDdlGates::alter_instance`](crate::dialect::StatementDdlGates::alter_instance).
///
/// A single instance-wide maintenance action (rotate an encryption master key, reload TLS
/// material or the keyring, toggle the InnoDB redo log) — see [`AlterInstanceAction`]. Unlike
/// the server and database DDL this names no object.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterInstance {
    /// The instance action performed.
    pub action: AlterInstanceAction,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The action of an [`AlterInstance`] (`sql_yacc.yy` `alter_instance_action`). The `INNODB`,
/// `BINLOG`, and `REDO_LOG` words are matched as identifiers by the server (`is_identifier`),
/// so a wrong word is `ER_PARSE_ERROR` (`ROTATE FOO MASTER KEY`, `ENABLE INNODB FOO`); this
/// models only the measured accepted set.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AlterInstanceAction {
    /// `ROTATE INNODB MASTER KEY` — rotate the InnoDB tablespace-encryption master key.
    RotateInnodbMasterKey {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ROTATE BINLOG MASTER KEY` — rotate the binary-log encryption master key.
    RotateBinlogMasterKey {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RELOAD TLS [FOR CHANNEL <channel>] [NO ROLLBACK ON ERROR]` — reconfigure the TLS
    /// context. The optional `FOR CHANNEL` names a bare `ident` (a quoted `'ch'` is
    /// `ER_PARSE_ERROR` on mysql:8.4.10, unlike `FLUSH … FOR CHANNEL`'s string); the default
    /// is to roll back the active context on error, and `NO ROLLBACK ON ERROR` suppresses it.
    ReloadTls {
        /// The `FOR CHANNEL <channel>` name (a bare `ident`), or `None`.
        channel: Option<Ident>,
        /// Whether `NO ROLLBACK ON ERROR` was written.
        no_rollback_on_error: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RELOAD KEYRING` — reload keyring component options from the manifest.
    ReloadKeyring {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ENABLE INNODB REDO_LOG` — re-enable redo logging.
    EnableInnodbRedoLog {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DISABLE INNODB REDO_LOG` — disable redo logging (for bulk load).
    DisableInnodbRedoLog {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL `CREATE [OR REPLACE] SPATIAL REFERENCE SYSTEM [IF NOT EXISTS] <srid> <attributes>`
/// spatial-reference-system definition (`sql_yacc.yy` `create_srs_stmt`), gated by
/// [`StatementDdlGates::spatial_reference_system`](crate::dialect::StatementDdlGates::spatial_reference_system).
///
/// `OR REPLACE` and `IF NOT EXISTS` are mutually exclusive: the grammar has one branch for each
/// (`CREATE OR REPLACE …` and `CREATE … IF NOT EXISTS …`), so `CREATE OR REPLACE … IF NOT EXISTS`
/// is `ER_PARSE_ERROR` on mysql:8.4.10. The `<srid>` is a `real_ulonglong_num` unsigned integer
/// (decimal or `0x` hex; a negative or fractional value is `ER_PARSE_ERROR`, an out-of-`u64`-range
/// value is the post-parse `ER_DATA_OUT_OF_RANGE`). Every attribute is optional and the list is
/// order-free at the grammar level — a missing NAME/DEFINITION is the post-parse semantic reject
/// `ER_SRS_MISSING_MANDATORY_ATTRIBUTE`, and a repeated attribute is
/// `ER_SRS_MULTIPLE_ATTRIBUTE_DEFINITIONS`, neither a syntax error — so [`attributes`](Self::attributes)
/// keeps the source order and admits repeats, matching the recognized grammar rather than the
/// narrower semantic rule.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateSpatialReferenceSystem {
    /// Whether the `OR REPLACE` branch was written (mutually exclusive with
    /// [`if_not_exists`](Self::if_not_exists)).
    pub or_replace: bool,
    /// Whether the `IF NOT EXISTS` guard was written (mutually exclusive with
    /// [`or_replace`](Self::or_replace)).
    pub if_not_exists: bool,
    /// The SRID — a `real_ulonglong_num` unsigned-integer literal (decimal or `0x` hex).
    pub srid: Literal,
    /// The attribute list, in source order (order-free grammar; repeats admitted).
    pub attributes: ThinVec<SrsAttribute>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `CREATE SPATIAL REFERENCE SYSTEM` attribute (`sql_yacc.yy` `srs_attributes`). The
/// attributes are order-free and each independently optional; a repeat of the same attribute is
/// a post-parse semantic reject (`ER_SRS_MULTIPLE_ATTRIBUTE_DEFINITIONS`), not a syntax error,
/// so the parser accepts the recognized grammar and preserves whatever the source wrote.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SrsAttribute {
    /// `NAME '<name>'` — the SRS name (a `TEXT_STRING_sys_nonewline` string literal).
    Name {
        /// The name string.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DEFINITION '<wkt>'` — the WKT coordinate-system definition string.
    Definition {
        /// The definition string.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ORGANIZATION '<org>' IDENTIFIED BY <id>` — the authoring organization and its numeric
    /// coordinate-system id. `IDENTIFIED BY` is mandatory (`ORGANIZATION 'o'` alone is
    /// `ER_PARSE_ERROR`) and the id is a `real_ulonglong_num` integer (`IDENTIFIED BY 'x'` is
    /// `ER_PARSE_ERROR`).
    Organization {
        /// The organization name string.
        organization: Literal,
        /// The `IDENTIFIED BY` numeric coordinate-system id (unsigned-integer literal).
        identifier: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DESCRIPTION '<text>'` — the free-text description string.
    Description {
        /// The description string.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL `DROP SPATIAL REFERENCE SYSTEM [IF EXISTS] <srid>` spatial-reference-system drop
/// (`sql_yacc.yy` `drop_srs_stmt`), gated by
/// [`StatementDdlGates::spatial_reference_system`](crate::dialect::StatementDdlGates::spatial_reference_system).
///
/// Removes a single SRS by id; no comma list (`DROP SPATIAL REFERENCE SYSTEM 1, 2` is
/// `ER_PARSE_ERROR` on mysql:8.4.10). The `<srid>` is a `real_ulonglong_num` unsigned integer.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DropSpatialReferenceSystem {
    /// Whether the `IF EXISTS` guard was written (`drop_srs_stmt`'s `if_exists`).
    pub if_exists: bool,
    /// The SRID — a `real_ulonglong_num` unsigned-integer literal.
    pub srid: Literal,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `CREATE RESOURCE GROUP <name> TYPE [=] {SYSTEM | USER} [VCPU …] [THREAD_PRIORITY …]
/// [ENABLE | DISABLE]` resource-group definition (`sql_yacc.yy` `create_resource_group_stmt`),
/// gated by [`StatementDdlGates::resource_group`](crate::dialect::StatementDdlGates::resource_group).
///
/// `TYPE` is mandatory (a bare `CREATE RESOURCE GROUP g` is `ER_PARSE_ERROR` on mysql:8.4.10) and
/// the option train is fixed-order — `VCPU`, then `THREAD_PRIORITY`, then `ENABLE`/`DISABLE`; any
/// permutation (`… ENABLE VCPU …`) is `ER_PARSE_ERROR`. `CREATE` admits no `FORCE` (`ENABLE FORCE`
/// is `ER_PARSE_ERROR`), unlike [`AlterResourceGroup`] and [`DropResourceGroup`], which share the
/// same [`resource_group`](crate::dialect::StatementDdlGates::resource_group) gate.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateResourceGroup {
    /// The resource-group name (`ident`).
    pub name: Ident,
    /// Whether the `TYPE =` spelling (vs bare `TYPE`) was written (`opt_equal`).
    pub type_equals: bool,
    /// The group type (`SYSTEM`/`USER`), mandatory.
    pub group_type: ResourceGroupType,
    /// The optional `VCPU [=] <ranges>` CPU-affinity clause.
    pub vcpu: Option<ResourceGroupVcpu>,
    /// The optional `THREAD_PRIORITY [=] <n>` clause.
    pub thread_priority: Option<ResourceGroupThreadPriority>,
    /// The optional `ENABLE`/`DISABLE` state clause.
    pub state: Option<ResourceGroupState>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `ALTER RESOURCE GROUP <name> [VCPU …] [THREAD_PRIORITY …] [ENABLE | DISABLE] [FORCE]`
/// resource-group change (`sql_yacc.yy` `alter_resource_group_stmt`), gated by
/// [`StatementDdlGates::resource_group`](crate::dialect::StatementDdlGates::resource_group).
///
/// Every clause is optional — a bare `ALTER RESOURCE GROUP g` grammar-accepts on mysql:8.4.10.
/// The trailing `FORCE` is an *independent* optional (`opt_force`), not a suffix of
/// `ENABLE`/`DISABLE`: `ALTER RESOURCE GROUP g FORCE`, with neither state keyword, grammar-accepts
/// — so [`force`](Self::force) is a peer of [`state`](Self::state), not nested inside it. Shares
/// the [`ResourceGroupVcpu`]/[`ResourceGroupThreadPriority`]/[`ResourceGroupState`] axes with
/// [`CreateResourceGroup`]; `CREATE` differs only in requiring `TYPE` and forbidding `FORCE`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterResourceGroup {
    /// The resource-group name (`ident`).
    pub name: Ident,
    /// The optional `VCPU [=] <ranges>` CPU-affinity clause.
    pub vcpu: Option<ResourceGroupVcpu>,
    /// The optional `THREAD_PRIORITY [=] <n>` clause.
    pub thread_priority: Option<ResourceGroupThreadPriority>,
    /// The optional `ENABLE`/`DISABLE` state clause.
    pub state: Option<ResourceGroupState>,
    /// Whether the trailing `FORCE` was written (`opt_force`, independent of
    /// [`state`](Self::state)).
    pub force: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `DROP RESOURCE GROUP <name> [FORCE]` resource-group drop (`sql_yacc.yy`
/// `drop_resource_group_stmt`), gated by
/// [`StatementDdlGates::resource_group`](crate::dialect::StatementDdlGates::resource_group).
///
/// Removes a single group by name; the optional trailing `FORCE` (`opt_force`) evicts running
/// threads.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DropResourceGroup {
    /// The resource-group name (`ident`).
    pub name: Ident,
    /// Whether the trailing `FORCE` was written (`opt_force`).
    pub force: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `TYPE` of a [`CreateResourceGroup`] (`sql_yacc.yy` `resource_group_types`): the fixed
/// `SYSTEM`/`USER` keyword set. A surface tag (no `meta` — its span rides the owning node); any
/// other word is `ER_PARSE_ERROR`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ResourceGroupType {
    /// `SYSTEM` — a system resource group.
    System,
    /// `USER` — a user resource group.
    User,
}

/// The `ENABLE`/`DISABLE` state clause of a resource-group `CREATE`/`ALTER`
/// (`opt_resource_group_enable_disable`). A surface tag (no `meta` — its span rides the owning
/// node).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ResourceGroupState {
    /// `ENABLE` — enable the group.
    Enable,
    /// `DISABLE` — disable the group.
    Disable,
}

/// The `VCPU [=] <range> [, <range> …]` CPU-affinity clause of a resource-group `CREATE`/`ALTER`
/// (`sql_yacc.yy` `VCPU_SYM opt_equal vcpu_range_spec_list`). The `[=]` presence
/// ([`equals`](Self::equals)) and the non-empty [`ranges`](Self::ranges) list round-trip. The
/// separator between ranges is `opt_comma` (a comma *or* whitespace both parse), so the parse is
/// separator-insensitive and the list is rendered canonically comma-separated.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ResourceGroupVcpu {
    /// Whether the `VCPU =` spelling (vs bare `VCPU`) was written (`opt_equal`).
    pub equals: bool,
    /// The CPU-id range list, in source order; non-empty (`vcpu_range_spec_list`).
    pub ranges: ThinVec<VcpuRange>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `vcpu_num_or_range`: a single CPU id ([`end`](Self::end) `None`) or an inclusive
/// `start-end` range. Each bound is a `NUM` unsigned-integer literal; a value beyond the host CPU
/// count is a post-parse semantic reject (`ER_RESOURCE_GROUP_…`), not a syntax error, so the
/// parser accepts any integer bounds.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct VcpuRange {
    /// The single id, or the inclusive-range start.
    pub start: Literal,
    /// The inclusive-range end (`start-end`), or `None` for a single id.
    pub end: Option<Literal>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `THREAD_PRIORITY [=] <n>` clause of a resource-group `CREATE`/`ALTER`
/// (`opt_resource_group_priority`). The value is a `signed_num` — an optionally negative integer
/// (`THREAD_PRIORITY = -5` grammar-accepts on mysql:8.4.10) — so the sign is carried apart from
/// the magnitude literal (`-` is a separate token). The `[=]` presence and the sign round-trip.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ResourceGroupThreadPriority {
    /// Whether the `THREAD_PRIORITY =` spelling (vs bare `THREAD_PRIORITY`) was written
    /// (`opt_equal`).
    pub equals: bool,
    /// Whether a leading `-` was written (`signed_num`'s negative branch).
    pub negative: bool,
    /// The magnitude — a `NUM` unsigned-integer literal.
    pub value: Literal,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `ALTER {DATABASE | SCHEMA} [<name>] <option> …` schema-option change (`sql_yacc.yy`
/// `alter_database_stmt`), gated by
/// [`StatementDdlGates::alter_database_options`](crate::dialect::StatementDdlGates::alter_database_options).
///
/// A distinct node — and gate — from DuckDB's [`AlterDatabase`] `SET ALIAS` relocation: the two
/// share only the `ALTER DATABASE` head, and their grammars are disjoint (DuckDB requires a name
/// and a single `SET ALIAS TO`; MySQL's name is optional — `ALTER DATABASE CHARACTER SET utf8`
/// binds the default schema — and it takes a non-empty, repeatable list of charset/collation/
/// encryption/read-only [`AlterDatabaseOption`]s). Modelling them apart keeps each behaviour's
/// invariants in the type rather than as XOR-populated fields on one node. The `DATABASE`/
/// `SCHEMA` [`spelling`](Self::spelling) is an exact synonym recorded for round-trip.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterDatabaseOptions {
    /// Which of the synonymous `DATABASE`/`SCHEMA` keywords was written.
    pub spelling: DatabaseKeyword,
    /// The database name, or `None` for the unqualified `ALTER DATABASE <option> …` form that
    /// targets the session's default schema (`ident_or_empty`; a dotted `d.x` is
    /// `ER_PARSE_ERROR`, so a bare [`Ident`]).
    pub name: Option<Ident>,
    /// The option list, in source order; non-empty (`alter_database_options`).
    pub options: ThinVec<AlterDatabaseOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `ALTER {DATABASE | SCHEMA}` option (`sql_yacc.yy` `alter_database_option`). The `[=]`
/// ([`opt_equal`]) and, where admitted, the leading `[DEFAULT]` are recorded so the surface
/// form round-trips verbatim. The value grammars are fixed; a value outside the set is a parse
/// or bind reject on mysql:8.4.10 (`READ ONLY 2` is `ER_PARSE_ERROR`; `ENCRYPTION 'X'` binds
/// then rejects `ER_WRONG_VALUE`, so any string parses).
///
/// [`opt_equal`]: https://dev.mysql.com/doc/refman/8.4/en/alter-database.html
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AlterDatabaseOption {
    /// `[DEFAULT] {CHARACTER SET | CHARSET} [=] <charset>` — the default character set. The
    /// charset name is `charset_name` (`ident_or_text`, or the reserved `BINARY`), folded to
    /// an [`Ident`].
    CharacterSet {
        /// Whether the leading `DEFAULT` was written.
        default: bool,
        /// Which of the `CHARACTER SET`/`CHARSET` synonyms was written.
        keyword: CharsetKeyword,
        /// Whether the optional `=` was written.
        equals: bool,
        /// The character-set name.
        charset: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `[DEFAULT] COLLATE [=] <collation>` — the default collation. The name is
    /// `collation_name` (`ident_or_text`, or the reserved `BINARY`), folded to an [`Ident`].
    Collate {
        /// Whether the leading `DEFAULT` was written.
        default: bool,
        /// Whether the optional `=` was written.
        equals: bool,
        /// The collation name.
        collation: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `[DEFAULT] ENCRYPTION [=] '<Y|N>'` — the default tablespace encryption. Any string
    /// literal parses; the `Y`/`N` restriction is a bind-time check (`ER_WRONG_VALUE`), so the
    /// value is held verbatim as a [`Literal`].
    Encryption {
        /// Whether the leading `DEFAULT` was written.
        default: bool,
        /// Whether the optional `=` was written.
        equals: bool,
        /// The encryption value (a string literal).
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `READ ONLY [=] {DEFAULT | 0 | 1}` — the schema read-only flag. Takes no leading
    /// `DEFAULT` prefix (unlike the charset/collation/encryption options — `DEFAULT READ ONLY`
    /// is `ER_PARSE_ERROR`); the value is a [`ReadOnlyValue`] `ternary_option`, and any other
    /// number (`READ ONLY 2`) is `ER_PARSE_ERROR`.
    ReadOnly {
        /// Whether the optional `=` was written.
        equals: bool,
        /// The read-only value.
        value: ReadOnlyValue,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Which of the synonymous `CHARACTER SET`/`CHARSET` keywords introduced an
/// [`AlterDatabaseOption::CharacterSet`]. A surface tag (no `meta`) recording the spelling for
/// round-trip; the two are semantically identical.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CharsetKeyword {
    /// The two-word `CHARACTER SET` spelling.
    CharacterSet,
    /// The one-word `CHARSET` synonym.
    Charset,
}

/// The value of an [`AlterDatabaseOption::ReadOnly`] — MySQL's `ternary_option`. A surface tag
/// (no `meta`); `0`/`1` are the boolean settings and `DEFAULT` inherits the global default.
/// Any other number is `ER_PARSE_ERROR` on mysql:8.4.10.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ReadOnlyValue {
    /// `DEFAULT` — inherit the global read-only default.
    Default,
    /// `0` — read-write.
    Off,
    /// `1` — read-only.
    On,
}

/// The binary unit suffix on a MySQL storage-DDL size literal (`size_number`'s `IDENT_sys`
/// form): `K`, `M`, or `G`. MySQL folds `<digits><suffix>` to a byte count by shifting the
/// number left 10/20/30 bits, so the tag names the multiplier a downstream converter would
/// otherwise re-decode from the spelling. Case is *not* carried here — the exact source
/// letter (`16m` vs `16M`) round-trips from the enclosing [`SizeLiteral`]'s span.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SizeUnit {
    /// `K` — the value is shifted left 10 bits (× 1024).
    Kilo,
    /// `M` — the value is shifted left 20 bits (× 1024²).
    Mega,
    /// `G` — the value is shifted left 30 bits (× 1024³).
    Giga,
}

/// A MySQL storage-DDL size literal (`size_number`): a byte count written either as a plain
/// integer (`134217728`) or as an integer with a binary unit suffix (`128M`, `2G`, `16k`).
///
/// MySQL lexes the suffixed form as a *single* `IDENT_sys` token (an unquoted identifier may
/// begin with digits), so the digits and the suffix letter must abut with no space — `16 M`
/// is a syntax error. This crate's tokenizer instead splits `16M` into a number token and an
/// adjacent word; the parser rejoins them by span adjacency, so [`meta`](Self::meta) spans the
/// whole literal and renders verbatim. [`unit`](Self::unit) records the multiplier as
/// structured metadata (`None` for a bare byte count); like a [`Literal`], the numeric value
/// and the exact spelling round-trip from the span, so the value carries only the surface tag
/// a consumer cannot otherwise recover.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct SizeLiteral {
    /// The binary unit suffix, or `None` for a bare integer byte count; see [`SizeUnit`].
    pub unit: Option<SizeUnit>,
    /// Source location and node identity. Spans the whole literal (digits + any suffix),
    /// which is what round-trips the exact spelling.
    pub meta: Meta,
}

/// Which storage-DDL option keyword carries a [`SizeLiteral`] value — the `size_number`-valued
/// members of MySQL's shared `ts_option_*` family, grouped here because they share the exact
/// `<KEYWORD> [=] <size>` shape and differ only in the keyword.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TablespaceSizeOption {
    /// `INITIAL_SIZE` — the file's initial size.
    InitialSize,
    /// `AUTOEXTEND_SIZE` — the auto-extension increment.
    AutoextendSize,
    /// `MAX_SIZE` — the file's maximum size.
    MaxSize,
    /// `EXTENT_SIZE` — the extent size.
    ExtentSize,
    /// `UNDO_BUFFER_SIZE` — the UNDO buffer size (logfile group).
    UndoBufferSize,
    /// `REDO_BUFFER_SIZE` — the REDO buffer size (logfile group).
    RedoBufferSize,
    /// `FILE_BLOCK_SIZE` — the file block size (tablespace).
    FileBlockSize,
}

/// One option in a MySQL tablespace / logfile-group statement — a member of the server's shared
/// `ts_option_*` family (every one of these DDL statements builds one
/// `PT_alter_tablespace_option` list). The full universe is modelled here as one axis; each
/// statement context accepts only its own subset (e.g. `UNDO TABLESPACE` takes `ENGINE` alone,
/// `ALTER TABLESPACE` excludes `FILE_BLOCK_SIZE`), enforced by the parser rather than the type,
/// so an out-of-context option ends the list and surfaces as a clean parse error — matching the
/// live server's per-context grammar.
///
/// Non-generic: every value is a size literal, an integer/string [`Literal`], an [`Ident`], or a
/// flag — no expressions or extension nodes. Each option records whether its optional `=`
/// (`opt_equal`) was written so the surface form round-trips.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TablespaceOption {
    /// A `size_number`-valued option (`INITIAL_SIZE [=] 128M`, …); see [`TablespaceSizeOption`].
    Size {
        /// Which size option keyword this is.
        kind: TablespaceSizeOption,
        /// Whether the optional `=` was written between the keyword and the value.
        equals: bool,
        /// The size value.
        size: SizeLiteral,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `NODEGROUP [=] <n>` — a plain integer node-group id (`real_ulong_num`, never suffixed).
    Nodegroup {
        /// Whether the optional `=` was written between the keyword and the value.
        equals: bool,
        /// The node-group id.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `[STORAGE] ENGINE [=] <name>` — the storage engine (`ident_or_text`: a bare identifier or
    /// a quoted string, folded to an [`Ident`] whose quote style round-trips).
    Engine {
        /// Whether the optional leading `STORAGE` noise keyword was written (`opt_storage`).
        storage: bool,
        /// Whether the optional `=` was written between `ENGINE` and the value.
        equals: bool,
        /// The engine name.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `WAIT` or `NO_WAIT` — the NDB completion-wait flag (`ts_option_wait`).
    Wait {
        /// `true` for `NO_WAIT`, `false` for `WAIT`.
        negated: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `COMMENT [=] '<text>'` — a free-text comment string.
    Comment {
        /// Whether the optional `=` was written between the keyword and the value.
        equals: bool,
        /// The comment string.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ENCRYPTION [=] '<y_or_n>'` — the tablespace encryption flag string (`'Y'`/`'N'`).
    Encryption {
        /// Whether the optional `=` was written between the keyword and the value.
        equals: bool,
        /// The encryption flag string.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ENGINE_ATTRIBUTE [=] '<json>'` — the engine-specific JSON attribute string.
    EngineAttribute {
        /// Whether the optional `=` was written between the keyword and the value.
        equals: bool,
        /// The JSON attribute string.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL `CREATE [UNDO] TABLESPACE <name> [ADD DATAFILE '<f>'] [USE LOGFILE GROUP <lg>]
/// [<option>...]` statement (gated by
/// [`StatementDdlGates::tablespace_ddl`](crate::dialect::StatementDdlGates::tablespace_ddl)).
///
/// One node for both the InnoDB/NDB tablespace (`Sql_cmd_create_tablespace`) and the
/// `UNDO`-tablespace variant (`Sql_cmd_create_undo_tablespace`), distinguished by
/// [`undo`](Self::undo). The two differ only in the leading `UNDO` keyword, whether the datafile
/// is mandatory (it is for `UNDO`, optional otherwise), and the accepted option set (`UNDO`
/// admits `ENGINE` alone) — all enforced by the parser. The `USE LOGFILE GROUP` clause is
/// NDB-only and never appears on the `UNDO` form.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateTablespace {
    /// Whether this is the `CREATE UNDO TABLESPACE` variant.
    pub undo: bool,
    /// The tablespace name.
    pub name: Ident,
    /// `ADD DATAFILE '<file>'`. Mandatory (`Some`) for the `UNDO` form; optional otherwise.
    pub datafile: Option<Literal>,
    /// `USE LOGFILE GROUP <name>` (NDB, regular tablespace only).
    pub use_logfile_group: Option<Ident>,
    /// Options supplied in source order.
    pub options: ThinVec<TablespaceOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The undo-tablespace access state set by `ALTER UNDO TABLESPACE <name> SET {ACTIVE | INACTIVE}`
/// (`undo_tablespace_state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum UndoTablespaceState {
    /// `SET ACTIVE`.
    Active,
    /// `SET INACTIVE`.
    Inactive,
}

/// A MySQL `ALTER [UNDO] TABLESPACE <name> <action>` statement (gated by
/// [`StatementDdlGates::tablespace_ddl`](crate::dialect::StatementDdlGates::tablespace_ddl)).
///
/// The four regular productions (`ADD`/`DROP DATAFILE`, `RENAME TO`, a bare option list) and the
/// `UNDO` production (`SET {ACTIVE | INACTIVE}`) share the `ALTER [UNDO] TABLESPACE <name>` head
/// and are unified under [`AlterTablespaceAction`]. The action itself distinguishes the `UNDO`
/// form (only it carries [`SetState`](AlterTablespaceAction::SetState)), so no separate `undo`
/// flag is needed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterTablespace {
    /// The tablespace name.
    pub name: Ident,
    /// The change applied to the tablespace.
    pub action: AlterTablespaceAction,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The change an [`AlterTablespace`] applies.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AlterTablespaceAction {
    /// `ADD DATAFILE '<f>' [<option>...]`.
    AddDatafile {
        /// The datafile path.
        datafile: Literal,
        /// The trailing alter-tablespace options in source order.
        options: ThinVec<TablespaceOption>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DROP DATAFILE '<f>' [<option>...]`.
    DropDatafile {
        /// The datafile path.
        datafile: Literal,
        /// The trailing alter-tablespace options in source order.
        options: ThinVec<TablespaceOption>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RENAME TO <new_name>` — takes no options (a trailing option is a parse error).
    Rename {
        /// The new tablespace name.
        new_name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bare (non-empty) option list — `ALTER TABLESPACE <name> <option> [, <option>]...`
    /// with no `ADD`/`DROP`/`RENAME` lead.
    Options {
        /// The options in source order (at least one).
        options: ThinVec<TablespaceOption>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET {ACTIVE | INACTIVE} [<option>...]` — the `ALTER UNDO TABLESPACE` form (the trailing
    /// options are the `UNDO` subset, `ENGINE` alone).
    SetState {
        /// The new access state.
        state: UndoTablespaceState,
        /// The trailing undo-tablespace options in source order.
        options: ThinVec<TablespaceOption>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL `DROP [UNDO] TABLESPACE <name> [<option>...]` statement (gated by
/// [`StatementDdlGates::tablespace_ddl`](crate::dialect::StatementDdlGates::tablespace_ddl)).
///
/// One node for `DROP TABLESPACE` (`Sql_cmd_drop_tablespace`) and `DROP UNDO TABLESPACE`
/// (`Sql_cmd_drop_undo_tablespace`), distinguished by [`undo`](Self::undo): the regular form
/// accepts `ENGINE`/`WAIT` options, the `UNDO` form `ENGINE` alone (parser-enforced).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DropTablespace {
    /// Whether this is the `DROP UNDO TABLESPACE` variant.
    pub undo: bool,
    /// The tablespace name.
    pub name: Ident,
    /// Options supplied in source order.
    pub options: ThinVec<TablespaceOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `CREATE LOGFILE GROUP <name> ADD UNDOFILE '<f>' [<option>...]` statement (NDB; gated
/// by
/// [`StatementDdlGates::logfile_group_ddl`](crate::dialect::StatementDdlGates::logfile_group_ddl)).
///
/// The `ADD UNDOFILE` clause is mandatory (a bare `CREATE LOGFILE GROUP <name>` is a syntax
/// error). The option set is the logfile-group subset of the shared `ts_option_*` family
/// (`INITIAL_SIZE`, `UNDO_BUFFER_SIZE`, `REDO_BUFFER_SIZE`, `NODEGROUP`, `ENGINE`, `WAIT`,
/// `COMMENT`), parser-enforced.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateLogfileGroup {
    /// The logfile-group name.
    pub name: Ident,
    /// The mandatory `ADD UNDOFILE '<file>'` path.
    pub undofile: Literal,
    /// Options supplied in source order.
    pub options: ThinVec<TablespaceOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `ALTER LOGFILE GROUP <name> ADD UNDOFILE '<f>' [<option>...]` statement (NDB; gated by
/// [`StatementDdlGates::logfile_group_ddl`](crate::dialect::StatementDdlGates::logfile_group_ddl)).
///
/// Like [`CreateLogfileGroup`] the `ADD UNDOFILE` clause is mandatory; the accepted option set is
/// the narrower alter subset (`INITIAL_SIZE`, `ENGINE`, `WAIT`), parser-enforced.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterLogfileGroup {
    /// The logfile-group name.
    pub name: Ident,
    /// The mandatory `ADD UNDOFILE '<file>'` path.
    pub undofile: Literal,
    /// Options supplied in source order.
    pub options: ThinVec<TablespaceOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A MySQL `DROP LOGFILE GROUP <name> [<option>...]` statement (NDB; gated by
/// [`StatementDdlGates::logfile_group_ddl`](crate::dialect::StatementDdlGates::logfile_group_ddl)).
///
/// The option set is `ENGINE`/`WAIT` (`opt_drop_ts_options`, shared with `DROP TABLESPACE`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DropLogfileGroup {
    /// The logfile-group name.
    pub name: Ident,
    /// Options supplied in source order.
    pub options: ThinVec<TablespaceOption>,
    /// Source location and node identity.
    pub meta: Meta,
}
