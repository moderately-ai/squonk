// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! MySQL stored-program (SQL/PSM) compound-statement AST nodes.
//!
//! The body sub-language of a routine/trigger/event: a `[label:] BEGIN … END
//! [label]` compound block carrying a strict [`Declaration`] prefix (variables,
//! conditions, cursors, handlers) then a statement list, plus the flow-control
//! statements (`IF`/`CASE`/`LOOP`/`WHILE`/`REPEAT`/`LEAVE`/`ITERATE`/`RETURN`) and
//! the cursor operations (`OPEN`/`FETCH`/`CLOSE`). These are body-context-only
//! [`Statement`] variants: the parser reaches them through the separate
//! `parse_body_statement` dispatcher, never the top-level one (a bare top-level
//! `BEGIN … END` is a transaction, not a compound block).
//!
//! Each nesting node's payload is boxed into its [`Statement`] variant (the
//! `CreateTrigger` precedent) so the enum stays within its 24-byte size budget; the
//! bodies are `ThinVec<Statement<X>>`, reusing the existing statement nodes exactly
//! as the trigger body does.

use super::{DataType, Expr, Extension, Ident, Literal, NoExt, Query, Statement};
use crate::vocab::Meta;
use thin_vec::ThinVec;

/// A `[<label>:] BEGIN [<declarations>] [<statements>] END [<label>]` compound block.
///
/// The declaration prefix is parse-time strict-ordered ({variables, conditions} →
/// cursors → handlers); the [`body`](Self::body) is the trailing statement list, in
/// source order, each element a full [`Statement`]. Both the opening `label:` and the
/// closing `END <label>` are optional and independent surface tags — a block may carry
/// either, both, or neither; when both are present they must match (a structural check
/// the parser enforces).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CompoundStatement<X: Extension = NoExt> {
    /// The opening block label (`<label>:` before `BEGIN`); `None` when unlabelled.
    pub label: Option<Ident>,
    /// The `DECLARE` prefix, in source order (always ahead of [`body`](Self::body)).
    pub declarations: ThinVec<Declaration<X>>,
    /// The block's statement list, in source order.
    pub body: ThinVec<Statement<X>>,
    /// The closing block label (`END <label>`); `None` when the close is bare `END`.
    pub end_label: Option<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `DECLARE …` item in a [`CompoundStatement`] prefix.
