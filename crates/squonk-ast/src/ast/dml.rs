// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! DML statement AST nodes: INSERT, UPDATE, DELETE, and MERGE.

use super::{
    AliasSpelling, Expr, Extension, Ident, Limit, NoExt, ObjectName, OrderByExpr, Query,
    RelationInheritance, SelectItem, TableAlias, TableWithJoins, With,
};
use crate::vocab::Meta;
use thin_vec::ThinVec;

/// Which keyword spelled an [`Insert`]: standard `INSERT` or MySQL `REPLACE`.
///
/// MySQL `REPLACE [INTO] t ...` is a delete-then-insert upsert that shares INSERT's
/// entire tail grammar — the `VALUES` / `SET` / `SELECT` sources, the target column
/// list — and differs only in the lead keyword and in carrying none of the
/// `OVERRIDING` / upsert / `RETURNING` tails (it *is* the conflict resolution, so it
/// has no `ON DUPLICATE KEY UPDATE`). That is one canonical shape with
/// two surface spellings, so it is a tag on [`Insert`] rather than a forked
/// `Statement::Replace` node; the renderer reads it to re-emit the original keyword.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum InsertVerb {
    /// `INSERT` — add rows, erroring on a duplicate key.
    Insert,
    /// `REPLACE` — MySQL's insert-or-replace (delete then insert on a duplicate key).
    Replace,
}

/// DuckDB column-matching mode on `INSERT`: `BY NAME` or `BY POSITION`.
///
/// Written between the insert target and the source (`INSERT INTO t BY NAME SELECT …`).
/// Gated by [`MutationSyntax::insert_column_matching`](crate::dialect::MutationSyntax).
/// Engine rule (1.5.4): `BY NAME` cannot combine with an explicit column list and only
/// applies to a SELECT source — the parser rejects the column-list combination; VALUES
/// with `BY NAME` is left for the binder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum InsertColumnMatching {
    /// `BY NAME` — match source columns to targets by name (DuckDB).
    ByName,
    /// `BY POSITION` — match source columns to targets by position (the default).
    ByPosition,
}

/// The SQLite `OR <action>` conflict-resolution algorithm on the mutation verb —
/// `INSERT OR {REPLACE | IGNORE | ABORT | FAIL | ROLLBACK}` and the same tail on
/// `UPDATE`. It selects what SQLite does when the statement hits a constraint
/// violation, so it carries genuine new information (which algorithm) rather than a
/// surface spelling; the [`Insert::or_action`] / [`Update::or_action`] slot models it
/// and the parser gates it on [`MutationSyntax::or_conflict_action`](crate::dialect::MutationSyntax).
///
/// This is *distinct* from two same-named neighbours: it is not the PostgreSQL
/// [`ConflictAction`] (the `ON CONFLICT DO {NOTHING | UPDATE}` upsert action — a
/// different construct in a different clause position, which SQLite also has and which
/// maps to [`Insert::upsert`]); and its [`Replace`](Self::Replace) variant is not the
/// [`InsertVerb::Replace`] statement spelling — `INSERT OR REPLACE` and `REPLACE INTO`
/// are equivalent SQLite semantics written as different source texts, so each keeps its
/// own representation and round-trips through it (one is never folded onto the other).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ConflictResolution {
    /// `OR ROLLBACK` — abort the statement and roll back the enclosing transaction.
    Rollback,
    /// `OR ABORT` — abort the statement, reverting its own prior changes (the default
    /// resolution when no `OR <action>` is written).
    Abort,
    /// `OR FAIL` — abort the statement without reverting the rows it already changed.
    Fail,
    /// `OR IGNORE` — skip the offending row and continue the statement.
    Ignore,
    /// `OR REPLACE` — delete the conflicting rows, then insert/update the new one.
    Replace,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL insert.
