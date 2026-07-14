// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Emit generated node size budget assertions.
//!
//! Every spanned AST node (`schema.spanned`: the struct/enum nodes that carry
//! `meta`, plus `ObjectName`) gets a compile-time `size_of` budget. The set is
//! derived from the schema, not hand-picked, and [`validate_coverage`] fails
//! generation if any spanned node lacks a budget — so a *new* node cannot slip
//! past the size gate, and coverage stays deliberate by construction (ADR-0007,
//! ADR-0013).
//!
//! # Why per-node budgets, and why the values live here
//!
//! The generator reads the AST as *text* (ADR-0013) and never compiles it, so it
//! cannot measure `size_of` itself. Each budget below is therefore a measured
//! constant on the supported 64-bit host layout: the node's current
//! `size_of::<T<NoExt>>()`, pinned *exactly*. The
//! asserts are zero-cost (`const { assert!(size_of == BUDGET) }`) and fail the
//! build the instant a node's size changes *in either direction* — the layout
//! tripwire ADR-0007 calls for, independent of the runtime allocation pins
//! (`bench/tests/allocations.rs`, ADR-0016). The `==` (not `<=`) is deliberate: a
//! *ceiling* silently absorbs a shrink, so a later change can re-grow the node back
//! up to the stale ceiling with the tripwire never firing — the pin and the real
//! layout drift apart. The exact pin keeps this number equal to the measured size at
//! all times, so every layout change lands here first, with its own commit and its
//! matching allocation-pin re-baseline.
//!
//! # Budget change policy
//!
//! A budget must always equal the node's real `size_of`, so it moves — up *or*
//! down — on every deliberate layout change, and never otherwise. Before changing a
//! number here, decide which case you are in:
//!
//! - **Legitimate increase — bump the budget.** A node genuinely needs to carry a
//!   new field/variant the grammar requires (e.g. a new clause), and the few extra
//!   inline bytes are the accepted cost of the feature. The hot recursive nodes
//!   (`Expr`, `Statement`, `SetExpr`) get the most scrutiny here, because their
//!   width is paid by every node of that kind in the parsed tree; prefer boxing a
//!   cold payload (below) before widening a hot node, and expect the matching
//!   allocation pins to move too. Record *why* in the commit, not here.
//! - **Regression — fix the node, do not bump the budget.** An increase that is
//!   really a layout mistake: a cold/fat variant inlined where ADR-0007 says to
//!   `Box` it (re-enable the `clippy::large_enum_variant` intuition by hand); a
//!   `String` where an interned `Symbol` belongs (identifiers must intern,
//!   ADR-0003); a `Vec` where a one-word `ThinVec` belongs (ADR-0007's child-
//!   sequence container); or padding from poor field ordering. The fix is to the
//!   node, and the budget stays put.
//!
//! Either way the dance is the same: change the node, update its number here to the
//! new measured `size_of`, then `cargo run -p squonk-sourcegen` to regenerate
//! `generated/size_asserts.rs`. The exact pin fails the build on a shrink too, so the
//! number here is refreshed as part of the same change, never left stale above the
//! layout.
//!
//! # Box vs inline: which variant payloads to box (ADR-0007)
//!
//! The budgets above pin *how wide* each node is; this section is the policy for
//! *why* — when a fat enum variant's payload should be `Box`ed (paying one heap
//! allocation + an indirection to move it off the stack) versus kept inline. A Rust
//! enum is sized to its **largest** variant, so a single fat variant taxes every
//! instance of that enum — but that tax only matters in proportion to how often the
//! enum, and the fat variant, actually occur. The decision therefore rides on two
//! axes, never one: `size_of` (pinned above) **and** real-corpus *frequency*. The
//! `variant_frequency` example measures the second axis — it walks the vendored
//! conformance corpora with the generated `Visit` traversal and counts node and
//! variant occurrences — so this policy is measured, not guessed:
//! `cargo run -p squonk-bench --example variant_frequency`.
//!
//! **Box a variant's payload when** it is fat **and** either the enum is *hot* (paid
//! by many nodes — box to keep the common node lean) **or** a *rare* fat variant
//! skews an enum whose common variant is small (the common case otherwise pays the
//! fat variant's width for nothing). The skew case is the clearest win: when the fat
//! variant is both rare and large, the allocation is paid only on the rare path while
//! every common instance shrinks.
//!
//! **Leave a variant inline when** the payload is small (a box saves few bytes but
//! still costs an allocation, an indirection, and a cache miss), **or** the variant
//! is cold *and* there is no skew to exploit — all the enum's meaningful variants are
//! about the same width, so boxing one cannot shrink the enum — *and* the enum is
//! scanned contiguously (stored in a `ThinVec` the consumer iterates), where inline
//! storage keeps the walk cache-friendly.
//!
//! The concrete per-node `size_of` × frequency numbers behind these criteria are not
//! copied here: they drift with every layout change and are reproduced verbatim by the
//! deterministic `variant_frequency` example (fixed corpora + presets, `BTreeMap`
//! ordering, ADR-0016). Re-run it after a layout change rather than maintaining a
//! snapshot in this comment.

