// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The Amazon Redshift dialect preset (ANSI-derived, deliberately conservative).
//!
//! Redshift is genuinely a **PostgreSQL 8 fork** — the honest temptation is to derive it from
//! [`FeatureSet::POSTGRES`]. This preset deliberately does *not*, and the reasoning is the
//! project's evidence bar rather than convenience:
//!
//! - **No Redshift oracle exists.** Like the five other no-oracle presets
//!   (BigQuery/ClickHouse/Snowflake/Databricks/MSSQL/Hive), over-acceptance here cannot be
//!   *measured*. A `POSTGRES`-derived base would inherit our PostgreSQL preset's whole surface —
//!   but that preset is oracle-fitted to **PostgreSQL 17**, decades past the PG-8 fork point, so
//!   it carries modern features Redshift never had (dollar-quoting, `jsonb` operators, SQL/JSON
//!   functions, `MERGE … RETURNING`, quantified `LIKE ANY (array)`, …). Deriving from `POSTGRES`
//!   would silently over-accept every one of those, and each omission would then need its *own*
//!   Redshift evidence to turn back off — a larger, less honest surface than starting strict.
//! - **Conservatism is the honesty bar.** Deriving from [`FeatureSet::ANSI`], the strict standard
//!   baseline, means every divergence from the standard is a documented, evidence-cited decision a
//!   reader can audit from this one module, and unsupported Redshift syntax is a clean reject
//!   routed to a focused follow-up ticket, never a silent over-accept.
//! - **Our flag docs attribute the PG-isms to PostgreSQL, not Redshift.** The evidence bar for
//!   turning a *dialect-attributed* grammar flag on is that our own flag doc names the dialect (as
//!   the sided-join doc names Hive). A sweep of `dialect/mod.rs` finds **zero** Redshift
//!   citations, and the candidate PG-heritage flags (`ilike`, `similar_to`, `distinct_on`,
//!   `qualify`) are each documented as PostgreSQL/DuckDB/Teradata features. Those flags are
//!   therefore conservative-off and deferred (see below) — not because Redshift lacks the
//!   feature, but because turning them on here would assert an unmeasured equivalence.
//!
//! # What this preset adds over ANSI
//!
//! Two axes (both evidence-backed; the rest stays ANSI verbatim):
//!
//! - **Case folding to lowercase.** Redshift resolves unquoted identifiers case-insensitively and
//!   folds them to lowercase (its default `enable_case_sensitive_identifier` is off — the
//!   PostgreSQL-inherited behaviour, differing from PG only in fold *direction* being the same
//!   lowercase), so [`identifier_casing`](FeatureSet::identifier_casing) is [`Casing::Lower`]
//!   rather than ANSI's [`Casing::Upper`]. The fold is identity-only: the interned text still
//!   renders exactly as written and acceptance never changes — this is a name-resolution fact, not
//!   a parse boundary. `Casing` is *dialect-open* (a general folding model, not a flag our docs
//!   tie to one engine), so external Redshift documentation is the admissible evidence.
//! - **Table-position PartiQL / SUPER JSON path**
//!   ([`table_json_path`](TableExpressionSyntax::table_json_path)) — `FROM src[0].a` navigation
//!   of a SUPER column (sqlparser-rs's `supports_partiql` surface). A parse boundary, not just
//!   identity.
//!
//! Lexis remains ANSI: Redshift quotes identifiers with the standard `"…"` (unlike Hive's
//! backtick or MSSQL's bracket) and spells strings with `'…'`, so the ANSI
//! [`STANDARD_IDENTIFIER_QUOTES`] and [`StringLiteralSyntax::ANSI`] are exact and this preset
//! adds no new lexical trigger (the `const` assert below stays clean).
//!
//! # Deliberately deferred (conservative reject)
//!
//! Redshift genuinely accepts each of these (it inherited most from PostgreSQL 8); each is a clean
//! reject here, routed to a follow-up rather than guessed at without an oracle:
//!
//! - **`ILIKE` and `SIMILAR TO`.** Redshift ships both (AWS documents them), but our
//!   [`ilike`](PredicateSyntax::ilike)/[`similar_to`](PredicateSyntax::similar_to) flag docs
//!   attribute them to PostgreSQL and do not name Redshift — dialect-attributed, so external
//!   Redshift evidence is not admissible under the bar. Deferred pending either a Redshift oracle
//!   or a flag-doc citation update.
//! - **`DISTINCT ON`.** Same shape: Redshift inherits PostgreSQL's `SELECT DISTINCT ON (…)`, but
//!   [`distinct_on`](SelectSyntax::distinct_on) is documented as the PostgreSQL extension.
//! - **`QUALIFY`.** Redshift added `QUALIFY`, but our [`qualify`](SelectSyntax::qualify) doc cites
//!   DuckDB (Teradata-origin) and needs the reserved-keyword modelling Snowflake's preset does;
//!   deferred rather than half-modelled.
//! - **The large unmodelled Redshift surface.** `DISTKEY`/`SORTKEY`/`DISTSTYLE`/`ENCODE` table
//!   attributes, `UNLOAD`/`COPY` bulk-load statements, full `SUPER` type DDL (beyond the table-
//!   position JSON path above), `INTERVAL` literal spellings, and Redshift's window-frame
//!   differences all have no modelled gate and are clean rejects routed to follow-up tickets.
//!
//! A reader can predict from this module exactly what Redshift accepts beyond the standard: the
//! lowercase identifier fold and table-position `table_json_path`, and nothing else, until each
//! deferred surface earns its own gate.

