// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Combination-lattice generative coverage (spike, `spike-combination-lattice-generative-coverage`).
//!
//! # The structural gap this closes
//!
//! The existing feature-combination instruments only sample the lattice at points that
//! are *one flip away* from a preset. [`LabeledCase`](super::labeled) baselines are
//! `POSTGRES` with each declared flag flipped **individually** (the falsely-required
//! check), and the proptest generative lanes ([`crate::properties`]) draw statements under
//! per-dialect *preset* `FeatureSet`s. Neither samples an adversarial **pair** of flags
//! held on together. Every known statement-head hazard — the `DO` both-on
//! mis-parse, the `PREPARE`/`EXECUTE`/`DEALLOCATE` incoherence, the account-grant route
//! deadening the extended-object grammar — lives at exactly such a pair, in the blind spot
//! between "one preset" and "one flip".
//!
//! This module is the prototype instrument for that blind spot: it enumerates the
//! statement-head gate family, samples every unordered pair over it, and property-checks
//! each pair the registries declare *valid* while asserting each pair they declare
//! *invalid* is genuinely registry-flagged.
//!
//! # 1. The statement-head gate family ([`HEAD_GATES`])
//!
//! A *statement-head gate* is a boolean [`FeatureSet`] flag that admits (or reshapes) a
//! statement dispatched on a leading keyword — the layer where two features can contend for
//! the same parser-position head. The family is assembled two ways, kept honest against
//! each other by [`head_gate_family_covers_ledger_claimants`]:
//!
//! - Every claimant named in the [`MULTI_CLAIMANT_STATEMENT_HEADS`] ledger — these are, by
//!   the ledger's own definition, flags two or more grammars claim at one head, so they are
//!   the adversarial core. The coverage test fails if a future ledger row introduces a
//!   claimant not in [`HEAD_GATES`], which forces the instrument to keep pace with the
//!   ledger rather than silently under-sampling.
//! - The leading-keyword statement gates of the statement-dispatch axes
//!   ([`UtilitySyntax`], [`ShowSyntax`], [`MaintenanceSyntax`], [`AccessControlSyntax`], plus
//!   the ledger's `SessionVariableSyntax`/`StatementDdlGates`/`IndexAlterSyntax` claimants),
//!   which broadens the pair space past the already-known contended heads so a *new* head
//!   contention can surface.
//!
//! Each gate carries `set`/`is_enabled` closures over [`FeatureDelta`] (mirroring the
//! `toggleable_features!` table in [`super::labeled`], but reaching flags that table omits
//! because they are not accept/reject discriminators — e.g. `do_expression_list`,
//! `variable_assignment`, `drop_database`). Reflection-free toggling is why the family is a
//! hand-maintained descriptor list rather than derived purely from the `&str` sub-flag
//! arrays: a name alone cannot flip a field.
//!
//! # 2. Pair sampling and space size
//!
//! Sampling is **exhaustive over unordered pairs** — `N·(N−1)/2` combinations for `N`
//! gates. Each pair's candidate is `FeatureSet::POSTGRES` with **both** flags forced on
//! (`POSTGRES` is the [`LabeledCase`](super::labeled) baseline, and it already enables one
//! side of each of these hazards — `do_statement`, `prepared_statements`,
//! `access_control_extended_objects` — so forcing the MySQL rival on rediscovers the hazard
//! without contrivance). Multiplied by a small probe corpus this is ~10⁴ parses — cheap
//! enough to run deterministically in one test, so no random generator (proptest/bolero) is
//! warranted; the space is small and fully enumerable.
//!
//! # 3. The validity gate — registries *before* the parser
//!
//! The three self-consistency registries ([`FeatureSet::lexical_conflict`],
//! [`FeatureSet::feature_dependencies`], [`FeatureSet::grammar_conflict`]) partition the
//! combinations the per-feature model cannot make independent. A candidate flagged by **any**
//! of them is an *invalid* generation target: it is **skipped** (never parsed) but
//! **asserted** to be registry-flagged. That assertion is itself coverage — it catches a
//! future pair that *should* be registered and is not (the registry would return `None`, the
//! stability property would then run and could panic or mis-parse). Checking the registries
//! *before* constructing the parser is load-bearing: the concurrent parse-entry
//! `debug_assert!`-all-registries lane makes parsing an invalid delta a panic, so a valid
//! generative lane must never hand the parser a flagged set. This module reads the registry
//! verdict off the `FeatureSet` and only parses the `None`-on-all-three residue.
//!
//! # 4. Properties checked per *valid* pair
//!
//! - **No panic.** Every probe is parsed under the candidate; the parser is panic-free by
//!   contract, so a panic (not an `Err`) is the bug this catches.
//! - **Parse→render→reparse stability.** Each probe that parses is rendered
//!   ([`RenderMode::Parenthesized`]) and reparsed under the same candidate; the two trees
//!   must be structurally equal through the shared test interner (the
//!   [`crate::shared_interner`] oracle, symbol-id independent).
//!
//! # 5. Ledger head-resolution as executable assertions
//!
//! [`ledger_head_resolution_is_consistent`] turns each [`MultiClaimantHead`] row into a
//! runtime check over the *all-claimants-on* candidate:
//!
//! - A **union** row ([`HeadResolution::MeceLookahead`] / [`HeadResolution::DispatchOrderUnion`])
//!   must be registry-*clean* (else ledger and registry disagree), and no claimant's own
//!   probe form may be **deadened** by enabling its siblings — the precise "route flag
//!   deadening a sibling" hazard, checked as `parses-with-one ⇒
//!   parses-with-all`.
//! - An **exclusion** row ([`HeadResolution::OneReadingExclusion`] / [`HeadResolution::Route`])
//!   forgoes a reading, so the all-on candidate must be **either** registry-flagged (the
//!   `DO`/`PREPARE`/`GRANT` hazards) **or** a documented ledger exclusion (`DROP`,
//!   whose two MySQL displacements are forgone with no registry variant).
//!
//! # 6. The follow-up lanes (the spike's productionization children, now landed)
//!
//! Each lane reuses the same registry-before-parse validity gate (§3) and the same no-panic +
//! render-reparse oracle (§4) via one shared [`probe_candidate`] step; only the candidate
//! generator differs. Every lane is deliberately runtime-bounded so the whole family stays an
//! every-build instrument (~1.7 s added across the five lanes, measured in debug):
//!
//! - **Triples** (`spike-lattice-followup-triples`, `head_gate_triple_scan` /
//!   `expr_gate_triple_scan`). `k` is capped at 3 — the known three-flag contentions (the
//!   `#`-trigger family, the three-claimant `LOAD` head) are triples, and no four-flag hazard
//!   is on record to justify the combinatorial step to `k=4`. Exhaustive `C(56,3) ≈ 27.7k`
//!   over the head family costs whole seconds per build, so the lane probes the exhaustive
//!   *adversarial core* (every triple within one ledger head's claimants, resp. within one
//!   shared-sigil family) plus a fixed-seed [`SplitMix64`] random sample of the remaining
//!   space (~2k head / ~1.5k expr triples per build, deterministic build-to-build); the rest
//!   of the space is the documented drop, and each run prints probed-vs-total.
//! - **The expression/operator/lexical-trigger axis**
//!   (`spike-lattice-followup-expression-axis`, [`EXPR_GATES`] + `expr_axis_pair_scan`). The
//!   family carries every boolean flag [`FeatureSet::lexical_conflict`] reads — so the
//!   registrable [`LexicalConflict`] pair space is sampled exhaustively, the lane's
//!   calibration target as [`GrammarConflict`] is the head lane's
//!   (`expr_lexical_conflicts_are_rediscovered`) — plus the non-lexical expression-grammar
//!   neighbours the head family skips (the interval trio, `numbered_question`,
//!   `national_strings`, the JSON arrows).
//! - **Value-carrying flags** (`spike-lattice-followup-value-carrying-flags`,
//!   `value_axis_scan`). The finite meaning enums ([`PipeOperator`], [`DoubleAmpersand`],
//!   [`CaretOperator`], [`KeywordOperators`]) are enumerated exhaustively — each variant is a
//!   lattice point — and crossed with every head-gate boolean, plus the full 72-point product
//!   over the four enums together. The unbounded axes are sampled at representative points:
//!   `versioned_comments` (`Option<u32>`) at the boundary-straddling
//!   [`VERSIONED_COMMENT_POINTS`], the binding-power / set-operation tables at the shipped
//!   preset tables (the axes' value spaces are infinite, so points-not-products is the
//!   documented bound).
//! - **Non-`POSTGRES` bases and flag-*off* interactions**
//!   (`spike-lattice-followup-nonpostgres-bases`, `nonpostgres_base_pair_scan`). The pair
//!   core reruns from all five preset bases (`POSTGRES`/`MYSQL`/`SQLITE`/`DUCKDB`/`ANSI`) in
//!   *both* directions — both-on and both-*off*, since a pair that only misbehaves when a
//!   third flag is off needs a base where it is off. Candidates identical to an
//!   already-probed feature set (notably the prototype's `POSTGRES`-on pairs, plus the many
//!   flips that are no-ops on a given base) are deduped by feature-set identity, which
//!   removes ~70% of the raw 15.4k candidate space and keeps the lane under a second.
//!
//! # 7. Remaining deliberate bounds
//!
//! - **`k = 3` cap and sampled (not exhaustive) triple space** — see the triples bullet
//!   above; raise the sample constants or add a nightly-tier exhaustive run if a triple-only
//!   hazard ever surfaces outside the adversarial cores.
//! - **Exact-form fidelity of the ledger probes.** A ledger probe that does not parse under
//!   its single-claimant config makes that row's non-deadening check *vacuous* rather than
//!   false (the implication is trivially satisfied); the meaningful-vs-vacuous split is
//!   reported, not asserted, so a wrong probe weakens coverage without a false alarm.
//! - **Triple/value lanes run from the `POSTGRES` base only.** The multi-base lane covers
//!   pairs; crossing five bases into the triple and value lanes multiplies runtime ~5× for
//!   spaces those lanes already sample, so base-variation is deliberately confined to the
//!   pair core where the known hazards live.

