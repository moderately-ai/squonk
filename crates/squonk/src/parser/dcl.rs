// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Session-configuration and access-control statement grammar (DCL family).
//!
//! Owns the run-time configuration statements `SET` / `RESET` / `SHOW` and the
//! access-control statements `GRANT` / `REVOKE`, plus the leading-token recognizers
//! the statement dispatcher in [`super::query`] consults. Like the other families,
//! the vocabulary is matched as contextual words (the full keyword inventory is a
//! separate ticket) and accepted dialect-agnostically — these are grammar families
//! gated by acceptance, not `FeatureSet` data.
//!
//! `GRANT`/`REVOKE` cover the full privilege/object/grantee matrix, including
//! role-membership grants. `SET` covers the generic `<name> {= | TO} <value>` form
//! and the special-cased subforms (`TIME ZONE`, `ROLE`, `SESSION AUTHORIZATION`,
//! `CONSTRAINTS`, `NAMES`, and `SESSION CHARACTERISTICS`).

use crate::ast::{
    AccessControlStatement, AccountName, AlterUser, AlterUserSpec, AuthOption, CharacterSetKeyword,
    ConfigParameter, ConstraintCheckTime, ConstraintsTarget, CreateUser, DefaultRoleTarget,
    DropBehavior, GrantAs, GrantObject, Grantee, Ident, InstallComponentSetElement,
    InstallComponentSetScope, InstallComponentSetValue, InstallStatement, Keyword, Literal,
    LiteralKind, Meta, NamedObjectKind, ObjectName, PasswordLockOption, Privilege, PrivilegeKind,
    PrivilegeLevel, PrivilegeLevelObject, PrivilegeObjectType, Privileges, QuoteStyle,
    ResourceLimit, RoleSpec, RoutineObjectKind, RoutineSignature, SchemaObjectKind,
    SessionStatement, SetAssignment, SetCharacterSetValue, SetNamesValue, SetParameterValue,
    SetScope, SetValue, SetVariableAssignment, SetVariableKeyword, SetVariableValue, Span,
    SpecialSetValue, Statement, SystemVariableScope, SystemVariableScopeKind, TlsOption,
    TlsRequirement, UninstallStatement, UserAttribute, UserRoleList, UserRoleListKind, UserSpec,
    WithRoleSpec,
};
use crate::error::ParseResult;
use crate::tokenizer::{Operator, Punctuation, Token, TokenKind};
use thin_vec::{ThinVec, thin_vec};

use super::Dialect;
use super::engine::Parser;
use super::expr::number_literal_kind;

/// One element of the leading comma list shared by privilege and role-membership
/// grants — a privilege/role word with its built-in classification (if any) and an
/// optional column scope. Reinterpreted as privileges (`ON` follows) or granted
/// role names (a bare `TO`/`FROM` follows) once the branch is known.
struct GrantElement {
    word: Ident,
    kind: Option<PrivilegeKind>,
    columns: ThinVec<Ident>,
    meta: Meta,
}

/// One element of the MySQL `role_or_privilege_list` shared by privilege, role, and revoke
/// grammars. A static privilege phrase sets [`kind`](Self::kind); any other word is captured as
/// an [`account`](Self::account) (a dynamic-privilege name, or a `role`/`role@host`). The `ON`
/// versus `TO`/`FROM` branch reinterprets each element as a [`Privilege`] or an [`AccountName`].
struct AccountGrantElement {
    kind: Option<PrivilegeKind>,
    account: Option<AccountName>,
    columns: ThinVec<Ident>,
    meta: Meta,
}

/// The MySQL `role_or_privilege` static-privilege keyword phrases, longest-first so a multi-word
/// phrase is matched before its single-word prefix (`CREATE ROUTINE` before `CREATE`). Dynamic
/// privileges and role names are not here — they fall through to the account branch.
const ACCOUNT_STATIC_PRIVILEGES: &[(&[&str], PrivilegeKind)] = &[
    (
        &["CREATE", "TEMPORARY", "TABLES"],
        PrivilegeKind::CreateTemporaryTables,
    ),
    (&["CREATE", "ROUTINE"], PrivilegeKind::CreateRoutine),
    (&["CREATE", "VIEW"], PrivilegeKind::CreateView),
    (&["CREATE", "USER"], PrivilegeKind::CreateUser),
    (&["CREATE", "TABLESPACE"], PrivilegeKind::CreateTablespace),
    (&["CREATE", "ROLE"], PrivilegeKind::CreateRole),
    (&["ALTER", "ROUTINE"], PrivilegeKind::AlterRoutine),
    (&["DROP", "ROLE"], PrivilegeKind::DropRole),
    (&["SHOW", "DATABASES"], PrivilegeKind::ShowDatabases),
    (&["SHOW", "VIEW"], PrivilegeKind::ShowView),
    (&["REPLICATION", "SLAVE"], PrivilegeKind::ReplicationSlave),
    (&["REPLICATION", "CLIENT"], PrivilegeKind::ReplicationClient),
    (&["LOCK", "TABLES"], PrivilegeKind::LockTables),
    (&["GRANT", "OPTION"], PrivilegeKind::GrantOption),
    (&["SELECT"], PrivilegeKind::Select),
    (&["INSERT"], PrivilegeKind::Insert),
    (&["UPDATE"], PrivilegeKind::Update),
    (&["DELETE"], PrivilegeKind::Delete),
    (&["REFERENCES"], PrivilegeKind::References),
    (&["USAGE"], PrivilegeKind::Usage),
    (&["INDEX"], PrivilegeKind::Index),
    (&["ALTER"], PrivilegeKind::Alter),
    (&["CREATE"], PrivilegeKind::Create),
    (&["DROP"], PrivilegeKind::Drop),
    (&["EXECUTE"], PrivilegeKind::Execute),
    (&["RELOAD"], PrivilegeKind::Reload),
    (&["SHUTDOWN"], PrivilegeKind::Shutdown),
    (&["PROCESS"], PrivilegeKind::Process),
    (&["FILE"], PrivilegeKind::File),
    (&["SUPER"], PrivilegeKind::Super),
    (&["EVENT"], PrivilegeKind::Event),
    (&["TRIGGER"], PrivilegeKind::Trigger),
];

/// The static privileges that carry an optional `( <column> [, …] )` scope in MySQL
/// (`SELECT`/`INSERT`/`UPDATE`/`REFERENCES`); every other static privilege rejects a column
/// list.
fn static_privilege_takes_columns(kind: PrivilegeKind) -> bool {
    matches!(
        kind,
        PrivilegeKind::Select
            | PrivilegeKind::Insert
            | PrivilegeKind::Update
            | PrivilegeKind::References
    )
}

/// A reset-sentinel keyword admissible as the operand of a special single-valued
/// `SET` (`TIME ZONE`/`ROLE`/`SESSION AUTHORIZATION`). Each form admits a
/// different subset (see [`SpecialSetValue`]); the caller passes the subset and
/// [`Parser::parse_special_set_value`] is the gatekeeper.
#[derive(Clone, Copy)]
enum Sentinel {
    Default,
    Local,
    None,
}

impl Sentinel {
    fn keyword(self) -> &'static str {
        match self {
            Sentinel::Default => "DEFAULT",
            Sentinel::Local => "LOCAL",
            Sentinel::None => "NONE",
        }
    }
}

/// Classify a MySQL `@@<prefix>.name` scope word (case-insensitive) into its
/// [`SystemVariableScopeKind`], or `None` when the word is not a scope (so `@@foo.bar`
/// is a two-part variable name, not a scoped reference).
fn system_variable_scope_kind(prefix: &str) -> Option<SystemVariableScopeKind> {
    if prefix.eq_ignore_ascii_case("global") {
        Some(SystemVariableScopeKind::Global)
    } else if prefix.eq_ignore_ascii_case("session") {
        Some(SystemVariableScopeKind::Session)
    } else if prefix.eq_ignore_ascii_case("local") {
        Some(SystemVariableScopeKind::Local)
    } else if prefix.eq_ignore_ascii_case("persist") {
        Some(SystemVariableScopeKind::Persist)
    } else if prefix.eq_ignore_ascii_case("persist_only") {
        Some(SystemVariableScopeKind::PersistOnly)
    } else {
        None
    }
}

impl<'a, D: Dialect> Parser<'a, D> {
    /// True if the current token begins a session statement (`SET`/`RESET`/`SHOW`).
    ///
    /// `SET TRANSACTION` is transaction control and must be claimed by
    /// [`peek_starts_transaction_statement`](Self::peek_starts_transaction_statement)
    /// first; this recognizer does not distinguish it, so the dispatcher tests
    /// transaction control before sessions.
    pub(super) fn peek_starts_session_statement(&mut self) -> ParseResult<bool> {
        Ok(self.peek_is_contextual_keyword("SET")?
            || self.peek_is_contextual_keyword("RESET")?
            || self.peek_is_contextual_keyword("SHOW")?)
    }

    /// True if the current token begins an access-control statement.
    pub(super) fn peek_starts_access_control_statement(&mut self) -> ParseResult<bool> {
        Ok(self.peek_is_contextual_keyword("GRANT")?
            || self.peek_is_contextual_keyword("REVOKE")?)
    }

