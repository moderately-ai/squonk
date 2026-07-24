// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! DDL statement grammar.
//!
//! This module owns the production DDL surface: `CREATE TABLE` plus the schema
//! evolution statements `ALTER TABLE` and the `DROP {TABLE | VIEW | INDEX |
//! SCHEMA}` family. It reuses the shared type, expression, query, and object-name
//! parsers rather than inventing DDL-specific copies. Most DDL vocabulary is
//! intentionally matched as contextual words here because the full ANSI/PostgreSQL
//! keyword inventory is a separate ticket; only already-tokenized structural
//! keywords such as `CREATE`, `TABLE`, `AS`, `NOT`, and `WITH` are matched by
//! keyword tag.
//!
//! The `ALTER`/`DROP` divergent tails — the `IF EXISTS` existence guard and the
//! `CASCADE`/`RESTRICT` drop behaviour — are gated by `schema_change_syntax`
//! dialect data: when a flag is off the keyword is left unconsumed and
//! the trailing clause surfaces as a parse error.

use crate::ast::{
    AccessControlStatement, AccountName, AggregateArgs, AlterColumnAction, AlterColumnTarget,
    AlterDatabase, AlterDatabaseAction, AlterDatabaseOption, AlterDatabaseOptions, AlterEvent,
    AlterExtension, AlterExtensionAction, AlterInstance, AlterInstanceAction, AlterLogfileGroup,
    AlterObjectDepends, AlterObjectSchema, AlterResourceGroup, AlterRoutine, AlterSequence,
    AlterSequenceOption, AlterServer, AlterSystem, AlterSystemAction, AlterTable, AlterTableAction,
    AlterTablespace, AlterTablespaceAction, AlterView, AutoIncrementSpelling, CharsetKeyword,
    CollateExpr, ColocationPartitionKind, ColumnConstraint, ColumnDef, ColumnOption,
    CommentOnStatement, CommentTarget, ConflictResolution, ConstraintCharacteristics,
    CreateColocationGroup, CreateDatabase, CreateEvent, CreateExtension, CreateExtensionOption,
    CreateFunction, CreateIndex, CreateLogfileGroup, CreateMacro, CreateProcedure,
    CreateResourceGroup, CreateSchema, CreateSecret, CreateSequence, CreateServer,
    CreateSpatialReferenceSystem, CreateStoredTrigger, CreateTable, CreateTableBody,
    CreateTableOption, CreateTableOptionKind, CreateTablespace, CreateTrigger, CreateType,
    CreateTypeDefinition, CreateView, CreateVirtualTable, DataType, DatabaseKeyword, Definer,
    DetachPartitionMode, DropBehavior, DropColocationGroup, DropDatabase, DropEvent,
    DropIndexOnTable, DropLogfileGroup, DropObjectKind, DropResourceGroup, DropSecretStmt,
    DropServer, DropSpatialReferenceSystem, DropStatement, DropTablespace, DropTransform,
    EventOnCompletion, EventSchedule, EventStatus, ExcludeConstraint, ExcludeElement,
    ExcludeOperator, Expr, ExtensionVersion, ForeignKeyMatch, ForeignKeyRef, FunctionBody,
    FunctionNullBehavior, FunctionOption, FunctionParam, FunctionParamDefault,
    FunctionParamDefaultSpelling, FunctionParamMode, GeneratedColumn, GeneratedColumnSpelling,
    GeneratedColumnStorage, Ident, IdentityColumn, IdentityGeneration, IdentityOption,
    IndexAlgorithm, IndexColumn, IndexLock, IndexLockAlgorithmOption, IntervalFields, Keyword,
    KeywordSet, LanguageName, Literal, LiteralKind, MacroBody, MacroParam, MacroSpelling,
    ModuleArg, NamedOperatorSpelling, ObjectName, ObjectRefKind, ObjectReference, OnCommitAction,
    OperatorArgs, PartitionBound, PartitionElem, PartitionSpec, PartitionStrategy, QuoteStyle,
    ReadOnlyValue, ReferentialAction, RefreshMaterializedView, ReplicaSpelling, ResourceGroupState,
    ResourceGroupThreadPriority, ResourceGroupType, ResourceGroupVcpu, RoutineKind,
    RoutineObjectKind, RoutineSignature, SchemaRelocationObject, SecretOption, SecretPersistence,
    ServerOption, ServerOptionKind, SizeLiteral, SizeUnit, Span, Spanned, SqlDataAccess,
    SqlSecurityContext, SrsAttribute, Statement, TableConstraint, TableConstraintDef, TableElement,
    TableLikeAction, TableLikeFeature, TableLikeOption, TableOption, TableOptionValue,
    TableStorageParameter, TablespaceOption, TablespaceSizeOption, TemporaryTableKind,
    TriggerEvent, TriggerOrder, TriggerTiming, UndoTablespaceState, UserRoleListKind, VcpuRange,
    ViewAlgorithm, ViewCheckOption, ViewOptions,
};
use crate::error::ParseResult;
use crate::tokenizer::{Operator, Punctuation, TokenKind};
use thin_vec::{ThinVec, thin_vec};

use super::engine::Parser;
use super::expr::{number_literal_kind, string_literal_is_sconst};
use super::{Dialect, HookResult};

type CreateTableBodyAndOptions<X> = (
    CreateTableBody<X>,
    Option<Box<Ident>>,
    ThinVec<CreateTableOption<X>>,
);
/// A parsed `CREATE TABLE` tail, in PostgreSQL grammar order after the body: the body itself,
/// the `INHERITS (parents)` list (empty when no clause), the optional trailing `PARTITION BY`
/// declarative spec, the optional `USING <access_method>` clause, and the trailing option list.
type CreateTableTail<X> = (
    CreateTableBody<X>,
    ThinVec<ObjectName>,
    Option<Box<PartitionSpec<X>>>,
    Option<Box<Ident>>,
    ThinVec<CreateTableOption<X>>,
);

/// The optional `CREATE [OR REPLACE] [TEMP|TEMPORARY] [MATERIALIZED|RECURSIVE] VIEW`
/// prefix already consumed before the view body. The flags are not independent —
/// `or_replace`/`materialized` are mutually exclusive, `recursive` never co-occurs
/// with `materialized`, and `temporary` only attaches to a regular view — but the
/// dispatcher enforces that, so the body parser takes them as given.
struct ViewPrefix {
    or_replace: bool,
    /// The MySQL `[ALGORITHM = …] [DEFINER = …] [SQL SECURITY …]` definition-option prefix,
    /// parsed by the dispatcher before the `VIEW` keyword; [`ViewOptions::default`] (all-`None`)
    /// for every non-MySQL view and a bare MySQL view.
    options: ViewOptions,
    materialized: bool,
    recursive: bool,
    temporary: Option<TemporaryTableKind>,
}

/// Which MySQL storage-DDL statement context is parsing its shared `ts_option_*` option list —
/// each accepts only a subset of the full option universe, matching the server's per-context
/// grammar so an out-of-context option ends the list and surfaces as a clean parse error.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TsOptionCtx {
    /// `CREATE TABLESPACE` — the full tablespace option set (`tablespace_option`).
    CreateTablespace,
    /// `ALTER TABLESPACE` — the alter subset (`alter_tablespace_option`).
    AlterTablespace,
    /// `CREATE`/`ALTER`/`DROP UNDO TABLESPACE` — `ENGINE` alone (`undo_tablespace_option`).
    UndoTablespace,
    /// `DROP TABLESPACE` / `DROP LOGFILE GROUP` — `ENGINE`/`WAIT` (`opt_drop_ts_options`).
    DropTablespace,
    /// `CREATE LOGFILE GROUP` — the logfile-group set (`logfile_group_option`).
    CreateLogfileGroup,
    /// `ALTER LOGFILE GROUP` — the alter subset (`alter_logfile_group_option`).
    AlterLogfileGroup,
}

impl<'a, D: Dialect> Parser<'a, D> {
    fn parse_create_colocation_group(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("GROUP")?;
        let if_not_exists = self.parse_if_not_exists()?;
        let name = self.parse_ident()?;
        self.expect_contextual_keyword("PARTITION")?;
        self.expect_contextual_keyword("BY")?;
        let partition = if self.eat_contextual_keyword("HASH")? {
            ColocationPartitionKind::Hash
        } else if self.eat_contextual_keyword("RANGE")? {
            ColocationPartitionKind::Range
        } else {
            return Err(self.unexpected("`HASH` or `RANGE`"));
        };
        let columns = self.parse_parenthesized_ident_list(
            "`(` to open the colocation key",
            "`)` to close the colocation key",
        )?;
        self.expect_contextual_keyword("SHARDS")?;
        let shards = self.expect_unsigned_integer_literal("SHARDS")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::CreateColocationGroup {
            create: Box::new(CreateColocationGroup {
                if_not_exists,
                name,
                partition,
                columns,
                shards,
                meta,
            }),
            meta,
        })
    }

    fn parse_drop_colocation_group(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("GROUP")?;
        let if_exists = self.parse_schema_change_if_exists()?;
        let name = self.parse_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::DropColocationGroup {
            drop: Box::new(DropColocationGroup {
                if_exists,
                name,
                meta,
            }),
            meta,
        })
    }

    pub(super) fn parse_refresh_materialized_view_statement(
        &mut self,
    ) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("REFRESH")?;
        self.expect_contextual_keyword("MATERIALIZED")?;
        self.expect_contextual_keyword("VIEW")?;
        let concurrently = self.eat_contextual_keyword("CONCURRENTLY")?;
        let name = self.parse_object_name()?;
        let with_data = if self.eat_contextual_keyword("WITH")? {
            let data = !self.eat_contextual_keyword("NO")?;
            self.expect_contextual_keyword("DATA")?;
            Some(data)
        } else {
            None
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::RefreshMaterializedView {
            refresh: Box::new(RefreshMaterializedView {
                concurrently,
                name,
                with_data,
                meta,
            }),
            meta,
        })
    }

    /// Dispatch a `CREATE` statement on the object kind that follows.
    ///
    /// `SCHEMA`, `MATERIALIZED VIEW`, and `[UNIQUE] INDEX` take no
    /// `TEMP`/`OR REPLACE` prefix and branch first; the remaining `OR REPLACE` and
    /// `TEMP`/`TEMPORARY` prefixes lead into either `TABLE` or `VIEW`. Most object
    /// keywords are matched contextually (the established DDL convention), reserving
    /// keyword tags for the already-tokenized structural words such as `TABLE`.
    pub(super) fn parse_create_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::Create)?;
        if self.features().statement_ddl_gates.colocation_groups
            && self.eat_contextual_keyword("COLOCATION")?
        {
            return self.parse_create_colocation_group(start);
        }

        // The object-kind gates below are whole-statement dispatch gates (like
        // `create_trigger`): each keyword is dispatched only under a dialect that models
        // the object, so a dialect without it (SQLite) leaves the keyword unconsumed and
        // it surfaces as an unknown statement / the `TABLE` expectation below.
        if self.features().statement_ddl_gates.schemas && self.eat_contextual_keyword("SCHEMA")? {
            return self.parse_create_schema(start);
        }
        if self.features().statement_ddl_gates.databases
            && self.eat_contextual_keyword("DATABASE")?
        {
            return self.parse_create_database(start);
        }
        // `CREATE EXTENSION` (PostgreSQL) — a whole-statement gate like `SCHEMA`/`DATABASE`,
        // intercepted here before the `OR REPLACE`/`TEMP` prefix because an extension takes
        // neither. Off elsewhere, where `EXTENSION` falls through to the `TABLE` expectation
        // below and surfaces as an unknown statement.
        if self.features().statement_ddl_gates.extension_ddl
            && self.eat_contextual_keyword("EXTENSION")?
        {
            return self.parse_create_extension(start);
        }
        // `CREATE SERVER <name> FOREIGN DATA WRAPPER <wrapper> OPTIONS ( … )` (MySQL) — a
        // whole-statement gate intercepted before the `OR REPLACE`/`TEMP` prefix (a server takes
        // neither). Off elsewhere, where `SERVER` falls through to the `TABLE` expectation.
        if self.features().statement_ddl_gates.server_definition
            && self.eat_contextual_keyword("SERVER")?
        {
            return self.parse_create_server(start);
        }
        // MySQL storage DDL — `CREATE [UNDO] TABLESPACE …` / `CREATE LOGFILE GROUP …`. Whole-
        // statement gates (like `EXTENSION`), intercepted before the `OR REPLACE`/`TEMP` prefix
        // because none of these forms take one. Off elsewhere, the keyword falls through to the
        // `TABLE` expectation and surfaces as an unknown statement.
        if self.features().statement_ddl_gates.tablespace_ddl {
            // `UNDO` leads only to `UNDO TABLESPACE` here, so eat it and require `TABLESPACE`.
            if self.eat_contextual_keyword("UNDO")? {
                self.expect_contextual_keyword("TABLESPACE")?;
                return self.parse_create_tablespace(start, true);
            }
            if self.eat_contextual_keyword("TABLESPACE")? {
                return self.parse_create_tablespace(start, false);
            }
        }
        if self.features().statement_ddl_gates.logfile_group_ddl
            && self.peek_is_contextual_keyword("LOGFILE")?
            && self.peek_nth_is_contextual_keyword(1, "GROUP")?
        {
            self.advance()?; // LOGFILE
            self.advance()?; // GROUP
            return self.parse_create_logfile_group(start);
        }
        // `CREATE SPATIAL REFERENCE SYSTEM [IF NOT EXISTS] <srid> …` (MySQL) — the bare (no
        // `OR REPLACE`) grammar branch, a whole-statement gate intercepted before the
        // `OR REPLACE`/`TEMP` prefix (the `OR REPLACE` branch is claimed inside that block
        // below; the two branches are exclusive — `CREATE OR REPLACE … IF NOT EXISTS` is
        // `ER_PARSE_ERROR` on mysql:8.4.10). The two-word `SPATIAL REFERENCE` lookahead keeps
        // the seam MECE with `CREATE SPATIAL INDEX` (unmodelled). Off elsewhere, `SPATIAL`
        // falls through to the `TABLE` expectation and surfaces as an unknown statement.
        if self.features().statement_ddl_gates.spatial_reference_system
            && self.peek_is_contextual_keyword("SPATIAL")?
            && self.peek_nth_is_contextual_keyword(1, "REFERENCE")?
        {
            self.expect_contextual_keyword("SPATIAL")?;
            self.expect_contextual_keyword("REFERENCE")?;
            self.expect_contextual_keyword("SYSTEM")?;
            return self.parse_create_spatial_reference_system(start, false);
        }
        // `CREATE RESOURCE GROUP <name> TYPE [=] {SYSTEM | USER} …` (MySQL) — a
        // whole-statement gate intercepted before the `OR REPLACE`/`TEMP` prefix (a resource
        // group takes neither). Off elsewhere, `RESOURCE` falls through to the `TABLE`
        // expectation and surfaces as an unknown statement.
        if self.features().statement_ddl_gates.resource_group
            && self.peek_is_contextual_keyword("RESOURCE")?
        {
            self.expect_contextual_keyword("RESOURCE")?;
            self.expect_contextual_keyword("GROUP")?;
            return self.parse_create_resource_group(start);
        }
        if self.features().statement_ddl_gates.materialized_views
            && self.eat_contextual_keyword("MATERIALIZED")?
        {
            self.expect_contextual_keyword("VIEW")?;
            return self.parse_create_view(
                start,
                ViewPrefix {
                    or_replace: false,
                    options: ViewOptions::default(),
                    materialized: true,
                    recursive: false,
                    temporary: None,
                },
            );
        }
        if self.peek_is_contextual_keyword("UNIQUE")? || self.peek_is_contextual_keyword("INDEX")? {
            return self.parse_create_index(start);
        }
        // `CREATE VIRTUAL TABLE … USING <module>[(<args>)]` (SQLite) — a whole-statement DDL
        // gate like the trigger/macro/secret gates, intercepted before the `TEMP`/`OR REPLACE`
        // prefix because a virtual table takes neither (SQLite engine-measured-rejects
        // `CREATE TEMP VIRTUAL TABLE`). Off elsewhere, where `VIRTUAL` falls through to the
        // `TABLE` expectation below and surfaces as an unknown statement.
        if self.features().statement_ddl_gates.create_virtual_table
            && self.eat_contextual_keyword("VIRTUAL")?
        {
            return self.parse_create_virtual_table(start);
        }
        // MySQL stored-routine DDL — `CREATE [DEFINER = <user>] {PROCEDURE | FUNCTION} …`.
        // A whole-statement gate (like the trigger/virtual-table gates) riding
        // `compound_statements` (the stored-program body sub-language): a MySQL routine's
        // body is a SQL/PSM compound statement, so a dialect without that grammar has no
        // routine-with-body form. The `DEFINER` account prefix and the `PROCEDURE` object
        // keyword are both intercepted here — a routine takes no `TEMP`/`OR REPLACE` prefix,
        // so this precedes that block. Off elsewhere, `PROCEDURE`/`DEFINER` fall through to
        // the `TABLE` expectation and surface as an unknown statement (the string-body
        // `CREATE FUNCTION` still routes to `routines` below, unaffected).
        if self.features().statement_ddl_gates.compound_statements {
            if self.peek_is_contextual_keyword("DEFINER")? {
                let definer = self.parse_definer()?;
                if self.eat_contextual_keyword("PROCEDURE")? {
                    return self.parse_create_procedure(start, Some(definer));
                }
                // A `DEFINER`-prefixed trigger is unambiguously the MySQL stored-program form
                // (the SQLite trigger has no `DEFINER`), so it routes here even under Lenient.
                if self.peek_is_contextual_keyword("TRIGGER")? {
                    return self.parse_create_stored_trigger(start, Some(definer));
                }
                if self.eat_contextual_keyword("EVENT")? {
                    return self.parse_create_event(start, Some(definer));
                }
                // A `DEFINER`-prefixed view — `CREATE DEFINER = <user> [SQL SECURITY …] VIEW …`
                // (the `definer definer_tail` → view grammar branch). Gated on the MySQL view
                // definition surface, which shares the definer prefix with the routine forms
                // above; a `DEFINER`-led view takes no `ALGORITHM` (that lives on the separate
                // `view_algorithm` branch), so only the `SQL SECURITY` sub-option follows here.
                if self
                    .features()
                    .view_sequence_clause_syntax
                    .view_definition_options
                    && (self.peek_is_contextual_keyword("VIEW")?
                        || self.peek_is_contextual_keyword("SQL")?)
                {
                    let sql_security = self.parse_optional_view_sql_security()?;
                    self.expect_contextual_keyword("VIEW")?;
                    return self.parse_create_view(
                        start,
                        ViewPrefix {
                            or_replace: false,
                            options: ViewOptions {
                                algorithm: None,
                                definer: Some(Box::new(definer)),
                                sql_security,
                            },
                            materialized: false,
                            recursive: false,
                            temporary: None,
                        },
                    );
                }
                self.expect_contextual_keyword("FUNCTION")?;
                return self.parse_create_function(start, false, Some(definer));
            }
            if self.eat_contextual_keyword("PROCEDURE")? {
                return self.parse_create_procedure(start, None);
            }
            // A bare (definer-less) `CREATE TRIGGER` collides with the SQLite trigger grammar,
            // which spells an incompatible `BEGIN <stmt>; … END` body. The two cannot both be
            // parsed unambiguously, so the MySQL stored-program form is claimed here only under
            // a dialect that does NOT also model the SQLite trigger (`!create_trigger` — true for
            // the MySQL preset, false for Lenient). Under Lenient a bare `CREATE TRIGGER` stays
            // with the SQLite parser below; the MySQL forms Lenient still reaches are the
            // `DEFINER`-prefixed one (above) and any the SQLite `BEGIN`-body grammar also accepts.
            if !self.features().statement_ddl_gates.create_trigger
                && self.peek_is_contextual_keyword("TRIGGER")?
            {
                return self.parse_create_stored_trigger(start, None);
            }
            if self.eat_contextual_keyword("EVENT")? {
                return self.parse_create_event(start, None);
            }
        }
        // MySQL `CREATE [ALGORITHM = …] [DEFINER = …] [SQL SECURITY …] VIEW …` — the view
        // definition-option prefix (the `view_algorithm`-led and bare `SQL SECURITY`-led
        // grammar branches). `ALGORITHM`/`SQL SECURITY` lead only to a view, so they are
        // intercepted before the `OR REPLACE`/`TEMP` prefix (a definer-led view was handled in
        // the routine block above; an `OR REPLACE`-prefixed view is handled in that block
        // below). The prefix keywords are fixed-order — a permutation is left for the `VIEW`
        // expectation and surfaces as a clean parse error, mirroring the engine.
        if self
            .features()
            .view_sequence_clause_syntax
            .view_definition_options
            && (self.peek_is_contextual_keyword("ALGORITHM")?
                || (self.peek_is_contextual_keyword("SQL")?
                    && self.peek_nth_is_contextual_keyword(1, "SECURITY")?))
        {
            let options = self.parse_view_option_prefix()?;
            self.expect_contextual_keyword("VIEW")?;
            return self.parse_create_view(
                start,
                ViewPrefix {
                    or_replace: false,
                    options,
                    materialized: false,
                    recursive: false,
                    temporary: None,
                },
            );
        }
        // MySQL account-management DDL — `CREATE USER` / `CREATE ROLE`. A whole-statement gate
        // (like the routine gates above), intercepted before the `OR REPLACE`/`TEMP` prefix
        // because neither takes one. Off elsewhere, `USER`/`ROLE` fall through to the `TABLE`
        // expectation below and surface as an unknown statement.
        if self.features().access_control_syntax.user_role_management {
            if self.eat_contextual_keyword("USER")? {
                return self.parse_create_user(start);
            }
            if self.eat_contextual_keyword("ROLE")? {
                return self.parse_user_role_list(start, UserRoleListKind::CreateRole);
            }
        }

        let or_replace =
            self.features().statement_ddl_gates.or_replace && self.parse_or_replace()?;
        let temporary = self.parse_temporary_table_kind()?;
        // `CREATE UNLOGGED TABLE` — in PostgreSQL's `OptTemp` grammar `UNLOGGED` is a peer of
        // `TEMP`/`TEMPORARY`, so it never co-occurs with a consumed `temporary` (guarded by
        // `temporary.is_none()`, reproducing PostgreSQL's `CREATE TEMP UNLOGGED TABLE` reject),
        // and it leads only to a `TABLE` — never a view/function/etc. Gated; off-dialect the
        // keyword is left unconsumed and surfaces as the `TABLE` expectation below.
        if temporary.is_none()
            && self.features().create_table_clause_syntax.unlogged_tables
            && self.eat_contextual_keyword("UNLOGGED")?
        {
            self.expect_keyword(Keyword::Table)?;
            return self.parse_create_table_rest(start, or_replace, temporary, true);
        }
        // DuckDB macro DDL — a whole-statement gate that intercepts `MACRO` and the
        // live-body `FUNCTION` spelling *before* the string-body routine paths below.
        // `MACRO` is unambiguous; `FUNCTION` shares the `CREATE … (params) AS` prefix with
        // the PostgreSQL routine (both flags are on under Lenient), so a bounded lookahead
        // decides whether this `FUNCTION` is the macro spelling (see
        // `create_function_is_macro`) or the routine parsed further down.
        if self.features().statement_ddl_gates.create_macro {
            if self.eat_contextual_keyword("MACRO")? {
                return self.parse_create_macro(start, or_replace, temporary, MacroSpelling::Macro);
            }
            if self.peek_is_contextual_keyword("FUNCTION")? && self.create_function_is_macro()? {
                self.expect_contextual_keyword("FUNCTION")?;
                return self.parse_create_macro(
                    start,
                    or_replace,
                    temporary,
                    MacroSpelling::Function,
                );
            }
        }
        // DuckDB `CREATE [OR REPLACE] [TEMP] TYPE <name> AS …` — a whole-statement gate
        // (like the macro/secret gates) that intercepts `TYPE` before the `OR REPLACE`
        // view/function/table dispatch and the plain `TABLE` expectation below. It carries
        // the already-parsed `or_replace`/`temporary` prefix; every form (`CREATE TYPE`,
        // `CREATE OR REPLACE TYPE`, `CREATE TEMP TYPE`) routes through here. Off elsewhere,
        // `TYPE` falls through to the `TABLE` expectation and surfaces as an unknown statement.
        if self.features().statement_ddl_gates.create_type && self.eat_contextual_keyword("TYPE")? {
            return self.parse_create_type(start, or_replace, temporary);
        }
        if or_replace {
            // `OR REPLACE` belongs to a view or a function — and, under DuckDB, a table; and,
            // under MySQL, a spatial reference system.
            if self.features().statement_ddl_gates.routines
                && self.eat_contextual_keyword("FUNCTION")?
            {
                return self.parse_create_function(start, true, None);
            }
            // `CREATE OR REPLACE SPATIAL REFERENCE SYSTEM <srid> …` (MySQL) — the `OR REPLACE`
            // grammar branch, which admits no `IF NOT EXISTS` (the srid follows `SYSTEM`
            // directly; `CREATE OR REPLACE … IF NOT EXISTS` is `ER_PARSE_ERROR` on
            // mysql:8.4.10) and no `TEMP` (a consumed `temporary` leaves `SPATIAL` for the
            // `VIEW` expectation below — a clean parse error).
            if temporary.is_none()
                && self.features().statement_ddl_gates.spatial_reference_system
                && self.peek_is_contextual_keyword("SPATIAL")?
                && self.peek_nth_is_contextual_keyword(1, "REFERENCE")?
            {
                self.expect_contextual_keyword("SPATIAL")?;
                self.expect_contextual_keyword("REFERENCE")?;
                self.expect_contextual_keyword("SYSTEM")?;
                return self.parse_create_spatial_reference_system(start, true);
            }
            // DuckDB's `CREATE OR REPLACE TABLE` — a flag on the CreateTable node, gated so
            // the other dialects (which take `OR REPLACE` only on `VIEW`/`FUNCTION`) still
            // reject it: with the flag off, `TABLE` is left for the `VIEW` expectation below
            // and surfaces as the "expected VIEW, found TABLE" parse error.
            if self.features().statement_ddl_gates.create_or_replace_table
                && self.eat_keyword(Keyword::Table)?
            {
                return self.parse_create_table_rest(start, true, temporary, false);
            }
            // `CREATE OR REPLACE [TEMP] RECURSIVE VIEW` — the `RECURSIVE` keyword sits
            // directly before `VIEW`, gated to the dialects that model it (DuckDB/Lenient).
            let recursive = self.features().view_sequence_clause_syntax.recursive_views
                && self.eat_keyword(Keyword::Recursive)?;
            // MySQL's `CREATE OR REPLACE [ALGORITHM = …] [DEFINER = …] [SQL SECURITY …] VIEW`
            // (the `view_replace [view_algorithm] definer_opt` grammar branch): the option
            // prefix sits between `OR REPLACE` and `VIEW`. All-`None` for a bare/non-MySQL
            // `OR REPLACE VIEW` (the gate is off, or no option keyword is present).
            let options = if self
                .features()
                .view_sequence_clause_syntax
                .view_definition_options
            {
                self.parse_view_option_prefix()?
            } else {
                ViewOptions::default()
            };
            self.expect_contextual_keyword("VIEW")?;
            return self.parse_create_view(
                start,
                ViewPrefix {
                    or_replace: true,
                    options,
                    materialized: false,
                    recursive,
                    temporary,
                },
            );
        }
        // A function takes no `TEMP` prefix, so a consumed `temporary` rules it out
        // and leaves the keyword for the table/view productions below.
        if temporary.is_none()
            && self.features().statement_ddl_gates.routines
            && self.eat_contextual_keyword("FUNCTION")?
        {
            return self.parse_create_function(start, false, None);
        }
        // `CREATE [TEMP] RECURSIVE VIEW` — after the `TEMP`/`TEMPORARY` prefix the
        // `RECURSIVE` keyword is unambiguous (it can only introduce a recursive view),
        // gated to the dialects that model it (DuckDB/Lenient).
        let recursive = self.features().view_sequence_clause_syntax.recursive_views
            && self.eat_keyword(Keyword::Recursive)?;
        if recursive {
            self.expect_contextual_keyword("VIEW")?;
            return self.parse_create_view(
                start,
                ViewPrefix {
                    or_replace: false,
                    options: ViewOptions::default(),
                    materialized: false,
                    recursive: true,
                    temporary,
                },
            );
        }
        if self.eat_contextual_keyword("VIEW")? {
            return self.parse_create_view(
                start,
                ViewPrefix {
                    or_replace: false,
                    options: ViewOptions::default(),
                    materialized: false,
                    recursive: false,
                    temporary,
                },
            );
        }
        // `CREATE [TEMP] TRIGGER` (SQLite): a *whole-statement* DDL gate — unlike the
        // always-accepted DDL families, only SQLite's `BEGIN … END` body form is
        // modelled, and other dialects genuinely reject it, so it rides
        // `statement_ddl_gates.create_trigger` (off elsewhere, where `TRIGGER` falls
        // through to the `TABLE` expectation and surfaces as an unknown statement).
        if self.features().statement_ddl_gates.create_trigger
            && self.peek_is_contextual_keyword("TRIGGER")?
        {
            return self.parse_create_trigger(start, temporary);
        }
        // DuckDB `CREATE [PERSISTENT] SECRET` — a whole-statement gate (like the trigger and
        // macro gates): only dispatched under a dialect that models secrets, so elsewhere the
        // `PERSISTENT`/`SECRET` keyword falls through to the `TABLE` expectation below and
        // surfaces as an unknown statement. Guarded on `temporary.is_none()` because a secret
        // takes no `TEMP` prefix (the modelled forms are the bare and `PERSISTENT` spellings).
        if temporary.is_none()
            && self.features().statement_ddl_gates.create_secret
            && (self.peek_is_contextual_keyword("PERSISTENT")?
                || self.peek_is_contextual_keyword("SECRET")?)
        {
            return self.parse_create_secret(start);
        }
        // `CREATE [TEMP] SEQUENCE [IF NOT EXISTS] <name> [<option> ...]` (SQL:2003 T176;
        // PostgreSQL/DuckDB) — a whole-statement gate like the type/secret gates. Placed
        // *after* the `OR REPLACE` block above, so `CREATE OR REPLACE SEQUENCE` (DuckDB-only,
        // unmodelled) routes into that block and rejects at the `VIEW` expectation; the bare
        // and `TEMP` forms carry `or_replace == false` and reach here. Off elsewhere, where
        // `SEQUENCE` falls through to the `TABLE` expectation and surfaces as an unknown
        // statement.
        if self.features().statement_ddl_gates.create_sequence
            && self.eat_contextual_keyword("SEQUENCE")?
        {
            return self.parse_create_sequence(start, temporary);
        }
        self.expect_keyword(Keyword::Table)?;
        self.parse_create_table_rest(start, false, temporary, false)
    }

    /// Parse the `CREATE [OR REPLACE] [TEMP | UNLOGGED] TABLE` body after the introducing prefix.
    fn parse_create_table_rest(
        &mut self,
        start: Span,
        or_replace: bool,
        temporary: Option<TemporaryTableKind>,
        unlogged: bool,
    ) -> ParseResult<Statement<D::Ext>> {
        // DuckDB (and the create-type path) treat `OR REPLACE` and `IF NOT EXISTS` as
        // mutually exclusive: under `OR REPLACE` the engine reads `IF` as the table name
        // and fails at `NOT`. Only parse the guard when `or_replace` is off.
        let if_not_exists = !or_replace && self.parse_if_not_exists()?;
        let name = self.parse_target_relation_name()?;
        let (body, inherits, partition_by, access_method, options) =
            self.parse_create_table_tail()?;

        let span = start.union(self.preceding_span());
        let create_meta = self.make_meta(span);
        let create = CreateTable {
            or_replace,
            temporary,
            unlogged,
            if_not_exists,
            name,
            body,
            inherits,
            partition_by,
            access_method,
            options,
            meta: create_meta,
        };
        let meta = self.make_meta(span);
        Ok(Statement::CreateTable {
            create: Box::new(create),
            meta,
        })
    }

    /// Parse a SQLite `CREATE VIRTUAL TABLE [IF NOT EXISTS] <name> USING <module>
    /// [( <arg> [, <arg>] * )]` after the leading `CREATE VIRTUAL` (the `VIRTUAL` keyword is
    /// already consumed).
    ///
    /// The module owns the argument grammar, so the parenthesized list is captured as
    /// opaque verbatim [`ModuleArg`]s split on the top-level commas SQLite's own parser
    /// recognizes — see [`parse_module_arg`](Self::parse_module_arg). A bare `USING <module>`
    /// with no parens yields `args: None`; the parenthesized form yields `Some`, so the two
    /// round-trip distinctly.
    fn parse_create_virtual_table(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        self.expect_keyword(Keyword::Table)?;
        let if_not_exists = self.parse_if_not_exists()?;
        let name = self.parse_target_relation_name()?;
        self.expect_keyword(Keyword::Using)?;
        let module = self.parse_ident()?;
        let args = if self.peek_is_punct(Punctuation::LParen)? {
            Some(self.parse_module_args()?)
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let create = CreateVirtualTable {
            if_not_exists,
            name,
            module,
            args,
            meta: self.make_meta(span),
        };
        Ok(Statement::CreateVirtualTable {
            create: Box::new(create),
            meta: self.make_meta(span),
        })
    }

    /// Parse the parenthesized module-argument list of a `CREATE VIRTUAL TABLE`.
    ///
    /// SQLite splits the argument text on the *top-level* commas only; a comma inside
    /// nested parens or a quoted string does not separate arguments, and an empty member
    /// (`m(a,,b)`, a trailing comma) is a legal empty argument. This mirrors SQLite's
    /// `vtabarglist` grammar, which never interprets the argument text.
    fn parse_module_args(&mut self) -> ParseResult<ThinVec<ModuleArg>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the module argument list")?;
        let mut args = ThinVec::new();
        // The empty parenthesized form `USING m()` carries no arguments.
        if self.eat_punct(Punctuation::RParen)? {
            return Ok(args);
        }
        loop {
            args.push(self.parse_module_arg()?);
            if self.eat_punct(Punctuation::Comma)? {
                continue;
            }
            self.expect_punct(
                Punctuation::RParen,
                "`,` or `)` in the module argument list",
            )?;
            break;
        }
        Ok(args)
    }

    /// Parse one opaque module argument: the verbatim source text up to the next
    /// top-level `,` or the closing `)`, tracking nested-parenthesis depth so those
    /// delimiters inside the argument do not end it. Quoted strings and identifiers are
    /// single tokens already, so a comma or paren inside one is never seen here. The
    /// captured span runs from the argument's first token to its last, and an argument
    /// with no tokens (an empty member) is legal — SQLite's `vtabarg` may be empty.
    fn parse_module_arg(&mut self) -> ParseResult<ModuleArg> {
        // A zero-token member (an empty `vtabarg`) anchors its node at the boundary offset
        // and interns the empty slice; the first consumed token both opens and, until the
        // last, closes the captured span.
        let boundary = self.current_span()?.start();
        let mut span: Option<Span> = None;
        let mut depth: u32 = 0;
        loop {
            if depth == 0
                && (self.peek_is_punct(Punctuation::Comma)?
                    || self.peek_is_punct(Punctuation::RParen)?)
            {
                break;
            }
            let Some(token) = self.advance()? else {
                return Err(self.unexpected("`)` to close the module argument list"));
            };
            span = Some(match span {
                Some(open) => open.union(token.span),
                None => token.span,
            });
            match token.kind {
                TokenKind::Punctuation(Punctuation::LParen) => depth += 1,
                TokenKind::Punctuation(Punctuation::RParen) => depth -= 1,
                _ => {}
            }
        }
        let span = span.unwrap_or_else(|| Span::new(boundary, boundary));
        let text = self.intern_text(self.span_text(span));
        Ok(ModuleArg {
            text,
            meta: self.make_meta(span),
        })
    }

    /// Parse a DuckDB `CREATE [PERSISTENT] SECRET <name> ( <option> <value> [, ...] )`
    /// after the `CREATE` prefix (the `PERSISTENT`/`SECRET` lead is still at the cursor).
    fn parse_create_secret(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let persistent = self.eat_contextual_keyword("PERSISTENT")?;
        self.expect_contextual_keyword("SECRET")?;
        let name = self.parse_object_name()?;
        let options = self.parse_secret_options()?;
        let span = start.union(self.preceding_span());
        let create = CreateSecret {
            persistent,
            name,
            options,
            meta: self.make_meta(span),
        };
        Ok(Statement::CreateSecret {
            create: Box::new(create),
            meta: self.make_meta(span),
        })
    }

    /// Parse the always-parenthesized, non-empty `CREATE SECRET` option list.
    fn parse_secret_options(&mut self) -> ParseResult<ThinVec<SecretOption<D::Ext>>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the secret option list")?;
        let mut options = ThinVec::new();
        options.push(self.parse_secret_option()?);
        while self.eat_punct(Punctuation::Comma)? {
            options.push(self.parse_secret_option()?);
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the secret option list")?;
        Ok(options)
    }

    /// Parse one `<name> <value>` secret option (`TYPE S3`, `KEY_ID '...'`). The value is a
    /// general expression — the wider grammar is the recorded acceptance bound.
    fn parse_secret_option(&mut self) -> ParseResult<SecretOption<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        let value = self.parse_expr()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SecretOption { name, value, meta })
    }

    /// True if the tokens after `DROP` open a `DROP SECRET` — either a bare `SECRET` or an
    /// `opt_persist` (`PERSISTENT`/`TEMPORARY`) modifier immediately followed by `SECRET`.
    /// Two-token lookahead keeps a `DROP PERSISTENT …` / `DROP TEMPORARY …` that is *not* a
    /// secret drop falling through to the name-list DROP path.
    fn peek_is_drop_secret(&mut self) -> ParseResult<bool> {
        if self.peek_is_contextual_keyword("SECRET")? {
            return Ok(true);
        }
        if self.peek_is_contextual_keyword("PERSISTENT")?
            || self.peek_is_contextual_keyword("TEMPORARY")?
        {
            return self.peek_nth_is_contextual_keyword(1, "SECRET");
        }
        Ok(false)
    }

    /// Parse a DuckDB `DROP [PERSISTENT | TEMPORARY] SECRET [IF EXISTS] <name> [FROM <storage>]`
    /// after `DROP` (the `opt_persist`/`SECRET` lead is still at the cursor). `drop_secret.y`.
    fn parse_drop_secret(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let persistence = if self.eat_contextual_keyword("PERSISTENT")? {
            SecretPersistence::Persistent
        } else if self.eat_contextual_keyword("TEMPORARY")? {
            SecretPersistence::Temporary
        } else {
            SecretPersistence::Default
        };
        self.expect_contextual_keyword("SECRET")?;
        // `IF EXISTS` is part of this dedicated production (not the `schema_change_syntax`
        // DROP tail), so it is always available under the secrets gate.
        let if_exists = if self.eat_contextual_keyword("IF")? {
            self.expect_keyword(Keyword::Exists)?;
            true
        } else {
            false
        };
        let name = self.parse_ident()?;
        // `opt_storage_drop_specifier`: `FROM <backend>`, a single identifier naming the
        // secret-storage backend (grammar `IDENT`).
        let storage = if self.eat_contextual_keyword("FROM")? {
            Some(self.parse_ident()?)
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let drop = DropSecretStmt {
            persistence,
            if_exists,
            name,
            storage,
            meta: self.make_meta(span),
        };
        Ok(Statement::DropSecret {
            drop: Box::new(drop),
            meta: self.make_meta(span),
        })
    }

    /// Parse a DuckDB `CREATE [OR REPLACE] [TEMP] TYPE <name> AS <definition>` after the
    /// `TYPE` keyword, carrying the already-parsed `or_replace`/`temporary` prefix.
    ///
    /// `OR REPLACE` and `IF NOT EXISTS` are mutually exclusive in DuckDB's grammar: under
    /// `OR REPLACE` the parser reads `IF` as the type name, so the guard is admissible only
    /// on the plain form. Parsing `IF NOT EXISTS` only when `!or_replace` reproduces that
    /// exactly — a `CREATE OR REPLACE TYPE IF …` then binds `IF` as the name and the trailing
    /// `NOT` surfaces as the expected-`AS` parse error DuckDB reports.
    fn parse_create_type(
        &mut self,
        start: Span,
        or_replace: bool,
        temporary: Option<TemporaryTableKind>,
    ) -> ParseResult<Statement<D::Ext>> {
        let if_not_exists = !or_replace && self.parse_if_not_exists()?;
        let name = self.parse_object_name()?;
        self.expect_keyword(Keyword::As)?;
        let definition = self.parse_create_type_definition()?;
        let span = start.union(self.preceding_span());
        let create = CreateType {
            or_replace,
            temporary,
            if_not_exists,
            name,
            definition,
            meta: self.make_meta(span),
        };
        Ok(Statement::CreateType {
            create: Box::new(create),
            meta: self.make_meta(span),
        })
    }

    /// Parse the `AS <definition>` body of a `CREATE TYPE`.
    ///
    /// `ENUM(...)` opens DuckDB's dedicated enum production — a string-label list (possibly
    /// empty, `ENUM ()`) or a label-supplying query (`ENUM (SELECT …)`). Its labels are
    /// parser-restricted to string constants, distinct from the data-type-position
    /// `x::ENUM(...)` cast target (which accepts any modifier and only bind-rejects a
    /// non-string), so it is modelled apart from [`DataType::Enum`]. Every other spelling
    /// aliases a data type through the shared type grammar.
    fn parse_create_type_definition(&mut self) -> ParseResult<CreateTypeDefinition<D::Ext>> {
        let start = self.current_span()?;
        // The `ENUM(` head is the dedicated enum production; a bare `ENUM` (no `(`) or any
        // other keyword falls through to the aliased data type. The `(` lookahead mirrors
        // the composite-type constructor detection.
        if self.peek_is_contextual_keyword("ENUM")?
            && self.peek_nth_is_punct(1, Punctuation::LParen)?
        {
            self.advance()?; // ENUM
            self.expect_punct(Punctuation::LParen, "`(` to open the enum label list")?;
            // Three shapes share the parens: empty `()`, a string-literal label list, or a
            // label-supplying query. A `)` or a string is unambiguously the label form;
            // anything else (a `SELECT`/`VALUES`/`FROM`/`TABLE` head) is the query form.
            if self.eat_punct(Punctuation::RParen)? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(CreateTypeDefinition::Enum {
                    labels: ThinVec::new(),
                    meta,
                });
            }
            if self.peek_is_string()? {
                let labels = self.parse_comma_separated(Self::parse_enum_set_value)?;
                self.expect_punct(Punctuation::RParen, "`)` to close the enum label list")?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(CreateTypeDefinition::Enum { labels, meta });
            }
            let query = self.parse_query()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the enum label query")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(CreateTypeDefinition::EnumFromQuery {
                query: Box::new(query),
                meta,
            });
        }
        let data_type = self.parse_data_type()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CreateTypeDefinition::Alias { data_type, meta })
    }

    /// Parse a `CREATE SCHEMA [IF NOT EXISTS] [<name>] [AUTHORIZATION <role>]`.
    fn parse_create_schema(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let if_not_exists = self.parse_if_not_exists()?;
        // `AUTHORIZATION <role>` may stand alone (PostgreSQL derives the schema name
        // from the role) or trail a schema name; at least one is present.
        let (name, authorization) = if self.eat_contextual_keyword("AUTHORIZATION")? {
            (None, Some(self.parse_ident()?))
        } else {
            let name = self.parse_object_name()?;
            let authorization = if self.eat_contextual_keyword("AUTHORIZATION")? {
                Some(self.parse_ident()?)
            } else {
                None
            };
            (Some(name), authorization)
        };
        // The SQL-standard embedded schema-element list (`CREATE SCHEMA s CREATE TABLE t
        // ...`): the component objects created inside the new schema, modelled as children
        // so the whole construct stays ONE statement. Gated by `schema_elements`
        // (PostgreSQL/Lenient) — off elsewhere, a following `CREATE`/`GRANT` is left to the
        // top-level statement loop (the prior split behaviour). PostgreSQL forbids combining
        // this with `IF NOT EXISTS`, so the loop is skipped for the INE form (the guard
        // below then rejects any trailing element).
        let elements_gated = self.features().statement_ddl_gates.schema_elements;
        let mut elements = ThinVec::new();
        if elements_gated && !if_not_exists {
            while self.schema_element_starts()? {
                elements.push(self.parse_schema_element()?);
            }
        }
        // After the head (and any consumed elements) PostgreSQL's `CREATE SCHEMA` grammar
        // requires the statement to end — nothing but schema elements may follow. Two
        // strictness sources meet here:
        //   * `IF NOT EXISTS` forbids elements entirely, in *every* dialect
        //     ("CREATE SCHEMA IF NOT EXISTS cannot include schema elements"); a trailing
        //     token after the INE head is that rejected element.
        //   * Under the element gate, a trailing token that is *not* an admissible element
        //     (`CREATE MATERIALIZED VIEW`, `CREATE FUNCTION`, `SELECT`, …) is a syntax error
        //     rather than a new top-level statement, matching PostgreSQL's greedy production
        //     (the admissible-element loop above has already consumed every valid element).
        // Off-gate dialects keep the prior leniency: a trailing `CREATE`/`GRANT` there falls
        // through to the top-level statement loop unchanged.
        if (if_not_exists || elements_gated)
            && !self.is_eof()?
            && !self.peek_is_punct(Punctuation::Semicolon)?
        {
            let span = start.union(self.preceding_span());
            let expectation = if if_not_exists {
                "the end of the statement: `CREATE SCHEMA IF NOT EXISTS` cannot include an inline schema element"
            } else {
                "the end of the statement: a `CREATE SCHEMA` element must be a CREATE TABLE/VIEW/INDEX/SEQUENCE/TRIGGER or GRANT"
            };
            return Err(self.error_at(span, expectation, self.span_text(span).to_owned()));
        }
        let span = start.union(self.preceding_span());
        let schema_meta = self.make_meta(span);
        let schema = CreateSchema {
            if_not_exists,
            name,
            authorization,
            elements,
            meta: schema_meta,
        };
        let meta = self.make_meta(span);
        Ok(Statement::CreateSchema {
            schema: Box::new(schema),
            meta,
        })
    }

    /// Whether the cursor is at the start of a `CREATE SCHEMA` embedded element: a
    /// leading `CREATE` (the object kind is validated when the element is parsed) or a
    /// `GRANT`/`REVOKE`. Only consulted under the `schema_elements` gate.
    fn schema_element_starts(&mut self) -> ParseResult<bool> {
        Ok(self.peek_is_keyword(Keyword::Create)?
            || (self.features().access_control_syntax.access_control
                && self.peek_starts_access_control_statement()?))
    }

    /// Parse one embedded `CREATE SCHEMA` element and validate it against the closed
    /// admissible set (measured against PostgreSQL: `CREATE TABLE`/`VIEW`/`INDEX`/
    /// `SEQUENCE`/`TRIGGER` and `GRANT`). The element reuses the ordinary statement
    /// dispatch, so a `CREATE MATERIALIZED VIEW`/`FUNCTION` (which parses fine as a
    /// top-level statement) is rejected here post-hoc — PostgreSQL admits neither as a
    /// schema element. A non-modelled element form (e.g. PostgreSQL's `EXECUTE
    /// FUNCTION` trigger body, which this parser does not model) errors during the
    /// inner parse, rejecting the whole `CREATE SCHEMA` exactly as before.
    fn parse_schema_element(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let statement = self.parse_statement()?;
        let admissible = match &statement {
            Statement::CreateTable { .. }
            | Statement::CreateIndex { .. }
            | Statement::CreateSequence { .. }
            | Statement::CreateTrigger { .. }
            | Statement::AccessControl { .. } => true,
            // `CREATE MATERIALIZED VIEW` shares the view node; only the plain view is a
            // schema element.
            Statement::CreateView { view, .. } => !view.materialized,
            _ => false,
        };
        if !admissible {
            let span = start.union(self.preceding_span());
            return Err(self.error_at(
                span,
                "an admissible `CREATE SCHEMA` element (CREATE TABLE/VIEW/INDEX/SEQUENCE/TRIGGER or GRANT)",
                self.span_text(span).to_owned(),
            ));
        }
        Ok(statement)
    }

    /// Parse a `CREATE DATABASE [IF NOT EXISTS] <name>` after the `DATABASE` keyword.
    ///
    /// The `IF NOT EXISTS` guard is gated by
    /// [`ExistenceGuards::create_database_if_not_exists`](crate::ast::dialect::ExistenceGuards):
    /// when off (ANSI/PostgreSQL/SQLite), the flag short-circuits before `IF` is peeked,
    /// so the guard is left unconsumed and surfaces as a trailing-input parse error.
    fn parse_create_database(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let if_not_exists = self
            .features()
            .existence_guards
            .create_database_if_not_exists
            && self.parse_if_not_exists()?;
        let name = self.parse_object_name()?;
        let span = start.union(self.preceding_span());
        let create = CreateDatabase {
            if_not_exists,
            name,
            meta: self.make_meta(span),
        };
        Ok(Statement::CreateDatabase {
            create: Box::new(create),
            meta: self.make_meta(span),
        })
    }

    /// Parse a `CREATE [OR REPLACE] [DEFINER = <user>] FUNCTION [IF NOT EXISTS]` body after the
    /// `FUNCTION` keyword. `definer` is the already-parsed MySQL `DEFINER =` prefix (`None` on
    /// the PostgreSQL string-body path).
    fn parse_create_function(
        &mut self,
        start: Span,
        or_replace: bool,
        definer: Option<Definer>,
    ) -> ParseResult<Statement<D::Ext>> {
        // MySQL admits `IF NOT EXISTS` (8.0.29+); the PostgreSQL string-body routine never
        // reaches this gate (compound_statements off), so it stays `false` there.
        let if_not_exists = self.features().statement_ddl_gates.compound_statements
            && self.parse_if_not_exists()?;
        // A routine name is capped like a relation name: MySQL/SQLite (no catalog qualifier)
        // reject a three-part `a.b.c` routine name, matching the engine.
        let name = self.parse_target_relation_name()?;
        // A stored function's parameters carry no mode (all `IN`); modes stay gated by
        // `routine_arg_modes` (off for MySQL) so `mysql_rejects_function_parameter_modes` holds.
        let params = self.parse_function_params(false)?;
        // `RETURNS` opens the result type here, but the same keyword also opens the
        // `RETURNS NULL ON NULL INPUT` null-call option; a following `NULL` marks that
        // option, so leave it for the option loop below.
        let returns = if self.peek_is_contextual_keyword("RETURNS")?
            && !self.peek_nth_is_contextual_keyword(1, "NULL")?
        {
            self.expect_contextual_keyword("RETURNS")?;
            Some(self.parse_data_type()?)
        } else {
            None
        };
        let options = self.parse_function_options()?;
        // The trailing routine body (`opt_routine_body`) is the strictly-after-options slot:
        // the SQL-standard `RETURN <expr>` (all routine dialects), or — under MySQL SQL/PSM —
        // a `BEGIN … END` compound-statement body. `RETURN` in a stored FUNCTION body is
        // legal, so the body is parsed with returns permitted.
        let body = self.parse_routine_function_body()?;
        let span = start.union(self.preceding_span());
        let create = CreateFunction {
            or_replace,
            definer: definer.map(Box::new),
            if_not_exists,
            name,
            params,
            returns,
            options,
            body,
            meta: self.make_meta(span),
        };
        Ok(Statement::CreateFunction {
            create: Box::new(create),
            meta: self.make_meta(span),
        })
    }

    /// Parse a `CREATE [DEFINER = <user>] PROCEDURE [IF NOT EXISTS] <name> (<params>)
    /// [<characteristic> …] <routine_body>` after the `PROCEDURE` keyword (already consumed).
    /// MySQL SQL/PSM, gated by `compound_statements`. `definer` is the already-parsed prefix.
    fn parse_create_procedure(
        &mut self,
        start: Span,
        definer: Option<Definer>,
    ) -> ParseResult<Statement<D::Ext>> {
        let if_not_exists = self.parse_if_not_exists()?;
        let name = self.parse_target_relation_name()?;
        // A stored procedure's parameters carry the MySQL `[IN | OUT | INOUT]` mode grammar.
        let params = self.parse_function_params(true)?;
        let characteristics = self.parse_routine_characteristics(true)?;
        // A procedure body may not contain `RETURN` (server `ER_SP_BADRETURN`): parse it with
        // returns disallowed so a `RETURN` anywhere in the body rejects.
        let body = self.parse_routine_body_statement(false)?;
        let span = start.union(self.preceding_span());
        let create = CreateProcedure {
            definer: definer.map(Box::new),
            if_not_exists,
            name,
            params,
            characteristics,
            body: Box::new(body),
            meta: self.make_meta(span),
        };
        Ok(Statement::CreateProcedure {
            create: Box::new(create),
            meta: self.make_meta(span),
        })
    }

    /// Parse the optional `DEFINER = <user>` routine prefix (the `PROCEDURE`/`FUNCTION` keyword
    /// follows).
    ///
    /// The account reference is parsed through the shared account-name axis
    /// ([`parse_account_name`](Self::parse_account_name)), so the `@<host>` split (unquoted or
    /// quoted) and the `CURRENT_USER [()]` self-reference are recovered here exactly as the
    /// user/role DDL recovers them. [`Definer`] keeps its own shape-identical node (an
    /// `Account`/`CurrentUser` split mirroring [`AccountName`]) — the routine/trigger/event
    /// landings that consume it converge onto the shared node once they merge; until then this
    /// converts rather than reshaping the field type.
    fn parse_definer(&mut self) -> ParseResult<Definer> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("DEFINER")?;
        self.expect_op(Operator::Eq, "`=` after `DEFINER`")?;
        let account = self.parse_account_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(match account {
            AccountName::Account { user, host, .. } => Definer::Account { user, host, meta },
            AccountName::CurrentUser { parens, .. } => Definer::CurrentUser { parens, meta },
        })
    }

    /// Parse the trailing routine-function body slot — the SQL-standard `RETURN <expr>` (any
    /// routine dialect) or a MySQL SQL/PSM compound-statement body (`BEGIN … END`, a
    /// flow-control construct, a labelled block) — after the whole option/characteristic list.
    /// `None` when no body follows (a PostgreSQL `AS`-string routine).
    ///
    /// `RETURN <expr>` is checked first and rides the shared [`FunctionBody::Return`] slot for
    /// both dialects (a stored function's bare-return body is the same shape as PostgreSQL's);
    /// a compound/flow-control body rides [`FunctionBody::Block`]. A stored FUNCTION admits
    /// `RETURN`, so a `Block` body is parsed with returns permitted.
    fn parse_routine_function_body(&mut self) -> ParseResult<Option<Box<FunctionBody<D::Ext>>>> {
        let start = self.current_span()?;
        if self.eat_keyword(Keyword::Return)? {
            let expr = self.parse_expr()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(Box::new(FunctionBody::Return {
                expr: Box::new(expr),
                meta,
            })));
        }
        if self.features().statement_ddl_gates.compound_statements
            && self.peek_starts_body_construct()?
        {
            let body = self.parse_routine_body_statement(true)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(Box::new(FunctionBody::Block {
                body: Box::new(body),
                meta,
            })));
        }
        Ok(None)
    }

    /// Parse one MySQL routine body statement through the `parse_body_statement` seam, with
    /// `RETURN` permitted iff `returns_allowed` (a stored FUNCTION body allows it; a PROCEDURE
    /// body rejects it — server `ER_SP_BADRETURN`). The `body_return_allowed` parser flag is
    /// saved and restored around the parse so nested parses are unaffected.
    fn parse_routine_body_statement(
        &mut self,
        returns_allowed: bool,
    ) -> ParseResult<Statement<D::Ext>> {
        let saved = self.body_return_allowed;
        self.body_return_allowed = returns_allowed;
        let result = self.parse_body_statement();
        self.body_return_allowed = saved;
        result
    }

    /// Parse the order-independent MySQL routine characteristic list (`LANGUAGE SQL`,
    /// `[NOT] DETERMINISTIC`, `CONTAINS SQL`/`NO SQL`/`READS SQL DATA`/`MODIFIES SQL DATA`,
    /// `SQL SECURITY {DEFINER | INVOKER}`, `COMMENT '…'`), on the shared [`FunctionOption`]
    /// axis. `allow_deterministic` is off on `ALTER` (the server rejects `DETERMINISTIC`
    /// there, so it is left unconsumed and surfaces as a clean parse error).
    fn parse_routine_characteristics(
        &mut self,
        allow_deterministic: bool,
    ) -> ParseResult<ThinVec<FunctionOption<D::Ext>>> {
        let mut characteristics = ThinVec::new();
        while let Some(characteristic) = self.parse_routine_characteristic(allow_deterministic)? {
            characteristics.push(characteristic);
        }
        Ok(characteristics)
    }

    /// Parse one MySQL routine characteristic, or `None` when the next token opens none.
    fn parse_routine_characteristic(
        &mut self,
        allow_deterministic: bool,
    ) -> ParseResult<Option<FunctionOption<D::Ext>>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("LANGUAGE")? {
            let name = self.parse_routine_language_name()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(FunctionOption::Language { name, meta }));
        }
        if self.eat_contextual_keyword("COMMENT")? {
            let comment = self.expect_string_literal("a routine COMMENT string literal")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(FunctionOption::Comment { comment, meta }));
        }
        if allow_deterministic {
            if self.eat_contextual_keyword("NOT")? {
                self.expect_contextual_keyword("DETERMINISTIC")?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(Some(FunctionOption::Deterministic { not: true, meta }));
            }
            if self.eat_contextual_keyword("DETERMINISTIC")? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(Some(FunctionOption::Deterministic { not: false, meta }));
            }
        }
        if let Some(access) = self.parse_sql_data_access()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(FunctionOption::DataAccess { access, meta }));
        }
        if self.eat_contextual_keyword("SQL")? {
            self.expect_contextual_keyword("SECURITY")?;
            let context = if self.eat_contextual_keyword("DEFINER")? {
                SqlSecurityContext::Definer
            } else {
                self.expect_contextual_keyword("INVOKER")?;
                SqlSecurityContext::Invoker
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(FunctionOption::SqlSecurity { context, meta }));
        }
        Ok(None)
    }

    /// Parse the `LANGUAGE <name>` argument: PostgreSQL's `NonReservedWord_or_Sconst`, a bare
    /// non-reserved word or an `Sconst` string constant, into the shared [`LanguageName`] union.
    ///
    /// The string arm (`LANGUAGE 'sql'`/`E'sql'`/`$$sql$$`) is gated on
    /// [`routine_language_string`](squonk_ast::dialect::IndexAlterSyntax::routine_language_string):
    /// on for PostgreSQL, off for MySQL, whose routine `LANGUAGE` admits only the bare word
    /// `SQL` (`LANGUAGE 'SQL'` is engine-measured `ER_PARSE_ERROR` on mysql:8). Like the
    /// `DO … LANGUAGE` operand it admits only an `Sconst`; a bit/hex/national constant falls
    /// through to the bare-word parse, which rejects it as PostgreSQL does.
    ///
    /// The bare-word arm's reserved set is separately dialect-gated: MySQL's routine language is
    /// the reserved word `SQL` (the only value the server accepts), so under the stored-program
    /// surface the name position admits any word; PostgreSQL keeps its `NonReservedWord` language
    /// name (the `reserved_column_name` set), where a reserved word is the syntax error it reports.
    fn parse_routine_language_name(&mut self) -> ParseResult<LanguageName> {
        let start = self.current_span()?;
        if self.features().index_alter_syntax.routine_language_string {
            if let Some(token) = self.peek()? {
                if token.kind == TokenKind::String
                    && string_literal_is_sconst(self.span_text(token.span))
                {
                    let value = self.expect_string_literal("a routine language name")?;
                    let meta = self.make_meta(start.union(self.preceding_span()));
                    return Ok(LanguageName::String { value, meta });
                }
            }
        }
        let reserved = if self.features().statement_ddl_gates.compound_statements {
            KeywordSet::EMPTY
        } else {
            self.features().reserved_column_name
        };
        let word = self.parse_ident_admitting(reserved, "a routine language name")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(LanguageName::Word { word, meta })
    }

    /// Parse a MySQL SQL-data-access characteristic (`CONTAINS SQL` / `NO SQL` /
    /// `READS SQL DATA` / `MODIFIES SQL DATA`), or `None`. `NO SQL` is distinguished from the
    /// `SQL SECURITY` characteristic by requiring the leading `NO`/`CONTAINS`/`READS`/`MODIFIES`
    /// keyword — a bare `SQL` opens `SQL SECURITY` instead.
    fn parse_sql_data_access(&mut self) -> ParseResult<Option<SqlDataAccess>> {
        if self.eat_contextual_keyword("CONTAINS")? {
            self.expect_contextual_keyword("SQL")?;
            return Ok(Some(SqlDataAccess::ContainsSql));
        }
        if self.eat_contextual_keyword("NO")? {
            self.expect_contextual_keyword("SQL")?;
            return Ok(Some(SqlDataAccess::NoSql));
        }
        if self.eat_contextual_keyword("READS")? {
            self.expect_contextual_keyword("SQL")?;
            self.expect_contextual_keyword("DATA")?;
            return Ok(Some(SqlDataAccess::ReadsSqlData));
        }
        if self.eat_contextual_keyword("MODIFIES")? {
            self.expect_contextual_keyword("SQL")?;
            self.expect_contextual_keyword("DATA")?;
            return Ok(Some(SqlDataAccess::ModifiesSqlData));
        }
        Ok(None)
    }

    /// Parse the always-parenthesized `CREATE FUNCTION` parameter list.
    fn parse_function_params(
        &mut self,
        mysql_proc_modes: bool,
    ) -> ParseResult<ThinVec<FunctionParam<D::Ext>>> {
        self.expect_punct(
            Punctuation::LParen,
            "`(` to open the function parameter list",
        )?;
        let mut params = ThinVec::new();
        if !self.peek_is_punct(Punctuation::RParen)? {
            params.push(self.parse_function_param(mysql_proc_modes)?);
            while self.eat_punct(Punctuation::Comma)? {
                params.push(self.parse_function_param(mysql_proc_modes)?);
            }
        }
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the function parameter list",
        )?;
        Ok(params)
    }

    /// Parse one `[mode] [name] <type> [DEFAULT <expr> | = <expr>]` parameter
    /// (PostgreSQL `func_arg`). The optional argument mode (`arg_class`) leads; the name
    /// follows in the `type_function_name` class, so a `type_func_name` keyword like
    /// `left` is a valid name while a `col_name` keyword like `int` is not — the
    /// speculative `name`-then-`type` parse therefore falls back to a bare type exactly
    /// when the first token cannot be a name (a lone `int`), matching how PostgreSQL
    /// resolves the name-vs-type ambiguity. The trailing default (PostgreSQL
    /// `func_arg_with_default`) is gated by
    /// [`routine_arg_defaults`](squonk_ast::dialect::IndexAlterSyntax::routine_arg_defaults);
    /// with the gate off the `DEFAULT`/`=` is left unconsumed and the parameter-list
    /// close surfaces it as a clean parse error (MySQL's no-default routine args).
    fn parse_function_param(
        &mut self,
        mysql_proc_modes: bool,
    ) -> ParseResult<FunctionParam<D::Ext>> {
        let start = self.current_span()?;
        let mode = self.parse_function_param_mode(mysql_proc_modes)?;
        let checkpoint = self.checkpoint();
        let named = match self.parse_type_function_name_ident() {
            Ok(name) => match self.parse_data_type() {
                Ok(data_type) => Some((Some(name), data_type)),
                Err(_) => None,
            },
            Err(_) => None,
        };
        let (name, data_type) = match named {
            Some(named) => named,
            None => {
                self.rewind(checkpoint);
                (None, self.parse_data_type()?)
            }
        };
        let default = self.parse_function_param_default()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(FunctionParam {
            mode,
            name,
            data_type,
            default,
            meta,
        })
    }

    /// Parse the optional `CREATE FUNCTION` argument mode (`arg_class`) — the
    /// `IN`/`OUT`/`INOUT`/`VARIADIC` prefix before a parameter (PostgreSQL `func_arg`),
    /// gated by
    /// [`routine_arg_modes`](squonk_ast::dialect::IndexAlterSyntax::routine_arg_modes).
    /// `IN`/`VARIADIC` are reserved and `OUT`/`INOUT` are `col_name` keywords the
    /// `type_function_name` name position rejects, so a leading mode keyword can never be
    /// the parameter's own name or type — no speculative rewind is needed. With the gate
    /// off the keyword is left for the name/type parse and a reserved mode word surfaces
    /// as a clean parse error (MySQL's mode-less `CREATE FUNCTION` args).
    ///
    /// `mysql_proc_modes` opens the MySQL stored-**procedure** mode grammar even where the
    /// PostgreSQL `routine_arg_modes` gate is off: a MySQL procedure parameter is
    /// `[IN | OUT | INOUT] name type` (a stored *function*'s params are always `IN`, so the
    /// gate stays off there — the `mysql_rejects_function_parameter_modes` pin). `VARIADIC` is
    /// PostgreSQL-only, so it is admitted only under the PostgreSQL gate, never for a procedure.
    fn parse_function_param_mode(
        &mut self,
        mysql_proc_modes: bool,
    ) -> ParseResult<Option<FunctionParamMode>> {
        let pg_modes = self.features().index_alter_syntax.routine_arg_modes;
        if !pg_modes && !mysql_proc_modes {
            return Ok(None);
        }
        let mode = if self.eat_keyword(Keyword::In)? {
            FunctionParamMode::In
        } else if self.eat_keyword(Keyword::Out)? {
            FunctionParamMode::Out
        } else if self.eat_keyword(Keyword::Inout)? {
            FunctionParamMode::InOut
        } else if pg_modes && self.eat_keyword(Keyword::Variadic)? {
            FunctionParamMode::Variadic
        } else {
            return Ok(None);
        };
        Ok(Some(mode))
    }

    /// Parse the optional routine-parameter default tail — `DEFAULT <expr>` or `=
    /// <expr>` (PostgreSQL `func_arg_with_default`), gated by
    /// [`routine_arg_defaults`](squonk_ast::dialect::IndexAlterSyntax::routine_arg_defaults).
    /// The spelling tag records which form the source used so both round-trip. This is
    /// definition-site parameter metadata and shares no code with the ordinary
    /// function-**call** argument path (`parse_function_arg` / `FunctionArg`).
    fn parse_function_param_default(
        &mut self,
    ) -> ParseResult<Option<Box<FunctionParamDefault<D::Ext>>>> {
        if !self.features().index_alter_syntax.routine_arg_defaults {
            return Ok(None);
        }
        let start = self.current_span()?;
        let spelling = if self.eat_keyword(Keyword::Default)? {
            FunctionParamDefaultSpelling::Default
        } else if self.eat_op(Operator::Eq)? {
            FunctionParamDefaultSpelling::Equals
        } else {
            return Ok(None);
        };
        let value = self.parse_expr()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(Box::new(FunctionParamDefault {
            spelling,
            value,
            meta,
        })))
    }

    /// Parse the order-independent `CREATE FUNCTION` option/characteristic list (`LANGUAGE` /
    /// `AS` / null-call behaviour, plus the MySQL routine characteristics under
    /// `compound_statements`). The trailing routine body (`RETURN <expr>` or a MySQL compound
    /// block) is *not* an option — it is parsed separately by
    /// [`parse_routine_function_body`](Self::parse_routine_function_body) after this loop, since
    /// it must follow the whole list.
    fn parse_function_options(&mut self) -> ParseResult<ThinVec<FunctionOption<D::Ext>>> {
        let mut options = ThinVec::new();
        while let Some(option) = self.parse_function_option()? {
            options.push(option);
        }
        Ok(options)
    }

    fn parse_function_option(&mut self) -> ParseResult<Option<FunctionOption<D::Ext>>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("LANGUAGE")? {
            let name = self.parse_routine_language_name()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(FunctionOption::Language { name, meta }));
        }
        if self.eat_keyword(Keyword::As)? {
            let definition = self.expect_string_literal("a string function body after `AS`")?;
            // The body kind rides the `FunctionBody` axis; a `Definition` (opaque source
            // string) is the only kind today. Its span is the literal's own slice — for a
            // dollar body `$tag$…$tag$` that covers the delimiters verbatim, the fidelity the
            // round-trip relies on.
            let body = FunctionBody::Definition {
                meta: self.make_meta(definition.meta.span),
                definition,
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(FunctionOption::As { body, meta }));
        }
        if let Some(behavior) = self.parse_function_null_behavior()? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(FunctionOption::NullBehavior { behavior, meta }));
        }
        // The MySQL SQL/PSM routine characteristics (`[NOT] DETERMINISTIC`, the SQL-data-access
        // class, `SQL SECURITY …`, `COMMENT '…'`) ride the same option list, gated on the
        // stored-program surface. `LANGUAGE` is already handled above (its PostgreSQL arm), so
        // the characteristic parser only contributes the MySQL-specific arms here.
        if self.features().statement_ddl_gates.compound_statements {
            return self.parse_routine_characteristic(true);
        }
        Ok(None)
    }

    /// Parse the null-call behaviour option: `STRICT` / `CALLED ON NULL INPUT` /
    /// `RETURNS NULL ON NULL INPUT`.
    fn parse_function_null_behavior(&mut self) -> ParseResult<Option<FunctionNullBehavior>> {
        if self.eat_contextual_keyword("STRICT")? {
            return Ok(Some(FunctionNullBehavior::Strict));
        }
        if self.eat_contextual_keyword("CALLED")? {
            self.expect_contextual_keyword("ON")?;
            self.expect_keyword(Keyword::Null)?;
            self.expect_contextual_keyword("INPUT")?;
            return Ok(Some(FunctionNullBehavior::CalledOnNull));
        }
        if self.peek_is_contextual_keyword("RETURNS")?
            && self.peek_nth_is_contextual_keyword(1, "NULL")?
        {
            self.expect_contextual_keyword("RETURNS")?;
            self.expect_keyword(Keyword::Null)?;
            self.expect_contextual_keyword("ON")?;
            self.expect_keyword(Keyword::Null)?;
            self.expect_contextual_keyword("INPUT")?;
            return Ok(Some(FunctionNullBehavior::ReturnsNullOnNull));
        }
        Ok(None)
    }

    /// Parse a DuckDB `CREATE [OR REPLACE] [TEMP] {MACRO | FUNCTION} [IF NOT EXISTS]
    /// <name>(<params>) AS <expr> | AS TABLE <query>` body after its introducing keyword
    /// (`MACRO`, or the `FUNCTION` synonym) has been consumed. `or_replace` / `temporary`
    /// are the already-parsed `CREATE` prefix; `spelling` records which keyword led so the
    /// source form round-trips.
    fn parse_create_macro(
        &mut self,
        start: Span,
        or_replace: bool,
        temporary: Option<TemporaryTableKind>,
        spelling: MacroSpelling,
    ) -> ParseResult<Statement<D::Ext>> {
        let if_not_exists = self.parse_if_not_exists()?;
        let name = self.parse_object_name()?;
        let params = self.parse_macro_params()?;
        self.expect_keyword(Keyword::As)?;
        let body = self.parse_macro_body()?;
        let span = start.union(self.preceding_span());
        let create = CreateMacro {
            or_replace,
            temporary,
            spelling,
            if_not_exists,
            name,
            params,
            body,
            meta: self.make_meta(span),
        };
        Ok(Statement::CreateMacro {
            create: Box::new(create),
            meta: self.make_meta(span),
        })
    }

    /// Parse the always-parenthesized macro parameter list: bare untyped names, each with
    /// an optional `:= <default>` (`m(a, b := 10)`). Unlike a routine parameter, a macro
    /// parameter carries no type.
    fn parse_macro_params(&mut self) -> ParseResult<ThinVec<MacroParam<D::Ext>>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the macro parameter list")?;
        let mut params = ThinVec::new();
        if !self.peek_is_punct(Punctuation::RParen)? {
            params.push(self.parse_macro_param()?);
            while self.eat_punct(Punctuation::Comma)? {
                params.push(self.parse_macro_param()?);
            }
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the macro parameter list")?;
        Ok(params)
    }

    /// Parse one macro parameter: a bare name with an optional `:= <default>` value.
    fn parse_macro_param(&mut self) -> ParseResult<MacroParam<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        let default = if self.eat_colon_equals()? {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(MacroParam {
            name,
            default,
            meta,
        })
    }

    /// Parse a macro body after its introducing `AS`: `TABLE <query>` for a table macro,
    /// otherwise a scalar `<expr>` (which may itself be a parenthesized subquery).
    fn parse_macro_body(&mut self) -> ParseResult<MacroBody<D::Ext>> {
        let start = self.current_span()?;
        if self.eat_keyword(Keyword::Table)? {
            let query = self.parse_query()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(MacroBody::Table {
                query: Box::new(query),
                meta,
            })
        } else {
            let expr = self.parse_expr()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(MacroBody::Scalar {
                expr: Box::new(expr),
                meta,
            })
        }
    }

    /// Eat a `:=` operator token (the macro-parameter default separator), returning
    /// whether one was present. Independent of the named-argument *call* gate — this is
    /// macro-definition grammar, not a function call.
    fn eat_colon_equals(&mut self) -> ParseResult<bool> {
        if matches!(self.peek()?, Some(token) if token.kind == TokenKind::Operator(Operator::ColonEquals))
        {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Decide whether a `CREATE [OR REPLACE] FUNCTION` at the cursor is DuckDB's live-body
    /// macro (`(names) AS <expr> | TABLE <query>`) rather than the string-body routine
    /// that shares the `CREATE … (params) AS` prefix. Reached only when both `create_macro`
    /// and `routines` are on (DuckDB / Lenient), where the `FUNCTION` keyword is genuinely
    /// ambiguous. A bounded speculative parse — always rewound — settles it; the STOP
    /// condition on the ticket named exactly this "lookahead past `AS`", resolved here by a
    /// single-token string-literal peek.
    fn create_function_is_macro(&mut self) -> ParseResult<bool> {
        let checkpoint = self.checkpoint();
        let verdict = self.macro_function_lookahead().unwrap_or(false);
        self.rewind(checkpoint);
        Ok(verdict)
    }

    /// The speculative body of [`create_function_is_macro`](Self::create_function_is_macro):
    /// consume the `FUNCTION` keyword, the name, and a *macro-shaped* parameter list, then
    /// inspect the body tail. Returns `Ok(false)` for any non-macro shape — a *typed*
    /// parameter (`f(a INT)`, the routine shape), a `RETURNS`/option tail with no bare `AS`,
    /// or an `AS '<string>'` routine body — the caller rewinds regardless of the answer.
    fn macro_function_lookahead(&mut self) -> ParseResult<bool> {
        self.expect_contextual_keyword("FUNCTION")?;
        if self.parse_object_name().is_err() {
            return Ok(false);
        }
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok(false);
        }
        if !self.peek_is_punct(Punctuation::RParen)? {
            loop {
                // A macro parameter is a bare name; a typed routine parameter (`a INT`)
                // leaves a stray token here and fails the `,`/`)`/`:=` shape below.
                if self.parse_ident().is_err() {
                    return Ok(false);
                }
                if self.eat_colon_equals()? && self.parse_expr().is_err() {
                    return Ok(false);
                }
                if !self.eat_punct(Punctuation::Comma)? {
                    break;
                }
            }
        }
        if !self.eat_punct(Punctuation::RParen)? {
            return Ok(false);
        }
        // A macro body is introduced by a bare `AS`; a routine tail (`RETURNS …`,
        // `LANGUAGE …`, volatility options) has none, and an `AS '<string>'` is the routine
        // string body — a string-literal token immediately after `AS` reads as a routine.
        if !self.eat_keyword(Keyword::As)? {
            return Ok(false);
        }
        let after_as_is_string =
            matches!(self.peek()?, Some(token) if token.kind == TokenKind::String);
        Ok(!after_as_is_string)
    }

    /// Parse a view body after its `CREATE [...] VIEW` prefix has been consumed.
    fn parse_create_view(
        &mut self,
        start: Span,
        prefix: ViewPrefix,
    ) -> ParseResult<Statement<D::Ext>> {
        // MySQL has temporary *tables* but no temporary *views*, so a consumed `TEMP`/
        // `TEMPORARY` prefix leading into `VIEW` is the syntax error MySQL reports
        // ([`ViewSequenceClauseSyntax::temporary_views`] off); the dialects that spell
        // session-local views (PostgreSQL/SQLite/DuckDB) keep the flag on.
        if prefix.temporary.is_some()
            && !self.features().view_sequence_clause_syntax.temporary_views
        {
            let span = start.union(self.preceding_span());
            let found = self.span_text(span).to_owned();
            return Err(self.error_at(
                span,
                "a `CREATE VIEW` without `TEMPORARY` (this dialect has no temporary views)",
                found,
            ));
        }
        // PostgreSQL accepts `IF NOT EXISTS` only on a materialized view (a regular
        // view uses `OR REPLACE`); SQLite spells it over a plain view too, gated by
        // `existence_guards.view_if_not_exists`. When neither admits it, the guard
        // is left unconsumed and surfaces as a clean parse error.
        let allows_if_not_exists =
            prefix.materialized || self.features().existence_guards.view_if_not_exists;
        let if_not_exists = allows_if_not_exists && self.parse_if_not_exists()?;
        let name = self.parse_object_name()?;
        let columns = if self.peek_is_punct(Punctuation::LParen)? {
            self.parse_parenthesized_ident_list(
                "`(` to open the view column list",
                "`)` to close the view column list",
            )?
        } else {
            ThinVec::new()
        };
        // A recursive view desugars to `WITH RECURSIVE`, which must name its output
        // columns, so the engine requires the explicit list (it rejects the bare
        // `CREATE RECURSIVE VIEW v AS …`). Enforced here to keep the accept bound exact.
        if prefix.recursive && columns.is_empty() {
            return Err(self.unexpected("a `(` to open the recursive view's required column list"));
        }
        let to = if prefix.materialized
            && self
                .features()
                .view_sequence_clause_syntax
                .materialized_view_to
            && self.eat_contextual_keyword("TO")?
        {
            Some(self.parse_object_name()?)
        } else {
            None
        };
        self.expect_keyword(Keyword::As)?;
        let query = self.parse_query()?;

        // The tails are mutually exclusive: `WITH [NO] DATA` populates a materialized
        // view, `WITH [CASCADED|LOCAL] CHECK OPTION` constrains a regular one.
        let (check_option, with_data) = if prefix.materialized {
            (None, self.parse_with_data()?)
        } else {
            (self.parse_view_check_option()?, None)
        };

        let span = start.union(self.preceding_span());
        let view_meta = self.make_meta(span);
        let view = CreateView {
            or_replace: prefix.or_replace,
            options: prefix.options,
            materialized: prefix.materialized,
            recursive: prefix.recursive,
            temporary: prefix.temporary,
            if_not_exists,
            name,
            columns,
            to,
            query: Box::new(query),
            check_option,
            with_data,
            meta: view_meta,
        };
        let meta = self.make_meta(span);
        Ok(Statement::CreateView {
            view: Box::new(view),
            meta,
        })
    }

    /// Parse a `WITH [ CASCADED | LOCAL ] CHECK OPTION` view constraint, if present.
    fn parse_view_check_option(&mut self) -> ParseResult<Option<ViewCheckOption>> {
        if !self.eat_keyword(Keyword::With)? {
            return Ok(None);
        }
        let option = if self.eat_contextual_keyword("CASCADED")? {
            ViewCheckOption::Cascaded
        } else if self.eat_contextual_keyword("LOCAL")? {
            ViewCheckOption::Local
        } else {
            ViewCheckOption::Unspecified
        };
        self.expect_contextual_keyword("CHECK")?;
        self.expect_contextual_keyword("OPTION")?;
        Ok(Some(option))
    }

    /// Parse the MySQL `ALGORITHM = { UNDEFINED | MERGE | TEMPTABLE }` view-processing algorithm
    /// (the `ALGORITHM` keyword already confirmed as the current token by the caller).
    fn parse_view_algorithm(&mut self) -> ParseResult<ViewAlgorithm> {
        self.expect_contextual_keyword("ALGORITHM")?;
        self.expect_op(Operator::Eq, "`=` after `ALGORITHM`")?;
        if self.eat_contextual_keyword("UNDEFINED")? {
            Ok(ViewAlgorithm::Undefined)
        } else if self.eat_contextual_keyword("MERGE")? {
            Ok(ViewAlgorithm::Merge)
        } else if self.eat_contextual_keyword("TEMPTABLE")? {
            Ok(ViewAlgorithm::TempTable)
        } else {
            Err(self.unexpected("`UNDEFINED`, `MERGE`, or `TEMPTABLE` after `ALGORITHM =`"))
        }
    }

    /// Parse the optional MySQL `SQL SECURITY { DEFINER | INVOKER }` view security clause — the
    /// last of the [`ViewOptions`] prefix (MySQL's `view_suid`, which sits immediately before the
    /// `VIEW` keyword). `None` when no `SQL SECURITY` clause is present.
    fn parse_optional_view_sql_security(&mut self) -> ParseResult<Option<SqlSecurityContext>> {
        if !self.eat_contextual_keyword("SQL")? {
            return Ok(None);
        }
        self.expect_contextual_keyword("SECURITY")?;
        let context = if self.eat_contextual_keyword("DEFINER")? {
            SqlSecurityContext::Definer
        } else {
            self.expect_contextual_keyword("INVOKER")?;
            SqlSecurityContext::Invoker
        };
        Ok(Some(context))
    }

    /// Parse the MySQL view definition-option prefix `[ALGORITHM = …] [DEFINER = …]
    /// [SQL SECURITY …]` in its fixed source order, up to (not including) the `VIEW` keyword.
    /// All-`None` when no option is present. The order is engine-required — a keyword written
    /// out of order is left unconsumed for the `VIEW` expectation and surfaces as a clean parse
    /// error, matching the server's `ER_PARSE_ERROR`.
    fn parse_view_option_prefix(&mut self) -> ParseResult<ViewOptions> {
        let algorithm = if self.peek_is_contextual_keyword("ALGORITHM")? {
            Some(self.parse_view_algorithm()?)
        } else {
            None
        };
        let definer = if self.peek_is_contextual_keyword("DEFINER")? {
            Some(Box::new(self.parse_definer()?))
        } else {
            None
        };
        let sql_security = self.parse_optional_view_sql_security()?;
        Ok(ViewOptions {
            algorithm,
            definer,
            sql_security,
        })
    }

    /// Parse the tail of a MySQL `ALTER … VIEW <name> [(<columns>)] AS <query> [WITH
    /// [CASCADED | LOCAL] CHECK OPTION]` redefinition after the option prefix, the `VIEW`
    /// keyword, and the `name` have all been consumed by the dispatcher. `options` carries the
    /// `[ALGORITHM] [DEFINER] [SQL SECURITY]` prefix (all-`None` for a bare `ALTER VIEW`). MySQL
    /// takes no `IF EXISTS` here — the dispatcher never admits it — and no
    /// `TEMP`/`MATERIALIZED`/`RECURSIVE` (MySQL has no such views), so this is the
    /// [`parse_create_view`](Self::parse_create_view) body minus those prefixes and the
    /// `WITH DATA` materialized tail.
    fn parse_alter_view(
        &mut self,
        start: Span,
        options: ViewOptions,
        name: ObjectName,
    ) -> ParseResult<Statement<D::Ext>> {
        let columns = if self.peek_is_punct(Punctuation::LParen)? {
            self.parse_parenthesized_ident_list(
                "`(` to open the view column list",
                "`)` to close the view column list",
            )?
        } else {
            ThinVec::new()
        };
        self.expect_keyword(Keyword::As)?;
        let query = self.parse_query()?;
        let check_option = self.parse_view_check_option()?;

        let span = start.union(self.preceding_span());
        let alter_meta = self.make_meta(span);
        let alter = AlterView {
            options,
            name,
            columns,
            query: Box::new(query),
            check_option,
            meta: alter_meta,
        };
        let meta = self.make_meta(span);
        Ok(Statement::AlterView {
            alter: Box::new(alter),
            meta,
        })
    }

    /// Parse a `CREATE [UNIQUE] INDEX ...` statement.
    fn parse_create_index(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let unique = self.eat_contextual_keyword("UNIQUE")?;
        self.expect_contextual_keyword("INDEX")?;
        // `CONCURRENTLY`, the `USING` access method, and the partial `WHERE` are
        // PostgreSQL clauses each gated by dialect data (ADR-0011); with a flag off
        // the keyword is left for the next production and surfaces as a parse error.
        let concurrently = self.features().index_alter_syntax.index_concurrently
            && self.eat_contextual_keyword("CONCURRENTLY")?;
        // MySQL has no `CREATE INDEX IF NOT EXISTS`; with the gate off the guard is left
        // unconsumed and the following index name surfaces as a clean parse error.
        let if_not_exists =
            self.features().index_alter_syntax.index_if_not_exists && self.parse_if_not_exists()?;
        // The index name is optional: PostgreSQL derives one when `ON` follows
        // directly (`CREATE INDEX ON t (a)`). `IF NOT EXISTS` always names an index.
        let name = if !if_not_exists && self.peek_is_keyword(Keyword::On)? {
            None
        } else {
            Some(self.parse_ident()?)
        };
        self.expect_keyword(Keyword::On)?;
        let table = self.parse_target_relation_name()?;
        let using = if self.features().index_alter_syntax.index_using_method
            && self.eat_contextual_keyword("USING")?
        {
            Some(self.parse_ident()?)
        } else {
            None
        };
        let columns = self.parse_index_columns()?;
        let with_params = if self.features().index_alter_syntax.index_storage_parameters
            && self.eat_keyword(Keyword::With)?
        {
            self.parse_reloptions_list()?
        } else {
            ThinVec::new()
        };
        let predicate = if self.features().index_alter_syntax.partial_index
            && self.eat_keyword(Keyword::Where)?
        {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let index_meta = self.make_meta(span);
        let index = CreateIndex {
            unique,
            concurrently,
            if_not_exists,
            name,
            table,
            using,
            columns,
            with_params,
            predicate,
            meta: index_meta,
        };
        let meta = self.make_meta(span);
        Ok(Statement::CreateIndex {
            index: Box::new(index),
            meta,
        })
    }

    /// Parse a `CREATE [TEMP] TRIGGER [IF NOT EXISTS] [<schema>.]<name>
    /// [BEFORE | AFTER | INSTEAD OF] <event> ON <table> [FOR EACH ROW] [WHEN <expr>]
    /// BEGIN <stmt>; ... END` after the `CREATE [TEMP]` prefix, reached under
    /// [`StatementDdlGates::create_trigger`](crate::ast::dialect::StatementDdlGates).
    ///
    /// The timing keyword is optional (SQLite defaults it to `BEFORE`); `FOR EACH ROW`
    /// is an accepted-and-ignored surface tag (SQLite rejects `FOR EACH STATEMENT`);
    /// the body is parsed by [`parse_trigger_body`](Self::parse_trigger_body).
    fn parse_create_trigger(
        &mut self,
        start: Span,
        temporary: Option<TemporaryTableKind>,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("TRIGGER")?;
        let if_not_exists = self.parse_if_not_exists()?;
        let name = self.parse_target_relation_name()?;
        let timing = self.parse_optional_trigger_timing()?;
        let event = self.parse_trigger_event()?;
        self.expect_keyword(Keyword::On)?;
        let table = self.parse_target_relation_name()?;
        let for_each_row = if self.eat_contextual_keyword("FOR")? {
            self.expect_contextual_keyword("EACH")?;
            self.expect_contextual_keyword("ROW")?;
            true
        } else {
            false
        };
        let when = if self.eat_contextual_keyword("WHEN")? {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let body = self.parse_trigger_body()?;
        let span = start.union(self.preceding_span());
        let trigger_meta = self.make_meta(span);
        let create = CreateTrigger {
            temporary,
            if_not_exists,
            name,
            timing,
            event,
            table,
            for_each_row,
            when,
            body,
            meta: trigger_meta,
        };
        let meta = self.make_meta(span);
        Ok(Statement::CreateTrigger {
            create: Box::new(create),
            meta,
        })
    }

    /// Parse the optional `BEFORE | AFTER | INSTEAD OF` fire-time keyword; `None` when
    /// unwritten (SQLite defaults to `BEFORE` but the absent form round-trips).
    fn parse_optional_trigger_timing(&mut self) -> ParseResult<Option<TriggerTiming>> {
        if self.eat_contextual_keyword("BEFORE")? {
            Ok(Some(TriggerTiming::Before))
        } else if self.eat_contextual_keyword("AFTER")? {
            Ok(Some(TriggerTiming::After))
        } else if self.eat_contextual_keyword("INSTEAD")? {
            self.expect_contextual_keyword("OF")?;
            Ok(Some(TriggerTiming::InsteadOf))
        } else {
            Ok(None)
        }
    }

    /// Parse the firing event: `DELETE | INSERT | UPDATE [OF <col> [, ...]]`.
    fn parse_trigger_event(&mut self) -> ParseResult<TriggerEvent> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("DELETE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TriggerEvent::Delete { meta })
        } else if self.eat_contextual_keyword("INSERT")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TriggerEvent::Insert { meta })
        } else if self.eat_contextual_keyword("UPDATE")? {
            let columns = if self.eat_contextual_keyword("OF")? {
                self.parse_comma_separated(Self::parse_ident)?
            } else {
                ThinVec::new()
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TriggerEvent::Update { columns, meta })
        } else {
            Err(self.unexpected("a trigger event: `DELETE`, `INSERT`, or `UPDATE`"))
        }
    }

    /// Parse a `BEGIN <stmt>; ... END` trigger body: a non-empty list of
    /// `INSERT`/`UPDATE`/`DELETE`/`SELECT` statements, each `;`-terminated.
    ///
    /// Each body statement routes through the recursion-guarded
    /// [`parse_statement`](Self::parse_statement), so the body needs no
    /// separate depth budget — the trigger cannot contain another trigger (the four
    /// admitted kinds exclude `CREATE`), so there is no unbounded self-recursion, and
    /// the body list is flat (breadth, not depth). The kind gate mirrors the `COPY
    /// (<query>)` source restriction: a non-DML/query body statement (e.g. a `CREATE`)
    /// is a SQLite syntax error, rejected here rather than silently accepted.
    fn parse_trigger_body(&mut self) -> ParseResult<ThinVec<Statement<D::Ext>>> {
        self.expect_contextual_keyword("BEGIN")?;
        let mut body = ThinVec::new();
        while !self.peek_is_contextual_keyword("END")? {
            let statement = self.parse_statement()?;
            if !matches!(
                statement,
                Statement::Query { .. }
                    | Statement::Insert { .. }
                    | Statement::Update { .. }
                    | Statement::Delete { .. }
            ) {
                return Err(self.unexpected(
                    "an INSERT, UPDATE, DELETE, or SELECT statement in the trigger body",
                ));
            }
            self.expect_punct(Punctuation::Semicolon, "`;` after a trigger body statement")?;
            body.push(statement);
        }
        if body.is_empty() {
            // SQLite rejects an empty `BEGIN END` (`near "END": syntax error`).
            return Err(self.unexpected("at least one statement in the trigger body"));
        }
        self.expect_contextual_keyword("END")?;
        Ok(body)
    }

    /// Parse a MySQL `CREATE [DEFINER = <user>] TRIGGER [IF NOT EXISTS] <name>
    /// {BEFORE | AFTER} {INSERT | UPDATE | DELETE} ON <table> FOR EACH ROW
    /// [{FOLLOWS | PRECEDES} <other>] <sp_proc_stmt>` (SQL/PSM), gated by
    /// `compound_statements`. The `TRIGGER` keyword is still on the input; `definer` is the
    /// already-parsed prefix. Every axis is mandatory but the ordering clause and the body's
    /// own `RETURN` (rejected — a trigger is not a function) — see
    /// [`parse_routine_body_statement`](Self::parse_routine_body_statement).
    fn parse_create_stored_trigger(
        &mut self,
        start: Span,
        definer: Option<Definer>,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_contextual_keyword("TRIGGER")?;
        let if_not_exists = self.parse_if_not_exists()?;
        let name = self.parse_target_relation_name()?;
        let timing = self.parse_stored_trigger_timing()?;
        let event = self.parse_stored_trigger_event()?;
        self.expect_keyword(Keyword::On)?;
        let table = self.parse_target_relation_name()?;
        // MySQL requires the full `FOR EACH ROW` (statement-level triggers do not exist).
        self.expect_contextual_keyword("FOR")?;
        self.expect_contextual_keyword("EACH")?;
        self.expect_contextual_keyword("ROW")?;
        let ordering = self.parse_optional_trigger_order()?;
        // The trigger body is one `sp_proc_stmt` (usually a `BEGIN … END` compound block); a
        // trigger is not a function, so `RETURN` in the body rejects (server-mirrored).
        let body = self.parse_routine_body_statement(false)?;
        let span = start.union(self.preceding_span());
        let create = CreateStoredTrigger {
            definer: definer.map(Box::new),
            if_not_exists,
            name,
            timing,
            event,
            table,
            ordering,
            body: Box::new(body),
            meta: self.make_meta(span),
        };
        Ok(Statement::CreateStoredTrigger {
            create: Box::new(create),
            meta: self.make_meta(span),
        })
    }

    /// Parse the mandatory MySQL `BEFORE | AFTER` fire time (`trg_action_time`). MySQL has no
    /// defaulted/absent timing and no `INSTEAD OF` (a view-trigger form it does not model), so
    /// both are rejected here.
    fn parse_stored_trigger_timing(&mut self) -> ParseResult<TriggerTiming> {
        if self.eat_contextual_keyword("BEFORE")? {
            Ok(TriggerTiming::Before)
        } else if self.eat_contextual_keyword("AFTER")? {
            Ok(TriggerTiming::After)
        } else {
            Err(self.unexpected("`BEFORE` or `AFTER`"))
        }
    }

    /// Parse the mandatory MySQL trigger event (`trg_event`): a bare `INSERT | UPDATE | DELETE`
    /// (no `UPDATE OF <cols>` — that is a SQLite-only decoration), so a
    /// [`TriggerEvent::Update`] always carries an empty column list.
    fn parse_stored_trigger_event(&mut self) -> ParseResult<TriggerEvent> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("INSERT")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TriggerEvent::Insert { meta })
        } else if self.eat_contextual_keyword("UPDATE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TriggerEvent::Update {
                columns: ThinVec::new(),
                meta,
            })
        } else if self.eat_contextual_keyword("DELETE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(TriggerEvent::Delete { meta })
        } else {
            Err(self.unexpected("a trigger event: `INSERT`, `UPDATE`, or `DELETE`"))
        }
    }

    /// Parse the optional MySQL `{FOLLOWS | PRECEDES} <other>` ordering clause
    /// (`trigger_follows_precedes_clause`); `None` for the unordered form. The anchor is a bare
    /// or backtick-quoted trigger name.
    fn parse_optional_trigger_order(&mut self) -> ParseResult<Option<TriggerOrder>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("FOLLOWS")? {
            let anchor = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Some(TriggerOrder::Follows { anchor, meta }))
        } else if self.eat_contextual_keyword("PRECEDES")? {
            let anchor = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(Some(TriggerOrder::Precedes { anchor, meta }))
        } else {
            Ok(None)
        }
    }

    fn parse_index_columns(&mut self) -> ParseResult<ThinVec<IndexColumn<D::Ext>>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the index column list")?;
        let columns = self.parse_comma_separated(Self::parse_index_column)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the index column list")?;
        Ok(columns)
    }

    /// Parse one index key: `<expr> [ASC|DESC] [NULLS FIRST|LAST]`. A bare column is
    /// an [`crate::ast::Expr::Column`]; a parenthesized expression indexes a computed
    /// value. The sort modifiers mirror `ORDER BY` keys but the index grammar is its
    /// own family, so the small modifier scan is kept local rather than shared.
    fn parse_index_column(&mut self) -> ParseResult<IndexColumn<D::Ext>> {
        let start = self.current_span()?;
        let expr = self.parse_expr()?;
        let asc = if self.eat_keyword(Keyword::Asc)? {
            Some(true)
        } else if self.eat_keyword(Keyword::Desc)? {
            Some(false)
        } else {
            None
        };
        // MySQL has no index-key `NULLS FIRST`/`LAST`; with the gate off the `NULLS` keyword
        // is left unconsumed and surfaces as a clean parse error.
        let nulls_first = if self.features().index_alter_syntax.index_nulls_order
            && self.eat_keyword(Keyword::Nulls)?
        {
            if self.eat_keyword(Keyword::First)? {
                Some(true)
            } else if self.eat_keyword(Keyword::Last)? {
                Some(false)
            } else {
                return Err(self.unexpected("`FIRST` or `LAST`"));
            }
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(IndexColumn {
            expr,
            asc,
            nulls_first,
            meta,
        })
    }

    /// Parse an `OR REPLACE` prefix (ungated; a `CREATE VIEW` spelling).
    fn parse_or_replace(&mut self) -> ParseResult<bool> {
        if self.eat_keyword(Keyword::Or)? {
            self.expect_contextual_keyword("REPLACE")?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Parse an `ALTER TABLE` statement.
    pub(super) fn parse_alter_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("ALTER")?;
        if self.features().access_control_syntax.alter_role_rename
            && self.eat_contextual_keyword("ROLE")?
        {
            let name = self.parse_ident()?;
            self.expect_contextual_keyword("RENAME")?;
            self.expect_contextual_keyword("TO")?;
            let new_name = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Statement::AccessControl {
                access: Box::new(AccessControlStatement::AlterRoleRename {
                    name,
                    new_name,
                    meta,
                }),
                meta,
            });
        }
        // `ALTER EXTENSION` (PostgreSQL) — a whole-statement gate, intercepted before the
        // `TABLE` expectation. Off elsewhere, where `EXTENSION` surfaces as the
        // "expected TABLE" parse error.
        if self.features().statement_ddl_gates.extension_ddl
            && self.eat_contextual_keyword("EXTENSION")?
        {
            return self.parse_alter_extension(start);
        }
        // `ALTER {FUNCTION|PROCEDURE|ROUTINE|TRIGGER|MATERIALIZED VIEW|INDEX} … [NO]
        // DEPENDS ON EXTENSION <ext>` (PostgreSQL's `AlterObjectDependsStmt`) — sharing the
        // extension-DDL gate, intercepted before the `TABLE` expectation. Only these six
        // object heads reach the `DEPENDS` production; each commits to it once matched,
        // since no other `ALTER <head>` form for these objects is modelled yet.
        if self.features().statement_ddl_gates.extension_ddl {
            if let Some(object) = self.parse_alter_depends_object()? {
                return self.parse_alter_object_depends(start, object);
            }
        }
        // `ALTER SYSTEM { SET … | RESET … }` (PostgreSQL's `AlterSystemStmt`) — a
        // whole-statement gate on its own behaviour-named flag, intercepted before the
        // `TABLE` expectation. Off elsewhere, where `SYSTEM` surfaces as the "expected
        // TABLE" parse error.
        if self.features().statement_ddl_gates.alter_system
            && self.eat_contextual_keyword("SYSTEM")?
        {
            return self.parse_alter_system(start);
        }
        // MySQL `ALTER {PROCEDURE | FUNCTION} <name> [<characteristic> …]` — a whole-statement
        // gate riding `compound_statements` (the stored-routine surface), intercepted before
        // the `TABLE` expectation. Off elsewhere, where `PROCEDURE`/`FUNCTION` surfaces as the
        // "expected TABLE" parse error.
        if self.features().statement_ddl_gates.compound_statements {
            if self.eat_contextual_keyword("PROCEDURE")? {
                return self.parse_alter_routine(start, RoutineKind::Procedure);
            }
            if self.eat_contextual_keyword("FUNCTION")? {
                return self.parse_alter_routine(start, RoutineKind::Function);
            }
            if self.eat_contextual_keyword("EVENT")? {
                return self.parse_alter_event(start, None);
            }
            // `ALTER DEFINER = <user> {EVENT … | [SQL SECURITY …] VIEW …}` — the definer-prefixed
            // event alteration and, under the MySQL view definition surface, the definer-prefixed
            // view redefinition (the `ALTER definer_opt view_tail` grammar branch). A definer-led
            // view takes no `ALGORITHM` (that heads the separate `view_algorithm` branch), so only
            // the `SQL SECURITY` sub-option follows the definer here.
            if self.peek_is_contextual_keyword("DEFINER")? {
                let definer = self.parse_definer()?;
                if self
                    .features()
                    .view_sequence_clause_syntax
                    .view_definition_options
                    && (self.peek_is_contextual_keyword("VIEW")?
                        || self.peek_is_contextual_keyword("SQL")?)
                {
                    let sql_security = self.parse_optional_view_sql_security()?;
                    self.expect_contextual_keyword("VIEW")?;
                    let name = self.parse_object_name()?;
                    let options = ViewOptions {
                        algorithm: None,
                        definer: Some(Box::new(definer)),
                        sql_security,
                    };
                    return self.parse_alter_view(start, options, name);
                }
                self.expect_contextual_keyword("EVENT")?;
                return self.parse_alter_event(start, Some(definer));
            }
        }
        // MySQL `ALTER USER …` account modification — a whole-statement gate, intercepted
        // before the `TABLE` expectation. Off elsewhere, `USER` surfaces as "expected TABLE".
        if self.features().access_control_syntax.user_role_management
            && self.eat_contextual_keyword("USER")?
        {
            return self.parse_alter_user(start);
        }
        // `ALTER {DATABASE | SCHEMA} …` — two disjoint behaviours share the `DATABASE` head:
        // DuckDB's `[IF EXISTS] <name> SET ALIAS TO <alias>` (`alter_database`) and MySQL's
        // charset/collate/encryption/read-only option list `[<name>] <option> …`
        // (`alter_database_options`, which also admits the `SCHEMA` synonym). Both are
        // intercepted before the `TABLE` expectation. The MySQL-only `SCHEMA` head routes
        // straight to the option parser; the shared `DATABASE` head is disambiguated in
        // `parse_alter_database_head` (needed where both gates are on, i.e. Lenient).
        if self.features().statement_ddl_gates.alter_database_options
            && self.eat_contextual_keyword("SCHEMA")?
        {
            return self.parse_alter_database_options(start, DatabaseKeyword::Schema);
        }
        if (self.features().statement_ddl_gates.alter_database
            || self.features().statement_ddl_gates.alter_database_options)
            && self.eat_contextual_keyword("DATABASE")?
        {
            return self.parse_alter_database_head(start);
        }
        // MySQL `ALTER SERVER <name> OPTIONS ( … )` — the server-object change, sharing the
        // `server_definition` gate with `CREATE`/`DROP SERVER`. Intercepted before the `TABLE`
        // expectation. Off elsewhere, where `SERVER` surfaces as "expected TABLE".
        if self.features().statement_ddl_gates.server_definition
            && self.eat_contextual_keyword("SERVER")?
        {
            return self.parse_alter_server(start);
        }
        // MySQL `ALTER INSTANCE <action>` — instance-wide administration, a whole-statement gate
        // like `alter_system`. Intercepted before the `TABLE` expectation. Off elsewhere, where
        // `INSTANCE` surfaces as "expected TABLE".
        if self.features().statement_ddl_gates.alter_instance
            && self.eat_contextual_keyword("INSTANCE")?
        {
            return self.parse_alter_instance(start);
        }
        // MySQL `ALTER RESOURCE GROUP <name> …` — the resource-group change, sharing the
        // `resource_group` gate with `CREATE`/`DROP RESOURCE GROUP` and `SET RESOURCE GROUP`.
        // Intercepted before the `TABLE` expectation. Off elsewhere, where `RESOURCE` surfaces
        // as "expected TABLE".
        if self.features().statement_ddl_gates.resource_group
            && self.peek_is_contextual_keyword("RESOURCE")?
        {
            self.expect_contextual_keyword("RESOURCE")?;
            self.expect_contextual_keyword("GROUP")?;
            return self.parse_alter_resource_group(start);
        }
        // DuckDB `ALTER SEQUENCE …`: either the schema relocation (`… SET SCHEMA <schema>`,
        // `AlterObjectSchemaStmt`) or the option list (`AlterSeqStmt`). The `SEQUENCE` head
        // is intercepted once for both, since neither reaches the `TABLE` expectation.
        if (self.features().statement_ddl_gates.alter_sequence
            || self.features().statement_ddl_gates.alter_object_set_schema)
            && self.eat_contextual_keyword("SEQUENCE")?
        {
            return self.parse_alter_sequence(start);
        }
        // MySQL `ALTER [ALGORITHM = …] [DEFINER = …] [SQL SECURITY …] VIEW <name> [(<cols>)] AS
        // <query> [WITH … CHECK OPTION]` — the view redefinition surface. `ALGORITHM`/`SQL
        // SECURITY` lead only to a view (a definer-led view was handled in the compound block
        // above). The bare `ALTER VIEW` head is shared with DuckDB's `… SET SCHEMA` relocation
        // (both gated on only under Lenient); MySQL takes no `IF EXISTS`, so an `IF EXISTS`
        // guard or a `SET SCHEMA` tail routes to the relocation, and a `(`/`AS` tail routes to
        // the redefinition.
        if self
            .features()
            .view_sequence_clause_syntax
            .view_definition_options
        {
            if self.peek_is_contextual_keyword("ALGORITHM")?
                || (self.peek_is_contextual_keyword("SQL")?
                    && self.peek_nth_is_contextual_keyword(1, "SECURITY")?)
            {
                let options = self.parse_view_option_prefix()?;
                self.expect_contextual_keyword("VIEW")?;
                let name = self.parse_object_name()?;
                return self.parse_alter_view(start, options, name);
            }
            if self.eat_contextual_keyword("VIEW")? {
                let relocates = self.features().statement_ddl_gates.alter_object_set_schema;
                // `IF EXISTS` belongs only to the DuckDB relocation (MySQL `ALTER VIEW IF EXISTS`
                // is `ER_PARSE_ERROR`); its presence commits to that path.
                if relocates && self.peek_is_keyword(Keyword::If)? {
                    let if_exists = self.parse_schema_change_if_exists()?;
                    let name = self.parse_target_relation_name()?;
                    return self.finish_alter_object_set_schema(
                        start,
                        SchemaRelocationObject::View,
                        if_exists,
                        name,
                    );
                }
                let name = self.parse_object_name()?;
                if relocates
                    && self.peek_is_keyword(Keyword::Set)?
                    && self.peek_nth_is_contextual_keyword(1, "SCHEMA")?
                {
                    return self.finish_alter_object_set_schema(
                        start,
                        SchemaRelocationObject::View,
                        false,
                        name,
                    );
                }
                return self.parse_alter_view(start, ViewOptions::default(), name);
            }
        }
        // DuckDB `ALTER VIEW [IF EXISTS] <name> SET SCHEMA <schema>`
        // (`AlterObjectSchemaStmt`). Only the schema relocation is modelled on this head; a
        // following non-`SET SCHEMA` tail surfaces as a clean parse error. (Under Lenient the
        // MySQL block above owns the `VIEW` head; this path is the pure-DuckDB dialect, where
        // that block's gate is off.)
        if self.features().statement_ddl_gates.alter_object_set_schema
            && self.eat_contextual_keyword("VIEW")?
        {
            let if_exists = self.parse_schema_change_if_exists()?;
            let name = self.parse_target_relation_name()?;
            return self.finish_alter_object_set_schema(
                start,
                SchemaRelocationObject::View,
                if_exists,
                name,
            );
        }
        // MySQL storage DDL — `ALTER [UNDO] TABLESPACE …` / `ALTER LOGFILE GROUP …`. Whole-
        // statement gates, intercepted before the `TABLE` expectation. Off elsewhere, the
        // keyword surfaces as the "expected TABLE" parse error.
        if self.features().statement_ddl_gates.tablespace_ddl {
            if self.eat_contextual_keyword("UNDO")? {
                self.expect_contextual_keyword("TABLESPACE")?;
                return self.parse_alter_undo_tablespace(start);
            }
            if self.eat_contextual_keyword("TABLESPACE")? {
                return self.parse_alter_tablespace(start);
            }
        }
        if self.features().statement_ddl_gates.logfile_group_ddl
            && self.peek_is_contextual_keyword("LOGFILE")?
            && self.peek_nth_is_contextual_keyword(1, "GROUP")?
        {
            self.advance()?; // LOGFILE
            self.advance()?; // GROUP
            return self.parse_alter_logfile_group(start);
        }
        self.expect_keyword(Keyword::Table)?;
        // SQLite's `ALTER TABLE` is narrow: no table-level `IF EXISTS`, and exactly one
        // action (a trailing comma then surfaces as a clean parse error).
        let extended = self.features().index_alter_syntax.alter_table_extended;
        // The table-level `ALTER TABLE IF EXISTS` guard is a separate gate from the extended
        // surface: MySQL has the extended surface but rejects this guard.
        let if_exists = if extended && self.features().index_alter_syntax.alter_existence_guards {
            self.parse_schema_change_if_exists()?
        } else {
            false
        };
        let name = self.parse_target_relation_name()?;
        // DuckDB `ALTER TABLE [IF EXISTS] <name> SET SCHEMA <schema>` (`AlterObjectSchemaStmt`)
        // — a separate production from the `AlterTableStmt` command list, split off here on
        // the exact two-token `SET SCHEMA` lead so any other `SET …` action stays with the
        // command parser.
        if self.features().statement_ddl_gates.alter_object_set_schema
            && self.peek_is_keyword(Keyword::Set)?
            && self.peek_nth_is_contextual_keyword(1, "SCHEMA")?
        {
            return self.finish_alter_object_set_schema(
                start,
                SchemaRelocationObject::Table,
                if_exists,
                name,
            );
        }
        let actions = if self
            .features()
            .create_table_clause_syntax
            .declarative_partitioning
            && (self.peek_is_contextual_keyword("ATTACH")?
                || self.peek_is_contextual_keyword("DETACH")?)
        {
            // PostgreSQL parses `ATTACH`/`DETACH PARTITION` as their own `AlterTableStmt`
            // productions, so each is a *single* standalone action — never in a comma list
            // (`ALTER TABLE p ADD COLUMN x, ATTACH …` is a syntax error). The comma loop is
            // therefore not entered.
            let mut actions = ThinVec::new();
            actions.push(self.parse_alter_table_partition_action()?);
            actions
        } else if extended
            && self
                .features()
                .index_alter_syntax
                .alter_table_multiple_actions
        {
            self.parse_comma_separated(Self::parse_alter_table_action)?
        } else {
            let mut actions = ThinVec::new();
            actions.push(self.parse_alter_table_action()?);
            actions
        };
        if actions.len() > 1
            && actions.iter().any(|action| {
                matches!(
                    action,
                    AlterTableAction::SetColocationGroup { .. }
                        | AlterTableAction::DropColocationGroup { .. }
                )
            })
        {
            return Err(self.unexpected("a standalone colocation ALTER TABLE action"));
        }
        let span = start.union(self.preceding_span());
        let alter_meta = self.make_meta(span);
        let alter = AlterTable {
            if_exists,
            name,
            actions,
            meta: alter_meta,
        };
        let meta = self.make_meta(span);
        Ok(Statement::AlterTable {
            alter: Box::new(alter),
            meta,
        })
    }

    /// Parse DuckDB's `ALTER DATABASE [IF EXISTS] <name> SET ALIAS TO <alias>` after the
    /// `DATABASE` keyword (already consumed). The grammar reduces `SET <ident> TO <name>` but
    /// rejects any keyword but `ALIAS` in its action, so only `SET ALIAS TO` parse-accepts
    /// (engine-measured on DuckDB 1.5.4: `SET FOO TO` is a syntax error).
    fn parse_alter_database(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let if_exists = self.parse_schema_change_if_exists()?;
        let name = self.parse_ident()?;
        self.finish_alter_database_set_alias(start, if_exists, name)
    }

    /// Finish DuckDB's `ALTER DATABASE … SET ALIAS TO <alias>` after the `[IF EXISTS] <name>`
    /// head (both already parsed) — the shared `SET ALIAS TO` tail, split out so the Lenient
    /// disambiguator in [`parse_alter_database_head`](Self::parse_alter_database_head) can reach
    /// it with a pre-parsed name.
    fn finish_alter_database_set_alias(
        &mut self,
        start: Span,
        if_exists: bool,
        name: Ident,
    ) -> ParseResult<Statement<D::Ext>> {
        let action_start = self.current_span()?;
        self.expect_keyword(Keyword::Set)?;
        self.expect_contextual_keyword("ALIAS")?;
        self.expect_keyword(Keyword::To)?;
        let new_name = self.parse_ident()?;
        let action = AlterDatabaseAction::SetAlias {
            new_name,
            meta: self.make_meta(action_start.union(self.preceding_span())),
        };
        let span = start.union(self.preceding_span());
        Ok(Statement::AlterDatabase {
            alter: Box::new(AlterDatabase {
                if_exists,
                name,
                action,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse the shared `ALTER DATABASE …` head (the `DATABASE` keyword already consumed),
    /// disambiguating DuckDB's `SET ALIAS` relocation (the `alter_database` gate) from MySQL's
    /// option list (the `alter_database_options` gate; see
    /// [`StatementDdlGates`](crate::ast::dialect::StatementDdlGates)). The two grammars share
    /// only this head; with a single gate on it commits directly, and where both are on (Lenient)
    /// a lookahead splits them: DuckDB's `IF EXISTS` guard or a `SET` tail after the name is the
    /// relocation, while an option keyword leading (no name) or a name followed by a non-`SET`
    /// tail is MySQL's option list.
    fn parse_alter_database_head(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let alias = self.features().statement_ddl_gates.alter_database;
        let options = self.features().statement_ddl_gates.alter_database_options;
        if alias && !options {
            return self.parse_alter_database(start);
        }
        if options && !alias {
            return self.parse_alter_database_options(start, DatabaseKeyword::Database);
        }
        // Both gates on (Lenient): disambiguate.
        if self.peek_is_keyword(Keyword::If)? {
            return self.parse_alter_database(start);
        }
        if self.peek_is_alter_database_option_lead()? {
            // An option keyword leads directly — the unqualified MySQL form (no name).
            return self.finish_alter_database_options(start, DatabaseKeyword::Database, None);
        }
        let name = self.parse_ident()?;
        if self.peek_is_keyword(Keyword::Set)? {
            return self.finish_alter_database_set_alias(start, false, name);
        }
        self.finish_alter_database_options(start, DatabaseKeyword::Database, Some(name))
    }

    /// Parse MySQL's `ALTER {DATABASE | SCHEMA} [<name>] <option> …` after the head keyword
    /// (already consumed). The name is absent when an option keyword leads directly (`ALTER
    /// DATABASE CHARACTER SET utf8` targets the session's default schema); otherwise a bare
    /// [`Ident`] name precedes the option list (a dotted `d.x` is `ER_PARSE_ERROR`).
    fn parse_alter_database_options(
        &mut self,
        start: Span,
        spelling: DatabaseKeyword,
    ) -> ParseResult<Statement<D::Ext>> {
        let name = if self.peek_is_alter_database_option_lead()? {
            None
        } else {
            Some(self.parse_ident()?)
        };
        self.finish_alter_database_options(start, spelling, name)
    }

    /// Finish MySQL's `ALTER {DATABASE | SCHEMA}` after the `[<name>]` head — the non-empty,
    /// space-separated `alter_database_options` list.
    fn finish_alter_database_options(
        &mut self,
        start: Span,
        spelling: DatabaseKeyword,
        name: Option<Ident>,
    ) -> ParseResult<Statement<D::Ext>> {
        let mut options = ThinVec::new();
        options.push(self.parse_alter_database_option()?);
        while self.peek_is_alter_database_option_lead()? {
            options.push(self.parse_alter_database_option()?);
        }
        let span = start.union(self.preceding_span());
        Ok(Statement::AlterDatabaseOptions {
            alter: Box::new(AlterDatabaseOptions {
                spelling,
                name,
                options,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Whether the current token can lead an [`AlterDatabaseOption`] — one of the option
    /// keywords `DEFAULT`, `CHARACTER`, `CHARSET`, `COLLATE`, `ENCRYPTION`, `READ`. Used both to
    /// decide an absent name and to continue the space-separated option loop.
    fn peek_is_alter_database_option_lead(&mut self) -> ParseResult<bool> {
        Ok(self.peek_is_keyword(Keyword::Default)?
            || self.peek_is_contextual_keyword("CHARACTER")?
            || self.peek_is_contextual_keyword("CHARSET")?
            || self.peek_is_contextual_keyword("COLLATE")?
            || self.peek_is_contextual_keyword("ENCRYPTION")?
            || self.peek_is_contextual_keyword("READ")?)
    }

    /// Parse one MySQL `alter_database_option` — `[DEFAULT] {CHARACTER SET | CHARSET} [=]
    /// <charset>`, `[DEFAULT] COLLATE [=] <collation>`, `[DEFAULT] ENCRYPTION [=] '<v>'`, or
    /// `READ ONLY [=] {DEFAULT | 0 | 1}`. `READ ONLY` takes no leading `DEFAULT` (`DEFAULT READ
    /// ONLY` is `ER_PARSE_ERROR`), so a consumed `DEFAULT` commits to the charset/collation/
    /// encryption alternatives.
    fn parse_alter_database_option(&mut self) -> ParseResult<AlterDatabaseOption> {
        let opt_start = self.current_span()?;
        let default = self.eat_keyword(Keyword::Default)?;
        // `CHARACTER SET` (two words) or its `CHARSET` synonym.
        if self.peek_is_contextual_keyword("CHARACTER")?
            && self.peek_nth_is_contextual_keyword(1, "SET")?
        {
            self.advance()?; // CHARACTER
            self.advance()?; // SET
            let equals = self.eat_op(Operator::Eq)?;
            let charset = self.parse_alter_database_charset_name()?;
            return Ok(AlterDatabaseOption::CharacterSet {
                default,
                keyword: CharsetKeyword::CharacterSet,
                equals,
                charset,
                meta: self.make_meta(opt_start.union(self.preceding_span())),
            });
        }
        if self.eat_contextual_keyword("CHARSET")? {
            let equals = self.eat_op(Operator::Eq)?;
            let charset = self.parse_alter_database_charset_name()?;
            return Ok(AlterDatabaseOption::CharacterSet {
                default,
                keyword: CharsetKeyword::Charset,
                equals,
                charset,
                meta: self.make_meta(opt_start.union(self.preceding_span())),
            });
        }
        if self.eat_contextual_keyword("COLLATE")? {
            let equals = self.eat_op(Operator::Eq)?;
            let collation = self.parse_alter_database_charset_name()?;
            return Ok(AlterDatabaseOption::Collate {
                default,
                equals,
                collation,
                meta: self.make_meta(opt_start.union(self.preceding_span())),
            });
        }
        if self.eat_contextual_keyword("ENCRYPTION")? {
            let equals = self.eat_op(Operator::Eq)?;
            let value = self.expect_string_literal("a string value after `ENCRYPTION`")?;
            return Ok(AlterDatabaseOption::Encryption {
                default,
                equals,
                value,
                meta: self.make_meta(opt_start.union(self.preceding_span())),
            });
        }
        // `READ ONLY [=] {DEFAULT | 0 | 1}` — no `DEFAULT` prefix.
        if !default && self.eat_contextual_keyword("READ")? {
            self.expect_contextual_keyword("ONLY")?;
            let equals = self.eat_op(Operator::Eq)?;
            let value = self.parse_read_only_value()?;
            return Ok(AlterDatabaseOption::ReadOnly {
                equals,
                value,
                meta: self.make_meta(opt_start.union(self.preceding_span())),
            });
        }
        Err(self.unexpected(
            "a database option (`[DEFAULT] {CHARACTER SET | CHARSET | COLLATE | ENCRYPTION}` or \
             `READ ONLY`)",
        ))
    }

    /// Parse a MySQL `charset_name`/`collation_name` for an `ALTER DATABASE` option — an
    /// `ident_or_text` (a bare/backtick identifier or a quoted string, folded to an [`Ident`]),
    /// or the reserved word `BINARY` (`charset_name: … | BINARY_SYM`).
    fn parse_alter_database_charset_name(&mut self) -> ParseResult<Ident> {
        if self.peek_is_contextual_keyword("BINARY")? {
            let span = self.current_span()?;
            let sym = self.intern_text(self.span_text(span));
            self.advance()?;
            return Ok(Ident {
                sym,
                quote: QuoteStyle::None,
                meta: self.make_meta(span),
            });
        }
        self.parse_ident_or_text()
    }

    /// Parse a MySQL `ternary_option` for `READ ONLY` — `DEFAULT`, or the number `0`/`1`. Any
    /// other number is `ER_PARSE_ERROR` (the server's `ternary_option` action rejects it), so
    /// only `0`/`1` are admitted here.
    fn parse_read_only_value(&mut self) -> ParseResult<ReadOnlyValue> {
        if self.eat_keyword(Keyword::Default)? {
            return Ok(ReadOnlyValue::Default);
        }
        if let Some(token) = self.peek()? {
            if token.kind == TokenKind::Number {
                let value = match self.span_text(token.span) {
                    "0" => Some(ReadOnlyValue::Off),
                    "1" => Some(ReadOnlyValue::On),
                    _ => None,
                };
                if let Some(value) = value {
                    self.advance()?;
                    return Ok(value);
                }
            }
        }
        Err(self.unexpected("`DEFAULT`, `0`, or `1` after `READ ONLY`"))
    }

    /// Parse a MySQL `CREATE SERVER <name> FOREIGN DATA WRAPPER <wrapper> OPTIONS ( … )` after
    /// the `SERVER` keyword (already consumed). The server and wrapper names are each an
    /// `ident_or_text`; the `OPTIONS` list is the non-empty [`ServerOption`] grammar.
    fn parse_create_server(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident_or_text()?;
        self.expect_contextual_keyword("FOREIGN")?;
        self.expect_contextual_keyword("DATA")?;
        self.expect_contextual_keyword("WRAPPER")?;
        let wrapper = self.parse_ident_or_text()?;
        self.expect_contextual_keyword("OPTIONS")?;
        let options = self.parse_server_options()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::CreateServer {
            create: Box::new(CreateServer {
                name,
                wrapper,
                options,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `ALTER SERVER <name> OPTIONS ( … )` after the `SERVER` keyword (already
    /// consumed). Names no `FOREIGN DATA WRAPPER` (fixed at creation); the `OPTIONS` list is the
    /// same non-empty [`ServerOption`] grammar as `CREATE SERVER`.
    fn parse_alter_server(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident_or_text()?;
        self.expect_contextual_keyword("OPTIONS")?;
        let options = self.parse_server_options()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::AlterServer {
            alter: Box::new(AlterServer {
                name,
                options,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse the shared `OPTIONS ( <option> [, ...] )` list of `CREATE`/`ALTER SERVER` — a
    /// parenthesized, comma-separated, non-empty `server_options_list` (an empty `()` or a
    /// trailing comma is `ER_PARSE_ERROR`).
    fn parse_server_options(&mut self) -> ParseResult<ThinVec<ServerOption>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the SERVER `OPTIONS` list")?;
        let options = self.parse_comma_separated(Self::parse_server_option)?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the SERVER `OPTIONS` list",
        )?;
        Ok(options)
    }

    /// Parse one `server_option` — a fixed keyword and its value. The string-valued keywords
    /// (`HOST`/`DATABASE`/`USER`/`PASSWORD`/`SOCKET`/`OWNER`) take a `TEXT_STRING_sys` string
    /// literal; `PORT` takes a `ulong_num` unsigned-integer literal. A wrong value type
    /// (`HOST 123`, `PORT '3306'`) is `ER_PARSE_ERROR`.
    fn parse_server_option(&mut self) -> ParseResult<ServerOption> {
        let start = self.current_span()?;
        let kind = if self.eat_contextual_keyword("HOST")? {
            ServerOptionKind::Host
        } else if self.eat_contextual_keyword("DATABASE")? {
            ServerOptionKind::Database
        } else if self.eat_contextual_keyword("USER")? {
            ServerOptionKind::User
        } else if self.eat_contextual_keyword("PASSWORD")? {
            ServerOptionKind::Password
        } else if self.eat_contextual_keyword("SOCKET")? {
            ServerOptionKind::Socket
        } else if self.eat_contextual_keyword("OWNER")? {
            ServerOptionKind::Owner
        } else if self.eat_contextual_keyword("PORT")? {
            ServerOptionKind::Port
        } else {
            return Err(self.unexpected(
                "a SERVER option keyword (`HOST`/`DATABASE`/`USER`/`PASSWORD`/`SOCKET`/`OWNER`/`PORT`)",
            ));
        };
        let value = if kind == ServerOptionKind::Port {
            self.expect_unsigned_integer_literal("PORT")?
        } else {
            self.expect_string_literal("a string value for the SERVER option")?
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ServerOption { kind, value, meta })
    }

    /// Parse a MySQL `ALTER INSTANCE <action>` after the `INSTANCE` keyword (already consumed) —
    /// see [`parse_alter_instance_action`](Self::parse_alter_instance_action).
    fn parse_alter_instance(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let action = self.parse_alter_instance_action()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::AlterInstance {
            alter: Box::new(AlterInstance {
                action,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `alter_instance_action`. The `INNODB`/`BINLOG`/`REDO_LOG` words are matched
    /// as identifiers by the server, so only the measured accepted spellings parse; the `RELOAD
    /// TLS` tail admits an optional `FOR CHANNEL <ident>` (a bare identifier — a quoted `'ch'`
    /// is `ER_PARSE_ERROR`) and a trailing `NO ROLLBACK ON ERROR`.
    fn parse_alter_instance_action(&mut self) -> ParseResult<AlterInstanceAction> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("ROTATE")? {
            let innodb = if self.eat_contextual_keyword("INNODB")? {
                true
            } else if self.eat_contextual_keyword("BINLOG")? {
                false
            } else {
                return Err(self.unexpected("`INNODB` or `BINLOG` after `ROTATE`"));
            };
            self.expect_contextual_keyword("MASTER")?;
            self.expect_contextual_keyword("KEY")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(if innodb {
                AlterInstanceAction::RotateInnodbMasterKey { meta }
            } else {
                AlterInstanceAction::RotateBinlogMasterKey { meta }
            });
        }
        if self.eat_contextual_keyword("RELOAD")? {
            if self.eat_contextual_keyword("TLS")? {
                let channel = if self.eat_contextual_keyword("FOR")? {
                    self.expect_contextual_keyword("CHANNEL")?;
                    Some(self.parse_ident()?)
                } else {
                    None
                };
                let no_rollback_on_error = if self.eat_contextual_keyword("NO")? {
                    self.expect_contextual_keyword("ROLLBACK")?;
                    self.expect_contextual_keyword("ON")?;
                    self.expect_contextual_keyword("ERROR")?;
                    true
                } else {
                    false
                };
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(AlterInstanceAction::ReloadTls {
                    channel,
                    no_rollback_on_error,
                    meta,
                });
            }
            self.expect_contextual_keyword("KEYRING")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AlterInstanceAction::ReloadKeyring { meta });
        }
        if self.eat_contextual_keyword("ENABLE")? {
            self.expect_contextual_keyword("INNODB")?;
            self.expect_contextual_keyword("REDO_LOG")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AlterInstanceAction::EnableInnodbRedoLog { meta });
        }
        if self.eat_contextual_keyword("DISABLE")? {
            self.expect_contextual_keyword("INNODB")?;
            self.expect_contextual_keyword("REDO_LOG")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AlterInstanceAction::DisableInnodbRedoLog { meta });
        }
        Err(self.unexpected(
            "an instance action (`ROTATE … MASTER KEY`, `RELOAD {TLS | KEYRING}`, or \
             `{ENABLE | DISABLE} INNODB REDO_LOG`)",
        ))
    }

    /// Parse a MySQL `DROP SERVER [IF EXISTS] <name>` after the `SERVER` keyword (already
    /// consumed). The name is an `ident_or_text`; no comma list (`DROP SERVER a, b` is
    /// `ER_PARSE_ERROR`).
    fn parse_drop_server(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let if_exists = self.parse_schema_change_if_exists()?;
        let name = self.parse_ident_or_text()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::DropServer {
            drop: Box::new(DropServer {
                if_exists,
                name,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `CREATE [OR REPLACE] SPATIAL REFERENCE SYSTEM [IF NOT EXISTS] <srid>
    /// <attributes>` after the `SYSTEM` keyword (already consumed). `IF NOT EXISTS` is admitted
    /// only on the bare branch — the `OR REPLACE` grammar branch goes straight to the srid
    /// (`CREATE OR REPLACE … IF NOT EXISTS` is `ER_PARSE_ERROR` on mysql:8.4.10). The srid is a
    /// `real_ulonglong_num` (decimal or `0x` hex integer; a signed or fractional value is
    /// `ER_PARSE_ERROR`); the attributes are order-free and may repeat (a repeat is the
    /// post-parse `ER_SRS_MULTIPLE_ATTRIBUTE_DEFINITIONS`, not a syntax error).
    fn parse_create_spatial_reference_system(
        &mut self,
        start: Span,
        or_replace: bool,
    ) -> ParseResult<Statement<D::Ext>> {
        let if_not_exists = !or_replace && self.parse_if_not_exists()?;
        let srid = self.expect_unsigned_integer_literal("SPATIAL REFERENCE SYSTEM")?;
        let mut attributes = ThinVec::new();
        loop {
            let attribute_start = self.current_span()?;
            if self.eat_contextual_keyword("NAME")? {
                let value = self.expect_string_literal("a string value for `NAME`")?;
                attributes.push(SrsAttribute::Name {
                    value,
                    meta: self.make_meta(attribute_start.union(self.preceding_span())),
                });
            } else if self.eat_contextual_keyword("DEFINITION")? {
                let value = self.expect_string_literal("a string value for `DEFINITION`")?;
                attributes.push(SrsAttribute::Definition {
                    value,
                    meta: self.make_meta(attribute_start.union(self.preceding_span())),
                });
            } else if self.eat_contextual_keyword("ORGANIZATION")? {
                let organization =
                    self.expect_string_literal("a string value for `ORGANIZATION`")?;
                self.expect_contextual_keyword("IDENTIFIED")?;
                self.expect_keyword(Keyword::By)?;
                let identifier = self.expect_unsigned_integer_literal("IDENTIFIED BY")?;
                attributes.push(SrsAttribute::Organization {
                    organization,
                    identifier,
                    meta: self.make_meta(attribute_start.union(self.preceding_span())),
                });
            } else if self.eat_contextual_keyword("DESCRIPTION")? {
                let value = self.expect_string_literal("a string value for `DESCRIPTION`")?;
                attributes.push(SrsAttribute::Description {
                    value,
                    meta: self.make_meta(attribute_start.union(self.preceding_span())),
                });
            } else {
                break;
            }
        }
        let span = start.union(self.preceding_span());
        Ok(Statement::CreateSpatialReferenceSystem {
            create: Box::new(CreateSpatialReferenceSystem {
                or_replace,
                if_not_exists,
                srid,
                attributes,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `DROP SPATIAL REFERENCE SYSTEM [IF EXISTS] <srid>` after the `SYSTEM`
    /// keyword (already consumed). Names exactly one integer srid; no comma list
    /// (`DROP SPATIAL REFERENCE SYSTEM 1, 2` is `ER_PARSE_ERROR` on mysql:8.4.10).
    fn parse_drop_spatial_reference_system(
        &mut self,
        start: Span,
    ) -> ParseResult<Statement<D::Ext>> {
        let if_exists = self.parse_schema_change_if_exists()?;
        let srid = self.expect_unsigned_integer_literal("SPATIAL REFERENCE SYSTEM")?;
        let span = start.union(self.preceding_span());
        Ok(Statement::DropSpatialReferenceSystem {
            drop: Box::new(DropSpatialReferenceSystem {
                if_exists,
                srid,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `CREATE RESOURCE GROUP <name> TYPE [=] {SYSTEM | USER} [VCPU …]
    /// [THREAD_PRIORITY …] [ENABLE | DISABLE]` after the `GROUP` keyword (already consumed).
    /// `TYPE` is mandatory and the option train is fixed-order (any permutation is
    /// `ER_PARSE_ERROR` on mysql:8.4.10); `CREATE` admits no trailing `FORCE`, unlike `ALTER`.
    fn parse_create_resource_group(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        self.expect_contextual_keyword("TYPE")?;
        let type_equals = self.eat_op(Operator::Eq)?;
        let group_type = if self.eat_contextual_keyword("SYSTEM")? {
            ResourceGroupType::System
        } else if self.eat_contextual_keyword("USER")? {
            ResourceGroupType::User
        } else {
            return Err(self.unexpected("`SYSTEM` or `USER` after `TYPE`"));
        };
        let vcpu = self.parse_optional_resource_group_vcpu()?;
        let thread_priority = self.parse_optional_resource_group_thread_priority()?;
        let state = self.parse_optional_resource_group_state()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::CreateResourceGroup {
            create: Box::new(CreateResourceGroup {
                name,
                type_equals,
                group_type,
                vcpu,
                thread_priority,
                state,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `ALTER RESOURCE GROUP <name> [VCPU …] [THREAD_PRIORITY …]
    /// [ENABLE | DISABLE] [FORCE]` after the `GROUP` keyword (already consumed). Every clause is
    /// optional (a bare `ALTER RESOURCE GROUP g` grammar-accepts); `FORCE` is an independent
    /// trailing optional, valid with or without a preceding `ENABLE`/`DISABLE`.
    fn parse_alter_resource_group(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        let vcpu = self.parse_optional_resource_group_vcpu()?;
        let thread_priority = self.parse_optional_resource_group_thread_priority()?;
        let state = self.parse_optional_resource_group_state()?;
        let force = self.eat_contextual_keyword("FORCE")?;
        let span = start.union(self.preceding_span());
        Ok(Statement::AlterResourceGroup {
            alter: Box::new(AlterResourceGroup {
                name,
                vcpu,
                thread_priority,
                state,
                force,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `DROP RESOURCE GROUP <name> [FORCE]` after the `GROUP` keyword (already
    /// consumed). Names exactly one group; no `IF EXISTS`, no comma list.
    fn parse_drop_resource_group(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        let force = self.eat_contextual_keyword("FORCE")?;
        let span = start.union(self.preceding_span());
        Ok(Statement::DropResourceGroup {
            drop: Box::new(DropResourceGroup {
                name,
                force,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse the optional `VCPU [=] <range> [[,] <range> …]` clause of a resource-group
    /// `CREATE`/`ALTER`. The range separator is `opt_comma` — a comma or bare whitespace both
    /// parse (`VCPU = 0 1 2` grammar-accepts on mysql:8.4.10) — so the list continues while the
    /// next token is a number, comma-or-not. Each bound is a decimal `NUM` (a `0x` hex bound is
    /// `ER_PARSE_ERROR`, unlike the `real_ulonglong_num` srid).
    fn parse_optional_resource_group_vcpu(&mut self) -> ParseResult<Option<ResourceGroupVcpu>> {
        let start = self.current_span()?;
        if !self.eat_contextual_keyword("VCPU")? {
            return Ok(None);
        }
        let equals = self.eat_op(Operator::Eq)?;
        let mut ranges = ThinVec::new();
        loop {
            let range_start = self.current_span()?;
            let range_bound_start = self.expect_decimal_integer_literal("VCPU")?;
            let end = if self.eat_op(Operator::Minus)? {
                Some(self.expect_decimal_integer_literal("`-` in a VCPU range")?)
            } else {
                None
            };
            ranges.push(VcpuRange {
                start: range_bound_start,
                end,
                meta: self.make_meta(range_start.union(self.preceding_span())),
            });
            let ate_comma = self.eat_punct(Punctuation::Comma)?;
            // `opt_comma`: another range follows on a comma or directly on the next number.
            let next_is_number = self
                .peek()?
                .is_some_and(|token| token.kind == TokenKind::Number);
            if !ate_comma && !next_is_number {
                break;
            }
        }
        Ok(Some(ResourceGroupVcpu {
            equals,
            ranges,
            meta: self.make_meta(start.union(self.preceding_span())),
        }))
    }

    /// Parse the optional `THREAD_PRIORITY [=] <signed_num>` clause of a resource-group
    /// `CREATE`/`ALTER`. The value is an optionally `-`-signed decimal `NUM`
    /// (`THREAD_PRIORITY = -5` grammar-accepts on mysql:8.4.10).
    fn parse_optional_resource_group_thread_priority(
        &mut self,
    ) -> ParseResult<Option<ResourceGroupThreadPriority>> {
        let start = self.current_span()?;
        if !self.eat_contextual_keyword("THREAD_PRIORITY")? {
            return Ok(None);
        }
        let equals = self.eat_op(Operator::Eq)?;
        let negative = self.eat_op(Operator::Minus)?;
        let value = self.expect_decimal_integer_literal("THREAD_PRIORITY")?;
        Ok(Some(ResourceGroupThreadPriority {
            equals,
            negative,
            value,
            meta: self.make_meta(start.union(self.preceding_span())),
        }))
    }

    /// Parse the optional `ENABLE`/`DISABLE` state clause of a resource-group `CREATE`/`ALTER`.
    fn parse_optional_resource_group_state(&mut self) -> ParseResult<Option<ResourceGroupState>> {
        Ok(if self.eat_contextual_keyword("ENABLE")? {
            Some(ResourceGroupState::Enable)
        } else if self.eat_contextual_keyword("DISABLE")? {
            Some(ResourceGroupState::Disable)
        } else {
            None
        })
    }

    /// Parse DuckDB's `ALTER SEQUENCE …` after the `SEQUENCE` keyword (already consumed). A
    /// following `SET SCHEMA` is the [`AlterObjectSchema`] relocation (dispatched here so the
    /// shared `SEQUENCE` head reaches both productions); otherwise the trailing `SeqOptList`
    /// is the [`AlterSequence`] option statement.
    fn parse_alter_sequence(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let if_exists = self.parse_schema_change_if_exists()?;
        let name = self.parse_target_relation_name()?;
        if self.features().statement_ddl_gates.alter_object_set_schema
            && self.peek_is_keyword(Keyword::Set)?
            && self.peek_nth_is_contextual_keyword(1, "SCHEMA")?
        {
            return self.finish_alter_object_set_schema(
                start,
                SchemaRelocationObject::Sequence,
                if_exists,
                name,
            );
        }
        let options = self.parse_alter_sequence_options()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::AlterSequence {
            alter: Box::new(AlterSequence {
                if_exists,
                name,
                options,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse DuckDB's `SeqOptList` for `ALTER SEQUENCE` — one or more options. The shared
    /// generator core reuses [`parse_identity_option`](Self::parse_identity_option) (with
    /// `CACHE` admitted, which `ALTER SEQUENCE` accepts unlike `CREATE SEQUENCE`); the
    /// ALTER-only leads (`RESTART`, `AS`, `OWNED BY`, `SEQUENCE NAME`) are parsed here.
    fn parse_alter_sequence_options(
        &mut self,
    ) -> ParseResult<ThinVec<AlterSequenceOption<D::Ext>>> {
        let mut options = ThinVec::new();
        loop {
            let start = self.current_span()?;
            if self.eat_contextual_keyword("RESTART")? {
                // DuckDB's `RESTART` is bare or `RESTART [WITH] NumericOnly`; a value follows
                // only on `WITH` or a signed number, never on the next option keyword.
                let has_with = self.peek_is_keyword(Keyword::With)?;
                let starts_number = self
                    .peek()?
                    .is_some_and(|token| token.kind == TokenKind::Number)
                    || self.peek_is_op(Operator::Plus)?
                    || self.peek_is_op(Operator::Minus)?;
                let value = if has_with || starts_number {
                    self.eat_keyword(Keyword::With)?;
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                options.push(AlterSequenceOption::Restart {
                    value,
                    meta: self.make_meta(start.union(self.preceding_span())),
                });
            } else if self.eat_keyword(Keyword::As)? {
                let data_type = self.parse_data_type()?;
                options.push(AlterSequenceOption::As {
                    data_type,
                    meta: self.make_meta(start.union(self.preceding_span())),
                });
            } else if self.eat_contextual_keyword("OWNED")? {
                self.expect_keyword(Keyword::By)?;
                // `OWNED BY NONE` detaches the sequence; any other `any_name` is the owning
                // column (DuckDB spells the sentinel as the bare word `NONE`).
                let owner = if self.eat_contextual_keyword("NONE")? {
                    None
                } else {
                    Some(self.parse_object_name()?)
                };
                options.push(AlterSequenceOption::OwnedBy {
                    owner,
                    meta: self.make_meta(start.union(self.preceding_span())),
                });
            } else if self.peek_is_contextual_keyword("SEQUENCE")?
                && self.peek_nth_is_contextual_keyword(1, "NAME")?
            {
                self.expect_contextual_keyword("SEQUENCE")?;
                self.expect_contextual_keyword("NAME")?;
                let name = self.parse_object_name()?;
                options.push(AlterSequenceOption::SequenceName {
                    name,
                    meta: self.make_meta(start.union(self.preceding_span())),
                });
            } else if self.peek_is_sequence_option()? || self.peek_is_contextual_keyword("CACHE")? {
                let option = self.parse_identity_option(true)?;
                options.push(AlterSequenceOption::Common {
                    option,
                    meta: self.make_meta(start.union(self.preceding_span())),
                });
            } else {
                break;
            }
        }
        // DuckDB's `SeqOptList` is one-or-more; a bare `ALTER SEQUENCE s` is a syntax error.
        if options.is_empty() {
            return Err(self.unexpected("an `ALTER SEQUENCE` option"));
        }
        Ok(options)
    }

    /// Finish an [`AlterObjectSchema`] once the object head, `IF EXISTS`, and name are parsed:
    /// consume the trailing `SET SCHEMA <schema>` and build the node. Shared by the TABLE,
    /// VIEW, and SEQUENCE heads.
    fn finish_alter_object_set_schema(
        &mut self,
        start: Span,
        object_type: SchemaRelocationObject,
        if_exists: bool,
        name: ObjectName,
    ) -> ParseResult<Statement<D::Ext>> {
        self.expect_keyword(Keyword::Set)?;
        self.expect_contextual_keyword("SCHEMA")?;
        let new_schema = self.parse_ident()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::AlterObjectSchema {
            alter: Box::new(AlterObjectSchema {
                object_type,
                if_exists,
                name,
                new_schema,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `ALTER {PROCEDURE | FUNCTION} <name> [<characteristic> …]` after the object
    /// keyword (already consumed). Characteristics only — no parameter list, no body — and the
    /// `ALTER`-legal characteristic subset (the server rejects `DETERMINISTIC` here, which
    /// `parse_routine_characteristics(false)` enforces by leaving it unconsumed).
    fn parse_alter_routine(
        &mut self,
        start: Span,
        kind: RoutineKind,
    ) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_target_relation_name()?;
        let characteristics = self.parse_routine_characteristics(false)?;
        let span = start.union(self.preceding_span());
        Ok(Statement::AlterRoutine {
            alter: Box::new(AlterRoutine {
                kind,
                name,
                characteristics,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `CREATE [DEFINER = <user>] EVENT [IF NOT EXISTS] <name> ON SCHEDULE
    /// <schedule> [ON COMPLETION [NOT] PRESERVE] [ENABLE | DISABLE [ON SLAVE|REPLICA]]
    /// [COMMENT '…'] DO <body>` after the `EVENT` keyword (already consumed). The clause order
    /// after the name is fixed by the grammar (server-measured: a comment before the status is
    /// a syntax error). `definer` is the already-parsed prefix.
    fn parse_create_event(
        &mut self,
        start: Span,
        definer: Option<Definer>,
    ) -> ParseResult<Statement<D::Ext>> {
        let if_not_exists = self.parse_if_not_exists()?;
        let name = self.parse_target_relation_name()?;
        self.expect_contextual_keyword("ON")?;
        self.expect_contextual_keyword("SCHEDULE")?;
        let schedule = self.parse_event_schedule()?;
        let on_completion = self.parse_event_on_completion()?;
        let status = self.parse_event_status()?;
        let comment = self.parse_event_comment()?;
        self.expect_contextual_keyword("DO")?;
        // An event body carries no return value (server `ER_SP_BADRETURN`), so `RETURN` is
        // rejected at parse level exactly as in a procedure body.
        let body = self.parse_routine_body_statement(false)?;
        let span = start.union(self.preceding_span());
        let create = CreateEvent {
            definer: definer.map(Box::new),
            if_not_exists,
            name,
            schedule,
            on_completion,
            status,
            comment,
            body: Box::new(body),
            meta: self.make_meta(span),
        };
        Ok(Statement::CreateEvent {
            create: Box::new(create),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `ALTER [DEFINER = <user>] EVENT <name> …` after the `EVENT` keyword
    /// (already consumed). Every clause is optional but at least one must appear (server-
    /// measured: a bare `ALTER EVENT e` is a syntax error). The clause order matches
    /// [`parse_create_event`](Self::parse_create_event) with `RENAME TO` between the
    /// schedule/completion and the status.
    fn parse_alter_event(
        &mut self,
        start: Span,
        definer: Option<Definer>,
    ) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_target_relation_name()?;
        // `ev_alter_on_schedule_completion`: the schedule and completion are independent here
        // (completion may appear alone, without a schedule), unlike CREATE where completion
        // follows a mandatory schedule. A leading `ON` opens `ON SCHEDULE …` (then an optional
        // trailing `ON COMPLETION …`) or a bare `ON COMPLETION …`.
        let (schedule, mut on_completion) = if self.eat_contextual_keyword("ON")? {
            if self.eat_contextual_keyword("SCHEDULE")? {
                (Some(self.parse_event_schedule()?), None)
            } else {
                self.expect_contextual_keyword("COMPLETION")?;
                (None, Some(self.finish_event_on_completion()?))
            }
        } else {
            (None, None)
        };
        if on_completion.is_none() {
            on_completion = self.parse_event_on_completion()?;
        }
        let rename_to = if self.eat_contextual_keyword("RENAME")? {
            self.expect_contextual_keyword("TO")?;
            Some(self.parse_target_relation_name()?)
        } else {
            None
        };
        let status = self.parse_event_status()?;
        let comment = self.parse_event_comment()?;
        let body = if self.eat_contextual_keyword("DO")? {
            Some(Box::new(self.parse_routine_body_statement(false)?))
        } else {
            None
        };
        // The grammar requires at least one clause; a bare `ALTER EVENT e` is a syntax error.
        if schedule.is_none()
            && on_completion.is_none()
            && rename_to.is_none()
            && status.is_none()
            && comment.is_none()
            && body.is_none()
        {
            return Err(self.unexpected(
                "at least one `ALTER EVENT` clause (ON SCHEDULE, ON COMPLETION, RENAME TO, \
                 ENABLE/DISABLE, COMMENT, or DO)",
            ));
        }
        let span = start.union(self.preceding_span());
        let alter = AlterEvent {
            definer: definer.map(Box::new),
            name,
            schedule,
            on_completion,
            rename_to,
            status,
            comment,
            body,
            meta: self.make_meta(span),
        };
        Ok(Statement::AlterEvent {
            alter: Box::new(alter),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `DROP EVENT [IF EXISTS] <name>` after the `EVENT` keyword (already
    /// consumed). Exactly one event name — no comma list, no `CASCADE`/`RESTRICT` (server-
    /// measured: both are syntax errors), so this does not route through the shared
    /// [`DropStatement`] grammar.
    fn parse_drop_event(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let if_exists = self.parse_schema_change_if_exists()?;
        let name = self.parse_target_relation_name()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::DropEvent {
            drop: Box::new(DropEvent {
                if_exists,
                name,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `DROP {DATABASE | SCHEMA} [IF EXISTS] <name>` after the already-consumed
    /// `DATABASE`/`SCHEMA` keyword (`spelling` records which). The name is a bare identifier
    /// (`ident`) — a dotted `db.x` is a server syntax error, so [`parse_ident`](Self::parse_ident)
    /// (not [`parse_object_name`](Self::parse_object_name)) enforces the single unqualified name.
    /// Exactly one name, no `CASCADE`/`RESTRICT`, so this does not route through the shared
    /// [`DropStatement`] grammar.
    fn parse_drop_database(
        &mut self,
        start: Span,
        spelling: DatabaseKeyword,
    ) -> ParseResult<Statement<D::Ext>> {
        let if_exists = self.parse_schema_change_if_exists()?;
        let name = self.parse_ident()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::DropDatabase {
            drop: Box::new(DropDatabase {
                spelling,
                if_exists,
                name,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a MySQL `DROP INDEX <name> ON <table> [ALGORITHM [=] …] [LOCK [=] …]` after the
    /// already-consumed `INDEX` keyword (`drop_index_stmt`, `sql_yacc.yy`). The index name is a
    /// bare identifier (`ident`; a dotted `i.j` is a syntax error) and the `ON` and its table are
    /// mandatory (`DROP INDEX i` with no `ON` is `ER_PARSE_ERROR`); the table may be
    /// schema-qualified (`db.t`). The trailing `opt_index_lock_and_algorithm` hints follow.
    fn parse_drop_index_on_table(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        self.expect_keyword(Keyword::On)?;
        let table = self.parse_object_name()?;
        let options = self.parse_index_lock_algorithm_options()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::DropIndex {
            drop: Box::new(DropIndexOnTable {
                name,
                table,
                options,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse the trailing `opt_index_lock_and_algorithm` tail — an optional `ALGORITHM [=] …`
    /// and/or `LOCK [=] …` in either order (`sql_yacc.yy`). The grammar admits at most one of
    /// each; a repeated keyword stops the loop, so the duplicate surfaces as trailing input and
    /// the statement rejects (matching the server's `ER_PARSE_ERROR` on `ALGORITHM … ALGORITHM
    /// …`). Source order and the optional `=` are preserved for round-trip.
    fn parse_index_lock_algorithm_options(
        &mut self,
    ) -> ParseResult<ThinVec<IndexLockAlgorithmOption>> {
        let mut options = ThinVec::new();
        let mut seen_algorithm = false;
        let mut seen_lock = false;
        loop {
            if !seen_algorithm && self.eat_contextual_keyword("ALGORITHM")? {
                seen_algorithm = true;
                let equals = self.eat_op(Operator::Eq)?;
                let value = self.parse_index_algorithm_value()?;
                options.push(IndexLockAlgorithmOption::Algorithm { equals, value });
            } else if !seen_lock && self.eat_contextual_keyword("LOCK")? {
                seen_lock = true;
                let equals = self.eat_op(Operator::Eq)?;
                let value = self.parse_index_lock_value()?;
                options.push(IndexLockAlgorithmOption::Lock { equals, value });
            } else {
                break;
            }
        }
        Ok(options)
    }

    /// Parse an `alter_algorithm_option_value` — `DEFAULT` (a keyword) or one of the
    /// case-insensitive identifiers `INPLACE`/`INSTANT`/`COPY`. An unknown value is a server
    /// *binding* reject (`ER_UNKNOWN_ALTER_ALGORITHM`), not a syntax error; only the
    /// bind-valid set is modelled, so an unknown value surfaces as a clean parse error here.
    fn parse_index_algorithm_value(&mut self) -> ParseResult<IndexAlgorithm> {
        if self.eat_keyword(Keyword::Default)? {
            Ok(IndexAlgorithm::Default)
        } else if self.eat_contextual_keyword("INPLACE")? {
            Ok(IndexAlgorithm::Inplace)
        } else if self.eat_contextual_keyword("INSTANT")? {
            Ok(IndexAlgorithm::Instant)
        } else if self.eat_contextual_keyword("COPY")? {
            Ok(IndexAlgorithm::Copy)
        } else {
            Err(self.unexpected("an ALGORITHM value (DEFAULT, INPLACE, INSTANT, or COPY)"))
        }
    }

    /// Parse an `alter_lock_option_value` — `DEFAULT` (a keyword) or one of the
    /// case-insensitive identifiers `NONE`/`SHARED`/`EXCLUSIVE`. An unknown value is a server
    /// *binding* reject (`ER_UNKNOWN_ALTER_LOCK`), not a syntax error; only the bind-valid set
    /// is modelled, so an unknown value surfaces as a clean parse error here.
    fn parse_index_lock_value(&mut self) -> ParseResult<IndexLock> {
        if self.eat_keyword(Keyword::Default)? {
            Ok(IndexLock::Default)
        } else if self.eat_contextual_keyword("NONE")? {
            Ok(IndexLock::None)
        } else if self.eat_contextual_keyword("SHARED")? {
            Ok(IndexLock::Shared)
        } else if self.eat_contextual_keyword("EXCLUSIVE")? {
            Ok(IndexLock::Exclusive)
        } else {
            Err(self.unexpected("a LOCK value (DEFAULT, NONE, SHARED, or EXCLUSIVE)"))
        }
    }

    /// Parse the `ON SCHEDULE` body: `AT <ts>` (one-shot) or `EVERY <value> <unit>
    /// [STARTS <ts>] [ENDS <ts>]` (recurring). The `AT`/`STARTS`/`ENDS` timestamps are
    /// ordinary expressions (a `NOW() + INTERVAL 1 DAY` offset rides the expression grammar).
    fn parse_event_schedule(&mut self) -> ParseResult<EventSchedule<D::Ext>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("AT")? {
            let at = Box::new(self.parse_expr()?);
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(EventSchedule::At { at, meta });
        }
        self.expect_contextual_keyword("EVERY")?;
        let value = Box::new(self.parse_expr()?);
        let unit = self.parse_mysql_interval_unit()?;
        let starts = if self.eat_contextual_keyword("STARTS")? {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let ends = if self.eat_contextual_keyword("ENDS")? {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(EventSchedule::Every {
            value,
            unit,
            starts,
            ends,
            meta,
        })
    }

    /// Parse one MySQL `interval` unit keyword into the shared [`IntervalFields`] vocabulary —
    /// the simple units (`YEAR`…`MICROSECOND`) and the underscore composites (`DAY_HOUR`,
    /// `MINUTE_SECOND`, `YEAR_MONTH`, the four `*_MICROSECOND` forms). Reuses the shared
    /// vocabulary rather than a bespoke enum; the composite units MySQL spells with an
    /// underscore map onto the same `*To*` variants the ANSI grammar spells with `TO`.
    fn parse_mysql_interval_unit(&mut self) -> ParseResult<IntervalFields> {
        match self.try_parse_mysql_interval_unit()? {
            Some(unit) => Ok(unit),
            None => Err(self.unexpected("a MySQL interval unit (DAY, HOUR, MINUTE_SECOND, …)")),
        }
    }

    /// Consume one MySQL `interval` unit keyword if the cursor is on one, returning `None`
    /// (no cursor movement) otherwise. The `Err`-on-absence sibling
    /// [`parse_mysql_interval_unit`](Self::parse_mysql_interval_unit) drives the mandatory
    /// event-schedule unit; the operator-position interval reader
    /// ([`try_parse_mysql_interval_operator`](Self::try_parse_mysql_interval_operator)) needs
    /// the optional form so it can decline (and fall through to the typed-string literal path).
    pub(in crate::parser) fn try_parse_mysql_interval_unit(
        &mut self,
    ) -> ParseResult<Option<IntervalFields>> {
        // Longer composite spellings are distinct whole tokens (the underscore is an identifier
        // character), so there is no shared-prefix ambiguity with the simple units.
        const UNITS: &[(&str, IntervalFields)] = &[
            ("YEAR_MONTH", IntervalFields::YearToMonth),
            ("DAY_MICROSECOND", IntervalFields::DayToMicrosecond),
            ("DAY_HOUR", IntervalFields::DayToHour),
            ("DAY_MINUTE", IntervalFields::DayToMinute),
            ("DAY_SECOND", IntervalFields::DayToSecond),
            ("HOUR_MICROSECOND", IntervalFields::HourToMicrosecond),
            ("HOUR_MINUTE", IntervalFields::HourToMinute),
            ("HOUR_SECOND", IntervalFields::HourToSecond),
            ("MINUTE_MICROSECOND", IntervalFields::MinuteToMicrosecond),
            ("MINUTE_SECOND", IntervalFields::MinuteToSecond),
            ("SECOND_MICROSECOND", IntervalFields::SecondToMicrosecond),
            ("YEAR", IntervalFields::Year),
            ("QUARTER", IntervalFields::Quarter),
            ("MONTH", IntervalFields::Month),
            ("WEEK", IntervalFields::Week),
            ("DAY", IntervalFields::Day),
            ("HOUR", IntervalFields::Hour),
            ("MINUTE", IntervalFields::Minute),
            ("SECOND", IntervalFields::Second),
            ("MICROSECOND", IntervalFields::Microsecond),
        ];
        for (spelling, unit) in UNITS {
            if self.eat_contextual_keyword(spelling)? {
                return Ok(Some(*unit));
            }
        }
        Ok(None)
    }

    /// Parse the optional `ON COMPLETION [NOT] PRESERVE` clause. Peeks `ON COMPLETION` before
    /// committing so a bare `ON` opening another clause is left untouched.
    fn parse_event_on_completion(&mut self) -> ParseResult<Option<EventOnCompletion>> {
        if self.peek_is_contextual_keyword("ON")?
            && self.peek_nth_is_contextual_keyword(1, "COMPLETION")?
        {
            self.expect_contextual_keyword("ON")?;
            self.expect_contextual_keyword("COMPLETION")?;
            return Ok(Some(self.finish_event_on_completion()?));
        }
        Ok(None)
    }

    /// Finish `[NOT] PRESERVE` after a consumed `ON COMPLETION`.
    fn finish_event_on_completion(&mut self) -> ParseResult<EventOnCompletion> {
        let not = self.eat_contextual_keyword("NOT")?;
        self.expect_contextual_keyword("PRESERVE")?;
        Ok(if not {
            EventOnCompletion::NotPreserve
        } else {
            EventOnCompletion::Preserve
        })
    }

    /// Parse the optional `ENABLE | DISABLE [ON SLAVE|REPLICA]` status clause. MySQL 8.4 admits
    /// both the deprecated `ON SLAVE` and the current `ON REPLICA` spellings, retained as a
    /// [`ReplicaSpelling`] round-trip tag.
    fn parse_event_status(&mut self) -> ParseResult<Option<EventStatus>> {
        if self.eat_contextual_keyword("ENABLE")? {
            return Ok(Some(EventStatus::Enable));
        }
        if self.eat_contextual_keyword("DISABLE")? {
            if self.eat_contextual_keyword("ON")? {
                let spelling = if self.eat_contextual_keyword("SLAVE")? {
                    ReplicaSpelling::Slave
                } else {
                    self.expect_contextual_keyword("REPLICA")?;
                    ReplicaSpelling::Replica
                };
                return Ok(Some(EventStatus::DisableOnReplica(spelling)));
            }
            return Ok(Some(EventStatus::Disable));
        }
        Ok(None)
    }

    /// Parse the optional `COMMENT '…'` event description (a string literal).
    fn parse_event_comment(&mut self) -> ParseResult<Option<Literal>> {
        if self.eat_contextual_keyword("COMMENT")? {
            Ok(Some(self.expect_string_literal(
                "an event COMMENT string literal",
            )?))
        } else {
            Ok(None)
        }
    }

    /// Parse a `CREATE EXTENSION [IF NOT EXISTS] <name> [WITH] [<option> ...]` body after
    /// the introducing `CREATE EXTENSION`.
    fn parse_create_extension(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let if_not_exists = self.parse_if_not_exists()?;
        let name = self.parse_ident()?;
        // `opt_with` — the `WITH` keyword is optional sugar before the option list.
        let with = self.eat_keyword(Keyword::With)?;
        let mut options = ThinVec::new();
        while let Some(option) = self.parse_create_extension_option()? {
            options.push(option);
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::CreateExtension {
            create: Box::new(CreateExtension {
                if_not_exists,
                name,
                with,
                options,
                meta,
            }),
            meta,
        })
    }

    /// Parse one `create_extension_opt_item` (`SCHEMA`/`VERSION`/`CASCADE`), or `None`
    /// when the next token starts no option. PostgreSQL's `FROM <old_version>` item is a
    /// parse-time `FEATURE_NOT_SUPPORTED` reject in its grammar action, so it is absent here.
    fn parse_create_extension_option(&mut self) -> ParseResult<Option<CreateExtensionOption>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("SCHEMA")? {
            let name = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CreateExtensionOption::Schema { name, meta }));
        }
        if self.eat_contextual_keyword("VERSION")? {
            let version = self.parse_extension_version()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CreateExtensionOption::Version { version, meta }));
        }
        if self.eat_contextual_keyword("CASCADE")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CreateExtensionOption::Cascade { meta }));
        }
        Ok(None)
    }

    /// Parse a `NonReservedWord_or_Sconst` version value: a string constant or a bare word.
    ///
    /// The string arm admits only an `Sconst` (plain, `E'…'`, `U&'…'`, dollar-quoted); a
    /// bit/hex (`b'…'`/`x'…'`) or national (`N'…'`) constant is not an `Sconst`, so it falls
    /// through to [`parse_ident`](Self::parse_ident), which rejects a string token — matching
    /// libpg_query, which rejects `VERSION b'0'`/`x'ab'`/`N'x'` where it accepts `VERSION 'v'`
    /// (see [`string_literal_is_sconst`]). A reserved word is likewise the `parse_ident` reject.
    fn parse_extension_version(&mut self) -> ParseResult<ExtensionVersion> {
        let start = self.current_span()?;
        if matches!(self.peek()?, Some(token)
            if token.kind == TokenKind::String
                && string_literal_is_sconst(self.span_text(token.span)))
        {
            let value = self.expect_string_literal("a version string or word")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ExtensionVersion::String { value, meta });
        }
        let word = self.parse_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ExtensionVersion::Word { word, meta })
    }

    /// Parse an `ALTER EXTENSION <name> {UPDATE [TO v] | ADD <member> | DROP <member>}`
    /// body after the introducing `ALTER EXTENSION`.
    fn parse_alter_extension(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        let action_start = self.current_span()?;
        let action = if self.eat_contextual_keyword("UPDATE")? {
            // `alter_extension_opt_list` admits only the `TO <version>` item.
            let version = if self.eat_contextual_keyword("TO")? {
                Some(self.parse_extension_version()?)
            } else {
                None
            };
            AlterExtensionAction::Update {
                version,
                meta: self.make_meta(action_start.union(self.preceding_span())),
            }
        } else {
            let add = if self.eat_contextual_keyword("ADD")? {
                true
            } else if self.eat_contextual_keyword("DROP")? {
                false
            } else {
                return Err(self.unexpected("`UPDATE`, `ADD`, or `DROP` after the extension name"));
            };
            let member = self.parse_object_reference()?;
            AlterExtensionAction::Change {
                add,
                member,
                meta: self.make_meta(action_start.union(self.preceding_span())),
            }
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::AlterExtension {
            alter: Box::new(AlterExtension { name, action, meta }),
            meta,
        })
    }

    /// Parse a `CREATE [UNDO] TABLESPACE <name> [ADD DATAFILE '<f>'] [USE LOGFILE GROUP <lg>]
    /// [<option>...]` body after the introducing `CREATE [UNDO] TABLESPACE` keywords.
    ///
    /// `ADD DATAFILE` is mandatory for the `UNDO` form (grammar `ADD ts_datafile`) and optional
    /// otherwise; `USE LOGFILE GROUP` is NDB-only and never appears on the `UNDO` form.
    fn parse_create_tablespace(
        &mut self,
        start: Span,
        undo: bool,
    ) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        let datafile = self.parse_optional_add_datafile()?;
        if undo && datafile.is_none() {
            return Err(self.unexpected("`ADD DATAFILE` (required by `CREATE UNDO TABLESPACE`)"));
        }
        let use_logfile_group = if !undo && self.eat_contextual_keyword("USE")? {
            self.expect_contextual_keyword("LOGFILE")?;
            self.expect_contextual_keyword("GROUP")?;
            Some(self.parse_ident()?)
        } else {
            None
        };
        let ctx = if undo {
            TsOptionCtx::UndoTablespace
        } else {
            TsOptionCtx::CreateTablespace
        };
        let options = self.parse_tablespace_options(ctx)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::CreateTablespace {
            create: Box::new(CreateTablespace {
                undo,
                name,
                datafile,
                use_logfile_group,
                options,
                meta,
            }),
            meta,
        })
    }

    /// Parse an optional `ADD DATAFILE '<file>'` clause; `None` when no `ADD` leads.
    fn parse_optional_add_datafile(&mut self) -> ParseResult<Option<Literal>> {
        if !self.eat_contextual_keyword("ADD")? {
            return Ok(None);
        }
        self.expect_contextual_keyword("DATAFILE")?;
        Ok(Some(self.expect_string_literal(
            "a datafile path string after `ADD DATAFILE`",
        )?))
    }

    /// Parse an `ALTER TABLESPACE <name> <action>` body after the introducing keywords: an
    /// `ADD`/`DROP DATAFILE` (with a trailing option list), a `RENAME TO` (no options), or a bare
    /// non-empty option list.
    fn parse_alter_tablespace(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        let action_start = self.current_span()?;
        let action = if self.eat_contextual_keyword("ADD")? {
            self.expect_contextual_keyword("DATAFILE")?;
            let datafile =
                self.expect_string_literal("a datafile path string after `ADD DATAFILE`")?;
            let options = self.parse_tablespace_options(TsOptionCtx::AlterTablespace)?;
            AlterTablespaceAction::AddDatafile {
                datafile,
                options,
                meta: self.make_meta(action_start.union(self.preceding_span())),
            }
        } else if self.eat_contextual_keyword("DROP")? {
            self.expect_contextual_keyword("DATAFILE")?;
            let datafile =
                self.expect_string_literal("a datafile path string after `DROP DATAFILE`")?;
            let options = self.parse_tablespace_options(TsOptionCtx::AlterTablespace)?;
            AlterTablespaceAction::DropDatafile {
                datafile,
                options,
                meta: self.make_meta(action_start.union(self.preceding_span())),
            }
        } else if self.eat_contextual_keyword("RENAME")? {
            self.expect_contextual_keyword("TO")?;
            let new_name = self.parse_ident()?;
            AlterTablespaceAction::Rename {
                new_name,
                meta: self.make_meta(action_start.union(self.preceding_span())),
            }
        } else {
            // The bare `alter_tablespace_option_list` is non-empty; an empty tail is a parse error
            // (server-measured: `ALTER TABLESPACE ts` alone is `ER_PARSE_ERROR`).
            let options = self.parse_tablespace_options(TsOptionCtx::AlterTablespace)?;
            if options.is_empty() {
                return Err(
                    self.unexpected("`ADD`/`DROP DATAFILE`, `RENAME TO`, or a tablespace option")
                );
            }
            AlterTablespaceAction::Options {
                options,
                meta: self.make_meta(action_start.union(self.preceding_span())),
            }
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::AlterTablespace {
            alter: Box::new(AlterTablespace { name, action, meta }),
            meta,
        })
    }

    /// Parse an `ALTER UNDO TABLESPACE <name> SET {ACTIVE | INACTIVE} [<option>...]` body after the
    /// introducing `ALTER UNDO TABLESPACE` keywords.
    fn parse_alter_undo_tablespace(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        let action_start = self.current_span()?;
        self.expect_keyword(Keyword::Set)?;
        let state = if self.eat_contextual_keyword("ACTIVE")? {
            UndoTablespaceState::Active
        } else if self.eat_contextual_keyword("INACTIVE")? {
            UndoTablespaceState::Inactive
        } else {
            return Err(self.unexpected("`ACTIVE` or `INACTIVE` after `SET`"));
        };
        let options = self.parse_tablespace_options(TsOptionCtx::UndoTablespace)?;
        let action = AlterTablespaceAction::SetState {
            state,
            options,
            meta: self.make_meta(action_start.union(self.preceding_span())),
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::AlterTablespace {
            alter: Box::new(AlterTablespace { name, action, meta }),
            meta,
        })
    }

    /// Parse a `DROP [UNDO] TABLESPACE <name> [<option>...]` body after the introducing keywords.
    fn parse_drop_tablespace(&mut self, start: Span, undo: bool) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        // `DROP UNDO TABLESPACE` takes `ENGINE` alone (`opt_undo_tablespace_options`); the plain
        // form takes `ENGINE`/`WAIT` (`opt_drop_ts_options`).
        let ctx = if undo {
            TsOptionCtx::UndoTablespace
        } else {
            TsOptionCtx::DropTablespace
        };
        let options = self.parse_tablespace_options(ctx)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::DropTablespace {
            drop: Box::new(DropTablespace {
                undo,
                name,
                options,
                meta,
            }),
            meta,
        })
    }

    /// Parse a `CREATE LOGFILE GROUP <name> ADD UNDOFILE '<f>' [<option>...]` body after the
    /// introducing `CREATE LOGFILE GROUP` keywords. `ADD UNDOFILE` is mandatory.
    fn parse_create_logfile_group(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        let undofile = self.parse_add_undofile()?;
        let options = self.parse_tablespace_options(TsOptionCtx::CreateLogfileGroup)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::CreateLogfileGroup {
            create: Box::new(CreateLogfileGroup {
                name,
                undofile,
                options,
                meta,
            }),
            meta,
        })
    }

    /// Parse an `ALTER LOGFILE GROUP <name> ADD UNDOFILE '<f>' [<option>...]` body after the
    /// introducing `ALTER LOGFILE GROUP` keywords. `ADD UNDOFILE` is mandatory.
    fn parse_alter_logfile_group(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        let undofile = self.parse_add_undofile()?;
        let options = self.parse_tablespace_options(TsOptionCtx::AlterLogfileGroup)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::AlterLogfileGroup {
            alter: Box::new(AlterLogfileGroup {
                name,
                undofile,
                options,
                meta,
            }),
            meta,
        })
    }

    /// Parse the mandatory `ADD UNDOFILE '<file>'` clause of a `CREATE`/`ALTER LOGFILE GROUP`.
    fn parse_add_undofile(&mut self) -> ParseResult<Literal> {
        self.expect_contextual_keyword("ADD")?;
        self.expect_contextual_keyword("UNDOFILE")?;
        self.expect_string_literal("an undofile path string after `ADD UNDOFILE`")
    }

    /// Parse a `DROP LOGFILE GROUP <name> [<option>...]` body after the introducing keywords. Its
    /// option set is `ENGINE`/`WAIT` (`opt_drop_ts_options`, shared with `DROP TABLESPACE`).
    fn parse_drop_logfile_group(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let name = self.parse_ident()?;
        let options = self.parse_tablespace_options(TsOptionCtx::DropTablespace)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::DropLogfileGroup {
            drop: Box::new(DropLogfileGroup {
                name,
                options,
                meta,
            }),
            meta,
        })
    }

    /// Parse a storage-DDL option list (`<option> [opt_comma <option>]...`) for `ctx`. Consumes as
    /// many context-valid options as follow; the optional comma separator (`opt_comma`) is
    /// admitted between options but a trailing comma with no following option is a parse error.
    /// An empty list is returned where the grammar's `opt_*` wrapper allows it — the two mandatory-
    /// option contexts (`ALTER TABLESPACE` bare form) enforce non-emptiness at their call site.
    fn parse_tablespace_options(
        &mut self,
        ctx: TsOptionCtx,
    ) -> ParseResult<ThinVec<TablespaceOption>> {
        let mut options = ThinVec::new();
        match self.parse_one_tablespace_option(ctx)? {
            Some(option) => options.push(option),
            None => return Ok(options),
        }
        loop {
            let had_comma = self.eat_punct(Punctuation::Comma)?;
            match self.parse_one_tablespace_option(ctx)? {
                Some(option) => options.push(option),
                None => {
                    if had_comma {
                        return Err(self.unexpected("a storage option after `,`"));
                    }
                    break;
                }
            }
        }
        Ok(options)
    }

    /// Parse one context-valid storage-DDL option, or `None` when the next token starts no option
    /// admissible in `ctx` (ending the list). Each context accepts only its own subset of the
    /// shared `ts_option_*` family, so an out-of-context keyword returns `None` and later surfaces
    /// as a leftover-token parse error — reproducing the server's per-context grammar.
    fn parse_one_tablespace_option(
        &mut self,
        ctx: TsOptionCtx,
    ) -> ParseResult<Option<TablespaceOption>> {
        use TsOptionCtx::{
            AlterLogfileGroup, AlterTablespace, CreateLogfileGroup, CreateTablespace,
            UndoTablespace,
        };
        let start = self.current_span()?;

        // Size-valued options (`<KEYWORD> [=] size_number`).
        let size_kind = if matches!(
            ctx,
            CreateTablespace | AlterTablespace | CreateLogfileGroup | AlterLogfileGroup
        ) && self.peek_is_contextual_keyword("INITIAL_SIZE")?
        {
            Some(TablespaceSizeOption::InitialSize)
        } else if matches!(ctx, CreateTablespace | AlterTablespace)
            && self.peek_is_contextual_keyword("AUTOEXTEND_SIZE")?
        {
            Some(TablespaceSizeOption::AutoextendSize)
        } else if matches!(ctx, CreateTablespace | AlterTablespace)
            && self.peek_is_contextual_keyword("MAX_SIZE")?
        {
            Some(TablespaceSizeOption::MaxSize)
        } else if matches!(ctx, CreateTablespace)
            && self.peek_is_contextual_keyword("EXTENT_SIZE")?
        {
            Some(TablespaceSizeOption::ExtentSize)
        } else if matches!(ctx, CreateLogfileGroup)
            && self.peek_is_contextual_keyword("UNDO_BUFFER_SIZE")?
        {
            Some(TablespaceSizeOption::UndoBufferSize)
        } else if matches!(ctx, CreateLogfileGroup)
            && self.peek_is_contextual_keyword("REDO_BUFFER_SIZE")?
        {
            Some(TablespaceSizeOption::RedoBufferSize)
        } else if matches!(ctx, CreateTablespace)
            && self.peek_is_contextual_keyword("FILE_BLOCK_SIZE")?
        {
            Some(TablespaceSizeOption::FileBlockSize)
        } else {
            None
        };
        if let Some(kind) = size_kind {
            self.advance()?; // the option keyword
            let equals = self.eat_op(Operator::Eq)?;
            let size = self.parse_size_literal()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(TablespaceOption::Size {
                kind,
                equals,
                size,
                meta,
            }));
        }

        // `NODEGROUP [=] <n>` — a plain integer (`real_ulong_num`).
        if matches!(ctx, CreateTablespace | CreateLogfileGroup)
            && self.eat_contextual_keyword("NODEGROUP")?
        {
            let equals = self.eat_op(Operator::Eq)?;
            let value = self.parse_tablespace_integer()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(TablespaceOption::Nodegroup {
                equals,
                value,
                meta,
            }));
        }

        // `[STORAGE] ENGINE [=] <ident_or_text>` — admissible in every context.
        let storage = self.peek_is_contextual_keyword("STORAGE")?
            && self.peek_nth_is_contextual_keyword(1, "ENGINE")?;
        if storage {
            self.advance()?; // STORAGE
        }
        if storage || self.peek_is_contextual_keyword("ENGINE")? {
            self.expect_contextual_keyword("ENGINE")?;
            let equals = self.eat_op(Operator::Eq)?;
            let name = self.parse_ident_or_text()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(TablespaceOption::Engine {
                storage,
                equals,
                name,
                meta,
            }));
        }

        // `WAIT` / `NO_WAIT` — admissible everywhere except the `UNDO` contexts.
        if !matches!(ctx, UndoTablespace) {
            if self.eat_contextual_keyword("WAIT")? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(Some(TablespaceOption::Wait {
                    negated: false,
                    meta,
                }));
            }
            if self.eat_contextual_keyword("NO_WAIT")? {
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(Some(TablespaceOption::Wait {
                    negated: true,
                    meta,
                }));
            }
        }

        // `COMMENT [=] '<text>'`.
        if matches!(ctx, CreateTablespace | CreateLogfileGroup)
            && self.eat_contextual_keyword("COMMENT")?
        {
            let equals = self.eat_op(Operator::Eq)?;
            let value = self.expect_string_literal("a comment string")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(TablespaceOption::Comment {
                equals,
                value,
                meta,
            }));
        }

        // `ENCRYPTION [=] '<y_or_n>'`.
        if matches!(ctx, CreateTablespace | AlterTablespace)
            && self.eat_contextual_keyword("ENCRYPTION")?
        {
            let equals = self.eat_op(Operator::Eq)?;
            let value = self.expect_string_literal("an encryption flag string ('Y'/'N')")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(TablespaceOption::Encryption {
                equals,
                value,
                meta,
            }));
        }

        // `ENGINE_ATTRIBUTE [=] '<json>'`.
        if matches!(ctx, CreateTablespace | AlterTablespace)
            && self.eat_contextual_keyword("ENGINE_ATTRIBUTE")?
        {
            let equals = self.eat_op(Operator::Eq)?;
            let value = self.expect_string_literal("a JSON engine-attribute string")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(TablespaceOption::EngineAttribute {
                equals,
                value,
                meta,
            }));
        }

        Ok(None)
    }

    /// Parse a `size_number` value: a plain decimal integer (`134217728`) or an integer with a
    /// binary unit suffix (`128M`, `2G`, `16k`).
    ///
    /// MySQL lexes the suffixed form as one `IDENT_sys` token (an unquoted identifier may begin
    /// with digits), so the digits and the suffix letter must abut — this tokenizer splits `16M`
    /// into a `Number` token and an adjacent word, which are rejoined here by span adjacency. A
    /// non-adjacent word (`16 M`), a multi-character suffix (`16MB`), or an unknown suffix letter
    /// leaves the word unconsumed, so the trailing token surfaces as a parse error — matching the
    /// server (which rejects those forms, `16 M` at parse and `16MB` as `ER_WRONG_SIZE_NUMBER`).
    fn parse_size_literal(&mut self) -> ParseResult<SizeLiteral> {
        let start = self.current_span()?;
        let number = self
            .peek()?
            .filter(|token| token.kind == TokenKind::Number)
            .ok_or_else(|| {
                self.unexpected("a size literal (an integer, optionally with a K/M/G suffix)")
            })?;
        // `size_number` admits only an integer base; a float/scientific/hex spelling is not a size.
        if !self
            .span_text(number.span)
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        {
            return Err(self.unexpected("an integer size literal"));
        }
        let number_end = number.span.end();
        self.advance()?; // the digits

        // The suffix letter is an adjacent word-like token — a plain `Word`, or a `Keyword` when
        // the letter happens to be a reserved word (e.g. `M`). Case-insensitive, exactly one of
        // `K`/`M`/`G`.
        let unit = match self.peek()? {
            Some(token)
                if matches!(token.kind, TokenKind::Word | TokenKind::Keyword(_))
                    && token.span.start() == number_end =>
            {
                let suffix = self.span_text(token.span);
                let unit = if suffix.eq_ignore_ascii_case("K") {
                    Some(SizeUnit::Kilo)
                } else if suffix.eq_ignore_ascii_case("M") {
                    Some(SizeUnit::Mega)
                } else if suffix.eq_ignore_ascii_case("G") {
                    Some(SizeUnit::Giga)
                } else {
                    None
                };
                if unit.is_some() {
                    self.advance()?; // the suffix letter
                }
                unit
            }
            _ => None,
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(SizeLiteral { unit, meta })
    }

    /// Parse a plain unsigned integer [`Literal`] (a `Number` token) for a `NODEGROUP` value.
    fn parse_tablespace_integer(&mut self) -> ParseResult<Literal> {
        match self.peek()? {
            Some(token) if token.kind == TokenKind::Number => {
                self.advance()?;
                Ok(Literal {
                    kind: LiteralKind::Integer,
                    meta: self.make_meta(token.span),
                })
            }
            _ => Err(self.unexpected("an integer node-group id")),
        }
    }

    /// Parse the object of an `AlterObjectDependsStmt`, or `None` when the next token
    /// starts no `DEPENDS` object head (so the caller falls through to `ALTER TABLE`).
    ///
    /// PostgreSQL's `DEPENDS ON EXTENSION` grammar admits only four object heads —
    /// `FUNCTION`/`PROCEDURE`/`ROUTINE` (a `function_with_argtypes`),
    /// `TRIGGER <name> ON <table>`, `MATERIALIZED VIEW`, and `INDEX` — a strict subset of
    /// the extension-member [`ObjectReference`] axis, so this dispatch is deliberately
    /// narrower than
    /// [`parse_object_reference`](Self::parse_object_reference) (which would over-accept
    /// `TABLE`, `SEQUENCE`, `VIEW`, … here).
    fn parse_alter_depends_object(&mut self) -> ParseResult<Option<ObjectReference<D::Ext>>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("FUNCTION")? {
            return Ok(Some(
                self.parse_routine_reference(start, RoutineObjectKind::Function)?,
            ));
        }
        if self.eat_contextual_keyword("PROCEDURE")? {
            return Ok(Some(
                self.parse_routine_reference(start, RoutineObjectKind::Procedure)?,
            ));
        }
        if self.eat_contextual_keyword("ROUTINE")? {
            return Ok(Some(
                self.parse_routine_reference(start, RoutineObjectKind::Routine)?,
            ));
        }
        if self.eat_contextual_keyword("TRIGGER")? {
            // `ALTER TRIGGER name ON qualified_name` — the trigger name is a bare `ColId`,
            // the table it fires on is the qualifiable name.
            let name = self.parse_ident()?;
            self.expect_keyword(Keyword::On)?;
            let table = self.parse_object_name()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ObjectReference::Trigger { name, table, meta }));
        }
        if self.eat_contextual_keyword("MATERIALIZED")? {
            self.expect_contextual_keyword("VIEW")?;
            let name = self.parse_object_name()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ObjectReference::Named {
                kind: ObjectRefKind::MaterializedView,
                name,
                meta,
            }));
        }
        if self.eat_contextual_keyword("INDEX")? {
            let name = self.parse_object_name()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(ObjectReference::Named {
                kind: ObjectRefKind::Index,
                name,
                meta,
            }));
        }
        Ok(None)
    }

    /// Parse the `[NO] DEPENDS ON EXTENSION <extension>` tail after the object of an
    /// `AlterObjectDependsStmt`. The extension is a bare `name` (not schema-qualifiable).
    fn parse_alter_object_depends(
        &mut self,
        start: Span,
        object: ObjectReference<D::Ext>,
    ) -> ParseResult<Statement<D::Ext>> {
        // `opt_no` precedes `DEPENDS`; `NO DEPENDS` removes the recorded dependency.
        let no = self.eat_keyword(Keyword::No)?;
        self.expect_contextual_keyword("DEPENDS")?;
        self.expect_keyword(Keyword::On)?;
        self.expect_contextual_keyword("EXTENSION")?;
        let extension = self.parse_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::AlterObjectDepends {
            alter: Box::new(AlterObjectDepends {
                object,
                no,
                extension,
                meta,
            }),
            meta,
        })
    }

    /// Parse an `ALTER SYSTEM { SET <name> {= | TO} <value> | RESET <name> | RESET ALL }`
    /// statement (PostgreSQL's `AlterSystemStmt`), with the leading `ALTER SYSTEM` already
    /// consumed and `start` the `ALTER` span.
    ///
    /// The setting-name / value grammar is PostgreSQL's `generic_set` / `generic_reset`,
    /// which is exactly the session-`SET` / `RESET` value axis — so the shared
    /// [`parse_set_value`](Self::parse_set_value), [`expect_set_assignment`](Self::expect_set_assignment),
    /// and [`parse_config_parameter`](Self::parse_config_parameter) helpers are reused rather
    /// than re-minted. Unlike the session `SET`, `ALTER SYSTEM SET` takes no `SESSION`/
    /// `LOCAL` scope: a scope keyword is not consumed here, so it lands in setting-name
    /// position and (with the following name) surfaces as the parse error PostgreSQL reports.
    fn parse_alter_system(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let action = if self.eat_contextual_keyword("SET")? {
            let name = self.parse_object_name()?;
            let assignment = self.expect_set_assignment()?;
            let value = self.parse_set_value()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            AlterSystemAction::Set {
                name,
                assignment,
                value,
                meta,
            }
        } else if self.eat_contextual_keyword("RESET")? {
            let target = self.parse_config_parameter()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            AlterSystemAction::Reset { target, meta }
        } else {
            return Err(self.unexpected("`SET` or `RESET` after `ALTER SYSTEM`"));
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Statement::AlterSystem {
            alter: Box::new(AlterSystem { action, meta }),
            meta,
        })
    }

    /// Parse a member-object reference — the shared object-reference axis behind
    /// `ALTER EXTENSION … ADD|DROP <member>` (and reused by the sibling object-DDL heads).
    /// Dispatches on the object-kind keyword to the kind-appropriate signature grammar.
    fn parse_object_reference(&mut self) -> ParseResult<ObjectReference<D::Ext>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("FUNCTION")? {
            return self.parse_routine_reference(start, RoutineObjectKind::Function);
        }
        if self.eat_contextual_keyword("PROCEDURE")? {
            return self.parse_routine_reference(start, RoutineObjectKind::Procedure);
        }
        if self.eat_contextual_keyword("ROUTINE")? {
            return self.parse_routine_reference(start, RoutineObjectKind::Routine);
        }
        if self.eat_contextual_keyword("AGGREGATE")? {
            return self.parse_aggregate_reference(start);
        }
        if self.eat_contextual_keyword("OPERATOR")? {
            return self.parse_operator_reference(start);
        }
        if self.eat_contextual_keyword("CAST")? {
            self.expect_punct(Punctuation::LParen, "`(` after `CAST`")?;
            let from = self.parse_data_type()?;
            self.expect_contextual_keyword("AS")?;
            let to = self.parse_data_type()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the cast signature")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ObjectReference::Cast { from, to, meta });
        }
        if self.eat_contextual_keyword("DOMAIN")? {
            let name = self.parse_data_type()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ObjectReference::Type {
                domain: true,
                name,
                meta,
            });
        }
        if self.eat_contextual_keyword("TYPE")? {
            let name = self.parse_data_type()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ObjectReference::Type {
                domain: false,
                name,
                meta,
            });
        }
        if self.eat_contextual_keyword("TRANSFORM")? {
            self.expect_contextual_keyword("FOR")?;
            let type_name = self.parse_data_type()?;
            self.expect_contextual_keyword("LANGUAGE")?;
            let language = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ObjectReference::Transform {
                type_name,
                language,
                meta,
            });
        }
        let kind = self.parse_object_ref_kind()?;
        // `object_type_any_name` kinds take a dotted `any_name`; `object_type_name` kinds
        // only a single `name` (PostgreSQL rejects `SCHEMA s.bad`).
        let name = if kind.schema_qualifiable() {
            self.parse_object_name()?
        } else {
            ObjectName(thin_vec![self.parse_ident()?])
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ObjectReference::Named { kind, name, meta })
    }

    /// Parse the tail of a `FUNCTION`/`PROCEDURE`/`ROUTINE` member — a
    /// `function_with_argtypes` (name plus optional argument-type list).
    fn parse_routine_reference(
        &mut self,
        start: Span,
        kind: RoutineObjectKind,
    ) -> ParseResult<ObjectReference<D::Ext>> {
        let name = self.parse_object_name()?;
        let arg_types = self.parse_optional_routine_arg_types()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        let signature = RoutineSignature {
            name,
            arg_types,
            meta,
        };
        Ok(ObjectReference::Routine {
            kind,
            signature,
            meta,
        })
    }

    /// Parse the tail of an `AGGREGATE` member — `name aggr_args`.
    fn parse_aggregate_reference(&mut self, start: Span) -> ParseResult<ObjectReference<D::Ext>> {
        let name = self.parse_object_name()?;
        let args_start = self.current_span()?;
        self.expect_punct(Punctuation::LParen, "`(` in the aggregate signature")?;
        let args = if self.peek_is_op(Operator::Star)? {
            self.advance()?;
            AggregateArgs::Star {
                meta: self.make_meta(args_start.union(self.preceding_span())),
            }
        } else {
            // `(ORDER BY …)` has no direct args; otherwise a direct type list optionally
            // followed by an ordered-set `ORDER BY` list.
            let direct = if self.peek_is_contextual_keyword("ORDER")? {
                ThinVec::new()
            } else {
                self.parse_data_type_list()?
            };
            let order_by = if self.eat_contextual_keyword("ORDER")? {
                self.expect_contextual_keyword("BY")?;
                Some(self.parse_data_type_list()?)
            } else {
                None
            };
            AggregateArgs::Types {
                direct,
                order_by,
                meta: self.make_meta(args_start.union(self.preceding_span())),
            }
        };
        self.expect_punct(Punctuation::RParen, "`)` to close the aggregate signature")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ObjectReference::Aggregate { name, args, meta })
    }

    /// Parse the tail of an `OPERATOR` member — either `CLASS`/`FAMILY <name> USING <am>`
    /// or an `operator_with_argtypes` (`[<schema>.]<sym>(<left>, <right>)`).
    fn parse_operator_reference(&mut self, start: Span) -> ParseResult<ObjectReference<D::Ext>> {
        if self.eat_contextual_keyword("CLASS")? || self.eat_contextual_keyword("FAMILY")? {
            // Both branches are folded: re-check which keyword matched via the preceding
            // token so the `family` flag is set correctly.
            let family = self
                .span_text(self.preceding_span())
                .eq_ignore_ascii_case("FAMILY");
            let name = self.parse_object_name()?;
            self.expect_contextual_keyword("USING")?;
            let access_method = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ObjectReference::OperatorClass {
                family,
                name,
                access_method,
                meta,
            });
        }
        // `any_operator` — an optional `(ColId '.')*` schema chain then the operator symbol.
        let mut schema = ThinVec::new();
        while self
            .peek()?
            .is_some_and(|token| self.token_can_be_column_name(token))
            && self.peek_nth_is_punct(1, Punctuation::Dot)?
        {
            schema.push(self.parse_ident()?);
            self.expect_punct(Punctuation::Dot, "`.` in the qualified operator name")?;
        }
        let op = self.parse_operator_symbol()?;
        let args = self.parse_operator_args()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ObjectReference::Operator {
            schema: ObjectName(schema),
            op,
            args,
            meta,
        })
    }

    /// Parse an `oper_argtypes` `(<left>, <right>)`, where a `NONE` operand is `None`.
    /// PostgreSQL requires at least one real operand (a unary operator names its missing
    /// side `NONE`), so `(NONE, NONE)` is a reject.
    fn parse_operator_args(&mut self) -> ParseResult<OperatorArgs<D::Ext>> {
        let start = self.current_span()?;
        self.expect_punct(Punctuation::LParen, "`(` in the operator signature")?;
        let left = self.parse_operator_operand()?;
        self.expect_punct(Punctuation::Comma, "`,` in the operator signature")?;
        let right = self.parse_operator_operand()?;
        self.expect_punct(Punctuation::RParen, "`)` to close the operator signature")?;
        if left.is_none() && right.is_none() {
            return Err(self.unexpected("at least one operand type in the operator signature"));
        }
        Ok(OperatorArgs {
            left,
            right,
            meta: self.make_meta(start.union(self.preceding_span())),
        })
    }

    /// Parse one operator operand — a `Typename`, or `NONE` (a unary operator's missing side).
    fn parse_operator_operand(&mut self) -> ParseResult<Option<DataType<D::Ext>>> {
        if self.eat_contextual_keyword("NONE")? {
            Ok(None)
        } else {
            Ok(Some(self.parse_data_type()?))
        }
    }

    /// Parse a non-empty comma-separated `Typename` list.
    fn parse_data_type_list(&mut self) -> ParseResult<ThinVec<DataType<D::Ext>>> {
        let mut types = ThinVec::new();
        types.push(self.parse_data_type()?);
        while self.eat_punct(Punctuation::Comma)? {
            types.push(self.parse_data_type()?);
        }
        Ok(types)
    }

    /// Parse a name-only member-object kind (PostgreSQL's `object_type_any_name` and
    /// `object_type_name` productions), consuming the kind keyword(s).
    fn parse_object_ref_kind(&mut self) -> ParseResult<ObjectRefKind> {
        if self.eat_contextual_keyword("TABLE")? {
            Ok(ObjectRefKind::Table)
        } else if self.eat_contextual_keyword("SEQUENCE")? {
            Ok(ObjectRefKind::Sequence)
        } else if self.eat_contextual_keyword("MATERIALIZED")? {
            self.expect_contextual_keyword("VIEW")?;
            Ok(ObjectRefKind::MaterializedView)
        } else if self.eat_contextual_keyword("VIEW")? {
            Ok(ObjectRefKind::View)
        } else if self.eat_contextual_keyword("INDEX")? {
            Ok(ObjectRefKind::Index)
        } else if self.eat_contextual_keyword("FOREIGN")? {
            if self.eat_contextual_keyword("TABLE")? {
                Ok(ObjectRefKind::ForeignTable)
            } else {
                self.expect_contextual_keyword("DATA")?;
                self.expect_contextual_keyword("WRAPPER")?;
                Ok(ObjectRefKind::ForeignDataWrapper)
            }
        } else if self.eat_contextual_keyword("COLLATION")? {
            Ok(ObjectRefKind::Collation)
        } else if self.eat_contextual_keyword("CONVERSION")? {
            Ok(ObjectRefKind::Conversion)
        } else if self.eat_contextual_keyword("STATISTICS")? {
            Ok(ObjectRefKind::Statistics)
        } else if self.eat_contextual_keyword("TEXT")? {
            self.expect_contextual_keyword("SEARCH")?;
            if self.eat_contextual_keyword("PARSER")? {
                Ok(ObjectRefKind::TextSearchParser)
            } else if self.eat_contextual_keyword("DICTIONARY")? {
                Ok(ObjectRefKind::TextSearchDictionary)
            } else if self.eat_contextual_keyword("TEMPLATE")? {
                Ok(ObjectRefKind::TextSearchTemplate)
            } else {
                self.expect_contextual_keyword("CONFIGURATION")?;
                Ok(ObjectRefKind::TextSearchConfiguration)
            }
        } else if self.eat_contextual_keyword("ACCESS")? {
            self.expect_contextual_keyword("METHOD")?;
            Ok(ObjectRefKind::AccessMethod)
        } else if self.eat_contextual_keyword("EVENT")? {
            self.expect_contextual_keyword("TRIGGER")?;
            Ok(ObjectRefKind::EventTrigger)
        } else if self.eat_contextual_keyword("EXTENSION")? {
            Ok(ObjectRefKind::Extension)
        } else if self.eat_contextual_keyword("PROCEDURAL")? {
            self.expect_contextual_keyword("LANGUAGE")?;
            Ok(ObjectRefKind::Language)
        } else if self.eat_contextual_keyword("LANGUAGE")? {
            Ok(ObjectRefKind::Language)
        } else if self.eat_contextual_keyword("PUBLICATION")? {
            Ok(ObjectRefKind::Publication)
        } else if self.eat_contextual_keyword("SCHEMA")? {
            Ok(ObjectRefKind::Schema)
        } else if self.eat_contextual_keyword("SERVER")? {
            Ok(ObjectRefKind::Server)
        } else if self.eat_contextual_keyword("DATABASE")? {
            Ok(ObjectRefKind::Database)
        } else if self.eat_contextual_keyword("ROLE")? {
            Ok(ObjectRefKind::Role)
        } else if self.eat_contextual_keyword("TABLESPACE")? {
            Ok(ObjectRefKind::Tablespace)
        } else {
            Err(self.unexpected("an object kind after `ADD`/`DROP`"))
        }
    }

    fn parse_alter_table_action(&mut self) -> ParseResult<AlterTableAction<D::Ext>> {
        let start = self.current_span()?;
        if self.features().statement_ddl_gates.colocation_groups
            && self.peek_is_contextual_keyword("SET")?
            && self.peek_nth_is_contextual_keyword(1, "COLOCATION")?
        {
            self.advance()?;
            self.advance()?;
            self.expect_contextual_keyword("GROUP")?;
            let group = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AlterTableAction::SetColocationGroup { group, meta });
        }
        if self.features().statement_ddl_gates.colocation_groups
            && self.peek_is_contextual_keyword("DROP")?
            && self.peek_nth_is_contextual_keyword(1, "COLOCATION")?
        {
            self.advance()?;
            self.advance()?;
            self.expect_contextual_keyword("GROUP")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AlterTableAction::DropColocationGroup { meta });
        }
        if self.eat_contextual_keyword("ADD")? {
            return self.parse_alter_table_add(start);
        }
        if self.eat_contextual_keyword("DROP")? {
            return self.parse_alter_table_drop(start);
        }
        if self.features().index_alter_syntax.alter_table_extended
            && self.eat_contextual_keyword("ALTER")?
        {
            let column_keyword = self.eat_contextual_keyword("COLUMN")?;
            let name = self.parse_ident()?;
            let change = self.parse_alter_column_action()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(AlterTableAction::AlterColumn {
                column_keyword,
                name,
                change,
                meta,
            });
        }
        if self.features().index_alter_syntax.alter_table_set_options
            && self.eat_contextual_keyword("SET")?
        {
            let params = self.parse_reloptions_list()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AlterTableAction::SetOptions { params, meta });
        }
        if self.eat_contextual_keyword("RENAME")? {
            return self.parse_alter_table_rename(start);
        }
        Err(self.unexpected("`ADD`, `DROP`, `ALTER`, or `RENAME`"))
    }

    /// Parse a `RENAME` action: `RENAME TO <name>` (the table) or `RENAME [COLUMN]
    /// <name> TO <name>` (a column).
    fn parse_alter_table_rename(&mut self, start: Span) -> ParseResult<AlterTableAction<D::Ext>> {
        if self.eat_contextual_keyword("TO")? {
            let new_name = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AlterTableAction::RenameTable { new_name, meta });
        }
        if self.features().index_alter_syntax.rename_constraint
            && self.eat_contextual_keyword("CONSTRAINT")?
        {
            let name = self.parse_ident()?;
            self.expect_contextual_keyword("TO")?;
            let new_name = self.parse_ident()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AlterTableAction::RenameConstraint {
                name,
                new_name,
                meta,
            });
        }
        let column_keyword = self.eat_contextual_keyword("COLUMN")?;
        let name = self.parse_alter_column_target(
            self.features().index_alter_syntax.alter_nested_column_paths,
        )?;
        self.expect_contextual_keyword("TO")?;
        let new_name = self.parse_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(AlterTableAction::RenameColumn {
            column_keyword,
            name,
            new_name,
            meta,
        })
    }

    /// Parse a standalone `ATTACH PARTITION <name> <bound>` or `DETACH PARTITION <name>
    /// [CONCURRENTLY | FINALIZE]` action (PostgreSQL declarative partitioning). Reached only
    /// under [`CreateTableClauseSyntax::declarative_partitioning`](crate::ast::dialect::CreateTableClauseSyntax),
    /// with `ATTACH`/`DETACH` already confirmed at the cursor.
    fn parse_alter_table_partition_action(&mut self) -> ParseResult<AlterTableAction<D::Ext>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("ATTACH")? {
            self.expect_contextual_keyword("PARTITION")?;
            let partition = self.parse_target_relation_name()?;
            let bound = self.parse_partition_bound()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AlterTableAction::AttachPartition {
                partition,
                bound: Box::new(bound),
                meta,
            });
        }
        self.expect_contextual_keyword("DETACH")?;
        self.expect_contextual_keyword("PARTITION")?;
        let partition = self.parse_target_relation_name()?;
        let mode = if self.eat_contextual_keyword("CONCURRENTLY")? {
            Some(DetachPartitionMode::Concurrently)
        } else if self.eat_contextual_keyword("FINALIZE")? {
            Some(DetachPartitionMode::Finalize)
        } else {
            None
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(AlterTableAction::DetachPartition {
            partition,
            mode,
            meta,
        })
    }

    /// Parse an `ADD` action: a column definition or a table constraint.
    ///
    /// `ADD [COLUMN] [IF NOT EXISTS] <column>` and `ADD <table constraint>` share the
    /// `ADD` keyword; an explicit `COLUMN` keyword, a `CONSTRAINT` name, or one of the
    /// constraint-leading keywords decides which production follows.
    fn parse_alter_table_add(&mut self, start: Span) -> ParseResult<AlterTableAction<D::Ext>> {
        if self.eat_contextual_keyword("COLUMN")? {
            return self.parse_alter_table_add_column(start, true);
        }
        // SQLite's lenient `ALTER TABLE … ADD` accepts a `CHECK` table constraint (bare or
        // `CONSTRAINT <name> CHECK`, engine-measured via rusqlite) but rejects `PRIMARY
        // KEY` / `UNIQUE` / `FOREIGN KEY` — so it admits only the `CHECK` kind, while the
        // extended dialects admit the whole table-constraint grammar.
        let take_constraint = if self.features().index_alter_syntax.alter_table_extended {
            self.peek_is_contextual_keyword("CONSTRAINT")? || self.peek_starts_table_constraint()?
        } else {
            self.peek_starts_check_constraint()?
        };
        if take_constraint {
            let constraint = self.parse_table_constraint_def()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(AlterTableAction::AddConstraint { constraint, meta });
        }
        self.parse_alter_table_add_column(start, false)
    }

    fn parse_alter_table_add_column(
        &mut self,
        start: Span,
        column_keyword: bool,
    ) -> ParseResult<AlterTableAction<D::Ext>> {
        // SQLite's `ADD COLUMN` has no `IF NOT EXISTS` guard, and MySQL (extended, but no
        // ALTER existence guards) rejects it too.
        let if_not_exists = self.features().index_alter_syntax.alter_table_extended
            && self.features().index_alter_syntax.alter_existence_guards
            && self.parse_schema_change_if_not_exists()?;
        let column_start = self.current_span()?;
        let (target, name) = if self.features().index_alter_syntax.alter_nested_column_paths {
            let path = self.parse_alter_column_target(true)?;
            let name = path
                .parts
                .last()
                .cloned()
                .expect("alter column target has at least one part");
            let target = if path.parts.len() > 1 {
                Some(path)
            } else {
                None
            };
            (target, name)
        } else {
            (None, self.parse_ident()?)
        };
        let column = self.parse_column_def_tail(column_start, name)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(AlterTableAction::AddColumn {
            if_not_exists,
            column_keyword,
            target,
            column,
            meta,
        })
    }

    /// Parse a `DROP` action: `DROP CONSTRAINT ...` or `DROP [COLUMN] ...`.
    fn parse_alter_table_drop(&mut self, start: Span) -> ParseResult<AlterTableAction<D::Ext>> {
        // SQLite's `ALTER TABLE … DROP` admits `DROP [COLUMN]` and `DROP CONSTRAINT`
        // (both engine-measured-accepted via rusqlite), but no `IF EXISTS` guard.
        let extended = self.features().index_alter_syntax.alter_table_extended;
        // MySQL has the extended `DROP [COLUMN|CONSTRAINT]` surface but rejects the
        // per-action `IF EXISTS` guard (`alter_existence_guards` off).
        let existence_guards =
            extended && self.features().index_alter_syntax.alter_existence_guards;
        if self.features().index_alter_syntax.drop_primary_key
            && self.eat_contextual_keyword("PRIMARY")?
        {
            self.expect_contextual_keyword("KEY")?;
            let behavior = self.parse_drop_behavior()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(AlterTableAction::DropPrimaryKey { behavior, meta });
        }
        if self.eat_contextual_keyword("CONSTRAINT")? {
            let if_exists = existence_guards && self.parse_schema_change_if_exists()?;
            let name = self.parse_ident()?;
            let behavior = self.parse_drop_behavior()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(AlterTableAction::DropConstraint {
                if_exists,
                name,
                behavior,
                meta,
            });
        }
        // MySQL's `ALTER TABLE … DROP {INDEX|KEY} <name>` — gated by
        // `alter_table_drop_index`, which no preset models the parse of yet, so this reject
        // is universal for now. Detect the two-token shape — the `INDEX`/`KEY` keyword
        // followed by an index NAME — and name the keyword; without this guard the keyword is
        // swallowed as the dropped column's name and the reject surfaces at the index name
        // instead. The disambiguation from a column literally named `index`/`key`: after the
        // column form (`DROP index [CASCADE|RESTRICT]`) only a drop-behavior word or a
        // terminator (`;`, `,`, end of input) can follow, whereas the index form is followed
        // by a name. An index name may itself be a non-reserved keyword (e.g. `a`, which is a
        // `Keyword` token, not a `Word`), so a name is any word/quoted/keyword token that is
        // not the `CASCADE`/`RESTRICT` drop-behavior. A bare `DROP index` (nothing following)
        // still routes to the column path below; when a dialect implements the action (flag
        // on), it handles it ahead of this guard.
        if !self.features().index_alter_syntax.alter_table_drop_index
            && (self.peek_is_contextual_keyword("INDEX")?
                || self.peek_is_contextual_keyword("KEY")?)
            && self.peek_nth(1)?.is_some_and(|token| {
                matches!(
                    token.kind,
                    TokenKind::Word | TokenKind::QuotedIdent | TokenKind::Keyword(_)
                )
            })
            && !self.peek_nth_is_contextual_keyword(1, "CASCADE")?
            && !self.peek_nth_is_contextual_keyword(1, "RESTRICT")?
        {
            return Err(self.unexpected("a column or constraint to drop"));
        }
        let column_keyword = self.eat_contextual_keyword("COLUMN")?;
        let if_exists = existence_guards && self.parse_schema_change_if_exists()?;
        let name = self.parse_alter_column_target(
            self.features().index_alter_syntax.alter_nested_column_paths,
        )?;
        let behavior = self.parse_drop_behavior()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(AlterTableAction::DropColumn {
            if_exists,
            column_keyword,
            name,
            behavior,
            meta,
        })
    }

    fn parse_alter_column_target(&mut self, nested_paths: bool) -> ParseResult<AlterColumnTarget> {
        let start = self.current_span()?;
        let parts = if nested_paths {
            self.parse_object_name()?.0
        } else {
            thin_vec![self.parse_ident()?]
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(AlterColumnTarget { parts, meta })
    }

    fn parse_alter_column_action(&mut self) -> ParseResult<AlterColumnAction<D::Ext>> {
        let start = self.current_span()?;
        // The PostgreSQL `ALTER COLUMN` actions beyond `SET`/`DROP DEFAULT` — `SET DATA
        // TYPE`/`TYPE`, `SET`/`DROP NOT NULL`. MySQL's `ALTER COLUMN` has only the `DEFAULT`
        // actions (type/nullability changes go through `MODIFY`/`CHANGE`), so with this off
        // the extra actions surface as a clean parse error.
        let type_and_null = self
            .features()
            .index_alter_syntax
            .alter_column_set_data_type;
        if self.features().index_alter_syntax.alter_column_add_identity
            && self.eat_contextual_keyword("ADD")?
        {
            self.expect_contextual_keyword("GENERATED")?;
            let generation = if self.eat_contextual_keyword("ALWAYS")? {
                IdentityGeneration::Always
            } else if self.eat_keyword(Keyword::By)? {
                self.expect_contextual_keyword("DEFAULT")?;
                IdentityGeneration::ByDefault
            } else {
                return Err(self.unexpected("`ALWAYS` or `BY DEFAULT`"));
            };
            self.expect_keyword(Keyword::As)?;
            self.expect_contextual_keyword("IDENTITY")?;
            let options = self.parse_identity_options()?;
            let span = start.union(self.preceding_span());
            let identity = IdentityColumn {
                generation,
                options,
                meta: self.make_meta(span),
            };
            return Ok(AlterColumnAction::AddIdentity {
                identity: Box::new(identity),
                meta: self.make_meta(span),
            });
        }
        if self.eat_contextual_keyword("SET")? {
            if self.eat_contextual_keyword("DEFAULT")? {
                let expr = self.parse_expr()?;
                let span = start.union(expr.span());
                let meta = self.make_meta(span);
                return Ok(AlterColumnAction::SetDefault {
                    expr: Box::new(expr),
                    meta,
                });
            }
            if type_and_null && self.eat_keyword(Keyword::Not)? {
                self.expect_keyword(Keyword::Null)?;
                let span = start.union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(AlterColumnAction::SetNotNull { meta });
            }
            if type_and_null && self.eat_contextual_keyword("DATA")? {
                self.expect_contextual_keyword("TYPE")?;
                return self.parse_alter_column_set_data_type(start, true);
            }
            return Err(self.unexpected(if type_and_null {
                "`DEFAULT`, `NOT NULL`, or `DATA TYPE`"
            } else {
                "`DEFAULT`"
            }));
        }
        if self.eat_contextual_keyword("DROP")? {
            if self.eat_contextual_keyword("DEFAULT")? {
                let span = start.union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(AlterColumnAction::DropDefault { meta });
            }
            if type_and_null && self.eat_keyword(Keyword::Not)? {
                self.expect_keyword(Keyword::Null)?;
                let span = start.union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(AlterColumnAction::DropNotNull { meta });
            }
            return Err(self.unexpected(if type_and_null {
                "`DEFAULT` or `NOT NULL`"
            } else {
                "`DEFAULT`"
            }));
        }
        // PostgreSQL accepts a bare `TYPE <type>` as a synonym for `SET DATA TYPE`.
        if type_and_null && self.eat_contextual_keyword("TYPE")? {
            return self.parse_alter_column_set_data_type(start, false);
        }
        Err(self.unexpected(if type_and_null {
            "`SET`, `DROP`, or `TYPE`"
        } else {
            "`SET` or `DROP`"
        }))
    }

    fn parse_alter_column_set_data_type(
        &mut self,
        start: Span,
        set_data: bool,
    ) -> ParseResult<AlterColumnAction<D::Ext>> {
        let data_type = self.parse_data_type()?;
        let using = if self.eat_contextual_keyword("USING")? {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(AlterColumnAction::SetDataType {
            set_data,
            data_type,
            using,
            meta,
        })
    }

    /// Parse a `DROP {TABLE | VIEW | MATERIALIZED VIEW | INDEX | SCHEMA}` statement,
    /// or a `DROP {FUNCTION | PROCEDURE | ROUTINE}` (a signature drop, split off).
    pub(super) fn parse_drop_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("DROP")?;
        if self.features().statement_ddl_gates.colocation_groups
            && self.eat_contextual_keyword("COLOCATION")?
        {
            return self.parse_drop_colocation_group(start);
        }
        if self.features().statement_ddl_gates.routines {
            if let Some(kind) = self.try_parse_routine_object_kind()? {
                return self.parse_drop_routine(start, kind);
            }
        }
        // `DROP TRANSFORM` names a `(type, language)` pair, not a name list, so it takes a
        // dedicated statement reusing the shared transform reference axis — dispatched here
        // before `parse_drop_object_kind`, which has no `TRANSFORM` kind.
        if self.features().statement_ddl_gates.transform_ddl
            && self.peek_is_contextual_keyword("TRANSFORM")?
        {
            return self.parse_drop_transform(start);
        }
        // MySQL `DROP EVENT [IF EXISTS] <name>` — a single-name drop with no `CASCADE`/`RESTRICT`,
        // so it does not route through the shared name-list drop grammar below. Gated on the
        // stored-program surface; off elsewhere `EVENT` falls through and surfaces as an unknown
        // drop object kind.
        if self.features().statement_ddl_gates.compound_statements
            && self.eat_contextual_keyword("EVENT")?
        {
            return self.parse_drop_event(start);
        }
        // DuckDB `DROP [PERSISTENT | TEMPORARY] SECRET …` is the only DROP with its own
        // top-level production (`drop_secret.y`), not a `parse_drop_object_kind` name-list
        // kind: it carries the `opt_persist` persistence modifier and a `FROM <storage>`
        // backend selector. Gated by the same `create_secret` flag that admits CREATE SECRET
        // (one secrets behaviour surface); dispatched before `parse_drop_object_kind`, whose
        // kind set has no `SECRET`.
        if self.features().statement_ddl_gates.create_secret && self.peek_is_drop_secret()? {
            return self.parse_drop_secret(start);
        }
        // MySQL account-management DDL — `DROP USER` / `DROP ROLE`. A whole-statement gate,
        // intercepted before the generic `DROP <object> <name-list>` path because the name list
        // is `user@host` account names, not the object names that path expects. Off elsewhere,
        // `USER`/`ROLE` fall through to `parse_drop_object_kind` and surface as an unknown drop.
        if self.features().access_control_syntax.user_role_management {
            if self.eat_contextual_keyword("USER")? {
                return self.parse_user_role_list(start, UserRoleListKind::DropUser);
            }
            if self.eat_contextual_keyword("ROLE")? {
                return self.parse_user_role_list(start, UserRoleListKind::DropRole);
            }
        }
        // MySQL `DROP PREPARE <name>` — the `deallocate_or_drop` DROP synonym for `DEALLOCATE
        // PREPARE`, releasing a prepared statement (not a catalogue object). Intercepted before
        // the generic name-list drop path, whose object-kind set has no `PREPARE`; gated by the
        // same `prepared_statements_from` flag as `PREPARE ... FROM`. Off elsewhere, `PREPARE`
        // falls through and surfaces as an unknown drop object kind.
        if self.features().utility_syntax.prepared_statements_from
            && self.peek_is_contextual_keyword("PREPARE")?
        {
            return self.parse_drop_prepare_statement(start);
        }
        // MySQL `DROP {DATABASE | SCHEMA} [IF EXISTS] <name>` — `DATABASE` and `SCHEMA` are
        // exact synonyms naming exactly one unqualified database with no `CASCADE`/`RESTRICT`
        // and no comma list, so it does not route through the shared name-list drop grammar.
        // Intercepted before `parse_drop_object_kind` (whose `SCHEMA` arm is the name-list form
        // other dialects use); off elsewhere, `DATABASE` falls through as an unknown drop kind.
        if self.features().statement_ddl_gates.drop_database {
            if self.eat_contextual_keyword("DATABASE")? {
                return self.parse_drop_database(start, DatabaseKeyword::Database);
            }
            if self.eat_contextual_keyword("SCHEMA")? {
                return self.parse_drop_database(start, DatabaseKeyword::Schema);
            }
        }
        // MySQL `DROP SERVER [IF EXISTS] <name>` — a single-name drop sharing the
        // `server_definition` gate with `CREATE`/`ALTER SERVER`, intercepted before the shared
        // name-list drop grammar. Off elsewhere, `SERVER` falls through as an unknown drop kind.
        if self.features().statement_ddl_gates.server_definition
            && self.eat_contextual_keyword("SERVER")?
        {
            return self.parse_drop_server(start);
        }
        // MySQL `DROP SPATIAL REFERENCE SYSTEM [IF EXISTS] <srid>` — a single-srid drop sharing
        // the `spatial_reference_system` gate with `CREATE`, intercepted before the shared
        // name-list drop grammar (its target is an integer srid, not an object name). Off
        // elsewhere, `SPATIAL` falls through as an unknown drop kind.
        if self.features().statement_ddl_gates.spatial_reference_system
            && self.peek_is_contextual_keyword("SPATIAL")?
        {
            self.expect_contextual_keyword("SPATIAL")?;
            self.expect_contextual_keyword("REFERENCE")?;
            self.expect_contextual_keyword("SYSTEM")?;
            return self.parse_drop_spatial_reference_system(start);
        }
        // MySQL `DROP RESOURCE GROUP <name> [FORCE]` — a single-name drop sharing the
        // `resource_group` gate with `CREATE`/`ALTER RESOURCE GROUP`, intercepted before the
        // shared name-list drop grammar (no `IF EXISTS`, no comma list). Off elsewhere,
        // `RESOURCE` falls through as an unknown drop kind.
        if self.features().statement_ddl_gates.resource_group
            && self.peek_is_contextual_keyword("RESOURCE")?
        {
            self.expect_contextual_keyword("RESOURCE")?;
            self.expect_contextual_keyword("GROUP")?;
            return self.parse_drop_resource_group(start);
        }
        // MySQL `DROP INDEX <name> ON <table> [ALGORITHM …] [LOCK …]` — names the owning table
        // with a mandatory `ON` and carries the online-DDL execution hints, so it does not route
        // through the shared name-list drop grammar (whose `INDEX` arm is the bare-name form
        // other dialects use). Off elsewhere, `INDEX` falls through to that shared arm.
        if self.features().index_alter_syntax.index_drop_on_table
            && self.eat_contextual_keyword("INDEX")?
        {
            return self.parse_drop_index_on_table(start);
        }
        // MySQL storage DDL — `DROP [UNDO] TABLESPACE …` / `DROP LOGFILE GROUP …`. Whole-statement
        // gates, intercepted before the generic `DROP <object> <name-list>` path (whose object-kind
        // set has no `TABLESPACE`/`LOGFILE`). Off elsewhere, the keyword falls through and surfaces
        // as an unknown drop object kind.
        if self.features().statement_ddl_gates.tablespace_ddl {
            if self.eat_contextual_keyword("UNDO")? {
                self.expect_contextual_keyword("TABLESPACE")?;
                return self.parse_drop_tablespace(start, true);
            }
            if self.eat_contextual_keyword("TABLESPACE")? {
                return self.parse_drop_tablespace(start, false);
            }
        }
        if self.features().statement_ddl_gates.logfile_group_ddl
            && self.peek_is_contextual_keyword("LOGFILE")?
            && self.peek_nth_is_contextual_keyword(1, "GROUP")?
        {
            self.advance()?; // LOGFILE
            self.advance()?; // GROUP
            return self.parse_drop_logfile_group(start);
        }
        let object_kind = self.parse_drop_object_kind()?;
        let if_exists = self.parse_schema_change_if_exists()?;
        let names = self.parse_comma_separated(Self::parse_target_relation_name)?;
        let behavior = self.parse_drop_behavior()?;
        let span = start.union(self.preceding_span());
        let drop_meta = self.make_meta(span);
        let drop = DropStatement {
            object_kind,
            if_exists,
            names,
            behavior,
            meta: drop_meta,
        };
        let meta = self.make_meta(span);
        Ok(Statement::Drop {
            drop: Box::new(drop),
            meta,
        })
    }

    /// `TRUNCATE [TABLE] <name> [, ...] [RESTART IDENTITY | CONTINUE IDENTITY]
    /// [CASCADE | RESTRICT]` (SQL:2008 F200; PostgreSQL `TruncateStmt`, gram.y).
    pub(super) fn parse_truncate_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("TRUNCATE")?;
        // The `TABLE` keyword is optional sugar (PostgreSQL's `opt_table`); record
        // whether it was written so a source-fidelity render replays it.
        let table_keyword = self.eat_contextual_keyword("TABLE")?;
        // `ONLY t` / `t *` relation forms are out of scope (not in the parity corpus);
        // a plain object-name list matches the constructs we target. DuckDB also admits
        // a single-part Sconst table name (`TRUNCATE ''`, `TRUNCATE TABLE 't'`;
        // engine-measured on libduckdb 1.5.4).
        let tables = self.parse_comma_separated(Self::parse_truncate_table_name)?;
        let restart_identity = if self.eat_contextual_keyword("RESTART")? {
            self.expect_contextual_keyword("IDENTITY")?;
            Some(true)
        } else if self.eat_contextual_keyword("CONTINUE")? {
            self.expect_contextual_keyword("IDENTITY")?;
            Some(false)
        } else {
            None
        };
        // TRUNCATE's `CASCADE`/`RESTRICT` is part of the statement grammar in every
        // dialect, not the dialect-gated DROP/ALTER tail, so it is parsed unconditionally
        // rather than through the `schema_change_syntax`-gated `parse_drop_behavior`.
        let behavior = if self.eat_contextual_keyword("CASCADE")? {
            Some(DropBehavior::Cascade)
        } else if self.eat_contextual_keyword("RESTRICT")? {
            Some(DropBehavior::Restrict)
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Statement::Truncate {
            tables,
            table_keyword,
            restart_identity,
            behavior,
            meta,
        })
    }

    /// `COMMENT [IF EXISTS] ON <object> IS '<text>' | NULL`.
    fn parse_truncate_table_name(&mut self) -> ParseResult<ObjectName> {
        if let Some(ident) =
            self.try_parse_string_literal_table_name("a table name after TRUNCATE")?
        {
            Ok(ObjectName(thin_vec![ident]))
        } else {
            self.parse_object_name()
        }
    }

    pub(super) fn parse_comment_on_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("COMMENT")?;
        let if_exists =
            self.features().utility_syntax.comment_if_exists && self.eat_keyword(Keyword::If)?;
        if if_exists {
            self.expect_keyword(Keyword::Exists)?;
        }
        self.expect_contextual_keyword("ON")?;
        let (target, name, constraint_table) = if self.eat_contextual_keyword("TABLE")? {
            (CommentTarget::Table, self.parse_object_name()?, None)
        } else if self.eat_contextual_keyword("COLUMN")? {
            (CommentTarget::Column, self.parse_object_name()?, None)
        } else if self.eat_contextual_keyword("DATABASE")? {
            (CommentTarget::Database, self.parse_object_name()?, None)
        } else if self.eat_contextual_keyword("SCHEMA")? {
            (CommentTarget::Schema, self.parse_object_name()?, None)
        } else if self.eat_contextual_keyword("SEQUENCE")? {
            (CommentTarget::Sequence, self.parse_object_name()?, None)
        } else if self.eat_contextual_keyword("VIEW")? {
            (CommentTarget::View, self.parse_object_name()?, None)
        } else if self.eat_contextual_keyword("MATERIALIZED")? {
            self.expect_contextual_keyword("VIEW")?;
            (
                CommentTarget::MaterializedView,
                self.parse_object_name()?,
                None,
            )
        } else if self.eat_contextual_keyword("INDEX")? {
            (CommentTarget::Index, self.parse_object_name()?, None)
        } else if self.eat_contextual_keyword("CONSTRAINT")? {
            let name = self.parse_object_name()?;
            self.expect_contextual_keyword("ON")?;
            let table = self.parse_object_name()?;
            (CommentTarget::Constraint, name, Some(table))
        } else if self.eat_contextual_keyword("PROCEDURE")? {
            let name = self.parse_object_name()?;
            let arg_types = self.parse_optional_routine_arg_types()?;
            (CommentTarget::Procedure { arg_types }, name, None)
        } else {
            return Err(self.unexpected("a supported COMMENT ON object kind"));
        };
        self.expect_keyword(Keyword::Is)?;
        let comment = self.parse_comment_value()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Statement::CommentOn {
            comment: Box::new(CommentOnStatement {
                if_exists,
                target,
                name,
                constraint_table,
                comment,
                meta,
            }),
            meta,
        })
    }

    /// Parse an optional parenthesized routine argument-type list (`(int, text)` /
    /// `()` / absent), mirroring `RoutineSignature::arg_types`: `None` for an
    /// unspecified signature, `Some` — possibly empty — when a list is written.
    fn parse_optional_routine_arg_types(
        &mut self,
    ) -> ParseResult<Option<ThinVec<DataType<D::Ext>>>> {
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok(None);
        }
        let mut types = ThinVec::new();
        if !self.peek_is_punct(Punctuation::RParen)? {
            types.push(self.parse_data_type()?);
            while self.eat_punct(Punctuation::Comma)? {
                types.push(self.parse_data_type()?);
            }
        }
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the procedure argument list",
        )?;
        Ok(Some(types))
    }

    /// Parse a `COMMENT ON ... IS` value: PostgreSQL's `comment_text` (`Sconst | NULL`).
    fn parse_comment_value(&mut self) -> ParseResult<Option<Literal>> {
        if self.eat_keyword(Keyword::Null)? {
            return Ok(None);
        }
        // PostgreSQL's `comment_text` is a bare string constant (`Sconst`): a plain
        // `'...'`, `E'...'`, or dollar-quoted literal. Our tokenizer also folds `N'...'`
        // (national) and `U&'...'` (unicode) into one String token, but PostgreSQL scans
        // those as a leading NCHAR/UESCAPE token plus a string — not a bare `Sconst` —
        // and bit strings (`B'...'`/`X'...'`) are their own constant class, so accept only
        // the plain/escape/dollar forms to keep accept/reject parity.
        if let Some(token) = self.peek()? {
            if token.kind == TokenKind::String {
                let is_sconst = {
                    let bytes = self.span_text(token.span).as_bytes();
                    matches!(bytes.first(), Some(b'\'' | b'$'))
                        || (matches!(bytes.first(), Some(b'e' | b'E'))
                            && bytes.get(1) == Some(&b'\''))
                };
                if !is_sconst {
                    return Err(self.unexpected("a plain string constant or NULL after `IS`"));
                }
            }
        }
        let literal = self.expect_string_literal("a comment string constant or NULL after `IS`")?;
        Ok(Some(literal))
    }

    fn parse_drop_object_kind(&mut self) -> ParseResult<DropObjectKind> {
        if self.eat_contextual_keyword("TABLE")? {
            Ok(DropObjectKind::Table)
        } else if self.features().statement_ddl_gates.materialized_views
            && self.eat_contextual_keyword("MATERIALIZED")?
        {
            self.expect_contextual_keyword("VIEW")?;
            Ok(DropObjectKind::MaterializedView)
        } else if self.eat_contextual_keyword("VIEW")? {
            Ok(DropObjectKind::View)
        } else if self.eat_contextual_keyword("INDEX")? {
            Ok(DropObjectKind::Index)
        } else if self.features().statement_ddl_gates.schemas
            && self.eat_contextual_keyword("SCHEMA")?
        {
            Ok(DropObjectKind::Schema)
        } else if self.features().statement_ddl_gates.create_type
            && self.eat_contextual_keyword("TYPE")?
        {
            Ok(DropObjectKind::Type)
        } else if self.features().statement_ddl_gates.create_sequence
            && self.eat_contextual_keyword("SEQUENCE")?
        {
            Ok(DropObjectKind::Sequence)
        } else if self.features().statement_ddl_gates.create_macro
            && self.eat_contextual_keyword("MACRO")?
        {
            // DuckDB spells a table-macro drop `DROP MACRO TABLE <name>` (only in this order —
            // `TABLE MACRO`/`FUNCTION TABLE` are syntax errors, engine-measured); the optional
            // `TABLE` selects the table-macro namespace and must round-trip verbatim.
            if self.eat_contextual_keyword("TABLE")? {
                Ok(DropObjectKind::MacroTable)
            } else {
                Ok(DropObjectKind::Macro)
            }
        } else if (self.features().statement_ddl_gates.create_trigger
            || self.features().statement_ddl_gates.compound_statements)
            && self.eat_contextual_keyword("TRIGGER")?
        {
            // `DROP TRIGGER [IF EXISTS] [<schema>.]<name>` — the name-only form both the SQLite
            // (`create_trigger`) and MySQL (`compound_statements`) trigger dialects share.
            // PostgreSQL's `DROP TRIGGER … ON <table>` shape is a separate concern (neither flag
            // is on there).
            Ok(DropObjectKind::Trigger)
        } else {
            Err(self.unexpected(
                "`TABLE`, `VIEW`, `MATERIALIZED VIEW`, `INDEX`, `SCHEMA`, `TYPE`, `SEQUENCE`, `MACRO`, or `TRIGGER`",
            ))
        }
    }

    /// Parse a `DROP {FUNCTION | PROCEDURE | ROUTINE}` after the routine keyword. The
    /// signature list reuses the DCL routine-reference grammar (name + optional
    /// argument-type list), the same shape a `GRANT ON FUNCTION` names.
    fn parse_drop_routine(
        &mut self,
        start: Span,
        kind: RoutineObjectKind,
    ) -> ParseResult<Statement<D::Ext>> {
        let if_exists = self.parse_schema_change_if_exists()?;
        let routines = self.parse_routine_signature_list()?;
        let behavior = self.parse_drop_behavior()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::DropRoutine {
            kind,
            if_exists,
            routines,
            behavior,
            meta: self.make_meta(span),
        })
    }

    /// Parse `DROP TRANSFORM [IF EXISTS] FOR <type> LANGUAGE <lang> [CASCADE | RESTRICT]`
    /// after the leading `DROP` (PostgreSQL `DropTransformStmt`, gram.y).
    ///
    /// The `IF EXISTS` guard sits between the `TRANSFORM` keyword and `FOR` — PostgreSQL
    /// rejects it in any other position — and PostgreSQL admits exactly one transform per
    /// statement (no comma list). The `FOR <type> LANGUAGE <lang>` body is captured as an
    /// [`ObjectReference::Transform`], the same shape `ALTER EXTENSION … ADD|DROP TRANSFORM`
    /// names a member with; the language name is a `ColId` ([`parse_ident`](Self::parse_ident)),
    /// so a bare reserved word (`for`, `default`) is a reject but a quoted `"default"` binds.
    fn parse_drop_transform(&mut self, start: Span) -> ParseResult<Statement<D::Ext>> {
        let object_start = self.current_span()?;
        self.expect_contextual_keyword("TRANSFORM")?;
        let if_exists = self.parse_schema_change_if_exists()?;
        self.expect_contextual_keyword("FOR")?;
        let type_name = self.parse_data_type()?;
        self.expect_contextual_keyword("LANGUAGE")?;
        let language = self.parse_ident()?;
        let object = ObjectReference::Transform {
            type_name,
            language,
            meta: self.make_meta(object_start.union(self.preceding_span())),
        };
        let behavior = self.parse_drop_behavior()?;
        let span = start.union(self.preceding_span());
        Ok(Statement::DropTransform {
            drop: Box::new(DropTransform {
                object,
                if_exists,
                behavior,
                meta: self.make_meta(span),
            }),
            meta: self.make_meta(span),
        })
    }

    /// Parse a trailing `CASCADE` / `RESTRICT` drop behaviour, gated by dialect data.
    ///
    /// When the dialect does not model drop behaviour the keyword is left unconsumed
    /// and surfaces as leftover input — the reject side of the acceptance criteria.
    fn parse_drop_behavior(&mut self) -> ParseResult<Option<DropBehavior>> {
        if !self.features().index_alter_syntax.drop_behavior {
            return Ok(None);
        }
        if self.eat_contextual_keyword("CASCADE")? {
            Ok(Some(DropBehavior::Cascade))
        } else if self.eat_contextual_keyword("RESTRICT")? {
            Ok(Some(DropBehavior::Restrict))
        } else {
            Ok(None)
        }
    }

    /// Parse an `IF EXISTS` existence guard, gated by `existence_guards.if_exists`.
    ///
    /// Off under ANSI: the `IF` is left unconsumed so the guard surfaces as a parse
    /// error, mirroring how the mutation extensions reject under ANSI.
    fn parse_schema_change_if_exists(&mut self) -> ParseResult<bool> {
        if !self.features().existence_guards.if_exists {
            return Ok(false);
        }
        if !self.eat_contextual_keyword("IF")? {
            return Ok(false);
        }
        self.expect_keyword(Keyword::Exists)?;
        Ok(true)
    }

    /// Parse an `IF NOT EXISTS` guard on `ADD COLUMN`, gated like `IF EXISTS`.
    fn parse_schema_change_if_not_exists(&mut self) -> ParseResult<bool> {
        if !self.features().existence_guards.if_exists {
            return Ok(false);
        }
        if !self.eat_contextual_keyword("IF")? {
            return Ok(false);
        }
        self.expect_keyword(Keyword::Not)?;
        self.expect_keyword(Keyword::Exists)?;
        Ok(true)
    }

    /// Parse an optional `TEMP`/`TEMPORARY` marker. Shared with the SELECT `INTO`
    /// create-table target ([`parse_select_into`](Self::parse_select_into)), which
    /// reuses the same `TEMP`/`TEMPORARY` spelling.
    pub(super) fn parse_temporary_table_kind(&mut self) -> ParseResult<Option<TemporaryTableKind>> {
        if self.eat_contextual_keyword("TEMP")? {
            Ok(Some(TemporaryTableKind::Temp))
        } else if self.eat_contextual_keyword("TEMPORARY")? {
            Ok(Some(TemporaryTableKind::Temporary))
        } else {
            Ok(None)
        }
    }

    fn parse_if_not_exists(&mut self) -> ParseResult<bool> {
        if !self.eat_contextual_keyword("IF")? {
            return Ok(false);
        }
        self.expect_keyword(Keyword::Not)?;
        self.expect_keyword(Keyword::Exists)?;
        Ok(true)
    }

    /// Parse the `CREATE TABLE` tail after the name: the body, an optional trailing
    /// `PARTITION BY` spec, and the option list.
    ///
    /// Three body shapes branch here. A `PARTITION OF` lead (PostgreSQL declarative
    /// partitioning, gated) is the child form; otherwise a `CTAS`/`(elements)` body is parsed and
    /// — for the `(elements)` definition — a trailing `PARTITION BY` may mark it a partitioned
    /// parent. `CTAS` never takes a `PARTITION BY` (PostgreSQL has no partitioned CTAS), so it
    /// returns `None` for the spec.
    fn parse_create_table_tail(&mut self) -> ParseResult<CreateTableTail<D::Ext>> {
        // A `PARTITION OF` immediately after the name is the declarative-partitioning child
        // body (the parent's own `PARTITION BY` trails its `(elements)` body instead, handled
        // below). Gated: off-dialect, `PARTITION` is left for the CTAS/`(` body parse, where it
        // surfaces as a clean parse error.
        if self
            .features()
            .create_table_clause_syntax
            .declarative_partitioning
            && self.peek_is_contextual_keyword("PARTITION")?
        {
            let start = self.current_span()?;
            self.expect_contextual_keyword("PARTITION")?;
            self.expect_contextual_keyword("OF")?;
            let parent = self.parse_target_relation_name()?;
            let elements = self.parse_partition_of_augmentations()?;
            let bound = self.parse_partition_bound()?;
            let body_meta = self.make_meta(start.union(self.preceding_span()));
            // A `PARTITION OF` child takes no `INHERITS` clause (PostgreSQL's grammar reserves
            // `OptInherit` for the plain `(elements)` body), so a trailing `INHERITS` is left as
            // unconsumed input and surfaces as a parse error — never parsed here.
            let partition_by = self.parse_optional_partition_spec()?;
            let access_method = self.parse_optional_access_method()?;
            let options = self.parse_create_table_options()?;
            return Ok((
                CreateTableBody::PartitionOf {
                    parent,
                    elements,
                    bound: Box::new(bound),
                    meta: body_meta,
                },
                ThinVec::new(),
                partition_by,
                access_method,
                options,
            ));
        }

        // `OF <type>` — a typed table (gated). Off-dialect, `OF` after the name is left for the
        // CTAS/`(` body parse, where it surfaces as a clean parse error. The `OF` form takes no
        // `INHERITS` clause (PostgreSQL's grammar reserves `OptInherit` for the `(elements)`
        // body), so its inherits list is always empty.
        if self.features().create_table_clause_syntax.typed_tables
            && self.peek_is_contextual_keyword("OF")?
        {
            let start = self.current_span()?;
            self.expect_contextual_keyword("OF")?;
            let type_name = self.parse_object_name()?;
            // The typed-table augmentation body is the same typeless-column-and-constraint list
            // as a `PARTITION OF` child (PostgreSQL's `OptTypedTableElementList`), so it reuses
            // the same parser; an empty `()` is a raw-parse error there too.
            let elements = self.parse_partition_of_augmentations()?;
            let body_meta = self.make_meta(start.union(self.preceding_span()));
            let partition_by = self.parse_optional_partition_spec()?;
            let access_method = self.parse_optional_access_method()?;
            let options = self.parse_create_table_options()?;
            return Ok((
                CreateTableBody::OfType {
                    type_name,
                    elements,
                    meta: body_meta,
                },
                ThinVec::new(),
                partition_by,
                access_method,
                options,
            ));
        }

        // `LIKE <source>` — MySQL's statement-level table-clone body (gated). The parenthesized
        // twin `(LIKE <source>)` is handled inside the `(` body below; off-dialect the `LIKE`
        // keyword is left for the `(` expect, where it surfaces as a clean parse error (or, under
        // SQLite's permissive identifier rules, is read as a keyword-named column — the flag stays
        // off there to keep that behaviour).
        if self
            .features()
            .create_table_clause_syntax
            .statement_level_table_like
            && self.peek_is_contextual_keyword("LIKE")?
        {
            let body = self.parse_statement_level_table_like(None)?;
            return Ok((body, ThinVec::new(), None, None, ThinVec::new()));
        }

        let checkpoint = self.checkpoint();
        if let Some((body, access_method, options)) = self.try_parse_create_table_as()? {
            // A `CREATE TABLE … AS <query>` is never a partition parent nor an inheritance
            // child. Its `USING <access_method>` slot sits *before* `AS` (between the column
            // list and the `WITH (…)` options — PostgreSQL's `CreateTableAsStmt`), parsed
            // inside the CTAS attempt; a `USING` *after* the query is a raw-parse error left
            // as leftover input.
            return Ok((body, ThinVec::new(), None, access_method, options));
        }
        self.rewind(checkpoint);

        let start = self.current_span()?;
        self.expect_punct(Punctuation::LParen, "`(` to open the table definition")?;
        // MySQL's parenthesized statement-level clone `(LIKE <source>)`. Taken only when the
        // PostgreSQL copy *element* is off (MySQL); when both are on (Lenient) the element wins as
        // the more general form (`(LIKE src INCLUDING …)`, multi-element), so this arm is skipped
        // and the `LIKE` is consumed by `parse_table_element` below. MySQL admits exactly one
        // `LIKE <source>` here and nothing else — the closing `)` expect rejects `(LIKE t, x INT)`
        // and `(LIKE t, LIKE u)` as parse errors, matching mysql:8.4.
        if self
            .features()
            .create_table_clause_syntax
            .statement_level_table_like
            && !self.features().create_table_clause_syntax.like_source_table
            && self.peek_is_contextual_keyword("LIKE")?
        {
            let body = self.parse_statement_level_table_like(Some(start))?;
            return Ok((body, ThinVec::new(), None, None, ThinVec::new()));
        }
        let mut elements = ThinVec::new();
        if !self.peek_is_punct(Punctuation::RParen)? {
            elements.push(self.parse_table_element()?);
            loop {
                if self.eat_punct(Punctuation::Comma)? {
                    // DuckDB tolerates a single trailing comma before the closing `)` of the
                    // table-element list (`CREATE TABLE t (a INT, b INT,)`), a column or a
                    // constraint element alike; off-dialect the dangling comma falls through to
                    // `parse_table_element` and yields the standard reject.
                    if self.trailing_comma_at(Punctuation::RParen)? {
                        break;
                    }
                    elements.push(self.parse_table_element()?);
                    continue;
                }
                // SQLite lets the comma before a trailing bare `CONSTRAINT <name>` be omitted
                // when it follows another table constraint (`UNIQUE(a) CONSTRAINT c`,
                // `CONSTRAINT a UNIQUE(x) CONSTRAINT b`, both engine-measured accepted) — but
                // only a bare name, and only right after a constraint element (a column can
                // never follow a table constraint at all, comma or not, so this never needs to
                // apply after a `TableElement::Column`).
                if matches!(elements.last(), Some(TableElement::Constraint { .. }))
                    && self.peek_bare_trailing_table_constraint()?
                {
                    elements.push(self.parse_table_element()?);
                    continue;
                }
                break;
            }
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the table definition")?;
        let body_span = start.union(self.preceding_span());
        let body_meta = self.make_meta(body_span);
        // PostgreSQL grammar order after the `(elements)` body: `INHERITS (…)`, then
        // `PARTITION BY …`, then `USING <method>`, then the options. INHERITS before PARTITION BY
        // is load-bearing — PostgreSQL rejects the reverse order, so parsing INHERITS first
        // reproduces that; `USING` likewise precedes the trailing `WITH (…)` options.
        let inherits = self.parse_optional_inherits()?;
        let partition_by = self.parse_optional_partition_spec()?;
        let access_method = self.parse_optional_access_method()?;
        let options = self.parse_create_table_options()?;
        Ok((
            CreateTableBody::Definition {
                elements,
                meta: body_meta,
            },
            inherits,
            partition_by,
            access_method,
            options,
        ))
    }

    /// Parse MySQL's statement-level `LIKE <source>` table-clone body, with `LIKE` confirmed at
    /// the cursor. `paren_start` is `Some(span)` for the parenthesized `(LIKE <source>)` spelling
    /// — the caller has already consumed the `(`, and this consumes the matching `)`, so the body
    /// meta spans the whole `(…)`; `None` for the bare `LIKE <source>` form, whose meta spans from
    /// `LIKE`. The source is a single (qualified) relation name; MySQL admits no `{INCLUDING |
    /// EXCLUDING}` options nor any co-element, so nothing else is consumed here (the caller's
    /// trailing expectations reject `LIKE src ENGINE=…` / `(LIKE src, x INT)`).
    fn parse_statement_level_table_like(
        &mut self,
        paren_start: Option<Span>,
    ) -> ParseResult<CreateTableBody<D::Ext>> {
        let start = match paren_start {
            Some(span) => span,
            None => self.current_span()?,
        };
        self.expect_contextual_keyword("LIKE")?;
        let source = self.parse_target_relation_name()?;
        let parenthesized = paren_start.is_some();
        if parenthesized {
            self.expect_punct(
                Punctuation::RParen,
                "`)` to close the `(LIKE <source>)` clone (MySQL admits no other element here)",
            )?;
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CreateTableBody::LikeSource {
            source,
            parenthesized,
            meta,
        })
    }

    /// Parse an optional trailing `USING <access_method>` clause, when the table access-method
    /// feature is on and a `USING` keyword is at the cursor. `None` otherwise — off-dialect the
    /// `USING` keyword is left as leftover input and surfaces as a clean parse error. The method
    /// is a single (optionally quoted) identifier; PostgreSQL rejects a qualified `schema.method`,
    /// which fails here when the trailing `.` is left for the caller.
    fn parse_optional_access_method(&mut self) -> ParseResult<Option<Box<Ident>>> {
        if !self
            .features()
            .create_table_clause_syntax
            .table_access_method
            || !self.peek_is_contextual_keyword("USING")?
        {
            return Ok(None);
        }
        self.expect_contextual_keyword("USING")?;
        Ok(Some(Box::new(self.parse_ident()?)))
    }

    /// Parse an optional `INHERITS (<parent>, ...)` legacy table-inheritance clause, when table
    /// inheritance is on and an `INHERITS` keyword is at the cursor. Returns an empty list
    /// otherwise — off-dialect the `INHERITS` keyword is left as leftover input and surfaces as a
    /// clean parse error. A non-empty parent list is required (PostgreSQL rejects `INHERITS ()`),
    /// so an empty `()` is a parse error here too.
    fn parse_optional_inherits(&mut self) -> ParseResult<ThinVec<ObjectName>> {
        let mut parents = ThinVec::new();
        if !self.features().create_table_clause_syntax.table_inheritance
            || !self.peek_is_contextual_keyword("INHERITS")?
        {
            return Ok(parents);
        }
        self.expect_contextual_keyword("INHERITS")?;
        self.expect_punct(Punctuation::LParen, "`(` to open the INHERITS parent list")?;
        parents.push(self.parse_target_relation_name()?);
        while self.eat_punct(Punctuation::Comma)? {
            parents.push(self.parse_target_relation_name()?);
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the INHERITS parent list")?;
        Ok(parents)
    }

    /// Parse an optional trailing `PARTITION BY {LIST | RANGE | HASH} (<key>, ...)` spec, when
    /// declarative partitioning is on and a `PARTITION` keyword is at the cursor. `None`
    /// otherwise — off-dialect the `PARTITION` keyword is left as leftover input and surfaces as
    /// a clean parse error.
    fn parse_optional_partition_spec(&mut self) -> ParseResult<Option<Box<PartitionSpec<D::Ext>>>> {
        if !self
            .features()
            .create_table_clause_syntax
            .declarative_partitioning
            || !self.peek_is_contextual_keyword("PARTITION")?
        {
            return Ok(None);
        }
        let start = self.current_span()?;
        self.expect_contextual_keyword("PARTITION")?;
        self.expect_contextual_keyword("BY")?;
        let strategy = self.parse_partition_strategy()?;
        self.expect_punct(Punctuation::LParen, "`(` to open the partition key list")?;
        let mut columns = ThinVec::new();
        columns.push(self.parse_partition_elem()?);
        while self.eat_punct(Punctuation::Comma)? {
            columns.push(self.parse_partition_elem()?);
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the partition key list")?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(Box::new(PartitionSpec {
            strategy,
            columns,
            meta,
        })))
    }

    /// Parse the partition strategy word. PostgreSQL 17 validates it in the grammar action, so an
    /// unrecognized word (`PARTITION BY foo`) is a parse error here — only `LIST` / `RANGE` /
    /// `HASH` are admitted.
    fn parse_partition_strategy(&mut self) -> ParseResult<PartitionStrategy> {
        if self.eat_contextual_keyword("LIST")? {
            Ok(PartitionStrategy::List)
        } else if self.eat_contextual_keyword("RANGE")? {
            Ok(PartitionStrategy::Range)
        } else if self.eat_contextual_keyword("HASH")? {
            Ok(PartitionStrategy::Hash)
        } else {
            Err(self.unexpected("a partition strategy: `LIST`, `RANGE`, or `HASH`"))
        }
    }

    /// Parse one partition-key element: `<key> [COLLATE <collation>] [<opclass>]`, where `<key>`
    /// is a bare column, a bare function call, or a parenthesized expression.
    fn parse_partition_elem(&mut self) -> ParseResult<PartitionElem<D::Ext>> {
        let start = self.current_span()?;
        let (expr, parenthesized) = self.parse_partition_key_head()?;
        // A non-parenthesized key must be PostgreSQL's `ColId` (a bare column) or
        // `func_expr_windowless` (a bare function call); a bare literal / operator expression
        // (`RANGE (5)`, `RANGE (-a)`) is a parse error — only the parenthesized `(a_expr)` form
        // admits an arbitrary expression.
        if !parenthesized
            && !matches!(
                expr,
                Expr::Column { .. } | Expr::Function { .. } | Expr::SpecialFunction { .. }
            )
        {
            let span = start.union(self.preceding_span());
            return Err(self.error_at(
                span,
                "a bare column, a function call, or a parenthesized expression as the partition key",
                self.span_text(span).to_owned(),
            ));
        }
        let collation = if self.eat_contextual_keyword("COLLATE")? {
            Some(self.parse_object_name()?)
        } else {
            None
        };
        // The operator-class name is an optional trailing qualified name, present only when a
        // token other than the element terminator (`,` / `)`) follows. A stray `::` / `[` after
        // the key (which PostgreSQL rejects) then fails inside `parse_object_name`, matching the
        // engine's parse error.
        let opclass = if self.peek_is_punct(Punctuation::Comma)?
            || self.peek_is_punct(Punctuation::RParen)?
        {
            None
        } else {
            Some(self.parse_object_name()?)
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(PartitionElem {
            expr,
            parenthesized,
            collation,
            opclass,
            meta,
        })
    }

    /// Parse the optional parenthesized augmentation list of a `PARTITION OF` child body:
    /// per-column overrides (a bare `ColId` with a `[WITH OPTIONS]` constraint list — *no* type)
    /// and table constraints. Empty when the `(...)` is absent; PostgreSQL rejects an empty `()`,
    /// so a present-but-empty list is a parse error here.
    fn parse_partition_of_augmentations(&mut self) -> ParseResult<ThinVec<TableElement<D::Ext>>> {
        let mut elements = ThinVec::new();
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok(elements);
        }
        elements.push(self.parse_partition_of_element()?);
        while self.eat_punct(Punctuation::Comma)? {
            elements.push(self.parse_partition_of_element()?);
        }
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the partition augmentation list",
        )?;
        Ok(elements)
    }

    /// Parse one `PARTITION OF` augmentation element: a table constraint, or a *typeless* column
    /// override — a column name, an optional `WITH OPTIONS` noise phrase, then its constraint
    /// list (PostgreSQL `columnOptions`: the child inherits the type from the parent, so no type
    /// is written).
    fn parse_partition_of_element(&mut self) -> ParseResult<TableElement<D::Ext>> {
        if self.peek_is_contextual_keyword("CONSTRAINT")? || self.peek_starts_table_constraint()? {
            let constraint = self.parse_table_constraint_def()?;
            let meta = self.make_meta(constraint.span());
            return Ok(TableElement::Constraint { constraint, meta });
        }
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        // `WITH OPTIONS` is an optional noise phrase between the column name and its
        // constraints; PostgreSQL does not preserve it (a bare `b NOT NULL` and `b WITH OPTIONS
        // NOT NULL` share one parse tree), so it is consumed and dropped.
        if self.eat_contextual_keyword("WITH")? {
            self.expect_contextual_keyword("OPTIONS")?;
        }
        let mut constraints = ThinVec::new();
        while !self.peek_ends_table_element()? {
            match self.parse_column_constraint()? {
                Some(constraint) => constraints.push(constraint),
                None => return Err(self.unexpected("a column constraint")),
            }
        }
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableElement::Column {
            column: ColumnDef {
                name,
                data_type: None,
                storage: None,
                compression: None,
                constraints,
                meta,
            },
            meta,
        })
    }

    /// Parse a partition-bound spec: `FOR VALUES {IN | FROM…TO | WITH} (…)` or `DEFAULT`. Shared
    /// by the `PARTITION OF` child body and `ALTER TABLE … ATTACH PARTITION`.
    fn parse_partition_bound(&mut self) -> ParseResult<PartitionBound<D::Ext>> {
        let start = self.current_span()?;
        if self.eat_contextual_keyword("DEFAULT")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(PartitionBound::Default { meta });
        }
        self.expect_contextual_keyword("FOR")?;
        self.expect_contextual_keyword("VALUES")?;
        if self.eat_contextual_keyword("IN")? {
            let values = self.parse_parenthesized_partition_datums()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PartitionBound::List { values, meta })
        } else if self.eat_contextual_keyword("FROM")? {
            let from = self.parse_parenthesized_partition_datums()?;
            self.expect_contextual_keyword("TO")?;
            let to = self.parse_parenthesized_partition_datums()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PartitionBound::Range { from, to, meta })
        } else if self.eat_contextual_keyword("WITH")? {
            let (modulus, remainder) = self.parse_hash_partition_bound()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PartitionBound::Hash {
                modulus,
                remainder,
                meta,
            })
        } else {
            Err(self.unexpected("`IN`, `FROM`, or `WITH` after `FOR VALUES`"))
        }
    }

    /// Parse a parenthesized, non-empty partition-bound datum list `(<expr>, ...)`. The datums
    /// are full expressions (the `minvalue`/`maxvalue` range sentinels parse as ordinary column
    /// references); PostgreSQL rejects an empty `()`, so ≥ 1 datum is required.
    fn parse_parenthesized_partition_datums(&mut self) -> ParseResult<ThinVec<Expr<D::Ext>>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the partition bound")?;
        let mut datums = ThinVec::new();
        datums.push(self.parse_expr()?);
        while self.eat_punct(Punctuation::Comma)? {
            datums.push(self.parse_expr()?);
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the partition bound")?;
        Ok(datums)
    }

    /// Parse the `(MODULUS <n>, REMAINDER <n>)` body of a hash partition bound. PostgreSQL's
    /// grammar action requires exactly one `MODULUS` and one `REMAINDER` — either order, each an
    /// unsigned integer literal — and rejects a missing, duplicated, or non-integer value; those
    /// rejects are reproduced here at the same raw-parse layer.
    fn parse_hash_partition_bound(&mut self) -> ParseResult<(Literal, Literal)> {
        self.expect_punct(Punctuation::LParen, "`(` to open the hash partition bound")?;
        let mut modulus = None;
        let mut remainder = None;
        loop {
            if self.eat_contextual_keyword("MODULUS")? {
                if modulus.is_some() {
                    return Err(self.unexpected("a single `MODULUS` in the hash partition bound"));
                }
                modulus = Some(self.expect_unsigned_integer_literal("MODULUS")?);
            } else if self.eat_contextual_keyword("REMAINDER")? {
                if remainder.is_some() {
                    return Err(self.unexpected("a single `REMAINDER` in the hash partition bound"));
                }
                remainder = Some(self.expect_unsigned_integer_literal("REMAINDER")?);
            } else {
                return Err(self.unexpected("`MODULUS` or `REMAINDER` in the hash partition bound"));
            }
            if !self.eat_punct(Punctuation::Comma)? {
                break;
            }
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the hash partition bound")?;
        match (modulus, remainder) {
            (Some(modulus), Some(remainder)) => Ok((modulus, remainder)),
            (None, _) => Err(self.unexpected("a `MODULUS` in the hash partition bound")),
            (_, None) => Err(self.unexpected("a `REMAINDER` in the hash partition bound")),
        }
    }

    /// Consume an unsigned integer literal (`MODULUS 4`), rejecting a non-integer or signed value
    /// — matching PostgreSQL's `Iconst` grammar (`MODULUS a` / `MODULUS -4` are parse errors). The
    /// literal rides its `meta.span` and materialises lazily (ADR-0006).
    pub(super) fn expect_unsigned_integer_literal(
        &mut self,
        context: &str,
    ) -> ParseResult<Literal> {
        if let Some(token) = self.peek()? {
            if token.kind == TokenKind::Number
                && number_literal_kind(self.span_text(token.span), self.float_as_decimal_enabled())
                    == LiteralKind::Integer
            {
                self.advance()?;
                return Ok(Literal {
                    kind: LiteralKind::Integer,
                    meta: self.make_meta(token.span),
                });
            }
        }
        Err(self.unexpected(format!("an integer literal after `{context}`")))
    }

    /// Consume a *decimal* unsigned integer literal — a base-10 `NUM`, rejecting the radix
    /// (`0x`/`0o`/`0b`) spellings [`expect_unsigned_integer_literal`](Self::expect_unsigned_integer_literal)
    /// admits. MySQL grammar slots typed `NUM` (resource-group `VCPU` bounds, `THREAD_PRIORITY`)
    /// reject a `HEX_NUM` where the `real_ulonglong_num` slots (an SRS srid) accept one; this
    /// helper holds those slots to the measured boundary.
    fn expect_decimal_integer_literal(&mut self, context: &str) -> ParseResult<Literal> {
        if let Some(token) = self.peek()? {
            let text = self.span_text(token.span);
            if token.kind == TokenKind::Number
                && number_literal_kind(text, self.float_as_decimal_enabled())
                    == LiteralKind::Integer
                && crate::ast::split_radix_prefix(text).0 == 10
            {
                self.advance()?;
                return Ok(Literal {
                    kind: LiteralKind::Integer,
                    meta: self.make_meta(token.span),
                });
            }
        }
        Err(self.unexpected(format!("a decimal integer literal after `{context}`")))
    }

    fn try_parse_create_table_as(
        &mut self,
    ) -> ParseResult<Option<CreateTableBodyAndOptions<D::Ext>>> {
        let checkpoint = self.checkpoint();
        let mut body_start = None;
        let columns = if self.peek_is_punct(Punctuation::LParen)? {
            body_start = Some(self.current_span()?);
            match self.parse_parenthesized_ident_list(
                "`(` to open the CTAS column list",
                "`)` to close the CTAS column list",
            ) {
                Ok(columns) => columns,
                Err(_) => {
                    self.rewind(checkpoint);
                    return Ok(None);
                }
            }
        } else {
            ThinVec::new()
        };

        // PostgreSQL's `CreateTableAsStmt` places the `USING <access_method>` slot between the
        // column list and the `WITH (…)` options (`CREATE TABLE t [(cols)] USING m [WITH (…)]
        // AS query` — the reverse `WITH (…) USING` order is a raw-parse error, like the
        // `CreateStmt` tail's).
        let access_method = self.parse_optional_access_method()?;
        let options = self.parse_create_table_options()?;
        let as_span = self.current_span()?;
        if !self.eat_keyword(Keyword::As)? {
            self.rewind(checkpoint);
            return Ok(None);
        }

        // `AS EXECUTE <prepared> [(args)]` is a distinct CTAS source (PostgreSQL's
        // `CreateTableAsStmt` over an `ExecuteStmt`), gated by `create_table_as_execute`; off
        // that gate the `EXECUTE` keyword is left for the inline-query path, which rejects it.
        if self
            .features()
            .create_table_clause_syntax
            .create_table_as_execute
            && self.peek_is_contextual_keyword("EXECUTE")?
        {
            let execute_start = self.current_span()?;
            self.expect_contextual_keyword("EXECUTE")?;
            let execute = self.parse_execute_statement_body(execute_start)?;
            let with_data = self.parse_with_data()?;
            let body_span = body_start.unwrap_or(as_span).union(self.preceding_span());
            let meta = self.make_meta(body_span);
            return Ok(Some((
                CreateTableBody::AsExecute {
                    columns,
                    execute: Box::new(execute),
                    with_data,
                    meta,
                },
                access_method,
                options,
            )));
        }

        let query = self.parse_query()?;
        let with_data = self.parse_with_data()?;
        let body_span = body_start.unwrap_or(as_span).union(self.preceding_span());
        let meta = self.make_meta(body_span);
        Ok(Some((
            CreateTableBody::AsQuery {
                columns,
                query: Box::new(query),
                with_data,
                meta,
            },
            access_method,
            options,
        )))
    }

    /// Parse a trailing `WITH [NO] DATA` populate clause, shared by `CREATE TABLE AS`
    /// and `CREATE MATERIALIZED VIEW`. `None` when no `WITH DATA` clause is present.
    fn parse_with_data(&mut self) -> ParseResult<Option<bool>> {
        // MySQL's `CREATE TABLE … AS SELECT` has no `WITH [NO] DATA` populate clause: with
        // this off the `WITH` keyword is left as leftover input and rejected.
        if !self
            .features()
            .create_table_clause_syntax
            .create_table_as_with_data
        {
            return Ok(None);
        }
        if !self.eat_keyword(Keyword::With)? {
            return Ok(None);
        }
        let with_data = if self.eat_keyword(Keyword::No)? {
            self.expect_contextual_keyword("DATA")?;
            false
        } else {
            self.expect_contextual_keyword("DATA")?;
            true
        };
        Ok(Some(with_data))
    }

    fn parse_table_element(&mut self) -> ParseResult<TableElement<D::Ext>> {
        // A `LIKE <source>` at an element position is PostgreSQL's source-table copy element
        // (gated). Off-dialect the `LIKE` keyword — reserved, so never a bare column name —
        // falls through to `parse_column_def` and surfaces as a clean parse error.
        if self.features().create_table_clause_syntax.like_source_table
            && self.peek_is_contextual_keyword("LIKE")?
        {
            return self.parse_table_like_element();
        }
        if self.peek_is_contextual_keyword("CONSTRAINT")? || self.peek_starts_table_constraint()? {
            let constraint = self.parse_table_constraint_def()?;
            let meta = self.make_meta(constraint.span());
            return Ok(TableElement::Constraint { constraint, meta });
        }
        if let Some(constraint) = self.parse_unnamed_table_constraint_hook()? {
            let meta = self.make_meta(constraint.span());
            return Ok(TableElement::Constraint { constraint, meta });
        }
        let column = self.parse_column_def()?;
        let meta = self.make_meta(column.span());
        Ok(TableElement::Column { column, meta })
    }

    /// Parse a `LIKE <source> [{INCLUDING | EXCLUDING} <feature> ...]` source-table copy element.
    /// Reached only under [`CreateTableClauseSyntax::like_source_table`](crate::ast::dialect::CreateTableClauseSyntax), with `LIKE` confirmed at the
    /// cursor. The `{INCLUDING | EXCLUDING} <feature>` options are repeatable and order-free
    /// (PostgreSQL's `TableLikeOptionList`), preserved as written.
    fn parse_table_like_element(&mut self) -> ParseResult<TableElement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("LIKE")?;
        let source = self.parse_target_relation_name()?;
        let mut options = ThinVec::new();
        loop {
            let opt_start = self.current_span()?;
            let action = if self.eat_contextual_keyword("INCLUDING")? {
                TableLikeAction::Including
            } else if self.eat_contextual_keyword("EXCLUDING")? {
                TableLikeAction::Excluding
            } else {
                break;
            };
            let feature = self.parse_table_like_feature()?;
            let opt_meta = self.make_meta(opt_start.union(self.preceding_span()));
            options.push(TableLikeOption {
                action,
                feature,
                meta: opt_meta,
            });
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(TableElement::Like {
            source,
            options,
            meta,
        })
    }

    /// Parse a single `LIKE` feature keyword after `INCLUDING` / `EXCLUDING`. PostgreSQL admits
    /// exactly the closed set below (plus `ALL`); any other word is a parse error, matching the
    /// engine's grammar-action rejection.
    fn parse_table_like_feature(&mut self) -> ParseResult<TableLikeFeature> {
        const FEATURES: &[(&str, TableLikeFeature)] = &[
            ("COMMENTS", TableLikeFeature::Comments),
            ("COMPRESSION", TableLikeFeature::Compression),
            ("CONSTRAINTS", TableLikeFeature::Constraints),
            ("DEFAULTS", TableLikeFeature::Defaults),
            ("GENERATED", TableLikeFeature::Generated),
            ("IDENTITY", TableLikeFeature::Identity),
            ("INDEXES", TableLikeFeature::Indexes),
            ("STATISTICS", TableLikeFeature::Statistics),
            ("STORAGE", TableLikeFeature::Storage),
            ("ALL", TableLikeFeature::All),
        ];
        for &(word, feature) in FEATURES {
            if self.eat_contextual_keyword(word)? {
                return Ok(feature);
            }
        }
        Err(self.unexpected(
            "a LIKE option: COMMENTS, COMPRESSION, CONSTRAINTS, DEFAULTS, GENERATED, IDENTITY, \
             INDEXES, STATISTICS, STORAGE, or ALL",
        ))
    }

    fn parse_column_def(&mut self) -> ParseResult<ColumnDef<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        self.parse_column_def_tail(start, name)
    }

    fn parse_column_def_tail(
        &mut self,
        start: Span,
        name: Ident,
    ) -> ParseResult<ColumnDef<D::Ext>> {
        // A column may omit its type under two dialect rules (see
        // [`column_definition_omits_type`](Self::column_definition_omits_type)); off-dialect the
        // missing type falls through to `parse_data_type`, which reports a clean parse error.
        let data_type = if self.column_definition_omits_type()? {
            None
        } else {
            Some(self.parse_data_type()?)
        };
        // PostgreSQL's `columnDef` places the `STORAGE` and `COMPRESSION` physical-storage
        // attributes between the type and the constraint list, `STORAGE` before `COMPRESSION`.
        // Parsing them here (not in the constraint loop) reproduces PostgreSQL's rejection of a
        // `STORAGE`/`COMPRESSION` written after a constraint, or of `COMPRESSION` before
        // `STORAGE`. Off-dialect the keywords are left for the constraint loop, where they
        // surface as a clean parse error.
        let storage = self.parse_optional_column_storage()?;
        let compression = self.parse_optional_column_compression()?;
        let mut constraints = ThinVec::new();
        while !self.peek_ends_table_element()? {
            match self.parse_column_constraint()? {
                Some(constraint) => constraints.push(constraint),
                None => return Err(self.unexpected("a column constraint")),
            }
        }
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(ColumnDef {
            name,
            data_type,
            storage,
            compression,
            constraints,
            meta,
        })
    }

    /// Parse an optional per-column `STORAGE {<name> | DEFAULT}` clause (PostgreSQL), when the
    /// column-storage feature is on and `STORAGE` is at the cursor. `None` otherwise —
    /// off-dialect the keyword is left for the constraint loop and surfaces as a clean parse
    /// error.
    fn parse_optional_column_storage(&mut self) -> ParseResult<Option<Box<Ident>>> {
        if !self.features().column_definition_syntax.column_storage
            || !self.peek_is_contextual_keyword("STORAGE")?
        {
            return Ok(None);
        }
        self.expect_contextual_keyword("STORAGE")?;
        Ok(Some(Box::new(self.parse_storage_method_value()?)))
    }

    /// Parse an optional per-column `COMPRESSION {<method> | DEFAULT}` clause (PostgreSQL), when
    /// the column-storage feature is on and `COMPRESSION` is at the cursor. `None` otherwise.
    fn parse_optional_column_compression(&mut self) -> ParseResult<Option<Box<Ident>>> {
        if !self.features().column_definition_syntax.column_storage
            || !self.peek_is_contextual_keyword("COMPRESSION")?
        {
            return Ok(None);
        }
        self.expect_contextual_keyword("COMPRESSION")?;
        Ok(Some(Box::new(self.parse_storage_method_value()?)))
    }

    /// Parse the shared `{ColId | DEFAULT}` value of a `STORAGE` / `COMPRESSION` clause —
    /// PostgreSQL's grammar takes any single (optionally quoted) identifier or the `DEFAULT`
    /// keyword, validating the specific word at analysis (out of this layer: `STORAGE bogus`
    /// is engine-measured accepted at raw parse, `STORAGE from` — a reserved keyword — is a
    /// raw-parse error, matched by `parse_ident`'s reserved set). A qualified `schema.method`
    /// is a raw-parse error too: only one identifier is read, and the trailing `.` is left for
    /// the constraint loop to reject. The `DEFAULT` keyword is interned as its written
    /// spelling via the every-keyword `ColLabel` parser, guarded to fire only on `DEFAULT`.
    fn parse_storage_method_value(&mut self) -> ParseResult<Ident> {
        if self.peek_is_contextual_keyword("DEFAULT")? {
            return self.parse_as_alias_ident();
        }
        self.parse_ident()
    }

    /// Whether an *unparenthesized* column default expression needs wrapping under MySQL's
    /// `DEFAULT (expr)` rule — a general function call (other than the `NOW()`
    /// `CURRENT_TIMESTAMP` synonym) or an operator expression. Literals, signed literals,
    /// and the special value functions (`CURRENT_TIMESTAMP`/`LOCALTIME`/… parse as
    /// [`Expr::SpecialFunction`], never [`Expr::Function`]) are simple defaults MySQL admits
    /// bare, so they never require wrapping.
    fn default_expr_requires_wrapping(&self, expr: &Expr<D::Ext>) -> bool {
        match expr {
            Expr::BinaryOp { .. } => true,
            Expr::Function { call, .. } => !self.is_now_synonym(&call.name),
            _ => false,
        }
    }

    /// Whether `name` is the bare, unqualified `NOW` function — the sole `CURRENT_TIMESTAMP`
    /// synonym that parses as an ordinary [`Expr::Function`] rather than a special value
    /// function, and so the one function-call default MySQL admits without parentheses.
    fn is_now_synonym(&self, name: &ObjectName) -> bool {
        let [part] = name.0.as_slice() else {
            return false;
        };
        part.quote == QuoteStyle::None && self.span_text(part.meta.span).eq_ignore_ascii_case("now")
    }

    fn parse_column_constraint(&mut self) -> ParseResult<Option<ColumnConstraint<D::Ext>>> {
        let start = self.current_span()?;
        let named = self.eat_contextual_keyword("CONSTRAINT")?;
        let name = if named {
            Some(self.parse_ident()?)
        } else {
            None
        };

        let Some(option) = self.parse_column_option()? else {
            // SQLite's trailing bodyless `CONSTRAINT <name>` (`a INT CONSTRAINT cn`, or
            // stacked `CONSTRAINT cn CONSTRAINT cn2` — each a separate bare marker): the name
            // is written but nothing recognizable as a column-option element follows, whether
            // that is the column/table-element terminator or another `CONSTRAINT` keyword
            // starting the next marker in this same loop. `CONSTRAINT <name> CHECK (…)` still
            // takes `CHECK` as its element above, unaffected by this flag.
            if named && self.features().constraint_syntax.bare_constraint_name {
                let span = start.union(self.preceding_span());
                // Two distinct nodes share this span (the wrapper and its bodyless element),
                // so each gets its own `make_meta` call — a fresh `node_id`, matching every
                // other constraint-with-element pairing below.
                let option_meta = self.make_meta(span);
                let meta = self.make_meta(span);
                return Ok(Some(ColumnConstraint {
                    name,
                    option: ColumnOption::Bare { meta: option_meta },
                    conflict: None,
                    characteristics: None,
                    meta,
                }));
            }
            if named {
                return Err(self.unexpected("a column constraint"));
            }
            return Ok(None);
        };
        // A `CONSTRAINT <symbol>` name on a column `COLLATE` is SQLite-only: SQLite's grammar
        // makes COLLATE an ordinary nameable column constraint, but in PostgreSQL `COLLATE
        // any_name` is a `ColConstraint` alternative *parallel to* the nameable
        // `ColConstraintElem` — `a text CONSTRAINT c COLLATE "C"` is engine-measured rejected on
        // libpg_query (and on DuckDB 1.5.4), so it rides its own `named_column_collate_constraint`
        // gate rather than the MySQL named-constraint one below. The bare column `COLLATE` itself
        // stays on `column_collation`; this flag only admits the `CONSTRAINT <name>` wrapper.
        if named
            && matches!(option, ColumnOption::Collate { .. })
            && !self
                .features()
                .column_definition_syntax
                .named_column_collate_constraint
        {
            let span = start.union(self.preceding_span());
            return Err(self.error_at(
                span,
                "no `CONSTRAINT <name>` prefix on a column COLLATE clause",
                self.span_text(span).to_owned(),
            ));
        }
        // MySQL admits a `CONSTRAINT <symbol>` name only on an inline `CHECK`; a named
        // inline `REFERENCES`/`UNIQUE`/`PRIMARY KEY`/`NOT NULL` is a syntax error there.
        if named
            && !matches!(option, ColumnOption::Check { .. })
            && !self
                .features()
                .constraint_syntax
                .named_inline_non_check_constraints
        {
            let span = start.union(self.preceding_span());
            return Err(self.error_at(
                span,
                "a `CONSTRAINT <name>` prefix only on an inline `CHECK` constraint",
                self.span_text(span).to_owned(),
            ));
        }
        let conflict = self.parse_optional_column_conflict_clause(&option)?;
        let characteristics = self.parse_constraint_characteristics()?.map(Box::new);
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(ColumnConstraint {
            name,
            option,
            conflict,
            characteristics,
            meta,
        }))
    }

    /// Parse the SQLite `ON CONFLICT <resolution>` clause attached to a column
    /// constraint, when the dialect admits it and `option` is one SQLite lets it qualify
    /// (`NOT NULL` / `UNIQUE` / `PRIMARY KEY` / `CHECK`). `None` when unwritten or
    /// off-dialect — the `ON` is then left for the caller, so a stray `ON CONFLICT`
    /// surfaces as a clean parse error.
    fn parse_optional_column_conflict_clause(
        &mut self,
        option: &ColumnOption<D::Ext>,
    ) -> ParseResult<Option<ConflictResolution>> {
        let admits_conflict = matches!(
            option,
            ColumnOption::NotNull { .. }
                | ColumnOption::Unique { .. }
                | ColumnOption::PrimaryKey { .. }
                | ColumnOption::Check { .. }
        );
        if !self
            .features()
            .column_definition_syntax
            .column_conflict_resolution_clause
            || !admits_conflict
            || !self.peek_is_keyword(Keyword::On)?
        {
            return Ok(None);
        }
        self.expect_keyword(Keyword::On)?;
        self.expect_contextual_keyword("CONFLICT")?;
        Ok(Some(self.parse_conflict_resolution()?))
    }

    /// Parse one SQLite conflict-resolution algorithm keyword.
    fn parse_conflict_resolution(&mut self) -> ParseResult<ConflictResolution> {
        if self.eat_contextual_keyword("ROLLBACK")? {
            Ok(ConflictResolution::Rollback)
        } else if self.eat_contextual_keyword("ABORT")? {
            Ok(ConflictResolution::Abort)
        } else if self.eat_contextual_keyword("FAIL")? {
            Ok(ConflictResolution::Fail)
        } else if self.eat_contextual_keyword("IGNORE")? {
            Ok(ConflictResolution::Ignore)
        } else if self.eat_contextual_keyword("REPLACE")? {
            Ok(ConflictResolution::Replace)
        } else {
            Err(self.unexpected(
                "a conflict resolution (`ROLLBACK`, `ABORT`, `FAIL`, `IGNORE`, or `REPLACE`)",
            ))
        }
    }

    fn parse_column_option(&mut self) -> ParseResult<Option<ColumnOption<D::Ext>>> {
        match D::parse_column_option_hook(self) {
            HookResult::Handled(option) => return Ok(Some(option)),
            HookResult::NotHandled => {}
            HookResult::Err(error) => return Err(error),
        }

        if self.peek_is_keyword(Keyword::Null)? {
            let token = self
                .advance()?
                .expect("peek_is_keyword confirmed NULL is present");
            let meta = self.make_meta(token.span);
            return Ok(Some(ColumnOption::Null { meta }));
        }
        if self.peek_is_keyword(Keyword::Not)? {
            let start = self.current_span()?;
            self.expect_keyword(Keyword::Not)?;
            self.expect_keyword(Keyword::Null)?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Some(ColumnOption::NotNull { meta }));
        }
        if self.peek_is_contextual_keyword("DEFAULT")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("DEFAULT")?;
            let parenthesized = self.peek_is_punct(Punctuation::LParen)?;
            // PostgreSQL parses a column-constraint `DEFAULT` as the restricted `b_expr`
            // (`ColConstraintElem: DEFAULT b_expr`), excluding the boolean/predicate
            // operators; the `ALTER COLUMN … SET DEFAULT` site keeps the full `a_expr`.
            let expr = if self
                .features()
                .column_definition_syntax
                .column_default_requires_b_expr
            {
                self.parse_b_expr()?
            } else {
                self.parse_expr()?
            };
            // MySQL requires a *functional* / general-expression default to be
            // parenthesized: `DEFAULT UUID()` / `DEFAULT 1 + 2` are syntax errors, while a
            // literal, a `CURRENT_TIMESTAMP`/`NOW()` temporal default, or `DEFAULT (expr)`
            // parse. A parenthesized leading `(` satisfies the requirement regardless of the
            // inner expression (parens are not preserved in the AST, ADR-0008).
            if self
                .features()
                .column_definition_syntax
                .default_expression_requires_parens
                && !parenthesized
                && self.default_expr_requires_wrapping(&expr)
            {
                let span = start.union(expr.span());
                return Err(self.error_at(
                    span,
                    "a parenthesized `DEFAULT (expr)` for a functional column default",
                    self.span_text(span).to_owned(),
                ));
            }
            let span = start.union(expr.span());
            let meta = self.make_meta(span);
            return Ok(Some(ColumnOption::Default {
                expr: Box::new(expr),
                meta,
            }));
        }
        if self.peek_is_contextual_keyword("GENERATED")? {
            return Ok(Some(self.parse_generated_or_identity_column()?));
        }
        if self
            .features()
            .column_definition_syntax
            .compact_identity_columns
            && self.peek_is_contextual_keyword("IDENTITY")?
        {
            let start = self.current_span()?;
            self.expect_contextual_keyword("IDENTITY")?;
            let mut options = ThinVec::new();
            if self.eat_punct(Punctuation::LParen)? {
                let seed = self.parse_expr()?;
                let seed_meta = self.make_meta(seed.span());
                options.push(IdentityOption::StartWith {
                    expr: seed,
                    meta: seed_meta,
                });
                self.expect_punct(
                    Punctuation::Comma,
                    "`,` between identity seed and increment",
                )?;
                let increment = self.parse_expr()?;
                let increment_meta = self.make_meta(increment.span());
                options.push(IdentityOption::IncrementBy {
                    expr: increment,
                    meta: increment_meta,
                });
                self.expect_punct(Punctuation::RParen, "`)` after identity increment")?;
            }
            let span = start.union(self.preceding_span());
            let identity = IdentityColumn {
                generation: IdentityGeneration::ByDefault,
                options,
                meta: self.make_meta(span),
            };
            return Ok(Some(ColumnOption::Identity {
                identity: Box::new(identity),
                meta: self.make_meta(span),
            }));
        }
        // MySQL/SQLite keywordless generated-column shorthand `AS (<expr>) [STORED|
        // VIRTUAL]`, gated by dialect data. The `AS (` two-token lookahead is
        // unambiguous — a column definition has no other `AS (` continuation — so when
        // off the `AS` is left for the caller and surfaces as a clean parse error.
        if self
            .features()
            .column_definition_syntax
            .generated_column_shorthand
            && self.peek_is_keyword(Keyword::As)?
            && self.peek_nth_is_punct(1, Punctuation::LParen)?
        {
            return Ok(Some(self.parse_generated_column_shorthand()?));
        }
        if self.peek_is_contextual_keyword("PRIMARY")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("PRIMARY")?;
            self.expect_contextual_keyword("KEY")?;
            // SQLite accepts an `ASC`/`DESC` sort-order qualifier on an inline primary key
            // (`id INT PRIMARY KEY DESC`); off-dialect the keyword is left for the next
            // parse step, so the trailing `ASC`/`DESC` surfaces as a clean parse error.
            let ascending = if self
                .features()
                .column_definition_syntax
                .inline_primary_key_ordering
            {
                if self.eat_keyword(Keyword::Asc)? {
                    Some(true)
                } else if self.eat_keyword(Keyword::Desc)? {
                    Some(false)
                } else {
                    None
                }
            } else {
                None
            };
            let index_tablespace = self.parse_optional_using_index_tablespace()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Some(ColumnOption::PrimaryKey {
                ascending,
                index_tablespace,
                meta,
            }));
        }
        if self.peek_is_contextual_keyword("UNIQUE")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("UNIQUE")?;
            let nulls_not_distinct = self.parse_optional_nulls_not_distinct()?;
            let index_tablespace = self.parse_optional_using_index_tablespace()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Some(ColumnOption::Unique {
                nulls_not_distinct,
                index_tablespace,
                meta,
            }));
        }
        // The auto-increment column attribute: MySQL's `AUTO_INCREMENT` (gated by the
        // trailing table options it partners with) and SQLite's `AUTOINCREMENT` (gated by
        // `joined_autoincrement_attribute`). The spelling tag round-trips whichever the source
        // wrote. Off under both gates, the keyword is left unconsumed and surfaces as a parse
        // error.
        if let Some(spelling) = self.parse_optional_auto_increment_spelling()? {
            let meta = self.make_meta(self.preceding_span());
            return Ok(Some(ColumnOption::AutoIncrement { spelling, meta }));
        }
        // The column-definition `COLLATE <collation>` clause: PostgreSQL (`a text COLLATE "C"`),
        // SQLite (`a TEXT COLLATE NOCASE`), and DuckDB all spell it here. Off-dialect
        // (ANSI/MySQL) the keyword is left unconsumed and surfaces as a parse error. The name is
        // a (possibly qualified) object name — matching the expression-level `COLLATE`
        // (`expression_syntax.collate`), which is a distinct surface qualifying an expression, not
        // a column.
        if self.features().column_definition_syntax.column_collation
            && self.peek_is_contextual_keyword("COLLATE")?
        {
            let start = self.current_span()?;
            self.expect_contextual_keyword("COLLATE")?;
            let collation = self.parse_object_name()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Some(ColumnOption::Collate {
                collation: Box::new(collation),
                meta,
            }));
        }
        if self.peek_is_contextual_keyword("CHECK")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("CHECK")?;
            let expr =
                self.parse_parenthesized_expr("`(` after `CHECK`", "`)` to close `CHECK`")?;
            self.reject_check_subquery_if_gated(&expr)?;
            // A column-level `CHECK (expr) NO INHERIT`: PostgreSQL bakes `opt_no_inherit` into
            // the column CHECK production (there is no column-level `NOT VALID`). Gated for
            // acceptance by `constraint_no_inherit_not_valid`; off-dialect the `NO INHERIT`
            // keyword is left unconsumed and surfaces as a clean parse error.
            let no_inherit = self
                .features()
                .constraint_syntax
                .constraint_no_inherit_not_valid
                && self.peek_is_contextual_keyword("NO")?
                && self.peek_nth_is_contextual_keyword(1, "INHERIT")?;
            if no_inherit {
                self.expect_contextual_keyword("NO")?;
                self.expect_contextual_keyword("INHERIT")?;
            }
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(Some(ColumnOption::Check {
                expr: Box::new(expr),
                no_inherit,
                meta,
            }));
        }
        if self.peek_is_contextual_keyword("REFERENCES")? {
            let reference = self.parse_foreign_key_ref()?;
            let meta = self.make_meta(reference.span());
            return Ok(Some(ColumnOption::References {
                reference: Box::new(reference),
                meta,
            }));
        }
        Ok(None)
    }

    /// Consume an auto-increment column attribute, if one is written and admitted:
    /// SQLite's joined `AUTOINCREMENT` (gated by `joined_autoincrement_attribute`) or the
    /// `AUTO_INCREMENT` underscore spelling (gated by
    /// `underscored_autoincrement_attribute`). Each spelling self-gates on its own flag
    /// so the two toggle independently. Returns the surface spelling so it round-trips,
    /// or `None` when no keyword is present or admitted (no input consumed).
    fn parse_optional_auto_increment_spelling(
        &mut self,
    ) -> ParseResult<Option<AutoIncrementSpelling>> {
        let column_def = self.features().column_definition_syntax;
        // SQLite's attribute is `AUTOINCREMENT` (one solid word); the underscore
        // `AUTO_INCREMENT` is *not* a SQLite attribute — SQLite reads it as a bareword
        // type-name token, so as a column attribute it is a syntax reject there. MySQL
        // admits the underscore spelling (its own gate, no longer piggybacked on the
        // trailing-table-options flag, so a preset can take the column attribute
        // without the whole MySQL option vocabulary).
        if column_def.joined_autoincrement_attribute
            && self.eat_contextual_keyword("AUTOINCREMENT")?
        {
            return Ok(Some(AutoIncrementSpelling::Joined));
        }
        if column_def.underscored_autoincrement_attribute
            && self.eat_contextual_keyword("AUTO_INCREMENT")?
        {
            return Ok(Some(AutoIncrementSpelling::Underscored));
        }
        Ok(None)
    }

    /// Whether the current token begins a column constraint — so a preceding SQLite
    /// typeless column has no type. Mirrors the keyword set
    /// [`parse_column_option`](Self::parse_column_option) dispatches on, plus the
    /// `CONSTRAINT` name prefix. Used only under the SQLite typeless gate, where
    /// `GENERATED`/`AUTOINCREMENT` are non-reserved and would otherwise be misread as an
    /// (affinity) type name.
    fn peek_starts_column_constraint(&mut self) -> ParseResult<bool> {
        Ok(self.peek_is_keyword(Keyword::Null)?
            || self.peek_is_keyword(Keyword::Not)?
            || self.peek_is_keyword(Keyword::As)?
            || self.peek_is_contextual_keyword("CONSTRAINT")?
            || self.peek_is_contextual_keyword("DEFAULT")?
            || self.peek_is_contextual_keyword("GENERATED")?
            || self.peek_is_contextual_keyword("PRIMARY")?
            || self.peek_is_contextual_keyword("UNIQUE")?
            || self.peek_is_contextual_keyword("CHECK")?
            || self.peek_is_contextual_keyword("REFERENCES")?
            || self.peek_is_contextual_keyword("COLLATE")?
            || self.peek_is_contextual_keyword("AUTOINCREMENT")?)
    }

    /// Whether the current column definition omits its data type. Two dialect rules widen the
    /// default (type required on every column):
    /// - SQLite's `typeless_column_definitions`: *any* column may drop its type when the next
    ///   token ends the element or begins a column constraint (`CREATE TABLE t (a, b)`).
    /// - DuckDB's narrower `typeless_generated_columns`: the type may be dropped *only* when the
    ///   column is a generated column — a `GENERATED …` clause or the `AS (<expr>)` shorthand
    ///   (`gen_x AS (x + 5)`) — but not a plain typeless column, which stays a parse error.
    ///
    /// When neither rule fires the type falls through to `parse_data_type`.
    fn column_definition_omits_type(&mut self) -> ParseResult<bool> {
        let column_def = self.features().column_definition_syntax;
        if column_def.typeless_column_definitions
            && (self.peek_ends_table_element()? || self.peek_starts_column_constraint()?)
        {
            return Ok(true);
        }
        if column_def.typeless_generated_columns && self.peek_starts_generated_column()? {
            return Ok(true);
        }
        Ok(false)
    }

    /// Whether the cursor sits on the start of a generated-column clause — the `GENERATED`
    /// keyword (covering both the `AS IDENTITY` and `AS (<expr>)` readings) or the keywordless
    /// `AS (` shorthand. Used only under `typeless_generated_columns`, where DuckDB lets a
    /// generated column omit its data type.
    fn peek_starts_generated_column(&mut self) -> ParseResult<bool> {
        Ok(self.peek_is_contextual_keyword("GENERATED")?
            || (self.peek_is_keyword(Keyword::As)?
                && self.peek_nth_is_punct(1, Punctuation::LParen)?))
    }

    fn parse_generated_or_identity_column(&mut self) -> ParseResult<ColumnOption<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("GENERATED")?;
        let generation = if self.eat_contextual_keyword("ALWAYS")? {
            IdentityGeneration::Always
        } else if self.eat_keyword(Keyword::By)? {
            self.expect_contextual_keyword("DEFAULT")?;
            IdentityGeneration::ByDefault
        } else {
            return Err(self.unexpected("`ALWAYS` or `BY DEFAULT`"));
        };
        self.expect_keyword(Keyword::As)?;

        if self.features().column_definition_syntax.identity_columns
            && self.eat_contextual_keyword("IDENTITY")?
        {
            let options = self.parse_identity_options()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            let identity = IdentityColumn {
                generation,
                options,
                meta,
            };
            let option_meta = self.make_meta(span);
            return Ok(ColumnOption::Identity {
                identity: Box::new(identity),
                meta: option_meta,
            });
        }

        if generation == IdentityGeneration::ByDefault {
            return Err(self.unexpected("`IDENTITY`"));
        }

        self.parse_generated_column_body(start, GeneratedColumnSpelling::GeneratedAlways)
    }

    /// Parse the MySQL/SQLite keywordless generated-column shorthand `AS (<expr>)
    /// [STORED|VIRTUAL]` (the `AS (` lookahead and the shorthand gate confirmed by
    /// [`parse_column_option`](Self::parse_column_option)). It folds onto the same
    /// [`GeneratedColumn`] node as the standard `GENERATED ALWAYS AS` form, tagged
    /// [`GeneratedColumnSpelling::Shorthand`] so the short spelling round-trips.
    fn parse_generated_column_shorthand(&mut self) -> ParseResult<ColumnOption<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::As)?;
        self.parse_generated_column_body(start, GeneratedColumnSpelling::Shorthand)
    }

    /// Parse the `( <expr> ) [STORED|VIRTUAL]` body shared by both generated-column
    /// spellings, tagging the built node with `spelling`. The introducing `AS` (and, for
    /// the standard form, `GENERATED ALWAYS`) is already consumed; the current token is
    /// the opening `(`. `start` is the span of the clause's first token so the node
    /// covers the whole extent.
    fn parse_generated_column_body(
        &mut self,
        start: Span,
        spelling: GeneratedColumnSpelling,
    ) -> ParseResult<ColumnOption<D::Ext>> {
        self.expect_punct(
            Punctuation::LParen,
            "`(` to open the generated column expression",
        )?;
        let expr = self.parse_expr()?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the generated column expression",
        )?;
        let storage = if self.eat_contextual_keyword("STORED")? {
            Some(GeneratedColumnStorage::Stored)
        } else if self.eat_contextual_keyword("VIRTUAL")? {
            Some(GeneratedColumnStorage::Virtual)
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        let generated = GeneratedColumn {
            expr,
            storage,
            spelling,
            meta,
        };
        let option_meta = self.make_meta(span);
        Ok(ColumnOption::Generated {
            generated: Box::new(generated),
            meta: option_meta,
        })
    }

    fn parse_identity_options(&mut self) -> ParseResult<ThinVec<IdentityOption<D::Ext>>> {
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok(ThinVec::new());
        }
        let mut options = ThinVec::new();
        while !self.peek_is_punct(Punctuation::RParen)? {
            options.push(self.parse_identity_option(true)?);
        }
        self.expect_punct(Punctuation::RParen, "`)` to close the identity options")?;
        Ok(options)
    }

    /// Parse one `START [WITH]` / `INCREMENT [BY]` / `MIN`·`MAXVALUE` (or `NO …`) / `CACHE` /
    /// `CYCLE` sequence-generator option. Shared by identity-column and sequence statements;
    /// callers select whether the independently gated `CACHE` branch is available.
    fn parse_identity_option(&mut self, allow_cache: bool) -> ParseResult<IdentityOption<D::Ext>> {
        if self.peek_is_contextual_keyword("START")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("START")?;
            self.eat_keyword(Keyword::With)?;
            let expr = self.parse_expr()?;
            let span = start.union(expr.span());
            let meta = self.make_meta(span);
            return Ok(IdentityOption::StartWith { expr, meta });
        }
        if self.peek_is_contextual_keyword("INCREMENT")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("INCREMENT")?;
            self.eat_keyword(Keyword::By)?;
            let expr = self.parse_expr()?;
            let span = start.union(expr.span());
            let meta = self.make_meta(span);
            return Ok(IdentityOption::IncrementBy { expr, meta });
        }
        if self.peek_is_keyword(Keyword::No)? {
            let start = self.current_span()?;
            self.expect_keyword(Keyword::No)?;
            if self.eat_contextual_keyword("MINVALUE")? {
                let span = start.union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(IdentityOption::MinValue { value: None, meta });
            }
            if self.eat_contextual_keyword("MAXVALUE")? {
                let span = start.union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(IdentityOption::MaxValue { value: None, meta });
            }
            if self.eat_contextual_keyword("CYCLE")? {
                let span = start.union(self.preceding_span());
                let meta = self.make_meta(span);
                return Ok(IdentityOption::Cycle { cycle: false, meta });
            }
            return Err(self.unexpected("`MINVALUE`, `MAXVALUE`, or `CYCLE`"));
        }
        if self.peek_is_contextual_keyword("MINVALUE")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("MINVALUE")?;
            let expr = self.parse_expr()?;
            let span = start.union(expr.span());
            let meta = self.make_meta(span);
            return Ok(IdentityOption::MinValue {
                value: Some(expr),
                meta,
            });
        }
        if self.peek_is_contextual_keyword("MAXVALUE")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("MAXVALUE")?;
            let expr = self.parse_expr()?;
            let span = start.union(expr.span());
            let meta = self.make_meta(span);
            return Ok(IdentityOption::MaxValue {
                value: Some(expr),
                meta,
            });
        }
        if allow_cache && self.peek_is_contextual_keyword("CACHE")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("CACHE")?;
            let expr = self.parse_expr()?;
            let span = start.union(expr.span());
            let meta = self.make_meta(span);
            return Ok(IdentityOption::Cache { expr, meta });
        }
        if self.peek_is_contextual_keyword("CYCLE")? {
            let token = self
                .advance()?
                .expect("peek_is_contextual_keyword confirmed CYCLE is present");
            let meta = self.make_meta(token.span);
            return Ok(IdentityOption::Cycle { cycle: true, meta });
        }
        Err(self.unexpected("an identity option"))
    }

    /// Parse a `CREATE [TEMP] SEQUENCE [IF NOT EXISTS] <name> [<option> ...]` after the
    /// leading `CREATE [TEMP] SEQUENCE` (the `SEQUENCE` keyword is already consumed).
    ///
    /// The trailing options are the shared SQL-standard T176 core both PostgreSQL and DuckDB
    /// parse-accept in any order (see [`parse_sequence_options`](Self::parse_sequence_options)).
    fn parse_create_sequence(
        &mut self,
        start: Span,
        temporary: Option<TemporaryTableKind>,
    ) -> ParseResult<Statement<D::Ext>> {
        let if_not_exists = self.parse_if_not_exists()?;
        let name = self.parse_target_relation_name()?;
        let options = self.parse_sequence_options()?;
        let span = start.union(self.preceding_span());
        let create = CreateSequence {
            temporary,
            if_not_exists,
            name,
            options,
            meta: self.make_meta(span),
        };
        Ok(Statement::CreateSequence {
            create: Box::new(create),
            meta: self.make_meta(span),
        })
    }

    /// Parse the trailing, space-separated sequence options of a `CREATE SEQUENCE`.
    ///
    /// Unlike the parenthesized identity-column option list, the options simply follow the
    /// name; the loop stops at the first token that is not an option lead. Each option reuses
    /// the shared [`parse_identity_option`](Self::parse_identity_option). `CACHE` is admitted
    /// only when the dialect's dedicated sequence-cache gate is enabled.
    fn parse_sequence_options(&mut self) -> ParseResult<ThinVec<IdentityOption<D::Ext>>> {
        let mut options = ThinVec::new();
        let allow_cache = self
            .features()
            .view_sequence_clause_syntax
            .create_sequence_cache;
        while self.peek_is_sequence_option()?
            || (allow_cache && self.peek_is_contextual_keyword("CACHE")?)
        {
            options.push(self.parse_identity_option(allow_cache)?);
        }
        Ok(options)
    }

    /// Whether the cursor sits on a `CREATE SEQUENCE` option lead — `START`, `INCREMENT`,
    /// `MINVALUE`, `MAXVALUE`, `CYCLE`, or the `NO` of `NO MIN`/`MAXVALUE`/`CYCLE`.
    /// `CACHE` is checked separately against its dialect gate.
    fn peek_is_sequence_option(&mut self) -> ParseResult<bool> {
        Ok(self.peek_is_contextual_keyword("START")?
            || self.peek_is_contextual_keyword("INCREMENT")?
            || self.peek_is_contextual_keyword("MINVALUE")?
            || self.peek_is_contextual_keyword("MAXVALUE")?
            || self.peek_is_contextual_keyword("CYCLE")?
            || self.peek_is_keyword(Keyword::No)?)
    }

    fn parse_table_constraint_def(&mut self) -> ParseResult<TableConstraintDef<D::Ext>> {
        let start = self.current_span()?;
        let named = self.eat_contextual_keyword("CONSTRAINT")?;
        let name = if named {
            Some(self.parse_ident()?)
        } else {
            None
        };
        // SQLite's trailing bodyless `CONSTRAINT <name>` as a standalone table constraint
        // (`CREATE TABLE t (a INT, CONSTRAINT cn)`): the name is written but nothing else
        // follows before the element terminator. Checked only for a *named* constraint with
        // nothing after it — `CONSTRAINT <name> UNIQUE (…)` still takes `UNIQUE` as its
        // element below, unaffected by this flag. The main table-element loop separately
        // tolerates the elided comma before a bare constraint that follows a preceding one.
        if named
            && self.features().constraint_syntax.bare_constraint_name
            && self.peek_ends_table_element()?
        {
            let span = start.union(self.preceding_span());
            // Two distinct nodes share this span (the wrapper and its bodyless element), so
            // each gets its own `make_meta` call — a fresh `node_id`.
            let constraint_meta = self.make_meta(span);
            let meta = self.make_meta(span);
            return Ok(TableConstraintDef {
                name,
                constraint: TableConstraint::Bare {
                    meta: constraint_meta,
                },
                no_inherit: false,
                not_valid: false,
                characteristics: None,
                meta,
            });
        }
        let constraint = self.parse_table_constraint()?;
        let (no_inherit, not_valid) = self.parse_constraint_markers(&constraint)?;
        let characteristics = self.parse_constraint_characteristics()?.map(Box::new);
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableConstraintDef {
            name,
            constraint,
            no_inherit,
            not_valid,
            characteristics,
            meta,
        })
    }

    /// Parse the order-free `NO INHERIT` / `NOT VALID` constraint markers that share
    /// PostgreSQL's `ConstraintAttributeSpec` slot (gated by
    /// [`ConstraintSyntax::constraint_no_inherit_not_valid`](crate::ast::dialect::ConstraintSyntax)). PostgreSQL validates which
    /// markers a constraint kind admits in the grammar action (`processCASbits`), rejecting
    /// `NOT VALID` on `PRIMARY KEY`/`UNIQUE`/`EXCLUDE` and `NO INHERIT` on anything but `CHECK`;
    /// this reproduces that reject at the parse layer so the acceptance stays faithful (a
    /// mismatched marker is a clean parse error, not a silent over-acceptance). The `NOT VALID`
    /// second word distinguishes it from a following `NOT DEFERRABLE` characteristic.
    fn parse_constraint_markers(
        &mut self,
        constraint: &TableConstraint<D::Ext>,
    ) -> ParseResult<(bool, bool)> {
        if !self
            .features()
            .constraint_syntax
            .constraint_no_inherit_not_valid
        {
            return Ok((false, false));
        }
        let is_check = matches!(constraint, TableConstraint::Check { .. });
        let is_foreign_key = matches!(constraint, TableConstraint::ForeignKey { .. });
        let mut no_inherit = false;
        let mut not_valid = false;
        loop {
            if !no_inherit
                && self.peek_is_contextual_keyword("NO")?
                && self.peek_nth_is_contextual_keyword(1, "INHERIT")?
            {
                let start = self.current_span()?;
                self.expect_contextual_keyword("NO")?;
                self.expect_contextual_keyword("INHERIT")?;
                // Only CHECK constraints admit NO INHERIT (`processCASbits`).
                if !is_check {
                    return Err(self.error_at(
                        start.union(self.preceding_span()),
                        "a `NO INHERIT` marker only on a CHECK constraint",
                        "NO INHERIT".to_owned(),
                    ));
                }
                no_inherit = true;
            } else if !not_valid
                && self.peek_is_keyword(Keyword::Not)?
                && self.peek_nth_is_contextual_keyword(1, "VALID")?
            {
                let start = self.current_span()?;
                self.expect_keyword(Keyword::Not)?;
                self.expect_contextual_keyword("VALID")?;
                // Only CHECK and FOREIGN KEY constraints admit NOT VALID (`processCASbits`).
                if !is_check && !is_foreign_key {
                    return Err(self.error_at(
                        start.union(self.preceding_span()),
                        "a `NOT VALID` marker only on a CHECK or FOREIGN KEY constraint",
                        "NOT VALID".to_owned(),
                    ));
                }
                not_valid = true;
            } else {
                break;
            }
        }
        Ok((no_inherit, not_valid))
    }

    fn parse_unnamed_table_constraint_hook(
        &mut self,
    ) -> ParseResult<Option<TableConstraintDef<D::Ext>>> {
        let start = self.current_span()?;
        match D::parse_table_constraint_hook(self) {
            HookResult::Handled(constraint) => {
                let span = start.union(self.preceding_span());
                let meta = self.make_meta(span);
                Ok(Some(TableConstraintDef {
                    name: None,
                    constraint,
                    no_inherit: false,
                    not_valid: false,
                    characteristics: None,
                    meta,
                }))
            }
            HookResult::NotHandled => Ok(None),
            HookResult::Err(error) => Err(error),
        }
    }

    fn parse_table_constraint(&mut self) -> ParseResult<TableConstraint<D::Ext>> {
        match D::parse_table_constraint_hook(self) {
            HookResult::Handled(constraint) => return Ok(constraint),
            HookResult::NotHandled => {}
            HookResult::Err(error) => return Err(error),
        }

        if self.peek_is_contextual_keyword("PRIMARY")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("PRIMARY")?;
            self.expect_contextual_keyword("KEY")?;
            let columns = self.parse_constraint_column_list(
                "`(` after `PRIMARY KEY`",
                "`)` to close `PRIMARY KEY`",
            )?;
            let include = self.parse_optional_include_columns()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(TableConstraint::PrimaryKey {
                columns,
                include,
                meta,
            });
        }
        if self.peek_is_contextual_keyword("UNIQUE")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("UNIQUE")?;
            let nulls_not_distinct = self.parse_optional_nulls_not_distinct()?;
            let columns =
                self.parse_constraint_column_list("`(` after `UNIQUE`", "`)` to close `UNIQUE`")?;
            let include = self.parse_optional_include_columns()?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(TableConstraint::Unique {
                columns,
                nulls_not_distinct,
                include,
                meta,
            });
        }
        if self.features().constraint_syntax.exclusion_constraints
            && self.peek_is_contextual_keyword("EXCLUDE")?
        {
            return self.parse_exclude_constraint();
        }
        if self.peek_is_contextual_keyword("CHECK")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("CHECK")?;
            let expr =
                self.parse_parenthesized_expr("`(` after `CHECK`", "`)` to close `CHECK`")?;
            self.reject_check_subquery_if_gated(&expr)?;
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            return Ok(TableConstraint::Check {
                expr: Box::new(expr),
                meta,
            });
        }
        if self.peek_is_contextual_keyword("FOREIGN")? {
            let start = self.current_span()?;
            self.expect_contextual_keyword("FOREIGN")?;
            self.expect_contextual_keyword("KEY")?;
            let columns = self.parse_parenthesized_ident_list(
                "`(` after `FOREIGN KEY`",
                "`)` to close `FOREIGN KEY`",
            )?;
            let references = self.parse_foreign_key_ref()?;
            let span = start.union(references.span());
            let meta = self.make_meta(span);
            return Ok(TableConstraint::ForeignKey {
                columns,
                references: Box::new(references),
                meta,
            });
        }
        Err(self.unexpected("a table constraint"))
    }

    /// Parse an optional `INCLUDE (<col>, ...)` covering-column list (gated by
    /// [`ConstraintSyntax::index_constraint_parameters`](crate::ast::dialect::ConstraintSyntax)). Empty when unwritten or off-dialect,
    /// where the `INCLUDE` keyword is left unconsumed and surfaces as a clean parse error.
    fn parse_optional_include_columns(&mut self) -> ParseResult<ThinVec<Ident>> {
        if !self
            .features()
            .constraint_syntax
            .index_constraint_parameters
            || !self.peek_is_contextual_keyword("INCLUDE")?
        {
            return Ok(ThinVec::new());
        }
        self.expect_contextual_keyword("INCLUDE")?;
        self.parse_parenthesized_ident_list(
            "`(` after `INCLUDE`",
            "`)` to close the `INCLUDE` column list",
        )
    }

    /// Parse an optional `NULLS [NOT] DISTINCT` null-treatment (gated by
    /// [`ConstraintSyntax::index_constraint_parameters`](crate::ast::dialect::ConstraintSyntax)). `Some(false)` for `NULLS NOT
    /// DISTINCT`, `Some(true)` for the explicit `NULLS DISTINCT` default, `None` when unwritten or
    /// off-dialect (the `NULLS` keyword is then left unconsumed and surfaces as a clean parse
    /// error).
    fn parse_optional_nulls_not_distinct(&mut self) -> ParseResult<Option<bool>> {
        if !self
            .features()
            .constraint_syntax
            .index_constraint_parameters
            || !self.peek_is_keyword(Keyword::Nulls)?
        {
            return Ok(None);
        }
        self.expect_keyword(Keyword::Nulls)?;
        if self.eat_keyword(Keyword::Not)? {
            self.expect_contextual_keyword("DISTINCT")?;
            Ok(Some(false))
        } else {
            self.expect_contextual_keyword("DISTINCT")?;
            Ok(Some(true))
        }
    }

    /// Parse an optional `USING INDEX TABLESPACE <name>` index-parameter clause on a column-level
    /// `PRIMARY KEY`/`UNIQUE` (gated by [`ConstraintSyntax::index_constraint_parameters`](crate::ast::dialect::ConstraintSyntax)).
    /// `None` when unwritten or off-dialect. The name is boxed as the cold clause it is.
    fn parse_optional_using_index_tablespace(&mut self) -> ParseResult<Option<Box<Ident>>> {
        if !self
            .features()
            .constraint_syntax
            .index_constraint_parameters
            || !self.peek_is_contextual_keyword("USING")?
        {
            return Ok(None);
        }
        self.expect_contextual_keyword("USING")?;
        self.expect_contextual_keyword("INDEX")?;
        self.expect_contextual_keyword("TABLESPACE")?;
        Ok(Some(Box::new(self.parse_ident()?)))
    }

    /// Parse a parenthesized reloptions list `(<name> [= <value>] [, ...])` — the PostgreSQL
    /// `reloptions` grammar shared by a constraint's `WITH (...)` storage parameters and an index
    /// element's operator-class parameters. Each element reuses [`parse_storage_parameter`](Self::parse_storage_parameter).
    fn parse_reloptions_list(&mut self) -> ParseResult<ThinVec<TableStorageParameter<D::Ext>>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the parameter list")?;
        let params = self.parse_comma_separated(Self::parse_storage_parameter)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the parameter list")?;
        Ok(params)
    }

    /// Parse a PostgreSQL `EXCLUDE [USING <method>] (<element> WITH <operator> [, ...]) [INCLUDE
    /// (...)] [WITH (...)] [USING INDEX TABLESPACE ...] [WHERE (...)]` exclusion constraint. Reached
    /// only under [`ConstraintSyntax::exclusion_constraints`](crate::ast::dialect::ConstraintSyntax) with `EXCLUDE` at the cursor.
    fn parse_exclude_constraint(&mut self) -> ParseResult<TableConstraint<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("EXCLUDE")?;
        let method = if self.eat_contextual_keyword("USING")? {
            Some(self.parse_ident()?)
        } else {
            None
        };
        self.expect_punct(Punctuation::LParen, "`(` to open the EXCLUDE element list")?;
        let elements = self.parse_comma_separated(Self::parse_exclude_element)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the EXCLUDE element list")?;
        let include = self.parse_optional_include_columns()?;
        // The `WITH (...)` index storage parameters (reloptions). Distinct from the element's
        // `WITH <operator>` (already consumed) — this `WITH` is immediately followed by `(`.
        let with_params = if self.peek_is_keyword(Keyword::With)? {
            self.expect_keyword(Keyword::With)?;
            self.parse_reloptions_list()?
        } else {
            ThinVec::new()
        };
        let index_tablespace = if self.eat_contextual_keyword("USING")? {
            self.expect_contextual_keyword("INDEX")?;
            self.expect_contextual_keyword("TABLESPACE")?;
            Some(self.parse_ident()?)
        } else {
            None
        };
        let predicate = if self.eat_keyword(Keyword::Where)? {
            self.expect_punct(Punctuation::LParen, "`(` after `WHERE`")?;
            let expr = self.parse_expr()?;
            self.expect_punct(Punctuation::RParen, "`)` to close the `WHERE` predicate")?;
            Some(Box::new(expr))
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableConstraint::Exclude {
            exclude: Box::new(ExcludeConstraint {
                method,
                elements,
                include,
                with_params,
                index_tablespace,
                predicate,
                meta,
            }),
            meta,
        })
    }

    /// Parse one `<index_element> WITH <operator>` EXCLUDE element (PostgreSQL
    /// `ExclusionConstraintElem`). The index element mirrors a `CREATE INDEX` key: a bare column,
    /// bare function call, or parenthesized `(a_expr)`, then optional `COLLATE`, operator-class
    /// name + reloptions, and `ASC`/`DESC` + `NULLS FIRST`/`LAST` — reusing
    /// [`parse_partition_key_head`](Self::parse_partition_key_head) for the arg (which stops before
    /// the `index_elem` tail clauses, so `COLLATE` / the opclass stay unconsumed).
    fn parse_exclude_element(&mut self) -> ParseResult<ExcludeElement<D::Ext>> {
        let start = self.current_span()?;
        let (expr, parenthesized) = self.parse_partition_key_head()?;
        if !parenthesized
            && !matches!(
                expr,
                Expr::Column { .. } | Expr::Function { .. } | Expr::SpecialFunction { .. }
            )
        {
            let span = start.union(self.preceding_span());
            return Err(self.error_at(
                span,
                "a bare column, a function call, or a parenthesized expression as the EXCLUDE key",
                self.span_text(span).to_owned(),
            ));
        }
        let collation = if self.eat_contextual_keyword("COLLATE")? {
            Some(self.parse_object_name()?)
        } else {
            None
        };
        // The operator-class name is an optional qualified name, taken only when the next token is
        // not the `WITH` operator keyword, an ordering keyword, or an element terminator.
        let opclass = if self.peek_is_keyword(Keyword::With)?
            || self.peek_is_keyword(Keyword::Asc)?
            || self.peek_is_keyword(Keyword::Desc)?
            || self.peek_is_keyword(Keyword::Nulls)?
        {
            None
        } else {
            Some(self.parse_object_name()?)
        };
        // The operator-class reloptions `(<param> [= <value>], ...)`, present only after an
        // operator-class name (PostgreSQL `index_elem` `any_name reloptions`).
        let opclass_params = if opclass.is_some() && self.peek_is_punct(Punctuation::LParen)? {
            self.parse_reloptions_list()?
        } else {
            ThinVec::new()
        };
        let asc = if self.eat_keyword(Keyword::Asc)? {
            Some(true)
        } else if self.eat_keyword(Keyword::Desc)? {
            Some(false)
        } else {
            None
        };
        let nulls_first = if self.eat_keyword(Keyword::Nulls)? {
            if self.eat_keyword(Keyword::First)? {
                Some(true)
            } else if self.eat_keyword(Keyword::Last)? {
                Some(false)
            } else {
                return Err(self.unexpected("`FIRST` or `LAST` after `NULLS`"));
            }
        } else {
            None
        };
        self.expect_keyword(Keyword::With)?;
        let operator = self.parse_exclude_operator()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(ExcludeElement {
            expr,
            parenthesized,
            collation,
            opclass,
            opclass_params,
            asc,
            nulls_first,
            operator,
            meta,
        })
    }

    /// Parse the `WITH <operator>` operator of an EXCLUDE element: PostgreSQL `any_operator`, a
    /// bare symbolic operator (`&&`, `=`, `-|-`) or the explicit `OPERATOR(<schema>.<op>)` keyword
    /// form.
    fn parse_exclude_operator(&mut self) -> ParseResult<ExcludeOperator> {
        if self.eat_contextual_keyword("OPERATOR")? {
            self.expect_punct(Punctuation::LParen, "`(` after `OPERATOR`")?;
            let mut schema = ThinVec::new();
            while self
                .peek()?
                .is_some_and(|token| self.token_can_be_column_name(token))
                && self.peek_nth_is_punct(1, Punctuation::Dot)?
            {
                schema.push(self.parse_ident()?);
                self.expect_punct(Punctuation::Dot, "`.` in the qualified operator name")?;
            }
            let op = self.parse_operator_symbol()?;
            self.expect_punct(Punctuation::RParen, "`)` to close `OPERATOR(...)`")?;
            Ok(ExcludeOperator {
                schema: ObjectName(schema),
                op,
                spelling: NamedOperatorSpelling::OperatorKeyword,
            })
        } else {
            let op = self.parse_operator_symbol()?;
            Ok(ExcludeOperator {
                schema: ObjectName(ThinVec::new()),
                op,
                spelling: NamedOperatorSpelling::Bare,
            })
        }
    }

    fn parse_foreign_key_ref(&mut self) -> ParseResult<ForeignKeyRef> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("REFERENCES")?;
        let table = self.parse_object_name()?;
        let columns = if self.peek_is_punct(Punctuation::LParen)? {
            self.parse_parenthesized_ident_list(
                "`(` to open the referenced column list",
                "`)` to close the referenced column list",
            )?
        } else {
            ThinVec::new()
        };

        // PostgreSQL's foreign-key grammar fixes the clause order as `key_match
        // key_actions`: an optional `MATCH` type precedes the referential actions, and a
        // `MATCH` written after them is rejected. The two actions themselves stay
        // order-independent (either `ON UPDATE` or `ON DELETE` first); a repeated action
        // stops the loop, left as leftover input the enclosing parser rejects — matching
        // PostgreSQL's "conflicting ON DELETE/UPDATE actions" error.
        let match_type = if self.peek_is_contextual_keyword("MATCH")? {
            self.expect_contextual_keyword("MATCH")?;
            Some(self.parse_foreign_key_match()?)
        } else {
            None
        };
        let mut on_delete = None;
        let mut on_update = None;
        // Records whether `ON UPDATE` was written before `ON DELETE`, the one bit of
        // order the two separate fields otherwise lose (the canonical render is
        // `ON DELETE`-first). `true` iff `ON UPDATE` is parsed while `ON DELETE` is
        // still absent.
        let mut update_before_delete = false;
        loop {
            if on_delete.is_none()
                && self.peek_is_contextual_keyword("ON")?
                && self.peek_nth_is_contextual_keyword(1, "DELETE")?
            {
                self.expect_contextual_keyword("ON")?;
                self.expect_contextual_keyword("DELETE")?;
                // The `SET NULL (col, ...)` column list is valid only on `ON DELETE`.
                on_delete = Some(Box::new(self.parse_referential_action(true)?));
            } else if on_update.is_none()
                && self.peek_is_contextual_keyword("ON")?
                && self.peek_nth_is_contextual_keyword(1, "UPDATE")?
            {
                self.expect_contextual_keyword("ON")?;
                self.expect_contextual_keyword("UPDATE")?;
                update_before_delete = on_delete.is_none();
                on_update = Some(Box::new(self.parse_referential_action(false)?));
            } else {
                break;
            }
        }

        // A `MATCH` after the referential actions is the over-acceptance rejected for PG
        // parity: the grammar places `MATCH` before them.
        if self.peek_is_contextual_keyword("MATCH")? {
            return Err(self.unexpected("`MATCH` before the `ON UPDATE` / `ON DELETE` actions"));
        }

        // The written-order tag is meaningful only when *both* actions are present; a
        // lone `ON UPDATE` (no `ON DELETE`) renders and re-parses in the canonical order,
        // so it must carry the canonical `false` rather than the "update seen first" bit.
        let update_before_delete = update_before_delete && on_delete.is_some();

        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(ForeignKeyRef {
            table,
            columns,
            match_type,
            on_delete,
            on_update,
            update_before_delete,
            meta,
        })
    }

    fn parse_foreign_key_match(&mut self) -> ParseResult<ForeignKeyMatch> {
        if self.eat_contextual_keyword("FULL")? {
            Ok(ForeignKeyMatch::Full)
        } else if self.peek_is_contextual_keyword("PARTIAL")? {
            // `MATCH PARTIAL` is standard-SQL syntax, but its referential semantics are
            // unimplemented in PostgreSQL, which rejects it at parse time ("MATCH PARTIAL
            // not yet implemented"). We match that verdict rather than accept a mode whose
            // constraint we would not enforce; `MATCH FULL`/`SIMPLE` stay accepted. The
            // token is left unconsumed — the parse aborts on this error.
            Err(self.unexpected("`FULL` or `SIMPLE` (`MATCH PARTIAL` is unsupported)"))
        } else if self.eat_contextual_keyword("SIMPLE")? {
            Ok(ForeignKeyMatch::Simple)
        } else {
            Err(self.unexpected("`FULL` or `SIMPLE` after `MATCH`"))
        }
    }

    /// Reject a `CHECK` body that contains a subquery when
    /// [`ConstraintSyntax::check_constraint_subqueries`](crate::ast::dialect::ConstraintSyntax) is off (DuckDB/SQLite).
    fn reject_check_subquery_if_gated(&mut self, expr: &Expr<D::Ext>) -> ParseResult<()> {
        if self
            .features()
            .constraint_syntax
            .check_constraint_subqueries
        {
            return Ok(());
        }
        if expr_contains_subquery(expr) {
            return Err(self.unexpected(
                "a CHECK expression without a subquery (subqueries are not admitted in CHECK under this dialect)",
            ));
        }
        Ok(())
    }

    /// Parse a `<referential action>` after `ON DELETE` / `ON UPDATE`.
    ///
    /// `allow_columns` enables the PostgreSQL `SET NULL (col, ...)` / `SET DEFAULT
    /// (col, ...)` column list, which the grammar permits only on `ON DELETE`; an
    /// `ON UPDATE` action leaves a trailing column list unconsumed so it surfaces as
    /// a parse error, matching PostgreSQL.
    fn parse_referential_action(&mut self, allow_columns: bool) -> ParseResult<ReferentialAction> {
        let start = self.current_span()?;
        let cascade_set = self
            .features()
            .constraint_syntax
            .referential_action_cascade_set;
        if self.eat_contextual_keyword("CASCADE")? {
            if !cascade_set {
                return Err(self.unexpected(
                    "RESTRICT or NO ACTION (CASCADE is not admitted under this dialect)",
                ));
            }
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ReferentialAction::Cascade { meta });
        }
        if self.eat_contextual_keyword("RESTRICT")? {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ReferentialAction::Restrict { meta });
        }
        if self.eat_contextual_keyword("NO")? {
            self.expect_contextual_keyword("ACTION")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(ReferentialAction::NoAction { meta });
        }
        if self.eat_contextual_keyword("SET")? {
            if !cascade_set {
                return Err(self.unexpected(
                    "RESTRICT or NO ACTION (SET NULL / SET DEFAULT are not admitted under this dialect)",
                ));
            }
            if self.eat_contextual_keyword("NULL")? {
                let columns = self.parse_referential_action_columns(allow_columns)?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(ReferentialAction::SetNull { columns, meta });
            }
            if self.eat_contextual_keyword("DEFAULT")? {
                let columns = self.parse_referential_action_columns(allow_columns)?;
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(ReferentialAction::SetDefault { columns, meta });
            }
            return Err(self.unexpected("`NULL` or `DEFAULT` after `SET`"));
        }
        Err(self.unexpected(if cascade_set {
            "a referential action (`NO ACTION`, `RESTRICT`, `CASCADE`, `SET NULL`, or `SET DEFAULT`)"
        } else {
            "a referential action (`NO ACTION` or `RESTRICT`)"
        }))
    }

    fn parse_referential_action_columns(
        &mut self,
        allow_columns: bool,
    ) -> ParseResult<ThinVec<Ident>> {
        if allow_columns && self.peek_is_punct(Punctuation::LParen)? {
            self.parse_parenthesized_ident_list(
                "`(` to open the referential-action column list",
                "`)` to close the referential-action column list",
            )
        } else {
            Ok(ThinVec::new())
        }
    }

    /// Parse a trailing `<constraint characteristics>` clause (`[NOT] DEFERRABLE` /
    /// `INITIALLY {DEFERRED | IMMEDIATE}`), or `None` when none is written.
    ///
    /// The two clauses are order-independent in the standard, so they parse in any
    /// order; a clause already filled stops the loop, leaving a duplicate as leftover
    /// input the enclosing parser rejects. `NOT` is committed only when `DEFERRABLE`
    /// follows, so a `NOT NULL` column constraint after this one is left intact.
    fn parse_constraint_characteristics(
        &mut self,
    ) -> ParseResult<Option<ConstraintCharacteristics>> {
        // MySQL has no deferrable constraints: with this off the `DEFERRABLE`/`INITIALLY`
        // keyword is left as leftover input and the enclosing parser rejects it.
        if !self.features().constraint_syntax.deferrable_constraints {
            return Ok(None);
        }
        let start = self.current_span()?;
        let mut deferrable = None;
        let mut initially_deferred = None;
        loop {
            if deferrable.is_none() && self.peek_is_contextual_keyword("DEFERRABLE")? {
                self.expect_contextual_keyword("DEFERRABLE")?;
                deferrable = Some(true);
            } else if deferrable.is_none()
                && self.peek_is_keyword(Keyword::Not)?
                && self.peek_nth_is_contextual_keyword(1, "DEFERRABLE")?
            {
                self.expect_keyword(Keyword::Not)?;
                self.expect_contextual_keyword("DEFERRABLE")?;
                deferrable = Some(false);
            } else if initially_deferred.is_none()
                && self.peek_is_contextual_keyword("INITIALLY")?
            {
                self.expect_contextual_keyword("INITIALLY")?;
                if self.eat_contextual_keyword("DEFERRED")? {
                    initially_deferred = Some(true);
                } else if self.eat_contextual_keyword("IMMEDIATE")? {
                    initially_deferred = Some(false);
                } else {
                    return Err(self.unexpected("`DEFERRED` or `IMMEDIATE` after `INITIALLY`"));
                }
            } else {
                break;
            }
        }
        if deferrable.is_none() && initially_deferred.is_none() {
            return Ok(None);
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(ConstraintCharacteristics {
            deferrable,
            initially_deferred,
            meta,
        }))
    }

    fn parse_create_table_options(&mut self) -> ParseResult<ThinVec<CreateTableOption<D::Ext>>> {
        let mut options = ThinVec::new();
        while let Some(option) = self.parse_create_table_option()? {
            let sqlite_option = matches!(
                option.kind,
                CreateTableOptionKind::WithoutRowid { .. } | CreateTableOptionKind::Strict { .. }
            );
            options.push(option);
            // SQLite comma-separates its trailing table options (`STRICT, WITHOUT ROWID`);
            // MySQL/PostgreSQL space-separate theirs. Consume the comma only when another
            // SQLite option follows, so a trailing comma is left for the statement
            // terminator to reject (SQLite rejects `... STRICT,`).
            if sqlite_option
                && self.peek_is_punct(Punctuation::Comma)?
                && (self.peek_nth_is_contextual_keyword(1, "WITHOUT")?
                    || self.peek_nth_is_contextual_keyword(1, "STRICT")?)
            {
                self.advance()?;
            }
        }
        let has_with = options
            .iter()
            .any(|option| matches!(option.kind, CreateTableOptionKind::With { .. }));
        let colocate_count = options
            .iter()
            .filter(|option| {
                matches!(
                    option.kind,
                    CreateTableOptionKind::ColocateWith { .. }
                        | CreateTableOptionKind::InColocationGroup { .. }
                )
            })
            .count();
        if colocate_count > 1 {
            return Err(self.unexpected("only one CREATE TABLE colocation clause"));
        }
        if options
            .iter()
            .any(|option| matches!(option.kind, CreateTableOptionKind::ColocateWith { .. }))
            && has_with
        {
            return Err(self.unexpected("no WITH storage options with COLOCATE WITH"));
        }
        if let Some(columns) = options.iter().find_map(|option| match &option.kind {
            CreateTableOptionKind::InColocationGroup { columns, .. } => Some(columns),
            _ => None,
        }) {
            if columns.is_empty() != has_with {
                return Err(self.unexpected(
                    "exactly one of ON (<columns>) or WITH storage options for IN COLOCATION GROUP",
                ));
            }
        }
        Ok(options)
    }

    /// Parse one SQLite bare keyword-style trailing table option — `WITHOUT ROWID` or
    /// `STRICT` — or `None` when the current token is neither (no input consumed). Each
    /// branch self-gates on its own feature: `WITHOUT ROWID` rides
    /// `without_rowid_table_option` and `STRICT` rides `strict_table_option` (both split out
    /// of the retired `sqlite_table_decorations` bundle), so the two options toggle independently.
    fn parse_optional_sqlite_table_option(
        &mut self,
    ) -> ParseResult<Option<CreateTableOption<D::Ext>>> {
        let start = self.current_span()?;
        if self
            .features()
            .create_table_clause_syntax
            .without_rowid_table_option
            && self.eat_contextual_keyword("WITHOUT")?
        {
            self.expect_contextual_keyword("ROWID")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CreateTableOption {
                kind: CreateTableOptionKind::WithoutRowid { meta },
                meta,
            }));
        }
        if self
            .features()
            .create_table_clause_syntax
            .strict_table_option
            && self.eat_contextual_keyword("STRICT")?
        {
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CreateTableOption {
                kind: CreateTableOptionKind::Strict { meta },
                meta,
            }));
        }
        Ok(None)
    }

    fn parse_create_table_option(&mut self) -> ParseResult<Option<CreateTableOption<D::Ext>>> {
        if self.features().statement_ddl_gates.colocation_groups
            && (self.peek_is_contextual_keyword("COLOCATE")?
                || self.peek_is_contextual_keyword("IN")?)
        {
            return Ok(Some(self.parse_colocation_table_option()?));
        }
        if self
            .features()
            .create_table_clause_syntax
            .storage_parameters
            && self.peek_is_keyword(Keyword::With)?
        {
            return Ok(Some(self.parse_storage_parameters_option()?));
        }
        if self.features().create_table_clause_syntax.on_commit
            && self.peek_is_keyword(Keyword::On)?
        {
            return Ok(Some(self.parse_on_commit_option()?));
        }
        if self.peek_is_contextual_keyword("TABLESPACE")? {
            return Ok(Some(self.parse_tablespace_option()?));
        }
        // The legacy PostgreSQL `WITHOUT OIDS` no-op trailing option (gated). Distinct from
        // SQLite's `WITHOUT ROWID` below: the two share the `WITHOUT` lead but a different second
        // word, and their gates (`without_oids` vs `without_rowid_table_option`) never overlap in
        // one dialect. The two-word lookahead keeps a bare `WITHOUT` (neither `OIDS` nor `ROWID`)
        // for the caller to reject.
        if self.features().create_table_clause_syntax.without_oids
            && self.peek_is_contextual_keyword("WITHOUT")?
            && self.peek_nth_is_contextual_keyword(1, "OIDS")?
        {
            let start = self.current_span()?;
            self.expect_contextual_keyword("WITHOUT")?;
            self.expect_contextual_keyword("OIDS")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(Some(CreateTableOption {
                kind: CreateTableOptionKind::WithoutOids { meta },
                meta,
            }));
        }
        // The SQLite bare keyword-style trailing options (`WITHOUT ROWID`, `STRICT`).
        // Enter when either constituent flag is on; `parse_optional_sqlite_table_option`
        // self-gates each branch (both `WITHOUT ROWID` and `STRICT` were split onto their
        // own flags off the retired `sqlite_table_decorations` bundle).
        if self
            .features()
            .create_table_clause_syntax
            .without_rowid_table_option
            || self
                .features()
                .create_table_clause_syntax
                .strict_table_option
        {
            if let Some(option) = self.parse_optional_sqlite_table_option()? {
                return Ok(Some(option));
            }
        }
        // MySQL trailing table options are an open `<name> [=] <value>` list with no
        // introducing keyword, so the option list runs until a token that cannot be a
        // name. `AS` is excluded because it introduces the CTAS body, not an option
        // (`CREATE TABLE t ENGINE = InnoDB AS SELECT ...`); the list otherwise ends at
        // `;`/end-of-input, both non-label tokens.
        //
        // A leading `ON` is the `ON COMMIT` clause opener: a dialect that *has* that clause
        // consumed it in the `on_commit` branch above, so an `ON` reaching here belongs to a
        // dialect whose `on_commit` knob is off (MySQL) and must NOT be swallowed as an open
        // `<name> <value>` table option — otherwise the ungated open list parses
        // `CREATE TABLE a (b INT) ON COMMIT PRESERVE ROWS` as the two bogus options
        // `ON = COMMIT` / `PRESERVE = ROWS` instead of the syntax error MySQL reports
        // (engine-measured-rejected on mysql:8). Wiring the exclusion to the knob keeps the
        // non-temp `CREATE TABLE` path honouring `on_commit` exactly as the temp-table path.
        if self.features().create_table_clause_syntax.table_options {
            if let Some(token) = self.peek()? {
                let is_on_commit_opener = token.kind == TokenKind::Keyword(Keyword::On)
                    && !self.features().create_table_clause_syntax.on_commit;
                if token.kind != TokenKind::Keyword(Keyword::As)
                    && !is_on_commit_opener
                    && self.token_can_be_label(token)
                {
                    return Ok(Some(self.parse_mysql_table_option()?));
                }
            }
        }
        Ok(None)
    }

    fn parse_colocation_table_option(&mut self) -> ParseResult<CreateTableOption<D::Ext>> {
        let start = self.current_span()?;
        let kind = if self.eat_contextual_keyword("COLOCATE")? {
            self.expect_keyword(Keyword::With)?;
            let table = self.parse_target_relation_name()?;
            self.expect_keyword(Keyword::On)?;
            let columns = self.parse_parenthesized_ident_list(
                "`(` to open the colocation key",
                "`)` to close the colocation key",
            )?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            CreateTableOptionKind::ColocateWith {
                table,
                columns,
                meta,
            }
        } else {
            self.expect_contextual_keyword("IN")?;
            self.expect_contextual_keyword("COLOCATION")?;
            self.expect_contextual_keyword("GROUP")?;
            let group = self.parse_ident()?;
            let columns = if self.eat_keyword(Keyword::On)? {
                self.parse_parenthesized_ident_list(
                    "`(` to open the colocation key",
                    "`)` to close the colocation key",
                )?
            } else {
                ThinVec::new()
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            CreateTableOptionKind::InColocationGroup {
                group,
                columns,
                meta,
            }
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(CreateTableOption { kind, meta })
    }

    /// Parse one MySQL trailing table option: `[DEFAULT] <name> [=] <value>`.
    ///
    /// Modelled as an open name/value pair, not a variant per option keyword —
    /// the MySQL option vocabulary is large and version-dependent, so one
    /// canonical shape round-trips an arbitrary list. The `DEFAULT` noise word MySQL
    /// accepts before `CHARSET`/`COLLATE` carries no meaning here and is normalized
    /// away; the `=` is optional (`ENGINE InnoDB` and `ENGINE = InnoDB` are the same).
    fn parse_mysql_table_option(&mut self) -> ParseResult<CreateTableOption<D::Ext>> {
        let start = self.current_span()?;
        self.eat_contextual_keyword("DEFAULT")?;
        let name = self.parse_as_alias_ident()?;
        if self.peek_is_op(Operator::Eq)? {
            self.advance()?;
        }
        let value = self.parse_table_option_value()?;
        let span = start.union(self.preceding_span());
        let option = TableOption {
            name,
            value,
            meta: self.make_meta(span),
        };
        Ok(CreateTableOption {
            kind: CreateTableOptionKind::KeyValue {
                option: Box::new(option),
                meta: self.make_meta(span),
            },
            meta: self.make_meta(span),
        })
    }

    /// Parse the value of a MySQL table option: a string (`COMMENT = '...'`), a number
    /// (`AUTO_INCREMENT = 100`), or a bareword/keyword (`ENGINE = InnoDB`). String and
    /// numeric values ride `meta.span` and materialise lazily.
    fn parse_table_option_value(&mut self) -> ParseResult<TableOptionValue> {
        let start = self.current_span()?;
        if let Some(token) = self.peek()? {
            // A string or numeric literal rides its span and materialises lazily; the
            // `kind` is all the parser commits to (ADR-0006).
            let kind = match token.kind {
                TokenKind::String => Some(LiteralKind::String),
                TokenKind::Number => Some(number_literal_kind(
                    self.span_text(token.span),
                    self.float_as_decimal_enabled(),
                )),
                _ => None,
            };
            if let Some(kind) = kind {
                let is_string = kind == LiteralKind::String;
                self.advance()?;
                let value = Literal {
                    kind,
                    meta: self.make_meta(token.span),
                };
                let meta = self.make_meta(start.union(self.preceding_span()));
                return Ok(if is_string {
                    TableOptionValue::String { value, meta }
                } else {
                    TableOptionValue::Number { value, meta }
                });
            }
        }
        let word = self.parse_as_alias_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(TableOptionValue::Word { word, meta })
    }

    fn parse_storage_parameters_option(&mut self) -> ParseResult<CreateTableOption<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::With)?;
        self.expect_punct(Punctuation::LParen, "`(` after `WITH`")?;
        let mut params = ThinVec::new();
        if !self.peek_is_punct(Punctuation::RParen)? {
            params.push(self.parse_storage_parameter()?);
            while self.eat_punct(Punctuation::Comma)? {
                params.push(self.parse_storage_parameter()?);
            }
        }
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close `WITH` storage parameters",
        )?;
        let span = start.union(self.preceding_span());
        let kind_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(CreateTableOption {
            kind: CreateTableOptionKind::With {
                params,
                meta: kind_meta,
            },
            meta,
        })
    }

    fn parse_storage_parameter(&mut self) -> ParseResult<TableStorageParameter<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_object_name()?;
        let value = if self.peek_is_op(Operator::Eq)? {
            self.advance()?;
            Some(self.parse_expr()?)
        } else {
            None
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(TableStorageParameter { name, value, meta })
    }

    fn parse_on_commit_option(&mut self) -> ParseResult<CreateTableOption<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::On)?;
        self.expect_contextual_keyword("COMMIT")?;
        let action = if self.eat_contextual_keyword("PRESERVE")? {
            self.expect_keyword(Keyword::Rows)?;
            OnCommitAction::PreserveRows
        } else if self.eat_contextual_keyword("DELETE")? {
            self.expect_keyword(Keyword::Rows)?;
            OnCommitAction::DeleteRows
        } else if self.eat_contextual_keyword("DROP")? {
            OnCommitAction::Drop
        } else {
            return Err(self.unexpected("`PRESERVE ROWS`, `DELETE ROWS`, or `DROP`"));
        };
        let span = start.union(self.preceding_span());
        let kind_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(CreateTableOption {
            kind: CreateTableOptionKind::OnCommit {
                action,
                meta: kind_meta,
            },
            meta,
        })
    }

    fn parse_tablespace_option(&mut self) -> ParseResult<CreateTableOption<D::Ext>> {
        let start = self.current_span()?;
        self.expect_contextual_keyword("TABLESPACE")?;
        let tablespace = self.parse_ident()?;
        let span = start.union(self.preceding_span());
        let kind_meta = self.make_meta(span);
        let meta = self.make_meta(span);
        Ok(CreateTableOption {
            kind: CreateTableOptionKind::Tablespace {
                tablespace,
                meta: kind_meta,
            },
            meta,
        })
    }

    fn parse_parenthesized_ident_list(
        &mut self,
        open_expected: &'static str,
        close_expected: &'static str,
    ) -> ParseResult<ThinVec<Ident>> {
        self.expect_punct(Punctuation::LParen, open_expected)?;
        let items = self.parse_comma_separated(Self::parse_ident)?;
        self.expect_punct(Punctuation::RParen, close_expected)?;
        Ok(items)
    }

    /// Parse a `PRIMARY KEY`/`UNIQUE` table-constraint column list as [`IndexColumn`]s —
    /// SQLite's "indexed-column" spelling in constraint position, `column-name [COLLATE
    /// <collation>] [ASC|DESC]` (`PRIMARY KEY(a COLLATE nocase)`, `UNIQUE('b' COLLATE nocase
    /// DESC)`).
    ///
    /// Each element is a bare column name, optionally a single-quoted string-literal identifier
    /// under [`IdentifierSyntax::string_literal_identifiers`](crate::ast::dialect::IdentifierSyntax)
    /// (SQLite, folded to [`QuoteStyle::Single`] so the quotes round-trip). The `COLLATE` /
    /// `ASC` / `DESC` decoration is admitted only under
    /// [`ConstraintSyntax::constraint_column_collate_order`](crate::ast::dialect::ConstraintSyntax);
    /// off elsewhere a bare name is the only accepted form and a trailing `COLLATE`/`ASC`/`DESC`
    /// is left unconsumed and rejects. SQLite prohibits general expressions and `NULLS
    /// FIRST`/`LAST` here (engine-measured), so — unlike [`parse_index_column`](Self::parse_index_column),
    /// which routes through `parse_expr` — the name is parsed as a plain identifier and
    /// `nulls_first` is never filled; the resulting [`IndexColumn::expr`] is an
    /// [`Expr::Column`] or, under `COLLATE`, an [`Expr::Collate`] wrapping it.
    fn parse_constraint_column_list(
        &mut self,
        open_expected: &'static str,
        close_expected: &'static str,
    ) -> ParseResult<ThinVec<IndexColumn<D::Ext>>> {
        self.expect_punct(Punctuation::LParen, open_expected)?;
        let items = self.parse_comma_separated(Self::parse_constraint_column)?;
        self.expect_punct(Punctuation::RParen, close_expected)?;
        Ok(items)
    }

    /// Parse one `PRIMARY KEY`/`UNIQUE` constraint column — see
    /// [`parse_constraint_column_list`](Self::parse_constraint_column_list).
    fn parse_constraint_column(&mut self) -> ParseResult<IndexColumn<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_column_ident_allowing_string_literal()?;
        let name_span = start.union(self.preceding_span());
        let mut expr = Expr::Column {
            name: ObjectName(thin_vec![name]),
            meta: self.make_meta(name_span),
        };

        let asc = if self
            .features()
            .constraint_syntax
            .constraint_column_collate_order
        {
            if self.eat_keyword(Keyword::Collate)? {
                let collation = self.parse_object_name()?;
                let span = start.union(self.preceding_span());
                expr = Expr::Collate {
                    collate: Box::new(CollateExpr {
                        expr,
                        collation,
                        meta: self.make_meta(span),
                    }),
                    meta: self.make_meta(span),
                };
            }
            if self.eat_keyword(Keyword::Asc)? {
                Some(true)
            } else if self.eat_keyword(Keyword::Desc)? {
                Some(false)
            } else {
                None
            }
        } else {
            None
        };

        let span = start.union(self.preceding_span());
        Ok(IndexColumn {
            expr,
            asc,
            // SQLite rejects `NULLS FIRST`/`LAST` in constraint position — never filled here.
            nulls_first: None,
            meta: self.make_meta(span),
        })
    }

    fn parse_parenthesized_expr(
        &mut self,
        open_expected: &'static str,
        close_expected: &'static str,
    ) -> ParseResult<crate::ast::Expr<D::Ext>> {
        self.expect_punct(Punctuation::LParen, open_expected)?;
        let expr = self.parse_expr()?;
        self.expect_punct(Punctuation::RParen, close_expected)?;
        Ok(expr)
    }

    fn peek_starts_table_constraint(&mut self) -> ParseResult<bool> {
        Ok((self.peek_is_contextual_keyword("PRIMARY")?
            && self.peek_nth_is_contextual_keyword(1, "KEY")?)
            || self.peek_is_contextual_keyword("UNIQUE")?
            || self.peek_is_contextual_keyword("CHECK")?
            || (self.features().constraint_syntax.exclusion_constraints
                && self.peek_is_contextual_keyword("EXCLUDE")?)
            || self.peek_is_contextual_keyword("FOREIGN")?)
    }

    /// Whether an `ALTER TABLE … ADD` tail is a `CHECK` table constraint — bare
    /// `CHECK (…)` or `CONSTRAINT <name> CHECK (…)`. This is the only table-constraint
    /// kind SQLite's lenient `ADD` accepts (its `PRIMARY KEY`/`UNIQUE`/`FOREIGN KEY` are
    /// rejected), so the [`alter_table_extended`](crate::ast::dialect::IndexAlterSyntax)
    /// -off path admits exactly this shape.
    fn peek_starts_check_constraint(&mut self) -> ParseResult<bool> {
        Ok(self.peek_is_contextual_keyword("CHECK")?
            || (self.peek_is_contextual_keyword("CONSTRAINT")?
                && self.peek_nth_is_contextual_keyword(2, "CHECK")?))
    }

    fn peek_ends_table_element(&mut self) -> ParseResult<bool> {
        Ok(self.is_eof()?
            || self.peek_is_punct(Punctuation::Comma)?
            || self.peek_is_punct(Punctuation::RParen)?)
    }

    /// Whether the cursor sits at a complete `CONSTRAINT <name>` with nothing else before the
    /// table-element terminator (`,` / `)` / EOF) — SQLite's trailing bare constraint. Used by
    /// the table-element loop to admit the elided comma this specific case engine-measured
    /// accepts (`UNIQUE(a) CONSTRAINT c`, `CONSTRAINT a UNIQUE(x) CONSTRAINT b`) without
    /// over-generalizing to any two table constraints (SQLite's `UNIQUE(a) UNIQUE(b)` comma
    /// elision is a broader, unmeasured surface left for its own ticket). `CONSTRAINT` at
    /// position 0, the name token at 1, so the terminator is checked at 2 — nothing is
    /// consumed, so a `false` result leaves the cursor for the normal comma-required path.
    fn peek_bare_trailing_table_constraint(&mut self) -> ParseResult<bool> {
        if !self.features().constraint_syntax.bare_constraint_name
            || !self.peek_is_contextual_keyword("CONSTRAINT")?
        {
            return Ok(false);
        }
        Ok(self.peek_nth(2)?.is_none()
            || self.peek_nth_is_punct(2, Punctuation::Comma)?
            || self.peek_nth_is_punct(2, Punctuation::RParen)?)
    }
}

/// Walk an expression tree for any subquery-shaped node (`Subquery` / `Exists` /
/// `InSubquery` / array-subquery). Used to enforce
/// [`ConstraintSyntax::check_constraint_subqueries`](crate::ast::dialect::ConstraintSyntax).
fn expr_contains_subquery<X: crate::ast::Extension>(expr: &Expr<X>) -> bool {
    use crate::ast::generated::visit::{self, Visit};
    struct Finder(bool);
    impl<'ast, X: crate::ast::Extension> Visit<'ast, X> for Finder {
        fn visit_expr(&mut self, node: &'ast Expr<X>) {
            use crate::ast::Expr::*;
            match node {
                Subquery { .. } | Exists { .. } | InSubquery { .. } => self.0 = true,
                _ => visit::walk_expr(self, node),
            }
        }
        fn visit_array_expr(&mut self, node: &'ast crate::ast::ArrayExpr<X>) {
            if matches!(node, crate::ast::ArrayExpr::Subquery { .. }) {
                self.0 = true;
            } else {
                visit::walk_array_expr(self, node);
            }
        }
    }
    let mut f = Finder(false);
    f.visit_expr(expr);
    f.0
}

#[cfg(test)]
mod tests {
    use crate::ast::dialect::{
        ColumnDefinitionSyntax, CreateTableClauseSyntax, FeatureDelta, FeatureSet,
        IndexAlterSyntax, StatementDdlGates,
    };
    use crate::ast::{
        AlterColumnAction, AlterTable, AlterTableAction, AutoIncrementSpelling, BinaryOperator,
        ColumnOption, CommentTarget, ConflictResolution, CreateIndex, CreateMacro, CreateSchema,
        CreateSecret, CreateSequence, CreateTable, CreateTableBody, CreateTableOptionKind,
        CreateTrigger, CreateType, CreateTypeDefinition, CreateView, CreateVirtualTable, DataType,
        DatabaseKeyword, DropBehavior, DropDatabase, DropIndexOnTable, DropObjectKind,
        DropSecretStmt, DropStatement, EventOnCompletion, EventSchedule, EventStatus, Expr,
        ForeignKeyMatch, ForeignKeyRef, FunctionBody, FunctionOption, FunctionParamDefaultSpelling,
        FunctionParamMode, GeneratedColumn, GeneratedColumnSpelling, GeneratedColumnStorage,
        IdentityGeneration, IdentityOption, IndexAlgorithm, IndexLock, IndexLockAlgorithmOption,
        IntegerTypeName, IntervalFields, LanguageName, LiteralKind, MacroBody, MacroSpelling,
        NoExt, OnCommitAction, QuoteStyle, ReferentialAction, ReplicaSpelling, Resolver as _,
        RoutineKind, SecretPersistence, SetExpr, Statement, TableConstraint, TableElement,
        TableOptionValue, TemporaryTableKind, TriggerEvent, TriggerOrder, TriggerTiming,
        ViewAlgorithm, ViewCheckOption,
    };
    use crate::ast::{AlterView, Definer, SqlSecurityContext};
    use crate::dialect::{Ansi, MySql, Sqlite};
    use crate::parser::{FeatureDialect, Parsed, TestDialect, parse_with};
    use crate::render::Renderer;

    /// A MySQL parse + render dialect, so the gated `CREATE DATABASE IF NOT EXISTS` and
    /// generated-column shorthand round-trip through the MySQL target.
    const MYSQL_RENDER: FeatureDialect = FeatureDialect {
        features: &FeatureSet::MYSQL,
    };

    fn create_database_of(parsed: &Parsed) -> &crate::ast::CreateDatabase {
        let Statement::CreateDatabase { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE DATABASE statement");
        };
        create
    }

    /// Extract the [`GeneratedColumn`] at table-element `index`, panicking on any other
    /// element or column option.
    fn generated_column_of(parsed: &Parsed, index: usize) -> &GeneratedColumn<NoExt> {
        let CreateTableBody::Definition { elements, .. } = &create_table_of(parsed).body else {
            panic!("expected a table definition body");
        };
        let TableElement::Column { column, .. } = &elements[index] else {
            panic!("expected a column element");
        };
        let ColumnOption::Generated { generated, .. } = &column.constraints[0].option else {
            panic!("expected a generated column option");
        };
        generated
    }

    #[test]
    fn create_database_if_not_exists_parses_and_round_trips() {
        let sql = "CREATE DATABASE IF NOT EXISTS d";
        let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
            .expect("CREATE DATABASE IF NOT EXISTS parses");
        assert!(create_database_of(&parsed).if_not_exists);
        assert_eq!(
            Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .expect("renders"),
            sql,
        );

        // The bare form leaves the flag off and still parses under MySQL.
        let bare = parse_with("CREATE DATABASE d", crate::ParseConfig::new(MySql))
            .expect("bare CREATE DATABASE parses");
        assert!(!create_database_of(&bare).if_not_exists);
    }

    fn drop_database_of(parsed: &Parsed) -> &DropDatabase {
        let Statement::DropDatabase { drop, .. } = &parsed.statements()[0] else {
            panic!("expected a DROP DATABASE statement");
        };
        drop
    }

    fn drop_index_of(parsed: &Parsed) -> &DropIndexOnTable {
        let Statement::DropIndex { drop, .. } = &parsed.statements()[0] else {
            panic!("expected a DROP INDEX statement");
        };
        drop
    }

    #[test]
    fn drop_database_and_schema_synonyms_parse_and_round_trip() {
        // Both keyword spellings, the `IF EXISTS` guard, and a backtick-quoted name each
        // round-trip byte-exact; the spelling and guard are captured distinctly. Every form is
        // engine-recognized on MySQL 8.4.10 (m3 PREPARE probe).
        for (sql, spelling, if_exists) in [
            ("DROP DATABASE d", DatabaseKeyword::Database, false),
            ("DROP SCHEMA d", DatabaseKeyword::Schema, false),
            ("DROP DATABASE IF EXISTS d", DatabaseKeyword::Database, true),
            ("DROP SCHEMA IF EXISTS d", DatabaseKeyword::Schema, true),
            (
                "DROP DATABASE `weird name`",
                DatabaseKeyword::Database,
                false,
            ),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let drop = drop_database_of(&parsed);
            assert_eq!(drop.spelling, spelling, "{sql:?}");
            assert_eq!(drop.if_exists, if_exists, "{sql:?}");
            assert_eq!(
                Renderer::new(MYSQL_RENDER)
                    .render_parsed(&parsed)
                    .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}")),
                sql,
                "round-trip",
            );
        }
    }

    #[test]
    fn drop_database_rejects_list_cascade_and_dotted_name() {
        // MySQL names exactly one unqualified database with no drop behaviour — server-measured
        // rejects (`ER_PARSE_ERROR`) on mysql:8.
        for sql in [
            "DROP DATABASE a, b",
            "DROP SCHEMA a, b",
            "DROP DATABASE a CASCADE",
            "DROP DATABASE a RESTRICT",
            "DROP DATABASE db.x",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql)).expect_err(sql);
        }
    }

    #[test]
    fn drop_index_on_table_parses_options_and_round_trips() {
        // The mandatory `ON <table>` (dotted table allowed), the `ALGORITHM`/`LOCK` hints with
        // and without `=`, and both option orderings each round-trip byte-exact. Every form is
        // engine-recognized on MySQL 8.4.10 (m3 PREPARE probe).
        for sql in [
            "DROP INDEX i ON t",
            "DROP INDEX i ON db.t",
            "DROP INDEX i ON t ALGORITHM = DEFAULT",
            "DROP INDEX i ON t ALGORITHM = INPLACE",
            "DROP INDEX i ON t ALGORITHM INSTANT",
            "DROP INDEX i ON t ALGORITHM = COPY",
            "DROP INDEX i ON t LOCK = NONE",
            "DROP INDEX i ON t LOCK SHARED",
            "DROP INDEX i ON t LOCK = EXCLUSIVE",
            "DROP INDEX i ON t LOCK = DEFAULT",
            "DROP INDEX i ON t ALGORITHM = COPY LOCK = SHARED",
            "DROP INDEX i ON t LOCK NONE ALGORITHM DEFAULT",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert_eq!(
                Renderer::new(MYSQL_RENDER)
                    .render_parsed(&parsed)
                    .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}")),
                sql,
                "round-trip",
            );
        }
    }

    #[test]
    fn drop_index_captures_name_table_and_ordered_options() {
        let parsed = parse_with(
            "DROP INDEX i ON db.t LOCK = NONE ALGORITHM INPLACE",
            crate::ParseConfig::new(MySql),
        )
        .expect("parses");
        let drop = drop_index_of(&parsed);
        assert_eq!(parsed.resolver().resolve(drop.name.sym), "i");
        assert_eq!(drop.table.0.len(), 2, "dotted table keeps both parts");
        // Source order (LOCK before ALGORITHM) and the `=` presence are both preserved.
        assert_eq!(
            drop.options.as_slice(),
            [
                IndexLockAlgorithmOption::Lock {
                    equals: true,
                    value: IndexLock::None,
                },
                IndexLockAlgorithmOption::Algorithm {
                    equals: false,
                    value: IndexAlgorithm::Inplace,
                },
            ],
        );
    }

    #[test]
    fn drop_index_rejects_missing_on_dotted_name_behaviour_and_duplicates() {
        // Server-measured rejects (`ER_PARSE_ERROR`) on mysql:8: the `ON` is mandatory, the
        // index name is a bare identifier, there is no `CASCADE`/`RESTRICT`, and each of
        // `ALGORITHM`/`LOCK` appears at most once. (`ALGORITHM/LOCK = <bad value>` is a *binding*
        // reject the grammar accepts, so it is not tested here — only the modelled value set is
        // admitted, which the parser rejects one stage earlier.)
        for sql in [
            "DROP INDEX i",
            "DROP INDEX i.j ON t",
            "DROP INDEX i ON t RESTRICT",
            "DROP INDEX i ON t CASCADE",
            "DROP INDEX i ON t ALGORITHM = COPY ALGORITHM = INPLACE",
            "DROP INDEX i ON t LOCK = NONE LOCK = SHARED",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql)).expect_err(sql);
        }
    }

    #[test]
    fn non_mysql_dialects_keep_the_shared_drop_index_and_schema_grammar() {
        // The MySQL `ON <table>` / single-name-`DATABASE` intercepts are gated off elsewhere:
        // ANSI keeps the shared name-list `DROP INDEX`/`DROP SCHEMA` and never grew a `DROP
        // DATABASE` drop, so the MySQL-only forms reject and the shared forms still parse.
        parse_with("DROP INDEX i ON t", crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no DROP INDEX … ON <table>");
        parse_with("DROP DATABASE d", crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no DROP DATABASE");
        parse_with("DROP INDEX i, j", crate::ParseConfig::new(Ansi))
            .expect("ANSI keeps the shared name-list DROP INDEX");
        parse_with("DROP SCHEMA s CASCADE", crate::ParseConfig::new(Ansi))
            .expect("ANSI keeps the shared name-list DROP SCHEMA");
    }

    #[test]
    fn ansi_and_postgres_reject_create_database_if_not_exists() {
        // The guard is gated by `create_database_if_not_exists` — off in ANSI/PostgreSQL,
        // which have no `CREATE DATABASE IF NOT EXISTS` — so `IF` is read as the database
        // name and the trailing `NOT EXISTS d` is leftover input -> reject.
        let sql = "CREATE DATABASE IF NOT EXISTS d";
        parse_with(sql, crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no CREATE DATABASE IF NOT EXISTS");
        parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
            .expect_err("PostgreSQL has no CREATE DATABASE IF NOT EXISTS");
    }

    #[test]
    fn generated_column_shorthand_parses_and_round_trips() {
        // MySQL keywordless shorthand `AS (<expr>) STORED`, tagged `Shorthand` so the
        // short spelling round-trips verbatim rather than growing `GENERATED ALWAYS`.
        // `INTEGER` (not `INT`) matches the MySQL target's canonical type spelling, so
        // the round-trip isolates the shorthand rather than tripping on type rendering.
        let sql = "CREATE TABLE t (a INTEGER, b INTEGER AS (a + 1) STORED)";
        let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
            .expect("shorthand generated column parses");
        let generated = generated_column_of(&parsed, 1);
        assert_eq!(generated.spelling, GeneratedColumnSpelling::Shorthand);
        assert_eq!(generated.storage, Some(GeneratedColumnStorage::Stored));
        assert_eq!(
            Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .expect("renders"),
            sql,
        );

        // The standard keyworded form still tags `GeneratedAlways`.
        let standard = parse_with(
            "CREATE TABLE t (a INT, b INT GENERATED ALWAYS AS (a + 1) STORED)",
            crate::ParseConfig::new(MySql),
        )
        .expect("standard generated column parses");
        assert_eq!(
            generated_column_of(&standard, 1).spelling,
            GeneratedColumnSpelling::GeneratedAlways,
        );
    }

    #[test]
    fn ansi_and_postgres_reject_generated_column_shorthand() {
        // The shorthand is gated by `generated_column_shorthand` — off in ANSI/PostgreSQL,
        // which require `GENERATED ALWAYS AS` — so the bare `AS (` after the column type
        // is not a column option and is leftover input in the element list -> reject.
        let sql = "CREATE TABLE t (a INT, b INT AS (a + 1) STORED)";
        parse_with(sql, crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no generated-column shorthand");
        parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
            .expect_err("PostgreSQL has no generated-column shorthand");
    }

    /// A SQLite parse + render dialect, so the gated `CREATE TABLE` decorations parse
    /// and round-trip through the SQLite target.
    const SQLITE_RENDER: FeatureDialect = FeatureDialect {
        features: &FeatureSet::SQLITE,
    };

    /// Assert `sql` parses under SQLite and renders back to itself.
    fn assert_sqlite_round_trips(sql: &str) {
        let parsed = parse_with(sql, crate::ParseConfig::new(SQLITE_RENDER))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let rendered = Renderer::new(SQLITE_RENDER)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
        assert_eq!(rendered, sql, "round-trip");
    }

    #[test]
    fn sqlite_create_table_decorations_parse_and_round_trip() {
        // Each SQLite `CREATE TABLE` decoration parses under the SQLite preset and renders
        // back verbatim: the `WITHOUT ROWID` / `STRICT` trailing options (and their
        // comma-separated combination), a typeless column, `AUTOINCREMENT`, a column
        // `ON CONFLICT`, a column `COLLATE`, an inline-`PRIMARY KEY` `ASC`/`DESC`, and the
        // keywordless generated-column shorthand (the flipped `generated_column_shorthand`).
        for sql in [
            "CREATE TABLE t (a INTEGER PRIMARY KEY, b TEXT) WITHOUT ROWID",
            "CREATE TABLE t (a INTEGER, b TEXT) STRICT",
            "CREATE TABLE t (a INTEGER) STRICT, WITHOUT ROWID",
            "CREATE TABLE t (a, b, c)",
            "CREATE TABLE t (a INTEGER PRIMARY KEY AUTOINCREMENT)",
            "CREATE TABLE t (a INTEGER UNIQUE ON CONFLICT REPLACE)",
            "CREATE TABLE t (a INTEGER NOT NULL ON CONFLICT ABORT)",
            "CREATE TABLE t (a TEXT COLLATE NOCASE)",
            "CREATE TABLE t (a INTEGER PRIMARY KEY ASC)",
            "CREATE TABLE t (a INTEGER PRIMARY KEY DESC)",
            "CREATE TABLE t (a INTEGER, b AS (a * 2))",
        ] {
            assert_sqlite_round_trips(sql);
        }
    }

    #[test]
    fn sqlite_accepts_string_literal_constraint_columns_and_round_trips() {
        // SQLite's string-literal identifier misfeature (`string_literal_identifiers`): a
        // single-quoted `'name'` is read as a column name in the `PRIMARY KEY`/`UNIQUE`
        // table-constraint column list (engine-verified parse-accept on rusqlite 3.53.2:
        // `CREATE TABLE t(a, b, PRIMARY KEY('a'))`). The folded name records
        // `QuoteStyle::Single`, so the quotes render back verbatim and round-trip.
        for sql in [
            "CREATE TABLE t (a, b, PRIMARY KEY ('a'))",
            "CREATE TABLE t (a, b, UNIQUE ('b'))",
            "CREATE TABLE t (a, b, PRIMARY KEY ('a'), UNIQUE ('b'))",
            // A mix of bare and string-quoted names in one list.
            "CREATE TABLE t (a, b, PRIMARY KEY (a, 'b'))",
        ] {
            assert_sqlite_round_trips(sql);
        }
        // PostgreSQL (flag off) syntax-rejects a string in the constraint column list.
        parse_with(
            "CREATE TABLE t (a, b, PRIMARY KEY ('a'))",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect_err("PostgreSQL rejects a string literal as a constraint column name");
    }

    #[test]
    fn sqlite_constraint_column_collate_and_order_round_trip() {
        // SQLite's indexed-column spelling in PRIMARY KEY / UNIQUE constraint position:
        // `column-name [COLLATE <collation>] [ASC|DESC]` (engine-verified parse-accept on
        // SQLite; the `constraint_column_collate_order` gate). Exprs and NULLS FIRST/LAST
        // reject — covered by the negative test below.
        for sql in [
            "CREATE TABLE t (a, b, PRIMARY KEY (a COLLATE nocase))",
            "CREATE TABLE t (a, b, UNIQUE (a COLLATE nocase DESC))",
            "CREATE TABLE t (a, b, PRIMARY KEY (a COLLATE nocase ASC, b DESC))",
            "CREATE TABLE t (a, b, UNIQUE (a ASC, b))",
            // Composition with the string-literal identifier spelling (the deferred
            // index3.test statement): a single-quoted column name carrying COLLATE + DESC.
            "CREATE TABLE t (a, b, UNIQUE ('b' COLLATE nocase DESC))",
        ] {
            assert_sqlite_round_trips(sql);
        }
    }

    #[test]
    fn sqlite_constraint_column_collate_order_shape() {
        // The composed case — string-literal column name + COLLATE + DESC — builds an
        // `IndexColumn` whose `expr` is an `Expr::Collate` wrapping the single-quoted
        // `Expr::Column`, `asc = Some(false)`, and `nulls_first = None`.
        let parsed = parse_with(
            "CREATE TABLE t (a, b, UNIQUE ('b' COLLATE nocase DESC))",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("parses under SQLite");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a table definition");
        };
        let TableElement::Constraint { constraint, .. } = elements.last().expect("a constraint")
        else {
            panic!("expected a table constraint element");
        };
        let TableConstraint::Unique { columns, .. } = &constraint.constraint else {
            panic!("expected a UNIQUE constraint");
        };
        assert_eq!(columns.len(), 1);
        let column = &columns[0];
        assert_eq!(column.asc, Some(false));
        assert_eq!(column.nulls_first, None);
        let Expr::Collate { collate, .. } = &column.expr else {
            panic!("expected a COLLATE-decorated key column");
        };
        assert_eq!(
            parsed.resolver().resolve(collate.collation.0[0].sym),
            "nocase"
        );
        let Expr::Column { name, .. } = &collate.expr else {
            panic!("expected the collated key to be a column");
        };
        assert_eq!(name.0[0].quote, QuoteStyle::Single);
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "b");
    }

    #[test]
    fn constraint_column_collate_order_gated_and_sqlite_rejects_expr_and_nulls() {
        use crate::dialect::{DuckDb, Postgres};
        // Gate off: PostgreSQL and DuckDB reject COLLATE / ASC / DESC in constraint position
        // (engine-measured — the decoration lives in their CREATE INDEX grammar, not here).
        for sql in [
            "CREATE TABLE t (a INT, b INT, PRIMARY KEY (a COLLATE \"C\"))",
            "CREATE TABLE t (a INT, b INT, UNIQUE (a DESC))",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err("PostgreSQL rejects COLLATE/ordering in a constraint column list");
            parse_with(sql, crate::ParseConfig::new(DuckDb))
                .expect_err("DuckDB rejects COLLATE/ordering in a constraint column list");
        }
        // SQLite prohibits a general expression and NULLS FIRST/LAST here (engine-measured):
        // the widened parser stays column-name + COLLATE + ASC/DESC, so these still reject —
        // the over-acceptance guard for the flipped gate.
        for sql in [
            "CREATE TABLE t (a, b, UNIQUE (a + b))",
            "CREATE TABLE t (a, b, PRIMARY KEY (lower(a)))",
            "CREATE TABLE t (a, b, PRIMARY KEY (a COLLATE nocase NULLS FIRST))",
        ] {
            parse_with(sql, crate::ParseConfig::new(SQLITE_RENDER))
                .expect_err("SQLite rejects exprs / NULLS ordering in a constraint column list");
        }
    }

    fn create_virtual_table_of(parsed: &Parsed) -> &CreateVirtualTable {
        let [Statement::CreateVirtualTable { create, .. }] = parsed.statements() else {
            panic!(
                "expected one CREATE VIRTUAL TABLE statement, got {:?}",
                parsed.statements(),
            );
        };
        create
    }

    #[test]
    fn create_virtual_table_parses_all_forms_and_round_trips() {
        // Every form is a bundled-SQLite parse-accept (engine-verified via rusqlite 3.53.2;
        // module resolution is deferred to execution, so the oracle boundary here is the
        // SQLite parser's opaque `vtabarglist`). The module owns the argument grammar, so the
        // parenthesized text is captured verbatim and re-emitted with canonical `, ` spacing —
        // which matches these inputs. Covered: bare `USING m` (no parens), the `fts5`/`rtree`
        // real modules, `IF NOT EXISTS`, a schema-qualified name, a `tokenize = '…'` option
        // with an embedded quoted string, and a nested-parenthesis argument.
        for sql in [
            "CREATE VIRTUAL TABLE t USING mymod",
            "CREATE VIRTUAL TABLE docs USING fts5(title, body)",
            "CREATE VIRTUAL TABLE t USING rtree(id, minX, maxX, minY, maxY)",
            "CREATE VIRTUAL TABLE IF NOT EXISTS docs USING fts5(content)",
            "CREATE VIRTUAL TABLE main.docs USING fts5(title, body)",
            "CREATE VIRTUAL TABLE t USING fts5(content, tokenize = 'porter unicode61')",
            "CREATE VIRTUAL TABLE t USING mymod(a, b(c, d), e)",
        ] {
            assert_sqlite_round_trips(sql);
        }
    }

    #[test]
    fn create_virtual_table_arg_list_shape() {
        // The module name is an ordinary identifier; the arguments are opaque verbatim slices
        // split only on the top-level commas.
        let parsed = parse_with(
            "CREATE VIRTUAL TABLE docs USING fts5(title, tokenize = 'porter unicode61')",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("virtual table parses under SQLite");
        let create = create_virtual_table_of(&parsed);
        assert!(!create.if_not_exists);
        assert_eq!(parsed.resolver().resolve(create.name.0[0].sym), "docs");
        assert_eq!(parsed.resolver().resolve(create.module.sym), "fts5");
        let args = create.args.as_ref().expect("parenthesized argument list");
        assert_eq!(args.len(), 2);
        assert_eq!(parsed.resolver().resolve(args[0].text), "title");
        // The second argument is captured verbatim, embedded spacing and quotes preserved.
        assert_eq!(
            parsed.resolver().resolve(args[1].text),
            "tokenize = 'porter unicode61'",
        );
    }

    #[test]
    fn create_virtual_table_no_parens_versus_empty_parens() {
        // `USING m` (no parens) and `USING m ()` (empty parens) are distinct surfaces that both
        // parse; the `Option` on `args` keeps them apart so each round-trips to itself.
        let bare = create_virtual_table_of(
            &parse_with(
                "CREATE VIRTUAL TABLE t USING mymod",
                crate::ParseConfig::new(SQLITE_RENDER),
            )
            .expect("bare parses"),
        )
        .args
        .is_none();
        assert!(bare, "no-parens form has args: None");

        let empty = parse_with(
            "CREATE VIRTUAL TABLE t USING mymod()",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("empty parens parse");
        let empty_args = create_virtual_table_of(&empty)
            .args
            .as_ref()
            .expect("empty parens form has args: Some");
        assert!(
            empty_args.is_empty(),
            "empty parens form has zero arguments"
        );
        assert_eq!(
            Renderer::new(SQLITE_RENDER)
                .render_parsed(&empty)
                .expect("renders"),
            "CREATE VIRTUAL TABLE t USING mymod()",
        );
    }

    #[test]
    fn create_virtual_table_permits_empty_arguments() {
        // SQLite's `vtabarg` may be empty, so a doubled/leading/trailing comma is a legal empty
        // member (engine-verified accept). The parser preserves the arity — three slices, the
        // middle empty — rather than rejecting.
        let parsed = parse_with(
            "CREATE VIRTUAL TABLE t USING mymod(a,,b)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("empty middle argument parses");
        let args = create_virtual_table_of(&parsed)
            .args
            .as_ref()
            .expect("argument list");
        assert_eq!(args.len(), 3);
        assert_eq!(parsed.resolver().resolve(args[1].text), "");
    }

    #[test]
    fn create_virtual_table_is_gated_and_reject_bounded() {
        use crate::dialect::{DuckDb, Postgres, Sqlite};
        let sql = "CREATE VIRTUAL TABLE t USING fts5(a)";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_ok(),
            "SQLite accepts {sql:?}"
        );
        // Whole-statement gate off in every non-SQLite preset: `VIRTUAL` falls through to the
        // `CREATE TABLE` expectation and surfaces as an unknown statement.
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI has no virtual tables");
        parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect_err("PostgreSQL has no virtual tables");
        parse_with(sql, crate::ParseConfig::new(MySql)).expect_err("MySQL has no virtual tables");
        parse_with(sql, crate::ParseConfig::new(DuckDb)).expect_err("DuckDB has no virtual tables");
        // A flipped-off SQLite base rejects too — the flag, not the dialect, drives acceptance.
        const SQLITE_NO_VTAB: FeatureSet =
            FeatureSet::SQLITE.with(FeatureDelta::EMPTY.statement_ddl_gates(StatementDdlGates {
                create_virtual_table: false,
                ..StatementDdlGates::SQLITE
            }));
        const SQLITE_NO_VTAB_DIALECT: FeatureDialect = FeatureDialect {
            features: &SQLITE_NO_VTAB,
        };
        parse_with(sql, crate::ParseConfig::new(SQLITE_NO_VTAB_DIALECT))
            .expect_err("the gate off must reject");

        // SQLite reject boundaries (all engine-verified rejects on rusqlite 3.53.2): no `TEMP`
        // virtual table, a three-part name, unbalanced parens, and a missing module name.
        for reject in [
            "CREATE TEMP VIRTUAL TABLE t USING mymod(a)",
            "CREATE VIRTUAL TABLE t.s.x USING mymod(a)",
            "CREATE VIRTUAL TABLE t USING mymod(a))",
            "CREATE VIRTUAL TABLE t USING mymod((a)",
            "CREATE VIRTUAL TABLE t USING",
        ] {
            parse_with(reject, crate::ParseConfig::new(Sqlite))
                .expect_err(&format!("SQLite rejects {reject:?}"));
        }
    }

    fn create_sequence_of(parsed: &Parsed) -> &CreateSequence<NoExt> {
        let [Statement::CreateSequence { create, .. }] = parsed.statements() else {
            panic!(
                "expected one CREATE SEQUENCE statement, got {:?}",
                parsed.statements(),
            );
        };
        create
    }

    #[test]
    fn create_sequence_parses_all_forms_and_round_trips() {
        // Every form is a parse-accept on BOTH engines (in-process libduckdb 1.10504.0 +
        // pg_query 6.1.1, engine-probed): the shared SQL:2003 T176 option core in the
        // canonical `START WITH`/`INCREMENT BY` spelling, `IF NOT EXISTS`, `TEMP`, a
        // schema-qualified name, `NO MINVALUE`/`NO MAXVALUE`/`NO CYCLE`, and an out-of-order
        // option run (both parsers accept the options order-free). Rendered back verbatim
        // under both the PostgreSQL and DuckDB targets.
        for sql in [
            "CREATE SEQUENCE s",
            "CREATE SEQUENCE IF NOT EXISTS s",
            "CREATE SEQUENCE myschema.s",
            "CREATE SEQUENCE s START WITH 3",
            "CREATE SEQUENCE s INCREMENT BY 2",
            "CREATE SEQUENCE s MINVALUE 1 MAXVALUE 10",
            "CREATE SEQUENCE s NO MINVALUE NO MAXVALUE NO CYCLE",
            "CREATE SEQUENCE s START WITH 1 INCREMENT BY 2 MINVALUE 1 MAXVALUE 10 CYCLE",
            "CREATE SEQUENCE s MAXVALUE 10 MINVALUE 1 INCREMENT BY 1",
        ] {
            let pg = parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            assert_eq!(
                Renderer::new(PG_DIALECT)
                    .render_parsed(&pg)
                    .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}")),
                sql,
                "PostgreSQL round-trip",
            );
            assert_duckdb_round_trips(sql);
        }
        // `TEMP`/`TEMPORARY` sequences round-trip too, on the preset that carries that spelling.
        assert_duckdb_round_trips("CREATE TEMPORARY SEQUENCE s");
    }

    #[test]
    fn create_sequence_option_shape() {
        // The name may be schema-qualified; the options are the shared identity/sequence
        // option vocabulary, parsed in written order.
        let parsed = parse_with(
            "CREATE SEQUENCE myschema.s START WITH 3 INCREMENT BY 2 NO MAXVALUE CYCLE",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("sequence parses under PostgreSQL");
        let create = create_sequence_of(&parsed);
        assert!(!create.if_not_exists);
        assert!(create.temporary.is_none());
        assert_eq!(create.name.0.len(), 2);
        assert_eq!(create.options.len(), 4);
        assert!(matches!(
            &create.options[0],
            IdentityOption::StartWith { .. }
        ));
        assert!(matches!(
            &create.options[1],
            IdentityOption::IncrementBy { .. }
        ));
        assert!(matches!(
            &create.options[2],
            IdentityOption::MaxValue { value: None, .. }
        ));
        assert!(matches!(
            &create.options[3],
            IdentityOption::Cycle { cycle: true, .. }
        ));
    }

    #[test]
    fn create_sequence_is_gated_and_reject_bounded() {
        use crate::dialect::{DuckDb, Postgres, Sqlite};
        let sql = "CREATE SEQUENCE s START WITH 1";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_ok(),
            "PostgreSQL accepts {sql:?}"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
            "DuckDB accepts {sql:?}"
        );
        // Whole-statement gate off in the dialects without sequences: `SEQUENCE` falls through
        // to the `CREATE TABLE` expectation and surfaces as an unknown statement.
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI has no sequences");
        parse_with(sql, crate::ParseConfig::new(MySql)).expect_err("MySQL has no sequences");
        parse_with(sql, crate::ParseConfig::new(Sqlite)).expect_err("SQLite has no sequences");
        // A flipped-off PostgreSQL base rejects too — the flag, not the dialect, drives it.
        const PG_NO_SEQ: FeatureSet =
            FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.statement_ddl_gates(StatementDdlGates {
                create_sequence: false,
                ..StatementDdlGates::POSTGRES
            }));
        const PG_NO_SEQ_DIALECT: FeatureDialect = FeatureDialect {
            features: &PG_NO_SEQ,
        };
        parse_with(sql, crate::ParseConfig::new(PG_NO_SEQ_DIALECT))
            .expect_err("the gate off must reject");

        // Zero new over-acceptance: DuckDB's `CREATE SEQUENCE` grammar rejects the PostgreSQL
        // extended tails, and the modelled shape never admits them under DuckDB (engine-probed
        // rejects), so they stay clean parse errors.
        for reject in [
            "CREATE SEQUENCE s CACHE 10",
            "CREATE SEQUENCE s AS integer",
            "CREATE SEQUENCE s OWNED BY t.c",
            "CREATE OR REPLACE SEQUENCE s",
        ] {
            parse_with(reject, crate::ParseConfig::new(DuckDb))
                .expect_err(&format!("DuckDB rejects {reject:?}"));
        }
    }

    #[test]
    fn drop_sequence_parses_and_is_gated() {
        // `DROP SEQUENCE` rides the same `create_sequence` flag and the shared `DropStatement`
        // grammar (`IF EXISTS`, comma list, `CASCADE`/`RESTRICT`).
        let parsed = parse_with(
            "DROP SEQUENCE IF EXISTS s, t CASCADE",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("DROP SEQUENCE parses under PostgreSQL");
        let [Statement::Drop { drop, .. }] = parsed.statements() else {
            panic!("expected a DROP statement, got {:?}", parsed.statements());
        };
        assert_eq!(drop.object_kind, DropObjectKind::Sequence);
        assert!(drop.if_exists);
        assert_eq!(drop.names.len(), 2);
        assert_eq!(drop.behavior, Some(DropBehavior::Cascade));
        // Off elsewhere: `SEQUENCE` is an unexpected DROP object kind under a dialect without it.
        parse_with("DROP SEQUENCE s", crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no DROP SEQUENCE");
        parse_with("DROP SEQUENCE s", crate::ParseConfig::new(MySql))
            .expect_err("MySQL has no DROP SEQUENCE");
    }

    fn create_trigger_of(parsed: &Parsed) -> &CreateTrigger<NoExt> {
        let [Statement::CreateTrigger { create, .. }] = parsed.statements() else {
            panic!(
                "expected one CREATE TRIGGER statement, got {:?}",
                parsed.statements(),
            );
        };
        create
    }

    #[test]
    fn create_trigger_parses_all_forms_and_round_trips() {
        // Every form is a bundled-SQLite accept (verified against `sqlite3`): the
        // optional `TEMP`/`TEMPORARY`, `IF NOT EXISTS`, and schema-qualified name; each
        // timing (`BEFORE`/`AFTER`/`INSTEAD OF`, and the absent default); each event
        // (`DELETE`/`INSERT`/`UPDATE`, with and without `OF <cols>`); `FOR EACH ROW`;
        // `WHEN`; and single- vs multi-statement bodies.
        for sql in [
            "CREATE TRIGGER trg AFTER INSERT ON t BEGIN UPDATE t SET c = c + 1; END",
            "CREATE TEMP TRIGGER trg BEFORE DELETE ON t BEGIN DELETE FROM u WHERE x = 1; END",
            "CREATE TEMPORARY TRIGGER trg AFTER UPDATE ON t BEGIN SELECT 1; END",
            "CREATE TRIGGER IF NOT EXISTS trg INSTEAD OF UPDATE OF a, b ON v BEGIN SELECT 1; END",
            "CREATE TRIGGER main.trg DELETE ON t BEGIN SELECT 1; END",
            "CREATE TRIGGER trg AFTER UPDATE ON t FOR EACH ROW WHEN c > 0 BEGIN UPDATE t SET c = 1; END",
            "CREATE TRIGGER trg AFTER INSERT ON t BEGIN INSERT INTO u VALUES (1, 'x'); DELETE FROM t WHERE a = 1; END",
        ] {
            assert_sqlite_round_trips(sql);
        }
    }

    #[test]
    fn create_trigger_captures_shape() {
        let parsed = parse_with(
            "CREATE TEMP TRIGGER IF NOT EXISTS trg INSTEAD OF UPDATE OF a, b ON t FOR EACH ROW WHEN c > 0 BEGIN UPDATE t SET c = 1; SELECT 1; END",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("parses");
        let trigger = create_trigger_of(&parsed);
        assert_eq!(trigger.temporary, Some(TemporaryTableKind::Temp));
        assert!(trigger.if_not_exists);
        assert_eq!(trigger.timing, Some(TriggerTiming::InsteadOf));
        let TriggerEvent::Update { columns, .. } = &trigger.event else {
            panic!("expected an UPDATE event, got {:?}", trigger.event);
        };
        assert_eq!(columns.len(), 2, "UPDATE OF a, b restricts two columns");
        assert!(trigger.for_each_row);
        assert!(trigger.when.is_some());
        assert_eq!(trigger.body.len(), 2, "two body statements");
        assert!(matches!(trigger.body[0], Statement::Update { .. }));
        assert!(matches!(trigger.body[1], Statement::Query { .. }));

        // The no-timing / bare-UPDATE / single-statement defaults.
        let bare = parse_with(
            "CREATE TRIGGER trg UPDATE ON t BEGIN SELECT 1; END",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("parses");
        let trigger = create_trigger_of(&bare);
        assert!(trigger.temporary.is_none());
        assert!(!trigger.if_not_exists);
        assert!(trigger.timing.is_none(), "no fire-time keyword written");
        let TriggerEvent::Update { columns, .. } = &trigger.event else {
            panic!("expected a bare UPDATE event");
        };
        assert!(columns.is_empty(), "no OF-column restriction");
        assert!(!trigger.for_each_row);
        assert!(trigger.when.is_none());
    }

    #[test]
    fn create_trigger_rejects_malformed_bodies() {
        // Each is a bundled-SQLite syntax reject; with the gate ON (SQLITE_RENDER) these
        // reject on the grammar, not the gate.
        for sql in [
            "CREATE TRIGGER trg AFTER INSERT ON t BEGIN END", // empty body
            "CREATE TRIGGER trg AFTER INSERT ON t BEGIN UPDATE t SET c = 1 END", // missing `;`
            "CREATE TRIGGER trg AFTER INSERT ON t BEGIN CREATE TABLE z (a INTEGER); END", // non-DML body
            "CREATE TRIGGER trg AFTER INSERT ON t FOR EACH STATEMENT BEGIN SELECT 1; END", // only ROW
            "CREATE TRIGGER trg BOGUS ON t BEGIN SELECT 1; END", // unknown event
            "CREATE TRIGGER trg AFTER INSERT t BEGIN SELECT 1; END", // missing ON
        ] {
            parse_with(sql, crate::ParseConfig::new(SQLITE_RENDER))
                .expect_err(&format!("{sql:?} should be rejected"));
        }
    }

    #[test]
    fn create_trigger_is_gated_off_outside_sqlite() {
        // `statement_ddl_gates.create_trigger` is off in ANSI/PostgreSQL/MySQL, so the
        // leading `TRIGGER` after `CREATE` is not dispatched and falls through to the
        // `CREATE TABLE` expectation — an unknown statement. It parses once the gate is
        // on (SQLITE_RENDER).
        let sql = "CREATE TRIGGER trg AFTER INSERT ON t BEGIN UPDATE t SET c = 1; END";
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI gates CREATE TRIGGER off");
        parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
            .expect_err("PostgreSQL gates the SQLite trigger body off");
        parse_with(sql, crate::ParseConfig::new(MySql))
            .expect_err("MySQL gates the SQLite trigger body off");
        parse_with(sql, crate::ParseConfig::new(SQLITE_RENDER))
            .expect("SQLite dispatches CREATE TRIGGER");
    }

    /// A DuckDB parse + render dialect, so the gated `CREATE MACRO`/live-body `FUNCTION`
    /// macro DDL parses and round-trips through the DuckDB target.
    const DUCKDB_RENDER: FeatureDialect = FeatureDialect {
        features: &FeatureSet::DUCKDB,
    };

    fn create_macro_of(parsed: &Parsed) -> &CreateMacro<NoExt> {
        let [Statement::CreateMacro { create, .. }] = parsed.statements() else {
            panic!(
                "expected one CREATE MACRO statement, got {:?}",
                parsed.statements(),
            );
        };
        create
    }

    /// Assert `sql` parses under DuckDB and renders back to itself verbatim.
    fn assert_duckdb_round_trips(sql: &str) {
        let parsed = parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
            .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        let rendered = Renderer::new(DUCKDB_RENDER)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
        assert_eq!(rendered, sql, "round-trip");
    }

    #[test]
    fn duckdb_generated_columns_parse_and_round_trip() {
        // DuckDB (libduckdb 1.5.4) accepts the keywordless `AS (<expr>)` shorthand and lets a
        // generated column drop its data type — the typeless shorthand `gen_x AS (x + 5)` is the
        // test-suite corpus form. The typed shorthand (with `VIRTUAL`), a typeless keyworded
        // `GENERATED ALWAYS AS (…)`, and the standard typed keyworded form all round-trip verbatim.
        for sql in [
            "CREATE TABLE tbl (x INTEGER, gen_x AS (x + 5))",
            "CREATE TABLE t (x INTEGER, y INTEGER AS (x + 1) VIRTUAL)",
            "CREATE TABLE t (x INTEGER, y GENERATED ALWAYS AS (x + 1))",
            "CREATE TABLE t (x INTEGER, y INTEGER GENERATED ALWAYS AS (x + 1) VIRTUAL)",
        ] {
            assert_duckdb_round_trips(sql);
        }

        // The typeless shorthand tags `Shorthand` and leaves the column's `data_type` unset.
        let parsed = parse_with(
            "CREATE TABLE tbl (x INTEGER, gen_x AS (x + 5))",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("typeless shorthand parses");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a table definition body");
        };
        let TableElement::Column { column, .. } = &elements[1] else {
            panic!("expected a column element");
        };
        assert!(
            column.data_type.is_none(),
            "generated column omits its type"
        );
        assert_eq!(
            generated_column_of(&parsed, 1).spelling,
            GeneratedColumnSpelling::Shorthand,
        );
    }

    #[test]
    fn duckdb_rejects_non_generated_typeless_column() {
        use crate::dialect::DuckDb;
        // DuckDB's type-optional rule is narrow: it applies *only* to generated columns. A plain
        // typeless column, or a typeless column carrying a non-generated constraint, still requires
        // a type — engine-measured parse rejects on libduckdb 1.5.4 — so the narrowing does not
        // widen into the SQLite any-column typeless rule.
        parse_with("CREATE TABLE t (x)", crate::ParseConfig::new(DuckDb))
            .expect_err("DuckDB rejects a bare typeless column");
        parse_with(
            "CREATE TABLE t (x INTEGER, y DEFAULT 5)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("DuckDB rejects a typeless non-generated column");
    }

    #[test]
    fn create_macro_parses_all_forms_and_round_trips() {
        // Each is an engine-probed DuckDB accept: the scalar and `TABLE` bodies, the
        // `MACRO`/`FUNCTION` synonyms, `OR REPLACE`, a schema-qualified name, an empty and a
        // multi-parameter list, `TEMP`, and a `:=` parameter default. Canonical spacing, so
        // the render is verbatim.
        for sql in [
            "CREATE MACRO plus1(x) AS x + 1",
            "CREATE MACRO m() AS 42",
            "CREATE MACRO m(a, b) AS a + b",
            "CREATE OR REPLACE MACRO m(a, b) AS a + b",
            "CREATE MACRO m(x) AS TABLE SELECT x AS c",
            "CREATE OR REPLACE MACRO m(x) AS TABLE SELECT x AS c",
            "CREATE FUNCTION f(x) AS x * 2",
            "CREATE OR REPLACE FUNCTION f(x) AS TABLE SELECT x",
            "CREATE MACRO s.m(x) AS x",
            "CREATE TEMP MACRO m(x) AS x",
            "CREATE MACRO m(a, b := 10) AS a + b",
        ] {
            assert_duckdb_round_trips(sql);
        }
    }

    #[test]
    fn create_macro_captures_shape() {
        // A scalar `MACRO` with `OR REPLACE`, a schema-qualified name, and a `:=` default.
        let parsed = parse_with(
            "CREATE OR REPLACE MACRO s.m(a, b := 10) AS a + b",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        let create = create_macro_of(&parsed);
        assert!(create.or_replace);
        assert!(create.temporary.is_none());
        assert!(!create.if_not_exists);
        assert_eq!(create.spelling, MacroSpelling::Macro);
        assert_eq!(create.params.len(), 2);
        assert!(create.params[0].default.is_none());
        assert!(
            create.params[1].default.is_some(),
            "`b := 10` has a default"
        );
        assert!(matches!(create.body, MacroBody::Scalar { .. }));

        // A `TABLE` macro spelled with the `FUNCTION` synonym.
        let table = parse_with(
            "CREATE FUNCTION f(x) AS TABLE SELECT x",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        let create = create_macro_of(&table);
        assert_eq!(create.spelling, MacroSpelling::Function);
        assert!(matches!(create.body, MacroBody::Table { .. }));
    }

    #[test]
    fn create_macro_is_gated_off_outside_duckdb() {
        // `statement_ddl_gates.create_macro` is off in ANSI/PostgreSQL/MySQL, so the bare
        // `MACRO` after `CREATE` is not dispatched and falls through to the `CREATE TABLE`
        // expectation — an unknown statement. It parses once the gate is on (DuckDB).
        let sql = "CREATE MACRO m(x) AS x + 1";
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI gates CREATE MACRO off");
        parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
            .expect_err("PostgreSQL gates CREATE MACRO off");
        parse_with(sql, crate::ParseConfig::new(MySql)).expect_err("MySQL gates CREATE MACRO off");
        parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
            .expect("DuckDB dispatches CREATE MACRO");

        // The live-body `FUNCTION` spelling is likewise DuckDB-only: under PostgreSQL,
        // `CREATE FUNCTION` routes to the string-body routine parser, which rejects the
        // live expression body (`AS x + 1`, not `AS '<string>'`).
        parse_with(
            "CREATE FUNCTION f(x) AS x + 1",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect_err("PostgreSQL routes CREATE FUNCTION to the string-body routine");
        parse_with(
            "CREATE FUNCTION f(x) AS x + 1",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("DuckDB dispatches the live-body FUNCTION macro");
    }

    #[test]
    fn drop_macro_parses_all_forms_and_round_trips() {
        // Each is an engine-probed DuckDB accept (v1.5.4): the bare scalar-macro drop, the
        // `MACRO TABLE` table-macro drop, `IF EXISTS`, a schema-qualified name, a comma list
        // (DuckDB parse-accepts and only bind-rejects "can only drop one object at a time"),
        // and `CASCADE`/`RESTRICT`. Canonical spacing, so the render is verbatim — including
        // the `TABLE` keyword and `IF EXISTS`.
        for sql in [
            "DROP MACRO m",
            "DROP MACRO TABLE m",
            "DROP MACRO IF EXISTS m",
            "DROP MACRO TABLE IF EXISTS m",
            "DROP MACRO s.m",
            "DROP MACRO IF EXISTS m, n",
            "DROP MACRO IF EXISTS m CASCADE",
            "DROP MACRO TABLE IF EXISTS m RESTRICT",
        ] {
            assert_duckdb_round_trips(sql);
        }
    }

    #[test]
    fn drop_macro_captures_object_kind_and_is_gated() {
        // The bare form is the scalar macro; the `TABLE` form selects the table-macro kind.
        let scalar = parse_with("DROP MACRO m", crate::ParseConfig::new(DUCKDB_RENDER))
            .expect("DROP MACRO parses");
        let [Statement::Drop { drop, .. }] = scalar.statements() else {
            panic!("expected a DROP statement, got {:?}", scalar.statements());
        };
        assert_eq!(drop.object_kind, DropObjectKind::Macro);

        let table = parse_with(
            "DROP MACRO TABLE IF EXISTS m",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("DROP MACRO TABLE parses");
        let [Statement::Drop { drop, .. }] = table.statements() else {
            panic!("expected a DROP statement, got {:?}", table.statements());
        };
        assert_eq!(drop.object_kind, DropObjectKind::MacroTable);
        assert!(drop.if_exists);

        // Gated off where `create_macro` is off: `MACRO` is an unexpected DROP object kind.
        parse_with("DROP MACRO m", crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no DROP MACRO");
        parse_with("DROP MACRO m", crate::ParseConfig::new(PG_DIALECT))
            .expect_err("PostgreSQL has no DROP MACRO");
        parse_with("DROP MACRO m", crate::ParseConfig::new(MySql))
            .expect_err("MySQL has no DROP MACRO");

        // No over-acceptance: the macro drop takes no argument-type signature (DuckDB
        // rejects `DROP MACRO m(int)`), and only `MACRO TABLE` (not `TABLE MACRO`) is a
        // table-macro drop — both are engine-measured DuckDB syntax rejects.
        parse_with("DROP MACRO m(int)", crate::ParseConfig::new(DUCKDB_RENDER))
            .expect_err("DuckDB rejects a signature on DROP MACRO");
        parse_with("DROP TABLE MACRO m", crate::ParseConfig::new(DUCKDB_RENDER))
            .expect_err("DuckDB rejects TABLE MACRO order");
    }

    fn create_secret_of(parsed: &Parsed) -> &CreateSecret<NoExt> {
        let [Statement::CreateSecret { create, .. }] = parsed.statements() else {
            panic!(
                "expected one CREATE SECRET statement, got {:?}",
                parsed.statements(),
            );
        };
        create
    }

    #[test]
    fn create_or_replace_table_parses_and_round_trips() {
        // DuckDB `CREATE OR REPLACE TABLE`, both the definition and CTAS bodies. Canonical
        // spacing, so the render is verbatim.
        for sql in [
            "CREATE OR REPLACE TABLE t3 (c VARCHAR)",
            "CREATE OR REPLACE TABLE t1 (a VARCHAR, c VARCHAR)",
            "CREATE OR REPLACE TABLE t AS SELECT 1 AS x",
        ] {
            assert_duckdb_round_trips(sql);
        }

        let parsed = parse_with(
            "CREATE OR REPLACE TABLE t3 (c VARCHAR)",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        let create = create_table_of(&parsed);
        assert!(create.or_replace, "OR REPLACE flag is set");
        assert!(create.temporary.is_none());
        assert!(!create.if_not_exists);
    }

    #[test]
    fn create_or_replace_table_is_gated_off_outside_duckdb() {
        // With `create_or_replace_table` off, `OR REPLACE` still parses (the other dialects
        // spell it on VIEW/FUNCTION) but a following `TABLE` is expected to be `VIEW`, so the
        // statement rejects. Only DuckDB (and Lenient) admit `OR REPLACE TABLE`.
        let sql = "CREATE OR REPLACE TABLE t (a INT)";
        parse_with(sql, crate::ParseConfig::new(Ansi))
            .expect_err("ANSI gates OR REPLACE TABLE off");
        parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
            .expect_err("PostgreSQL gates OR REPLACE TABLE off");
        parse_with(sql, crate::ParseConfig::new(MySql))
            .expect_err("MySQL gates OR REPLACE TABLE off");
        parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
            .expect("DuckDB accepts OR REPLACE TABLE");
    }

    #[test]
    fn create_secret_parses_and_round_trips() {
        // DuckDB `CREATE [PERSISTENT] SECRET`, the corpus `TYPE <provider>` option and a
        // multi-option form. Canonical spacing, so the render is verbatim.
        for sql in [
            "CREATE PERSISTENT SECRET my_s (TYPE S3)",
            "CREATE SECRET my_secret (TYPE HTTP)",
            "CREATE PERSISTENT SECRET s (TYPE S3, KEY_ID 'k')",
        ] {
            assert_duckdb_round_trips(sql);
        }

        let parsed = parse_with(
            "CREATE PERSISTENT SECRET my_s (TYPE S3, KEY_ID 'k')",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        let create = create_secret_of(&parsed);
        assert!(create.persistent, "PERSISTENT keyword recorded");
        assert_eq!(create.options.len(), 2);

        let temp = parse_with(
            "CREATE SECRET t (TYPE HTTP)",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        assert!(
            !create_secret_of(&temp).persistent,
            "bare SECRET is not persistent"
        );
    }

    #[test]
    fn create_secret_is_gated_off_outside_duckdb() {
        // The whole-statement gate: with `create_secret` off the `PERSISTENT`/`SECRET`
        // keyword falls through to the `CREATE TABLE` expectation — an unknown statement.
        let sql = "CREATE PERSISTENT SECRET s (TYPE S3)";
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI gates CREATE SECRET off");
        parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
            .expect_err("PostgreSQL gates CREATE SECRET off");
        parse_with(sql, crate::ParseConfig::new(MySql)).expect_err("MySQL gates CREATE SECRET off");
        parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
            .expect("DuckDB dispatches CREATE SECRET");
    }

    fn drop_secret_of(parsed: &Parsed) -> &DropSecretStmt {
        let [Statement::DropSecret { drop, .. }] = parsed.statements() else {
            panic!(
                "expected one DROP SECRET statement, got {:?}",
                parsed.statements(),
            );
        };
        drop
    }

    #[test]
    fn drop_secret_parses_and_round_trips() {
        // Every `opt_persist` / `IF EXISTS` / `FROM <storage>` combination the grammar admits.
        // Canonical spacing, so the render is verbatim.
        for sql in [
            "DROP SECRET s",
            "DROP SECRET IF EXISTS s",
            "DROP PERSISTENT SECRET s",
            "DROP TEMPORARY SECRET IF EXISTS s",
            "DROP SECRET s FROM local_file",
            "DROP PERSISTENT SECRET IF EXISTS s FROM local_file",
        ] {
            assert_duckdb_round_trips(sql);
        }

        let bare =
            parse_with("DROP SECRET s", crate::ParseConfig::new(DUCKDB_RENDER)).expect("parses");
        let drop = drop_secret_of(&bare);
        assert_eq!(drop.persistence, SecretPersistence::Default);
        assert!(!drop.if_exists);
        assert!(drop.storage.is_none());

        let full = parse_with(
            "DROP PERSISTENT SECRET IF EXISTS s FROM local_file",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        let drop = drop_secret_of(&full);
        assert_eq!(drop.persistence, SecretPersistence::Persistent);
        assert!(drop.if_exists);
        assert!(drop.storage.is_some());

        let temp = parse_with(
            "DROP TEMPORARY SECRET s",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        assert_eq!(
            drop_secret_of(&temp).persistence,
            SecretPersistence::Temporary
        );
    }

    #[test]
    fn drop_secret_is_gated_off_outside_duckdb() {
        // The whole-statement gate: with `create_secret` off, `DROP SECRET` never dispatches —
        // `SECRET` is an unexpected DROP object kind, so the statement rejects. The persist
        // modifier forms (`DROP PERSISTENT/TEMPORARY SECRET`) share the one gate.
        for sql in [
            "DROP SECRET s",
            "DROP PERSISTENT SECRET s",
            "DROP TEMPORARY SECRET IF EXISTS s",
        ] {
            parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI gates DROP SECRET off");
            parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .expect_err("PostgreSQL gates DROP SECRET off");
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err("MySQL gates DROP SECRET off");
            parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
                .expect("DuckDB dispatches DROP SECRET");
        }

        // A `DROP PERSISTENT`/`DROP TEMPORARY` not followed by `SECRET` is not a secret drop and
        // must not be swallowed by the two-token lookahead — it falls through to the name-list
        // DROP path (where `PERSISTENT`/`TEMPORARY` is an unknown object kind and rejects).
        parse_with("DROP PERSISTENT t", crate::ParseConfig::new(DUCKDB_RENDER))
            .expect_err("DROP PERSISTENT without SECRET is not a secret drop");
    }

    #[test]
    fn sqlite_typeless_column_records_no_type() {
        let parsed = parse_with(
            "CREATE TABLE t (a, b INTEGER)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("parses");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        assert!(column.data_type.is_none(), "the bare column is typeless");
        let TableElement::Column { column, .. } = &elements[1] else {
            panic!("expected a column");
        };
        assert!(
            column.data_type.is_some(),
            "the typed column keeps its type"
        );
    }

    #[test]
    fn typeless_column_follows_the_flag_not_the_dialect() {
        // The parser branches on `typeless_column_definitions` alone, never on dialect
        // identity: flipping the flag on top of a preset that otherwise disagrees moves the
        // accept/reject boundary with it, so a future dialect cannot bypass the shared table.
        const ANSI_PLUS_TYPELESS: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                typeless_column_definitions: true,
                ..ColumnDefinitionSyntax::ANSI
            }),
        );
        const ANSI_PLUS_TYPELESS_DIALECT: FeatureDialect = FeatureDialect {
            features: &ANSI_PLUS_TYPELESS,
        };
        const SQLITE_NO_TYPELESS: FeatureSet = FeatureSet::SQLITE.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                typeless_column_definitions: false,
                ..ColumnDefinitionSyntax::SQLITE
            }),
        );
        const SQLITE_NO_TYPELESS_DIALECT: FeatureDialect = FeatureDialect {
            features: &SQLITE_NO_TYPELESS,
        };

        let sql = "CREATE TABLE t (a, b)";
        // ANSI with the flag forced on now accepts the typeless column and leaves it untyped.
        let parsed = parse_with(sql, crate::ParseConfig::new(ANSI_PLUS_TYPELESS_DIALECT))
            .expect("flag on accepts");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        assert!(column.data_type.is_none(), "the bare column is typeless");
        // SQLite with the flag forced off rejects it, proving the preset default is honoured.
        parse_with(sql, crate::ParseConfig::new(SQLITE_NO_TYPELESS_DIALECT))
            .expect_err("flag off rejects the typeless column definition");
    }

    #[test]
    fn sqlite_auto_increment_uses_the_joined_spelling_and_rejects_the_underscore() {
        // SQLite's attribute is `AUTOINCREMENT` (tagged `Joined` so it renders back as one
        // word, not MySQL's `AUTO_INCREMENT`).
        let sql = "CREATE TABLE t (a INTEGER PRIMARY KEY AUTOINCREMENT)";
        let parsed = parse_with(sql, crate::ParseConfig::new(SQLITE_RENDER)).expect("parses");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        let spelling = column.constraints.iter().find_map(|c| match c.option {
            ColumnOption::AutoIncrement { spelling, .. } => Some(spelling),
            _ => None,
        });
        assert_eq!(spelling, Some(AutoIncrementSpelling::Joined));
        assert_sqlite_round_trips(sql);

        // The MySQL underscore spelling is *not* a SQLite column attribute (SQLite reads
        // it as a bareword type token), so as an attribute after a constraint it rejects —
        // matching bundled SQLite, which syntax-rejects `... UNIQUE AUTO_INCREMENT`.
        parse_with(
            "CREATE TABLE t (a INTEGER UNIQUE AUTO_INCREMENT)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect_err("SQLite has no `AUTO_INCREMENT` column attribute");
    }

    #[test]
    fn joined_autoincrement_follows_the_flag_not_the_dialect() {
        // The parser branches on `joined_autoincrement_attribute` alone, never on dialect
        // identity: flipping the flag on top of a preset that otherwise disagrees moves the
        // accept/reject boundary with it, so a future dialect cannot bypass the shared table.
        const ANSI_PLUS_JOINED: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                joined_autoincrement_attribute: true,
                ..ColumnDefinitionSyntax::ANSI
            }),
        );
        const ANSI_PLUS_JOINED_DIALECT: FeatureDialect = FeatureDialect {
            features: &ANSI_PLUS_JOINED,
        };
        const SQLITE_NO_JOINED: FeatureSet = FeatureSet::SQLITE.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                joined_autoincrement_attribute: false,
                ..ColumnDefinitionSyntax::SQLITE
            }),
        );
        const SQLITE_NO_JOINED_DIALECT: FeatureDialect = FeatureDialect {
            features: &SQLITE_NO_JOINED,
        };

        let sql = "CREATE TABLE t (a INTEGER PRIMARY KEY AUTOINCREMENT)";
        // ANSI with the flag forced on now accepts the joined attribute and records the
        // spelling.
        let parsed = parse_with(sql, crate::ParseConfig::new(ANSI_PLUS_JOINED_DIALECT))
            .expect("flag on accepts");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        let spelling = column.constraints.iter().find_map(|c| match c.option {
            ColumnOption::AutoIncrement { spelling, .. } => Some(spelling),
            _ => None,
        });
        assert_eq!(spelling, Some(AutoIncrementSpelling::Joined));
        // SQLite with the flag forced off rejects it, proving the preset default is honoured.
        parse_with(sql, crate::ParseConfig::new(SQLITE_NO_JOINED_DIALECT))
            .expect_err("flag off rejects the joined AUTOINCREMENT attribute");

        // The joined `AUTOINCREMENT` and the underscored MySQL `AUTO_INCREMENT` gate on
        // separate flags, so neither admits the other's spelling. Enabling the joined flag
        // on ANSI does *not* enable the underscored attribute (its own
        // `underscored_autoincrement_attribute` flag, off here)…
        parse_with(
            "CREATE TABLE t (a INTEGER AUTO_INCREMENT)",
            crate::ParseConfig::new(ANSI_PLUS_JOINED_DIALECT),
        )
        .expect_err("the joined flag does not admit the underscored spelling");
        // …and MySQL — which admits the underscored spelling but leaves the joined flag
        // off — rejects the bare `AUTOINCREMENT` keyword.
        parse_with(
            "CREATE TABLE t (a INTEGER PRIMARY KEY AUTOINCREMENT)",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .expect_err("MySQL has no joined AUTOINCREMENT attribute");

        // The underscored attribute is its own flag, not a rider on the MySQL
        // trailing-table-options gate: forcing it on over ANSI (whose
        // `table_options` stays off) admits `AUTO_INCREMENT` and records the
        // underscored spelling, while the trailing-option grammar stays rejected.
        const ANSI_PLUS_UNDERSCORED: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                underscored_autoincrement_attribute: true,
                ..ColumnDefinitionSyntax::ANSI
            }),
        );
        const ANSI_PLUS_UNDERSCORED_DIALECT: FeatureDialect = FeatureDialect {
            features: &ANSI_PLUS_UNDERSCORED,
        };
        let parsed = parse_with(
            "CREATE TABLE t (a INTEGER AUTO_INCREMENT)",
            crate::ParseConfig::new(ANSI_PLUS_UNDERSCORED_DIALECT),
        )
        .expect("the underscored flag admits AUTO_INCREMENT on its own");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        let spelling = column.constraints.iter().find_map(|c| match c.option {
            ColumnOption::AutoIncrement { spelling, .. } => Some(spelling),
            _ => None,
        });
        assert_eq!(spelling, Some(AutoIncrementSpelling::Underscored));
        parse_with(
            "CREATE TABLE t (a INTEGER) ENGINE = InnoDB",
            crate::ParseConfig::new(ANSI_PLUS_UNDERSCORED_DIALECT),
        )
        .expect_err("the underscored flag does not open the trailing-option grammar");

        // The QuiltDB preset admits BOTH spellings: the engine contract treats
        // them as SERIAL-equivalent identity columns.
        const QUILTDB_DIALECT: FeatureDialect = FeatureDialect {
            features: &FeatureSet::QUILTDB,
        };
        for sql in [
            "CREATE TABLE t (a INTEGER AUTO_INCREMENT)",
            "CREATE TABLE t (a INTEGER AUTOINCREMENT)",
        ] {
            parse_with(sql, crate::ParseConfig::new(QUILTDB_DIALECT))
                .unwrap_or_else(|e| panic!("QuiltDB accepts {sql}: {e:?}"));
        }
    }

    #[test]
    fn inline_primary_key_ordering_follows_the_flag_not_the_dialect() {
        // The parser branches on `inline_primary_key_ordering` alone, never on dialect identity:
        // flipping the flag on top of a preset that otherwise disagrees moves the accept/reject
        // boundary with it, so a future dialect cannot bypass the shared table.
        const ANSI_PLUS_ORDERING: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                inline_primary_key_ordering: true,
                ..ColumnDefinitionSyntax::ANSI
            }),
        );
        const ANSI_PLUS_ORDERING_DIALECT: FeatureDialect = FeatureDialect {
            features: &ANSI_PLUS_ORDERING,
        };
        const SQLITE_NO_ORDERING: FeatureSet = FeatureSet::SQLITE.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                inline_primary_key_ordering: false,
                ..ColumnDefinitionSyntax::SQLITE
            }),
        );
        const SQLITE_NO_ORDERING_DIALECT: FeatureDialect = FeatureDialect {
            features: &SQLITE_NO_ORDERING,
        };

        // ANSI with the flag forced on accepts both order directions and records the parsed
        // orientation in `ascending` (`DESC` → `Some(false)`, `ASC` → `Some(true)`).
        let ascending_of = |sql: &str, dialect| {
            let parsed =
                parse_with(sql, crate::ParseConfig::new(dialect)).expect("flag on accepts");
            let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body
            else {
                panic!("expected a definition body");
            };
            let TableElement::Column { column, .. } = &elements[0] else {
                panic!("expected a column");
            };
            column
                .constraints
                .iter()
                .find_map(|c| match c.option {
                    ColumnOption::PrimaryKey { ascending, .. } => Some(ascending),
                    _ => None,
                })
                .expect("a PRIMARY KEY column option")
        };
        assert_eq!(
            ascending_of(
                "CREATE TABLE t (a INTEGER PRIMARY KEY DESC)",
                ANSI_PLUS_ORDERING_DIALECT,
            ),
            Some(false),
        );
        assert_eq!(
            ascending_of(
                "CREATE TABLE t (a INTEGER PRIMARY KEY ASC)",
                ANSI_PLUS_ORDERING_DIALECT,
            ),
            Some(true),
        );

        // SQLite with the flag forced off rejects the trailing order qualifier, proving the
        // preset default is honoured and the branch keys on the flag, not the dialect.
        parse_with(
            "CREATE TABLE t (a INTEGER PRIMARY KEY DESC)",
            crate::ParseConfig::new(SQLITE_NO_ORDERING_DIALECT),
        )
        .expect_err("flag off rejects the inline PRIMARY KEY ASC/DESC qualifier");
    }

    #[test]
    fn named_column_collate_follows_the_flag_not_the_dialect() {
        // The `CONSTRAINT <name>` prefix on a column `COLLATE` gates on
        // `named_column_collate_constraint` alone, never on dialect identity. The wrapper only
        // names an already-admitted COLLATE clause, so acceptance needs the bare-COLLATE
        // `column_collation` flag too; flipping the wrapper flag on a preset that otherwise
        // disagrees moves the accept/reject boundary with it, so a future dialect cannot bypass
        // the shared table.
        const ANSI_PLUS_NAMED: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                column_collation: true,
                named_column_collate_constraint: true,
                ..ColumnDefinitionSyntax::ANSI
            }),
        );
        const ANSI_PLUS_NAMED_DIALECT: FeatureDialect = FeatureDialect {
            features: &ANSI_PLUS_NAMED,
        };
        const SQLITE_NO_NAMED: FeatureSet = FeatureSet::SQLITE.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                named_column_collate_constraint: false,
                ..ColumnDefinitionSyntax::SQLITE
            }),
        );
        const SQLITE_NO_NAMED_DIALECT: FeatureDialect = FeatureDialect {
            features: &SQLITE_NO_NAMED,
        };

        let named = "CREATE TABLE t (a TEXT CONSTRAINT c COLLATE nocase)";
        // ANSI with both flags forced on now accepts the named COLLATE constraint and binds the
        // constraint name to it.
        let parsed = parse_with(named, crate::ParseConfig::new(ANSI_PLUS_NAMED_DIALECT))
            .expect("flag on accepts");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        let collate = column
            .constraints
            .iter()
            .find(|c| matches!(c.option, ColumnOption::Collate { .. }))
            .expect("a COLLATE constraint");
        assert!(
            collate.name.is_some(),
            "the CONSTRAINT name binds to COLLATE"
        );
        // SQLite with the wrapper flag forced off rejects the named form, proving the preset
        // default is honoured…
        parse_with(named, crate::ParseConfig::new(SQLITE_NO_NAMED_DIALECT))
            .expect_err("flag off rejects the CONSTRAINT-named COLLATE");
        // …while the bare column COLLATE still parses there, since it rides `column_collation`,
        // not the wrapper flag.
        parse_with(
            "CREATE TABLE t (a TEXT COLLATE nocase)",
            crate::ParseConfig::new(SQLITE_NO_NAMED_DIALECT),
        )
        .expect("bare column COLLATE stays on column_collation");
    }

    #[test]
    fn sqlite_column_conflict_clause_records_resolution() {
        let parsed = parse_with(
            "CREATE TABLE t (a INTEGER UNIQUE ON CONFLICT REPLACE)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("parses");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        assert_eq!(
            column.constraints[0].conflict,
            Some(ConflictResolution::Replace),
        );
    }

    #[test]
    fn column_conflict_clause_follows_the_flag_not_the_dialect() {
        // The parser branches on `column_conflict_resolution_clause` alone, never on dialect
        // identity: flipping the flag on top of a preset that otherwise disagrees moves the
        // accept/reject boundary with it, so a future dialect cannot bypass the shared table.
        const ANSI_PLUS_CONFLICT: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                column_conflict_resolution_clause: true,
                ..ColumnDefinitionSyntax::ANSI
            }),
        );
        const ANSI_PLUS_CONFLICT_DIALECT: FeatureDialect = FeatureDialect {
            features: &ANSI_PLUS_CONFLICT,
        };
        const SQLITE_NO_CONFLICT: FeatureSet = FeatureSet::SQLITE.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                column_conflict_resolution_clause: false,
                ..ColumnDefinitionSyntax::SQLITE
            }),
        );
        const SQLITE_NO_CONFLICT_DIALECT: FeatureDialect = FeatureDialect {
            features: &SQLITE_NO_CONFLICT,
        };

        let sql = "CREATE TABLE t (a INTEGER UNIQUE ON CONFLICT REPLACE)";
        // ANSI with the flag forced on now accepts the clause and records the resolution.
        let parsed = parse_with(sql, crate::ParseConfig::new(ANSI_PLUS_CONFLICT_DIALECT))
            .expect("flag on accepts");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        assert_eq!(
            column.constraints[0].conflict,
            Some(ConflictResolution::Replace),
        );
        // SQLite with the flag forced off rejects it, proving the preset default is honoured.
        parse_with(sql, crate::ParseConfig::new(SQLITE_NO_CONFLICT_DIALECT))
            .expect_err("flag off rejects the column ON CONFLICT clause");
    }

    #[test]
    fn sqlite_bare_trailing_column_constraint_name_parses_and_round_trips() {
        // A trailing bodyless `CONSTRAINT <name>` in a column def: the name parses with no
        // element after it and round-trips verbatim.
        assert_sqlite_round_trips("CREATE TABLE t (a INTEGER CONSTRAINT cn)");
        let parsed = parse_with(
            "CREATE TABLE t (a INTEGER CONSTRAINT cn)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("bare trailing constraint name parses");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        assert_eq!(column.constraints.len(), 1);
        let name = column.constraints[0].name.as_ref().expect("a name");
        assert_eq!(parsed.resolver().resolve(name.sym), "cn");
        assert!(matches!(
            column.constraints[0].option,
            ColumnOption::Bare { .. }
        ));

        // A named constraint WITH a body is unaffected — it still binds the name to the
        // element, never to `Bare`.
        assert_sqlite_round_trips("CREATE TABLE t (a INTEGER CONSTRAINT cn NOT NULL)");
        let parsed = parse_with(
            "CREATE TABLE t (a INTEGER CONSTRAINT cn NOT NULL)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("named constraint with a body parses");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        assert!(matches!(
            column.constraints[0].option,
            ColumnOption::NotNull { .. }
        ));

        // SQLite tolerates any number of bare/named constraint markers stacked back-to-back in
        // one column def, with no separator (engine-measured: `CONSTRAINT cn CONSTRAINT cn2`
        // accepts, each a separate bare marker).
        assert_sqlite_round_trips("CREATE TABLE t (a INTEGER CONSTRAINT cn CONSTRAINT cn2)");
        let parsed = parse_with(
            "CREATE TABLE t (a INTEGER CONSTRAINT cn CONSTRAINT cn2)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("stacked bare constraint names parse");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        assert_eq!(column.constraints.len(), 2);
        assert!(
            column
                .constraints
                .iter()
                .all(|c| matches!(c.option, ColumnOption::Bare { .. }))
        );

        // `CONSTRAINT` with no name at all is still a clean parse error — the name is
        // mandatory, only the element after it is optional.
        parse_with(
            "CREATE TABLE t (a INTEGER CONSTRAINT)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect_err("a CONSTRAINT with no name is still a parse error");

        // Off-dialect (ANSI/PostgreSQL both require a constraint element after the name), the
        // bare form is a clean parse error.
        let sql = "CREATE TABLE t (a INTEGER CONSTRAINT cn)";
        parse_with(sql, crate::ParseConfig::new(Ansi))
            .expect_err("ANSI requires a constraint element after the name");
        parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
            .expect_err("PostgreSQL requires a constraint element after the name");
    }

    #[test]
    fn sqlite_bare_trailing_table_constraint_name_parses_and_round_trips() {
        // A standalone bare `CONSTRAINT <name>` as a whole table constraint (no preceding
        // constraint in the list): round-trips verbatim.
        assert_sqlite_round_trips("CREATE TABLE t (a INTEGER, CONSTRAINT cn)");
        let parsed = parse_with(
            "CREATE TABLE t (a INTEGER, CONSTRAINT cn)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("a standalone bare table constraint parses");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        let TableElement::Constraint { constraint, .. } = &elements[1] else {
            panic!("expected a table constraint");
        };
        let name = constraint.name.as_ref().expect("a name");
        assert_eq!(parsed.resolver().resolve(name.sym), "cn");
        assert!(matches!(
            constraint.constraint,
            TableConstraint::Bare { .. }
        ));

        // SQLite also elides the comma separating a bare trailing name from a *preceding* table
        // constraint (engine-measured: `UNIQUE(a) CONSTRAINT cn` and a named-with-body
        // constraint immediately followed by a bare one both accept with no comma between them
        // — the actual shape of the closed testsuite corpus gaps). The comma is a lossy input
        // spelling with no AST representation — the canonical renderer always inserts it — so
        // these are accept-only checks, not round-trips.
        parse_with(
            "CREATE TABLE t (a INTEGER, UNIQUE (a) CONSTRAINT cn)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("elided comma after a bodied table constraint accepts");
        parse_with(
            "CREATE TABLE t (a INTEGER, CONSTRAINT u1 UNIQUE (a) CONSTRAINT u2)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("elided comma after a named-with-body table constraint accepts");
        let parsed = parse_with(
            "CREATE TABLE t (a INTEGER, UNIQUE(a) CONSTRAINT cn)",
            crate::ParseConfig::new(SQLITE_RENDER),
        )
        .expect("elided-comma bare trailing table constraint parses");
        let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body else {
            panic!("expected a definition body");
        };
        assert_eq!(
            elements.len(),
            3,
            "the UNIQUE(a) and CONSTRAINT cn are two separate elements"
        );
        let TableElement::Constraint { constraint, .. } = &elements[2] else {
            panic!("expected a second table constraint");
        };
        assert!(matches!(
            constraint.constraint,
            TableConstraint::Bare { .. }
        ));

        // Off-dialect (ANSI/PostgreSQL both require a constraint element after the name), the
        // bare table constraint is a clean parse error.
        let sql = "CREATE TABLE t (a INTEGER, CONSTRAINT cn)";
        parse_with(sql, crate::ParseConfig::new(Ansi))
            .expect_err("ANSI requires a constraint element after the name");
        parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
            .expect_err("PostgreSQL requires a constraint element after the name");
    }

    #[test]
    fn without_rowid_option_follows_the_flag_not_the_dialect() {
        // The parser branches on `without_rowid_table_option` alone, never on dialect
        // identity: flipping the flag on top of a preset that otherwise disagrees moves the
        // accept/reject boundary with it, so a future dialect cannot bypass the shared table.
        const ANSI_PLUS_WITHOUT_ROWID: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                without_rowid_table_option: true,
                ..CreateTableClauseSyntax::ANSI
            }),
        );
        const ANSI_PLUS_WITHOUT_ROWID_DIALECT: FeatureDialect = FeatureDialect {
            features: &ANSI_PLUS_WITHOUT_ROWID,
        };
        const SQLITE_NO_WITHOUT_ROWID: FeatureSet = FeatureSet::SQLITE.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                without_rowid_table_option: false,
                ..CreateTableClauseSyntax::SQLITE
            }),
        );
        const SQLITE_NO_WITHOUT_ROWID_DIALECT: FeatureDialect = FeatureDialect {
            features: &SQLITE_NO_WITHOUT_ROWID,
        };

        let sql = "CREATE TABLE t (a INTEGER PRIMARY KEY) WITHOUT ROWID";
        // ANSI with the flag forced on now accepts the trailing option and records it.
        let parsed = parse_with(
            sql,
            crate::ParseConfig::new(ANSI_PLUS_WITHOUT_ROWID_DIALECT),
        )
        .expect("flag on accepts");
        let create = create_table_of(&parsed);
        assert!(
            matches!(
                create.options[0].kind,
                CreateTableOptionKind::WithoutRowid { .. }
            ),
            "expected a WITHOUT ROWID option, got {:?}",
            create.options[0].kind,
        );
        // SQLite with the flag forced off rejects it, proving the preset default is honoured.
        parse_with(
            sql,
            crate::ParseConfig::new(SQLITE_NO_WITHOUT_ROWID_DIALECT),
        )
        .expect_err("flag off rejects the WITHOUT ROWID table option");
    }

    #[test]
    fn strict_option_follows_the_flag_not_the_dialect() {
        // The parser branches on `strict_table_option` alone, never on dialect identity:
        // flipping the flag on top of a preset that otherwise disagrees moves the
        // accept/reject boundary with it, so a future dialect cannot bypass the shared table.
        const ANSI_PLUS_STRICT: FeatureSet = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                strict_table_option: true,
                ..CreateTableClauseSyntax::ANSI
            }),
        );
        const ANSI_PLUS_STRICT_DIALECT: FeatureDialect = FeatureDialect {
            features: &ANSI_PLUS_STRICT,
        };
        const SQLITE_NO_STRICT: FeatureSet = FeatureSet::SQLITE.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                strict_table_option: false,
                ..CreateTableClauseSyntax::SQLITE
            }),
        );
        const SQLITE_NO_STRICT_DIALECT: FeatureDialect = FeatureDialect {
            features: &SQLITE_NO_STRICT,
        };

        let sql = "CREATE TABLE t (a INTEGER) STRICT";
        // ANSI with the flag forced on now accepts the trailing option and records it.
        let parsed = parse_with(sql, crate::ParseConfig::new(ANSI_PLUS_STRICT_DIALECT))
            .expect("flag on accepts");
        let create = create_table_of(&parsed);
        assert!(
            matches!(create.options[0].kind, CreateTableOptionKind::Strict { .. }),
            "expected a STRICT option, got {:?}",
            create.options[0].kind,
        );
        // SQLite with the flag forced off rejects it, proving the preset default is honoured.
        parse_with(sql, crate::ParseConfig::new(SQLITE_NO_STRICT_DIALECT))
            .expect_err("flag off rejects the STRICT table option");
    }

    #[test]
    fn split_decoration_flags_rejected_off_dialect() {
        // Every split decoration flag is off in ANSI: the inline-`PRIMARY KEY` `ASC`/`DESC` rides
        // `inline_primary_key_ordering`, the typeless column rides `typeless_column_definitions`,
        // the trailing `WITHOUT ROWID` rides `without_rowid_table_option`, the trailing
        // `STRICT` rides `strict_table_option`, the column `ON CONFLICT` rides
        // `column_conflict_resolution_clause`, the joined `AUTOINCREMENT` rides
        // `joined_autoincrement_attribute`, the bare column `COLLATE` rides `column_collation`,
        // and the `CONSTRAINT <name>` prefix on a column `COLLATE` rides
        // `named_column_collate_constraint` (each its own flag) — all surface as a clean parse
        // error rather than being silently accepted.
        for sql in [
            "CREATE TABLE t (a INTEGER) WITHOUT ROWID",
            "CREATE TABLE t (a INTEGER) STRICT",
            "CREATE TABLE t (a, b)",
            "CREATE TABLE t (a INTEGER PRIMARY KEY AUTOINCREMENT)",
            "CREATE TABLE t (a INTEGER UNIQUE ON CONFLICT REPLACE)",
            "CREATE TABLE t (a TEXT COLLATE NOCASE)",
            "CREATE TABLE t (a TEXT CONSTRAINT c COLLATE NOCASE)",
            "CREATE TABLE t (a INTEGER PRIMARY KEY DESC)",
        ] {
            parse_with(sql, crate::ParseConfig::new(Ansi))
                .expect_err(&format!("ANSI must reject {sql:?}"));
        }
    }

    /// A PostgreSQL-featured dialect so the gated `IF EXISTS` forms parse.
    const PG_DIALECT: FeatureDialect = FeatureDialect {
        features: &FeatureSet::POSTGRES,
    };

    /// The `EXTENSION ... VERSION` operand is PostgreSQL's `NonReservedWord_or_Sconst`
    /// (nonreserved-word-or-sconst-literal-kind-siblings): a bare non-reserved word, a quoted
    /// identifier, or an `Sconst` string (plain, `E'…'`, dollar-quoted) is accepted, while a
    /// bit-string (`b'…'`/`x'…'`), a national (`N'…'`) constant, or a reserved keyword is a
    /// syntax error — the string-kind boundary libpg_query enforces (engine-measured on
    /// pg_query 6.1.1). Both the `CREATE` and `ALTER … UPDATE TO` forms share the operand.
    #[test]
    fn extension_version_accepts_word_or_sconst_rejects_non_sconst_and_round_trips() {
        for sql in [
            "CREATE EXTENSION foo VERSION bar",
            "CREATE EXTENSION foo VERSION \"bar\"",
            "CREATE EXTENSION foo VERSION 'bar'",
            "CREATE EXTENSION foo VERSION E'bar'",
            "CREATE EXTENSION foo VERSION $$bar$$",
            "ALTER EXTENSION foo UPDATE TO 'bar'",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip {sql:?}");
        }
        // A bit/hex (`BCONST`/`XCONST`) or national (`N'…'`) string constant is not an `Sconst`,
        // so it is not a valid version operand; a reserved keyword is not a `NonReservedWord`.
        for sql in [
            "CREATE EXTENSION foo VERSION b'0'",
            "CREATE EXTENSION foo VERSION x'ab'",
            "CREATE EXTENSION foo VERSION N'x'",
            "CREATE EXTENSION foo VERSION select",
            "ALTER EXTENSION foo UPDATE TO b'0'",
        ] {
            parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .expect_err(&format!("PostgreSQL must reject {sql:?}"));
        }
        // The word and `Sconst` spellings resolve to their distinct surface variants.
        let word = parse_with(
            "CREATE EXTENSION foo VERSION bar",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .unwrap();
        let string = parse_with(
            "CREATE EXTENSION foo VERSION 'bar'",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .unwrap();
        assert!(matches!(
            extension_version_of(&word),
            crate::ast::ExtensionVersion::Word { .. }
        ));
        assert!(matches!(
            extension_version_of(&string),
            crate::ast::ExtensionVersion::String { .. }
        ));
    }

    fn extension_version_of(parsed: &Parsed) -> &crate::ast::ExtensionVersion {
        let Statement::CreateExtension { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE EXTENSION statement");
        };
        for option in &create.options {
            if let crate::ast::CreateExtensionOption::Version { version, .. } = option {
                return version;
            }
        }
        panic!("expected a VERSION option");
    }

    /// A dollar-quoted `CREATE FUNCTION` body parses onto the [`FunctionBody`] axis and
    /// round-trips byte-for-byte: the delimiter tag (`$$`, `$body$`, `$tag$`) and the verbatim
    /// body text ride the body [`Literal`]'s span, so no normalization touches either. The
    /// single-quoted body is the same axis variant — dollar-quoting is a spelling of the
    /// string body, not a separate body kind. `AS` stays an order-independent option, so
    /// `LANGUAGE … AS …` and `AS … LANGUAGE …` both reproduce their written order.
    #[test]
    fn create_function_dollar_body_rides_axis_and_round_trips() {
        for sql in [
            "CREATE FUNCTION f() RETURNS INTEGER AS $$ select 1 $$ LANGUAGE sql",
            "CREATE FUNCTION f() RETURNS INTEGER AS $body$ select 1 $body$ LANGUAGE sql",
            "CREATE FUNCTION f() RETURNS INTEGER LANGUAGE sql AS $$select 1$$",
            "CREATE FUNCTION f() RETURNS INTEGER AS $tag$ multi\nline $tag$ LANGUAGE plpgsql",
            // The single-quoted body is the same axis variant (dollar-quoting is a spelling).
            "CREATE FUNCTION add(a INTEGER, b INTEGER) RETURNS INTEGER AS 'select $1 + $2' LANGUAGE sql",
        ] {
            let pg = parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .unwrap_or_else(|err| panic!("PARSE {sql:?}: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&pg)
                .unwrap_or_else(|err| panic!("RENDER {sql:?}: {err:?}"));
            assert_eq!(rendered, sql, "round-trip {sql:?}");

            let [Statement::CreateFunction { create, .. }] = pg.statements() else {
                panic!(
                    "{sql:?}: expected one CREATE FUNCTION, got {:?}",
                    pg.statements()
                );
            };
            let body = create
                .options
                .iter()
                .find_map(|option| match option {
                    FunctionOption::As { body, .. } => Some(body),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{sql:?}: expected an AS body option"));
            let FunctionBody::Definition { definition, .. } = body else {
                panic!("{sql:?}: an `AS` body is always a string Definition, never a RETURN body");
            };
            assert_eq!(
                definition.kind,
                LiteralKind::String,
                "{sql:?}: the body is a string literal",
            );
        }
    }

    /// The `CREATE FUNCTION … LANGUAGE <name>` operand is PostgreSQL's
    /// `NonReservedWord_or_Sconst` (routine-language-name-word-or-sconst): a bare word or an
    /// `Sconst` string (plain, `E'…'`, dollar-quoted) parses onto the shared
    /// [`LanguageName`] union and round-trips byte-identically, while a bit-string
    /// (`b'…'`/`x'…'`) or national (`N'…'`) constant is not an `Sconst` and is rejected — the
    /// string-kind boundary pg_query enforces (engine-measured on pg_query 6.1.1, mirrored by
    /// the `PG_DIFFERENTIAL_RAW_BYTES_REPLAYS` fuzz replays).
    #[test]
    fn create_function_language_name_word_or_sconst_round_trips() {
        // The bare word and each `Sconst` spelling; the trailing `AS` body isolates the
        // LANGUAGE operand from the round-trip.
        for (sql, is_string) in [
            (
                "CREATE FUNCTION f() RETURNS INTEGER LANGUAGE sql AS 'x'",
                false,
            ),
            (
                "CREATE FUNCTION f() RETURNS INTEGER LANGUAGE 'sql' AS 'x'",
                true,
            ),
            (
                "CREATE FUNCTION f() RETURNS INTEGER LANGUAGE E'sql' AS 'x'",
                true,
            ),
            (
                "CREATE FUNCTION f() RETURNS INTEGER LANGUAGE $$sql$$ AS 'x'",
                true,
            ),
        ] {
            let pg = parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .unwrap_or_else(|err| panic!("PARSE {sql:?}: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&pg)
                .unwrap_or_else(|err| panic!("RENDER {sql:?}: {err:?}"));
            assert_eq!(rendered, sql, "round-trip {sql:?}");

            let [Statement::CreateFunction { create, .. }] = pg.statements() else {
                panic!("{sql:?}: expected one CREATE FUNCTION");
            };
            let name = create
                .options
                .iter()
                .find_map(|option| match option {
                    FunctionOption::Language { name, .. } => Some(name),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{sql:?}: expected a LANGUAGE option"));
            assert_eq!(
                matches!(name, LanguageName::String { .. }),
                is_string,
                "{sql:?}: LANGUAGE operand variant",
            );
        }

        // A bit-string, hex, or national constant is not an `Sconst`, so it is rejected in the
        // LANGUAGE position (matching pg_query), just as the `DO`/extension-version siblings do.
        for sql in [
            "CREATE FUNCTION f() RETURNS INTEGER LANGUAGE b'0' AS 'x'",
            "CREATE FUNCTION f() RETURNS INTEGER LANGUAGE x'ab' AS 'x'",
            "CREATE FUNCTION f() RETURNS INTEGER LANGUAGE N'sql' AS 'x'",
        ] {
            parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .expect_err(&format!("{sql:?}: a non-Sconst LANGUAGE name is rejected"));
        }
    }

    /// MySQL's routine `LANGUAGE` admits only the bare word `SQL`; the string spelling
    /// `LANGUAGE 'SQL'` is engine-measured `ER_PARSE_ERROR` (1064) on mysql:8, so the MySQL
    /// preset (`routine_language_string` off) rejects it while still accepting the bare word.
    /// The boundary the DO precedent set — the string arm is PostgreSQL-only.
    #[test]
    fn mysql_routine_language_rejects_string_spelling() {
        parse_with(
            "CREATE PROCEDURE p() LANGUAGE SQL BEGIN SELECT 1; END",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL admits the bare-word routine LANGUAGE");
        parse_with(
            "CREATE PROCEDURE p() LANGUAGE 'SQL' BEGIN SELECT 1; END",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("MySQL rejects the string-constant routine LANGUAGE spelling");
    }

    /// A `CREATE FUNCTION` parameter default (`func_arg_with_default`) parses onto the
    /// `FunctionParam::default` slot and round-trips its `DEFAULT`-vs-`=` spelling verbatim: the
    /// `FunctionParamDefaultSpelling` tag records which form the source used, so neither
    /// normalizes to the other (sqlparser-rs collapses both to `=`).
    #[test]
    fn create_function_parameter_defaults_carry_spelling_and_round_trip() {
        // `INTEGER` (not `INT`) matches the PostgreSQL canonical type spelling, so the
        // round-trip isolates the default clause rather than tripping on type rendering.
        for (sql, spelling) in [
            (
                "CREATE FUNCTION f(a INTEGER DEFAULT 0) LANGUAGE sql",
                FunctionParamDefaultSpelling::Default,
            ),
            (
                "CREATE FUNCTION f(a INTEGER = 0) LANGUAGE sql",
                FunctionParamDefaultSpelling::Equals,
            ),
        ] {
            let pg = parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .unwrap_or_else(|err| panic!("PARSE {sql:?}: {err:?}"));
            let [Statement::CreateFunction { create, .. }] = pg.statements() else {
                panic!("{sql:?}: expected one CREATE FUNCTION");
            };
            let default = create.params[0]
                .default
                .as_ref()
                .unwrap_or_else(|| panic!("{sql:?}: parameter should carry a default"));
            assert_eq!(default.spelling, spelling, "{sql:?}: spelling tag");
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&pg)
                .unwrap_or_else(|err| panic!("RENDER {sql:?}: {err:?}"));
            assert_eq!(rendered, sql, "round-trip {sql:?}");
        }

        // A default-less parameter leaves the slot empty (the bare-type form is unchanged).
        let bare = parse_with(
            "CREATE FUNCTION f(a INTEGER) LANGUAGE sql",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("bare parameter parses");
        let [Statement::CreateFunction { create, .. }] = bare.statements() else {
            panic!("expected one CREATE FUNCTION");
        };
        assert!(create.params[0].default.is_none(), "no default written");
    }

    /// A `CREATE FUNCTION` parameter carries its argument mode (`arg_class`) on the
    /// `FunctionParam::mode` slot, independent of its name, type, and default: the four
    /// PostgreSQL modes parse and round-trip, an unnamed mode-bearing type is admitted
    /// (`IN INTEGER`), and a bare parameter leaves the slot empty.
    #[test]
    fn create_function_parameter_modes_carry_and_round_trip() {
        for (sql, mode) in [
            (
                "CREATE FUNCTION f(IN a INTEGER) LANGUAGE sql",
                FunctionParamMode::In,
            ),
            (
                "CREATE FUNCTION f(OUT b INTEGER) LANGUAGE sql",
                FunctionParamMode::Out,
            ),
            (
                "CREATE FUNCTION f(INOUT c INTEGER) LANGUAGE sql",
                FunctionParamMode::InOut,
            ),
            (
                "CREATE FUNCTION f(VARIADIC d INTEGER[]) LANGUAGE sql",
                FunctionParamMode::Variadic,
            ),
            (
                "CREATE FUNCTION f(IN INTEGER) LANGUAGE sql",
                FunctionParamMode::In,
            ),
            (
                "CREATE FUNCTION f(IN a INTEGER DEFAULT 0) LANGUAGE sql",
                FunctionParamMode::In,
            ),
        ] {
            let pg = parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .unwrap_or_else(|err| panic!("PARSE {sql:?}: {err:?}"));
            let [Statement::CreateFunction { create, .. }] = pg.statements() else {
                panic!("{sql:?}: expected one CREATE FUNCTION");
            };
            assert_eq!(create.params[0].mode, Some(mode), "{sql:?}: mode tag");
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&pg)
                .unwrap_or_else(|err| panic!("RENDER {sql:?}: {err:?}"));
            assert_eq!(rendered, sql, "round-trip {sql:?}");
        }

        // A mode-less parameter leaves the slot empty, and modes compose across a list.
        let bare = parse_with(
            "CREATE FUNCTION f(a INTEGER) LANGUAGE sql",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("bare parameter parses");
        let [Statement::CreateFunction { create, .. }] = bare.statements() else {
            panic!("expected one CREATE FUNCTION");
        };
        assert!(create.params[0].mode.is_none(), "no mode written");

        let multi = parse_with(
            "CREATE FUNCTION f(IN a INTEGER, OUT b INTEGER) LANGUAGE sql",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("multi-parameter mode list parses");
        let [Statement::CreateFunction { create, .. }] = multi.statements() else {
            panic!("expected one CREATE FUNCTION");
        };
        assert_eq!(create.params[0].mode, Some(FunctionParamMode::In));
        assert_eq!(create.params[1].mode, Some(FunctionParamMode::Out));
    }

    /// The parameter *name* is PostgreSQL's `type_function_name` class, not `ColId`: a
    /// `type_func_name` keyword (`left`) is a legal parameter name, while a `col_name`
    /// keyword (`int`) is not — so `f(int int)` is rejected (the first `int` cannot be a
    /// name) and a lone `int` is unambiguously the type, matching how PostgreSQL resolves
    /// the name-vs-type ambiguity.
    #[test]
    fn create_function_parameter_name_uses_type_function_name_class() {
        // A `type_func_name` keyword is a valid parameter name (a bare `ColId` rejects it).
        let ok = parse_with(
            "CREATE FUNCTION f(left INTEGER) LANGUAGE sql",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("a type_func_name keyword is a valid parameter name");
        let [Statement::CreateFunction { create, .. }] = ok.statements() else {
            panic!("expected one CREATE FUNCTION");
        };
        let name = create.params[0]
            .name
            .as_ref()
            .expect("`left` is the parameter name");
        assert_eq!(
            ok.resolver().resolve(name.sym),
            "left",
            "`left` parses as the name"
        );

        // A `col_name` keyword cannot be a parameter name, so `f(int int)` is a parse error
        // (PostgreSQL rejects it), while a lone `int` is the type.
        parse_with(
            "CREATE FUNCTION f(int int) LANGUAGE sql",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect_err("a col_name keyword is rejected as a parameter name");
        let lone = parse_with(
            "CREATE FUNCTION f(int) LANGUAGE sql",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("a lone col_name type keyword is the type");
        let [Statement::CreateFunction { create, .. }] = lone.statements() else {
            panic!("expected one CREATE FUNCTION");
        };
        assert!(
            create.params[0].name.is_none(),
            "lone `int` is the type, not a name"
        );
    }

    /// MySQL's `CREATE FUNCTION` parameters carry no argument mode (the `IN`/`OUT`/`INOUT`
    /// modes are a stored-*procedure* form), so the `routine_arg_modes` gate is off: a mode
    /// keyword before a parameter surfaces as a parse error, while the bare parameter still
    /// parses. The `routines`-sibling `routine_arg_defaults` precedent.
    #[test]
    fn mysql_rejects_function_parameter_modes() {
        parse_with("CREATE FUNCTION f(a INT)", crate::ParseConfig::new(MySql))
            .expect("MySQL accepts a bare parameter");
        parse_with(
            "CREATE FUNCTION f(IN a INT)",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("MySQL rejects an IN parameter mode");
        parse_with(
            "CREATE FUNCTION f(VARIADIC a INT)",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("MySQL rejects a VARIADIC parameter mode");
    }

    /// MySQL has no routine-parameter defaults, so the `routine_arg_defaults` gate is off: a
    /// `DEFAULT`/`=` after the parameter type is left unconsumed and the parameter-list close
    /// surfaces it as a parse error. The bare parameter still parses under MySQL.
    #[test]
    fn mysql_rejects_function_parameter_defaults() {
        // The bare parameter parses under MySQL, so the rejection below isolates the default
        // clause rather than an unrelated trailing option.
        parse_with("CREATE FUNCTION f(a INT)", crate::ParseConfig::new(MySql))
            .expect("MySQL accepts a bare parameter");
        parse_with(
            "CREATE FUNCTION f(a INT DEFAULT 0)",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("MySQL rejects a DEFAULT parameter default");
        parse_with(
            "CREATE FUNCTION f(a INT = 0)",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("MySQL rejects an `=` parameter default");
    }

    /// Parse `sql` under MySQL, render it back through the MySQL target, and assert the text
    /// round-trips exactly — the routine-DDL faithfulness check.
    fn mysql_round_trips(sql: &str) -> Parsed {
        let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
            .unwrap_or_else(|err| panic!("PARSE {sql:?}: {err:?}"));
        let rendered = Renderer::new(MYSQL_RENDER)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("RENDER {sql:?}: {err:?}"));
        assert_eq!(rendered, sql, "routine round-trip {sql:?}");
        parsed
    }

    #[test]
    fn mysql_create_procedure_parses_and_round_trips() {
        // The measured `CREATE PROCEDURE` family probe.
        let parsed = mysql_round_trips("CREATE PROCEDURE zzp_p() BEGIN END");
        let Statement::CreateProcedure { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE PROCEDURE");
        };
        assert!(create.definer.is_none());
        assert!(!create.if_not_exists);
        assert!(create.params.is_empty());
        assert!(create.characteristics.is_empty());
        assert!(matches!(*create.body, Statement::Compound { .. }));
    }

    #[test]
    fn mysql_create_function_with_characteristics_parses_and_round_trips() {
        // The measured `CREATE FUNCTION` family probe: a `RETURN <expr>` body function with the
        // `DETERMINISTIC` characteristic.
        let parsed =
            mysql_round_trips("CREATE FUNCTION zzp_f() RETURNS INTEGER DETERMINISTIC RETURN 1");
        let Statement::CreateFunction { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE FUNCTION");
        };
        assert!(create.returns.is_some());
        // The `DETERMINISTIC` characteristic rides the shared `options` (FunctionOption) list.
        assert!(
            create
                .options
                .iter()
                .any(|opt| matches!(opt, FunctionOption::Deterministic { not: false, .. })),
            "DETERMINISTIC characteristic captured on the option list",
        );
        assert!(matches!(
            create.body.as_deref(),
            Some(FunctionBody::Return { .. })
        ));
    }

    #[test]
    fn mysql_procedure_parameter_modes_and_characteristics_matrix() {
        // Parameter modes (IN/OUT/INOUT) ride the existing FunctionParam vocabulary; the full
        // characteristic set rides the shared FunctionOption axis.
        // A canonical `SELECT` body isolates the round-trip from the orthogonal, existing
        // session-`SET` `=`->`TO` spelling canonicalization.
        mysql_round_trips(
            "CREATE PROCEDURE p(IN a INTEGER, OUT b INTEGER, INOUT c INTEGER) \
             LANGUAGE SQL NOT DETERMINISTIC MODIFIES SQL DATA SQL SECURITY INVOKER \
             COMMENT 'doc' BEGIN SELECT a; END",
        );
        mysql_round_trips(
            "CREATE FUNCTION f() RETURNS INTEGER DETERMINISTIC CONTAINS SQL \
             SQL SECURITY DEFINER RETURN 1",
        );
        mysql_round_trips("CREATE PROCEDURE p() READS SQL DATA BEGIN END");
        mysql_round_trips("CREATE PROCEDURE p() NO SQL BEGIN END");
    }

    #[test]
    fn mysql_function_block_body_and_return_context() {
        // A `BEGIN … END` function body rides FunctionBody::Block; RETURN is legal inside it.
        let parsed = mysql_round_trips("CREATE FUNCTION f() RETURNS INTEGER BEGIN RETURN 1; END");
        let Statement::CreateFunction { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE FUNCTION");
        };
        assert!(matches!(
            create.body.as_deref(),
            Some(FunctionBody::Block { .. })
        ));
        // RETURN in a PROCEDURE body rejects (server ER_SP_BADRETURN) — top-level and nested.
        parse_with(
            "CREATE PROCEDURE p() BEGIN RETURN 1; END",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("RETURN in a procedure body must reject");
        parse_with(
            "CREATE PROCEDURE p() BEGIN IF 1 THEN RETURN 1; END IF; END",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("a nested RETURN in a procedure body must reject");
        // The bare non-compound procedure body (a single statement) parses.
        mysql_round_trips("CREATE PROCEDURE p() SELECT 1");
    }

    #[test]
    fn mysql_definer_prefix_parses_and_round_trips() {
        let parsed = mysql_round_trips("CREATE DEFINER = admin PROCEDURE p() BEGIN END");
        let Statement::CreateProcedure { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE PROCEDURE");
        };
        assert!(create.definer.is_some());
        mysql_round_trips("CREATE DEFINER = CURRENT_USER PROCEDURE p() BEGIN END");
        mysql_round_trips("CREATE DEFINER = CURRENT_USER() FUNCTION f() RETURNS INTEGER RETURN 1");
        mysql_round_trips("CREATE PROCEDURE IF NOT EXISTS p() BEGIN END");
    }

    #[test]
    fn mysql_create_trigger_parses_and_round_trips() {
        // The measured `CREATE TRIGGER` family shape: a single-statement `sp_proc_stmt` body.
        let parsed = mysql_round_trips(
            "CREATE TRIGGER zzp_tr BEFORE INSERT ON t1 FOR EACH ROW INSERT INTO zzp_t2 VALUES (1)",
        );
        let Statement::CreateStoredTrigger { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE (stored) TRIGGER");
        };
        assert!(create.definer.is_none());
        assert!(!create.if_not_exists);
        assert_eq!(create.timing, TriggerTiming::Before);
        assert!(matches!(create.event, TriggerEvent::Insert { .. }));
        assert!(create.ordering.is_none());
        assert!(matches!(*create.body, Statement::Insert { .. }));

        // A `BEGIN … END` compound-statement body (the common real form).
        let compound = mysql_round_trips(
            "CREATE TRIGGER zzp_tr AFTER UPDATE ON t1 FOR EACH ROW BEGIN INSERT INTO zzp_t2 VALUES (1); END",
        );
        let Statement::CreateStoredTrigger { create, .. } = &compound.statements()[0] else {
            panic!("expected a CREATE (stored) TRIGGER");
        };
        assert_eq!(create.timing, TriggerTiming::After);
        assert!(matches!(create.event, TriggerEvent::Update { .. }));
        assert!(matches!(*create.body, Statement::Compound { .. }));
    }

    #[test]
    fn mysql_create_trigger_axes_round_trip() {
        // Every timing/event combination, `IF NOT EXISTS`, `DEFINER`, and the
        // `FOLLOWS`/`PRECEDES` ordering anchor round-trip.
        for sql in [
            "CREATE TRIGGER tr BEFORE DELETE ON t1 FOR EACH ROW INSERT INTO t2 VALUES (1)",
            "CREATE TRIGGER tr AFTER INSERT ON t1 FOR EACH ROW INSERT INTO t2 VALUES (1)",
            "CREATE TRIGGER IF NOT EXISTS tr BEFORE UPDATE ON t1 FOR EACH ROW BEGIN END",
            "CREATE DEFINER = admin TRIGGER tr BEFORE INSERT ON t1 FOR EACH ROW BEGIN END",
            "CREATE DEFINER = CURRENT_USER TRIGGER tr AFTER INSERT ON t1 FOR EACH ROW BEGIN END",
            "CREATE TRIGGER tr BEFORE INSERT ON t1 FOR EACH ROW FOLLOWS other_tr INSERT INTO t2 VALUES (1)",
            "CREATE TRIGGER tr AFTER DELETE ON s.t1 FOR EACH ROW PRECEDES other_tr BEGIN END",
        ] {
            mysql_round_trips(sql);
        }

        // The ordering anchor is captured on the node.
        let parsed = parse_with(
            "CREATE TRIGGER tr BEFORE INSERT ON t1 FOR EACH ROW FOLLOWS other_tr BEGIN END",
            crate::ParseConfig::new(MySql),
        )
        .expect("ordered trigger parses");
        let Statement::CreateStoredTrigger { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE (stored) TRIGGER");
        };
        assert!(matches!(
            create.ordering,
            Some(TriggerOrder::Follows { .. })
        ));
    }

    #[test]
    fn mysql_create_trigger_rejects_non_mysql_shapes() {
        // `INSTEAD OF` timing and statement-level triggers do not exist in MySQL.
        parse_with(
            "CREATE TRIGGER tr INSTEAD OF INSERT ON t1 FOR EACH ROW BEGIN END",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("MySQL has no INSTEAD OF trigger timing");
        parse_with(
            "CREATE TRIGGER tr BEFORE INSERT ON t1 BEGIN END",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("MySQL requires FOR EACH ROW");
        // A trigger body is not a function body: `RETURN` rejects (server ER_SP_BADRETURN).
        parse_with(
            "CREATE TRIGGER tr BEFORE INSERT ON t1 FOR EACH ROW BEGIN RETURN 1; END",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("RETURN is illegal in a trigger body");
        // Non-MySQL dialects have no stored-program trigger: the SQLite `create_trigger` grammar
        // rejects the MySQL `FOR EACH ROW <stmt>` body, and ANSI/PostgreSQL reject `TRIGGER`.
        parse_with(
            "CREATE TRIGGER tr BEFORE INSERT ON t1 FOR EACH ROW INSERT INTO t2 VALUES (1)",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no CREATE TRIGGER");
    }

    #[test]
    fn drop_trigger_parses_and_round_trips() {
        // The measured `DROP TRIGGER` family probe, plus `IF EXISTS` and a schema qualifier.
        for sql in [
            "DROP TRIGGER zzp_tr",
            "DROP TRIGGER IF EXISTS zzp_tr",
            "DROP TRIGGER s.zzp_tr",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("PARSE {sql:?}: {err:?}"));
            let Statement::Drop { drop, .. } = &parsed.statements()[0] else {
                panic!("expected a DROP for {sql:?}");
            };
            assert_eq!(drop.object_kind, DropObjectKind::Trigger);
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("RENDER {sql:?}: {err:?}"));
            assert_eq!(rendered, sql, "drop-trigger round-trip {sql:?}");
        }
        // SQLite shares the same name-only trigger drop (its `create_trigger` flag also gates it).
        let sqlite = parse_with("DROP TRIGGER zzp_tr", crate::ParseConfig::new(Sqlite))
            .expect("SQLite has DROP TRIGGER");
        let Statement::Drop { drop, .. } = &sqlite.statements()[0] else {
            panic!("expected a DROP");
        };
        assert_eq!(drop.object_kind, DropObjectKind::Trigger);
        // ANSI/PostgreSQL do not model this name-only trigger drop (no trigger flag on).
        parse_with("DROP TRIGGER zzp_tr", crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no DROP TRIGGER object kind");
    }

    #[test]
    fn mysql_alter_routine_parses_and_round_trips() {
        // The measured `ALTER PROCEDURE` / `ALTER FUNCTION` family probes.
        let proc = mysql_round_trips("ALTER PROCEDURE zzp_p COMMENT 'zzp'");
        let Statement::AlterRoutine { alter, .. } = &proc.statements()[0] else {
            panic!("expected an ALTER ROUTINE");
        };
        assert_eq!(alter.kind, RoutineKind::Procedure);
        let func = mysql_round_trips("ALTER FUNCTION zzp_f COMMENT 'zzp'");
        let Statement::AlterRoutine { alter, .. } = &func.statements()[0] else {
            panic!("expected an ALTER ROUTINE");
        };
        assert_eq!(alter.kind, RoutineKind::Function);
        mysql_round_trips("ALTER PROCEDURE p LANGUAGE SQL SQL SECURITY INVOKER");
        // DETERMINISTIC is not an ALTER-legal characteristic (server rejects it): left
        // unconsumed, it surfaces as a clean parse error.
        parse_with(
            "ALTER PROCEDURE p DETERMINISTIC",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("ALTER rejects the DETERMINISTIC characteristic");
    }

    #[test]
    fn mysql_create_event_schedule_shapes_parse_and_round_trip() {
        // The measured `CREATE EVENT` family probe (AT one-shot, `DO` body).
        let parsed = mysql_round_trips("CREATE EVENT zzp_e ON SCHEDULE AT NOW() DO BEGIN END");
        let Statement::CreateEvent { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE EVENT");
        };
        assert!(create.definer.is_none());
        assert!(!create.if_not_exists);
        assert!(matches!(create.schedule, EventSchedule::At { .. }));
        assert!(create.on_completion.is_none());
        assert!(create.status.is_none());
        assert!(create.comment.is_none());

        // The recurring EVERY form with an interval unit reused from the shared IntervalFields
        // vocabulary, plus STARTS/ENDS window bounds (each an ordinary expression). The MySQL
        // `<ts> + INTERVAL <n> <unit>` arithmetic offset the schedule also admits rides the
        // expression grammar's interval-arithmetic operator (a separate, still-missing surface),
        // so a plain timestamp expression is used here.
        let every = mysql_round_trips(
            "CREATE EVENT e ON SCHEDULE EVERY 1 HOUR STARTS NOW() ENDS '2025-12-31 00:00:00' \
             DO BEGIN END",
        );
        let Statement::CreateEvent { create, .. } = &every.statements()[0] else {
            panic!("expected a CREATE EVENT");
        };
        let EventSchedule::Every {
            unit, starts, ends, ..
        } = &create.schedule
        else {
            panic!("expected an EVERY schedule");
        };
        assert_eq!(*unit, IntervalFields::Hour);
        assert!(starts.is_some() && ends.is_some());

        // MySQL composite interval units (underscore spelling) map onto the shared *To* variants.
        let composite =
            mysql_round_trips("CREATE EVENT e ON SCHEDULE EVERY 2 DAY_HOUR DO BEGIN END");
        let Statement::CreateEvent { create, .. } = &composite.statements()[0] else {
            panic!("expected a CREATE EVENT");
        };
        assert!(matches!(
            create.schedule,
            EventSchedule::Every {
                unit: IntervalFields::DayToHour,
                ..
            }
        ));
        // The MySQL-only microsecond composite (grammar-valid; server 1235-rejects semantically).
        mysql_round_trips("CREATE EVENT e ON SCHEDULE EVERY 5 DAY_MICROSECOND DO BEGIN END");
    }

    #[test]
    fn mysql_create_event_full_clause_set_parses_and_round_trips() {
        // Every optional clause in its fixed grammar order: DEFINER, IF NOT EXISTS, completion,
        // status, comment. `BEGIN END` is the canonical body isolating the event surface.
        let parsed = mysql_round_trips(
            "CREATE DEFINER = root EVENT IF NOT EXISTS e ON SCHEDULE AT NOW() \
             ON COMPLETION NOT PRESERVE ENABLE COMMENT 'c' DO BEGIN END",
        );
        let Statement::CreateEvent { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE EVENT");
        };
        assert!(create.definer.is_some());
        assert!(create.if_not_exists);
        assert_eq!(create.on_completion, Some(EventOnCompletion::NotPreserve));
        assert_eq!(create.status, Some(EventStatus::Enable));
        assert!(create.comment.is_some());

        // Both replica spellings round-trip (MySQL 8.4 admits SLAVE and REPLICA).
        let slave =
            mysql_round_trips("CREATE EVENT e ON SCHEDULE AT NOW() DISABLE ON SLAVE DO BEGIN END");
        let Statement::CreateEvent { create, .. } = &slave.statements()[0] else {
            panic!("expected a CREATE EVENT");
        };
        assert_eq!(
            create.status,
            Some(EventStatus::DisableOnReplica(ReplicaSpelling::Slave))
        );
        mysql_round_trips("CREATE EVENT e ON SCHEDULE AT NOW() DISABLE ON REPLICA DO BEGIN END");
        mysql_round_trips(
            "CREATE EVENT e ON SCHEDULE AT NOW() ON COMPLETION PRESERVE DO BEGIN END",
        );

        // The clause order is fixed: a COMMENT before the status is a syntax error (server 1064).
        parse_with(
            "CREATE EVENT e ON SCHEDULE AT NOW() COMMENT 'c' ENABLE DO BEGIN END",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("COMMENT must follow the status clause");
        // An event body carries no return value; RETURN is rejected (server ER_SP_BADRETURN).
        parse_with(
            "CREATE EVENT e ON SCHEDULE AT NOW() DO RETURN 1",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("RETURN is rejected in an event body");
    }

    #[test]
    fn mysql_alter_event_clauses_parse_and_round_trip() {
        // The measured `ALTER EVENT` family probe (status-only).
        let parsed = mysql_round_trips("ALTER EVENT zzp_e DISABLE");
        let Statement::AlterEvent { alter, .. } = &parsed.statements()[0] else {
            panic!("expected an ALTER EVENT");
        };
        assert_eq!(alter.status, Some(EventStatus::Disable));

        mysql_round_trips("ALTER EVENT e RENAME TO f");
        mysql_round_trips("ALTER EVENT e ON SCHEDULE EVERY 1 HOUR");
        // ON COMPLETION alone (no schedule) is legal in ALTER, unlike CREATE.
        mysql_round_trips("ALTER EVENT e ON COMPLETION PRESERVE");
        mysql_round_trips(
            "ALTER DEFINER = root EVENT e ON SCHEDULE EVERY 1 HOUR ON COMPLETION PRESERVE \
             RENAME TO f ENABLE COMMENT 'c' DO BEGIN END",
        );

        // At least one clause is required: a bare `ALTER EVENT e` is a syntax error (server 1064).
        parse_with("ALTER EVENT e", crate::ParseConfig::new(MySql))
            .expect_err("ALTER EVENT requires at least one clause");
    }

    #[test]
    fn mysql_drop_event_parses_and_round_trips() {
        // The measured `DROP EVENT` family probe.
        let parsed = mysql_round_trips("DROP EVENT zzp_e");
        let Statement::DropEvent { drop, .. } = &parsed.statements()[0] else {
            panic!("expected a DROP EVENT");
        };
        assert!(!drop.if_exists);
        let guarded = mysql_round_trips("DROP EVENT IF EXISTS e");
        let Statement::DropEvent { drop, .. } = &guarded.statements()[0] else {
            panic!("expected a DROP EVENT");
        };
        assert!(drop.if_exists);
        // A single name only — no comma list, no CASCADE/RESTRICT (server 1064).
        parse_with("DROP EVENT a, b", crate::ParseConfig::new(MySql))
            .expect_err("DROP EVENT names exactly one event");
        parse_with("DROP EVENT e CASCADE", crate::ParseConfig::new(MySql))
            .expect_err("DROP EVENT takes no drop behaviour");
    }

    #[test]
    fn non_mysql_dialects_reject_event_ddl() {
        // Event DDL rides `compound_statements` (off for ANSI/PostgreSQL): `EVENT` falls through
        // to the ordinary CREATE/ALTER/DROP paths and rejects.
        parse_with(
            "CREATE EVENT e ON SCHEDULE AT NOW() DO BEGIN END",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no CREATE EVENT");
        parse_with("ALTER EVENT e DISABLE", crate::ParseConfig::new(PG_DIALECT))
            .expect_err("PostgreSQL has no ALTER EVENT");
        parse_with("DROP EVENT e", crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no DROP EVENT");
    }

    #[test]
    fn non_mysql_dialects_reject_stored_routine_ddl() {
        // The routine-with-body surface rides `compound_statements` (off for ANSI/PostgreSQL):
        // `PROCEDURE`/`DEFINER` fall through to the `TABLE` expectation and reject.
        parse_with(
            "CREATE PROCEDURE p() BEGIN END",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no CREATE PROCEDURE");
        parse_with(
            "CREATE PROCEDURE p() BEGIN END",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect_err("PostgreSQL has no MySQL CREATE PROCEDURE body");
        parse_with(
            "ALTER PROCEDURE p COMMENT 'x'",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no ALTER PROCEDURE");
        // The PostgreSQL string-body / RETURN-expr CREATE FUNCTION is unaffected.
        parse_with(
            "CREATE FUNCTION f() RETURNS INTEGER LANGUAGE sql RETURN 1",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("PostgreSQL RETURN-expr routine still parses");
    }

    /// The parameter-default grammar is definition-site only: an ordinary function **call**
    /// (a distinct `parse_function_arg` / `FunctionArg` path) is untouched and round-trips as
    /// before, proving the default slot does not leak into call-argument parsing.
    #[test]
    fn function_call_arguments_are_unaffected_by_parameter_defaults() {
        for sql in [
            "SELECT f(1 + 2, 'x')",
            "SELECT sqrt(4)",
            "SELECT COALESCE(a, b, c) FROM t",
        ] {
            let pg = parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .unwrap_or_else(|err| panic!("PARSE {sql:?}: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&pg)
                .unwrap_or_else(|err| panic!("RENDER {sql:?}: {err:?}"));
            assert_eq!(rendered, sql, "call round-trip {sql:?}");
        }
    }

    /// A SQL-standard `RETURN <expr>` body (`opt_routine_body`, PostgreSQL 14+) parses onto the
    /// trailing [`CreateFunction::body`] slot as [`FunctionBody::Return`] carrying a live
    /// [`Expr`], and round-trips. It is a *distinct* body kind from the `AS` string body: the
    /// `RETURN` body never lands in the option list, and the string body never lands in the
    /// trailing slot.
    #[test]
    fn create_function_return_expression_body_rides_axis_and_round_trips() {
        for sql in [
            "CREATE FUNCTION f() RETURNS INTEGER RETURN 1",
            "CREATE FUNCTION f(a INTEGER) RETURNS INTEGER RETURN a + 1",
            "CREATE OR REPLACE FUNCTION f() RETURNS INTEGER RETURN 42",
            "CREATE FUNCTION f() RETURNS INTEGER RETURN (SELECT max(x) FROM t)",
        ] {
            let pg = parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .unwrap_or_else(|err| panic!("PARSE {sql:?}: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&pg)
                .unwrap_or_else(|err| panic!("RENDER {sql:?}: {err:?}"));
            assert_eq!(rendered, sql, "round-trip {sql:?}");

            let [Statement::CreateFunction { create, .. }] = pg.statements() else {
                panic!("{sql:?}: expected one CREATE FUNCTION");
            };
            let body = create
                .body
                .as_ref()
                .unwrap_or_else(|| panic!("{sql:?}: expected a trailing RETURN body"));
            assert!(
                matches!(**body, FunctionBody::Return { .. }),
                "{sql:?}: trailing body is a live RETURN expression",
            );
            assert!(
                create.options.is_empty(),
                "{sql:?}: the RETURN body is the trailing slot, not an option",
            );
        }
    }

    /// The `RETURN` body is the trailing `opt_routine_body`, a disjoint grammatical slot that
    /// strictly *follows* the whole option list. Proven against the PostgreSQL oracle
    /// (`pg_query`): `... LANGUAGE sql RETURN 1` accepts (option then body) but
    /// `... RETURN 1 LANGUAGE sql` rejects (a body cannot precede an option). The parser mirrors
    /// both verdicts, and the accepted form keeps the `LANGUAGE` option in the option list with
    /// the `RETURN` in the trailing slot.
    #[test]
    fn create_function_return_body_is_a_trailing_slot_after_options() {
        let ok = "CREATE FUNCTION f() RETURNS INTEGER LANGUAGE sql RETURN 1";
        let pg = parse_with(ok, crate::ParseConfig::new(PG_DIALECT))
            .unwrap_or_else(|err| panic!("PARSE {ok:?}: {err:?}"));
        let rendered = Renderer::new(PG_DIALECT)
            .render_parsed(&pg)
            .unwrap_or_else(|err| panic!("RENDER {ok:?}: {err:?}"));
        assert_eq!(rendered, ok, "round-trip {ok:?}");
        let [Statement::CreateFunction { create, .. }] = pg.statements() else {
            panic!("{ok:?}: expected one CREATE FUNCTION");
        };
        assert_eq!(create.options.len(), 1, "LANGUAGE stays an option");
        assert!(create.body.is_some(), "RETURN body in the trailing slot");

        // A `RETURN` body before an option is rejected — it is strictly trailing (the oracle
        // rejects this exact spelling).
        parse_with(
            "CREATE FUNCTION f() RETURNS INTEGER RETURN 1 LANGUAGE sql",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect_err("a RETURN body cannot precede an option");
    }

    /// The `RETURN` expression body rides the `routines` DDL gate with no narrowing: MySQL has
    /// stored routines (gate on) and its `routine_body` admits a `RETURN <expr>` statement, so
    /// MySQL accepts the same spelling. SQLite has no stored routines (gate off), so the whole
    /// `CREATE FUNCTION` never reaches the routine parser and is rejected.
    #[test]
    fn create_function_return_body_rides_routines_gate() {
        use crate::dialect::Sqlite;
        parse_with(
            "CREATE FUNCTION f() RETURNS INT RETURN 1",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL routines accept a RETURN expression body");
        parse_with(
            "CREATE FUNCTION f() RETURNS INT RETURN 1",
            crate::ParseConfig::new(Sqlite),
        )
        .expect_err("SQLite has no stored routines");
    }

    /// PostgreSQL with drop behaviour disabled, to prove `CASCADE`/`RESTRICT` is gated.
    const NO_DROP_BEHAVIOR: FeatureSet =
        FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.index_alter_syntax(IndexAlterSyntax {
            drop_behavior: false,
            ..IndexAlterSyntax::POSTGRES
        }));

    const NO_DROP_BEHAVIOR_DIALECT: FeatureDialect = FeatureDialect {
        features: &NO_DROP_BEHAVIOR,
    };

    fn alter_of(parsed: &Parsed) -> &AlterTable<NoExt> {
        let Statement::AlterTable { alter, .. } = &parsed.statements()[0] else {
            panic!("expected an ALTER TABLE statement");
        };
        alter
    }

    fn drop_of(parsed: &Parsed) -> &DropStatement {
        let Statement::Drop { drop, .. } = &parsed.statements()[0] else {
            panic!("expected a DROP statement");
        };
        drop
    }

    fn create_table_of(parsed: &Parsed) -> &CreateTable<NoExt> {
        let Statement::CreateTable { create, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE TABLE statement");
        };
        create
    }

    fn create_schema_of(parsed: &Parsed) -> &CreateSchema {
        let Statement::CreateSchema { schema, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE SCHEMA statement");
        };
        schema
    }

    fn create_view_of(parsed: &Parsed) -> &CreateView<NoExt> {
        let Statement::CreateView { view, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE VIEW statement");
        };
        view
    }

    fn create_index_of(parsed: &Parsed) -> &CreateIndex<NoExt> {
        let Statement::CreateIndex { index, .. } = &parsed.statements()[0] else {
            panic!("expected a CREATE INDEX statement");
        };
        index
    }

    /// The dispatch contract: every DDL leading form is routed by the central
    /// `parse_statement` to this module — `CREATE` fans out to the table/schema/view/
    /// index entries, and `ALTER`/`DROP` to their own — each yielding the family's
    /// variant. The `*_of` helpers panic on any other variant, pinning the boundary.
    #[test]
    fn dispatch_routes_ddl_keywords_to_this_family() {
        let _ = create_table_of(
            &parse_with(
                "CREATE TABLE t (id INT)",
                crate::ParseConfig::new(TestDialect),
            )
            .expect("table"),
        );
        let _ = create_schema_of(
            &parse_with("CREATE SCHEMA s", crate::ParseConfig::new(TestDialect)).expect("schema"),
        );
        let _ = create_view_of(
            &parse_with(
                "CREATE VIEW v AS SELECT 1",
                crate::ParseConfig::new(TestDialect),
            )
            .expect("view"),
        );
        let _ = create_index_of(
            &parse_with(
                "CREATE INDEX i ON t (a)",
                crate::ParseConfig::new(TestDialect),
            )
            .expect("index"),
        );
        let _ = alter_of(
            &parse_with(
                "ALTER TABLE t ADD COLUMN c INT",
                crate::ParseConfig::new(TestDialect),
            )
            .expect("alter"),
        );
        let _ = drop_of(
            &parse_with("DROP TABLE t", crate::ParseConfig::new(TestDialect)).expect("drop"),
        );
    }

    #[test]
    fn create_table_definition_parses_columns_and_constraints() {
        let sql = "CREATE TABLE t (\
            id INT PRIMARY KEY, \
            name TEXT NOT NULL DEFAULT 'x', \
            n INT GENERATED ALWAYS AS (id + 1) STORED, \
            ident BIGINT GENERATED BY DEFAULT AS IDENTITY, \
            CONSTRAINT u UNIQUE (name), \
            CHECK (id > 0)\
        )";
        let parsed =
            parse_with(sql, crate::ParseConfig::new(TestDialect)).expect("CREATE TABLE parses");
        let create = create_table_of(&parsed);

        assert_eq!(create.temporary, None);
        assert!(!create.if_not_exists);
        assert_eq!(parsed.resolver().resolve(create.name.0[0].sym), "t");

        let CreateTableBody::Definition { elements, .. } = &create.body else {
            panic!("expected a table definition body");
        };
        assert_eq!(elements.len(), 6);

        let TableElement::Column { column: id, .. } = &elements[0] else {
            panic!("expected id column");
        };
        assert_eq!(parsed.resolver().resolve(id.name.sym), "id");
        assert!(matches!(
            &id.data_type,
            Some(DataType::Integer {
                spelling: IntegerTypeName::Int,
                ..
            })
        ));
        assert!(matches!(
            &id.constraints[0].option,
            ColumnOption::PrimaryKey { .. }
        ));

        let TableElement::Column { column: name, .. } = &elements[1] else {
            panic!("expected name column");
        };
        assert!(matches!(&name.data_type, Some(DataType::Text { .. })));
        assert!(matches!(
            &name.constraints[0].option,
            ColumnOption::NotNull { .. }
        ));
        let ColumnOption::Default { expr: default, .. } = &name.constraints[1].option else {
            panic!("expected a DEFAULT constraint");
        };
        assert!(matches!(
            &**default,
            Expr::Literal {
                literal: crate::ast::Literal {
                    kind: LiteralKind::String,
                    ..
                },
                ..
            }
        ));

        let TableElement::Column {
            column: generated, ..
        } = &elements[2]
        else {
            panic!("expected generated column");
        };
        let ColumnOption::Generated {
            generated: generated_option,
            ..
        } = &generated.constraints[0].option
        else {
            panic!("expected generated column option");
        };
        assert_eq!(
            generated_option.storage,
            Some(GeneratedColumnStorage::Stored)
        );
        assert!(matches!(
            &generated_option.expr,
            Expr::BinaryOp {
                op: BinaryOperator::Plus,
                ..
            }
        ));

        let TableElement::Column {
            column: identity, ..
        } = &elements[3]
        else {
            panic!("expected identity column");
        };
        let ColumnOption::Identity {
            identity: identity_option,
            ..
        } = &identity.constraints[0].option
        else {
            panic!("expected identity option");
        };
        assert_eq!(identity_option.generation, IdentityGeneration::ByDefault);
        assert!(identity_option.options.is_empty());

        let TableElement::Constraint {
            constraint: unique, ..
        } = &elements[4]
        else {
            panic!("expected table UNIQUE constraint");
        };
        assert_eq!(
            parsed
                .resolver()
                .resolve(unique.name.as_ref().expect("named constraint").sym),
            "u",
        );
        let TableConstraint::Unique { columns, .. } = &unique.constraint else {
            panic!("expected UNIQUE table constraint");
        };
        let Expr::Column { name, .. } = &columns[0].expr else {
            panic!("expected a bare column key");
        };
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "name");

        let TableElement::Constraint {
            constraint: check, ..
        } = &elements[5]
        else {
            panic!("expected table CHECK constraint");
        };
        assert!(matches!(&check.constraint, TableConstraint::Check { .. }));
    }

    #[test]
    fn create_table_trailing_comma_gated_to_duckdb() {
        use crate::dialect::{DuckDb, Postgres, Sqlite};
        // DuckDB tolerates a single trailing comma before the closing `)`, after a column
        // or a constraint element alike (engine-probed on 1.5.4); the comma is discarded,
        // so the element list shape is unchanged (a bare two-element table here).
        for sql in [
            "CREATE TABLE t (a INT, b INT,)",
            "CREATE TABLE t (a INT,)",
            "CREATE TABLE t (a INT, PRIMARY KEY (a),)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|e| panic!("DuckDB {sql:?}: {e:?}"));
            let CreateTableBody::Definition { elements, .. } = &create_table_of(&parsed).body
            else {
                panic!("expected a table definition body for {sql:?}");
            };
            assert!(!elements.is_empty(), "{sql:?} kept its elements");
        }
        // Only a single trailing comma; a doubled or leading comma stays a parse error.
        parse_with(
            "CREATE TABLE t (a INT, b INT,,)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("doubled trailing comma rejects");
        parse_with("CREATE TABLE t (,)", crate::ParseConfig::new(DuckDb))
            .expect_err("empty-after-comma rejects");
        parse_with("CREATE TABLE t (, a INT)", crate::ParseConfig::new(DuckDb))
            .expect_err("leading comma rejects");

        // Flag off elsewhere: the dangling comma falls to the element parser and yields the
        // standard clean parse error — the same reject the engines report.
        let sql = "CREATE TABLE t (a INT, b INT,)";
        parse_with(sql, crate::ParseConfig::new(Ansi))
            .expect_err("ANSI rejects the trailing comma");
        parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect_err("PostgreSQL rejects the trailing comma");
        parse_with(sql, crate::ParseConfig::new(MySql))
            .expect_err("MySQL rejects the trailing comma");
        parse_with(sql, crate::ParseConfig::new(Sqlite))
            .expect_err("SQLite rejects the trailing comma");
    }

    #[test]
    fn create_temp_table_as_select_parses_with_no_data() {
        let parsed = parse_with(
            "CREATE TEMP TABLE IF NOT EXISTS t AS SELECT 1 WITH NO DATA",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("CTAS parses");
        let create = create_table_of(&parsed);

        assert_eq!(create.temporary, Some(TemporaryTableKind::Temp));
        assert!(create.if_not_exists);
        let CreateTableBody::AsQuery {
            columns,
            query,
            with_data,
            ..
        } = &create.body
        else {
            panic!("expected CTAS body");
        };
        assert!(columns.is_empty());
        assert_eq!(*with_data, Some(false));
        assert!(matches!(&query.body, SetExpr::Select { .. }));
    }

    #[test]
    fn create_table_as_select_parses_columns_and_options_before_as() {
        let parsed = parse_with(
            "CREATE TEMP TABLE t (id) ON COMMIT DROP AS SELECT 1 WITH DATA",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("CTAS columns and options parse");
        let create = create_table_of(&parsed);

        let CreateTableBody::AsQuery {
            columns,
            query,
            with_data,
            ..
        } = &create.body
        else {
            panic!("expected CTAS body");
        };
        assert_eq!(columns.len(), 1);
        assert_eq!(parsed.resolver().resolve(columns[0].sym), "id");
        assert!(matches!(&query.body, SetExpr::Select { .. }));
        assert_eq!(*with_data, Some(true));
        assert_eq!(create.options.len(), 1);
        assert!(matches!(
            &create.options[0].kind,
            CreateTableOptionKind::OnCommit {
                action: OnCommitAction::Drop,
                ..
            }
        ));
    }

    #[test]
    fn create_table_as_select_rejects_typed_columns_before_as() {
        parse_with(
            "CREATE TABLE t (id INT) AS SELECT 1",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("PostgreSQL rejects typed column definitions before CTAS AS");
    }

    #[test]
    fn create_table_options_parse_after_definition() {
        let parsed = parse_with(
            "CREATE TEMPORARY TABLE t (id INT) \
             WITH (fillfactor = 70) ON COMMIT DROP TABLESPACE pg_default",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("CREATE TABLE options parse");
        let create = create_table_of(&parsed);

        assert_eq!(create.temporary, Some(TemporaryTableKind::Temporary));
        assert_eq!(create.options.len(), 3);
        let CreateTableOptionKind::With { params, .. } = &create.options[0].kind else {
            panic!("expected WITH storage parameters");
        };
        assert_eq!(
            parsed.resolver().resolve(params[0].name.0[0].sym),
            "fillfactor"
        );
        assert!(matches!(
            &params[0].value,
            Some(Expr::Literal {
                literal: crate::ast::Literal {
                    kind: LiteralKind::Integer,
                    ..
                },
                ..
            })
        ));
        assert!(matches!(
            &create.options[1].kind,
            CreateTableOptionKind::OnCommit {
                action: OnCommitAction::Drop,
                ..
            }
        ));
        let CreateTableOptionKind::Tablespace { tablespace, .. } = &create.options[2].kind else {
            panic!("expected TABLESPACE option");
        };
        assert_eq!(parsed.resolver().resolve(tablespace.sym), "pg_default");
    }

    #[test]
    fn mysql_create_table_options_and_auto_increment_column() {
        let sql = "CREATE TABLE t (id INT AUTO_INCREMENT PRIMARY KEY) \
                   ENGINE=InnoDB AUTO_INCREMENT=100 DEFAULT CHARSET=utf8mb4 COMMENT='x'";
        let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
            .expect("MySQL CREATE TABLE options parse");
        let create = create_table_of(&parsed);

        // The column carries the `AUTO_INCREMENT` attribute alongside `PRIMARY KEY`.
        let CreateTableBody::Definition { elements, .. } = &create.body else {
            panic!("expected a table definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column element");
        };
        assert!(matches!(
            column.constraints[0].option,
            ColumnOption::AutoIncrement { .. }
        ));
        assert!(matches!(
            column.constraints[1].option,
            ColumnOption::PrimaryKey { .. }
        ));

        // Four trailing options, all modelled as the open key/value shape. The
        // `DEFAULT` prefix on `DEFAULT CHARSET` is normalized away (ADR-0011).
        let options: Vec<(&str, &TableOptionValue)> = create
            .options
            .iter()
            .map(|option| match &option.kind {
                CreateTableOptionKind::KeyValue { option, .. } => {
                    (parsed.resolver().resolve(option.name.sym), &option.value)
                }
                other => panic!("expected a MySQL key/value option, got {other:?}"),
            })
            .collect();
        assert_eq!(options.len(), 4);

        assert_eq!(options[0].0, "ENGINE");
        let TableOptionValue::Word { word, .. } = options[0].1 else {
            panic!("ENGINE value should be a bareword");
        };
        assert_eq!(parsed.resolver().resolve(word.sym), "InnoDB");

        assert_eq!(options[1].0, "AUTO_INCREMENT");
        assert!(matches!(
            options[1].1,
            TableOptionValue::Number {
                value: crate::ast::Literal {
                    kind: LiteralKind::Integer,
                    ..
                },
                ..
            }
        ));

        assert_eq!(options[2].0, "CHARSET");
        let TableOptionValue::Word { word, .. } = options[2].1 else {
            panic!("CHARSET value should be a bareword");
        };
        assert_eq!(parsed.resolver().resolve(word.sym), "utf8mb4");

        assert_eq!(options[3].0, "COMMENT");
        assert!(matches!(
            options[3].1,
            TableOptionValue::String {
                value: crate::ast::Literal {
                    kind: LiteralKind::String,
                    ..
                },
                ..
            }
        ));
    }

    #[test]
    fn mysql_table_options_accept_optional_equals_and_keyword_values() {
        // The `=` is optional and a value may be a keyword (`ROW_FORMAT DYNAMIC`); both
        // collapse to the same canonical key/value shape.
        let parsed = parse_with(
            "CREATE TABLE t (id INT) ENGINE InnoDB ROW_FORMAT = DYNAMIC",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL accepts the `=`-less and keyword-valued forms");
        let create = create_table_of(&parsed);
        assert_eq!(create.options.len(), 2);
        for element in &create.options {
            let CreateTableOptionKind::KeyValue { option, .. } = &element.kind else {
                panic!("expected a MySQL key/value option");
            };
            assert!(matches!(option.value, TableOptionValue::Word { .. }));
        }
    }

    #[test]
    fn ansi_and_postgres_reject_mysql_table_options_and_auto_increment() {
        // The gating flag is off under ANSI/PostgreSQL, so the keyword is left
        // unconsumed and the trailing clause is leftover input -> parse error.
        for sql in [
            "CREATE TABLE t (id INT) ENGINE=InnoDB",
            "CREATE TABLE t (id INT AUTO_INCREMENT)",
        ] {
            parse_with(sql, crate::ParseConfig::new(Ansi))
                .expect_err("ANSI rejects MySQL CREATE TABLE storage syntax");
            parse_with(sql, crate::ParseConfig::new(TestDialect))
                .expect_err("PostgreSQL rejects MySQL CREATE TABLE storage syntax");
        }
    }

    /// The `ForeignKeyRef` from the first column's first constraint.
    fn column_reference_of(parsed: &Parsed) -> &ForeignKeyRef {
        let create = create_table_of(parsed);
        let CreateTableBody::Definition { elements, .. } = &create.body else {
            panic!("expected a table definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column element");
        };
        let ColumnOption::References { reference, .. } = &column.constraints[0].option else {
            panic!("expected a REFERENCES column constraint");
        };
        reference
    }

    #[test]
    fn foreign_key_referential_actions_parse() {
        // Regression: before referential actions were modelled, the trailing `ON
        // DELETE` / `ON UPDATE` clauses were a parse error even though PostgreSQL
        // accepts them.
        let parsed = parse_with(
            "CREATE TABLE t (a INT REFERENCES p (id) ON DELETE CASCADE ON UPDATE SET NULL)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("REFERENCES with ON DELETE/ON UPDATE parses");
        let reference = column_reference_of(&parsed);

        assert_eq!(parsed.resolver().resolve(reference.table.0[0].sym), "p");
        assert_eq!(reference.match_type, None);
        assert!(matches!(
            reference.on_delete.as_deref(),
            Some(ReferentialAction::Cascade { .. })
        ));
        let Some(ReferentialAction::SetNull { columns, .. }) = reference.on_update.as_deref()
        else {
            panic!("expected ON UPDATE SET NULL");
        };
        // The `SET NULL` column list is valid only on `ON DELETE`, so an `ON UPDATE`
        // action never carries one.
        assert!(columns.is_empty());
    }

    /// The six DuckDB grammar over-accept classes closed by
    /// `duckdb-parser-over-accept-tighten` (engine-probed on DuckDB 1.5.4).
    #[test]
    fn duckdb_rejects_the_tranche2_grammar_over_accepts() {
        use crate::dialect::{DuckDb, Postgres};

        // (1) FK cascading actions — RESTRICT / NO ACTION still parse.
        parse_with(
            "CREATE TABLE c (id INT REFERENCES p (id) ON DELETE CASCADE)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("DuckDB rejects ON DELETE CASCADE");
        parse_with(
            "CREATE TABLE c (id INT REFERENCES p (id) ON UPDATE SET NULL)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("DuckDB rejects ON UPDATE SET NULL");
        parse_with(
            "CREATE TABLE c (id INT REFERENCES p (id) ON DELETE SET DEFAULT)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("DuckDB rejects ON DELETE SET DEFAULT");
        parse_with(
            "CREATE TABLE c (id INT REFERENCES p (id) ON DELETE RESTRICT)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDB still admits ON DELETE RESTRICT");
        parse_with(
            "CREATE TABLE c (id INT REFERENCES p (id) ON DELETE NO ACTION)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDB still admits ON DELETE NO ACTION");
        // PostgreSQL keeps the cascading surface.
        parse_with(
            "CREATE TABLE c (id INT REFERENCES p (id) ON DELETE CASCADE)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("PostgreSQL admits ON DELETE CASCADE");

        // (2) Subqueries in CHECK.
        parse_with(
            "CREATE TABLE t (x INT CHECK (x IN (SELECT 1)))",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("DuckDB rejects subquery in CHECK");
        parse_with(
            "CREATE TABLE t (x INT CHECK (x > 0))",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDB admits plain CHECK");

        // (3) CREATE DATABASE — DuckDB uses ATTACH.
        parse_with("CREATE DATABASE mydb", crate::ParseConfig::new(DuckDb))
            .expect_err("DuckDB has no CREATE DATABASE");
        parse_with("CREATE DATABASE mydb", crate::ParseConfig::new(Postgres))
            .expect("PostgreSQL admits CREATE DATABASE");

        // (4) OR REPLACE + IF NOT EXISTS mutual exclusion on TABLE.
        parse_with(
            "CREATE OR REPLACE TABLE IF NOT EXISTS integers (i INTEGER)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("DuckDB rejects OR REPLACE + IF NOT EXISTS on TABLE");
        parse_with(
            "CREATE OR REPLACE TABLE integers (i INTEGER)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDB admits bare OR REPLACE TABLE");

        // (5) Multi-action ALTER TABLE.
        parse_with(
            "ALTER TABLE t ADD COLUMN j INT, DROP COLUMN j",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("DuckDB rejects multi-action ALTER");
        parse_with(
            "ALTER TABLE t ADD COLUMN j INT",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDB admits single-action ALTER");
        parse_with(
            "ALTER TABLE t ADD COLUMN j INT, DROP COLUMN j",
            crate::ParseConfig::new(Postgres),
        )
        .expect("PostgreSQL admits multi-action ALTER");
    }

    #[test]
    fn foreign_key_actions_are_order_independent() {
        // The referential actions parse in either order; a `MATCH` type, when present,
        // must precede them (PG parity). Here `ON UPDATE` is written before `ON DELETE`.
        let parsed = parse_with(
            "CREATE TABLE t (a INT, b INT, \
             FOREIGN KEY (a, b) REFERENCES p (x, y) \
             MATCH FULL ON UPDATE RESTRICT ON DELETE SET NULL (a, b))",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("order-independent FK actions parse");
        let create = create_table_of(&parsed);
        let CreateTableBody::Definition { elements, .. } = &create.body else {
            panic!("expected a table definition body");
        };
        let TableElement::Constraint { constraint, .. } = &elements[2] else {
            panic!("expected a table constraint element");
        };
        let TableConstraint::ForeignKey { references, .. } = &constraint.constraint else {
            panic!("expected a FOREIGN KEY constraint");
        };

        assert_eq!(references.match_type, Some(ForeignKeyMatch::Full));
        assert!(matches!(
            references.on_update.as_deref(),
            Some(ReferentialAction::Restrict { .. })
        ));
        let Some(ReferentialAction::SetNull { columns, .. }) = references.on_delete.as_deref()
        else {
            panic!("expected ON DELETE SET NULL (a, b)");
        };
        assert_eq!(columns.len(), 2);
        assert_eq!(parsed.resolver().resolve(columns[0].sym), "a");
        assert_eq!(parsed.resolver().resolve(columns[1].sym), "b");
    }

    #[test]
    fn foreign_key_match_after_actions_is_rejected() {
        // PostgreSQL's grammar puts `MATCH` before the referential actions; a `MATCH`
        // after `ON UPDATE` / `ON DELETE` is a syntax error there, so the parser rejects
        // it to match (run-pg-accept-reject-over-vendored-corpora). The MATCH-first
        // spelling stays accepted (see `foreign_key_actions_are_order_independent`).
        for sql in [
            "CREATE TABLE t (a INT REFERENCES p ON DELETE CASCADE MATCH FULL)",
            "CREATE TABLE t (a INT, b INT, FOREIGN KEY (a, b) REFERENCES p \
             ON UPDATE NO ACTION ON DELETE NO ACTION MATCH FULL)",
        ] {
            parse_with(sql, crate::ParseConfig::new(TestDialect))
                .expect_err(&format!("MATCH after actions is rejected: {sql:?}"));
        }
    }

    #[test]
    fn foreign_key_match_partial_is_rejected() {
        // `MATCH PARTIAL` is standard-SQL syntax, but PostgreSQL rejects it at parse time
        // ("MATCH PARTIAL not yet implemented"); we match that verdict. `MATCH FULL` and
        // `MATCH SIMPLE` stay accepted.
        parse_with(
            "CREATE TABLE t (a INT REFERENCES p MATCH PARTIAL)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("MATCH PARTIAL is rejected");
        assert_eq!(
            column_reference_of(
                &parse_with(
                    "CREATE TABLE t (a INT REFERENCES p MATCH FULL)",
                    crate::ParseConfig::new(TestDialect)
                )
                .expect("MATCH FULL parses"),
            )
            .match_type,
            Some(ForeignKeyMatch::Full)
        );
        assert_eq!(
            column_reference_of(
                &parse_with(
                    "CREATE TABLE t (a INT REFERENCES p MATCH SIMPLE)",
                    crate::ParseConfig::new(TestDialect),
                )
                .expect("MATCH SIMPLE parses"),
            )
            .match_type,
            Some(ForeignKeyMatch::Simple)
        );
    }

    #[test]
    fn foreign_key_malformed_actions_are_rejected() {
        for sql in [
            "CREATE TABLE t (a INT REFERENCES p ON DELETE)", // missing action
            "CREATE TABLE t (a INT REFERENCES p ON DELETE BOGUS)", // unknown action
            "CREATE TABLE t (a INT REFERENCES p MATCH BOGUS)", // unknown match type
            "CREATE TABLE t (a INT REFERENCES p ON DELETE SET BOGUS)", // bad SET target
            "CREATE TABLE t (a INT REFERENCES p ON UPDATE SET NULL (a))", // list only on ON DELETE
            "CREATE TABLE t (a INT REFERENCES p ON DELETE CASCADE ON DELETE SET NULL)", // duplicate
        ] {
            parse_with(sql, crate::ParseConfig::new(TestDialect))
                .expect_err(&format!("should reject {sql:?}"));
        }
    }

    #[test]
    fn identity_options_parse_in_sequence() {
        let parsed = parse_with(
            "CREATE TABLE t (id BIGINT GENERATED ALWAYS AS IDENTITY \
             (START WITH 10 INCREMENT BY 2 NO MINVALUE MAXVALUE 100 CACHE 5 NO CYCLE))",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("identity options parse");
        let create = create_table_of(&parsed);
        let CreateTableBody::Definition { elements, .. } = &create.body else {
            panic!("expected table definition");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected identity column");
        };
        let ColumnOption::Identity { identity, .. } = &column.constraints[0].option else {
            panic!("expected identity option");
        };

        assert_eq!(identity.generation, IdentityGeneration::Always);
        assert_eq!(identity.options.len(), 6);
        assert!(matches!(
            &identity.options[0],
            IdentityOption::StartWith { .. }
        ));
        assert!(matches!(
            &identity.options[1],
            IdentityOption::IncrementBy { .. }
        ));
        assert!(matches!(
            &identity.options[2],
            IdentityOption::MinValue { value: None, .. }
        ));
        assert!(matches!(
            &identity.options[3],
            IdentityOption::MaxValue { value: Some(_), .. }
        ));
        assert!(matches!(&identity.options[4], IdentityOption::Cache { .. }));
        assert!(matches!(
            &identity.options[5],
            IdentityOption::Cycle { cycle: false, .. }
        ));
    }

    #[test]
    fn alter_table_parses_add_drop_alter_actions_and_constraints() {
        let parsed = parse_with(
            "ALTER TABLE s.t \
             ADD COLUMN a INT NOT NULL, \
             ADD b TEXT, \
             DROP COLUMN c CASCADE, \
             ADD CONSTRAINT u UNIQUE (a), \
             ADD PRIMARY KEY (a), \
             DROP CONSTRAINT old_pk RESTRICT, \
             ALTER COLUMN a SET DEFAULT 0, \
             ALTER b DROP NOT NULL",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("multi-action ALTER TABLE parses");
        let alter = alter_of(&parsed);

        assert!(!alter.if_exists);
        assert_eq!(parsed.resolver().resolve(alter.name.0[0].sym), "s");
        assert_eq!(parsed.resolver().resolve(alter.name.0[1].sym), "t");
        assert_eq!(alter.actions.len(), 8);

        let AlterTableAction::AddColumn {
            if_not_exists,
            column,
            ..
        } = &alter.actions[0]
        else {
            panic!("expected ADD COLUMN");
        };
        assert!(!if_not_exists);
        assert_eq!(parsed.resolver().resolve(column.name.sym), "a");
        assert!(matches!(
            column.constraints[0].option,
            ColumnOption::NotNull { .. }
        ));

        // `ADD b TEXT` without the optional COLUMN keyword still parses as a column.
        assert!(matches!(
            &alter.actions[1],
            AlterTableAction::AddColumn { .. }
        ));

        let AlterTableAction::DropColumn { name, behavior, .. } = &alter.actions[2] else {
            panic!("expected DROP COLUMN");
        };
        assert_eq!(parsed.resolver().resolve(name.parts[0].sym), "c");
        assert_eq!(*behavior, Some(DropBehavior::Cascade));

        assert!(matches!(
            &alter.actions[3],
            AlterTableAction::AddConstraint { .. }
        ));
        // `ADD PRIMARY KEY (...)` without CONSTRAINT is still a table constraint.
        assert!(matches!(
            &alter.actions[4],
            AlterTableAction::AddConstraint { .. }
        ));

        let AlterTableAction::DropConstraint { name, behavior, .. } = &alter.actions[5] else {
            panic!("expected DROP CONSTRAINT");
        };
        assert_eq!(parsed.resolver().resolve(name.sym), "old_pk");
        assert_eq!(*behavior, Some(DropBehavior::Restrict));

        let AlterTableAction::AlterColumn { change, .. } = &alter.actions[6] else {
            panic!("expected ALTER COLUMN");
        };
        assert!(matches!(change, AlterColumnAction::SetDefault { .. }));

        let AlterTableAction::AlterColumn { change, .. } = &alter.actions[7] else {
            panic!("expected ALTER COLUMN");
        };
        assert!(matches!(change, AlterColumnAction::DropNotNull { .. }));
    }

    #[test]
    fn alter_column_type_and_set_drop_forms_parse() {
        let parsed = parse_with(
            "ALTER TABLE t \
             ALTER COLUMN a SET DATA TYPE BIGINT, \
             ALTER COLUMN b TYPE TEXT USING b::TEXT, \
             ALTER COLUMN c SET NOT NULL, \
             ALTER COLUMN d DROP DEFAULT",
            // `b::TEXT` needs the PostgreSQL typecast operator.
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("ALTER COLUMN type/set/drop forms parse");
        let alter = alter_of(&parsed);
        assert_eq!(alter.actions.len(), 4);

        let AlterTableAction::AlterColumn { change, .. } = &alter.actions[0] else {
            panic!("expected ALTER COLUMN");
        };
        let AlterColumnAction::SetDataType { using, .. } = change else {
            panic!("expected SET DATA TYPE");
        };
        assert!(using.is_none());

        let AlterTableAction::AlterColumn { change, .. } = &alter.actions[1] else {
            panic!("expected ALTER COLUMN");
        };
        let AlterColumnAction::SetDataType { using, .. } = change else {
            panic!("expected TYPE alteration mapped to SET DATA TYPE");
        };
        assert!(using.is_some(), "USING conversion expression is captured");

        assert!(matches!(
            &alter.actions[2],
            AlterTableAction::AlterColumn {
                change: AlterColumnAction::SetNotNull { .. },
                ..
            }
        ));
        assert!(matches!(
            &alter.actions[3],
            AlterTableAction::AlterColumn {
                change: AlterColumnAction::DropDefault { .. },
                ..
            }
        ));
    }

    #[test]
    fn duckdb_alter_table_nested_column_paths_match_engine_boundary() {
        use crate::dialect::{DuckDb, Postgres};

        let parsed = parse_with(
            "ALTER TABLE t ADD COLUMN s.s2.j INTEGER",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDB accepts nested ADD COLUMN targets");
        let alter = alter_of(&parsed);
        let AlterTableAction::AddColumn { target, column, .. } = &alter.actions[0] else {
            panic!("expected ADD COLUMN");
        };
        let target = target.as_ref().expect("nested target preserved");
        let names: Vec<_> = target
            .parts
            .iter()
            .map(|part| parsed.resolver().resolve(part.sym))
            .collect();
        assert_eq!(names, ["s", "s2", "j"]);
        assert_eq!(parsed.resolver().resolve(column.name.sym), "j");

        let parsed = parse_with(
            "ALTER TABLE t DROP COLUMN IF EXISTS s.s2.k",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDB accepts nested DROP COLUMN targets");
        let alter = alter_of(&parsed);
        let AlterTableAction::DropColumn {
            if_exists, name, ..
        } = &alter.actions[0]
        else {
            panic!("expected DROP COLUMN");
        };
        assert!(*if_exists);
        let names: Vec<_> = name
            .parts
            .iter()
            .map(|part| parsed.resolver().resolve(part.sym))
            .collect();
        assert_eq!(names, ["s", "s2", "k"]);

        parse_with(
            "ALTER TABLE t RENAME COLUMN s.s2.k TO kk",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDB accepts nested old-side RENAME COLUMN targets");

        parse_with(
            "ALTER TABLE t ADD COLUMN s.s2.j INTEGER",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL does not parse nested ADD COLUMN targets");
        parse_with(
            "ALTER TABLE t DROP COLUMN s.s2.k",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL does not parse nested DROP COLUMN targets");
        parse_with(
            "ALTER TABLE t RENAME COLUMN s.s2.k TO kk",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL does not parse nested RENAME COLUMN targets");

        parse_with(
            "ALTER TABLE t ALTER COLUMN s.s2.k SET DATA TYPE BIGINT",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("DuckDB rejects nested ALTER COLUMN targets");
        parse_with(
            "ALTER TABLE t RENAME COLUMN s.s2.k TO s.s2.kk",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("DuckDB rejects nested RENAME COLUMN destinations");
    }

    #[test]
    fn alter_table_if_exists_guards_parse_under_postgres() {
        let parsed = parse_with(
            "ALTER TABLE IF EXISTS t \
             ADD COLUMN IF NOT EXISTS a INT, \
             DROP COLUMN IF EXISTS b, \
             DROP CONSTRAINT IF EXISTS c",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("ALTER TABLE IF EXISTS parses under PostgreSQL");
        let alter = alter_of(&parsed);
        assert!(alter.if_exists);
        assert!(matches!(
            &alter.actions[0],
            AlterTableAction::AddColumn {
                if_not_exists: true,
                ..
            }
        ));
        assert!(matches!(
            &alter.actions[1],
            AlterTableAction::DropColumn {
                if_exists: true,
                ..
            }
        ));
        assert!(matches!(
            &alter.actions[2],
            AlterTableAction::DropConstraint {
                if_exists: true,
                ..
            }
        ));
    }

    #[test]
    fn drop_family_parses_object_kinds_names_and_behaviour() {
        let parsed = parse_with(
            "DROP TABLE a, b.c RESTRICT",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("DROP TABLE parses");
        let drop = drop_of(&parsed);
        assert_eq!(drop.object_kind, DropObjectKind::Table);
        assert!(!drop.if_exists);
        assert_eq!(drop.names.len(), 2);
        assert_eq!(parsed.resolver().resolve(drop.names[0].0[0].sym), "a");
        assert_eq!(parsed.resolver().resolve(drop.names[1].0[1].sym), "c");
        assert_eq!(drop.behavior, Some(DropBehavior::Restrict));

        for (sql, kind) in [
            ("DROP VIEW v CASCADE", DropObjectKind::View),
            ("DROP INDEX i", DropObjectKind::Index),
            ("DROP SCHEMA s CASCADE", DropObjectKind::Schema),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(TestDialect))
                .unwrap_or_else(|err| panic!("{sql}: {err:?}"));
            assert_eq!(drop_of(&parsed).object_kind, kind, "{sql}");
        }
    }

    #[test]
    fn drop_if_exists_parses_under_postgres() {
        let parsed = parse_with(
            "DROP TABLE IF EXISTS t CASCADE",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("DROP TABLE IF EXISTS parses under PostgreSQL");
        let drop = drop_of(&parsed);
        assert!(drop.if_exists);
        assert_eq!(drop.behavior, Some(DropBehavior::Cascade));
    }

    #[test]
    fn ansi_rejects_if_exists_existence_guard() {
        // `IF EXISTS` is gated off under ANSI, so the guard is left unconsumed and the
        // trailing tokens surface as a parse error.
        parse_with(
            "DROP TABLE IF EXISTS t",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("ANSI has no IF EXISTS on DROP");
        parse_with(
            "ALTER TABLE IF EXISTS t DROP COLUMN c",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("ANSI has no IF EXISTS on ALTER");
        parse_with(
            "ALTER TABLE t ADD COLUMN IF NOT EXISTS c INT",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("ANSI has no IF NOT EXISTS on ADD COLUMN");
    }

    #[test]
    fn drop_behavior_is_gated_by_dialect_data() {
        // With drop behaviour disabled the trailing CASCADE is leftover input.
        parse_with(
            "DROP TABLE t CASCADE",
            crate::ParseConfig::new(NO_DROP_BEHAVIOR_DIALECT),
        )
        .expect_err("CASCADE is rejected when drop behaviour is disabled");
        parse_with(
            "ALTER TABLE t DROP COLUMN c RESTRICT",
            crate::ParseConfig::new(NO_DROP_BEHAVIOR_DIALECT),
        )
        .expect_err("RESTRICT is rejected when drop behaviour is disabled");
        // Without the trailing behaviour the same statement parses.
        parse_with(
            "DROP TABLE t",
            crate::ParseConfig::new(NO_DROP_BEHAVIOR_DIALECT),
        )
        .expect("a behaviour-free DROP parses with the flag off");
    }

    #[test]
    fn malformed_alter_and_drop_statements_are_rejected() {
        for sql in [
            "ALTER TABLE t",                    // no action
            "ALTER TABLE t ADD",                // ADD with nothing to add
            "ALTER TABLE t ALTER COLUMN a",     // ALTER COLUMN with no alteration
            "ALTER TABLE t ALTER COLUMN a SET", // SET with no target
            "DROP t",                           // missing object kind
            "DROP TABLE",                       // missing name
            "DROP TABLE a,",                    // dangling comma
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "{sql:?} should be rejected",
            );
        }
    }

    #[test]
    fn create_schema_parses_name_guard_and_authorization() {
        let parsed = parse_with(
            "CREATE SCHEMA IF NOT EXISTS s AUTHORIZATION joe",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("CREATE SCHEMA parses");
        let schema = create_schema_of(&parsed);
        assert!(schema.if_not_exists);
        assert_eq!(
            parsed
                .resolver()
                .resolve(schema.name.as_ref().expect("schema name").0[0].sym),
            "s",
        );
        assert_eq!(
            parsed
                .resolver()
                .resolve(schema.authorization.as_ref().expect("authorization").sym),
            "joe",
        );

        // `AUTHORIZATION <role>` alone, with the schema name derived from the role.
        let parsed = parse_with(
            "CREATE SCHEMA AUTHORIZATION joe",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("CREATE SCHEMA AUTHORIZATION parses");
        let schema = create_schema_of(&parsed);
        assert!(schema.name.is_none());
        assert!(schema.authorization.is_some());
    }

    #[test]
    fn create_or_replace_view_parses_columns_and_check_option() {
        let parsed = parse_with(
            "CREATE OR REPLACE VIEW v (a, b) AS SELECT 1, 2 WITH CASCADED CHECK OPTION",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("CREATE VIEW parses");
        let view = create_view_of(&parsed);
        assert!(view.or_replace);
        assert!(!view.materialized);
        assert_eq!(view.columns.len(), 2);
        assert_eq!(parsed.resolver().resolve(view.columns[0].sym), "a");
        assert_eq!(view.check_option, Some(ViewCheckOption::Cascaded));
        assert!(view.with_data.is_none());
        assert!(matches!(&view.query.body, SetExpr::Select { .. }));
    }

    fn alter_view_of(parsed: &Parsed) -> &AlterView<NoExt> {
        let Statement::AlterView { alter, .. } = &parsed.statements()[0] else {
            panic!("expected an ALTER VIEW statement");
        };
        alter
    }

    fn assert_mysql_view_round_trips(sql: &str) {
        let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
            .unwrap_or_else(|err| panic!("PARSE {sql:?}: {err:?}"));
        let rendered = Renderer::new(MYSQL_RENDER)
            .render_parsed(&parsed)
            .unwrap_or_else(|err| panic!("RENDER {sql:?}: {err:?}"));
        assert_eq!(rendered, sql, "round-trip {sql:?}");
    }

    /// The MySQL `CREATE [OR REPLACE] [ALGORITHM = …] [DEFINER = …] [SQL SECURITY …] VIEW`
    /// definition-option prefix — each option alone, combined, and with `OR REPLACE`, all
    /// oracle-accepted (mysql:8.4.10) and round-tripping. The prefix order is fixed
    /// (algorithm, definer, sql security); the shared [`ViewOptions`] axis backs both this and
    /// `ALTER VIEW`.
    #[test]
    fn create_view_parses_mysql_option_prefix_and_round_trips() {
        for sql in [
            "CREATE ALGORITHM = UNDEFINED VIEW v AS SELECT 1",
            "CREATE ALGORITHM = MERGE VIEW v AS SELECT 1",
            "CREATE ALGORITHM = TEMPTABLE VIEW v AS SELECT 1",
            "CREATE DEFINER = root VIEW v AS SELECT 1",
            "CREATE DEFINER = 'root'@'localhost' VIEW v AS SELECT 1",
            "CREATE DEFINER = CURRENT_USER VIEW v AS SELECT 1",
            "CREATE DEFINER = CURRENT_USER() VIEW v AS SELECT 1",
            "CREATE SQL SECURITY DEFINER VIEW v AS SELECT 1",
            "CREATE SQL SECURITY INVOKER VIEW v AS SELECT 1",
            "CREATE ALGORITHM = MERGE DEFINER = root SQL SECURITY INVOKER VIEW v AS SELECT 1",
            "CREATE OR REPLACE ALGORITHM = MERGE DEFINER = root SQL SECURITY INVOKER VIEW v \
             AS SELECT 1",
        ] {
            assert_mysql_view_round_trips(sql);
        }

        let parsed = parse_with(
            "CREATE ALGORITHM = MERGE DEFINER = root SQL SECURITY INVOKER VIEW v AS SELECT 1",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL view option prefix parses");
        let view = create_view_of(&parsed);
        assert_eq!(view.options.algorithm, Some(ViewAlgorithm::Merge));
        assert!(matches!(
            view.options.definer.as_deref(),
            Some(Definer::Account { host: None, .. })
        ));
        assert_eq!(view.options.sql_security, Some(SqlSecurityContext::Invoker));
    }

    /// The MySQL view option prefix is fixed-order and view-exclusive — oracle-measured reject
    /// boundaries (mysql:8.4.10, all `ER_PARSE_ERROR`).
    #[test]
    fn create_view_option_prefix_reject_boundaries() {
        for sql in [
            // A bad ALGORITHM value.
            "CREATE ALGORITHM = BOGUS VIEW v AS SELECT 1",
            // Out-of-order prefixes.
            "CREATE DEFINER = root ALGORITHM = MERGE VIEW v AS SELECT 1",
            "CREATE SQL SECURITY INVOKER ALGORITHM = MERGE VIEW v AS SELECT 1",
            "CREATE ALGORITHM = MERGE OR REPLACE VIEW v AS SELECT 1",
            // SQL SECURITY without a value.
            "CREATE SQL SECURITY VIEW v AS SELECT 1",
            // The options after the VIEW keyword.
            "CREATE VIEW ALGORITHM = MERGE v AS SELECT 1",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err(&format!("MySQL rejects {sql:?}"));
        }
    }

    /// The option prefix rides `view_definition_options`; a dialect without it (PostgreSQL)
    /// leaves the option keyword unconsumed before the expected `VIEW` and rejects.
    #[test]
    fn create_view_option_prefix_rides_view_definition_options_gate() {
        use crate::dialect::Postgres;
        parse_with(
            "CREATE ALGORITHM = MERGE VIEW v AS SELECT 1",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL has no view definition-option prefix");
        parse_with(
            "CREATE SQL SECURITY INVOKER VIEW v AS SELECT 1",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL has no SQL SECURITY view prefix");
    }

    /// The MySQL `ALTER [ALGORITHM = …] [DEFINER = …] [SQL SECURITY …] VIEW <name> [(cols)]
    /// AS <query> [WITH … CHECK OPTION]` redefinition — every option alone, combined, the
    /// column list, and the `CHECK OPTION` variants, all oracle grammar-positive
    /// (mysql:8.4.10, `ER_UNSUPPORTED_PS` on the missing view — parse succeeds, PREPARE
    /// declines) and round-tripping.
    #[test]
    fn alter_view_parses_all_forms_and_round_trips() {
        for sql in [
            "ALTER VIEW v AS SELECT 1",
            "ALTER VIEW v (a) AS SELECT 1",
            "ALTER ALGORITHM = UNDEFINED VIEW v AS SELECT 1",
            "ALTER ALGORITHM = MERGE VIEW v AS SELECT 1",
            "ALTER ALGORITHM = TEMPTABLE VIEW v AS SELECT 1",
            "ALTER DEFINER = root VIEW v AS SELECT 1",
            "ALTER DEFINER = 'root'@'localhost' VIEW v AS SELECT 1",
            "ALTER DEFINER = CURRENT_USER VIEW v AS SELECT 1",
            "ALTER DEFINER = CURRENT_USER() VIEW v AS SELECT 1",
            "ALTER SQL SECURITY DEFINER VIEW v AS SELECT 1",
            "ALTER SQL SECURITY INVOKER VIEW v AS SELECT 1",
            "ALTER ALGORITHM = MERGE DEFINER = root SQL SECURITY INVOKER VIEW v AS SELECT 1",
            "ALTER DEFINER = root SQL SECURITY INVOKER VIEW v AS SELECT 1",
            "ALTER VIEW v AS SELECT 1 WITH CHECK OPTION",
            "ALTER VIEW v AS SELECT 1 WITH CASCADED CHECK OPTION",
            "ALTER VIEW v AS SELECT 1 WITH LOCAL CHECK OPTION",
        ] {
            assert_mysql_view_round_trips(sql);
        }

        let parsed = parse_with(
            "ALTER ALGORITHM = MERGE DEFINER = root SQL SECURITY INVOKER VIEW v (a) \
             AS SELECT 1 WITH LOCAL CHECK OPTION",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL ALTER VIEW parses");
        let alter = alter_view_of(&parsed);
        assert_eq!(alter.options.algorithm, Some(ViewAlgorithm::Merge));
        assert!(matches!(
            alter.options.definer.as_deref(),
            Some(Definer::Account { host: None, .. })
        ));
        assert_eq!(
            alter.options.sql_security,
            Some(SqlSecurityContext::Invoker)
        );
        assert_eq!(alter.columns.len(), 1);
        assert_eq!(alter.check_option, Some(ViewCheckOption::Local));
    }

    /// MySQL `ALTER VIEW` reject boundaries — oracle-measured (mysql:8.4.10, all
    /// `ER_PARSE_ERROR`): no `OR REPLACE`, no `IF EXISTS`, fixed prefix order, no options
    /// after the `VIEW` keyword, and `ALGORITHM` requires its `= <value>`.
    #[test]
    fn alter_view_reject_boundaries() {
        for sql in [
            "ALTER OR REPLACE VIEW v AS SELECT 1",
            "ALTER VIEW IF EXISTS v AS SELECT 1",
            "ALTER DEFINER = root ALGORITHM = MERGE VIEW v AS SELECT 1",
            "ALTER SQL SECURITY INVOKER ALGORITHM = MERGE VIEW v AS SELECT 1",
            "ALTER SQL SECURITY INVOKER DEFINER = root VIEW v AS SELECT 1",
            "ALTER VIEW ALGORITHM = MERGE v AS SELECT 1",
            "ALTER ALGORITHM = BOGUS VIEW v AS SELECT 1",
            "ALTER ALGORITHM VIEW v AS SELECT 1",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err(&format!("MySQL rejects {sql:?}"));
        }
    }

    /// `ALTER VIEW` rides `view_definition_options`; a dialect without it (PostgreSQL) has no
    /// `ALTER VIEW` and rejects at the `ALTER TABLE` expectation.
    #[test]
    fn alter_view_rides_view_definition_options_gate() {
        use crate::dialect::Postgres;
        parse_with(
            "ALTER VIEW v AS SELECT 1",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL has no ALTER VIEW redefinition");
    }

    /// Under Lenient (the permissive superset) the MySQL `ALTER VIEW … AS` redefinition and the
    /// DuckDB `ALTER VIEW … SET SCHEMA` relocation both ride the `VIEW` head; the shared
    /// dispatcher disambiguates on the tail after the name (and on `IF EXISTS`, which only the
    /// relocation admits), so neither surface shadows the other.
    #[test]
    fn alter_view_lenient_disambiguates_redefinition_from_set_schema() {
        use crate::dialect::Lenient;
        let redefine = parse_with("ALTER VIEW v AS SELECT 1", crate::ParseConfig::new(Lenient))
            .expect("Lenient parses redefinition");
        assert!(matches!(
            &redefine.statements()[0],
            Statement::AlterView { .. }
        ));
        let relocate = parse_with(
            "ALTER VIEW v SET SCHEMA s",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses the DuckDB relocation");
        assert!(matches!(
            &relocate.statements()[0],
            Statement::AlterObjectSchema { .. }
        ));
        let relocate_guarded = parse_with(
            "ALTER VIEW IF EXISTS v SET SCHEMA s",
            crate::ParseConfig::new(Lenient),
        )
        .expect("Lenient parses the guarded relocation");
        assert!(matches!(
            &relocate_guarded.statements()[0],
            Statement::AlterObjectSchema { .. }
        ));
    }

    #[test]
    fn create_materialized_view_parses_guard_and_with_data() {
        let parsed = parse_with(
            "CREATE MATERIALIZED VIEW IF NOT EXISTS m AS SELECT 1 WITH NO DATA",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("CREATE MATERIALIZED VIEW parses");
        let view = create_view_of(&parsed);
        assert!(view.materialized);
        assert!(view.if_not_exists);
        assert_eq!(view.with_data, Some(false));
        assert!(view.check_option.is_none());
    }

    #[test]
    fn create_recursive_view_parses_all_forms_and_round_trips() {
        // Engine-probed DuckDB accepts (duckdb 1.5.4): `RECURSIVE` sits between the
        // `TEMP`/`TEMPORARY` prefix and `VIEW` and composes with `OR REPLACE` and
        // `TEMPORARY`. The required column list renders verbatim in canonical spacing.
        for sql in [
            "CREATE RECURSIVE VIEW v (x) AS SELECT 1",
            "CREATE OR REPLACE RECURSIVE VIEW v (x) AS SELECT 1",
            "CREATE TEMPORARY RECURSIVE VIEW v (x) AS SELECT 1",
            "CREATE OR REPLACE TEMPORARY RECURSIVE VIEW v (x, y) AS SELECT 1, 2",
        ] {
            assert_duckdb_round_trips(sql);
        }

        let parsed = parse_with(
            "CREATE RECURSIVE VIEW v (x, y) AS SELECT 1, 2",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("recursive view parses");
        let view = create_view_of(&parsed);
        assert!(view.recursive);
        assert!(!view.materialized);
        assert_eq!(view.columns.len(), 2);
    }

    #[test]
    fn create_recursive_view_requires_column_list() {
        // The engine desugars a recursive view to `WITH RECURSIVE`, which names its
        // output columns, so the bare form (no column list) is rejected — mirroring the
        // engine's "syntax error at or near AS".
        parse_with(
            "CREATE RECURSIVE VIEW v AS SELECT 1",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect_err("a recursive view requires an explicit column list");
    }

    #[test]
    fn create_recursive_view_rides_recursive_views_gate() {
        use crate::dialect::Postgres;
        // `RECURSIVE VIEW` is gated to DuckDB/Lenient; PostgreSQL leaves `RECURSIVE`
        // unconsumed before the expected `VIEW` and rejects it.
        parse_with(
            "CREATE RECURSIVE VIEW v (x) AS SELECT 1",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL does not dispatch CREATE RECURSIVE VIEW");
    }

    #[test]
    fn create_recursive_view_never_composes_with_materialized() {
        // `MATERIALIZED` and `RECURSIVE` are mutually exclusive (engine-rejected both
        // orders): `MATERIALIZED` dispatches before the `TEMP`/`RECURSIVE` prefix, so
        // `RECURSIVE` never follows it and a stray `MATERIALIZED` after `RECURSIVE` is a
        // syntax error at the `VIEW` expectation.
        parse_with(
            "CREATE MATERIALIZED RECURSIVE VIEW v (x) AS SELECT 1",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect_err("MATERIALIZED RECURSIVE is rejected");
        parse_with(
            "CREATE RECURSIVE MATERIALIZED VIEW v (x) AS SELECT 1",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect_err("RECURSIVE MATERIALIZED is rejected");
    }

    #[test]
    fn create_index_parses_postgres_options() {
        let parsed = parse_with(
            "CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS i ON s.t \
             USING btree (a, lower(b) DESC NULLS LAST) WHERE a IS NOT NULL",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("CREATE INDEX with PostgreSQL options parses");
        let index = create_index_of(&parsed);
        assert!(index.unique);
        assert!(index.concurrently);
        assert!(index.if_not_exists);
        assert_eq!(
            parsed
                .resolver()
                .resolve(index.name.as_ref().expect("index name").sym),
            "i",
        );
        assert_eq!(parsed.resolver().resolve(index.table.0[0].sym), "s");
        assert_eq!(
            parsed
                .resolver()
                .resolve(index.using.as_ref().expect("using method").sym),
            "btree",
        );
        assert_eq!(index.columns.len(), 2);
        assert!(matches!(&index.columns[0].expr, Expr::Column { .. }));
        assert!(index.columns[0].asc.is_none());
        assert!(matches!(&index.columns[1].expr, Expr::Function { .. }));
        assert_eq!(index.columns[1].asc, Some(false));
        assert_eq!(index.columns[1].nulls_first, Some(false));
        assert!(index.predicate.is_some());
    }

    #[test]
    fn create_index_without_a_name_parses() {
        let parsed = parse_with(
            "CREATE INDEX ON t (a)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("an unnamed CREATE INDEX parses");
        let index = create_index_of(&parsed);
        assert!(index.name.is_none());
        assert!(!index.unique);
        assert!(!index.concurrently);
        assert_eq!(index.columns.len(), 1);
    }

    #[test]
    fn ansi_rejects_postgres_index_extensions() {
        // CONCURRENTLY / USING / the partial WHERE are gated by dialect data; under
        // ANSI the keyword is left unconsumed and surfaces as a parse error.
        parse_with(
            "CREATE INDEX CONCURRENTLY i ON t (a)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("ANSI has no CONCURRENTLY index build");
        parse_with(
            "CREATE INDEX i ON t USING btree (a)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("ANSI has no USING access-method clause");
        parse_with(
            "CREATE INDEX i ON t (a) WHERE a IS NOT NULL",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("ANSI has no partial-index WHERE");
        // The base form (and `UNIQUE`) still parse under ANSI.
        parse_with(
            "CREATE UNIQUE INDEX i ON t (a, b)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("a base UNIQUE index parses under ANSI");
    }

    #[test]
    fn malformed_create_schema_view_index_are_rejected() {
        for sql in [
            "CREATE SCHEMA",                     // no name or authorization
            "CREATE VIEW v",                     // missing AS query
            "CREATE VIEW v AS",                  // AS with no query
            "CREATE MATERIALIZED v AS SELECT 1", // MATERIALIZED without VIEW
            "CREATE INDEX i ON t",               // missing column list
            "CREATE INDEX i ON t ()",            // empty column list
            "CREATE INDEX i t (a)",              // missing ON
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(TestDialect)).is_err(),
                "{sql:?} should be rejected",
            );
        }
    }

    #[test]
    fn truncate_parses_clauses_and_is_ungated() {
        // No FeatureSet gate: TRUNCATE is standard SQL accepted by every dialect.
        assert!(parse_with("TRUNCATE t", crate::ParseConfig::new(Ansi)).is_ok());
        assert!(parse_with("TRUNCATE TABLE t", crate::ParseConfig::new(MySql)).is_ok());

        let parsed = parse_with(
            "TRUNCATE TABLE a, b RESTART IDENTITY CASCADE",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .expect("truncate parses");
        let Statement::Truncate {
            tables,
            restart_identity,
            behavior,
            ..
        } = &parsed.statements()[0]
        else {
            panic!("expected a truncate statement");
        };
        assert_eq!(tables.len(), 2);
        assert_eq!(*restart_identity, Some(true));
        assert_eq!(*behavior, Some(DropBehavior::Cascade));

        // The absent and `CONTINUE IDENTITY` forms stay distinct so they round-trip.
        let continue_id = parse_with(
            "TRUNCATE TABLE t CONTINUE IDENTITY",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .unwrap();
        let Statement::Truncate {
            restart_identity, ..
        } = &continue_id.statements()[0]
        else {
            panic!("expected a truncate statement");
        };
        assert_eq!(*restart_identity, Some(false));

        let bare = parse_with("TRUNCATE TABLE t", crate::ParseConfig::new(PG_DIALECT)).unwrap();
        let Statement::Truncate {
            restart_identity,
            behavior,
            ..
        } = &bare.statements()[0]
        else {
            panic!("expected a truncate statement");
        };
        assert_eq!(*restart_identity, None);
        assert_eq!(*behavior, None);
    }

    #[test]
    fn comment_on_parses_targets_null_and_procedure_signature() {
        let table = parse_with(
            "COMMENT ON TABLE t IS 'a note'",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .unwrap();
        let Statement::CommentOn { comment, .. } = &table.statements()[0] else {
            panic!("expected a comment-on statement");
        };
        assert_eq!(comment.target, CommentTarget::Table);
        assert!(comment.comment.is_some());

        // `IS NULL` clears the comment, so it maps to `None`.
        let null = parse_with(
            "COMMENT ON TABLE t IS NULL",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .unwrap();
        let Statement::CommentOn { comment, .. } = &null.statements()[0] else {
            panic!("expected a comment-on statement");
        };
        assert!(comment.comment.is_none());

        // A procedure carries its argument-type signature alongside the name.
        let proc = parse_with(
            "COMMENT ON PROCEDURE p(integer, text) IS 'r'",
            crate::ParseConfig::new(PG_DIALECT),
        )
        .unwrap();
        let Statement::CommentOn { comment, .. } = &proc.statements()[0] else {
            panic!("expected a comment-on statement");
        };
        let CommentTarget::Procedure { arg_types } = &comment.target else {
            panic!("expected a procedure target");
        };
        assert_eq!(arg_types.as_ref().map(|types| types.len()), Some(2));

        // PostgreSQL's `comment_text` object kinds also cover SCHEMA and
        // SEQUENCE; both parse to their own target so a consumer can act on
        // (or reject) them typed rather than seeing a parse error.
        for (sql, expected) in [
            ("COMMENT ON SCHEMA s IS 'x'", CommentTarget::Schema),
            ("COMMENT ON SEQUENCE seq IS 'x'", CommentTarget::Sequence),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(PG_DIALECT)).unwrap();
            let Statement::CommentOn { comment, .. } = &parsed.statements()[0] else {
                panic!("expected a comment-on statement for {sql:?}");
            };
            assert_eq!(comment.target, expected, "{sql:?}");
        }
    }

    #[test]
    fn comment_on_rejects_non_sconst_values_and_is_gated() {
        // `N'...'` is a national-string prefix the PostgreSQL scanner treats as NCHAR +
        // string, not a bare `Sconst`, so the comment value rejects it to keep parity —
        // even though the tokenizer folds it into one String token under this preset.
        assert!(
            parse_with(
                "COMMENT ON TABLE t IS N'x'",
                crate::ParseConfig::new(PG_DIALECT)
            )
            .is_err()
        );
        // ANSI and MySQL gate the whole statement off (`comment_on` false).
        assert!(parse_with("COMMENT ON TABLE t IS 'x'", crate::ParseConfig::new(Ansi)).is_err());
        assert!(parse_with("COMMENT ON TABLE t IS 'x'", crate::ParseConfig::new(MySql)).is_err());
    }

    fn create_type_of(parsed: &Parsed) -> &CreateType<NoExt> {
        let [Statement::CreateType { create, .. }] = parsed.statements() else {
            panic!(
                "expected one CREATE TYPE statement, got {:?}",
                parsed.statements(),
            );
        };
        create
    }

    #[test]
    fn create_type_parses_all_forms_and_round_trips() {
        // Each is an engine-probed DuckDB accept: the ENUM label list (and its empty and
        // schema-qualified forms), the composite constructors, an alias to a scalar / array
        // / parameterized type, `OR REPLACE`, `IF NOT EXISTS`, `TEMP`, and the ENUM label
        // query. Canonical spacing, so the render is verbatim.
        for sql in [
            "CREATE TYPE mood AS ENUM('sad', 'ok', 'happy')",
            "CREATE TYPE bla AS ENUM()",
            "CREATE TYPE s1.mood AS ENUM('a', 'b')",
            "CREATE TYPE p AS STRUCT(i INTEGER, j INTEGER)",
            "CREATE TYPE m AS MAP(INTEGER, INTEGER)",
            "CREATE TYPE u AS UNION(a INTEGER, b INTEGER)",
            "CREATE TYPE alias AS INTEGER",
            "CREATE TYPE my_int_list AS my_int[]",
            "CREATE TYPE d AS NUMERIC(10, 2)",
            "CREATE OR REPLACE TYPE mood AS ENUM('a')",
            "CREATE TYPE IF NOT EXISTS mood AS ENUM('a')",
            "CREATE TEMP TYPE m AS ENUM('a')",
            "CREATE TYPE e AS ENUM (SELECT l FROM t)",
        ] {
            assert_duckdb_round_trips(sql);
        }
    }

    #[test]
    fn create_type_captures_shape() {
        // An ENUM with `OR REPLACE` and a schema-qualified name; the labels are string
        // literals, not a data type.
        let parsed = parse_with(
            "CREATE OR REPLACE TYPE s.mood AS ENUM('sad', 'ok')",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        let create = create_type_of(&parsed);
        assert!(create.or_replace);
        assert!(!create.if_not_exists);
        assert!(create.temporary.is_none());
        assert_eq!(create.name.0.len(), 2, "schema-qualified name");
        let CreateTypeDefinition::Enum { labels, .. } = &create.definition else {
            panic!("expected an ENUM definition, got {:?}", create.definition);
        };
        assert_eq!(labels.len(), 2);
        assert!(labels.iter().all(|l| l.kind == LiteralKind::String));

        // The empty label list is a distinct, valid shape (`ENUM ()`).
        let empty = parse_with(
            "CREATE TYPE e AS ENUM()",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        let CreateTypeDefinition::Enum { labels, .. } = &create_type_of(&empty).definition else {
            panic!("expected an ENUM definition");
        };
        assert!(labels.is_empty());

        // The label-query form is its own variant.
        let from_query = parse_with(
            "CREATE TYPE e AS ENUM (SELECT l FROM t)",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        assert!(matches!(
            create_type_of(&from_query).definition,
            CreateTypeDefinition::EnumFromQuery { .. }
        ));

        // A non-ENUM spelling aliases a data type; `IF NOT EXISTS` sets the guard.
        let alias = parse_with(
            "CREATE TYPE IF NOT EXISTS p AS STRUCT(i INTEGER)",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        let create = create_type_of(&alias);
        assert!(create.if_not_exists);
        assert!(!create.or_replace);
        assert!(matches!(
            create.definition,
            CreateTypeDefinition::Alias {
                data_type: DataType::Struct { .. },
                ..
            }
        ));
    }

    #[test]
    fn create_type_reject_boundary() {
        // Every case is an engine-probed DuckDB parse reject (not a bind reject): ENUM labels
        // are string constants only (not numbers / NULL), the list takes no trailing comma
        // and needs separators, `OR REPLACE` and `IF NOT EXISTS` are mutually exclusive (the
        // parser reads `IF` as the name under `OR REPLACE`, so `NOT` is unexpected), there is
        // no `CASCADE` tail on `CREATE TYPE`, and an empty `STRUCT()` is rejected.
        for sql in [
            "CREATE TYPE bla AS ENUM (1, 2, 3)",
            "CREATE TYPE bla AS ENUM ('sad', NULL)",
            "CREATE TYPE e AS ENUM ('a', 'b',)",
            "CREATE TYPE e AS ENUM ('a' 'b')",
            "CREATE OR REPLACE TYPE IF NOT EXISTS m AS ENUM ('a')",
            "CREATE TYPE m AS ENUM ('a') CASCADE",
            "CREATE TYPE s AS STRUCT()",
        ] {
            parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
                .expect_err(&format!("DuckDB rejects {sql:?}"));
        }
    }

    #[test]
    fn drop_type_parses_and_round_trips() {
        // `DROP TYPE` rides the shared drop grammar: `IF EXISTS`, a schema-qualified name, a
        // comma list (DuckDB parse-accepts it, rejecting the multi-drop only at plan time),
        // and `CASCADE`/`RESTRICT`.
        for sql in [
            "DROP TYPE mood",
            "DROP TYPE IF EXISTS mood",
            "DROP TYPE mood CASCADE",
            "DROP TYPE mood RESTRICT",
            "DROP TYPE s1.mood",
            "DROP TYPE a, b, c",
        ] {
            assert_duckdb_round_trips(sql);
        }
        let parsed = parse_with(
            "DROP TYPE IF EXISTS a, b CASCADE",
            crate::ParseConfig::new(DUCKDB_RENDER),
        )
        .expect("parses");
        let [Statement::Drop { drop, .. }] = parsed.statements() else {
            panic!("expected a DROP statement");
        };
        assert_eq!(drop.object_kind, DropObjectKind::Type);
        assert!(drop.if_exists);
        assert_eq!(drop.names.len(), 2);
        assert_eq!(drop.behavior, Some(DropBehavior::Cascade));
    }

    #[test]
    fn create_drop_type_gated_off_outside_duckdb() {
        // `statement_ddl_gates.create_type` gates the whole statement: off in ANSI/
        // PostgreSQL/MySQL, so `TYPE` after `CREATE` falls through to the `CREATE TABLE`
        // expectation and `TYPE` is an unexpected `DROP` object kind — both reject.
        for sql in ["CREATE TYPE mood AS ENUM ('a')", "DROP TYPE mood"] {
            parse_with(sql, crate::ParseConfig::new(Ansi))
                .expect_err("ANSI gates CREATE/DROP TYPE off");
            parse_with(sql, crate::ParseConfig::new(PG_DIALECT))
                .expect_err("PostgreSQL gates CREATE/DROP TYPE off");
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err("MySQL gates CREATE/DROP TYPE off");
            parse_with(sql, crate::ParseConfig::new(DUCKDB_RENDER))
                .expect("DuckDB dispatches CREATE/DROP TYPE");
        }

        // Prove the flag itself drives dispatch, not the concrete dialect: a POSTGRES base
        // with only `create_type` flipped on now accepts, and the ANSI base with it forced
        // off still rejects — so a future dialect cannot bypass the shared gate.
        const PG_PLUS_TYPE: FeatureSet =
            FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.statement_ddl_gates(StatementDdlGates {
                create_type: true,
                ..StatementDdlGates::POSTGRES
            }));
        const FLAGGED_DIALECT: FeatureDialect = FeatureDialect {
            features: &PG_PLUS_TYPE,
        };
        parse_with(
            "CREATE TYPE mood AS ENUM ('a')",
            crate::ParseConfig::new(FLAGGED_DIALECT),
        )
        .expect("create_type on: PostgreSQL base now dispatches CREATE TYPE");
        parse_with("DROP TYPE mood", crate::ParseConfig::new(FLAGGED_DIALECT))
            .expect("create_type on: PostgreSQL base now dispatches DROP TYPE");
    }

    #[test]
    fn inline_enum_cast_rides_the_type_grammar_but_set_does_not() {
        // The inline `x::ENUM('a','b')` cast is a DuckDB `DataType::Enum` (the `enum_type`
        // gate), a separate production from the `CREATE TYPE ... AS ENUM` statement. `SET(...)`
        // is deliberately not recognized as a value-list type under DuckDB (`set_type` off) —
        // DuckDB has no `SET` type — so it does not ride the `Enum` grammar. It instead parses
        // as an ordinary user-defined type name carrying string modifiers (DuckDB's constant
        // type-modifier form, `string_type_modifiers`), which the engine then *binder*-rejects
        // "Type with name SET does not exist" — a binding reject outside a parse-only
        // validator's scope, exactly like `MYTYPE('x','y')`. It therefore round-trips
        // token-for-token as a plain user-defined type rather than an `ENUM` value list.
        assert_duckdb_round_trips("SELECT 'hello'::ENUM('world', 'hello')");
        assert_duckdb_round_trips("SELECT 'a'::SET('x', 'y')");
        // Under MySQL both `ENUM` and `SET` are value-list column types (SET is a column
        // type but not a `CAST` target, so it is exercised in a column definition).
        parse_with(
            "CREATE TABLE t (c SET('x', 'y'))",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL has SET");
    }

    #[test]
    fn postgres_column_default_requires_b_expr() {
        use crate::dialect::Postgres;
        // A column-constraint `DEFAULT` is PostgreSQL's restricted `b_expr`, so the whole
        // `a_expr`-only surface — `IN`/`BETWEEN`/`LIKE`, `AND`/`OR`/`NOT`, `IS NULL`, a
        // quantified `= ANY`, and `AT TIME ZONE` — is a syntax error there.
        for sql in [
            "CREATE TABLE error_tbl (b1 bool DEFAULT 1 IN (1, 2))",
            "CREATE TABLE t (a int DEFAULT 1 BETWEEN 0 AND 2)",
            "CREATE TABLE t (a bool DEFAULT true AND false)",
            "CREATE TABLE t (a bool DEFAULT 1 IS NULL)",
            "CREATE TABLE t (a bool DEFAULT NOT true)",
            "CREATE TABLE t (a bool DEFAULT 'x' LIKE 'y')",
            "CREATE TABLE t (a bool DEFAULT 1 = ANY(ARRAY[1, 2]))",
            "CREATE TABLE t (a int DEFAULT 1 + 2 IN (3, 4))",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres)).expect_err(&format!(
                "PostgreSQL restricts a column DEFAULT to b_expr: {sql:?}"
            ));
        }
        // `b_expr` members (arithmetic, comparison, `IS DISTINCT FROM`, cast) and a
        // parenthesized `c_expr` (which resets to the full `a_expr`) still parse.
        for sql in [
            "CREATE TABLE t (a int DEFAULT 1 + 2)",
            "CREATE TABLE t (a bool DEFAULT 1 < 2)",
            "CREATE TABLE t (a bool DEFAULT 1 IS DISTINCT FROM 2)",
            "CREATE TABLE t (a int DEFAULT 1::int)",
            "CREATE TABLE t (a int DEFAULT abs(1 IN (2, 3)))",
            "CREATE TABLE error_tbl (b1 bool DEFAULT (1 IN (1, 2)))",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        }
        // The asymmetric `ALTER COLUMN … SET DEFAULT` site keeps the full `a_expr`.
        parse_with(
            "ALTER TABLE t ALTER COLUMN a SET DEFAULT 1 IN (1, 2)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("ALTER … SET DEFAULT is a_expr, not b_expr");
    }

    #[test]
    fn postgres_create_schema_if_not_exists_rejects_schema_element() {
        use crate::dialect::Postgres;
        parse_with(
            "CREATE SCHEMA IF NOT EXISTS s CREATE TABLE abc (a int)",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("CREATE SCHEMA IF NOT EXISTS cannot include a schema element");
        // The plain `IF NOT EXISTS` schema (terminator follows) parses, as does the element
        // form without `IF NOT EXISTS`.
        parse_with(
            "CREATE SCHEMA IF NOT EXISTS s",
            crate::ParseConfig::new(Postgres),
        )
        .expect("CREATE SCHEMA IF NOT EXISTS with no element parses");
        parse_with(
            "CREATE SCHEMA IF NOT EXISTS s; CREATE TABLE abc (a int)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("a semicolon-separated CREATE after INE schema parses");
        parse_with(
            "CREATE SCHEMA s CREATE TABLE abc (a int)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("CREATE SCHEMA + element without IF NOT EXISTS parses");
    }

    /// The SQL-standard embedded schema-element form parses as ONE `CreateSchema`
    /// carrying the elements as children (not a `;`-joined statement rewrite), and
    /// renders them back embedded so the statement count round-trips.
    #[test]
    fn postgres_create_schema_embeds_elements_and_round_trips() {
        use crate::dialect::Postgres;

        // One element: exactly one top-level statement, one embedded child.
        let parsed = parse_with(
            "CREATE SCHEMA s CREATE TABLE t (a INTEGER)",
            crate::ParseConfig::new(Postgres),
        )
        .expect("embedded element parses");
        assert_eq!(
            parsed.statements().len(),
            1,
            "embedded form is ONE statement"
        );
        let schema = create_schema_of(&parsed);
        assert_eq!(schema.elements.len(), 1, "one embedded element");
        assert!(matches!(schema.elements[0], Statement::CreateTable { .. }));

        // Multiple elements, and canonical render preserves the embedded (semicolon-free)
        // shape verbatim.
        for sql in [
            "CREATE SCHEMA s CREATE TABLE t (a INTEGER)",
            "CREATE SCHEMA s CREATE TABLE t (a INTEGER) CREATE TABLE u (b INTEGER)",
            "CREATE SCHEMA AUTHORIZATION joe CREATE TABLE t (a INTEGER)",
            "CREATE SCHEMA s AUTHORIZATION joe CREATE VIEW v AS SELECT 1",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            assert_eq!(parsed.statements().len(), 1, "{sql:?} is ONE statement");
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }

        // The admissible set is closed: a non-element trailing statement is rejected here
        // (matching PostgreSQL's greedy `CREATE SCHEMA` production), not accepted as a
        // separate statement — so over-accept stays zero.
        for sql in [
            "CREATE SCHEMA s CREATE MATERIALIZED VIEW mv AS SELECT 1",
            "CREATE SCHEMA s CREATE FUNCTION f() RETURNS int AS 'x' LANGUAGE sql",
            "CREATE SCHEMA s CREATE SCHEMA nested",
            "CREATE SCHEMA s SELECT 1",
            "CREATE SCHEMA s DROP TABLE t",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("{sql:?} is not an admissible schema element"));
        }

        // Off-gate dialects (TestDialect here) do not embed schema elements: the schema head
        // parses as `CREATE SCHEMA s` with no elements, so a *following* `CREATE` is trailing
        // input. The top-level statement list is `;`-delimited, so without a `;` separator that
        // trailing `CREATE` is a syntax error; with an explicit `;` the same input is two
        // ordinary statements, the embedding being PostgreSQL/Lenient only.
        assert!(
            parse_with(
                "CREATE SCHEMA s CREATE TABLE t (a INTEGER)",
                crate::ParseConfig::new(TestDialect)
            )
            .is_err(),
            "off-gate dialect rejects the separator-less trailing CREATE",
        );
        let split = parse_with(
            "CREATE SCHEMA s; CREATE TABLE t (a INTEGER)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("off-gate dialect parses the `;`-separated form");
        assert_eq!(
            split.statements().len(),
            2,
            "off-gate dialect leaves the element as a separate `;`-delimited statement",
        );
    }

    /// Round-trip every declarative-partitioning surface under the PostgreSQL target: the parent
    /// `PARTITION BY` (all three strategies, opclass / collate / parenthesized-expr / funccall
    /// keys), the `PARTITION OF` child bounds (list / range with `minvalue`/`maxvalue` /
    /// hash / default), the child augmentation list, nesting, and `ATTACH`/`DETACH`.
    #[test]
    fn postgres_declarative_partitioning_round_trips() {
        // Types use the canonical `INTEGER` spelling so the exact-text assertion isolates the
        // partitioning render (a bare `INT` renders back as `INTEGER`).
        for sql in [
            // parent PARTITION BY
            "CREATE TABLE t (a INTEGER) PARTITION BY LIST (a)",
            "CREATE TABLE t (a INTEGER, b INTEGER) PARTITION BY RANGE (a, (a + b + 1))",
            "CREATE TABLE t (a INTEGER) PARTITION BY HASH (a part_test_int4_ops)",
            "CREATE TABLE t (a INTEGER) PARTITION BY RANGE (a COLLATE \"C\" text_ops)",
            "CREATE TABLE t (a INTEGER) PARTITION BY LIST (lower(a))",
            "CREATE TABLE t (a INTEGER) PARTITION BY RANGE (a myschema.text_ops)",
            "CREATE TEMP TABLE t (a INTEGER) PARTITION BY LIST (a)",
            // child PARTITION OF, each bound kind
            "CREATE TABLE c PARTITION OF p FOR VALUES IN (1, 2)",
            "CREATE TABLE c PARTITION OF p FOR VALUES FROM (0) TO (100)",
            "CREATE TABLE c PARTITION OF p FOR VALUES FROM (minvalue) TO (maxvalue)",
            "CREATE TABLE c PARTITION OF p FOR VALUES WITH (MODULUS 4, REMAINDER 0)",
            "CREATE TABLE c PARTITION OF p DEFAULT",
            // child augmentations (typeless column overrides + table constraints)
            "CREATE TABLE c PARTITION OF p (b NOT NULL, CONSTRAINT ck CHECK (b >= 0)) FOR VALUES IN ('b')",
            "CREATE TABLE c PARTITION OF p (a PRIMARY KEY) FOR VALUES FROM (0) TO (100)",
            // nesting: a sub-partitioned child
            "CREATE TABLE c PARTITION OF p FOR VALUES IN (1) PARTITION BY RANGE (a)",
            "CREATE TABLE c PARTITION OF p DEFAULT PARTITION BY LIST (b)",
            // ALTER TABLE ATTACH / DETACH
            "ALTER TABLE p ATTACH PARTITION c FOR VALUES IN (1, 2)",
            "ALTER TABLE p ATTACH PARTITION c FOR VALUES WITH (MODULUS 4, REMAINDER 1)",
            "ALTER TABLE p ATTACH PARTITION c DEFAULT",
            "ALTER TABLE p DETACH PARTITION c",
            "ALTER TABLE p DETACH PARTITION c CONCURRENTLY",
            "ALTER TABLE p DETACH PARTITION c FINALIZE",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }
    }

    /// The `WITH OPTIONS` noise phrase and a bare-parenthesized column key normalize away on
    /// render (matching PostgreSQL, which does not preserve either in its parse tree).
    #[test]
    fn postgres_partitioning_normalizes_noise() {
        use crate::dialect::Postgres;
        let parsed = parse_with(
            "CREATE TABLE c PARTITION OF p (b WITH OPTIONS NOT NULL) FOR VALUES IN ('c')",
            crate::ParseConfig::new(Postgres),
        )
        .expect("WITH OPTIONS augmentation parses");
        assert_eq!(
            Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "CREATE TABLE c PARTITION OF p (b NOT NULL) FOR VALUES IN ('c')",
        );
    }

    /// The reject boundary PostgreSQL enforces at raw-parse (the grammar action layer): an
    /// unknown strategy, a malformed hash bound, a non-column bare key, an empty / half bound.
    #[test]
    fn postgres_partitioning_reject_boundary() {
        use crate::dialect::Postgres;
        for sql in [
            // unrecognized strategy word (PG's `parsePartitionStrategy` rejects it)
            "CREATE TABLE t (a INT) PARTITION BY FOO (a)",
            // empty partition key list
            "CREATE TABLE t (a INT) PARTITION BY LIST ()",
            // bare non-column, non-funccall key
            "CREATE TABLE t (a INT) PARTITION BY RANGE (5)",
            "CREATE TABLE t (a INT) PARTITION BY RANGE (-a)",
            "CREATE TABLE t (a INT) PARTITION BY RANGE (a + b)",
            "CREATE TABLE t (a INT) PARTITION BY RANGE (a::text)",
            // hash bound: missing / duplicate / non-integer / signed component
            "CREATE TABLE c PARTITION OF p FOR VALUES WITH (MODULUS 4)",
            "CREATE TABLE c PARTITION OF p FOR VALUES WITH (REMAINDER 0)",
            "CREATE TABLE c PARTITION OF p FOR VALUES WITH (MODULUS 4, REMAINDER 0, MODULUS 2)",
            "CREATE TABLE c PARTITION OF p FOR VALUES WITH (MODULUS a, REMAINDER 0)",
            "CREATE TABLE c PARTITION OF p FOR VALUES WITH (BOGUS 4, REMAINDER 0)",
            // range bound missing its `TO`, empty bound lists
            "CREATE TABLE c PARTITION OF p FOR VALUES FROM (0)",
            "CREATE TABLE c PARTITION OF p FOR VALUES IN ()",
            // an empty `()` augmentation list
            "CREATE TABLE c PARTITION OF p () FOR VALUES IN (1)",
            // a typed column in the child augmentation list (only overrides are allowed)
            "CREATE TABLE c PARTITION OF p (a INT) FOR VALUES IN (1)",
            // ATTACH PARTITION requires a bound; it is not a comma-list action
            "ALTER TABLE p ATTACH PARTITION c",
            "ALTER TABLE p ADD COLUMN x INT, ATTACH PARTITION c DEFAULT",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("PostgreSQL must reject {sql:?}"));
        }
    }

    /// Declarative partitioning is gated to PostgreSQL/Lenient: every other preset rejects the
    /// `PARTITION` / `ATTACH` / `DETACH` keyword as leftover input. Proves the flag, not the
    /// concrete dialect, drives acceptance (a PostgreSQL base with the flag cleared also rejects).
    #[test]
    fn declarative_partitioning_is_gated() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};
        const PG_NO_PARTITIONING: FeatureSet = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                declarative_partitioning: false,
                ..CreateTableClauseSyntax::POSTGRES
            }),
        );
        const PG_NO_PARTITIONING_DIALECT: FeatureDialect = FeatureDialect {
            features: &PG_NO_PARTITIONING,
        };
        for sql in [
            "CREATE TABLE t (a INT) PARTITION BY LIST (a)",
            "CREATE TABLE c PARTITION OF p FOR VALUES IN (1)",
            "ALTER TABLE p ATTACH PARTITION c DEFAULT",
            "ALTER TABLE p DETACH PARTITION c",
        ] {
            parse_with(sql, crate::ParseConfig::new(Ansi))
                .expect_err(&format!("ANSI has no partitioning: {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err(&format!("MySQL has no PG partitioning: {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .expect_err(&format!("SQLite has no partitioning: {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(DuckDb))
                .expect_err(&format!("DuckDB has no partitioning: {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(PG_NO_PARTITIONING_DIALECT))
                .expect_err(&format!("the gate off must reject: {sql:?}"));
            // On under PostgreSQL, the same statement parses.
            parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("PostgreSQL accepts {sql:?}: {err:?}"));
        }
    }

    /// Round-trip every INHERITS / LIKE surface under the PostgreSQL target: the trailing
    /// `INHERITS (parents)` clause (single / multiple / qualified parents, empty column list, its
    /// order relative to `PARTITION BY` and options), and the `(LIKE src …)` source-table copy
    /// element (bare, each option keyword, `INCLUDING`/`EXCLUDING`, repeated + interleaved with
    /// columns, combined with `INHERITS` and `PARTITION BY`).
    #[test]
    fn postgres_inherits_and_like_round_trip() {
        // Types use the canonical `INTEGER` spelling so the exact-text assertion isolates the
        // clause render (a bare `INT` renders back as `INTEGER`).
        for sql in [
            // INHERITS
            "CREATE TABLE t (a INTEGER) INHERITS (parent)",
            "CREATE TABLE t (a INTEGER) INHERITS (p1, p2)",
            "CREATE TABLE t () INHERITS (p)",
            "CREATE TABLE t (a INTEGER) INHERITS (myschema.parent)",
            "CREATE TEMP TABLE t (a INTEGER) INHERITS (p)",
            "CREATE TABLE t (a INTEGER) INHERITS (p) ON COMMIT DROP",
            // INHERITS precedes PARTITION BY (the load-bearing order)
            "CREATE TABLE t (a INTEGER) INHERITS (p) PARTITION BY RANGE (a)",
            // LIKE source-table element
            "CREATE TABLE t (LIKE src)",
            "CREATE TABLE t (LIKE myschema.src)",
            "CREATE TABLE t (LIKE src INCLUDING ALL)",
            "CREATE TABLE t (LIKE src EXCLUDING ALL)",
            "CREATE TABLE t (LIKE src INCLUDING DEFAULTS EXCLUDING CONSTRAINTS)",
            "CREATE TABLE t (LIKE src INCLUDING ALL EXCLUDING INDEXES)",
            "CREATE TABLE t (LIKE src INCLUDING COMMENTS INCLUDING COMPRESSION INCLUDING CONSTRAINTS INCLUDING DEFAULTS INCLUDING GENERATED INCLUDING IDENTITY INCLUDING INDEXES INCLUDING STATISTICS INCLUDING STORAGE)",
            "CREATE TABLE t (a INTEGER, LIKE src, b INTEGER)",
            "CREATE TABLE t (LIKE src INCLUDING STORAGE, a INTEGER)",
            // LIKE + INHERITS + PARTITION BY all at once
            "CREATE TABLE t (LIKE src INCLUDING IDENTITY) INHERITS (p) PARTITION BY LIST (a)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }
    }

    /// The reject boundary PostgreSQL enforces at raw-parse for INHERITS and the LIKE element: an
    /// empty / unparenthesized / trailing-comma parent list, INHERITS after PARTITION BY or the
    /// options (wrong order), INHERITS on a `PARTITION OF` child, and a bare / unknown / missing
    /// LIKE option.
    #[test]
    fn postgres_inherits_and_like_reject_boundary() {
        use crate::dialect::Postgres;
        for sql in [
            // INHERITS: empty / unparenthesized / trailing-comma parent list
            "CREATE TABLE t () INHERITS ()",
            "CREATE TABLE t (a INT) INHERITS p",
            "CREATE TABLE t (a INT) INHERITS (p,)",
            // INHERITS must precede PARTITION BY and the trailing options
            "CREATE TABLE t (a INT) PARTITION BY RANGE (a) INHERITS (p)",
            "CREATE TABLE t (a INT) WITH (fillfactor=70) INHERITS (p)",
            // INHERITS is not admitted on a PARTITION OF child
            "CREATE TABLE t PARTITION OF p FOR VALUES IN (1) INHERITS (q)",
            // INHERITS is not admitted on an AS <query> body
            "CREATE TABLE t AS SELECT 1 INHERITS (p)",
            // LIKE element: bare / unknown / missing option
            "CREATE TABLE t (LIKE)",
            "CREATE TABLE t (LIKE src INCLUDING)",
            "CREATE TABLE t (LIKE src INCLUDING FOO)",
            "CREATE TABLE t (LIKE src COMMENTS)",
            // LIKE as a statement-level clause (MySQL's form) is not the PostgreSQL element
            "CREATE TABLE t LIKE src",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("PostgreSQL must reject {sql:?}"));
        }
    }

    /// INHERITS and the LIKE source-table element are gated to PostgreSQL/Lenient: every other
    /// preset rejects the `INHERITS` / element-position `LIKE` keyword as leftover input. Proves
    /// the flags, not the concrete dialect, drive acceptance (a PostgreSQL base with either flag
    /// cleared also rejects the corresponding form).
    #[test]
    fn inherits_and_like_are_gated() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};
        const PG_NO_INHERITS: FeatureSet = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                table_inheritance: false,
                ..CreateTableClauseSyntax::POSTGRES
            }),
        );
        const PG_NO_INHERITS_DIALECT: FeatureDialect = FeatureDialect {
            features: &PG_NO_INHERITS,
        };
        const PG_NO_LIKE: FeatureSet = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                like_source_table: false,
                ..CreateTableClauseSyntax::POSTGRES
            }),
        );
        const PG_NO_LIKE_DIALECT: FeatureDialect = FeatureDialect {
            features: &PG_NO_LIKE,
        };
        // (SQL, the flag-cleared PostgreSQL dialect that must still reject it). The LIKE case uses
        // the `INCLUDING ALL` form so it rejects uniformly across the non-PG presets: SQLite, whose
        // permissive identifier rules let a keyword be a column name, otherwise reads a *bare*
        // `LIKE src` as an ordinary `LIKE`-named column of type `src` (real SQLite behaviour, not a
        // LIKE element) — the trailing `INCLUDING ALL` then makes even SQLite reject.
        for (sql, flag_off) in [
            (
                "CREATE TABLE t (a INT) INHERITS (p)",
                PG_NO_INHERITS_DIALECT,
            ),
            (
                "CREATE TABLE t (LIKE src INCLUDING ALL)",
                PG_NO_LIKE_DIALECT,
            ),
        ] {
            parse_with(sql, crate::ParseConfig::new(Ansi))
                .expect_err(&format!("ANSI rejects {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err(&format!("MySQL rejects {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .expect_err(&format!("SQLite rejects {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(DuckDb))
                .expect_err(&format!("DuckDB rejects {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(flag_off))
                .expect_err(&format!("the gate off must reject {sql:?}"));
            // On under PostgreSQL, the same statement parses.
            parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("PostgreSQL accepts {sql:?}: {err:?}"));
        }
        // The bare `LIKE src` element (no options): the flag-cleared PostgreSQL dialect rejects it
        // (PostgreSQL reserves `LIKE`, so it cannot be a bare column name), proving the flag drives
        // acceptance even where SQLite's laxer identifier rules would read it as a column.
        parse_with(
            "CREATE TABLE t (LIKE src)",
            crate::ParseConfig::new(PG_NO_LIKE_DIALECT),
        )
        .expect_err("the LIKE gate off must reject a bare LIKE element on PostgreSQL");
        parse_with(
            "CREATE TABLE t (LIKE src)",
            crate::ParseConfig::new(crate::dialect::Postgres),
        )
        .expect("PostgreSQL accepts a bare LIKE element");
    }

    /// MySQL's statement-level `CREATE TABLE t LIKE src` table-clone body and its parenthesized
    /// twin `CREATE TABLE t (LIKE src)` — a whole-statement production distinct from PostgreSQL's
    /// copy element. Engine-verified accepts on mysql:8.4: both spellings, `IF NOT EXISTS`,
    /// `TEMPORARY`, and a qualified source. The `parenthesized` flag preserves the spelling, so
    /// each round-trips to the exact source text.
    #[test]
    fn mysql_statement_level_table_like_round_trip() {
        for (sql, parenthesized) in [
            ("CREATE TABLE t LIKE src", false),
            ("CREATE TABLE t (LIKE src)", true),
            ("CREATE TABLE IF NOT EXISTS t LIKE src", false),
            ("CREATE TABLE IF NOT EXISTS t (LIKE src)", true),
            ("CREATE TEMPORARY TABLE t LIKE src", false),
            ("CREATE TEMPORARY TABLE t (LIKE src)", true),
            ("CREATE TABLE t LIKE myschema.src", false),
            ("CREATE TABLE myschema.t (LIKE src)", true),
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            match &create_table_of(&parsed).body {
                CreateTableBody::LikeSource {
                    parenthesized: p, ..
                } => assert_eq!(*p, parenthesized, "spelling flag for {sql:?}"),
                other => panic!("{sql:?} expected a LikeSource body, got {other:?}"),
            }
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }
    }

    /// MySQL's reject boundary for the statement-level clone (engine-verified `ER_PARSE_ERROR` on
    /// mysql:8.4): a missing source, trailing table options, a co-element beside `(LIKE …)`, a
    /// second `LIKE`, and the PostgreSQL `INCLUDING`/`EXCLUDING` feature options — MySQL carries a
    /// bare source name only, so its form never grows the element's copy options.
    #[test]
    fn mysql_statement_level_table_like_reject_boundary() {
        for sql in [
            "CREATE TABLE t LIKE",
            "CREATE TABLE t LIKE src ENGINE=InnoDB",
            "CREATE TABLE t LIKE src (x INT)",
            "CREATE TABLE t (LIKE src, x INT)",
            "CREATE TABLE t (x INT, LIKE src)",
            "CREATE TABLE t (LIKE src, LIKE other)",
            "CREATE TABLE t (LIKE src INCLUDING ALL)",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err(&format!("MySQL must reject {sql:?}"));
        }
    }

    /// The statement-level clone is gated by `statement_level_table_like`: on for MySQL/Lenient,
    /// off for ANSI/PostgreSQL/SQLite/DuckDB. The bare `LIKE src` form rejects everywhere it is
    /// off (PostgreSQL reserves `LIKE`, so it is never a bare column name), proving the flag — not
    /// the dialect — drives acceptance (a MySQL base with the flag cleared also rejects).
    #[test]
    fn mysql_statement_level_table_like_is_gated() {
        use crate::dialect::{Ansi, DuckDb, Postgres, Sqlite};
        const MYSQL_NO_STMT_LIKE: FeatureSet = FeatureSet::MYSQL.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                statement_level_table_like: false,
                ..CreateTableClauseSyntax::MYSQL
            }),
        );
        const MYSQL_NO_STMT_LIKE_DIALECT: FeatureDialect = FeatureDialect {
            features: &MYSQL_NO_STMT_LIKE,
        };
        let sql = "CREATE TABLE t LIKE src";
        parse_with(sql, crate::ParseConfig::new(Ansi))
            .expect_err("ANSI rejects the statement-level clone");
        parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect_err("PostgreSQL rejects the bare statement-level clone");
        parse_with(sql, crate::ParseConfig::new(Sqlite))
            .expect_err("SQLite rejects the statement-level clone");
        parse_with(sql, crate::ParseConfig::new(DuckDb))
            .expect_err("DuckDB rejects the statement-level clone");
        parse_with(sql, crate::ParseConfig::new(MYSQL_NO_STMT_LIKE_DIALECT))
            .expect_err("the gate off must reject the statement-level clone");
        parse_with(sql, crate::ParseConfig::new(MySql))
            .expect("MySQL accepts the statement-level clone");
        // Lenient reads the parenthesized `(LIKE src)` as the PostgreSQL element (its superset),
        // but the bare form is this MySQL body — both accept there.
        parse_with(sql, crate::ParseConfig::new(crate::dialect::Lenient))
            .expect("Lenient accepts the bare clone");
    }

    /// Round-trip the column-definition `COLLATE <collation>` surface under the PostgreSQL
    /// target: quoted / bare / qualified collation names, its free interleaving with the other
    /// column constraints (PostgreSQL's `ColQualList` admits `COLLATE` at any position, engine-
    /// verified), a user-typed (domain) column, a trailing constraint-attribute `DEFERRABLE`
    /// (accepted standalone at raw parse), and the shared `parse_column_def` path in
    /// `ALTER TABLE ADD COLUMN`.
    ///
    /// A repeated `COLLATE "C" COLLATE "POSIX"` is a *recorded acceptance bound*: PostgreSQL
    /// rejects it in a grammar action ("multiple COLLATE clauses not allowed") while the
    /// constraint loop here accepts it — SQLite genuinely accepts repeats, the pg-regress corpus
    /// never exercises the form (over-accept stays 0 measured), and the expression-position
    /// COLLATE surface records the same repeat bound.
    #[test]
    fn postgres_column_collate_round_trip() {
        for sql in [
            "CREATE TABLE t (a TEXT COLLATE \"C\")",
            "CREATE TABLE t (a TEXT COLLATE \"en_US\")",
            "CREATE TABLE t (a TEXT COLLATE mycoll)",
            "CREATE TABLE t (a TEXT COLLATE public.mycoll)",
            "CREATE TABLE t (a TEXT COLLATE pg_catalog.\"default\")",
            "CREATE TABLE t (a TEXT COLLATE \"C\" NOT NULL)",
            "CREATE TABLE t (a TEXT NOT NULL COLLATE \"C\")",
            "CREATE TABLE t (a TEXT COLLATE \"C\" PRIMARY KEY)",
            "CREATE TABLE t (a TEXT DEFAULT 'x' COLLATE \"C\")",
            "CREATE TABLE t (a TEXT COLLATE \"C\" CHECK (a <> ''))",
            "CREATE TABLE t (a mydomain COLLATE \"C\")",
            "CREATE TABLE t (a TEXT COLLATE \"C\" DEFERRABLE)",
            "ALTER TABLE t ADD COLUMN a TEXT COLLATE \"C\"",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }
    }

    /// The reject boundary PostgreSQL enforces at raw-parse around a column `COLLATE`: a missing
    /// collation name, and a `CONSTRAINT <name>` prefix on the clause (PostgreSQL's `COLLATE
    /// any_name` is a `ColConstraint` alternative *parallel to* the nameable
    /// `ColConstraintElem`, so the named form is a syntax error there — engine-measured; SQLite,
    /// whose grammar makes `COLLATE` an ordinary nameable constraint, accepts it below).
    #[test]
    fn postgres_column_collate_reject_boundary() {
        use crate::dialect::Postgres;
        for sql in [
            "CREATE TABLE t (a TEXT COLLATE)",
            "CREATE TABLE t (a TEXT CONSTRAINT c COLLATE \"C\")",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("PostgreSQL must reject {sql:?}"));
        }
    }

    /// The column-definition `COLLATE` is one cross-dialect shape gated by `column_collation`:
    /// PostgreSQL, SQLite, and DuckDB all accept the bare-identifier form (each engine-verified),
    /// ANSI/MySQL reject it, and a PostgreSQL base with the flag cleared rejects it too — the
    /// flag, not the concrete dialect, drives acceptance. The SQLite-only `CONSTRAINT <name>`
    /// prefix on the clause rides its own `named_column_collate_constraint` flag and stays
    /// rejected under PostgreSQL/DuckDB (both engine-verified rejects); a repeated bare COLLATE
    /// rides `column_collation`.
    #[test]
    fn column_collate_is_gated() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};
        const PG_NO_COLLATION: FeatureSet = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                column_collation: false,
                ..ColumnDefinitionSyntax::POSTGRES
            }),
        );
        const PG_NO_COLLATION_DIALECT: FeatureDialect = FeatureDialect {
            features: &PG_NO_COLLATION,
        };
        let sql = "CREATE TABLE t (a TEXT COLLATE nocase)";
        for dialect_accepts in [
            parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres)).is_ok(),
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_ok(),
            parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
        ] {
            assert!(dialect_accepts, "PG/SQLite/DuckDB accept {sql:?}");
        }
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI has no column COLLATE");
        parse_with(sql, crate::ParseConfig::new(MySql))
            .expect_err("MySQL's column COLLATE is a distinct attribute grammar");
        parse_with(sql, crate::ParseConfig::new(PG_NO_COLLATION_DIALECT))
            .expect_err("the gate off must reject");
        // The named form is SQLite-only (`named_column_collate_constraint`): SQLite accepts, DuckDB
        // rejects (both engine-verified on the bundled/live engines).
        let named = "CREATE TABLE t (a TEXT CONSTRAINT c COLLATE nocase)";
        parse_with(named, crate::ParseConfig::new(Sqlite))
            .expect("SQLite names a COLLATE constraint");
        parse_with(named, crate::ParseConfig::new(DuckDb))
            .expect_err("DuckDB rejects a named COLLATE");
        // SQLite accepts a repeated column COLLATE (engine-verified); the constraint loop
        // preserves both.
        parse_with(
            "CREATE TABLE t (a TEXT COLLATE nocase COLLATE binary)",
            crate::ParseConfig::new(Sqlite),
        )
        .expect("SQLite accepts repeated COLLATE clauses");
    }

    /// Round-trip the persistence/storage surface under the PostgreSQL target: `CREATE UNLOGGED
    /// TABLE` (definition and CTAS bodies), the per-column `STORAGE`/`COMPRESSION` fixed-position
    /// attributes (each strategy spelling, the `DEFAULT` keyword form, a quoted value, and the
    /// open `bogus` value PostgreSQL admits at raw parse), the trailing `USING <access_method>`
    /// clause in its exact grammar slot (after INHERITS/PARTITION BY, before the options, on
    /// every non-CTAS body), and the legacy `WITHOUT OIDS` no-op.
    #[test]
    fn postgres_persistence_and_storage_round_trip() {
        for sql in [
            // UNLOGGED
            "CREATE UNLOGGED TABLE t (a INTEGER)",
            "CREATE UNLOGGED TABLE t AS SELECT 1",
            "CREATE UNLOGGED TABLE t (a INTEGER) USING heap",
            // column STORAGE (an open ColId | DEFAULT value)
            "CREATE TABLE t (a TEXT STORAGE PLAIN)",
            "CREATE TABLE t (a TEXT STORAGE EXTERNAL)",
            "CREATE TABLE t (a TEXT STORAGE EXTENDED)",
            "CREATE TABLE t (a TEXT STORAGE MAIN)",
            "CREATE TABLE t (a TEXT STORAGE DEFAULT)",
            "CREATE TABLE t (a TEXT STORAGE bogus)",
            "CREATE TABLE t (a TEXT STORAGE PLAIN NOT NULL)",
            // column COMPRESSION
            "CREATE TABLE t (a TEXT COMPRESSION pglz)",
            "CREATE TABLE t (a TEXT COMPRESSION lz4)",
            "CREATE TABLE t (a TEXT COMPRESSION DEFAULT)",
            "CREATE TABLE t (a TEXT COMPRESSION \"pglz\")",
            "CREATE TABLE t (a TEXT COMPRESSION pglz NOT NULL)",
            // STORAGE before COMPRESSION (the load-bearing order), stacked with constraints
            "CREATE TABLE t (a TEXT STORAGE MAIN COMPRESSION lz4 NOT NULL)",
            "CREATE TABLE t (a TEXT STORAGE MAIN COMPRESSION lz4 COLLATE \"C\" NOT NULL DEFAULT 'x')",
            // USING access method
            "CREATE TABLE t (a INTEGER) USING heap",
            "CREATE TABLE t (a INTEGER) USING \"heap\"",
            "CREATE TABLE t (a INTEGER) USING heap WITH (fillfactor = 70)",
            "CREATE TABLE t (a INTEGER) USING heap ON COMMIT DROP",
            "CREATE TABLE t (a INTEGER) PARTITION BY RANGE (a) USING heap",
            "CREATE TABLE t (a INTEGER) INHERITS (p) USING heap",
            "CREATE TABLE c PARTITION OF p FOR VALUES IN (1) USING heap",
            // the CTAS `USING` slot precedes `AS` (and the `WITH (…)` options)
            "CREATE TABLE t USING heap AS SELECT 1",
            "CREATE TABLE t (a, b) USING heap AS SELECT 1, 2",
            "CREATE UNLOGGED TABLE t USING heap AS SELECT 1",
            "CREATE TABLE t USING heap WITH (fillfactor = 70) AS SELECT 1",
            // WITHOUT OIDS (the legacy no-op, preserved as written)
            "CREATE TABLE t (a INTEGER) WITHOUT OIDS",
            "CREATE TABLE t (a INTEGER) WITHOUT OIDS ON COMMIT DROP",
            "CREATE TABLE t (a INTEGER) WITHOUT OIDS TABLESPACE ts",
            "CREATE TABLE t (a INTEGER) USING heap WITHOUT OIDS",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }
    }

    /// The reject boundary PostgreSQL enforces at raw-parse for the persistence/storage surface
    /// (every case engine-measured on libpg_query): `UNLOGGED` never combines with
    /// `TEMP`/`TEMPORARY` (peers in `OptTemp`, either order), the fixed positions of
    /// `STORAGE`/`COMPRESSION` (never after a constraint, `STORAGE` before `COMPRESSION`, no
    /// repeats, no reserved-word or qualified values), and the `USING` slot (a single bare
    /// method name after `PARTITION BY` and before `WITH`, never on a CTAS). PostgreSQL's
    /// grammar has no affirmative `WITH OIDS` (only the negative no-op survives), so it
    /// rejects.
    ///
    /// Reordered/repeated *trailing options* (`WITHOUT OIDS WITHOUT OIDS`, `WITH (…) WITHOUT
    /// OIDS`) are a pre-existing recorded bound: PostgreSQL's `OptWith OptOnCommit OptTableSpace`
    /// tail is one ordered single-slot sequence, while the shared option loop here is order-free
    /// and repeatable for every dialect's trailing options — the pg-regress corpus never
    /// exercises the malformed orders (over-accept stays 0 measured).
    #[test]
    fn postgres_persistence_and_storage_reject_boundary() {
        use crate::dialect::Postgres;
        for sql in [
            // UNLOGGED / TEMP are mutually exclusive, both orders
            "CREATE TEMP UNLOGGED TABLE t (a INT)",
            "CREATE UNLOGGED TEMP TABLE t (a INT)",
            "CREATE TEMPORARY UNLOGGED TABLE t (a INT)",
            // STORAGE / COMPRESSION positions and values
            "CREATE TABLE t (a TEXT NOT NULL STORAGE PLAIN)",
            "CREATE TABLE t (a TEXT NOT NULL COMPRESSION pglz)",
            "CREATE TABLE t (a TEXT COMPRESSION pglz STORAGE PLAIN)",
            "CREATE TABLE t (a TEXT STORAGE MAIN STORAGE PLAIN)",
            "CREATE TABLE t (a TEXT STORAGE)",
            "CREATE TABLE t (a TEXT COMPRESSION)",
            "CREATE TABLE t (a TEXT STORAGE from)",
            "CREATE TABLE t (a TEXT COMPRESSION pg_catalog.pglz)",
            // USING slot: after PARTITION BY / before WITH; single bare name; on a CTAS it
            // precedes AS (and the options), never trails the query
            "CREATE TABLE t (a INT) WITH (fillfactor = 70) USING heap",
            "CREATE TABLE t (a INT) USING my.heap",
            "CREATE TABLE t (a INT) USING",
            "CREATE TABLE t AS SELECT 1 USING heap",
            "CREATE TABLE t WITH (fillfactor = 70) USING heap AS SELECT 1",
            // PostgreSQL's grammar has no affirmative WITH OIDS
            "CREATE TABLE t (a INT) WITH OIDS",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("PostgreSQL must reject {sql:?}"));
        }
    }

    /// The persistence/storage flags are dialect data, not dialect identity: `UNLOGGED` is on
    /// for PostgreSQL *and* DuckDB (both engine-verified accepts, including DuckDB's
    /// `OR REPLACE UNLOGGED` combination), while the column `STORAGE`/`COMPRESSION`, the table
    /// `USING`, and `WITHOUT OIDS` are PostgreSQL-only (DuckDB engine-verified rejects all
    /// three). A PostgreSQL base with each flag cleared rejects the corresponding form.
    #[test]
    fn persistence_and_storage_are_gated() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};
        const PG_NO_UNLOGGED: FeatureSet = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                unlogged_tables: false,
                ..CreateTableClauseSyntax::POSTGRES
            }),
        );
        const PG_NO_UNLOGGED_DIALECT: FeatureDialect = FeatureDialect {
            features: &PG_NO_UNLOGGED,
        };
        const PG_NO_STORAGE: FeatureSet = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY.column_definition_syntax(ColumnDefinitionSyntax {
                column_storage: false,
                ..ColumnDefinitionSyntax::POSTGRES
            }),
        );
        const PG_NO_STORAGE_DIALECT: FeatureDialect = FeatureDialect {
            features: &PG_NO_STORAGE,
        };
        const PG_NO_ACCESS_METHOD: FeatureSet = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                table_access_method: false,
                ..CreateTableClauseSyntax::POSTGRES
            }),
        );
        const PG_NO_ACCESS_METHOD_DIALECT: FeatureDialect = FeatureDialect {
            features: &PG_NO_ACCESS_METHOD,
        };
        const PG_NO_WITHOUT_OIDS: FeatureSet = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                without_oids: false,
                ..CreateTableClauseSyntax::POSTGRES
            }),
        );
        const PG_NO_WITHOUT_OIDS_DIALECT: FeatureDialect = FeatureDialect {
            features: &PG_NO_WITHOUT_OIDS,
        };
        // UNLOGGED: PostgreSQL + DuckDB accept; the rest (and the flag-cleared base) reject.
        let unlogged = "CREATE UNLOGGED TABLE t (a INT)";
        parse_with(unlogged, crate::ParseConfig::new(crate::dialect::Postgres))
            .expect("PostgreSQL accepts UNLOGGED");
        parse_with(unlogged, crate::ParseConfig::new(DuckDb)).expect("DuckDB accepts UNLOGGED");
        parse_with(
            "CREATE OR REPLACE UNLOGGED TABLE t (a INT)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("DuckDB accepts OR REPLACE UNLOGGED (engine-verified)");
        parse_with(unlogged, crate::ParseConfig::new(Ansi)).expect_err("ANSI rejects UNLOGGED");
        parse_with(unlogged, crate::ParseConfig::new(MySql)).expect_err("MySQL rejects UNLOGGED");
        parse_with(unlogged, crate::ParseConfig::new(Sqlite)).expect_err("SQLite rejects UNLOGGED");
        parse_with(unlogged, crate::ParseConfig::new(PG_NO_UNLOGGED_DIALECT))
            .expect_err("the gate off must reject");
        // DuckDB matches PostgreSQL's TEMP/UNLOGGED mutual exclusion (engine-verified).
        parse_with(
            "CREATE TEMP UNLOGGED TABLE t (a INT)",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("DuckDB rejects TEMP UNLOGGED");
        // The PostgreSQL-only clauses: off everywhere else, including DuckDB. MySQL is asserted
        // only where its open `<name> [=] <value>` trailing-option list does not apply: that
        // recorded-bound list reads a trailing `USING heap` / `WITHOUT OIDS` as an ordinary
        // KeyValue option pair, so those two forms parse (as MySQL options, not these clauses)
        // under the MySQL preset.
        // `sqlite_rejects`: the column-level `STORAGE MAIN COMPRESSION lz4` words are absorbed
        // by SQLite's liberal affinity type name (`liberal_type_names`) — SQLite parse-accepts
        // `a TEXT STORAGE MAIN COMPRESSION lz4` as a five-word affinity type (engine-verified on
        // sqlite3 3.43.2), so this is not a SQLite reject. The two table-level clauses (`USING
        // heap`, `WITHOUT OIDS`) sit past the column list where no type-name run reaches them,
        // so SQLite still rejects those.
        for (sql, flag_off, mysql_rejects, sqlite_rejects) in [
            (
                "CREATE TABLE t (a TEXT STORAGE MAIN COMPRESSION lz4)",
                PG_NO_STORAGE_DIALECT,
                true,
                false,
            ),
            (
                "CREATE TABLE t (a INT) USING heap",
                PG_NO_ACCESS_METHOD_DIALECT,
                false,
                true,
            ),
            (
                "CREATE TABLE t (a INT) WITHOUT OIDS",
                PG_NO_WITHOUT_OIDS_DIALECT,
                false,
                true,
            ),
        ] {
            parse_with(sql, crate::ParseConfig::new(Ansi))
                .expect_err(&format!("ANSI rejects {sql:?}"));
            if mysql_rejects {
                parse_with(sql, crate::ParseConfig::new(MySql))
                    .expect_err(&format!("MySQL rejects {sql:?}"));
            }
            if sqlite_rejects {
                parse_with(sql, crate::ParseConfig::new(Sqlite))
                    .expect_err(&format!("SQLite rejects {sql:?}"));
            }
            parse_with(sql, crate::ParseConfig::new(DuckDb))
                .expect_err(&format!("DuckDB rejects {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(flag_off))
                .expect_err(&format!("the gate off must reject {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("PostgreSQL accepts {sql:?}: {err:?}"));
        }
    }

    /// The parsed persistence/storage clauses land on the right nodes: `unlogged` on the
    /// statement, the `STORAGE`/`COMPRESSION` idents on the column (not its constraint list),
    /// and the access method on the table.
    #[test]
    fn persistence_and_storage_populate_the_ast() {
        let parsed = parse_with(
            "CREATE UNLOGGED TABLE t (a TEXT STORAGE MAIN COMPRESSION lz4 NOT NULL) USING heap2",
            crate::ParseConfig::new(crate::dialect::Postgres),
        )
        .expect("parses");
        let create = create_table_of(&parsed);
        assert!(create.unlogged, "UNLOGGED is recorded");
        assert!(create.temporary.is_none(), "UNLOGGED is not temporary");
        let method = create.access_method.as_ref().expect("USING is recorded");
        assert_eq!(parsed.resolver().resolve(method.sym), "heap2");
        let CreateTableBody::Definition { elements, .. } = &create.body else {
            panic!("expected a definition body");
        };
        let TableElement::Column { column, .. } = &elements[0] else {
            panic!("expected a column");
        };
        let storage = column.storage.as_ref().expect("STORAGE is recorded");
        assert_eq!(parsed.resolver().resolve(storage.sym), "MAIN");
        let compression = column
            .compression
            .as_ref()
            .expect("COMPRESSION is recorded");
        assert_eq!(parsed.resolver().resolve(compression.sym), "lz4");
        assert_eq!(
            column.constraints.len(),
            1,
            "the storage attributes stay out of the constraint list"
        );
    }

    /// Round-trip the typed-table (`OF <type>`) surface under the PostgreSQL target: the bare
    /// and schema-qualified type name, `TEMP` / `IF NOT EXISTS` prefixes, every augmentation
    /// element form (a typeless column with constraints, a table constraint, both mixed), and
    /// the trailing `PARTITION BY` / `USING` / options interplay.
    #[test]
    fn postgres_of_type_round_trip() {
        for sql in [
            "CREATE TABLE t OF mytype",
            "CREATE TABLE t OF myschema.mytype",
            "CREATE TEMP TABLE t OF mytype",
            "CREATE TABLE IF NOT EXISTS t OF mytype",
            "CREATE TABLE t OF mytype (a NOT NULL)",
            "CREATE TABLE t OF mytype (a DEFAULT 5)",
            "CREATE TABLE t OF mytype (PRIMARY KEY (a))",
            "CREATE TABLE t OF mytype (a NOT NULL, PRIMARY KEY (a))",
            "CREATE TABLE t OF mytype (CONSTRAINT ck CHECK (a > 0))",
            "CREATE TABLE t OF mytype PARTITION BY RANGE (a)",
            "CREATE TABLE t OF mytype (a NOT NULL) PARTITION BY RANGE (a)",
            "CREATE TABLE t OF mytype USING heap",
            "CREATE TABLE t OF mytype WITH (fillfactor = 70)",
            "CREATE TABLE t OF mytype ON COMMIT DROP",
            "CREATE TABLE t OF mytype TABLESPACE ts",
            "CREATE TABLE t OF mytype USING heap WITHOUT OIDS",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }
    }

    /// The `WITH OPTIONS` noise phrase in a typed-table augmentation column normalizes away
    /// (PostgreSQL's `columnOptions` does not preserve it — the `PARTITION OF` precedent), and
    /// the augmentation columns are typeless (`data_type` stays `None`) with the type name on
    /// the body.
    #[test]
    fn of_type_augmentation_normalizes_with_options_and_is_typeless() {
        let with_options = parse_with(
            "CREATE TABLE t OF mytype (a WITH OPTIONS NOT NULL) PARTITION BY RANGE (a)",
            crate::ParseConfig::new(crate::dialect::Postgres),
        )
        .expect("WITH OPTIONS parses");
        let rendered = Renderer::new(PG_DIALECT)
            .render_parsed(&with_options)
            .expect("renders");
        assert_eq!(
            rendered, "CREATE TABLE t OF mytype (a NOT NULL) PARTITION BY RANGE (a)",
            "WITH OPTIONS is consumed, not preserved"
        );
        let create = create_table_of(&with_options);
        let CreateTableBody::OfType {
            type_name,
            elements,
            ..
        } = &create.body
        else {
            panic!("expected an OF-type body");
        };
        assert_eq!(type_name.0.len(), 1, "bare type name");
        let [TableElement::Column { column, .. }] = elements.as_slice() else {
            panic!("expected one augmentation column");
        };
        assert!(
            column.data_type.is_none(),
            "an augmentation column inherits its type from the composite type"
        );
        assert!(create.inherits.is_empty(), "an OF body takes no INHERITS");
        assert!(
            create.partition_by.is_some(),
            "PARTITION BY combines with an OF body"
        );
    }

    /// The reject boundary PostgreSQL enforces at raw-parse for typed tables (each case
    /// engine-measured on libpg_query): an empty augmentation list, a *typed* augmentation
    /// column (with or without `WITH OPTIONS` — the composite type owns the column types), and
    /// an `INHERITS` clause on the `OF` body (the grammar reserves `OptInherit` for the plain
    /// `(elements)` body).
    #[test]
    fn postgres_of_type_reject_boundary() {
        use crate::dialect::Postgres;
        for sql in [
            "CREATE TABLE t OF mytype ()",
            "CREATE TABLE t OF mytype (a INT)",
            "CREATE TABLE t OF mytype (a WITH OPTIONS INT)",
            "CREATE TABLE t OF mytype INHERITS (p)",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("PostgreSQL must reject {sql:?}"));
        }
    }

    /// Typed tables are gated to PostgreSQL/Lenient: every other preset (DuckDB engine-verified)
    /// leaves `OF` after the table name as leftover input, and a PostgreSQL base with
    /// `typed_tables` cleared rejects it too — the flag, not the concrete dialect, drives
    /// acceptance.
    #[test]
    fn of_type_is_gated() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};
        const PG_NO_TYPED: FeatureSet = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY.create_table_clause_syntax(CreateTableClauseSyntax {
                typed_tables: false,
                ..CreateTableClauseSyntax::POSTGRES
            }),
        );
        const PG_NO_TYPED_DIALECT: FeatureDialect = FeatureDialect {
            features: &PG_NO_TYPED,
        };
        let sql = "CREATE TABLE t OF mytype (a WITH OPTIONS NOT NULL)";
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI has no typed tables");
        parse_with(sql, crate::ParseConfig::new(MySql)).expect_err("MySQL has no typed tables");
        parse_with(sql, crate::ParseConfig::new(Sqlite)).expect_err("SQLite has no typed tables");
        parse_with(sql, crate::ParseConfig::new(DuckDb)).expect_err("DuckDB has no typed tables");
        parse_with(sql, crate::ParseConfig::new(PG_NO_TYPED_DIALECT))
            .expect_err("the gate off must reject");
        parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
            .unwrap_or_else(|err| panic!("PostgreSQL accepts {sql:?}: {err:?}"));
    }

    /// Round-trip every PostgreSQL `EXCLUDE` exclusion-constraint surface: the `USING <method>`
    /// and bare (default-btree) forms, multi-element lists, the three `index_elem` key shapes
    /// (bare column, parenthesized expr, function call), `COLLATE` / operator-class / ordering
    /// element tails, bare and `OPERATOR(...)` operators, and the `INCLUDE` / `WITH (…)` /
    /// `USING INDEX TABLESPACE` / `WHERE` constraint tails plus a named constraint with
    /// characteristics.
    #[test]
    fn postgres_exclude_constraint_round_trips() {
        for sql in [
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (a WITH =))",
            "CREATE TABLE t (a INTEGER, EXCLUDE (a WITH =))",
            "CREATE TABLE t (a INTEGER, b INTEGER, EXCLUDE USING gist (a WITH =, b WITH &&))",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist ((a + 1) WITH =))",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (lower(a) WITH =))",
            "CREATE TABLE t (a TEXT, EXCLUDE USING gist (a COLLATE \"C\" WITH =))",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (a int4_ops WITH =))",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING btree (a DESC NULLS LAST WITH =))",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (a WITH -|-))",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (a WITH OPERATOR(=)))",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (a WITH OPERATOR(pg_catalog.=)))",
            "CREATE TABLE t (a INTEGER, b INTEGER, EXCLUDE USING btree (a WITH =) INCLUDE (b))",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (a WITH =) WITH (fillfactor = 70))",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (a WITH =) USING INDEX TABLESPACE ts)",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (a WITH =) WHERE (a > 0))",
            "CREATE TABLE t (a INTEGER, CONSTRAINT e EXCLUDE (a WITH =) INITIALLY DEFERRED)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }
    }

    /// The `EXCLUDE` element key follows PostgreSQL's `index_elem`, not a full `a_expr`: a bare
    /// non-parenthesized expression (a literal, a binary op) is a parse error, exactly as
    /// PostgreSQL rejects it — only the bare-column / function-call / parenthesized-expr forms
    /// parse.
    #[test]
    fn postgres_exclude_element_key_is_index_elem() {
        for sql in [
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (5 WITH =))",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (a + 1 WITH =))",
        ] {
            parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres)).expect_err(
                &format!("a bare non-parenthesized key must reject: {sql:?}"),
            );
        }
    }

    /// `EXCLUDE` is gated by `exclusion_constraints`: on for PostgreSQL, off for every other
    /// stock dialect (DuckDB included), where the keyword surfaces as a clean parse error.
    #[test]
    fn postgres_exclude_constraint_is_gated() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};
        let sql = "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (a WITH =))";
        parse_with(sql, crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no exclusion constraints");
        parse_with(sql, crate::ParseConfig::new(MySql))
            .expect_err("MySQL has no exclusion constraints");
        parse_with(sql, crate::ParseConfig::new(Sqlite))
            .expect_err("SQLite has no exclusion constraints");
        parse_with(sql, crate::ParseConfig::new(DuckDb))
            .expect_err("DuckDB rejects exclusion constraints");
        parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
            .expect("PostgreSQL accepts EXCLUDE");
    }

    /// Round-trip the `CREATE TABLE … AS EXECUTE <prepared> [(args)] [WITH [NO] DATA]` CTAS
    /// source, with and without a column list, arguments, and the populate flag.
    #[test]
    fn postgres_create_table_as_execute_round_trips() {
        for sql in [
            "CREATE TABLE t AS EXECUTE p",
            "CREATE TABLE t (x, y) AS EXECUTE plan(1, 2)",
            "CREATE TABLE t AS EXECUTE p WITH DATA",
            "CREATE TEMPORARY TABLE t AS EXECUTE q(200, 'x') WITH NO DATA",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }
    }

    /// `AS EXECUTE` is gated by `create_table_as_execute`: on for PostgreSQL, off elsewhere
    /// (DuckDB included), where the inline-query CTAS path rejects the `EXECUTE` keyword.
    #[test]
    fn postgres_create_table_as_execute_is_gated() {
        use crate::dialect::{Ansi, DuckDb, MySql};
        let sql = "CREATE TABLE t AS EXECUTE p";
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI has no AS EXECUTE");
        parse_with(sql, crate::ParseConfig::new(MySql)).expect_err("MySQL has no AS EXECUTE");
        parse_with(sql, crate::ParseConfig::new(DuckDb)).expect_err("DuckDB rejects AS EXECUTE");
        parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
            .expect("PostgreSQL accepts AS EXECUTE");
    }

    /// Round-trip the `NO INHERIT` / `NOT VALID` constraint markers on the constraint kinds
    /// PostgreSQL admits them (table + column `CHECK`, `NOT VALID` on `FOREIGN KEY`), and reject
    /// the combinations PostgreSQL rejects in its grammar action (`NOT VALID` on
    /// `PRIMARY KEY`/`UNIQUE`/`EXCLUDE`, `NO INHERIT` on anything but `CHECK`, and any column
    /// `NOT VALID`) — the over-acceptance boundary.
    #[test]
    fn postgres_constraint_markers_round_trip_and_reject() {
        for sql in [
            "CREATE TABLE t (a INTEGER, CHECK (a > 0) NO INHERIT)",
            "CREATE TABLE t (a INTEGER, CHECK (a > 0) NOT VALID)",
            "CREATE TABLE t (a INTEGER, CHECK (a > 0) NO INHERIT NOT VALID)",
            "CREATE TABLE t (a INTEGER, FOREIGN KEY (a) REFERENCES u (x) NOT VALID)",
            "CREATE TABLE t (a INTEGER CHECK (a > 0) NO INHERIT)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }
        for sql in [
            "CREATE TABLE t (a INTEGER, PRIMARY KEY (a) NOT VALID)",
            "CREATE TABLE t (a INTEGER, UNIQUE (a) NOT VALID)",
            "CREATE TABLE t (a INTEGER, EXCLUDE USING gist (a WITH =) NOT VALID)",
            "CREATE TABLE t (a INTEGER, FOREIGN KEY (a) REFERENCES u (x) NO INHERIT)",
            "CREATE TABLE t (a INTEGER, PRIMARY KEY (a) NO INHERIT)",
            "CREATE TABLE t (a INTEGER CHECK (a > 0) NOT VALID)",
            "CREATE TABLE t (a INTEGER REFERENCES u (x) NOT VALID)",
        ] {
            parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres)).expect_err(
                &format!("PostgreSQL rejects this marker/kind pair: {sql:?}"),
            );
        }
    }

    /// The `NO INHERIT` / `NOT VALID` markers are gated by `constraint_no_inherit_not_valid`, a
    /// shared PostgreSQL+DuckDB surface — on for both, off for ANSI/MySQL/SQLite.
    #[test]
    fn postgres_constraint_markers_are_gated() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};
        let sql = "CREATE TABLE t (a INTEGER, CHECK (a > 0) NO INHERIT)";
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI has no NO INHERIT marker");
        parse_with(sql, crate::ParseConfig::new(MySql))
            .expect_err("MySQL has no NO INHERIT marker");
        parse_with(sql, crate::ParseConfig::new(Sqlite))
            .expect_err("SQLite has no NO INHERIT marker");
        parse_with(sql, crate::ParseConfig::new(DuckDb))
            .expect("DuckDB shares the NO INHERIT marker");
        parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
            .expect("PostgreSQL accepts NO INHERIT");
    }

    /// Round-trip the PostgreSQL `UNIQUE`/`PRIMARY KEY` index-constraint parameters: the covering
    /// `INCLUDE`, the `NULLS [NOT] DISTINCT` null-treatment (table + column), and the column
    /// `USING INDEX TABLESPACE`.
    #[test]
    fn postgres_index_constraint_parameters_round_trip() {
        for sql in [
            "CREATE TABLE t (a INTEGER, b INTEGER, PRIMARY KEY (a) INCLUDE (b))",
            "CREATE TABLE t (a INTEGER, b INTEGER, UNIQUE (a) INCLUDE (b))",
            "CREATE TABLE t (a INTEGER, UNIQUE NULLS NOT DISTINCT (a))",
            "CREATE TABLE t (a INTEGER, b INTEGER, UNIQUE NULLS NOT DISTINCT (a) INCLUDE (b))",
            "CREATE TABLE t (a INTEGER UNIQUE NULLS NOT DISTINCT, t TEXT)",
            "CREATE TABLE t (a INTEGER UNIQUE NULLS DISTINCT)",
            "CREATE TABLE t (a INTEGER PRIMARY KEY USING INDEX TABLESPACE pg_default)",
            "CREATE TABLE t (a INTEGER UNIQUE USING INDEX TABLESPACE ts)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }
    }

    /// The index-constraint parameters are gated by `index_constraint_parameters`: on for
    /// PostgreSQL, off elsewhere (DuckDB included).
    #[test]
    fn postgres_index_constraint_parameters_are_gated() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};
        let sql = "CREATE TABLE t (a INTEGER, b INTEGER, PRIMARY KEY (a) INCLUDE (b))";
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI has no INCLUDE");
        parse_with(sql, crate::ParseConfig::new(MySql)).expect_err("MySQL has no INCLUDE");
        parse_with(sql, crate::ParseConfig::new(Sqlite)).expect_err("SQLite has no INCLUDE");
        parse_with(sql, crate::ParseConfig::new(DuckDb)).expect_err("DuckDB rejects INCLUDE");
        parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
            .expect("PostgreSQL accepts INCLUDE");
    }

    /// A signed `numeric`/`decimal` precision/scale (`NUMERIC(3, -6)`) parses and round-trips
    /// under PostgreSQL — a raw-parse laxity gated by `signed_type_modifier` — while ANSI/MySQL/
    /// SQLite/DuckDB reject the leading sign as a clean parse error.
    #[test]
    fn postgres_signed_numeric_type_modifier() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};
        for sql in [
            "CREATE TABLE t (a NUMERIC(3, -6))",
            "CREATE TABLE t (a NUMERIC(-3, 6))",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            let rendered = Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders");
            assert_eq!(rendered, sql, "canonical render must round-trip {sql:?}");
        }
        // A `DECIMAL` spelling with a signed scale parses and canonicalizes to `NUMERIC` under the
        // PostgreSQL target (the pre-existing DECIMAL -> NUMERIC spelling normalization), carrying
        // the negative scale through.
        let parsed = parse_with(
            "CREATE TABLE t (a DECIMAL(5, -2))",
            crate::ParseConfig::new(crate::dialect::Postgres),
        )
        .expect("DECIMAL(5, -2) parses");
        assert_eq!(
            Renderer::new(PG_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "CREATE TABLE t (a NUMERIC(5, -2))",
        );
        let sql = "CREATE TABLE t (a NUMERIC(3, -6))";
        parse_with(sql, crate::ParseConfig::new(Ansi))
            .expect_err("ANSI requires an unsigned modifier");
        parse_with(sql, crate::ParseConfig::new(MySql))
            .expect_err("MySQL requires an unsigned modifier");
        parse_with(sql, crate::ParseConfig::new(Sqlite))
            .expect_err("SQLite requires an unsigned modifier");
        parse_with(sql, crate::ParseConfig::new(DuckDb))
            .expect_err("DuckDB requires an unsigned modifier");
    }

    /// A Lenient parse + render dialect (the permissive superset), for the shared `ALTER
    /// DATABASE` head disambiguation (both DuckDB's `SET ALIAS` and MySQL's option list accept).
    const LENIENT_RENDER: FeatureDialect = FeatureDialect {
        features: &FeatureSet::LENIENT,
    };

    /// Every grammar-valid form of the MySQL server (`CREATE`/`ALTER`/`DROP SERVER`), `ALTER
    /// INSTANCE`, and `ALTER {DATABASE|SCHEMA}` option families round-trips byte-identically —
    /// the option `[=]`/`[DEFAULT]` spellings, the `CHARACTER SET`/`CHARSET` synonym, the
    /// `DATABASE`/`SCHEMA` synonym, the optional (default-schema) name, and every instance
    /// action. The same forms are live-oracle-verified grammar-valid in
    /// `corpus_mysql_verdicts::mysql_server_instance_database_live_oracle_parity`.
    #[test]
    fn server_instance_database_ddl_round_trips() {
        use crate::ast::{
            AlterDatabaseOption, AlterInstanceAction, CharsetKeyword, ServerOptionKind,
        };

        // Structural spot-checks.
        let create = parse_with(
            "CREATE SERVER s FOREIGN DATA WRAPPER mysql OPTIONS (HOST 'h', PORT 3306)",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .unwrap();
        let Statement::CreateServer { create, .. } = &create.statements()[0] else {
            panic!("expected a CREATE SERVER statement");
        };
        assert_eq!(create.options.len(), 2);
        assert_eq!(create.options[0].kind, ServerOptionKind::Host);
        assert_eq!(create.options[1].kind, ServerOptionKind::Port);

        // The unqualified `ALTER DATABASE` form (default schema) leaves the name unset.
        let noname = parse_with(
            "ALTER DATABASE CHARACTER SET utf8mb4",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .unwrap();
        let Statement::AlterDatabaseOptions { alter, .. } = &noname.statements()[0] else {
            panic!("expected an ALTER DATABASE options statement");
        };
        assert!(alter.name.is_none(), "no name binds the default schema");
        assert!(matches!(
            alter.options[0],
            AlterDatabaseOption::CharacterSet {
                keyword: CharsetKeyword::CharacterSet,
                ..
            },
        ));

        let inst = parse_with(
            "ALTER INSTANCE RELOAD TLS FOR CHANNEL ch NO ROLLBACK ON ERROR",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .unwrap();
        let Statement::AlterInstance { alter, .. } = &inst.statements()[0] else {
            panic!("expected an ALTER INSTANCE statement");
        };
        assert!(matches!(
            &alter.action,
            AlterInstanceAction::ReloadTls {
                channel: Some(_),
                no_rollback_on_error: true,
                ..
            },
        ));

        for sql in [
            // CREATE / ALTER / DROP SERVER.
            "CREATE SERVER s FOREIGN DATA WRAPPER mysql OPTIONS (HOST 'localhost')",
            "CREATE SERVER s FOREIGN DATA WRAPPER mysql OPTIONS (HOST 'h', DATABASE 'd', \
             USER 'u', PASSWORD 'p', SOCKET 'sk', OWNER 'o', PORT 3306)",
            "CREATE SERVER 'srv' FOREIGN DATA WRAPPER 'w' OPTIONS (PORT 3306)",
            "ALTER SERVER s OPTIONS (HOST 'h2')",
            "ALTER SERVER s OPTIONS (PORT 3307, USER 'u')",
            "DROP SERVER s",
            "DROP SERVER IF EXISTS s",
            // ALTER INSTANCE.
            "ALTER INSTANCE ROTATE INNODB MASTER KEY",
            "ALTER INSTANCE ROTATE BINLOG MASTER KEY",
            "ALTER INSTANCE RELOAD TLS",
            "ALTER INSTANCE RELOAD TLS NO ROLLBACK ON ERROR",
            "ALTER INSTANCE RELOAD TLS FOR CHANNEL ch",
            "ALTER INSTANCE RELOAD TLS FOR CHANNEL ch NO ROLLBACK ON ERROR",
            "ALTER INSTANCE RELOAD KEYRING",
            "ALTER INSTANCE ENABLE INNODB REDO_LOG",
            "ALTER INSTANCE DISABLE INNODB REDO_LOG",
            // ALTER DATABASE / SCHEMA option list.
            "ALTER DATABASE d CHARACTER SET utf8mb4",
            "ALTER DATABASE d CHARACTER SET = utf8mb4",
            "ALTER DATABASE d DEFAULT CHARACTER SET utf8mb4",
            "ALTER DATABASE d CHARSET utf8mb4",
            "ALTER DATABASE d CHARACTER SET binary",
            "ALTER DATABASE d COLLATE utf8mb4_bin",
            "ALTER DATABASE d DEFAULT COLLATE = utf8mb4_bin",
            "ALTER DATABASE d ENCRYPTION 'Y'",
            "ALTER DATABASE d DEFAULT ENCRYPTION = 'N'",
            "ALTER DATABASE d READ ONLY 1",
            "ALTER DATABASE d READ ONLY = 0",
            "ALTER DATABASE d READ ONLY DEFAULT",
            "ALTER DATABASE d CHARACTER SET utf8mb4 COLLATE utf8mb4_bin",
            "ALTER DATABASE CHARACTER SET utf8mb4",
            "ALTER SCHEMA d CHARACTER SET utf8mb4",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MYSQL_RENDER))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    /// The measured reject boundaries — both the MySQL preset and the live oracle reject these
    /// (the oracle side is pinned in `m3::SCHEMA_INDEPENDENT_REJECT`). Server: an empty/absent
    /// `OPTIONS` list, a wrong option value type, an unknown option, a comma-list `DROP`.
    /// Instance: a wrong keyword and a rollback tail on the wrong action. Database: no option,
    /// a non-`ternary` `READ ONLY`, a `DEFAULT READ ONLY` prefix, a dotted name.
    #[test]
    fn server_instance_database_ddl_reject_boundaries() {
        for sql in [
            "CREATE SERVER s FOREIGN DATA WRAPPER mysql OPTIONS ()",
            "CREATE SERVER s FOREIGN DATA WRAPPER mysql",
            "CREATE SERVER s OPTIONS (HOST 'h')",
            "ALTER SERVER s",
            "CREATE SERVER s FOREIGN DATA WRAPPER mysql OPTIONS (PORT '3306')",
            "CREATE SERVER s FOREIGN DATA WRAPPER mysql OPTIONS (HOST 123)",
            "CREATE SERVER s FOREIGN DATA WRAPPER mysql OPTIONS (FOO 'bar')",
            "DROP SERVER a, b",
            "ALTER INSTANCE ROTATE FOO MASTER KEY",
            "ALTER INSTANCE ENABLE INNODB FOO",
            "ALTER INSTANCE RELOAD TLS FOR CHANNEL 'ch'",
            "ALTER INSTANCE",
            "ALTER INSTANCE ROTATE INNODB MASTER KEY NO ROLLBACK ON ERROR",
            "ALTER DATABASE d",
            "ALTER DATABASE d READ ONLY 2",
            "ALTER DATABASE d DEFAULT READ ONLY 1",
            "ALTER DATABASE d.x CHARACTER SET utf8mb4",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err(&format!("MySQL rejects {sql:?}"));
        }
    }

    /// The three families are gated: off in ANSI/SQLite/PostgreSQL (no `SERVER`/`INSTANCE`
    /// object and no MySQL `ALTER DATABASE` option grammar), on in MySQL and the Lenient
    /// superset. Under Lenient both `ALTER DATABASE` behaviours coexist: DuckDB's `SET ALIAS`
    /// relocation and MySQL's option list, disambiguated by lookahead.
    #[test]
    fn server_instance_database_ddl_dialect_gating() {
        for sql in [
            "CREATE SERVER s FOREIGN DATA WRAPPER mysql OPTIONS (HOST 'h')",
            "ALTER INSTANCE RELOAD TLS",
            "ALTER DATABASE d CHARACTER SET utf8mb4",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("MySQL accepts {sql:?}: {err:?}"));
            parse_with(sql, crate::ParseConfig::new(LENIENT_RENDER))
                .unwrap_or_else(|err| panic!("Lenient accepts {sql:?}: {err:?}"));
            parse_with(sql, crate::ParseConfig::new(Ansi))
                .expect_err(&format!("ANSI rejects {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .expect_err(&format!("SQLite rejects {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .expect_err(&format!("PostgreSQL rejects {sql:?}"));
        }
        // Lenient keeps DuckDB's disjoint `ALTER DATABASE … SET ALIAS TO` behaviour alongside
        // MySQL's option list — the head is disambiguated by the `SET` lookahead.
        let alias = parse_with(
            "ALTER DATABASE d SET ALIAS TO e",
            crate::ParseConfig::new(LENIENT_RENDER),
        )
        .expect("Lenient keeps DuckDB SET ALIAS");
        assert!(matches!(
            alias.statements()[0],
            Statement::AlterDatabase { .. }
        ));
        let options = parse_with(
            "ALTER DATABASE d CHARACTER SET utf8mb4",
            crate::ParseConfig::new(LENIENT_RENDER),
        )
        .expect("Lenient accepts MySQL options");
        assert!(matches!(
            options.statements()[0],
            Statement::AlterDatabaseOptions { .. }
        ));
    }
    /// Every grammar-valid MySQL tablespace / logfile-group storage-DDL form round-trips
    /// byte-identically: the `[UNDO]` variants, `ADD DATAFILE`, `USE LOGFILE GROUP`, every option
    /// (size / integer / string / engine / wait), `=`-optionality, comma-separated options, and
    /// the size-literal suffixes (`128M`, `2G`, `16k`) and plain byte counts. The same forms are
    /// live-oracle-verified grammar-valid in
    /// `corpus_mysql_verdicts::mysql_tablespace_logfile_live_oracle_parity`.
    #[test]
    fn tablespace_logfile_family_round_trips() {
        for sql in [
            // CREATE TABLESPACE
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd'",
            "CREATE TABLESPACE ts",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' FILE_BLOCK_SIZE = 8192",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' FILE_BLOCK_SIZE 8192",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENCRYPTION = 'Y'",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENGINE = InnoDB",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENGINE InnoDB",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENGINE = 'InnoDB'",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' STORAGE ENGINE = ndbcluster",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' AUTOEXTEND_SIZE = 4M",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 128M",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 134217728",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' MAX_SIZE = 2G",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' EXTENT_SIZE = 1M",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' NODEGROUP = 0",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' WAIT",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' NO_WAIT",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' COMMENT = 'hi'",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENGINE_ATTRIBUTE = '{}'",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 128M ENGINE = InnoDB",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' ENGINE = InnoDB INITIAL_SIZE = 128M",
            "CREATE TABLESPACE ts USE LOGFILE GROUP lg INITIAL_SIZE = 128M",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' USE LOGFILE GROUP lg",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 16k",
            // CREATE UNDO TABLESPACE
            "CREATE UNDO TABLESPACE ut ADD DATAFILE 'ut.ibu'",
            "CREATE UNDO TABLESPACE ut ADD DATAFILE 'ut.ibu' ENGINE = InnoDB",
            // ALTER TABLESPACE
            "ALTER TABLESPACE ts ADD DATAFILE 'ts2.ibd'",
            "ALTER TABLESPACE ts DROP DATAFILE 'ts2.ibd'",
            "ALTER TABLESPACE ts RENAME TO ts2",
            "ALTER TABLESPACE ts INITIAL_SIZE = 128M",
            "ALTER TABLESPACE ts AUTOEXTEND_SIZE = 4M",
            "ALTER TABLESPACE ts ENGINE = InnoDB",
            "ALTER TABLESPACE ts ENCRYPTION = 'Y'",
            "ALTER TABLESPACE ts WAIT",
            "ALTER TABLESPACE ts ADD DATAFILE 'ts2.ibd' ENGINE = InnoDB",
            "ALTER TABLESPACE ts ADD DATAFILE 'ts2.ibd' WAIT",
            // ALTER UNDO TABLESPACE
            "ALTER UNDO TABLESPACE ut SET ACTIVE",
            "ALTER UNDO TABLESPACE ut SET INACTIVE",
            "ALTER UNDO TABLESPACE ut SET ACTIVE ENGINE = InnoDB",
            // DROP TABLESPACE
            "DROP TABLESPACE ts",
            "DROP TABLESPACE ts ENGINE = InnoDB",
            "DROP TABLESPACE ts ENGINE InnoDB",
            "DROP TABLESPACE ts WAIT",
            "DROP TABLESPACE ts ENGINE = InnoDB WAIT",
            // DROP UNDO TABLESPACE
            "DROP UNDO TABLESPACE ut",
            "DROP UNDO TABLESPACE ut ENGINE = InnoDB",
            // CREATE LOGFILE GROUP
            "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat'",
            "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' INITIAL_SIZE = 16M",
            "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' INITIAL_SIZE 16M",
            "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' UNDO_BUFFER_SIZE = 8M",
            "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' REDO_BUFFER_SIZE = 8M",
            "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' NODEGROUP = 0",
            "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' WAIT",
            "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' COMMENT = 'x'",
            "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' ENGINE = ndbcluster",
            "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' INITIAL_SIZE = 16M ENGINE = InnoDB",
            // ALTER LOGFILE GROUP
            "ALTER LOGFILE GROUP lg ADD UNDOFILE 'undo2.dat'",
            "ALTER LOGFILE GROUP lg ADD UNDOFILE 'undo2.dat' INITIAL_SIZE = 16M",
            "ALTER LOGFILE GROUP lg ADD UNDOFILE 'undo2.dat' ENGINE = ndbcluster",
            "ALTER LOGFILE GROUP lg ADD UNDOFILE 'undo2.dat' WAIT",
            "ALTER LOGFILE GROUP lg ADD UNDOFILE 'undo2.dat' INITIAL_SIZE = 16M ENGINE = ndbcluster",
            // DROP LOGFILE GROUP
            "DROP LOGFILE GROUP lg",
            "DROP LOGFILE GROUP lg ENGINE = ndbcluster",
            "DROP LOGFILE GROUP lg ENGINE = ndbcluster WAIT",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("{sql:?} should parse: {err:?}"));
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip {sql:?}");
        }

        // The `opt_comma` option separator is noise MySQL ignores; the canonical render drops it in
        // favour of the space form, so the comma spelling is asserted as a normalization, not a
        // byte-identical round-trip.
        let comma = parse_with(
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 128M, ENGINE = InnoDB",
            crate::ParseConfig::new(MySql),
        )
        .expect("comma-separated options parse");
        assert_eq!(
            Renderer::new(MYSQL_RENDER)
                .render_parsed(&comma)
                .expect("renders"),
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 128M ENGINE = InnoDB",
        );
    }

    /// Structural spot-checks: the `UNDO` flag, the size-literal unit tag, and the action-enum
    /// distinctions the round-trip alone does not pin.
    #[test]
    fn tablespace_logfile_structural() {
        use crate::ast::{
            AlterTablespaceAction, SizeUnit, TablespaceOption, TablespaceSizeOption,
            UndoTablespaceState,
        };

        let parsed = parse_with(
            "CREATE UNDO TABLESPACE ut ADD DATAFILE 'ut.ibu' ENGINE = InnoDB",
            crate::ParseConfig::new(MySql),
        )
        .expect("parses");
        let Statement::CreateTablespace { create, .. } = &parsed.statements()[0] else {
            panic!("expected CREATE TABLESPACE");
        };
        assert!(create.undo, "UNDO flag recorded");
        assert!(create.datafile.is_some(), "datafile recorded");

        let parsed = parse_with(
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' MAX_SIZE = 2G",
            crate::ParseConfig::new(MySql),
        )
        .unwrap();
        let Statement::CreateTablespace { create, .. } = &parsed.statements()[0] else {
            panic!("expected CREATE TABLESPACE");
        };
        let TablespaceOption::Size { kind, size, .. } = &create.options[0] else {
            panic!("expected a size option");
        };
        assert_eq!(*kind, TablespaceSizeOption::MaxSize);
        assert_eq!(size.unit, Some(SizeUnit::Giga), "the G suffix is tagged");

        let parsed = parse_with(
            "ALTER UNDO TABLESPACE ut SET INACTIVE",
            crate::ParseConfig::new(MySql),
        )
        .unwrap();
        let Statement::AlterTablespace { alter, .. } = &parsed.statements()[0] else {
            panic!("expected ALTER TABLESPACE");
        };
        assert!(matches!(
            alter.action,
            AlterTablespaceAction::SetState {
                state: UndoTablespaceState::Inactive,
                ..
            },
        ));

        // A bare byte-count size carries no unit tag.
        let parsed = parse_with(
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 134217728",
            crate::ParseConfig::new(MySql),
        )
        .unwrap();
        let Statement::CreateTablespace { create, .. } = &parsed.statements()[0] else {
            panic!("expected CREATE TABLESPACE");
        };
        let TablespaceOption::Size { size, .. } = &create.options[0] else {
            panic!("expected a size option");
        };
        assert_eq!(size.unit, None, "a plain integer size has no unit");
    }

    /// The parser rejects each form the live server 1064-rejects (per-context option restriction,
    /// mandatory datafile / undofile, `RENAME` taking no options, the size-literal adjacency rule),
    /// and the two-character / spaced size suffixes the server also rejects (`16MB` as
    /// `ER_WRONG_SIZE_NUMBER`, `16 M` at parse). Live-oracle-measured boundaries.
    #[test]
    fn tablespace_logfile_reject_boundaries() {
        for sql in [
            // UNDO tablespace: only ENGINE options; datafile mandatory.
            "CREATE UNDO TABLESPACE ut ADD DATAFILE 'ut.ibu' INITIAL_SIZE = 128M",
            "CREATE UNDO TABLESPACE ut",
            "ALTER UNDO TABLESPACE ut SET INACTIVE INITIAL_SIZE = 1M",
            "DROP UNDO TABLESPACE ut WAIT",
            // ALTER TABLESPACE: FILE_BLOCK_SIZE not in the alter set; RENAME takes no options;
            // the bare form needs at least one option.
            "ALTER TABLESPACE ts FILE_BLOCK_SIZE = 8192",
            "ALTER TABLESPACE ts RENAME TO ts2 ENGINE = InnoDB",
            "ALTER TABLESPACE ts",
            // LOGFILE GROUP: FILE_BLOCK_SIZE not in the create set; ADD UNDOFILE mandatory;
            // COMMENT not in the alter set.
            "CREATE LOGFILE GROUP lg ADD UNDOFILE 'undo.dat' FILE_BLOCK_SIZE = 8192",
            "CREATE LOGFILE GROUP lg",
            "ALTER LOGFILE GROUP lg ADD UNDOFILE 'undo2.dat' COMMENT = 'x'",
            "ALTER LOGFILE GROUP lg",
            // Size-literal boundaries: adjacency required, single K/M/G suffix only.
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 16 M",
            "CREATE TABLESPACE ts ADD DATAFILE 'ts.ibd' INITIAL_SIZE = 16MB",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err(&format!("{sql:?} must reject"));
        }
    }
    /// Every grammar-valid form of the MySQL spatial-reference-system and resource-group DDL
    /// families round-trips byte-identically — the SRS `OR REPLACE`/`IF NOT EXISTS` branches,
    /// the order-free (and repeatable) attribute list, the hex srid, and the resource-group
    /// `TYPE`/`VCPU`/`THREAD_PRIORITY` `[=]` spellings, range list, negative priority, state
    /// keyword, and the `ALTER`/`DROP` `FORCE` tail. The same forms are live-oracle-verified
    /// grammar-valid in `corpus_mysql_verdicts::mysql_srs_resource_group_live_oracle_parity`.
    #[test]
    fn srs_resource_group_ddl_round_trips() {
        use crate::ast::{ResourceGroupState, ResourceGroupType, SrsAttribute};

        // Structural spot-checks: the attribute list keeps source order.
        let srs = parse_with(
            "CREATE SPATIAL REFERENCE SYSTEM 990001 DESCRIPTION 'd' ORGANIZATION 'o' \
             IDENTIFIED BY 5 NAME 'z' DEFINITION 'w'",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .unwrap();
        let Statement::CreateSpatialReferenceSystem { create, .. } = &srs.statements()[0] else {
            panic!("expected a CREATE SPATIAL REFERENCE SYSTEM statement");
        };
        assert!(!create.or_replace);
        assert!(!create.if_not_exists);
        assert_eq!(create.attributes.len(), 4);
        assert!(matches!(
            create.attributes[0],
            SrsAttribute::Description { .. }
        ));
        assert!(matches!(
            create.attributes[1],
            SrsAttribute::Organization { .. }
        ));
        assert!(matches!(create.attributes[2], SrsAttribute::Name { .. }));
        assert!(matches!(
            create.attributes[3],
            SrsAttribute::Definition { .. }
        ));

        let rg = parse_with(
            "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0-2, 4 THREAD_PRIORITY = -5 DISABLE",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .unwrap();
        let Statement::CreateResourceGroup { create, .. } = &rg.statements()[0] else {
            panic!("expected a CREATE RESOURCE GROUP statement");
        };
        assert!(create.type_equals);
        assert_eq!(create.group_type, ResourceGroupType::User);
        let vcpu = create.vcpu.as_ref().expect("VCPU clause");
        assert!(vcpu.equals);
        assert_eq!(vcpu.ranges.len(), 2);
        assert!(vcpu.ranges[0].end.is_some());
        assert!(vcpu.ranges[1].end.is_none());
        let priority = create.thread_priority.as_ref().expect("THREAD_PRIORITY");
        assert!(priority.equals && priority.negative);
        assert_eq!(create.state, Some(ResourceGroupState::Disable));

        // `ALTER … FORCE` with no state keyword: `force` is a peer of `state`, not its suffix.
        let force = parse_with(
            "ALTER RESOURCE GROUP g FORCE",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .unwrap();
        let Statement::AlterResourceGroup { alter, .. } = &force.statements()[0] else {
            panic!("expected an ALTER RESOURCE GROUP statement");
        };
        assert!(alter.state.is_none() && alter.force);

        for sql in [
            // CREATE / DROP SPATIAL REFERENCE SYSTEM.
            "CREATE SPATIAL REFERENCE SYSTEM 990001 NAME 'zzp_srs' DEFINITION 'LOCAL_CS[\"z\"]'",
            "CREATE SPATIAL REFERENCE SYSTEM 990001 DEFINITION 'w' NAME 'z'",
            "CREATE SPATIAL REFERENCE SYSTEM 990001 DESCRIPTION 'd' ORGANIZATION 'o' \
             IDENTIFIED BY 5 NAME 'z' DEFINITION 'w'",
            // A repeated attribute and a bare (attribute-less) form are grammar-valid — the
            // engine rejects post-parse (ER_SRS_MULTIPLE_ATTRIBUTE_DEFINITIONS 3709 /
            // ER_SRS_MISSING_MANDATORY_ATTRIBUTE 3708, both non-1064 on mysql:8.4.10).
            "CREATE SPATIAL REFERENCE SYSTEM 990001 NAME 'a' NAME 'b'",
            "CREATE SPATIAL REFERENCE SYSTEM 990001",
            "CREATE OR REPLACE SPATIAL REFERENCE SYSTEM 990001 NAME 'z' DEFINITION 'w'",
            "CREATE SPATIAL REFERENCE SYSTEM IF NOT EXISTS 990001 NAME 'z' DEFINITION 'w'",
            "CREATE SPATIAL REFERENCE SYSTEM 0x10 NAME 'z' DEFINITION 'w'",
            "CREATE SPATIAL REFERENCE SYSTEM 990001 ORGANIZATION 'o' IDENTIFIED BY 0x10",
            "DROP SPATIAL REFERENCE SYSTEM 990001",
            "DROP SPATIAL REFERENCE SYSTEM IF EXISTS 990001",
            // CREATE RESOURCE GROUP.
            "CREATE RESOURCE GROUP zzp_rg TYPE = USER",
            "CREATE RESOURCE GROUP g TYPE USER",
            "CREATE RESOURCE GROUP g TYPE = SYSTEM",
            "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0-3",
            "CREATE RESOURCE GROUP g TYPE = USER VCPU 0-3",
            "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0, 1, 2",
            "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0-2, 4, 6-8",
            "CREATE RESOURCE GROUP g TYPE = USER VCPU = 5",
            "CREATE RESOURCE GROUP g TYPE = USER THREAD_PRIORITY = 5",
            "CREATE RESOURCE GROUP g TYPE = USER THREAD_PRIORITY = -5",
            "CREATE RESOURCE GROUP g TYPE = USER THREAD_PRIORITY 5",
            "CREATE RESOURCE GROUP g TYPE = USER ENABLE",
            "CREATE RESOURCE GROUP g TYPE = USER DISABLE",
            "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0-3 THREAD_PRIORITY = 5 ENABLE",
            // ALTER RESOURCE GROUP — every clause optional, FORCE independent of the state.
            "ALTER RESOURCE GROUP g",
            "ALTER RESOURCE GROUP zzp_rg VCPU = 0",
            "ALTER RESOURCE GROUP g VCPU = 0-3",
            "ALTER RESOURCE GROUP g THREAD_PRIORITY = 5",
            "ALTER RESOURCE GROUP g ENABLE",
            "ALTER RESOURCE GROUP g DISABLE FORCE",
            "ALTER RESOURCE GROUP g ENABLE FORCE",
            "ALTER RESOURCE GROUP g FORCE",
            "ALTER RESOURCE GROUP g VCPU 1-2 THREAD_PRIORITY 3 DISABLE",
            "ALTER RESOURCE GROUP g VCPU = 0-3 THREAD_PRIORITY = 5 ENABLE FORCE",
            // DROP RESOURCE GROUP.
            "DROP RESOURCE GROUP zzp_rg",
            "DROP RESOURCE GROUP g FORCE",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MYSQL_RENDER))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }

        // The `opt_comma` VCPU separator: whitespace-separated ranges parse (grammar-accepted
        // on mysql:8.4.10) and normalize to the canonical comma list on render.
        let bare = parse_with(
            "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0 1 2",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .unwrap();
        let rendered = Renderer::new(MYSQL_RENDER).render_parsed(&bare).unwrap();
        assert_eq!(
            rendered,
            "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0, 1, 2"
        );
    }

    /// The measured reject boundaries — every one is `ER_PARSE_ERROR` 1064 on mysql:8.4.10
    /// (two-sided-verified in the live-oracle evidence test). SRS: `OR REPLACE` + `IF NOT
    /// EXISTS` together, a signed or fractional srid, `ORGANIZATION` without `IDENTIFIED BY`
    /// or with a string id, a comma-list `DROP`. Resource group: a missing/unknown `TYPE`,
    /// an out-of-order option train, `FORCE` on `CREATE`, and a hex `VCPU`/`THREAD_PRIORITY`
    /// value (those slots are `NUM`-typed, unlike the `real_ulonglong_num` srid).
    #[test]
    fn srs_resource_group_ddl_reject_boundaries() {
        for sql in [
            "CREATE OR REPLACE SPATIAL REFERENCE SYSTEM IF NOT EXISTS 990001 NAME 'z'",
            "CREATE SPATIAL REFERENCE SYSTEM -1 NAME 'z' DEFINITION 'w'",
            "CREATE SPATIAL REFERENCE SYSTEM 1.5 NAME 'z' DEFINITION 'w'",
            "CREATE SPATIAL REFERENCE SYSTEM 990001 ORGANIZATION 'o'",
            "CREATE SPATIAL REFERENCE SYSTEM 990001 ORGANIZATION 'o' IDENTIFIED BY 'x'",
            "DROP SPATIAL REFERENCE SYSTEM 990001, 990002",
            "CREATE RESOURCE GROUP g",
            "CREATE RESOURCE GROUP g TYPE = FOO",
            "CREATE RESOURCE GROUP g TYPE = USER ENABLE VCPU = 0-3",
            "CREATE RESOURCE GROUP g TYPE = USER ENABLE FORCE",
            "CREATE RESOURCE GROUP g TYPE = USER VCPU = 0x1",
            "CREATE RESOURCE GROUP g TYPE = USER THREAD_PRIORITY = 0x1",
            "DROP RESOURCE GROUP g, h",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err(&format!("MySQL rejects {sql:?}"));
        }
    }

    /// Both families are gated: off in ANSI/SQLite/PostgreSQL (no MySQL SRS or resource-group
    /// object), on in MySQL and the Lenient superset.
    #[test]
    fn srs_resource_group_ddl_dialect_gating() {
        for sql in [
            "CREATE SPATIAL REFERENCE SYSTEM 990001 NAME 'z' DEFINITION 'w'",
            "DROP SPATIAL REFERENCE SYSTEM 990001",
            "CREATE RESOURCE GROUP g TYPE = USER",
            "ALTER RESOURCE GROUP g ENABLE",
            "DROP RESOURCE GROUP g",
        ] {
            parse_with(sql, crate::ParseConfig::new(MySql))
                .unwrap_or_else(|err| panic!("MySQL accepts {sql:?}: {err:?}"));
            parse_with(sql, crate::ParseConfig::new(LENIENT_RENDER))
                .unwrap_or_else(|err| panic!("Lenient accepts {sql:?}: {err:?}"));
            parse_with(sql, crate::ParseConfig::new(Ansi))
                .expect_err(&format!("ANSI rejects {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .expect_err(&format!("SQLite rejects {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(crate::dialect::Postgres))
                .expect_err(&format!("PostgreSQL rejects {sql:?}"));
        }
    }
}