use super::harness::AdHocDialect;
use super::*;
use squonk::ast::dialect::TransactionSyntax;
use squonk::ast::dialect::{
    GrammarConflict, HeadResolution, LexicalConflict, MULTI_CLAIMANT_STATEMENT_HEADS,
};
use squonk_ast::render::RenderMode;

/// A toggleable boolean [`FeatureSet`] flag with the closures to read and flip it.
/// `set`/`is_enabled` are bare `fn` pointers (the closures capture nothing), so gate tables
/// are `const`. The same descriptor serves both the statement-head family ([`HEAD_GATES`],
/// the pair/triple lanes) and the expression/operator/lexical-trigger family
/// ([`EXPR_GATES`], the lexical-conflict lane); a "gate" is any boolean whose flip the lattice
/// samples.
struct Gate {
    /// The `FeatureSet` sub-flag name, spelled exactly as the struct field and (for head
    /// gates) the ledger `claimants` column so [`head_gate_family_covers_ledger_claimants`]
    /// can cross-check.
    name: &'static str,
    is_enabled: fn(&FeatureSet) -> bool,
    set: fn(&FeatureSet, bool) -> FeatureSet,
}

/// Define one `const` [`Gate`] for a boolean field inside a [`FeatureDelta`] axis struct: the
/// const name, the sub-flag name, the owning [`FeatureDelta`] axis method, the axis struct
/// type, and the boolean field. The `..f.$axis` spread copies every sibling field so exactly
/// one flag flips.
macro_rules! axis_gate {
    ($const:ident, $name:literal, $axis:ident, $ty:ident, $field:ident) => {
        const $const: Gate = Gate {
            name: $name,
            is_enabled: |f| f.$axis.$field,
            set: |f, on| {
                f.with(FeatureDelta::EMPTY.$axis($ty {
                    $field: on,
                    ..f.$axis
                }))
            },
        };
    };
}

/// Define one `const` [`Gate`] for a top-level scalar boolean [`FeatureSet`] field — one
/// reached by a same-named [`FeatureDelta`] setter rather than through an axis struct (e.g.
/// `hash_bitwise_xor`).
macro_rules! scalar_gate {
    ($const:ident, $name:literal, $field:ident) => {
        const $const: Gate = Gate {
            name: $name,
            is_enabled: |f| f.$field,
            set: |f, on| f.with(FeatureDelta::EMPTY.$field(on)),
        };
    };
}

/// Define a gate family: one `const` [`Gate`] per axis line, collected into the `$slice`.
macro_rules! gate_family {
    ($slice:ident : $(($const:ident, $name:literal, $axis:ident, $ty:ident, $field:ident)),+ $(,)?) => {
        $( axis_gate!($const, $name, $axis, $ty, $field); )+
        const $slice: &[&Gate] = &[ $(&$const),+ ];
    };
}

/// Define a gate family from axis lines plus pre-defined scalar gate consts (the `extras`),
/// collecting all into `$slice`. Used by [`EXPR_GATES`], whose family mixes axis-struct flags
/// with the top-level `hash_bitwise_xor` scalar gate.
macro_rules! gate_family_with_extras {
    ($slice:ident : [ $(($const:ident, $name:literal, $axis:ident, $ty:ident, $field:ident)),+ $(,)? ], extras: [ $($extra:ident),+ $(,)? ]) => {
        $( axis_gate!($const, $name, $axis, $ty, $field); )+
        const $slice: &[&Gate] = &[ $(&$const),+ , $(&$extra),+ ];
    };
}

