// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The QuiltDB dialect preset.
//!
//! This preset starts from the PostgreSQL grammar and adds angle-bracket and composite
//! types (`ARRAY<T>`, `STRUCT<...>`, `MAP(K, V)`), collection constructors including
//! `MAP { ... }`, `* REPLACE`, mutation modifier shapes, multi-row `MERGE INSERT`, joined
//! `UPDATE`/`DELETE`, compact and alter-column identity forms, sequence `CACHE`,
//! `DROP PRIMARY KEY`, the front-position
//! `COMMENT IF EXISTS ON ...` guard, and colocation-group DDL.
//!
//! Its PostgreSQL-compatible query surface rejects wildcard `EXCLUDE`/`RENAME`,
//! `ILIKE`, `IS [NOT] DISTINCT FROM`, `NATURAL CROSS JOIN`, `TABLESAMPLE`, and
//! `INTERSECT ALL`/`EXCEPT ALL`. Alternative DDL spellings such as `MODIFY COLUMN`,
//! `CHANGE COLUMN`, `SET TBLPROPERTIES`, table-scoped `DROP INDEX`, and trailing
//! index `USING` remain outside the grammar.

use super::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, ConstraintSyntax, CreateTableClauseSyntax,
    DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FeatureSet, GroupingSyntax,
    IdentifierSyntax, IndexAlterSyntax, JoinSyntax, KeywordOperators, KeywordSet,
    MaintenanceSyntax, MutationSyntax, NullOrdering, NumericLiteralSyntax, OperatorSyntax,
    POSTGRES_BYTE_CLASSES, ParameterSyntax, PipeOperator, PredicateSyntax, QueryTailSyntax,
    RESERVED_BARE_ALIAS, RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME, RESERVED_TYPE_NAME,
    STANDARD_IDENTIFIER_QUOTES, SelectSyntax, SessionVariableSyntax, ShowSyntax, StatementDdlGates, ViewSequenceClauseSyntax,
    StringFuncForms, StringLiteralSyntax, TableExpressionSyntax, TableFactorSyntax, TargetSpelling,
    TypeNameSyntax, TransactionSyntax, UtilitySyntax,
};
use crate::precedence::STANDARD_SET_OPERATION_BINDING_POWERS;

impl AccessControlSyntax {
    /// Access-control syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        // Preserve user/role statement structure for downstream validation. The richer
        // PostgreSQL GRANT/REVOKE route remains selected.
        user_role_management: true,
        alter_role_rename: true,
        access_control: true,
        access_control_extended_objects: true,
        access_control_account_grants: false,
    };
}

impl ExpressionSyntax {
    /// Expression syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        struct_constructor: true,
        collection_literals: true,
        typecast_operator: true,
        subscript: true,
        slice_step: false,
        collate: true,
        at_time_zone: true,
        semi_structured_access: false,
        array_constructor: true,
        multidim_array_literals: true,
        row_constructor: true,
        field_selection: true,
        field_wildcard: true,
        typed_string_literals: true,
        typed_interval_literal: true,
        relaxed_interval_syntax: false,
        mysql_interval_operator: false,
        positional_column: false,
        lambda_keyword: false,
    };
}

impl SelectSyntax {
    /// Query syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        // Enables wildcard projection modifiers; individual spellings are gated below.
        wildcard_modifiers: false,
        wildcard_replace: true,
        intersect_all: false,
        except_all: false,
        distinct_on: true,
        select_into: true,
        empty_target_list: true,
        qualify: false,
        alias_string_literals: false,
        bare_alias_string_literals: false,
        union_by_name: false,
        qualified_wildcard_alias: true,
        from_first: false,
        explicit_table: true,
        parenthesized_query_operands: true,
        values_rows_require_equal_arity: false,
        values_row_constructor: true,
        as_alias_rejects_reserved: false,
        trailing_comma: false,
        prefix_colon_alias: false,
        lateral_view_clause: false,
        connect_by_clause: false,
    };
}

impl PredicateSyntax {
    /// Predicate syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        is_distinct_from: false,
        ilike: false,
        like: true,
        similar_to: true,
        overlaps_period_predicate: true,
        unparenthesized_in_list: false,
        pattern_match_quantifier: true,
        between_symmetric: true,
        is_normalized: true,
        empty_in_list: false,
        null_test_two_word_postfix: false,
    };
}

