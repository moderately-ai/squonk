// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The Databricks dialect preset (ANSI-derived, deliberately conservative).
//!
//! Databricks SQL (the Spark-derived engine) diverges widely across its type, function,
//! and statement surface, and — unlike the five shipped oracle-compared presets — this
//! workspace has **no Databricks oracle**, so over-acceptance cannot be measured.
//! Conservatism is therefore the honesty bar: this preset derives from
//! [`FeatureSet::ANSI`], the strict standard baseline, and enables only the Databricks
//! surface that already has a modelled, tested parser gate and clear documentary evidence.
//! Every other axis keeps its ANSI value; a reader can predict from this module exactly
//! what Databricks accepts beyond the standard, and unsupported Databricks syntax is a
//! clean reject routed to a focused follow-up ticket, never a silent over-accept.
//!
//! # What this preset adds over ANSI
//!
//! Six grammar gates, each documented as Databricks SQL surface:
//!
//! - [`sided_semi_anti_join`](JoinSyntax::sided_semi_anti_join) — the
//!   `{LEFT|RIGHT} {SEMI|ANTI} JOIN` sided semi-/anti-join spelling, Spark/Hive/Databricks'
//!   signature join family. This is the flag this preset exists to make real: it shipped
//!   staged (Lenient-only) with no engine preset home until now. The leading `LEFT`/`RIGHT`
//!   is already a reserved join side, so no reserved-word interplay is needed (the
//!   preceding factor's alias can never swallow it). Databricks documents the `LEFT`-sided
//!   spelling; the atomic flag also admits the `RIGHT`-sided spelling, a known
//!   conservative-direction over-acceptance a future side-refinement (or a Databricks
//!   oracle) would tighten — captured as a deferral on the owning ticket.
//! - [`semi_structured_access`](ExpressionSyntax::semi_structured_access) — the
//!   `base:key[0].field` colon path over `VARIANT`/JSON-string columns, Databricks' JSON
//!   path accessor. Its `:` trigger contends with
//!   [`ParameterSyntax::named_colon`](super::ParameterSyntax::named_colon), which stays off
//!   (ANSI's value) so the `:` trigger has a single claimant — the lexical-consistency
//!   `const` assert below enforces it.
//! - [`qualify`](SelectSyntax::qualify) — the `QUALIFY <predicate>` post-window filter
//!   (Databricks Runtime 10.4 LTS and above). `QUALIFY` is reserved in every identifier
//!   position (see [`DATABRICKS_QUALIFY_RESERVATION`]); the reservation is what lets
//!   `FROM t QUALIFY …` read the clause rather than a table alias named `qualify`, matching
//!   Spark's ANSI reserved-keyword parser. (Default Databricks does not enforce reserved
//!   keywords, so `SELECT qualify FROM t` becomes a conservative reject here rather than an
//!   ordinary column — the honest cost of shipping the clause whole instead of half.)
//! - [`group_by_all`](GroupingSyntax::group_by_all) — the `GROUP BY ALL` clause mode.
//! - [`order_by_all`](GroupingSyntax::order_by_all) — the `ORDER BY ALL` clause mode. Unlike
//!   the Snowflake preset (Snowflake ships `GROUP BY ALL` *without* `ORDER BY ALL`),
//!   Databricks documents **both**, so both flags are on here.
//! - [`lateral_view_clause`](SelectSyntax::lateral_view_clause) — the Spark-inherited
//!   `LATERAL VIEW [OUTER] generator(args) tblName [AS cols]` table-generating clause
//!   (Spark `SqlBaseParser.g4` `lateralView`, documented Databricks SQL surface). The
//!   derived-table `LATERAL` factor stays off, so `LATERAL` leads only this clause under
//!   the preset; the flag doc records the acceptance bound the Spark grammar evidences.
//!
//! It also takes one utility-statement delta: [`show_functions`](ShowSyntax::show_functions),
//! the typed `SHOW FUNCTIONS` function-listing statement. This is the first typed-`SHOW`
//! gate on under Databricks — the MySQL-shaped `SHOW TABLES`/`COLUMNS`/`CREATE TABLE`
//! siblings stay off, but a bare `SHOW FUNCTIONS` listing is documented Spark/Databricks
//! surface with a modelled, tested parser gate, so it clears the conservatism bar.
//!
//! Databricks' identifier lexis takes one delta over ANSI: it quotes identifiers with the
//! MySQL-style backtick `` `…` `` **and** the standard `"…"`, so
//! [`DATABRICKS_IDENTIFIER_QUOTES`] lists both. `double_quoted_strings` stays off (ANSI's
//! value) so `"x"` reads as an identifier, keeping the preset lexically consistent. Spark
//! preserves the exact case of an unquoted identifier at parse time (its case-insensitive
//! *resolution* is an analysis-time concern past this parse-level contract), so
//! [`identifier_casing`](FeatureSet::identifier_casing) is [`Casing::Preserve`] rather than
//! ANSI's upper-fold — an identity-only delta that never affects acceptance.

