// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The Snowflake dialect preset (ANSI-derived, deliberately conservative).
//!
//! Snowflake diverges widely across its type, function, and statement surface, and —
//! unlike the five shipped oracle-compared presets — this workspace has **no Snowflake
//! oracle**, so over-acceptance cannot be measured. Conservatism is therefore the honesty
//! bar: this preset derives from [`FeatureSet::ANSI`], the strict standard baseline, and
//! enables only the Snowflake surface that already has a modelled, tested parser gate and
//! clear documentary evidence. Every other axis keeps its ANSI value; a reader can predict
//! from this module exactly what Snowflake accepts beyond the standard, and unsupported
//! Snowflake syntax is a clean reject routed to a focused follow-up ticket, never a silent
//! over-accept.
//!
//! # What this preset adds over ANSI
//!
//! Five grammar gates and the `COPY INTO` utility delta are enabled as data deltas:
//!
//! - [`semi_structured_access`](ExpressionSyntax::semi_structured_access) — the
//!   `base:key[0].field` path syntax over `VARIANT`/`OBJECT`/`ARRAY` columns, Snowflake's
//!   signature semi-structured accessor. This is the flag this preset exists to expose (it
//!   shipped staged and dark, exercised only by a test dialect until now).
//! - [`qualify`](SelectSyntax::qualify) — the `QUALIFY <predicate>` post-window filter.
//!   Snowflake ships `QUALIFY` and lists it in its reserved-keyword set, so the clause is
//!   admitted *and* `QUALIFY` is reserved in every identifier position (see
//!   [`SNOWFLAKE_QUALIFY_RESERVATION`]); the reservation is what lets `FROM t QUALIFY …`
//!   read the clause rather than a table alias named `qualify`.
//! - [`group_by_all`](GroupingSyntax::group_by_all) — the `GROUP BY ALL` clause mode.
//!   Snowflake ships `GROUP BY ALL`; it does **not** ship `ORDER BY ALL`, so
//!   [`order_by_all`](GroupingSyntax::order_by_all) stays off (the two are separate flags for
//!   exactly this reason — the field doc names Snowflake as the engine that adopts one
//!   without the other).
//! - [`connect_by_clause`](SelectSyntax::connect_by_clause) — the Oracle-style
//!   `START WITH … CONNECT BY [PRIOR] col = [PRIOR] col` hierarchical query clause
//!   (Snowflake `CONNECT BY` reference, the citable public grammar). Modelled as the
//!   Oracle superset — either clause order, the after-`WHERE` position, and `NOCYCLE`
//!   (which Snowflake's docs omit) — a documented conservative-direction over-acceptance;
//!   the `PRIOR` operator is scoped to the `CONNECT BY` condition alone.
//! - [`table_json_path`](TableExpressionSyntax::table_json_path) — PartiQL-style `@path`
//!   lookups in `FROM` table-positioned JSON/VARIANT expressions.
//! - [`copy_into`](UtilitySyntax::copy_into) and [`stage_references`](UtilitySyntax::stage_references)
//!   for bulk load/unload statements with staged locations.
//!
//! Snowflake's identifier lexis needs no delta: it folds unquoted identifiers to
//! **upper**case ([`Casing::Upper`], already ANSI's value) and quotes with the standard
//! `"…"` ([`STANDARD_IDENTIFIER_QUOTES`], likewise ANSI). The `semi_structured_access`
//! path rides the `:` trigger, which contends with
//! [`ParameterSyntax::named_colon`](super::ParameterSyntax::named_colon); Snowflake speaks
//! bind variables with `?`/`:name` *outside* SQL text and does not enable colon parameters
//! in-grammar, so `named_colon` stays off (ANSI's value) and the `:` trigger has a single
//! claimant — the lexical-consistency `const` assert below enforces it.

