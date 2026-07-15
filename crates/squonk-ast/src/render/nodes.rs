// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Hand-written `Render` impls for the M1 node set.
//!
//! Parenthesization is *derived* from the one binding-power table,
//! never stored: `Canonical`/`Redacted` add only the parens a looser-binding
//! child would otherwise re-associate away, while `Parenthesized` wraps every
//! binary/unary node and every nested set-operation operand so a precedence
//! mis-bind shows up as different grouping.
//!
//! `Other(X)` arms recurse into the extension via `X: Render`. The `Render` bound is
//! kept off the AST's `Extension` trait to avoid an AST -> renderer
//! layering cycle, so the renderer adds it here at its own impl sites.

use super::{Render, RenderCtx, RenderMode, RenderSpelling};
use crate::ast::{
    AccessControlStatement, AccountName, AfterMatchSkip, AggregateArgs, AliasSpelling,
    AlterColumnAction, AlterColumnTarget, AlterDatabase, AlterDatabaseAction, AlterDatabaseOption,
    AlterDatabaseOptions, AlterEvent, AlterExtension, AlterExtensionAction, AlterInstance,
    AlterInstanceAction, AlterLogfileGroup, AlterObjectDepends, AlterObjectSchema,
    AlterResourceGroup, AlterRoutine, AlterSequence, AlterSequenceOption, AlterServer, AlterSystem,
    AlterSystemAction, AlterTable, AlterTableAction, AlterTablespace, AlterTablespaceAction,
    AlterUser, AlterUserSpec, AlterView, AnalyzeHistogram, AnalyzeStatement, ApplyKind, ArgSyntax,
    ArrayExpr, ArraySpelling, ArrayTypeSpelling, AsOfJoinKind, AssignGtidsKind, AttachStatement,
    AuthOption, AutoIncrementSpelling, BinaryOperator, BinaryTypeName, BinlogStatement,
    BitStringRadix, BitwiseXorSpelling, BlobTypeName, BooleanTypeName, CacheIndexKeyList,
    CacheIndexKeyword, CacheIndexStatement, CacheIndexTable, CacheIndexTargets, CallStatement,
    CaseExpr, CaseStatement, CastSyntax, CeilSpelling, ChangeReplicationSourceOption,
    ChangeReplicationSourceOptionValue, CharacterSetKeyword, CharacterTypeName, Charset,
    CharsetAnnotation, CharsetKeyword, CheckTableOption, CheckpointStatement, ChecksumTableOption,
    CloneDataDirectory, CloneSsl, CloneStatement, CloseCursorStatement, ColocationPartitionKind,
    ColumnConstraint, ColumnDef, ColumnOption, ColumnsSpelling, CommentOnStatement, CommentTarget,
    CompoundStatement, ComprehensionSource, ConditionInfoItem, ConditionInfoItemName,
    ConditionValue, ConditionalBranch, ConfigParameter, ConflictAction, ConflictResolution,
    ConflictTarget, ConstraintCharacteristics, ConstraintCheckTime, ConstraintsTarget,
    CopyDirection, CopyIntoSource, CopyIntoStatement, CopyIntoTarget, CopyOption, CopyOptionValue,
    CopySource, CopyStatement, CopyTarget, CreateColocationGroup, CreateDatabase, CreateEvent,
    CreateExtension, CreateExtensionOption, CreateFunction, CreateIndex, CreateLogfileGroup,
    CreateMacro, CreateProcedure, CreateResourceGroup, CreateSchema, CreateSecret, CreateSequence,
    CreateServer, CreateSpatialReferenceSystem, CreateStoredTrigger, CreateTable, CreateTableBody,
    CreateTableOption, CreateTableOptionKind, CreateTablespace, CreateTrigger, CreateType,
    CreateTypeDefinition, CreateUser, CreateView, CreateVirtualTable, Cte, CteBody, CteCycleClause,
    CteCycleMark, CteSearchClause, DataType, DatabaseKeyword, DeallocateKeyword,
    DeallocateStatement, DecimalTypeName, Declaration, DefaultRoleTarget, DefaultValue, Definer,
    Delete, DerivedSpelling, DescribeColumn, DescribeStatement, DetachPartitionMode,
    DetachStatement, DiagnosticsArea, DiagnosticsInfo, DmlSelection, DmlTarget, DoArg,
    DoExpressionsStatement, DoStatement, DoubleTypeName, DropBehavior, DropColocationGroup,
    DropDatabase, DropEvent, DropIndexOnTable, DropLogfileGroup, DropObjectKind, DropResourceGroup,
    DropSecretStmt, DropServer, DropSpatialReferenceSystem, DropStatement, DropTablespace,
    DropTransform, EmptyMatchesMode, EqualsSpelling, EventOnCompletion, EventSchedule, EventStatus,
    ExcludeConstraint, ExcludeElement, ExcludeOperator, ExecuteStatement, ExecuteUsingStatement,
    ExplainFormat, ExplainKeyword, ExplainOption, ExplainStatement, ExportStatement, Expr,
    Extension, ExtensionVersion, ExtractExpr, FetchCursorStatement, FetchSpelling, FieldSelector,
    FilterWhereSpelling, FlushOption, FlushStatement, FlushTablesLock, FlushTarget, ForClause,
    ForJsonMode, ForRoot, ForXmlElements, ForXmlMode, ForceKind, ForeignKeyMatch, ForeignKeyRef,
    FormatClause, FromFirstLast, FunctionArg, FunctionBody, FunctionCall, FunctionNullBehavior,
    FunctionOption, FunctionParam, FunctionParamDefault, FunctionParamDefaultSpelling,
    FunctionParamMode, GeneratedColumn, GeneratedColumnSpelling, GeneratedColumnStorage,
    GetDiagnosticsStatement, GrantAs, GrantObject, Grantee, GroupByAllSpelling, GroupByItem,
    GroupReplicationOption, HandlerAction, HandlerCondition, HandlerIndexDirection,
    HandlerKeyComparison, HandlerOperation, HandlerReadSelector, HandlerScanDirection,
    HandlerStatement, HelpStatement, HierarchicalClause, Ident, IdentityColumn, IdentityGeneration,
    IdentityOption, IfStatement, ImportStatement, ImportTableStatement, IndexAlgorithm,
    IndexColumn, IndexHint, IndexHintAction, IndexHintKeyword, IndexHintScope, IndexLock,
    IndexLockAlgorithmOption, IndexedBy, Insert, InsertColumnMatching, InsertModifier,
    InsertOverriding, InsertSource, InsertTarget, InsertValue, InsertValues, InsertVerb,
    InstallComponentSetElement, InstallComponentSetScope, InstallComponentSetValue,
    InstallStatement, InstanceLockStatement, IntWidth, IntegerDivideSpelling, IntegerTypeName,
    IntervalFields, IntoTarget, IoThreadKeyword, IsDistinctFromSpelling, IsJsonExpr,
    IsNotDistinctFromSpelling, IsolationLevel, IterateStatement, Join, JoinConstraint,
    JoinOperator, JsonAggregateBody, JsonAggregateExpr, JsonArrayBody, JsonArrayExpr, JsonBehavior,
    JsonBehaviorKind, JsonConstructorExpr, JsonConstructorKind, JsonEncoding, JsonFormat,
    JsonFuncExpr, JsonFuncKind, JsonItemType, JsonKeyValue, JsonKeyValueSpelling, JsonNullClause,
    JsonObjectExpr, JsonPassingArg, JsonQuotesBehavior, JsonReturning, JsonTable, JsonTableColumn,
    JsonValueExpr, JsonWrapperBehavior, KeyCacheName, KillStatement, KillTarget,
    LambdaParamSpelling, LanguageName, LateralView, LeaveStatement, LikeSpelling, Limit, LimitBy,
    LimitPercent, LimitSyntax, Literal, LiteralKind, LoadDataConcurrency, LoadDataDuplicate,
    LoadDataEnclosed, LoadDataFieldOrVar, LoadDataFields, LoadDataFormat, LoadDataIgnoreRows,
    LoadDataIgnoreUnit, LoadDataLines, LoadDataStatement, LoadFieldsSpelling, LoadIndexStatement,
    LoadIndexTable, LoadIndexTargets, LoadStatement, LoadTarget, LockStrength, LockTablesStatement,
    LockWait, LockingClause, LockingSpelling, LoopStatement, MacroBody, MacroParam, MacroSpelling,
    MatchRecognize, MatchRecognizePattern, MatchSearchModifier, Measure, Merge, MergeAction,
    MergeMatchKind, MergeWhenClause, ModuleArg, ModuloSpelling, NamedObjectKind,
    NamedOperatorSpelling, NamedWindow, NoExt, NoWriteToBinlog, NormalizationForm, NotEqSpelling,
    NullInclusion, NullTestSpelling, NullTreatment, ObjectName, ObjectRefKind, ObjectReference,
    OnCommitAction, OnConflict, OnlySyntax, OpenCursorStatement, OpenJson, OpenJsonColumn,
    OperatorArgs, OrderByAll, OrderByExpr, OrderByUsing, ParameterKind, ParameterSigil,
    PartitionBound, PartitionElem, PartitionSelection, PartitionSpec, PartitionStrategy,
    PasswordLockOption, PipeAggregateExpr, PipeOperator, PipeRenameItem, Pivot, PivotColumn,
    PivotExpr, PivotSpelling, PivotValueSource, PragmaStatement, PrepareFromStatement,
    PrepareSource, PrepareStatement, Privilege, PrivilegeKind, PrivilegeLevel,
    PrivilegeLevelObject, PrivilegeObjectType, Privileges, PurgeStatement, PurgeTarget, Quantifier,
    Query, QuoteStyle, ReadOnlyValue, ReferentialAction, RefreshMaterializedView, RegexpSpelling,
    ReindexStatement, RelationInheritance, RenameStatement, RepairTableOption, RepeatStatement,
    RepetitionQuantifier, ReplicaSpelling, ReplicaThreadOption, ReplicaUntilCondition,
    ReplicationFilterRule, ReplicationStatement, RequirePrimaryKeyCheck, ResourceGroupState,
    ResourceGroupThreadPriority, ResourceGroupType, ResourceGroupVcpu, ResourceLimit,
    ReturnStatement, Returning, RewriteDbPair, RoleSpec, RollupSpelling, RoutineKind,
    RoutineObjectKind, RoutineSignature, RowsFromItem, RowsPerMatch, SampleClause, SampleUnit,
    SchemaObjectKind, SchemaRelocationObject, SecretOption, SecretPersistence, Select,
    SelectDistinct, SelectItem, SelectSpelling, SemiAntiSide, SemiStructuredAccessExpr,
    SemiStructuredPathSegment, ServerOption, ServerOptionKind, SessionStatement,
    SessionVariableKind, SetAssignment, SetCharacterSetValue, SetExpr, SetNamesValue, SetOperator,
    SetParameterValue, SetQuantifier, SetScope, SetValue, SetVariableAssignment,
    SetVariableKeyword, SetVariableValue, Setting, ShowBare, ShowColumnsSpelling, ShowCreateKind,
    ShowDiagnosticKind, ShowEngineArtifact, ShowFilter, ShowFrom, ShowFromKeyword,
    ShowFunctionsFilter, ShowFunctionsScope, ShowIndexSpelling, ShowLimit, ShowListing,
    ShowProfileType, ShowRef, ShowRefKind, ShowRefTarget, ShowRoutineKind, ShowScope,
    ShowStatement, ShowTarget, SignalItem, SignalItemName, SignalStatement, Signedness,
    SizeLiteral, SizeUnit, SpecialFunctionKeyword, SpecialSetValue, SqlDataAccess,
    SqlSecurityContext, SrsAttribute, Statement, StatementInfoItem, StatementInfoItemName,
    StringFunc, StructConstructorArg, StructConstructorField, StructField, StructKeySpelling,
    StructTypeField, StructTypeSpelling, SubscriptKind, SubsetDefinition, SymbolDefinition,
    SystemVariableScope, SystemVariableScopeKind, TableAlias, TableConstraint, TableConstraintDef,
    TableElement, TableFactor, TableFunctionColumn, TableHint, TableKeyword, TableLikeAction,
    TableLikeFeature, TableLikeOption, TableLock, TableLockKind, TableMaintenanceKind,
    TableMaintenanceStatement, TableOption, TableOptionValue, TableRename, TableSample,
    TableStorageParameter, TableVersion, TableWithJoins, TablespaceOption, TablespaceSizeOption,
    TemporaryTableKind, TextTypeName, TimeTypeName, TimeZone, TimestampTypeName, TlsOption,
    TlsRequirement, TransactionAccessMode, TransactionBlockKeyword, TransactionCommitKeyword,
    TransactionMode, TransactionModeKind, TransactionRollbackKeyword, TransactionStart,
    TransactionStatement, TriggerEvent, TriggerOrder, TriggerTiming, TrimSide, TruthValue,
    UnaryOperator, UndoTablespaceState, UninstallStatement, UnlockTablesStatement, Unpivot,
    UnpivotColumn, UnpivotSpelling, Update, UpdateAssignment, UpdateExtensionsStatement,
    UpdateTupleSource, UpdateValue, Upsert, UseStatement, UserAttribute, UserRename, UserRoleList,
    UserRoleListKind, UserSpec, VacuumAnalyze, VacuumStatement, Values, ValuesItem, VcpuRange,
    ViewAlgorithm, ViewCheckOption, ViewOptions, WhileStatement, WildcardOptions, WildcardRename,
    WildcardReplace, WindowDefinition, WindowFrame, WindowFrameBound, WindowFrameExclusion,
    WindowFrameUnits, WindowSpec, With, WithRoleSpec, WrappedTypeKind, XaAssociation,
    XaStartKeyword, XaStatement, XaSuspend, Xid, XmlAttribute, XmlDocumentOrContent, XmlFunc,
    XmlIndentOption, XmlNamespace, XmlPassingMechanism, XmlStandalone, XmlTable, XmlTableColumn,
    XmlWhitespaceOption,
};
use crate::dialect::TargetSpelling;
use crate::precedence::{
    BindingPower, BindingPowerTable, SetOperationBindingPowerTable, Side, UNPARENTHESIZED_IN_LIST,
    needs_parens_between,
};
use std::fmt;
use thin_vec::ThinVec;

// The render-shape fingerprint pins (ADR-0013), one per AST source file
// (`ast/<family>.rs`). Each `const _` compiles only while its `<0x…>` still equals
// the matching `CURRENT_RENDER_SHAPE_<FAMILY>` hash sourcegen last wrote for that
// file's slice of the AST shape. Change a shape in `ast/<family>.rs` (add, drop, or
// retype a field or variant) and sourcegen regenerates a new
// `RenderShapeFingerprint<0x…>` for THAT family only, so only that family's pin
// below stops compiling with an `expected RenderShapeFingerprint<…>, found
// RenderShapeFingerprint<…>` mismatch (the two const-generic fingerprints differ) —
// the alarm that the hand-written impls below may no longer cover every field or
// variant of that file's nodes. Do NOT just paste the new hash in to silence it:
// that skips the audit this pin exists to force. Fix it in order (the procedure below):
//   1. rerun `cargo run -p squonk-sourcegen` so `generated/render_skeleton.rs`
//      reflects the new shape;
//   2. audit the hand-written `Render` impls for that family's nodes (the impls are
//      interleaved through this file — the const name, not a line range, is the
//      index) against the regenerated `render_skeleton`, updating the render text;
//   3. THEN copy the regenerated `CURRENT_RENDER_SHAPE_<FAMILY>` hash into the
//      matching `<0x…>` below — leave every other family's pin untouched.
// The compile error can only carry the two hashes, not this procedure, so it lives
// here.
//
// Why per-family (ADR-0013 split): a shape change moves ONE pin line, so two agents
// editing disjoint files (`expr.rs` vs `ddl.rs`) touch disjoint pins and merge with
// no conflict, while two agents landing shapes in the SAME file still collide loudly
// on that file's single shared pin. The pins and the `render_skeleton` module they
// reference are all `#[cfg(test)]`-gated, so product builds don't compile the
// thousands of dead skeleton lines; the drift alarm surfaces under `cargo nextest
// run` (test builds), not `cargo build -p squonk-ast`.
#[cfg(test)]
use crate::generated::render_skeleton as skeleton;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0x0bc66b9b4759b4e4> = skeleton::CURRENT_RENDER_SHAPE_DCL;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0xb7fb9938befad896> = skeleton::CURRENT_RENDER_SHAPE_DDL;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0x5bdcbbc8cc864aec> = skeleton::CURRENT_RENDER_SHAPE_DML;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0xde8cec2513ed42e5> = skeleton::CURRENT_RENDER_SHAPE_EXPR;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0xffd1225e92e4dab2> = skeleton::CURRENT_RENDER_SHAPE_EXT;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0xbc5427055a4e813c> =
    skeleton::CURRENT_RENDER_SHAPE_IDENT;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0x509ea17dec9af986> =
    skeleton::CURRENT_RENDER_SHAPE_LITERAL;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0x8b1ed704455adf27> =
    skeleton::CURRENT_RENDER_SHAPE_MATCH_RECOGNIZE;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0x5ebfaa9fb0c1caf9> =
    skeleton::CURRENT_RENDER_SHAPE_PIPE_OPS;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0xd3444362d3f03fe8> =
    skeleton::CURRENT_RENDER_SHAPE_PIVOT;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0xe6cc8d2df67c101c> =
    skeleton::CURRENT_RENDER_SHAPE_QUERY;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0x957db431c4a1db51> = skeleton::CURRENT_RENDER_SHAPE_STMT;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0xfb50d22f049b4cc7> =
    skeleton::CURRENT_RENDER_SHAPE_STORED_PROGRAM;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0x1a58662f47080916> = skeleton::CURRENT_RENDER_SHAPE_TCL;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0x03efd7c0644eb26a> = skeleton::CURRENT_RENDER_SHAPE_TY;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0x30c6b7a3fb94d049> = skeleton::CURRENT_RENDER_SHAPE_UTIL;
#[cfg(test)]
const _: skeleton::RenderShapeFingerprint<0x585b3bd313272236> =
    skeleton::CURRENT_RENDER_SHAPE_WINDOW;

// ---------------------------------------------------------------------------
// Leaf vocabulary
// ---------------------------------------------------------------------------

impl Render for NoExt {
    fn render(&self, _ctx: &RenderCtx<'_>, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `NoExt` is uninhabited: the stock AST has no extension node to render.
        match *self {}
    }
}

impl Render for Ident {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Redacted rendering masks every identifier to a fixed placeholder so no
        // original name reaches a query fingerprint or a PII-free log (ADR-0010),
        // mirroring how `Literal` masks values to `?`. Emitting the mask before the
        // symbol is resolved keeps the source spelling out of the output for every
        // quote style and for a keyword used as an identifier; `ObjectName` renders
        // one placeholder per dotted part, so qualified-name arity survives for
        // query-shape discrimination while the names themselves do not.
        if ctx.mode() == RenderMode::Redacted {
            return f.write_str("id");
        }
        let text = ctx.resolve(self.sym);
        // Doubling the close delimiter escapes an embedded copy, mirroring the
        // lexer's doubled-close rule so the identifier round-trips. Bracket quoting
        // is asymmetric, so only the close `]` doubles, never the open `[`.
        let (open, close, doubled) = match self.quote {
            QuoteStyle::None => return f.write_str(text),
            // A `U&"…"` Unicode-escaped identifier's decoded value (`sym`) differs from its
            // source spelling. Under a source-fidelity render the exact `U&"…" [UESCAPE 'c']`
            // slice replays verbatim; a `TargetDialect` re-spell, the redacted fingerprint,
            // or a detached node with no backing source fall through to the plain
            // double-quoted decoded form — semantically identical, and the only spelling a
            // non-`unicode_strings` target can carry.
            QuoteStyle::UnicodeDouble => {
                if honours_source_spelling(ctx) {
                    if let Some(src) = ctx.slice(self.meta.span) {
                        return f.write_str(src);
                    }
                }
                ('"', '"', "\"\"")
            }
            QuoteStyle::Single => ('\'', '\'', "''"),
            QuoteStyle::Double => ('"', '"', "\"\""),
            QuoteStyle::Backtick => ('`', '`', "``"),
            QuoteStyle::Bracket => ('[', ']', "]]"),
        };
        // The common identifier embeds no delimiter, so keep that path borrow-only
        // and allocate the escaped copy only when one is actually present.
        if text.contains(close) {
            let escaped = text.replace(close, doubled);
            write!(f, "{open}{escaped}{close}")
        } else {
            write!(f, "{open}{text}{close}")
        }
    }
}

impl Render for ObjectName {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, part) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            part.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for Literal {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if ctx.mode() == RenderMode::Redacted {
            return f.write_str("?");
        }
        if let Some(text) = ctx.slice(self.meta.span) {
            // Exact source spelling round-trips (hex/scientific/quote style).
            return f.write_str(text);
        }
        // A synthesized or detached literal has no backing source; fall back to a
        // kind-based spelling so rendering stays total instead of panicking. The
        // temporal forms re-emit a placeholder value of the right type, carrying the
        // time-zone flag / interval qualifier the kind tag records.
        match &self.kind {
            LiteralKind::Null => f.write_str("NULL"),
            LiteralKind::Boolean(true) => f.write_str("TRUE"),
            LiteralKind::Boolean(false) => f.write_str("FALSE"),
            LiteralKind::Integer | LiteralKind::Float | LiteralKind::Decimal => f.write_str("0"),
            LiteralKind::String => f.write_str("''"),
            LiteralKind::Date => f.write_str("DATE '1970-01-01'"),
            LiteralKind::Time { time_zone } => {
                f.write_str("TIME")?;
                render_time_zone_suffix(*time_zone, f)?;
                f.write_str(" '00:00:00'")
            }
            LiteralKind::Timestamp { time_zone } => {
                f.write_str("TIMESTAMP")?;
                render_time_zone_suffix(*time_zone, f)?;
                f.write_str(" '1970-01-01 00:00:00'")
            }
            LiteralKind::Interval { fields, .. } => {
                f.write_str("INTERVAL '0'")?;
                match fields {
                    Some(fields) => f.write_str(interval_fields_suffix(*fields)),
                    None => Ok(()),
                }
            }
            LiteralKind::BitString { radix } => f.write_str(match radix {
                BitStringRadix::Binary => "B'0'",
                BitStringRadix::Hex => "X'0'",
            }),
            LiteralKind::Money => f.write_str("$0"),
        }
    }
}

impl<X: Extension + Render> Render for DataType<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let target = type_render_target(ctx);
        match self {
            DataType::Boolean { spelling, .. } => f.write_str(boolean_type_name(target, *spelling)),
            DataType::TinyInt { display_width, .. } => render_sized(f, "TINYINT", *display_width),
            DataType::SmallInt { display_width, .. } => render_sized(f, "SMALLINT", *display_width),
            DataType::MediumInt { display_width, .. } => {
                render_sized(f, "MEDIUMINT", *display_width)
            }
            DataType::Integer {
                spelling,
                display_width,
                ..
            } => render_sized(f, integer_type_name(target, *spelling), *display_width),
            DataType::BigInt { display_width, .. } => render_sized(f, "BIGINT", *display_width),
            DataType::Decimal {
                spelling,
                precision,
                scale,
                ..
            } => {
                render_precision_scale(f, decimal_type_name(target, *spelling), *precision, *scale)
            }
            DataType::Float { precision, .. } => render_sized(f, "FLOAT", *precision),
            DataType::Real { .. } => f.write_str("REAL"),
            DataType::Double { spelling, .. } => f.write_str(double_type_name(*spelling)),
            DataType::Text {
                spelling, charset, ..
            } => {
                f.write_str(text_type_name(target, *spelling))?;
                render_optional_charset_annotation(ctx, target, charset.as_deref(), f)
            }
            DataType::Blob { spelling, .. } => f.write_str(blob_type_name(target, *spelling)),
            DataType::Character {
                spelling,
                size,
                charset,
                ..
            } => {
                render_sized(f, character_type_name(target, *spelling), *size)?;
                render_optional_charset_annotation(ctx, target, charset.as_deref(), f)
            }
            DataType::Binary { spelling, size, .. } => {
                render_sized(f, binary_type_name(target, *spelling), *size)
            }
            DataType::Bit { varying, size, .. } => {
                render_sized(f, if *varying { "BIT VARYING" } else { "BIT" }, *size)
            }
            DataType::Json { .. } => f.write_str("JSON"),
            DataType::Uuid { .. } => f.write_str("UUID"),
            DataType::Date { .. } => f.write_str("DATE"),
            DataType::Time {
                spelling,
                precision,
                time_zone,
                ..
            } => render_time_type(target, *spelling, *precision, *time_zone, f),
            DataType::Timestamp {
                spelling,
                precision,
                time_zone,
                ..
            } => render_timestamp_type(target, *spelling, *precision, *time_zone, f),
            DataType::Interval {
                fields, precision, ..
            } => render_interval_type(*fields, *precision, f),
            DataType::Enum {
                values, charset, ..
            } => {
                render_value_list_type(ctx, "ENUM", values, f)?;
                render_optional_charset_annotation(ctx, target, charset.as_deref(), f)
            }
            DataType::Set {
                values, charset, ..
            } => {
                render_value_list_type(ctx, "SET", values, f)?;
                render_optional_charset_annotation(ctx, target, charset.as_deref(), f)
            }
            DataType::NumericModifier {
                element,
                signedness,
                zerofill,
                ..
            } => render_numeric_modifier_type(ctx, element.as_deref(), *signedness, *zerofill, f),
            DataType::Array {
                element,
                size,
                spelling,
                ..
            } => render_array_type(ctx, element, *size, *spelling, f),
            DataType::Struct {
                fields, spelling, ..
            } => match spelling {
                StructTypeSpelling::AngleBracket => {
                    render_composite_type_angle(ctx, "STRUCT", fields, f)
                }
                _ => render_composite_type(ctx, struct_type_keyword(*spelling), fields, f),
            },
            DataType::Union { members, .. } => render_composite_type(ctx, "UNION", members, f),
            DataType::Map { key, value, .. } => {
                f.write_str("MAP(")?;
                key.render(ctx, f)?;
                f.write_str(", ")?;
                value.render(ctx, f)?;
                f.write_str(")")
            }
            DataType::Wrapped { kind, inner, .. } => {
                f.write_str(wrapped_type_keyword(*kind))?;
                f.write_str("(")?;
                inner.render(ctx, f)?;
                f.write_str(")")
            }
            // ClickHouse's case-sensitive mixed-case spelling round-trips (`FixedString`,
            // never `FIXEDSTRING`), the same canonical casing as `wrapped_type_keyword`.
            DataType::FixedString { length, .. } => write!(f, "FixedString({length})"),
            // ClickHouse's mixed-case spelling round-trips (`DateTime64`, never
            // `DATETIME64`); the optional timezone re-emits its source-spelled string literal.
            DataType::DateTime64 {
                precision,
                timezone,
                ..
            } => {
                write!(f, "DateTime64({precision}")?;
                if let Some(timezone) = timezone {
                    f.write_str(", ")?;
                    timezone.render(ctx, f)?;
                }
                f.write_str(")")
            }
            // ClickHouse's mixed-case spelling round-trips (`Nested`, never `NESTED`); the
            // named-field list reuses the composite renderer, distinct from `STRUCT`/`UNION`
            // only in its keyword.
            DataType::Nested { fields, .. } => render_composite_type(ctx, "Nested", fields, f),
            // ClickHouse's mixed-case bit-width spelling round-trips (`Int256`/`UInt256`,
            // never `INT256`), the same canonical casing as the wrapper keywords.
            DataType::FixedWidthInt { signed, width, .. } => {
                f.write_str(fixed_width_int_name(*signed, *width))
            }
            DataType::UserDefined {
                name, modifiers, ..
            } => {
                name.render(ctx, f)?;
                render_literal_modifiers(ctx, f, modifiers)
            }
            // SQLite's liberal affinity name: its words re-rendered space-separated
            // (preserving each word's source case and quote style), then the optional
            // one-or-two-argument modifier list, so `LONG INTEGER` / `VARCHAR(123,456)`
            // round-trip token-for-token.
            DataType::Liberal { words, args, .. } => {
                for (index, word) in words.iter().enumerate() {
                    if index > 0 {
                        f.write_str(" ")?;
                    }
                    word.render(ctx, f)?;
                }
                render_numeric_modifiers(f, args)
            }
            // The `Other(X)` seam delegates to the host node's own `Render`, exactly as
            // `Expr::Other` / `Statement::Other` do; uninhabited under `NoExt`.
            DataType::Other { ext, .. } => ext.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for StructTypeField<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        f.write_str(" ")?;
        self.ty.render(ctx, f)
    }
}

/// The canonical type-name spelling family a `DataType` render targets.
///
/// Derived from the render [`RenderSpelling`] mode and — for a target-dialect render —
/// the target [`FeatureSet`](crate::dialect::FeatureSet)'s
/// [`TargetSpelling`] data, so the renderer reads the
/// PostgreSQL-vs-ANSI choice from a field rather than recognizing a preset by identity
/// (no `postgres` feature gate). `PreserveSource` keeps the AST's own syntax tag; the
/// dialect families select the per-construct spelling tables below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeRenderTarget {
    PreserveSource,
    Ansi,
    Postgres,
}

fn type_render_target(ctx: &RenderCtx<'_>) -> TypeRenderTarget {
    // The target's `TargetSpelling` is the data-driven contract for which canonical
    // spellings a target-dialect render emits; `PreserveSource` ignores the target and
    // keeps the AST's own syntax tag (ADR-0011).
    match ctx.spelling() {
        RenderSpelling::PreserveSource => TypeRenderTarget::PreserveSource,
        RenderSpelling::TargetDialect => match ctx.target().target_spelling {
            TargetSpelling::Ansi => TypeRenderTarget::Ansi,
            TargetSpelling::Postgres => TypeRenderTarget::Postgres,
        },
    }
}

fn boolean_type_name(target: TypeRenderTarget, spelling: BooleanTypeName) -> &'static str {
    match target {
        TypeRenderTarget::PreserveSource => match spelling {
            BooleanTypeName::Boolean => "BOOLEAN",
            BooleanTypeName::Bool => "BOOL",
        },
        TypeRenderTarget::Ansi => "BOOLEAN",
        TypeRenderTarget::Postgres => "BOOLEAN",
    }
}

fn integer_type_name(target: TypeRenderTarget, spelling: IntegerTypeName) -> &'static str {
    match target {
        TypeRenderTarget::PreserveSource => match spelling {
            IntegerTypeName::Int => "INT",
            IntegerTypeName::Integer => "INTEGER",
        },
        TypeRenderTarget::Ansi => "INTEGER",
        TypeRenderTarget::Postgres => "INTEGER",
    }
}

fn decimal_type_name(target: TypeRenderTarget, spelling: DecimalTypeName) -> &'static str {
    match target {
        TypeRenderTarget::PreserveSource => match spelling {
            DecimalTypeName::Decimal => "DECIMAL",
            DecimalTypeName::Dec => "DEC",
            DecimalTypeName::Numeric => "NUMERIC",
        },
        TypeRenderTarget::Ansi => "DECIMAL",
        TypeRenderTarget::Postgres => "NUMERIC",
    }
}

fn double_type_name(spelling: DoubleTypeName) -> &'static str {
    match spelling {
        DoubleTypeName::DoublePrecision => "DOUBLE PRECISION",
        DoubleTypeName::Double => "DOUBLE",
    }
}

fn text_type_name(target: TypeRenderTarget, spelling: TextTypeName) -> &'static str {
    match target {
        TypeRenderTarget::PreserveSource => match spelling {
            TextTypeName::Text => "TEXT",
            TextTypeName::TinyText => "TINYTEXT",
            TextTypeName::MediumText => "MEDIUMTEXT",
            TextTypeName::LongText => "LONGTEXT",
        },
        // The MySQL size family has no standard spelling; collapse it to the portable
        // `TEXT` when targeting another dialect.
        TypeRenderTarget::Ansi | TypeRenderTarget::Postgres => "TEXT",
    }
}

fn blob_type_name(target: TypeRenderTarget, spelling: BlobTypeName) -> &'static str {
    match target {
        TypeRenderTarget::PreserveSource => match spelling {
            BlobTypeName::Blob => "BLOB",
            BlobTypeName::TinyBlob => "TINYBLOB",
            BlobTypeName::MediumBlob => "MEDIUMBLOB",
            BlobTypeName::LongBlob => "LONGBLOB",
        },
        // No standard binary-LOB spelling; keep `BLOB` for ANSI and map to the
        // PostgreSQL binary type for a PostgreSQL target.
        TypeRenderTarget::Ansi => "BLOB",
        TypeRenderTarget::Postgres => "BYTEA",
    }
}

fn character_type_name(target: TypeRenderTarget, spelling: CharacterTypeName) -> &'static str {
    // Whether this spelling is a `VARYING` (variable-length) character type, used
    // to map the national-character spellings onto a target's varying/fixed form.
    let varying = matches!(
        spelling,
        CharacterTypeName::CharVarying
            | CharacterTypeName::CharacterVarying
            | CharacterTypeName::Varchar
            | CharacterTypeName::NcharVarying
            | CharacterTypeName::NationalCharVarying
            | CharacterTypeName::NationalCharacterVarying
    );
    match target {
        TypeRenderTarget::PreserveSource => match spelling {
            CharacterTypeName::Char => "CHAR",
            CharacterTypeName::Character => "CHARACTER",
            CharacterTypeName::CharVarying => "CHAR VARYING",
            CharacterTypeName::CharacterVarying => "CHARACTER VARYING",
            CharacterTypeName::Varchar => "VARCHAR",
            CharacterTypeName::Nchar => "NCHAR",
            CharacterTypeName::NcharVarying => "NCHAR VARYING",
            CharacterTypeName::NationalChar => "NATIONAL CHAR",
            CharacterTypeName::NationalCharVarying => "NATIONAL CHAR VARYING",
            CharacterTypeName::NationalCharacter => "NATIONAL CHARACTER",
            CharacterTypeName::NationalCharacterVarying => "NATIONAL CHARACTER VARYING",
        },
        TypeRenderTarget::Ansi => {
            if varying {
                "CHARACTER VARYING"
            } else {
                "CHARACTER"
            }
        }
        TypeRenderTarget::Postgres => {
            if varying {
                "VARCHAR"
            } else {
                "CHAR"
            }
        }
    }
}

/// Render a MySQL character-set type annotation ([`CharsetAnnotation`]) in its canonical
/// order — the charset selector first, then `BINARY` — with a leading space to follow the
/// type. MySQL's reversed spellings (`BINARY CHARACTER SET x`, `BINARY ASCII`) and the
/// `CHARSET` synonym fold onto this canonical form (an ADR-0011 spelling trade; the exact
/// written order stays recoverable from the node span).
///
/// The annotation has no portable spelling, so it renders only for a source-preserving
/// round-trip; a target-dialect render drops it (like the TEXT-family size collapse).
/// `PreserveSource` is the only target that can carry one — the field is `None` for every
/// non-MySQL dialect.
fn render_optional_charset_annotation(
    ctx: &RenderCtx<'_>,
    target: TypeRenderTarget,
    annotation: Option<&CharsetAnnotation>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let Some(annotation) = annotation else {
        return Ok(());
    };
    if target != TypeRenderTarget::PreserveSource {
        return Ok(());
    }
    match annotation.charset {
        Some(Charset::Named) => {
            f.write_str(" CHARACTER SET ")?;
            // `name` is `Some` for the `Named` selector (parser invariant); a detached node
            // with a missing name renders the bare keyword rather than panicking.
            if let Some(name) = &annotation.name {
                name.render(ctx, f)?;
            }
        }
        Some(Charset::Ascii) => f.write_str(" ASCII")?,
        Some(Charset::Unicode) => f.write_str(" UNICODE")?,
        Some(Charset::Byte) => f.write_str(" BYTE")?,
        None => {}
    }
    if annotation.binary {
        f.write_str(" BINARY")?;
    }
    Ok(())
}

fn binary_type_name(target: TypeRenderTarget, spelling: BinaryTypeName) -> &'static str {
    match target {
        TypeRenderTarget::PreserveSource => match spelling {
            BinaryTypeName::Binary => "BINARY",
            BinaryTypeName::BinaryVarying => "BINARY VARYING",
            BinaryTypeName::Varbinary => "VARBINARY",
            BinaryTypeName::Bytea => "BYTEA",
        },
        TypeRenderTarget::Ansi => match spelling {
            BinaryTypeName::Binary => "BINARY",
            BinaryTypeName::BinaryVarying => "BINARY VARYING",
            BinaryTypeName::Varbinary => "BINARY VARYING",
            BinaryTypeName::Bytea => "BYTEA",
        },
        TypeRenderTarget::Postgres => match spelling {
            BinaryTypeName::Binary => "BYTEA",
            BinaryTypeName::BinaryVarying => "BYTEA",
            BinaryTypeName::Varbinary => "BYTEA",
            BinaryTypeName::Bytea => "BYTEA",
        },
    }
}

fn render_time_type(
    target: TypeRenderTarget,
    spelling: TimeTypeName,
    precision: Option<u32>,
    time_zone: TimeZone,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match target {
        TypeRenderTarget::PreserveSource => match spelling {
            TimeTypeName::Time => {
                render_sized(f, "TIME", precision)?;
                render_time_zone_suffix(time_zone, f)
            }
            TimeTypeName::Timetz => render_sized(f, "TIMETZ", precision),
        },
        TypeRenderTarget::Ansi => {
            render_sized(f, "TIME", precision)?;
            render_time_zone_suffix(time_zone_from_spelling(time_zone, spelling), f)
        }
        TypeRenderTarget::Postgres => match time_zone_from_spelling(time_zone, spelling) {
            TimeZone::Unspecified => render_sized(f, "TIME", precision),
            TimeZone::WithTimeZone => render_sized(f, "TIMETZ", precision),
            TimeZone::WithoutTimeZone => {
                render_sized(f, "TIME", precision)?;
                f.write_str(" WITHOUT TIME ZONE")
            }
        },
    }
}

fn render_timestamp_type(
    target: TypeRenderTarget,
    spelling: TimestampTypeName,
    precision: Option<u32>,
    time_zone: TimeZone,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match target {
        TypeRenderTarget::PreserveSource => match spelling {
            TimestampTypeName::Timestamp => {
                render_sized(f, "TIMESTAMP", precision)?;
                render_time_zone_suffix(time_zone, f)
            }
            TimestampTypeName::Timestamptz => render_sized(f, "TIMESTAMPTZ", precision),
            TimestampTypeName::Datetime => render_sized(f, "DATETIME", precision),
        },
        TypeRenderTarget::Ansi => {
            render_sized(f, "TIMESTAMP", precision)?;
            render_time_zone_suffix(time_zone_from_timestamp_spelling(time_zone, spelling), f)
        }
        TypeRenderTarget::Postgres => {
            match time_zone_from_timestamp_spelling(time_zone, spelling) {
                TimeZone::Unspecified => render_sized(f, "TIMESTAMP", precision),
                TimeZone::WithTimeZone => render_sized(f, "TIMESTAMPTZ", precision),
                TimeZone::WithoutTimeZone => {
                    render_sized(f, "TIMESTAMP", precision)?;
                    f.write_str(" WITHOUT TIME ZONE")
                }
            }
        }
    }
}

fn time_zone_from_spelling(time_zone: TimeZone, spelling: TimeTypeName) -> TimeZone {
    match spelling {
        TimeTypeName::Time => time_zone,
        TimeTypeName::Timetz => TimeZone::WithTimeZone,
    }
}

fn time_zone_from_timestamp_spelling(time_zone: TimeZone, spelling: TimestampTypeName) -> TimeZone {
    match spelling {
        TimestampTypeName::Timestamp => time_zone,
        TimestampTypeName::Timestamptz => TimeZone::WithTimeZone,
        // MySQL `DATETIME` carries no zone, so a non-PreserveSource target renders it
        // as a plain `TIMESTAMP` (its closest portable form).
        TimestampTypeName::Datetime => TimeZone::Unspecified,
    }
}

fn render_time_zone_suffix(time_zone: TimeZone, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match time_zone {
        TimeZone::Unspecified => Ok(()),
        TimeZone::WithTimeZone => f.write_str(" WITH TIME ZONE"),
        TimeZone::WithoutTimeZone => f.write_str(" WITHOUT TIME ZONE"),
    }
}

fn render_interval_type(
    fields: Option<IntervalFields>,
    precision: Option<u32>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str("INTERVAL")?;
    match fields {
        Some(IntervalFields::Year) => render_interval_field(f, "YEAR", precision),
        Some(IntervalFields::Month) => render_interval_field(f, "MONTH", precision),
        Some(IntervalFields::Day) => render_interval_field(f, "DAY", precision),
        Some(IntervalFields::Hour) => render_interval_field(f, "HOUR", precision),
        Some(IntervalFields::Minute) => render_interval_field(f, "MINUTE", precision),
        Some(IntervalFields::Second) => render_interval_field(f, "SECOND", precision),
        Some(IntervalFields::YearToMonth) => f.write_str(" YEAR TO MONTH"),
        Some(IntervalFields::DayToHour) => f.write_str(" DAY TO HOUR"),
        Some(IntervalFields::DayToMinute) => f.write_str(" DAY TO MINUTE"),
        Some(IntervalFields::DayToSecond) => {
            f.write_str(" DAY TO ")?;
            render_sized(f, "SECOND", precision)
        }
        Some(IntervalFields::HourToMinute) => f.write_str(" HOUR TO MINUTE"),
        Some(IntervalFields::HourToSecond) => {
            f.write_str(" HOUR TO ")?;
            render_sized(f, "SECOND", precision)
        }
        Some(IntervalFields::MinuteToSecond) => {
            f.write_str(" MINUTE TO ")?;
            render_sized(f, "SECOND", precision)
        }
        Some(IntervalFields::Week) => render_interval_field(f, "WEEK", precision),
        Some(IntervalFields::Quarter) => render_interval_field(f, "QUARTER", precision),
        Some(IntervalFields::Decade) => render_interval_field(f, "DECADE", precision),
        Some(IntervalFields::Century) => render_interval_field(f, "CENTURY", precision),
        Some(IntervalFields::Millennium) => render_interval_field(f, "MILLENNIUM", precision),
        Some(IntervalFields::Millisecond) => render_interval_field(f, "MILLISECOND", precision),
        Some(IntervalFields::Microsecond) => render_interval_field(f, "MICROSECOND", precision),
        // MySQL-only microsecond composites: no ANSI INTERVAL-type spelling exists (they are
        // produced only by the MySQL `interval` vocabulary, rendered in underscore form by
        // `render_mysql_interval_unit`), so the ANSI type render uses the descriptive `TO`
        // form for exhaustiveness — no dialect's interval-type grammar reaches these arms.
        Some(IntervalFields::DayToMicrosecond) => {
            render_interval_field(f, "DAY TO MICROSECOND", precision)
        }
        Some(IntervalFields::HourToMicrosecond) => {
            render_interval_field(f, "HOUR TO MICROSECOND", precision)
        }
        Some(IntervalFields::MinuteToMicrosecond) => {
            render_interval_field(f, "MINUTE TO MICROSECOND", precision)
        }
        Some(IntervalFields::SecondToMicrosecond) => {
            render_interval_field(f, "SECOND TO MICROSECOND", precision)
        }
        None => match precision {
            Some(precision) => write!(f, "({precision})"),
            None => Ok(()),
        },
    }
}

fn render_interval_field(
    f: &mut fmt::Formatter<'_>,
    name: &str,
    precision: Option<u32>,
) -> fmt::Result {
    f.write_str(" ")?;
    render_sized(f, name, precision)
}

/// The trailing field qualifier of an interval, as a leading-space suffix
/// (` DAY`, ` YEAR TO MONTH`, …). Used by the synthetic-literal fallback, which has
/// no source precision to place, so this omits any `(precision)`.
fn interval_fields_suffix(fields: IntervalFields) -> &'static str {
    match fields {
        IntervalFields::Year => " YEAR",
        IntervalFields::Month => " MONTH",
        IntervalFields::Day => " DAY",
        IntervalFields::Hour => " HOUR",
        IntervalFields::Minute => " MINUTE",
        IntervalFields::Second => " SECOND",
        IntervalFields::YearToMonth => " YEAR TO MONTH",
        IntervalFields::DayToHour => " DAY TO HOUR",
        IntervalFields::DayToMinute => " DAY TO MINUTE",
        IntervalFields::DayToSecond => " DAY TO SECOND",
        IntervalFields::HourToMinute => " HOUR TO MINUTE",
        IntervalFields::HourToSecond => " HOUR TO SECOND",
        IntervalFields::MinuteToSecond => " MINUTE TO SECOND",
        IntervalFields::Week => " WEEK",
        IntervalFields::Quarter => " QUARTER",
        IntervalFields::Decade => " DECADE",
        IntervalFields::Century => " CENTURY",
        IntervalFields::Millennium => " MILLENNIUM",
        IntervalFields::Millisecond => " MILLISECOND",
        IntervalFields::Microsecond => " MICROSECOND",
        IntervalFields::DayToMicrosecond => " DAY TO MICROSECOND",
        IntervalFields::HourToMicrosecond => " HOUR TO MICROSECOND",
        IntervalFields::MinuteToMicrosecond => " MINUTE TO MICROSECOND",
        IntervalFields::SecondToMicrosecond => " SECOND TO MICROSECOND",
    }
}

/// The MySQL `interval` unit vocabulary — one keyword per [`IntervalFields`], in MySQL's
/// underscore spelling (`DAY_HOUR`, `MINUTE_SECOND`, `YEAR_MONTH`), as a leading-space
/// suffix. This is the render counterpart of the parser's MySQL interval-unit reader: the
/// MySQL `EVERY <expr> <unit>` event schedule reuses the shared [`IntervalFields`]
/// vocabulary but spells its composites with an underscore, never the ANSI `TO` form. The
/// DuckDB-only extended units (`DECADE`/`CENTURY`/… and `MILLISECOND`) are unreachable here:
/// MySQL's `interval` production admits only the units below, so the parser never yields
/// them for a MySQL schedule.
fn mysql_interval_unit_suffix(unit: IntervalFields) -> &'static str {
    match unit {
        IntervalFields::Year => " YEAR",
        IntervalFields::Month => " MONTH",
        IntervalFields::Day => " DAY",
        IntervalFields::Hour => " HOUR",
        IntervalFields::Minute => " MINUTE",
        IntervalFields::Second => " SECOND",
        IntervalFields::Week => " WEEK",
        IntervalFields::Quarter => " QUARTER",
        IntervalFields::Microsecond => " MICROSECOND",
        IntervalFields::YearToMonth => " YEAR_MONTH",
        IntervalFields::DayToHour => " DAY_HOUR",
        IntervalFields::DayToMinute => " DAY_MINUTE",
        IntervalFields::DayToSecond => " DAY_SECOND",
        IntervalFields::HourToMinute => " HOUR_MINUTE",
        IntervalFields::HourToSecond => " HOUR_SECOND",
        IntervalFields::MinuteToSecond => " MINUTE_SECOND",
        IntervalFields::DayToMicrosecond => " DAY_MICROSECOND",
        IntervalFields::HourToMicrosecond => " HOUR_MICROSECOND",
        IntervalFields::MinuteToMicrosecond => " MINUTE_MICROSECOND",
        IntervalFields::SecondToMicrosecond => " SECOND_MICROSECOND",
        // No MySQL `interval` keyword; the parser never yields these for a MySQL schedule.
        IntervalFields::Decade
        | IntervalFields::Century
        | IntervalFields::Millennium
        | IntervalFields::Millisecond => " MICROSECOND",
    }
}

fn render_sized(f: &mut fmt::Formatter<'_>, name: &str, size: Option<u32>) -> fmt::Result {
    match size {
        Some(size) => write!(f, "{name}({size})"),
        None => f.write_str(name),
    }
}

fn render_precision_scale(
    f: &mut fmt::Formatter<'_>,
    name: &str,
    precision: Option<i32>,
    scale: Option<i32>,
) -> fmt::Result {
    f.write_str(name)?;
    match (precision, scale) {
        (Some(precision), Some(scale)) => write!(f, "({precision}, {scale})"),
        (Some(precision), None) => write!(f, "({precision})"),
        (None, Some(scale)) => write!(f, "(*, {scale})"),
        (None, None) => Ok(()),
    }
}

/// Render a user-defined type's constant modifier list — `(3)`, `(10, 2)`, or the DuckDB
/// string form `('OGC:CRS84')` — each modifier by its exact source spelling. Empty list
/// renders nothing.
///
/// A type modifier is part of the *type*, not a value literal (`DECIMAL(10, 2)` and
/// `DECIMAL(5, 3)` are different types), so it is rendered verbatim even in
/// [`RenderMode::Redacted`] — matching the built-in numeric-modifier path
/// ([`render_numeric_modifiers`]) and never masking to `?`. The exact span text
/// round-trips the integer/string spelling; a detached node with no backing source falls
/// back to the literal's kind-based spelling for totality.
fn render_literal_modifiers(
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
    modifiers: &[Literal],
) -> fmt::Result {
    if modifiers.is_empty() {
        return Ok(());
    }
    f.write_str("(")?;
    for (index, modifier) in modifiers.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        if let Some(text) = ctx.slice(modifier.meta.span) {
            f.write_str(text)?;
        } else {
            modifier.render(ctx, f)?;
        }
    }
    f.write_str(")")
}

fn render_numeric_modifiers(f: &mut fmt::Formatter<'_>, modifiers: &[u32]) -> fmt::Result {
    if modifiers.is_empty() {
        return Ok(());
    }
    f.write_str("(")?;
    for (index, modifier) in modifiers.iter().enumerate() {
        if index > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{modifier}")?;
    }
    f.write_str(")")
}

/// Render a MySQL `ENUM(...)` / `SET(...)` value-list type. Each member renders as
/// its source-spelled string constant.
fn render_value_list_type(
    ctx: &RenderCtx<'_>,
    name: &str,
    values: &[Literal],
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str(name)?;
    f.write_str("(")?;
    render_comma_separated(values, ctx, f)?;
    f.write_str(")")
}

/// The keyword an anonymous composite type ([`DataType::Struct`]) renders under.
/// The ClickHouse keyword a [`DataType::Wrapped`] type combinator renders under. The
/// canonical mixed-case spelling round-trips ClickHouse's case-sensitive type name
/// (`Nullable`, never `NULLABLE`), even though the case-insensitive keyword parser
/// accepts any casing on input.
fn wrapped_type_keyword(kind: WrappedTypeKind) -> &'static str {
    match kind {
        WrappedTypeKind::Nullable => "Nullable",
        WrappedTypeKind::LowCardinality => "LowCardinality",
    }
}

/// The canonical spelling of a ClickHouse [`DataType::FixedWidthInt`] type name, e.g.
/// `(true, IntWidth::W256)` → `Int256`, `(false, IntWidth::W8)` → `UInt8`. Mixed-case
/// round-trips ClickHouse's case-sensitive type name (`Int256`, never `INT256`), even
/// though the case-insensitive keyword parser accepts any casing on input.
fn fixed_width_int_name(signed: bool, width: IntWidth) -> &'static str {
    match (signed, width) {
        (true, IntWidth::W8) => "Int8",
        (true, IntWidth::W16) => "Int16",
        (true, IntWidth::W32) => "Int32",
        (true, IntWidth::W64) => "Int64",
        (true, IntWidth::W128) => "Int128",
        (true, IntWidth::W256) => "Int256",
        (false, IntWidth::W8) => "UInt8",
        (false, IntWidth::W16) => "UInt16",
        (false, IntWidth::W32) => "UInt32",
        (false, IntWidth::W64) => "UInt64",
        (false, IntWidth::W128) => "UInt128",
        (false, IntWidth::W256) => "UInt256",
    }
}

fn struct_type_keyword(spelling: StructTypeSpelling) -> &'static str {
    match spelling {
        StructTypeSpelling::Struct | StructTypeSpelling::AngleBracket => "STRUCT",
        StructTypeSpelling::Row => "ROW",
    }
}

/// BigQuery `STRUCT<field TYPE, …>` / angle-bracket composite list.
fn render_composite_type_angle<X: Extension + Render>(
    ctx: &RenderCtx<'_>,
    keyword: &str,
    fields: &[StructTypeField<X>],
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str(keyword)?;
    f.write_str("<")?;
    render_comma_separated(fields, ctx, f)?;
    f.write_str(">")
}

/// Render an anonymous composite type: `<keyword>(name TYPE, ...)`, shared by
/// [`DataType::Struct`] (`STRUCT`/`ROW`), [`DataType::Union`], and the ClickHouse
/// [`DataType::Nested`].
fn render_composite_type<X: Extension + Render>(
    ctx: &RenderCtx<'_>,
    keyword: &str,
    fields: &[StructTypeField<X>],
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str(keyword)?;
    f.write_str("(")?;
    render_comma_separated(fields, ctx, f)?;
    f.write_str(")")
}

/// Render an array-type suffix per its written surface: bracket `T[]`/`T[n]` or keyword
/// `T ARRAY`/`T ARRAY[n]`, with the fixed-size bound when present.
fn render_array_type<X: Extension + Render>(
    ctx: &RenderCtx<'_>,
    element: &DataType<X>,
    size: Option<u32>,
    spelling: ArrayTypeSpelling,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if matches!(spelling, ArrayTypeSpelling::AngleBracket) {
        f.write_str("ARRAY<")?;
        element.render(ctx, f)?;
        return f.write_str(">");
    }
    element.render(ctx, f)?;
    match spelling {
        ArrayTypeSpelling::Bracket => match size {
            Some(n) => write!(f, "[{n}]"),
            None => f.write_str("[]"),
        },
        ArrayTypeSpelling::Keyword => {
            f.write_str(" ARRAY")?;
            match size {
                Some(n) => write!(f, "[{n}]"),
                None => Ok(()),
            }
        }
        ArrayTypeSpelling::AngleBracket => unreachable!("handled above"),
    }
}

/// Render a MySQL numeric type with its `SIGNED`/`UNSIGNED`/`ZEROFILL` modifiers,
/// space-separating only the parts that are present. A `None` element is the
/// standalone integer cast target (`CAST(x AS UNSIGNED)`).
fn render_numeric_modifier_type<X: Extension + Render>(
    ctx: &RenderCtx<'_>,
    element: Option<&DataType<X>>,
    signedness: Signedness,
    zerofill: bool,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let mut wrote = false;
    if let Some(element) = element {
        element.render(ctx, f)?;
        wrote = true;
    }
    let sign = match signedness {
        Signedness::Unspecified => None,
        Signedness::Signed => Some("SIGNED"),
        Signedness::Unsigned => Some("UNSIGNED"),
    };
    if let Some(sign) = sign {
        if wrote {
            f.write_str(" ")?;
        }
        f.write_str(sign)?;
        wrote = true;
    }
    if zerofill {
        if wrote {
            f.write_str(" ")?;
        }
        f.write_str("ZEROFILL")?;
    }
    Ok(())
}

impl Render for BinaryOperator {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The surrounding spaces are added by the parent expression so the token
        // itself stays a bare operator.
        f.write_str(match self {
            BinaryOperator::Plus => "+",
            BinaryOperator::Minus => "-",
            BinaryOperator::Multiply => "*",
            BinaryOperator::Divide => "/",
            // Each spelling tag renders its own surface form so the exact source
            // round-trips (`%` vs the MySQL `MOD` keyword).
            BinaryOperator::Modulo(ModuloSpelling::Percent) => "%",
            BinaryOperator::Modulo(ModuloSpelling::Mod) => "MOD",
            // One integer-division operator; the spelling tag restores the exact source
            // form. Load-bearing, not cosmetic: DuckDB has no `DIV` keyword and MySQL no
            // `//` operator, so a normalized spelling would not re-parse (like the XOR tags).
            BinaryOperator::IntegerDivide(IntegerDivideSpelling::Div) => "DIV",
            BinaryOperator::IntegerDivide(IntegerDivideSpelling::SlashSlash) => "//",
            // PostgreSQL exponentiation. Distinct from the MySQL `^` bitwise-XOR spelling
            // above (`BitwiseXor(Caret)`): same glyph, different operator and precedence.
            BinaryOperator::Exponent => "^",
            BinaryOperator::StringConcat => "||",
            BinaryOperator::Contains => "@>",
            BinaryOperator::ContainedBy => "<@",
            BinaryOperator::Overlap => "&&",
            BinaryOperator::JsonGet => "->",
            BinaryOperator::JsonGetText => "->>",
            BinaryOperator::JsonExists => "?",
            BinaryOperator::JsonExistsAny => "?|",
            BinaryOperator::JsonExistsAll => "?&",
            BinaryOperator::JsonPathExists => "@?",
            BinaryOperator::JsonPathMatch => "@@",
            BinaryOperator::JsonExtractPath => "#>",
            BinaryOperator::JsonExtractPathText => "#>>",
            BinaryOperator::JsonDeletePath => "#-",
            BinaryOperator::BitwiseOr => "|",
            BinaryOperator::BitwiseAnd => "&",
            BinaryOperator::BitwiseShiftLeft => "<<",
            BinaryOperator::BitwiseShiftRight => ">>",
            // The two XOR spellings restore the exact source form; this is load-bearing,
            // not cosmetic (PostgreSQL rejects `^` as XOR, MySQL treats `#` as a comment).
            BinaryOperator::BitwiseXor(BitwiseXorSpelling::Hash) => "#",
            BinaryOperator::BitwiseXor(BitwiseXorSpelling::Caret) => "^",
            // One equality operator; the spelling tag restores the exact source form
            // (`=` vs the SQLite `==`), mirroring the modulo/regex tags above.
            BinaryOperator::Eq(EqualsSpelling::Single) => "=",
            BinaryOperator::Eq(EqualsSpelling::Double) => "==",
            // One inequality operator; the spelling tag restores the exact source form
            // (the SQL-standard `<>` vs the C-style `!=`). Both spellings parse under
            // every dialect, so a target re-spell and the redacted fingerprint normalize
            // to the canonical `<>` (unlike the `==`/`DIV`/XOR tags, this is not
            // load-bearing for validity), keeping the fingerprint stable.
            BinaryOperator::NotEq(NotEqSpelling::Bang) if honours_source_spelling(ctx) => "!=",
            BinaryOperator::NotEq(_) => "<>",
            BinaryOperator::Lt => "<",
            BinaryOperator::LtEq => "<=",
            BinaryOperator::Gt => ">",
            BinaryOperator::GtEq => ">=",
            // One null-safe-inequality operator; the spelling tag restores the exact
            // source form (the `IS DISTINCT FROM` keyword vs SQLite's bare `IS NOT`).
            BinaryOperator::IsDistinctFrom(IsDistinctFromSpelling::Keyword) => "IS DISTINCT FROM",
            BinaryOperator::IsDistinctFrom(IsDistinctFromSpelling::Is) => "IS NOT",
            // One null-safe-equality operator; the spelling tag restores the exact source
            // form. The keyword/`<=>` split is load-bearing, not cosmetic: MySQL rejects the
            // keyword form and the other dialects reject `<=>`, so a normalized render would
            // not re-parse. SQLite's bare `IS` folds on here too and renders back as `IS`.
            BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::Keyword) => {
                "IS NOT DISTINCT FROM"
            }
            BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::NullSafeEq) => "<=>",
            BinaryOperator::IsNotDistinctFrom(IsNotDistinctFromSpelling::Is) => "IS",
            BinaryOperator::Regexp(RegexpSpelling::Rlike) => "RLIKE",
            BinaryOperator::Regexp(RegexpSpelling::Regexp) => "REGEXP",
            BinaryOperator::Glob => "GLOB",
            BinaryOperator::StartsWith => "^@",
            BinaryOperator::Match => "MATCH",
            BinaryOperator::Overlaps => "OVERLAPS",
            BinaryOperator::And => "AND",
            BinaryOperator::Xor => "XOR",
            BinaryOperator::Or => "OR",
        })
    }
}

impl Render for UnaryOperator {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UnaryOperator::Not => "NOT",
            UnaryOperator::Minus => "-",
            UnaryOperator::Plus => "+",
            UnaryOperator::BitwiseNot => "~",
            UnaryOperator::Prior => "PRIOR",
        })
    }
}

impl Render for SetOperator {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SetOperator::Union => "UNION",
            SetOperator::Intersect => "INTERSECT",
            SetOperator::Except => "EXCEPT",
        })
    }
}

impl Render for Quantifier {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Quantifier::Any => "ANY",
            Quantifier::All => "ALL",
            Quantifier::Some => "SOME",
        })
    }
}

impl Render for SetQuantifier {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SetQuantifier::All => "ALL",
            SetQuantifier::Distinct => "DISTINCT",
        })
    }
}

// ---------------------------------------------------------------------------
// Expressions and bp-derived parenthesization
// ---------------------------------------------------------------------------

impl<X: Extension + Render> Render for Expr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Column { name, .. } => name.render(ctx, f),
            Expr::Literal { literal, .. } => literal.render(ctx, f),
            Expr::BinaryOp {
                left, op, right, ..
            } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_operand(
                    left,
                    binary_child_needs_parens(&ctx.target().binding_powers, op, left, Side::Left),
                    ctx,
                    f,
                )?;
                f.write_str(" ")?;
                op.render(ctx, f)?;
                f.write_str(" ")?;
                render_operand(
                    right,
                    binary_child_needs_parens(&ctx.target().binding_powers, op, right, Side::Right),
                    ctx,
                    f,
                )?;
                close_group(full, f)
            }
            Expr::UnaryOp { op, expr, .. } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                op.render(ctx, f)?;
                // `NOT` and `PRIOR` are alphabetic and must be separated from their
                // operand; the sign operators bind tight against theirs (`-a`, `+a`).
                if matches!(op, UnaryOperator::Not | UnaryOperator::Prior) {
                    f.write_str(" ")?;
                }
                render_operand(
                    expr,
                    prefix_operand_needs_parens(&ctx.target().binding_powers, op, expr),
                    ctx,
                    f,
                )?;
                close_group(full, f)
            }
            Expr::Function { call, .. } => call.render(ctx, f),
            Expr::Case { case, .. } => case.render(ctx, f),
            Expr::Extract { extract, .. } => extract.render(ctx, f),
            Expr::JsonFunc { json_func, .. } => json_func.render(ctx, f),
            Expr::JsonObject { json_object, .. } => json_object.render(ctx, f),
            Expr::JsonArray { json_array, .. } => json_array.render(ctx, f),
            Expr::JsonAggregate { json_aggregate, .. } => json_aggregate.render(ctx, f),
            Expr::JsonConstructor {
                json_constructor, ..
            } => json_constructor.render(ctx, f),
            Expr::IsJson { is_json, .. } => is_json.render(ctx, f),
            Expr::XmlFunc { xml_func, .. } => xml_func.render(ctx, f),
            Expr::StringFunc { string_func, .. } => string_func.render(ctx, f),
            Expr::IsDocument { expr, negated, .. } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_predicate_operand(
                    expr,
                    ctx.target().binding_powers.predicate(),
                    Side::Left,
                    ctx,
                    f,
                )?;
                f.write_str(if *negated {
                    " IS NOT DOCUMENT"
                } else {
                    " IS DOCUMENT"
                })?;
                close_group(full, f)
            }
            Expr::Cast {
                expr,
                data_type,
                syntax,
                try_cast,
                ..
            } => match syntax {
                // The `try` flag only reaches the `Call` spelling — DuckDB's `TRY_CAST`
                // has no `::`/prefix form — so it selects the `TRY_CAST(` vs `CAST(` lead.
                CastSyntax::Call => {
                    f.write_str(if *try_cast { "TRY_CAST(" } else { "CAST(" })?;
                    expr.render(ctx, f)?;
                    f.write_str(" AS ")?;
                    data_type.render(ctx, f)?;
                    f.write_str(")")
                }
                CastSyntax::DoubleColon => {
                    let full = ctx.mode() == RenderMode::Parenthesized;
                    open_group(full, f)?;
                    render_pg_operand(
                        expr,
                        ctx.target().binding_powers.typecast,
                        Side::Left,
                        ctx,
                        f,
                    )?;
                    f.write_str("::")?;
                    data_type.render(ctx, f)?;
                    close_group(full, f)
                }
                // `type 'string'`: the type name ahead of its string constant. It is a
                // primary (atom-like, like a `CAST(...)` call), so it never self-wraps
                // for the `Parenthesized` oracle mode and its string operand — always a
                // literal — renders verbatim without operand parentheses.
                CastSyntax::PrefixTyped => {
                    data_type.render(ctx, f)?;
                    f.write_str(" ")?;
                    expr.render(ctx, f)
                }
                // MySQL comma-form cast — a primary (atom-like) `CONVERT(<expr>, <type>)`
                // call, like the `CAST(...)` spelling above.
                CastSyntax::Convert => {
                    f.write_str("CONVERT(")?;
                    expr.render(ctx, f)?;
                    f.write_str(", ")?;
                    data_type.render(ctx, f)?;
                    f.write_str(")")
                }
            },
            Expr::IsNull {
                expr,
                negated,
                spelling,
                ..
            } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_predicate_operand(
                    expr,
                    ctx.target().binding_powers.predicate(),
                    Side::Left,
                    ctx,
                    f,
                )?;
                f.write_str(match (spelling, negated) {
                    (NullTestSpelling::Is, false) => " IS NULL",
                    (NullTestSpelling::Is, true) => " IS NOT NULL",
                    (NullTestSpelling::Postfix, false) => " ISNULL",
                    (NullTestSpelling::Postfix, true) => " NOTNULL",
                    // The two-word postfix is only ever produced with `negated: true`.
                    (NullTestSpelling::PostfixNotNull, _) => " NOT NULL",
                })?;
                close_group(full, f)
            }
            Expr::IsTruth {
                expr,
                value,
                negated,
                ..
            } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_predicate_operand(
                    expr,
                    ctx.target().binding_powers.predicate(),
                    Side::Left,
                    ctx,
                    f,
                )?;
                f.write_str(match (negated, value) {
                    (false, TruthValue::True) => " IS TRUE",
                    (true, TruthValue::True) => " IS NOT TRUE",
                    (false, TruthValue::False) => " IS FALSE",
                    (true, TruthValue::False) => " IS NOT FALSE",
                    (false, TruthValue::Unknown) => " IS UNKNOWN",
                    (true, TruthValue::Unknown) => " IS NOT UNKNOWN",
                })?;
                close_group(full, f)
            }
            Expr::IsNormalized {
                expr,
                form,
                negated,
                ..
            } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_predicate_operand(
                    expr,
                    ctx.target().binding_powers.predicate(),
                    Side::Left,
                    ctx,
                    f,
                )?;
                f.write_str(if *negated { " IS NOT " } else { " IS " })?;
                if let Some(form) = form {
                    f.write_str(match form {
                        NormalizationForm::Nfc => "NFC ",
                        NormalizationForm::Nfd => "NFD ",
                        NormalizationForm::Nfkc => "NFKC ",
                        NormalizationForm::Nfkd => "NFKD ",
                    })?;
                }
                f.write_str("NORMALIZED")?;
                close_group(full, f)
            }
            Expr::Between {
                expr,
                low,
                high,
                negated,
                symmetric,
                ..
            } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                let range = ctx.target().binding_powers.range_predicate();
                render_predicate_operand(expr, range, Side::Left, ctx, f)?;
                f.write_str(if *negated {
                    " NOT BETWEEN "
                } else {
                    " BETWEEN "
                })?;
                if *symmetric {
                    f.write_str("SYMMETRIC ")?;
                }
                render_predicate_operand(low, range, Side::Right, ctx, f)?;
                f.write_str(" AND ")?;
                render_predicate_operand(high, range, Side::Right, ctx, f)?;
                close_group(full, f)
            }
            Expr::Like {
                expr,
                pattern,
                escape,
                negated,
                spelling,
                ..
            } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                let range = ctx.target().binding_powers.range_predicate();
                render_predicate_operand(expr, range, Side::Left, ctx, f)?;
                f.write_str(match (negated, spelling) {
                    (false, LikeSpelling::Like) => " LIKE ",
                    (true, LikeSpelling::Like) => " NOT LIKE ",
                    (false, LikeSpelling::ILike) => " ILIKE ",
                    (true, LikeSpelling::ILike) => " NOT ILIKE ",
                    (false, LikeSpelling::SimilarTo) => " SIMILAR TO ",
                    (true, LikeSpelling::SimilarTo) => " NOT SIMILAR TO ",
                })?;
                render_predicate_operand(pattern, range, Side::Right, ctx, f)?;
                if let Some(escape) = escape {
                    f.write_str(" ESCAPE ")?;
                    render_predicate_operand(escape, range, Side::Right, ctx, f)?;
                }
                close_group(full, f)
            }
            Expr::InList {
                expr,
                list,
                negated,
                ..
            } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_predicate_operand(
                    expr,
                    ctx.target().binding_powers.range_predicate(),
                    Side::Left,
                    ctx,
                    f,
                )?;
                f.write_str(if *negated { " NOT IN (" } else { " IN (" })?;
                render_comma_separated(list, ctx, f)?;
                f.write_str(")")?;
                close_group(full, f)
            }
            Expr::InSubquery {
                expr,
                subquery,
                negated,
                ..
            } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_predicate_operand(
                    expr,
                    ctx.target().binding_powers.range_predicate(),
                    Side::Left,
                    ctx,
                    f,
                )?;
                f.write_str(if *negated { " NOT IN " } else { " IN " })?;
                render_query_in_parens(subquery, ctx, f)?;
                close_group(full, f)
            }
            Expr::InExpr {
                expr, rhs, negated, ..
            } => {
                // DuckDB's unparenthesized `IN <value>` binds at its own rank
                // (`UNPARENTHESIZED_IN_LIST`), tighter than the comparison predicates, so
                // both operands parenthesize by the binding-power oracle against that rank
                // rather than the predicate level.
                let table = &ctx.target().binding_powers;
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_operand(
                    expr,
                    operand_needs_parens(table, UNPARENTHESIZED_IN_LIST, expr, Side::Left),
                    ctx,
                    f,
                )?;
                f.write_str(if *negated { " NOT IN " } else { " IN " })?;
                render_operand(
                    rhs,
                    operand_needs_parens(table, UNPARENTHESIZED_IN_LIST, rhs, Side::Right),
                    ctx,
                    f,
                )?;
                close_group(full, f)
            }
            Expr::Exists { query, .. } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                f.write_str("EXISTS ")?;
                render_query_in_parens(query, ctx, f)?;
                close_group(full, f)
            }
            Expr::QuantifiedComparison {
                left,
                op,
                quantifier,
                subquery,
                ..
            } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_predicate_operand(
                    left,
                    ctx.target().binding_powers.comparison,
                    Side::Left,
                    ctx,
                    f,
                )?;
                f.write_str(" ")?;
                op.render(ctx, f)?;
                f.write_str(" ")?;
                quantifier.render(ctx, f)?;
                f.write_str(" ")?;
                render_query_in_parens(subquery, ctx, f)?;
                close_group(full, f)
            }
            Expr::QuantifiedList {
                left,
                op,
                quantifier,
                array,
                ..
            } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_predicate_operand(
                    left,
                    ctx.target().binding_powers.comparison,
                    Side::Left,
                    ctx,
                    f,
                )?;
                f.write_str(" ")?;
                op.render(ctx, f)?;
                f.write_str(" ")?;
                quantifier.render(ctx, f)?;
                f.write_str(" (")?;
                array.render(ctx, f)?;
                f.write_str(")")?;
                close_group(full, f)
            }
            Expr::QuantifiedLike {
                left,
                pattern,
                quantifier,
                negated,
                spelling,
                ..
            } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_predicate_operand(
                    left,
                    ctx.target().binding_powers.range_predicate(),
                    Side::Left,
                    ctx,
                    f,
                )?;
                f.write_str(match (negated, spelling) {
                    (false, LikeSpelling::Like) => " LIKE ",
                    (true, LikeSpelling::Like) => " NOT LIKE ",
                    (false, LikeSpelling::ILike) => " ILIKE ",
                    (true, LikeSpelling::ILike) => " NOT ILIKE ",
                    // `SIMILAR TO` has no quantified form; the parser never builds it, so
                    // these arms are unreachable but kept total for the match.
                    (false, LikeSpelling::SimilarTo) => " SIMILAR TO ",
                    (true, LikeSpelling::SimilarTo) => " NOT SIMILAR TO ",
                })?;
                quantifier.render(ctx, f)?;
                f.write_str(" (")?;
                pattern.render(ctx, f)?;
                f.write_str(")")?;
                close_group(full, f)
            }
            Expr::Subquery { query, .. } => render_query_in_parens(query, ctx, f),
            // A placeholder's identity is query structure, not a value, so it renders
            // verbatim in every mode.
            Expr::Parameter { kind, .. } => render_parameter_kind(*kind, ctx, f),
            // A positional column reference's identity — its 1-based index — is query
            // structure, not a value, so it renders verbatim in every mode (never masked
            // the way a `Literal` value is), like the positional parameter above.
            Expr::PositionalColumn { index, .. } => write!(f, "#{index}"),
            // A session variable's identity — its sigil, optional scope, and name — is
            // query structure, not a value, so it renders verbatim in every mode (the
            // name is never masked the way a `Literal` value is), like the named
            // placeholder above. The `kind` tag restores the sigil and the canonical
            // lowercase scope keyword so all four forms round-trip.
            Expr::SessionVariable { kind, name, .. } => {
                let prefix = match kind {
                    SessionVariableKind::User => "@",
                    SessionVariableKind::System => "@@",
                    SessionVariableKind::SystemGlobal => "@@global.",
                    SessionVariableKind::SystemSession => "@@session.",
                };
                write!(f, "{prefix}{}", ctx.resolve(*name))
            }
            Expr::Subscript { subscript, .. } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                // A `::`-cast base must be parenthesized even though the typecast binds
                // tighter than the subscript: the cast's rendered type-tail would
                // otherwise re-absorb the following `[` as an array-type suffix
                // (`a::INT[1]` is a cast to `INT[1]`, not a subscript of `a::INT`), so
                // binding power alone would wrongly drop the parens. The `CAST(...)` call
                // and prefixed `TYPE 'string'` spellings self-delimit and are unaffected.
                let base_needs_parens = matches!(
                    subscript.base,
                    Expr::Cast {
                        syntax: CastSyntax::DoubleColon,
                        ..
                    }
                ) || operand_needs_parens(
                    &ctx.target().binding_powers,
                    ctx.target().binding_powers.subscript,
                    &subscript.base,
                    Side::Left,
                );
                render_operand(&subscript.base, base_needs_parens, ctx, f)?;
                f.write_str("[")?;
                match subscript.kind {
                    SubscriptKind::Index => {
                        // A bare index carries its single value in `lower`.
                        if let Some(index) = &subscript.lower {
                            index.render(ctx, f)?;
                        }
                    }
                    SubscriptKind::Slice => {
                        if let Some(lower) = &subscript.lower {
                            lower.render(ctx, f)?;
                        }
                        f.write_str(":")?;
                        if let Some(upper) = &subscript.upper {
                            upper.render(ctx, f)?;
                        }
                    }
                    SubscriptKind::SliceWithStep => {
                        if let Some(lower) = &subscript.lower {
                            lower.render(ctx, f)?;
                        }
                        f.write_str(":")?;
                        // The middle bound is mandatory; an omitted upper is DuckDB's `-`
                        // open-upper placeholder, so a `None` renders as `-`, not empty.
                        match &subscript.upper {
                            Some(upper) => upper.render(ctx, f)?,
                            None => f.write_str("-")?,
                        }
                        f.write_str(":")?;
                        if let Some(step) = &subscript.step {
                            step.render(ctx, f)?;
                        }
                    }
                }
                f.write_str("]")?;
                close_group(full, f)
            }
            Expr::SemiStructuredAccess {
                semi_structured_access,
                ..
            } => semi_structured_access.render(ctx, f),
            Expr::Collate { collate, .. } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                render_pg_operand(
                    &collate.expr,
                    ctx.target().binding_powers.collate,
                    Side::Left,
                    ctx,
                    f,
                )?;
                f.write_str(" COLLATE ")?;
                collate.collation.render(ctx, f)?;
                close_group(full, f)
            }
            Expr::AtTimeZone { at_time_zone, .. } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                let bp = ctx.target().binding_powers.at_time_zone;
                render_pg_operand(&at_time_zone.expr, bp, Side::Left, ctx, f)?;
                f.write_str(" AT TIME ZONE ")?;
                // The zone is the right operand, parsed at the operator's right
                // binding power, so it parenthesizes by the same rule.
                render_pg_operand(&at_time_zone.zone, bp, Side::Right, ctx, f)?;
                close_group(full, f)
            }
            Expr::Interval { value, unit, .. } => {
                // The MySQL operator-position interval quantity, a primary (highest binding
                // power): the amount is terminated by the unit keyword, so it renders as a bare
                // sub-expression with no operator-precedence parens. The unit reuses the shared
                // IntervalFields vocabulary in MySQL underscore spelling (never the ANSI `TO`
                // form); its suffix carries its own leading space.
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                f.write_str("INTERVAL ")?;
                value.render(ctx, f)?;
                f.write_str(mysql_interval_unit_suffix(*unit))?;
                close_group(full, f)
            }
            Expr::Array { array, .. } => match &**array {
                ArrayExpr::Elements {
                    elements, spelling, ..
                } => {
                    f.write_str(match spelling {
                        ArraySpelling::Keyword => "ARRAY[",
                        ArraySpelling::Bracket => "[",
                    })?;
                    render_comma_separated(elements, ctx, f)?;
                    f.write_str("]")
                }
                ArrayExpr::Subquery { query, .. } => {
                    f.write_str("ARRAY")?;
                    render_query_in_parens(query, ctx, f)
                }
                ArrayExpr::Comprehension { comprehension, .. } => {
                    f.write_str("[")?;
                    comprehension.element.render(ctx, f)?;
                    f.write_str(" for ")?;
                    render_ident_list(&comprehension.vars, ctx, f)?;
                    f.write_str(" in ")?;
                    render_comprehension_source(&comprehension.source, ctx, f)?;
                    if let Some(filter) = &comprehension.filter {
                        f.write_str(" if ")?;
                        filter.render(ctx, f)?;
                    }
                    f.write_str("]")
                }
            },
            Expr::Struct { r#struct, .. } => {
                f.write_str("{")?;
                for (i, field) in r#struct.fields.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    field.render(ctx, f)?;
                }
                f.write_str("}")
            }
            Expr::StructConstructor { constructor, .. } => {
                f.write_str("STRUCT")?;
                if !constructor.fields.is_empty() {
                    f.write_str("<")?;
                    render_comma_separated(&constructor.fields, ctx, f)?;
                    f.write_str(">")?;
                }
                f.write_str("(")?;
                render_comma_separated(&constructor.args, ctx, f)?;
                f.write_str(")")
            }
            Expr::Map { map, .. } => {
                f.write_str("MAP {")?;
                for (i, entry) in map.entries.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    entry.key.render(ctx, f)?;
                    f.write_str(": ")?;
                    entry.value.render(ctx, f)?;
                }
                f.write_str("}")
            }
            Expr::Row { row, .. } => {
                f.write_str(if row.explicit { "ROW(" } else { "(" })?;
                render_comma_separated(&row.fields, ctx, f)?;
                f.write_str(")")
            }
            Expr::FieldSelection {
                field_selection, ..
            } => {
                // The base is always parenthesized so `(c).f` cannot re-parse as the
                // qualified column `c.f`; the `.*` star form keeps the same parens so a
                // whole-row `tbl.*` value renders `(tbl).*` (re-parses to the same node).
                f.write_str("(")?;
                field_selection.base.render(ctx, f)?;
                f.write_str(").")?;
                match &field_selection.selector {
                    FieldSelector::Field { field, .. } => field.render(ctx, f),
                    FieldSelector::Star { .. } => f.write_str("*"),
                }
            }
            Expr::SpecialFunction {
                keyword, precision, ..
            } => {
                f.write_str(special_function_keyword(*keyword))?;
                if let Some(precision) = precision {
                    write!(f, "({precision})")?;
                }
                Ok(())
            }
            // A PostgreSQL infix operator at the "any other operator" rank (the
            // `any_operator` level, ADR-0008), so it routes through the same
            // binding-power-driven operand parenthesization as the extension infix
            // operators. The bare spelling renders `a op b` (`a ~ b`, `a <-> b`); the
            // explicit form renders `a OPERATOR(schema.op) b`, carrying its optional schema
            // qualification. PostgreSQL's own deparse keeps the two apart (a bare `~` stays
            // bare, `OPERATOR(pg_catalog.+)` stays wrapped), so the spelling tag is
            // load-bearing for round-trip fidelity.
            Expr::NamedOperator { named_operator, .. } => render_extension_infix(
                ctx,
                f,
                ctx.target().binding_powers.any_operator,
                (&named_operator.left, &named_operator.right),
                |f| match named_operator.spelling {
                    NamedOperatorSpelling::Bare => {
                        f.write_str(" ")?;
                        f.write_str(ctx.resolve(named_operator.op))?;
                        f.write_str(" ")
                    }
                    NamedOperatorSpelling::OperatorKeyword => {
                        f.write_str(" OPERATOR(")?;
                        for part in &named_operator.schema.0 {
                            part.render(ctx, f)?;
                            f.write_str(".")?;
                        }
                        f.write_str(ctx.resolve(named_operator.op))?;
                        f.write_str(") ")
                    }
                },
            ),
            // A PostgreSQL prefix operator (`@ x`, `|/ x`, `@@ box`, `@#@ x`) at the "any
            // other operator" rank. The trailing space keeps the operator a bare token and
            // stops the operand's own lead byte (`-5`, another operator) from re-lexing into
            // a longer operator.
            Expr::PrefixOperator {
                prefix_operator, ..
            } => render_extension_prefix(
                ctx,
                f,
                ctx.target().binding_powers.any_operator.left,
                |f| {
                    f.write_str(ctx.resolve(prefix_operator.op))?;
                    f.write_str(" ")
                },
                &prefix_operator.operand,
            ),
            // A DuckDB postfix operator (`10 !`, `1 ~`, `1 <->`) at the "any other operator"
            // rank. The leading space keeps the operator a bare token so the operand's own
            // trailing byte does not re-lex into a longer operator.
            Expr::PostfixOperator {
                postfix_operator, ..
            } => render_extension_postfix(
                ctx,
                f,
                ctx.target().binding_powers.any_operator,
                &postfix_operator.operand,
                |f| {
                    f.write_str(" ")?;
                    f.write_str(ctx.resolve(postfix_operator.op))
                },
            ),
            // The DuckDB lambda `->` is the JSON-arrow token at the JSON-arrow rank,
            // so its body parenthesizes as that operator's right operand. The
            // parameter side is closed (bare idents), never parenthesized beyond its
            // recorded spelling.
            Expr::Lambda { lambda, .. } => {
                let full = ctx.mode() == RenderMode::Parenthesized;
                open_group(full, f)?;
                match lambda.spelling {
                    // The python-style spelling `lambda <params>: <body>`: the `:` and the
                    // enclosing delimiter bound the body, so it renders like a function
                    // argument — no operator-precedence parens, unlike the arrow forms.
                    LambdaParamSpelling::Keyword => {
                        f.write_str("lambda ")?;
                        render_comma_separated(&lambda.params, ctx, f)?;
                        f.write_str(": ")?;
                        lambda.body.render(ctx, f)?;
                    }
                    arrow => {
                        match arrow {
                            // `Bare` implies one parameter (a parser invariant); rendering
                            // falls back to the parenthesized list on a synthesized
                            // multi-parameter value so output stays re-parseable.
                            LambdaParamSpelling::Bare if lambda.params.len() == 1 => {
                                lambda.params[0].render(ctx, f)?;
                            }
                            LambdaParamSpelling::RowKeyword => {
                                f.write_str("ROW(")?;
                                render_comma_separated(&lambda.params, ctx, f)?;
                                f.write_str(")")?;
                            }
                            // `Parenthesized`, or a synthesized multi-parameter `Bare`.
                            _ => {
                                f.write_str("(")?;
                                render_comma_separated(&lambda.params, ctx, f)?;
                                f.write_str(")")?;
                            }
                        }
                        f.write_str(" -> ")?;
                        render_pg_operand(
                            &lambda.body,
                            ctx.target().binding_powers.binary(&BinaryOperator::JsonGet),
                            Side::Right,
                            ctx,
                            f,
                        )?;
                    }
                }
                close_group(full, f)
            }
            // The DuckDB star node in its three spellings (`spelling`): the wrapped
            // `COLUMNS(<pattern>)` / star `COLUMNS(*)` / `COLUMNS(t.*)`, the `*COLUMNS(…)`
            // unpack prefix, and the bare `*` / `t.*` written without the wrapper — each
            // carrying the wildcard modifiers on its star form (`COLUMNS(* EXCLUDE (i))`,
            // `* EXCLUDE (id)`). An atom like a call: no self-parenthesization, the parent
            // decides grouping.
            Expr::Columns {
                qualifier,
                pattern,
                options,
                spelling,
                ..
            } => {
                if matches!(spelling, ColumnsSpelling::Star) {
                    // The bare star has no `COLUMNS(...)` wrapper and never a pattern.
                    if let Some(qualifier) = qualifier {
                        qualifier.render(ctx, f)?;
                        f.write_str(".")?;
                    }
                    f.write_str("*")?;
                    if let Some(options) = options {
                        render_wildcard_options(options, ctx, f)?;
                    }
                    return Ok(());
                }
                if matches!(spelling, ColumnsSpelling::Unpack) {
                    f.write_str("*")?;
                }
                f.write_str("COLUMNS(")?;
                match pattern {
                    Some(pattern) => pattern.render(ctx, f)?,
                    None => {
                        if let Some(qualifier) = qualifier {
                            qualifier.render(ctx, f)?;
                            f.write_str(".")?;
                        }
                        f.write_str("*")?;
                        if let Some(options) = options {
                            render_wildcard_options(options, ctx, f)?;
                        }
                    }
                }
                f.write_str(")")
            }
            Expr::Other { ext, .. } => ext.render(ctx, f),
        }
    }
}

/// The canonical uppercase spelling of a SQL special value function keyword.
fn special_function_keyword(keyword: SpecialFunctionKeyword) -> &'static str {
    match keyword {
        SpecialFunctionKeyword::CurrentCatalog => "CURRENT_CATALOG",
        SpecialFunctionKeyword::CurrentDate => "CURRENT_DATE",
        SpecialFunctionKeyword::CurrentRole => "CURRENT_ROLE",
        SpecialFunctionKeyword::CurrentSchema => "CURRENT_SCHEMA",
        SpecialFunctionKeyword::CurrentTime => "CURRENT_TIME",
        SpecialFunctionKeyword::CurrentTimestamp => "CURRENT_TIMESTAMP",
        SpecialFunctionKeyword::CurrentUser => "CURRENT_USER",
        SpecialFunctionKeyword::LocalTime => "LOCALTIME",
        SpecialFunctionKeyword::LocalTimestamp => "LOCALTIMESTAMP",
        SpecialFunctionKeyword::SessionUser => "SESSION_USER",
        SpecialFunctionKeyword::SystemUser => "SYSTEM_USER",
        SpecialFunctionKeyword::User => "USER",
        SpecialFunctionKeyword::UtcDate => "UTC_DATE",
        SpecialFunctionKeyword::UtcTime => "UTC_TIME",
        SpecialFunctionKeyword::UtcTimestamp => "UTC_TIMESTAMP",
    }
}

fn open_group(group: bool, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if group {
        f.write_str("(")?;
    }
    Ok(())
}

fn close_group(group: bool, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if group {
        f.write_str(")")?;
    }
    Ok(())
}

/// Render a child expression, adding parentheses when required.
///
/// In `Parenthesized` mode every binary/unary node already wraps itself, so the
/// parent contributes nothing; in the other modes it adds exactly the parens the
/// binding-power table demands (`canonical_parens`).
fn render_operand<X: Extension + Render>(
    child: &Expr<X>,
    canonical_parens: bool,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let wrap = match ctx.mode() {
        RenderMode::Parenthesized => false,
        RenderMode::Canonical | RenderMode::Redacted => canonical_parens,
    };
    open_group(wrap, f)?;
    child.render(ctx, f)?;
    close_group(wrap, f)
}

/// Render an operand of an `IS NULL` / `BETWEEN` / `IN` predicate.
///
/// The operand is parenthesized by the same binding-power rule as any other child rather than
/// conservatively whenever it is compound: only when it binds looser than the predicate's own
/// level, or is an equal-precedence non-associative sibling. `side` is `Left` for the principal
/// operand (left of the keyword) and `Right` for a `BETWEEN` bound / `LIKE` pattern, which the
/// parser parses at the predicate's right binding power.
///
/// `parent` is the binding power the predicate node itself binds at — `bp.predicate()` for the
/// `IS`-family (`IS NULL`/`IS TRUE`/`IS DISTINCT FROM`…), `bp.range_predicate()` for the
/// range/pattern/membership family (`BETWEEN`/`LIKE`/`IN`…), `bp.comparison` for a quantified
/// comparison — so the operand is grouped against the SAME rank the parser climbed the
/// predicate at (ADR-0008). Child-shape dispatch is the shared [`operand_needs_parens`] oracle,
/// so a predicate operand parenthesizes by exactly the same rule as any other operand position.
fn render_predicate_operand<X: Extension + Render>(
    child: &Expr<X>,
    parent: BindingPower,
    side: Side,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    render_operand(
        child,
        operand_needs_parens(&ctx.target().binding_powers, parent, child, side),
        ctx,
        f,
    )
}

fn render_query_in_parens<X: Extension + Render>(
    query: &Query<X>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str("(")?;
    query.render(ctx, f)?;
    f.write_str(")")
}

/// Whether a binary parent's child needs parens on the given side.
///
/// Binary children defer to the authoritative [`BindingPowerTable::needs_parens`]
/// oracle (which also encodes equal-precedence associativity), read from the
/// target dialect's table so render-time grouping honours the same per-dialect
/// binding powers the parser used. A prefix-operator child closes its
/// left edge, so it only needs parens as a *left* operand whose binding power the
/// parent's left side outbinds (e.g. `(NOT a) * b`). A comparison-level predicate
/// (`IS NULL` / `BETWEEN` / `IN`) binds at the predicate level, so it parenthesizes
/// by the same binding-power rule against the parent: `(a IS NULL) = b` wraps
/// (equal-precedence, non-associative), while `a IS NULL AND b` stays bare.
fn binary_child_needs_parens<X: Extension + Render>(
    bp: &BindingPowerTable,
    parent: &BinaryOperator,
    child: &Expr<X>,
    side: Side,
) -> bool {
    match child {
        Expr::BinaryOp { op, .. } => bp.needs_parens(parent, op, side),
        Expr::UnaryOp { op, .. } => match side {
            Side::Right => false,
            Side::Left => bp.prefix(op) < bp.binary(parent).left,
        },
        Expr::Between { .. }
        | Expr::Like { .. }
        | Expr::QuantifiedLike { .. }
        | Expr::InList { .. }
        | Expr::InSubquery { .. } => {
            needs_parens_between(bp.binary(parent), bp.range_predicate(), side)
        }
        Expr::IsNull { .. } | Expr::IsTruth { .. } | Expr::IsNormalized { .. } => {
            needs_parens_between(bp.binary(parent), bp.predicate(), side)
        }
        // A quantified comparison (`x = ANY (…)`) binds at its comparison operator's rank, not
        // the `IS`-family predicate tier — the parser climbs it at `binding_power(op)`.
        Expr::QuantifiedComparison { .. } => {
            needs_parens_between(bp.binary(parent), bp.comparison, side)
        }
        Expr::InExpr { .. } => {
            needs_parens_between(bp.binary(parent), UNPARENTHESIZED_IN_LIST, side)
        }
        Expr::NamedOperator { .. } => {
            needs_parens_between(bp.binary(parent), bp.any_operator, side)
        }
        Expr::PrefixOperator { .. } => match side {
            Side::Right => false,
            Side::Left => bp.any_operator.left < bp.binary(parent).left,
        },
        // A postfix operator is the mirror of a prefix operator: it closes its outer-left
        // edge with its operand, so it needs parens only as a looser-binding *right* operand.
        Expr::PostfixOperator { .. } => match side {
            Side::Left => false,
            Side::Right => bp.any_operator.left < bp.binary(parent).right,
        },
        // A lambda binds at the JSON-arrow rank (it is that token), so it groups as
        // a `JsonGet` child of the parent operator.
        Expr::Lambda { .. } => {
            needs_parens_between(bp.binary(parent), bp.binary(&BinaryOperator::JsonGet), side)
        }
        Expr::Other { ext, .. } => match ext.operand_binding_power() {
            Some(child) => needs_parens_between(bp.binary(parent), child, side),
            None => false,
        },
        _ => false,
    }
}

/// Whether a prefix operator's operand needs parens.
///
/// A binary operand needs parens when it binds looser than the prefix on the side
/// the prefix reaches across (`NOT (a AND b)`, `-(a + b)`), per the target
/// dialect's binding-power table. A nested sign operand is parenthesized so the
/// spelling does not collide with `--` / `++` tokens. A comparison-level predicate
/// operand binds at the predicate level, so it mirrors the binary arm against that
/// level: tighter-binding `-` wraps it (`-(a IS NULL)`, else `-a IS NULL` re-parses
/// as `(-a) IS NULL`), while looser `NOT` leaves `NOT a IS NULL` bare.
fn prefix_operand_needs_parens<X: Extension + Render>(
    bp: &BindingPowerTable,
    op: &UnaryOperator,
    operand: &Expr<X>,
) -> bool {
    match operand {
        Expr::BinaryOp { op: child, .. } => bp.binary(child).left < bp.prefix(op),
        Expr::UnaryOp { op: inner, .. } => is_sign(op) && is_sign(inner),
        Expr::Between { .. }
        | Expr::Like { .. }
        | Expr::QuantifiedLike { .. }
        | Expr::InList { .. }
        | Expr::InSubquery { .. } => bp.range_predicate().left < bp.prefix(op),
        Expr::IsNull { .. } | Expr::IsTruth { .. } | Expr::IsNormalized { .. } => {
            bp.predicate().left < bp.prefix(op)
        }
        // A quantified comparison binds at its comparison operator's rank (see
        // `binary_child_needs_parens`).
        Expr::QuantifiedComparison { .. } => bp.comparison.left < bp.prefix(op),
        Expr::InExpr { .. } => UNPARENTHESIZED_IN_LIST.left < bp.prefix(op),
        Expr::NamedOperator { .. } => bp.any_operator.left < bp.prefix(op),
        // A prefix symbolic operator binds at the "any other operator" rank; a tighter
        // sign prefix wraps it (so `-@a` renders `-(@a)`, never the `-@` operator).
        Expr::PrefixOperator { .. } => bp.any_operator.left < bp.prefix(op),
        // A postfix symbolic operator binds at the "any other operator" rank; a tighter sign
        // prefix wraps it (so `-(1!)` never renders `-1!`, which re-parses `(-1)!`).
        Expr::PostfixOperator { .. } => bp.any_operator.left < bp.prefix(op),
        // A lambda binds at the JSON-arrow rank, so a tighter prefix wraps it.
        Expr::Lambda { .. } => bp.binary(&BinaryOperator::JsonGet).left < bp.prefix(op),
        Expr::Other { ext, .. } => ext
            .operand_binding_power()
            .is_some_and(|child| child.left < bp.prefix(op)),
        _ => false,
    }
}

fn is_sign(op: &UnaryOperator) -> bool {
    matches!(op, UnaryOperator::Minus | UnaryOperator::Plus)
}

/// Render an operand of a PostgreSQL postfix operator (`::`, `[]`, `COLLATE`, the
/// left of `AT TIME ZONE`) or the `AT TIME ZONE` zone, adding the binding-power
/// parentheses `parent`/`side` demand.
fn render_pg_operand<X: Extension + Render>(
    child: &Expr<X>,
    parent: BindingPower,
    side: Side,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let canonical = operand_needs_parens(&ctx.target().binding_powers, parent, child, side);
    render_operand(child, canonical, ctx, f)
}

/// Whether `child` needs parentheses as an operand of an operator whose binding
/// power is `parent`, on the given `side`.
///
/// Centralizes child-precedence derivation for every operand position keyed by a
/// parent [`BindingPower`] — the PostgreSQL postfix operators and the extension
/// infix operators ([`render_extension_infix`]). A child binding looser than the
/// side the parent reaches across re-associates away without parentheses and must
/// be wrapped; a prefix-operator child closes its inner edge, so it only
/// needs parens as a looser-binding *left* operand. An extension operator child
/// reports its own precedence via [`Render::operand_binding_power`], so a custom
/// operator nested in any operand position groups by the same rule. Atoms, calls,
/// `CAST(...)` calls, the prefixed typed string constant (`type 'string'`), and the
/// self-delimiting `ARRAY`/`ROW`/`[…]`/`{…}`/`MAP {…}` constructors never need them.
fn operand_needs_parens<X: Extension + Render>(
    bp: &BindingPowerTable,
    parent: BindingPower,
    child: &Expr<X>,
    side: Side,
) -> bool {
    let child_bp = match child {
        Expr::BinaryOp { op, .. } => bp.binary(op),
        Expr::UnaryOp { op, .. } => {
            return side == Side::Left && bp.prefix(op) < parent.left;
        }
        Expr::Between { .. }
        | Expr::Like { .. }
        | Expr::QuantifiedLike { .. }
        | Expr::InList { .. }
        | Expr::InSubquery { .. } => bp.range_predicate(),
        Expr::IsNull { .. } | Expr::IsTruth { .. } | Expr::IsNormalized { .. } => bp.predicate(),
        // A quantified comparison binds at its comparison operator's rank (see
        // `binary_child_needs_parens`).
        Expr::QuantifiedComparison { .. } => bp.comparison,
        Expr::InExpr { .. } => UNPARENTHESIZED_IN_LIST,
        Expr::Cast {
            syntax: CastSyntax::DoubleColon,
            ..
        } => bp.typecast,
        Expr::Subscript { .. } => bp.subscript,
        Expr::Collate { .. } => bp.collate,
        Expr::AtTimeZone { .. } => bp.at_time_zone,
        Expr::FieldSelection { .. } => bp.field_selection,
        // A named operator (bare `a ~ b` or `OPERATOR(...)`) binds at the "any other
        // operator" rank.
        Expr::NamedOperator { .. } => bp.any_operator,
        // A prefix symbolic operator closes its inner edge, so — like a unary op — it only
        // needs parens as a looser-binding left operand.
        Expr::PrefixOperator { .. } => {
            return side == Side::Left && bp.any_operator.left < parent.left;
        }
        // A postfix symbolic operator closes its outer-left edge with its operand (the mirror
        // of a prefix operator), so it only needs parens as a looser-binding right operand.
        Expr::PostfixOperator { .. } => {
            return side == Side::Right && bp.any_operator.left < parent.right;
        }
        // A lambda binds at the JSON-arrow rank (it is that token).
        Expr::Lambda { .. } => bp.binary(&BinaryOperator::JsonGet),
        Expr::Other { ext, .. } => match ext.operand_binding_power() {
            Some(child_bp) => child_bp,
            None => return false,
        },
        _ => return false,
    };
    needs_parens_between(parent, child_bp, side)
}

// ---------------------------------------------------------------------------
// Extension operator rendering (the public ADR-0009 seam)
// ---------------------------------------------------------------------------

/// Render a binary extension operator — `operands.0`, the operator token written by
/// `op`, then `operands.1` — with exactly the parentheses its binding power `bp`
/// requires, plus the full self-wrapping the `Parenthesized` oracle mode adds.
///
/// This is the blessed way an `Expr::Other` infix-operator node renders: it reuses
/// the same machinery the built-in [`Expr::BinaryOp`]
/// arm uses, so a custom-operator tree round-trips by the same binding-power rule.
/// `bp` MUST equal the binding power the dialect's
/// `peek_infix_operator_hook` reported and that the node returns from
/// [`Render::operand_binding_power`]. `op` writes the operator token with its own
/// surrounding spaces (e.g. `|f| f.write_str(" ~ ")`). `operands` is the `(left,
/// right)` pair, bundled so the signature stays within the argument budget.
pub fn render_extension_infix<X: Extension + Render>(
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
    bp: BindingPower,
    operands: (&Expr<X>, &Expr<X>),
    op: impl FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
) -> fmt::Result {
    let (left, right) = operands;
    let table = &ctx.target().binding_powers;
    let full = ctx.mode() == RenderMode::Parenthesized;
    open_group(full, f)?;
    render_operand(
        left,
        operand_needs_parens(table, bp, left, Side::Left),
        ctx,
        f,
    )?;
    op(f)?;
    render_operand(
        right,
        operand_needs_parens(table, bp, right, Side::Right),
        ctx,
        f,
    )?;
    close_group(full, f)
}

/// Render a prefix extension operator — the operator token written by `op`, then
/// `operand` — with exactly the parentheses its prefix binding power `bp` requires,
/// plus the `Parenthesized` self-wrapping.
///
/// The prefix analogue of [`render_extension_infix`]; `bp` MUST equal the binding
/// power the dialect's `peek_prefix_operator_hook` reported.
pub fn render_extension_prefix<X: Extension + Render>(
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
    bp: u8,
    op: impl FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
    operand: &Expr<X>,
) -> fmt::Result {
    let full = ctx.mode() == RenderMode::Parenthesized;
    open_group(full, f)?;
    op(f)?;
    render_operand(
        operand,
        prefix_extension_operand_needs_parens(&ctx.target().binding_powers, bp, operand),
        ctx,
        f,
    )?;
    close_group(full, f)
}

/// Render a postfix extension operator — `operand`, then the operator token written by
/// `op` — with exactly the parentheses its binding power `bp` requires, plus the
/// `Parenthesized` self-wrapping.
///
/// The postfix analogue of [`render_extension_prefix`]; `bp` MUST equal the operator's
/// "any other operator" binding power. The operand is a *left* operand at that rank — a
/// postfix operator is a complete unary token, so its operand groups exactly as the left
/// operand of a same-rank infix operator (`operand_needs_parens(..., Side::Left)`): a
/// tighter operand needs no parens (`a + b !` re-parses `(a + b)!`), a looser one is
/// wrapped (`(a = b)!`).
pub fn render_extension_postfix<X: Extension + Render>(
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
    bp: BindingPower,
    operand: &Expr<X>,
    op: impl FnOnce(&mut fmt::Formatter<'_>) -> fmt::Result,
) -> fmt::Result {
    let table = &ctx.target().binding_powers;
    let full = ctx.mode() == RenderMode::Parenthesized;
    open_group(full, f)?;
    render_operand(
        operand,
        operand_needs_parens(table, bp, operand, Side::Left),
        ctx,
        f,
    )?;
    op(f)?;
    close_group(full, f)
}

/// Whether a prefix extension operator's operand needs parens, given the operator's
/// prefix binding power `bp`. A binary or comparison-level-predicate operand binding
/// looser than the prefix is wrapped; an extension operand reports its own
/// precedence; atoms and a nested unary operand (which closes its own edge) never
/// need them.
fn prefix_extension_operand_needs_parens<X: Extension + Render>(
    table: &BindingPowerTable,
    bp: u8,
    operand: &Expr<X>,
) -> bool {
    match operand {
        Expr::BinaryOp { op, .. } => table.binary(op).left < bp,
        Expr::IsNull { .. } | Expr::IsTruth { .. } | Expr::IsNormalized { .. } => {
            table.predicate().left < bp
        }
        // The range/pattern/membership family binds at the range-predicate rank; a quantified
        // comparison binds at its comparison operator's rank (parse-aligned, ADR-0008).
        Expr::Between { .. }
        | Expr::Like { .. }
        | Expr::QuantifiedLike { .. }
        | Expr::InList { .. }
        | Expr::InSubquery { .. } => table.range_predicate().left < bp,
        Expr::QuantifiedComparison { .. } => table.comparison.left < bp,
        Expr::InExpr { .. } => UNPARENTHESIZED_IN_LIST.left < bp,
        Expr::NamedOperator { .. } => table.any_operator.left < bp,
        Expr::PrefixOperator { .. } => table.any_operator.left < bp,
        Expr::PostfixOperator { .. } => table.any_operator.left < bp,
        // A lambda binds at the JSON-arrow rank, so a tighter prefix wraps it.
        Expr::Lambda { .. } => table.binary(&BinaryOperator::JsonGet).left < bp,
        Expr::Other { ext, .. } => ext
            .operand_binding_power()
            .is_some_and(|child| child.left < bp),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Statements and queries
// ---------------------------------------------------------------------------

impl<X: Extension + Render> Render for Statement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Statement::Query { query, .. } => query.render(ctx, f),
            Statement::CreateTable { create, .. } => create.render(ctx, f),
            Statement::AlterTable { alter, .. } => alter.render(ctx, f),
            Statement::Drop { drop, .. } => drop.render(ctx, f),
            Statement::CreateSchema { schema, .. } => schema.render(ctx, f),
            Statement::CreateView { view, .. } => view.render(ctx, f),
            Statement::RefreshMaterializedView { refresh, .. } => refresh.render(ctx, f),
            Statement::CreateColocationGroup { create, .. } => create.render(ctx, f),
            Statement::DropColocationGroup { drop, .. } => drop.render(ctx, f),
            Statement::AlterView { alter, .. } => alter.render(ctx, f),
            Statement::CreateIndex { index, .. } => index.render(ctx, f),
            Statement::CreateFunction { create, .. } => create.render(ctx, f),
            Statement::CreateProcedure { create, .. } => create.render(ctx, f),
            Statement::AlterRoutine { alter, .. } => alter.render(ctx, f),
            Statement::CreateEvent { create, .. } => create.render(ctx, f),
            Statement::AlterEvent { alter, .. } => alter.render(ctx, f),
            Statement::DropEvent { drop, .. } => drop.render(ctx, f),
            Statement::DropDatabase { drop, .. } => drop.render(ctx, f),
            Statement::DropIndex { drop, .. } => drop.render(ctx, f),
            Statement::CreateDatabase { create, .. } => create.render(ctx, f),
            Statement::DropRoutine {
                kind,
                if_exists,
                routines,
                behavior,
                ..
            } => {
                f.write_str("DROP ")?;
                kind.render(ctx, f)?;
                if *if_exists {
                    f.write_str(" IF EXISTS")?;
                }
                f.write_str(" ")?;
                render_comma_separated(routines, ctx, f)?;
                render_drop_behavior(*behavior, ctx, f)
            }
            Statement::DropTransform { drop, .. } => drop.render(ctx, f),
            Statement::Truncate {
                tables,
                table_keyword,
                restart_identity,
                behavior,
                ..
            } => {
                // The optional `TABLE` keyword is exact-synonym sugar: the canonical form
                // always emits it, a source-fidelity render drops it when the source did.
                if *table_keyword || !honours_source_spelling(ctx) {
                    f.write_str("TRUNCATE TABLE ")?;
                } else {
                    f.write_str("TRUNCATE ")?;
                }
                render_comma_separated(tables, ctx, f)?;
                match restart_identity {
                    Some(true) => f.write_str(" RESTART IDENTITY")?,
                    Some(false) => f.write_str(" CONTINUE IDENTITY")?,
                    None => {}
                }
                render_drop_behavior(*behavior, ctx, f)
            }
            Statement::CommentOn { comment, .. } => {
                let CommentOnStatement {
                    if_exists,
                    target,
                    name,
                    constraint_table,
                    comment,
                    ..
                } = comment.as_ref();
                f.write_str("COMMENT ")?;
                if *if_exists {
                    f.write_str("IF EXISTS ")?;
                }
                f.write_str("ON ")?;
                f.write_str(match target {
                    CommentTarget::Table => "TABLE ",
                    CommentTarget::Column => "COLUMN ",
                    CommentTarget::Database => "DATABASE ",
                    CommentTarget::View => "VIEW ",
                    CommentTarget::MaterializedView => "MATERIALIZED VIEW ",
                    CommentTarget::Index => "INDEX ",
                    CommentTarget::Constraint => "CONSTRAINT ",
                    CommentTarget::Procedure { .. } => "PROCEDURE ",
                })?;
                name.render(ctx, f)?;
                if let Some(table) = constraint_table {
                    f.write_str(" ON ")?;
                    table.render(ctx, f)?;
                }
                if let CommentTarget::Procedure {
                    arg_types: Some(arg_types),
                } = target
                {
                    f.write_str("(")?;
                    render_comma_separated(arg_types, ctx, f)?;
                    f.write_str(")")?;
                }
                f.write_str(" IS ")?;
                match comment {
                    Some(literal) => literal.render(ctx, f)?,
                    None => f.write_str("NULL")?,
                }
                Ok(())
            }
            Statement::Insert { insert, .. } => insert.render(ctx, f),
            Statement::Update { update, .. } => update.render(ctx, f),
            Statement::Delete { delete, .. } => delete.render(ctx, f),
            Statement::Merge { merge, .. } => merge.render(ctx, f),
            Statement::Transaction { transaction, .. } => transaction.render(ctx, f),
            Statement::Xa { xa, .. } => xa.render(ctx, f),
            Statement::Session { session, .. } => session.render(ctx, f),
            Statement::AccessControl { access, .. } => access.render(ctx, f),
            Statement::Copy { copy, .. } => copy.render(ctx, f),
            Statement::CopyInto { copy, .. } => copy.render(ctx, f),
            Statement::Export { export, .. } => export.render(ctx, f),
            Statement::Import { import, .. } => import.render(ctx, f),
            Statement::Explain { explain, .. } => explain.render(ctx, f),
            Statement::Describe { describe, .. } => describe.render(ctx, f),
            Statement::Show { show, .. } => show.render(ctx, f),
            Statement::Kill { kill, .. } => kill.render(ctx, f),
            Statement::Handler { handler, .. } => handler.render(ctx, f),
            Statement::Install { install, .. } => install.render(ctx, f),
            Statement::Uninstall { uninstall, .. } => uninstall.render(ctx, f),
            Statement::Shutdown { .. } => f.write_str("SHUTDOWN"),
            Statement::Restart { .. } => f.write_str("RESTART"),
            Statement::Clone { clone, .. } => clone.render(ctx, f),
            Statement::ImportTable { import_table, .. } => import_table.render(ctx, f),
            Statement::Help { help, .. } => help.render(ctx, f),
            Statement::Binlog { binlog, .. } => binlog.render(ctx, f),
            Statement::Pragma { pragma, .. } => pragma.render(ctx, f),
            Statement::Attach { attach, .. } => attach.render(ctx, f),
            Statement::Detach { detach, .. } => detach.render(ctx, f),
            Statement::Checkpoint { checkpoint, .. } => checkpoint.render(ctx, f),
            Statement::Load { load, .. } => load.render(ctx, f),
            Statement::LoadData { load_data, .. } => load_data.render(ctx, f),
            Statement::UpdateExtensions {
                update_extensions, ..
            } => update_extensions.render(ctx, f),
            Statement::Vacuum { vacuum, .. } => vacuum.render(ctx, f),
            Statement::Reindex { reindex, .. } => reindex.render(ctx, f),
            Statement::Analyze { analyze, .. } => analyze.render(ctx, f),
            Statement::TableMaintenance {
                table_maintenance, ..
            } => table_maintenance.render(ctx, f),
            Statement::CacheIndex { cache_index, .. } => cache_index.render(ctx, f),
            Statement::LoadIndex { load_index, .. } => load_index.render(ctx, f),
            Statement::Rename { rename, .. } => rename.render(ctx, f),
            Statement::Flush { flush, .. } => flush.render(ctx, f),
            Statement::Purge { purge, .. } => purge.render(ctx, f),
            Statement::Replication { replication, .. } => replication.render(ctx, f),
            Statement::CreateUser { create, .. } => create.render(ctx, f),
            Statement::AlterUser { alter, .. } => alter.render(ctx, f),
            Statement::UserRoleList { statement, .. } => statement.render(ctx, f),
            Statement::Use { use_statement, .. } => use_statement.render(ctx, f),
            Statement::CreateTrigger { create, .. } => create.render(ctx, f),
            Statement::CreateStoredTrigger { create, .. } => create.render(ctx, f),
            Statement::CreateMacro { create, .. } => create.render(ctx, f),
            Statement::CreateSecret { create, .. } => create.render(ctx, f),
            Statement::DropSecret { drop, .. } => drop.render(ctx, f),
            Statement::CreateType { create, .. } => create.render(ctx, f),
            Statement::CreateVirtualTable { create, .. } => create.render(ctx, f),
            Statement::CreateSequence { create, .. } => create.render(ctx, f),
            Statement::CreateExtension { create, .. } => create.render(ctx, f),
            Statement::AlterExtension { alter, .. } => alter.render(ctx, f),
            Statement::CreateTablespace { create, .. } => create.render(ctx, f),
            Statement::AlterTablespace { alter, .. } => alter.render(ctx, f),
            Statement::DropTablespace { drop, .. } => drop.render(ctx, f),
            Statement::CreateLogfileGroup { create, .. } => create.render(ctx, f),
            Statement::AlterLogfileGroup { alter, .. } => alter.render(ctx, f),
            Statement::DropLogfileGroup { drop, .. } => drop.render(ctx, f),
            Statement::AlterObjectDepends { alter, .. } => alter.render(ctx, f),
            Statement::AlterSystem { alter, .. } => alter.render(ctx, f),
            Statement::AlterDatabase { alter, .. } => alter.render(ctx, f),
            Statement::AlterDatabaseOptions { alter, .. } => alter.render(ctx, f),
            Statement::CreateServer { create, .. } => create.render(ctx, f),
            Statement::AlterServer { alter, .. } => alter.render(ctx, f),
            Statement::DropServer { drop, .. } => drop.render(ctx, f),
            Statement::AlterInstance { alter, .. } => alter.render(ctx, f),
            Statement::CreateSpatialReferenceSystem { create, .. } => create.render(ctx, f),
            Statement::DropSpatialReferenceSystem { drop, .. } => drop.render(ctx, f),
            Statement::CreateResourceGroup { create, .. } => create.render(ctx, f),
            Statement::AlterResourceGroup { alter, .. } => alter.render(ctx, f),
            Statement::DropResourceGroup { drop, .. } => drop.render(ctx, f),
            Statement::AlterSequence { alter, .. } => alter.render(ctx, f),
            Statement::AlterObjectSchema { alter, .. } => alter.render(ctx, f),
            Statement::Pivot { pivot, .. } => pivot.render(ctx, f),
            Statement::Unpivot { unpivot, .. } => unpivot.render(ctx, f),
            // The statement form is the bare `SHOW_REF` (`DESCRIBE …` / `SUMMARIZE …`),
            // unparenthesized — the `ShowRef` core already renders the keyword + target; the
            // parentheses are the table-factor position's alone.
            Statement::ShowRef { show, .. } => show.render(ctx, f),
            Statement::Prepare { prepare, .. } => prepare.render(ctx, f),
            Statement::Execute { execute, .. } => execute.render(ctx, f),
            Statement::PrepareFrom { prepare_from, .. } => prepare_from.render(ctx, f),
            Statement::ExecuteUsing { execute_using, .. } => execute_using.render(ctx, f),
            Statement::Deallocate { deallocate, .. } => deallocate.render(ctx, f),
            Statement::Call { call, .. } => call.render(ctx, f),
            Statement::Do { do_block, .. } => do_block.render(ctx, f),
            Statement::DoExpressions { do_expressions, .. } => do_expressions.render(ctx, f),
            Statement::LockTables { lock_tables, .. } => lock_tables.render(ctx, f),
            Statement::UnlockTables { unlock_tables, .. } => unlock_tables.render(ctx, f),
            Statement::InstanceLock { instance_lock, .. } => instance_lock.render(ctx, f),
            Statement::Compound { compound, .. } => compound.render(ctx, f),
            Statement::If { if_statement, .. } => if_statement.render(ctx, f),
            Statement::Case { case_statement, .. } => case_statement.render(ctx, f),
            Statement::Loop { loop_statement, .. } => loop_statement.render(ctx, f),
            Statement::While {
                while_statement, ..
            } => while_statement.render(ctx, f),
            Statement::Repeat { repeat, .. } => repeat.render(ctx, f),
            Statement::Leave { leave, .. } => leave.render(ctx, f),
            Statement::Iterate { iterate, .. } => iterate.render(ctx, f),
            Statement::Return {
                return_statement, ..
            } => return_statement.render(ctx, f),
            Statement::OpenCursor { open, .. } => open.render(ctx, f),
            Statement::FetchCursor { fetch, .. } => fetch.render(ctx, f),
            Statement::CloseCursor { close, .. } => close.render(ctx, f),
            Statement::Signal { signal, .. } => signal.render_as(ctx, f, "SIGNAL"),
            Statement::Resignal { resignal, .. } => resignal.render_as(ctx, f, "RESIGNAL"),
            Statement::GetDiagnostics {
                get_diagnostics, ..
            } => get_diagnostics.render(ctx, f),
            Statement::Other { ext, .. } => ext.render(ctx, f),
        }
    }
}

impl Render for RefreshMaterializedView {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("REFRESH MATERIALIZED VIEW")?;
        if self.concurrently {
            f.write_str(" CONCURRENTLY")?;
        }
        f.write_str(" ")?;
        self.name.render(ctx, f)?;
        match self.with_data {
            Some(true) => f.write_str(" WITH DATA"),
            Some(false) => f.write_str(" WITH NO DATA"),
            None => Ok(()),
        }
    }
}

impl Render for CreateColocationGroup {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE COLOCATION GROUP")?;
        if self.if_not_exists {
            f.write_str(" IF NOT EXISTS")?;
        }
        f.write_str(" ")?;
        self.name.render(ctx, f)?;
        f.write_str(match self.partition {
            ColocationPartitionKind::Hash => " PARTITION BY HASH (",
            ColocationPartitionKind::Range => " PARTITION BY RANGE (",
        })?;
        render_ident_list(&self.columns, ctx, f)?;
        f.write_str(") SHARDS ")?;
        self.shards.render(ctx, f)
    }
}

impl Render for DropColocationGroup {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DROP COLOCATION GROUP")?;
        if self.if_exists {
            f.write_str(" IF EXISTS")?;
        }
        f.write_str(" ")?;
        self.name.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for Insert<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(with) = &self.with {
            with.render(ctx, f)?;
            f.write_str(" ")?;
        }
        // `INTO` is mandatory after `INSERT` and rendered canonically after `REPLACE`
        // too (the MySQL shorthand `REPLACE t` parses but normalizes to `REPLACE INTO`,
        // matching how a bare DML alias normalizes to the `AS` spelling). The SQLite
        // `OR <action>` conflict prefix sits between the verb keyword and `INTO`; the
        // parser only sets it on the `INSERT` spelling (`REPLACE` takes none).
        f.write_str(match self.verb {
            InsertVerb::Insert => "INSERT",
            InsertVerb::Replace => "REPLACE",
        })?;
        if let Some(modifier) = self.modifier {
            f.write_str(match modifier {
                InsertModifier::Ignore => " IGNORE",
                InsertModifier::Overwrite => " OVERWRITE",
            })?;
        }
        if let Some(or_action) = self.or_action {
            f.write_str(" OR ")?;
            or_action.render(ctx, f)?;
        }
        f.write_str(" INTO ")?;
        self.target.render(ctx, f)?;
        if let Some(mode) = self.column_matching {
            f.write_str(match mode {
                InsertColumnMatching::ByName => " BY NAME",
                InsertColumnMatching::ByPosition => " BY POSITION",
            })?;
        }
        if let Some(overriding) = self.overriding {
            f.write_str(" ")?;
            overriding.render(ctx, f)?;
        }
        f.write_str(" ")?;
        self.source.render(ctx, f)?;
        // The MySQL row alias sits between the source and the `ON DUPLICATE KEY UPDATE`
        // clause; `render_table_alias` writes the leading ` AS `.
        render_table_alias(self.row_alias.as_ref(), ctx, f)?;
        if let Some(upsert) = &self.upsert {
            f.write_str(" ")?;
            upsert.render(ctx, f)?;
        }
        render_returning_clause(self.returning.as_ref(), ctx, f)
    }
}

impl Render for InsertTarget {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        render_alias(self.alias.as_ref(), self.alias_spelling, ctx, f)?;
        if !self.columns.is_empty() {
            f.write_str(" (")?;
            render_ident_list(&self.columns, ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl Render for InsertOverriding {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SystemValue => "OVERRIDING SYSTEM VALUE",
            Self::UserValue => "OVERRIDING USER VALUE",
        })
    }
}

impl Render for ConflictResolution {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Rollback => "ROLLBACK",
            Self::Abort => "ABORT",
            Self::Fail => "FAIL",
            Self::Ignore => "IGNORE",
            Self::Replace => "REPLACE",
        })
    }
}

impl<X: Extension + Render> Render for InsertSource<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefaultValues { default, .. } => {
                default.render(ctx, f)?;
                f.write_str(" VALUES")
            }
            Self::Values { values, .. } => values.render(ctx, f),
            Self::Query { query, .. } => query.render(ctx, f),
            Self::Set { assignments, .. } => {
                f.write_str("SET ")?;
                render_update_assignments(assignments, ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for InsertValues<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The `INSERT ... VALUES` source list uses bare `( ... )` rows on every dialect.
        f.write_str("VALUES ")?;
        render_values_rows(&self.rows, false, ctx, f)
    }
}

impl<X: Extension + Render> Render for InsertValue<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expr { expr, .. } => expr.render(ctx, f),
            Self::Default { default, .. } => default.render(ctx, f),
        }
    }
}

impl Render for DefaultValue {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DEFAULT")
    }
}

impl Render for DmlTarget {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        render_relation_inheritance(&self.inheritance, &self.name, ctx, f)?;
        render_alias(self.alias.as_ref(), self.alias_spelling, ctx, f)
    }
}

impl<X: Extension + Render> Render for Update<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(with) = &self.with {
            with.render(ctx, f)?;
            f.write_str(" ")?;
        }
        f.write_str("UPDATE")?;
        if let Some(or_action) = self.or_action {
            f.write_str(" OR ")?;
            or_action.render(ctx, f)?;
        }
        f.write_str(" ")?;
        self.target.render(ctx, f)?;
        for join in &self.target_joins {
            f.write_str(" ")?;
            join.render(ctx, f)?;
        }
        f.write_str(" SET ")?;
        render_update_assignments(&self.assignments, ctx, f)?;
        if !self.from.is_empty() {
            f.write_str(" FROM ")?;
            render_comma_separated(&self.from, ctx, f)?;
        }
        render_dml_selection(self.selection.as_ref(), ctx, f)?;
        render_mutation_order_by_limit(&self.order_by, self.limit.as_ref(), ctx, f)?;
        render_returning_clause(self.returning.as_ref(), ctx, f)
    }
}

impl<X: Extension + Render> Render for UpdateAssignment<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Single { target, value, .. } => {
                target.render(ctx, f)?;
                f.write_str(" = ")?;
                value.render(ctx, f)
            }
            Self::Tuple {
                targets, source, ..
            } => {
                f.write_str("(")?;
                render_comma_separated(targets, ctx, f)?;
                f.write_str(") = ")?;
                source.render(ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for UpdateValue<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expr { expr, .. } => expr.render(ctx, f),
            Self::Default { default, .. } => default.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for UpdateTupleSource<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Row {
                explicit, values, ..
            } => {
                f.write_str(if *explicit { "ROW(" } else { "(" })?;
                render_comma_separated(values, ctx, f)?;
                f.write_str(")")
            }
            Self::Subquery { query, .. } => {
                f.write_str("(")?;
                query.render(ctx, f)?;
                f.write_str(")")
            }
            Self::Default { default, .. } => default.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for Delete<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(with) = &self.with {
            with.render(ctx, f)?;
            f.write_str(" ")?;
        }
        f.write_str("DELETE FROM ")?;
        self.target.render(ctx, f)?;
        for target in &self.additional_targets {
            f.write_str(", ")?;
            target.render(ctx, f)?;
        }
        for join in &self.target_joins {
            f.write_str(" ")?;
            join.render(ctx, f)?;
        }
        if !self.using.is_empty() {
            f.write_str(" USING ")?;
            render_comma_separated(&self.using, ctx, f)?;
        }
        render_dml_selection(self.selection.as_ref(), ctx, f)?;
        render_mutation_order_by_limit(&self.order_by, self.limit.as_ref(), ctx, f)?;
        render_returning_clause(self.returning.as_ref(), ctx, f)
    }
}

/// Render a trailing ` WHERE ...` filter when present; the leading space is part of
/// the clause separator, mirroring the other mutation tails.
/// Render a non-empty comma-separated `UpdateAssignment` list — the shared `SET`
/// body of an `UPDATE`, a PostgreSQL `ON CONFLICT DO UPDATE`, and a MySQL
/// `ON DUPLICATE KEY UPDATE`. The `SET` keyword (and any clause lead-in) is the
/// caller's, so the three spellings differ only in their prefix, not this body.
fn render_update_assignments<X: Extension + Render>(
    assignments: &[UpdateAssignment<X>],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    for (i, assignment) in assignments.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        assignment.render(ctx, f)?;
    }
    Ok(())
}

fn render_dml_selection<X: Extension + Render>(
    selection: Option<&DmlSelection<X>>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(selection) = selection {
        f.write_str(" WHERE ")?;
        selection.render(ctx, f)?;
    }
    Ok(())
}

impl<X: Extension + Render> Render for DmlSelection<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Where { condition, .. } => condition.render(ctx, f),
            Self::CurrentOf { cursor, .. } => {
                f.write_str("CURRENT OF ")?;
                cursor.render(ctx, f)
            }
        }
    }
}

/// Render the MySQL `[ORDER BY <keys>] [LIMIT <count>]` row-limiting tails shared by a
/// single-table `UPDATE` and `DELETE`; each leading space is part of the clause
/// separator, mirroring [`Query`]'s `ORDER BY` / `LIMIT` rendering.
fn render_mutation_order_by_limit<X: Extension + Render>(
    order_by: &[OrderByExpr<X>],
    limit: Option<&Limit<X>>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if !order_by.is_empty() {
        f.write_str(" ORDER BY ")?;
        render_comma_separated(order_by, ctx, f)?;
    }
    if let Some(limit) = limit {
        f.write_str(" ")?;
        limit.render(ctx, f)?;
    }
    Ok(())
}

/// Render a trailing ` RETURNING <output> [, ...]` clause when present; the leading
/// space is part of the clause separator, mirroring the other mutation tails.
fn render_returning_clause<X: Extension + Render>(
    returning: Option<&Returning<X>>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(returning) = returning {
        f.write_str(" ")?;
        returning.render(ctx, f)?;
    }
    Ok(())
}

impl<X: Extension + Render> Render for Returning<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RETURNING ")?;
        render_comma_separated(&self.items, ctx, f)
    }
}

impl<X: Extension + Render> Render for Upsert<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Upsert::OnConflict { conflict, .. } => conflict.render(ctx, f),
            Upsert::OnDuplicateKeyUpdate { assignments, .. } => {
                f.write_str("ON DUPLICATE KEY UPDATE ")?;
                render_update_assignments(assignments, ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for OnConflict<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ON CONFLICT")?;
        if let Some(target) = &self.target {
            f.write_str(" ")?;
            target.render(ctx, f)?;
        }
        f.write_str(" ")?;
        self.action.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for ConflictTarget<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConflictTarget::Index {
                columns, predicate, ..
            } => {
                f.write_str("(")?;
                render_comma_separated(columns, ctx, f)?;
                f.write_str(")")?;
                if let Some(predicate) = predicate {
                    f.write_str(" WHERE ")?;
                    predicate.render(ctx, f)?;
                }
                Ok(())
            }
            ConflictTarget::Constraint { name, .. } => {
                f.write_str("ON CONSTRAINT ")?;
                name.render(ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for ConflictAction<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConflictAction::Nothing { .. } => f.write_str("DO NOTHING"),
            ConflictAction::Update {
                assignments,
                selection,
                ..
            } => {
                f.write_str("DO UPDATE SET ")?;
                render_update_assignments(assignments, ctx, f)?;
                if let Some(selection) = selection {
                    f.write_str(" WHERE ")?;
                    selection.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl<X: Extension + Render> Render for Merge<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(with) = &self.with {
            with.render(ctx, f)?;
            f.write_str(" ")?;
        }
        f.write_str("MERGE INTO ")?;
        self.target.render(ctx, f)?;
        f.write_str(" USING ")?;
        self.using.render(ctx, f)?;
        f.write_str(" ON ")?;
        self.on.render(ctx, f)?;
        for clause in &self.clauses {
            f.write_str(" ")?;
            clause.render(ctx, f)?;
        }
        render_returning_clause(self.returning.as_ref(), ctx, f)
    }
}

impl<X: Extension + Render> Render for MergeWhenClause<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `NotMatchedByTarget` renders the canonical bare spelling — `BY TARGET` is
        // the same production (both fold at parse), so the qualifier only appears
        // where it is load-bearing (`BY SOURCE`).
        f.write_str(match self.match_kind {
            MergeMatchKind::Matched => "WHEN MATCHED",
            MergeMatchKind::NotMatchedByTarget => "WHEN NOT MATCHED",
            MergeMatchKind::NotMatchedBySource => "WHEN NOT MATCHED BY SOURCE",
        })?;
        if let Some(condition) = &self.condition {
            f.write_str(" AND ")?;
            condition.render(ctx, f)?;
        }
        f.write_str(" THEN ")?;
        self.action.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for MergeAction<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MergeAction::Insert {
                columns,
                overriding,
                values,
                additional_rows,
                ..
            } => {
                f.write_str("INSERT")?;
                if !columns.is_empty() {
                    f.write_str(" (")?;
                    render_ident_list(columns, ctx, f)?;
                    f.write_str(")")?;
                }
                if let Some(overriding) = overriding {
                    f.write_str(" ")?;
                    overriding.render(ctx, f)?;
                }
                f.write_str(" VALUES (")?;
                render_comma_separated(values, ctx, f)?;
                f.write_str(")")?;
                for row in additional_rows {
                    f.write_str(", (")?;
                    render_comma_separated(row, ctx, f)?;
                    f.write_str(")")?;
                }
                Ok(())
            }
            MergeAction::InsertDefault { default, .. } => {
                f.write_str("INSERT ")?;
                default.render(ctx, f)?;
                f.write_str(" VALUES")
            }
            MergeAction::Update { assignments, .. } => {
                f.write_str("UPDATE SET ")?;
                render_update_assignments(assignments, ctx, f)
            }
            MergeAction::UpdateStar { .. } => f.write_str("UPDATE SET *"),
            MergeAction::InsertStar { .. } => f.write_str("INSERT *"),
            MergeAction::InsertByName { star, .. } => {
                f.write_str("INSERT BY NAME")?;
                if *star {
                    f.write_str(" *")?;
                }
                Ok(())
            }
            MergeAction::Error { .. } => f.write_str("ERROR"),
            MergeAction::Delete { .. } => f.write_str("DELETE"),
            MergeAction::DoNothing { .. } => f.write_str("DO NOTHING"),
        }
    }
}

impl Render for TransactionStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Begin {
                syntax,
                mode,
                block,
                name,
                modes,
                ..
            } => {
                match syntax {
                    TransactionStart::Begin => f.write_str("BEGIN")?,
                    TransactionStart::Start => {
                        f.write_str("START")?;
                        if honours_source_spelling(ctx) {
                            render_transaction_block_keyword(*block, f)?;
                        } else {
                            f.write_str(" TRANSACTION")?;
                        }
                    }
                }
                if let Some(mode) = mode {
                    f.write_str(" ")?;
                    mode.render(ctx, f)?;
                }
                // The `TRANSACTION`/`WORK` block noise word is exact-synonym fidelity —
                // replayed only by a source-fidelity render, dropped by a re-spell and
                // the redacted fingerprint. The START spelling handles its block word
                // above because its standard re-spell must restore `TRANSACTION`.
                if *syntax == TransactionStart::Begin {
                    render_transaction_block_and_name(*block, name.as_deref(), ctx, f)?;
                }
                render_transaction_modes(modes, ctx, f)
            }
            Self::Commit {
                syntax,
                block,
                name,
                chain,
                release,
                ..
            } => {
                if honours_source_spelling(ctx) && *syntax == TransactionCommitKeyword::End {
                    f.write_str("END")?;
                } else {
                    f.write_str("COMMIT")?;
                }
                render_transaction_block_and_name(*block, name.as_deref(), ctx, f)?;
                render_transaction_completion(*chain, *release, f)
            }
            Self::Rollback {
                syntax,
                block,
                name,
                savepoint_keyword,
                to_savepoint,
                chain,
                release,
                ..
            } => {
                if honours_source_spelling(ctx) && *syntax == TransactionRollbackKeyword::Abort {
                    f.write_str("ABORT")?;
                } else {
                    f.write_str("ROLLBACK")?;
                }
                render_transaction_block_and_name(*block, name.as_deref(), ctx, f)?;
                if let Some(name) = to_savepoint {
                    // The `SAVEPOINT` keyword is optional after `TO`; the canonical
                    // render emits it, a source-fidelity render drops it when the
                    // source did (`ROLLBACK TO x`).
                    if *savepoint_keyword || !honours_source_spelling(ctx) {
                        f.write_str(" TO SAVEPOINT ")?;
                    } else {
                        f.write_str(" TO ")?;
                    }
                    name.render(ctx, f)?;
                }
                render_transaction_completion(*chain, *release, f)
            }
            Self::Savepoint { name, .. } => {
                f.write_str("SAVEPOINT ")?;
                name.render(ctx, f)
            }
            Self::Release {
                savepoint_keyword,
                savepoint,
                ..
            } => {
                // The `SAVEPOINT` keyword is optional; the canonical render emits it, a
                // source-fidelity render drops it when the source did (`RELEASE x`).
                if *savepoint_keyword || !honours_source_spelling(ctx) {
                    f.write_str("RELEASE SAVEPOINT ")?;
                } else {
                    f.write_str("RELEASE ")?;
                }
                savepoint.render(ctx, f)
            }
            Self::SetCharacteristics { modes, .. } => {
                f.write_str("SET TRANSACTION")?;
                render_transaction_modes(modes, ctx, f)
            }
        }
    }
}

fn render_transaction_chain(chain: Option<bool>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match chain {
        Some(true) => f.write_str(" AND CHAIN"),
        Some(false) => f.write_str(" AND NO CHAIN"),
        None => Ok(()),
    }
}

fn render_transaction_completion(
    chain: Option<bool>,
    release: Option<bool>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    render_transaction_chain(chain, f)?;
    match release {
        Some(true) => f.write_str(" RELEASE"),
        Some(false) => f.write_str(" NO RELEASE"),
        None => Ok(()),
    }
}

/// Render the optional `TRANSACTION` / `WORK` block noise word (with its leading
/// space) after `BEGIN` / `COMMIT` / `ROLLBACK`; a `None` tag renders nothing. Only a
/// source-fidelity render calls this — the callers already gate on
/// [`honours_source_spelling`].
fn render_transaction_block_keyword(
    block: Option<TransactionBlockKeyword>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match block {
        Some(TransactionBlockKeyword::Transaction) => f.write_str(" TRANSACTION"),
        Some(TransactionBlockKeyword::Work) => f.write_str(" WORK"),
        None => Ok(()),
    }
}

fn render_transaction_block_and_name(
    block: Option<TransactionBlockKeyword>,
    name: Option<&Ident>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(name) = name {
        f.write_str(" TRANSACTION ")?;
        name.render(ctx, f)
    } else if honours_source_spelling(ctx) {
        render_transaction_block_keyword(block, f)
    } else {
        Ok(())
    }
}

/// Render a transaction mode list, each mode preceded by its separator (a leading
/// space for the first, `, ` thereafter), so an empty list renders nothing.
fn render_transaction_modes(
    modes: &[TransactionMode],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    render_leading_space_comma_separated(modes, ctx, f)
}

impl Render for TransactionModeKind {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Deferred => "DEFERRED",
            Self::Immediate => "IMMEDIATE",
            Self::Exclusive => "EXCLUSIVE",
        })
    }
}

impl Render for TransactionMode {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IsolationLevel { level, .. } => {
                f.write_str("ISOLATION LEVEL ")?;
                level.render(ctx, f)
            }
            Self::AccessMode { access, .. } => access.render(ctx, f),
            Self::Deferrable { deferrable, .. } => f.write_str(if *deferrable {
                "DEFERRABLE"
            } else {
                "NOT DEFERRABLE"
            }),
            Self::ConsistentSnapshot { .. } => f.write_str("WITH CONSISTENT SNAPSHOT"),
        }
    }
}

impl Render for IsolationLevel {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ReadUncommitted => "READ UNCOMMITTED",
            Self::ReadCommitted => "READ COMMITTED",
            Self::RepeatableRead => "REPEATABLE READ",
            Self::Serializable => "SERIALIZABLE",
        })
    }
}

impl Render for TransactionAccessMode {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ReadOnly => "READ ONLY",
            Self::ReadWrite => "READ WRITE",
        })
    }
}

impl Render for XaStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start {
                keyword,
                xid,
                association,
                ..
            } => {
                f.write_str(match keyword {
                    XaStartKeyword::Start => "XA START ",
                    XaStartKeyword::Begin => "XA BEGIN ",
                })?;
                xid.render(ctx, f)?;
                if let Some(association) = association {
                    f.write_str(match association {
                        XaAssociation::Join => " JOIN",
                        XaAssociation::Resume => " RESUME",
                    })?;
                }
                Ok(())
            }
            Self::End { xid, suspend, .. } => {
                f.write_str("XA END ")?;
                xid.render(ctx, f)?;
                if let Some(suspend) = suspend {
                    f.write_str(match suspend {
                        XaSuspend::Suspend => " SUSPEND",
                        XaSuspend::SuspendForMigrate => " SUSPEND FOR MIGRATE",
                    })?;
                }
                Ok(())
            }
            Self::Prepare { xid, .. } => {
                f.write_str("XA PREPARE ")?;
                xid.render(ctx, f)
            }
            Self::Commit { xid, one_phase, .. } => {
                f.write_str("XA COMMIT ")?;
                xid.render(ctx, f)?;
                if *one_phase {
                    f.write_str(" ONE PHASE")?;
                }
                Ok(())
            }
            Self::Rollback { xid, .. } => {
                f.write_str("XA ROLLBACK ")?;
                xid.render(ctx, f)
            }
            Self::Recover { convert_xid, .. } => {
                f.write_str("XA RECOVER")?;
                if *convert_xid {
                    f.write_str(" CONVERT XID")?;
                }
                Ok(())
            }
        }
    }
}

impl Render for Xid {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.gtrid.render(ctx, f)?;
        if let Some(bqual) = &self.bqual {
            f.write_str(", ")?;
            bqual.render(ctx, f)?;
        }
        if let Some(format_id) = &self.format_id {
            f.write_str(", ")?;
            format_id.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for SessionStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Set {
                scope,
                name,
                assignment,
                value,
                ..
            } => {
                f.write_str("SET")?;
                render_set_scope(*scope, ctx, f)?;
                f.write_str(" ")?;
                name.render(ctx, f)?;
                // `=` and `TO` are exact synonyms; a source-fidelity render replays the
                // written `=`, a target re-spell and the redacted fingerprint keep `TO`.
                f.write_str(match assignment {
                    SetAssignment::Equals if honours_source_spelling(ctx) => " = ",
                    _ => " TO ",
                })?;
                value.render(ctx, f)
            }
            Self::SetTimeZone { scope, value, .. } => {
                f.write_str("SET")?;
                render_set_scope(*scope, ctx, f)?;
                f.write_str(" TIME ZONE ")?;
                value.render(ctx, f)
            }
            Self::SetRole { scope, role, .. } => {
                f.write_str("SET")?;
                render_set_scope(*scope, ctx, f)?;
                f.write_str(" ROLE ")?;
                role.render(ctx, f)
            }
            Self::SetSessionAuthorization { scope, user, .. } => {
                f.write_str("SET")?;
                render_set_scope(*scope, ctx, f)?;
                f.write_str(" SESSION AUTHORIZATION ")?;
                user.render(ctx, f)
            }
            Self::SetConstraints {
                constraints,
                check_time,
                ..
            } => {
                f.write_str("SET CONSTRAINTS ")?;
                constraints.render(ctx, f)?;
                f.write_str(" ")?;
                check_time.render(ctx, f)
            }
            Self::SetNames { value, .. } => {
                f.write_str("SET NAMES ")?;
                value.render(ctx, f)
            }
            Self::SetSessionCharacteristics { modes, .. } => {
                f.write_str("SET SESSION CHARACTERISTICS AS TRANSACTION")?;
                render_transaction_modes(modes, ctx, f)
            }
            Self::Reset { scope, target, .. } => {
                f.write_str("RESET")?;
                render_set_scope(*scope, ctx, f)?;
                f.write_str(" ")?;
                target.render(ctx, f)
            }
            Self::Show {
                target, verbose, ..
            } => {
                f.write_str("SHOW ")?;
                target.render(ctx, f)?;
                if *verbose {
                    f.write_str(" VERBOSE")?;
                }
                Ok(())
            }
            Self::SetVariables { assignments, .. } => {
                f.write_str("SET ")?;
                render_comma_separated(assignments, ctx, f)
            }
            Self::SetCharacterSet { keyword, value, .. } => {
                f.write_str(match keyword {
                    CharacterSetKeyword::CharacterSet => "SET CHARACTER SET ",
                    CharacterSetKeyword::Charset => "SET CHARSET ",
                })?;
                value.render(ctx, f)
            }
            Self::SetResourceGroup {
                name, thread_ids, ..
            } => {
                f.write_str("SET RESOURCE GROUP ")?;
                name.render(ctx, f)?;
                if let Some(thread_ids) = thread_ids {
                    f.write_str(" FOR ")?;
                    render_comma_separated(thread_ids, ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl<X: Extension + Render> Render for SetVariableAssignment<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SystemVariable {
                scope,
                name,
                assignment,
                value,
                ..
            } => {
                scope.render_prefix(name, ctx, f)?;
                render_mysql_set_assignment(*assignment, ctx, f)?;
                value.render(ctx, f)
            }
            Self::UserVariable {
                name,
                assignment,
                value,
                ..
            } => {
                f.write_str("@")?;
                name.render(ctx, f)?;
                render_mysql_set_assignment(*assignment, ctx, f)?;
                value.render(ctx, f)
            }
        }
    }
}

/// Render the `=` / `:=` separator of a MySQL variable assignment. The two are exact
/// synonyms; a source-fidelity render replays the written `:=`, a target re-spell and the
/// redacted fingerprint normalize to `=` (there is no `TO` spelling in MySQL's `SET`).
fn render_mysql_set_assignment(
    assignment: SetAssignment,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str(match assignment {
        SetAssignment::ColonEquals if honours_source_spelling(ctx) => " := ",
        _ => " = ",
    })
}

impl SystemVariableScope {
    /// Render the scope prefix and the variable `name` together — a keyword prefix
    /// (`GLOBAL x`), the `@@` sigil (`@@x` / `@@global.x`), or the bare name.
    fn render_prefix(
        self,
        name: &ObjectName,
        ctx: &RenderCtx<'_>,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::Implicit => name.render(ctx, f),
            Self::Keyword(kind) => {
                kind.render(ctx, f)?;
                f.write_str(" ")?;
                name.render(ctx, f)
            }
            Self::AtAt => {
                f.write_str("@@")?;
                name.render(ctx, f)
            }
            Self::AtAtScoped(kind) => {
                // The `@@scope.` prefix renders lowercase, matching the canonical
                // `Expr::SessionVariable` spelling (`@@global.` / `@@session.`).
                f.write_str("@@")?;
                f.write_str(kind.at_at_prefix())?;
                f.write_str(".")?;
                name.render(ctx, f)
            }
        }
    }
}

impl SystemVariableScopeKind {
    /// The lowercase `@@`-form scope prefix word (`global`/`session`/…), matching the
    /// canonical `Expr::SessionVariable` spelling.
    fn at_at_prefix(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Session => "session",
            Self::Local => "local",
            Self::Persist => "persist",
            Self::PersistOnly => "persist_only",
        }
    }
}

impl Render for SystemVariableScopeKind {
    /// The uppercase keyword-prefix spelling (`SET GLOBAL x = …`).
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Global => "GLOBAL",
            Self::Session => "SESSION",
            Self::Local => "LOCAL",
            Self::Persist => "PERSIST",
            Self::PersistOnly => "PERSIST_ONLY",
        })
    }
}

impl<X: Extension + Render> Render for SetVariableValue<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default { .. } => f.write_str("DEFAULT"),
            Self::Keyword { keyword, .. } => keyword.render(ctx, f),
            Self::Expr { expr, .. } => expr.render(ctx, f),
        }
    }
}

impl Render for SetVariableKeyword {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::On => "ON",
            Self::All => "ALL",
            Self::Binary => "BINARY",
            Self::Row => "ROW",
            Self::System => "SYSTEM",
        })
    }
}

impl Render for SetCharacterSetValue {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default { .. } => f.write_str("DEFAULT"),
            Self::Charset { charset, .. } => charset.render(ctx, f),
        }
    }
}

impl Render for AlterSystem {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER SYSTEM ")?;
        match &self.action {
            AlterSystemAction::Set {
                name,
                assignment,
                value,
                ..
            } => {
                f.write_str("SET ")?;
                name.render(ctx, f)?;
                // `=` and `TO` are exact synonyms; a source-fidelity render replays the
                // written `=`, a target re-spell and the redacted fingerprint keep `TO`.
                f.write_str(match assignment {
                    SetAssignment::Equals if honours_source_spelling(ctx) => " = ",
                    _ => " TO ",
                })?;
                value.render(ctx, f)
            }
            AlterSystemAction::Reset { target, .. } => {
                f.write_str("RESET ")?;
                target.render(ctx, f)
            }
        }
    }
}

impl Render for AlterDatabase {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER DATABASE ")?;
        if self.if_exists {
            f.write_str("IF EXISTS ")?;
        }
        self.name.render(ctx, f)?;
        match &self.action {
            AlterDatabaseAction::SetAlias { new_name, .. } => {
                f.write_str(" SET ALIAS TO ")?;
                new_name.render(ctx, f)
            }
        }
    }
}

impl Render for AlterDatabaseOptions {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.spelling {
            DatabaseKeyword::Database => "ALTER DATABASE",
            DatabaseKeyword::Schema => "ALTER SCHEMA",
        })?;
        if let Some(name) = &self.name {
            f.write_str(" ")?;
            name.render(ctx, f)?;
        }
        for option in &self.options {
            f.write_str(" ")?;
            option.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for AlterDatabaseOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `[=]` presence round-trips verbatim (the `IndexLockAlgorithmOption` precedent), so the
        // written spacing is reproduced in both source-fidelity and canonical renders.
        let sep = |equals: bool| if equals { " = " } else { " " };
        match self {
            Self::CharacterSet {
                default,
                keyword,
                equals,
                charset,
                ..
            } => {
                if *default {
                    f.write_str("DEFAULT ")?;
                }
                f.write_str(match keyword {
                    CharsetKeyword::CharacterSet => "CHARACTER SET",
                    CharsetKeyword::Charset => "CHARSET",
                })?;
                f.write_str(sep(*equals))?;
                charset.render(ctx, f)
            }
            Self::Collate {
                default,
                equals,
                collation,
                ..
            } => {
                if *default {
                    f.write_str("DEFAULT ")?;
                }
                f.write_str("COLLATE")?;
                f.write_str(sep(*equals))?;
                collation.render(ctx, f)
            }
            Self::Encryption {
                default,
                equals,
                value,
                ..
            } => {
                if *default {
                    f.write_str("DEFAULT ")?;
                }
                f.write_str("ENCRYPTION")?;
                f.write_str(sep(*equals))?;
                value.render(ctx, f)
            }
            Self::ReadOnly { equals, value, .. } => {
                f.write_str("READ ONLY")?;
                f.write_str(sep(*equals))?;
                f.write_str(match value {
                    ReadOnlyValue::Default => "DEFAULT",
                    ReadOnlyValue::Off => "0",
                    ReadOnlyValue::On => "1",
                })
            }
        }
    }
}

impl Render for CreateServer {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE SERVER ")?;
        self.name.render(ctx, f)?;
        f.write_str(" FOREIGN DATA WRAPPER ")?;
        self.wrapper.render(ctx, f)?;
        f.write_str(" OPTIONS (")?;
        render_server_options(&self.options, ctx, f)?;
        f.write_str(")")
    }
}

impl Render for AlterServer {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER SERVER ")?;
        self.name.render(ctx, f)?;
        f.write_str(" OPTIONS (")?;
        render_server_options(&self.options, ctx, f)?;
        f.write_str(")")
    }
}

impl Render for DropServer {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DROP SERVER ")?;
        if self.if_exists {
            f.write_str("IF EXISTS ")?;
        }
        self.name.render(ctx, f)
    }
}

/// The shared `<option>[, ...]` body of a `CREATE`/`ALTER SERVER` `OPTIONS ( … )` list
/// (comma-separated, no surrounding parentheses — the caller writes those).
fn render_server_options(
    options: &[ServerOption],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    for (i, option) in options.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        option.render(ctx, f)?;
    }
    Ok(())
}

impl Render for ServerOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            ServerOptionKind::Host => "HOST ",
            ServerOptionKind::Database => "DATABASE ",
            ServerOptionKind::User => "USER ",
            ServerOptionKind::Password => "PASSWORD ",
            ServerOptionKind::Socket => "SOCKET ",
            ServerOptionKind::Owner => "OWNER ",
            ServerOptionKind::Port => "PORT ",
        })?;
        self.value.render(ctx, f)
    }
}

impl Render for CreateSpatialReferenceSystem {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE ")?;
        if self.or_replace {
            f.write_str("OR REPLACE ")?;
        }
        f.write_str("SPATIAL REFERENCE SYSTEM ")?;
        if self.if_not_exists {
            f.write_str("IF NOT EXISTS ")?;
        }
        self.srid.render(ctx, f)?;
        for attribute in &self.attributes {
            f.write_str(" ")?;
            attribute.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for SrsAttribute {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { value, .. } => {
                f.write_str("NAME ")?;
                value.render(ctx, f)
            }
            Self::Definition { value, .. } => {
                f.write_str("DEFINITION ")?;
                value.render(ctx, f)
            }
            Self::Organization {
                organization,
                identifier,
                ..
            } => {
                f.write_str("ORGANIZATION ")?;
                organization.render(ctx, f)?;
                f.write_str(" IDENTIFIED BY ")?;
                identifier.render(ctx, f)
            }
            Self::Description { value, .. } => {
                f.write_str("DESCRIPTION ")?;
                value.render(ctx, f)
            }
        }
    }
}

impl Render for DropSpatialReferenceSystem {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DROP SPATIAL REFERENCE SYSTEM ")?;
        if self.if_exists {
            f.write_str("IF EXISTS ")?;
        }
        self.srid.render(ctx, f)
    }
}

impl Render for CreateResourceGroup {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE RESOURCE GROUP ")?;
        self.name.render(ctx, f)?;
        f.write_str(if self.type_equals {
            " TYPE = "
        } else {
            " TYPE "
        })?;
        self.group_type.render(ctx, f)?;
        if let Some(vcpu) = &self.vcpu {
            f.write_str(" ")?;
            vcpu.render(ctx, f)?;
        }
        if let Some(priority) = &self.thread_priority {
            f.write_str(" ")?;
            priority.render(ctx, f)?;
        }
        if let Some(state) = &self.state {
            f.write_str(" ")?;
            state.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for AlterResourceGroup {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER RESOURCE GROUP ")?;
        self.name.render(ctx, f)?;
        if let Some(vcpu) = &self.vcpu {
            f.write_str(" ")?;
            vcpu.render(ctx, f)?;
        }
        if let Some(priority) = &self.thread_priority {
            f.write_str(" ")?;
            priority.render(ctx, f)?;
        }
        if let Some(state) = &self.state {
            f.write_str(" ")?;
            state.render(ctx, f)?;
        }
        if self.force {
            f.write_str(" FORCE")?;
        }
        Ok(())
    }
}

impl Render for DropResourceGroup {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DROP RESOURCE GROUP ")?;
        self.name.render(ctx, f)?;
        if self.force {
            f.write_str(" FORCE")?;
        }
        Ok(())
    }
}

impl Render for ResourceGroupType {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::System => "SYSTEM",
            Self::User => "USER",
        })
    }
}

impl Render for ResourceGroupState {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Enable => "ENABLE",
            Self::Disable => "DISABLE",
        })
    }
}

impl Render for ResourceGroupVcpu {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.equals { "VCPU = " } else { "VCPU " })?;
        for (i, range) in self.ranges.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            range.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for VcpuRange {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.start.render(ctx, f)?;
        if let Some(end) = &self.end {
            f.write_str("-")?;
            end.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for ResourceGroupThreadPriority {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.equals {
            "THREAD_PRIORITY = "
        } else {
            "THREAD_PRIORITY "
        })?;
        if self.negative {
            f.write_str("-")?;
        }
        self.value.render(ctx, f)
    }
}

impl Render for AlterInstance {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER INSTANCE ")?;
        self.action.render(ctx, f)
    }
}

impl Render for AlterInstanceAction {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RotateInnodbMasterKey { .. } => f.write_str("ROTATE INNODB MASTER KEY"),
            Self::RotateBinlogMasterKey { .. } => f.write_str("ROTATE BINLOG MASTER KEY"),
            Self::ReloadTls {
                channel,
                no_rollback_on_error,
                ..
            } => {
                f.write_str("RELOAD TLS")?;
                if let Some(channel) = channel {
                    f.write_str(" FOR CHANNEL ")?;
                    channel.render(ctx, f)?;
                }
                if *no_rollback_on_error {
                    f.write_str(" NO ROLLBACK ON ERROR")?;
                }
                Ok(())
            }
            Self::ReloadKeyring { .. } => f.write_str("RELOAD KEYRING"),
            Self::EnableInnodbRedoLog { .. } => f.write_str("ENABLE INNODB REDO_LOG"),
            Self::DisableInnodbRedoLog { .. } => f.write_str("DISABLE INNODB REDO_LOG"),
        }
    }
}

impl<X: Extension + Render> Render for AlterSequence<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER SEQUENCE ")?;
        if self.if_exists {
            f.write_str("IF EXISTS ")?;
        }
        self.name.render(ctx, f)?;
        // Options are space-separated in their canonical spelling; the shared core reuses the
        // `IdentityOption` render.
        for option in &self.options {
            f.write_str(" ")?;
            option.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for AlterSequenceOption<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Common { option, .. } => option.render(ctx, f),
            Self::Restart { value: None, .. } => f.write_str("RESTART"),
            Self::Restart {
                value: Some(expr), ..
            } => {
                f.write_str("RESTART WITH ")?;
                expr.render(ctx, f)
            }
            Self::As { data_type, .. } => {
                f.write_str("AS ")?;
                data_type.render(ctx, f)
            }
            Self::OwnedBy { owner: None, .. } => f.write_str("OWNED BY NONE"),
            Self::OwnedBy {
                owner: Some(name), ..
            } => {
                f.write_str("OWNED BY ")?;
                name.render(ctx, f)
            }
            Self::SequenceName { name, .. } => {
                f.write_str("SEQUENCE NAME ")?;
                name.render(ctx, f)
            }
        }
    }
}

impl Render for AlterObjectSchema {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER ")?;
        f.write_str(match self.object_type {
            SchemaRelocationObject::Table => "TABLE ",
            SchemaRelocationObject::View => "VIEW ",
            SchemaRelocationObject::Sequence => "SEQUENCE ",
        })?;
        if self.if_exists {
            f.write_str("IF EXISTS ")?;
        }
        self.name.render(ctx, f)?;
        f.write_str(" SET SCHEMA ")?;
        self.new_schema.render(ctx, f)
    }
}

/// Render an optional `SET` scope as ` SESSION` / ` LOCAL` (leading space), or
/// nothing when absent — shared by the generic and special `SET` forms.
fn render_set_scope(
    scope: Option<SetScope>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(scope) = scope {
        f.write_str(" ")?;
        scope.render(ctx, f)?;
    }
    Ok(())
}

impl Render for SpecialSetValue {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default { .. } => f.write_str("DEFAULT"),
            Self::Local { .. } => f.write_str("LOCAL"),
            Self::None { .. } => f.write_str("NONE"),
            Self::Value { value, .. } => value.render(ctx, f),
        }
    }
}

impl Render for ConstraintsTarget {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All { .. } => f.write_str("ALL"),
            Self::Names { names, .. } => render_comma_separated(names, ctx, f),
        }
    }
}

impl Render for ConstraintCheckTime {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Deferred => "DEFERRED",
            Self::Immediate => "IMMEDIATE",
        })
    }
}

impl Render for SetNamesValue {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default { .. } => f.write_str("DEFAULT"),
            Self::Charset {
                charset, collation, ..
            } => {
                charset.render(ctx, f)?;
                if let Some(collation) = collation {
                    f.write_str(" COLLATE ")?;
                    collation.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl Render for SetScope {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Session => "SESSION",
            Self::Local => "LOCAL",
            Self::Global => "GLOBAL",
        })
    }
}

impl Render for SetValue {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default { .. } => f.write_str("DEFAULT"),
            Self::Values { values, .. } => render_comma_separated(values, ctx, f),
        }
    }
}

impl Render for SetParameterValue {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal { literal, .. } => literal.render(ctx, f),
            Self::Name { name, .. } => name.render(ctx, f),
            Self::Parameter { kind, .. } => render_parameter_kind(*kind, ctx, f),
            Self::List { values, .. } => {
                f.write_str("[")?;
                render_comma_separated(values, ctx, f)?;
                f.write_str("]")
            }
        }
    }
}

fn render_parameter_kind(
    kind: ParameterKind,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match kind {
        ParameterKind::Positional(index) => write!(f, "${index}"),
        ParameterKind::PositionalLarge { digits } => write!(f, "${}", ctx.resolve(digits)),
        ParameterKind::Numbered(index) => write!(f, "?{index}"),
        ParameterKind::Anonymous => f.write_str("?"),
        ParameterKind::Named { name, sigil } => {
            let sigil = match sigil {
                ParameterSigil::Colon => ':',
                ParameterSigil::At => '@',
                ParameterSigil::Dollar => '$',
            };
            write!(f, "{sigil}{}", ctx.resolve(name))
        }
    }
}

impl Render for ConfigParameter {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All { .. } => f.write_str("ALL"),
            Self::Named { name, .. } => name.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for AccessControlStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlterRoleRename { name, new_name, .. } => {
                f.write_str("ALTER ROLE ")?;
                name.render(ctx, f)?;
                f.write_str(" RENAME TO ")?;
                new_name.render(ctx, f)
            }
            Self::Grant {
                privileges,
                object,
                grantees,
                with_grant_option,
                granted_by,
                ..
            } => {
                f.write_str("GRANT ")?;
                privileges.render(ctx, f)?;
                f.write_str(" ON ")?;
                object.render(ctx, f)?;
                f.write_str(" TO ")?;
                render_comma_separated(grantees, ctx, f)?;
                if *with_grant_option {
                    f.write_str(" WITH GRANT OPTION")?;
                }
                render_granted_by(granted_by, ctx, f)
            }
            Self::Revoke {
                grant_option_for,
                privileges,
                object,
                grantees,
                granted_by,
                behavior,
                ..
            } => {
                f.write_str("REVOKE ")?;
                if *grant_option_for {
                    f.write_str("GRANT OPTION FOR ")?;
                }
                privileges.render(ctx, f)?;
                f.write_str(" ON ")?;
                object.render(ctx, f)?;
                f.write_str(" FROM ")?;
                render_comma_separated(grantees, ctx, f)?;
                render_granted_by(granted_by, ctx, f)?;
                render_drop_behavior(*behavior, ctx, f)
            }
            Self::GrantRole {
                roles,
                grantees,
                with_admin_option,
                granted_by,
                ..
            } => {
                f.write_str("GRANT ")?;
                render_ident_list(roles, ctx, f)?;
                f.write_str(" TO ")?;
                render_comma_separated(grantees, ctx, f)?;
                if *with_admin_option {
                    f.write_str(" WITH ADMIN OPTION")?;
                }
                render_granted_by(granted_by, ctx, f)
            }
            Self::RevokeRole {
                admin_option_for,
                roles,
                grantees,
                granted_by,
                behavior,
                ..
            } => {
                f.write_str("REVOKE ")?;
                if *admin_option_for {
                    f.write_str("ADMIN OPTION FOR ")?;
                }
                render_ident_list(roles, ctx, f)?;
                f.write_str(" FROM ")?;
                render_comma_separated(grantees, ctx, f)?;
                render_granted_by(granted_by, ctx, f)?;
                render_drop_behavior(*behavior, ctx, f)
            }
            Self::AccountGrantPrivilege {
                privileges,
                object,
                grantees,
                with_grant_option,
                grant_as,
                ..
            } => {
                f.write_str("GRANT ")?;
                privileges.render(ctx, f)?;
                f.write_str(" ON ")?;
                object.render(ctx, f)?;
                f.write_str(" TO ")?;
                render_comma_separated(grantees, ctx, f)?;
                if *with_grant_option {
                    f.write_str(" WITH GRANT OPTION")?;
                }
                if let Some(grant_as) = grant_as {
                    f.write_str(" ")?;
                    grant_as.render(ctx, f)?;
                }
                Ok(())
            }
            Self::AccountGrantProxy {
                proxy,
                grantees,
                with_grant_option,
                ..
            } => {
                f.write_str("GRANT PROXY ON ")?;
                proxy.render(ctx, f)?;
                f.write_str(" TO ")?;
                render_comma_separated(grantees, ctx, f)?;
                if *with_grant_option {
                    f.write_str(" WITH GRANT OPTION")?;
                }
                Ok(())
            }
            Self::AccountGrantRole {
                roles,
                grantees,
                with_admin_option,
                ..
            } => {
                f.write_str("GRANT ")?;
                render_comma_separated(roles, ctx, f)?;
                f.write_str(" TO ")?;
                render_comma_separated(grantees, ctx, f)?;
                if *with_admin_option {
                    f.write_str(" WITH ADMIN OPTION")?;
                }
                Ok(())
            }
            Self::AccountRevokePrivilege {
                if_exists,
                privileges,
                object,
                grantees,
                ignore_unknown_user,
                ..
            } => {
                f.write_str("REVOKE ")?;
                if *if_exists {
                    f.write_str("IF EXISTS ")?;
                }
                privileges.render(ctx, f)?;
                f.write_str(" ON ")?;
                object.render(ctx, f)?;
                f.write_str(" FROM ")?;
                render_comma_separated(grantees, ctx, f)?;
                render_ignore_unknown_user(*ignore_unknown_user, f)
            }
            Self::AccountRevokeAll {
                if_exists,
                privileges_keyword,
                grantees,
                ignore_unknown_user,
                ..
            } => {
                f.write_str("REVOKE ")?;
                if *if_exists {
                    f.write_str("IF EXISTS ")?;
                }
                // The optional `PRIVILEGES` noise word is fidelity-only; the canonical render
                // emits it.
                if *privileges_keyword || !honours_source_spelling(ctx) {
                    f.write_str("ALL PRIVILEGES, GRANT OPTION FROM ")?;
                } else {
                    f.write_str("ALL, GRANT OPTION FROM ")?;
                }
                render_comma_separated(grantees, ctx, f)?;
                render_ignore_unknown_user(*ignore_unknown_user, f)
            }
            Self::AccountRevokeProxy {
                if_exists,
                proxy,
                grantees,
                ignore_unknown_user,
                ..
            } => {
                f.write_str("REVOKE ")?;
                if *if_exists {
                    f.write_str("IF EXISTS ")?;
                }
                f.write_str("PROXY ON ")?;
                proxy.render(ctx, f)?;
                f.write_str(" FROM ")?;
                render_comma_separated(grantees, ctx, f)?;
                render_ignore_unknown_user(*ignore_unknown_user, f)
            }
            Self::AccountRevokeRole {
                if_exists,
                roles,
                grantees,
                ignore_unknown_user,
                ..
            } => {
                f.write_str("REVOKE ")?;
                if *if_exists {
                    f.write_str("IF EXISTS ")?;
                }
                render_comma_separated(roles, ctx, f)?;
                f.write_str(" FROM ")?;
                render_comma_separated(grantees, ctx, f)?;
                render_ignore_unknown_user(*ignore_unknown_user, f)
            }
        }
    }
}

/// Render the MySQL `IGNORE UNKNOWN USER` trailer, when present.
fn render_ignore_unknown_user(
    ignore_unknown_user: bool,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if ignore_unknown_user {
        f.write_str(" IGNORE UNKNOWN USER")?;
    }
    Ok(())
}

impl Render for PrivilegeLevelObject {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.object_type {
            PrivilegeObjectType::Table { explicit } => {
                if explicit {
                    f.write_str("TABLE ")?;
                }
            }
            PrivilegeObjectType::Function => f.write_str("FUNCTION ")?,
            PrivilegeObjectType::Procedure => f.write_str("PROCEDURE ")?,
        }
        self.level.render(ctx, f)
    }
}

impl Render for PrivilegeLevel {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Global { .. } => f.write_str("*.*"),
            Self::CurrentDatabase { .. } => f.write_str("*"),
            Self::Database { database, .. } => {
                database.render(ctx, f)?;
                f.write_str(".*")
            }
            Self::Object { name, .. } => name.render(ctx, f),
        }
    }
}

impl Render for GrantAs {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AS ")?;
        self.user.render(ctx, f)?;
        if let Some(with_role) = &self.with_role {
            f.write_str(" WITH ROLE ")?;
            with_role.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for WithRoleSpec {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Roles { roles, .. } => render_comma_separated(roles, ctx, f),
            Self::All { except, .. } => {
                f.write_str("ALL")?;
                if !except.is_empty() {
                    f.write_str(" EXCEPT ")?;
                    render_comma_separated(except, ctx, f)?;
                }
                Ok(())
            }
            Self::None { .. } => f.write_str("NONE"),
            Self::Default { .. } => f.write_str("DEFAULT"),
        }
    }
}

/// Render an optional `GRANTED BY <grantor>` trailer.
fn render_granted_by(
    granted_by: &Option<RoleSpec>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(grantor) = granted_by {
        f.write_str(" GRANTED BY ")?;
        grantor.render(ctx, f)?;
    }
    Ok(())
}

impl Render for Privileges {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The optional `PRIVILEGES` keyword is exact-synonym sugar: the canonical
            // render emits it, a source-fidelity render drops it when the source did.
            Self::All {
                privileges_keyword, ..
            } => {
                if *privileges_keyword || !honours_source_spelling(ctx) {
                    f.write_str("ALL PRIVILEGES")
                } else {
                    f.write_str("ALL")
                }
            }
            Self::List { privileges, .. } => render_comma_separated(privileges, ctx, f),
        }
    }
}

impl Render for Privilege {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Known { kind, columns, .. } => {
                kind.render(ctx, f)?;
                render_privilege_columns(columns, ctx, f)
            }
            Self::Other { name, columns, .. } => {
                name.render(ctx, f)?;
                render_privilege_columns(columns, ctx, f)
            }
        }
    }
}

/// Render a privilege's optional `( <column> [, ...] )` scope.
fn render_privilege_columns(
    columns: &[Ident],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if !columns.is_empty() {
        f.write_str(" (")?;
        render_ident_list(columns, ctx, f)?;
        f.write_str(")")?;
    }
    Ok(())
}

impl Render for PrivilegeKind {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Select => "SELECT",
            Self::Insert => "INSERT",
            Self::Update => "UPDATE",
            Self::Delete => "DELETE",
            Self::Truncate => "TRUNCATE",
            Self::References => "REFERENCES",
            Self::Trigger => "TRIGGER",
            Self::Usage => "USAGE",
            Self::Execute => "EXECUTE",
            Self::Create => "CREATE",
            Self::Connect => "CONNECT",
            Self::Temporary => "TEMPORARY",
            Self::Temp => "TEMP",
            Self::Maintain => "MAINTAIN",
            Self::Index => "INDEX",
            Self::Alter => "ALTER",
            Self::Drop => "DROP",
            Self::Reload => "RELOAD",
            Self::Shutdown => "SHUTDOWN",
            Self::Process => "PROCESS",
            Self::File => "FILE",
            Self::Super => "SUPER",
            Self::Event => "EVENT",
            Self::GrantOption => "GRANT OPTION",
            Self::ShowDatabases => "SHOW DATABASES",
            Self::CreateTemporaryTables => "CREATE TEMPORARY TABLES",
            Self::LockTables => "LOCK TABLES",
            Self::ReplicationSlave => "REPLICATION SLAVE",
            Self::ReplicationClient => "REPLICATION CLIENT",
            Self::CreateView => "CREATE VIEW",
            Self::ShowView => "SHOW VIEW",
            Self::CreateRoutine => "CREATE ROUTINE",
            Self::AlterRoutine => "ALTER ROUTINE",
            Self::CreateUser => "CREATE USER",
            Self::CreateTablespace => "CREATE TABLESPACE",
            Self::CreateRole => "CREATE ROLE",
            Self::DropRole => "DROP ROLE",
        })
    }
}

impl<X: Extension + Render> Render for GrantObject<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Table {
                explicit, names, ..
            } => {
                if *explicit {
                    f.write_str("TABLE ")?;
                }
                render_comma_separated(names, ctx, f)
            }
            Self::Named { kind, names, .. } => {
                kind.render(ctx, f)?;
                f.write_str(" ")?;
                render_comma_separated(names, ctx, f)
            }
            Self::Routines { kind, routines, .. } => {
                kind.render(ctx, f)?;
                f.write_str(" ")?;
                render_comma_separated(routines, ctx, f)
            }
            Self::AllInSchema { kind, schemas, .. } => {
                f.write_str("ALL ")?;
                kind.render(ctx, f)?;
                f.write_str(" IN SCHEMA ")?;
                render_comma_separated(schemas, ctx, f)
            }
        }
    }
}

impl Render for NamedObjectKind {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Sequence => "SEQUENCE",
            Self::Database => "DATABASE",
            Self::Schema => "SCHEMA",
            Self::Domain => "DOMAIN",
            Self::Type => "TYPE",
            Self::Language => "LANGUAGE",
            Self::Tablespace => "TABLESPACE",
            Self::ForeignDataWrapper => "FOREIGN DATA WRAPPER",
            Self::ForeignServer => "FOREIGN SERVER",
        })
    }
}

impl Render for RoutineObjectKind {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Function => "FUNCTION",
            Self::Procedure => "PROCEDURE",
            Self::Routine => "ROUTINE",
        })
    }
}

impl Render for SchemaObjectKind {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Tables => "TABLES",
            Self::Sequences => "SEQUENCES",
            Self::Functions => "FUNCTIONS",
            Self::Procedures => "PROCEDURES",
            Self::Routines => "ROUTINES",
        })
    }
}

impl<X: Extension + Render> Render for RoutineSignature<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        if let Some(arg_types) = &self.arg_types {
            f.write_str("(")?;
            render_comma_separated(arg_types, ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl Render for Grantee {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.group {
            f.write_str("GROUP ")?;
        }
        self.spec.render(ctx, f)
    }
}

impl Render for RoleSpec {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public { .. } => f.write_str("PUBLIC"),
            Self::CurrentRole { .. } => f.write_str("CURRENT_ROLE"),
            Self::CurrentUser { .. } => f.write_str("CURRENT_USER"),
            Self::SessionUser { .. } => f.write_str("SESSION_USER"),
            Self::Name { name, .. } => name.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for CopyStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("COPY ")?;
        // The `opt_binary` prefix, when written.
        if self.binary {
            f.write_str("BINARY ")?;
        }
        self.source.render(ctx, f)?;
        f.write_str(" ")?;
        self.direction.render(ctx, f)?;
        f.write_str(" ")?;
        self.target.render(ctx, f)?;
        // The `[USING] DELIMITERS '<str>'` clause, rendered in canonical form
        // (the optional `USING` is dropped) between the endpoint and the options.
        if let Some(delimiters) = &self.delimiters {
            f.write_str(" DELIMITERS ")?;
            delimiters.render(ctx, f)?;
        }
        // An empty option list renders nothing in either spelling, so the surface
        // tag only matters when populated. The parenthesized spelling canonicalizes
        // the optional `WITH` as present; the legacy spelling is space-separated
        // with neither `WITH` nor parentheses.
        if !self.options.is_empty() {
            if self.parenthesized {
                f.write_str(" WITH (")?;
                render_comma_separated(&self.options, ctx, f)?;
                f.write_str(")")?;
            } else {
                for option in &self.options {
                    f.write_str(" ")?;
                    option.render(ctx, f)?;
                }
            }
        }
        // The `COPY FROM ... WHERE <predicate>` filter, last (the parser only ever
        // populates it on a `FROM` table source).
        if let Some(filter) = &self.filter {
            f.write_str(" WHERE ")?;
            filter.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for CopySource<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Table { table, columns, .. } => {
                table.render(ctx, f)?;
                if !columns.is_empty() {
                    f.write_str(" (")?;
                    render_ident_list(columns, ctx, f)?;
                    f.write_str(")")?;
                }
                Ok(())
            }
            Self::Query { query, .. } => {
                f.write_str("(")?;
                query.render(ctx, f)?;
                f.write_str(")")
            }
        }
    }
}

impl Render for CopyDirection {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::From => "FROM",
            Self::To => "TO",
        })
    }
}

impl Render for CopyTarget {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { path, .. } => path.render(ctx, f),
            Self::Stdin { .. } => f.write_str("STDIN"),
            Self::Stdout { .. } => f.write_str("STDOUT"),
            Self::Program { command, .. } => {
                f.write_str("PROGRAM ")?;
                command.render(ctx, f)
            }
        }
    }
}

impl Render for CopyOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        if let Some(value) = &self.value {
            f.write_str(" ")?;
            value.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for CopyOptionValue {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Word { word, .. } => word.render(ctx, f),
            Self::String { value, .. } => value.render(ctx, f),
            Self::Number { value, .. } => value.render(ctx, f),
            Self::Star { .. } => f.write_str("*"),
            Self::List { values, .. } => {
                f.write_str("(")?;
                render_comma_separated(values, ctx, f)?;
                f.write_str(")")
            }
            // The Snowflake `FILE_FORMAT = (TYPE = CSV ...)` nested list: space-separated
            // `key = value` pairs (the `= (...)` argument spelling), never PostgreSQL's
            // comma-separated `List` shape above.
            Self::OptionList { options, .. } => {
                f.write_str("(")?;
                render_copy_into_options(options, ctx, f)?;
                f.write_str(")")
            }
            Self::Force { kind, columns, .. } => {
                kind.render(ctx, f)?;
                f.write_str(" ")?;
                // An empty column list is the `*` (all-columns) form.
                if columns.is_empty() {
                    f.write_str("*")
                } else {
                    render_ident_list(columns, ctx, f)
                }
            }
        }
    }
}

impl Render for ForceKind {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Quote => "QUOTE",
            Self::Null => "NULL",
            Self::NotNull => "NOT NULL",
        })
    }
}

/// Render the Snowflake `COPY INTO` option list: space-separated `<name> = <value>`
/// pairs (the `KEY = VALUE` spelling), reused for both the top-level option list and
/// the nested [`CopyOptionValue::OptionList`] argument. A valueless option — which
/// Snowflake's grammar never emits, but the shared [`CopyOption`] type permits —
/// renders as the bare name.
fn render_copy_into_options(
    options: &[CopyOption],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    for (i, option) in options.iter().enumerate() {
        if i > 0 {
            f.write_str(" ")?;
        }
        option.name.render(ctx, f)?;
        if let Some(value) = &option.value {
            f.write_str(" = ")?;
            value.render(ctx, f)?;
        }
    }
    Ok(())
}

impl<X: Extension + Render> Render for CopyIntoStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("COPY INTO ")?;
        self.target.render(ctx, f)?;
        f.write_str(" FROM ")?;
        self.source.render(ctx, f)?;
        if !self.options.is_empty() {
            f.write_str(" ")?;
            render_copy_into_options(&self.options, ctx, f)?;
        }
        Ok(())
    }
}

impl Render for ExportStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EXPORT DATABASE ")?;
        // A named catalogue reconstructs the required `<db> TO` before the path; the bare
        // form renders just the path.
        if let Some(database) = &self.database {
            database.render(ctx, f)?;
            f.write_str(" TO ")?;
        }
        self.path.render(ctx, f)?;
        // The option trailer mirrors `COPY`'s two spellings, minus the leading `WITH` the
        // `EXPORT` grammar omits: the parenthesized form renders bare `(...)`, the legacy
        // form space-separated. An empty list renders nothing in either spelling.
        if !self.options.is_empty() {
            if self.parenthesized {
                f.write_str(" (")?;
                render_comma_separated(&self.options, ctx, f)?;
                f.write_str(")")?;
            } else {
                for option in &self.options {
                    f.write_str(" ")?;
                    option.render(ctx, f)?;
                }
            }
        }
        Ok(())
    }
}

impl Render for ImportStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IMPORT DATABASE ")?;
        self.path.render(ctx, f)
    }
}

impl Render for CopyIntoTarget {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Table { table, columns, .. } => {
                table.render(ctx, f)?;
                if !columns.is_empty() {
                    f.write_str(" (")?;
                    render_ident_list(columns, ctx, f)?;
                    f.write_str(")")?;
                }
                Ok(())
            }
            Self::External { location, .. } => location.render(ctx, f),
            Self::Stage { reference, .. } => reference.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for CopyIntoSource<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Table { table, .. } => table.render(ctx, f),
            Self::External { location, .. } => location.render(ctx, f),
            Self::Stage { reference, .. } => reference.render(ctx, f),
            Self::Query { query, .. } => {
                f.write_str("(")?;
                query.render(ctx, f)?;
                f.write_str(")")
            }
        }
    }
}

impl Render for ExplainKeyword {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Explain => "EXPLAIN",
            Self::Describe => "DESCRIBE",
            Self::Desc => "DESC",
        })
    }
}

impl<X: Extension + Render> Render for ExplainStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.spelling.render(ctx, f)?;
        if self.parenthesized {
            f.write_str(" (")?;
            render_comma_separated(&self.options, ctx, f)?;
            f.write_str(")")?;
        } else {
            // The legacy keyword prefix: each option is a bare space-separated word.
            for option in &self.options {
                f.write_str(" ")?;
                option.render(ctx, f)?;
            }
        }
        f.write_str(" ")?;
        self.statement.render(ctx, f)
    }
}

impl Render for ExplainOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Analyze { value, .. } => {
                render_explain_word_option("ANALYZE", value.as_ref(), ctx, f)
            }
            Self::Verbose { value, .. } => {
                render_explain_word_option("VERBOSE", value.as_ref(), ctx, f)
            }
            Self::Format { format, .. } => {
                f.write_str("FORMAT ")?;
                format.render(ctx, f)
            }
            Self::Other { name, value, .. } => {
                name.render(ctx, f)?;
                if let Some(value) = value {
                    f.write_str(" ")?;
                    value.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

/// Render a built-in `EXPLAIN` option keyword with its optional argument word.
fn render_explain_word_option(
    keyword: &str,
    value: Option<&Ident>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str(keyword)?;
    if let Some(value) = value {
        f.write_str(" ")?;
        value.render(ctx, f)?;
    }
    Ok(())
}

impl Render for ExplainFormat {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Text => "TEXT",
            Self::Xml => "XML",
            Self::Json => "JSON",
            Self::Yaml => "YAML",
        })
    }
}

impl Render for DescribeStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.keyword.render(ctx, f)?;
        f.write_str(" ")?;
        self.table.render(ctx, f)?;
        if let Some(column) = &self.column {
            f.write_str(" ")?;
            column.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for DescribeColumn {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { name, .. } => name.render(ctx, f),
            Self::Wild { pattern, .. } => pattern.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for ShowStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.target.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for ShowTarget<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tables {
                extended,
                full,
                all,
                from,
                filter,
                ..
            } => {
                f.write_str("SHOW ")?;
                if *extended {
                    f.write_str("EXTENDED ")?;
                }
                if *full {
                    f.write_str("FULL ")?;
                }
                if *all {
                    f.write_str("ALL ")?;
                }
                f.write_str("TABLES")?;
                if let Some(from) = from {
                    f.write_str(" ")?;
                    from.render(ctx, f)?;
                }
                if let Some(filter) = filter {
                    f.write_str(" ")?;
                    filter.render(ctx, f)?;
                }
                Ok(())
            }
            Self::Columns {
                extended,
                full,
                spelling,
                table,
                database,
                filter,
                ..
            } => {
                f.write_str("SHOW ")?;
                if *extended {
                    f.write_str("EXTENDED ")?;
                }
                if *full {
                    f.write_str("FULL ")?;
                }
                f.write_str(match spelling {
                    ShowColumnsSpelling::Columns => "COLUMNS ",
                    ShowColumnsSpelling::Fields => "FIELDS ",
                })?;
                table.render(ctx, f)?;
                if let Some(database) = database {
                    f.write_str(" ")?;
                    database.render(ctx, f)?;
                }
                if let Some(filter) = filter {
                    f.write_str(" ")?;
                    filter.render(ctx, f)?;
                }
                Ok(())
            }
            Self::Create {
                kind,
                name,
                if_not_exists,
                ..
            } => {
                f.write_str("SHOW CREATE ")?;
                f.write_str(match kind {
                    ShowCreateKind::Table => "TABLE ",
                    ShowCreateKind::View => "VIEW ",
                    ShowCreateKind::Database { schema: false } => "DATABASE ",
                    ShowCreateKind::Database { schema: true } => "SCHEMA ",
                    ShowCreateKind::Event => "EVENT ",
                    ShowCreateKind::Procedure => "PROCEDURE ",
                    ShowCreateKind::Function => "FUNCTION ",
                    ShowCreateKind::Trigger => "TRIGGER ",
                })?;
                if *if_not_exists {
                    f.write_str("IF NOT EXISTS ")?;
                }
                name.render(ctx, f)
            }
            Self::Functions {
                kind, from, filter, ..
            } => {
                f.write_str("SHOW ")?;
                if let Some(kind) = kind {
                    f.write_str(match kind {
                        ShowFunctionsScope::User => "USER ",
                        ShowFunctionsScope::System => "SYSTEM ",
                        ShowFunctionsScope::All => "ALL ",
                    })?;
                }
                f.write_str("FUNCTIONS")?;
                if let Some(from) = from {
                    f.write_str(" ")?;
                    from.render(ctx, f)?;
                }
                if let Some(filter) = filter {
                    f.write_str(" ")?;
                    filter.render(ctx, f)?;
                }
                Ok(())
            }
            Self::RoutineStatus { kind, filter, .. } => {
                f.write_str(match kind {
                    ShowRoutineKind::Function => "SHOW FUNCTION STATUS",
                    ShowRoutineKind::Procedure => "SHOW PROCEDURE STATUS",
                })?;
                if let Some(filter) = filter {
                    f.write_str(" ")?;
                    filter.render(ctx, f)?;
                }
                Ok(())
            }
            Self::Listing {
                kind, from, filter, ..
            } => {
                f.write_str("SHOW ")?;
                match kind {
                    ShowListing::Databases { schemas } => {
                        f.write_str(if *schemas { "SCHEMAS" } else { "DATABASES" })?;
                    }
                    ShowListing::CharacterSet { charset } => {
                        f.write_str(if *charset { "CHARSET" } else { "CHARACTER SET" })?;
                    }
                    ShowListing::Collation => f.write_str("COLLATION")?,
                    ShowListing::Status { scope } => {
                        render_show_scope(*scope, f)?;
                        f.write_str("STATUS")?;
                    }
                    ShowListing::Variables { scope } => {
                        render_show_scope(*scope, f)?;
                        f.write_str("VARIABLES")?;
                    }
                    ShowListing::Events => f.write_str("EVENTS")?,
                    ShowListing::TableStatus => f.write_str("TABLE STATUS")?,
                    ShowListing::OpenTables => f.write_str("OPEN TABLES")?,
                    ShowListing::Triggers { full } => {
                        if *full {
                            f.write_str("FULL ")?;
                        }
                        f.write_str("TRIGGERS")?;
                    }
                }
                if let Some(from) = from {
                    f.write_str(" ")?;
                    from.render(ctx, f)?;
                }
                if let Some(filter) = filter {
                    f.write_str(" ")?;
                    filter.render(ctx, f)?;
                }
                Ok(())
            }
            Self::Bare { kind, .. } => {
                f.write_str("SHOW ")?;
                f.write_str(match kind {
                    ShowBare::Plugins => "PLUGINS",
                    ShowBare::Engines { storage: true } => "STORAGE ENGINES",
                    ShowBare::Engines { storage: false } => "ENGINES",
                    ShowBare::Privileges => "PRIVILEGES",
                    ShowBare::Profiles => "PROFILES",
                    ShowBare::Processlist { full: true } => "FULL PROCESSLIST",
                    ShowBare::Processlist { full: false } => "PROCESSLIST",
                    ShowBare::BinaryLogs => "BINARY LOGS",
                    ShowBare::Replicas => "REPLICAS",
                    ShowBare::BinaryLogStatus => "BINARY LOG STATUS",
                })
            }
            Self::Index {
                spelling,
                extended,
                table,
                database,
                filter,
                ..
            } => {
                f.write_str("SHOW ")?;
                if *extended {
                    f.write_str("EXTENDED ")?;
                }
                f.write_str(match spelling {
                    ShowIndexSpelling::Index => "INDEX ",
                    ShowIndexSpelling::Indexes => "INDEXES ",
                    ShowIndexSpelling::Keys => "KEYS ",
                })?;
                table.render(ctx, f)?;
                if let Some(database) = database {
                    f.write_str(" ")?;
                    database.render(ctx, f)?;
                }
                if let Some(filter) = filter {
                    f.write_str(" ")?;
                    filter.render(ctx, f)?;
                }
                Ok(())
            }
            Self::Engine {
                engine, artifact, ..
            } => {
                f.write_str("SHOW ENGINE ")?;
                match engine {
                    Some(name) => name.render(ctx, f)?,
                    None => f.write_str("ALL")?,
                }
                f.write_str(match artifact {
                    ShowEngineArtifact::Status => " STATUS",
                    ShowEngineArtifact::Mutex => " MUTEX",
                    ShowEngineArtifact::Logs => " LOGS",
                })
            }
            Self::ReplicaStatus { channel, .. } => {
                f.write_str("SHOW REPLICA STATUS")?;
                if let Some(channel) = channel {
                    f.write_str(" FOR CHANNEL ")?;
                    channel.render(ctx, f)?;
                }
                Ok(())
            }
            Self::Diagnostics {
                kind, count, limit, ..
            } => {
                f.write_str("SHOW ")?;
                if *count {
                    f.write_str("COUNT(*) ")?;
                }
                f.write_str(match kind {
                    ShowDiagnosticKind::Warnings => "WARNINGS",
                    ShowDiagnosticKind::Errors => "ERRORS",
                })?;
                if let Some(limit) = limit {
                    f.write_str(" ")?;
                    render_show_limit(limit, ctx, f)?;
                }
                Ok(())
            }
            Self::RoutineCode { kind, name, .. } => {
                f.write_str(match kind {
                    ShowRoutineKind::Function => "SHOW FUNCTION CODE ",
                    ShowRoutineKind::Procedure => "SHOW PROCEDURE CODE ",
                })?;
                name.render(ctx, f)
            }
            Self::Grants {
                user, using_roles, ..
            } => {
                f.write_str("SHOW GRANTS")?;
                if let Some(user) = user {
                    f.write_str(" FOR ")?;
                    user.render(ctx, f)?;
                    if !using_roles.is_empty() {
                        f.write_str(" USING ")?;
                        render_comma_separated(using_roles, ctx, f)?;
                    }
                }
                Ok(())
            }
            Self::CreateUser { user, .. } => {
                f.write_str("SHOW CREATE USER ")?;
                user.render(ctx, f)
            }
            Self::Profile {
                types,
                query,
                limit,
                ..
            } => {
                f.write_str("SHOW PROFILE")?;
                for (index, ty) in types.iter().enumerate() {
                    f.write_str(if index == 0 { " " } else { ", " })?;
                    f.write_str(ty.keyword())?;
                }
                if let Some(query) = query {
                    f.write_str(" FOR QUERY ")?;
                    query.render(ctx, f)?;
                }
                if let Some(limit) = limit {
                    f.write_str(" ")?;
                    render_show_limit(limit, ctx, f)?;
                }
                Ok(())
            }
            Self::LogEvents {
                relay,
                log_name,
                position,
                limit,
                channel,
                ..
            } => {
                f.write_str(if *relay {
                    "SHOW RELAYLOG EVENTS"
                } else {
                    "SHOW BINLOG EVENTS"
                })?;
                if let Some(log_name) = log_name {
                    f.write_str(" IN ")?;
                    log_name.render(ctx, f)?;
                }
                if let Some(position) = position {
                    f.write_str(" FROM ")?;
                    position.render(ctx, f)?;
                }
                if let Some(limit) = limit {
                    f.write_str(" ")?;
                    render_show_limit(limit, ctx, f)?;
                }
                if let Some(channel) = channel {
                    f.write_str(" FOR CHANNEL ")?;
                    channel.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl ShowProfileType {
    /// The canonical keyword spelling of this profile type.
    fn keyword(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::BlockIo => "BLOCK IO",
            Self::ContextSwitches => "CONTEXT SWITCHES",
            Self::Cpu => "CPU",
            Self::Ipc => "IPC",
            Self::Memory => "MEMORY",
            Self::PageFaults => "PAGE FAULTS",
            Self::Source => "SOURCE",
            Self::Swaps => "SWAPS",
        }
    }
}

/// Render a [`ShowLimit`] — the shared `SHOW`-family `LIMIT` tail — as `LIMIT <row_count>`,
/// `LIMIT <offset>, <row_count>`, or `LIMIT <row_count> OFFSET <offset>`, matching the written
/// offset form. No surrounding whitespace; the caller writes the leading space.
fn render_show_limit(
    limit: &ShowLimit,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str("LIMIT ")?;
    match &limit.offset {
        Some(offset) if limit.offset_keyword => {
            limit.row_count.render(ctx, f)?;
            f.write_str(" OFFSET ")?;
            offset.render(ctx, f)
        }
        Some(offset) => {
            offset.render(ctx, f)?;
            f.write_str(", ")?;
            limit.row_count.render(ctx, f)
        }
        None => limit.row_count.render(ctx, f),
    }
}

/// Render an optional `GLOBAL`/`SESSION`/`LOCAL` scope keyword (trailing space included) for
/// the `SHOW … STATUS` / `SHOW … VARIABLES` listings; nothing when the scope is `None`.
fn render_show_scope(scope: Option<ShowScope>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if let Some(scope) = scope {
        f.write_str(match scope {
            ShowScope::Global => "GLOBAL ",
            ShowScope::Session => "SESSION ",
            ShowScope::Local => "LOCAL ",
        })?;
    }
    Ok(())
}

impl Render for ShowFunctionsFilter {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { like, name, .. } => {
                if *like {
                    f.write_str("LIKE ")?;
                }
                name.render(ctx, f)
            }
            Self::Regex { like, pattern, .. } => {
                if *like {
                    f.write_str("LIKE ")?;
                }
                pattern.render(ctx, f)
            }
        }
    }
}

impl Render for ShowFrom {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.keyword {
            ShowFromKeyword::From => "FROM ",
            ShowFromKeyword::In => "IN ",
        })?;
        self.name.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for ShowFilter<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Like { pattern, .. } => {
                f.write_str("LIKE ")?;
                pattern.render(ctx, f)
            }
            Self::Where { predicate, .. } => {
                f.write_str("WHERE ")?;
                predicate.render(ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for KillStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KILL")?;
        match self.target {
            KillTarget::Unspecified => {}
            KillTarget::Connection => f.write_str(" CONNECTION")?,
            KillTarget::Query => f.write_str(" QUERY")?,
        }
        f.write_str(" ")?;
        self.id.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for InstallStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallStatement::Plugin { name, soname, .. } => {
                f.write_str("INSTALL PLUGIN ")?;
                name.render(ctx, f)?;
                f.write_str(" SONAME ")?;
                soname.render(ctx, f)
            }
            InstallStatement::Component { urns, set, .. } => {
                f.write_str("INSTALL COMPONENT ")?;
                render_comma_separated(urns, ctx, f)?;
                if !set.is_empty() {
                    f.write_str(" SET ")?;
                    render_comma_separated(set, ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl Render for UninstallStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UninstallStatement::Plugin { name, .. } => {
                f.write_str("UNINSTALL PLUGIN ")?;
                name.render(ctx, f)
            }
            UninstallStatement::Component { urns, .. } => {
                f.write_str("UNINSTALL COMPONENT ")?;
                render_comma_separated(urns, ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for InstallComponentSetElement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.scope {
            None => {}
            Some(InstallComponentSetScope::Global) => f.write_str("GLOBAL ")?,
            Some(InstallComponentSetScope::Persist) => f.write_str("PERSIST ")?,
        }
        self.name.render(ctx, f)?;
        render_mysql_set_assignment(self.assignment, ctx, f)?;
        self.value.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for InstallComponentSetValue<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallComponentSetValue::On { .. } => f.write_str("ON"),
            InstallComponentSetValue::Expr { expr, .. } => expr.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for HandlerStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HANDLER ")?;
        self.table.render(ctx, f)?;
        f.write_str(" ")?;
        self.operation.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for HandlerOperation<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandlerOperation::Open {
                alias, as_keyword, ..
            } => {
                f.write_str("OPEN")?;
                if let Some(alias) = alias {
                    // `AS` is optional noise; the tag round-trips whichever was written.
                    f.write_str(if *as_keyword { " AS " } else { " " })?;
                    alias.render(ctx, f)?;
                }
                Ok(())
            }
            HandlerOperation::Close { .. } => f.write_str("CLOSE"),
            HandlerOperation::Read {
                selector,
                selection,
                limit,
                ..
            } => {
                f.write_str("READ ")?;
                selector.render(ctx, f)?;
                if let Some(selection) = selection {
                    f.write_str(" WHERE ")?;
                    selection.render(ctx, f)?;
                }
                if let Some(limit) = limit {
                    f.write_str(" ")?;
                    limit.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl<X: Extension + Render> Render for HandlerReadSelector<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandlerReadSelector::Scan { direction, .. } => f.write_str(match direction {
                HandlerScanDirection::First => "FIRST",
                HandlerScanDirection::Next => "NEXT",
            }),
            HandlerReadSelector::Index {
                index, direction, ..
            } => {
                index.render(ctx, f)?;
                f.write_str(match direction {
                    HandlerIndexDirection::First => " FIRST",
                    HandlerIndexDirection::Next => " NEXT",
                    HandlerIndexDirection::Prev => " PREV",
                    HandlerIndexDirection::Last => " LAST",
                })
            }
            HandlerReadSelector::Key {
                index,
                comparison,
                key,
                ..
            } => {
                index.render(ctx, f)?;
                f.write_str(match comparison {
                    HandlerKeyComparison::Eq => " = (",
                    HandlerKeyComparison::GreaterOrEqual => " >= (",
                    HandlerKeyComparison::LessOrEqual => " <= (",
                    HandlerKeyComparison::Greater => " > (",
                    HandlerKeyComparison::Less => " < (",
                })?;
                render_comma_separated(key, ctx, f)?;
                f.write_str(")")
            }
        }
    }
}

impl Render for CloneStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CloneStatement::Local { data_directory, .. } => {
                f.write_str("CLONE LOCAL ")?;
                data_directory.render(ctx, f)
            }
            CloneStatement::Instance {
                source,
                port,
                password,
                data_directory,
                ssl,
                ..
            } => {
                f.write_str("CLONE INSTANCE FROM ")?;
                source.render(ctx, f)?;
                f.write_str(":")?;
                port.render(ctx, f)?;
                f.write_str(" IDENTIFIED BY ")?;
                password.render(ctx, f)?;
                if let Some(data_directory) = data_directory {
                    f.write_str(" ")?;
                    data_directory.render(ctx, f)?;
                }
                match ssl {
                    CloneSsl::Unspecified => {}
                    CloneSsl::Require => f.write_str(" REQUIRE SSL")?,
                    CloneSsl::RequireNo => f.write_str(" REQUIRE NO SSL")?,
                }
                Ok(())
            }
        }
    }
}

impl Render for CloneDataDirectory {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.equals {
            "DATA DIRECTORY = "
        } else {
            "DATA DIRECTORY "
        })?;
        self.path.render(ctx, f)
    }
}

impl Render for ImportTableStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IMPORT TABLE FROM ")?;
        render_comma_separated(&self.files, ctx, f)
    }
}

impl Render for HelpStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HELP ")?;
        self.topic.render(ctx, f)
    }
}

impl Render for BinlogStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BINLOG ")?;
        self.event.render(ctx, f)
    }
}

impl Render for PragmaStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PRAGMA ")?;
        self.name.render(ctx, f)?;
        if let Some(value) = &self.value {
            // The `parenthesized` surface tag picks the spelling; SQLite writes no
            // space before the call form's `(`.
            if self.parenthesized {
                f.write_str("(")?;
                value.render(ctx, f)?;
                f.write_str(")")?;
            } else {
                f.write_str(" = ")?;
                value.render(ctx, f)?;
            }
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for AttachStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ATTACH ")?;
        if self.database_keyword {
            f.write_str("DATABASE ")?;
        }
        self.target.render(ctx, f)?;
        f.write_str(" AS ")?;
        self.schema.render(ctx, f)
    }
}

impl Render for DetachStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DETACH ")?;
        if self.database_keyword {
            f.write_str("DATABASE ")?;
        }
        if self.if_exists {
            f.write_str("IF EXISTS ")?;
        }
        self.schema.render(ctx, f)
    }
}

impl Render for CheckpointStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.force {
            f.write_str("FORCE ")?;
        }
        f.write_str("CHECKPOINT")?;
        if let Some(database) = &self.database {
            f.write_str(" ")?;
            database.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for LoadStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LOAD ")?;
        self.target.render(ctx, f)
    }
}

impl Render for LoadTarget {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { name, .. } => name.render(ctx, f),
            Self::Path { path, .. } => path.render(ctx, f),
        }
    }
}

impl Render for UseStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("USE ")?;
        self.name.render(ctx, f)
    }
}

impl Render for UpdateExtensionsStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UPDATE EXTENSIONS")?;
        // An empty list is the bare `UPDATE EXTENSIONS` (refresh all); a written list is
        // never empty (`UPDATE EXTENSIONS ()` is a DuckDB parser error), so a non-empty
        // vector renders the parenthesized form.
        if !self.extensions.is_empty() {
            f.write_str(" (")?;
            render_comma_separated(&self.extensions, ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for VacuumStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VACUUM")?;
        // DuckDB `ANALYZE` option precedes the operands; SQLite never sets it. The
        // parenthesized list canonicalizes to the single `(ANALYZE)` form (repeats carry
        // no meaning — see `VacuumAnalyze`).
        match self.analyze {
            Some(VacuumAnalyze::Keyword) => f.write_str(" ANALYZE")?,
            Some(VacuumAnalyze::Parenthesized) => f.write_str(" (ANALYZE)")?,
            None => {}
        }
        // The name operand: SQLite's single-ident schema or DuckDB's qualified table
        // (mutually exclusive by dialect).
        if let Some(schema) = &self.schema {
            f.write_str(" ")?;
            schema.render(ctx, f)?;
        }
        if let Some(table) = &self.table {
            f.write_str(" ")?;
            table.render(ctx, f)?;
        }
        // DuckDB column list, only alongside a table.
        if let Some(columns) = &self.columns {
            f.write_str(" (")?;
            render_comma_separated(columns, ctx, f)?;
            f.write_str(")")?;
        }
        // SQLite `INTO <expr>` tail.
        if let Some(into) = &self.into {
            f.write_str(" INTO ")?;
            into.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for ReindexStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("REINDEX")?;
        if let Some(target) = &self.target {
            f.write_str(" ")?;
            target.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for AnalyzeStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ANALYZE")?;
        if let Some(target) = &self.target {
            f.write_str(" ")?;
            target.render(ctx, f)?;
        }
        // DuckDB column list, only alongside a target.
        if let Some(columns) = &self.columns {
            f.write_str(" (")?;
            render_comma_separated(columns, ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl Render for TableMaintenanceStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Verb + the optional `NO_WRITE_TO_BINLOG | LOCAL` prefix (ANALYZE/OPTIMIZE/REPAIR).
        let (verb, prefix) = match &self.kind {
            TableMaintenanceKind::Analyze {
                no_write_to_binlog, ..
            } => ("ANALYZE", *no_write_to_binlog),
            TableMaintenanceKind::Check { .. } => ("CHECK", None),
            TableMaintenanceKind::Checksum { .. } => ("CHECKSUM", None),
            TableMaintenanceKind::Optimize {
                no_write_to_binlog, ..
            } => ("OPTIMIZE", *no_write_to_binlog),
            TableMaintenanceKind::Repair {
                no_write_to_binlog, ..
            } => ("REPAIR", *no_write_to_binlog),
        };
        f.write_str(verb)?;
        if let Some(prefix) = prefix {
            f.write_str(" ")?;
            f.write_str(no_write_to_binlog_keyword(prefix))?;
        }
        f.write_str(" ")?;
        f.write_str(table_keyword_str(self.table_keyword))?;
        f.write_str(" ")?;
        render_comma_separated(&self.tables, ctx, f)?;
        // The per-verb trailing options.
        match &self.kind {
            TableMaintenanceKind::Analyze {
                histogram: Some(histogram),
                ..
            } => {
                f.write_str(" ")?;
                histogram.render(ctx, f)?;
            }
            TableMaintenanceKind::Check { options, .. } => {
                for option in options {
                    f.write_str(" ")?;
                    f.write_str(check_table_option_keyword(*option))?;
                }
            }
            TableMaintenanceKind::Checksum {
                option: Some(option),
                ..
            } => {
                f.write_str(" ")?;
                f.write_str(checksum_table_option_keyword(*option))?;
            }
            TableMaintenanceKind::Repair { options, .. } => {
                for option in options {
                    f.write_str(" ")?;
                    f.write_str(repair_table_option_keyword(*option))?;
                }
            }
            TableMaintenanceKind::Analyze {
                histogram: None, ..
            }
            | TableMaintenanceKind::Checksum { option: None, .. }
            | TableMaintenanceKind::Optimize { .. } => {}
        }
        Ok(())
    }
}

impl Render for AnalyzeHistogram {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalyzeHistogram::Update {
                columns, buckets, ..
            } => {
                f.write_str("UPDATE HISTOGRAM ON ")?;
                render_comma_separated(columns, ctx, f)?;
                if let Some(buckets) = buckets {
                    f.write_str(" WITH ")?;
                    buckets.render(ctx, f)?;
                    f.write_str(" BUCKETS")?;
                }
            }
            AnalyzeHistogram::Drop { columns, .. } => {
                f.write_str("DROP HISTOGRAM ON ")?;
                render_comma_separated(columns, ctx, f)?;
            }
        }
        Ok(())
    }
}

impl Render for CacheIndexStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CACHE INDEX ")?;
        self.targets.render(ctx, f)?;
        f.write_str(" IN ")?;
        self.cache.render(ctx, f)
    }
}

impl Render for CacheIndexTargets {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheIndexTargets::Tables { tables, .. } => render_comma_separated(tables, ctx, f),
            CacheIndexTargets::Partition {
                table,
                partition,
                keys,
                ..
            } => {
                table.render(ctx, f)?;
                f.write_str(" ")?;
                partition.render(ctx, f)?;
                if let Some(keys) = keys {
                    f.write_str(" ")?;
                    keys.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl Render for CacheIndexTable {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.table.render(ctx, f)?;
        if let Some(keys) = &self.keys {
            f.write_str(" ")?;
            keys.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for LoadIndexStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LOAD INDEX INTO CACHE ")?;
        self.targets.render(ctx, f)
    }
}

impl Render for LoadIndexTargets {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadIndexTargets::Tables { tables, .. } => render_comma_separated(tables, ctx, f),
            LoadIndexTargets::Partition {
                table,
                partition,
                keys,
                ignore_leaves,
                ..
            } => {
                table.render(ctx, f)?;
                f.write_str(" ")?;
                partition.render(ctx, f)?;
                if let Some(keys) = keys {
                    f.write_str(" ")?;
                    keys.render(ctx, f)?;
                }
                if *ignore_leaves {
                    f.write_str(" IGNORE LEAVES")?;
                }
                Ok(())
            }
        }
    }
}

impl Render for LoadIndexTable {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.table.render(ctx, f)?;
        if let Some(keys) = &self.keys {
            f.write_str(" ")?;
            keys.render(ctx, f)?;
        }
        if self.ignore_leaves {
            f.write_str(" IGNORE LEAVES")?;
        }
        Ok(())
    }
}

impl Render for CacheIndexKeyList {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.keyword {
            CacheIndexKeyword::Index => "INDEX",
            CacheIndexKeyword::Key => "KEY",
        })?;
        f.write_str(" (")?;
        render_comma_separated(&self.keys, ctx, f)?;
        f.write_str(")")
    }
}

impl Render for KeyCacheName {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyCacheName::Named { name, .. } => name.render(ctx, f),
            KeyCacheName::Default { .. } => f.write_str("DEFAULT"),
        }
    }
}

impl Render for PartitionSelection {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartitionSelection::All { .. } => f.write_str("PARTITION (ALL)"),
            PartitionSelection::Names { names, .. } => {
                f.write_str("PARTITION (")?;
                render_comma_separated(names, ctx, f)?;
                f.write_str(")")
            }
        }
    }
}

impl Render for RenameStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenameStatement::Table {
                table_keyword,
                renames,
                ..
            } => {
                f.write_str("RENAME ")?;
                f.write_str(table_keyword_str(*table_keyword))?;
                f.write_str(" ")?;
                render_comma_separated(renames, ctx, f)
            }
            RenameStatement::User { renames, .. } => {
                f.write_str("RENAME USER ")?;
                render_comma_separated(renames, ctx, f)
            }
        }
    }
}

impl Render for TableRename {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.from.render(ctx, f)?;
        f.write_str(" TO ")?;
        self.to.render(ctx, f)
    }
}

impl Render for UserRename {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.from.render(ctx, f)?;
        f.write_str(" TO ")?;
        self.to.render(ctx, f)
    }
}

impl Render for FlushStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FLUSH")?;
        if let Some(prefix) = self.no_write_to_binlog {
            f.write_str(" ")?;
            f.write_str(no_write_to_binlog_keyword(prefix))?;
        }
        f.write_str(" ")?;
        self.target.render(ctx, f)
    }
}

impl Render for FlushTarget {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlushTarget::Tables {
                table_keyword,
                tables,
                lock,
                ..
            } => {
                f.write_str(table_keyword_str(*table_keyword))?;
                if !tables.is_empty() {
                    f.write_str(" ")?;
                    render_comma_separated(tables, ctx, f)?;
                }
                if let Some(lock) = lock {
                    f.write_str(" ")?;
                    f.write_str(match lock {
                        FlushTablesLock::WithReadLock => "WITH READ LOCK",
                        FlushTablesLock::ForExport => "FOR EXPORT",
                    })?;
                }
                Ok(())
            }
            FlushTarget::Options { options, .. } => render_comma_separated(options, ctx, f),
        }
    }
}

impl Render for FlushOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlushOption::Privileges { .. } => f.write_str("PRIVILEGES"),
            FlushOption::Logs { .. } => f.write_str("LOGS"),
            FlushOption::BinaryLogs { .. } => f.write_str("BINARY LOGS"),
            FlushOption::EngineLogs { .. } => f.write_str("ENGINE LOGS"),
            FlushOption::ErrorLogs { .. } => f.write_str("ERROR LOGS"),
            FlushOption::GeneralLogs { .. } => f.write_str("GENERAL LOGS"),
            FlushOption::SlowLogs { .. } => f.write_str("SLOW LOGS"),
            FlushOption::RelayLogs { channel, .. } => {
                f.write_str("RELAY LOGS")?;
                if let Some(channel) = channel {
                    f.write_str(" FOR CHANNEL ")?;
                    channel.render(ctx, f)?;
                }
                Ok(())
            }
            FlushOption::Status { .. } => f.write_str("STATUS"),
            FlushOption::UserResources { .. } => f.write_str("USER_RESOURCES"),
            FlushOption::OptimizerCosts { .. } => f.write_str("OPTIMIZER_COSTS"),
        }
    }
}

impl<X: Extension + Render> Render for PurgeStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PURGE BINARY LOGS ")?;
        self.target.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for PurgeTarget<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PurgeTarget::To { log, .. } => {
                f.write_str("TO ")?;
                log.render(ctx, f)
            }
            PurgeTarget::Before { datetime, .. } => {
                f.write_str("BEFORE ")?;
                datetime.render(ctx, f)
            }
        }
    }
}

/// Render a trailing `[FOR CHANNEL '<name>']` suffix, shared by the four replication verbs
/// that carry one.
fn render_for_channel(
    channel: &Option<Literal>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(channel) = channel {
        f.write_str(" FOR CHANNEL ")?;
        channel.render(ctx, f)?;
    }
    Ok(())
}

/// Render one fixed-position `START REPLICA` connection option (`keyword` includes its
/// leading space and trailing ` = `), if present.
fn render_replica_connection_option(
    keyword: &str,
    value: &Option<Literal>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(value) = value {
        f.write_str(keyword)?;
        value.render(ctx, f)?;
    }
    Ok(())
}

impl Render for ReplicationStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplicationStatement::ChangeSource {
                options, channel, ..
            } => {
                f.write_str("CHANGE REPLICATION SOURCE TO ")?;
                render_comma_separated(options, ctx, f)?;
                render_for_channel(channel, ctx, f)
            }
            ReplicationStatement::ChangeFilter { rules, channel, .. } => {
                f.write_str("CHANGE REPLICATION FILTER ")?;
                render_comma_separated(rules, ctx, f)?;
                render_for_channel(channel, ctx, f)
            }
            ReplicationStatement::StartReplica {
                threads,
                until,
                user,
                password,
                default_auth,
                plugin_dir,
                channel,
                ..
            } => {
                f.write_str("START REPLICA")?;
                if !threads.is_empty() {
                    f.write_str(" ")?;
                    render_comma_separated(threads, ctx, f)?;
                }
                if !until.is_empty() {
                    f.write_str(" UNTIL ")?;
                    render_comma_separated(until, ctx, f)?;
                }
                render_replica_connection_option(" USER = ", user, ctx, f)?;
                render_replica_connection_option(" PASSWORD = ", password, ctx, f)?;
                render_replica_connection_option(" DEFAULT_AUTH = ", default_auth, ctx, f)?;
                render_replica_connection_option(" PLUGIN_DIR = ", plugin_dir, ctx, f)?;
                render_for_channel(channel, ctx, f)
            }
            ReplicationStatement::StopReplica {
                threads, channel, ..
            } => {
                f.write_str("STOP REPLICA")?;
                if !threads.is_empty() {
                    f.write_str(" ")?;
                    render_comma_separated(threads, ctx, f)?;
                }
                render_for_channel(channel, ctx, f)
            }
            ReplicationStatement::StartGroupReplication { options, .. } => {
                f.write_str("START GROUP_REPLICATION")?;
                if !options.is_empty() {
                    f.write_str(" ")?;
                    render_comma_separated(options, ctx, f)?;
                }
                Ok(())
            }
            ReplicationStatement::StopGroupReplication { .. } => {
                f.write_str("STOP GROUP_REPLICATION")
            }
        }
    }
}

impl Render for ChangeReplicationSourceOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name.keyword())?;
        f.write_str(" = ")?;
        self.value.render(ctx, f)
    }
}

impl Render for ChangeReplicationSourceOptionValue {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String { value, .. } | Self::Number { value, .. } => value.render(ctx, f),
            Self::NullableString { value, .. } => match value {
                Some(value) => value.render(ctx, f),
                None => f.write_str("NULL"),
            },
            Self::User { account, .. } => match account {
                Some(account) => account.render(ctx, f),
                None => f.write_str("NULL"),
            },
            Self::ServerIds { ids, .. } => {
                f.write_str("(")?;
                render_comma_separated(ids, ctx, f)?;
                f.write_str(")")
            }
            Self::PrimaryKeyCheck { check, .. } => f.write_str(match check {
                RequirePrimaryKeyCheck::On => "ON",
                RequirePrimaryKeyCheck::Off => "OFF",
                RequirePrimaryKeyCheck::Stream => "STREAM",
                RequirePrimaryKeyCheck::Generate => "GENERATE",
            }),
            Self::AssignGtids { kind, uuid, .. } => match kind {
                AssignGtidsKind::Off => f.write_str("OFF"),
                AssignGtidsKind::Local => f.write_str("LOCAL"),
                AssignGtidsKind::Uuid => match uuid {
                    Some(uuid) => uuid.render(ctx, f),
                    None => Ok(()),
                },
            },
        }
    }
}

impl Render for ReplicationFilterRule {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DoDb { databases, .. } => {
                f.write_str("REPLICATE_DO_DB = (")?;
                render_comma_separated(databases, ctx, f)?;
                f.write_str(")")
            }
            Self::IgnoreDb { databases, .. } => {
                f.write_str("REPLICATE_IGNORE_DB = (")?;
                render_comma_separated(databases, ctx, f)?;
                f.write_str(")")
            }
            Self::DoTable { tables, .. } => {
                f.write_str("REPLICATE_DO_TABLE = (")?;
                render_comma_separated(tables, ctx, f)?;
                f.write_str(")")
            }
            Self::IgnoreTable { tables, .. } => {
                f.write_str("REPLICATE_IGNORE_TABLE = (")?;
                render_comma_separated(tables, ctx, f)?;
                f.write_str(")")
            }
            Self::WildDoTable { patterns, .. } => {
                f.write_str("REPLICATE_WILD_DO_TABLE = (")?;
                render_comma_separated(patterns, ctx, f)?;
                f.write_str(")")
            }
            Self::WildIgnoreTable { patterns, .. } => {
                f.write_str("REPLICATE_WILD_IGNORE_TABLE = (")?;
                render_comma_separated(patterns, ctx, f)?;
                f.write_str(")")
            }
            Self::RewriteDb { pairs, .. } => {
                f.write_str("REPLICATE_REWRITE_DB = (")?;
                render_comma_separated(pairs, ctx, f)?;
                f.write_str(")")
            }
        }
    }
}

impl Render for RewriteDbPair {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(")?;
        self.from.render(ctx, f)?;
        f.write_str(", ")?;
        self.to.render(ctx, f)?;
        f.write_str(")")
    }
}

impl Render for ReplicaThreadOption {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Sql { .. } => "SQL_THREAD",
            Self::Io {
                keyword: IoThreadKeyword::Io,
                ..
            } => "IO_THREAD",
            Self::Io {
                keyword: IoThreadKeyword::Relay,
                ..
            } => "RELAY_THREAD",
        })
    }
}

impl Render for ReplicaUntilCondition {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceLogFile { value, .. } => {
                f.write_str("SOURCE_LOG_FILE = ")?;
                value.render(ctx, f)
            }
            Self::SourceLogPos { value, .. } => {
                f.write_str("SOURCE_LOG_POS = ")?;
                value.render(ctx, f)
            }
            Self::RelayLogFile { value, .. } => {
                f.write_str("RELAY_LOG_FILE = ")?;
                value.render(ctx, f)
            }
            Self::RelayLogPos { value, .. } => {
                f.write_str("RELAY_LOG_POS = ")?;
                value.render(ctx, f)
            }
            Self::SqlBeforeGtids { value, .. } => {
                f.write_str("SQL_BEFORE_GTIDS = ")?;
                value.render(ctx, f)
            }
            Self::SqlAfterGtids { value, .. } => {
                f.write_str("SQL_AFTER_GTIDS = ")?;
                value.render(ctx, f)
            }
            Self::SqlAfterMtsGaps { .. } => f.write_str("SQL_AFTER_MTS_GAPS"),
        }
    }
}

impl Render for GroupReplicationOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User { value, .. } => {
                f.write_str("USER = ")?;
                value.render(ctx, f)
            }
            Self::Password { value, .. } => {
                f.write_str("PASSWORD = ")?;
                value.render(ctx, f)
            }
            Self::DefaultAuth { value, .. } => {
                f.write_str("DEFAULT_AUTH = ")?;
                value.render(ctx, f)
            }
        }
    }
}

impl Render for AccountName {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Account { user, host, .. } => {
                user.render(ctx, f)?;
                if let Some(host) = host {
                    f.write_str("@")?;
                    host.render(ctx, f)?;
                }
                Ok(())
            }
            Self::CurrentUser { parens, .. } => {
                f.write_str("CURRENT_USER")?;
                if *parens {
                    f.write_str("()")?;
                }
                Ok(())
            }
        }
    }
}

// --- User / role administration DDL render ---------------------------------------------

/// The shared `[REQUIRE …] [WITH <resource> …] [<lock option> …] [<attribute>]` option tail of
/// `CREATE USER` and `ALTER USER … <list>` — a borrowing view over the four clauses, grouped so
/// [`render_user_option_tail`] takes one data parameter rather than four.
struct UserOptionTail<'a> {
    require: &'a Option<TlsRequirement>,
    resource_options: &'a [ResourceLimit],
    password_lock_options: &'a [PasswordLockOption],
    attribute: &'a Option<UserAttribute>,
}

/// Render the shared option tail, each clause prefixed by one leading space so it abuts the
/// preceding account list.
fn render_user_option_tail(
    tail: UserOptionTail<'_>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(require) = tail.require {
        f.write_str(" ")?;
        require.render(ctx, f)?;
    }
    if !tail.resource_options.is_empty() {
        // `WITH` is written once, ahead of a whitespace-separated option run.
        f.write_str(" WITH")?;
        for option in tail.resource_options {
            f.write_str(" ")?;
            option.render(ctx, f)?;
        }
    }
    for option in tail.password_lock_options {
        f.write_str(" ")?;
        option.render(ctx, f)?;
    }
    if let Some(attribute) = tail.attribute {
        f.write_str(" ")?;
        attribute.render(ctx, f)?;
    }
    Ok(())
}

impl Render for CreateUser {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE USER ")?;
        if self.if_not_exists {
            f.write_str("IF NOT EXISTS ")?;
        }
        render_comma_separated(&self.users, ctx, f)?;
        if !self.default_roles.is_empty() {
            f.write_str(" DEFAULT ROLE ")?;
            render_comma_separated(&self.default_roles, ctx, f)?;
        }
        render_user_option_tail(
            UserOptionTail {
                require: &self.require,
                resource_options: &self.resource_options,
                password_lock_options: &self.password_lock_options,
                attribute: &self.attribute,
            },
            ctx,
            f,
        )
    }
}

impl Render for UserSpec {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.account.render(ctx, f)?;
        if let Some(auth) = &self.auth {
            f.write_str(" ")?;
            auth.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for AuthOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password { password, .. } => {
                f.write_str("IDENTIFIED BY ")?;
                password.render(ctx, f)
            }
            Self::RandomPassword { .. } => f.write_str("IDENTIFIED BY RANDOM PASSWORD"),
            Self::Plugin { plugin, .. } => {
                f.write_str("IDENTIFIED WITH ")?;
                plugin.render(ctx, f)
            }
            Self::PluginAs {
                plugin,
                auth_string,
                ..
            } => {
                f.write_str("IDENTIFIED WITH ")?;
                plugin.render(ctx, f)?;
                f.write_str(" AS ")?;
                auth_string.render(ctx, f)
            }
            Self::PluginByPassword {
                plugin, password, ..
            } => {
                f.write_str("IDENTIFIED WITH ")?;
                plugin.render(ctx, f)?;
                f.write_str(" BY ")?;
                password.render(ctx, f)
            }
            Self::PluginByRandomPassword { plugin, .. } => {
                f.write_str("IDENTIFIED WITH ")?;
                plugin.render(ctx, f)?;
                f.write_str(" BY RANDOM PASSWORD")
            }
        }
    }
}

impl Render for TlsRequirement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("REQUIRE ")?;
        match self {
            Self::None { .. } => f.write_str("NONE"),
            Self::Ssl { .. } => f.write_str("SSL"),
            Self::X509 { .. } => f.write_str("X509"),
            Self::Options { options, .. } => {
                for (i, option) in options.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" AND ")?;
                    }
                    option.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl Render for TlsOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Subject { value, .. } => {
                f.write_str("SUBJECT ")?;
                value.render(ctx, f)
            }
            Self::Issuer { value, .. } => {
                f.write_str("ISSUER ")?;
                value.render(ctx, f)
            }
            Self::Cipher { value, .. } => {
                f.write_str("CIPHER ")?;
                value.render(ctx, f)
            }
        }
    }
}

impl Render for ResourceLimit {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (keyword, value) = match self {
            Self::MaxQueriesPerHour { value, .. } => ("MAX_QUERIES_PER_HOUR", value),
            Self::MaxUpdatesPerHour { value, .. } => ("MAX_UPDATES_PER_HOUR", value),
            Self::MaxConnectionsPerHour { value, .. } => ("MAX_CONNECTIONS_PER_HOUR", value),
            Self::MaxUserConnections { value, .. } => ("MAX_USER_CONNECTIONS", value),
        };
        f.write_str(keyword)?;
        f.write_str(" ")?;
        value.render(ctx, f)
    }
}

impl Render for PasswordLockOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountLock { .. } => f.write_str("ACCOUNT LOCK"),
            Self::AccountUnlock { .. } => f.write_str("ACCOUNT UNLOCK"),
            Self::PasswordExpire { .. } => f.write_str("PASSWORD EXPIRE"),
            Self::PasswordExpireDefault { .. } => f.write_str("PASSWORD EXPIRE DEFAULT"),
            Self::PasswordExpireNever { .. } => f.write_str("PASSWORD EXPIRE NEVER"),
            Self::PasswordExpireInterval { days, .. } => {
                f.write_str("PASSWORD EXPIRE INTERVAL ")?;
                days.render(ctx, f)?;
                f.write_str(" DAY")
            }
            Self::PasswordHistory { count, .. } => {
                f.write_str("PASSWORD HISTORY ")?;
                count.render(ctx, f)
            }
            Self::PasswordHistoryDefault { .. } => f.write_str("PASSWORD HISTORY DEFAULT"),
            Self::PasswordReuseInterval { days, .. } => {
                f.write_str("PASSWORD REUSE INTERVAL ")?;
                days.render(ctx, f)?;
                f.write_str(" DAY")
            }
            Self::PasswordReuseIntervalDefault { .. } => {
                f.write_str("PASSWORD REUSE INTERVAL DEFAULT")
            }
            Self::PasswordRequireCurrent { .. } => f.write_str("PASSWORD REQUIRE CURRENT"),
            Self::PasswordRequireCurrentDefault { .. } => {
                f.write_str("PASSWORD REQUIRE CURRENT DEFAULT")
            }
            Self::PasswordRequireCurrentOptional { .. } => {
                f.write_str("PASSWORD REQUIRE CURRENT OPTIONAL")
            }
            Self::FailedLoginAttempts { count, .. } => {
                f.write_str("FAILED_LOGIN_ATTEMPTS ")?;
                count.render(ctx, f)
            }
            Self::PasswordLockTime { days, .. } => {
                f.write_str("PASSWORD_LOCK_TIME ")?;
                days.render(ctx, f)
            }
            Self::PasswordLockTimeUnbounded { .. } => f.write_str("PASSWORD_LOCK_TIME UNBOUNDED"),
        }
    }
}

impl Render for UserAttribute {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Comment { comment, .. } => {
                f.write_str("COMMENT ")?;
                comment.render(ctx, f)
            }
            Self::Attribute { attribute, .. } => {
                f.write_str("ATTRIBUTE ")?;
                attribute.render(ctx, f)
            }
        }
    }
}

impl Render for AlterUser {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER USER ")?;
        match self {
            Self::Modify {
                if_exists,
                users,
                require,
                resource_options,
                password_lock_options,
                attribute,
                ..
            } => {
                if *if_exists {
                    f.write_str("IF EXISTS ")?;
                }
                render_comma_separated(users, ctx, f)?;
                render_user_option_tail(
                    UserOptionTail {
                        require,
                        resource_options,
                        password_lock_options,
                        attribute,
                    },
                    ctx,
                    f,
                )
            }
            Self::DefaultRole {
                if_exists,
                user,
                roles,
                ..
            } => {
                if *if_exists {
                    f.write_str("IF EXISTS ")?;
                }
                user.render(ctx, f)?;
                f.write_str(" DEFAULT ROLE ")?;
                roles.render(ctx, f)
            }
        }
    }
}

impl Render for AlterUserSpec {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.account.render(ctx, f)?;
        if let Some(auth) = &self.auth {
            f.write_str(" ")?;
            auth.render(ctx, f)?;
        }
        if let Some(replace) = &self.replace {
            f.write_str(" REPLACE ")?;
            replace.render(ctx, f)?;
        }
        if self.retain_current_password {
            f.write_str(" RETAIN CURRENT PASSWORD")?;
        }
        if self.discard_old_password {
            f.write_str(" DISCARD OLD PASSWORD")?;
        }
        Ok(())
    }
}

impl Render for DefaultRoleTarget {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All { .. } => f.write_str("ALL"),
            Self::None { .. } => f.write_str("NONE"),
            Self::Roles { roles, .. } => render_comma_separated(roles, ctx, f),
        }
    }
}

impl Render for UserRoleList {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (verb, guard) = match self.kind {
            UserRoleListKind::DropUser => ("DROP USER ", "IF EXISTS "),
            UserRoleListKind::CreateRole => ("CREATE ROLE ", "IF NOT EXISTS "),
            UserRoleListKind::DropRole => ("DROP ROLE ", "IF EXISTS "),
        };
        f.write_str(verb)?;
        if self.if_guard {
            f.write_str(guard)?;
        }
        render_comma_separated(&self.names, ctx, f)
    }
}

/// The surface keyword for a [`NoWriteToBinlog`] binlog-suppression prefix.
fn no_write_to_binlog_keyword(prefix: NoWriteToBinlog) -> &'static str {
    match prefix {
        NoWriteToBinlog::NoWriteToBinlog => "NO_WRITE_TO_BINLOG",
        NoWriteToBinlog::Local => "LOCAL",
    }
}

/// The surface keyword for a [`TableKeyword`] (`TABLE`/`TABLES`).
fn table_keyword_str(keyword: TableKeyword) -> &'static str {
    match keyword {
        TableKeyword::Table => "TABLE",
        TableKeyword::Tables => "TABLES",
    }
}

/// The surface keyword(s) for a [`CheckTableOption`].
fn check_table_option_keyword(option: CheckTableOption) -> &'static str {
    match option {
        CheckTableOption::ForUpgrade => "FOR UPGRADE",
        CheckTableOption::Quick => "QUICK",
        CheckTableOption::Fast => "FAST",
        CheckTableOption::Medium => "MEDIUM",
        CheckTableOption::Extended => "EXTENDED",
        CheckTableOption::Changed => "CHANGED",
    }
}

/// The surface keyword for a [`ChecksumTableOption`].
fn checksum_table_option_keyword(option: ChecksumTableOption) -> &'static str {
    match option {
        ChecksumTableOption::Quick => "QUICK",
        ChecksumTableOption::Extended => "EXTENDED",
    }
}

/// The surface keyword for a [`RepairTableOption`].
fn repair_table_option_keyword(option: RepairTableOption) -> &'static str {
    match option {
        RepairTableOption::Quick => "QUICK",
        RepairTableOption::Extended => "EXTENDED",
        RepairTableOption::UseFrm => "USE_FRM",
    }
}

impl<X: Extension + Render> Render for PrepareStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PREPARE ")?;
        self.name.render(ctx, f)?;
        // An empty `parameter_types` is the bare `PREPARE name AS ...` form (no list);
        // PostgreSQL rejects an empty written `()`, so a non-empty list unambiguously
        // means the parentheses were written.
        if !self.parameter_types.is_empty() {
            f.write_str("(")?;
            render_comma_separated(&self.parameter_types, ctx, f)?;
            f.write_str(")")?;
        }
        f.write_str(" AS ")?;
        self.statement.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for ExecuteStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EXECUTE ")?;
        self.name.render(ctx, f)?;
        // An empty `args` is the bare `EXECUTE v1` form (no list); a non-empty one renders
        // the parenthesized arguments. An empty written `()` is never built (parser-rejected).
        if !self.args.is_empty() {
            f.write_str("(")?;
            render_comma_separated(&self.args, ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl Render for DeallocateStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The leading verb round-trips MySQL's `deallocate_or_drop` synonym choice; DuckDB
        // only ever spells `DEALLOCATE`.
        f.write_str(match self.keyword {
            DeallocateKeyword::Deallocate => "DEALLOCATE ",
            DeallocateKeyword::Drop => "DROP ",
        })?;
        if self.prepare_keyword {
            f.write_str("PREPARE ")?;
        }
        self.name.render(ctx, f)
    }
}

impl Render for PrepareFromStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PREPARE ")?;
        self.name.render(ctx, f)?;
        f.write_str(" FROM ")?;
        self.source.render(ctx, f)
    }
}

impl Render for PrepareSource {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The string source round-trips verbatim from the `Literal`.
            PrepareSource::Text { source, .. } => source.render(ctx, f),
            // A `@variable` source: the `@` sigil plus the name, whose quote style the `Ident`
            // carries.
            PrepareSource::Variable { name, .. } => {
                f.write_str("@")?;
                name.render(ctx, f)
            }
        }
    }
}

impl Render for ExecuteUsingStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EXECUTE ")?;
        self.name.render(ctx, f)?;
        // A non-empty list renders the `USING` clause; an empty one is the bare `EXECUTE name`
        // form (no `USING` written — MySQL has no empty-`USING` spelling).
        if let Some((first, rest)) = self.using.split_first() {
            f.write_str(" USING @")?;
            first.render(ctx, f)?;
            for name in rest {
                f.write_str(", @")?;
                name.render(ctx, f)?;
            }
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for CallStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CALL ")?;
        self.name.render(ctx, f)?;
        // The parenthesized argument list is a surface flag: the DuckDB form always writes
        // it (empty or not), MySQL's bare `CALL name` writes no list at all.
        if self.parenthesized {
            f.write_str("(")?;
            render_comma_separated(&self.args, ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl Render for DoStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DO")?;
        // The list is always non-empty (the parser rejects a bare `DO`); each item follows a
        // single space, and the source order round-trips (a body and a language clause can
        // appear in either order, and either may repeat).
        for arg in &self.args {
            f.write_str(" ")?;
            arg.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for DoArg {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DoArg::Body { body, .. } => body.render(ctx, f),
            DoArg::Language { name, .. } => {
                f.write_str("LANGUAGE ")?;
                name.render(ctx, f)
            }
        }
    }
}

impl Render for LanguageName {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Word { word, .. } => word.render(ctx, f),
            Self::String { value, .. } => value.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for DoExpressionsStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The list is always non-empty (the parser rejects a bare `DO`).
        f.write_str("DO ")?;
        render_comma_separated(&self.items, ctx, f)
    }
}

impl Render for LockTablesStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `plural` preserves the interchangeable `TABLES`/`TABLE` source spelling; the
        // list is always non-empty (the parser rejects a bare `LOCK TABLES`).
        f.write_str(if self.plural {
            "LOCK TABLES "
        } else {
            "LOCK TABLE "
        })?;
        render_comma_separated(&self.tables, ctx, f)
    }
}

impl Render for TableLock {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        if let Some(alias) = &self.alias {
            // The canonical `AS`-less spelling: MySQL's `opt_as` makes the keyword pure
            // noise the AST does not record.
            f.write_str(" ")?;
            alias.render(ctx, f)?;
        }
        f.write_str(" ")?;
        self.kind.render(ctx, f)
    }
}

impl Render for TableLockKind {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TableLockKind::Read => "READ",
            TableLockKind::ReadLocal => "READ LOCAL",
            TableLockKind::Write => "WRITE",
        })
    }
}

impl Render for UnlockTablesStatement {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.plural {
            "UNLOCK TABLES"
        } else {
            "UNLOCK TABLE"
        })
    }
}

impl Render for InstanceLockStatement {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.acquire {
            "LOCK INSTANCE FOR BACKUP"
        } else {
            "UNLOCK INSTANCE"
        })
    }
}

impl<X: Extension + Render> Render for LoadDataStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The grammar's canonical clause order — the parse layer is order-sensitive, so the
        // rendered order is the only round-trippable one.
        f.write_str("LOAD ")?;
        self.format.render(ctx, f)?;
        if let Some(concurrency) = self.concurrency {
            f.write_str(" ")?;
            concurrency.render(ctx, f)?;
        }
        if self.local {
            f.write_str(" LOCAL")?;
        }
        f.write_str(" INFILE ")?;
        self.file.render(ctx, f)?;
        if let Some(on_duplicate) = self.on_duplicate {
            f.write_str(" ")?;
            on_duplicate.render(ctx, f)?;
        }
        f.write_str(" INTO TABLE ")?;
        self.table.render(ctx, f)?;
        if !self.partitions.is_empty() {
            f.write_str(" PARTITION (")?;
            render_comma_separated(&self.partitions, ctx, f)?;
            f.write_str(")")?;
        }
        if let Some(charset) = &self.charset {
            f.write_str(" CHARACTER SET ")?;
            charset.render(ctx, f)?;
        }
        if let Some(tag) = &self.rows_identified_by {
            f.write_str(" ROWS IDENTIFIED BY ")?;
            tag.render(ctx, f)?;
        }
        if let Some(fields) = &self.fields {
            f.write_str(" ")?;
            fields.render(ctx, f)?;
        }
        if let Some(lines) = &self.lines {
            f.write_str(" ")?;
            lines.render(ctx, f)?;
        }
        if let Some(ignore_rows) = &self.ignore_rows {
            f.write_str(" ")?;
            ignore_rows.render(ctx, f)?;
        }
        if !self.columns.is_empty() {
            f.write_str(" (")?;
            render_comma_separated(&self.columns, ctx, f)?;
            f.write_str(")")?;
        }
        if !self.set.is_empty() {
            f.write_str(" SET ")?;
            render_comma_separated(&self.set, ctx, f)?;
        }
        Ok(())
    }
}

impl Render for LoadDataFormat {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LoadDataFormat::Data => "DATA",
            LoadDataFormat::Xml => "XML",
        })
    }
}

impl Render for LoadDataConcurrency {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LoadDataConcurrency::LowPriority => "LOW_PRIORITY",
            LoadDataConcurrency::Concurrent => "CONCURRENT",
        })
    }
}

impl Render for LoadDataDuplicate {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LoadDataDuplicate::Replace => "REPLACE",
            LoadDataDuplicate::Ignore => "IGNORE",
        })
    }
}

impl Render for LoadDataFields {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The sub-clauses render in the grammar's canonical order regardless of source order
        // (any order re-parses to the same node); at least one is always present.
        self.spelling.render(ctx, f)?;
        if let Some(terminated_by) = &self.terminated_by {
            f.write_str(" TERMINATED BY ")?;
            terminated_by.render(ctx, f)?;
        }
        if let Some(enclosed_by) = &self.enclosed_by {
            f.write_str(" ")?;
            enclosed_by.render(ctx, f)?;
        }
        if let Some(escaped_by) = &self.escaped_by {
            f.write_str(" ESCAPED BY ")?;
            escaped_by.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for LoadFieldsSpelling {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LoadFieldsSpelling::Fields => "FIELDS",
            LoadFieldsSpelling::Columns => "COLUMNS",
        })
    }
}

impl Render for LoadDataEnclosed {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.optionally {
            f.write_str("OPTIONALLY ")?;
        }
        f.write_str("ENCLOSED BY ")?;
        self.value.render(ctx, f)
    }
}

impl Render for LoadDataLines {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LINES")?;
        if let Some(starting_by) = &self.starting_by {
            f.write_str(" STARTING BY ")?;
            starting_by.render(ctx, f)?;
        }
        if let Some(terminated_by) = &self.terminated_by {
            f.write_str(" TERMINATED BY ")?;
            terminated_by.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for LoadDataIgnoreRows {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IGNORE ")?;
        self.count.render(ctx, f)?;
        f.write_str(" ")?;
        self.unit.render(ctx, f)
    }
}

impl Render for LoadDataIgnoreUnit {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LoadDataIgnoreUnit::Lines => "LINES",
            LoadDataIgnoreUnit::Rows => "ROWS",
        })
    }
}

impl Render for LoadDataFieldOrVar {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadDataFieldOrVar::Column { name, .. } => name.render(ctx, f),
            // A `@variable` target: the `@` sigil plus the name (whose quote style the `Ident`
            // carries).
            LoadDataFieldOrVar::Variable { name, .. } => {
                f.write_str("@")?;
                name.render(ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for CreateTrigger<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE")?;
        if let Some(temporary) = self.temporary {
            f.write_str(" ")?;
            temporary.render(ctx, f)?;
        }
        f.write_str(" TRIGGER")?;
        if self.if_not_exists {
            f.write_str(" IF NOT EXISTS")?;
        }
        f.write_str(" ")?;
        self.name.render(ctx, f)?;
        if let Some(timing) = self.timing {
            f.write_str(" ")?;
            timing.render(ctx, f)?;
        }
        f.write_str(" ")?;
        self.event.render(ctx, f)?;
        f.write_str(" ON ")?;
        self.table.render(ctx, f)?;
        if self.for_each_row {
            f.write_str(" FOR EACH ROW")?;
        }
        if let Some(when) = &self.when {
            f.write_str(" WHEN ")?;
            when.render(ctx, f)?;
        }
        f.write_str(" BEGIN ")?;
        for statement in &self.body {
            statement.render(ctx, f)?;
            f.write_str("; ")?;
        }
        f.write_str("END")
    }
}

impl Render for TriggerTiming {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TriggerTiming::Before => "BEFORE",
            TriggerTiming::After => "AFTER",
            TriggerTiming::InsteadOf => "INSTEAD OF",
        })
    }
}

impl Render for TriggerEvent {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriggerEvent::Delete { .. } => f.write_str("DELETE"),
            TriggerEvent::Insert { .. } => f.write_str("INSERT"),
            TriggerEvent::Update { columns, .. } => {
                f.write_str("UPDATE")?;
                if !columns.is_empty() {
                    f.write_str(" OF ")?;
                    render_ident_list(columns, ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl<X: Extension + Render> Render for CreateStoredTrigger<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE ")?;
        if let Some(definer) = &self.definer {
            definer.render(ctx, f)?;
            f.write_str(" ")?;
        }
        f.write_str("TRIGGER ")?;
        if self.if_not_exists {
            f.write_str("IF NOT EXISTS ")?;
        }
        self.name.render(ctx, f)?;
        f.write_str(" ")?;
        self.timing.render(ctx, f)?;
        f.write_str(" ")?;
        self.event.render(ctx, f)?;
        f.write_str(" ON ")?;
        self.table.render(ctx, f)?;
        f.write_str(" FOR EACH ROW")?;
        if let Some(ordering) = &self.ordering {
            f.write_str(" ")?;
            ordering.render(ctx, f)?;
        }
        f.write_str(" ")?;
        self.body.render(ctx, f)
    }
}

impl Render for TriggerOrder {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TriggerOrder::Follows { anchor, .. } => {
                f.write_str("FOLLOWS ")?;
                anchor.render(ctx, f)
            }
            TriggerOrder::Precedes { anchor, .. } => {
                f.write_str("PRECEDES ")?;
                anchor.render(ctx, f)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Stored-program compound statements (MySQL SQL/PSM)
// ---------------------------------------------------------------------------

/// Render a `;`-terminated compound-body statement list, each element prefixed by a
/// single leading space (` <stmt>;`) — the shared shape of every block, branch, and
/// loop body, mirroring the trigger body's `<stmt>; ` join.
fn render_compound_body<X: Extension + Render>(
    body: &[Statement<X>],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    for statement in body {
        f.write_str(" ")?;
        statement.render(ctx, f)?;
        f.write_str(";")?;
    }
    Ok(())
}

impl<X: Extension + Render> Render for CompoundStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(label) = &self.label {
            label.render(ctx, f)?;
            f.write_str(": ")?;
        }
        f.write_str("BEGIN")?;
        for declaration in &self.declarations {
            f.write_str(" ")?;
            declaration.render(ctx, f)?;
            f.write_str(";")?;
        }
        render_compound_body(&self.body, ctx, f)?;
        f.write_str(" END")?;
        if let Some(end_label) = &self.end_label {
            f.write_str(" ")?;
            end_label.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for Declaration<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Declaration::Variable {
                names,
                data_type,
                default,
                ..
            } => {
                f.write_str("DECLARE ")?;
                render_ident_list(names, ctx, f)?;
                f.write_str(" ")?;
                data_type.render(ctx, f)?;
                if let Some(default) = default {
                    f.write_str(" DEFAULT ")?;
                    default.render(ctx, f)?;
                }
                Ok(())
            }
            Declaration::Condition { name, value, .. } => {
                f.write_str("DECLARE ")?;
                name.render(ctx, f)?;
                f.write_str(" CONDITION FOR ")?;
                value.render(ctx, f)
            }
            Declaration::Cursor { name, query, .. } => {
                f.write_str("DECLARE ")?;
                name.render(ctx, f)?;
                f.write_str(" CURSOR FOR ")?;
                query.render(ctx, f)
            }
            Declaration::Handler {
                action,
                conditions,
                body,
                ..
            } => {
                f.write_str("DECLARE ")?;
                action.render(ctx, f)?;
                f.write_str(" HANDLER FOR ")?;
                render_comma_separated(conditions, ctx, f)?;
                f.write_str(" ")?;
                body.render(ctx, f)
            }
        }
    }
}

impl Render for HandlerAction {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            HandlerAction::Continue => "CONTINUE",
            HandlerAction::Exit => "EXIT",
            HandlerAction::Undo => "UNDO",
        })
    }
}

impl Render for ConditionValue {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConditionValue::SqlState {
                value_keyword,
                sqlstate,
                ..
            } => {
                f.write_str("SQLSTATE ")?;
                if *value_keyword {
                    f.write_str("VALUE ")?;
                }
                sqlstate.render(ctx, f)
            }
            ConditionValue::ErrorCode { code, .. } => code.render(ctx, f),
            ConditionValue::ConditionName { name, .. } => name.render(ctx, f),
        }
    }
}

impl Render for HandlerCondition {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandlerCondition::SqlState {
                value_keyword,
                sqlstate,
                ..
            } => {
                f.write_str("SQLSTATE ")?;
                if *value_keyword {
                    f.write_str("VALUE ")?;
                }
                sqlstate.render(ctx, f)
            }
            HandlerCondition::ErrorCode { code, .. } => code.render(ctx, f),
            HandlerCondition::ConditionName { name, .. } => name.render(ctx, f),
            HandlerCondition::SqlWarning { .. } => f.write_str("SQLWARNING"),
            HandlerCondition::NotFound { .. } => f.write_str("NOT FOUND"),
            HandlerCondition::SqlException { .. } => f.write_str("SQLEXCEPTION"),
        }
    }
}

impl<X: Extension + Render> Render for ConditionalBranch<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.guard.render(ctx, f)?;
        f.write_str(" THEN")?;
        render_compound_body(&self.body, ctx, f)
    }
}

impl<X: Extension + Render> Render for IfStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IF ")?;
        for (i, branch) in self.branches.iter().enumerate() {
            if i > 0 {
                f.write_str(" ELSEIF ")?;
            }
            branch.render(ctx, f)?;
        }
        if let Some(else_body) = &self.else_body {
            f.write_str(" ELSE")?;
            render_compound_body(else_body, ctx, f)?;
        }
        f.write_str(" END IF")
    }
}

impl<X: Extension + Render> Render for CaseStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CASE")?;
        if let Some(operand) = &self.operand {
            f.write_str(" ")?;
            operand.render(ctx, f)?;
        }
        for branch in &self.when_branches {
            f.write_str(" WHEN ")?;
            branch.render(ctx, f)?;
        }
        if let Some(else_body) = &self.else_body {
            f.write_str(" ELSE")?;
            render_compound_body(else_body, ctx, f)?;
        }
        f.write_str(" END CASE")
    }
}

impl<X: Extension + Render> Render for LoopStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(label) = &self.label {
            label.render(ctx, f)?;
            f.write_str(": ")?;
        }
        f.write_str("LOOP")?;
        render_compound_body(&self.body, ctx, f)?;
        f.write_str(" END LOOP")?;
        if let Some(end_label) = &self.end_label {
            f.write_str(" ")?;
            end_label.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for WhileStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(label) = &self.label {
            label.render(ctx, f)?;
            f.write_str(": ")?;
        }
        f.write_str("WHILE ")?;
        self.condition.render(ctx, f)?;
        f.write_str(" DO")?;
        render_compound_body(&self.body, ctx, f)?;
        f.write_str(" END WHILE")?;
        if let Some(end_label) = &self.end_label {
            f.write_str(" ")?;
            end_label.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for RepeatStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(label) = &self.label {
            label.render(ctx, f)?;
            f.write_str(": ")?;
        }
        f.write_str("REPEAT")?;
        render_compound_body(&self.body, ctx, f)?;
        f.write_str(" UNTIL ")?;
        self.condition.render(ctx, f)?;
        f.write_str(" END REPEAT")?;
        if let Some(end_label) = &self.end_label {
            f.write_str(" ")?;
            end_label.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for LeaveStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LEAVE ")?;
        self.label.render(ctx, f)
    }
}

impl Render for IterateStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ITERATE ")?;
        self.label.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for ReturnStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RETURN ")?;
        self.value.render(ctx, f)
    }
}

impl Render for OpenCursorStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OPEN ")?;
        self.cursor.render(ctx, f)
    }
}

impl Render for CloseCursorStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CLOSE ")?;
        self.cursor.render(ctx, f)
    }
}

impl Render for FetchCursorStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FETCH ")?;
        if self.next_keyword {
            f.write_str("NEXT ")?;
        }
        if self.from_keyword {
            f.write_str("FROM ")?;
        }
        self.cursor.render(ctx, f)?;
        f.write_str(" INTO ")?;
        render_ident_list(&self.targets, ctx, f)
    }
}

impl<X: Extension + Render> SignalStatement<X> {
    /// Render as `SIGNAL`/`RESIGNAL` — the two share this payload and differ only in the
    /// leading keyword the caller supplies.
    fn render_as(
        &self,
        ctx: &RenderCtx<'_>,
        f: &mut fmt::Formatter<'_>,
        keyword: &str,
    ) -> fmt::Result {
        f.write_str(keyword)?;
        if let Some(condition) = &self.condition {
            f.write_str(" ")?;
            condition.render(ctx, f)?;
        }
        if !self.set_items.is_empty() {
            f.write_str(" SET ")?;
            render_comma_separated(&self.set_items, ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for SignalItem<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        f.write_str(" = ")?;
        self.value.render(ctx, f)
    }
}

impl Render for SignalItemName {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SignalItemName::ClassOrigin => "CLASS_ORIGIN",
            SignalItemName::SubclassOrigin => "SUBCLASS_ORIGIN",
            SignalItemName::ConstraintCatalog => "CONSTRAINT_CATALOG",
            SignalItemName::ConstraintSchema => "CONSTRAINT_SCHEMA",
            SignalItemName::ConstraintName => "CONSTRAINT_NAME",
            SignalItemName::CatalogName => "CATALOG_NAME",
            SignalItemName::SchemaName => "SCHEMA_NAME",
            SignalItemName::TableName => "TABLE_NAME",
            SignalItemName::ColumnName => "COLUMN_NAME",
            SignalItemName::CursorName => "CURSOR_NAME",
            SignalItemName::MessageText => "MESSAGE_TEXT",
            SignalItemName::MysqlErrno => "MYSQL_ERRNO",
        })
    }
}

impl<X: Extension + Render> Render for GetDiagnosticsStatement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GET")?;
        match self.area {
            DiagnosticsArea::Implicit => {}
            DiagnosticsArea::Current => f.write_str(" CURRENT")?,
            DiagnosticsArea::Stacked => f.write_str(" STACKED")?,
        }
        f.write_str(" DIAGNOSTICS ")?;
        self.info.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for DiagnosticsInfo<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticsInfo::Statement { items, .. } => render_comma_separated(items, ctx, f),
            DiagnosticsInfo::Condition { number, items, .. } => {
                f.write_str("CONDITION ")?;
                number.render(ctx, f)?;
                f.write_str(" ")?;
                render_comma_separated(items, ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for StatementInfoItem<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.target.render(ctx, f)?;
        f.write_str(" = ")?;
        self.name.render(ctx, f)
    }
}

impl Render for StatementInfoItemName {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            StatementInfoItemName::Number => "NUMBER",
            StatementInfoItemName::RowCount => "ROW_COUNT",
        })
    }
}

impl<X: Extension + Render> Render for ConditionInfoItem<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.target.render(ctx, f)?;
        f.write_str(" = ")?;
        self.name.render(ctx, f)
    }
}

impl Render for ConditionInfoItemName {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ConditionInfoItemName::ClassOrigin => "CLASS_ORIGIN",
            ConditionInfoItemName::SubclassOrigin => "SUBCLASS_ORIGIN",
            ConditionInfoItemName::ConstraintCatalog => "CONSTRAINT_CATALOG",
            ConditionInfoItemName::ConstraintSchema => "CONSTRAINT_SCHEMA",
            ConditionInfoItemName::ConstraintName => "CONSTRAINT_NAME",
            ConditionInfoItemName::CatalogName => "CATALOG_NAME",
            ConditionInfoItemName::SchemaName => "SCHEMA_NAME",
            ConditionInfoItemName::TableName => "TABLE_NAME",
            ConditionInfoItemName::ColumnName => "COLUMN_NAME",
            ConditionInfoItemName::CursorName => "CURSOR_NAME",
            ConditionInfoItemName::MessageText => "MESSAGE_TEXT",
            ConditionInfoItemName::MysqlErrno => "MYSQL_ERRNO",
            ConditionInfoItemName::ReturnedSqlstate => "RETURNED_SQLSTATE",
        })
    }
}

impl<X: Extension + Render> Render for CreateTable<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE")?;
        if self.or_replace {
            f.write_str(" OR REPLACE")?;
        }
        if let Some(temporary) = self.temporary {
            f.write_str(" ")?;
            temporary.render(ctx, f)?;
        }
        // `UNLOGGED` is a peer of `TEMP`/`TEMPORARY` in PostgreSQL's `OptTemp`, so it never
        // co-occurs with `temporary` — exactly one persistence keyword is written.
        if self.unlogged {
            f.write_str(" UNLOGGED")?;
        }
        f.write_str(" TABLE")?;
        if self.if_not_exists {
            f.write_str(" IF NOT EXISTS")?;
        }
        f.write_str(" ")?;
        self.name.render(ctx, f)?;
        match &self.body {
            // PostgreSQL grammar order after the body: `INHERITS (…)`, then `PARTITION BY …`,
            // then `USING <method>`, then the trailing options. `inherits` is only ever non-empty
            // on a `Definition` body (never `PartitionOf`/`OfType`), and an `AS <query>` body
            // carries none of these, so they are unrendered there.
            CreateTableBody::Definition { .. }
            | CreateTableBody::PartitionOf { .. }
            | CreateTableBody::OfType { .. }
            // MySQL's `LIKE <source>` clone body carries no trailing clause of its own (the
            // grammar rejects options after it), so `inherits`/`partition_by`/`access_method`/
            // `options` are empty/`None` and their renders are no-ops here.
            | CreateTableBody::LikeSource { .. } => {
                self.body.render(ctx, f)?;
                render_inherits(&self.inherits, ctx, f)?;
                render_partition_by(&self.partition_by, ctx, f)?;
                render_access_method(&self.access_method, ctx, f)?;
                render_create_table_options(&self.options, ctx, f)?;
            }
            CreateTableBody::AsQuery {
                columns,
                query,
                with_data,
                ..
            } => {
                render_ctas_columns(columns, ctx, f)?;
                // The CTAS `USING` slot precedes the options and the `AS` (PostgreSQL's
                // `CreateTableAsStmt` order, mirroring the tail order above).
                render_access_method(&self.access_method, ctx, f)?;
                render_create_table_options(&self.options, ctx, f)?;
                render_ctas_query_tail(query, *with_data, ctx, f)?;
            }
            CreateTableBody::AsExecute {
                columns,
                execute,
                with_data,
                ..
            } => {
                render_ctas_columns(columns, ctx, f)?;
                render_access_method(&self.access_method, ctx, f)?;
                render_create_table_options(&self.options, ctx, f)?;
                f.write_str(" AS ")?;
                execute.render(ctx, f)?;
                render_with_data(*with_data, f)?;
            }
        }
        Ok(())
    }
}

/// Render the ` INHERITS (<parent>, ...)` clause when the parent list is non-empty; the
/// leading space is part of the clause separator, and an empty list (no clause) renders nothing.
fn render_inherits(
    inherits: &[ObjectName],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if !inherits.is_empty() {
        f.write_str(" INHERITS (")?;
        render_comma_separated(inherits, ctx, f)?;
        f.write_str(")")?;
    }
    Ok(())
}

/// Render the trailing ` PARTITION BY {LIST | RANGE | HASH} (…)` clause when present; the
/// leading space is part of the clause separator.
fn render_partition_by<X: Extension + Render>(
    partition_by: &Option<Box<PartitionSpec<X>>>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(spec) = partition_by {
        f.write_str(" PARTITION BY ")?;
        spec.render(ctx, f)?;
    }
    Ok(())
}

/// Render the trailing ` USING <access_method>` clause when present; the leading space is part
/// of the clause separator.
fn render_access_method(
    access_method: &Option<Box<Ident>>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(method) = access_method {
        f.write_str(" USING ")?;
        method.render(ctx, f)?;
    }
    Ok(())
}

impl Render for TemporaryTableKind {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Temp => "TEMP",
            Self::Temporary => "TEMPORARY",
        })
    }
}

/// Render a `CREATE TABLE`'s trailing options with the dialect-appropriate separator.
/// SQLite comma-separates its options (`STRICT, WITHOUT ROWID`); MySQL/PostgreSQL
/// space-separate theirs (`ENGINE = InnoDB AUTO_INCREMENT = 100`). The two never mix in
/// one statement, so a SQLite keyword-style option after the first carries the comma;
/// everything else keeps the plain space, and the first option always leads with a space.
fn render_create_table_options<X: Extension + Render>(
    options: &[CreateTableOption<X>],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    for (index, option) in options.iter().enumerate() {
        let sqlite_option = matches!(
            option.kind,
            CreateTableOptionKind::WithoutRowid { .. } | CreateTableOptionKind::Strict { .. }
        );
        f.write_str(if index > 0 && sqlite_option {
            ", "
        } else {
            " "
        })?;
        option.render(ctx, f)?;
    }
    Ok(())
}

impl<X: Extension + Render> Render for CreateTableBody<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Definition { elements, .. } => {
                f.write_str(" (")?;
                render_comma_separated(elements, ctx, f)?;
                f.write_str(")")
            }
            Self::AsQuery {
                columns,
                query,
                with_data,
                ..
            } => {
                render_ctas_columns(columns, ctx, f)?;
                render_ctas_query_tail(query, *with_data, ctx, f)
            }
            Self::AsExecute {
                columns,
                execute,
                with_data,
                ..
            } => {
                render_ctas_columns(columns, ctx, f)?;
                f.write_str(" AS ")?;
                execute.render(ctx, f)?;
                render_with_data(*with_data, f)
            }
            Self::PartitionOf {
                parent,
                elements,
                bound,
                ..
            } => {
                f.write_str(" PARTITION OF ")?;
                parent.render(ctx, f)?;
                // The augmentation list is absent (never empty-with-parens: PostgreSQL rejects
                // an empty `()`), so an empty vec renders no parentheses.
                if !elements.is_empty() {
                    f.write_str(" (")?;
                    render_comma_separated(elements, ctx, f)?;
                    f.write_str(")")?;
                }
                f.write_str(" ")?;
                bound.render(ctx, f)
            }
            Self::OfType {
                type_name,
                elements,
                ..
            } => {
                f.write_str(" OF ")?;
                type_name.render(ctx, f)?;
                // The augmentation list is absent (never empty-with-parens: PostgreSQL rejects an
                // empty `()`), so an empty vec renders no parentheses.
                if !elements.is_empty() {
                    f.write_str(" (")?;
                    render_comma_separated(elements, ctx, f)?;
                    f.write_str(")")?;
                }
                Ok(())
            }
            Self::LikeSource {
                source,
                parenthesized,
                ..
            } => {
                // MySQL's two spellings differ only by the parentheses `parenthesized` records.
                if *parenthesized {
                    f.write_str(" (LIKE ")?;
                    source.render(ctx, f)?;
                    f.write_str(")")
                } else {
                    f.write_str(" LIKE ")?;
                    source.render(ctx, f)
                }
            }
        }
    }
}

impl<X: Extension + Render> Render for PartitionSpec<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.strategy.render(ctx, f)?;
        f.write_str(" (")?;
        render_comma_separated(&self.columns, ctx, f)?;
        f.write_str(")")
    }
}

impl Render for PartitionStrategy {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::List => "LIST",
            Self::Range => "RANGE",
            Self::Hash => "HASH",
        })
    }
}

impl<X: Extension + Render> Render for PartitionElem<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A parenthesized key (`(a + b)`) re-adds the grammar-mandated wrapping parentheses; a
        // bare column / function-call key renders unwrapped (its own binding power never needs
        // them here). The flag, not the expression kind, drives this so `(a)` and `a` round-trip
        // distinctly.
        if self.parenthesized {
            f.write_str("(")?;
            self.expr.render(ctx, f)?;
            f.write_str(")")?;
        } else {
            self.expr.render(ctx, f)?;
        }
        if let Some(collation) = &self.collation {
            f.write_str(" COLLATE ")?;
            collation.render(ctx, f)?;
        }
        if let Some(opclass) = &self.opclass {
            f.write_str(" ")?;
            opclass.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for PartitionBound<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::List { values, .. } => {
                f.write_str("FOR VALUES IN (")?;
                render_comma_separated(values, ctx, f)?;
                f.write_str(")")
            }
            Self::Range { from, to, .. } => {
                f.write_str("FOR VALUES FROM (")?;
                render_comma_separated(from, ctx, f)?;
                f.write_str(") TO (")?;
                render_comma_separated(to, ctx, f)?;
                f.write_str(")")
            }
            Self::Hash {
                modulus, remainder, ..
            } => {
                f.write_str("FOR VALUES WITH (MODULUS ")?;
                modulus.render(ctx, f)?;
                f.write_str(", REMAINDER ")?;
                remainder.render(ctx, f)?;
                f.write_str(")")
            }
            Self::Default { .. } => f.write_str("DEFAULT"),
        }
    }
}

fn render_ctas_columns(
    columns: &[Ident],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if !columns.is_empty() {
        f.write_str(" (")?;
        render_ident_list(columns, ctx, f)?;
        f.write_str(")")?;
    }
    Ok(())
}

fn render_ctas_query_tail<X: Extension + Render>(
    query: &Query<X>,
    with_data: Option<bool>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str(" AS ")?;
    query.render(ctx, f)?;
    render_with_data(with_data, f)
}

/// Render the trailing ` WITH DATA` / ` WITH NO DATA` populate-option of a CTAS or
/// materialized-view body, or nothing when unspecified. The leading space is part
/// of the clause separator.
fn render_with_data(with_data: Option<bool>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match with_data {
        Some(true) => f.write_str(" WITH DATA"),
        Some(false) => f.write_str(" WITH NO DATA"),
        None => Ok(()),
    }
}

impl<X: Extension + Render> Render for TableElement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Column { column, .. } => column.render(ctx, f),
            Self::Constraint { constraint, .. } => constraint.render(ctx, f),
            Self::Like {
                source, options, ..
            } => {
                f.write_str("LIKE ")?;
                source.render(ctx, f)?;
                for option in options {
                    f.write_str(" ")?;
                    option.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl Render for TableLikeOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.action.render(ctx, f)?;
        f.write_str(" ")?;
        self.feature.render(ctx, f)
    }
}

impl Render for TableLikeAction {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Including => "INCLUDING",
            Self::Excluding => "EXCLUDING",
        })
    }
}

impl Render for TableLikeFeature {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Comments => "COMMENTS",
            Self::Compression => "COMPRESSION",
            Self::Constraints => "CONSTRAINTS",
            Self::Defaults => "DEFAULTS",
            Self::Generated => "GENERATED",
            Self::Identity => "IDENTITY",
            Self::Indexes => "INDEXES",
            Self::Statistics => "STATISTICS",
            Self::Storage => "STORAGE",
            Self::All => "ALL",
        })
    }
}

impl<X: Extension + Render> Render for ColumnDef<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        // A SQLite typeless column omits the type entirely (`a`, then its constraints),
        // so the name-to-type separator is written only when a type is present.
        // PostgreSQL grammar order keeps STORAGE/COMPRESSION before constraints.
        render_column_def_tail(self, ctx, f)
    }
}

impl<X: Extension + Render> Render for ColumnConstraint<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.name {
            f.write_str("CONSTRAINT ")?;
            name.render(ctx, f)?;
            // A `Bare` constraint has no element after the name, so no separating space —
            // the `Bare` render arm below writes nothing.
            if !matches!(self.option, ColumnOption::Bare { .. }) {
                f.write_str(" ")?;
            }
        }
        self.option.render(ctx, f)?;
        if let Some(conflict) = self.conflict {
            f.write_str(" ON CONFLICT ")?;
            conflict.render(ctx, f)?;
        }
        render_constraint_characteristics(&self.characteristics, ctx, f)
    }
}

/// Render a trailing ` DEFERRABLE`/` INITIALLY …` characteristics clause when
/// present; the leading space is part of the clause separator.
fn render_constraint_characteristics(
    characteristics: &Option<Box<ConstraintCharacteristics>>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(characteristics) = characteristics {
        f.write_str(" ")?;
        characteristics.render(ctx, f)?;
    }
    Ok(())
}

impl Render for ConstraintCharacteristics {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A separator is needed only between two written clauses, so the second
        // clause carries its own leading space when the first was also present.
        let mut written = false;
        if let Some(deferrable) = self.deferrable {
            f.write_str(if deferrable {
                "DEFERRABLE"
            } else {
                "NOT DEFERRABLE"
            })?;
            written = true;
        }
        if let Some(initially_deferred) = self.initially_deferred {
            if written {
                f.write_str(" ")?;
            }
            f.write_str(if initially_deferred {
                "INITIALLY DEFERRED"
            } else {
                "INITIALLY IMMEDIATE"
            })?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for ColumnOption<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null { .. } => f.write_str("NULL"),
            Self::NotNull { .. } => f.write_str("NOT NULL"),
            Self::Default { expr, .. } => {
                f.write_str("DEFAULT ")?;
                expr.render(ctx, f)
            }
            Self::Generated { generated, .. } => generated.render(ctx, f),
            Self::Identity { identity, .. } => identity.render(ctx, f),
            Self::PrimaryKey {
                ascending,
                index_tablespace,
                ..
            } => {
                f.write_str("PRIMARY KEY")?;
                match ascending {
                    Some(true) => f.write_str(" ASC")?,
                    Some(false) => f.write_str(" DESC")?,
                    None => {}
                }
                render_using_index_tablespace(index_tablespace.as_deref(), ctx, f)
            }
            Self::Unique {
                nulls_not_distinct,
                index_tablespace,
                ..
            } => {
                f.write_str("UNIQUE")?;
                render_nulls_not_distinct(*nulls_not_distinct, f)?;
                render_using_index_tablespace(index_tablespace.as_deref(), ctx, f)
            }
            Self::AutoIncrement { spelling, .. } => spelling.render(ctx, f),
            Self::Collate { collation, .. } => {
                f.write_str("COLLATE ")?;
                collation.render(ctx, f)
            }
            Self::Check {
                expr, no_inherit, ..
            } => {
                f.write_str("CHECK (")?;
                expr.render(ctx, f)?;
                f.write_str(")")?;
                if *no_inherit {
                    f.write_str(" NO INHERIT")?;
                }
                Ok(())
            }
            Self::References { reference, .. } => reference.render(ctx, f),
            // No element text — the enclosing `ColumnConstraint` render skips the separating
            // space it would otherwise write between the name and this element.
            Self::Bare { .. } => Ok(()),
            Self::Other { ext, .. } => ext.render(ctx, f),
        }
    }
}

impl Render for AutoIncrementSpelling {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Underscored => "AUTO_INCREMENT",
            Self::Joined => "AUTOINCREMENT",
        })
    }
}

impl Render for ForeignKeyRef {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("REFERENCES ")?;
        self.table.render(ctx, f)?;
        if !self.columns.is_empty() {
            f.write_str(" (")?;
            render_ident_list(&self.columns, ctx, f)?;
            f.write_str(")")?;
        }
        // Canonical order: MATCH, then ON DELETE, then ON UPDATE. The clauses parse
        // order-independently, so the delete/update order is a normalization — replayed
        // only by a source-fidelity render honouring the `update_before_delete` tag.
        if let Some(match_type) = self.match_type {
            f.write_str(" MATCH ")?;
            match_type.render(ctx, f)?;
        }
        let render_on_delete = |f: &mut fmt::Formatter<'_>| -> fmt::Result {
            if let Some(on_delete) = &self.on_delete {
                f.write_str(" ON DELETE ")?;
                on_delete.render(ctx, f)?;
            }
            Ok(())
        };
        let render_on_update = |f: &mut fmt::Formatter<'_>| -> fmt::Result {
            if let Some(on_update) = &self.on_update {
                f.write_str(" ON UPDATE ")?;
                on_update.render(ctx, f)?;
            }
            Ok(())
        };
        if self.update_before_delete && honours_source_spelling(ctx) {
            render_on_update(f)?;
            render_on_delete(f)?;
        } else {
            render_on_delete(f)?;
            render_on_update(f)?;
        }
        Ok(())
    }
}

impl Render for ForeignKeyMatch {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Full => "FULL",
            Self::Partial => "PARTIAL",
            Self::Simple => "SIMPLE",
        })
    }
}

impl Render for ReferentialAction {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAction { .. } => f.write_str("NO ACTION"),
            Self::Restrict { .. } => f.write_str("RESTRICT"),
            Self::Cascade { .. } => f.write_str("CASCADE"),
            Self::SetNull { columns, .. } => render_set_action(ctx, f, "SET NULL", columns),
            Self::SetDefault { columns, .. } => render_set_action(ctx, f, "SET DEFAULT", columns),
        }
    }
}

/// Render `SET NULL` / `SET DEFAULT` with its optional `(col, ...)` column list.
fn render_set_action(
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
    keyword: &str,
    columns: &[Ident],
) -> fmt::Result {
    f.write_str(keyword)?;
    if !columns.is_empty() {
        f.write_str(" (")?;
        render_ident_list(columns, ctx, f)?;
        f.write_str(")")?;
    }
    Ok(())
}

impl<X: Extension + Render> Render for GeneratedColumn<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The surface tag restores the source spelling: the standard keyworded form or
        // the MySQL/SQLite keywordless `AS (…)` shorthand (ADR-0011).
        f.write_str(match self.spelling {
            GeneratedColumnSpelling::GeneratedAlways => "GENERATED ALWAYS AS (",
            GeneratedColumnSpelling::Shorthand => "AS (",
        })?;
        self.expr.render(ctx, f)?;
        f.write_str(")")?;
        if let Some(storage) = self.storage {
            f.write_str(" ")?;
            storage.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for GeneratedColumnStorage {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Stored => "STORED",
            Self::Virtual => "VIRTUAL",
        })
    }
}

impl<X: Extension + Render> Render for IdentityColumn<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GENERATED ")?;
        self.generation.render(ctx, f)?;
        f.write_str(" AS IDENTITY")?;
        if !self.options.is_empty() {
            f.write_str(" (")?;
            for (index, option) in self.options.iter().enumerate() {
                if index > 0 {
                    f.write_str(" ")?;
                }
                option.render(ctx, f)?;
            }
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl Render for IdentityGeneration {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Always => "ALWAYS",
            Self::ByDefault => "BY DEFAULT",
        })
    }
}

impl<X: Extension + Render> Render for IdentityOption<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartWith { expr, .. } => {
                f.write_str("START WITH ")?;
                expr.render(ctx, f)
            }
            Self::IncrementBy { expr, .. } => {
                f.write_str("INCREMENT BY ")?;
                expr.render(ctx, f)
            }
            Self::MinValue {
                value: Some(expr), ..
            } => {
                f.write_str("MINVALUE ")?;
                expr.render(ctx, f)
            }
            Self::MinValue { value: None, .. } => f.write_str("NO MINVALUE"),
            Self::MaxValue {
                value: Some(expr), ..
            } => {
                f.write_str("MAXVALUE ")?;
                expr.render(ctx, f)
            }
            Self::MaxValue { value: None, .. } => f.write_str("NO MAXVALUE"),
            Self::Cache { expr, .. } => {
                f.write_str("CACHE ")?;
                expr.render(ctx, f)
            }
            Self::Cycle { cycle: true, .. } => f.write_str("CYCLE"),
            Self::Cycle { cycle: false, .. } => f.write_str("NO CYCLE"),
        }
    }
}

impl<X: Extension + Render> Render for TableConstraintDef<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.name {
            f.write_str("CONSTRAINT ")?;
            name.render(ctx, f)?;
            // A `Bare` constraint has no element after the name, so no separating space —
            // the `Bare` render arm below writes nothing.
            if !matches!(self.constraint, TableConstraint::Bare { .. }) {
                f.write_str(" ")?;
            }
        }
        self.constraint.render(ctx, f)?;
        // The `NO INHERIT` / `NOT VALID` markers share PostgreSQL's constraint-attribute slot
        // with the deferral characteristics; a fixed canonical order round-trips (PostgreSQL
        // accepts the markers in any order).
        if self.no_inherit {
            f.write_str(" NO INHERIT")?;
        }
        if self.not_valid {
            f.write_str(" NOT VALID")?;
        }
        render_constraint_characteristics(&self.characteristics, ctx, f)
    }
}

impl<X: Extension + Render> Render for TableConstraint<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrimaryKey {
                columns, include, ..
            } => {
                f.write_str("PRIMARY KEY (")?;
                render_comma_separated(columns, ctx, f)?;
                f.write_str(")")?;
                render_include_columns(include, ctx, f)
            }
            Self::Unique {
                columns,
                nulls_not_distinct,
                include,
                ..
            } => {
                f.write_str("UNIQUE")?;
                render_nulls_not_distinct(*nulls_not_distinct, f)?;
                f.write_str(" (")?;
                render_comma_separated(columns, ctx, f)?;
                f.write_str(")")?;
                render_include_columns(include, ctx, f)
            }
            Self::Check { expr, .. } => {
                f.write_str("CHECK (")?;
                expr.render(ctx, f)?;
                f.write_str(")")
            }
            Self::Exclude { exclude, .. } => exclude.render(ctx, f),
            Self::ForeignKey {
                columns,
                references,
                ..
            } => {
                f.write_str("FOREIGN KEY (")?;
                render_ident_list(columns, ctx, f)?;
                f.write_str(") ")?;
                references.render(ctx, f)
            }
            // No element text — the enclosing `TableConstraintDef` render skips the separating
            // space it would otherwise write between the name and this element.
            Self::Bare { .. } => Ok(()),
            Self::Other { ext, .. } => ext.render(ctx, f),
        }
    }
}

/// Render an `INCLUDE (<col>, ...)` covering-column list; the leading space is part of the
/// clause separator, and an empty list (no clause) renders nothing.
fn render_include_columns(
    include: &[Ident],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if !include.is_empty() {
        f.write_str(" INCLUDE (")?;
        render_ident_list(include, ctx, f)?;
        f.write_str(")")?;
    }
    Ok(())
}

/// Render the ` NULLS [NOT] DISTINCT` null-treatment; the leading space is part of the clause
/// separator, and an unwritten treatment (`None`) renders nothing.
fn render_nulls_not_distinct(
    nulls_not_distinct: Option<bool>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match nulls_not_distinct {
        Some(false) => f.write_str(" NULLS NOT DISTINCT"),
        Some(true) => f.write_str(" NULLS DISTINCT"),
        None => Ok(()),
    }
}

/// Render the ` USING INDEX TABLESPACE <name>` index-parameter clause when present; the leading
/// space is part of the clause separator.
fn render_using_index_tablespace(
    tablespace: Option<&Ident>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(tablespace) = tablespace {
        f.write_str(" USING INDEX TABLESPACE ")?;
        tablespace.render(ctx, f)?;
    }
    Ok(())
}

impl<X: Extension + Render> Render for ExcludeConstraint<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EXCLUDE")?;
        if let Some(method) = &self.method {
            f.write_str(" USING ")?;
            method.render(ctx, f)?;
        }
        f.write_str(" (")?;
        render_comma_separated(&self.elements, ctx, f)?;
        f.write_str(")")?;
        render_include_columns(&self.include, ctx, f)?;
        if !self.with_params.is_empty() {
            f.write_str(" WITH (")?;
            render_comma_separated(&self.with_params, ctx, f)?;
            f.write_str(")")?;
        }
        render_using_index_tablespace(self.index_tablespace.as_ref(), ctx, f)?;
        if let Some(predicate) = &self.predicate {
            f.write_str(" WHERE (")?;
            predicate.render(ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for ExcludeElement<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A parenthesized key re-adds the grammar-mandated wrapping parentheses; a bare column /
        // function-call key renders unwrapped, exactly as `PartitionElem`.
        if self.parenthesized {
            f.write_str("(")?;
            self.expr.render(ctx, f)?;
            f.write_str(")")?;
        } else {
            self.expr.render(ctx, f)?;
        }
        if let Some(collation) = &self.collation {
            f.write_str(" COLLATE ")?;
            collation.render(ctx, f)?;
        }
        if let Some(opclass) = &self.opclass {
            f.write_str(" ")?;
            opclass.render(ctx, f)?;
            if !self.opclass_params.is_empty() {
                f.write_str(" (")?;
                render_comma_separated(&self.opclass_params, ctx, f)?;
                f.write_str(")")?;
            }
        }
        match self.asc {
            Some(true) => f.write_str(" ASC")?,
            Some(false) => f.write_str(" DESC")?,
            None => {}
        }
        match self.nulls_first {
            Some(true) => f.write_str(" NULLS FIRST")?,
            Some(false) => f.write_str(" NULLS LAST")?,
            None => {}
        }
        f.write_str(" WITH ")?;
        self.operator.render(ctx, f)
    }
}

impl Render for ExcludeOperator {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.spelling {
            // The bare form is always unqualified: `a WITH &&`.
            NamedOperatorSpelling::Bare => f.write_str(ctx.resolve(self.op)),
            // The `OPERATOR(...)` keyword form carries the optional schema: `a WITH
            // OPERATOR(pg_catalog.=)`.
            NamedOperatorSpelling::OperatorKeyword => {
                f.write_str("OPERATOR(")?;
                for part in &self.schema.0 {
                    part.render(ctx, f)?;
                    f.write_str(".")?;
                }
                f.write_str(ctx.resolve(self.op))?;
                f.write_str(")")
            }
        }
    }
}

impl<X: Extension + Render> Render for CreateTableOption<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for CreateTableOptionKind<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ColocateWith { table, columns, .. } => {
                f.write_str("COLOCATE WITH ")?;
                table.render(ctx, f)?;
                f.write_str(" ON (")?;
                render_ident_list(columns, ctx, f)?;
                f.write_str(")")
            }
            Self::InColocationGroup { group, columns, .. } => {
                f.write_str("IN COLOCATION GROUP ")?;
                group.render(ctx, f)?;
                if !columns.is_empty() {
                    f.write_str(" ON (")?;
                    render_ident_list(columns, ctx, f)?;
                    f.write_str(")")?;
                }
                Ok(())
            }
            Self::With { params, .. } => {
                f.write_str("WITH (")?;
                render_comma_separated(params, ctx, f)?;
                f.write_str(")")
            }
            Self::OnCommit { action, .. } => {
                f.write_str("ON COMMIT ")?;
                action.render(ctx, f)
            }
            Self::Tablespace { tablespace, .. } => {
                f.write_str("TABLESPACE ")?;
                tablespace.render(ctx, f)
            }
            Self::KeyValue { option, .. } => option.render(ctx, f),
            Self::WithoutRowid { .. } => f.write_str("WITHOUT ROWID"),
            Self::Strict { .. } => f.write_str("STRICT"),
            Self::WithoutOids { .. } => f.write_str("WITHOUT OIDS"),
        }
    }
}

impl Render for TableOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Canonical MySQL spelling: `<name> = <value>`. The optional source `=` and
        // the `DEFAULT` noise prefix are normalized to this fixed form (ADR-0011); the
        // ` = ` matches the `WITH` storage-parameter rendering.
        self.name.render(ctx, f)?;
        f.write_str(" = ")?;
        self.value.render(ctx, f)
    }
}

impl Render for TableOptionValue {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Word { word, .. } => word.render(ctx, f),
            Self::String { value, .. } | Self::Number { value, .. } => value.render(ctx, f),
        }
    }
}

impl Render for OnCommitAction {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PreserveRows => "PRESERVE ROWS",
            Self::DeleteRows => "DELETE ROWS",
            Self::Drop => "DROP",
        })
    }
}

impl<X: Extension + Render> Render for TableStorageParameter<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        if let Some(value) = &self.value {
            f.write_str(" = ")?;
            value.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for AlterTable<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER TABLE")?;
        if self.if_exists {
            f.write_str(" IF EXISTS")?;
        }
        f.write_str(" ")?;
        self.name.render(ctx, f)?;
        f.write_str(" ")?;
        render_comma_separated(&self.actions, ctx, f)
    }
}

impl Render for AlterColumnTarget {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            part.render(ctx, f)?;
        }
        Ok(())
    }
}

/// Render the optional `COLUMN` noise word (with its leading space) after an
/// `ADD`/`DROP`/`ALTER`/`RENAME` column action. The canonical render always emits it;
/// a source-fidelity render drops it when the source did (exact-synonym fidelity).
fn render_optional_column_keyword(
    column_keyword: bool,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if column_keyword || !honours_source_spelling(ctx) {
        f.write_str(" COLUMN")?;
    }
    Ok(())
}

fn render_column_def_tail<X: Extension + Render>(
    column: &ColumnDef<X>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(data_type) = &column.data_type {
        f.write_str(" ")?;
        data_type.render(ctx, f)?;
    }
    if let Some(storage) = &column.storage {
        f.write_str(" STORAGE ")?;
        storage.render(ctx, f)?;
    }
    if let Some(compression) = &column.compression {
        f.write_str(" COMPRESSION ")?;
        compression.render(ctx, f)?;
    }
    for constraint in &column.constraints {
        f.write_str(" ")?;
        constraint.render(ctx, f)?;
    }
    Ok(())
}

impl<X: Extension + Render> Render for AlterTableAction<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SetColocationGroup { group, .. } => {
                f.write_str("SET COLOCATION GROUP ")?;
                group.render(ctx, f)
            }
            Self::DropColocationGroup { .. } => f.write_str("DROP COLOCATION GROUP"),
            // The optional `COLUMN` noise word is exact-synonym fidelity: the canonical
            // render emits it, a source-fidelity render drops it when the source did.
            Self::AddColumn {
                if_not_exists,
                column_keyword,
                target,
                column,
                ..
            } => {
                f.write_str("ADD")?;
                render_optional_column_keyword(*column_keyword, ctx, f)?;
                if *if_not_exists {
                    f.write_str(" IF NOT EXISTS")?;
                }
                f.write_str(" ")?;
                if let Some(target) = target {
                    target.render(ctx, f)?;
                    render_column_def_tail(column, ctx, f)
                } else {
                    column.render(ctx, f)
                }
            }
            Self::DropColumn {
                if_exists,
                column_keyword,
                name,
                behavior,
                ..
            } => {
                f.write_str("DROP")?;
                render_optional_column_keyword(*column_keyword, ctx, f)?;
                if *if_exists {
                    f.write_str(" IF EXISTS")?;
                }
                f.write_str(" ")?;
                name.render(ctx, f)?;
                render_drop_behavior(*behavior, ctx, f)
            }
            Self::AlterColumn {
                column_keyword,
                name,
                change,
                ..
            } => {
                f.write_str("ALTER")?;
                render_optional_column_keyword(*column_keyword, ctx, f)?;
                f.write_str(" ")?;
                name.render(ctx, f)?;
                f.write_str(" ")?;
                change.render(ctx, f)
            }
            Self::AddConstraint { constraint, .. } => {
                f.write_str("ADD ")?;
                constraint.render(ctx, f)
            }
            Self::DropConstraint {
                if_exists,
                name,
                behavior,
                ..
            } => {
                f.write_str("DROP CONSTRAINT")?;
                if *if_exists {
                    f.write_str(" IF EXISTS")?;
                }
                f.write_str(" ")?;
                name.render(ctx, f)?;
                render_drop_behavior(*behavior, ctx, f)
            }
            Self::DropPrimaryKey { behavior, .. } => {
                f.write_str("DROP PRIMARY KEY")?;
                render_drop_behavior(*behavior, ctx, f)
            }
            Self::SetOptions { params, .. } => {
                f.write_str("SET (")?;
                render_comma_separated(params, ctx, f)?;
                f.write_str(")")
            }
            Self::RenameColumn {
                column_keyword,
                name,
                new_name,
                ..
            } => {
                f.write_str("RENAME")?;
                render_optional_column_keyword(*column_keyword, ctx, f)?;
                f.write_str(" ")?;
                name.render(ctx, f)?;
                f.write_str(" TO ")?;
                new_name.render(ctx, f)
            }
            Self::RenameConstraint { name, new_name, .. } => {
                f.write_str("RENAME CONSTRAINT ")?;
                name.render(ctx, f)?;
                f.write_str(" TO ")?;
                new_name.render(ctx, f)
            }
            Self::RenameTable { new_name, .. } => {
                f.write_str("RENAME TO ")?;
                new_name.render(ctx, f)
            }
            Self::AttachPartition {
                partition, bound, ..
            } => {
                f.write_str("ATTACH PARTITION ")?;
                partition.render(ctx, f)?;
                f.write_str(" ")?;
                bound.render(ctx, f)
            }
            Self::DetachPartition {
                partition, mode, ..
            } => {
                f.write_str("DETACH PARTITION ")?;
                partition.render(ctx, f)?;
                match mode {
                    Some(DetachPartitionMode::Concurrently) => f.write_str(" CONCURRENTLY"),
                    Some(DetachPartitionMode::Finalize) => f.write_str(" FINALIZE"),
                    None => Ok(()),
                }
            }
        }
    }
}

impl<X: Extension + Render> Render for AlterColumnAction<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SetDefault { expr, .. } => {
                f.write_str("SET DEFAULT ")?;
                expr.render(ctx, f)
            }
            Self::DropDefault { .. } => f.write_str("DROP DEFAULT"),
            Self::SetNotNull { .. } => f.write_str("SET NOT NULL"),
            Self::DropNotNull { .. } => f.write_str("DROP NOT NULL"),
            Self::AddIdentity { identity, .. } => {
                f.write_str("ADD ")?;
                identity.render(ctx, f)
            }
            // The ANSI `SET DATA TYPE` spelling is canonical; a source-fidelity render
            // replays the bare PostgreSQL `TYPE` when the source wrote it.
            Self::SetDataType {
                set_data,
                data_type,
                using,
                ..
            } => {
                if *set_data || !honours_source_spelling(ctx) {
                    f.write_str("SET DATA TYPE ")?;
                } else {
                    f.write_str("TYPE ")?;
                }
                data_type.render(ctx, f)?;
                if let Some(using) = using {
                    f.write_str(" USING ")?;
                    using.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl Render for DropStatement {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DROP ")?;
        self.object_kind.render(ctx, f)?;
        if self.if_exists {
            f.write_str(" IF EXISTS")?;
        }
        f.write_str(" ")?;
        render_comma_separated(&self.names, ctx, f)?;
        render_drop_behavior(self.behavior, ctx, f)
    }
}

impl Render for DropObjectKind {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Table => "TABLE",
            Self::View => "VIEW",
            Self::MaterializedView => "MATERIALIZED VIEW",
            Self::Index => "INDEX",
            Self::Schema => "SCHEMA",
            Self::Type => "TYPE",
            Self::Sequence => "SEQUENCE",
            Self::Macro => "MACRO",
            Self::MacroTable => "MACRO TABLE",
            Self::Trigger => "TRIGGER",
        })
    }
}

impl Render for DropBehavior {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Cascade => "CASCADE",
            Self::Restrict => "RESTRICT",
        })
    }
}

/// Render a trailing ` CASCADE` / ` RESTRICT` drop behaviour when present; the
/// leading space is part of the clause separator.
fn render_drop_behavior(
    behavior: Option<DropBehavior>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(behavior) = behavior {
        f.write_str(" ")?;
        behavior.render(ctx, f)?;
    }
    Ok(())
}

impl<X: Extension + Render> Render for CreateSchema<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE SCHEMA")?;
        if self.if_not_exists {
            f.write_str(" IF NOT EXISTS")?;
        }
        if let Some(name) = &self.name {
            f.write_str(" ")?;
            name.render(ctx, f)?;
        }
        if let Some(authorization) = &self.authorization {
            f.write_str(" AUTHORIZATION ")?;
            authorization.render(ctx, f)?;
        }
        // Embedded schema elements render space-separated after the head (no `;`), so
        // the whole `CREATE SCHEMA s CREATE TABLE t ...` stays one rendered statement —
        // preserving the statement count and round-tripping through the parser's
        // schema-element loop.
        for element in &self.elements {
            f.write_str(" ")?;
            element.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for CreateView<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE")?;
        if self.or_replace {
            f.write_str(" OR REPLACE")?;
        }
        if let Some(temporary) = self.temporary {
            f.write_str(" ")?;
            temporary.render(ctx, f)?;
        }
        if self.materialized {
            f.write_str(" MATERIALIZED")?;
        }
        // `RECURSIVE` sits between the `TEMP`/`TEMPORARY` prefix and `VIEW`; it never
        // co-occurs with `MATERIALIZED` (parser-enforced), so the order is unambiguous.
        if self.recursive {
            f.write_str(" RECURSIVE")?;
        }
        // The MySQL `[ALGORITHM = …] [DEFINER = …] [SQL SECURITY …]` prefix, between `OR
        // REPLACE` and `VIEW`. All-`None` (every non-MySQL view) emits nothing.
        self.options.render(ctx, f)?;
        f.write_str(" VIEW")?;
        if self.if_not_exists {
            f.write_str(" IF NOT EXISTS")?;
        }
        f.write_str(" ")?;
        self.name.render(ctx, f)?;
        if !self.columns.is_empty() {
            f.write_str(" (")?;
            render_ident_list(&self.columns, ctx, f)?;
            f.write_str(")")?;
        }
        if let Some(to) = &self.to {
            f.write_str(" TO ")?;
            to.render(ctx, f)?;
        }
        f.write_str(" AS ")?;
        self.query.render(ctx, f)?;
        if let Some(check_option) = self.check_option {
            f.write_str(" ")?;
            check_option.render(ctx, f)?;
        }
        render_with_data(self.with_data, f)
    }
}

impl<X: Extension + Render> Render for AlterView<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER")?;
        // The `[ALGORITHM = …] [DEFINER = …] [SQL SECURITY …]` prefix, between `ALTER` and
        // `VIEW`. All-`None` (a bare `ALTER VIEW`) emits nothing.
        self.options.render(ctx, f)?;
        f.write_str(" VIEW ")?;
        self.name.render(ctx, f)?;
        if !self.columns.is_empty() {
            f.write_str(" (")?;
            render_ident_list(&self.columns, ctx, f)?;
            f.write_str(")")?;
        }
        f.write_str(" AS ")?;
        self.query.render(ctx, f)?;
        if let Some(check_option) = self.check_option {
            f.write_str(" ")?;
            check_option.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for ViewOptions {
    /// Emit each present option with a *leading* space, so the prefix slots between the
    /// `CREATE`/`ALTER` keyword and `VIEW`. All-`None` emits nothing (every non-MySQL view).
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(algorithm) = self.algorithm {
            f.write_str(" ALGORITHM = ")?;
            algorithm.render(ctx, f)?;
        }
        if let Some(definer) = &self.definer {
            f.write_str(" ")?;
            definer.render(ctx, f)?;
        }
        if let Some(sql_security) = self.sql_security {
            f.write_str(" SQL SECURITY ")?;
            sql_security.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for ViewAlgorithm {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Undefined => "UNDEFINED",
            Self::Merge => "MERGE",
            Self::TempTable => "TEMPTABLE",
        })
    }
}

impl<X: Extension + Render> Render for CreateSecret<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE ")?;
        if self.persistent {
            f.write_str("PERSISTENT ")?;
        }
        f.write_str("SECRET ")?;
        self.name.render(ctx, f)?;
        f.write_str(" (")?;
        render_comma_separated(&self.options, ctx, f)?;
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for SecretOption<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        f.write_str(" ")?;
        self.value.render(ctx, f)
    }
}

impl Render for DropSecretStmt {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DROP ")?;
        match self.persistence {
            SecretPersistence::Default => {}
            SecretPersistence::Temporary => f.write_str("TEMPORARY ")?,
            SecretPersistence::Persistent => f.write_str("PERSISTENT ")?,
        }
        f.write_str("SECRET ")?;
        if self.if_exists {
            f.write_str("IF EXISTS ")?;
        }
        self.name.render(ctx, f)?;
        if let Some(storage) = &self.storage {
            f.write_str(" FROM ")?;
            storage.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for CreateType<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE")?;
        if self.or_replace {
            f.write_str(" OR REPLACE")?;
        }
        if let Some(temporary) = self.temporary {
            f.write_str(" ")?;
            temporary.render(ctx, f)?;
        }
        f.write_str(" TYPE")?;
        if self.if_not_exists {
            f.write_str(" IF NOT EXISTS")?;
        }
        f.write_str(" ")?;
        self.name.render(ctx, f)?;
        f.write_str(" AS ")?;
        self.definition.render(ctx, f)
    }
}

impl Render for CreateVirtualTable {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE VIRTUAL TABLE ")?;
        if self.if_not_exists {
            f.write_str("IF NOT EXISTS ")?;
        }
        self.name.render(ctx, f)?;
        f.write_str(" USING ")?;
        self.module.render(ctx, f)?;
        if let Some(args) = &self.args {
            f.write_str("(")?;
            render_comma_separated(args, ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl Render for ModuleArg {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The verbatim slice is opaque module-owned content (column names, quoted
        // option strings) that can carry PII, so Redacted rendering masks each argument
        // to one placeholder — arg arity (query shape) survives, the text does not
        // (ADR-0010), mirroring how `Literal` masks values to `?`.
        if ctx.mode() == RenderMode::Redacted {
            return f.write_str("?");
        }
        // Otherwise emit the interned source slice as-is: the module (not this parser)
        // owns the argument grammar, so the text round-trips verbatim.
        f.write_str(ctx.resolve(self.text))
    }
}

impl<X: Extension + Render> Render for CreateSequence<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE")?;
        if let Some(temporary) = self.temporary {
            f.write_str(" ")?;
            temporary.render(ctx, f)?;
        }
        f.write_str(" SEQUENCE")?;
        if self.if_not_exists {
            f.write_str(" IF NOT EXISTS")?;
        }
        f.write_str(" ")?;
        self.name.render(ctx, f)?;
        // The options are space-separated in the canonical `START WITH`/`INCREMENT BY`
        // spelling (which both engines accept); reuse the shared `IdentityOption` render.
        for option in &self.options {
            f.write_str(" ")?;
            option.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for CreateExtension {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE EXTENSION")?;
        if self.if_not_exists {
            f.write_str(" IF NOT EXISTS")?;
        }
        f.write_str(" ")?;
        self.name.render(ctx, f)?;
        // `WITH` is optional sugar; replay it only when the source wrote it.
        if self.with {
            f.write_str(" WITH")?;
        }
        for option in &self.options {
            f.write_str(" ")?;
            option.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for CreateExtensionOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema { name, .. } => {
                f.write_str("SCHEMA ")?;
                name.render(ctx, f)
            }
            Self::Version { version, .. } => {
                f.write_str("VERSION ")?;
                version.render(ctx, f)
            }
            Self::Cascade { .. } => f.write_str("CASCADE"),
        }
    }
}

impl Render for ExtensionVersion {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Word { word, .. } => word.render(ctx, f),
            Self::String { value, .. } => value.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for AlterExtension<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER EXTENSION ")?;
        self.name.render(ctx, f)?;
        f.write_str(" ")?;
        self.action.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for AlterExtensionAction<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Update { version, .. } => {
                f.write_str("UPDATE")?;
                if let Some(version) = version {
                    f.write_str(" TO ")?;
                    version.render(ctx, f)?;
                }
                Ok(())
            }
            Self::Change { add, member, .. } => {
                f.write_str(if *add { "ADD " } else { "DROP " })?;
                member.render(ctx, f)
            }
        }
    }
}

impl Render for SizeLiteral {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The exact source spelling (digits + any suffix, and its case) round-trips from the
        // whole-literal span, exactly as a `Literal` does.
        if let Some(text) = ctx.slice(self.meta.span) {
            return f.write_str(text);
        }
        // A synthesized or detached literal has no backing source; fall back to a
        // placeholder of the tagged unit so rendering stays total.
        f.write_str("0")?;
        match self.unit {
            Some(SizeUnit::Kilo) => f.write_str("K"),
            Some(SizeUnit::Mega) => f.write_str("M"),
            Some(SizeUnit::Giga) => f.write_str("G"),
            None => Ok(()),
        }
    }
}

impl Render for TablespaceOption {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Size {
                kind, equals, size, ..
            } => {
                f.write_str(match kind {
                    TablespaceSizeOption::InitialSize => "INITIAL_SIZE",
                    TablespaceSizeOption::AutoextendSize => "AUTOEXTEND_SIZE",
                    TablespaceSizeOption::MaxSize => "MAX_SIZE",
                    TablespaceSizeOption::ExtentSize => "EXTENT_SIZE",
                    TablespaceSizeOption::UndoBufferSize => "UNDO_BUFFER_SIZE",
                    TablespaceSizeOption::RedoBufferSize => "REDO_BUFFER_SIZE",
                    TablespaceSizeOption::FileBlockSize => "FILE_BLOCK_SIZE",
                })?;
                f.write_str(if *equals { " = " } else { " " })?;
                size.render(ctx, f)
            }
            Self::Nodegroup { equals, value, .. } => {
                f.write_str("NODEGROUP")?;
                f.write_str(if *equals { " = " } else { " " })?;
                value.render(ctx, f)
            }
            Self::Engine {
                storage,
                equals,
                name,
                ..
            } => {
                f.write_str(if *storage { "STORAGE ENGINE" } else { "ENGINE" })?;
                f.write_str(if *equals { " = " } else { " " })?;
                name.render(ctx, f)
            }
            Self::Wait { negated, .. } => f.write_str(if *negated { "NO_WAIT" } else { "WAIT" }),
            Self::Comment { equals, value, .. } => {
                f.write_str("COMMENT")?;
                f.write_str(if *equals { " = " } else { " " })?;
                value.render(ctx, f)
            }
            Self::Encryption { equals, value, .. } => {
                f.write_str("ENCRYPTION")?;
                f.write_str(if *equals { " = " } else { " " })?;
                value.render(ctx, f)
            }
            Self::EngineAttribute { equals, value, .. } => {
                f.write_str("ENGINE_ATTRIBUTE")?;
                f.write_str(if *equals { " = " } else { " " })?;
                value.render(ctx, f)
            }
        }
    }
}

/// Render a trailing option list, each option preceded by a single space (the canonical form
/// space-separates; the source `opt_comma` separators are not replayed). Shared by every
/// tablespace / logfile-group render.
fn render_tablespace_options(
    options: &[TablespaceOption],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    for option in options {
        f.write_str(" ")?;
        option.render(ctx, f)?;
    }
    Ok(())
}

impl Render for CreateTablespace {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.undo {
            "CREATE UNDO TABLESPACE "
        } else {
            "CREATE TABLESPACE "
        })?;
        self.name.render(ctx, f)?;
        if let Some(datafile) = &self.datafile {
            f.write_str(" ADD DATAFILE ")?;
            datafile.render(ctx, f)?;
        }
        if let Some(lg) = &self.use_logfile_group {
            f.write_str(" USE LOGFILE GROUP ")?;
            lg.render(ctx, f)?;
        }
        render_tablespace_options(&self.options, ctx, f)
    }
}

impl Render for AlterTablespace {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The `ALTER UNDO TABLESPACE` head is used exactly when the action is `SetState`.
        f.write_str(
            if matches!(self.action, AlterTablespaceAction::SetState { .. }) {
                "ALTER UNDO TABLESPACE "
            } else {
                "ALTER TABLESPACE "
            },
        )?;
        self.name.render(ctx, f)?;
        f.write_str(" ")?;
        self.action.render(ctx, f)
    }
}

impl Render for AlterTablespaceAction {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddDatafile {
                datafile, options, ..
            } => {
                f.write_str("ADD DATAFILE ")?;
                datafile.render(ctx, f)?;
                render_tablespace_options(options, ctx, f)
            }
            Self::DropDatafile {
                datafile, options, ..
            } => {
                f.write_str("DROP DATAFILE ")?;
                datafile.render(ctx, f)?;
                render_tablespace_options(options, ctx, f)
            }
            Self::Rename { new_name, .. } => {
                f.write_str("RENAME TO ")?;
                new_name.render(ctx, f)
            }
            Self::Options { options, .. } => {
                // The bare option list is non-empty; render the first without a leading space,
                // the rest space-separated.
                let mut first = true;
                for option in options {
                    if !first {
                        f.write_str(" ")?;
                    }
                    first = false;
                    option.render(ctx, f)?;
                }
                Ok(())
            }
            Self::SetState { state, options, .. } => {
                f.write_str(match state {
                    UndoTablespaceState::Active => "SET ACTIVE",
                    UndoTablespaceState::Inactive => "SET INACTIVE",
                })?;
                render_tablespace_options(options, ctx, f)
            }
        }
    }
}

impl Render for DropTablespace {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.undo {
            "DROP UNDO TABLESPACE "
        } else {
            "DROP TABLESPACE "
        })?;
        self.name.render(ctx, f)?;
        render_tablespace_options(&self.options, ctx, f)
    }
}

impl Render for CreateLogfileGroup {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE LOGFILE GROUP ")?;
        self.name.render(ctx, f)?;
        f.write_str(" ADD UNDOFILE ")?;
        self.undofile.render(ctx, f)?;
        render_tablespace_options(&self.options, ctx, f)
    }
}

impl Render for AlterLogfileGroup {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER LOGFILE GROUP ")?;
        self.name.render(ctx, f)?;
        f.write_str(" ADD UNDOFILE ")?;
        self.undofile.render(ctx, f)?;
        render_tablespace_options(&self.options, ctx, f)
    }
}

impl Render for DropLogfileGroup {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DROP LOGFILE GROUP ")?;
        self.name.render(ctx, f)?;
        render_tablespace_options(&self.options, ctx, f)
    }
}

impl<X: Extension + Render> Render for ObjectReference<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named { kind, name, .. } => {
                kind.render(ctx, f)?;
                f.write_str(" ")?;
                name.render(ctx, f)
            }
            Self::Routine {
                kind, signature, ..
            } => {
                kind.render(ctx, f)?;
                f.write_str(" ")?;
                signature.render(ctx, f)
            }
            Self::Aggregate { name, args, .. } => {
                f.write_str("AGGREGATE ")?;
                name.render(ctx, f)?;
                args.render(ctx, f)
            }
            Self::Operator {
                schema, op, args, ..
            } => {
                f.write_str("OPERATOR ")?;
                for part in &schema.0 {
                    part.render(ctx, f)?;
                    f.write_str(".")?;
                }
                f.write_str(ctx.resolve(*op))?;
                f.write_str(" ")?;
                args.render(ctx, f)
            }
            Self::OperatorClass {
                family,
                name,
                access_method,
                ..
            } => {
                f.write_str(if *family {
                    "OPERATOR FAMILY "
                } else {
                    "OPERATOR CLASS "
                })?;
                name.render(ctx, f)?;
                f.write_str(" USING ")?;
                access_method.render(ctx, f)
            }
            Self::Cast { from, to, .. } => {
                f.write_str("CAST (")?;
                from.render(ctx, f)?;
                f.write_str(" AS ")?;
                to.render(ctx, f)?;
                f.write_str(")")
            }
            Self::Type { domain, name, .. } => {
                f.write_str(if *domain { "DOMAIN " } else { "TYPE " })?;
                name.render(ctx, f)
            }
            Self::Transform {
                type_name,
                language,
                ..
            } => {
                f.write_str("TRANSFORM FOR ")?;
                type_name.render(ctx, f)?;
                f.write_str(" LANGUAGE ")?;
                language.render(ctx, f)
            }
            Self::Trigger { name, table, .. } => {
                f.write_str("TRIGGER ")?;
                name.render(ctx, f)?;
                f.write_str(" ON ")?;
                table.render(ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for AlterObjectDepends<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER ")?;
        self.object.render(ctx, f)?;
        if self.no {
            f.write_str(" NO")?;
        }
        f.write_str(" DEPENDS ON EXTENSION ")?;
        self.extension.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for DropTransform<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The `IF EXISTS` guard sits between `TRANSFORM` and `FOR`, so the transform
        // reference's `TRANSFORM FOR type LANGUAGE lang` render can't be emitted as one
        // unit — the parts are spelled out here around the guard. The object is always the
        // `Transform` variant (the parser builds no other for this node).
        let ObjectReference::Transform {
            type_name,
            language,
            ..
        } = &self.object
        else {
            unreachable!("DROP TRANSFORM always carries an ObjectReference::Transform")
        };
        f.write_str("DROP TRANSFORM")?;
        if self.if_exists {
            f.write_str(" IF EXISTS")?;
        }
        f.write_str(" FOR ")?;
        type_name.render(ctx, f)?;
        f.write_str(" LANGUAGE ")?;
        language.render(ctx, f)?;
        render_drop_behavior(self.behavior, ctx, f)
    }
}

impl Render for ObjectRefKind {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Table => "TABLE",
            Self::Sequence => "SEQUENCE",
            Self::View => "VIEW",
            Self::MaterializedView => "MATERIALIZED VIEW",
            Self::Index => "INDEX",
            Self::ForeignTable => "FOREIGN TABLE",
            Self::Collation => "COLLATION",
            Self::Conversion => "CONVERSION",
            Self::Statistics => "STATISTICS",
            Self::TextSearchParser => "TEXT SEARCH PARSER",
            Self::TextSearchDictionary => "TEXT SEARCH DICTIONARY",
            Self::TextSearchTemplate => "TEXT SEARCH TEMPLATE",
            Self::TextSearchConfiguration => "TEXT SEARCH CONFIGURATION",
            Self::AccessMethod => "ACCESS METHOD",
            Self::EventTrigger => "EVENT TRIGGER",
            Self::Extension => "EXTENSION",
            Self::ForeignDataWrapper => "FOREIGN DATA WRAPPER",
            // The `PROCEDURAL` prefix is exact-synonym sugar; the canonical render drops it.
            Self::Language => "LANGUAGE",
            Self::Publication => "PUBLICATION",
            Self::Schema => "SCHEMA",
            Self::Server => "SERVER",
            Self::Database => "DATABASE",
            Self::Role => "ROLE",
            Self::Tablespace => "TABLESPACE",
        })
    }
}

impl<X: Extension + Render> Render for AggregateArgs<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Star { .. } => f.write_str("(*)"),
            Self::Types {
                direct, order_by, ..
            } => {
                f.write_str("(")?;
                render_comma_separated(direct, ctx, f)?;
                if let Some(order_by) = order_by {
                    if !direct.is_empty() {
                        f.write_str(" ")?;
                    }
                    f.write_str("ORDER BY ")?;
                    render_comma_separated(order_by, ctx, f)?;
                }
                f.write_str(")")
            }
        }
    }
}

impl<X: Extension + Render> Render for OperatorArgs<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("(")?;
        match &self.left {
            Some(left) => left.render(ctx, f)?,
            None => f.write_str("NONE")?,
        }
        f.write_str(", ")?;
        match &self.right {
            Some(right) => right.render(ctx, f)?,
            None => f.write_str("NONE")?,
        }
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for CreateTypeDefinition<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Reuses the `ENUM(...)` value-list spelling of `DataType::Enum`; an empty label
            // list renders `ENUM()` (which DuckDB accepts).
            Self::Enum { labels, .. } => render_value_list_type(ctx, "ENUM", labels, f),
            Self::EnumFromQuery { query, .. } => {
                f.write_str("ENUM (")?;
                query.render(ctx, f)?;
                f.write_str(")")
            }
            Self::Alias { data_type, .. } => data_type.render(ctx, f),
        }
    }
}

impl Render for ViewCheckOption {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unspecified => "WITH CHECK OPTION",
            Self::Cascaded => "WITH CASCADED CHECK OPTION",
            Self::Local => "WITH LOCAL CHECK OPTION",
        })
    }
}

impl<X: Extension + Render> Render for CreateIndex<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE")?;
        if self.unique {
            f.write_str(" UNIQUE")?;
        }
        f.write_str(" INDEX")?;
        if self.concurrently {
            f.write_str(" CONCURRENTLY")?;
        }
        if self.if_not_exists {
            f.write_str(" IF NOT EXISTS")?;
        }
        if let Some(name) = &self.name {
            f.write_str(" ")?;
            name.render(ctx, f)?;
        }
        f.write_str(" ON ")?;
        self.table.render(ctx, f)?;
        if let Some(using) = &self.using {
            f.write_str(" USING ")?;
            using.render(ctx, f)?;
        }
        f.write_str(" (")?;
        render_comma_separated(&self.columns, ctx, f)?;
        f.write_str(")")?;
        if !self.with_params.is_empty() {
            f.write_str(" WITH (")?;
            render_comma_separated(&self.with_params, ctx, f)?;
            f.write_str(")")?;
        }
        if let Some(predicate) = &self.predicate {
            f.write_str(" WHERE ")?;
            predicate.render(ctx, f)?;
        }
        Ok(())
    }
}

/// Render the trailing ` ASC`/` DESC` then ` NULLS FIRST`/` NULLS LAST` sort
/// suffixes of a sort key — shared by `IndexColumn` and `OrderByExpr`. Each is a
/// leading-space clause omitted when its option is `None`.
fn render_sort_direction(
    asc: Option<bool>,
    nulls_first: Option<bool>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match asc {
        Some(true) => f.write_str(" ASC")?,
        Some(false) => f.write_str(" DESC")?,
        None => {}
    }
    match nulls_first {
        Some(true) => f.write_str(" NULLS FIRST")?,
        Some(false) => f.write_str(" NULLS LAST")?,
        None => {}
    }
    Ok(())
}

impl<X: Extension + Render> Render for IndexColumn<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expr.render(ctx, f)?;
        render_sort_direction(self.asc, self.nulls_first, f)
    }
}

impl Render for CreateDatabase {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE DATABASE ")?;
        if self.if_not_exists {
            f.write_str("IF NOT EXISTS ")?;
        }
        self.name.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for CreateFunction<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE ")?;
        if self.or_replace {
            f.write_str("OR REPLACE ")?;
        }
        if let Some(definer) = &self.definer {
            definer.render(ctx, f)?;
            f.write_str(" ")?;
        }
        f.write_str("FUNCTION ")?;
        if self.if_not_exists {
            f.write_str("IF NOT EXISTS ")?;
        }
        self.name.render(ctx, f)?;
        // The parameter list is always parenthesized, even when empty.
        f.write_str("(")?;
        render_comma_separated(&self.params, ctx, f)?;
        f.write_str(")")?;
        if let Some(returns) = &self.returns {
            f.write_str(" RETURNS ")?;
            returns.render(ctx, f)?;
        }
        for option in &self.options {
            f.write_str(" ")?;
            option.render(ctx, f)?;
        }
        // The trailing SQL-standard routine body (`RETURN <expr>`) follows the whole option
        // list, its written source position (proven by oracle: it never precedes an option).
        if let Some(body) = &self.body {
            f.write_str(" ")?;
            body.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for FunctionParam<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(mode) = &self.mode {
            f.write_str(match mode {
                FunctionParamMode::In => "IN ",
                FunctionParamMode::Out => "OUT ",
                FunctionParamMode::InOut => "INOUT ",
                FunctionParamMode::Variadic => "VARIADIC ",
            })?;
        }
        if let Some(name) = &self.name {
            name.render(ctx, f)?;
            f.write_str(" ")?;
        }
        self.data_type.render(ctx, f)?;
        if let Some(default) = &self.default {
            f.write_str(" ")?;
            default.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for FunctionParamDefault<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.spelling {
            FunctionParamDefaultSpelling::Default => "DEFAULT ",
            FunctionParamDefaultSpelling::Equals => "= ",
        })?;
        self.value.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for CreateMacro<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE")?;
        if self.or_replace {
            f.write_str(" OR REPLACE")?;
        }
        if let Some(temporary) = self.temporary {
            f.write_str(" ")?;
            temporary.render(ctx, f)?;
        }
        f.write_str(match self.spelling {
            MacroSpelling::Macro => " MACRO",
            MacroSpelling::Function => " FUNCTION",
        })?;
        if self.if_not_exists {
            f.write_str(" IF NOT EXISTS")?;
        }
        f.write_str(" ")?;
        self.name.render(ctx, f)?;
        // The parameter list is always parenthesized, even when empty.
        f.write_str("(")?;
        render_comma_separated(&self.params, ctx, f)?;
        f.write_str(") AS ")?;
        self.body.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for MacroParam<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        if let Some(default) = &self.default {
            f.write_str(" := ")?;
            default.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for MacroBody<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scalar { expr, .. } => expr.render(ctx, f),
            Self::Table { query, .. } => {
                f.write_str("TABLE ")?;
                query.render(ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for FunctionOption<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Language { name, .. } => {
                f.write_str("LANGUAGE ")?;
                name.render(ctx, f)
            }
            Self::As { body, .. } => {
                f.write_str("AS ")?;
                body.render(ctx, f)
            }
            Self::NullBehavior { behavior, .. } => behavior.render(ctx, f),
            Self::Deterministic { not, .. } => f.write_str(if *not {
                "NOT DETERMINISTIC"
            } else {
                "DETERMINISTIC"
            }),
            Self::DataAccess { access, .. } => access.render(ctx, f),
            Self::SqlSecurity { context, .. } => {
                f.write_str("SQL SECURITY ")?;
                context.render(ctx, f)
            }
            Self::Comment { comment, .. } => {
                f.write_str("COMMENT ")?;
                comment.render(ctx, f)
            }
        }
    }
}

impl Render for SqlDataAccess {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ContainsSql => "CONTAINS SQL",
            Self::NoSql => "NO SQL",
            Self::ReadsSqlData => "READS SQL DATA",
            Self::ModifiesSqlData => "MODIFIES SQL DATA",
        })
    }
}

impl Render for SqlSecurityContext {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Definer => "DEFINER",
            Self::Invoker => "INVOKER",
        })
    }
}

impl<X: Extension + Render> Render for FunctionBody<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The body [`Literal`] renders from its source span, so a dollar-quoted body
            // (`$tag$…$tag$`) reproduces its delimiters and verbatim text exactly.
            Self::Definition { definition, .. } => definition.render(ctx, f),
            // The SQL-standard live body: the `RETURN` keyword then the expression.
            Self::Return { expr, .. } => {
                f.write_str("RETURN ")?;
                expr.render(ctx, f)
            }
            // The MySQL SQL/PSM body statement (usually a `BEGIN … END` compound block).
            Self::Block { body, .. } => body.render(ctx, f),
        }
    }
}

impl Render for Definer {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DEFINER = ")?;
        match self {
            Self::Account { user, host, .. } => {
                user.render(ctx, f)?;
                if let Some(host) = host {
                    f.write_str("@")?;
                    host.render(ctx, f)?;
                }
                Ok(())
            }
            Self::CurrentUser { parens, .. } => {
                f.write_str("CURRENT_USER")?;
                if *parens {
                    f.write_str("()")?;
                }
                Ok(())
            }
        }
    }
}

impl<X: Extension + Render> Render for CreateProcedure<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE ")?;
        if let Some(definer) = &self.definer {
            definer.render(ctx, f)?;
            f.write_str(" ")?;
        }
        f.write_str("PROCEDURE ")?;
        if self.if_not_exists {
            f.write_str("IF NOT EXISTS ")?;
        }
        self.name.render(ctx, f)?;
        // The parameter list is always parenthesized, even when empty.
        f.write_str("(")?;
        render_comma_separated(&self.params, ctx, f)?;
        f.write_str(")")?;
        for characteristic in &self.characteristics {
            f.write_str(" ")?;
            characteristic.render(ctx, f)?;
        }
        f.write_str(" ")?;
        self.body.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for AlterRoutine<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            RoutineKind::Procedure => "ALTER PROCEDURE ",
            RoutineKind::Function => "ALTER FUNCTION ",
        })?;
        self.name.render(ctx, f)?;
        for characteristic in &self.characteristics {
            f.write_str(" ")?;
            characteristic.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for FunctionNullBehavior {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CalledOnNull => "CALLED ON NULL INPUT",
            Self::ReturnsNullOnNull => "RETURNS NULL ON NULL INPUT",
            Self::Strict => "STRICT",
        })
    }
}

impl<X: Extension + Render> Render for EventSchedule<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::At { at, .. } => {
                f.write_str("AT ")?;
                at.render(ctx, f)
            }
            Self::Every {
                value,
                unit,
                starts,
                ends,
                ..
            } => {
                f.write_str("EVERY ")?;
                value.render(ctx, f)?;
                // The unit reuses the shared IntervalFields vocabulary in MySQL underscore
                // spelling; the suffix carries its own leading space.
                f.write_str(mysql_interval_unit_suffix(*unit))?;
                if let Some(starts) = starts {
                    f.write_str(" STARTS ")?;
                    starts.render(ctx, f)?;
                }
                if let Some(ends) = ends {
                    f.write_str(" ENDS ")?;
                    ends.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

/// Render the ` ON COMPLETION [NOT] PRESERVE` clause (leading space) shared by
/// `CREATE`/`ALTER EVENT`.
fn render_event_on_completion(
    on_completion: EventOnCompletion,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str(match on_completion {
        EventOnCompletion::Preserve => " ON COMPLETION PRESERVE",
        EventOnCompletion::NotPreserve => " ON COMPLETION NOT PRESERVE",
    })
}

/// Render the ` ENABLE | DISABLE [ON SLAVE|REPLICA]` status clause (leading space).
fn render_event_status(status: EventStatus, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match status {
        EventStatus::Enable => " ENABLE",
        EventStatus::Disable => " DISABLE",
        EventStatus::DisableOnReplica(ReplicaSpelling::Slave) => " DISABLE ON SLAVE",
        EventStatus::DisableOnReplica(ReplicaSpelling::Replica) => " DISABLE ON REPLICA",
    })
}

impl<X: Extension + Render> Render for CreateEvent<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CREATE ")?;
        if let Some(definer) = &self.definer {
            definer.render(ctx, f)?;
            f.write_str(" ")?;
        }
        f.write_str("EVENT ")?;
        if self.if_not_exists {
            f.write_str("IF NOT EXISTS ")?;
        }
        self.name.render(ctx, f)?;
        f.write_str(" ON SCHEDULE ")?;
        self.schedule.render(ctx, f)?;
        if let Some(on_completion) = self.on_completion {
            render_event_on_completion(on_completion, f)?;
        }
        if let Some(status) = self.status {
            render_event_status(status, f)?;
        }
        if let Some(comment) = &self.comment {
            f.write_str(" COMMENT ")?;
            comment.render(ctx, f)?;
        }
        f.write_str(" DO ")?;
        self.body.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for AlterEvent<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALTER ")?;
        if let Some(definer) = &self.definer {
            definer.render(ctx, f)?;
            f.write_str(" ")?;
        }
        f.write_str("EVENT ")?;
        self.name.render(ctx, f)?;
        if let Some(schedule) = &self.schedule {
            f.write_str(" ON SCHEDULE ")?;
            schedule.render(ctx, f)?;
        }
        if let Some(on_completion) = self.on_completion {
            render_event_on_completion(on_completion, f)?;
        }
        if let Some(rename_to) = &self.rename_to {
            f.write_str(" RENAME TO ")?;
            rename_to.render(ctx, f)?;
        }
        if let Some(status) = self.status {
            render_event_status(status, f)?;
        }
        if let Some(comment) = &self.comment {
            f.write_str(" COMMENT ")?;
            comment.render(ctx, f)?;
        }
        if let Some(body) = &self.body {
            f.write_str(" DO ")?;
            body.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for DropEvent {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DROP EVENT ")?;
        if self.if_exists {
            f.write_str("IF EXISTS ")?;
        }
        self.name.render(ctx, f)
    }
}

impl Render for DropDatabase {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.spelling {
            DatabaseKeyword::Database => "DROP DATABASE ",
            DatabaseKeyword::Schema => "DROP SCHEMA ",
        })?;
        if self.if_exists {
            f.write_str("IF EXISTS ")?;
        }
        self.name.render(ctx, f)
    }
}

impl Render for DropIndexOnTable {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DROP INDEX ")?;
        self.name.render(ctx, f)?;
        f.write_str(" ON ")?;
        self.table.render(ctx, f)?;
        for option in &self.options {
            f.write_str(" ")?;
            option.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for IndexLockAlgorithmOption {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (keyword, equals, value) = match self {
            Self::Algorithm { equals, value } => ("ALGORITHM", *equals, value.render_str()),
            Self::Lock { equals, value } => ("LOCK", *equals, value.render_str()),
        };
        f.write_str(keyword)?;
        f.write_str(if equals { " = " } else { " " })?;
        f.write_str(value)
    }
}

impl IndexAlgorithm {
    /// The canonical keyword spelling of this algorithm value.
    fn render_str(self) -> &'static str {
        match self {
            Self::Default => "DEFAULT",
            Self::Inplace => "INPLACE",
            Self::Instant => "INSTANT",
            Self::Copy => "COPY",
        }
    }
}

impl IndexLock {
    /// The canonical keyword spelling of this lock value.
    fn render_str(self) -> &'static str {
        match self {
            Self::Default => "DEFAULT",
            Self::None => "NONE",
            Self::Shared => "SHARED",
            Self::Exclusive => "EXCLUSIVE",
        }
    }
}

impl<X: Extension + Render> Render for Query<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(with) = &self.with {
            with.render(ctx, f)?;
            f.write_str(" ")?;
        }
        self.body.render(ctx, f)?;
        if !self.order_by.is_empty() {
            f.write_str(" ORDER BY ")?;
            render_comma_separated(&self.order_by, ctx, f)?;
        }
        // DuckDB's `ORDER BY ALL` mode; the parser never populates it alongside
        // `order_by` (the engine rejects mixing), so the two arms cannot both fire.
        if let Some(all) = &self.order_by_all {
            f.write_str(" ORDER BY ")?;
            all.render(ctx, f)?;
        }
        // ClickHouse `LIMIT n [OFFSET m] BY …` precedes the ordinary `LIMIT`; a query
        // may carry both, so this is emitted before, not instead of, the `limit` tail.
        if let Some(limit_by) = &self.limit_by {
            f.write_str(" ")?;
            limit_by.render(ctx, f)?;
        }
        if let Some(limit) = &self.limit {
            f.write_str(" ")?;
            limit.render(ctx, f)?;
        }
        // ClickHouse `SETTINGS name = value, …` follows the ordinary `LIMIT` tail.
        if !self.settings.is_empty() {
            f.write_str(" SETTINGS ")?;
            render_comma_separated(&self.settings, ctx, f)?;
        }
        // ClickHouse `FORMAT <name>` closes the query, the last tail after `SETTINGS`.
        if let Some(format) = &self.format {
            f.write_str(" ")?;
            format.render(ctx, f)?;
        }
        // Row-locking clauses trail the whole query, after LIMIT (MySQL's fixed
        // position; PostgreSQL also accepts them here). Space-separated when stacked.
        for locking in &self.locking {
            f.write_str(" ")?;
            locking.render(ctx, f)?;
        }
        // BigQuery/ZetaSQL `|>` pipe operators trail everything else, one ` |> …` step per
        // element in written order. Empty for every shipped preset (the gate is off).
        for op in &self.pipe_operators {
            f.write_str(" ")?;
            op.render(ctx, f)?;
        }
        // MSSQL `FOR XML`/`FOR JSON` result-shaping tail closes the query, after every
        // other clause. `None` unless the MSSQL/Lenient gate is on.
        if let Some(for_clause) = &self.for_clause {
            f.write_str(" ")?;
            for_clause.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for PipeOperator<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The framework seam (render side): one arm per operator, each emitting
        // `|> <KEYWORD> …`. The leading ` ` and the whole-list loop live on `Query`'s
        // render, so an arm writes only its own `|> …`.
        match self {
            PipeOperator::Where { predicate, .. } => {
                f.write_str("|> WHERE ")?;
                predicate.render(ctx, f)
            }
            PipeOperator::Select { items, .. } => {
                f.write_str("|> SELECT ")?;
                render_comma_separated(items, ctx, f)
            }
            PipeOperator::Extend { items, .. } => {
                f.write_str("|> EXTEND ")?;
                render_comma_separated(items, ctx, f)
            }
            PipeOperator::As { alias, .. } => {
                f.write_str("|> AS ")?;
                alias.render(ctx, f)
            }
            PipeOperator::OrderBy { keys, .. } => {
                f.write_str("|> ORDER BY ")?;
                render_comma_separated(keys, ctx, f)
            }
            PipeOperator::Limit { count, offset, .. } => {
                f.write_str("|> LIMIT ")?;
                count.render(ctx, f)?;
                if let Some(offset) = offset {
                    f.write_str(" OFFSET ")?;
                    offset.render(ctx, f)?;
                }
                Ok(())
            }
            PipeOperator::Join { join, .. } => {
                // `Join` renders `<keyword> <relation> <constraint>`, so `|> ` + the join is
                // `|> [<type>] JOIN <relation> [ON | USING]`.
                f.write_str("|> ")?;
                join.render(ctx, f)
            }
            PipeOperator::SetOperation {
                op,
                quantifier,
                queries,
                ..
            } => {
                f.write_str("|> ")?;
                op.render(ctx, f)?;
                if let Some(quantifier) = quantifier {
                    f.write_str(" ")?;
                    quantifier.render(ctx, f)?;
                }
                f.write_str(" ")?;
                // Each operand is a parenthesized query; `Query`'s render emits no
                // surrounding parentheses, so they are written here.
                for (index, query) in queries.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    f.write_str("(")?;
                    query.render(ctx, f)?;
                    f.write_str(")")?;
                }
                Ok(())
            }
            PipeOperator::Set { assignments, .. } => {
                f.write_str("|> SET ")?;
                render_comma_separated(assignments, ctx, f)
            }
            PipeOperator::Call { call, alias, .. } => {
                f.write_str("|> CALL ")?;
                call.render(ctx, f)?;
                if let Some(alias) = alias {
                    f.write_str(" AS ")?;
                    alias.render(ctx, f)?;
                }
                Ok(())
            }
            PipeOperator::Aggregate {
                aggregates,
                group_by,
                ..
            } => {
                // The aggregate list is empty for a grouping-only operator, so the
                // separating space is written only when there is a list to follow it.
                f.write_str("|> AGGREGATE")?;
                if !aggregates.is_empty() {
                    f.write_str(" ")?;
                    render_comma_separated(aggregates, ctx, f)?;
                }
                if !group_by.is_empty() {
                    f.write_str(" GROUP BY ")?;
                    render_comma_separated(group_by, ctx, f)?;
                }
                Ok(())
            }
            PipeOperator::Drop { columns, .. } => {
                f.write_str("|> DROP ")?;
                render_comma_separated(columns, ctx, f)
            }
            PipeOperator::Rename { renames, .. } => {
                f.write_str("|> RENAME ")?;
                render_comma_separated(renames, ctx, f)
            }
            PipeOperator::Pivot {
                aggregates, column, ..
            } => {
                f.write_str("|> PIVOT (")?;
                render_comma_separated(aggregates, ctx, f)?;
                f.write_str(" FOR ")?;
                // `PivotColumn` renders `<col> IN (<values>)`.
                column.render(ctx, f)?;
                f.write_str(")")
            }
            PipeOperator::Unpivot {
                value,
                name,
                columns,
                ..
            } => {
                f.write_str("|> UNPIVOT (")?;
                render_unpivot_name_list(value, ctx, f)?;
                f.write_str(" FOR ")?;
                render_unpivot_name_list(name, ctx, f)?;
                f.write_str(" IN (")?;
                render_comma_separated(columns, ctx, f)?;
                f.write_str("))")
            }
            PipeOperator::TableSample { sample, .. } => {
                // `TableSample`'s render carries its own leading ` TABLESAMPLE ` (it is a
                // `FROM`-suffix node), so the `|>` is written bare against it.
                f.write_str("|>")?;
                sample.render(ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for SemiStructuredAccessExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let full = ctx.mode() == RenderMode::Parenthesized;
        open_group(full, f)?;
        render_operand(
            &self.base,
            operand_needs_parens(
                &ctx.target().binding_powers,
                ctx.target().binding_powers.subscript,
                &self.base,
                Side::Left,
            ),
            ctx,
            f,
        )?;
        let mut segments = self.path.iter();
        if let Some(first) = segments.next() {
            f.write_str(":")?;
            render_semi_structured_path_segment(first, false, ctx, f)?;
        }
        for segment in segments {
            render_semi_structured_path_segment(segment, true, ctx, f)?;
        }
        close_group(full, f)
    }
}

fn render_semi_structured_path_segment<X: Extension + Render>(
    segment: &SemiStructuredPathSegment<X>,
    suffix: bool,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match segment {
        SemiStructuredPathSegment::Key { key, .. } => {
            if suffix {
                f.write_str(".")?;
            }
            key.render(ctx, f)
        }
        SemiStructuredPathSegment::Index { index, .. } => {
            f.write_str("[")?;
            index.render(ctx, f)?;
            f.write_str("]")
        }
    }
}

impl<X: Extension + Render> Render for PipeAggregateExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expr.render(ctx, f)?;
        if let Some(alias) = &self.alias {
            f.write_str(" AS ")?;
            alias.render(ctx, f)?;
        }
        render_sort_direction(self.asc, self.nulls_first, f)
    }
}

impl Render for PipeRenameItem {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.old.render(ctx, f)?;
        f.write_str(" AS ")?;
        self.new.render(ctx, f)
    }
}

impl Render for LockingClause {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.spelling {
            // MySQL's legacy spelling of `FOR SHARE`, a bare clause with no `OF`/wait
            // tail (the parser only builds it on `LockStrength::Share`).
            LockingSpelling::LockInShareMode => return f.write_str("LOCK IN SHARE MODE"),
            LockingSpelling::Modern => {}
        }
        f.write_str(match self.strength {
            LockStrength::Update => "FOR UPDATE",
            LockStrength::NoKeyUpdate => "FOR NO KEY UPDATE",
            LockStrength::Share => "FOR SHARE",
            LockStrength::KeyShare => "FOR KEY SHARE",
        })?;
        if !self.of.is_empty() {
            f.write_str(" OF ")?;
            render_comma_separated(&self.of, ctx, f)?;
        }
        match self.wait {
            Some(LockWait::NoWait) => f.write_str(" NOWAIT")?,
            Some(LockWait::SkipLocked) => f.write_str(" SKIP LOCKED")?,
            None => {}
        }
        Ok(())
    }
}

impl Render for IndexHint {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.action {
            IndexHintAction::Use => "USE ",
            IndexHintAction::Ignore => "IGNORE ",
            IndexHintAction::Force => "FORCE ",
        })?;
        f.write_str(match self.keyword {
            IndexHintKeyword::Index => "INDEX",
            IndexHintKeyword::Key => "KEY",
        })?;
        if let Some(scope) = self.scope {
            f.write_str(match scope {
                IndexHintScope::Join => " FOR JOIN",
                IndexHintScope::OrderBy => " FOR ORDER BY",
                IndexHintScope::GroupBy => " FOR GROUP BY",
            })?;
        }
        // The parenthesized index list is mandatory syntax even when empty
        // (`USE INDEX ()`), so the parens are always written.
        f.write_str(" (")?;
        render_comma_separated(&self.indexes, ctx, f)?;
        f.write_str(")")
    }
}

impl Render for IndexedBy {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IndexedBy::Named { index, .. } => {
                f.write_str("INDEXED BY ")?;
                index.render(ctx, f)
            }
            IndexedBy::NotIndexed { .. } => f.write_str("NOT INDEXED"),
        }
    }
}

impl Render for TableHint {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableHint::Keyword { keyword, .. } => f.write_str(keyword.as_str()),
            // `INDEX = <index>` for a single index under the `=` spelling; otherwise the
            // parenthesized list (`INDEX (a, b)` / `INDEX = (a, b)`).
            TableHint::Index {
                equals, indexes, ..
            } => {
                if *equals && indexes.len() == 1 {
                    f.write_str("INDEX = ")?;
                    indexes[0].render(ctx, f)
                } else {
                    f.write_str(if *equals { "INDEX = (" } else { "INDEX (" })?;
                    render_comma_separated(indexes, ctx, f)?;
                    f.write_str(")")
                }
            }
            TableHint::ForceSeek { target: None, .. } => f.write_str("FORCESEEK"),
            TableHint::ForceSeek {
                target: Some(target),
                ..
            } => {
                f.write_str("FORCESEEK (")?;
                target.index.render(ctx, f)?;
                f.write_str(" (")?;
                render_comma_separated(&target.columns, ctx, f)?;
                f.write_str("))")
            }
            TableHint::Other { ident, .. } => ident.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for TableVersion<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableVersion::ForSystemTimeAsOf { point, .. } => {
                f.write_str("FOR SYSTEM_TIME AS OF ")?;
                point.render(ctx, f)
            }
            TableVersion::ForSystemTimeFromTo { start, end, .. } => {
                f.write_str("FOR SYSTEM_TIME FROM ")?;
                start.render(ctx, f)?;
                f.write_str(" TO ")?;
                end.render(ctx, f)
            }
            TableVersion::ForSystemTimeBetween { start, end, .. } => {
                f.write_str("FOR SYSTEM_TIME BETWEEN ")?;
                start.render(ctx, f)?;
                f.write_str(" AND ")?;
                end.render(ctx, f)
            }
            TableVersion::ForSystemTimeContainedIn { start, end, .. } => {
                f.write_str("FOR SYSTEM_TIME CONTAINED IN (")?;
                start.render(ctx, f)?;
                f.write_str(", ")?;
                end.render(ctx, f)?;
                f.write_str(")")
            }
            TableVersion::ForSystemTimeAll { .. } => f.write_str("FOR SYSTEM_TIME ALL"),
            TableVersion::VersionAsOf { version, .. } => {
                f.write_str("VERSION AS OF ")?;
                version.render(ctx, f)
            }
            TableVersion::TimestampAsOf { point, .. } => {
                f.write_str("TIMESTAMP AS OF ")?;
                point.render(ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for SetExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetExpr::Select { select, .. } => select.render(ctx, f),
            SetExpr::Values { values, .. } => values.render(ctx, f),
            // A nested query body carries its own ORDER BY / LIMIT, so it is
            // parenthesized to stay unambiguous inside an enclosing query.
            SetExpr::Query { query, .. } => {
                f.write_str("(")?;
                query.render(ctx, f)?;
                f.write_str(")")
            }
            SetExpr::SetOperation {
                op,
                all,
                by_name,
                left,
                right,
                ..
            } => {
                render_set_operand(op, left, Side::Left, ctx, f)?;
                f.write_str(" ")?;
                op.render(ctx, f)?;
                if *all {
                    f.write_str(" ALL")?;
                }
                // DuckDB's name-matched `UNION [ALL] BY NAME`, written after the
                // optional `ALL` (`UNION ALL BY NAME`; probed on 1.5.4).
                if *by_name {
                    f.write_str(" BY NAME")?;
                }
                f.write_str(" ")?;
                render_set_operand(op, right, Side::Right, ctx, f)
            }
            // A statement-spelled PIVOT/UNPIVOT standing as a query body renders bare;
            // the enclosing position (a CTE's `( … )`, a `CREATE VIEW … AS`) supplies
            // any parentheses, exactly as `SetExpr::Select` does.
            SetExpr::Pivot { pivot, .. } => pivot.render(ctx, f),
            SetExpr::Unpivot { unpivot, .. } => unpivot.render(ctx, f),
        }
    }
}

/// Render a child set expression under a set-operation parent.
///
/// In `Parenthesized` mode every nested `SetOperation` operand is wrapped; the
/// root query body stays unwrapped so a rendered statement still starts with its
/// query keyword. Canonical/redacted rendering adds only the parentheses needed
/// to preserve grouping under the target dialect's set-operation table.
fn render_set_operand<X: Extension + Render>(
    parent: &SetOperator,
    child: &SetExpr<X>,
    side: Side,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let wrap = match ctx.mode() {
        RenderMode::Parenthesized => matches!(child, SetExpr::SetOperation { .. }),
        RenderMode::Canonical | RenderMode::Redacted => {
            set_child_needs_parens(&ctx.target().set_operation_powers, parent, child, side)
        }
    };
    open_group(wrap, f)?;
    child.render(ctx, f)?;
    close_group(wrap, f)
}

/// Whether a set-operation parent's child needs canonical parentheses.
fn set_child_needs_parens<X: Extension>(
    bp: &SetOperationBindingPowerTable,
    parent: &SetOperator,
    child: &SetExpr<X>,
    side: Side,
) -> bool {
    match child {
        SetExpr::SetOperation { op, .. } => bp.needs_parens(parent, op, side),
        _ => false,
    }
}

impl<X: Extension + Render> Render for With<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WITH")?;
        if self.recursive {
            f.write_str(" RECURSIVE")?;
        }
        render_leading_space_comma_separated(&self.ctes, ctx, f)
    }
}

impl<X: Extension + Render> Render for Cte<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        if !self.columns.is_empty() {
            f.write_str("(")?;
            render_ident_list(&self.columns, ctx, f)?;
            f.write_str(")")?;
        }
        // DuckDB's `USING KEY (cols)` key clause sits between the column list and `AS`.
        if let Some(key_columns) = &self.using_key {
            f.write_str(" USING KEY (")?;
            render_ident_list(key_columns, ctx, f)?;
            f.write_str(")")?;
        }
        f.write_str(" AS")?;
        match self.materialized {
            Some(true) => f.write_str(" MATERIALIZED")?,
            Some(false) => f.write_str(" NOT MATERIALIZED")?,
            None => {}
        }
        f.write_str(" (")?;
        self.body.render(ctx, f)?;
        f.write_str(")")?;
        // The SQL:2023 recursive-query clauses trail the body's `)`, SEARCH before CYCLE.
        if let Some(search) = &self.search {
            f.write_str(" ")?;
            search.render(ctx, f)?;
        }
        if let Some(cycle) = &self.cycle {
            f.write_str(" ")?;
            cycle.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for CteSearchClause {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.breadth_first {
            "SEARCH BREADTH FIRST BY "
        } else {
            "SEARCH DEPTH FIRST BY "
        })?;
        render_ident_list(&self.columns, ctx, f)?;
        f.write_str(" SET ")?;
        self.set_column.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for CteCycleClause<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CYCLE ")?;
        render_ident_list(&self.columns, ctx, f)?;
        f.write_str(" SET ")?;
        self.mark_column.render(ctx, f)?;
        // The `TO value DEFAULT default` mark pair, when present, renders its own leading
        // space; the short form emits nothing between the mark column and `USING`.
        if let Some(mark) = &self.mark {
            mark.render(ctx, f)?;
        }
        f.write_str(" USING ")?;
        self.path_column.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for CteCycleMark<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(" TO ")?;
        self.value.render(ctx, f)?;
        f.write_str(" DEFAULT ")?;
        self.default.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for CteBody<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Each DML arm's node renders its own leading `WITH` and `RETURNING` tail,
        // so a nested `WITH u AS (…) INSERT …` body round-trips through the arm.
        match self {
            Self::Query { query, .. } => query.render(ctx, f),
            Self::Insert { insert, .. } => insert.render(ctx, f),
            Self::Update { update, .. } => update.render(ctx, f),
            Self::Delete { delete, .. } => delete.render(ctx, f),
            Self::Merge { merge, .. } => merge.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for Values<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VALUES ")?;
        render_values_rows(&self.rows, self.explicit_row, ctx, f)
    }
}

impl<X: Extension + Render> Render for ValuesItem<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expr { expr, .. } => expr.render(ctx, f),
            Self::Default { default, .. } => default.render(ctx, f),
        }
    }
}

/// Render the `SELECT [DISTINCT …] [STRAIGHT_JOIN] <projection>` head shared by the
/// SELECT-first and FROM-first spellings. Writes the leading `SELECT`; any preceding
/// space is the caller's.
fn render_select_projection_clause<X: Extension + Render>(
    select: &Select<X>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str("SELECT")?;
    match &select.distinct {
        None => {}
        Some(SelectDistinct::Quantifier { quantifier, .. }) => {
            f.write_str(" ")?;
            quantifier.render(ctx, f)?;
        }
        Some(SelectDistinct::On { exprs, .. }) => {
            f.write_str(" DISTINCT ON (")?;
            render_comma_separated(exprs, ctx, f)?;
            f.write_str(")")?;
        }
    }
    // MySQL `SELECT STRAIGHT_JOIN ...`, after the quantifier and before the list.
    if select.straight_join {
        f.write_str(" STRAIGHT_JOIN")?;
    }
    render_leading_space_comma_separated(&select.projection, ctx, f)
}

/// Render the `[WHERE …] [GROUP BY … | GROUP BY ALL] [HAVING …] [WINDOW …] [QUALIFY …]`
/// tail shared by both spellings — every clause after the projection/`FROM` prefix, in
/// SQL order.
fn render_select_body_tail<X: Extension + Render>(
    select: &Select<X>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    // Hive/Spark lateral views trail the FROM clause and precede WHERE (the parser
    // only fills the field after a non-empty FROM).
    for view in &select.lateral_views {
        f.write_str(" ")?;
        view.render(ctx, f)?;
    }
    if let Some(selection) = &select.selection {
        f.write_str(" WHERE ")?;
        selection.render(ctx, f)?;
    }
    // The Oracle-style hierarchical query clause sits after WHERE and before GROUP BY;
    // the node renders its `START WITH`/`CONNECT BY` pair in the written order.
    if let Some(connect_by) = &select.connect_by {
        f.write_str(" ")?;
        connect_by.render(ctx, f)?;
    }
    if !select.group_by.is_empty() {
        f.write_str(" GROUP BY ")?;
        // PostgreSQL's grouping-set quantifier prefixes the item list (`GROUP BY DISTINCT
        // <items>`); the parser only sets it alongside a non-empty list, so it renders here.
        if let Some(quantifier) = &select.group_by_quantifier {
            quantifier.render(ctx, f)?;
            f.write_str(" ")?;
        }
        render_comma_separated(&select.group_by, ctx, f)?;
    }
    // DuckDB's `GROUP BY ALL` mode; the parser never sets it alongside a
    // non-empty `group_by` (the engine rejects mixing), so at most one arm fires.
    // The `*` shorthand round-trips only under a source-fidelity render; a
    // target-dialect re-spell and the redacted fingerprint canonicalize to `ALL`.
    if let Some(spelling) = select.group_by_all {
        let bare_star = spelling == GroupByAllSpelling::Star && honours_source_spelling(ctx);
        f.write_str(if bare_star {
            " GROUP BY *"
        } else {
            " GROUP BY ALL"
        })?;
    }
    if let Some(having) = &select.having {
        f.write_str(" HAVING ")?;
        having.render(ctx, f)?;
    }
    if !select.windows.is_empty() {
        f.write_str(" WINDOW ")?;
        render_comma_separated(&select.windows, ctx, f)?;
    }
    // QUALIFY follows the WINDOW clause — DuckDB's grammar order (`QUALIFY …
    // WINDOW …` is a DuckDB syntax error), so emitting it last round-trips.
    if let Some(qualify) = &select.qualify {
        f.write_str(" QUALIFY ")?;
        qualify.render(ctx, f)?;
    }
    // DuckDB's `USING SAMPLE` sample clause follows QUALIFY and precedes the enclosing
    // query's `ORDER BY` (the reverse order is a DuckDB syntax error).
    if let Some(sample) = &select.sample {
        f.write_str(" USING SAMPLE ")?;
        sample.render(ctx, f)?;
    }
    Ok(())
}

/// True when a FROM-first body carries the implicit `SELECT *` of the bare
/// `FROM <tables>` form — a single unmodified wildcard with no `DISTINCT`. That surface
/// round-trips to `FROM <tables>` with the `SELECT *` left implicit (the one canonical
/// render for a FROM-first wildcard projection; an explicit `FROM t SELECT *` normalizes
/// onto it).
fn from_first_projection_is_implicit<X: Extension>(select: &Select<X>) -> bool {
    select.distinct.is_none()
        && matches!(select.projection.as_slice(), [SelectItem::Wildcard { .. }])
}

impl<X: Extension + Render> Render for Select<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.spelling {
            // `TABLE name` round-trips its short-form spelling rather than expanding to
            // the canonical `SELECT * FROM name`. The parser is the only constructor of
            // this tag and guarantees the shape — a wildcard projection over one relation
            // — so the relation alone renders the whole command (ADR-0011).
            SelectSpelling::TableCommand => {
                f.write_str("TABLE ")?;
                if let Some(table) = self.from.first() {
                    table.relation.render(ctx, f)?;
                }
                Ok(())
            }
            // DuckDB's FROM-first order: the `FROM` clause leads, then the projection
            // (dropped when it is the bare `FROM t` implicit `SELECT *`), then the shared
            // tail. Every clause after the projection sits in its ordinary place.
            SelectSpelling::FromFirst => {
                f.write_str("FROM ")?;
                render_comma_separated(&self.from, ctx, f)?;
                if !from_first_projection_is_implicit(self) {
                    f.write_str(" ")?;
                    render_select_projection_clause(self, ctx, f)?;
                }
                render_select_body_tail(self, ctx, f)
            }
            SelectSpelling::Select => {
                render_select_projection_clause(self, ctx, f)?;
                // PostgreSQL `SELECT … INTO <table>` sits between the projection and `FROM`.
                if let Some(into) = &self.into {
                    f.write_str(" ")?;
                    into.render(ctx, f)?;
                }
                if !self.from.is_empty() {
                    f.write_str(" FROM ")?;
                    render_comma_separated(&self.from, ctx, f)?;
                }
                render_select_body_tail(self, ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for LateralView<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LATERAL VIEW ")?;
        if self.outer {
            f.write_str("OUTER ")?;
        }
        self.function.render(ctx, f)?;
        f.write_str(" ")?;
        self.alias.render(ctx, f)?;
        if !self.columns.is_empty() {
            // The `AS` is canonical: the AS-less Spark spelling re-renders with the
            // keyword (a structural, not byte-exact, round-trip — see the node doc).
            f.write_str(" AS ")?;
            render_comma_separated(&self.columns, ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for HierarchicalClause<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `START WITH` and `CONNECT BY` render in the written order (Oracle admits
        // either); `NOCYCLE` always modifies `CONNECT BY`, whichever side it lands on.
        let render_start_with = |f: &mut fmt::Formatter<'_>| -> fmt::Result {
            if let Some(start_with) = &self.start_with {
                f.write_str("START WITH ")?;
                start_with.render(ctx, f)?;
            }
            Ok(())
        };
        let render_connect_by = |f: &mut fmt::Formatter<'_>| -> fmt::Result {
            f.write_str("CONNECT BY ")?;
            if self.nocycle {
                f.write_str("NOCYCLE ")?;
            }
            self.connect_by.render(ctx, f)
        };
        if self.start_with_leads && self.start_with.is_some() {
            render_start_with(f)?;
            f.write_str(" ")?;
            render_connect_by(f)
        } else if self.start_with.is_some() {
            render_connect_by(f)?;
            f.write_str(" ")?;
            render_start_with(f)
        } else {
            render_connect_by(f)
        }
    }
}

impl Render for IntoTarget {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("INTO ")?;
        if let Some(temporary) = self.temporary {
            temporary.render(ctx, f)?;
            f.write_str(" ")?;
        }
        self.name.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for SelectItem<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SelectItem::Wildcard {
                options,
                alias,
                alias_spelling,
                ..
            } => {
                f.write_str("*")?;
                render_optional_wildcard_options(options.as_deref(), ctx, f)?;
                render_alias(alias.as_ref(), *alias_spelling, ctx, f)
            }
            SelectItem::QualifiedWildcard {
                name,
                options,
                alias,
                alias_spelling,
                ..
            } => {
                name.render(ctx, f)?;
                f.write_str(".*")?;
                render_optional_wildcard_options(options.as_deref(), ctx, f)?;
                render_alias(alias.as_ref(), *alias_spelling, ctx, f)
            }
            SelectItem::Expr {
                expr,
                alias,
                alias_spelling,
                ..
            } => {
                // DuckDB's prefix form `alias: expr` writes the alias before the
                // value; a source-fidelity render reproduces it, a normalizing render
                // (`TargetDialect`/`Redacted`) falls through to the canonical trailing
                // `AS`.
                if let (Some(alias), AliasSpelling::PrefixColon) = (alias.as_ref(), alias_spelling)
                {
                    if honours_alias_spelling(ctx) {
                        alias.render(ctx, f)?;
                        f.write_str(": ")?;
                        return expr.render(ctx, f);
                    }
                }
                expr.render(ctx, f)?;
                if let Some(alias) = alias {
                    f.write_str(alias_lead(*alias_spelling, ctx))?;
                    alias.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

/// Render the `EXCLUDE`/`REPLACE`/`RENAME` wildcard-modifier tail in DuckDB's fixed
/// canonical order, writing only the non-empty lists. Each list is parenthesized —
/// DuckDB's general spelling, which re-parses for the single-item bare form too (the
/// round-trip contract is structural, not byte-exact).
fn render_wildcard_options<X: Extension + Render>(
    options: &WildcardOptions<X>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if !options.exclude.is_empty() {
        f.write_str(" EXCLUDE (")?;
        render_comma_separated(&options.exclude, ctx, f)?;
        f.write_str(")")?;
    }
    if !options.replace.is_empty() {
        f.write_str(" REPLACE (")?;
        render_comma_separated(&options.replace, ctx, f)?;
        f.write_str(")")?;
    }
    if !options.rename.is_empty() {
        f.write_str(" RENAME (")?;
        render_comma_separated(&options.rename, ctx, f)?;
        f.write_str(")")?;
    }
    Ok(())
}

fn render_optional_wildcard_options<X: Extension + Render>(
    options: Option<&WildcardOptions<X>>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match options {
        Some(options) => render_wildcard_options(options, ctx, f),
        None => Ok(()),
    }
}

/// Render a list-comprehension source: a general expression, or the DuckDB column-star
/// `*` / `(* EXCLUDE (i))` form with its recorded parenthesization and wildcard modifiers.
fn render_comprehension_source<X: Extension + Render>(
    source: &ComprehensionSource<X>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match source {
        ComprehensionSource::Expr { expr, .. } => expr.render(ctx, f),
        ComprehensionSource::Star {
            parenthesized,
            options,
            ..
        } => {
            if *parenthesized {
                f.write_str("(*")?;
                render_optional_wildcard_options(options.as_deref(), ctx, f)?;
                f.write_str(")")
            } else {
                f.write_str("*")?;
                render_optional_wildcard_options(options.as_deref(), ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for WildcardReplace<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expr.render(ctx, f)?;
        f.write_str(" AS ")?;
        self.column.render(ctx, f)
    }
}

impl Render for WildcardRename {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.column.render(ctx, f)?;
        f.write_str(" AS ")?;
        self.alias.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for GroupByItem<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroupByItem::Expr { expr, .. } => expr.render(ctx, f),
            GroupByItem::Rollup {
                exprs, spelling, ..
            } => match spelling {
                RollupSpelling::Function => {
                    f.write_str("ROLLUP (")?;
                    render_comma_separated(exprs, ctx, f)?;
                    f.write_str(")")
                }
                // MySQL's trailing modifier. The parser wraps the whole key list into
                // this single item, so it is the sole GROUP BY item — the GROUP BY
                // list's comma separator never fires between it and a sibling, so the
                // per-item renderer can emit the trailing form directly (no one-item
                // special case needed in the Select group_by rendering).
                RollupSpelling::WithRollup => {
                    render_comma_separated(exprs, ctx, f)?;
                    f.write_str(" WITH ROLLUP")
                }
            },
            GroupByItem::Cube { exprs, .. } => {
                f.write_str("CUBE (")?;
                render_comma_separated(exprs, ctx, f)?;
                f.write_str(")")
            }
            GroupByItem::GroupingSets { sets, .. } => {
                f.write_str("GROUPING SETS (")?;
                render_comma_separated(sets, ctx, f)?;
                f.write_str(")")
            }
            GroupByItem::Empty { .. } => f.write_str("()"),
        }
    }
}

// ---------------------------------------------------------------------------
// FROM clause
// ---------------------------------------------------------------------------

impl<X: Extension + Render> Render for TableWithJoins<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.relation.render(ctx, f)?;
        for join in &self.joins {
            f.write_str(" ")?;
            join.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for TableFactor<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TableFactor::Table {
                name,
                inheritance,
                json_path,
                version,
                partition,
                alias,
                indexed_by,
                index_hints,
                sample,
                table_hints,
                ..
            } => {
                render_relation_inheritance(inheritance, name, ctx, f)?;
                // A PartiQL / SUPER JSON path, attached directly to the table name with no
                // separating space (`FROM src[0].a`). The root is a bracket index; each
                // segment renders as its own suffix (`[index]` or `.key`).
                for segment in json_path {
                    render_semi_structured_path_segment(segment, true, ctx, f)?;
                }
                // A version / time-travel modifier, between the table name and the alias.
                if let Some(version) = version {
                    f.write_str(" ")?;
                    version.render(ctx, f)?;
                }
                // MySQL `PARTITION (p0, p1)`, between the table name and the alias.
                if !partition.is_empty() {
                    f.write_str(" PARTITION (")?;
                    render_comma_separated(partition, ctx, f)?;
                    f.write_str(")")?;
                }
                render_table_alias(alias.as_deref(), ctx, f)?;
                // SQLite `INDEXED BY <index>` / `NOT INDEXED`, immediately after the alias.
                if let Some(indexed_by) = indexed_by {
                    f.write_str(" ")?;
                    indexed_by.render(ctx, f)?;
                }
                // MySQL index hints, after the alias, space-separated (no comma).
                for hint in index_hints {
                    f.write_str(" ")?;
                    hint.render(ctx, f)?;
                }
                if let Some(sample) = sample {
                    sample.render(ctx, f)?;
                }
                // MSSQL `WITH (...)` table hints, after the tablesample clause,
                // comma-separated inside one parenthesized list.
                if !table_hints.is_empty() {
                    f.write_str(" WITH (")?;
                    render_comma_separated(table_hints, ctx, f)?;
                    f.write_str(")")?;
                }
                Ok(())
            }
            TableFactor::Derived {
                lateral,
                subquery,
                alias,
                spelling,
                ..
            } => {
                if *lateral {
                    f.write_str("LATERAL ")?;
                }
                match spelling {
                    DerivedSpelling::Parenthesized => {
                        f.write_str("(")?;
                        subquery.render(ctx, f)?;
                        f.write_str(")")?;
                    }
                    // DuckDB's bare `FROM VALUES (…) AS t`: the body (always a `VALUES`
                    // constructor) renders without the wrapping parentheses; the alias
                    // the parser required trails as usual via `render_table_alias`.
                    DerivedSpelling::BareValues => subquery.render(ctx, f)?,
                }
                render_table_alias(alias.as_deref(), ctx, f)
            }
            TableFactor::Function {
                lateral,
                function,
                with_ordinality,
                alias,
                column_defs,
                ..
            } => {
                if *lateral {
                    f.write_str("LATERAL ")?;
                }
                function.render(ctx, f)?;
                if *with_ordinality {
                    f.write_str(" WITH ORDINALITY")?;
                }
                render_function_alias(alias.as_deref(), column_defs, ctx, f)
            }
            TableFactor::RowsFrom {
                lateral,
                functions,
                with_ordinality,
                alias,
                ..
            } => {
                if *lateral {
                    f.write_str("LATERAL ")?;
                }
                f.write_str("ROWS FROM (")?;
                render_comma_separated(functions, ctx, f)?;
                f.write_str(")")?;
                if *with_ordinality {
                    f.write_str(" WITH ORDINALITY")?;
                }
                render_table_alias(alias.as_deref(), ctx, f)
            }
            TableFactor::Unnest {
                lateral,
                array_exprs,
                with_ordinality,
                alias,
                column_defs,
                with_offset,
                with_offset_alias,
                ..
            } => {
                if *lateral {
                    f.write_str("LATERAL ")?;
                }
                f.write_str("UNNEST(")?;
                render_comma_separated(array_exprs, ctx, f)?;
                f.write_str(")")?;
                // `WITH ORDINALITY` precedes the alias (PostgreSQL/DuckDB); `WITH OFFSET`
                // follows it (BigQuery). The two never co-occur, so rendering each at its
                // grammar position re-parses cleanly under whichever dialect produced it.
                if *with_ordinality {
                    f.write_str(" WITH ORDINALITY")?;
                }
                render_function_alias(alias.as_deref(), column_defs, ctx, f)?;
                if *with_offset {
                    f.write_str(" WITH OFFSET")?;
                    // The `WITH OFFSET` position carries no spelling tag; it keeps the
                    // canonical `AS`, unchanged by this fidelity pass.
                    render_alias(with_offset_alias.as_ref(), AliasSpelling::As, ctx, f)?;
                }
                Ok(())
            }
            TableFactor::NestedJoin { table, alias, .. } => {
                f.write_str("(")?;
                table.render(ctx, f)?;
                f.write_str(")")?;
                render_table_alias(alias.as_deref(), ctx, f)
            }
            TableFactor::SpecialFunction {
                keyword,
                precision,
                alias,
                ..
            } => {
                f.write_str(special_function_keyword(*keyword))?;
                if let Some(precision) = precision {
                    write!(f, "({precision})")?;
                }
                render_table_alias(alias.as_deref(), ctx, f)
            }
            // A statement-spelled core in factor position is the parenthesized
            // statement form (`FROM (PIVOT t ON …)`), so the parentheses rederive
            // from the spelling tag; the suffix spelling needs none of its own.
            TableFactor::Pivot { pivot, alias, .. } => {
                if matches!(pivot.spelling, PivotSpelling::Statement) {
                    f.write_str("(")?;
                    pivot.render(ctx, f)?;
                    f.write_str(")")?;
                } else {
                    pivot.render(ctx, f)?;
                }
                render_table_alias(alias.as_deref(), ctx, f)
            }
            TableFactor::Unpivot { unpivot, alias, .. } => {
                if matches!(unpivot.spelling, UnpivotSpelling::Statement) {
                    f.write_str("(")?;
                    unpivot.render(ctx, f)?;
                    f.write_str(")")?;
                } else {
                    unpivot.render(ctx, f)?;
                }
                render_table_alias(alias.as_deref(), ctx, f)
            }
            TableFactor::MatchRecognize {
                match_recognize,
                alias,
                ..
            } => {
                match_recognize.render(ctx, f)?;
                render_table_alias(alias.as_deref(), ctx, f)
            }
            // DuckDB's `SHOW_REF` table source is always written parenthesized in
            // `FROM` (`FROM (DESCRIBE …)`, `FROM (SHOW databases)`); the parentheses are
            // load-bearing (a bare leading keyword is a top-level statement), so they
            // render unconditionally, like the statement-spelled `TableFactor::Pivot`.
            TableFactor::ShowRef { show, alias, .. } => {
                f.write_str("(")?;
                show.render(ctx, f)?;
                f.write_str(")")?;
                render_table_alias(alias.as_deref(), ctx, f)
            }
            TableFactor::JsonTable {
                json_table, alias, ..
            } => {
                json_table.render(ctx, f)?;
                render_table_alias(alias.as_deref(), ctx, f)
            }
            TableFactor::XmlTable {
                xml_table, alias, ..
            } => {
                xml_table.render(ctx, f)?;
                render_table_alias(alias.as_deref(), ctx, f)
            }
            TableFactor::OpenJson {
                open_json, alias, ..
            } => {
                open_json.render(ctx, f)?;
                render_table_alias(alias.as_deref(), ctx, f)
            }
            TableFactor::TableExpr { expr, alias, .. } => {
                f.write_str("TABLE(")?;
                expr.render(ctx, f)?;
                f.write_str(")")?;
                render_table_alias(alias.as_deref(), ctx, f)
            }
            TableFactor::Other { ext, .. } => ext.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for ShowRef<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            ShowRefKind::Describe => "DESCRIBE",
            ShowRefKind::Desc => "DESC",
            ShowRefKind::Show => "SHOW",
            ShowRefKind::Summarize => "SUMMARIZE",
        })?;
        match &self.target {
            ShowRefTarget::Empty { .. } => Ok(()),
            ShowRefTarget::Query { query, .. } => {
                f.write_str(" ")?;
                query.render(ctx, f)
            }
            ShowRefTarget::Name { name, .. } => {
                f.write_str(" ")?;
                name.render(ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for JsonTable<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.lateral {
            f.write_str("LATERAL ")?;
        }
        f.write_str("JSON_TABLE(")?;
        self.context.render(ctx, f)?;
        f.write_str(", ")?;
        self.path.render(ctx, f)?;
        if let Some(name) = &self.path_name {
            f.write_str(" AS ")?;
            name.render(ctx, f)?;
        }
        if !self.passing.is_empty() {
            f.write_str(" PASSING ")?;
            render_comma_separated(&self.passing, ctx, f)?;
        }
        f.write_str(" COLUMNS (")?;
        render_comma_separated(&self.columns, ctx, f)?;
        f.write_str(")")?;
        render_json_on_behavior(&self.on_error, "ERROR", ctx, f)?;
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for JsonTableColumn<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonTableColumn::ForOrdinality { name, .. } => {
                name.render(ctx, f)?;
                f.write_str(" FOR ORDINALITY")
            }
            JsonTableColumn::Regular {
                name,
                data_type,
                format,
                path,
                wrapper,
                quotes,
                on_empty,
                on_error,
                ..
            } => {
                name.render(ctx, f)?;
                f.write_str(" ")?;
                data_type.render(ctx, f)?;
                if let Some(format) = format {
                    f.write_str(" ")?;
                    format.render(ctx, f)?;
                }
                if let Some(path) = path {
                    f.write_str(" PATH ")?;
                    path.render(ctx, f)?;
                }
                render_json_wrapper(*wrapper, f)?;
                render_json_quotes(*quotes, f)?;
                render_json_on_behavior(on_empty, "EMPTY", ctx, f)?;
                render_json_on_behavior(on_error, "ERROR", ctx, f)
            }
            JsonTableColumn::Exists {
                name,
                data_type,
                path,
                on_error,
                ..
            } => {
                name.render(ctx, f)?;
                f.write_str(" ")?;
                data_type.render(ctx, f)?;
                f.write_str(" EXISTS")?;
                if let Some(path) = path {
                    f.write_str(" PATH ")?;
                    path.render(ctx, f)?;
                }
                render_json_on_behavior(on_error, "ERROR", ctx, f)
            }
            JsonTableColumn::Nested {
                path,
                path_name,
                columns,
                ..
            } => {
                f.write_str("NESTED PATH ")?;
                path.render(ctx, f)?;
                if let Some(name) = path_name {
                    f.write_str(" AS ")?;
                    name.render(ctx, f)?;
                }
                f.write_str(" COLUMNS (")?;
                render_comma_separated(columns, ctx, f)?;
                f.write_str(")")
            }
        }
    }
}

/// Render an expression that occupies a PostgreSQL `c_expr` operand position, wrapping it in
/// parentheses when it is a compound `a_expr` (a binary/unary operator, predicate, …) rather
/// than a `c_expr` primary. In a `c_expr` slot a bare `a || b` is a syntax error, so a parsed
/// `(a || b)` must re-emit its parentheses to round-trip; a primary (a literal, column,
/// function call, parenthesized/atom form) needs none. Redundant parentheses on a primary are
/// harmless — PostgreSQL folds them away — so the allowlist errs toward the common atoms.
fn render_c_expr_operand<X: Extension + Render>(
    expr: &Expr<X>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let is_primary = matches!(
        expr,
        Expr::Column { .. }
            | Expr::Literal { .. }
            | Expr::Function { .. }
            | Expr::Parameter { .. }
            | Expr::PositionalColumn { .. }
            | Expr::SessionVariable { .. }
            | Expr::SpecialFunction { .. }
    );
    if is_primary {
        expr.render(ctx, f)
    } else {
        f.write_str("(")?;
        expr.render(ctx, f)?;
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for XmlTable<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.lateral {
            f.write_str("LATERAL ")?;
        }
        f.write_str("XMLTABLE(")?;
        if !self.namespaces.is_empty() {
            f.write_str("XMLNAMESPACES(")?;
            render_comma_separated(&self.namespaces, ctx, f)?;
            f.write_str("), ")?;
        }
        render_c_expr_operand(&self.row_expr, ctx, f)?;
        f.write_str(" PASSING")?;
        render_xml_passing_mechanism(&self.passing_mechanism_before, f)?;
        f.write_str(" ")?;
        render_c_expr_operand(&self.document, ctx, f)?;
        render_xml_passing_mechanism(&self.passing_mechanism_after, f)?;
        f.write_str(" COLUMNS ")?;
        render_comma_separated(&self.columns, ctx, f)?;
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for XmlNamespace<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Some(name) => {
                self.uri.render(ctx, f)?;
                f.write_str(" AS ")?;
                name.render(ctx, f)
            }
            None => {
                f.write_str("DEFAULT ")?;
                self.uri.render(ctx, f)
            }
        }
    }
}

impl<X: Extension + Render> Render for XmlTableColumn<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XmlTableColumn::ForOrdinality { name, .. } => {
                name.render(ctx, f)?;
                f.write_str(" FOR ORDINALITY")
            }
            XmlTableColumn::Regular {
                name,
                data_type,
                path,
                default,
                not_null,
                ..
            } => {
                name.render(ctx, f)?;
                f.write_str(" ")?;
                data_type.render(ctx, f)?;
                // Canonical order: PATH, DEFAULT, then the nullability declaration.
                // PostgreSQL admits these order-free and normalizes them into fixed
                // fields, so re-emitting one order round-trips to the same node.
                if let Some(path) = path {
                    f.write_str(" PATH ")?;
                    path.render(ctx, f)?;
                }
                if let Some(default) = default {
                    f.write_str(" DEFAULT ")?;
                    default.render(ctx, f)?;
                }
                match not_null {
                    Some(true) => f.write_str(" NOT NULL"),
                    Some(false) => f.write_str(" NULL"),
                    None => Ok(()),
                }
            }
        }
    }
}

impl<X: Extension + Render> Render for OpenJson<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OPENJSON(")?;
        self.json_expr.render(ctx, f)?;
        if let Some(path) = &self.path {
            f.write_str(", ")?;
            path.render(ctx, f)?;
        }
        f.write_str(")")?;
        // An empty `columns` is the absent `WITH` clause (the default schema); a present
        // clause is always non-empty.
        if !self.columns.is_empty() {
            f.write_str(" WITH (")?;
            render_comma_separated(&self.columns, ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for OpenJsonColumn<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        f.write_str(" ")?;
        self.data_type.render(ctx, f)?;
        if let Some(path) = &self.path {
            f.write_str(" ")?;
            path.render(ctx, f)?;
        }
        if self.as_json {
            f.write_str(" AS JSON")?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for Pivot<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.spelling {
            // `[WITH …] PIVOT <source> [ON …] [USING …] [GROUP BY …] [ORDER BY …]
            // [LIMIT …]` — the leading-keyword statement; every clause but the
            // source may be absent.
            PivotSpelling::Statement => {
                if let Some(with) = &self.with {
                    with.render(ctx, f)?;
                    f.write_str(" ")?;
                }
                f.write_str("PIVOT ")?;
                self.source.render(ctx, f)?;
                if !self.pivot_on.is_empty() {
                    f.write_str(" ON ")?;
                    render_comma_separated(&self.pivot_on, ctx, f)?;
                }
                if !self.aggregates.is_empty() {
                    f.write_str(" USING ")?;
                    render_comma_separated(&self.aggregates, ctx, f)?;
                }
                if !self.group_by.is_empty() {
                    f.write_str(" GROUP BY ")?;
                    render_comma_separated(&self.group_by, ctx, f)?;
                }
                render_pivot_statement_tail(
                    &self.order_by,
                    self.order_by_all.as_deref(),
                    self.limit.as_deref(),
                    ctx,
                    f,
                )
            }
            // `<source> PIVOT (<aggregates> FOR <col> IN (<values>) [GROUP BY …])` — the
            // table factor; exactly one `FOR` column (the enclosing alias is rendered by
            // the `TableFactor::Pivot` arm).
            PivotSpelling::TableFactor => {
                self.source.render(ctx, f)?;
                f.write_str(" PIVOT (")?;
                render_comma_separated(&self.aggregates, ctx, f)?;
                // One `FOR` keyword heads the whole column list; the extra heads
                // are written bare, space-separated (`FOR y IN (…) m IN (…)`).
                for (index, column) in self.pivot_on.iter().enumerate() {
                    f.write_str(if index == 0 { " FOR " } else { " " })?;
                    column.render(ctx, f)?;
                }
                if !self.group_by.is_empty() {
                    f.write_str(" GROUP BY ")?;
                    render_comma_separated(&self.group_by, ctx, f)?;
                }
                if let Some(default) = &self.default_on_null {
                    f.write_str(" DEFAULT ON NULL (")?;
                    default.render(ctx, f)?;
                    f.write_str(")")?;
                }
                f.write_str(")")
            }
        }
    }
}

impl<X: Extension + Render> Render for Unpivot<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.spelling {
            // `[WITH …] UNPIVOT <source> ON <cols> [INTO NAME <name> VALUE <value>]
            // [ORDER BY …] [LIMIT …]`.
            UnpivotSpelling::Statement => {
                if let Some(with) = &self.with {
                    with.render(ctx, f)?;
                    f.write_str(" ")?;
                }
                f.write_str("UNPIVOT ")?;
                self.source.render(ctx, f)?;
                f.write_str(" ON ")?;
                render_comma_separated(&self.columns, ctx, f)?;
                if !self.name.is_empty() || !self.value.is_empty() {
                    f.write_str(" INTO NAME ")?;
                    render_comma_separated(&self.name, ctx, f)?;
                    f.write_str(" VALUE ")?;
                    render_comma_separated(&self.value, ctx, f)?;
                }
                render_pivot_statement_tail(
                    &self.order_by,
                    self.order_by_all.as_deref(),
                    self.limit.as_deref(),
                    ctx,
                    f,
                )
            }
            // `<source> UNPIVOT [INCLUDE|EXCLUDE NULLS] (<value> FOR <name> IN (<cols>))`.
            UnpivotSpelling::TableFactor => {
                self.source.render(ctx, f)?;
                f.write_str(" UNPIVOT ")?;
                // A written marker round-trips; the unwritten default (`None`) renders
                // bare (`EXCLUDE NULLS` semantics).
                match self.null_inclusion {
                    Some(NullInclusion::IncludeNulls) => f.write_str("INCLUDE NULLS ")?,
                    Some(NullInclusion::ExcludeNulls) => f.write_str("EXCLUDE NULLS ")?,
                    None => {}
                }
                f.write_str("(")?;
                render_unpivot_name_list(&self.value, ctx, f)?;
                f.write_str(" FOR ")?;
                render_unpivot_name_list(&self.name, ctx, f)?;
                f.write_str(" IN (")?;
                render_comma_separated(&self.columns, ctx, f)?;
                f.write_str("))")
            }
        }
    }
}

/// Render a row-pattern list joined by `separator` (`" "` for concatenation, `" | "`
/// for alternation, `", "` for `PERMUTE`).
fn render_row_patterns(
    patterns: &[MatchRecognizePattern],
    separator: &str,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    for (index, pattern) in patterns.iter().enumerate() {
        if index > 0 {
            f.write_str(separator)?;
        }
        pattern.render(ctx, f)?;
    }
    Ok(())
}

impl<X: Extension + Render> Render for MatchRecognize<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `<source> MATCH_RECOGNIZE ( [PARTITION BY …] [ORDER BY …] [MEASURES …]
        // [ROWS PER MATCH] [AFTER MATCH SKIP …] PATTERN (…) [SUBSET …] [DEFINE …] )`.
        // Every clause but `PATTERN` is optional; `wrote` tracks whether a preceding
        // clause has been emitted so the single-space separators stay tidy.
        self.source.render(ctx, f)?;
        f.write_str(" MATCH_RECOGNIZE (")?;
        let mut wrote = false;
        if !self.partition_by.is_empty() {
            f.write_str("PARTITION BY ")?;
            render_comma_separated(&self.partition_by, ctx, f)?;
            wrote = true;
        }
        if !self.order_by.is_empty() {
            if wrote {
                f.write_str(" ")?;
            }
            f.write_str("ORDER BY ")?;
            render_comma_separated(&self.order_by, ctx, f)?;
            wrote = true;
        }
        if !self.measures.is_empty() {
            if wrote {
                f.write_str(" ")?;
            }
            f.write_str("MEASURES ")?;
            render_comma_separated(&self.measures, ctx, f)?;
            wrote = true;
        }
        if let Some(rows_per_match) = &self.rows_per_match {
            if wrote {
                f.write_str(" ")?;
            }
            rows_per_match.render(ctx, f)?;
            wrote = true;
        }
        if let Some(after_match_skip) = &self.after_match_skip {
            if wrote {
                f.write_str(" ")?;
            }
            after_match_skip.render(ctx, f)?;
            wrote = true;
        }
        if wrote {
            f.write_str(" ")?;
        }
        f.write_str("PATTERN (")?;
        self.pattern.render(ctx, f)?;
        f.write_str(")")?;
        if !self.subsets.is_empty() {
            f.write_str(" SUBSET ")?;
            render_comma_separated(&self.subsets, ctx, f)?;
        }
        if !self.define.is_empty() {
            f.write_str(" DEFINE ")?;
            render_comma_separated(&self.define, ctx, f)?;
        }
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for Measure<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expr.render(ctx, f)?;
        f.write_str(" AS ")?;
        self.alias.render(ctx, f)
    }
}

impl Render for RowsPerMatch {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RowsPerMatch::OneRow => f.write_str("ONE ROW PER MATCH"),
            RowsPerMatch::AllRows(mode) => {
                f.write_str("ALL ROWS PER MATCH")?;
                if let Some(mode) = mode {
                    f.write_str(" ")?;
                    mode.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

impl Render for EmptyMatchesMode {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            EmptyMatchesMode::Show => "SHOW EMPTY MATCHES",
            EmptyMatchesMode::Omit => "OMIT EMPTY MATCHES",
            EmptyMatchesMode::WithUnmatched => "WITH UNMATCHED ROWS",
        })
    }
}

impl Render for AfterMatchSkip {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AFTER MATCH SKIP ")?;
        match self {
            AfterMatchSkip::PastLastRow { .. } => f.write_str("PAST LAST ROW"),
            AfterMatchSkip::ToNextRow { .. } => f.write_str("TO NEXT ROW"),
            AfterMatchSkip::ToFirst { symbol, .. } => {
                f.write_str("TO FIRST ")?;
                symbol.render(ctx, f)
            }
            AfterMatchSkip::ToLast { symbol, .. } => {
                f.write_str("TO LAST ")?;
                symbol.render(ctx, f)
            }
        }
    }
}

impl Render for SubsetDefinition {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        f.write_str(" = (")?;
        render_comma_separated(&self.members, ctx, f)?;
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for SymbolDefinition<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.symbol.render(ctx, f)?;
        f.write_str(" AS ")?;
        self.definition.render(ctx, f)
    }
}

impl Render for MatchRecognizePattern {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchRecognizePattern::Symbol { symbol, .. } => symbol.render(ctx, f),
            MatchRecognizePattern::Start { .. } => f.write_str("^"),
            MatchRecognizePattern::End { .. } => f.write_str("$"),
            MatchRecognizePattern::Concat { patterns, .. } => {
                render_row_patterns(patterns, " ", ctx, f)
            }
            MatchRecognizePattern::Alternation { patterns, .. } => {
                render_row_patterns(patterns, " | ", ctx, f)
            }
            MatchRecognizePattern::Group { pattern, .. } => {
                f.write_str("(")?;
                pattern.render(ctx, f)?;
                f.write_str(")")
            }
            MatchRecognizePattern::Exclude { pattern, .. } => {
                f.write_str("{- ")?;
                pattern.render(ctx, f)?;
                f.write_str(" -}")
            }
            MatchRecognizePattern::Permute { patterns, .. } => {
                f.write_str("PERMUTE(")?;
                render_row_patterns(patterns, ", ", ctx, f)?;
                f.write_str(")")
            }
            MatchRecognizePattern::Repetition {
                pattern,
                quantifier,
                ..
            } => {
                pattern.render(ctx, f)?;
                quantifier.render(ctx, f)
            }
        }
    }
}

impl Render for RepetitionQuantifier {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepetitionQuantifier::ZeroOrMore => f.write_str("*"),
            RepetitionQuantifier::OneOrMore => f.write_str("+"),
            RepetitionQuantifier::AtMostOne => f.write_str("?"),
            RepetitionQuantifier::Exactly(n) => write!(f, "{{{n}}}"),
            RepetitionQuantifier::AtLeast(n) => write!(f, "{{{n},}}"),
            RepetitionQuantifier::AtMost(m) => write!(f, "{{,{m}}}"),
            RepetitionQuantifier::Range(n, m) => write!(f, "{{{n},{m}}}"),
        }
    }
}

impl<X: Extension + Render> Render for PivotExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expr.render(ctx, f)?;
        render_alias(self.alias.as_ref(), self.alias_spelling, ctx, f)
    }
}

impl<X: Extension + Render> Render for PivotValueSource<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PivotValueSource::Any { order_by, .. } => {
                f.write_str("ANY")?;
                if !order_by.is_empty() {
                    f.write_str(" ORDER BY ")?;
                    render_comma_separated(order_by, ctx, f)?;
                }
                Ok(())
            }
            PivotValueSource::Subquery { query, .. } => query.render(ctx, f),
        }
    }
}

impl<X: Extension + Render> Render for PivotColumn<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(enum_source) = &self.enum_source {
            self.expr.render(ctx, f)?;
            f.write_str(" IN ")?;
            return enum_source.render(ctx, f);
        }
        // The standard `IN (ANY [ORDER BY …])` / `IN (<subquery>)` sources — the left
        // operand rederives its grouping parens exactly like the value-list branch.
        if let Some(value_source) = &self.value_source {
            render_predicate_operand(
                &self.expr,
                ctx.target().binding_powers.range_predicate(),
                Side::Left,
                ctx,
                f,
            )?;
            f.write_str(" IN (")?;
            value_source.render(ctx, f)?;
            return f.write_str(")");
        }
        if self.values.is_empty() {
            return self.expr.render(ctx, f);
        }
        // The written `IN` binds at the range-predicate rank exactly like the `IN` predicate
        // the statement parse literally reads it as before unfolding, so the column is rendered
        // as that predicate's left operand — rederiving the grouping parens a comparison-or-looser
        // expression needs (ADR-0008; `(a = b) IN (false, true)` must not re-render as the invalid
        // `a = b IN (…)` chain).
        render_predicate_operand(
            &self.expr,
            ctx.target().binding_powers.range_predicate(),
            Side::Left,
            ctx,
            f,
        )?;
        f.write_str(" IN (")?;
        render_comma_separated(&self.values, ctx, f)?;
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for UnpivotColumn<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A single column renders bare (`a`); a group parenthesizes (`(a, b)`).
        if self.columns.len() == 1 {
            self.columns[0].render(ctx, f)?;
        } else {
            f.write_str("(")?;
            render_comma_separated(&self.columns, ctx, f)?;
            f.write_str(")")?;
        }
        render_alias(self.alias.as_ref(), self.alias_spelling, ctx, f)
    }
}

/// Render an `UNPIVOT` value/name list: a single name bare (`v`), several parenthesized
/// (`(v1, v2)`), matching DuckDB's multi-column unpivot surface.
fn render_unpivot_name_list(
    names: &[Ident],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if names.len() == 1 {
        names[0].render(ctx, f)
    } else {
        f.write_str("(")?;
        render_comma_separated(names, ctx, f)?;
        f.write_str(")")
    }
}

/// Render the pivot statements' trailing `ORDER BY` / `LIMIT` modifiers (the [`Query`]
/// tail pattern, minus the clauses the pivot statements have no grammar for). The
/// parser never populates `order_by_all` alongside `order_by` (the engine rejects
/// mixing), so the two arms cannot both fire.
fn render_pivot_statement_tail<X: Extension + Render>(
    order_by: &[OrderByExpr<X>],
    order_by_all: Option<&OrderByAll>,
    limit: Option<&Limit<X>>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if !order_by.is_empty() {
        f.write_str(" ORDER BY ")?;
        render_comma_separated(order_by, ctx, f)?;
    }
    if let Some(all) = order_by_all {
        f.write_str(" ORDER BY ")?;
        all.render(ctx, f)?;
    }
    if let Some(limit) = limit {
        f.write_str(" ")?;
        limit.render(ctx, f)?;
    }
    Ok(())
}

impl<X: Extension + Render> Render for RowsFromItem<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.function.render(ctx, f)?;
        if !self.column_defs.is_empty() {
            f.write_str(" AS (")?;
            render_comma_separated(&self.column_defs, ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for TableFunctionColumn<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        f.write_str(" ")?;
        self.data_type.render(ctx, f)
    }
}

impl Render for TableAlias {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        if !self.columns.is_empty() {
            f.write_str("(")?;
            render_ident_list(&self.columns, ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for TableSample<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(" TABLESAMPLE ")?;
        self.method.render(ctx, f)?;
        f.write_str(" (")?;
        render_comma_separated(&self.args, ctx, f)?;
        f.write_str(")")?;
        if let Some(repeatable) = &self.repeatable {
            f.write_str(" REPEATABLE (")?;
            repeatable.render(ctx, f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl Render for SampleClause {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Canonical (method-first) spelling of DuckDB's two equivalent entry surfaces
        // (ADR-0011): a named method wraps the count `method(size unit)`, then the seed
        // rides a trailing `REPEATABLE (seed)`; a bare count renders `size unit`. The
        // caller has already written the `USING SAMPLE ` lead.
        if let Some(method) = &self.method {
            method.render(ctx, f)?;
            f.write_str("(")?;
            self.size.render(ctx, f)?;
            self.unit.render(ctx, f)?;
            f.write_str(")")?;
            if let Some(seed) = &self.seed {
                f.write_str(" REPEATABLE (")?;
                seed.render(ctx, f)?;
                f.write_str(")")?;
            }
        } else {
            self.size.render(ctx, f)?;
            self.unit.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for SampleUnit {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SampleUnit::Count => "",
            SampleUnit::Rows => " ROWS",
            SampleUnit::Percent => " PERCENT",
            SampleUnit::PercentSign => "%",
        })
    }
}

/// Whether this render honours the source alias-introducer tag ([`AliasSpelling`]),
/// versus normalizing every alias to the canonical `AS`.
///
/// Only a source-fidelity [`RenderSpelling::PreserveSource`] render honours it. A
/// `TargetDialect` re-spell canonicalizes to `AS` (the prefix-colon form has no
/// target-neutral spelling); the [`Redacted`](RenderMode::Redacted) fingerprint also
/// canonicalizes, so two statements differing only in `AS`-vs-bare share one
/// fingerprint — an alias introducer is cosmetic, like the keyword casing the mask
/// already erases.
fn honours_alias_spelling(ctx: &RenderCtx<'_>) -> bool {
    honours_source_spelling(ctx)
}

/// Whether a source-spelling surface tag is replayed (a fidelity render) versus
/// normalized to its canonical spelling.
///
/// Only a source-fidelity [`RenderSpelling::PreserveSource`] render outside the
/// [`Redacted`](RenderMode::Redacted) fingerprint honours such a tag. A `TargetDialect`
/// re-spell emits the canonical form, and the redacted fingerprint canonicalizes too —
/// so two statements differing only in a cosmetic spelling (`<>` vs `!=`, `LEFT JOIN`
/// vs `LEFT OUTER JOIN`, `SET x = 1` vs `SET x TO 1`) share one fingerprint. The
/// operator/keyword spelling is cosmetic, like the keyword casing the mask already
/// erases. Shared by every keyword/operator spelling tag in this doctrine.
fn honours_source_spelling(ctx: &RenderCtx<'_>) -> bool {
    ctx.spelling() == RenderSpelling::PreserveSource && ctx.mode() != RenderMode::Redacted
}

/// The lead written before an alias name: ` AS ` for an `AS`-introduced or
/// synthesized alias, a bare ` ` when the source omitted `AS` and the render honours
/// that ([`honours_alias_spelling`]). Mirrors the operator spelling tags: a fidelity
/// distinction, not a semantic one.
fn alias_lead(spelling: AliasSpelling, ctx: &RenderCtx<'_>) -> &'static str {
    if !honours_alias_spelling(ctx) {
        return " AS ";
    }
    match spelling {
        AliasSpelling::Bare => " ",
        AliasSpelling::As | AliasSpelling::PrefixColon => " AS ",
    }
}

fn render_table_alias(
    alias: Option<&TableAlias>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(alias) = alias {
        f.write_str(alias_lead(alias.spelling, ctx))?;
        alias.render(ctx, f)?;
    }
    Ok(())
}

/// Render a table function's alias plus its column definition list. An empty
/// `column_defs` is a plain alias; otherwise the typed list renders as
/// `AS [name](col type, ...)` — PostgreSQL's record-returning form, where the
/// optional correlation name precedes the parenthesized definitions.
fn render_function_alias<X: Extension + Render>(
    alias: Option<&TableAlias>,
    column_defs: &[TableFunctionColumn<X>],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if column_defs.is_empty() {
        return render_table_alias(alias, ctx, f);
    }
    // The record-returning form `func(...) AS [name](col type, ...)`: the `AS` leads
    // the definition list even when the correlation name is elided, so a bare source
    // alias only drops the keyword, never the required column list.
    f.write_str(alias.map_or(" AS ", |a| alias_lead(a.spelling, ctx)))?;
    if let Some(alias) = alias {
        alias.name.render(ctx, f)?;
    }
    f.write_str("(")?;
    render_comma_separated(column_defs, ctx, f)?;
    f.write_str(")")
}

fn render_alias(
    alias: Option<&Ident>,
    spelling: AliasSpelling,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(alias) = alias {
        f.write_str(alias_lead(spelling, ctx))?;
        alias.render(ctx, f)?;
    }
    Ok(())
}

/// Render a PostgreSQL `relation_expr` reference: the relation `name` wrapped by
/// its inheritance marker. Shared by `TableFactor::Table` and `DmlTarget` so the
/// four spellings round-trip identically wherever a relation can appear.
fn render_relation_inheritance(
    inheritance: &RelationInheritance,
    name: &ObjectName,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match inheritance {
        RelationInheritance::Plain => name.render(ctx, f),
        RelationInheritance::Descendants => {
            name.render(ctx, f)?;
            f.write_str(" *")
        }
        RelationInheritance::Only(OnlySyntax::Bare) => {
            f.write_str("ONLY ")?;
            name.render(ctx, f)
        }
        RelationInheritance::Only(OnlySyntax::Parenthesized) => {
            f.write_str("ONLY (")?;
            name.render(ctx, f)?;
            f.write_str(")")
        }
    }
}

fn render_ident_list(
    items: &[Ident],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    render_comma_separated(items, ctx, f)
}

/// Render a slice of renderable nodes joined by `, `.
fn render_comma_separated<T: Render>(
    items: &[T],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        item.render(ctx, f)?;
    }
    Ok(())
}

/// Render a comma-separated list attached to a preceding keyword by a single
/// leading space (` a, b, c`), or nothing when empty — the shared shape of a
/// `SELECT` projection, a `WITH` CTE list, and a transaction-mode list.
fn render_leading_space_comma_separated<T: Render>(
    items: &[T],
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if items.is_empty() {
        return Ok(());
    }
    f.write_str(" ")?;
    render_comma_separated(items, ctx, f)
}

/// Render the parenthesized row tuples of a `VALUES` clause — `(a, b), (c, d)` —
/// shared by `InsertValues` and `Values`. The leading `VALUES ` keyword is the
/// caller's; this renders only the comma-separated rows, each a parenthesized
/// comma-separated item list.
fn render_values_rows<T: Render>(
    rows: &[ThinVec<T>],
    explicit_row: bool,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    // MySQL spells the query-position constructor `ROW( ... )`; the bare `( ... )` is the
    // PostgreSQL/DuckDB/SQLite/ANSI spelling. The flag round-trips the `ROW` keyword.
    let open = if explicit_row { "ROW(" } else { "(" };
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        f.write_str(open)?;
        render_comma_separated(row, ctx, f)?;
        f.write_str(")")?;
    }
    Ok(())
}

impl<X: Extension + Render> Render for Join<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Order is `<keyword> <relation> <constraint>`, so the relation sits
        // between the operator's keyword phrase and its trailing ON/USING clause.
        self.operator.render(ctx, f)?;
        f.write_str(" ")?;
        self.relation.render(ctx, f)?;
        if let Some(constraint) = self.operator.constraint() {
            constraint.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension> JoinOperator<X> {
    /// The constraint embedded in a constraint-carrying join operator.
    fn constraint(&self) -> Option<&JoinConstraint<X>> {
        match self {
            JoinOperator::Inner { constraint, .. }
            | JoinOperator::LeftOuter { constraint, .. }
            | JoinOperator::RightOuter { constraint, .. }
            | JoinOperator::FullOuter { constraint, .. }
            | JoinOperator::AsOf { constraint, .. }
            | JoinOperator::Semi { constraint, .. }
            | JoinOperator::Anti { constraint, .. } => Some(constraint),
            JoinOperator::Cross { .. }
            | JoinOperator::Positional { .. }
            | JoinOperator::Apply { .. } => None,
        }
    }
}

impl<X: Extension + Render> Render for JoinOperator<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `NATURAL` is a prefix modifier on the keyword, not a trailing clause.
        if matches!(self.constraint(), Some(JoinConstraint::Natural { .. })) {
            f.write_str("NATURAL ")?;
        }
        // The optional `INNER`/`OUTER` keyword is exact-synonym noise: only a
        // source-fidelity render replays the written form, a target re-spell and the
        // redacted fingerprint collapse to the canonical bare `JOIN` / `LEFT JOIN`.
        let fidelity = honours_source_spelling(ctx);
        f.write_str(match self {
            // The MySQL `straight` surface tag selects the `STRAIGHT_JOIN` keyword;
            // both spellings are the one canonical inner-join shape (ADR-0011).
            JoinOperator::Inner { straight: true, .. } => "STRAIGHT_JOIN",
            JoinOperator::Inner {
                straight: false,
                inner: true,
                ..
            } if fidelity => "INNER JOIN",
            JoinOperator::Inner {
                straight: false, ..
            } => "JOIN",
            JoinOperator::LeftOuter { outer: true, .. } if fidelity => "LEFT OUTER JOIN",
            JoinOperator::LeftOuter { .. } => "LEFT JOIN",
            JoinOperator::RightOuter { outer: true, .. } if fidelity => "RIGHT OUTER JOIN",
            JoinOperator::RightOuter { .. } => "RIGHT JOIN",
            JoinOperator::FullOuter { outer: true, .. } if fidelity => "FULL OUTER JOIN",
            JoinOperator::FullOuter { .. } => "FULL JOIN",
            // The canonical ASOF spelling records the side, not the `INNER`/`OUTER`
            // noise (like the side joins above).
            JoinOperator::AsOf { kind, .. } => match kind {
                AsOfJoinKind::Inner => "ASOF JOIN",
                AsOfJoinKind::Left => "ASOF LEFT JOIN",
                AsOfJoinKind::Right => "ASOF RIGHT JOIN",
                AsOfJoinKind::Full => "ASOF FULL JOIN",
            },
            JoinOperator::Cross { .. } => "CROSS JOIN",
            JoinOperator::Positional { .. } => "POSITIONAL JOIN",
            // DuckDB's side-less `SEMI`/`ANTI` is the whole `join_type`; its `ASOF`
            // composition prefixes the keyword and its `NATURAL` one is emitted by the
            // prefix above (the two never co-occur, so `asof` is `false` under NATURAL).
            // Spark's sided spelling writes the `LEFT`/`RIGHT` keyword instead and never
            // composes with `ASOF`/`NATURAL`, so `asof` is always `false` there.
            JoinOperator::Semi { asof, side, .. } => match (side, asof) {
                (SemiAntiSide::Sideless, false) => "SEMI JOIN",
                (SemiAntiSide::Sideless, true) => "ASOF SEMI JOIN",
                (SemiAntiSide::Left, _) => "LEFT SEMI JOIN",
                (SemiAntiSide::Right, _) => "RIGHT SEMI JOIN",
            },
            JoinOperator::Anti { asof, side, .. } => match (side, asof) {
                (SemiAntiSide::Sideless, false) => "ANTI JOIN",
                (SemiAntiSide::Sideless, true) => "ASOF ANTI JOIN",
                (SemiAntiSide::Left, _) => "LEFT ANTI JOIN",
                (SemiAntiSide::Right, _) => "RIGHT ANTI JOIN",
            },
            // The `CROSS`/`OUTER` flavour is the whole operator keyword; the right table
            // factor renders after it (no constraint, like CROSS/POSITIONAL above).
            JoinOperator::Apply { kind, .. } => match kind {
                ApplyKind::Cross => "CROSS APPLY",
                ApplyKind::Outer => "OUTER APPLY",
            },
        })
    }
}

impl<X: Extension + Render> Render for JoinConstraint<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JoinConstraint::On { expr, .. } => {
                f.write_str(" ON ")?;
                expr.render(ctx, f)
            }
            JoinConstraint::Using { columns, alias, .. } => {
                f.write_str(" USING (")?;
                render_comma_separated(columns, ctx, f)?;
                f.write_str(")")?;
                if let Some(alias) = alias {
                    f.write_str(" AS ")?;
                    alias.render(ctx, f)?;
                }
                Ok(())
            }
            // `Natural` is emitted as the keyword prefix; `None` has no clause.
            JoinConstraint::Natural { .. } | JoinConstraint::None { .. } => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// ORDER BY / LIMIT
// ---------------------------------------------------------------------------

impl<X: Extension + Render> Render for ExtractExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EXTRACT(")?;
        self.field.render(ctx, f)?;
        f.write_str(" FROM ")?;
        self.source.render(ctx, f)?;
        f.write_str(")")
    }
}

impl Render for NullTreatment {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            NullTreatment::IgnoreNulls => "IGNORE NULLS",
            NullTreatment::RespectNulls => "RESPECT NULLS",
        })
    }
}

// ---------------------------------------------------------------------------
// SQL/JSON expression functions (pg-sqljson-expression-functions)
// ---------------------------------------------------------------------------

impl Render for JsonEncoding {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            JsonEncoding::Utf8 => "UTF8",
            JsonEncoding::Utf16 => "UTF16",
            JsonEncoding::Utf32 => "UTF32",
        })
    }
}

impl Render for JsonFormat {
    /// Renders `FORMAT JSON [ENCODING <enc>]`; callers write the leading space.
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FORMAT JSON")?;
        if let Some(encoding) = &self.encoding {
            f.write_str(" ENCODING ")?;
            encoding.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for JsonValueExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expr.render(ctx, f)?;
        if let Some(format) = &self.format {
            f.write_str(" ")?;
            format.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for JsonReturning<X> {
    /// Renders `RETURNING <type> [FORMAT JSON …]`; callers write the leading space.
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RETURNING ")?;
        self.data_type.render(ctx, f)?;
        if let Some(format) = &self.format {
            f.write_str(" ")?;
            format.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for JsonPassingArg<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.render(ctx, f)?;
        f.write_str(" AS ")?;
        self.name.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for JsonBehavior<X> {
    /// Renders the behaviour keyword(s) only; callers append ` ON EMPTY` / ` ON ERROR`.
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            JsonBehaviorKind::Error => f.write_str("ERROR"),
            JsonBehaviorKind::Null => f.write_str("NULL"),
            JsonBehaviorKind::True => f.write_str("TRUE"),
            JsonBehaviorKind::False => f.write_str("FALSE"),
            JsonBehaviorKind::Unknown => f.write_str("UNKNOWN"),
            JsonBehaviorKind::Empty => f.write_str("EMPTY"),
            JsonBehaviorKind::EmptyArray => f.write_str("EMPTY ARRAY"),
            JsonBehaviorKind::EmptyObject => f.write_str("EMPTY OBJECT"),
            JsonBehaviorKind::Default => {
                f.write_str("DEFAULT ")?;
                // `default_expr` is always `Some` when the kind is `Default`; the render
                // is a no-op otherwise rather than panicking on a malformed node.
                if let Some(expr) = &self.default_expr {
                    expr.render(ctx, f)?;
                }
                Ok(())
            }
        }
    }
}

/// Render an `ON EMPTY` / `ON ERROR` behaviour, when present: ` <behaviour> ON <slot>`.
fn render_json_on_behavior<X: Extension + Render>(
    behavior: &Option<JsonBehavior<X>>,
    slot: &str,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(behavior) = behavior {
        f.write_str(" ")?;
        behavior.render(ctx, f)?;
        f.write_str(" ON ")?;
        f.write_str(slot)?;
    }
    Ok(())
}

/// Render a SQL/JSON `WRAPPER` clause when specified; callers write no leading space.
fn render_json_wrapper(wrapper: JsonWrapperBehavior, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match wrapper {
        JsonWrapperBehavior::Unspecified => Ok(()),
        JsonWrapperBehavior::Without => f.write_str(" WITHOUT WRAPPER"),
        JsonWrapperBehavior::Unconditional => f.write_str(" WITH WRAPPER"),
        JsonWrapperBehavior::Conditional => f.write_str(" WITH CONDITIONAL WRAPPER"),
    }
}

/// Render a SQL/JSON `QUOTES` clause when specified; callers write no leading space.
fn render_json_quotes(quotes: JsonQuotesBehavior, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match quotes {
        JsonQuotesBehavior::Unspecified => Ok(()),
        JsonQuotesBehavior::Keep => f.write_str(" KEEP QUOTES"),
        JsonQuotesBehavior::Omit => f.write_str(" OMIT QUOTES"),
    }
}

/// Render a null-handling clause when present: ` ABSENT ON NULL` / ` NULL ON NULL`.
fn render_json_null_clause(
    clause: &Option<JsonNullClause>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match clause {
        Some(JsonNullClause::AbsentOnNull) => f.write_str(" ABSENT ON NULL"),
        Some(JsonNullClause::NullOnNull) => f.write_str(" NULL ON NULL"),
        None => Ok(()),
    }
}

/// Render a key-uniqueness clause when present: ` WITH UNIQUE KEYS` / ` WITHOUT UNIQUE KEYS`.
fn render_json_unique(unique_keys: Option<bool>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match unique_keys {
        Some(true) => f.write_str(" WITH UNIQUE KEYS"),
        Some(false) => f.write_str(" WITHOUT UNIQUE KEYS"),
        None => Ok(()),
    }
}

impl<X: Extension + Render> Render for JsonFuncExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            JsonFuncKind::Value => "JSON_VALUE(",
            JsonFuncKind::Query => "JSON_QUERY(",
            JsonFuncKind::Exists => "JSON_EXISTS(",
        })?;
        self.context.render(ctx, f)?;
        f.write_str(", ")?;
        self.path.render(ctx, f)?;
        if !self.passing.is_empty() {
            f.write_str(" PASSING ")?;
            render_comma_separated(&self.passing, ctx, f)?;
        }
        if let Some(returning) = &self.returning {
            f.write_str(" ")?;
            returning.render(ctx, f)?;
        }
        render_json_wrapper(self.wrapper, f)?;
        render_json_quotes(self.quotes, f)?;
        render_json_on_behavior(&self.on_empty, "EMPTY", ctx, f)?;
        render_json_on_behavior(&self.on_error, "ERROR", ctx, f)?;
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for JsonKeyValue<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.key.render(ctx, f)?;
        f.write_str(match self.spelling {
            JsonKeyValueSpelling::Colon => ": ",
            JsonKeyValueSpelling::Value => " VALUE ",
        })?;
        self.value.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for JsonObjectExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON_OBJECT(")?;
        render_comma_separated(&self.entries, ctx, f)?;
        render_json_null_clause(&self.null_clause, f)?;
        render_json_unique(self.unique_keys, f)?;
        if let Some(returning) = &self.returning {
            if !self.entries.is_empty() || self.null_clause.is_some() || self.unique_keys.is_some()
            {
                f.write_str(" ")?;
            }
            returning.render(ctx, f)?;
        }
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for JsonArrayExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON_ARRAY(")?;
        let empty_body = match &self.body {
            JsonArrayBody::Values {
                items, null_clause, ..
            } => {
                render_comma_separated(items, ctx, f)?;
                render_json_null_clause(null_clause, f)?;
                items.is_empty() && null_clause.is_none()
            }
            JsonArrayBody::Query { query, format, .. } => {
                query.render(ctx, f)?;
                if let Some(format) = format {
                    f.write_str(" ")?;
                    format.render(ctx, f)?;
                }
                false
            }
        };
        if let Some(returning) = &self.returning {
            if !empty_body {
                f.write_str(" ")?;
            }
            returning.render(ctx, f)?;
        }
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for JsonAggregateExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.body {
            JsonAggregateBody::Object {
                entry, unique_keys, ..
            } => {
                f.write_str("JSON_OBJECTAGG(")?;
                entry.render(ctx, f)?;
                render_json_null_clause(&self.null_clause, f)?;
                render_json_unique(*unique_keys, f)?;
            }
            JsonAggregateBody::Array {
                value, order_by, ..
            } => {
                f.write_str("JSON_ARRAYAGG(")?;
                value.render(ctx, f)?;
                if !order_by.is_empty() {
                    f.write_str(" ORDER BY ")?;
                    render_comma_separated(order_by, ctx, f)?;
                }
                render_json_null_clause(&self.null_clause, f)?;
            }
        }
        if let Some(returning) = &self.returning {
            f.write_str(" ")?;
            returning.render(ctx, f)?;
        }
        f.write_str(")")?;
        if let Some(filter) = &self.filter {
            f.write_str(" FILTER (WHERE ")?;
            filter.render(ctx, f)?;
            f.write_str(")")?;
        }
        if let Some(over) = &self.over {
            f.write_str(" OVER ")?;
            over.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for JsonConstructorExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            JsonConstructorKind::Json => "JSON(",
            JsonConstructorKind::Scalar => "JSON_SCALAR(",
            JsonConstructorKind::Serialize => "JSON_SERIALIZE(",
        })?;
        self.value.render(ctx, f)?;
        render_json_unique(self.unique_keys, f)?;
        if let Some(returning) = &self.returning {
            f.write_str(" ")?;
            returning.render(ctx, f)?;
        }
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for IsJsonExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let full = ctx.mode() == RenderMode::Parenthesized;
        open_group(full, f)?;
        render_predicate_operand(
            &self.expr,
            ctx.target().binding_powers.predicate(),
            Side::Left,
            ctx,
            f,
        )?;
        f.write_str(if self.negated {
            " IS NOT JSON"
        } else {
            " IS JSON"
        })?;
        f.write_str(match self.item_type {
            JsonItemType::Any => "",
            JsonItemType::Value => " VALUE",
            JsonItemType::Array => " ARRAY",
            JsonItemType::Object => " OBJECT",
            JsonItemType::Scalar => " SCALAR",
        })?;
        if self.unique_keys {
            f.write_str(" WITH UNIQUE KEYS")?;
        }
        close_group(full, f)
    }
}

// ---------------------------------------------------------------------------
// SQL/XML expression functions (pg-xml-expression-functions)
// ---------------------------------------------------------------------------

impl Render for XmlDocumentOrContent {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            XmlDocumentOrContent::Document => "DOCUMENT",
            XmlDocumentOrContent::Content => "CONTENT",
        })
    }
}

impl<X: Extension + Render> Render for XmlAttribute<X> {
    /// Renders `<value> [AS <name>]` — one `xmlattributes` / `xmlforest` element.
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.render(ctx, f)?;
        if let Some(name) = &self.name {
            f.write_str(" AS ")?;
            name.render(ctx, f)?;
        }
        Ok(())
    }
}

/// Render an optional `xmlexists` passing mechanism: ` BY REF` / ` BY VALUE`.
fn render_xml_passing_mechanism(
    mechanism: &Option<XmlPassingMechanism>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match mechanism {
        Some(XmlPassingMechanism::ByRef) => f.write_str(" BY REF"),
        Some(XmlPassingMechanism::ByValue) => f.write_str(" BY VALUE"),
        None => Ok(()),
    }
}

impl<X: Extension + Render> Render for XmlFunc<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XmlFunc::Element {
                name,
                attributes,
                content,
                ..
            } => {
                f.write_str("xmlelement(NAME ")?;
                name.render(ctx, f)?;
                if !attributes.is_empty() {
                    f.write_str(", xmlattributes(")?;
                    render_comma_separated(attributes, ctx, f)?;
                    f.write_str(")")?;
                }
                if !content.is_empty() {
                    f.write_str(", ")?;
                    render_comma_separated(content, ctx, f)?;
                }
                f.write_str(")")
            }
            XmlFunc::Forest { elements, .. } => {
                f.write_str("xmlforest(")?;
                render_comma_separated(elements, ctx, f)?;
                f.write_str(")")
            }
            XmlFunc::Concat { args, .. } => {
                f.write_str("xmlconcat(")?;
                render_comma_separated(args, ctx, f)?;
                f.write_str(")")
            }
            XmlFunc::Parse {
                option,
                arg,
                whitespace,
                ..
            } => {
                f.write_str("xmlparse(")?;
                option.render(ctx, f)?;
                f.write_str(" ")?;
                arg.render(ctx, f)?;
                match whitespace {
                    XmlWhitespaceOption::Unspecified => {}
                    XmlWhitespaceOption::Preserve => f.write_str(" PRESERVE WHITESPACE")?,
                    XmlWhitespaceOption::Strip => f.write_str(" STRIP WHITESPACE")?,
                }
                f.write_str(")")
            }
            XmlFunc::Pi { name, content, .. } => {
                f.write_str("xmlpi(NAME ")?;
                name.render(ctx, f)?;
                if let Some(content) = content {
                    f.write_str(", ")?;
                    content.render(ctx, f)?;
                }
                f.write_str(")")
            }
            XmlFunc::Root {
                arg,
                version,
                standalone,
                ..
            } => {
                f.write_str("xmlroot(")?;
                arg.render(ctx, f)?;
                f.write_str(", VERSION ")?;
                match version {
                    Some(expr) => expr.render(ctx, f)?,
                    None => f.write_str("NO VALUE")?,
                }
                match standalone {
                    XmlStandalone::Unspecified => {}
                    XmlStandalone::Yes => f.write_str(", STANDALONE YES")?,
                    XmlStandalone::No => f.write_str(", STANDALONE NO")?,
                    XmlStandalone::NoValue => f.write_str(", STANDALONE NO VALUE")?,
                }
                f.write_str(")")
            }
            XmlFunc::Serialize {
                option,
                arg,
                data_type,
                indent,
                ..
            } => {
                f.write_str("xmlserialize(")?;
                option.render(ctx, f)?;
                f.write_str(" ")?;
                arg.render(ctx, f)?;
                f.write_str(" AS ")?;
                data_type.render(ctx, f)?;
                match indent {
                    XmlIndentOption::Unspecified => {}
                    XmlIndentOption::Indent => f.write_str(" INDENT")?,
                    XmlIndentOption::NoIndent => f.write_str(" NO INDENT")?,
                }
                f.write_str(")")
            }
            XmlFunc::Exists {
                path,
                mechanism_before,
                arg,
                mechanism_after,
                ..
            } => {
                f.write_str("xmlexists(")?;
                path.render(ctx, f)?;
                f.write_str(" PASSING")?;
                render_xml_passing_mechanism(mechanism_before, f)?;
                f.write_str(" ")?;
                arg.render(ctx, f)?;
                render_xml_passing_mechanism(mechanism_after, f)?;
                f.write_str(")")
            }
        }
    }
}

impl<X: Extension + Render> Render for StringFunc<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // The reversed `FOR … FROM …` source order folds onto the same fields, so
            // the render is canonically `FROM`-first; a `FOR`-only form keeps its bare
            // `FOR` (there is no `FROM` operand to write).
            StringFunc::Substring {
                expr, start, count, ..
            } => {
                f.write_str("SUBSTRING(")?;
                expr.render(ctx, f)?;
                if let Some(start) = start {
                    f.write_str(" FROM ")?;
                    start.render(ctx, f)?;
                }
                if let Some(count) = count {
                    f.write_str(" FOR ")?;
                    count.render(ctx, f)?;
                }
                f.write_str(")")
            }
            StringFunc::SubstringSimilar {
                expr,
                pattern,
                escape,
                ..
            } => {
                f.write_str("SUBSTRING(")?;
                expr.render(ctx, f)?;
                f.write_str(" SIMILAR ")?;
                pattern.render(ctx, f)?;
                f.write_str(" ESCAPE ")?;
                escape.render(ctx, f)?;
                f.write_str(")")
            }
            StringFunc::Position { substr, string, .. } => {
                f.write_str("POSITION(")?;
                substr.render(ctx, f)?;
                f.write_str(" IN ")?;
                string.render(ctx, f)?;
                f.write_str(")")
            }
            StringFunc::Overlay {
                target,
                replacement,
                start,
                count,
                ..
            } => {
                f.write_str("OVERLAY(")?;
                target.render(ctx, f)?;
                f.write_str(" PLACING ")?;
                replacement.render(ctx, f)?;
                f.write_str(" FROM ")?;
                start.render(ctx, f)?;
                if let Some(count) = count {
                    f.write_str(" FOR ")?;
                    count.render(ctx, f)?;
                }
                f.write_str(")")
            }
            StringFunc::Trim {
                side,
                trim_chars,
                from,
                sources,
                ..
            } => {
                f.write_str("TRIM(")?;
                if let Some(side) = side {
                    side.render(ctx, f)?;
                    f.write_str(" ")?;
                }
                if let Some(trim_chars) = trim_chars {
                    trim_chars.render(ctx, f)?;
                    f.write_str(" ")?;
                }
                if *from {
                    f.write_str("FROM ")?;
                }
                render_comma_separated(sources, ctx, f)?;
                f.write_str(")")
            }
            StringFunc::CollationFor { expr, .. } => {
                f.write_str("COLLATION FOR (")?;
                expr.render(ctx, f)?;
                f.write_str(")")
            }
            StringFunc::ConvertUsing { expr, charset, .. } => {
                f.write_str("CONVERT(")?;
                expr.render(ctx, f)?;
                f.write_str(" USING ")?;
                charset.render(ctx, f)?;
                f.write_str(")")
            }
            StringFunc::MatchAgainst {
                columns,
                against,
                modifier,
                ..
            } => {
                f.write_str("MATCH(")?;
                render_comma_separated(columns, ctx, f)?;
                f.write_str(") AGAINST(")?;
                against.render(ctx, f)?;
                if let Some(modifier) = modifier {
                    f.write_str(" ")?;
                    modifier.render(ctx, f)?;
                }
                f.write_str(")")
            }
            StringFunc::CeilTo {
                expr,
                field,
                spelling,
                ..
            } => {
                f.write_str(match spelling {
                    CeilSpelling::Ceil => "CEIL(",
                    CeilSpelling::Ceiling => "CEILING(",
                })?;
                expr.render(ctx, f)?;
                f.write_str(" TO ")?;
                field.render(ctx, f)?;
                f.write_str(")")
            }
            StringFunc::FloorTo { expr, field, .. } => {
                f.write_str("FLOOR(")?;
                expr.render(ctx, f)?;
                f.write_str(" TO ")?;
                field.render(ctx, f)?;
                f.write_str(")")
            }
        }
    }
}

impl Render for MatchSearchModifier {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MatchSearchModifier::NaturalLanguage => "IN NATURAL LANGUAGE MODE",
            MatchSearchModifier::NaturalLanguageQueryExpansion => {
                "IN NATURAL LANGUAGE MODE WITH QUERY EXPANSION"
            }
            MatchSearchModifier::Boolean => "IN BOOLEAN MODE",
            MatchSearchModifier::QueryExpansion => "WITH QUERY EXPANSION",
        })
    }
}

impl Render for TrimSide {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TrimSide::Both => "BOTH",
            TrimSide::Leading => "LEADING",
            TrimSide::Trailing => "TRAILING",
        })
    }
}

impl<X: Extension + Render> Render for CaseExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CASE")?;
        if let Some(operand) = &self.operand {
            f.write_str(" ")?;
            operand.render(ctx, f)?;
        }
        for clause in &self.when_clauses {
            f.write_str(" WHEN ")?;
            clause.condition.render(ctx, f)?;
            f.write_str(" THEN ")?;
            clause.result.render(ctx, f)?;
        }
        if let Some(else_result) = &self.else_result {
            f.write_str(" ELSE ")?;
            else_result.render(ctx, f)?;
        }
        f.write_str(" END")
    }
}

impl<X: Extension + Render> Render for FunctionCall<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        f.write_str("(")?;
        if let Some(quantifier) = &self.quantifier {
            quantifier.render(ctx, f)?;
            f.write_str(" ")?;
        }
        if self.wildcard {
            f.write_str("*")?;
        } else {
            render_comma_separated(&self.args, ctx, f)?;
        }
        if !self.order_by.is_empty() {
            // DuckDB's standalone form (`rank(ORDER BY x)`) has no positional argument, so
            // the `ORDER BY` opens the argument list — no separating space after `(`. A
            // preceding wildcard or argument list takes the space; a quantifier already
            // rendered its own trailing space.
            if self.wildcard || !self.args.is_empty() {
                f.write_str(" ")?;
            }
            f.write_str("ORDER BY ")?;
            render_comma_separated(&self.order_by, ctx, f)?;
        }
        // The MySQL `GROUP_CONCAT` delimiter rides inside the parentheses, after any
        // in-parenthesis `ORDER BY`, matching the source order it was parsed in.
        if let Some(separator) = &self.separator {
            f.write_str(" SEPARATOR ")?;
            separator.render(ctx, f)?;
        }
        // DuckDB's `IGNORE NULLS` / `RESPECT NULLS` null-treatment rides inside the
        // parentheses, after any in-parenthesis `ORDER BY` — the position the engine
        // accepts (the standard's post-`)` spelling engine-rejects there).
        if let Some(null_treatment) = &self.null_treatment {
            f.write_str(" ")?;
            null_treatment.render(ctx, f)?;
        }
        f.write_str(")")?;
        // MySQL's window-function post-`)` tail rides after the closing parenthesis and
        // before `OVER` (and the aggregate clauses MySQL leaves off), in the fixed
        // `FROM {FIRST | LAST}` then null-treatment order the grammar admits.
        if let Some(tail) = &self.window_tail {
            if let Some(from_first_last) = tail.from_first_last {
                f.write_str(match from_first_last {
                    FromFirstLast::First => " FROM FIRST",
                    FromFirstLast::Last => " FROM LAST",
                })?;
            }
            if let Some(null_treatment) = &tail.null_treatment {
                f.write_str(" ")?;
                null_treatment.render(ctx, f)?;
            }
        }
        // WITHIN GROUP precedes FILTER and OVER, matching PostgreSQL's
        // `func_application within_group_clause filter_clause over_clause` order.
        if let Some(within_group) = &self.within_group {
            f.write_str(" WITHIN GROUP (ORDER BY ")?;
            render_comma_separated(within_group, ctx, f)?;
            f.write_str(")")?;
        }
        if let Some(filter) = &self.filter {
            // DuckDB round-trips the keyword-less `FILTER (<predicate>)` spelling; every
            // other dialect wrote (and re-renders) the standard `WHERE`.
            match self.filter_where {
                FilterWhereSpelling::Where => f.write_str(" FILTER (WHERE ")?,
                FilterWhereSpelling::Omitted => f.write_str(" FILTER (")?,
            }
            filter.render(ctx, f)?;
            f.write_str(")")?;
        }
        if let Some(over) = &self.over {
            f.write_str(" OVER ")?;
            over.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for FunctionArg<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The `VARIADIC` array-spread marker prefixes the whole argument, ahead of any
        // named-argument arrow (`VARIADIC name => value`). It is a structural keyword, so
        // it is emitted in both the plain and the redacted fingerprint modes.
        if self.variadic {
            f.write_str("VARIADIC ")?;
        }
        // A named argument prints its `name` and the arrow the source wrote; a
        // positional argument carries no name and prints just the value. The name is
        // an identifier, so it is masked like one for the redacted fingerprint.
        if let Some(name) = self.name {
            if ctx.mode() == RenderMode::Redacted {
                f.write_str("id")?;
            } else {
                f.write_str(ctx.resolve(name))?;
            }
            f.write_str(match self.syntax {
                ArgSyntax::ColonEquals => " := ",
                // A named argument is never `Positional`; the current `=>` is the
                // canonical spelling for the otherwise-unreachable case.
                ArgSyntax::Arrow | ArgSyntax::Positional => " => ",
            })?;
        }
        self.value.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for StructField<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The key is a field name, so the redacted fingerprint masks it like an
        // identifier (collapsing the quote spelling, mirroring `Ident`); otherwise it
        // re-emits the source spelling, doubling the embedded delimiter exactly as
        // the lexer's doubled-close rule expects so the key round-trips.
        if ctx.mode() == RenderMode::Redacted {
            f.write_str("id")?;
        } else {
            let text = ctx.resolve(self.key);
            match self.key_spelling {
                StructKeySpelling::Bare => f.write_str(text)?,
                StructKeySpelling::SingleQuoted => {
                    if text.contains('\'') {
                        write!(f, "'{}'", text.replace('\'', "''"))?;
                    } else {
                        write!(f, "'{text}'")?;
                    }
                }
                StructKeySpelling::DoubleQuoted => {
                    if text.contains('"') {
                        write!(f, "\"{}\"", text.replace('"', "\"\""))?;
                    } else {
                        write!(f, "\"{text}\"")?;
                    }
                }
            }
        }
        f.write_str(": ")?;
        self.value.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for StructConstructorField<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.name {
            name.render(ctx, f)?;
            f.write_str(" ")?;
        }
        self.ty.render(ctx, f)
    }
}

impl<X: Extension + Render> Render for StructConstructorArg<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.render(ctx, f)?;
        if let Some(alias) = &self.alias {
            f.write_str(" AS ")?;
            alias.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for WindowSpec<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowSpec::Named { name, .. } => name.render(ctx, f),
            WindowSpec::Inline { definition, .. } => {
                f.write_str("(")?;
                definition.render(ctx, f)?;
                f.write_str(")")
            }
        }
    }
}

impl<X: Extension + Render> Render for WindowDefinition<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut wrote = false;
        if let Some(existing) = &self.existing {
            existing.render(ctx, f)?;
            wrote = true;
        }
        if !self.partition_by.is_empty() {
            if wrote {
                f.write_str(" ")?;
            }
            f.write_str("PARTITION BY ")?;
            render_comma_separated(&self.partition_by, ctx, f)?;
            wrote = true;
        }
        if !self.order_by.is_empty() {
            if wrote {
                f.write_str(" ")?;
            }
            f.write_str("ORDER BY ")?;
            render_comma_separated(&self.order_by, ctx, f)?;
            wrote = true;
        }
        if let Some(frame) = &self.frame {
            if wrote {
                f.write_str(" ")?;
            }
            frame.render(ctx, f)?;
        }
        Ok(())
    }
}

impl<X: Extension + Render> Render for WindowFrame<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.units.render(ctx, f)?;
        f.write_str(" ")?;
        if let Some(end) = &self.end {
            f.write_str("BETWEEN ")?;
            self.start.render(ctx, f)?;
            f.write_str(" AND ")?;
            end.render(ctx, f)?;
        } else {
            self.start.render(ctx, f)?;
        }
        if let Some(exclusion) = &self.exclusion {
            f.write_str(" EXCLUDE ")?;
            exclusion.render(ctx, f)?;
        }
        Ok(())
    }
}

impl Render for WindowFrameUnits {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            WindowFrameUnits::Rows => "ROWS",
            WindowFrameUnits::Range => "RANGE",
            WindowFrameUnits::Groups => "GROUPS",
        })
    }
}

impl<X: Extension + Render> Render for WindowFrameBound<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowFrameBound::CurrentRow { .. } => f.write_str("CURRENT ROW"),
            WindowFrameBound::UnboundedPreceding { .. } => f.write_str("UNBOUNDED PRECEDING"),
            WindowFrameBound::UnboundedFollowing { .. } => f.write_str("UNBOUNDED FOLLOWING"),
            WindowFrameBound::Preceding { offset, .. } => {
                offset.render(ctx, f)?;
                f.write_str(" PRECEDING")
            }
            WindowFrameBound::Following { offset, .. } => {
                offset.render(ctx, f)?;
                f.write_str(" FOLLOWING")
            }
        }
    }
}

impl Render for WindowFrameExclusion {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            WindowFrameExclusion::CurrentRow => "CURRENT ROW",
            WindowFrameExclusion::Group => "GROUP",
            WindowFrameExclusion::Ties => "TIES",
            WindowFrameExclusion::NoOthers => "NO OTHERS",
        })
    }
}

impl<X: Extension + Render> Render for NamedWindow<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        f.write_str(" AS (")?;
        self.definition.render(ctx, f)?;
        f.write_str(")")
    }
}

impl<X: Extension + Render> Render for OrderByExpr<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.expr.render(ctx, f)?;
        // `USING <operator>` (PostgreSQL) is mutually exclusive with `ASC`/`DESC`, so
        // `asc` is `None` here and `render_sort_direction` emits only the `NULLS`
        // suffix that may still follow.
        if let Some(using) = &self.using {
            f.write_str(" USING ")?;
            using.render(ctx, f)?;
        }
        render_sort_direction(self.asc, self.nulls_first, f)
    }
}

impl Render for OrderByUsing {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A schema-qualified operator must round-trip through the explicit
        // `OPERATOR(schema.op)` spelling; a bare operator renders symbolically
        // (which also canonicalizes an unqualified `OPERATOR(<)` to bare `<`,
        // the same operator).
        let Some(schema) = &self.schema else {
            return f.write_str(ctx.resolve(self.op));
        };
        f.write_str("OPERATOR(")?;
        for part in &schema.0 {
            part.render(ctx, f)?;
            f.write_str(".")?;
        }
        f.write_str(ctx.resolve(self.op))?;
        f.write_str(")")
    }
}

impl Render for OrderByAll {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ALL")?;
        render_sort_direction(self.asc, self.nulls_first, f)
    }
}

impl<X: Extension + Render> Render for Limit<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.syntax {
            // MySQL/SQLite `LIMIT <offset>, <count>`: a source-fidelity render replays
            // the comma spelling; a re-spell and the redacted fingerprint fall through
            // to the canonical `LIMIT <count> OFFSET <offset>` below.
            LimitSyntax::CommaOffset if honours_source_spelling(ctx) => {
                f.write_str("LIMIT ")?;
                if let Some(offset) = &self.offset {
                    offset.render(ctx, f)?;
                }
                f.write_str(", ")?;
                if let Some(limit) = &self.limit {
                    limit.render(ctx, f)?;
                }
                Ok(())
            }
            LimitSyntax::LimitOffset | LimitSyntax::CommaOffset => {
                let mut wrote = false;
                if let Some(limit) = &self.limit {
                    f.write_str("LIMIT ")?;
                    // DuckDB percentage limit: re-emit the written marker (`LIMIT 40 PERCENT`
                    // / `LIMIT 35%`). `None` is the ordinary row count. The `%` spelling
                    // renders with no separating space (`LIMIT 20 %` canonicalizes onto it).
                    // The marker reduces onto a multiplicative-or-tighter operand, so a
                    // count carrying a looser binary operator is re-parenthesized to reparse
                    // as the percentage count rather than re-associating with a `%` read as
                    // modulo (`LIMIT (30-10)%`, not `LIMIT 30 - 10%`) — mirroring the parse
                    // threshold in `parse_limit_percent_operand` (`Side::Right` of the
                    // multiplicative rank). A `PERCENT`-keyword count is always a literal, so
                    // the wrap is a no-op there.
                    match self.percent {
                        Some(marker) => {
                            render_pg_operand(
                                limit,
                                ctx.target().binding_powers.multiplicative,
                                Side::Right,
                                ctx,
                                f,
                            )?;
                            f.write_str(match marker {
                                LimitPercent::Symbol => "%",
                                LimitPercent::Keyword => " PERCENT",
                            })?;
                        }
                        None => limit.render(ctx, f)?,
                    }
                    wrote = true;
                }
                if let Some(offset) = &self.offset {
                    if wrote {
                        f.write_str(" ")?;
                    }
                    f.write_str("OFFSET ")?;
                    offset.render(ctx, f)?;
                }
                Ok(())
            }
            LimitSyntax::FetchFirst => {
                // `FIRST`/`NEXT` and `ROW`/`ROWS` are interchangeable surface noise: a
                // source-fidelity render replays the written pair (`fetch_spelling`), a
                // re-spell and the redacted fingerprint keep the canonical `FIRST` /
                // `ROWS`. The `OFFSET … ROWS` word always renders plural (its rare
                // singular form is not tagged — it never crossed the sweep).
                let fidelity = honours_source_spelling(ctx);
                let mut wrote = false;
                if let Some(offset) = &self.offset {
                    f.write_str("OFFSET ")?;
                    offset.render(ctx, f)?;
                    f.write_str(" ROWS")?;
                    wrote = true;
                }
                // `with_ties: Some(_)` is the signal that a `FETCH` tail was
                // actually written (see `Limit::with_ties`'s doc comment) — the
                // count itself is optional (`FETCH FIRST ROWS ONLY`), so `limit`
                // alone cannot carry that.
                if let Some(with_ties) = self.with_ties {
                    if wrote {
                        f.write_str(" ")?;
                    }
                    let spelling = if fidelity {
                        self.fetch_spelling
                    } else {
                        FetchSpelling::FirstRows
                    };
                    f.write_str(spelling.fetch_keyword())?;
                    if let Some(limit) = &self.limit {
                        f.write_str(" ")?;
                        limit.render(ctx, f)?;
                    }
                    f.write_str(spelling.row_word())?;
                    f.write_str(if with_ties { "WITH TIES" } else { "ONLY" })?;
                }
                Ok(())
            }
        }
    }
}

impl<X: Extension + Render> Render for LimitBy<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LIMIT ")?;
        self.limit.render(ctx, f)?;
        if let Some(offset) = &self.offset {
            f.write_str(" OFFSET ")?;
            offset.render(ctx, f)?;
        }
        f.write_str(" BY ")?;
        render_comma_separated(&self.by, ctx, f)
    }
}

impl<X: Extension + Render> Render for Setting<X> {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.render(ctx, f)?;
        f.write_str(" = ")?;
        self.value.render(ctx, f)
    }
}

impl Render for FormatClause {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FORMAT ")?;
        self.name.render(ctx, f)
    }
}

impl Render for ForClause {
    /// Renders the MSSQL `FOR XML`/`FOR JSON` tail with the directives in the canonical
    /// MSSQL order (parse accepts them order-independently). Callers write the leading
    /// space.
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForClause::Xml {
                mode,
                elements,
                binary_base64,
                typed,
                root,
                ..
            } => {
                f.write_str("FOR XML ")?;
                mode.render(ctx, f)?;
                if *binary_base64 {
                    f.write_str(", BINARY BASE64")?;
                }
                if *typed {
                    f.write_str(", TYPE")?;
                }
                if let Some(root) = root {
                    f.write_str(", ")?;
                    root.render(ctx, f)?;
                }
                if let Some(elements) = elements {
                    f.write_str(", ")?;
                    elements.render(ctx, f)?;
                }
                Ok(())
            }
            ForClause::Json {
                mode,
                root,
                include_null_values,
                without_array_wrapper,
                ..
            } => {
                f.write_str("FOR JSON ")?;
                mode.render(ctx, f)?;
                if let Some(root) = root {
                    f.write_str(", ")?;
                    root.render(ctx, f)?;
                }
                if *include_null_values {
                    f.write_str(", INCLUDE_NULL_VALUES")?;
                }
                if *without_array_wrapper {
                    f.write_str(", WITHOUT_ARRAY_WRAPPER")?;
                }
                Ok(())
            }
        }
    }
}

impl Render for ForXmlMode {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForXmlMode::Raw { name, .. } => {
                f.write_str("RAW")?;
                render_optional_for_name(name.as_ref(), ctx, f)
            }
            ForXmlMode::Auto { .. } => f.write_str("AUTO"),
            ForXmlMode::Explicit { .. } => f.write_str("EXPLICIT"),
            ForXmlMode::Path { name, .. } => {
                f.write_str("PATH")?;
                render_optional_for_name(name.as_ref(), ctx, f)
            }
        }
    }
}

impl Render for ForXmlElements {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ForXmlElements::Plain => "ELEMENTS",
            ForXmlElements::XsiNil => "ELEMENTS XSINIL",
            ForXmlElements::Absent => "ELEMENTS ABSENT",
        })
    }
}

impl Render for ForJsonMode {
    fn render(&self, _ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ForJsonMode::Auto => "AUTO",
            ForJsonMode::Path => "PATH",
        })
    }
}

impl Render for ForRoot {
    fn render(&self, ctx: &RenderCtx<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ROOT")?;
        render_optional_for_name(self.name.as_ref(), ctx, f)
    }
}

/// Render an optional `('name')` element/root name (`RAW`/`PATH`/`ROOT`); a no-op when
/// the name is absent.
fn render_optional_for_name(
    name: Option<&Literal>,
    ctx: &RenderCtx<'_>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    if let Some(name) = name {
        f.write_str("(")?;
        name.render(ctx, f)?;
        f.write_str(")")?;
    }
    Ok(())
}
