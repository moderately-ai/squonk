// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! DuckDB `PIVOT` / `UNPIVOT` — the row-to-column (and inverse) relational operators.
//!
//! DuckDB exposes each operator through *two* surfaces that build the same operator:
//! the leading-keyword **statement** (`PIVOT t ON year USING sum(x) GROUP BY city`,
//! `UNPIVOT t ON a, b INTO NAME n VALUE v`) and the SQL-standard **table factor**
//! written after a relation in `FROM` (`t PIVOT (sum(x) FOR year IN (2000, 2010))`,
//! `t UNPIVOT (v FOR n IN (a, b))`). The two spellings canonicalize onto
//! one shape per operator — [`Pivot`] and [`Unpivot`] — carrying a [`PivotSpelling`] /
//! [`UnpivotSpelling`] tag so rendering reproduces the written surface without a second
//! node. Each core is hosted in both positions:
//! [`Statement::Pivot`](crate::ast::Statement)/`Unpivot` (dispatched as a top-level
//! statement, DuckDB's `PivotStatement`) and
//! [`TableFactor::Pivot`](crate::ast::TableFactor)/`Unpivot` (the `FROM`-suffix form),
//! sharing the operator fields while the table-factor position owns the trailing alias.
//!
//! Both operators are gated on
//! [`TableFactorSyntax::pivot`](crate::dialect::TableExpressionSyntax) /
//! [`unpivot`](crate::dialect::TableExpressionSyntax) (DuckDb/Lenient) and rest on the
//! reservation of `PIVOT`/`UNPIVOT` (DuckDB `duckdb_keywords()` class `reserved`, like
//! `QUALIFY`) — without it a trailing `PIVOT` would be swallowed as the source's alias.

use super::{
    AliasSpelling, Expr, Extension, Ident, Limit, NoExt, OrderByAll, OrderByExpr, Query,
    TableFactor, With,
};
use crate::vocab::Meta;
use thin_vec::ThinVec;

/// DuckDB's `PIVOT` operator: rotate the distinct values of the pivot column(s) into
/// columns, aggregating each cell.
///
/// One canonical shape for both DuckDB surfaces; [`spelling`](Self::spelling)
/// records which was written so it round-trips. The fields cover both:
///
/// - the statement `PIVOT <source> [ON <pivot_on>] [USING <aggregates>] [GROUP BY
///   <group_by>]` — any of `ON`/`USING`/`GROUP BY` may be absent, and an `ON` entry may
///   carry an inline `IN (...)` value list (`ON year IN (2000, 2010)`) or none
///   (auto-detected at bind time);
/// - the table factor `<source> PIVOT (<aggregates> FOR <col> IN (…) [<col> IN (…)]…
///   [GROUP BY <group_by>])` — one `FOR` keyword heading one or more column heads
///   (the extra heads are written bare; a second `FOR` is an engine syntax error),
///   each with a required `IN` source, and at least one aggregate (DuckDB
///   syntax-rejects an empty aggregate list here; both probed on 1.5.4).
///
/// `source` is boxed to break the `TableFactor` → `Pivot` → `TableFactor` type cycle
/// (`FROM (SELECT …) PIVOT (…)` nests a derived table; a bare `PIVOT (SELECT …) ON …`
/// nests one too). The source keeps its own correlation alias; the *pivot's* alias
/// (`… PIVOT (…) AS p`) belongs to the enclosing
/// [`TableFactor::Pivot`](crate::ast::TableFactor), never to a statement.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Pivot<X: Extension = NoExt> {
    /// Input source for this syntax.
    pub source: Box<TableFactor<X>>,
    /// The `USING` aggregate list (statement) / the leading `(<aggregates> FOR …)` list
    /// (table factor); each an expression with an optional output-name alias. Empty for
    /// the aggregate-less statement forms (`PIVOT t ON year`, `PIVOT t GROUP BY city`).
    pub aggregates: ThinVec<PivotExpr<X>>,
    /// The pivot column(s): `ON <col>, …` (statement, one entry per comma item, each
    /// with an optional inline `IN (...)`) or the `FOR`-headed column list (table
    /// factor). Empty when the statement writes no `ON`.
    pub pivot_on: ThinVec<PivotColumn<X>>,
    /// Grouping terms in source order.
    pub group_by: ThinVec<Expr<X>>,
    /// The `WITH` clause prefixing a *statement*-spelled pivot (`WITH c AS (…) PIVOT c
    /// ON …`): DuckDB attaches the CTEs to the pivot statement itself, exactly as it
    /// does for INSERT/UPDATE/DELETE. Always `None` in the table-factor spelling, whose
    /// enclosing query owns any `WITH`.
    pub with: Option<With<X>>,
    /// The statement form's trailing `ORDER BY` keys (`PIVOT t USING sum(x) ORDER BY
    /// c`, engine-verified); always empty in the table-factor spelling, where ordering
    /// belongs to the enclosing SELECT.
    pub order_by: ThinVec<OrderByExpr<X>>,
    /// The statement form's `ORDER BY ALL` clause mode (engine-verified) — mutually
    /// exclusive with a non-empty [`order_by`](Self::order_by), exactly as on
    /// [`Query`]. Always `None` in the table-factor spelling.
    pub order_by_all: Option<Box<OrderByAll>>,
    /// The statement form's trailing `LIMIT`/`OFFSET` tail (engine-verified); boxed
    /// because the tail is rare while [`Limit`] is wide. Always `None` in
    /// the table-factor spelling.
    pub limit: Option<Box<Limit<X>>>,
    /// The Snowflake table-factor `DEFAULT ON NULL (<expr>)` tail — the value
    /// substituted for a pivoted cell that would otherwise be `NULL`
    /// (`PIVOT (sum(x) FOR y IN (…) DEFAULT ON NULL (0))`). Gated on
    /// [`TableFactorSyntax::pivot_value_sources`](crate::dialect::TableFactorSyntax);
    /// `None` in the DuckDB surfaces (which have no such clause) and in any dialect
    /// off the gate. Boxed because the clause is rare while [`Expr`] is wide.
    pub default_on_null: Option<Box<Expr<X>>>,
    /// Which surface produced this node — drives rendering.
    pub spelling: PivotSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `UNPIVOT` operator: collapse a set of columns into `NAME`/`VALUE` row pairs