use super::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, ConstraintSyntax, CreateTableClauseSyntax,
    DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FeatureSet, GroupingSyntax,
    IdentifierQuote, IdentifierSyntax, IndexAlterSyntax, JoinSyntax, Keyword, KeywordOperators,
    KeywordSet, MaintenanceSyntax, MutationSyntax, NullOrdering, NumericLiteralSyntax,
    OperatorSyntax, ParameterSyntax, PipeOperator, PredicateSyntax, QueryTailSyntax,
    RESERVED_BARE_ALIAS, RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME, RESERVED_TYPE_NAME,
    STANDARD_BYTE_CLASSES, SelectSyntax, SessionVariableSyntax, ShowSyntax, StatementDdlGates,
    StringFuncForms, StringLiteralSyntax, TableExpressionSyntax, TableFactorSyntax, TargetSpelling,
    TypeNameSyntax, UtilitySyntax,
};
use crate::precedence::{STANDARD_BINDING_POWERS, STANDARD_SET_OPERATION_BINDING_POWERS};

/// Databricks identifier quoting: the MySQL-style backtick `` `…` `` **and** the SQL
/// standard `"…"`, both at once. The two openers are distinct bytes, so their order is
/// immaterial. `"` stays a quote here (and `double_quoted_strings` is correspondingly off
/// in [`StringLiteralSyntax::ANSI`], which this preset keeps), so `"x"` is an identifier,
/// never a string.
pub const DATABRICKS_IDENTIFIER_QUOTES: &[IdentifierQuote] = &[
    IdentifierQuote::Symmetric('"'),
    IdentifierQuote::Symmetric('`'),
];

/// `QUALIFY`, reserved by Databricks' ANSI-strict Spark parser (its reserved-keyword list
/// rejects the word as an unquoted identifier). Unioned into all four per-position reject
/// sets below, mirroring the Snowflake preset: the bare-alias reservation is load-bearing
/// for the grammar — it is what lets `FROM t QUALIFY …` read the clause instead of a table
/// alias named `qualify`, and the column/function/type reservations match the "reserved
/// everywhere" status. `AS`-label position stays open (`SELECT 1 AS qualify`), keeping
/// `reserved_as_label` empty like every ANSI-derived preset.
pub const DATABRICKS_QUALIFY_RESERVATION: KeywordSet =
    KeywordSet::from_keywords(&[Keyword::Qualify]);

/// The ANSI column-name reject set plus [`DATABRICKS_QUALIFY_RESERVATION`].
pub const DATABRICKS_RESERVED_COLUMN_NAME: KeywordSet =
    RESERVED_COLUMN_NAME.union(DATABRICKS_QUALIFY_RESERVATION);

/// The ANSI function-name reject set plus [`DATABRICKS_QUALIFY_RESERVATION`].
pub const DATABRICKS_RESERVED_FUNCTION_NAME: KeywordSet =
    RESERVED_FUNCTION_NAME.union(DATABRICKS_QUALIFY_RESERVATION);