pub struct Insert<X: Extension = NoExt> {
    /// Whether this was spelled `INSERT` or MySQL `REPLACE` (a surface tag over one
    /// shape; see [`InsertVerb`]).
    pub verb: InsertVerb,
    /// The SQLite `INSERT OR <action>` conflict-resolution algorithm on the verb, gated
    /// by [`MutationSyntax::or_conflict_action`](crate::dialect::MutationSyntax); `None`
    /// when unwritten (and always `None` on the [`InsertVerb::Replace`] spelling, which
    /// *is* the conflict resolution and takes no `OR <action>`). A distinct slot from
    /// [`upsert`](Self::upsert): the `OR <action>` prefix and the `ON CONFLICT` tail are
    /// different constructs (SQLite forbids both on one statement), never one overloaded
    /// field. See [`ConflictResolution`].
    pub or_action: Option<ConflictResolution>,
    /// Common table expressions visible to this statement.
    pub with: Option<With<X>>,
    /// Object targeted by this syntax.
    pub target: InsertTarget,
    /// DuckDB `BY NAME` / `BY POSITION` between target and source; `None` when unwritten.
    pub column_matching: Option<InsertColumnMatching>,
    /// Optional overriding for this syntax.
    pub overriding: Option<InsertOverriding>,
    /// Input source for this syntax.
    pub source: InsertSource<X>,
    /// MySQL 8.0.19+ row alias — `INSERT ... VALUES (...) AS <alias>[(<col>, ...)]`,
    /// the modern replacement for `VALUES(<col>)` inside `ON DUPLICATE KEY UPDATE`.
    /// Parsed between the [`source`](Self::source) and the [`upsert`](Self::upsert)
    /// clause and gated by the same dialect data as `ON DUPLICATE KEY UPDATE`
    /// ([`MutationSyntax::on_duplicate_key_update`](crate::dialect::MutationSyntax)) —
    /// the alias is the input side of that MySQL upsert surface, and no shipped dialect
    /// admits one without the other. Reuses the shared [`TableAlias`] shape (a name plus
    /// an optional column-alias list), never a parallel alias type; `None`
    /// when unwritten.
    pub row_alias: Option<TableAlias>,
    /// Upsert clause — insert-or-update on a unique-key conflict (gated by dialect
    /// data). One canonical field for the mutually-exclusive dialect spellings (see
    /// [`Upsert`]); boxed because the clause is heavy (its PostgreSQL arm carries two
    /// child lists) and absent from the common non-upsert `INSERT`, so the box keeps
    /// the inline `Insert` lean.
    pub upsert: Option<Box<Upsert<X>>>,
    /// Expressions returned by the statement.
    pub returning: Option<Returning<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL insert target.
pub struct InsertTarget {
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Alias assigned by this syntax.
    pub alias: Option<Ident>,
    /// How the source introduced `alias` (`INSERT INTO t AS x` vs `INSERT INTO t x`).
    /// Meaningful only when `alias` is `Some`; [`AliasSpelling::As`] otherwise.
    pub alias_spelling: AliasSpelling,
    /// Columns in source order.
    pub columns: ThinVec<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL insert overriding forms represented by the AST.
pub enum InsertOverriding {
    /// `OVERRIDING SYSTEM VALUE` — allow explicit values for `GENERATED ALWAYS` identity columns.
    SystemValue,
    /// `OVERRIDING USER VALUE` — ignore supplied values for `GENERATED BY DEFAULT` identity columns.
    UserValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL insert source forms represented by the AST.
pub enum InsertSource<X: Extension = NoExt> {
    /// `DEFAULT VALUES` — insert a single all-defaults row.
    DefaultValues {
        /// The `DEFAULT VALUES` marker; see [`DefaultValue`].
        default: DefaultValue,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `VALUES (…), …` row list.
    Values {
        /// Values in source order.
        values: Box<InsertValues<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An `INSERT … SELECT` query source.
    Query {
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL `SET <col> = {<expr> | DEFAULT} [, ...]` — the assignment-list source
    /// equivalent to a single-row `(<cols>) VALUES (<values>)`. Gated by dialect data
    /// ([`MutationSyntax::insert_set`](crate::dialect::MutationSyntax)) and reused by
    /// both `INSERT ... SET` and `REPLACE ... SET`. It reuses the shared
    /// [`UpdateAssignment`] shape (exactly as `UPDATE ... SET` and `ON DUPLICATE KEY
    /// UPDATE` do), never a parallel assignment representation.
    Set {
        /// assignments in source order.
        assignments: ThinVec<UpdateAssignment<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL insert values.
pub struct InsertValues<X: Extension = NoExt> {
    /// rows in source order.
    pub rows: ThinVec<ThinVec<InsertValue<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL insert value forms represented by the AST.
pub enum InsertValue<X: Extension = NoExt> {
    /// An ordinary value expression.
    Expr {
        /// Expression evaluated by this syntax.
        expr: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bare `DEFAULT` placeholder (use the column's default).
    Default {
        /// Explicit `DEFAULT` value.
        default: DefaultValue,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Shared target table shape for `UPDATE` and `DELETE`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct DmlTarget {
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// PostgreSQL `relation_expr` inheritance marker on the target relation (gated
    /// by dialect data): the bare/`*` spellings include descendant tables, while
    /// `ONLY table` / `ONLY (table)` suppress them.
    pub inheritance: RelationInheritance,
    /// Alias assigned by this syntax.
    pub alias: Option<Ident>,
    /// How the source introduced `alias` (`DELETE FROM t AS x` vs `DELETE FROM t x`).
    /// Meaningful only when `alias` is `Some`; [`AliasSpelling::As`] otherwise.
    pub alias_spelling: AliasSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL update.
pub struct Update<X: Extension = NoExt> {
    /// Common table expressions visible to this statement.
    pub with: Option<With<X>>,
    /// The SQLite `UPDATE OR <action>` conflict-resolution algorithm on the verb, gated
    /// by [`MutationSyntax::or_conflict_action`](crate::dialect::MutationSyntax); `None`
    /// when unwritten. The `UPDATE` counterpart of [`Insert::or_action`] — see
    /// [`ConflictResolution`].
    pub or_action: Option<ConflictResolution>,
    /// Object targeted by this syntax.
    pub target: DmlTarget,
    /// assignments in source order.
    pub assignments: ThinVec<UpdateAssignment<X>>,
    /// from in source order.
    pub from: ThinVec<TableWithJoins<X>>,
    /// Predicate that filters input rows.
    pub selection: Option<DmlSelection<X>>,
    /// MySQL single-table `UPDATE ... ORDER BY <keys>` tail — orders the rows a
    /// row-limited update touches. Parsed after [`selection`](Self::selection) and gated
    /// by [`MutationSyntax::update_delete_tails`](crate::dialect::MutationSyntax); empty
    /// when the dialect leaves the flag off or the clause is unwritten. Reuses the shared
    /// [`OrderByExpr`] key shape, never a mutation-specific one.
    pub order_by: ThinVec<OrderByExpr<X>>,
    /// MySQL single-table `UPDATE ... LIMIT <count>` tail — caps how many rows the
    /// update touches. Parsed after [`order_by`](Self::order_by) and gated by the same
    /// [`MutationSyntax::update_delete_tails`](crate::dialect::MutationSyntax); `None`
    /// when off or unwritten. Reuses the canonical [`Limit`] shape; MySQL's
    /// mutation `LIMIT` is a bare row count, so `offset` stays `None`.
    pub limit: Option<Limit<X>>,
    /// Expressions returned by the statement.
    pub returning: Option<Returning<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `UPDATE ... SET` assignment.
///
/// The single form assigns one column; the tuple form is the SQL multiple-column
/// assignment (feature T641, PostgreSQL `'(' set_target_list ')' '=' a_expr`),
/// gated by dialect data and kept a distinct variant so a single `(a) = (1)` and
/// a single `a = 1` never alias to one shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum UpdateAssignment<X: Extension = NoExt> {
    /// A single-column assignment (`<col> = <value>`).
    Single {
        /// Object targeted by this syntax.
        target: ObjectName,
        /// Value supplied by this syntax.
        value: UpdateValue<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A multi-column assignment (`(<cols>) = <source>`).
    Tuple {
        /// targets in source order.
        targets: ThinVec<ObjectName>,
        /// Input source for this syntax.
        source: UpdateTupleSource<X>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL update value forms represented by the AST.
pub enum UpdateValue<X: Extension = NoExt> {
    /// An ordinary value expression.
    Expr {
        /// Expression evaluated by this syntax.
        expr: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bare `DEFAULT` (reset the column to its default).
    Default {
        /// Explicit `DEFAULT` value.
        default: DefaultValue,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Right-hand side of a multiple-column (`( ... ) = <source>`) assignment.
///
/// PostgreSQL maps the source positionally onto the target columns, so the three
/// forms mirror the row-value sources the standard allows: a value row (with the
/// `ROW` keyword optional), a row subquery, or a bare `DEFAULT` that defaults every
/// target. `DEFAULT` is kept out of the expression grammar (like [`UpdateValue`]),
/// so a per-element default inside a row stays a [`UpdateValue::Default`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum UpdateTupleSource<X: Extension = NoExt> {
    /// A `[ROW] (<values>)` value row assigned positionally to the targets.
    Row {
        /// Whether the explicit form was present in the source.
        explicit: bool,
        /// Values in source order.
        values: ThinVec<UpdateValue<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A row subquery (`(<query>)`) assigned positionally to the targets.
    Subquery {
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bare `DEFAULT` defaulting every target column.
    Default {
        /// The `DEFAULT` marker; see [`DefaultValue`].
        default: DefaultValue,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL delete.
pub struct Delete<X: Extension = NoExt> {
    /// Common table expressions visible to this statement.
    pub with: Option<With<X>>,
    /// Object targeted by this syntax.
    pub target: DmlTarget,
    /// using in source order.
    pub using: ThinVec<TableWithJoins<X>>,
    /// Predicate that filters input rows.
    pub selection: Option<DmlSelection<X>>,
    /// MySQL single-table `DELETE ... ORDER BY <keys>` tail — the row-ordering half of
    /// the row-limited delete. Parsed after [`selection`](Self::selection) and gated by
    /// [`MutationSyntax::update_delete_tails`](crate::dialect::MutationSyntax); empty
    /// when off or unwritten. Reuses the shared [`OrderByExpr`] key shape.
    pub order_by: ThinVec<OrderByExpr<X>>,
    /// MySQL single-table `DELETE ... LIMIT <count>` tail — caps how many rows the
    /// delete removes. Parsed after [`order_by`](Self::order_by) and gated by the same
    /// [`MutationSyntax::update_delete_tails`](crate::dialect::MutationSyntax); `None`
    /// when off or unwritten. Reuses the canonical [`Limit`] shape, a bare
    /// row count with `offset` left `None`.
    pub limit: Option<Limit<X>>,
    /// Expressions returned by the statement.
    pub returning: Option<Returning<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `WHERE` filter on an `UPDATE` or `DELETE`.
///
/// A condition is the ordinary row filter; `WHERE CURRENT OF <cursor>` is the
/// positioned form (gated by dialect data) that targets the row a named open
/// cursor sits on. The two are mutually exclusive in the grammar, so one enum
/// keeps an invalid "condition *and* cursor" state unrepresentable.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DmlSelection<X: Extension = NoExt> {
    /// A `WHERE <predicate>` row filter.
    Where {
        /// Predicate that controls this clause.
        condition: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `WHERE CURRENT OF <cursor>` positioned update/delete.
    CurrentOf {
        /// The open cursor whose current row is targeted.
        cursor: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL default value.
pub struct DefaultValue {
    /// Source location and node identity.
    pub meta: Meta,
}

/// `RETURNING <output> [, ...]` clause on a mutation statement (PostgreSQL).
///
/// The output list is a projection — `*`, `<table>.*`, or `<expr> [[AS] alias]` —
/// so it reuses [`SelectItem`] rather than a parallel item type. A `RETURNING`
/// clause is always non-empty; its absence is modelled by `Option<Returning>` on
/// the statement.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Returning<X: Extension = NoExt> {
    /// Child items in source order.
    pub items: ThinVec<SelectItem<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// `INSERT ...` upsert clause: insert-or-update on a unique-key conflict.
///
/// Dialects spell this with structurally different clauses — PostgreSQL's
/// `ON CONFLICT` carries an explicit conflict arbiter and a do-nothing-or-update
/// action, while MySQL's `ON DUPLICATE KEY UPDATE` infers the conflicting key and
/// is only an assignment list — so this is one canonical *clause* enum (a
/// forked shape, not a surface tag over one shape) rather than two `Insert` fields:
/// an `INSERT` carries at most one upsert clause, and a single field makes the
/// invalid "both clauses" state unrepresentable. Both arms reuse [`UpdateAssignment`]
/// for the SET list — the shared assignment shape, never a parallel representation.
///
/// Each variant carries `meta` so the clause is a directly-addressable spanned node
/// (the same wrapper-with-`meta` discipline [`super::Statement`] uses over its boxed
/// statement nodes), not one whose span is reconstructed from its children.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum Upsert<X: Extension = NoExt> {
    /// PostgreSQL `ON CONFLICT [<target>] DO {NOTHING | UPDATE SET …}`.
    OnConflict {
        /// The conflict target and action; see [`OnConflict`].
        conflict: OnConflict<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL `ON DUPLICATE KEY UPDATE <col> = <expr> [, ...]`. MySQL infers the
    /// violated unique key (no arbiter) and has no `DO NOTHING`, so the clause is
    /// just the assignment list — reusing [`UpdateAssignment`] exactly as the
    /// PostgreSQL `DO UPDATE SET` action does.
    OnDuplicateKeyUpdate {
        /// assignments in source order.
        assignments: ThinVec<UpdateAssignment<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL on conflict.
pub struct OnConflict<X: Extension = NoExt> {
    /// The conflict arbiter. `None` is the bare `ON CONFLICT DO NOTHING`, which
    /// lets any unique violation trigger the action.
    pub target: Option<ConflictTarget<X>>,
    /// What to do on a conflict (`DO NOTHING`/`DO UPDATE`); see [`ConflictAction`].
    pub action: ConflictAction<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL conflict target forms represented by the AST.
pub enum ConflictTarget<X: Extension = NoExt> {
    /// `( <index-element> [, ...] ) [ WHERE <predicate> ]` — index inference with an
    /// optional partial-index predicate. Each element is an expression so a bare
    /// column and an index expression share one shape.
    Index {
        /// Columns in source order.
        columns: ThinVec<Expr<X>>,
        /// Predicate that controls this clause.
        predicate: Option<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An `ON CONFLICT ON CONSTRAINT <name>` named-constraint arbiter.
    Constraint {
        /// Name referenced by this syntax.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL conflict action forms represented by the AST.
pub enum ConflictAction<X: Extension = NoExt> {
    /// `DO NOTHING` — silently ignore the conflicting row.
    Nothing {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DO UPDATE SET <assignment> [, ...] [ WHERE <condition> ]`. The assignment
    /// shape matches `UPDATE ... SET`, so it reuses [`UpdateAssignment`].
    Update {
        /// assignments in source order.
        assignments: ThinVec<UpdateAssignment<X>>,
        /// Predicate that filters input rows.
        selection: Option<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// `MERGE INTO <target> [AS alias] USING <source> ON <condition> <when_clause>+`
/// statement (SQL:2003 feature F312; the standard upsert).
///
/// One canonical shape: the standard models MERGE directly, so this is a
/// shared node gated for *acceptance* by [`MutationSyntax::merge`](crate::dialect::MutationSyntax)
/// — on in ANSI (SQL:2016) and PostgreSQL 15+, off in MySQL — never a dialect fork.
/// The clause reuses the existing mutation shapes rather than parallel ones: the
/// `MERGE INTO <target>` relation is the shared [`DmlTarget`] (carrying the same
/// `ONLY`/`*` inheritance marker `UPDATE`/`DELETE` targets do), the `USING` source is
/// a [`TableWithJoins`] (SQL:2016's `<table reference>` — a table, a derived subquery,
/// or a joined table, exactly as in a `FROM` clause), and the `WHEN` actions reuse the
/// `INSERT` value element ([`InsertValue`]) and the `UPDATE ... SET` assignment list
/// ([`UpdateAssignment`]).
///
/// Boxed at the [`Statement`](super::Statement) seam (`Statement::Merge`) because MERGE
/// is a fat, infrequent variant — its `when_clause` list and the embedded source and
/// condition make it one of the widest statements — so boxing keeps the common
/// `Statement` lean.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Merge<X: Extension = NoExt> {
    /// Optional statement-level `WITH` clause (`WITH ... MERGE INTO ...`; PostgreSQL
    /// 15+ and DuckDB, probed on 1.5.4). Not standard surface — SQL:2016's
    /// `<merge statement>` takes no `WITH` — so it is gated by
    /// [`MutationSyntax::cte_before_merge`](crate::dialect::MutationSyntax), the
    /// `MERGE` counterpart of [`Insert::with`]'s `cte_before_insert` gate.
    pub with: Option<With<X>>,
    /// `MERGE INTO <target>` — the table rows are merged into, plus its optional
    /// correlation alias and PostgreSQL `ONLY`/`*` inheritance marker. Reuses the shared
    /// [`DmlTarget`] shape (`UPDATE`/`DELETE` use the same one), so `MERGE INTO ONLY t`
    /// and `MERGE INTO t *` ride the same `table_expressions.only` gate as those
    /// statements — never a parallel target type.
    pub target: DmlTarget,
    /// `USING <source>` — the data source the target is merged against: a table, a
    /// derived subquery, or a joined table, reusing the [`TableWithJoins`] shape (a
    /// relation plus its chained joins, exactly as a `FROM` item; SQL:2016's
    /// `USING <table reference>`) rather than a parallel source type. A bare
    /// comma-separated list is not admitted — the merge source is one `<table
    /// reference>`, so `USING a, b` rejects (engine-verified on pg_query 17).
    pub using: TableWithJoins<X>,
    /// The `ON <join predicate>` matching condition.
    pub on: Expr<X>,
    /// `WHEN [NOT] MATCHED ...` clauses, in source order. The parser guarantees at
    /// least one; the standard requires a non-empty list.
    pub clauses: ThinVec<MergeWhenClause<X>>,
    /// `RETURNING ...` output clause (PostgreSQL 17+ and DuckDB, probed on 1.5.4;
    /// where `merge_action()` is also admitted as an output expression). Rides the
    /// same [`MutationSyntax::returning`](crate::dialect::MutationSyntax) gate as the
    /// other DML statements — every shipped dialect with `merge` on either accepts
    /// both or rejects both.
    pub returning: Option<Returning<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `WHEN <match kind> [AND <predicate>] THEN <action>` arm of a [`Merge`].
///
/// [`match_kind`](Self::match_kind) distinguishes the three productions
/// ([`MergeMatchKind`]) and, with them, the actions the parser admits: a
/// `MATCHED ... THEN INSERT` (like a `NOT MATCHED ... THEN UPDATE`) is rejected. The
/// single-enum-plus-[`MergeAction`] shape keeps all three productions one node.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct MergeWhenClause<X: Extension = NoExt> {
    /// Which `WHEN` production (matched / not-matched by target/source); see [`MergeMatchKind`].
    pub match_kind: MergeMatchKind,
    /// Predicate that controls this clause.
    pub condition: Option<Expr<X>>,
    /// The `THEN` action; see [`MergeAction`].
    pub action: MergeAction<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Which `WHEN` production a [`MergeWhenClause`] is, and thus which target/source
/// pairing it fires on and which [`MergeAction`]s it admits.
///
/// PostgreSQL 17 (and DuckDB, probed on 1.5.4) split the unmatched case by *which*
/// side is missing, so the standard two-way `MATCHED`/`NOT MATCHED` grows a third arm.
/// A bare `WHEN NOT MATCHED` is `NOT MATCHED BY TARGET` (no target row for a source
/// row → insert), mirroring pg_query's own `matchKind` collapse, so the two spellings
/// fold to [`NotMatchedByTarget`](Self::NotMatchedByTarget). The `BY SOURCE`/`BY
/// TARGET` spellings are gated by
/// [`MutationSyntax::merge_when_not_matched_by`](crate::dialect::MutationSyntax) (off
/// in ANSI — not SQL:2016 surface); the bare forms ride the
/// [`MutationSyntax::merge`](crate::dialect::MutationSyntax) statement gate itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum MergeMatchKind {
    /// `WHEN MATCHED` — a source row paired with a target row; admits `UPDATE`/`DELETE`/
    /// `DO NOTHING`.
    Matched,
    /// `WHEN NOT MATCHED` / `WHEN NOT MATCHED BY TARGET` — an unpaired source row (no
    /// target); admits `INSERT`/`DO NOTHING`.
    NotMatchedByTarget,
    /// `WHEN NOT MATCHED BY SOURCE` — an unpaired target row (no source); admits
    /// `UPDATE`/`DELETE`/`DO NOTHING`, exactly as the `MATCHED` arm.
    NotMatchedBySource,
}

/// The `THEN` action of a [`MergeWhenClause`].
///
/// Each arm reuses an existing mutation shape rather than forking one: `Insert` reuses
/// the `INSERT` value element ([`InsertValue`], which carries `DEFAULT` as well as
/// expressions) for its single `VALUES` row plus the shared identity-override marker
/// ([`InsertOverriding`]), and `Update` reuses the `UPDATE ... SET` assignment list
/// ([`UpdateAssignment`]). `InsertDefault` (`INSERT DEFAULT VALUES`) is a separate
/// variant so its incompatibility with a column list and an `OVERRIDING` clause is
/// unrepresentable — PostgreSQL rejects `INSERT (a) DEFAULT VALUES` and `INSERT
/// OVERRIDING ... DEFAULT VALUES` (engine-verified on pg_query 17). `Delete` and
/// `DoNothing` carry only `meta`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum MergeAction<X: Extension = NoExt> {
    /// `INSERT [(<column> [, ...])] [OVERRIDING {SYSTEM | USER} VALUE] VALUES (<value>
    /// [, ...])` (a `NOT MATCHED [BY TARGET]` arm). The standard merge insert is a
    /// single value row, so `values` is one row of [`InsertValue`] items — never the
    /// multi-row [`InsertValues`], which would make an illegal multi-row merge insert
    /// representable. `overriding` is the identity-column override
    /// ([`InsertOverriding`], reused from top-level `INSERT`), gated by
    /// [`MutationSyntax::merge_insert_overriding`](crate::dialect::MutationSyntax)
    /// (SQL:2016 surface, but DuckDB rejects it in `MERGE`; probed on 1.5.4).
    Insert {
        /// Columns in source order.
        columns: ThinVec<Ident>,
        /// Optional overriding for this syntax.
        overriding: Option<InsertOverriding>,
        /// Values in source order.
        values: ThinVec<InsertValue<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `INSERT DEFAULT VALUES` (a `NOT MATCHED [BY TARGET]` arm) — insert a row of
    /// column defaults, taking neither a column list nor an `OVERRIDING` clause. Gated
    /// by [`MutationSyntax::merge_insert_default_values`](crate::dialect::MutationSyntax)
    /// (PostgreSQL/DuckDB, not SQL:2016).
    InsertDefault {
        /// The `DEFAULT VALUES` marker; see [`DefaultValue`].
        default: DefaultValue,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `UPDATE SET <assignment> [, ...]` (a `MATCHED` or `NOT MATCHED BY SOURCE` arm).
    /// Reuses [`UpdateAssignment`].
    Update {
        /// assignments in source order.
        assignments: ThinVec<UpdateAssignment<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB `UPDATE SET *` — copy every matching column from the source row.
    /// Gated by [`MutationSyntax::merge_update_set_star`](crate::dialect::MutationSyntax).
    UpdateStar {
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB `INSERT *` — insert every source column by position/name.
    /// Gated by [`MutationSyntax::merge_insert_star_by_name`](crate::dialect::MutationSyntax).
    InsertStar {
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB `INSERT BY NAME` / `INSERT BY NAME *` — insert matching columns by name.
    /// Gated by [`MutationSyntax::merge_insert_star_by_name`](crate::dialect::MutationSyntax).
    InsertByName {
        /// True for the `INSERT BY NAME *` spelling; false for bare `INSERT BY NAME`.
        star: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB `THEN ERROR` — raise when the arm fires.
    /// Gated by [`MutationSyntax::merge_error_action`](crate::dialect::MutationSyntax).
    Error {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `THEN DELETE` — delete the matched target row.
    Delete {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `THEN DO NOTHING` — take no action for this row.
    DoNothing {
        /// Source location and node identity.
        meta: Meta,
    },
}