/// (the inverse of [`Pivot`]).
///
/// One canonical shape for both surfaces; [`spelling`](Self::spelling)
/// records which was written. The fields cover both:
///
/// - the DuckDB statement `UNPIVOT <source> ON <columns> [INTO NAME <name> VALUE
///   <value>]` — the `INTO` clause renames the output name/value columns (absent leaves
///   DuckDB's default `name`/`value`);
/// - the table factor `<source> UNPIVOT [INCLUDE|EXCLUDE NULLS] (<value> FOR <name> IN
///   (<columns>))` — the shared DuckDB/BigQuery/Snowflake surface (the standard is
///   reachable off the DuckDB [`unpivot`](crate::dialect::TableFactorSyntax::unpivot)
///   flag under
///   [`pivot_value_sources`](crate::dialect::TableFactorSyntax::pivot_value_sources)).
///   The `NULLS` marker is table-factor-only (the statement form rejects it, so
///   [`null_inclusion`](Self::null_inclusion) is always `None` there).
///
/// [`value`](Self::value) and [`name`](Self::name) are lists because DuckDB admits a
/// multi-column unpivot (`(v1, v2) FOR n IN ((a, b), (c, d))`), whose value name list
/// has more than one entry; the common form fills each with a single [`Ident`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Unpivot<X: Extension = NoExt> {
    /// Input source for this syntax.
    pub source: Box<TableFactor<X>>,
    /// The output *value* column name(s) (`INTO … VALUE v` / the `v` in `(v FOR n IN
    /// …)`); empty when a statement writes no `INTO`.
    pub value: ThinVec<Ident>,
    /// The output *name* column name(s) (`INTO NAME n …` / the `n` in `(v FOR n IN …)`);
    /// empty when a statement writes no `INTO`.
    pub name: ThinVec<Ident>,
    /// The columns being unpivoted: `ON <col>, …` (statement) / the `IN (<cols>)` list
    /// (table factor). Each entry is one or more column expressions (a grouped
    /// `(a, b)`) with an optional alias.
    pub columns: ThinVec<UnpivotColumn<X>>,
    /// The explicit `INCLUDE NULLS` / `EXCLUDE NULLS` marker (table factor only). `None`
    /// is the unwritten default (`EXCLUDE NULLS` semantics, rendered bare); `Some` records
    /// a written marker so it round-trips — including an explicit `EXCLUDE NULLS`, which
    /// would otherwise elide to the default. Always `None` in the statement spelling,
    /// which rejects the marker.
    pub null_inclusion: Option<NullInclusion>,
    /// The `WITH` clause prefixing a *statement*-spelled unpivot; always `None` in the
    /// table-factor spelling (the [`Pivot::with`] mirror).
    pub with: Option<With<X>>,
    /// The statement form's trailing `ORDER BY` keys; always empty in the table-factor
    /// spelling (the [`Pivot::order_by`] mirror).
    pub order_by: ThinVec<OrderByExpr<X>>,
    /// The statement form's `ORDER BY ALL` clause mode (the [`Pivot::order_by_all`]
    /// mirror); always `None` in the table-factor spelling.
    pub order_by_all: Option<Box<OrderByAll>>,
    /// The statement form's trailing `LIMIT`/`OFFSET` tail; always `None` in the
    /// table-factor spelling (the [`Pivot::limit`] mirror).
    pub limit: Option<Box<Limit<X>>>,
    /// Which surface produced this node — drives rendering.
    pub spelling: UnpivotSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A table-factor `UNPIVOT`'s explicit null-row treatment — the `INCLUDE NULLS` /
