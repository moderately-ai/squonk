// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The top-level `Statement` enum unifying every statement AST node.

use super::{
    AccessControlStatement, AlterDatabase, AlterDatabaseOptions, AlterEvent, AlterExtension,
    AlterInstance, AlterLogfileGroup, AlterObjectDepends, AlterObjectSchema, AlterResourceGroup,
    AlterRoutine, AlterSequence, AlterServer, AlterSystem, AlterTable, AlterTablespace, AlterUser,
    AlterView, AnalyzeStatement, AttachStatement, BinlogStatement, CacheIndexStatement,
    CallStatement, CaseStatement, CheckpointStatement, CloneStatement, CloseCursorStatement,
    CommentOnStatement, CompoundStatement, CopyIntoStatement, CopyStatement, CreateDatabase,
    CreateEvent, CreateExtension, CreateFunction, CreateIndex, CreateLogfileGroup, CreateMacro,
    CreateProcedure, CreateResourceGroup, CreateSchema, CreateSecret, CreateSequence, CreateServer,
    CreateSpatialReferenceSystem, CreateStoredTrigger, CreateTable, CreateTablespace,
    CreateTrigger, CreateType, CreateUser, CreateView, CreateVirtualTable, DeallocateStatement,
    Delete, DescribeStatement, DetachStatement, DoExpressionsStatement, DoStatement, DropBehavior,
    DropDatabase, DropEvent, DropIndexOnTable, DropLogfileGroup, DropResourceGroup, DropSecretStmt,
    DropServer, DropSpatialReferenceSystem, DropStatement, DropTablespace, DropTransform,
    ExecuteStatement, ExecuteUsingStatement, ExplainStatement, ExportStatement, Extension,
    FetchCursorStatement, FlushStatement, GetDiagnosticsStatement, HandlerStatement, HelpStatement,
    IfStatement, ImportStatement, ImportTableStatement, Insert, InstallStatement,
    InstanceLockStatement, IterateStatement, KillStatement, LeaveStatement, LoadDataStatement,
    LoadIndexStatement, LoadStatement, LockTablesStatement, LoopStatement, Merge, NoExt,
    ObjectName, OpenCursorStatement, Pivot, PragmaStatement, PrepareFromStatement,
    PrepareStatement, PurgeStatement, Query, ReindexStatement, RenameStatement, RepeatStatement,
    ReplicationStatement, ReturnStatement, RoutineObjectKind, RoutineSignature, SessionStatement,
    ShowRef, ShowStatement, SignalStatement, TableMaintenanceStatement, ThinVec,
    TransactionStatement, UninstallStatement, UnlockTablesStatement, Unpivot, Update,
    UpdateExtensionsStatement, UseStatement, UserRoleList, VacuumStatement, WhileStatement,
    XaStatement,
};
use crate::vocab::Meta;

