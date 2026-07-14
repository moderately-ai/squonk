// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Feature-labelled cases (prod-coverage-required-features-labels): the sub-flag arrays,
//! [`ToggleableFeature`] + the `toggleable_features!` table, [`Expect`] / [`LabeledCase`] /
//! [`LABELED_CASES`], the labelled shape predicates, and the feature-flip harness fns.

use super::harness::*;
use super::*;

// --- feature-labelled cases (prod-coverage-required-features-labels) ----------
//
// Beyond "feature X has >=1 positive and negative case" (the matrix above), each
// gated construct is tied to the dialect features it *exercises*: a `LabeledCase`
// declares the features it requires (or forbids), so coverage is traceable in the
// registry's vocabulary and a partial dialect can skip cases it cannot satisfy
// instead of failing (ZetaSQL's skip-vs-fail). A verification pass flips each
// declared feature to prove the label is genuine, not decorative (the
// falsely-required check). This generalizes the per-sub-flag coverage of
// prod-coverage-subflag-granularity: every gated `FeatureSet` sub-flag is a
// `ToggleableFeature` and must be required by some `LabeledCase`.

const STRING_LITERAL_SUBFLAGS: &[&str] = &[
    "escape_strings",
    "dollar_quoted_strings",
    "national_strings",
    "double_quoted_strings",
    "backslash_escapes",
    "unicode_strings",
    "bit_string_literals",
    "blob_literals",
    "charset_introducers",
    "same_line_adjacent_concat",
];

const NUMERIC_LITERAL_SUBFLAGS: &[&str] = &[
    "hex_integers",
    "octal_integers",
    "binary_integers",
    "underscore_separators",
    "radix_leading_underscore",
    "money_literals",
    "reject_trailing_junk",
];

const TABLE_EXPRESSION_SUBFLAGS: &[&str] = &[
    "only",
    "table_sample",
    "parenthesized_joins",
    "table_alias_column_lists",
    "join_using_alias",
    "index_hints",
    "table_hints",
    "partition_selection",
    "base_table_alias_column_lists",
    "string_literal_aliases",
    "aliased_parenthesized_join",
];
const JOIN_SUBFLAGS: &[&str] = &[
    "stacked_join_qualifiers",
    "full_outer_join",
    "natural_cross_join",
    "straight_join",
    "asof_join",
    "positional_join",
    "semi_anti_join",
    "sided_semi_anti_join",
    "recursive_search_cycle",
    "recursive_union_rejects_order_limit",
];
const TABLE_FACTOR_SUBFLAGS: &[&str] = &[
    "lateral",
    "table_functions",
    "rows_from",
    "unnest",
    "unnest_with_offset",
    "table_function_ordinality",
    "pivot",
    "unpivot",
    "show_ref",
    "from_values",
    "special_function_table_source",
    "json_table",
    "xml_table",
    "table_expr_factor",
    "pivot_value_sources",
    "match_recognize",
    "open_json",
];

const COMMENT_SUBFLAGS: &[&str] = &[
    "line_comment_hash",
    "line_comment_ends_at_carriage_return",
    "nested_block_comments",
    "versioned_comments",
    "unterminated_block_comment_at_eof",
];

const PARAMETER_SUBFLAGS: &[&str] = &[
    "positional_dollar",
    "anonymous_question",
    "named_colon",
    "named_at",
    "numbered_question",
];

const SESSION_VARIABLE_SUBFLAGS: &[&str] =
    &["user_variables", "system_variables", "variable_assignment"];

const IDENTIFIER_SUBFLAGS: &[&str] = &[
    "dollar_in_identifiers",
    "string_literal_identifiers",
    "empty_quoted_identifiers",
];

const MUTATION_SUBFLAGS: &[&str] = &[
    "returning",
    "on_conflict",
    "on_duplicate_key_update",
    "multi_column_assignment",
    "where_current_of",
    "merge",
    "replace_into",
    "insert_set",
    "update_delete_tails",
    "or_conflict_action",
    "delete_using",
    "update_from",
    "delete_using_target_alias",
    "cte_before_insert",
    "cte_before_merge",
    "data_modifying_ctes",
    "merge_when_not_matched_by",
    "merge_insert_default_values",
    "merge_insert_overriding",
];
const STATEMENT_DDL_GATES_SUBFLAGS: &[&str] = &[
    "create_trigger",
    "create_macro",
    "create_secret",
    "create_type",
    "create_virtual_table",
    "create_sequence",
    "schemas",
    "databases",
    "materialized_views",
    "temporary_views",
    "routines",
    "compound_statements",
    "or_replace",
    "spatial_reference_system",
    "resource_group",
    "extension_ddl",
    "transform_ddl",
    "alter_system",
    "tablespace_ddl",
    "logfile_group_ddl",
    "schema_elements",
    "drop_database",
    "recursive_views",
    "alter_database",
    "alter_database_options",
    "server_definition",
    "alter_instance",
    "alter_sequence",
    "alter_object_set_schema",
    "view_definition_options",
];
const CREATE_TABLE_CLAUSE_SUBFLAGS: &[&str] = &[
    "table_options",
    "without_rowid_table_option",
    "strict_table_option",
    "create_or_replace_table",
    "storage_parameters",
    "on_commit",
    "create_table_as_with_data",
    "declarative_partitioning",
    "table_inheritance",
    "like_source_table",
    "statement_level_table_like",
    "unlogged_tables",
    "table_access_method",
    "without_oids",
    "typed_tables",
    "create_table_as_execute",
];
const COLUMN_DEFINITION_SUBFLAGS: &[&str] = &[
    "generated_column_shorthand",
    "column_conflict_resolution_clause",
    "typeless_column_definitions",
    "joined_autoincrement_attribute",
    "inline_primary_key_ordering",
    "named_column_collate_constraint",
    "identity_columns",
    "default_expression_requires_parens",
    "column_collation",
    "column_storage",
];
const CONSTRAINT_SUBFLAGS: &[&str] = &[
    "deferrable_constraints",
    "named_inline_non_check_constraints",
    "bare_constraint_name",
    "exclusion_constraints",
    "constraint_no_inherit_not_valid",
    "index_constraint_parameters",
    "constraint_column_collate_order",
];
const INDEX_ALTER_SUBFLAGS: &[&str] = &[
    "drop_behavior",
    "index_drop_on_table",
    "index_concurrently",
    "index_using_method",
    "partial_index",
    "alter_table_extended",
    "index_if_not_exists",
    "index_nulls_order",
    "routine_arg_types",
    "routine_arg_defaults",
    "routine_arg_modes",
    "alter_existence_guards",
    "alter_column_set_data_type",
];
// `view_if_not_exists` is deliberately *not* here: like the pre-restructure bundle, the
// plain-`CREATE VIEW` guard has no independent accept/reject coverage case, so it stays a
// non-toggleable field (bound `_` in the enumeration guard below).
const EXISTENCE_GUARDS_SUBFLAGS: &[&str] = &["if_exists", "create_database_if_not_exists"];
const SELECT_SUBFLAGS: &[&str] = &[
    "distinct_on",
    "select_into",
    "empty_target_list",
    "qualify",
    "alias_string_literals",
    "bare_alias_string_literals",
    "union_by_name",
    "from_first",
    "wildcard_modifiers",
    "qualified_wildcard_alias",
    "parenthesized_query_operands",
    "values_rows_require_equal_arity",
    "values_row_constructor",
    "as_alias_rejects_reserved",
];
const QUERY_TAIL_SUBFLAGS: &[&str] = &[
    "fetch_first",
    "limit_offset_comma",
    "using_sample",
    "locking_clauses",
    "key_lock_strengths",
    "stacked_locking_clauses",
    "leading_offset",
    "limit_expressions",
    "limit_percent",
    "pipe_syntax",
];
const GROUPING_SUBFLAGS: &[&str] = &[
    "grouping_sets",
    "with_rollup",
    "order_by_using",
    "group_by_all",
    "order_by_all",
];
const UTILITY_SUBFLAGS: &[&str] = &[
    "copy",
    "copy_into",
    "comment_on",
    "pragma",
    "attach",
    "kill",
    "handler_statements",
    "plugin_component_statements",
    "shutdown",
    "restart",
    "clone",
    "import_table",
    "help_statement",
    "binlog",
    "key_cache_statements",
    "use_statement",
    "prepared_statements",
    "prepare_typed_parameters",
    "prepared_statements_from",
    "call",
    "call_bare_name",
    "load_extension",
    "load_bare_name",
    "reset_scope",
    "detach_if_exists",
    "do_statement",
    "begin_transaction_mode",
    "xa_transactions",
    "rename_statement",
    "flush",
    "purge_binary_logs",
    "replication_statements",
    "use_qualified_name",
    "load_data",
    "do_expression_list",
    "lock_tables",
    "lock_instance",
    "stage_references",
    "signal_diagnostics",
    "export_import_database",
    "update_extensions",
];
const SHOW_SUBFLAGS: &[&str] = &[
    "describe",
    "describe_summarize",
    "session_statements",
    "show_tables",
    "show_columns",
    "show_create_table",
    "show_functions",
    "show_routine_status",
    "show_verbose",
    "show_admin",
];
const MAINTENANCE_SUBFLAGS: &[&str] = &[
    "vacuum",
    "vacuum_analyze",
    "reindex",
    "analyze",
    "analyze_columns",
    "checkpoint",
    "checkpoint_database",
    "table_maintenance",
];
const ACCESS_CONTROL_SUBFLAGS: &[&str] = &[
    "access_control",
    "access_control_extended_objects",
    "user_role_management",
    "access_control_account_grants",
];
const TYPE_NAME_SUBFLAGS: &[&str] = &[
    "extended_scalar_type_names",
    "enum_type",
    "set_type",
    "numeric_modifiers",
    "integer_display_width",
    "composite_types",
    "varchar_requires_length",
    "zoned_temporal_types",
    "empty_type_parens",
    "character_set_annotation",
    "signed_type_modifier",
    "liberal_type_names",
    "string_type_modifiers",
];
const EXPRESSION_SYNTAX_SUBFLAGS: &[&str] = &[
    "typecast_operator",
    "subscript",
    "slice_step",
    "collate",
    "at_time_zone",
    "semi_structured_access",
    "array_constructor",
    "collection_literals",
    "row_constructor",
    "struct_constructor",
    "field_selection",
    "field_wildcard",
    "typed_string_literals",
    "typed_interval_literal",
    "relaxed_interval_syntax",
    "mysql_interval_operator",
    "positional_column",
    "lambda_keyword",
];
// `double_equals`, `integer_divide_slash`, and `is_general_equality` are deliberately not
// here: like the pre-split ExpressionSyntax, the SQLite/DuckDB-only equality (`==`) and
// integer-division (`//`) *spellings* fold onto existing operators and carry no independent
// accept/reject coverage case, so they stay non-toggleable fields (bound `_` in the
// enumeration guard below).
const OPERATOR_SYNTAX_SUBFLAGS: &[&str] = &[
    "operator_construct",
    "containment_operators",
    "json_arrow_operators",
    "jsonb_operators",
    "truth_value_tests",
    "null_safe_equals",
    "lambda_expressions",
    "bitwise_operators",
    "quantified_comparisons",
    "custom_operators",
    "postfix_operators",
];
const CALL_SYNTAX_SUBFLAGS: &[&str] = &[
    "named_argument",
    "variadic_argument",
    "utc_special_functions",
    "columns_expression",
    "extract_from_syntax",
    "try_cast",
    "restricted_cast_targets",
    "extract_string_field",
    "method_chaining",
    "sqljson_expression_functions",
    "xml_expression_functions",
];
const STRING_FUNC_FORMS_SUBFLAGS: &[&str] = &[
    "substring_from_for",
    "substring_leading_for",
    "substring_similar",
    "substring_plain_call_requires_2_or_3_args",
    "substr_from_for",
    "position_in",
    "position_asymmetric_operands",
    "overlay_placing",
    "overlay_requires_placing",
    "trim_from",
    "trim_list_syntax",
    "ceil_to_field",
    "floor_to_field",
];
const AGGREGATE_CALL_SYNTAX_SUBFLAGS: &[&str] = &[
    "group_concat_separator",
    "within_group",
    "aggregate_filter",
    "aggregate_args_require_adjacent_paren",
    "null_treatment",
    "aggregate_calls_reject_empty_arguments",
    "over_requires_windowable_function",
];
const PREDICATE_SUBFLAGS: &[&str] = &["like", "ilike", "similar_to", "unparenthesized_in_list"];

const COMPOSITE_SUBFLAGS: &[(Feature, &[&str])] = &[
    (Feature::StringLiterals, STRING_LITERAL_SUBFLAGS),
    (Feature::NumericLiterals, NUMERIC_LITERAL_SUBFLAGS),
    (Feature::CommentSyntax, COMMENT_SUBFLAGS),
    (Feature::Parameters, PARAMETER_SUBFLAGS),
    (Feature::SessionVariables, SESSION_VARIABLE_SUBFLAGS),
    (Feature::IdentifierSyntax, IDENTIFIER_SUBFLAGS),
    (Feature::TableExpressions, TABLE_EXPRESSION_SUBFLAGS),
    (Feature::JoinSyntax, JOIN_SUBFLAGS),
    (Feature::TableFactorSyntax, TABLE_FACTOR_SUBFLAGS),
    (Feature::MutationSyntax, MUTATION_SUBFLAGS),
    (Feature::StatementDdlGates, STATEMENT_DDL_GATES_SUBFLAGS),
    (
        Feature::CreateTableClauseSyntax,
        CREATE_TABLE_CLAUSE_SUBFLAGS,
    ),
    (Feature::ColumnDefinitionSyntax, COLUMN_DEFINITION_SUBFLAGS),
    (Feature::ConstraintSyntax, CONSTRAINT_SUBFLAGS),
    (Feature::IndexAlterSyntax, INDEX_ALTER_SUBFLAGS),
    (Feature::ExistenceGuards, EXISTENCE_GUARDS_SUBFLAGS),
    (Feature::ExpressionSyntax, EXPRESSION_SYNTAX_SUBFLAGS),
    (Feature::OperatorSyntax, OPERATOR_SYNTAX_SUBFLAGS),
    (Feature::CallSyntax, CALL_SYNTAX_SUBFLAGS),
    (Feature::StringFuncForms, STRING_FUNC_FORMS_SUBFLAGS),
    (Feature::AggregateCallSyntax, AGGREGATE_CALL_SYNTAX_SUBFLAGS),
    (Feature::PredicateSyntax, PREDICATE_SUBFLAGS),
    (Feature::SelectSyntax, SELECT_SUBFLAGS),
    (Feature::QueryTailSyntax, QUERY_TAIL_SUBFLAGS),
    (Feature::GroupingSyntax, GROUPING_SUBFLAGS),
    (Feature::UtilitySyntax, UTILITY_SUBFLAGS),
    (Feature::ShowSyntax, SHOW_SUBFLAGS),
    (Feature::MaintenanceSyntax, MAINTENANCE_SUBFLAGS),
    (Feature::AccessControlSyntax, ACCESS_CONTROL_SUBFLAGS),
    (Feature::TypeNameSyntax, TYPE_NAME_SUBFLAGS),
];

/// A dialect-data feature that can be independently toggled on a `FeatureSet`, so a
/// case's label is *executable*: we can ask whether a dialect enables it and flip it
/// for the falsely-declared verification pass.
struct ToggleableFeature {
    /// `FeatureSet` sub-flag name — the stable local id and coverage-matrix key.
    sub_flag: &'static str,
    /// Owning dialect-data knob (coverage-matrix cross-reference).
    feature: Feature,
    /// `STANDARD_FEATURE_CATALOG` id when this sub-flag is catalogued (the two
    /// PostgreSQL string-literal extensions); `None` otherwise, where `sub_flag`
    /// is itself the stable id.
    catalog_id: Option<&'static str>,
    /// Whether `features` enables this feature.
    is_enabled: fn(&FeatureSet) -> bool,
    /// `features` with exactly this feature forced to `enabled`.
    set_enabled: fn(&FeatureSet, bool) -> FeatureSet,
}

/// Constructs the common `ToggleableFeature` shape shared by every additive
/// boolean sub-flag: the struct literal is identical across all of them modulo the
/// const name, sub-flag id, owning `Feature`/category, the field that varies, and
/// an optional catalogue id — this is what turns ~880 lines of copy-pasted structs
/// into data. The non-boolean `versioned_comments` (an `Option<u32>` bound) and the
/// enum-valued `LOGICAL_OR_PIPE` stay hand-written below.
macro_rules! toggleable_features {
    ($(($name:ident, $sub_flag:literal, $feature:ident, $field:ident, $ty:ident, $bool_field:ident $(, catalog_id: $catalog_id:literal)?)),+ $(,)?) => {
        $(
            const $name: ToggleableFeature = ToggleableFeature {
                sub_flag: $sub_flag,
                feature: Feature::$feature,
                catalog_id: toggleable_features!(@catalog_id $($catalog_id)?),
                is_enabled: |features| features.$field.$bool_field,
                set_enabled: |features, on| {
                    features.with(FeatureDelta::EMPTY.$field($ty {
                        $bool_field: on,
                        ..features.$field
                    }))
                },
            };
        )+
    };
    (@catalog_id) => { None };
    (@catalog_id $catalog_id:literal) => { Some($catalog_id) };
}