use super::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, ConstraintSyntax, CreateTableClauseSyntax,
    DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FeatureSet, GroupingSyntax,
    IdentifierSyntax, IndexAlterSyntax, JoinSyntax, KeywordOperators, KeywordSet,
    MaintenanceSyntax, MutationSyntax, NullOrdering, NumericLiteralSyntax, OperatorSyntax,
    ParameterSyntax, PipeOperator, PredicateSyntax, QueryTailSyntax, RESERVED_BARE_ALIAS,
    RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME, RESERVED_TYPE_NAME, STANDARD_BYTE_CLASSES,
    STANDARD_IDENTIFIER_QUOTES, SelectSyntax, SessionVariableSyntax, ShowSyntax, StatementDdlGates,
    StringFuncForms, StringLiteralSyntax, TableExpressionSyntax, TableFactorSyntax, TargetSpelling,
    TransactionSyntax, TypeNameSyntax, UtilitySyntax, ViewSequenceClauseSyntax,
};
use crate::precedence::{STANDARD_BINDING_POWERS, STANDARD_SET_OPERATION_BINDING_POWERS};

impl FeatureSet {
    /// Amazon Redshift as ANSI-derived dialect data (see the module docs for the full derivation
    /// rationale — including why a PostgreSQL-8 fork still derives from ANSI, not `POSTGRES` — and
    /// the conservatism bar).
    pub const REDSHIFT: Self = Self {
        // Delta 1/2 over ANSI: Redshift folds unquoted identifiers to lowercase (its default
        // `enable_case_sensitive_identifier` off — the PostgreSQL-inherited lowercase model).
        // Identity only: the interned text still renders exactly as written, so this never affects
        // acceptance. `Casing` is dialect-open, so external Redshift docs are the admissible
        // evidence here (unlike the dialect-attributed PG-heritage flags, which stay off).
        identifier_casing: Casing::Lower,
        // Standard `"…"` identifier quoting; Redshift adds no second delimiter (unlike Hive's
        // backtick or MSSQL's bracket), so no lexical trigger is added over ANSI.
        identifier_quotes: STANDARD_IDENTIFIER_QUOTES,
        default_null_ordering: NullOrdering::NullsLast,
        // No reserved-set delta over ANSI — this conservative preset reserves no extra keyword
        // (the deferred `QUALIFY`/`DISTINCT ON` gates that would need reservation are off).
        reserved_column_name: RESERVED_COLUMN_NAME,
        reserved_function_name: RESERVED_FUNCTION_NAME,
        reserved_type_name: RESERVED_TYPE_NAME,
        reserved_bare_alias: RESERVED_BARE_ALIAS,
        reserved_as_label: KeywordSet::EMPTY,
        catalog_qualified_names: true,
        byte_classes: STANDARD_BYTE_CLASSES,
        binding_powers: STANDARD_BINDING_POWERS,
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        // Conservative ANSI string surface: Redshift spells strings with `'…'` (its `"…"` is a
        // quoted identifier, exactly ANSI). Redshift's own extensions have no modelled gate here.
        string_literals: StringLiteralSyntax::ANSI,
        numeric_literals: NumericLiteralSyntax::ANSI,
        parameters: ParameterSyntax::ANSI,
        session_variables: SessionVariableSyntax::ANSI,
        identifier_syntax: IdentifierSyntax::ANSI,
        // ANSI table-expression surface plus the PartiQL / SUPER table-position JSON path
        // (`FROM src[0].a`) navigating a SUPER column, sqlparser-rs's `supports_partiql`.
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
        table_factor_syntax: TableFactorSyntax::ANSI,
        expression_syntax: ExpressionSyntax::ANSI,
        operator_syntax: OperatorSyntax::ANSI,
        call_syntax: CallSyntax::ANSI,
        string_func_forms: StringFuncForms::ANSI,
        aggregate_call_syntax: AggregateCallSyntax::ANSI,
        // `ILIKE`/`SIMILAR TO` are deferred (dialect-attributed to PostgreSQL in our docs, no
        // Redshift citation) — every predicate knob is conservatively ANSI.
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
        // `DISTINCT ON`/`QUALIFY` are deferred (see the module docs) — every SELECT knob is
        // conservatively ANSI.
        select_syntax: SelectSyntax::ANSI,
        query_tail_syntax: QueryTailSyntax::ANSI,
        grouping_syntax: GroupingSyntax::ANSI,
        utility_syntax: UtilitySyntax::ANSI,
        transaction_syntax: TransactionSyntax::ANSI,
        show_syntax: ShowSyntax::ANSI,
        maintenance_syntax: MaintenanceSyntax::ANSI,
        access_control_syntax: AccessControlSyntax::ANSI,
        type_name_syntax: TypeNameSyntax::ANSI,
        // No Redshift-specific Tier-1 output spelling yet; render the portable ANSI canonical type
        // names (a `TargetSpelling::Redshift` is render work a later ticket owns).
        target_spelling: TargetSpelling::Ansi,
    };
}

