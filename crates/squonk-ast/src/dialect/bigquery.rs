// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The BigQuery / ZetaSQL dialect preset (ANSI-derived, deliberately conservative).
//!
//! Google BigQuery's GoogleSQL (ZetaSQL) diverges widely across its type, function, and
//! statement surface, and — unlike the five shipped oracle-compared presets — this workspace
//! has **no BigQuery oracle**, so over-acceptance cannot be measured. Conservatism is
//! therefore the honesty bar: this preset derives from [`FeatureSet::ANSI`], the strict
//! standard baseline, and enables only the BigQuery surface that already has a modelled,
//! tested parser gate and clear documentary evidence — in the headline case the flag's *own*
//! doc names BigQuery as the motivating dialect. Every other axis keeps its ANSI value; a
//! reader can predict from this module exactly what BigQuery accepts beyond the standard, and
//! unsupported BigQuery syntax is a clean reject routed to a focused follow-up ticket, never a
//! silent over-accept.
//!
//! # What this preset adds over ANSI
//!
//! Three headline gates carry the BigQuery surface:
//!
//! - [`unnest`](TableFactorSyntax::unnest) — the first-class `UNNEST(<expr>)` table
//!   factor (`FROM UNNEST(…)`), BigQuery's array-to-relation expansion. It is not
//!   BigQuery-exclusive (PostgreSQL/DuckDB/Lenient enable it too), but it is genuine BigQuery
//!   `FROM` surface *and* the base the next gate rides.
//! - [`unnest_with_offset`](TableFactorSyntax::unnest_with_offset) — the
//!   `WITH OFFSET [AS <alias>]` tail on an `UNNEST` factor, a 0-based ordinal column and the
//!   BigQuery counterpart of PostgreSQL's `WITH ORDINALITY`. Its own flag doc records this
//!   surface as on for the BigQuery preset alone — this preset is that home, the first to
//!   enable the flag positively. The gate rides `unnest`
//!   ([`FeatureDependencyViolation::UnnestWithOffsetWithoutUnnest`](super::FeatureDependencyViolation)):
//!   `unnest` is therefore on above, and this is the first shipped preset to exercise that
//!   dependency in the satisfied direction. PostgreSQL and DuckDB both parse-*reject*
//!   `WITH OFFSET` (engine-probed), so the tail is a clean cross-preset reject.
//!
//! - [`angle_bracket_types`](TypeNameSyntax::angle_bracket_types) — type-position support
//!   for BigQuery-style `ARRAY<...>` / `STRUCT<...>` and array-of-struct declarations.
//!
//! One expression gate:
//!
//! - [`struct_constructor`](ExpressionSyntax::struct_constructor) — the `STRUCT(...)`
//!   value constructor (`STRUCT(1, 2)`, `STRUCT(x AS a)`, `STRUCT<a INT64>(1)`), the
//!   documented GoogleSQL tuple builder. Its own flag doc names BigQuery as the motivating
//!   dialect; the `(`/`<` lookahead keeps a bare `struct` an ordinary name, so the gate is
//!   additive over ANSI.
//!
//! # The two lexical facts over ANSI
//!
//! - **Backtick identifier quoting.** BigQuery quotes identifiers with the backtick
//!   `` `name` `` alone — its `"…"` and `'…'` are *both* string literals. So
//!   [`BIGQUERY_IDENTIFIER_QUOTES`] lists only the backtick (unlike the SQLite/Databricks/MSSQL
//!   bracket-or-double-quote sets, and unlike ANSI's `"…"`), and
//!   [`double_quoted_strings`](StringLiteralSyntax::double_quoted_strings) is correspondingly
//!   **on** so `"x"` lexes as a string, never an identifier — exactly the MySQL default
//!   (`ANSI_QUOTES` off) lexis. The two facts are coupled: `"` has a single claimant (the
//!   string scanner) precisely because it is absent from the quote set, the
//!   [`DoubleQuoteStringVersusIdentifier`](super::LexicalConflict) hazard the `const` assert
//!   below rules out. The backtick likewise has a single claimant — no enabled expression
//!   grammar lexes a backtick.
//! - **Case folding.** BigQuery column and alias references resolve case-insensitively (table
//!   and dataset names are case-sensitive), so [`identifier_casing`](FeatureSet::identifier_casing)
//!   is [`Casing::Lower`] — the closest single fit per the [`Casing`] doc's
//!   *known-limitation* paragraph, which names exactly this "case-insensitive column beside a
//!   case-sensitive table" shape (shared with MySQL/T-SQL) as one no single fold can express;
//!   `Casing::Lower` is the value it prescribes. The interned text still renders exactly as
//!   written; the fold is identity-only and never affects acceptance. (The
//!   per-identifier-kind table-vs-column sensitivity split is that documented `Casing`
//!   limitation and a deliberate future extension, not modelled here.)
//!
//! # Deliberately deferred (conservative reject)
//!
//! [`pipe_syntax`](QueryTailSyntax::pipe_syntax) — BigQuery/ZetaSQL query pipe syntax
//! (`FROM t |> WHERE x |> SELECT a`) — stays **off**, and this is a considered judgment, not an
//! oversight. Its own flag doc names the BigQuery preset as the eventual home, but reads
//! the honesty bar the other way for now: the framework ships only the reference `|> WHERE`
//! operator, so enabling the gate today would accept `|> WHERE` while rejecting every other
//! pipe operator — a fragment a reader of this module could not predict, and with no BigQuery
//! oracle the boundary cannot be measured either. The flag doc's own argument (that even the
//! permissive `LENIENT` must wait until the `planner-parity-pipe-*` tickets make the
//! pipe-operator surface coherent) cuts identically for BigQuery. Leaving it off with that
//! reasoning cited is the correct call; flipping it on merely because the doc names BigQuery
//! would ship an incoherent half-surface. The flip is deferred to the pipe-surface tickets.
//!
//! Everything else BigQuery (the `STRUCT<…>`/`ARRAY<…>` *type-position* surface —
//! `CAST(x AS STRUCT<…>)` and column types, distinct from the value constructor above —
//! `SELECT AS STRUCT/VALUE`,
//! `EXCEPT`/`REPLACE` in `SELECT *`, `QUALIFY`, table-name backtick paths with dots,
//! parameterised `@name` / `?` binds, the `SAFE.` function prefix, …) has no modelled gate and
//! is a clean reject routed to follow-up tickets, never a silent over-accept.

