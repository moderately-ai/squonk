// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Statement-head dispatch — the table-of-arms router for leading keywords.
//!
//! # Ownership
//!
//! Statement-head FeatureSet gates (`utility_syntax.*`, `show_syntax.*`,
//! `maintenance_syntax.*`, `mutation_syntax.merge`/`replace_into`, …) are **owned by
//! this module**: they are consulted here when choosing which family entry to call.
//! Domain modules (`util`, `dml`, `dcl`, `tcl`, `ddl`, …) own parse *bodies* and do not
//! re-check the leading-keyword gate (except where a body needs a finer-grained flag).
//!
//! This is the extract of the former mega-`query::parse_statement_inner` so the
//! query-level grammar (set ops, LIMIT, pipe, CTEs) can stay in [`super::query`]
//! without also owning every SHOW/utility/maintenance head.
//!
//! New statement families add one arm here and their helpers in the owning family
//! module — never a body of family-specific parsing in this router.
//!
//! # Table-driven simple heads
//!
//! Single-keyword, single-gate utility/maintenance heads are dispatched from
//! [`SIMPLE_CONTEXTUAL_HEADS`] / [`try_parse_simple_contextual_head`] so adding a
//! nullary keyword gate is one table row. Multi-word lookaheads (SHOW TABLES,
//! LOAD DATA, PREPARE/EXECUTE duals, LOCK TABLES vs INSTANCE, …) stay as explicit
//! arms below — they are not single-row table entries by nature.

use super::engine::Parser;
use super::{Dialect, HookResult};
use crate::ast::dialect::keyword::Keyword;
use crate::ast::dialect::FeatureSet;
use crate::ast::{Statement, Spanned};
use crate::error::ParseResult;
use crate::tokenizer::Punctuation;

/// A single contextual-keyword statement head: gate + keyword + parse entry.
///
/// `parse` is an associated method on [`Parser`] selected by the match in
/// [`Parser::try_parse_simple_contextual_head`].
#[derive(Clone, Copy)]
enum SimpleContextualHead {
    CommentOn,
    Pragma,
    Kill,
    Shutdown,
    Restart,
    Clone,
    Help,
    Binlog,
    Flush,
    Purge,
    Rename,
    Call,
    Reindex,
    Xa,
}

/// Table of simple (keyword, gate, head) rows — order is try-order among these heads only.
const SIMPLE_CONTEXTUAL_HEADS: &[(&str, fn(&FeatureSet) -> bool, SimpleContextualHead)] = &[
    ("COMMENT", |f| f.utility_syntax.comment_on, SimpleContextualHead::CommentOn),
    ("PRAGMA", |f| f.utility_syntax.pragma, SimpleContextualHead::Pragma),
    ("KILL", |f| f.utility_syntax.kill, SimpleContextualHead::Kill),
    ("SHUTDOWN", |f| f.utility_syntax.shutdown, SimpleContextualHead::Shutdown),
    ("RESTART", |f| f.utility_syntax.restart, SimpleContextualHead::Restart),
    ("CLONE", |f| f.utility_syntax.clone, SimpleContextualHead::Clone),
    ("HELP", |f| f.utility_syntax.help_statement, SimpleContextualHead::Help),
    ("BINLOG", |f| f.utility_syntax.binlog, SimpleContextualHead::Binlog),
    ("FLUSH", |f| f.utility_syntax.flush, SimpleContextualHead::Flush),
    ("PURGE", |f| f.utility_syntax.purge_binary_logs, SimpleContextualHead::Purge),
    ("RENAME", |f| f.utility_syntax.rename_statement, SimpleContextualHead::Rename),
    ("CALL", |f| f.utility_syntax.call, SimpleContextualHead::Call),
    ("REINDEX", |f| f.maintenance_syntax.reindex, SimpleContextualHead::Reindex),
    ("XA", |f| f.transaction_syntax.xa_transactions, SimpleContextualHead::Xa),
];

impl<D: Dialect> Parser<'_, D> {
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

        // Table-driven single-keyword utility/maintenance heads (see SIMPLE_CONTEXTUAL_HEADS).
        if let Some(statement) = self.try_parse_simple_contextual_head()? {
            return Ok(statement);
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
           } else if self.features().utility_syntax.use_statement
            && self.peek_is_keyword(Keyword::Use)?
        {
            // `use_statement` gates the leading `USE` keyword like `pragma`: on for DuckDB and
            // MySQL (and Lenient), off elsewhere, where it falls through to the
            // unknown-statement error. A *non-leading* `USE` is the MySQL index-hint keyword,
            // consumed by the FROM grammar, so it never reaches this statement-leading
            // position. The accepted name arity is dialect data (`use_qualified_name`).
            self.parse_use_statement()
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

    /// Try the [`SIMPLE_CONTEXTUAL_HEADS`] table: one contextual keyword, one gate, one parse.
    fn try_parse_simple_contextual_head(&mut self) -> ParseResult<Option<Statement<D::Ext>>> {
        for &(keyword, gate, head) in SIMPLE_CONTEXTUAL_HEADS {
            // Evaluate the gate before any mutable peek so `features` is not held across
            // `&mut self` borrows.
            if !gate(self.features()) {
                continue;
            }
            if !self.peek_is_contextual_keyword(keyword)? {
                continue;
            }
            let statement = match head {
                SimpleContextualHead::CommentOn => self.parse_comment_on_statement()?,
                SimpleContextualHead::Pragma => self.parse_pragma_statement()?,
                SimpleContextualHead::Kill => self.parse_kill_statement()?,
                SimpleContextualHead::Shutdown => self.parse_shutdown_statement()?,
                SimpleContextualHead::Restart => self.parse_restart_statement()?,
                SimpleContextualHead::Clone => self.parse_clone_statement()?,
                SimpleContextualHead::Help => self.parse_help_statement()?,
                SimpleContextualHead::Binlog => self.parse_binlog_statement()?,
                SimpleContextualHead::Flush => self.parse_flush_statement()?,
                SimpleContextualHead::Purge => self.parse_purge_statement()?,
                SimpleContextualHead::Rename => self.parse_rename_statement()?,
                SimpleContextualHead::Call => self.parse_call_statement()?,
                SimpleContextualHead::Reindex => self.parse_reindex_statement()?,
                SimpleContextualHead::Xa => self.parse_xa_statement()?,
            };
            return Ok(Some(statement));
        }
        Ok(None)
    }
}
