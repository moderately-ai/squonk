// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The ClickHouse dialect preset (ANSI-derived, deliberately conservative).
//!
//! ClickHouse is PostgreSQL-adjacent in its expression syntax but diverges widely
//! across its statement, type, and function surface, and — unlike the five shipped
//! oracle-compared presets — this workspace has **no ClickHouse oracle**, so
//! over-acceptance cannot be measured. Conservatism is therefore the honesty bar: this
//! preset derives from [`FeatureSet::ANSI`], the strict standard baseline, and enables
//! only the ClickHouse surface that already has a modelled, tested parser gate. Every
//! other axis keeps its ANSI value; a reader can predict from this module exactly what
//! ClickHouse accepts beyond the standard, and unsupported ClickHouse syntax is a clean
//! reject routed to a focused follow-up ticket, never a silent over-accept.
//!
//! # What this preset adds over ANSI
//!
//! Nine ClickHouse features, each staged (Lenient-only) behind its own parser gate
//! and turned on here — the query tails [`limit_by_clause`](QueryTailSyntax::limit_by_clause),
//! [`settings_clause`](QueryTailSyntax::settings_clause), and
//! [`format_clause`](QueryTailSyntax::format_clause), and the type constructors
//! [`nullable_type`](TypeNameSyntax::nullable_type),
//! [`low_cardinality_type`](TypeNameSyntax::low_cardinality_type),
//! [`fixed_string_type`](TypeNameSyntax::fixed_string_type),
//! [`datetime64_type`](TypeNameSyntax::datetime64_type),
//! [`nested_type`](TypeNameSyntax::nested_type), and
//! [`bit_width_integer_names`](TypeNameSyntax::bit_width_integer_names) — plus two
//! lexical facts with clear documentary evidence: ClickHouse quotes identifiers with
//! **both** backticks and double quotes, and it is case-sensitive (no identity fold).
//!
//! All nine grammar gates are contextual — each type keyword is matched by spelling with
//! a `(`-lookahead and each query tail by a contextual keyword at a clause boundary — so
//! none reserves a word and none claims a new tokenizer trigger. The identifier-quote
//! delta is the only lexical change, and `double_quoted_strings` stays off (ANSI's
//! value) so `"…"` reads as an identifier, keeping the preset lexically consistent (the
//! `const` assert below).

use super::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, ConstraintSyntax, CreateTableClauseSyntax,
    DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FeatureSet, GroupingSyntax,
    IdentifierQuote, IdentifierSyntax, IndexAlterSyntax, JoinSyntax, KeywordOperators, KeywordSet,
    MaintenanceSyntax, MutationSyntax, NullOrdering, NumericLiteralSyntax, OperatorSyntax,
    ParameterSyntax, PipeOperator, PredicateSyntax, QueryTailSyntax, RESERVED_BARE_ALIAS,
    RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME, RESERVED_TYPE_NAME, STANDARD_BYTE_CLASSES,
    SelectSyntax, SessionVariableSyntax, ShowSyntax, StatementDdlGates, StringFuncForms,
    StringLiteralSyntax, TableExpressionSyntax, TableFactorSyntax, TargetSpelling, TypeNameSyntax,
    UtilitySyntax,
};
use crate::precedence::{STANDARD_BINDING_POWERS, STANDARD_SET_OPERATION_BINDING_POWERS};

/// ClickHouse identifier quoting: the SQL-standard `"…"` **and** the MySQL-style
/// backtick `` `…` ``, both at once. ClickHouse accepts either delimiter for a quoted
/// identifier; the two openers are distinct bytes, so their order is immaterial. `"`
/// stays a quote here (and `double_quoted_strings` is correspondingly off in
/// [`StringLiteralSyntax::ANSI`], which this preset keeps), so `"x"` is an identifier,
/// never a string.
pub const CLICKHOUSE_IDENTIFIER_QUOTES: &[IdentifierQuote] = &[
    IdentifierQuote::Symmetric('"'),
    IdentifierQuote::Symmetric('`'),
];