use super::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, ConstraintSyntax, CreateTableClauseSyntax,
    DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FeatureSet, GroupingSyntax,
    IdentifierQuote, IdentifierSyntax, IndexAlterSyntax, JoinSyntax, Keyword, KeywordOperators,
    KeywordSet, MaintenanceSyntax, MutationSyntax, NullOrdering, NumericLiteralSyntax,
    OperatorSyntax, ParameterSyntax, PipeOperator, PredicateSyntax, QueryTailSyntax,
    RESERVED_BARE_ALIAS, RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME, RESERVED_TYPE_NAME,
    STANDARD_BYTE_CLASSES, SelectSyntax, SessionVariableSyntax, ShowSyntax, StatementDdlGates, ViewSequenceClauseSyntax,
    StringFuncForms, StringLiteralSyntax, TableExpressionSyntax, TableFactorSyntax, TargetSpelling,
    TypeNameSyntax, TransactionSyntax, UtilitySyntax,
};
use crate::precedence::{STANDARD_BINDING_POWERS, STANDARD_SET_OPERATION_BINDING_POWERS};

/// BigQuery identifier quoting: the backtick `` `…` `` alone. BigQuery spells a quoted
/// identifier `` `a` ``; its `"a"` and `'a'` are *both* string constants, so `"` is
/// deliberately absent here (and [`StringLiteralSyntax::BIGQUERY`] turns
/// `double_quoted_strings` on so `"` unambiguously lexes a string) — the same backtick-only
/// lexis MySQL uses under its default `ANSI_QUOTES`-off mode.
pub const BIGQUERY_IDENTIFIER_QUOTES: &[IdentifierQuote] = &[IdentifierQuote::Symmetric('`')];