toggleable_features! {
    // `line_comment_hash` is hand-written below (coupled with `hash_bitwise_xor` — both claim
    // the `#` trigger).
    (LINE_COMMENT_ENDS_AT_CARRIAGE_RETURN, "line_comment_ends_at_carriage_return", CommentSyntax, comment_syntax, CommentSyntax, line_comment_ends_at_carriage_return),
    (NESTED_BLOCK_COMMENTS, "nested_block_comments", CommentSyntax, comment_syntax, CommentSyntax, nested_block_comments),
    (UNTERMINATED_BLOCK_COMMENT_AT_EOF, "unterminated_block_comment_at_eof", CommentSyntax, comment_syntax, CommentSyntax, unterminated_block_comment_at_eof),
    (ESCAPE_STRINGS, "escape_strings", StringLiterals, string_literals, StringLiteralSyntax, escape_strings, catalog_id: "pg:escape-string-syntax"),
    (DOLLAR_QUOTED, "dollar_quoted_strings", StringLiterals, string_literals, StringLiteralSyntax, dollar_quoted_strings, catalog_id: "pg:dollar-quoted-strings"),
    (NATIONAL_STRINGS, "national_strings", StringLiterals, string_literals, StringLiteralSyntax, national_strings),
    // `double_quoted_strings` is hand-written below (not an independent additive flag —
    // it is coupled with `"` identifier quoting).
    (BACKSLASH_ESCAPES, "backslash_escapes", StringLiterals, string_literals, StringLiteralSyntax, backslash_escapes),
    (UNICODE_STRINGS, "unicode_strings", StringLiterals, string_literals, StringLiteralSyntax, unicode_strings),
    (BIT_STRING_LITERALS, "bit_string_literals", StringLiterals, string_literals, StringLiteralSyntax, bit_string_literals),
    (BLOB_LITERALS, "blob_literals", StringLiterals, string_literals, StringLiteralSyntax, blob_literals),
    (CHARSET_INTRODUCERS, "charset_introducers", StringLiterals, string_literals, StringLiteralSyntax, charset_introducers),
    (SAME_LINE_ADJACENT_CONCAT, "same_line_adjacent_concat", StringLiterals, string_literals, StringLiteralSyntax, same_line_adjacent_concat),
    (HEX_INTEGERS, "hex_integers", NumericLiterals, numeric_literals, NumericLiteralSyntax, hex_integers),
    (OCTAL_INTEGERS, "octal_integers", NumericLiterals, numeric_literals, NumericLiteralSyntax, octal_integers),
    (BINARY_INTEGERS, "binary_integers", NumericLiterals, numeric_literals, NumericLiteralSyntax, binary_integers),
    (UNDERSCORE_SEPARATORS, "underscore_separators", NumericLiterals, numeric_literals, NumericLiteralSyntax, underscore_separators),
    (RADIX_LEADING_UNDERSCORE, "radix_leading_underscore", NumericLiterals, numeric_literals, NumericLiteralSyntax, radix_leading_underscore),
    // A restrictive (negative-polarity) sub-flag, like `aggregate_calls_reject_empty_arguments`:
    // enabling it makes `123abc` *reject*, so its LabeledCase expects Reject.
    (REJECT_TRAILING_JUNK, "reject_trailing_junk", NumericLiterals, numeric_literals, NumericLiteralSyntax, reject_trailing_junk),
    // `money_literals` is hand-written below (coupled with `positional_dollar` — both
    // claim `$`+digit).
    (POSITIONAL_DOLLAR, "positional_dollar", Parameters, parameters, ParameterSyntax, positional_dollar),
    // `numbered_question` (`?NNN`) is follow-set-disjoint from the anonymous `?` (needs a
    // digit) and from the `jsonb` `?`/`?|`/`?&` operators (never `?`+digit), exactly like
    // `positional_dollar` (`$`+digit) coexisting with dollar-quoting — a plain additive flag.
    (NUMBERED_QUESTION, "numbered_question", Parameters, parameters, ParameterSyntax, numbered_question),
    // `anonymous_question` is hand-written below (coupled with the `jsonb` `?` operator — both
    // claim the `?` trigger).
    // `named_colon`, `named_at`, and `user_variables` are hand-written below (each coupled
    // with the PostgreSQL feature it shares a `:`/`<@` trigger with).
    // `system_variables` is hand-written below (coupled with the `jsonb` `@@` operator — both
    // claim the `@@` trigger).
    (DOLLAR_IN_IDENTIFIERS, "dollar_in_identifiers", IdentifierSyntax, identifier_syntax, IdentifierSyntax, dollar_in_identifiers),
    (STRING_LITERAL_IDENTIFIERS, "string_literal_identifiers", IdentifierSyntax, identifier_syntax, IdentifierSyntax, string_literal_identifiers),
    (EMPTY_QUOTED_IDENTIFIERS, "empty_quoted_identifiers", IdentifierSyntax, identifier_syntax, IdentifierSyntax, empty_quoted_identifiers),
    (LATERAL, "lateral", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, lateral),
    (TABLE_FUNCTIONS, "table_functions", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, table_functions),
    (ROWS_FROM, "rows_from", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, rows_from),
    (UNNEST, "unnest", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, unnest),
    (UNNEST_WITH_OFFSET, "unnest_with_offset", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, unnest_with_offset),
    (TABLE_FUNCTION_ORDINALITY, "table_function_ordinality", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, table_function_ordinality),
    (ONLY, "only", TableExpressions, table_expressions, TableExpressionSyntax, only),
    (TABLE_SAMPLE, "table_sample", TableExpressions, table_expressions, TableExpressionSyntax, table_sample),
    (PARENTHESIZED_JOINS, "parenthesized_joins", TableExpressions, table_expressions, TableExpressionSyntax, parenthesized_joins),
    (TABLE_ALIAS_COLUMN_LISTS, "table_alias_column_lists", TableExpressions, table_expressions, TableExpressionSyntax, table_alias_column_lists),
    (JOIN_USING_ALIAS, "join_using_alias", TableExpressions, table_expressions, TableExpressionSyntax, join_using_alias),
    (STACKED_JOIN_QUALIFIERS, "stacked_join_qualifiers", JoinSyntax, join_syntax, JoinSyntax, stacked_join_qualifiers),
    (FULL_OUTER_JOIN, "full_outer_join", JoinSyntax, join_syntax, JoinSyntax, full_outer_join),
    (NATURAL_CROSS_JOIN, "natural_cross_join", JoinSyntax, join_syntax, JoinSyntax, natural_cross_join),
    (STRAIGHT_JOIN, "straight_join", JoinSyntax, join_syntax, JoinSyntax, straight_join),
    (ASOF_JOIN, "asof_join", JoinSyntax, join_syntax, JoinSyntax, asof_join),
    (POSITIONAL_JOIN, "positional_join", JoinSyntax, join_syntax, JoinSyntax, positional_join),
    (SEMI_ANTI_JOIN, "semi_anti_join", JoinSyntax, join_syntax, JoinSyntax, semi_anti_join),
    (SIDED_SEMI_ANTI_JOIN, "sided_semi_anti_join", JoinSyntax, join_syntax, JoinSyntax, sided_semi_anti_join),
    (INDEX_HINTS, "index_hints", TableExpressions, table_expressions, TableExpressionSyntax, index_hints),
    (TABLE_HINTS, "table_hints", TableExpressions, table_expressions, TableExpressionSyntax, table_hints),
    (PARTITION_SELECTION, "partition_selection", TableExpressions, table_expressions, TableExpressionSyntax, partition_selection),
    (PIVOT, "pivot", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, pivot),
    (UNPIVOT, "unpivot", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, unpivot),
    (SHOW_REF, "show_ref", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, show_ref),
    (FROM_VALUES, "from_values", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, from_values),
    (BASE_TABLE_ALIAS_COLUMN_LISTS, "base_table_alias_column_lists", TableExpressions, table_expressions, TableExpressionSyntax, base_table_alias_column_lists),
    (STRING_LITERAL_ALIASES, "string_literal_aliases", TableExpressions, table_expressions, TableExpressionSyntax, string_literal_aliases),
    (SPECIAL_FUNCTION_TABLE_SOURCE, "special_function_table_source", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, special_function_table_source),
    (ALIASED_PARENTHESIZED_JOIN, "aliased_parenthesized_join", TableExpressions, table_expressions, TableExpressionSyntax, aliased_parenthesized_join),
    (RECURSIVE_SEARCH_CYCLE, "recursive_search_cycle", JoinSyntax, join_syntax, JoinSyntax, recursive_search_cycle),
    // A restrictive (negative-polarity) sub-flag: enabling it makes a recursive CTE's
    // `UNION`-body `ORDER BY`/`LIMIT`/`OFFSET` *reject*, so its LabeledCase expects Reject.
    (RECURSIVE_UNION_REJECTS_ORDER_LIMIT, "recursive_union_rejects_order_limit", JoinSyntax, join_syntax, JoinSyntax, recursive_union_rejects_order_limit),
    (JSON_TABLE, "json_table", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, json_table),
    (XML_TABLE, "xml_table", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, xml_table),
    (TABLE_EXPR_FACTOR, "table_expr_factor", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, table_expr_factor),
    (PIVOT_VALUE_SOURCES, "pivot_value_sources", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, pivot_value_sources),
    (MATCH_RECOGNIZE, "match_recognize", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, match_recognize),
    (OPEN_JSON, "open_json", TableFactorSyntax, table_factor_syntax, TableFactorSyntax, open_json),
    (RETURNING, "returning", MutationSyntax, mutation_syntax, MutationSyntax, returning),
    (ON_CONFLICT, "on_conflict", MutationSyntax, mutation_syntax, MutationSyntax, on_conflict),
    (ON_DUPLICATE_KEY_UPDATE, "on_duplicate_key_update", MutationSyntax, mutation_syntax, MutationSyntax, on_duplicate_key_update),
    (IF_EXISTS, "if_exists", ExistenceGuards, existence_guards, ExistenceGuards, if_exists),
    (DROP_BEHAVIOR, "drop_behavior", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, drop_behavior),
    (MULTI_COLUMN_ASSIGNMENT, "multi_column_assignment", MutationSyntax, mutation_syntax, MutationSyntax, multi_column_assignment),
    (WHERE_CURRENT_OF, "where_current_of", MutationSyntax, mutation_syntax, MutationSyntax, where_current_of),
    (MERGE, "merge", MutationSyntax, mutation_syntax, MutationSyntax, merge),
    (REPLACE_INTO, "replace_into", MutationSyntax, mutation_syntax, MutationSyntax, replace_into),
    (INSERT_SET, "insert_set", MutationSyntax, mutation_syntax, MutationSyntax, insert_set),
    (UPDATE_DELETE_TAILS, "update_delete_tails", MutationSyntax, mutation_syntax, MutationSyntax, update_delete_tails),
    (OR_CONFLICT_ACTION, "or_conflict_action", MutationSyntax, mutation_syntax, MutationSyntax, or_conflict_action),
    (DELETE_USING, "delete_using", MutationSyntax, mutation_syntax, MutationSyntax, delete_using),
    (UPDATE_FROM, "update_from", MutationSyntax, mutation_syntax, MutationSyntax, update_from),
    (DELETE_USING_TARGET_ALIAS, "delete_using_target_alias", MutationSyntax, mutation_syntax, MutationSyntax, delete_using_target_alias),
    (CTE_BEFORE_INSERT, "cte_before_insert", MutationSyntax, mutation_syntax, MutationSyntax, cte_before_insert),
    (CTE_BEFORE_MERGE, "cte_before_merge", MutationSyntax, mutation_syntax, MutationSyntax, cte_before_merge),
    (DATA_MODIFYING_CTES, "data_modifying_ctes", MutationSyntax, mutation_syntax, MutationSyntax, data_modifying_ctes),
    (MERGE_WHEN_NOT_MATCHED_BY, "merge_when_not_matched_by", MutationSyntax, mutation_syntax, MutationSyntax, merge_when_not_matched_by),
    (MERGE_INSERT_DEFAULT_VALUES, "merge_insert_default_values", MutationSyntax, mutation_syntax, MutationSyntax, merge_insert_default_values),
    (MERGE_INSERT_OVERRIDING, "merge_insert_overriding", MutationSyntax, mutation_syntax, MutationSyntax, merge_insert_overriding),
    (INDEX_CONCURRENTLY, "index_concurrently", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, index_concurrently),
    (INDEX_USING_METHOD, "index_using_method", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, index_using_method),
    (PARTIAL_INDEX, "partial_index", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, partial_index),
    (TABLE_OPTIONS, "table_options", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, table_options),
    (CREATE_DATABASE_IF_NOT_EXISTS, "create_database_if_not_exists", ExistenceGuards, existence_guards, ExistenceGuards, create_database_if_not_exists),
    (GENERATED_COLUMN_SHORTHAND, "generated_column_shorthand", ColumnDefinitionSyntax, column_definition_syntax, ColumnDefinitionSyntax, generated_column_shorthand),
    (WITHOUT_ROWID_TABLE_OPTION, "without_rowid_table_option", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, without_rowid_table_option),
    (STRICT_TABLE_OPTION, "strict_table_option", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, strict_table_option),
    (COLUMN_CONFLICT_RESOLUTION_CLAUSE, "column_conflict_resolution_clause", ColumnDefinitionSyntax, column_definition_syntax, ColumnDefinitionSyntax, column_conflict_resolution_clause),
    (TYPELESS_COLUMN_DEFINITIONS, "typeless_column_definitions", ColumnDefinitionSyntax, column_definition_syntax, ColumnDefinitionSyntax, typeless_column_definitions),
    (JOINED_AUTOINCREMENT_ATTRIBUTE, "joined_autoincrement_attribute", ColumnDefinitionSyntax, column_definition_syntax, ColumnDefinitionSyntax, joined_autoincrement_attribute),
    (INLINE_PRIMARY_KEY_ORDERING, "inline_primary_key_ordering", ColumnDefinitionSyntax, column_definition_syntax, ColumnDefinitionSyntax, inline_primary_key_ordering),
    (NAMED_COLUMN_COLLATE_CONSTRAINT, "named_column_collate_constraint", ColumnDefinitionSyntax, column_definition_syntax, ColumnDefinitionSyntax, named_column_collate_constraint),
    (CREATE_TRIGGER, "create_trigger", StatementDdlGates, statement_ddl_gates, StatementDdlGates, create_trigger),
    (CREATE_MACRO, "create_macro", StatementDdlGates, statement_ddl_gates, StatementDdlGates, create_macro),
    (CREATE_OR_REPLACE_TABLE, "create_or_replace_table", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, create_or_replace_table),
    (CREATE_SECRET, "create_secret", StatementDdlGates, statement_ddl_gates, StatementDdlGates, create_secret),
    (CREATE_TYPE, "create_type", StatementDdlGates, statement_ddl_gates, StatementDdlGates, create_type),
    (CREATE_VIRTUAL_TABLE, "create_virtual_table", StatementDdlGates, statement_ddl_gates, StatementDdlGates, create_virtual_table),
    (CREATE_SEQUENCE, "create_sequence", StatementDdlGates, statement_ddl_gates, StatementDdlGates, create_sequence),
    (SCHEMAS, "schemas", StatementDdlGates, statement_ddl_gates, StatementDdlGates, schemas),
    (DATABASES, "databases", StatementDdlGates, statement_ddl_gates, StatementDdlGates, databases),
    (MATERIALIZED_VIEWS, "materialized_views", StatementDdlGates, statement_ddl_gates, StatementDdlGates, materialized_views),
    (TEMPORARY_VIEWS, "temporary_views", StatementDdlGates, statement_ddl_gates, StatementDdlGates, temporary_views),
    (ROUTINES, "routines", StatementDdlGates, statement_ddl_gates, StatementDdlGates, routines),
    (COMPOUND_STATEMENTS, "compound_statements", StatementDdlGates, statement_ddl_gates, StatementDdlGates, compound_statements),
    (SPATIAL_REFERENCE_SYSTEM, "spatial_reference_system", StatementDdlGates, statement_ddl_gates, StatementDdlGates, spatial_reference_system),
    (RESOURCE_GROUP, "resource_group", StatementDdlGates, statement_ddl_gates, StatementDdlGates, resource_group),
    (OR_REPLACE, "or_replace", StatementDdlGates, statement_ddl_gates, StatementDdlGates, or_replace),
    (IDENTITY_COLUMNS, "identity_columns", ColumnDefinitionSyntax, column_definition_syntax, ColumnDefinitionSyntax, identity_columns),
    (STORAGE_PARAMETERS, "storage_parameters", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, storage_parameters),
    (ON_COMMIT, "on_commit", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, on_commit),
    (ALTER_TABLE_EXTENDED, "alter_table_extended", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, alter_table_extended),
    (INDEX_IF_NOT_EXISTS, "index_if_not_exists", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, index_if_not_exists),
    (INDEX_NULLS_ORDER, "index_nulls_order", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, index_nulls_order),
    (ROUTINE_ARG_TYPES, "routine_arg_types", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, routine_arg_types),
    (ROUTINE_ARG_DEFAULTS, "routine_arg_defaults", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, routine_arg_defaults),
    (ROUTINE_ARG_MODES, "routine_arg_modes", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, routine_arg_modes),
    (ALTER_EXISTENCE_GUARDS, "alter_existence_guards", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, alter_existence_guards),
    (ALTER_COLUMN_SET_DATA_TYPE, "alter_column_set_data_type", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, alter_column_set_data_type),
    (DEFERRABLE_CONSTRAINTS, "deferrable_constraints", ConstraintSyntax, constraint_syntax, ConstraintSyntax, deferrable_constraints),
    (CREATE_TABLE_AS_WITH_DATA, "create_table_as_with_data", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, create_table_as_with_data),
    (DEFAULT_EXPRESSION_REQUIRES_PARENS, "default_expression_requires_parens", ColumnDefinitionSyntax, column_definition_syntax, ColumnDefinitionSyntax, default_expression_requires_parens),
    (NAMED_INLINE_NON_CHECK_CONSTRAINTS, "named_inline_non_check_constraints", ConstraintSyntax, constraint_syntax, ConstraintSyntax, named_inline_non_check_constraints),
    (BARE_CONSTRAINT_NAME, "bare_constraint_name", ConstraintSyntax, constraint_syntax, ConstraintSyntax, bare_constraint_name),
    (DECLARATIVE_PARTITIONING, "declarative_partitioning", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, declarative_partitioning),
    (TABLE_INHERITANCE, "table_inheritance", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, table_inheritance),
    (LIKE_SOURCE_TABLE, "like_source_table", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, like_source_table),
    (STATEMENT_LEVEL_TABLE_LIKE, "statement_level_table_like", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, statement_level_table_like),
    (COLUMN_COLLATION, "column_collation", ColumnDefinitionSyntax, column_definition_syntax, ColumnDefinitionSyntax, column_collation),
    (UNLOGGED_TABLES, "unlogged_tables", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, unlogged_tables),
    (COLUMN_STORAGE, "column_storage", ColumnDefinitionSyntax, column_definition_syntax, ColumnDefinitionSyntax, column_storage),
    (TABLE_ACCESS_METHOD, "table_access_method", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, table_access_method),
    (WITHOUT_OIDS, "without_oids", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, without_oids),
    (TYPED_TABLES, "typed_tables", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, typed_tables),
    (EXCLUSION_CONSTRAINTS, "exclusion_constraints", ConstraintSyntax, constraint_syntax, ConstraintSyntax, exclusion_constraints),
    (CREATE_TABLE_AS_EXECUTE, "create_table_as_execute", CreateTableClauseSyntax, create_table_clause_syntax, CreateTableClauseSyntax, create_table_as_execute),
    (CONSTRAINT_NO_INHERIT_NOT_VALID, "constraint_no_inherit_not_valid", ConstraintSyntax, constraint_syntax, ConstraintSyntax, constraint_no_inherit_not_valid),
    (INDEX_CONSTRAINT_PARAMETERS, "index_constraint_parameters", ConstraintSyntax, constraint_syntax, ConstraintSyntax, index_constraint_parameters),
    (CONSTRAINT_COLUMN_COLLATE_ORDER, "constraint_column_collate_order", ConstraintSyntax, constraint_syntax, ConstraintSyntax, constraint_column_collate_order),
    (TYPECAST_OPERATOR, "typecast_operator", ExpressionSyntax, expression_syntax, ExpressionSyntax, typecast_operator),
    (SUBSCRIPT, "subscript", ExpressionSyntax, expression_syntax, ExpressionSyntax, subscript),
    (SLICE_STEP, "slice_step", ExpressionSyntax, expression_syntax, ExpressionSyntax, slice_step),
    (COLLATE, "collate", ExpressionSyntax, expression_syntax, ExpressionSyntax, collate),
    (AT_TIME_ZONE, "at_time_zone", ExpressionSyntax, expression_syntax, ExpressionSyntax, at_time_zone),
    (ARRAY_CONSTRUCTOR, "array_constructor", ExpressionSyntax, expression_syntax, ExpressionSyntax, array_constructor),
    (COLLECTION_LITERALS, "collection_literals", ExpressionSyntax, expression_syntax, ExpressionSyntax, collection_literals),
    (ROW_CONSTRUCTOR, "row_constructor", ExpressionSyntax, expression_syntax, ExpressionSyntax, row_constructor),
    (STRUCT_CONSTRUCTOR, "struct_constructor", ExpressionSyntax, expression_syntax, ExpressionSyntax, struct_constructor),
    (FIELD_SELECTION, "field_selection", ExpressionSyntax, expression_syntax, ExpressionSyntax, field_selection),
    (FIELD_WILDCARD, "field_wildcard", ExpressionSyntax, expression_syntax, ExpressionSyntax, field_wildcard),
    (NAMED_ARGUMENT, "named_argument", CallSyntax, call_syntax, CallSyntax, named_argument),
    (VARIADIC_ARGUMENT, "variadic_argument", CallSyntax, call_syntax, CallSyntax, variadic_argument),
    (OPERATOR_CONSTRUCT, "operator_construct", OperatorSyntax, operator_syntax, OperatorSyntax, operator_construct),
    // `containment_operators` / `json_arrow_operators` / `jsonb_operators` are hand-written
    // below: each is a SUBSET of the general operator surface (`custom_operators`), so with
    // that superset on, `@>` / `->` / `#>` parse as a generic `Custom` operator even when the
    // specific flag is off (a node-shape difference, not accept/reject). To keep each an
    // accept/reject discriminator, the coupled `set_enabled` vacates `custom_operators` too.
    (GROUP_CONCAT_SEPARATOR, "group_concat_separator", AggregateCallSyntax, aggregate_call_syntax, AggregateCallSyntax, group_concat_separator),
    (TRUTH_VALUE_TESTS, "truth_value_tests", OperatorSyntax, operator_syntax, OperatorSyntax, truth_value_tests),
    // `null_safe_equals` (`<=>`) is hand-written below: like the containment/JSON operators,
    // its `<=>` symbol munches to a generic `Custom` operator under `custom_operators`, so the
    // coupled `set_enabled` vacates that superset to keep `<=>` an accept/reject discriminator.
    (UTC_SPECIAL_FUNCTIONS, "utc_special_functions", CallSyntax, call_syntax, CallSyntax, utc_special_functions),
    (LAMBDA_EXPRESSIONS, "lambda_expressions", OperatorSyntax, operator_syntax, OperatorSyntax, lambda_expressions),
    (BITWISE_OPERATORS, "bitwise_operators", OperatorSyntax, operator_syntax, OperatorSyntax, bitwise_operators),
    (COLUMNS_EXPRESSION, "columns_expression", CallSyntax, call_syntax, CallSyntax, columns_expression),
    (TYPED_STRING_LITERALS, "typed_string_literals", ExpressionSyntax, expression_syntax, ExpressionSyntax, typed_string_literals),
    (TYPED_INTERVAL_LITERAL, "typed_interval_literal", ExpressionSyntax, expression_syntax, ExpressionSyntax, typed_interval_literal),
    (RELAXED_INTERVAL_SYNTAX, "relaxed_interval_syntax", ExpressionSyntax, expression_syntax, ExpressionSyntax, relaxed_interval_syntax),
    (MYSQL_INTERVAL_OPERATOR, "mysql_interval_operator", ExpressionSyntax, expression_syntax, ExpressionSyntax, mysql_interval_operator),
    (LAMBDA_KEYWORD, "lambda_keyword", ExpressionSyntax, expression_syntax, ExpressionSyntax, lambda_keyword),
    (WITHIN_GROUP, "within_group", AggregateCallSyntax, aggregate_call_syntax, AggregateCallSyntax, within_group),
    (AGGREGATE_FILTER, "aggregate_filter", AggregateCallSyntax, aggregate_call_syntax, AggregateCallSyntax, aggregate_filter),
    (QUANTIFIED_COMPARISONS, "quantified_comparisons", OperatorSyntax, operator_syntax, OperatorSyntax, quantified_comparisons),
    (CUSTOM_OPERATORS, "custom_operators", OperatorSyntax, operator_syntax, OperatorSyntax, custom_operators),
    (POSTFIX_OPERATORS, "postfix_operators", OperatorSyntax, operator_syntax, OperatorSyntax, postfix_operators),
    (EXTRACT_FROM_SYNTAX, "extract_from_syntax", CallSyntax, call_syntax, CallSyntax, extract_from_syntax),
    (TRY_CAST, "try_cast", CallSyntax, call_syntax, CallSyntax, try_cast),
    (RESTRICTED_CAST_TARGETS, "restricted_cast_targets", CallSyntax, call_syntax, CallSyntax, restricted_cast_targets),
    (AGGREGATE_ARGS_REQUIRE_ADJACENT_PAREN, "aggregate_args_require_adjacent_paren", AggregateCallSyntax, aggregate_call_syntax, AggregateCallSyntax, aggregate_args_require_adjacent_paren),
    (EXTRACT_STRING_FIELD, "extract_string_field", CallSyntax, call_syntax, CallSyntax, extract_string_field),
    (METHOD_CHAINING, "method_chaining", CallSyntax, call_syntax, CallSyntax, method_chaining),
    (NULL_TREATMENT, "null_treatment", AggregateCallSyntax, aggregate_call_syntax, AggregateCallSyntax, null_treatment),
    (AGGREGATE_CALLS_REJECT_EMPTY_ARGUMENTS, "aggregate_calls_reject_empty_arguments", AggregateCallSyntax, aggregate_call_syntax, AggregateCallSyntax, aggregate_calls_reject_empty_arguments),
    (OVER_REQUIRES_WINDOWABLE_FUNCTION, "over_requires_windowable_function", AggregateCallSyntax, aggregate_call_syntax, AggregateCallSyntax, over_requires_windowable_function),
    (SQLJSON_EXPRESSION_FUNCTIONS, "sqljson_expression_functions", CallSyntax, call_syntax, CallSyntax, sqljson_expression_functions),
    (XML_EXPRESSION_FUNCTIONS, "xml_expression_functions", CallSyntax, call_syntax, CallSyntax, xml_expression_functions),
    // The standard string special forms anchor in the ISO catalogue as
    // `realized_by: None` rows (E021-06/-09/-11, T312 — `CallSyntax` is an
    // aggregate knob with no 1:1 anchor, the F031-03/GRANT precedent), so the
    // toggles carry no `catalog_id`; the dialect-variant flags (FOR-leading
    // orders, SIMILAR, arity floors, operand asymmetry, loose trim tails) are
    // engine-fidelity knobs with no ISO row at all.
    (SUBSTRING_FROM_FOR, "substring_from_for", StringFuncForms, string_func_forms, StringFuncForms, substring_from_for),
    (SUBSTRING_LEADING_FOR, "substring_leading_for", StringFuncForms, string_func_forms, StringFuncForms, substring_leading_for),
    (SUBSTRING_SIMILAR, "substring_similar", StringFuncForms, string_func_forms, StringFuncForms, substring_similar),
    (SUBSTRING_PLAIN_CALL_REQUIRES_2_OR_3_ARGS, "substring_plain_call_requires_2_or_3_args", StringFuncForms, string_func_forms, StringFuncForms, substring_plain_call_requires_2_or_3_args),
    (SUBSTR_FROM_FOR, "substr_from_for", StringFuncForms, string_func_forms, StringFuncForms, substr_from_for),
    (POSITION_IN, "position_in", StringFuncForms, string_func_forms, StringFuncForms, position_in),
    (POSITION_ASYMMETRIC_OPERANDS, "position_asymmetric_operands", StringFuncForms, string_func_forms, StringFuncForms, position_asymmetric_operands),
    (OVERLAY_PLACING, "overlay_placing", StringFuncForms, string_func_forms, StringFuncForms, overlay_placing),
    (OVERLAY_REQUIRES_PLACING, "overlay_requires_placing", StringFuncForms, string_func_forms, StringFuncForms, overlay_requires_placing),
    (TRIM_FROM, "trim_from", StringFuncForms, string_func_forms, StringFuncForms, trim_from),
    (TRIM_LIST_SYNTAX, "trim_list_syntax", StringFuncForms, string_func_forms, StringFuncForms, trim_list_syntax),
    // `CEIL(x TO field)` is sqlparser-rs-parity surface only — no probed oracle grammar
    // admits it (pg_query/DuckDB/mysql:8.4.10 all reject), so it carries no catalog id.
    (CEIL_TO_FIELD, "ceil_to_field", StringFuncForms, string_func_forms, StringFuncForms, ceil_to_field),
    // `FLOOR(x TO field)` is sqlparser-rs-parity surface only — no probed oracle grammar
    // admits it (pg_query/DuckDB/mysql:8.4.10 all reject), so it carries no catalog id.
    (FLOOR_TO_FIELD, "floor_to_field", StringFuncForms, string_func_forms, StringFuncForms, floor_to_field),
    // The standard pattern-match predicate is catalogued as SQL:2016 Core E021-08.
    (LIKE, "like", PredicateSyntax, predicate_syntax, PredicateSyntax, like, catalog_id: "E021-08"),
    (ILIKE, "ilike", PredicateSyntax, predicate_syntax, PredicateSyntax, ilike),
    (SIMILAR_TO, "similar_to", PredicateSyntax, predicate_syntax, PredicateSyntax, similar_to),
    (UNPARENTHESIZED_IN_LIST, "unparenthesized_in_list", PredicateSyntax, predicate_syntax, PredicateSyntax, unparenthesized_in_list),
    (DISTINCT_ON, "distinct_on", SelectSyntax, select_syntax, SelectSyntax, distinct_on),
    (FETCH_FIRST, "fetch_first", QueryTailSyntax, query_tail_syntax, QueryTailSyntax, fetch_first),
    (LIMIT_OFFSET_COMMA, "limit_offset_comma", QueryTailSyntax, query_tail_syntax, QueryTailSyntax, limit_offset_comma),
    (SELECT_INTO, "select_into", SelectSyntax, select_syntax, SelectSyntax, select_into),
    (GROUPING_SETS, "grouping_sets", GroupingSyntax, grouping_syntax, GroupingSyntax, grouping_sets),
    (WITH_ROLLUP, "with_rollup", GroupingSyntax, grouping_syntax, GroupingSyntax, with_rollup),
    (ORDER_BY_USING, "order_by_using", GroupingSyntax, grouping_syntax, GroupingSyntax, order_by_using),
    (EMPTY_TARGET_LIST, "empty_target_list", SelectSyntax, select_syntax, SelectSyntax, empty_target_list),
    (QUALIFY, "qualify", SelectSyntax, select_syntax, SelectSyntax, qualify),
    (ALIAS_STRING_LITERALS, "alias_string_literals", SelectSyntax, select_syntax, SelectSyntax, alias_string_literals),
    (BARE_ALIAS_STRING_LITERALS, "bare_alias_string_literals", SelectSyntax, select_syntax, SelectSyntax, bare_alias_string_literals),
    (GROUP_BY_ALL, "group_by_all", GroupingSyntax, grouping_syntax, GroupingSyntax, group_by_all),
    (ORDER_BY_ALL, "order_by_all", GroupingSyntax, grouping_syntax, GroupingSyntax, order_by_all),
    (UNION_BY_NAME, "union_by_name", SelectSyntax, select_syntax, SelectSyntax, union_by_name),
    (USING_SAMPLE, "using_sample", QueryTailSyntax, query_tail_syntax, QueryTailSyntax, using_sample),
    (LOCKING_CLAUSES, "locking_clauses", QueryTailSyntax, query_tail_syntax, QueryTailSyntax, locking_clauses),
    (KEY_LOCK_STRENGTHS, "key_lock_strengths", QueryTailSyntax, query_tail_syntax, QueryTailSyntax, key_lock_strengths),
    (STACKED_LOCKING_CLAUSES, "stacked_locking_clauses", QueryTailSyntax, query_tail_syntax, QueryTailSyntax, stacked_locking_clauses),
    (COPY, "copy", UtilitySyntax, utility_syntax, UtilitySyntax, copy),
    (COPY_INTO, "copy_into", UtilitySyntax, utility_syntax, UtilitySyntax, copy_into),
    (COMMENT_ON, "comment_on", UtilitySyntax, utility_syntax, UtilitySyntax, comment_on),
    (PRAGMA, "pragma", UtilitySyntax, utility_syntax, UtilitySyntax, pragma),
    (ATTACH, "attach", UtilitySyntax, utility_syntax, UtilitySyntax, attach),
    (VACUUM, "vacuum", MaintenanceSyntax, maintenance_syntax, MaintenanceSyntax, vacuum),
    (VACUUM_ANALYZE, "vacuum_analyze", MaintenanceSyntax, maintenance_syntax, MaintenanceSyntax, vacuum_analyze),
    (REINDEX, "reindex", MaintenanceSyntax, maintenance_syntax, MaintenanceSyntax, reindex),
    (ANALYZE, "analyze", MaintenanceSyntax, maintenance_syntax, MaintenanceSyntax, analyze),
    (ANALYZE_COLUMNS, "analyze_columns", MaintenanceSyntax, maintenance_syntax, MaintenanceSyntax, analyze_columns),
    (KILL, "kill", UtilitySyntax, utility_syntax, UtilitySyntax, kill),
    (HANDLER_STATEMENTS, "handler_statements", UtilitySyntax, utility_syntax, UtilitySyntax, handler_statements),
    (PLUGIN_COMPONENT_STATEMENTS, "plugin_component_statements", UtilitySyntax, utility_syntax, UtilitySyntax, plugin_component_statements),
    (SHUTDOWN, "shutdown", UtilitySyntax, utility_syntax, UtilitySyntax, shutdown),
    (RESTART, "restart", UtilitySyntax, utility_syntax, UtilitySyntax, restart),
    (CLONE, "clone", UtilitySyntax, utility_syntax, UtilitySyntax, clone),
    (IMPORT_TABLE, "import_table", UtilitySyntax, utility_syntax, UtilitySyntax, import_table),
    (HELP_STATEMENT, "help_statement", UtilitySyntax, utility_syntax, UtilitySyntax, help_statement),
    (BINLOG, "binlog", UtilitySyntax, utility_syntax, UtilitySyntax, binlog),
    (KEY_CACHE_STATEMENTS, "key_cache_statements", UtilitySyntax, utility_syntax, UtilitySyntax, key_cache_statements),
    (DESCRIBE, "describe", ShowSyntax, show_syntax, ShowSyntax, describe),
    (SESSION_STATEMENTS, "session_statements", ShowSyntax, show_syntax, ShowSyntax, session_statements),
    (ACCESS_CONTROL, "access_control", AccessControlSyntax, access_control_syntax, AccessControlSyntax, access_control),
    (ACCESS_CONTROL_EXTENDED_OBJECTS, "access_control_extended_objects", AccessControlSyntax, access_control_syntax, AccessControlSyntax, access_control_extended_objects),
    (USER_ROLE_MANAGEMENT, "user_role_management", AccessControlSyntax, access_control_syntax, AccessControlSyntax, user_role_management),
    (ACCESS_CONTROL_ACCOUNT_GRANTS, "access_control_account_grants", AccessControlSyntax, access_control_syntax, AccessControlSyntax, access_control_account_grants),
    (USE_STATEMENT, "use_statement", UtilitySyntax, utility_syntax, UtilitySyntax, use_statement),
    (PREPARED_STATEMENTS, "prepared_statements", UtilitySyntax, utility_syntax, UtilitySyntax, prepared_statements),
    (PREPARE_TYPED_PARAMETERS, "prepare_typed_parameters", UtilitySyntax, utility_syntax, UtilitySyntax, prepare_typed_parameters),
    (PREPARED_STATEMENTS_FROM, "prepared_statements_from", UtilitySyntax, utility_syntax, UtilitySyntax, prepared_statements_from),
    (CALL, "call", UtilitySyntax, utility_syntax, UtilitySyntax, call),
    (CALL_BARE_NAME, "call_bare_name", UtilitySyntax, utility_syntax, UtilitySyntax, call_bare_name),
    (CHECKPOINT, "checkpoint", MaintenanceSyntax, maintenance_syntax, MaintenanceSyntax, checkpoint),
    (CHECKPOINT_DATABASE, "checkpoint_database", MaintenanceSyntax, maintenance_syntax, MaintenanceSyntax, checkpoint_database),
    (LOAD_EXTENSION, "load_extension", UtilitySyntax, utility_syntax, UtilitySyntax, load_extension),
    (LOAD_BARE_NAME, "load_bare_name", UtilitySyntax, utility_syntax, UtilitySyntax, load_bare_name),
    (RESET_SCOPE, "reset_scope", UtilitySyntax, utility_syntax, UtilitySyntax, reset_scope),
    (DETACH_IF_EXISTS, "detach_if_exists", UtilitySyntax, utility_syntax, UtilitySyntax, detach_if_exists),
    (DO_STATEMENT, "do_statement", UtilitySyntax, utility_syntax, UtilitySyntax, do_statement),
    (SHOW_TABLES, "show_tables", ShowSyntax, show_syntax, ShowSyntax, show_tables),
    (SHOW_COLUMNS, "show_columns", ShowSyntax, show_syntax, ShowSyntax, show_columns),
    (SHOW_CREATE_TABLE, "show_create_table", ShowSyntax, show_syntax, ShowSyntax, show_create_table),
    (SHOW_FUNCTIONS, "show_functions", ShowSyntax, show_syntax, ShowSyntax, show_functions),
    (SHOW_ROUTINE_STATUS, "show_routine_status", ShowSyntax, show_syntax, ShowSyntax, show_routine_status),
    (SHOW_VERBOSE, "show_verbose", ShowSyntax, show_syntax, ShowSyntax, show_verbose),
    (SHOW_ADMIN, "show_admin", ShowSyntax, show_syntax, ShowSyntax, show_admin),
    (BEGIN_TRANSACTION_MODE, "begin_transaction_mode", UtilitySyntax, utility_syntax, UtilitySyntax, begin_transaction_mode),
    (XA_TRANSACTIONS, "xa_transactions", UtilitySyntax, utility_syntax, UtilitySyntax, xa_transactions),
    (TABLE_MAINTENANCE, "table_maintenance", MaintenanceSyntax, maintenance_syntax, MaintenanceSyntax, table_maintenance),
    (RENAME_STATEMENT, "rename_statement", UtilitySyntax, utility_syntax, UtilitySyntax, rename_statement),
    (FLUSH, "flush", UtilitySyntax, utility_syntax, UtilitySyntax, flush),
    (PURGE_BINARY_LOGS, "purge_binary_logs", UtilitySyntax, utility_syntax, UtilitySyntax, purge_binary_logs),
    (REPLICATION_STATEMENTS, "replication_statements", UtilitySyntax, utility_syntax, UtilitySyntax, replication_statements),
    // Statement-head / leading-keyword gates, each a flip-verified toggle exercised by an
    // accept/reject LabeledCase.
    (LOAD_DATA, "load_data", UtilitySyntax, utility_syntax, UtilitySyntax, load_data),
    (DO_EXPRESSION_LIST, "do_expression_list", UtilitySyntax, utility_syntax, UtilitySyntax, do_expression_list),
    (LOCK_TABLES, "lock_tables", UtilitySyntax, utility_syntax, UtilitySyntax, lock_tables),
    (LOCK_INSTANCE, "lock_instance", UtilitySyntax, utility_syntax, UtilitySyntax, lock_instance),
    (STAGE_REFERENCES, "stage_references", UtilitySyntax, utility_syntax, UtilitySyntax, stage_references),
    (SIGNAL_DIAGNOSTICS, "signal_diagnostics", UtilitySyntax, utility_syntax, UtilitySyntax, signal_diagnostics),
    (EXPORT_IMPORT_DATABASE, "export_import_database", UtilitySyntax, utility_syntax, UtilitySyntax, export_import_database),
    (UPDATE_EXTENSIONS, "update_extensions", UtilitySyntax, utility_syntax, UtilitySyntax, update_extensions),
    (USE_QUALIFIED_NAME, "use_qualified_name", UtilitySyntax, utility_syntax, UtilitySyntax, use_qualified_name),
    (EXTENSION_DDL, "extension_ddl", StatementDdlGates, statement_ddl_gates, StatementDdlGates, extension_ddl),
    (TRANSFORM_DDL, "transform_ddl", StatementDdlGates, statement_ddl_gates, StatementDdlGates, transform_ddl),
    (ALTER_SYSTEM, "alter_system", StatementDdlGates, statement_ddl_gates, StatementDdlGates, alter_system),
    (TABLESPACE_DDL, "tablespace_ddl", StatementDdlGates, statement_ddl_gates, StatementDdlGates, tablespace_ddl),
    (LOGFILE_GROUP_DDL, "logfile_group_ddl", StatementDdlGates, statement_ddl_gates, StatementDdlGates, logfile_group_ddl),
    (SCHEMA_ELEMENTS, "schema_elements", StatementDdlGates, statement_ddl_gates, StatementDdlGates, schema_elements),
    (DROP_DATABASE, "drop_database", StatementDdlGates, statement_ddl_gates, StatementDdlGates, drop_database),
    (RECURSIVE_VIEWS, "recursive_views", StatementDdlGates, statement_ddl_gates, StatementDdlGates, recursive_views),
    (ALTER_DATABASE, "alter_database", StatementDdlGates, statement_ddl_gates, StatementDdlGates, alter_database),
    (ALTER_DATABASE_OPTIONS, "alter_database_options", StatementDdlGates, statement_ddl_gates, StatementDdlGates, alter_database_options),
    (SERVER_DEFINITION, "server_definition", StatementDdlGates, statement_ddl_gates, StatementDdlGates, server_definition),
    (ALTER_INSTANCE, "alter_instance", StatementDdlGates, statement_ddl_gates, StatementDdlGates, alter_instance),
    (ALTER_SEQUENCE, "alter_sequence", StatementDdlGates, statement_ddl_gates, StatementDdlGates, alter_sequence),
    (ALTER_OBJECT_SET_SCHEMA, "alter_object_set_schema", StatementDdlGates, statement_ddl_gates, StatementDdlGates, alter_object_set_schema),
    (VIEW_DEFINITION_OPTIONS, "view_definition_options", StatementDdlGates, statement_ddl_gates, StatementDdlGates, view_definition_options),
    (DESCRIBE_SUMMARIZE, "describe_summarize", ShowSyntax, show_syntax, ShowSyntax, describe_summarize),
    (INDEX_DROP_ON_TABLE, "index_drop_on_table", IndexAlterSyntax, index_alter_syntax, IndexAlterSyntax, index_drop_on_table),
    (FROM_FIRST, "from_first", SelectSyntax, select_syntax, SelectSyntax, from_first),
    (WILDCARD_MODIFIERS, "wildcard_modifiers", SelectSyntax, select_syntax, SelectSyntax, wildcard_modifiers),
    (QUALIFIED_WILDCARD_ALIAS, "qualified_wildcard_alias", SelectSyntax, select_syntax, SelectSyntax, qualified_wildcard_alias),
    (LEADING_OFFSET, "leading_offset", QueryTailSyntax, query_tail_syntax, QueryTailSyntax, leading_offset),
    (PARENTHESIZED_QUERY_OPERANDS, "parenthesized_query_operands", SelectSyntax, select_syntax, SelectSyntax, parenthesized_query_operands),
    (VALUES_ROWS_REQUIRE_EQUAL_ARITY, "values_rows_require_equal_arity", SelectSyntax, select_syntax, SelectSyntax, values_rows_require_equal_arity),
    (LIMIT_EXPRESSIONS, "limit_expressions", QueryTailSyntax, query_tail_syntax, QueryTailSyntax, limit_expressions),
    (LIMIT_PERCENT, "limit_percent", QueryTailSyntax, query_tail_syntax, QueryTailSyntax, limit_percent),
    (VALUES_ROW_CONSTRUCTOR, "values_row_constructor", SelectSyntax, select_syntax, SelectSyntax, values_row_constructor),
    (AS_ALIAS_REJECTS_RESERVED, "as_alias_rejects_reserved", SelectSyntax, select_syntax, SelectSyntax, as_alias_rejects_reserved),
    (PIPE_SYNTAX, "pipe_syntax", QueryTailSyntax, query_tail_syntax, QueryTailSyntax, pipe_syntax),
    (EXTENDED_SCALAR_TYPE_NAMES, "extended_scalar_type_names", TypeNameSyntax, type_name_syntax, TypeNameSyntax, extended_scalar_type_names),
    (ENUM_TYPE, "enum_type", TypeNameSyntax, type_name_syntax, TypeNameSyntax, enum_type),
    (SET_TYPE, "set_type", TypeNameSyntax, type_name_syntax, TypeNameSyntax, set_type),
    (NUMERIC_MODIFIERS, "numeric_modifiers", TypeNameSyntax, type_name_syntax, TypeNameSyntax, numeric_modifiers),
    (INTEGER_DISPLAY_WIDTH, "integer_display_width", TypeNameSyntax, type_name_syntax, TypeNameSyntax, integer_display_width),
    (COMPOSITE_TYPES, "composite_types", TypeNameSyntax, type_name_syntax, TypeNameSyntax, composite_types),
    (VARCHAR_REQUIRES_LENGTH, "varchar_requires_length", TypeNameSyntax, type_name_syntax, TypeNameSyntax, varchar_requires_length),
    (ZONED_TEMPORAL_TYPES, "zoned_temporal_types", TypeNameSyntax, type_name_syntax, TypeNameSyntax, zoned_temporal_types),
    (EMPTY_TYPE_PARENS, "empty_type_parens", TypeNameSyntax, type_name_syntax, TypeNameSyntax, empty_type_parens),
    (CHARACTER_SET_ANNOTATION, "character_set_annotation", TypeNameSyntax, type_name_syntax, TypeNameSyntax, character_set_annotation),
    (SIGNED_TYPE_MODIFIER, "signed_type_modifier", TypeNameSyntax, type_name_syntax, TypeNameSyntax, signed_type_modifier),
    (LIBERAL_TYPE_NAMES, "liberal_type_names", TypeNameSyntax, type_name_syntax, TypeNameSyntax, liberal_type_names),
    (STRING_TYPE_MODIFIERS, "string_type_modifiers", TypeNameSyntax, type_name_syntax, TypeNameSyntax, string_type_modifiers),
}

// `versioned_comments` is an `Option<u32>` (the modelled `MYSQL_VERSION_ID`
// bound), not a boolean, so it cannot be a `toggleable_features!` arm: the
// toggle realizes "on" as the MySQL preset's bound.
const VERSIONED_COMMENTS: ToggleableFeature = ToggleableFeature {
    sub_flag: "versioned_comments",
    feature: Feature::CommentSyntax,
    catalog_id: None,
    is_enabled: |features| features.comment_syntax.versioned_comments.is_some(),
    set_enabled: |features, on| {
        features.with(FeatureDelta::EMPTY.comment_syntax(CommentSyntax {
            versioned_comments: on.then_some(CommentSyntax::MYSQL_8_VERSION_BOUND),
            ..features.comment_syntax
        }))
    },
};

// `double_quoted_strings` is *not* an independent additive flag: it and a `"`
// identifier quote both claim `"` (`LexicalConflict::DoubleQuoteStringVersusIdentifier`),
// so it is hand-written here rather than a `toggleable_features!` arm. Enabling it must
// also vacate `"` from `identifier_quotes`, or the toggled set is lexically inconsistent
// and the parser's construction-time `debug_assert!` (ADR-0011) rejects it. The toggle
// therefore models the real dialect coupling: on, `"` opens a string and no longer
// quotes identifiers (MySQL under `ANSI_QUOTES` off); off, `"` quotes identifiers and is
// not a string (PostgreSQL/ANSI). Every base here is PostgreSQL-derived (`"`-only
// quoting), so restoring the standard `"` quote on the off side is exact.
const DOUBLE_QUOTED_STRINGS: ToggleableFeature = ToggleableFeature {
    sub_flag: "double_quoted_strings",
    feature: Feature::StringLiterals,
    catalog_id: None,
    is_enabled: |features| features.string_literals.double_quoted_strings,
    set_enabled: |features, on| {
        let identifier_quotes: &'static [IdentifierQuote] = if on {
            &[]
        } else {
            &[IdentifierQuote::Symmetric('"')]
        };
        features.with(
            FeatureDelta::EMPTY
                .string_literals(StringLiteralSyntax {
                    double_quoted_strings: on,
                    ..features.string_literals
                })
                .identifier_quotes(identifier_quotes),
        )
    },
};

// The remaining coupled toggles: each pairs a feature with the PostgreSQL feature it
// shares a context-free tokenizer trigger with (a `LexicalConflict`), so enabling one
// must vacate the other or the toggled set is lexically inconsistent and the parser's
// construction-time `debug_assert!` (ADR-0011) rejects it. Each models the real dialect:
// a `money`/`:name`/`@name` dialect has none of PostgreSQL's rival `$n`/subscript/`<@`.
// Bases here are PostgreSQL-derived (rival on), so `!on` restores the rival exactly on
// the off side. Hand-written for the same reason as `double_quoted_strings`: they are not
// independent additive flags.
const MONEY_LITERALS: ToggleableFeature = ToggleableFeature {
    sub_flag: "money_literals",
    feature: Feature::NumericLiterals,
    catalog_id: None,
    is_enabled: |features| features.numeric_literals.money_literals,
    set_enabled: |features, on| {
        // money vs positional `$n`: `LexicalConflict::MoneyVersusPositionalDollar`.
        features.with(
            FeatureDelta::EMPTY
                .numeric_literals(NumericLiteralSyntax {
                    money_literals: on,
                    ..features.numeric_literals
                })
                .parameters(ParameterSyntax {
                    positional_dollar: !on,
                    ..features.parameters
                }),
        )
    },
};

const NAMED_COLON: ToggleableFeature = ToggleableFeature {
    sub_flag: "named_colon",
    feature: Feature::Parameters,
    catalog_id: None,
    is_enabled: |features| features.parameters.named_colon,
    set_enabled: |features, on| {
        // `:name` vs the `a[x:y]` slice bound and the `{a: b}` collection separator:
        // `LexicalConflict::ColonParameterVersusSliceBound` names all claimants of the
        // `:`+identifier trigger.
        features.with(
            FeatureDelta::EMPTY
                .parameters(ParameterSyntax {
                    named_colon: on,
                    ..features.parameters
                })
                .expression_syntax(ExpressionSyntax {
                    subscript: !on,
                    collection_literals: !on && features.expression_syntax.collection_literals,
                    semi_structured_access: !on
                        && features.expression_syntax.semi_structured_access,
                    ..features.expression_syntax
                }),
        )
    },
};

const SEMI_STRUCTURED_ACCESS: ToggleableFeature = ToggleableFeature {
    sub_flag: "semi_structured_access",
    feature: Feature::ExpressionSyntax,
    catalog_id: None,
    is_enabled: |features| features.expression_syntax.semi_structured_access,
    set_enabled: |features, on| {
        features.with(
            FeatureDelta::EMPTY
                .expression_syntax(ExpressionSyntax {
                    semi_structured_access: on,
                    ..features.expression_syntax
                })
                .parameters(ParameterSyntax {
                    named_colon: !on && features.parameters.named_colon,
                    ..features.parameters
                }),
        )
    },
};

// Anonymous `?` placeholder vs the PostgreSQL `jsonb` `?` key-existence operator (and the
// `?|`/`?&` it leads): both claim the `?` trigger
// (`LexicalConflict::JsonbKeyExistsVersusAnonymousParameter`), so enabling the placeholder
// must vacate the `jsonb` operators or the toggled set is lexically inconsistent. The base is
// PostgreSQL (`jsonb_operators = true`), so `!on` restores it exactly on the off side —
// matching the coupled `named_at`/`user_variables` pattern.
const ANONYMOUS_QUESTION: ToggleableFeature = ToggleableFeature {
    sub_flag: "anonymous_question",
    feature: Feature::Parameters,
    catalog_id: None,
    is_enabled: |features| features.parameters.anonymous_question,
    set_enabled: |features, on| {
        features.with(
            FeatureDelta::EMPTY
                .parameters(ParameterSyntax {
                    anonymous_question: on,
                    ..features.parameters
                })
                .operator_syntax(OperatorSyntax {
                    jsonb_operators: !on,
                    ..features.operator_syntax
                }),
        )
    },
};

// MySQL `@@name` system variable vs the PostgreSQL `jsonb`/text-search `@@` match operator:
// both claim the `@@` trigger (`LexicalConflict::JsonbSearchOperatorVersusSystemVariable`),
// so enabling the system variable must vacate the `jsonb` operators or the toggled set is
// lexically inconsistent. The base is PostgreSQL (`jsonb_operators = true`), so `!on` restores
// it exactly on the off side. (`@?` and the single-`@` sigils stay disjoint by their second
// byte, so only `@@` needs this coupling.) The general `@@` operator (`custom_operators`) also
// claims `@@` once `jsonb` is off (`LexicalConflict::CustomOperatorVersusSystemVariable`), so it
// is held off unconditionally (see the inline note).
const SYSTEM_VARIABLES: ToggleableFeature = ToggleableFeature {
    sub_flag: "system_variables",
    feature: Feature::SessionVariables,
    catalog_id: None,
    is_enabled: |features| features.session_variables.system_variables,
    set_enabled: |features, on| {
        features.with(
            FeatureDelta::EMPTY
                .session_variables(SessionVariableSyntax {
                    system_variables: on,
                    ..features.session_variables
                })
                .operator_syntax(OperatorSyntax {
                    jsonb_operators: !on,
                    // Held OFF on BOTH sides (not `!on`): the general operator surface gives
                    // bare `@`/`@@` a prefix-operator meaning, so restoring it on the sigil-off
                    // side would make `@name`/`@@name` parse as that operator (accept) instead
                    // of the clean reject the sigil discriminator needs. Off is lexically
                    // consistent with the sigil on OR off, so this stays off throughout.
                    custom_operators: false,
                    ..features.operator_syntax
                }),
        )
    },
};

// `@>`/`<@` containment, `->`/`->>` JSON-arrow, and the `jsonb` operators are each a SUBSET
// of the general operator surface (`custom_operators`): with that superset on, the operator
// still parses when the specific flag is off, only as a generic `Custom` operator instead of
// its dedicated key (a node-shape difference, not accept/reject). Each hand-written feature
// therefore vacates `custom_operators` (holds it off on BOTH sides) so its own flag remains a
// clean accept/reject discriminator — the operator rejects when the specific flag is off.
const CONTAINMENT_OPERATORS: ToggleableFeature = ToggleableFeature {
    sub_flag: "containment_operators",
    feature: Feature::OperatorSyntax,
    catalog_id: None,
    is_enabled: |features| features.operator_syntax.containment_operators,
    set_enabled: |features, on| {
        features.with(FeatureDelta::EMPTY.operator_syntax(OperatorSyntax {
            containment_operators: on,
            custom_operators: false,
            ..features.operator_syntax
        }))
    },
};

const JSON_ARROW_OPERATORS: ToggleableFeature = ToggleableFeature {
    sub_flag: "json_arrow_operators",
    feature: Feature::OperatorSyntax,
    catalog_id: None,
    is_enabled: |features| features.operator_syntax.json_arrow_operators,
    set_enabled: |features, on| {
        features.with(FeatureDelta::EMPTY.operator_syntax(OperatorSyntax {
            json_arrow_operators: on,
            custom_operators: false,
            ..features.operator_syntax
        }))
    },
};

const JSONB_OPERATORS: ToggleableFeature = ToggleableFeature {
    sub_flag: "jsonb_operators",
    feature: Feature::OperatorSyntax,
    catalog_id: None,
    is_enabled: |features| features.operator_syntax.jsonb_operators,
    set_enabled: |features, on| {
        features.with(FeatureDelta::EMPTY.operator_syntax(OperatorSyntax {
            jsonb_operators: on,
            custom_operators: false,
            ..features.operator_syntax
        }))
    },
};

// MySQL's `<=>` null-safe equality: its `<=>` symbol is a subset of the general operator
// surface (`custom_operators` munches it to a generic `Custom` operator when this flag is
// off), so — like the containment/JSON operators above — the coupled `set_enabled` vacates
// `custom_operators` to keep `<=>` a clean accept/reject discriminator.
const NULL_SAFE_EQUALS: ToggleableFeature = ToggleableFeature {
    sub_flag: "null_safe_equals",
    feature: Feature::OperatorSyntax,
    catalog_id: None,
    is_enabled: |features| features.operator_syntax.null_safe_equals,
    set_enabled: |features, on| {
        features.with(FeatureDelta::EMPTY.operator_syntax(OperatorSyntax {
            null_safe_equals: on,
            custom_operators: false,
            ..features.operator_syntax
        }))
    },
};

