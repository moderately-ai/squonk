// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The MSSQL / T-SQL dialect preset (ANSI-derived, deliberately conservative).
//!
//! Microsoft SQL Server's T-SQL diverges widely across its type, function, and statement
//! surface, and — unlike the five shipped oracle-compared presets — this workspace has **no
//! MSSQL oracle**, so over-acceptance cannot be measured. Conservatism is therefore the
//! honesty bar: this preset derives from [`FeatureSet::ANSI`], the strict standard baseline,
//! and enables only the T-SQL surface that already has a modelled, tested parser gate and
//! clear documentary evidence — in six cases the flag's *own* doc names T-SQL as the
//! motivating dialect. Every other axis keeps its ANSI value; a reader can predict from this
//! module exactly what MSSQL accepts beyond the standard, and unsupported T-SQL syntax is a
//! clean reject routed to a focused follow-up ticket, never a silent over-accept.
//!
//! # What this preset adds over ANSI
//!
//! Seven gates, each documented (in the flag's own doc) as T-SQL surface:
//!
//! - [`apply_join`](JoinSyntax::apply_join) — the `CROSS APPLY` / `OUTER APPLY`
//!   lateral-correlated join operators. This is the flag this preset exists to make real: it
//!   shipped staged (Lenient-only) before this preset gave it an engine home. The leading
//!   `CROSS`/`OUTER` keyword anchors the operator, so no reserved-word interplay is needed
//!   (the preceding factor's alias can never swallow it).
//! - [`table_hints`](TableExpressionSyntax::table_hints) — the `WITH ( <hint>, … )` table-hint
//!   tail (`FROM t WITH (NOLOCK)`, `WITH (INDEX(ix), FORCESEEK)`), a T-SQL locking / optimizer
//!   directive on a table factor. Reached only through the `WITH (` sequence at the
//!   table-factor tail, where `WITH` is never otherwise consumed on a base table, so it never
//!   contends with the leading-`WITH` CTE clause at statement start; when off (ANSI base) the
//!   trailing `WITH` is left unconsumed and the construct is a clean reject. The common
//!   documented hints ([`TableHintKeyword`](crate::ast::TableHintKeyword)) are typed for
//!   planner consumers; an unrecognized word is preserved verbatim
//!   ([`TableHint::Other`](crate::ast::TableHint)). Numeric index ids (`INDEX(0)`) and
//!   the legacy no-`WITH` parenthesized form (`FROM t (NOLOCK)`) are conservative deferrals.
//! - [`table_version`](TableExpressionSyntax::table_version) — the temporal-table
//!   `FOR SYSTEM_TIME {AS OF | FROM..TO | BETWEEN..AND | CONTAINED IN | ALL}` query modifier
//!   on a table factor, a typed [`TableVersion`](crate::ast::TableVersion) for planner
//!   consumers. It sits at the table-factor position (right after the table name, before the
//!   alias), so its `FOR SYSTEM_TIME` trigger never contends with the query-level `FOR XML` /
//!   `FOR JSON` tail or the `FOR` locking clause — those are read only after the whole
//!   `FROM`/`WHERE`, and the word after `FOR` (`SYSTEM_TIME` vs `XML`) partitions them.
//! - Bracket identifiers `[name]` — T-SQL's signature delimiter, modelled by the
//!   [`IdentifierQuote::Asymmetric`] `{ open: '[', close: ']' }` style whose own doc cites
//!   T-SQL. Listed in [`MSSQL_IDENTIFIER_QUOTES`] alongside the standard `"…"`. Enabling the
//!   `[` opener would contend with the `[`-punctuation expression grammar
//!   ([`ExpressionSyntax::subscript`] / [`array_constructor`](ExpressionSyntax::array_constructor)
//!   / [`collection_literals`](ExpressionSyntax::collection_literals), the
//!   [`LexicalConflict::BracketIdentifierVersusArraySyntax`](super::LexicalConflict) hazard),
//!   but all three are **off** in the ANSI base this preset keeps — and that is
//!   behaviour-accurate: T-SQL genuinely lacks `[]` array subscripting, so the bracket is
//!   unambiguously an identifier delimiter here. The lexical-consistency `const` assert below
//!   enforces the single claimant.
//! - [`named_at`](ParameterSyntax::named_at) — the `@name` parameter / local-variable sigil,
//!   whose own doc names T-SQL. Its `@name` trigger contends with
//!   [`SessionVariableSyntax::user_variables`] (MySQL's `@name` read), which stays off (ANSI's
//!   value) so `@name` has a single claimant — the
//!   [`AtNameParameterVersusUserVariable`](super::LexicalConflict) hazard the assert below
//!   rules out. The `@@name` system-variable form is disjoint (its second `@` is not an
//!   identifier byte) and stays off too.
//! - [`national_strings`](StringLiteralSyntax::national_strings) — `N'…'` national-character
//!   string constants, whose own doc names T-SQL. A pure lexical gate over the ANSI string
//!   surface; `backslash_escapes` stays off (its doc reads "MySQL default; not T-SQL").
//! - [`money_literals`](NumericLiteralSyntax::money_literals) — `$1234.56` / `$.5` money
//!   literals (the `$` currency sigil prefixes a decimal), whose own doc names T-SQL. Its
//!   `$`+digit trigger contends with [`ParameterSyntax::positional_dollar`] (PostgreSQL's
//!   `$1`), which stays off (ANSI's value) so `$`+digit has a single claimant — the
//!   [`MoneyVersusPositionalDollar`](super::LexicalConflict) hazard the assert below rules
//!   out.
//!
//! # The two lexical facts over ANSI
//!
//! - **Dual identifier quoting.** T-SQL quotes identifiers with the bracket `[…]` **and** the
//!   standard `"…"` (its default `QUOTED_IDENTIFIER ON`). [`MSSQL_IDENTIFIER_QUOTES`] lists
//!   both; T-SQL has no MySQL-style backtick, so — unlike SQLite/Databricks — no `` ` `` opener
//!   appears. `double_quoted_strings` stays off (ANSI's value) so `"x"` reads as an
//!   identifier, keeping `QUOTED_IDENTIFIER ON` behaviour and the preset lexically consistent.
//! - **Case folding.** T-SQL identifier resolution is case-insensitive (collation-dependent),
//!   so [`identifier_casing`](FeatureSet::identifier_casing) is [`Casing::Lower`] — the
//!   [`Casing`] doc names T-SQL as one of the two dialects [`Casing::Lower`] approximates
//!   ("case-preserving storage, case-insensitive comparison"). The interned text still renders
//!   exactly as written; the fold is identity-only and never affects acceptance. (The
//!   per-identifier-kind table-vs-column sensitivity split is a documented `Casing` limitation
//!   and a deliberate future extension, not modelled here.)
//!
//! # Deliberately deferred (conservative reject)
//!
//! `SELECT … INTO <table>` stays off: [`select_into`](SelectSyntax::select_into)'s own doc
//! models only PostgreSQL's `SELECT … INTO [TEMP] <table>` create-table form and does not cite
//! T-SQL, and with no MSSQL oracle the exact boundary (T-SQL has no `TEMP` keyword — it spells
//! temp tables `#name` instead) cannot be verified, so shipping the PG-shaped gate would be an
//! unmeasured guess. Likewise `TOP (n)`, `GO` batch separators,
//! `#temp` tables, the `OUTPUT` clause, and the T-SQL `MERGE` variants have no modelled gate;
//! several are already catalogued by the structural-extensibility spike. All are clean rejects
//! routed to follow-up tickets, never silent over-accepts.

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

