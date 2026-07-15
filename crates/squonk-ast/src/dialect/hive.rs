// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The Hive / HiveQL dialect preset (ANSI-derived, deliberately conservative).
//!
//! Apache Hive's HiveQL diverges widely across its type, function, and statement surface, and
//! — unlike the five shipped oracle-compared presets — this workspace has **no Hive oracle**,
//! so over-acceptance cannot be measured. Conservatism is therefore the honesty bar: this
//! preset derives from [`FeatureSet::ANSI`], the strict standard baseline, and enables only the
//! Hive surface that already has a modelled, tested parser gate and clear documentary evidence
//! — in the headline case the flag's *own* doc names Hive as a motivating dialect. Every other
//! axis keeps its ANSI value; a reader can predict from this module exactly what Hive accepts
//! beyond the standard, and unsupported Hive syntax is a clean reject routed to a focused
//! follow-up ticket, never a silent over-accept.
//!
//! # What this preset adds over ANSI
//!
//! Two grammar gates carry the headline Hive surface:
//!
//! - [`sided_semi_anti_join`](JoinSyntax::sided_semi_anti_join) — the
//!   `{LEFT|RIGHT} {SEMI|ANTI} JOIN` sided semi-/anti-join spelling, whose flag doc names
//!   Spark/Hive/Databricks as the motivating family. Hive is the dialect that *originated* the
//!   `LEFT SEMI JOIN`; the Databricks preset already turned this flag on, and Hive is the
//!   second engine preset to enable it. The leading `LEFT`/`RIGHT` is already a reserved join
//!   side, so no reserved-word interplay is needed (the preceding factor's alias can never
//!   swallow it). See the over-acceptance deferral note below.
//! - [`lateral_view_clause`](SelectSyntax::lateral_view_clause) — the
//!   `LATERAL VIEW [OUTER] explode(col) t [AS a, b]` table-generating clause Hive
//!   originated (LanguageManual LateralView: `fromClause: FROM baseTable (lateralView)*`).
//!   The derived-table `LATERAL` factor stays off, so `LATERAL` leads only this clause
//!   under the preset. See the `AS`-optional over-acceptance deferral note below.
//!
//! # The two lexical facts over ANSI
//!
//! - **Backtick identifier quoting.** Hive quotes identifiers with the backtick `` `name` ``
//!   alone (its default `hive.support.quoted.identifiers=column` mode), and its `"…"` and `'…'`
//!   are *both* string literals — HiveQL string literals may be written with single or double
//!   quotes. So [`HIVE_IDENTIFIER_QUOTES`] lists only the backtick (unlike the
//!   SQLite/Databricks/MSSQL bracket-or-double-quote sets, and unlike ANSI's `"…"`), and
//!   [`double_quoted_strings`](StringLiteralSyntax::double_quoted_strings) is correspondingly
//!   **on** so `"x"` lexes as a string, never an identifier — exactly the MySQL default
//!   (`ANSI_QUOTES` off) lexis, and the same shape the BigQuery preset ships. The two facts are
//!   coupled: `"` has a single claimant (the string scanner) precisely because it is absent
//!   from the quote set, the [`DoubleQuoteStringVersusIdentifier`](super::LexicalConflict)
//!   hazard the `const` assert below rules out. The backtick likewise has a single claimant —
//!   no enabled expression grammar lexes a backtick.
//! - **Case folding.** Hive resolves unquoted identifiers case-insensitively (its metastore
//!   lowercases table and column names), so [`identifier_casing`](FeatureSet::identifier_casing)
//!   is [`Casing::Lower`] — the value the [`Casing`] doc prescribes for the "case-preserving
//!   storage, case-insensitive comparison" model it describes for MySQL/T-SQL columns. Hive is
//!   uniformly case-insensitive (it lacks even the case-sensitive-table split that paragraph
//!   flags as the single-fold known limitation), so the fit is exact. The interned text still
//!   renders exactly as written; the fold is identity-only and never affects acceptance.
//!
//! # Deliberately deferred (conservative reject)
//!
//! - **`RIGHT`/`ANTI` sided joins.** Classic Hive documents only `LEFT SEMI JOIN` (and later
//!   `LEFT ANTI JOIN`); the atomic [`sided_semi_anti_join`](JoinSyntax::sided_semi_anti_join) flag also admits the `RIGHT`-sided
//!   spelling and the `ANTI` flavour across both sides — a known conservative-direction
//!   over-acceptance a future side-refinement (or a Hive oracle) would tighten. This mirrors
//!   the deferral the Databricks preset recorded for the same atomic flag; captured on the
//!   owning ticket rather than split into a narrower gate that no measured boundary yet
//!   justifies.
//! - **`AS`-less lateral-view column aliases.** Hive's grammar requires the `AS` before a
//!   lateral view's column aliases; Spark's spells it `AS?`, and the one atomic
//!   [`lateral_view_clause`](SelectSyntax::lateral_view_clause) gate accepts the wider
//!   Spark bound — a known conservative-direction over-acceptance for this preset, the
//!   same class as the sided-join deferral above (see the [`LateralView`](crate::ast::LateralView)
//!   acceptance-bound doc); captured on the owning ticket.
//!
//! Everything else Hive (the `STRUCT`/`ARRAY<…>`/`MAP<…>` complex type surface,
//! `TRANSFORM`/`MAP`/`REDUCE` script operators, `DISTRIBUTE BY`/`SORT BY`/`CLUSTER BY` clauses,
//! `TABLESAMPLE` bucketing, backslash string escapes, …) has no modelled gate and is a clean
//! reject routed to follow-up tickets, never a silent over-accept.

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

