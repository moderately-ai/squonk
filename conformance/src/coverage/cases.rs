// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! ADR-0015 behaviour-coverage cases: the `*_FEATURES` fixtures, [`COVERAGE_CASES`],
//! `render_to_target`, the probe predicates, the metadata/semantic asserts, and the objectivity
//! gate predicate [`has_objective_behavior`].

use super::harness::*;
use super::labeled::*;
use super::*;

const NO_RESERVED_FEATURES: FeatureSet = FeatureSet::ANSI.with(
    FeatureDelta::EMPTY
        .reserved_column_name(KeywordSet::EMPTY)
        .reserved_function_name(KeywordSet::EMPTY)
        .reserved_type_name(KeywordSet::EMPTY)
        .reserved_bare_alias(KeywordSet::EMPTY),
);

const BACKTICK_QUOTE_FEATURES: FeatureSet = FeatureSet::ANSI
    .with(FeatureDelta::EMPTY.identifier_quotes(&[IdentifierQuote::Symmetric('`')]));

const NULLS_FIRST_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.default_null_ordering(NullOrdering::NullsFirst));

const CUSTOM_BYTE_CLASS_FEATURES: FeatureSet = FeatureSet::ANSI.with(
    FeatureDelta::EMPTY.byte_classes(
        FeatureSet::ANSI
            .byte_classes
            .with_class(b'@', CLASS_IDENTIFIER_START | CLASS_IDENTIFIER_CONTINUE),
    ),
);

const CUSTOM_BINDING_POWER_FEATURES: FeatureSet = FeatureSet::ANSI.with(
    FeatureDelta::EMPTY.binding_powers(FeatureSet::ANSI.binding_powers.with_binary(
        &BinaryOperator::StringConcat,
        BindingPower {
            left: 70,
            right: 71,
            assoc: Assoc::Left,
        },
    )),
);

const LEFT_ASSOC_COMPARISON_FEATURES: FeatureSet = FeatureSet::ANSI.with(
    FeatureDelta::EMPTY.binding_powers(FeatureSet::ANSI.binding_powers.with_binary(
        &BinaryOperator::Lt,
        BindingPower {
            left: 40,
            right: 41,
            assoc: Assoc::Left,
        },
    )),
);

const CUSTOM_SET_OPERATION_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.set_operation_powers(
        FeatureSet::ANSI.set_operation_powers.with_set_operator(
            &SetOperator::Union,
            BindingPower {
                left: 30,
                right: 31,
                assoc: Assoc::Left,
            },
        ),
    ));

const LOGICAL_OR_PIPE_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.pipe_operator(PipeOperator::LogicalOr));

const HEX_NUMBER_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.numeric_literals(NumericLiteralSyntax {
        hex_integers: true,
        ..NumericLiteralSyntax::ANSI
    }));

const LOGICAL_AND_AMP_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.double_ampersand(DoubleAmpersand::LogicalAnd));

const MYSQL_KEYWORD_OPERATOR_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.keyword_operators(KeywordOperators::MySql));

// `#` bitwise XOR (PostgreSQL's spelling) over the ANSI baseline: `#` lexes as the operator
// (ANSI has no `#` comment, so no conflict).
const HASH_BITWISE_XOR_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.hash_bitwise_xor(true));
// The two infix readings of `^` over the ANSI baseline: bitwise XOR (MySQL's spelling) and
// arithmetic power (PostgreSQL/DuckDB). `^` always tokenizes, so only its meaning changes.
const CARET_BITWISE_XOR_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.caret_operator(CaretOperator::BitwiseXor));
const CARET_EXPONENT_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.caret_operator(CaretOperator::Exponent));

const HASH_COMMENT_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.comment_syntax(CommentSyntax {
        line_comment_hash: true,
        ..CommentSyntax::ANSI
    }));

/// The full MySQL comment shape (`/*!…*/` conditional inclusion gated at the
/// modelled 8.4 bound, non-nesting block comments, `#` line comments) on the
/// ANSI base, so the versioned-comment cases pin the comment surface in
/// isolation from the rest of the MySQL preset.
const VERSIONED_COMMENT_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.comment_syntax(CommentSyntax::MYSQL));

const POSITIONAL_PARAMETER_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.parameters(ParameterSyntax {
        positional_dollar: true,
        ..ParameterSyntax::ANSI
    }));

const NAMED_COLON_PARAMETER_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.parameters(ParameterSyntax {
        named_colon: true,
        ..ParameterSyntax::ANSI
    }));

const NAMED_AT_PARAMETER_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.parameters(ParameterSyntax {
        named_at: true,
        ..ParameterSyntax::ANSI
    }));

const SESSION_VARIABLE_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.session_variables(SessionVariableSyntax::MYSQL));

const PG_STRING_LITERAL_FEATURES: FeatureSet =
    FeatureSet::ANSI.with(FeatureDelta::EMPTY.string_literals(StringLiteralSyntax::POSTGRES));

const NO_PG_STRING_LITERAL_FEATURES: FeatureSet =
    FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.string_literals(StringLiteralSyntax::ANSI));

const PG_TABLE_EXPRESSION_FEATURES: FeatureSet = FeatureSet::ANSI.with(
    FeatureDelta::EMPTY
        .table_expressions(TableExpressionSyntax::POSTGRES)
        .join_syntax(JoinSyntax::POSTGRES)
        .table_factor_syntax(TableFactorSyntax::POSTGRES),
);

const NO_PG_TABLE_EXPRESSION_FEATURES: FeatureSet = FeatureSet::POSTGRES.with(
    FeatureDelta::EMPTY
        .table_expressions(TableExpressionSyntax::ANSI)
        .join_syntax(JoinSyntax::ANSI)
        .table_factor_syntax(TableFactorSyntax::ANSI),
);