use super::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, ConstraintSyntax, CreateTableClauseSyntax,
    DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FeatureSet, GroupingSyntax,
    IdentifierSyntax, IndexAlterSyntax, JoinSyntax, Keyword, KeywordOperators, KeywordSet,
    MaintenanceSyntax, MutationSyntax, NullOrdering, NumericLiteralSyntax, OperatorSyntax,
    ParameterSyntax, PipeOperator, PredicateSyntax, QueryTailSyntax, RESERVED_BARE_ALIAS,
    RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME, RESERVED_TYPE_NAME, STANDARD_BYTE_CLASSES,
    STANDARD_IDENTIFIER_QUOTES, SelectSyntax, SessionVariableSyntax, ShowSyntax, StatementDdlGates,
    StringFuncForms, StringLiteralSyntax, TableExpressionSyntax, TableFactorSyntax, TargetSpelling,
    TransactionSyntax, TypeNameSyntax, UtilitySyntax, ViewSequenceClauseSyntax,
};
use crate::precedence::{STANDARD_BINDING_POWERS, STANDARD_SET_OPERATION_BINDING_POWERS};

/// `QUALIFY`, reserved by Snowflake (its documented reserved-keyword list rejects the word
/// as any unquoted identifier). Unioned into all four per-position reject sets below,
/// mirroring the engine-probed DuckDB profile: the bare-alias reservation is load-bearing
/// for the grammar — it is what lets `FROM t QUALIFY …` read the clause instead of a table
/// alias named `qualify`, and the column/function/type reservations match Snowflake's
/// "reserved everywhere" status. `AS`-label position stays open (`SELECT 1 AS qualify`),
/// keeping `reserved_as_label` empty like every ANSI-derived preset.
pub const SNOWFLAKE_QUALIFY_RESERVATION: KeywordSet =
    KeywordSet::from_keywords(&[Keyword::Qualify]);

/// `PIVOT`, `UNPIVOT`, and `MATCH_RECOGNIZE` — Snowflake's FROM-clause table operators.
/// Unlike [`SNOWFLAKE_QUALIFY_RESERVATION`], none of these is a Snowflake *reserved*
/// keyword: all three are absent from Snowflake's reserved-keyword list and stay usable
/// as ordinary unquoted identifiers
/// (<https://docs.snowflake.com/en/sql-reference/reserved-keywords>). Each is instead
/// *position-reserved* — recognized as an operator immediately after a table reference —
/// so a bare factor must not swallow the keyword as a correlation alias:
/// `FROM t PIVOT (…)` / `FROM t UNPIVOT (…)`
/// (<https://docs.snowflake.com/en/sql-reference/constructs/pivot>,
/// <https://docs.snowflake.com/en/sql-reference/constructs/unpivot>) and
/// `FROM t MATCH_RECOGNIZE (…)`
/// (<https://docs.snowflake.com/en/sql-reference/constructs/match_recognize>) all attach
/// the operator directly to the `FROM` object.
///
/// As with BigQuery's `BIGQUERY_PIVOT_RESERVATION`, the
/// reservation is confined to the `ColId` axis ([`SNOWFLAKE_RESERVED_COLUMN_NAME`]) — the
/// load-bearing set for bare-alias reachability — and deliberately *not* the function,
/// type, or projection bare-label axes, which Snowflake keeps open (`pivot(1)`,
/// `CAST(1 AS pivot)`, `SELECT 1 pivot` still parse). This is the minimal deviation from
/// the DuckDB `DUCKDB_PIVOT_RESERVATION` (all
/// four positions, because DuckDB's engine genuinely reserves the words); QUALIFY, a real
/// Snowflake reserved keyword, still rides all four via
/// [`SNOWFLAKE_QUALIFY_RESERVATION`]. `MATCH_RECOGNIZE` rides Snowflake alone — BigQuery
/// has no such operator. The reachability cost mirrors BigQuery's: under this preset an
/// unquoted `pivot`/`unpivot`/`match_recognize` is not admitted as a column/table name or
/// table alias — quote it (`"pivot"`) to use it as an identifier there.
pub const SNOWFLAKE_TABLE_OPERATOR_RESERVATION: KeywordSet =
    KeywordSet::from_keywords(&[Keyword::Pivot, Keyword::Unpivot, Keyword::MatchRecognize]);

/// The ANSI column-name reject set plus [`SNOWFLAKE_QUALIFY_RESERVATION`] and the
/// [`SNOWFLAKE_TABLE_OPERATOR_RESERVATION`] `ColId`-axis reservation.
pub const SNOWFLAKE_RESERVED_COLUMN_NAME: KeywordSet = RESERVED_COLUMN_NAME
    .union(SNOWFLAKE_QUALIFY_RESERVATION)
    .union(SNOWFLAKE_TABLE_OPERATOR_RESERVATION);