/// `PIVOT` and `UNPIVOT`, GoogleSQL's row/column rotation operators. Neither is a
/// BigQuery *reserved* keyword — both are absent from the GoogleSQL reserved-keyword
/// list and stay usable as ordinary unquoted identifiers (GoogleSQL lexical-structure
/// reference, "Reserved keywords":
/// <https://cloud.google.com/bigquery/docs/reference/standard-sql/lexical#reserved_keywords>).
/// They are instead *position-reserved*: the `pivot_operator` / `unpivot_operator`
/// grammar attaches directly to a `from_item`, so `FROM t PIVOT (…)` must read the
/// operator, not a correlation alias named `pivot` (GoogleSQL query-syntax reference,
/// "Pivot operator" / "Unpivot operator":
/// <https://cloud.google.com/bigquery/docs/reference/standard-sql/query-syntax#pivot_operator>).
///
/// Modelling that reachability in this parser's shared-`ColId` alias grammar means
/// reserving both on the `ColId` axis only — [`RESERVED_COLUMN_NAME`], the set a bare
/// *and* `AS`-introduced table alias, a column name, and a table name all draw from —
/// and *not* the function-name, type-name, or projection bare-label axes, which stay
/// open (`pivot(1)`, `CAST(1 AS pivot)`, `SELECT 1 pivot` still parse), matching
/// BigQuery's non-reserved status. This is the deliberate minimal deviation from the
/// DuckDB `DUCKDB_PIVOT_RESERVATION`, which unions into all four positions because
/// DuckDB's engine genuinely classes the words `reserved`. The unavoidable reachability
/// cost: under this preset an unquoted `pivot`/`unpivot` is not admitted as a column/table
/// name or a table alias — quote it (`` `pivot` ``) to use it as an identifier there.
pub const BIGQUERY_PIVOT_RESERVATION: KeywordSet =
    KeywordSet::from_keywords(&[Keyword::Pivot, Keyword::Unpivot]);

/// The ANSI `ColId` reject set plus [`BIGQUERY_PIVOT_RESERVATION`]; see that const for
/// why the reservation is confined to this one axis. The function-name, type-name, and
/// bare-label reject sets take no delta over ANSI, so BigQuery keeps the shared consts
/// for those three positions.
pub const BIGQUERY_RESERVED_COLUMN_NAME: KeywordSet =
    RESERVED_COLUMN_NAME.union(BIGQUERY_PIVOT_RESERVATION);