/// Hive identifier quoting: the backtick `` `…` `` alone. Hive spells a quoted identifier
/// `` `a` `` (its default `hive.support.quoted.identifiers=column` mode); its `"a"` and `'a'`
/// are *both* string constants, so `"` is deliberately absent here (and
/// [`StringLiteralSyntax::HIVE`] turns `double_quoted_strings` on so `"` unambiguously lexes a
/// string) — the same backtick-only lexis MySQL uses under its default `ANSI_QUOTES`-off mode.
pub const HIVE_IDENTIFIER_QUOTES: &[IdentifierQuote] = &[IdentifierQuote::Symmetric('`')];

impl StringLiteralSyntax {
    /// Hive string surface: the ANSI baseline plus `"…"` double-quoted string constants (HiveQL
    /// string literals may be written with single *or* double quotes, reserving the backtick
    /// for identifiers). Enabling this is what lets [`HIVE_IDENTIFIER_QUOTES`] drop `"` from the
    /// quote set without stranding the byte. Every other string knob is conservatively ANSI —
    /// Hive's backslash escape sequences have no Hive-citing flag doc or oracle here and are
    /// deferred rather than guessed at (`backslash_escapes` stays off).
    pub const HIVE: Self = Self {
        double_quoted_strings: true,
        escape_strings: false,
        dollar_quoted_strings: false,
        national_strings: false,
        backslash_escapes: false,
        unicode_strings: false,
        bit_string_literals: false,
        blob_literals: false,
        charset_introducers: false,
        same_line_adjacent_concat: false,
    };
}

impl TableExpressionSyntax {
    /// Hive table-expression surface: the ANSI baseline plus the sided
    /// `{LEFT|RIGHT} {SEMI|ANTI} JOIN` family, whose flag doc names Hive as a motivating
    /// dialect (Hive originated `LEFT SEMI JOIN`). The side-less DuckDB spelling
    /// (`semi_anti_join`) stays off — it is a different engine family and needs `SEMI`/`ANTI`
    /// bare-alias reservation modelling not done here. Every other table knob is conservatively
    /// ANSI.
    pub const HIVE: Self = Self {
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
        table_version: false,
        table_json_path: false,
        indexed_by: false,
    };
}