impl MutationSyntax {
    /// Mutation syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        insert_ignore: true,
        insert_overwrite: true,
        merge_insert_multirow: true,
        // Preserve modifier and joined-target structure for downstream validation.
        replace_into: true,
        update_delete_tails: true,
        joined_update_delete: true,
        or_conflict_action: true,
        returning: true,
        on_conflict: true,
        on_duplicate_key_update: false,
        multi_column_assignment: true,
        update_tuple_value_row_arity: false,
        where_current_of: true,
        merge: true,
        insert_set: false,
        insert_column_matching: false,
        delete_using: true,
        update_from: true,
        delete_using_target_alias: true,
        cte_before_insert: true,
        cte_before_merge: true,
        data_modifying_ctes: true,
        merge_when_not_matched_by: true,
        merge_insert_default_values: true,
        merge_insert_overriding: true,
        merge_update_set_star: false,
        merge_insert_star_by_name: false,
        merge_error_action: false,
        update_set_qualified_column: true,
    };
}

impl IndexAlterSyntax {
    /// Index and extended `ALTER` syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        drop_primary_key: true,
        alter_column_add_identity: true,
        rename_constraint: true,
        alter_table_set_options: true,
        index_storage_parameters: true,
        drop_behavior: true,
        index_drop_on_table: false,
        index_concurrently: true,
        index_using_method: true,
        partial_index: true,
        index_if_not_exists: true,
        index_nulls_order: true,
        alter_table_extended: true,
        alter_nested_column_paths: false,
        alter_existence_guards: true,
        alter_column_set_data_type: true,
        routine_arg_types: true,
        routine_arg_defaults: true,
        routine_arg_modes: true,
        routine_language_string: true,
        alter_table_multiple_actions: true,
    };
}

impl ColumnDefinitionSyntax {
    /// Column-definition syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        compact_identity_columns: true,
        generated_column_shorthand: false,
        column_conflict_resolution_clause: false,
        typeless_column_definitions: false,
        typeless_generated_columns: false,
        joined_autoincrement_attribute: false,
        inline_primary_key_ordering: false,
        named_column_collate_constraint: false,
        identity_columns: true,
        default_expression_requires_parens: false,
        column_default_requires_b_expr: true,
        column_collation: true,
        column_storage: true,
    };
}

impl StatementDdlGates {
    /// Statement-level DDL productions enabled by this preset.
    pub const QUILTDB: Self = Self {

        colocation_groups: true,
        create_trigger: false,
        create_macro: false,
        create_secret: false,
        create_type: false,
        create_virtual_table: false,
        create_sequence: true,
        extension_ddl: true,
        transform_ddl: true,
        alter_system: true,
        tablespace_ddl: false,
        logfile_group_ddl: false,
        schemas: true,
        schema_elements: true,
        databases: true,
        drop_database: false,
        materialized_views: true,
        routines: true,
        or_replace: true,
        create_or_replace_table: false,
        compound_statements: false,
        alter_database: false,
        alter_database_options: false,
        server_definition: false,
        alter_instance: false,
        spatial_reference_system: false,
        resource_group: false,
        alter_sequence: false,
        alter_object_set_schema: false,
};
}
impl ViewSequenceClauseSyntax {
    /// View/sequence clause surface for the `QUILTDB` preset.
    pub const QUILTDB: Self = Self {
        materialized_view_to: true,
        create_sequence_cache: true,
        temporary_views: true,
        recursive_views: false,
        view_definition_options: false,
    };
}


impl TypeNameSyntax {
    /// Type-name syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        extended_scalar_type_names: true,
        enum_type: true,
        set_type: true,
        numeric_modifiers: true,
        angle_bracket_types: true,
        composite_types: true,
        nullable_type: true,
        low_cardinality_type: true,
        fixed_string_type: true,
        datetime64_type: true,
        bit_width_integer_names: true,
        liberal_type_names: true,
        string_type_modifiers: true,
        integer_display_width: false,
        varchar_requires_length: false,
        zoned_temporal_types: true,
        empty_type_parens: false,
        character_set_annotation: false,
        signed_type_modifier: true,
        nested_type: false,
    };
}

impl TableExpressionSyntax {
    /// Table-expression syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        table_sample: false,
        only: true,
        parenthesized_joins: true,
        table_alias_column_lists: true,
        join_using_alias: true,
        index_hints: false,
        table_hints: false,
        partition_selection: false,
        base_table_alias_column_lists: true,
        string_literal_aliases: false,
        aliased_parenthesized_join: true,
        bare_table_alias_is_bare_label: false,
        table_version: false,
        table_json_path: false,
        indexed_by: false,
        prefix_colon_alias: false,
    };
}