    /// Parse a `SET`/`RESET`/`SHOW` statement into [`Statement::Session`].
    pub(super) fn parse_session_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let session = self.parse_session_statement_kind(start)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Statement::Session {
            session: Box::new(session),
            meta,
        })
    }

    fn parse_session_statement_kind(
        &mut self,
        start: Span,
    ) -> ParseResult<SessionStatement<D::Ext>> {
        if self.eat_contextual_keyword("SET")? {
            self.parse_set(start)
        } else if self.eat_contextual_keyword("RESET")? {
            let scope = self.parse_optional_reset_scope()?;
            let target = self.parse_config_parameter()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(SessionStatement::Reset {
                scope,
                target,
                meta,
            })
        } else if self.eat_contextual_keyword("SHOW")? {
            let target = self.parse_config_parameter()?;
            // The trailing `VERBOSE` is the planner (sqlparser-rs/DataFusion) spelling; no
            // shipped oracle accepts it, so it is consumed only under the permissive
            // superset. Short-circuiting on the flag leaves `VERBOSE` in the token stream
            // for every other dialect, preserving today's trailing-token error there.
            let verbose = self.features().show_syntax.show_verbose
                && self.eat_contextual_keyword("VERBOSE")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(SessionStatement::Show {
                target,
                verbose,
                meta,
            })
        } else {
            Err(self.unexpected("a session statement"))
        }
    }

    /// Parse the body of a `SET` after its keyword, dispatching the special-cased
    /// subforms before falling back to the generic `SET <name> {= | TO} <value>`.
    ///
    /// `CONSTRAINTS`, `NAMES`, and `SESSION CHARACTERISTICS` take no `SESSION`/
    /// `LOCAL` scope, so they are recognized before scope is parsed (a leading
    /// `SESSION` there is part of the form, not a scope qualifier).
    fn parse_set(&mut self, start: Span) -> ParseResult<SessionStatement<D::Ext>> {
        if self.peek_is_contextual_keyword("CONSTRAINTS")? {
            return self.parse_set_constraints(start);
        }
        if self.peek_is_contextual_keyword("NAMES")? {
            return self.parse_set_names(start);
        }
        if self.peek_is_contextual_keyword("SESSION")?
            && self.peek_nth_is_contextual_keyword(1, "CHARACTERISTICS")?
        {
            return self.parse_set_session_characteristics(start);
        }
        // MySQL's `SET RESOURCE GROUP <name> [FOR <thread_ids>]` — a dedicated statement
        // grammar (`set_resource_group_stmt`) sharing only the `SET` head with the
        // variable-assignment forms, so it is claimed here on the two-word `RESOURCE GROUP`
        // lookahead, before both the scope parse (the grammar takes no `SESSION`/`LOCAL`/
        // `GLOBAL` prefix) and the MySQL variable-list fallback below. The seam is MECE with
        // that fallback: a variable named `resource` is always followed by `=`/`:=`/`.`,
        // never by the word `GROUP`, so the lookahead can never steal an assignment — and
        // with the gate off, `SET RESOURCE GROUP g` falls through to the assignment grammar
        // and surfaces as its parse error, mirroring a server without resource groups.
        if self.features().statement_ddl_gates.resource_group
            && self.peek_is_contextual_keyword("RESOURCE")?
            && self.peek_nth_is_contextual_keyword(1, "GROUP")?
        {
            return self.parse_set_resource_group(start);
        }
        // MySQL's `SET` is a distinct statement grammar (a comma list of heterogeneous
        // variable assignments over full-expression values), claimed here after the shared
        // standalone `NAMES`/`CONSTRAINTS`/`SESSION CHARACTERISTICS` forms and before the
        // generic single-target `SET`. `ROLE` (shared with PostgreSQL) is delegated to
        // [`parse_set_role`] so the MySQL `SET ROLE …` family keeps its dedicated node.
        if self.features().session_variables.variable_assignment {
            if self.eat_contextual_keyword("CHARSET")? {
                return self.parse_set_character_set(start, CharacterSetKeyword::Charset);
            }
            if self.peek_is_contextual_keyword("CHARACTER")?
                && self.peek_nth_is_contextual_keyword(1, "SET")?
            {
                self.expect_contextual_keyword("CHARACTER")?;
                self.expect_contextual_keyword("SET")?;
                return self.parse_set_character_set(start, CharacterSetKeyword::CharacterSet);
            }
            if self.peek_is_contextual_keyword("ROLE")? {
                return self.parse_set_role(start, None);
            }
            return self.parse_mysql_set_variables(start);
        }
        let scope = self.parse_optional_set_scope()?;
        if self.peek_is_contextual_keyword("TIME")?
            && self.peek_nth_is_contextual_keyword(1, "ZONE")?
        {
            return self.parse_set_time_zone(start, scope);
        }
        if self.peek_is_contextual_keyword("ROLE")? {
            return self.parse_set_role(start, scope);
        }
        if self.peek_is_contextual_keyword("SESSION")?
            && self.peek_nth_is_contextual_keyword(1, "AUTHORIZATION")?
        {
            return self.parse_set_session_authorization(start, scope);
        }
        let name = self.parse_object_name()?;
        let assignment = self.expect_set_assignment()?;
        let value = self.parse_set_value()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SessionStatement::Set {
            scope,
            name,
            assignment,
            value,
            meta,
        })
    }

    /// Parse the optional `SESSION` / `LOCAL` scope of a `SET`.
    ///
    /// A leading `SESSION` is a scope qualifier only when it does not open `SESSION
    /// AUTHORIZATION` (which is a parameter form, not a scope); the `SESSION
    /// CHARACTERISTICS` form is already claimed before this point.
    fn parse_optional_set_scope(&mut self) -> ParseResult<Option<SetScope>> {
        if self.eat_contextual_keyword("LOCAL")? {
            Ok(Some(SetScope::Local))
        } else if self.peek_is_contextual_keyword("SESSION")?
            && !self.peek_nth_is_contextual_keyword(1, "AUTHORIZATION")?
        {
            self.expect_contextual_keyword("SESSION")?;
            Ok(Some(SetScope::Session))
        } else {
            Ok(None)
        }
    }

    /// Parse `[SESSION | LOCAL] TIME ZONE { <value> | LOCAL | DEFAULT }` after `SET`.
    fn parse_set_time_zone(
        &mut self,
        start: Span,
        scope: Option<SetScope>,
    ) -> ParseResult<SessionStatement<D::Ext>> {
        self.expect_contextual_keyword("TIME")?;
        self.expect_contextual_keyword("ZONE")?;
        let value = Box::new(self.parse_special_set_value(&[Sentinel::Local, Sentinel::Default])?);
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SessionStatement::SetTimeZone { scope, value, meta })
    }

    /// Parse `[SESSION | LOCAL] ROLE { <name> | NONE }` after `SET`.
    fn parse_set_role(
        &mut self,
        start: Span,
        scope: Option<SetScope>,
    ) -> ParseResult<SessionStatement<D::Ext>> {
        self.expect_contextual_keyword("ROLE")?;
        let role = Box::new(self.parse_special_set_value(&[Sentinel::None])?);
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SessionStatement::SetRole { scope, role, meta })
    }

    /// Parse MySQL's `SET RESOURCE GROUP <name> [FOR <thread_id> [[,] …]]` after `SET` (the
    /// `RESOURCE GROUP` words are still pending). The thread-id list is `real_ulong_num`s under
    /// `opt_comma` separators — `FOR 1, 2` and `FOR 1 2` both grammar-accept on mysql:8.4.10 —
    /// so the list continues while the next token is a number, comma or not; each id admits the
    /// `real_ulong_num` radix spellings (decimal or `0x` hex).
    fn parse_set_resource_group(&mut self, start: Span) -> ParseResult<SessionStatement<D::Ext>> {
        self.expect_contextual_keyword("RESOURCE")?;
        self.expect_contextual_keyword("GROUP")?;
        let name = self.parse_ident()?;
        let thread_ids = if self.eat_keyword(Keyword::For)? {
            let mut ids = ThinVec::new();
            loop {
                ids.push(self.expect_unsigned_integer_literal("FOR")?);
                let ate_comma = self.eat_punct(Punctuation::Comma)?;
                let next_is_number = self
                    .peek()?
                    .is_some_and(|token| token.kind == TokenKind::Number);
                if !ate_comma && !next_is_number {
                    break;
                }
            }
            Some(ids)
        } else {
            None
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SessionStatement::SetResourceGroup {
            name,
            thread_ids,
            meta,
        })
    }

    /// Parse `[SESSION | LOCAL] SESSION AUTHORIZATION { <name> | DEFAULT }` after `SET`.
    fn parse_set_session_authorization(
        &mut self,
        start: Span,
        scope: Option<SetScope>,
    ) -> ParseResult<SessionStatement<D::Ext>> {
        self.expect_contextual_keyword("SESSION")?;
        self.expect_contextual_keyword("AUTHORIZATION")?;
        let user = Box::new(self.parse_special_set_value(&[Sentinel::Default])?);
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SessionStatement::SetSessionAuthorization { scope, user, meta })
    }

    /// Parse the operand of a special single-valued `SET`: one of the admissible
    /// reset sentinels, else an explicit value. A sentinel is checked before the
    /// generic value so its keyword is not captured as a bareword name.
    fn parse_special_set_value(&mut self, sentinels: &[Sentinel]) -> ParseResult<SpecialSetValue> {
        let start = self.current_span()?;
        for &sentinel in sentinels {
            if self.eat_contextual_keyword(sentinel.keyword())? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(match sentinel {
                    Sentinel::Default => SpecialSetValue::Default { meta },
                    Sentinel::Local => SpecialSetValue::Local { meta },
                    Sentinel::None => SpecialSetValue::None { meta },
                });
            }
        }
        let value = self.parse_set_parameter_value()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SpecialSetValue::Value { value, meta })
    }

    /// Parse `CONSTRAINTS { ALL | <name> [, ...] } { DEFERRED | IMMEDIATE }` after `SET`.
    fn parse_set_constraints(&mut self, start: Span) -> ParseResult<SessionStatement<D::Ext>> {
        self.expect_contextual_keyword("CONSTRAINTS")?;
        let constraints = self.parse_constraints_target()?;
        let check_time = self.parse_constraint_check_time()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SessionStatement::SetConstraints {
            constraints,
            check_time,
            meta,
        })
    }

    fn parse_constraints_target(&mut self) -> ParseResult<ConstraintsTarget> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("ALL")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ConstraintsTarget::All { meta })
        } else {
            let names = self.parse_object_name_list()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ConstraintsTarget::Names { names, meta })
        }
    }

    fn parse_constraint_check_time(&mut self) -> ParseResult<ConstraintCheckTime> {
        if self.eat_contextual_keyword("DEFERRED")? {
            Ok(ConstraintCheckTime::Deferred)
        } else if self.eat_contextual_keyword("IMMEDIATE")? {
            Ok(ConstraintCheckTime::Immediate)
        } else {
            Err(self.unexpected("`DEFERRED` or `IMMEDIATE` after the constraint list"))
        }
    }

    /// Parse `NAMES { <charset> [COLLATE <collation>] | DEFAULT }` after `SET` (MySQL).
    fn parse_set_names(&mut self, start: Span) -> ParseResult<SessionStatement<D::Ext>> {
        self.expect_contextual_keyword("NAMES")?;
        let value = Box::new(self.parse_set_names_value()?);
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SessionStatement::SetNames { value, meta })
    }

    fn parse_set_names_value(&mut self) -> ParseResult<SetNamesValue> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("DEFAULT")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(SetNamesValue::Default { meta })
        } else {
            let charset = self.parse_set_parameter_value()?;
            let collation = if self.eat_contextual_keyword("COLLATE")? {
                Some(self.parse_ident()?)
            } else {
                None
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(SetNamesValue::Charset {
                charset,
                collation,
                meta,
            })
        }
    }

    // --- MySQL variable-assignment SET (option_value_list) -------------------

    /// Parse the MySQL `SET` variable-assignment list after the `SET` keyword — a
    /// comma-separated list of heterogeneous [`SetVariableAssignment`]s — into
    /// [`SessionStatement::SetVariables`]. Reached only under
    /// [`SessionVariableSyntax::variable_assignment`](crate::ast::dialect::SessionVariableSyntax);
    /// the shared `NAMES`/`CONSTRAINTS`/`SESSION CHARACTERISTICS`/`ROLE`/`CHARACTER SET`
    /// forms are already claimed by [`parse_set`](Self::parse_set) before this point.
    fn parse_mysql_set_variables(&mut self, start: Span) -> ParseResult<SessionStatement<D::Ext>> {
        let assignments = self.parse_comma_separated(Self::parse_mysql_set_item)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SessionStatement::SetVariables { assignments, meta })
    }

    /// Parse one MySQL `SET` list item: a user-defined `@var` assignment, an `@@[scope.]`
    /// system-variable assignment, a keyword-scoped `GLOBAL x = …` assignment, or a plain
    /// `x = …` (session-implicit) assignment.
    fn parse_mysql_set_item(&mut self) -> ParseResult<SetVariableAssignment<D::Ext>> {
        let start = self.current_span()?;
        // A user variable spells its name two ways at the lexer: `@name` folds into one
        // `Variable` token, while `@'name'`/`@"name"`/`` @`name` `` cannot fold, so the
        // lexer emits a standalone `@` then the quoted `ident_or_text` — mirroring the
        // account-host `@` reconciliation.
        if let Some(token) = self.peek()? {
            match token.kind {
                TokenKind::Punctuation(Punctuation::At) => {
                    self.advance()?; // the standalone `@`
                    let name = self.parse_ident_or_text()?;
                    return self.finish_user_variable(start, name);
                }
                TokenKind::Variable => {
                    let text = self.span_text(token.span);
                    if text.starts_with("@@") {
                        return self.parse_mysql_at_at_variable(start, token);
                    }
                    // A single-`@` user variable (`@name`); the sigil is one ASCII byte.
                    self.advance()?;
                    let sym = self.intern_text(&text[1..]);
                    let name = Ident {
                        sym,
                        quote: QuoteStyle::None,
                        meta: self.make_meta(token.span),
                    };
                    return self.finish_user_variable(start, name);
                }
                _ => {}
            }
        }
        // A keyword scope prefix (`GLOBAL`/`SESSION`/`LOCAL`/`PERSIST`/`PERSIST_ONLY`) makes
        // this a scoped system variable; otherwise it is a plain session-implicit one.
        let scope = match self.eat_system_variable_scope_keyword()? {
            Some(kind) => SystemVariableScope::Keyword(kind),
            None => SystemVariableScope::Implicit,
        };
        let name = self.parse_set_lvalue_name()?;
        self.finish_system_variable(start, scope, name)
    }

    /// Finish a user-variable assignment (`@v {= | :=} <expr>`) whose name is already read.
    /// The value is a full expression — the sentinels a system variable admits
    /// (`DEFAULT`/`ON`/…) are not valid here (MySQL's `expr`, not `set_expr_or_default`).
    fn finish_user_variable(
        &mut self,
        start: Span,
        name: Ident,
    ) -> ParseResult<SetVariableAssignment<D::Ext>> {
        let assignment = self.expect_mysql_set_assignment()?;
        let value = Box::new(self.parse_expr()?);
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SetVariableAssignment::UserVariable {
            name,
            assignment,
            value,
            meta,
        })
    }

    /// Parse an `@@[scope.]<name>` system-variable assignment. The lexer folds the whole
    /// `@@`/`scope.`/`name` reference into one `Variable` token, so the sigil, optional
    /// scope word, and (possibly dotted) name are recovered from the token text here.
    fn parse_mysql_at_at_variable(
        &mut self,
        start: Span,
        token: Token,
    ) -> ParseResult<SetVariableAssignment<D::Ext>> {
        self.advance()?; // the `@@…` token
        let text = self.span_text(token.span);
        let rest = &text[2..]; // strip the `@@` sigil
        let (scope, name_text) = match rest.split_once('.') {
            Some((prefix, tail)) => match system_variable_scope_kind(prefix) {
                Some(kind) => (SystemVariableScope::AtAtScoped(kind), tail),
                // A non-scope first part is a two-part variable name (`@@foo.bar`).
                None => (SystemVariableScope::AtAt, rest),
            },
            None => (SystemVariableScope::AtAt, rest),
        };
        let name = self.object_name_from_dotted(name_text, token.span);
        self.finish_system_variable(start, scope, name)
    }

    /// Finish a system-variable assignment (`<name> {= | :=} <set_expr_or_default>`) whose
    /// scope and name are already read.
    fn finish_system_variable(
        &mut self,
        start: Span,
        scope: SystemVariableScope,
        name: ObjectName,
    ) -> ParseResult<SetVariableAssignment<D::Ext>> {
        let assignment = self.expect_mysql_set_assignment()?;
        let value = self.parse_set_variable_value()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SetVariableAssignment::SystemVariable {
            scope,
            name,
            assignment,
            value,
            meta,
        })
    }

    /// Parse a `lvalue_variable` name: a one- or two-part `name[.name]`, or the
    /// `DEFAULT.<name>` default-scope spelling. (A `GLOBAL.`/`LOCAL.`/`SESSION.` prefix is a
    /// syntax error in MySQL — those words are reserved as scope keywords — so a bare name
    /// part is read verbatim; the server rejects a reserved prefix at bind, past parse.)
    fn parse_set_lvalue_name(&mut self) -> ParseResult<ObjectName> {
        let start = self.current_span()?;
        // `DEFAULT '.' ident` names the variable's default scope; `DEFAULT` is a keyword, so
        // it is matched explicitly rather than through `parse_ident` (which rejects keywords).
        if self.peek_is_contextual_keyword("DEFAULT")?
            && self.peek_nth_is_punct(1, Punctuation::Dot)?
        {
            self.expect_contextual_keyword("DEFAULT")?;
            let first = Ident {
                sym: self.intern_text("default"),
                quote: QuoteStyle::None,
                meta: self.make_meta(start),
            };
            self.expect_punct(
                Punctuation::Dot,
                "`.` after `DEFAULT` in a SET variable name",
            )?;
            let second = self.parse_ident()?;
            return Ok(ObjectName(thin_vec![first, second]));
        }
        self.parse_object_name()
    }

    /// Build an [`ObjectName`] from the `.`-separated `name_text` of a folded `@@…` token,
    /// each part interned exact-case with no quote (the sigil form is never quoted). All
    /// parts share the token's span, since the lexer folded them into one lexeme.
    fn object_name_from_dotted(&mut self, name_text: &str, span: Span) -> ObjectName {
        let parts: ThinVec<Ident> = name_text
            .split('.')
            .map(|part| Ident {
                sym: self.intern_text(part),
                quote: QuoteStyle::None,
                meta: self.make_meta(span),
            })
            .collect();
        ObjectName(parts)
    }

    /// Consume a MySQL `SET` scope keyword (`GLOBAL`/`SESSION`/`LOCAL`/`PERSIST`/
    /// `PERSIST_ONLY`), returning which — or `None` when the next token is not one.
    fn eat_system_variable_scope_keyword(
        &mut self,
    ) -> ParseResult<Option<SystemVariableScopeKind>> {
        if self.eat_contextual_keyword("GLOBAL")? {
            Ok(Some(SystemVariableScopeKind::Global))
        } else if self.eat_contextual_keyword("PERSIST_ONLY")? {
            Ok(Some(SystemVariableScopeKind::PersistOnly))
        } else if self.eat_contextual_keyword("PERSIST")? {
            Ok(Some(SystemVariableScopeKind::Persist))
        } else if self.eat_contextual_keyword("SESSION")? {
            Ok(Some(SystemVariableScopeKind::Session))
        } else if self.eat_contextual_keyword("LOCAL")? {
            Ok(Some(SystemVariableScopeKind::Local))
        } else {
            Ok(None)
        }
    }

    /// Consume the `=` or `:=` separator of a MySQL variable assignment. MySQL's `SET_VAR`
    /// admits both; unlike PostgreSQL's generic `SET` there is no `TO` spelling here.
    fn expect_mysql_set_assignment(&mut self) -> ParseResult<SetAssignment> {
        if self.peek_is_op(Operator::Eq)? {
            self.advance()?;
            Ok(SetAssignment::Equals)
        } else if self.peek_is_op(Operator::ColonEquals)? {
            self.advance()?;
            Ok(SetAssignment::ColonEquals)
        } else {
            Err(self.unexpected("`=` or `:=` in a MySQL SET assignment"))
        }
    }

    /// Parse a `set_expr_or_default` value: `DEFAULT`, one of the special keyword sentinels
    /// (`ON`/`ALL`/`BINARY`/`ROW`/`SYSTEM`) the grammar folds to a string, or a full
    /// expression. The keyword sentinels are checked before the expression because they are
    /// reserved words the expression grammar would reject in value position.
    fn parse_set_variable_value(&mut self) -> ParseResult<SetVariableValue<D::Ext>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("DEFAULT")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(SetVariableValue::Default { meta });
        }
        for (word, keyword) in [
            ("ON", SetVariableKeyword::On),
            ("ALL", SetVariableKeyword::All),
            ("BINARY", SetVariableKeyword::Binary),
            ("ROW", SetVariableKeyword::Row),
            ("SYSTEM", SetVariableKeyword::System),
        ] {
            if self.eat_contextual_keyword(word)? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(SetVariableValue::Keyword { keyword, meta });
            }
        }
        let expr = Box::new(self.parse_expr()?);
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SetVariableValue::Expr { expr, meta })
    }

    /// Parse a MySQL `INSTALL PLUGIN`/`INSTALL COMPONENT` statement into
    /// [`Statement::Install`], reached under
    /// [`UtilitySyntax::plugin_component_statements`](crate::ast::dialect::UtilitySyntax). The
    /// leading `INSTALL` is peeked by the dispatcher; the following `PLUGIN`/`COMPONENT` word
    /// selects the form (`sql_yacc.yy` `install_stmt`). `PLUGIN` takes a single bare `ident`
    /// name and a required `SONAME` string; `COMPONENT` a non-empty comma list of string URNs
    /// and an optional `SET` tail.
    pub(super) fn parse_install_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("INSTALL")?;
        let install = if self.eat_contextual_keyword("PLUGIN")? {
            let name = self.parse_ident()?;
            self.expect_contextual_keyword("SONAME")?;
            let soname =
                self.expect_string_literal("a SONAME string literal after the plugin name")?;
            InstallStatement::Plugin {
                name,
                soname,
                meta: self.make_meta(start.union(self.preceding_span())),
            }
        } else if self.eat_contextual_keyword("COMPONENT")? {
            let urns = self.parse_component_urn_list()?;
            let set = self.parse_install_component_set()?;
            InstallStatement::Component {
                urns,
                set,
                meta: self.make_meta(start.union(self.preceding_span())),
            }
        } else {
            return Err(self.unexpected("`PLUGIN` or `COMPONENT` after INSTALL"));
        };
        let statement_meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::Install {
            install: Box::new(install),
            meta: statement_meta,
        })
    }

    /// Parse a MySQL `UNINSTALL PLUGIN`/`UNINSTALL COMPONENT` statement into
    /// [`Statement::Uninstall`], the inverse of [`parse_install_statement`](Self::parse_install_statement)
    /// under the same gate (`sql_yacc.yy` `uninstall`). `PLUGIN` takes a single bare `ident`;
    /// `COMPONENT` a non-empty comma list of string URNs. Neither form has a `SONAME` or `SET`
    /// tail.
    pub(super) fn parse_uninstall_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("UNINSTALL")?;
        let uninstall = if self.eat_contextual_keyword("PLUGIN")? {
            let name = self.parse_ident()?;
            UninstallStatement::Plugin {
                name,
                meta: self.make_meta(start.union(self.preceding_span())),
            }
        } else if self.eat_contextual_keyword("COMPONENT")? {
            let urns = self.parse_component_urn_list()?;
            UninstallStatement::Component {
                urns,
                meta: self.make_meta(start.union(self.preceding_span())),
            }
        } else {
            return Err(self.unexpected("`PLUGIN` or `COMPONENT` after UNINSTALL"));
        };
        let statement_meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::Uninstall {
            uninstall: Box::new(uninstall),
            meta: statement_meta,
        })
    }

    /// Parse the `COMPONENT` URN list (`sql_yacc.yy` `TEXT_STRING_sys_list`): a non-empty,
    /// no-trailing-comma list of string literals, shared by `INSTALL`/`UNINSTALL COMPONENT`.
    fn parse_component_urn_list(&mut self) -> ParseResult<ThinVec<Literal>> {
        self.parse_comma_separated(|p| p.expect_string_literal("a component URN string literal"))
    }

    /// Parse the optional `SET <install_set_value> [, …]` tail of `INSTALL COMPONENT`
    /// (`sql_yacc.yy` `opt_install_set_value_list`) — an empty list when no `SET` is written.
    fn parse_install_component_set(
        &mut self,
    ) -> ParseResult<ThinVec<InstallComponentSetElement<D::Ext>>> {
        if !self.eat_contextual_keyword("SET")? {
            return Ok(ThinVec::new());
        }
        self.parse_comma_separated(Self::parse_install_component_set_element)
    }

    /// Parse one `INSTALL COMPONENT … SET` assignment (`sql_yacc.yy` `install_set_value`):
    /// `[GLOBAL | PERSIST] <lvalue_variable> {= | :=} <install_set_rvalue>`. The variable name
    /// and assignment operator reuse the general MySQL `SET` machinery
    /// ([`parse_set_lvalue_name`](Self::parse_set_lvalue_name),
    /// [`expect_mysql_set_assignment`](Self::expect_mysql_set_assignment)); the scope and value
    /// are the narrower `install_option_type` / `install_set_rvalue` grammars.
    fn parse_install_component_set_element(
        &mut self,
    ) -> ParseResult<InstallComponentSetElement<D::Ext>> {
        let start = self.current_span()?;
        let scope = self.eat_install_component_set_scope()?;
        let name = self.parse_set_lvalue_name()?;
        let assignment = self.expect_mysql_set_assignment()?;
        let value = self.parse_install_set_rvalue()?;
        Ok(InstallComponentSetElement {
            scope,
            name,
            assignment,
            value,
            meta: self.make_meta(start.union(self.preceding_span())),
        })
    }

    /// Consume the `install_option_type` scope keyword — `GLOBAL` or `PERSIST` — or `None` for
    /// the implicit default (which MySQL resolves to `GLOBAL`). Deliberately narrower than the
    /// general `SET` scope set: `SESSION`/`LOCAL`/`PERSIST_ONLY` are not part of this grammar,
    /// so an unmatched word falls through to the variable-name parse and rejects there (as
    /// mysql:8 does).
    fn eat_install_component_set_scope(&mut self) -> ParseResult<Option<InstallComponentSetScope>> {
        if self.eat_contextual_keyword("GLOBAL")? {
            Ok(Some(InstallComponentSetScope::Global))
        } else if self.eat_contextual_keyword("PERSIST")? {
            Ok(Some(InstallComponentSetScope::Persist))
        } else {
            Ok(None)
        }
    }

    /// Parse an `install_set_rvalue`: the `ON` keyword or a value [`Expr`](crate::ast::Expr).
    /// `ON` is tried first because it is a reserved word the expression grammar would reject in
    /// value position (the general `SET`'s other value sentinels — `DEFAULT`/`ALL`/… — are not
    /// part of this narrower grammar).
    fn parse_install_set_rvalue(&mut self) -> ParseResult<InstallComponentSetValue<D::Ext>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("ON")? {
            return Ok(InstallComponentSetValue::On {
                meta: self.make_meta(start.union(self.preceding_span())),
            });
        }
        let expr = self.parse_expr()?;
        Ok(InstallComponentSetValue::Expr {
            expr,
            meta: self.make_meta(start.union(self.preceding_span())),
        })
    }

    /// Parse `SET { CHARACTER SET | CHARSET } { <charset> | DEFAULT }` (the leading keyword
    /// already consumed) into [`SessionStatement::SetCharacterSet`].
    fn parse_set_character_set(
        &mut self,
        start: Span,
        keyword: CharacterSetKeyword,
    ) -> ParseResult<SessionStatement<D::Ext>> {
        let value_start = self.current_span()?;
        let value = if self.eat_contextual_keyword("DEFAULT")? {
            let meta = self.make_meta(value_start.union(self.preceding_span()));
            SetCharacterSetValue::Default { meta }
        } else {
            let charset = self.parse_set_parameter_value()?;
            let meta = self.make_meta(value_start.union(self.preceding_span()));
            SetCharacterSetValue::Charset { charset, meta }
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SessionStatement::SetCharacterSet {
            keyword,
            value: Box::new(value),
            meta,
        })
    }

    /// Parse `SESSION CHARACTERISTICS AS TRANSACTION <mode> [, ...]` after `SET`.
    fn parse_set_session_characteristics(
        &mut self,
        start: Span,
    ) -> ParseResult<SessionStatement<D::Ext>> {
        self.expect_contextual_keyword("SESSION")?;
        self.expect_contextual_keyword("CHARACTERISTICS")?;
        self.expect_contextual_keyword("AS")?;
        self.expect_contextual_keyword("TRANSACTION")?;
        let modes = self.parse_transaction_modes()?;
        if modes.is_empty() {
            return Err(self.unexpected(
                "a transaction mode after `SET SESSION CHARACTERISTICS AS TRANSACTION`",
            ));
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SessionStatement::SetSessionCharacteristics { modes, meta })
    }

    /// Consume the `=` or `TO` separator of a `SET <name> {= | TO} <value>`.
    ///
    /// The two spellings are interchangeable and not preserved.
    pub(super) fn expect_set_assignment(&mut self) -> ParseResult<SetAssignment> {
        if self.peek_is_op(Operator::Eq)? {
            self.advance()?;
            Ok(SetAssignment::Equals)
        } else if self.eat_contextual_keyword("TO")? {
            Ok(SetAssignment::To)
        } else {
            Err(self.unexpected("`=` or `TO` in a `SET` statement"))
        }
    }

    pub(super) fn parse_set_value(&mut self) -> ParseResult<SetValue> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("DEFAULT")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(SetValue::Default { meta });
        }
        let values = self.parse_comma_separated(Self::parse_generic_set_parameter_value)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SetValue::Values { values, meta })
    }

    /// Parse one `SET` value: a numeric/string literal, a bareword name, or — under
    /// DuckDB's [`collection_literals`](crate::ast::dialect::ExpressionSyntax) — a
    /// bracketed list value (`['a', 'b']`, `[]`).
    ///
    /// A bareword value may be a reserved keyword (e.g. `on`), so it is accepted as
    /// a verbatim name rather than going through [`parse_ident`](Self::parse_ident),
    /// which (correctly) rejects reserved keywords in identifier position.
    /// `pub(super)` because SQLite's `PRAGMA` value grammar is this exact shape
    /// (`signed-number | name | string-literal`) and reuses it (see [`super::util`]);
    /// DuckDB's `PRAGMA`/`SET` additionally admit the list value through the same gate.
    pub(super) fn parse_set_parameter_value(&mut self) -> ParseResult<SetParameterValue> {
        let start = self.current_span()?;
        // DuckDB admits a bracketed list value (`SET allowed_paths = ['a', 'b']`,
        // `SET allowed_directories = []`), reusing the `[…]` collection-literal syntax.
        // Gated by the same `collection_literals` flag that makes `[` open a list rather
        // than a quoted identifier, so no dialect that reads `[` as a quote misreads a
        // list value here.
        if self.features().expression_syntax.collection_literals
            && self.peek_is_punct(Punctuation::LBracket)?
        {
            return self.parse_set_parameter_list(start);
        }
        // A leading sign binds only to a numeric value: PG's `NumericOnly` folds the
        // sign into the constant (so `-1` is one value, not a unary expression). The
        // sign and digits are unioned into the literal's span so it round-trips whole.
        if self.peek_is_op(Operator::Minus)? || self.peek_is_op(Operator::Plus)? {
            let sign_span = self.current_span()?;
            self.advance()?;
            let number = self
                .peek()?
                .filter(|token| token.kind == TokenKind::Number)
                .ok_or_else(|| self.unexpected("a number after a sign in a SET value"))?;
            self.advance()?;
            let span = sign_span.union(number.span);
            let literal = Literal {
                kind: number_literal_kind(self.span_text(span), self.float_as_decimal_enabled()),
                meta: self.make_meta(span),
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(SetParameterValue::Literal { literal, meta });
        }
        match self.peek()? {
            Some(token) if token.kind == TokenKind::Number => {
                self.advance()?;
                let kind = number_literal_kind(
                    self.span_text(token.span),
                    self.float_as_decimal_enabled(),
                );
                let literal = Literal {
                    kind,
                    meta: self.make_meta(token.span),
                };
                let meta = self.make_meta(start.union(self.preceding_span()));
                Ok(SetParameterValue::Literal { literal, meta })
            }
            Some(token) if token.kind == TokenKind::String => {
                self.advance()?;
                let literal = Literal {
                    kind: LiteralKind::String,
                    meta: self.make_meta(token.span),
                };
                let meta = self.make_meta(start.union(self.preceding_span()));
                Ok(SetParameterValue::Literal { literal, meta })
            }
            Some(token) if matches!(token.kind, TokenKind::Word | TokenKind::Keyword(_)) => {
                self.advance()?;
                let sym = self.intern_identifier(token);
                let name = Ident {
                    sym,
                    quote: QuoteStyle::None,
                    meta: self.make_meta(token.span),
                };
                let meta = self.make_meta(start.union(self.preceding_span()));
                Ok(SetParameterValue::Name { name, meta })
            }
            _ => Err(self.unexpected("a SET value")),
        }
    }

    /// Parse one value in the generic `SET <parameter> {= | TO} ...` production,
    /// applying that production's dialect-specific keyword class before materializing the
    /// shared value node. SQLite `PRAGMA`, `SET NAMES`, and other consumers of
    /// [`parse_set_parameter_value`](Self::parse_set_parameter_value) intentionally do not
    /// inherit this restriction merely because they reuse the same AST shape.
    fn parse_generic_set_parameter_value(&mut self) -> ParseResult<SetParameterValue> {
        if let Some(token) = self.peek()?
            && matches!(token.kind, TokenKind::Word | TokenKind::Keyword(_))
            && !self.token_is_set_bareword_value(token)
            && !self.token_is_set_special_keyword_value(token)
        {
            return Err(self.unexpected("a non-reserved word or literal as a SET value"));
        }
        self.parse_set_parameter_value()
    }

    /// Whether a token belongs to the dialect's ordinary bareword class for a generic
    /// `SET` value.
    fn token_is_set_bareword_value(&self, token: Token) -> bool {
        match token.kind {
            TokenKind::Word => true,
            TokenKind::Keyword(keyword) => !self
                .features()
                .show_syntax
                .set_value_reserved_words
                .contains(keyword),
            _ => false,
        }
    }

    /// Whether an otherwise-reserved keyword is explicitly admitted by the generic `SET`
    /// value grammar.
    fn token_is_set_special_keyword_value(&self, token: Token) -> bool {
        if !matches!(token.kind, TokenKind::Word | TokenKind::Keyword(_)) {
            return false;
        }
        let text = self.span_text(token.span);
        text.eq_ignore_ascii_case("TRUE")
            || text.eq_ignore_ascii_case("FALSE")
            || (self.features().show_syntax.set_value_on_keyword && text.eq_ignore_ascii_case("ON"))
            || (self.features().show_syntax.set_value_null_keyword
                && text.eq_ignore_ascii_case("NULL"))
    }

    /// Parse a DuckDB bracketed list value `[ <value> [, ...] ]` — possibly empty (`[]`)
    /// — with the leading `[` already peeked and `start` its span. Reached only under
    /// `collection_literals`. Each element is again the restricted [`SetParameterValue`]
    /// grammar, so a nested list is representable; DuckDB's parser accepts richer element
    /// expressions but rejects them at bind, past this validator's parse-level contract.
    fn parse_set_parameter_list(&mut self, start: Span) -> ParseResult<SetParameterValue> {
        self.expect_punct(Punctuation::LBracket, "`[` to open the SET list value")?;
        let values = if self.peek_is_punct(Punctuation::RBracket)? {
            ThinVec::new()
        } else {
            self.parse_comma_separated(Self::parse_set_parameter_value)?
        };
        self.expect_punct(Punctuation::RBracket, "`]` to close the SET list value")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SetParameterValue::List { values, meta })
    }

    /// Parse a `RESET`/`SHOW` target: `ALL` or a named parameter.
    /// Parse the optional `SESSION | LOCAL | GLOBAL` scope before a `RESET` target
    /// (DuckDB; gated by
    /// [`UtilitySyntax::reset_scope`](crate::ast::dialect::UtilitySyntax)).
    ///
    /// DuckDB's grammar is `RESET (SESSION | LOCAL | GLOBAL)? <var_name>`: the scope
    /// keyword is a scope *only* when a configuration parameter follows it. `RESET
    /// SESSION` alone resets the parameter literally named `session` (a `Catalog Error`
    /// at bind, so a parse accept), and `RESET SESSION AUTHORIZATION` is a DuckDB parser
    /// error because `AUTHORIZATION` cannot be a `var_name` (all probed on 1.5.4). So the
    /// scope keyword is consumed only when the next token can start a parameter name;
    /// otherwise the cursor is rewound and the keyword becomes the parameter.
    fn parse_optional_reset_scope(&mut self) -> ParseResult<Option<SetScope>> {
        if !self.features().utility_syntax.reset_scope {
            return Ok(None);
        }
        let scope = if self.peek_is_contextual_keyword("SESSION")? {
            SetScope::Session
        } else if self.peek_is_contextual_keyword("LOCAL")? {
            SetScope::Local
        } else if self.peek_is_contextual_keyword("GLOBAL")? {
            SetScope::Global
        } else {
            return Ok(None);
        };
        let checkpoint = self.checkpoint();
        self.advance()?;
        if self.peek_can_start_column_name()? {
            Ok(Some(scope))
        } else {
            self.rewind(checkpoint);
            Ok(None)
        }
    }

    pub(super) fn parse_config_parameter(&mut self) -> ParseResult<ConfigParameter> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("ALL")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ConfigParameter::All { meta })
        } else {
            let name = self.parse_object_name()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(ConfigParameter::Named { name, meta })
        }
    }

    /// Parse a `GRANT`/`REVOKE` statement into [`Statement::AccessControl`].
    pub(super) fn parse_access_control_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let access = self.parse_access_control_statement_kind(start)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Statement::AccessControl {
            access: Box::new(access),
            meta,
        })
    }

    fn parse_access_control_statement_kind(
        &mut self,
        start: Span,
    ) -> ParseResult<AccessControlStatement<D::Ext>> {
        if self.eat_contextual_keyword("GRANT")? {
            self.parse_grant_kind(start)
        } else if self.eat_contextual_keyword("REVOKE")? {
            self.parse_revoke_kind(start)
        } else {
            Err(self.unexpected("`GRANT` or `REVOKE`"))
        }
    }

    /// Parse a `GRANT` body after the keyword.
    ///
    /// `ALL [PRIVILEGES]` and a comma list of privilege/role words share the leading
    /// position; a following `ON` selects the object-privilege grant and a bare `TO`
    /// the role-membership grant, mirroring PostgreSQL's reuse of one
    /// `privilege_list` production for both branches.
    fn parse_grant_kind(&mut self, start: Span) -> ParseResult<AccessControlStatement<D::Ext>> {
        if self
            .features()
            .access_control_syntax
            .access_control_account_grants
        {
            return self.parse_account_grant(start);
        }
        // `ALL [PRIVILEGES]` is valid only as an object-privilege grant.
        if self.peek_is_contextual_keyword("ALL")? {
            let privileges = self.parse_all_privileges()?;
            return self.finish_privilege_grant(privileges, start);
        }
        let list_start = self.current_span()?;
        let elements = self.parse_grant_element_list()?;
        if self.peek_is_contextual_keyword("ON")? {
            let privileges = self.privileges_from_elements(elements, list_start);
            self.finish_privilege_grant(privileges, start)
        } else if self.eat_contextual_keyword("TO")? {
            let roles = self.roles_from_elements(elements)?;
            let grantees = self.parse_grantee_list()?;
            let with_admin_option = self.parse_with_option("ADMIN")?;
            let granted_by = self.parse_granted_by()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(AccessControlStatement::GrantRole {
                roles,
                grantees,
                with_admin_option,
                granted_by,
                meta,
            })
        } else {
            Err(self.unexpected("`ON` or `TO`"))
        }
    }

    /// Finish an object-privilege `GRANT` once its privilege list is known. The
    /// `object … TO grantees [WITH GRANT OPTION] [GRANTED BY]` tail is shared by the
    /// `ALL [PRIVILEGES]` and explicit-list forms; only the privilege source differs.
    fn finish_privilege_grant(
        &mut self,
        privileges: Privileges,
        start: Span,
    ) -> ParseResult<AccessControlStatement<D::Ext>> {
        let object = self.parse_grant_object()?;
        self.expect_contextual_keyword("TO")?;
        let grantees = self.parse_grantee_list()?;
        let with_grant_option = self.parse_with_option("GRANT")?;
        let granted_by = self.parse_granted_by()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(AccessControlStatement::Grant {
            privileges,
            object,
            grantees,
            with_grant_option,
            granted_by,
            meta,
        })
    }

    /// Parse a `REVOKE` body after the keyword.
    ///
    /// `GRANT OPTION FOR` forces the object-privilege form and `ADMIN OPTION FOR` the
    /// role-membership form; otherwise the trailing `ON` versus `FROM` decides, as in
    /// [`parse_grant_kind`](Self::parse_grant_kind).
    fn parse_revoke_kind(&mut self, start: Span) -> ParseResult<AccessControlStatement<D::Ext>> {
        if self
            .features()
            .access_control_syntax
            .access_control_account_grants
        {
            return self.parse_account_revoke(start);
        }
        let grant_option_for = self.parse_option_for("GRANT")?;
        let admin_option_for = !grant_option_for && self.parse_option_for("ADMIN")?;
        // MySQL has no `{GRANT | ADMIN} OPTION FOR` REVOKE prefix (engine-measured 1064); it
        // spells grant-option removal as the `GRANT OPTION` privilege in the list. Gated by
        // [`AccessControlSyntax::access_control_extended_objects`], alongside the schema-scoped
        // grant objects, since both are the extended standard/PostgreSQL surface MySQL lacks.
        if !self
            .features()
            .access_control_syntax
            .access_control_extended_objects
            && (grant_option_for || admin_option_for)
        {
            return Err(self.error_at(
                start,
                "a REVOKE with no `GRANT OPTION FOR` / `ADMIN OPTION FOR` prefix: this \
                 dialect has no such prefix",
                self.span_text(start).to_owned(),
            ));
        }
        let list_start = self.current_span()?;
        let elements = self.parse_grant_element_list()?;
        if !admin_option_for && self.peek_is_contextual_keyword("ON")? {
            let privileges = self.privileges_from_elements(elements, list_start);
            let object = self.parse_grant_object()?;
            self.expect_contextual_keyword("FROM")?;
            let grantees = self.parse_grantee_list()?;
            let granted_by = self.parse_granted_by()?;
            let behavior = self.parse_revoke_behavior()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(AccessControlStatement::Revoke {
                grant_option_for,
                privileges,
                object,
                grantees,
                granted_by,
                behavior,
                meta,
            })
        } else if !grant_option_for && self.eat_contextual_keyword("FROM")? {
            let roles = self.roles_from_elements(elements)?;
            let grantees = self.parse_grantee_list()?;
            let granted_by = self.parse_granted_by()?;
            let behavior = self.parse_revoke_behavior()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(AccessControlStatement::RevokeRole {
                admin_option_for,
                roles,
                grantees,
                granted_by,
                behavior,
                meta,
            })
        } else if grant_option_for {
            Err(self.unexpected("`ON`"))
        } else if admin_option_for {
            Err(self.unexpected("`FROM`"))
        } else {
            Err(self.unexpected("`ON` or `FROM`"))
        }
    }

    // --- MySQL account-based GRANT/REVOKE -----------------------------------
    //
    // Reached only under
    // [`AccessControlSyntax::access_control_account_grants`](crate::ast::dialect::AccessControlSyntax)
    // (MySQL). The object is a `priv_level` and every grantee/role is an [`AccountName`]; see the
    // `AccessControlStatement::MySql*` variants for the grammar map.

    /// Parse a MySQL `GRANT` body after the keyword.
    fn parse_account_grant(&mut self, start: Span) -> ParseResult<AccessControlStatement<D::Ext>> {
        if self.eat_contextual_keyword("PROXY")? {
            self.expect_contextual_keyword("ON")?;
            let proxy = self.parse_account_name()?;
            self.expect_contextual_keyword("TO")?;
            let grantees = self.parse_comma_separated(Self::parse_account_name)?;
            let with_grant_option = self.parse_with_option("GRANT")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AccessControlStatement::AccountGrantProxy {
                proxy,
                grantees,
                with_grant_option,
                meta,
            });
        }
        // `ALL [PRIVILEGES]` is valid only as an object-privilege grant.
        if self.peek_is_contextual_keyword("ALL")? {
            let privileges = self.parse_all_privileges()?;
            return self.finish_account_privilege_grant(privileges, start);
        }
        let list_start = self.current_span()?;
        let elements = self.parse_comma_separated(Self::parse_account_grant_element)?;
        if self.peek_is_contextual_keyword("ON")? {
            let privileges = self.account_privileges_from_elements(elements, list_start)?;
            self.finish_account_privilege_grant(privileges, start)
        } else if self.eat_contextual_keyword("TO")? {
            let roles = self.account_roles_from_elements(elements)?;
            let grantees = self.parse_comma_separated(Self::parse_account_name)?;
            let with_admin_option = self.parse_with_option("ADMIN")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(AccessControlStatement::AccountGrantRole {
                roles,
                grantees,
                with_admin_option,
                meta,
            })
        } else {
            Err(self.unexpected("`ON` or `TO`"))
        }
    }

    /// Finish a MySQL object-privilege `GRANT` once its privilege list is known — the shared
    /// `ON <object> TO <grantees> [WITH GRANT OPTION] [AS <user> [WITH ROLE …]]` tail.
    fn finish_account_privilege_grant(
        &mut self,
        privileges: Privileges,
        start: Span,
    ) -> ParseResult<AccessControlStatement<D::Ext>> {
        let object = self.parse_privilege_level_object()?;
        self.expect_contextual_keyword("TO")?;
        let grantees = self.parse_comma_separated(Self::parse_account_name)?;
        let with_grant_option = self.parse_with_option("GRANT")?;
        let grant_as = self.parse_grant_as()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(AccessControlStatement::AccountGrantPrivilege {
            privileges,
            object,
            grantees,
            with_grant_option,
            grant_as,
            meta,
        })
    }

    /// Parse a MySQL `REVOKE` body after the keyword.
    fn parse_account_revoke(&mut self, start: Span) -> ParseResult<AccessControlStatement<D::Ext>> {
        let if_exists = self.parse_account_if_exists()?;
        if self.eat_contextual_keyword("PROXY")? {
            self.expect_contextual_keyword("ON")?;
            let proxy = self.parse_account_name()?;
            self.expect_contextual_keyword("FROM")?;
            let grantees = self.parse_comma_separated(Self::parse_account_name)?;
            let ignore_unknown_user = self.parse_ignore_unknown_user()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AccessControlStatement::AccountRevokeProxy {
                if_exists,
                proxy,
                grantees,
                ignore_unknown_user,
                meta,
            });
        }
        if self.peek_is_contextual_keyword("ALL")? {
            let privileges = self.parse_all_privileges()?;
            // `REVOKE ALL [PRIVILEGES], GRANT OPTION FROM …` — the global "revoke everything"
            // form, distinguished from the object form by the comma (it takes no `ON` object).
            if self.eat_punct(Punctuation::Comma)? {
                self.expect_contextual_keyword("GRANT")?;
                self.expect_contextual_keyword("OPTION")?;
                self.expect_contextual_keyword("FROM")?;
                let grantees = self.parse_comma_separated(Self::parse_account_name)?;
                let ignore_unknown_user = self.parse_ignore_unknown_user()?;
                let privileges_keyword = matches!(
                    privileges,
                    Privileges::All {
                        privileges_keyword: true,
                        ..
                    }
                );
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(AccessControlStatement::AccountRevokeAll {
                    if_exists,
                    privileges_keyword,
                    grantees,
                    ignore_unknown_user,
                    meta,
                });
            }
            return self.finish_account_privilege_revoke(if_exists, privileges, start);
        }
        let list_start = self.current_span()?;
        let elements = self.parse_comma_separated(Self::parse_account_grant_element)?;
        if self.peek_is_contextual_keyword("ON")? {
            let privileges = self.account_privileges_from_elements(elements, list_start)?;
            self.finish_account_privilege_revoke(if_exists, privileges, start)
        } else if self.eat_contextual_keyword("FROM")? {
            let roles = self.account_roles_from_elements(elements)?;
            let grantees = self.parse_comma_separated(Self::parse_account_name)?;
            let ignore_unknown_user = self.parse_ignore_unknown_user()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(AccessControlStatement::AccountRevokeRole {
                if_exists,
                roles,
                grantees,
                ignore_unknown_user,
                meta,
            })
        } else {
            Err(self.unexpected("`ON` or `FROM`"))
        }
    }

    /// Finish a MySQL object-privilege `REVOKE` — the shared `ON <object> FROM <grantees>
    /// [IGNORE UNKNOWN USER]` tail.
    fn finish_account_privilege_revoke(
        &mut self,
        if_exists: bool,
        privileges: Privileges,
        start: Span,
    ) -> ParseResult<AccessControlStatement<D::Ext>> {
        let object = self.parse_privilege_level_object()?;
        self.expect_contextual_keyword("FROM")?;
        let grantees = self.parse_comma_separated(Self::parse_account_name)?;
        let ignore_unknown_user = self.parse_ignore_unknown_user()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(AccessControlStatement::AccountRevokePrivilege {
            if_exists,
            privileges,
            object,
            grantees,
            ignore_unknown_user,
            meta,
        })
    }

    /// Parse one MySQL `role_or_privilege` element: a static-privilege phrase, or an account
    /// (dynamic-privilege name, or a `role` / `role@host`), each with an optional column scope.
    fn parse_account_grant_element(&mut self) -> ParseResult<AccountGrantElement> {
        let start = self.current_span()?;
        if let Some(kind) = self.try_parse_static_privilege()? {
            let columns = if static_privilege_takes_columns(kind) {
                self.parse_optional_column_list()?
            } else {
                ThinVec::new()
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AccountGrantElement {
                kind: Some(kind),
                account: None,
                columns,
                meta,
            });
        }
        let account = self.parse_account_name()?;
        let columns = self.parse_optional_column_list()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(AccountGrantElement {
            kind: None,
            account: Some(account),
            columns,
            meta,
        })
    }

    /// Consume a MySQL static-privilege keyword phrase, longest-first (see
    /// [`ACCOUNT_STATIC_PRIVILEGES`]).
    fn try_parse_static_privilege(&mut self) -> ParseResult<Option<PrivilegeKind>> {
        for &(words, kind) in ACCOUNT_STATIC_PRIVILEGES {
            let mut all_match = true;
            for (offset, word) in words.iter().enumerate() {
                if !self.peek_nth_is_contextual_keyword(offset, word)? {
                    all_match = false;
                    break;
                }
            }
            if all_match {
                for _ in words {
                    self.advance()?;
                }
                return Ok(Some(kind));
            }
        }
        Ok(None)
    }

    /// Reinterpret a MySQL element list as a privilege list (the `ON` branch). A static-privilege
    /// element becomes a [`Privilege::Known`]; a bare-name account becomes a [`Privilege::Other`]
    /// (a dynamic privilege). A `role@host` or `CURRENT_USER` element is not a privilege.
    fn account_privileges_from_elements(
        &mut self,
        elements: ThinVec<AccountGrantElement>,
        list_start: Span,
    ) -> ParseResult<Privileges> {
        let mut privileges = ThinVec::with_capacity(elements.len());
        for element in elements {
            let privilege = match (element.kind, element.account) {
                (Some(kind), _) => Privilege::Known {
                    kind,
                    columns: element.columns,
                    meta: element.meta,
                },
                (
                    None,
                    Some(AccountName::Account {
                        user, host: None, ..
                    }),
                ) => Privilege::Other {
                    name: user,
                    columns: element.columns,
                    meta: element.meta,
                },
                _ => return Err(self.unexpected("a privilege name")),
            };
            privileges.push(privilege);
        }
        let meta = self.make_meta(list_start.union(self.preceding_span()));
        Ok(Privileges::List { privileges, meta })
    }

    /// Reinterpret a MySQL element list as granted role accounts (the bare `TO`/`FROM` branch).
    /// A privilege keyword or a column scope is not a role.
    fn account_roles_from_elements(
        &mut self,
        elements: ThinVec<AccountGrantElement>,
    ) -> ParseResult<ThinVec<AccountName>> {
        let mut roles = ThinVec::with_capacity(elements.len());
        for element in elements {
            if element.kind.is_some() {
                return Err(self.unexpected("a role name, not a privilege keyword"));
            }
            if !element.columns.is_empty() {
                return Err(self.unexpected("a role name without a column list"));
            }
            let account = element
                .account
                .expect("a MySQL grant element carries a privilege kind or an account");
            roles.push(account);
        }
        Ok(roles)
    }

    /// Parse a MySQL grant object: `ON [TABLE | FUNCTION | PROCEDURE] <priv_level>`.
    fn parse_privilege_level_object(&mut self) -> ParseResult<PrivilegeLevelObject> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("ON")?;
        let object_type = if self.eat_contextual_keyword("TABLE")? {
            PrivilegeObjectType::Table { explicit: true }
        } else if self.eat_contextual_keyword("FUNCTION")? {
            PrivilegeObjectType::Function
        } else if self.eat_contextual_keyword("PROCEDURE")? {
            PrivilegeObjectType::Procedure
        } else {
            PrivilegeObjectType::Table { explicit: false }
        };
        let level = self.parse_privilege_level()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(PrivilegeLevelObject {
            object_type,
            level,
            meta,
        })
    }

    /// Parse a MySQL `priv_level`: `*`, `*.*`, `<db>.*`, `<obj>`, or `<db>.<obj>`.
    fn parse_privilege_level(&mut self) -> ParseResult<PrivilegeLevel> {
        let start = self.current_span()?;
        if self.eat_op(Operator::Star)? {
            if self.eat_punct(Punctuation::Dot)? {
                if !self.eat_op(Operator::Star)? {
                    return Err(self.unexpected("`*` after `*.` in a MySQL priv_level"));
                }
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(PrivilegeLevel::Global { meta });
            }
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(PrivilegeLevel::CurrentDatabase { meta });
        }
        let first = self.parse_ident()?;
        if self.eat_punct(Punctuation::Dot)? {
            if self.eat_op(Operator::Star)? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(PrivilegeLevel::Database {
                    database: first,
                    meta,
                });
            }
            let second = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(PrivilegeLevel::Object {
                name: ObjectName(thin_vec![first, second]),
                meta,
            });
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(PrivilegeLevel::Object {
            name: ObjectName(thin_vec![first]),
            meta,
        })
    }

    /// Parse the optional MySQL `AS <user> [WITH ROLE …]` grantor-context clause.
    fn parse_grant_as(&mut self) -> ParseResult<Option<Box<GrantAs>>> {
        let start = self.current_span()?;
        if !self.eat_contextual_keyword("AS")? {
            return Ok(None);
        }
        let user = self.parse_account_name()?;
        let with_role = self.parse_with_role_spec()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(Box::new(GrantAs {
            user,
            with_role,
            meta,
        })))
    }

    /// Parse the optional `WITH ROLE { <role> [, …] | ALL [EXCEPT <role> [, …]] | NONE | DEFAULT }`
    /// restriction of a MySQL `AS` clause.
    fn parse_with_role_spec(&mut self) -> ParseResult<Option<WithRoleSpec>> {
        let start = self.current_span()?;
        if !self.eat_contextual_keyword("WITH")? {
            return Ok(None);
        }
        self.expect_contextual_keyword("ROLE")?;
        let spec = if self.eat_contextual_keyword("ALL")? {
            let except = if self.eat_contextual_keyword("EXCEPT")? {
                self.parse_comma_separated(Self::parse_account_name)?
            } else {
                ThinVec::new()
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            WithRoleSpec::All { except, meta }
        } else if self.eat_contextual_keyword("NONE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            WithRoleSpec::None { meta }
        } else if self.eat_contextual_keyword("DEFAULT")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            WithRoleSpec::Default { meta }
        } else {
            let roles = self.parse_comma_separated(Self::parse_account_name)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            WithRoleSpec::Roles { roles, meta }
        };
        Ok(Some(spec))
    }

    /// Parse the optional MySQL `IGNORE UNKNOWN USER` trailer on a `REVOKE`.
    fn parse_ignore_unknown_user(&mut self) -> ParseResult<bool> {
        if self.peek_is_contextual_keyword("IGNORE")?
            && self.peek_nth_is_contextual_keyword(1, "UNKNOWN")?
        {
            self.expect_contextual_keyword("IGNORE")?;
            self.expect_contextual_keyword("UNKNOWN")?;
            self.expect_contextual_keyword("USER")?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Parse a `{GRANT | ADMIN} OPTION FOR` prefix on a `REVOKE`.
    ///
    /// Only consumed when the full three-word prefix is present, so a following
    /// privilege/role list whose first word happens to be `GRANT`/`ADMIN` is never
    /// mistaken for it.
    fn parse_option_for(&mut self, keyword: &'static str) -> ParseResult<bool> {
        if self.peek_is_contextual_keyword(keyword)?
            && self.peek_nth_is_contextual_keyword(1, "OPTION")?
        {
            self.expect_contextual_keyword(keyword)?;
            self.expect_contextual_keyword("OPTION")?;
            self.expect_contextual_keyword("FOR")?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Parse a `WITH <middle> OPTION` trailer on a `GRANT`: `GRANT` for a privilege
    /// grant, `ADMIN` for a role-membership grant. Mirrors the REVOKE-side
    /// [`parse_option_for`](Self::parse_option_for) parameterization.
    fn parse_with_option(&mut self, middle: &'static str) -> ParseResult<bool> {
        if self.eat_contextual_keyword("WITH")? {
            self.expect_contextual_keyword(middle)?;
            self.expect_contextual_keyword("OPTION")?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Parse an optional `GRANTED BY <grantor>` trailer shared by both grant forms.
    fn parse_granted_by(&mut self) -> ParseResult<Option<RoleSpec>> {
        if self.eat_contextual_keyword("GRANTED")? {
            self.expect_contextual_keyword("BY")?;
            Ok(Some(self.parse_role_spec()?))
        } else {
            Ok(None)
        }
    }

    /// Parse a trailing `CASCADE` / `RESTRICT` on a `REVOKE`. Ungated, unlike the
    /// `DROP` behaviour: the revoke `<drop behavior>` is core SQL on every dialect,
    /// not a schema-change extension.
    fn parse_revoke_behavior(&mut self) -> ParseResult<Option<DropBehavior>> {
        if self.eat_contextual_keyword("CASCADE")? {
            Ok(Some(DropBehavior::Cascade))
        } else if self.eat_contextual_keyword("RESTRICT")? {
            Ok(Some(DropBehavior::Restrict))
        } else {
            Ok(None)
        }
    }

    /// Parse `ALL [PRIVILEGES]` (the `PRIVILEGES` noise word is optional).
    fn parse_all_privileges(&mut self) -> ParseResult<Privileges> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("ALL")?;
        let privileges_keyword = self.eat_contextual_keyword("PRIVILEGES")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Privileges::All {
            privileges_keyword,
            meta,
        })
    }

    /// Parse the leading comma list of privilege/role words.
    fn parse_grant_element_list(&mut self) -> ParseResult<ThinVec<GrantElement>> {
        let elements = self.parse_comma_separated(Self::parse_grant_element)?;
        Ok(elements)
    }

    fn parse_grant_element(&mut self) -> ParseResult<GrantElement> {
        let start = self.current_span()?;
        let (word, kind) = self.parse_privilege_or_role_word()?;
        let columns = self.parse_optional_column_list()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(GrantElement {
            word,
            kind,
            columns,
            meta,
        })
    }

    /// Parse one privilege/role word, classifying it as a built-in privilege when the
    /// spelling matches.
    ///
    /// Built-in privilege keywords — including the reserved `SELECT`, `REFERENCES`,
    /// and `CREATE` — are matched by spelling so they are admitted here despite being
    /// reserved elsewhere; any other word is a `ColId` identifier, which covers
    /// dialect privileges and role names alike. This mirrors PostgreSQL's `privilege`
    /// production (`SELECT | REFERENCES | CREATE | ColId`).
    fn parse_privilege_or_role_word(&mut self) -> ParseResult<(Ident, Option<PrivilegeKind>)> {
        if let Some((word, kind)) = self.try_parse_known_privilege()? {
            Ok((word, Some(kind)))
        } else {
            Ok((self.parse_ident()?, None))
        }
    }

    /// Consume a built-in privilege keyword by spelling, returning its source
    /// identifier (for the role-membership reinterpretation) and classification.
    fn try_parse_known_privilege(&mut self) -> ParseResult<Option<(Ident, PrivilegeKind)>> {
        const KNOWN: &[(&str, PrivilegeKind)] = &[
            ("SELECT", PrivilegeKind::Select),
            ("INSERT", PrivilegeKind::Insert),
            ("UPDATE", PrivilegeKind::Update),
            ("DELETE", PrivilegeKind::Delete),
            ("TRUNCATE", PrivilegeKind::Truncate),
            ("REFERENCES", PrivilegeKind::References),
            ("TRIGGER", PrivilegeKind::Trigger),
            ("USAGE", PrivilegeKind::Usage),
            ("EXECUTE", PrivilegeKind::Execute),
            ("CREATE", PrivilegeKind::Create),
            ("CONNECT", PrivilegeKind::Connect),
            ("TEMPORARY", PrivilegeKind::Temporary),
            ("TEMP", PrivilegeKind::Temp),
            ("MAINTAIN", PrivilegeKind::Maintain),
        ];
        for &(spelling, kind) in KNOWN {
            if self.peek_is_contextual_keyword(spelling)? {
                let token = self.peek()?.expect("peek matched a contextual keyword");
                self.advance()?;
                let word = Ident {
                    sym: self.intern_identifier(token),
                    quote: QuoteStyle::None,
                    meta: self.make_meta(token.span),
                };
                return Ok(Some((word, kind)));
            }
        }
        Ok(None)
    }

    /// Parse an optional `( <column> [, ...] )` privilege column scope.
    fn parse_optional_column_list(&mut self) -> ParseResult<ThinVec<Ident>> {
        if self.eat_punct(Punctuation::LParen)? {
            let columns = self.parse_comma_separated(Self::parse_ident)?;
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the privilege column list",
            )?;
            Ok(columns)
        } else {
            Ok(ThinVec::new())
        }
    }

    /// Reinterpret the leading list as a privilege list (the `ON` branch).
    fn privileges_from_elements(
        &mut self,
        elements: ThinVec<GrantElement>,
        list_start: Span,
    ) -> Privileges {
        let privileges = elements
            .into_iter()
            .map(|element| match element.kind {
                Some(kind) => Privilege::Known {
                    kind,
                    columns: element.columns,
                    meta: element.meta,
                },
                None => Privilege::Other {
                    name: element.word,
                    columns: element.columns,
                    meta: element.meta,
                },
            })
            .collect();
        let meta = self.make_meta(list_start.union(self.preceding_span()));
        Privileges::List { privileges, meta }
    }

    /// Reinterpret the leading list as granted role names (the bare `TO`/`FROM`
    /// branch). A column scope is meaningless on a role and is rejected.
    fn roles_from_elements(
        &mut self,
        elements: ThinVec<GrantElement>,
    ) -> ParseResult<ThinVec<Ident>> {
        let mut roles = ThinVec::with_capacity(elements.len());
        for element in elements {
            if !element.columns.is_empty() {
                return Err(self.unexpected("a role name without a column list"));
            }
            roles.push(element.word);
        }
        Ok(roles)
    }

    /// Parse the object of a privilege grant: `ON [<object-type>] <target> [, ...]`.
    fn parse_grant_object(&mut self) -> ParseResult<GrantObject<D::Ext>> {
        self.expect_contextual_keyword("ON")?;
        // MySQL's grant object grammar is `ON [TABLE | FUNCTION | PROCEDURE] priv_level`; it
        // has no schema-scoped object (`ON SCHEMA s`, `ON DATABASE d`) nor the `ON ALL …
        // IN SCHEMA s` bulk form — `SCHEMA`/`DATABASE`/`ALL` there are engine-measured 1064.
        // Gated by [`AccessControlSyntax::access_control_extended_objects`]; `TABLE`/`FUNCTION`/
        // `PROCEDURE` and a bare/`db.tbl` target stay accepted (the non-reserved object words
        // like `SEQUENCE` fall through to a plain target, which MySQL binds, not rejects).
        if !self
            .features()
            .access_control_syntax
            .access_control_extended_objects
            && (self.peek_is_contextual_keyword("ALL")?
                || self.peek_is_contextual_keyword("SCHEMA")?
                || self.peek_is_contextual_keyword("DATABASE")?)
        {
            return Err(self.unexpected(
                "a table, function, or procedure grant object: this dialect has no \
                 schema-scoped (`ON SCHEMA`/`ON DATABASE`) or `ALL … IN SCHEMA` grant object",
            ));
        }
        let start = self.current_span()?;
        if self.eat_contextual_keyword("ALL")? {
            let kind = self.parse_schema_object_kind()?;
            self.expect_contextual_keyword("IN")?;
            self.expect_contextual_keyword("SCHEMA")?;
            let schemas = self.parse_object_name_list()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(GrantObject::AllInSchema {
                kind,
                schemas,
                meta,
            })
        } else if self.eat_contextual_keyword("TABLE")? {
            let names = self.parse_object_name_list()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(GrantObject::Table {
                explicit: true,
                names,
                meta,
            })
        } else if let Some(kind) = self.try_parse_named_object_kind()? {
            let names = self.parse_object_name_list()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(GrantObject::Named { kind, names, meta })
        } else if let Some(kind) = self.try_parse_routine_object_kind()? {
            let routines = self.parse_routine_signature_list()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(GrantObject::Routines {
                kind,
                routines,
                meta,
            })
        } else {
            // No object-type keyword: the default object type is a table.
            let names = self.parse_object_name_list()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(GrantObject::Table {
                explicit: false,
                names,
                meta,
            })
        }
    }

    fn parse_schema_object_kind(&mut self) -> ParseResult<SchemaObjectKind> {
        if self.eat_contextual_keyword("TABLES")? {
            Ok(SchemaObjectKind::Tables)
        } else if self.eat_contextual_keyword("SEQUENCES")? {
            Ok(SchemaObjectKind::Sequences)
        } else if self.eat_contextual_keyword("FUNCTIONS")? {
            Ok(SchemaObjectKind::Functions)
        } else if self.eat_contextual_keyword("PROCEDURES")? {
            Ok(SchemaObjectKind::Procedures)
        } else if self.eat_contextual_keyword("ROUTINES")? {
            Ok(SchemaObjectKind::Routines)
        } else {
            Err(self.unexpected("`TABLES`, `SEQUENCES`, `FUNCTIONS`, `PROCEDURES`, or `ROUTINES`"))
        }
    }

    fn try_parse_named_object_kind(&mut self) -> ParseResult<Option<NamedObjectKind>> {
        if self.eat_contextual_keyword("SEQUENCE")? {
            Ok(Some(NamedObjectKind::Sequence))
        } else if self.eat_contextual_keyword("DATABASE")? {
            Ok(Some(NamedObjectKind::Database))
        } else if self.eat_contextual_keyword("SCHEMA")? {
            Ok(Some(NamedObjectKind::Schema))
        } else if self.eat_contextual_keyword("DOMAIN")? {
            Ok(Some(NamedObjectKind::Domain))
        } else if self.eat_contextual_keyword("TYPE")? {
            Ok(Some(NamedObjectKind::Type))
        } else if self.eat_contextual_keyword("LANGUAGE")? {
            Ok(Some(NamedObjectKind::Language))
        } else if self.eat_contextual_keyword("TABLESPACE")? {
            Ok(Some(NamedObjectKind::Tablespace))
        } else if self.eat_contextual_keyword("FOREIGN")? {
            // `FOREIGN` is reserved, so it can only introduce one of these two types.
            if self.eat_contextual_keyword("DATA")? {
                self.expect_contextual_keyword("WRAPPER")?;
                Ok(Some(NamedObjectKind::ForeignDataWrapper))
            } else {
                self.expect_contextual_keyword("SERVER")?;
                Ok(Some(NamedObjectKind::ForeignServer))
            }
        } else {
            Ok(None)
        }
    }

    pub(super) fn try_parse_routine_object_kind(
        &mut self,
    ) -> ParseResult<Option<RoutineObjectKind>> {
        if self.eat_contextual_keyword("FUNCTION")? {
            Ok(Some(RoutineObjectKind::Function))
        } else if self.eat_contextual_keyword("PROCEDURE")? {
            Ok(Some(RoutineObjectKind::Procedure))
        } else if self.eat_contextual_keyword("ROUTINE")? {
            Ok(Some(RoutineObjectKind::Routine))
        } else {
            Ok(None)
        }
    }

    fn parse_object_name_list(&mut self) -> ParseResult<ThinVec<ObjectName>> {
        let names = self.parse_comma_separated(Self::parse_object_name)?;
        Ok(names)
    }

    pub(super) fn parse_routine_signature_list(
        &mut self,
    ) -> ParseResult<ThinVec<RoutineSignature<D::Ext>>> {
        let routines = self.parse_comma_separated(Self::parse_routine_signature)?;
        Ok(routines)
    }

    /// Parse one routine reference: a name with an optional argument-type list.
    ///
    /// Only the bare type list is modelled (`f(int, text)`); argument names and modes
    /// are out of scope. A missing list (`f`) and an empty list (`f()`) are kept
    /// distinct so they round-trip.
    fn parse_routine_signature(&mut self) -> ParseResult<RoutineSignature<D::Ext>> {
        let start = self.current_span()?;
        // A routine name is capped like a relation name: dialects without a catalog
        // qualifier (MySQL, SQLite) reject a three-part `a.b.c` routine name — MySQL
        // engine-measured `ER_PARSE_ERROR` on mysql:8 for `DROP FUNCTION a.b.c`.
        let name = self.parse_target_relation_name()?;
        // MySQL identifies a routine by name alone, so the `(<types>)` overload signature is
        // a syntax error there; with the gate off the `(` is left unconsumed and surfaces as
        // a clean parse error.
        let arg_types = if self.features().index_alter_syntax.routine_arg_types
            && self.eat_punct(Punctuation::LParen)?
        {
            let mut types = ThinVec::new();
            if !self.peek_is_punct(Punctuation::RParen)? {
                types.push(self.parse_data_type()?);
                while self.eat_punct(Punctuation::Comma)? {
                    types.push(self.parse_data_type()?);
                }
            }
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the routine argument list",
            )?;
            Some(types)
        } else {
            None
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(RoutineSignature {
            name,
            arg_types,
            meta,
        })
    }

    fn parse_grantee_list(&mut self) -> ParseResult<ThinVec<Grantee>> {
        let grantees = self.parse_comma_separated(Self::parse_grantee)?;
        Ok(grantees)
    }

    /// Parse one grantee: an optional legacy `GROUP` keyword then a role spec.
    fn parse_grantee(&mut self) -> ParseResult<Grantee> {
        let start = self.current_span()?;
        let group = self.eat_contextual_keyword("GROUP")?;
        let spec = self.parse_role_spec()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Grantee { group, spec, meta })
    }

    /// Parse a role specification: `PUBLIC`, a session-role pseudo-role, or a name.
    fn parse_role_spec(&mut self) -> ParseResult<RoleSpec> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("PUBLIC")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(RoleSpec::Public { meta })
        } else if self.eat_contextual_keyword("CURRENT_ROLE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(RoleSpec::CurrentRole { meta })
        } else if self.eat_contextual_keyword("CURRENT_USER")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(RoleSpec::CurrentUser { meta })
        } else if self.eat_contextual_keyword("SESSION_USER")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(RoleSpec::SessionUser { meta })
        } else {
            let name = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(RoleSpec::Name { name, meta })
        }
    }

    // --- User / role administration DDL (MySQL) -----------------------------

    /// Parse the tail of `CREATE USER …` — the `USER` keyword already consumed by the
    /// `CREATE` dispatcher. `[IF NOT EXISTS] <user> [<auth>] [, …] [DEFAULT ROLE …]
    /// [REQUIRE …] [WITH …] [<lock option> …] [COMMENT | ATTRIBUTE '…']`.
    pub(super) fn parse_create_user(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let if_not_exists = self.parse_account_if_not_exists()?;
        let users = self.parse_comma_separated(Self::parse_create_user_spec)?;
        let default_roles = self.parse_default_role_clause()?;
        let require = self.parse_require_clause()?;
        let resource_options = self.parse_connect_options()?;
        let password_lock_options = self.parse_password_lock_options()?;
        let attribute = self.parse_user_attribute()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::CreateUser {
            create: Box::new(CreateUser {
                if_not_exists,
                users,
                default_roles,
                require,
                resource_options,
                password_lock_options,
                attribute,
                meta,
            }),
            meta,
        })
    }

    /// Parse one `<user> [<auth>]` element of a `CREATE USER` list.
    fn parse_create_user_spec(&mut self) -> ParseResult<UserSpec> {
        let start = self.current_span()?;
        let account = self.parse_account_name()?;
        let auth = self.parse_optional_auth_option()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(UserSpec {
            account,
            auth,
            meta,
        })
    }

    /// Parse the tail of `ALTER USER …` — the `USER` keyword already consumed by the `ALTER`
    /// dispatcher. Either the `<user> DEFAULT ROLE {ALL | NONE | <role> [, …]}` single-account
    /// form, or the `<user> [<auth>] [REPLACE …] [RETAIN … | DISCARD …] [, …] [<tail>]` list form.
    pub(super) fn parse_alter_user(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let if_exists = self.parse_account_if_exists()?;
        let first_start = self.current_span()?;
        let first_account = self.parse_account_name()?;
        // `<user> DEFAULT ROLE …` — the single-account default-role reset. `DEFAULT` follows the
        // sole account here and nowhere in the list form (which continues with auth, a rotation
        // keyword, a comma, the option tail, or the statement end), so it disambiguates cleanly.
        if self.eat_contextual_keyword("DEFAULT")? {
            self.expect_contextual_keyword("ROLE")?;
            let roles = self.parse_default_role_target()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Statement::AlterUser {
                alter: Box::new(AlterUser::DefaultRole {
                    if_exists,
                    user: first_account,
                    roles,
                    meta,
                }),
                meta,
            });
        }
        let first_spec = self.parse_alter_user_spec_tail(first_start, first_account)?;
        let mut users = thin_vec![first_spec];
        while self.eat_punct(Punctuation::Comma)? {
            users.push(self.parse_alter_user_spec()?);
        }
        let require = self.parse_require_clause()?;
        let resource_options = self.parse_connect_options()?;
        let password_lock_options = self.parse_password_lock_options()?;
        let attribute = self.parse_user_attribute()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::AlterUser {
            alter: Box::new(AlterUser::Modify {
                if_exists,
                users,
                require,
                resource_options,
                password_lock_options,
                attribute,
                meta,
            }),
            meta,
        })
    }

    /// Parse one per-account element of an `ALTER USER` list.
    fn parse_alter_user_spec(&mut self) -> ParseResult<AlterUserSpec> {
        let start = self.current_span()?;
        let account = self.parse_account_name()?;
        self.parse_alter_user_spec_tail(start, account)
    }

    /// Parse the auth / password-rotation tail of an `ALTER USER` account element, given the
    /// already-parsed account and its start span.
    fn parse_alter_user_spec_tail(
        &mut self,
        start: Span,
        account: AccountName,
    ) -> ParseResult<AlterUserSpec> {
        let auth = self.parse_optional_auth_option()?;
        let replace = if self.eat_contextual_keyword("REPLACE")? {
            Some(self.expect_string_literal("the current password string after REPLACE")?)
        } else {
            None
        };
        let retain_current_password = if self.eat_contextual_keyword("RETAIN")? {
            self.expect_contextual_keyword("CURRENT")?;
            self.expect_contextual_keyword("PASSWORD")?;
            true
        } else {
            false
        };
        let discard_old_password = if self.eat_contextual_keyword("DISCARD")? {
            self.expect_contextual_keyword("OLD")?;
            self.expect_contextual_keyword("PASSWORD")?;
            true
        } else {
            false
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(AlterUserSpec {
            account,
            auth,
            replace,
            retain_current_password,
            discard_old_password,
            meta,
        })
    }

    /// Parse the tail of `DROP USER` / `CREATE ROLE` / `DROP ROLE` — the verb and its `USER`/
    /// `ROLE` keyword already consumed. `[<if-guard>] <name> [, …]`.
    pub(super) fn parse_user_role_list(
        &mut self,
        start: Span,
        kind: UserRoleListKind,
    ) -> ParseResult<Statement<D::Ext>> {
        let if_guard = match kind {
            UserRoleListKind::CreateRole => self.parse_account_if_not_exists()?,
            UserRoleListKind::DropUser | UserRoleListKind::DropRole => {
                self.parse_account_if_exists()?
            }
        };
        let names = self.parse_comma_separated(Self::parse_account_name)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::UserRoleList {
            statement: Box::new(UserRoleList {
                kind,
                if_guard,
                names,
                meta,
            }),
            meta,
        })
    }

    /// Parse an optional primary-factor authentication clause — `IDENTIFIED BY …` /
    /// `IDENTIFIED WITH <plugin> …`. `None` when no `IDENTIFIED` follows.
    fn parse_optional_auth_option(&mut self) -> ParseResult<Option<AuthOption>> {
        let start = self.current_span()?;
        if !self.eat_contextual_keyword("IDENTIFIED")? {
            return Ok(None);
        }
        if self.eat_contextual_keyword("BY")? {
            if self.eat_contextual_keyword("RANDOM")? {
                self.expect_contextual_keyword("PASSWORD")?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(Some(AuthOption::RandomPassword { meta }));
            }
            let password = self.expect_string_literal("a password string after IDENTIFIED BY")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(AuthOption::Password { password, meta }));
        }
        self.expect_contextual_keyword("WITH")?;
        let plugin = self.parse_ident_or_text()?;
        if self.eat_contextual_keyword("AS")? {
            let auth_string =
                self.expect_string_literal("an authentication string after IDENTIFIED WITH … AS")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(AuthOption::PluginAs {
                plugin,
                auth_string,
                meta,
            }));
        }
        if self.eat_contextual_keyword("BY")? {
            if self.eat_contextual_keyword("RANDOM")? {
                self.expect_contextual_keyword("PASSWORD")?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(Some(AuthOption::PluginByRandomPassword { plugin, meta }));
            }
            let password =
                self.expect_string_literal("a password string after IDENTIFIED WITH … BY")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(AuthOption::PluginByPassword {
                plugin,
                password,
                meta,
            }));
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(AuthOption::Plugin { plugin, meta }))
    }

    /// Parse an optional `DEFAULT ROLE <role> [, …]` clause (CREATE USER); empty when absent.
    fn parse_default_role_clause(&mut self) -> ParseResult<ThinVec<AccountName>> {
        if !self.eat_contextual_keyword("DEFAULT")? {
            return Ok(ThinVec::new());
        }
        self.expect_contextual_keyword("ROLE")?;
        self.parse_comma_separated(Self::parse_account_name)
    }

    /// Parse an `ALTER USER … DEFAULT ROLE` target — `ALL`, `NONE`, or a role list.
    fn parse_default_role_target(&mut self) -> ParseResult<DefaultRoleTarget> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("ALL")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(DefaultRoleTarget::All { meta });
        }
        if self.eat_contextual_keyword("NONE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(DefaultRoleTarget::None { meta });
        }
        let roles = self.parse_comma_separated(Self::parse_account_name)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(DefaultRoleTarget::Roles { roles, meta })
    }

    /// Parse an optional `REQUIRE {NONE | SSL | X509 | <tls option> [AND …]}` clause.
    fn parse_require_clause(&mut self) -> ParseResult<Option<TlsRequirement>> {
        let start = self.current_span()?;
        if !self.eat_contextual_keyword("REQUIRE")? {
            return Ok(None);
        }
        if self.eat_contextual_keyword("NONE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(TlsRequirement::None { meta }));
        }
        if self.eat_contextual_keyword("SSL")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(TlsRequirement::Ssl { meta }));
        }
        if self.eat_contextual_keyword("X509")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(TlsRequirement::X509 { meta }));
        }
        let mut options = thin_vec![self.parse_tls_option()?];
        loop {
            // Elements are joined by an optional `AND` (grammar `require_list_element opt_and
            // require_list`); a consumed `AND` demands a following element.
            let had_and = self.eat_contextual_keyword("AND")?;
            if self.peek_starts_tls_option()? {
                options.push(self.parse_tls_option()?);
            } else if had_and {
                return Err(self.unexpected("a TLS option (SUBJECT / ISSUER / CIPHER) after AND"));
            } else {
                break;
            }
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(TlsRequirement::Options { options, meta }))
    }

    /// True if the current token starts a `REQUIRE` certificate-attribute option.
    fn peek_starts_tls_option(&mut self) -> ParseResult<bool> {
        Ok(self.peek_is_contextual_keyword("SUBJECT")?
            || self.peek_is_contextual_keyword("ISSUER")?
            || self.peek_is_contextual_keyword("CIPHER")?)
    }

    /// Parse one `SUBJECT`/`ISSUER`/`CIPHER '<string>'` certificate-attribute requirement.
    fn parse_tls_option(&mut self) -> ParseResult<TlsOption> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("SUBJECT")? {
            let value = self.expect_string_literal("a subject string after SUBJECT")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(TlsOption::Subject { value, meta });
        }
        if self.eat_contextual_keyword("ISSUER")? {
            let value = self.expect_string_literal("an issuer string after ISSUER")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(TlsOption::Issuer { value, meta });
        }
        self.expect_contextual_keyword("CIPHER")?;
        let value = self.expect_string_literal("a cipher string after CIPHER")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(TlsOption::Cipher { value, meta })
    }

    /// Parse an optional `WITH <resource limit> …` clause; empty when no `WITH` follows. `WITH`
    /// introduces a whitespace-separated run of at least one limit.
    fn parse_connect_options(&mut self) -> ParseResult<ThinVec<ResourceLimit>> {
        if !self.eat_contextual_keyword("WITH")? {
            return Ok(ThinVec::new());
        }
        let mut options = ThinVec::new();
        while let Some(option) = self.parse_optional_resource_limit()? {
            options.push(option);
        }
        if options.is_empty() {
            return Err(self.unexpected("a resource limit (MAX_QUERIES_PER_HOUR, …) after WITH"));
        }
        Ok(options)
    }

    /// Parse one `WITH`-list resource limit, or `None` if the current token is not one.
    fn parse_optional_resource_limit(&mut self) -> ParseResult<Option<ResourceLimit>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("MAX_QUERIES_PER_HOUR")? {
            let value = self.expect_unsigned_integer_literal("a MAX_QUERIES_PER_HOUR value")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ResourceLimit::MaxQueriesPerHour { value, meta }));
        }
        if self.eat_contextual_keyword("MAX_UPDATES_PER_HOUR")? {
            let value = self.expect_unsigned_integer_literal("a MAX_UPDATES_PER_HOUR value")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ResourceLimit::MaxUpdatesPerHour { value, meta }));
        }
        if self.eat_contextual_keyword("MAX_CONNECTIONS_PER_HOUR")? {
            let value = self.expect_unsigned_integer_literal("a MAX_CONNECTIONS_PER_HOUR value")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ResourceLimit::MaxConnectionsPerHour { value, meta }));
        }
        if self.eat_contextual_keyword("MAX_USER_CONNECTIONS")? {
            let value = self.expect_unsigned_integer_literal("a MAX_USER_CONNECTIONS value")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ResourceLimit::MaxUserConnections { value, meta }));
        }
        Ok(None)
    }

    /// Parse the whitespace-separated run of password / account-lock options (possibly empty).
    fn parse_password_lock_options(&mut self) -> ParseResult<ThinVec<PasswordLockOption>> {
        let mut options = ThinVec::new();
        while let Some(option) = self.parse_optional_password_lock_option()? {
            options.push(option);
        }
        Ok(options)
    }

    /// Parse one password / account-lock option, or `None` if the current token is not one.
    fn parse_optional_password_lock_option(&mut self) -> ParseResult<Option<PasswordLockOption>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("ACCOUNT")? {
            if self.eat_contextual_keyword("LOCK")? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(Some(PasswordLockOption::AccountLock { meta }));
            }
            self.expect_contextual_keyword("UNLOCK")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(PasswordLockOption::AccountUnlock { meta }));
        }
        if self.eat_contextual_keyword("FAILED_LOGIN_ATTEMPTS")? {
            let count = self.expect_unsigned_integer_literal("a FAILED_LOGIN_ATTEMPTS value")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(PasswordLockOption::FailedLoginAttempts {
                count,
                meta,
            }));
        }
        if self.eat_contextual_keyword("PASSWORD_LOCK_TIME")? {
            if self.eat_contextual_keyword("UNBOUNDED")? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(Some(PasswordLockOption::PasswordLockTimeUnbounded { meta }));
            }
            let days = self.expect_unsigned_integer_literal("a PASSWORD_LOCK_TIME value")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(PasswordLockOption::PasswordLockTime { days, meta }));
        }
        if self.eat_contextual_keyword("PASSWORD")? {
            return self.parse_password_management_option(start).map(Some);
        }
        Ok(None)
    }

    /// Parse the tail of a `PASSWORD …` management option (the `PASSWORD` keyword already
    /// consumed): `EXPIRE`, `HISTORY`, `REUSE INTERVAL`, or `REQUIRE CURRENT`.
    fn parse_password_management_option(&mut self, start: Span) -> ParseResult<PasswordLockOption> {
        if self.eat_contextual_keyword("EXPIRE")? {
            if self.eat_contextual_keyword("DEFAULT")? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(PasswordLockOption::PasswordExpireDefault { meta });
            }
            if self.eat_contextual_keyword("NEVER")? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(PasswordLockOption::PasswordExpireNever { meta });
            }
            if self.eat_contextual_keyword("INTERVAL")? {
                let days =
                    self.expect_unsigned_integer_literal("a PASSWORD EXPIRE INTERVAL value")?;
                self.expect_contextual_keyword("DAY")?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(PasswordLockOption::PasswordExpireInterval { days, meta });
            }
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(PasswordLockOption::PasswordExpire { meta });
        }
        if self.eat_contextual_keyword("HISTORY")? {
            if self.eat_contextual_keyword("DEFAULT")? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(PasswordLockOption::PasswordHistoryDefault { meta });
            }
            let count = self.expect_unsigned_integer_literal("a PASSWORD HISTORY value")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(PasswordLockOption::PasswordHistory { count, meta });
        }
        if self.eat_contextual_keyword("REUSE")? {
            self.expect_contextual_keyword("INTERVAL")?;
            if self.eat_contextual_keyword("DEFAULT")? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(PasswordLockOption::PasswordReuseIntervalDefault { meta });
            }
            let days = self.expect_unsigned_integer_literal("a PASSWORD REUSE INTERVAL value")?;
            self.expect_contextual_keyword("DAY")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(PasswordLockOption::PasswordReuseInterval { days, meta });
        }
        self.expect_contextual_keyword("REQUIRE")?;
        self.expect_contextual_keyword("CURRENT")?;
        if self.eat_contextual_keyword("DEFAULT")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(PasswordLockOption::PasswordRequireCurrentDefault { meta });
        }
        if self.eat_contextual_keyword("OPTIONAL")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(PasswordLockOption::PasswordRequireCurrentOptional { meta });
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(PasswordLockOption::PasswordRequireCurrent { meta })
    }

    /// Parse an optional `COMMENT '…'` / `ATTRIBUTE '…'` account attribute.
    fn parse_user_attribute(&mut self) -> ParseResult<Option<UserAttribute>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("COMMENT")? {
            let comment = self.expect_string_literal("a comment string after COMMENT")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(UserAttribute::Comment { comment, meta }));
        }
        if self.eat_contextual_keyword("ATTRIBUTE")? {
            let attribute =
                self.expect_string_literal("a JSON attribute string after ATTRIBUTE")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(UserAttribute::Attribute { attribute, meta }));
        }
        Ok(None)
    }

    /// Parse an ungated `IF NOT EXISTS` guard for the account-management DDL.
    fn parse_account_if_not_exists(&mut self) -> ParseResult<bool> {
        if !self.eat_contextual_keyword("IF")? {
            return Ok(false);
        }
        self.expect_contextual_keyword("NOT")?;
        self.expect_contextual_keyword("EXISTS")?;
        Ok(true)
    }

    /// Parse an ungated `IF EXISTS` guard for the account-management DDL.
    fn parse_account_if_exists(&mut self) -> ParseResult<bool> {
        if !self.eat_contextual_keyword("IF")? {
            return Ok(false);
        }
        self.expect_contextual_keyword("EXISTS")?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::dialect::{
        ExpressionSyntax, FeatureDelta, FeatureSet, SessionVariableSyntax, ShowSyntax,
    };
    use crate::ast::{
        AccessControlStatement, AccountName, AlterUser, AuthOption, CharacterSetKeyword,
        ConfigParameter, ConstraintCheckTime, ConstraintsTarget, Expr, GrantObject, Grantee,
        InstallComponentSetScope, InstallComponentSetValue, InstallStatement, NamedObjectKind,
        Privilege, PrivilegeKind, PrivilegeLevel, PrivilegeObjectType, Privileges, Resolver as _,
        RoleSpec, RoutineObjectKind, SchemaObjectKind, SessionStatement, SetAssignment,
        SetCharacterSetValue, SetNamesValue, SetParameterValue, SetScope, SetValue,
        SetVariableAssignment, SetVariableKeyword, SetVariableValue, SpecialSetValue, Statement,
        SystemVariableScope, SystemVariableScopeKind, UninstallStatement, UserRoleListKind,
        WithRoleSpec,
    };
    use crate::dialect::{DuckDb, Lenient, MySql, Postgres};
    use crate::parser::{Dialect, FeatureDialect, Parsed, TestDialect, parse_with};
    use crate::render::Renderer;

    fn parse_one(sql: &str) -> Parsed {
        parse_with(sql, crate::ParseConfig::new(TestDialect))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"))
    }

    fn parse_session(sql: &str) -> SessionStatement {
        let parsed = parse_one(sql);
        let [Statement::Session { session, .. }] = parsed.statements() else {
            panic!(
                "{sql:?} did not parse to one session statement: {:?}",
                parsed.statements(),
            );
        };
        (**session).clone()
    }

    fn parse_access(sql: &str) -> AccessControlStatement {
        let parsed = parse_one(sql);
        let [Statement::AccessControl { access, .. }] = parsed.statements() else {
            panic!(
                "{sql:?} did not parse to one access-control statement: {:?}",
                parsed.statements(),
            );
        };
        (**access).clone()
    }

    /// The dispatch contract: the session (`SET`/`RESET`/`SHOW`) and access
    /// control (`GRANT`/`REVOKE`) keywords are routed by the central `parse_statement`
    /// to this module's two entries, yielding `Statement::Session` and
    /// `Statement::AccessControl` (the helpers panic on any other variant).
    /// `SET TRANSACTION` is deliberately absent: transaction control claims it first
    /// (see `tcl`), so only the bare `SET` forms route to the session entry.
    #[test]
    fn dispatch_routes_dcl_keywords_to_this_family() {
        for sql in ["SET x = 1", "RESET ALL", "SHOW ALL"] {
            let _ = parse_session(sql);
        }
        for sql in [
            "GRANT mypriv ON t TO alice",
            "REVOKE SELECT ON t FROM alice",
        ] {
            let _ = parse_access(sql);
        }
    }

    #[test]
    fn set_captures_scope_name_and_value() {
        // `=` and `TO` are interchangeable; the scope is optional.
        let parsed = parse_one("SET search_path = public");
        let [Statement::Session { session, .. }] = parsed.statements() else {
            panic!("expected a session statement");
        };
        let SessionStatement::Set {
            scope, name, value, ..
        } = &**session
        else {
            panic!("expected SET");
        };
        assert!(scope.is_none());
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "search_path");
        let SetValue::Values { values, .. } = value else {
            panic!("expected a value list");
        };
        let [SetParameterValue::Name { name, .. }] = values.as_slice() else {
            panic!("expected one bareword value");
        };
        assert_eq!(parsed.resolver().resolve(name.sym), "public");
    }

    #[test]
    fn set_supports_scope_keyword_to_separator_and_default() {
        let SessionStatement::Set { scope, .. } = parse_session("SET SESSION x TO 1") else {
            panic!("expected SET");
        };
        assert_eq!(scope, Some(SetScope::Session));

        let SessionStatement::Set { scope, .. } = parse_session("SET LOCAL x = 1") else {
            panic!("expected SET");
        };
        assert_eq!(scope, Some(SetScope::Local));

        let SessionStatement::Set { value, .. } = parse_session("SET x TO DEFAULT") else {
            panic!("expected SET");
        };
        assert!(matches!(value, SetValue::Default { .. }));
    }

    #[test]
    fn set_value_lists_and_literals_parse() {
        // A comma-separated value list with a reserved-keyword bareword (`on`).
        let SessionStatement::Set { value, .. } = parse_session("SET search_path TO a, b") else {
            panic!("expected SET");
        };
        let SetValue::Values { values, .. } = value else {
            panic!("expected a value list");
        };
        assert_eq!(values.len(), 2);

        let SessionStatement::Set { value, .. } = parse_session("SET statement_timeout = 100")
        else {
            panic!("expected SET");
        };
        assert!(matches!(
            value,
            SetValue::Values { values, .. } if matches!(
                values.as_slice(),
                [SetParameterValue::Literal { .. }],
            ),
        ));

        // `on` is a reserved keyword in our M1 set but a valid bareword SET value.
        assert!(matches!(
            parse_session("SET autocommit = on"),
            SessionStatement::Set { .. }
        ));
    }

    #[test]
    fn generic_set_values_follow_dialect_reserved_word_boundaries() {
        for sql in ["SET o = do", "SET o = select"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
                "PostgreSQL rejects a fully reserved keyword as a SET value: {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDB rejects a fully reserved keyword as a SET value: {sql:?}",
            );
        }

        for sql in ["SET o = true", "SET o = false", "SET o = off"] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(Postgres)).is_ok(),
                "PostgreSQL accepts {sql:?}",
            );
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDB accepts {sql:?}",
            );
        }

        assert!(parse_with("SET o = on", crate::ParseConfig::new(Postgres)).is_ok());
        assert!(parse_with("SET o = on", crate::ParseConfig::new(DuckDb)).is_err());
        assert!(parse_with("SET o = null", crate::ParseConfig::new(Postgres)).is_err());
        assert!(parse_with("SET o = null", crate::ParseConfig::new(DuckDb)).is_ok());
    }

    #[test]
    fn signed_numeric_set_values_parse() {
        // A leading sign binds to a numeric value (folded into the literal).
        for sql in ["SET x = -1", "SET x = +1", "SET x TO -2"] {
            let SessionStatement::Set {
                value: SetValue::Values { values, .. },
                ..
            } = parse_session(sql)
            else {
                panic!("{sql:?} should be a generic SET");
            };
            assert!(
                matches!(values.as_slice(), [SetParameterValue::Literal { .. }]),
                "{sql:?} values: {values:?}",
            );
        }
    }

    #[test]
    fn set_time_zone_parses_value_and_sentinels() {
        let SessionStatement::SetTimeZone { scope, value, .. } =
            parse_session("SET TIME ZONE 'UTC'")
        else {
            panic!("expected SET TIME ZONE");
        };
        assert!(scope.is_none());
        assert!(matches!(*value, SpecialSetValue::Value { .. }));

        let SessionStatement::SetTimeZone { value, .. } = parse_session("SET TIME ZONE LOCAL")
        else {
            panic!("expected SET TIME ZONE");
        };
        assert!(matches!(*value, SpecialSetValue::Local { .. }));

        // A scope qualifier may precede the two-word parameter.
        let SessionStatement::SetTimeZone { scope, value, .. } =
            parse_session("SET LOCAL TIME ZONE DEFAULT")
        else {
            panic!("expected SET TIME ZONE");
        };
        assert_eq!(scope, Some(SetScope::Local));
        assert!(matches!(*value, SpecialSetValue::Default { .. }));
    }

    #[test]
    fn set_role_and_session_authorization_distinguish_scope_from_form() {
        let SessionStatement::SetRole { role, .. } = parse_session("SET ROLE NONE") else {
            panic!("expected SET ROLE");
        };
        assert!(matches!(*role, SpecialSetValue::None { .. }));

        let SessionStatement::SetRole { role, .. } = parse_session("SET ROLE admin") else {
            panic!("expected SET ROLE");
        };
        assert!(matches!(*role, SpecialSetValue::Value { .. }));

        // `SESSION` here opens `SESSION AUTHORIZATION`; it is not a scope qualifier.
        let SessionStatement::SetSessionAuthorization { scope, user, .. } =
            parse_session("SET SESSION AUTHORIZATION bob")
        else {
            panic!("expected SET SESSION AUTHORIZATION");
        };
        assert!(scope.is_none());
        assert!(matches!(*user, SpecialSetValue::Value { .. }));

        let SessionStatement::SetSessionAuthorization { user, .. } =
            parse_session("SET SESSION AUTHORIZATION DEFAULT")
        else {
            panic!("expected SET SESSION AUTHORIZATION");
        };
        assert!(matches!(*user, SpecialSetValue::Default { .. }));

        // A real scope can still precede the form.
        assert!(matches!(
            parse_session("SET LOCAL SESSION AUTHORIZATION bob"),
            SessionStatement::SetSessionAuthorization {
                scope: Some(SetScope::Local),
                ..
            }
        ));
        // And a plain `SESSION` scope on a generic SET is unaffected.
        assert!(matches!(
            parse_session("SET SESSION x TO 1"),
            SessionStatement::Set {
                scope: Some(SetScope::Session),
                ..
            }
        ));
    }

    #[test]
    fn set_constraints_names_and_session_characteristics_parse() {
        assert!(matches!(
            parse_session("SET CONSTRAINTS ALL DEFERRED"),
            SessionStatement::SetConstraints {
                constraints: ConstraintsTarget::All { .. },
                check_time: ConstraintCheckTime::Deferred,
                ..
            }
        ));
        let SessionStatement::SetConstraints {
            constraints: ConstraintsTarget::Names { names, .. },
            check_time: ConstraintCheckTime::Immediate,
            ..
        } = parse_session("SET CONSTRAINTS a, b IMMEDIATE")
        else {
            panic!("expected a named SET CONSTRAINTS");
        };
        assert_eq!(names.len(), 2);

        let SessionStatement::SetNames { value, .. } = parse_session("SET NAMES DEFAULT") else {
            panic!("expected SET NAMES");
        };
        assert!(matches!(*value, SetNamesValue::Default { .. }));

        let SessionStatement::SetNames { value, .. } =
            parse_session("SET NAMES utf8 COLLATE utf8_bin")
        else {
            panic!("expected SET NAMES");
        };
        let SetNamesValue::Charset { collation, .. } = *value else {
            panic!("expected a charset value");
        };
        assert!(collation.is_some());

        let SessionStatement::SetSessionCharacteristics { modes, .. } =
            parse_session("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY, NOT DEFERRABLE")
        else {
            panic!("expected SET SESSION CHARACTERISTICS");
        };
        assert_eq!(modes.len(), 2);
    }

    #[test]
    fn reset_and_show_target_all_or_a_named_parameter() {
        assert!(matches!(
            parse_session("RESET ALL"),
            SessionStatement::Reset {
                target: ConfigParameter::All { .. },
                ..
            }
        ));
        assert!(matches!(
            parse_session("RESET search_path"),
            SessionStatement::Reset {
                target: ConfigParameter::Named { .. },
                ..
            }
        ));
        assert!(matches!(
            parse_session("SHOW ALL"),
            SessionStatement::Show {
                target: ConfigParameter::All { .. },
                ..
            }
        ));
        assert!(matches!(
            parse_session("SHOW search_path"),
            SessionStatement::Show {
                target: ConfigParameter::Named { .. },
                ..
            }
        ));
    }

    /// The built-in privilege kinds of a privilege list, panicking on an `Other`.
    fn known_kinds(privileges: &Privileges) -> Vec<PrivilegeKind> {
        let Privileges::List { privileges, .. } = privileges else {
            panic!("expected a privilege list");
        };
        privileges
            .iter()
            .map(|privilege| match privilege {
                Privilege::Known { kind, .. } => *kind,
                Privilege::Other { .. } => panic!("expected a known privilege"),
            })
            .collect()
    }

    #[test]
    fn grant_captures_privileges_object_and_grantees() {
        let parsed = parse_one("GRANT SELECT, INSERT ON t TO alice, bob");
        let [Statement::AccessControl { access, .. }] = parsed.statements() else {
            panic!("expected an access-control statement");
        };
        let AccessControlStatement::Grant {
            privileges,
            object,
            grantees,
            with_grant_option,
            granted_by,
            ..
        } = &**access
        else {
            panic!("expected GRANT");
        };
        assert_eq!(
            known_kinds(privileges),
            [PrivilegeKind::Select, PrivilegeKind::Insert],
        );
        let GrantObject::Table {
            explicit, names, ..
        } = object
        else {
            panic!("expected a table object");
        };
        assert!(!explicit);
        assert_eq!(names.len(), 1);
        assert_eq!(parsed.resolver().resolve(names[0].0[0].sym), "t");
        assert_eq!(grantees.len(), 2);
        assert!(!with_grant_option);
        assert!(granted_by.is_none());
    }

    #[test]
    fn grant_all_with_grant_option_and_explicit_table() {
        let AccessControlStatement::Grant {
            privileges,
            object,
            with_grant_option,
            ..
        } = parse_access("GRANT ALL PRIVILEGES ON TABLE t TO alice WITH GRANT OPTION")
        else {
            panic!("expected GRANT");
        };
        assert!(matches!(privileges, Privileges::All { .. }));
        assert!(matches!(object, GrantObject::Table { explicit: true, .. }));
        assert!(with_grant_option);
    }

    #[test]
    fn grant_supports_column_level_privileges() {
        let AccessControlStatement::Grant { privileges, .. } =
            parse_access("GRANT SELECT (a, b) ON t TO alice")
        else {
            panic!("expected GRANT");
        };
        let Privileges::List { privileges, .. } = privileges else {
            panic!("expected a privilege list");
        };
        let Privilege::Known { columns, .. } = &privileges[0] else {
            panic!("expected a known privilege");
        };
        assert_eq!(columns.len(), 2);
    }

    #[test]
    fn grant_non_table_privilege_kinds_and_other_escape() {
        // Non-table privilege keywords classify; an unknown identifier rides `Other`.
        let AccessControlStatement::Grant { privileges, .. } =
            parse_access("GRANT USAGE, EXECUTE ON SCHEMA s TO alice")
        else {
            panic!("expected GRANT");
        };
        assert_eq!(
            known_kinds(&privileges),
            [PrivilegeKind::Usage, PrivilegeKind::Execute],
        );

        let parsed = parse_one("GRANT mypriv ON t TO alice");
        let [Statement::AccessControl { access, .. }] = parsed.statements() else {
            panic!("expected an access-control statement");
        };
        let AccessControlStatement::Grant { privileges, .. } = &**access else {
            panic!("expected GRANT");
        };
        let Privileges::List { privileges, .. } = privileges else {
            panic!("expected a privilege list");
        };
        let Privilege::Other { name, .. } = &privileges[0] else {
            panic!("expected an `Other` privilege");
        };
        assert_eq!(parsed.resolver().resolve(name.sym), "mypriv");
    }

    #[test]
    fn grant_object_types_beyond_table() {
        assert!(matches!(
            parse_access("GRANT USAGE ON SEQUENCE s TO alice"),
            AccessControlStatement::Grant {
                object: GrantObject::Named {
                    kind: NamedObjectKind::Sequence,
                    ..
                },
                ..
            },
        ));
        assert!(matches!(
            parse_access("GRANT USAGE ON FOREIGN DATA WRAPPER w TO alice"),
            AccessControlStatement::Grant {
                object: GrantObject::Named {
                    kind: NamedObjectKind::ForeignDataWrapper,
                    ..
                },
                ..
            },
        ));
        assert!(matches!(
            parse_access("GRANT SELECT ON ALL TABLES IN SCHEMA s TO alice"),
            AccessControlStatement::Grant {
                object: GrantObject::AllInSchema {
                    kind: SchemaObjectKind::Tables,
                    ..
                },
                ..
            },
        ));
    }

    #[test]
    fn grant_routine_objects_keep_optional_signatures() {
        let AccessControlStatement::Grant {
            object: GrantObject::Routines { kind, routines, .. },
            ..
        } = parse_access("GRANT EXECUTE ON FUNCTION f(int, text), g TO alice")
        else {
            panic!("expected a routine grant");
        };
        assert_eq!(kind, RoutineObjectKind::Function);
        assert_eq!(routines.len(), 2);
        // `f(int, text)` keeps its argument-type list; the bare `g` records no list.
        assert_eq!(
            routines[0].arg_types.as_ref().map(|types| types.len()),
            Some(2usize),
        );
        assert!(routines[1].arg_types.is_none());
    }

    #[test]
    fn grant_grantee_kinds_and_granted_by() {
        let AccessControlStatement::Grant {
            grantees,
            granted_by,
            ..
        } = parse_access("GRANT SELECT ON t TO PUBLIC, GROUP admins, bob GRANTED BY CURRENT_USER")
        else {
            panic!("expected GRANT");
        };
        assert_eq!(grantees.len(), 3);
        assert!(matches!(
            grantees[0],
            Grantee {
                group: false,
                spec: RoleSpec::Public { .. },
                ..
            },
        ));
        assert!(matches!(
            grantees[1],
            Grantee {
                group: true,
                spec: RoleSpec::Name { .. },
                ..
            },
        ));
        assert!(matches!(granted_by, Some(RoleSpec::CurrentUser { .. })));
    }

    #[test]
    fn grant_role_membership_with_admin_option() {
        let AccessControlStatement::GrantRole {
            roles,
            grantees,
            with_admin_option,
            ..
        } = parse_access("GRANT admin, staff TO alice WITH ADMIN OPTION")
        else {
            panic!("expected a role-membership grant");
        };
        assert_eq!(roles.len(), 2);
        assert_eq!(grantees.len(), 1);
        assert!(with_admin_option);
    }

    #[test]
    fn grant_select_to_role_is_role_membership() {
        // PostgreSQL parity: with no `ON`, `GRANT SELECT TO alice` is a role grant
        // whose granted "role" is spelled like a privilege keyword.
        let parsed = parse_one("GRANT SELECT TO alice");
        let [Statement::AccessControl { access, .. }] = parsed.statements() else {
            panic!("expected an access-control statement");
        };
        let AccessControlStatement::GrantRole { roles, .. } = &**access else {
            panic!("expected a role-membership grant");
        };
        assert_eq!(roles.len(), 1);
        assert_eq!(parsed.resolver().resolve(roles[0].sym), "SELECT");
    }

    #[test]
    fn revoke_privilege_and_role_forms() {
        assert!(matches!(
            parse_access("REVOKE SELECT ON t FROM alice"),
            AccessControlStatement::Revoke {
                grant_option_for: false,
                ..
            }
        ));
        assert!(matches!(
            parse_access("REVOKE GRANT OPTION FOR INSERT ON t FROM alice"),
            AccessControlStatement::Revoke {
                grant_option_for: true,
                ..
            }
        ));
        assert!(matches!(
            parse_access("REVOKE admin FROM alice"),
            AccessControlStatement::RevokeRole {
                admin_option_for: false,
                ..
            }
        ));
        assert!(matches!(
            parse_access("REVOKE ADMIN OPTION FOR admin FROM alice"),
            AccessControlStatement::RevokeRole {
                admin_option_for: true,
                ..
            }
        ));
    }

    #[test]
    fn malformed_session_and_access_control_statements_are_rejected() {
        for sql in [
            "SET x",                                      // missing =/TO and value
            "SET x =",                                    // missing value
            "SET x = -",                                  // sign without a number
            "SET TIME ZONE",                              // missing value
            "SET CONSTRAINTS a",                          // missing DEFERRED / IMMEDIATE
            "SET SESSION CHARACTERISTICS AS TRANSACTION", // missing modes
            "GRANT SELECT ON t",                          // missing TO grantees
            "GRANT ALL TO alice", // `ALL` requires `ON <object>`, never a role grant
            "REVOKE SELECT ON t", // missing FROM grantees
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "{sql:?} should be rejected",
            );
        }
    }

    // --- SET <name> = [ ... ] list value (DuckDB) ---------------------------

    /// ANSI with only the `collection_literals` gate on, so the DuckDB `SET x = [ ... ]`
    /// list value is exercised in isolation; implements `RenderDialect` for round-trip.
    /// The same flag that makes `[` open a list rather than a quoted identifier is what
    /// admits the list value, so a single knob drives both readings.
    const SET_LIST_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
                collection_literals: true,
                ..ExpressionSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    fn parse_session_with<D: Dialect<Ext = crate::ast::NoExt>>(
        sql: &str,
        dialect: D,
    ) -> SessionStatement {
        let parsed = parse_with(sql, crate::ParseConfig::new(dialect))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let [Statement::Session { session, .. }] = parsed.statements() else {
            panic!(
                "{sql:?} did not parse to one session statement: {:?}",
                parsed.statements(),
            );
        };
        (**session).clone()
    }

    fn set_list_values(session: &SessionStatement) -> &[SetParameterValue] {
        let SessionStatement::Set {
            value: SetValue::Values { values, .. },
            ..
        } = session
        else {
            panic!("expected `SET <name> = <values>`, got {session:?}");
        };
        values
    }

    #[test]
    fn set_list_value_parses_and_captures_elements() {
        // A single bracketed list value (the corpus's `allowed_configs` setting).
        let session = parse_session_with(
            "SET allowed_configs = ['lock_configuration']",
            SET_LIST_DIALECT,
        );
        let values = set_list_values(&session);
        assert_eq!(values.len(), 1);
        let SetParameterValue::List { values: elems, .. } = &values[0] else {
            panic!("expected a list value, got {:?}", values[0]);
        };
        assert_eq!(elems.len(), 1);
        assert!(matches!(elems[0], SetParameterValue::Literal { .. }));

        // The empty list `[]` (the `allowed_directories` setting) yields an empty list.
        let session = parse_session_with("SET allowed_directories = []", SET_LIST_DIALECT);
        assert!(matches!(
            &set_list_values(&session)[0],
            SetParameterValue::List { values, .. } if values.is_empty()
        ));
    }

    #[test]
    fn set_list_value_is_gated_on_collection_literals() {
        // With `collection_literals` off (the ANSI/TestDialect baseline), `[` in SET
        // value position is a clean parse error ("expected a SET value, found ["), the
        // reading of every dialect that does not open a list with `[`; the same text
        // parses once the gate is on.
        assert!(parse_with("SET x = ['a']", crate::ParseConfig::new(TestDialect)).is_err());
        assert!(parse_with("SET x = ['a']", crate::ParseConfig::new(SET_LIST_DIALECT)).is_ok());
    }

    #[test]
    fn set_list_value_round_trips() {
        // The canonical render uses `TO` and a `[e, ...]` list; assert exact round-trip on
        // that form (including the empty list).
        for sql in [
            "SET allowed_paths TO ['a', 'b']",
            "SET allowed_directories TO []",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SET_LIST_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(SET_LIST_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?}: {err}"));
            assert_eq!(rendered, sql, "exact round-trip for {sql:?}");
        }
    }

    /// ANSI with only the planner `VERBOSE` tail enabled — session statements are already
    /// on in the ANSI baseline, so `SHOW` dispatches and the delta is just the tail.
    const SHOW_VERBOSE_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.show_syntax(ShowSyntax {
                show_verbose: true,
                ..ShowSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    #[test]
    fn show_verbose_tail_is_captured_on_both_targets_when_the_flag_is_on() {
        // The planner `VERBOSE` tail attaches to both `SHOW ALL` and `SHOW <setting>`;
        // an absent tail is `verbose = false`, not a distinct shape.
        assert!(matches!(
            parse_session_with("SHOW ALL VERBOSE", SHOW_VERBOSE_DIALECT),
            SessionStatement::Show {
                target: ConfigParameter::All { .. },
                verbose: true,
                ..
            }
        ));
        assert!(matches!(
            parse_session_with("SHOW work_mem VERBOSE", SHOW_VERBOSE_DIALECT),
            SessionStatement::Show {
                target: ConfigParameter::Named { .. },
                verbose: true,
                ..
            }
        ));
        assert!(matches!(
            parse_session_with("SHOW ALL", SHOW_VERBOSE_DIALECT),
            SessionStatement::Show { verbose: false, .. }
        ));
        assert!(matches!(
            parse_session_with("SHOW work_mem", SHOW_VERBOSE_DIALECT),
            SessionStatement::Show { verbose: false, .. }
        ));
    }

    #[test]
    fn show_is_byte_identical_when_the_verbose_flag_is_off() {
        // Flag off (the ANSI/TestDialect baseline): the trailing `VERBOSE` is never
        // consumed, so it surfaces as the same trailing-token error as before this flag,
        // while the bare `SHOW ALL`/`SHOW <var>` forms parse exactly as today —
        // `verbose = false` on the node.
        assert!(parse_with("SHOW ALL VERBOSE", crate::ParseConfig::new(TestDialect)).is_err());
        assert!(
            parse_with(
                "SHOW work_mem VERBOSE",
                crate::ParseConfig::new(TestDialect)
            )
            .is_err()
        );
        assert!(matches!(
            parse_session("SHOW ALL"),
            SessionStatement::Show {
                target: ConfigParameter::All { .. },
                verbose: false,
                ..
            }
        ));
        assert!(matches!(
            parse_session("SHOW search_path"),
            SessionStatement::Show {
                target: ConfigParameter::Named { .. },
                verbose: false,
                ..
            }
        ));
    }

    #[test]
    fn show_verbose_round_trips_exactly() {
        for sql in ["SHOW ALL VERBOSE", "SHOW work_mem VERBOSE", "SHOW ALL"] {
            let parsed = parse_with(sql, crate::ParseConfig::new(SHOW_VERBOSE_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(SHOW_VERBOSE_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?}: {err}"));
            assert_eq!(rendered, sql, "exact round-trip for {sql:?}");
        }
    }

    #[test]
    fn show_all_verbose_stays_session_show_and_never_collides_with_typed_show_all_tables() {
        // MECE seam under Lenient (all `show_*` gates on): the typed-`SHOW` lookaheads each
        // insist on their keyword after the `ALL` modifier, so `SHOW ALL VERBOSE` (no
        // `TABLES`) is the session `SHOW` with the planner tail, `SHOW ALL TABLES` is the
        // typed catalogue listing, and bare `SHOW ALL` keeps today's session reading.
        assert!(matches!(
            parse_session_with("SHOW ALL VERBOSE", Lenient),
            SessionStatement::Show {
                target: ConfigParameter::All { .. },
                verbose: true,
                ..
            }
        ));
        assert!(matches!(
            parse_session_with("SHOW ALL", Lenient),
            SessionStatement::Show {
                target: ConfigParameter::All { .. },
                verbose: false,
                ..
            }
        ));
        // `SHOW ALL TABLES` routes to the typed `Statement::Show`, not the session family.
        let parsed = parse_with("SHOW ALL TABLES", crate::ParseConfig::new(Lenient))
            .unwrap_or_else(|err| panic!("SHOW ALL TABLES: {err:?}"));
        assert!(
            matches!(parsed.statements(), [Statement::Show { .. }]),
            "SHOW ALL TABLES should be a typed SHOW, got {:?}",
            parsed.statements(),
        );
    }

    // --- User / role administration DDL (MySQL) -----------------------------

    const MYSQL_RENDER: FeatureDialect = FeatureDialect {
        features: &FeatureSet::MYSQL,
    };

    /// Parse `sql` under MySQL, render it back, and assert the render is byte-identical — the
    /// source-fidelity round-trip contract (quoting spellings, keyword casing preserved).
    fn mysql_round_trips(sql: &str) -> Parsed {
        use crate::dialect::MySql;
        let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
            .unwrap_or_else(|err| panic!("PARSE {sql:?}: {err:?}"));
        let rendered = Renderer::new(MYSQL_RENDER)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("RENDER {sql:?}: {err:?}"));
        assert_eq!(rendered, sql, "account DDL round-trip {sql:?}");
        parsed
    }

    // --- MySQL variable-assignment SET --------------------------------------

    /// Parse `sql` under MySQL to its single [`SessionStatement`].
    fn mysql_session(sql: &str) -> SessionStatement {
        let parsed = mysql_round_trips(sql);
        let [Statement::Session { session, .. }] = parsed.statements() else {
            panic!(
                "{sql:?} is not a session statement: {:?}",
                parsed.statements()
            );
        };
        (**session).clone()
    }

    /// The single [`SetVariableAssignment`] of a one-item `SET`.
    fn mysql_one_assignment(sql: &str) -> SetVariableAssignment {
        let SessionStatement::SetVariables { assignments, .. } = mysql_session(sql) else {
            panic!("{sql:?} is not a SetVariables statement");
        };
        assert_eq!(assignments.len(), 1, "{sql:?} has one assignment");
        assignments.into_iter().next().unwrap()
    }

    fn mysql_rejects(sql: &str) {
        use crate::dialect::MySql;
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "{sql:?} should be a MySQL parse error",
        );
    }

    /// Parse `sql` under MySQL to its single [`SessionStatement`] *without* the canonical
    /// round-trip assertion — for forms whose canonical render normalizes an exact-synonym
    /// spelling (`:=` → `=`), where the fidelity lives in the AST tag, not a byte replay.
    fn mysql_session_no_roundtrip(sql: &str) -> SessionStatement {
        use crate::dialect::MySql;
        let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
            .unwrap_or_else(|err| panic!("PARSE {sql:?}: {err:?}"));
        let [Statement::Session { session, .. }] = parsed.statements() else {
            panic!("{sql:?} is not a session statement");
        };
        (**session).clone()
    }

    /// ANSI with MySQL's `variable_assignment` on but session statements OFF — the
    /// cross-axis combination this test exercises. Isolates the flag's lexer facet
    /// from its parser facet: ANSI has `named_argument` off, so any `:=` munch here is
    /// solely `variable_assignment`'s doing.
    const VARIABLE_ASSIGNMENT_ONLY: FeatureSet = FeatureSet::ANSI.with(
        FeatureDelta::EMPTY
            .show_syntax(ShowSyntax {
                session_statements: false,
                ..FeatureSet::ANSI.show_syntax
            })
            .session_variables(SessionVariableSyntax {
                variable_assignment: true,
                ..FeatureSet::ANSI.session_variables
            }),
    );

    #[test]
    fn variable_assignment_lexer_facet_fires_without_session_statements() {
        use crate::tokenizer::{Operator, Punctuation, TokenKind, tokenize_with};

        // Lexer facet: `:=` munches to one `ColonEquals` operator under
        // `variable_assignment`, independent of the `session_statements` axis that gates the
        // parser facet — this is *why* the flag is not fully inert when session statements
        // are off, and thus why it is a documented exemption from the `feature_dependencies`
        // inertness registry rather than a `FeatureDependencyViolation` variant.
        let var_only = tokenize_with(":=", &VARIABLE_ASSIGNMENT_ONLY).expect("lexes");
        assert_eq!(
            var_only.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![TokenKind::Operator(Operator::ColonEquals)],
        );
        // The same source under a set with `variable_assignment` off (ANSI, `named_argument`
        // also off) leaves `:=` as two separate tokens — proving the single-token munch above
        // is `variable_assignment`'s effect, present with the parser axis switched off.
        let neither = tokenize_with(":=", &FeatureSet::ANSI).expect("lexes");
        assert_eq!(
            neither.iter().map(|t| t.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::Punctuation(Punctuation::Colon),
                TokenKind::Operator(Operator::Eq),
            ],
        );
    }

    #[test]
    fn variable_assignment_parser_facet_is_dead_without_session_statements() {
        // Parser facet: with `session_statements` off, the leading `SET` is never dispatched
        // (query.rs gates `parse_session_statement` on it), so no `SET` form parses — the
        // measured cross-axis dependency. `SET x = 1` reaches the statement dispatcher and
        // errors as an unknown statement leader.
        let dialect = FeatureDialect {
            features: &VARIABLE_ASSIGNMENT_ONLY,
        };
        assert!(
            parse_with("SET x = 1", crate::ParseConfig::new(dialect)).is_err(),
            "no SET form parses when session_statements is off",
        );
        // And the combination is neither a feature-dependency violation (the flag is not
        // inert — its lexer facet fires above) nor a lexical conflict: it is the documented
        // exemption, so both registries return `None`.
        assert_eq!(VARIABLE_ASSIGNMENT_ONLY.feature_dependencies(), None);
        assert_eq!(VARIABLE_ASSIGNMENT_ONLY.lexical_conflict(), None);
    }

    #[test]
    fn mysql_set_family_probe_user_variable_parses() {
        // The measured `SET` family probe body: a user-variable assignment.
        let parsed = mysql_round_trips("SET @zzp_v = 1");
        let [Statement::Session { session, .. }] = parsed.statements() else {
            panic!("expected a session statement");
        };
        let SessionStatement::SetVariables { assignments, .. } = &**session else {
            panic!("expected SetVariables");
        };
        let [
            SetVariableAssignment::UserVariable {
                name,
                assignment,
                value,
                ..
            },
        ] = assignments.as_slice()
        else {
            panic!("expected one user-variable assignment");
        };
        assert_eq!(parsed.resolver().resolve(name.sym), "zzp_v");
        assert_eq!(*assignment, SetAssignment::Equals);
        assert!(matches!(**value, Expr::Literal { .. }));
    }

    #[test]
    fn mysql_set_user_variable_shapes_round_trip() {
        // `:=` and `=`, full-expression values, quoted names, comma lists.
        mysql_round_trips("SET @v = 1");
        mysql_round_trips("SET @v = 1 + 2");
        mysql_round_trips("SET @v = (SELECT 1)");
        mysql_round_trips("SET @v = @w");
        mysql_round_trips("SET @a = 1, @b = 2");
        mysql_round_trips("SET @'v' = 1");
        mysql_round_trips("SET @\"v\" = 1");
        mysql_round_trips("SET @`v` = 1");

        // `:=` is captured as a distinct assignment tag (the canonical render normalizes it
        // to `=`; a source-fidelity render replays it).
        let SessionStatement::SetVariables { assignments, .. } =
            mysql_session_no_roundtrip("SET @v := 1")
        else {
            panic!("expected SetVariables");
        };
        let SetVariableAssignment::UserVariable { assignment, .. } = &assignments[0] else {
            panic!("expected user-variable");
        };
        assert_eq!(*assignment, SetAssignment::ColonEquals);
    }

    #[test]
    fn mysql_set_scoped_system_variable_shapes_round_trip() {
        for (sql, kind) in [
            (
                "SET GLOBAL max_connections = 100",
                SystemVariableScopeKind::Global,
            ),
            (
                "SET SESSION sql_mode = 'x'",
                SystemVariableScopeKind::Session,
            ),
            ("SET LOCAL sql_mode = 'x'", SystemVariableScopeKind::Local),
            (
                "SET PERSIST max_connections = 100",
                SystemVariableScopeKind::Persist,
            ),
            (
                "SET PERSIST_ONLY max_connections = 100",
                SystemVariableScopeKind::PersistOnly,
            ),
        ] {
            let SetVariableAssignment::SystemVariable { scope, .. } = mysql_one_assignment(sql)
            else {
                panic!("{sql:?} expected a system-variable assignment");
            };
            assert_eq!(scope, SystemVariableScope::Keyword(kind), "{sql:?}");
        }
        // Plain (session-implicit) and `:=` on a system variable.
        let SetVariableAssignment::SystemVariable { scope, .. } =
            mysql_one_assignment("SET sql_mode = 'x'")
        else {
            panic!("expected a system-variable assignment");
        };
        assert_eq!(scope, SystemVariableScope::Implicit);
        mysql_round_trips("SET GLOBAL innodb_x.y = 1");

        // `:=` on a system variable is captured as its own tag (canonical render → `=`).
        let SessionStatement::SetVariables { assignments, .. } =
            mysql_session_no_roundtrip("SET GLOBAL max_connections := 100")
        else {
            panic!("expected SetVariables");
        };
        let SetVariableAssignment::SystemVariable { assignment, .. } = &assignments[0] else {
            panic!("expected system-variable");
        };
        assert_eq!(*assignment, SetAssignment::ColonEquals);
    }

    #[test]
    fn mysql_set_at_at_system_variable_shapes_round_trip() {
        for (sql, scope) in [
            ("SET @@max_connections = 100", SystemVariableScope::AtAt),
            (
                "SET @@global.max_connections = 100",
                SystemVariableScope::AtAtScoped(SystemVariableScopeKind::Global),
            ),
            (
                "SET @@session.sql_mode = 'x'",
                SystemVariableScope::AtAtScoped(SystemVariableScopeKind::Session),
            ),
            (
                "SET @@local.sql_mode = 'x'",
                SystemVariableScope::AtAtScoped(SystemVariableScopeKind::Local),
            ),
            (
                "SET @@persist.max_connections = 100",
                SystemVariableScope::AtAtScoped(SystemVariableScopeKind::Persist),
            ),
            (
                "SET @@persist_only.max_connections = 100",
                SystemVariableScope::AtAtScoped(SystemVariableScopeKind::PersistOnly),
            ),
        ] {
            let SetVariableAssignment::SystemVariable {
                scope: got_scope, ..
            } = mysql_one_assignment(sql)
            else {
                panic!("{sql:?} expected a system-variable assignment");
            };
            assert_eq!(got_scope, scope, "{sql:?}");
        }
    }

    #[test]
    fn mysql_set_value_sentinels_and_default_round_trip() {
        mysql_round_trips("SET sql_mode = DEFAULT");
        mysql_round_trips("SET autocommit = ON");
        mysql_round_trips("SET big_tables = ALL");
        mysql_round_trips("SET x = BINARY");
        mysql_round_trips("SET x = ROW");
        mysql_round_trips("SET x = SYSTEM");

        let SetVariableAssignment::SystemVariable { value, .. } =
            mysql_one_assignment("SET sql_mode = DEFAULT")
        else {
            panic!("expected a system-variable assignment");
        };
        assert!(matches!(value, SetVariableValue::Default { .. }));
        let SetVariableAssignment::SystemVariable { value, .. } =
            mysql_one_assignment("SET autocommit = ON")
        else {
            panic!("expected a system-variable assignment");
        };
        assert!(matches!(
            value,
            SetVariableValue::Keyword {
                keyword: SetVariableKeyword::On,
                ..
            }
        ));
    }

    #[test]
    fn mysql_set_mixed_scope_lists_round_trip() {
        mysql_round_trips("SET GLOBAL max_connections = 100, SESSION sql_mode = 'x'");
        mysql_round_trips("SET @a = 1, GLOBAL max_connections = 2, @b = 3");
        mysql_round_trips("SET sql_mode = 'x', @v = 1");
        mysql_round_trips("SET @@global.x = 1, @@session.y = 2");

        let SessionStatement::SetVariables { assignments, .. } =
            mysql_session("SET @a = 1, GLOBAL max_connections = 2, @b = 3")
        else {
            panic!("expected SetVariables");
        };
        assert_eq!(assignments.len(), 3);
        assert!(matches!(
            assignments[0],
            SetVariableAssignment::UserVariable { .. }
        ));
        assert!(matches!(
            assignments[1],
            SetVariableAssignment::SystemVariable {
                scope: SystemVariableScope::Keyword(SystemVariableScopeKind::Global),
                ..
            }
        ));
        assert!(matches!(
            assignments[2],
            SetVariableAssignment::UserVariable { .. }
        ));
    }

    #[test]
    fn mysql_set_character_set_and_names_round_trip() {
        // CHARACTER SET / CHARSET (new), and NAMES (shared standalone variant).
        mysql_round_trips("SET CHARACTER SET utf8mb4");
        mysql_round_trips("SET CHARACTER SET DEFAULT");
        mysql_round_trips("SET CHARSET utf8mb4");
        mysql_round_trips("SET CHARSET DEFAULT");
        mysql_round_trips("SET CHARACTER SET binary");
        mysql_round_trips("SET NAMES utf8mb4");
        mysql_round_trips("SET NAMES utf8mb4 COLLATE utf8mb4_bin");
        mysql_round_trips("SET NAMES DEFAULT");

        let SessionStatement::SetCharacterSet { keyword, value, .. } =
            mysql_session("SET CHARSET DEFAULT")
        else {
            panic!("expected SetCharacterSet");
        };
        assert_eq!(keyword, CharacterSetKeyword::Charset);
        assert!(matches!(*value, SetCharacterSetValue::Default { .. }));

        // NAMES stays the shared standalone variant, not the MySQL list.
        assert!(matches!(
            mysql_session("SET NAMES utf8mb4"),
            SessionStatement::SetNames { .. }
        ));
    }

    #[test]
    fn mysql_set_rejects_match_the_oracle_boundary() {
        // Measured 1064 (syntax) rejects on live 8.4.10.
        mysql_rejects("SET @v = DEFAULT"); // user-var value is a plain expr, DEFAULT invalid
        mysql_rejects("SET @v = ON"); // keyword sentinels are system-var-only
        mysql_rejects("SET NAMES = utf8mb4"); // `NAMES =` is always a syntax error
        mysql_rejects("SET GLOBAL @@x = 1"); // keyword scope + `@@` are mutually exclusive
        mysql_rejects("SET x = 1, 2"); // no bare value list; each comma is a new assignment
        mysql_rejects("SET @v"); // missing `= value`
        mysql_rejects("SET"); // empty
    }

    #[test]
    fn mysql_set_role_still_parses() {
        // `SET ROLE` remains the shared dedicated node, not the variable list.
        assert!(matches!(
            mysql_session("SET ROLE NONE"),
            SessionStatement::SetRole { .. }
        ));
    }

    /// The `SET RESOURCE GROUP` member of the MySQL resource-group family
    /// (`set_resource_group_stmt`): the bare session-binding form and the `FOR <thread_ids>`
    /// form, whose ids are `real_ulong_num`s (hex admitted) under `opt_comma` separators —
    /// `FOR 1, 2` and `FOR 1 2` both grammar-accept on mysql:8.4.10; the whitespace spelling
    /// normalizes to the canonical comma list on render.
    #[test]
    fn mysql_set_resource_group_parses_and_round_trips() {
        let SessionStatement::SetResourceGroup { thread_ids, .. } =
            mysql_session("SET RESOURCE GROUP zzp_rg")
        else {
            panic!("expected SET RESOURCE GROUP");
        };
        assert!(thread_ids.is_none(), "the bare form binds the session");

        let SessionStatement::SetResourceGroup { thread_ids, .. } =
            mysql_session("SET RESOURCE GROUP g FOR 1, 2, 3")
        else {
            panic!("expected SET RESOURCE GROUP");
        };
        assert_eq!(thread_ids.expect("FOR list").len(), 3);

        // Byte round-trips, including a hex thread id (`real_ulong_num` admits `HEX_NUM`).
        mysql_round_trips("SET RESOURCE GROUP g FOR 1");
        mysql_round_trips("SET RESOURCE GROUP g FOR 0x10");
        mysql_round_trips("SET RESOURCE GROUP `g` FOR 1, 2");

        // The `opt_comma` separator: whitespace-separated ids parse and render canonically.
        use crate::dialect::MySql;
        let parsed = parse_with(
            "SET RESOURCE GROUP g FOR 1 2 3",
            crate::ParseConfig::new(MySql),
        )
        .unwrap();
        let rendered = Renderer::new(MYSQL_RENDER).render_parsed(&parsed).unwrap();
        assert_eq!(rendered, "SET RESOURCE GROUP g FOR 1, 2, 3");
    }

    /// The `SET RESOURCE GROUP` seam against the variable-assignment `SET` head is MECE: a
    /// variable named `resource` still routes to the assignment grammar (the two-word
    /// `RESOURCE GROUP` lookahead can never steal it), the truncated forms reject, and the
    /// gate keeps the statement MySQL/Lenient-only.
    #[test]
    fn mysql_set_resource_group_seam_and_gating() {
        assert!(matches!(
            mysql_session("SET resource = 1"),
            SessionStatement::SetVariables { .. }
        ));
        mysql_rejects("SET RESOURCE GROUP");
        mysql_rejects("SET RESOURCE GROUP g FOR");
        mysql_rejects("SET RESOURCE GROUP g FOR x");

        use crate::dialect::{Lenient, MySql};
        let sql = "SET RESOURCE GROUP g";
        parse_with(sql, crate::ParseConfig::new(MySql))
            .unwrap_or_else(|err| panic!("MySQL accepts {sql:?}: {err:?}"));
        parse_with(sql, crate::ParseConfig::new(Lenient))
            .unwrap_or_else(|err| panic!("Lenient accepts {sql:?}: {err:?}"));
        parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
            .expect_err(&format!("PostgreSQL rejects {sql:?}"));
        parse_with(sql, crate::ParseConfig::new(crate::dialect::Ansi))
            .expect_err(&format!("ANSI rejects {sql:?}"));
    }

    #[test]
    fn create_user_family_probe_parses_and_round_trips() {
        // The measured `CREATE USER` family probe.
        let parsed = mysql_round_trips("CREATE USER zzp_u@localhost");
        let [Statement::CreateUser { create, .. }] = parsed.statements() else {
            panic!("expected CREATE USER, got {:?}", parsed.statements());
        };
        assert!(!create.if_not_exists);
        assert_eq!(create.users.len(), 1);
        assert!(create.users[0].auth.is_none());
        let AccountName::Account { host, .. } = &create.users[0].account else {
            panic!("expected a named account");
        };
        assert!(host.is_some(), "the @host is captured");
    }

    #[test]
    fn create_role_and_drop_family_probes_round_trip() {
        // The measured `CREATE ROLE` / `DROP USER` / `DROP ROLE` family probes.
        for (sql, kind) in [
            ("CREATE ROLE zzp_r", UserRoleListKind::CreateRole),
            ("DROP USER zzp_u@localhost", UserRoleListKind::DropUser),
            ("DROP ROLE zzp_r", UserRoleListKind::DropRole),
        ] {
            let parsed = mysql_round_trips(sql);
            let [Statement::UserRoleList { statement, .. }] = parsed.statements() else {
                panic!("expected a user/role list statement for {sql:?}");
            };
            assert_eq!(statement.kind, kind);
            assert!(!statement.if_guard);
            assert_eq!(statement.names.len(), 1);
        }
    }

    #[test]
    fn alter_user_family_probe_parses_and_round_trips() {
        // The measured `ALTER USER` family probe.
        let parsed = mysql_round_trips("ALTER USER zzp_u@localhost IDENTIFIED BY 'zzp_pw'");
        let [Statement::AlterUser { alter, .. }] = parsed.statements() else {
            panic!("expected ALTER USER, got {:?}", parsed.statements());
        };
        let AlterUser::Modify { users, .. } = alter.as_ref() else {
            panic!("expected the ALTER USER modify form");
        };
        assert_eq!(users.len(), 1);
        assert!(matches!(users[0].auth, Some(AuthOption::Password { .. })));
    }

    #[test]
    fn account_name_host_spellings_round_trip() {
        // The full account-name axis: every measured `@host` spelling and the quoting matrix.
        for sql in [
            "DROP USER u",                                // bare user, absent host
            "DROP USER u@localhost",                      // unquoted host (folded @-token)
            "DROP USER u@'localhost'",                    // single-quoted host (standalone @)
            "DROP USER u@\"localhost\"",                  // double-quoted host
            "DROP USER `u`@`localhost`",                  // backtick-quoted both parts
            "DROP USER 'admin'@'10.0.0.1'",               // quoted user and host
            "DROP USER CURRENT_USER",                     // CURRENT_USER pseudo-account
            "DROP USER CURRENT_USER()",                   // CURRENT_USER() call form
            "DROP USER a@localhost, b@'%', CURRENT_USER", // a list mixing spellings
        ] {
            mysql_round_trips(sql);
        }
    }

    #[test]
    fn create_user_auth_shapes_round_trip() {
        for sql in [
            "CREATE USER u@localhost IDENTIFIED BY 'secret'",
            "CREATE USER u@localhost IDENTIFIED BY RANDOM PASSWORD",
            "CREATE USER u@localhost IDENTIFIED WITH mysql_native_password",
            "CREATE USER u@localhost IDENTIFIED WITH caching_sha2_password AS '0xABCDEF'",
            "CREATE USER u@localhost IDENTIFIED WITH mysql_native_password BY 'secret'",
            "CREATE USER u@localhost IDENTIFIED WITH mysql_native_password BY RANDOM PASSWORD",
            "CREATE USER IF NOT EXISTS u@localhost IDENTIFIED BY 'p', v@localhost IDENTIFIED BY 'q'",
        ] {
            mysql_round_trips(sql);
        }
    }

    #[test]
    fn create_user_tls_resource_and_lock_shapes_round_trip() {
        for sql in [
            "CREATE USER u@localhost REQUIRE NONE",
            "CREATE USER u@localhost REQUIRE SSL",
            "CREATE USER u@localhost REQUIRE X509",
            "CREATE USER u@localhost REQUIRE SUBJECT '/CN=x' AND ISSUER '/CN=ca' AND CIPHER 'DHE-RSA'",
            "CREATE USER u@localhost WITH MAX_QUERIES_PER_HOUR 100 MAX_USER_CONNECTIONS 5",
            "CREATE USER u@localhost PASSWORD EXPIRE",
            "CREATE USER u@localhost PASSWORD EXPIRE INTERVAL 30 DAY",
            "CREATE USER u@localhost PASSWORD EXPIRE NEVER ACCOUNT LOCK",
            "CREATE USER u@localhost PASSWORD HISTORY 5 PASSWORD REUSE INTERVAL 365 DAY",
            "CREATE USER u@localhost PASSWORD REQUIRE CURRENT OPTIONAL",
            "CREATE USER u@localhost DEFAULT ROLE admin, dev",
            "CREATE USER u@localhost COMMENT 'a comment'",
            "CREATE USER u@localhost ATTRIBUTE '{\"k\": \"v\"}'",
            "CREATE USER u@localhost IDENTIFIED BY 'p' DEFAULT ROLE r REQUIRE SSL WITH MAX_QUERIES_PER_HOUR 10 ACCOUNT UNLOCK COMMENT 'c'",
        ] {
            mysql_round_trips(sql);
        }
    }

    #[test]
    fn alter_user_shapes_round_trip() {
        for sql in [
            "ALTER USER IF EXISTS u@localhost IDENTIFIED BY 'new'",
            "ALTER USER u@localhost IDENTIFIED BY 'new' REPLACE 'old'",
            "ALTER USER u@localhost IDENTIFIED BY 'new' RETAIN CURRENT PASSWORD",
            "ALTER USER u@localhost DISCARD OLD PASSWORD",
            "ALTER USER u@localhost IDENTIFIED BY RANDOM PASSWORD",
            "ALTER USER u@localhost FAILED_LOGIN_ATTEMPTS 3 PASSWORD_LOCK_TIME 2",
            "ALTER USER u@localhost PASSWORD_LOCK_TIME UNBOUNDED",
            "ALTER USER CURRENT_USER IDENTIFIED BY 'new'",
            "ALTER USER u@localhost DEFAULT ROLE ALL",
            "ALTER USER u@localhost DEFAULT ROLE NONE",
            "ALTER USER u@localhost DEFAULT ROLE admin, dev",
        ] {
            mysql_round_trips(sql);
        }
    }

    #[test]
    fn account_ddl_is_gated_off_for_non_mysql_dialects() {
        use crate::dialect::Postgres;
        // With `user_role_management` off, `CREATE USER` is not dispatched and the parse fails
        // (the leading `USER` surfaces as the ordinary CREATE-object parse error).
        assert!(parse_with("CREATE USER u@localhost", crate::ParseConfig::new(Postgres)).is_err());
        assert!(parse_with("DROP ROLE r", crate::ParseConfig::new(Postgres)).is_err());
    }

    // --- MySQL account-based GRANT / REVOKE ---------------------------------

    /// Parse `sql` under MySQL and return its single access-control statement.
    fn mysql_access(sql: &str) -> AccessControlStatement {
        let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let [Statement::AccessControl { access, .. }] = parsed.statements() else {
            panic!(
                "{sql:?} did not parse to one access-control statement: {:?}",
                parsed.statements(),
            );
        };
        (**access).clone()
    }

    #[test]
    fn mysql_grant_priv_levels_round_trip() {
        // The four `priv_level` spellings and the acl-type keywords, all live-oracle accepted.
        for sql in [
            "GRANT SELECT ON *.* TO 'a'@'localhost'",
            "GRANT SELECT ON * TO 'a'@'localhost'",
            "GRANT SELECT ON db.* TO 'a'@'localhost'",
            "GRANT SELECT ON db.tbl TO 'a'@'localhost'",
            "GRANT SELECT ON tbl TO 'a'@'localhost'",
            "GRANT SELECT ON TABLE db.tbl TO 'a'@'localhost'",
            "GRANT EXECUTE ON FUNCTION db.f TO 'a'@'localhost'",
            "GRANT EXECUTE ON PROCEDURE db.p TO 'a'@'localhost'",
        ] {
            mysql_round_trips(sql);
        }
    }

    #[test]
    fn mysql_grant_priv_level_variants_are_distinguished() {
        let global = mysql_access("GRANT SELECT ON *.* TO u");
        let AccessControlStatement::AccountGrantPrivilege { object, .. } = &global else {
            panic!("expected a MySQL privilege grant");
        };
        assert!(matches!(object.level, PrivilegeLevel::Global { .. }));
        assert!(matches!(
            object.object_type,
            PrivilegeObjectType::Table { explicit: false }
        ));

        let current_db = mysql_access("GRANT SELECT ON * TO u");
        let AccessControlStatement::AccountGrantPrivilege { object, .. } = &current_db else {
            panic!("expected a grant");
        };
        assert!(matches!(
            object.level,
            PrivilegeLevel::CurrentDatabase { .. }
        ));

        let db_wide = mysql_access("GRANT SELECT ON db.* TO u");
        let AccessControlStatement::AccountGrantPrivilege { object, .. } = &db_wide else {
            panic!("expected a grant");
        };
        assert!(matches!(object.level, PrivilegeLevel::Database { .. }));

        let explicit_table = mysql_access("GRANT SELECT ON TABLE db.tbl TO u");
        let AccessControlStatement::AccountGrantPrivilege { object, .. } = &explicit_table else {
            panic!("expected a grant");
        };
        assert!(matches!(
            object.object_type,
            PrivilegeObjectType::Table { explicit: true }
        ));
        assert!(matches!(object.level, PrivilegeLevel::Object { .. }));
    }

    #[test]
    fn mysql_grant_privileges_columns_and_dynamic_round_trip() {
        for sql in [
            "GRANT ALL PRIVILEGES ON db.* TO u",
            "GRANT SELECT (c1, c2), INSERT ON db.tbl TO u",
            "GRANT CREATE, DROP, ALTER, INDEX ON db.* TO u",
            "GRANT GRANT OPTION ON db.* TO u",
            "GRANT CREATE TEMPORARY TABLES ON db.* TO u",
            "GRANT SHOW DATABASES ON *.* TO u",
            "GRANT REPLICATION SLAVE ON *.* TO u",
            "GRANT CREATE ROUTINE ON db.* TO u",
            "GRANT CREATE USER ON *.* TO u",
            "GRANT BACKUP_ADMIN ON *.* TO u",
        ] {
            mysql_round_trips(sql);
        }
    }

    #[test]
    fn mysql_grant_dynamic_privilege_is_other() {
        let grant = mysql_access("GRANT BACKUP_ADMIN ON *.* TO u");
        let AccessControlStatement::AccountGrantPrivilege { privileges, .. } = &grant else {
            panic!("expected a grant");
        };
        let Privileges::List { privileges, .. } = privileges else {
            panic!("expected a privilege list");
        };
        assert!(matches!(privileges[0], Privilege::Other { .. }));
    }

    #[test]
    fn mysql_grant_with_grant_option_and_as_clause_round_trip() {
        for sql in [
            "GRANT SELECT ON db.* TO 'a'@'localhost' WITH GRANT OPTION",
            "GRANT SELECT ON db.* TO 'a'@'localhost' AS 'b'@'localhost'",
            "GRANT SELECT ON db.* TO 'a'@'localhost' AS 'b'@'localhost' WITH ROLE 'r1', 'r2'",
            "GRANT SELECT ON db.* TO 'a'@'localhost' AS 'b'@'localhost' WITH ROLE ALL",
            "GRANT SELECT ON db.* TO 'a'@'localhost' AS 'b'@'localhost' WITH ROLE ALL EXCEPT 'r1'",
            "GRANT SELECT ON db.* TO 'a'@'localhost' AS 'b'@'localhost' WITH ROLE NONE",
            "GRANT SELECT ON db.* TO 'a'@'localhost' AS 'b'@'localhost' WITH ROLE DEFAULT",
        ] {
            mysql_round_trips(sql);
        }
    }

    #[test]
    fn mysql_grant_as_with_role_variants() {
        let grant = mysql_access("GRANT SELECT ON db.* TO u AS admin WITH ROLE ALL EXCEPT r1, r2");
        let AccessControlStatement::AccountGrantPrivilege { grant_as, .. } = &grant else {
            panic!("expected a grant");
        };
        let grant_as = grant_as.as_ref().expect("AS clause present");
        let Some(WithRoleSpec::All { except, .. }) = &grant_as.with_role else {
            panic!("expected WITH ROLE ALL EXCEPT");
        };
        assert_eq!(except.len(), 2);
    }

    #[test]
    fn mysql_grant_role_and_proxy_round_trip() {
        for sql in [
            "GRANT r1 TO 'a'@'localhost'",
            "GRANT r1, r2 TO u1, u2",
            "GRANT r1 TO u1 WITH ADMIN OPTION",
            "GRANT 'r1'@'localhost' TO 'a'@'localhost'",
            "GRANT PROXY ON 'b'@'localhost' TO 'a'@'localhost'",
            "GRANT PROXY ON 'b'@'localhost' TO 'a'@'localhost' WITH GRANT OPTION",
        ] {
            mysql_round_trips(sql);
        }
    }

    #[test]
    fn mysql_grant_role_is_not_privilege_grant() {
        assert!(matches!(
            mysql_access("GRANT r1, r2 TO u WITH ADMIN OPTION"),
            AccessControlStatement::AccountGrantRole {
                with_admin_option: true,
                ..
            },
        ));
        assert!(matches!(
            mysql_access("GRANT PROXY ON b TO a"),
            AccessControlStatement::AccountGrantProxy { .. },
        ));
    }

    #[test]
    fn mysql_revoke_forms_round_trip() {
        for sql in [
            "REVOKE SELECT ON db.* FROM 'a'@'localhost'",
            "REVOKE SELECT, INSERT ON db.tbl FROM u1, u2",
            "REVOKE ALL PRIVILEGES ON db.* FROM 'a'@'localhost'",
            "REVOKE ALL PRIVILEGES, GRANT OPTION FROM 'a'@'localhost'",
            "REVOKE GRANT OPTION ON db.* FROM 'a'@'localhost'",
            "REVOKE PROXY ON 'b'@'localhost' FROM 'a'@'localhost'",
            "REVOKE r1 FROM 'a'@'localhost'",
            "REVOKE r1, r2 FROM u1, u2",
        ] {
            mysql_round_trips(sql);
        }
    }

    #[test]
    fn mysql_revoke_if_exists_and_ignore_unknown_user_round_trip() {
        for sql in [
            "REVOKE IF EXISTS SELECT ON db.* FROM 'a'@'localhost'",
            "REVOKE SELECT ON db.* FROM 'a'@'localhost' IGNORE UNKNOWN USER",
            "REVOKE IF EXISTS SELECT ON db.* FROM 'a'@'localhost' IGNORE UNKNOWN USER",
            "REVOKE IF EXISTS r1 FROM 'a'@'localhost' IGNORE UNKNOWN USER",
            "REVOKE IF EXISTS PROXY ON 'b'@'localhost' FROM 'a'@'localhost' IGNORE UNKNOWN USER",
            "REVOKE ALL PRIVILEGES, GRANT OPTION FROM 'a'@'localhost' IGNORE UNKNOWN USER",
        ] {
            mysql_round_trips(sql);
        }
    }

    #[test]
    fn mysql_revoke_all_global_variant() {
        assert!(matches!(
            mysql_access(
                "REVOKE IF EXISTS ALL PRIVILEGES, GRANT OPTION FROM u IGNORE UNKNOWN USER"
            ),
            AccessControlStatement::AccountRevokeAll {
                if_exists: true,
                privileges_keyword: true,
                ignore_unknown_user: true,
                ..
            },
        ));
        // The bare `ALL` (no `PRIVILEGES` noise word) parses too; the fidelity tag is recorded.
        assert!(matches!(
            mysql_access("REVOKE ALL, GRANT OPTION FROM u"),
            AccessControlStatement::AccountRevokeAll {
                privileges_keyword: false,
                ..
            },
        ));
        assert!(matches!(
            mysql_access("GRANT ALL ON db.* TO u"),
            AccessControlStatement::AccountGrantPrivilege {
                privileges: Privileges::All {
                    privileges_keyword: false,
                    ..
                },
                ..
            },
        ));
    }

    #[test]
    fn mysql_grant_rejects_non_mysql_forms() {
        // Engine-measured `ER_PARSE_ERROR` on mysql:8.4.10 — the parser must reject them too.
        for sql in [
            "GRANT SELECT ON SCHEMA db TO u",
            "GRANT SELECT ON DATABASE db TO u",
            "GRANT SELECT ON ALL TABLES IN SCHEMA db TO u",
            "REVOKE GRANT OPTION FOR SELECT ON db.* FROM u",
            "GRANT SELECT ON db.* TO u GRANTED BY b",
            "REVOKE SELECT ON db.* FROM u CASCADE",
            "GRANT r1 TO u AS b",
            "GRANT PROXY ON b TO a AS c",
            "GRANT r1 TO u WITH GRANT OPTION",
            "GRANT IF EXISTS SELECT ON db.* TO u",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
                "MySQL must reject {sql:?}",
            );
        }
    }

    #[test]
    fn mysql_and_postgres_grant_grammars_are_disjoint_routes() {
        // MySQL routes to the account grammar (accepts `*.*`); PostgreSQL does not.
        assert!(parse_with("GRANT SELECT ON *.* TO u", crate::ParseConfig::new(MySql)).is_ok());
        use crate::dialect::Postgres;
        assert!(
            parse_with(
                "GRANT SELECT ON *.* TO u",
                crate::ParseConfig::new(Postgres)
            )
            .is_err()
        );
        // PostgreSQL routes to the typed-object grammar (accepts `ON SCHEMA`); MySQL does not.
        assert!(
            parse_with(
                "GRANT USAGE ON SCHEMA s TO u",
                crate::ParseConfig::new(Postgres)
            )
            .is_ok()
        );
        assert!(
            parse_with(
                "GRANT USAGE ON SCHEMA s TO u",
                crate::ParseConfig::new(MySql)
            )
            .is_err()
        );
    }

    // --- INSTALL / UNINSTALL PLUGIN / COMPONENT (MySQL) ----------------------

    /// Every grammar-valid form of the MySQL `INSTALL`/`UNINSTALL` plugin/component family
    /// round-trips byte-identically — the plugin `SONAME` form, the multi-URN component lists,
    /// and the `SET` tail's scope/name/value shapes. The same forms are live-oracle-verified
    /// grammar-valid in `corpus_mysql_verdicts::mysql_plugin_component_live_oracle_parity`.
    #[test]
    fn install_uninstall_family_round_trips() {
        // Structural spot-checks on the component SET tail.
        let parsed = mysql_round_trips(
            "INSTALL COMPONENT 'file://x', 'file://y' SET GLOBAL v = 1, PERSIST w.x = ON, y = 2",
        );
        let [Statement::Install { install, .. }] = parsed.statements() else {
            panic!("expected an INSTALL statement");
        };
        let InstallStatement::Component { urns, set, .. } = &**install else {
            panic!("expected the COMPONENT form");
        };
        assert_eq!(urns.len(), 2, "both URNs carried");
        assert_eq!(set.len(), 3, "all three SET elements carried");
        assert_eq!(set[0].scope, Some(InstallComponentSetScope::Global));
        assert_eq!(set[1].scope, Some(InstallComponentSetScope::Persist));
        assert_eq!(set[1].name.0.len(), 2, "two-part lvalue_variable name");
        assert!(matches!(set[1].value, InstallComponentSetValue::On { .. }));
        assert_eq!(set[2].scope, None, "implicit default scope");
        assert!(matches!(
            set[2].value,
            InstallComponentSetValue::Expr { .. }
        ));

        // The `:=` assignment synonym parses; the canonical render normalizes it to `=`
        // (the general-`SET` precedent), so it is asserted structurally, not by byte replay.
        let parsed = parse_with(
            "INSTALL COMPONENT 'x' SET v := 1",
            crate::ParseConfig::new(MySql),
        )
        .unwrap_or_else(|err| panic!("`:=` form: {err:?}"));
        let [Statement::Install { install, .. }] = parsed.statements() else {
            panic!("expected an INSTALL statement");
        };
        let InstallStatement::Component { set, .. } = &**install else {
            panic!("expected the COMPONENT form");
        };
        assert_eq!(set[0].assignment, SetAssignment::ColonEquals);

        let parsed = mysql_round_trips("UNINSTALL PLUGIN p");
        let [Statement::Uninstall { uninstall, .. }] = parsed.statements() else {
            panic!("expected an UNINSTALL statement");
        };
        assert!(matches!(&**uninstall, UninstallStatement::Plugin { .. }));

        for sql in [
            "INSTALL PLUGIN p SONAME 'lib.so'",
            "INSTALL PLUGIN `p` SONAME 'lib.so'",
            "INSTALL COMPONENT 'file://x'",
            "INSTALL COMPONENT 'file://x', 'file://y', 'file://z'",
            "INSTALL COMPONENT 'x' SET v = 1",
            "INSTALL COMPONENT 'x' SET GLOBAL v = 1",
            "INSTALL COMPONENT 'x' SET PERSIST v = 1",
            "INSTALL COMPONENT 'x' SET comp.v = 1",
            "INSTALL COMPONENT 'x' SET v = ON",
            "INSTALL COMPONENT 'x' SET v = OFF",
            "INSTALL COMPONENT 'x' SET v = 'str'",
            "INSTALL COMPONENT 'x' SET v = 1 + 2",
            "INSTALL COMPONENT 'x', 'y' SET v = 1",
            "UNINSTALL PLUGIN p",
            "UNINSTALL PLUGIN `p`",
            "UNINSTALL COMPONENT 'file://x'",
            "UNINSTALL COMPONENT 'file://x', 'file://y'",
        ] {
            mysql_round_trips(sql);
        }
    }

    #[test]
    fn install_uninstall_family_reject_edge_cases() {
        // Engine-verified `ER_PARSE_ERROR` (1064) on mysql:8.4.10, both-reject-pinned in
        // `m3::SCHEMA_INDEPENDENT_REJECT`: exactly one plugin per statement, an `ident` (not a
        // string) plugin name, a mandatory string `SONAME`, string (not ident) component URNs,
        // the narrow `GLOBAL`/`PERSIST`-only SET scope set (no sigil variables, no `DEFAULT`
        // value sentinel), and no `SET` tail on UNINSTALL COMPONENT.
        for sql in [
            "INSTALL PLUGIN p",
            "INSTALL PLUGIN 'p' SONAME 'lib.so'",
            "INSTALL PLUGIN p SONAME lib",
            "INSTALL PLUGIN p SONAME 'lib.so', q SONAME 'x.so'",
            "UNINSTALL PLUGIN p, q",
            "UNINSTALL PLUGIN 'p'",
            "UNINSTALL PLUGIN",
            "INSTALL COMPONENT file",
            "INSTALL COMPONENT",
            "INSTALL COMPONENT 'x' SET",
            "INSTALL COMPONENT 'x' SET SESSION v = 1",
            "INSTALL COMPONENT 'x' SET LOCAL v = 1",
            "INSTALL COMPONENT 'x' SET PERSIST_ONLY v = 1",
            "INSTALL COMPONENT 'x' SET @v = 1",
            "INSTALL COMPONENT 'x' SET @@v = 1",
            "INSTALL COMPONENT 'x' SET v = DEFAULT",
            "UNINSTALL COMPONENT file",
            "UNINSTALL COMPONENT",
            "UNINSTALL COMPONENT 'x' SET v = 1",
        ] {
            mysql_rejects(sql);
        }
    }
}