use crate::schema::{NodeItem, Schema, screaming_snake_case};

const HEADER: &str = "\
//! @generated by `squonk-sourcegen`; do not edit by hand.
//! Regenerate with `cargo run -p squonk-sourcegen`.

#![allow(clippy::all, dead_code, unused_imports)]

";

/// Exact `size_of::<T<NoExt>>()` pins for every spanned AST node, measured on the
/// stock layout. Keep this set equal to `schema.spanned` (the validators below
/// enforce equality in both directions). See the module docs for the policy that
/// governs when an entry may change. Alphabetical for stable, reviewable diffs.
const SIZE_BUDGETS: &[(&str, usize)] = &[
    ("AccessControlStatement", 104),
    ("AfterMatchSkip", 36),
    ("AggregateArgs", 32),
    ("AlterColumnAction", 56),
    ("AlterColumnTarget", 24),
    ("AlterDatabase", 68),
    ("AlterDatabaseAction", 32),
    ("AlterDatabaseOption", 40),
    ("AlterDatabaseOptions", 48),
    ("AlterEvent", 112),
    ("AlterExtension", 152),
    ("AlterExtensionAction", 120),
    ("AlterInstance", 48),
    ("AlterInstanceAction", 36),
    ("AlterLogfileGroup", 64),
    ("AlterObjectDepends", 144),
    ("AlterObjectSchema", 48),
    ("AlterResourceGroup", 104),
    ("AlterRoutine", 32),
    ("AlterSequence", 32),
    ("AlterSequenceOption", 72),
    ("AlterServer", 40),
    ("AlterSystem", 64),
    ("AlterSystemAction", 48),
    ("AlterTable", 32),
    ("AlterTableAction", 128),
    ("AlterTablespace", 80),
    ("AlterTablespaceAction", 48),
    ("AlterUser", 104),
    ("AlterUserSpec", 152),
    ("AlterView", 56),
    ("AnalyzeHistogram", 48),
    ("AnalyzeStatement", 32),
    ("ArrayExpr", 24),
    ("AtTimeZoneExpr", 96),
    ("AttachStatement", 80),
    ("AuthOption", 60),
    ("CacheIndexKeyList", 24),
    ("CacheIndexStatement", 120),
    ("CacheIndexTable", 48),
    ("CacheIndexTargets", 72),
    ("CallStatement", 32),
    ("CaseExpr", 40),
    ("CaseStatement", 40),
    ("ChangeReplicationSourceOption", 80),
    ("ChangeReplicationSourceOptionValue", 64),
    ("CharsetAnnotation", 36),
    ("CheckpointStatement", 36),
    ("CloseCursorStatement", 32),
    ("CollateExpr", 64),
    ("ColumnConstraint", 72),
    ("ColumnDef", 88),
    ("ColumnOption", 24),
    ("CommentOnStatement", 72),
    ("CompoundStatement", 72),
    ("ComprehensionSource", 24),
    ("ConditionalBranch", 64),
    ("ConditionInfoItem", 56),
    ("ConditionValue", 40),
    ("ConfigParameter", 24),
    ("ConflictAction", 64),
    ("ConflictTarget", 64),
    ("ConstraintCharacteristics", 16),
    ("ConstraintsTarget", 24),
    ("CopyIntoSource", 40),
    ("CopyIntoStatement", 104),
    ("CopyIntoTarget", 40),
    ("CopyOption", 72),
    ("CopyOptionValue", 40),
    ("CopySource", 32),
    ("CopyStatement", 160),
    ("CopyTarget", 40),
    ("CreateDatabase", 24),
    ("CreateColocationGroup", 72),
    ("CreateEvent", 104),
    ("CreateExtension", 48),
    ("CreateExtensionOption", 48),
    ("CreateFunction", 88),
    ("CreateIndex", 88),
    ("CreateLogfileGroup", 64),
    ("CreateMacro", 56),
    ("CreateProcedure", 56),
    ("CreateResourceGroup", 104),
    ("CreateSchema", 56),
    ("CreateSecret", 32),
    ("CreateServer", 64),
    ("CreateSequence", 32),
    ("CreateSpatialReferenceSystem", 48),
    ("CreateStoredTrigger", 112),
    ("CreateTable", 96),
    ("CreateTableBody", 40),
    ("CreateTableOption", 56),
    ("CreateTableOptionKind", 40),
    ("CreateTablespace", 88),
    ("CreateTrigger", 104),
    ("CreateType", 72),
    ("CreateTypeDefinition", 48),
    ("CreateUser", 112),
    ("CreateView", 72),
    ("CreateVirtualTable", 56),
    ("Cte", 96),
    ("CteBody", 24),
    ("CteCycleClause", 96),
    ("CteCycleMark", 32),
    ("CteSearchClause", 48),
    ("DataType", 32),
    ("DeallocateStatement", 36),
    ("Declaration", 96),
    ("DefaultRoleTarget", 24),
    ("DefaultValue", 12),
    ("Definer", 52),
    ("Delete", 296),
    ("DescribeColumn", 36),
    ("DescribeStatement", 64),
    ("DetachStatement", 36),
    ("DiagnosticsInfo", 32),
    ("DmlSelection", 56),
    ("DmlTarget", 48),
    ("DoArg", 48),
    ("DoExpressionsStatement", 24),
    ("DoStatement", 24),
    ("DropDatabase", 36),
    ("DropColocationGroup", 36),
    ("DropEvent", 24),
    ("DropIndexOnTable", 48),
    ("DropLogfileGroup", 40),
    ("DropResourceGroup", 36),
    ("DropSecretStmt", 56),
    ("DropServer", 36),
    ("DropSpatialReferenceSystem", 40),
    ("DropStatement", 24),
    ("DropTablespace", 48),
    ("DropTransform", 120),
    ("EventSchedule", 40),
    ("ExcludeConstraint", 88),
    ("ExcludeElement", 96),
    ("ExecuteStatement", 40),
    ("ExecuteUsingStatement", 40),
    ("ExplainOption", 52),
    ("ExplainStatement", 32),
    ("ExportStatement", 72),
    ("Expr", 40),
    ("ExtensionVersion", 36),
    ("ExtractExpr", 40),
    ("FetchCursorStatement", 48),
    ("FieldSelectionExpr", 88),
    ("FieldSelector", 32),
    ("FlushOption", 36),
    ("FlushStatement", 40),
    ("FlushTarget", 24),
    ("ForceSeekTarget", 40),
    ("ForeignKeyRef", 48),
    ("ForClause", 92),
    ("ForRoot", 36),
    ("ForXmlMode", 40),
    ("FormatClause", 32),
    ("FunctionArg", 64),
    ("FunctionBody", 40),
    ("FunctionCall", 120),
    ("FunctionOption", 56),
    ("FunctionParam", 80),
    ("FunctionParamDefault", 56),
    ("GeneratedColumn", 56),
    ("GetDiagnosticsStatement", 48),
    ("GrantAs", 88),
    ("GrantObject", 24),
    ("Grantee", 48),
    ("PrivilegeLevelObject", 48),
    ("PrivilegeLevel", 32),
    ("WithRoleSpec", 24),
    ("GroupByItem", 56),
    ("GroupReplicationOption", 40),
    ("HandlerCondition", 40),
    ("HandlerStatement", 224),
    ("HandlerOperation", 200),
    ("HandlerReadSelector", 48),
    ("CloneStatement", 156),
    ("CloneDataDirectory", 40),
    ("ImportTableStatement", 24),
    ("HelpStatement", 32),
    ("BinlogStatement", 36),
    ("HierarchicalClause", 96),
    ("Ident", 20),
    ("IdentityColumn", 24),
    ("IdentityOption", 56),
    ("IfStatement", 32),
    ("ImportStatement", 36),
    ("IndexColumn", 56),
    ("IndexHint", 24),
    ("IndexedBy", 32),
    ("Insert", 216),
    ("InsertSource", 32),
    ("InsertTarget", 56),
    ("InsertValue", 56),
    ("InsertValues", 24),
    ("InstanceLockStatement", 16),
    ("IntoTarget", 24),
    ("IsJsonExpr", 24),
    ("IterateStatement", 32),
    ("Join", 208),
    ("JoinConstraint", 56),
    ("JoinOperator", 72),
    ("JsonAggregateBody", 64),
    ("JsonAggregateExpr", 120),
    ("JsonArrayBody", 24),
    ("JsonArrayExpr", 64),
    ("JsonBehavior", 24),
    ("JsonConstructorExpr", 64),
    ("JsonFuncExpr", 128),
    ("JsonKeyValue", 48),
    ("JsonObjectExpr", 48),
    ("JsonPassingArg", 56),
    ("JsonReturning", 24),
    ("JsonTable", 112),
    ("JsonTableColumn", 104),
    ("JsonValueExpr", 24),
    ("KeyCacheName", 32),
    ("LambdaExpr", 64),
    ("LanguageName", 36),
    ("LateralView", 168),
    ("LeaveStatement", 32),
    ("KillStatement", 56),
    ("InstallStatement", 56),
    ("UninstallStatement", 32),
    ("InstallComponentSetElement", 80),
    ("InstallComponentSetValue", 56),
    ("Limit", 96),
    ("LimitBy", 104),
    ("ListComprehension", 64),
    ("Literal", 24),
    ("LoadDataEnclosed", 40),
    ("LoadDataFieldOrVar", 36),
    ("LoadDataFields", 104),
    ("LoadDataIgnoreRows", 40),
    ("LoadDataLines", 60),
    ("LoadDataStatement", 320),
    ("LoadIndexStatement", 88),
    ("LoadIndexTable", 48),
    ("LoadIndexTargets", 72),
    ("LoadStatement", 48),
    ("LoadTarget", 36),
    ("LockTablesStatement", 24),
    ("LockingClause", 24),
    ("LoopStatement", 64),
    ("MacroBody", 24),
    ("MacroParam", 72),
    ("MapEntry", 96),
    ("MapExpr", 24),
    ("MatchRecognize", 144),
    ("MatchRecognizePattern", 40),
    ("Measure", 72),
    ("Merge", 304),
    ("MergeAction", 40),
    ("MergeWhenClause", 96),
    ("ModuleArg", 16),
    ("AccountName", 52),
    ("NamedOperatorExpr", 112),
    ("NamedWindow", 144),
    ("ObjectName", 8),
    ("ObjectReference", 104),
    ("OnConflict", 144),
    ("OpenCursorStatement", 32),
    ("OpenJson", 40),
    ("OpenJsonColumn", 56),
    ("OperatorArgs", 80),
    ("OrderByAll", 16),
    ("OrderByExpr", 64),
    ("OrderByUsing", 24),
    ("PartitionBound", 64),
    ("PartitionElem", 72),
    ("PartitionSelection", 24),
    ("PartitionSpec", 24),
    ("PasswordLockOption", 40),
    ("PipeAggregateExpr", 80),
    ("PipeOperator", 64),
    ("PipeRenameItem", 52),
    ("Pivot", 104),
    ("PivotColumn", 88),
    ("PivotExpr", 80),
    ("PivotValueSource", 24),
    ("PostfixOperatorExpr", 56),
    ("PragmaStatement", 64),
    ("PrefixOperatorExpr", 56),
    ("PrepareFromStatement", 68),
    ("PrepareSource", 36),
    ("PrepareStatement", 48),
    ("Privilege", 40),
    ("Privileges", 24),
    ("PurgeStatement", 72),
    ("PurgeTarget", 56),
    ("Query", 232),
    ("ReferentialAction", 24),
    ("RefreshMaterializedView", 24),
    ("ReindexStatement", 24),
    ("RenameStatement", 24),
    ("ResourceLimit", 40),
    ("Returning", 24),
    ("RepeatStatement", 72),
    ("ReplicaThreadOption", 16),
    ("ReplicaUntilCondition", 40),
    ("ReplicationFilterRule", 24),
    ("ReplicationStatement", 152),
    ("ResourceGroupThreadPriority", 40),
    ("ResourceGroupVcpu", 24),
    ("ReturnStatement", 56),
    ("RewriteDbPair", 52),
    ("RoleSpec", 32),
    ("RoutineSignature", 32),
    ("RowExpr", 24),
    ("RowsFromItem", 144),
    ("SampleClause", 72),
    ("SecretOption", 72),
    ("Select", 192),
    ("SelectDistinct", 24),
    ("SelectItem", 80),
    ("SemiStructuredAccessExpr", 64),
    ("SemiStructuredPathSegment", 32),
    ("ServerOption", 40),
    ("SessionStatement", 48),
    ("SetCharacterSetValue", 56),
    ("SetExpr", 32),
    ("SetNamesValue", 72),
    ("SetParameterValue", 40),
    ("SetValue", 24),
    ("SetVariableAssignment", 48),
    ("SetVariableValue", 24),
    ("Setting", 72),
    ("ShowFilter", 56),
    ("ShowFrom", 24),
    ("ShowFunctionsFilter", 40),
    ("ShowLimit", 64),
    ("ShowRef", 40),
    ("ShowRefTarget", 24),
    ("ShowStatement", 168),
    ("ShowTarget", 152),
    ("SignalItem", 56),
    ("SignalStatement", 64),
    ("SizeLiteral", 16),
    ("SpecialSetValue", 56),
    ("SrsAttribute", 60),
    ("Statement", 24),
    ("StatementInfoItem", 56),
    ("StructConstructorArg", 72),
    ("StructConstructorExpr", 32),
    ("StructConstructorField", 64),
    ("StructExpr", 24),
    ("StructField", 64),
    ("StructTypeField", 64),
    ("SubscriptExpr", 176),
    ("SubsetDefinition", 40),
    ("SymbolDefinition", 72),
    ("TableAlias", 48),
    ("StringFunc", 48),
    ("TableConstraint", 32),
    ("TableConstraintDef", 80),
    ("TableElement", 104),
    ("TableFactor", 120),
    ("TableFunctionColumn", 64),
    ("TableHint", 56),
    ("TableLikeOption", 16),
    ("TableLock", 48),
    ("TableMaintenanceKind", 64),
    ("TableMaintenanceStatement", 88),
    ("TableOption", 72),
    ("TableOptionValue", 40),
    ("TableRename", 32),
    ("TableSample", 40),
    ("TableStorageParameter", 64),
    ("TableVersion", 32),
    ("TableWithJoins", 144),
    ("TablespaceOption", 40),
    ("TlsOption", 40),
    ("TlsRequirement", 24),
    ("TransactionMode", 16),
    ("TransactionStatement", 40),
    ("TriggerEvent", 24),
    ("TriggerOrder", 36),
    ("UnlockTablesStatement", 16),
    ("Unpivot", 96),
    ("UnpivotColumn", 48),
    ("Update", 296),
    ("UpdateAssignment", 80),
    ("UpdateExtensionsStatement", 24),
    ("UpdateTupleSource", 32),
    ("UpdateValue", 56),
    ("Upsert", 160),
    ("UseStatement", 24),
    ("UserAttribute", 40),
    ("UserRename", 116),
    ("UserRoleList", 24),
    ("UserSpec", 124),
    ("VacuumStatement", 96),
    ("Values", 24),
    ("ValuesItem", 56),
    ("VcpuRange", 60),
    ("WhenClause", 96),
    ("WhileStatement", 72),
    ("WildcardOptions", 40),
    ("WildcardRename", 40),
    ("WildcardReplace", 72),
    ("WindowDefinition", 112),
    ("WindowFrame", 64),
    ("WindowFrameBound", 24),
    ("WindowSpec", 32),
    ("With", 24),
    ("XaStatement", 100),
    ("Xid", 84),
    ("XmlAttribute", 40),
    ("XmlFunc", 48),
    ("XmlNamespace", 40),
    ("XmlTable", 48),
    ("XmlTableColumn", 64),
];