impl SelectSyntax {
    /// ClickHouse SELECT surface: the ANSI baseline plus the three ClickHouse query
    /// tails, each a contextual clause at the query boundary that shadows no existing
    /// reading (a plain `LIMIT n` still parses; `SETTINGS`/`FORMAT` are contextual, so
    /// they divert only with the gate on). Every other SELECT knob is conservatively
    /// ANSI: ClickHouse's further SELECT extensions (e.g. `WITH FILL`, `ARRAY JOIN`,
    /// `SAMPLE`) have no modelled gate and are deferred to focused tickets.
    pub const CLICKHOUSE: Self = Self {
        distinct_on: false,
        select_into: false,
        empty_target_list: false,
        qualify: false,
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
        lateral_view_clause: false,
        connect_by_clause: false,
    };
}

impl QueryTailSyntax {
    /// The `CLICKHOUSE` preset for query tail syntax.
    pub const CLICKHOUSE: Self = Self {
        limit_by_clause: true,
        settings_clause: true,
        format_clause: true,
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
        for_xml_json_clause: false,
    };
}

impl GroupingSyntax {
    /// The `CLICKHOUSE` preset for grouping syntax.
    pub const CLICKHOUSE: Self = Self {
        grouping_sets: true,
        with_rollup: false,
        order_by_using: false,
        group_by_all: false,
        group_by_set_quantifier: false,
        order_by_all: false,
    };
}

impl TypeNameSyntax {
    /// ClickHouse type surface: the ANSI baseline plus the six ClickHouse type
    /// constructors, each a contextual keyword + `(`-lookahead form (a bare `Nullable`
    /// or `Int256` with no `(` stays an ordinary type/column name), so none reserves a
    /// word. Every other type knob is conservatively ANSI: ClickHouse types with no
    /// modelled gate (`Array(T)`, `Map(K, V)`, `Tuple(...)`, `Enum8`/`Enum16`,
    /// `Decimal32`, `AggregateFunction`, …) are deferred to focused tickets.
    pub const CLICKHOUSE: Self = Self {
        nullable_type: true,
        low_cardinality_type: true,
        fixed_string_type: true,
        datetime64_type: true,
        nested_type: true,
        bit_width_integer_names: true,
        extended_scalar_type_names: false,
        enum_type: false,
        set_type: false,
        numeric_modifiers: false,
        integer_display_width: false,
        composite_types: false,
        varchar_requires_length: false,
        zoned_temporal_types: true,
        empty_type_parens: false,
        character_set_annotation: false,
        signed_type_modifier: false,
        liberal_type_names: false,
        string_type_modifiers: false,
        angle_bracket_types: false,
    };
}

impl FeatureSet {
    /// ClickHouse as ANSI-derived dialect data (see the module docs for the full
    /// derivation rationale and the conservatism bar).
    pub const CLICKHOUSE: Self = Self {
        // ClickHouse is case-sensitive: an unquoted identifier keeps its exact text, so
        // this diverges from ANSI's upper-fold. Identity only — never affects acceptance.
        identifier_casing: Casing::Preserve,
        // The one lexical delta over ANSI: backtick *and* double-quote identifier quoting.
        identifier_quotes: CLICKHOUSE_IDENTIFIER_QUOTES,
        default_null_ordering: NullOrdering::NullsLast,
        // The ANSI/PostgreSQL position-aware reserved model. ClickHouse's own reservation
        // profile is unmodelled here (no oracle to fit it), so the conservative standard
        // sets stand; the nine enabled gates are all contextual and reserve nothing.
        reserved_column_name: RESERVED_COLUMN_NAME,
        reserved_function_name: RESERVED_FUNCTION_NAME,
        reserved_type_name: RESERVED_TYPE_NAME,
        reserved_bare_alias: RESERVED_BARE_ALIAS,
        reserved_as_label: KeywordSet::EMPTY,
        catalog_qualified_names: true,
        byte_classes: STANDARD_BYTE_CLASSES,
        binding_powers: STANDARD_BINDING_POWERS,
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        // Conservative ANSI string/number/parameter surface: ClickHouse's own forms
        // (backslash escapes, `0x` radix, `{name:Type}` parameters) have no modelled
        // gate here and are deferred rather than guessed at without an oracle.
        string_literals: StringLiteralSyntax::ANSI,
        numeric_literals: NumericLiteralSyntax::ANSI,
        parameters: ParameterSyntax::ANSI,
        session_variables: SessionVariableSyntax::ANSI,
        identifier_syntax: IdentifierSyntax::ANSI,
        table_expressions: TableExpressionSyntax::ANSI,
        join_syntax: JoinSyntax::ANSI,
        table_factor_syntax: TableFactorSyntax::ANSI,
        expression_syntax: ExpressionSyntax::ANSI,
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
        // The two divergent sub-presets: the three query tails and the six type
        // constructors (the capstone this preset exists to expose).
        select_syntax: SelectSyntax::CLICKHOUSE,
        query_tail_syntax: QueryTailSyntax::CLICKHOUSE,
        grouping_syntax: GroupingSyntax::CLICKHOUSE,
        utility_syntax: UtilitySyntax::ANSI,
        show_syntax: ShowSyntax::ANSI,
        maintenance_syntax: MaintenanceSyntax::ANSI,
        access_control_syntax: AccessControlSyntax::ANSI,
        type_name_syntax: TypeNameSyntax::CLICKHOUSE,
        // No ClickHouse-specific Tier-1 output spelling yet; render the portable ANSI
        // canonical type names (a `TargetSpelling::ClickHouse` is render work a later
        // ticket owns).
        target_spelling: TargetSpelling::Ansi,
    };
}

