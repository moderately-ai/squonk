// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Scalar-expression AST nodes: the `Expr` tree and its operand shapes.

use super::{
    DataType, Extension, Ident, IntervalFields, Literal, NoExt, ObjectName, OrderByExpr, Query,
    SetQuantifier, WildcardOptions, WindowSpec,
};
use crate::vocab::{Meta, Symbol};
use thin_vec::ThinVec;

/// SQL expressions for the M1 parser surface.
///
/// Each variant carries `Meta` so the expression node itself has side-table
/// identity. Child nodes keep their own metadata; `Meta` remains structural
/// equality-neutral.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum Expr<X: Extension = NoExt> {
    /// A column reference: a bare or qualified name (`c`, `t.c`, `s.t.c`).
    Column {
        /// The referenced name, one or more dot-separated identifier parts.
        name: ObjectName,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A literal constant: a number, string, boolean, `NULL`, or typed date/time value.
    Literal {
        /// The literal value; see [`Literal`].
        literal: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A binary operator application: `left op right` (`a + b`, `x AND y`, `p || q`).
    BinaryOp {
        /// Left-hand operand.
        left: Box<Expr<X>>,
        /// The binary operator joining the two operands.
        op: BinaryOperator,
        /// Right-hand operand.
        right: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A unary operator application: prefix (`-x`, `NOT p`, `+n`) or postfix, per the operator.
    UnaryOp {
        /// The unary operator applied to the operand.
        op: UnaryOperator,
        /// The operand the operator applies to.
        expr: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A function, aggregate, or window call (`count(*)`, `substr(s, 1, 3)`); see [`FunctionCall`].
    Function {
        /// The call's target, arguments, and any `OVER`/`FILTER`/`WITHIN GROUP` clauses; see [`FunctionCall`].
        call: Box<FunctionCall<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `CASE` expression: searched (`CASE WHEN … THEN …`) or simple (`CASE x WHEN …`); see [`CaseExpr`].
    Case {
        /// The optional operand, `WHEN`/`THEN` arms, and optional `ELSE`; see [`CaseExpr`].
        case: Box<CaseExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An `EXTRACT(field FROM source)` datetime-field extraction; see [`ExtractExpr`].
    Extract {
        /// The extracted field and the source expression; see [`ExtractExpr`].
        extract: Box<ExtractExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A type cast: standard `CAST(expr AS type)`, PostgreSQL `expr::type`, or DuckDB `TRY_CAST`.
    Cast {
        /// The expression being cast.
        expr: Box<Expr<X>>,
        /// The target data type.
        data_type: Box<DataType<X>>,
        /// Whether the cast was spelled `CAST(expr AS type)` or `expr::type`.
        syntax: CastSyntax,
        /// DuckDB's `TRY_CAST(expr AS type)` null-on-failure cast. This is *semantics*,
        /// not a spelling: a failed `TRY_CAST` yields `NULL` where `CAST` raises, so it
        /// is a distinct flag rather than a [`CastSyntax`] variant. DuckDB's
        /// own serialized tree carries the same `try_cast` boolean on its one `CAST`
        /// node. Always `false` for the `::` and prefix-typed spellings, which have no
        /// try form.
        try_cast: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A null test: `<expr> IS [NOT] NULL`, or its postfix synonyms — one-word
    /// `<expr> ISNULL` / `<expr> NOTNULL` and the two-word `<expr> NOT NULL`.
    ///
    /// `negated` is the `NOT` axis (`IS NOT NULL` / `NOTNULL` / `NOT NULL`);
    /// [`spelling`](NullTestSpelling) records whether the source wrote the standard
    /// `IS [NOT] NULL`, the one-word `ISNULL`/`NOTNULL` postfix, or the two-word `NOT NULL`
    /// postfix so rendering round-trips. The one-word synonyms are gated by
    /// [`OperatorSyntax::null_test_postfix`](crate::dialect::OperatorSyntax) and the two-word
    /// form by [`PredicateSyntax::null_test_two_word_postfix`](crate::dialect::PredicateSyntax)
    /// (they diverge — see the flag docs); the standard spelling is always available.
    IsNull {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// Whether the negated form was present in the source.
        negated: bool,
        /// Exact source spelling retained for faithful rendering.
        spelling: NullTestSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A truth-value test: `<expr> IS [NOT] {TRUE | FALSE | UNKNOWN}` (SQL:2016 F571).
    ///
    /// The postfix sibling of [`IsNull`](Self::IsNull) — a unary predicate carrying a
    /// `negated` flag for the `IS NOT` form and binding at comparison precedence — with a
    /// [`TruthValue`] tag recording which of the three truth values was tested. A distinct
    /// node rather than a widening of `IsNull`: the standard models truth tests and the null
    /// test as separate predicates (`<boolean test>` vs `<null predicate>`), and the operand
    /// is a boolean value here rather than an arbitrary comparable.
    ///
    /// Reachable only under
    /// [`OperatorSyntax::truth_value_tests`](crate::dialect::OperatorSyntax) (ANSI /
    /// PostgreSQL / MySQL / DuckDB / Lenient). SQLite has no truth-value predicate: its `IS`
    /// is a general null-safe equality, so it folds `IS TRUE` / `IS FALSE` onto
    /// [`IsNotDistinctFrom`](BinaryOperator::IsNotDistinctFrom) against the boolean literal and
    /// reads `IS UNKNOWN` as equality against an identifier named `unknown` (engine-measured
    /// via rusqlite) — so the flag stays off there and this node is never produced.
    IsTruth {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// Value supplied by this syntax.
        value: TruthValue,
        /// Whether the negated form was present in the source.
        negated: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A Unicode-normalization test: `<expr> IS [NOT] [NFC|NFD|NFKC|NFKD] NORMALIZED`
    /// (SQL:2016 T061).
    ///
    /// The postfix sibling of [`IsNull`](Self::IsNull)/[`IsTruth`](Self::IsTruth) — a unary
    /// predicate carrying a `negated` flag for the `IS NOT` form, binding at comparison
    /// precedence — testing whether a string is in the given Unicode normal form. `form` is
    /// the optional [`NormalizationForm`]; `None` is the bare `IS NORMALIZED`, which defaults
    /// to NFC. Gated by
    /// [`PredicateSyntax::is_normalized`](crate::dialect::PredicateSyntax).
    IsNormalized {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// Optional form for this syntax.
        form: Option<NormalizationForm>,
        /// Whether the negated form was present in the source.
        negated: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A range test: `<expr> [NOT] BETWEEN [SYMMETRIC] <low> AND <high>`.
    ///
    /// `symmetric` records the SQL-standard `SYMMETRIC` modifier, which — unlike the
    /// default `ASYMMETRIC` — is semantically load-bearing: it permits `low > high` by
    /// testing the value against the ordered pair, so it is kept as data rather than
    /// dropped. The explicit `ASYMMETRIC` noise word is the default and is not retained
    /// (it renders back as the bare form). Gated by
    /// [`PredicateSyntax::between_symmetric`](crate::dialect::PredicateSyntax).
    Between {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// The lower bound of the range.
        low: Box<Expr<X>>,
        /// The upper bound of the range.
        high: Box<Expr<X>>,
        /// Whether the negated form was present in the source.
        negated: bool,
        /// Whether the symmetric form was present in the source.
        symmetric: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A pattern-match predicate: `<expr> [NOT] LIKE|ILIKE|SIMILAR TO <pattern>
    /// [ESCAPE <c>]`.
    ///
    /// One canonical shape covers all three spellings; [`LikeSpelling`]
    /// records which the source used so it round-trips. Like the other comparison
    /// predicates ([`Between`](Self::Between)/[`InList`](Self::InList)) it carries a
    /// `negated` flag for the `NOT` form and binds at comparison precedence. The
    /// optional `escape` holds the ISO `ESCAPE '<c>'` character (usually a string
    /// literal); `None` when the source wrote no `ESCAPE`.
    Like {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// The pattern to match against.
        pattern: Box<Expr<X>>,
        /// Optional escape for this syntax.
        escape: Option<Box<Expr<X>>>,
        /// Whether the negated form was present in the source.
        negated: bool,
        /// Exact source spelling retained for faithful rendering.
        spelling: LikeSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A list-membership test: `<expr> [NOT] IN (v1, v2, …)`.
    InList {
        /// The value tested for membership.
        expr: Box<Expr<X>>,
        /// The candidate values, in source order.
        list: ThinVec<Expr<X>>,
        /// Whether the negated form was present in the source.
        negated: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A subquery-membership test: `<expr> [NOT] IN (<subquery>)`.
    InSubquery {
        /// The value tested for membership.
        expr: Box<Expr<X>>,
        /// The subquery whose rows form the candidate set.
        subquery: Box<Query<X>>,
        /// Whether the negated form was present in the source.
        negated: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's unparenthesized `<expr> [NOT] IN <rhs>` list-membership operator, where
    /// the right operand is a single value expression rather than a parenthesized list
    /// or subquery: `z IN y` (DuckDB desugars it to `contains(y, z)`), distinct from the
    /// standard `IN (…)` forms ([`InList`](Self::InList) / [`InSubquery`](Self::InSubquery)).
    /// A new canonical node, not a widening of `InList`: the two round-trip to
    /// different surface syntax (`x IN y` vs `x IN (y)`), and DuckDB models them as
    /// distinct constructs (a `contains` function call vs a `COMPARE_IN`).
    ///
    /// `rhs` is DuckDB's restricted `c_expr` right operand: a column/qualified reference,
    /// function call, subscript (`y[1]`), array/struct/map literal, `CAST`/`CASE`, or a
    /// parameter — but never a leading constant or unary sign (`IN 4` / `IN 'a'` / `IN -5`
    /// are DuckDB parser errors, an LALR grammar-generator restriction the parser
    /// replicates via a leading-token gate). Binds tighter than the comparison operators
    /// and the standard `IN (list)` predicate (`z = w IN y` is `z = (w IN y)`; measured on
    /// 1.5.4), left-associative. Reachable only under
    /// [`PredicateSyntax::unparenthesized_in_list`](crate::dialect::PredicateSyntax)
    /// (DuckDB / Lenient).
    InExpr {
        /// The value tested for membership.
        expr: Box<Expr<X>>,
        /// The single-value right operand (DuckDB's restricted `c_expr`).
        rhs: Box<Expr<X>>,
        /// Whether the negated form was present in the source.
        negated: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An `EXISTS (<subquery>)` test — true when the subquery returns at least one row.
    Exists {
        /// The subquery tested for row existence.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A quantified comparison against a subquery: `<expr> op {ANY | ALL | SOME} (<subquery>)`.
    QuantifiedComparison {
        /// Left-hand operand.
        left: Box<Expr<X>>,
        /// The comparison operator applied against each row.
        op: BinaryOperator,
        /// Whether the comparison is `ANY`/`SOME` or `ALL`.
        quantifier: Quantifier,
        /// The subquery supplying the right-hand rows.
        subquery: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A quantified comparison whose right operand is a scalar list/array *value*
    /// rather than a subquery: DuckDB `ax = ANY (b)` over a `LIST`/`ARRAY` column or
    /// literal, `x = ANY ([1, 2, 3])`, PostgreSQL `x = ANY (ARRAY[…])`.
    ///
    /// A distinct node from [`QuantifiedComparison`](Self::QuantifiedComparison), not a
    /// widening of its `subquery` field: the reference engine models the two
    /// as separate constructs — the array form is PostgreSQL's `ScalarArrayOpExpr` (the
    /// operator is applied elementwise to an array value) while the subquery form is an
    /// `AnySublink`/`AllSublink` (the operator ranges over a query's rows). The operands
    /// differ in kind (a value [`Expr`] vs a [`Query`]) and in evaluation, so folding
    /// them onto one node would misrepresent the engine's shape. The parser splits the
    /// two on the parenthesized content exactly as `IN (…)` splits `InSubquery` from
    /// `InList`: a leading query keyword is the subquery form, anything else is this one.
    ///
    /// `array` is the operand expression as written (the list literal `[…]`, an
    /// `ARRAY[…]` constructor, or a bare column). Reachable only under
    /// [`OperatorSyntax::quantified_comparison_lists`](crate::dialect::OperatorSyntax)
    /// (DuckDB/PostgreSQL/Lenient); the subquery form's
    /// [`quantified_comparisons`](crate::dialect::OperatorSyntax::quantified_comparisons)
    /// gate is a prerequisite (both are on wherever this is).
    QuantifiedList {
        /// Left-hand operand.
        left: Box<Expr<X>>,
        /// Operator applied by this expression.
        op: BinaryOperator,
        /// Whether the quantifier is `ANY`/`SOME` or `ALL`.
        quantifier: Quantifier,
        /// The array/list right operand.
        array: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A pattern-match predicate quantified over an array operand: PostgreSQL
    /// `<expr> [NOT] LIKE|ILIKE {ANY | ALL | SOME} (<array>)` — `'foo' LIKE ANY
    /// (ARRAY['%a', '%o'])`.
    ///
    /// A distinct node from [`Like`](Self::Like), for the same reason
    /// [`QuantifiedList`](Self::QuantifiedList) is distinct from a plain
    /// comparison: PostgreSQL models the plain `<expr> LIKE <pattern>` as an
    /// `OpExpr` over the `~~` operator, but the quantified form as a
    /// `ScalarArrayOpExpr` applying `~~`/`~~*` elementwise across an array value.
    /// The operands differ in kind (a single pattern [`Expr`] vs an array-valued
    /// operand) and in evaluation, so folding a quantifier onto `Like` would
    /// misrepresent the engine's shape. `SIMILAR TO` has no quantified form
    /// (PostgreSQL rejects `SIMILAR TO ANY`), so `spelling` is only ever
    /// [`Like`](LikeSpelling::Like)/[`ILike`](LikeSpelling::ILike) here.
    ///
    /// `pattern` is the array operand as written. Reachable only under
    /// [`PredicateSyntax::pattern_match_quantifier`](crate::dialect::PredicateSyntax)
    /// (PostgreSQL/Lenient).
    QuantifiedLike {
        /// Left-hand operand.
        left: Box<Expr<X>>,
        /// The array of patterns to match against.
        pattern: Box<Expr<X>>,
        /// Whether the quantifier is `ANY`/`SOME` or `ALL`.
        quantifier: Quantifier,
        /// Whether the negated form was present in the source.
        negated: bool,
        /// Exact source spelling retained for faithful rendering.
        spelling: LikeSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A scalar subquery used as a value: `(SELECT …)`.
    Subquery {
        /// The parenthesized subquery evaluated as a scalar value.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A placeholder parameter bound at execute time (`?`, `$1`, `:name`); see [`ParameterKind`].
    Parameter {
        /// Which placeholder syntax was used; see [`ParameterKind`].
        kind: ParameterKind,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `#n` positional column reference — a select-list column named by its
    /// 1-based output position, used mainly in `ORDER BY #1` / `GROUP BY #2` but valid
    /// wherever a value expression is (`SELECT #1 + 1`). A new canonical node,
    /// not folded onto an integer [`Literal`](Self::Literal): the `#` sigil carries the
    /// "resolve to the n-th projected column" semantics DuckDB's binder acts on, which a
    /// bare integer does not. Distinct from a prepared-statement
    /// [`Parameter`](Self::Parameter) placeholder (a hole bound at execute time) — this
    /// reads a column already in the query. `index` is the parsed position, always `>= 1`
    /// (DuckDB rejects `#0` at parse time — "Positional reference node needs to be >= 1").
    /// Reachable only under
    /// [`ExpressionSyntax::positional_column`](crate::dialect::ExpressionSyntax), which
    /// also gates the `#<digits>` lexeme.
    PositionalColumn {
        /// The 1-based select-list position (always `>= 1`).
        index: u32,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL session-variable read: a user-defined `@name` variable or a server
    /// `@@[scope.]name` system variable, evaluated as a value expression.
    ///
    /// Distinct from a prepared-statement placeholder ([`Parameter`](Self::Parameter)):
    /// a placeholder is a hole bound at execute time, whereas this reads the current
    /// value of the named variable in place. One canonical shape covers all
    /// four surface forms; the [`SessionVariableKind`] tag records the sigil (`@` vs
    /// `@@`) and the optional system scope so `@x`, `@@x`, `@@global.x`, and
    /// `@@session.x` each round-trip exactly. `name` is the interned variable name,
    /// exact-case (sigil and scope stripped), so it renders verbatim like an
    /// identifier.
    SessionVariable {
        /// Which sigil/scope form (`@`/`@@`/`@@global.`/`@@session.`); see [`SessionVariableKind`].
        kind: SessionVariableKind,
        /// Name referenced by this syntax.
        name: Symbol,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL array element/slice access: `base[index]` or
    /// `base[lower:upper]` (boxed; see [`SubscriptExpr`]).
    Subscript {
        /// The array element/slice access; see [`SubscriptExpr`].
        subscript: Box<SubscriptExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A colon-led semi-structured value path: `base:key[0].field` (boxed; see
    /// [`SemiStructuredAccessExpr`]).
    SemiStructuredAccess {
        /// The semi-structured path access; see [`SemiStructuredAccessExpr`].
        semi_structured_access: Box<SemiStructuredAccessExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL `expr COLLATE collation` collation override (boxed; see
    /// [`CollateExpr`]).
    Collate {
        /// The collation override; see [`CollateExpr`].
        collate: Box<CollateExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL operator-position interval quantity: `INTERVAL <value> <unit>` — the
    /// `INTERVAL 3 DAY` operand of MySQL date arithmetic (`d - INTERVAL 3 DAY`,
    /// `INTERVAL 1 DAY + d`, `DATE_ADD(d, INTERVAL 1 DAY)`, a window-frame bound).
    ///
    /// Distinct from the ANSI/PostgreSQL typed-string interval *literal*
    /// [`LiteralKind::Interval`](crate::ast::LiteralKind::Interval): MySQL's `INTERVAL` is
    /// not a standalone value but the second operand of the date-add/date-sub production
    /// (`Item_date_add_interval`), so it carries an arbitrary amount *expression* and a
    /// mandatory unit keyword rather than a quoted amount string and an optional ANSI
    /// qualifier. Gated by
    /// [`ExpressionSyntax::mysql_interval_operator`](crate::dialect::ExpressionSyntax); see
    /// that flag for the position/unit boundary. The `unit` reuses the shared
    /// [`IntervalFields`] vocabulary but always renders in MySQL's underscore spelling
    /// (`DAY_HOUR`, `YEAR_MONTH`), never the ANSI `TO` composite.
    Interval {
        /// The interval amount — an arbitrary expression (`3`, `'3-2'`, `?`, `@v`, `n + 1`).
        value: Box<Expr<X>>,
        /// The unit keyword (`DAY`, `HOUR_SECOND`, `YEAR_MONTH`, …).
        unit: IntervalFields,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL `expr AT TIME ZONE zone` time-zone conversion (boxed; see
    /// [`AtTimeZoneExpr`]).
    AtTimeZone {
        /// The time-zone conversion; see [`AtTimeZoneExpr`].
        at_time_zone: Box<AtTimeZoneExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An array constructor: PostgreSQL `ARRAY[...]` / `ARRAY(<query>)` or the
    /// DuckDB bare-bracket list literal `[...]` (boxed; see [`ArrayExpr`]).
    Array {
        /// The array/list constructor; see [`ArrayExpr`].
        array: Box<ArrayExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB struct literal `{'key': value, …}` (boxed; see [`StructExpr`]).
    Struct {
        /// The struct literal; see [`StructExpr`].
        r#struct: Box<StructExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A BigQuery `STRUCT(...)` value constructor: the typeless `STRUCT(1, 2)` /
    /// `STRUCT(x AS a, y AS b)` forms and the typed `STRUCT<a INT64, b STRING>(1, 'x')`
    /// form (boxed; see [`StructConstructorExpr`]). Distinct from the DuckDB brace
    /// literal [`Struct`](Self::Struct) and from an ordinary `struct(...)` catalog-function
    /// call, which a dialect without
    /// [`ExpressionSyntax::struct_constructor`](crate::dialect::ExpressionSyntax) keeps as
    /// a [`Function`](Self::Function).
    StructConstructor {
        /// The `STRUCT(...)` constructor; see [`StructConstructorExpr`].
        constructor: Box<StructConstructorExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB map literal `MAP {k: v, …}` (boxed; see [`MapExpr`]).
    ///
    /// Only the brace form is a dedicated grammar production; the two-list
    /// `MAP(<keys>, <values>)` spelling is an ordinary call to the (case-insensitive)
    /// `map` function, so it parses as [`Function`](Self::Function) like any other
    /// call.
    Map {
        /// The map literal; see [`MapExpr`].
        map: Box<MapExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A row constructor: explicit `ROW(...)` or the implicit parenthesized
    /// `(a, b, …)` form (boxed; see [`RowExpr`]).
    Row {
        /// The row constructor; see [`RowExpr`].
        row: Box<RowExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL composite navigation off a value: `(expr).field` field selection
    /// or the `(expr).*` / whole-row `tbl.*` star expansion (boxed; see
    /// [`FieldSelectionExpr`]).
    FieldSelection {
        /// The composite field selection; see [`FieldSelectionExpr`].
        field_selection: Box<FieldSelectionExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A general infix operator application — the explicit `a OPERATOR(schema.+) b`
    /// or a bare symbolic operator `a ~ b` / `a <-> b` (boxed; see [`NamedOperatorExpr`]).
    NamedOperator {
        /// The named/infix operator application; see [`NamedOperatorExpr`].
        named_operator: Box<NamedOperatorExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A general prefix operator application: `@ x` (absolute value), `|/ x` (square
    /// root), `@@ box` (centre), or a fully user-defined prefix operator `@#@ x` (boxed;
    /// see [`PrefixOperatorExpr`]).
    PrefixOperator {
        /// The prefix operator application; see [`PrefixOperatorExpr`].
        prefix_operator: Box<PrefixOperatorExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A general postfix operator application: `10!` (factorial), `1 ~`, `1 <->`, or any
    /// other trailing symbolic operator (boxed; see [`PostfixOperatorExpr`]). DuckDB is the
    /// only enabler — it keeps the postfix reading PostgreSQL removed in 14.
    PostfixOperator {
        /// The postfix operator application; see [`PostfixOperatorExpr`].
        postfix_operator: Box<PostfixOperatorExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB single-arrow lambda: `x -> x + 1` / `(x, y) -> x + y` (boxed; see
    /// [`LambdaExpr`]).
    Lambda {
        /// The lambda expression; see [`LambdaExpr`].
        lambda: Box<LambdaExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's `COLUMNS(<selector>)` column-set selector, a star expression that
    /// stands wherever a value expression does (`SELECT COLUMNS('re')`,
    /// `sum(COLUMNS(*))`, `COLUMNS(*)::JSON`). A new canonical node, not a function
    /// call: DuckDB models it as its `STAR` node with `columns: true`, and
    /// modelling it as a call would lose the star semantics the structural oracle
    /// compares against. Reachable only under
    /// [`CallSyntax::columns_expression`](crate::dialect::ExpressionSyntax).
    ///
    /// `pattern` is the selector argument: `None` for the star form `COLUMNS(*)` /
    /// `COLUMNS(t.*)` (mirroring DuckDB's null `expr`), `Some(e)` for
    /// `COLUMNS(<expr>)` — the regex string `COLUMNS('re')`, the lambda
    /// `COLUMNS(c -> …)`, the name list `COLUMNS([…])`, or a bare column. `qualifier`
    /// and `options` belong to the star form only (both always `None` alongside a
    /// `pattern`): `qualifier` is the relation of a qualified star `COLUMNS(t.*)` —
    /// exactly one name part, DuckDB rejects `COLUMNS(s.t.*)` (probed on 1.5.4) — and
    /// `options` carries the `EXCLUDE`/`REPLACE`/`RENAME` modifiers DuckDB allows
    /// inside the star form (`COLUMNS(* EXCLUDE i)`, `COLUMNS(t.* EXCLUDE (k))`).
    ///
    /// `spelling` records which of DuckDB's three surface forms of the star node the
    /// source used (one-shape-plus-tag), all the same STAR node in the engine:
    /// [`Columns`](ColumnsSpelling::Columns) is the `COLUMNS(<selector>)` wrapper
    /// (DuckDB's STAR `columns:true`); [`Unpack`](ColumnsSpelling::Unpack) is the
    /// `*COLUMNS(<selector>)` spread-into-arguments prefix (same node, unpack flag set)
    /// admitted in call/`IN`-list argument positions; [`Star`](ColumnsSpelling::Star) is
    /// the bare `*` / `t.*` written without the wrapper (DuckDB's STAR `columns:false`),
    /// admitted in the `ORDER BY` and `UNPIVOT` `ON`/`IN` positions. A `Star` spelling
    /// never carries a `pattern` (the bare star has no selector argument).
    Columns {
        /// Optional qualifier for this syntax.
        qualifier: Option<ObjectName>,
        /// Optional pattern for this syntax.
        pattern: Option<Box<Expr<X>>>,
        /// Options supplied in source order.
        options: Option<Box<WildcardOptions<X>>>,
        /// Exact source spelling retained for faithful rendering.
        spelling: ColumnsSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQL special value function with a dedicated grammar production rather than
    /// an ordinary call: a nullary keyword form (`CURRENT_DATE`, `CURRENT_USER`,
    /// `USER`, …) or one of the temporal forms that take an optional precision
    /// (`CURRENT_TIME(p)`, `LOCALTIMESTAMP(p)`, …). PostgreSQL lowers these to
    /// `SQLValueFunction`; the keyword is the whole construct, so there is no
    /// argument list. Held inline (the keyword tag plus an optional precision is
    /// small) rather than boxed.
    SpecialFunction {
        /// Which special-value function; see [`SpecialFunctionKeyword`].
        keyword: SpecialFunctionKeyword,
        /// The `(precision)` modifier, only valid on the temporal forms
        /// (`CURRENT_TIME`, `CURRENT_TIMESTAMP`, `LOCALTIME`, `LOCALTIMESTAMP`).
        precision: Option<u32>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQL/JSON query function: `JSON_VALUE` / `JSON_QUERY` / `JSON_EXISTS`
    /// (SQL:2016, PostgreSQL's `JsonFuncExpr`; boxed, see [`JsonFuncExpr`]).
    JsonFunc {
        /// The SQL/JSON query function; see [`JsonFuncExpr`].
        json_func: Box<JsonFuncExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQL/JSON object constructor: `JSON_OBJECT(k : v, …)` (boxed; see
    /// [`JsonObjectExpr`]).
    JsonObject {
        /// The `JSON_OBJECT(...)` constructor; see [`JsonObjectExpr`].
        json_object: Box<JsonObjectExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQL/JSON array constructor: `JSON_ARRAY(v, …)` or `JSON_ARRAY(<query>)`
    /// (boxed; see [`JsonArrayExpr`]).
    JsonArray {
        /// The `JSON_ARRAY(...)` constructor; see [`JsonArrayExpr`].
        json_array: Box<JsonArrayExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQL/JSON aggregate constructor: `JSON_OBJECTAGG(k : v …)` or
    /// `JSON_ARRAYAGG(v …)` (boxed; see [`JsonAggregateExpr`]).
    JsonAggregate {
        /// The SQL/JSON aggregate; see [`JsonAggregateExpr`].
        json_aggregate: Box<JsonAggregateExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bare SQL/JSON constructor: `JSON(x)` / `JSON_SCALAR(x)` /
    /// `JSON_SERIALIZE(x)` (boxed; see [`JsonConstructorExpr`]).
    JsonConstructor {
        /// The bare SQL/JSON constructor; see [`JsonConstructorExpr`].
        json_constructor: Box<JsonConstructorExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The SQL/JSON `expr IS [NOT] JSON [type] [WITH|WITHOUT UNIQUE [KEYS]]`
    /// predicate (boxed; see [`IsJsonExpr`]).
    IsJson {
        /// The operand, optional JSON type, and uniqueness clause; see [`IsJsonExpr`].
        is_json: Box<IsJsonExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQL/XML expression function — `xmlelement`/`xmlforest`/`xmlconcat`/
    /// `xmlparse`/`xmlpi`/`xmlroot`/`xmlserialize`/`xmlexists` (SQL:2006, PostgreSQL's
    /// `XmlExpr`/`XmlSerialize`). One kind-tagged [`XmlFunc`] enum carries all eight
    /// (boxed; the shapes differ, so a single fat inline variant would tax every
    /// `Expr`).
    XmlFunc {
        /// The specific XML function and its arguments; see [`XmlFunc`].
        xml_func: Box<XmlFunc<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The SQL/XML `<expr> IS [NOT] DOCUMENT` predicate (PostgreSQL's `XmlExpr` with
    /// `IS_DOCUMENT`). Held inline like [`IsNull`](Self::IsNull) — a boxed operand,
    /// a negation flag, no clause tail.
    IsDocument {
        /// The operand tested for being a valid XML document.
        expr: Box<Expr<X>>,
        /// Whether the negated form was present in the source.
        negated: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A standard-SQL string special form — the `SUBSTRING`/`POSITION`/`OVERLAY`/
    /// `TRIM` keyword-argument grammar (SQL-92 E021 + SQL:1999 T312, PostgreSQL's
    /// `func_expr_common_subexpr` string productions). One kind-tagged [`StringFunc`]
    /// enum carries all five shapes (boxed; the shapes differ, so a single fat
    /// inline variant would tax every `Expr`). The comma plain-call spellings
    /// (`substring(x, 1, 2)`, `trim(x, y)`) stay ordinary [`Function`](Self::Function)
    /// calls — this node holds only the keyword-argument surface.
    StringFunc {
        /// The specific string function and its keyword arguments; see [`StringFunc`].
        string_func: Box<StringFunc<X>>,
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

/// Which of DuckDB's three surface spellings of the [`Expr::Columns`] star node the
/// source used.
///
/// All three lower to the one STAR node in the engine (one-shape-plus-tag);
/// the tag is what lets the shared shape round-trip to the exact spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ColumnsSpelling {
    /// `COLUMNS(<selector>)` — the wrapped column-set selector (DuckDB's STAR node with
    /// `columns:true`).
    Columns,
    /// `*COLUMNS(<selector>)` — the unpack prefix that spreads the selected columns into
    /// the enclosing call / `IN`-list argument list (`struct_pack(*COLUMNS(*))`,
    /// `2 IN (*COLUMNS(*))`). The same wrapped node with DuckDB's unpack flag set; a bare
    /// `*` never carries it.
    Unpack,
    /// A bare `*` / `t.*` written without the `COLUMNS(...)` wrapper (DuckDB's STAR node
    /// with `columns:false`), admitted in the `ORDER BY` sort key and the `UNPIVOT`
    /// `ON`/`IN` column positions. Always pattern-free.
    Star,
}

/// A SQL special value function keyword (PostgreSQL `SQLValueFunction`).
///
/// Each names a dedicated grammar production whose whole spelling is the keyword;
/// the four temporal forms additionally accept a parenthesized precision, carried
/// on [`Expr::SpecialFunction`] rather than here so the keyword stays a copy tag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SpecialFunctionKeyword {
    /// `CURRENT_CATALOG` — the current database/catalog name.
    CurrentCatalog,
    /// `CURRENT_DATE` — the current date.
    CurrentDate,
    /// `CURRENT_ROLE` — the current role name.
    CurrentRole,
    /// `CURRENT_SCHEMA` — the current schema name.
    CurrentSchema,
    /// `CURRENT_TIME` — the current time of day (optional precision).
    CurrentTime,
    /// `CURRENT_TIMESTAMP` — the current date and time (optional precision).
    CurrentTimestamp,
    /// `CURRENT_USER` — the current authorization identifier.
    CurrentUser,
    /// `LOCALTIME` — the current local time of day (optional precision).
    LocalTime,
    /// `LOCALTIMESTAMP` — the current local date and time (optional precision).
    LocalTimestamp,
    /// `SESSION_USER` — the session authorization identifier.
    SessionUser,
    /// `SYSTEM_USER` — the operating-system-level user identifier.
    SystemUser,
    /// `USER` — synonym for `CURRENT_USER`.
    User,
    /// MySQL `UTC_DATE` — the UTC-clock analogue of `CURRENT_DATE`. Nullary.
    UtcDate,
    /// MySQL `UTC_TIME` — the UTC-clock analogue of `CURRENT_TIME`; takes an optional
    /// fractional-seconds precision (`UTC_TIME(6)`).
    UtcTime,
    /// MySQL `UTC_TIMESTAMP` — the UTC-clock analogue of `CURRENT_TIMESTAMP`; takes an
    /// optional fractional-seconds precision (`UTC_TIMESTAMP(6)`).
    UtcTimestamp,
}

/// How a cast is spelled.
///
/// One canonical [`Expr::Cast`] shape covers all three spellings; this
/// tag records which surface form the source used so rendering round-trips: the
/// standard `CAST(expr AS type)` call, the PostgreSQL `expr::type` postfix
/// operator, or the PostgreSQL prefixed typed string constant `type 'string'`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CastSyntax {
    /// `CAST(expr AS type)`.
    Call,
    /// `expr::type` (PostgreSQL).
    DoubleColon,
    /// `type 'string'` — a typed string constant whose semantics are a cast of the
    /// string to the named type (PostgreSQL's generalized `ConstTypename Sconst` /
    /// `func_name Sconst`, e.g. `float8 'NaN'`, `int4 '42'`, `double precision
    /// '1.5'`). The operand is always a string constant; the canonical shape is the
    /// same [`Expr::Cast`] as `'NaN'::float8` and `CAST('NaN' AS float8)`, so only
    /// this tag and the rendered spelling distinguish the three.
    PrefixTyped,
    /// `CONVERT(<expr>, <type>)` — MySQL's comma-form cast, the same canonical
    /// [`Expr::Cast`] shape as `CAST(<expr> AS <type>)` with only this tag (and the
    /// rendered spelling) distinguishing it. The target is the identical restricted
    /// `cast_type` set as `CAST` under
    /// [`CallSyntax::restricted_cast_targets`](crate::dialect::CallSyntax) — engine-measured
    /// on mysql:8.4, `CONVERT(1, INT)` / `CONVERT(x, VARCHAR)` reject exactly as the
    /// matching `CAST` forms do, and the charset-annotated `CHAR` target rides in free via
    /// the shared type grammar. Recognized only under
    /// [`CallSyntax::convert_function`](crate::dialect::CallSyntax); MySQL-only. The
    /// transcoding `CONVERT(<expr> USING <charset>)` form is a *different* production —
    /// [`StringFunc::ConvertUsing`], not a cast.
    Convert,
}

/// A subscript into an array/list value: an element access `base[index]`, a
/// two-bound slice `base[lower:upper]`, or a DuckDB three-bound slice with a step
/// `base[lower:upper:step]`.
///
/// [`kind`](SubscriptExpr::kind) distinguishes the three forms. An index carries its
/// index in `lower` (`upper`/`step` both `None`); a two-bound slice may omit either
/// bound (`base[lower:]`, `base[:upper]`, `base[:]`); a three-bound slice may omit the
/// lower bound and the step (`base[:upper:step]`, `base[lower:upper:]`) but not the
/// middle bound — DuckDB spells an open upper bound as the `-` placeholder, carried
/// here as `upper == None` (an empty middle `base[lower::step]` is a DuckDB parse
/// error). Boxed inside [`Expr::Subscript`] to keep the hot expression enum within its
/// size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct SubscriptExpr<X: Extension = NoExt> {
    /// The value being subscripted.
    pub base: Expr<X>,
    /// The element index ([`SubscriptKind::Index`]) or the slice lower bound, if present.
    pub lower: Option<Expr<X>>,
    /// The slice upper bound, if present. Always `None` for an index; under
    /// [`SubscriptKind::SliceWithStep`] a `None` is the `-` open-upper placeholder (an
    /// absent middle bound is a parse error there), not an omitted bound.
    pub upper: Option<Expr<X>>,
    /// The slice step, if present. Only reachable under [`SubscriptKind::SliceWithStep`],
    /// and `None` there for an omitted trailing step (`base[lower:upper:]`); always `None`
    /// for an index or a two-bound slice.
    pub step: Option<Expr<X>>,
    /// Which bracketed form the subscript holds; see [`SubscriptKind`].
    pub kind: SubscriptKind,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Which bracketed form an [`Expr::Subscript`] holds — the colon count in the brackets.
///
/// The tag drives rendering: a slice re-emits its `:` separators (and, for a stepped
/// slice, the `-` open-upper placeholder) that a bare index does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SubscriptKind {
    /// `base[index]` — a single element access; the index is in
    /// [`lower`](SubscriptExpr::lower).
    Index,
    /// `base[lower:upper]` — a two-bound slice; either bound may be omitted.
    Slice,
    /// `base[lower:upper:step]` — a DuckDB three-bound slice with a step. The lower bound
    /// and the step may each be omitted; the middle bound may not be empty, so a `None`
    /// [`upper`](SubscriptExpr::upper) renders as the `-` open-upper placeholder.
    SliceWithStep,
}

/// A semi-structured object/array path rooted at a value expression.
///
/// The first segment is introduced by `:` and later segments use JSON-path-like
/// `.`/`[...]` suffixes. Kept distinct from PostgreSQL [`Expr::Subscript`] and
/// [`Expr::FieldSelection`]: those address SQL arrays/composites, while this node
/// preserves the Snowflake/Databricks-style semi-structured path surface.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct SemiStructuredAccessExpr<X: Extension = NoExt> {
    /// The value the path is rooted at.
    pub base: Expr<X>,
    /// path in source order.
    pub path: ThinVec<SemiStructuredPathSegment<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One segment of a semi-structured value path.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SemiStructuredPathSegment<X: Extension = NoExt> {
    /// A field/key segment, written as `:key` for the first segment or `.key` after that.
    Key {
        /// The field/key name.
        key: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An array/list index segment, written as `[index]`.
    Index {
        /// Index referenced by this syntax.
        index: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A PostgreSQL `expr COLLATE collation` collation override.
///
/// Boxed inside [`Expr::Collate`] to keep the hot expression enum within its size
/// budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CollateExpr<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Expr<X>,
    /// The collation name.
    pub collation: ObjectName,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A PostgreSQL `expr AT TIME ZONE zone` time-zone conversion.
///
/// Boxed inside [`Expr::AtTimeZone`] to keep the hot expression enum within its
/// size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AtTimeZoneExpr<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Expr<X>,
    /// The target time zone.
    pub zone: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// An array/list constructor.
///
/// Either an element list (`ARRAY[a, b]`, possibly empty `ARRAY[]`, or the DuckDB
/// bare-bracket `[a, b]`) or a subquery (`ARRAY(SELECT …)`). Boxed inside
/// [`Expr::Array`] to keep the hot expression enum within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ArrayExpr<X: Extension = NoExt> {
    /// `ARRAY[a, b, …]` / `[a, b, …]` (or empty `ARRAY[]` / `[]`).
    Elements {
        /// elements in source order.
        elements: ThinVec<Expr<X>>,
        /// Whether the `ARRAY` keyword was written (vs. the bare `[…]` form).
        spelling: ArraySpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ARRAY(<query>)` — always the keyword form (DuckDB has no bracket-subquery
    /// spelling).
    Subquery {
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB list comprehension `[element for var in source (if filter)?]`
    /// (boxed; see [`ListComprehension`]). A distinct list-literal production from the
    /// element/subquery forms above: the same `[…]` bracket opens it, but the
    /// `for` after the first element selects the comprehension rather than an element
    /// list. Gated by
    /// [`collection_literals`](crate::dialect::ExpressionSyntax::collection_literals) —
    /// the same flag that admits the bracket list.
    Comprehension {
        /// The list comprehension; see [`ListComprehension`].
        comprehension: Box<ListComprehension<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// Surface spelling for an array element constructor ([`ArrayExpr::Elements`]).
///
/// The keyword `ARRAY[…]` (SQL:1999 / PostgreSQL) and the bare `[…]` (DuckDB) are one
/// canonical shape; this tag records which the source used so rendering
/// round-trips exactly. Each spelling has its own acceptance gate —
/// [`ExpressionSyntax::array_constructor`](crate::dialect::ExpressionSyntax::array_constructor)
/// for the keyword,
/// [`collection_literals`](crate::dialect::ExpressionSyntax::collection_literals) for
/// the bracket — so a dialect can admit either independently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ArraySpelling {
    /// The `ARRAY[…]` keyword-prefixed constructor.
    Keyword,
    /// The DuckDB bare-bracket `[…]` list literal.
    Bracket,
}

/// A DuckDB list comprehension `[element for var in source (if filter)?]`.
///
/// Python-style syntax DuckDB desugars to `list_transform`/`list_filter`: it maps
/// `element` over each `var` drawn from `source`, keeping only those matching the
/// optional `filter`. The canonical node keeps the source spelling so a
/// comprehension round-trips; DuckDB binds it only inside `COLUMNS(…)` when `source`
/// is a column star, but that is a bind-time rule past this parse-level shape. Boxed
/// inside [`ArrayExpr::Comprehension`] to keep the array node within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ListComprehension<X: Extension = NoExt> {
    /// The output expression evaluated for each iteration.
    pub element: Box<Expr<X>>,
    /// Loop variables: one name (`for x in …`) or two (`for x, i in …` — value plus
    /// 1-based index). DuckDB's binder rejects three or more (lambda max 2); the
    /// parser admits a non-empty list so the multi-var form is representable.
    pub vars: ThinVec<Ident>,
    /// Input source for this syntax.
    pub source: ComprehensionSource<X>,
    /// Optional filter for this syntax.
    pub filter: Option<Box<Expr<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The source of a [`ListComprehension`] — the thing iterated over.
///
/// A general list-valued expression, or the DuckDB column-star source that is valid
/// only inside `COLUMNS(…)` (`[x for x in *]`, `[x for x in (* EXCLUDE (i))]`). The star
/// is a distinct spelling rather than an expression because DuckDB has no bare `*`
/// value expression — it is grammar special to this slot — and because it cannot
/// canonicalize to `COLUMNS(*)` (DuckDB rejects a `COLUMNS` nested inside another).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ComprehensionSource<X: Extension = NoExt> {
    /// A general list-valued expression source (`[y for y in <expr>]`).
    Expr {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The DuckDB column-star source: a bare `*` or a parenthesized `(* …)`, optionally
    /// carrying the `EXCLUDE`/`REPLACE`/`RENAME` wildcard modifiers (only legal in the
    /// parenthesized form). `parenthesized` records the `(…)` spelling so it round-trips.
    Star {
        /// Whether the parenthesized form was present in the source.
        parenthesized: bool,
        /// Options supplied in source order.
        options: Option<Box<WildcardOptions<X>>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A DuckDB struct literal `{'key': value, …}`.
///
/// The fields keep source order (a struct is positional as well as named). Boxed
/// inside [`Expr::Struct`] to keep the hot expression enum within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct StructExpr<X: Extension = NoExt> {
    /// fields in source order.
    pub fields: ThinVec<StructField<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `key: value` field of a [`StructExpr`].
///
/// The key is a field *name*, not a value — DuckDB rejects a non-identifier key
/// (`{1: 'x'}` is a syntax error) — so it is an interned [`Symbol`] rather than an
/// expression. One canonical shape covers the three key spellings; the
/// [`StructKeySpelling`] tag records which the source used so `{'a': 1}`, `{a: 1}`,
/// and `{"a": 1}` each round-trip exactly.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct StructField<X: Extension = NoExt> {
    /// The interned field name.
    pub key: Symbol,
    /// Source spelling used for the key spelling.
    pub key_spelling: StructKeySpelling,
    /// Value supplied by this syntax.
    pub value: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// How a [`StructField`] key was spelled.
///
/// The surface-form tag for the one canonical struct-field shape,
/// mirroring [`CastSyntax`]: DuckDB folds all three to the same field name, so only
/// this tag and the rendered spelling distinguish them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum StructKeySpelling {
    /// A single-quoted string key, as in `{'key': v}`.
    SingleQuoted,
    /// A bare identifier key, as in `{key: v}`.
    Bare,
    /// A double-quoted identifier key, as in `{"key": v}`.
    DoubleQuoted,
}

/// A BigQuery `STRUCT(...)` value constructor — the typeless `STRUCT(1, 2)` /
/// `STRUCT(x AS a)` forms and the typed `STRUCT<a INT64, b STRING>(1, 'x')` form.
///
/// Mirrors sqlparser-rs's `Expr::Struct { values, fields }`, reshaped to this crate's
/// canonical-shape-plus-owned-sub-nodes convention: `args` are the positional value
/// arguments (each optionally `AS`-aliased in the typeless form, which sqlparser-rs
/// spells as a separate `Expr::Named` node), and `fields` are the typed field
/// declarations from the `STRUCT<...>` angle-bracket prefix. `fields` is empty for the
/// typeless form — BigQuery requires at least one type inside `<...>`, so an empty list
/// is the unambiguous "typeless" marker and no separate flag is needed. Boxed inside
/// [`Expr::StructConstructor`] to keep the hot expression enum within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct StructConstructorExpr<X: Extension = NoExt> {
    /// fields in source order.
    pub fields: ThinVec<StructConstructorField<X>>,
    /// Arguments in source order.
    pub args: ThinVec<StructConstructorArg<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One typed field declaration of a typed [`StructConstructorExpr`]: `name TYPE` in
/// `STRUCT<a INT64>`, or an anonymous `TYPE` in `STRUCT<INT64>` (BigQuery permits both).
///
/// A dedicated node rather than the type-side
/// [`StructTypeField`](crate::ast::StructTypeField): BigQuery's angle-bracket field name
/// is *optional*, whereas the DuckDB paren composite type ([`DataType::Struct`](crate::ast::DataType))
/// requires it, so the two grammars carry different name arities and cannot share one shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct StructConstructorField<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: Option<Ident>,
    /// Data type named by this syntax.
    pub ty: DataType<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One positional value argument of a [`StructConstructorExpr`], optionally aliased with
/// `AS name` (`STRUCT(1 AS a)`).
///
/// The alias is only grammatical in the typeless form; the typed form takes its field
/// names from the `STRUCT<...>` prefix, so `alias` is always `None` there.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct StructConstructorArg<X: Extension = NoExt> {
    /// Value supplied by this syntax.
    pub value: Expr<X>,
    /// Alias assigned by this syntax.
    pub alias: Option<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A DuckDB map literal `MAP {k1: v1, k2: v2, …}` (possibly empty `MAP {}`).
///
/// Unlike a [`StructExpr`] field name, a map key is an arbitrary value expression
/// (`MAP {1: 'a'}`, `MAP {[1,2]: 'x'}`), so the entries hold [`MapEntry`] expression
/// pairs. Boxed inside [`Expr::Map`] to keep the hot expression enum within its size
/// budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct MapExpr<X: Extension = NoExt> {
    /// entries in source order.
    pub entries: ThinVec<MapEntry<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL map entry.
pub struct MapEntry<X: Extension = NoExt> {
    /// The map entry key expression.
    pub key: Expr<X>,
    /// Value supplied by this syntax.
    pub value: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A row constructor: explicit `ROW(a, b, …)` (or empty `ROW()`) or the implicit
/// parenthesized `(a, b, …)` form.
///
/// `explicit` records whether the `ROW` keyword was written, since the two
/// spellings are otherwise the same construct. Boxed inside
/// [`Expr::Row`] to keep the hot expression enum within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct RowExpr<X: Extension = NoExt> {
    /// fields in source order.
    pub fields: ThinVec<Expr<X>>,
    /// Whether the `ROW` keyword was written (vs. the implicit `(a, b)` form).
    pub explicit: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A PostgreSQL composite navigation off a value: `(expr).field`, `(expr).*`, or a
/// whole-row `tbl.*` used as a value.
///
/// PostgreSQL models both selectors as one `.`-indirection off an operand (a
/// `String` attribute name or an `A_Star`); this shares that shape, with
/// [`selector`](Self::selector) distinguishing the named field from the star. A
/// whole-row `tbl.*` written in a *value* position (a `ROW(...)` field, a function
/// argument, a cast operand) parses to this node with the bare column as `base` and a
/// [`FieldSelector::Star`] selector — distinct from a select-list `tbl.*`, which is a
/// [`SelectItem::QualifiedWildcard`](crate::ast::SelectItem::QualifiedWildcard)
/// projection target. Boxed inside [`Expr::FieldSelection`] to keep the hot expression
/// enum within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct FieldSelectionExpr<X: Extension = NoExt> {
    /// The composite value being navigated.
    pub base: Expr<X>,
    /// Which selector applies — a named field or the whole-composite star; see [`FieldSelector`].
    pub selector: FieldSelector,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Which composite selector a [`FieldSelectionExpr`] applies to its `base`.
///
/// PostgreSQL's `.`-indirection element is either a named attribute (`.field`) or the
/// whole-composite star (`.*`); the two are the same grammar production and bind
/// identically, so one node carries both with this tag. The star form is gated apart
/// from the named form ([`ExpressionSyntax::field_wildcard`](crate::dialect::ExpressionSyntax::field_wildcard)
/// vs [`field_selection`](crate::dialect::ExpressionSyntax::field_selection)): DuckDB
/// admits `.field` but rejects every `.*` value expansion (engine-probed 1.5.4).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum FieldSelector {
    /// A named composite attribute: `(expr).field`.
    Field {
        /// The selected attribute name.
        field: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The whole-composite / whole-row star: `(expr).*` or a value-position `tbl.*`.
    Star {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A PostgreSQL explicit-operator infix application: `a OPERATOR(schema.op) b`.
///
/// `OPERATOR(...)` names an operator explicitly, optionally schema-qualified
/// (`OPERATOR(pg_catalog.+)`), and binds at PostgreSQL's "any other operator" rank
/// — the same `%left Op OPERATOR` precedence as `||`. Boxed inside
/// [`Expr::NamedOperator`] to keep the hot expression enum within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct NamedOperatorExpr<X: Extension = NoExt> {
    /// Left-hand operand.
    pub left: Expr<X>,
    /// The optional schema qualification (`pg_catalog` in `OPERATOR(pg_catalog.+)`);
    /// an empty name when the operator is unqualified (`OPERATOR(+)`) or bare.
    pub schema: ObjectName,
    /// The operator symbol spelling (`+`, `->>`, `~`, `<->`), interned exact-case so it
    /// round-trips. The operator is always symbolic, never a word, so it is held as
    /// a bare [`Symbol`] rather than an [`Ident`].
    pub op: Symbol,
    /// Right-hand operand.
    pub right: Expr<X>,
    /// Which surface form spelled the operator application — the explicit
    /// `OPERATOR(schema.op)` keyword or a bare `a op b`.
    pub spelling: NamedOperatorSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

/// How a [`NamedOperatorExpr`] was spelled — the whole surface form.
///
/// Under the general operator surface
/// ([`OperatorSyntax::custom_operators`](crate::dialect::OperatorSyntax::custom_operators);
/// PostgreSQL is the current enabler) a dialect admits ANY operator from its `Op` character
/// class over a user-extensible operator set, and a bare `a ~ b` / `a <-> b` / `a @#@ b` is
/// the SAME grammar production as the explicit `a OPERATOR(schema.op) b` — both name the
/// operator, at the "any other operator" precedence. This tag records which surface the
/// source used so rendering round-trips exactly (PostgreSQL's own deparse keeps the bare form
/// bare and the qualified form wrapped: `OPERATOR(~)` normalizes to `~`, but
/// `OPERATOR(pg_catalog.+)` stays wrapped — engine-measured). The [`Bare`](Self::Bare)
/// form is always unqualified (a bare operator carries no schema); the
/// [`OperatorKeyword`](Self::OperatorKeyword) form carries the optional schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum NamedOperatorSpelling {
    /// The explicit `a OPERATOR(schema.op) b` keyword form (may be schema-qualified).
    OperatorKeyword,
    /// A bare `a op b` — a general symbolic operator (regex `~`/`!~`/`~*`/`!~*`,
    /// geometric/network/text-search ops, or a fully user-defined operator), munched under
    /// [`OperatorSyntax::custom_operators`](crate::dialect::OperatorSyntax::custom_operators).
    Bare,
}

/// A prefix operator application: the symbolic operator token, then its operand, as in
/// `@ -5` (absolute value), `|/ 25` (square root), `||/ 27` (cube root), `@@ box`
/// (centre point), or a fully user-defined prefix operator `@#@ 24`.
///
/// Under the general operator surface
/// ([`OperatorSyntax::custom_operators`](crate::dialect::OperatorSyntax::custom_operators);
/// PostgreSQL is the current enabler) a dialect admits ANY operator from its `Op` character
/// class in prefix position (PostgreSQL grammar `qual_Op a_expr %prec Op`), so — like the
/// bare infix [`NamedOperatorExpr`] — this is one canonical shape carrying the interned
/// operator spelling rather than an enumerated set. It binds at the "any other operator" rank
/// ([`BindingPowerTable::any_operator`](crate::precedence::BindingPowerTable::any_operator)),
/// the same tier the bare infix operators use. Boxed inside [`Expr::PrefixOperator`] to
/// keep the hot expression enum within its size budget. Distinct from the fixed
/// [`UnaryOperator`] prefixes (`+`/`-`/`NOT`/`~`), which have their own tight precedence
/// and dedicated node.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PrefixOperatorExpr<X: Extension = NoExt> {
    /// The operator symbol spelling (`@`, `|/`, `@@`, `@#@`), interned exact-case so it
    /// round-trips. Always symbolic, never a word, so a bare [`Symbol`] rather than an
    /// [`Ident`].
    pub op: Symbol,
    /// The operand the prefix operator applies to.
    pub operand: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A postfix operator application: the operand, then the trailing symbolic operator token, as
/// in `10 !` (factorial), `1 ~`, `1 <->`, `1 &`, or a fully user-defined operator `1 @#@`.
///
/// DuckDB keeps the generalized postfix reading PostgreSQL removed in version 14
/// ([`OperatorSyntax::postfix_operators`](crate::dialect::OperatorSyntax::postfix_operators);
/// DuckDB is the only enabler): any operator from its `Op` character class — the general
/// residue (the interned [`Symbol`] of a `Custom` operator token, e.g. `!!`/`<->`/`~*`), the
/// lone `~`/`!`/`@`, and the dedicated symbolic infix operators (`&`/`|`/`<<`/`>>`/`||`/`<@`/
/// `@>`/`^@`) — folds here when no operand follows it. Like the bare infix
/// [`NamedOperatorExpr`] it is one canonical shape carrying the interned operator spelling
/// rather than an enumerated set. It binds at the "any other operator" left rank
/// ([`BindingPowerTable::any_operator`](crate::precedence::BindingPowerTable::any_operator)),
/// so a tighter operand groups first (`2 * 3 !` is `(2 * 3)!`) while the postfix stays a
/// complete unary token (`1 ! < 2` is `(1!) < 2`). Boxed inside [`Expr::PostfixOperator`] to
/// keep the hot expression enum within its size budget. The infix reading wins whenever an
/// operand *does* follow (`1 ! + 2` is the infix `1 ! (+2)`), so this node is reached only in
/// the operand-absent position.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PostfixOperatorExpr<X: Extension = NoExt> {
    /// The operand the postfix operator applies to.
    pub operand: Expr<X>,
    /// The operator symbol spelling (`!`, `~`, `<->`, `@#@`), interned exact-case so it
    /// round-trips. Always symbolic, never a word, so a bare [`Symbol`] rather than an
    /// [`Ident`].
    pub op: Symbol,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A DuckDB single-arrow lambda: bound parameters and a body, as in
/// `list_transform([1, 2, 3], x -> x + 1)` or `list_reduce(l, (x, y) -> x + y)`.
///
/// One canonical shape for the new construct: a lambda has no existing
/// shape to fold onto — DuckDB's own tree confirms it with a dedicated `LAMBDA`
/// node. The parameters are plain unqualified identifiers, never expressions:
/// DuckDB's binder rejects anything else ("Parameters must be unqualified
/// comma-separated names like x or (x, y)", probed on 1.5.4), and our parser
/// applies that same shape test to the `->` left operand to pick this node over
/// the JSON-arrow operator — see
/// [`OperatorSyntax::lambda_expressions`](crate::dialect::OperatorSyntax::lambda_expressions)
/// for the whole disambiguation story. The `->` binds at the JSON-arrow rank (it is
/// the same token), so the body captures everything tighter (`x -> x + 1` is
/// `x -> (x + 1)`). Boxed inside [`Expr::Lambda`] to keep the hot expression enum
/// within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LambdaExpr<X: Extension = NoExt> {
    /// The bound parameter names, in source order (always at least one — DuckDB
    /// rejects an empty `() -> …` at parse time).
    pub params: ThinVec<Ident>,
    /// Exact source spelling retained for faithful rendering.
    pub spelling: LambdaParamSpelling,
    /// The lambda body expression.
    pub body: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// How a [`LambdaExpr`] was spelled — the whole surface form, arrow versus keyword.
///
/// The surface-form tag for the one canonical lambda shape, mirroring
/// [`ArraySpelling`]. The three arrow forms differ only in how the parameter list is
/// delimited: DuckDB accepts a bare single name, a parenthesized list, and — because
/// `(x, y)` and `ROW(x, y)` parse to the same row node there — the explicit `ROW(…)`
/// keyword form (all three probed against 1.5.4). The [`Keyword`](Self::Keyword) form is
/// the python-style spelling `lambda x, y: body` DuckDB 1.3.0 introduced (and now
/// prefers over the deprecated arrow): a distinct surface for the same node. The forms
/// bind the same parameters, so only this tag and the rendered spelling distinguish them.
/// [`Bare`](Self::Bare) implies exactly one parameter; the parser upholds that invariant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LambdaParamSpelling {
    /// A bare single parameter with the arrow, as in `x -> x + 1`.
    Bare,
    /// A parenthesized list with the arrow, as in `(x, y) -> x + y` (also the single
    /// `(x) -> …`).
    Parenthesized,
    /// The explicit row-keyword list with the arrow, as in `ROW(x, y) -> x + y`.
    RowKeyword,
    /// The python-style keyword spelling `lambda x, y: body` (comma-separated bare names,
    /// a `:` before the body). DuckDB's current preferred spelling.
    Keyword,
}

/// A prepared-statement parameter placeholder.
///
/// One canonical shape: the placeholder *meaning* is the data, while
/// which surface forms a dialect accepts is gated by
/// [`ParameterSyntax`](crate::dialect::ParameterSyntax). The positional index, the
/// anonymous-by-occurrence form, and a by-name binding are distinct values (not a
/// single surface tag) because they differ semantically, not merely in spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ParameterKind {
    /// PostgreSQL positional `$1`, `$2`, … — the carried value is the 1-based index.
    Positional(u32),
    /// SQLite numbered `?1`, `?123` — the `?`-spelled positional parameter (distinct from
    /// PostgreSQL's `$`-spelled [`Positional`](Self::Positional) so it round-trips as `?N`).
    /// The carried value is the 1-based index; SQLite restricts it to `1..=32766`
    /// (`SQLITE_MAX_VARIABLE_NUMBER`), enforced at parse time.
    Numbered(u32),
    /// Anonymous positional `?` (ODBC/JDBC); its ordinal is its occurrence order.
    Anonymous,
    /// A named placeholder bound by name rather than position (`:name`, `@name`).
    ///
    /// `name` is the interned name *without* its sigil; [`ParameterSigil`] records
    /// which sigil spelled it, so a dialect accepting both forms round-trips each
    /// exactly. The by-name binding is a distinct meaning from the
    /// positional/anonymous forms, hence a value here rather than only a tag.
    Named {
        /// Name referenced by this syntax.
        name: Symbol,
        /// Which sigil introduced the name; see [`ParameterSigil`].
        sigil: ParameterSigil,
    },
}

/// Which sigil introduced a named parameter placeholder.
///
/// The surface-form tag on [`ParameterKind::Named`]: one canonical named-parameter
/// shape covers the spellings, and this records which the source used —
/// the colon form (`:name`; Oracle, SQLite, JDBC/psycopg), the at-sign form
/// (`@name`; T-SQL, SQLite), or the dollar form (`$name`; SQLite) — so rendering
/// restores the original sigil.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ParameterSigil {
    /// `:name`.
    Colon,
    /// `@name`.
    At,
    /// `$name` — SQLite's dollar-named placeholder. Disjoint from the PostgreSQL
    /// positional `$1` ([`ParameterKind::Positional`]) by its follow byte (an
    /// identifier, not a digit).
    Dollar,
}

/// Which session-variable form an [`Expr::SessionVariable`] names.
///
/// One flat enum over the four valid MySQL spellings, so an impossible pairing — a
/// `@`-user variable carrying an explicit system scope — is unrepresentable rather
/// than a runtime invariant (make illegal states unrepresentable). The system forms
/// differ semantically, not merely in spelling: `@@x` reads the server's implicit
/// scope for the variable while `@@global.x` / `@@session.x` force one, so each is
/// its own value, and the sigil (`@` vs `@@`) is recovered from the
/// variant so every form round-trips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SessionVariableKind {
    /// `@name` — a user-defined session variable.
    User,
    /// `@@name` — a system variable at the server's implicit scope.
    System,
    /// `@@global.name` — a system variable read at global scope.
    SystemGlobal,
    /// `@@session.name` — a system variable read at session scope.
    SystemSession,
}

/// A function or aggregate call: `name([DISTINCT] args | *)` plus the optional
/// aggregate modifiers an ordered-set / filtered aggregate carries.
///
/// Boxed inside [`Expr::Function`] so the hot expression enum stays within its
/// size budget. The same canonical shape covers plain scalar calls
/// (`COALESCE`, `NULLIF`, `GREATEST`, `LEAST`, …) and aggregates; unused modifier
/// fields stay at their empty defaults (one shape per construct).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct FunctionCall<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// The `ALL` / `DISTINCT` argument quantifier, as in `count(DISTINCT x)` or
    /// `count(ALL x)`; `None` for a call written with no quantifier. `DISTINCT ON`
    /// is a SELECT-list-only form, so an aggregate never carries it.
    pub quantifier: Option<SetQuantifier>,
    /// The argument list, each a positional or PostgreSQL named argument (see
    /// [`FunctionArg`]). Empty for a no-arg call or a `*` call.
    pub args: ThinVec<FunctionArg<X>>,
    /// `*` as the sole argument, as in `count(*)`. Mutually exclusive with `args`.
    pub wildcard: bool,
    /// `ORDER BY` inside an ordered-set aggregate, as in `array_agg(x ORDER BY y)`.
    pub order_by: ThinVec<OrderByExpr<X>>,
    /// `WITHIN GROUP (ORDER BY <keys>)` — the SQL:2008 ordered-set aggregate clause
    /// (T612/T614), as in `percentile_cont(0.5) WITHIN GROUP (ORDER BY x)`. A
    /// post-argument clause parsed after the `)`, mirroring the optionality of
    /// [`filter`](Self::filter) and [`over`](Self::over); `None` when absent, and the
    /// list is non-empty when present (the grammar requires a sort key). Distinct from
    /// the in-parenthesis [`order_by`](Self::order_by): PostgreSQL admits at most one of
    /// the two, and they render in different positions.
    pub within_group: Option<ThinVec<OrderByExpr<X>>>,
    /// `SEPARATOR <string>` — the MySQL `GROUP_CONCAT(... SEPARATOR ',')` delimiter,
    /// an in-parenthesis argument tail parsed after [`order_by`](Self::order_by) and
    /// before the closing `)`. Gated for acceptance by
    /// [`AggregateCallSyntax::group_concat_separator`](crate::dialect::ExpressionSyntax):
    /// `None` when the dialect leaves the flag off or the clause is unwritten. The
    /// delimiter is always a string constant, so it is a bare [`Literal`] rather than a
    /// general expression — one canonical field on the shared call shape,
    /// not a `GROUP_CONCAT`-specific node.
    pub separator: Option<Literal>,
    /// `FILTER (WHERE <predicate>)` applied to an aggregate.
    pub filter: Option<Box<Expr<X>>>,
    /// Whether the [`filter`](Self::filter) clause wrote the standard `WHERE` keyword
    /// (`FILTER (WHERE p)`) or DuckDB's keyword-less `FILTER (p)`. A pure fidelity tag —
    /// the [`FilterWhereSpelling`] round-trips the surface spelling. Carries the canonical
    /// [`Where`](FilterWhereSpelling::Where) when no filter is present.
    pub filter_where: FilterWhereSpelling,
    /// The `OVER (…)` / `OVER name` window clause that makes this a window
    /// function call; `None` for a plain scalar or aggregate call.
    pub over: Option<WindowSpec<X>>,
    /// `IGNORE NULLS` / `RESPECT NULLS` null-treatment written *inside* the call
    /// parentheses, after the argument list (and any in-parenthesis `ORDER BY`), as in
    /// DuckDB's `last(s IGNORE NULLS) OVER (…)`. `None` when unwritten. One canonical
    /// field on the shared call shape, the [`NullTreatment`] tag recording
    /// which the source used so it round-trips. Gated for acceptance by
    /// [`AggregateCallSyntax::null_treatment`](crate::dialect::CallSyntax): DuckDB spells the
    /// SQL:2016 null-treatment inside the parentheses (the standard's post-`)` position
    /// engine-rejects on 1.5.4), so it rides the in-parenthesis tail like
    /// [`separator`](Self::separator) rather than a post-argument clause. When the
    /// dialect leaves the flag off, `IGNORE`/`RESPECT` is left unconsumed and the
    /// unmatched `)` surfaces as a clean parse error.
    pub null_treatment: Option<NullTreatment>,
    /// The MySQL window-function post-`)` tail — the SQL:2016
    /// `[FROM {FIRST | LAST}] [{RESPECT | IGNORE} NULLS]` clauses a null-treatment
    /// window function carries *between* its argument `)` and its `OVER` clause, as in
    /// `NTH_VALUE(x, 2) FROM FIRST RESPECT NULLS OVER (…)`. `None` when no tail was
    /// written. A separate field from the in-parenthesis DuckDB
    /// [`null_treatment`](Self::null_treatment) precisely because the surface *position*
    /// differs: MySQL spells these after the `)` and rejects the in-paren spelling
    /// (`ER_PARSE_ERROR`, 1064), while DuckDB rejects the post-`)` one — so the two
    /// positions round-trip as distinct fields on the shared call shape
    /// rather than a shared field plus a position tag the DuckDB path would also carry.
    /// Gated for acceptance by
    /// [`AggregateCallSyntax::window_function_tail`](crate::dialect::CallSyntax): MySQL admits it
    /// only on the null-treatment window functions
    /// (`LEAD`/`LAG`/`FIRST_VALUE`/`LAST_VALUE`/`NTH_VALUE`, with `FROM FIRST` on
    /// `NTH_VALUE` alone), enforced by the parser's window-grammar gate.
    pub window_tail: Option<WindowFunctionTail>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One argument of a [`FunctionCall`]: a bare positional value, or a PostgreSQL
/// named argument `name => value` / `name := value`, optionally prefixed by the
/// `VARIADIC` array-spread marker (`VARIADIC arr`, `VARIADIC name => arr`).
///
/// One canonical shape covers all three spellings: a positional
/// argument is `{ name: None, syntax: Positional, value }`, while a named argument
/// carries `name: Some(_)` and an [`ArgSyntax`] surface tag recording which arrow
/// the source wrote so it round-trips. `name` being `Some` ⟺ `syntax` is a named
/// form is an invariant the parser upholds. The argument is a spanned node so the
/// `name => value` extent has side-table identity of its own.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct FunctionArg<X: Extension = NoExt> {
    /// The argument name for a named argument (`name` in `name => value`);
    /// `None` for a positional argument.
    pub name: Option<Symbol>,
    /// The `VARIADIC` marker that spreads an array over a variadic parameter
    /// (`f(a, VARIADIC arr)`). A parse-layer prefix the parser admits only on the
    /// *last* argument of a call and never alongside an `ALL`/`DISTINCT` quantifier
    /// (both PostgreSQL and DuckDB parse-reject the other positions); it composes with
    /// the `name => value` named form, so it is a flag on the argument rather than a
    /// separate call-level field. `false` for an ordinary argument.
    pub variadic: bool,
    /// Which surface form spelled this argument (positional, `=>`, or `:=`).
    pub syntax: ArgSyntax,
    /// Value supplied by this syntax.
    pub value: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// How a [`FunctionArg`] was spelled.
///
/// The surface-form tag for the one canonical [`FunctionArg`] shape,
/// mirroring [`CastSyntax`]: a positional argument carries no name, while the two
/// PostgreSQL named-argument arrows are otherwise the same construct (`=>` is
/// current, `:=` is the deprecated spelling PostgreSQL still accepts), so this
/// records which the source used to restore it on render.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ArgSyntax {
    /// A bare positional argument, as in `f(x)`.
    Positional,
    /// A named argument written `name => value`.
    Arrow,
    /// A named argument written with the deprecated `name := value` spelling.
    ColonEquals,
}

/// The `IGNORE NULLS` / `RESPECT NULLS` null-treatment on a window/aggregate
/// [`FunctionCall`] (SQL:2016 window functions; DuckDB spells it inside the call
/// parentheses).
///
/// Two spellings of the same construct kept as data so the surface round-trips
/// (the [`ArgSyntax`] precedent): `RESPECT NULLS` is the default the
/// engine may canonicalize away, but the parser preserves whichever the source wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum NullTreatment {
    /// `IGNORE NULLS` — skip null values when computing the window/aggregate result.
    IgnoreNulls,
    /// `RESPECT NULLS` — include null values (the default).
    RespectNulls,
}

/// The MySQL window-function post-`)` argument tail (SQL:2016):
/// `[FROM {FIRST | LAST}] [{RESPECT | IGNORE} NULLS]`, written between a null-treatment
/// window function's argument `)` and its `OVER` clause.
///
/// Both clauses are independently optional and appear in this fixed order (the reverse
/// order is a mysql:8 `ER_PARSE_ERROR`); the struct is only constructed when at least
/// one is present, so a [`FunctionCall::window_tail`] of `Some` always carries a
/// clause. The [`NullTreatment`] value is reused from the in-parenthesis DuckDB form —
/// only its surface *position* differs (post-`)` here, in-paren there), which is why
/// this is a separate field rather than a shared one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct WindowFunctionTail {
    /// `FROM FIRST` / `FROM LAST` — which end of the frame `NTH_VALUE` counts its `N`th
    /// row from. `None` when unwritten. On mysql:8 only `FROM FIRST` is accepted (and
    /// only on `NTH_VALUE`); `FROM LAST` is grammar-admitted but feature-rejected
    /// (`ER_NOT_SUPPORTED_YET`, 1235), so the MySQL parser never produces
    /// [`FromFirstLast::Last`].
    pub from_first_last: Option<FromFirstLast>,
    /// `RESPECT NULLS` / `IGNORE NULLS` — the post-`)` null treatment. `None` when
    /// unwritten. On mysql:8 only `RESPECT NULLS` is accepted; `IGNORE NULLS` is
    /// grammar-admitted but feature-rejected (`ER_NOT_SUPPORTED_YET`, 1235), so the
    /// MySQL parser never produces [`NullTreatment::IgnoreNulls`] in this position.
    pub null_treatment: Option<NullTreatment>,
}

/// `FROM FIRST` / `FROM LAST` — which end of the window frame `NTH_VALUE` counts its
/// `N`th row from (SQL:2016 `from_first_last`).
///
/// Two spellings of the same construct kept as data so the surface round-trips (the
/// [`NullTreatment`] precedent). `FROM FIRST` is the default; `FROM LAST` completes the
/// surface for a dialect that admits it (mysql:8 feature-rejects it, so its parser only
/// ever produces `First`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum FromFirstLast {
    /// `FROM FIRST` — count from the first row of the frame (the default).
    First,
    /// `FROM LAST` — count from the last row of the frame.
    Last,
}

/// A `CASE` expression in either standard form.
///
/// Searched `CASE WHEN cond THEN result … [ELSE result] END` has no `operand`;
/// simple `CASE operand WHEN value THEN result … END` carries the `operand` each
/// `WHEN` value is compared against. Boxed inside [`Expr::Case`] to keep the hot
/// expression enum within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CaseExpr<X: Extension = NoExt> {
    /// Optional operand for this syntax.
    pub operand: Option<Box<Expr<X>>>,
    /// The `WHEN … THEN …` branches, in source order (always at least one).
    pub when_clauses: ThinVec<WhenClause<X>>,
    /// Optional else result for this syntax.
    pub else_result: Option<Box<Expr<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL when clause.
pub struct WhenClause<X: Extension = NoExt> {
    /// Predicate that controls this clause.
    pub condition: Expr<X>,
    /// The value produced when the condition holds.
    pub result: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// An `EXTRACT(<field> FROM <source>)` expression.
///
/// `field` is the datetime field name (`YEAR`, `MONTH`, …) interned as an
/// identifier; `source` is the value it is pulled from. Boxed inside
/// [`Expr::Extract`] to keep the hot expression enum within its size budget.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ExtractExpr<X: Extension = NoExt> {
    /// The datetime field name (`YEAR`, `MONTH`, …).
    pub field: Ident,
    /// The value the field is pulled from.
    pub source: Box<Expr<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The SQL/JSON `FORMAT JSON [ENCODING <enc>]` specifier (SQL:2016).
///
/// The only spellable format is `JSON` (PostgreSQL rejects `FORMAT JSONB` at raw
/// parse), so the struct's mere presence records "`FORMAT JSON` was written"; the
/// optional `encoding` is the `ENCODING` tail. PostgreSQL validates the encoding
/// name against `UTF8`/`UTF16`/`UTF32` *at raw parse* ("unrecognized JSON encoding"),
/// so it is a closed [`JsonEncoding`] rather than a free identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct JsonFormat {
    /// Optional encoding for this syntax.
    pub encoding: Option<JsonEncoding>,
}

/// A SQL/JSON `ENCODING` name — the closed set PostgreSQL accepts at raw parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum JsonEncoding {
    /// `UTF8` encoding.
    Utf8,
    /// `UTF16` encoding.
    Utf16,
    /// `UTF32` encoding.
    Utf32,
}

/// A SQL/JSON value expression — an operand with an optional [`JsonFormat`]
/// (`<expr> [FORMAT JSON [ENCODING …]]`, PostgreSQL's `JsonValueExpr`).
///
/// The context item of a query function, each element of a `JSON_ARRAY` value
/// list, the value half of an object member, and the argument of `JSON`/
/// `JSON_SERIALIZE`/aggregates all take this shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct JsonValueExpr<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Box<Expr<X>>,
    /// Optional format for this syntax.
    pub format: Option<JsonFormat>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// An SQL json returning.
pub struct JsonReturning<X: Extension = NoExt> {
    /// Data type named by this syntax.
    pub data_type: Box<DataType<X>>,
    /// Optional format for this syntax.
    pub format: Option<JsonFormat>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One SQL/JSON `PASSING` binding: `<value> [FORMAT JSON] AS <name>`.
///
/// The `FORMAT` rides the [`JsonValueExpr`] value; the name is a `ColLabel`
/// (interns any quote style).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct JsonPassingArg<X: Extension = NoExt> {
    /// Value supplied by this syntax.
    pub value: JsonValueExpr<X>,
    /// Name referenced by this syntax.
    pub name: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A SQL/JSON `ON EMPTY` / `ON ERROR` behaviour handler (PostgreSQL's
/// `JsonBehavior`).
///
/// One shared grammar backs every slot: PostgreSQL accepts any behaviour in any
/// `ON EMPTY`/`ON ERROR` position at raw parse (the per-function legality — e.g.
/// `JSON_VALUE` cannot yield `EMPTY ARRAY` — is a parse-*analysis* check, not a
/// syntax rule), so the [`JsonBehaviorKind`] set is not restricted per function.
/// `default_expr` is `Some` exactly when `kind` is [`Default`](JsonBehaviorKind::Default).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct JsonBehavior<X: Extension = NoExt> {
    /// Which behaviour applies (`NULL`/`ERROR`/`DEFAULT`/…); see [`JsonBehaviorKind`].
    pub kind: JsonBehaviorKind,
    /// Optional default expr for this syntax.
    pub default_expr: Option<Box<Expr<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL json behavior kind forms represented by the AST.
pub enum JsonBehaviorKind {
    /// `ERROR` — raise an error.
    Error,
    /// `NULL` — yield SQL null.
    Null,
    /// `TRUE` — yield `TRUE` (`JSON_EXISTS` only).
    True,
    /// `FALSE` — yield `FALSE` (`JSON_EXISTS` only).
    False,
    /// `UNKNOWN` — yield unknown (`JSON_EXISTS` only).
    Unknown,
    /// `EMPTY` — PostgreSQL's shorthand for `EMPTY ARRAY`; kept distinct so it
    /// round-trips to the bare spelling.
    Empty,
    /// `EMPTY ARRAY` — yield an empty JSON array.
    EmptyArray,
    /// `EMPTY OBJECT` — yield an empty JSON object.
    EmptyObject,
    /// `DEFAULT <expr>` — the fallback value carried in
    /// [`JsonBehavior::default_expr`].
    Default,
}

/// The SQL/JSON `WRAPPER` behaviour of `JSON_QUERY` (SQL:2016).
///
/// The optional `ARRAY` keyword and the `UNCONDITIONAL` default are semantically
/// inert (PostgreSQL normalizes them away), so this closed enum folds
/// `WITH [UNCONDITIONAL] [ARRAY] WRAPPER` onto [`Unconditional`](Self::Unconditional)
/// and `WITHOUT [ARRAY] WRAPPER` onto [`Without`](Self::Without) — the render
/// re-parses to the same value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum JsonWrapperBehavior {
    /// No wrapper clause written.
    Unspecified,
    /// `WITHOUT [ARRAY] WRAPPER`.
    Without,
    /// `WITH [UNCONDITIONAL] [ARRAY] WRAPPER`.
    Unconditional,
    /// `WITH CONDITIONAL [ARRAY] WRAPPER`.
    Conditional,
}

/// The SQL/JSON `QUOTES` behaviour of `JSON_QUERY` (SQL:2016).
///
/// The `ON SCALAR STRING` tail is semantically inert (PostgreSQL normalizes it
/// away), so it is not preserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum JsonQuotesBehavior {
    /// No quotes clause written.
    Unspecified,
    /// `KEEP QUOTES [ON SCALAR STRING]`.
    Keep,
    /// `OMIT QUOTES [ON SCALAR STRING]`.
    Omit,
}

/// Which SQL/JSON query function a [`JsonFuncExpr`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum JsonFuncKind {
    /// `JSON_VALUE` — extract a scalar SQL value at the path.
    Value,
    /// `JSON_QUERY` — extract a JSON fragment at the path.
    Query,
    /// `JSON_EXISTS` — test whether the path matches.
    Exists,
}

/// A SQL/JSON query function — `JSON_VALUE` / `JSON_QUERY` / `JSON_EXISTS`
/// (SQL:2016; PostgreSQL's one `JsonFuncExpr` node with an `op` tag).
///
/// One canonical shape with a [`JsonFuncKind`] tag covers all three, mirroring the
/// engine. The three differ in which trailing clauses their grammar admits — only
/// `JSON_QUERY` takes `wrapper`/`quotes`; `JSON_EXISTS` takes neither `returning`
/// nor `on_empty`; only `JSON_QUERY`/`JSON_VALUE` take `on_empty` — and the parser
/// enforces those restrictions, so an illegal field stays `None`/`Unspecified`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct JsonFuncExpr<X: Extension = NoExt> {
    /// Which function this is (`JSON_VALUE`/`JSON_QUERY`/`JSON_EXISTS`); see [`JsonFuncKind`].
    pub kind: JsonFuncKind,
    /// The JSON value the path is applied to.
    pub context: JsonValueExpr<X>,
    /// The SQL/JSON path expression.
    pub path: Box<Expr<X>>,
    /// The `PASSING` argument bindings, in source order.
    pub passing: ThinVec<JsonPassingArg<X>>,
    /// The optional `RETURNING <type>` clause; see [`JsonReturning`].
    pub returning: Option<JsonReturning<X>>,
    /// The `WITH`/`WITHOUT WRAPPER` behaviour; see [`JsonWrapperBehavior`].
    pub wrapper: JsonWrapperBehavior,
    /// The `KEEP`/`OMIT QUOTES` behaviour; see [`JsonQuotesBehavior`].
    pub quotes: JsonQuotesBehavior,
    /// Optional on empty for this syntax.
    pub on_empty: Option<JsonBehavior<X>>,
    /// Optional on error for this syntax.
    pub on_error: Option<JsonBehavior<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Whether a SQL/JSON object member was spelled `key : value` or `key VALUE value`.
///
/// The optional leading `KEY` keyword is inert (PostgreSQL normalizes it away) and
/// is not preserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum JsonKeyValueSpelling {
    /// `key : value`.
    Colon,
    /// `key VALUE value`.
    Value,
}

/// One SQL/JSON object member: `[KEY] <key> {: | VALUE} <value> [FORMAT JSON]`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct JsonKeyValue<X: Extension = NoExt> {
    /// The member key expression.
    pub key: Box<Expr<X>>,
    /// Value supplied by this syntax.
    pub value: JsonValueExpr<X>,
    /// Exact source spelling retained for faithful rendering.
    pub spelling: JsonKeyValueSpelling,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The SQL/JSON null-handling clause of a constructor: `ABSENT ON NULL` (drop null
/// entries) or `NULL ON NULL` (keep them). `None` records that no clause was written
/// — the two constructors default oppositely (`JSON_OBJECT` keeps, `JSON_ARRAY`
/// drops), so the render must not synthesize the absent clause.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum JsonNullClause {
    /// `ABSENT ON NULL` — drop members/elements whose value is null.
    AbsentOnNull,
    /// `NULL ON NULL` — keep members/elements whose value is null.
    NullOnNull,
}

/// A SQL/JSON object constructor: `JSON_OBJECT([members] [null] [unique]
/// [RETURNING …])` (SQL:2016).
///
/// `entries` may be empty (`JSON_OBJECT()` / `JSON_OBJECT(RETURNING …)`).
/// `unique_keys` is `Some(true)` for `WITH UNIQUE [KEYS]`, `Some(false)` for
/// `WITHOUT UNIQUE [KEYS]`, `None` when unwritten (the inert `KEYS` word is not
/// preserved). This is the standard constructor only; the legacy
/// `json_object(text[])` function keeps the ordinary [`Expr::Function`] shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct JsonObjectExpr<X: Extension = NoExt> {
    /// entries in source order.
    pub entries: ThinVec<JsonKeyValue<X>>,
    /// Optional null clause for this syntax.
    pub null_clause: Option<JsonNullClause>,
    /// Whether the unique keys form was present in the source.
    pub unique_keys: Option<bool>,
    /// The optional `RETURNING <type>` clause; see [`JsonReturning`].
    pub returning: Option<JsonReturning<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The body of a [`JsonArrayExpr`] — a value list or a subquery, PostgreSQL's two
/// distinct `JSON_ARRAY` productions.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum JsonArrayBody<X: Extension = NoExt> {
    /// `JSON_ARRAY(v, … [null])` — a (possibly empty) value list with an optional
    /// null-handling clause.
    Values {
        /// Child items in source order.
        items: ThinVec<JsonValueExpr<X>>,
        /// Optional null clause for this syntax.
        null_clause: Option<JsonNullClause>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `JSON_ARRAY(<query> [FORMAT JSON])` — a subquery whose rows become elements.
    Query {
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Optional format for this syntax.
        format: Option<JsonFormat>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A SQL/JSON array constructor: `JSON_ARRAY(<body> [RETURNING …])` (SQL:2016).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct JsonArrayExpr<X: Extension = NoExt> {
    /// Statement or query body governed by this node.
    pub body: JsonArrayBody<X>,
    /// The optional `RETURNING <type>` clause; see [`JsonReturning`].
    pub returning: Option<JsonReturning<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The body of a [`JsonAggregateExpr`] — the object or array aggregate shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum JsonAggregateBody<X: Extension = NoExt> {
    /// `JSON_OBJECTAGG(k {: | VALUE} v [unique])` — one member, no `ORDER BY`.
    Object {
        /// The single key/value member; see [`JsonKeyValue`].
        entry: JsonKeyValue<X>,
        /// Whether the unique keys form was present in the source.
        unique_keys: Option<bool>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `JSON_ARRAYAGG(v [ORDER BY …])` — one value with an optional sort.
    Array {
        /// Value supplied by this syntax.
        value: JsonValueExpr<X>,
        /// Ordering terms in source order.
        order_by: ThinVec<OrderByExpr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A SQL/JSON aggregate constructor — `JSON_OBJECTAGG` / `JSON_ARRAYAGG`
/// (SQL:2016). Shares the ordinary-aggregate `FILTER (WHERE …)` / `OVER (…)` tail.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct JsonAggregateExpr<X: Extension = NoExt> {
    /// The object or array aggregate body; see [`JsonAggregateBody`].
    pub body: JsonAggregateBody<X>,
    /// Optional null clause for this syntax.
    pub null_clause: Option<JsonNullClause>,
    /// The optional `RETURNING <type>` clause; see [`JsonReturning`].
    pub returning: Option<JsonReturning<X>>,
    /// Optional filter for this syntax.
    pub filter: Option<Box<Expr<X>>>,
    /// Optional over for this syntax.
    pub over: Option<Box<WindowSpec<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Which bare SQL/JSON constructor a [`JsonConstructorExpr`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum JsonConstructorKind {
    /// `JSON(<value> [WITH|WITHOUT UNIQUE [KEYS]])`.
    Json,
    /// `JSON_SCALAR(<expr>)` — a plain argument, no `FORMAT`/`RETURNING`/`UNIQUE`.
    Scalar,
    /// `JSON_SERIALIZE(<value> [RETURNING <type> [FORMAT JSON]])`.
    Serialize,
}

/// A bare SQL/JSON constructor — `JSON(x)` / `JSON_SCALAR(x)` / `JSON_SERIALIZE(x)`
/// (SQL:2016). The unused fields per kind stay `None` (the parser admits only each
/// kind's grammar): `unique_keys` is `JSON`-only, `returning` is `JSON_SERIALIZE`-only,
/// and `JSON_SCALAR`'s value never carries a `FORMAT`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct JsonConstructorExpr<X: Extension = NoExt> {
    /// Which constructor this is (`JSON`/`JSON_SCALAR`/`JSON_SERIALIZE`); see [`JsonConstructorKind`].
    pub kind: JsonConstructorKind,
    /// Value supplied by this syntax.
    pub value: JsonValueExpr<X>,
    /// Whether the unique keys form was present in the source.
    pub unique_keys: Option<bool>,
    /// The optional `RETURNING <type>` clause; see [`JsonReturning`].
    pub returning: Option<JsonReturning<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The type constraint of an `IS JSON` predicate: `IS JSON [VALUE|ARRAY|OBJECT|SCALAR]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum JsonItemType {
    /// Bare `IS JSON` — any JSON type.
    Any,
    /// `IS JSON VALUE` — any JSON scalar, array, or object.
    Value,
    /// `IS JSON ARRAY` — a JSON array.
    Array,
    /// `IS JSON OBJECT` — a JSON object.
    Object,
    /// `IS JSON SCALAR` — a JSON scalar (not an array or object).
    Scalar,
}

/// The SQL/JSON `<expr> IS [NOT] JSON [type] [WITH|WITHOUT UNIQUE [KEYS]]` predicate
/// (SQL:2016; PostgreSQL's `JsonIsPredicate`).
///
/// Binds at `IS`-predicate precedence like the other `IS` tests. `unique_keys`
/// records `WITH UNIQUE [KEYS]` (the inert `KEYS` word and the default `WITHOUT`
/// spelling are not preserved).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct IsJsonExpr<X: Extension = NoExt> {
    /// Expression evaluated by this syntax.
    pub expr: Box<Expr<X>>,
    /// Whether the negated form was present in the source.
    pub negated: bool,
    /// The asserted JSON type (`VALUE`/`ARRAY`/`OBJECT`/`SCALAR`/any); see [`JsonItemType`].
    pub item_type: JsonItemType,
    /// Whether the unique keys form was present in the source.
    pub unique_keys: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A SQL/XML expression function (SQL:2006; PostgreSQL `func_expr_common_subexpr`).
///
/// One kind-tagged enum for the eight special forms PostgreSQL lowers to `XmlExpr`
/// (and `XmlSerialize`): each differs in the clause grammar inside its parens, so a
/// per-form variant carries exactly that form's fields (the aggregate `xmlagg` is an
/// *ordinary* aggregate, not a keyword special form, so it is not here). The
/// contextual keywords each form opens with — `NAME`, `DOCUMENT`/`CONTENT`,
/// `VERSION`, `STANDALONE`, `PASSING`, `INDENT`, `WHITESPACE` — stay unreserved
/// (usable as ordinary names elsewhere); they are consumed only inside these parens.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum XmlFunc<X: Extension = NoExt> {
    /// `xmlelement(NAME <name> [, xmlattributes(<attr>, …)] [, <content>, …])`.
    /// The `xmlattributes(…)` list, when present, precedes the content list (PostgreSQL
    /// rejects content before it); either list may be empty.
    Element {
        /// Name referenced by this syntax.
        name: Ident,
        /// attributes in source order.
        attributes: ThinVec<XmlAttribute<X>>,
        /// content in source order.
        content: ThinVec<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `xmlforest(<value> [AS <name>], …)` — a non-empty list of the same
    /// `<value> [AS <name>]` element shape as an `xmlelement` attribute (reusing
    /// [`XmlAttribute`]).
    Forest {
        /// elements in source order.
        elements: ThinVec<XmlAttribute<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `xmlconcat(<value>, …)` — a non-empty ordinary expression list.
    Concat {
        /// Arguments in source order.
        args: ThinVec<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `xmlparse({DOCUMENT | CONTENT} <value> [{PRESERVE | STRIP} WHITESPACE])`.
    Parse {
        /// Whether the input is a `DOCUMENT` or a `CONTENT` fragment; see [`XmlDocumentOrContent`].
        option: XmlDocumentOrContent,
        /// The XML value to parse.
        arg: Box<Expr<X>>,
        /// The `PRESERVE`/`STRIP WHITESPACE` handling; see [`XmlWhitespaceOption`].
        whitespace: XmlWhitespaceOption,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `xmlpi(NAME <name> [, <content>])` — a single optional content expression.
    Pi {
        /// Name referenced by this syntax.
        name: Ident,
        /// Optional content for this syntax.
        content: Option<Box<Expr<X>>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `xmlroot(<value>, VERSION {<expr> | NO VALUE} [, STANDALONE {YES | NO | NO VALUE}])`.
    /// The `VERSION` clause is mandatory: `version` is `None` exactly for `NO VALUE`
    /// (an unwritten clause is not representable, matching PostgreSQL's grammar).
    Root {
        /// The XML value to modify.
        arg: Box<Expr<X>>,
        /// Optional version for this syntax.
        version: Option<Box<Expr<X>>>,
        /// The `STANDALONE {YES | NO | NO VALUE}` clause; see [`XmlStandalone`].
        standalone: XmlStandalone,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `xmlserialize({DOCUMENT | CONTENT} <value> AS <type> [[NO] INDENT])`.
    Serialize {
        /// Whether the input is a `DOCUMENT` or a `CONTENT` fragment; see [`XmlDocumentOrContent`].
        option: XmlDocumentOrContent,
        /// The XML value to serialize.
        arg: Box<Expr<X>>,
        /// Data type named by this syntax.
        data_type: Box<DataType<X>>,
        /// The `[NO] INDENT` option; see [`XmlIndentOption`].
        indent: XmlIndentOption,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `xmlexists(<path> PASSING [BY {REF | VALUE}] <doc> [BY {REF | VALUE}])`.
    /// The passing mechanism is admitted on either side of the document argument
    /// (PostgreSQL's `xmlexists_argument`); each side round-trips independently.
    Exists {
        /// The XPath expression to evaluate.
        path: Box<Expr<X>>,
        /// Optional mechanism before for this syntax.
        mechanism_before: Option<XmlPassingMechanism>,
        /// The XML document the path is tested against.
        arg: Box<Expr<X>>,
        /// Optional mechanism after for this syntax.
        mechanism_after: Option<XmlPassingMechanism>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One `<value> [AS <name>]` element of an `xmlelement` attribute list or an
/// `xmlforest` list. The `AS <name>` label is a `ColLabel` (any keyword admitted);
/// `None` when the element is a bare value (PostgreSQL derives the tag at analysis).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct XmlAttribute<X: Extension = NoExt> {
    /// Value supplied by this syntax.
    pub value: Box<Expr<X>>,
    /// Name referenced by this syntax.
    pub name: Option<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `DOCUMENT` / `CONTENT` mode word of `xmlparse` / `xmlserialize`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum XmlDocumentOrContent {
    /// `DOCUMENT` — the input is a single well-formed XML document.
    Document,
    /// `CONTENT` — the input is an XML content fragment.
    Content,
}

/// The optional whitespace-handling clause of `xmlparse`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum XmlWhitespaceOption {
    /// No clause written.
    Unspecified,
    /// `PRESERVE WHITESPACE`.
    Preserve,
    /// `STRIP WHITESPACE`.
    Strip,
}

/// The optional indentation clause of `xmlserialize`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum XmlIndentOption {
    /// No clause written.
    Unspecified,
    /// `INDENT`.
    Indent,
    /// `NO INDENT`.
    NoIndent,
}

/// The optional `STANDALONE` clause value of `xmlroot`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum XmlStandalone {
    /// No `STANDALONE` clause written.
    Unspecified,
    /// `STANDALONE YES`.
    Yes,
    /// `STANDALONE NO`.
    No,
    /// `STANDALONE NO VALUE`.
    NoValue,
}

/// The `BY REF` / `BY VALUE` passing mechanism of `xmlexists`. Both spellings are
/// admitted at parse (PostgreSQL treats `BY REF` as the default and rejects `BY
/// VALUE` only later), so the tag round-trips the exact source word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum XmlPassingMechanism {
    /// `BY REF` — pass the XML by reference (PostgreSQL's default).
    ByRef,
    /// `BY VALUE` — pass the XML by value.
    ByValue,
}

/// A standard-SQL string special form (SQL-92 E021-06/-09/-11 + SQL:1999 T312;
/// PostgreSQL's `func_expr_common_subexpr` string productions).
///
/// One kind-tagged enum for the keyword-argument string functions: each variant
/// carries exactly its form's typed fields, and every operand keyword (`FROM`,
/// `FOR`, `SIMILAR`, `ESCAPE`, `PLACING`, `IN`, `LEADING`/`TRAILING`/`BOTH`) is
/// grammar, not data. The comma plain-call spellings (`substring(x, 1, 2)`,
/// `trim(x, y)`, `overlay(a, b, c)`) are *not* here — they stay ordinary
/// [`FunctionCall`]s, so the plain-call surface every engine also accepts is
/// unaffected by the special forms.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum StringFunc<X: Extension = NoExt> {
    /// `SUBSTRING(<expr> FROM <start> [FOR <count>])` and its variants: PostgreSQL
    /// admits `FOR <count>` alone and the reversed `FOR <count> FROM <start>`
    /// spelling (both orders fold onto the same fields and render canonically
    /// `FROM`-first), so at least one of `start`/`count` is always present. The
    /// string-pattern form `SUBSTRING(x FROM 'pat')`/`… FOR '#'` is this same
    /// production (the regex reading is runtime semantics, not grammar).
    Substring {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// The `FROM <start>` operand, `None` for the `FOR`-only form.
        start: Option<Box<Expr<X>>>,
        /// The `FOR <count>` operand, `None` for the `FROM`-only form.
        count: Option<Box<Expr<X>>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// PostgreSQL's `SUBSTRING(<expr> SIMILAR <pattern> ESCAPE <escape>)` regex
    /// form (SQL:1999 `<regular expression substring function>`). All three
    /// operands are mandatory (`SUBSTRING(x SIMILAR p)` without `ESCAPE` rejects,
    /// engine-verified).
    SubstringSimilar {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// The SQL-regex pattern.
        pattern: Box<Expr<X>>,
        /// The escape-character expression.
        escape: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `POSITION(<substr> IN <string>)` (SQL-92 E021-11). The operands are the
    /// restricted `b_expr` in PostgreSQL/DuckDB (`POSITION(1 IN 2 OR 3)` rejects)
    /// and MySQL's asymmetric `bit_expr IN expr` under
    /// [`StringFuncForms::position_asymmetric_operands`](crate::dialect::CallSyntax).
    /// There is no plain-call spelling: every keyword-form engine parse-rejects
    /// `position(a, b)`.
    Position {
        /// The substring to search for.
        substr: Box<Expr<X>>,
        /// The string searched within.
        string: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `OVERLAY(<target> PLACING <replacement> FROM <start> [FOR <count>])`
    /// (SQL:1999 T312). `FROM <start>` is mandatory — `OVERLAY(x PLACING y)` and
    /// the `FOR`-without-`FROM` form parse-reject on every keyword-form engine.
    Overlay {
        /// Object targeted by this syntax.
        target: Box<Expr<X>>,
        /// The replacement string spliced in.
        replacement: Box<Expr<X>>,
        /// The 1-based start position.
        start: Box<Expr<X>>,
        /// Optional count for this syntax.
        count: Option<Box<Expr<X>>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `TRIM([{BOTH | LEADING | TRAILING}] [<chars>] FROM <sources>…)` (SQL-92
    /// E021-09) plus PostgreSQL's loose `trim_list` tails under
    /// [`StringFuncForms::trim_list_syntax`](crate::dialect::CallSyntax): a bare
    /// `FROM <list>`, a side without `FROM` (`TRIM(TRAILING ' foo ')`), and a
    /// multi-expression list (`TRIM('a' FROM 'b', 'c')`). At least one of
    /// `side`/`from` is present — a bare `TRIM(x)`/`TRIM(x, y)` is an ordinary
    /// call, never this node.
    Trim {
        /// The written `BOTH`/`LEADING`/`TRAILING` side, `None` when omitted.
        side: Option<TrimSide>,
        /// The trim-character expression written before `FROM`; `None` for the
        /// bare-`FROM` and side-without-`FROM` forms.
        trim_chars: Option<Box<Expr<X>>>,
        /// Whether `FROM` was written (distinguishes `TRIM(TRAILING ' foo ')`
        /// from `TRIM(TRAILING FROM ' foo ')` — both are valid PostgreSQL with
        /// different meanings, so the bit is load-bearing for round-trips).
        from: bool,
        /// The source expression list after `FROM` (or the bare list when a side
        /// is written without `FROM`). PostgreSQL's `trim_list` is an `expr_list`,
        /// so more than one source parses there; the restricted dialects hold this
        /// to exactly one at parse.
        sources: ThinVec<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// PostgreSQL's `COLLATION FOR (<expr>)` — the common-subexpr that reports the
    /// collation name derived for its operand. PostgreSQL gives it a dedicated
    /// `COLLATION FOR '(' a_expr ')'` production that lowers to a
    /// `pg_catalog.pg_collation_for(<expr>)` call, but the surface keyword form is
    /// kept here (not folded to a [`FunctionCall`]) so it round-trips as written. The
    /// parentheses and single `a_expr` operand are mandatory — `COLLATION FOR 'x'`,
    /// `COLLATION FOR ()`, and a two-argument list all parse-reject (engine-verified).
    CollationFor {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's `CONVERT(<expr> USING <charset>)` transcoding form — reinterprets its
    /// string operand in another character set (the grammar's
    /// `CONVERT '(' expr USING charset_name ')'`). Distinct from the comma-form cast
    /// [`CastSyntax::Convert`]: this changes the charset, not the type, so it is a string
    /// special form rather than a cast. The operand is a full `a_expr`
    /// (`CONVERT(1+2 USING utf8mb4)` parses, engine-verified); `charset` is a MySQL
    /// `charset_name` — an `ident_or_text` (a bare or backtick identifier, or a quoted
    /// string, round-tripping by the [`Ident`]'s quote style) or the `BINARY` transcoding
    /// name (`CONVERT(x USING binary)`), which reaches here as a bare [`Ident`]. Recognized
    /// only under [`CallSyntax::convert_function`](crate::dialect::CallSyntax); MySQL-only.
    ConvertUsing {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// The target character set name.
        charset: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's full-text search `MATCH (<col>, …) AGAINST (<expr> [<modifier>])`
    /// (`simple_expr: MATCH ident_list_arg AGAINST '(' bit_expr fulltext_options ')'`).
    /// The `columns` are a comma list of column references (bare or 1–3-part dotted
    /// [`Expr::Column`]s — an arbitrary expression, literal, function call, or empty
    /// list all parse-reject, engine-verified); `against` is a MySQL `bit_expr` (below
    /// the comparison level, so a trailing `IN`/`WITH` opens the modifier rather than an
    /// `IN` predicate). The optional [`MatchSearchModifier`] is exactly one of the four
    /// documented combinations; `None` is the default (no modifier words) and round-trips
    /// as written rather than as an explicit `IN NATURAL LANGUAGE MODE`. Recognized only
    /// under [`StringFuncForms::match_against`](crate::dialect::CallSyntax); MySQL-only, and
    /// distinct from SQLite's infix `<expr> MATCH <expr>` operator. Semantic constraints
    /// (a full-text index must cover the columns, the operand must be constant) are a
    /// server binding concern, not grammar.
    MatchAgainst {
        /// Columns in source order.
        columns: ThinVec<Expr<X>>,
        /// The full-text search expression.
        against: Box<Expr<X>>,
        /// Optional modifier for this syntax.
        modifier: Option<MatchSearchModifier>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CEIL(<expr> TO <field>)` / `CEILING(<expr> TO <field>)` — a rounding-field
    /// keyword form distinct from the ordinary `CEIL(<expr>)` call and the comma-form
    /// scale spelling `CEIL(<expr>, <scale>)` (which stays an ordinary [`FunctionCall`]:
    /// no probed oracle grammar admits the `TO` tail, engine-verified against pg_query,
    /// DuckDB, and mysql:8.4). The field (`DAY`, `HOUR`, …) is stored as written and
    /// validated, if at all, by the consuming engine at analysis time, not parse.
    /// Recognized only under
    /// [`StringFuncForms::ceil_to_field`](crate::dialect::StringFuncForms::ceil_to_field).
    CeilTo {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// The rounding field (`DAY`, `HOUR`, …).
        field: Ident,
        /// Exact source spelling retained for faithful rendering.
        spelling: CeilSpelling,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `FLOOR(<expr> TO <field>)` — a rounding-field keyword form distinct from the
    /// ordinary `FLOOR(<expr>)` call and the comma-form scale spelling
    /// `FLOOR(<expr>, <scale>)` (which stays an ordinary [`FunctionCall`]: no probed
    /// oracle grammar admits the `TO` tail, engine-verified against pg_query, DuckDB, and
    /// mysql:8.4). Unlike [`StringFunc::CeilTo`], `FLOOR` has no `FLOORING` synonym, so
    /// there is no spelling field to track. The field (`DAY`, `HOUR`, …) is stored as
    /// written and validated, if at all, by the consuming engine at analysis time, not
    /// parse. Recognized only under
    /// [`StringFuncForms::floor_to_field`](crate::dialect::StringFuncForms::floor_to_field).
    FloorTo {
        /// Expression evaluated by this syntax.
        expr: Box<Expr<X>>,
        /// The rounding field (`DAY`, `HOUR`, …).
        field: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The optional search modifier of a MySQL [`StringFunc::MatchAgainst`] — one of the
/// four documented full-text combinations. `WITH QUERY EXPANSION` combines only with
/// the (implicit or explicit) natural-language mode; `IN BOOLEAN MODE WITH QUERY
/// EXPANSION` parse-rejects (engine-verified).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum MatchSearchModifier {
    /// `IN NATURAL LANGUAGE MODE`.
    NaturalLanguage,
    /// `IN NATURAL LANGUAGE MODE WITH QUERY EXPANSION`.
    NaturalLanguageQueryExpansion,
    /// `IN BOOLEAN MODE`.
    Boolean,
    /// `WITH QUERY EXPANSION`.
    QueryExpansion,
}

/// The `BOTH` / `LEADING` / `TRAILING` side word of a [`StringFunc::Trim`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TrimSide {
    /// `TRIM(BOTH …)` — trim the character from both ends.
    Both,
    /// `TRIM(LEADING …)` — trim from the start only.
    Leading,
    /// `TRIM(TRAILING …)` — trim from the end only.
    Trailing,
}

/// Quantifier used by `op ANY/ALL/SOME (<query>)` predicates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum Quantifier {
    /// `ANY` — the predicate holds for at least one row/element.
    Any,
    /// `ALL` — the predicate holds for every row/element.
    All,
    /// `SOME` — standard synonym for `ANY`.
    Some,
}

/// Closed operator keys for dialect binding-power tables.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum BinaryOperator {
    /// `+` — addition.
    Plus,
    /// `-` — subtraction.
    Minus,
    /// `*` — multiplication.
    Multiply,
    /// `/` — division.
    Divide,
    /// Modulo, spelled `%` everywhere and additionally `MOD` in MySQL. The two
    /// spellings are one operator; the [`ModuloSpelling`] tag records
    /// which the source used so it round-trips.
    Modulo(ModuloSpelling),
    /// Integer division — a distinct operator from `/` (it truncates to an integer), so
    /// it gets its own key rather than reusing [`Divide`](Self::Divide). Two
    /// dialect-disjoint spellings fold onto it: MySQL's `DIV` keyword and
    /// DuckDB's `//` symbol; the [`IntegerDivideSpelling`] tag records which the source
    /// used so it round-trips. Binds at multiplicative precedence.
    IntegerDivide(IntegerDivideSpelling),
    /// Arithmetic exponentiation, spelled `^`. A distinct operator from the
    /// [`BitwiseXor`](Self::BitwiseXor) `^` (a dialect gives the `^` lexeme one meaning or the
    /// other — engine truth): under
    /// [`CaretOperator::Exponent`](crate::dialect::CaretOperator)
    /// (PostgreSQL/DuckDB) `^` is arithmetic power,
    /// and binds at its OWN precedence tier — tighter than `*`/`/`/`%`
    /// ([`multiplicative`](crate::precedence::BindingPowerTable::multiplicative)) and looser
    /// than the unary sign, left-associative (`2 ^ 3 ^ 2` is `(2 ^ 3) ^ 2`, `2 ^ 3 * 2` is
    /// `(2 ^ 3) * 2` — engine-measured on pg_query). Its binding power is the dedicated
    /// [`exponent`](crate::precedence::BindingPowerTable::exponent) row, not the multiplicative
    /// rank the other arithmetic operators share.
    Exponent,
    /// `||` — string concatenation.
    StringConcat,
    /// PostgreSQL `@>` containment — "left contains right" over arrays, ranges, and
    /// `jsonb`. Binds at PostgreSQL's "any other operator" precedence (the rank shared
    /// with [`StringConcat`](Self::StringConcat)), left-associative.
    Contains,
    /// PostgreSQL `<@` containment — "left is contained by right", the mirror of
    /// [`Contains`](Self::Contains). Same "any other operator" precedence.
    ContainedBy,
    /// DuckDB `^@` — "left string starts with right string". Binds at the "any other
    /// operator" precedence. Gated by [`starts_with_operator`](crate::dialect::OperatorSyntax::starts_with_operator).
    StartsWith,
    /// The `&&` overlap operator — "do the two operands overlap?" over arrays, ranges
    /// (PostgreSQL), and geometries (DuckDB, whose `&&` is bounding-box overlap). The one
    /// operator key for the `&&` spelling across dialects that give it this meaning:
    /// DuckDB routes it here through [`DoubleAmpersand::Overlaps`], and PostgreSQL's
    /// range/array `&&` (deferred) would fold onto the same variant. Binds at the "any
    /// other operator" precedence (the [`Contains`](Self::Contains) rank), left-associative.
    ///
    /// Distinct from [`Overlaps`](Self::Overlaps), the SQL-standard `OVERLAPS` *keyword*
    /// period predicate over `(start, end)` rows — a different surface (keyword vs symbol),
    /// operand shape (two-element rows vs scalars), and render (`OVERLAPS` vs `&&`).
    ///
    /// [`DoubleAmpersand::Overlaps`]: crate::dialect::DoubleAmpersand::Overlaps
    Overlap,
    /// PostgreSQL `->` JSON access — object field / array element returned as
    /// `json`/`jsonb`. Same "any other operator" precedence.
    JsonGet,
    /// PostgreSQL `->>` JSON access — object field / array element returned as `text`,
    /// the text-typed form of [`JsonGet`](Self::JsonGet). Same precedence.
    JsonGetText,
    /// PostgreSQL `?` `jsonb` key/element existence — "does the right text string exist as
    /// a top-level key (object) or array element (array) of the left `jsonb`?". Binds at
    /// PostgreSQL's "any other operator" precedence (the [`Contains`](Self::Contains) rank),
    /// left-associative. Lexed only under [`OperatorSyntax::jsonb_operators`]; the `?` byte
    /// is otherwise a stray byte in PostgreSQL (it has no `?` parameter) or the anonymous
    /// placeholder elsewhere.
    ///
    /// [`OperatorSyntax::jsonb_operators`]: crate::dialect::OperatorSyntax::jsonb_operators
    JsonExists,
    /// PostgreSQL `?|` `jsonb` any-key existence — "does any of the right `text[]` strings
    /// exist as a top-level key/element of the left `jsonb`?". Same "any other operator"
    /// precedence as [`JsonExists`](Self::JsonExists).
    JsonExistsAny,
    /// PostgreSQL `?&` `jsonb` all-keys existence — "do all of the right `text[]` strings
    /// exist as top-level keys/elements of the left `jsonb`?". Same precedence.
    JsonExistsAll,
    /// PostgreSQL `@?` — "does the right `jsonpath` return any item for the left `jsonb`?".
    /// Same "any other operator" precedence.
    JsonPathExists,
    /// PostgreSQL `@@` — the match operator: for `jsonb @@ jsonpath` it returns the result
    /// of the JSON-path predicate check, and it is also the `tsvector @@ tsquery` full-text
    /// search match. One operator key for the shared `@@` spelling; same "any other operator"
    /// precedence.
    JsonPathMatch,
    /// PostgreSQL `#>` — extract the `jsonb` sub-object at the right `text[]` path, returned
    /// as `jsonb`. Same "any other operator" precedence.
    JsonExtractPath,
    /// PostgreSQL `#>>` — extract the `jsonb` sub-object at the right `text[]` path, returned
    /// as `text`, the text-typed form of [`JsonExtractPath`](Self::JsonExtractPath). Same
    /// precedence.
    JsonExtractPathText,
    /// PostgreSQL `#-` — delete the field/element of the left `jsonb` at the right `text[]`
    /// path. Same "any other operator" precedence. The `#-` lexeme is munched over the two
    /// contiguous bytes ahead of the bare `#` bitwise-XOR (engine-verified: `5#-3` is
    /// `5 #- 3`, while a space splits it into `#` then `-3`).
    JsonDeletePath,
    /// Bitwise OR, spelled `|` (PostgreSQL, MySQL, SQLite, DuckDB). In PostgreSQL/SQLite/
    /// DuckDB it shares one "bitwise" precedence with `&`/`<<`/`>>` (between additive and
    /// comparison); MySQL ranks it strictly looser than `&` (the load-bearing per-dialect
    /// precedence split), so its binding power is dialect data
    /// ([`BindingPowerTable::bitwise_or`](crate::precedence::BindingPowerTable::bitwise_or)).
    BitwiseOr,
    /// Bitwise AND, spelled `&` (PostgreSQL, MySQL, SQLite, DuckDB). Shares the one bitwise
    /// rank with `|`/`<<`/`>>` in PostgreSQL/SQLite/DuckDB; binds tighter than `|` and
    /// looser than the shifts in MySQL (dialect data, see
    /// [`BindingPowerTable::bitwise_and`](crate::precedence::BindingPowerTable::bitwise_and)).
    BitwiseAnd,
    /// Bitwise left shift, spelled `<<` (PostgreSQL, MySQL, SQLite, DuckDB). Grouped with
    /// [`BitwiseShiftRight`](Self::BitwiseShiftRight) at one shift rank in every dialect —
    /// looser than additive everywhere (`1 << 2 + 3` is `1 << (2 + 3)`, engine-measured on
    /// SQLite/PG/DuckDB) — but that rank sits *below* additive and *above* `&` in MySQL
    /// (dialect data,
    /// [`BindingPowerTable::bitwise_shift`](crate::precedence::BindingPowerTable::bitwise_shift)).
    BitwiseShiftLeft,
    /// Bitwise right shift, spelled `>>` (PostgreSQL, MySQL, SQLite, DuckDB). The mirror of
    /// [`BitwiseShiftLeft`](Self::BitwiseShiftLeft); same shift rank.
    BitwiseShiftRight,
    /// Bitwise exclusive-or. Two dialect-disjoint spellings fold onto this one operator:
    /// PostgreSQL's `#` and MySQL's `^`. The [`BitwiseXorSpelling`] tag is
    /// load-bearing for validity, not only fidelity — PostgreSQL rejects `^` as XOR (there
    /// `^` is exponentiation) and MySQL treats `#` as a comment — so a normalized render
    /// would not re-parse under the dialect that produced it (the same contract
    /// [`IsNotDistinctFrom`](Self::IsNotDistinctFrom) keeps). The two spellings *also* bind
    /// at different precedences: PostgreSQL's `#` is an "any other operator" (looser than
    /// additive), MySQL's `^` binds tighter than `*` — dialect data on
    /// [`BindingPowerTable::bitwise_xor`](crate::precedence::BindingPowerTable::bitwise_xor).
    /// Distinct from the *logical* [`Xor`](Self::Xor) keyword operator (MySQL `XOR`).
    BitwiseXor(BitwiseXorSpelling),
    /// Equality, spelled `=` everywhere and additionally `==` in SQLite. The two
    /// spellings are one operator; the [`EqualsSpelling`] tag records
    /// which the source used so it round-trips.
    Eq(EqualsSpelling),
    /// Inequality, spelled `<>` (SQL-standard) everywhere and additionally `!=`
    /// (C-style) in every bundled dialect. The two spellings are one operator; the
    /// [`NotEqSpelling`] tag records which the source used so it round-trips.
    NotEq(NotEqSpelling),
    /// `<` — less-than.
    Lt,
    /// `<=` — less-than-or-equal.
    LtEq,
    /// `>` — greater-than.
    Gt,
    /// `>=` — greater-than-or-equal.
    GtEq,
    /// The null-safe inequality predicate `IS DISTINCT FROM` (SQL:1999 T151): true when
    /// the operands differ, treating `NULL` as an ordinary comparable value rather than
    /// yielding `NULL`. The parser recognizes it in the `IS` predicate arm, not the
    /// symbolic-operator loop. Binds at comparison precedence, non-associative like `=`
    /// (PostgreSQL `a_expr IS DISTINCT FROM a_expr %prec IS`, gram.y). Two
    /// interchangeable-semantics spellings fold onto it: the SQL:1999 `IS DISTINCT FROM`
    /// keyword form and SQLite's bare `IS NOT` (`a IS NOT b`, SQLite's general negated
    /// `IS`). The [`IsDistinctFromSpelling`] tag records which the source used so the
    /// surface round-trips, mirroring the [`IsNotDistinctFrom`](Self::IsNotDistinctFrom)
    /// complement.
    IsDistinctFrom(IsDistinctFromSpelling),
    /// The complement of [`IsDistinctFrom`](Self::IsDistinctFrom): the null-safe
    /// equality predicate, true when the operands are equal or both `NULL`. A distinct
    /// operator key at the same comparison precedence. Two interchangeable-semantics
    /// spellings fold onto it: the SQL:1999 `IS NOT DISTINCT FROM` keyword
    /// form (which SQLite's general `IS` also produces), recognized in the `IS`-predicate
    /// arm; and MySQL's `<=>` operator, recognized in the symbolic-operator loop. The
    /// [`IsNotDistinctFromSpelling`] tag records which the source used. Unlike the other
    /// spelling tags this is load-bearing for validity, not only fidelity: MySQL rejects
    /// the keyword form and the other dialects reject `<=>`, so the spelling cannot be
    /// normalized away without producing input the same dialect fails to re-parse.
    IsNotDistinctFrom(IsNotDistinctFromSpelling),
    /// MySQL `RLIKE` / `REGEXP` regular-expression match. The two keywords are
    /// synonyms folding onto one operator; the [`RegexpSpelling`] tag
    /// records which the source used. Binds at comparison precedence (like `LIKE`).
    Regexp(RegexpSpelling),
    /// SQLite's `GLOB` pattern-match operator — case-sensitive Unix-glob matching
    /// (`*`/`?`/`[…]`). A keyword infix operator ([`KeywordOperators::Sqlite`]), the
    /// SQLite analogue of MySQL's `RLIKE`. Binds at comparison precedence (like
    /// `LIKE`/`REGEXP`).
    ///
    /// [`KeywordOperators::Sqlite`]: crate::dialect::KeywordOperators::Sqlite
    Glob,
    /// SQLite's `MATCH` operator — a grammar hook whose meaning is supplied by an
    /// application-defined function (FTS, R-Tree). A keyword infix operator
    /// ([`KeywordOperators::Sqlite`]) sibling of [`Glob`](Self::Glob); binds at
    /// comparison precedence. The bundled engine registers no `match` backing, so a
    /// bare `prepare` rejects it — it is grammar-only, guarded by round-trip rather
    /// than an accept/reject oracle.
    ///
    /// [`KeywordOperators::Sqlite`]: crate::dialect::KeywordOperators::Sqlite
    Match,
    /// The SQL-standard `OVERLAPS` period predicate (SQL:2016 F251): `(s1, e1) OVERLAPS
    /// (s2, e2)` — true when the two time periods, each given as a `(start, end |
    /// duration)` pair, share any instant. Both operands are exactly-two-element rows
    /// ([`Expr::Row`], bare parenthesized pair or `ROW(...)`), a shape the parser enforces
    /// (a scalar, a single-element grouping, or a three-element row is a parse error,
    /// matching PostgreSQL's `row OVERLAPS row` production and its wrong-arity `ereport`).
    /// The boolean result is not itself a row, so the predicate never chains
    /// (`x OVERLAPS y OVERLAPS z` rejects) — modelled non-associative. Binds tighter than
    /// the comparison operators (`x OVERLAPS y = TRUE` groups `(x OVERLAPS y) = TRUE`) and
    /// looser than the arithmetic/`Op` rank, its own PostgreSQL `%nonassoc OVERLAPS` gram.y
    /// row. Gated by
    /// [`PredicateSyntax::overlaps_period_predicate`](crate::dialect::PredicateSyntax::overlaps_period_predicate).
    Overlaps,
    /// `AND` — logical conjunction.
    And,
    /// MySQL `XOR` logical exclusive-or. Binds between `AND` and `OR` in precedence.
    Xor,
    /// `OR` — logical disjunction.
    Or,
}

/// Surface spelling for the modulo operator [`BinaryOperator::Modulo`].
///
/// `%` is universal; MySQL additionally spells the same operation with the `MOD`
/// keyword. The canonical AST keeps one modulo operator and this tag
/// only records which spelling the source used so rendering round-trips exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ModuloSpelling {
    /// The `%` operator.
    Percent,
    /// The MySQL `MOD` keyword.
    Mod,
}

/// Surface spelling for the integer-division operator [`BinaryOperator::IntegerDivide`].
///
/// Integer division has two dialect-disjoint spellings that fold onto one operator:
/// MySQL's `DIV` keyword and DuckDB's `//` symbol. Neither dialect accepts the
/// other's spelling (MySQL has no `//` operator — a bare `//` is a syntax error there — and
/// DuckDB has no `DIV` keyword), so this tag is load-bearing for validity, not only
/// fidelity, mirroring [`BitwiseXorSpelling`]: a normalized render would not re-parse under
/// the dialect that produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IntegerDivideSpelling {
    /// The MySQL `DIV` keyword.
    Div,
    /// The DuckDB `//` operator.
    SlashSlash,
}

/// Surface spelling for the equality operator [`BinaryOperator::Eq`].
///
/// `=` is universal; SQLite additionally spells the same comparison with a doubled
/// `==`. The canonical AST keeps one equality operator and this tag only
/// records which spelling the source used so rendering round-trips exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum EqualsSpelling {
    /// The `=` operator.
    Single,
    /// The SQLite `==` operator.
    Double,
}

/// Whether an aggregate `FILTER (…)` clause wrote the SQL-standard `WHERE` keyword
/// before its predicate ([`FunctionCall::filter_where`]).
///
/// SQL:2003 (and PostgreSQL/SQLite) require `FILTER (WHERE <predicate>)`; DuckDB
/// additionally accepts the keyword-less `FILTER (<predicate>)`
/// ([`AggregateCallSyntax::filter_optional_where`](crate::dialect::AggregateCallSyntax::filter_optional_where)).
/// The canonical AST keeps one filtered-aggregate shape and this tag records which the
/// source used so rendering round-trips exactly, mirroring [`EqualsSpelling`]. Only
/// meaningful when [`FunctionCall::filter`] is `Some`; a call with no filter carries
/// the canonical [`Where`](Self::Where).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum FilterWhereSpelling {
    /// The SQL-standard `FILTER (WHERE <predicate>)` spelling (the canonical form).
    Where,
    /// DuckDB's keyword-less `FILTER (<predicate>)` spelling.
    Omitted,
}

/// Surface spelling for the inequality operator [`BinaryOperator::NotEq`].
///
/// The SQL-standard `<>` is universal; every bundled dialect additionally spells the
/// same comparison with the C-style `!=`. The canonical AST keeps one inequality
/// operator and this tag only records which spelling the source used so rendering
/// round-trips exactly, mirroring [`EqualsSpelling`]. A fidelity tag, not a validity
/// one: both spellings parse under every dialect, so the canonical `<>` re-parses
/// wherever the source `!=` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum NotEqSpelling {
    /// The SQL-standard `<>` operator (the canonical spelling).
    AngleBracket,
    /// The C-style `!=` operator.
    Bang,
}

/// Surface spelling for the null-safe inequality operator
/// [`BinaryOperator::IsDistinctFrom`].
///
/// The predicate has two interchangeable-semantics spellings: the SQL:1999
/// `IS DISTINCT FROM` keyword form and SQLite's bare `IS NOT` (SQLite's general
/// negated `IS`). The canonical AST keeps one operator and this tag records which the
/// source used so rendering round-trips, mirroring [`IsNotDistinctFromSpelling`]. Both
/// spellings are valid under SQLite (which accepts the explicit keyword form too), so
/// this is a fidelity tag rather than a validity one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IsDistinctFromSpelling {
    /// The `IS DISTINCT FROM` keyword form (SQL:1999 T151).
    Keyword,
    /// SQLite's bare `IS NOT`: `a IS NOT b` is null-safe inequality, folding onto this
    /// operator. Renders back as bare `IS NOT` so the surface round-trips.
    Is,
}

/// Surface spelling for the null-safe equality operator
/// [`BinaryOperator::IsNotDistinctFrom`].
///
/// The predicate has two interchangeable-semantics spellings: the SQL:1999
/// `IS NOT DISTINCT FROM` keyword form (which SQLite's general `IS` also folds onto)
/// and MySQL's `<=>` operator. The canonical AST keeps one operator and this
/// tag records which the source used so rendering round-trips. Unlike the sibling
/// spelling tags this is load-bearing for validity, not only fidelity: MySQL rejects
/// `IS NOT DISTINCT FROM` and the other dialects reject `<=>`, so a normalized render
/// would not re-parse under the dialect that produced it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum IsNotDistinctFromSpelling {
    /// The `IS NOT DISTINCT FROM` keyword form (SQL:1999 T151).
    Keyword,
    /// The MySQL `<=>` null-safe-equality operator.
    NullSafeEq,
    /// SQLite's bare general `IS`: `a IS b` is null-safe equality, folding onto this
    /// operator (SQLite also accepts the explicit `IS NOT DISTINCT FROM`, which keeps
    /// [`Keyword`](Self::Keyword)). Renders back as bare `IS` so the surface round-trips.
    Is,
}

/// Which truth value an [`Expr::IsTruth`] predicate tests (SQL:2016 F571,
/// `<truth value>` in the `<boolean test>` grammar). The three-valued-logic constants:
/// `TRUE`, `FALSE`, and `UNKNOWN` (the boolean `NULL`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TruthValue {
    /// `IS [NOT] TRUE`.
    True,
    /// `IS [NOT] FALSE`.
    False,
    /// `IS [NOT] UNKNOWN`.
    Unknown,
}

/// Surface spelling for the bitwise exclusive-or operator [`BinaryOperator::BitwiseXor`].
///
/// XOR has two dialect-disjoint spellings that fold onto one operator:
/// PostgreSQL's `#` and MySQL's `^`. Unlike the cosmetic spelling tags, this is
/// load-bearing for validity, mirroring [`IsNotDistinctFromSpelling`]: PostgreSQL's `^`
/// is *exponentiation* (not XOR) and MySQL's `#` opens a comment, so each dialect rejects
/// the other's spelling — a normalized render would not re-parse. The two also differ in
/// precedence, which lives on the dialect's binding-power table, not here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum BitwiseXorSpelling {
    /// The PostgreSQL `#` operator.
    Hash,
    /// The MySQL `^` operator.
    Caret,
}

/// Surface spelling for the regex-match operator [`BinaryOperator::Regexp`].
///
/// MySQL spells regular-expression match two interchangeable ways, `RLIKE` and its
/// `REGEXP` synonym. The canonical AST keeps one operator and this tag
/// records which keyword the source used so rendering round-trips exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum RegexpSpelling {
    /// The `RLIKE` keyword.
    Rlike,
    /// The `REGEXP` keyword.
    Regexp,
}

/// Surface spelling for the pattern-match predicate [`Expr::Like`].
///
/// SQL spells pattern matching three interchangeable-shape ways — case-sensitive
/// `LIKE` (SQL-92 core E021-08), PostgreSQL's case-insensitive `ILIKE`, and the
/// regex-flavoured `SIMILAR TO` (SQL:1999 F841). The canonical AST keeps one
/// [`Expr::Like`] node and this tag records which keyword the source
/// used so rendering round-trips exactly, mirroring [`RegexpSpelling`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum LikeSpelling {
    /// The `LIKE` keyword.
    Like,
    /// The `ILIKE` keyword (PostgreSQL).
    ILike,
    /// The `SIMILAR TO` keyword pair (SQL:1999 F841).
    SimilarTo,
}

/// Surface spelling for the null test [`Expr::IsNull`].
///
/// SQL's `<expr> IS [NOT] NULL` has one-word postfix synonyms in PostgreSQL and SQLite —
/// `<expr> ISNULL` (for `IS NULL`) and `<expr> NOTNULL` (for `IS NOT NULL`). The canonical
/// AST keeps one [`Expr::IsNull`] node with its `negated` flag and this tag records which
/// keyword form the source used so rendering round-trips, mirroring [`LikeSpelling`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum NullTestSpelling {
    /// The standard two/three-word form `IS NULL` / `IS NOT NULL`.
    Is,
    /// The one-word postfix synonym `ISNULL` (for `IS NULL`) / `NOTNULL` (for `IS NOT NULL`)
    /// — PostgreSQL and SQLite.
    Postfix,
    /// The two-word postfix synonym `<expr> NOT NULL` (for `IS NOT NULL`) — a `NOT`-led
    /// predicate spelling distinct from the one-word [`Postfix`](Self::Postfix) `NOTNULL`.
    /// Only ever paired with `negated: true` (there is no un-negated two-word form).
    /// SQLite and DuckDB accept it; PostgreSQL, despite the one-word synonyms, rejects it
    /// (engine-measured), so it rides its own
    /// [`PredicateSyntax::null_test_two_word_postfix`](crate::dialect::PredicateSyntax) gate
    /// rather than [`OperatorSyntax::null_test_postfix`](crate::dialect::OperatorSyntax).
    PostfixNotNull,
}

/// Surface spelling for the [`StringFunc::CeilTo`] head word — `CEIL` vs its `CEILING`
/// synonym, recorded so rendering round-trips the word the source wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CeilSpelling {
    /// Source used the `CEIL` spelling.
    Ceil,
    /// Source used the `CEILING` spelling.
    Ceiling,
}

/// The Unicode normal form named in an `<expr> IS [NOT] <form> NORMALIZED` test
/// ([`Expr::IsNormalized`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum NormalizationForm {
    /// Normalization Form C (canonical composition) — the default when `IS NORMALIZED`
    /// names no form.
    Nfc,
    /// Normalization Form D (canonical decomposition).
    Nfd,
    /// Normalization Form KC (compatibility composition).
    Nfkc,
    /// Normalization Form KD (compatibility decomposition).
    Nfkd,
}

/// Unary expression operators.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum UnaryOperator {
    /// `NOT` — logical negation.
    Not,
    /// `-` — arithmetic negation.
    Minus,
    /// `+` — unary plus (identity).
    Plus,
    /// Bitwise complement, spelled `~` (PostgreSQL, MySQL, SQLite, DuckDB). A prefix
    /// operator whose binding power is dialect data
    /// ([`prefix_bitwise_not`](crate::precedence::BindingPowerTable::prefix_bitwise_not)):
    /// it binds tightly (like the unary sign) in SQLite/MySQL, but in PostgreSQL/DuckDB it
    /// is looser than the arithmetic operators and tighter than the binary bitwise family
    /// (`~ 1 + 1` is `~ (1 + 1)` but `~ 1 & 3` is `(~ 1) & 3` — engine-measured).
    BitwiseNot,
    /// Oracle/Snowflake `PRIOR <expr>` — inside a `CONNECT BY` condition, marks the
    /// operand taken from the *parent* row of the hierarchical walk (Oracle *SQL
    /// Language Reference*, hierarchical queries; Snowflake `CONNECT BY`). It is an
    /// expression-level operator meaningful only in that clause: the parser produces it
    /// solely while parsing a [`Select::connect_by`](crate::ast::Select) condition
    /// (gated by [`SelectSyntax::connect_by_clause`](crate::dialect::SelectSyntax)), so
    /// the global expression grammar admits no bare `PRIOR` and a `prior` elsewhere
    /// stays an ordinary column name. Binds like the unary sign (tighter than
    /// comparison), so `PRIOR a = b` groups as `(PRIOR a) = b`.
    Prior,
}
