// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The SQLite dialect preset and its reserved-keyword sets.
//!
//! The module is self-contained for feature gating: a build without the `sqlite`
//! cargo feature compiles none of this preset's data and never depends on a gated
//! sibling preset.

use super::keyword::Keyword;
use super::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, ConstraintSyntax, CreateTableClauseSyntax,
    DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FeatureSet, GroupingSyntax,
    IdentifierQuote, IdentifierSyntax, IndexAlterSyntax, JoinSyntax, KeywordOperators, KeywordSet,
    MaintenanceSyntax, MutationSyntax, NullOrdering, NumericLiteralSyntax, OperatorSyntax,
    ParameterSyntax, PipeOperator, PredicateSyntax, QueryTailSyntax, SQLITE_BYTE_CLASSES,
    SelectSyntax, SessionVariableSyntax, ShowSyntax, StatementDdlGates, StringFuncForms,
    StringLiteralSyntax, TableExpressionSyntax, TableFactorSyntax, TargetSpelling, TypeNameSyntax,
    UtilitySyntax,
};
use crate::precedence::{
    Assoc, BindingPower, BindingPowerTable, STANDARD_SET_OPERATION_BINDING_POWERS,
};

/// SQLite identifier quoting: the standard `"a"`, MySQL-style `` `a` ``, and T-SQL
/// `[a]`. SQLite additionally *falls back* a double-quoted token to a string
/// constant when it resolves to no identifier — a resolution-time misfeature our
/// parse-time model cannot express and the accept/reject oracle cannot see (`"x"`
/// is accepted by both engines). Modelling `"` as the identifier quote is the
/// faithful, conflict-free choice: [`StringLiteralSyntax::double_quoted_strings`]
/// stays off, so no [`LexicalConflict::DoubleQuoteStringVersusIdentifier`] arises,
/// and the fallback is recorded as an excluded-with-reason semantic divergence, not
/// parsed (the sweep's `Control` class).
///
/// [`StringLiteralSyntax::double_quoted_strings`]: super::StringLiteralSyntax::double_quoted_strings
/// [`LexicalConflict::DoubleQuoteStringVersusIdentifier`]: super::LexicalConflict::DoubleQuoteStringVersusIdentifier
pub const SQLITE_IDENTIFIER_QUOTES: &[IdentifierQuote] = &[
    IdentifierQuote::Symmetric('"'),
    IdentifierQuote::Symmetric('`'),
    IdentifierQuote::Asymmetric {
        open: '[',
        close: ']',
    },
];

// --- SQLite per-position reject sets (POSITION-AWARE) -------------------------
//
// SQLite's reserved set is far smaller than ANSI's: its tokenizer keyword table
// (`parse.y`) puts most keywords in the `%fallback ID` list, so `END`/`DESC`/`ASC`/
// `ANALYZE`/`REPLACE`/… serve as ordinary identifiers. But the remaining reservations
// are genuinely *position-dependent* — SQLite's grammar admits some non-fallback
// keywords as a name (`nm`) while rejecting them as a bare alias / type name (`ids`),
// so the five positions do NOT share one set (measured, not assumed — an in-process
// rusqlite 3.53.2 probe over every position; see the ticket transcript). Three word
// classes drive the split (each already a `Keyword` variant; hand-composed like the
// `POSTGRES_NON_GENERIC_FUNCTION_KEYWORDS` precedent in `ansi.rs`, not a generated
// per-dialect bitset — SQLite's sets are small subsets the union already holds):
//
//   * STRUCTURAL — the core keywords reserved in EVERY position (`SELECT`/`FROM`/`WHERE`/
//     …). Not in `%fallback ID`, and not a `JOIN_KW`, so no production admits them.
//   * JOIN keywords (`CROSS`/`INNER`/`LEFT`/`NATURAL`/`OUTER`/`RIGHT`/`FULL`) — tokenized
//     as `JOIN_KW`, which the grammar admits via `nm ::= JOIN_KW`. So they ARE valid as a
//     table/column name (`CREATE TABLE cross(…)`, `FROM left`), a function name, and an
//     `AS` label (`AS nm`) — but NOT as a bare alias or a type name, both of which are
//     the narrower `ids ::= ID|STRING` class that excludes `JOIN_KW` (`FROM t cross` is
//     the CROSS JOIN, `CAST(1 AS cross)` a syntax error). This is the higher-impact half
//     (`SELECT 1 AS left` / `CREATE TABLE left(…)` are common valid SQLite we rejected).
//   * NAME-reserved residuals (`ISNULL`/`NOTNULL`/`RETURNING`/`NOTHING`) — non-fallback
//     keywords SQLite rejects as an identifier in every NAME position (probed: `SELECT
//     isnull`, `AS returning`, `CREATE TABLE nothing(…)` all syntax-reject). `RETURNING`/
//     `NOTHING` are also rejected as a bare alias; `ISNULL`/`NOTNULL` are the exception —
//     SQLite reads `SELECT 1 isnull` as the postfix `IS NULL` operator, so it accepts
//     there. We do not model that postfix operator, so admitting them as a bare alias
//     matches SQLite's ACCEPT verdict (a different tree, same accept/reject) rather than
//     over-rejecting the common `col isnull` null test.
//
// The operator keywords `GLOB`/`MATCH`/`REGEXP` are deliberately absent from every set:
// they double as function/identifier names (the built-in `glob(pattern, string)`), and a
// dangling `SELECT 1 glob` is rejected by the Pratt operator path, not the reserved set.

/// The SQLite keywords reserved in every position (`STRUCTURAL`): not in the `%fallback
/// ID` list and not a `JOIN_KW`, so no production admits them as an identifier.
const SQLITE_STRUCTURAL_RESERVED: &[Keyword] = &[
    Keyword::Add,
    Keyword::All,
    Keyword::Alter,
    Keyword::And,
    Keyword::As,
    Keyword::Between,
    Keyword::Case,
    Keyword::Check,
    Keyword::Collate,
    Keyword::Commit,
    Keyword::Constraint,
    Keyword::Create,
    Keyword::Default,
    Keyword::Deferrable,
    Keyword::Delete,
    Keyword::Distinct,
    Keyword::Drop,
    Keyword::Else,
    Keyword::Escape,
    Keyword::Except,
    Keyword::Exists,
    Keyword::Foreign,
    Keyword::From,
    Keyword::Group,
    Keyword::Having,
    Keyword::In,
    Keyword::Index,
    Keyword::Insert,
    Keyword::Intersect,
    Keyword::Into,
    Keyword::Is,
    Keyword::Join,
    Keyword::Limit,
    Keyword::Not,
    Keyword::Null,
    Keyword::On,
    Keyword::Or,
    Keyword::Order,
    Keyword::Primary,
    Keyword::References,
    Keyword::Select,
    Keyword::Set,
    Keyword::Table,
    Keyword::Then,
    Keyword::To,
    Keyword::Transaction,
    Keyword::Union,
    Keyword::Unique,
    Keyword::Update,
    Keyword::Using,
    Keyword::Values,
    Keyword::When,
    Keyword::Where,
];

/// The seven SQLite `JOIN_KW` keywords: admissible as a name (`nm ::= JOIN_KW`) but not
/// as a bare alias / type name (`ids ::= ID|STRING`), and consumed by the join grammar in
/// join position. Reserved only in the bare-alias and type-name sets below.
const SQLITE_JOIN_KEYWORDS: &[Keyword] = &[
    Keyword::Cross,
    Keyword::Inner,
    Keyword::Left,
    Keyword::Natural,
    Keyword::Outer,
    Keyword::Right,
    Keyword::Full,
];

/// SQLite residual keywords reserved in every NAME position AND as a bare alias
/// (`RETURNING`/`NOTHING` — probed syntax-rejects in all positions including
/// `SELECT 1 nothing`).
const SQLITE_RESIDUAL_NAME_AND_BARE: &[Keyword] = &[Keyword::Returning, Keyword::Nothing];

/// SQLite residual keywords reserved in every NAME position but ADMITTED as a bare alias
/// (`ISNULL`/`NOTNULL` — SQLite reads `SELECT 1 isnull` as the postfix `IS NULL` operator
/// and accepts; we do not model that operator, so admitting them as a bare alias matches
/// the ACCEPT verdict instead of over-rejecting the common `col isnull` null test).
const SQLITE_RESIDUAL_NAME_ONLY: &[Keyword] = &[Keyword::Isnull, Keyword::Notnull];

/// SQLite `ColId` reject set (column/table name, `AS` correlation alias, qualifier) —
/// SQLite's `nm` production: `STRUCTURAL ∪ {all four NAME-reserved residuals}`. The seven
/// `JOIN_KW` keywords are ABSENT (`nm ::= JOIN_KW` admits them), so `CREATE TABLE left(…)`
/// / `FROM cross` / `INSERT INTO right …` parse.
pub const SQLITE_RESERVED_COLUMN_NAME: KeywordSet =
    KeywordSet::from_keywords(SQLITE_STRUCTURAL_RESERVED)
        .union(KeywordSet::from_keywords(SQLITE_RESIDUAL_NAME_AND_BARE))
        .union(KeywordSet::from_keywords(SQLITE_RESIDUAL_NAME_ONLY));