/// The dialect-feature coverage matrix. Each behaviour case lifts its SQL input and the
/// expected accept/reject-or-structural outcome into [`Probe`] data the harness runs and
/// confirms, so a `Behavior` case's kind is derived from an actual `parse_with` /
/// `tokenize_with` / render run — not a hand-set tag (see [`has_objective_behavior`]).
/// The two semantic-default features route through the documented [`Coverage::SemanticDefault`]
/// escape hatch instead, the only residual hand-set trust surface.
pub(crate) const COVERAGE_CASES: &[CoverageCase] = &[
    CoverageCase {
        feature: Feature::IdentifierCasing,
        polarity: Polarity::Positive,
        name: "ansi_identifier_casing_upper",
        coverage: Coverage::Metadata(assert_ansi_identifier_casing_upper),
    },
    CoverageCase {
        feature: Feature::IdentifierCasing,
        polarity: Polarity::Negative,
        name: "postgres_identifier_casing_diverges",
        coverage: Coverage::Metadata(assert_postgres_identifier_casing_diverges),
    },
    CoverageCase {
        feature: Feature::IdentifierQuote,
        polarity: Polarity::Positive,
        name: "ansi_identifier_quote_double",
        coverage: Coverage::Metadata(assert_ansi_identifier_quote_double),
    },
    CoverageCase {
        feature: Feature::IdentifierQuote,
        polarity: Polarity::Negative,
        name: "custom_identifier_quote_diverges",
        coverage: Coverage::Metadata(assert_custom_identifier_quote_diverges),
    },
    CoverageCase {
        feature: Feature::DefaultNullOrdering,
        polarity: Polarity::Positive,
        name: "ansi_default_null_ordering_last",
        coverage: Coverage::Metadata(assert_ansi_default_null_ordering_last),
    },
    CoverageCase {
        feature: Feature::DefaultNullOrdering,
        polarity: Polarity::Negative,
        name: "custom_default_null_ordering_diverges",
        coverage: Coverage::Metadata(assert_custom_default_null_ordering_diverges),
    },
    CoverageCase {
        feature: Feature::ReservedColumnName,
        polarity: Polarity::Positive,
        name: "reserved_keyword_rejects_column_name",
        // JOIN (type_func_name) is not a bare column name (ColId).
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT join",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::ReservedColumnName,
        polarity: Polarity::Negative,
        name: "cleared_column_name_accepts_keyword",
        // Clearing the column-name reject set makes JOIN available as a column.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT join",
            features: &NO_RESERVED_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::ReservedFunctionName,
        polarity: Polarity::Positive,
        name: "reserved_keyword_rejects_function_name",
        // SELECT (reserved) is not a function name.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT select(1)",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::ReservedFunctionName,
        polarity: Polarity::Negative,
        name: "cleared_function_name_accepts_keyword",
        // Clearing the function-name reject set makes SELECT available as a call.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT select(1)",
            features: &NO_RESERVED_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::ReservedTypeName,
        polarity: Polarity::Positive,
        name: "col_name_keyword_rejects_type_name",
        // COALESCE (col_name) is not a type name.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT CAST(a AS coalesce)",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::ReservedTypeName,
        polarity: Polarity::Negative,
        name: "cleared_type_name_accepts_keyword",
        // Clearing the type-name reject set makes COALESCE available as a type.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT CAST(a AS coalesce)",
            features: &NO_RESERVED_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::ReservedBareAlias,
        polarity: Polarity::Positive,
        name: "as_label_keyword_rejects_bare_alias",
        // OVER (AS_LABEL) is not a bare alias.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT a over",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::ReservedBareAlias,
        polarity: Polarity::Negative,
        name: "cleared_bare_alias_accepts_keyword",
        // Clearing the bare-alias reject set makes OVER available as a bare alias.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT a over",
            features: &NO_RESERVED_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::ReservedAsLabel,
        polarity: Polarity::Positive,
        // SQLite's non-empty `reserved_as_label` rejects a reserved word as an AS-label.
        name: "reserved_keyword_rejects_as_label",
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT 1 AS delete",
            features: &FeatureSet::SQLITE,
        }]),
    },
    CoverageCase {
        feature: Feature::ReservedAsLabel,
        polarity: Polarity::Negative,
        // ANSI/PostgreSQL admit every keyword as an AS-label (`reserved_as_label` empty).
        name: "empty_as_label_set_accepts_keyword",
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT 1 AS delete",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::CatalogQualifiedNames,
        polarity: Polarity::Positive,
        // SQLite caps a relation at schema.table; a three-part `a.b.c` is rejected.
        name: "sqlite_relation_name_capped_at_two_parts",
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT * FROM a.b.c",
            features: &FeatureSet::SQLITE,
        }]),
    },
    CoverageCase {
        feature: Feature::CatalogQualifiedNames,
        polarity: Polarity::Negative,
        // Catalog-qualified presets admit the three-part `catalog.schema.table`.
        name: "catalog_qualified_relation_name_accepts_three_parts",
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT * FROM a.b.c",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::ByteClasses,
        polarity: Polarity::Positive,
        name: "ansi_byte_class_marks_operator",
        coverage: Coverage::Metadata(assert_ansi_byte_class_marks_operator),
    },
    CoverageCase {
        feature: Feature::ByteClasses,
        polarity: Polarity::Negative,
        name: "custom_byte_class_accepts_extra_identifier_start",
        coverage: Coverage::Metadata(assert_custom_byte_class_accepts_extra_identifier_start),
    },
    CoverageCase {
        feature: Feature::BindingPowers,
        polarity: Polarity::Positive,
        name: "ansi_binding_power_orders_string_concat",
        coverage: Coverage::Metadata(assert_ansi_binding_power_orders_string_concat),
    },
    CoverageCase {
        feature: Feature::BindingPowers,
        polarity: Polarity::Negative,
        name: "custom_binding_power_diverges",
        coverage: Coverage::Metadata(assert_custom_binding_power_diverges),
    },
    CoverageCase {
        feature: Feature::SetOperationPowers,
        polarity: Polarity::Positive,
        name: "ansi_set_operation_power_ranks_intersect_above_union",
        coverage: Coverage::Metadata(assert_ansi_set_operation_power_ranks_intersect_above_union),
    },
    CoverageCase {
        feature: Feature::SetOperationPowers,
        polarity: Polarity::Negative,
        name: "custom_set_operation_power_diverges",
        coverage: Coverage::Metadata(assert_custom_set_operation_power_diverges),
    },
    CoverageCase {
        feature: Feature::StringLiterals,
        polarity: Polarity::Positive,
        name: "ansi_string_literal_syntax_standard_only",
        coverage: Coverage::Metadata(assert_ansi_string_literal_syntax_standard_only),
    },
    CoverageCase {
        feature: Feature::StringLiterals,
        polarity: Polarity::Negative,
        name: "postgres_string_literal_syntax_diverges",
        coverage: Coverage::Metadata(assert_postgres_string_literal_syntax_diverges),
    },
    CoverageCase {
        feature: Feature::TableExpressions,
        polarity: Polarity::Positive,
        name: "ansi_table_expression_syntax_baseline",
        coverage: Coverage::Metadata(assert_ansi_table_expression_syntax_baseline),
    },
    CoverageCase {
        feature: Feature::TableExpressions,
        polarity: Polarity::Negative,
        name: "postgres_table_expression_syntax_diverges",
        coverage: Coverage::Metadata(assert_postgres_table_expression_syntax_diverges),
    },
    CoverageCase {
        feature: Feature::TableExpressions,
        polarity: Polarity::Positive,
        name: "postgres_table_expressions_accept_only",
        // PostgreSQL's `ONLY <table>` inheritance-suppression factor tail.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT * FROM ONLY t",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::TableExpressions,
        polarity: Polarity::Negative,
        name: "ansi_table_expressions_reject_only",
        // ANSI leaves `only` off, so the `ONLY` keyword is left unconsumed and surfaces
        // as a clean parse error.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT * FROM ONLY t",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::JoinSyntax,
        polarity: Polarity::Positive,
        name: "postgres_join_syntax_accepts_full_outer_join",
        // The `FULL [OUTER] JOIN` bilateral outer join.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT * FROM a FULL OUTER JOIN b ON TRUE",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::JoinSyntax,
        polarity: Polarity::Negative,
        name: "mysql_join_syntax_rejects_full_outer_join",
        // MySQL has no `FULL` join, so the `FULL [OUTER] JOIN` keyword sequence is a
        // clean parse error.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT * FROM a FULL OUTER JOIN b ON TRUE",
            features: &FeatureSet::MYSQL,
        }]),
    },
    CoverageCase {
        feature: Feature::IdentifierCasing,
        polarity: Polarity::Positive,
        name: "ansi_identifier_casing_folds_upper",
        coverage: Coverage::SemanticDefault(assert_ansi_identifier_casing_folds_upper),
    },
    CoverageCase {
        feature: Feature::IdentifierCasing,
        polarity: Polarity::Negative,
        name: "postgres_identifier_casing_folds_lower",
        coverage: Coverage::SemanticDefault(assert_postgres_identifier_casing_folds_lower),
    },
    CoverageCase {
        feature: Feature::IdentifierQuote,
        polarity: Polarity::Positive,
        name: "ansi_identifier_quote_accepts_double_quote",
        // ANSI lexes a double-quoted string as one quoted identifier.
        coverage: Coverage::Behavior(&[Probe::TokenShape {
            sql: "\"Odd\"",
            features: &FeatureSet::ANSI,
            tokens: quoted_ident_singleton,
        }]),
    },
    CoverageCase {
        feature: Feature::IdentifierQuote,
        polarity: Polarity::Negative,
        name: "custom_identifier_quote_accepts_backtick",
        // ANSI does not accept backticks; a backtick-quote dialect lexes `Odd` as one
        // quoted identifier.
        coverage: Coverage::Behavior(&[
            Probe::TokenRejects {
                sql: "`Odd`",
                features: &FeatureSet::ANSI,
            },
            Probe::TokenShape {
                sql: "`Odd`",
                features: &BACKTICK_QUOTE_FEATURES,
                tokens: quoted_ident_singleton,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::DefaultNullOrdering,
        polarity: Polarity::Positive,
        name: "ansi_default_null_ordering_places_nulls_last",
        coverage: Coverage::SemanticDefault(assert_ansi_default_null_ordering_places_nulls_last),
    },
    CoverageCase {
        feature: Feature::DefaultNullOrdering,
        polarity: Polarity::Negative,
        name: "custom_default_null_ordering_places_nulls_first",
        coverage: Coverage::SemanticDefault(assert_custom_default_null_ordering_places_nulls_first),
    },
    CoverageCase {
        feature: Feature::ByteClasses,
        polarity: Polarity::Positive,
        name: "ansi_byte_class_tokenizes_plus_operator",
        // The standard byte classes classify `+` as an operator.
        coverage: Coverage::Behavior(&[Probe::TokenShape {
            sql: "a + b",
            features: &FeatureSet::ANSI,
            tokens: has_plus_operator,
        }]),
    },
    CoverageCase {
        feature: Feature::ByteClasses,
        polarity: Polarity::Negative,
        name: "custom_byte_class_accepts_at_identifier",
        // ANSI has no token beginning with `@`; a custom byte-class dialect lexes `@name`
        // as one word.
        coverage: Coverage::Behavior(&[
            Probe::TokenRejects {
                sql: "@name",
                features: &FeatureSet::ANSI,
            },
            Probe::TokenShape {
                sql: "@name",
                features: &CUSTOM_BYTE_CLASS_FEATURES,
                tokens: word_singleton,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::BindingPowers,
        polarity: Polarity::Positive,
        name: "ansi_binding_power_structures_concat_below_multiply",
        // Multiply binds tighter than concat under ANSI.
        coverage: Coverage::Behavior(&[Probe::ParseShape {
            sql: "SELECT a || b * c",
            features: &FeatureSet::ANSI,
            shape: concat_below_multiply,
        }]),
    },
    CoverageCase {
        feature: Feature::BindingPowers,
        polarity: Polarity::Positive,
        name: "ansi_binding_power_rejects_unparenthesized_comparison_chain",
        // ANSI/PostgreSQL comparison associativity is non-associative; explicit grouping
        // is a parse-time barrier, not an AST node.
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "SELECT a < b < c",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseAccepts {
                sql: "SELECT (a < b) < c",
                features: &FeatureSet::ANSI,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::BindingPowers,
        polarity: Polarity::Negative,
        name: "custom_binding_power_structures_concat_above_multiply",
        // Raising concat above multiply changes the structural parse.
        coverage: Coverage::Behavior(&[Probe::ParseShape {
            sql: "SELECT a || b * c",
            features: &CUSTOM_BINDING_POWER_FEATURES,
            shape: concat_above_multiply,
        }]),
    },
    CoverageCase {
        feature: Feature::BindingPowers,
        polarity: Polarity::Negative,
        name: "custom_binding_power_allows_left_assoc_comparison_chain",
        // Left-associative comparison chains parse left-deep.
        coverage: Coverage::Behavior(&[Probe::ParseShape {
            sql: "SELECT a < b < c",
            features: &LEFT_ASSOC_COMPARISON_FEATURES,
            shape: lt_left_deep,
        }]),
    },
    CoverageCase {
        feature: Feature::SetOperationPowers,
        polarity: Polarity::Positive,
        name: "ansi_set_operation_power_structures_intersect_under_union",
        // INTERSECT binds under UNION in the standard table.
        coverage: Coverage::Behavior(&[Probe::ParseShape {
            sql: "SELECT 1 UNION SELECT 2 INTERSECT SELECT 3",
            features: &FeatureSet::ANSI,
            shape: intersect_under_union,
        }]),
    },
    CoverageCase {
        feature: Feature::SetOperationPowers,
        polarity: Polarity::Negative,
        name: "custom_set_operation_power_structures_union_under_intersect",
        // Raising UNION/EXCEPT above INTERSECT changes the structural parse.
        coverage: Coverage::Behavior(&[Probe::ParseShape {
            sql: "SELECT 1 UNION SELECT 2 INTERSECT SELECT 3",
            features: &CUSTOM_SET_OPERATION_FEATURES,
            shape: union_under_intersect,
        }]),
    },
    CoverageCase {
        feature: Feature::StringLiterals,
        polarity: Polarity::Positive,
        name: "postgres_string_literals_accept_pg_extensions",
        // Enabling PostgreSQL string literal syntax accepts E-strings and dollar quotes.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT E'line\\n', $$body$$",
            features: &PG_STRING_LITERAL_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::StringLiterals,
        polarity: Polarity::Negative,
        name: "disabled_pg_string_literals_reject_pg_extensions",
        // With escape strings off, `E'x'` is the typed literal `E 'x'`, not an escape
        // string; dollar quotes have no identifier fallback, so they stay a clean reject.
        coverage: Coverage::Behavior(&[
            Probe::ParseShape {
                sql: "SELECT E'x'",
                features: &NO_PG_STRING_LITERAL_FEATURES,
                shape: is_prefix_typed_cast,
            },
            Probe::ParseRejects {
                sql: "SELECT $$body$$",
                features: &NO_PG_STRING_LITERAL_FEATURES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::TableFactorSyntax,
        polarity: Polarity::Positive,
        name: "postgres_table_factor_syntax_accepts_lateral",
        // Enabling PostgreSQL table expressions accepts LATERAL table functions.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT * FROM LATERAL generate_series(1, 3) AS g(x)",
            features: &PG_TABLE_EXPRESSION_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::TableFactorSyntax,
        polarity: Polarity::Negative,
        name: "disabled_table_factor_syntax_rejects_lateral",
        // Disabling PostgreSQL table expressions rejects LATERAL table functions.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT * FROM LATERAL generate_series(1, 3) AS g(x)",
            features: &NO_PG_TABLE_EXPRESSION_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::ExpressionSyntax,
        polarity: Polarity::Positive,
        name: "postgres_expression_syntax_accepts_pg_forms",
        // One representative per sub-flag: typecast, subscript/slice, COLLATE, AT TIME
        // ZONE, the array and (implicit + explicit) row constructors, and field selection.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "SELECT a::int",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a[1], a[1:2]",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a COLLATE \"C\"",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a AT TIME ZONE 'UTC'",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT ARRAY[1, 2], ARRAY(SELECT 1)",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT ROW(1, 2), (a, b)",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT (a).b",
                features: &FeatureSet::POSTGRES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::ExpressionSyntax,
        polarity: Polarity::Negative,
        name: "ansi_expression_syntax_rejects_pg_forms",
        // The same forms are all rejected under ANSI, which leaves the gates off.
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "SELECT a::int",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "SELECT a[1]",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "SELECT a COLLATE \"C\"",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "SELECT a AT TIME ZONE 'UTC'",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "SELECT ARRAY[1, 2]",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "SELECT (a, b)",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "SELECT (a).b",
                features: &FeatureSet::ANSI,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::OperatorSyntax,
        polarity: Polarity::Positive,
        name: "postgres_operator_syntax_accepts_pg_forms",
        // One representative per PostgreSQL-gated operator: containment `@>`, the JSON
        // arrow `->`, and the shared bitwise `|`.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "SELECT a @> b",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a -> 'k'",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a | b",
                features: &FeatureSet::POSTGRES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::OperatorSyntax,
        polarity: Polarity::Negative,
        name: "ansi_operator_syntax_rejects_pg_forms",
        // The same operators are all rejected under ANSI, which leaves the gates off.
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "SELECT a @> b",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "SELECT a -> 'k'",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "SELECT a | b",
                features: &FeatureSet::ANSI,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::CallSyntax,
        polarity: Polarity::Positive,
        name: "postgres_call_syntax_accepts_named_arguments",
        // PostgreSQL named function arguments (`=>` and the deprecated `:=`).
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "SELECT f(x => 1)",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT f(x := 1)",
                features: &FeatureSet::POSTGRES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::CallSyntax,
        polarity: Polarity::Negative,
        name: "ansi_call_syntax_rejects_named_arguments",
        // ANSI leaves the gate off, so the `=>` arrow never lexes and the call is rejected.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT f(x => 1)",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::StringFuncForms,
        polarity: Polarity::Positive,
        name: "postgres_string_func_forms_accepts_substring_from_for",
        // The SQL-standard `SUBSTRING(<expr> FROM <start> FOR <count>)` keyword special
        // form parses to the `StringFunc` AST family.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT SUBSTRING('abcdef' FROM 2 FOR 3)",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::StringFuncForms,
        polarity: Polarity::Negative,
        name: "sqlite_string_func_forms_rejects_substring_from_for",
        // SQLite has none of the keyword string special forms, so the inner `FROM`
        // surfaces as a clean parse error.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT SUBSTRING('abcdef' FROM 2 FOR 3)",
            features: &FeatureSet::SQLITE,
        }]),
    },
    CoverageCase {
        feature: Feature::AggregateCallSyntax,
        polarity: Polarity::Positive,
        name: "postgres_aggregate_call_syntax_accepts_filter_clause",
        // The `FILTER (WHERE <predicate>)` aggregate-filter tail (SQL:2003 T612).
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT sum(x) FILTER (WHERE x > 1) FROM t",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::AggregateCallSyntax,
        polarity: Polarity::Negative,
        name: "mysql_aggregate_call_syntax_rejects_filter_clause",
        // MySQL has no aggregate `FILTER` clause, so the `FILTER` keyword is left
        // unconsumed and surfaces as a clean parse error.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT sum(x) FILTER (WHERE x > 1) FROM t",
            features: &FeatureSet::MYSQL,
        }]),
    },
    CoverageCase {
        feature: Feature::PredicateSyntax,
        polarity: Polarity::Positive,
        name: "predicate_syntax_accepts_like_family",
        // PostgreSQL enables the whole family; one representative per spelling plus the
        // `NOT` and `ESCAPE` tails. `LIKE` is SQL core (E021-08): on in *every* dialect.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "SELECT a LIKE 'b%'",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a NOT LIKE 'b%'",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a LIKE 'b!%' ESCAPE '!'",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a ILIKE 'B%'",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a SIMILAR TO '(a|b)%'",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a NOT SIMILAR TO '(a|b)%'",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a LIKE 'b%'",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseAccepts {
                sql: "SELECT a NOT LIKE 'b%' ESCAPE '!'",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseAccepts {
                sql: "SELECT a LIKE 'b%'",
                features: &FeatureSet::MYSQL,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::PredicateSyntax,
        polarity: Polarity::Negative,
        name: "ansi_predicate_syntax_rejects_ilike_and_similar_to",
        // ANSI leaves `ilike`/`similar_to` off, so each gated form is a parse error.
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "SELECT a ILIKE 'B%'",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "SELECT a SIMILAR TO '(a|b)%'",
                features: &FeatureSet::ANSI,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::PredicateSyntax,
        polarity: Polarity::Positive,
        name: "duckdb_predicate_syntax_accepts_unparenthesized_in",
        // DuckDB's unparenthesized `<expr> [NOT] IN <value>` list-membership: a restricted
        // `c_expr` RHS (column / call / subscript / array literal / parameter) parses,
        // while a leading constant or unary sign is a parse error (DuckDB's gram.y rule).
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "SELECT a IN b",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseAccepts {
                sql: "SELECT a NOT IN b",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseAccepts {
                sql: "SELECT a IN [1, 2, 3]",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseAccepts {
                sql: "SELECT a IN f(x)",
                features: &FeatureSet::DUCKDB,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::PredicateSyntax,
        polarity: Polarity::Negative,
        name: "unparenthesized_in_rejects_off_dialect_and_constant_rhs",
        // Off in ANSI/PostgreSQL — the bare value needs a `(` there — and even in DuckDB
        // a leading constant or unary sign in the RHS is a parse error (the leading-token
        // gate that keeps the syntax over-acceptance ledger honest).
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "SELECT a IN b",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "SELECT a IN b",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseRejects {
                sql: "SELECT a IN 4",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseRejects {
                sql: "SELECT a IN -5",
                features: &FeatureSet::DUCKDB,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::PipeOperator,
        polarity: Polarity::Positive,
        name: "ansi_pipe_operator_concatenates",
        // ANSI parses `||` as string concatenation.
        coverage: Coverage::Behavior(&[Probe::ParseShape {
            sql: "SELECT a || b",
            features: &FeatureSet::ANSI,
            shape: pipe_is_string_concat,
        }]),
    },
    CoverageCase {
        feature: Feature::PipeOperator,
        polarity: Polarity::Negative,
        name: "logical_or_pipe_operator_diverges",
        // A logical-OR `||` dialect parses `||` as OR, diverging from ANSI.
        coverage: Coverage::Behavior(&[Probe::ParseShape {
            sql: "SELECT a || b",
            features: &LOGICAL_OR_PIPE_FEATURES,
            shape: pipe_is_logical_or,
        }]),
    },
    CoverageCase {
        feature: Feature::NumericLiterals,
        polarity: Polarity::Positive,
        name: "hex_numeric_literal_accepts",
        // `+ 1` makes the split-token reading a hard error, so acceptance depends on
        // `0x1F` lexing as one number.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT 0x1F + 1",
            features: &HEX_NUMBER_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::NumericLiterals,
        polarity: Polarity::Negative,
        name: "ansi_rejects_dialect_numeric_literal",
        // ANSI does not accept `0x..` hexadecimal literals.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT 0x1F + 1",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::DoubleAmpersand,
        polarity: Polarity::Positive,
        name: "logical_and_ampersand_parses_as_and",
        // Under `&&`-as-AND, `&&` roots `a = b && c` with `a = b` on its left (AND is
        // looser than `=`), the same canonical shape as the `AND` keyword.
        coverage: Coverage::Behavior(&[Probe::ParseShape {
            sql: "SELECT a = b && c",
            features: &LOGICAL_AND_AMP_FEATURES,
            shape: and_over_eq,
        }]),
    },
    CoverageCase {
        feature: Feature::DoubleAmpersand,
        polarity: Polarity::Negative,
        name: "ansi_rejects_double_ampersand_operator",
        // ANSI does not accept `&&` as a scalar operator.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT a && b",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::KeywordOperators,
        polarity: Polarity::Positive,
        name: "mysql_keyword_operators_parse_with_their_precedence",
        // `DIV` is multiplicative (binds tighter than `+`), and `XOR` ranks between `OR`
        // and `AND` (so `AND` binds tighter) — each keyword operator carries its own
        // precedence.
        coverage: Coverage::Behavior(&[
            Probe::ParseShape {
                sql: "SELECT a + b DIV c",
                features: &MYSQL_KEYWORD_OPERATOR_FEATURES,
                shape: plus_over_div,
            },
            Probe::ParseShape {
                sql: "SELECT a XOR b AND c",
                features: &MYSQL_KEYWORD_OPERATOR_FEATURES,
                shape: xor_over_and,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::KeywordOperators,
        polarity: Polarity::Negative,
        name: "ansi_rejects_keyword_operators",
        // ANSI does not treat `DIV` as an operator, so the word ends the expression and
        // the trailing operand is unexpected input.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT a DIV b",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::CaretOperator,
        polarity: Polarity::Positive,
        name: "caret_operator_readings_parse_under_their_dialect",
        // The `^` byte's two infix readings both accept: MySQL reads it as bitwise XOR, and
        // PostgreSQL/DuckDB as arithmetic power. The always-lexed caret is read as an infix
        // operator under either meaning.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "SELECT 5 ^ 3",
                features: &CARET_BITWISE_XOR_FEATURES,
            },
            Probe::ParseAccepts {
                sql: "SELECT 5 ^ 3",
                features: &CARET_EXPONENT_FEATURES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::CaretOperator,
        polarity: Polarity::Negative,
        name: "ansi_rejects_caret_operator",
        // ANSI gives `^` no infix meaning: its `Caret` token maps to no operator, so `^` ends
        // the expression and the trailing operand is unexpected input.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT 5 ^ 3",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::HashBitwiseXor,
        polarity: Polarity::Positive,
        name: "hash_bitwise_xor_parses",
        // PostgreSQL spells bitwise XOR `#` (lexed as the operator over the ANSI baseline,
        // where `#` is otherwise a stray byte).
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT 5 # 3",
            features: &HASH_BITWISE_XOR_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::HashBitwiseXor,
        polarity: Polarity::Negative,
        name: "ansi_rejects_hash_bitwise_xor",
        // ANSI does not read `#` as the XOR operator: `#` is a stray byte, so it is
        // unexpected input.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT 5 # 3",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::CommentSyntax,
        polarity: Polarity::Positive,
        name: "hash_comment_dialect_skips_line_comment",
        // A `#`-comment dialect skips the comment and parses the rest.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT 1 # trailing comment\n+ 2",
            features: &HASH_COMMENT_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::CommentSyntax,
        polarity: Polarity::Negative,
        name: "ansi_rejects_hash_line_comment",
        // ANSI treats `#` as a stray byte, not a comment.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT 1 # trailing comment",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::CommentSyntax,
        polarity: Polarity::Positive,
        name: "versioned_comment_body_is_live_input",
        // MySQL versioned comments are conditional inclusion: the engine
        // *executes* the body, so a dialect with the gate parses it as live
        // tokens (`SELECT /*!40101 1 */ AS x` is `SELECT 1 AS x`), and a
        // version above the modelled bound is discarded like the engine
        // discards it (the body vanishes, so the same wrapper around the only
        // projection makes the statement unparseable).
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "SELECT /*!40101 1 */ AS x",
                features: &VERSIONED_COMMENT_FEATURES,
            },
            Probe::ParseRejects {
                sql: "SELECT /*!99999 1 */ AS x",
                features: &VERSIONED_COMMENT_FEATURES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::CommentSyntax,
        polarity: Polarity::Negative,
        name: "ansi_keeps_versioned_comment_a_plain_comment",
        // Without the gate the whole `/*!…*/` construct stays a skipped block
        // comment (the pre-existing behaviour of every non-MySQL dialect): the
        // body is not input, so the projection-less statement rejects while a
        // statement that is complete without the body still parses.
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "SELECT /*!40101 1 */ AS x",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseAccepts {
                sql: "SELECT 1 /*!40101 + 1 */",
                features: &FeatureSet::ANSI,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::Parameters,
        polarity: Polarity::Positive,
        name: "positional_parameter_accepts",
        // A positional-parameter dialect accepts `$1`.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT $1",
            features: &POSITIONAL_PARAMETER_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::Parameters,
        polarity: Polarity::Negative,
        name: "ansi_rejects_parameter_placeholder",
        // ANSI does not accept `$1` placeholders (`$` is a stray byte).
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT $1",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::Parameters,
        polarity: Polarity::Positive,
        name: "named_colon_parameter_accepts",
        // A named-colon dialect accepts `:name`.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT :name",
            features: &NAMED_COLON_PARAMETER_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::Parameters,
        polarity: Polarity::Positive,
        name: "named_at_parameter_accepts",
        // A named-at dialect accepts `@name`.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT @name",
            features: &NAMED_AT_PARAMETER_FEATURES,
        }]),
    },
    CoverageCase {
        feature: Feature::SessionVariables,
        polarity: Polarity::Positive,
        name: "mysql_session_variable_accepts",
        // All four surface forms lex and parse: the user variable, the implicit-scope
        // system variable, and the two explicitly scoped ones.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "SELECT @user_count",
                features: &SESSION_VARIABLE_FEATURES,
            },
            Probe::ParseAccepts {
                sql: "SELECT @@max_connections",
                features: &SESSION_VARIABLE_FEATURES,
            },
            Probe::ParseAccepts {
                sql: "SELECT @@global.time_zone",
                features: &SESSION_VARIABLE_FEATURES,
            },
            Probe::ParseAccepts {
                sql: "SELECT @@session.sql_mode",
                features: &SESSION_VARIABLE_FEATURES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::SessionVariables,
        polarity: Polarity::Negative,
        name: "ansi_rejects_session_variable",
        // ANSI does not accept `@x`/`@@x` variables (`@` is a stray byte).
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "SELECT @x",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "SELECT @@x",
                features: &FeatureSet::ANSI,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::IdentifierSyntax,
        polarity: Polarity::Positive,
        name: "postgres_accepts_dollar_in_identifier",
        // PostgreSQL accepts `$` as an identifier-continue character.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT foo$bar",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::IdentifierSyntax,
        polarity: Polarity::Negative,
        name: "ansi_rejects_dollar_in_identifier",
        // ANSI forbids `$` in identifiers: `foo` ends and the `$` is then a stray byte.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT foo$bar",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::MutationSyntax,
        polarity: Polarity::Positive,
        name: "postgres_mutation_syntax_accepts_returning_and_on_conflict",
        // PostgreSQL accepts RETURNING and ON CONFLICT clauses on INSERT.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "INSERT INTO t VALUES (1) RETURNING id",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "INSERT INTO t VALUES (1) ON CONFLICT DO NOTHING",
                features: &FeatureSet::POSTGRES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::MutationSyntax,
        polarity: Polarity::Negative,
        name: "ansi_mutation_syntax_rejects_returning_and_on_conflict",
        // ANSI has no RETURNING or ON CONFLICT clause, so the trailing clause is an error.
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "INSERT INTO t VALUES (1) RETURNING id",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "INSERT INTO t VALUES (1) ON CONFLICT DO NOTHING",
                features: &FeatureSet::ANSI,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::ExistenceGuards,
        polarity: Polarity::Positive,
        name: "postgres_existence_guards_accepts_if_exists",
        // PostgreSQL accepts an IF EXISTS guard on DROP and ALTER TABLE.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "DROP TABLE IF EXISTS t",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "ALTER TABLE IF EXISTS t DROP COLUMN c",
                features: &FeatureSet::POSTGRES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::ExistenceGuards,
        polarity: Polarity::Negative,
        name: "ansi_existence_guards_rejects_if_exists",
        // ANSI has no IF EXISTS guard, so the unconsumed clause is a parse error.
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "DROP TABLE IF EXISTS t",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "ALTER TABLE IF EXISTS t DROP COLUMN c",
                features: &FeatureSet::ANSI,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::SelectSyntax,
        polarity: Polarity::Positive,
        name: "postgres_select_syntax_accepts_distinct_on_and_fetch_first",
        // DISTINCT ON, FETCH FIRST … ROWS ONLY, and the SELECT … INTO <table> create-table
        // form (including the TEMP marker).
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "SELECT DISTINCT ON (a) a FROM t",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT 1 FETCH FIRST 2 ROWS ONLY",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a INTO t FROM s",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseAccepts {
                sql: "SELECT a INTO TEMP t FROM s",
                features: &FeatureSet::POSTGRES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::SelectSyntax,
        polarity: Polarity::Negative,
        name: "ansi_select_syntax_rejects_distinct_on",
        // ANSI leaves `DISTINCT ON` ungated, so `ON` is an unexpected projection token.
        // (FETCH FIRST is SQL-standard, so ANSI accepts it — its gate-off reject is
        // exercised by the `fetch_first` labelled case.)
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT DISTINCT ON (a) a FROM t",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::SelectSyntax,
        polarity: Polarity::Positive,
        name: "duckdb_select_syntax_accepts_from_first",
        // DuckDB's FROM-first order: `FROM t SELECT x`, and the SELECT-less bare `FROM t`
        // (implicit `SELECT *`).
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "FROM t SELECT a",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseAccepts {
                sql: "FROM t",
                features: &FeatureSet::DUCKDB,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::SelectSyntax,
        polarity: Polarity::Negative,
        name: "ansi_select_syntax_rejects_from_first",
        // ANSI leaves `from_first` off, so a statement-position `FROM` is not a query
        // start — a clean parse error (the over-acceptance guard).
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "FROM t SELECT a",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::QueryTailSyntax,
        polarity: Polarity::Positive,
        name: "postgres_query_tail_syntax_accepts_locking_clause",
        // The trailing `FOR UPDATE` row-locking clause parsed by the query tail.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT a FROM t FOR UPDATE",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::QueryTailSyntax,
        polarity: Polarity::Negative,
        name: "ansi_query_tail_syntax_rejects_locking_clause",
        // ANSI has no query-tail lock clause, so the `FOR` keyword is left unconsumed
        // and surfaces as a clean parse error.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT a FROM t FOR UPDATE",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::GroupingSyntax,
        polarity: Polarity::Positive,
        name: "postgres_grouping_syntax_accepts_order_by_using",
        // PostgreSQL's `ORDER BY <expr> USING <operator>` sort form.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "SELECT a FROM t ORDER BY a USING <",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::GroupingSyntax,
        polarity: Polarity::Negative,
        name: "ansi_grouping_syntax_rejects_order_by_using",
        // ANSI sorts only by `ASC`/`DESC`, so the `USING` keyword is left unconsumed
        // and surfaces as a trailing-input parse error.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SELECT a FROM t ORDER BY a USING <",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::UtilitySyntax,
        polarity: Polarity::Positive,
        name: "postgres_utility_syntax_accepts_copy",
        // PostgreSQL dispatches the `COPY` bulk data-transfer statement.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "COPY t TO STDOUT",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::UtilitySyntax,
        polarity: Polarity::Negative,
        name: "ansi_utility_syntax_rejects_copy",
        // ANSI (and MySQL) gate `COPY` off, so the leading keyword is not dispatched and
        // surfaces as an unknown statement.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "COPY t TO STDOUT",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::UtilitySyntax,
        polarity: Polarity::Positive,
        name: "postgres_utility_syntax_accepts_comment_on",
        // PostgreSQL dispatches the `COMMENT ON` object-metadata statement.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "COMMENT ON TABLE t IS 'note'",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::UtilitySyntax,
        polarity: Polarity::Negative,
        name: "ansi_utility_syntax_rejects_comment_on",
        // ANSI (and MySQL) gate `COMMENT ON` off, so the leading keyword is not
        // dispatched and surfaces as an unknown statement.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "COMMENT ON TABLE t IS 'note'",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::UtilitySyntax,
        polarity: Polarity::Positive,
        name: "sqlite_utility_syntax_accepts_pragma",
        // SQLite dispatches the `PRAGMA` configuration statement in its bare,
        // assignment, and call forms.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "PRAGMA user_version",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseAccepts {
                sql: "PRAGMA foreign_keys = ON",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseAccepts {
                sql: "PRAGMA table_info(sqlite_master)",
                features: &FeatureSet::SQLITE,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::UtilitySyntax,
        polarity: Polarity::Negative,
        name: "ansi_and_postgres_utility_syntax_reject_pragma",
        // ANSI, PostgreSQL, and MySQL gate `PRAGMA` off, so the leading keyword is
        // not dispatched and surfaces as an unknown statement.
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "PRAGMA user_version",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "PRAGMA user_version",
                features: &FeatureSet::POSTGRES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::UtilitySyntax,
        polarity: Polarity::Positive,
        name: "sqlite_utility_syntax_accepts_attach_and_detach",
        // One `attach` flag dispatches the `ATTACH`/`DETACH` pair (a single dialect
        // unit), with and without the optional `DATABASE` keyword.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "ATTACH DATABASE ':memory:' AS aux",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseAccepts {
                sql: "ATTACH ':memory:' AS aux",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseAccepts {
                sql: "DETACH DATABASE aux",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseAccepts {
                sql: "DETACH aux",
                features: &FeatureSet::SQLITE,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::UtilitySyntax,
        polarity: Polarity::Negative,
        name: "ansi_and_postgres_utility_syntax_reject_attach_and_detach",
        // ANSI, PostgreSQL, and MySQL gate the pair off, so both leading keywords
        // surface as unknown statements.
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "ATTACH ':memory:' AS aux",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "DETACH aux",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "ATTACH ':memory:' AS aux",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseRejects {
                sql: "DETACH aux",
                features: &FeatureSet::POSTGRES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::MaintenanceSyntax,
        polarity: Polarity::Positive,
        name: "sqlite_maintenance_syntax_accepts_vacuum_reindex_and_analyze",
        // SQLite dispatches the three maintenance statements: bare, with the optional
        // (single, non-dotted) schema / qualified-name argument, and the `VACUUM INTO`
        // expression target.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "VACUUM",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseAccepts {
                sql: "VACUUM main INTO 'a' || '.db'",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseAccepts {
                sql: "REINDEX main.t",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseAccepts {
                sql: "ANALYZE sqlite_master",
                features: &FeatureSet::SQLITE,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::MaintenanceSyntax,
        polarity: Polarity::Negative,
        name: "ansi_and_postgres_maintenance_syntax_reject_vacuum_reindex_and_analyze",
        // ANSI, PostgreSQL, and MySQL gate the trio off, so each leading keyword
        // surfaces as an unknown statement (PostgreSQL's own differently-shaped
        // VACUUM/REINDEX/ANALYZE statements are not modelled).
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "VACUUM",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "REINDEX",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "ANALYZE",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "VACUUM",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseRejects {
                sql: "ANALYZE",
                features: &FeatureSet::POSTGRES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::UtilitySyntax,
        polarity: Polarity::Positive,
        name: "mysql_utility_syntax_accepts_kill_and_describe",
        // MySQL dispatches `KILL` (bare / CONNECTION / QUERY, with an expression id) and
        // the `DESCRIBE`/`DESC` EXPLAIN synonyms — both the query-plan form and the
        // `{DESCRIBE|DESC|EXPLAIN} <table> [<column>]` table-metadata overload.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "KILL 5",
                features: &FeatureSet::MYSQL,
            },
            Probe::ParseAccepts {
                sql: "KILL CONNECTION 5",
                features: &FeatureSet::MYSQL,
            },
            Probe::ParseAccepts {
                sql: "KILL QUERY '123'",
                features: &FeatureSet::MYSQL,
            },
            Probe::ParseAccepts {
                sql: "DESCRIBE SELECT 1",
                features: &FeatureSet::MYSQL,
            },
            Probe::ParseAccepts {
                sql: "DESC SELECT 1",
                features: &FeatureSet::MYSQL,
            },
            Probe::ParseAccepts {
                sql: "DESCRIBE t col",
                features: &FeatureSet::MYSQL,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::UtilitySyntax,
        polarity: Polarity::Negative,
        name: "non_mysql_utility_syntax_rejects_kill_and_describe",
        // ANSI, PostgreSQL, SQLite, and DuckDB gate `kill`/`describe` (the MySQL flags) off,
        // so a leading `KILL` surfaces as an unknown statement and the MySQL table-metadata
        // overload after `EXPLAIN` is rejected (their `EXPLAIN` keeps the query-plan-only
        // grammar). On ANSI/PostgreSQL/SQLite the `DESCRIBE`/`DESC` synonyms are also unknown
        // statements; DuckDB is the exception — its own `describe_summarize` flag makes
        // `DESCRIBE`/`SUMMARIZE` the `SHOW_REF` introspection statement, so the DuckDB reject
        // probe uses `EXPLAIN t` (still off, since that overload rides the MySQL `describe`).
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "KILL 5",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "DESCRIBE SELECT 1",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseRejects {
                sql: "EXPLAIN t",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseRejects {
                sql: "KILL 5",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseRejects {
                sql: "DESC t",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseRejects {
                sql: "KILL 5",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseRejects {
                sql: "EXPLAIN t",
                features: &FeatureSet::DUCKDB,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::ShowSyntax,
        polarity: Polarity::Positive,
        name: "duckdb_show_syntax_accepts_describe_summarize_statement",
        // DuckDB's `describe_summarize`: a leading `DESCRIBE`/`SUMMARIZE` is the `SHOW_REF`
        // introspection statement over a describable query or a bare table name (desugaring
        // to `SELECT * FROM (SHOW_REF …)`), reusing the same core as the `FROM (…)` factor.
        // The named-target form takes no trailing clause and bare `SUMMARIZE` has no target —
        // both DuckDB-verified reject boundaries kept out of the accept set.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "DESCRIBE SELECT 42 AS a",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseAccepts {
                sql: "SUMMARIZE SELECT 42 AS a",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseAccepts {
                sql: "SUMMARIZE arrays",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseRejects {
                sql: "SUMMARIZE arrays WHERE a > 1",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseRejects {
                sql: "SUMMARIZE",
                features: &FeatureSet::DUCKDB,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::ShowSyntax,
        polarity: Polarity::Negative,
        name: "ansi_show_syntax_rejects_describe_summarize",
        // ANSI has no `DESCRIBE`/`SUMMARIZE` introspection statement, so the leading
        // keyword is not dispatched and surfaces as an unknown statement.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "SUMMARIZE SELECT 42 AS a",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::AccessControlSyntax,
        polarity: Polarity::Positive,
        name: "postgres_access_control_syntax_accepts_grant",
        // PostgreSQL dispatches the `GRANT`/`REVOKE` access-control statements.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "GRANT SELECT ON t TO u",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::AccessControlSyntax,
        polarity: Polarity::Negative,
        name: "sqlite_access_control_syntax_rejects_grant",
        // SQLite has no permission system, so the leading `GRANT` keyword is not
        // dispatched and surfaces as an unknown statement.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "GRANT SELECT ON t TO u",
            features: &FeatureSet::SQLITE,
        }]),
    },
    CoverageCase {
        feature: Feature::StatementDdlGates,
        polarity: Polarity::Positive,
        name: "sqlite_schema_change_syntax_accepts_create_trigger",
        // SQLite dispatches the whole `CREATE TRIGGER … BEGIN … END` statement: the
        // timing/event matrix, the `UPDATE OF` column restriction, `FOR EACH ROW`,
        // `WHEN`, and a multi-statement body.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "CREATE TRIGGER trg AFTER INSERT ON t BEGIN UPDATE t SET c = c + 1; END",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseAccepts {
                sql: "CREATE TEMP TRIGGER IF NOT EXISTS trg BEFORE UPDATE OF a, b ON t FOR EACH ROW WHEN a > 0 BEGIN SELECT 1; DELETE FROM t WHERE a = 1; END",
                features: &FeatureSet::SQLITE,
            },
            Probe::ParseAccepts {
                sql: "CREATE TRIGGER trg INSTEAD OF DELETE ON v BEGIN INSERT INTO log VALUES (1); END",
                features: &FeatureSet::SQLITE,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::StatementDdlGates,
        polarity: Polarity::Negative,
        name: "ansi_and_postgres_schema_change_syntax_reject_create_trigger",
        // The whole-statement gate: ANSI/PostgreSQL/MySQL do not model the SQLite
        // trigger body form, so `TRIGGER` after `CREATE` is not dispatched and the
        // statement rejects.
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "CREATE TRIGGER trg AFTER INSERT ON t BEGIN UPDATE t SET c = c + 1; END",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "CREATE TRIGGER trg AFTER INSERT ON t BEGIN UPDATE t SET c = c + 1; END",
                features: &FeatureSet::POSTGRES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::StatementDdlGates,
        polarity: Polarity::Positive,
        name: "duckdb_schema_change_syntax_accepts_create_macro",
        // DuckDB dispatches the whole `CREATE … {MACRO | FUNCTION} … AS <body>` statement:
        // the scalar and `TABLE` bodies, `OR REPLACE`, the `FUNCTION` synonym, and a
        // schema-qualified name.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "CREATE MACRO plus1(x) AS x + 1",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseAccepts {
                sql: "CREATE OR REPLACE MACRO s.m(x) AS TABLE SELECT x AS c",
                features: &FeatureSet::DUCKDB,
            },
            Probe::ParseAccepts {
                sql: "CREATE FUNCTION f(x) AS TABLE SELECT x",
                features: &FeatureSet::DUCKDB,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::StatementDdlGates,
        polarity: Polarity::Negative,
        name: "ansi_and_postgres_schema_change_syntax_reject_create_macro",
        // The whole-statement gate: ANSI has no `CREATE MACRO` (the `MACRO` keyword is not
        // dispatched -> reject), and PostgreSQL routes `CREATE FUNCTION` to the string-body
        // routine parser, which rejects the live expression body.
        coverage: Coverage::Behavior(&[
            Probe::ParseRejects {
                sql: "CREATE MACRO plus1(x) AS x + 1",
                features: &FeatureSet::ANSI,
            },
            Probe::ParseRejects {
                sql: "CREATE MACRO plus1(x) AS x + 1",
                features: &FeatureSet::POSTGRES,
            },
            Probe::ParseRejects {
                sql: "CREATE FUNCTION f(x) AS x + 1",
                features: &FeatureSet::POSTGRES,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::CreateTableClauseSyntax,
        polarity: Polarity::Positive,
        name: "mysql_create_table_clause_syntax_accepts_table_options",
        // MySQL's trailing `CREATE TABLE` storage options (`ENGINE = …`).
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "CREATE TABLE t (a INT) ENGINE = InnoDB",
            features: &FeatureSet::MYSQL,
        }]),
    },
    CoverageCase {
        feature: Feature::CreateTableClauseSyntax,
        polarity: Polarity::Negative,
        name: "ansi_create_table_clause_syntax_rejects_table_options",
        // ANSI has no trailing table options, so the `ENGINE` keyword is left as leftover
        // input and surfaces as a clean parse error.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "CREATE TABLE t (a INT) ENGINE = InnoDB",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::ColumnDefinitionSyntax,
        polarity: Polarity::Positive,
        name: "postgres_column_definition_syntax_accepts_column_collation",
        // PostgreSQL's per-column `COLLATE <collation>` clause.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "CREATE TABLE t (a text COLLATE \"C\")",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::ColumnDefinitionSyntax,
        polarity: Polarity::Negative,
        name: "ansi_column_definition_syntax_rejects_column_collation",
        // ANSI has no column-level `COLLATE`, so it is left unconsumed and surfaces as a
        // clean parse error.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "CREATE TABLE t (a text COLLATE \"C\")",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::ConstraintSyntax,
        polarity: Polarity::Positive,
        name: "postgres_constraint_syntax_accepts_deferrable",
        // PostgreSQL's `[NOT] DEFERRABLE` constraint characteristics.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "CREATE TABLE t (a INT REFERENCES b (id) DEFERRABLE)",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::ConstraintSyntax,
        polarity: Polarity::Negative,
        name: "mysql_constraint_syntax_rejects_deferrable",
        // MySQL has no deferrable constraints, so `DEFERRABLE` is left unconsumed and
        // surfaces as a clean parse error.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "CREATE TABLE t (a INT REFERENCES b (id) DEFERRABLE)",
            features: &FeatureSet::MYSQL,
        }]),
    },
    CoverageCase {
        feature: Feature::IndexAlterSyntax,
        polarity: Polarity::Positive,
        name: "postgres_index_alter_syntax_accepts_index_concurrently",
        // PostgreSQL's `CREATE INDEX CONCURRENTLY`.
        coverage: Coverage::Behavior(&[Probe::ParseAccepts {
            sql: "CREATE INDEX CONCURRENTLY i ON t (a)",
            features: &FeatureSet::POSTGRES,
        }]),
    },
    CoverageCase {
        feature: Feature::IndexAlterSyntax,
        polarity: Polarity::Negative,
        name: "ansi_index_alter_syntax_rejects_index_concurrently",
        // ANSI has no `CONCURRENTLY`, so the keyword is left unconsumed and surfaces as a
        // clean parse error.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "CREATE INDEX CONCURRENTLY i ON t (a)",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::TypeNameSyntax,
        polarity: Polarity::Positive,
        name: "mysql_type_name_syntax_accepts_extended_type_names",
        // The value-list ENUM/SET forms, the numeric UNSIGNED/ZEROFILL modifiers, and the
        // standalone cast target.
        coverage: Coverage::Behavior(&[
            Probe::ParseAccepts {
                sql: "CREATE TABLE t (c ENUM('a', 'b'))",
                features: &FeatureSet::MYSQL,
            },
            Probe::ParseAccepts {
                sql: "CREATE TABLE t (c INT UNSIGNED ZEROFILL)",
                features: &FeatureSet::MYSQL,
            },
            Probe::ParseAccepts {
                sql: "SELECT CAST(a AS UNSIGNED) FROM t",
                features: &FeatureSet::MYSQL,
            },
        ]),
    },
    CoverageCase {
        feature: Feature::TypeNameSyntax,
        polarity: Polarity::Negative,
        name: "ansi_type_name_syntax_rejects_value_list_types",
        // ANSI leaves the type-name extensions ungated, so the structural `ENUM('a','b')`
        // value list is a parse error.
        coverage: Coverage::Behavior(&[Probe::ParseRejects {
            sql: "CREATE TABLE t (c ENUM('a', 'b'))",
            features: &FeatureSet::ANSI,
        }]),
    },
    CoverageCase {
        feature: Feature::TargetSpelling,
        polarity: Polarity::Positive,
        name: "ansi_target_spelling_renders_standard_types",
        // The ANSI baseline target spelling keeps `DECIMAL` in its SQL-standard form.
        coverage: Coverage::Behavior(&[Probe::RenderText {
            sql: "SELECT CAST(a AS DECIMAL(10, 2))",
            target: &FeatureSet::ANSI,
            text: renders_standard_decimal,
        }]),
    },
    CoverageCase {
        feature: Feature::TargetSpelling,
        polarity: Polarity::Negative,
        name: "postgres_target_spelling_renders_pg_types",
        // The PostgreSQL target spelling diverges: the same `DECIMAL` renders as `NUMERIC`.
        coverage: Coverage::Behavior(&[Probe::RenderText {
            sql: "SELECT CAST(a AS DECIMAL(10, 2))",
            target: &FeatureSet::POSTGRES,
            text: renders_pg_numeric,
        }]),
    },
];

/// Render the sole statement of `sql` (parsed under [`Ansi`]) with `target`'s
/// preferred spellings — the [`RenderSpelling::TargetDialect`] path that the
/// `target_spelling` field drives. Used by the `target_spelling` coverage probes to
/// assert which canonical type spelling a render target emits.
pub(crate) fn render_to_target(sql: &str, target: &FeatureSet) -> String {
    let parsed = parse_with(sql, Ansi).expect("type cast parses under ANSI");
    let config = RenderConfig {
        target: target.clone(),
        spelling: RenderSpelling::TargetDialect,
        ..RenderConfig::default()
    };
    let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
    let [statement] = parsed.statements() else {
        panic!("expected exactly one statement to render");
    };
    statement.displayed(&ctx).to_string()
}

// --- probe predicates -----------------------------------------------------------
//
// The structural / token / render predicates a `Probe` checks after the harness runs.
// Each is total (returns `false` on an unexpected shape, never panics) so `Probe::holds`
// cleanly reports a mismatch — that is what lets `confirm_case` decide a behaviour case's
// kind from the harness run rather than a tag.

/// The ANSI baseline target spelling keeps the divergent type name in its SQL-standard
/// form: `DECIMAL` stays `DECIMAL` (no PostgreSQL `NUMERIC` rename).
fn renders_standard_decimal(rendered: &str) -> bool {
    rendered.contains("DECIMAL(10, 2)")
}

/// The PostgreSQL target spelling diverges: the same standard `DECIMAL` renders as
/// PostgreSQL's canonical `NUMERIC`.
fn renders_pg_numeric(rendered: &str) -> bool {
    rendered.contains("NUMERIC(10, 2)") && !rendered.contains("DECIMAL")
}

/// A single quoted-identifier token — the shape a dialect's identifier quote lexes.
fn quoted_ident_singleton(tokens: &[Token]) -> bool {
    matches!(tokens, [token] if token.kind == TokenKind::QuotedIdent)
}

/// A single word token — the shape a custom identifier-start byte class lexes `@name` to.
fn word_singleton(tokens: &[Token]) -> bool {
    matches!(tokens, [token] if token.kind == TokenKind::Word)
}

/// Any `+` operator token — the standard byte classes classify `+` as an operator.
fn has_plus_operator(tokens: &[Token]) -> bool {
    tokens
        .iter()
        .any(|token| token.kind == TokenKind::Operator(Operator::Plus))
}

/// `a || b * c` roots at `||` (string concat) with the `*` multiply on its right —
/// multiply binds tighter than concat under the ANSI table.
fn concat_below_multiply(parsed: &Parsed) -> bool {
    let Expr::BinaryOp {
        op: BinaryOperator::StringConcat,
        right,
        ..
    } = sole_projection_expr(parsed)
    else {
        return false;
    };
    matches!(
        &**right,
        Expr::BinaryOp {
            op: BinaryOperator::Multiply,
            ..
        }
    )
}

/// `a || b * c` roots at `*` (multiply) with the `||` concat on its left — raising concat
/// above multiply changes the structural parse.
fn concat_above_multiply(parsed: &Parsed) -> bool {
    let Expr::BinaryOp {
        op: BinaryOperator::Multiply,
        left,
        ..
    } = sole_projection_expr(parsed)
    else {
        return false;
    };
    matches!(
        &**left,
        Expr::BinaryOp {
            op: BinaryOperator::StringConcat,
            ..
        }
    )
}

/// `a < b < c` parses left-deep (a left-associative comparison chain): the `<` root
/// carries another `<` on its left.
fn lt_left_deep(parsed: &Parsed) -> bool {
    let Expr::BinaryOp {
        op: BinaryOperator::Lt,
        left,
        ..
    } = sole_projection_expr(parsed)
    else {
        return false;
    };
    matches!(
        &**left,
        Expr::BinaryOp {
            op: BinaryOperator::Lt,
            ..
        }
    )
}

/// `1 UNION 2 INTERSECT 3` roots at UNION with INTERSECT on its right — INTERSECT binds
/// under UNION in the standard set-operation table.
fn intersect_under_union(parsed: &Parsed) -> bool {
    let SetExpr::SetOperation {
        op: SetOperator::Union,
        right,
        ..
    } = query_body(parsed)
    else {
        return false;
    };
    matches!(
        &**right,
        SetExpr::SetOperation {
            op: SetOperator::Intersect,
            ..
        }
    )
}

/// `1 UNION 2 INTERSECT 3` roots at INTERSECT with UNION on its left — raising
/// UNION/EXCEPT above INTERSECT changes the structural parse.
fn union_under_intersect(parsed: &Parsed) -> bool {
    let SetExpr::SetOperation {
        op: SetOperator::Intersect,
        left,
        ..
    } = query_body(parsed)
    else {
        return false;
    };
    matches!(
        &**left,
        SetExpr::SetOperation {
            op: SetOperator::Union,
            ..
        }
    )
}

/// The sole projection is a `||` logical OR — a logical-OR `||` dialect diverges from the
/// ANSI string-concatenation reading.
fn pipe_is_logical_or(parsed: &Parsed) -> bool {
    matches!(
        sole_projection_expr(parsed),
        Expr::BinaryOp {
            op: BinaryOperator::Or,
            ..
        }
    )
}

/// `a = b && c` roots at `&&`-as-AND with `a = b` on its left — `&&` takes the canonical
/// `AND` binding power, looser than `=`.
fn and_over_eq(parsed: &Parsed) -> bool {
    let Expr::BinaryOp {
        op: BinaryOperator::And,
        left,
        ..
    } = sole_projection_expr(parsed)
    else {
        return false;
    };
    matches!(
        &**left,
        Expr::BinaryOp {
            op: BinaryOperator::Eq(_),
            ..
        }
    )
}

/// `a + b DIV c` roots at `+` with the multiplicative `DIV` on its right — `DIV` binds
/// tighter than `+`.
fn plus_over_div(parsed: &Parsed) -> bool {
    let Expr::BinaryOp {
        op: BinaryOperator::Plus,
        right,
        ..
    } = sole_projection_expr(parsed)
    else {
        return false;
    };
    matches!(
        &**right,
        Expr::BinaryOp {
            op: BinaryOperator::IntegerDivide(_),
            ..
        }
    )
}

/// `a XOR b AND c` roots at `XOR` with `b AND c` on its right — `AND` binds tighter than
/// `XOR`.
fn xor_over_and(parsed: &Parsed) -> bool {
    let Expr::BinaryOp {
        op: BinaryOperator::Xor,
        right,
        ..
    } = sole_projection_expr(parsed)
    else {
        return false;
    };
    matches!(
        &**right,
        Expr::BinaryOp {
            op: BinaryOperator::And,
            ..
        }
    )
}

/// The sole projection is a prefix-typed cast (`E 'x'`) — with escape strings off, the
/// `E'x'` marker re-reads as the generalized typed literal, not an escape-string literal.
fn is_prefix_typed_cast(parsed: &Parsed) -> bool {
    matches!(
        sole_projection_expr(parsed),
        Expr::Cast {
            syntax: CastSyntax::PrefixTyped,
            ..
        }
    )
}

// --- metadata assertions (diagnostic only — do NOT satisfy the objectivity gate) ---
//
// These compare dialect data directly (`assert_eq!`/`assert_ne!` over `FeatureSet`
// fields). They are useful diagnostics carried alongside the behaviour probes, but by
// design they do NOT satisfy `has_objective_behavior` — a feature covered only by these
// fails the gate (ADR-0015).

fn assert_ansi_identifier_casing_upper() {
    assert_eq!(Ansi.features().identifier_casing, Casing::Upper);
}

fn assert_postgres_identifier_casing_diverges() {
    assert_ne!(
        Postgres.features().identifier_casing,
        Ansi.features().identifier_casing,
    );
}

fn assert_ansi_identifier_quote_double() {
    assert!(
        Ansi.features()
            .identifier_quotes
            .contains(&IdentifierQuote::Symmetric('"')),
        "ANSI quotes identifiers with double quotes",
    );
}

fn assert_custom_identifier_quote_diverges() {
    assert_ne!(
        BACKTICK_QUOTE_FEATURES.identifier_quotes,
        Ansi.features().identifier_quotes,
    );
}

fn assert_ansi_default_null_ordering_last() {
    assert_eq!(
        Ansi.features().default_null_ordering,
        NullOrdering::NullsLast,
    );
}

fn assert_custom_default_null_ordering_diverges() {
    assert_ne!(
        NULLS_FIRST_FEATURES.default_null_ordering,
        Ansi.features().default_null_ordering,
    );
}

fn assert_ansi_byte_class_marks_operator() {
    assert!(Ansi.features().has_byte_class(b'+', CLASS_OPERATOR));
}

fn assert_custom_byte_class_accepts_extra_identifier_start() {
    assert!(!Ansi.features().has_byte_class(b'@', CLASS_IDENTIFIER_START));
    assert!(CUSTOM_BYTE_CLASS_FEATURES.has_byte_class(b'@', CLASS_IDENTIFIER_START));
}

fn assert_ansi_binding_power_orders_string_concat() {
    let concat = Ansi.features().binding_power(&BinaryOperator::StringConcat);
    let additive = Ansi.features().binding_power(&BinaryOperator::Plus);

    assert!(concat.left < additive.left);
}

fn assert_custom_binding_power_diverges() {
    assert_ne!(
        CUSTOM_BINDING_POWER_FEATURES
            .binding_power(&BinaryOperator::StringConcat)
            .left,
        Ansi.features()
            .binding_power(&BinaryOperator::StringConcat)
            .left,
    );
}

fn assert_ansi_set_operation_power_ranks_intersect_above_union() {
    let union = Ansi
        .features()
        .set_operation_binding_power(&SetOperator::Union);
    let intersect = Ansi
        .features()
        .set_operation_binding_power(&SetOperator::Intersect);

    assert!(union.right < intersect.left);
}

fn assert_custom_set_operation_power_diverges() {
    assert_ne!(
        CUSTOM_SET_OPERATION_FEATURES
            .set_operation_binding_power(&SetOperator::Union)
            .left,
        Ansi.features()
            .set_operation_binding_power(&SetOperator::Union)
            .left,
    );
}

fn assert_ansi_string_literal_syntax_standard_only() {
    assert_eq!(Ansi.features().string_literals, StringLiteralSyntax::ANSI);
}

fn assert_postgres_string_literal_syntax_diverges() {
    assert_eq!(
        Postgres.features().string_literals,
        StringLiteralSyntax::POSTGRES,
    );
    assert_ne!(
        Postgres.features().string_literals,
        Ansi.features().string_literals,
    );
}

fn assert_ansi_table_expression_syntax_baseline() {
    assert_eq!(
        Ansi.features().table_expressions,
        TableExpressionSyntax::ANSI,
    );
}

fn assert_postgres_table_expression_syntax_diverges() {
    assert_eq!(
        Postgres.features().table_expressions,
        TableExpressionSyntax::POSTGRES,
    );
    assert_ne!(
        Postgres.features().table_expressions,
        Ansi.features().table_expressions,
    );
}

// --- semantic-default escape hatch (the documented, bounded residual trust surface) ---
//
// `DefaultNullOrdering` (a downstream semantic default) and `IdentifierCasing` (observable
// only as folded-identifier text, not an accept/reject or structural *parse* differential)
// have no parse/tokenize/render observation the harness could derive a kind from. These
// four asserts — routed through `Coverage::SemanticDefault` and pinned to exactly two
// features by `semantic_escape_hatch_is_limited_to_the_two_documented_features` — are the
// only coverage the objectivity gate still trusts on a hand-set basis.

fn assert_ansi_identifier_casing_folds_upper() {
    assert_eq!(
        Ansi.features().fold_unquoted_identifier("MiXeD"),
        "MIXED",
        "ANSI identity folding uppercases unquoted identifiers",
    );
}

fn assert_postgres_identifier_casing_folds_lower() {
    assert_eq!(
        Postgres.features().fold_unquoted_identifier("MiXeD"),
        "mixed",
        "PostgreSQL identity folding lowercases unquoted identifiers",
    );
    assert_ne!(
        Postgres.features().fold_unquoted_identifier("MiXeD"),
        Ansi.features().fold_unquoted_identifier("MiXeD"),
    );
}

fn assert_ansi_default_null_ordering_places_nulls_last() {
    assert!(
        !Ansi.features().default_nulls_first(),
        "ANSI's M1 default sorts nulls last",
    );
}

fn assert_custom_default_null_ordering_places_nulls_first() {
    assert!(
        NULLS_FIRST_FEATURES.default_nulls_first(),
        "custom null ordering changes default null placement",
    );
}

/// Whether `cases` carry an *objective* behaviour case of `polarity` for `feature`: a
/// harness-run [`Coverage::Behavior`] (whose kind is derived from an actual
/// `parse_with`/`tokenize_with`/render run) or the documented
/// [`Coverage::SemanticDefault`] escape hatch — never a [`Coverage::Metadata`] divergence
/// assertion.
///
/// This is the production-gate predicate behind
/// `every_feature_has_positive_and_negative_coverage` (ADR-0015): a flag covered only by
/// metadata (two [`FeatureSet`]s compared unequal) does not satisfy it, so adding a
/// `FeatureSet` flag without a real accept/reject-or-structural test fails the build.
/// `cases` is a parameter — not hard-wired to `COVERAGE_CASES` — so the teeth test
/// `objective_gate_rejects_metadata_only_coverage` can drive the *same* predicate over
/// synthetic case sets, keeping the proof from drifting away from the gate.
///
/// # Trust model (tag-independent)
///
/// The predicate never trusts a hand-set kind tag. Behaviour is carried as [`Probe`]
/// data (SQL + the recorded accept/reject-or-structural outcome) that [`confirm_case`]
/// runs through the harness and checks (`coverage_cases_are_executable`), so a case counts
/// as [`Coverage::Behavior`] only because it *actually exercises* the harness with its
/// recorded outcome — `behavior_kind_is_harness_derived` proves a case that does not is
/// rejected. A metadata-only assert is a [`Coverage::Metadata`] `fn` this predicate never
/// counts, and it cannot be smuggled in as behaviour: [`Coverage::Behavior`] takes probes,
/// not a `fn`.
///
/// The one remaining hand-set trust surface is [`Coverage::SemanticDefault`]: the two
/// features with no parse/tokenize/render differential — `DefaultNullOrdering` and
/// `IdentifierCasing` — assert their semantic default directly. That residual is explicit,
/// named, and pinned to exactly those two features by
/// `semantic_escape_hatch_is_limited_to_the_two_documented_features`, so it cannot grow
/// silently. The composite features additionally keep the un-fakeable `LABELED_CASES`
/// backstop (`every_gated_subflag_is_required_by_a_labeled_case` +
/// `declared_features_are_genuinely_required`, which flips a feature and re-runs
/// `parse_with`).
fn has_objective_behavior(cases: &[CoverageCase], feature: Feature, polarity: Polarity) -> bool {
    cases.iter().any(|case| {
        case.feature == feature
            && case.polarity == polarity
            && case.coverage.is_objective_behavior()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_cases_are_executable() {
        // Runs every metadata/semantic assert and, for each behaviour case, runs its
        // probes through the harness and confirms each recorded outcome — so a behaviour
        // case whose kind the harness cannot reproduce fails here (see `confirm_case`).
        for case in COVERAGE_CASES {
            confirm_case(case).unwrap_or_else(|reason| panic!("{reason}"));
        }
    }

    /// The MySQL `CREATE TABLE` storage decorations parse, render, and round-trip
    /// under [`MySql`], and are rejected by ANSI/PostgreSQL. Exercises every
    /// table-option value form (bareword, number, string) and the column attribute in
    /// one statement, driving the render + structural-compare path (ADR-0011/0006).
    #[test]
    fn mysql_table_options_and_auto_increment_round_trip_and_gate() {
        let sql = "CREATE TABLE t (id INT AUTO_INCREMENT PRIMARY KEY) \
                   ENGINE=InnoDB AUTO_INCREMENT=100 DEFAULT CHARSET=utf8mb4 COMMENT='x'";

        // Parses and round-trips (canonical + fully-parenthesized) under MySQL.
        match crate::corpus_roundtrip::roundtrip(sql, MySql) {
            crate::corpus_roundtrip::Roundtrip::Ok => {}
            crate::corpus_roundtrip::Roundtrip::Unparsable => {
                panic!("MySQL rejected its own table options + AUTO_INCREMENT: {sql:?}")
            }
            crate::corpus_roundtrip::Roundtrip::Failed(diff) => panic!("{diff}"),
        }

        // ANSI/PostgreSQL reject both the trailing options and the column attribute:
        // the gating flag is off, so the keyword is left unconsumed -> leftover input.
        for reject in [
            "CREATE TABLE t (id INT) ENGINE=InnoDB",
            "CREATE TABLE t (id INT AUTO_INCREMENT)",
        ] {
            parse_with(reject, Ansi).expect_err("ANSI rejects MySQL table-option syntax");
            parse_with(reject, Postgres).expect_err("PostgreSQL rejects MySQL table-option syntax");
        }
    }

    #[test]
    fn copy_statement_is_postgres_gated_across_shipped_dialects() {
        // COPY is a PostgreSQL utility statement: PostgreSQL dispatches it, while ANSI
        // (the generic baseline) and MySQL gate it off, so the leading keyword is an
        // unknown statement there. Locks the shipped-preset matrix onto the real
        // `Postgres`/`Ansi`/`MySql` dialects, beside the `UtilitySyntax` coverage cases
        // that assert the same at the `FeatureSet` level.
        let sql = "COPY t TO STDOUT";
        parse_with(sql, Postgres).expect("PostgreSQL accepts COPY");
        parse_with(sql, Ansi).expect_err("ANSI gates COPY off");
        parse_with(sql, MySql).expect_err("MySQL gates COPY off");
    }

    #[test]
    fn comment_on_statement_is_postgres_gated_across_shipped_dialects() {
        // COMMENT ON is a PostgreSQL utility statement, gated exactly like COPY:
        // PostgreSQL dispatches it while ANSI and MySQL gate it off, so the leading
        // `COMMENT` keyword is an unknown statement there.
        let sql = "COMMENT ON TABLE t IS 'note'";
        parse_with(sql, Postgres).expect("PostgreSQL accepts COMMENT ON");
        parse_with(sql, Ansi).expect_err("ANSI gates COMMENT ON off");
        parse_with(sql, MySql).expect_err("MySQL gates COMMENT ON off");
    }

    #[test]
    fn mysql_rejects_zero_length_backtick_identifiers() {
        // reject-zero-length-delimited-identifier-pg-mysql-parity: MySQL rejects an
        // empty backtick-quoted identifier at scan time, the same way PostgreSQL
        // rejects an empty `"..."` one (asserted against the real parser oracle in
        // `pg.rs`) — SQL's `<delimited identifier body>` requires at least one
        // character, unconditionally, so every identifier position is covered here: a
        // projection item, a qualified column reference, an `AS` alias, and a table
        // name.
        for sql in [
            "SELECT `` FROM t",
            "SELECT x.`` FROM t",
            "SELECT a AS `` FROM t",
            "SELECT * FROM ``",
        ] {
            parse_with(sql, MySql).expect_err("MySQL rejects an empty backtick identifier");
        }

        // Non-regression: a non-empty backtick identifier still parses under MySQL.
        parse_with("SELECT `x` FROM t", MySql).expect("a non-empty backtick identifier is valid");
    }

    #[test]
    fn every_feature_has_positive_and_negative_coverage() {
        for feature in FEATURES {
            // ADR-0015: missing behaviour coverage for a *stable* feature is a build
            // failure. Keying on maturity lets a future Experimental/Preview knob land
            // before its coverage without a red build, while every Stable feature must
            // still carry both polarities. All M1 features are Stable, so this enforces
            // the full matrix today; the key only changes behaviour once a non-stable
            // feature exists.
            if feature.maturity() != Maturity::Stable {
                continue;
            }
            // `has_objective_behavior` excludes `Coverage::Metadata`, so a feature
            // covered only by `assert_ne!(FeatureSetA, FeatureSetB)` divergence
            // assertions fails here — the objectivity requirement. That rejection has
            // teeth in `objective_gate_rejects_metadata_only_coverage`.
            let has_positive = has_objective_behavior(COVERAGE_CASES, *feature, Polarity::Positive);
            let has_negative = has_objective_behavior(COVERAGE_CASES, *feature, Polarity::Negative);

            assert!(
                has_positive && has_negative,
                "feature `{}` lacks behavior coverage: positive={has_positive}, negative={has_negative}",
                feature.id(),
            );
        }
    }

    #[test]
    fn objective_gate_rejects_metadata_only_coverage() {
        // Teeth for `every_feature_has_positive_and_negative_coverage` (ADR-0015): the
        // gate predicate must reject a `FeatureSet` flag shipped with only metadata
        // divergence assertions — or with nothing — so adding a flag without a real
        // accept/reject-or-structural test fails the build instead of being satisfied
        // by `assert_ne!(FeatureSetA, FeatureSetB)`. Driving the *same*
        // `has_objective_behavior` the gate runs, over synthetic case sets, keeps this
        // proof welded to the gate rather than re-stating it.
        fn noop() {}
        // The feature identity is immaterial here; what is under test is that the
        // coverage *kind*, not the feature, decides whether the gate is satisfied.
        const FEATURE: Feature = Feature::DefaultNullOrdering;
        let metadata_case = |polarity: Polarity| CoverageCase {
            feature: FEATURE,
            polarity,
            name: "synthetic_metadata_divergence",
            coverage: Coverage::Metadata(noop),
        };

        // A flag covered only by metadata divergence assertions satisfies neither
        // polarity, so the gate would fail it.
        let metadata_only = [
            metadata_case(Polarity::Positive),
            metadata_case(Polarity::Negative),
        ];
        assert!(!has_objective_behavior(
            &metadata_only,
            FEATURE,
            Polarity::Positive
        ));
        assert!(!has_objective_behavior(
            &metadata_only,
            FEATURE,
            Polarity::Negative
        ));
        // A flag with no coverage at all is likewise rejected.
        assert!(!has_objective_behavior(&[], FEATURE, Polarity::Positive));

        // Control: a harness-run behaviour case and the documented semantic-default escape
        // hatch each satisfy the predicate, so the rejections above turn on the kind of
        // coverage, not an always-false bug.
        let objective = [
            CoverageCase {
                feature: FEATURE,
                polarity: Polarity::Positive,
                name: "synthetic_behavior_positive",
                coverage: Coverage::Behavior(&[Probe::ParseRejects {
                    sql: "SELECT $1",
                    features: &FeatureSet::ANSI,
                }]),
            },
            CoverageCase {
                feature: FEATURE,
                polarity: Polarity::Negative,
                name: "synthetic_semantic_negative",
                coverage: Coverage::SemanticDefault(noop),
            },
        ];
        assert!(has_objective_behavior(
            &objective,
            FEATURE,
            Polarity::Positive
        ));
        assert!(has_objective_behavior(
            &objective,
            FEATURE,
            Polarity::Negative
        ));
    }

    #[test]
    fn behavior_kind_is_harness_derived() {
        // The core teeth of the tag-independent gate: a `Behavior` case's kind is decided
        // by the harness actually running its probes and observing the recorded outcome
        // (`coverage_cases_are_executable` runs `confirm_case` over every real case). A
        // case that claims `Behavior` but does not run the harness with its recorded
        // outcome fails confirmation — the mis-tag the parent spike smuggled past the
        // tag-only gate.
        let case = |name, coverage| CoverageCase {
            feature: Feature::DefaultNullOrdering,
            polarity: Polarity::Positive,
            name,
            coverage,
        };

        // (1) A `Behavior` case that runs no probe exercises no harness observation, so
        //     its kind cannot be confirmed.
        assert!(confirm_case(&case("tamper_no_probe", Coverage::Behavior(&[]))).is_err());

        // (2) A `Behavior` case whose recorded outcome the harness does not reproduce — a
        //     probe claiming ANSI *accepts* `SELECT $1`, which it rejects — fails: the kind
        //     is decided by running `parse_with`, not by the `Behavior` label.
        let false_outcome = case(
            "tamper_false_outcome",
            Coverage::Behavior(&[Probe::ParseAccepts {
                sql: "SELECT $1",
                features: &FeatureSet::ANSI,
            }]),
        );
        assert!(confirm_case(&false_outcome).is_err());

        // Control: the genuine outcome (ANSI rejects `SELECT $1`) is confirmed, so the
        // rejections above turn on the harness result, not an always-err bug.
        let genuine = case(
            "tamper_true_outcome",
            Coverage::Behavior(&[Probe::ParseRejects {
                sql: "SELECT $1",
                features: &FeatureSet::ANSI,
            }]),
        );
        assert!(confirm_case(&genuine).is_ok());
    }

    #[test]
    fn semantic_escape_hatch_is_limited_to_the_two_documented_features() {
        // The escape hatch is the explicit, bounded residual trust surface: only
        // `DefaultNullOrdering` and `IdentifierCasing` have no parse/tokenize/render
        // differential, so only they may assert a semantic default via
        // `Coverage::SemanticDefault` instead of running a harness probe. Pinning the set
        // here means any *new* use of the escape hatch is a deliberate, reviewed decision,
        // not silent growth of the residual.
        let escape_hatch: std::collections::BTreeSet<&str> = COVERAGE_CASES
            .iter()
            .filter(|case| matches!(case.coverage, Coverage::SemanticDefault(_)))
            .map(|case| case.feature.id())
            .collect();
        let expected: std::collections::BTreeSet<&str> = [
            Feature::DefaultNullOrdering.id(),
            Feature::IdentifierCasing.id(),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            escape_hatch, expected,
            "the semantic escape hatch must stay limited to the two documented features",
        );
    }

    #[test]
    fn coverage_case_names_are_unique_and_non_empty() {
        for (index, case) in COVERAGE_CASES.iter().enumerate() {
            assert!(!case.name.is_empty());
            assert!(
                COVERAGE_CASES[index + 1..]
                    .iter()
                    .all(|later| later.name != case.name),
                "duplicate coverage case name `{}`",
                case.name,
            );
        }
    }
}