///
/// The four forms are parsed uniformly into this prefix; their *ordering* is enforced
/// post-hoc by the parser's declaration 4-counter ({[`Variable`](Self::Variable),
/// [`Condition`](Self::Condition)} → [`Cursor`](Self::Cursor) →
/// [`Handler`](Self::Handler)), mirroring the server's own mechanism rather than
/// encoding the order as grammar productions.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum Declaration<X: Extension = NoExt> {
    /// `DECLARE <name> [, <name> …] <type> [DEFAULT <expr>]` — one or more local
    /// variables sharing a type and optional default.
    Variable {
        /// The declared variable names, in source order (always at least one).
        names: ThinVec<Ident>,
        /// The shared declared type.
        data_type: DataType<X>,
        /// The optional `DEFAULT <expr>` initializer.
        default: Option<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DECLARE <name> CONDITION FOR <condition-value>` — a named condition alias.
    Condition {
        /// The condition name.
        name: Ident,
        /// The `SQLSTATE '…'` or MySQL error-code value the name aliases.
        value: ConditionValue,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DECLARE <name> CURSOR FOR <select>` — a cursor over a query.
    Cursor {
        /// The cursor name.
        name: Ident,
        /// The `SELECT` the cursor iterates; boxed to keep the variant small.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DECLARE {CONTINUE | EXIT | UNDO} HANDLER FOR <condition> [, …] <statement>` — a
    /// condition handler whose body runs when any listed condition is raised.
    Handler {
        /// The handler action (`CONTINUE`/`EXIT`/`UNDO`).
        action: HandlerAction,
        /// The condition values the handler catches, in source order (at least one).
        conditions: ThinVec<HandlerCondition>,
        /// The handler body — a single body statement (often a compound block); boxed
        /// to keep the variant small.
        body: Box<Statement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The action of a `DECLARE … HANDLER` — what happens after the handler body runs.
///
/// A tag (no `meta`): the keyword's span is subsumed by the enclosing
/// [`Declaration::Handler`], exactly as [`super::TriggerTiming`] rides its parent's span.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum HandlerAction {
    /// `CONTINUE` — resume execution after the statement that raised the condition.
    Continue,
    /// `EXIT` — leave the compound block the handler is declared in.
    Exit,
    /// `UNDO` — roll back and leave (parsed for completeness; the server restricts it).
    Undo,
}

/// A condition value across the shared condition family: a `SQLSTATE` string, a MySQL
/// error code, or a declared condition name.
///
/// The variant SET each parser produces is the grammar's subset for its site, not the whole
/// enum: `DECLARE … CONDITION FOR` produces [`SqlState`](Self::SqlState) or
/// [`ErrorCode`](Self::ErrorCode) (never a name — that form is a syntax error there), while
/// `SIGNAL`/`RESIGNAL` produce [`SqlState`](Self::SqlState) or
/// [`ConditionName`](Self::ConditionName) (a bare error code is a *syntax* error for
/// `SIGNAL`, engine-measured `1064`). The type is the union so the two siblings share one
/// vocabulary rather than minting parallel `SqlState` shapes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ConditionValue {
    /// `SQLSTATE [VALUE] '<sqlstate>'` — a five-character SQLSTATE string constant.
    SqlState {
        /// Whether the optional `VALUE` noise keyword was written (fidelity tag).
        value_keyword: bool,
        /// The SQLSTATE string literal.
        sqlstate: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bare MySQL error-code integer (e.g. `1051`). `DECLARE … CONDITION FOR` only; a
    /// bare code is a syntax error in `SIGNAL`/`RESIGNAL`.
    ErrorCode {
        /// The error-code integer literal.
        code: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A condition name declared earlier in the block (`DECLARE … CONDITION`), signalled by
    /// `SIGNAL <name>` / `RESIGNAL <name>`. Grammar-valid at top level (the sp-context
    /// resolution restriction MySQL enforces — `1319` outside a stored program — is a
    /// semantic check this parser leaves to name resolution, not a syntax reject).
    ConditionName {
        /// The referenced condition name.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One condition in a `DECLARE … HANDLER FOR <condition> [, …]` list.
///
/// A superset of [`ConditionValue`]: a handler additionally catches a condition name
/// declared earlier in the block and the three general condition classes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum HandlerCondition {
    /// `SQLSTATE [VALUE] '<sqlstate>'` — a SQLSTATE string constant.
    SqlState {
        /// Whether the optional `VALUE` noise keyword was written (fidelity tag).
        value_keyword: bool,
        /// The SQLSTATE string literal.
        sqlstate: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bare MySQL error-code integer (e.g. `1051`).
    ErrorCode {
        /// The error-code integer literal.
        code: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A condition name declared earlier in the block (`DECLARE … CONDITION`).
    ConditionName {
        /// The referenced condition name.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SQLWARNING` — the class of `SQLSTATE` values beginning `01`.
    SqlWarning {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `NOT FOUND` — the class of `SQLSTATE` values beginning `02` (cursor exhaustion).
    NotFound {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SQLEXCEPTION` — every remaining `SQLSTATE` class.
    SqlException {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// An `IF <cond> THEN … [ELSEIF <cond> THEN …] … [ELSE …] END IF` statement.
///
/// The `IF` and each `ELSEIF` are folded into one ordered [`branches`](Self::branches)
/// list (the first is the `IF`, the rest the `ELSEIF`s); the optional trailing `ELSE`
/// body is [`else_body`](Self::else_body).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct IfStatement<X: Extension = NoExt> {
    /// The `IF`/`ELSEIF` branches, in source order (always at least the `IF`).
    pub branches: ThinVec<ConditionalBranch<X>>,
    /// The optional `ELSE` body; `None` when no `ELSE` clause was written.
    pub else_body: Option<ThinVec<Statement<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `<cond> THEN <statements>` arm of an [`IfStatement`] or the WHEN arm of a
/// searched [`CaseStatement`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ConditionalBranch<X: Extension = NoExt> {
    /// The branch guard — the `IF`/`ELSEIF`/`WHEN` expression.
    pub guard: Expr<X>,
    /// The branch body, in source order.
    pub body: ThinVec<Statement<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `CASE … END CASE` statement, in either the simple or searched form.
///
/// [`operand`](Self::operand) distinguishes them: `Some` is the simple `CASE <operand>
/// WHEN <value> …` (each branch guard is a value compared to the operand); `None` is
/// the searched `CASE WHEN <condition> …` (each branch guard is a boolean predicate).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CaseStatement<X: Extension = NoExt> {
    /// The simple-`CASE` operand; `None` for the searched form. Boxed to keep the
    /// node small.
    pub operand: Option<Box<Expr<X>>>,
    /// The `WHEN … THEN …` branches, in source order (always at least one).
    pub when_branches: ThinVec<ConditionalBranch<X>>,
    /// The optional `ELSE` body; `None` when no `ELSE` clause was written.
    pub else_body: Option<ThinVec<Statement<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `[<label>:] LOOP … END LOOP [<label>]` unconditional loop.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LoopStatement<X: Extension = NoExt> {
    /// The opening loop label; `None` when unlabelled.
    pub label: Option<Ident>,
    /// The loop body, in source order.
    pub body: ThinVec<Statement<X>>,
    /// The closing `END LOOP <label>`; `None` when the close is bare.
    pub end_label: Option<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `[<label>:] WHILE <cond> DO … END WHILE [<label>]` pre-tested loop.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct WhileStatement<X: Extension = NoExt> {
    /// The opening loop label; `None` when unlabelled.
    pub label: Option<Ident>,
    /// The `WHILE` continuation condition; boxed to keep the node small.
    pub condition: Box<Expr<X>>,
    /// The loop body, in source order.
    pub body: ThinVec<Statement<X>>,
    /// The closing `END WHILE <label>`; `None` when the close is bare.
    pub end_label: Option<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `[<label>:] REPEAT … UNTIL <cond> END REPEAT [<label>]` post-tested loop.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct RepeatStatement<X: Extension = NoExt> {
    /// The opening loop label; `None` when unlabelled.
    pub label: Option<Ident>,
    /// The loop body, in source order.
    pub body: ThinVec<Statement<X>>,
    /// The `UNTIL` termination condition (tested after each pass); boxed to keep the
    /// node small.
    pub condition: Box<Expr<X>>,
    /// The closing `END REPEAT <label>`; `None` when the close is bare.
    pub end_label: Option<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `LEAVE <label>` statement — leave the labelled block or loop.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct LeaveStatement {
    /// The target label.
    pub label: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// An `ITERATE <label>` statement — restart the labelled loop.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct IterateStatement {
    /// The target loop label.
    pub label: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `RETURN <expr>` statement — return a value from a stored function.
///
/// Function-only in the server (`RETURN` in a procedure is rejected); the node lands
/// here and the routine family enforces the context.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ReturnStatement<X: Extension = NoExt> {
    /// The returned value expression.
    pub value: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// An `OPEN <cursor>` statement — open a declared cursor.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct OpenCursorStatement {
    /// The cursor name.
    pub cursor: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `CLOSE <cursor>` statement — close an open cursor.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CloseCursorStatement {
    /// The cursor name.
    pub cursor: Ident,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `FETCH [[NEXT] FROM] <cursor> INTO <var> [, …]` statement — fetch the next row
/// into local variables.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct FetchCursorStatement {
    /// Whether the optional `NEXT` noise keyword was written (fidelity tag; `NEXT`
    /// implies `FROM`).
    pub next_keyword: bool,
    /// Whether the optional `FROM` noise keyword was written (fidelity tag).
    pub from_keyword: bool,
    /// The cursor name.
    pub cursor: Ident,
    /// The `INTO` target variables, in source order (always at least one).
    pub targets: ThinVec<Ident>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `SIGNAL`/`RESIGNAL` statement: raise (or re-raise) a condition, optionally amending
/// the diagnostics area with a `SET` item list.
///
/// One payload for both keywords (the enclosing [`Statement::Signal`]/[`Statement::Resignal`]
/// records which): they share the grammar bar one difference the parser enforces — `SIGNAL`
/// requires a [`condition`](Self::condition), `RESIGNAL` leaves it optional (re-raise the
/// current condition, so a bare `RESIGNAL` and `RESIGNAL SET …` are both legal).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct SignalStatement<X: Extension = NoExt> {
    /// The signalled condition — a `SQLSTATE '…'` or a declared condition name. Mandatory
    /// for `SIGNAL` (parser-enforced), optional for `RESIGNAL`.
    pub condition: Option<ConditionValue>,
    /// The `SET <item> = <expr> [, …]` amendments; empty when no `SET` clause was written.
    pub set_items: ThinVec<SignalItem<X>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `<name> = <expr>` amendment in a `SIGNAL`/`RESIGNAL` `SET` list.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct SignalItem<X: Extension = NoExt> {
    /// Which diagnostics-area field is being set.
    pub name: SignalItemName,
    /// The value expression assigned to the field.
    pub value: Expr<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The settable condition-information item name in a `SIGNAL`/`RESIGNAL` `SET` list.
///
/// A closed keyword set (a tag, no `meta` — the span is the enclosing [`SignalItem`]'s). The
/// signal-settable subset deliberately EXCLUDES `RETURNED_SQLSTATE` (readable via
/// `GET DIAGNOSTICS` but not signal-settable), so it is a distinct enum from
/// [`ConditionInfoItemName`], not a shared one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SignalItemName {
    /// `CLASS_ORIGIN`.
    ClassOrigin,
    /// `SUBCLASS_ORIGIN`.
    SubclassOrigin,
    /// `CONSTRAINT_CATALOG`.
    ConstraintCatalog,
    /// `CONSTRAINT_SCHEMA`.
    ConstraintSchema,
    /// `CONSTRAINT_NAME`.
    ConstraintName,
    /// `CATALOG_NAME`.
    CatalogName,
    /// `SCHEMA_NAME`.
    SchemaName,
    /// `TABLE_NAME`.
    TableName,
    /// `COLUMN_NAME`.
    ColumnName,
    /// `CURSOR_NAME`.
    CursorName,
    /// `MESSAGE_TEXT`.
    MessageText,
    /// `MYSQL_ERRNO`.
    MysqlErrno,
}

/// A `GET [CURRENT | STACKED] DIAGNOSTICS …` statement — read the diagnostics area into
/// target variables.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct GetDiagnosticsStatement<X: Extension = NoExt> {
    /// Which diagnostics area (the `CURRENT`/`STACKED` keyword, or none — implicit current).
    pub area: DiagnosticsArea,
    /// The requested information: statement-level items or a single condition's items.
    pub info: DiagnosticsInfo<X>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `which_area` written before `DIAGNOSTICS`. A tag: `None` and `Current` name the same
/// area and differ only in spelling (a fidelity distinction so both round-trip).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DiagnosticsArea {
    /// No area keyword — the implicit current area.
    Implicit,
    /// Explicit `CURRENT`.
    Current,
    /// Explicit `STACKED`.
    Stacked,
}

/// The information a `GET DIAGNOSTICS` requests: statement-level items, or a single
/// condition's items keyed by a `CONDITION <number>` selector.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DiagnosticsInfo<X: Extension = NoExt> {
    /// `<target> = {NUMBER | ROW_COUNT} [, …]` — the statement-level diagnostics items.
    Statement {
        /// The requested statement items, in source order (always at least one).
        items: ThinVec<StatementInfoItem<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CONDITION <number> <target> = <cond-item> [, …]` — one condition's items.
    Condition {
        /// The `CONDITION <number>` selector expression (a limited expr subset).
        number: Box<Expr<X>>,
        /// The requested condition items, in source order (always at least one).
        items: ThinVec<ConditionInfoItem<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One `<target> = {NUMBER | ROW_COUNT}` item in a `GET DIAGNOSTICS` statement-information
/// list.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct StatementInfoItem<X: Extension = NoExt> {
    /// The target lvalue receiving the value (a `@user` variable or a local variable name).
    pub target: Expr<X>,
    /// Which statement-level field is read.
    pub name: StatementInfoItemName,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The readable statement-level diagnostics item name (a tag, no `meta`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum StatementInfoItemName {
    /// `NUMBER` — the count of conditions in the diagnostics area.
    Number,
    /// `ROW_COUNT` — the affected-row count of the previous statement.
    RowCount,
}

/// One `<target> = <cond-item>` item in a `GET DIAGNOSTICS CONDITION` list.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct ConditionInfoItem<X: Extension = NoExt> {
    /// The target lvalue receiving the value (a `@user` variable or a local variable name).
    pub target: Expr<X>,
    /// Which condition-level field is read.
    pub name: ConditionInfoItemName,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The readable condition-level diagnostics item name (a tag, no `meta`).
///
/// A superset of [`SignalItemName`]: every signal-settable field plus `RETURNED_SQLSTATE`,
/// which is readable here but not signal-settable — the two lists are intentionally
/// asymmetric, so they are distinct enums.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ConditionInfoItemName {
    /// `CLASS_ORIGIN`.
    ClassOrigin,
    /// `SUBCLASS_ORIGIN`.
    SubclassOrigin,
    /// `CONSTRAINT_CATALOG`.
    ConstraintCatalog,
    /// `CONSTRAINT_SCHEMA`.
    ConstraintSchema,
    /// `CONSTRAINT_NAME`.
    ConstraintName,
    /// `CATALOG_NAME`.
    CatalogName,
    /// `SCHEMA_NAME`.
    SchemaName,
    /// `TABLE_NAME`.
    TableName,
    /// `COLUMN_NAME`.
    ColumnName,
    /// `CURSOR_NAME`.
    CursorName,
    /// `MESSAGE_TEXT`.
    MessageText,
    /// `MYSQL_ERRNO`.
    MysqlErrno,
    /// `RETURNED_SQLSTATE` — readable only (not signal-settable).
    ReturnedSqlstate,
}