/// MSSQL / T-SQL identifier quoting: the bracket `[…]` **and** the SQL standard `"…"`, both at
/// once. The two openers are distinct bytes, so their order is immaterial. T-SQL has no
/// MySQL-style backtick, so — unlike the SQLite/Lenient bracket sets — no `` ` `` opener
/// appears. `"` stays a quote here (and `double_quoted_strings` is correspondingly off in
/// [`StringLiteralSyntax::ANSI`], which this preset keeps, matching T-SQL's default
/// `QUOTED_IDENTIFIER ON`), so `"x"` is an identifier, never a string.
pub const MSSQL_IDENTIFIER_QUOTES: &[IdentifierQuote] = &[
    IdentifierQuote::Symmetric('"'),
    IdentifierQuote::Asymmetric {
        open: '[',
        close: ']',
    },
];

impl StringLiteralSyntax {
    /// MSSQL string surface: the ANSI baseline plus `N'…'` national-character string constants.
    /// `backslash_escapes` stays off (its doc reads "MySQL default; not T-SQL"). Every other
    /// string knob is conservatively ANSI.
    pub const MSSQL: Self = Self {
        national_strings: true,
        ..StringLiteralSyntax::ANSI
    };
}

impl NumericLiteralSyntax {
    /// MSSQL numeric surface: the ANSI baseline plus `$1234.56` / `$.5` money literals. Every
    /// other numeric knob is conservatively ANSI.
    pub const MSSQL: Self = Self {
        money_literals: true,
        ..NumericLiteralSyntax::ANSI
    };
}

