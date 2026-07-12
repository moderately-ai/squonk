// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! BigQuery / ZetaSQL **query pipe syntax** — the `|>` operators that post-process a
//! query result, one operator at a time (`FROM t |> WHERE x > 1 |> SELECT a, b`).
//!
//! # What this models
//!
//! A [`Query`] carries a trailing
//! [`pipe_operators`](super::Query::pipe_operators) list — each element one `|>`
//! operator applied left to right to the preceding query's result. The list is empty
//! for an ordinary query (the common case, so the field is a bare `ThinVec` that costs
//! one null pointer when unused, mirroring [`Query::locking`](super::Query)). This is
//! deliberate parity with `sqlparser-rs`'s `Query.pipe_operators: Vec<PipeOperator>`.
//!
//! # NOT the `||` operator
//!
//! This [`PipeOperator`] enum is unrelated to
//! [`dialect::PipeOperator`](crate::dialect::PipeOperator), which is the *meaning* tag
//! for the `||` token (string-concat vs logical-OR). The names collide only across
//! modules; nothing imports both by bare name. This file's `PipeOperator` is a query
//! **AST node** keyed on the `|>` (pipe-arrow) separator, a distinct token
//! (`Operator::PipeArrow`, defined in the tokenizer crate).
//!
//! # The `|>` separator (lexing)
//!
//! `|>` is a two-byte token munched in the tokenizer's `|` arm, ahead of a lone `|`
//! and beside `||`, **gated on
//! [`QueryTailSyntax::pipe_syntax`](crate::dialect::SelectSyntax)**. With the gate off
//! (every shipped preset today) the two bytes stay `|` then `>` exactly as before, so
//! no dialect's lexing shifts — the same feature-gated maximal-munch idiom the parser
//! uses for `->`/`<=>`/`//`. The PostgreSQL geometric operator `|>>` ("is strictly
//! above") is **not** lexed by this parser (it has no geometric-operator support). If
//! geometric operators are added, they need their own dialect gate; no shipped preset
//! enables both pipe syntax and PG geometry, so the two munches never contend under one
//! `FeatureSet`.
//!
use super::{
    Expr, Extension, FunctionCall, Ident, Join, NoExt, OrderByExpr, PivotColumn, PivotExpr, Query,
    SelectItem, SetOperator, SetQuantifier, TableAlias, TableSample, ThinVec, UnpivotColumn,
    UpdateAssignment,
};
use crate::vocab::Meta;