/// The ANSI type-name reject set plus [`DATABRICKS_QUALIFY_RESERVATION`].
pub const DATABRICKS_RESERVED_TYPE_NAME: KeywordSet =
    RESERVED_TYPE_NAME.union(DATABRICKS_QUALIFY_RESERVATION);

/// The ANSI bare-alias reject set plus [`DATABRICKS_QUALIFY_RESERVATION`].
pub const DATABRICKS_RESERVED_BARE_ALIAS: KeywordSet =
    RESERVED_BARE_ALIAS.union(DATABRICKS_QUALIFY_RESERVATION);

impl SelectSyntax {
    /// Databricks SELECT surface: the ANSI baseline plus the documented Databricks
    /// clauses — the `QUALIFY <predicate>` post-window filter, the `GROUP BY ALL` clause
    /// mode, the `ORDER BY ALL` clause mode, and the Spark-inherited `LATERAL VIEW`
    /// generator clause. Unlike Snowflake, Databricks ships `ORDER BY ALL` alongside
    /// `GROUP BY ALL`, so both are on. Every other SELECT knob is conservatively ANSI.
    pub const DATABRICKS: Self = Self {
        qualify: true,
        // Spark's `LATERAL VIEW [OUTER] generator(args) tblName [AS cols]` clause
        // (SqlBaseParser.g4 `lateralView`, documented Databricks SQL surface); the
        // derived-table `LATERAL` factor stays off, so `LATERAL` leads only this
        // clause under the preset.
        lateral_view_clause: true,
        distinct_on: false,
        select_into: false,
        empty_target_list: false,
        alias_string_literals: false,
        bare_alias_string_literals: false,
        union_by_name: false,
        wildcard_modifiers: false,
        wildcard_replace: false,
        intersect_all: true,
        except_all: true,
        qualified_wildcard_alias: false,
        from_first: false,
        parenthesized_query_operands: true,
        values_rows_require_equal_arity: false,
        values_row_constructor: true,
        as_alias_rejects_reserved: false,
        trailing_comma: false,
        prefix_colon_alias: false,
        connect_by_clause: false,
    };
}

impl QueryTailSyntax {
    /// The `DATABRICKS` preset for query tail syntax.
    pub const DATABRICKS: Self = Self {
        fetch_first: true,
        limit_offset_comma: false,
        locking_clauses: false,
        key_lock_strengths: false,
        stacked_locking_clauses: false,
        using_sample: false,
        leading_offset: true,
        limit_expressions: true,
        limit_percent: false,
        with_ties_requires_order_by: false,
        pipe_syntax: false,
        limit_by_clause: false,
        settings_clause: false,
        format_clause: false,
        for_xml_json_clause: false,
    };
}

impl GroupingSyntax {
    /// The `DATABRICKS` preset for grouping syntax.
    pub const DATABRICKS: Self = Self {
        group_by_all: true,
        group_by_set_quantifier: false,
        order_by_all: true,
        grouping_sets: true,
        with_rollup: false,
        order_by_using: false,
    };
}

impl ExpressionSyntax {
    /// Databricks expression surface: the ANSI baseline plus semi-structured colon path
    /// access (`base:key[0].field`), Databricks' `VARIANT`/JSON accessor. Every other
    /// expression knob is conservatively ANSI.
    pub const DATABRICKS: Self = Self {
        semi_structured_access: true,
        typecast_operator: false,
        subscript: false,
        slice_step: false,
        collate: false,
        at_time_zone: false,
        array_constructor: false,
        multidim_array_literals: false,
        collection_literals: false,
        row_constructor: false,
        struct_constructor: false,
        field_selection: false,
        field_wildcard: false,
        typed_string_literals: true,
        typed_interval_literal: true,
        relaxed_interval_syntax: false,
        mysql_interval_operator: false,
        positional_column: false,
        lambda_keyword: false,
    };
}