pub(crate) fn render(schema: &Schema) -> String {
    validate_budgets(schema);
    validate_coverage(schema);

    let mut out = crate::license_header::block(crate::license_header::Comment::Slash);
    out.push_str(HEADER);
    out.push_str("use crate::ast::*;\n");
    out.push_str("use std::mem::size_of;\n\n");

    for (name, budget) in SIZE_BUDGETS {
        out.push_str(&format!(
            "pub(crate) const {}: usize = {budget};\n",
            budget_name(name),
        ));
    }

    out.push_str("\n#[cfg(target_pointer_width = \"64\")]\n");
    out.push_str("const _: () = {\n");
    for (name, _) in SIZE_BUDGETS {
        out.push_str(&format!(
            "    assert!(size_of::<{}>() == {});\n",
            node_type(schema, name),
            budget_name(name),
        ));
    }
    out.push_str("};\n");
    out
}

/// The spanned type spelled for a `size_of` (with `<NoExt>` when it is generic).
fn node_type(schema: &Schema, name: &str) -> String {
    if find_node(schema, name).generics().params.is_empty() {
        name.to_string()
    } else {
        format!("{name}<NoExt>")
    }
}

/// Every budget names a real spanned AST node (catches a stale/misspelled entry
/// left behind when a node is renamed or removed).
fn validate_budgets(schema: &Schema) {
    for (name, _) in SIZE_BUDGETS {
        assert!(
            schema.is_spanned(name),
            "size budget configured for `{name}`, but it is not a spanned AST node",
        );
    }
}

/// Every spanned AST node has a budget. This is the completeness gate: adding a
/// node (a struct with `meta`, or an enum with `meta` on every variant) without a
/// budget fails generation here, so no node can silently escape the size ratchet.
fn validate_coverage(schema: &Schema) {
    for name in &schema.spanned {
        assert!(
            SIZE_BUDGETS.iter().any(|(budget, _)| budget == name),
            "spanned AST node `{name}` has no size budget; measure \
             `size_of::<{name}<NoExt>>()` and add it to SIZE_BUDGETS in \
             crates/squonk-sourcegen/src/size_asserts.rs (see the increase policy there)",
        );
    }
}

fn find_node<'a>(schema: &'a Schema, name: &str) -> &'a NodeItem {
    schema
        .items
        .iter()
        .find(|item| item.name() == name)
        .unwrap_or_else(|| panic!("missing AST node `{name}`"))
}

fn budget_name(name: &str) -> String {
    format!("{}_SIZE_BUDGET", screaming_snake_case(name))
}
