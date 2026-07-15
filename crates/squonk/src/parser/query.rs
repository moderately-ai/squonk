// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Statement dispatch and the query-level grammar.
//!
//! This is the query grammar family: the statement dispatcher plus the
//! query-level clauses that wrap a SELECT body — set operations, `ORDER BY`, and
//! `LIMIT`/`OFFSET`. The SELECT body itself (projection, `FROM`, `WHERE`,
//! `GROUP BY`, `HAVING`) lives in [`super::select`], and the `FROM` relation
//! grammar (table factors, joins, qualified names) in [`super::from`]; all three
//! are `impl<D> Parser<D>` blocks over the same engine, so a clause helper in any
//! of them is reachable from the others.
//!
//! Keyword recognition is tokenized up front, so the shared lexical predicates
//! that the whole family leans on — keyword / punctuation / operator peeks,
//! identifier admissibility, and their `eat`/`expect` companions — are gathered
//! here as the one place every clause helper draws from.

use crate::ast::{
    AliasSpelling, ColumnsSpelling, Cte, CteBody, CteCycleClause, CteCycleMark, CteSearchClause,
    DefaultValue, Expr, FetchSpelling, ForClause, ForJsonMode, ForRoot, ForXmlElements, ForXmlMode,
    FormatClause, HandlerIndexDirection, HandlerKeyComparison, HandlerOperation,
    HandlerReadSelector, HandlerScanDirection, HandlerStatement, Keyword, KeywordSet, Limit,
    LimitBy, LimitPercent, LimitSyntax, Literal, LiteralKind, LockStrength, LockWait,
    LockingClause, LockingSpelling, ObjectName, OrderByAll, OrderByExpr, OrderByUsing,
    PipeAggregateExpr, PipeOperator, PipeRenameItem, Query, SetExpr, SetOperator, SetQuantifier,
    Setting, Span, Spanned, Statement, Symbol, TableAlias, UpdateAssignment, UpdateValue, Values,
    ValuesItem, With,
};
use crate::error::ParseResult;
use crate::tokenizer::{Operator, Punctuation, Token, TokenKind};
use thin_vec::{ThinVec, thin_vec};

use super::clause_marks::ClauseKw;
use super::engine::Parser;
use super::{Dialect, HookResult};

/// The parsed query-tail `ORDER BY` clause: the ordinary sort-key list, or DuckDB's
/// whole-clause `ALL` mode — exactly one of the pair is populated (the grammar
/// branches before either is parsed).
type OrderByClause<X> = (ThinVec<OrderByExpr<X>>, Option<Box<OrderByAll>>);

/// The parsed body of a SQL:2008 `FETCH { FIRST | NEXT } [<count>] { ROW | ROWS }
/// { ONLY | WITH TIES }` tail: the optional row count, whether `WITH TIES` was
/// written, and the folded `FIRST`/`NEXT` × `ROW`/`ROWS` surface spelling.
type FetchFirstClause<X> = (Option<Expr<X>>, bool, FetchSpelling);

impl<'a, D: Dialect> Parser<'a, D> {
    /// Parse the next top-level statement, or `Ok(None)` at end of input.
    ///
    /// Statements are separated and terminated by `;`; empty statements (stray or
    /// trailing semicolons) are skipped. A parsed statement must be followed by a `;`
    /// separator or end of input: two statements abutting without a `;` between them
    /// (`DO '' DO ''`, `VALUES (1) VALUES (2)`) are a syntax error, matching every SQL
    /// engine — a top-level statement list is `;`-delimited, so leftover tokens that do
    /// not begin with a separator are rejected here rather than being silently parsed as
    /// a second, separator-less statement. (Statements whose grammar happens to choke on
    /// the following token — `SELECT 1 SELECT 2`, where the reserved `SELECT` cannot be a
    /// projection alias — were already rejected incidentally; this makes the rule
    /// uniform across the statement kinds that *can* cleanly stop mid-stream.)
    ///
    /// The tokens consumed by the previous statement are released first (streaming:
    /// only the in-flight statement's tokens stay buffered), so repeated
    /// calls walk a multi-statement script in bounded memory. This single step backs
    /// both [`parse_with`](super::parse_with) and the public
    /// [`statements`](super::statements) iterator.
    pub(crate) fn parse_next_statement(&mut self) -> ParseResult<Option<Statement<D::Ext>>> {
        self.skip_statement_separators()?;
        self.discard_consumed_tokens();
        if self.is_eof()? {
            return Ok(None);
        }
        let statement = self.parse_statement()?;
        // The top-level statement list is `;`-delimited: what follows a statement must
        // be a separator or end of input. Anything else is trailing input — including a
        // token sequence that would itself parse as a statement — so reject it rather
        // than splitting a separator-less run into multiple statements.
        if !self.is_eof()? && !self.peek_is_punct(Punctuation::Semicolon)? {
            return Err(self.unexpected("`;` or end of input after a statement"));
        }
        Ok(Some(statement))
    }

    /// Consume any run of `;` statement separators.
    fn skip_statement_separators(&mut self) -> ParseResult<()> {
        while self.peek_is_punct(Punctuation::Semicolon)? {
            self.advance()?;
        }
        Ok(())
    }