impl StringLiteralSyntax {
    /// BigQuery string surface: the ANSI baseline plus `"…"` double-quoted string constants
    /// (BigQuery quotes strings with both `'…'` and `"…"`, reserving the backtick for
    /// identifiers). Enabling this is what lets [`BIGQUERY_IDENTIFIER_QUOTES`] drop `"` from the
    /// quote set without stranding the byte. Every other string knob is conservatively ANSI —
    /// BigQuery's backslash escape sequences have no BigQuery-citing flag doc or oracle here and
    /// are deferred rather than guessed at (`backslash_escapes` stays off).
    pub const BIGQUERY: Self = Self {
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
    /// BigQuery table-expression surface: the ANSI baseline plus the `FOR SYSTEM_TIME AS OF`
    /// time-travel modifier. The first-class `UNNEST(…)` factor and its `WITH OFFSET` tail
    /// ride [`TableFactorSyntax`]; every other table knob is conservatively ANSI.
    pub const BIGQUERY: Self = Self {
        // `FROM t FOR SYSTEM_TIME AS OF <ts>` — BigQuery's sole time-travel spelling.
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
        prefix_colon_alias: false,
    };
}

impl JoinSyntax {
    /// The `BIGQUERY` preset for join syntax.
    pub const BIGQUERY: Self = Self {
        stacked_join_qualifiers: true,
        full_outer_join: true,
        natural_cross_join: false,
        straight_join: false,
        asof_join: false,
        positional_join: false,
        semi_anti_join: false,
        sided_semi_anti_join: false,
        apply_join: false,
        recursive_search_cycle: false,
        recursive_union_rejects_order_limit: false,
        recursive_using_key: false,
    };
}

impl ExpressionSyntax {
    /// BigQuery expression surface: the ANSI baseline plus the `STRUCT(...)` value
    /// constructor (`STRUCT(1, 2)`, `STRUCT(x AS a)`, `STRUCT<a INT64>(1)`), a documented
    /// GoogleSQL form with no differential oracle here. Every other expression knob stays
    /// conservatively ANSI.
    pub const BIGQUERY: Self = Self {
        struct_constructor: true,
        typecast_operator: false,
        subscript: false,
        slice_step: false,
        collate: false,
        at_time_zone: false,
        semi_structured_access: false,
        array_constructor: false,
        multidim_array_literals: false,
        collection_literals: false,
        row_constructor: false,
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

impl TableFactorSyntax {
    /// The `BIGQUERY` preset for table factor syntax.
    pub const BIGQUERY: Self = Self {
        unnest: true,
        unnest_with_offset: true,
        // GoogleSQL's `FROM t PIVOT(<agg> FOR <col> IN (<vals>))` table factor. No
        // BigQuery oracle ships here, so the standard-PIVOT gate is enabled on the
        // conservative preset per the documented grammar (the `unnest_with_offset`
        // precedent). BigQuery uses only the explicit value list, but the shared gate
        // also admits `ANY`/subquery and `DEFAULT ON NULL` — over-acceptance that is
        // unmeasurable without an oracle.
        pivot_value_sources: true,
        lateral: false,
        table_functions: false,
        rows_from: false,
        table_function_ordinality: false,
        special_function_table_source: true,
        pivot: false,
        unpivot: false,
        show_ref: false,
        from_values: false,
        json_table: false,
        xml_table: false,
        table_expr_factor: false,
        match_recognize: false,
        open_json: false,
    };
}

impl TypeNameSyntax {
    /// BigQuery type names: ANSI baseline plus angle-bracket `STRUCT<>`/`ARRAY<>`.
    pub const BIGQUERY: Self = Self {
        angle_bracket_types: true,
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
        nullable_type: false,
        low_cardinality_type: false,
        fixed_string_type: false,
        datetime64_type: false,
        nested_type: false,
        bit_width_integer_names: false,
        liberal_type_names: false,
        string_type_modifiers: false,
    };
}

impl FeatureSet {
    /// BigQuery / ZetaSQL as ANSI-derived dialect data (see the module docs for the full
    /// derivation rationale and the conservatism bar).
    pub const BIGQUERY: Self = Self {
        // BigQuery column/alias resolution is case-insensitive (table/dataset names are
        // case-sensitive); `Casing::Lower` is the closest single fit (the `Casing` known-
        // limitation paragraph names this shape). Identity only — the interned text still
        // renders exactly as written, so this never affects acceptance.
        identifier_casing: Casing::Lower,
        // The lexical delta over ANSI: backtick-only identifier quoting (with `"` handed to the
        // string scanner via `double_quoted_strings` below).
        identifier_quotes: BIGQUERY_IDENTIFIER_QUOTES,
        default_null_ordering: NullOrdering::NullsLast,
        // The one reserved-set delta over ANSI: `PIVOT`/`UNPIVOT` are position-reserved on
        // the `ColId` axis so a bare `FROM t PIVOT (…)` reaches the operator instead of
        // aliasing `t` as `pivot` (see [`BIGQUERY_PIVOT_RESERVATION`]). Confined to
        // `reserved_column_name`; the function/type/bare-label positions stay ANSI, matching
        // BigQuery's non-reserved status for the words. (BigQuery's `QUALIFY`/`EXCEPT`-in-star
        // reservations ride gates not modelled here.)
        reserved_column_name: BIGQUERY_RESERVED_COLUMN_NAME,
        reserved_function_name: RESERVED_FUNCTION_NAME,
        reserved_type_name: RESERVED_TYPE_NAME,
        reserved_bare_alias: RESERVED_BARE_ALIAS,
        reserved_as_label: KeywordSet::EMPTY,
        catalog_qualified_names: true,
        byte_classes: STANDARD_BYTE_CLASSES,
        binding_powers: STANDARD_BINDING_POWERS,
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        // `"…"` double-quoted strings (the coupled half of the backtick-only identifier lexis).
        string_literals: StringLiteralSyntax::BIGQUERY,
        numeric_literals: NumericLiteralSyntax::ANSI,
        parameters: ParameterSyntax::ANSI,
        session_variables: SessionVariableSyntax::ANSI,
        identifier_syntax: IdentifierSyntax::ANSI,
        // `FROM UNNEST(…)` and its `WITH OFFSET` tail — the capstone this preset exposes.
        table_expressions: TableExpressionSyntax::BIGQUERY,
        join_syntax: JoinSyntax::BIGQUERY,
        table_factor_syntax: TableFactorSyntax::BIGQUERY,
        expression_syntax: ExpressionSyntax::BIGQUERY,
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
        view_sequence_clause_syntax: ViewSequenceClauseSyntax::ANSI,
        create_table_clause_syntax: CreateTableClauseSyntax::ANSI,
        column_definition_syntax: ColumnDefinitionSyntax::ANSI,
        constraint_syntax: ConstraintSyntax::ANSI,
        index_alter_syntax: IndexAlterSyntax::ANSI,
        existence_guards: ExistenceGuards::ANSI,
        // `pipe_syntax` stays off (deferred judgment — see the module docs); every other SELECT
        // knob is conservatively ANSI.
        select_syntax: SelectSyntax::ANSI,
        query_tail_syntax: QueryTailSyntax::ANSI,
        grouping_syntax: GroupingSyntax::ANSI,
        utility_syntax: UtilitySyntax::ANSI,
        transaction_syntax: TransactionSyntax::ANSI,
        show_syntax: ShowSyntax::ANSI,
        maintenance_syntax: MaintenanceSyntax::ANSI,
        access_control_syntax: AccessControlSyntax::ANSI,
        type_name_syntax: TypeNameSyntax::BIGQUERY,
        // No BigQuery-specific Tier-1 output spelling yet; render the portable ANSI canonical
        // type names (a `TargetSpelling::BigQuery` is render work a later ticket owns).
        target_spelling: TargetSpelling::Ansi,
    };
}

/// Prefer [`FeatureSet::BIGQUERY`] for struct update.
pub const BIGQUERY: FeatureSet = FeatureSet::BIGQUERY;

// Compile-time proof the BigQuery preset claims no shared tokenizer trigger twice. Beyond ANSI
// it adds one lexical trigger — the backtick identifier opener — with a single claimant (no
// enabled expression grammar lexes a backtick), and it hands `"` to the string scanner
// (`double_quoted_strings` on) *while dropping `"` from the identifier quote set*, so `"` also
// keeps a single claimant. The two `UNNEST` gates are contextual keyword grammar with no
// tokenizer trigger. Kept as a ratchet so a future BigQuery delta that *does* add a contending
// trigger (e.g. re-listing `"` as an identifier quote) fails the build here.
const _: () = assert!(FeatureSet::BIGQUERY.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: the
// `unnest_with_offset` tail rides the enabled `unnest` base, and no two features contend
// for one parser-position head.
const _: () = assert!(FeatureSet::BIGQUERY.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::BIGQUERY.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bigquery_is_ansi_plus_the_gates_and_two_lexical_facts() {
        // The preset is ANSI with a documented, closed set of divergent axes: the two lexical
        // facts (case-folding, backtick-only quoting coupled with double-quoted strings), the
        // enabled table-expression/factor gates, the `STRUCT`/`ARRAY` angle-bracket type-position
        // support, and the `PIVOT`/`UNPIVOT` `ColId` reservation.
        // Asserting the whole rest equals ANSI keeps the "ANSI-derived, every delta documented"
        // claim honest against a future stray edit.
        // Bind to locals so the const reads are not flagged by clippy's
        // `assertions_on_constants`.
        let ansi = FeatureSet::ANSI;
        let bq = FeatureSet::BIGQUERY;

        // The two lexical facts.
        assert_eq!(bq.identifier_casing, Casing::Lower);
        assert_ne!(bq.identifier_casing, ansi.identifier_casing);
        assert_eq!(bq.identifier_quotes, BIGQUERY_IDENTIFIER_QUOTES);
        assert_ne!(bq.identifier_quotes, ansi.identifier_quotes);
        // The coupling: `"` is dropped from the quote set and handed to the string scanner.
        assert!(bq.string_literals.double_quoted_strings);
        assert!(!bq.identifier_quotes.iter().any(|quote| quote.open() == '"'));
        // Backtick is the sole identifier quote; no bracket (unlike SQLite/MSSQL) and no `"`.
        assert!(bq.identifier_quotes.iter().any(|quote| quote.open() == '`'));
        assert_eq!(bq.identifier_quotes.len(), 1);

        // The one divergent string sub-preset (the double-quote coupling).
        assert_eq!(bq.string_literals, StringLiteralSyntax::BIGQUERY);
        assert_ne!(bq.string_literals, ansi.string_literals);
        // The divergent table-expression sub-preset.
        assert_eq!(bq.table_expressions, TableExpressionSyntax::BIGQUERY);
        assert_eq!(bq.table_factor_syntax, TableFactorSyntax::BIGQUERY);
        assert_ne!(bq.table_factor_syntax, ansi.table_factor_syntax);

        // The reserved-set delta: `PIVOT`/`UNPIVOT` are added on the `ColId` axis only, so a
        // bare `FROM t PIVOT (…)` reaches the operator. Dropping them recovers the ANSI set
        // verbatim; the other three positions take no delta over ANSI.
        assert_eq!(bq.reserved_column_name, BIGQUERY_RESERVED_COLUMN_NAME);
        assert_ne!(bq.reserved_column_name, ansi.reserved_column_name);
        assert_eq!(
            bq.reserved_column_name
                .difference(BIGQUERY_PIVOT_RESERVATION),
            ansi.reserved_column_name,
        );
        assert!(bq.reserved_column_name.contains(Keyword::Pivot));
        assert!(bq.reserved_column_name.contains(Keyword::Unpivot));
        // Confined to `ColId`: function/type/bare-label positions stay ANSI, so `pivot(1)`,
        // `CAST(1 AS pivot)`, and `SELECT 1 pivot` keep parsing (BigQuery's non-reserved
        // status for the words).
        assert_eq!(bq.reserved_function_name, ansi.reserved_function_name);
        assert_eq!(bq.reserved_type_name, ansi.reserved_type_name);
        assert_eq!(bq.reserved_bare_alias, ansi.reserved_bare_alias);
        assert!(!bq.reserved_function_name.contains(Keyword::Pivot));
        assert!(!bq.reserved_type_name.contains(Keyword::Pivot));
        assert!(!bq.reserved_bare_alias.contains(Keyword::Pivot));
        assert_eq!(bq.reserved_as_label, KeywordSet::EMPTY);

        // Everything else is inherited verbatim from ANSI — including SELECT (pipe_syntax is
        // deferred, so it stays off) and numeric/parameter surfaces.
        assert_eq!(bq.select_syntax, ansi.select_syntax);
        assert!(!bq.query_tail_syntax.pipe_syntax);
        assert_eq!(bq.numeric_literals, ansi.numeric_literals);
        assert_eq!(bq.parameters, ansi.parameters);
        // BigQuery adds the `STRUCT(...)` value constructor; every other expression knob
        // is inherited verbatim from ANSI.
        assert_eq!(bq.expression_syntax, ExpressionSyntax::BIGQUERY);
        assert!(bq.expression_syntax.struct_constructor);
        assert!(!ansi.expression_syntax.struct_constructor);
        assert_eq!(
            ExpressionSyntax {
                struct_constructor: false,
                ..bq.expression_syntax
            },
            ansi.expression_syntax
        );
        assert_eq!(bq.session_variables, ansi.session_variables);
        assert_eq!(bq.identifier_syntax, ansi.identifier_syntax);
        assert_eq!(bq.operator_syntax, ansi.operator_syntax);
        assert_eq!(bq.call_syntax, ansi.call_syntax);
        assert_eq!(bq.predicate_syntax, ansi.predicate_syntax);
        assert_eq!(bq.mutation_syntax, ansi.mutation_syntax);
        assert_eq!(bq.statement_ddl_gates, ansi.statement_ddl_gates);
        assert_eq!(
            bq.create_table_clause_syntax,
            ansi.create_table_clause_syntax
        );
        assert_eq!(bq.column_definition_syntax, ansi.column_definition_syntax);
        assert_eq!(bq.constraint_syntax, ansi.constraint_syntax);
        assert_eq!(bq.index_alter_syntax, ansi.index_alter_syntax);
        assert_eq!(bq.existence_guards, ansi.existence_guards);
        assert_eq!(bq.utility_syntax, ansi.utility_syntax);
        assert!(bq.type_name_syntax.angle_bracket_types);
        assert!(!ansi.type_name_syntax.angle_bracket_types);
        assert_eq!(
            TypeNameSyntax {
                angle_bracket_types: false,
                ..bq.type_name_syntax
            },
            ansi.type_name_syntax,
        );
        assert_eq!(bq.byte_classes, ansi.byte_classes);
        assert_eq!(bq.binding_powers, ansi.binding_powers);
        assert_eq!(bq.target_spelling, ansi.target_spelling);
        assert_eq!(bq.default_null_ordering, ansi.default_null_ordering);
    }

    #[test]
    fn bigquery_enables_exactly_the_unnest_gates_and_double_quote_string() {
        // The capstone: the first-class UNNEST factor, its BigQuery WITH OFFSET tail, and
        // double-quoted strings are on, and each is off in the ANSI base it derives from.
        // Forcing the flags back off recovers the ANSI sub-presets verbatim.
        let ansi = FeatureSet::ANSI;
        let bq = FeatureSet::BIGQUERY;

        assert!(bq.table_factor_syntax.unnest && !ansi.table_factor_syntax.unnest);
        assert!(
            bq.table_factor_syntax.unnest_with_offset
                && !ansi.table_factor_syntax.unnest_with_offset
        );
        assert!(
            bq.string_literals.double_quoted_strings && !ansi.string_literals.double_quoted_strings
        );
        // The backtick opener is the lexical gate (an identifier-quote delta, not a bool).
        assert!(bq.identifier_quotes.iter().any(|quote| quote.open() == '`'));
        assert!(
            !ansi
                .identifier_quotes
                .iter()
                .any(|quote| quote.open() == '`')
        );

        assert_eq!(
            TableFactorSyntax {
                unnest: false,
                unnest_with_offset: false,
                // BigQuery adds the standard PIVOT gate over the ANSI baseline.
                pivot_value_sources: false,
                ..bq.table_factor_syntax
            },
            ansi.table_factor_syntax,
        );
        assert_eq!(
            StringLiteralSyntax {
                double_quoted_strings: false,
                ..bq.string_literals
            },
            ansi.string_literals,
        );
    }

    #[test]
    fn bigquery_is_lexically_consistent_and_dependency_clean() {
        // Both self-consistency registries must be clean: the backtick quote has a single
        // claimant, `"` is handed to the string scanner (dropped from the quote set), and — the
        // fact this preset is the first to prove positively — the `unnest_with_offset` tail
        // rides the enabled `unnest` base, so the feature-dependency registry is satisfied.
        let bq = FeatureSet::BIGQUERY;
        assert_eq!(bq.lexical_conflict(), None);
        assert!(bq.is_lexically_consistent());
        assert_eq!(bq.feature_dependencies(), None);
        assert!(bq.has_satisfied_feature_dependencies());
        assert_eq!(bq.grammar_conflict(), None);
        assert!(bq.has_no_grammar_conflict());
        // Guard the dependency direction explicitly: dropping the `unnest` base while keeping
        // the `WITH OFFSET` tail must trip the registry — this preset is the first whose flip
        // exercises `UnnestWithOffsetWithoutUnnest`.
        let broken = FeatureSet::BIGQUERY.with(
            super::super::FeatureDelta::EMPTY.table_factor_syntax(TableFactorSyntax {
                unnest: false,
                ..bq.table_factor_syntax
            }),
        );
        assert_eq!(
            broken.feature_dependencies(),
            Some(super::super::FeatureDependencyViolation::UnnestWithOffsetWithoutUnnest),
        );
    }
}