/// The ANSI function-name reject set plus [`SNOWFLAKE_QUALIFY_RESERVATION`].
pub const SNOWFLAKE_RESERVED_FUNCTION_NAME: KeywordSet =
    RESERVED_FUNCTION_NAME.union(SNOWFLAKE_QUALIFY_RESERVATION);

/// The ANSI type-name reject set plus [`SNOWFLAKE_QUALIFY_RESERVATION`].
pub const SNOWFLAKE_RESERVED_TYPE_NAME: KeywordSet =
    RESERVED_TYPE_NAME.union(SNOWFLAKE_QUALIFY_RESERVATION);

/// The ANSI bare-alias reject set plus [`SNOWFLAKE_QUALIFY_RESERVATION`].
pub const SNOWFLAKE_RESERVED_BARE_ALIAS: KeywordSet =
    RESERVED_BARE_ALIAS.union(SNOWFLAKE_QUALIFY_RESERVATION);

impl SelectSyntax {
    /// Snowflake SELECT surface: the ANSI baseline plus the documented Snowflake
    /// clauses — the `QUALIFY <predicate>` post-window filter, the `GROUP BY ALL` clause
    /// mode, and the Oracle-style `START WITH`/`CONNECT BY` hierarchical query clause.
    /// `ORDER BY ALL` is deliberately *not* enabled: Snowflake ships `GROUP BY ALL`
    /// without `ORDER BY ALL` (see [`order_by_all`](GroupingSyntax::order_by_all)).
    /// Every other SELECT knob is conservatively ANSI.
    pub const SNOWFLAKE: Self = Self {
        qualify: true,
        // Snowflake's `START WITH … CONNECT BY [PRIOR] col = [PRIOR] col` hierarchical
        // query clause (Snowflake `CONNECT BY` reference — the citable public grammar,
        // there being no Oracle preset). Modelled as the Oracle superset (either clause
        // order, the after-`WHERE` position, `NOCYCLE`); see
        // [`connect_by_clause`](SelectSyntax::connect_by_clause) for the acceptance bound.
        connect_by_clause: true,
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
        explicit_table: true,
        parenthesized_query_operands: true,
        values_rows_require_equal_arity: false,
        values_row_constructor: true,
        as_alias_rejects_reserved: false,
        trailing_comma: false,
        prefix_colon_alias: false,
        lateral_view_clause: false,
    };
}