impl JoinSyntax {
    /// The `HIVE` preset for join syntax.
    pub const HIVE: Self = Self {
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
    /// The `HIVE` preset for table factor syntax.
    pub const HIVE: Self = Self {
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

impl SelectSyntax {
    /// Hive SELECT surface: the ANSI baseline plus the `LATERAL VIEW` table-generating
    /// clause Hive originated (LanguageManual LateralView). The derived-table `LATERAL`
    /// factor ([`TableFactorSyntax::HIVE`]) stays off, so `LATERAL` leads only this
    /// clause under the preset. Every other SELECT knob is conservatively ANSI.
    pub const HIVE: Self = Self {
        lateral_view_clause: true,
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
        explicit_table: true,
        parenthesized_query_operands: true,
        values_rows_require_equal_arity: false,
        values_row_constructor: true,
        as_alias_rejects_reserved: false,
        trailing_comma: false,
        prefix_colon_alias: false,
        connect_by_clause: false,
    };
}

impl FeatureSet {
    /// Hive / HiveQL as ANSI-derived dialect data (see the module docs for the full derivation
    /// rationale and the conservatism bar).
    pub const HIVE: Self = Self {
        // Hive resolves unquoted identifiers case-insensitively (its metastore lowercases table
        // and column names); `Casing::Lower` is the exact fit (Hive lacks even the
        // case-sensitive-table split the `Casing` known-limitation paragraph flags). Identity
        // only — the interned text still renders exactly as written, so this never affects
        // acceptance.
        identifier_casing: Casing::Lower,
        // The lexical delta over ANSI: backtick-only identifier quoting (with `"` handed to the
        // string scanner via `double_quoted_strings` below).
        identifier_quotes: HIVE_IDENTIFIER_QUOTES,
        default_null_ordering: NullOrdering::NullsLast,
        // No reserved-set delta over ANSI — this conservative preset adds no keyword
        // reservation (the sided-join gate rides the already-reserved LEFT/RIGHT join sides).
        reserved_column_name: RESERVED_COLUMN_NAME,
        reserved_function_name: RESERVED_FUNCTION_NAME,
        reserved_type_name: RESERVED_TYPE_NAME,
        reserved_bare_alias: RESERVED_BARE_ALIAS,
        reserved_as_label: KeywordSet::EMPTY,
        catalog_qualified_names: true,
        byte_classes: STANDARD_BYTE_CLASSES,
        binding_powers: STANDARD_BINDING_POWERS,
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        // `"…"` double-quoted strings (the coupled half of the backtick-only identifier lexis).
        string_literals: StringLiteralSyntax::HIVE,
        numeric_literals: NumericLiteralSyntax::ANSI,
        parameters: ParameterSyntax::ANSI,
        session_variables: SessionVariableSyntax::ANSI,
        identifier_syntax: IdentifierSyntax::ANSI,
        // The sided `{LEFT|RIGHT} {SEMI|ANTI} JOIN` family — the capstone this preset exposes.
        table_expressions: TableExpressionSyntax::HIVE,
        join_syntax: JoinSyntax::HIVE,
        table_factor_syntax: TableFactorSyntax::HIVE,
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
        // The `LATERAL VIEW` table-generating clause — the second capstone this preset
        // exposes; every other SELECT knob is conservatively ANSI.
        select_syntax: SelectSyntax::HIVE,
        query_tail_syntax: QueryTailSyntax::ANSI,
        grouping_syntax: GroupingSyntax::ANSI,
        utility_syntax: UtilitySyntax::ANSI,
        show_syntax: ShowSyntax::ANSI,
        maintenance_syntax: MaintenanceSyntax::ANSI,
        access_control_syntax: AccessControlSyntax::ANSI,
        type_name_syntax: TypeNameSyntax::ANSI,
        // No Hive-specific Tier-1 output spelling yet; render the portable ANSI canonical type
        // names (a `TargetSpelling::Hive` is render work a later ticket owns).
        target_spelling: TargetSpelling::Ansi,
    };
}

/// Prefer [`FeatureSet::HIVE`] for struct update.
pub const HIVE: FeatureSet = FeatureSet::HIVE;

// Compile-time proof the Hive preset claims no shared tokenizer trigger twice. Beyond ANSI it
// adds one lexical trigger — the backtick identifier opener — with a single claimant (no
// enabled expression grammar lexes a backtick), and it hands `"` to the string scanner
// (`double_quoted_strings` on) *while dropping `"` from the identifier quote set*, so `"` also
// keeps a single claimant. The sided-join gate is contextual keyword grammar with no tokenizer
// trigger. Kept as a ratchet so a future Hive delta that *does* add a contending trigger (e.g.
// re-listing `"` as an identifier quote) fails the build here.
const _: () = assert!(FeatureSet::HIVE.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: no refinement
// flag rides an unset base, and no two features contend for one parser-position head.
const _: () = assert!(FeatureSet::HIVE.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::HIVE.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hive_is_ansi_plus_the_two_gates_and_two_lexical_facts() {
        // The preset is ANSI with a documented, closed set of divergent axes: the two lexical
        // facts (case-folding, backtick-only quoting coupled with double-quoted strings), the
        // enabled table-expression gate, and the enabled SELECT gate. Asserting the whole rest
        // equals ANSI keeps the "ANSI-derived, every delta documented" claim honest against a
        // future stray edit. Bind to locals so the const reads are not flagged by clippy's
        // `assertions_on_constants`.
        let ansi = FeatureSet::ANSI;
        let hive = FeatureSet::HIVE;

        // The two lexical facts.
        assert_eq!(hive.identifier_casing, Casing::Lower);
        assert_ne!(hive.identifier_casing, ansi.identifier_casing);
        assert_eq!(hive.identifier_quotes, HIVE_IDENTIFIER_QUOTES);
        assert_ne!(hive.identifier_quotes, ansi.identifier_quotes);
        // The coupling: `"` is dropped from the quote set and handed to the string scanner.
        assert!(hive.string_literals.double_quoted_strings);
        assert!(
            !hive
                .identifier_quotes
                .iter()
                .any(|quote| quote.open() == '"')
        );
        // Backtick is the sole identifier quote; no bracket (unlike SQLite/MSSQL) and no `"`.
        assert!(
            hive.identifier_quotes
                .iter()
                .any(|quote| quote.open() == '`')
        );
        assert_eq!(hive.identifier_quotes.len(), 1);

        // The one divergent string sub-preset (the double-quote coupling).
        assert_eq!(hive.string_literals, StringLiteralSyntax::HIVE);
        assert_ne!(hive.string_literals, ansi.string_literals);
        // The divergent table-expression sub-preset.
        assert_eq!(hive.table_expressions, TableExpressionSyntax::HIVE);
        assert_eq!(hive.join_syntax, JoinSyntax::HIVE);
        assert_ne!(hive.join_syntax, ansi.join_syntax);
        // The divergent SELECT sub-preset (the LATERAL VIEW clause).
        assert_eq!(hive.select_syntax, SelectSyntax::HIVE);
        assert_ne!(hive.select_syntax, ansi.select_syntax);

        // No reserved-set delta: every position is inherited verbatim from ANSI.
        assert_eq!(hive.reserved_column_name, ansi.reserved_column_name);
        assert_eq!(hive.reserved_function_name, ansi.reserved_function_name);
        assert_eq!(hive.reserved_type_name, ansi.reserved_type_name);
        assert_eq!(hive.reserved_bare_alias, ansi.reserved_bare_alias);
        assert_eq!(hive.reserved_as_label, KeywordSet::EMPTY);

        // Everything else is inherited verbatim from ANSI.
        assert_eq!(hive.numeric_literals, ansi.numeric_literals);
        assert_eq!(hive.parameters, ansi.parameters);
        assert_eq!(hive.expression_syntax, ansi.expression_syntax);
        assert_eq!(hive.session_variables, ansi.session_variables);
        assert_eq!(hive.identifier_syntax, ansi.identifier_syntax);
        assert_eq!(hive.operator_syntax, ansi.operator_syntax);
        assert_eq!(hive.call_syntax, ansi.call_syntax);
        assert_eq!(hive.predicate_syntax, ansi.predicate_syntax);
        assert_eq!(hive.mutation_syntax, ansi.mutation_syntax);
        assert_eq!(hive.statement_ddl_gates, ansi.statement_ddl_gates);
        assert_eq!(
            hive.create_table_clause_syntax,
            ansi.create_table_clause_syntax
        );
        assert_eq!(hive.column_definition_syntax, ansi.column_definition_syntax);
        assert_eq!(hive.constraint_syntax, ansi.constraint_syntax);
        assert_eq!(hive.index_alter_syntax, ansi.index_alter_syntax);
        assert_eq!(hive.existence_guards, ansi.existence_guards);
        assert_eq!(hive.utility_syntax, ansi.utility_syntax);
        assert_eq!(hive.type_name_syntax, ansi.type_name_syntax);
        assert_eq!(hive.byte_classes, ansi.byte_classes);
        assert_eq!(hive.binding_powers, ansi.binding_powers);
        assert_eq!(hive.target_spelling, ansi.target_spelling);
        assert_eq!(hive.default_null_ordering, ansi.default_null_ordering);
    }

    #[test]
    fn hive_enables_exactly_the_two_gates_and_double_quote_string() {
        // The capstones: the sided semi-/anti-join family, the LATERAL VIEW clause, and
        // double-quoted strings are on, and each is off in the ANSI base it derives from.
        // Forcing the flags back off recovers the ANSI sub-presets verbatim.
        let ansi = FeatureSet::ANSI;
        let hive = FeatureSet::HIVE;

        assert!(hive.join_syntax.sided_semi_anti_join);
        assert!(!ansi.join_syntax.sided_semi_anti_join);
        // The side-less DuckDB spelling stays off (a different engine family).
        assert!(!hive.join_syntax.semi_anti_join);
        // The LATERAL VIEW clause; the derived-table LATERAL factor stays off, so
        // `LATERAL` leads only the view clause under this preset.
        assert!(hive.select_syntax.lateral_view_clause && !ansi.select_syntax.lateral_view_clause);
        assert!(!hive.table_factor_syntax.lateral);
        assert!(
            hive.string_literals.double_quoted_strings
                && !ansi.string_literals.double_quoted_strings
        );
        // The backtick opener is the lexical gate (an identifier-quote delta, not a bool).
        assert!(
            hive.identifier_quotes
                .iter()
                .any(|quote| quote.open() == '`')
        );
        assert!(
            !ansi
                .identifier_quotes
                .iter()
                .any(|quote| quote.open() == '`')
        );

        assert_eq!(
            JoinSyntax {
                sided_semi_anti_join: false,
                ..hive.join_syntax
            },
            ansi.join_syntax,
        );
        assert_eq!(
            SelectSyntax {
                lateral_view_clause: false,
                ..hive.select_syntax
            },
            ansi.select_syntax,
        );
        assert_eq!(
            StringLiteralSyntax {
                double_quoted_strings: false,
                ..hive.string_literals
            },
            ansi.string_literals,
        );
    }

    #[test]
    fn hive_is_lexically_consistent_and_dependency_clean() {
        // Both self-consistency registries must be clean: the backtick quote has a single
        // claimant, `"` is handed to the string scanner (dropped from the quote set), and the
        // sided-join gate rides no unset base flag.
        let hive = FeatureSet::HIVE;
        assert_eq!(hive.lexical_conflict(), None);
        assert!(hive.is_lexically_consistent());
        assert_eq!(hive.feature_dependencies(), None);
        assert!(hive.has_satisfied_feature_dependencies());
        assert_eq!(hive.grammar_conflict(), None);
        assert!(hive.has_no_grammar_conflict());
    }
}