gate_family! {
    HEAD_GATES:
    // --- MULTI_CLAIMANT_STATEMENT_HEADS claimants (the adversarial core) ---------------
    (DO_STATEMENT, "do_statement", utility_syntax, UtilitySyntax, do_statement),
    (DO_EXPRESSION_LIST, "do_expression_list", utility_syntax, UtilitySyntax, do_expression_list),
    (PREPARED_STATEMENTS, "prepared_statements", utility_syntax, UtilitySyntax, prepared_statements),
    (PREPARED_STATEMENTS_FROM, "prepared_statements_from", utility_syntax, UtilitySyntax, prepared_statements_from),
    (ACCESS_CONTROL_ACCOUNT_GRANTS, "access_control_account_grants", access_control_syntax, AccessControlSyntax, access_control_account_grants),
    (ACCESS_CONTROL_EXTENDED_OBJECTS, "access_control_extended_objects", access_control_syntax, AccessControlSyntax, access_control_extended_objects),
    (VARIABLE_ASSIGNMENT, "variable_assignment", session_variables, SessionVariableSyntax, variable_assignment),
    (LOCK_TABLES, "lock_tables", utility_syntax, UtilitySyntax, lock_tables),
    (LOCK_INSTANCE, "lock_instance", utility_syntax, UtilitySyntax, lock_instance),
    (LOAD_DATA, "load_data", utility_syntax, UtilitySyntax, load_data),
    (LOAD_EXTENSION, "load_extension", utility_syntax, UtilitySyntax, load_extension),
    (KEY_CACHE_STATEMENTS, "key_cache_statements", utility_syntax, UtilitySyntax, key_cache_statements),
    (ANALYZE, "analyze", maintenance_syntax, MaintenanceSyntax, analyze),
    (TABLE_MAINTENANCE, "table_maintenance", maintenance_syntax, MaintenanceSyntax, table_maintenance),
    (UPDATE_EXTENSIONS, "update_extensions", utility_syntax, UtilitySyntax, update_extensions),
    (VIEW_DEFINITION_OPTIONS, "view_definition_options", statement_ddl_gates, StatementDdlGates, view_definition_options),
    (ALTER_OBJECT_SET_SCHEMA, "alter_object_set_schema", statement_ddl_gates, StatementDdlGates, alter_object_set_schema),
    (ALTER_DATABASE, "alter_database", statement_ddl_gates, StatementDdlGates, alter_database),
    (ALTER_DATABASE_OPTIONS, "alter_database_options", statement_ddl_gates, StatementDdlGates, alter_database_options),
    (DROP_DATABASE, "drop_database", statement_ddl_gates, StatementDdlGates, drop_database),
    (INDEX_DROP_ON_TABLE, "index_drop_on_table", index_alter_syntax, IndexAlterSyntax, index_drop_on_table),
    (IMPORT_TABLE, "import_table", utility_syntax, UtilitySyntax, import_table),
    (EXPORT_IMPORT_DATABASE, "export_import_database", utility_syntax, UtilitySyntax, export_import_database),
    (VACUUM, "vacuum", maintenance_syntax, MaintenanceSyntax, vacuum),
    (VACUUM_ANALYZE, "vacuum_analyze", maintenance_syntax, MaintenanceSyntax, vacuum_analyze),

    // --- other leading-keyword statement gates (broaden the pair space) ----------------
    (COPY, "copy", utility_syntax, UtilitySyntax, copy),
    (COPY_INTO, "copy_into", utility_syntax, UtilitySyntax, copy_into),
    (COMMENT_ON, "comment_on", utility_syntax, UtilitySyntax, comment_on),
    (PRAGMA, "pragma", utility_syntax, UtilitySyntax, pragma),
    (ATTACH, "attach", utility_syntax, UtilitySyntax, attach),
    (KILL, "kill", utility_syntax, UtilitySyntax, kill),
    (HANDLER_STATEMENTS, "handler_statements", utility_syntax, UtilitySyntax, handler_statements),
    (SHUTDOWN, "shutdown", utility_syntax, UtilitySyntax, shutdown),
    (RESTART, "restart", utility_syntax, UtilitySyntax, restart),
    (CLONE, "clone", utility_syntax, UtilitySyntax, clone),
    (HELP_STATEMENT, "help_statement", utility_syntax, UtilitySyntax, help_statement),
    (BINLOG, "binlog", utility_syntax, UtilitySyntax, binlog),
    (USE_STATEMENT, "use_statement", utility_syntax, UtilitySyntax, use_statement),
    (CALL, "call", utility_syntax, UtilitySyntax, call),
    (RESET_SCOPE, "reset_scope", utility_syntax, UtilitySyntax, reset_scope),
    (START_TRANSACTION, "start_transaction", transaction_syntax, TransactionSyntax, start_transaction),
    (START_TRANSACTION_BLOCK_OPTIONAL, "start_transaction_block_optional", transaction_syntax, TransactionSyntax, start_transaction_block_optional),
    (TRANSACTION_WORK_KEYWORD, "transaction_work_keyword", transaction_syntax, TransactionSyntax, transaction_work_keyword),
    (BEGIN_TRANSACTION_KEYWORD, "begin_transaction_keyword", transaction_syntax, TransactionSyntax, begin_transaction_keyword),
    (COMMIT_TRANSACTION_KEYWORD, "commit_transaction_keyword", transaction_syntax, TransactionSyntax, commit_transaction_keyword),
    (ROLLBACK_TRANSACTION_KEYWORD, "rollback_transaction_keyword", transaction_syntax, TransactionSyntax, rollback_transaction_keyword),
    (BEGIN_TRANSACTION_MODES, "begin_transaction_modes", transaction_syntax, TransactionSyntax, begin_transaction_modes),
    (TRANSACTION_SAVEPOINTS, "transaction_savepoints", transaction_syntax, TransactionSyntax, transaction_savepoints),
    (SET_TRANSACTION, "set_transaction", transaction_syntax, TransactionSyntax, set_transaction),
    (TRANSACTION_ISOLATION_MODE, "transaction_isolation_mode", transaction_syntax, TransactionSyntax, transaction_isolation_mode),
    (TRANSACTION_ACCESS_MODE, "transaction_access_mode", transaction_syntax, TransactionSyntax, transaction_access_mode),
    (TRANSACTION_DEFERRABLE_MODE, "transaction_deferrable_mode", transaction_syntax, TransactionSyntax, transaction_deferrable_mode),
    (START_TRANSACTION_ISOLATION_MODE, "start_transaction_isolation_mode", transaction_syntax, TransactionSyntax, start_transaction_isolation_mode),
    (START_TRANSACTION_DEFERRABLE_MODE, "start_transaction_deferrable_mode", transaction_syntax, TransactionSyntax, start_transaction_deferrable_mode),
    (START_TRANSACTION_CONSISTENT_SNAPSHOT, "start_transaction_consistent_snapshot", transaction_syntax, TransactionSyntax, start_transaction_consistent_snapshot),
    (TRANSACTION_MULTIPLE_MODES, "transaction_multiple_modes", transaction_syntax, TransactionSyntax, transaction_multiple_modes),
    (TRANSACTION_MODES_REQUIRE_COMMAS, "transaction_modes_require_commas", transaction_syntax, TransactionSyntax, transaction_modes_require_commas),
    (TRANSACTION_MODES_REJECT_DUPLICATES, "transaction_modes_reject_duplicates", transaction_syntax, TransactionSyntax, transaction_modes_reject_duplicates),
    (ABORT_TRANSACTION_ALIAS, "abort_transaction_alias", transaction_syntax, TransactionSyntax, abort_transaction_alias),
    (END_TRANSACTION_ALIAS, "end_transaction_alias", transaction_syntax, TransactionSyntax, end_transaction_alias),
    (TRANSACTION_RELEASE, "transaction_release", transaction_syntax, TransactionSyntax, transaction_release),
    (TRANSACTION_CHAIN, "transaction_chain", transaction_syntax, TransactionSyntax, transaction_chain),
    (RELEASE_SAVEPOINT_KEYWORD_OPTIONAL, "release_savepoint_keyword_optional", transaction_syntax, TransactionSyntax, release_savepoint_keyword_optional),
    (BEGIN_TRANSACTION_MODE, "begin_transaction_mode", transaction_syntax, TransactionSyntax, begin_transaction_mode),
    (XA_TRANSACTIONS, "xa_transactions", transaction_syntax, TransactionSyntax, xa_transactions),
    (RENAME_STATEMENT, "rename_statement", utility_syntax, UtilitySyntax, rename_statement),
    (FLUSH, "flush", utility_syntax, UtilitySyntax, flush),
    (PURGE_BINARY_LOGS, "purge_binary_logs", utility_syntax, UtilitySyntax, purge_binary_logs),
    (REPLICATION_STATEMENTS, "replication_statements", utility_syntax, UtilitySyntax, replication_statements),
    (REINDEX, "reindex", maintenance_syntax, MaintenanceSyntax, reindex),
    (CHECKPOINT, "checkpoint", maintenance_syntax, MaintenanceSyntax, checkpoint),
    (ACCESS_CONTROL, "access_control", access_control_syntax, AccessControlSyntax, access_control),
    (USER_ROLE_MANAGEMENT, "user_role_management", access_control_syntax, AccessControlSyntax, user_role_management),
    (DESCRIBE, "describe", show_syntax, ShowSyntax, describe),
    (DESCRIBE_SUMMARIZE, "describe_summarize", show_syntax, ShowSyntax, describe_summarize),
    (SESSION_STATEMENTS, "session_statements", show_syntax, ShowSyntax, session_statements),
    (SHOW_TABLES, "show_tables", show_syntax, ShowSyntax, show_tables),
    (SHOW_COLUMNS, "show_columns", show_syntax, ShowSyntax, show_columns),
    (SHOW_CREATE_TABLE, "show_create_table", show_syntax, ShowSyntax, show_create_table),
    (SHOW_FUNCTIONS, "show_functions", show_syntax, ShowSyntax, show_functions),
}

// `hash_bitwise_xor` is a top-level scalar `FeatureSet` field, not an axis-struct flag, so it
// takes the scalar constructor; it joins [`EXPR_GATES`] as one of the three `#`-trigger
// claimants (with `line_comment_hash` and `positional_column`).
scalar_gate!(HASH_BITWISE_XOR, "hash_bitwise_xor", hash_bitwise_xor);