impl TableExpressionSyntax {
    /// Databricks table-expression surface: the ANSI baseline plus the Delta/Databricks
    /// `VERSION`/`TIMESTAMP AS OF` time-travel modifiers. The sided `{LEFT|RIGHT}
    /// {SEMI|ANTI} JOIN` family rides [`JoinSyntax`]; the side-less DuckDB `SEMI JOIN`
    /// spelling stays off pending `SEMI`/`ANTI` bare-alias reservation modelling. Every
    /// other table knob is conservatively ANSI.
    pub const DATABRICKS: Self = Self {
        // `VERSION AS OF <n>` / `TIMESTAMP AS OF <ts>` — Delta Lake time travel.
        table_version: true,
        only: false,
        table_sample: false,
        parenthesized_joins: true,
        table_alias_column_lists: true,
        join_using_alias: false,
        index_hints: false,
        table_hints: false,
        partition_selection: false,
        base_table_alias_column_lists: true,
        string_literal_aliases: false,
        aliased_parenthesized_join: true,
        bare_table_alias_is_bare_label: false,
        table_json_path: false,
        indexed_by: false,
    };
}

impl JoinSyntax {
    /// The `DATABRICKS` preset for join syntax.
    pub const DATABRICKS: Self = Self {
        sided_semi_anti_join: true,
        stacked_join_qualifiers: true,
        full_outer_join: true,
        natural_cross_join: false,
        straight_join: false,
        asof_join: false,
        positional_join: false,
        semi_anti_join: false,
        apply_join: false,
        recursive_search_cycle: false,
        recursive_union_rejects_order_limit: false,
        recursive_using_key: false,
    };
}

impl TableFactorSyntax {
    /// The `DATABRICKS` preset for table factor syntax.
    pub const DATABRICKS: Self = Self {
        lateral: false,
        table_functions: false,
        rows_from: false,
        unnest: false,
        unnest_with_offset: false,
        table_function_ordinality: false,
        special_function_table_source: true,
        pivot: false,
        unpivot: false,
        show_ref: false,
        from_values: false,
        json_table: false,
        xml_table: false,
        table_expr_factor: false,
        pivot_value_sources: false,
        match_recognize: false,
        open_json: false,
    };
}

impl UtilitySyntax {
    /// Databricks utility surface: the ANSI baseline plus the typed `SHOW FUNCTIONS`
    /// listing. This is the first typed-`SHOW` gate on under Databricks — the sibling
    /// `SHOW TABLES`/`COLUMNS`/`CREATE TABLE` gates are MySQL-shaped and stay off, but a
    /// bare `SHOW FUNCTIONS` listing is documented Spark/Databricks surface with a modelled,
    /// tested parser gate, so it clears the preset's conservatism bar. Every other utility
    /// knob is conservatively ANSI.
    pub const DATABRICKS: Self = Self {
        start_transaction: true,
        start_transaction_block_optional: false,
        transaction_work_keyword: true,
        begin_transaction_keyword: true,
        commit_transaction_keyword: true,
        rollback_transaction_keyword: true,
        transaction_name: false,
        begin_transaction_modes: true,
        transaction_savepoints: true,
        set_transaction: true,
        transaction_isolation_mode: true,
        transaction_access_mode: true,
        transaction_deferrable_mode: true,
        start_transaction_isolation_mode: true,
        start_transaction_deferrable_mode: true,
        start_transaction_consistent_snapshot: false,
        transaction_multiple_modes: true,
        transaction_mode_comma_required: false,
        transaction_modes_unique: false,
        abort_transaction_alias: false,
        end_transaction_alias: false,
        transaction_release: false,
        transaction_chain: true,
        release_savepoint_keyword_optional: true,
        copy: false,
        copy_into: false,
        stage_references: false,
        comment_on: false,
        comment_if_exists: false,
        pragma: false,
        attach: false,
        kill: false,
        handler_statements: false,
        plugin_component_statements: false,
        shutdown: false,
        restart: false,
        clone: false,
        import_table: false,
        help_statement: false,
        binlog: false,
        key_cache_statements: false,
        use_statement: false,
        use_qualified_name: false,
        prepared_statements: false,
        prepare_typed_parameters: false,
        prepared_statements_from: false,
        call: false,
        call_bare_name: false,
        load_extension: false,
        load_bare_name: false,
        load_data: false,
        reset_scope: false,
        detach_if_exists: false,
        do_statement: false,
        do_expression_list: false,
        lock_tables: false,
        lock_instance: false,
        begin_transaction_mode: false,
        xa_transactions: false,
        rename_statement: false,
        signal_diagnostics: false,
        export_import_database: false,
        update_extensions: false,
        flush: false,
        purge_binary_logs: false,
        replication_statements: false,
    };
}