impl QueryTailSyntax {
    /// The `SNOWFLAKE` preset for query tail syntax.
    pub const SNOWFLAKE: Self = Self {
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

impl TableFactorSyntax {
    /// Snowflake table-factor surface: the ANSI baseline plus the standard PIVOT table
    /// factor (`FROM t PIVOT(<agg> FOR <col> IN (<vals> | ANY [ORDER BY …] | <subquery>)
    /// [DEFAULT ON NULL (<expr>)]))`). Snowflake has no differential oracle here, so the
    /// gate follows the documented grammar on this conservative preset (the `qualify`
    /// precedent). The DuckDB [`pivot`](TableFactorSyntax::pivot) flag stays off — Snowflake has no
    /// leading-keyword `PIVOT` statement, `IN <enum>`, or multi-`FOR`-column form.
    pub const SNOWFLAKE: Self = Self {
        pivot_value_sources: true,
        // The SQL:2016 MATCH_RECOGNIZE row-pattern table factor (documented; no
        // differential oracle here, so the gate follows the grammar on this
        // conservative preset — the `qualify`/`pivot_value_sources` precedent).
        match_recognize: true,
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
        open_json: false,
    };
}

impl GroupingSyntax {
    /// The `SNOWFLAKE` preset for grouping syntax.
    pub const SNOWFLAKE: Self = Self {
        group_by_all: true,
        grouping_sets: true,
        with_rollup: false,
        order_by_using: false,
        group_by_set_quantifier: false,
        order_by_all: false,
    };
}

impl UtilitySyntax {
    /// Snowflake utility surface: the ANSI baseline plus the `COPY INTO` bulk
    /// load/unload statement. Every other utility knob is conservatively ANSI — the
    /// PostgreSQL/DuckDB `COPY`, `COMMENT ON`, the SQLite/MySQL statements, and the
    /// prepared-statement lifecycle all stay off.
    pub const SNOWFLAKE: Self = Self {
        copy_into: true,
        stage_references: true,
        copy: false,
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
        use_string_literal_name: false,
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
    /// Transaction-control surface for the `SNOWFLAKE` preset (split from UtilitySyntax).
    pub const SNOWFLAKE: Self = Self {
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
        abort_transaction_alias: false,
        end_transaction_alias: false,
        transaction_release: false,
        transaction_chain: true,
        release_savepoint_keyword_optional: true,
        begin_transaction_mode: false,
        xa_transactions: false,
    };
}

impl ExpressionSyntax {
    /// Snowflake expression surface: the ANSI baseline plus semi-structured path access
    /// (`base:key[0].field`), Snowflake's `VARIANT`/`OBJECT`/`ARRAY` accessor. Every other
    /// expression knob is conservatively ANSI.
    pub const SNOWFLAKE: Self = Self {
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

impl FeatureSet {
    /// Snowflake as ANSI-derived dialect data (see the module docs for the full derivation
    /// rationale and the conservatism bar).
    pub const SNOWFLAKE: Self = Self {
        // Snowflake folds unquoted identifiers to uppercase — the classic upper-folding
        // dialect, which is already ANSI's value (kept explicit as the documented fact).
        identifier_casing: Casing::Upper,
        // Standard `"…"` identifier quoting; Snowflake adds no second delimiter.
        identifier_quotes: STANDARD_IDENTIFIER_QUOTES,
        default_null_ordering: NullOrdering::NullsLast,
        // The reserved-set delta over ANSI: `QUALIFY` is reserved in every identifier
        // position (Snowflake's documented reserved-keyword status), which the `QUALIFY`
        // clause gate depends on to disambiguate `FROM t QUALIFY …`; and the FROM-clause
        // table operators `PIVOT`/`UNPIVOT`/`MATCH_RECOGNIZE` are position-reserved on the
        // `ColId` axis only (see [`SNOWFLAKE_TABLE_OPERATOR_RESERVATION`]) so a bare
        // `FROM t PIVOT (…)` / `FROM t MATCH_RECOGNIZE (…)` reaches the operator instead of
        // aliasing the source. The operators are not Snowflake reserved keywords, so — unlike
        // QUALIFY — they stay out of the function/type/bare-label reject sets below.
        reserved_column_name: SNOWFLAKE_RESERVED_COLUMN_NAME,
        reserved_function_name: SNOWFLAKE_RESERVED_FUNCTION_NAME,
        reserved_type_name: SNOWFLAKE_RESERVED_TYPE_NAME,
        reserved_bare_alias: SNOWFLAKE_RESERVED_BARE_ALIAS,
        reserved_as_label: KeywordSet::EMPTY,
        catalog_qualified_names: true,
        byte_classes: STANDARD_BYTE_CLASSES,
        binding_powers: STANDARD_BINDING_POWERS,
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        // Conservative ANSI string/number/parameter surface: Snowflake's own forms
        // (`$$…$$` dollar-quoting, backslash escapes, `{name:Type}`-free bind syntax) have
        // no modelled gate here and are deferred rather than guessed at without an oracle.
        // Crucially `named_colon` stays off so the `:` trigger belongs solely to
        // `semi_structured_access` (the lexical-consistency assert below enforces it).
        string_literals: StringLiteralSyntax::ANSI,
        numeric_literals: NumericLiteralSyntax::ANSI,
        parameters: ParameterSyntax::ANSI,
        session_variables: SessionVariableSyntax::ANSI,
        identifier_syntax: IdentifierSyntax::ANSI,
        // ANSI table-expression surface plus the PartiQL / SUPER table-position JSON path
        // (`FROM src[0].a`), sqlparser-rs's `supports_partiql`.
        table_expressions: TableExpressionSyntax {
            table_json_path: true,
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
            indexed_by: false,
            prefix_colon_alias: false,
        },
        join_syntax: JoinSyntax::ANSI,
        table_factor_syntax: TableFactorSyntax::SNOWFLAKE,
        // The semi-structured path accessor — the capstone this preset exposes.
        expression_syntax: ExpressionSyntax::SNOWFLAKE,
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
        // The `QUALIFY` and `GROUP BY ALL` clauses.
        select_syntax: SelectSyntax::SNOWFLAKE,
        query_tail_syntax: QueryTailSyntax::SNOWFLAKE,
        grouping_syntax: GroupingSyntax::SNOWFLAKE,
        // The `COPY INTO` bulk load/unload statement.
        utility_syntax: UtilitySyntax::SNOWFLAKE,
        transaction_syntax: TransactionSyntax::SNOWFLAKE,
        show_syntax: ShowSyntax::ANSI,
        maintenance_syntax: MaintenanceSyntax::ANSI,
        access_control_syntax: AccessControlSyntax::ANSI,
        type_name_syntax: TypeNameSyntax::ANSI,
        // No Snowflake-specific Tier-1 output spelling yet; render the portable ANSI
        // canonical type names (a `TargetSpelling::Snowflake` is render work a later
        // ticket owns).
        target_spelling: TargetSpelling::Ansi,
    };
}

/// Prefer [`FeatureSet::SNOWFLAKE`] for struct update.
pub const SNOWFLAKE: FeatureSet = FeatureSet::SNOWFLAKE;

// Compile-time proof the Snowflake preset claims no shared tokenizer trigger twice. Its
// one contended trigger is `:` — claimed by `semi_structured_access` here and by
// `named_colon` when on — and the preset keeps `named_colon` off (ANSI's value), so `:`
// has a single claimant. Every other delta is a contextual grammar gate or a keyword
// reservation with no tokenizer trigger. Kept as a ratchet so a future Snowflake delta
// that *does* add a contending trigger (e.g. enabling colon parameters) fails the build
// here.
const _: () = assert!(FeatureSet::SNOWFLAKE.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: no refinement
// flag rides an unset base, and no two features contend for one parser-position head.
const _: () = assert!(FeatureSet::SNOWFLAKE.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::SNOWFLAKE.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snowflake_is_ansi_plus_the_five_gates_and_the_qualify_reservation() {
        // The preset is ANSI with a documented, closed set of divergent axes: five grammar
        // gates (including `table_json_path`), the `COPY INTO` utility gate, the `QUALIFY`
        // reservation, and the `PIVOT`/`UNPIVOT`/`MATCH_RECOGNIZE` table-operator `ColId`
        // reservation. Asserting the whole rest equals ANSI keeps the "ANSI-derived, every
        // delta documented" claim honest against a future stray edit. Bind to locals so the
        // const reads are not flagged by clippy's `assertions_on_constants`.
        let ansi = FeatureSet::ANSI;
        let sf = FeatureSet::SNOWFLAKE;

        // The four divergent sub-presets.
        assert_eq!(sf.select_syntax, SelectSyntax::SNOWFLAKE);
        assert_ne!(sf.select_syntax, ansi.select_syntax);
        assert_eq!(sf.expression_syntax, ExpressionSyntax::SNOWFLAKE);
        assert_ne!(sf.expression_syntax, ansi.expression_syntax);

        // The reserved-set delta: QUALIFY in every identifier position (a real Snowflake
        // reserved keyword) plus the `PIVOT`/`UNPIVOT`/`MATCH_RECOGNIZE` table operators on
        // the `ColId` axis only (position-reserved, not reserved keywords).
        assert_eq!(sf.reserved_column_name, SNOWFLAKE_RESERVED_COLUMN_NAME);
        assert_ne!(sf.reserved_column_name, ansi.reserved_column_name);
        assert_eq!(sf.reserved_function_name, SNOWFLAKE_RESERVED_FUNCTION_NAME);
        assert_ne!(sf.reserved_function_name, ansi.reserved_function_name);
        assert_eq!(sf.reserved_type_name, SNOWFLAKE_RESERVED_TYPE_NAME);
        assert_ne!(sf.reserved_type_name, ansi.reserved_type_name);
        assert_eq!(sf.reserved_bare_alias, SNOWFLAKE_RESERVED_BARE_ALIAS);
        assert_ne!(sf.reserved_bare_alias, ansi.reserved_bare_alias);
        // Dropping QUALIFY *and* the table operators recovers the ANSI column set verbatim.
        assert_eq!(
            sf.reserved_column_name
                .difference(SNOWFLAKE_QUALIFY_RESERVATION)
                .difference(SNOWFLAKE_TABLE_OPERATOR_RESERVATION),
            ansi.reserved_column_name,
        );
        assert!(sf.reserved_column_name.contains(Keyword::Qualify));
        assert!(sf.reserved_bare_alias.contains(Keyword::Qualify));
        // The table operators are `ColId`-only: reserved as a column/table name and table
        // alias (bare-alias reachability) but not as a function name, type name, or projection
        // bare label (Snowflake's non-reserved status — `pivot(1)`, `SELECT 1 pivot` parse).
        for kw in [Keyword::Pivot, Keyword::Unpivot, Keyword::MatchRecognize] {
            assert!(sf.reserved_column_name.contains(kw));
            assert!(!sf.reserved_function_name.contains(kw));
            assert!(!sf.reserved_type_name.contains(kw));
            assert!(!sf.reserved_bare_alias.contains(kw));
        }
        // The function/type/bare-label sets carry exactly the QUALIFY delta — no table
        // operator leaks in.
        assert_eq!(
            sf.reserved_function_name,
            RESERVED_FUNCTION_NAME.union(SNOWFLAKE_QUALIFY_RESERVATION),
        );
        assert_eq!(
            sf.reserved_type_name,
            RESERVED_TYPE_NAME.union(SNOWFLAKE_QUALIFY_RESERVATION),
        );
        assert_eq!(
            sf.reserved_bare_alias,
            RESERVED_BARE_ALIAS.union(SNOWFLAKE_QUALIFY_RESERVATION),
        );
        // `AS`-label position stays open (`SELECT 1 AS qualify`).
        assert_eq!(sf.reserved_as_label, KeywordSet::EMPTY);

        // Snowflake's identifier lexis needs no delta — both facts equal ANSI's.
        assert_eq!(sf.identifier_casing, Casing::Upper);
        assert_eq!(sf.identifier_casing, ansi.identifier_casing);
        assert_eq!(sf.identifier_quotes, ansi.identifier_quotes);
        // `named_colon` off is the interplay the semi-structured accessor depends on.
        assert!(!sf.parameters.named_colon);

        // Everything else is inherited verbatim from ANSI.
        assert_eq!(sf.string_literals, ansi.string_literals);
        assert_eq!(sf.numeric_literals, ansi.numeric_literals);
        assert_eq!(sf.parameters, ansi.parameters);
        assert_eq!(sf.session_variables, ansi.session_variables);
        assert_eq!(sf.identifier_syntax, ansi.identifier_syntax);
        // Snowflake diverges from ANSI on exactly the PartiQL / SUPER table-position path.
        assert_eq!(
            TableExpressionSyntax {
                table_json_path: false,
                ..sf.table_expressions
            },
            ansi.table_expressions,
        );
        assert!(sf.table_expressions.table_json_path);
        // Table-factor surface: pivot_value_sources + match_recognize (not ANSI).
        assert_eq!(sf.table_factor_syntax, TableFactorSyntax::SNOWFLAKE);
        assert_ne!(sf.table_factor_syntax, ansi.table_factor_syntax);
        assert_eq!(sf.operator_syntax, ansi.operator_syntax);
        assert_eq!(
            sf.view_sequence_clause_syntax,
            ansi.view_sequence_clause_syntax
        );
        assert_eq!(sf.transaction_syntax, ansi.transaction_syntax);
        assert_eq!(sf.call_syntax, ansi.call_syntax);
        assert_eq!(sf.predicate_syntax, ansi.predicate_syntax);
        assert_eq!(sf.mutation_syntax, ansi.mutation_syntax);
        assert_eq!(sf.statement_ddl_gates, ansi.statement_ddl_gates);
        assert_eq!(
            sf.create_table_clause_syntax,
            ansi.create_table_clause_syntax
        );
        assert_eq!(sf.column_definition_syntax, ansi.column_definition_syntax);
        assert_eq!(sf.constraint_syntax, ansi.constraint_syntax);
        assert_eq!(sf.index_alter_syntax, ansi.index_alter_syntax);
        assert_eq!(sf.existence_guards, ansi.existence_guards);
        // The utility surface diverges from ANSI by the `COPY INTO` gate and stage
        // references (`@stage` / `@~` / `@%table`).
        assert_eq!(sf.utility_syntax, UtilitySyntax::SNOWFLAKE);
        assert_ne!(sf.utility_syntax, ansi.utility_syntax);
        assert!(sf.utility_syntax.copy_into);
        assert!(sf.utility_syntax.stage_references);
        assert_eq!(
            UtilitySyntax {
                copy_into: false,
                stage_references: false,
                ..sf.utility_syntax
            },
            ansi.utility_syntax,
        );
        assert_eq!(sf.type_name_syntax, ansi.type_name_syntax);
        assert_eq!(sf.byte_classes, ansi.byte_classes);
        assert_eq!(sf.binding_powers, ansi.binding_powers);
        assert_eq!(sf.target_spelling, ansi.target_spelling);
    }

    #[test]
    fn snowflake_enables_exactly_the_five_staged_gates() {
        // The capstone: semi-structured access, QUALIFY, GROUP BY ALL, table-JSON path,
        // and the Oracle-style CONNECT BY hierarchical query clause are on, and each
        // is off in the ANSI base it derives from — while ORDER BY ALL stays off (Snowflake
        // ships GROUP BY ALL without it). Forcing the five back off recovers the ANSI
        // sub-presets verbatim.
        let ansi = FeatureSet::ANSI;
        let sf = FeatureSet::SNOWFLAKE;

        assert!(sf.expression_syntax.semi_structured_access);
        assert!(!ansi.expression_syntax.semi_structured_access);
        assert!(sf.select_syntax.qualify && !ansi.select_syntax.qualify);
        // The Oracle-style hierarchical query clause; on for Snowflake, off in ANSI.
        assert!(sf.select_syntax.connect_by_clause && !ansi.select_syntax.connect_by_clause);
        assert!(sf.grouping_syntax.group_by_all && !ansi.grouping_syntax.group_by_all);
        assert!(sf.table_expressions.table_json_path);
        assert!(!ansi.table_expressions.table_json_path);
        // Snowflake has no ORDER BY ALL — the flag the field doc names as the split.
        assert!(!sf.grouping_syntax.order_by_all);
        assert_eq!(
            sf.grouping_syntax.order_by_all,
            ansi.grouping_syntax.order_by_all
        );

        assert_eq!(
            SelectSyntax {
                qualify: false,
                connect_by_clause: false,
                ..sf.select_syntax
            },
            ansi.select_syntax,
        );
        assert_eq!(
            GroupingSyntax {
                group_by_all: false,
                ..sf.grouping_syntax
            },
            ansi.grouping_syntax,
        );
        assert_eq!(
            ExpressionSyntax {
                semi_structured_access: false,
                ..sf.expression_syntax
            },
            ansi.expression_syntax,
        );
        assert_eq!(
            TableExpressionSyntax {
                table_json_path: false,
                ..sf.table_expressions
            },
            ansi.table_expressions,
        );
    }

    #[test]
    fn snowflake_is_lexically_consistent_and_dependency_clean() {
        // Both self-consistency registries must be clean: the semi-structured `:` trigger
        // has a single claimant (colon parameters stay off), and none of the five contextual
        // gates rides an unset base flag.
        let sf = FeatureSet::SNOWFLAKE;
        assert_eq!(sf.lexical_conflict(), None);
        assert!(sf.is_lexically_consistent());
        assert_eq!(sf.feature_dependencies(), None);
        assert!(sf.has_satisfied_feature_dependencies());
        assert_eq!(sf.grammar_conflict(), None);
        assert!(sf.has_no_grammar_conflict());
    }
    #[test]
    fn snowflake_closed_delta_axes_match_documented_set() {
        crate::dialect::closed_delta::assert_closed_delta(
            &FeatureSet::ANSI,
            &FeatureSet::SNOWFLAKE,
            &[
                "reserved_column_name",
                "reserved_function_name",
                "reserved_type_name",
                "reserved_bare_alias",
                "table_expressions",
                "table_factor_syntax",
                "expression_syntax",
                "select_syntax",
                "grouping_syntax",
                "utility_syntax",
            ],
        );
    }
}