/// Prefer [`FeatureSet::CLICKHOUSE`] for struct update.
pub const CLICKHOUSE: FeatureSet = FeatureSet::CLICKHOUSE;

// Compile-time proof the ClickHouse preset claims no shared tokenizer trigger twice.
// Beyond ANSI's lexical surface it adds exactly one trigger — the backtick identifier
// quote — which has a single claimant (no ClickHouse feature enabled here lexes a
// backtick otherwise), and it keeps `double_quoted_strings` off so `"` stays the sole
// identifier quote it already was. Every other delta is a contextual grammar gate with
// no tokenizer trigger. Kept as a ratchet so a future ClickHouse delta that *does* add a
// contending trigger fails the build here.
const _: () = assert!(FeatureSet::CLICKHOUSE.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: no refinement
// flag rides an unset base, and no two features contend for one parser-position head.
const _: () = assert!(FeatureSet::CLICKHOUSE.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::CLICKHOUSE.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clickhouse_is_ansi_plus_the_nine_gates_and_two_lexical_facts() {
        // The preset is ANSI with a documented, closed set of divergent axes: the two
        // lexical facts (case-sensitivity, dual identifier quoting) and the two enabled
        // sub-presets (the query tails and the type constructors). Asserting the whole
        // rest equals ANSI keeps the "ANSI-derived, every delta documented" claim honest
        // against a future stray edit. Bind to locals so the const reads are not flagged
        // by clippy's `assertions_on_constants`.
        let ansi = FeatureSet::ANSI;
        let ch = FeatureSet::CLICKHOUSE;

        // The two lexical facts.
        assert_eq!(ch.identifier_casing, Casing::Preserve);
        assert_ne!(ch.identifier_casing, ansi.identifier_casing);
        assert_eq!(ch.identifier_quotes, CLICKHOUSE_IDENTIFIER_QUOTES);
        assert_ne!(ch.identifier_quotes, ansi.identifier_quotes);
        // `"` stays an identifier quote, so ClickHouse must keep `double_quoted_strings`
        // off (the interplay the preset depends on for lexical consistency).
        assert!(!ch.string_literals.double_quoted_strings);

        // The two divergent sub-presets.
        assert_eq!(ch.query_tail_syntax, QueryTailSyntax::CLICKHOUSE);
        assert_ne!(ch.query_tail_syntax, ansi.query_tail_syntax);
        assert_eq!(ch.type_name_syntax, TypeNameSyntax::CLICKHOUSE);
        assert_ne!(ch.type_name_syntax, ansi.type_name_syntax);

        // Everything else is inherited verbatim from ANSI.
        assert_eq!(ch.string_literals, ansi.string_literals);
        assert_eq!(ch.numeric_literals, ansi.numeric_literals);
        assert_eq!(ch.parameters, ansi.parameters);
        assert_eq!(ch.session_variables, ansi.session_variables);
        assert_eq!(ch.identifier_syntax, ansi.identifier_syntax);
        assert_eq!(ch.table_expressions, ansi.table_expressions);
        assert_eq!(ch.expression_syntax, ansi.expression_syntax);
        assert_eq!(ch.operator_syntax, ansi.operator_syntax);
        assert_eq!(ch.call_syntax, ansi.call_syntax);
        assert_eq!(ch.predicate_syntax, ansi.predicate_syntax);
        assert_eq!(ch.mutation_syntax, ansi.mutation_syntax);
        assert_eq!(ch.statement_ddl_gates, ansi.statement_ddl_gates);
        assert_eq!(
            ch.create_table_clause_syntax,
            ansi.create_table_clause_syntax
        );
        assert_eq!(ch.column_definition_syntax, ansi.column_definition_syntax);
        assert_eq!(ch.constraint_syntax, ansi.constraint_syntax);
        assert_eq!(ch.index_alter_syntax, ansi.index_alter_syntax);
        assert_eq!(ch.existence_guards, ansi.existence_guards);
        assert_eq!(ch.utility_syntax, ansi.utility_syntax);
        assert_eq!(ch.reserved_column_name, ansi.reserved_column_name);
        assert_eq!(ch.reserved_function_name, ansi.reserved_function_name);
        assert_eq!(ch.reserved_type_name, ansi.reserved_type_name);
        assert_eq!(ch.reserved_bare_alias, ansi.reserved_bare_alias);
        assert_eq!(ch.byte_classes, ansi.byte_classes);
        assert_eq!(ch.binding_powers, ansi.binding_powers);
        assert_eq!(ch.target_spelling, ansi.target_spelling);
    }

    #[test]
    fn clickhouse_enables_exactly_the_nine_staged_gates() {
        // The capstone: the three query tails and the six type constructors are on, and
        // each is off in the ANSI base it derives from — so the ClickHouse preset is the
        // shipped home for the nine feature gates, not the Lenient tooling union alone.
        let ansi = FeatureSet::ANSI;
        let ch = FeatureSet::CLICKHOUSE;

        assert!(ch.query_tail_syntax.limit_by_clause && !ansi.query_tail_syntax.limit_by_clause);
        assert!(ch.query_tail_syntax.settings_clause && !ansi.query_tail_syntax.settings_clause);
        assert!(ch.query_tail_syntax.format_clause && !ansi.query_tail_syntax.format_clause);

        assert!(ch.type_name_syntax.nullable_type && !ansi.type_name_syntax.nullable_type);
        assert!(
            ch.type_name_syntax.low_cardinality_type && !ansi.type_name_syntax.low_cardinality_type
        );
        assert!(ch.type_name_syntax.fixed_string_type && !ansi.type_name_syntax.fixed_string_type);
        assert!(ch.type_name_syntax.datetime64_type && !ansi.type_name_syntax.datetime64_type);
        assert!(ch.type_name_syntax.nested_type && !ansi.type_name_syntax.nested_type);
        assert!(
            ch.type_name_syntax.bit_width_integer_names
                && !ansi.type_name_syntax.bit_width_integer_names
        );

        // The enabled sub-presets are ANSI plus exactly those gates and nothing else —
        // forcing the nine back off recovers the ANSI sub-presets verbatim.
        assert_eq!(
            QueryTailSyntax {
                limit_by_clause: false,
                settings_clause: false,
                format_clause: false,
                ..ch.query_tail_syntax
            },
            ansi.query_tail_syntax,
        );
        assert_eq!(
            TypeNameSyntax {
                nullable_type: false,
                low_cardinality_type: false,
                fixed_string_type: false,
                datetime64_type: false,
                nested_type: false,
                bit_width_integer_names: false,
                ..ch.type_name_syntax
            },
            ansi.type_name_syntax,
        );
    }

    #[test]
    fn clickhouse_is_lexically_consistent_and_dependency_clean() {
        // Both self-consistency registries must be clean: the dual identifier quoting
        // and off `double_quoted_strings` introduce no lexical conflict, and none of the
        // nine contextual gates rides an unset base flag.
        let ch = FeatureSet::CLICKHOUSE;
        assert_eq!(ch.lexical_conflict(), None);
        assert!(ch.is_lexically_consistent());
        assert_eq!(ch.feature_dependencies(), None);
        assert!(ch.has_satisfied_feature_dependencies());
        assert_eq!(ch.grammar_conflict(), None);
        assert!(ch.has_no_grammar_conflict());
    }
}