    /// Dispatch a single statement on its leading token.
    ///
    /// The family dispatcher: it grows a new arm per statement family as the
    /// grammar tickets land. `pub(super)` so the `EXPLAIN` grammar in
    /// [`super::util`] can recurse into it for its inner statement.
    pub(super) fn parse_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        // Recursion-guarded (ADR-0012): a top-level statement enters at depth zero,
        // but `EXPLAIN <stmt>` and `COPY (<stmt>) TO` recurse back into a nested
        // statement that neither `parse_query` nor `parse_expr_bp` would otherwise
        // bound, so guarding the statement dispatcher caps that nesting too.
        let span = self.current_span()?;
        let mut guard = self.enter_recursion(span)?;
        guard.parser().parse_statement_inner()
    }

    /// Dispatch one statement, one level deep under the recursion guard.
    fn parse_statement_inner(&mut self) -> ParseResult<Statement<D::Ext>> {
        match D::parse_statement_hook(self) {
            HookResult::Handled(statement) => return Ok(statement),
            HookResult::NotHandled => {}
            HookResult::Err(error) => return Err(error),
        }

        if self.peek_is_keyword(Keyword::Create)? {
            self.parse_create_statement()
        } else if self.peek_is_contextual_keyword("ALTER")? {
            self.parse_alter_statement()
        } else if self.peek_is_contextual_keyword("DROP")? {
            self.parse_drop_statement()
        } else if self.peek_is_contextual_keyword("TRUNCATE")? {
            // `TRUNCATE` is standard SQL:2008 (F200) and accepted by every shipped
            // dialect (PostgreSQL, ANSI, Lenient, and MySQL all have `TRUNCATE TABLE`),
            // so unlike `COPY`/`COMMENT ON` it carries no FeatureSet gate.
            self.parse_truncate_statement()
        } else if self.features().statement_ddl_gates.materialized_views
            && self.peek_is_contextual_keyword("REFRESH")?
        {
            self.parse_refresh_materialized_view_statement()
        } else if self.peek_is_keyword(Keyword::Insert)? {
            let start = self.current_span()?;
            self.parse_insert_statement_with(start, None)
        } else if self.peek_is_contextual_keyword("UPDATE")? {
            // Refinement of the leading `UPDATE`, mirroring the typed-`SHOW` seams: a called
            // peek helper claims only `UPDATE EXTENSIONS [(names)]` (DuckDB extension refresh)
            // and returns for everything else, so the hot DML `UPDATE` path is one keyword
            // comparison heavier and otherwise untouched. Off in every non-DuckDB preset.
            if self.features().utility_syntax.update_extensions
                && self.peek_starts_update_extensions()?
            {
                self.parse_update_extensions_statement()
            } else {
                let start = self.current_span()?;
                self.parse_update_statement_with(start, None)
            }
        } else if self.peek_is_contextual_keyword("DELETE")? {
            let start = self.current_span()?;
            self.parse_delete_statement_with(start, None)
        } else if self.features().mutation_syntax.merge && self.peek_is_keyword(Keyword::Merge)? {
            // `merge` gates a *leading* keyword: when off (MySQL), the arm is skipped
            // and `MERGE` falls through to the unknown-statement error — the reject
            // path, mirroring how a disabled trailing clause is left unconsumed.
            let start = self.current_span()?;
            self.parse_merge_statement(start, None)
        } else if self.features().mutation_syntax.replace_into
            && self.peek_is_keyword(Keyword::Replace)?
        {
            // `replace_into` gates the leading `REPLACE` keyword the same way `merge`
            // gates `MERGE`: off in ANSI/PostgreSQL, so `REPLACE` is never dispatched
            // there and surfaces as an unknown statement.
            let start = self.current_span()?;
            self.parse_replace_statement(start)
        } else if self.peek_is_keyword(Keyword::With)? {
            self.parse_statement_starting_with_with()
        } else if self.features().utility_syntax.replication_statements
            && self.peek_starts_replication_statement()?
        {
            // `replication_statements` refines the shared `CHANGE`/`START`/`STOP` leading
            // keywords: the two-word lookahead claims `CHANGE REPLICATION …`, `START`/`STOP
            // REPLICA`, and `START`/`STOP GROUP_REPLICATION` only, so `START TRANSACTION` (and
            // every other use of those words) still falls through to the transaction dispatch
            // below. Checked first so the transaction dispatcher — which claims any leading
            // `START` — never swallows `START REPLICA`. Off outside MySQL (and Lenient), where
            // the sequences are not dispatched and surface as unknown statements.
            self.parse_replication_statement()
        } else if self.peek_starts_transaction_statement()? {
            // Checked before sessions: `SET TRANSACTION` is transaction control,
            // while every other `SET` is a session statement.
            self.parse_transaction_statement()
        } else if self.features().utility_syntax.xa_transactions
            && self.peek_is_contextual_keyword("XA")?
        {
            // `xa_transactions` gates MySQL's `XA` distributed-transaction family on its own
            // leading keyword (like `kill`): off outside MySQL (and Lenient), where `XA` is
            // not dispatched and surfaces as an unknown statement.
            self.parse_xa_statement()
        } else if self.features().show_syntax.show_tables
            && self.peek_is_contextual_keyword("SHOW")?
            && self.peek_starts_show_tables()?
        {
            // `show_tables` refines the generic-`SHOW` dispatch: a top-level `SHOW` whose
            // next word (past the optional `EXTENDED`/`FULL`/`ALL` modifiers) is `TABLES`
            // is the typed catalogue listing (MySQL/DuckDB), claimed here before the
            // session branch below. Every other `SHOW <var>` — and `SHOW ALL`/`SHOW FULL`
            // with no `TABLES` — still falls through to the session statement, so the two
            // seams are MECE. Off in ANSI/PostgreSQL/SQLite, where a top-level `SHOW`
            // reaches only the session branch (or nothing, in SQLite).
            self.parse_show_statement()
        } else if self.features().show_syntax.show_columns
            && self.peek_is_contextual_keyword("SHOW")?
            && self.peek_starts_show_columns()?
        {
            // `show_columns` refines the generic-`SHOW` dispatch the same way `show_tables`
            // does: a top-level `SHOW` whose next word (past the optional `EXTENDED`/`FULL`
            // modifiers) is `COLUMNS` or its `FIELDS` synonym is the typed column listing
            // (MySQL-only — DuckDB has no such grammar), claimed here before the session
            // branch. Every other `SHOW <var>` still falls through, so the seams stay MECE.
            self.parse_show_columns_statement()
        } else if self.features().show_syntax.show_create_table
            && self.peek_is_contextual_keyword("SHOW")?
            && self.peek_starts_show_create_table()?
        {
            // `show_create_table` refines the generic-`SHOW` dispatch the same way
            // `show_tables`/`show_columns` do: a top-level `SHOW` whose next two words are
            // `CREATE TABLE` is the typed DDL-recall statement (MySQL-only), claimed here
            // before the session branch. The lookahead requires *both* keywords, so a bare
            // `SHOW create` (PostgreSQL reading `create` as a session variable) and every
            // other `SHOW <var>` still fall through, keeping the seams MECE.
            self.parse_show_create_table_statement()
        } else if self.features().show_syntax.show_functions
            && self.peek_is_contextual_keyword("SHOW")?
            && self.peek_starts_show_functions()?
        {
            // `show_functions` refines the generic-`SHOW` dispatch the same way the sibling
            // `show_*` gates do: a top-level `SHOW` whose next word (past the optional
            // `USER`/`SYSTEM`/`ALL` scope) is `FUNCTIONS` is the typed function listing
            // (Spark/Databricks), claimed here before the session branch. A bare `SHOW <var>`
            // — including `SHOW ALL` with no `FUNCTIONS` — still falls through, keeping the
            // seams MECE.
            self.parse_show_functions_statement()
        } else if self.features().show_syntax.show_routine_status
            && self.peek_is_contextual_keyword("SHOW")?
            && self.peek_starts_show_routine_status()?
        {
            // `show_routine_status` refines the generic-`SHOW` dispatch the same way the
            // sibling `show_*` gates do: a top-level `SHOW` whose next two words are
            // `FUNCTION STATUS` or `PROCEDURE STATUS` is the typed MySQL stored-routine
            // listing, claimed here before the session branch. The lookahead requires both
            // the object keyword and the trailing `STATUS`, so the seam steals only that full
            // two-keyword prefix; `FUNCTION`/`PROCEDURE` are reserved, so a bare `SHOW
            // FUNCTION` cannot be a generic session `SHOW <var>` (like `SHOW CREATE`) and
            // every other `SHOW <var>` still falls through, keeping the seams MECE. Distinct
            // from `show_functions`: singular `FUNCTION`/`PROCEDURE`, not the plural
            // `FUNCTIONS`.
            self.parse_show_routine_status_statement()
        } else if self.features().show_syntax.show_admin
            && self.peek_is_contextual_keyword("SHOW")?
            && self.peek_starts_show_admin()?
        {
            // `show_admin` refines the generic-`SHOW` dispatch like the sibling `show_*`
            // gates, but claims the whole MySQL server-administration / catalogue family
            // (`SHOW DATABASES`, `SHOW STATUS`, `SHOW ENGINES`, `SHOW CREATE VIEW`, `SHOW
            // INDEX`, …) through one table-driven parse rather than one arm per keyword —
            // the sub-command is DATA on the `ShowTarget` axis. Checked after the
            // individually-gated `TABLES`/`COLUMNS`/`CREATE TABLE`/`{FUNCTION|PROCEDURE}
            // STATUS` seams, so it never steals their forms; every `SHOW <var>` outside the
            // family still falls through to the session branch, keeping the seams MECE. Off
            // in every non-MySQL preset (bar the Lenient superset).
            self.parse_show_admin_statement()
        } else if self.features().show_syntax.session_statements
            && self.peek_starts_session_statement()?
        {
            // `session_statements` gates the leading `SET`/`RESET`/`SHOW` like `copy`
            // gates `COPY`: off in SQLite, so the keyword is not dispatched there and
            // falls through to the unknown-statement error. (`SET TRANSACTION` was already
            // claimed by the transaction dispatch above, so it is unaffected.)
            self.parse_session_statement()
        } else if self.features().access_control_syntax.access_control
            && self.peek_starts_access_control_statement()?
        {
            // `access_control` gates the leading `GRANT`/`REVOKE`: off in SQLite (no
            // permission system), where they fall through to the unknown-statement error.
            self.parse_access_control_statement()
        } else if (self.features().utility_syntax.copy || self.features().utility_syntax.copy_into)
            && self.peek_is_contextual_keyword("COPY")?
        {
            // `copy`/`copy_into` gate a *leading* statement keyword, like
            // `merge`/`replace_into`: both off in ANSI/MySQL, so `COPY` is never dispatched
            // there and falls through to the unknown-statement error. The two share the
            // leading keyword; the helper branches on the `INTO` that distinguishes
            // Snowflake `COPY INTO` from the PostgreSQL `COPY <table> {FROM | TO}`.
            self.parse_copy_or_copy_into_statement()
        } else if self.features().utility_syntax.comment_on
            && self.peek_is_contextual_keyword("COMMENT")?
        {
            // `comment_on` gates the leading `COMMENT` keyword like `copy`: off in
            // ANSI/MySQL, so `COMMENT ON ...` is never dispatched there and falls through
            // to the unknown-statement error — the reject path for the PostgreSQL-only
            // object-metadata statement.
            self.parse_comment_on_statement()
        } else if self.features().utility_syntax.pragma
            && self.peek_is_contextual_keyword("PRAGMA")?
        {
            // `pragma` gates the leading `PRAGMA` keyword like `copy`: off everywhere
            // but SQLite (and Lenient), so elsewhere it falls through to the
            // unknown-statement error — the reject path for the SQLite-only
            // configuration statement.
            self.parse_pragma_statement()
        } else if self.features().utility_syntax.attach
            && self.peek_is_contextual_keyword("ATTACH")?
        {
            // `attach` gates `ATTACH` and its `DETACH` inverse as one dialect unit
            // (a dialect with one has both); off everywhere but SQLite (and Lenient).
            self.parse_attach_statement()
        } else if self.features().utility_syntax.attach
            && self.peek_is_contextual_keyword("DETACH")?
        {
            self.parse_detach_statement()
        } else if self.features().utility_syntax.export_import_database
            && self.peek_is_contextual_keyword("EXPORT")?
        {
            // `export_import_database` gates the DuckDB `EXPORT DATABASE` catalogue dump and
            // its `IMPORT DATABASE` inverse as one dialect unit (a dialect with one has the
            // other); off everywhere but DuckDB (and Lenient), where both leading keywords
            // fall through to the unknown-statement error.
            self.parse_export_statement()
        } else if self.features().utility_syntax.import_table
            && self.peek_is_contextual_keyword("IMPORT")?
            && self.peek_nth_is_contextual_keyword(1, "TABLE")?
        {
            // `import_table` gates the MySQL `IMPORT TABLE FROM …` statement. The leading
            // `IMPORT` collides with DuckDB's `IMPORT DATABASE`, so this arm — checked before
            // the `export_import_database` arm below — distinguishes on the second keyword
            // (`TABLE` vs `DATABASE`); the two gates are independent, so the permissive superset
            // has both on without ambiguity.
            self.parse_import_table_statement()
        } else if self.features().utility_syntax.export_import_database
            && self.peek_is_contextual_keyword("IMPORT")?
        {
            self.parse_import_statement()
        } else if self.features().maintenance_syntax.checkpoint
            && (self.peek_is_contextual_keyword("CHECKPOINT")?
                || (self.features().maintenance_syntax.checkpoint_database
                    && self.peek_is_contextual_keyword("FORCE")?
                    && self.peek_nth_is_contextual_keyword(1, "CHECKPOINT")?))
        {
            // `checkpoint` gates the leading `CHECKPOINT` (PostgreSQL/DuckDB); the DuckDB
            // `FORCE CHECKPOINT` form only dispatches under `checkpoint_database`, so a
            // leading `FORCE` surfaces as an unknown statement where that gate is off.
            self.parse_checkpoint_statement()
        } else if self.features().utility_syntax.load_data
            && self.peek_is_contextual_keyword("LOAD")?
            && (self.peek_nth_is_contextual_keyword(1, "DATA")?
                || self.peek_nth_is_contextual_keyword(1, "XML")?)
        {
            // `load_data` gates MySQL's `LOAD {DATA | XML}` bulk-import reading of the leading
            // `LOAD` keyword — a DIFFERENT grammar from the PostgreSQL/DuckDB `load_extension`
            // shared-library load below. The two-word `LOAD` + `DATA`/`XML` lookahead keeps the
            // seams MECE where both gates are on (Lenient): `LOAD DATA`/`LOAD XML` route here,
            // every other `LOAD <arg>` (and MySQL's own `LOAD INDEX INTO CACHE`) falls through.
            // Placed before `load_extension` because that arm matches a bare leading `LOAD` with
            // no second-word check, so it would otherwise swallow `LOAD DATA` under Lenient.
            self.parse_load_data_statement()
        } else if self.features().utility_syntax.key_cache_statements
            && self.peek_is_keyword(Keyword::Cache)?
        {
            // `key_cache_statements` gates the leading `CACHE` keyword (MySQL's `CACHE INDEX`,
            // the only leading-`CACHE` statement) like `kill`: off outside MySQL (and the
            // permissive superset), where it falls through to the unknown-statement error.
            self.parse_cache_index_statement()
        } else if self.features().utility_syntax.key_cache_statements
            && self.peek_is_keyword(Keyword::Load)?
            && self.peek_nth_is_keyword(1, Keyword::Index)?
        {
            // MySQL's `LOAD INDEX INTO CACHE` shares its `key_cache_statements` gate with `CACHE
            // INDEX`. The `LOAD INDEX` two-token lookahead separates it from the DuckDB/PostgreSQL
            // `LOAD <extension>` statement below (which never leads with `INDEX`), so the two LOAD
            // grammars stay MECE even where both gates are on (the permissive superset).
            self.parse_load_index_statement()
        } else if self.features().utility_syntax.load_extension
            && self.peek_is_contextual_keyword("LOAD")?
        {
            // `load_extension` gates the leading `LOAD` (PostgreSQL/DuckDB); off elsewhere it
            // falls through to the unknown-statement error.
            self.parse_load_statement()
        } else if (self.features().maintenance_syntax.vacuum
            || self.features().maintenance_syntax.vacuum_analyze)
            && self.peek_is_contextual_keyword("VACUUM")?
        {
            // `vacuum`/`reindex`/`analyze` gate their leading keywords like `pragma`:
            // each is an independent SQLite maintenance statement (they are not an
            // inverse pair, so they take separate flags rather than sharing one like
            // `attach`/`detach`). The leading `VACUUM` dispatches under either the SQLite
            // `vacuum` gate (`[<schema>] INTO <expr>`) or the DuckDB `vacuum_analyze` gate
            // (`[ANALYZE] <table> (<cols>)`); the parser reads whichever tail its gate
            // admits (off everywhere but SQLite/DuckDB/Lenient).
            self.parse_vacuum_statement()
        } else if self.features().maintenance_syntax.reindex
            && self.peek_is_contextual_keyword("REINDEX")?
        {
            self.parse_reindex_statement()
        } else if self.features().maintenance_syntax.table_maintenance
            && self.peek_starts_table_maintenance()?
        {
            // The MySQL admin-table verb family (`ANALYZE/CHECK/CHECKSUM/OPTIMIZE/REPAIR
            // TABLE`), reached through one table-driven parse — the verb is DATA on the
            // `TableMaintenanceKind` axis. Placed *before* the bare-`ANALYZE` maintenance
            // arm below: MySQL's `ANALYZE` always takes `{TABLE | TABLES}` (the lookahead
            // insists on it), so a bare `ANALYZE` under a preset with both gates on (Lenient)
            // still falls through to the SQLite/DuckDB sibling, keeping the seams MECE.
            self.parse_table_maintenance_statement()
        } else if self.features().maintenance_syntax.analyze
            && self.peek_is_contextual_keyword("ANALYZE")?
        {
            // A *leading* `ANALYZE` is the maintenance statement; the `ANALYZE` inside
            // `EXPLAIN ANALYSE` is consumed by the `EXPLAIN` grammar below, never here.
            self.parse_analyze_statement()
        } else if self.features().utility_syntax.rename_statement
            && self.peek_is_contextual_keyword("RENAME")?
        {
            // `rename_statement` gates the leading `RENAME` keyword like `kill`: off outside
            // MySQL (and Lenient), where it falls through to the unknown-statement error. A
            // non-leading `RENAME` — the `ALTER TABLE … RENAME TO` sub-clause — is consumed
            // by the `ALTER TABLE` grammar and never reaches this statement-leading position.
            self.parse_rename_statement()
        } else if self.features().utility_syntax.flush
            && self.peek_is_contextual_keyword("FLUSH")?
        {
            // `flush` gates the leading `FLUSH` keyword like `kill`: off outside MySQL (and
            // Lenient), where it falls through to the unknown-statement error.
            self.parse_flush_statement()
        } else if self.features().utility_syntax.purge_binary_logs
            && self.peek_is_contextual_keyword("PURGE")?
        {
            // `purge_binary_logs` gates the leading `PURGE` keyword like `kill`: off outside
            // MySQL (and Lenient), where it falls through to the unknown-statement error.
            self.parse_purge_statement()
        } else if self.features().utility_syntax.use_statement
            && self.peek_is_keyword(Keyword::Use)?
        {
            // `use_statement` gates the leading `USE` keyword like `pragma`: on for DuckDB and
            // MySQL (and Lenient), off elsewhere, where it falls through to the
            // unknown-statement error. A *non-leading* `USE` is the MySQL index-hint keyword,
            // consumed by the FROM grammar, so it never reaches this statement-leading
            // position. The accepted name arity is dialect data (`use_qualified_name`).
            self.parse_use_statement()
        } else if self.features().utility_syntax.kill && self.peek_is_contextual_keyword("KILL")? {
            // `kill` gates the leading `KILL` keyword like `copy`: off outside MySQL, so
            // there it is not dispatched and falls through to the unknown-statement error.
            self.parse_kill_statement()
        } else if self.features().utility_syntax.handler_statements
            && self.peek_is_keyword(Keyword::Handler)?
        {
            // `handler_statements` gates the leading `HANDLER` keyword like `kill`: off outside
            // MySQL (and the permissive superset), where it falls through to the
            // unknown-statement error. A *non-leading* `HANDLER` is the stored-program
            // `DECLARE … HANDLER FOR` keyword, consumed by the compound-body grammar, so it
            // never reaches this statement-leading position.
            self.parse_handler_statement()
        } else if self.features().utility_syntax.plugin_component_statements
            && self.peek_is_contextual_keyword("INSTALL")?
        {
            // `plugin_component_statements` gates the leading `INSTALL`/`UNINSTALL` keywords like
            // `kill`: off outside MySQL (and the permissive superset), where they fall through to
            // the unknown-statement error. `INSTALL PLUGIN … SONAME …` / `INSTALL COMPONENT …`.
            self.parse_install_statement()
        } else if self.features().utility_syntax.plugin_component_statements
            && self.peek_is_contextual_keyword("UNINSTALL")?
        {
            // The inverse of `INSTALL`, sharing the same gate (an install/uninstall pair, like
            // `ATTACH`/`DETACH`): `UNINSTALL PLUGIN <name>` / `UNINSTALL COMPONENT <urn> …`.
            self.parse_uninstall_statement()
        } else if self.features().utility_syntax.shutdown
            && self.peek_is_contextual_keyword("SHUTDOWN")?
        {
            // `shutdown` gates the nullary leading `SHUTDOWN` keyword like `kill`: off outside
            // MySQL (and the permissive superset), where it surfaces as an unknown statement.
            self.parse_shutdown_statement()
        } else if self.features().utility_syntax.restart
            && self.peek_is_contextual_keyword("RESTART")?
        {
            // `restart` gates the nullary leading `RESTART` keyword like `kill`.
            self.parse_restart_statement()
        } else if self.features().utility_syntax.clone
            && self.peek_is_contextual_keyword("CLONE")?
        {
            // `clone` gates the leading `CLONE` keyword like `kill`; both the `LOCAL` and
            // `INSTANCE` forms follow it.
            self.parse_clone_statement()
        } else if self.features().utility_syntax.help_statement
            && self.peek_is_contextual_keyword("HELP")?
        {
            // `help_statement` gates the leading `HELP` keyword like `kill`.
            self.parse_help_statement()
        } else if self.features().utility_syntax.binlog
            && self.peek_is_contextual_keyword("BINLOG")?
        {
            // `binlog` gates the leading `BINLOG` keyword like `kill`.
            self.parse_binlog_statement()
        } else if self.features().utility_syntax.prepared_statements
            && self.peek_is_contextual_keyword("PREPARE")?
        {
            // `prepared_statements` gates DuckDB's `PREPARE`/`EXECUTE`/`DEALLOCATE` leading
            // keywords like `copy`: off outside DuckDb (and Lenient), where each falls
            // through to the unknown-statement error.
            self.parse_prepare_statement()
        } else if self.features().utility_syntax.prepared_statements_from
            && self.peek_is_contextual_keyword("PREPARE")?
        {
            // `prepared_statements_from` gates MySQL's `PREPARE ... FROM` on the same leading
            // `PREPARE` keyword — a DIFFERENT grammar (statement source, not typed-`AS`) from
            // `prepared_statements` above. No shipped preset arms both (the `DO`-keyword split
            // precedent), and the both-on combination is registry-rejected as
            // `GrammarConflict::PreparedStatementsVersusPreparedStatementsFrom`: this arm resolves
            // the head DuckDB-first while the `DEALLOCATE` tail (below) resolves MySQL-first, so
            // the two winners are incoherent and the both-on semantics are left undefined.
            self.parse_prepare_from_statement()
        } else if self.features().utility_syntax.prepared_statements
            && self.peek_is_contextual_keyword("EXECUTE")?
        {
            // A *leading* `EXECUTE` is the prepared-statement invocation; the `EXECUTE`
            // privilege inside `GRANT EXECUTE` is claimed by the access-control dispatch
            // above and never reaches this leading position.
            self.parse_execute_statement()
        } else if self.features().utility_syntax.prepared_statements_from
            && self.peek_is_contextual_keyword("EXECUTE")?
        {
            // MySQL's `EXECUTE ... USING @var` — a DIFFERENT argument surface from DuckDB's
            // parenthesized `EXECUTE name(args)` above, on the same leading `EXECUTE` keyword.
            self.parse_execute_using_statement()
        } else if (self.features().utility_syntax.prepared_statements
            || self.features().utility_syntax.prepared_statements_from)
            && self.peek_is_contextual_keyword("DEALLOCATE")?
        {
            // Both prepared-statement dialects share the leading `DEALLOCATE` keyword;
            // `parse_deallocate_statement` branches on the active gate for the `PREPARE`
            // keyword's optional-vs-mandatory rule. MySQL's `DROP PREPARE` synonym is reached
            // from the DROP dispatcher instead (`parse_drop_prepare_statement`). When both gates
            // are on (registry-rejected as
            // `GrammarConflict::PreparedStatementsVersusPreparedStatementsFrom`) the tail resolves
            // MySQL-first — mandatory `PREPARE` — disagreeing with the DuckDB-first head dispatch
            // above; that incoherence is why the combination has no defined semantics.
            self.parse_deallocate_statement()
        } else if self.features().utility_syntax.call && self.peek_is_contextual_keyword("CALL")? {
            // `call` gates the leading `CALL` routine-invocation keyword like `copy`: off
            // outside DuckDb (and Lenient), where it falls through to the unknown-statement
            // error.
            self.parse_call_statement()
        } else if self.features().utility_syntax.do_statement
            && self.peek_is_contextual_keyword("DO")?
        {
            // `do_statement` gates the leading `DO` anonymous-code-block keyword like `copy`:
            // off outside PostgreSQL (and Lenient), where it falls through to the
            // unknown-statement error.
            self.parse_do_statement()
        } else if self.features().utility_syntax.do_expression_list
            && self.peek_is_contextual_keyword("DO")?
        {
            // `do_expression_list` gates MySQL's `DO <expr-list>` evaluate-and-discard
            // statement on the same leading `DO` keyword — a DIFFERENT behaviour from the
            // PostgreSQL `do_statement` code block above. The two gates are never both on in
            // one preset (each dialect arms at most one), so this dispatches unambiguously;
            // off outside MySQL, the leading `DO` falls through to the unknown-statement error.
            self.parse_do_expressions_statement()
        } else if self.features().utility_syntax.lock_tables
            && self.peek_is_contextual_keyword("LOCK")?
            && self.peek_nth_starts_table_or_tables(1)?
        {
            // `lock_tables` gates MySQL's per-table lock-kind reading of the leading `LOCK`
            // keyword — the `do_statement`/`do_expression_list`-style behaviour split: the
            // (unimplemented) PostgreSQL statement-level mode-list reading takes its own
            // future gate, a preset arming at most one, so the keyword dispatches
            // unambiguously. The two-word lookahead (`LOCK` + `TABLES`/`TABLE`) keeps this
            // seam and the `LOCK INSTANCE` seam below MECE on the shared first word.
            self.parse_lock_tables_statement()
        } else if self.features().utility_syntax.lock_instance
            && self.peek_is_contextual_keyword("LOCK")?
            && self.peek_nth_is_contextual_keyword(1, "INSTANCE")?
        {
            // `lock_instance` gates the `LOCK INSTANCE FOR BACKUP` half of MySQL's
            // instance-wide backup-lock pair; the `UNLOCK INSTANCE` release half rides the
            // same gate below.
            self.parse_lock_instance_statement()
        } else if self.features().utility_syntax.lock_tables
            && self.peek_is_contextual_keyword("UNLOCK")?
            && self.peek_nth_starts_table_or_tables(1)?
        {
            // The `UNLOCK {TABLES|TABLE}` release counterpart rides the same `lock_tables`
            // gate (MySQL's `lock`/`unlock` grammar rules travel together — the
            // `rename_statement` one-gate precedent).
            self.parse_unlock_tables_statement()
        } else if self.features().utility_syntax.lock_instance
            && self.peek_is_contextual_keyword("UNLOCK")?
            && self.peek_nth_is_contextual_keyword(1, "INSTANCE")?
        {
            self.parse_unlock_instance_statement()
        } else if self.features().utility_syntax.signal_diagnostics
            && self.peek_is_contextual_keyword("SIGNAL")?
        {
            // `signal_diagnostics` gates the MySQL diagnostics-area family's leading keywords
            // (`SIGNAL`/`RESIGNAL`/`GET DIAGNOSTICS`) like `kill`: off outside MySQL (and
            // Lenient), where each falls through to the unknown-statement error. These are
            // top-level `simple_statement` productions that also serve stored-program bodies,
            // which reach them through this same dispatcher's fall-through.
            self.parse_signal_statement()
        } else if self.features().utility_syntax.signal_diagnostics
            && self.peek_is_contextual_keyword("RESIGNAL")?
        {
            self.parse_resignal_statement()
        } else if self.features().utility_syntax.signal_diagnostics
            && self.peek_is_contextual_keyword("GET")?
            && self.peek_starts_get_diagnostics()?
        {
            // The two-word lookahead (`GET` + `DIAGNOSTICS`/`CURRENT`/`STACKED`) keeps the
            // leading `GET` from stealing an unrelated statement; a bare `GET <other>` still
            // falls through.
            self.parse_get_diagnostics_statement()
        } else if self.features().table_factor_syntax.pivot
            && self.peek_is_keyword(Keyword::Pivot)?
        {
            // `pivot`/`unpivot` gate their leading statement keywords like `merge`: off
            // outside DuckDb (and Lenient), so elsewhere the word falls through to the
            // unknown-statement error — the reject path for the DuckDB-only operators.
            let start = self.current_span()?;
            self.parse_pivot_statement(start, None)
        } else if self.features().table_factor_syntax.unpivot
            && self.peek_is_keyword(Keyword::Unpivot)?
        {
            let start = self.current_span()?;
            self.parse_unpivot_statement(start, None)
        } else if self.peek_is_contextual_keyword("EXPLAIN")?
            || (self.features().show_syntax.describe
                && (self.peek_is_contextual_keyword("DESCRIBE")?
                    || self.peek_is_contextual_keyword("DESC")?))
        {
            // `EXPLAIN` is ungated (accepted everywhere). MySQL's `DESCRIBE`/`DESC` EXPLAIN
            // synonyms (and the `{DESCRIBE|DESC|EXPLAIN} <table>` table-metadata overload)
            // ride the `describe` gate, so those two leading keywords dispatch here only
            // under MySQL (and Lenient). A non-leading `DESC` — the `ORDER BY … DESC` sort
            // direction — is consumed by the order-by grammar and never reaches this
            // statement-leading position.
            self.parse_explain_or_describe_statement()
        } else if self.features().show_syntax.describe_summarize
            && (self.peek_is_contextual_keyword("DESCRIBE")?
                || self.peek_is_contextual_keyword("DESC")?
                || self.peek_is_contextual_keyword("SUMMARIZE")?)
        {
            // DuckDB's `{DESCRIBE | DESC | SUMMARIZE} <query> | <table>` `SHOW_REF` utility as a
            // top-level statement — placed after the MySQL EXPLAIN/`describe` branch so the
            // permissive superset (both gates on) keeps reading `DESCRIBE` as the EXPLAIN
            // synonym and only routes `SUMMARIZE` here; DuckDB (`describe` off) routes both.
            self.parse_describe_summarize_statement()
        } else if self.peek_starts_query()? || self.peek_is_punct(Punctuation::LParen)? {
            // A leading `(` can only open a parenthesized query at statement level;
            // `parse_query` routes it through the set-operation operand grammar.
            let query = self.parse_query()?;
            let meta = self.make_meta(query.span());
            Ok(Statement::Query {
                query: Box::new(query),
                meta,
            })
        } else {
            Err(self.unexpected("a statement"))
        }
    }

    /// Parse a statement whose leading token is `WITH`.
    ///
    /// `WITH` can introduce either a query expression or a DML statement such as
    /// PostgreSQL's `WITH ... INSERT ...`. The CTE grammar is shared, so parse it
    /// once and dispatch on the token after the CTE list.
    fn parse_statement_starting_with_with(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        let with = self
            .parse_with_clause()?
            .expect("parse_statement_starting_with_with is reached only at WITH");
        if self.peek_is_keyword(Keyword::Insert)? {
            return self.parse_insert_statement_with(start, Some(with));
        }
        if self.peek_is_contextual_keyword("UPDATE")? {
            return self.parse_update_statement_with(start, Some(with));
        }
        if self.peek_is_contextual_keyword("DELETE")? {
            return self.parse_delete_statement_with(start, Some(with));
        }
        // Same gate as the leading-keyword `MERGE` dispatch; the `cte_before_merge`
        // restriction (ANSI: SQL:2016's `<merge statement>` takes no `WITH`) is
        // enforced inside `parse_merge_statement`, mirroring `cte_before_insert`.
        if self.features().mutation_syntax.merge && self.peek_is_keyword(Keyword::Merge)? {
            return self.parse_merge_statement(start, Some(with));
        }
        // DuckDB attaches a `WITH` prefix to the pivot statements exactly as it does
        // to the DML statements above (`WITH c AS (…) PIVOT c ON …`); same gates as
        // the leading-keyword dispatch.
        if self.features().table_factor_syntax.pivot && self.peek_is_keyword(Keyword::Pivot)? {
            return self.parse_pivot_statement(start, Some(with));
        }
        if self.features().table_factor_syntax.unpivot && self.peek_is_keyword(Keyword::Unpivot)? {
            return self.parse_unpivot_statement(start, Some(with));
        }
        if self.peek_is_keyword(Keyword::Select)?
            || (self.features().select_syntax.explicit_table
                && self.peek_is_keyword(Keyword::Table)?)
            || self.peek_is_keyword(Keyword::Values)?
            || self.peek_is_punct(Punctuation::LParen)?
            || (self.features().select_syntax.from_first && self.peek_is_keyword(Keyword::From)?)
        {
            let query = self.parse_query_after_with(start, Some(with))?;
            let meta = self.make_meta(query.span());
            return Ok(Statement::Query {
                query: Box::new(query),
                meta,
            });
        }
        Err(self.unexpected(
            "`SELECT`, `TABLE`, `VALUES`, `FROM`, `(`, `INSERT`, `UPDATE`, or `DELETE` after `WITH`",
        ))
    }

    /// Parse a query expression: a set-expression body plus the query-level
    /// `ORDER BY` and `LIMIT`/`OFFSET` clauses that bind the whole result.
    ///
    /// `ORDER BY`/`LIMIT` attach here, outside the set-operation tree, so they
    /// order/limit the combined result rather than a single set-op operand.
    pub(super) fn parse_query(&mut self) -> ParseResult<Query<D::Ext>> {
        // The recursion-guarded entry to the query grammar (ADR-0012): every nested
        // query re-enters here — a scalar/`IN`/`EXISTS` subquery, a parenthesized
        // set-operation operand, a derived table in `FROM`, a CTE body — so guarding
        // this one method bounds all query nesting that is not a bare parenthesized
        // join factor (which has its own guard in `from`, since it is not a query).
        let span = self.current_span()?;
        // A nested query never inherits the enclosing `CONNECT BY` condition's `PRIOR`
        // context: a scalar/`IN`/`EXISTS` subquery inside a hierarchical condition reads
        // its own `PRIOR` as an ordinary identifier. Cleared for the whole nested query
        // and restored after, exactly as the `a_expr` boundary resets `restrict_b_expr`.
        let saved_connect_by = self.in_connect_by;
        self.in_connect_by = false;
        let result = {
            let mut guard = self.enter_recursion(span)?;
            guard.parser().parse_query_inner()
        };
        self.in_connect_by = saved_connect_by;
        result
    }

    /// Parse one query, one level deep under the recursion guard.
    fn parse_query_inner(&mut self) -> ParseResult<Query<D::Ext>> {
        // A grouping context (armed by a `(`-led FROM/scalar-subquery grouping helper)
        // applies only to a *bare* parenthesized query body — never across a `WITH` clause.
        // A CTE body is its own `( select-stmt )` (a leading paren operand is rejected
        // there), and a `WITH … <body>` query's body is a select-core, so a leading `(`
        // after the `WITH` is an operand SQLite rejects, not a grouping. Capture and clear
        // the flag across `parse_with_clause`, then restore it for `parse_set_expr` only
        // when no `WITH` clause intervened.
        let grouping = self.take_paren_query_grouping();
        // Anchor the query span at the first query token (`WITH`, `SELECT`,
        // `VALUES`, or a leading `(` opening a parenthesized operand).
        let start = self.current_span()?;
        let with = self.parse_with_clause()?;
        if with.is_none() {
            self.set_paren_query_grouping(grouping);
        }
        self.parse_query_after_with(start, with)
    }

    /// Finish a query after an already-parsed optional `WITH` clause.
    pub(super) fn parse_query_after_with(
        &mut self,
        start: Span,
        with: Option<With<D::Ext>>,
    ) -> ParseResult<Query<D::Ext>> {
        // Saved before the body (and its clause keywords) is parsed; the query-tail
        // ORDER BY/LIMIT/OFFSET keywords recorded below carry a placeholder owner
        // patched to this `Query`'s id at the end. The body's own SELECT/subquery
        // clauses are already owned by their inner nodes by then, so this patch —
        // scoped to still-pending marks from `clause_marks_start` on — never claims
        // them.
        let clause_marks_start = self.clause_marks_checkpoint();
        let body = self.parse_set_expr()?;
        if self.take_grouped_query_complete() {
            // SQLite grouping context: `body` is exactly a parenthesized-query grouping
            // operand, a complete standalone primary — no `ORDER BY`/`LIMIT` tail may
            // extend it. Return with an empty tail so any following token is left for the
            // grouping helper's closing `)` to require, rejecting `((SELECT 1) LIMIT 1)`
            // as SQLite does. (`with` is `None` here: a grouping query opens with `(`, not
            // `WITH`.)
            let span = start.union(self.preceding_span());
            let meta = self.make_meta(span);
            // No query-tail clauses on a grouping operand; the body's own clause marks
            // are already owned, so this is a no-op kept for the invariant that every
            // Query return patches its range.
            if self.capturing_clause_marks() {
                self.patch_clause_marks(clause_marks_start, meta.node_id);
            }
            return Ok(Query {
                with,
                body,
                order_by: ThinVec::new(),
                order_by_all: None,
                limit_by: None,
                limit: None,
                settings: ThinVec::new(),
                format: None,
                locking: ThinVec::new(),
                pipe_operators: ThinVec::new(),
                for_clause: None,
                meta,
            });
        }
        if self.capturing_clause_marks() && self.peek_is_keyword(Keyword::Order)? {
            let offset = self.current_span()?.start();
            self.record_clause_mark(ClauseKw::OrderBy, offset);
        }
        let (order_by, order_by_all) = self.parse_order_by_clause()?;
        // The limit/offset section head: `LIMIT` (ordinary or ClickHouse `LIMIT … BY`)
        // or a leading `OFFSET`. Recorded from the query tail rather than inside the
        // shared `parse_limit`/`parse_limit_by` (which the PIVOT tail also drives, and
        // whose speculative `LIMIT … BY` probe rewinds), so the mark is taken once,
        // here, against the committed head token.
        if self.capturing_clause_marks() {
            if self.peek_is_keyword(Keyword::Limit)? {
                let offset = self.current_span()?.start();
                self.record_clause_mark(ClauseKw::Limit, offset);
            } else if self.peek_is_keyword(Keyword::Offset)? {
                let offset = self.current_span()?.start();
                self.record_clause_mark(ClauseKw::Offset, offset);
            }
        }
        // ClickHouse `LIMIT n [OFFSET m] BY …` sits between `ORDER BY` and the ordinary
        // `LIMIT` tail, and a query may carry both. `parse_limit_by` speculatively reads
        // a leading `LIMIT` and rewinds unless a `BY` follows, leaving the ordinary
        // `LIMIT` for `parse_limit`.
        let limit_by = self.parse_limit_by()?;
        let limit = self.parse_limit()?;
        // ClickHouse writes `SETTINGS name = value, …` after the ordinary `LIMIT` tail.
        let settings = self.parse_settings()?;
        // ClickHouse `FORMAT <name>` closes the query, the last tail after `SETTINGS`.
        let format = self.parse_format()?;
        let locking = self.parse_locking_clauses()?;
        let pipe_operators = self.parse_pipe_operators()?;
        // MSSQL `FOR XML`/`FOR JSON` closes the query, after every other tail. It shares
        // the `FOR` lead with the locking clauses but partitions on the follow token, so
        // `parse_locking_clauses` above has already declined a `FOR XML`/`FOR JSON` lead
        // (leaving it here) and consumed only genuine `FOR UPDATE`/`FOR SHARE` clauses.
        let for_clause = self.parse_for_clause()?;
        // PostgreSQL's `gram.y` `insertSelectOptions` raises two `FETCH … WITH TIES` guards
        // during raw parsing (so a parse-only oracle rejects them): `WITH TIES` needs a
        // governing `ORDER BY` at this query level, and cannot combine with a `SKIP LOCKED`
        // locking clause. Gated to PostgreSQL; other `fetch_first` dialects keep both forms.
        if self
            .features()
            .query_tail_syntax
            .with_ties_requires_order_by
            && limit
                .as_ref()
                .is_some_and(|limit| limit.with_ties == Some(true))
        {
            let span = start.union(self.preceding_span());
            if order_by.is_empty() && order_by_all.is_none() {
                return Err(self.error_at(
                    span,
                    "an ORDER BY clause: `FETCH … WITH TIES` cannot be specified without `ORDER BY`",
                    self.span_text(span).to_owned(),
                ));
            }
            if locking
                .iter()
                .any(|clause| clause.wait == Some(LockWait::SkipLocked))
            {
                return Err(self.error_at(
                    span,
                    "no `SKIP LOCKED`: `FETCH … WITH TIES` and `SKIP LOCKED` cannot be used together",
                    self.span_text(span).to_owned(),
                ));
            }
        }
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        if self.capturing_clause_marks() {
            self.patch_clause_marks(clause_marks_start, meta.node_id);
        }
        Ok(Query {
            with,
            body,
            order_by,
            order_by_all,
            limit_by,
            limit,
            settings,
            format,
            locking,
            pipe_operators,
            for_clause,
            meta,
        })
    }

    /// Parse the trailing BigQuery/ZetaSQL `|>` pipe-operator chain
    /// ([`Query::pipe_operators`](crate::ast::Query)), written after every other
    /// query-tail clause (`… LIMIT 10 |> WHERE x |> …`).
    ///
    /// Gated by [`QueryTailSyntax::pipe_syntax`](crate::ast::dialect::SelectSyntax): a dialect
    /// without it never even lexes `|>` (the tokenizer shares the gate), so the `|` is left
    /// unconsumed and surfaces as a trailing-input parse error — the reject mechanism the
    /// other query-tail gates use. The loop consumes one `|>`-led operator per turn until no
    /// `|>` leads.
    fn parse_pipe_operators(&mut self) -> ParseResult<ThinVec<PipeOperator<D::Ext>>> {
        if !self.features().query_tail_syntax.pipe_syntax {
            return Ok(ThinVec::new());
        }
        let mut operators = ThinVec::new();
        while self.eat_pipe_arrow()? {
            operators.push(self.parse_pipe_operator()?);
        }
        Ok(operators)
    }

    /// Eat a `|>` pipe-arrow separator, reporting whether one led. Only reached under
    /// `pipe_syntax` (the token lexes only there).
    fn eat_pipe_arrow(&mut self) -> ParseResult<bool> {
        if matches!(self.peek()?, Some(token) if token.kind == TokenKind::Operator(Operator::PipeArrow))
        {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Parse one `|> <KEYWORD> …` pipe operator, dispatched on the keyword after `|>`.
    ///
    /// **The framework seam.** Each `planner-parity-pipe-*` ticket adds exactly one arm
    /// here (matching its leading keyword, parsing its keyword-led body into its
    /// [`PipeOperator`] variant) — the `|>` separator, the
    /// chaining loop, and the `pipe_syntax` gate are already wired by the framework, so an
    /// operator ticket never touches the surrounding query grammar. The framework ships the
    /// reference `WHERE` operator; every other keyword falls to the unexpected-token reject.
    fn parse_pipe_operator(&mut self) -> ParseResult<PipeOperator<D::Ext>> {
        let start = self.current_span()?;
        if self.eat_keyword(Keyword::Where)? {
            let predicate = self.parse_expr()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Where { predicate, meta })
        } else if self.eat_keyword(Keyword::Select)? {
            // `|> SELECT <items>` reuses the ordinary projection item (`SelectItem`) via a
            // plain comma list; the pipe form has no empty/trailing-comma target list, so
            // this is the bare non-empty list rather than `parse_projection`.
            let items = self.parse_comma_separated(Self::parse_select_item)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Select { items, meta })
        } else if self.eat_keyword(Keyword::Extend)? {
            // `|> EXTEND <items>` appends computed columns; it shares `SELECT`'s item shape.
            let items = self.parse_comma_separated(Self::parse_select_item)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Extend { items, meta })
        } else if self.eat_keyword(Keyword::As)? {
            // `|> AS <alias>` names only a range variable — no column-alias list — so it
            // parses a single `ColId` identifier into a `TableAlias` with empty columns; a
            // trailing `(a, b)` is left unconsumed and surfaces as the caller's reject.
            let alias_start = self.current_span()?;
            let name = self.parse_ident()?;
            let alias_meta = self.make_meta(alias_start.union(self.preceding_span()));
            let alias = TableAlias {
                name,
                columns: ThinVec::new(),
                spelling: AliasSpelling::As,
                meta: alias_meta,
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::As { alias, meta })
        } else if self.eat_keyword(Keyword::Order)? {
            // `|> ORDER BY <keys>` reuses the ordinary sort-key parser (`allow_star: false`,
            // matching the query-tail `ORDER BY`), so the keys are identical `OrderByExpr`s.
            self.expect_keyword(Keyword::By)?;
            let keys = self.parse_comma_separated(|parser| parser.parse_order_by_item(false))?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::OrderBy { keys, meta })
        } else if self.eat_keyword(Keyword::Limit)? {
            // `|> LIMIT <count> [OFFSET <skip>]`: the narrow pipe form. Both operands reuse
            // `parse_limit_operand` (the integer-literal-or-`?` count grammar the ordinary
            // `LIMIT` uses), but there is no `FETCH`/`PERCENT`/comma spelling here.
            let count = Box::new(self.parse_limit_operand()?);
            let offset = if self.eat_keyword(Keyword::Offset)? {
                Some(Box::new(self.parse_limit_operand()?))
            } else {
                None
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Limit {
                count,
                offset,
                meta,
            })
        } else if self.eat_keyword(Keyword::Union)? {
            self.parse_pipe_set_operation(SetOperator::Union, start)
        } else if self.eat_keyword(Keyword::Intersect)? {
            self.parse_pipe_set_operation(SetOperator::Intersect, start)
        } else if self.eat_keyword(Keyword::Except)? {
            self.parse_pipe_set_operation(SetOperator::Except, start)
        } else if self.eat_keyword(Keyword::Set)? {
            // `|> SET <col> = <expr>, …` reuses `UpdateAssignment::Single` (the `UPDATE …
            // SET` node) but only its bare `<col> = <expr>` form — no tuple targets, no
            // `DEFAULT` value — so it parses its own narrow assignment rather than the
            // `UPDATE` grammar's `parse_update_assignment`.
            let assignments = self.parse_comma_separated(Self::parse_pipe_set_assignment)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Set { assignments, meta })
        } else if self.eat_keyword(Keyword::Call)? {
            // `|> CALL <func>(<args>) [AS <alias>]` reuses `parse_function_call` for the
            // call and the name-only `TableAlias` (as `|> AS` does) for the optional alias.
            let call_start = self.current_span()?;
            let name = self.parse_object_name()?;
            if !self.peek_is_punct(Punctuation::LParen)? {
                return Err(self.unexpected("`(` after the pipe `CALL` function name"));
            }
            let call = self.parse_function_call(name, call_start)?;
            let alias = if self.eat_keyword(Keyword::As)? {
                let alias_start = self.current_span()?;
                let alias_name = self.parse_ident()?;
                let alias_meta = self.make_meta(alias_start.union(self.preceding_span()));
                Some(Box::new(TableAlias {
                    name: alias_name,
                    columns: ThinVec::new(),
                    spelling: AliasSpelling::As,
                    meta: alias_meta,
                }))
            } else {
                None
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Call {
                call: Box::new(call),
                alias,
                meta,
            })
        } else if self.eat_keyword(Keyword::Aggregate)? {
            // `|> AGGREGATE <aggregates> [GROUP BY <keys>]`. The aggregate list is empty
            // for a grouping-only operator (`|> AGGREGATE GROUP BY x`), detected by a
            // leading `GROUP`; otherwise it is the non-empty comma list. Both lists share
            // `parse_pipe_aggregate_expr` (`<expr> [AS alias] [ASC|DESC] [NULLS …]`).
            let aggregates = if self.peek_is_keyword(Keyword::Group)? {
                ThinVec::new()
            } else {
                self.parse_comma_separated(Self::parse_pipe_aggregate_expr)?
            };
            let group_by = if self.eat_keyword(Keyword::Group)? {
                self.expect_keyword(Keyword::By)?;
                self.parse_comma_separated(Self::parse_pipe_aggregate_expr)?
            } else {
                ThinVec::new()
            };
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Aggregate {
                aggregates,
                group_by,
                meta,
            })
        } else if self.eat_keyword(Keyword::Drop)? {
            // `|> DROP <column>, …` is a bare identifier list — the columns are output
            // names of the current table, never qualified.
            let columns = self.parse_comma_separated(Self::parse_ident)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Drop { columns, meta })
        } else if self.eat_keyword(Keyword::Rename)? {
            // `|> RENAME <old> AS <new>, …` — each mapping a pair of bare identifiers.
            let renames = self.parse_comma_separated(Self::parse_pipe_rename_item)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Rename { renames, meta })
        } else if self.eat_keyword(Keyword::Pivot)? {
            // `|> PIVOT (<aggregates> FOR <column> IN (<values>))` reuses the shared pivot
            // sub-parsers: `parse_pivot_expr` for the aggregate list and
            // `parse_pivot_for_column` for the single `FOR` head. Exactly one `FOR`
            // column, and the aggregate list is non-empty (both ZetaSQL-shaped; the
            // parenthesized body has no `source`, GROUP BY, or statement tail).
            self.expect_punct(Punctuation::LParen, "`(` after pipe `PIVOT`")?;
            let aggregates = self.parse_comma_separated(Self::parse_pivot_expr)?;
            self.expect_keyword(Keyword::For)?;
            let column = Box::new(self.parse_pivot_for_column()?);
            self.expect_punct(Punctuation::RParen, "`)` to close pipe `PIVOT`")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Pivot {
                aggregates,
                column,
                meta,
            })
        } else if self.eat_keyword(Keyword::Unpivot)? {
            // `|> UNPIVOT (<value> FOR <name> IN (<columns>))` mirrors the table-factor
            // UNPIVOT body (minus source, `NULLS`, and alias), reusing
            // `parse_unpivot_name_list` for the value side and `parse_unpivot_column` for
            // the `IN` list; the name side is a single identifier.
            self.expect_punct(Punctuation::LParen, "`(` after pipe `UNPIVOT`")?;
            let value = self.parse_unpivot_name_list()?;
            self.expect_keyword(Keyword::For)?;
            let name = thin_vec![self.parse_ident()?];
            self.expect_keyword(Keyword::In)?;
            self.expect_punct(Punctuation::LParen, "`(` after `IN`")?;
            let columns = self.parse_comma_separated(Self::parse_unpivot_column)?;
            self.expect_punct(Punctuation::RParen, "`)` to close the UNPIVOT column list")?;
            self.expect_punct(Punctuation::RParen, "`)` to close pipe `UNPIVOT`")?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Unpivot {
                value,
                name,
                columns,
                meta,
            })
        } else if self.eat_keyword(Keyword::Tablesample)? {
            // `|> TABLESAMPLE <method> (<args>) [REPEATABLE (<seed>)]` reuses the whole
            // `TableSample` node via the shared body parser (`start` is the `TABLESAMPLE`
            // keyword span). Gated only by `pipe_syntax`, so it does not re-check the
            // `FROM`-relation `table_sample` feature.
            let sample = Box::new(self.parse_table_sample_tail(start)?);
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::TableSample { sample, meta })
        } else if let Some(join) = self.parse_join()? {
            // `|> [<join-type>] JOIN <relation> [ON | USING]` reuses the whole `Join` node.
            // Dispatched last (after the keyword-led arms) because a join leads with one of
            // several keywords (`JOIN`/`INNER`/`LEFT`/`RIGHT`/`FULL`/`CROSS`/`NATURAL`), none
            // of which overlaps a batch-1/2 operator keyword; `parse_join` returns `None`
            // (consuming nothing) when no join keyword leads, so an unknown keyword still
            // falls to the reject below.
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(PipeOperator::Join {
                join: Box::new(join),
                meta,
            })
        } else {
            Err(self.unexpected("a pipe operator keyword"))
        }
    }

    /// Parse a `|> {UNION | INTERSECT | EXCEPT}` set operation after its operator keyword:
    /// an optional `ALL`/`DISTINCT` quantifier and a comma-separated list of parenthesized
    /// operand queries (at least one). `start` is the span of the `|>`-led keyword.
    fn parse_pipe_set_operation(
        &mut self,
        op: SetOperator,
        start: Span,
    ) -> ParseResult<PipeOperator<D::Ext>> {
        let quantifier = if self.eat_keyword(Keyword::All)? {
            Some(SetQuantifier::All)
        } else if self.eat_keyword(Keyword::Distinct)? {
            Some(SetQuantifier::Distinct)
        } else {
            None
        };
        let queries = self.parse_comma_separated(Self::parse_parenthesized_pipe_query)?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(PipeOperator::SetOperation {
            op,
            quantifier,
            queries,
            meta,
        })
    }

    /// Parse one `( <query> )` operand of a `|>` pipe set operation.
    fn parse_parenthesized_pipe_query(&mut self) -> ParseResult<Query<D::Ext>> {
        self.expect_punct(Punctuation::LParen, "`(` before a pipe set-operation query")?;
        let query = self.parse_query()?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close a pipe set-operation query",
        )?;
        Ok(query)
    }

    /// Parse one `|> SET` assignment: the narrow `<column> = <expr>` form, reusing the
    /// `UpdateAssignment::Single` node (as `UPDATE … SET`) with a bare-expression value —
    /// no `( … ) = <source>` tuple targets and no `DEFAULT` right-hand side.
    fn parse_pipe_set_assignment(&mut self) -> ParseResult<UpdateAssignment<D::Ext>> {
        let start = self.current_span()?;
        let target = self.parse_object_name()?;
        if !self.peek_is_op(Operator::Eq)? {
            return Err(self.unexpected("`=` in a pipe `SET` assignment"));
        }
        self.advance()?; // `=`
        let expr = self.parse_expr()?;
        let value_meta = self.make_meta(expr.span());
        let value = UpdateValue::Expr {
            expr,
            meta: value_meta,
        };
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(UpdateAssignment::Single {
            target,
            value,
            meta,
        })
    }

    /// Parse one `|> AGGREGATE` item — an aggregate-list or `GROUP BY` entry: `<expr> [AS
    /// <alias>] [ASC | DESC] [NULLS FIRST | LAST]`. The alias and the ordering suffix
    /// co-occur on one item (ZetaSQL folds grouping and aggregate-driven ordering into
    /// one operator), so both lists share this combined [`PipeAggregateExpr`] shape,
    /// reusing the `ORDER BY` sort-direction / nulls-order helpers for the ordering tail.
    /// The bare (`AS`-less) alias spelling is deferred.
    fn parse_pipe_aggregate_expr(&mut self) -> ParseResult<PipeAggregateExpr<D::Ext>> {
        let start = self.current_span()?;
        let expr = self.parse_expr()?;
        let alias = if self.eat_keyword(Keyword::As)? {
            Some(self.parse_ident()?)
        } else {
            None
        };
        let asc = self.parse_sort_direction()?;
        let nulls_first = self.parse_nulls_order()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(PipeAggregateExpr {
            expr,
            alias,
            asc,
            nulls_first,
            meta,
        })
    }

    /// Parse one `|> RENAME` mapping: `<old> AS <new>`, a pair of bare identifiers (the
    /// `AS` is required; the source column is an unqualified output name).
    fn parse_pipe_rename_item(&mut self) -> ParseResult<PipeRenameItem> {
        let start = self.current_span()?;
        let old = self.parse_ident()?;
        self.expect_keyword(Keyword::As)?;
        let new = self.parse_ident()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(PipeRenameItem { old, new, meta })
    }

    /// Parse the trailing row-locking clauses (`FOR UPDATE`/`FOR SHARE`/`FOR NO KEY
    /// UPDATE`/`FOR KEY SHARE` `[OF <table>, …] [NOWAIT|SKIP LOCKED]`, or MySQL's legacy
    /// `LOCK IN SHARE MODE`), written after `LIMIT`.
    ///
    /// Gated by [`QueryTailSyntax::locking_clauses`](crate::ast::dialect::SelectSyntax); a
    /// dialect without it leaves the `FOR`/`LOCK` keyword unconsumed, so the construct
    /// surfaces as a trailing-input parse error — the reject mechanism the other
    /// query-tail gates use. Exactly one clause is parsed unless
    /// [`QueryTailSyntax::stacked_locking_clauses`](crate::ast::dialect::SelectSyntax) is on
    /// (PostgreSQL), which loops for stacked clauses; MySQL admits exactly one (a trailing
    /// second `FOR …` is then left for the trailing-input reject) — engine truth from
    /// `mysql-select-tails-locking-hints-partition`. The `NO KEY UPDATE`/`KEY SHARE`
    /// strengths ride [`QueryTailSyntax::key_lock_strengths`](crate::ast::dialect::SelectSyntax).
    fn parse_locking_clauses(&mut self) -> ParseResult<ThinVec<LockingClause>> {
        if !self.features().query_tail_syntax.locking_clauses {
            return Ok(ThinVec::new());
        }
        let Some(first) = self.parse_optional_locking_clause()? else {
            return Ok(ThinVec::new());
        };
        let mut clauses = thin_vec![first];
        // PostgreSQL stacks clauses (`FOR UPDATE OF a FOR SHARE OF b`); MySQL stops at one
        // (`stacked_locking_clauses` off), so a following `FOR`/`LOCK` stays unconsumed and
        // fails the trailing-input reject.
        if self.features().query_tail_syntax.stacked_locking_clauses {
            while let Some(clause) = self.parse_optional_locking_clause()? {
                clauses.push(clause);
            }
        }
        Ok(clauses)
    }

    /// Parse one optional locking clause, or `None` when no `FOR`/`LOCK IN SHARE MODE`
    /// leads. Only reached under [`QueryTailSyntax::locking_clauses`](crate::ast::dialect::QueryTailSyntax::locking_clauses).
    fn parse_optional_locking_clause(&mut self) -> ParseResult<Option<LockingClause>> {
        let start = self.current_span()?;
        // Follow-token partition against the MSSQL `FOR XML`/`FOR JSON` result-shaping
        // tail (both gates on only under Lenient): a `FOR` whose follow token is
        // `XML`/`JSON` is that clause, not a locking clause, so decline it here and let
        // `parse_for_clause` read it. This keeps a stacked `FOR UPDATE … FOR XML` valid —
        // the stacking loop stops at the `FOR XML` instead of mis-reading it as a locking
        // strength — and needs no `GrammarConflict` entry (the follow sets are disjoint).
        if self.for_clause_leads()? {
            return Ok(None);
        }
        if self.eat_keyword(Keyword::For)? {
            // `FOR UPDATE` / `FOR SHARE` are the modern strengths MySQL and PostgreSQL
            // share; PostgreSQL adds `FOR NO KEY UPDATE` / `FOR KEY SHARE` under
            // `key_lock_strengths`. PostgreSQL pairs `NO KEY` only with `UPDATE` and
            // `KEY` only with `SHARE` (`FOR KEY UPDATE` / `FOR NO KEY SHARE` are engine
            // rejects), so `expect_keyword` after each lead enforces the pairing.
            let key_strengths = self.features().query_tail_syntax.key_lock_strengths;
            let strength = if self.eat_keyword(Keyword::Update)? {
                LockStrength::Update
            } else if self.eat_keyword(Keyword::Share)? {
                LockStrength::Share
            } else if key_strengths && self.eat_keyword(Keyword::No)? {
                self.expect_keyword(Keyword::Key)?;
                self.expect_keyword(Keyword::Update)?;
                LockStrength::NoKeyUpdate
            } else if key_strengths && self.eat_keyword(Keyword::Key)? {
                self.expect_keyword(Keyword::Share)?;
                LockStrength::KeyShare
            } else if key_strengths {
                return Err(self
                    .unexpected("`UPDATE`, `SHARE`, `NO KEY UPDATE`, or `KEY SHARE` after `FOR`"));
            } else {
                return Err(self.unexpected("`UPDATE` or `SHARE` after `FOR`"));
            };
            // Optional `OF <table>, …` restriction — a relation list (both MySQL and
            // PostgreSQL admit the comma form; engine-verified).
            let of = if self.eat_keyword(Keyword::Of)? {
                self.parse_comma_separated(Self::parse_object_name)?
            } else {
                ThinVec::new()
            };
            let wait = self.parse_lock_wait()?;
            let span = start.union(self.preceding_span());
            Ok(Some(LockingClause {
                strength,
                of,
                wait,
                spelling: LockingSpelling::Modern,
                meta: self.make_meta(span),
            }))
        } else if self.peek_is_keyword(Keyword::Lock)?
            && self.peek_nth_is_keyword(1, Keyword::In)?
        {
            // MySQL legacy `LOCK IN SHARE MODE` — a bare spelling of `FOR SHARE`.
            self.advance()?; // LOCK
            self.expect_keyword(Keyword::In)?;
            self.expect_keyword(Keyword::Share)?;
            self.expect_keyword(Keyword::Mode)?;
            let span = start.union(self.preceding_span());
            Ok(Some(LockingClause {
                strength: LockStrength::Share,
                of: ThinVec::new(),
                wait: None,
                spelling: LockingSpelling::LockInShareMode,
                meta: self.make_meta(span),
            }))
        } else {
            Ok(None)
        }
    }

    /// Parse the optional `NOWAIT` / `SKIP LOCKED` wait-policy tail of a locking clause.
    fn parse_lock_wait(&mut self) -> ParseResult<Option<LockWait>> {
        if self.eat_keyword(Keyword::Nowait)? {
            Ok(Some(LockWait::NoWait))
        } else if self.eat_keyword(Keyword::Skip)? {
            self.expect_keyword(Keyword::Locked)?;
            Ok(Some(LockWait::SkipLocked))
        } else {
            Ok(None)
        }
    }

    /// Parse an optional `WITH [RECURSIVE] <cte> [, ...]` clause.
    pub(super) fn parse_with_clause(&mut self) -> ParseResult<Option<With<D::Ext>>> {
        if !self.eat_keyword(Keyword::With)? {
            return Ok(None);
        }

        let start = self.preceding_span();
        let recursive = self.eat_keyword(Keyword::Recursive)?;
        let ctes = self.parse_comma_separated(Self::parse_cte)?;
        // DuckDB parse-rejects a top-level `ORDER BY`/`LIMIT`/`OFFSET` on a recursive CTE
        // whose body is a `UNION [ALL]` set operation — the recursive term's grammar
        // special-case (`Parser Error: ORDER BY in a recursive query is not allowed`;
        // probed on 1.5.4). Only under `WITH RECURSIVE` and the dialect flag, so every
        // other preset keeps the modifier (PostgreSQL parse-accepts it; the rest defer the
        // restriction to binding). Detection lives in a separate function so its locals
        // stay off this frame, which sits on the recursive query-nesting path.
        if recursive
            && self
                .features()
                .join_syntax
                .recursive_union_rejects_order_limit
        {
            if let Some(span) = ctes.iter().find_map(Self::recursive_union_modifier_reject) {
                return Err(self.error_at(
                    span,
                    "no ORDER BY/LIMIT/OFFSET on a recursive CTE's UNION query (a recursive \
                     query cannot carry these modifiers)",
                    self.span_text(span).to_owned(),
                ));
            }
        }
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(With {
            recursive,
            ctes,
            meta,
        }))
    }

    /// The span of a modifier DuckDB parse-rejects on a recursive query, or `None`.
    ///
    /// Under a `WITH RECURSIVE`, a CTE whose body is a `UNION [ALL]` set operation is a
    /// *recursive query*, and DuckDB forbids a top-level `ORDER BY`/`LIMIT`/`OFFSET` on it
    /// (returns the offending query's span so the caller can point the error at it). Three
    /// boundaries are engine-probed (1.5.4) and mirrored here: the body must be a `UNION`
    /// set operation — an `INTERSECT`/`EXCEPT` body or a non-set-op body is not
    /// recursive-eligible and keeps its modifiers; the modifier must sit on the set
    /// operation itself, so a parenthesized arm (`((SELECT … LIMIT 1) UNION ALL …)`) or a
    /// nested subquery is a distinct query node this never inspects; and self-reference is
    /// not required (the check is syntactic, exactly as DuckDB's parser is).
    fn recursive_union_modifier_reject(cte: &Cte<D::Ext>) -> Option<Span> {
        let CteBody::Query { query, .. } = &cte.body else {
            return None;
        };
        if !matches!(
            query.body,
            SetExpr::SetOperation {
                op: SetOperator::Union,
                ..
            }
        ) {
            return None;
        }
        let has_order = !query.order_by.is_empty() || query.order_by_all.is_some();
        let has_limit = query
            .limit
            .as_ref()
            .is_some_and(|limit| limit.limit.is_some() || limit.offset.is_some());
        (has_order || has_limit).then(|| query.span())
    }

    /// Parse one common table expression:
    /// `<name> [(<col>, ...)] AS [MATERIALIZED|NOT MATERIALIZED] (<body>)`
    /// `[SEARCH …] [CYCLE …]`.
    fn parse_cte(&mut self) -> ParseResult<Cte<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        let columns = self.parse_cte_columns()?;
        let using_key = self.parse_cte_using_key()?;
        self.expect_keyword(Keyword::As)?;
        let materialized = self.parse_cte_materialization_hint()?;
        self.expect_punct(Punctuation::LParen, "`(` to open the CTE query")?;
        let body = self.parse_cte_body()?;
        self.expect_punct(Punctuation::RParen, "`)` to close the CTE query")?;
        // The SQL:2023 recursive-query clauses trail the body's `)`, in fixed order:
        // SEARCH before CYCLE, at most one of each (`CYCLE … SEARCH …` and a repeated
        // clause both parse-reject; probed on pg_query 17). Both ride
        // `recursive_search_cycle`; when off, a trailing `SEARCH`/`CYCLE` keyword is left
        // for the enclosing grammar (after the CTE list expects `,` or the main query) and
        // surfaces as the clean parse error the other dialects give.
        let (search, cycle) = if self.features().join_syntax.recursive_search_cycle {
            (
                self.parse_cte_search_clause()?,
                self.parse_cte_cycle_clause()?,
            )
        } else {
            (None, None)
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Cte {
            name,
            columns,
            using_key,
            materialized,
            body,
            search,
            cycle,
            meta,
        })
    }

    /// Parse an optional DuckDB `USING KEY (col [, ...])` recursive-CTE key clause, sitting
    /// between the CTE column list and `AS`. Gated by
    /// [`JoinSyntax::recursive_using_key`](crate::ast::dialect::JoinSyntax); when off, a
    /// `USING` here is left for the enclosing grammar (which expects `AS`) and surfaces as
    /// the clean parse error the other dialects give. The key list is a non-empty bare
    /// column list (`(x)` in `USING KEY (x)`).
    fn parse_cte_using_key(&mut self) -> ParseResult<Option<ThinVec<crate::ast::Ident>>> {
        if !self.features().join_syntax.recursive_using_key || !self.eat_keyword(Keyword::Using)? {
            return Ok(None);
        }
        self.expect_keyword(Keyword::Key)?;
        self.expect_punct(Punctuation::LParen, "`(` to open the USING KEY column list")?;
        let columns = self.parse_comma_separated(Self::parse_ident)?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the USING KEY column list",
        )?;
        Ok(Some(columns))
    }

    /// Parse an optional `SEARCH { DEPTH | BREADTH } FIRST BY col [, ...] SET seqcol`
    /// clause ([`CteSearchClause`]) trailing a CTE body. One of `DEPTH`/`BREADTH` is
    /// mandatory (`SEARCH FIRST …` with neither is a parse error), the columns are bare
    /// names, and the `SET` sequence column is required.
    fn parse_cte_search_clause(&mut self) -> ParseResult<Option<Box<CteSearchClause>>> {
        let start = self.current_span()?;
        if !self.eat_keyword(Keyword::Search)? {
            return Ok(None);
        }
        let breadth_first = if self.eat_keyword(Keyword::Breadth)? {
            true
        } else {
            self.expect_keyword(Keyword::Depth)?;
            false
        };
        self.expect_keyword(Keyword::First)?;
        self.expect_keyword(Keyword::By)?;
        let columns = self.parse_comma_separated(Self::parse_ident)?;
        self.expect_keyword(Keyword::Set)?;
        let set_column = self.parse_ident()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(Box::new(CteSearchClause {
            breadth_first,
            columns,
            set_column,
            meta,
        })))
    }

    /// Parse an optional `CYCLE col [, ...] SET mark [TO value DEFAULT default] USING path`
    /// clause ([`CteCycleClause`]). The `SET` mark and `USING` path columns are required;
    /// the `TO … DEFAULT …` mark values are admitted only together — `TO` without `DEFAULT`
    /// (or the reverse) is a parse error — and each is a restricted
    /// [`AexprConst`](Self::parse_aexpr_const), never a general expression.
    fn parse_cte_cycle_clause(&mut self) -> ParseResult<Option<Box<CteCycleClause<D::Ext>>>> {
        let start = self.current_span()?;
        if !self.eat_keyword(Keyword::Cycle)? {
            return Ok(None);
        }
        let columns = self.parse_comma_separated(Self::parse_ident)?;
        self.expect_keyword(Keyword::Set)?;
        let mark_column = self.parse_ident()?;
        let mark = if self.eat_keyword(Keyword::To)? {
            let mark_start = self.preceding_span();
            let value = self.parse_aexpr_const()?;
            self.expect_keyword(Keyword::Default)?;
            let default = self.parse_aexpr_const()?;
            let meta = self.make_meta(mark_start.union(self.preceding_span()));
            Some(CteCycleMark {
                value: Box::new(value),
                default: Box::new(default),
                meta,
            })
        } else {
            None
        };
        self.expect_keyword(Keyword::Using)?;
        let path_column = self.parse_ident()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(Box::new(CteCycleClause {
            columns,
            mark_column,
            mark,
            path_column,
            meta,
        })))
    }

    /// Parse the parenthesized CTE body ([`CteBody`]): a query, or — under
    /// [`MutationSyntax::data_modifying_ctes`](crate::ast::dialect::MutationSyntax) —
    /// one of PostgreSQL's `PreparableStmt` DML statements
    /// (`WITH t AS (DELETE FROM x RETURNING *) SELECT * FROM t`).
    fn parse_cte_body(&mut self) -> ParseResult<CteBody<D::Ext>> {
        // Recursion-guarded like `parse_query` (ADR-0012): the WITH-led DML dispatch
        // below consumes a nested `WITH` clause *before* reaching any other guarded
        // entry point, so `WITH t AS (WITH t AS (…` nesting must be bounded here.
        let span = self.current_span()?;
        let mut guard = self.enter_recursion(span)?;
        guard.parser().parse_cte_body_inner()
    }

    /// Parse one CTE body, one level deep under the recursion guard.
    fn parse_cte_body_inner(&mut self) -> ParseResult<CteBody<D::Ext>> {
        let start = self.current_span()?;
        if self.features().mutation_syntax.data_modifying_ctes {
            // The DML arms of PostgreSQL's `common_table_expr` grammar. Only the four
            // `PreparableStmt` DML heads are dispatched — a utility statement inside a
            // CTE stays a parse error, as PostgreSQL rejects it. `MERGE` additionally
            // requires the `merge` gate, exactly as the statement-level dispatch does.
            if self.peek_is_keyword(Keyword::Insert)? {
                return Ok(Self::cte_body_dml(
                    self.parse_insert_statement_with(start, None)?,
                ));
            }
            if self.peek_is_contextual_keyword("UPDATE")? {
                return Ok(Self::cte_body_dml(
                    self.parse_update_statement_with(start, None)?,
                ));
            }
            if self.peek_is_contextual_keyword("DELETE")? {
                return Ok(Self::cte_body_dml(
                    self.parse_delete_statement_with(start, None)?,
                ));
            }
            if self.features().mutation_syntax.merge && self.peek_is_keyword(Keyword::Merge)? {
                return Ok(Self::cte_body_dml(self.parse_merge_statement(start, None)?));
            }
            if self.peek_is_keyword(Keyword::With)? {
                // A DML body may itself lead with a `WITH` clause (PostgreSQL nests
                // `WITH t AS (WITH u AS (…) INSERT …)`; probed on pg_query 17), so
                // mirror the statement-level dispatch: consume the clause once, then
                // route by the following keyword — the DML statements attach it to
                // their own `with` slot, everything else finishes as a query.
                let with = self.parse_with_clause()?;
                if self.peek_is_keyword(Keyword::Insert)? {
                    return Ok(Self::cte_body_dml(
                        self.parse_insert_statement_with(start, with)?,
                    ));
                }
                if self.peek_is_contextual_keyword("UPDATE")? {
                    return Ok(Self::cte_body_dml(
                        self.parse_update_statement_with(start, with)?,
                    ));
                }
                if self.peek_is_contextual_keyword("DELETE")? {
                    return Ok(Self::cte_body_dml(
                        self.parse_delete_statement_with(start, with)?,
                    ));
                }
                if self.features().mutation_syntax.merge && self.peek_is_keyword(Keyword::Merge)? {
                    return Ok(Self::cte_body_dml(self.parse_merge_statement(start, with)?));
                }
                let query = self.parse_query_after_with(start, with)?;
                let meta = self.make_meta(query.span());
                return Ok(CteBody::Query {
                    query: Box::new(query),
                    meta,
                });
            }
        }
        let query = self.parse_query()?;
        let meta = self.make_meta(query.span());
        Ok(CteBody::Query {
            query: Box::new(query),
            meta,
        })
    }

    /// Wrap a parsed DML statement into its [`CteBody`] arm. The CTE-body dispatch
    /// only routes the four DML heads here, so every other variant is unreachable.
    fn cte_body_dml(statement: Statement<D::Ext>) -> CteBody<D::Ext> {
        match statement {
            Statement::Insert { insert, meta } => CteBody::Insert { insert, meta },
            Statement::Update { update, meta } => CteBody::Update { update, meta },
            Statement::Delete { delete, meta } => CteBody::Delete { delete, meta },
            Statement::Merge { merge, meta } => CteBody::Merge { merge, meta },
            _ => unreachable!("only the four DML statements are dispatched into a CTE body"),
        }
    }

    /// Parse an optional CTE column list.
    fn parse_cte_columns(&mut self) -> ParseResult<ThinVec<crate::ast::Ident>> {
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok(ThinVec::new());
        }

        let columns = self.parse_comma_separated(Self::parse_ident)?;
        self.expect_punct(Punctuation::RParen, "`)` to close the CTE column list")?;
        Ok(columns)
    }

    /// Parse PostgreSQL's CTE materialization hint spelling after `AS`.
    fn parse_cte_materialization_hint(&mut self) -> ParseResult<Option<bool>> {
        if self.eat_keyword(Keyword::Materialized)? {
            Ok(Some(true))
        } else if self.eat_keyword(Keyword::Not)? {
            self.expect_keyword(Keyword::Materialized)?;
            Ok(Some(false))
        } else {
            Ok(None)
        }
    }

    /// Parse a set-expression body using dialect set-operation binding powers.
    ///
    /// Set operations are query-body infix operators, so they use a small
    /// precedence climb parallel to expression Pratt parsing. SQL/PostgreSQL rank
    /// `INTERSECT` above `UNION`/`EXCEPT`, and all current M1 set operators are
    /// left-associative within their precedence level. The binding powers live in
    /// [`FeatureSet`](crate::ast::dialect::FeatureSet), which is the same source
    /// the renderer reads when deciding whether nested set operations need
    /// parentheses.
    fn parse_set_expr(&mut self) -> ParseResult<SetExpr<D::Ext>> {
        self.parse_set_expr_bp(0)
    }

    /// Parse a set expression whose leading operator binds at least `min_bp`.
    fn parse_set_expr_bp(&mut self, min_bp: u8) -> ParseResult<SetExpr<D::Ext>> {
        // A grouping context (SQLite) arms only the *leading* operand's `(` — a
        // `( table-or-subquery )` / `( select-stmt )` grouping — never a compound's right
        // operand (whose leading `(` is a bare paren operand SQLite rejects). Capture the
        // one-shot flag and re-arm it solely when a leading `(` follows.
        let grouping = self.take_paren_query_grouping();
        if grouping && self.peek_is_punct(Punctuation::LParen)? {
            self.set_paren_query_grouping(true);
        }
        let mut body = self.parse_set_operand()?;
        // A grouping paren-operand is a complete standalone primary (SQLite): it is not a
        // compound operand, so do not climb the set-operation loop — the enclosing
        // grouping helper's required `)` then rejects any trailing set operator
        // (`((SELECT 1) UNION (SELECT 2))`). The flag is left set for
        // `parse_query_after_with` to also suppress the `ORDER BY`/`LIMIT` tail.
        if self.grouped_query_complete() {
            return Ok(body);
        }
        while let Some(op) = self.peek_set_operator()? {
            let bp = self.features().set_operation_binding_power(&op);
            if bp.left < min_bp {
                break;
            }
            self.advance()?; // the set-operator keyword (UNION / INTERSECT / EXCEPT)
            let all = if self.peek_is_keyword(Keyword::All)? {
                let allowed = match op {
                    SetOperator::Intersect => self.features().select_syntax.intersect_all,
                    SetOperator::Except => self.features().select_syntax.except_all,
                    SetOperator::Union => true,
                };
                if !allowed {
                    return Err(self.unexpected("a set-operation modifier allowed by the dialect"));
                }
                self.advance()?;
                true
            } else {
                false
            };
            // DuckDB's name-matched `UNION [ALL] BY NAME` (columns paired by name, not
            // position). UNION-only: `INTERSECT BY NAME` / `EXCEPT BY NAME` are DuckDB
            // syntax errors (probed on 1.5.4), so after a non-`UNION` operator `BY` is
            // left unconsumed and surfaces as the usual operand reject. Consuming the
            // pair here (before the right operand) chains left-associatively with the
            // enclosing loop exactly as an ordinary set operator does. Gated by
            // `SelectSyntax::union_by_name`; `BY` opens no operand, so the peek is
            // unambiguous.
            let by_name = self.features().select_syntax.union_by_name
                && matches!(op, SetOperator::Union)
                && self.peek_is_keyword(Keyword::By)?;
            if by_name {
                self.advance()?; // BY
                self.expect_keyword(Keyword::Name)?;
            }
            let right = self.parse_set_expr_bp(bp.right)?;
            let span = body.span().union(right.span());
            let meta = self.make_meta(span);
            body = SetExpr::SetOperation {
                op,
                all,
                by_name,
                left: Box::new(body),
                right: Box::new(right),
                meta,
            };
        }
        Ok(body)
    }

    /// Parse a single set-operation operand.
    ///
    /// Parenthesized set operands are grouping only when the inner query carries
    /// no query-level clauses; in that case the public AST stores no paren node,
    /// matching expression grouping. When the inner query has `WITH`, `ORDER BY`,
    /// or `LIMIT`/`OFFSET`, the parentheses are load-bearing and the operand stays
    /// a [`SetExpr::Query`].
    fn parse_set_operand(&mut self) -> ParseResult<SetExpr<D::Ext>> {
        if self.peek_is_keyword(Keyword::Select)? {
            let select = self.parse_select()?;
            let meta = self.make_meta(select.span());
            Ok(SetExpr::Select {
                select: Box::new(select),
                meta,
            })
        } else if self.features().select_syntax.explicit_table
            && self.peek_is_keyword(Keyword::Table)?
        {
            // `TABLE name` is `SELECT * FROM name` (SQL `<explicit table>`), so it
            // canonicalizes to a `Select` operand and composes in set operations
            // (`TABLE a UNION TABLE b`) exactly as a bare `SELECT` does. Gated by
            // [`SelectSyntax::explicit_table`]: off for SQLite, which syntax-rejects a
            // leading `TABLE` (engine-measured on rusqlite).
            let select = self.parse_table_command()?;
            let meta = self.make_meta(select.span());
            Ok(SetExpr::Select {
                select: Box::new(select),
                meta,
            })
        } else if self.peek_is_keyword(Keyword::Values)? {
            let values = self.parse_values()?;
            let meta = self.make_meta(values.span());
            Ok(SetExpr::Values {
                values: Box::new(values),
                meta,
            })
        } else if self.features().select_syntax.from_first && self.peek_is_keyword(Keyword::From)? {
            // DuckDB's FROM-first primary (`FROM t SELECT x`, bare `FROM t`): the standard
            // `Select` shape written FROM-first, so it composes as a set operand
            // (`FROM a SELECT x UNION FROM b SELECT y`) exactly as a bare `SELECT` does.
            let select = self.parse_select_from_first()?;
            let meta = self.make_meta(select.span());
            Ok(SetExpr::Select {
                select: Box::new(select),
                meta,
            })
        } else if self.features().table_factor_syntax.pivot
            && self.peek_is_keyword(Keyword::Pivot)?
        {
            // DuckDB admits `PIVOT`/`UNPIVOT` as a query body (a CTE body, a
            // `CREATE VIEW/TABLE AS`/`CREATE MACRO … AS TABLE` body) but *not*
            // `DESCRIBE`/`SHOW` (those parse-reject there — `A CTE needs a SELECT`), so
            // the statement heads are admitted here at the single query-body leaf rather
            // than through `peek_starts_query` (which would also reroute the derived-table
            // `FROM (PIVOT …)` case off its `TableFactor::Pivot` reading). Any leading
            // `WITH` was already consumed by `parse_query_after_with` and attached to the
            // enclosing `Query`, so the operator itself carries none.
            let start = self.current_span()?;
            let pivot = self.parse_pivot_operator(start, None)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(SetExpr::Pivot {
                pivot: Box::new(pivot),
                meta,
            })
        } else if self.features().table_factor_syntax.unpivot
            && self.peek_is_keyword(Keyword::Unpivot)?
        {
            let start = self.current_span()?;
            let unpivot = self.parse_unpivot_operator(start, None)?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            Ok(SetExpr::Unpivot {
                unpivot: Box::new(unpivot),
                meta,
            })
        } else if self.peek_is_punct(Punctuation::LParen)? {
            self.parse_parenthesized_set_operand()
        } else {
            Err(self.unexpected("`SELECT`, `TABLE`, `VALUES`, or `(`"))
        }
    }

    /// Parse a parenthesized query in set-expression operand position.
    ///
    /// Gated by [`SelectSyntax::parenthesized_query_operands`](crate::ast::dialect::SelectSyntax):
    /// off (SQLite), a leading `(` is a bare compound operand SQLite lacks and is rejected —
    /// *except* in a grouping context (armed by the FROM / scalar-subquery helper), where the
    /// paren-query is a `( table-or-subquery )` / `( select-stmt )` grouping. There it is a
    /// complete standalone primary: the operand is parsed, [`mark_grouped_query_complete`](Self::mark_grouped_query_complete)
    /// forbids the enclosing query from extending it, and the context is propagated one level
    /// down so `( ( select-stmt ) )` nests.
    fn parse_parenthesized_set_operand(&mut self) -> ParseResult<SetExpr<D::Ext>> {
        let grouping = self.take_paren_query_grouping();
        let paren_operands = self.features().select_syntax.parenthesized_query_operands;
        if !paren_operands && !grouping {
            return Err(self.unexpected("`SELECT`, `TABLE`, or `VALUES`"));
        }
        let open = self
            .advance()?
            .expect("parse_parenthesized_set_operand is reached only at `(`");
        if !(self.peek_starts_query()? || self.peek_is_punct(Punctuation::LParen)?) {
            return Err(self.unexpected("a query inside the set-operation parentheses"));
        }
        // Propagate the grouping context: a `(` directly inside a grouping `(` is itself a
        // table-or-subquery grouping (`( ( select-stmt ) )`).
        self.set_paren_query_grouping(grouping);
        let query = self.parse_query()?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the set-operation operand",
        )?;
        if grouping && !paren_operands {
            // SQLite grouping: this paren-query is the whole grouped content — no compound
            // or `ORDER BY`/`LIMIT` may extend it (enforced by the callers that read the
            // flag), so `((SELECT 1) UNION …)` / `((SELECT 1) LIMIT 1)` reject.
            self.mark_grouped_query_complete();
        }

        if query.with.is_none()
            && query.order_by.is_empty()
            && query.order_by_all.is_none()
            && query.limit.is_none()
            && query.locking.is_empty()
        {
            Ok(query.body)
        } else {
            let span = open.span.union(self.preceding_span());
            let meta = self.make_meta(span);
            Ok(SetExpr::Query {
                query: Box::new(query),
                meta,
            })
        }
    }

    /// Parse a `VALUES (<expr>, ...) [, ...]` query body.
    ///
    /// This is the *query-position* constructor — a top-level query body, a set-operation
    /// operand, a CTE body, or a derived table — distinct from the `INSERT … VALUES (…)`
    /// source list ([`parse_insert_values`](Self::parse_insert_values)), which is a separate
    /// path. [`SelectSyntax::values_row_constructor`](crate::ast::dialect::SelectSyntax) gates
    /// the bare-parenthesized row here: MySQL spells the query-position constructor
    /// `VALUES ROW(…), …` and syntax-rejects a bare `(…)` row (engine-measured 1064), so with
    /// the flag off a leading `(` after `VALUES` is that reject. The INSERT source, which
    /// admits bare rows on every dialect, is unaffected.
    ///
    /// `pub(super)` so the `FROM` grammar in [`super::from`] can read the same row list
    /// for DuckDB's bare `FROM VALUES (…) AS t` table factor, reusing this constructor
    /// (and its ragged-row reject) rather than re-deriving the row grammar.
    pub(super) fn parse_values(&mut self) -> ParseResult<Values<D::Ext>> {
        let keyword = self
            .advance()?
            .expect("reached parse_values only at VALUES");
        // MySQL spells the query-position constructor `VALUES ROW(1), ROW(2)` and
        // syntax-rejects a bare `(…)` row; every other dialect requires the bare row and
        // has no `ROW` keyword here. The gate is exactly the constructor spelling, so it
        // both selects the row grammar and records the `explicit_row` surface tag.
        let explicit_row = !self.features().select_syntax.values_row_constructor;
        let rows = if explicit_row {
            // A MySQL `ROW( ... )` list. MySQL rejects a trailing comma, so no tolerance.
            self.parse_comma_separated(Self::parse_values_row_explicit)?
        } else {
            // DuckDB tolerates a trailing comma after the row list (`VALUES (1), (2),`); a row
            // always opens with `(`, so a post-comma token that is not `(` is that trailing
            // comma (the closer is `)`/`AS`/a query-tail clause/`;`/end — all non-`(`).
            self.parse_comma_separated_trailing(Self::parse_values_row, |p| {
                Ok(!p.peek_is_punct(Punctuation::LParen)?)
            })?
        };
        let span = keyword.span.union(self.preceding_span());
        self.reject_ragged_values_rows(&rows, span)?;
        let meta = self.make_meta(span);
        Ok(Values {
            explicit_row,
            rows,
            meta,
        })
    }

    /// Parse one MySQL `ROW( ... )` explicit-row-constructor `VALUES` row (the query-position
    /// spelling when [`SelectSyntax::values_row_constructor`](crate::ast::dialect::SelectSyntax)
    /// is off). The `ROW` keyword and its parentheses are mandatory here — a bare `(…)` row is
    /// the `ER_PARSE_ERROR` MySQL reports. The row items reuse
    /// [`parse_values_item`](Self::parse_values_item) (an expression or a bare `DEFAULT`), and
    /// the item list is non-empty: an empty `ROW()` is engine-measured `ER_EMPTY_ROW_IN_TVC`
    /// on mysql:8 — a resolver reject the parse layer folds into the natural
    /// non-empty-list reject here.
    fn parse_values_row_explicit(&mut self) -> ParseResult<ThinVec<ValuesItem<D::Ext>>> {
        if !self.peek_is_keyword(Keyword::Row)? {
            return Err(self.unexpected(
                "a `ROW(…)` row constructor: a query-position `VALUES` spells its rows \
                 `VALUES ROW(…), …` here, not with bare parentheses",
            ));
        }
        self.expect_keyword(Keyword::Row)?;
        self.expect_punct(
            Punctuation::LParen,
            "`(` to open the `ROW( ... )` VALUES row",
        )?;
        let row = self.parse_comma_separated(Self::parse_values_item)?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the `ROW( ... )` VALUES row",
        )?;
        Ok(row)
    }

    /// Reject a `VALUES` table-value constructor whose rows differ in width, under
    /// [`SelectSyntax::values_rows_require_equal_arity`](crate::ast::dialect::SelectSyntax).
    /// DuckDB enforces equal row degree at *parse* (`VALUES lists must all be the same
    /// length`); PostgreSQL/MySQL defer it to bind, so the gate is dialect data. Shared by
    /// the query-body constructor (here) and the `INSERT ... VALUES` source
    /// ([`parse_insert_values`](Self::parse_insert_values)) — the same reject fires in
    /// every VALUES position DuckDB checks — so it is generic over the row element type.
    /// `span` locates the whole constructor for the diagnostic.
    pub(super) fn reject_ragged_values_rows<T>(
        &mut self,
        rows: &[ThinVec<T>],
        span: Span,
    ) -> ParseResult<()> {
        if !self
            .features()
            .select_syntax
            .values_rows_require_equal_arity
        {
            return Ok(());
        }
        let Some(width) = rows.first().map(ThinVec::len) else {
            return Ok(());
        };
        if let Some(bad_width) = rows.iter().map(ThinVec::len).find(|&w| w != width) {
            let found = format!("a row with {bad_width} columns after a row with {width}");
            return Err(self.error_at(
                span,
                "every VALUES row to have the same number of columns",
                found,
            ));
        }
        Ok(())
    }

    /// Parse one parenthesized `VALUES` row.
    fn parse_values_row(&mut self) -> ParseResult<ThinVec<ValuesItem<D::Ext>>> {
        self.expect_punct(Punctuation::LParen, "`(` to open the VALUES row")?;
        // DuckDB tolerates a trailing comma inside a row (`VALUES (1, 2,)`); the closer is
        // the row's `)`.
        let row = self.parse_comma_separated_trailing(Self::parse_values_item, |p| {
            p.peek_is_punct(Punctuation::RParen)
        })?;
        self.expect_punct(Punctuation::RParen, "`)` to close the VALUES row")?;
        Ok(row)
    }

    /// Parse one `VALUES` row item: a bare `DEFAULT` or an expression.
    ///
    /// `DEFAULT` is admitted as a row element to match PostgreSQL, which parses it to
    /// a `SetToDefault` node rather than a column reference. It is kept out of the
    /// expression grammar so a bare `DEFAULT` never aliases to a column — the same
    /// split the INSERT values path uses, reusing the [`DefaultValue`] leaf.
    fn parse_values_item(&mut self) -> ParseResult<ValuesItem<D::Ext>> {
        if self.peek_is_contextual_keyword("DEFAULT")? {
            let token = self
                .advance()?
                .expect("peek_is_contextual_keyword confirmed DEFAULT is present");
            let default = DefaultValue {
                meta: self.make_meta(token.span),
            };
            let meta = self.make_meta(token.span);
            Ok(ValuesItem::Default { default, meta })
        } else {
            let expr = self.parse_expr()?;
            let meta = self.make_meta(expr.span());
            Ok(ValuesItem::Expr { expr, meta })
        }
    }

    /// Parse a MySQL `HANDLER` low-level cursor statement into [`Statement::Handler`], reached
    /// under [`UtilitySyntax::handler_statements`](crate::ast::dialect::UtilitySyntax).
    ///
    /// The table name is read greedily as a dotted `ident [. ident …]`, then the verb selects
    /// the arity rule (`sql_yacc.yy` `handler_stmt`): `OPEN` admits a schema-qualified
    /// `table_ident` (`HANDLER db.t OPEN`), while `READ`/`CLOSE` take a bare unqualified
    /// `ident` — `HANDLER db.t {READ | CLOSE}` is `ER_PARSE_ERROR` on mysql:8, enforced here.
    pub(super) fn parse_handler_statement(&mut self) -> ParseResult<Statement<D::Ext>> {
        let start = self.current_span()?;
        self.expect_keyword(Keyword::Handler)?;

        let table_start = self.current_span()?;
        let mut parts = thin_vec![self.parse_ident()?];
        while self.eat_punct(Punctuation::Dot)? {
            parts.push(self.parse_ident()?);
        }
        let table = ObjectName(parts);
        let table_span = table_start.union(self.preceding_span());

        let op_start = self.current_span()?;
        let operation = if self.eat_keyword(Keyword::Open)? {
            // `OPEN` admits a schema-qualified `table_ident` (`db.t`); MySQL's grammar goes no
            // deeper than two parts, so a longer name is rejected.
            if table.0.len() > 2 {
                let found = self.span_text(table_span).to_owned();
                return Err(self.error_at(
                    table_span,
                    "a HANDLER OPEN table name of at most two parts (schema.table)",
                    found,
                ));
            }
            let (alias, as_keyword) = self.parse_handler_open_alias()?;
            HandlerOperation::Open {
                alias,
                as_keyword,
                meta: self.make_meta(op_start.union(self.preceding_span())),
            }
        } else if self.eat_keyword(Keyword::Close)? {
            self.require_unqualified_handler_table(&table, table_span)?;
            HandlerOperation::Close {
                meta: self.make_meta(op_start.union(self.preceding_span())),
            }
        } else if self.eat_keyword(Keyword::Read)? {
            self.require_unqualified_handler_table(&table, table_span)?;
            let selector = self.parse_handler_read_selector()?;
            let selection = if self.eat_keyword(Keyword::Where)? {
                Some(self.parse_expr()?)
            } else {
                None
            };
            let limit = self.parse_limit()?;
            HandlerOperation::Read {
                selector,
                selection,
                limit,
                meta: self.make_meta(op_start.union(self.preceding_span())),
            }
        } else {
            return Err(self.unexpected("`OPEN`, `READ`, or `CLOSE` after the HANDLER table"));
        };

        let span = start.union(self.preceding_span());
        let statement_meta = self.make_meta(span);
        Ok(Statement::Handler {
            handler: Box::new(HandlerStatement {
                table,
                operation,
                meta: self.make_meta(span),
            }),
            meta: statement_meta,
        })
    }

    /// Reject a schema-qualified table under the `HANDLER … {READ | CLOSE}` verbs, whose
    /// grammar takes a bare `ident` (only `OPEN` admits `db.t`).
    fn require_unqualified_handler_table(
        &mut self,
        table: &ObjectName,
        table_span: Span,
    ) -> ParseResult<()> {
        if table.0.len() > 1 {
            let found = self.span_text(table_span).to_owned();
            return Err(self.error_at(
                table_span,
                "an unqualified HANDLER table name (READ/CLOSE take a bare identifier)",
                found,
            ));
        }
        Ok(())
    }

    /// Parse the optional `[AS] <alias>` tail of `HANDLER <t> OPEN` (`sql_yacc.yy`
    /// `opt_table_alias`): a bare alias when a following identifier can start one, mandatory
    /// after an explicit `AS`, absent otherwise.
    fn parse_handler_open_alias(&mut self) -> ParseResult<(Option<crate::ast::Ident>, bool)> {
        if self.eat_keyword(Keyword::As)? {
            return Ok((Some(self.parse_bare_alias_ident()?), true));
        }
        if self.peek_can_start_bare_alias()? {
            Ok((Some(self.parse_bare_alias_ident()?), false))
        } else {
            Ok((None, false))
        }
    }

    /// Parse the selector of a `HANDLER <t> READ …` after the `READ` keyword (`sql_yacc.yy`
    /// `handler_scan_function` / `handler_rkey_function` / `handler_rkey_mode`): a bare
    /// `{FIRST | NEXT}` scan, a named-index `{FIRST | NEXT | PREV | LAST}` traversal, or a
    /// named-index `<op> (<values>)` key seek.
    fn parse_handler_read_selector(&mut self) -> ParseResult<HandlerReadSelector<D::Ext>> {
        let start = self.current_span()?;
        // Bare scan (no index named): `FIRST`/`NEXT` only — `PREV`/`LAST` require an index.
        if self.eat_keyword(Keyword::First)? {
            return Ok(HandlerReadSelector::Scan {
                direction: HandlerScanDirection::First,
                meta: self.make_meta(start.union(self.preceding_span())),
            });
        }
        if self.eat_keyword(Keyword::Next)? {
            return Ok(HandlerReadSelector::Scan {
                direction: HandlerScanDirection::Next,
                meta: self.make_meta(start.union(self.preceding_span())),
            });
        }
        // Otherwise a named index, followed by a traversal direction or a key comparison.
        let index = self.parse_ident()?;
        if let Some(direction) = self.eat_handler_index_direction()? {
            return Ok(HandlerReadSelector::Index {
                index,
                direction,
                meta: self.make_meta(start.union(self.preceding_span())),
            });
        }
        let comparison = self.parse_handler_key_comparison()?;
        self.expect_punct(
            Punctuation::LParen,
            "`(` to open the HANDLER key value list",
        )?;
        // MySQL's `values` is a non-empty `expr_or_default` list (reusing the INSERT
        // values-item grammar); an empty `()` and a trailing comma are both syntax errors,
        // which `parse_comma_separated` enforces by requiring at least one item and no dangling
        // separator.
        let key = self.parse_comma_separated(Self::parse_values_item)?;
        self.expect_punct(
            Punctuation::RParen,
            "`)` to close the HANDLER key value list",
        )?;
        Ok(HandlerReadSelector::Key {
            index,
            comparison,
            key,
            meta: self.make_meta(start.union(self.preceding_span())),
        })
    }

    /// Eat an indexed-read traversal direction (`FIRST`/`NEXT`/`PREV`/`LAST`), or `None` when
    /// the next token opens a key comparison instead.
    fn eat_handler_index_direction(&mut self) -> ParseResult<Option<HandlerIndexDirection>> {
        if self.eat_keyword(Keyword::First)? {
            Ok(Some(HandlerIndexDirection::First))
        } else if self.eat_keyword(Keyword::Next)? {
            Ok(Some(HandlerIndexDirection::Next))
        } else if self.eat_keyword(Keyword::Prev)? {
            Ok(Some(HandlerIndexDirection::Prev))
        } else if self.eat_keyword(Keyword::Last)? {
            Ok(Some(HandlerIndexDirection::Last))
        } else {
            Ok(None)
        }
    }

    /// Parse a `HANDLER … READ <index> <op> (…)` key comparison operator — `sql_yacc.yy`
    /// `handler_rkey_mode`, exactly `= >= <= > <`. The inequality operators `<>`/`!=` are
    /// `ER_PARSE_ERROR` on mysql:8 and are rejected here.
    fn parse_handler_key_comparison(&mut self) -> ParseResult<HandlerKeyComparison> {
        if self.eat_op(Operator::Eq)? {
            Ok(HandlerKeyComparison::Eq)
        } else if self.eat_op(Operator::GtEq)? {
            Ok(HandlerKeyComparison::GreaterOrEqual)
        } else if self.eat_op(Operator::LtEq)? {
            Ok(HandlerKeyComparison::LessOrEqual)
        } else if self.eat_op(Operator::Gt)? {
            Ok(HandlerKeyComparison::Greater)
        } else if self.eat_op(Operator::Lt)? {
            Ok(HandlerKeyComparison::Less)
        } else {
            Err(self.unexpected("a HANDLER key comparison operator (`=`, `>=`, `<=`, `>`, or `<`)"))
        }
    }

    /// Map the current token to a [`SetOperator`] keyword, without consuming it.
    fn peek_set_operator(&mut self) -> ParseResult<Option<SetOperator>> {
        if self.peek_is_keyword(Keyword::Union)? {
            Ok(Some(SetOperator::Union))
        } else if self.peek_is_keyword(Keyword::Intersect)? {
            Ok(Some(SetOperator::Intersect))
        } else if self.peek_is_keyword(Keyword::Except)? {
            Ok(Some(SetOperator::Except))
        } else {
            Ok(None)
        }
    }

    /// True if the current token can begin a query statement.
    ///
    /// `TABLE` opens the `<explicit table>` short form (`TABLE name`), a query primary
    /// like `SELECT`/`VALUES`, so it composes everywhere a query does — a statement, a
    /// parenthesized set operand, a scalar/`IN` subquery, a derived table, a CTE body.
    /// It is fully reserved, so (unlike `VALUES`) it never contends with a column name.
    ///
    /// A leading `FROM` opens DuckDB's FROM-first primary (`FROM t SELECT x`, bare
    /// `FROM t`) under [`SelectSyntax::from_first`](crate::ast::dialect::SelectSyntax);
    /// gating this one choke point makes the form reachable in every query position
    /// above at once. `FROM` is fully reserved, so it never contends with a column name;
    /// off the flag it is not a query start, so a statement-position `FROM` stays a clean
    /// parse error in the other dialects (the over-acceptance guard).
    pub(super) fn peek_starts_query(&mut self) -> ParseResult<bool> {
        Ok(self.peek_is_keyword(Keyword::Select)?
            || (self.features().select_syntax.explicit_table
                && self.peek_is_keyword(Keyword::Table)?)
            || self.peek_is_keyword(Keyword::Values)?
            || self.peek_is_keyword(Keyword::With)?
            || (self.features().select_syntax.from_first && self.peek_is_keyword(Keyword::From)?))
    }

    /// True if, just after consuming a `(` in an expression context, the operand is
    /// a scalar/`IN` subquery rather than a parenthesized expression or expression
    /// list.
    ///
    /// This is [`peek_starts_query`](Self::peek_starts_query) with one carve-out:
    /// `VALUES` is non-reserved as a column name, so `(values + 1)` and
    /// `x IN (values)` must parse as column references, not as the head of a
    /// `VALUES` row. A genuine `VALUES` constructor is always `VALUES (`, while a
    /// column `values` never is — it is reserved as a function name, so `values(…)`
    /// cannot be a call — so the trailing `(` disambiguates the two reserved uses
    /// with no loss to either, matching how `values` parses outside parentheses.
    /// `SELECT`/`WITH` are reserved and so are unconditional query starts.
    pub(super) fn peek_starts_subquery_in_parens(&mut self) -> ParseResult<bool> {
        Ok(self.peek_starts_query()?
            && (!self.peek_is_keyword(Keyword::Values)?
                || self.peek_nth_is_punct(1, Punctuation::LParen)?))
    }

    /// Parse the *query-tail* `ORDER BY` clause: DuckDB's `ALL [ASC | DESC]
    /// [NULLS FIRST | LAST]` clause mode, or the ordinary sort-key list via
    /// [`parse_order_by`](Self::parse_order_by). Exactly one of the pair is
    /// populated.
    ///
    /// The `ALL` branch lives here — not in `parse_order_by` — because only the
    /// query-level clause admits the mode: DuckDB rejects `ALL` in a window
    /// `ORDER BY` with a dedicated parse error, its aggregate-internal
    /// `agg(x ORDER BY ALL)` is a `COLUMNS(*)` star expansion (a different
    /// construct — a sort *key*, parsed by
    /// [`parse_aggregate_order_by`](Self::parse_aggregate_order_by)), and the MySQL
    /// DML sort tails have no `ALL`. The gate is on only where `ALL` is reserved, so
    /// the bare keyword can never open an ordinary sort expression (`"all"` tokenizes
    /// as a `QuotedIdent`), and DuckDB admits no second key (`ORDER BY ALL, x`
    /// syntax-errors; probed on 1.5.4) — consume the keyword and its modifiers and
    /// let anything trailing surface as the usual trailing-input error, matching
    /// the engine's reject.
    pub(super) fn parse_order_by_clause(&mut self) -> ParseResult<OrderByClause<D::Ext>> {
        if self.features().grouping_syntax.order_by_all
            && self.peek_is_keyword(Keyword::Order)?
            && self.peek_nth_is_keyword(1, Keyword::By)?
            && self.peek_nth_is_keyword(2, Keyword::All)?
        {
            self.advance()?; // ORDER
            self.advance()?; // BY
            let start = self.advance_span()?; // ALL
            let asc = self.parse_sort_direction()?;
            let nulls_first = self.parse_nulls_order()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok((
                ThinVec::new(),
                Some(Box::new(OrderByAll {
                    asc,
                    nulls_first,
                    meta,
                })),
            ));
        }
        Ok((self.parse_order_by_allowing_star()?, None))
    }

    /// Parse an optional `ORDER BY <item> [, …]`; empty when absent. The shared entry —
    /// window `OVER (ORDER BY …)` and the MySQL DML sort tails — never admits DuckDB's
    /// bare-`*` sort key (`ORDER BY *`), which the engine syntax-rejects in a window
    /// ("Cannot ORDER BY ALL in a window expression"; probed on 1.5.4); the query-tail
    /// and aggregate-internal callers route through
    /// [`parse_order_by_allowing_star`](Self::parse_order_by_allowing_star) instead.
    pub(super) fn parse_order_by(&mut self) -> ParseResult<ThinVec<OrderByExpr<D::Ext>>> {
        self.parse_order_by_items(false)
    }

    /// [`parse_order_by`](Self::parse_order_by) admitting DuckDB's bare-`*` star sort key
    /// (`ORDER BY *`, `ORDER BY * EXCLUDE (id)`) — the query-tail and aggregate-internal
    /// positions where the engine accepts it. The star surface itself is gated inside
    /// [`parse_star_or_expr`](Self::parse_star_or_expr), so under a dialect without the
    /// columns expression this is exactly [`parse_order_by`](Self::parse_order_by).
    pub(super) fn parse_order_by_allowing_star(
        &mut self,
    ) -> ParseResult<ThinVec<OrderByExpr<D::Ext>>> {
        self.parse_order_by_items(true)
    }

    fn parse_order_by_items(
        &mut self,
        allow_star: bool,
    ) -> ParseResult<ThinVec<OrderByExpr<D::Ext>>> {
        if !self.eat_keyword(Keyword::Order)? {
            return Ok(ThinVec::new());
        }
        self.expect_keyword(Keyword::By)?;
        self.parse_comma_separated(|parser| parser.parse_order_by_item(allow_star))
    }

    /// Parse the *function-internal* `ORDER BY` (an aggregate's argument-list tail and
    /// `WITHIN GROUP`): DuckDB additionally admits `ALL [ASC | DESC]
    /// [NULLS FIRST | LAST]` there, which the engine reads as a sole sort key over the
    /// whole-projection `COLUMNS(*)` star expansion — `agg(x ORDER BY ALL)` and
    /// `agg(x ORDER BY COLUMNS(*))` serialize identically, in both the plain and the
    /// `WITHIN GROUP` position (probed on 1.5.4) — so it parses to a sole
    /// [`OrderByExpr`] whose key is [`Expr::Columns`], gated with the node's own
    /// [`CallSyntax::columns_expression`](crate::ast::dialect::ExpressionSyntax)
    /// gate. DuckDB admits no sibling key (`agg(x ORDER BY ALL, y)` / `ORDER BY y,
    /// ALL` both syntax-error), so the branch consumes only the keyword and its
    /// modifiers: a trailing `,` surfaces as the caller's close-paren error, and a
    /// non-first `ALL` falls into the expression grammar where the reserved keyword
    /// rejects — both matching the engine. The window `OVER (ORDER BY …)` position
    /// deliberately keeps calling [`parse_order_by`](Self::parse_order_by): DuckDB
    /// rejects `ALL` there with a dedicated error ("Cannot ORDER BY ALL in a window
    /// expression").
    pub(super) fn parse_aggregate_order_by(&mut self) -> ParseResult<ThinVec<OrderByExpr<D::Ext>>> {
        if self.features().call_syntax.columns_expression
            && self.peek_is_keyword(Keyword::Order)?
            && self.peek_nth_is_keyword(1, Keyword::By)?
            && self.peek_nth_is_keyword(2, Keyword::All)?
        {
            self.advance()?; // ORDER
            self.advance()?; // BY
            let start = self.advance_span()?; // ALL
            let columns_meta = self.make_meta(start);
            let expr = Expr::Columns {
                qualifier: None,
                pattern: None,
                options: None,
                spelling: ColumnsSpelling::Columns,
                meta: columns_meta,
            };
            let asc = self.parse_sort_direction()?;
            let nulls_first = self.parse_nulls_order()?;
            let meta = self.make_meta(start.union(self.preceding_span()));
            return Ok(thin_vec![OrderByExpr {
                expr,
                asc,
                using: None,
                nulls_first,
                meta,
            }]);
        }
        self.parse_order_by_allowing_star()
    }

    /// Parse an optional `ASC` / `DESC` direction; `None` when unwritten.
    fn parse_sort_direction(&mut self) -> ParseResult<Option<bool>> {
        Ok(if self.eat_keyword(Keyword::Asc)? {
            Some(true)
        } else if self.eat_keyword(Keyword::Desc)? {
            Some(false)
        } else {
            None
        })
    }

    /// Parse an optional `NULLS FIRST` / `NULLS LAST` tail; `None` when unwritten.
    fn parse_nulls_order(&mut self) -> ParseResult<Option<bool>> {
        if !self.eat_keyword(Keyword::Nulls)? {
            return Ok(None);
        }
        if self.eat_keyword(Keyword::First)? {
            Ok(Some(true))
        } else if self.eat_keyword(Keyword::Last)? {
            Ok(Some(false))
        } else {
            Err(self.unexpected("`FIRST` or `LAST`"))
        }
    }

    /// Parse one `ORDER BY` key: `<expr> [ASC|DESC | USING <op>] [NULLS FIRST|LAST]`.
    ///
    /// `asc`/`using`/`nulls_first` stay `None` when the modifier is unwritten, so the
    /// node records exactly what the source said and leaves the default ordering to
    /// the consumer (which is dialect-dependent). `USING <operator>` (PostgreSQL) is
    /// an operator-driven alternative to `ASC`/`DESC` — the two are mutually exclusive
    /// in `gram.y`'s `sortby`, so a written `USING` leaves `asc` unset.
    fn parse_order_by_item(&mut self, allow_star: bool) -> ParseResult<OrderByExpr<D::Ext>> {
        let start = self.current_span()?;
        let expr = if allow_star {
            self.parse_star_or_expr()?
        } else {
            self.parse_expr()?
        };
        let (asc, using) = if self.features().grouping_syntax.order_by_using
            && self.eat_keyword(Keyword::Using)?
        {
            (None, Some(Box::new(self.parse_order_by_using_operator()?)))
        } else {
            (self.parse_sort_direction()?, None)
        };
        let nulls_first = self.parse_nulls_order()?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(OrderByExpr {
            expr,
            asc,
            using,
            nulls_first,
            meta,
        })
    }

    /// Parse the `qual_all_Op` operator after `ORDER BY <expr> USING` (the `USING`
    /// keyword is already consumed): a bare symbolic operator (`<`, `~<~`) or the
    /// schema-qualified `OPERATOR(schema.op)` form (PostgreSQL `any_operator`).
    fn parse_order_by_using_operator(&mut self) -> ParseResult<OrderByUsing> {
        let start = self.current_span()?;
        let (schema, op) = if self.peek_is_keyword(Keyword::Operator)? {
            self.advance()?; // `OPERATOR`
            self.expect_punct(Punctuation::LParen, "`(` after `OPERATOR`")?;
            // Optional `(ColId '.')*` schema qualification: a part is taken only when a
            // name is immediately followed by `.`, so the operator symbol ends the chain.
            let mut parts = ThinVec::new();
            while self
                .peek()?
                .is_some_and(|token| self.token_can_be_column_name(token))
                && self.peek_nth_is_punct(1, Punctuation::Dot)?
            {
                parts.push(self.parse_ident()?);
                self.expect_punct(Punctuation::Dot, "`.` in the qualified operator name")?;
            }
            let op = self.parse_using_operator_symbol()?;
            self.expect_punct(Punctuation::RParen, "`)` to close `OPERATOR(...)`")?;
            // An unqualified `OPERATOR(<)` carries no schema node at all — an empty
            // `ObjectName` would have no source span (the span walker rejects it).
            let schema = (!parts.is_empty()).then(|| ObjectName(parts));
            (schema, op)
        } else {
            (None, self.parse_using_operator_symbol()?)
        };
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(OrderByUsing { schema, op, meta })
    }

    /// Intern a contiguous run of operator tokens (`<`, `<=`, `~<~`) as the sort
    /// operator symbol, preserving its exact spelling; at least one is required.
    fn parse_using_operator_symbol(&mut self) -> ParseResult<Symbol> {
        let start = self.current_span()?;
        if !matches!(
            self.peek()?.map(|token| token.kind),
            Some(TokenKind::Operator(_))
        ) {
            return Err(self.unexpected("an operator after `USING`"));
        }
        while matches!(
            self.peek()?.map(|token| token.kind),
            Some(TokenKind::Operator(_))
        ) {
            self.advance()?;
        }
        let span = start.union(self.preceding_span());
        let text = self.span_text(span);
        Ok(self.intern_text(text))
    }

    /// Parse an optional `LIMIT`/`OFFSET`/`FETCH` tail; `None` when absent.
    ///
    /// `LIMIT <count> [OFFSET <start>]` and `OFFSET <start> [LIMIT <count>]` fold
    /// into the canonical [`Limit`] tagged [`LimitSyntax::LimitOffset`]:
    /// the surface order is not load-bearing once both counts are captured. The
    /// SQL:2008 spelling — `OFFSET <start> { ROW | ROWS }` and
    /// `FETCH { FIRST | NEXT } <count> { ROW | ROWS } ONLY` — folds into the same
    /// shape tagged [`LimitSyntax::FetchFirst`], distinguished from the
    /// PostgreSQL `OFFSET` by the trailing `ROW`/`ROWS` noise word. The fetch-first
    /// spelling is gated by [`QueryTailSyntax::fetch_first`](crate::ast::dialect::SelectSyntax).
    ///
    /// The MySQL `LIMIT <offset>, <count>` comma form folds into the same
    /// [`LimitSyntax::LimitOffset`] shape (offset first, count second) and is gated
    /// by [`QueryTailSyntax::limit_offset_comma`](crate::ast::dialect::SelectSyntax).
    /// Parse a `LIMIT`/`OFFSET` row-count operand. Most dialects admit an arbitrary
    /// expression, but MySQL restricts the count to an unsigned integer literal or a `?`
    /// placeholder ([`QueryTailSyntax::limit_expressions`](crate::ast::dialect::QueryTailSyntax::limit_expressions) off). The whole operand is parsed,
    /// then rejected on shape when the flag is off and it is neither — so `LIMIT 1 + 1`
    /// (a binary op) and `LIMIT (SELECT 1)` (a subquery) are the syntax error MySQL reports
    /// (engine-measured-rejected on mysql:8), the diagnostic pointing at the operand span,
    /// while `LIMIT <int>` / `LIMIT ?` / the comma / `OFFSET <int>` forms stay accepted.
    fn parse_limit_operand(&mut self) -> ParseResult<Expr<D::Ext>> {
        let start = self.current_span()?;
        let expr = self.parse_expr()?;
        if !self.features().query_tail_syntax.limit_expressions
            && !matches!(
                expr,
                Expr::Literal {
                    literal: Literal {
                        kind: LiteralKind::Integer,
                        ..
                    },
                    ..
                } | Expr::Parameter { .. }
            )
        {
            let span = start.union(self.preceding_span());
            let found = self.span_text(span).to_owned();
            return Err(self.error_at(
                span,
                "an integer literal or `?` placeholder as the `LIMIT`/`OFFSET` count",
                found,
            ));
        }
        Ok(expr)
    }

    /// Parse an optional ClickHouse `LIMIT n [OFFSET m] BY expr, …` clause
    /// ([`LimitBy`]); `None` when the leading `LIMIT` is the ordinary tail instead.
    ///
    /// Gated by [`QueryTailSyntax::limit_by_clause`](crate::ast::dialect::SelectSyntax) —
    /// off for every shipped preset but Lenient. `LIMIT` opens both this clause and the
    /// ordinary [`parse_limit`](Self::parse_limit) tail, so the two are told apart by a
    /// trailing `BY`: read `LIMIT <count> [OFFSET <skip>]` speculatively and commit only
    /// when a `BY` follows. Any other continuation — a bare `LIMIT n`, `LIMIT n OFFSET m`
    /// with no `BY`, or an operand that fails to parse — rewinds to before the `LIMIT`
    /// and returns `None`, leaving the token for `parse_limit`. This ordering matches
    /// ClickHouse, where `LIMIT BY` precedes the final `LIMIT` and both may appear.
    pub(super) fn parse_limit_by(&mut self) -> ParseResult<Option<Box<LimitBy<D::Ext>>>> {
        if !self.features().query_tail_syntax.limit_by_clause {
            return Ok(None);
        }
        if !self.peek_is_keyword(Keyword::Limit)? {
            return Ok(None);
        }
        let checkpoint = self.checkpoint();
        let start = self.current_span()?;
        self.expect_keyword(Keyword::Limit)?;
        // Speculative head: a count and an optional `OFFSET m`. A failure here means the
        // token is the ordinary `LIMIT` tail (`parse_limit` re-reads it and reports the
        // real diagnostic), so rewind rather than error.
        let Ok(limit) = self.parse_limit_operand() else {
            self.rewind(checkpoint);
            return Ok(None);
        };
        let offset = if self.eat_keyword(Keyword::Offset)? {
            match self.parse_limit_operand() {
                Ok(offset) => Some(offset),
                Err(_) => {
                    self.rewind(checkpoint);
                    return Ok(None);
                }
            }
        } else {
            None
        };
        if !self.eat_keyword(Keyword::By)? {
            // No `BY`: an ordinary `LIMIT n` / `LIMIT n OFFSET m`. Hand it back untouched.
            self.rewind(checkpoint);
            return Ok(None);
        }
        // Past `BY` the clause is committed — a malformed grouping list is a real error.
        let by = self.parse_comma_separated(Self::parse_expr)?;
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Ok(Some(Box::new(LimitBy {
            limit,
            offset,
            by,
            meta,
        })))
    }

    /// Parse an optional ClickHouse `SETTINGS name = value, …` query tail
    /// ([`Query::settings`](crate::ast::Query::settings)); empty when absent.
    ///
    /// Gated by [`QueryTailSyntax::settings_clause`](crate::ast::dialect::SelectSyntax) —
    /// on for Lenient only. `SETTINGS` is a contextual keyword (unreserved everywhere,
    /// so it stays an ordinary identifier off this position), matched by spelling; with
    /// the gate off a trailing `SETTINGS …` is left unconsumed and rejected as trailing
    /// input, the reject mechanism the other query-tail gates use.
    pub(super) fn parse_settings(&mut self) -> ParseResult<ThinVec<Setting<D::Ext>>> {
        if !self.features().query_tail_syntax.settings_clause {
            return Ok(ThinVec::new());
        }
        if !self.eat_contextual_keyword("SETTINGS")? {
            return Ok(ThinVec::new());
        }
        self.parse_comma_separated(Self::parse_setting)
    }

    /// Parse one `name = value` setting pair. The value is a general expression (the
    /// `SecretOption` precedent); ClickHouse writes a literal, the wider grammar is the
    /// recorded acceptance bound.
    fn parse_setting(&mut self) -> ParseResult<Setting<D::Ext>> {
        let start = self.current_span()?;
        let name = self.parse_ident()?;
        if !self.eat_op(Operator::Eq)? {
            return Err(self.unexpected("`=` in a SETTINGS pair"));
        }
        let value = self.parse_expr()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Setting { name, value, meta })
    }

    /// Parse an optional ClickHouse `FORMAT <name>` query tail
    /// ([`Query::format`](crate::ast::Query::format)); `None` when absent.
    ///
    /// Gated by [`QueryTailSyntax::format_clause`](crate::ast::dialect::SelectSyntax) — on
    /// for Lenient only. `FORMAT` is a contextual keyword (unreserved everywhere, so it
    /// stays an ordinary identifier off this position), matched by spelling; the format
    /// name is a bare, case-sensitive identifier (`JSON`, `TabSeparated`, `Null`) read as
    /// an [`Ident`](crate::ast::Ident) rather than a string literal. With the gate off a
    /// trailing `FORMAT …` is left unconsumed and rejected as trailing input, the reject
    /// mechanism the other query-tail gates use.
    pub(super) fn parse_format(&mut self) -> ParseResult<Option<Box<FormatClause>>> {
        if !self.features().query_tail_syntax.format_clause {
            return Ok(None);
        }
        let start = self.current_span()?;
        if !self.eat_contextual_keyword("FORMAT")? {
            return Ok(None);
        }
        let name = self.parse_format_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(Some(Box::new(FormatClause { name, meta })))
    }

    /// True if a `FOR XML`/`FOR JSON` result-shaping tail leads at the cursor, under a
    /// preset that spells it ([`QueryTailSyntax::for_xml_json_clause`](crate::ast::dialect::SelectSyntax)).
    ///
    /// The disambiguation predicate `FOR`-led clauses partition on: `FOR` followed by
    /// `XML`/`JSON` is this clause; `FOR` followed by `UPDATE`/`SHARE`/`NO`/`KEY` is a
    /// locking clause. Read by both `parse_for_clause` (to claim its `FOR`) and
    /// `parse_optional_locking_clause` (to decline it), so the two never race for the
    /// shared `FOR` lead. `XML`/`JSON` are matched by spelling (contextual), so the
    /// predicate holds whether or not the tokenizer classified them as keywords.
    fn for_clause_leads(&mut self) -> ParseResult<bool> {
        Ok(self.features().query_tail_syntax.for_xml_json_clause
            && self.peek_is_keyword(Keyword::For)?
            && (self.peek_nth_is_contextual_keyword(1, "XML")?
                || self.peek_nth_is_contextual_keyword(1, "JSON")?))
    }

    /// Parse an optional MSSQL `FOR XML …` / `FOR JSON …` result-shaping tail
    /// ([`Query::for_clause`](crate::ast::Query::for_clause)); `None` when absent.
    ///
    /// Gated by [`QueryTailSyntax::for_xml_json_clause`](crate::ast::dialect::SelectSyntax)
    /// — on for MSSQL and Lenient. Consumes the `FOR` only when the follow token is
    /// `XML`/`JSON` (via [`for_clause_leads`](Self::for_clause_leads)), so a `FOR UPDATE`/
    /// `FOR SHARE` locking clause is left for `parse_locking_clauses`. The directives
    /// (`ELEMENTS`, `BINARY BASE64`, `TYPE`, `ROOT`, `INCLUDE_NULL_VALUES`,
    /// `WITHOUT_ARRAY_WRAPPER`) are accepted order-independently — the mode selector is
    /// required, the rest are an optional comma-separated tail; there being no MSSQL
    /// oracle, the accepted grammar is the recorded acceptance bound.
    pub(super) fn parse_for_clause(&mut self) -> ParseResult<Option<Box<ForClause>>> {
        if !self.for_clause_leads()? {
            return Ok(None);
        }
        let start = self.current_span()?;
        self.expect_keyword(Keyword::For)?;
        let clause = if self.eat_contextual_keyword("XML")? {
            self.parse_for_xml(start)?
        } else {
            // `for_clause_leads` guaranteed the follow token is `XML` or `JSON`.
            self.expect_contextual_keyword("JSON")?;
            self.parse_for_json(start)?
        };
        Ok(Some(Box::new(clause)))
    }

    /// Parse the body after `FOR XML`: the mode selector then the optional directive
    /// tail. `start` spans from the `FOR` keyword.
    fn parse_for_xml(&mut self, start: Span) -> ParseResult<ForClause> {
        let mode = self.parse_for_xml_mode()?;
        let mut elements = None;
        let mut binary_base64 = false;
        let mut typed = false;
        let mut root = None;
        while self.eat_punct(Punctuation::Comma)? {
            if self.eat_contextual_keyword("BINARY")? {
                self.expect_contextual_keyword("BASE64")?;
                binary_base64 = true;
            } else if self.eat_contextual_keyword("TYPE")? {
                typed = true;
            } else if self.eat_contextual_keyword("ROOT")? {
                root = Some(self.parse_for_root()?);
            } else if self.eat_contextual_keyword("ELEMENTS")? {
                elements = Some(self.parse_for_xml_elements()?);
            } else {
                return Err(self.unexpected(
                    "`BINARY BASE64`, `TYPE`, `ROOT`, or `ELEMENTS` after `,` in a FOR XML clause",
                ));
            }
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ForClause::Xml {
            mode,
            elements,
            binary_base64,
            typed,
            root,
            meta,
        })
    }

    /// Parse the `RAW ['name'] | AUTO | EXPLICIT | PATH ['name']` selector after
    /// `FOR XML`. All four are matched by spelling (contextual), so `PATH` — a keyword —
    /// reads the same as the unreserved `RAW`/`AUTO`/`EXPLICIT`.
    fn parse_for_xml_mode(&mut self) -> ParseResult<ForXmlMode> {
        let start = self.current_span()?;
        let mode = if self.eat_contextual_keyword("RAW")? {
            let name = self.parse_optional_for_name()?;
            ForXmlMode::Raw {
                name,
                meta: self.make_meta(start.union(self.preceding_span())),
            }
        } else if self.eat_contextual_keyword("AUTO")? {
            ForXmlMode::Auto {
                meta: self.make_meta(start.union(self.preceding_span())),
            }
        } else if self.eat_contextual_keyword("EXPLICIT")? {
            ForXmlMode::Explicit {
                meta: self.make_meta(start.union(self.preceding_span())),
            }
        } else if self.eat_contextual_keyword("PATH")? {
            let name = self.parse_optional_for_name()?;
            ForXmlMode::Path {
                name,
                meta: self.make_meta(start.union(self.preceding_span())),
            }
        } else {
            return Err(self.unexpected("`RAW`, `AUTO`, `EXPLICIT`, or `PATH` after `FOR XML`"));
        };
        Ok(mode)
    }

    /// Parse the `[XSINIL | ABSENT]` refinement after `ELEMENTS`; a bare `ELEMENTS`
    /// is [`ForXmlElements::Plain`].
    fn parse_for_xml_elements(&mut self) -> ParseResult<ForXmlElements> {
        if self.eat_contextual_keyword("XSINIL")? {
            Ok(ForXmlElements::XsiNil)
        } else if self.eat_contextual_keyword("ABSENT")? {
            Ok(ForXmlElements::Absent)
        } else {
            Ok(ForXmlElements::Plain)
        }
    }

    /// Parse the body after `FOR JSON`: the `AUTO | PATH` mode then the optional
    /// directive tail. `start` spans from the `FOR` keyword.
    fn parse_for_json(&mut self, start: Span) -> ParseResult<ForClause> {
        let mode = if self.eat_contextual_keyword("AUTO")? {
            ForJsonMode::Auto
        } else if self.eat_contextual_keyword("PATH")? {
            ForJsonMode::Path
        } else {
            return Err(self.unexpected("`AUTO` or `PATH` after `FOR JSON`"));
        };
        let mut root = None;
        let mut include_null_values = false;
        let mut without_array_wrapper = false;
        while self.eat_punct(Punctuation::Comma)? {
            if self.eat_contextual_keyword("ROOT")? {
                root = Some(self.parse_for_root()?);
            } else if self.eat_contextual_keyword("INCLUDE_NULL_VALUES")? {
                include_null_values = true;
            } else if self.eat_contextual_keyword("WITHOUT_ARRAY_WRAPPER")? {
                without_array_wrapper = true;
            } else {
                return Err(self.unexpected(
                    "`ROOT`, `INCLUDE_NULL_VALUES`, or `WITHOUT_ARRAY_WRAPPER` after `,` in a FOR JSON clause",
                ));
            }
        }
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ForClause::Json {
            mode,
            root,
            include_null_values,
            without_array_wrapper,
            meta,
        })
    }

    /// Parse a `ROOT ['name']` directive (the `ROOT` keyword is already consumed).
    fn parse_for_root(&mut self) -> ParseResult<ForRoot> {
        let start = self.preceding_span();
        let name = self.parse_optional_for_name()?;
        let meta = self.make_meta(start.union(self.preceding_span()));
        Ok(ForRoot { name, meta })
    }

    /// Parse an optional `('name')` quoted-string element/root name; `None` when no
    /// `(` follows.
    fn parse_optional_for_name(&mut self) -> ParseResult<Option<Literal>> {
        if !self.eat_punct(Punctuation::LParen)? {
            return Ok(None);
        }
        let name = self.expect_string_literal("a quoted name inside `( … )`")?;
        self.expect_punct(Punctuation::RParen, "`)` to close the FOR clause name")?;
        Ok(Some(name))
    }

    pub(super) fn parse_limit(&mut self) -> ParseResult<Option<Limit<D::Ext>>> {
        let start = self.current_span()?;
        if self.eat_keyword(Keyword::Limit)? {
            let (first, percent) = self.parse_limit_count()?;
            // A percentage count (`LIMIT 40 PERCENT` / `LIMIT 35%`) never takes the MySQL
            // comma form (DuckDB rejects `LIMIT 40 PERCENT, 2`); it may still carry an
            // `OFFSET` tail. The comma branch is otherwise reachable only where the
            // gate is on, which no percent-enabled dialect sets.
            if percent.is_none()
                && self.features().query_tail_syntax.limit_offset_comma
                && self.eat_punct(Punctuation::Comma)?
            {
                // MySQL/MariaDB/SQLite `LIMIT <offset>, <count>`: the comma form binds the
                // offset *first* and the count second — the reverse of
                // `LIMIT <count> OFFSET <offset>` — so the two arguments cannot be read
                // positionally without the dialect gate. It is the same row limit, so it
                // folds into the one canonical `Limit` shape (ADR-0011); the
                // `CommaOffset` spelling tag lets a source-fidelity render replay the
                // comma form. When the gate is off the comma is left unconsumed and
                // surfaces as a trailing-input parse error (how ANSI/PostgreSQL reject it).
                let count = self.parse_limit_operand()?;
                return Ok(Some(self.finish_limit(
                    start,
                    Some(count),
                    Some(first),
                    LimitSyntax::CommaOffset,
                    None,
                    None,
                )));
            }
            let limit = Some(first);
            let offset = if self.eat_keyword(Keyword::Offset)? {
                Some(self.parse_limit_operand()?)
            } else {
                None
            };
            return Ok(Some(self.finish_limit(
                start,
                limit,
                offset,
                LimitSyntax::LimitOffset,
                None,
                percent,
            )));
        }
        if self.features().query_tail_syntax.leading_offset && self.eat_keyword(Keyword::Offset)? {
            let offset = Some(self.parse_limit_operand()?);
            // A trailing `ROW`/`ROWS` selects the SQL:2008 offset/fetch spelling
            // (`OFFSET <n> ROWS [FETCH …]`); a bare `OFFSET <n>` is the PostgreSQL
            // spelling, where a trailing `LIMIT` may still follow.
            if self.features().query_tail_syntax.fetch_first {
                // The `OFFSET … ROW`/`ROWS` word is read but its (rare) singular
                // spelling is not tagged — the canonical plural round-trips the corpus.
                if self.eat_row_or_rows()?.is_some() {
                    // `with_ties: Some(_)` marks that a `FETCH` tail was actually
                    // written (as opposed to a bare `OFFSET … ROWS`) — see `Limit`'s
                    // doc comment: with the count itself optional, `limit: None` alone
                    // cannot tell the two apart, and they bound different result sets.
                    let (limit, with_ties, fetch_spelling) =
                        if self.peek_is_keyword(Keyword::Fetch)? {
                            let (count, with_ties, spelling) = self.parse_fetch_first()?;
                            (count, Some(with_ties), spelling)
                        } else {
                            (None, None, FetchSpelling::FirstRows)
                        };
                    let mut limit = self.finish_limit(
                        start,
                        limit,
                        offset,
                        LimitSyntax::FetchFirst,
                        with_ties,
                        None,
                    );
                    limit.fetch_spelling = fetch_spelling;
                    return Ok(Some(limit));
                }
            }
            let limit = if self.eat_keyword(Keyword::Limit)? {
                Some(self.parse_limit_operand()?)
            } else {
                None
            };
            return Ok(Some(self.finish_limit(
                start,
                limit,
                offset,
                LimitSyntax::LimitOffset,
                None,
                None,
            )));
        }
        if self.features().query_tail_syntax.fetch_first && self.peek_is_keyword(Keyword::Fetch)? {
            let (limit, with_ties, fetch_spelling) = self.parse_fetch_first()?;
            let mut limit = self.finish_limit(
                start,
                limit,
                None,
                LimitSyntax::FetchFirst,
                Some(with_ties),
                None,
            );
            limit.fetch_spelling = fetch_spelling;
            return Ok(Some(limit));
        }
        Ok(None)
    }

    /// Parse a `LIMIT` row count and its optional DuckDB percentage marker
    /// (`LIMIT 40 PERCENT` / `LIMIT 35%`), returning the count expression and the
    /// marker spelling (`None` for an ordinary row count).
    ///
    /// DuckDB's two percentage markers take *different* operand grammars (both verified
    /// on 1.5.4). The `PERCENT` keyword folds onto a bare numeric literal only: `LIMIT a
    /// PERCENT`, `LIMIT (1 + 1) PERCENT`, and `LIMIT NULL PERCENT` are all parser errors.
    /// The `%` operator folds onto any *primary-with-postfix* operand — a constant,
    /// parenthesized expression, function call, subquery, cast, or unary-signed value
    /// (`LIMIT (30-10) %`, `LIMIT RANDOM() %`, `LIMIT ?::VARCHAR %`) — but not a bare
    /// binary-infix expression (`LIMIT 1+2 %` is a parser error, the `%` shifting as a
    /// right-operand-less modulo). A `%` *with* a right operand is ordinary modulo
    /// (`LIMIT 10 % 3` is `LIMIT 1`). So the count is parsed as a full operand first —
    /// that path handles modulo, plain counts, and (for the `PERCENT` keyword) leaves the
    /// keyword for the caller — and the one shape it cannot reach, a trailing boundary
    /// `%` after a primary-with-postfix operand, is re-read on the rewind
    /// ([`parse_limit_percent_operand`](Self::parse_limit_percent_operand)). A `%` count
    /// whose operand carries a top-level binary operator (`LIMIT (5*2) %`) still stays a
    /// coverage gap — the restricted rewind operand stops before every infix — not a
    /// divergence.
    fn parse_limit_count(&mut self) -> ParseResult<(Expr<D::Ext>, Option<LimitPercent>)> {
        if !self.features().query_tail_syntax.limit_percent {
            return Ok((self.parse_limit_operand()?, None));
        }
        let checkpoint = self.checkpoint();
        match self.parse_limit_operand() {
            Ok(value) => {
                let percent =
                    if Self::is_numeric_literal(&value) && self.eat_keyword(Keyword::Percent)? {
                        Some(LimitPercent::Keyword)
                    } else {
                        None
                    };
                Ok((value, percent))
            }
            // The operand parse consumed a trailing `%` as modulo and found no right
            // operand — the one failure that is a valid DuckDB percent count. Re-read a
            // bare numeric literal followed by that `%`; anything else is the real error.
            Err(err) => {
                self.rewind(checkpoint);
                match self.parse_percent_symbol_count()? {
                    Some(value) => Ok((value, Some(LimitPercent::Symbol))),
                    None => {
                        self.rewind(checkpoint);
                        Err(err)
                    }
                }
            }
        }
    }

    /// Re-read the `<operand> %` percent count on the rewind path: a primary-with-postfix
    /// operand ([`parse_limit_percent_operand`](Self::parse_limit_percent_operand))
    /// directly followed by the `%` operator. Returns `None` (leaving the cursor for the
    /// caller to restore) when the shape does not match, so the caller can surface the
    /// original operand-parse error instead.
    fn parse_percent_symbol_count(&mut self) -> ParseResult<Option<Expr<D::Ext>>> {
        let Ok(value) = self.parse_limit_percent_operand() else {
            return Ok(None);
        };
        if !self.eat_op(Operator::Percent)? {
            return Ok(None);
        }
        Ok(Some(value))
    }

    /// Consume a `ROW` or `ROWS` noise word (interchangeable in the SQL:2008
    /// offset/fetch spelling); report which was present: `Some(true)` for the singular
    /// `ROW`, `Some(false)` for the plural `ROWS`, `None` when neither follows.
    fn eat_row_or_rows(&mut self) -> ParseResult<Option<bool>> {
        if self.eat_keyword(Keyword::Row)? {
            Ok(Some(true))
        } else if self.eat_keyword(Keyword::Rows)? {
            Ok(Some(false))
        } else {
            Ok(None)
        }
    }

    /// Parse `FETCH { FIRST | NEXT } [<count>] { ROW | ROWS } { ONLY | WITH TIES
    /// }`, returning the optional `<count>` row-limit expression and whether `WITH
    /// TIES` (rather than the default `ONLY`) was written.
    ///
    /// `FIRST`/`NEXT` and `ROW`/`ROWS` are interchangeable surface noise; the
    /// spelling tags on [`Limit`] record which the source wrote so a source-fidelity
    /// render replays it, while the canonical render re-emits `FETCH FIRST [<count>]
    /// ROWS { ONLY | WITH TIES }`. The count is genuinely optional in `gram.y`
    /// (`FETCH first_or_next row_or_rows ONLY`, PostgreSQL defaults it to 1) — `None`
    /// here means the source wrote no count, matching every other [`Limit`] field's
    /// "unwritten stays `None`" convention rather than synthesizing the implicit `1`.
    ///
    /// Returns `(count, with_ties, fetch_spelling)`: `with_ties` is `true` for the
    /// `WITH TIES` tail, and `fetch_spelling` folds the written `FIRST`/`NEXT` and
    /// `ROW`/`ROWS` synonyms for the source-fidelity render.
    fn parse_fetch_first(&mut self) -> ParseResult<FetchFirstClause<D::Ext>> {
        self.expect_keyword(Keyword::Fetch)?;
        let next = if self.eat_keyword(Keyword::First)? {
            false
        } else if self.eat_keyword(Keyword::Next)? {
            true
        } else {
            return Err(self.unexpected("`FIRST` or `NEXT`"));
        };
        let count = if self.peek_is_keyword(Keyword::Row)? || self.peek_is_keyword(Keyword::Rows)? {
            None
        } else {
            Some(self.parse_expr()?)
        };
        let Some(row_singular) = self.eat_row_or_rows()? else {
            return Err(self.unexpected("`ROW` or `ROWS`"));
        };
        let spelling = FetchSpelling::from_axes(next, row_singular);
        if self.eat_keyword(Keyword::Only)? {
            Ok((count, false, spelling))
        } else if self.eat_keyword(Keyword::With)? {
            self.expect_keyword(Keyword::Ties)?;
            Ok((count, true, spelling))
        } else {
            Err(self.unexpected("`ONLY` or `WITH TIES`"))
        }
    }

    /// Assemble a [`Limit`] spanning from `start` to the last consumed token.
    #[allow(clippy::too_many_arguments)]
    fn finish_limit(
        &mut self,
        start: Span,
        limit: Option<Expr<D::Ext>>,
        offset: Option<Expr<D::Ext>>,
        syntax: LimitSyntax,
        with_ties: Option<bool>,
        percent: Option<LimitPercent>,
    ) -> Limit<D::Ext> {
        let span = start.union(self.preceding_span());
        let meta = self.make_meta(span);
        Limit {
            limit,
            offset,
            syntax,
            with_ties,
            percent,
            // Defaults to the canonical `FETCH FIRST … ROWS`; the two SQL:2008
            // offset/fetch call sites overwrite it from the written source spelling.
            fetch_spelling: FetchSpelling::FirstRows,
            meta,
        }
    }

    /// Whether `expr` is a bare numeric-literal count — the only shape DuckDB folds the
    /// `PERCENT` *keyword* marker onto (`LIMIT a PERCENT` and `LIMIT NULL PERCENT` are
    /// DuckDB syntax errors). The `%` *operator* marker takes the wider primary-with-
    /// postfix operand ([`parse_limit_percent_operand`](Self::parse_limit_percent_operand)).
    fn is_numeric_literal(expr: &Expr<D::Ext>) -> bool {
        matches!(
            expr,
            Expr::Literal {
                literal: Literal {
                    kind: LiteralKind::Integer | LiteralKind::Float | LiteralKind::Decimal,
                    ..
                },
                ..
            }
        )
    }

    // --- Shared lexical predicates -----------------------------------------
    //
    // Keyword, punctuation, and operator matching are tag checks. Each `eat_*`
    // consumes only on a match and reports whether it did; each `expect_*`
    // consumes or yields a precise error at the offending token. They are
    // `pub(super)` so every clause helper in the query family — here, `select`,
    // and `from` — shares one set.

    /// True if the current token is the given keyword.
    pub(super) fn peek_is_keyword(&mut self, keyword: Keyword) -> ParseResult<bool> {
        Ok(matches!(
            self.peek()?,
            Some(token) if token.kind == TokenKind::Keyword(keyword)
        ))
    }

    /// True if the token `n` positions ahead is the given keyword.
    pub(super) fn peek_nth_is_keyword(&mut self, n: usize, keyword: Keyword) -> ParseResult<bool> {
        Ok(matches!(
            self.peek_nth(n)?.map(|token| token.kind),
            Some(TokenKind::Keyword(found)) if found == keyword
        ))
    }

    /// Consume the current token if it is `keyword`; report whether it was.
    pub(super) fn eat_keyword(&mut self, keyword: Keyword) -> ParseResult<bool> {
        Ok(if self.peek_is_keyword(keyword)? {
            self.advance()?;
            true
        } else {
            false
        })
    }

    /// Consume `keyword` or report a precise error at the offending token.
    pub(super) fn expect_keyword(&mut self, keyword: Keyword) -> ParseResult<()> {
        if self.eat_keyword(keyword)? {
            Ok(())
        } else {
            // Build the expectation from the keyword itself: ``"`BY`"`` etc.
            Err(self.unexpected(format!("`{}`", keyword.as_str().to_ascii_uppercase())))
        }
    }

    /// Consume the current token if its spelling is `expected`, regardless of
    /// whether the tokenizer classified it as a keyword or a plain word.
    pub(super) fn eat_contextual_keyword(&mut self, expected: &'static str) -> ParseResult<bool> {
        Ok(if self.peek_is_contextual_keyword(expected)? {
            self.advance()?;
            true
        } else {
            false
        })
    }

    /// Consume a contextual keyword or report a precise error.
    pub(super) fn expect_contextual_keyword(&mut self, expected: &'static str) -> ParseResult<()> {
        if self.eat_contextual_keyword(expected)? {
            Ok(())
        } else {
            Err(self.unexpected(format!("`{}`", expected.to_ascii_uppercase())))
        }
    }

    /// True if the current token's source spelling is `expected`.
    pub(super) fn peek_is_contextual_keyword(
        &mut self,
        expected: &'static str,
    ) -> ParseResult<bool> {
        Ok(self
            .peek()?
            .is_some_and(|token| self.token_is_contextual_keyword(token, expected)))
    }

    /// True if the token `n` positions ahead has source spelling `expected`.
    pub(super) fn peek_nth_is_contextual_keyword(
        &mut self,
        n: usize,
        expected: &'static str,
    ) -> ParseResult<bool> {
        Ok(self
            .peek_nth(n)?
            .is_some_and(|token| self.token_is_contextual_keyword(token, expected)))
    }

    /// True if `token` is a word-like token whose source spelling is `expected`.
    pub(super) fn token_is_contextual_keyword(&self, token: Token, expected: &str) -> bool {
        match token.kind {
            TokenKind::Word | TokenKind::Keyword(_) => {
                self.span_text(token.span).eq_ignore_ascii_case(expected)
            }
            TokenKind::Number
            | TokenKind::String
            | TokenKind::QuotedIdent
            | TokenKind::Parameter
            | TokenKind::PositionalColumn
            | TokenKind::Variable
            | TokenKind::StageReference
            | TokenKind::Operator(_)
            | TokenKind::Punctuation(_)
            | TokenKind::Unknown => false,
        }
    }

    /// True if `token` is admissible as an identifier in a grammatical position
    /// whose reject set is `reserved`.
    ///
    /// PostgreSQL reserves keywords *per position* (prod-keyword-position-reserved-sets),
    /// so the gate is parameterized by the dialect's reject set for the position
    /// rather than one global "reserved" flag. A plain word or quoted identifier is
    /// always admissible — the tokenizer only emits a `QuotedIdent` for a quote
    /// style the dialect enables, and quoting bypasses reservation, so even a
    /// reserved word is a valid identifier once quoted. A keyword is admissible
    /// unless this position reserves it. It is an `O(1)` bitset test with no
    /// allocation, matching the hot identifier path. Materialization (delimiter
    /// stripping and doubled-close unescape) happens later in
    /// `parse_ident`.
    pub(super) fn token_admissible(&self, token: Token, reserved: KeywordSet) -> bool {
        match token.kind {
            TokenKind::Word | TokenKind::QuotedIdent => true,
            TokenKind::Keyword(keyword) => !reserved.contains(keyword),
            _ => false,
        }
    }

    /// True if `token` can be a column/table name or a `ColId` correlation alias:
    /// admits `unreserved ∪ col_name` (rejects `type_func_name`, so `JOIN`/`LEFT`
    /// cannot be a bare ColId).
    pub(super) fn token_can_be_column_name(&self, token: Token) -> bool {
        self.token_admissible(token, self.features().reserved_column_name)
    }

    /// True if `token` can be a label (`ColLabel`): an `AS`-introduced alias and a
    /// qualified-name continuation (`a.b`'s `b`) admit every keyword.
    pub(super) fn token_can_be_label(&self, token: Token) -> bool {
        self.token_admissible(token, KeywordSet::EMPTY)
    }

    /// The reject set for the *leading* part of a name that may be a column
    /// reference, table factor, or function call.
    ///
    /// A `type_func_name` keyword (e.g. `left`) is admissible only as a *function*
    /// name, never as a bare `ColId`, so the head widens to the function-name set
    /// exactly when `(` immediately follows the leading word (an unqualified call);
    /// otherwise the leading word is a bare name or a qualifier, both `ColId`. The
    /// returned set is threaded into
    /// [`parse_object_name_with`](Parser::parse_object_name_with) so the gate that
    /// *admits* the leading token is the same one that *parses* it.
    pub(super) fn name_or_call_head_reserved(&mut self) -> ParseResult<KeywordSet> {
        Ok(if self.peek_nth_is_punct(1, Punctuation::LParen)? {
            self.features().reserved_function_name
        } else {
            self.features().reserved_column_name
        })
    }

    /// True if the current token can start a column/table name (`ColId`).
    pub(super) fn peek_can_start_column_name(&mut self) -> ParseResult<bool> {
        Ok(self
            .peek()?
            .is_some_and(|token| self.token_can_be_column_name(token)))
    }

    /// True if the current token can be a bare column alias — one without `AS`
    /// (`BareColLabel`): rejects the `AS_LABEL` keywords (`OVER`, `FILTER`, …).
    pub(super) fn peek_can_start_bare_alias(&mut self) -> ParseResult<bool> {
        Ok(self
            .peek()?
            .is_some_and(|token| self.token_admissible(token, self.features().reserved_bare_alias)))
    }

    /// True when the next two tokens open DuckDB's prefix colon alias — a
    /// bare-alias-admissible identifier immediately followed by a single `:` — under
    /// [`SelectSyntax::prefix_colon_alias`](crate::ast::dialect::SelectSyntax::prefix_colon_alias).
    /// Shared by the projection (`SELECT j : 42`) and table-factor (`FROM b : a`) heads.
    /// A `::` typecast lexes as [`Punctuation::DoubleColon`], not `Colon`, so `x :: int`
    /// never matches; the flag is checked first so the peeks are skipped when off.
    pub(super) fn peek_starts_prefix_colon_alias(&mut self) -> ParseResult<bool> {
        if !self.features().select_syntax.prefix_colon_alias {
            return Ok(false);
        }
        // DuckDB admits a single-part Sconst as the prefix alias too (`FROM '' : t`,
        // `FROM '' : ''`; engine-measured on libduckdb 1.5.4), so a name-Sconst head
        // followed by `:` opens the same form as a bare-alias identifier.
        let head_ok = self.peek_can_start_bare_alias()?
            || (self.features().identifier_syntax.string_literal_table_names
                && self.peek_is_name_sconst()?);
        Ok(head_ok && self.peek_nth_is_punct(1, Punctuation::Colon)?)
    }

    /// True if the current token is the punctuation `punct`.
    pub(super) fn peek_is_punct(&mut self, punct: Punctuation) -> ParseResult<bool> {
        Ok(matches!(
            self.peek()?.map(|token| token.kind),
            Some(TokenKind::Punctuation(found)) if found == punct
        ))
    }

    /// Consume the current token if it is `punct`; report whether it was.
    pub(super) fn eat_punct(&mut self, punct: Punctuation) -> ParseResult<bool> {
        Ok(if self.peek_is_punct(punct)? {
            self.advance()?;
            true
        } else {
            false
        })
    }

    /// Consume the operator `op` if it is the current token; report whether it was.
    pub(super) fn eat_op(&mut self, op: Operator) -> ParseResult<bool> {
        Ok(if self.peek_is_op(op)? {
            self.advance()?;
            true
        } else {
            false
        })
    }

    /// Consume `punct` or report `expected` at the offending token.
    pub(super) fn expect_punct(
        &mut self,
        punct: Punctuation,
        expected: &'static str,
    ) -> ParseResult<()> {
        if self.eat_punct(punct)? {
            Ok(())
        } else {
            Err(self.unexpected(expected))
        }
    }

    /// True if the current token is the operator `op`.
    pub(super) fn peek_is_op(&mut self, op: Operator) -> ParseResult<bool> {
        Ok(matches!(
            self.peek()?.map(|token| token.kind),
            Some(TokenKind::Operator(found)) if found == op
        ))
    }

    /// True if the token `n` positions ahead is the operator `op`.
    pub(super) fn peek_nth_is_op(&mut self, n: usize, op: Operator) -> ParseResult<bool> {
        Ok(matches!(
            self.peek_nth(n)?.map(|token| token.kind),
            Some(TokenKind::Operator(found)) if found == op
        ))
    }

    /// True if the token `n` positions ahead is the punctuation `punct`.
    pub(super) fn peek_nth_is_punct(&mut self, n: usize, punct: Punctuation) -> ParseResult<bool> {
        Ok(matches!(
            self.peek_nth(n)?.map(|token| token.kind),
            Some(TokenKind::Punctuation(found)) if found == punct
        ))
    }

    /// Consume the operator `op` or report `expected` at the offending token.
    pub(super) fn expect_op(&mut self, op: Operator, expected: &'static str) -> ParseResult<()> {
        if self.eat_op(op)? {
            Ok(())
        } else {
            Err(self.unexpected(expected))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::dialect::{
        FeatureDelta, FeatureSet, GroupingSyntax, JoinSyntax, QueryTailSyntax, SelectSyntax,
    };
    use crate::ast::{
        BinaryOperator, CteBody, Expr, ForClause, ForXmlMode, FormatClause, JoinConstraint,
        JoinOperator, Limit, LimitBy, LimitPercent, LimitSyntax, LockStrength, LockWait,
        LockingSpelling, NoExt, PipeOperator, Resolver as _, SelectDistinct, SelectItem,
        SelectSpelling, SetExpr, SetOperator, SetQuantifier, Setting, Span, Statement,
        UpdateAssignment, ValuesItem,
    };
    use crate::parser::{FeatureDialect, Parsed, TestDialect, parse_with};

    /// ANSI with the fetch-first row-limiting spelling disabled, to exercise the
    /// `query_tail_syntax.fetch_first` gate's reject path.
    const NO_FETCH_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                fetch_first: false,
                ..QueryTailSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus the `ORDER BY ALL` clause-mode flag alone, isolating the gate from
    /// the rest of the DuckDb preset. Implements `RenderDialect` for the exact-text
    /// round-trip checks (the stock DuckDb preset has no Tier-1 render target yet).
    const ORDER_BY_ALL_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.grouping_syntax(GroupingSyntax {
                order_by_all: true,
                ..GroupingSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus the `union_by_name` flag alone, isolating the gate from the rest of
    /// the DuckDb preset. Implements `RenderDialect` for the exact-text round-trip
    /// checks (the stock DuckDb preset has no Tier-1 render target yet).
    const UNION_BY_NAME_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.select_syntax(SelectSyntax {
                union_by_name: true,
                ..SelectSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus the BigQuery/ZetaSQL `pipe_syntax` flag alone, isolating the framework
    /// gate from any future BigQuery preset. Round-trips the `|>` operator surface.
    const PIPE_SYNTAX_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                pipe_syntax: true,
                ..QueryTailSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus the ClickHouse `limit_by_clause` flag alone, isolating the gate from
    /// the rest of the (feature-gated) Lenient preset. Renders for the exact-text
    /// round-trip checks (no ClickHouse Tier-1 render target exists).
    const LIMIT_BY_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                limit_by_clause: true,
                ..QueryTailSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus DuckDB's `recursive_using_key` flag alone, isolating the `USING KEY`
    /// recursive-CTE clause from the rest of the DuckDb preset. Renders for the exact-text
    /// round-trip checks.
    const USING_KEY_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.join_syntax(JoinSyntax {
                recursive_using_key: true,
                ..JoinSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus the ClickHouse `settings_clause` flag alone, isolating the gate from
    /// the rest of the (feature-gated) Lenient preset. Renders for the exact-text
    /// round-trip checks (no ClickHouse Tier-1 render target exists).
    const SETTINGS_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                settings_clause: true,
                ..QueryTailSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus the ClickHouse `format_clause` flag alone, isolating the gate from the
    /// rest of the (feature-gated) Lenient preset. Renders for the exact-text round-trip
    /// checks (no ClickHouse Tier-1 render target exists).
    const FORMAT_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                format_clause: true,
                ..QueryTailSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus the MSSQL `for_xml_json_clause` flag alone, isolating the gate from the
    /// rest of the (feature-gated) MSSQL/Lenient presets. Renders for the exact-text
    /// round-trip checks.
    const FOR_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                for_xml_json_clause: true,
                ..QueryTailSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// ANSI plus BOTH the `locking_clauses` and `for_xml_json_clause` gates — the Lenient
    /// combination — used to prove the `FOR` lead partitions unambiguously between the two.
    const FOR_AND_LOCKING_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                locking_clauses: true,
                for_xml_json_clause: true,
                ..QueryTailSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// Borrow the single query of a one-statement parse.
    fn query_of(parsed: &Parsed) -> &crate::ast::Query<NoExt> {
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        query
    }

    /// Borrow the SELECT body of a one-statement parse (no set operation).
    fn select_of(parsed: &Parsed) -> &crate::ast::Select<NoExt> {
        let SetExpr::Select { select, .. } = &query_of(parsed).body else {
            panic!("expected a plain SELECT body");
        };
        select
    }

    /// The dispatch contract for the query family and the router it owns: a bare
    /// `SELECT`, a `VALUES`, a leading `WITH`, and a parenthesized query all route
    /// through `parse_statement` to a `Statement::Query`, while an input that opens no
    /// statement family is a parse error rather than being silently dropped. This is
    /// the central dispatcher's own boundary — the routing every other family relies on.
    #[test]
    fn dispatch_routes_query_forms_and_rejects_non_statements() {
        for sql in [
            "SELECT 1",
            "VALUES (1, 2)",
            "WITH c AS (SELECT 1) SELECT * FROM c",
            "(SELECT 1)",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(TestDialect))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let _ = query_of(&parsed);
        }
        // A leading word that opens no statement family hits the router's final arm.
        assert!(parse_with("kangaroo", crate::ParseConfig::new(TestDialect)).is_err());
    }

    #[test]
    fn full_query_grammar_parses_to_the_expected_shape() {
        let src = "SELECT DISTINCT a, b AS x \
                   FROM t1 JOIN t2 ON t1.id = t2.id \
                   WHERE a > 1 GROUP BY a HAVING a < 9 \
                   ORDER BY a DESC, b LIMIT 10 OFFSET 5";
        let parsed = parse_with(src, crate::ParseConfig::new(TestDialect))
            .expect("the whole query grammar parses");
        let query = query_of(&parsed);
        let select = select_of(&parsed);

        // DISTINCT + an aliased projection item.
        assert!(
            matches!(
                select.distinct,
                Some(SelectDistinct::Quantifier {
                    quantifier: SetQuantifier::Distinct,
                    ..
                }),
            ),
            "DISTINCT sets the flag",
        );
        assert_eq!(select.projection.len(), 2);
        assert!(matches!(
            select.projection[0],
            SelectItem::Expr { alias: None, .. }
        ));
        let SelectItem::Expr {
            alias: Some(alias), ..
        } = &select.projection[1]
        else {
            panic!("the second item is aliased `b AS x`");
        };
        assert_eq!(parsed.resolver().resolve(alias.sym), "x");

        // FROM with exactly one inner join carrying an ON predicate.
        assert_eq!(select.from.len(), 1, "one comma-free table reference");
        let table = &select.from[0];
        assert_eq!(table.joins.len(), 1, "one join on the relation");
        assert!(matches!(
            table.joins[0].operator,
            JoinOperator::Inner {
                constraint: JoinConstraint::On { .. },
                ..
            },
        ));

        // WHERE / GROUP BY / HAVING all present.
        assert!(select.selection.is_some(), "WHERE selection");
        assert_eq!(select.group_by.len(), 1, "one GROUP BY key");
        assert!(select.having.is_some(), "HAVING predicate");

        // Two ORDER BY keys, the first descending, the second unspecified.
        assert_eq!(query.order_by.len(), 2);
        assert_eq!(query.order_by[0].asc, Some(false), "first key is DESC");
        assert_eq!(query.order_by[1].asc, None, "second key has no direction");

        // LIMIT 10 OFFSET 5.
        let Some(Limit {
            limit: Some(_),
            offset: Some(_),
            syntax: LimitSyntax::LimitOffset,
            ..
        }) = &query.limit
        else {
            panic!("expected LIMIT 10 OFFSET 5");
        };
    }

    #[test]
    fn union_all_builds_a_set_operation() {
        let parsed = parse_with(
            "SELECT 1 UNION ALL SELECT 2",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("set op parses");
        let SetExpr::SetOperation {
            op: SetOperator::Union,
            all,
            left,
            right,
            ..
        } = &query_of(&parsed).body
        else {
            panic!("expected a UNION ALL set operation");
        };
        assert!(*all, "ALL was written");
        assert!(
            matches!(**left, SetExpr::Select { .. }),
            "left operand is a SELECT"
        );
        assert!(
            matches!(**right, SetExpr::Select { .. }),
            "right operand is a SELECT"
        );
    }

    #[test]
    fn union_by_name_sets_the_flag_orthogonally_to_all() {
        use crate::dialect::DuckDb;

        // `UNION BY NAME`: name-matched, not `ALL`. `by_name` and `all` are independent
        // (DuckDB serializes `setop_type: UNION_BY_NAME` with a separate `setop_all`;
        // probed on 1.5.4).
        let parsed = parse_with(
            "SELECT 1 a UNION BY NAME SELECT 2 a",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("UNION BY NAME parses");
        let SetExpr::SetOperation {
            op: SetOperator::Union,
            all,
            by_name,
            ..
        } = &query_of(&parsed).body
        else {
            panic!("expected a UNION BY NAME set operation");
        };
        assert!(!*all, "no ALL was written");
        assert!(*by_name, "BY NAME was written");

        // `UNION ALL BY NAME`: both modifiers set.
        let parsed = parse_with(
            "SELECT 1 a UNION ALL BY NAME SELECT 2 a",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("UNION ALL BY NAME parses");
        let SetExpr::SetOperation { all, by_name, .. } = &query_of(&parsed).body else {
            panic!("expected a set operation");
        };
        assert!(*all, "ALL was written");
        assert!(*by_name, "BY NAME was written");
    }

    #[test]
    fn union_by_name_chains_left_associatively() {
        use crate::dialect::DuckDb;

        // A sweep case: `a UNION ALL BY NAME b UNION BY NAME c` chains left-to-right
        // exactly as ordinary same-precedence set operators do — the outer node is the
        // second `UNION BY NAME`, whose left operand is the first.
        let parsed = parse_with(
            "SELECT 1 a UNION ALL BY NAME SELECT 2 a UNION BY NAME SELECT 3 a",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("a chained BY NAME set operation parses");
        let SetExpr::SetOperation {
            op: SetOperator::Union,
            all,
            by_name,
            left,
            ..
        } = &query_of(&parsed).body
        else {
            panic!("expected the outer set operation");
        };
        assert!(!*all, "the outer op is a bare `UNION BY NAME`");
        assert!(*by_name);
        let SetExpr::SetOperation {
            all: inner_all,
            by_name: inner_by_name,
            ..
        } = &**left
        else {
            panic!("the left operand is the inner set operation");
        };
        assert!(*inner_all, "the inner op is `UNION ALL BY NAME`");
        assert!(*inner_by_name);
    }

    #[test]
    fn union_by_name_round_trips_each_spelling() {
        use crate::render::Renderer;

        for sql in [
            "SELECT 1 UNION BY NAME SELECT 2",
            "SELECT 1 UNION ALL BY NAME SELECT 2",
            "SELECT 1 UNION ALL BY NAME SELECT 2 UNION BY NAME SELECT 3",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(UNION_BY_NAME_DIALECT))
                .expect("UNION BY NAME parses");
            assert_eq!(
                Renderer::new(UNION_BY_NAME_DIALECT)
                    .render_parsed(&parsed)
                    .expect("UNION BY NAME renders"),
                sql,
            );
        }
    }

    #[test]
    fn pipe_where_parses_chains_and_round_trips() {
        use crate::render::Renderer;

        // One `|> WHERE` operator: it lands on the query's `pipe_operators` tail (not the
        // SELECT's `WHERE`), carrying the predicate expression, and round-trips verbatim.
        let parsed = parse_with(
            "SELECT a FROM t |> WHERE a > 1",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe WHERE parses");
        let query = query_of(&parsed);
        assert!(
            matches!(&query.body, SetExpr::Select { select, .. } if select.selection.is_none()),
            "the predicate belongs to the pipe operator, not the SELECT body's WHERE",
        );
        assert_eq!(query.pipe_operators.len(), 1);
        assert!(matches!(
            &query.pipe_operators[0],
            PipeOperator::Where {
                predicate: Expr::BinaryOp {
                    op: BinaryOperator::Gt,
                    ..
                },
                ..
            }
        ));
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe WHERE renders"),
            "SELECT a FROM t |> WHERE a > 1",
        );

        // The chain loops: two `|>` operators produce two tail elements, in written order.
        let chained = parse_with(
            "SELECT a FROM t |> WHERE a > 1 |> WHERE a < 9",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe chain parses");
        assert_eq!(query_of(&chained).pipe_operators.len(), 2);
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&chained)
                .expect("pipe chain renders"),
            "SELECT a FROM t |> WHERE a > 1 |> WHERE a < 9",
        );
    }

    #[test]
    fn pipe_syntax_is_rejected_without_the_dialect_gate() {
        use crate::dialect::{Ansi, Postgres};

        // The gate is shared by the tokenizer, so with it off `|>` never lexes (the bytes
        // stay `|` then `>`); a `|>` after a query is trailing junk the parser rejects.
        parse_with(
            "SELECT a FROM t |> WHERE a > 1",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no pipe syntax");
        parse_with(
            "SELECT a FROM t |> WHERE a > 1",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL has no pipe syntax");
    }

    #[test]
    fn limit_by_parses_distinctly_and_round_trips() {
        use crate::render::Renderer;

        // `LIMIT n BY expr` populates the dedicated `limit_by` field, leaving the ordinary
        // `limit` tail `None` — the two are represented distinctly.
        let parsed = parse_with(
            "SELECT a FROM t LIMIT 2 BY x",
            crate::ParseConfig::new(LIMIT_BY_DIALECT),
        )
        .expect("LIMIT BY parses");
        let query = query_of(&parsed);
        let LimitBy {
            limit, offset, by, ..
        } = query
            .limit_by
            .as_deref()
            .expect("LIMIT BY populates the limit_by field");
        assert!(matches!(limit, Expr::Literal { .. }));
        assert!(offset.is_none());
        assert_eq!(by.len(), 1);
        assert!(query.limit.is_none(), "the ordinary LIMIT tail stays empty");
        assert_eq!(
            Renderer::new(LIMIT_BY_DIALECT)
                .render_parsed(&parsed)
                .expect("LIMIT BY renders"),
            "SELECT a FROM t LIMIT 2 BY x",
        );
    }

    #[test]
    fn limit_by_carries_offset_and_multiple_keys() {
        use crate::render::Renderer;

        // `LIMIT n OFFSET m BY a, b` — the `OFFSET`-spelled skip is valid *and* is the
        // disambiguation crux: the `BY` after `OFFSET m` diverts to LIMIT BY rather than
        // the ordinary `LIMIT n OFFSET m`.
        let parsed = parse_with(
            "SELECT a FROM t LIMIT 5 OFFSET 3 BY x, y",
            crate::ParseConfig::new(LIMIT_BY_DIALECT),
        )
        .expect("LIMIT BY with OFFSET parses");
        let query = query_of(&parsed);
        let by = query.limit_by.as_deref().expect("limit_by populated");
        assert!(by.offset.is_some(), "OFFSET skip captured");
        assert_eq!(by.by.len(), 2, "two grouping keys");
        assert!(query.limit.is_none());
        assert_eq!(
            Renderer::new(LIMIT_BY_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "SELECT a FROM t LIMIT 5 OFFSET 3 BY x, y",
        );
    }

    #[test]
    fn limit_by_coexists_with_a_trailing_limit() {
        use crate::render::Renderer;

        // A query may carry BOTH a LIMIT BY and a final LIMIT, in that order (per-group
        // limit first, whole-result cap second).
        let parsed = parse_with(
            "SELECT a FROM t ORDER BY a LIMIT 2 BY x LIMIT 10",
            crate::ParseConfig::new(LIMIT_BY_DIALECT),
        )
        .expect("LIMIT BY + LIMIT parses");
        let query = query_of(&parsed);
        assert!(query.limit_by.is_some(), "LIMIT BY captured");
        let limit = query
            .limit
            .as_ref()
            .expect("the trailing LIMIT is captured");
        assert!(matches!(limit.limit, Some(Expr::Literal { .. })));
        assert_eq!(
            Renderer::new(LIMIT_BY_DIALECT)
                .render_parsed(&parsed)
                .expect("renders"),
            "SELECT a FROM t ORDER BY a LIMIT 2 BY x LIMIT 10",
        );
    }

    #[test]
    fn plain_limit_is_untouched_under_the_limit_by_gate() {
        // With the gate on, a `LIMIT` with no trailing `BY` is still the ordinary tail:
        // the speculative LIMIT-BY read rewinds. `LIMIT n` and `LIMIT n OFFSET m` both
        // land on `limit`, not `limit_by`.
        let plain = parse_with(
            "SELECT a FROM t LIMIT 10",
            crate::ParseConfig::new(LIMIT_BY_DIALECT),
        )
        .expect("plain LIMIT parses");
        assert!(query_of(&plain).limit_by.is_none());
        assert!(query_of(&plain).limit.is_some());

        let with_offset = parse_with(
            "SELECT a FROM t LIMIT 10 OFFSET 5",
            crate::ParseConfig::new(LIMIT_BY_DIALECT),
        )
        .expect("LIMIT/OFFSET parses");
        assert!(query_of(&with_offset).limit_by.is_none());
        let limit = query_of(&with_offset)
            .limit
            .as_ref()
            .expect("ordinary limit");
        assert!(
            limit.offset.is_some(),
            "OFFSET folds onto the ordinary tail"
        );
    }

    #[test]
    fn limit_by_is_rejected_without_the_dialect_gate() {
        use crate::dialect::{Ansi, Postgres};

        // No shipped preset but Lenient spells LIMIT BY; the ungated presets parse the
        // leading `LIMIT 2` as the ordinary tail and reject the trailing `BY x` as junk.
        parse_with(
            "SELECT a FROM t LIMIT 2 BY x",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no LIMIT BY");
        parse_with(
            "SELECT a FROM t LIMIT 2 BY x",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL has no LIMIT BY");
        // The plain LIMIT those presets DO support is unaffected.
        parse_with("SELECT a FROM t LIMIT 2", crate::ParseConfig::new(Ansi))
            .expect("plain LIMIT still parses");
        parse_with("SELECT a FROM t LIMIT 2", crate::ParseConfig::new(Postgres))
            .expect("plain LIMIT still parses");
    }

    #[test]
    fn using_key_recursive_cte_parses_and_round_trips() {
        use crate::render::Renderer;

        // DuckDB's keyed-recursion clause sits between the CTE column list and `AS`; it
        // populates `Cte::using_key` with the bare key columns and round-trips verbatim.
        let sql = "WITH RECURSIVE cte(x, y) USING KEY (x) AS (SELECT 1, 0 \
                   UNION SELECT x, y + 1 FROM cte WHERE y < 10) TABLE cte";
        let parsed =
            parse_with(sql, crate::ParseConfig::new(USING_KEY_DIALECT)).expect("USING KEY parses");
        let with = query_of(&parsed)
            .with
            .as_ref()
            .expect("the statement is a WITH query");
        let key = with.ctes[0]
            .using_key
            .as_ref()
            .expect("the CTE carries a USING KEY clause");
        assert_eq!(key.len(), 1, "one key column");
        assert_eq!(parsed.resolver().resolve(key[0].sym), "x");
        assert_eq!(
            Renderer::new(USING_KEY_DIALECT)
                .render_parsed(&parsed)
                .expect("USING KEY renders"),
            sql,
        );

        // Multi-column key round-trips too.
        let multi = "WITH RECURSIVE cte(x, y) USING KEY (x, y) AS (SELECT 1, 0 \
                     UNION ALL SELECT x, y + 1 FROM cte WHERE y < 10) TABLE cte";
        let parsed_multi = parse_with(multi, crate::ParseConfig::new(USING_KEY_DIALECT))
            .expect("multi-key USING KEY parses");
        assert_eq!(
            Renderer::new(USING_KEY_DIALECT)
                .render_parsed(&parsed_multi)
                .expect("multi-key renders"),
            multi,
        );
    }

    #[test]
    fn using_key_is_rejected_without_the_dialect_gate() {
        use crate::dialect::{Ansi, Postgres};

        // No shipped preset but DuckDb/Lenient spells USING KEY; with the gate off the `USING`
        // after the CTE column list is unexpected where `AS` is required.
        let sql = "WITH RECURSIVE cte(x) USING KEY (x) AS (SELECT 1 UNION SELECT x FROM cte) \
                   TABLE cte";
        parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI has no USING KEY");
        parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect_err("PostgreSQL has no USING KEY");
        // The ordinary recursive CTE those presets DO support is unaffected.
        parse_with(
            "WITH RECURSIVE cte(x) AS (SELECT 1 UNION SELECT x FROM cte) TABLE cte",
            crate::ParseConfig::new(Postgres),
        )
        .expect("plain recursive CTE still parses");
    }

    #[test]
    fn settings_clause_parses_typed_pairs_and_round_trips() {
        use crate::render::Renderer;

        // `SETTINGS name = value, …` populates `Query::settings` with typed pairs: a
        // numeric, a string, and an identifier value all reuse the `Expr` value node.
        let parsed = parse_with(
            "SELECT a FROM t SETTINGS max_threads = 8, join_algorithm = 'auto', mode = best_effort",
            crate::ParseConfig::new(SETTINGS_DIALECT),
        )
        .expect("SETTINGS parses");
        let query = query_of(&parsed);
        assert_eq!(query.settings.len(), 3, "three settings captured");
        let Setting { name, value, .. } = &query.settings[0];
        assert_eq!(parsed.resolver().resolve(name.sym), "max_threads");
        assert!(
            matches!(value, Expr::Literal { .. }),
            "numeric literal value"
        );
        assert!(
            matches!(&query.settings[1].value, Expr::Literal { .. }),
            "string literal value",
        );
        assert!(
            matches!(&query.settings[2].value, Expr::Column { .. }),
            "identifier value reuses the column-reference Expr",
        );
        assert_eq!(
            Renderer::new(SETTINGS_DIALECT)
                .render_parsed(&parsed)
                .expect("SETTINGS renders"),
            "SELECT a FROM t SETTINGS max_threads = 8, join_algorithm = 'auto', mode = best_effort",
        );
    }

    #[test]
    fn settings_coexists_with_limit_by_and_trailing_limit() {
        use crate::render::Renderer;

        // All three ClickHouse tails together, in order: `LIMIT BY` (per-group),
        // ordinary `LIMIT`, then `SETTINGS`. The dialect must open the LIMIT-family
        // gates too, so build one carrying both flags.
        const BOTH: FeatureDialect = {
            const FEATURES: FeatureSet =
                FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                    limit_by_clause: true,
                    settings_clause: true,
                    ..QueryTailSyntax::ANSI
                }));
            FeatureDialect {
                features: &FEATURES,
            }
        };
        let sql = "SELECT a FROM t ORDER BY a LIMIT 2 BY x LIMIT 10 SETTINGS max_threads = 8";
        let parsed = parse_with(sql, crate::ParseConfig::new(BOTH)).expect("all three tails parse");
        let query = query_of(&parsed);
        assert!(query.limit_by.is_some(), "LIMIT BY captured");
        assert!(query.limit.is_some(), "trailing LIMIT captured");
        assert_eq!(query.settings.len(), 1, "SETTINGS captured");
        assert_eq!(
            Renderer::new(BOTH).render_parsed(&parsed).expect("renders"),
            sql,
        );
    }

    #[test]
    fn plain_query_is_untouched_under_the_settings_gate() {
        // With the gate on, a query writing no `SETTINGS` carries an empty list — the
        // contextual keyword only diverts when the word `SETTINGS` actually leads.
        let parsed = parse_with(
            "SELECT a FROM t LIMIT 10",
            crate::ParseConfig::new(SETTINGS_DIALECT),
        )
        .expect("plain query parses");
        assert!(query_of(&parsed).settings.is_empty());
        // `settings` stays an ordinary bare alias when no `name =` head follows.
        let aliased = parse_with(
            "SELECT a FROM t settings",
            crate::ParseConfig::new(SETTINGS_DIALECT),
        )
        .expect("bare alias parses");
        assert!(query_of(&aliased).settings.is_empty());
    }

    #[test]
    fn settings_parses_on_a_from_less_query() {
        // A FROM-less `SELECT <expr> SETTINGS …` reaches the tail — the projection-alias
        // position declines the `SETTINGS name =` head just as the table-alias position
        // does, so the clause parses instead of aliasing the projection.
        let parsed = parse_with(
            "SELECT 1 SETTINGS max_threads = 8",
            crate::ParseConfig::new(SETTINGS_DIALECT),
        )
        .expect("FROM-less SETTINGS parses");
        assert_eq!(query_of(&parsed).settings.len(), 1);
    }

    #[test]
    fn settings_clause_is_rejected_without_the_dialect_gate() {
        use crate::dialect::{Ansi, Postgres};

        // No shipped preset but Lenient spells SETTINGS; the ungated presets leave the
        // trailing `SETTINGS …` unconsumed and reject it as junk. `SETTINGS` stays an
        // ordinary identifier there, so it is usable as an alias.
        parse_with(
            "SELECT a FROM t SETTINGS max_threads = 8",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no SETTINGS tail");
        parse_with(
            "SELECT a FROM t SETTINGS max_threads = 8",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL has no SETTINGS tail");
        // The bare word is a plain identifier off this position (a column alias).
        parse_with("SELECT a AS settings FROM t", crate::ParseConfig::new(Ansi))
            .expect("SETTINGS is an ordinary identifier");
    }

    #[test]
    fn format_clause_parses_and_round_trips() {
        use crate::render::Renderer;

        // `FORMAT <name>` populates `Query::format` with the bare format name, preserving
        // its source spelling (`JSON`, not lowered) so it round-trips case-sensitively.
        let parsed = parse_with(
            "SELECT a FROM t FORMAT JSON",
            crate::ParseConfig::new(FORMAT_DIALECT),
        )
        .expect("FORMAT parses");
        let query = query_of(&parsed);
        let FormatClause { name, .. } = query
            .format
            .as_deref()
            .expect("FORMAT populates the format field");
        assert_eq!(parsed.resolver().resolve(name.sym), "JSON");
        assert_eq!(
            Renderer::new(FORMAT_DIALECT)
                .render_parsed(&parsed)
                .expect("FORMAT renders"),
            "SELECT a FROM t FORMAT JSON",
        );

        // A multi-word-looking format name is still a single bare identifier
        // (`TabSeparated`), and the otherwise-reserved `Null` keyword is a legal format
        // name (`FORMAT Null` discards output in ClickHouse) — both round-trip.
        for name in ["TabSeparated", "Null"] {
            let sql = format!("SELECT a FROM t FORMAT {name}");
            let parsed = parse_with(&sql, crate::ParseConfig::new(FORMAT_DIALECT))
                .expect("format name parses");
            assert_eq!(
                parsed
                    .resolver()
                    .resolve(query_of(&parsed).format.as_deref().unwrap().name.sym),
                name,
            );
            assert_eq!(
                Renderer::new(FORMAT_DIALECT)
                    .render_parsed(&parsed)
                    .expect("renders"),
                sql,
            );
        }
    }

    #[test]
    fn format_coexists_with_the_other_query_tails() {
        use crate::render::Renderer;

        // All four ClickHouse tails together, in order: `LIMIT BY` (per-group), ordinary
        // `LIMIT`, `SETTINGS`, then `FORMAT` last of all. The dialect must open every gate,
        // so build one carrying all three flags.
        const ALL: FeatureDialect = {
            const FEATURES: FeatureSet =
                FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                    limit_by_clause: true,
                    settings_clause: true,
                    format_clause: true,
                    ..QueryTailSyntax::ANSI
                }));
            FeatureDialect {
                features: &FEATURES,
            }
        };
        let sql =
            "SELECT a FROM t ORDER BY a LIMIT 2 BY x LIMIT 10 SETTINGS max_threads = 8 FORMAT JSON";
        let parsed = parse_with(sql, crate::ParseConfig::new(ALL)).expect("all four tails parse");
        let query = query_of(&parsed);
        assert!(query.limit_by.is_some(), "LIMIT BY captured");
        assert!(query.limit.is_some(), "trailing LIMIT captured");
        assert_eq!(query.settings.len(), 1, "SETTINGS captured");
        assert!(query.format.is_some(), "FORMAT captured");
        assert_eq!(
            Renderer::new(ALL).render_parsed(&parsed).expect("renders"),
            sql,
        );
    }

    #[test]
    fn plain_query_and_bare_alias_untouched_under_the_format_gate() {
        // With the gate on, a query naming no format carries `None` — the contextual
        // keyword only diverts when the word `FORMAT` actually leads a name.
        let parsed = parse_with(
            "SELECT a FROM t LIMIT 10",
            crate::ParseConfig::new(FORMAT_DIALECT),
        )
        .expect("plain query parses");
        assert!(query_of(&parsed).format.is_none());
        // `format` stays an ordinary bare table alias when no format-name head follows.
        let aliased = parse_with(
            "SELECT a FROM t format",
            crate::ParseConfig::new(FORMAT_DIALECT),
        )
        .expect("bare alias parses");
        assert!(query_of(&aliased).format.is_none());
        // Even before a following clause keyword, `format` is the alias, not the clause.
        let aliased_where = parse_with(
            "SELECT a FROM t format WHERE a > 1",
            crate::ParseConfig::new(FORMAT_DIALECT),
        )
        .expect("bare alias before WHERE parses");
        assert!(query_of(&aliased_where).format.is_none());
    }

    #[test]
    fn format_parses_on_a_from_less_query() {
        // A FROM-less `SELECT <expr> FORMAT <name>` reaches the tail — the projection-alias
        // position declines the `FORMAT <name>` head just as the table-alias position does,
        // so the clause parses instead of aliasing the projection.
        let parsed = parse_with(
            "SELECT 1 FORMAT JSON",
            crate::ParseConfig::new(FORMAT_DIALECT),
        )
        .expect("FROM-less FORMAT parses");
        assert!(query_of(&parsed).format.is_some());
    }

    #[test]
    fn format_clause_is_rejected_without_the_dialect_gate() {
        use crate::dialect::{Ansi, Postgres};

        // No shipped preset but Lenient spells FORMAT; the ungated presets leave the
        // trailing `FORMAT …` unconsumed and reject it as junk. `FORMAT` stays an ordinary
        // identifier there, so it is usable as an alias.
        parse_with("SELECT a FROM t FORMAT JSON", crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no FORMAT tail");
        parse_with(
            "SELECT a FROM t FORMAT JSON",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL has no FORMAT tail");
        // The bare word is a plain identifier off this position (a column alias).
        parse_with("SELECT a AS format FROM t", crate::ParseConfig::new(Ansi))
            .expect("FORMAT is an ordinary identifier");
    }

    #[test]
    fn for_xml_clause_parses_modes_and_options_and_round_trips() {
        use crate::render::Renderer;

        // Each mode and each directive combination round-trips exactly; the parser accepts
        // the directives in any order and renders them in the canonical MSSQL order.
        for sql in [
            "SELECT a FROM t FOR XML RAW",
            "SELECT a FROM t FOR XML RAW('MyRow')",
            "SELECT a FROM t FOR XML AUTO",
            "SELECT a FROM t FOR XML EXPLICIT",
            "SELECT a FROM t FOR XML PATH",
            "SELECT a FROM t FOR XML PATH('row')",
            "SELECT a FROM t FOR XML AUTO, BINARY BASE64",
            "SELECT a FROM t FOR XML AUTO, TYPE",
            "SELECT a FROM t FOR XML AUTO, ROOT",
            "SELECT a FROM t FOR XML AUTO, ROOT('r')",
            "SELECT a FROM t FOR XML AUTO, ELEMENTS",
            "SELECT a FROM t FOR XML AUTO, ELEMENTS XSINIL",
            "SELECT a FROM t FOR XML AUTO, ELEMENTS ABSENT",
            "SELECT a FROM t FOR XML RAW('r'), BINARY BASE64, TYPE, ROOT('root'), ELEMENTS XSINIL",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(FOR_DIALECT))
                .unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
            assert!(
                matches!(
                    query_of(&parsed).for_clause.as_deref(),
                    Some(ForClause::Xml { .. })
                ),
                "{sql:?} populates a FOR XML clause",
            );
            assert_eq!(
                Renderer::new(FOR_DIALECT)
                    .render_parsed(&parsed)
                    .expect("renders"),
                sql,
                "{sql:?} round-trips",
            );
        }

        // Directives are order-independent on parse but canonicalized on render.
        let reordered = parse_with(
            "SELECT a FROM t FOR XML AUTO, TYPE, BINARY BASE64",
            crate::ParseConfig::new(FOR_DIALECT),
        )
        .expect("out-of-order directives parse");
        assert_eq!(
            Renderer::new(FOR_DIALECT)
                .render_parsed(&reordered)
                .expect("renders"),
            "SELECT a FROM t FOR XML AUTO, BINARY BASE64, TYPE",
            "directives render in canonical MSSQL order",
        );

        // Structural check: the mode and typed flag land where expected.
        let parsed = parse_with(
            "SELECT a FROM t FOR XML PATH('p'), TYPE",
            crate::ParseConfig::new(FOR_DIALECT),
        )
        .unwrap();
        let Some(ForClause::Xml { mode, typed, .. }) = query_of(&parsed).for_clause.as_deref()
        else {
            panic!("expected FOR XML");
        };
        assert!(matches!(mode, ForXmlMode::Path { name: Some(_), .. }));
        assert!(*typed);
    }

    #[test]
    fn for_json_clause_parses_modes_and_options_and_round_trips() {
        use crate::render::Renderer;

        for sql in [
            "SELECT a FROM t FOR JSON AUTO",
            "SELECT a FROM t FOR JSON PATH",
            "SELECT a FROM t FOR JSON AUTO, ROOT",
            "SELECT a FROM t FOR JSON AUTO, ROOT('r')",
            "SELECT a FROM t FOR JSON AUTO, INCLUDE_NULL_VALUES",
            "SELECT a FROM t FOR JSON AUTO, WITHOUT_ARRAY_WRAPPER",
            "SELECT a FROM t FOR JSON PATH, ROOT('r'), INCLUDE_NULL_VALUES, WITHOUT_ARRAY_WRAPPER",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(FOR_DIALECT))
                .unwrap_or_else(|e| panic!("{sql:?}: {e:?}"));
            assert!(
                matches!(
                    query_of(&parsed).for_clause.as_deref(),
                    Some(ForClause::Json { .. })
                ),
                "{sql:?} populates a FOR JSON clause",
            );
            assert_eq!(
                Renderer::new(FOR_DIALECT)
                    .render_parsed(&parsed)
                    .expect("renders"),
                sql,
                "{sql:?} round-trips",
            );
        }
    }

    #[test]
    fn for_clause_is_rejected_without_the_dialect_gate() {
        use crate::dialect::{Ansi, Postgres};

        // No shipped preset but MSSQL and Lenient spells `FOR XML`/`FOR JSON`; the ungated
        // presets leave the trailing `FOR XML …` unconsumed and reject it as junk.
        parse_with(
            "SELECT a FROM t FOR XML AUTO",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no FOR XML tail");
        parse_with(
            "SELECT a FROM t FOR JSON AUTO",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL has no FOR JSON tail");
    }

    #[test]
    fn for_clause_and_locking_partition_on_the_follow_token() {
        // Under Lenient both gates are on; `FOR` disambiguates on its follow token —
        // `XML`/`JSON` is the result-shaping clause, `UPDATE`/`SHARE` a locking clause.
        // A bare locking clause is unaffected.
        let locking = parse_with(
            "SELECT a FROM t FOR UPDATE",
            crate::ParseConfig::new(FOR_AND_LOCKING_DIALECT),
        )
        .expect("FOR UPDATE");
        assert_eq!(query_of(&locking).locking.len(), 1);
        assert!(query_of(&locking).for_clause.is_none());

        // A `FOR XML` lead is the result-shaping clause even with locking enabled.
        let shaping = parse_with(
            "SELECT a FROM t FOR XML AUTO",
            crate::ParseConfig::new(FOR_AND_LOCKING_DIALECT),
        )
        .expect("FOR XML AUTO");
        assert!(query_of(&shaping).locking.is_empty());
        assert!(query_of(&shaping).for_clause.is_some());

        // Both on one query: the locking clause parses, then the stacking loop declines the
        // trailing `FOR XML` (it is not a locking strength), leaving it for the FOR clause.
        use crate::render::Renderer;
        let both = parse_with(
            "SELECT a FROM t FOR UPDATE FOR XML AUTO",
            crate::ParseConfig::new(FOR_AND_LOCKING_DIALECT),
        )
        .expect("stacked locking + FOR XML");
        assert_eq!(query_of(&both).locking.len(), 1);
        assert!(query_of(&both).for_clause.is_some());
        assert_eq!(
            Renderer::new(FOR_AND_LOCKING_DIALECT)
                .render_parsed(&both)
                .expect("renders"),
            "SELECT a FROM t FOR UPDATE FOR XML AUTO",
        );
    }

    #[test]
    fn pipe_operator_rejects_unknown_keyword() {
        // The framework ships only `WHERE`; an unrecognised keyword after `|>` is a clean
        // reject (the seam for the `planner-parity-pipe-*` operator tickets).
        parse_with(
            "SELECT a FROM t |> BOGUS a",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> BOGUS` is not a known pipe operator");
    }

    #[test]
    fn pipe_select_reuses_select_items_and_round_trips() {
        use crate::render::Renderer;

        // `|> SELECT` carries an ordinary projection-item list (`SelectItem`), including an
        // aliased item — the same node a leading `SELECT` list builds — and round-trips.
        let parsed = parse_with(
            "SELECT a FROM t |> SELECT a, b AS x",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe SELECT parses");
        let query = query_of(&parsed);
        assert_eq!(query.pipe_operators.len(), 1);
        let PipeOperator::Select { items, .. } = &query.pipe_operators[0] else {
            panic!("expected a pipe SELECT operator");
        };
        assert_eq!(items.len(), 2);
        assert!(matches!(items[1], SelectItem::Expr { alias: Some(_), .. }));
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe SELECT renders"),
            "SELECT a FROM t |> SELECT a, b AS x",
        );

        // An empty projection has no first item to parse — a clean reject.
        parse_with(
            "SELECT a FROM t |> SELECT",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> SELECT` needs at least one item");
    }

    #[test]
    fn pipe_extend_shares_select_item_shape_and_round_trips() {
        use crate::render::Renderer;

        // `|> EXTEND` appends computed columns; it shares `SELECT`'s `SelectItem` shape, so a
        // computed-and-aliased item parses to the same node and round-trips.
        let parsed = parse_with(
            "SELECT a FROM t |> EXTEND a + 1 AS b",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe EXTEND parses");
        let query = query_of(&parsed);
        assert_eq!(query.pipe_operators.len(), 1);
        let PipeOperator::Extend { items, .. } = &query.pipe_operators[0] else {
            panic!("expected a pipe EXTEND operator");
        };
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], SelectItem::Expr { alias: Some(_), .. }));
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe EXTEND renders"),
            "SELECT a FROM t |> EXTEND a + 1 AS b",
        );

        // An empty computed-column list is a clean reject, like `|> SELECT`.
        parse_with(
            "SELECT a FROM t |> EXTEND",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> EXTEND` needs at least one item");
    }

    #[test]
    fn pipe_as_carries_a_bare_alias_and_round_trips() {
        use crate::render::Renderer;

        // `|> AS` names a range variable only: it parses a single identifier into a
        // `TableAlias` whose column list is empty, and round-trips.
        let parsed = parse_with(
            "SELECT a FROM t |> AS u",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe AS parses");
        let query = query_of(&parsed);
        assert_eq!(query.pipe_operators.len(), 1);
        let PipeOperator::As { alias, .. } = &query.pipe_operators[0] else {
            panic!("expected a pipe AS operator");
        };
        assert!(
            alias.columns.is_empty(),
            "pipe AS names only a range variable — no column-alias list",
        );
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe AS renders"),
            "SELECT a FROM t |> AS u",
        );

        // The alias is required, and a column-alias list is not part of the pipe form — the
        // trailing `(x)` is left unconsumed and surfaces as a reject.
        parse_with(
            "SELECT a FROM t |> AS",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> AS` needs an alias name");
        parse_with(
            "SELECT a FROM t |> AS u (x)",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("pipe AS admits no column-alias list");
    }

    #[test]
    fn pipe_order_by_reuses_sort_keys_and_round_trips() {
        use crate::render::Renderer;

        // `|> ORDER BY` reuses the ordinary sort-key list (`OrderByExpr`), directions and
        // all, and round-trips.
        let parsed = parse_with(
            "SELECT a FROM t |> ORDER BY a DESC, b",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe ORDER BY parses");
        let query = query_of(&parsed);
        assert_eq!(query.pipe_operators.len(), 1);
        let PipeOperator::OrderBy { keys, .. } = &query.pipe_operators[0] else {
            panic!("expected a pipe ORDER BY operator");
        };
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].asc, Some(false));
        assert_eq!(keys[1].asc, None);
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe ORDER BY renders"),
            "SELECT a FROM t |> ORDER BY a DESC, b",
        );

        // `BY` is required after `ORDER` — a bare `|> ORDER a` is a clean reject.
        parse_with(
            "SELECT a FROM t |> ORDER a",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> ORDER` requires `BY`");
    }

    #[test]
    fn pipe_limit_carries_count_and_offset_and_round_trips() {
        use crate::render::Renderer;

        // `|> LIMIT <count> OFFSET <skip>`: both operands present, boxed on the narrow pipe
        // variant, round-tripping verbatim.
        let parsed = parse_with(
            "SELECT a FROM t |> LIMIT 10 OFFSET 5",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe LIMIT+OFFSET parses");
        let query = query_of(&parsed);
        assert_eq!(query.pipe_operators.len(), 1);
        let PipeOperator::Limit { offset, .. } = &query.pipe_operators[0] else {
            panic!("expected a pipe LIMIT operator");
        };
        assert!(offset.is_some(), "the OFFSET tail is carried");
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe LIMIT renders"),
            "SELECT a FROM t |> LIMIT 10 OFFSET 5",
        );

        // The bare `|> LIMIT <count>` form omits the OFFSET tail and round-trips.
        let bare = parse_with(
            "SELECT a FROM t |> LIMIT 10",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe LIMIT parses");
        assert!(matches!(
            &query_of(&bare).pipe_operators[0],
            PipeOperator::Limit { offset: None, .. }
        ));
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&bare)
                .expect("bare pipe LIMIT renders"),
            "SELECT a FROM t |> LIMIT 10",
        );

        // A `|> LIMIT` with no count has no operand to parse — a clean reject.
        parse_with(
            "SELECT a FROM t |> LIMIT",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> LIMIT` needs a count");
    }

    #[test]
    fn pipe_operators_chain_in_written_order_and_round_trip() {
        use crate::render::Renderer;

        // A chain mixing all five operators (plus the framework `WHERE`) lands one tail
        // element per `|>` in written order, and the whole chain round-trips verbatim.
        let src = "SELECT a FROM t |> WHERE a > 1 |> SELECT a, b \
                   |> EXTEND a + 1 AS c |> AS u |> ORDER BY a |> LIMIT 10 OFFSET 5";
        let parsed = parse_with(src, crate::ParseConfig::new(PIPE_SYNTAX_DIALECT))
            .expect("pipe chain parses");
        let ops = &query_of(&parsed).pipe_operators;
        assert_eq!(ops.len(), 6);
        assert!(matches!(
            (&ops[0], &ops[1], &ops[2], &ops[3], &ops[4], &ops[5],),
            (
                PipeOperator::Where { .. },
                PipeOperator::Select { .. },
                PipeOperator::Extend { .. },
                PipeOperator::As { .. },
                PipeOperator::OrderBy { .. },
                PipeOperator::Limit { .. },
            )
        ));
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe chain renders"),
            src,
        );
    }

    #[test]
    fn pipe_join_reuses_the_join_node_and_round_trips() {
        use crate::render::Renderer;

        // `|> JOIN` reuses the whole `Join` node — side spelling, relation, and the
        // embedded `ON`/`USING` constraint — independently of any `FROM`-clause join.
        for (src, is_using) in [
            ("SELECT a FROM t |> JOIN u ON t.a = u.a", false),
            ("SELECT a FROM t |> LEFT JOIN u USING (a)", true),
            ("SELECT a FROM t |> CROSS JOIN u", false),
        ] {
            let parsed = parse_with(src, crate::ParseConfig::new(PIPE_SYNTAX_DIALECT))
                .expect("pipe JOIN parses");
            let query = query_of(&parsed);
            assert_eq!(query.pipe_operators.len(), 1);
            let PipeOperator::Join { join, .. } = &query.pipe_operators[0] else {
                panic!("expected a pipe JOIN operator");
            };
            if is_using {
                assert!(matches!(
                    join.operator,
                    JoinOperator::LeftOuter {
                        constraint: JoinConstraint::Using { .. },
                        ..
                    }
                ));
            }
            assert_eq!(
                Renderer::new(PIPE_SYNTAX_DIALECT)
                    .render_parsed(&parsed)
                    .expect("pipe JOIN renders"),
                src,
            );
        }

        // A bare `|> JOIN` with no relation to join has nothing for the table factor to
        // parse — a clean reject.
        parse_with(
            "SELECT a FROM t |> JOIN",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> JOIN` needs a relation");
    }

    #[test]
    fn pipe_set_operation_carries_operator_quantifier_and_queries() {
        use crate::render::Renderer;

        // One `SetOperation` variant carries the `SetOperator` tag; `UNION` admits the
        // `ALL`/`DISTINCT` quantifier and a comma-separated list of parenthesized queries.
        let parsed = parse_with(
            "SELECT a FROM t |> UNION ALL (SELECT a FROM u), (SELECT a FROM v)",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe UNION parses");
        let query = query_of(&parsed);
        assert_eq!(query.pipe_operators.len(), 1);
        let PipeOperator::SetOperation {
            op,
            quantifier,
            queries,
            ..
        } = &query.pipe_operators[0]
        else {
            panic!("expected a pipe set operation");
        };
        assert_eq!(*op, SetOperator::Union);
        assert_eq!(*quantifier, Some(SetQuantifier::All));
        assert_eq!(queries.len(), 2);
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe UNION renders"),
            "SELECT a FROM t |> UNION ALL (SELECT a FROM u), (SELECT a FROM v)",
        );

        // `INTERSECT`/`EXCEPT` reuse the same variant with their own operator tag; the
        // quantifier is optional (`None` renders the bare operator) and round-trips.
        for (src, expected_op) in [
            (
                "SELECT a FROM t |> INTERSECT DISTINCT (SELECT a FROM u)",
                SetOperator::Intersect,
            ),
            (
                "SELECT a FROM t |> EXCEPT (SELECT a FROM u)",
                SetOperator::Except,
            ),
        ] {
            let parsed = parse_with(src, crate::ParseConfig::new(PIPE_SYNTAX_DIALECT))
                .expect("pipe set op parses");
            let PipeOperator::SetOperation { op, .. } = &query_of(&parsed).pipe_operators[0] else {
                panic!("expected a pipe set operation");
            };
            assert_eq!(*op, expected_op);
            assert_eq!(
                Renderer::new(PIPE_SYNTAX_DIALECT)
                    .render_parsed(&parsed)
                    .expect("pipe set op renders"),
                src,
            );
        }

        // The operand must be a parenthesized query — a bare `SELECT` has no leading `(`.
        parse_with(
            "SELECT a FROM t |> UNION SELECT a FROM u",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("a pipe set-operation operand must be parenthesized");
    }

    #[test]
    fn pipe_set_assigns_columns_and_round_trips() {
        use crate::render::Renderer;

        // `|> SET` reuses `UpdateAssignment::Single` (`<column> = <expr>`) for each
        // comma-separated assignment, and round-trips.
        let parsed = parse_with(
            "SELECT a FROM t |> SET a = a + 1, b = 2",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe SET parses");
        let query = query_of(&parsed);
        assert_eq!(query.pipe_operators.len(), 1);
        let PipeOperator::Set { assignments, .. } = &query.pipe_operators[0] else {
            panic!("expected a pipe SET operator");
        };
        assert_eq!(assignments.len(), 2);
        assert!(matches!(assignments[0], UpdateAssignment::Single { .. }));
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe SET renders"),
            "SELECT a FROM t |> SET a = a + 1, b = 2",
        );

        // An assignment needs an `=`; a bare column is a clean reject.
        parse_with(
            "SELECT a FROM t |> SET a",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> SET` needs `<column> = <expr>`");
    }

    #[test]
    fn pipe_call_reuses_function_call_and_optional_alias() {
        use crate::render::Renderer;

        // `|> CALL` reuses the `FunctionCall` node and carries an optional name-only alias.
        let parsed = parse_with(
            "SELECT a FROM t |> CALL tvf(a, 1) AS u",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe CALL parses");
        let query = query_of(&parsed);
        assert_eq!(query.pipe_operators.len(), 1);
        let PipeOperator::Call { call, alias, .. } = &query.pipe_operators[0] else {
            panic!("expected a pipe CALL operator");
        };
        assert_eq!(call.args.len(), 2);
        let alias = alias.as_ref().expect("the AS alias is carried");
        assert!(
            alias.columns.is_empty(),
            "pipe CALL's alias names only a range variable — no column-alias list",
        );
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe CALL renders"),
            "SELECT a FROM t |> CALL tvf(a, 1) AS u",
        );

        // The alias is optional — a bare `|> CALL f()` round-trips with no alias.
        let bare = parse_with(
            "SELECT a FROM t |> CALL f()",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("bare pipe CALL parses");
        assert!(matches!(
            &query_of(&bare).pipe_operators[0],
            PipeOperator::Call { alias: None, .. }
        ));
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&bare)
                .expect("bare pipe CALL renders"),
            "SELECT a FROM t |> CALL f()",
        );

        // `CALL` requires a call, not a bare name — the missing `(` is a clean reject.
        parse_with(
            "SELECT a FROM t |> CALL f",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> CALL` needs a function call");
    }

    #[test]
    fn pipe_batch_two_operators_chain_and_round_trip() {
        use crate::render::Renderer;

        // A chain mixing the batch-2 operators lands one tail element per `|>` in written
        // order, and the whole chain round-trips verbatim.
        let src = "SELECT a FROM t |> JOIN u ON t.a = u.a \
                   |> SET a = a + 1 |> UNION ALL (SELECT a FROM w) |> CALL tvf(a) AS r";
        let parsed = parse_with(src, crate::ParseConfig::new(PIPE_SYNTAX_DIALECT))
            .expect("pipe batch-2 chain parses");
        let ops = &query_of(&parsed).pipe_operators;
        assert_eq!(ops.len(), 4);
        assert!(matches!(
            (&ops[0], &ops[1], &ops[2], &ops[3]),
            (
                PipeOperator::Join { .. },
                PipeOperator::Set { .. },
                PipeOperator::SetOperation { .. },
                PipeOperator::Call { .. },
            )
        ));
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe batch-2 chain renders"),
            src,
        );
    }

    #[test]
    fn pipe_aggregate_carries_aliases_orderings_and_grouping() {
        use crate::render::Renderer;

        // `|> AGGREGATE` carries an aggregate list and an optional `GROUP BY`; both lists
        // admit an `AS` alias and an `ASC`/`DESC` ordering suffix (the combined
        // `PipeAggregateExpr` shape), and round-trip.
        let parsed = parse_with(
            "SELECT a FROM t |> AGGREGATE SUM(x) AS total DESC GROUP BY city ASC",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe AGGREGATE parses");
        let query = query_of(&parsed);
        assert_eq!(query.pipe_operators.len(), 1);
        let PipeOperator::Aggregate {
            aggregates,
            group_by,
            ..
        } = &query.pipe_operators[0]
        else {
            panic!("expected a pipe AGGREGATE operator");
        };
        assert_eq!(aggregates.len(), 1);
        assert!(aggregates[0].alias.is_some());
        assert_eq!(aggregates[0].asc, Some(false));
        assert_eq!(group_by.len(), 1);
        assert_eq!(group_by[0].asc, Some(true));
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe AGGREGATE renders"),
            "SELECT a FROM t |> AGGREGATE SUM(x) AS total DESC GROUP BY city ASC",
        );

        // A grouping-only operator has an empty aggregate list (`GROUP BY` leads) and
        // still round-trips without a stray separating space.
        let grouping_only = parse_with(
            "SELECT a FROM t |> AGGREGATE GROUP BY city",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("grouping-only pipe AGGREGATE parses");
        let PipeOperator::Aggregate { aggregates, .. } =
            &query_of(&grouping_only).pipe_operators[0]
        else {
            panic!("expected a pipe AGGREGATE operator");
        };
        assert!(aggregates.is_empty());
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&grouping_only)
                .expect("grouping-only pipe AGGREGATE renders"),
            "SELECT a FROM t |> AGGREGATE GROUP BY city",
        );

        // A bare `|> AGGREGATE` has no aggregate to parse and no grouping — a clean reject.
        parse_with(
            "SELECT a FROM t |> AGGREGATE",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> AGGREGATE` needs an aggregate or a GROUP BY");
    }

    #[test]
    fn pipe_drop_carries_a_column_list_and_round_trips() {
        use crate::render::Renderer;

        // `|> DROP` is a bare identifier list.
        let parsed = parse_with(
            "SELECT a FROM t |> DROP a, b",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe DROP parses");
        let query = query_of(&parsed);
        let PipeOperator::Drop { columns, .. } = &query.pipe_operators[0] else {
            panic!("expected a pipe DROP operator");
        };
        assert_eq!(columns.len(), 2);
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe DROP renders"),
            "SELECT a FROM t |> DROP a, b",
        );

        // An empty column list has no first identifier to parse — a clean reject.
        parse_with(
            "SELECT a FROM t |> DROP",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> DROP` needs at least one column");
    }

    #[test]
    fn pipe_rename_carries_old_as_new_pairs_and_round_trips() {
        use crate::render::Renderer;

        // `|> RENAME` carries `old AS new` identifier pairs, distinct from a projection
        // alias, and round-trips.
        let parsed = parse_with(
            "SELECT a FROM t |> RENAME a AS x, b AS y",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe RENAME parses");
        let query = query_of(&parsed);
        let PipeOperator::Rename { renames, .. } = &query.pipe_operators[0] else {
            panic!("expected a pipe RENAME operator");
        };
        assert_eq!(renames.len(), 2);
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe RENAME renders"),
            "SELECT a FROM t |> RENAME a AS x, b AS y",
        );

        // A mapping needs the `AS` keyword — a bare column is a clean reject.
        parse_with(
            "SELECT a FROM t |> RENAME a",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> RENAME` needs `<old> AS <new>`");
    }

    #[test]
    fn pipe_pivot_reuses_the_pivot_core_and_round_trips() {
        use crate::render::Renderer;

        // `|> PIVOT` reuses the shared `PivotExpr` aggregate list and the single
        // `FOR <col> IN (<values>)` head (`PivotColumn`), with no `source` or statement
        // tail, and round-trips.
        let parsed = parse_with(
            "SELECT a FROM t |> PIVOT (SUM(sales) FOR quarter IN (1, 2))",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe PIVOT parses");
        let query = query_of(&parsed);
        let PipeOperator::Pivot {
            aggregates, column, ..
        } = &query.pipe_operators[0]
        else {
            panic!("expected a pipe PIVOT operator");
        };
        assert_eq!(aggregates.len(), 1);
        assert_eq!(column.values.len(), 2);
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe PIVOT renders"),
            "SELECT a FROM t |> PIVOT (SUM(sales) FOR quarter IN (1, 2))",
        );

        // The `FOR` head is mandatory — an aggregate-only body is a clean reject.
        parse_with(
            "SELECT a FROM t |> PIVOT (SUM(sales))",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> PIVOT` needs a `FOR` column");
    }

    #[test]
    fn pipe_unpivot_reuses_the_unpivot_core_and_round_trips() {
        use crate::render::Renderer;

        // `|> UNPIVOT` mirrors the table-factor UNPIVOT body (value / name / `IN` list),
        // with no `source`, `NULLS`, or alias, and round-trips.
        let parsed = parse_with(
            "SELECT a FROM t |> UNPIVOT (sales FOR quarter IN (q1, q2))",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect("pipe UNPIVOT parses");
        let query = query_of(&parsed);
        let PipeOperator::Unpivot {
            value,
            name,
            columns,
            ..
        } = &query.pipe_operators[0]
        else {
            panic!("expected a pipe UNPIVOT operator");
        };
        assert_eq!(value.len(), 1);
        assert_eq!(name.len(), 1);
        assert_eq!(columns.len(), 2);
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe UNPIVOT renders"),
            "SELECT a FROM t |> UNPIVOT (sales FOR quarter IN (q1, q2))",
        );

        // The `IN` list is mandatory — omitting it is a clean reject.
        parse_with(
            "SELECT a FROM t |> UNPIVOT (sales FOR quarter)",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> UNPIVOT` needs an `IN` column list");
    }

    #[test]
    fn pipe_tablesample_reuses_the_table_sample_node_and_round_trips() {
        use crate::render::Renderer;

        // `|> TABLESAMPLE` reuses the whole `TableSample` node, including the optional
        // `REPEATABLE` seed, and round-trips — gated only by `pipe_syntax`.
        for src in [
            "SELECT a FROM t |> TABLESAMPLE BERNOULLI (10)",
            "SELECT a FROM t |> TABLESAMPLE SYSTEM (10) REPEATABLE (42)",
        ] {
            let parsed = parse_with(src, crate::ParseConfig::new(PIPE_SYNTAX_DIALECT))
                .expect("pipe TABLESAMPLE parses");
            assert!(matches!(
                &query_of(&parsed).pipe_operators[0],
                PipeOperator::TableSample { .. }
            ));
            assert_eq!(
                Renderer::new(PIPE_SYNTAX_DIALECT)
                    .render_parsed(&parsed)
                    .expect("pipe TABLESAMPLE renders"),
                src,
            );
        }

        // The argument list is mandatory — a bare method has no `(` to open it.
        parse_with(
            "SELECT a FROM t |> TABLESAMPLE BERNOULLI",
            crate::ParseConfig::new(PIPE_SYNTAX_DIALECT),
        )
        .expect_err("`|> TABLESAMPLE` needs an argument list");
    }

    #[test]
    fn pipe_batch_three_operators_chain_and_round_trip() {
        use crate::render::Renderer;

        // A chain mixing all six batch-3 operators lands one tail element per `|>` in
        // written order, and the whole chain round-trips verbatim.
        let src = "SELECT a FROM t |> AGGREGATE COUNT(*) AS c GROUP BY a \
                   |> DROP c |> RENAME a AS b |> PIVOT (SUM(x) FOR b IN (1, 2)) \
                   |> UNPIVOT (v FOR n IN (p, q)) |> TABLESAMPLE SYSTEM (5)";
        let parsed = parse_with(src, crate::ParseConfig::new(PIPE_SYNTAX_DIALECT))
            .expect("pipe batch-3 chain parses");
        let ops = &query_of(&parsed).pipe_operators;
        assert_eq!(ops.len(), 6);
        assert!(matches!(
            (&ops[0], &ops[1], &ops[2], &ops[3], &ops[4], &ops[5],),
            (
                PipeOperator::Aggregate { .. },
                PipeOperator::Drop { .. },
                PipeOperator::Rename { .. },
                PipeOperator::Pivot { .. },
                PipeOperator::Unpivot { .. },
                PipeOperator::TableSample { .. },
            )
        ));
        assert_eq!(
            Renderer::new(PIPE_SYNTAX_DIALECT)
                .render_parsed(&parsed)
                .expect("pipe batch-3 chain renders"),
            src,
        );
    }

    #[test]
    fn by_name_is_rejected_without_the_dialect_gate() {
        use crate::dialect::{Ansi, Postgres};

        // `BY NAME` is a DuckDB-only modifier: with the gate off, `BY` after the set
        // operator is left unconsumed and cannot open a set operand — a clean reject.
        parse_with(
            "SELECT 1 a UNION BY NAME SELECT 2 a",
            crate::ParseConfig::new(Ansi),
        )
        .expect_err("ANSI has no UNION BY NAME");
        parse_with(
            "SELECT 1 a UNION ALL BY NAME SELECT 2 a",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("PostgreSQL has no UNION BY NAME");
    }

    #[test]
    fn by_name_rejects_on_intersect_and_except() {
        use crate::dialect::DuckDb;

        // `BY NAME` is UNION-only: DuckDB syntax-errors on `INTERSECT BY NAME` /
        // `EXCEPT BY NAME` (probed on 1.5.4), so even with the gate on, `BY` after a
        // non-`UNION` operator is left unconsumed and surfaces as the usual reject.
        parse_with(
            "SELECT 1 a INTERSECT BY NAME SELECT 2 a",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("INTERSECT BY NAME is not a DuckDB set operation");
        parse_with(
            "SELECT 1 a EXCEPT BY NAME SELECT 2 a",
            crate::ParseConfig::new(DuckDb),
        )
        .expect_err("EXCEPT BY NAME is not a DuckDB set operation");
    }

    #[test]
    fn values_query_body_parses() {
        let parsed = parse_with(
            "VALUES (1, 2), (3, 4)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("VALUES parses");
        let SetExpr::Values { values, .. } = &query_of(&parsed).body else {
            panic!("expected a VALUES query body");
        };
        assert_eq!(values.rows.len(), 2);
        assert_eq!(values.rows[0].len(), 2);
        assert_eq!(values.rows[1].len(), 2);
    }

    #[test]
    fn values_row_default_item_parses_as_default_not_column() {
        // `DEFAULT` in a VALUES row is its own item (PostgreSQL `SetToDefault`), not a
        // column reference smuggled in through the expression grammar.
        let parsed = parse_with(
            "VALUES (1, DEFAULT), (DEFAULT, 2)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("VALUES parses");
        let SetExpr::Values { values, .. } = &query_of(&parsed).body else {
            panic!("expected a VALUES query body");
        };
        assert_eq!(values.rows.len(), 2);
        assert!(matches!(values.rows[0][0], ValuesItem::Expr { .. }));
        assert!(matches!(values.rows[0][1], ValuesItem::Default { .. }));
        assert!(matches!(values.rows[1][0], ValuesItem::Default { .. }));
        assert!(matches!(values.rows[1][1], ValuesItem::Expr { .. }));
    }

    /// The fitted `MySql` `FeatureSet` wired as a `RenderDialect` (the bare `MySql` preset
    /// struct is parse-only on this crate's test surface, like [`UNION_BY_NAME_DIALECT`]).
    const MYSQL_RENDER: FeatureDialect = FeatureDialect {
        features: &FeatureSet::MYSQL,
    };

    /// Every grammar-valid form of the MySQL `HANDLER` cursor family round-trips
    /// byte-identically — the `[AS] alias` spelling, the schema-qualified `OPEN` table, all
    /// three `READ` selector shapes, every key operator, and the `WHERE`/`LIMIT` tail
    /// (including MySQL's `LIMIT off, n` comma form). The same forms are live-oracle-verified
    /// grammar-valid in `corpus_mysql_verdicts::mysql_handler_live_oracle_parity`.
    #[test]
    fn handler_family_round_trips() {
        use crate::ast::{HandlerOperation, HandlerReadSelector};
        use crate::render::Renderer;

        // Structural spot-checks on the operation shapes and the arity asymmetry.
        let open = parse_with(
            "HANDLER db.t OPEN AS a",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .unwrap();
        let Statement::Handler { handler, .. } = &open.statements()[0] else {
            panic!("expected a HANDLER statement");
        };
        assert_eq!(
            handler.table.0.len(),
            2,
            "OPEN admits a schema-qualified table"
        );
        assert!(matches!(
            handler.operation,
            HandlerOperation::Open {
                as_keyword: true,
                ..
            },
        ));

        let key = parse_with(
            "HANDLER t READ idx >= (1, 2)",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .unwrap();
        let Statement::Handler { handler, .. } = &key.statements()[0] else {
            panic!("expected a HANDLER statement");
        };
        let HandlerOperation::Read {
            selector: HandlerReadSelector::Key { key, .. },
            ..
        } = &handler.operation
        else {
            panic!("expected a key-seek READ");
        };
        assert_eq!(key.len(), 2, "the key tuple carries both values");

        // The MySQL `LIMIT <offset>, <count>` comma form parses (its own `CommaOffset` tag);
        // the canonical render below rewrites it to `LIMIT <count> OFFSET <offset>`, so it is
        // asserted structurally here rather than in the byte-identical round-trip list.
        let comma_limit = parse_with(
            "HANDLER t READ idx = (1) LIMIT 2, 5",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .unwrap();
        let Statement::Handler { handler, .. } = &comma_limit.statements()[0] else {
            panic!("expected a HANDLER statement");
        };
        let HandlerOperation::Read {
            limit: Some(limit), ..
        } = &handler.operation
        else {
            panic!("expected a READ with a LIMIT");
        };
        assert_eq!(limit.syntax, crate::ast::LimitSyntax::CommaOffset);

        for sql in [
            "HANDLER t OPEN",
            "HANDLER t OPEN AS a",
            "HANDLER t OPEN a",
            "HANDLER db.t OPEN",
            "HANDLER db.t OPEN AS a",
            "HANDLER t CLOSE",
            "HANDLER t READ FIRST",
            "HANDLER t READ NEXT",
            "HANDLER t READ FIRST WHERE a > 1",
            "HANDLER t READ NEXT WHERE a > 1 LIMIT 5",
            "HANDLER t READ idx FIRST",
            "HANDLER t READ idx NEXT",
            "HANDLER t READ idx PREV",
            "HANDLER t READ idx LAST",
            "HANDLER t READ `PRIMARY` FIRST",
            "HANDLER t READ idx = (1)",
            "HANDLER t READ idx >= (1)",
            "HANDLER t READ idx <= (1)",
            "HANDLER t READ idx > (1)",
            "HANDLER t READ idx < (1)",
            "HANDLER t READ idx = (1, 2)",
            "HANDLER t READ idx = (1 + 1)",
            "HANDLER t READ idx = (@v)",
            "HANDLER t READ idx = (DEFAULT)",
            "HANDLER t READ `PRIMARY` = (1)",
            "HANDLER t READ idx = (1) WHERE a > 1 LIMIT 5",
            "HANDLER t READ idx = (1) LIMIT 5 OFFSET 2",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(MYSQL_RENDER))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
            let rendered = Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .unwrap_or_else(|err| panic!("{sql:?} renders: {err:?}"));
            assert_eq!(rendered, sql, "round-trip");
        }
    }

    #[test]
    fn handler_family_reject_edge_cases() {
        // Engine-verified `ER_PARSE_ERROR` (1064) on mysql:8.4.10, both-reject-pinned in
        // `m3::SCHEMA_INDEPENDENT_REJECT`: `READ`/`CLOSE` take a bare (unqualified) table, a
        // bare scan admits only `FIRST`/`NEXT`, the key operator set is `= >= <= > <`, and the
        // value list is non-empty. The trailing edge cases (`AS` with no alias, a bare
        // `HANDLER t` with no verb) are our own grammar boundaries.
        for sql in [
            "HANDLER db.t READ FIRST",
            "HANDLER db.t CLOSE",
            "HANDLER t READ PREV",
            "HANDLER t READ LAST",
            "HANDLER t READ idx <> (1)",
            "HANDLER t READ idx != (1)",
            "HANDLER t READ idx = ()",
            "HANDLER t OPEN AS",
            "HANDLER t",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(MYSQL_RENDER)).is_err(),
                "{sql:?} must reject",
            );
        }
    }

    #[test]
    fn mysql_values_row_constructor_parses_and_round_trips() {
        use crate::dialect::Ansi;
        use crate::render::Renderer;

        // MySQL spells the query-position constructor `VALUES ROW( ... )`; the bare `(...)`
        // row is `ER_PARSE_ERROR`. The `explicit_row` surface tag records the spelling and the
        // `ROW` keyword round-trips.
        let parsed = parse_with(
            "VALUES ROW(1, 2), ROW(3, 4)",
            crate::ParseConfig::new(MYSQL_RENDER),
        )
        .expect("VALUES ROW parses");
        let SetExpr::Values { values, .. } = &query_of(&parsed).body else {
            panic!("expected a VALUES query body");
        };
        assert!(
            values.explicit_row,
            "MySQL rows are the explicit ROW( ... ) form"
        );
        assert_eq!(values.rows.len(), 2);
        assert_eq!(values.rows[0].len(), 2);
        assert_eq!(
            Renderer::new(MYSQL_RENDER)
                .render_parsed(&parsed)
                .expect("VALUES ROW renders"),
            "VALUES ROW(1, 2), ROW(3, 4)",
        );

        // A bare `(...)` row is rejected in query position under MySQL.
        parse_with("VALUES (1, 2)", crate::ParseConfig::new(MYSQL_RENDER))
            .expect_err("MySQL query-position VALUES requires the ROW( ... ) spelling");

        // The bare form is the ANSI/PostgreSQL/DuckDB spelling; `explicit_row` is off there
        // and the constructor round-trips with bare parentheses.
        let bare = parse_with("VALUES (1, 2), (3, 4)", crate::ParseConfig::new(Ansi))
            .expect("bare VALUES parses");
        let SetExpr::Values { values, .. } = &query_of(&bare).body else {
            panic!("expected a VALUES query body");
        };
        assert!(
            !values.explicit_row,
            "the bare form is not the ROW( ... ) constructor"
        );
        assert_eq!(
            Renderer::new(Ansi)
                .render_parsed(&bare)
                .expect("bare VALUES renders"),
            "VALUES (1, 2), (3, 4)",
        );
    }

    /// ANSI plus the `values_rows_require_equal_arity` flag alone, isolating the parse-time
    /// ragged-VALUES reject from every other DuckDb delta.
    const VALUES_ARITY_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.select_syntax(SelectSyntax {
                values_rows_require_equal_arity: true,
                ..SelectSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// The DuckDb preset with the ragged-VALUES gate forced *off*, to prove the flag
    /// alone drives the reject with every other DuckDb delta held constant.
    const DUCKDB_NO_ARITY_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::DUCKDB.with(FeatureDelta::EMPTY.select_syntax(SelectSyntax {
                values_rows_require_equal_arity: false,
                ..SelectSyntax::DUCKDB
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    #[test]
    fn duckdb_rejects_ragged_values_rows_in_every_position() {
        use crate::dialect::DuckDb;
        // A VALUES constructor whose rows differ in width is DuckDB's `VALUES lists must
        // all be the same length` parse error (1.5.4), in the standalone query body, a
        // derived table, and an INSERT source alike.
        for sql in [
            "VALUES (1, 2), (3)",
            "VALUES (1), (1, 2)",
            "SELECT * FROM (VALUES (1, 2), (3)) t",
            "INSERT INTO t VALUES (1, 2), (3)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_err(),
                "DuckDb should reject the ragged VALUES: {sql:?}",
            );
        }
    }

    #[test]
    fn duckdb_accepts_equal_arity_values_rows() {
        use crate::dialect::DuckDb;
        // The gate must not false-positive: equal-width rows, a single row, and a
        // single-column constructor stay accepted in every position.
        for sql in [
            "VALUES (1, 2), (3, 4)",
            "VALUES (1)",
            "VALUES (1), (2), (3)",
            "SELECT * FROM (VALUES (1, 2), (3, 4)) t",
            "INSERT INTO t VALUES (1, 2), (3, 4)",
        ] {
            assert!(
                parse_with(sql, crate::ParseConfig::new(DuckDb)).is_ok(),
                "DuckDb should accept the equal-arity VALUES: {sql:?}",
            );
        }
    }

    #[test]
    fn ragged_values_reject_is_the_arity_gate_alone() {
        use crate::dialect::{DuckDb, Postgres};
        const RAGGED: &str = "VALUES (1, 2), (3)";
        // On (the flag alone over ANSI, and the DuckDb preset) rejects; off (ANSI default,
        // PostgreSQL, and DuckDb with only this flag cleared) accepts — so the reject is
        // this flag's doing and no other DuckDb delta's.
        assert!(parse_with(RAGGED, crate::ParseConfig::new(VALUES_ARITY_DIALECT)).is_err());
        assert!(parse_with(RAGGED, crate::ParseConfig::new(DuckDb)).is_err());
        assert!(parse_with(RAGGED, crate::ParseConfig::new(TestDialect)).is_ok());
        assert!(parse_with(RAGGED, crate::ParseConfig::new(Postgres)).is_ok());
        assert!(parse_with(RAGGED, crate::ParseConfig::new(DUCKDB_NO_ARITY_DIALECT)).is_ok());
    }

    #[test]
    fn with_clause_parses_cte_shape() {
        let parsed = parse_with(
            "WITH c AS (SELECT 1) SELECT * FROM c",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("WITH query parses");
        let Some(with) = &query_of(&parsed).with else {
            panic!("expected a WITH clause");
        };
        assert!(!with.recursive);
        assert_eq!(with.ctes.len(), 1);
        assert_eq!(parsed.resolver().resolve(with.ctes[0].name.sym), "c");
        assert!(with.ctes[0].columns.is_empty());
        assert_eq!(with.ctes[0].materialized, None);
        assert!(matches!(
            with.ctes[0].body.as_query().expect("a query CTE body").body,
            SetExpr::Select { .. },
        ));
    }

    #[test]
    fn recursive_cte_column_list_and_materialization_hint_parse() {
        let parsed = parse_with(
            "WITH RECURSIVE seed(n) AS NOT MATERIALIZED (VALUES (1)) SELECT n FROM seed",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("recursive CTE query parses");
        let Some(with) = &query_of(&parsed).with else {
            panic!("expected a WITH clause");
        };
        assert!(with.recursive);
        assert_eq!(with.ctes.len(), 1);
        let cte = &with.ctes[0];
        assert_eq!(parsed.resolver().resolve(cte.name.sym), "seed");
        assert_eq!(cte.columns.len(), 1);
        assert_eq!(parsed.resolver().resolve(cte.columns[0].sym), "n");
        assert_eq!(cte.materialized, Some(false));
        assert!(matches!(
            cte.body.as_query().expect("a query CTE body").body,
            SetExpr::Values { .. },
        ));
    }

    #[test]
    fn cte_materialized_hint_parses() {
        let parsed = parse_with(
            "WITH seed AS MATERIALIZED (SELECT 1) SELECT * FROM seed",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("MATERIALIZED CTE query parses");
        let cte = &query_of(&parsed).with.as_ref().expect("WITH clause").ctes[0];
        assert_eq!(cte.materialized, Some(true));
    }

    /// PostgreSQL's data-modifying CTE (`data_modifying_ctes`): each of the four
    /// `PreparableStmt` DML heads parses into its [`CteBody`] arm, and the whole
    /// statement round-trips through the Postgres renderer.
    #[test]
    fn data_modifying_cte_bodies_parse_into_their_arms() {
        use crate::dialect::Postgres;
        use crate::render::Renderer;
        let sql = "WITH ins AS (INSERT INTO t VALUES (1) RETURNING *), \
                   upd AS (UPDATE t SET a = 1 RETURNING a), \
                   del AS (DELETE FROM t RETURNING *), \
                   mrg AS (MERGE INTO t USING u ON true WHEN MATCHED THEN DELETE RETURNING *) \
                   SELECT * FROM ins, upd, del, mrg";
        let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
            .expect("all four DML CTE bodies parse");
        let with = query_of(&parsed).with.as_ref().expect("a WITH clause");
        assert!(matches!(with.ctes[0].body, CteBody::Insert { .. }));
        assert!(matches!(with.ctes[1].body, CteBody::Update { .. }));
        assert!(matches!(with.ctes[2].body, CteBody::Delete { .. }));
        assert!(matches!(with.ctes[3].body, CteBody::Merge { .. }));
        assert_eq!(
            Renderer::new(Postgres)
                .render_parsed(&parsed)
                .expect("the DML CTEs render"),
            sql,
        );
    }

    /// The parse boundary matches pg_query 17 (probed): a DML CTE body needs no
    /// `RETURNING` (only its *use* fails, at analysis), and the body may lead with
    /// its own `WITH` clause, which rides the DML statement's `with` slot.
    #[test]
    fn data_modifying_cte_body_without_returning_and_nested_with_parse() {
        use crate::dialect::Postgres;
        parse_with(
            "WITH t AS (INSERT INTO x VALUES (1)) SELECT * FROM t",
            crate::ParseConfig::new(Postgres),
        )
        .expect("a DML CTE body parses without RETURNING");
        let parsed = parse_with(
            "WITH t AS (WITH u AS (SELECT 1) INSERT INTO x SELECT * FROM u RETURNING *) \
             SELECT * FROM t",
            crate::ParseConfig::new(Postgres),
        )
        .expect("a WITH-led DML CTE body parses");
        let with = query_of(&parsed).with.as_ref().expect("a WITH clause");
        let CteBody::Insert { insert, .. } = &with.ctes[0].body else {
            panic!("expected an INSERT CTE body");
        };
        assert!(
            insert.with.is_some(),
            "the nested WITH rides the INSERT's own slot"
        );
    }

    /// `data_modifying_ctes` is off outside PostgreSQL/Lenient, so the DML keyword
    /// after `AS (` is not dispatched and surfaces as the ordinary query-body
    /// reject — matching DuckDB (`A CTE needs a SELECT`, probed on 1.5.4), SQLite,
    /// and MySQL (`ER_PARSE_ERROR`, probed on mysql:8).
    #[test]
    fn query_only_dialects_reject_data_modifying_cte_bodies() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};
        for sql in [
            "WITH t AS (INSERT INTO x VALUES (1) RETURNING *) SELECT * FROM t",
            "WITH t AS (UPDATE x SET a = 1 RETURNING a) SELECT * FROM t",
            "WITH t AS (DELETE FROM x RETURNING *) SELECT * FROM t",
        ] {
            parse_with(sql, crate::ParseConfig::new(Ansi))
                .expect_err("ANSI CTE bodies are queries only");
            parse_with(sql, crate::ParseConfig::new(DuckDb))
                .expect_err("DuckDB CTE bodies are queries only");
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .expect_err("SQLite CTE bodies are queries only");
            parse_with(sql, crate::ParseConfig::new(MySql))
                .expect_err("MySQL CTE bodies are queries only");
        }
    }

    /// The CTE body stays a *closed* `PreparableStmt` set even under PostgreSQL: a
    /// utility statement, a parenthesized DML operand, and DML as a derived table
    /// all keep rejecting, exactly as pg_query 17 does (all five probed).
    #[test]
    fn cte_body_admits_only_preparable_statements() {
        use crate::dialect::Postgres;
        for sql in [
            "WITH t AS (VACUUM) SELECT * FROM t",
            "WITH t AS (COPY x TO STDOUT) SELECT 1",
            "WITH t AS (EXPLAIN SELECT 1) SELECT * FROM t",
            "WITH t AS ((INSERT INTO x VALUES (1))) SELECT * FROM t",
            "SELECT * FROM (INSERT INTO x VALUES (1) RETURNING *) s",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("PostgreSQL parse-rejects {sql:?}"));
        }
    }

    /// SQL:2023 `SEARCH`/`CYCLE` recursive-query clauses (`recursive_search_cycle`): both
    /// SEARCH orders, the CYCLE short form and the `TO … DEFAULT …` form (including a
    /// typed-string constant value), and both clauses in their fixed order all round-trip
    /// through the PostgreSQL renderer.
    #[test]
    fn recursive_search_and_cycle_clauses_parse_and_round_trip() {
        use crate::dialect::Postgres;
        use crate::render::Renderer;
        for sql in [
            "WITH RECURSIVE t(n) AS (SELECT 1) SEARCH DEPTH FIRST BY n SET seq SELECT * FROM t",
            "WITH RECURSIVE t(n, m) AS (SELECT 1, 2) SEARCH BREADTH FIRST BY n, m SET ord SELECT * FROM t",
            "WITH RECURSIVE t(f, u) AS (SELECT 1, 2) CYCLE f, u SET is_cycle USING path SELECT * FROM t",
            "WITH RECURSIVE t(f) AS (SELECT 1) CYCLE f SET c TO true DEFAULT false USING p SELECT * FROM t",
            "WITH RECURSIVE t(f) AS (SELECT 1) CYCLE f SET c TO point '(1,1)' DEFAULT point '(0,0)' USING p SELECT * FROM t",
            "WITH RECURSIVE t(f) AS (SELECT 1) SEARCH DEPTH FIRST BY f SET seq CYCLE f SET c USING p SELECT * FROM t",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|e| panic!("{sql:?} parses: {e}"));
            assert_eq!(
                Renderer::new(Postgres)
                    .render_parsed(&parsed)
                    .expect("the SEARCH/CYCLE query renders"),
                sql,
                "round-trip for {sql:?}",
            );
        }
    }

    /// The SEARCH clause populates [`Cte::search`] and the CYCLE clause [`Cte::cycle`];
    /// the short CYCLE form leaves the `TO … DEFAULT …` mark absent, and a `TO` value is a
    /// restricted constant ([`Expr::Literal`]), never a general expression.
    #[test]
    fn search_cycle_clauses_attach_to_the_cte() {
        use crate::dialect::Postgres;
        let parsed = parse_with(
            "WITH RECURSIVE t(f) AS (SELECT 1) SEARCH BREADTH FIRST BY f SET seq \
             CYCLE f SET c TO 1 DEFAULT 0 USING p SELECT * FROM t",
            crate::ParseConfig::new(Postgres),
        )
        .expect("SEARCH + CYCLE parse");
        let cte = &query_of(&parsed).with.as_ref().expect("a WITH clause").ctes[0];
        let search = cte.search.as_ref().expect("a SEARCH clause");
        assert!(search.breadth_first);
        assert_eq!(search.columns.len(), 1);
        assert_eq!(parsed.resolver().resolve(search.set_column.sym), "seq");
        let cycle = cte.cycle.as_ref().expect("a CYCLE clause");
        assert_eq!(parsed.resolver().resolve(cycle.path_column.sym), "p");
        let mark = cycle.mark.as_ref().expect("a TO/DEFAULT mark");
        assert!(matches!(mark.value.as_ref(), Expr::Literal { .. }));

        // The short form leaves the mark absent.
        let short = parse_with(
            "WITH RECURSIVE t(f) AS (SELECT 1) CYCLE f SET c USING p SELECT * FROM t",
            crate::ParseConfig::new(Postgres),
        )
        .expect("short CYCLE parses");
        assert!(
            query_of(&short).with.as_ref().unwrap().ctes[0]
                .cycle
                .as_ref()
                .unwrap()
                .mark
                .is_none()
        );
    }

    /// The `SEARCH`/`CYCLE` reject shapes match pg_query 17 (probed): a missing order
    /// keyword, a missing `SET`/`USING`, `CYCLE` before `SEARCH`, a repeated clause, a
    /// non-constant `TO`/`DEFAULT` value (parenthesized, signed, a column reference, or a
    /// `CAST`), and `TO` without `DEFAULT` all parse-reject.
    #[test]
    fn recursive_search_cycle_reject_shapes() {
        use crate::dialect::Postgres;
        for sql in [
            "WITH RECURSIVE t(n) AS (SELECT 1) SEARCH FIRST BY n SET seq SELECT * FROM t",
            "WITH RECURSIVE t(n) AS (SELECT 1) SEARCH DEPTH FIRST BY n SELECT * FROM t",
            "WITH RECURSIVE t(n) AS (SELECT 1) CYCLE n SET c SELECT * FROM t",
            "WITH RECURSIVE t(n) AS (SELECT 1) CYCLE n c USING p SELECT * FROM t",
            "WITH RECURSIVE t(n) AS (SELECT 1) CYCLE n SET c USING p SEARCH DEPTH FIRST BY n SET seq SELECT * FROM t",
            "WITH RECURSIVE t(n) AS (SELECT 1) SEARCH DEPTH FIRST BY n SET a SEARCH BREADTH FIRST BY n SET b SELECT * FROM t",
            "WITH RECURSIVE t(n) AS (SELECT 1) CYCLE n SET c TO (1 + 2) DEFAULT (3) USING p SELECT * FROM t",
            "WITH RECURSIVE t(n) AS (SELECT 1) CYCLE n SET c TO -1 DEFAULT 0 USING p SELECT * FROM t",
            "WITH RECURSIVE t(n) AS (SELECT 1) CYCLE n SET c TO x DEFAULT y USING p SELECT * FROM t",
            "WITH RECURSIVE t(n) AS (SELECT 1) CYCLE n SET c TO CAST(1 AS int) DEFAULT 0 USING p SELECT * FROM t",
            "WITH RECURSIVE t(n) AS (SELECT 1) CYCLE n SET c TO true USING p SELECT * FROM t",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .expect_err(&format!("PostgreSQL parse-rejects {sql:?}"));
        }
    }

    /// With `recursive_search_cycle` off, the trailing `SEARCH`/`CYCLE` keyword is left for
    /// the enclosing grammar and surfaces as a parse error: DuckDB parse-rejects both
    /// (`syntax error at or near "SEARCH"`, probed on 1.5.4), as do ANSI (the conservative
    /// baseline), MySQL (whose recursive CTEs have no such clauses), and SQLite — which
    /// rejects *both* clauses at the keyword (`near "SEARCH"/"CYCLE": syntax error`, probed
    /// on the bundled rusqlite 3.53.2). SQLite ships neither the `SEARCH` nor the `CYCLE`
    /// clause at any version (contrary to the `sqlite-cycle-without-search` premise; the
    /// bundled engine rejects `CYCLE … SET … USING …` in every form), so it stays with the
    /// no-clause dialects and the paired flag is not split — no dialect admits one clause
    /// without the other, which is exactly what the paired-flag doctrine requires.
    #[test]
    fn gate_off_dialects_reject_search_cycle_clauses() {
        use crate::dialect::{Ansi, DuckDb, MySql, Sqlite};
        for sql in [
            "WITH RECURSIVE t(n) AS (SELECT 1) SEARCH DEPTH FIRST BY n SET seq SELECT * FROM t",
            "WITH RECURSIVE t(n) AS (SELECT 1) CYCLE n SET c USING p SELECT * FROM t",
        ] {
            parse_with(sql, crate::ParseConfig::new(Ansi)).expect_err("ANSI has no SEARCH/CYCLE");
            parse_with(sql, crate::ParseConfig::new(DuckDb))
                .expect_err("DuckDB parse-rejects SEARCH/CYCLE");
            parse_with(sql, crate::ParseConfig::new(MySql)).expect_err("MySQL has no SEARCH/CYCLE");
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .expect_err("SQLite ships neither SEARCH nor CYCLE");
        }
    }

    /// DuckDB parse-rejects a top-level `ORDER BY`/`LIMIT`/`OFFSET` on a recursive CTE
    /// whose body is a `UNION [ALL]` set operation (`Parser Error: ORDER BY in a recursive
    /// query is not allowed` / `LIMIT or OFFSET …`; probed on 1.5.4), while PostgreSQL
    /// parse-accepts the same shape (the recursion restriction is a binder check there;
    /// probed on pg_query 17). The `MATERIALIZED` and aggregate spellings are the vendored
    /// corpus's full modifier grid, all rejected by the same single rule.
    #[test]
    fn duckdb_recursive_union_query_rejects_order_limit_offset() {
        use crate::dialect::{DuckDb, Postgres};
        for sql in [
            "WITH RECURSIVE t AS (SELECT 1 AS x UNION ALL SELECT x + 1 FROM t WHERE x < 3 ORDER BY x) SELECT * FROM t",
            "WITH RECURSIVE t AS (SELECT 1 AS x UNION ALL SELECT x + 1 FROM t WHERE x < 3 LIMIT 1) SELECT * FROM t",
            "WITH RECURSIVE t AS (SELECT 1 AS x UNION ALL SELECT x + 1 FROM t WHERE x < 3 OFFSET 1) SELECT * FROM t",
            "WITH RECURSIVE t AS (SELECT 1 AS x UNION ALL SELECT x + 1 FROM t WHERE x < 3 LIMIT 1 OFFSET 1) SELECT * FROM t",
            "WITH RECURSIVE t AS (SELECT 1 AS x UNION SELECT sum(x + 1) FROM t WHERE x < 3 ORDER BY x) SELECT * FROM t",
            "WITH RECURSIVE t AS MATERIALIZED (SELECT 1 AS x UNION ALL SELECT x + 1 FROM t WHERE x < 3 LIMIT 1) SELECT * FROM t",
            "WITH RECURSIVE t AS MATERIALIZED (SELECT 1 AS x UNION SELECT sum(x + 1) FROM t WHERE x < 3 OFFSET 1) SELECT * FROM t",
        ] {
            parse_with(sql, crate::ParseConfig::new(DuckDb)).expect_err(&format!(
                "DuckDB parse-rejects the recursive-query modifier: {sql:?}"
            ));
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|e| panic!("PostgreSQL parse-accepts {sql:?}: {e}"));
        }
    }

    /// The DuckDB recursive-query modifier restriction is scoped exactly to a `UNION`-bodied
    /// recursive CTE's own top-level modifier — every near-miss still parses under DuckDb,
    /// each an engine-probed accept (1.5.4). A non-set-op or `INTERSECT`/`EXCEPT` body is not
    /// recursive-eligible; a modifier on the outer query, a parenthesized set-op arm, or a
    /// nested subquery sits on a different query node; and a non-recursive `WITH` is
    /// unrestricted even with a `UNION` body.
    #[test]
    fn recursive_union_modifier_restriction_boundaries() {
        use crate::dialect::DuckDb;
        for sql in [
            // No modifier on the recursive UNION body.
            "WITH RECURSIVE t AS (SELECT 1 AS x UNION ALL SELECT x + 1 FROM t WHERE x < 3) SELECT * FROM t",
            // Recursive CTE with no set-op body — ORDER BY/LIMIT ride the ordinary tail.
            "WITH RECURSIVE t AS (SELECT 1 AS x ORDER BY x) SELECT * FROM t",
            "WITH RECURSIVE t AS (SELECT 1 AS x LIMIT 1) SELECT * FROM t",
            // INTERSECT/EXCEPT bodies are not recursive-eligible, so modifiers stay legal.
            "WITH RECURSIVE t AS (SELECT 1 AS x INTERSECT SELECT 2 ORDER BY x) SELECT * FROM t",
            "WITH RECURSIVE t AS (SELECT 1 AS x EXCEPT SELECT 2 LIMIT 1) SELECT * FROM t",
            // Modifier on the outer query, not the recursive CTE.
            "WITH RECURSIVE t AS (SELECT 1 AS x UNION ALL SELECT x + 1 FROM t WHERE x < 3) SELECT * FROM t ORDER BY x",
            // Modifier on a parenthesized set-op arm — a distinct query node.
            "WITH RECURSIVE t AS ((SELECT 1 AS x LIMIT 1) UNION ALL SELECT x + 1 FROM t WHERE x < 3) SELECT * FROM t",
            // A non-recursive UNION-bodied CTE keeps its modifiers.
            "WITH t AS (SELECT 1 AS x UNION ALL SELECT 2 ORDER BY x) SELECT * FROM t",
        ] {
            parse_with(sql, crate::ParseConfig::new(DuckDb))
                .unwrap_or_else(|e| panic!("DuckDB parse-accepts the boundary case {sql:?}: {e}"));
        }
    }

    #[test]
    fn same_precedence_set_operations_left_associate() {
        // UNION and EXCEPT share one precedence level, so the chain is
        // `(a UNION b) EXCEPT c`.
        let parsed = parse_with(
            "SELECT 1 UNION SELECT 2 EXCEPT SELECT 3",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("parses");
        let SetExpr::SetOperation {
            op: SetOperator::Except,
            left,
            ..
        } = &query_of(&parsed).body
        else {
            panic!("the outermost operator should be EXCEPT");
        };
        assert!(
            matches!(
                **left,
                SetExpr::SetOperation {
                    op: SetOperator::Union,
                    ..
                }
            ),
            "its left arm is the inner UNION",
        );
    }

    #[test]
    fn intersect_binds_tighter_than_union_and_except() {
        let parsed = parse_with(
            "SELECT 1 UNION SELECT 2 INTERSECT SELECT 3",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("mixed set operation parses");
        let SetExpr::SetOperation {
            op: SetOperator::Union,
            right,
            ..
        } = &query_of(&parsed).body
        else {
            panic!("UNION should be the outer operation");
        };
        assert!(
            matches!(
                **right,
                SetExpr::SetOperation {
                    op: SetOperator::Intersect,
                    ..
                }
            ),
            "right arm is the tighter INTERSECT",
        );

        let parsed = parse_with(
            "SELECT 1 INTERSECT SELECT 2 EXCEPT SELECT 3",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("mixed set operation parses");
        let SetExpr::SetOperation {
            op: SetOperator::Except,
            left,
            ..
        } = &query_of(&parsed).body
        else {
            panic!("EXCEPT should be the outer operation");
        };
        assert!(
            matches!(
                **left,
                SetExpr::SetOperation {
                    op: SetOperator::Intersect,
                    ..
                }
            ),
            "left arm is the tighter INTERSECT",
        );
    }

    #[test]
    fn parenthesized_set_operands_group_without_stored_query_when_simple() {
        let parsed = parse_with(
            "SELECT 1 UNION (SELECT 2 EXCEPT SELECT 3)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("parenthesized set operand parses");
        let SetExpr::SetOperation {
            op: SetOperator::Union,
            right,
            ..
        } = &query_of(&parsed).body
        else {
            panic!("UNION should be the outer operation");
        };
        assert!(
            matches!(
                **right,
                SetExpr::SetOperation {
                    op: SetOperator::Except,
                    ..
                }
            ),
            "simple parenthesized operands are grouping only",
        );
    }

    #[test]
    fn parenthesized_set_operands_keep_query_clauses() {
        let parsed = parse_with(
            "SELECT 1 UNION (SELECT 2 ORDER BY 1 LIMIT 1)",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("parenthesized set operand with query clauses parses");
        let SetExpr::SetOperation { right, .. } = &query_of(&parsed).body else {
            panic!("expected a UNION set operation");
        };
        let SetExpr::Query { query, .. } = &**right else {
            panic!("query-level clauses inside parentheses need a query wrapper");
        };
        assert_eq!(query.order_by.len(), 1);
        assert!(query.limit.is_some());
    }

    #[test]
    fn leading_parenthesized_set_operand_parses_as_a_statement() {
        // The renderer emits a leading `(` when a looser-binding set operation
        // sits on the left of a tighter parent; re-parsing must accept it.
        let parsed = parse_with(
            "(SELECT 1 UNION SELECT 2) EXCEPT SELECT 3",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("leading parenthesized set operand parses");
        let SetExpr::SetOperation {
            op: SetOperator::Except,
            left,
            ..
        } = &query_of(&parsed).body
        else {
            panic!("EXCEPT should be the outer operation");
        };
        assert!(
            matches!(
                **left,
                SetExpr::SetOperation {
                    op: SetOperator::Union,
                    ..
                }
            ),
            "the parenthesized UNION is the grouped left operand",
        );
    }

    #[test]
    fn with_clause_then_leading_parenthesized_operand_parses() {
        let parsed = parse_with(
            "WITH x AS (SELECT 1) (SELECT 2 UNION SELECT 3) EXCEPT SELECT 4",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("WITH then a leading parenthesized operand parses");
        let query = query_of(&parsed);
        assert!(query.with.is_some(), "the WITH clause is retained");
        assert!(
            matches!(
                query.body,
                SetExpr::SetOperation {
                    op: SetOperator::Except,
                    ..
                }
            ),
            "the body is the EXCEPT over the parenthesized UNION",
        );
    }

    #[test]
    fn order_by_and_limit_attach_outside_a_set_operation() {
        let parsed = parse_with(
            "SELECT 1 UNION SELECT 2 ORDER BY 1 LIMIT 3",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("query-level clauses parse");
        let query = query_of(&parsed);
        assert!(
            matches!(query.body, SetExpr::SetOperation { .. }),
            "the body stays a bare set operation",
        );
        assert_eq!(query.order_by.len(), 1, "ORDER BY binds the whole query");
        assert!(query.limit.is_some(), "LIMIT binds the whole query");
    }

    #[test]
    fn offset_first_limit_form_is_accepted() {
        let parsed = parse_with(
            "SELECT 1 OFFSET 5 LIMIT 10",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("offset-first");
        let Some(Limit {
            limit: Some(_),
            offset: Some(_),
            syntax: LimitSyntax::LimitOffset,
            ..
        }) = &query_of(&parsed).limit
        else {
            panic!("OFFSET-first still captures both counts");
        };
    }

    #[test]
    fn fetch_first_only_folds_into_the_fetch_first_syntax() {
        let parsed = parse_with(
            "SELECT 1 FETCH FIRST 2 ROWS ONLY",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("FETCH FIRST … ROWS ONLY parses");
        let Some(Limit {
            limit: Some(_),
            offset: None,
            syntax: LimitSyntax::FetchFirst,
            ..
        }) = &query_of(&parsed).limit
        else {
            panic!("FETCH FIRST sets the FetchFirst syntax with no offset");
        };
    }

    #[test]
    fn offset_rows_then_fetch_next_captures_both_counts() {
        // `OFFSET … ROWS` (the ANSI offset, distinguished by `ROWS`) pairs with
        // `FETCH NEXT … ROW ONLY`; `NEXT`/`ROW` are accepted as `FIRST`/`ROWS` aliases.
        let parsed = parse_with(
            "SELECT 1 OFFSET 5 ROWS FETCH NEXT 2 ROW ONLY",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("ANSI offset + fetch parses");
        let Some(Limit {
            limit: Some(_),
            offset: Some(_),
            syntax: LimitSyntax::FetchFirst,
            ..
        }) = &query_of(&parsed).limit
        else {
            panic!("OFFSET ROWS … FETCH NEXT captures both counts as FetchFirst");
        };
    }

    #[test]
    fn ansi_offset_rows_without_fetch_is_fetch_first_syntax() {
        // A bare `OFFSET <n> ROWS` (with the `ROWS` noise word) is the SQL:2008
        // offset, distinct from the PostgreSQL `OFFSET <n>` LIMIT/OFFSET spelling.
        let parsed = parse_with(
            "SELECT 1 OFFSET 5 ROWS",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("ANSI bare offset parses");
        let Some(Limit {
            limit: None,
            offset: Some(_),
            syntax: LimitSyntax::FetchFirst,
            ..
        }) = &query_of(&parsed).limit
        else {
            panic!("OFFSET ROWS with no fetch is a FetchFirst-syntax offset");
        };
    }

    #[test]
    fn fetch_first_is_rejected_without_the_dialect_gate() {
        let err = parse_with(
            "SELECT 1 FETCH FIRST 2 ROWS ONLY",
            crate::ParseConfig::new(NO_FETCH_DIALECT),
        )
        .expect_err("a dialect without fetch_first rejects FETCH FIRST");
        // `FETCH` at bytes 9..14 is leftover after the query and is not a statement.
        assert_eq!(err.span, Span::new(9, 14));
    }

    /// ANSI plus the `limit_percent` flag alone, isolating the DuckDB percentage-`LIMIT`
    /// gate from the rest of the DuckDb preset.
    const LIMIT_PERCENT_DIALECT: FeatureDialect = {
        const FEATURES: FeatureSet =
            FeatureSet::ANSI.with(FeatureDelta::EMPTY.query_tail_syntax(QueryTailSyntax {
                limit_percent: true,
                ..QueryTailSyntax::ANSI
            }));
        FeatureDialect {
            features: &FEATURES,
        }
    };

    /// The percentage-`LIMIT` count and its marker spelling for an accepted parse.
    fn percent_of(parsed: &Parsed) -> (bool, Option<LimitPercent>) {
        let Some(Limit {
            limit,
            offset: _,
            syntax: LimitSyntax::LimitOffset,
            percent,
            ..
        }) = &query_of(parsed).limit
        else {
            panic!("a percentage LIMIT is a LimitOffset-syntax count");
        };
        (limit.is_some(), *percent)
    }

    #[test]
    fn limit_percent_keyword_spelling_is_captured() {
        let parsed = parse_with(
            "SELECT a FROM t LIMIT 40 PERCENT",
            crate::ParseConfig::new(LIMIT_PERCENT_DIALECT),
        )
        .expect("`LIMIT 40 PERCENT` parses under the gate");
        assert_eq!(percent_of(&parsed), (true, Some(LimitPercent::Keyword)));
    }

    #[test]
    fn limit_percent_symbol_spelling_is_captured() {
        // Integer, fractional, and whitespace-before-`%` counts all fold onto the
        // `%` operator spelling.
        for sql in [
            "SELECT a FROM t LIMIT 35%",
            "SELECT a FROM t LIMIT 79.9%",
            "SELECT a FROM t LIMIT 20 %",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(LIMIT_PERCENT_DIALECT))
                .expect("`%` percent parses");
            assert_eq!(
                percent_of(&parsed),
                (true, Some(LimitPercent::Symbol)),
                "{sql}"
            );
        }
    }

    #[test]
    fn limit_percent_composes_with_a_trailing_offset() {
        let parsed = parse_with(
            "SELECT a FROM t LIMIT 40 PERCENT OFFSET 1",
            crate::ParseConfig::new(LIMIT_PERCENT_DIALECT),
        )
        .expect("percentage LIMIT + OFFSET parses");
        let Some(Limit {
            limit: Some(_),
            offset: Some(_),
            percent: Some(LimitPercent::Keyword),
            ..
        }) = &query_of(&parsed).limit
        else {
            panic!("the percentage count still carries an OFFSET tail");
        };
    }

    #[test]
    fn limit_percent_leaves_ordinary_modulo_untouched() {
        // `%` with a right operand is ordinary modulo, never a percentage marker, even
        // under the gate — DuckDB reads `LIMIT 10 % 3` as `LIMIT 1`.
        let parsed = parse_with(
            "SELECT a FROM t LIMIT 10 % 3",
            crate::ParseConfig::new(LIMIT_PERCENT_DIALECT),
        )
        .expect("`LIMIT 10 % 3` is a modulo count");
        let Some(Limit {
            limit:
                Some(Expr::BinaryOp {
                    op: BinaryOperator::Modulo(_),
                    ..
                }),
            percent: None,
            ..
        }) = &query_of(&parsed).limit
        else {
            panic!("a `%` with a right operand stays modulo, not percent");
        };
    }

    #[test]
    fn limit_percent_keyword_rejects_a_non_literal_count() {
        // The `PERCENT` *keyword* folds only onto a bare numeric literal: `LIMIT a
        // PERCENT`, `LIMIT (1 + 1) PERCENT`, and `LIMIT RANDOM() PERCENT` are DuckDB
        // syntax errors, so the marker is left unconsumed (verified on 1.5.4).
        for sql in [
            "SELECT a FROM t LIMIT a PERCENT",
            "SELECT a FROM t LIMIT (1 + 1) PERCENT",
            "SELECT a FROM t LIMIT RANDOM() PERCENT",
        ] {
            parse_with(sql, crate::ParseConfig::new(LIMIT_PERCENT_DIALECT)).expect_err(sql);
        }
    }

    #[test]
    fn limit_percent_symbol_accepts_a_primary_with_postfix_operand() {
        // Unlike the `PERCENT` keyword, the `%` operator folds onto a primary operand
        // DuckDB reduces at multiplicative-or-tighter precedence: a parenthesized
        // expression, function call, subquery, or unary sign (all accepted on 1.5.4).
        // Each is the count of a `Symbol`-spelled percentage limit.
        for sql in [
            "SELECT a FROM t LIMIT (30-10) %",
            "SELECT * FROM t LIMIT RANDOM() %",
            "SELECT * FROM t LIMIT (SELECT d FROM t) %",
            "SELECT a FROM t LIMIT -5 %",
        ] {
            let parsed =
                parse_with(sql, crate::ParseConfig::new(LIMIT_PERCENT_DIALECT)).expect(sql);
            assert_eq!(
                percent_of(&parsed),
                (true, Some(LimitPercent::Symbol)),
                "{sql}"
            );
        }
    }

    #[test]
    fn limit_percent_symbol_folds_postfix_operators_into_the_operand() {
        use crate::dialect::DuckDb;
        // The postfix operators bind tighter than the `%` marker, so a cast or subscript
        // is part of the operand, not left dangling: `LIMIT ?::VARCHAR %` /
        // `LIMIT abs(a)[1] %` parse (DuckDB parse-accepts both on 1.5.4; the postfix
        // features are DuckDB's, absent from the ANSI-based isolation dialect above).
        for sql in [
            "SELECT * FROM t LIMIT ?::VARCHAR %",
            "SELECT a FROM t LIMIT abs(a)[1] %",
        ] {
            parse_with(sql, crate::ParseConfig::new(DuckDb)).expect(sql);
        }
    }

    #[test]
    fn limit_percent_symbol_rejects_a_bare_binary_operand() {
        // The `%` marker reduces only once the operand is complete at multiplicative-or-
        // tighter precedence: a bare additive/concat operand leaves the `%` to shift as a
        // right-operand-less modulo, a DuckDB parser error (`LIMIT 1+2 %` — measured on
        // 1.5.4). The parenthesized form (`LIMIT (1+2) %`) is the accepted spelling.
        for sql in [
            "SELECT a FROM t LIMIT 1+2 %",
            "SELECT a FROM t LIMIT 1||2 %",
        ] {
            parse_with(sql, crate::ParseConfig::new(LIMIT_PERCENT_DIALECT)).expect_err(sql);
        }
    }

    #[test]
    fn limit_percent_is_rejected_without_the_dialect_gate() {
        // ANSI has no percentage LIMIT: the trailing `PERCENT` at bytes 23..30 is leftover
        // after `LIMIT 40` and is not a statement.
        let err = parse_with(
            "SELECT a FROM t LIMIT 40 PERCENT",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("ANSI rejects the percentage LIMIT");
        assert_eq!(err.span, Span::new(25, 32));
    }

    #[test]
    fn order_by_using_operator_records_the_sort_operator() {
        use crate::ast::OrderByUsing;
        use crate::dialect::Postgres;

        // PostgreSQL `ORDER BY <expr> USING <operator>`: the bare symbolic operator and
        // the schema-qualified `OPERATOR(schema.op)` form both land in `using` (not
        // `asc`), preserving the exact operator spelling.
        let parsed = parse_with(
            "SELECT a FROM t ORDER BY a USING <, b USING OPERATOR(pg_catalog.>) NULLS LAST",
            crate::ParseConfig::new(Postgres),
        )
        .expect("ORDER BY USING parses under Postgres");
        let order_by = &query_of(&parsed).order_by;
        assert_eq!(order_by.len(), 2);

        assert_eq!(order_by[0].asc, None, "USING is exclusive with ASC/DESC");
        let OrderByUsing { schema, op, .. } = order_by[0]
            .using
            .as_deref()
            .expect("first key has a USING operator");
        assert!(schema.is_none(), "the bare operator has no schema");
        assert_eq!(parsed.resolver().resolve(*op), "<");

        let OrderByUsing { schema, op, .. } = order_by[1]
            .using
            .as_deref()
            .expect("second key has a USING operator");
        let schema = schema
            .as_ref()
            .expect("OPERATOR(schema.op) keeps its schema");
        assert_eq!(schema.0.len(), 1);
        assert_eq!(parsed.resolver().resolve(schema.0[0].sym), "pg_catalog");
        assert_eq!(parsed.resolver().resolve(*op), ">");
        assert_eq!(
            order_by[1].nulls_first,
            Some(false),
            "NULLS LAST after USING"
        );
    }

    #[test]
    fn order_by_using_is_rejected_without_the_dialect_gate() {
        // ANSI (`TestDialect`) has only ASC/DESC, so `USING` is left unconsumed and the
        // trailing `USING <` fails as leftover input rather than opening a sort form.
        parse_with(
            "SELECT a FROM t ORDER BY a USING <",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("a dialect without order_by_using rejects the USING sort");
    }

    #[test]
    fn order_by_all_parses_as_the_clause_mode() {
        use crate::dialect::DuckDb;

        let parsed = parse_with(
            "SELECT * FROM t ORDER BY ALL",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("ORDER BY ALL parses");
        let query = query_of(&parsed);
        assert!(
            query.order_by.is_empty(),
            "no sort keys — ALL is a mode of the clause, never a key expression"
        );
        let all = query.order_by_all.as_deref().expect("the mode is captured");
        assert_eq!(all.asc, None, "bare ALL records no direction");
        assert_eq!(all.nulls_first, None);
    }

    #[test]
    fn order_by_all_carries_direction_and_nulls_modifiers() {
        use crate::dialect::DuckDb;

        // Probed on DuckDB 1.5.4: `ORDER BY ALL DESC NULLS LAST` is valid — the mode
        // carries the ordinary sort-key modifier surface — and `LIMIT` follows it.
        let parsed = parse_with(
            "SELECT * FROM t ORDER BY ALL DESC NULLS LAST LIMIT 2",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("modifiers and LIMIT compose with the mode");
        let query = query_of(&parsed);
        let all = query.order_by_all.as_deref().expect("the mode is captured");
        assert_eq!(all.asc, Some(false));
        assert_eq!(all.nulls_first, Some(false));
        assert!(query.limit.is_some(), "LIMIT still parses after the mode");
    }

    #[test]
    fn order_by_all_attaches_to_a_whole_set_operation() {
        use crate::dialect::DuckDb;

        let parsed = parse_with(
            "SELECT i FROM t UNION ALL SELECT j FROM t ORDER BY ALL",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("the mode is the query tail of a set operation");
        let query = query_of(&parsed);
        assert!(matches!(query.body, SetExpr::SetOperation { .. }));
        assert!(query.order_by_all.is_some());
    }

    #[test]
    fn a_parenthesized_operand_keeps_its_order_by_all() {
        use crate::dialect::DuckDb;

        // The pure-grouping collapse must treat the mode as a clause: an operand
        // with only `ORDER BY ALL` keeps its `Query` wrapper (DuckDB accepts the
        // operand-level clause; probed on 1.5.4).
        let parsed = parse_with(
            "(SELECT i FROM t ORDER BY ALL) UNION SELECT j FROM t",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("an operand-level ORDER BY ALL parses");
        let SetExpr::SetOperation { left, .. } = &query_of(&parsed).body else {
            panic!("expected a set operation");
        };
        assert!(
            matches!(&**left, SetExpr::Query { query, .. } if query.order_by_all.is_some()),
            "the operand's mode survives the pure-grouping collapse guard"
        );
    }

    #[test]
    fn order_by_all_rejects_mixing_and_the_using_tail() {
        use crate::dialect::DuckDb;

        // Probed on DuckDB 1.5.4: `ALL` admits no sibling keys in either order and
        // no `USING` tail; the mode consumes only its keyword and modifiers, so the
        // leftovers fail as trailing input — the same verdict as the engine.
        assert!(
            parse_with(
                "SELECT * FROM t ORDER BY ALL, i",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT * FROM t ORDER BY i, ALL",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
        assert!(
            parse_with(
                "SELECT * FROM t ORDER BY ALL USING <",
                crate::ParseConfig::new(DuckDb)
            )
            .is_err()
        );
    }

    #[test]
    fn quoted_all_in_order_by_stays_a_sort_key() {
        use crate::dialect::DuckDb;

        // The disambiguation trap: `ALL` is reserved under DuckDB, so a column named
        // `all` must be quoted — and the quoted spelling tokenizes as an identifier,
        // never the mode keyword.
        let parsed = parse_with(
            "SELECT \"all\" FROM t ORDER BY \"all\" DESC",
            crate::ParseConfig::new(DuckDb),
        )
        .expect("parses");
        let query = query_of(&parsed);
        assert!(query.order_by_all.is_none());
        assert_eq!(query.order_by.len(), 1);
        assert_eq!(query.order_by[0].asc, Some(false));
    }

    #[test]
    fn order_by_all_is_rejected_where_the_gate_is_off() {
        use crate::dialect::{Ansi, MySql, Postgres, Sqlite};

        // Every shipped dialect reserves `ALL`, so with the gate off the keyword
        // cannot open a sort expression — the over-acceptance guard on all four.
        let sql = "SELECT a FROM t ORDER BY ALL";
        assert!(
            parse_with(sql, crate::ParseConfig::new(Ansi)).is_err(),
            "ANSI rejects ORDER BY ALL"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Postgres)).is_err(),
            "PostgreSQL rejects ORDER BY ALL"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(MySql)).is_err(),
            "MySQL rejects ORDER BY ALL"
        );
        assert!(
            parse_with(sql, crate::ParseConfig::new(Sqlite)).is_err(),
            "SQLite rejects ORDER BY ALL"
        );
    }

    #[test]
    fn order_by_all_round_trips_each_modifier_spelling() {
        use crate::render::Renderer;

        for sql in [
            "SELECT * FROM t ORDER BY ALL",
            "SELECT * FROM t ORDER BY ALL ASC",
            "SELECT * FROM t ORDER BY ALL DESC",
            "SELECT * FROM t ORDER BY ALL NULLS FIRST",
            "SELECT * FROM t ORDER BY ALL DESC NULLS LAST",
            "SELECT * FROM t ORDER BY ALL LIMIT 2",
        ] {
            let parsed = parse_with(sql, crate::ParseConfig::new(ORDER_BY_ALL_DIALECT))
                .expect("ORDER BY ALL parses");
            assert_eq!(
                Renderer::new(ORDER_BY_ALL_DIALECT)
                    .render_parsed(&parsed)
                    .expect("ORDER BY ALL renders"),
                sql,
            );
        }
    }

    #[test]
    fn qualified_column_keeps_every_dotted_part() {
        let parsed = parse_with("SELECT t.c", crate::ParseConfig::new(TestDialect))
            .expect("qualified column parses");
        let SelectItem::Expr {
            expr: Expr::Column { name, .. },
            ..
        } = &select_of(&parsed).projection[0]
        else {
            panic!("expected a qualified column projection");
        };
        assert_eq!(name.0.len(), 2, "two dotted parts");
        assert_eq!(parsed.resolver().resolve(name.0[0].sym), "t");
        assert_eq!(parsed.resolver().resolve(name.0[1].sym), "c");
    }

    #[test]
    fn where_predicate_is_a_full_expression() {
        let parsed = parse_with(
            "SELECT a FROM t WHERE a > 1",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("where parses");
        assert!(matches!(
            select_of(&parsed).selection,
            Some(Expr::BinaryOp {
                op: BinaryOperator::Gt,
                ..
            }),
        ));
    }

    #[test]
    fn from_with_no_table_errors_at_end_of_input() {
        let err = parse_with("SELECT a FROM", crate::ParseConfig::new(TestDialect))
            .expect_err("FROM needs a relation");
        // The relation is missing at end of input: an empty span past `FROM`.
        assert_eq!(err.span, Span::new(13, 13));
    }

    #[test]
    fn order_by_with_no_expression_errors_at_end_of_input() {
        let err = parse_with("SELECT 1 ORDER BY", crate::ParseConfig::new(TestDialect))
            .expect_err("ORDER BY needs a key");
        assert_eq!(err.span, Span::new(17, 17));
    }

    #[test]
    fn group_by_without_by_reports_the_missing_keyword() {
        let err = parse_with(
            "SELECT a FROM t GROUP a",
            crate::ParseConfig::new(TestDialect),
        )
        .expect_err("GROUP needs BY");
        // `GROUP` at 16..21, `a` at 22..23: the error points at the stray `a`.
        assert_eq!(err.span, Span::new(22, 23));
    }

    #[test]
    fn table_command_dispatches_as_a_query_statement() {
        // `TABLE t` is a query primary (`<explicit table>`), so it routes through the
        // statement dispatcher to a `Statement::Query` whose body is the canonical
        // `SELECT`-shaped operand tagged as a TABLE command.
        let parsed = parse_with("TABLE t", crate::ParseConfig::new(TestDialect))
            .expect("TABLE dispatches as a statement");
        let SetExpr::Select { select, .. } = &query_of(&parsed).body else {
            panic!("expected the canonicalized SELECT body");
        };
        assert_eq!(select.spelling, SelectSpelling::TableCommand);
    }

    #[test]
    fn table_command_composes_in_set_operations() {
        // Each `TABLE` operand is a star-projection Select, so set operations fold over
        // them exactly as over `SELECT` operands (`TABLE a UNION TABLE b`), and the
        // `TABLE` form mixes with a `SELECT` operand.
        let parsed = parse_with(
            "TABLE a UNION TABLE b",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("TABLE set operation parses");
        let SetExpr::SetOperation {
            op: SetOperator::Union,
            left,
            right,
            ..
        } = &query_of(&parsed).body
        else {
            panic!("expected a UNION over two TABLE operands");
        };
        for operand in [left, right] {
            let SetExpr::Select { select, .. } = &**operand else {
                panic!("each operand is a canonicalized SELECT");
            };
            assert_eq!(select.spelling, SelectSpelling::TableCommand);
        }

        let mixed = parse_with(
            "TABLE a UNION ALL SELECT 1",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("TABLE mixes with SELECT operands");
        assert!(matches!(
            query_of(&mixed).body,
            SetExpr::SetOperation {
                op: SetOperator::Union,
                all: true,
                ..
            },
        ));
    }

    #[test]
    fn table_command_binds_query_level_order_and_limit() {
        // `ORDER BY`/`LIMIT` bind the whole query, outside the `TABLE` primary, exactly
        // as they do for a bare `SELECT` body.
        let parsed = parse_with(
            "TABLE t ORDER BY 1 LIMIT 2",
            crate::ParseConfig::new(TestDialect),
        )
        .expect("query-level clauses follow a TABLE command");
        let query = query_of(&parsed);
        assert!(matches!(query.body, SetExpr::Select { .. }));
        assert_eq!(query.order_by.len(), 1);
        assert!(query.limit.is_some());
    }

    #[test]
    fn mysql_locking_clauses_parse_with_of_and_wait_tails() {
        use crate::dialect::MySql;

        // The sole locking clause on a query, with the modern spelling and no tails.
        let update = parse_with(
            "SELECT a FROM t1 FOR UPDATE",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses FOR UPDATE");
        let clause = &query_of(&update).locking[..];
        assert!(matches!(
            clause,
            [crate::ast::LockingClause {
                strength: LockStrength::Update,
                wait: None,
                spelling: LockingSpelling::Modern,
                ..
            }],
        ));
        assert!(clause[0].of.is_empty(), "no OF clause was written");

        // `FOR SHARE` reaches the other shared strength.
        let share = parse_with("SELECT a FROM t1 FOR SHARE", crate::ParseConfig::new(MySql))
            .expect("MySQL parses FOR SHARE");
        assert!(matches!(
            query_of(&share).locking[0].strength,
            LockStrength::Share
        ));

        // `OF <table>` records the single restricted relation.
        let of = parse_with(
            "SELECT a FROM t1 FOR UPDATE OF t1",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses FOR UPDATE OF t1");
        assert_eq!(query_of(&of).locking[0].of.len(), 1);

        // `NOWAIT` and `SKIP LOCKED` wait policies.
        let nowait = parse_with(
            "SELECT a FROM t1 FOR UPDATE NOWAIT",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses NOWAIT");
        assert!(matches!(
            query_of(&nowait).locking[0].wait,
            Some(LockWait::NoWait)
        ));
        let skip = parse_with(
            "SELECT a FROM t1 FOR SHARE SKIP LOCKED",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses SKIP LOCKED");
        assert!(matches!(
            query_of(&skip).locking[0].wait,
            Some(LockWait::SkipLocked)
        ));

        // The legacy `LOCK IN SHARE MODE` folds onto `FOR SHARE` with the spelling tag.
        let legacy = parse_with(
            "SELECT a FROM t1 LOCK IN SHARE MODE",
            crate::ParseConfig::new(MySql),
        )
        .expect("MySQL parses LOCK IN SHARE MODE");
        assert!(matches!(
            query_of(&legacy).locking[0],
            crate::ast::LockingClause {
                strength: LockStrength::Share,
                spelling: LockingSpelling::LockInShareMode,
                ..
            },
        ));
    }

    #[test]
    fn locking_clause_binds_after_limit_and_gates_per_dialect() {
        use crate::dialect::{Ansi, MySql, Postgres};

        // The clause trails `LIMIT` (MySQL's fixed position).
        let after = parse_with(
            "SELECT a FROM t1 LIMIT 1 FOR UPDATE",
            crate::ParseConfig::new(MySql),
        )
        .expect("locking follows LIMIT");
        assert_eq!(query_of(&after).locking.len(), 1);
        assert!(query_of(&after).limit.is_some());
        // Before `LIMIT` is not MySQL's grammar — the leftover `LIMIT` fails the parse.
        parse_with(
            "SELECT a FROM t1 FOR UPDATE LIMIT 1",
            crate::ParseConfig::new(MySql),
        )
        .expect_err("MySQL rejects a locking clause before LIMIT");

        // PostgreSQL shares the modern `FOR UPDATE`/`FOR SHARE` core.
        assert_eq!(
            query_of(
                &parse_with(
                    "SELECT a FROM t1 FOR UPDATE",
                    crate::ParseConfig::new(Postgres)
                )
                .expect("PostgreSQL parses FOR UPDATE")
            )
            .locking
            .len(),
            1,
        );

        // ANSI has no locking gate, so the trailing `FOR` is a clean parse error.
        parse_with("SELECT a FROM t1 FOR UPDATE", crate::ParseConfig::new(Ansi))
            .expect_err("ANSI has no query-tail locking clause");
    }

    #[test]
    fn sqlite_parenthesized_query_operand_gate_is_position_aware() {
        use crate::dialect::{Postgres, Sqlite};

        // SQLite has no parenthesized compound operand (`parenthesized_query_operands`
        // off): a leading `(` in statement / set-op / CTE-body / CTAS / INSERT-source
        // position is a syntax error (engine-measured via rusqlite); PostgreSQL accepts it
        // (`select_with_parens`). Each is bidirectionally checked.
        for sql in [
            "(SELECT 1) UNION (SELECT 2)",
            "(SELECT 1) UNION SELECT 2",
            "SELECT 1 UNION (SELECT 2)",
            "(SELECT 1) ORDER BY x LIMIT 1 OFFSET 1",
            "((SELECT 1) EXCEPT (SELECT 2))",
            "((SELECT 1)) LIMIT 1",
            "WITH a AS ((SELECT 1) UNION ALL (SELECT 2)) SELECT * FROM a",
            "CREATE TABLE foo AS (SELECT 1) UNION ALL (SELECT 2)",
            "CREATE TABLE z AS (WITH cte AS (SELECT 1) SELECT * FROM cte)",
            "INSERT INTO x (SELECT * FROM y)",
        ] {
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .expect_err(&format!("SQLite rejects the paren operand {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("PostgreSQL accepts {sql:?}: {err:?}"));
        }

        // The parenthesized query SQLite *does* admit — a FROM table-or-subquery grouping
        // and an expression scalar subquery — is a complete standalone primary and stays
        // accepted with the flag off (the FROM/scalar grouping context, nested arbitrarily).
        for sql in [
            "SELECT * FROM ((SELECT 1))",
            "SELECT * FROM (((SELECT 1)))",
            "SELECT * FROM ((SELECT 1 UNION SELECT 2))",
            "SELECT * FROM ((SELECT 1)) AS t",
            "SELECT ((SELECT 1))",
            "SELECT (SELECT 1)",
        ] {
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .unwrap_or_else(|err| panic!("SQLite keeps the grouping {sql:?}: {err:?}"));
        }

        // But a grouping paren *extended* by a compound or an `ORDER BY`/`LIMIT` tail is not
        // a standalone primary — SQLite rejects it while PostgreSQL accepts.
        for sql in [
            "SELECT * FROM ((SELECT 1) UNION (SELECT 2))",
            "SELECT * FROM ((SELECT 1) LIMIT 1)",
            "SELECT * FROM (((SELECT 1) UNION SELECT 2) ORDER BY x LIMIT 1 OFFSET 1)",
        ] {
            parse_with(sql, crate::ParseConfig::new(Sqlite))
                .expect_err(&format!("SQLite rejects the extended grouping {sql:?}"));
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("PostgreSQL accepts {sql:?}: {err:?}"));
        }

        // The grouping context does not leak across a `WITH` clause: a grouped
        // `( WITH … <select-body> )` is a valid SQLite subquery, but a `WITH …`-query whose
        // *body* is itself a bare paren operand (`(SELECT 2)`) is a syntax error — the leading
        // `(` after `WITH` is an operand, not a grouping (engine-measured via rusqlite).
        parse_with(
            "SELECT * FROM ((WITH a AS (SELECT 1) SELECT * FROM a))",
            crate::ParseConfig::new(Sqlite),
        )
        .expect("SQLite keeps a grouped WITH-query subquery");
        for sql in [
            "SELECT * FROM ((WITH a AS (SELECT 1) (SELECT 2)))",
            "SELECT ((WITH a AS (SELECT 1) (SELECT 2)))",
        ] {
            parse_with(sql, crate::ParseConfig::new(Sqlite)).expect_err(&format!(
                "SQLite rejects a WITH-query with a paren body {sql:?}"
            ));
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("PostgreSQL accepts {sql:?}: {err:?}"));
        }
    }

    #[test]
    fn postgres_with_ties_requires_order_by() {
        use crate::dialect::Postgres;
        // PostgreSQL's `gram.y` `insertSelectOptions` guards, raised during raw parsing:
        // `WITH TIES` needs a governing `ORDER BY`, and cannot combine with `SKIP LOCKED`.
        parse_with(
            "SELECT a FROM t FETCH FIRST 1 ROW WITH TIES",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("WITH TIES requires ORDER BY");
        parse_with(
            "SELECT a FROM t ORDER BY a FETCH FIRST 1 ROW WITH TIES FOR UPDATE SKIP LOCKED",
            crate::ParseConfig::new(Postgres),
        )
        .expect_err("WITH TIES cannot combine with SKIP LOCKED");
        // The valid forms parse: WITH TIES under an ORDER BY, and WITH TIES with a non-SKIP
        // LOCKED locking clause; `ONLY` (not WITH TIES) admits SKIP LOCKED either way.
        for sql in [
            "SELECT a FROM t ORDER BY a FETCH FIRST 1 ROW WITH TIES",
            "SELECT a FROM t ORDER BY a FETCH FIRST 1 ROW WITH TIES FOR UPDATE",
            "SELECT a FROM t ORDER BY a FETCH FIRST 1 ROW ONLY FOR UPDATE SKIP LOCKED",
        ] {
            parse_with(sql, crate::ParseConfig::new(Postgres))
                .unwrap_or_else(|err| panic!("{sql:?}: {err:?}"));
        }
    }
}