impl ShowSyntax {
    /// The `DATABRICKS` preset for show syntax.
    pub const DATABRICKS: Self = Self {
        show_functions: true,
        describe: false,
        describe_summarize: false,
        session_statements: true,
        set_value_reserved_words: super::RESERVED_SET_VALUE_WORDS,
        set_value_on_keyword: true,
        set_value_null_keyword: false,
        show_tables: false,
        show_columns: false,
        show_create_table: false,
        show_routine_status: false,
        show_verbose: false,
        show_admin: false,
    };
}

impl MaintenanceSyntax {
    /// The `DATABRICKS` preset for maintenance syntax.
    pub const DATABRICKS: Self = Self {
        vacuum: false,
        vacuum_analyze: false,
        reindex: false,
        analyze: false,
        analyze_columns: false,
        checkpoint: false,
        checkpoint_database: false,
        table_maintenance: false,
    };
}

impl AccessControlSyntax {
    /// The `DATABRICKS` preset for access control syntax.
    pub const DATABRICKS: Self = Self {
        alter_role_rename: false,
        access_control: true,
        access_control_extended_objects: true,
        user_role_management: false,
        access_control_account_grants: false,
    };
}

impl FeatureSet {
    /// Databricks as ANSI-derived dialect data (see the module docs for the full derivation
    /// rationale and the conservatism bar).
    pub const DATABRICKS: Self = Self {
        // Spark preserves the exact case of an unquoted identifier at parse time; its
        // case-insensitive resolution is an analysis-time concern past this parse contract.
        // Identity only — never affects acceptance.
        identifier_casing: Casing::Preserve,
        // The one lexical delta over ANSI: backtick *and* double-quote identifier quoting.
        identifier_quotes: DATABRICKS_IDENTIFIER_QUOTES,
        default_null_ordering: NullOrdering::NullsLast,
        // The one reserved-set delta over ANSI: `QUALIFY` is reserved in every identifier
        // position (matching Spark's ANSI reserved-keyword parser), which the `QUALIFY`
        // clause gate depends on to disambiguate `FROM t QUALIFY …`.
        reserved_column_name: DATABRICKS_RESERVED_COLUMN_NAME,
        reserved_function_name: DATABRICKS_RESERVED_FUNCTION_NAME,
        reserved_type_name: DATABRICKS_RESERVED_TYPE_NAME,
        reserved_bare_alias: DATABRICKS_RESERVED_BARE_ALIAS,
        reserved_as_label: KeywordSet::EMPTY,
        catalog_qualified_names: true,
        byte_classes: STANDARD_BYTE_CLASSES,
        binding_powers: STANDARD_BINDING_POWERS,
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        // Conservative ANSI string/number/parameter surface: Databricks' own forms
        // (backslash escapes, stage/`@`-path references, `$$`-free bind syntax) have no
        // modelled gate here and are deferred rather than guessed at without an oracle.
        // Crucially `named_colon` stays off (in `ParameterSyntax::ANSI`) so the `:` trigger
        // belongs solely to `semi_structured_access` (the lexical assert below enforces it).
        string_literals: StringLiteralSyntax::ANSI,
        numeric_literals: NumericLiteralSyntax::ANSI,
        parameters: ParameterSyntax::ANSI,
        session_variables: SessionVariableSyntax::ANSI,
        identifier_syntax: IdentifierSyntax::ANSI,
        // The sided semi-/anti-join family — one of the capstones this preset exposes.
        table_expressions: TableExpressionSyntax::DATABRICKS,
        join_syntax: JoinSyntax::DATABRICKS,
        table_factor_syntax: TableFactorSyntax::DATABRICKS,
        // The semi-structured colon path accessor.
        expression_syntax: ExpressionSyntax::DATABRICKS,
        operator_syntax: OperatorSyntax::ANSI,
        call_syntax: CallSyntax::ANSI,
        string_func_forms: StringFuncForms::ANSI,
        aggregate_call_syntax: AggregateCallSyntax::ANSI,
        predicate_syntax: PredicateSyntax::ANSI,
        pipe_operator: PipeOperator::StringConcat,
        double_ampersand: DoubleAmpersand::Unsupported,
        keyword_operators: KeywordOperators::Unsupported,
        caret_operator: CaretOperator::Unsupported,
        hash_bitwise_xor: false,
        comment_syntax: CommentSyntax::ANSI,
        mutation_syntax: MutationSyntax::ANSI,
        statement_ddl_gates: StatementDdlGates::ANSI,
        create_table_clause_syntax: CreateTableClauseSyntax::ANSI,
        column_definition_syntax: ColumnDefinitionSyntax::ANSI,
        constraint_syntax: ConstraintSyntax::ANSI,
        index_alter_syntax: IndexAlterSyntax::ANSI,
        existence_guards: ExistenceGuards::ANSI,
        // The `QUALIFY`, `GROUP BY ALL`, and `ORDER BY ALL` clauses.
        select_syntax: SelectSyntax::DATABRICKS,
        query_tail_syntax: QueryTailSyntax::DATABRICKS,
        grouping_syntax: GroupingSyntax::DATABRICKS,
        // The typed `SHOW FUNCTIONS` listing — the first typed-`SHOW` gate on under
        // Databricks (see `UtilitySyntax::DATABRICKS`); every other utility knob is ANSI.
        utility_syntax: UtilitySyntax::DATABRICKS,
        show_syntax: ShowSyntax::DATABRICKS,
        maintenance_syntax: MaintenanceSyntax::DATABRICKS,
        access_control_syntax: AccessControlSyntax::DATABRICKS,
        type_name_syntax: TypeNameSyntax::ANSI,
        // No Databricks-specific Tier-1 output spelling yet; render the portable ANSI
        // canonical type names (a `TargetSpelling::Databricks` is render work a later
        // ticket owns).
        target_spelling: TargetSpelling::Ansi,
    };
}

