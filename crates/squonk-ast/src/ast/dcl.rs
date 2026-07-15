// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Session-configuration and access-control statement AST nodes (ADR-0012 DCL family).

use super::{
    AccountName, DataType, DropBehavior, Expr, Extension, Ident, Literal, NoExt, ObjectName,
    ParameterKind, TransactionMode,
};
use crate::vocab::Meta;
use thin_vec::ThinVec;

/// A run-time configuration / session statement: `SET`, `RESET`, or `SHOW`.
///
/// PostgreSQL groups these as run-time configuration statements; they write, clear,
/// and read the same session parameters. One canonical shape per construct.
///
/// The generic-parameter form is `SET <name> {= | TO} <value>`; the special-cased
/// subforms whose grammar departs from it each keep their own canonical shape
/// rather than being forced through the generic one. `SET TRANSACTION`
/// is transaction control, not a session statement, and lives on
/// [`TransactionStatement`](super::TransactionStatement); the session-level
/// [`SET SESSION CHARACTERISTICS`](Self::SetSessionCharacteristics) is its
/// distinct session counterpart.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SessionStatement<X: Extension = NoExt> {
    /// A generic `SET <param> {= | TO} <value>` statement.
    Set {
        /// Scope in which this syntax applies.
        scope: Option<SetScope>,
        /// Name referenced by this syntax.
        name: ObjectName,
        /// Which of the exact-synonym `=` / `TO` separators the source wrote. A
        /// source-fidelity render replays it; a target re-spell and the redacted
        /// fingerprint normalize to the canonical `TO`.
        assignment: SetAssignment,
        /// Value supplied by this syntax.
        value: SetValue,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET [SESSION | LOCAL] TIME ZONE { <value> | LOCAL | DEFAULT }`.
    ///
    /// A two-word parameter taken with no `=`/`TO` separator; the `LOCAL`/`DEFAULT`
    /// sentinels are kept distinct from an ordinary value (see [`SpecialSetValue`]).
    /// The value is boxed to keep the common `SET`/`RESET`/`SHOW` shapes lean.
    SetTimeZone {
        /// Scope in which this syntax applies.
        scope: Option<SetScope>,
        /// Value supplied by this syntax.
        value: Box<SpecialSetValue>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `SET ROLE` statement.
    SetRole {
        /// Scope in which this syntax applies.
        scope: Option<SetScope>,
        /// The target role (`SET ROLE <role>` / `NONE`); see [`SpecialSetValue`].
        role: Box<SpecialSetValue>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `SET SESSION AUTHORIZATION` statement.
    SetSessionAuthorization {
        /// Scope in which this syntax applies.
        scope: Option<SetScope>,
        /// The target authorization (`SET SESSION AUTHORIZATION <user>` / `DEFAULT`); see [`SpecialSetValue`].
        user: Box<SpecialSetValue>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET CONSTRAINTS { ALL | <name> [, ...] } { DEFERRED | IMMEDIATE }`.
    ///
    /// Sets the check timing of the current transaction's deferrable constraints;
    /// it is its own statement in the grammar and takes no `SESSION`/`LOCAL` scope.
    SetConstraints {
        /// Which constraints (`ALL` or a name list); see [`ConstraintsTarget`].
        constraints: ConstraintsTarget,
        /// The new check timing (`DEFERRED`/`IMMEDIATE`); see [`ConstraintCheckTime`].
        check_time: ConstraintCheckTime,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET NAMES { <charset> [COLLATE <collation>] | DEFAULT }` (MySQL): set the
    /// client connection character set. The value is boxed to keep the common
    /// `SET`/`RESET`/`SHOW` shapes lean.
    SetNames {
        /// Value supplied by this syntax.
        value: Box<SetNamesValue>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET SESSION CHARACTERISTICS AS TRANSACTION <mode> [, ...]`.
    ///
    /// Sets the default characteristics for subsequent transactions in the
    /// session, distinct from the per-transaction
    /// [`SET TRANSACTION`](super::TransactionStatement::SetCharacteristics) — which
    /// is why it is a session statement reusing the shared [`TransactionMode`].
    SetSessionCharacteristics {
        /// modes in source order.
        modes: ThinVec<TransactionMode>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RESET [SESSION | LOCAL | GLOBAL] <name>` / `RESET ALL`.
    ///
    /// The optional scope is a DuckDB extension (gated by
    /// [`UtilitySyntax::reset_scope`](crate::dialect::UtilitySyntax)); PostgreSQL's
    /// `RESET` takes no scope qualifier (`RESET SESSION x` is a PG parser error), so
    /// [`scope`](SessionStatement::Reset::scope) is `None` for every non-DuckDB dialect.
    Reset {
        /// Scope in which this syntax applies.
        scope: Option<SetScope>,
        /// Object targeted by this syntax.
        target: ConfigParameter,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SHOW { ALL | <name> } [VERBOSE]`.
    ///
    /// The `VERBOSE` tail is the sqlparser-rs/DataFusion planner spelling on `SHOW ALL`
    /// / `SHOW <setting>`; no shipped oracle accepts it (`pg_query` and DuckDB both
    /// reject `SHOW ALL VERBOSE` and `SHOW <setting> VERBOSE`), so it is admitted only
    /// under the permissive superset via
    /// [`ShowSyntax::show_verbose`](crate::dialect::UtilitySyntax) and carried here as
    /// data — [`verbose`](SessionStatement::Show::verbose) is `false` for every form the
    /// oracle-backed dialects parse.
    Show {
        /// Object targeted by this syntax.
        target: ConfigParameter,
        /// Whether the verbose form was present in the source.
        verbose: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The MySQL `SET` variable-assignment statement: a comma-separated list of
    /// heterogeneous assignments, each a system variable (with optional scope) or a
    /// user-defined `@var`.
    ///
    /// Distinct from the generic single-target [`Set`](Self::Set): MySQL assigns a
    /// *list* (`SET @a = 1, GLOBAL x = 2, SESSION y = 3`), each item carrying its own
    /// scope and a **full-expression** value — where PostgreSQL's `SET` takes one name
    /// and a restricted literal/bareword value list. Gated by
    /// [`SessionVariableSyntax::variable_assignment`](crate::dialect::SessionVariableSyntax).
    SetVariables {
        /// The assignments in source order (at least one).
        assignments: ThinVec<SetVariableAssignment<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL `SET { CHARACTER SET | CHARSET } { <charset> | DEFAULT }`: set the client
    /// connection character set (and reset the collation to that charset's default).
    ///
    /// A sibling of [`SetNames`](Self::SetNames) — the same client-charset surface with
    /// a narrower value (no `COLLATE`) and its own leading keyword spelling. Gated by
    /// [`SessionVariableSyntax::variable_assignment`](crate::dialect::SessionVariableSyntax).
    SetCharacterSet {
        /// Which of the exact-synonym `CHARACTER SET` / `CHARSET` spellings the source wrote.
        keyword: CharacterSetKeyword,
        /// Value supplied by this syntax. Boxed to keep the common `SET`/`RESET`/`SHOW`
        /// shapes lean (mirroring [`SetNames`](Self::SetNames)).
        value: Box<SetCharacterSetValue>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// The MySQL `SET RESOURCE GROUP <name> [FOR <thread_id> [, ...]]` statement
    /// (`sql_yacc.yy` `set_resource_group_stmt`): assign a thread (or the current session)
    /// to a resource group.
    ///
    /// It shares the `SET` statement head with the variable-assignment forms above but is a
    /// distinct grammar (`SQLCOM_SET_RESOURCE_GROUP`), not a variable assignment — so it is
    /// dispatched off the `RESOURCE GROUP` two-word lookahead in the `SET` parser and carried
    /// here rather than in [`SetVariables`](Self::SetVariables). It is the resource-group
    /// family's session-statement member; its `CREATE`/`ALTER`/`DROP` siblings are top-level
    /// [`Statement`](super::Statement) DDL. Gated by
    /// [`StatementDdlGates::resource_group`](crate::dialect::StatementDdlGates::resource_group).
    SetResourceGroup {
        /// The resource-group name (`ident`).
        name: Ident,
        /// The `FOR <thread_id> [, ...]` thread-id list (`thread_id_list`), or `None` for the
        /// bare form that binds the current session's thread. Non-empty when present; each id
        /// is a `real_ulong_num` unsigned-integer literal. The `opt_comma` separator (comma or
        /// whitespace both parse) is normalized to `, ` on render.
        thread_ids: Option<ThinVec<Literal>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One item of a MySQL [`SetVariables`](SessionStatement::SetVariables) list: a system
/// variable assignment (with an optional scope on either the keyword or `@@` axis) or a
/// user-defined `@var` assignment.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SetVariableAssignment<X: Extension = NoExt> {
    /// A system-variable assignment: `[GLOBAL|SESSION|LOCAL|PERSIST|PERSIST_ONLY] <name>`
    /// or `@@[scope.]<name>`, then `{= | :=}` and a [`set_expr_or_default`](SetVariableValue)
    /// value. The scope spelling (keyword prefix vs `@@` sigil) is captured in
    /// [`scope`](Self::SystemVariable::scope); the two are mutually exclusive in the
    /// grammar (`SET GLOBAL @@x = 1` is a syntax error).
    SystemVariable {
        /// The scope qualifier and its spelling; see [`SystemVariableScope`].
        scope: SystemVariableScope,
        /// The (optionally dotted) variable name.
        name: ObjectName,
        /// Which of the exact-synonym `=` / `:=` separators the source wrote.
        assignment: SetAssignment,
        /// The new value; see [`SetVariableValue`].
        value: SetVariableValue<X>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A user-defined `@var := <expr>` / `@var = <expr>` assignment. The value is a full
    /// expression (`@v = a + 1`, `@v = (SELECT ...)`); the sentinels a system variable
    /// admits (`DEFAULT`/`ON`/…) are *not* valid here, matching the grammar's plain `expr`.
    UserVariable {
        /// The user-variable name (sigil stripped), with its source quote style for round-trip.
        name: Ident,
        /// Which of the exact-synonym `=` / `:=` separators the source wrote.
        assignment: SetAssignment,
        /// The assigned expression.
        value: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The scope qualifier of a MySQL system-variable assignment, capturing both *which*
/// scope and *how* it was spelled — a keyword prefix (`GLOBAL x`) or the `@@` sigil
/// (`@@global.x`). Modelled as one enum so the mutually-exclusive keyword/`@@` spellings
/// cannot both be present.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SystemVariableScope {
    /// No scope prefix — a bare `SET x = ...` (server-implicit session scope).
    Implicit,
    /// A keyword prefix — `SET GLOBAL x = ...` / `SESSION` / `LOCAL` / `PERSIST` /
    /// `PERSIST_ONLY`.
    Keyword(SystemVariableScopeKind),
    /// The `@@` sigil with no explicit scope — `SET @@x = ...`.
    AtAt,
    /// The `@@` sigil with an explicit scope — `SET @@global.x = ...` / `@@session.` /
    /// `@@local.` / `@@persist.` / `@@persist_only.`.
    AtAtScoped(SystemVariableScopeKind),
}

/// Which system-variable scope a [`SystemVariableScope`] names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SystemVariableScopeKind {
    /// `GLOBAL` — the server-wide value.
    Global,
    /// `SESSION` — the current session's value.
    Session,
    /// `LOCAL` — a `SESSION` synonym, kept distinct for round-trip fidelity.
    Local,
    /// `PERSIST` — set the runtime value and persist it to `mysqld-auto.cnf`.
    Persist,
    /// `PERSIST_ONLY` — persist without changing the runtime value.
    PersistOnly,
}

/// The value of a MySQL system-variable assignment (the grammar's `set_expr_or_default`):
/// a general expression, the `DEFAULT` reset sentinel, or one of the keyword sentinels the
/// grammar special-cases into a string value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SetVariableValue<X: Extension = NoExt> {
    /// `DEFAULT` — reset the variable to its compiled/configured default.
    Default {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bareword keyword the SET grammar special-cases into a string value: `ON`, `ALL`,
    /// `BINARY`, `ROW`, or `SYSTEM`. (`OFF` is not among them — it lexes as an ordinary
    /// identifier expression.)
    Keyword {
        /// Which keyword sentinel; see [`SetVariableKeyword`].
        keyword: SetVariableKeyword,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A general value expression (`1`, `1 + 2`, `'utf8'`, `@x`, `(SELECT ...)`).
    Expr {
        /// The value expression.
        expr: Box<Expr<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A keyword the MySQL SET grammar special-cases as a string value in
/// [`set_expr_or_default`](SetVariableValue) position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SetVariableKeyword {
    /// `ON`.
    On,
    /// `ALL`.
    All,
    /// `BINARY`.
    Binary,
    /// `ROW`.
    Row,
    /// `SYSTEM`.
    System,
}

/// Which exact-synonym spelling opened a [`SetCharacterSet`](SessionStatement::SetCharacterSet):
/// the two-word `CHARACTER SET` or the one-word `CHARSET`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum CharacterSetKeyword {
    /// The two-word `CHARACTER SET` spelling.
    CharacterSet,
    /// The one-word `CHARSET` spelling.
    Charset,
}

/// The operand of a MySQL [`SetCharacterSet`](SessionStatement::SetCharacterSet): a client
/// character set (an identifier, string, or `binary`), or `DEFAULT`. Unlike
/// [`SetNamesValue`] it carries no `COLLATE` (the grammar admits none here).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SetCharacterSetValue {
    /// `DEFAULT`: the server's configured character set.
    Default {
        /// Source location and node identity.
        meta: Meta,
    },
    /// An explicit character set value.
    Charset {
        /// The client character set value.
        charset: SetParameterValue,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The scope qualifier on a `SET`/`RESET`: `SESSION` (the default), `LOCAL`, or
/// `GLOBAL`.
///
/// `SESSION`/`LOCAL` are the PostgreSQL `SET` scopes; `GLOBAL` is a DuckDB `RESET`
/// scope (`RESET GLOBAL x`), reached only through
/// [`SessionStatement::Reset`] under
/// [`UtilitySyntax::reset_scope`](crate::dialect::UtilitySyntax) — the PostgreSQL
/// `SET` grammar never yields it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SetScope {
    /// `SET SESSION` — apply for the whole session.
    Session,
    /// `SET LOCAL` — apply only until the current transaction ends.
    Local,
    /// `SET GLOBAL` — apply server-wide (MySQL).
    Global,
}

/// Surface spelling of the `SET <name> {= | TO} <value>` assignment separator
/// ([`SessionStatement::Set`]).
///
/// `=` and `TO` are exact synonyms in PostgreSQL's generic `SET`; the canonical AST
/// keeps one shape and this tag records which the source wrote so a source-fidelity
/// render replays it. A fidelity tag, not a validity one — a target re-spell and the
/// redacted fingerprint normalize to the canonical `TO`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SetAssignment {
    /// The `TO` keyword (the canonical spelling).
    To,
    /// The `=` operator.
    Equals,
    /// The `:=` operator — MySQL's `SET_VAR` assignment separator, an exact synonym of
    /// `=` in `SET` (and in `@v := expr`). Never appears in the PostgreSQL generic `SET`,
    /// which has no `:=`; a source-fidelity render replays it, a target re-spell and the
    /// redacted fingerprint normalize it to `=` (MySQL has no `TO` spelling here).
    ColonEquals,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL set value forms represented by the AST.
pub enum SetValue {
    /// `SET … = DEFAULT` — reset the parameter to its default value.
    Default {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET … = <value>[, …]` — an explicit value list.
    Values {
        /// Values in source order.
        values: ThinVec<SetParameterValue>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One value in a `SET` value list: a literal, a bareword name, a parameter, or a
/// DuckDB bracketed list of values.
///
/// PostgreSQL accepts numbers, strings, and barewords such as `on`/`off`/`iso`
/// here; arbitrary expressions are not valid, so the value is this restricted
/// literal-or-name shape rather than an [`Expr`]. A leading sign on a
/// numeric value (PG `NumericOnly`, e.g. `-1`) is folded into the numeric
/// [`Literal`]'s span rather than modelled as a unary operator.
/// PostgreSQL also admits a positional parameter such as `$1` as a `var_value`.
///
/// DuckDB additionally admits a bracketed list value ([`List`](Self::List)) —
/// `SET allowed_paths = ['a', 'b']`, `SET allowed_directories = []` — reusing the same
/// `[…]` collection-literal syntax DuckDB accepts in expression position (gated by the
/// same [`ExpressionSyntax::collection_literals`](crate::dialect::ExpressionSyntax), the
/// one dialect for which `[` opens a list rather than a quoted identifier). Its elements
/// are again this restricted value grammar (so nesting is expressible), *not* a general
/// [`Expr`]: DuckDB's parser accepts richer element expressions but rejects
/// them at bind, past this validator's parse-level contract.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SetParameterValue {
    /// A literal value (a number or string).
    Literal {
        /// The literal value; see [`Literal`].
        literal: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A bareword name value (`on`/`off`/`iso`/…).
    Name {
        /// Name referenced by this syntax.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A prepared-statement parameter accepted by the active dialect's parameter
    /// syntax, such as PostgreSQL's positional `$1`.
    Parameter {
        /// Placeholder identity and spelling.
        kind: ParameterKind,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB bracketed list value (`['a', 'b']`, `[]`). Empty when the list was
    /// `[]`; the elements are the same restricted value grammar, so a nested list is
    /// representable.
    List {
        /// Values in source order.
        values: ThinVec<SetParameterValue>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// The operand of a single-valued special `SET` (`TIME ZONE`, `ROLE`, `SESSION
/// AUTHORIZATION`): an explicit value or a reset-sentinel keyword.
///
/// Which sentinel is admissible is a per-form rule the parser enforces — `LOCAL`
/// and `DEFAULT` for `TIME ZONE`, `NONE` for `ROLE`, `DEFAULT` for `SESSION
/// AUTHORIZATION`. One shape keeps the near-identical forms uniform; a
/// sentinel a given form never accepts simply never appears for it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SpecialSetValue {
    /// `DEFAULT` — reset the parameter to its default value.
    Default {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `LOCAL` — the `SET TIME ZONE LOCAL` reset sentinel.
    Local {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `NONE` — the `SET ROLE NONE` reset sentinel.
    None {
        /// Source location and node identity.
        meta: Meta,
    },
    /// An explicit value.
    Value {
        /// Value supplied by this syntax.
        value: SetParameterValue,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL constraints target forms represented by the AST.
pub enum ConstraintsTarget {
    /// `SET CONSTRAINTS ALL …` — every deferrable constraint.
    All {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SET CONSTRAINTS <name>, … …` — the named constraints.
    Names {
        /// Names in source order.
        names: ThinVec<ObjectName>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL constraint check time forms represented by the AST.
pub enum ConstraintCheckTime {
    /// `DEFERRED` — check the constraints at transaction commit.
    Deferred,
    /// `IMMEDIATE` — check the constraints at the end of each statement.
    Immediate,
}

/// The operand of `SET NAMES` (MySQL): a client character set with an optional
/// collation, or `DEFAULT`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum SetNamesValue {
    /// `DEFAULT`: the server's configured character set.
    Default {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `<charset> [COLLATE <collation>]`.
    Charset {
        /// The client character set value.
        charset: SetParameterValue,
        /// Optional collation for this syntax.
        collation: Option<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL config parameter forms represented by the AST.
pub enum ConfigParameter {
    /// `ALL` — every configuration parameter (`SHOW ALL` / `RESET ALL`).
    All {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A single named parameter.
    Named {
        /// Name referenced by this syntax.
        name: ObjectName,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A PostgreSQL `ALTER SYSTEM { SET <name> {= | TO} <value> | RESET <name> | RESET ALL }`
/// statement (`AlterSystemStmt`), gated by
/// [`StatementDdlGates::alter_system`](crate::dialect::StatementDdlGates::alter_system).
///
/// `ALTER SYSTEM` writes and clears the *server-wide* persisted configuration (PostgreSQL's
/// `postgresql.auto.conf`), not a session parameter — but its grammar is a thin wrapper over
/// the very `generic_set` / `generic_reset` productions the session `SET` / `RESET` use, so
/// the setting-name / value axis is shared verbatim rather than re-minted. The `SET` form is a
/// dotted `var_name` plus the `=` / `TO` separator ([`SetAssignment`]) and a `var_list`-or-
/// `DEFAULT` value ([`SetValue`]); the `RESET` form is the `var_name`-or-`ALL` target
/// ([`ConfigParameter`]).
///
/// Unlike the session `SET`, `ALTER SYSTEM SET` admits no `SESSION` / `LOCAL` scope (a scope
/// keyword is a reject) and no `SET FROM CURRENT` form — the wrapper is exactly
/// `generic_set` / `generic_reset`, measured against `pg_query`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterSystem {
    /// The change applied to the server-wide configuration — a `SET` assignment or a `RESET`.
    pub action: AlterSystemAction,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The change an [`AlterSystem`] applies — PostgreSQL's `generic_set` / `generic_reset`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AlterSystemAction {
    /// `SET <name> {= | TO} { <value>[, …] | DEFAULT }` — persist a new server-wide value for
    /// the named parameter (PostgreSQL's `generic_set`). Reuses the session-`SET` value axis:
    /// [`name`](Self::Set::name) is the dotted `var_name`, [`assignment`](Self::Set::assignment)
    /// records the `=` / `TO` spelling, and [`value`](Self::Set::value) is the shared
    /// [`SetValue`] (a value list or `DEFAULT`).
    Set {
        /// The configuration parameter name — a dotted `var_name` (`work_mem`, `myapp.foo`).
        name: ObjectName,
        /// Which of the exact-synonym `=` / `TO` separators the source wrote (a fidelity tag).
        assignment: SetAssignment,
        /// The new value — a value list or `DEFAULT`; see [`SetValue`].
        value: SetValue,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `RESET { <name> | ALL }` — clear the persisted server-wide setting(s), restoring the
    /// compiled-in / `postgresql.conf` default (PostgreSQL's `generic_reset`). The target is
    /// the shared [`ConfigParameter`] (`ALL` or a named parameter).
    Reset {
        /// The parameter(s) to reset (`ALL` or a named parameter); see [`ConfigParameter`].
        target: ConfigParameter,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// An access-control (DCL) statement: `GRANT` or `REVOKE`.
///
/// Covers both branches of the SQL access-control grammar. A *privilege* grant
/// names privileges on a database object ([`Grant`](Self::Grant) /
/// [`Revoke`](Self::Revoke)); a *role-membership* grant confers membership in one
/// role on another ([`GrantRole`](Self::GrantRole) /
/// [`RevokeRole`](Self::RevokeRole)). The two share a leading comma list that only
/// `ON` (privilege grant) versus a bare `TO`/`FROM` (role grant) disambiguates —
/// exactly as PostgreSQL's grammar reuses one `privilege_list` production for both,
/// which is why `GRANT SELECT TO alice` is a role grant whose granted "role" merely
/// happens to be spelled like a privilege keyword.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AccessControlStatement<X: Extension = NoExt> {
    /// `ALTER ROLE <name> RENAME TO <new_name>`.
    AlterRoleRename {
        /// Existing role name.
        name: Ident,
        /// New role name.
        new_name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `GRANT <privileges> ON <object> TO <grantees>` statement.
    Grant {
        /// The privileges granted/revoked; see [`Privileges`].
        privileges: Privileges,
        /// The object the privileges apply to; see [`GrantObject`].
        object: GrantObject<X>,
        /// grantees in source order.
        grantees: ThinVec<Grantee>,
        /// Whether the with grant option form was present in the source.
        with_grant_option: bool,
        /// Optional granted by for this syntax.
        granted_by: Option<RoleSpec>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REVOKE [GRANT OPTION FOR] <privileges> ON <object> FROM <grantees> [GRANTED BY
    /// <grantor>] [CASCADE | RESTRICT]`.
    ///
    /// `behavior` carries the trailing `<drop behavior>` that governs whether
    /// dependent grants are revoked too; `GRANT` has no such clause, so it lives only
    /// on the two `REVOKE` variants.
    Revoke {
        /// Whether the grant option for form was present in the source.
        grant_option_for: bool,
        /// The privileges granted/revoked; see [`Privileges`].
        privileges: Privileges,
        /// The object the privileges apply to; see [`GrantObject`].
        object: GrantObject<X>,
        /// grantees in source order.
        grantees: ThinVec<Grantee>,
        /// Optional granted by for this syntax.
        granted_by: Option<RoleSpec>,
        /// Optional behavior for this syntax.
        behavior: Option<DropBehavior>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `GRANT <role> TO <grantees>` role-membership statement.
    GrantRole {
        /// roles in source order.
        roles: ThinVec<Ident>,
        /// grantees in source order.
        grantees: ThinVec<Grantee>,
        /// Whether the with admin option form was present in the source.
        with_admin_option: bool,
        /// Optional granted by for this syntax.
        granted_by: Option<RoleSpec>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REVOKE [ADMIN OPTION FOR] <roles> FROM <grantees> [GRANTED BY <grantor>]
    /// [CASCADE | RESTRICT]`.
    RevokeRole {
        /// Whether the admin option for form was present in the source.
        admin_option_for: bool,
        /// roles in source order.
        roles: ThinVec<Ident>,
        /// grantees in source order.
        grantees: ThinVec<Grantee>,
        /// Optional granted by for this syntax.
        granted_by: Option<RoleSpec>,
        /// Optional behavior for this syntax.
        behavior: Option<DropBehavior>,
        /// Source location and node identity.
        meta: Meta,
    },
    // --- MySQL account-based access control ------------------------------------------------
    //
    // The MySQL `GRANT`/`REVOKE` grammar, gated by
    // [`AccessControlSyntax::access_control_account_grants`](crate::dialect::AccessControlSyntax).
    // It forks from the standard/PostgreSQL forms above on two axes that admit no shared node —
    // the object is a `priv_level` ([`PrivilegeLevelObject`], with `*`/`*.*`/`db.*` wildcards) rather
    // than a typed object with a name list, and every grantee/role is an [`AccountName`]
    // (`user@host` / `CURRENT_USER`) rather than a [`RoleSpec`]. The privilege list itself reuses
    // the shared [`Privileges`] axis. MySQL has no `GRANTED BY`, no `CASCADE`/`RESTRICT`, and no
    // `{GRANT | ADMIN} OPTION FOR` prefix (all engine-measured `ER_PARSE_ERROR` on 8.4.10); it
    // adds `PROXY` grants, the `AS <user> [WITH ROLE …]` grantor context, and the
    // `[IF EXISTS] … [IGNORE UNKNOWN USER]` `REVOKE` guards.
    /// `GRANT <priv> ON [TABLE | FUNCTION | PROCEDURE] <priv_level> TO <user> [, …]
    /// [WITH GRANT OPTION] [AS <user> [WITH ROLE …]]` — a MySQL object-privilege grant.
    AccountGrantPrivilege {
        /// The privileges granted; see [`Privileges`].
        privileges: Privileges,
        /// The `priv_level` object the privileges apply to; see [`PrivilegeLevelObject`].
        object: PrivilegeLevelObject,
        /// The grantee accounts, in source order (always non-empty).
        grantees: ThinVec<AccountName>,
        /// Whether the `WITH GRANT OPTION` trailer was written.
        with_grant_option: bool,
        /// The `AS <user> [WITH ROLE …]` grantor context; `None` when omitted. Boxed — it is a
        /// rare, wide clause (ADR-0007), kept off the common privilege-grant path.
        grant_as: Option<Box<GrantAs>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `GRANT PROXY ON <user> TO <user> [, …] [WITH GRANT OPTION]` — a MySQL proxy grant.
    AccountGrantProxy {
        /// The proxied account (`ON <user>`).
        proxy: AccountName,
        /// The grantee accounts, in source order (always non-empty).
        grantees: ThinVec<AccountName>,
        /// Whether the `WITH GRANT OPTION` trailer was written.
        with_grant_option: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `GRANT <role> [, …] TO <user> [, …] [WITH ADMIN OPTION]` — a MySQL role-membership grant.
    AccountGrantRole {
        /// The granted roles, in source order (always non-empty).
        roles: ThinVec<AccountName>,
        /// The grantee accounts, in source order (always non-empty).
        grantees: ThinVec<AccountName>,
        /// Whether the `WITH ADMIN OPTION` trailer was written.
        with_admin_option: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REVOKE [IF EXISTS] <priv> ON [TABLE | FUNCTION | PROCEDURE] <priv_level> FROM <user> [, …]
    /// [IGNORE UNKNOWN USER]` — a MySQL object-privilege revoke.
    AccountRevokePrivilege {
        /// Whether the `IF EXISTS` guard was written.
        if_exists: bool,
        /// The privileges revoked; see [`Privileges`].
        privileges: Privileges,
        /// The `priv_level` object; see [`PrivilegeLevelObject`].
        object: PrivilegeLevelObject,
        /// The grantee accounts, in source order (always non-empty).
        grantees: ThinVec<AccountName>,
        /// Whether the `IGNORE UNKNOWN USER` trailer was written.
        ignore_unknown_user: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REVOKE [IF EXISTS] ALL [PRIVILEGES], GRANT OPTION FROM <user> [, …] [IGNORE UNKNOWN USER]`
    /// — the MySQL "revoke everything" form, which takes no `ON` object.
    AccountRevokeAll {
        /// Whether the `IF EXISTS` guard was written.
        if_exists: bool,
        /// Whether the optional `PRIVILEGES` noise word followed `ALL` (fidelity only; the
        /// canonical render emits `ALL PRIVILEGES, GRANT OPTION`).
        privileges_keyword: bool,
        /// The grantee accounts, in source order (always non-empty).
        grantees: ThinVec<AccountName>,
        /// Whether the `IGNORE UNKNOWN USER` trailer was written.
        ignore_unknown_user: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REVOKE [IF EXISTS] PROXY ON <user> FROM <user> [, …] [IGNORE UNKNOWN USER]` — a MySQL
    /// proxy revoke.
    AccountRevokeProxy {
        /// Whether the `IF EXISTS` guard was written.
        if_exists: bool,
        /// The proxied account (`ON <user>`).
        proxy: AccountName,
        /// The grantee accounts, in source order (always non-empty).
        grantees: ThinVec<AccountName>,
        /// Whether the `IGNORE UNKNOWN USER` trailer was written.
        ignore_unknown_user: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REVOKE [IF EXISTS] <role> [, …] FROM <user> [, …] [IGNORE UNKNOWN USER]` — a MySQL
    /// role-membership revoke.
    AccountRevokeRole {
        /// Whether the `IF EXISTS` guard was written.
        if_exists: bool,
        /// The revoked roles, in source order (always non-empty).
        roles: ThinVec<AccountName>,
        /// The grantee accounts, in source order (always non-empty).
        grantees: ThinVec<AccountName>,
        /// Whether the `IGNORE UNKNOWN USER` trailer was written.
        ignore_unknown_user: bool,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL grant object: an object-type keyword and a `priv_level`.
///
/// `GRANT … ON [TABLE | FUNCTION | PROCEDURE] <priv_level>` — the optional object-type keyword
/// defaults to `TABLE`, and the `priv_level` is one of the four spellings in [`PrivilegeLevel`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct PrivilegeLevelObject {
    /// The `TABLE`/`FUNCTION`/`PROCEDURE` object type (`TABLE` is the default).
    pub object_type: PrivilegeObjectType,
    /// The privilege level (`*`, `*.*`, `db.*`, or a named object).
    pub level: PrivilegeLevel,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The object-type keyword of a MySQL grant object (`opt_acl_type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PrivilegeObjectType {
    /// `TABLE` — the default object type. [`explicit`](Self::Table::explicit) records whether the
    /// redundant `TABLE` keyword was written so a source-fidelity render replays it.
    Table {
        /// Whether the redundant `TABLE` object-type keyword was written.
        explicit: bool,
    },
    /// `FUNCTION`.
    Function,
    /// `PROCEDURE`.
    Procedure,
}

/// A MySQL `priv_level` — the object a privilege grant/revoke targets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PrivilegeLevel {
    /// `*.*` — global (every database).
    Global {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `*` — every object in the default database.
    CurrentDatabase {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `<db>.*` — every object in a named database.
    Database {
        /// The database name.
        database: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `<obj>` / `<db>.<obj>` — a specific named object.
    Object {
        /// The object name (one or two parts).
        name: ObjectName,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL `AS <user> [WITH ROLE …]` grantor-context clause on a privilege grant.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct GrantAs {
    /// The grantor account (`AS <user>`).
    pub user: AccountName,
    /// The `WITH ROLE …` role restriction; `None` for a bare `AS <user>`.
    pub with_role: Option<WithRoleSpec>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The `WITH ROLE …` restriction of a MySQL [`GrantAs`] clause.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum WithRoleSpec {
    /// `WITH ROLE <role> [, …]` — an explicit role list (always non-empty).
    Roles {
        /// The roles, in source order.
        roles: ThinVec<AccountName>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `WITH ROLE ALL [EXCEPT <role> [, …]]` — all roles, minus an optional exception list
    /// (empty [`except`](Self::All::except) is a bare `WITH ROLE ALL`).
    All {
        /// The `EXCEPT` exception roles, in source order; empty when no `EXCEPT` was written.
        except: ThinVec<AccountName>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `WITH ROLE NONE`.
    None {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `WITH ROLE DEFAULT`.
    Default {
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL privileges forms represented by the AST.
pub enum Privileges {
    /// `ALL [PRIVILEGES]`. The optional `PRIVILEGES` noise word is exact-synonym
    /// sugar; [`privileges_keyword`](Self::All::privileges_keyword) records whether it
    /// was written so a source-fidelity render replays it (the canonical render emits
    /// `ALL PRIVILEGES`).
    All {
        /// Whether the optional `PRIVILEGES` keyword was written (`ALL PRIVILEGES` vs a
        /// bare `ALL`). Fidelity only; the canonical render emits `PRIVILEGES`.
        privileges_keyword: bool,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An explicit privilege list (`SELECT, UPDATE (a, b), …`).
    List {
        /// privileges in source order.
        privileges: ThinVec<Privilege>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One privilege in a `GRANT`/`REVOKE` list, optionally column-scoped.
///
/// A built-in privilege word carries a structured [`PrivilegeKind`]; any other
/// identifier rides [`Other`](Self::Other). PostgreSQL accepts an arbitrary
/// identifier in privilege position and only rejects an unknown name at execution,
/// so the [`Other`](Self::Other) escape keeps parse-time acceptance aligned and
/// captures dialect/extension privileges the built-in set does not name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum Privilege {
    /// A built-in privilege keyword, e.g. `SELECT` or `UPDATE (a, b)`.
    Known {
        /// Which built-in privilege; see [`PrivilegeKind`].
        kind: PrivilegeKind,
        /// Column-level scope (`SELECT (a, b)`); empty for a table-wide privilege.
        columns: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A privilege named by an arbitrary identifier (a dialect/extension privilege).
    Other {
        /// Name referenced by this syntax.
        name: Ident,
        /// Column-level scope; empty for a table-wide privilege.
        columns: ThinVec<Ident>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A built-in privilege keyword.
///
/// A closed set of the SQL-standard table privileges (SQL:2016 §12.3) and the
/// common non-table privileges. Privileges outside this set ride
/// [`Privilege::Other`], so the enum stays a `Copy` classifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PrivilegeKind {
    /// `SELECT` — read rows from the object.
    Select,
    /// `INSERT` — add rows to the object.
    Insert,
    /// `UPDATE` — modify rows in the object.
    Update,
    /// `DELETE` — remove rows from the object.
    Delete,
    /// `TRUNCATE` — empty the table.
    Truncate,
    /// `REFERENCES` — create foreign keys referencing the object.
    References,
    /// `TRIGGER` — create triggers on the object.
    Trigger,
    /// `USAGE` — use a schema, sequence, type, or language.
    Usage,
    /// `EXECUTE` — call a function or procedure.
    Execute,
    /// `CREATE` — create objects within the database/schema.
    Create,
    /// `CONNECT` — connect to the database.
    Connect,
    /// `TEMPORARY`. The `TEMP` alias is preserved separately as [`Temp`](Self::Temp).
    Temporary,
    /// `TEMP`, the shorthand alias for `TEMPORARY`, kept distinct so it round-trips.
    Temp,
    /// `MAINTAIN` — run maintenance commands (`VACUUM`/`ANALYZE`/…, PostgreSQL 17+).
    Maintain,
    // --- MySQL static privileges ---------------------------------------------------------
    //
    // The MySQL `role_or_privilege` static-privilege keywords not already named above, each a
    // fixed keyword phrase the render reproduces verbatim. Recognized only by the MySQL
    // account-grant grammar
    // ([`AccessControlSyntax::access_control_account_grants`](crate::dialect::AccessControlSyntax)),
    // so the PostgreSQL privilege parser never yields them; MySQL dynamic privileges
    // (`BACKUP_ADMIN`, …) and role names ride [`Privilege::Other`] instead. `SELECT`/`INSERT`/
    // `UPDATE`/`DELETE`/`REFERENCES`/`USAGE`/`EXECUTE`/`CREATE`/`TRIGGER` reuse the shared
    // variants above.
    /// `INDEX`.
    Index,
    /// `ALTER`.
    Alter,
    /// `DROP`.
    Drop,
    /// `RELOAD`.
    Reload,
    /// `SHUTDOWN`.
    Shutdown,
    /// `PROCESS`.
    Process,
    /// `FILE`.
    File,
    /// `SUPER` (deprecated in MySQL, still grammar-valid).
    Super,
    /// `EVENT`.
    Event,
    /// `GRANT OPTION` — spelled as a privilege in a `REVOKE`/`GRANT` list (distinct from the
    /// `WITH GRANT OPTION` trailer).
    GrantOption,
    /// `SHOW DATABASES`.
    ShowDatabases,
    /// `CREATE TEMPORARY TABLES`.
    CreateTemporaryTables,
    /// `LOCK TABLES`.
    LockTables,
    /// `REPLICATION SLAVE`.
    ReplicationSlave,
    /// `REPLICATION CLIENT`.
    ReplicationClient,
    /// `CREATE VIEW`.
    CreateView,
    /// `SHOW VIEW`.
    ShowView,
    /// `CREATE ROUTINE`.
    CreateRoutine,
    /// `ALTER ROUTINE`.
    AlterRoutine,
    /// `CREATE USER`.
    CreateUser,
    /// `CREATE TABLESPACE`.
    CreateTablespace,
    /// `CREATE ROLE`.
    CreateRole,
    /// `DROP ROLE`.
    DropRole,
}

/// The object a privilege applies to.
///
/// `TABLE` is the default object type, so [`Table`](Self::Table) records whether the
/// redundant keyword was written; every other object type names its keyword
/// explicitly.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum GrantObject<X: Extension = NoExt> {
    /// A table (or the default object type when the `TABLE` keyword is omitted).
    Table {
        /// Whether the redundant `TABLE` object-type keyword was written.
        explicit: bool,
        /// Names in source order.
        names: ThinVec<ObjectName>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An object type whose target is a plain name list (`SEQUENCE <name> [, ...]`,
    /// `DATABASE …`, `SCHEMA …`, `DOMAIN …`, `TYPE …`, `LANGUAGE …`, `TABLESPACE …`,
    /// `FOREIGN DATA WRAPPER …`, `FOREIGN SERVER …`).
    Named {
        /// Which named object type; see [`NamedObjectKind`].
        kind: NamedObjectKind,
        /// Names in source order.
        names: ThinVec<ObjectName>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// Routine targets (`FUNCTION`/`PROCEDURE`/`ROUTINE <name>(<sig>)`).
    Routines {
        /// Which routine kind; see [`RoutineObjectKind`].
        kind: RoutineObjectKind,
        /// routines in source order.
        routines: ThinVec<RoutineSignature<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ALL {TABLES | SEQUENCES | FUNCTIONS | PROCEDURES | ROUTINES} IN SCHEMA <name> [, ...]`.
    AllInSchema {
        /// Which object class is granted schema-wide; see [`SchemaObjectKind`].
        kind: SchemaObjectKind,
        /// schemas in source order.
        schemas: ThinVec<ObjectName>,
        /// Source location and node identity.
        meta: Meta,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL named object kind forms represented by the AST.
pub enum NamedObjectKind {
    /// `SEQUENCE` object.
    Sequence,
    /// `DATABASE` object.
    Database,
    /// `SCHEMA` object.
    Schema,
    /// `DOMAIN` object.
    Domain,
    /// `TYPE` object.
    Type,
    /// `LANGUAGE` object.
    Language,
    /// `TABLESPACE` object.
    Tablespace,
    /// `FOREIGN DATA WRAPPER` object.
    ForeignDataWrapper,
    /// `FOREIGN SERVER` object.
    ForeignServer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL routine object kind forms represented by the AST.
pub enum RoutineObjectKind {
    /// `FUNCTION`.
    Function,
    /// `PROCEDURE`.
    Procedure,
    /// `ROUTINE` — a function or procedure.
    Routine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL schema object kind forms represented by the AST.
pub enum SchemaObjectKind {
    /// `ALL TABLES IN SCHEMA`.
    Tables,
    /// `ALL SEQUENCES IN SCHEMA`.
    Sequences,
    /// `ALL FUNCTIONS IN SCHEMA`.
    Functions,
    /// `ALL PROCEDURES IN SCHEMA`.
    Procedures,
    /// `ALL ROUTINES IN SCHEMA`.
    Routines,
}

/// A routine reference in a `FUNCTION`/`PROCEDURE`/`ROUTINE` grant: a name with an
/// optional argument-type signature.
///
/// `arg_types` is `None` when no parenthesized list was written (`FUNCTION foo`) and
/// `Some` — possibly empty — when it was (`FUNCTION foo()` / `FUNCTION foo(int, text)`),
/// preserving the distinction. Argument names and modes (`IN`/`OUT`/`VARIADIC`) are
/// out of scope; only the type list is modelled.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct RoutineSignature<X: Extension = NoExt> {
    /// Name referenced by this syntax.
    pub name: ObjectName,
    /// Optional arg types for this syntax.
    pub arg_types: Option<ThinVec<DataType<X>>>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A `GRANT`/`REVOKE` grantee: a role specification, optionally with the legacy
/// `GROUP` keyword prefix (`GROUP <role>`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct Grantee {
    /// Whether the PostgreSQL-legacy `GROUP` keyword preceded the role.
    pub group: bool,
    /// The role specification; see [`RoleSpec`].
    pub spec: RoleSpec,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A role specification: a named role, `PUBLIC`, or a session-role pseudo-role.
///
/// Shared by grantees and the `GRANTED BY` grantor.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum RoleSpec {
    /// `PUBLIC` — the implicit role every role belongs to.
    Public {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CURRENT_ROLE` — the current role.
    CurrentRole {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CURRENT_USER` — the current user.
    CurrentUser {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `SESSION_USER` — the session user.
    SessionUser {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A named role.
    Name {
        /// Name referenced by this syntax.
        name: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

// --- User / role administration DDL (MySQL) --------------------------------------------
//
// `CREATE`/`ALTER`/`DROP USER` and `CREATE`/`DROP ROLE` — the MySQL account-management
// family, gated by
// [`AccessControlSyntax::user_role_management`](crate::dialect::AccessControlSyntax). Every
// account reference rides the shared [`AccountName`] axis; the authentication, TLS, resource,
// and password/lock option lists below are shared by `CREATE USER` and `ALTER USER`.

/// `CREATE USER [IF NOT EXISTS] <user> [<auth>] [, …] [DEFAULT ROLE <role> [, …]]
/// [REQUIRE <tls>] [WITH <resource> …] [<password/lock option> …]
/// [COMMENT | ATTRIBUTE '<string>']` — the MySQL account-creation statement.
///
/// The `<user> [<auth>]` elements are a comma list of [`UserSpec`]s. The trailing clauses are
/// each written at most once (MySQL's grammar has them as single optional tails, not a
/// repeatable option bag), except the resource-limit and password/lock lists, which are
/// whitespace-separated repeatable runs.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct CreateUser {
    /// Whether the `IF NOT EXISTS` guard was written.
    pub if_not_exists: bool,
    /// The `<user> [<auth>]` account list, in source order (always non-empty).
    pub users: ThinVec<UserSpec>,
    /// The `DEFAULT ROLE <role> [, …]` roles, in source order; empty when the clause was not
    /// written (MySQL rejects an empty `DEFAULT ROLE`, so empty is unambiguously "absent").
    pub default_roles: ThinVec<AccountName>,
    /// The `REQUIRE …` TLS requirement; `None` when the clause was omitted.
    pub require: Option<TlsRequirement>,
    /// The `WITH <resource> …` resource limits, in source order; empty when no `WITH` was
    /// written.
    pub resource_options: ThinVec<ResourceLimit>,
    /// The password / account-lock options, in source order; empty when none were written.
    pub password_lock_options: ThinVec<PasswordLockOption>,
    /// The `COMMENT '…'` / `ATTRIBUTE '…'` account attribute; `None` when neither was written
    /// (the two are mutually exclusive in the grammar).
    pub attribute: Option<UserAttribute>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// One `<user> [<auth>]` element of a [`CreateUser`] list: an account name and its optional
/// primary-factor authentication.
///
/// The multi-factor `AND IDENTIFIED …` tail, the `INITIAL AUTHENTICATION` clause, and the
/// `ALTER USER` factor-registration surface are a separate MySQL multi-factor-authentication
/// family, deferred from this measured single-factor axis; this spec extends additively when
/// that family lands.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct UserSpec {
    /// The account being created.
    pub account: AccountName,
    /// The primary-factor authentication (`IDENTIFIED BY …` / `IDENTIFIED WITH …`); `None`
    /// for a bare account with no authentication clause.
    pub auth: Option<AuthOption>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// A primary-factor authentication clause: `IDENTIFIED BY …` / `IDENTIFIED WITH <plugin> …`.
///
/// The password and auth-string bodies are [`Literal`]s so the exact source spelling
/// round-trips from the span (and redacts to `?` under the redacted render). The plugin name
/// is a MySQL `ident_or_text`, folded to an [`Ident`] whose quote style round-trips.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AuthOption {
    /// `IDENTIFIED BY '<password>'`.
    Password {
        /// The cleartext password literal.
        password: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `IDENTIFIED BY RANDOM PASSWORD` — the server generates and returns a random password.
    RandomPassword {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `IDENTIFIED WITH <plugin>`.
    Plugin {
        /// The authentication-plugin name.
        plugin: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `IDENTIFIED WITH <plugin> AS '<auth string>'` — a pre-hashed authentication string.
    PluginAs {
        /// The authentication-plugin name.
        plugin: Ident,
        /// The plugin-specific authentication (hash) string literal.
        auth_string: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `IDENTIFIED WITH <plugin> BY '<password>'`.
    PluginByPassword {
        /// The authentication-plugin name.
        plugin: Ident,
        /// The cleartext password literal.
        password: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `IDENTIFIED WITH <plugin> BY RANDOM PASSWORD`.
    PluginByRandomPassword {
        /// The authentication-plugin name.
        plugin: Ident,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// `ALTER USER [IF EXISTS] …` — the MySQL account-modification statement.
///
/// Two shapes share the `[IF EXISTS]` guard: the [`Modify`](AlterUser::Modify) list form
/// (re-authenticate / re-configure a comma list of accounts) and the
/// [`DefaultRole`](AlterUser::DefaultRole) single-account `DEFAULT ROLE` reset. The
/// multi-factor factor-registration forms (`ADD`/`MODIFY`/`DROP FACTOR`, `… REGISTRATION`)
/// are a deferred multi-factor-authentication family (see [`UserSpec`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum AlterUser {
    /// `ALTER USER [IF EXISTS] <user> [<auth>] [REPLACE '…'] [RETAIN CURRENT PASSWORD |
    /// DISCARD OLD PASSWORD] [, …] [REQUIRE …] [WITH …] [<password/lock option> …]
    /// [COMMENT | ATTRIBUTE '…']`.
    Modify {
        /// Whether the `IF EXISTS` guard was written.
        if_exists: bool,
        /// The per-account modifications, in source order (always non-empty).
        users: ThinVec<AlterUserSpec>,
        /// The `REQUIRE …` TLS requirement; `None` when omitted.
        require: Option<TlsRequirement>,
        /// The `WITH <resource> …` resource limits, in source order; empty when no `WITH`.
        resource_options: ThinVec<ResourceLimit>,
        /// The password / account-lock options, in source order; empty when none.
        password_lock_options: ThinVec<PasswordLockOption>,
        /// The `COMMENT '…'` / `ATTRIBUTE '…'` account attribute; `None` when neither.
        attribute: Option<UserAttribute>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ALTER USER [IF EXISTS] <user> DEFAULT ROLE {ALL | NONE | <role> [, …]}`.
    DefaultRole {
        /// Whether the `IF EXISTS` guard was written.
        if_exists: bool,
        /// The account whose default roles are reset.
        user: AccountName,
        /// The default-role target — `ALL`, `NONE`, or an explicit role list.
        roles: DefaultRoleTarget,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One per-account element of an [`AlterUser::Modify`] list: an account, its optional new
/// authentication, and the password-rotation flags that attach to it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct AlterUserSpec {
    /// The account being altered.
    pub account: AccountName,
    /// The new primary-factor authentication; `None` for a bare account (e.g. a lone
    /// `DISCARD OLD PASSWORD` or a re-configuration with no re-authentication).
    pub auth: Option<AuthOption>,
    /// The `REPLACE '<current password>'` verification string; `None` when not written.
    pub replace: Option<Literal>,
    /// Whether `RETAIN CURRENT PASSWORD` was written (keep the old password as a secondary).
    pub retain_current_password: bool,
    /// Whether `DISCARD OLD PASSWORD` was written (drop the retained secondary password).
    pub discard_old_password: bool,
    /// Source location and node identity.
    pub meta: Meta,
}

/// The target of an `ALTER USER … DEFAULT ROLE` clause.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum DefaultRoleTarget {
    /// `DEFAULT ROLE ALL` — every granted role becomes a default.
    All {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DEFAULT ROLE NONE` — clear the default roles.
    None {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `DEFAULT ROLE <role> [, …]` — an explicit default-role list (always non-empty).
    Roles {
        /// The default roles, in source order.
        roles: ThinVec<AccountName>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A MySQL account/role-name-list DDL statement whose whole grammar is `<verb> [<if-guard>]
/// <name> [, …]`: `DROP USER`, `CREATE ROLE`, or `DROP ROLE`.
///
/// The three share one spine — a verb, an existence guard, and a comma list of
/// [`AccountName`]s — so they ride one node with the verb carried as data ([`kind`](Self::kind)),
/// the same "sub-command identity as data" precedent the SHOW / table-maintenance families use.
/// The richer `CREATE USER` / `ALTER USER` statements keep their own nodes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub struct UserRoleList {
    /// Which verb this is; also fixes the [`if_guard`](Self::if_guard) spelling
    /// (`IF EXISTS` for the drops, `IF NOT EXISTS` for `CREATE ROLE`).
    pub kind: UserRoleListKind,
    /// Whether the existence guard was written (`IF EXISTS` / `IF NOT EXISTS` per `kind`).
    pub if_guard: bool,
    /// The account/role names, in source order (always non-empty).
    pub names: ThinVec<AccountName>,
    /// Source location and node identity.
    pub meta: Meta,
}

/// Which account/role-list verb a [`UserRoleList`] carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum UserRoleListKind {
    /// `DROP USER [IF EXISTS] <user> [, …]`.
    DropUser,
    /// `CREATE ROLE [IF NOT EXISTS] <role> [, …]`.
    CreateRole,
    /// `DROP ROLE [IF EXISTS] <role> [, …]`.
    DropRole,
}

/// A `REQUIRE …` TLS requirement on a `CREATE`/`ALTER USER`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TlsRequirement {
    /// `REQUIRE NONE` — no TLS required.
    None {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REQUIRE SSL` — any TLS connection.
    Ssl {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REQUIRE X509` — a valid client certificate.
    X509 {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `REQUIRE <option> [AND <option> …]` — one or more certificate-attribute requirements
    /// (`SUBJECT`/`ISSUER`/`CIPHER`), always non-empty.
    Options {
        /// The certificate-attribute requirements, in source order.
        options: ThinVec<TlsOption>,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One certificate-attribute requirement inside a [`TlsRequirement::Options`] list.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum TlsOption {
    /// `SUBJECT '<subject>'`.
    Subject {
        /// The required certificate subject string literal.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ISSUER '<issuer>'`.
    Issuer {
        /// The required certificate issuer string literal.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `CIPHER '<cipher>'`.
    Cipher {
        /// The required cipher string literal.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One `WITH <resource>`-list resource limit on a `CREATE`/`ALTER USER`. Each value is an
/// unsigned-integer [`Literal`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum ResourceLimit {
    /// `MAX_QUERIES_PER_HOUR <n>`.
    MaxQueriesPerHour {
        /// The limit value.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `MAX_UPDATES_PER_HOUR <n>`.
    MaxUpdatesPerHour {
        /// The limit value.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `MAX_CONNECTIONS_PER_HOUR <n>`.
    MaxConnectionsPerHour {
        /// The limit value.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `MAX_USER_CONNECTIONS <n>`.
    MaxUserConnections {
        /// The limit value.
        value: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}

/// One password-management / account-lock option on a `CREATE`/`ALTER USER`. Numeric bodies
/// are unsigned-integer [`Literal`]s.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum PasswordLockOption {
    /// `ACCOUNT LOCK`.
    AccountLock {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ACCOUNT UNLOCK`.
    AccountUnlock {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD EXPIRE` — expire the password immediately.
    PasswordExpire {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD EXPIRE DEFAULT` — use the global `default_password_lifetime`.
    PasswordExpireDefault {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD EXPIRE NEVER`.
    PasswordExpireNever {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD EXPIRE INTERVAL <n> DAY`.
    PasswordExpireInterval {
        /// The interval, in days.
        days: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD HISTORY <n>` — remember the last `n` passwords.
    PasswordHistory {
        /// The number of remembered passwords.
        count: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD HISTORY DEFAULT`.
    PasswordHistoryDefault {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD REUSE INTERVAL <n> DAY`.
    PasswordReuseInterval {
        /// The reuse-blocking interval, in days.
        days: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD REUSE INTERVAL DEFAULT`.
    PasswordReuseIntervalDefault {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD REQUIRE CURRENT` — a password change must verify the current password.
    PasswordRequireCurrent {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD REQUIRE CURRENT DEFAULT`.
    PasswordRequireCurrentDefault {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD REQUIRE CURRENT OPTIONAL`.
    PasswordRequireCurrentOptional {
        /// Source location and node identity.
        meta: Meta,
    },
    /// `FAILED_LOGIN_ATTEMPTS <n>`.
    FailedLoginAttempts {
        /// The failed-login threshold.
        count: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD_LOCK_TIME <n>` — lock duration in days after `FAILED_LOGIN_ATTEMPTS`.
    PasswordLockTime {
        /// The lock duration, in days.
        days: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `PASSWORD_LOCK_TIME UNBOUNDED` — lock until manually unlocked.
    PasswordLockTimeUnbounded {
        /// Source location and node identity.
        meta: Meta,
    },
}

/// A `CREATE`/`ALTER USER` account attribute: `COMMENT '…'` or `ATTRIBUTE '<json>'`
/// (mutually exclusive in the grammar).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
pub enum UserAttribute {
    /// `COMMENT '<text>'`.
    Comment {
        /// The comment string literal.
        comment: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
    /// `ATTRIBUTE '<json>'` — a JSON object of user attributes.
    Attribute {
        /// The JSON attribute string literal.
        attribute: Literal,
        /// Source location and node identity.
        meta: Meta,
    },
}