impl ParameterSyntax {
    /// MSSQL parameter surface: the ANSI baseline plus the `@name` parameter / local-variable
    /// sigil. `positional_dollar` stays off (ANSI's value) so `$`+digit belongs solely to
    /// [`NumericLiteralSyntax::money_literals`], and `user_variables` stays off (in
    /// [`SessionVariableSyntax::ANSI`]) so `@name` belongs solely to this — both enforced by
    /// the lexical assert below.
    pub const MSSQL: Self = Self {
        named_at: true,
        ..ParameterSyntax::ANSI
    };
}

impl TableExpressionSyntax {
    /// MSSQL table-expression surface: the ANSI baseline plus the `WITH (...)` table-hint
    /// tail and the temporal-table `FOR SYSTEM_TIME` modifier (all five forms). The
    /// `CROSS APPLY` / `OUTER APPLY` join operators ride [`JoinSyntax`]; every other table
    /// knob is conservatively ANSI.
    pub const MSSQL: Self = Self {
        // `WITH (NOLOCK)` / `WITH (INDEX(ix), FORCESEEK)` table hints — see the module docs.
        table_hints: true,
        // `FOR SYSTEM_TIME {AS OF | FROM..TO | BETWEEN..AND | CONTAINED IN | ALL}` — the
        // temporal-table query modifier.
        table_version: true,
        ..TableExpressionSyntax::ANSI
    };
}

impl JoinSyntax {
    /// The `MSSQL` preset for join syntax.
    pub const MSSQL: Self = Self {
        apply_join: true,
        ..JoinSyntax::ANSI
    };
}

impl QueryTailSyntax {
    /// MSSQL query-tail surface: the ANSI baseline plus the `FOR XML`/`FOR JSON`
    /// result-shaping tail. MSSQL has no query-tail row-locking clause (it spells
    /// isolation with `WITH (…)` table hints, [`TableExpressionSyntax::MSSQL`]), so
    /// `locking_clauses` stays off and the `FOR` lead is unambiguously the
    /// result-shaping clause here.
    pub const MSSQL: Self = Self {
        for_xml_json_clause: true,
        ..QueryTailSyntax::ANSI
    };
}

impl TableFactorSyntax {
    /// The `MSSQL` preset for table factor syntax.
    pub const MSSQL: Self = Self {
        // SQL Server's `OPENJSON(<json> [, <path>]) [WITH (…)]` rowset-function table factor —
        // the sole engine with this exact form (documented; no differential oracle here, so the
        // gate follows the grammar on this conservative preset, the `apply_join` precedent).
        open_json: true,
        ..TableFactorSyntax::ANSI
    };
}