#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-deserialize", derive(serde::Deserialize))]
/// The SQL statement forms represented by the AST.
pub enum Statement<X: Extension = NoExt> {
    /// A `SELECT` / query statement.
    Query {
        /// Query governed by this node.
        query: Box<Query<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `CREATE TABLE` statement.
    CreateTable {
        /// The `CREATE TABLE` details; see [`CreateTable`].
        create: Box<CreateTable<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An `ALTER TABLE` statement.
    AlterTable {
        /// The `ALTER TABLE` details; see [`AlterTable`].
        alter: Box<AlterTable<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `DROP` statement.
    Drop {
        /// The `DROP` details; see [`DropStatement`].
        drop: Box<DropStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `CREATE SCHEMA` statement.
    CreateSchema {
        /// The `CREATE SCHEMA` details; see [`CreateSchema`].
        schema: Box<CreateSchema<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `CREATE VIEW` statement.
    CreateView {
        /// The `CREATE VIEW` details; see [`CreateView`].
        view: Box<CreateView<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `ALTER [ALGORITHM = …] [DEFINER = …] [SQL SECURITY …] VIEW …` statement — the
    /// view redefinition, kept apart from [`AlterTable`](Self::AlterTable) and the DuckDB
    /// `ALTER … SET SCHEMA` relocation. Boxed to keep the enum within its size budget.
    AlterView {
        /// The `ALTER VIEW` details; see [`AlterView`].
        alter: Box<AlterView<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `CREATE INDEX` statement.
    CreateIndex {
        /// The `CREATE INDEX` details; see [`CreateIndex`].
        index: Box<CreateIndex<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `CREATE [OR REPLACE] FUNCTION` statement. Boxed, like the other family
    /// payloads, to keep the enum within its size budget.
    CreateFunction {
        /// The `CREATE FUNCTION` details; see [`CreateFunction`].
        create: Box<CreateFunction<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `CREATE [DEFINER = …] PROCEDURE …` statement — the stored-procedure
    /// definition, kept apart from [`CreateFunction`](Self::CreateFunction) (no `RETURNS`,
    /// a `CALL`-invoked body that rejects `RETURN`). Boxed to keep the enum within its size
    /// budget.
    CreateProcedure {
        /// The `CREATE PROCEDURE` details; see [`CreateProcedure`].
        create: Box<CreateProcedure<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `ALTER {PROCEDURE | FUNCTION} <name> [<characteristic> …]` statement — the
    /// routine-characteristics alteration (no body). Boxed to keep the enum within its size
    /// budget.
    AlterRoutine {
        /// The `ALTER PROCEDURE`/`ALTER FUNCTION` details; see [`AlterRoutine`].
        alter: Box<AlterRoutine<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `CREATE [DEFINER = …] EVENT …` statement — the scheduled-event definition.
    /// Boxed to keep the enum within its size budget.
    CreateEvent {
        /// The `CREATE EVENT` details; see [`CreateEvent`].
        create: Box<CreateEvent<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `ALTER [DEFINER = …] EVENT …` statement — the scheduled-event alteration.
    /// Boxed to keep the enum within its size budget.
    AlterEvent {
        /// The `ALTER EVENT` details; see [`AlterEvent`].
        alter: Box<AlterEvent<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `DROP EVENT [IF EXISTS] <name>` statement — kept apart from
    /// [`Drop`](Self::Drop) because an event drop names exactly one event and takes no
    /// `CASCADE`/`RESTRICT`. Boxed to keep the enum within its size budget.
    DropEvent {
        /// The `DROP EVENT` details; see [`DropEvent`].
        drop: Box<DropEvent>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `DROP {DATABASE | SCHEMA} [IF EXISTS] <name>` statement — kept apart from
    /// [`Drop`](Self::Drop) because it names exactly one unqualified database and takes no
    /// `CASCADE`/`RESTRICT`, and because `DATABASE`/`SCHEMA` are synonyms here (unlike the
    /// shared name-list `DROP SCHEMA`). Boxed to keep the enum within its size budget.
    DropDatabase {
        /// The `DROP DATABASE`/`DROP SCHEMA` details; see [`DropDatabase`].
        drop: Box<DropDatabase>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `DROP INDEX <name> ON <table> [ALGORITHM …] [LOCK …]` statement — kept apart
    /// from [`Drop`](Self::Drop) because it names the owning table with a mandatory `ON` and
    /// carries the online-DDL `ALGORITHM`/`LOCK` hints. Boxed to keep the enum within its
    /// size budget.
    DropIndex {
        /// The `DROP INDEX … ON …` details; see [`DropIndexOnTable`].
        drop: Box<DropIndexOnTable>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `CREATE DATABASE` statement.
    CreateDatabase {
        /// The `CREATE DATABASE` details; see [`CreateDatabase`].
        create: Box<CreateDatabase>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `DROP {FUNCTION | PROCEDURE | ROUTINE} <signature> [, ...]` statement — the
    /// routine drop, kept apart from [`Drop`](Self::Drop) because a routine is named
    /// by an argument-type signature ([`RoutineSignature`]), not a plain name.
    DropRoutine {
        /// Which routine kind is dropped (`FUNCTION`/`PROCEDURE`/`ROUTINE`); see [`RoutineObjectKind`].
        kind: RoutineObjectKind,
        /// Whether the if exists form was present in the source.
        if_exists: bool,
        /// routines in source order.
        routines: ThinVec<RoutineSignature<X>>,
        /// Optional behavior for this syntax.
        behavior: Option<super::DropBehavior>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL `DROP TRANSFORM [IF EXISTS] FOR <type> LANGUAGE <lang> [CASCADE |
    /// RESTRICT]` statement (`DropTransformStmt`), kept apart from [`Drop`](Self::Drop)
    /// because a transform is named by a `(type, language)` pair (an
    /// [`ObjectReference::Transform`](super::ObjectReference)), not a plain name list.
    /// Gated by [`StatementDdlGates::transform_ddl`](crate::dialect::StatementDdlGates::transform_ddl).
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    DropTransform {
        /// The `DROP TRANSFORM` details; see [`DropTransform`].
        drop: Box<DropTransform<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `TRUNCATE [TABLE] <name> [, ...] [RESTART IDENTITY | CONTINUE IDENTITY]
    /// [CASCADE | RESTRICT]` statement. Standard SQL:2008 (F200) and accepted by every
    /// shipped dialect, so it carries no [`FeatureSet`](crate::dialect::FeatureSet) gate.
    /// The optional `TABLE` keyword is exact-synonym sugar; a
    /// [`table_keyword`](Self::Truncate::table_keyword) tag records whether it was
    /// written so a source-fidelity render replays it (the canonical render emits
    /// `TRUNCATE TABLE`). Inline-field, like [`DropRoutine`](Self::DropRoutine): a table
    /// list plus its flags carries no expressions or extension nodes.
    Truncate {
        /// Tables in source order.
        tables: ThinVec<ObjectName>,
        /// Whether the optional `TABLE` keyword was written (`TRUNCATE TABLE t` vs the
        /// bare `TRUNCATE t`). Fidelity only; the canonical render emits `TABLE`.
        table_keyword: bool,
        /// `Some(true)` = `RESTART IDENTITY`, `Some(false)` = `CONTINUE IDENTITY`, `None`
        /// = neither clause written. PostgreSQL collapses the absent and `CONTINUE` forms
        /// (both leave sequences untouched); the tag keeps them distinct so they
        /// round-trip.
        restart_identity: Option<bool>,
        /// Optional behavior for this syntax.
        behavior: Option<DropBehavior>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `COMMENT ON <object> IS '<text>' | NULL` object-metadata statement
    /// (PostgreSQL-specific; gated by
    /// [`UtilitySyntax::comment_on`](crate::dialect::UtilitySyntax)). Boxed, like the
    /// other family payloads, to keep the enum within its size budget; the fields live on
    /// [`CommentOnStatement`].
    CommentOn {
        /// The `COMMENT ON` details; see [`CommentOnStatement`].
        comment: Box<CommentOnStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An `INSERT` statement.
    Insert {
        /// The `INSERT` details; see [`Insert`].
        insert: Box<Insert<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An `UPDATE` statement.
    Update {
        /// The `UPDATE` details; see [`Update`].
        update: Box<Update<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `DELETE` statement.
    Delete {
        /// The `DELETE` details; see [`Delete`].
        delete: Box<Delete<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `MERGE INTO ... USING ... WHEN [NOT] MATCHED ...` statement (SQL:2003).
    /// Boxed, like the other DML payloads, to keep the enum within its size budget.
    Merge {
        /// The `MERGE` details; see [`Merge`].
        merge: Box<Merge<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A transaction-control statement (`BEGIN`/`COMMIT`/`ROLLBACK`/…). Boxed, like
    /// the other family payloads, to keep the enum within its size budget.
    Transaction {
        /// The transaction-control details (`BEGIN`/`COMMIT`/…); see [`TransactionStatement`].
        transaction: Box<TransactionStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `XA` distributed-transaction statement (gated by
    /// [`UtilitySyntax::xa_transactions`](crate::dialect::UtilitySyntax)) — the X/Open XA
    /// two-phase-commit verbs, a family distinct from the ANSI
    /// [`Transaction`](Self::Transaction) control statements. Boxed, like the other family
    /// payloads, to keep the enum within its size budget; see [`XaStatement`].
    Xa {
        /// The `XA` statement details; see [`XaStatement`].
        xa: Box<XaStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A session statement (`SET`/`RESET`/`SET ROLE`/…).
    Session {
        /// The session `SET`/`RESET` details; see [`SessionStatement`].
        session: Box<SessionStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `GRANT`/`REVOKE` access-control statement.
    AccessControl {
        /// The `GRANT`/`REVOKE` details; see [`AccessControlStatement`].
        access: Box<AccessControlStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `COPY` statement.
    Copy {
        /// The `COPY` details; see [`CopyStatement`].
        copy: Box<CopyStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A Snowflake `COPY INTO <target> FROM <source> ...` bulk load/unload statement
    /// (gated by [`UtilitySyntax::copy_into`](crate::dialect::UtilitySyntax)). A sibling
    /// of [`Copy`](Self::Copy) rather than a variant of it — the two share only the
    /// `COPY` keyword; see [`CopyIntoStatement`].
    CopyInto {
        /// The `COPY INTO` details; see [`CopyIntoStatement`].
        copy: Box<CopyIntoStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `EXPORT DATABASE ['<db>' TO] '<path>' [<opts>]` catalogue-dump statement
    /// (gated — with its [`Import`](Self::Import) inverse — by
    /// [`UtilitySyntax::export_import_database`](crate::dialect::UtilitySyntax)). First-class
    /// for the same builtin-blind-seam reason as [`Pragma`](Self::Pragma). Boxed, like the
    /// other family payloads, to keep the enum within its size budget; see [`ExportStatement`].
    Export {
        /// The `EXPORT DATABASE` details; see [`ExportStatement`].
        export: Box<ExportStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `IMPORT DATABASE '<path>'` catalogue-replay statement — the
    /// [`Export`](Self::Export) inverse, sharing its gate; see [`ImportStatement`].
    Import {
        /// The `IMPORT DATABASE` details; see [`ImportStatement`].
        import: Box<ImportStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// An `EXPLAIN` / `EXPLAIN ANALYZE` query-plan statement (also spelled `DESCRIBE` /
    /// `DESC` under MySQL — the [`spelling`](ExplainStatement::spelling) tag records which).
    Explain {
        /// The `EXPLAIN` details; see [`ExplainStatement`].
        explain: Box<ExplainStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `{DESCRIBE | DESC | EXPLAIN} <table> [<column> | '<pattern>']`
    /// table-metadata statement (gated by
    /// [`ShowSyntax::describe`](crate::dialect::UtilitySyntax)); the MySQL overload of
    /// the EXPLAIN keyword that describes a table rather than planning a query. First-class
    /// for the same builtin-blind-seam reason as [`Pragma`](Self::Pragma). Boxed, like the
    /// other family payloads, to keep the enum within its size budget.
    Describe {
        /// The `DESCRIBE` details; see [`DescribeStatement`].
        describe: Box<DescribeStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A typed `SHOW TABLES` catalogue-listing statement (MySQL/DuckDB; gated by
    /// [`ShowSyntax::show_tables`](crate::dialect::UtilitySyntax)) — distinct from the
    /// generic session `SHOW <var>` ([`Session`](Self::Session)). Opener of the typed-`SHOW`
    /// family; first-class for the same builtin-blind-seam reason as [`Pragma`](Self::Pragma).
    /// Boxed, like the other family payloads, to keep the enum within its size budget; the
    /// fields live on [`ShowStatement`].
    Show {
        /// The `SHOW` details; see [`ShowStatement`].
        show: Box<ShowStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `KILL [CONNECTION | QUERY] <id>` thread/query-termination statement (gated
    /// by [`UtilitySyntax::kill`](crate::dialect::UtilitySyntax)); first-class for the same
    /// builtin-blind-seam reason as [`Pragma`](Self::Pragma). Boxed, like the other family
    /// payloads, to keep the enum within its size budget.
    Kill {
        /// The `KILL` details; see [`KillStatement`].
        kill: Box<KillStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `HANDLER` low-level cursor statement — `OPEN`/`READ`/`CLOSE` direct
    /// storage-engine access (gated by
    /// [`UtilitySyntax::handler_statements`](crate::dialect::UtilitySyntax)); see
    /// [`HandlerStatement`]. Boxed, like the other family payloads, to keep the enum within
    /// its size budget.
    Handler {
        /// The `HANDLER` details; see [`HandlerStatement`].
        handler: Box<HandlerStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `INSTALL PLUGIN`/`INSTALL COMPONENT` server-administration statement (gated by
    /// [`UtilitySyntax::plugin_component_statements`](crate::dialect::UtilitySyntax)); see
    /// [`InstallStatement`]. Boxed, like the other family payloads, to keep the enum within
    /// its size budget.
    Install {
        /// The `INSTALL` details; see [`InstallStatement`].
        install: Box<InstallStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `UNINSTALL PLUGIN`/`UNINSTALL COMPONENT` server-administration statement, the
    /// inverse of [`Install`](Self::Install) (gated by the same
    /// [`UtilitySyntax::plugin_component_statements`](crate::dialect::UtilitySyntax)); see
    /// [`UninstallStatement`]. Boxed, like the other family payloads, to keep the enum within
    /// its size budget.
    Uninstall {
        /// The `UNINSTALL` details; see [`UninstallStatement`].
        uninstall: Box<UninstallStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `SHUTDOWN` server-shutdown statement (gated by
    /// [`UtilitySyntax::shutdown`](crate::dialect::UtilitySyntax)) — a nullary leading keyword
    /// with no operand (`SHUTDOWN 1` is `ER_PARSE_ERROR` on mysql:8), so it carries no payload
    /// node.
    Shutdown {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `RESTART` server-restart statement (gated by
    /// [`UtilitySyntax::restart`](crate::dialect::UtilitySyntax)) — a nullary leading keyword
    /// with no operand (`RESTART 1` is `ER_PARSE_ERROR` on mysql:8), so it carries no payload
    /// node.
    Restart {
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `CLONE` local/remote data-directory provisioning statement (gated by
    /// [`UtilitySyntax::clone`](crate::dialect::UtilitySyntax)); see [`CloneStatement`]. Boxed,
    /// like the other family payloads, to keep the enum within its size budget.
    Clone {
        /// The `CLONE` details; see [`CloneStatement`].
        clone: Box<CloneStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `IMPORT TABLE FROM '<file>' [, …]` tablespace-import statement (gated by
    /// [`UtilitySyntax::import_table`](crate::dialect::UtilitySyntax)); see
    /// [`ImportTableStatement`]. Distinct from the DuckDB `IMPORT DATABASE`
    /// ([`Import`](Self::Import)). Boxed, like the other family payloads, to keep the enum
    /// within its size budget.
    ImportTable {
        /// The `IMPORT TABLE` details; see [`ImportTableStatement`].
        import_table: Box<ImportTableStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `HELP '<topic>'` help-lookup statement (gated by
    /// [`UtilitySyntax::help_statement`](crate::dialect::UtilitySyntax)); see [`HelpStatement`].
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    Help {
        /// The `HELP` details; see [`HelpStatement`].
        help: Box<HelpStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `BINLOG '<base64-event>'` binary-log-event replay statement (gated by
    /// [`UtilitySyntax::binlog`](crate::dialect::UtilitySyntax)); see [`BinlogStatement`].
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    Binlog {
        /// The `BINLOG` details; see [`BinlogStatement`].
        binlog: Box<BinlogStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQLite `PRAGMA` configuration statement (gated by
    /// [`UtilitySyntax::pragma`](crate::dialect::UtilitySyntax)). A first-class
    /// variant rather than an [`Other`](Self::Other) extension because the shipped
    /// builtin dialects are `NoExt` — the `Other(X)` seam is reachable only by an
    /// out-of-tree dialect with its own `Ext`, never by a builtin. Boxed, like the
    /// other family payloads, to keep the enum within its size budget.
    Pragma {
        /// The `PRAGMA` details; see [`PragmaStatement`].
        pragma: Box<PragmaStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQLite `ATTACH [DATABASE] <expr> AS <schema>` statement (gated by
    /// [`UtilitySyntax::attach`](crate::dialect::UtilitySyntax)); first-class for
    /// the same builtin-blind-seam reason as [`Pragma`](Self::Pragma).
    Attach {
        /// The `ATTACH` details; see [`AttachStatement`].
        attach: Box<AttachStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `DETACH [DATABASE] [IF EXISTS] <schema>` statement — the
    /// [`Attach`](Self::Attach) inverse, sharing its gate.
    Detach {
        /// The `DETACH` details; see [`DetachStatement`].
        detach: Box<DetachStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `[FORCE] CHECKPOINT [<database>]` write-ahead-log flush statement
    /// (PostgreSQL/DuckDB; gated by
    /// [`MaintenanceSyntax::checkpoint`](crate::dialect::UtilitySyntax)). First-class for
    /// the same builtin-blind-seam reason as [`Pragma`](Self::Pragma). Boxed, like the
    /// other family payloads, to keep the enum within its size budget.
    Checkpoint {
        /// The `CHECKPOINT` details; see [`CheckpointStatement`].
        checkpoint: Box<CheckpointStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `LOAD <extension>` extension/shared-library load statement (PostgreSQL/DuckDB;
    /// gated by [`UtilitySyntax::load_extension`](crate::dialect::UtilitySyntax)).
    /// First-class for the same builtin-blind-seam reason as [`Pragma`](Self::Pragma).
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    Load {
        /// The `LOAD` details; see [`LoadStatement`].
        load: Box<LoadStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `LOAD {DATA | XML} … INFILE … INTO TABLE …` bulk-import statement (gated by
    /// [`UtilitySyntax::load_data`](crate::dialect::UtilitySyntax)) — a DIFFERENT behaviour on
    /// the leading `LOAD` keyword from PostgreSQL/DuckDB's extension [`Load`](Self::Load) form
    /// (the two gates are never both armed in one preset, dispatched on the two-word
    /// `LOAD DATA`/`LOAD XML` lookahead); see [`LoadDataStatement`]. Boxed, like the other
    /// family payloads, to keep the enum within its size budget.
    LoadData {
        /// The `LOAD DATA`/`LOAD XML` details; see [`LoadDataStatement`].
        load_data: Box<LoadDataStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `UPDATE EXTENSIONS [( <name>, ... )]` extension-refresh statement
    /// (gated by [`UtilitySyntax::update_extensions`](crate::dialect::UtilitySyntax));
    /// first-class for the same builtin-blind-seam reason as [`Pragma`](Self::Pragma).
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    UpdateExtensions {
        /// The `UPDATE EXTENSIONS` details; see [`UpdateExtensionsStatement`].
        update_extensions: Box<UpdateExtensionsStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQLite `VACUUM [<schema>] [INTO <expr>]` maintenance statement (gated by
    /// [`MaintenanceSyntax::vacuum`](crate::dialect::UtilitySyntax)); first-class for the
    /// same builtin-blind-seam reason as [`Pragma`](Self::Pragma). Boxed, like the
    /// other family payloads, to keep the enum within its size budget.
    Vacuum {
        /// The `VACUUM` details; see [`VacuumStatement`].
        vacuum: Box<VacuumStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQLite `REINDEX [<name>]` index-rebuild statement (gated by
    /// [`MaintenanceSyntax::reindex`](crate::dialect::UtilitySyntax)).
    Reindex {
        /// The `REINDEX` details; see [`ReindexStatement`].
        reindex: Box<ReindexStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQLite `ANALYZE [<name>]` statistics statement (gated by
    /// [`MaintenanceSyntax::analyze`](crate::dialect::UtilitySyntax)).
    Analyze {
        /// The `ANALYZE` details; see [`AnalyzeStatement`].
        analyze: Box<AnalyzeStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL admin-table maintenance statement `{ANALYZE | CHECK | CHECKSUM | OPTIMIZE
    /// | REPAIR} {TABLE | TABLES} <list> [options]` (gated by
    /// [`MaintenanceSyntax::table_maintenance`](crate::dialect::MaintenanceSyntax)).
    /// Distinct from the SQLite [`Analyze`](Self::Analyze) leading-`ANALYZE` statement:
    /// MySQL's `ANALYZE` always takes `TABLE`, so the dispatch reserves the bare form for
    /// the SQLite/DuckDB sibling. Boxed, like the other family payloads, to keep the enum
    /// within its size budget.
    TableMaintenance {
        /// The maintenance details; see [`TableMaintenanceStatement`].
        table_maintenance: Box<TableMaintenanceStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `CACHE INDEX <t> [<keys>][, ...] [PARTITION (...)] IN <cache>` statement —
    /// assign a table's indexes to a named key cache (gated by
    /// [`UtilitySyntax::key_cache_statements`](crate::dialect::UtilitySyntax)); see
    /// [`CacheIndexStatement`]. Boxed, like the other family payloads, to keep the enum
    /// within its size budget.
    CacheIndex {
        /// The `CACHE INDEX` details; see [`CacheIndexStatement`].
        cache_index: Box<CacheIndexStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `LOAD INDEX INTO CACHE <t> [PARTITION (...)] [<keys>] [IGNORE LEAVES][, ...]`
    /// statement — preload a table's index blocks into its key cache; the
    /// [`CacheIndex`](Self::CacheIndex) preload sibling, sharing the
    /// [`key_cache_statements`](crate::dialect::UtilitySyntax) gate. See
    /// [`LoadIndexStatement`]. Distinct from the `LOAD <extension>`
    /// [`Load`](Self::Load) statement (a different grammar and gate). Boxed, like the other
    /// family payloads, to keep the enum within its size budget.
    LoadIndex {
        /// The `LOAD INDEX INTO CACHE` details; see [`LoadIndexStatement`].
        load_index: Box<LoadIndexStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL standalone `RENAME TABLE`/`RENAME USER` object-rename statement (gated by
    /// [`UtilitySyntax::rename_statement`](crate::dialect::UtilitySyntax)); first-class
    /// for the same builtin-blind-seam reason as [`Pragma`](Self::Pragma). Boxed, like the
    /// other family payloads, to keep the enum within its size budget.
    Rename {
        /// The `RENAME` details; see [`RenameStatement`].
        rename: Box<RenameStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `FLUSH [NO_WRITE_TO_BINLOG | LOCAL] <target>` server-administration statement
    /// (gated by [`UtilitySyntax::flush`](crate::dialect::UtilitySyntax)); first-class for
    /// the same builtin-blind-seam reason as [`Pragma`](Self::Pragma). Boxed, like the other
    /// family payloads, to keep the enum within its size budget.
    Flush {
        /// The `FLUSH` details; see [`FlushStatement`].
        flush: Box<FlushStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `PURGE BINARY LOGS {TO '<log>' | BEFORE <datetime>}` binary-log purge
    /// statement (gated by
    /// [`UtilitySyntax::purge_binary_logs`](crate::dialect::UtilitySyntax)); first-class for
    /// the same builtin-blind-seam reason as [`Pragma`](Self::Pragma). Boxed, like the other
    /// family payloads, to keep the enum within its size budget.
    Purge {
        /// The `PURGE` details; see [`PurgeStatement`].
        purge: Box<PurgeStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL replication-administration statement (gated by
    /// [`UtilitySyntax::replication_statements`](crate::dialect::UtilitySyntax)) — `CHANGE
    /// REPLICATION SOURCE/FILTER`, `START`/`STOP REPLICA`, and `START`/`STOP
    /// GROUP_REPLICATION`. Boxed, like the other family payloads, to keep the enum within its
    /// size budget; see [`ReplicationStatement`].
    Replication {
        /// The replication-statement details; see [`ReplicationStatement`].
        replication: Box<ReplicationStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `CREATE USER …` account-creation statement (gated by
    /// [`AccessControlSyntax::user_role_management`](crate::dialect::AccessControlSyntax)).
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    CreateUser {
        /// The account-creation details; see [`CreateUser`].
        create: Box<CreateUser>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `ALTER USER …` account-modification statement (gated by
    /// [`AccessControlSyntax::user_role_management`](crate::dialect::AccessControlSyntax)).
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    AlterUser {
        /// The account-modification details; see [`AlterUser`].
        alter: Box<AlterUser>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `DROP USER` / `CREATE ROLE` / `DROP ROLE` account-or-role-list statement
    /// (gated by
    /// [`AccessControlSyntax::user_role_management`](crate::dialect::AccessControlSyntax)) —
    /// the three verbs share one [`UserRoleList`] node with the verb carried as data. Boxed,
    /// like the other family payloads, to keep the enum within its size budget.
    UserRoleList {
        /// The verb, existence guard, and name list; see [`UserRoleList`].
        statement: Box<UserRoleList>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `USE <catalog> [. <schema>]` catalog/schema-switch statement (gated by
    /// [`UtilitySyntax::use_statement`](crate::dialect::UtilitySyntax)); first-class for
    /// the same builtin-blind-seam reason as [`Pragma`](Self::Pragma).
    Use {
        /// The `USE` details; see [`UseStatement`].
        use_statement: Box<UseStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQLite `CREATE [TEMP] TRIGGER ... BEGIN ... END` statement (gated by
    /// [`StatementDdlGates::create_trigger`](crate::dialect::StatementDdlGates::create_trigger)).
    /// Boxed, like the other `CREATE` payloads, to keep the enum within its size
    /// budget.
    CreateTrigger {
        /// The `CREATE TRIGGER` details; see [`CreateTrigger`].
        create: Box<CreateTrigger<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `CREATE [DEFINER = …] TRIGGER … FOR EACH ROW <sp_proc_stmt>` statement — the
    /// stored-program (SQL/PSM) trigger, kept apart from the SQLite
    /// [`CreateTrigger`](Self::CreateTrigger) whose body is a `BEGIN <stmt>; … END` list of
    /// plain SQL statements. Boxed to keep the enum within its size budget.
    CreateStoredTrigger {
        /// The MySQL `CREATE TRIGGER` details; see [`CreateStoredTrigger`].
        create: Box<CreateStoredTrigger<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `CREATE [OR REPLACE] [TEMP] {MACRO | FUNCTION} <name>(<params>) AS
    /// <expr> | AS TABLE <query>` statement (gated by
    /// [`StatementDdlGates::create_macro`](crate::dialect::StatementDdlGates::create_macro)). A
    /// live-body macro, kept apart from [`CreateFunction`](Self::CreateFunction) whose
    /// body is an opaque routine string. Boxed, like the other `CREATE` payloads, to
    /// keep the enum within its size budget.
    CreateMacro {
        /// The `CREATE MACRO` details; see [`CreateMacro`].
        create: Box<CreateMacro<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `CREATE [PERSISTENT] SECRET <name> (<option> <value>, …)`
    /// secrets-management statement (gated by
    /// [`StatementDdlGates::create_secret`](crate::dialect::StatementDdlGates::create_secret)). Boxed,
    /// like the other `CREATE` payloads, to keep the enum within its size budget.
    CreateSecret {
        /// The `CREATE SECRET` details; see [`CreateSecret`].
        create: Box<CreateSecret<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `DROP [PERSISTENT | TEMPORARY] SECRET [IF EXISTS] <name> [FROM <storage>]`
    /// secrets-management statement (gated by
    /// [`StatementDdlGates::create_secret`](crate::dialect::StatementDdlGates::create_secret), the
    /// same flag that admits [`Self::CreateSecret`]). Its own statement — not a
    /// [`Self::Drop`] object kind — because `drop_secret.y` carries the persistence modifier
    /// and `FROM <storage>` selector the shared name-list DROP grammar lacks. Boxed, like the
    /// other DDL payloads, to keep the enum within its size budget.
    DropSecret {
        /// The `DROP SECRET` details; see [`DropSecretStmt`].
        drop: Box<DropSecretStmt>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `CREATE [OR REPLACE] [TEMP] TYPE <name> AS ENUM(…)/STRUCT(…)/<alias>`
    /// user-defined-type statement (gated by
    /// [`StatementDdlGates::create_type`](crate::dialect::StatementDdlGates::create_type)). Boxed,
    /// like the other `CREATE` payloads, to keep the enum within its size budget.
    CreateType {
        /// The `CREATE TYPE` details; see [`CreateType`].
        create: Box<CreateType<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A SQLite `CREATE VIRTUAL TABLE [IF NOT EXISTS] <name> USING <module> [(<args>)]`
    /// statement (gated by
    /// [`StatementDdlGates::create_virtual_table`](crate::dialect::StatementDdlGates::create_virtual_table)).
    /// Boxed, like the other `CREATE` payloads, to keep the enum within its size budget.
    /// The payload is non-generic — a virtual table's module arguments are opaque
    /// verbatim text, carrying no expressions or extension nodes.
    CreateVirtualTable {
        /// The `CREATE VIRTUAL TABLE` details; see [`CreateVirtualTable`].
        create: Box<CreateVirtualTable>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `CREATE [TEMPORARY] SEQUENCE [IF NOT EXISTS] <name> [<option> ...]` sequence
    /// generator (SQL:2003 T176; PostgreSQL/DuckDB), gated by
    /// [`StatementDdlGates::create_sequence`](crate::dialect::StatementDdlGates::create_sequence). One
    /// shared node gated per-dialect (ADR-0011), not parallel engine nodes: both engines'
    /// parsers accept the same standard option core. Boxed, like the other `CREATE`
    /// payloads, to keep the enum within its size budget.
    CreateSequence {
        /// The `CREATE SEQUENCE` details; see [`CreateSequence`].
        create: Box<CreateSequence<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL `CREATE EXTENSION [IF NOT EXISTS] <name> [WITH] [SCHEMA s]
    /// [VERSION v] [CASCADE]` statement (gated by
    /// [`StatementDdlGates::extension_ddl`](crate::dialect::StatementDdlGates::extension_ddl)).
    /// Boxed, like the other `CREATE` payloads, to keep the enum within its size budget.
    CreateExtension {
        /// The `CREATE EXTENSION` details; see [`CreateExtension`].
        create: Box<CreateExtension>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL `ALTER EXTENSION <name> {UPDATE [TO v] | ADD <member> | DROP
    /// <member>}` statement, sharing the extension-DDL gate with
    /// [`CreateExtension`](Self::CreateExtension). Boxed, like the other family payloads,
    /// to keep the enum within its size budget.
    AlterExtension {
        /// The `ALTER EXTENSION` details; see [`AlterExtension`].
        alter: Box<AlterExtension<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `CREATE [UNDO] TABLESPACE <name> …` NDB/InnoDB storage-DDL statement (gated by
    /// [`StatementDdlGates::tablespace_ddl`](crate::dialect::StatementDdlGates::tablespace_ddl));
    /// see [`CreateTablespace`]. Boxed, like the other `CREATE` payloads, to keep the enum
    /// within its size budget.
    CreateTablespace {
        /// The `CREATE TABLESPACE` details; see [`CreateTablespace`].
        create: Box<CreateTablespace>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `ALTER [UNDO] TABLESPACE <name> <action>` statement, sharing the tablespace-DDL
    /// gate with [`CreateTablespace`](Self::CreateTablespace); see [`AlterTablespace`]. Boxed,
    /// like the other family payloads, to keep the enum within its size budget.
    AlterTablespace {
        /// The `ALTER TABLESPACE` details; see [`AlterTablespace`].
        alter: Box<AlterTablespace>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `DROP [UNDO] TABLESPACE <name> [<option>...]` statement, sharing the tablespace-DDL
    /// gate with [`CreateTablespace`](Self::CreateTablespace); see [`DropTablespace`]. Boxed,
    /// like the other family payloads, to keep the enum within its size budget.
    DropTablespace {
        /// The `DROP TABLESPACE` details; see [`DropTablespace`].
        drop: Box<DropTablespace>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `CREATE LOGFILE GROUP <name> ADD UNDOFILE '<f>' [<option>...]` NDB storage-DDL
    /// statement (gated by
    /// [`StatementDdlGates::logfile_group_ddl`](crate::dialect::StatementDdlGates::logfile_group_ddl));
    /// see [`CreateLogfileGroup`]. Boxed, like the other `CREATE` payloads, to keep the enum
    /// within its size budget.
    CreateLogfileGroup {
        /// The `CREATE LOGFILE GROUP` details; see [`CreateLogfileGroup`].
        create: Box<CreateLogfileGroup>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `ALTER LOGFILE GROUP <name> ADD UNDOFILE '<f>' [<option>...]` statement, sharing the
    /// logfile-group-DDL gate with [`CreateLogfileGroup`](Self::CreateLogfileGroup); see
    /// [`AlterLogfileGroup`]. Boxed, like the other family payloads, to keep the enum within its
    /// size budget.
    AlterLogfileGroup {
        /// The `ALTER LOGFILE GROUP` details; see [`AlterLogfileGroup`].
        alter: Box<AlterLogfileGroup>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `DROP LOGFILE GROUP <name> [<option>...]` statement, sharing the logfile-group-DDL
    /// gate with [`CreateLogfileGroup`](Self::CreateLogfileGroup); see [`DropLogfileGroup`]. Boxed,
    /// like the other family payloads, to keep the enum within its size budget.
    DropLogfileGroup {
        /// The `DROP LOGFILE GROUP` details; see [`DropLogfileGroup`].
        drop: Box<DropLogfileGroup>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL `ALTER <object> [NO] DEPENDS ON EXTENSION <extension>` statement
    /// (`AlterObjectDependsStmt`), sharing the extension-DDL gate with
    /// [`CreateExtension`](Self::CreateExtension). Boxed, like the other family payloads,
    /// to keep the enum within its size budget.
    AlterObjectDepends {
        /// The `ALTER … DEPENDS ON EXTENSION` details; see [`AlterObjectDepends`].
        alter: Box<AlterObjectDepends<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL `ALTER SYSTEM { SET <name> {= | TO} <value> | RESET <name> | RESET ALL }`
    /// server-configuration statement (`AlterSystemStmt`), gated by
    /// [`StatementDdlGates::alter_system`](crate::dialect::StatementDdlGates::alter_system).
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    AlterSystem {
        /// The `ALTER SYSTEM` details; see [`AlterSystem`].
        alter: Box<AlterSystem>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's `ALTER DATABASE [IF EXISTS] <name> SET ALIAS TO <alias>` statement
    /// (`AlterDatabaseStmt`), gated by
    /// [`StatementDdlGates::alter_database`](crate::dialect::StatementDdlGates::alter_database).
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    AlterDatabase {
        /// The `ALTER DATABASE` details; see [`AlterDatabase`].
        alter: Box<AlterDatabase>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's `ALTER {DATABASE | SCHEMA} [<name>] <option> …` schema-option change
    /// (`alter_database_stmt`), gated by
    /// [`StatementDdlGates::alter_database_options`](crate::dialect::StatementDdlGates::alter_database_options);
    /// see [`AlterDatabaseOptions`]. A distinct node and gate from DuckDB's
    /// [`AlterDatabase`](Self::AlterDatabase) `SET ALIAS` relocation. Boxed, like the other
    /// family payloads, to keep the enum within its size budget.
    AlterDatabaseOptions {
        /// The `ALTER {DATABASE | SCHEMA}` option-change details; see [`AlterDatabaseOptions`].
        alter: Box<AlterDatabaseOptions>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's `CREATE SERVER <name> FOREIGN DATA WRAPPER <wrapper> OPTIONS ( … )`
    /// federated-server definition, gated by
    /// [`StatementDdlGates::server_definition`](crate::dialect::StatementDdlGates::server_definition);
    /// see [`CreateServer`]. Boxed, like the other family payloads, to keep the enum within its
    /// size budget.
    CreateServer {
        /// The `CREATE SERVER` details; see [`CreateServer`].
        create: Box<CreateServer>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's `ALTER SERVER <name> OPTIONS ( … )` federated-server change, gated by
    /// [`StatementDdlGates::server_definition`](crate::dialect::StatementDdlGates::server_definition);
    /// see [`AlterServer`]. Boxed, like the other family payloads, to keep the enum within its
    /// size budget.
    AlterServer {
        /// The `ALTER SERVER` details; see [`AlterServer`].
        alter: Box<AlterServer>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's `DROP SERVER [IF EXISTS] <name>` federated-server drop, gated by
    /// [`StatementDdlGates::server_definition`](crate::dialect::StatementDdlGates::server_definition);
    /// see [`DropServer`]. Boxed, like the other family payloads, to keep the enum within its
    /// size budget.
    DropServer {
        /// The `DROP SERVER` details; see [`DropServer`].
        drop: Box<DropServer>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's `ALTER INSTANCE <action>` server-instance administration statement
    /// (`alter_instance_stmt`), gated by
    /// [`StatementDdlGates::alter_instance`](crate::dialect::StatementDdlGates::alter_instance);
    /// see [`AlterInstance`]. Boxed, like the other family payloads, to keep the enum within its
    /// size budget.
    AlterInstance {
        /// The `ALTER INSTANCE` details; see [`AlterInstance`].
        alter: Box<AlterInstance>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's `CREATE [OR REPLACE] SPATIAL REFERENCE SYSTEM [IF NOT EXISTS] <srid> <attrs>`
    /// spatial-reference-system definition, gated by
    /// [`StatementDdlGates::spatial_reference_system`](crate::dialect::StatementDdlGates::spatial_reference_system);
    /// see [`CreateSpatialReferenceSystem`]. Boxed, like the other family payloads, to keep the
    /// enum within its size budget.
    CreateSpatialReferenceSystem {
        /// The `CREATE SPATIAL REFERENCE SYSTEM` details; see [`CreateSpatialReferenceSystem`].
        create: Box<CreateSpatialReferenceSystem>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's `DROP SPATIAL REFERENCE SYSTEM [IF EXISTS] <srid>` drop, gated by
    /// [`StatementDdlGates::spatial_reference_system`](crate::dialect::StatementDdlGates::spatial_reference_system);
    /// see [`DropSpatialReferenceSystem`]. Boxed, like the other family payloads, to keep the enum
    /// within its size budget.
    DropSpatialReferenceSystem {
        /// The `DROP SPATIAL REFERENCE SYSTEM` details; see [`DropSpatialReferenceSystem`].
        drop: Box<DropSpatialReferenceSystem>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's `CREATE RESOURCE GROUP <name> TYPE [=] {SYSTEM | USER} …` definition, gated by
    /// [`StatementDdlGates::resource_group`](crate::dialect::StatementDdlGates::resource_group);
    /// see [`CreateResourceGroup`]. Boxed, like the other family payloads, to keep the enum within
    /// its size budget.
    CreateResourceGroup {
        /// The `CREATE RESOURCE GROUP` details; see [`CreateResourceGroup`].
        create: Box<CreateResourceGroup>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's `ALTER RESOURCE GROUP <name> …` change, gated by
    /// [`StatementDdlGates::resource_group`](crate::dialect::StatementDdlGates::resource_group);
    /// see [`AlterResourceGroup`]. Boxed, like the other family payloads, to keep the enum within
    /// its size budget.
    AlterResourceGroup {
        /// The `ALTER RESOURCE GROUP` details; see [`AlterResourceGroup`].
        alter: Box<AlterResourceGroup>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// MySQL's `DROP RESOURCE GROUP <name> [FORCE]` drop, gated by
    /// [`StatementDdlGates::resource_group`](crate::dialect::StatementDdlGates::resource_group);
    /// see [`DropResourceGroup`]. Boxed, like the other family payloads, to keep the enum within
    /// its size budget. (The `SET RESOURCE GROUP` family member is a
    /// [`SessionStatement::SetResourceGroup`], dispatched off the shared `SET` head.)
    DropResourceGroup {
        /// The `DROP RESOURCE GROUP` details; see [`DropResourceGroup`].
        drop: Box<DropResourceGroup>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's `ALTER SEQUENCE [IF EXISTS] <name> <option>...` statement (`AlterSeqStmt`),
    /// gated by
    /// [`StatementDdlGates::alter_sequence`](crate::dialect::StatementDdlGates::alter_sequence).
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    AlterSequence {
        /// The `ALTER SEQUENCE` details; see [`AlterSequence`].
        alter: Box<AlterSequence<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's `ALTER {TABLE | VIEW | SEQUENCE} [IF EXISTS] <name> SET SCHEMA <schema>`
    /// statement (`AlterObjectSchemaStmt`), gated by
    /// [`StatementDdlGates::alter_object_set_schema`](crate::dialect::StatementDdlGates::alter_object_set_schema).
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    AlterObjectSchema {
        /// The `ALTER … SET SCHEMA` details; see [`AlterObjectSchema`].
        alter: Box<AlterObjectSchema>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's leading-keyword `PIVOT <source> [ON …] [USING …] [GROUP BY …]`
    /// statement (DuckDB's `PivotStatement`). Dispatched like the utility statements —
    /// not a [`Query`](Self::Query) body, since DuckDB models it as its own top-level
    /// statement (`json_serialize_sql` rejects it as "Only SELECT statements can be
    /// serialized"). Shares the [`Pivot`] core with the
    /// [`TableFactor::Pivot`](super::TableFactor) surface (tagged
    /// [`PivotSpelling::Statement`](super::PivotSpelling)). Boxed, like the other family
    /// payloads, to keep the enum within its size budget. Gated by
    /// [`TableFactorSyntax::pivot`](crate::dialect::TableExpressionSyntax).
    Pivot {
        /// The `PIVOT` details; see [`Pivot`].
        pivot: Box<Pivot<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's leading-keyword `UNPIVOT <source> ON <cols> [INTO NAME … VALUE …]`
    /// statement — the [`Unpivot`] counterpart of [`Pivot`](Self::Pivot).
    Unpivot {
        /// The `UNPIVOT` details; see [`Unpivot`].
        unpivot: Box<Unpivot<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// DuckDB's leading-keyword `{DESCRIBE | SUMMARIZE} <query> | <table>` introspection
    /// statement (gated by
    /// [`ShowSyntax::describe_summarize`](crate::dialect::UtilitySyntax)). DuckDB
    /// desugars it to `SELECT * FROM (<SHOW_REF>)`, so it shares the [`ShowRef`] core with
    /// the [`TableFactor::ShowRef`](super::TableFactor) table source — the same `kind` +
    /// `target`, at statement rather than table-factor position (mirroring how
    /// [`Pivot`](Self::Pivot) shares its core with the pivot table factor). Boxed, like the
    /// other family payloads, to keep the enum within its size budget.
    ShowRef {
        /// The `DESCRIBE`/`SHOW`/`SUMMARIZE` reference; see [`ShowRef`].
        show: Box<ShowRef<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `PREPARE <name> AS <statement>` prepared-statement definition (gated by
    /// [`UtilitySyntax::prepared_statements`](crate::dialect::UtilitySyntax)); first-class
    /// for the same builtin-blind-seam reason as [`Pragma`](Self::Pragma). Boxed, like the
    /// other family payloads, to keep the enum within its size budget.
    Prepare {
        /// The `PREPARE` details; see [`PrepareStatement`].
        prepare: Box<PrepareStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `EXECUTE <name> [(<args>)]` prepared-statement invocation, sharing the
    /// [`Prepare`](Self::Prepare)
    /// [`prepared_statements`](crate::dialect::UtilitySyntax) gate.
    Execute {
        /// The `EXECUTE` details; see [`ExecuteStatement`].
        execute: Box<ExecuteStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `PREPARE <name> FROM {'text' | @var}` prepared-statement definition (gated by
    /// [`UtilitySyntax::prepared_statements_from`](crate::dialect::UtilitySyntax)) — a
    /// distinct behaviour on the `PREPARE` keyword from DuckDB's typed [`Prepare`](Self::Prepare)
    /// form; see [`PrepareFromStatement`]. Boxed, like the other family payloads, to keep the
    /// enum within its size budget.
    PrepareFrom {
        /// The `PREPARE ... FROM` details; see [`PrepareFromStatement`].
        prepare_from: Box<PrepareFromStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `EXECUTE <name> [USING @var, ...]` prepared-statement invocation (gated by
    /// [`UtilitySyntax::prepared_statements_from`](crate::dialect::UtilitySyntax)) — a distinct
    /// argument surface on the `EXECUTE` keyword from DuckDB's parenthesized
    /// [`Execute`](Self::Execute) form; see [`ExecuteUsingStatement`]. Boxed, like the other
    /// family payloads, to keep the enum within its size budget.
    ExecuteUsing {
        /// The `EXECUTE ... USING` details; see [`ExecuteUsingStatement`].
        execute_using: Box<ExecuteUsingStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A `{DEALLOCATE | DROP} [PREPARE] <name>` prepared-statement release. DuckDB shares the
    /// [`Prepare`](Self::Prepare) [`prepared_statements`](crate::dialect::UtilitySyntax) gate;
    /// MySQL's `{DEALLOCATE | DROP} PREPARE <name>` rides
    /// [`prepared_statements_from`](crate::dialect::UtilitySyntax). Both spell the same
    /// [`DeallocateStatement`] node.
    Deallocate {
        /// The `DEALLOCATE` details; see [`DeallocateStatement`].
        deallocate: Box<DeallocateStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A DuckDB `CALL <name>(<args>)` routine invocation (gated by
    /// [`UtilitySyntax::call`](crate::dialect::UtilitySyntax)); first-class for the same
    /// builtin-blind-seam reason as [`Pragma`](Self::Pragma). Boxed, like the other family
    /// payloads, to keep the enum within its size budget.
    Call {
        /// The `CALL` details; see [`CallStatement`].
        call: Box<CallStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A PostgreSQL `DO [LANGUAGE <lang>] '<body>'` anonymous code block (gated by
    /// [`UtilitySyntax::do_statement`](crate::dialect::UtilitySyntax)). Non-generic in its
    /// payload — the block body is an opaque string, not a SQL expression — but the enum
    /// arm still carries `X` from the surrounding [`Statement`]. Boxed, like the other
    /// family payloads, to keep the enum within its size budget.
    Do {
        /// The `DO` block details; see [`DoStatement`].
        do_block: Box<DoStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `DO <expr> [, <expr> ...]` evaluate-and-discard statement (gated by
    /// [`UtilitySyntax::do_expression_list`](crate::dialect::UtilitySyntax)) — a distinct
    /// behaviour on the `DO` keyword from PostgreSQL's [`Do`](Self::Do) code block; see
    /// [`DoExpressionsStatement`]. Boxed, like the other family payloads, to keep the enum
    /// within its size budget.
    DoExpressions {
        /// The evaluated expression list; see [`DoExpressionsStatement`].
        do_expressions: Box<DoExpressionsStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `LOCK {TABLES | TABLE} <tbl> [[AS] <alias>] <lock-kind> [, ...]` explicit
    /// table-locking statement (gated by
    /// [`UtilitySyntax::lock_tables`](crate::dialect::UtilitySyntax)) — the per-table
    /// lock-kind reading of the leading `LOCK` keyword, distinct from PostgreSQL's
    /// unimplemented statement-level mode-list reading; see [`LockTablesStatement`]. Boxed,
    /// like the other family payloads, to keep the enum within its size budget.
    LockTables {
        /// The `LOCK TABLES` details; see [`LockTablesStatement`].
        lock_tables: Box<LockTablesStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `UNLOCK {TABLES | TABLE}` statement releasing the session's table locks
    /// (gated by [`UtilitySyntax::lock_tables`](crate::dialect::UtilitySyntax), the release
    /// counterpart of [`LockTables`](Self::LockTables)); see [`UnlockTablesStatement`].
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    UnlockTables {
        /// The `UNLOCK TABLES` details; see [`UnlockTablesStatement`].
        unlock_tables: Box<UnlockTablesStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `LOCK INSTANCE FOR BACKUP` / `UNLOCK INSTANCE` instance-wide backup-lock
    /// statement (gated by
    /// [`UtilitySyntax::lock_instance`](crate::dialect::UtilitySyntax)); one variant for
    /// the acquire/release pair — see [`InstanceLockStatement`]. Boxed, like the other
    /// family payloads, to keep the enum within its size budget.
    InstanceLock {
        /// The instance-lock details; see [`InstanceLockStatement`].
        instance_lock: Box<InstanceLockStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `[<label>:] BEGIN … END [<label>]` compound block — the stored-program
    /// body node. Body-context-only: reached through the `parse_body_statement`
    /// dispatcher, never at top level (a bare top-level `BEGIN` is transaction-start).
    /// Boxed, like the other family payloads, to keep the enum within its size budget.
    Compound {
        /// The compound-block details; see [`CompoundStatement`].
        compound: Box<CompoundStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `IF … THEN … [ELSEIF …] [ELSE …] END IF` compound-body statement. Boxed,
    /// like the other family payloads, to keep the enum within its size budget.
    If {
        /// The `IF` details; see [`IfStatement`].
        if_statement: Box<IfStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `CASE … END CASE` compound-body statement (simple or searched). Boxed,
    /// like the other family payloads, to keep the enum within its size budget.
    Case {
        /// The `CASE` details; see [`CaseStatement`].
        case_statement: Box<CaseStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `[<label>:] LOOP … END LOOP` compound-body statement. Boxed, like the
    /// other family payloads, to keep the enum within its size budget.
    Loop {
        /// The `LOOP` details; see [`LoopStatement`].
        loop_statement: Box<LoopStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `[<label>:] WHILE … DO … END WHILE` compound-body statement. Boxed, like
    /// the other family payloads, to keep the enum within its size budget.
    While {
        /// The `WHILE` details; see [`WhileStatement`].
        while_statement: Box<WhileStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `[<label>:] REPEAT … UNTIL … END REPEAT` compound-body statement. Boxed,
    /// like the other family payloads, to keep the enum within its size budget.
    Repeat {
        /// The `REPEAT` details; see [`RepeatStatement`].
        repeat: Box<RepeatStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `LEAVE <label>` compound-body statement. Boxed for a uniform family shape
    /// (the payload alone exceeds the 24-byte enum budget once its own `meta` is added).
    Leave {
        /// The `LEAVE` details; see [`LeaveStatement`].
        leave: Box<LeaveStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `ITERATE <label>` compound-body statement. Boxed for a uniform family
    /// shape.
    Iterate {
        /// The `ITERATE` details; see [`IterateStatement`].
        iterate: Box<IterateStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `RETURN <expr>` compound-body statement (stored functions only). Boxed,
    /// like the other family payloads, to keep the enum within its size budget.
    Return {
        /// The `RETURN` details; see [`ReturnStatement`].
        return_statement: Box<ReturnStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `OPEN <cursor>` compound-body statement. Boxed for a uniform family shape.
    OpenCursor {
        /// The `OPEN` details; see [`OpenCursorStatement`].
        open: Box<OpenCursorStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `FETCH [[NEXT] FROM] <cursor> INTO …` compound-body statement. Boxed for a
    /// uniform family shape.
    FetchCursor {
        /// The `FETCH` details; see [`FetchCursorStatement`].
        fetch: Box<FetchCursorStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `CLOSE <cursor>` compound-body statement. Boxed for a uniform family shape.
    CloseCursor {
        /// The `CLOSE` details; see [`CloseCursorStatement`].
        close: Box<CloseCursorStatement>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `SIGNAL {SQLSTATE '…' | <condition-name>} [SET …]` statement — raise a
    /// condition. A top-level statement (its own `signal_diagnostics` gate) that also appears
    /// in stored-program bodies. Boxed, like the other family payloads, to keep the enum
    /// within its size budget.
    Signal {
        /// The `SIGNAL` details; see [`SignalStatement`].
        signal: Box<SignalStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `RESIGNAL [{SQLSTATE '…' | <condition-name>}] [SET …]` statement — re-raise the
    /// current condition, optionally amended. Shares [`SignalStatement`] with
    /// [`Signal`](Self::Signal). Boxed, like the other family payloads, to keep the enum
    /// within its size budget.
    Resignal {
        /// The `RESIGNAL` details; see [`SignalStatement`].
        resignal: Box<SignalStatement<X>>,
        /// Source location and node identity.
        meta: Meta,
    },
    /// A MySQL `GET [CURRENT | STACKED] DIAGNOSTICS …` statement — read the diagnostics area.
    /// A top-level statement (its own `signal_diagnostics` gate) that also appears in
    /// stored-program bodies. Boxed, like the other family payloads, to keep the enum within
    /// its size budget.
    GetDiagnostics {
        /// The `GET DIAGNOSTICS` details; see [`GetDiagnosticsStatement`].
        get_diagnostics: Box<GetDiagnosticsStatement<X>>,
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

impl<X: Extension> Statement<X> {
    /// Borrow this statement as a query, when it is one.
    pub fn as_query(&self) -> Option<&Query<X>> {
        match self {
            Self::Query { query, .. } => Some(query),
            Self::CreateTable { .. }
            | Self::Insert { .. }
            | Self::Update { .. }
            | Self::Delete { .. }
            | Self::Merge { .. }
            | Self::Transaction { .. }
            | Self::Xa { .. }
            | Self::Session { .. }
            | Self::AccessControl { .. }
            | Self::AlterTable { .. }
            | Self::Drop { .. }
            | Self::CreateSchema { .. }
            | Self::CreateView { .. }
            | Self::AlterView { .. }
            | Self::CreateIndex { .. }
            | Self::CreateFunction { .. }
            | Self::CreateProcedure { .. }
            | Self::AlterRoutine { .. }
            | Self::CreateEvent { .. }
            | Self::AlterEvent { .. }
            | Self::DropEvent { .. }
            | Self::DropDatabase { .. }
            | Self::DropIndex { .. }
            | Self::CreateDatabase { .. }
            | Self::DropRoutine { .. }
            | Self::DropTransform { .. }
            | Self::Truncate { .. }
            | Self::CommentOn { .. }
            | Self::Copy { .. }
            | Self::CopyInto { .. }
            | Self::Export { .. }
            | Self::Import { .. }
            | Self::Explain { .. }
            | Self::Describe { .. }
            | Self::Show { .. }
            | Self::Kill { .. }
            | Self::Handler { .. }
            | Self::Install { .. }
            | Self::Uninstall { .. }
            | Self::Shutdown { .. }
            | Self::Restart { .. }
            | Self::Clone { .. }
            | Self::ImportTable { .. }
            | Self::Help { .. }
            | Self::Binlog { .. }
            | Self::Pragma { .. }
            | Self::Attach { .. }
            | Self::Detach { .. }
            | Self::Checkpoint { .. }
            | Self::Load { .. }
            | Self::LoadData { .. }
            | Self::UpdateExtensions { .. }
            | Self::Vacuum { .. }
            | Self::Reindex { .. }
            | Self::Analyze { .. }
            | Self::TableMaintenance { .. }
            | Self::CacheIndex { .. }
            | Self::LoadIndex { .. }
            | Self::Rename { .. }
            | Self::Flush { .. }
            | Self::Purge { .. }
            | Self::Replication { .. }
            | Self::CreateUser { .. }
            | Self::AlterUser { .. }
            | Self::UserRoleList { .. }
            | Self::Use { .. }
            | Self::CreateTrigger { .. }
            | Self::CreateStoredTrigger { .. }
            | Self::CreateMacro { .. }
            | Self::CreateSecret { .. }
            | Self::DropSecret { .. }
            | Self::CreateType { .. }
            | Self::CreateVirtualTable { .. }
            | Self::CreateSequence { .. }
            | Self::CreateExtension { .. }
            | Self::AlterExtension { .. }
            | Self::CreateTablespace { .. }
            | Self::AlterTablespace { .. }
            | Self::DropTablespace { .. }
            | Self::CreateLogfileGroup { .. }
            | Self::AlterLogfileGroup { .. }
            | Self::DropLogfileGroup { .. }
            | Self::AlterObjectDepends { .. }
            | Self::AlterSystem { .. }
            | Self::AlterDatabase { .. }
            | Self::AlterDatabaseOptions { .. }
            | Self::CreateServer { .. }
            | Self::AlterServer { .. }
            | Self::DropServer { .. }
            | Self::AlterInstance { .. }
            | Self::CreateSpatialReferenceSystem { .. }
            | Self::DropSpatialReferenceSystem { .. }
            | Self::CreateResourceGroup { .. }
            | Self::AlterResourceGroup { .. }
            | Self::DropResourceGroup { .. }
            | Self::AlterSequence { .. }
            | Self::AlterObjectSchema { .. }
            | Self::Pivot { .. }
            | Self::Unpivot { .. }
            | Self::ShowRef { .. }
            | Self::Prepare { .. }
            | Self::Execute { .. }
            | Self::PrepareFrom { .. }
            | Self::ExecuteUsing { .. }
            | Self::Deallocate { .. }
            | Self::Call { .. }
            | Self::Do { .. }
            | Self::DoExpressions { .. }
            | Self::LockTables { .. }
            | Self::UnlockTables { .. }
            | Self::InstanceLock { .. }
            | Self::Compound { .. }
            | Self::If { .. }
            | Self::Case { .. }
            | Self::Loop { .. }
            | Self::While { .. }
            | Self::Repeat { .. }
            | Self::Leave { .. }
            | Self::Iterate { .. }
            | Self::Return { .. }
            | Self::OpenCursor { .. }
            | Self::FetchCursor { .. }
            | Self::CloseCursor { .. }
            | Self::Signal { .. }
            | Self::Resignal { .. }
            | Self::GetDiagnostics { .. }
            | Self::Other { .. } => None,
        }
    }
}