// The expression / operator / lexical-trigger gate family (`spike-lattice-followup-expression-axis`).
// Every boolean flag that `FeatureSet::lexical_conflict` reads is present, so the lexical-conflict
// lane samples every registrable `LexicalConflict` pair exhaustively — the family's calibration
// target, exactly as `GrammarConflict` is the head lane's. The family also carries neighbouring
// expression-grammar flags the head lane skips (the interval trio, `national_strings`,
// `numbered_question`, the JSON-arrow operators) so a non-lexical expression pair still gets the
// no-panic / render-reparse oracle. Value-carrying axes (`||`/`&&`/`^` meaning enums, identifier
// quotes, byte classes, binding powers) are not booleans and belong to the value lane
// (`spike-lattice-followup-value-carrying-flags`), not here.
gate_family_with_extras! {
    EXPR_GATES: [
        // --- string / numeric literal triggers ---------------------------------------------
        (DOUBLE_QUOTED_STRINGS, "double_quoted_strings", string_literals, StringLiteralSyntax, double_quoted_strings),
        (DOLLAR_QUOTED_STRINGS, "dollar_quoted_strings", string_literals, StringLiteralSyntax, dollar_quoted_strings),
        (NATIONAL_STRINGS, "national_strings", string_literals, StringLiteralSyntax, national_strings),
        (MONEY_LITERALS, "money_literals", numeric_literals, NumericLiteralSyntax, money_literals),
        // --- parameter sigils --------------------------------------------------------------
        (POSITIONAL_DOLLAR, "positional_dollar", parameters, ParameterSyntax, positional_dollar),
        (ANONYMOUS_QUESTION, "anonymous_question", parameters, ParameterSyntax, anonymous_question),
        (NUMBERED_QUESTION, "numbered_question", parameters, ParameterSyntax, numbered_question),
        (NAMED_COLON, "named_colon", parameters, ParameterSyntax, named_colon),
        (NAMED_AT, "named_at", parameters, ParameterSyntax, named_at),
        (NAMED_DOLLAR, "named_dollar", parameters, ParameterSyntax, named_dollar),
        // --- session-variable sigils -------------------------------------------------------
        (USER_VARIABLES, "user_variables", session_variables, SessionVariableSyntax, user_variables),
        (SYSTEM_VARIABLES, "system_variables", session_variables, SessionVariableSyntax, system_variables),
        // --- comment trigger ---------------------------------------------------------------
        (LINE_COMMENT_HASH, "line_comment_hash", comment_syntax, CommentSyntax, line_comment_hash),
        // --- expression-grammar `[` / `:` / `#` triggers -----------------------------------
        (SUBSCRIPT, "subscript", expression_syntax, ExpressionSyntax, subscript),
        (ARRAY_CONSTRUCTOR, "array_constructor", expression_syntax, ExpressionSyntax, array_constructor),
        (COLLECTION_LITERALS, "collection_literals", expression_syntax, ExpressionSyntax, collection_literals),
        (SEMI_STRUCTURED_ACCESS, "semi_structured_access", expression_syntax, ExpressionSyntax, semi_structured_access),
        (POSITIONAL_COLUMN, "positional_column", expression_syntax, ExpressionSyntax, positional_column),
        // --- expression-grammar neighbours the head lane skips (non-lexical) ---------------
        (RELAXED_INTERVAL_SYNTAX, "relaxed_interval_syntax", expression_syntax, ExpressionSyntax, relaxed_interval_syntax),
        (TYPED_INTERVAL_LITERAL, "typed_interval_literal", expression_syntax, ExpressionSyntax, typed_interval_literal),
        (MYSQL_INTERVAL_OPERATOR, "mysql_interval_operator", expression_syntax, ExpressionSyntax, mysql_interval_operator),
        // --- symbolic operator triggers ----------------------------------------------------
        (CONTAINMENT_OPERATORS, "containment_operators", operator_syntax, OperatorSyntax, containment_operators),
        (JSONB_OPERATORS, "jsonb_operators", operator_syntax, OperatorSyntax, jsonb_operators),
        (CUSTOM_OPERATORS, "custom_operators", operator_syntax, OperatorSyntax, custom_operators),
        (JSON_ARROW_OPERATORS, "json_arrow_operators", operator_syntax, OperatorSyntax, json_arrow_operators),
        // --- table-position `[` trigger ----------------------------------------------------
        (TABLE_JSON_PATH, "table_json_path", table_expressions, TableExpressionSyntax, table_json_path),
    ],
    extras: [HASH_BITWISE_XOR]
}

/// The probe corpus each valid pair is parsed against. Kept small (statement heads plus a
/// handful of generic forms) so the pair × corpus product stays ~10⁴. A probe need not parse
/// under bare `POSTGRES`; the no-panic property holds for a rejected parse too, and the
/// render-reparse property only runs on the `Ok` ones.
const PROBE_CORPUS: &[&str] = &[
    "SELECT 1",
    "SELECT a, b FROM t WHERE a = b",
    "CREATE TABLE t (id INT)",
    "INSERT INTO t VALUES (1)",
    "UPDATE t SET a = 1",
    "DELETE FROM t",
    "DO 'x'",
    "PREPARE p AS SELECT 1",
    "DEALLOCATE p",
    "GRANT SELECT ON t TO r",
    "DROP DATABASE d",
    "DROP INDEX i",
    "VACUUM",
    "ANALYZE",
    "LOCK TABLES t READ",
    "LOAD 'ext'",
];