/// `EXCLUDE NULLS` marker shared by DuckDB, BigQuery, and Snowflake.
///
/// `EXCLUDE NULLS` is every engine's default, so [`Unpivot::null_inclusion`] wraps this
/// in an `Option`: `None` is the unwritten default and each variant records the marker as
/// written so it round-trips (an explicit `EXCLUDE NULLS` is preserved rather than elided
/// to the bare default). A fieldless presence tag — the [`UnpivotSpelling`] precedent, and
/// the direct sqlparser-rs `NullInclusion` parity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum NullInclusion {
    /// `INCLUDE NULLS` — keep output rows whose unpivoted value is `NULL`.
    IncludeNulls,
    /// `EXCLUDE NULLS` — drop null-valued rows (every engine's default; recorded only
    /// when written so the spelling round-trips).
    ExcludeNulls,
}

/// An aliased expression inside a [`Pivot`]: a `USING` aggregate (`sum(x) AS total`) or
/// an `IN`-list value (`2000 AS y2000`).
///
/// One shape for both because they are identical surface — an expression with an
/// optional output-name alias. The alias may be written as an identifier or a string
/// literal (`2000 AS 'y2k'`), recorded via the [`Ident`]'s
/// [`QuoteStyle::Single`](super::QuoteStyle) exactly like MySQL's string projection
/// aliases, so the spelling round-trips.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PivotExpr<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Expr<X>,
    /// Alias assigned by this syntax.
    pub alias: Option<Ident>,
    /// How the source introduced `alias` (`sum(x) AS total` vs `sum(x) total`).
    /// Meaningful only when `alias` is `Some`; [`AliasSpelling::As`] otherwise.
    pub alias_spelling: AliasSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One pivot column and its optional `IN` source — a statement `ON` entry (`year`,
/// `year IN (2000, 2010)`, `year IN (SELECT …)`) or a table-factor `FOR` head
/// (`FOR <col> IN (<values>)`, `FOR <col> IN <enum>`,
/// `FOR <col> IN (ANY [ORDER BY …])`, `FOR <col> IN (<subquery>)`).
///
/// The value source is spread across three mutually-exclusive fields rather than one
/// enum: [`values`](Self::values) (the explicit list) and
/// [`enum_source`](Self::enum_source) are DuckDB's native shapes and stay untouched,
/// while [`value_source`](Self::value_source) carries the standard `ANY`/subquery forms
/// added for the Snowflake/BigQuery/Oracle table factor. This deliberately deviates from
/// sqlparser-rs's single `PivotValueSource { List, Any, Subquery }` enum: that shape
/// lives at the whole-pivot level and cannot express DuckDB's *per-column* `FOR y IN (…)
/// m IN (…)` heads or its `IN <enum>` form, so this node keeps the source per column and
/// folds the two new forms into an `Option` beside the existing DuckDB fields (see
/// [`PivotValueSource`]). Exactly one of the three is populated for any one column.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PivotColumn<X: Extension = NoExt> {
    /// The pivoted column or expression (a grouped `(a, b)` reads as a row
    /// expression). A statement entry whose `IN` source is a subquery keeps the whole
    /// [`Expr::InSubquery`](super::Expr) here — the engine admits it (probed on
    /// 1.5.4) and the subquery is a value *source*, not a value list.
    pub expr: Expr<X>,
    /// The explicit `IN (<values>)` list; empty for a statement `ON` column whose
    /// values DuckDB auto-detects at bind time (and whenever
    /// [`enum_source`](Self::enum_source) or [`value_source`](Self::value_source)
    /// carries the `IN` instead).
    pub values: ThinVec<PivotExpr<X>>,
    /// The table factor's `IN <enum>` form (`FOR m IN month_enum`): the values come
    /// from a named ENUM type rather than a written list. A single unqualified name —
    /// the engine rejects a qualified one (probed on 1.5.4) — and mutually exclusive
    /// with [`values`](Self::values).
    pub enum_source: Option<Ident>,
    /// The standard PIVOT's non-list value sources — `IN (ANY [ORDER BY …])` and
    /// `IN (<subquery>)` (Snowflake/BigQuery/Oracle), gated on
    /// [`TableFactorSyntax::pivot_value_sources`](crate::dialect::TableFactorSyntax).
    /// `None` for the explicit-list, enum, and auto-detected DuckDB forms; mutually
    /// exclusive with [`values`](Self::values)/[`enum_source`](Self::enum_source).
    /// Boxed to keep the common (list) column small while the source is wide.
    pub value_source: Option<Box<PivotValueSource<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A standard PIVOT table factor's non-list `IN` value source — the Snowflake/BigQuery/
/// Oracle forms beyond an explicit value list.
///
/// The explicit list (`IN (v1 [AS a], …)`) stays on [`PivotColumn::values`] and DuckDB's
/// `IN <enum>` on [`PivotColumn::enum_source`]; this enum carries only the two forms
/// those fields cannot: the wildcard `ANY` and a value-supplying subquery. It mirrors the
/// `Any`/`Subquery` arms of sqlparser-rs's `PivotValueSource` (the `List` arm is our
/// `values` field — see [`PivotColumn`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PivotValueSource<X: Extension = NoExt> {
    /// `IN (ANY [ORDER BY <keys>])` — pivot on every distinct value of the column, in
    /// the optional key order (Snowflake/Oracle). The `order_by` list is empty for a
    /// bare `ANY`.
    Any {
        /// Ordering terms in source order.
        order_by: ThinVec<OrderByExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `IN (<subquery>)` — pivot on the values a subquery returns
    /// (`FOR q IN (SELECT DISTINCT quarter FROM sales)`; Snowflake/Oracle). Boxed to
    /// break the `TableFactor` → `Pivot` → `Query` → `TableFactor` type cycle.
    Subquery {
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One `UNPIVOT` column entry: a statement `ON` item or an `IN`-list entry, holding one
/// column (`a`) or a group (`(a, b)`), with an optional alias (`(a, b) AS ab`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct UnpivotColumn<X: Extension = NoExt> {
    /// Columns in source order.
    pub columns: ThinVec<Expr<X>>,
    /// The `AS <alias>` name for the group; `None` when unwritten. An identifier or, as
    /// DuckDB admits in this position, a string literal (`(Q1, Q2) AS 'sem1'`) recorded
    /// via [`QuoteStyle::Single`](super::QuoteStyle) so the spelling round-trips.
    pub alias: Option<Ident>,
    /// How the source introduced `alias` (`mar AS q1` vs `mar q1`). Meaningful only when
    /// `alias` is `Some`; [`AliasSpelling::As`] otherwise.
    pub alias_spelling: AliasSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Which surface produced a [`Pivot`] — the leading-keyword statement or the `FROM`
/// table factor. One operator, two spellings kept as data (the
/// [`SelectSpelling`](crate::ast::SelectSpelling) precedent); the renderer re-emits the
/// written form (`PIVOT t ON …` vs `t PIVOT (… FOR … IN …)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PivotSpelling {
    /// Source used the `STATEMENT` spelling.
    Statement,
    /// Source used the `TABLE FACTOR` spelling.
    TableFactor,
}

/// Which surface produced an [`Unpivot`] — the mirror of [`PivotSpelling`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum UnpivotSpelling {
    /// Source used the `STATEMENT` spelling.
    Statement,
    /// The `FROM` table factor `<source> UNPIVOT [… NULLS] (<value> FOR <name> IN
    /// (<cols>))`.
    TableFactor,
}
