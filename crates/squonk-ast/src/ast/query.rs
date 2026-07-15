// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Query-level AST nodes: `Query`, set operations, `SELECT`, CTEs, and locking clauses.

use super::{
    DataType, DefaultValue, Delete, Expr, Extension, FunctionCall, Ident, Insert, JsonBehavior,
    JsonFormat, JsonPassingArg, JsonQuotesBehavior, JsonValueExpr, JsonWrapperBehavior, Literal,
    MatchRecognize, Merge, NamedWindow, NoExt, ObjectName, PipeOperator, Pivot,
    SemiStructuredPathSegment, SpecialFunctionKeyword, TemporaryTableKind, Unpivot, Update,
    XmlPassingMechanism,
};
use crate::vocab::{Meta, Symbol};
use thin_vec::ThinVec;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL query.
pub struct Query<X: Extension = NoExt> {
    /// Common table expressions visible to this statement.
    pub with: Option<With<X>>,
    /// Statement or query body governed by this node.
    pub body: SetExpr<X>,
    /// Ordering terms in source order.
    pub order_by: ThinVec<OrderByExpr<X>>,
    /// DuckDB's `ORDER BY ALL` mode — sort by every projection column, left to
    /// right — with its optional direction/nulls modifiers; `None` for an ordinary
    /// (or absent) `ORDER BY`. Mutually exclusive with a non-empty
    /// [`order_by`](Self::order_by): DuckDB rejects mixing `ALL` with explicit sort
    /// keys (`ORDER BY ALL, x` / `ORDER BY x, ALL` are syntax errors; probed on
    /// 1.5.4), so `ALL` is a *mode of the whole clause*, never a sort-key
    /// expression — the column set is unknowable at parse time, so a synthetic key
    /// list would be a lie (the shape is anchored on the semantics). `Box`ed
    /// because the clause is rare while `Query` is a hot node (the
    /// [`Select::into`] precedent). Gated by
    /// [`GroupingSyntax::order_by_all`](crate::dialect::SelectSyntax); only the
    /// query-level clause admits `ALL` — DuckDB rejects it in window `ORDER BY`
    /// ("Cannot ORDER BY ALL in a window expression") and has no DML sort tails.
    pub order_by_all: Option<Box<OrderByAll>>,
    /// ClickHouse `LIMIT n [OFFSET m] BY expr, …` — per-group row limiting, written
    /// after `ORDER BY` and *before* the ordinary [`limit`](Self::limit) tail. Its own
    /// field because it coexists with `limit` and means something different (see
    /// [`LimitBy`]); `None` for the common query that writes no `LIMIT BY`. Gated by
    /// [`QueryTailSyntax::limit_by_clause`](crate::dialect::SelectSyntax) — off for every
    /// shipped preset but Lenient, so it stays `None` elsewhere. `Box`ed because the
    /// clause is rare while `Query` is a hot node (the [`order_by_all`](Self::order_by_all)
    /// precedent) — keeping the 104-byte [`LimitBy`] off the inline `Query` footprint.
    pub limit_by: Option<Box<LimitBy<X>>>,
    /// Row limit applied to the result.
    pub limit: Option<Limit<X>>,
    /// ClickHouse `SETTINGS name = value, …` — query-level setting overrides written
    /// after the ordinary [`limit`](Self::limit) tail (`SELECT … LIMIT 10 SETTINGS
    /// max_threads = 8`). Empty for the common query that writes none — a bare
    /// [`ThinVec`] costing one null pointer when unused, following the
    /// [`locking`](Self::locking)/[`pipe_operators`](Self::pipe_operators) list-tail
    /// precedent rather than a boxed option (a list's own rarity optimization is the
    /// empty vector, so `Query`'s hot footprint grows by only that one pointer). Gated
    /// by [`QueryTailSyntax::settings_clause`](crate::dialect::SelectSyntax) — on for
    /// Lenient only, so it stays empty under every oracle-compared preset.
    pub settings: ThinVec<Setting<X>>,
    /// ClickHouse `FORMAT <name>` — the output-format clause that closes the query, the
    /// last tail of all (after [`settings`](Self::settings)); `None` for the common query
    /// that names no format. The format name is a bare identifier (`JSON`, `CSV`,
    /// `TabSeparated`, `Null`), see [`FormatClause`]. Gated by
    /// [`QueryTailSyntax::format_clause`](crate::dialect::SelectSyntax) — off for every
    /// shipped preset but Lenient, so it stays `None` elsewhere. `Box`ed because the
    /// clause is rare while `Query` is a hot node (the
    /// [`order_by_all`](Self::order_by_all)/[`limit_by`](Self::limit_by) precedent),
    /// keeping the clause off the inline `Query` footprint at one pointer's cost.
    pub format: Option<Box<FormatClause>>,
    /// The row-locking clauses (`FOR UPDATE`/`FOR SHARE`/…) written after `LIMIT`.
    /// PostgreSQL admits several stacked clauses (`FOR UPDATE OF a FOR SHARE OF b`),
    /// MySQL exactly one, so this is a list; empty when the query writes none. Gated
    /// by [`QueryTailSyntax::locking_clauses`](crate::dialect::SelectSyntax). Not generic
    /// over `X`: a [`LockingClause`] carries only names and surface tags, no expression.
    pub locking: ThinVec<LockingClause>,
    /// BigQuery/ZetaSQL `|>` pipe operators applied to this query's result, left to
    /// right (`FROM t |> WHERE x |> SELECT a`). Empty for an ordinary query — the common
    /// case, so a bare [`ThinVec`] that costs one null pointer when unused, like
    /// [`locking`](Self::locking). Each element is one [`PipeOperator`] step. Gated by
    /// [`QueryTailSyntax::pipe_syntax`](crate::dialect::SelectSyntax); off for every shipped
    /// preset, so the field stays empty there and the `|>` token never even lexes.
    pub pipe_operators: ThinVec<PipeOperator<X>>,
    /// MSSQL `FOR XML …` / `FOR JSON …` result-shaping tail — the last clause of all,
    /// serializing the result as XML/JSON instead of a rowset; `None` for the common
    /// query that writes no `FOR XML`/`FOR JSON`. See [`ForClause`]. Gated by
    /// [`QueryTailSyntax::for_xml_json_clause`](crate::dialect::SelectSyntax) — on for
    /// MSSQL and Lenient, so it stays `None` elsewhere. `Box`ed because the clause is
    /// rare while `Query` is a hot node (the
    /// [`order_by_all`](Self::order_by_all)/[`format`](Self::format) precedent), keeping
    /// the 92-byte [`ForClause`] off the inline `Query` footprint at one pointer's cost.
    /// Not generic over `X`: a [`ForClause`] carries only mode tags and quoted names,
    /// no expression.
    pub for_clause: Option<Box<ForClause>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One row-locking clause on a [`Query`]: `FOR UPDATE`/`FOR SHARE`/… with the
/// optional `OF <table>, …` target list and a `NOWAIT`/`SKIP LOCKED` wait policy.
///
/// PostgreSQL and MySQL share this modern surface (PG `for_locking_clause`), so it
/// is one canonical shape gated per-dialect rather than a parallel node
/// per engine. MySQL's legacy `LOCK IN SHARE MODE` is a *spelling* of `FOR SHARE`
/// (it predates the `FOR SHARE` keyword), folded onto this shape with a
/// [`LockingSpelling`] tag so the surface round-trips. Carries no [`Expr`], so it is
/// not generic over the extension parameter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LockingClause {
    /// Which lock strength (`FOR UPDATE`/`FOR SHARE`/…); see [`LockStrength`].
    pub strength: LockStrength,
    /// The `OF <table>, …` restriction naming which relations the lock applies to;
    /// empty when the clause locks every table in the query (no `OF`). PostgreSQL
    /// maps these to `RangeVar`s (relation names), so [`ObjectName`], not `Ident`.
    pub of: ThinVec<ObjectName>,
    /// Optional wait for this syntax.
    pub wait: Option<LockWait>,
    /// Exact source spelling retained for faithful rendering.
    pub spelling: LockingSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The strength of a [`LockingClause`], strongest first as PostgreSQL orders them.
///
/// All four PostgreSQL `for_locking_strength` levels are modelled so the one shape
/// covers both dialects (MySQL writes only [`Update`](Self::Update) and
/// [`Share`](Self::Share); the `KEY`/`NO KEY` refinements are PostgreSQL-only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LockStrength {
    /// `FOR UPDATE` — the strongest row lock.
    Update,
    /// `FOR NO KEY UPDATE` — a weaker exclusive lock that still admits `KEY SHARE` (PostgreSQL).
    NoKeyUpdate,
    /// `FOR SHARE` (also MySQL's `LOCK IN SHARE MODE`, distinguished by
    /// [`LockingSpelling`]).
    Share,
    /// `FOR KEY SHARE` — the weakest row lock (PostgreSQL).
    KeyShare,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL lock wait forms represented by the AST.
pub enum LockWait {
    /// `NOWAIT`: error immediately rather than wait for a conflicting lock.
    NoWait,
    /// `SKIP LOCKED`: silently omit rows that are already locked.
    SkipLocked,
}

/// Surface syntax that produced a [`LockingClause`].
///
/// One `FOR SHARE` semantic, two spellings kept as data (mirroring
/// [`RollupSpelling`]): the modern `FOR UPDATE`/`FOR SHARE`/… keyword form and
/// MySQL's legacy `LOCK IN SHARE MODE`. Canonicalizing the legacy spelling onto the
/// [`LockStrength::Share`] shape lets the differential oracle compare one shape; the
/// tag exists only so rendering reproduces the written form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LockingSpelling {
    /// The modern `FOR <strength> [OF …] [NOWAIT|SKIP LOCKED]` keyword form; the
    /// construction default and the only form PostgreSQL writes.
    Modern,
    /// MySQL's legacy `LOCK IN SHARE MODE` (a bare `FOR SHARE` with no `OF`/wait
    /// tail). Only valid on [`LockStrength::Share`].
    LockInShareMode,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL set expr forms represented by the AST.
pub enum SetExpr<X: Extension = NoExt> {
    /// A `SELECT` query body.
    Select {
        /// The select body; see [`Select`].
        select: Box<Select<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `VALUES (...)` row-constructor query body.
    Values {
        /// Values in source order.
        values: Box<Values<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A parenthesized nested query body.
    Query {
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A set operation combining two query bodies (`UNION`/`INTERSECT`/`EXCEPT`).
    SetOperation {
        /// Operator applied by this expression.
        op: SetOperator,
        /// Whether the all form was present in the source.
        all: bool,
        /// DuckDB's `UNION [ALL] BY NAME` modifier: pair the two inputs' columns by
        /// *name* (padding a side's missing columns with NULL) instead of by
        /// position. A semantic modifier of the operation — it changes column
        /// correspondence — so it is a flag on this node, not a spelling tag or a new
        /// [`SetOperator`] variant (a semantic modifier is a shape field, a
        /// pure spelling would be a tag). Orthogonal to [`all`](Self::SetOperation::all),
        /// exactly as DuckDB models it — `UNION [ALL] BY NAME` serializes as
        /// `setop_type: UNION_BY_NAME` with a separate `setop_all` (probed on 1.5.4).
        /// DuckDB accepts `BY NAME` on `UNION` only (`INTERSECT`/`EXCEPT BY NAME` are
        /// syntax errors; probed on 1.5.4), so the parser sets this `true` only for
        /// [`SetOperator::Union`]. Gated by
        /// [`SelectSyntax::union_by_name`](crate::dialect::SelectSyntax); `false` for
        /// every ordinary (positional) set operation.
        by_name: bool,
        /// Left-hand operand.
        left: Box<SetExpr<X>>,
        /// Right-hand operand.
        right: Box<SetExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's `PIVOT` operator standing as a **query body** — the CTE body, the
    /// `CREATE VIEW`/`CREATE TABLE AS`/`CREATE MACRO … AS TABLE` body, or any other
    /// query-body position (`WITH p AS (PIVOT t ON x USING sum(y)) SELECT …`,
    /// `CREATE VIEW v AS PIVOT t ON x IN (…) USING sum(y)`; both probed on 1.5.4).
    /// DuckDB admits `PIVOT`/`UNPIVOT` at query-body position but *not*
    /// `DESCRIBE`/`SHOW` (a CTE body with those is `Parser Error: A CTE needs a
    /// SELECT`), so this is a query-body variant — reusing the shared [`Pivot`] core
    /// (tagged [`PivotSpelling::Statement`](super::PivotSpelling)) exactly as
    /// [`Statement::Pivot`](super::Statement) and [`TableFactor::Pivot`] do — rather
    /// than a general "statement in query position" carrier (which the
    /// canonical-shape rule bans as a vibe-union, and which the divergent composition
    /// of `DESCRIBE`/`SHOW` would misrepresent). `Box`ed to keep this hot enum within
    /// its size budget. Gated by
    /// [`TableFactorSyntax::pivot`](crate::dialect::TableExpressionSyntax).
    Pivot {
        /// The pivot operation; see [`Pivot`].
        pivot: Box<Pivot<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's `UNPIVOT` operator standing as a query body — the
    /// [`Pivot`](Self::Pivot) counterpart, sharing the [`Unpivot`] core with
    /// [`Statement::Unpivot`](super::Statement) and [`TableFactor::Unpivot`]. Gated by
    /// [`TableFactorSyntax::unpivot`](crate::dialect::TableExpressionSyntax).
    Unpivot {
        /// The unpivot operation; see [`Unpivot`].
        unpivot: Box<Unpivot<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL set operator forms represented by the AST.
pub enum SetOperator {
    /// `UNION` — all rows from both inputs (deduplicated unless `ALL`).
    Union,
    /// `INTERSECT` — rows present in both inputs.
    Intersect,
    /// `EXCEPT` — rows in the left input but not the right.
    Except,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL with.
pub struct With<X: Extension = NoExt> {
    /// Whether the recursive form was present in the source.
    pub recursive: bool,
    /// ctes in source order.
    pub ctes: ThinVec<Cte<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL cte.
pub struct Cte<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Columns in source order.
    pub columns: ThinVec<Ident>,
    /// DuckDB's `USING KEY (col, ...)` recursive-CTE key clause, written between the CTE
    /// column list and `AS` (`WITH RECURSIVE t(a, b) USING KEY (a) AS (…)`). `None` is the
    /// ordinary CTE; `Some` carries the key columns (always at least one — the parser
    /// requires a non-empty parenthesized list). Bare CTE-column names ([`Ident`]), never
    /// expressions, so a reserved word here is a parse error. Gated by
    /// [`JoinSyntax::recursive_using_key`](crate::dialect::JoinSyntax); a plain
    /// `Option<ThinVec>` (one pointer, not boxed) keeps the hot node's ADR-0007 budget
    /// while carrying the rare clause inline.
    pub using_key: Option<ThinVec<Ident>>,
    /// Whether the materialized form was present in the source.
    pub materialized: Option<bool>,
    /// Statement or query body governed by this node.
    pub body: CteBody<X>,
    /// The SQL:2023 `SEARCH { DEPTH | BREADTH } FIRST BY … SET …` recursive-ordering
    /// clause, written after the body's `)` ([`CteSearchClause`]). Boxed and optional
    /// because it rides only the rare recursive CTE, so the hot node pays one pointer
    /// (ADR-0007) rather than the inline clause; gated by
    /// [`JoinSyntax::recursive_search_cycle`](crate::dialect::TableExpressionSyntax).
    pub search: Option<Box<CteSearchClause>>,
    /// The SQL:2023 `CYCLE … SET … [TO … DEFAULT …] USING …` cycle-detection clause
    /// ([`CteCycleClause`]), written after any [`search`](Self::search) clause — the
    /// fixed grammar order (`CYCLE … SEARCH …` is a parse error; probed on pg_query 17).
    /// Boxed and optional for the same reason, under the same gate.
    pub cycle: Option<Box<CteCycleClause<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The SQL:2023 recursive-query result-ordering clause on a [`Cte`]:
/// `SEARCH { DEPTH | BREADTH } FIRST BY col [, ...] SET seqcol`.
///
/// PostgreSQL attaches it after the CTE body's closing `)` (gram.y `opt_search_clause`).
/// One of the two orders is mandatory — `SEARCH FIRST …` with neither `DEPTH` nor
/// `BREADTH` is a parse error — and the `SET` sequence column is required. Non-generic:
/// its column lists are bare names ([`Ident`]), never expressions. Gated by
/// [`JoinSyntax::recursive_search_cycle`](crate::dialect::TableExpressionSyntax);
/// admitted at parse even on a non-`RECURSIVE` `WITH` (the recursion requirement is an
/// analysis check past this crate's parse boundary; probed on pg_query 17).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CteSearchClause {
    /// `BREADTH FIRST` (`true`) vs `DEPTH FIRST` (`false`); mirrors PostgreSQL's
    /// `search_breadth_first` flag.
    pub breadth_first: bool,
    /// The `BY col [, ...]` ordering columns — bare CTE column names (`columnList`), so
    /// a reserved word here (`SEARCH … BY select …`) is a parse error.
    pub columns: ThinVec<Ident>,
    /// The `SET seqcol` sequence column the ordering is materialized into.
    pub set_column: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The SQL:2023 recursive-query cycle-detection clause on a [`Cte`]:
/// `CYCLE col [, ...] SET mark [TO value DEFAULT default] USING path`.
///
/// PostgreSQL attaches it after any [`CteSearchClause`] (gram.y `opt_cycle_clause`). The
/// `SET` mark column and the `USING` path column are both required; the `TO … DEFAULT …`
/// mark values are optional (the short form defaults the mark to boolean `TRUE`/`FALSE`).
/// Gated like [`CteSearchClause`] by
/// [`JoinSyntax::recursive_search_cycle`](crate::dialect::TableExpressionSyntax).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CteCycleClause<X: Extension = NoExt> {
    /// The `CYCLE col [, ...]` columns compared to detect a repeated row (`columnList`,
    /// bare names).
    pub columns: ThinVec<Ident>,
    /// The `SET mark` cycle-mark column.
    pub mark_column: Ident,
    /// The optional `TO value DEFAULT default` mark values ([`CteCycleMark`]); `None` is
    /// the short `SET mark USING path` form. Modelled as one node, not two independent
    /// options, because PostgreSQL admits the pair only together — `TO` without `DEFAULT`
    /// (or the reverse) is a parse error (probed on pg_query 17).
    pub mark: Option<CteCycleMark<X>>,
    /// The `USING path` array column recording the traversal path.
    pub path_column: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `TO value DEFAULT default` cycle-mark values of a [`CteCycleClause`].
///
/// Both values are PostgreSQL `AexprConst` constants — a literal or a typed-string
/// constant (`point '(1,1)'`), never a general expression: `TO (1+2)`, a column
/// reference, `CAST(…)`, a parameter, or a signed number all parse-reject (probed on
/// pg_query 17). The parser constrains each to that grammar, so a value here is always an
/// [`Expr::Literal`](super::Expr) or a prefix-typed
/// [`Expr::Cast`](super::Expr) ([`CastSyntax::PrefixTyped`](super::CastSyntax)); the
/// nodes are boxed because [`Expr`] is a hot, larger node while the mark is
/// rare.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CteCycleMark<X: Extension = NoExt> {
    /// Value supplied by this syntax.
    pub value: Box<Expr<X>>,
    /// The value marking rows not on a cycle (the `DEFAULT` operand).
    pub default: Box<Expr<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The parenthesized body of a [`Cte`]: a query, or — PostgreSQL's data-modifying
/// CTE — a DML statement whose `RETURNING` rows the outer query reads
/// (`WITH t AS (DELETE FROM x RETURNING *) SELECT * FROM t`).
///
/// A closed enum of exactly PostgreSQL's `PreparableStmt` set (`gram.y`
/// `common_table_expr`: `SELECT`/`INSERT`/`UPDATE`/`DELETE`/`MERGE`, the `MERGE`
/// arm PG 17+) — never a general statement carrier, so a utility statement in
/// query position stays unrepresentable (`WITH t AS (VACUUM)` is a PostgreSQL
/// parse error; probed on pg_query 17). `RETURNING` is *not* required at the
/// parse layer — PostgreSQL parses a DML body without one and only its *use*
/// fails at analysis — so the DML arms carry their nodes unrestricted. Each DML
/// arm reuses its statement family's node, which carries its own leading `WITH`
/// (`WITH t AS (WITH u AS (…) INSERT …)` nests exactly as PostgreSQL parses it),
/// boxed to keep `Cte` lean. The DML arms are gated by
/// [`MutationSyntax::data_modifying_ctes`](crate::dialect::MutationSyntax):
/// DuckDB parse-rejects them (`A CTE needs a SELECT`, probed on 1.5.4), as do
/// SQLite and MySQL (`ER_PARSE_ERROR` 1064, probed on mysql:8).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CteBody<X: Extension = NoExt> {
    /// A `SELECT`/query CTE body (the common case).
    Query {
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A data-modifying `INSERT` CTE body.
    Insert {
        /// The `INSERT` statement; see [`Insert`].
        insert: Box<Insert<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A data-modifying `UPDATE` CTE body.
    Update {
        /// The `UPDATE` statement; see [`Update`].
        update: Box<Update<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A data-modifying `DELETE` CTE body.
    Delete {
        /// The `DELETE` statement; see [`Delete`].
        delete: Box<Delete<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A data-modifying `MERGE` CTE body.
    Merge {
        /// The `MERGE` statement; see [`Merge`].
        merge: Box<Merge<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

impl<X: Extension> CteBody<X> {
    /// The body's query when it is one — the overwhelmingly common case — else
    /// `None` for the data-modifying arms (the [`Statement::as_query`](super::Statement)
    /// counterpart for CTE bodies).
    pub fn as_query(&self) -> Option<&Query<X>> {
        match self {
            Self::Query { query, .. } => Some(query),
            Self::Insert { .. }
            | Self::Update { .. }
            | Self::Delete { .. }
            | Self::Merge { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL values.
pub struct Values<X: Extension = NoExt> {
    /// Whether each row is written with the explicit `ROW( ... )` row constructor
    /// (MySQL's query-position spelling — `VALUES ROW(1, 2), ROW(3, 4)`) rather than a
    /// bare `( ... )` row (`VALUES (1, 2), (3, 4)`, the PostgreSQL/DuckDB/SQLite/ANSI
    /// spelling). A single flag, not a per-row bit, because the spelling is uniform across
    /// the constructor: MySQL requires `ROW` on every row and syntax-rejects the bare form
    /// ([`SelectSyntax::values_row_constructor`](crate::dialect::SelectSyntax) off), while
    /// the others require the bare form (that gate on). Preserving it keeps the `ROW`
    /// keyword round-tripping — the same [`explicit`](super::UpdateTupleSource::Row) axis
    /// the `UPDATE ... SET (a, b) = ROW(...)` tuple source and the [`Expr::Row`] value
    /// constructor already carry.
    pub explicit_row: bool,
    /// rows in source order.
    pub rows: ThinVec<ThinVec<ValuesItem<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One item in a `VALUES` query row: an expression or a bare `DEFAULT`.
///
/// PostgreSQL admits a bare `DEFAULT` as a `VALUES` row element (it parses to
/// `SetToDefault`, distinct from a column reference), so a row item is this enum
/// rather than a plain [`Expr`] — keeping `DEFAULT` out of the general expression
/// grammar exactly as the INSERT values path does
/// ([`InsertValue`](super::InsertValue)), and reusing its
/// [`DefaultValue`] leaf.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ValuesItem<X: Extension = NoExt> {
    /// An ordinary value expression.
    Expr {
        /// Expression evaluated by this syntax.
        expr: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bare `DEFAULT` placeholder (PostgreSQL).
    Default {
        /// Explicit `DEFAULT` value.
        default: DefaultValue,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL select.
pub struct Select<X: Extension = NoExt> {
    /// The `ALL` / `DISTINCT` / `DISTINCT ON (...)` set quantifier, or `None` when
    /// the SELECT writes no quantifier (the implicit `ALL`).
    pub distinct: Option<SelectDistinct<X>>,
    /// MySQL's `SELECT STRAIGHT_JOIN ...` modifier — the query-wide form of the
    /// [`JoinOperator::Inner`] `straight` join-order hint, written after the
    /// `DISTINCT`/`ALL` quantifier. A flag (not a node) because it is a surface
    /// modifier that rides `Select` exactly as `distinct` does; only MySQL parses it
    /// (gated by [`JoinSyntax::straight_join`](crate::dialect::TableExpressionSyntax)).
    pub straight_join: bool,
    /// projection in source order.
    pub projection: ThinVec<SelectItem<X>>,
    /// PostgreSQL's `SELECT … INTO <table>` create-table target, written between the
    /// projection and `FROM`; `None` for every standard SELECT. `Box`ed because the
    /// clause is rare (gated to PostgreSQL via
    /// [`SelectSyntax::select_into`](crate::dialect::SelectSyntax)) while `Select` is
    /// a hot node, so the common case pays one pointer, not the inline
    /// target. This is the *materialize-into-a-new-table* form; the SQL-standard
    /// `SELECT … INTO <variable>` (PSM host/local-variable assignment) is a different
    /// construct and is deliberately not modelled here.
    pub into: Option<Box<IntoTarget>>,
    /// from in source order.
    pub from: ThinVec<TableWithJoins<X>>,
    /// Hive/Spark `LATERAL VIEW [OUTER] explode(col) t AS a, b` generator clauses,
    /// written after the whole `FROM` clause and before `WHERE`, each cross-joining the
    /// rows a table-generating function produces; empty for every SELECT that writes
    /// none. See [`LateralView`]. Gated by
    /// [`SelectSyntax::lateral_view_clause`](crate::dialect::SelectSyntax) — on for
    /// Hive/Databricks/Lenient, so the field stays empty elsewhere. A `ThinVec` because
    /// the clause is repeatable and rare while `Select` is a hot node: the empty vector
    /// is one pointer (the [`Query::pipe_operators`] precedent), so the common case
    /// pays one word.
    pub lateral_views: ThinVec<LateralView<X>>,
    /// Predicate that filters input rows.
    pub selection: Option<Expr<X>>,
    /// The Oracle-style `[START WITH <cond>] CONNECT BY [NOCYCLE] <cond>` hierarchical
    /// query clause, written after `WHERE` and before `GROUP BY`; `None` for every
    /// SELECT that writes none. See [`HierarchicalClause`]. Gated by
    /// [`SelectSyntax::connect_by_clause`](crate::dialect::SelectSyntax) — on for
    /// Snowflake and the Lenient union, so the field stays `None` elsewhere.
    /// `Option<Box<…>>` because the clause is rare while `Select` is a hot node: the
    /// common (absent) case is one null pointer (the [`Select::into`] precedent), and
    /// boxing the whole `START WITH`/`CONNECT BY` pair keeps both of its inline `Expr`s
    /// off the `Select` footprint.
    pub connect_by: Option<Box<HierarchicalClause<X>>>,
    /// The `GROUP BY` list. Each item is a [`GroupByItem`]: an ordinary grouping
    /// expression or one of the SQL:1999 grouping-set constructs
    /// (`ROLLUP`/`CUBE`/`GROUPING SETS`/empty `()`), which PostgreSQL lowers in this
    /// position and are therefore their own grammar node rather than
    /// [`FunctionCall`] expressions.
    pub group_by: ThinVec<GroupByItem<X>>,
    /// PostgreSQL's `GROUP BY {DISTINCT | ALL} <grouping items>` set-quantifier
    /// (SQL:2016 feature T434): a quantifier on the *whole* grouping clause that
    /// governs deduplication of the grouping sets the items generate — `DISTINCT`
    /// collapses duplicate sets, `ALL` (the default) keeps them. `None` when the
    /// clause writes no quantifier; `Some(SetQuantifier::All)` /
    /// `Some(SetQuantifier::Distinct)` record the explicit spelling so rendering
    /// round-trips it (the [`Select::distinct`] precedent for the projection
    /// quantifier). The quantifier prefixes the [`group_by`](Self::group_by) list and
    /// requires it to be non-empty — PostgreSQL rejects a bare `GROUP BY ALL` /
    /// `GROUP BY DISTINCT` (verified on pg_query PG-17), which is exactly what keeps it
    /// MECE with [`group_by_all`](Self::group_by_all): this quantifier is *ALL as a
    /// modifier of a non-empty item list*, whereas DuckDB's `GROUP BY ALL` mode is
    /// *ALL standing alone as the entire clause* (empty item list). Gated by
    /// [`GroupingSyntax::group_by_set_quantifier`](crate::dialect::SelectSyntax).
    pub group_by_quantifier: Option<SetQuantifier>,
    /// DuckDB's `GROUP BY ALL` mode: group by every non-aggregated projection
    /// column, resolved at bind time. A flag with an empty
    /// [`group_by`](Self::group_by) list rather than a [`GroupByItem`] variant
    /// because `ALL` is a *mode of the whole clause*, never one grouping item —
    /// DuckDB rejects mixing it with explicit keys or grouping sets (`GROUP BY
    /// ALL, x` / `GROUP BY ROLLUP(x), ALL` are syntax errors; probed on 1.5.4) —
    /// and the key list is unknowable at parse time, so a synthetic item would be
    /// a lie (the resolved shape decision). DuckDB's own tree
    /// corroborates the mode framing: `GROUP BY ALL` serializes as
    /// `aggregate_handling: FORCE_AGGREGATES` with empty `group_expressions`. A
    /// bare flag like [`straight_join`](Self::straight_join); the parser never
    /// sets it alongside a non-empty `group_by`. `None` when the clause writes no
    /// `ALL` mode; `Some(_)` records which surface spelling opened it — DuckDB writes
    /// the mode two interchangeable ways, the keyword `GROUP BY ALL` and the
    /// shorthand `GROUP BY *` (both bind to "group by every non-aggregated projection
    /// column"; `*` is bare-only, DuckDB rejects `GROUP BY *, x` — probed on 1.5.4),
    /// so the spelling is data the renderer round-trips (the [`GroupByAllSpelling`]
    /// tag) rather than a normalized flag. Gated by
    /// [`GroupingSyntax::group_by_all`](crate::dialect::SelectSyntax).
    pub group_by_all: Option<GroupByAllSpelling>,
    /// Predicate applied after grouping.
    pub having: Option<Expr<X>>,
    /// windows in source order.
    pub windows: ThinVec<NamedWindow<X>>,
    /// DuckDB's `QUALIFY <predicate>` clause: a filter over window-function results,
    /// applied after grouping. A distinct slot rather than a spelling of `HAVING`
    /// (which filters groups) because the two have different semantics; QUALIFY is a
    /// common cross-dialect extension (Teradata-origin; Snowflake/BigQuery/DuckDB), so
    /// this is the common shape anchored where the standard is silent. Sits
    /// after [`windows`](Self::windows) because DuckDB's grammar places the clause
    /// after the `WINDOW` clause (`… HAVING … WINDOW … QUALIFY …`; `QUALIFY … WINDOW …`
    /// is a DuckDB syntax error — verified against DuckDB 1.5.4). `Box`ed because the
    /// clause is rare (gated to DuckDB via
    /// [`SelectSyntax::qualify`](crate::dialect::SelectSyntax)) while `Select` is a
    /// hot node, so the common case pays one pointer (the
    /// [`into`](Self::into) precedent). Whether the predicate references a window
    /// function is a bind-time check (DuckDB: "at least one window function must
    /// appear…"), past the parse-level contract — the parser accepts any expression.
    pub qualify: Option<Box<Expr<X>>>,
    /// DuckDB's `USING SAMPLE <entry>` query-level sample clause, written after
    /// [`qualify`](Self::qualify) and before the enclosing query's `ORDER BY`
    /// (`… QUALIFY … USING SAMPLE 3 ORDER BY …`; the reverse order is a DuckDB syntax
    /// error, verified against 1.5.4). `Box`ed because the clause is rare (gated to
    /// DuckDB via [`QueryTailSyntax::using_sample`](crate::dialect::SelectSyntax)) while
    /// `Select` is a hot node, so the common case pays one pointer (the
    /// [`qualify`](Self::qualify)/[`into`](Self::into) precedent). `None` when unwritten.
    pub sample: Option<Box<SampleClause>>,
    /// The surface syntax that produced this SELECT body — an ordinary `SELECT …`
    /// or the `TABLE <name>` short form. Kept as data so the renderer round-trips the
    /// written spelling (the [`LimitSyntax`] precedent).
    pub spelling: SelectSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Surface syntax that produced a [`Select`] body.
///
/// One SELECT semantic, three spellings kept as data (mirroring
/// [`LimitSyntax`]/[`RollupSpelling`]): the ordinary `SELECT <projection> …`, the
/// standard `TABLE <name>` short form (`<explicit table>`), and DuckDB's FROM-first
/// order (`FROM <tables> [SELECT …]`). All lower to the same star- or explicit-projection
/// `Select` — PostgreSQL lowers `TABLE <name>` to `SELECT * FROM <name>`, and DuckDB's own
/// tree serializes `FROM t SELECT x` identically to `SELECT x FROM t` (and bare `FROM t`
/// identically to `SELECT * FROM t`; probed on 1.5.4). Canonicalizing each surface into
/// [`Select`] keeps the differential oracle comparing one shape and the set-operation
/// grammar composing (`TABLE a UNION TABLE b`, `FROM a SELECT x UNION FROM b SELECT y`);
/// the tag exists only so rendering reproduces the written order rather than normalizing
/// every form to `SELECT <projection> FROM …`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SelectSpelling {
    /// An ordinary `SELECT <projection> [FROM …] …` body; the construction default.
    Select,
    /// The `TABLE <name>` short form for `SELECT * FROM <name>` (SQL `<explicit
    /// table>`). The projection is a single wildcard and the `FROM` is the one named
    /// relation (with its optional PostgreSQL `ONLY`/`*` inheritance marker); the
    /// renderer re-emits `TABLE <name>`.
    TableCommand,
    /// DuckDB's FROM-first order: `FROM <tables> [SELECT [DISTINCT] <projection>] …`,
    /// where the `FROM` clause leads and the projection follows it — or is omitted, the
    /// bare `FROM <tables>` form whose implicit projection is a single wildcard. The
    /// stored [`Select`] is the ordinary shape (`from`, `projection`, and the tail
    /// clauses fill exactly as for [`Select`](Self::Select)); this tag only records that
    /// the source wrote the `FROM` first, so the renderer re-emits that order. The bare
    /// form round-trips to `FROM <tables>` (the `SELECT *` stays implicit) and an explicit
    /// `FROM <tables> SELECT *` normalizes onto it — one canonical render for the
    /// wildcard projection, not a second tag state (the corpus writes the bare form far
    /// more often). Gated by
    /// [`SelectSyntax::from_first`](crate::dialect::SelectSyntax); reachable only where a
    /// query primary may begin, so it composes in every query position.
    FromFirst,
}

/// Surface spelling of DuckDB's `GROUP BY ALL` mode ([`Select::group_by_all`]).
///
/// One mode, two interchangeable spellings kept as data (the [`SelectSpelling`] /
/// [`RollupSpelling`] precedent): the keyword `GROUP BY ALL` and the shorthand
/// `GROUP BY *`. Both name the same bind-time instruction — group by every
/// non-aggregated projection column — so they collapse to one semantic in the
/// differential oracle; the tag exists only so a source-fidelity render reproduces
/// the written form rather than normalizing `*` onto `ALL`. A `TargetDialect`
/// re-spell and the redacted fingerprint canonicalize to `ALL` (the shared
/// spelling-tag doctrine, keyed on `honours_source_spelling`). Fieldless because
/// the mode carries no expression — the key list is unknowable at parse time
/// (the [`Select::group_by_all`] rationale).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum GroupByAllSpelling {
    /// The keyword form `GROUP BY ALL`; the construction default.
    Keyword,
    /// DuckDB's shorthand `GROUP BY *` — a bare wildcard standing for the whole
    /// clause (never a grouping key), which the renderer re-emits as `*`.
    Star,
}

/// The destination table of a PostgreSQL `SELECT … INTO <table>` query: the new
/// relation the result rows are materialized into.
///
/// This models PostgreSQL's create-table form of `SELECT INTO`, equivalent to
/// `CREATE TABLE <name> AS <query>`. It reuses [`TemporaryTableKind`] for the
/// `TEMP`/`TEMPORARY` spelling so the surface round-trips exactly and the temporary
/// axis stays one canonical shape with `CREATE TABLE`; `None` is a plain
/// (non-temporary) target. The clause carries no extension data, so it is not
/// generic over `X`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct IntoTarget {
    /// `SELECT … INTO TEMP`/`TEMPORARY <table>` marker; `None` for a permanent table.
    pub temporary: Option<TemporaryTableKind>,
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One item in a `GROUP BY` list: an ordinary grouping expression or one of the
/// SQL:1999 grouping-set constructs.
///
/// PostgreSQL's grammar (`group_by_item`) lowers `ROLLUP (…)`, `CUBE (…)`,
/// `GROUPING SETS (…)`, and the empty grouping set `()` to grouping-set nodes in
/// GROUP BY item position in *any* case spelling — a user function named
/// `rollup`/`cube` cannot be called there without quoting it (`"rollup"(…)` stays a
/// [`FunctionCall`]) — so these are their own grammar position, never
/// ordinary expressions. Acceptance is gated by
/// [`GroupingSyntax::grouping_sets`](crate::dialect::SelectSyntax); when off the
/// keywords fall through to the expression grammar as ordinary function calls, which
/// is how MySQL — whose only grouping surface is the different `WITH ROLLUP` — reads
/// them.
///
/// `ROLLUP`/`CUBE` take a plain expression list, but `GROUPING SETS` nests PG's
/// `group_by_list`, so its members are this same node — admitting
/// `GROUPING SETS (ROLLUP (a, b), (c), ())`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum GroupByItem<X: Extension = NoExt> {
    /// An ordinary grouping expression — a column, a parenthesized `(a, b)` row, or
    /// any other expression (PG's `a_expr` group-by item).
    Expr {
        /// Expression evaluated by this syntax.
        expr: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `ROLLUP` grouping: hierarchical super-aggregate subtotals over the prefixes
    /// of the listed expressions. `spelling` records the surface that produced it —
    /// the SQL:1999 item form `ROLLUP (a, b)` or MySQL's trailing `a, b WITH ROLLUP` —
    /// so rendering round-trips the source; the two are one semantic, spelling kept as
    /// data (the [`LimitSyntax`] precedent).
    Rollup {
        /// exprs in source order.
        exprs: ThinVec<Expr<X>>,
        /// Exact source spelling retained for faithful rendering.
        spelling: RollupSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CUBE (a, b, …)`: super-aggregate subtotals over every subset of the listed
    /// expressions.
    Cube {
        /// exprs in source order.
        exprs: ThinVec<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `GROUPING SETS (<item>, …)`: an explicit list of grouping sets, each itself a
    /// grouping item — so `ROLLUP`/`CUBE`/nested `GROUPING SETS`/`()` may appear
    /// inside.
    GroupingSets {
        /// sets in source order.
        sets: ThinVec<GroupByItem<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The empty grouping set `()` — the grand total. Admitted both as a bare
    /// GROUP BY item and inside `GROUPING SETS` (PG's `empty_grouping_set`).
    Empty {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Surface syntax that produced a [`GroupByItem::Rollup`].
///
/// One `ROLLUP` semantic, two spellings kept as data (mirroring
/// [`LimitSyntax`]): the SQL:1999 item form `ROLLUP (a, b)` and MySQL's trailing
/// modifier `GROUP BY a, b WITH ROLLUP`. Canonicalizing MySQL's surface into this
/// same node lets the differential oracle see one shape; the tag exists only so
/// rendering reproduces the written form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum RollupSpelling {
    /// The SQL:1999 item form `ROLLUP (a, b)`; the construction default.
    Function,
    /// MySQL's trailing modifier, written after the key list: `a, b WITH ROLLUP`.
    WithRollup,
}

/// Surface spelling for an alias introducer on a [`SelectItem::Expr`] projection
/// alias or a [`TableAlias`] correlation name.
///
/// One canonical alias, kept as data so the source form round-trips: SQL makes the
/// `AS` keyword optional before an alias (`SELECT a b` == `SELECT a AS b`,
/// `FROM t u` == `FROM t AS u`), and DuckDB additionally admits a prefix form
/// (`SELECT alias: expr`). The AST keeps one alias field and this tag records which
/// introducer the source wrote, mirroring [`EqualsSpelling`](crate::ast::EqualsSpelling)
/// / [`QuoteStyle`](crate::ast::QuoteStyle): a fieldless `Copy` tag, not a semantic
/// distinction. All three forms name the same alias.
///
/// A synthesized alias (no source, or a rewrite) takes [`As`](Self::As), the
/// canonical introducer a `TargetDialect` render always emits; a `PreserveSource`
/// render honours the tag so a bare alias stays bare. [`PrefixColon`](Self::PrefixColon)
/// is reachable only on a [`SelectItem::Expr`] alias — the table-factor prefix form
/// (`FROM b : a`) folds onto its trailing alias slot and canonicalizes to `As`, since
/// its correlation name renders after the relation, not before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AliasSpelling {
    /// The alias written with no introducer: `SELECT a b`, `FROM t u`.
    Bare,
    /// The explicit `AS` introducer: `SELECT a AS b`, `FROM t AS u`. Also the
    /// synthetic/canonical default.
    As,
    /// DuckDB's prefix form `SELECT alias: expr` — the alias written before the
    /// value. Reachable only on a [`SelectItem::Expr`] alias.
    PrefixColon,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL select item forms represented by the AST.
pub enum SelectItem<X: Extension = NoExt> {
    /// An unqualified `*` projection (with optional DuckDB `EXCLUDE`/`REPLACE`/`RENAME` modifiers).
    Wildcard {
        /// DuckDB's `EXCLUDE`/`REPLACE`/`RENAME` wildcard modifiers
        /// ([`SelectSyntax::wildcard_modifiers`](crate::dialect::SelectSyntax)), which
        /// change *which* columns the `*` expands to. `None` for a plain `*` — the
        /// overwhelming common case, so the modifiers are boxed off the hot projection
        /// item rather than widening every wildcard.
        options: Option<Box<WildcardOptions<X>>>,
        /// DuckDB's alias on a star projection: `SELECT * AS idx` names *every*
        /// star-expanded column `idx` (a rename-all, not a struct pack; engine-probed on
        /// 1.5.4). It rides the same
        /// [`SelectSyntax::wildcard_modifiers`](crate::dialect::SelectSyntax) gate as the
        /// modifiers and is written *after* them (`* EXCLUDE (a) AS idx`); `None` for the
        /// common unaliased `*`.
        alias: Option<Ident>,
        /// How the source introduced `alias`. Meaningful only when `alias` is `Some`;
        /// [`AliasSpelling::As`] (the canonical default) when there is no alias. Only
        /// [`Bare`](AliasSpelling::Bare) / [`As`](AliasSpelling::As) are reachable here —
        /// the [`PrefixColon`](AliasSpelling::PrefixColon) form has no star spelling.
        alias_spelling: AliasSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A qualified `t.*` / `s.t.*` projection.
    QualifiedWildcard {
        /// The relation the star expands (`t` in `t.*`).
        name: ObjectName,
        /// DuckDB's wildcard modifiers on a qualified `t.*`; see
        /// [`Wildcard`](Self::Wildcard). `None` for a plain `t.*`.
        options: Option<Box<WildcardOptions<X>>>,
        /// DuckDB's alias on a qualified star: `SELECT t.* AS x`; see
        /// [`Wildcard`](Self::Wildcard). `None` for a plain `t.*`.
        alias: Option<Ident>,
        /// How the source introduced `alias`; see [`Wildcard`](Self::Wildcard).
        alias_spelling: AliasSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A projected value expression with an optional alias.
    Expr {
        /// Expression evaluated by this syntax.
        expr: Expr<X>,
        /// Alias assigned by this syntax.
        alias: Option<Ident>,
        /// How the source introduced `alias`. Meaningful only when `alias` is
        /// `Some`; [`AliasSpelling::As`] (the canonical default) when there is no
        /// alias, and never rendered in that case.
        alias_spelling: AliasSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// DuckDB's wildcard modifiers, the `EXCLUDE`/`REPLACE`/`RENAME` tail that follows a
/// `*` / `t.*` (and the `COLUMNS(*)` star, [`Expr::Columns`]).
///
/// A new canonical shape, not a spelling tag: the modifiers change which
/// columns the wildcard expands to — genuine semantics with no equivalent standard
/// spelling to fold onto. DuckDB's own tree confirms the shape, carrying
/// `exclude_list`/`replace_list`/`rename_list` on its `STAR` node (probed on 1.5.4).
/// DuckDB fixes the surface order `EXCLUDE`, then `REPLACE`, then `RENAME`, each at
/// most once (a different order is a syntax error), so the three lists are stored
/// separately and rendered back in that canonical order; only present lists are
/// written. Reachable only under
/// [`SelectSyntax::wildcard_modifiers`](crate::dialect::SelectSyntax).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct WildcardOptions<X: Extension = NoExt> {
    /// `EXCLUDE (a, t.b)`: columns dropped from the expansion. Each entry is a column
    /// reference that may be qualified (DuckDB accepts `EXCLUDE (t.a)`), so an
    /// [`ObjectName`] rather than a bare [`Ident`] — this unifies DuckDB's split
    /// `exclude_list` (unqualified) / `qualified_exclude_list` (qualified).
    pub exclude: ThinVec<ObjectName>,
    /// `REPLACE (expr AS col, …)`: columns whose value is swapped for an expression
    /// while keeping their position.
    pub replace: ThinVec<WildcardReplace<X>>,
    /// `RENAME (col AS new, …)`: columns renamed in the expansion.
    pub rename: ThinVec<WildcardRename>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `REPLACE (expr AS col)` entry of a [`WildcardOptions`]: the replacement
/// `expr` and the output column `column` it stands in for. DuckDB's replaced column
/// name is always unqualified (an output label), hence an [`Ident`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct WildcardReplace<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Expr<X>,
    /// Column referenced by this syntax.
    pub column: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `RENAME (col AS new)` entry of a [`WildcardOptions`]: the source `column`
/// (which DuckDB permits to be qualified, e.g. `t.a`) renamed to the unqualified
/// output name `alias`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct WildcardRename {
    /// Column referenced by this syntax.
    pub column: ObjectName,
    /// Alias assigned by this syntax.
    pub alias: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The standard SQL set quantifier `ALL` / `DISTINCT`.
///
/// `ALL` is the explicit spelling of the default (no deduplication); it is kept
/// distinct from "no quantifier" so the surface round-trips, mirroring how
/// [`OrderByExpr::asc`] preserves an explicit `ASC`. This is the quantifier an
/// aggregate call carries ([`FunctionCall::quantifier`](super::FunctionCall)); a
/// SELECT list reuses it inside [`SelectDistinct`], which also admits PostgreSQL's
/// `DISTINCT ON`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SetQuantifier {
    /// Explicit `ALL` — the default, no deduplication.
    All,
    /// `DISTINCT` deduplication.
    Distinct,
}

/// A SELECT-list set quantifier: the standard [`SetQuantifier`] (`ALL`/`DISTINCT`)
/// or PostgreSQL's `DISTINCT ON (<expr>, ...)`.
///
/// The enclosing [`Select::distinct`] is `None` when the SELECT writes no
/// quantifier; the `ON` variant carries the deduplication keys, which only PG's
/// `DISTINCT ON` admits (so it has no [`SetQuantifier`] counterpart).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SelectDistinct<X: Extension = NoExt> {
    /// A standard `ALL` or `DISTINCT` quantifier.
    Quantifier {
        /// Whether `ALL` or `DISTINCT`; see [`SetQuantifier`].
        quantifier: SetQuantifier,
        /// Source location and node identity.
        meta: Meta,
    },
    /// PostgreSQL `DISTINCT ON (<expr>, ...)`: deduplicate on the listed keys only.
    On {
        /// exprs in source order.
        exprs: ThinVec<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL table with joins.
pub struct TableWithJoins<X: Extension = NoExt> {
    /// The leading table factor (the first `FROM` item).
    pub relation: TableFactor<X>,
    /// joins in source order.
    pub joins: ThinVec<Join<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL table alias.
pub struct TableAlias {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Columns in source order.
    pub columns: ThinVec<Ident>,
    /// How the source introduced this correlation name (`FROM t AS u` vs
    /// `FROM t u`). [`AliasSpelling::As`] for a synthesized alias.
    pub spelling: AliasSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Source spelling for PostgreSQL inheritance suppression.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum OnlySyntax {
    /// Source used the `BARE` spelling.
    Bare,
    /// Source used the `PARENTHESIZED` spelling.
    Parenthesized,
}

/// PostgreSQL `relation_expr` inheritance marker on a table reference: whether a
/// query reaches the relation's descendant (inheritance-child) tables, plus the
/// source spelling that asked for it.
///
/// One canonical shape for the four legal `relation_expr` spellings,
/// chosen so the impossible `ONLY name *` combination is structurally
/// unrepresentable: the `*` and `ONLY` markers are sibling variants that can
/// never co-occur. `Plain` and `Descendants` are semantically identical in
/// PostgreSQL (both leave `inh = true`, so a bare `t` and an explicit `t *`
/// select the same rows); the distinct variant exists only to round-trip the
/// source `*` exactly.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum RelationInheritance {
    /// Bare `name`: descendant tables included implicitly.
    Plain,
    /// `name *`: descendant tables included via the explicit legacy star marker.
    Descendants,
    /// `ONLY name` / `ONLY (name)`: descendant tables suppressed.
    Only(OnlySyntax),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL table sample.
pub struct TableSample<X: Extension = NoExt> {
    /// The sampling method name (`BERNOULLI`, `SYSTEM`, …).
    pub method: ObjectName,
    /// Arguments in source order.
    pub args: ThinVec<Expr<X>>,
    /// Optional repeatable for this syntax.
    pub repeatable: Option<Box<Expr<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A table version / time-travel modifier on a base-table factor, written between the
/// table name and its correlation alias (`FROM t FOR SYSTEM_TIME AS OF <ts> AS e`) — the
/// [`version`](TableFactor::Table::version) slot of a [`TableFactor::Table`].
///
/// Available to a planner as a typed value, so a time-travel query is recognized without
/// string-inspecting the source. This reshapes sqlparser-rs's two-arm `TableVersion`
/// (`ForSystemTimeAsOf(Expr)` / `Function(Expr)`), which collapses MSSQL's five distinct
/// `FOR SYSTEM_TIME` temporal forms into one: each spelling is its own variant here, so a
/// planner sees the endpoints (`FROM … TO`, `BETWEEN … AND`, `CONTAINED IN`) directly and
/// the renderer round-trips the written form. The Snowflake `AT`/`BEFORE` function form
/// (sqlparser-rs's `Function` arm) is out of scope — no shipped preset carries it — and is
/// a deferral, not modelled here.
///
/// Gated by
/// [`TableExpressionSyntax::table_version`](crate::dialect::TableExpressionSyntax): on for
/// BigQuery (its `FOR SYSTEM_TIME AS OF`), MSSQL (the five temporal-table forms),
/// Databricks/Delta (`VERSION`/`TIMESTAMP AS OF`), and Lenient; off elsewhere, where the
/// clause keyword is left unconsumed so a query-level `FOR` (locking, `FOR XML`) still
/// parses.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TableVersion<X: Extension = NoExt> {
    /// `FOR SYSTEM_TIME AS OF <expr>` — the point-in-time snapshot shared by BigQuery
    /// (its only spelling) and MSSQL temporal tables.
    ForSystemTimeAsOf {
        /// Point in time selected by this syntax.
        point: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MSSQL `FOR SYSTEM_TIME FROM <start> TO <end>` — the half-open `[start, end)`
    /// row-version range (the endpoint `end` is excluded, unlike `BETWEEN … AND`).
    ForSystemTimeFromTo {
        /// The range start expression.
        start: Box<Expr<X>>,
        /// The range end expression.
        end: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MSSQL `FOR SYSTEM_TIME BETWEEN <start> AND <end>` — the closed-on-both-ends
    /// `[start, end]` row-version range (`end` included, unlike `FROM … TO`).
    ForSystemTimeBetween {
        /// The range start expression.
        start: Box<Expr<X>>,
        /// The range end expression.
        end: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MSSQL `FOR SYSTEM_TIME CONTAINED IN (<start>, <end>)` — rows whose validity
    /// period is fully contained within `[start, end]`.
    ForSystemTimeContainedIn {
        /// The range start expression.
        start: Box<Expr<X>>,
        /// The range end expression.
        end: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MSSQL `FOR SYSTEM_TIME ALL` — every row version, historical and current; the one
    /// endpoint-free temporal form.
    ForSystemTimeAll {
        /// Source location and node identity.
        meta: Meta,
    },
    /// Delta/Databricks `VERSION AS OF <expr>` — a snapshot selected by table version
    /// number.
    VersionAsOf {
        /// Version selected by this syntax.
        version: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// Delta/Databricks `TIMESTAMP AS OF <expr>` — a snapshot selected by timestamp.
    TimestampAsOf {
        /// Point in time selected by this syntax.
        point: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// DuckDB's `USING SAMPLE <entry>` query-level sample specification (its
/// `tablesample_entry` grammar).
///
/// DuckDB admits two surface shapes for the entry that are semantically identical, so
/// they fold to this one canonical shape (the renderer re-derives a
/// method-first spelling): a count-first `<size> [unit] [ '(' method [',' seed] ')' ]`
/// (`USING SAMPLE 3`, `USING SAMPLE 50% (bernoulli)`) and a method-first `method '('
/// <size> [unit] ')' [REPEATABLE '(' seed ')']` (`USING SAMPLE reservoir(20 PERCENT)
/// REPEATABLE (42)`). The size and seed are always numeric literals (DuckDB rejects a
/// negative or general-expression size), so the node carries no expression and is not
/// generic over `X`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct SampleClause {
    /// The sampling method (`reservoir`/`bernoulli`/`system`), or `None` for the bare
    /// count form (`USING SAMPLE 3`). Whether the method led or trailed the count in
    /// source is not preserved — the two orders are equivalent and render method-first.
    pub method: Option<ObjectName>,
    /// The sample size literal (`3`, `50`, `3.5`).
    pub size: Literal,
    /// The size unit: a bare count, an explicit `ROWS`, or a percentage (`PERCENT`
    /// keyword or the `%` sign).
    pub unit: SampleUnit,
    /// The random seed from `REPEATABLE (seed)` or the inline `(method, seed)` form;
    /// `None` when unwritten.
    pub seed: Option<Literal>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The size unit of a [`SampleClause`].
///
/// Four spellings kept as data so the surface round-trips: a bare count
/// (`3`), the explicit `3 ROWS`, and the two percentage spellings `50 PERCENT` and
/// `50%` — distinct surfaces DuckDB accepts for the same percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SampleUnit {
    /// A bare row count with no unit keyword, as in `USING SAMPLE 3`.
    Count,
    /// An explicit `ROWS` count, as in `USING SAMPLE 3 ROWS`.
    Rows,
    /// A `PERCENT`-keyword percentage, as in `USING SAMPLE 10 PERCENT`.
    Percent,
    /// A `%`-sign percentage, as in `USING SAMPLE 10%`.
    PercentSign,
}

/// One typed column in a table function's column definition list, e.g. `id int`
/// in `json_to_record(...) AS x(id int, name text)`.
///
/// PostgreSQL's `TableFuncElement` (`ColId Typename`) is the record-returning
/// counterpart to a plain alias column list: every entry carries a [`DataType`],
/// so a typed definition is never confused with an untyped alias column name (the
/// `columns` of a [`TableAlias`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct TableFunctionColumn<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Data type named by this syntax.
    pub data_type: DataType<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One function inside a `ROWS FROM ( ... )` list with its optional per-function
/// column definition list (PostgreSQL `rowsfrom_item`: `func opt_col_def_list`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct RowsFromItem<X: Extension = NoExt> {
    /// The function call; see [`FunctionCall`].
    pub function: FunctionCall<X>,
    /// column defs in source order.
    pub column_defs: ThinVec<TableFunctionColumn<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One MySQL index hint on a table factor:
/// `{USE|FORCE|IGNORE} {INDEX|KEY} [FOR {JOIN|ORDER BY|GROUP BY}] (<index>, …)`.
///
/// A MySQL-only optimizer directive constraining which indexes the planner may
/// consider for the table. Carries no [`Expr`], so it is not generic over `X`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct IndexHint {
    /// Whether the hint is `USE`/`FORCE`/`IGNORE`; see [`IndexHintAction`].
    pub action: IndexHintAction,
    /// The `INDEX` vs `KEY` spelling — synonyms in MySQL, kept as data so the surface
    /// round-trips (the [`RollupSpelling`] precedent).
    pub keyword: IndexHintKeyword,
    /// The optional `FOR {JOIN|ORDER BY|GROUP BY}` scope restricting the hint to one
    /// planning phase; `None` applies it to all phases.
    pub scope: Option<IndexHintScope>,
    /// The parenthesized index-name list. Empty models the `USE INDEX ()` form (use
    /// no index) — the parentheses are always written, so an empty list is distinct
    /// from the hint's absence (which is the empty
    /// [`TableFactor::Table::index_hints`] list instead).
    pub indexes: ThinVec<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Which indexes a MySQL [`IndexHint`] tells the planner to use, ignore, or force.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IndexHintAction {
    /// `USE INDEX`: consider only the listed indexes (or none, for the empty list).
    Use,
    /// `IGNORE INDEX`: consider every index except the listed ones.
    Ignore,
    /// `FORCE INDEX`: use one of the listed indexes, preferring it over a table scan.
    Force,
}

/// The `INDEX` / `KEY` keyword spelling of a MySQL [`IndexHint`] — exact synonyms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IndexHintKeyword {
    /// The `INDEX` spelling.
    Index,
    /// The `KEY` spelling.
    Key,
}

/// The `FOR {JOIN|ORDER BY|GROUP BY}` scope of a MySQL [`IndexHint`]: the planning
/// phase the hint is restricted to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IndexHintScope {
    /// `FOR JOIN`: row retrieval and join processing.
    Join,
    /// `FOR ORDER BY`: only when resolving the `ORDER BY`.
    OrderBy,
    /// `FOR GROUP BY`: only when resolving the `GROUP BY`.
    GroupBy,
}

/// A SQLite `INDEXED BY <index>` / `NOT INDEXED` index directive on a base-table factor,
/// written after the table name and its optional correlation alias
/// (`FROM t AS e INDEXED BY ix`) — the [`indexed_by`](TableFactor::Table::indexed_by) slot
/// of a [`TableFactor::Table`].
///
/// SQLite's `indexed-clause` on a `qualified-table-name`: `INDEXED BY <name>` forces the
/// named index for the table, while `NOT INDEXED` forbids any index (a full table scan).
/// Distinct from MySQL's [`IndexHint`] and modelled on its own slot rather than folded in:
/// a different grammar (`INDEXED BY name` vs `USE|FORCE|IGNORE INDEX (…)`), a single
/// directive rather than a comma-joined list, and a single-index-or-none choice rather than
/// a planning-scope set — the same modelling split sqlparser-rs draws between its `IndexHint`
/// list and its `TableFactor::Table` index directives. Carries no [`Expr`], so it is not
/// generic over `X`.
///
/// Gated by
/// [`TableExpressionSyntax::indexed_by`](crate::dialect::TableExpressionSyntax): on for
/// SQLite, off elsewhere, where the `INDEXED` keyword is left to the identifier grammar.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IndexedBy {
    /// `INDEXED BY <index>` — force the named index for the table scan.
    Named {
        /// Index referenced by this syntax.
        index: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `NOT INDEXED` — forbid any index; force a full table scan.
    NotIndexed {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One MSSQL / T-SQL table hint inside a `WITH (...)` list on a table factor:
/// `FROM t WITH (NOLOCK)`, `FROM t WITH (INDEX(ix), FORCESEEK)`.
///
/// A T-SQL-only locking / optimizer directive, distinct from the MySQL
/// [`IndexHint`] tail (a different dialect, a different grammar position — MySQL's
/// juxtaposed after the alias, T-SQL's introduced by `WITH (`). Carries no [`Expr`],
/// so it is not generic over `X`. The common documented hints are given typed
/// variants so a downstream planner can key on a specific hint
/// ([`Keyword`](Self::Keyword)) without re-parsing text; an unrecognized single-word
/// hint is preserved verbatim in [`Other`](Self::Other) rather than over-rejecting,
/// the same conservative-round-trip stance the MSSQL preset takes elsewhere. Every
/// variant carries `meta` (each hint is a directly-addressable span). Gated by
/// [`TableExpressionSyntax::table_hints`](crate::dialect::TableExpressionSyntax).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TableHint {
    /// A single-keyword hint (`NOLOCK`, `HOLDLOCK`, `TABLOCK`, …); see
    /// [`TableHintKeyword`] for the modelled set.
    Keyword {
        /// Which modelled hint keyword; see [`TableHintKeyword`].
        keyword: TableHintKeyword,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The `INDEX` access-path hint: `INDEX (<index>, …)`, `INDEX = <index>`, or
    /// `INDEX = (<index>, …)`. `equals` records whether the `=` spelling was used so
    /// both round-trip; the parenthesized and the bare `INDEX = <index>` forms both
    /// fill `indexes`. Numeric index ids (`INDEX(0)`) are a deliberate conservative
    /// deferral — only named indexes are modelled here.
    Index {
        /// Whether the equals form was present in the source.
        equals: bool,
        /// indexes in source order.
        indexes: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `FORCESEEK [ ( <index> ( <column>, … ) ) ]`: force an index seek, optionally
    /// pinned to a named index and a leading column prefix ([`ForceSeekTarget`]).
    /// `None` is the bare `FORCESEEK`.
    ForceSeek {
        /// Object targeted by this syntax.
        target: Option<ForceSeekTarget>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An unrecognized single-word hint, preserved verbatim so the surface round-trips.
    Other {
        /// The unrecognized hint word, preserved verbatim.
        ident: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The optional `( <index> ( <column>, … ) )` argument of a MSSQL `FORCESEEK` table
/// hint ([`TableHint::ForceSeek`]): the index to seek and the leading key columns.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ForceSeekTarget {
    /// The index the seek is forced to use.
    pub index: Ident,
    /// The leading index-key columns the seek is forced over; non-empty (T-SQL
    /// requires at least one column inside the inner parentheses).
    pub columns: ThinVec<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The modelled single-keyword MSSQL table hints ([`TableHint::Keyword`]) — the
/// documented locking, isolation, and planner directives. Each spelling round-trips
/// verbatim via [`TableHintKeyword::as_str`]; the classifier
/// [`TableHintKeyword::from_upper`] maps an uppercased source word back. Words outside
/// this set stay [`TableHint::Other`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TableHintKeyword {
    /// `NOLOCK` — read without shared locks (allows dirty reads).
    NoLock,
    /// `HOLDLOCK` — hold shared locks until the transaction completes (= `SERIALIZABLE`).
    HoldLock,
    /// `UPDLOCK` — take update locks instead of shared locks.
    UpdLock,
    /// `XLOCK` — take exclusive locks.
    XLock,
    /// `ROWLOCK` — force row-level locking granularity.
    RowLock,
    /// `PAGLOCK` — force page-level locking granularity.
    PagLock,
    /// `TABLOCK` — force table-level locking.
    TabLock,
    /// `TABLOCKX` — force an exclusive table-level lock.
    TabLockX,
    /// `READPAST` — skip rows locked by other transactions.
    ReadPast,
    /// `READUNCOMMITTED` — read-uncommitted isolation (= `NOLOCK`).
    ReadUncommitted,
    /// `READCOMMITTED` — read-committed isolation.
    ReadCommitted,
    /// `READCOMMITTEDLOCK` — read-committed isolation enforced with locking.
    ReadCommittedLock,
    /// `REPEATABLEREAD` — repeatable-read isolation.
    RepeatableRead,
    /// `SERIALIZABLE` — serializable isolation.
    Serializable,
    /// `SNAPSHOT` — snapshot isolation.
    Snapshot,
    /// `NOWAIT` — error immediately instead of waiting for a conflicting lock.
    NoWait,
    /// `NOEXPAND` — do not expand indexed views.
    NoExpand,
    /// `FORCESCAN` — force a scan access path.
    ForceScan,
    /// `KEEPIDENTITY` — (bulk insert) keep source identity values.
    KeepIdentity,
    /// `KEEPDEFAULTS` — (bulk insert) apply column defaults for missing values.
    KeepDefaults,
    /// `IGNORE_CONSTRAINTS` — (bulk insert) skip CHECK/foreign-key enforcement.
    IgnoreConstraints,
    /// `IGNORE_TRIGGERS` — (bulk insert) skip trigger firing.
    IgnoreTriggers,
}

impl TableHintKeyword {
    /// The canonical T-SQL spelling of this hint, rendered verbatim.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoLock => "NOLOCK",
            Self::HoldLock => "HOLDLOCK",
            Self::UpdLock => "UPDLOCK",
            Self::XLock => "XLOCK",
            Self::RowLock => "ROWLOCK",
            Self::PagLock => "PAGLOCK",
            Self::TabLock => "TABLOCK",
            Self::TabLockX => "TABLOCKX",
            Self::ReadPast => "READPAST",
            Self::ReadUncommitted => "READUNCOMMITTED",
            Self::ReadCommitted => "READCOMMITTED",
            Self::ReadCommittedLock => "READCOMMITTEDLOCK",
            Self::RepeatableRead => "REPEATABLEREAD",
            Self::Serializable => "SERIALIZABLE",
            Self::Snapshot => "SNAPSHOT",
            Self::NoWait => "NOWAIT",
            Self::NoExpand => "NOEXPAND",
            Self::ForceScan => "FORCESCAN",
            Self::KeepIdentity => "KEEPIDENTITY",
            Self::KeepDefaults => "KEEPDEFAULTS",
            Self::IgnoreConstraints => "IGNORE_CONSTRAINTS",
            Self::IgnoreTriggers => "IGNORE_TRIGGERS",
        }
    }

    /// Classify an already-uppercased source word into a modelled hint keyword, or
    /// `None` when it is not one (the caller keeps it as [`TableHint::Other`]).
    pub fn from_upper(word: &str) -> Option<Self> {
        Some(match word {
            "NOLOCK" => Self::NoLock,
            "HOLDLOCK" => Self::HoldLock,
            "UPDLOCK" => Self::UpdLock,
            "XLOCK" => Self::XLock,
            "ROWLOCK" => Self::RowLock,
            "PAGLOCK" => Self::PagLock,
            "TABLOCK" => Self::TabLock,
            "TABLOCKX" => Self::TabLockX,
            "READPAST" => Self::ReadPast,
            "READUNCOMMITTED" => Self::ReadUncommitted,
            "READCOMMITTED" => Self::ReadCommitted,
            "READCOMMITTEDLOCK" => Self::ReadCommittedLock,
            "REPEATABLEREAD" => Self::RepeatableRead,
            "SERIALIZABLE" => Self::Serializable,
            "SNAPSHOT" => Self::Snapshot,
            "NOWAIT" => Self::NoWait,
            "NOEXPAND" => Self::NoExpand,
            "FORCESCAN" => Self::ForceScan,
            "KEEPIDENTITY" => Self::KeepIdentity,
            "KEEPDEFAULTS" => Self::KeepDefaults,
            "IGNORE_CONSTRAINTS" => Self::IgnoreConstraints,
            "IGNORE_TRIGGERS" => Self::IgnoreTriggers,
            _ => return None,
        })
    }
}

/// Surface syntax that produced a [`TableFactor::Derived`]: the standard
/// parenthesized derived table `( <query> )`, or DuckDB's bare `FROM VALUES (…) AS t`
/// row-list table factor written *without* the surrounding parentheses.
///
/// One derived-table semantic, the paren spelling kept as data (the
/// [`SelectSpelling`] precedent). The bare form's body is always a
/// [`SetExpr::Values`] constructor and it always carries a table alias — DuckDB
/// parse-*requires* one (`FROM VALUES (1) t`, never a bare `FROM VALUES (1)`; probed on
/// 1.5.4) — so the renderer re-emits `VALUES (…) AS t` with no wrapping parentheses,
/// while the [`Parenthesized`](Self::Parenthesized) default re-emits `( <query> )`. The
/// tag is load-bearing: a parenthesized `FROM (VALUES (1) )` and a bare `FROM VALUES (1)
/// t` both hold a `Values` body, so only this spelling tells the renderer whether to
/// wrap. Gated by
/// [`TableFactorSyntax::from_values`](crate::dialect::TableExpressionSyntax).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DerivedSpelling {
    /// The standard parenthesized derived table `( <query> )`; the construction default.
    Parenthesized,
    /// DuckDB's bare `FROM VALUES (…) AS t` row-list table factor (no parentheses).
    BareValues,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL table factor forms represented by the AST.
pub enum TableFactor<X: Extension = NoExt> {
    /// A named table/relation reference, with optional alias, hints, sampling, and time-travel.
    Table {
        /// The table name (one or more dot-separated parts).
        name: ObjectName,
        /// PostgreSQL `ONLY`/`*` inheritance modifier; see [`RelationInheritance`].
        inheritance: RelationInheritance,
        /// A PartiQL / SUPER JSON path navigating into a semi-structured column at the
        /// table-source position (`FROM src[0].a`), attached directly to the table name.
        /// Redshift's SUPER navigation and Snowflake's PartiQL access (sqlparser-rs's
        /// `TableFactor::Table::json_path`, gated by its `supports_partiql`). The path is
        /// entered only by a `[` immediately after the name — a bracket index root, then
        /// `.key` / `[index]` suffixes — so a dotted `FROM src.a.b` stays a compound
        /// [`name`](Self::Table::name), never a path. Empty when absent (the path is always
        /// non-empty when present, so an empty [`ThinVec`] is the unambiguous "no path"
        /// sentinel — the same pattern as [`partition`](Self::Table::partition)). Reuses the
        /// expression-position [`SemiStructuredPathSegment`]
        /// vocabulary. Gated by
        /// [`TableExpressionSyntax::table_json_path`](crate::dialect::TableExpressionSyntax).
        json_path: ThinVec<SemiStructuredPathSegment<X>>,
        /// A version / time-travel modifier (`FOR SYSTEM_TIME AS OF …`, `VERSION AS OF …`),
        /// written between the table name and the alias; `None` when absent. `Box`ed to
        /// keep this hot enum within its size budget (ADR-0007). Gated by
        /// [`TableExpressionSyntax::table_version`](crate::dialect::TableExpressionSyntax);
        /// see [`TableVersion`].
        version: Option<Box<TableVersion<X>>>,
        /// MySQL explicit partition selection `PARTITION (p0, p1)`, written between
        /// the table name and the alias; empty when absent. Restricts the scan to the
        /// named partitions/subpartitions. Gated by
        /// [`TableExpressionSyntax::partition_selection`](crate::dialect::TableExpressionSyntax).
        partition: ThinVec<Ident>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// SQLite `INDEXED BY <index>` / `NOT INDEXED` index directive, written after the
        /// table name and its optional alias (`FROM t AS e INDEXED BY ix`); `None` when
        /// absent. `Box`ed to keep this hot enum within its size budget (ADR-0007). A
        /// separate axis from MySQL [`index_hints`](Self::Table::index_hints): a different
        /// dialect, grammar, and cardinality (see [`IndexedBy`]). Gated by
        /// [`TableExpressionSyntax::indexed_by`](crate::dialect::TableExpressionSyntax).
        indexed_by: Option<Box<IndexedBy>>,
        /// MySQL index hints (`USE|FORCE|IGNORE INDEX|KEY …`), written after the
        /// alias; empty when absent. A list because MySQL admits several comma-joined
        /// hints on one table. Gated by
        /// [`TableExpressionSyntax::index_hints`](crate::dialect::TableExpressionSyntax).
        index_hints: ThinVec<IndexHint>,
        /// Optional sample for this syntax.
        sample: Option<TableSample<X>>,
        /// MSSQL / T-SQL `WITH (...)` table hints (`WITH (NOLOCK)`,
        /// `WITH (INDEX(ix), FORCESEEK)`), written after the alias and the
        /// tablesample clause; empty when absent. A list because T-SQL admits several
        /// comma-joined hints in one `WITH (...)`. A separate axis from
        /// [`index_hints`](Self::Table::index_hints): a different dialect (T-SQL vs
        /// MySQL) and a different grammar position. Gated by
        /// [`TableExpressionSyntax::table_hints`](crate::dialect::TableExpressionSyntax).
        table_hints: ThinVec<TableHint>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A derived table — a parenthesized subquery in `FROM`, optionally `LATERAL`.
    Derived {
        /// Whether the lateral form was present in the source.
        lateral: bool,
        /// The subquery producing the derived rows.
        subquery: Box<Query<X>>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Whether the source wrote the standard parenthesized `( <query> )` or DuckDB's
        /// bare `FROM VALUES (…) AS t` row list (no parentheses); see [`DerivedSpelling`].
        /// A [`BareValues`](DerivedSpelling::BareValues) factor's `subquery` body is
        /// always a [`SetExpr::Values`] and its `alias` is always `Some` (the
        /// parser rejects a bare `FROM VALUES` without one), the invariant the
        /// [`Render`](crate::render::Render) impl relies on to drop the parentheses.
        spelling: DerivedSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A set-returning function used as a table (a table function), optionally `LATERAL`.
    Function {
        /// Whether the lateral form was present in the source.
        lateral: bool,
        /// The table-function call; see [`FunctionCall`].
        function: Box<FunctionCall<X>>,
        /// Whether the with ordinality form was present in the source.
        with_ordinality: bool,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// PostgreSQL `func_alias_clause` column definition list, e.g. the
        /// `(id int, name text)` of `func(...) AS x(id int, name text)`. Empty
        /// unless the function returns an anonymous record typed at the call site.
        column_defs: ThinVec<TableFunctionColumn<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL `ROWS FROM(f1(…), f2(…))` multi-function table factor.
    RowsFrom {
        /// Whether the lateral form was present in the source.
        lateral: bool,
        /// functions in source order.
        functions: ThinVec<RowsFromItem<X>>,
        /// Whether the with ordinality form was present in the source.
        with_ordinality: bool,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A first-class `UNNEST(<expr>[, <expr>…])` table factor: an array/collection
    /// expression expanded into a relation. Modelled as a dedicated node rather than
    /// the generic [`Function`](Self::Function) table function for planner-consumer
    /// parity (the downstream planner keys on a distinct `UNNEST`) — even though
    /// PostgreSQL itself lowers `FROM unnest(…)` to the same `RangeFunction` as any
    /// other set-returning function (its parse tree draws no distinction). Gated by
    /// [`TableFactorSyntax::unnest`](crate::dialect::TableExpressionSyntax);
    /// reached only when `UNNEST` is immediately followed by `(`, so a bare `UNNEST`
    /// stays an ordinary relation name.
    Unnest {
        /// `LATERAL UNNEST(…)`: the array expressions correlate against earlier FROM
        /// items (PostgreSQL `CROSS JOIN LATERAL unnest(t.arr)`).
        lateral: bool,
        /// The unnested array expressions. PostgreSQL admits several (`unnest(a, b)`,
        /// the multi-array zip); DuckDB and BigQuery take exactly one. An empty list
        /// models PostgreSQL's degenerate `unnest()` accept.
        array_exprs: ThinVec<Expr<X>>,
        /// PostgreSQL/DuckDB `WITH ORDINALITY`: append a 1-based ordinal column. BigQuery
        /// has no `WITH ORDINALITY` — it spells the same idea `WITH OFFSET` (0-based).
        with_ordinality: bool,
        /// The correlation alias and its optional untyped column-name list
        /// (`AS u(v, ord)`), read *before* the [`with_offset`](Self::Unnest::with_offset)
        /// tail so both the PostgreSQL (`… WITH ORDINALITY AS u(…)`) and BigQuery
        /// (`… AS u WITH OFFSET`) orderings round-trip.
        alias: Option<Box<TableAlias>>,
        /// PostgreSQL's typed `func_alias_clause` column-definition list
        /// (`unnest(x) AS t(a int)`); empty for the common untyped form. Carried so the
        /// rare typed spelling round-trips losslessly rather than over-rejecting.
        column_defs: ThinVec<TableFunctionColumn<X>>,
        /// BigQuery `WITH OFFSET`: append a 0-based offset column. Gated by
        /// [`TableFactorSyntax::unnest_with_offset`](crate::dialect::TableExpressionSyntax)
        /// — a preset-less flag (no shipped dialect enables it, mirroring
        /// [`QueryTailSyntax::pipe_syntax`](crate::dialect::SelectSyntax)), since only
        /// BigQuery/ZetaSQL accepts the tail and there is no BigQuery oracle yet.
        with_offset: bool,
        /// The BigQuery `WITH OFFSET AS <alias>` column alias; `None` for a bare
        /// `WITH OFFSET`, and always `None` when [`with_offset`](Self::Unnest::with_offset)
        /// is `false`.
        with_offset_alias: Option<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A parenthesized join nested as a table factor: `(t1 JOIN t2 ON …)`.
    NestedJoin {
        /// The parenthesized join tree; see [`TableWithJoins`].
        table: Box<TableWithJoins<X>>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bare SQL special value function used as a table reference (PostgreSQL
    /// `func_table: func_expr_windowless`, e.g. `SELECT * FROM current_date`):
    /// `pg_query` lowers this to a `RangeFunction` wrapping a `SQLValueFunction`,
    /// distinct from an ordinary call — mirrors [`Expr::SpecialFunction`], the same
    /// grammar production in expression position.
    SpecialFunction {
        /// Which special-value function; see [`SpecialFunctionKeyword`].
        keyword: SpecialFunctionKeyword,
        /// The `(precision)` modifier, only valid on the temporal forms (mirrors
        /// [`Expr::SpecialFunction`]'s `precision`).
        precision: Option<u32>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's `<source> PIVOT (<aggregates> FOR <col> IN (<values>) [GROUP BY …])`
    /// table factor. The [`Pivot`] core is shared with the leading-keyword
    /// [`Statement::Pivot`](super::Statement) (tagged
    /// [`PivotSpelling::TableFactor`](super::PivotSpelling)); this position owns the
    /// trailing `AS p` alias the statement form has no place for. `Box`ed — like the
    /// other payload-bearing variants — to keep this hot enum within its size
    /// budget. Gated by
    /// [`TableFactorSyntax::pivot`](crate::dialect::TableExpressionSyntax).
    Pivot {
        /// The pivot operation; see [`Pivot`].
        pivot: Box<Pivot<X>>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's `<source> UNPIVOT [… NULLS] (<value> FOR <name> IN (<cols>))` table
    /// factor — the [`Unpivot`] counterpart of [`Pivot`](Self::Pivot), sharing its core
    /// with [`Statement::Unpivot`](super::Statement). Gated by
    /// [`TableFactorSyntax::unpivot`](crate::dialect::TableExpressionSyntax).
    Unpivot {
        /// The unpivot operation; see [`Unpivot`].
        unpivot: Box<Unpivot<X>>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The SQL:2016 `<source> MATCH_RECOGNIZE (…)` row-pattern-recognition table factor
    /// (Snowflake / Oracle). The [`MatchRecognize`] operator core carries the
    /// `PARTITION BY` / `ORDER BY` / `MEASURES` / rows-per-match / after-match-skip /
    /// `PATTERN` / `SUBSET` / `DEFINE` clauses; this position owns the trailing `AS mr`
    /// alias. `Box`ed — like the other payload-bearing variants — to keep this hot enum
    /// within its size budget (ADR-0007). Gated by
    /// [`TableFactorSyntax::match_recognize`](crate::dialect::TableExpressionSyntax).
    MatchRecognize {
        /// The `MATCH_RECOGNIZE` operator; see [`MatchRecognize`].
        match_recognize: Box<MatchRecognize<X>>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's `DESCRIBE`/`SHOW`/`SUMMARIZE` utility standing as a **table source** —
    /// DuckDB's `SHOW_REF` table reference (`FROM (DESCRIBE SELECT …)`,
    /// `FROM (DESCRIBE PIVOT …)`, `FROM (SHOW databases)`; all probed on 1.5.4). Unlike
    /// [`Pivot`](Self::Pivot)/[`Unpivot`](Self::Unpivot), these are relation-producing
    /// constructs, *not* query bodies — DuckDB parse-rejects them at CTE-body position
    /// (`A CTE needs a SELECT`) while admitting them here — so they get a table-factor
    /// wrapper around the shared [`ShowRef`] core, the shape DuckDB itself uses
    /// (its `SHOW_REF` node carries the same kind + target). This position owns the
    /// trailing `AS t` alias. `Box`ed to keep this hot enum within its size
    /// budget. Gated by
    /// [`TableFactorSyntax::show_ref`](crate::dialect::TableExpressionSyntax).
    ShowRef {
        /// The `DESCRIBE`/`SHOW`/`SUMMARIZE` reference; see [`ShowRef`].
        show: Box<ShowRef<X>>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// SQL/JSON `JSON_TABLE(…)` table factor (SQL:2016) — a JSON document decomposed into a
    /// relation by a `COLUMNS` specification. `Box`ed to keep this hot enum within its size
    /// budget. Gated by
    /// [`TableFactorSyntax::json_table`](crate::dialect::TableExpressionSyntax).
    JsonTable {
        /// The `JSON_TABLE(...)` specification; see [`JsonTable`].
        json_table: Box<JsonTable<X>>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// SQL/XML `XMLTABLE(…)` table factor (SQL:2006) — an XML document decomposed into a
    /// relation by an XPath row expression and per-column paths. `Box`ed to keep this hot enum
    /// within its size budget. Gated by
    /// [`TableFactorSyntax::xml_table`](crate::dialect::TableExpressionSyntax).
    XmlTable {
        /// The `XMLTABLE(...)` specification; see [`XmlTable`].
        xml_table: Box<XmlTable<X>>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// SQL Server's `OPENJSON(<json> [, <path>]) [WITH (<col> <type> [<path>] [AS JSON], …)]`
    /// table factor — a JSON document parsed into a relation, either with the default
    /// key/value/type schema (no `WITH`) or an explicit column schema. `Box`ed to keep this
    /// hot enum within its size budget (ADR-0007). Gated by
    /// [`TableFactorSyntax::open_json`](crate::dialect::TableFactorSyntax::open_json).
    OpenJson {
        /// The `OPENJSON(...)` specification; see [`OpenJson`].
        open_json: Box<OpenJson<X>>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `TABLE(<expr>)` — an arbitrary expression evaluated as a set-returning table
    /// source (sqlparser-rs's `TableFactor::TableFunction`). Distinct from a *named*
    /// table function ([`Function`](Self::Function), `FROM f(1)`), whose head is a
    /// call, not a parenthesized expression, and from the standalone `TABLE t` query
    /// form (`Select::spelling` [`SelectSpelling::TableCommand`](super::SelectSpelling)),
    /// which is a statement-level `<explicit table>`, not a `FROM`-position factor at
    /// all. Only Snowflake and Oracle document this exact shape and neither carries a
    /// differential oracle here, so this is gated
    /// [`TableFactorSyntax::table_expr_factor`](crate::dialect::TableFactorSyntax::table_expr_factor),
    /// on for Lenient only. `Box`ed to keep this hot enum within its size budget.
    TableExpr {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// Alias assigned by this syntax.
        alias: Option<Box<TableAlias>>,
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

impl<X: Extension> TableFactor<X> {
    /// A mutable handle to this factor's correlation-alias slot, or `None` for the
    /// extension [`Other`](Self::Other) variant, which carries no alias.
    ///
    /// Every grammar factor holds an `Option<Box<TableAlias>>`; exposing it uniformly lets
    /// a caller inspect or set the alias without matching all twelve variants — used by the
    /// DuckDB prefix-colon-alias reader (`FROM <alias> : <factor>`) to attach the alias it
    /// parsed ahead of the factor.
    pub fn alias_slot_mut(&mut self) -> Option<&mut Option<Box<TableAlias>>> {
        match self {
            Self::Table { alias, .. }
            | Self::Derived { alias, .. }
            | Self::Function { alias, .. }
            | Self::RowsFrom { alias, .. }
            | Self::Unnest { alias, .. }
            | Self::NestedJoin { alias, .. }
            | Self::SpecialFunction { alias, .. }
            | Self::Pivot { alias, .. }
            | Self::Unpivot { alias, .. }
            | Self::MatchRecognize { alias, .. }
            | Self::ShowRef { alias, .. }
            | Self::JsonTable { alias, .. }
            | Self::XmlTable { alias, .. }
            | Self::OpenJson { alias, .. }
            | Self::TableExpr { alias, .. } => Some(alias),
            Self::Other { .. } => None,
        }
    }
}

/// DuckDB's `SHOW_REF` table reference: a `DESCRIBE` / `SHOW` / `SUMMARIZE` utility
/// statement standing as a relation-producing table source ([`TableFactor::ShowRef`]).
///
/// DuckDB models all three uniformly as one `SHOW_REF` node with a `show_type` tag and a
/// target that is either a query (`DESCRIBE <query>` / `SUMMARIZE <query>`) or a name
/// (`DESCRIBE <table>`, `SHOW databases`) — the canonical shape reproduced here.
///
/// The same core is reused in two grammar positions: inside a parenthesized `FROM`
/// factor ([`TableFactor::ShowRef`]), and — for the `DESCRIBE`/`SUMMARIZE` spellings — as
/// a top-level statement ([`Statement::ShowRef`](crate::ast::Statement)), the form DuckDB
/// desugars to `SELECT * FROM (<SHOW_REF>)`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ShowRef<X: Extension = NoExt> {
    /// Which utility keyword (`DESCRIBE`/`SHOW`/`SUMMARIZE`); see [`ShowRefKind`].
    pub kind: ShowRefKind,
    /// Object targeted by this syntax.
    pub target: ShowRefTarget<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Which utility keyword produced a [`ShowRef`] — DuckDB's `show_type`, kept as data so
/// the renderer round-trips the written keyword.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ShowRefKind {
    /// `DESCRIBE` — column metadata of the target.
    Describe,
    /// `DESC` — the short spelling of DuckDB's `DESCRIBE` utility.
    Desc,
    /// `SHOW` — the unqualified list forms (`SHOW databases`, `SHOW tables`) and
    /// `SHOW <table>`.
    Show,
    /// `SUMMARIZE` — summary statistics of the target.
    Summarize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL show ref target forms represented by the AST.
pub enum ShowRefTarget<X: Extension = NoExt> {
    /// Bare `DESCRIBE` or `DESC`, which DuckDB parses before rejecting at a later semantic
    /// stage.
    Empty {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DESCRIBE <query>` / `SUMMARIZE <query>` — the described query (a `SELECT`,
    /// `PIVOT`, or `UNPIVOT` body).
    Query {
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DESCRIBE <table>`, `SHOW <name>` (`SHOW databases`, `SHOW tables`,
    /// `SHOW <table>`) — the named target.
    Name {
        /// Name referenced by this syntax.
        name: ObjectName,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The payload of a [`TableFactor::JsonTable`] — SQL/JSON `JSON_TABLE` (SQL:2016,
/// PostgreSQL's `JsonTable`).
///
/// The clause vocabulary is shared with the SQL/JSON expression functions: the
/// [`context`](Self::context) reuses [`JsonValueExpr`] (`<doc> [FORMAT JSON …]`), the
/// [`passing`](Self::passing) list reuses [`JsonPassingArg`], and the top-level
/// [`on_error`](Self::on_error) reuses [`JsonBehavior`] — no parallel copies. The row
/// [`path`](Self::path) and every column path are restricted to *string literals* at parse
/// (PostgreSQL: "only string constants are supported in JSON_TABLE path" — a bare column or
/// operator rejects), so they hold a string-literal [`Expr`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct JsonTable<X: Extension = NoExt> {
    /// Whether the lateral form was present in the source.
    pub lateral: bool,
    /// The JSON document value; see [`JsonValueExpr`].
    pub context: JsonValueExpr<X>,
    /// The row path — a string literal (PostgreSQL restricts it to a string constant).
    pub path: Box<Expr<X>>,
    /// Optional path name for this syntax.
    pub path_name: Option<Ident>,
    /// passing in source order.
    pub passing: ThinVec<JsonPassingArg<X>>,
    /// Non-empty — PostgreSQL rejects `COLUMNS ()`.
    pub columns: ThinVec<JsonTableColumn<X>>,
    /// The top-level `<behaviour> ON ERROR`; there is no top-level `ON EMPTY`.
    pub on_error: Option<JsonBehavior<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One column of a [`JsonTable`] `COLUMNS` specification (PostgreSQL's `JsonTableColumn`,
/// tagged by `coltype`).
///
/// The wrapper/quotes/behaviour clauses reuse the SQL/JSON expression-function nodes. The
/// per-kind clause legality is enforced at parse (matching PostgreSQL): only a
/// [`Regular`](Self::Regular) column takes `FORMAT`/wrapper/quotes/`ON EMPTY`;
/// [`Exists`](Self::Exists) takes only `ON ERROR`; [`Nested`](Self::Nested) takes no
/// behaviours and recurses through its own `COLUMNS`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum JsonTableColumn<X: Extension = NoExt> {
    /// `<name> FOR ORDINALITY` — a 1-based row-sequence column; no type, no other clauses.
    ForOrdinality {
        /// Name referenced by this syntax.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `<name> <type> [FORMAT JSON …] [PATH <string>] [wrapper] [quotes] [<b> ON EMPTY]
    /// [<b> ON ERROR]` — a value column projecting the JSON at its path.
    Regular {
        /// Name referenced by this syntax.
        name: Ident,
        /// Data type named by this syntax.
        data_type: Box<DataType<X>>,
        /// Optional format for this syntax.
        format: Option<JsonFormat>,
        /// `PATH <string>` — a string literal; `None` uses the implicit `$.<name>` path.
        path: Option<Box<Expr<X>>>,
        /// The `WITH`/`WITHOUT WRAPPER` behaviour; see [`JsonWrapperBehavior`].
        wrapper: JsonWrapperBehavior,
        /// The `KEEP`/`OMIT QUOTES` behaviour; see [`JsonQuotesBehavior`].
        quotes: JsonQuotesBehavior,
        /// Optional on empty for this syntax.
        on_empty: Option<JsonBehavior<X>>,
        /// Optional on error for this syntax.
        on_error: Option<JsonBehavior<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `<name> <type> EXISTS [PATH <string>] [<b> ON ERROR]` — a boolean-ish column testing
    /// whether the path matches; takes neither `FORMAT`/wrapper/quotes nor `ON EMPTY`.
    Exists {
        /// Name referenced by this syntax.
        name: Ident,
        /// Data type named by this syntax.
        data_type: Box<DataType<X>>,
        /// Optional path for this syntax.
        path: Option<Box<Expr<X>>>,
        /// Optional on error for this syntax.
        on_error: Option<JsonBehavior<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `NESTED [PATH] <string> [AS <name>] COLUMNS ( … )` — a sub-table joined against a
    /// nested path, recursing through its own column list (PostgreSQL requires the nested
    /// `COLUMNS` to be present and non-empty).
    Nested {
        /// The nested JSON path (a string literal).
        path: Box<Expr<X>>,
        /// Optional path name for this syntax.
        path_name: Option<Ident>,
        /// Columns in source order.
        columns: ThinVec<JsonTableColumn<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The payload of a [`TableFactor::XmlTable`] — SQL/XML `XMLTABLE` (SQL:2006, PostgreSQL's
/// `RangeTableFunc`).
///
/// The [`passing_mechanism_before`](Self::passing_mechanism_before)/`_after` reuse the
/// [`XmlPassingMechanism`] of `xmlexists` (`PASSING [BY REF|VALUE] doc [BY REF|VALUE]`);
/// PostgreSQL admits a mechanism on either side and normalizes it away, so it is preserved
/// only for round-trip fidelity. The [`row_expr`](Self::row_expr) and
/// [`document`](Self::document) are `c_expr` operands (a bare `a || b` rejects; parenthesize
/// to re-admit a full expression), matching the engine.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct XmlTable<X: Extension = NoExt> {
    /// Whether the lateral form was present in the source.
    pub lateral: bool,
    /// namespaces in source order.
    pub namespaces: ThinVec<XmlNamespace<X>>,
    /// The row-generating XPath (a `c_expr`).
    pub row_expr: Box<Expr<X>>,
    /// The `PASSING` document (a `c_expr`).
    pub document: Box<Expr<X>>,
    /// Optional passing mechanism before for this syntax.
    pub passing_mechanism_before: Option<XmlPassingMechanism>,
    /// Optional passing mechanism after for this syntax.
    pub passing_mechanism_after: Option<XmlPassingMechanism>,
    /// Columns in source order.
    pub columns: ThinVec<XmlTableColumn<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `XMLNAMESPACES` declaration inside an [`XmlTable`]: `<uri> AS <name>` or
/// `DEFAULT <uri>`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct XmlNamespace<X: Extension = NoExt> {
    /// The namespace URI expression.
    pub uri: Box<Expr<X>>,
    /// `None` for the `DEFAULT <uri>` (unnamed) form.
    pub name: Option<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One column of an [`XmlTable`] `COLUMNS` specification (PostgreSQL's `RangeTableFuncCol`).
///
/// The regular-column options (`PATH`/`DEFAULT`/`NULL`/`NOT NULL`) are order-free at parse —
/// PostgreSQL normalizes them into fixed node fields — so they are stored positionally here
/// and re-rendered in canonical order. PostgreSQL rejects a repeated `PATH`/`DEFAULT` and a
/// conflicting/redundant `NULL`/`NOT NULL` at parse, which the parser reproduces.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum XmlTableColumn<X: Extension = NoExt> {
    /// `<name> FOR ORDINALITY` — a 1-based row-sequence column; no type or options.
    ForOrdinality {
        /// Name referenced by this syntax.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `<name> <type> [PATH <b_expr>] [DEFAULT <b_expr>] [NULL | NOT NULL]`.
    Regular {
        /// Name referenced by this syntax.
        name: Ident,
        /// Data type named by this syntax.
        data_type: Box<DataType<X>>,
        /// Optional path for this syntax.
        path: Option<Box<Expr<X>>>,
        /// Optional default for this syntax.
        default: Option<Box<Expr<X>>>,
        /// `Some(true)` for `NOT NULL`, `Some(false)` for `NULL`, `None` when unwritten.
        not_null: Option<bool>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The payload of a [`TableFactor::OpenJson`] — SQL Server's `OPENJSON` rowset function
/// (sqlparser-rs's `TableFactor::OpenJsonTable`).
///
/// Reshaped from sqlparser-rs per ADR-0011: its `json_expr: Expr` / `json_path: Option<Value>` /
/// per-column `path: Option<String>` become span-bearing [`Expr`] holders here — the row
/// [`path`](Self::path) and every column path are *string literals* (matching JSON_TABLE's
/// [`JsonTable::path`](JsonTable::path)), so they round-trip from their spans — and the column
/// list is a [`ThinVec`]. An absent `WITH` clause is the empty [`columns`](Self::columns) (MSSQL
/// rejects an empty `WITH ()`, so a present clause is always non-empty); the default
/// `key`/`value`/`type` schema then applies.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct OpenJson<X: Extension = NoExt> {
    /// The JSON source expression (a column, variable, or literal), evaluated to a JSON
    /// string. Unrestricted — any [`Expr`], unlike the string-literal-only paths.
    pub json_expr: Box<Expr<X>>,
    /// The optional `, <path>` second argument — a string-literal JSON path selecting the
    /// array/object to iterate; `None` iterates the root value.
    pub path: Option<Box<Expr<X>>>,
    /// The `WITH (…)` explicit column schema; empty when the clause is absent.
    pub columns: ThinVec<OpenJsonColumn<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One column of an [`OpenJson`] `WITH (…)` schema — `<name> <type> [<path>] [AS JSON]`
/// (sqlparser-rs's `OpenJsonTableColumn`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct OpenJsonColumn<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Data type named by this syntax.
    pub data_type: Box<DataType<X>>,
    /// The optional `<column_path>` string literal; `None` uses the implicit `$.<name>` path.
    pub path: Option<Box<Expr<X>>>,
    /// The `AS JSON` marker — the column holds nested JSON (MSSQL requires an
    /// `nvarchar(max)` type).
    pub as_json: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL join.
pub struct Join<X: Extension = NoExt> {
    /// The right-hand table factor being joined.
    pub relation: TableFactor<X>,
    /// The join operator — side and constraint; see [`JoinOperator`].
    pub operator: JoinOperator<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL join operator forms represented by the AST.
pub enum JoinOperator<X: Extension = NoExt> {
    /// An `[INNER] JOIN` — keeps only row pairs that satisfy the constraint.
    Inner {
        /// MySQL `STRAIGHT_JOIN`: an inner join that additionally forces the
        /// optimizer to read the left table before the right. It is semantically a
        /// plain `INNER JOIN`, so it is the canonical inner-join shape
        /// carrying this surface tag (a join-order hint) rather than a new operator
        /// variant — mirroring how [`SetQuantifier::All`] preserves an explicit `ALL`.
        /// `false` is a bare `[INNER] JOIN`; only MySQL parses the `true` spelling
        /// (gated by [`JoinSyntax::straight_join`](crate::dialect::TableExpressionSyntax)).
        straight: bool,
        /// Whether the redundant `INNER` keyword was written (`INNER JOIN` vs a bare
        /// `JOIN` — the two are exact synonyms). A source-fidelity render replays it; a
        /// target re-spell and the redacted fingerprint drop it. Always `false` under
        /// `straight` (`STRAIGHT_JOIN` is its own keyword, never spelled `INNER`).
        inner: bool,
        /// The join condition (`ON`/`USING`/`NATURAL`/none); see [`JoinConstraint`].
        constraint: JoinConstraint<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `LEFT [OUTER] JOIN` — keeps every left row, NULL-padding unmatched right columns.
    LeftOuter {
        /// Whether the redundant `OUTER` keyword was written (`LEFT OUTER JOIN` vs a
        /// bare `LEFT JOIN`). Fidelity only, like [`Inner::inner`](Self::Inner::inner).
        outer: bool,
        /// The join condition (`ON`/`USING`/`NATURAL`/none); see [`JoinConstraint`].
        constraint: JoinConstraint<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `RIGHT [OUTER] JOIN` — keeps every right row, NULL-padding unmatched left columns.
    RightOuter {
        /// Whether the redundant `OUTER` keyword was written (`RIGHT OUTER JOIN`).
        outer: bool,
        /// The join condition (`ON`/`USING`/`NATURAL`/none); see [`JoinConstraint`].
        constraint: JoinConstraint<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `FULL [OUTER] JOIN` — keeps all rows from both sides, NULL-padding non-matches.
    FullOuter {
        /// Whether the redundant `OUTER` keyword was written (`FULL OUTER JOIN`).
        outer: bool,
        /// The join condition (`ON`/`USING`/`NATURAL`/none); see [`JoinConstraint`].
        constraint: JoinConstraint<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB `ASOF [INNER|LEFT|RIGHT|FULL [OUTER]] JOIN`: an inexact-match temporal
    /// join pairing each left row with the nearest right row under the `ON`
    /// inequality. A new operator (nearest-match semantics), not a spelling of the
    /// side joins — DuckDB serializes it as an orthogonal `ref_type: ASOF` on top of
    /// the `join_type` side, mirrored here as [`kind`](Self::AsOf::kind). The engine
    /// *parse*-requires an `ON`/`USING` constraint (a bare `ASOF JOIN` is a syntax
    /// error), so the parser never builds [`JoinConstraint::None`] here; the
    /// inequality requirement itself is bind-time (`ASOF JOIN … ON a = b` parses,
    /// then fails DuckDB's binder), so an equality constraint still parses. Gated by
    /// [`JoinSyntax::asof_join`](crate::dialect::TableExpressionSyntax).
    AsOf {
        /// Which side the `ASOF` join keeps; see [`AsOfJoinKind`].
        kind: AsOfJoinKind,
        /// The join condition (`ON`/`USING`/`NATURAL`/none); see [`JoinConstraint`].
        constraint: JoinConstraint<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `CROSS JOIN` — the unconstrained Cartesian product.
    Cross {
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB `POSITIONAL JOIN`: pairs rows by position (first with first, …). Like
    /// [`Cross`](Self::Cross) it never carries a constraint — the engine
    /// parse-rejects a trailing `ON`/`USING` and any side keyword (`POSITIONAL LEFT
    /// JOIN` is a syntax error) — so the variant has no constraint or kind field.
    /// Gated by
    /// [`JoinSyntax::positional_join`](crate::dialect::TableExpressionSyntax).
    Positional {
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB `[ASOF|NATURAL] SEMI JOIN`: a semi-join — keeps each left row that has
    /// at least one right match, projecting left columns only. DuckDB serializes it as
    /// a `join_type: SEMI` (engine-verified on 1.5.4), *mutually exclusive* with the
    /// `INNER`/`LEFT`/`RIGHT`/`FULL` sides (`LEFT SEMI JOIN` is a syntax error), so it
    /// is a new operator rather than a side spelling. It composes only with the
    /// `REGULAR`, `NATURAL` (`NATURAL SEMI JOIN`, carried as
    /// [`JoinConstraint::Natural`]) and `ASOF` (`ASOF SEMI JOIN`,
    /// [`asof`](Self::Semi::asof) `= true`) ref-types — never a side, `CROSS`, or
    /// `POSITIONAL`. Like [`AsOf`](Self::AsOf) it *parse*-requires an `ON`/`USING`
    /// constraint (a bare `SEMI JOIN` is a syntax error) unless `NATURAL` supplies the
    /// match, so the parser never builds [`JoinConstraint::None`] here. `ASOF` and
    /// `NATURAL` never co-occur (both engine parse-rejected), so `asof: true` always
    /// carries an `ON`/`USING` constraint.
    ///
    /// The [`side`](Self::Semi::side) axis records the Spark/Hive/Databricks *sided*
    /// spelling — `LEFT SEMI JOIN` / `RIGHT SEMI JOIN` — as one operator with DuckDB's
    /// side-less `SEMI JOIN` rather than a separate variant (the
    /// [`AsOfJoinKind`]/[`ApplyKind`] axis precedent): all three are the same semi-join,
    /// differing only in whether an explicit side keyword is written and which side's
    /// rows are tested. The two spellings come from different engine families and are
    /// gated apart — DuckDB's side-less form by
    /// [`JoinSyntax::semi_anti_join`](crate::dialect::TableExpressionSyntax),
    /// the sided form by
    /// [`JoinSyntax::sided_semi_anti_join`](crate::dialect::TableExpressionSyntax)
    /// (DuckDB engine-parse-rejects `LEFT SEMI JOIN`). The two axes are mutually
    /// exclusive: [`SemiAntiSide::Left`]/[`Right`](SemiAntiSide::Right) never carry the
    /// DuckDB-only `ASOF`/`NATURAL` compositions, so a sided operator always has
    /// `asof: false` and an `ON`/`USING` constraint (Spark requires the qualifier).
    Semi {
        /// Whether the asof form was present in the source.
        asof: bool,
        /// Which sided spelling (`LEFT`/`RIGHT`/side-less); see [`SemiAntiSide`].
        side: SemiAntiSide,
        /// The join condition (`ON`/`USING`/`NATURAL`/none); see [`JoinConstraint`].
        constraint: JoinConstraint<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB `[ASOF|NATURAL] ANTI JOIN` / Spark `[LEFT|RIGHT] ANTI JOIN`: an anti-join —
    /// keeps each left row with *no* right match. The [`Semi`](Self::Semi) counterpart:
    /// identical grammar (DuckDB serializes `join_type: ANTI`) with the opposite
    /// membership test, so see [`Semi`](Self::Semi) for the composition, constraint,
    /// `asof`-flag, and [`side`](Self::Anti::side) rules and the two gates.
    Anti {
        /// Whether the asof form was present in the source.
        asof: bool,
        /// Which sided spelling (`LEFT`/`RIGHT`/side-less); see [`SemiAntiSide`].
        side: SemiAntiSide,
        /// The join condition (`ON`/`USING`/`NATURAL`/none); see [`JoinConstraint`].
        constraint: JoinConstraint<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MSSQL `CROSS APPLY` / `OUTER APPLY`: an implicitly-correlated (lateral) join
    /// whose right operand — a derived table `(SELECT …)` or a table-valued function
    /// call — may reference columns of the left source. Like [`Cross`](Self::Cross) it
    /// never carries an `ON`/`USING` constraint (the correlation is positional, in the
    /// right operand's own references), so the variant holds no constraint. The
    /// [`kind`](Self::Apply::kind) axis is the `CROSS`/`OUTER` flavour — inner-style vs
    /// left-style row preservation — mirroring how [`AsOf`](Self::AsOf) records its side
    /// on a `kind` rather than splitting into per-spelling variants: `CROSS APPLY` and
    /// `OUTER APPLY` are one grammar production differing only by that keyword. Gated by
    /// [`JoinSyntax::apply_join`](crate::dialect::TableExpressionSyntax).
    Apply {
        /// The `CROSS`/`OUTER` apply flavour; see [`ApplyKind`].
        kind: ApplyKind,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The flavour of a MSSQL [`JoinOperator::Apply`] operator.
///
/// `CROSS`/`OUTER` are the two spellings of the single `APPLY` grammar production
/// (a lateral join over a right table factor), differing only in row preservation —
/// `CROSS` drops left rows whose right operand is empty, `OUTER` keeps them with
/// nulls — so they are one operator with a two-value axis, not two operators (the
/// [`AsOfJoinKind`] precedent). Only `CROSS` is wired into the parser today; `OUTER`
/// is the sibling `planner-parity-join-outer-apply` extension, and the enum carries
/// it so that landing is a parser-only change with no AST/render churn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ApplyKind {
    /// `CROSS APPLY` — evaluate the right side per left row, like a `LATERAL` inner join.
    Cross,
    /// `OUTER APPLY` — like `CROSS APPLY` but keeps left rows with no right match (`LATERAL` left join).
    Outer,
}

/// The side spelling of a [`JoinOperator::Semi`]/[`JoinOperator::Anti`] operator.
///
/// DuckDB spells the semi-/anti-join side-*less* (`SEMI JOIN` / `ANTI JOIN`, a
/// left-semi/left-anti by definition) and composes it with the `NATURAL`/`ASOF`
/// ref-types; Spark/Hive/Databricks instead *require* an explicit side keyword
/// (`LEFT SEMI JOIN`, `RIGHT ANTI JOIN`) and never compose with those ref-types. The
/// two are the same operator differing only in this surface axis, so — like
/// [`ApplyKind`]/[`AsOfJoinKind`] — the side rides a `kind`-style axis rather than
/// splitting into per-spelling variants.
///
/// [`Sideless`](Self::Sideless) is the DuckDB spelling (its `asof` flag may then be
/// set). [`Left`](Self::Left) is Spark's `LEFT` — semantically the same left-semi as
/// [`Sideless`](Self::Sideless), differing only in whether the keyword is written;
/// [`Right`](Self::Right) is the mirror `RIGHT` (right-row test), genuinely distinct
/// semantics. A sided value always carries `asof: false` and an `ON`/`USING`
/// constraint (Spark requires the qualifier and has no `ASOF`/`NATURAL`).
///
/// Only [`Sideless`](Self::Sideless) and [`Left`](Self::Left) are wired into the
/// parser today; [`Right`](Self::Right) — and the [`Anti`](JoinOperator::Anti)
/// pairing of both sides — ships here so the sibling `RIGHT SEMI` / `LEFT ANTI` /
/// `RIGHT ANTI` tickets land as parser-only changes with no AST/render churn (the
/// [`ApplyKind::Outer`] precedent).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SemiAntiSide {
    /// No side keyword — a plain `SEMI`/`ANTI` join.
    Sideless,
    /// `LEFT SEMI`/`LEFT ANTI`.
    Left,
    /// `RIGHT SEMI`/`RIGHT ANTI`.
    Right,
}

/// The side of a DuckDB [`JoinOperator::AsOf`] join.
///
/// `ASOF` composes with all four standard sides (engine-verified on DuckDB 1.5.4,
/// including the `OUTER` spellings) but not with `NATURAL`/`CROSS`, so this is a
/// dedicated four-side kind rather than a nested [`JoinOperator`]. `Inner` covers
/// both the bare `ASOF JOIN` and the explicit `ASOF INNER JOIN` spelling (the
/// canonical shape records the side, not the spelling, like the side joins).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AsOfJoinKind {
    /// `ASOF [INNER] JOIN`.
    Inner,
    /// `ASOF LEFT JOIN`.
    Left,
    /// `ASOF RIGHT JOIN`.
    Right,
    /// `ASOF FULL JOIN`.
    Full,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL join constraint forms represented by the AST.
pub enum JoinConstraint<X: Extension = NoExt> {
    /// An `ON <predicate>` join condition.
    On {
        /// Expression evaluated by this syntax.
        expr: Expr<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `USING (col, …)` join condition — equate the named common columns.
    Using {
        /// Columns in source order.
        columns: ThinVec<Ident>,
        /// Alias assigned by this syntax.
        alias: Option<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `NATURAL` join — an implicit equi-join on all same-named columns.
    Natural {
        /// Source location and node identity.
        meta: Meta,
    },
    /// No join constraint (a `CROSS JOIN` or comma join).
    None {
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL order by expr.
pub struct OrderByExpr<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Expr<X>,
    /// Whether the asc form was present in the source.
    pub asc: Option<bool>,
    /// PostgreSQL `USING <operator>` sort form (`gram.y` `sortby: a_expr USING
    /// qual_all_Op opt_nulls_order`): sort by a named ordering operator instead of
    /// `ASC`/`DESC`. Mutually exclusive with [`asc`](Self::asc), which stays `None`
    /// when this is `Some`; boxed since it is a rare tail on a common sort key.
    pub using: Option<Box<OrderByUsing>>,
    /// Whether the nulls first form was present in the source.
    pub nulls_first: Option<bool>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `USING <qual_all_Op>` operator of a PostgreSQL `ORDER BY` sort key.
///
/// `qual_all_Op` is a possibly schema-qualified operator: the bare `USING <` form
/// carries no [`schema`](Self::schema) node at all, while `USING
/// OPERATOR(pg_catalog.<)` records the qualification. Modelled like the operator
/// half of [`NamedOperatorExpr`](super::NamedOperatorExpr): the operator
/// is always symbolic, never a word, so it is a bare interned [`Symbol`]. `schema`
/// is `Option` rather than an empty [`ObjectName`] because every present node must
/// carry a real source span (the span-containment walker enforces it) — an empty
/// name would have none.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct OrderByUsing {
    /// Schema qualification (`pg_catalog` in `OPERATOR(pg_catalog.<)`); `None` for
    /// the bare `USING <` form.
    pub schema: Option<ObjectName>,
    /// The operator symbol spelling (`<`, `~<~`), interned exact-case so it
    /// round-trips.
    pub op: Symbol,
    /// Source location and node identity.
    pub meta: Meta,
}

/// DuckDB's `ORDER BY ALL [ASC | DESC] [NULLS FIRST | LAST]` clause mode: sort by
/// every projection column, left to right.
///
/// A first-class marker node, not an [`OrderByExpr`] whose expression spells `ALL`:
/// the sort keys are resolved at bind time from the projection, so there is no
/// expression to carry, and DuckDB's own tree corroborates the framing (it
/// serializes the clause as a single order whose expression is the `COLUMNS(*)`
/// star node — a whole-projection expansion, not a column named `all`). The
/// direction and nulls modifiers ride the clause exactly as they ride an ordinary
/// sort key (`ORDER BY ALL DESC NULLS LAST` is valid; probed on 1.5.4), hence the
/// same `asc`/`nulls_first` surface as [`OrderByExpr`] — but no `USING` (DuckDB
/// rejects `ORDER BY ALL USING <`). Carries no [`Expr`], so it is not generic over
/// the extension parameter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct OrderByAll {
    /// `ASC` (`Some(true)`) / `DESC` (`Some(false)`), or `None` when unwritten —
    /// recording exactly what the source said, mirroring [`OrderByExpr::asc`].
    pub asc: Option<bool>,
    /// `NULLS FIRST` (`Some(true)`) / `NULLS LAST` (`Some(false)`), or `None` when
    /// unwritten, mirroring [`OrderByExpr::nulls_first`].
    pub nulls_first: Option<bool>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Canonical LIMIT/OFFSET node plus original surface spelling.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Limit<X: Extension = NoExt> {
    /// Row limit applied to the result.
    pub limit: Option<Expr<X>>,
    /// Row offset applied before returning results.
    pub offset: Option<Expr<X>>,
    /// Source spelling used for the syntax.
    pub syntax: LimitSyntax,
    /// Whether a written `FETCH { FIRST | NEXT } ...` tail chose `WITH TIES`
    /// (rows tying the last row's `ORDER BY` key are also returned) over the
    /// default `ONLY`, or `None` when no `FETCH` clause was written at all.
    ///
    /// A third state is load-bearing here, not just `bool`: the SQL:2008
    /// `OFFSET ... ROWS` spelling ([`LimitSyntax::FetchFirst`]) admits a `FETCH`
    /// tail whose row count is itself optional (`FETCH FIRST ROWS ONLY`,
    /// PostgreSQL defaults it to 1), so `limit: None` alone cannot tell "no
    /// `FETCH` clause" (`OFFSET 5 ROWS`, unbounded) apart from "`FETCH` written
    /// with no count" (`OFFSET 5 ROWS FETCH FIRST ROWS ONLY`, bounded to 1) —
    /// two different result sets. `with_ties` disambiguates them instead: `None`
    /// is the former, `Some(_)` the latter. Always `None` under
    /// [`LimitSyntax::LimitOffset`], which has no `FETCH` tail at all.
    pub with_ties: Option<bool>,
    /// DuckDB's percentage row limit: the count is a *fraction of rows* rather than a
    /// row number (`LIMIT 40 PERCENT`, `LIMIT 35%` — return 40%/35% of the result).
    /// `None` is the ordinary row-count limit; `Some(_)` records which surface spelling
    /// wrote the marker so the renderer round-trips it (the [`LimitSyntax`]
    /// precedent). Only meaningful with a written [`limit`](Self::limit) count under
    /// [`LimitSyntax::LimitOffset`]; gated to DuckDB via
    /// [`QueryTailSyntax::limit_percent`](crate::dialect::SelectSyntax).
    pub percent: Option<LimitPercent>,
    /// Surface spelling of a written `FETCH { FIRST | NEXT } … { ROW | ROWS }` tail:
    /// which of the interchangeable `FIRST`/`NEXT` and `ROW`/`ROWS` synonyms the source
    /// wrote, so a source-fidelity render replays them. Meaningful only under
    /// [`LimitSyntax::FetchFirst`] with a written `FETCH` (`with_ties` is `Some`); the
    /// canonical render (and a target re-spell / the redacted fingerprint) emit the
    /// canonical `FETCH FIRST … ROWS`. One byte, so it rides the struct's existing
    /// padding — the leaner axis-per-field alternative crossed an alignment word.
    pub fetch_spelling: FetchSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Surface spelling of a written `FETCH { FIRST | NEXT } … { ROW | ROWS }` tail
/// ([`Limit::fetch_spelling`]).
///
/// `FIRST`/`NEXT` and `ROW`/`ROWS` are interchangeable noise words; the canonical AST
/// keeps one shape and this tag records the written pair so a source-fidelity render
/// replays it. The two axes are folded onto one 1-byte enum (rather than two `bool`
/// fields) so the tag rides [`Limit`]'s existing padding instead of growing the node
/// (ADR-0007). A fidelity tag — a target re-spell and the redacted fingerprint emit the
/// canonical [`FirstRows`](Self::FirstRows).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum FetchSpelling {
    /// The canonical `FETCH FIRST … ROWS`.
    #[default]
    FirstRows,
    /// `FETCH FIRST … ROW` (singular row word).
    FirstRow,
    /// `FETCH NEXT … ROWS`.
    NextRows,
    /// `FETCH NEXT … ROW`.
    NextRow,
}

impl FetchSpelling {
    /// Build the tag from the two written-spelling axes: `next` selects `NEXT` over
    /// `FIRST`, `row_singular` the singular `ROW` over `ROWS`.
    pub fn from_axes(next: bool, row_singular: bool) -> Self {
        match (next, row_singular) {
            (false, false) => Self::FirstRows,
            (false, true) => Self::FirstRow,
            (true, false) => Self::NextRows,
            (true, true) => Self::NextRow,
        }
    }

    /// The written keyword: `"FETCH NEXT"` or the canonical `"FETCH FIRST"`.
    pub fn fetch_keyword(self) -> &'static str {
        match self {
            Self::NextRows | Self::NextRow => "FETCH NEXT",
            Self::FirstRows | Self::FirstRow => "FETCH FIRST",
        }
    }

    /// The written row word (with surrounding spaces): `" ROW "` or the canonical
    /// `" ROWS "`.
    pub fn row_word(self) -> &'static str {
        match self {
            Self::FirstRow | Self::NextRow => " ROW ",
            Self::FirstRows | Self::NextRows => " ROWS ",
        }
    }
}

/// Surface syntax used to write a canonical [`Limit`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LimitSyntax {
    /// Source used the `LIMIT OFFSET` spelling.
    LimitOffset,
    /// MySQL/MariaDB/SQLite `LIMIT <offset>, <count>` — the comma spelling of
    /// `LIMIT <count> OFFSET <offset>` (the offset binds first, the count second). The
    /// same row limit, folded onto the canonical [`Limit`] shape; this variant records
    /// the comma spelling so a source-fidelity render replays `LIMIT <offset>, <count>`
    /// while a target re-spell and the redacted fingerprint emit the canonical
    /// `LIMIT <count> OFFSET <offset>`.
    CommaOffset,
    /// Source used the `FETCH FIRST` spelling.
    FetchFirst,
}

/// Surface spelling of a DuckDB percentage-limit marker ([`Limit::percent`]).
///
/// One percent semantic, two spellings kept as data (mirroring
/// [`LimitSyntax`]/[`RollupSpelling`]): the `%` operator (`LIMIT 35%`) and the
/// `PERCENT` keyword (`LIMIT 40 PERCENT`). The two are interchangeable in DuckDB —
/// both return the same fraction of rows — so canonicalizing them onto one node keeps
/// the differential oracle comparing one shape; the tag exists only so rendering
/// reproduces the written marker rather than normalizing every form to one spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LimitPercent {
    /// The `%` operator spelling: `LIMIT 35%` (the whitespace-insensitive `LIMIT 20 %`
    /// canonicalizes onto this — the space before `%` is not a distinct spelling).
    Symbol,
    /// The `PERCENT` keyword spelling: `LIMIT 40 PERCENT`.
    Keyword,
}

/// ClickHouse `LIMIT n [OFFSET m] BY expr, …` — per-group row limiting.
///
/// Keeps the first `n` rows for each distinct value of the `by` expression list
/// (with an optional `OFFSET m` skip *within* each group), a wholly different
/// operation from the ordinary [`Limit`] tail that bounds the result as a whole. A
/// query may carry **both**, in that order: `SELECT … ORDER BY … LIMIT 2 BY x LIMIT
/// 10` limits to two rows per `x`, then caps the whole result at ten. So this is its
/// own [`Query::limit_by`] field, never folded onto the `Limit` shape — the two
/// clauses coexist and mean different things.
///
/// Gated by [`QueryTailSyntax::limit_by_clause`](crate::dialect::SelectSyntax); no
/// shipped preset spells it but Lenient (the permissive union). The `by` list is
/// always non-empty (the `BY` keyword requires at least one expression).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LimitBy<X: Extension = NoExt> {
    /// The per-group row count `n`; always written (`LIMIT BY` has no bare form).
    pub limit: Expr<X>,
    /// The `OFFSET m` skip applied within each group before the `n` rows are kept;
    /// `None` when unwritten. Rendered as `OFFSET m`, the canonical spelling.
    pub offset: Option<Expr<X>>,
    /// The `BY expr, …` grouping expressions; always at least one.
    pub by: ThinVec<Expr<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One ClickHouse `SETTINGS` pair: `<name> = <value>` (`max_threads = 8`,
/// `join_algorithm = 'auto'`), an element of [`Query::settings`].
///
/// ClickHouse's grammar is `identifier '=' literal` — the value is a scalar literal
/// (number, string, boolean). It is modelled as a general [`Expr`] (the `SecretOption`
/// precedent — a `<name> <value>` option whose value is likewise a general expression),
/// so the corpus literals round-trip while the wider expression grammar is the recorded
/// acceptance bound.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Setting<X: Extension = NoExt> {
    /// The setting name (`max_threads`); a bare identifier.
    pub name: Ident,
    /// The assigned value; ClickHouse writes a literal, modelled as a general [`Expr`].
    pub value: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// ClickHouse `FORMAT <name>` — the output-format clause that closes a query
/// ([`Query::format`]), naming the serialization of the result (`FORMAT JSON`,
/// `FORMAT CSV`, `FORMAT TabSeparated`, `FORMAT Null`).
///
/// The format name is a bare identifier, case-sensitive (`JSON` ≠ `json` to
/// ClickHouse), never a string literal — so it is carried as an [`Ident`] preserving
/// the source spelling, not a [`Literal`]. `Null` is an ordinary format name here, not
/// the null literal. Carries no [`Expr`], so it is not generic over the extension
/// parameter (the [`OrderByAll`] precedent).
///
/// Gated by [`QueryTailSyntax::format_clause`](crate::dialect::SelectSyntax); no shipped
/// preset spells it but Lenient (the permissive union). There is no ClickHouse oracle,
/// so the accepted grammar is the recorded acceptance bound.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct FormatClause {
    /// The output-format name (`JSON`, `TabSeparated`, `Null`); a bare, case-sensitive
    /// identifier.
    pub name: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// MSSQL `FOR XML` / `FOR JSON` result-shaping tail on a [`Query`]
/// ([`Query::for_clause`]): `SELECT … FOR XML {RAW|AUTO|EXPLICIT|PATH} [, …]` and
/// `SELECT … FOR JSON {AUTO|PATH} [, …]`, which serialize the result set as XML or
/// JSON instead of a rowset.
///
/// # Parity with sqlparser-rs `ForClause`
///
/// Mirrors sqlparser-rs's `ForClause` enum (its `Xml { for_xml, elements,
/// binary_base64, root, r#type }` / `Json { for_json, root, include_null_values,
/// without_array_wrapper }` variants), with three deliberate reshapings:
/// - The `RAW`/`AUTO`/`EXPLICIT`/`PATH` and `AUTO`/`PATH` selectors are their own
///   [`ForXmlMode`] / [`ForJsonMode`] axes carrying the optional `('name')` element
///   name on the arms that take one (`RAW`/`PATH`), rather than sqlparser-rs's
///   flatter `ForXml`/`ForJson` with a separate name — the name is a property of the
///   mode, so it rides the mode arm (the canonical-shape doctrine, ADR-0011).
/// - `ELEMENTS` is [`Option`]`<`[`ForXmlElements`]`>` — `None` for the attribute-centric
///   default, `Some` carrying the `XSINIL`/`ABSENT` null-handling refinement —
///   where sqlparser-rs drops the refinement onto a bare `elements: bool`.
/// - `ROOT ['name']` is [`Option`]`<`[`ForRoot`]`>` (presence = the `ROOT` keyword;
///   the inner name is optional) shared by both variants, where sqlparser-rs models
///   it per-variant as `root: Option<String>` — losing the bare-`ROOT` vs no-`ROOT`
///   distinction our shape keeps.
///
/// sqlparser-rs's `ForClause::Browse` (`FOR BROWSE`) is out of scope: this ticket
/// covers only the `FOR XML`/`FOR JSON` result-shaping tails.
///
/// # Gating and `FOR` disambiguation
///
/// Gated by [`QueryTailSyntax::for_xml_json_clause`](crate::dialect::SelectSyntax) —
/// on for MSSQL and the permissive Lenient union, off elsewhere; with the gate off the
/// `FOR` keyword in this position is left
/// unconsumed and surfaces as a clean parse error. `FOR` also introduces the
/// row-locking clauses ([`Query::locking`], gated by
/// [`QueryTailSyntax::locking_clauses`](crate::dialect::SelectSyntax)); the two share
/// the `FOR` lead but **partition on the follow token** — `XML`/`JSON` here versus
/// `UPDATE`/`SHARE`/`NO`/`KEY` for locking — so the dispatch is unambiguous under
/// every preset combination, including Lenient (the one preset that enables both), and
/// needs no [`GrammarConflict`](crate::dialect::GrammarConflict) registry entry.
///
/// There is no MSSQL oracle, so the accepted grammar is the recorded acceptance bound
/// (self-consistent round-trip tests plus the MSSQL `FOR XML`/`FOR JSON` docs cited on
/// the gating flag). Options are accepted order-independently and rendered in the
/// canonical MSSQL directive order. Carries only mode tags and quoted-name
/// [`Literal`]s (no [`Expr`]), so it is not generic over the extension parameter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ForClause {
    /// `FOR XML {RAW|AUTO|EXPLICIT|PATH} [, BINARY BASE64] [, TYPE] [, ROOT ['name']]
    /// [, ELEMENTS [XSINIL|ABSENT]]`.
    Xml {
        /// Mode selected by this syntax.
        mode: ForXmlMode,
        /// The `ELEMENTS [XSINIL|ABSENT]` element-centric directive; `None` for the
        /// attribute-centric default.
        elements: Option<ForXmlElements>,
        /// `BINARY BASE64` — encode binary columns as Base64 rather than a URL
        /// reference.
        binary_base64: bool,
        /// `TYPE` — return the result as an `xml`-typed value rather than text.
        typed: bool,
        /// `ROOT ['name']` wrapper element; `None` when no `ROOT` directive is written.
        root: Option<ForRoot>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `FOR JSON {AUTO|PATH} [, ROOT ['name']] [, INCLUDE_NULL_VALUES]
    /// [, WITHOUT_ARRAY_WRAPPER]`.
    Json {
        /// Mode selected by this syntax.
        mode: ForJsonMode,
        /// `ROOT ['name']` wrapper property; `None` when no `ROOT` directive is written.
        root: Option<ForRoot>,
        /// `INCLUDE_NULL_VALUES` — emit `null`-valued properties instead of omitting them.
        include_null_values: bool,
        /// `WITHOUT_ARRAY_WRAPPER` — emit a single object rather than a JSON array.
        without_array_wrapper: bool,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The `FOR XML` serialization mode ([`ForClause::Xml`]): `RAW`, `AUTO`, `EXPLICIT`,
/// or `PATH`. `RAW` and `PATH` carry an optional `('ElementName')` naming the row
/// element (a quoted string [`Literal`]); `AUTO` and `EXPLICIT` take no name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ForXmlMode {
    /// `RAW ['ElementName']` — one `<row>` (or the named element) per result row.
    Raw {
        /// The `('ElementName')` row-element name; `None` for a bare `RAW`.
        name: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `AUTO` — nest elements to reflect the `FROM` join hierarchy.
    Auto {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `EXPLICIT` — the query's own universal-table shape defines the XML.
    Explicit {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PATH ['ElementName']` — column names as XPath expressions.
    Path {
        /// The `('ElementName')` row-element name; `None` for a bare `PATH`.
        name: Option<Literal>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The `FOR XML … ELEMENTS` null-handling refinement ([`ForClause::Xml`]): a bare
/// `ELEMENTS`, or one of the `XSINIL`/`ABSENT` variants. A pure surface tag (no span
/// of its own), like [`LockStrength`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ForXmlElements {
    /// Bare `ELEMENTS` — element-centric XML, NULL columns omitted (the `ABSENT`
    /// default MSSQL applies when neither refinement is written).
    Plain,
    /// `ELEMENTS XSINIL` — emit an empty element with `xsi:nil="true"` for NULLs.
    XsiNil,
    /// `ELEMENTS ABSENT` — omit the element for NULL columns (explicit spelling of
    /// the default).
    Absent,
}

/// The `FOR JSON` serialization mode ([`ForClause::Json`]): `AUTO` or `PATH`. Neither
/// takes a name (unlike [`ForXmlMode`]), so a pure surface tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ForJsonMode {
    /// `AUTO` — nest JSON to reflect the `FROM` join hierarchy.
    Auto,
    /// `PATH` — dotted column aliases define the JSON object shape.
    Path,
}

/// A `ROOT ['name']` directive shared by [`ForClause::Xml`] and [`ForClause::Json`]:
/// the `ROOT` keyword wraps the output in a single root element/property, optionally
/// named. Its own spanned node so the `ROOT` keyword span is addressable; the
/// optional name is a quoted string [`Literal`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ForRoot {
    /// The `('name')` root name; `None` for a bare `ROOT` (MSSQL then names it `root`).
    pub name: Option<Literal>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One Hive/Spark `LATERAL VIEW [OUTER] <generator>(args) <alias> [AS <col> [, …]]`
/// clause on a [`Select`] ([`Select::lateral_views`]): a table-generating function
/// (`explode`, `posexplode`, `json_tuple`, …) whose output rows are cross-joined
/// against each input row, with `OUTER` keeping input rows the generator produces no
/// rows for (NULL-padded, like an outer join).
///
/// # Parity with sqlparser-rs `LateralView`
///
/// Mirrors sqlparser-rs's `LateralView` struct (`lateral_view: Expr`,
/// `lateral_view_name: ObjectName`, `lateral_col_alias: Vec<Ident>`, `outer: bool`),
/// with two deliberate reshapings (ADR-0011's typed canonical shape) and one rename:
/// - `lateral_view: Expr` → [`function`](Self::function)`:` [`FunctionCall`] — both
///   grammars require a generator *call* here (Hive `FromClauseParser.g` `lateralView:
///   … function tableAlias …`; Spark `SqlBaseParser.g4` `lateralView: LATERAL VIEW
///   OUTER? qualifiedName '(' expression* ')' …`), so the typed call node is the
///   grammar, where sqlparser-rs's arbitrary expression over-admits non-calls.
/// - `lateral_view_name: ObjectName` → [`alias`](Self::alias)`:` [`Ident`] — the
///   correlation alias is a single identifier in both grammars (Hive `tableAlias`,
///   Spark `tblName=identifier`), never a qualified name.
/// - `lateral_col_alias` → [`columns`](Self::columns), a [`ThinVec`]`<`[`Ident`]`>`
///   (empty when unwritten — column aliases are optional since Hive 0.12).
///
/// # Acceptance bound (no Hive/Spark oracle)
///
/// The table alias is required (both grammars make it non-optional) and the `AS`
/// before the column list is optional — Spark's grammar spells `AS?` while Hive's
/// requires the keyword, so accepting the bare-column spelling under the one atomic
/// flag is a known conservative-direction over-acceptance for the Hive preset
/// (the [`JoinSyntax::sided_semi_anti_join`](crate::dialect::TableExpressionSyntax)
/// precedent, captured on the owning ticket). Rendering canonicalizes the bare
/// spelling to `AS` — a structural, not byte-exact, round-trip (the wildcard-modifier
/// precedent). Whether the named function is a genuine UDTF is a bind-time check past
/// the parse-level contract.
///
/// # Gating and `LATERAL` disambiguation
///
/// Gated by [`SelectSyntax::lateral_view_clause`](crate::dialect::SelectSyntax) — on
/// for Hive, Databricks, and the permissive Lenient union, off elsewhere; with the
/// gate off the `LATERAL` keyword in this position is left unconsumed and surfaces as
/// a clean parse error. `LATERAL` also introduces the standard LATERAL derived-table /
/// function factor ([`TableFactorSyntax::lateral`](crate::dialect::TableExpressionSyntax)),
/// but the two occupy disjoint grammar positions — a table-factor head (after
/// `FROM`/`,`/a join keyword) versus after the *whole* FROM list — and additionally
/// partition on the follow token (`VIEW` here; `(` or a function/subquery head there),
/// so the dispatch is unambiguous under every preset combination, including Lenient
/// (which enables both), and needs no
/// [`GrammarConflict`](crate::dialect::GrammarConflict) registry entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LateralView<X: Extension = NoExt> {
    /// `OUTER` — keep input rows the generator returns no rows for, NULL-padding the
    /// generated columns (Hive/Spark's outer-join refinement of the default
    /// cross-join semantics).
    pub outer: bool,
    /// The table-generating function call (`explode(col)`); inline, not boxed,
    /// because this node lives only behind [`Select::lateral_views`]' heap allocation
    /// (the [`RowsFromItem`] precedent).
    pub function: FunctionCall<X>,
    /// The required correlation alias naming the generated relation (Hive
    /// `tableAlias` / Spark `tblName`).
    pub alias: Ident,
    /// The `[AS] c1, c2` column aliases naming the generator's output columns; empty
    /// when unwritten.
    pub columns: ThinVec<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// An Oracle-style hierarchical (recursive) query clause on a [`Select`]
/// ([`Select::connect_by`]): the `CONNECT BY [NOCYCLE] <condition>` parent/child walk,
/// with an optional `START WITH <condition>` seed selecting the root rows. Rows are
/// expanded top-down from each root, following the `CONNECT BY` condition, in which the
/// [`UnaryOperator::Prior`](crate::ast::UnaryOperator) operator marks the operand taken
/// from the parent row.
///
/// # Grammar position and clause order
///
/// The clause sits **after `WHERE` and before `GROUP BY`**, Oracle's syntactic position
/// for the `hierarchical_query_clause` (Oracle *Database SQL Language Reference*,
/// `SELECT` → `hierarchical_query_clause`). `START WITH` and `CONNECT BY` may be written
/// in **either order** — Oracle's grammar admits both
/// `START WITH <c> CONNECT BY [NOCYCLE] <c>` and
/// `CONNECT BY [NOCYCLE] <c> [START WITH <c>]` — so the written order is recorded by
/// [`start_with_leads`](Self::start_with_leads) and round-trips exactly (the
/// spelling-fidelity doctrine: an ordering both engines accept is a spelling, not a
/// normalization). Snowflake, whose public docs are the citable grammar here (there is
/// no Oracle preset/oracle), documents only the `START WITH … CONNECT BY …` order and
/// the `[PRIOR] col = [PRIOR] col` equality shape (*Snowflake SQL Reference*,
/// `CONNECT BY`); it places the pair right after `FROM`, but modelling the Oracle
/// after-`WHERE` position is the strict superset that also accepts the Snowflake
/// `FROM … START WITH … CONNECT BY … [WHERE]`-less spelling.
///
/// # Acceptance bound (no Oracle/Snowflake oracle)
///
/// - **`NOCYCLE`.** Oracle accepts `CONNECT BY NOCYCLE <cond>` (return rows despite a
///   `CONNECT BY` loop); Snowflake's docs explicitly do *not* support `NOCYCLE`. The one
///   atomic [`connect_by_clause`](crate::dialect::SelectSyntax) gate accepts the wider
///   Oracle bound, a documented conservative-direction over-acceptance under the
///   Snowflake preset (the [`LateralView`] `AS`-optional precedent), captured on the
///   owning ticket.
/// - **`PRIOR`.** Both engines require exactly one `PRIOR` per `CONNECT BY` equality;
///   this parser admits it as an ordinary unary operator anywhere in the condition and
///   does not enforce the once-per-conjunct rule (a bind-time check past the parse-level
///   contract). `PRIOR` is recognized **only inside the `CONNECT BY` condition**, so the
///   global expression grammar is unchanged and a bare `prior` stays an ordinary column
///   name everywhere else.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct HierarchicalClause<X: Extension = NoExt> {
    /// The optional `START WITH <condition>` root-row seed; `None` when the clause
    /// writes only `CONNECT BY`. Inline (not boxed) because this node lives only behind
    /// [`Select::connect_by`]' heap allocation (the [`LateralView`] precedent).
    pub start_with: Option<Expr<X>>,
    /// `NOCYCLE` — return rows even when the `CONNECT BY` walk hits a loop (Oracle; not
    /// Snowflake — see the acceptance-bound doc). Always modifies `CONNECT BY`,
    /// whichever order the pair is written in.
    pub nocycle: bool,
    /// The required `CONNECT BY [NOCYCLE] <condition>` parent/child predicate. Ordinary
    /// expression riding the guarded expression path, with
    /// [`UnaryOperator::Prior`](crate::ast::UnaryOperator) recognized inside it.
    pub connect_by: Expr<X>,
    /// `true` when `START WITH` was written **before** `CONNECT BY`, `false` when after
    /// (or absent — canonically `false` when [`start_with`](Self::start_with) is `None`,
    /// where the order is moot). Preserves the written order for an exact round-trip.
    pub start_with_leads: bool,
    /// Source location and node identity.
    pub meta: Meta,
}