impl FeatureSet {
    /// MSSQL / T-SQL as ANSI-derived dialect data (see the module docs for the full derivation
    /// rationale and the conservatism bar).
    pub const MSSQL: Self = Self {
        // T-SQL identifier resolution is case-insensitive (collation-dependent); `Casing::Lower`
        // is the closest single fit (the `Casing` doc names T-SQL). Identity only — the interned
        // text still renders exactly as written, so this never affects acceptance.
        identifier_casing: Casing::Lower,
        // The lexical delta over ANSI: bracket `[…]` *and* double-quote identifier quoting.
        identifier_quotes: MSSQL_IDENTIFIER_QUOTES,
        default_null_ordering: NullOrdering::NullsLast,
        // No reserved-set delta over ANSI — MSSQL adds no keyword reservation here.
        reserved_column_name: RESERVED_COLUMN_NAME,
        reserved_function_name: RESERVED_FUNCTION_NAME,
        reserved_type_name: RESERVED_TYPE_NAME,
        reserved_bare_alias: RESERVED_BARE_ALIAS,
        reserved_as_label: KeywordSet::EMPTY,
        catalog_qualified_names: true,
        byte_classes: STANDARD_BYTE_CLASSES,
        binding_powers: STANDARD_BINDING_POWERS,
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        // `N'…'` national-character strings.
        string_literals: StringLiteralSyntax::MSSQL,
        // `$1234.56` money literals.
        numeric_literals: NumericLiteralSyntax::MSSQL,
        // `@name` parameters / local variables.
        parameters: ParameterSyntax::MSSQL,
        session_variables: SessionVariableSyntax::ANSI,
        identifier_syntax: IdentifierSyntax::ANSI,
        // `CROSS APPLY` / `OUTER APPLY` — the capstone this preset exposes.
        table_expressions: TableExpressionSyntax::MSSQL,
        join_syntax: JoinSyntax::MSSQL,
        table_factor_syntax: TableFactorSyntax::MSSQL,
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
        // `SELECT … INTO <table>` stays off (deferred — see the module docs); every other
        // SELECT knob is conservatively ANSI.
        select_syntax: SelectSyntax::ANSI,
        query_tail_syntax: QueryTailSyntax::MSSQL,
        grouping_syntax: GroupingSyntax::ANSI,
        utility_syntax: UtilitySyntax::ANSI,
        show_syntax: ShowSyntax::ANSI,
        maintenance_syntax: MaintenanceSyntax::ANSI,
        access_control_syntax: AccessControlSyntax::ANSI,
        type_name_syntax: TypeNameSyntax::ANSI,
        // No MSSQL-specific Tier-1 output spelling yet; render the portable ANSI canonical type
        // names (a `TargetSpelling::Mssql` is render work a later ticket owns).
        target_spelling: TargetSpelling::Ansi,
    };
}

/// Prefer [`FeatureSet::MSSQL`] for struct update.
pub const MSSQL: FeatureSet = FeatureSet::MSSQL;

