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
//! It narrows the inherited query grammar by rejecting wildcard `EXCLUDE`/`RENAME`,
//! `ILIKE`, `IS [NOT] DISTINCT FROM`, `NATURAL CROSS JOIN`, `TABLESAMPLE`, and
//! `INTERSECT ALL`/`EXCEPT ALL`. Alternative DDL spellings such as `MODIFY COLUMN`,
//! `CHANGE COLUMN`, `SET TBLPROPERTIES`, table-scoped `DROP INDEX`, and trailing
//! index `USING` remain outside the grammar.

use super::{
    AccessControlSyntax, ExpressionSyntax, FeatureSet, IndexAlterSyntax, MutationSyntax,
    PredicateSyntax, SelectSyntax, StatementDdlGates, TableExpressionSyntax, TypeNameSyntax,
    UtilitySyntax,
};

impl AccessControlSyntax {
    /// Access-control syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        // Preserve user/role statement structure for downstream validation. The richer
        // PostgreSQL GRANT/REVOKE route remains selected.
        user_role_management: true,
        ..Self::POSTGRES
    };
}

impl ExpressionSyntax {
    /// Expression syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        struct_constructor: true,
        collection_literals: true,
        ..Self::POSTGRES
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
        ..Self::POSTGRES
    };
}

impl PredicateSyntax {
    /// Predicate syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        is_distinct_from: false,
        ilike: false,
        ..Self::POSTGRES
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
        ..Self::POSTGRES
    };
}

impl IndexAlterSyntax {
    /// Index and extended `ALTER` syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        drop_primary_key: true,
        alter_column_add_identity: true,
        ..Self::POSTGRES
    };
}

impl super::ColumnDefinitionSyntax {
    /// Column-definition syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        compact_identity_columns: true,
        ..Self::POSTGRES
    };
}

impl StatementDdlGates {
    /// Statement-level DDL productions enabled by this preset.
    pub const QUILTDB: Self = Self {
        colocation_groups: true,
        materialized_view_to: true,
        create_sequence_cache: true,
        ..Self::POSTGRES
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
        ..Self::POSTGRES
    };
}

impl TableExpressionSyntax {
    /// Table-expression syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        table_sample: false,
        ..Self::POSTGRES
    };
}

impl UtilitySyntax {
    /// Utility-statement syntax enabled by this preset.
    pub const QUILTDB: Self = Self {
        comment_if_exists: true,
        ..Self::POSTGRES
    };
}

impl FeatureSet {
    /// The complete QuiltDB feature set.
    pub const QUILTDB: Self = Self {
        access_control_syntax: AccessControlSyntax::QUILTDB,
        expression_syntax: ExpressionSyntax::QUILTDB,
        index_alter_syntax: IndexAlterSyntax::QUILTDB,
        column_definition_syntax: super::ColumnDefinitionSyntax::QUILTDB,
        statement_ddl_gates: StatementDdlGates::QUILTDB,
        table_expressions: TableExpressionSyntax::QUILTDB,
        mutation_syntax: MutationSyntax::QUILTDB,
        predicate_syntax: PredicateSyntax::QUILTDB,
        select_syntax: SelectSyntax::QUILTDB,
        type_name_syntax: TypeNameSyntax::QUILTDB,
        utility_syntax: UtilitySyntax::QUILTDB,
        ..Self::POSTGRES
    };
}

/// Prefer [`FeatureSet::QUILTDB`] for struct update.
pub const QUILTDB: FeatureSet = FeatureSet::QUILTDB;

const _: () = assert!(FeatureSet::QUILTDB.is_lexically_consistent());
const _: () = assert!(FeatureSet::QUILTDB.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::QUILTDB.has_no_grammar_conflict());