const NAMED_AT: ToggleableFeature = ToggleableFeature {
    sub_flag: "named_at",
    feature: Feature::Parameters,
    catalog_id: None,
    is_enabled: |features| features.parameters.named_at,
    set_enabled: |features, on| {
        // `@name` vs the `<@` containment operator (`ContainmentOperatorVersusAtName`, vacated
        // `!on`) and vs the bare `@` general operator (`CustomOperatorVersusAtName`, held off
        // unconditionally — see the inline note): both are `@`-triggered.
        features.with(
            FeatureDelta::EMPTY
                .parameters(ParameterSyntax {
                    named_at: on,
                    ..features.parameters
                })
                .operator_syntax(OperatorSyntax {
                    containment_operators: !on,
                    // Held OFF on BOTH sides (not `!on`): the general operator surface gives
                    // bare `@`/`@@` a prefix-operator meaning, so restoring it on the sigil-off
                    // side would make `@name`/`@@name` parse as that operator (accept) instead
                    // of the clean reject the sigil discriminator needs. Off is lexically
                    // consistent with the sigil on OR off, so this stays off throughout.
                    custom_operators: false,
                    ..features.operator_syntax
                }),
        )
    },
};

const USER_VARIABLES: ToggleableFeature = ToggleableFeature {
    sub_flag: "user_variables",
    feature: Feature::SessionVariables,
    catalog_id: None,
    is_enabled: |features| features.session_variables.user_variables,
    set_enabled: |features, on| {
        // `@name` user variable vs the `<@` containment operator
        // (`ContainmentOperatorVersusAtName`, vacated `!on`) and vs the bare `@` general
        // operator (`CustomOperatorVersusAtName`, held off unconditionally — inline note):
        // both `@`-triggered.
        features.with(
            FeatureDelta::EMPTY
                .session_variables(SessionVariableSyntax {
                    user_variables: on,
                    ..features.session_variables
                })
                .operator_syntax(OperatorSyntax {
                    containment_operators: !on,
                    // Held OFF on BOTH sides (not `!on`): the general operator surface gives
                    // bare `@`/`@@` a prefix-operator meaning, so restoring it on the sigil-off
                    // side would make `@name`/`@@name` parse as that operator (accept) instead
                    // of the clean reject the sigil discriminator needs. Off is lexically
                    // consistent with the sigil on OR off, so this stays off throughout.
                    custom_operators: false,
                    ..features.operator_syntax
                }),
        )
    },
};

// The MySQL variable-assignment `SET` grammar (and its `:=` operator). A parser-level
// behaviour, not a lexical trigger: with it off the base's generic `SET <name> {= | TO}
// <value>` grammar rejects a scope-keyword-prefixed assignment (`SET GLOBAL x = 1`), so the
// flag alone drives an objective accept/reject flip. The `:=` lexing it also enables shares
// [`Operator::ColonEquals`] with the named-argument separator, so no lexical vacating is
// needed (the same token is produced whichever behaviour owns it).
const VARIABLE_ASSIGNMENT: ToggleableFeature = ToggleableFeature {
    sub_flag: "variable_assignment",
    feature: Feature::SessionVariables,
    catalog_id: None,
    is_enabled: |features| features.session_variables.variable_assignment,
    set_enabled: |features, on| {
        features.with(
            FeatureDelta::EMPTY.session_variables(SessionVariableSyntax {
                variable_assignment: on,
                ..features.session_variables
            }),
        )
    },
};

// `#` line comment vs the PostgreSQL `#` bitwise-XOR operator: both claim the `#` trigger
// (`LexicalConflict::HashXorOperatorVersusHashComment`), so enabling the comment must vacate
// the XOR spelling or the toggled set is lexically inconsistent. The base is PostgreSQL
// (`hash_bitwise_xor = true`), so `!on` restores it exactly on the off side — where `#` is
// then the XOR operator, matching the coupled `money`/`named_colon` pattern.
const LINE_COMMENT_HASH: ToggleableFeature = ToggleableFeature {
    sub_flag: "line_comment_hash",
    feature: Feature::CommentSyntax,
    catalog_id: None,
    is_enabled: |features| features.comment_syntax.line_comment_hash,
    set_enabled: |features, on| {
        features.with(
            FeatureDelta::EMPTY
                .comment_syntax(CommentSyntax {
                    line_comment_hash: on,
                    ..features.comment_syntax
                })
                .hash_bitwise_xor(!on),
        )
    },
};

// `#n` positional column vs the PostgreSQL `#` bitwise-XOR operator: both claim the `#`
// trigger (`LexicalConflict::HashXorOperatorVersusPositionalColumn`), so enabling the
// positional form must vacate the XOR spelling or the toggled set is lexically
// inconsistent. The base is PostgreSQL (`hash_bitwise_xor = true`), so `!on` restores it
// exactly on the off side — where `#` is then the XOR operator — matching the coupled
// `line_comment_hash` pattern above. (DuckDB, the only shipped preset with the positional
// form, has `hash_bitwise_xor: false`, so this models the real dialect.)
//
// `custom_operators` is held OFF on BOTH sides (not `!on`), mirroring `SYSTEM_VARIABLES`
// above: the general operator surface gives a bare `#` a *prefix*-operator meaning
// (`SELECT #1` reads as `# 1`, pg-bare-prefix-operator-glyphs), so restoring it on the
// positional-off side would make `#n` parse as that prefix operator (accept) instead of the
// clean reject the positional discriminator needs. Off is lexically consistent with the
// positional form on OR off, so it stays off throughout, keeping `positional_column` a clean
// accept/reject discriminator.
const POSITIONAL_COLUMN: ToggleableFeature = ToggleableFeature {
    sub_flag: "positional_column",
    feature: Feature::ExpressionSyntax,
    catalog_id: None,
    is_enabled: |features| features.expression_syntax.positional_column,
    set_enabled: |features, on| {
        features.with(
            FeatureDelta::EMPTY
                .expression_syntax(ExpressionSyntax {
                    positional_column: on,
                    ..features.expression_syntax
                })
                .hash_bitwise_xor(!on)
                .operator_syntax(OperatorSyntax {
                    custom_operators: false,
                    ..features.operator_syntax
                }),
        )
    },
};

/// `pipe_operator = LogicalOr` as a toggle: enabled means a dialect reads `||` as
/// logical OR (MySQL-like), off means string concatenation (ANSI/PostgreSQL). It is
/// deliberately *not* in `TOGGLEABLE_FEATURES`: that array enumerates the additive
/// boolean sub-flags the granularity gates iterate, whereas this is an enum knob with
/// no accept/reject effect (both meanings parse). It exists to give the structural
/// `forbidden_features` case an executable, flippable label — the canonical first use
/// of `forbidden` (prod-coverage-labels-differential-corpus).
const LOGICAL_OR_PIPE: ToggleableFeature = ToggleableFeature {
    sub_flag: "logical_or_pipe",
    feature: Feature::PipeOperator,
    catalog_id: None,
    is_enabled: |features| matches!(features.pipe_operator, PipeOperator::LogicalOr),
    set_enabled: |features, on| {
        features.with(FeatureDelta::EMPTY.pipe_operator(if on {
            PipeOperator::LogicalOr
        } else {
            PipeOperator::StringConcat
        }))
    },
};

// `caret_operator = Exponent` as a toggle: on means a dialect reads `^` as arithmetic power
// (PostgreSQL/DuckDB), off means `^` has no infix meaning. It IS an accept/reject
// discriminator (`2 ^ 3` parses only when on) and the `SELECT 2 ^ 3` labelled case requires
// it. Like `LOGICAL_OR_PIPE` it is deliberately *not* in `TOGGLEABLE_FEATURES`: that array
// enumerates the additive boolean sub-flags owned by a composite knob, whereas the `^`
// meaning is a top-level `CaretOperator` enum — so it is hand-written and referenced only by
// its labelled case. The off side sets `Unsupported` rather than the MySQL `BitwiseXor`
// reading, so flipping the exponent label off never silently turns `^` into XOR.
const EXPONENT_OPERATOR: ToggleableFeature = ToggleableFeature {
    sub_flag: "exponent_operator",
    feature: Feature::CaretOperator,
    catalog_id: None,
    is_enabled: |features| matches!(features.caret_operator, CaretOperator::Exponent),
    set_enabled: |features, on| {
        features.with(FeatureDelta::EMPTY.caret_operator(if on {
            CaretOperator::Exponent
        } else {
            CaretOperator::Unsupported
        }))
    },
};

/// Every gated sub-flag as a toggleable feature. The enumeration guard below
/// destructures the sub-flag structs, so a new flag must be added here too.
const TOGGLEABLE_FEATURES: &[&ToggleableFeature] = &[
    &ESCAPE_STRINGS,
    &DOLLAR_QUOTED,
    &NATIONAL_STRINGS,
    &DOUBLE_QUOTED_STRINGS,
    &BACKSLASH_ESCAPES,
    &UNICODE_STRINGS,
    &BIT_STRING_LITERALS,
    &BLOB_LITERALS,
    &CHARSET_INTRODUCERS,
    &SAME_LINE_ADJACENT_CONCAT,
    &HEX_INTEGERS,
    &OCTAL_INTEGERS,
    &BINARY_INTEGERS,
    &UNDERSCORE_SEPARATORS,
    &RADIX_LEADING_UNDERSCORE,
    &REJECT_TRAILING_JUNK,
    &MONEY_LITERALS,
    &LINE_COMMENT_HASH,
    &LINE_COMMENT_ENDS_AT_CARRIAGE_RETURN,
    &NESTED_BLOCK_COMMENTS,
    &UNTERMINATED_BLOCK_COMMENT_AT_EOF,
    &VERSIONED_COMMENTS,
    &POSITIONAL_DOLLAR,
    &ANONYMOUS_QUESTION,
    &NUMBERED_QUESTION,
    &NAMED_COLON,
    &NAMED_AT,
    &USER_VARIABLES,
    &SYSTEM_VARIABLES,
    &VARIABLE_ASSIGNMENT,
    &DOLLAR_IN_IDENTIFIERS,
    &STRING_LITERAL_IDENTIFIERS,
    &EMPTY_QUOTED_IDENTIFIERS,
    &LATERAL,
    &TABLE_FUNCTIONS,
    &ROWS_FROM,
    &UNNEST,
    &UNNEST_WITH_OFFSET,
    &TABLE_FUNCTION_ORDINALITY,
    &ONLY,
    &TABLE_SAMPLE,
    &PARENTHESIZED_JOINS,
    &TABLE_ALIAS_COLUMN_LISTS,
    &JOIN_USING_ALIAS,
    &STACKED_JOIN_QUALIFIERS,
    &FULL_OUTER_JOIN,
    &NATURAL_CROSS_JOIN,
    &STRAIGHT_JOIN,
    &ASOF_JOIN,
    &POSITIONAL_JOIN,
    &SEMI_ANTI_JOIN,
    &SIDED_SEMI_ANTI_JOIN,
    &INDEX_HINTS,
    &TABLE_HINTS,
    &PARTITION_SELECTION,
    &PIVOT,
    &UNPIVOT,
    &SHOW_REF,
    &FROM_VALUES,
    &BASE_TABLE_ALIAS_COLUMN_LISTS,
    &STRING_LITERAL_ALIASES,
    &SPECIAL_FUNCTION_TABLE_SOURCE,
    &ALIASED_PARENTHESIZED_JOIN,
    &RECURSIVE_SEARCH_CYCLE,
    &RECURSIVE_UNION_REJECTS_ORDER_LIMIT,
    &JSON_TABLE,
    &XML_TABLE,
    &TABLE_EXPR_FACTOR,
    &PIVOT_VALUE_SOURCES,
    &MATCH_RECOGNIZE,
    &OPEN_JSON,
    &RETURNING,
    &ON_CONFLICT,
    &ON_DUPLICATE_KEY_UPDATE,
    &IF_EXISTS,
    &DROP_BEHAVIOR,
    &MULTI_COLUMN_ASSIGNMENT,
    &WHERE_CURRENT_OF,
    &MERGE,
    &REPLACE_INTO,
    &INSERT_SET,
    &UPDATE_DELETE_TAILS,
    &OR_CONFLICT_ACTION,
    &DELETE_USING,
    &UPDATE_FROM,
    &DELETE_USING_TARGET_ALIAS,
    &CTE_BEFORE_INSERT,
    &CTE_BEFORE_MERGE,
    &DATA_MODIFYING_CTES,
    &MERGE_WHEN_NOT_MATCHED_BY,
    &MERGE_INSERT_DEFAULT_VALUES,
    &MERGE_INSERT_OVERRIDING,
    &INDEX_CONCURRENTLY,
    &INDEX_USING_METHOD,
    &PARTIAL_INDEX,
    &TABLE_OPTIONS,
    &CREATE_DATABASE_IF_NOT_EXISTS,
    &GENERATED_COLUMN_SHORTHAND,
    &WITHOUT_ROWID_TABLE_OPTION,
    &STRICT_TABLE_OPTION,
    &COLUMN_CONFLICT_RESOLUTION_CLAUSE,
    &TYPELESS_COLUMN_DEFINITIONS,
    &JOINED_AUTOINCREMENT_ATTRIBUTE,
    &INLINE_PRIMARY_KEY_ORDERING,
    &NAMED_COLUMN_COLLATE_CONSTRAINT,
    &CREATE_TRIGGER,
    &CREATE_MACRO,
    &CREATE_OR_REPLACE_TABLE,
    &CREATE_SECRET,
    &CREATE_TYPE,
    &CREATE_VIRTUAL_TABLE,
    &CREATE_SEQUENCE,
    &SCHEMAS,
    &DATABASES,
    &MATERIALIZED_VIEWS,
    &TEMPORARY_VIEWS,
    &ROUTINES,
    &COMPOUND_STATEMENTS,
    &SPATIAL_REFERENCE_SYSTEM,
    &RESOURCE_GROUP,
    &OR_REPLACE,
    &IDENTITY_COLUMNS,
    &STORAGE_PARAMETERS,
    &ON_COMMIT,
    &ALTER_TABLE_EXTENDED,
    &INDEX_IF_NOT_EXISTS,
    &INDEX_NULLS_ORDER,
    &ROUTINE_ARG_TYPES,
    &ROUTINE_ARG_DEFAULTS,
    &ROUTINE_ARG_MODES,
    &ALTER_EXISTENCE_GUARDS,
    &ALTER_COLUMN_SET_DATA_TYPE,
    &DEFERRABLE_CONSTRAINTS,
    &CREATE_TABLE_AS_WITH_DATA,
    &DEFAULT_EXPRESSION_REQUIRES_PARENS,
    &NAMED_INLINE_NON_CHECK_CONSTRAINTS,
    &BARE_CONSTRAINT_NAME,
    &DECLARATIVE_PARTITIONING,
    &TABLE_INHERITANCE,
    &LIKE_SOURCE_TABLE,
    &STATEMENT_LEVEL_TABLE_LIKE,
    &COLUMN_COLLATION,
    &UNLOGGED_TABLES,
    &COLUMN_STORAGE,
    &TABLE_ACCESS_METHOD,
    &WITHOUT_OIDS,
    &TYPED_TABLES,
    &EXCLUSION_CONSTRAINTS,
    &CREATE_TABLE_AS_EXECUTE,
    &CONSTRAINT_NO_INHERIT_NOT_VALID,
    &INDEX_CONSTRAINT_PARAMETERS,
    &CONSTRAINT_COLUMN_COLLATE_ORDER,
    &TYPECAST_OPERATOR,
    &SUBSCRIPT,
    &SLICE_STEP,
    &COLLATE,
    &AT_TIME_ZONE,
    &SEMI_STRUCTURED_ACCESS,
    &ARRAY_CONSTRUCTOR,
    &COLLECTION_LITERALS,
    &ROW_CONSTRUCTOR,
    &STRUCT_CONSTRUCTOR,
    &FIELD_SELECTION,
    &FIELD_WILDCARD,
    &POSITIONAL_COLUMN,
    &NAMED_ARGUMENT,
    &VARIADIC_ARGUMENT,
    &OPERATOR_CONSTRUCT,
    &CONTAINMENT_OPERATORS,
    &JSON_ARROW_OPERATORS,
    &JSONB_OPERATORS,
    &CUSTOM_OPERATORS,
    &POSTFIX_OPERATORS,
    &GROUP_CONCAT_SEPARATOR,
    &TRUTH_VALUE_TESTS,
    &NULL_SAFE_EQUALS,
    &UTC_SPECIAL_FUNCTIONS,
    &LAMBDA_EXPRESSIONS,
    &BITWISE_OPERATORS,
    &COLUMNS_EXPRESSION,
    &TYPED_STRING_LITERALS,
    &TYPED_INTERVAL_LITERAL,
    &RELAXED_INTERVAL_SYNTAX,
    &MYSQL_INTERVAL_OPERATOR,
    &LAMBDA_KEYWORD,
    &WITHIN_GROUP,
    &AGGREGATE_FILTER,
    &QUANTIFIED_COMPARISONS,
    &EXTRACT_FROM_SYNTAX,
    &TRY_CAST,
    &RESTRICTED_CAST_TARGETS,
    &AGGREGATE_ARGS_REQUIRE_ADJACENT_PAREN,
    &EXTRACT_STRING_FIELD,
    &METHOD_CHAINING,
    &NULL_TREATMENT,
    &AGGREGATE_CALLS_REJECT_EMPTY_ARGUMENTS,
    &OVER_REQUIRES_WINDOWABLE_FUNCTION,
    &SQLJSON_EXPRESSION_FUNCTIONS,
    &XML_EXPRESSION_FUNCTIONS,
    &SUBSTRING_FROM_FOR,
    &SUBSTRING_LEADING_FOR,
    &SUBSTRING_SIMILAR,
    &SUBSTRING_PLAIN_CALL_REQUIRES_2_OR_3_ARGS,
    &SUBSTR_FROM_FOR,
    &POSITION_IN,
    &POSITION_ASYMMETRIC_OPERANDS,
    &OVERLAY_PLACING,
    &OVERLAY_REQUIRES_PLACING,
    &TRIM_FROM,
    &TRIM_LIST_SYNTAX,
    &CEIL_TO_FIELD,
    &FLOOR_TO_FIELD,
    &LIKE,
    &ILIKE,
    &SIMILAR_TO,
    &UNPARENTHESIZED_IN_LIST,
    &DISTINCT_ON,
    &FETCH_FIRST,
    &LIMIT_OFFSET_COMMA,
    &SELECT_INTO,
    &GROUPING_SETS,
    &WITH_ROLLUP,
    &ORDER_BY_USING,
    &EMPTY_TARGET_LIST,
    &QUALIFY,
    &ALIAS_STRING_LITERALS,
    &BARE_ALIAS_STRING_LITERALS,
    &GROUP_BY_ALL,
    &ORDER_BY_ALL,
    &UNION_BY_NAME,
    &USING_SAMPLE,
    &LOCKING_CLAUSES,
    &KEY_LOCK_STRENGTHS,
    &STACKED_LOCKING_CLAUSES,
    &FROM_FIRST,
    &WILDCARD_MODIFIERS,
    &QUALIFIED_WILDCARD_ALIAS,
    &LEADING_OFFSET,
    &PARENTHESIZED_QUERY_OPERANDS,
    &VALUES_ROWS_REQUIRE_EQUAL_ARITY,
    &LIMIT_EXPRESSIONS,
    &LIMIT_PERCENT,
    &VALUES_ROW_CONSTRUCTOR,
    &AS_ALIAS_REJECTS_RESERVED,
    &PIPE_SYNTAX,
    &COPY,
    &COPY_INTO,
    &COMMENT_ON,
    &PRAGMA,
    &ATTACH,
    &VACUUM,
    &VACUUM_ANALYZE,
    &REINDEX,
    &ANALYZE,
    &ANALYZE_COLUMNS,
    &KILL,
    &HANDLER_STATEMENTS,
    &PLUGIN_COMPONENT_STATEMENTS,
    &SHUTDOWN,
    &RESTART,
    &CLONE,
    &IMPORT_TABLE,
    &HELP_STATEMENT,
    &BINLOG,
    &KEY_CACHE_STATEMENTS,
    &DESCRIBE,
    &SESSION_STATEMENTS,
    &ACCESS_CONTROL,
    &ACCESS_CONTROL_EXTENDED_OBJECTS,
    &USER_ROLE_MANAGEMENT,
    &ACCESS_CONTROL_ACCOUNT_GRANTS,
    &USE_STATEMENT,
    &PREPARED_STATEMENTS,
    &PREPARE_TYPED_PARAMETERS,
    &PREPARED_STATEMENTS_FROM,
    &CALL,
    &CALL_BARE_NAME,
    &CHECKPOINT,
    &CHECKPOINT_DATABASE,
    &LOAD_EXTENSION,
    &LOAD_BARE_NAME,
    &RESET_SCOPE,
    &DETACH_IF_EXISTS,
    &DO_STATEMENT,
    &SHOW_TABLES,
    &SHOW_COLUMNS,
    &SHOW_CREATE_TABLE,
    &SHOW_FUNCTIONS,
    &SHOW_ROUTINE_STATUS,
    &SHOW_VERBOSE,
    &SHOW_ADMIN,
    &BEGIN_TRANSACTION_MODE,
    &XA_TRANSACTIONS,
    &TABLE_MAINTENANCE,
    &RENAME_STATEMENT,
    &FLUSH,
    &PURGE_BINARY_LOGS,
    &REPLICATION_STATEMENTS,
    &LOAD_DATA,
    &DO_EXPRESSION_LIST,
    &LOCK_TABLES,
    &LOCK_INSTANCE,
    &STAGE_REFERENCES,
    &SIGNAL_DIAGNOSTICS,
    &EXPORT_IMPORT_DATABASE,
    &UPDATE_EXTENSIONS,
    &USE_QUALIFIED_NAME,
    &EXTENSION_DDL,
    &TRANSFORM_DDL,
    &ALTER_SYSTEM,
    &TABLESPACE_DDL,
    &LOGFILE_GROUP_DDL,
    &SCHEMA_ELEMENTS,
    &DROP_DATABASE,
    &RECURSIVE_VIEWS,
    &ALTER_DATABASE,
    &ALTER_DATABASE_OPTIONS,
    &SERVER_DEFINITION,
    &ALTER_INSTANCE,
    &ALTER_SEQUENCE,
    &ALTER_OBJECT_SET_SCHEMA,
    &VIEW_DEFINITION_OPTIONS,
    &DESCRIBE_SUMMARIZE,
    &INDEX_DROP_ON_TABLE,
    &EXTENDED_SCALAR_TYPE_NAMES,
    &ENUM_TYPE,
    &SET_TYPE,
    &NUMERIC_MODIFIERS,
    &INTEGER_DISPLAY_WIDTH,
    &COMPOSITE_TYPES,
    &VARCHAR_REQUIRES_LENGTH,
    &ZONED_TEMPORAL_TYPES,
    &EMPTY_TYPE_PARENS,
    &CHARACTER_SET_ANNOTATION,
    &SIGNED_TYPE_MODIFIER,
    &LIBERAL_TYPE_NAMES,
    &STRING_TYPE_MODIFIERS,
];

/// What a labelled case expects of a parse: an accept/reject outcome, or — for a
/// structural divergence where both feature settings parse — a required parse *shape*.
#[derive(Clone, Copy)]
enum Expect {
    Accept,
    Reject,
    /// The parse must succeed and satisfy this structural predicate. Unlike
    /// accept/reject, a structural case stays *parseable* when a declared feature is
    /// flipped — only its shape changes — which is what gives `forbidden_features` a
    /// real case (the `pipe_operator` `||`-as-concat vs `||`-as-OR divergence).
    Shape(fn(&Parsed) -> bool),
}

impl Expect {
    /// Whether the expectation holds for `sql` parsed under `features`: the declared
    /// accept/reject outcome, or (for a structural case) a successful parse whose shape
    /// matches the predicate.
    ///
    /// The set is projected onto the parser's self-consistency precondition first
    /// (`FeatureSet::without_dangling_dependents`): a case baseline may carry a
    /// dependency dangler — a refinement flag whose base a `forbidden` entry turned off —
    /// which is inert, so clearing it before the parse-entry `debug_assert!` sees it is
    /// outcome-preserving. Baselines never carry a *lexical* or *grammar* conflict (that
    /// would be a soundness hazard with no defined parse); `baselines_are_self_consistent`
    /// guards that invariant.
    fn holds(self, sql: &str, features: &FeatureSet) -> bool {
        let features = &features.without_dangling_dependents();
        match self {
            Self::Accept => accepts_under(sql, features),
            Self::Reject => !accepts_under(sql, features),
            Self::Shape(matches_shape) => {
                matches!(parse_with(sql, squonk::ParseConfig::new(AdHocDialect(features))), Ok(parsed) if matches_shape(&parsed))
            }
        }
    }

    /// Whether flipping a declared feature *genuinely* changed the outcome — the
    /// ZetaSQL falsely-declared check. An accept/reject case must flip accept<->reject;
    /// a structural case must still parse while its shape check fails under the flip
    /// (a changed shape, not merely a different accept/reject outcome).
    ///
    /// A flip that turns a `forbidden` feature *on* can make the set carry a registered
    /// `LexicalConflict` or `GrammarConflict` — the flipped feature contends with an
    /// enabled one for a shared tokenizer trigger or parser-position head. Such a set has
    /// no defined parse (the parse-entry `debug_assert!` forbids it), so the registry is
    /// the authority: the contention definitionally shadows one reading, i.e. the flip
    /// changed the outcome. A flip that only leaves a dependency dangler is normalized away
    /// (inert, outcome-preserving) before the parse.
    fn flip_changes_outcome(self, sql: &str, flipped: &FeatureSet) -> bool {
        if flipped.lexical_conflict().is_some() || flipped.grammar_conflict().is_some() {
            return true;
        }
        let flipped = &flipped.without_dangling_dependents();
        match self {
            Self::Accept => !accepts_under(sql, flipped),
            Self::Reject => accepts_under(sql, flipped),
            Self::Shape(matches_shape) => {
                matches!(parse_with(sql, squonk::ParseConfig::new(AdHocDialect(flipped))), Ok(parsed) if !matches_shape(&parsed))
            }
        }
    }
}

/// A conformance case labelled with the dialect features it exercises.
struct LabeledCase {
    sql: &'static str,
    /// The outcome when every `required` feature is enabled and every `forbidden`
    /// feature disabled.
    expect: Expect,
    /// Features that must be enabled for `expect` to hold; disabling any one flips
    /// the outcome (the falsely-required check).
    required: &'static [&'static ToggleableFeature],
    /// Features that must be disabled for `expect` to hold; enabling any one changes
    /// the outcome. No accept/reject case needs one — every gated sub-flag is additive
    /// — but the structural `pipe_operator` case forbids `logical_or_pipe` (enabling it
    /// re-parses `||` as OR), and the skip logic supports it for restrictive features
    /// (see `forbidden_labels_drive_skip_logic`).
    forbidden: &'static [&'static ToggleableFeature],
}

impl LabeledCase {
    /// The dialect `FeatureSet` under which `expect` holds: `POSTGRES` (all
    /// sub-flags on) with every forbidden feature forced off and every required
    /// feature forced on.
    fn baseline(&self) -> FeatureSet {
        let mut features = FeatureSet::POSTGRES;
        for feature in self.required {
            features = (feature.set_enabled)(&features, true);
        }
        for feature in self.forbidden {
            features = (feature.set_enabled)(&features, false);
        }
        features
    }

    /// Whether this case applies to a dialect: all required features enabled and
    /// all forbidden features disabled. A case that does not apply is skipped.
    fn applies_to(&self, features: &FeatureSet) -> bool {
        self.required.iter().all(|f| (f.is_enabled)(features))
            && self.forbidden.iter().all(|f| !(f.is_enabled)(features))
    }

    /// Every declared (required or forbidden) feature, for the falsely-declared pass.
    fn declared(&self) -> impl Iterator<Item = &&'static ToggleableFeature> {
        self.required.iter().chain(self.forbidden)
    }
}