/// SQLite function-name reject set. SQLite draws no `type_func_name` carve-out, so the
/// name set governs the function position too (JOIN keywords ride `nm` and are admitted as
/// call heads — `left(…)` — while `isnull(…)` is a syntax reject like every other position).
pub const SQLITE_RESERVED_FUNCTION_NAME: KeywordSet = SQLITE_RESERVED_COLUMN_NAME;

/// SQLite (user-defined / affinity) type-name reject set — SQLite's `ids`-class type name
/// (`typename ::= ids …`, excluding `JOIN_KW`): the name set PLUS the seven JOIN keywords,
/// so `CAST(1 AS cross)` is the syntax error SQLite reports even though the affinity
/// fallback would otherwise treat `cross` as a user type. Arbitrary affinity names
/// (`BANANA`) still ride the user-defined fallback before this gate.
pub const SQLITE_RESERVED_TYPE_NAME: KeywordSet =
    SQLITE_RESERVED_COLUMN_NAME.union(KeywordSet::from_keywords(SQLITE_JOIN_KEYWORDS));

/// SQLite bare-alias reject set — the `ids ::= ID|STRING` class for a bare (`AS`-less)
/// alias: `STRUCTURAL ∪ JOIN keywords ∪ {RETURNING, NOTHING}`. The JOIN keywords are
/// reserved here (so `FROM t cross JOIN u` keeps `cross` for the join grammar and
/// `SELECT 1 cross` is a syntax error), but `ISNULL`/`NOTNULL` are ADMITTED (they parse as
/// the postfix null-test operator in SQLite; see `SQLITE_RESIDUAL_NAME_ONLY`). This set
/// also governs the bare (`AS`-less) *table* correlation alias via
/// [`TableExpressionSyntax::bare_table_alias_is_bare_label`](super::TableExpressionSyntax::bare_table_alias_is_bare_label).
pub const SQLITE_RESERVED_BARE_ALIAS: KeywordSet =
    KeywordSet::from_keywords(SQLITE_STRUCTURAL_RESERVED)
        .union(KeywordSet::from_keywords(SQLITE_JOIN_KEYWORDS))
        .union(KeywordSet::from_keywords(SQLITE_RESIDUAL_NAME_AND_BARE));

/// SQLite `ColLabel` reject set — the `AS`-alias (`AS nm`) and dotted-name-continuation
/// position. PostgreSQL admits every keyword there ([`KeywordSet::EMPTY`]); SQLite's `AS`
/// label is the same `nm` production as a column name, so it reuses the ColId set —
/// admitting the JOIN keywords (`SELECT 1 AS left`) while rejecting `STRUCTURAL` and the
/// NAME-reserved residuals (`SELECT 1 AS delete` / `AS returning` are parse errors).
pub const SQLITE_RESERVED_AS_LABEL: KeywordSet = SQLITE_RESERVED_COLUMN_NAME;

impl StringLiteralSyntax {
    /// The `SQLITE` preset for string literal syntax.
    pub const SQLITE: Self = Self {
        escape_strings: false,
        dollar_quoted_strings: false,
        national_strings: false,
        double_quoted_strings: false,
        backslash_escapes: false,
        unicode_strings: false,
        bit_string_literals: false,
        blob_literals: true,
        charset_introducers: false,
        // SQLite requires a newline in the separator between adjacent literals.
        same_line_adjacent_concat: false,
    };
}

impl NumericLiteralSyntax {
    /// The `SQLITE` preset for numeric literal syntax.
    pub const SQLITE: Self = Self {
        hex_integers: true,
        octal_integers: false,
        binary_integers: false,
        // `_` digit-group separators (SQLite 3.46+, oracle-probed on rusqlite 3.53.2):
        // `1_000`/`0x1_F` accept, `1_`/`1__0`/`0x_1F` reject.
        underscore_separators: true,
        money_literals: false,
        // SQLite's radix grammar is `0[xX]{hexdigit}(_?{hexdigit})*` — a `_` may sit
        // between hex digits but not lead the body (`0x_1F` rejects, unlike PG).
        radix_leading_underscore: false,
        // SQLite lexes a numeric literal abutting identifier chars as one TK_ILLEGAL
        // token (`1SETECT`, `2ES`, `0x1g`, `1e5x` all reject; oracle-probed). Enabling
        // this requires `underscore_separators` above so `1_000_000` (SQLite 3.46+) stays
        // one number rather than a newly-rejected `1` plus junk suffix.
        reject_trailing_junk: true,
    };
}

impl CommentSyntax {
    /// The `SQLITE` preset for comment syntax.
    ///
    /// SQLite's block comments diverge from the ANSI baseline in two engine-measured ways
    /// (rusqlite): they do **not** nest (`/* a /* b */` closes at the first `*/`, so the
    /// whole input is one comment and accepts — a nesting scanner would read it as
    /// unterminated), and an unterminated `/* …` running to end of input is silently closed
    /// as trailing trivia rather than an error (`SELECT 1/* eof`, `\t\t/*\t\t` prepare). Line
    /// comments (`--`) stay `\n`-terminated like the ANSI baseline (a bare `/*` at EOF is the
    /// `/` slash operator, handled by the tokenizer — see the field doc).
    pub const SQLITE: Self = Self {
        nested_block_comments: false,
        unterminated_block_comment_at_eof: true,
        line_comment_hash: false,
        line_comment_ends_at_carriage_return: false,
        versioned_comments: None,
    };
}

impl ParameterSyntax {
    /// The `SQLITE` preset for parameter syntax.
    pub const SQLITE: Self = Self {
        positional_dollar: false,
        positional_dollar_large: false,
        anonymous_question: true,
        named_colon: true,
        named_at: true,
        named_dollar: true,
        // SQLite numbered `?NNN` positional parameters (`?1`, `?123`), range-checked to
        // `1..=32766` when materialised (engine-measured on rusqlite).
        numbered_question: true,
    };
}

impl IdentifierSyntax {
    /// The `SQLITE` preset for identifier syntax.
    pub const SQLITE: Self = Self {
        // SQLite's IdChar class admits every code point at or above U+0080.
        non_ascii: super::NonAsciiIdentifierSyntax::Any,
        // SQLite's IdChar set includes `$` as a *continuation* byte (`L$C3`, `a$b`, `t$x` are
        // one identifier each; engine-measured on rusqlite). `$` never *starts* an identifier —
        // a leading `$name` is the dollar-named placeholder (`named_dollar`) and a lone `$` is a
        // stray byte — so this widens only the continue run and never contends with the sigil.
        dollar_in_identifiers: true,
        // SQLite reads a single-quoted `'name'` string as a name wherever the grammar wants a
        // `nm` identifier. Corpus-admitted for the relation-target and `PRIMARY
        // KEY`/`UNIQUE` column-name positions (see the field doc); each is position-driven
        // and unambiguous.
        string_literal_identifiers: true,
        // DuckDB-only single-part Sconst table-name form; SQLite's broader multi-part
        // string-identifier misfeature is [`string_literal_identifiers`] above.
        string_literal_table_names: false,
        // SQLite admits an empty quoted identifier in every quote style (`` `` ``, `[]`, `""`);
        // engine-measured on rusqlite, unique among the shipped engines.
        empty_quoted_identifiers: true,
    };
}

impl TypeNameSyntax {
    /// The `SQLITE` preset for type name syntax.
    pub const SQLITE: Self = Self {
        integer_display_width: true,
        // SQLite's `typename` is a free `ids ...` token run: an arbitrary multi-word affinity
        // name (`UNSIGNED BIG INT`, `LONG INTEGER`) with an optional two-argument modifier
        // (`VARCHAR(123,456)`), terminated by a column-constraint keyword / comma / close
        // paren (engine-probed on rusqlite/sqlite3 3.53.2 & 3.43.2). Typed variants still win
        // where they can faithfully hold the input.
        liberal_type_names: true,
        string_type_modifiers: false,
        extended_scalar_type_names: false,
        enum_type: false,
        set_type: false,
        numeric_modifiers: false,
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
        angle_bracket_types: false,
    };
}

impl TableExpressionSyntax {
    /// The `SQLITE` preset for table expression syntax.
    pub const SQLITE: Self = Self {
        table_alias_column_lists: false,
        bare_table_alias_is_bare_label: true,
        // SQLite's `INDEXED BY <index>` / `NOT INDEXED` index directive on a table reference.
        indexed_by: true,
        only: false,
        table_sample: false,
        parenthesized_joins: true,
        join_using_alias: false,
        index_hints: false,
        table_hints: false,
        partition_selection: false,
        base_table_alias_column_lists: true,
        string_literal_aliases: false,
        aliased_parenthesized_join: true,
        table_version: false,
        table_json_path: false,
    };
}