/// Look up a gate by its sub-flag name within a given family.
fn find_gate(gates: &[&'static Gate], name: &str) -> Option<&'static Gate> {
    gates.iter().copied().find(|g| g.name == name)
}

/// Look up a statement-head gate by its sub-flag name.
fn gate_by_name(name: &str) -> Option<&'static Gate> {
    find_gate(HEAD_GATES, name)
}

/// Parse→render→reparse a probe under `candidate`; on a stability mismatch return the
/// diagnostic, else `None`. A parse `Err` is not a mismatch (the probe simply does not apply
/// to this feature set); only a tree that renders and reparses to something structurally
/// different is a finding.
fn stability_finding(sql: &str, candidate: &FeatureSet) -> Option<String> {
    let Ok(parsed) = parse_with(sql, squonk::ParseConfig::new(AdHocDialect(candidate))) else {
        return None;
    };
    let rendered = crate::render_statements(&parsed, RenderMode::Parenthesized);
    let Ok(reparsed) = parse_with(&rendered, squonk::ParseConfig::new(AdHocDialect(candidate)))
    else {
        return Some(format!(
            "probe {sql:?} parsed but its render {rendered:?} did not reparse under the pair"
        ));
    };
    let comparison = crate::shared_interner::compare_statements_with_shared_symbols(
        parsed.statements(),
        parsed.resolver(),
        reparsed.statements(),
        reparsed.resolver(),
    );
    (!comparison.structurally_equal()).then(|| {
        comparison.failure_message(
            &format!("probe {sql:?} render-reparse structural mismatch"),
            &[("rendered SQL", &rendered)],
            None,
        )
    })
}

/// The registry verdict on a candidate feature set, as a printable reason, or `None` when
/// the set is clean on all three registries. Reading all three (not just the two the ticket
/// names) keeps the lane forward-compatible with the parse-entry debug-assert, which trips on
/// any registry violation.
fn registry_verdict(candidate: &FeatureSet) -> Option<String> {
    if let Some(c) = candidate.lexical_conflict() {
        return Some(format!("lexical_conflict::{c:?}"));
    }
    if let Some(d) = candidate.feature_dependencies() {
        return Some(format!("feature_dependencies::{d:?}"));
    }
    if let Some(g) = candidate.grammar_conflict() {
        return Some(format!("grammar_conflict::{g:?}"));
    }
    None
}

/// Outcome of probing one lattice candidate.
enum CandidateOutcome {
    /// A registry flagged the candidate: it is skipped (never parsed), carrying the printable
    /// reason. The flagging is itself asserted coverage — a pair/triple that *should* be
    /// registered but is not would land in [`Valid`](Self::Valid) and could then panic or
    /// mis-parse.
    Flagged(String),
    /// The candidate is registry-clean and was parsed against the whole corpus.
    Valid {
        /// How many corpus probes parsed `Ok` (a calibration figure, not an assertion).
        corpus_parses: usize,
    },
}

/// Probe one candidate feature set — the shared inner step of every lattice lane. Skip-as-flagged
/// when any registry flags it; otherwise parse the whole corpus (panic-free by contract) and
/// render-reparse each `Ok` for structural stability, pushing any finding under `label()`. The
/// label is a closure so its `format!` is paid only on the rare finding path.
fn probe_candidate(
    candidate: &FeatureSet,
    corpus: &[&str],
    label: impl Fn() -> String,
    findings: &mut Vec<String>,
) -> CandidateOutcome {
    if let Some(reason) = registry_verdict(candidate) {
        return CandidateOutcome::Flagged(reason);
    }
    let mut corpus_parses = 0usize;
    for sql in corpus {
        if parse_with(sql, squonk::ParseConfig::new(AdHocDialect(candidate))).is_ok() {
            corpus_parses += 1;
        }
        if let Some(finding) = stability_finding(sql, candidate) {
            findings.push(format!("{}: {finding}", label()));
        }
    }
    CandidateOutcome::Valid { corpus_parses }
}

/// A tiny deterministic SplitMix64 PRNG — enough to draw a fixed, reproducible sample of the
/// triple space without a `rand` dependency. Seeded from a lane constant, so the CI sample is
/// identical build-to-build: the "fixed seed" the triples follow-up calls for.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n` (`n > 0`).
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// The `||`/`&&`/`^` meaning enum spaces (`spike-lattice-followup-value-carrying-flags`) —
/// enumerated exhaustively, each variant a lattice point. These are meaning enums with no
/// registry variant (an invalid "both meanings" state is unrepresentable by construction), so
/// the boolean lanes cannot express them at all.
const PIPE_OPERATOR_VALUES: &[PipeOperator] =
    &[PipeOperator::StringConcat, PipeOperator::LogicalOr];
const DOUBLE_AMPERSAND_VALUES: &[DoubleAmpersand] = &[
    DoubleAmpersand::Unsupported,
    DoubleAmpersand::LogicalAnd,
    DoubleAmpersand::Overlaps,
];
const CARET_OPERATOR_VALUES: &[CaretOperator] = &[
    CaretOperator::Unsupported,
    CaretOperator::Exponent,
    CaretOperator::BitwiseXor,
];
const KEYWORD_OPERATORS_VALUES: &[KeywordOperators] = &[
    KeywordOperators::Unsupported,
    KeywordOperators::MySql,
    KeywordOperators::Sqlite,
    KeywordOperators::DuckDb,
];

/// Representative `CommentSyntax::versioned_comments` points: the `None` plain-comment mode and
/// `Some(bound)` servers straddling the 5-digit / 6-digit `MYSQL_VERSION_ID` boundary the flag
/// doc describes (`Option<u32>` is not finitely enumerable, so the space is sampled here).
const VERSIONED_COMMENT_POINTS: &[Option<u32>] = &[
    None,
    Some(0),
    Some(1),
    Some(50000),
    Some(80101),
    Some(100000),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Gate names are unique — a duplicate would double-count pairs and mask a real gate
    /// behind a copy.
    #[test]
    fn head_gate_names_are_unique() {
        let mut names: Vec<&str> = HEAD_GATES.iter().map(|g| g.name).collect();
        names.sort_unstable();
        let mut deduped = names.clone();
        deduped.dedup();
        assert_eq!(names, deduped, "a head-gate name is listed twice");
    }

    /// Each gate's `set`/`is_enabled` closures address the same flag: forcing it on then off
    /// is observable through `is_enabled`, so a mis-wired macro line (wrong axis or field)
    /// fails here rather than silently sampling the wrong flag.
    #[test]
    fn gate_setters_are_observable() {
        for gate in HEAD_GATES {
            let on = (gate.set)(&FeatureSet::POSTGRES, true);
            let off = (gate.set)(&FeatureSet::POSTGRES, false);
            assert!(
                (gate.is_enabled)(&on),
                "{}: set(true) not observed",
                gate.name
            );
            assert!(
                !(gate.is_enabled)(&off),
                "{}: set(false) not observed",
                gate.name
            );
        }
    }

    /// Every ledger claimant is a head gate. This is the self-maintaining enumeration
    /// guard: a new [`MULTI_CLAIMANT_STATEMENT_HEADS`] row whose claimant is not in
    /// [`HEAD_GATES`] fails here, so the instrument cannot silently fall behind the ledger.
    #[test]
    fn head_gate_family_covers_ledger_claimants() {
        let mut missing: Vec<&str> = Vec::new();
        for head in MULTI_CLAIMANT_STATEMENT_HEADS {
            for claimant in head.claimants {
                if gate_by_name(claimant).is_none() {
                    missing.push(claimant);
                }
            }
        }
        assert!(
            missing.is_empty(),
            "ledger claimants absent from HEAD_GATES (extend the family): {missing:?}",
        );
    }

    /// The calibration proof: the three known head-contention hazards are rediscovered as registry-flagged
    /// pairs by the instrument, so forcing each MySQL rival on top of `POSTGRES` is a
    /// `grammar_conflict` — the exact combination the generative lane must SKIP-as-flagged
    /// rather than parse.
    #[test]
    fn audit_hazards_are_rediscovered_as_grammar_conflicts() {
        let cases: [(&str, &str, GrammarConflict); 3] = [
            (
                "do_statement",
                "do_expression_list",
                GrammarConflict::DoStatementVersusDoExpressionList,
            ),
            (
                "prepared_statements",
                "prepared_statements_from",
                GrammarConflict::PreparedStatementsVersusPreparedStatementsFrom,
            ),
            (
                "access_control_account_grants",
                "access_control_extended_objects",
                GrammarConflict::AccountGrantsVersusExtendedObjects,
            ),
        ];
        for (a, b, expected) in cases {
            let ga = gate_by_name(a).unwrap();
            let gb = gate_by_name(b).unwrap();
            let candidate = (gb.set)(&(ga.set)(&FeatureSet::POSTGRES, true), true);
            assert_eq!(
                candidate.grammar_conflict(),
                Some(expected),
                "pair ({a}, {b}) must be grammar-flagged so the lane skips it as invalid",
            );
        }
    }

    /// The main lane: exhaustive pairs over [`HEAD_GATES`]. Each invalid (registry-flagged)
    /// pair is skipped-as-flagged; each valid pair is parsed against the probe corpus and
    /// checked for render-reparse stability. Fails with the full triage list if any valid
    /// pair produces a stability finding; otherwise prints the calibration summary.
    #[test]
    fn combination_lattice_pair_scan() {
        let mut pairs = 0usize;
        let mut skipped_flagged = 0usize;
        let mut valid = 0usize;
        let mut corpus_parses = 0usize;
        // A few representative flagged reasons, for the calibration readout.
        let mut sample_flagged: Vec<String> = Vec::new();
        let mut findings: Vec<String> = Vec::new();

        for (i, ga) in HEAD_GATES.iter().enumerate() {
            for gb in HEAD_GATES.iter().skip(i + 1) {
                pairs += 1;
                let candidate = (gb.set)(&(ga.set)(&FeatureSet::POSTGRES, true), true);

                if let Some(reason) = registry_verdict(&candidate) {
                    skipped_flagged += 1;
                    if sample_flagged.len() < 12 {
                        sample_flagged.push(format!("({}, {}) -> {reason}", ga.name, gb.name));
                    }
                    continue;
                }

                valid += 1;
                for sql in PROBE_CORPUS {
                    if parse_with(sql, squonk::ParseConfig::new(AdHocDialect(&candidate))).is_ok() {
                        corpus_parses += 1;
                    }
                    if let Some(finding) = stability_finding(sql, &candidate) {
                        findings.push(format!("pair ({}, {}): {finding}", ga.name, gb.name));
                    }
                }
            }
        }

        eprintln!(
            "lattice pair scan: gates={} pairs={pairs} skipped_flagged={skipped_flagged} \
             valid={valid} corpus_parses={corpus_parses} (probe corpus={})",
            HEAD_GATES.len(),
            PROBE_CORPUS.len(),
        );
        eprintln!("sample flagged pairs:");
        for line in &sample_flagged {
            eprintln!("  {line}");
        }

        assert!(
            findings.is_empty(),
            "combination-lattice stability findings ({}):\n{}",
            findings.len(),
            findings.join("\n"),
        );
    }

    /// The ledger's resolution column made executable over the all-claimants-on candidate:
    /// union rows must be registry-clean and free of sibling deadening; exclusion rows must
    /// be registry-flagged or a documented ledger exclusion. Prints the meaningful-vs-vacuous
    /// split of the non-deadening probes so a wrong probe is visible, not silently vacuous.
    #[test]
    fn ledger_head_resolution_is_consistent() {
        let mut meaningful = 0usize;
        let mut vacuous = 0usize;
        let mut findings: Vec<String> = Vec::new();

        for head in MULTI_CLAIMANT_STATEMENT_HEADS {
            let gates: Vec<&Gate> = head
                .claimants
                .iter()
                .filter_map(|c| gate_by_name(c))
                .collect();
            // Single-claimant rows (SET, UPDATE, CACHE/LOAD INDEX) express no both-on pair.
            if gates.len() < 2 {
                continue;
            }
            let all_on = gates
                .iter()
                .fold(FeatureSet::POSTGRES, |acc, g| (g.set)(&acc, true));
            let verdict = registry_verdict(&all_on);

            match head.resolution {
                HeadResolution::MeceLookahead | HeadResolution::DispatchOrderUnion => {
                    if let Some(reason) = &verdict {
                        findings.push(format!(
                            "{:?}: a union row must be registry-clean, but is flagged: {reason}",
                            head.heads,
                        ));
                        continue;
                    }
                    // No claimant's own form may be deadened by enabling its siblings.
                    for gate in &gates {
                        let Some(sql) = ledger_probe(gate.name) else {
                            continue;
                        };
                        let one_on = (gate.set)(&FeatureSet::POSTGRES, true);
                        if !accepts_under(sql, &one_on) {
                            vacuous += 1;
                            continue;
                        }
                        meaningful += 1;
                        if !accepts_under(sql, &all_on) {
                            findings.push(format!(
                                "{:?}: enabling siblings deadened {}'s own form {sql:?}",
                                head.heads, gate.name,
                            ));
                        }
                    }
                }
                HeadResolution::OneReadingExclusion | HeadResolution::Route => {
                    let documented_exclusion = !head.lenient_excludes.is_empty();
                    assert!(
                        verdict.is_some() || documented_exclusion,
                        "{:?}: an exclusion row must be registry-flagged or a documented \
                         ledger exclusion, but the all-on candidate is clean and forgoes nothing",
                        head.heads,
                    );
                }
            }
        }

        eprintln!("ledger non-deadening probes: meaningful={meaningful} vacuous={vacuous}");
        assert!(
            findings.is_empty(),
            "ledger head-resolution findings ({}):\n{}",
            findings.len(),
            findings.join("\n"),
        );
    }

    /// Best-effort representative SQL for a union-row claimant's *own* form — the input that
    /// should stay parseable when its siblings are also enabled. A `None` return (or an SQL
    /// that does not parse under the single-claimant config) makes that claimant's
    /// non-deadening check vacuous rather than false.
    fn ledger_probe(name: &str) -> Option<&'static str> {
        Some(match name {
            // Explicit `AS x` alias so `READ` is read as the lock kind, not `t`'s alias:
            // POSTGRES does not reserve `READ`/`WRITE`, so a bare `LOCK TABLES t READ` binds
            // `READ` as the alias and then fails the mandatory-kind expectation.
            "lock_tables" => "LOCK TABLES t AS x READ",
            "lock_instance" => "LOCK INSTANCE FOR BACKUP",
            "load_data" => "LOAD DATA INFILE 'f' INTO TABLE t",
            "load_extension" => "LOAD 'ext'",
            "key_cache_statements" => "LOAD INDEX INTO CACHE t",
            "vacuum" => "VACUUM",
            "vacuum_analyze" => "VACUUM s.t",
            "analyze" => "ANALYZE",
            "table_maintenance" => "ANALYZE TABLE t",
            "view_definition_options" => "ALTER VIEW v AS SELECT 1",
            "alter_object_set_schema" => "ALTER VIEW v SET SCHEMA s",
            "alter_database" => "ALTER DATABASE d SET ALIAS TO a",
            "alter_database_options" => "ALTER DATABASE d DEFAULT CHARACTER SET utf8mb4",
            "import_table" => "IMPORT TABLE FROM 'f'",
            "export_import_database" => "IMPORT DATABASE 'd'",
            _ => return None,
        })
    }

    // ====================================================================================
    //  Follow-up lanes (the productionization children filed off the spike)
    // ====================================================================================

    /// Running tally over a lattice lane's candidates.
    #[derive(Default)]
    struct ScanStats {
        probed: usize,
        flagged: usize,
        valid: usize,
        corpus_parses: usize,
    }

    impl ScanStats {
        fn record(&mut self, outcome: CandidateOutcome) {
            self.probed += 1;
            match outcome {
                CandidateOutcome::Flagged(_) => self.flagged += 1,
                CandidateOutcome::Valid { corpus_parses } => {
                    self.valid += 1;
                    self.corpus_parses += corpus_parses;
                }
            }
        }
    }

    /// The expression/lexical-trigger probe corpus — one input per trigger the [`EXPR_GATES`]
    /// family reshapes, plus a couple of generic forms. A probe need not parse under bare
    /// `POSTGRES`; the no-panic property holds for a reject and the stability property runs only
    /// on the `Ok` parses.
    const EXPR_PROBE_CORPUS: &[&str] = &[
        "SELECT \"x\"",
        "SELECT $tag$body$tag$",
        "SELECT N'x'",
        "SELECT $1",
        "SELECT ?",
        "SELECT :name",
        "SELECT @x",
        "SELECT @@v",
        "SELECT a[1]",
        "SELECT a[1:2]",
        "SELECT ARRAY[1, 2]",
        "SELECT {'k': 1}",
        "SELECT a:b",
        "SELECT #1",
        "SELECT a @> b",
        "SELECT a -> b",
        "SELECT INTERVAL 3 DAY",
        "SELECT 1 # 2",
        "SELECT a, b FROM t WHERE a = b",
        "SELECT 1",
    ];

    /// The value-axis probe corpus — one input per meaning enum / versioned-comment form, plus
    /// generic statements so a value change that reshapes ordinary parsing still surfaces.
    const VALUE_PROBE_CORPUS: &[&str] = &[
        "SELECT 'a' || 'b'",
        "SELECT a && b",
        "SELECT 2 ^ 3",
        "SELECT a DIV b",
        "SELECT a MOD b",
        "SELECT a XOR b",
        "SELECT a GLOB b",
        "SELECT 1 /*!50000 + 1 */",
        "SELECT a, b FROM t WHERE a = b",
        "SELECT 1",
    ];

    /// A trimmed corpus for the multi-base lane (five bases × both directions is the widest lane,
    /// so it runs a statement-head-focused subset rather than the full pair corpus).
    const MULTIBASE_PROBE_CORPUS: &[&str] = &[
        "SELECT 1",
        "SELECT a FROM t WHERE a = b",
        "CREATE TABLE t (id INT)",
        "GRANT SELECT ON t TO r",
        "DROP DATABASE d",
        "ANALYZE",
        "LOCK TABLES t READ",
        "PREPARE p AS SELECT 1",
    ];

    /// All C(k,3) index triples drawn from the gates named in `names` that exist in `gates` — the
    /// adversarial core of a triple lane (a family that all claims one trigger / one head).
    fn triples_over_names(gates: &[&'static Gate], names: &[&str]) -> Vec<[usize; 3]> {
        let idxs: Vec<usize> = names
            .iter()
            .filter_map(|n| gates.iter().position(|g| g.name == *n))
            .collect();
        let mut out = Vec::new();
        for i in 0..idxs.len() {
            for j in i + 1..idxs.len() {
                for k in j + 1..idxs.len() {
                    let mut t = [idxs[i], idxs[j], idxs[k]];
                    t.sort_unstable();
                    out.push(t);
                }
            }
        }
        out
    }

    /// One triple lane's shape: which gate family it scans, where the sample budget sits, and
    /// what it parses. Bundled as a struct so [`TripleScanSpec::scan`] reads as one call per lane.
    struct TripleScanSpec<'a> {
        gates: &'a [&'static Gate],
        base: &'a FeatureSet,
        /// Lane name for finding labels (`"head"` / `"expr"`).
        family: &'a str,
        /// Triples always probed — the exhaustive adversarial core.
        adversarial: &'a [[usize; 3]],
        /// How many extra triples the fixed-seed sampler draws from the rest of the space.
        sample_size: usize,
        seed: u64,
        corpus: &'a [&'static str],
    }

    impl TripleScanSpec<'_> {
        /// Scan a set of unordered triples over `gates` from `base`: the `adversarial` triples
        /// (always probed) unioned with a fixed-seed random sample of `sample_size` more from
        /// the full `C(n,3)` space. Each triple forces its three flags on; a registry-flagged
        /// candidate is skipped-as-flagged, a clean one is parsed + render-reparse-checked over
        /// `corpus`.
        fn scan(&self, findings: &mut Vec<String>) -> ScanStats {
            use std::collections::BTreeSet;
            let n = self.gates.len();
            let mut triples: BTreeSet<[usize; 3]> = self.adversarial.iter().copied().collect();
            let mut rng = SplitMix64(self.seed);
            let target = triples.len() + self.sample_size;
            // Bounded draw budget: distinct triples get rarer as the set fills, so cap attempts
            // so a near-exhausted space cannot spin.
            let mut budget = target.saturating_mul(64) + 64;
            while triples.len() < target && budget > 0 {
                budget -= 1;
                let (a, b, c) = (rng.below(n), rng.below(n), rng.below(n));
                if a == b || b == c || a == c {
                    continue;
                }
                let mut t = [a, b, c];
                t.sort_unstable();
                triples.insert(t);
            }

            let mut stats = ScanStats::default();
            for t in &triples {
                let candidate = t
                    .iter()
                    .fold(self.base.clone(), |acc, &i| (self.gates[i].set)(&acc, true));
                let outcome = probe_candidate(
                    &candidate,
                    self.corpus,
                    || {
                        format!(
                            "{} triple ({}, {}, {})",
                            self.family,
                            self.gates[t[0]].name,
                            self.gates[t[1]].name,
                            self.gates[t[2]].name
                        )
                    },
                    findings,
                );
                stats.record(outcome);
            }
            stats
        }
    }

    // ---- spike-lattice-followup-expression-axis ---------------------------------------------

    /// Each [`EXPR_GATES`] setter addresses the field it names — the mis-wired-macro guard for the
    /// expression family, mirroring [`gate_setters_are_observable`] for the head family.
    #[test]
    fn expr_gate_setters_are_observable() {
        for gate in EXPR_GATES {
            let on = (gate.set)(&FeatureSet::POSTGRES, true);
            let off = (gate.set)(&FeatureSet::POSTGRES, false);
            assert!(
                (gate.is_enabled)(&on),
                "{}: set(true) not observed",
                gate.name
            );
            assert!(
                !(gate.is_enabled)(&off),
                "{}: set(false) not observed",
                gate.name
            );
        }
    }

    /// Expression-gate names are unique and disjoint from the head family, so no flag is
    /// double-counted or sampled in the wrong lane.
    #[test]
    fn expr_gate_names_are_unique_and_disjoint_from_heads() {
        let mut names: Vec<&str> = EXPR_GATES.iter().map(|g| g.name).collect();
        names.sort_unstable();
        let mut deduped = names.clone();
        deduped.dedup();
        assert_eq!(names, deduped, "an expression-gate name is listed twice");
        for g in EXPR_GATES {
            assert!(
                gate_by_name(g.name).is_none(),
                "{} is in both HEAD_GATES and EXPR_GATES",
                g.name
            );
        }
    }

    /// The calibration proof for the expression lane: the known shared-tokenizer-trigger hazards
    /// are rediscovered as the exact [`LexicalConflict`] variant by forcing the two claimants on
    /// over `POSTGRES` — the lexical analog of [`audit_hazards_are_rediscovered_as_grammar_conflicts`]
    /// (`LexicalConflict` is this family's calibration target as `GrammarConflict` is the head
    /// family's). Each pair's verdict is the *first* conflict `lexical_conflict` reports, so the
    /// asserted variant also pins the registry's check order.
    #[test]
    fn expr_lexical_conflicts_are_rediscovered() {
        let cases: [(&str, &str, LexicalConflict); 4] = [
            (
                "named_at",
                "user_variables",
                LexicalConflict::AtNameParameterVersusUserVariable,
            ),
            (
                "line_comment_hash",
                "hash_bitwise_xor",
                LexicalConflict::HashXorOperatorVersusHashComment,
            ),
            (
                "jsonb_operators",
                "system_variables",
                LexicalConflict::JsonbSearchOperatorVersusSystemVariable,
            ),
            (
                "jsonb_operators",
                "anonymous_question",
                LexicalConflict::JsonbKeyExistsVersusAnonymousParameter,
            ),
        ];
        for (a, b, expected) in cases {
            let ga = find_gate(EXPR_GATES, a).unwrap();
            let gb = find_gate(EXPR_GATES, b).unwrap();
            let candidate = (gb.set)(&(ga.set)(&FeatureSet::POSTGRES, true), true);
            assert_eq!(
                candidate.lexical_conflict(),
                Some(expected),
                "pair ({a}, {b}) must be lexically flagged so the lane skips it as invalid",
            );
        }
    }

    /// The expression lane: exhaustive unordered pairs over [`EXPR_GATES`] from `POSTGRES`. Each
    /// registry-flagged pair is skipped-as-flagged (the lexical-conflict registry is the family's
    /// calibration target); each clean pair is parsed against the expression corpus and checked
    /// for render-reparse stability.
    #[test]
    fn expr_axis_pair_scan() {
        let mut stats = ScanStats::default();
        let mut findings: Vec<String> = Vec::new();
        let mut sample_flagged: Vec<String> = Vec::new();

        for (i, ga) in EXPR_GATES.iter().enumerate() {
            for gb in EXPR_GATES.iter().skip(i + 1) {
                let candidate = (gb.set)(&(ga.set)(&FeatureSet::POSTGRES, true), true);
                let outcome = probe_candidate(
                    &candidate,
                    EXPR_PROBE_CORPUS,
                    || format!("pair ({}, {})", ga.name, gb.name),
                    &mut findings,
                );
                if let CandidateOutcome::Flagged(reason) = &outcome {
                    if sample_flagged.len() < 12 {
                        sample_flagged.push(format!("({}, {}) -> {reason}", ga.name, gb.name));
                    }
                }
                stats.record(outcome);
            }
        }

        eprintln!(
            "lattice expr pair scan: gates={} pairs={} skipped_flagged={} valid={} corpus_parses={} (probe corpus={})",
            EXPR_GATES.len(),
            stats.probed,
            stats.flagged,
            stats.valid,
            stats.corpus_parses,
            EXPR_PROBE_CORPUS.len(),
        );
        eprintln!("sample flagged expr pairs:");
        for line in &sample_flagged {
            eprintln!("  {line}");
        }
        assert!(
            findings.is_empty(),
            "expression-axis stability findings ({}):\n{}",
            findings.len(),
            findings.join("\n"),
        );
    }

    // ---- spike-lattice-followup-triples -----------------------------------------------------

    /// The `#`-trigger three-flag contention the triples follow-up names — a line comment, the
    /// XOR operator, and the DuckDB positional column all claiming `#` — must be registry-flagged
    /// (so a triple lane skips it as invalid rather than parsing an incoherent tokenizer).
    #[test]
    fn hash_trigger_triple_is_registry_flagged() {
        let candidate = ["line_comment_hash", "hash_bitwise_xor", "positional_column"]
            .iter()
            .fold(FeatureSet::POSTGRES, |acc, n| {
                (find_gate(EXPR_GATES, n).unwrap().set)(&acc, true)
            });
        assert!(
            registry_verdict(&candidate).is_some(),
            "the #-trigger triple must be registry-flagged",
        );
    }

    /// The head-family triple lane (`k = 3`). Exhaustive triples over gates that share a ledger
    /// head (the adversarial core) unioned with a fixed-seed random sample of the wider
    /// `C(56,3) ≈ 27.7k` space — the whole space is too hot for every build, so the sample is the
    /// deliberately bounded tier and the readout states what fraction was drawn.
    #[test]
    fn head_gate_triple_scan() {
        let mut adversarial: Vec<[usize; 3]> = Vec::new();
        for head in MULTI_CLAIMANT_STATEMENT_HEADS {
            adversarial.extend(triples_over_names(HEAD_GATES, head.claimants));
        }
        adversarial.sort_unstable();
        adversarial.dedup();

        let mut findings: Vec<String> = Vec::new();
        let stats = TripleScanSpec {
            gates: HEAD_GATES,
            base: &FeatureSet::POSTGRES,
            family: "head",
            adversarial: &adversarial,
            sample_size: 2_000,
            seed: 0x51A7_7E5D_5A71_CE55,
            corpus: &PROBE_CORPUS[..12],
        }
        .scan(&mut findings);

        let total = HEAD_GATES.len() * (HEAD_GATES.len() - 1) * (HEAD_GATES.len() - 2) / 6;
        eprintln!(
            "lattice head triple scan: gates={} triples_probed={} (of C(n,3)={total}) adversarial={} skipped_flagged={} valid={} corpus_parses={}",
            HEAD_GATES.len(),
            stats.probed,
            adversarial.len(),
            stats.flagged,
            stats.valid,
            stats.corpus_parses,
        );
        assert!(
            findings.is_empty(),
            "head-triple stability findings ({}):\n{}",
            findings.len(),
            findings.join("\n"),
        );
    }

    /// The expression-family triple lane. The `EXPR_GATES` space is smaller, so its adversarial
    /// core is the four shared-sigil families (`#`, `@`, `:`, `$`) — every triple that could
    /// contend for one trigger — unioned with a fixed-seed sample of the rest.
    #[test]
    fn expr_gate_triple_scan() {
        let sigil_families: &[&[&str]] = &[
            &["line_comment_hash", "hash_bitwise_xor", "positional_column"],
            &[
                "named_at",
                "user_variables",
                "system_variables",
                "containment_operators",
                "jsonb_operators",
                "custom_operators",
            ],
            &[
                "named_colon",
                "subscript",
                "collection_literals",
                "semi_structured_access",
            ],
            &[
                "positional_dollar",
                "named_dollar",
                "dollar_quoted_strings",
                "money_literals",
            ],
        ];
        let mut adversarial: Vec<[usize; 3]> = Vec::new();
        for family in sigil_families {
            adversarial.extend(triples_over_names(EXPR_GATES, family));
        }
        adversarial.sort_unstable();
        adversarial.dedup();

        let mut findings: Vec<String> = Vec::new();
        let stats = TripleScanSpec {
            gates: EXPR_GATES,
            base: &FeatureSet::POSTGRES,
            family: "expr",
            adversarial: &adversarial,
            sample_size: 1_500,
            seed: 0x2545_F491_4F6C_DD1D,
            corpus: EXPR_PROBE_CORPUS,
        }
        .scan(&mut findings);

        let n = EXPR_GATES.len();
        eprintln!(
            "lattice expr triple scan: gates={n} triples_probed={} (of C(n,3)={}) adversarial={} skipped_flagged={} valid={} corpus_parses={}",
            stats.probed,
            n * (n - 1) * (n - 2) / 6,
            adversarial.len(),
            stats.flagged,
            stats.valid,
            stats.corpus_parses,
        );
        assert!(
            findings.is_empty(),
            "expr-triple stability findings ({}):\n{}",
            findings.len(),
            findings.join("\n"),
        );
    }

    // ---- spike-lattice-followup-value-carrying-flags ----------------------------------------

    /// Cross a single value-axis point (`base_v` = `POSTGRES` with that axis set) against every
    /// head gate turned on, probing each clean candidate over the value corpus.
    fn cross_value_with_heads(
        base_v: &FeatureSet,
        label: &str,
        corpus: &[&str],
        stats: &mut ScanStats,
        findings: &mut Vec<String>,
    ) {
        for head in HEAD_GATES {
            let candidate = (head.set)(base_v, true);
            let outcome = probe_candidate(
                &candidate,
                corpus,
                || format!("{label} + {}", head.name),
                findings,
            );
            stats.record(outcome);
        }
    }

    /// The value-carrying lane. Boolean gates cannot express the `||`/`&&`/`^`/keyword-operator
    /// meaning enums, the `versioned_comments` `Option<u32>`, or the binding-power tables; this
    /// enumerates the finite enums exhaustively and the unbounded axes (`Option<u32>`, the
    /// tables) at representative points, crosses each against the head-gate booleans, and adds a
    /// small full product over the four operator-meaning enums (a value×value cross the head
    /// cross cannot reach).
    #[test]
    fn value_axis_scan() {
        let mut stats = ScanStats::default();
        let mut findings: Vec<String> = Vec::new();
        let base = FeatureSet::POSTGRES;

        for &v in PIPE_OPERATOR_VALUES {
            let bv = base.with(FeatureDelta::EMPTY.pipe_operator(v));
            cross_value_with_heads(
                &bv,
                &format!("pipe_operator={v:?}"),
                VALUE_PROBE_CORPUS,
                &mut stats,
                &mut findings,
            );
        }
        for &v in DOUBLE_AMPERSAND_VALUES {
            let bv = base.with(FeatureDelta::EMPTY.double_ampersand(v));
            cross_value_with_heads(
                &bv,
                &format!("double_ampersand={v:?}"),
                VALUE_PROBE_CORPUS,
                &mut stats,
                &mut findings,
            );
        }
        for &v in CARET_OPERATOR_VALUES {
            let bv = base.with(FeatureDelta::EMPTY.caret_operator(v));
            cross_value_with_heads(
                &bv,
                &format!("caret_operator={v:?}"),
                VALUE_PROBE_CORPUS,
                &mut stats,
                &mut findings,
            );
        }
        for &v in KEYWORD_OPERATORS_VALUES {
            let bv = base.with(FeatureDelta::EMPTY.keyword_operators(v));
            cross_value_with_heads(
                &bv,
                &format!("keyword_operators={v:?}"),
                VALUE_PROBE_CORPUS,
                &mut stats,
                &mut findings,
            );
        }
        for &p in VERSIONED_COMMENT_POINTS {
            let bv = base.with(FeatureDelta::EMPTY.comment_syntax(CommentSyntax {
                versioned_comments: p,
                ..base.comment_syntax
            }));
            cross_value_with_heads(
                &bv,
                &format!("versioned_comments={p:?}"),
                VALUE_PROBE_CORPUS,
                &mut stats,
                &mut findings,
            );
        }
        // Binding-power and set-operation-power tables are large structs with no finite value
        // space; sample them at the shipped preset tables (representative adversarial points).
        let binding_tables = [
            ("ansi", FeatureSet::ANSI.binding_powers),
            ("mysql", FeatureSet::MYSQL.binding_powers),
            ("sqlite", FeatureSet::SQLITE.binding_powers),
            ("duckdb", FeatureSet::DUCKDB.binding_powers),
        ];
        for (name, table) in binding_tables {
            let bv = base.with(FeatureDelta::EMPTY.binding_powers(table));
            cross_value_with_heads(
                &bv,
                &format!("binding_powers={name}"),
                VALUE_PROBE_CORPUS,
                &mut stats,
                &mut findings,
            );
        }
        let setop_tables = [
            ("ansi", FeatureSet::ANSI.set_operation_powers),
            ("mysql", FeatureSet::MYSQL.set_operation_powers),
            ("sqlite", FeatureSet::SQLITE.set_operation_powers),
            ("duckdb", FeatureSet::DUCKDB.set_operation_powers),
        ];
        for (name, table) in setop_tables {
            let bv = base.with(FeatureDelta::EMPTY.set_operation_powers(table));
            cross_value_with_heads(
                &bv,
                &format!("set_operation_powers={name}"),
                VALUE_PROBE_CORPUS,
                &mut stats,
                &mut findings,
            );
        }

        // The value×value cross the head cross cannot reach: the full product of the four
        // operator-meaning enums (2·3·3·4 = 72), no head gate.
        let mut product = 0usize;
        for &pipe in PIPE_OPERATOR_VALUES {
            for &da in DOUBLE_AMPERSAND_VALUES {
                for &caret in CARET_OPERATOR_VALUES {
                    for &kw in KEYWORD_OPERATORS_VALUES {
                        product += 1;
                        let candidate = base
                            .with(FeatureDelta::EMPTY.pipe_operator(pipe))
                            .with(FeatureDelta::EMPTY.double_ampersand(da))
                            .with(FeatureDelta::EMPTY.caret_operator(caret))
                            .with(FeatureDelta::EMPTY.keyword_operators(kw));
                        let outcome = probe_candidate(
                            &candidate,
                            VALUE_PROBE_CORPUS,
                            || format!("operators({pipe:?}, {da:?}, {caret:?}, {kw:?})"),
                            &mut findings,
                        );
                        stats.record(outcome);
                    }
                }
            }
        }

        eprintln!(
            "lattice value axis scan: candidates={} skipped_flagged={} valid={} corpus_parses={} operator_product={product}",
            stats.probed, stats.flagged, stats.valid, stats.corpus_parses,
        );
        assert!(
            findings.is_empty(),
            "value-carrying stability findings ({}):\n{}",
            findings.len(),
            findings.join("\n"),
        );
    }

    // ---- spike-lattice-followup-nonpostgres-bases -------------------------------------------

    /// The multi-base / flag-off lane. The prototype pair lane fixes the base at `POSTGRES` and
    /// forces both flags *on*; a hazard reachable only from another base — or one that needs a
    /// base flag turned *off* — is unsampled there. This reruns the pair core from every preset
    /// base in **both** directions (both-on and both-off), skipping candidates already covered by
    /// the `POSTGRES`-on pair lane (deduped by feature-set identity) so runtime stays bounded.
    #[test]
    fn nonpostgres_base_pair_scan() {
        use std::collections::HashSet;

        // Seed the dedup set with the POSTGRES-base both-on candidates the prototype lane already
        // covers, so this lane spends its budget only on genuinely new feature sets.
        let mut seen: HashSet<String> = HashSet::new();
        for (i, ga) in HEAD_GATES.iter().enumerate() {
            for gb in HEAD_GATES.iter().skip(i + 1) {
                let candidate = (gb.set)(&(ga.set)(&FeatureSet::POSTGRES, true), true);
                seen.insert(format!("{candidate:?}"));
            }
        }
        let seeded = seen.len();

        let bases: [(&str, FeatureSet); 5] = [
            ("postgres", FeatureSet::POSTGRES),
            ("mysql", FeatureSet::MYSQL),
            ("sqlite", FeatureSet::SQLITE),
            ("duckdb", FeatureSet::DUCKDB),
            ("ansi", FeatureSet::ANSI),
        ];

        let mut stats = ScanStats::default();
        let mut duplicates = 0usize;
        let mut findings: Vec<String> = Vec::new();

        for (base_name, base) in &bases {
            for (i, ga) in HEAD_GATES.iter().enumerate() {
                for gb in HEAD_GATES.iter().skip(i + 1) {
                    for on in [true, false] {
                        let candidate = (gb.set)(&(ga.set)(base, on), on);
                        // POSTGRES-on pairs are the seeded prototype results — skip as covered.
                        if !seen.insert(format!("{candidate:?}")) {
                            duplicates += 1;
                            continue;
                        }
                        let dir = if on { "on" } else { "off" };
                        let outcome = probe_candidate(
                            &candidate,
                            MULTIBASE_PROBE_CORPUS,
                            || format!("{base_name}/{dir} ({}, {})", ga.name, gb.name),
                            &mut findings,
                        );
                        stats.record(outcome);
                    }
                }
            }
        }

        eprintln!(
            "lattice multi-base scan: bases={} seeded_postgres_on={seeded} candidates={} deduped={duplicates} skipped_flagged={} valid={} corpus_parses={}",
            bases.len(),
            stats.probed,
            stats.flagged,
            stats.valid,
            stats.corpus_parses,
        );
        assert!(
            findings.is_empty(),
            "multi-base / flag-off stability findings ({}):\n{}",
            findings.len(),
            findings.join("\n"),
        );
    }
}