// Compile-time proof the MSSQL preset claims no shared tokenizer trigger twice. Beyond ANSI it
// adds three triggers — the `[` bracket identifier opener, the `@` named-at sigil, and the `$`
// money sigil — each with a single claimant: no enabled expression grammar lexes `[` as
// punctuation (`subscript`/`array_constructor`/`collection_literals` stay off, so T-SQL's
// bracket is unambiguously an identifier delimiter), `user_variables` stays off so `@name`
// belongs solely to `named_at`, and `positional_dollar` stays off so `$`+digit belongs solely
// to `money_literals`. `"` stays the sole identifier quote it already was (`double_quoted_strings`
// off). Kept as a ratchet so a future MSSQL delta that *does* add a contending trigger fails the
// build here.
const _: () = assert!(FeatureSet::MSSQL.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: no refinement
// flag rides an unset base, and no two features contend for one parser-position head.
const _: () = assert!(FeatureSet::MSSQL.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::MSSQL.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mssql_is_ansi_plus_the_six_gates_and_two_lexical_facts() {
        // The preset is ANSI with a documented, closed set of divergent axes: the two lexical
        // facts (case-folding, dual identifier quoting) and the four enabled sub-presets that
        // carry the six gates (the `table_expressions` sub-preset carries two — `apply_join`
        // and `table_hints`). Asserting the whole rest equals ANSI keeps the "ANSI-derived,
        // every delta documented" claim honest against a future stray edit. Bind to locals so
        // the const reads are not flagged by clippy's `assertions_on_constants`.
        let ansi = FeatureSet::ANSI;
        let mssql = FeatureSet::MSSQL;

        // The two lexical facts.
        assert_eq!(mssql.identifier_casing, Casing::Lower);
        assert_ne!(mssql.identifier_casing, ansi.identifier_casing);
        assert_eq!(mssql.identifier_quotes, MSSQL_IDENTIFIER_QUOTES);
        assert_ne!(mssql.identifier_quotes, ansi.identifier_quotes);
        // `"` stays an identifier quote, so MSSQL must keep `double_quoted_strings` off (the
        // `QUOTED_IDENTIFIER ON` behaviour the preset depends on for lexical consistency).
        assert!(!mssql.string_literals.double_quoted_strings);
        // The single-claimant interplays the enabled sigils depend on.
        assert!(!mssql.session_variables.user_variables);
        assert!(!mssql.parameters.positional_dollar);
        // T-SQL has no backtick identifier quote.
        assert!(
            !mssql
                .identifier_quotes
                .iter()
                .any(|quote| quote.open() == '`')
        );

        // The four divergent sub-presets.
        assert_eq!(mssql.string_literals, StringLiteralSyntax::MSSQL);
        assert_ne!(mssql.string_literals, ansi.string_literals);
        assert_eq!(mssql.numeric_literals, NumericLiteralSyntax::MSSQL);
        assert_ne!(mssql.numeric_literals, ansi.numeric_literals);
        assert_eq!(mssql.parameters, ParameterSyntax::MSSQL);
        assert_ne!(mssql.parameters, ansi.parameters);
        assert_eq!(mssql.table_expressions, TableExpressionSyntax::MSSQL);
        assert_ne!(mssql.table_expressions, ansi.table_expressions);

        // No reserved-set delta: every position is inherited verbatim from ANSI.
        assert_eq!(mssql.reserved_column_name, ansi.reserved_column_name);
        assert_eq!(mssql.reserved_function_name, ansi.reserved_function_name);
        assert_eq!(mssql.reserved_type_name, ansi.reserved_type_name);
        assert_eq!(mssql.reserved_bare_alias, ansi.reserved_bare_alias);
        assert_eq!(mssql.reserved_as_label, KeywordSet::EMPTY);

        // Everything else is inherited verbatim from ANSI — including SELECT (SELECT INTO is
        // deferred, so it stays off).
        assert_eq!(mssql.select_syntax, ansi.select_syntax);
        assert_eq!(mssql.expression_syntax, ansi.expression_syntax);
        assert_eq!(mssql.session_variables, ansi.session_variables);
        assert_eq!(mssql.identifier_syntax, ansi.identifier_syntax);
        assert_eq!(mssql.operator_syntax, ansi.operator_syntax);
        assert_eq!(mssql.call_syntax, ansi.call_syntax);
        assert_eq!(mssql.predicate_syntax, ansi.predicate_syntax);
        assert_eq!(mssql.mutation_syntax, ansi.mutation_syntax);
        assert_eq!(mssql.statement_ddl_gates, ansi.statement_ddl_gates);
        assert_eq!(
            mssql.create_table_clause_syntax,
            ansi.create_table_clause_syntax
        );
        assert_eq!(
            mssql.column_definition_syntax,
            ansi.column_definition_syntax
        );
        assert_eq!(mssql.constraint_syntax, ansi.constraint_syntax);
        assert_eq!(mssql.index_alter_syntax, ansi.index_alter_syntax);
        assert_eq!(mssql.existence_guards, ansi.existence_guards);
        assert_eq!(mssql.utility_syntax, ansi.utility_syntax);
        assert_eq!(mssql.type_name_syntax, ansi.type_name_syntax);
        assert_eq!(mssql.byte_classes, ansi.byte_classes);
        assert_eq!(mssql.binding_powers, ansi.binding_powers);
        assert_eq!(mssql.target_spelling, ansi.target_spelling);
        assert_eq!(mssql.default_null_ordering, ansi.default_null_ordering);
    }

    #[test]
    fn mssql_enables_exactly_the_six_staged_gates() {
        // The capstone: CROSS/OUTER APPLY, `WITH (...)` table hints, bracket identifiers,
        // `@name` parameters, `N'…'` national strings, and `$…` money literals are on, and each
        // is off in the ANSI base it derives from. Forcing the flags back off recovers the ANSI
        // sub-presets verbatim.
        let ansi = FeatureSet::ANSI;
        let mssql = FeatureSet::MSSQL;

        assert!(mssql.join_syntax.apply_join);
        assert!(!ansi.join_syntax.apply_join);
        assert!(mssql.table_expressions.table_hints);
        assert!(!ansi.table_expressions.table_hints);
        assert!(mssql.parameters.named_at && !ansi.parameters.named_at);
        assert!(mssql.string_literals.national_strings && !ansi.string_literals.national_strings);
        assert!(mssql.numeric_literals.money_literals && !ansi.numeric_literals.money_literals);
        // The bracket opener is the fifth gate (an identifier-quote delta, not a bool).
        assert!(
            mssql
                .identifier_quotes
                .iter()
                .any(|quote| quote.open() == '[')
        );
        assert!(
            !ansi
                .identifier_quotes
                .iter()
                .any(|quote| quote.open() == '[')
        );

        assert_eq!(
            TableExpressionSyntax {
                table_hints: false,
                table_version: false,
                ..mssql.table_expressions
            },
            ansi.table_expressions,
        );
        assert_eq!(
            JoinSyntax {
                apply_join: false,
                ..mssql.join_syntax
            },
            ansi.join_syntax,
        );
        assert_eq!(
            ParameterSyntax {
                named_at: false,
                ..mssql.parameters
            },
            ansi.parameters,
        );
        assert_eq!(
            StringLiteralSyntax {
                national_strings: false,
                ..mssql.string_literals
            },
            ansi.string_literals,
        );
        assert_eq!(
            NumericLiteralSyntax {
                money_literals: false,
                ..mssql.numeric_literals
            },
            ansi.numeric_literals,
        );
    }

    #[test]
    fn mssql_is_lexically_consistent_and_dependency_clean() {
        // Both self-consistency registries must be clean: the `[` bracket quote, the `@` named-at
        // sigil, and the `$` money sigil each have a single claimant (array/subscript grammar off,
        // `user_variables` off, `positional_dollar` off, `double_quoted_strings` off), and none of
        // the five enabled gates rides an unset base flag.
        let mssql = FeatureSet::MSSQL;
        assert_eq!(mssql.lexical_conflict(), None);
        assert!(mssql.is_lexically_consistent());
        assert_eq!(mssql.feature_dependencies(), None);
        assert!(mssql.has_satisfied_feature_dependencies());
        assert_eq!(mssql.grammar_conflict(), None);
        assert!(mssql.has_no_grammar_conflict());
    }
}