impl JoinSyntax {
    /// The `SQLITE` preset for join syntax.
    pub const SQLITE: Self = Self {
        stacked_join_qualifiers: false,
        // SQLite admits `NATURAL` before any join type; `NATURAL CROSS JOIN` is a natural
        // inner join (engine-probed on rusqlite 3.53.2: shared-column equijoin shape, not
        // the cross product), normalized into the canonical Inner+Natural shape.
        natural_cross_join: true,
        full_outer_join: true,
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

impl TableFactorSyntax {
    /// The `SQLITE` preset for table factor syntax.
    pub const SQLITE: Self = Self {
        // SQLite's `table-or-subquery` grammar admits a generic `table-function-name (
        // args )` factor (the `pragma_table_info('t')` / `json_each('[]')` table-valued
        // functions). Table-valued-ness is resolved at bind time, not parse time:
        // `FROM abs(1)` and `FROM nofn(1)` parse-accept and fail only at prepare with a
        // *binding* reject ("no such table"), while `FROM SELECT` / `FROM 123` are genuine
        // syntax errors (engine-probed via rusqlite 3.53.2). Our parse-only parser matches
        // that grammar with the flag on; the binding reject is not a parser concern.
        table_functions: true,
        lateral: false,
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

impl ExpressionSyntax {
    /// The `SQLITE` preset for expression syntax.
    pub const SQLITE: Self = Self {
        typecast_operator: false,
        subscript: false,
        // DuckDB's three-bound `[lower:upper:step]` slice is a dialect extension (and `[`
        // is a bracket identifier quote here regardless).
        slice_step: false,
        // `expr COLLATE <name>` as an expression postfix: `ORDER BY a COLLATE nocase`,
        // `WHERE a < 'x' COLLATE binary`, and — since a `CREATE INDEX` key is a full
        // expression — `CREATE INDEX i ON t(a COLLATE nocase)`. SQLite ranks it above the
        // comparison operators exactly as PostgreSQL does (the shared postfix binding power),
        // so `a = b COLLATE c` binds `a = (b COLLATE c)`. The collation name is an ordinary
        // identifier (`nocase`/`binary`/`rtrim`, or a quoted `"reverse sort"`), read by the
        // shared object-name grammar; an undefined collation is a SQLite *binding* reject the
        // parser cannot and does not screen.
        collate: true,
        at_time_zone: false,
        semi_structured_access: false,
        array_constructor: false,
        multidim_array_literals: false,
        collection_literals: false,
        row_constructor: false,
        struct_constructor: false,
        field_selection: false,
        field_wildcard: false,
        typed_string_literals: false,
        // SQLite has no prefix-typed literal at all (`typed_string_literals` off), so the
        // interval literal is never reached; kept off for uniformity.
        typed_interval_literal: false,
        // DuckDB's relaxed interval spellings are a dialect extension.
        relaxed_interval_syntax: false,
        mysql_interval_operator: false,
        // DuckDB's `#n` positional column reference is a dialect extension.
        positional_column: false,
        lambda_keyword: false,
    };
}

impl PredicateSyntax {
    /// The `SQLITE` preset for predicate syntax.
    pub const SQLITE: Self = Self {
        // SQLite accepts an empty `IN ()` list (`x IN ()` is false, `x NOT IN ()` true);
        // engine-measured via rusqlite. Otherwise SQLite's predicate surface is the ANSI
        // baseline (standard `LIKE`, no `ILIKE`/`SIMILAR TO`/`OVERLAPS`/`NORMALIZED`).
        empty_in_list: true,
        // SQLite accepts the two-word `<expr> NOT NULL` postfix (a synonym for `IS NOT NULL`)
        // alongside the one-word `NOTNULL`/`ISNULL`; both engine-measured via rusqlite 3.53.2
        // (`SELECT 1 WHERE 1 NOT NULL` -> 1).
        null_test_two_word_postfix: true,
        is_distinct_from: true,
        like: true,
        ilike: false,
        similar_to: false,
        overlaps_period_predicate: false,
        unparenthesized_in_list: false,
        pattern_match_quantifier: false,
        between_symmetric: false,
        is_normalized: false,
    };
}

impl OperatorSyntax {
    /// The `SQLITE` preset for operator syntax.
    pub const SQLITE: Self = Self {
        operator_construct: false,
        containment_operators: false,
        json_arrow_operators: true,
        // SQLite spells `?` as the anonymous placeholder and has none of the PostgreSQL
        // `jsonb` operators, so this stays off (it would contend for the `?` trigger).
        jsonb_operators: false,
        double_equals: true,
        // DuckDB-only `//` spelling; SQLite has no integer-division operator.
        integer_divide_slash: false,
        starts_with_operator: false,
        is_general_equality: true,
        // No truth-value predicate: SQLite's general `IS` folds `IS TRUE`/`IS FALSE` onto
        // the boolean literal and reads `IS UNKNOWN` as equality against an identifier
        // `unknown`, rejecting it unless bound (engine-measured via rusqlite). Off keeps
        // that reading; on would over-accept `IS UNKNOWN`.
        truth_value_tests: false,
        // `<=>` is MySQL-only.
        null_safe_equals: false,
        // The single-arrow lambda is DuckDB-only: SQLite's `->` (on above) is always
        // the JSON accessor, so `x -> x + 1` stays a `JsonGet` binary op here.
        lambda_expressions: false,
        // SQLite accepts the bitwise `| & ~ << >>` operators (engine-measured via rusqlite):
        // SQLite has no bitwise XOR, so `bitwise_xor` stays off on the preset below.
        bitwise_operators: true,
        quantified_comparisons: false,
        quantified_comparison_lists: false,
        // SQLite has no quantified comparison at all, so the any-operator extension is
        // vacuously off.
        quantified_arbitrary_operator: false,
        // SQLite has no general `Op`-class operator surface (its `^` has no infix meaning,
        // via `caret_operator` on the preset below).
        custom_operators: false,
        null_test_postfix: true,
        // SQLite has no postfix operator surface — a trailing symbolic operator rejects.
        postfix_operators: false,
    };
}

impl CallSyntax {
    /// The `SQLITE` preset for call syntax.
    pub const SQLITE: Self = Self {
        named_argument: false,
        // `<=>` and the UTC_* niladic functions are MySQL-only.
        utc_special_functions: false,
        columns_expression: false,
        extract_from_syntax: false,
        try_cast: false,
        // SQLite's flexible typing accepts any affinity name as a cast target.
        restricted_cast_targets: false,
        // DuckDB-specific call tails; off for SQLite.
        extract_string_field: false,
        method_chaining: false,
        sqljson_constructors_require_argument: false,
        // SQLite has no SQL/JSON standard expression functions; keep the names ordinary.
        sqljson_expression_functions: false,
        // SQLite has no SQL/XML expression functions; keep the `xml*` names ordinary.
        xml_expression_functions: false,
        variadic_argument: false,
        // `merge_action()` is a PostgreSQL-only support function.
        merge_action_function: false,
        convert_function: false,
    };
}

impl StringFuncForms {
    /// The `SQLITE` preset for string func forms.
    pub const SQLITE: Self = Self {
        // SQLite has none of the keyword string special forms (probed on the bundled
        // engine: `SUBSTRING(x FROM 2)` / `TRIM(LEADING …)` / `OVERLAY(… PLACING …)`
        // are all syntax errors; its `substring`/`substr`/`trim` are ordinary
        // functions, and `position(a, b)` fails only at binding), so the whole
        // family stays off and the heads keep their plain-call readings.
        substring_from_for: false,
        substring_leading_for: false,
        substring_similar: false,
        substring_plain_call_requires_2_or_3_args: false,
        substr_from_for: false,
        position_in: false,
        position_asymmetric_operands: false,
        overlay_placing: false,
        overlay_requires_placing: false,
        trim_from: false,
        trim_list_syntax: false,
        // `COLLATION FOR (<expr>)` is a PostgreSQL-only common-subexpr.
        collation_for_expression: false,
        // The `CEIL TO <field>` keyword form is sqlparser-rs-parity surface only —
        // no probed oracle engine's grammar admits it.
        ceil_to_field: false,
        // The `FLOOR TO <field>` keyword form is sqlparser-rs-parity surface only —
        // no probed oracle engine's grammar admits it.
        floor_to_field: false,
        match_against: false,
    };
}

impl AggregateCallSyntax {
    /// The `SQLITE` preset for aggregate call syntax.
    pub const SQLITE: Self = Self {
        group_concat_separator: false,
        within_group: false,
        // SQLite *does* have the `FILTER (WHERE …)` aggregate filter (since 3.30.0,
        // engine-measured-accepted via rusqlite), so it stays on — unlike MySQL.
        aggregate_filter: true,
        // SQLite requires `FILTER (WHERE …)` (the keyword-less body is a syntax error).
        filter_optional_where: false,
        // SQLite admits an aggregate's argument forms regardless of a space before the `(`;
        // the significant-space rule is MySQL's `IGNORE_SPACE`-off tokenizer only.
        aggregate_args_require_adjacent_paren: false,
        null_treatment: false,
        // MySQL-only built-in aggregate/window arity restrictions; SQLite admits an empty
        // aggregate call and `OVER` on any function.
        aggregate_calls_reject_empty_arguments: false,
        over_requires_windowable_function: false,
        window_function_tail: false,
        standalone_argument_order_by: false,
    };
}

impl MutationSyntax {
    /// The `SQLITE` preset for mutation syntax.
    pub const SQLITE: Self = Self {
        insert_ignore: false,
        insert_overwrite: false,
        returning: true,
        on_conflict: true,
        on_duplicate_key_update: false,
        multi_column_assignment: false,
        update_tuple_value_row_arity: false,
        where_current_of: false,
        merge: false,
        replace_into: true,
        insert_set: false,
        // The MySQL `UPDATE`/`DELETE ... ORDER BY ... LIMIT` tails are a MySQL surface;
        // SQLite's own (compile-time `SQLITE_ENABLE_UPDATE_DELETE_LIMIT`) form is off by
        // default and out of this preset's scope.
        update_delete_tails: false,
        joined_update_delete: false,
        // SQLite's `INSERT OR <action>` / `UPDATE OR <action>` conflict-resolution prefix.
        or_conflict_action: true,
        insert_column_matching: false,
        // SQLite has no `DELETE ... USING` multi-relation delete.
        delete_using: false,
        // SQLite (3.33+) admits `UPDATE … SET … FROM <tables>`.
        update_from: true,
        // SQLite has no `DELETE … USING`, so the target-alias gate is moot; a leading
        // `WITH` before `INSERT` is admitted.
        delete_using_target_alias: true,
        cte_before_insert: true,
        // SQLite has no `MERGE`, so the leading-`WITH` gate is moot; off.
        cte_before_merge: false,
        // SQLite CTE bodies are select statements only — a DML body is a syntax error
        // (probed via rusqlite prepare).
        data_modifying_ctes: false,
        // SQLite has no `MERGE`, so its residual-grammar gates are all moot; off.
        merge_when_not_matched_by: false,
        merge_insert_default_values: false,
        merge_insert_overriding: false,
        merge_insert_multirow: false,
        merge_update_set_star: false,
        merge_insert_star_by_name: false,
        merge_error_action: false,
        update_set_qualified_column: true,
    };
}

impl StatementDdlGates {
    /// The `SQLITE` preset for statement ddl gates.
    pub const SQLITE: Self = Self {
        colocation_groups: false,
        materialized_view_to: false,
        // SQLite's `CREATE TRIGGER … BEGIN … END` compound-statement body.
        create_trigger: true,
        // SQLite has no `CREATE MACRO` (its functions are C-registered).
        create_macro: false,
        create_secret: false,
        create_type: false,
        // SQLite's `CREATE VIRTUAL TABLE <name> USING <module>(<args>)` — the module owns the
        // opaque argument grammar; the parser only splits the args on top-level commas.
        create_virtual_table: true,
        // SQLite has no sequence generators (it uses AUTOINCREMENT rowids); `CREATE SEQUENCE`
        // rejects — the `SEQUENCE` keyword falls through to the `CREATE TABLE` expectation.
        create_sequence: false,
        create_sequence_cache: false,
        extension_ddl: false,
        transform_ddl: false,
        alter_system: false,
        // MySQL's tablespace / logfile-group storage DDL is not a SQLite statement.
        tablespace_ddl: false,
        logfile_group_ddl: false,
        // SQLite lacks schema objects, CREATE DATABASE, materialized views, stored
        // routines, and the OR REPLACE modifier (all engine-measured-rejected).
        schemas: false,
        // SQLite has no schema objects at all, so the embedded-element form is off too.
        schema_elements: false,
        databases: false,
        // SQLite has no `DROP DATABASE`/`DROP SCHEMA` (databases are files, reached via ATTACH).
        drop_database: false,
        materialized_views: false,
        // SQLite spells session-local views `CREATE TEMP VIEW`.
        temporary_views: true,
        routines: false,
        or_replace: false,
        // `CREATE RECURSIVE VIEW` is a DuckDB form; SQLite leaves `RECURSIVE`
        // unconsumed before the expected `VIEW`.
        recursive_views: false,
        // SQLite has no stored programs, so no compound-statement routine body.
        compound_statements: false,
        alter_database: false,
        alter_database_options: false,
        server_definition: false,
        alter_instance: false,
        spatial_reference_system: false,
        resource_group: false,
        alter_sequence: false,
        alter_object_set_schema: false,
        view_definition_options: false,
    };
}

impl CreateTableClauseSyntax {
    /// The `SQLITE` preset for create table clause syntax.
    pub const SQLITE: Self = Self {
        table_options: false,
        // SQLite accepts the trailing `WITHOUT ROWID` table option (a rowid-less table).
        without_rowid_table_option: true,
        // SQLite accepts the trailing `STRICT` table option (strict column-type enforcement).
        strict_table_option: true,
        // `OR REPLACE TABLE` and `CREATE SECRET` are DuckDB-specific.
        create_or_replace_table: false,
        storage_parameters: false,
        on_commit: false,
        create_table_as_with_data: true,
        create_table_as_execute: false,
        // SQLite has no table partitioning.
        declarative_partitioning: false,
        // SQLite has no table inheritance nor the PostgreSQL LIKE source-table element, and no
        // statement-level `LIKE src` clone — SQLite reads a bare `LIKE` in element position as a
        // keyword-named column instead, a behaviour this off flag preserves.
        table_inheritance: false,
        like_source_table: false,
        statement_level_table_like: false,
        unlogged_tables: false,
        table_access_method: false,
        without_oids: false,
        typed_tables: false,
    };
}

impl ColumnDefinitionSyntax {
    /// The `SQLITE` preset for column definition syntax.
    pub const SQLITE: Self = Self {
        generated_column_shorthand: true,
        // SQLite accepts a column-level `ON CONFLICT <resolution>` on an inline
        // `NOT NULL`/`UNIQUE`/`PRIMARY KEY`/`CHECK` constraint.
        column_conflict_resolution_clause: true,
        // SQLite accepts a column with no declared type (`CREATE TABLE t (a, b)`).
        typeless_column_definitions: true,
        // The DuckDB generated-column narrowing is redundant under SQLite's wider typeless
        // rule above (any column may drop its type), so it stays off — no need to also fire
        // the narrow gate.
        typeless_generated_columns: false,
        // SQLite accepts the joined `AUTOINCREMENT` attribute on an inline `PRIMARY KEY` column
        // (its own one-word keyword, distinct from MySQL's underscored `AUTO_INCREMENT`).
        joined_autoincrement_attribute: true,
        // SQLite accepts an `ASC`/`DESC` order qualifier on an inline `PRIMARY KEY` column
        // (`a INTEGER PRIMARY KEY DESC`).
        inline_primary_key_ordering: true,
        // SQLite makes `COLLATE` an ordinary nameable column constraint, so it accepts a
        // `CONSTRAINT <name>` prefix on a column COLLATE clause.
        named_column_collate_constraint: true,
        // SQLite has no IDENTITY column, WITH (storage params), ON COMMIT action, or
        // extended ALTER surface (its ALTER is RENAME/ADD/DROP COLUMN only).
        identity_columns: false,
        compact_identity_columns: false,
        // SQLite accepts a bare expression default and a `CONSTRAINT <name>` on any inline
        // column constraint.
        default_expression_requires_parens: false,
        column_default_requires_b_expr: false,
        // SQLite spells a per-column `COLLATE <name>` (a single bare identifier). The remaining
        // residue surfaces — UNLOGGED, column STORAGE/COMPRESSION, the USING access method,
        // WITHOUT OIDS, typed `OF <type>` tables — are PostgreSQL-only and absent here.
        column_collation: true,
        column_storage: false,
    };
}

impl ConstraintSyntax {
    /// The `SQLITE` preset for constraint syntax.
    pub const SQLITE: Self = Self {
        deferrable_constraints: true,
        named_inline_non_check_constraints: true,
        // SQLite accepts a trailing bodyless `CONSTRAINT <name>` — the constraint element
        // after the name is optional — chaining freely with bodied constraints and, in the
        // table-constraint list, needing no separating comma.
        bare_constraint_name: true,
        exclusion_constraints: false,
        constraint_no_inherit_not_valid: false,
        index_constraint_parameters: false,
        // SQLite's indexed-column spelling in PRIMARY KEY / UNIQUE constraint position:
        // `column-name [COLLATE <collation>] [ASC|DESC]` (engine-measured accepts, exprs and
        // NULLS FIRST/LAST rejected).
        constraint_column_collate_order: true,
        referential_action_cascade_set: true,
        check_constraint_subqueries: false,
    };
}

impl IndexAlterSyntax {
    /// The `SQLITE` preset for index alter syntax.
    pub const SQLITE: Self = Self {
        rename_constraint: false,
        alter_table_set_options: false,
        drop_primary_key: false,
        alter_column_add_identity: false,
        index_storage_parameters: false,
        drop_behavior: false,
        // SQLite's `DROP INDEX` is the shared name-list drop, not the MySQL `ON <table>` form.
        index_drop_on_table: false,
        index_concurrently: false,
        index_using_method: false,
        partial_index: true,
        // SQLite supports `CREATE INDEX IF NOT EXISTS` and per-key `NULLS FIRST`/`LAST`
        // (3.30+); it has no stored routines, so `routine_arg_types` is moot (off).
        index_if_not_exists: true,
        index_nulls_order: true,
        alter_table_extended: false,
        // SQLite's narrow `ALTER TABLE` never reaches these (it is not
        // `alter_table_extended`), so the guard/type-change gates are inert — held off to
        // keep the preset clean under `FeatureSet::feature_dependencies` (both ride
        // `alter_table_extended`). It does parse `DEFERRABLE` constraints and (like the ANSI
        // baseline) a `WITH [NO] DATA` clause.
        alter_nested_column_paths: false,
        alter_existence_guards: false,
        alter_column_set_data_type: false,
        routine_arg_types: false,
        routine_arg_defaults: false,
        routine_arg_modes: false,
        // No stored routines, so the routine `LANGUAGE` operand is moot (off).
        routine_language_string: false,
        alter_table_multiple_actions: false,
    };
}

impl ExistenceGuards {
    /// The `SQLITE` preset for existence guards.
    pub const SQLITE: Self = Self {
        if_exists: true,
        view_if_not_exists: true,
        create_database_if_not_exists: false,
    };
}

impl SelectSyntax {
    /// The `SQLITE` preset for select syntax.
    pub const SQLITE: Self = Self {
        distinct_on: false,
        select_into: false,
        empty_target_list: false,
        // SQLite has no `QUALIFY` clause (a DuckDB extension).
        qualify: false,
        // SQLite accepts a string literal as a projection alias after `AS` (`SELECT v AS
        // 'x'`) — engine-measured via rusqlite 3.53.2, where `'x'` becomes the result-column
        // name. Reuses the MySQL/DuckDB round-trip machinery (the alias renders back
        // single-quoted, which SQLite re-accepts). The *bare* (`AS`-less) form rides the
        // separate `bare_alias_string_literals` axis below (DuckDB accepts only the `AS` form,
        // so the two split).
        alias_string_literals: true,
        // SQLite also accepts the *bare* string alias (`SELECT v 'x'`; engine-measured) —
        // SQLite has no adjacent-string concatenation, so a string after an expression is
        // unambiguously the alias.
        bare_alias_string_literals: true,
        // SQLite has no `UNION [ALL] BY NAME` name-matched set operation (a DuckDB
        // extension); `BY` after a set operator is a syntax error there.
        union_by_name: false,
        wildcard_modifiers: false,
        wildcard_replace: false,
        intersect_all: false,
        except_all: false,
        // SQLite's `table.*` result-column is a non-aliasable production; a trailing alias
        // rejects (measured Reject on rusqlite with the table provisioned).
        qualified_wildcard_alias: false,
        // FROM-first SELECT is a DuckDB extension; SQLite rejects a statement-position
        // `FROM`.
        from_first: false,
        explicit_table: false,
        // SQLite rejects a ragged VALUES constructor at parse — engine-measured via
        // rusqlite `prepare`: "all VALUES must have the same number of terms" — so the
        // preset enforces equal row arity, matching DuckDB (the shared parse-time gate
        // `Parser::reject_ragged_values_rows`, fired in every VALUES position).
        values_rows_require_equal_arity: true,
        // SQLite has no parenthesized compound operand (a `select-core` is `SELECT`/
        // `VALUES`, never `( … )`); a leading `(` in statement / set-op / CTE-body /
        // CTAS / INSERT-source position is a syntax error. The FROM table-or-subquery
        // grouping and expression scalar-subquery keep their parenthesized query — a
        // complete standalone primary — through the parser's grouping context.
        parenthesized_query_operands: false,
        // SQLite spells the query-position VALUES constructor with bare `(…)` rows.
        values_row_constructor: true,
        // SQLite draws no `ColId`/`ColLabel` split, so its projection `AS` alias already
        // rejects reserved words via the non-empty `reserved_as_label` set — no reroute.
        as_alias_rejects_reserved: false,
        // A trailing comma in a list is a DuckDB tolerance; SQLite rejects it.
        trailing_comma: false,
        // The prefix colon alias is a DuckDB extension; a `:` at a select-item /
        // table-factor head is a parse error in SQLite.
        prefix_colon_alias: false,
        // Hive/Spark `LATERAL VIEW` is not SQLite; a post-FROM `LATERAL` is a parse
        // error there.
        lateral_view_clause: false,
        // The Oracle-style `START WITH`/`CONNECT BY` hierarchical query clause is not
        // SQLite; a post-WHERE `CONNECT BY`/`START WITH` is a parse error there.
        connect_by_clause: false,
    };
}

impl QueryTailSyntax {
    /// The `SQLITE` preset for query tail syntax.
    pub const SQLITE: Self = Self {
        fetch_first: false,
        limit_offset_comma: true,
        // SQLite has no `FOR UPDATE`/`FOR SHARE` row-locking clause.
        locking_clauses: false,
        // No locking clause at all, so the PostgreSQL strength/stacking refinements are
        // moot here.
        key_lock_strengths: false,
        stacked_locking_clauses: false,
        using_sample: false,
        // SQLite requires LIMIT before OFFSET; a bare leading OFFSET is a syntax error.
        leading_offset: false,
        limit_expressions: true,
        limit_percent: false,
        with_ties_requires_order_by: false,
        // BigQuery/ZetaSQL `|>` pipe syntax is not SQLite; off here. A `|>` after a query is
        // a parse error, and the token never lexes with the gate off.
        pipe_syntax: false,
        // ClickHouse `LIMIT n BY …` is not SQLite; a `BY` after `LIMIT` is a parse error.
        limit_by_clause: false,
        // ClickHouse `SETTINGS …` is not SQLite; a trailing `SETTINGS` is a parse error.
        settings_clause: false,
        // ClickHouse `FORMAT …` is not SQLite; a trailing `FORMAT` is a parse error.
        format_clause: false,
        // MSSQL `FOR XML`/`FOR JSON` is not SQLite; a trailing `FOR XML`/`FOR JSON` is a
        // parse error.
        for_xml_json_clause: false,
    };
}

impl GroupingSyntax {
    /// The `SQLITE` preset for grouping syntax.
    pub const SQLITE: Self = Self {
        grouping_sets: false,
        with_rollup: false,
        order_by_using: false,
        // `GROUP BY ALL` / `ORDER BY ALL` are DuckDB clause modes; SQLite reserves
        // `ALL` (`parse.y` keeps it out of the `%fallback ID` list), so either
        // spelling is a syntax error there.
        group_by_all: false,
        group_by_set_quantifier: false,
        order_by_all: false,
    };
}

impl UtilitySyntax {
    /// The `SQLITE` preset for utility syntax.
    pub const SQLITE: Self = Self {
        start_transaction: false,
        start_transaction_block_optional: false,
        transaction_work_keyword: false,
        begin_transaction_keyword: true,
        commit_transaction_keyword: true,
        rollback_transaction_keyword: true,
        transaction_name: true,
        begin_transaction_modes: false,
        transaction_savepoints: true,
        set_transaction: false,
        transaction_isolation_mode: false,
        transaction_access_mode: false,
        transaction_deferrable_mode: false,
        start_transaction_isolation_mode: false,
        start_transaction_deferrable_mode: false,
        start_transaction_consistent_snapshot: false,
        transaction_multiple_modes: false,
        transaction_mode_comma_required: false,
        transaction_modes_unique: false,
        abort_transaction_alias: false,
        end_transaction_alias: true,
        transaction_release: false,
        transaction_chain: false,
        release_savepoint_keyword_optional: true,
        copy: false,
        // `COPY INTO` is Snowflake bulk load/unload; SQLite has no such statement.
        copy_into: false,
        stage_references: false,
        comment_on: false,
        comment_if_exists: false,
        pragma: true,
        attach: true,
        // `KILL` and the MySQL `DESCRIBE`/`DESC` overloads are MySQL-only; SQLite's own
        // `EXPLAIN [QUERY PLAN]` keeps the ungated query-plan grammar.
        kill: false,
        // MySQL's `HANDLER` cursor family is not a SQLite statement.
        handler_statements: false,
        // MySQL's `INSTALL`/`UNINSTALL` `PLUGIN`/`COMPONENT` family is not a SQLite statement.
        plugin_component_statements: false,
        // MySQL's server-administration families are not SQLite statements.
        shutdown: false,
        restart: false,
        clone: false,
        import_table: false,
        help_statement: false,
        binlog: false,
        // MySQL's `CACHE INDEX` / `LOAD INDEX INTO CACHE` key-cache pair is not a SQLite
        // statement.
        key_cache_statements: false,
        // The `USE` catalog-switch statement is DuckDB/MySQL-only; SQLite has none.
        use_statement: false,
        // Moot: `use_statement` is off, so the name-arity refinement is unreachable.
        use_qualified_name: false,
        // Moot: `use_statement` is off, so the string-name refinement is unreachable.
        use_string_literal_name: false,
        // The DuckDB prepared-statement lifecycle and `CALL` are not SQLite statements.
        prepared_statements: false,
        // Moot: the typed parameter list widens `PREPARE`, which is already off.
        prepare_typed_parameters: false,
        // MySQL's `PREPARE ... FROM` lifecycle is not a SQLite statement either.
        prepared_statements_from: false,
        call: false,
        // `call` is off (no SQLite `CALL`), so its MySQL bare-name widening is moot and off.
        call_bare_name: false,
        load_extension: false,
        load_bare_name: false,
        load_data: false,
        reset_scope: false,
        detach_if_exists: false,
        // `DO` is the PostgreSQL anonymous code block; SQLite has no such statement.
        do_statement: false,
        // MySQL's `DO <expr-list>` statement; SQLite has none.
        do_expression_list: false,
        // MySQL's `LOCK/UNLOCK TABLES` and `LOCK/UNLOCK INSTANCE`; SQLite has neither
        // (its locking is implicit in transaction modes), so both stay undispatched.
        lock_tables: false,
        lock_instance: false,
        // SQLite's `BEGIN {DEFERRED|IMMEDIATE|EXCLUSIVE}` transaction-mode modifier
        // (engine-measured on rusqlite 3.53.2: all three accept, `BEGIN CONCURRENT`
        // rejects, doubling the modifier rejects).
        begin_transaction_mode: true,
        // MySQL's `XA` distributed-transaction family is MySQL-only; SQLite has no `XA`
        // statement, so the leading `XA` keyword is not dispatched.
        xa_transactions: false,
        // The standalone `RENAME TABLE`/`RENAME USER` statements are MySQL-only; SQLite
        // renames via `ALTER TABLE … RENAME TO`, so the leading `RENAME` is not dispatched.
        rename_statement: false,
        signal_diagnostics: false,
        // SQLite has no `EXPORT`/`IMPORT DATABASE` (its dump surface is the `.dump` shell
        // command, not SQL), so the leading keywords stay undispatched.
        export_import_database: false,
        // SQLite has no `UPDATE EXTENSIONS` statement, so the `EXTENSIONS` lookahead is never
        // taken and every `UPDATE` reaches the DML parser.
        update_extensions: false,
        // MySQL's `FLUSH` / `PURGE BINARY LOGS` server-administration statements — SQLite has
        // neither, so both leading-keyword gates stay off.
        flush: false,
        purge_binary_logs: false,
        replication_statements: false,
    };
}

impl ShowSyntax {
    /// The `SQLITE` preset for show syntax.
    pub const SQLITE: Self = Self {
        describe: false,
        describe_summarize: false,
        // SQLite has no SET/RESET/SHOW session statements and no GRANT/REVOKE.
        session_statements: false,
        set_value_reserved_words: KeywordSet::EMPTY,
        set_value_on_keyword: false,
        set_value_null_keyword: false,
        show_tables: false,
        show_columns: false,
        show_create_table: false,
        show_functions: false,
        show_routine_status: false,
        show_verbose: false,
        show_admin: false,
    };
}

impl MaintenanceSyntax {
    /// The `SQLITE` preset for maintenance syntax.
    pub const SQLITE: Self = Self {
        vacuum: true,
        // DuckDB's `VACUUM [ANALYZE] <table> (<cols>)` grammar is DuckDB-only; SQLite's
        // `VACUUM` rides `vacuum` and admits `[<schema>] INTO <expr>` instead.
        vacuum_analyze: false,
        reindex: true,
        analyze: true,
        // DuckDB's `ANALYZE <table> (<cols>)` column list is DuckDB-only; SQLite's
        // `ANALYZE` takes a bare name with no column list.
        analyze_columns: false,
        // `CHECKPOINT` and `LOAD` are PostgreSQL/DuckDB statements (SQLite's checkpoint is
        // the `PRAGMA wal_checkpoint` form, already covered by `pragma`), and the DuckDB
        // `RESET`-scope / `DETACH … IF EXISTS` extensions are DuckDB-only. SQLite's own
        // `DETACH [DATABASE] name` (via `attach`) has no `IF EXISTS` guard.
        checkpoint: false,
        checkpoint_database: false,
        // The MySQL admin-table verbs are MySQL-only; SQLite's `ANALYZE` is the bare
        // leading-`analyze` form, not `ANALYZE TABLE`.
        table_maintenance: false,
    };
}

impl AccessControlSyntax {
    /// The `SQLITE` preset for access control syntax.
    pub const SQLITE: Self = Self {
        alter_role_rename: false,
        access_control: false,
        // Moot: SQLite has no permission system, so `access_control` is already off.
        access_control_extended_objects: false,
        // SQLite has no accounts or roles.
        user_role_management: false,
        // Moot: SQLite has no permission system, so `access_control` is already off.
        access_control_account_grants: false,
    };
}

/// SQLite binding powers, explicitly enumerated with left-associative comparisons and
/// the tight unary bitwise-NOT rank.
///
/// **Comparison associativity.** The comparison row is `Assoc::Left`, the same delta
/// MySQL applies: SQLite parses `1 < 2 < 3` as `(1 < 2) < 3` (the 0/1 result feeding the
/// outer comparison) where ANSI/PostgreSQL reject the chain. Rewriting the whole
/// `comparison` row from one representative operator (`Eq`) moves every comparison
/// variant — including the `==` spelling and the `GLOB`/`MATCH`/`REGEXP` keyword
/// operators — together; only associativity changes.
///
/// **Prefix `~` rank.** SQLite binds unary `~` tightly (its precedence table puts `~` with
/// the unary sign, above every binary operator), so `~ 1 + 1` groups `(~ 1) + 1` —
/// engine-measured, and the *opposite* of PostgreSQL/DuckDB's loose placement. The binary
/// bitwise operators keep STANDARD's shared rank (engine-measured: `1 | 2 & 2` is
/// `(1 | 2) & 2`, all four at one level between additive and comparison).
pub const SQLITE_BINDING_POWERS: BindingPowerTable = BindingPowerTable {
    or: BindingPower {
        left: 10,
        right: 11,
        assoc: Assoc::Left,
    },
    xor: BindingPower {
        left: 15,
        right: 16,
        assoc: Assoc::Left,
    },
    and: BindingPower {
        left: 20,
        right: 21,
        assoc: Assoc::Left,
    },
    comparison: BindingPower {
        left: 40,
        right: 41,
        assoc: Assoc::Left,
    },
    range_predicate_override: None,
    is_predicate_override: None,
    double_equals: BindingPower {
        left: 40,
        right: 41,
        assoc: Assoc::Left,
    },
    additive: BindingPower {
        left: 50,
        right: 51,
        assoc: Assoc::Left,
    },
    multiplicative: BindingPower {
        left: 60,
        right: 61,
        assoc: Assoc::Left,
    },
    exponent: BindingPower {
        left: 65,
        right: 66,
        assoc: Assoc::Left,
    },
    string_concat: BindingPower {
        left: 45,
        right: 46,
        assoc: Assoc::Left,
    },
    any_operator: BindingPower {
        left: 45,
        right: 46,
        assoc: Assoc::Left,
    },
    json_get: BindingPower {
        left: 45,
        right: 46,
        assoc: Assoc::Left,
    },
    bitwise_or: BindingPower {
        left: 45,
        right: 46,
        assoc: Assoc::Left,
    },
    bitwise_and: BindingPower {
        left: 45,
        right: 46,
        assoc: Assoc::Left,
    },
    bitwise_shift: BindingPower {
        left: 45,
        right: 46,
        assoc: Assoc::Left,
    },
    bitwise_xor: BindingPower {
        left: 45,
        right: 46,
        assoc: Assoc::Left,
    },
    prefix_not: 30,
    prefix_sign: 80,
    // SQLite's `~` binds like the unary sign, not PostgreSQL/DuckDB's between-arithmetic
    // rank.
    prefix_bitwise_not: 80,
    at_time_zone: BindingPower {
        left: 70,
        right: 71,
        assoc: Assoc::Left,
    },
    collate: BindingPower {
        left: 74,
        right: 75,
        assoc: Assoc::Left,
    },
    subscript: BindingPower {
        left: 84,
        right: 85,
        assoc: Assoc::Left,
    },
    typecast: BindingPower {
        left: 88,
        right: 89,
        assoc: Assoc::Left,
    },
    field_selection: BindingPower {
        left: 92,
        right: 93,
        assoc: Assoc::Left,
    },
};

impl FeatureSet {
    /// SQLite as dialect data, including the position-aware reserved sets that its
    /// `%fallback` keyword model requires.
    pub const SQLITE: Self = Self {
        // SQLite compares identifiers case-insensitively (ASCII) while preserving the
        // written text; `Lower` models that identity (fold lower, render exact).
        identifier_casing: Casing::Lower,
        // `"a"`, `` `a` ``, and `[a]`. `"` quotes identifiers, so `double_quoted_strings`
        // is off (see `StringLiteralSyntax::SQLITE`) — the DQS fallback is excluded.
        identifier_quotes: SQLITE_IDENTIFIER_QUOTES,
        // SQLite sorts NULLs first under ascending order (NULL ranks lowest).
        default_null_ordering: NullOrdering::NullsFirst,
        // SQLite's reserved set is far smaller than ANSI's (its `%fallback` frees
        // `END`/`DESC`/`ASC`/`ANALYZE`/…), so it gets its own hand-composed per-position
        // sets rather than reusing the PostgreSQL-derived ANSI ones.
        reserved_column_name: SQLITE_RESERVED_COLUMN_NAME,
        reserved_function_name: SQLITE_RESERVED_FUNCTION_NAME,
        reserved_type_name: SQLITE_RESERVED_TYPE_NAME,
        reserved_bare_alias: SQLITE_RESERVED_BARE_ALIAS,
        // SQLite rejects reserved words in ColLabel position too (`SELECT 1 AS delete`,
        // `x.update`, `schema.case`) — no ColId/ColLabel split, unlike PostgreSQL.
        reserved_as_label: SQLITE_RESERVED_AS_LABEL,
        // SQLite relation names are `schema.table` (two parts); a database is the schema
        // namespace, so there is no catalog qualifier — `a.b.c` in table/index position
        // is a syntax error.
        catalog_qualified_names: false,
        // Backtick/bracket quotes, `?`/`:`/`@`/`$` placeholders, and `0x` integers all
        // dispatch from the standard byte classes gated by the knobs below — plus the
        // vertical tab as a whitespace-run *continuation* (`SQLITE_BYTE_CLASSES`):
        // `rusqlite`-measured, a `\v` rides an open whitespace run (`"\x20\x0b"` accepts)
        // but cannot start one (lone `"\x0b"` rejects), the sole byte-class divergence.
        byte_classes: SQLITE_BYTE_CLASSES,
        // SQLite's comparison family is left-associative like MySQL's, not `STANDARD`'s
        // `NonAssoc` (`SELECT 1 < 2 < 3` is legal SQLite meaning `(1 < 2) < 3`), so it
        // needs its own table (`SQLITE_BINDING_POWERS`).
        binding_powers: SQLITE_BINDING_POWERS,
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        string_literals: StringLiteralSyntax::SQLITE,
        numeric_literals: NumericLiteralSyntax::SQLITE,
        parameters: ParameterSyntax::SQLITE,
        // SQLite has no `@name`/`@@sysvar` session variables — its `@name` is a bind
        // parameter (`ParameterSyntax::SQLITE`), not a variable — so these stay off.
        session_variables: SessionVariableSyntax::ANSI,
        // SQLite identifiers are letters/digits/`_` only (`$` leads a placeholder, not an
        // identifier byte) but a `'name'` string is admitted in the relation-target and
        // `PRIMARY KEY`/`UNIQUE` column-name positions.
        identifier_syntax: IdentifierSyntax::SQLITE,
        // SQLite's join grammar is the ANSI surface (no `LATERAL`/`STRAIGHT_JOIN`/…)
        // minus the FROM table-alias column list, which SQLite lacks.
        table_expressions: TableExpressionSyntax::SQLITE,
        join_syntax: JoinSyntax::SQLITE,
        table_factor_syntax: TableFactorSyntax::SQLITE,
        expression_syntax: ExpressionSyntax::SQLITE,
        operator_syntax: OperatorSyntax::SQLITE,
        call_syntax: CallSyntax::SQLITE,
        string_func_forms: StringFuncForms::SQLITE,
        aggregate_call_syntax: AggregateCallSyntax::SQLITE,
        // SQLite has the standard `LIKE` predicate but neither `ILIKE` nor `SIMILAR TO`;
        // it diverges from ANSI only by accepting an empty `IN ()` list.
        predicate_syntax: PredicateSyntax::SQLITE,
        // SQLite `||` concatenates (the standard meaning), and `&&` is not an operator.
        pipe_operator: PipeOperator::StringConcat,
        double_ampersand: DoubleAmpersand::Unsupported,
        keyword_operators: KeywordOperators::Sqlite,
        // SQLite has no bitwise XOR operator (engine-measured: both `#` and `^` reject), and
        // `^` has no infix meaning at all.
        caret_operator: CaretOperator::Unsupported,
        hash_bitwise_xor: false,
        // SQLite comments are `--` and `/* … */`; no `#` line comment, block comments do not
        // nest, and an unterminated `/* …` at EOF is silently closed (`CommentSyntax::SQLITE`).
        comment_syntax: CommentSyntax::SQLITE,
        mutation_syntax: MutationSyntax::SQLITE,
        statement_ddl_gates: StatementDdlGates::SQLITE,
        create_table_clause_syntax: CreateTableClauseSyntax::SQLITE,
        column_definition_syntax: ColumnDefinitionSyntax::SQLITE,
        constraint_syntax: ConstraintSyntax::SQLITE,
        index_alter_syntax: IndexAlterSyntax::SQLITE,
        existence_guards: ExistenceGuards::SQLITE,
        select_syntax: SelectSyntax::SQLITE,
        query_tail_syntax: QueryTailSyntax::SQLITE,
        grouping_syntax: GroupingSyntax::SQLITE,
        // SQLite's `PRAGMA` and `ATTACH`/`DETACH` config statements and the
        // `VACUUM`/`REINDEX`/`ANALYZE` maintenance trio are dispatched; the PostgreSQL
        // `COPY`/`COMMENT ON` flags stay off (SQLite has neither).
        utility_syntax: UtilitySyntax::SQLITE,
        show_syntax: ShowSyntax::SQLITE,
        maintenance_syntax: MaintenanceSyntax::SQLITE,
        access_control_syntax: AccessControlSyntax::SQLITE,
        // SQLite's flexible typing accepts an arbitrary affinity type name through the
        // user-defined-type fallback, so it needs no extended type vocabulary — save the
        // one built-in decoration affinity absorbs, integer display widths (`INT(11)`).
        type_name_syntax: TypeNameSyntax::SQLITE,
        // No SQLite-specific Tier-1 output spelling yet: a target-dialect render falls
        // back to the portable ANSI canonical spellings.
        target_spelling: TargetSpelling::Ansi,
    };
}

/// Prefer [`FeatureSet::SQLITE`] for struct update.
pub const SQLITE: FeatureSet = FeatureSet::SQLITE;

// Compile-time proof the SQLite preset claims no shared tokenizer trigger twice —
// notably that `"` quotes identifiers while `double_quoted_strings` stays off (no
// `DoubleQuoteStringVersusIdentifier`), and that `named_dollar` never meets a
// contending `dollar_quoted_strings`. The ratchet fails the build if a future edit
// adds a conflict, rather than silently shadowing a meaning (uniform with the ANSI
// and MySQL asserts).
const _: () = assert!(FeatureSet::SQLITE.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: no refinement
// flag rides an unset base, and no two features contend for one parser-position head.
const _: () = assert!(FeatureSet::SQLITE.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::SQLITE.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::super::{
        LexicalConflict, RESERVED_BARE_ALIAS, RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME,
        RESERVED_TYPE_NAME,
    };
    use super::*;
    use crate::ast::{BinaryOperator, EqualsSpelling, RegexpSpelling};
    use crate::precedence::STANDARD_BINDING_POWERS;

    #[test]
    fn sqlite_reserved_set_is_smaller_than_ansi_freeing_sqlite_identifiers() {
        // The inventory evidence: SQLite's `%fallback` frees words the ANSI/PostgreSQL
        // model reserves, so `DESC`/`ASC`/`END`/`ANALYZE` serve as bare identifiers
        // (`CREATE TABLE z (end INT)`, `SELECT 'a' AS desc`, …). Each is reserved in
        // some ANSI position and free in every SQLite one.
        for keyword in [Keyword::Desc, Keyword::Asc, Keyword::End, Keyword::Analyze] {
            assert!(
                !SQLITE_RESERVED_COLUMN_NAME.contains(keyword),
                "{keyword:?} must be a free SQLite identifier",
            );
            assert!(!SQLITE_RESERVED_FUNCTION_NAME.contains(keyword));
            assert!(!SQLITE_RESERVED_TYPE_NAME.contains(keyword));
            assert!(!SQLITE_RESERVED_BARE_ALIAS.contains(keyword));
        }
        // At least one of them is genuinely reserved under ANSI, so the sets diverge.
        assert!(
            RESERVED_COLUMN_NAME.contains(Keyword::Desc)
                || RESERVED_BARE_ALIAS.contains(Keyword::Desc),
            "DESC should be reserved somewhere under the ANSI/PostgreSQL model",
        );

        // The core structural keywords stay reserved in every position, so ordinary
        // SQL still parses (`SELECT`/`FROM`/`WHERE` cannot be bare identifiers).
        for keyword in [
            Keyword::Select,
            Keyword::From,
            Keyword::Where,
            Keyword::Join,
        ] {
            assert!(SQLITE_RESERVED_COLUMN_NAME.contains(keyword));
        }

        // The keyword operators double as identifier / function names in SQLite (the
        // built-in `glob(pattern, string)`), so they are deliberately *not* reserved.
        for keyword in [Keyword::Glob, Keyword::Match, Keyword::Regexp] {
            assert!(!SQLITE_RESERVED_FUNCTION_NAME.contains(keyword));
        }
        // The four ANSI sets are referenced so their `use` is load-bearing.
        let _ = (RESERVED_FUNCTION_NAME, RESERVED_TYPE_NAME);
    }

    #[test]
    fn sqlite_join_keywords_are_reserved_by_position_not_uniformly() {
        // The seven `JOIN_KW` keywords: admissible as a name (`nm ::= JOIN_KW`) — column /
        // table name, function name, `AS` label — but reserved as a bare alias and a type
        // name (`ids ::= ID|STRING`). Probed cell-by-cell against rusqlite 3.53.2 (the
        // conformance `sqlite_position_aware_reserved_matches_the_engine` oracle test).
        for keyword in [
            Keyword::Cross,
            Keyword::Inner,
            Keyword::Left,
            Keyword::Natural,
            Keyword::Outer,
            Keyword::Right,
            Keyword::Full,
        ] {
            // Admitted as a name (`CREATE TABLE left(…)`, `FROM cross`, `SELECT 1 AS left`).
            assert!(
                !SQLITE_RESERVED_COLUMN_NAME.contains(keyword),
                "{keyword:?} must be admissible as a SQLite table/column name",
            );
            assert!(!SQLITE_RESERVED_FUNCTION_NAME.contains(keyword));
            assert!(!SQLITE_RESERVED_AS_LABEL.contains(keyword));
            // Reserved as a bare alias (so `FROM t cross JOIN u` keeps the join) and a type
            // name (`CAST(1 AS cross)` is a syntax error).
            assert!(
                SQLITE_RESERVED_BARE_ALIAS.contains(keyword),
                "{keyword:?} must be reserved as a SQLite bare alias (the JOIN guard)",
            );
            assert!(SQLITE_RESERVED_TYPE_NAME.contains(keyword));
        }
    }

    #[test]
    fn sqlite_name_reserved_residuals_are_reserved_as_a_name_but_isnull_notnull_bare_admit() {
        // `RETURNING`/`NOTHING`/`ISNULL`/`NOTNULL` are reserved in every NAME position
        // (probed: `SELECT isnull`, `AS returning`, `CREATE TABLE nothing(…)` syntax-reject)
        // — the four AS-label over-acceptances the sibling sweep found.
        for keyword in [
            Keyword::Returning,
            Keyword::Nothing,
            Keyword::Isnull,
            Keyword::Notnull,
        ] {
            assert!(
                SQLITE_RESERVED_COLUMN_NAME.contains(keyword),
                "{keyword:?} must be reserved as a SQLite name",
            );
            assert!(SQLITE_RESERVED_AS_LABEL.contains(keyword));
            assert!(SQLITE_RESERVED_FUNCTION_NAME.contains(keyword));
            assert!(SQLITE_RESERVED_TYPE_NAME.contains(keyword));
        }
        // `RETURNING`/`NOTHING` are also reserved as a bare alias (`SELECT 1 nothing` rejects)…
        assert!(SQLITE_RESERVED_BARE_ALIAS.contains(Keyword::Returning));
        assert!(SQLITE_RESERVED_BARE_ALIAS.contains(Keyword::Nothing));
        // …but `ISNULL`/`NOTNULL` are ADMITTED as a bare alias: SQLite reads `SELECT 1
        // isnull` as the postfix null-test operator (which we do not model), so admitting
        // them there matches its ACCEPT verdict rather than over-rejecting.
        assert!(!SQLITE_RESERVED_BARE_ALIAS.contains(Keyword::Isnull));
        assert!(!SQLITE_RESERVED_BARE_ALIAS.contains(Keyword::Notnull));
    }

    #[test]
    fn sqlite_preset_is_lexically_consistent_with_double_quote_resolved_to_identifier() {
        // The load-bearing decision: `"` quotes identifiers and `double_quoted_strings`
        // stays off, so the preset carries no lexical conflict.
        assert!(FeatureSet::SQLITE.is_lexically_consistent());
        assert_eq!(FeatureSet::SQLITE.lexical_conflict(), None);

        // Flipping `double_quoted_strings` on — while `"` still quotes identifiers —
        // is exactly the conflict the decision avoids: the two claim the same `"`
        // trigger. Proven by construction so the decision cannot silently rot.
        let with_dqs = FeatureSet::SQLITE.with(super::super::FeatureDelta::EMPTY.string_literals(
            StringLiteralSyntax {
                double_quoted_strings: true,
                ..StringLiteralSyntax::SQLITE
            },
        ));
        assert_eq!(
            with_dqs.lexical_conflict(),
            Some(LexicalConflict::DoubleQuoteStringVersusIdentifier),
        );
    }

    #[test]
    fn sqlite_named_dollar_parameter_never_meets_dollar_quoting() {
        // `$name` is on and `$tag$…$tag$` dollar-quoting is off in the preset (see
        // `ParameterSyntax::SQLITE` / `StringLiteralSyntax::SQLITE`), so the shared
        // `$`+identifier-start trigger is uncontended. Turning dollar-quoting on is the
        // tracked conflict — SQLite has no dollar-quoting, so no shipped preset does.
        let with_dollar_quote = FeatureSet::SQLITE.with(
            super::super::FeatureDelta::EMPTY.string_literals(StringLiteralSyntax {
                dollar_quoted_strings: true,
                ..StringLiteralSyntax::SQLITE
            }),
        );
        assert_eq!(
            with_dollar_quote.lexical_conflict(),
            Some(LexicalConflict::NamedDollarParameterVersusDollarQuotedString),
        );
    }

    #[test]
    fn sqlite_keyword_operators_map_glob_match_regexp() {
        // `GLOB`/`MATCH` get their own operator keys; `REGEXP` folds onto the shared
        // regex operator with the `Regexp` spelling tag (the MySQL `RLIKE`/`REGEXP`
        // round-trip pattern). Every other keyword is inert (ends the expression).
        assert_eq!(
            KeywordOperators::Sqlite.binary_operator(Keyword::Glob),
            Some(BinaryOperator::Glob),
        );
        assert_eq!(
            KeywordOperators::Sqlite.binary_operator(Keyword::Match),
            Some(BinaryOperator::Match),
        );
        assert_eq!(
            KeywordOperators::Sqlite.binary_operator(Keyword::Regexp),
            Some(BinaryOperator::Regexp(RegexpSpelling::Regexp)),
        );
        assert_eq!(
            KeywordOperators::Sqlite.binary_operator(Keyword::Div),
            None,
            "DIV is MySQL's, not SQLite's",
        );
    }

    #[test]
    fn sqlite_comparison_row_is_left_associative_carrying_the_new_operators() {
        // The binding-power deltas from STANDARD: the comparison row goes `NonAssoc` ->
        // `Left` (so `1 < 2 < 3` / `1 == 2 == 3` chain left-associatively), and prefix `~`
        // takes the tight unary-sign rank (SQLite binds `~` above every binary operator, the
        // opposite of PostgreSQL/DuckDB's loose placement — engine-measured `~ 1 + 1` is
        // `(~ 1) + 1`).
        let mut expected = STANDARD_BINDING_POWERS;
        expected.comparison.assoc = Assoc::Left;
        // `==` rides the comparison row with `=` (SQLite treats them identically), so the
        // `double_equals` field moves to `Left` with `comparison` — DuckDB is the only
        // dialect that splits `==` off the comparisons.
        expected.double_equals.assoc = Assoc::Left;
        expected.prefix_bitwise_not = expected.prefix_sign;
        assert_eq!(SQLITE_BINDING_POWERS, expected);

        // The bitwise binaries keep STANDARD's shared rank (one level between additive and
        // comparison, all left-associative): SQLite groups `1 | 2 & 2` as `(1 | 2) & 2`.
        for op in [
            BinaryOperator::BitwiseOr,
            BinaryOperator::BitwiseAnd,
            BinaryOperator::BitwiseShiftLeft,
            BinaryOperator::BitwiseShiftRight,
        ] {
            assert_eq!(
                SQLITE_BINDING_POWERS.binary(&op),
                STANDARD_BINDING_POWERS.binary(&op),
            );
        }

        // Both `Eq` spellings and the `GLOB`/`MATCH`/`REGEXP` operators fold onto the
        // comparison row, so they ride the associativity delta together with `=`.
        for op in [
            BinaryOperator::Eq(EqualsSpelling::Single),
            BinaryOperator::Eq(EqualsSpelling::Double),
            BinaryOperator::Glob,
            BinaryOperator::Match,
            BinaryOperator::Regexp(RegexpSpelling::Regexp),
        ] {
            let bp = SQLITE_BINDING_POWERS.binary(&op);
            assert_eq!(bp.assoc, Assoc::Left, "{op:?} rides the comparison row");
            assert_eq!(bp.left, 40, "{op:?} keeps the STANDARD left rank");
            assert_eq!(bp.right, 41, "{op:?} keeps the STANDARD right rank");
        }
    }
}