/// Prefer [`FeatureSet::REDSHIFT`] for struct update.
pub const REDSHIFT: FeatureSet = FeatureSet::REDSHIFT;

// Compile-time proof the Redshift preset claims no shared tokenizer trigger twice. It adds *no*
// lexical trigger over ANSI (standard `"…"` identifier quoting, `'…'` strings — the same lexis as
// the ANSI baseline), so this holds as trivially as ANSI's own assert. Kept as a ratchet so a
// future Redshift delta that *does* add a contending trigger (e.g. `$$…$$` dollar-quoting) fails
// the build here rather than silently shadowing a meaning.
const _: () = assert!(FeatureSet::REDSHIFT.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: no refinement
// flag rides an unset base, and no two features contend for one parser-position head.
const _: () = assert!(FeatureSet::REDSHIFT.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::REDSHIFT.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redshift_is_ansi_plus_only_the_lowercase_fold() {
        // Closed-delta honesty: despite Redshift being a PostgreSQL-8 fork, this preset is
        // ANSI plus exactly two divergent top-level axes (casing + table_json_path).
        let ansi = FeatureSet::ANSI;
        let redshift = FeatureSet::REDSHIFT;

        assert_eq!(redshift.identifier_casing, Casing::Lower);
        assert!(redshift.table_expressions.table_json_path);
        // Deferred PG-heritage flags stay off.
        assert!(!redshift.predicate_syntax.ilike);
        assert!(!redshift.predicate_syntax.similar_to);
        assert!(!redshift.select_syntax.distinct_on);
        assert!(!redshift.select_syntax.qualify);

        crate::dialect::closed_delta::assert_closed_delta(
            &ansi,
            &redshift,
            &["identifier_casing", "table_expressions"],
        );
    }

    #[test]
    fn redshift_recovers_ansi_when_the_fold_is_forced_back() {
        // The two deltas isolated: forcing the fold direction back to ANSI's `Upper` and
        // dropping the PartiQL / SUPER table-position path recovers the ANSI FeatureSet
        // verbatim, proving the lowercase fold and the `table_json_path` grant are the *only*
        // divergences.
        let ansi = FeatureSet::ANSI;
        let redshift = FeatureSet::REDSHIFT;
        assert_eq!(
            FeatureSet {
                identifier_casing: Casing::Upper,
                table_expressions: TableExpressionSyntax {
                    table_json_path: false,
                    ..redshift.table_expressions
                },
                ..redshift
            },
            ansi,
        );
    }

    #[test]
    fn redshift_is_lexically_consistent_and_dependency_clean() {
        // Both self-consistency registries must be clean: adding no lexical trigger over ANSI, the
        // conflict registry is trivially empty, and riding no dependent grammar flag, the
        // dependency registry is empty too.
        let redshift = FeatureSet::REDSHIFT;
        assert_eq!(redshift.lexical_conflict(), None);
        assert!(redshift.is_lexically_consistent());
        assert_eq!(redshift.feature_dependencies(), None);
        assert!(redshift.has_satisfied_feature_dependencies());
        assert_eq!(redshift.grammar_conflict(), None);
        assert!(redshift.has_no_grammar_conflict());
    }
}