impl UtilitySyntax {
    /// Utility-statement syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        comment_if_exists: true,
        copy: true,
        copy_into: false,
        stage_references: false,
        comment_on: true,
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
        use_string_literal_name: false,
        prepared_statements: true,
        prepare_typed_parameters: true,
        prepared_statements_from: false,
        call: false,
        call_bare_name: false,
        load_extension: true,
        load_bare_name: false,
        load_data: false,
        reset_scope: false,
        detach_if_exists: false,
        do_statement: true,
        do_expression_list: false,
        lock_tables: false,
        lock_instance: false,
        rename_statement: false,
        signal_diagnostics: false,
        export_import_database: false,
        update_extensions: false,
        flush: false,
        purge_binary_logs: false,
        replication_statements: false,
};
}
impl TransactionSyntax {
    /// Transaction-control surface for the `QUILTDB` preset (split from UtilitySyntax).
    pub const QUILTDB: Self = Self {
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
        transaction_modes_require_commas: false,
        transaction_modes_reject_duplicates: false,
        abort_transaction_alias: true,
        end_transaction_alias: true,
        transaction_release: false,
        transaction_chain: true,
        release_savepoint_keyword_optional: true,
        begin_transaction_mode: false,
        xa_transactions: false,
    };
}


impl FeatureSet {
    /// The complete QuiltDB feature set.
    pub const QUILTDB: Self = Self {
        access_control_syntax: AccessControlSyntax::QUILTDB,
        expression_syntax: ExpressionSyntax::QUILTDB,
        index_alter_syntax: IndexAlterSyntax::QUILTDB,
        column_definition_syntax: ColumnDefinitionSyntax::QUILTDB,
        statement_ddl_gates: StatementDdlGates::QUILTDB,
        view_sequence_clause_syntax: ViewSequenceClauseSyntax::QUILTDB,
        table_expressions: TableExpressionSyntax::QUILTDB,
        mutation_syntax: MutationSyntax::QUILTDB,
        predicate_syntax: PredicateSyntax::QUILTDB,
        select_syntax: SelectSyntax::QUILTDB,
        type_name_syntax: TypeNameSyntax::QUILTDB,
        utility_syntax: UtilitySyntax::QUILTDB,
        transaction_syntax: TransactionSyntax::QUILTDB,
        identifier_casing: Casing::Lower,
        identifier_quotes: STANDARD_IDENTIFIER_QUOTES,
        default_null_ordering: NullOrdering::NullsLast,
        reserved_column_name: RESERVED_COLUMN_NAME,
        reserved_function_name: RESERVED_FUNCTION_NAME,
        reserved_type_name: RESERVED_TYPE_NAME,
        reserved_bare_alias: RESERVED_BARE_ALIAS,
        reserved_as_label: KeywordSet::EMPTY,
        catalog_qualified_names: true,
        byte_classes: POSTGRES_BYTE_CLASSES,
        binding_powers: FeatureSet::POSTGRES.binding_powers, // shared with Postgres (byte-identical; avoid dual SoT)
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        string_literals: StringLiteralSyntax::POSTGRES,
        numeric_literals: NumericLiteralSyntax::POSTGRES,
        parameters: ParameterSyntax::POSTGRES,
        session_variables: SessionVariableSyntax::ANSI,
        identifier_syntax: IdentifierSyntax::POSTGRES,
        join_syntax: JoinSyntax::POSTGRES,
        table_factor_syntax: TableFactorSyntax::POSTGRES,
        operator_syntax: OperatorSyntax::POSTGRES,
        call_syntax: CallSyntax::POSTGRES,
        string_func_forms: StringFuncForms::POSTGRES,
        aggregate_call_syntax: AggregateCallSyntax::POSTGRES,
        pipe_operator: PipeOperator::StringConcat,
        double_ampersand: DoubleAmpersand::Unsupported,
        keyword_operators: KeywordOperators::Unsupported,
        caret_operator: CaretOperator::Exponent,
        hash_bitwise_xor: true,
        comment_syntax: CommentSyntax::POSTGRES,
        create_table_clause_syntax: CreateTableClauseSyntax::POSTGRES,
        constraint_syntax: ConstraintSyntax::POSTGRES,
        existence_guards: ExistenceGuards::POSTGRES,
        query_tail_syntax: QueryTailSyntax::POSTGRES,
        grouping_syntax: GroupingSyntax::POSTGRES,
        show_syntax: ShowSyntax::POSTGRES,
        maintenance_syntax: MaintenanceSyntax::POSTGRES,
        target_spelling: TargetSpelling::Postgres,
    };
}

/// Prefer [`FeatureSet::QUILTDB`] for struct update.
pub const QUILTDB: FeatureSet = FeatureSet::QUILTDB;

const _: () = assert!(FeatureSet::QUILTDB.is_lexically_consistent());
const _: () = assert!(FeatureSet::QUILTDB.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::QUILTDB.has_no_grammar_conflict());