/// Prefer [`FeatureSet::DATABRICKS`] for struct update.
pub const DATABRICKS: FeatureSet = FeatureSet::DATABRICKS;

// Compile-time proof the Databricks preset claims no shared tokenizer trigger twice. Beyond
// ANSI it adds two triggers — the backtick identifier quote and the `:` semi-structured
// accessor — each with a single claimant (no other enabled feature lexes a backtick, and
// `named_colon` stays off so `:` belongs solely to `semi_structured_access`), and it keeps
// `double_quoted_strings` off so `"` stays the sole identifier quote it already was. Every
// other delta is a contextual grammar gate or a keyword reservation with no tokenizer
// trigger. Kept as a ratchet so a future Databricks delta that *does* add a contending
// trigger (e.g. enabling colon parameters) fails the build here.
const _: () = assert!(FeatureSet::DATABRICKS.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: no refinement
// flag rides an unset base, and no two features contend for one parser-position head.
const _: () = assert!(FeatureSet::DATABRICKS.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::DATABRICKS.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn databricks_is_ansi_plus_the_six_gates_and_two_lexical_facts() {
        // The preset is ANSI with a documented, closed set of divergent axes: the two
        // lexical facts (case-preservation, dual identifier quoting), the `QUALIFY`
        // reservation, and the three enabled sub-presets. Asserting the whole rest equals
        // ANSI keeps the "ANSI-derived, every delta documented" claim honest against a
        // future stray edit. Bind to locals so the const reads are not flagged by clippy's
        // `assertions_on_constants`.
        let ansi = FeatureSet::ANSI;
        let dbx = FeatureSet::DATABRICKS;

        // The two lexical facts.
        assert_eq!(dbx.identifier_casing, Casing::Preserve);
        assert_ne!(dbx.identifier_casing, ansi.identifier_casing);
        assert_eq!(dbx.identifier_quotes, DATABRICKS_IDENTIFIER_QUOTES);
        assert_ne!(dbx.identifier_quotes, ansi.identifier_quotes);
        // `"` stays an identifier quote, so Databricks must keep `double_quoted_strings`
        // off (the interplay the preset depends on for lexical consistency).
        assert!(!dbx.string_literals.double_quoted_strings);
        // `named_colon` off is the interplay the semi-structured accessor depends on.
        assert!(!dbx.parameters.named_colon);

        // The four divergent sub-presets.
        assert_eq!(dbx.select_syntax, SelectSyntax::DATABRICKS);
        assert_ne!(dbx.select_syntax, ansi.select_syntax);
        assert_eq!(dbx.expression_syntax, ExpressionSyntax::DATABRICKS);
        assert_ne!(dbx.expression_syntax, ansi.expression_syntax);
        assert_eq!(dbx.table_expressions, TableExpressionSyntax::DATABRICKS);
        assert_eq!(dbx.join_syntax, JoinSyntax::DATABRICKS);
        assert_ne!(dbx.join_syntax, ansi.join_syntax);
        // The utility surface diverges only in the typed `SHOW FUNCTIONS` gate — the first
        // typed-`SHOW` flag on under Databricks; forcing it off recovers ANSI verbatim.
        assert_eq!(dbx.utility_syntax, UtilitySyntax::DATABRICKS);
        assert_eq!(dbx.show_syntax, ShowSyntax::DATABRICKS);
        assert_ne!(dbx.show_syntax, ansi.show_syntax);
        assert!(dbx.show_syntax.show_functions);
        assert_eq!(
            ShowSyntax {
                show_functions: false,
                ..dbx.show_syntax
            },
            ansi.show_syntax,
        );

        // The reserved-set delta: ANSI base plus QUALIFY, in every identifier position.
        assert_eq!(dbx.reserved_column_name, DATABRICKS_RESERVED_COLUMN_NAME);
        assert_ne!(dbx.reserved_column_name, ansi.reserved_column_name);
        assert_eq!(
            dbx.reserved_function_name,
            DATABRICKS_RESERVED_FUNCTION_NAME
        );
        assert_eq!(dbx.reserved_type_name, DATABRICKS_RESERVED_TYPE_NAME);
        assert_eq!(dbx.reserved_bare_alias, DATABRICKS_RESERVED_BARE_ALIAS);
        // `QUALIFY` is the sole addition — dropping it recovers the ANSI sets verbatim.
        assert_eq!(
            dbx.reserved_column_name
                .difference(DATABRICKS_QUALIFY_RESERVATION),
            ansi.reserved_column_name,
        );
        assert!(dbx.reserved_column_name.contains(Keyword::Qualify));
        assert!(dbx.reserved_bare_alias.contains(Keyword::Qualify));
        // `AS`-label position stays open (`SELECT 1 AS qualify`).
        assert_eq!(dbx.reserved_as_label, KeywordSet::EMPTY);

        // Everything else is inherited verbatim from ANSI.
        assert_eq!(dbx.string_literals, ansi.string_literals);
        assert_eq!(dbx.numeric_literals, ansi.numeric_literals);
        assert_eq!(dbx.parameters, ansi.parameters);
        assert_eq!(dbx.session_variables, ansi.session_variables);
        assert_eq!(dbx.identifier_syntax, ansi.identifier_syntax);
        assert_eq!(dbx.operator_syntax, ansi.operator_syntax);
        assert_eq!(dbx.call_syntax, ansi.call_syntax);
        assert_eq!(dbx.predicate_syntax, ansi.predicate_syntax);
        assert_eq!(dbx.mutation_syntax, ansi.mutation_syntax);
        assert_eq!(dbx.statement_ddl_gates, ansi.statement_ddl_gates);
        assert_eq!(
            dbx.create_table_clause_syntax,
            ansi.create_table_clause_syntax
        );
        assert_eq!(dbx.column_definition_syntax, ansi.column_definition_syntax);
        assert_eq!(dbx.constraint_syntax, ansi.constraint_syntax);
        assert_eq!(dbx.index_alter_syntax, ansi.index_alter_syntax);
        assert_eq!(dbx.existence_guards, ansi.existence_guards);
        assert_eq!(dbx.type_name_syntax, ansi.type_name_syntax);
        assert_eq!(dbx.byte_classes, ansi.byte_classes);
        assert_eq!(dbx.binding_powers, ansi.binding_powers);
        assert_eq!(dbx.target_spelling, ansi.target_spelling);
        assert_eq!(dbx.default_null_ordering, ansi.default_null_ordering);
    }

    #[test]
    fn databricks_enables_exactly_the_six_staged_gates() {
        // The capstone: sided semi-/anti-joins, semi-structured access, QUALIFY,
        // GROUP BY ALL, ORDER BY ALL, and LATERAL VIEW are on, and each is off in the
        // ANSI base it derives from. Forcing the gates back off recovers the ANSI
        // sub-presets verbatim.
        let ansi = FeatureSet::ANSI;
        let dbx = FeatureSet::DATABRICKS;

        assert!(dbx.join_syntax.sided_semi_anti_join);
        assert!(!ansi.join_syntax.sided_semi_anti_join);
        // The side-less DuckDB spelling stays off (deferred).
        assert!(!dbx.join_syntax.semi_anti_join);
        assert!(dbx.expression_syntax.semi_structured_access);
        assert!(!ansi.expression_syntax.semi_structured_access);
        assert!(dbx.select_syntax.qualify && !ansi.select_syntax.qualify);
        assert!(dbx.grouping_syntax.group_by_all && !ansi.grouping_syntax.group_by_all);
        // Unlike Snowflake, Databricks ships ORDER BY ALL alongside GROUP BY ALL.
        assert!(dbx.grouping_syntax.order_by_all && !ansi.grouping_syntax.order_by_all);
        // The Spark-inherited LATERAL VIEW clause; the derived-table LATERAL factor
        // stays off, so `LATERAL` leads only the view clause under this preset.
        assert!(dbx.select_syntax.lateral_view_clause && !ansi.select_syntax.lateral_view_clause);
        assert!(!dbx.table_factor_syntax.lateral);

        assert_eq!(
            SelectSyntax {
                qualify: false,
                lateral_view_clause: false,
                ..dbx.select_syntax
            },
            ansi.select_syntax,
        );
        assert_eq!(
            GroupingSyntax {
                group_by_all: false,
                order_by_all: false,
                ..dbx.grouping_syntax
            },
            ansi.grouping_syntax,
        );
        assert_eq!(
            ExpressionSyntax {
                semi_structured_access: false,
                ..dbx.expression_syntax
            },
            ansi.expression_syntax,
        );
        assert_eq!(
            JoinSyntax {
                sided_semi_anti_join: false,
                ..dbx.join_syntax
            },
            ansi.join_syntax,
        );
    }

    #[test]
    fn databricks_is_lexically_consistent_and_dependency_clean() {
        // Both self-consistency registries must be clean: the backtick quote and the
        // semi-structured `:` trigger each have a single claimant (colon parameters stay
        // off, `double_quoted_strings` off), and none of the five contextual gates rides an
        // unset base flag.
        let dbx = FeatureSet::DATABRICKS;
        assert_eq!(dbx.lexical_conflict(), None);
        assert!(dbx.is_lexically_consistent());
        assert_eq!(dbx.feature_dependencies(), None);
        assert!(dbx.has_satisfied_feature_dependencies());
        assert_eq!(dbx.grammar_conflict(), None);
        assert!(dbx.has_no_grammar_conflict());
    }
}