/// One BigQuery/ZetaSQL `|>` pipe operator applied to a query result.
///
/// Each variant is one `|> <KEYWORD> …` step; a [`Query`] holds a list of
/// them applied left to right. Generic over the extension parameter `X` because most
/// operators carry expressions.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PipeOperator<X: Extension = NoExt> {
    /// `|> WHERE <predicate>` — keep only the rows for which `predicate` holds.
    ///
    /// The framework's reference operator: the minimal single-expression body, mirroring
    /// an ordinary `WHERE` clause's one predicate. Its parse/render/test wiring is the
    /// template every later operator arm follows.
    Where {
        /// Predicate that controls this clause.
        predicate: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> SELECT <select-items>` — replace the current row shape with a new projection.
    ///
    /// Reuses [`SelectItem`], the ordinary `SELECT`-list item shape (an
    /// expression with an optional alias, or a `*` / `t.*` wildcard), so a pipe projection
    /// and a leading `SELECT` list carry identical nodes. ZetaSQL's `|> SELECT` also admits
    /// the `DISTINCT` and `AS STRUCT | VALUE` modifiers; those modifiers are intentionally
    /// absent from this item-only shape.
    Select {
        /// Child items in source order.
        items: ThinVec<SelectItem<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> EXTEND <select-items>` — append computed columns to the current row shape.
    ///
    /// The same operand shape as [`Select`](Self::Select) ([`SelectItem`]):
    /// the difference is semantic — `EXTEND` keeps the existing columns and adds these, where
    /// `SELECT` replaces them — not structural, so the two share the item node rather than a
    /// parallel copy.
    Extend {
        /// Child items in source order.
        items: ThinVec<SelectItem<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> AS <alias>` — bind a range-variable name (table alias) to the current result.
    ///
    /// Reuses [`TableAlias`], the correlation-name shape a `FROM` item
    /// carries. ZetaSQL's `|> AS` names only the range variable — it has no column-alias list
    /// — so the reused node always carries an empty
    /// [`columns`](super::TableAlias::columns): the parser never populates it, and a trailing
    /// `(a, b)` is left unconsumed for the caller to reject.
    As {
        /// Alias assigned by this syntax.
        alias: TableAlias,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> ORDER BY <sort-keys>` — sort the current result.
    ///
    /// Reuses [`OrderByExpr`], the ordinary query `ORDER BY` sort-key
    /// shape (`<expr> [ASC | DESC | USING <op>] [NULLS FIRST | LAST]`), so a pipe sort and a
    /// query-tail sort carry identical keys.
    OrderBy {
        /// keys in source order.
        keys: ThinVec<OrderByExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> LIMIT <count> [OFFSET <skip>]` — bound the current result to `count` rows,
    /// optionally skipping the first `skip`.
    ///
    /// The narrow ZetaSQL pipe form: a required row-count expression and an optional `OFFSET`
    /// skip expression, both restricted to the ordinary `LIMIT`-operand grammar (an integer
    /// literal or a `?` placeholder). It deliberately does *not* reuse the full
    /// [`Limit`](super::Limit) clause node — the pipe form has no `FETCH FIRST`, `PERCENT`,
    /// `WITH TIES`, or MySQL `LIMIT a, b` spelling — so it models only the two operands the
    /// surface carries rather than widening onto the clause shape.
    ///
    /// The two [`Expr`] operands are boxed (unlike the reference [`Where`](Self::Where)'s
    /// single inline predicate): a two-`Expr` inline payload would be the lone fat variant of
    /// an otherwise-small enum whose common variants hold a one-word `ThinVec`, so ADR-0007's
    /// skew rule boxes it to keep [`PipeOperator`] lean — the allocation is paid only on the
    /// rare pipe-`LIMIT` path.
    Limit {
        /// The `LIMIT` row count.
        count: Box<Expr<X>>,
        /// Row offset applied before returning results.
        offset: Option<Box<Expr<X>>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> [<join-type>] JOIN <relation> [ON <predicate> | USING (<cols>)]` — join the
    /// current result to another relation.
    ///
    /// Reuses the ordinary [`Join`] node wholesale (its
    /// [`relation`](super::Join::relation) table factor, its
    /// [`operator`](super::Join::operator) side/kind spelling, and the embedded
    /// `ON`/`USING` constraint), so a pipe join and a `FROM`-clause join carry identical
    /// nodes — the pipe form admits exactly the join grammar the dialect's join parser
    /// admits, nothing invented here. The whole [`Join`] is boxed (it is a 176-byte node,
    /// far past [`PipeOperator`]'s 56-byte budget); the allocation is paid only on the
    /// pipe-join path, exactly as [`Limit`](Self::Limit) boxes its operands (ADR-0007).
    Join {
        /// The reused join node — relation, operator, and constraint; see [`Join`].
        join: Box<Join<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> {UNION | INTERSECT | EXCEPT} [ALL | DISTINCT] (<query>) [, (<query>) …]` —
    /// combine the current result with one or more parenthesized queries under a set
    /// operation.
    ///
    /// One variant carries the [`SetOperator`] tag rather than three
    /// operator-named variants — reusing the same `op: SetOperator` shape the ordinary
    /// query set operation ([`SetExpr::SetOperation`](super::SetExpr)) uses, so the three
    /// pipe set operations share this node exactly as the three query set operations share
    /// theirs. The [`quantifier`](Self::SetOperation::quantifier) is
    /// [`Option`]al — `None` renders the bare operator, `Some` the explicit `ALL`/`DISTINCT`
    /// — capturing all three surfaces for an exact round-trip (unlike the query set op's
    /// `all: bool`, which cannot distinguish a bare operator from an explicit `DISTINCT`).
    /// The operand [`queries`](Self::SetOperation::queries) live in the `ThinVec`'s heap
    /// buffer, so this variant costs only that one-word pointer inline.
    SetOperation {
        /// Operator applied by this expression.
        op: SetOperator,
        /// Optional quantifier for this syntax.
        quantifier: Option<SetQuantifier>,
        /// queries in source order.
        queries: ThinVec<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> SET <column> = <expr> [, …]` — replace named columns of the current row shape
    /// with new expressions.
    ///
    /// Reuses [`UpdateAssignment`], the `UPDATE … SET` assignment
    /// node, but the parser builds only its [`Single`](super::UpdateAssignment::Single)
    /// `<column> = <expr>` form: the ZetaSQL pipe surface has no multiple-column
    /// `( … ) = <source>` tuple assignment and no `DEFAULT` right-hand side, so those
    /// arms of the reused node are never populated here (one shape per construct, no
    /// parallel copy).
    Set {
        /// assignments in source order.
        assignments: ThinVec<UpdateAssignment<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> CALL <function>(<args>) [AS <alias>]` — apply a table-valued function to the
    /// current result, optionally binding a range-variable name to the output.
    ///
    /// Reuses [`FunctionCall`], the ordinary call node, for
    /// `<function>(<args>)` (boxed — it is a 120-byte node, past the budget). The optional
    /// trailing alias reuses [`TableAlias`] name-only, exactly as
    /// [`As`](Self::As) does: the pipe `AS <alias>` names a range variable with no
    /// column-alias list, so [`columns`](super::TableAlias::columns) is always empty and a
    /// trailing `(a, b)` is left to reject. The alias is boxed because the variant already
    /// carries the boxed call (an inline [`TableAlias`] would push it past the budget).
    Call {
        /// The table-function call applied to the pipe input; see [`FunctionCall`].
        call: Box<FunctionCall<X>>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> AGGREGATE <aggregates> [GROUP BY <keys>]` — collapse the current result into
    /// aggregate rows, optionally grouped by the trailing keys.
    ///
    /// Both lists carry [`PipeAggregateExpr`] — an expression with an optional output
    /// alias *and* an optional `ASC`/`DESC` + `NULLS FIRST`/`LAST` ordering suffix,
    /// matching `sqlparser-rs`'s single `ExprWithAliasAndOrderBy` shape for the two
    /// lists (its `full_table_exprs` / `group_by_expr`). ZetaSQL's pipe `AGGREGATE` folds
    /// grouping and the aggregate-driven "GROUP AND ORDER BY" ordering into one operator,
    /// so an aggregate or a grouping key may each name its own sort direction; a
    /// dedicated combined item is used because neither [`SelectItem`]
    /// (alias, no ordering) nor [`GroupByItem`](super::GroupByItem) (grouping semantics,
    /// no alias/ordering) carries both. The [`aggregates`](Self::Aggregate::aggregates)
    /// list is empty for a grouping-only `|> AGGREGATE GROUP BY x`; the
    /// [`group_by`](Self::Aggregate::group_by) list is empty when no `GROUP BY` is
    /// written. The `GROUP AND ORDER BY` keyword spelling and the bare (`AS`-less) item
    /// alias are intentionally absent from this shape.
    Aggregate {
        /// aggregates in source order.
        aggregates: ThinVec<PipeAggregateExpr<X>>,
        /// Grouping terms in source order.
        group_by: ThinVec<PipeAggregateExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> DROP <column> [, …]` — remove named columns from the current row shape.
    ///
    /// The columns are a bare [`Ident`] list, not
    /// [`ObjectName`](super::ObjectName)s: ZetaSQL's `DROP` names output columns of the
    /// current table by their unqualified name, so a qualified `t.c` has no meaning here.
    Drop {
        /// Columns in source order.
        columns: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> RENAME <old> AS <new> [, …]` — rename columns of the current row shape.
    ///
    /// Each mapping is a [`PipeRenameItem`] pair of bare identifiers, kept distinct from a
    /// projection alias ([`SelectItem::Expr`](super::SelectItem)'s `alias`): a rename
    /// keeps the column's value and changes only its name, where a projection alias names
    /// a computed item. A dedicated two-[`Ident`] shape is used rather than
    /// the DuckDB wildcard-modifier [`WildcardRename`](super::WildcardRename), whose source
    /// is a qualifiable [`ObjectName`](super::ObjectName) and which belongs to the `*
    /// RENAME (…)` surface — reusing it here would both over-accept a qualified source and
    /// conflate two unrelated features.
    Rename {
        /// renames in source order.
        renames: ThinVec<PipeRenameItem>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> PIVOT (<aggregates> FOR <column> IN (<values>))` — rotate the distinct values
    /// of one pivot column into columns, aggregating each cell.
    ///
    /// Reuses the shared pivot sub-shapes — [`PivotExpr`] for the
    /// aggregate list and [`PivotColumn`] for the single `FOR <column>
    /// IN (<values>)` head — rather than the whole [`Pivot`](super::Pivot) node: the pipe
    /// operator has no `source` relation (it pivots the pipe input) and none of the
    /// statement-only `WITH`/`ORDER BY`/`LIMIT` tail, so it carries only the parenthesized
    /// body's two operands, keeping the table-factor `PIVOT` and the pipe `PIVOT`
    /// unconflated. Exactly one `FOR` column (ZetaSQL admits no second head), and the
    /// aggregate list is non-empty. The [`column`](Self::Pivot::column) is boxed — it
    /// carries an [`Expr`] and would otherwise widen the variant past the
    /// budget — while the aggregates ride the `ThinVec`'s heap buffer.
    Pivot {
        /// aggregates in source order.
        aggregates: ThinVec<PivotExpr<X>>,
        /// Column referenced by this syntax.
        column: Box<PivotColumn<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> UNPIVOT (<value> FOR <name> IN (<columns>))` — collapse a set of columns into
    /// `name`/`value` row pairs (the inverse of [`Pivot`](Self::Pivot)).
    ///
    /// Reuses the shared [`UnpivotColumn`] sub-shape for the `IN`
    /// list and mirrors the [`Unpivot`](super::Unpivot) core's `value`/`name` identifier
    /// lists (each a `ThinVec<Ident>` so the multi-column `(v1, v2) FOR n IN ((a, b), (c,
    /// d))` surface reuses one shape), rather than the whole `Unpivot` node: as with
    /// [`Pivot`](Self::Pivot) the pipe operator has no `source` and no statement tail, so
    /// it carries only the parenthesized body. The `INCLUDE`/`EXCLUDE NULLS` marker is
    /// intentionally absent from this shape.
    Unpivot {
        /// The output *value* column name(s) (`<value>` before `FOR`); one for the common
        /// form, several for a multi-column unpivot.
        value: ThinVec<Ident>,
        /// The output *name* column name(s) (`<name>` after `FOR`); one in the common
        /// form.
        name: ThinVec<Ident>,
        /// Columns in source order.
        columns: ThinVec<UnpivotColumn<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `|> TABLESAMPLE <method> (<args>) [REPEATABLE (<seed>)]` — sample the current
    /// result.
    ///
    /// Reuses the whole [`TableSample`] node (the `FROM`-clause
    /// sampling suffix), boxed — it is a wide node past the budget, and the allocation is
    /// paid only on the pipe-sample path, exactly as [`Join`](Self::Join) boxes its reused
    /// node. The pipe form admits precisely the argument grammar the table-factor
    /// `TABLESAMPLE` admits (a bare numeric percentage rather than the BigQuery
    /// `PERCENT`/`ROWS` unit keyword, which the reused shape has no field for).
    TableSample {
        /// The `TABLESAMPLE` clause; see [`TableSample`].
        sample: Box<TableSample<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One item of a pipe [`AGGREGATE`](PipeOperator::Aggregate) operator's aggregate list or
/// `GROUP BY` list: an expression with an optional output alias and an optional
/// `ASC`/`DESC` + `NULLS FIRST`/`LAST` ordering suffix.
///
/// The single shape both lists share, matching `sqlparser-rs`'s
/// `ExprWithAliasAndOrderBy` (an aliased expression plus `OrderByOptions`): ZetaSQL's
/// pipe `AGGREGATE` lets an aggregate drive output ordering and lets a grouping key carry
/// its own direction, so the alias and the ordering co-occur on one item. Modelled
/// separately from [`OrderByExpr`] (which has no alias and carries the
/// PostgreSQL `USING <op>` sort form the pipe operator has no grammar for) and from
/// [`SelectItem`] (which carries no ordering).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PipeAggregateExpr<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Expr<X>,
    /// Alias assigned by this syntax.
    pub alias: Option<Ident>,
    /// `ASC` (`Some(true)`) / `DESC` (`Some(false)`), or `None` when unwritten — the
    /// ordering direction, mirroring [`OrderByExpr::asc`](super::OrderByExpr).
    pub asc: Option<bool>,
    /// `NULLS FIRST` (`Some(true)`) / `NULLS LAST` (`Some(false)`), or `None` when
    /// unwritten, mirroring [`OrderByExpr::nulls_first`](super::OrderByExpr).
    pub nulls_first: Option<bool>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `<old> AS <new>` mapping of a pipe [`RENAME`](PipeOperator::Rename) operator: the
/// source column [`old`](Self::old) renamed to the output name [`new`](Self::new).
///
/// Both sides are bare [`Ident`]s — the pipe `RENAME` renames output columns
/// of the current table by their unqualified name — which is what keeps a rename distinct
/// from both a projection alias and the qualifiable DuckDB wildcard
/// [`WildcardRename`](super::WildcardRename).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PipeRenameItem {
    /// The source column being renamed.
    pub old: Ident,
    /// The new output column name.
    pub new: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}