const LABELED_CASES: &[LabeledCase] = &[
    // `E'x'`: with escape strings off, `E` lexes as an ordinary word and `'x'` as a
    // string, so `E'x'` re-reads as the generalized typed literal `E 'x'` — an
    // `Expr::Cast`, not the escape-string `Expr::Literal` (prod-literal-generic-typed).
    // Both *parse*, so the flag's effect here is structural, not accept/reject.
    LabeledCase {
        sql: "SELECT E'x'",
        expect: Expect::Shape(projection_is_a_string_constant),
        required: &[&ESCAPE_STRINGS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT $$x$$",
        expect: Expect::Accept,
        required: &[&DOLLAR_QUOTED],
        forbidden: &[],
    },
    // `N'x'`: with national strings off, `N` lexes as a word and `'x'` as a string, so
    // `N'x'` re-reads as the typed literal `N 'x'` (an `Expr::Cast`) rather than the
    // national-string `Expr::Literal` — a structural flip, not accept/reject (like
    // `E'x'`, prod-literal-generic-typed).
    LabeledCase {
        sql: "SELECT N'x'",
        expect: Expect::Shape(projection_is_a_string_constant),
        required: &[&NATIONAL_STRINGS],
        forbidden: &[],
    },
    // `_utf8'x'`: with charset introducers off, `_utf8` lexes as a word and `'x'` as a
    // string, so `_utf8'x'` re-reads as the generalized typed literal `_utf8 'x'` (an
    // `Expr::Cast`) rather than the charset-introduced `Expr::Literal` — a structural
    // flip, not accept/reject (like `N'x'`, prod-literal-generic-typed).
    LabeledCase {
        sql: "SELECT _utf8'x'",
        expect: Expect::Shape(projection_is_a_string_constant),
        required: &[&CHARSET_INTRODUCERS],
        forbidden: &[],
    },
    // `_latin1"x"`: MySQL accepts the charset introducer before a *double*-quoted string
    // too, but only when `"..."` is itself a string (`double_quoted_strings` on, MySQL
    // `ANSI_QUOTES` off). It then folds into one charset-introduced `Expr::Literal`, the
    // mirror of the single-quoted `_utf8'x'` case above — so *both* knobs are required.
    // Flipping either one keeps the SQL parsing but changes the shape (a structural flip,
    // like `N'x'`): with `charset_introducers` off it re-reads as the typed literal
    // `_latin1 "x"` (an `Expr::Cast`); with `double_quoted_strings` off `"x"` is a quoted
    // identifier that aliases the column `_latin1`.
    LabeledCase {
        sql: "SELECT _latin1\"x\"",
        expect: Expect::Shape(projection_is_a_string_constant),
        required: &[&CHARSET_INTRODUCERS, &DOUBLE_QUOTED_STRINGS],
        forbidden: &[],
    },
    // `"x"` in *table-name* position. The bare-expression `SELECT "x"` no longer
    // flips accept/reject now that the parser accepts quoted identifiers
    // (prod-sql-quoted-identifiers): on, `"x"` is a string literal; off, a quoted
    // column ref — both *parse*, so there the flag's effect is structural, asserted
    // in `double_quoted_strings_flips_string_vs_quoted_ident`. Identifier position
    // keeps an objective accept/reject flip: on, `"x"` is a string literal, which is
    // not a valid table name -> reject; off, it is a quoted identifier, a valid
    // table name -> accept. So the flag stays genuinely required.
    LabeledCase {
        sql: "SELECT * FROM \"x\"",
        expect: Expect::Reject,
        required: &[&DOUBLE_QUOTED_STRINGS],
        forbidden: &[],
    },
    // A ragged VALUES constructor (`VALUES (1, 2), (3)`) is a parse-time reject under the
    // equal-arity gate (DuckDB: `VALUES lists must all be the same length`; SQLite: `all
    // VALUES must have the same number of terms`); with the gate on it is a parse error,
    // and clearing the flag accepts the statement (PostgreSQL defers the check to bind) —
    // an objective accept/reject flip this flag alone drives.
    LabeledCase {
        sql: "VALUES (1, 2), (3)",
        expect: Expect::Reject,
        required: &[&VALUES_ROWS_REQUIRE_EQUAL_ARITY],
        forbidden: &[],
    },
    // A recursive CTE whose `UNION` body carries a top-level `ORDER BY` is rejected with
    // the recursive-query modifier gate on (DuckDB: `ORDER BY in a recursive query is not
    // allowed`) and accepted with it off (PostgreSQL parse-accepts, deferring the
    // recursion restriction to bind) — an objective accept/reject flip this flag alone
    // drives.
    LabeledCase {
        sql: "WITH RECURSIVE t AS (SELECT 1 AS x UNION ALL SELECT x + 1 FROM t WHERE x < 3 ORDER BY x) SELECT * FROM t",
        expect: Expect::Reject,
        required: &[&RECURSIVE_UNION_REJECTS_ORDER_LIMIT],
        forbidden: &[],
    },
    // A bare-parenthesized query-position `VALUES (1)` is accepted with the row-constructor
    // gate on (PostgreSQL/standard) and rejected with it off (MySQL spells the query-position
    // form `VALUES ROW(1)`), an objective accept/reject flip this flag alone drives (the
    // `INSERT … VALUES (…)` source is a separate path the gate never reaches).
    LabeledCase {
        sql: "VALUES (1)",
        expect: Expect::Accept,
        required: &[&VALUES_ROW_CONSTRUCTOR],
        forbidden: &[],
    },
    // A schema-scoped grant object (`GRANT … ON SCHEMA s …`) is accepted with the extended
    // grant-object gate on (PostgreSQL/standard) and rejected with it off (MySQL has no
    // `ON SCHEMA`/`ON DATABASE` object nor the `{GRANT|ADMIN} OPTION FOR` prefix), an
    // objective accept/reject flip this flag alone drives (`access_control` stays on, so the
    // `GRANT` statement is still dispatched — only the object grammar narrows).
    LabeledCase {
        sql: "GRANT SELECT ON SCHEMA s TO alice",
        expect: Expect::Accept,
        required: &[&ACCESS_CONTROL_EXTENDED_OBJECTS],
        forbidden: &[],
    },
    // The prefix-typed interval literal (`INTERVAL '1' HOUR TO SECOND`, the ANSI spelling
    // only this literal path reads) parses with the gate on (ANSI/PostgreSQL/DuckDB/Lenient)
    // and rejects with it off (MySQL has no first-class interval literal — every typed
    // `INTERVAL '…'` form is 1064 on mysql:8.4.10): `INTERVAL` falls back to an ordinary
    // name and the trailing string is a clean parse error — an objective accept/reject
    // flip this flag alone drives.
    LabeledCase {
        sql: "SELECT INTERVAL '1' HOUR TO SECOND",
        expect: Expect::Accept,
        required: &[&TYPED_INTERVAL_LITERAL],
        forbidden: &[],
    },
    // The MySQL operator-position interval `INTERVAL <int> <unit>` (`NOW() - INTERVAL 3 DAY`)
    // parses with the operator gate on (MySQL's `Item_date_add_interval` operand) and rejects
    // with it off (without it a bare-integer `INTERVAL 3` is neither a quoted-string literal
    // nor a DuckDB relaxed amount, so `INTERVAL` falls back to a name and the trailing tokens
    // do not parse) — an objective accept/reject flip this flag alone drives.
    LabeledCase {
        sql: "SELECT NOW() - INTERVAL 3 DAY",
        expect: Expect::Accept,
        required: &[&MYSQL_INTERVAL_OPERATOR],
        forbidden: &[],
    },
    // An empty built-in aggregate call (`COUNT()`) is rejected with the empty-argument
    // gate on (MySQL's aggregate grammar requires an argument or `COUNT(*)`) and accepted
    // with it off (every other dialect admits the empty call, deferring the arity check to
    // bind) — an objective accept/reject flip this flag alone drives.
    LabeledCase {
        sql: "SELECT COUNT()",
        expect: Expect::Reject,
        required: &[&AGGREGATE_CALLS_REJECT_EMPTY_ARGUMENTS],
        forbidden: &[],
    },
    // `OVER` on a non-windowable function (`ABS(x) OVER ()`) is rejected with the
    // windowable-function gate on (MySQL admits `OVER` only on aggregate ∪ window
    // functions) and accepted with it off (other dialects attach `OVER` to any call) — an
    // objective accept/reject flip this flag alone drives.
    LabeledCase {
        sql: "SELECT ABS(x) OVER ()",
        expect: Expect::Reject,
        required: &[&OVER_REQUIRES_WINDOWABLE_FUNCTION],
        forbidden: &[],
    },
    // `'a\'b'`: with backslash escapes on, `\'` does not terminate, so the whole
    // string is one token. Off, `'a\'` closes and the trailing `'` is unterminated.
    LabeledCase {
        sql: "SELECT 'a\\'b'",
        expect: Expect::Accept,
        required: &[&BACKSLASH_ESCAPES],
        forbidden: &[],
    },
    // `U&'x'`: with unicode strings on, `U&'x'` is one Unicode-escape string constant. Off,
    // `U` is a word, `&` a bitwise-AND operator (the PostgreSQL baseline admits the bitwise
    // family) and `'x'` a string, so it re-reads as the expression `U & 'x'` — a structural
    // flip (a `BinaryOp`, not the string `Expr::Literal`), like `E'x'`/`N'x'`, not
    // accept/reject.
    LabeledCase {
        sql: "SELECT U&'x'",
        expect: Expect::Shape(projection_is_a_string_constant),
        required: &[&UNICODE_STRINGS],
        forbidden: &[],
    },
    // `B'1010'`: with bit strings off, `B` lexes as a word and `'1010'` as a string, so
    // `B'1010'` re-reads as the typed literal `B '1010'` (an `Expr::Cast`) rather than
    // the bit-string `Expr::Literal` — a structural flip, not accept/reject (like `N'x'`,
    // prod-literal-generic-typed).
    LabeledCase {
        sql: "SELECT B'1010'",
        expect: Expect::Shape(projection_is_a_string_constant),
        required: &[&BIT_STRING_LITERALS],
        forbidden: &[],
    },
    // `x'ABC'`: the SQLite/MySQL hexadecimal blob literal is validated *eagerly* at lex
    // time (an even count of hex digits), so an odd-length body is a tokenize-time
    // reject — the boundary both engines enforce (probed: SQLite "unrecognized token",
    // MySQL ER_PARSE_ERROR). The malformed body is what makes the label decisive against
    // the POSTGRES flip baseline: with `blob_literals` off, the deferred
    // `bit_string_literals` arm reclaims `x'…'` and `x'ABC'` parses (odd hex tolerated),
    // so the flip is a clean reject<->accept. The even-hex accept direction rides the
    // parser/tokenizer regression tests (identical `BitString` shape to `X'…'` above).
    LabeledCase {
        sql: "SELECT x'ABC'",
        expect: Expect::Reject,
        required: &[&BLOB_LITERALS],
        forbidden: &[],
    },
    // Radix / separator numeric forms. The trailing `+ 1` makes the split-token
    // reading (e.g. `0` then word `x1F`) a hard parse error rather than an
    // implicit alias, so the "flag off -> reject" half is robust.
    LabeledCase {
        sql: "SELECT 0x1F + 1",
        expect: Expect::Accept,
        required: &[&HEX_INTEGERS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT 0o17 + 1",
        expect: Expect::Accept,
        required: &[&OCTAL_INTEGERS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT 0b101 + 1",
        expect: Expect::Accept,
        required: &[&BINARY_INTEGERS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT 1_500 + 1",
        expect: Expect::Accept,
        required: &[&UNDERSCORE_SEPARATORS],
        forbidden: &[],
    },
    // A radix body that opens with `_` (`0x_1F`, PG's `0[xX](_?{hexdigit})+`): with the
    // flag on it lexes as one hex number -> accept; with it off the radix does not open,
    // so the bare `0` abuts the word `x_1F` and (PostgreSQL's `reject_trailing_junk` being
    // on in the baseline) rejects as trailing junk — an accept/reject flip. SQLite sets
    // this off (`0x_1F` rejects there); PostgreSQL sets it on.
    LabeledCase {
        sql: "SELECT 0x_1F + 1",
        expect: Expect::Accept,
        required: &[&RADIX_LEADING_UNDERSCORE],
        forbidden: &[],
    },
    // `123abc` is trailing junk after a numeric literal — rejected with the strict-scanner
    // gate on (PostgreSQL/SQLite treat a number as maximal munch) and accepted with it off
    // (DuckDB/MySQL re-read it as `123` aliased `abc`) — an objective accept/reject flip.
    LabeledCase {
        sql: "SELECT 123abc",
        expect: Expect::Reject,
        required: &[&REJECT_TRAILING_JUNK],
        forbidden: &[],
    },
    // T-SQL money literal, coupled with positional `$n`: both claim `$`+digit
    // (`LexicalConflict::MoneyVersusPositionalDollar`), so a money dialect vacates
    // positional. With money on (positional vacated) `$1234.56` is one money literal ->
    // accept; with money off (positional restored) it splits into the `$1234` parameter
    // and a `.56` number — two adjacent primaries, a hard parse error — so the flag is
    // genuinely required. (The money-before-positional scan precedence itself is pinned
    // by the tokenizer test `money_resolves_the_dollar_dispatch_deterministically`.)
    LabeledCase {
        sql: "SELECT $1234.56",
        expect: Expect::Accept,
        required: &[&MONEY_LITERALS],
        forbidden: &[],
    },
    // `#` line comment: on, `# trailing comment` is skipped and the `+ 2` on the next line
    // continues, so the statement is `SELECT 1 + 2`. Off, the coupled toggle restores the
    // PostgreSQL `#` XOR operator (`SELECT 1 # trailing …`), whose second operand `trailing`
    // is then followed by the bare word `comment` — unexpected input -> reject. So the flip
    // stays a clean accept/reject swap even though `#` has an operator meaning off the
    // comment side.
    LabeledCase {
        sql: "SELECT 1 # trailing comment\n+ 2",
        expect: Expect::Accept,
        required: &[&LINE_COMMENT_HASH],
        forbidden: &[],
    },
    // Carriage-return line-comment termination is dialect data (Postgres/DuckDb, not
    // Sqlite/MySql — tokenizer-line-comment-terminator-set). On, a `--` comment ends at the
    // `\r`, so the trailing `FROM` is a live token and `SELECT 1 FROM` is a syntax error;
    // off, the whole `-- c\rFROM` tail is comment content and `SELECT 1` accepts. The
    // reproducer-shaped reject flips to accept when the flag is disabled.
    LabeledCase {
        sql: "SELECT 1 -- c\rFROM",
        expect: Expect::Reject,
        required: &[&LINE_COMMENT_ENDS_AT_CARRIAGE_RETURN],
        forbidden: &[],
    },
    // Block-comment nesting is dialect data: with nesting (the baseline) the
    // outer comment swallows the balanced inner one, leaving `SELECT 1`; without
    // it (MySQL) the first `*/` closes the comment, so the tail leaks as tokens.
    // The leaked tail here is a lone `'` opening an unterminated string literal —
    // a hard *lexer* error that no operator reading can rescue (an ordinary word
    // tail would parse as an operand of the leaked `*/` under the general operator
    // surface, so it would not discriminate).
    LabeledCase {
        sql: "SELECT /* a /* b */ ' */ 1",
        expect: Expect::Accept,
        required: &[&NESTED_BLOCK_COMMENTS],
        forbidden: &[],
    },
    // MySQL versioned comments: with the gate on the body is live input
    // (`SELECT 1 AS x`); off, the whole construct is a skipped comment and the
    // projection-less `SELECT AS x` is a hard parse error.
    LabeledCase {
        sql: "SELECT /*!40101 1 */ AS x",
        expect: Expect::Accept,
        required: &[&VERSIONED_COMMENTS],
        forbidden: &[],
    },
    // Parameter placeholders. With the flag off the sigil is a stray byte (and `$1`
    // is not a valid dollar-quote opener even under POSTGRES, which keeps
    // dollar-quoting on), so the "flag off -> reject" half is a hard error.
    LabeledCase {
        sql: "SELECT $1",
        expect: Expect::Accept,
        required: &[&POSITIONAL_DOLLAR],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT ?",
        expect: Expect::Accept,
        required: &[&ANONYMOUS_QUESTION],
        forbidden: &[],
    },
    // Named placeholders. With `named_colon` off the lone `:` is the slice-separator
    // punctuation, so `SELECT :name` is a parse error; with `named_at` off the `@` is a
    // lexical stray byte — both flip the outcome, so each label is genuine. Each is
    // coupled with the PostgreSQL feature it shares a trigger with — `named_colon` with
    // `subscript` (`ColonParameterVersusSliceBound`) and `named_at` with the `<@`
    // containment operator (`ContainmentOperatorVersusAtName`) — so enabling the sigil
    // vacates that rival and the baseline stays lexically consistent.
    LabeledCase {
        sql: "SELECT :name",
        expect: Expect::Accept,
        required: &[&NAMED_COLON],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT @name",
        expect: Expect::Accept,
        required: &[&NAMED_AT],
        forbidden: &[],
    },
    // MySQL session variables. With the flag off the sigil is a lexical stray byte, so
    // each flip is a hard reject. `user_variables` is coupled with the `<@` containment
    // operator (`ContainmentOperatorVersusAtName`) — a `@var` read and `<@` share the
    // `<`+`@` trigger — so enabling it vacates `containment_operators`, keeping the
    // baseline lexically consistent. `named_at` stays off (POSTGRES baseline), so `@x`
    // never contends with a parameter, and the `@@` system form is disjoint from every
    // single-`@` form by its second `@`, so `system_variables` needs no such coupling.
    LabeledCase {
        sql: "SELECT @user_count",
        expect: Expect::Accept,
        required: &[&USER_VARIABLES],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT @@global.time_zone",
        expect: Expect::Accept,
        required: &[&SYSTEM_VARIABLES],
        forbidden: &[],
    },
    // The MySQL variable-assignment SET grammar: a scope-keyword-prefixed assignment the
    // base's generic single-target `SET` cannot parse, so the flag alone flips accept/reject.
    LabeledCase {
        sql: "SET GLOBAL max_connections = 1",
        expect: Expect::Accept,
        required: &[&VARIABLE_ASSIGNMENT],
        forbidden: &[],
    },
    // `foo$bar`: with `$` allowed in identifiers it lexes as one identifier -> accept.
    // Off, `foo` ends at the `$`; the POSTGRES baseline keeps dollar-quoting on, but a
    // lone `$bar` opens no complete dollar-quote, so the `$` is a hard lexical error.
    LabeledCase {
        sql: "SELECT foo$bar",
        expect: Expect::Accept,
        required: &[&DOLLAR_IN_IDENTIFIERS],
        forbidden: &[],
    },
    // SQLite's string-literal identifier misfeature: with the flag on, `'table1'` is read
    // as the relation-target name so `DELETE FROM 'table1'` parses; off, a string is not a
    // valid target name and the statement rejects — an objective accept/reject flip this
    // flag alone drives.
    LabeledCase {
        sql: "DELETE FROM 'table1'",
        expect: Expect::Accept,
        required: &[&STRING_LITERAL_IDENTIFIERS],
        forbidden: &[],
    },
    // SQLite's empty quoted identifier: with the flag on the zero-length `""` lexes as an
    // (empty) quoted identifier and `SELECT ""` parses; off, the tokenizer rejects a
    // zero-length delimited identifier — an objective accept/reject flip this flag alone drives
    // (the POSTGRES base quotes `"` as an identifier, so `""` never reads as a string).
    LabeledCase {
        sql: "SELECT \"\"",
        expect: Expect::Accept,
        required: &[&EMPTY_QUOTED_IDENTIFIERS],
        forbidden: &[],
    },
    // SQLite silently closes an unterminated `/* …` at EOF: with the flag on the trailing
    // comment is trivia so `SELECT 1/*x` parses as `SELECT 1`; off, it is the hard
    // unterminated-block-comment lexer error.
    LabeledCase {
        sql: "SELECT 1/*x",
        expect: Expect::Accept,
        required: &[&UNTERMINATED_BLOCK_COMMENT_AT_EOF],
        forbidden: &[],
    },
    // SQLite's numbered `?NNN` parameter: with the flag on `?1` lexes as the numbered
    // placeholder (a maximal digit munch), so `SELECT ?1abc` is `?1` aliased `abc` and parses;
    // off, the base reads `?` as a prefix operator whose operand `1abc` is trailing-junk
    // (`reject_trailing_junk` is on under POSTGRES), a hard lexer error — so the flag alone
    // drives the flip. (`SELECT ?1` alone would parse either way, as `?` prefixes `1`.)
    LabeledCase {
        sql: "SELECT ?1abc",
        expect: Expect::Accept,
        required: &[&NUMBERED_QUESTION],
        forbidden: &[],
    },
    // SQLite's bare (`AS`-less) string alias: with the flag on `SELECT 1 'x'` reads `'x'` as
    // the column name; off, the string is left unconsumed after the projection and the
    // statement rejects — an accept/reject flip this flag alone drives.
    LabeledCase {
        sql: "SELECT 1 'x'",
        expect: Expect::Accept,
        required: &[&BARE_ALIAS_STRING_LITERALS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM LATERAL (SELECT 1) AS s",
        expect: Expect::Accept,
        required: &[&LATERAL],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM generate_series(1, 3)",
        expect: Expect::Accept,
        required: &[&TABLE_FUNCTIONS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM ROWS FROM (generate_series(1, 3))",
        expect: Expect::Accept,
        required: &[&ROWS_FROM],
        forbidden: &[],
    },
    // `WITH ORDINALITY` on the generic table-function factor. With the flag off the tail
    // is left unconsumed and the statement rejects (SQLite admits `table_functions` but
    // syntax-rejects `WITH ORDINALITY`, engine-probed), so it drives an objective
    // accept/reject flip; `table_functions` is co-required — the function factor must
    // parse for the tail to attach.
    LabeledCase {
        sql: "SELECT * FROM generate_series(1, 3) WITH ORDINALITY",
        expect: Expect::Accept,
        required: &[&TABLE_FUNCTIONS, &TABLE_FUNCTION_ORDINALITY],
        forbidden: &[],
    },
    // `FROM unnest(…)` is the first-class UNNEST factor. With `unnest` off it still
    // parses (PostgreSQL keeps `table_functions` on, so the generic path reads it as a
    // `TableFactor::Function`), so the flag's effect is structural — the sole FROM
    // relation is `TableFactor::Unnest` only with the flag on.
    LabeledCase {
        sql: "SELECT * FROM unnest(ARRAY[1, 2, 3]) WITH ORDINALITY AS u(v, ord)",
        expect: Expect::Shape(from_relation_is_unnest),
        required: &[&UNNEST],
        forbidden: &[],
    },
    // BigQuery `UNNEST(…) WITH OFFSET` — the preset-less offset tail. With
    // `unnest_with_offset` off the `WITH OFFSET` is left unconsumed and the statement
    // rejects (PostgreSQL/DuckDB both parse-reject it), so this flag drives an objective
    // accept/reject flip; `unnest` is co-required — the factor must be the first-class
    // node for the tail to attach (with `unnest` off the fallback function path errors on
    // the `WITH OFFSET` too).
    LabeledCase {
        sql: "SELECT * FROM unnest(ARRAY[1, 2, 3]) WITH OFFSET AS off",
        expect: Expect::Accept,
        required: &[&UNNEST, &UNNEST_WITH_OFFSET],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM ONLY t",
        expect: Expect::Accept,
        required: &[&ONLY],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM t TABLESAMPLE BERNOULLI (10)",
        expect: Expect::Accept,
        required: &[&TABLE_SAMPLE],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM (t JOIN u ON t.a = u.a)",
        expect: Expect::Accept,
        required: &[&PARENTHESIZED_JOINS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM t AS a(c)",
        expect: Expect::Accept,
        required: &[&TABLE_ALIAS_COLUMN_LISTS],
        forbidden: &[],
    },
    // DuckDB's string-literal table alias: `AS 't'` is a valid correlation name only
    // when the flag is on; with it off the string is not a `ColId` and the alias parse
    // rejects (an accept/reject fork, the `table_alias_column_lists` pattern). The
    // bare-name-plus-string-column form (`t('k')`) additionally relies on the
    // DuckDb-baseline corpus, so it has dedicated unit tests rather than a case here.
    LabeledCase {
        sql: "SELECT * FROM t AS 'x'",
        expect: Expect::Accept,
        required: &[&STRING_LITERAL_ALIASES],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM t JOIN u USING (a) AS x",
        expect: Expect::Accept,
        required: &[&JOIN_USING_ALIAS],
        forbidden: &[],
    },
    // MySQL `SELECT STRAIGHT_JOIN ...`: with the flag off, `STRAIGHT_JOIN` is an
    // ordinary (non-reserved) word the projection reads as a column aliased by the
    // following name, so both *parse* — the flag's effect is structural (the
    // `straight_join` modifier flag), not accept/reject, like the `pipe_operator`
    // case. The join-operator surface additionally relies on `STRAIGHT_JOIN` being a
    // reserved bare alias (true only under MySQL), so it has dedicated unit tests
    // rather than a POSTGRES-baseline labelled case.
    LabeledCase {
        sql: "SELECT STRAIGHT_JOIN x",
        expect: Expect::Shape(select_has_straight_join_modifier),
        required: &[&STRAIGHT_JOIN],
        forbidden: &[],
    },
    // DuckDB nonstandard joins: the explicit `AS a` alias keeps `ASOF`/`POSITIONAL`
    // out of alias-competition position (`asof`/`positional` are unreserved outside
    // DuckDb), so with the flag off the trailing `... JOIN u ...` is leftover input
    // -> reject; each flag is genuinely required (an accept/reject fork, the
    // `index_hints` pattern). The bare-factor spellings additionally rely on the
    // DuckDb ColId reservation, so they have dedicated unit tests rather than
    // POSTGRES-baseline labelled cases.
    LabeledCase {
        sql: "SELECT * FROM t AS a ASOF JOIN u ON a.x >= u.x",
        expect: Expect::Accept,
        required: &[&ASOF_JOIN],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM t AS a POSITIONAL JOIN u",
        expect: Expect::Accept,
        required: &[&POSITIONAL_JOIN],
        forbidden: &[],
    },
    // DuckDB SEMI/ANTI joins: the explicit `AS a` alias keeps `SEMI`/`ANTI` out of
    // alias-competition position (both are unreserved outside DuckDb), so with the
    // flag off the trailing `... JOIN u ON …` is leftover input -> reject; the flag is
    // genuinely required (the `asof_join` pattern). The one flag gates both keywords,
    // so a case per keyword exercises the same fork. The bare-factor spellings
    // additionally rely on the DuckDb ColId reservation, so they have dedicated unit
    // tests rather than POSTGRES-baseline labelled cases.
    LabeledCase {
        sql: "SELECT * FROM t AS a SEMI JOIN u ON a.x = u.x",
        expect: Expect::Accept,
        required: &[&SEMI_ANTI_JOIN],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM t AS a ANTI JOIN u ON a.x = u.x",
        expect: Expect::Accept,
        required: &[&SEMI_ANTI_JOIN],
        forbidden: &[],
    },
    // Spark/Hive sided semi-join: with `sided_semi_anti_join` off the leading `LEFT`
    // reads as a plain `LEFT JOIN` side and the following `SEMI` is leftover input ->
    // reject, so the flag is genuinely required. The POSTGRES baseline has
    // `semi_anti_join` off, so an accept here also proves the sided spelling rides the
    // *separate* `sided_semi_anti_join` gate (DuckDB parse-rejects the sided form) — not
    // DuckDB's side-less flag. The `AS a` alias keeps `LEFT` in join position; the other
    // three sided spellings ride the same flag.
    LabeledCase {
        sql: "SELECT * FROM t AS a LEFT SEMI JOIN u ON a.x = u.x",
        expect: Expect::Accept,
        required: &[&SIDED_SEMI_ANTI_JOIN],
        forbidden: &[],
    },
    // SQLite `NATURAL CROSS JOIN`: `NATURAL` is a reserved keyword that anchors the join,
    // so no alias-model interplay is needed; with `natural_cross_join` off the `NATURAL`
    // arm falls through to its mandatory `JOIN` on the `CROSS` token and rejects, so the
    // flag is genuinely required (an accept/reject fork). PostgreSQL/DuckDB parse-reject it.
    LabeledCase {
        sql: "SELECT * FROM t NATURAL CROSS JOIN u",
        expect: Expect::Accept,
        required: &[&NATURAL_CROSS_JOIN],
        forbidden: &[],
    },
    // DuckDB pivot operators: the explicit `AS a` alias keeps `PIVOT`/`UNPIVOT` out
    // of alias-competition position (both are unreserved in the POSTGRES flip-baseline
    // these cases parse under), so with the flag off the trailing `(…)` body is leftover
    // input -> reject; the leading-keyword statement forms fork on the same flags (an
    // unknown statement when off). Each flag gates both of its operator's surfaces as one
    // dialect unit (the `straight_join` precedent); the bare-factor spellings additionally
    // rely on a `ColId` reservation (DuckDb, and BigQuery/Snowflake per
    // `bigquery-snowflake-pivot-keyword-reservation`), so they have dedicated unit tests
    // rather than POSTGRES-baseline labelled cases.
    LabeledCase {
        sql: "SELECT * FROM t AS a PIVOT (sum(x) FOR y IN (1, 2))",
        expect: Expect::Accept,
        required: &[&PIVOT],
        forbidden: &[],
    },
    LabeledCase {
        sql: "PIVOT t ON y USING sum(x) GROUP BY z",
        expect: Expect::Accept,
        required: &[&PIVOT],
        forbidden: &[],
    },
    // The standard PIVOT table factor's extended value sources / default
    // (`planner-parity-pivot-multidialect-fields`): `pivot_value_sources` both reaches
    // the standard table-factor PIVOT (independent of the DuckDB `pivot` flag) and admits
    // the `IN (ANY [ORDER BY …])` wildcard, the `IN (<subquery>)` source, and the
    // Snowflake `DEFAULT ON NULL (<expr>)` tail. The explicit `AS p` alias keeps `PIVOT`
    // out of alias-competition (it is unreserved in the POSTGRES flip-baseline, the base
    // pivot convention above); with the flag off the trailing `(…)` is leftover input -> reject.
    LabeledCase {
        sql: "SELECT * FROM t AS p PIVOT (sum(x) FOR y IN (ANY ORDER BY y) DEFAULT ON NULL (0))",
        expect: Expect::Accept,
        required: &[&PIVOT_VALUE_SOURCES],
        forbidden: &[],
    },
    // `pivot_value_sources` doubles as the standard UNPIVOT table factor's reachability
    // gate off the DuckDB `unpivot` flag (PIVOT/UNPIVOT co-travel): the shared
    // BigQuery/Snowflake surface — the `EXCLUDE NULLS` marker, per-column aliases, and
    // value/name lists are all DuckDB fields reused. The explicit `AS u` alias keeps
    // `UNPIVOT` out of alias-competition; with the flag off (and `unpivot` off in the
    // POSTGRES baseline) the suffix is unreachable -> reject.
    LabeledCase {
        sql: "SELECT * FROM t AS u UNPIVOT EXCLUDE NULLS (v FOR n IN (b AS x, c))",
        expect: Expect::Accept,
        required: &[&PIVOT_VALUE_SOURCES],
        forbidden: &[],
    },
    // The SQL:2016 MATCH_RECOGNIZE row-pattern table factor
    // (`planner-parity-table-factor-match-recognize`), gated on `match_recognize`
    // (Snowflake/Lenient). The explicit `AS m` alias keeps `MATCH_RECOGNIZE` out of
    // alias-competition (it is unreserved in the POSTGRES flip-baseline — the standard-PIVOT
    // convention above); with the flag off the trailing `MATCH_RECOGNIZE (…)` is leftover
    // input -> reject.
    LabeledCase {
        sql: "SELECT * FROM t AS m MATCH_RECOGNIZE (PATTERN (A) DEFINE A AS a > 0)",
        expect: Expect::Accept,
        required: &[&MATCH_RECOGNIZE],
        forbidden: &[],
    },
    // SQL Server's OPENJSON rowset-function table factor
    // (`planner-parity-table-factor-openjson`), gated on `open_json` (MSSQL/Lenient). The
    // `WITH (…)` schema carries a column path string literal and the `AS JSON` marker; with
    // the flag off `OPENJSON(` falls to the ordinary function/name path and the `WITH (…)`
    // tail is leftover input -> reject.
    LabeledCase {
        sql: "SELECT * FROM OPENJSON(j, '$.items') WITH (id INT '$.id', doc TEXT '$.doc' AS JSON)",
        expect: Expect::Accept,
        required: &[&OPEN_JSON],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM t AS a UNPIVOT (v FOR n IN (b, c))",
        expect: Expect::Accept,
        required: &[&UNPIVOT],
        forbidden: &[],
    },
    LabeledCase {
        sql: "UNPIVOT t ON a, b INTO NAME n VALUE v",
        expect: Expect::Accept,
        required: &[&UNPIVOT],
        forbidden: &[],
    },
    // DuckDB `PIVOT`/`UNPIVOT` as a *query body* (`duckdb-statement-in-query-position`):
    // the same `pivot`/`unpivot` flag admits the operator at the CTE / `CREATE VIEW … AS`
    // body position (`SetExpr::Pivot`), so off the flag the CTE body has no `PIVOT`
    // query-body reading and the statement rejects — a genuine accept/reject fork.
    LabeledCase {
        sql: "WITH p AS (PIVOT t ON y USING sum(x) GROUP BY z) SELECT * FROM p",
        expect: Expect::Accept,
        required: &[&PIVOT],
        forbidden: &[],
    },
    LabeledCase {
        sql: "WITH u AS (UNPIVOT t ON a, b INTO NAME n VALUE v) SELECT * FROM u",
        expect: Expect::Accept,
        required: &[&UNPIVOT],
        forbidden: &[],
    },
    // DuckDB `DESCRIBE`/`SHOW` as a parenthesized `FROM` table source (its `SHOW_REF`;
    // `duckdb-statement-in-query-position`). With `show_ref` off, the leading keyword
    // inside the FROM parens is neither a query start nor a joined table, so the
    // statement rejects — the flag is genuinely required.
    LabeledCase {
        sql: "SELECT column_name FROM (DESCRIBE SELECT 1) AS d",
        expect: Expect::Accept,
        required: &[&SHOW_REF],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT * FROM (SHOW databases) AS t",
        expect: Expect::Accept,
        required: &[&SHOW_REF],
        forbidden: &[],
    },
    // The SQL/JSON `JSON_TABLE` column-defining table factor
    // (`planner-parity-table-factor-json-table`). With `json_table` off, `JSON_TABLE(` falls
    // to the ordinary function-call path, which rejects at the `COLUMNS` clause — the flag is
    // genuinely required.
    LabeledCase {
        sql: "SELECT * FROM JSON_TABLE('[1,2]', '$[*]' COLUMNS (a int PATH '$', NESTED PATH '$.b' COLUMNS (c text))) AS t",
        expect: Expect::Accept,
        required: &[&JSON_TABLE],
        forbidden: &[],
    },
    // The SQL/XML `XMLTABLE` column-defining table factor (`pg-table-factor-xmltable`). With
    // `xml_table` off, `XMLTABLE(` falls to the ordinary function-call path, which rejects at
    // the unparenthesized `COLUMNS` clause — the flag is genuinely required.
    LabeledCase {
        sql: "SELECT * FROM XMLTABLE(XMLNAMESPACES('u' AS n), '/root' PASSING doc COLUMNS a int PATH 'x' NOT NULL, o FOR ORDINALITY) AS t",
        expect: Expect::Accept,
        required: &[&XML_TABLE],
        forbidden: &[],
    },
    // Snowflake/Oracle's `TABLE(<expr>)` first-class table-expression factor
    // (`planner-parity-table-factor-table-expr`). With `table_expr_factor` off, `TABLE(`
    // falls to the named-table path, where the reserved `TABLE` keyword is not an
    // admissible relation name — the flag is genuinely required.
    LabeledCase {
        sql: "SELECT * FROM TABLE(generate_series(1, 3))",
        expect: Expect::Accept,
        required: &[&TABLE_EXPR_FACTOR],
        forbidden: &[],
    },
    // DuckDB's bare `FROM VALUES (…) AS t` row-list table factor
    // (`duckdb-from-values-table-factor`). With `from_values` off, `VALUES` is not a
    // table name and the factor rejects — the flag is genuinely required. The alias
    // (`AS t`) is mandatory in the form itself, so the case carries no column list and
    // no other sub-flag governs it.
    LabeledCase {
        sql: "SELECT * FROM VALUES (1, 2) AS t",
        expect: Expect::Accept,
        required: &[&FROM_VALUES],
        forbidden: &[],
    },
    // MySQL row-locking clause: `FOR` is reserved everywhere, so with `locking_clauses`
    // off the trailing `FOR UPDATE` is leftover input -> reject; the flag is genuinely
    // required (an accept/reject fork).
    LabeledCase {
        sql: "SELECT a FROM t1 FOR UPDATE",
        expect: Expect::Accept,
        required: &[&LOCKING_CLAUSES],
        forbidden: &[],
    },
    // PostgreSQL `FOR NO KEY UPDATE` strength: with `key_lock_strengths` off the `NO`
    // after `FOR` is no strength lead, so the clause is a reject; on, it parses — an
    // accept/reject fork isolating the strength gate (engine-verified accept,
    // pg-locking-clause-strengths-and-stacking).
    LabeledCase {
        sql: "SELECT a FROM t1 FOR NO KEY UPDATE",
        expect: Expect::Accept,
        required: &[&KEY_LOCK_STRENGTHS],
        forbidden: &[],
    },
    // PostgreSQL stacked locking clauses: with `stacked_locking_clauses` off exactly one
    // clause parses and the trailing `FOR SHARE` is leftover input -> reject; on, the loop
    // consumes both — an accept/reject fork isolating the stacking gate.
    LabeledCase {
        sql: "SELECT a FROM t1 FOR UPDATE FOR SHARE",
        expect: Expect::Accept,
        required: &[&STACKED_LOCKING_CLAUSES],
        forbidden: &[],
    },
    // MySQL index hint: the hint keyword rides *after* the alias, so an explicit `AS x`
    // keeps `USE` out of alias-competition position (`USE`/`FORCE`/`IGNORE` are
    // unreserved outside MySQL). With `index_hints` off the trailing `USE INDEX (...)`
    // is leftover input -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "SELECT a FROM th AS x USE INDEX (idx_a)",
        expect: Expect::Accept,
        required: &[&INDEX_HINTS],
        forbidden: &[],
    },
    // MSSQL table hint: the `WITH (...)` hint tail rides after the alias; with
    // `table_hints` off the trailing `WITH (NOLOCK)` is leftover input (`WITH` stays
    // CTE-only, introduced only at statement start) -> reject, so the flag is genuinely
    // required.
    LabeledCase {
        sql: "SELECT a FROM th AS x WITH (NOLOCK)",
        expect: Expect::Accept,
        required: &[&TABLE_HINTS],
        forbidden: &[],
    },
    // MySQL partition selection: with `partition_selection` off, `PARTITION` is an
    // ordinary (non-reserved) word the table factor reads as its alias with a `(p0)`
    // derived-column list, so both *parse* — the flag's effect is structural (the
    // `partition` field populated vs an alias), not accept/reject, like `straight_join`.
    LabeledCase {
        sql: "SELECT a FROM tp PARTITION (p0)",
        expect: Expect::Shape(from_relation_has_partition_selection),
        required: &[&PARTITION_SELECTION],
        forbidden: &[],
    },
    // Multi-feature intersection: LATERAL together with the table-function form —
    // disabling either feature alone must reject, proving both are genuinely required.
    LabeledCase {
        sql: "SELECT * FROM LATERAL generate_series(1, 3) AS g(x)",
        expect: Expect::Accept,
        required: &[&LATERAL, &TABLE_FUNCTIONS],
        forbidden: &[],
    },
    // Base-table column-list alias (`mysql-preset-over-acceptance-residual`): on, `FROM t
    // AS y(a, b)` accepts; off (MySQL's base-vs-derived split) the `(` after the base-table
    // alias name is a syntax error, so the flag is genuinely required.
    LabeledCase {
        sql: "SELECT * FROM t AS y(a, b)",
        expect: Expect::Accept,
        required: &[&BASE_TABLE_ALIAS_COLUMN_LISTS],
        forbidden: &[],
    },
    // `UPDATE … FROM`: on, the additional-relations clause parses; off (MySQL) the `FROM`
    // keyword is leftover input -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "UPDATE t SET a = 1 FROM u",
        expect: Expect::Accept,
        required: &[&UPDATE_FROM],
        forbidden: &[],
    },
    // ALTER existence guard: with all three on, `DROP COLUMN IF EXISTS` parses; flipping
    // `alter_existence_guards` off (MySQL) makes `IF` a column name and the trailing
    // `EXISTS c` leftover input -> reject, and flipping either co-required flag (the shared
    // `IF EXISTS` guard, or the extended `ALTER` surface) likewise rejects, so each is
    // genuinely required. All three are on together only in PostgreSQL/DuckDB/Lenient.
    LabeledCase {
        sql: "ALTER TABLE t DROP COLUMN IF EXISTS c",
        expect: Expect::Accept,
        required: &[&ALTER_EXISTENCE_GUARDS, &IF_EXISTS, &ALTER_TABLE_EXTENDED],
        forbidden: &[],
    },
    // ALTER COLUMN type change: with both on, `SET DATA TYPE` parses; flipping
    // `alter_column_set_data_type` off (MySQL, which uses MODIFY/CHANGE) or the extended
    // `ALTER` surface off rejects, so each is genuinely required.
    LabeledCase {
        sql: "ALTER TABLE t ALTER COLUMN c SET DATA TYPE INT",
        expect: Expect::Accept,
        required: &[&ALTER_COLUMN_SET_DATA_TYPE, &ALTER_TABLE_EXTENDED],
        forbidden: &[],
    },
    // Deferrable constraint characteristic: on, `… DEFERRABLE` parses; off (MySQL) the
    // `DEFERRABLE` keyword is leftover input -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t (a INT REFERENCES b (id) DEFERRABLE)",
        expect: Expect::Accept,
        required: &[&DEFERRABLE_CONSTRAINTS],
        forbidden: &[],
    },
    // CTAS `WITH [NO] DATA`: on, the populate clause parses; off (MySQL) the trailing
    // `WITH` is leftover input -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t AS SELECT 1 WITH NO DATA",
        expect: Expect::Accept,
        required: &[&CREATE_TABLE_AS_WITH_DATA],
        forbidden: &[],
    },
    // Reserved-word `AS` projection alias: this *restrictive* flag (MySQL) routes the
    // position to the bare-alias reserved set. `FROM` sits in that set for both the
    // baseline (PostgreSQL, an `AS_LABEL`-only keyword) and MySQL (a reserved word), so on
    // `SELECT 1 AS from` *rejects*; off (the empty `reserved_as_label`) `FROM` is admitted
    // as the alias -> accept, an objective flip this flag alone drives (a reject case,
    // unlike the additive flags above).
    LabeledCase {
        sql: "SELECT 1 AS from",
        expect: Expect::Reject,
        required: &[&AS_ALIAS_REJECTS_RESERVED],
        forbidden: &[],
    },
    // Special value function as a `FROM` source (`mysql-preset-over-acceptance-residual`):
    // on, `FROM current_date` is PostgreSQL's `func_table` promotion; off (MySQL) the
    // special-function path is skipped and the reserved word `current_date` in table
    // position -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "SELECT * FROM current_date",
        expect: Expect::Accept,
        required: &[&SPECIAL_FUNCTION_TABLE_SOURCE],
        forbidden: &[],
    },
    // Alias on a parenthesized joined table: on, `(a CROSS JOIN b) AS x` accepts; off
    // (MySQL) the trailing alias on the join group is a syntax error, so the flag is
    // genuinely required.
    LabeledCase {
        sql: "SELECT * FROM (a CROSS JOIN b) AS x",
        expect: Expect::Accept,
        required: &[&ALIASED_PARENTHESIZED_JOIN],
        forbidden: &[],
    },
    // Alias on a `DELETE … USING` target: on, `DELETE FROM t AS e USING u …` accepts; off
    // (MySQL) an aliased target with `USING` present -> reject, so the flag is genuinely
    // required (co-requires `delete_using` to reach the `USING` clause at all).
    LabeledCase {
        sql: "DELETE FROM t AS e USING u WHERE e.a = 1",
        expect: Expect::Accept,
        required: &[&DELETE_USING_TARGET_ALIAS, &DELETE_USING],
        forbidden: &[],
    },
    // Leading `WITH` before `INSERT`: on, `WITH a AS (…) INSERT …` accepts; off (MySQL) the
    // `INSERT` after the CTE list -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "WITH a AS (SELECT 1) INSERT INTO b SELECT * FROM a",
        expect: Expect::Accept,
        required: &[&CTE_BEFORE_INSERT],
        forbidden: &[],
    },
    // Leading `WITH` before `MERGE`: on (PostgreSQL 15+/DuckDB), `WITH a AS (…) MERGE …`
    // accepts; off (ANSI — SQL:2016's merge statement takes no WITH clause) -> reject.
    // Co-requires `merge` to reach the dispatch arm at all.
    LabeledCase {
        sql: "WITH a AS (SELECT 1) MERGE INTO t USING a ON true WHEN MATCHED THEN DELETE",
        expect: Expect::Accept,
        required: &[&CTE_BEFORE_MERGE, &MERGE],
        forbidden: &[],
    },
    // Data-modifying CTE body: on (PostgreSQL), `WITH t AS (INSERT …) SELECT …` accepts;
    // off (DuckDB/SQLite/MySQL/ANSI — a CTE body must be a query) the `INSERT` after
    // `AS (` -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "WITH t AS (INSERT INTO x VALUES (1)) SELECT * FROM t",
        expect: Expect::Accept,
        required: &[&DATA_MODIFYING_CTES],
        forbidden: &[],
    },
    // A MERGE CTE body (PG 17) needs both gates: the CTE-body widening AND the merge
    // statement itself — flipping either off must reject.
    LabeledCase {
        sql: "WITH t AS (MERGE INTO x USING y ON true WHEN MATCHED THEN DELETE) SELECT * FROM t",
        expect: Expect::Accept,
        required: &[&DATA_MODIFYING_CTES, &MERGE],
        forbidden: &[],
    },
    // The SQL:2023 recursive-query SEARCH/CYCLE clauses on a CTE: on (PostgreSQL/Lenient)
    // the `SEARCH … CYCLE …` tail after the body's `)` accepts; off (ANSI/DuckDB/MySQL —
    // DuckDB parse-rejects `SEARCH`, probed on 1.5.4) the trailing keyword is left
    // unconsumed and rejects, so the flag is genuinely required.
    LabeledCase {
        sql: "WITH RECURSIVE t(n) AS (SELECT 1) SEARCH DEPTH FIRST BY n SET seq \
              CYCLE n SET c TO true DEFAULT false USING p SELECT * FROM t",
        expect: Expect::Accept,
        required: &[&RECURSIVE_SEARCH_CYCLE],
        forbidden: &[],
    },
    // `MERGE … RETURNING` (PG 17/DuckDB) rides the shared `returning` gate on top of
    // `merge`: flipping `returning` off leaves the RETURNING tail unconsumed -> reject.
    LabeledCase {
        sql: "MERGE INTO t USING u ON true WHEN MATCHED THEN DELETE RETURNING *",
        expect: Expect::Accept,
        required: &[&MERGE, &RETURNING],
        forbidden: &[],
    },
    // `WHEN NOT MATCHED BY SOURCE` (PG 17/DuckDB, both probed): on, the unpaired-target
    // arm accepts; off (ANSI — SQL:2016 has only MATCHED/NOT MATCHED) the `BY` after
    // `NOT MATCHED` is leftover input -> reject. Co-requires `merge` for dispatch.
    LabeledCase {
        sql: "MERGE INTO t USING u ON true WHEN NOT MATCHED BY SOURCE THEN DELETE",
        expect: Expect::Accept,
        required: &[&MERGE, &MERGE_WHEN_NOT_MATCHED_BY],
        forbidden: &[],
    },
    // The `BY TARGET` spelling rides the same gate (one production with the bare
    // `NOT MATCHED`, so one flag governs both qualifier spellings).
    LabeledCase {
        sql: "MERGE INTO t USING u ON true WHEN NOT MATCHED BY TARGET THEN INSERT VALUES (1)",
        expect: Expect::Accept,
        required: &[&MERGE, &MERGE_WHEN_NOT_MATCHED_BY],
        forbidden: &[],
    },
    // The merge `INSERT DEFAULT VALUES` action (PG/DuckDB, both probed): on, it accepts;
    // off (ANSI — the standard merge insert has no DEFAULT VALUES alternative) the
    // `DEFAULT` after `INSERT` -> reject. Co-requires `merge` for dispatch.
    LabeledCase {
        sql: "MERGE INTO t USING u ON true WHEN NOT MATCHED THEN INSERT DEFAULT VALUES",
        expect: Expect::Accept,
        required: &[&MERGE, &MERGE_INSERT_DEFAULT_VALUES],
        forbidden: &[],
    },
    // The merge insert `OVERRIDING {SYSTEM|USER} VALUE` override (SQL:2016 standard —
    // on in ANSI/PG; DuckDB parse-rejects it inside MERGE, probed on 1.5.4, so its
    // preset leaves it off): off, the `OVERRIDING` before `VALUES` is leftover input
    // -> reject. Co-requires `merge` for dispatch.
    LabeledCase {
        sql: "MERGE INTO t USING u ON true WHEN NOT MATCHED THEN INSERT (a) OVERRIDING USER VALUE VALUES (1)",
        expect: Expect::Accept,
        required: &[&MERGE, &MERGE_INSERT_OVERRIDING],
        forbidden: &[],
    },
    // `MERGE INTO ONLY t` (PG/DuckDB, both probed) reuses the shared `DmlTarget`
    // relation shape, so the inheritance marker rides the same `only` gate as
    // `UPDATE`/`DELETE`/`FROM`: flipping `only` off must reject it.
    LabeledCase {
        sql: "MERGE INTO ONLY t USING u ON true WHEN MATCHED THEN DELETE",
        expect: Expect::Accept,
        required: &[&MERGE, &ONLY],
        forbidden: &[],
    },
    // `CONSTRAINT <name>` on a non-CHECK inline column constraint: on, `CONSTRAINT c
    // REFERENCES b` accepts; off (MySQL, which admits a named inline constraint only for
    // CHECK) -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE TABLE k (s INT CONSTRAINT c REFERENCES b)",
        expect: Expect::Accept,
        required: &[&NAMED_INLINE_NON_CHECK_CONSTRAINTS],
        forbidden: &[],
    },
    // A trailing bodyless `CONSTRAINT <name>` (SQLite, engine-measured): on, the name parses
    // with no constraint element after it; off (every other preset, which require a
    // constraint element after the name) the missing element -> reject, so the flag is
    // genuinely required.
    LabeledCase {
        sql: "CREATE TABLE k (s INT CONSTRAINT c)",
        expect: Expect::Accept,
        required: &[&BARE_CONSTRAINT_NAME],
        forbidden: &[],
    },
    // Declarative partitioning: on (PostgreSQL), the `PARTITION BY` clause parses; off (every
    // other preset, none of which spell the PostgreSQL declarative form) the `PARTITION`
    // keyword is leftover input -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t (a INT) PARTITION BY LIST (a)",
        expect: Expect::Accept,
        required: &[&DECLARATIVE_PARTITIONING],
        forbidden: &[],
    },
    // Legacy table inheritance: on (PostgreSQL), the `INHERITS (parent)` clause parses; off (every
    // other preset, none of which have table inheritance) the `INHERITS` keyword is leftover input
    // -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t (a INT) INHERITS (p)",
        expect: Expect::Accept,
        required: &[&TABLE_INHERITANCE],
        forbidden: &[],
    },
    // LIKE source-table element: on (PostgreSQL), the `(LIKE src INCLUDING ALL)` element parses;
    // off (every other preset) the `INCLUDING` tail is leftover input -> reject, so the flag is
    // genuinely required. The `INCLUDING ALL` tail (over a bare `LIKE src`) keeps the case a clean
    // reject even on SQLite, whose lax identifier rules would otherwise read `LIKE src` as a
    // keyword-named column.
    LabeledCase {
        sql: "CREATE TABLE t (LIKE src INCLUDING ALL)",
        expect: Expect::Accept,
        required: &[&LIKE_SOURCE_TABLE],
        forbidden: &[],
    },
    // MySQL's statement-level table clone: on (MySQL/Lenient), the bare `CREATE TABLE t LIKE src`
    // body parses; off, `LIKE` after the table name is leftover input -> reject, so the flag is
    // genuinely required. The BARE spelling (no parentheses) isolates this from the PostgreSQL
    // copy *element* (`like_source_table`), which requires the parenthesized `(LIKE …)` form and
    // so never accepts the bare statement — the flip stays a clean reject even where that flag is
    // on.
    LabeledCase {
        sql: "CREATE TABLE t LIKE src",
        expect: Expect::Accept,
        required: &[&STATEMENT_LEVEL_TABLE_LIKE],
        forbidden: &[],
    },
    // Column-definition COLLATE: on (PostgreSQL/SQLite/DuckDB), the collation clause parses;
    // off (ANSI/MySQL) the `COLLATE` keyword is leftover input -> reject, so the flag is
    // genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t (a TEXT COLLATE \"C\")",
        expect: Expect::Accept,
        required: &[&COLUMN_COLLATION],
        forbidden: &[],
    },
    // UNLOGGED persistence: on (PostgreSQL/DuckDB), the keyword parses in the `OptTemp` slot;
    // off, `UNLOGGED` after `CREATE` is leftover input (an unknown statement) -> reject, so the
    // flag is genuinely required.
    LabeledCase {
        sql: "CREATE UNLOGGED TABLE t (a INT)",
        expect: Expect::Accept,
        required: &[&UNLOGGED_TABLES],
        forbidden: &[],
    },
    // Column STORAGE + COMPRESSION (one flag gates both): on (PostgreSQL), the fixed-position
    // physical-storage attributes parse between the type and the constraints; off, `STORAGE`
    // reaches the constraint loop -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t (a TEXT STORAGE MAIN COMPRESSION lz4)",
        expect: Expect::Accept,
        required: &[&COLUMN_STORAGE],
        forbidden: &[],
    },
    // Table USING access method: on (PostgreSQL), the trailing `USING <method>` parses; off,
    // the `USING` keyword is leftover input -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t (a INT) USING heap",
        expect: Expect::Accept,
        required: &[&TABLE_ACCESS_METHOD],
        forbidden: &[],
    },
    // Legacy WITHOUT OIDS no-op: on (PostgreSQL), the option parses (and is preserved); off,
    // `WITHOUT` is leftover input -> reject (SQLite's `WITHOUT ROWID` is a different second
    // word under a different gate), so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t (a INT) WITHOUT OIDS",
        expect: Expect::Accept,
        required: &[&WITHOUT_OIDS],
        forbidden: &[],
    },
    // Typed (OF) tables: on (PostgreSQL), the `OF <type>` body with its typeless augmentation
    // list parses; off, `OF` after the table name is leftover input -> reject, so the flag is
    // genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t OF ty (a WITH OPTIONS NOT NULL)",
        expect: Expect::Accept,
        required: &[&TYPED_TABLES],
        forbidden: &[],
    },
    // Exclusion constraint: on (PostgreSQL), the `EXCLUDE USING <method> (elem WITH op)` table
    // element parses; off, `EXCLUDE` at a constraint position is leftover input -> reject, so the
    // flag is genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t (a INT, EXCLUDE USING gist (a WITH =))",
        expect: Expect::Accept,
        required: &[&EXCLUSION_CONSTRAINTS],
        forbidden: &[],
    },
    // CTAS `AS EXECUTE`: on (PostgreSQL), `CREATE TABLE t AS EXECUTE p` parses; off, the
    // inline-query CTAS path rejects the `EXECUTE` keyword, so the flag alone drives the flip.
    LabeledCase {
        sql: "CREATE TABLE t AS EXECUTE p",
        expect: Expect::Accept,
        required: &[&CREATE_TABLE_AS_EXECUTE],
        forbidden: &[],
    },
    // The `NO INHERIT` / `NOT VALID` constraint markers: on (PostgreSQL/DuckDB), a table `CHECK
    // (…) NO INHERIT` parses; off, the `NO INHERIT` after the constraint is leftover input ->
    // reject, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t (a INT, CHECK (a > 0) NO INHERIT)",
        expect: Expect::Accept,
        required: &[&CONSTRAINT_NO_INHERIT_NOT_VALID],
        forbidden: &[],
    },
    // Index-constraint parameters: on (PostgreSQL), `PRIMARY KEY (a) INCLUDE (b)` parses; off,
    // `INCLUDE` after the key list is leftover input -> reject, so the flag alone drives the flip.
    LabeledCase {
        sql: "CREATE TABLE t (a INT, b INT, PRIMARY KEY (a) INCLUDE (b))",
        expect: Expect::Accept,
        required: &[&INDEX_CONSTRAINT_PARAMETERS],
        forbidden: &[],
    },
    // Constraint-column COLLATE/order: on (SQLite), `PRIMARY KEY (a COLLATE nocase)` parses;
    // off, the `COLLATE` after the key name is leftover input -> reject, so the flag alone
    // drives the flip (`constraint-column-list-collate-ordering`).
    LabeledCase {
        sql: "CREATE TABLE t (a INT, b INT, PRIMARY KEY (a COLLATE nocase))",
        expect: Expect::Accept,
        required: &[&CONSTRAINT_COLUMN_COLLATE_ORDER],
        forbidden: &[],
    },
    // Zoned temporal type: on, `TIMESTAMPTZ` is a recognised type; off (MySQL, which has no
    // zoned temporal type) the time-zone-qualified type -> reject, so the flag is genuinely
    // required.
    LabeledCase {
        sql: "CREATE TABLE t (a TIMESTAMPTZ)",
        expect: Expect::Accept,
        required: &[&ZONED_TEMPORAL_TYPES],
        forbidden: &[],
    },
    // Empty `DECIMAL()` parens: on (DuckDb/Lenient) the default-precision form is a
    // recognised type; off (the standard, which requires a precision inside the parens) the
    // empty `()` -> reject, so the flag alone drives the flip (`duckdb-empty-type-parens`).
    LabeledCase {
        sql: "SELECT CAST(NULL AS DECIMAL())",
        expect: Expect::Accept,
        required: &[&EMPTY_TYPE_PARENS],
        forbidden: &[],
    },
    // MySQL's character-set type annotation: on (MySQL/Lenient), `CHAR CHARACTER SET
    // utf8mb4` carries the charset on the type node; off (the standard/PostgreSQL, which
    // reject `CHARACTER SET` — `pg_query`-verified) the trailing `CHARACTER` is not a
    // column option, so the column definition -> reject. The flag alone drives the flip.
    // Signed numeric type modifier: on (PostgreSQL/Lenient), `NUMERIC(3, -6)` parses (a
    // raw-parse laxity); off (the standard/MySQL/SQLite/DuckDB, which require an unsigned
    // modifier) the leading `-` -> reject, so the flag alone drives the flip.
    LabeledCase {
        sql: "CREATE TABLE t (a NUMERIC(3, -6))",
        expect: Expect::Accept,
        required: &[&SIGNED_TYPE_MODIFIER],
        forbidden: &[],
    },
    // DuckDB's string-literal type modifier on a user-defined type name: on (DuckDB),
    // `'x'::GEOMETRY('OGC:CRS84')` parses (the coordinate-system annotation rides the
    // `UserDefined` modifier list as a string `Literal`); off (the standard/PostgreSQL/
    // MySQL/SQLite, which admit only unsigned-integer modifiers) the string modifier ->
    // reject, so the flag alone drives the flip.
    LabeledCase {
        sql: "SELECT 'x'::GEOMETRY('OGC:CRS84')",
        expect: Expect::Accept,
        required: &[&STRING_TYPE_MODIFIERS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "CREATE TABLE t (a CHAR CHARACTER SET utf8mb4)",
        expect: Expect::Accept,
        required: &[&CHARACTER_SET_ANNOTATION],
        forbidden: &[],
    },
    // `VARCHAR` length requirement: this *restrictive* flag (MySQL) rejects a length-less
    // `VARCHAR`; on `CREATE TABLE t (a VARCHAR)` *rejects*, off (the standard, unlimited
    // `VARCHAR`) accepts — an objective flip this flag alone drives (a reject case).
    LabeledCase {
        sql: "CREATE TABLE t (a VARCHAR)",
        expect: Expect::Reject,
        required: &[&VARCHAR_REQUIRES_LENGTH],
        forbidden: &[],
    },
    // Parenthesized functional default: this *restrictive* flag (MySQL) requires
    // `DEFAULT (expr)` for a function call; on `CREATE TABLE t (a INT DEFAULT UUID())`
    // *rejects*, off (a bare expression default) accepts — an objective flip (a reject case).
    LabeledCase {
        sql: "CREATE TABLE t (a INT DEFAULT UUID())",
        expect: Expect::Reject,
        required: &[&DEFAULT_EXPRESSION_REQUIRES_PARENS],
        forbidden: &[],
    },
    // RETURNING / ON CONFLICT: with the flag off the keyword is left unconsumed and
    // the trailing clause is leftover input -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "INSERT INTO t VALUES (1) RETURNING id",
        expect: Expect::Accept,
        required: &[&RETURNING],
        forbidden: &[],
    },
    LabeledCase {
        sql: "INSERT INTO t VALUES (1) ON CONFLICT DO NOTHING",
        expect: Expect::Accept,
        required: &[&ON_CONFLICT],
        forbidden: &[],
    },
    // MySQL upsert: with the flag off the keyword after `ON` is neither `CONFLICT`
    // (the baseline's `ON CONFLICT`) nor an enabled `DUPLICATE`, so `ON` is left
    // unconsumed and the trailing clause is leftover input -> reject.
    LabeledCase {
        sql: "INSERT INTO t VALUES (1) ON DUPLICATE KEY UPDATE a = 1",
        expect: Expect::Accept,
        required: &[&ON_DUPLICATE_KEY_UPDATE],
        forbidden: &[],
    },
    // MySQL single-table `UPDATE`/`DELETE ... ORDER BY ... LIMIT` tails: with the flag
    // off the trailing `ORDER BY a LIMIT 1` is left unconsumed after the WHERE-less
    // statement body and surfaces as leftover input -> reject, so the flag is required.
    LabeledCase {
        sql: "UPDATE t SET a = 1 ORDER BY a LIMIT 1",
        expect: Expect::Accept,
        required: &[&UPDATE_DELETE_TAILS],
        forbidden: &[],
    },
    // SQLite `INSERT OR <action>` / `UPDATE OR <action>` conflict-resolution prefix: with
    // the flag off the `OR` after the verb is left unconsumed — on `INSERT` the following
    // `INTO` fails to match, on `UPDATE` the reserved `OR` cannot be the target name — so
    // each rejects, and the flag is genuinely required.
    LabeledCase {
        sql: "INSERT OR IGNORE INTO t VALUES (1)",
        expect: Expect::Accept,
        required: &[&OR_CONFLICT_ACTION],
        forbidden: &[],
    },
    LabeledCase {
        sql: "UPDATE OR ROLLBACK t SET a = 1",
        expect: Expect::Accept,
        required: &[&OR_CONFLICT_ACTION],
        forbidden: &[],
    },
    // Schema-change guards: with `if_exists` off the unconsumed `IF` leaves
    // `EXISTS t` as leftover input -> reject; with `drop_behavior` off the trailing
    // `CASCADE` is leftover -> reject. So each flag is genuinely required.
    LabeledCase {
        sql: "DROP TABLE IF EXISTS t",
        expect: Expect::Accept,
        required: &[&IF_EXISTS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "DROP TABLE t CASCADE",
        expect: Expect::Accept,
        required: &[&DROP_BEHAVIOR],
        forbidden: &[],
    },
    // `CREATE DATABASE IF NOT EXISTS`: with the flag off the guard is not admitted, so
    // `IF` is read as the database name and the trailing `NOT EXISTS d` is leftover
    // input -> reject. PostgreSQL has no such guard, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE DATABASE IF NOT EXISTS d",
        expect: Expect::Accept,
        required: &[&CREATE_DATABASE_IF_NOT_EXISTS],
        forbidden: &[],
    },
    // Keywordless generated-column shorthand `AS (<expr>)`: with the flag off the `AS`
    // after the column type is not a recognised column option, so it is leftover input
    // in the table-element list -> reject. PostgreSQL requires `GENERATED ALWAYS AS`.
    LabeledCase {
        sql: "CREATE TABLE t (a INT AS (1))",
        expect: Expect::Accept,
        required: &[&GENERATED_COLUMN_SHORTHAND],
        forbidden: &[],
    },
    // Multi-column SET: with the flag off the leading `(` cannot open a tuple
    // assignment, so the single-assignment path rejects the parenthesis.
    LabeledCase {
        sql: "UPDATE t SET (a, b) = (1, 2)",
        expect: Expect::Accept,
        required: &[&MULTI_COLUMN_ASSIGNMENT],
        forbidden: &[],
    },
    // WHERE CURRENT OF: with the flag off `CURRENT` is parsed as an ordinary
    // expression and the trailing `OF c` is leftover input -> reject.
    LabeledCase {
        sql: "UPDATE t SET a = 1 WHERE CURRENT OF c",
        expect: Expect::Accept,
        required: &[&WHERE_CURRENT_OF],
        forbidden: &[],
    },
    // MERGE (SQL:2003): the standard upsert, on in ANSI/PostgreSQL and off in MySQL.
    // With `merge` off the leading `MERGE` keyword is not dispatched and surfaces as
    // an unknown statement -> reject, so the flag is genuinely required. The case
    // exercises both a MATCHED (UPDATE) and a NOT MATCHED (INSERT) arm with an
    // AND-predicate, the full round-trip shape the ticket calls for.
    LabeledCase {
        sql: "MERGE INTO t USING s ON t.id = s.id \
              WHEN MATCHED AND s.flag THEN UPDATE SET a = s.a \
              WHEN NOT MATCHED THEN INSERT (id, a) VALUES (s.id, s.a)",
        expect: Expect::Accept,
        required: &[&MERGE],
        forbidden: &[],
    },
    // MySQL `REPLACE` (delete-then-insert): a leading keyword gated like `MERGE`. With
    // `replace_into` off the keyword is not dispatched and surfaces as an unknown
    // statement -> reject. The `VALUES` and query sources need only `replace_into`.
    LabeledCase {
        sql: "REPLACE INTO t VALUES (1)",
        expect: Expect::Accept,
        required: &[&REPLACE_INTO],
        forbidden: &[],
    },
    LabeledCase {
        sql: "REPLACE INTO t SELECT 1",
        expect: Expect::Accept,
        required: &[&REPLACE_INTO],
        forbidden: &[],
    },
    // The `INSERT`/`REPLACE ... SET <col> = <value>` assignment-list source, gated by
    // `insert_set`. The `REPLACE ... SET` form needs both flags (the statement keyword
    // and the source); the `INSERT ... SET` form isolates `insert_set` so flipping it
    // off (leaving `replace_into` aside) genuinely flips the outcome.
    LabeledCase {
        sql: "REPLACE INTO t SET a = 1",
        expect: Expect::Accept,
        required: &[&REPLACE_INTO, &INSERT_SET],
        forbidden: &[],
    },
    LabeledCase {
        sql: "INSERT INTO t SET a = 1",
        expect: Expect::Accept,
        required: &[&INSERT_SET],
        forbidden: &[],
    },
    // COPY (PostgreSQL utility statement): a leading keyword gated like `MERGE` /
    // `REPLACE`. With `copy` off (ANSI/MySQL) the keyword is not dispatched and
    // surfaces as an unknown statement -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "COPY t TO STDOUT",
        expect: Expect::Accept,
        required: &[&COPY],
        forbidden: &[],
    },
    // COPY INTO (Snowflake bulk load/unload): its own leading-surface flag distinct from
    // `copy`, dispatched on the `INTO` after `COPY`. The baseline `POSTGRES` keeps `copy`
    // on, so flipping `copy_into` off still leaves the PostgreSQL `COPY` parser, which
    // rejects the `INTO` (`INTO` is not a table name) -> reject, so the flag is genuinely
    // required (the falsely-required flip holds even with `copy` on).
    LabeledCase {
        sql: "COPY INTO t FROM 's3://bucket/data/'",
        expect: Expect::Accept,
        required: &[&COPY_INTO],
        forbidden: &[],
    },
    // COMMENT ON (PostgreSQL utility statement): a leading keyword gated like `COPY`.
    // With `comment_on` off (ANSI/MySQL) the keyword is not dispatched and surfaces as
    // an unknown statement -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "COMMENT ON TABLE t IS 'note'",
        expect: Expect::Accept,
        required: &[&COMMENT_ON],
        forbidden: &[],
    },
    // PRAGMA (SQLite configuration statement): a leading keyword gated like `COPY`.
    // With `pragma` off (ANSI/PostgreSQL/MySQL) the keyword is not dispatched and
    // surfaces as an unknown statement -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "PRAGMA foreign_keys = ON",
        expect: Expect::Accept,
        required: &[&PRAGMA],
        forbidden: &[],
    },
    // ATTACH/DETACH (SQLite): one `attach` flag gates the pair as a single dialect
    // unit, so each leading keyword is a labelled case requiring the same toggle.
    LabeledCase {
        sql: "ATTACH DATABASE ':memory:' AS aux",
        expect: Expect::Accept,
        required: &[&ATTACH],
        forbidden: &[],
    },
    LabeledCase {
        sql: "DETACH DATABASE aux",
        expect: Expect::Accept,
        required: &[&ATTACH],
        forbidden: &[],
    },
    // DETACH's DuckDB `IF EXISTS` guard: `detach_if_exists` on top of the `attach`
    // `DETACH` statement. With `attach` off the keyword is undispatched; with
    // `detach_if_exists` off the guard rejects (`IF` is read as the schema name and
    // `EXISTS aux` is a stray token), so both are required.
    LabeledCase {
        sql: "DETACH DATABASE IF EXISTS aux",
        expect: Expect::Accept,
        required: &[&ATTACH, &DETACH_IF_EXISTS],
        forbidden: &[],
    },
    // CHECKPOINT (PostgreSQL/DuckDB): the bare form is gated by `checkpoint`; with it off
    // the leading keyword surfaces as an unknown statement -> reject.
    LabeledCase {
        sql: "CHECKPOINT",
        expect: Expect::Accept,
        required: &[&CHECKPOINT],
        forbidden: &[],
    },
    // DuckDB's `FORCE CHECKPOINT` operands: `checkpoint_database` on top of the base
    // `checkpoint` statement. With `checkpoint` off nothing dispatches; with
    // `checkpoint_database` off the `FORCE`-led form is not dispatched -> reject, so both
    // are required.
    LabeledCase {
        sql: "FORCE CHECKPOINT",
        expect: Expect::Accept,
        required: &[&CHECKPOINT, &CHECKPOINT_DATABASE],
        forbidden: &[],
    },
    // LOAD (PostgreSQL/DuckDB): the string-argument form is gated by `load_extension`;
    // off elsewhere the leading `LOAD` surfaces as an unknown statement -> reject.
    LabeledCase {
        sql: "LOAD 'ext'",
        expect: Expect::Accept,
        required: &[&LOAD_EXTENSION],
        forbidden: &[],
    },
    // DuckDB's bare-identifier `LOAD` argument: `load_bare_name` on top of the base
    // `load_extension` statement. With `load_extension` off nothing dispatches; with
    // `load_bare_name` off the bare name rejects (only a string is admitted), so both are
    // required.
    LabeledCase {
        sql: "LOAD myext",
        expect: Expect::Accept,
        required: &[&LOAD_EXTENSION, &LOAD_BARE_NAME],
        forbidden: &[],
    },
    // DuckDB's `RESET SESSION <name>` scope prefix: `reset_scope` on top of the base
    // `session_statements` `RESET`. With it off the scope keyword is read as the parameter
    // name and the trailing name is a stray token -> reject, so the flag is required.
    LabeledCase {
        sql: "RESET SESSION myvar",
        expect: Expect::Accept,
        required: &[&RESET_SCOPE],
        forbidden: &[],
    },
    // VACUUM / REINDEX / ANALYZE (SQLite maintenance statements): each leading keyword
    // is gated by its own flag (independent statements, not an inverse pair like
    // ATTACH/DETACH), so each is a labelled case requiring only its own toggle.
    LabeledCase {
        sql: "VACUUM main INTO 'backup.db'",
        expect: Expect::Accept,
        required: &[&VACUUM],
        forbidden: &[],
    },
    LabeledCase {
        sql: "REINDEX main.t",
        expect: Expect::Accept,
        required: &[&REINDEX],
        forbidden: &[],
    },
    LabeledCase {
        sql: "ANALYZE sqlite_master",
        expect: Expect::Accept,
        required: &[&ANALYZE],
        forbidden: &[],
    },
    // DuckDB's `VACUUM [ANALYZE] [<table>]` statement: a separate leading-`VACUUM` base
    // gate from SQLite's `INTO`-shaped `vacuum` (both off in the POSTGRES baseline). With
    // `vacuum_analyze` off the leading `VACUUM` is not dispatched -> reject, so only its
    // own toggle is required.
    LabeledCase {
        sql: "VACUUM ANALYZE",
        expect: Expect::Accept,
        required: &[&VACUUM_ANALYZE],
        forbidden: &[],
    },
    // DuckDB's `ANALYZE <table> (<cols>)` column list: `analyze_columns` on top of the
    // base `analyze` statement. With `analyze` off nothing dispatches; with
    // `analyze_columns` off the trailing `(` is a stray token after `ANALYZE t1` ->
    // reject, so both are required (the `checkpoint_database` precedent).
    LabeledCase {
        sql: "ANALYZE t1 (a, b)",
        expect: Expect::Accept,
        required: &[&ANALYZE, &ANALYZE_COLUMNS],
        forbidden: &[],
    },
    // Table maintenance (MySQL admin-table verbs): one gate for the five-verb family. A
    // clean accept/reject flip on a verb with no sibling gate — with `table_maintenance`
    // off the leading `OPTIMIZE` is not dispatched and surfaces as an unknown statement.
    LabeledCase {
        sql: "OPTIMIZE TABLE t1",
        expect: Expect::Accept,
        required: &[&TABLE_MAINTENANCE],
        forbidden: &[],
    },
    // Standalone RENAME (MySQL): a leading keyword gated like `kill`. With `rename_statement`
    // off the leading `RENAME` is not dispatched and surfaces as an unknown statement.
    LabeledCase {
        sql: "RENAME TABLE a TO b",
        expect: Expect::Accept,
        required: &[&RENAME_STATEMENT],
        forbidden: &[],
    },
    // FLUSH (MySQL): a leading keyword gated like `kill`. With `flush` off the leading
    // `FLUSH` is not dispatched and surfaces as an unknown statement -> reject.
    LabeledCase {
        sql: "FLUSH PRIVILEGES",
        expect: Expect::Accept,
        required: &[&FLUSH],
        forbidden: &[],
    },
    // PURGE BINARY LOGS (MySQL): a leading keyword gated like `kill`. With
    // `purge_binary_logs` off the leading `PURGE` is not dispatched and surfaces as an
    // unknown statement -> reject.
    LabeledCase {
        sql: "PURGE BINARY LOGS TO 'log.000001'",
        expect: Expect::Accept,
        required: &[&PURGE_BINARY_LOGS],
        forbidden: &[],
    },
    // MySQL replication administration (`replication_statements`): one gate for the five
    // families. `START REPLICA` refines the leading `START` — with the flag off it is left to
    // the transaction dispatcher, which requires `TRANSACTION` after `START` and rejects
    // `REPLICA`, so the flag is required to accept.
    LabeledCase {
        sql: "START REPLICA",
        expect: Expect::Accept,
        required: &[&REPLICATION_STATEMENTS],
        forbidden: &[],
    },
    // MySQL account-management DDL (`CREATE`/`ALTER`/`DROP USER`, `CREATE`/`DROP ROLE`): one gate
    // for the five-family axis. With `user_role_management` off the `USER` after `CREATE` is not
    // dispatched and falls through to the `TABLE` expectation -> reject, so the flag is required.
    // A bare account (no `@host`) keeps the probe independent of the `@`-lexing user-variable
    // surface, so the gate is exercised in isolation.
    LabeledCase {
        sql: "CREATE USER u",
        expect: Expect::Accept,
        required: &[&USER_ROLE_MANAGEMENT],
        forbidden: &[],
    },
    // MySQL account-based GRANT/REVOKE (`access_control_account_grants`): the flag routes
    // GRANT/REVOKE to the MySQL grammar, whose `*.*` priv_level is a syntax error under the base
    // access-control grammar the flag-off path falls to, so flipping it flips accept<->reject. Its
    // registered grammar rival `access_control_extended_objects`
    // (`GrammarConflict::AccountGrantsVersusExtendedObjects`) is *forbidden* so the baseline stays
    // self-consistent — the two cannot coexist — and turning that rival on is a genuine forbidden
    // flip the registry adjudicates (the both-on set has no defined parse).
    LabeledCase {
        sql: "GRANT SELECT ON *.* TO u",
        expect: Expect::Accept,
        required: &[&ACCESS_CONTROL_ACCOUNT_GRANTS],
        forbidden: &[&ACCESS_CONTROL_EXTENDED_OBJECTS],
    },
    // KILL (MySQL): a leading keyword gated like `copy`. With `kill` off the keyword is not
    // dispatched and surfaces as an unknown statement -> reject, so the flag is required.
    LabeledCase {
        sql: "KILL CONNECTION 5",
        expect: Expect::Accept,
        required: &[&KILL],
        forbidden: &[],
    },
    // HANDLER (MySQL): a leading keyword gated like `kill`. With `handler_statements` off the
    // keyword is not dispatched and surfaces as an unknown statement -> reject, so the flag is
    // required. The index key-seek form exercises the value list too.
    LabeledCase {
        sql: "HANDLER t READ idx = (1)",
        expect: Expect::Accept,
        required: &[&HANDLER_STATEMENTS],
        forbidden: &[],
    },
    // INSTALL/UNINSTALL PLUGIN/COMPONENT (MySQL): one flag gates both leading keywords, an
    // install/uninstall pair like `attach`/`detach`. With `plugin_component_statements` off
    // the keywords are not dispatched and surface as unknown statements -> reject, so the
    // flag is required. The component form exercises the URN list plus the scoped SET tail.
    LabeledCase {
        sql: "INSTALL COMPONENT 'file://c', 'file://d' SET GLOBAL v = 1",
        expect: Expect::Accept,
        required: &[&PLUGIN_COMPONENT_STATEMENTS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "UNINSTALL PLUGIN p",
        expect: Expect::Accept,
        required: &[&PLUGIN_COMPONENT_STATEMENTS],
        forbidden: &[],
    },
    // MySQL server-administration families (parse-mysql-server-admin): each is a leading
    // keyword gated like `kill` — with the flag off the keyword is not dispatched and surfaces
    // as an unknown statement -> reject, so the flag is required. IMPORT TABLE additionally
    // proves the second-keyword split from DuckDB's `IMPORT DATABASE`.
    LabeledCase {
        sql: "SHUTDOWN",
        expect: Expect::Accept,
        required: &[&SHUTDOWN],
        forbidden: &[],
    },
    LabeledCase {
        sql: "RESTART",
        expect: Expect::Accept,
        required: &[&RESTART],
        forbidden: &[],
    },
    LabeledCase {
        sql: "CLONE LOCAL DATA DIRECTORY 'd'",
        expect: Expect::Accept,
        required: &[&CLONE],
        forbidden: &[],
    },
    LabeledCase {
        sql: "IMPORT TABLE FROM 'f', 'g'",
        expect: Expect::Accept,
        required: &[&IMPORT_TABLE],
        forbidden: &[],
    },
    LabeledCase {
        sql: "HELP 'contents'",
        expect: Expect::Accept,
        required: &[&HELP_STATEMENT],
        forbidden: &[],
    },
    LabeledCase {
        sql: "BINLOG 'YWJj'",
        expect: Expect::Accept,
        required: &[&BINLOG],
        forbidden: &[],
    },
    // CACHE INDEX (MySQL): the key-cache assignment gated like `kill`. With
    // `key_cache_statements` off the leading `CACHE` keyword is not dispatched and surfaces as
    // an unknown statement -> reject, so the flag is required. (The `LOAD INDEX INTO CACHE`
    // preload sibling shares the same gate.)
    LabeledCase {
        sql: "CACHE INDEX t1 IN c",
        expect: Expect::Accept,
        required: &[&KEY_CACHE_STATEMENTS],
        forbidden: &[],
    },
    // The statement-head / leading-keyword gates, each a flip-verified toggle.
    // Every probe accepts under its flag and rejects with it off (the leading keyword falls
    // through to the `CREATE`/`ALTER`/`DROP TABLE`/unknown-statement expectation), so the flag
    // alone flips accept<->reject. The probes are kept minimal because the labeled sweep runs
    // every case against every preset.
    //
    // MySQL `LOAD DATA` — the two-word `LOAD DATA` lookahead splits it from `load_extension`.
    LabeledCase {
        sql: "LOAD DATA INFILE 'f' INTO TABLE t",
        expect: Expect::Accept,
        required: &[&LOAD_DATA],
        forbidden: &[],
    },
    // MySQL `DO <expr-list>` — a behaviour split on the `DO` keyword against PostgreSQL's
    // anonymous code block, so the case forbids `do_statement`: with both on the code-block
    // branch shadows the list and `DO 1, 2` over-rejects (the registered grammar conflict), so
    // enabling the forbidden flag flips the outcome just as disabling the required one does.
    LabeledCase {
        sql: "DO 1, 2",
        expect: Expect::Accept,
        required: &[&DO_EXPRESSION_LIST],
        forbidden: &[&DO_STATEMENT],
    },
    // MySQL `LOCK/UNLOCK TABLES` — probed via `UNLOCK TABLES` rather than `LOCK TABLES t READ`
    // because `READ`/`WRITE` are non-reserved on the PostgreSQL baseline and get swallowed as a
    // table alias there; `UNLOCK TABLES` exercises the same gate with no lock-kind tail.
    LabeledCase {
        sql: "UNLOCK TABLES",
        expect: Expect::Accept,
        required: &[&LOCK_TABLES],
        forbidden: &[],
    },
    // MySQL `LOCK INSTANCE FOR BACKUP` — the instance-backup lock, a separate gate from
    // `lock_tables` (no table list, MySQL-8-specific).
    LabeledCase {
        sql: "LOCK INSTANCE FOR BACKUP",
        expect: Expect::Accept,
        required: &[&LOCK_INSTANCE],
        forbidden: &[],
    },
    // Snowflake stage reference `@stage` as a `COPY INTO` endpoint. Requires `copy_into` too
    // (the endpoint only exists inside that statement): with `copy_into` off the statement is
    // not dispatched, and with `stage_references` off the `@s` endpoint is not a valid operand,
    // so each required flag flips accept<->reject.
    LabeledCase {
        sql: "COPY INTO t FROM @s",
        expect: Expect::Accept,
        required: &[&COPY_INTO, &STAGE_REFERENCES],
        forbidden: &[],
    },
    // MySQL diagnostics area — the leading `SIGNAL` keyword gated like `kill`.
    LabeledCase {
        sql: "SIGNAL SQLSTATE '45000'",
        expect: Expect::Accept,
        required: &[&SIGNAL_DIAGNOSTICS],
        forbidden: &[],
    },
    // DuckDB `IMPORT DATABASE` — the import half of the export/import pair (one gate for both).
    LabeledCase {
        sql: "IMPORT DATABASE 'p'",
        expect: Expect::Accept,
        required: &[&EXPORT_IMPORT_DATABASE],
        forbidden: &[],
    },
    // DuckDB `UPDATE EXTENSIONS (…)` — a refinement of the `UPDATE` head claimed only when
    // `EXTENSIONS` is followed by `(`; off, the DML `UPDATE` parser rejects the missing `SET`.
    LabeledCase {
        sql: "UPDATE EXTENSIONS (foo)",
        expect: Expect::Accept,
        required: &[&UPDATE_EXTENSIONS],
        forbidden: &[],
    },
    // DuckDB dotted `USE <catalog>.<schema>` — refines the `USE` name grammar, so it requires
    // `use_statement` too: off, the leading `USE` is not dispatched; with the dotted-name flag
    // off, the MySQL single-name `USE` grammar rejects the dot.
    LabeledCase {
        sql: "USE a.b",
        expect: Expect::Accept,
        required: &[&USE_STATEMENT, &USE_QUALIFIED_NAME],
        forbidden: &[],
    },
    // PostgreSQL `CREATE EXTENSION` — a whole-statement gate; off, `EXTENSION` falls through to
    // the `CREATE TABLE` expectation.
    LabeledCase {
        sql: "CREATE EXTENSION foo",
        expect: Expect::Accept,
        required: &[&EXTENSION_DDL],
        forbidden: &[],
    },
    // PostgreSQL `DROP TRANSFORM` — a whole-statement gate separate from `extension_ddl`.
    LabeledCase {
        sql: "DROP TRANSFORM FOR int LANGUAGE c",
        expect: Expect::Accept,
        required: &[&TRANSFORM_DDL],
        forbidden: &[],
    },
    // PostgreSQL `ALTER SYSTEM` — a whole-statement gate; off, `SYSTEM` falls to the `ALTER
    // TABLE` expectation.
    LabeledCase {
        sql: "ALTER SYSTEM SET x = 1",
        expect: Expect::Accept,
        required: &[&ALTER_SYSTEM],
        forbidden: &[],
    },
    // MySQL tablespace storage-DDL — the `TABLESPACE` dispatch after `CREATE`.
    LabeledCase {
        sql: "CREATE TABLESPACE ts ADD DATAFILE 'd'",
        expect: Expect::Accept,
        required: &[&TABLESPACE_DDL],
        forbidden: &[],
    },
    // MySQL NDB logfile-group storage-DDL — the `LOGFILE GROUP` dispatch after `CREATE`.
    LabeledCase {
        sql: "CREATE LOGFILE GROUP g ADD UNDOFILE 'u'",
        expect: Expect::Accept,
        required: &[&LOGFILE_GROUP_DDL],
        forbidden: &[],
    },
    // PostgreSQL embedded schema-element list on `CREATE SCHEMA` — off, the trailing `CREATE
    // TABLE` is not consumed as an element and the single-statement parse rejects the trailing
    // input (the head itself stays accepted via `schemas`).
    LabeledCase {
        sql: "CREATE SCHEMA s CREATE TABLE t (a INT)",
        expect: Expect::Accept,
        required: &[&SCHEMA_ELEMENTS],
        forbidden: &[],
    },
    // MySQL single-name `DROP DATABASE` synonym — off, no `DROP DATABASE` grammar claims the
    // head on the PostgreSQL baseline and it rejects (the shared name-list drop covers `SCHEMA`,
    // not `DATABASE`).
    LabeledCase {
        sql: "DROP DATABASE db",
        expect: Expect::Accept,
        required: &[&DROP_DATABASE],
        forbidden: &[],
    },
    // DuckDB `CREATE RECURSIVE VIEW` — off, the `RECURSIVE` keyword is left unconsumed before
    // the expected `VIEW`.
    LabeledCase {
        sql: "CREATE RECURSIVE VIEW v (a) AS SELECT 1",
        expect: Expect::Accept,
        required: &[&RECURSIVE_VIEWS],
        forbidden: &[],
    },
    // DuckDB `ALTER DATABASE … SET ALIAS TO` — the `DATABASE` dispatch after `ALTER`.
    LabeledCase {
        sql: "ALTER DATABASE d SET ALIAS TO x",
        expect: Expect::Accept,
        required: &[&ALTER_DATABASE],
        forbidden: &[],
    },
    // MySQL `ALTER {DATABASE|SCHEMA}` option list — a disjoint behaviour from DuckDB's `SET
    // ALIAS`; off, `DATABASE` falls to the `ALTER TABLE` expectation.
    LabeledCase {
        sql: "ALTER DATABASE d DEFAULT CHARACTER SET utf8",
        expect: Expect::Accept,
        required: &[&ALTER_DATABASE_OPTIONS],
        forbidden: &[],
    },
    // MySQL federated-server DDL — the `SERVER` head after `CREATE`.
    LabeledCase {
        sql: "CREATE SERVER s FOREIGN DATA WRAPPER w OPTIONS (Host 'h')",
        expect: Expect::Accept,
        required: &[&SERVER_DEFINITION],
        forbidden: &[],
    },
    // MySQL `ALTER INSTANCE` — the `INSTANCE` dispatch after `ALTER`.
    LabeledCase {
        sql: "ALTER INSTANCE ROTATE INNODB MASTER KEY",
        expect: Expect::Accept,
        required: &[&ALTER_INSTANCE],
        forbidden: &[],
    },
    // DuckDB `ALTER SEQUENCE` — the `SEQUENCE` dispatch after `ALTER`.
    LabeledCase {
        sql: "ALTER SEQUENCE s START WITH 1",
        expect: Expect::Accept,
        required: &[&ALTER_SEQUENCE],
        forbidden: &[],
    },
    // DuckDB `ALTER … SET SCHEMA` relocation — off, the `SET SCHEMA` tail is left to the `ALTER
    // TABLE` command parser and rejects.
    LabeledCase {
        sql: "ALTER TABLE t SET SCHEMA sc",
        expect: Expect::Accept,
        required: &[&ALTER_OBJECT_SET_SCHEMA],
        forbidden: &[],
    },
    // MySQL view-definition options — probed via the `ALTER VIEW` redefinition it heads.
    LabeledCase {
        sql: "ALTER VIEW v AS SELECT 1",
        expect: Expect::Accept,
        required: &[&VIEW_DEFINITION_OPTIONS],
        forbidden: &[],
    },
    // DuckDB statement-leading `SUMMARIZE` (the `SHOW_REF` utility) — off, the leading
    // `SUMMARIZE` keyword is not dispatched.
    LabeledCase {
        sql: "SUMMARIZE t",
        expect: Expect::Accept,
        required: &[&DESCRIBE_SUMMARIZE],
        forbidden: &[],
    },
    // MySQL `DROP INDEX … ON <table>` — off, the mandatory `ON` is left unconsumed after the
    // shared name-list index drop.
    LabeledCase {
        sql: "DROP INDEX i ON t",
        expect: Expect::Accept,
        required: &[&INDEX_DROP_ON_TABLE],
        forbidden: &[],
    },
    // MySQL spatial-reference-system DDL: a whole-statement gate on the `CREATE` head. With
    // `spatial_reference_system` off the `SPATIAL` word falls through to the `TABLE`
    // expectation -> reject, so the flag is required.
    LabeledCase {
        sql: "CREATE SPATIAL REFERENCE SYSTEM 990001 NAME 'z' DEFINITION 'w'",
        expect: Expect::Accept,
        required: &[&SPATIAL_REFERENCE_SYSTEM],
        forbidden: &[],
    },
    // MySQL resource-group DDL: a whole-statement gate on the `CREATE` head. With
    // `resource_group` off the `RESOURCE` word falls through -> reject, so the flag is
    // required.
    LabeledCase {
        sql: "CREATE RESOURCE GROUP g TYPE = USER",
        expect: Expect::Accept,
        required: &[&RESOURCE_GROUP],
        forbidden: &[],
    },
    // The family's `SET` member rides the same flag from inside the shared `SET` dispatch:
    // on, the two-word `RESOURCE GROUP` lookahead claims the dedicated statement; off, the
    // words fall to the generic `SET <name> {= | TO} <value>` grammar, which rejects at
    // `GROUP` -> the flag alone flips accept/reject.
    LabeledCase {
        sql: "SET RESOURCE GROUP g",
        expect: Expect::Accept,
        required: &[&RESOURCE_GROUP],
        forbidden: &[],
    },
    // DESCRIBE (MySQL): the `describe` gate dispatches the `DESCRIBE`/`DESC` synonyms as
    // statement leaders. `DESCRIBE SELECT 1` is the EXPLAIN query synonym; `DESCRIBE t` is
    // the table-metadata overload. Both need the flag (with it off they reject), so it is
    // genuinely required for each.
    LabeledCase {
        sql: "DESCRIBE SELECT 1",
        expect: Expect::Accept,
        required: &[&DESCRIBE],
        forbidden: &[],
    },
    LabeledCase {
        sql: "DESCRIBE t",
        expect: Expect::Accept,
        required: &[&DESCRIBE],
        forbidden: &[],
    },
    // PREPARE / EXECUTE / DEALLOCATE (the prepared-statement lifecycle, on for DuckDB
    // and PostgreSQL): one flag gates all three leading keywords. With
    // `prepared_statements` off each falls through to the unknown-statement error ->
    // reject, so the flag is required for each.
    LabeledCase {
        sql: "PREPARE p AS SELECT 1",
        expect: Expect::Accept,
        required: &[&PREPARED_STATEMENTS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "EXECUTE p(1)",
        expect: Expect::Accept,
        required: &[&PREPARED_STATEMENTS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "DEALLOCATE p",
        expect: Expect::Accept,
        required: &[&PREPARED_STATEMENTS],
        forbidden: &[],
    },
    // PostgreSQL's `PREPARE name(<type>, …)` typed parameter-type list: a widening of
    // the `PREPARE` name position, so both flags are required — with
    // `prepare_typed_parameters` off, the `(` after the name is left untouched and the
    // statement falls through to the `AS` expectation, which then sees `(` and rejects.
    LabeledCase {
        sql: "PREPARE p(int, text) AS SELECT $1, $2",
        expect: Expect::Accept,
        required: &[&PREPARED_STATEMENTS, &PREPARE_TYPED_PARAMETERS],
        forbidden: &[],
    },
    // MySQL's `PREPARE ... FROM` lifecycle (`prepared_statements_from`): a DIFFERENT grammar
    // on the same leading keywords as `prepared_statements`, its registered grammar rival
    // (`GrammarConflict::PreparedStatementsVersusPreparedStatementsFrom`), which the PostgreSQL
    // baseline arms — so that flag is *forbidden* here to keep the baseline self-consistent. With
    // `prepared_statements` on the set is grammar-conflicting (no defined parse), so the registry
    // is the authority that the flip changes the reading (the genuine forbidden flip); with
    // `prepared_statements_from` off the keyword is not dispatched at all -> reject (the genuine
    // required flip).
    LabeledCase {
        sql: "PREPARE s FROM 'SELECT 1'",
        expect: Expect::Accept,
        required: &[&PREPARED_STATEMENTS_FROM],
        forbidden: &[&PREPARED_STATEMENTS],
    },
    // The `USING` members are user-variable references, so the `@name` lexing surface is
    // required too: with `user_variables` off, `@a` never lexes as a variable and the list
    // member rejects.
    LabeledCase {
        sql: "EXECUTE s USING @a",
        expect: Expect::Accept,
        required: &[&PREPARED_STATEMENTS_FROM, &USER_VARIABLES],
        forbidden: &[&PREPARED_STATEMENTS],
    },
    // The `DROP` spelling of the release verb is reached from the DROP dispatcher on the
    // `prepared_statements_from` gate: with the flag off, `PREPARE` falls through to the generic
    // drop-object-kind path and rejects as an unknown kind. The DuckDB `prepared_statements`
    // grammar has no `DROP PREPARE`, but the two flags are set-level grammar rivals
    // (`GrammarConflict::PreparedStatementsVersusPreparedStatementsFrom`), so it is *forbidden*
    // here to keep the baseline self-consistent; turning it on is a forbidden flip the registry
    // adjudicates.
    LabeledCase {
        sql: "DROP PREPARE s",
        expect: Expect::Accept,
        required: &[&PREPARED_STATEMENTS_FROM],
        forbidden: &[&PREPARED_STATEMENTS],
    },
    // CALL (DuckDB routine invocation): its own leading-keyword flag; off elsewhere the
    // keyword is not dispatched and surfaces as an unknown statement -> reject.
    LabeledCase {
        sql: "CALL my_proc(1)",
        expect: Expect::Accept,
        required: &[&CALL],
        forbidden: &[],
    },
    // MySQL's bare `CALL <name>` form: `call_bare_name` on top of the base `call` statement.
    // With `call` off nothing dispatches; with `call_bare_name` off the missing argument list
    // rejects (DuckDB's parentheses are mandatory), so both are required.
    LabeledCase {
        sql: "CALL my_proc",
        expect: Expect::Accept,
        required: &[&CALL, &CALL_BARE_NAME],
        forbidden: &[],
    },
    // DO (PostgreSQL anonymous code block): its own leading-keyword flag; off elsewhere the
    // `DO` keyword is not dispatched and surfaces as an unknown statement -> reject.
    LabeledCase {
        sql: "DO $$ BEGIN NULL; END $$",
        expect: Expect::Accept,
        required: &[&DO_STATEMENT],
        forbidden: &[],
    },
    // BEGIN IMMEDIATE (SQLite transaction-mode modifier): with `begin_transaction_mode`
    // off (ANSI/PostgreSQL/MySQL) the modifier keyword is not recognized and is left as a
    // stray trailing token -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "BEGIN IMMEDIATE",
        expect: Expect::Accept,
        required: &[&BEGIN_TRANSACTION_MODE],
        forbidden: &[],
    },
    // XA PREPARE (MySQL distributed-transaction family): its own leading-keyword flag; with
    // `xa_transactions` off (every non-MySQL preset) the `XA` keyword is not dispatched and
    // surfaces as an unknown statement -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "XA PREPARE 'gtrid'",
        expect: Expect::Accept,
        required: &[&XA_TRANSACTIONS],
        forbidden: &[],
    },
    // CREATE TRIGGER (SQLite): the whole-statement DDL gate — with `create_trigger`
    // off the `TRIGGER` after `CREATE` falls through to the `TABLE` expectation ->
    // reject, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE TRIGGER trg AFTER INSERT ON t BEGIN UPDATE t SET c = c + 1; END",
        expect: Expect::Accept,
        required: &[&CREATE_TRIGGER],
        forbidden: &[],
    },
    // CREATE MACRO (DuckDB): the whole-statement DDL gate — with `create_macro` off the
    // `MACRO` after `CREATE` falls through to the `TABLE` expectation -> reject, so the
    // flag is genuinely required.
    LabeledCase {
        sql: "CREATE MACRO m(x) AS x + 1",
        expect: Expect::Accept,
        required: &[&CREATE_MACRO],
        forbidden: &[],
    },
    // CREATE OR REPLACE TABLE (DuckDB): with `create_or_replace_table` off, `OR REPLACE`
    // still parses (POSTGRES has `or_replace`) but a following `TABLE` is expected to be
    // `VIEW` -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE OR REPLACE TABLE t (a INT)",
        expect: Expect::Accept,
        required: &[&CREATE_OR_REPLACE_TABLE],
        forbidden: &[],
    },
    // CREATE [PERSISTENT] SECRET (DuckDB): the whole-statement DDL gate — with
    // `create_secret` off the `PERSISTENT`/`SECRET` keyword falls through to the `TABLE`
    // expectation -> reject, so the flag is genuinely required.
    LabeledCase {
        sql: "CREATE PERSISTENT SECRET s (TYPE S3)",
        expect: Expect::Accept,
        required: &[&CREATE_SECRET],
        forbidden: &[],
    },
    // CREATE TYPE (DuckDB): the whole-statement DDL gate — with `create_type` off the `TYPE`
    // after `CREATE` falls through to the `TABLE` expectation -> reject, so the flag is
    // genuinely required.
    LabeledCase {
        sql: "CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy')",
        expect: Expect::Accept,
        required: &[&CREATE_TYPE],
        forbidden: &[],
    },
    // DROP TYPE (DuckDB): the same `create_type` gate admits the `TYPE` DROP object kind —
    // with it off, `TYPE` is an unexpected drop object kind -> reject.
    LabeledCase {
        sql: "DROP TYPE mood",
        expect: Expect::Accept,
        required: &[&CREATE_TYPE],
        forbidden: &[],
    },
    // CREATE VIRTUAL TABLE (SQLite): the whole-statement DDL gate — with
    // `create_virtual_table` off the `VIRTUAL` after `CREATE` falls through to the `TABLE`
    // expectation -> reject, so the flag is genuinely required. The module owns the argument
    // grammar, so the arguments are opaque verbatim slices split on the top-level commas.
    LabeledCase {
        sql: "CREATE VIRTUAL TABLE docs USING fts5(title, body)",
        expect: Expect::Accept,
        required: &[&CREATE_VIRTUAL_TABLE],
        forbidden: &[],
    },
    // CREATE SEQUENCE (PostgreSQL/DuckDB): the whole-statement T176 gate — with
    // `create_sequence` off the `SEQUENCE` after `CREATE` falls through to the `TABLE`
    // expectation -> reject, so the flag is genuinely required. The trailing options are the
    // shared standard core both engines' parsers accept.
    LabeledCase {
        sql: "CREATE SEQUENCE s START WITH 1 INCREMENT BY 2 MINVALUE 1 MAXVALUE 10 CYCLE",
        expect: Expect::Accept,
        required: &[&CREATE_SEQUENCE],
        forbidden: &[],
    },
    // DROP SEQUENCE rides the same flag (one flag gates both leading forms); off, `SEQUENCE`
    // is an unexpected DROP object kind -> reject.
    LabeledCase {
        sql: "DROP SEQUENCE s",
        expect: Expect::Accept,
        required: &[&CREATE_SEQUENCE],
        forbidden: &[],
    },
    // CREATE INDEX PostgreSQL clauses: with the gating flag off the keyword is left
    // unconsumed and the remaining tokens are leftover input -> reject, so each flag
    // is genuinely required.
    LabeledCase {
        sql: "CREATE INDEX CONCURRENTLY i ON t (a)",
        expect: Expect::Accept,
        required: &[&INDEX_CONCURRENTLY],
        forbidden: &[],
    },
    LabeledCase {
        sql: "CREATE INDEX i ON t USING btree (a)",
        expect: Expect::Accept,
        required: &[&INDEX_USING_METHOD],
        forbidden: &[],
    },
    LabeledCase {
        sql: "CREATE INDEX i ON t (a) WHERE a IS NOT NULL",
        expect: Expect::Accept,
        required: &[&PARTIAL_INDEX],
        forbidden: &[],
    },
    // MySQL CREATE TABLE storage decorations, both gated by `table_options`: with the
    // flag off the trailing `ENGINE = InnoDB` is leftover input, and the column-level
    // `AUTO_INCREMENT` is an unconsumed attribute -> reject. So the flag is required.
    LabeledCase {
        sql: "CREATE TABLE t (id INT) ENGINE = InnoDB",
        expect: Expect::Accept,
        required: &[&TABLE_OPTIONS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "CREATE TABLE t (id INT AUTO_INCREMENT)",
        expect: Expect::Accept,
        required: &[&TABLE_OPTIONS],
        forbidden: &[],
    },
    // The trailing `WITHOUT ROWID` table option rides its own `without_rowid_table_option`
    // flag (split out of the retired `sqlite_table_decorations` bundle).
    LabeledCase {
        sql: "CREATE TABLE t (a INTEGER) WITHOUT ROWID",
        expect: Expect::Accept,
        required: &[&WITHOUT_ROWID_TABLE_OPTION],
        forbidden: &[],
    },
    // The trailing `STRICT` table option rides its own `strict_table_option` flag (split out
    // of the retired `sqlite_table_decorations` bundle).
    LabeledCase {
        sql: "CREATE TABLE t (a INTEGER) STRICT",
        expect: Expect::Accept,
        required: &[&STRICT_TABLE_OPTION],
        forbidden: &[],
    },
    // The typeless column definition rides its own `typeless_column_definitions` flag (split
    // out of the retired `sqlite_table_decorations` bundle).
    LabeledCase {
        sql: "CREATE TABLE t (a, b)",
        expect: Expect::Accept,
        required: &[&TYPELESS_COLUMN_DEFINITIONS],
        forbidden: &[],
    },
    // The joined `AUTOINCREMENT` attribute rides its own `joined_autoincrement_attribute` flag
    // (split out of the retired `sqlite_table_decorations` bundle); the underscored
    // MySQL `AUTO_INCREMENT` spelling stays on `table_options` (see the case above).
    LabeledCase {
        sql: "CREATE TABLE t (a INTEGER PRIMARY KEY AUTOINCREMENT)",
        expect: Expect::Accept,
        required: &[&JOINED_AUTOINCREMENT_ATTRIBUTE],
        forbidden: &[],
    },
    // The column-level `ON CONFLICT` clause rides its own `column_conflict_resolution_clause`
    // flag (split out of the retired `sqlite_table_decorations` bundle).
    LabeledCase {
        sql: "CREATE TABLE t (a INTEGER UNIQUE ON CONFLICT REPLACE)",
        expect: Expect::Accept,
        required: &[&COLUMN_CONFLICT_RESOLUTION_CLAUSE],
        forbidden: &[],
    },
    // The inline `PRIMARY KEY` `ASC`/`DESC` order qualifier rides its own
    // `inline_primary_key_ordering` flag (split out of the retired `sqlite_table_decorations`
    // bundle); both directions are gated by the same flag.
    LabeledCase {
        sql: "CREATE TABLE t (a INTEGER PRIMARY KEY ASC)",
        expect: Expect::Accept,
        required: &[&INLINE_PRIMARY_KEY_ORDERING],
        forbidden: &[],
    },
    LabeledCase {
        sql: "CREATE TABLE t (a INTEGER PRIMARY KEY DESC)",
        expect: Expect::Accept,
        required: &[&INLINE_PRIMARY_KEY_ORDERING],
        forbidden: &[],
    },
    // The `CONSTRAINT <name>` prefix on a column COLLATE rides its own
    // `named_column_collate_constraint` flag (split out of the retired `sqlite_table_decorations`
    // bundle); it wraps
    // an already-admitted COLLATE clause, so the accepting case needs the bare-COLLATE
    // `column_collation` flag too — both are genuinely required.
    LabeledCase {
        sql: "CREATE TABLE t (a TEXT CONSTRAINT c COLLATE nocase)",
        expect: Expect::Accept,
        required: &[&NAMED_COLUMN_COLLATE_CONSTRAINT, &COLUMN_COLLATION],
        forbidden: &[],
    },
    // PostgreSQL expression forms: each is gated by its own sub-flag, so disabling
    // it turns the construct into a trailing parse error (`::int`, `[1]`, …).
    LabeledCase {
        sql: "SELECT a::int",
        expect: Expect::Accept,
        required: &[&TYPECAST_OPERATOR],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT a[1]",
        expect: Expect::Accept,
        required: &[&SUBSCRIPT],
        forbidden: &[],
    },
    // The DuckDB three-bound stepped slice needs both gates: without `subscript` the
    // brackets never open; without `slice_step` the second `:` is a clean parse error.
    LabeledCase {
        sql: "SELECT a[1:2:3]",
        expect: Expect::Accept,
        required: &[&SUBSCRIPT, &SLICE_STEP],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT a[1:-:2]",
        expect: Expect::Accept,
        required: &[&SUBSCRIPT, &SLICE_STEP],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT a COLLATE \"C\"",
        expect: Expect::Accept,
        required: &[&COLLATE],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT a AT TIME ZONE 'UTC'",
        expect: Expect::Accept,
        required: &[&AT_TIME_ZONE],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT src:customer[0].name",
        expect: Expect::Accept,
        required: &[&SEMI_STRUCTURED_ACCESS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT ARRAY[1, 2]",
        expect: Expect::Accept,
        required: &[&ARRAY_CONSTRUCTOR],
        forbidden: &[],
    },
    // The DuckDB collection literals, all riding one gate: with it off, a primary
    // `[`/`{` falls to the punctuation reject arm and a `MAP {` leaves the `{` as
    // leftover input after the `map` name — each a clean parse error.
    LabeledCase {
        sql: "SELECT [1, 2, 3]",
        expect: Expect::Accept,
        required: &[&COLLECTION_LITERALS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT {'a': 1, 'b': [2, 3]}",
        expect: Expect::Accept,
        required: &[&COLLECTION_LITERALS],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT MAP {'a': 1}",
        expect: Expect::Accept,
        required: &[&COLLECTION_LITERALS],
        forbidden: &[],
    },
    // DuckDB `#n` positional column reference. With `positional_column` off, its coupled
    // toggle restores the PostgreSQL `#` XOR operator, so `#2` in expression position has
    // no prefix reading and the parse rejects — the falsely-required flip. The `#`
    // lexeme is scanned only under this gate, so no other dialect reaches this shape.
    LabeledCase {
        sql: "SELECT a, b FROM t ORDER BY #2, #1",
        expect: Expect::Accept,
        required: &[&POSITIONAL_COLUMN],
        forbidden: &[],
    },
    // The implicit `(a, b)` row form flips cleanly: with the constructor off, the
    // comma inside the grouping is a parse error rather than a function-call arg.
    LabeledCase {
        sql: "SELECT (a, b)",
        expect: Expect::Accept,
        required: &[&ROW_CONSTRUCTOR],
        forbidden: &[],
    },
    // The BigQuery `STRUCT(...)` value constructor. Off, `struct(x AS a)` is an
    // ordinary call whose `AS` has no argument grammar — a clean parse error — and the
    // typed form reads `struct < a` with the trailing type name unconsumed: both flips
    // reject, making the flag genuinely required.
    LabeledCase {
        sql: "SELECT STRUCT(x AS a, y AS b)",
        expect: Expect::Accept,
        required: &[&STRUCT_CONSTRUCTOR],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT STRUCT<a INT64, b STRING>(1, 'x')",
        expect: Expect::Accept,
        required: &[&STRUCT_CONSTRUCTOR],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT (a).b",
        expect: Expect::Accept,
        required: &[&FIELD_SELECTION],
        forbidden: &[],
    },
    // The value-position `.*` composite/whole-row star: with the flag off the `.*` off a
    // `ROW(...)` field is left unconsumed and rejects, distinct from the select-list
    // `t.*` qualified wildcard (engine-probed accept on PG, reject on DuckDB).
    LabeledCase {
        sql: "SELECT ROW(t.*)",
        expect: Expect::Accept,
        required: &[&FIELD_WILDCARD],
        forbidden: &[],
    },
    // Named function arguments: with the flag off the `=>` arrow does not lex as one
    // token (it is `=` then `>`), so `f(a => 1)` parses `a = ` and then hits the
    // bare `>`, a trailing parse error rather than a named argument.
    LabeledCase {
        sql: "SELECT f(a => 1)",
        expect: Expect::Accept,
        required: &[&NAMED_ARGUMENT],
        forbidden: &[],
    },
    // The call-site VARIADIC array-spread marker on the final argument. With the flag
    // off the `VARIADIC` keyword is not admitted before the argument, so it surfaces as
    // a parse error rather than an accepted spread (engine-probed on PG and DuckDB).
    LabeledCase {
        sql: "SELECT f(a, VARIADIC arr)",
        expect: Expect::Accept,
        required: &[&VARIADIC_ARGUMENT],
        forbidden: &[],
    },
    // The explicit-operator infix form: with the flag off the `OPERATOR` keyword is
    // left unconsumed after `a`, so the trailing `OPERATOR(+) b` is a parse error.
    LabeledCase {
        sql: "SELECT a OPERATOR(+) b",
        expect: Expect::Accept,
        required: &[&OPERATOR_CONSTRUCT],
        forbidden: &[],
    },
    // PostgreSQL containment operators. With the gate off, the `@>` lexeme is not
    // recognised (the dispatch arm declines and `@` is a stray byte), so `SELECT a @> b`
    // is a lex/parse error — the falsely-required flip. `<@` rides the same sub-flag.
    LabeledCase {
        sql: "SELECT a @> b",
        expect: Expect::Accept,
        required: &[&CONTAINMENT_OPERATORS],
        forbidden: &[],
    },
    // With the JSON-arrow gate off, `->` stays a `-` then a `>`, so the RHS after `-`
    // starts with `>` — an expression error. Only the gate on accepts `a -> b`.
    LabeledCase {
        sql: "SELECT a -> b",
        expect: Expect::Accept,
        required: &[&JSON_ARROW_OPERATORS],
        forbidden: &[],
    },
    // The PostgreSQL `jsonb` operators (the whole `?`/`?|`/`?&`/`@?`/`@@`/`#>`/`#>>`/`#-`
    // family rides one gate). With it off, `#>` stays the `#` bitwise-XOR then a stray `>`,
    // so `a #> b` rejects — the falsely-required flip. `#>` is the representative member.
    LabeledCase {
        sql: "SELECT a #> b",
        expect: Expect::Accept,
        required: &[&JSONB_OPERATORS],
        forbidden: &[],
    },
    // PostgreSQL's general symbolic-operator surface — regex `~`/`!~`/`~*`/`!~*`,
    // geometric/network ops, and fully user-defined operators. With the gate off, `~` is
    // prefix-only (bitwise complement) and never an infix operator, so `a ~ b` ends the
    // expression at `a` and rejects. `~` (regex-match) is the representative member.
    LabeledCase {
        sql: "SELECT a ~ b",
        expect: Expect::Accept,
        required: &[&CUSTOM_OPERATORS],
        forbidden: &[],
    },
    // DuckDB's postfix operator reduction (the reading PostgreSQL removed in 14): a trailing
    // symbolic operator with no operand folds to `Expr::PostfixOperator`. With the gate off,
    // `10!` reads `!` as a bare infix operator whose right operand is missing, so it ends the
    // expression at `10` and the trailing `!` rejects; only the gate on accepts the postfix.
    // `!` (factorial) is the representative member — it lexes without `custom_operators`, so
    // this flag is the sole discriminator.
    LabeledCase {
        sql: "SELECT 10!",
        expect: Expect::Accept,
        required: &[&POSTFIX_OPERATORS],
        forbidden: &[],
    },
    // PostgreSQL's `^` exponentiation. With the gate off (and no `Caret`-XOR spelling), `^`
    // is not an infix operator, so `2 ^ 3` ends the expression at `2` and rejects. Only the
    // gate on accepts it as arithmetic power.
    LabeledCase {
        sql: "SELECT 2 ^ 3",
        expect: Expect::Accept,
        required: &[&EXPONENT_OPERATOR],
        forbidden: &[],
    },
    // With the bitwise gate off, `&` is not an infix operator, so the expression ends at
    // `1` and the trailing `& 2` is leftover input -> reject. Only the gate on accepts the
    // shared `| & ~ << >>` family. (The `&&`-as-AND meaning is a separate knob, so plain
    // `&` never mis-binds when this is off.)
    LabeledCase {
        sql: "SELECT 1 & 2",
        expect: Expect::Accept,
        required: &[&BITWISE_OPERATORS],
        forbidden: &[],
    },
    // MySQL `GROUP_CONCAT(... SEPARATOR '<sep>')`: with the flag off the `SEPARATOR`
    // keyword inside the call is left unconsumed and the expected closing `)` sees it
    // as leftover input -> reject. Only the gate on accepts the delimiter tail.
    LabeledCase {
        sql: "SELECT group_concat(a SEPARATOR ',')",
        expect: Expect::Accept,
        required: &[&GROUP_CONCAT_SEPARATOR],
        forbidden: &[],
    },
    // The pattern-match predicates. With each gate off the keyword is left unconsumed
    // after the left operand (or read as a bare alias), so the trailing pattern is a
    // parse error — making every sub-flag genuinely required (the falsely-required flip).
    LabeledCase {
        sql: "SELECT a LIKE 'b%'",
        expect: Expect::Accept,
        required: &[&LIKE],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT a ILIKE 'b%'",
        expect: Expect::Accept,
        required: &[&ILIKE],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT a SIMILAR TO 'b%'",
        expect: Expect::Accept,
        required: &[&SIMILAR_TO],
        forbidden: &[],
    },
    // DuckDB's unparenthesized `IN <value>`: with the flag off, `IN b` needs a `(`, so
    // the bare value is a parse error — an executable flip (accept -> reject).
    LabeledCase {
        sql: "SELECT a IN b",
        expect: Expect::Accept,
        required: &[&UNPARENTHESIZED_IN_LIST],
        forbidden: &[],
    },
    // SELECT-clause gates: with the flag off the introducing keyword is left
    // unconsumed and the trailing tokens are a parse error, so each flag is
    // genuinely required.
    LabeledCase {
        sql: "SELECT DISTINCT ON (a) a FROM t",
        expect: Expect::Accept,
        required: &[&DISTINCT_ON],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT 1 FETCH FIRST 2 ROWS ONLY",
        expect: Expect::Accept,
        required: &[&FETCH_FIRST],
        forbidden: &[],
    },
    // The MySQL `LIMIT <offset>, <count>` comma form. With the gate off the `,` is
    // left unconsumed after `LIMIT 1`, so `, 2` is trailing input — a parse error —
    // which makes the flag genuinely required (the falsely-required flip).
    LabeledCase {
        sql: "SELECT 1 LIMIT 1, 2",
        expect: Expect::Accept,
        required: &[&LIMIT_OFFSET_COMMA],
        forbidden: &[],
    },
    // PostgreSQL `SELECT … INTO <table>` create-table form (the new-table target sits
    // between the projection and `FROM`). With the gate off, `INTO` is left
    // unconsumed and is trailing input — a parse error — so the flag is genuinely
    // required (the falsely-required flip). This is the materialize-into-a-new-table
    // form, distinct from the SQL-standard `SELECT … INTO <variable>` assignment.
    LabeledCase {
        sql: "SELECT a INTO t FROM s",
        expect: Expect::Accept,
        required: &[&SELECT_INTO],
        forbidden: &[],
    },
    // The grouping-set GROUP BY items. `ROLLUP (a, b)` is a *structural* flip, and it
    // is the exact mis-parse this ticket fixes: with `grouping_sets` on it is a
    // `GroupByItem::Rollup` grouping construct; with the gate off, `rollup` is an
    // unreserved (in PostgreSQL) word and `rollup (a, b)` re-parses as an ordinary
    // function-call expression — byte-identical to a user function, which is why the
    // old AST silently swallowed it. The shape check pins the grouping-construct
    // reading, and the falsely-required flip proves the function-call reading returns
    // with the gate off.
    LabeledCase {
        sql: "SELECT a FROM t GROUP BY ROLLUP (a, b)",
        expect: Expect::Shape(group_by_is_rollup),
        required: &[&GROUPING_SETS],
        forbidden: &[],
    },
    // The empty grouping set `()` is the one grouping form that is a genuine
    // accept/reject flip: with the gate off it falls through to the expression
    // grammar, where `()` is not a valid expression, so the trailing `()` is a parse
    // error (the falsely-required flip).
    LabeledCase {
        sql: "SELECT a FROM t GROUP BY ()",
        expect: Expect::Accept,
        required: &[&GROUPING_SETS],
        forbidden: &[],
    },
    // MySQL's trailing `GROUP BY <keys> WITH ROLLUP` modifier — its only grouping-set
    // surface. A genuine accept/reject flip: with `with_rollup` off (PostgreSQL/ANSI,
    // which spell the super-aggregate `ROLLUP (…)`), the `WITH` is left unconsumed and
    // the trailing `WITH ROLLUP` is a parse error (the falsely-required flip); on, the
    // key list canonicalizes into one `GroupByItem::Rollup` tagged `WithRollup`. This
    // is the coverage-level proof that PostgreSQL/ANSI reject the form.
    LabeledCase {
        sql: "SELECT a FROM t GROUP BY a, b WITH ROLLUP",
        expect: Expect::Accept,
        required: &[&WITH_ROLLUP],
        forbidden: &[],
    },
    // The PostgreSQL `ORDER BY <expr> USING <operator>` sort. A genuine accept/reject
    // flip: with `order_by_using` off, the `USING` keyword is left unconsumed and the
    // trailing `USING <` is a parse error (the falsely-required flip); on, it is the
    // operator-driven sort form.
    // The empty SELECT target list (`SELECT FROM t` — no projection). A genuine
    // accept/reject flip: with `empty_target_list` off (ANSI/MySQL, which require ≥1
    // select item), the projection's first-item requirement stands and `FROM` — a
    // reserved keyword, not an expression — is a parse error (the falsely-required
    // flip); on (PostgreSQL/Lenient), the projection is admitted empty.
    LabeledCase {
        sql: "SELECT FROM t",
        expect: Expect::Accept,
        required: &[&EMPTY_TARGET_LIST],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT a FROM t ORDER BY a USING <",
        expect: Expect::Accept,
        required: &[&ORDER_BY_USING],
        forbidden: &[],
    },
    // DuckDB's `QUALIFY` post-window filter. A genuine accept/reject flip: with the
    // gate off the keyword is left unconsumed after the GROUP BY key, so the trailing
    // `QUALIFY …` is a parse error (the falsely-required flip). The GROUP BY key
    // precedes it because the flip baseline (POSTGRES + the flag) does not reserve
    // `QUALIFY` — after a grouping expression no alias can claim the word, isolating
    // the clause gate from the DuckDb preset's reservation delta.
    LabeledCase {
        sql: "SELECT a FROM t GROUP BY a QUALIFY row_number() OVER () = 1",
        expect: Expect::Accept,
        required: &[&QUALIFY],
        forbidden: &[],
    },
    // DuckDB's `GROUP BY ALL` / `ORDER BY ALL` clause modes. Genuine accept/reject
    // flips: `ALL` is reserved in every shipped dialect (and in the POSTGRES flip
    // baseline), so with the gate off the keyword cannot open a grouping/sort
    // expression and the clause is a parse error; on, it is the whole-clause mode.
    LabeledCase {
        sql: "SELECT a, count(*) FROM t GROUP BY ALL",
        expect: Expect::Accept,
        required: &[&GROUP_BY_ALL],
        forbidden: &[],
    },
    LabeledCase {
        sql: "SELECT a FROM t ORDER BY ALL DESC NULLS LAST",
        expect: Expect::Accept,
        required: &[&ORDER_BY_ALL],
        forbidden: &[],
    },
    // DuckDB's name-matched set operation. The flip proves the gate: off, `BY` after a
    // set operator is a syntax error.
    LabeledCase {
        sql: "SELECT 1 AS a UNION BY NAME SELECT 2 AS a",
        expect: Expect::Accept,
        required: &[&UNION_BY_NAME],
        forbidden: &[],
    },
    // DuckDB's FROM-first SELECT order. A genuine accept/reject flip: `FROM` is reserved
    // in every shipped dialect (and in the POSTGRES flip baseline), so with the gate off a
    // statement-position `FROM` is not a query start and the parse errors; on, it opens the
    // FROM-first primary.
    LabeledCase {
        sql: "FROM t SELECT a",
        expect: Expect::Accept,
        required: &[&FROM_FIRST],
        forbidden: &[],
    },
    // BigQuery/ZetaSQL query pipe syntax — the framework-level gate for the
    // `planner-parity-pipe-*` operators. A genuine accept/reject flip: the gate is shared
    // by the tokenizer, so with it off `|>` never lexes (the bytes stay `|` then `>`),
    // leaving a `|>` after a query as trailing junk the statement parser rejects; on, the
    // `|>` opens the pipe-operator chain. The framework ships the reference `WHERE`
    // operator, so this exercises `|> WHERE`.
    LabeledCase {
        sql: "SELECT a FROM t |> WHERE a > 1",
        expect: Expect::Accept,
        required: &[&PIPE_SYNTAX],
        forbidden: &[],
    },
    // Type-name vocabulary gates. The extended *scalar* names are a structural
    // (not accept/reject) feature: with `extended_scalar_type_names` off, `TINYINT` is still a
    // valid bare type name and falls through to the user-defined-type path, so the SQL
    // keeps parsing but the cast target's shape changes (built-in `TinyInt` vs
    // `UserDefined`) — like the string-prefix marker cases.
    LabeledCase {
        sql: "SELECT CAST(a AS TINYINT)",
        expect: Expect::Shape(cast_target_is_builtin_tinyint),
        required: &[&EXTENDED_SCALAR_TYPE_NAMES],
        forbidden: &[],
    },
    // `ENUM('a','b')` is genuinely accept/reject: with `enum_type` off, `ENUM` is a
    // user-defined type name and the string value list is not the numeric modifier list
    // that path accepts, so the trailing `('a','b')` is a parse error.
    LabeledCase {
        sql: "CREATE TABLE t (c ENUM('a', 'b'))",
        expect: Expect::Accept,
        required: &[&ENUM_TYPE],
        forbidden: &[],
    },
    // `SET('a','b')` is the independent MySQL sibling: with `set_type` off, `SET` is a
    // user-defined type name and the string value list is not the numeric modifier list —
    // a parse error. Split from `enum_type` because DuckDB has `ENUM` but no `SET` type.
    LabeledCase {
        sql: "CREATE TABLE t (c SET('a', 'b'))",
        expect: Expect::Accept,
        required: &[&SET_TYPE],
        forbidden: &[],
    },
    // `INT UNSIGNED` is accept/reject: with `numeric_modifiers` off the postfix
    // `UNSIGNED` is left unconsumed after the recognized `INT`, so it is trailing input
    // in the column definition — a parse error.
    LabeledCase {
        sql: "CREATE TABLE t (c INT UNSIGNED)",
        expect: Expect::Accept,
        required: &[&NUMERIC_MODIFIERS],
        forbidden: &[],
    },
    // `INT(11)` display width is accept/reject: with `integer_display_width` off the
    // `(11)` after the recognized built-in `INT` is left unconsumed — trailing input in
    // the column definition, a parse error (matching pg_query's reject).
    LabeledCase {
        sql: "CREATE TABLE t (c INT(11))",
        expect: Expect::Accept,
        required: &[&INTEGER_DISPLAY_WIDTH],
        forbidden: &[],
    },
    // DuckDB's anonymous composite type in a cast: with `composite_types` off, `STRUCT`
    // falls through to the user-defined-type name and `(x INTEGER)` is read as a type
    // modifier list, whose `x` is not an unsigned integer — a clean parse error (matching
    // PostgreSQL's syntax reject).
    LabeledCase {
        sql: "SELECT CAST(a AS STRUCT(x INTEGER))",
        expect: Expect::Accept,
        required: &[&COMPOSITE_TYPES],
        forbidden: &[],
    },
    // SQLite's liberal multi-word affinity type name: with `liberal_type_names` off, the head
    // word parses as a single-word user-defined type and the trailing word is unconsumed input
    // in the column definition — a clean parse error (matching pg_query's reject of a closed
    // type vocabulary). Plain identifier words (not `LONG INTEGER`) so the continuation word is
    // admissible under the POSTGRES-derived baseline's `reserved_type_name` too — the SQLite
    // corpus surface `LONG INTEGER` relies on SQLite leaving `INTEGER` unreserved as a
    // type-name word. A genuine accept/reject flip.
    LabeledCase {
        sql: "CREATE TABLE t (c foo bar)",
        expect: Expect::Accept,
        required: &[&LIBERAL_TYPE_NAMES],
        forbidden: &[],
    },
    // DuckDB's `TRY_CAST(... AS ...)` null-on-failure cast: with `try_cast` off, `TRY_CAST`
    // reads as an ordinary function name and the `AS` inside its parentheses is a clean
    // parse error (matching PostgreSQL's syntax reject at `AS`).
    LabeledCase {
        sql: "SELECT TRY_CAST(a AS INTEGER)",
        expect: Expect::Accept,
        required: &[&TRY_CAST],
        forbidden: &[],
    },
    // DuckDB's quoted `EXTRACT('field' FROM x)` field: with `extract_string_field` off, the
    // field must be a bare identifier, so the leading string constant is a clean parse error
    // (the standard `EXTRACT` field grammar). A genuine accept/reject flip.
    LabeledCase {
        sql: "SELECT EXTRACT('year' FROM x)",
        expect: Expect::Accept,
        required: &[&EXTRACT_STRING_FIELD],
        forbidden: &[],
    },
    // DuckDB dot-method call chaining `<receiver>.<method>(<args>)`: with `method_chaining`
    // off, `list(x).foo` is ordinary composite field selection and the trailing `(1)` is
    // unconsumed input — a clean parse error. A genuine accept/reject flip.
    LabeledCase {
        sql: "SELECT list(x).foo(1)",
        expect: Expect::Accept,
        required: &[&METHOD_CHAINING],
        forbidden: &[],
    },
    // DuckDB's in-parenthesis `IGNORE NULLS` null-treatment: with `null_treatment` off,
    // `IGNORE` after the argument is left unconsumed and the `)` that closes the call cannot
    // follow it — a clean parse error. A genuine accept/reject flip.
    LabeledCase {
        sql: "SELECT last(x IGNORE NULLS) OVER ()",
        expect: Expect::Accept,
        required: &[&NULL_TREATMENT],
        forbidden: &[],
    },
    // DuckDB's `USING SAMPLE <entry>` query-level sample clause: with `using_sample` off, the
    // `USING` keyword in that position is unconsumed trailing input — a clean parse error. A
    // genuine accept/reject flip.
    LabeledCase {
        sql: "SELECT c FROM t USING SAMPLE 3",
        expect: Expect::Accept,
        required: &[&USING_SAMPLE],
        forbidden: &[],
    },
    // The structural `forbidden_features` teeth (prod-coverage-labels-differential-corpus).
    // `||` is string concatenation under the ANSI/PostgreSQL `pipe_operator`, but a
    // MySQL-like dialect reads it as logical OR. Both meanings *parse*, so this is a
    // structural divergence, not accept/reject: the case forbids `logical_or_pipe` and
    // asserts the concat shape. Enabling the forbidden feature re-parses `||` as OR —
    // the SQL still parses but the shape changes, which the falsely-forbidden flip in
    // `declared_features_are_genuinely_required` verifies. This is the first label to
    // put `forbidden` to real use.
    LabeledCase {
        sql: "SELECT a || b",
        expect: Expect::Shape(pipe_is_string_concat),
        required: &[],
        forbidden: &[&LOGICAL_OR_PIPE],
    },
    // MySQL same-line adjacent-string concatenation. On, `'a' 'b'` (no newline between)
    // folds into one string; off, the standard requires a newline in the separator, so
    // same-line adjacency is a clean parse error — a genuine accept/reject flip.
    LabeledCase {
        sql: "SELECT 'a' 'b'",
        expect: Expect::Accept,
        required: &[&SAME_LINE_ADJACENT_CONCAT],
        forbidden: &[],
    },
    // MySQL `<=>` null-safe equality. On, the `<=>` lexeme is munched (ahead of `<=`) and
    // folds onto the null-safe operator; off, the bytes are `<=` then a dangling `>`,
    // which cannot open the right operand — a parse error. A genuine accept/reject flip.
    LabeledCase {
        sql: "SELECT 1 <=> 2",
        expect: Expect::Accept,
        required: &[&NULL_SAFE_EQUALS],
        forbidden: &[],
    },
    // The SQL:2016 truth-value test `IS UNKNOWN` (F571). The `UNKNOWN` form isolates the
    // predicate: on, `IS UNKNOWN` parses to `Expr::IsTruth`; off (and without SQLite's
    // general equality, which the POSTGRES baseline lacks) `IS` requires `NULL`/`DISTINCT
    // FROM`, so `UNKNOWN` after it is a parse error — a genuine accept/reject flip.
    LabeledCase {
        sql: "SELECT a IS UNKNOWN",
        expect: Expect::Accept,
        required: &[&TRUTH_VALUE_TESTS],
        forbidden: &[],
    },
    // MySQL `UTC_DATE` niladic function. Structural, not accept/reject: on, the bare
    // keyword is a special value function; off, it is an ordinary (non-reserved) column
    // reference — both parse, so flipping the flag changes the shape (like the
    // string-prefix marker cases).
    LabeledCase {
        sql: "SELECT UTC_DATE",
        expect: Expect::Shape(projection_is_a_special_function),
        required: &[&UTC_SPECIAL_FUNCTIONS],
        forbidden: &[],
    },
    // MySQL string-literal column alias. On, `AS 'x'` names the projection with the
    // string's value; off, a string is not an admissible alias, so `'x'` after `AS` is a
    // parse error — a genuine accept/reject flip.
    LabeledCase {
        sql: "SELECT 1 AS 'x'",
        expect: Expect::Accept,
        required: &[&ALIAS_STRING_LITERALS],
        forbidden: &[],
    },
    // DuckDB single-arrow lambda. Structural, not accept/reject: the `->` token is
    // lexed by `json_arrow_operators` (on in the POSTGRES baseline) either way, so
    // the text always parses — on, a parameter-shaped left operand builds the
    // dedicated lambda node; off, the same text keeps the inherited `JsonGet` fold
    // (like the `pipe_operator` structural case, the flag changes only the shape).
    LabeledCase {
        sql: "SELECT x -> x + 1",
        expect: Expect::Shape(projection_is_a_lambda),
        required: &[&LAMBDA_EXPRESSIONS],
        forbidden: &[],
    },
    // DuckDB wildcard modifiers. A genuine accept/reject flip: with the gate off, a
    // `*` projection item is complete and the trailing `EXCLUDE` keyword is
    // unconsumed input — a clean parse error.
    LabeledCase {
        sql: "SELECT * EXCLUDE (a) REPLACE (b + 1 AS b) RENAME (c AS d) FROM t",
        expect: Expect::Accept,
        required: &[&WILDCARD_MODIFIERS],
        forbidden: &[],
    },
    // Bare (AS-less) output alias after a qualified wildcard `t.*`
    // (parse-qualified-wildcard-bare-alias). A genuine accept/reject flip: with the gate
    // off, the `t.*` item is complete and the trailing word `a` is unconsumed input — a
    // clean parse error — while PostgreSQL reads `t.*` as an aliasable columnref.
    LabeledCase {
        sql: "SELECT t.* a FROM t",
        expect: Expect::Accept,
        required: &[&QUALIFIED_WILDCARD_ALIAS],
        forbidden: &[],
    },
    // DuckDB `COLUMNS(...)` star expression. Structural, not accept/reject: with the
    // gate off, `COLUMNS('re')` re-parses as an ordinary call to a function named
    // `columns` (the word is non-reserved), so the SQL keeps parsing and only the
    // projection's shape changes — the `lambda_expressions` structural precedent.
    LabeledCase {
        sql: "SELECT COLUMNS('re') FROM t",
        expect: Expect::Shape(projection_is_a_columns_selector),
        required: &[&COLUMNS_EXPRESSION],
        forbidden: &[],
    },
    // Prefix-typed string literal. Off, the `DATE` type keyword and the following string
    // do not compose (no juxtaposition of a name and a string), a clean parse error.
    LabeledCase {
        sql: "SELECT DATE '2020-01-01'",
        expect: Expect::Accept,
        required: &[&TYPED_STRING_LITERALS],
        forbidden: &[],
    },
    // DuckDB python-style keyword lambda. Off, `lambda` reads as an ordinary column and
    // the trailing parameter leaves `x` unconsumed — a clean parse error.
    LabeledCase {
        sql: "SELECT list_filter(NULL, lambda x: x)",
        expect: Expect::Accept,
        required: &[&LAMBDA_KEYWORD],
        forbidden: &[],
    },
    // DuckDB's relaxed interval spellings — the unquoted integer amount and plural unit in
    // one case. Off, `INTERVAL` has no string to open the standard literal, so it falls
    // back to an ordinary name and the trailing `3` is unconsumed — a clean parse error.
    LabeledCase {
        sql: "SELECT INTERVAL 3 DAYS",
        expect: Expect::Accept,
        required: &[&RELAXED_INTERVAL_SYNTAX],
        forbidden: &[],
    },
    // `WITHIN GROUP` ordered-set aggregate. Off, `WITHIN` is left unconsumed after the
    // call — a trailing-input parse error.
    LabeledCase {
        sql: "SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY x)",
        expect: Expect::Accept,
        required: &[&WITHIN_GROUP],
        forbidden: &[],
    },
    // `FILTER (WHERE …)` aggregate filter. Off (MySQL), `FILTER` is left unconsumed after
    // the call — a trailing-input parse error.
    LabeledCase {
        sql: "SELECT sum(x) FILTER (WHERE x > 1)",
        expect: Expect::Accept,
        required: &[&AGGREGATE_FILTER],
        forbidden: &[],
    },
    // SQL/JSON expression functions. On (PostgreSQL/Lenient), the `JSON_VALUE(context, path
    // RETURNING …)` special form parses; off, `JSON_VALUE` is a reserved keyword head that
    // cannot open a call, so the clause-tail form is a parse error.
    LabeledCase {
        sql: "SELECT JSON_VALUE(js, '$.a' RETURNING int DEFAULT 0 ON ERROR)",
        expect: Expect::Accept,
        required: &[&SQLJSON_EXPRESSION_FUNCTIONS],
        forbidden: &[],
    },
    // SQL/XML expression functions. On (PostgreSQL/Lenient), the `xmlelement(NAME …,
    // xmlattributes(…))` special form parses; off, `xmlelement` is a bare keyword head that
    // cannot open the `NAME`-clause form, so it is a parse error.
    LabeledCase {
        sql: "SELECT xmlelement(NAME root, xmlattributes(1 AS a), 'body')",
        expect: Expect::Accept,
        required: &[&XML_EXPRESSION_FUNCTIONS],
        forbidden: &[],
    },
    // SUBSTRING keyword form. Off (SQLite), the head is an ordinary call name and the
    // inner `FROM` is a parse error.
    LabeledCase {
        sql: "SELECT SUBSTRING('abcdef' FROM 2 FOR 3)",
        expect: Expect::Accept,
        required: &[&SUBSTRING_FROM_FOR],
        forbidden: &[],
    },
    // The FOR-leading SUBSTRING order. Off (MySQL/ANSI), the `FOR` tail is not admitted,
    // so the head falls to the plain-call path where `FOR` is a parse error.
    LabeledCase {
        sql: "SELECT SUBSTRING('abcdef' FOR 3)",
        expect: Expect::Accept,
        required: &[&SUBSTRING_LEADING_FOR],
        forbidden: &[],
    },
    // PostgreSQL's SIMILAR/ESCAPE regex substring. Off (DuckDB/MySQL), `SIMILAR` after
    // the first operand is a parse error through the plain-call fallback.
    LabeledCase {
        sql: "SELECT SUBSTRING('abcdef' SIMILAR 'a' ESCAPE '#')",
        expect: Expect::Accept,
        required: &[&SUBSTRING_SIMILAR],
        forbidden: &[],
    },
    // MySQL's grammar-level substring arity floor: a 1-argument plain call is
    // ER_PARSE_ERROR there, while PostgreSQL parse-accepts any arity (catalog concern).
    // A restricting flag, so the driver is a reject case.
    LabeledCase {
        sql: "SELECT SUBSTRING('abcdef')",
        expect: Expect::Reject,
        required: &[&SUBSTRING_PLAIN_CALL_REQUIRES_2_OR_3_ARGS],
        forbidden: &[],
    },
    // MySQL's SUBSTR keyword-grammar synonym. Off (PostgreSQL/DuckDB — `substr` is an
    // ordinary catalog function there), the inner `FROM` is a parse error.
    LabeledCase {
        sql: "SELECT SUBSTR('abcdef' FROM 2 FOR 3)",
        expect: Expect::Accept,
        required: &[&SUBSTR_FROM_FOR],
        forbidden: &[],
    },
    // POSITION keyword form. Off (SQLite), `position` is an ordinary call name and the
    // inner `IN` is a parse error; on, there is no comma fallback (every keyword-form
    // engine parse-rejects `position(a, b)`).
    LabeledCase {
        sql: "SELECT POSITION('b' IN 'abc')",
        expect: Expect::Accept,
        required: &[&POSITION_IN],
        forbidden: &[],
    },
    // MySQL's asymmetric POSITION operands: the haystack widens to a full expression
    // (`… IN 2 OR 3`), which the standard symmetric b_expr grammar rejects.
    LabeledCase {
        sql: "SELECT POSITION(1 IN 2 OR 3)",
        expect: Expect::Accept,
        required: &[&POSITION_ASYMMETRIC_OPERANDS],
        forbidden: &[],
    },
    // OVERLAY keyword form. Off (MySQL/SQLite), `PLACING` after the first operand is a
    // parse error through the plain-call path.
    LabeledCase {
        sql: "SELECT OVERLAY('abc' PLACING 'X' FROM 2 FOR 1)",
        expect: Expect::Accept,
        required: &[&OVERLAY_PLACING],
        forbidden: &[],
    },
    // DuckDB's PLACING-only OVERLAY grammar: the comma plain call is a parser error
    // there, while PostgreSQL parse-accepts it. A restricting flag, so the driver is a
    // reject case.
    LabeledCase {
        sql: "SELECT OVERLAY('abc', 'X', 2, 1)",
        expect: Expect::Reject,
        required: &[&OVERLAY_REQUIRES_PLACING],
        forbidden: &[],
    },
    // TRIM keyword form. Off (SQLite), `BOTH` inside the parens is a parse error.
    LabeledCase {
        sql: "SELECT TRIM(BOTH 'x' FROM 'xxabc')",
        expect: Expect::Accept,
        required: &[&TRIM_FROM],
        forbidden: &[],
    },
    // PostgreSQL's loose trim_list tails: a side without FROM. Off (MySQL/ANSI), the
    // restricted grammar requires `FROM` after the side's operand, a parse error here.
    LabeledCase {
        sql: "SELECT TRIM(TRAILING ' foo ')",
        expect: Expect::Accept,
        required: &[&TRIM_LIST_SYNTAX],
        forbidden: &[],
    },
    // `CEIL`'s rounding-field keyword form — sqlparser-rs-parity surface only (no probed
    // oracle grammar admits it). Off, `TO` after the first operand is a parse error
    // through the plain-call path.
    LabeledCase {
        sql: "SELECT CEIL(x TO DAY)",
        expect: Expect::Accept,
        required: &[&CEIL_TO_FIELD],
        forbidden: &[],
    },
    // `FLOOR`'s rounding-field keyword form — sqlparser-rs-parity surface only (no probed
    // oracle grammar admits it). Off, `TO` after the first operand is a parse error
    // through the plain-call path.
    LabeledCase {
        sql: "SELECT FLOOR(x TO DAY)",
        expect: Expect::Accept,
        required: &[&FLOOR_TO_FIELD],
        forbidden: &[],
    },
    // Restricted CAST targets. On (MySQL), the target is the narrow `cast_type` set, so
    // `CAST(1 AS INT)` is a syntax error (INT is valid only as a column type); off, every
    // other dialect accepts it. A restricting flag, so the driver is a reject case.
    LabeledCase {
        sql: "SELECT CAST(1 AS INT)",
        expect: Expect::Reject,
        required: &[&RESTRICTED_CAST_TARGETS],
        forbidden: &[],
    },
    // Quantified subquery comparison. Off, `ANY` is not read as a quantifier, so
    // `ANY (SELECT 1)` re-reads as a call whose bare-subquery argument is a parse error.
    LabeledCase {
        sql: "SELECT x FROM t WHERE x = ANY (SELECT 1)",
        expect: Expect::Accept,
        required: &[&QUANTIFIED_COMPARISONS],
        forbidden: &[],
    },
    // `EXTRACT(field FROM source)`. Off, `EXTRACT` is an ordinary function name and the
    // `FROM` inside its parentheses is a parse error.
    LabeledCase {
        sql: "SELECT EXTRACT(YEAR FROM d)",
        expect: Expect::Accept,
        required: &[&EXTRACT_FROM_SYNTAX],
        forbidden: &[],
    },
    // `DELETE ... USING`. Off, `USING` is left unconsumed and the trailing relation list
    // is a parse error.
    LabeledCase {
        sql: "DELETE FROM t USING s WHERE t.a = s.a",
        expect: Expect::Accept,
        required: &[&DELETE_USING],
        forbidden: &[],
    },
    // `CREATE SCHEMA`. Off, the `SCHEMA` keyword is not dispatched and falls to the
    // `TABLE` expectation — an unknown statement.
    LabeledCase {
        sql: "CREATE SCHEMA s",
        expect: Expect::Accept,
        required: &[&SCHEMAS],
        forbidden: &[],
    },
    // `CREATE DATABASE`. Off, `DATABASE` is not dispatched — an unknown statement.
    LabeledCase {
        sql: "CREATE DATABASE d",
        expect: Expect::Accept,
        required: &[&DATABASES],
        forbidden: &[],
    },
    // `CREATE MATERIALIZED VIEW`. Off, `MATERIALIZED` is not dispatched — an unknown
    // statement (the plain `CREATE VIEW` family is unaffected).
    LabeledCase {
        sql: "CREATE MATERIALIZED VIEW v AS SELECT 1",
        expect: Expect::Accept,
        required: &[&MATERIALIZED_VIEWS],
        forbidden: &[],
    },
    // Stored-routine DDL. Off, `FUNCTION` is not a drop-object kind — a parse error.
    LabeledCase {
        sql: "DROP FUNCTION f",
        expect: Expect::Accept,
        required: &[&ROUTINES],
        forbidden: &[],
    },
    // The SQL-standard `RETURN <expr>` routine body (`opt_routine_body`) rides the same
    // routines gate — no dedicated flag. Off, `CREATE FUNCTION` never dispatches, so the whole
    // statement is a parse error.
    LabeledCase {
        sql: "CREATE FUNCTION f() RETURNS INT RETURN 1",
        expect: Expect::Accept,
        required: &[&ROUTINES],
        forbidden: &[],
    },
    // The MySQL stored-program surface (`compound_statements`): a `CREATE PROCEDURE` with a
    // `BEGIN … END` routine body accepts under the flag and, off it, `PROCEDURE` falls through
    // to the `CREATE TABLE` expectation — an accept/reject flip the flag alone drives. This is
    // the top-level toggle that promoted `compound_statements` from a body-context-only gate.
    LabeledCase {
        sql: "CREATE PROCEDURE p() BEGIN END",
        expect: Expect::Accept,
        required: &[&COMPOUND_STATEMENTS],
        forbidden: &[],
    },
    // `CREATE OR REPLACE`. Off, the `OR` after `CREATE` is left unconsumed — a parse error.
    LabeledCase {
        sql: "CREATE OR REPLACE VIEW v AS SELECT 1",
        expect: Expect::Accept,
        required: &[&OR_REPLACE],
        forbidden: &[],
    },
    // Session `SET`. Off, `SET` is not dispatched — an unknown statement.
    LabeledCase {
        sql: "SET x = 1",
        expect: Expect::Accept,
        required: &[&SESSION_STATEMENTS],
        forbidden: &[],
    },
    // Typed `SHOW TABLES` (MySQL/DuckDB). A *structural* case, not accept/reject: with
    // `show_tables` off, `SHOW TABLES` still parses — as a generic session `SHOW` — so the
    // flag flips the shape (typed `Statement::Show` <-> `Statement::Session`), not the
    // accept/reject outcome (`session_statements` stays on in the POSTGRES baseline).
    LabeledCase {
        sql: "SHOW TABLES",
        expect: Expect::Shape(statement_is_typed_show),
        required: &[&SHOW_TABLES],
        forbidden: &[],
    },
    // Typed `SHOW COLUMNS` (MySQL). Unlike `SHOW TABLES`, this is a genuine accept/reject
    // flip: the mandatory `{FROM | IN} <tbl>` qualifier means `SHOW COLUMNS FROM t` cannot
    // parse as a generic session `SHOW <var>` (the trailing `FROM t` is leftover), so with
    // `show_columns` off it is a parse error, and the flag is genuinely required.
    LabeledCase {
        sql: "SHOW COLUMNS FROM t",
        expect: Expect::Accept,
        required: &[&SHOW_COLUMNS],
        forbidden: &[],
    },
    // Typed `SHOW CREATE TABLE` (MySQL). Like `SHOW COLUMNS`, a genuine accept/reject flip:
    // the two fixed `CREATE TABLE` keywords plus the table operand mean `SHOW CREATE TABLE t`
    // cannot parse as a generic session `SHOW <var>` (the trailing `TABLE t` is leftover),
    // so with `show_create_table` off it is a parse error and the flag is genuinely required.
    LabeledCase {
        sql: "SHOW CREATE TABLE t",
        expect: Expect::Accept,
        required: &[&SHOW_CREATE_TABLE],
        forbidden: &[],
    },
    // Typed `SHOW FUNCTIONS` (Spark/Databricks). A genuine accept/reject flip: with
    // `show_functions` off, the generic session `SHOW <var>` reads `FUNCTIONS` as the
    // variable name and the trailing `LIKE 't*'` is leftover, so it is a parse error — the
    // flag is genuinely required to reach the typed function listing.
    LabeledCase {
        sql: "SHOW FUNCTIONS LIKE 't*'",
        expect: Expect::Accept,
        required: &[&SHOW_FUNCTIONS],
        forbidden: &[],
    },
    // Typed `SHOW FUNCTION STATUS` (MySQL). Like `SHOW CREATE TABLE`, a genuine accept/reject
    // flip: `FUNCTION` is a reserved keyword, so `SHOW FUNCTION STATUS` cannot parse as a
    // generic session `SHOW <var>` (the reserved word cannot name a variable), so with
    // `show_routine_status` off it is a parse error and the flag is genuinely required.
    // Distinct from `show_functions`: singular `FUNCTION`/`PROCEDURE`, not the plural
    // `FUNCTIONS`.
    LabeledCase {
        sql: "SHOW FUNCTION STATUS LIKE 'a%'",
        expect: Expect::Accept,
        required: &[&SHOW_ROUTINE_STATUS],
        forbidden: &[],
    },
    // The planner `VERBOSE` tail on the generic session `SHOW` (sqlparser-rs/DataFusion).
    // A genuine accept/reject flip: with `show_verbose` off, the trailing `VERBOSE` is
    // left unconsumed after `SHOW ALL` — a parse error — so the flag is genuinely required
    // to reach the verbose reading. No shipped oracle accepts it (pg_query/DuckDB both
    // reject `SHOW ALL VERBOSE`), so it is on for the permissive superset only.
    LabeledCase {
        sql: "SHOW ALL VERBOSE",
        expect: Expect::Accept,
        required: &[&SHOW_VERBOSE],
        forbidden: &[],
    },
    // The MySQL server-administration / catalogue `SHOW` family. A genuine accept/reject
    // flip: with `show_admin` off, the generic session `SHOW <var>` reads `STORAGE` as the
    // variable name and the trailing `ENGINES` is leftover — a parse error — so the flag is
    // genuinely required to reach the typed listing. (`session_statements` stays on in the
    // POSTGRES baseline, so a bare single-keyword form like `SHOW PLUGINS` would *not* flip.)
    LabeledCase {
        sql: "SHOW STORAGE ENGINES",
        expect: Expect::Accept,
        required: &[&SHOW_ADMIN],
        forbidden: &[],
    },
    // `GRANT`. Off, `GRANT` is not dispatched — an unknown statement.
    LabeledCase {
        sql: "GRANT SELECT ON t TO u",
        expect: Expect::Accept,
        required: &[&ACCESS_CONTROL],
        forbidden: &[],
    },
    // DuckDB `USE <catalog>`. A leading keyword gated like `PRAGMA`: off, the leading
    // `USE` is not dispatched and surfaces as an unknown statement -> reject, so the flag
    // is genuinely required.
    LabeledCase {
        sql: "USE s1",
        expect: Expect::Accept,
        required: &[&USE_STATEMENT],
        forbidden: &[],
    },
    // `GENERATED ... AS IDENTITY`. Off, `IDENTITY` is left unconsumed after `AS` — a parse
    // error (the `GENERATED ALWAYS AS (<expr>)` computed column is a separate family).
    LabeledCase {
        sql: "CREATE TABLE t (x INT GENERATED ALWAYS AS IDENTITY)",
        expect: Expect::Accept,
        required: &[&IDENTITY_COLUMNS],
        forbidden: &[],
    },
    // `WITH (...)` storage parameters. Off, `WITH` is not read as a table option — a parse
    // error.
    LabeledCase {
        sql: "CREATE TABLE t (a INT) WITH (fillfactor = 70)",
        expect: Expect::Accept,
        required: &[&STORAGE_PARAMETERS],
        forbidden: &[],
    },
    // `ON COMMIT` action. Off, `ON` is left unconsumed as a table option — a parse error.
    LabeledCase {
        sql: "CREATE TABLE t (a INT) ON COMMIT PRESERVE ROWS",
        expect: Expect::Accept,
        required: &[&ON_COMMIT],
        forbidden: &[],
    },
    // Extended `ALTER TABLE`. Off, the `ALTER COLUMN` action is not dispatched — a parse
    // error (SQLite's narrow ALTER admits no `ALTER COLUMN`).
    LabeledCase {
        sql: "ALTER TABLE t ALTER COLUMN a SET DEFAULT 1",
        expect: Expect::Accept,
        required: &[&ALTER_TABLE_EXTENDED],
        forbidden: &[],
    },
    // Bare leading `OFFSET`. Off, an `OFFSET` with no preceding `LIMIT` is left unconsumed
    // — a parse error.
    LabeledCase {
        sql: "SELECT 1 OFFSET 3",
        expect: Expect::Accept,
        required: &[&LEADING_OFFSET],
        forbidden: &[],
    },
    // Parenthesized compound operand. Off (SQLite), a leading `(` in operand position is a
    // parse error (statement position has no grouping context to admit it).
    LabeledCase {
        sql: "(SELECT 1) UNION (SELECT 2)",
        expect: Expect::Accept,
        required: &[&PARENTHESIZED_QUERY_OPERANDS],
        forbidden: &[],
    },
    // Stacked join qualifiers. Off (SQLite), the right operand is not extended and the
    // second `ON` is left unconsumed — a parse error.
    LabeledCase {
        sql: "SELECT 1 FROM a JOIN b JOIN c ON b.id = c.id ON a.id = b.id",
        expect: Expect::Accept,
        required: &[&STACKED_JOIN_QUALIFIERS],
        forbidden: &[],
    },
    // `FULL [OUTER] JOIN`. Off (MySQL), the `FULL` join-side keyword is not consumed and the
    // `FULL OUTER JOIN …` tail is left unconsumed — a parse error.
    LabeledCase {
        sql: "SELECT 1 FROM a FULL OUTER JOIN b ON a.x = b.x",
        expect: Expect::Accept,
        required: &[&FULL_OUTER_JOIN],
        forbidden: &[],
    },
    // `CREATE TEMPORARY VIEW`. Off (MySQL), the consumed `TEMPORARY` prefix leading into
    // `VIEW` is a parse error (MySQL has temporary tables but no temporary views).
    LabeledCase {
        sql: "CREATE TEMPORARY VIEW v AS SELECT 1",
        expect: Expect::Accept,
        required: &[&TEMPORARY_VIEWS],
        forbidden: &[],
    },
    // Expression `LIMIT`/`OFFSET` operand. Off (MySQL), a non-literal count (`1 + 1`) is
    // rejected — MySQL admits only an integer literal or a `?` placeholder there.
    LabeledCase {
        sql: "SELECT 1 LIMIT 1 + 1",
        expect: Expect::Accept,
        required: &[&LIMIT_EXPRESSIONS],
        forbidden: &[],
    },
    // Aggregate-only argument behind a spaced paren. On (MySQL), a space before the `(`
    // demotes the built-in aggregate to a general call where the `*` wildcard argument is a
    // syntax error; off, every other dialect accepts `COUNT ( * )`. A restricting flag, so
    // the driver is a reject case (like `restricted_cast_targets`).
    LabeledCase {
        sql: "SELECT COUNT ( * )",
        expect: Expect::Reject,
        required: &[&AGGREGATE_ARGS_REQUIRE_ADJACENT_PAREN],
        forbidden: &[],
    },
    // `CREATE INDEX IF NOT EXISTS`. Off (MySQL), the guard is left unconsumed and the
    // following index name surfaces as a parse error.
    LabeledCase {
        sql: "CREATE INDEX IF NOT EXISTS i ON t (a)",
        expect: Expect::Accept,
        required: &[&INDEX_IF_NOT_EXISTS],
        forbidden: &[],
    },
    // Index-key `NULLS LAST`. Off (MySQL), the `NULLS` keyword is left unconsumed after the
    // index column and surfaces as a parse error.
    LabeledCase {
        sql: "CREATE INDEX i ON t (a NULLS LAST)",
        expect: Expect::Accept,
        required: &[&INDEX_NULLS_ORDER],
        forbidden: &[],
    },
    // Routine argument-type list. Off (MySQL), the `(` after the routine name is left
    // unconsumed and surfaces as a parse error (MySQL identifies a routine by name alone).
    LabeledCase {
        sql: "DROP FUNCTION f(INT)",
        expect: Expect::Accept,
        required: &[&ROUTINE_ARG_TYPES],
        forbidden: &[],
    },
    // Routine parameter default (`func_arg_with_default`). Off (MySQL), the `DEFAULT` after
    // the parameter type is left unconsumed and the parameter-list close surfaces it as a
    // parse error (MySQL routine args carry no default).
    LabeledCase {
        sql: "CREATE FUNCTION f(a INT DEFAULT 0) LANGUAGE sql",
        expect: Expect::Accept,
        required: &[&ROUTINE_ARG_DEFAULTS],
        forbidden: &[],
    },
    // Routine parameter argument mode (`arg_class`). Off (MySQL), the leading `IN` mode
    // keyword is left for the name/type parse and, being reserved, surfaces as a parse
    // error (MySQL `CREATE FUNCTION` args carry no mode — that is a stored-procedure form).
    LabeledCase {
        sql: "CREATE FUNCTION f(IN a INT) LANGUAGE sql",
        expect: Expect::Accept,
        required: &[&ROUTINE_ARG_MODES],
        forbidden: &[],
    },
    // DuckDB percentage `LIMIT` (`LIMIT 40 PERCENT`). Off, the trailing `PERCENT` keyword
    // is left unconsumed after the `40` count and surfaces as a trailing-input parse error
    // — the reject side of the gate.
    LabeledCase {
        sql: "SELECT 1 LIMIT 40 PERCENT",
        expect: Expect::Accept,
        required: &[&LIMIT_PERCENT],
        forbidden: &[],
    },
];

/// Whether the first GROUP BY item of `parsed` is a `ROLLUP` grouping construct — the
/// shape the `grouping_sets` gate produces. With the gate off, `rollup (a, b)`
/// re-parses as an ordinary function-call expression, so this returns false: the
/// falsely-required flip that proves the mis-parse returns without the gate.
fn group_by_is_rollup(parsed: &Parsed) -> bool {
    let Some(Statement::Query { query, .. }) = parsed.statements().first() else {
        return false;
    };
    let SetExpr::Select { select, .. } = &query.body else {
        return false;
    };
    matches!(select.group_by.first(), Some(GroupByItem::Rollup { .. }))
}

/// Whether the sole projection of `parsed` is a `||` string-concatenation — the
/// structural shape the `pipe_operator` forbidden case requires at its baseline.
pub(crate) fn pipe_is_string_concat(parsed: &Parsed) -> bool {
    matches!(
        sole_projection_expr(parsed),
        Expr::BinaryOp {
            op: BinaryOperator::StringConcat,
            ..
        }
    )
}

/// Whether the sole projection is a single unaliased string/bit *constant*
/// (`Expr::Literal`) — the baseline shape of a string-prefix marker
/// (`E'x'`/`N'x'`/`B'1010'`/`_utf8'x'`). With the marker's lexer feature off the marker
/// is an ordinary identifier and `marker'x'` re-reads as the generalized typed literal
/// `marker 'x'` — an `Expr::Cast`, not a literal (prod-literal-generic-typed) — so
/// flipping the feature changes this shape rather than accept/reject, like the
/// `double_quoted_strings` and `pipe_operator` structural cases.
///
/// Total by design: a projection that still parses but takes a *different* shape is
/// `false`, never a panic. The `_latin1"x"` charset-introducer case needs this — with
/// `double_quoted_strings` off, `"x"` is a quoted-identifier *alias* on the column
/// `_latin1`, an aliased projection that is legal SQL but not a string constant, exactly
/// the "still parses, changed shape" a structural `Expect::Shape` flip asserts.
/// True when the statement is the typed `SHOW TABLES` node rather than a generic session
/// `SHOW`. With `show_tables` off, `SHOW TABLES` still *parses* (as
/// [`Statement::Session`]) but takes this other shape — the structural flip that gives the
/// `show_tables` flag a falsely-required check, since the accept/reject outcome does not
/// move (both shapes parse).
fn statement_is_typed_show(parsed: &Parsed) -> bool {
    matches!(parsed.statements(), [Statement::Show { .. }])
}

fn projection_is_a_string_constant(parsed: &Parsed) -> bool {
    let [Statement::Query { query, .. }] = parsed.statements() else {
        return false;
    };
    let SetExpr::Select { select, .. } = &query.body else {
        return false;
    };
    matches!(
        select.projection.as_slice(),
        [SelectItem::Expr {
            expr: Expr::Literal { .. },
            alias: None,
            ..
        }]
    )
}

/// Whether the sole projection is a DuckDB single-arrow lambda (`Expr::Lambda`) — the
/// shape `lambda_expressions` produces for a parameter-shaped `->` left operand. With
/// the flag off the identical text parses as the inherited JSON-arrow `JsonGet` fold,
/// so the SQL still parses but this returns false — the "still parses, changed shape"
/// structural flip, like the `pipe_operator` case.
fn projection_is_a_lambda(parsed: &Parsed) -> bool {
    let SetExpr::Select { select, .. } = query_body(parsed) else {
        return false;
    };
    matches!(
        select.projection.as_slice(),
        [SelectItem::Expr {
            expr: Expr::Lambda { .. },
            ..
        }]
    )
}

/// Whether the sole projection is the `COLUMNS(...)` star expression
/// (`Expr::Columns`) — the shape `columns_expression` produces. With the gate off,
/// the same text re-parses as an ordinary call to a function named `columns`, so
/// this returns false: the falsely-required flip that proves the mis-parse returns
/// without the gate.
fn projection_is_a_columns_selector(parsed: &Parsed) -> bool {
    let SetExpr::Select { select, .. } = query_body(parsed) else {
        return false;
    };
    matches!(
        select.projection.as_slice(),
        [SelectItem::Expr {
            expr: Expr::Columns { .. },
            ..
        }]
    )
}

/// Whether the sole projection is a SQL special value function (`Expr::SpecialFunction`)
/// — the shape MySQL's `UTC_DATE`/`UTC_TIME`/`UTC_TIMESTAMP` take under
/// `utc_special_functions`. With the flag off the keyword is an ordinary (non-reserved)
/// identifier the projection reads as a column reference, so the SQL still parses but this
/// shape check fails — a structural flip, not accept/reject, like the string-marker cases.
fn projection_is_a_special_function(parsed: &Parsed) -> bool {
    let SetExpr::Select { select, .. } = query_body(parsed) else {
        return false;
    };
    matches!(
        select.projection.as_slice(),
        [SelectItem::Expr {
            expr: Expr::SpecialFunction { .. },
            ..
        }]
    )
}

/// Whether the `SELECT` carries the MySQL `STRAIGHT_JOIN` modifier. With
/// `straight_join` off, `STRAIGHT_JOIN` is an ordinary (non-reserved) word the
/// projection grammar reads as a column reference aliased by the following name, so
/// the SQL still parses but this flag is unset — flipping the feature changes the
/// shape rather than accept/reject, like the other structural cases.
fn select_has_straight_join_modifier(parsed: &Parsed) -> bool {
    let SetExpr::Select { select, .. } = query_body(parsed) else {
        return false;
    };
    select.straight_join
}

/// Whether the first FROM relation carries MySQL partition selection. With
/// `partition_selection` off, `PARTITION` is an ordinary (non-reserved) word the table
/// factor reads as its alias with a `(p0)` derived-column list, so the SQL still parses
/// but this field stays empty — flipping the feature changes the shape, not accept/reject.
fn from_relation_has_partition_selection(parsed: &Parsed) -> bool {
    let SetExpr::Select { select, .. } = query_body(parsed) else {
        return false;
    };
    matches!(
        select.from.first().map(|table| &table.relation),
        Some(TableFactor::Table { partition, .. }) if !partition.is_empty(),
    )
}

/// Whether the first FROM relation is the first-class `UNNEST` factor. With `unnest`
/// off, `FROM unnest(…)` falls to the generic table-function path and parses as a
/// [`TableFactor::Function`] — the SQL still accepts (PostgreSQL keeps `table_functions`
/// on), so flipping the feature changes the shape, not accept/reject.
fn from_relation_is_unnest(parsed: &Parsed) -> bool {
    let SetExpr::Select { select, .. } = query_body(parsed) else {
        return false;
    };
    matches!(
        select.from.first().map(|table| &table.relation),
        Some(TableFactor::Unnest { .. }),
    )
}

/// Whether the sole projection casts to the *built-in* `TINYINT` type. With
/// `extended_scalar_type_names` off, `TINYINT` is an unreserved word that the cast target
/// reads as a [`DataType::UserDefined`] name instead — the SQL still parses, but this
/// shape check fails, which is what makes the flag genuinely required.
fn cast_target_is_builtin_tinyint(parsed: &Parsed) -> bool {
    let Expr::Cast { data_type, .. } = sole_projection_expr(parsed) else {
        return false;
    };
    matches!(**data_type, DataType::TinyInt { .. })
}

/// Whether turning the sub-flag `name` off genuinely changes how `sql` parses under
/// PostgreSQL — either it stops accepting (an accept/reject flip) or it still parses
/// but with a different tree (a *shape* feature, e.g. a string-prefix marker re-read as
/// the generalized typed literal `marker 'string'`, prod-literal-generic-typed). This
/// is the differential mirror of [`Expect`]'s accept/reject-or-shape genuineness, so a
/// label that changes neither acceptance nor shape is caught as decoration. A baseline
/// (all-features-on) reject returns `false` — that is a separate bug the caller asserts.
pub(crate) fn feature_flip_changes_parse(sql: &str, name: &str) -> bool {
    let baseline = FeatureSet::POSTGRES;
    // Turning a base flag off can leave an inert dependency dangler; clear it so the parse
    // honours the parser's self-consistency precondition (the flip is off-only, so it never
    // introduces a lexical/grammar conflict — those need a second enabled claimant).
    let flipped = feature_set_with(name, &baseline, false).without_dangling_dependents();
    match (
        parse_with(sql, squonk::ParseConfig::new(AdHocDialect(&baseline))),
        parse_with(sql, squonk::ParseConfig::new(AdHocDialect(&flipped))),
    ) {
        (Ok(_), Err(_)) => true,
        (Ok(on), Ok(off)) => !crate::shared_interner::compare_statements_with_shared_symbols(
            on.statements(),
            on.resolver(),
            off.statements(),
            off.resolver(),
        )
        .structurally_equal(),
        (Err(_), _) => false,
    }
}

/// Resolve a gated sub-flag label to its executable toggle, or `None` for an unknown
/// id. The label vocabulary is the `FeatureSet` sub-flag ids (the ZetaSQL
/// `[required_features=...]` style), so a corpus in another module can declare labels
/// as strings and resolve them against the toggles the coverage cases already use.
fn toggleable_feature(sub_flag: &str) -> Option<&'static ToggleableFeature> {
    TOGGLEABLE_FEATURES
        .iter()
        .copied()
        .find(|feature| feature.sub_flag == sub_flag)
}

/// Whether `features` satisfies a `required_features` label set: every named sub-flag
/// resolves and is enabled. Panics on an unknown id — a label typo is a test bug, not
/// a silent skip.
pub(crate) fn required_features_satisfied(required: &[&str], features: &FeatureSet) -> bool {
    required.iter().all(|name| {
        let feature = toggleable_feature(name)
            .unwrap_or_else(|| panic!("unknown required-feature label `{name}`"));
        (feature.is_enabled)(features)
    })
}

/// `features` with the named sub-flag forced on or off — the flip a corpus uses to
/// prove a `required_features` label is genuine (turning a required feature off must
/// change the outcome).
pub(crate) fn feature_set_with(sub_flag: &str, features: &FeatureSet, on: bool) -> FeatureSet {
    let feature = toggleable_feature(sub_flag)
        .unwrap_or_else(|| panic!("unknown feature label `{sub_flag}`"));
    (feature.set_enabled)(features, on)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labeled_cases_hold_at_their_baseline() {
        // The positive direction: under a dialect that satisfies the label, the case
        // produces its declared outcome (or, for a structural case, its declared shape).
        for case in LABELED_CASES {
            let baseline = case.baseline();
            assert!(
                case.expect.holds(case.sql, &baseline),
                "baseline outcome for {:?} should hold under its satisfying dialect",
                case.sql,
            );
        }
    }

    #[test]
    fn declared_features_are_genuinely_required() {
        // The falsely-required (and falsely-forbidden) pass: flipping any single
        // declared feature from the baseline must change the outcome. For accept/reject
        // cases that is an accept<->reject flip; for a structural case the parse must
        // stay valid but its shape must change. A label that does not change behaviour
        // is a lie, and this catches it.
        for case in LABELED_CASES {
            let baseline = case.baseline();
            for feature in case.declared() {
                let was_enabled = (feature.is_enabled)(&baseline);
                let flipped = (feature.set_enabled)(&baseline, !was_enabled);
                assert!(
                    case.expect.flip_changes_outcome(case.sql, &flipped),
                    "flipping `{}` should change the outcome for {:?}",
                    feature.sub_flag,
                    case.sql,
                );
            }
        }
    }

    #[test]
    fn baselines_are_self_consistent() {
        // The invariant `flip_changes_outcome` relies on: a case's baseline (the set under
        // which its `expect` holds) is handed to the parser, whose precondition is a
        // self-consistent FeatureSet. A *lexical* or *grammar* conflict is a soundness
        // hazard with no defined parse, so no baseline may carry one — a case exercising a
        // feature that is a registered lexical/grammar rival of a baseline-on feature must
        // `forbid` that rival (e.g. the `access_control_account_grants` case forbids its
        // `access_control_extended_objects` rival). A *dependency* dangler is benign (inert)
        // and `holds`/`flip_changes_outcome` normalize it away, so it is allowed here; only
        // the two soundness registries gate a baseline.
        for case in LABELED_CASES {
            let baseline = case.baseline();
            assert_eq!(
                baseline.lexical_conflict(),
                None,
                "baseline for {:?} carries a lexical conflict",
                case.sql,
            );
            assert_eq!(
                baseline.grammar_conflict(),
                None,
                "baseline for {:?} carries a grammar conflict; forbid the registered rival",
                case.sql,
            );
        }
    }

    #[test]
    fn double_quoted_strings_flips_string_vs_quoted_ident() {
        // The structural half of the `double_quoted_strings` coupling
        // (prod-sql-quoted-identifiers): in bare-expression position `SELECT "x"`
        // parses under both settings, so the flag's effect there is the *node kind*,
        // not accept/reject. On, `"x"` is a string literal; off, it is a quoted
        // column reference (the accept/reject flip lives in table-name position, the
        // `SELECT * FROM "x"` labelled case above).
        let on = (DOUBLE_QUOTED_STRINGS.set_enabled)(&FeatureSet::POSTGRES, true);
        let parsed = parse_with("SELECT \"x\"", squonk::ParseConfig::new(AdHocDialect(&on)))
            .expect("`\"x\"` parses with double_quoted_strings on");
        assert!(
            matches!(sole_projection_expr(&parsed), Expr::Literal { .. }),
            "double_quoted_strings on makes `\"x\"` a string literal",
        );

        let off = (DOUBLE_QUOTED_STRINGS.set_enabled)(&FeatureSet::POSTGRES, false);
        let parsed = parse_with("SELECT \"x\"", squonk::ParseConfig::new(AdHocDialect(&off)))
            .expect("`\"x\"` parses with double_quoted_strings off");
        assert!(
            matches!(sole_projection_expr(&parsed), Expr::Column { .. }),
            "double_quoted_strings off makes `\"x\"` a quoted column reference",
        );
    }

    #[test]
    fn partial_dialect_skips_unsatisfiable_cases_without_failing() {
        // ANSI enables `parenthesized_joins` and `table_alias_column_lists` but no
        // other gated sub-flag, so the labelled suite *runs* the cases ANSI satisfies
        // and *skips* the PostgreSQL-only ones rather than failing — the ZetaSQL
        // skip-vs-fail property for a partial dialect.
        let ansi = FeatureSet::ANSI;
        let mut applied = 0;
        let mut skipped = 0;
        for case in LABELED_CASES {
            if case.applies_to(&ansi) {
                applied += 1;
                assert!(
                    case.expect.holds(case.sql, &ansi),
                    "a case ANSI satisfies must hold under ANSI: {:?}",
                    case.sql,
                );
            } else {
                skipped += 1;
            }
        }
        assert!(
            applied > 0,
            "ANSI should satisfy at least one labelled case"
        );
        assert!(
            skipped > 0,
            "ANSI should skip the PostgreSQL-only labelled cases",
        );
    }

    #[test]
    fn labels_resolve_in_the_feature_registry() {
        // Machine-verify every label against the dialect-data registry: a catalogued
        // sub-flag resolves to its STANDARD_FEATURE_CATALOG row (realized by the same
        // knob), and every sub_flag is an enumerated leaf of its owning feature.
        for feature in TOGGLEABLE_FEATURES {
            if let Some(catalog_id) = feature.catalog_id {
                let row = standard_feature(catalog_id)
                    .unwrap_or_else(|| panic!("label `{catalog_id}` missing from the catalogue"));
                assert_eq!(row.realized_by, Some(feature.feature));
            }
            let (_, sub_flags) = COMPOSITE_SUBFLAGS
                .iter()
                .find(|(knob, _)| *knob == feature.feature)
                .expect("a toggleable feature belongs to a composite knob");
            assert!(
                sub_flags.contains(&feature.sub_flag),
                "`{}` is not an enumerated sub-flag of {}",
                feature.sub_flag,
                feature.feature.id(),
            );
        }
    }

    #[test]
    fn every_gated_subflag_is_required_by_a_labeled_case() {
        // The subflag-granularity guarantee, now expressed over labels: every
        // enumerated sub-flag is a ToggleableFeature that some LabeledCase requires,
        // so a new gated flag cannot ship without an objective accept/reject case.
        for (feature, sub_flags) in COMPOSITE_SUBFLAGS {
            for sub_flag in *sub_flags {
                assert!(
                    TOGGLEABLE_FEATURES
                        .iter()
                        .any(|t| t.feature == *feature && t.sub_flag == *sub_flag),
                    "sub-flag `{}::{sub_flag}` has no ToggleableFeature",
                    feature.id(),
                );
                assert!(
                    LABELED_CASES.iter().any(|case| {
                        case.required
                            .iter()
                            .any(|t| t.feature == *feature && t.sub_flag == *sub_flag)
                    }),
                    "sub-flag `{}::{sub_flag}` is not required by any LabeledCase",
                    feature.id(),
                );
            }
        }
    }

    #[test]
    fn statement_head_ledger_claimants_are_all_toggleable() {
        // Ledger/toggle consistency: every flag the head-contention ledger
        // (`MULTI_CLAIMANT_STATEMENT_HEADS`) names as a claimant is a statement-head gate, so
        // each must carry a flip-verified ToggleableFeature here. A new contended head whose
        // claimant is not yet toggleable
        // fails this, keeping the two tables from drifting apart.
        for head in squonk::ast::dialect::MULTI_CLAIMANT_STATEMENT_HEADS {
            for claimant in head.claimants {
                assert!(
                    TOGGLEABLE_FEATURES.iter().any(|t| t.sub_flag == *claimant),
                    "ledger claimant `{claimant}` has no ToggleableFeature (promote it to a flip-verified toggle)",
                );
            }
        }
    }

    #[test]
    fn forbidden_labels_drive_skip_logic() {
        // No current accept/reject case needs a forbidden label (every gated sub-flag
        // is additive), so exercise the forbidden skip semantics directly: a case that
        // forbids `table_sample` does not apply to a dialect that enables it (POSTGRES)
        // but does apply to one that does not (ANSI).
        let forbids_sample = LabeledCase {
            sql: "SELECT 1",
            expect: Expect::Accept,
            required: &[],
            forbidden: &[&TABLE_SAMPLE],
        };
        assert!(!forbids_sample.applies_to(&FeatureSet::POSTGRES));
        assert!(forbids_sample.applies_to(&FeatureSet::ANSI));
    }

    #[test]
    fn structural_forbidden_pipe_case_has_real_teeth() {
        // The ticket acceptance: at least one *real* `forbidden_features` case exists,
        // verified by the falsely-forbidden flip. `SELECT a || b` forbids
        // `logical_or_pipe`; at its baseline `||` is string concatenation, and enabling
        // the forbidden feature must keep the SQL parsing but change the shape to logical
        // OR — a structural divergence, not an accept/reject flip.
        let case = LABELED_CASES
            .iter()
            .find(|case| case.sql == "SELECT a || b")
            .expect("the structural pipe-operator forbidden case is registered");
        assert!(
            !case.forbidden.is_empty(),
            "the case must put `forbidden` to real use",
        );

        let baseline = case.baseline();
        assert!(
            case.expect.holds(case.sql, &baseline),
            "at its baseline `||` parses as string concatenation",
        );

        let with_or = (LOGICAL_OR_PIPE.set_enabled)(&baseline, true);
        assert!(
            accepts_under(case.sql, &with_or),
            "the falsely-forbidden flip must keep the SQL parseable (a structural, not \
             accept/reject, divergence)",
        );
        assert!(
            case.expect.flip_changes_outcome(case.sql, &with_or),
            "enabling `logical_or_pipe` must change the parse shape away from concat",
        );
    }

    #[test]
    fn feature_subflags_are_explicitly_enumerated() {
        // Destructure the sub-flag structs so adding a field fails to compile here and
        // forces a matching ToggleableFeature + LabeledCase (kept honest by the gates above).
        let StringLiteralSyntax {
            escape_strings: _,
            dollar_quoted_strings: _,
            national_strings: _,
            double_quoted_strings: _,
            backslash_escapes: _,
            unicode_strings: _,
            bit_string_literals: _,
            blob_literals: _,
            charset_introducers: _,
            same_line_adjacent_concat: _,
        } = StringLiteralSyntax::POSTGRES;
        let NumericLiteralSyntax {
            hex_integers: _,
            octal_integers: _,
            binary_integers: _,
            underscore_separators: _,
            radix_leading_underscore: _,
            money_literals: _,
            reject_trailing_junk: _,
        } = NumericLiteralSyntax::POSTGRES;
        let CommentSyntax {
            line_comment_hash: _,
            line_comment_ends_at_carriage_return: _,
            nested_block_comments: _,
            versioned_comments: _,
            unterminated_block_comment_at_eof: _,
        } = CommentSyntax::ANSI;
        let ParameterSyntax {
            positional_dollar: _,
            anonymous_question: _,
            named_colon: _,
            named_at: _,
            named_dollar: _,
            numbered_question: _,
        } = ParameterSyntax::POSTGRES;
        let SessionVariableSyntax {
            user_variables: _,
            system_variables: _,
            variable_assignment: _,
        } = SessionVariableSyntax::MYSQL;
        let IdentifierSyntax {
            dollar_in_identifiers: _,
            string_literal_identifiers: _,
            empty_quoted_identifiers: _,
        } = IdentifierSyntax::POSTGRES;
        let TableExpressionSyntax {
            only: _,
            table_sample: _,
            parenthesized_joins: _,
            table_alias_column_lists: _,
            join_using_alias: _,
            index_hints: _,
            table_hints: _,
            partition_selection: _,
            base_table_alias_column_lists: _,
            string_literal_aliases: _,
            aliased_parenthesized_join: _,
            bare_table_alias_is_bare_label: _,
            table_version: _,
            table_json_path: _,
            indexed_by: _,
        } = TableExpressionSyntax::POSTGRES;
        let JoinSyntax {
            stacked_join_qualifiers: _,
            full_outer_join: _,
            natural_cross_join: _,
            straight_join: _,
            asof_join: _,
            positional_join: _,
            semi_anti_join: _,
            sided_semi_anti_join: _,
            recursive_search_cycle: _,
            recursive_union_rejects_order_limit: _,
            recursive_using_key: _,
            apply_join: _,
        } = JoinSyntax::POSTGRES;
        let TableFactorSyntax {
            lateral: _,
            table_functions: _,
            rows_from: _,
            unnest: _,
            unnest_with_offset: _,
            table_function_ordinality: _,
            pivot: _,
            unpivot: _,
            show_ref: _,
            from_values: _,
            special_function_table_source: _,
            json_table: _,
            xml_table: _,
            table_expr_factor: _,
            pivot_value_sources: _,
            match_recognize: _,
            open_json: _,
        } = TableFactorSyntax::POSTGRES;
        let MutationSyntax {
            insert_ignore: _,
            insert_overwrite: _,
            joined_update_delete: _,
            merge_insert_multirow: _,
            returning: _,
            on_conflict: _,
            on_duplicate_key_update: _,
            multi_column_assignment: _,
            update_tuple_value_row_arity: _,
            where_current_of: _,
            merge: _,
            replace_into: _,
            insert_set: _,
            update_delete_tails: _,
            or_conflict_action: _,
            delete_using: _,
            update_from: _,
            delete_using_target_alias: _,
            cte_before_insert: _,
            cte_before_merge: _,
            data_modifying_ctes: _,
            merge_when_not_matched_by: _,
            merge_insert_default_values: _,
            merge_insert_overriding: _,
            update_set_qualified_column: _,
            merge_update_set_star: _,
            merge_insert_star_by_name: _,
            merge_error_action: _,
            insert_column_matching: _,
        } = MutationSyntax::POSTGRES;
        // Every `StatementDdlGates` field is a toggleable statement-head gate with a
        // flip-verified LabeledCase, so the `_` bindings carry no exemption.
        let StatementDdlGates {
            colocation_groups: _,
            materialized_view_to: _,
            create_trigger: _,
            create_macro: _,
            create_secret: _,
            create_type: _,
            create_virtual_table: _,
            create_sequence: _,
            create_sequence_cache: _,
            extension_ddl: _,
            transform_ddl: _,
            alter_system: _,
            tablespace_ddl: _,
            logfile_group_ddl: _,
            schemas: _,
            schema_elements: _,
            databases: _,
            drop_database: _,
            materialized_views: _,
            temporary_views: _,
            routines: _,
            or_replace: _,
            recursive_views: _,
            compound_statements: _,
            alter_database: _,
            alter_database_options: _,
            server_definition: _,
            alter_instance: _,
            spatial_reference_system: _,
            resource_group: _,
            alter_sequence: _,
            alter_object_set_schema: _,
            view_definition_options: _,
        } = StatementDdlGates::POSTGRES;
        let CreateTableClauseSyntax {
            table_options: _,
            without_rowid_table_option: _,
            strict_table_option: _,
            create_or_replace_table: _,
            storage_parameters: _,
            on_commit: _,
            create_table_as_with_data: _,
            declarative_partitioning: _,
            table_inheritance: _,
            like_source_table: _,
            statement_level_table_like: _,
            unlogged_tables: _,
            table_access_method: _,
            without_oids: _,
            typed_tables: _,
            create_table_as_execute: _,
        } = CreateTableClauseSyntax::POSTGRES;
        let ColumnDefinitionSyntax {
            generated_column_shorthand: _,
            column_conflict_resolution_clause: _,
            typeless_column_definitions: _,
            typeless_generated_columns: _,
            joined_autoincrement_attribute: _,
            inline_primary_key_ordering: _,
            named_column_collate_constraint: _,
            identity_columns: _,
            compact_identity_columns: _,
            default_expression_requires_parens: _,
            column_default_requires_b_expr: _,
            column_collation: _,
            column_storage: _,
        } = ColumnDefinitionSyntax::POSTGRES;
        let ConstraintSyntax {
            deferrable_constraints: _,
            named_inline_non_check_constraints: _,
            bare_constraint_name: _,
            exclusion_constraints: _,
            constraint_no_inherit_not_valid: _,
            index_constraint_parameters: _,
            constraint_column_collate_order: _,
            referential_action_cascade_set: _,
            check_constraint_subqueries: _,
        } = ConstraintSyntax::POSTGRES;
        // `index_drop_on_table` is this struct's statement-head gate (the `DROP … INDEX ON`
        // dispatch), now a flip-verified toggle. The remaining `_` bindings are
        // `CREATE INDEX` / `ALTER TABLE` clause decorations, outside the statement-head pass.
        let IndexAlterSyntax {
            rename_constraint: _,
            alter_table_set_options: _,
            drop_primary_key: _,
            alter_column_add_identity: _,
            index_storage_parameters: _,
            drop_behavior: _,
            index_drop_on_table: _,
            index_concurrently: _,
            index_using_method: _,
            partial_index: _,
            alter_table_extended: _,
            index_if_not_exists: _,
            index_nulls_order: _,
            routine_arg_types: _,
            routine_arg_defaults: _,
            routine_arg_modes: _,
            routine_language_string: _,
            alter_existence_guards: _,
            alter_nested_column_paths: _,
            alter_column_set_data_type: _,
            alter_table_multiple_actions: _,
        } = IndexAlterSyntax::POSTGRES;
        let ExistenceGuards {
            if_exists: _,
            view_if_not_exists: _,
            create_database_if_not_exists: _,
        } = ExistenceGuards::POSTGRES;
        let ExpressionSyntax {
            typecast_operator: _,
            subscript: _,
            slice_step: _,
            collate: _,
            at_time_zone: _,
            semi_structured_access: _,
            array_constructor: _,
            multidim_array_literals: _,
            collection_literals: _,
            row_constructor: _,
            struct_constructor: _,
            field_selection: _,
            field_wildcard: _,
            typed_string_literals: _,
            typed_interval_literal: _,
            relaxed_interval_syntax: _,
            mysql_interval_operator: _,
            positional_column: _,
            lambda_keyword: _,
        } = ExpressionSyntax::POSTGRES;
        let OperatorSyntax {
            operator_construct: _,
            containment_operators: _,
            json_arrow_operators: _,
            jsonb_operators: _,
            double_equals: _,
            integer_divide_slash: _,
            is_general_equality: _,
            truth_value_tests: _,
            null_safe_equals: _,
            lambda_expressions: _,
            bitwise_operators: _,
            quantified_comparisons: _,
            quantified_comparison_lists: _,
            quantified_arbitrary_operator: _,
            custom_operators: _,
            null_test_postfix: _,
            starts_with_operator: _,
            postfix_operators: _,
        } = OperatorSyntax::POSTGRES;
        let CallSyntax {
            named_argument: _,
            utc_special_functions: _,
            columns_expression: _,
            extract_from_syntax: _,
            try_cast: _,
            restricted_cast_targets: _,
            extract_string_field: _,
            method_chaining: _,
            sqljson_constructors_require_argument: _,
            sqljson_expression_functions: _,
            xml_expression_functions: _,
            variadic_argument: _,
            merge_action_function: _,
            convert_function: _,
        } = CallSyntax::POSTGRES;
        let StringFuncForms {
            substring_from_for: _,
            substring_leading_for: _,
            substring_similar: _,
            substring_plain_call_requires_2_or_3_args: _,
            substr_from_for: _,
            position_in: _,
            position_asymmetric_operands: _,
            overlay_placing: _,
            overlay_requires_placing: _,
            trim_from: _,
            trim_list_syntax: _,
            collation_for_expression: _,
            ceil_to_field: _,
            floor_to_field: _,
            match_against: _,
        } = StringFuncForms::POSTGRES;
        let AggregateCallSyntax {
            group_concat_separator: _,
            within_group: _,
            aggregate_filter: _,
            filter_optional_where: _,
            aggregate_args_require_adjacent_paren: _,
            null_treatment: _,
            aggregate_calls_reject_empty_arguments: _,
            over_requires_windowable_function: _,
            window_function_tail: _,
            standalone_argument_order_by: _,
        } = AggregateCallSyntax::POSTGRES;
        let PredicateSyntax {
            is_distinct_from: _,
            like: _,
            ilike: _,
            similar_to: _,
            overlaps_period_predicate: _,
            unparenthesized_in_list: _,
            pattern_match_quantifier: _,
            between_symmetric: _,
            is_normalized: _,
            empty_in_list: _,
            null_test_two_word_postfix: _,
        } = PredicateSyntax::POSTGRES;
        let SelectSyntax {
            wildcard_replace: _,
            intersect_all: _,
            except_all: _,
            distinct_on: _,
            select_into: _,
            empty_target_list: _,
            qualify: _,
            alias_string_literals: _,
            bare_alias_string_literals: _,
            union_by_name: _,
            from_first: _,
            wildcard_modifiers: _,
            qualified_wildcard_alias: _,
            parenthesized_query_operands: _,
            values_rows_require_equal_arity: _,
            values_row_constructor: _,
            as_alias_rejects_reserved: _,
            trailing_comma: _,
            prefix_colon_alias: _,
            lateral_view_clause: _,
            connect_by_clause: _,
        } = SelectSyntax::POSTGRES;
        let QueryTailSyntax {
            fetch_first: _,
            limit_offset_comma: _,
            locking_clauses: _,
            key_lock_strengths: _,
            stacked_locking_clauses: _,
            using_sample: _,
            leading_offset: _,
            limit_expressions: _,
            limit_percent: _,
            with_ties_requires_order_by: _,
            pipe_syntax: _,
            limit_by_clause: _,
            settings_clause: _,
            format_clause: _,
            for_xml_json_clause: _,
        } = QueryTailSyntax::POSTGRES;
        let GroupingSyntax {
            grouping_sets: _,
            with_rollup: _,
            order_by_using: _,
            group_by_all: _,
            group_by_set_quantifier: _,
            order_by_all: _,
        } = GroupingSyntax::POSTGRES;
        let UtilitySyntax {
            comment_if_exists: _,
            transaction_chain: _,
            copy: _,
            copy_into: _,
            comment_on: _,
            pragma: _,
            attach: _,
            kill: _,
            handler_statements: _,
            plugin_component_statements: _,
            shutdown: _,
            restart: _,
            clone: _,
            import_table: _,
            help_statement: _,
            binlog: _,
            key_cache_statements: _,
            use_statement: _,
            use_qualified_name: _,
            prepared_statements: _,
            prepare_typed_parameters: _,
            prepared_statements_from: _,
            call: _,
            call_bare_name: _,
            load_extension: _,
            load_bare_name: _,
            load_data: _,
            reset_scope: _,
            detach_if_exists: _,
            do_statement: _,
            do_expression_list: _,
            lock_tables: _,
            lock_instance: _,
            begin_transaction_mode: _,
            xa_transactions: _,
            rename_statement: _,
            stage_references: _,
            signal_diagnostics: _,
            export_import_database: _,
            update_extensions: _,
            flush: _,
            purge_binary_logs: _,
            replication_statements: _,
        } = UtilitySyntax::POSTGRES;
        let ShowSyntax {
            describe: _,
            describe_summarize: _,
            session_statements: _,
            show_tables: _,
            show_columns: _,
            show_create_table: _,
            show_functions: _,
            show_routine_status: _,
            show_verbose: _,
            show_admin: _,
        } = ShowSyntax::POSTGRES;
        let MaintenanceSyntax {
            vacuum: _,
            vacuum_analyze: _,
            reindex: _,
            analyze: _,
            analyze_columns: _,
            checkpoint: _,
            checkpoint_database: _,
            table_maintenance: _,
        } = MaintenanceSyntax::POSTGRES;
        let AccessControlSyntax {
            alter_role_rename: _,
            access_control: _,
            access_control_extended_objects: _,
            user_role_management: _,
            access_control_account_grants: _,
        } = AccessControlSyntax::POSTGRES;
        let TypeNameSyntax {
            extended_scalar_type_names: _,
            enum_type: _,
            set_type: _,
            numeric_modifiers: _,
            integer_display_width: _,
            composite_types: _,
            varchar_requires_length: _,
            zoned_temporal_types: _,
            empty_type_parens: _,
            character_set_annotation: _,
            signed_type_modifier: _,
            nullable_type: _,
            low_cardinality_type: _,
            fixed_string_type: _,
            datetime64_type: _,
            nested_type: _,
            bit_width_integer_names: _,
            liberal_type_names: _,
            string_type_modifiers: _,
            angle_bracket_types: _,
        } = TypeNameSyntax::MYSQL;

        // Name snapshots replace the hand-bumped count pins. Two branches that each add
        // a flag now land distinct lines that git merges cleanly (a genuine same-slot
        // collision conflicts loudly instead of auto-merging a bogus count), the review
        // shows exactly which flags arrived, and a mismatch names the flag rather than a
        // count delta. The anti-vanishing property the count pins carried survives: a
        // missing snapshot file is a hard insta failure, so neither a silently dropped
        // flag nor a vanished snapshot passes. `subflag_arrays` pins each per-feature
        // sub-flag array; `toggleable_feature_names` pins the flattened toggle set (the
        // two are held equal by `labels_resolve_in_the_feature_registry` and
        // `every_gated_subflag_is_required_by_a_labeled_case`). Regenerate with
        // `cargo insta accept` after an intended change.
        insta::assert_snapshot!("subflag_arrays", render_subflag_arrays());
        insta::assert_snapshot!(
            "toggleable_feature_names",
            render_toggleable_feature_names()
        );
    }

    #[test]
    fn sub_flag_names_are_unique() {
        // Sub-flag strings are the global label vocabulary — `toggleable_feature` and
        // `feature_set_with` resolve a bare name — so a name repeated across features
        // (the classic both-sides-of-a-merge slip) would silently shadow one toggle.
        // Uniqueness across TOGGLEABLE_FEATURES and within the sub-flag arrays is the
        // true hazard the count pins never caught.
        let mut seen = std::collections::BTreeSet::new();
        for t in TOGGLEABLE_FEATURES {
            assert!(
                seen.insert(t.sub_flag),
                "duplicate toggleable sub-flag `{}`",
                t.sub_flag,
            );
        }
        let mut array_seen = std::collections::BTreeSet::new();
        for (feature, sub_flags) in COMPOSITE_SUBFLAGS {
            for &sub_flag in *sub_flags {
                assert!(
                    array_seen.insert(sub_flag),
                    "duplicate sub-flag `{sub_flag}` in {}",
                    feature.id(),
                );
            }
        }
    }

    fn render_toggleable_feature_names() -> String {
        let mut names: Vec<String> = TOGGLEABLE_FEATURES
            .iter()
            .map(|t| format!("{}::{}", t.feature.id(), t.sub_flag))
            .collect();
        names.sort_unstable();
        names.join("\n")
    }

    fn render_subflag_arrays() -> String {
        let mut out = String::new();
        for (feature, sub_flags) in COMPOSITE_SUBFLAGS {
            out.push_str(feature.id());
            out.push('\n');
            for &sub_flag in *sub_flags {
                out.push_str("  ");
                out.push_str(sub_flag);
                out.push('\n');
            }
        }
        out.truncate(out.trim_end().len());
        out
    }
}
