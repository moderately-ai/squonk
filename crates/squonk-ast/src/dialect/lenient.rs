// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The optional `LENIENT` tooling preset — an honest, documented permissive union.
//!
//! Gated behind the default-off `lenient` cargo feature, so a build without it
//! compiles none of this data. [`FeatureSet::LENIENT`](super::FeatureSet::LENIENT) is the "parse anything" mode tooling reaches
//! for when it must accept SQL of unknown origin: it enables the permissive value of
//! every independent feature and, where two features contend for one tokenizer trigger,
//! resolves the conflict explicitly (documented per rule below).
//!
//! It is deliberately **not** called "generic": "generic" is [`ANSI`](super::ANSI), the
//! principled SQL:2016 baseline. `LENIENT` is the opposite construction — a maximal
//! union whose every inclusion and every conflict-resolution is spelled out, never a
//! vibe-union of whatever several dialects happen to accept. The honesty bar is that a
//! reader can predict, from this module alone, exactly what `LENIENT` accepts and which
//! meaning it picks for every contested form.
//!
//! # Conflict-resolution rules
//!
//! Most features are independent and simply take their accepting value. The exceptions
//! are the features that claim a *shared* context-free tokenizer trigger; for those,
//! enabling both claimants is a [`LexicalConflict`](super::LexicalConflict), so `LENIENT`
//! picks one meaning and forgoes the other:
//!
//! 1. **`"…"` is a quoted identifier, not a string.** `double_quoted_strings` is OFF so
//!    `"` stays an [identifier quote](LENIENT_IDENTIFIER_QUOTES). The whole point of the
//!    multi-quote union is to accept `"weird name"` as an identifier everywhere; the
//!    MySQL `ANSI_QUOTES`-off reading of `"…"` as a string is the sacrifice.
//! 2. **`[…]` is a quoted identifier; `[`-punctuation syntax is off.** The tokenizer
//!    resolves `[` context-free, so the same byte cannot also be array subscript /
//!    constructor punctuation. `subscript`, `array_constructor`, and
//!    `collection_literals` are OFF (and array *type* suffixes `T[]`, which are not
//!    feature-gated, likewise will not parse). T-SQL `[bracketed]` identifiers win;
//!    PostgreSQL `a[1]` / `ARRAY[…]` and the DuckDB `[1, 2]` list literal are the
//!    sacrifice.
//! 3. **`$<digit>` is a positional parameter, not money.** `money_literals` is OFF and
//!    `positional_dollar` is ON. The PostgreSQL `$1` parameter is the dominant real-world
//!    meaning; the T-SQL `$1234.56` money literal is the sacrifice (the scanner tries
//!    money first, so the two cannot coexist on `$`+digit).
//! 4. **`||` is string concatenation, not logical OR.** [`PipeOperator::StringConcat`] is
//!    the SQL-standard / ANSI / PostgreSQL meaning (and MySQL under `PIPES_AS_CONCAT`);
//!    MySQL's default `||`=OR is the sacrifice. Both still parse — only the operator's
//!    identity and precedence differ.
//! 5. **Reserved-identifier model is ANSI's.** `LENIENT` keeps the position-aware
//!    [`RESERVED_COLUMN_NAME`] family. It is deliberately
//!    *not* the empty set (the load-bearing reserved words must stay reserved or the
//!    grammar cannot disambiguate `SELECT a select b`) and *not* a cross-dialect
//!    intersection (that would couple `lenient` to the gated `mysql`/`postgres` data). A
//!    handful of words a specific dialect frees but ANSI reserves (e.g. `OFFSET`) stay
//!    reserved — quote them to use as identifiers.
//! 6. **Identity folding is [`Casing::Preserve`].** Neither ANSI's upper-fold nor the
//!    PostgreSQL/MySQL lower-fold is imposed; exact text is preserved. This is a tooling
//!    convenience (do not invent a fold a source we cannot identify never asked for) and
//!    does not affect what parses.
//! 7. **Null ordering is [`NullOrdering::NullsLast`]** (the ANSI/PostgreSQL default).
//!    MySQL's nulls-first default is the sacrifice; it only affects the semantics of an
//!    omitted `NULLS FIRST`/`LAST`, never acceptance.
//! 8. **Bitwise XOR is `^`, not `#`.** [`CaretOperator::BitwiseXor`] admits the MySQL `^`
//!    XOR spelling; the PostgreSQL `#` spelling would need `#` to lex as an operator, but
//!    `#` opens a line comment here (rule for `#`, [`CommentSyntax::LENIENT`]), so
//!    `#`-XOR is the sacrifice ([`LexicalConflict::HashXorOperatorVersusHashComment`](super::LexicalConflict::HashXorOperatorVersusHashComment)).
//!    Its precedence follows the STANDARD table (looser than additive), not MySQL's tight
//!    `^` rank — a precedence-only sacrifice, like rule 4's `||`.
//!
//! Rules 1-8 above are the *lexical* conflict resolutions — a shared tokenizer trigger
//! whose either/or is a [`LexicalConflict`](super::LexicalConflict). Many permissive
//! choices genuinely are pure additions with no contended trigger: every
//! string/number/parameter prefix form, `&&`-as-`AND`, the MySQL keyword infix operators,
//! `#` line comments, MySQL `/*!…*/` versioned comments (always included — see
//! [`CommentSyntax::LENIENT`]), `$`-in-identifiers, and most of the table / mutation /
//! select / type-name / utility (`COPY`) extension surface.
//!
//! But the *statement-head* extensions are not blanket-additive: several leading keywords
//! are claimed by two or more features, and `LENIENT` resolves each by a lookahead split, a
//! dispatch precedence, or a deliberate one-reading exclusion. Those resolutions — including
//! the flags this preset turns *off* (`do_expression_list`, `prepared_statements_from`,
//! `drop_database`, `index_drop_on_table`, `access_control_account_grants`,
//! `variable_assignment`) and the accepted-both unions (`LOAD`, `VACUUM`, `ANALYZE`,
//! `ALTER VIEW`, `ALTER DATABASE`, `IMPORT`, `UPDATE`, `LOCK`/`UNLOCK`, `CACHE`) — are
//! enumerated, one row per head, in the
//! [`MULTI_CLAIMANT_STATEMENT_HEADS`](super::MULTI_CLAIMANT_STATEMENT_HEADS) ledger. Each
//! `false` in a statement-gate field below that carries a "stays off" / conflict-resolution
//! note has a corresponding exclusion row there; the ledger's `lenient_exclusions_match_the_ledger`
//! test proves that correspondence is complete in both directions.

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
use crate::precedence::{
    Assoc, BindingPower, BindingPowerTable, IS_PREDICATE_BELOW_COMPARISON,
    RANGE_PREDICATE_ABOVE_COMPARISON, STANDARD_SET_OPERATION_BINDING_POWERS,
};

/// The permissive multi-style identifier quoting `LENIENT` accepts: the SQL-standard
/// `"…"`, the MySQL backtick `` `…` ``, and the T-SQL bracket `[…]` — all at once.
///
/// This is the one real cost of "parse anything": the tokenizer matches an opening
/// delimiter against the whole set ([`FeatureSet::identifier_quotes`] is already a
/// slice, and the scanner iterates it), so no per-dialect single-style assumption is
/// baked in. The three openers (`"`, `` ` ``, `[`) are distinct bytes, so their order is
/// immaterial — there is no precedence ambiguity to resolve. `"` appears here (and
/// `double_quoted_strings` is correspondingly OFF) per conflict-resolution rule 1.
pub const LENIENT_IDENTIFIER_QUOTES: &[IdentifierQuote] = &[
    IdentifierQuote::Symmetric('"'),
    IdentifierQuote::Symmetric('`'),
    IdentifierQuote::Asymmetric {
        open: '[',
        close: ']',
    },
];

impl CommentSyntax {
    /// `LENIENT`: accept `#` line comments (MySQL) on top of the baseline `--` / `/* */`.
    /// Safe with [`STANDARD_BYTE_CLASSES`], where `#` is not an identifier-start byte
    /// (conflict-resolution: a `#`-led identifier would be shadowed by the comment).
    ///
    /// MySQL `/*!…*/` versioned comments are conditional inclusion with an unbounded
    /// version gate (`u32::MAX`): a version-agnostic reader executes every conditional
    /// region rather than modelling one server's skip window, so a MySQL or MariaDB
    /// dump's `/*!NNNNN … */` bodies always parse. Block-comment *nesting* stays on —
    /// the two knobs pull in opposite directions for MySQL fidelity, but `LENIENT`
    /// keeps the permissive PostgreSQL superset it has always accepted (nesting
    /// rejects strictly fewer inputs than it accepts only on the degenerate
    /// `/* a /* b */`-unbalanced family, which no dump emits).
    pub const LENIENT: Self = Self {
        line_comment_hash: true,
        // Off: a `\r` that does not end a comment keeps the comment running longer, which
        // rejects strictly fewer inputs — the permissive reading `LENIENT` favours. (A
        // lone-`\r`-separated old-Mac source would merge lines into one comment, the only
        // input this changes; `\r\n` endings still terminate at the `\n`.)
        line_comment_ends_at_carriage_return: false,
        nested_block_comments: true,
        versioned_comments: Some(u32::MAX),
        // Union widening: SQLite silently closes an unterminated `/* …` at EOF, so the
        // permissive union accepts it too (a pure accept-side addition — a `/*` with a byte
        // after it that runs off the end becomes trailing trivia rather than an error).
        unterminated_block_comment_at_eof: true,
    };
}

impl StringLiteralSyntax {
    /// `LENIENT`: every dialect string form *except* `double_quoted_strings`.
    ///
    /// `double_quoted_strings` is OFF because `"` is a quoted-identifier delimiter here
    /// (conflict-resolution rule 1). `backslash_escapes` is ON: it is a strict superset
    /// of escape recognition — the standard `''` doubling still closes a string, and `\'`
    /// additionally escapes — so it accepts the most input (the one form it changes is a
    /// trailing `'a\'`, which becomes an escaped quote rather than a close).
    ///
    /// `national_strings` is ON from the MySQL/T-SQL presets (PostgreSQL does not arm it —
    /// its scanner has no `N'…'` constant and reads `N'x'` as the typed literal `nchar 'x'`;
    /// see the `POSTGRES` preset). Keeping it here means `N'x'` lexes as one national-string
    /// token under LENIENT, shadowing that PG typed-literal reading. This is a grammar-level
    /// shape choice, not a [`LexicalConflict`](super::LexicalConflict): the alternative
    /// reading is the parser's typed-literal fallback, not a second enabled lexer claimant,
    /// and both readings *accept* — so the union keeps the token form, consistent with the
    /// accept-most-input model.
    pub const LENIENT: Self = Self {
        escape_strings: true,
        dollar_quoted_strings: true,
        national_strings: true,
        double_quoted_strings: false,
        backslash_escapes: true,
        unicode_strings: true,
        bit_string_literals: true,
        // The lenient superset keeps `X'…'` the permissive deferred bit-string (odd hex
        // tolerated), so the eager even-byte blob gate stays off.
        blob_literals: false,
        charset_introducers: true,
        // Accept MySQL's same-line adjacent-literal concatenation in the superset.
        same_line_adjacent_concat: true,
    };
}

impl NumericLiteralSyntax {
    /// `LENIENT`: every radix and separator form, but *not* `money_literals`.
    ///
    /// `money_literals` is OFF because `$`+digit is a positional parameter here
    /// (conflict-resolution rule 3): the two cannot coexist, and `$1` parameters are the
    /// dominant real-world meaning.
    pub const LENIENT: Self = Self {
        hex_integers: true,
        octal_integers: true,
        binary_integers: true,
        underscore_separators: true,
        // Every separator form, including PG's leading-underscore radix body (`0x_1F`).
        // A pure widening: with `reject_trailing_junk` off `0x_1F` already accepts as a
        // bare `0` plus a `x_1F` alias, so this only folds it into one number token.
        radix_leading_underscore: true,
        money_literals: false,
        // Lenient accepts the maximal input surface, so it never rejects trailing
        // numeric junk — the leftover falls through to the ordinary word/alias scan.
        reject_trailing_junk: false,
    };
}

impl ParameterSyntax {
    /// `LENIENT`: every placeholder spelling — PostgreSQL `$1`, ODBC/JDBC `?`,
    /// Oracle/SQLite `:name`, and T-SQL `@name`. The sigils are lookahead-disjoint, so
    /// enabling all four is a pure addition. `@name` keeps its parameter meaning under
    /// `LENIENT` (`named_at`), so `user_variables` stays off in
    /// [`SessionVariableSyntax::LENIENT`] — the two claim the same `@name` trigger — but
    /// the disjoint `@@sysvar` form is still admitted there.
    ///
    /// The SQLite `$name` form (`named_dollar`) is the one placeholder left off: it
    /// contends with the PostgreSQL `$tag$…$tag$` dollar-quote this union already
    /// admits ([`StringLiteralSyntax::LENIENT`], both lead with `$`+identifier), so the
    /// union resolves that `$` trigger to dollar-quoting — the spelled-out
    /// conflict-resolution the `LENIENT` honesty bar requires.
    pub const LENIENT: Self = Self {
        positional_dollar: true,
        positional_dollar_large: true,
        anonymous_question: true,
        named_colon: true,
        named_at: true,
        named_dollar: false,
        // Union widening: SQLite's numbered `?NNN` parameter is follow-set-disjoint from the
        // anonymous `?` (it needs a digit), a pure accept-side addition.
        numbered_question: true,
    };
}

impl SessionVariableSyntax {
    /// `LENIENT`: the `@@[scope.]name` MySQL system-variable form, additive over the
    /// `@name` parameter that [`ParameterSyntax::LENIENT`] already lexes (`@@` is
    /// disjoint from `@name` by its second `@`). `user_variables` stays *off*: `@name`
    /// is claimed as a named-at parameter here, and the two meanings cannot both hold
    /// the trigger (a [`LexicalConflict`](super::LexicalConflict)).
    pub const LENIENT: Self = Self {
        user_variables: false,
        system_variables: true,
        // The MySQL variable-assignment `SET` grammar (and its `:=` operator) stays *off*
        // here: LENIENT lexes `@name` as a *parameter* (`user_variables` off, above), so the
        // user-variable `SET @v = …` item cannot lex anyway, and keeping the generic
        // PostgreSQL `SET` avoids reshaping every `SET x = …` in the superset. A MySQL-only
        // behaviour, enabled solely by the fitted `MySql` preset.
        variable_assignment: false,
    };
}

impl IdentifierSyntax {
    /// `LENIENT`: accept `$` as an identifier-continue byte (PostgreSQL/MySQL). A *leading*
    /// `$`+digit still dispatches to the parameter form — `$` only continues an
    /// identifier, it does not start one — so this does not conflict with rule 3.
    pub const LENIENT: Self = Self {
        non_ascii: super::NonAsciiIdentifierSyntax::Any,
        dollar_in_identifiers: true,
        // The permissive union admits SQLite's string-literal identifier in the two
        // corpus-admitted name positions (relation target, `PRIMARY KEY`/`UNIQUE` column
        // list). Each is position-driven and unambiguous — a bare string is never a valid
        // literal there — so the union shadows no rival reading (unlike `indexed_by`, whose
        // contextual `INDEXED` keyword would collide with a table named `indexed`, kept off
        // here). Matches the `empty_in_list` / `bare_constraint_name` Lenient-on precedent.
        string_literal_identifiers: true,
        // Union widening: SQLite accepts an empty quoted identifier in every quote style, so
        // the permissive union does too (a pure accept-side addition).
        empty_quoted_identifiers: true,
    };
}

impl TableExpressionSyntax {
    /// The `LENIENT` preset for table expression syntax.
    pub const LENIENT: Self = Self {
        only: true,
        table_sample: true,
        parenthesized_joins: true,
        table_alias_column_lists: true,
        join_using_alias: true,
        // Accept MySQL's index hints and `PARTITION (…)` selection — additive forms.
        index_hints: true,
        // Accept MSSQL's `WITH (...)` table hints — additive form.
        table_hints: true,
        partition_selection: true,
        base_table_alias_column_lists: true,
        // Accept DuckDB's string-literal table alias (`FROM t AS 't'('k')`) — additive.
        string_literal_aliases: true,
        aliased_parenthesized_join: true,
        // The permissive union keeps the bare table alias on the wide `ColId` set (the
        // accepting side), so it never sacrifices the SQLite JOIN-keyword-as-alias reading.
        bare_table_alias_is_bare_label: false,
        // Accept the BigQuery/MSSQL/Databricks table version / time-travel modifiers —
        // additive form.
        table_version: true,
        // OFF, not additive: the Redshift/Snowflake PartiQL / SUPER table-position path is
        // entered on a `[` after a table name, but Lenient claims `[` for its bracket
        // identifier quote (like `subscript` / `collection_literals` above, kept off for the
        // same reason). Enabling it would trip the
        // `BracketIdentifierVersusArraySyntax` lexical conflict, so Lenient forgoes the path
        // to keep bracket identifiers.
        table_json_path: false,
        // OFF, not additive: turning on SQLite's `INDEXED BY` directive would decline a bare
        // `INDEXED` as a correlation alias at the base-table position (so `FROM t indexed`
        // rejects), and the directive reading versus the bare-alias reading are mutually
        // exclusive given the keyword's one-position semantics. Lenient's maximal-accept goal
        // prefers the more permissive bare-alias reading, so it forgoes the directive to keep
        // `indexed` an ordinary alias everywhere.
        indexed_by: false,
    };
}

impl JoinSyntax {
    /// The `LENIENT` preset for join syntax.
    pub const LENIENT: Self = Self {
        stacked_join_qualifiers: true,
        full_outer_join: true,
        // Accept SQLite's `NATURAL CROSS JOIN` — an additive form led by the reserved
        // `NATURAL` keyword, normalized into the canonical natural-inner shape.
        natural_cross_join: true,
        // `LENIENT` accepts every additive form, including MySQL's `STRAIGHT_JOIN`.
        straight_join: true,
        // Accept DuckDB's `ASOF`/`POSITIONAL` joins — additive keyword-led forms with
        // no tokenizer trigger. The words stay unreserved (conflict-resolution rule 5
        // keeps the ANSI reserved model), so on a bare table factor the alias reading
        // wins (`FROM l ASOF JOIN r …` reads `asof` as `l`'s alias, then a plain
        // join), exactly as before these flags; the join parses where no alias can
        // claim the word (after an explicit alias, `FROM l AS a ASOF JOIN r …`).
        // DuckDB itself reserves both words, so the bare-factor spelling needs the
        // DuckDb preset — the same reserved-model sacrifice documented for `QUALIFY`.
        asof_join: true,
        positional_join: true,
        semi_anti_join: true,
        // Accept Spark/Hive's sided `{LEFT|RIGHT} {SEMI|ANTI} JOIN` — an additive form
        // led by the reserved `LEFT`/`RIGHT` keyword, so no alias-model sacrifice; a
        // separate gate from `semi_anti_join` because DuckDb rejects the sided spelling.
        sided_semi_anti_join: true,
        // Accept MSSQL's `CROSS`/`OUTER APPLY` — an additive form led by the reserved
        // `CROSS`/`OUTER` keyword, so no alias-model sacrifice like the `ASOF` pair
        // above (the leading keyword already anchors the operator).
        apply_join: true,
        // Accept the SQL:2023 recursive-query SEARCH/CYCLE clauses (the accepting side).
        recursive_search_cycle: true,
        // Keep the recursive-CTE UNION modifier restriction off (the accepting side).
        recursive_union_rejects_order_limit: false,
        // Accept DuckDB's `USING KEY` recursive-CTE key clause (the accepting side); the
        // leading `USING` sits before `AS`, shadowing no other spelling.
        recursive_using_key: true,
    };
}

impl TableFactorSyntax {
    /// The `LENIENT` preset for table factor syntax.
    pub const LENIENT: Self = Self {
        lateral: true,
        table_functions: true,
        rows_from: true,
        // The permissive superset admits the first-class UNNEST factor; `WITH OFFSET`
        // stays preset-less (no oracle-backed dialect enables it).
        unnest: true,
        unnest_with_offset: false,
        table_function_ordinality: true,
        // Accept a special value function as a `FROM` source and an alias on a parenthesized
        // join — additive (the permissive union takes the accepting side).
        special_function_table_source: true,
        // Accept DuckDB's PIVOT/UNPIVOT operators — additive forms.
        pivot: true,
        unpivot: true,
        // Accept DuckDB's DESCRIBE/SHOW/SUMMARIZE table source — an additive form.
        show_ref: true,
        // Accept DuckDB's bare `FROM VALUES (…) AS t` row-list table factor — additive.
        from_values: true,
        // Accept the SQL/JSON JSON_TABLE and SQL/XML XMLTABLE table factors (accepting side).
        json_table: true,
        xml_table: true,
        // Accept `TABLE(<expr>)` (Snowflake/Oracle's factor, the accepting side): no
        // oracle-backed preset enables it, so it stays a Lenient-only permissive extra.
        table_expr_factor: true,
        // Accept the standard PIVOT's extended value sources (`ANY`, subquery) and
        // `DEFAULT ON NULL` — the accepting side of the permissive superset.
        pivot_value_sources: true,
        // Accept the SQL:2016 MATCH_RECOGNIZE table factor — the accepting side of the
        // permissive superset (also on for Snowflake).
        match_recognize: true,
        // Accept SQL Server's OPENJSON table factor — the accepting side of the permissive
        // superset (also on for MSSQL).
        open_json: true,
    };
}

impl ExpressionSyntax {
    /// `LENIENT`: every PostgreSQL postfix/constructor form *except* the `[`-punctuation
    /// forms.
    ///
    /// `subscript`, `array_constructor`, and `collection_literals` are OFF because `[`
    /// is a bracket identifier-quote opener here (conflict-resolution rule 2): the
    /// tokenizer claims `[` before the parser can read it as array punctuation.
    /// Everything that does not need `[` (typecast `::`, `COLLATE`, `AT TIME ZONE`,
    /// `ROW(...)`, field selection, typed literals) is ON.
    pub const LENIENT: Self = Self {
        typecast_operator: true,
        subscript: false,
        // `[` is a bracket identifier quote here, so subscripting is off and the three-bound
        // slice (a subscript extension) cannot apply either.
        slice_step: false,
        collate: true,
        at_time_zone: true,
        semi_structured_access: false,
        array_constructor: false,
        multidim_array_literals: false,
        collection_literals: false,
        row_constructor: true,
        // BigQuery's `STRUCT(...)` value constructor: additive over ANSI (the `STRUCT`
        // word only opens it before `(`/`<`, otherwise stays an ordinary call/name), so
        // the permissive union admits it.
        struct_constructor: true,
        field_selection: true,
        field_wildcard: true,
        typed_string_literals: true,
        // Lenient is maximally permissive: it keeps the ANSI prefix-typed interval literal
        // (`INTERVAL '1' HOUR TO SECOND`, the unit-less `INTERVAL '1'`) that MySQL lacks, so
        // the operator reader's declined ANSI spellings still parse via this literal path.
        typed_interval_literal: true,
        // Lenient accepts DuckDB's relaxed interval spellings too — a purely additive
        // superset over the standard quoted form.
        relaxed_interval_syntax: true,
        mysql_interval_operator: true,
        // DuckDB's `#n` positional column reference is OFF here: `#` is already claimed by
        // the MySQL-style line comment (`line_comment_hash` on), so the positional form
        // would be the `HashCommentVersusPositionalColumn` sacrifice — Lenient keeps `#`
        // a comment (conflict-resolution rule 8, the same call it makes for `#`-XOR).
        positional_column: false,
        lambda_keyword: true,
    };
}

impl OperatorSyntax {
    /// `LENIENT`: the explicit-operator construct, both SQLite equality spellings, MySQL
    /// `<=>`, the bitwise family, and quantified comparisons — all pure additions with no
    /// contended trigger. The PostgreSQL `@`-family operators (`<@`/`->`/`->>`) stay off:
    /// `LENIENT` enables the `@name` session-variable read
    /// ([`SessionVariableSyntax::LENIENT`]), which claims the same `@`+identifier trigger
    /// the prefix `@` absolute-value operator would, so a permissive superset gaining the
    /// `@`-family is a scoped follow-up.
    pub const LENIENT: Self = Self {
        operator_construct: true,
        containment_operators: false,
        json_arrow_operators: false,
        // OFF, not on principle but by conflict: the `?`-led members share the `?` trigger
        // with `anonymous_question` (on in this union), and the `@@`/`@?` members ride the
        // `@` family held off above — so the `jsonb` operators cannot join the parse-anything
        // union without shadowing the placeholder. Held off with the `@`-family, like `->`.
        jsonb_operators: false,
        // SQLite's `==` and general `IS` are pure additions — no shared trigger, and
        // each only widens what parses — so the parse-anything union admits both.
        double_equals: true,
        // DuckDB's `//` integer-division spelling — a pure addition (no shared trigger; no
        // preset lexes `//` as a comment), so the parse-anything union admits it.
        integer_divide_slash: true,
        starts_with_operator: false,
        is_general_equality: true,
        // The standard truth-value tests (F571); the parser checks them ahead of the
        // general-equality reading, so the superset admits `IS UNKNOWN` as the predicate
        // while `is_general_equality` still covers SQLite's `IS <expr>`.
        truth_value_tests: true,
        // Accept MySQL's `<=>` (no lexical conflict with `<=`/`<>`) in the superset.
        null_safe_equals: true,
        // OFF, not on principle but by dependency: the lambda gate re-reads a `->`
        // token that only `json_arrow_operators` lexes, and that flag is off here
        // (held with the `@`-family above), so enabling this would be dead data —
        // `a -> b` does not tokenize under `LENIENT` at all. It follows the arrows:
        // the same scoped follow-up that admits `->`/`->>` decides this one.
        lambda_expressions: false,
        // The shared bitwise `| & ~ << >>` family is a pure addition (no contended
        // trigger), so the parse-anything union admits it. Bitwise XOR's `^` spelling is on
        // via `caret_operator` on the preset below (conflict-resolution rule 8).
        bitwise_operators: true,
        quantified_comparisons: true,
        quantified_comparison_lists: true,
        // The permissive union carries PostgreSQL's any-operator quantifier.
        quantified_arbitrary_operator: true,
        // OFF, not on principle but by conflict: the general operator surface adds the bare
        // `@` operator, which shares the `@` trigger with the `@name`/`@@name` sigils this
        // union keeps (`named_at`/`system_variables` on) — the tracked
        // `LexicalConflict::CustomOperatorVersusAtName` / `CustomOperatorVersusSystemVariable`.
        // Held off with the rest of the `@`-family (containment/`jsonb`/`->`) for the same
        // reason: the union picks the sigils over the `@`-lead operators.
        custom_operators: false,
        null_test_postfix: true,
        // ON as a pure additive parser position: a trailing symbolic operator with no operand
        // conflicts with nothing, so the parse-anything union admits DuckDB's postfix reading.
        // Only the always-lexed `Op` tokens (`!`/`~`/the bitwise family) reach it here —
        // `custom_operators` is held off above for the `@`-sigil conflict, so the `Custom`
        // residue does not tokenize under this preset.
        postfix_operators: true,
    };
}

impl CallSyntax {
    /// The `LENIENT` preset for call syntax.
    pub const LENIENT: Self = Self {
        named_argument: true,
        utc_special_functions: true,
        columns_expression: true,
        extract_from_syntax: true,
        try_cast: true,
        // `LENIENT` accepts every cast target; it never narrows to MySQL's `cast_type`.
        restricted_cast_targets: false,
        // The DuckDB call tails — quoted `EXTRACT` field, dot-method chaining, and
        // in-parenthesis null-treatment — are pure additions with no contended trigger.
        extract_string_field: true,
        method_chaining: true,
        sqljson_constructors_require_argument: false,
        // Lenient admits the full SQL/JSON expression-function grammar; the empty
        // constructor floor stays off (the arity-floor flag above is false), so
        // `JSON()`/`JSON_SCALAR()`/`JSON_SERIALIZE()` fall back to ordinary niladic calls.
        sqljson_expression_functions: true,
        // Lenient admits the full SQL/XML expression-function grammar.
        xml_expression_functions: true,
        variadic_argument: true,
        // Lenient inherits PostgreSQL's `merge_action()` support function.
        merge_action_function: true,
        convert_function: true,
    };
}

impl StringFuncForms {
    /// The `LENIENT` preset for string func forms.
    pub const LENIENT: Self = Self {
        // Lenient admits every additive string special form — the full PostgreSQL
        // substring/position/overlay/trim surface plus MySQL's SUBSTR keyword head —
        // and none of the restrictions (no plain-call arity floor, no
        // PLACING-required overlay, symmetric b_expr POSITION operands, the loose
        // trim_list tails on).
        substring_from_for: true,
        substring_leading_for: true,
        substring_similar: true,
        substring_plain_call_requires_2_or_3_args: false,
        substr_from_for: true,
        position_in: true,
        position_asymmetric_operands: false,
        overlay_placing: true,
        overlay_requires_placing: false,
        trim_from: true,
        trim_list_syntax: true,
        // Lenient inherits PostgreSQL's `COLLATION FOR (<expr>)` common-subexpr.
        collation_for_expression: true,
        // Lenient accepts sqlparser-rs's `CEIL(x TO field)` parity surface — no probed
        // oracle engine's grammar admits it.
        ceil_to_field: true,
        // Lenient accepts sqlparser-rs's `FLOOR(x TO field)` parity surface — no probed
        // oracle engine's grammar admits it.
        floor_to_field: true,
        // Lenient is a permissive superset — accept MySQL's `MATCH (…) AGAINST (…)`.
        match_against: true,
    };
}

impl AggregateCallSyntax {
    /// The `LENIENT` preset for aggregate call syntax.
    pub const LENIENT: Self = Self {
        group_concat_separator: true,
        within_group: true,
        aggregate_filter: true,
        // Lenient is the permissive superset: it admits both the standard `FILTER (WHERE …)`
        // and DuckDB's keyword-less `FILTER (…)`.
        filter_optional_where: true,
        // `LENIENT` is maximally permissive: it never makes a space before `(` significant,
        // so both `COUNT ( * )` and the adjacent `COUNT(*)` parse.
        aggregate_args_require_adjacent_paren: false,
        null_treatment: true,
        // Lenient is maximally permissive: it admits empty aggregate calls and `OVER` on
        // any function, so neither MySQL-only restriction fires.
        aggregate_calls_reject_empty_arguments: false,
        over_requires_windowable_function: false,
        window_function_tail: false,
        standalone_argument_order_by: true,
    };
}

impl PredicateSyntax {
    /// The `LENIENT` preset for predicate syntax.
    pub const LENIENT: Self = Self {
        is_distinct_from: true,
        like: true,
        ilike: true,
        similar_to: true,
        // The permissive union carries the standard `OVERLAPS` period predicate.
        overlaps_period_predicate: true,
        // The permissive union accepts DuckDB's unparenthesized `IN <value>` too; it does
        // not contend with the standard `IN (list)` (the `(` lookahead splits them).
        unparenthesized_in_list: true,
        // The permissive union carries PostgreSQL's `LIKE/ILIKE ANY|ALL (array)`.
        pattern_match_quantifier: true,
        between_symmetric: true,
        is_normalized: true,
        // The permissive union carries SQLite's empty `IN ()` list.
        empty_in_list: true,
        // The permissive union carries the SQLite/DuckDB two-word `<expr> NOT NULL` postfix.
        null_test_two_word_postfix: true,
    };
}

impl MutationSyntax {
    /// The `LENIENT` preset for mutation syntax.
    pub const LENIENT: Self = Self {
        insert_ignore: true,
        insert_overwrite: true,
        returning: true,
        on_conflict: true,
        on_duplicate_key_update: true,
        multi_column_assignment: true,
        update_tuple_value_row_arity: false,
        where_current_of: true,
        merge: true,
        // The permissive union also accepts the MySQL `REPLACE` statement and the
        // `INSERT`/`REPLACE ... SET` source — both additive, distinct keyword triggers.
        replace_into: true,
        insert_set: true,
        // The MySQL single-table `UPDATE`/`DELETE ... ORDER BY ... LIMIT` tails are
        // additive (trailing clauses in a distinct position), so the union accepts them.
        update_delete_tails: true,
        joined_update_delete: true,
        // The SQLite `INSERT OR`/`UPDATE OR <action>` prefix is additive (a distinct `OR`
        // trigger after the verb), so the permissive union accepts it.
        or_conflict_action: true,
        insert_column_matching: true,
        delete_using: true,
        update_from: true,
        // Accept an alias on a `DELETE … USING` target and a leading `WITH` before
        // `INSERT` or `MERGE` — additive.
        delete_using_target_alias: true,
        cte_before_insert: true,
        cte_before_merge: true,
        // Data-modifying CTE bodies are additive (distinct DML keyword triggers inside
        // `AS (…)`), so the permissive union accepts them.
        data_modifying_ctes: true,
        // The MERGE residual grammar (`WHEN NOT MATCHED BY SOURCE/TARGET`, `INSERT
        // DEFAULT VALUES`, and the `OVERRIDING` merge insert override) is additive, so
        // the permissive union accepts all three.
        merge_when_not_matched_by: true,
        merge_insert_default_values: true,
        merge_insert_overriding: true,
        merge_insert_multirow: true,
        merge_update_set_star: true,
        merge_insert_star_by_name: true,
        merge_error_action: true,
        update_set_qualified_column: true,
    };
}

impl StatementDdlGates {
    /// The `LENIENT` preset for statement ddl gates.
    pub const LENIENT: Self = Self {
        colocation_groups: true,
        materialized_view_to: true,
        // The parse-anything union accepts the SQLite `CREATE TRIGGER` body form too.
        create_trigger: true,
        // …and the DuckDB `CREATE MACRO`/live-body `FUNCTION` macro DDL.
        create_macro: true,
        create_secret: true,
        // …and DuckDB's `CREATE`/`DROP TYPE` user-defined-type DDL.
        create_type: true,
        // …and SQLite's `CREATE VIRTUAL TABLE … USING <module>(<args>)`.
        create_virtual_table: true,
        // …and the PostgreSQL/DuckDB `CREATE`/`DROP SEQUENCE` T176 generator.
        create_sequence: true,
        create_sequence_cache: true,
        extension_ddl: true,
        transform_ddl: true,
        alter_system: true,
        // MySQL's tablespace / logfile-group storage DDL — a pure addition in the union: the
        // leading `TABLESPACE`/`LOGFILE`/`UNDO` keywords collide with no other statement.
        tablespace_ddl: true,
        logfile_group_ddl: true,
        schemas: true,
        // Lenient is the union of every real dialect's surface, so PostgreSQL's
        // embedded schema-element form is on here too (no-shadowing doctrine).
        schema_elements: true,
        databases: true,
        // Conflict resolution: `drop_database` would recast `DROP SCHEMA` as MySQL's
        // single-name synonym drop and forfeit the more permissive PostgreSQL/DuckDB
        // name-list-plus-`CASCADE` `DROP SCHEMA`. LENIENT keeps the name-list path and
        // forgoes the MySQL `DROP DATABASE` spelling.
        drop_database: false,
        materialized_views: true,
        temporary_views: true,
        routines: true,
        or_replace: true,
        // …and DuckDB's `CREATE [OR REPLACE] [TEMP] RECURSIVE VIEW` (no-shadowing
        // doctrine: the union carries every real dialect's surface).
        recursive_views: true,
        // The permissive union carries MySQL's compound-statement body grammar.
        compound_statements: true,
        alter_database: true,
        alter_database_options: true,
        server_definition: true,
        alter_instance: true,
        spatial_reference_system: true,
        resource_group: true,
        alter_sequence: true,
        alter_object_set_schema: true,
        view_definition_options: true,
    };
}

impl CreateTableClauseSyntax {
    /// The `LENIENT` preset for create table clause syntax.
    pub const LENIENT: Self = Self {
        table_options: true,
        // The parse-anything union accepts the SQLite trailing `WITHOUT ROWID` table
        // option — additive over the shared surface.
        without_rowid_table_option: true,
        // The parse-anything union accepts the SQLite trailing `STRICT` table option —
        // additive over the shared surface.
        strict_table_option: true,
        // …and DuckDB's `CREATE OR REPLACE TABLE` / `CREATE [PERSISTENT] SECRET`.
        create_or_replace_table: true,
        storage_parameters: true,
        on_commit: true,
        create_table_as_with_data: true,
        create_table_as_execute: true,
        // Lenient is the permissive superset — accept the PostgreSQL partitioning grammar.
        declarative_partitioning: true,
        // The permissive superset accepts the legacy inheritance clause and the LIKE element.
        table_inheritance: true,
        like_source_table: true,
        // …and MySQL's statement-level table-clone body (the bare `LIKE src` form; the
        // parenthesized `(LIKE …)` reads as the PostgreSQL element superset when both are on).
        statement_level_table_like: true,
        unlogged_tables: true,
        table_access_method: true,
        without_oids: true,
        typed_tables: true,
    };
}

impl ColumnDefinitionSyntax {
    /// The `LENIENT` preset for column definition syntax.
    pub const LENIENT: Self = Self {
        // The union accepts the keywordless generated-column `AS (…)` shorthand and the
        // SQLite `CREATE TABLE` decoration cluster — all additive.
        generated_column_shorthand: true,
        // The parse-anything union accepts the SQLite column-level `ON CONFLICT
        // <resolution>` clause — additive over the shared surface.
        column_conflict_resolution_clause: true,
        // The parse-anything union accepts the SQLite typeless column definition too.
        typeless_column_definitions: true,
        // The parse-anything union accepts DuckDB's type-optional generated column as well
        // (subsumed by the wider typeless rule above, but flagged on for completeness).
        typeless_generated_columns: true,
        // The parse-anything union accepts the SQLite joined `AUTOINCREMENT` attribute —
        // additive over the shared surface.
        joined_autoincrement_attribute: true,
        // The parse-anything union accepts the SQLite inline-`PRIMARY KEY` `ASC`/`DESC`
        // ordering too — additive over the shared surface.
        inline_primary_key_ordering: true,
        // The parse-anything union accepts the SQLite `CONSTRAINT <name>` prefix on a column
        // `COLLATE` too — additive over the bare column COLLATE surface.
        named_column_collate_constraint: true,
        identity_columns: true,
        compact_identity_columns: true,
        // Accept a bare expression default and a `CONSTRAINT <name>` on any inline column
        // constraint — the permissive union never adds the MySQL restriction.
        default_expression_requires_parens: false,
        column_default_requires_b_expr: false,
        // Lenient is the permissive superset: every CREATE TABLE residue surface is on.
        column_collation: true,
        column_storage: true,
    };
}

impl ConstraintSyntax {
    /// The `LENIENT` preset for constraint syntax.
    pub const LENIENT: Self = Self {
        deferrable_constraints: true,
        named_inline_non_check_constraints: true,
        // Lenient is the permissive superset — accept SQLite's bodyless `CONSTRAINT <name>`.
        bare_constraint_name: true,
        exclusion_constraints: true,
        constraint_no_inherit_not_valid: true,
        index_constraint_parameters: true,
        constraint_column_collate_order: true,
        referential_action_cascade_set: true,
        check_constraint_subqueries: true,
    };
}

impl IndexAlterSyntax {
    /// The `LENIENT` preset for index alter syntax.
    pub const LENIENT: Self = Self {
        rename_constraint: true,
        alter_table_set_options: true,
        drop_primary_key: true,
        alter_column_add_identity: true,
        index_storage_parameters: true,
        drop_behavior: true,
        // Conflict resolution: `index_drop_on_table`'s mandatory-`ON` MySQL form would displace
        // the shared bare-name `DROP INDEX <name> [, …]`. LENIENT keeps the more permissive
        // name-list drop and forgoes the MySQL `DROP INDEX … ON <table>` form.
        index_drop_on_table: false,
        index_concurrently: true,
        index_using_method: true,
        partial_index: true,
        index_if_not_exists: true,
        index_nulls_order: true,
        alter_table_extended: true,
        alter_nested_column_paths: true,
        alter_existence_guards: true,
        alter_column_set_data_type: true,
        routine_arg_types: true,
        routine_arg_defaults: true,
        routine_arg_modes: true,
        // The permissive superset admits the PostgreSQL string-constant `LANGUAGE` spelling.
        routine_language_string: true,
        alter_table_multiple_actions: true,
    };
}

impl ExistenceGuards {
    /// The `LENIENT` preset for existence guards.
    pub const LENIENT: Self = Self {
        if_exists: true,
        view_if_not_exists: true,
        create_database_if_not_exists: true,
    };
}

impl SelectSyntax {
    /// The `LENIENT` preset for select syntax.
    pub const LENIENT: Self = Self {
        distinct_on: true,
        select_into: true,
        // Accept the empty target list (`SELECT`, `SELECT FROM t`) — a pure addition.
        empty_target_list: true,
        // Accept DuckDB's `QUALIFY` clause — a pure acceptance addition. `QUALIFY`
        // stays unreserved here (conflict-resolution rule 5 keeps the ANSI reserved
        // model), so in a position where a bare alias is legal (`SELECT 1 qualify`,
        // `FROM t QUALIFY …`) the alias reading wins, exactly as before this flag;
        // the clause parses where no alias can claim the word (after a GROUP
        // BY/HAVING/WINDOW clause or a non-aliasable expression). DuckDB itself
        // reserves the word, so its `FROM t QUALIFY …` spelling needs the DuckDb
        // preset, not `LENIENT` — the same reserved-model sacrifice rule 5 documents
        // for `OFFSET`.
        qualify: true,
        // Accept MySQL's string-literal column aliases in the permissive superset.
        alias_string_literals: true,
        // Union widening: accept SQLite's bare (`AS`-less) string alias (`SELECT 1 'x'`) too.
        bare_alias_string_literals: true,
        // Accept DuckDB's `UNION [ALL] BY NAME` name-matched set operation — a pure
        // acceptance addition. Conflict-free: `BY` after a set operator opens no other
        // grammar, so admitting it shadows no existing reading.
        union_by_name: true,
        wildcard_modifiers: true,
        wildcard_replace: true,
        intersect_all: true,
        except_all: true,
        // Pure-accept superset: admit the PostgreSQL/DuckDB qualified-wildcard alias too.
        qualified_wildcard_alias: true,
        // Accept DuckDB's FROM-first SELECT (`FROM t SELECT x`, bare `FROM t`) — a pure
        // acceptance addition, conflict-free because `FROM` is reserved in the ANSI model
        // rule 5 keeps, so a leading `FROM` can never be a bare column/alias.
        from_first: true,
        parenthesized_query_operands: true,
        // `LENIENT` is a pure-acceptance superset, so it does *not* enforce DuckDB's
        // parse-time equal-arity reject — a ragged VALUES constructor stays accepted here.
        values_rows_require_equal_arity: false,
        // A pure-acceptance superset admits the bare-parenthesized query-position VALUES
        // constructor every non-MySQL dialect spells.
        values_row_constructor: true,
        // LENIENT is a pure-acceptance superset, so the projection `AS` alias admits
        // reserved words (no reroute to the stricter bare-alias set).
        as_alias_rejects_reserved: false,
        // A pure-acceptance superset admits DuckDB's trailing-comma list tolerance.
        trailing_comma: true,
        // DuckDB's prefix colon alias (`SELECT j : 42`, `FROM b : a`): a conflict-free
        // pure-acceptance addition the lenient charter admits. `semi_structured_access` is
        // off here, so nothing else claims the `<ident> :` head — the one construct it
        // would collide with — and a `:` at a select-item / table-factor head was
        // otherwise a clean reject, so turning it on only widens acceptance.
        prefix_colon_alias: true,
        // Hive/Spark `LATERAL VIEW`: a conflict-free pure-acceptance addition the
        // lenient charter admits. It shares the `LATERAL` lead with the derived-table
        // factor (`table_factor_syntax.lateral`, also on here), but the two occupy
        // disjoint grammar positions (a table-factor head vs after the complete FROM
        // list) and split on the `VIEW` follow token, so each declines the other's
        // `LATERAL` and the union stays unambiguous — the property that makes enabling
        // both conflict-free.
        lateral_view_clause: true,
        // The Oracle-style `START WITH`/`CONNECT BY` hierarchical query clause: a
        // conflict-free pure-acceptance addition the lenient charter admits. It parses
        // only after `WHERE` (a position no other clause claims), and its `PRIOR` operator
        // is scoped to the `CONNECT BY` condition, so it shadows no existing reading —
        // `START WITH`/`CONNECT BY`/`PRIOR`/`NOCYCLE` stay ordinary identifiers everywhere
        // else, exactly as before this flag.
        connect_by_clause: true,
    };
}

impl QueryTailSyntax {
    /// The `LENIENT` preset for query tail syntax.
    pub const LENIENT: Self = Self {
        fetch_first: true,
        limit_offset_comma: true,
        // Accept the query-tail row-locking clauses (`FOR UPDATE`/`FOR SHARE`, MySQL's
        // `LOCK IN SHARE MODE`) — a pure acceptance addition.
        locking_clauses: true,
        // Accept PostgreSQL's `NO KEY UPDATE`/`KEY SHARE` strengths and stacked clauses —
        // pure acceptance additions over the shared locking core.
        key_lock_strengths: true,
        stacked_locking_clauses: true,
        using_sample: true,
        leading_offset: true,
        limit_expressions: true,
        // A pure-acceptance superset: DuckDB's percentage `LIMIT` (`LIMIT 40 PERCENT` /
        // `LIMIT 35%`) is admitted alongside every other dialect's `LIMIT` surface.
        limit_percent: true,
        with_ties_requires_order_by: false,
        // BigQuery/ZetaSQL `|>` pipe syntax stays OFF here *for now*, the one deliberate
        // exception to "admit every conflict-free pure-acceptance form". The `|>` munch is
        // feature-gated so it shadows nothing (conflict-free), and the charter would
        // otherwise admit it — but the framework ships only the reference `WHERE` operator,
        // so enabling it now would make the "parse anything" preset accept `|> WHERE` while
        // rejecting every other pipe operator: a fragment a reader of this module could not
        // predict, breaking the honesty bar. Flip it on as a pure-acceptance addition once
        // the `planner-parity-pipe-*` operator surface is coherent.
        pipe_syntax: false,
        // ClickHouse `LIMIT n [OFFSET m] BY …` per-group limiting: a conflict-free
        // pure-acceptance addition the lenient charter admits (a plain `LIMIT n` still
        // parses unchanged; only a trailing `BY` diverts to the LIMIT BY shape).
        limit_by_clause: true,
        // ClickHouse `SETTINGS name = value, …` query tail: a conflict-free
        // pure-acceptance addition the lenient charter admits (`SETTINGS` is contextual,
        // so it only diverts at the query tail with the gate on).
        settings_clause: true,
        // ClickHouse `FORMAT <name>` query tail: a conflict-free pure-acceptance addition
        // the lenient charter admits (`FORMAT` is contextual, so it only diverts at the
        // query tail with the gate on).
        format_clause: true,
        // MSSQL `FOR XML`/`FOR JSON` result-shaping tail: a conflict-free pure-acceptance
        // addition the lenient charter admits. It shares the `FOR` lead with the locking
        // clauses (also on here), but the two partition on the follow token
        // (`XML`/`JSON` vs `UPDATE`/`SHARE`/`NO`/`KEY`), so each declines the other's
        // `FOR` and the union stays unambiguous — the property that makes enabling both
        // conflict-free.
        for_xml_json_clause: true,
    };
}

impl GroupingSyntax {
    /// The `LENIENT` preset for grouping syntax.
    pub const LENIENT: Self = Self {
        grouping_sets: true,
        with_rollup: true,
        // Accept PostgreSQL's operator-driven `USING` sort form (a pure addition).
        order_by_using: true,
        // Accept DuckDB's `GROUP BY ALL` / `ORDER BY ALL` clause modes — pure
        // acceptance additions, conflict-free because `ALL` is reserved in the ANSI
        // model rule 5 keeps (no dialect reads a bare `all` there as an identifier).
        group_by_all: true,
        // PostgreSQL's grouping-set quantifier stays disambiguated from the DuckDB mode
        // above by lookahead: bare `GROUP BY ALL` is the mode, `GROUP BY ALL <items>` is
        // the quantifier (an item list follows). Conflict-free superset (see the flag doc).
        group_by_set_quantifier: true,
        order_by_all: true,
    };
}

impl UtilitySyntax {
    /// The `LENIENT` preset for utility syntax.
    pub const LENIENT: Self = Self {
        start_transaction: true,
        start_transaction_block_optional: true,
        transaction_work_keyword: true,
        begin_transaction_keyword: true,
        commit_transaction_keyword: true,
        rollback_transaction_keyword: true,
        transaction_name: true,
        begin_transaction_modes: true,
        transaction_savepoints: true,
        set_transaction: true,
        transaction_isolation_mode: true,
        transaction_access_mode: true,
        transaction_deferrable_mode: true,
        start_transaction_isolation_mode: true,
        start_transaction_deferrable_mode: true,
        start_transaction_consistent_snapshot: true,
        transaction_multiple_modes: true,
        transaction_mode_comma_required: false,
        transaction_modes_unique: false,
        abort_transaction_alias: true,
        end_transaction_alias: true,
        transaction_release: true,
        transaction_chain: true,
        release_savepoint_keyword_optional: true,
        copy: true,
        // Snowflake's `COPY INTO` load/unload — a pure addition on top of the PostgreSQL
        // `COPY`, dispatched by the `INTO` after `COPY`, in keeping with the permissive
        // parse-anything union.
        copy_into: true,
        stage_references: false,
        comment_on: true,
        comment_if_exists: true,
        pragma: true,
        attach: true,
        kill: true,
        // MySQL's `HANDLER` cursor family — a pure addition in the union: the leading
        // `HANDLER` keyword collides with no other statement, so the superset accepts it.
        handler_statements: true,
        // MySQL's `INSTALL`/`UNINSTALL` `PLUGIN`/`COMPONENT` family — a pure addition in the
        // union: the leading `INSTALL`/`UNINSTALL` keywords collide with no other statement.
        plugin_component_statements: true,
        // MySQL's server-administration families — pure additions in the union. The leading
        // SHUTDOWN/RESTART/CLONE/HELP/BINLOG keywords collide with nothing; IMPORT TABLE and
        // DuckDB's IMPORT DATABASE (export_import_database) share the `IMPORT` keyword but split
        // on the second keyword (TABLE vs DATABASE), so both can be on without colliding.
        shutdown: true,
        restart: true,
        clone: true,
        import_table: true,
        help_statement: true,
        binlog: true,
        // MySQL's `CACHE INDEX` / `LOAD INDEX INTO CACHE` key-cache pair — a pure addition:
        // leading `CACHE` collides with nothing, and the `LOAD INDEX` lookahead keeps it MECE
        // against the DuckDB/PostgreSQL `LOAD <extension>` statement the union also admits.
        key_cache_statements: true,
        // The `USE` catalog-switch statement — a pure addition (its leading `USE`
        // contends with nothing at statement position), in keeping with the union.
        use_statement: true,
        // Admit the DuckDB dotted `USE catalog.schema` name (the superset direction: it
        // accepts a strict superset of MySQL's single-ident form).
        use_qualified_name: true,
        // Admit DuckDB's string-literal `USE` target (`USE 'n'` / `E'n'` / `$$n$$`) — a pure
        // addition over MySQL's identifier-only form.
        use_string_literal_name: true,
        // The DuckDB prepared-statement lifecycle and `CALL` — pure additions (their
        // leading keywords `PREPARE`/`EXECUTE`/`DEALLOCATE`/`CALL` contend with nothing),
        // in keeping with the parse-anything union.
        prepared_statements: true,
        // The PostgreSQL `PREPARE name(<type>, …)` typed parameter-type list — a pure
        // addition on top of `prepared_statements` (a widening of the name position,
        // contending with nothing), in keeping with the union.
        prepare_typed_parameters: true,
        call: true,
        // MySQL's bare `CALL name` form — a pure addition (a `CALL name` with no `(` contends
        // with nothing), in keeping with the parse-anything union.
        call_bare_name: true,
        load_extension: true,
        load_bare_name: true,
        load_data: true,
        reset_scope: true,
        detach_if_exists: true,
        // PostgreSQL's `DO` anonymous code block — a pure addition, in keeping with the
        // permissive union.
        do_statement: true,
        // MySQL's `DO <expr-list>` is a DIFFERENT behaviour on the same `DO` keyword, not a
        // pure addition: it collides with the PostgreSQL code block on inputs like `DO 'x'`
        // and cannot express the `DO LANGUAGE <lang>` clause. The union resolves the one
        // keyword to the richer PostgreSQL code-block reading (`do_statement` above), so this
        // stays off — the sole non-additive `DO` choice, called out here deliberately.
        do_expression_list: false,
        // MySQL's `PREPARE ... FROM` / `EXECUTE ... USING` / `{DEALLOCATE|DROP} PREPARE` is a
        // DIFFERENT grammar on the same three keywords as DuckDB's typed-`AS`
        // `prepared_statements` (on above), not a pure addition: `PREPARE p FROM 'x'` collides
        // with `PREPARE p AS <stmt>`, and the `USING @var` / bare-`FROM` surfaces have no
        // positional-argument spelling. The union resolves the keywords to the richer DuckDB
        // reading, so this stays off — a non-additive choice mirroring `do_expression_list`.
        prepared_statements_from: false,
        // MySQL's `LOCK/UNLOCK {TABLES|TABLE}` per-table locking — a pure addition *today*
        // (no other shipped preset dispatches a leading `LOCK`/`UNLOCK`), in keeping with
        // the union. When the PostgreSQL statement-level mode-list reading of `LOCK` lands,
        // the union owes the same one-reading decision `DO` got above; that gate does not
        // exist yet, so nothing is being resolved away here.
        lock_tables: true,
        // MySQL's `LOCK INSTANCE FOR BACKUP`/`UNLOCK INSTANCE` backup-lock pair — a pure
        // addition (collision-free even against the future PostgreSQL `LOCK` reading, which
        // never continues `LOCK instance` with `FOR`).
        lock_instance: true,
        // SQLite's `BEGIN {DEFERRED|IMMEDIATE|EXCLUSIVE}` transaction-mode modifier — a
        // pure addition, in keeping with the union.
        begin_transaction_mode: true,
        // MySQL's `XA` distributed-transaction family — a pure addition: `XA` is a unique
        // leading keyword no other dialect claims, so the union simply admits it.
        xa_transactions: true,
        // MySQL's standalone `RENAME TABLE`/`RENAME USER` statements — a pure addition.
        rename_statement: true,
        signal_diagnostics: true,
        // DuckDB's `EXPORT`/`IMPORT DATABASE` pair — a pure addition, in keeping with the
        // permissive union.
        export_import_database: true,
        // DuckDB's `UPDATE EXTENSIONS` refresh statement — a pure addition. The `EXTENSIONS`
        // lookahead only claims `UPDATE EXTENSIONS [(names)]` at statement end / before the
        // list, so the DML `UPDATE` union surface is untouched.
        update_extensions: true,
        // MySQL's `FLUSH` / `PURGE BINARY LOGS` server-administration statements — armed in
        // the permissive superset.
        flush: true,
        purge_binary_logs: true,
        replication_statements: true,
    };
}

impl ShowSyntax {
    /// The `LENIENT` preset for show syntax.
    pub const LENIENT: Self = Self {
        describe: true,
        describe_summarize: true,
        session_statements: true,
        set_value_reserved_words: KeywordSet::EMPTY,
        set_value_on_keyword: true,
        set_value_null_keyword: true,
        show_tables: true,
        show_columns: true,
        show_create_table: true,
        show_functions: true,
        show_routine_status: true,
        show_verbose: true,
        show_admin: true,
    };
}

impl MaintenanceSyntax {
    /// The `LENIENT` preset for maintenance syntax.
    pub const LENIENT: Self = Self {
        vacuum: true,
        // DuckDB's `VACUUM [ANALYZE] <table> (<cols>)` grammar. The parser accepts the
        // exact union of the two grammars, never a cross-dialect hybrid (the SQLite `INTO`
        // tail is admitted only on a SQLite-shaped prefix — engine-measured, both engines
        // reject every hybrid such as `VACUUM ANALYZE t (a) INTO 'f'`). Still NOT a pure
        // addition: the shared bare `VACUUM <name>` operand takes the DuckDB reading (the
        // *qualified* table) when both gates are on — a one-reading precedence, recorded as
        // the `DispatchOrderUnion` `VACUUM` row of
        // [`MULTI_CLAIMANT_STATEMENT_HEADS`](super::MULTI_CLAIMANT_STATEMENT_HEADS).
        vacuum_analyze: true,
        reindex: true,
        analyze: true,
        // DuckDB's `ANALYZE <table> (<cols>)` column list — a pure addition to the union.
        analyze_columns: true,
        // The PostgreSQL/DuckDB `CHECKPOINT`/`LOAD` statements and DuckDB's
        // `[FORCE] CHECKPOINT [db]` / bare-name `LOAD` / `RESET`-scope /
        // `DETACH … IF EXISTS` extensions — pure additions, in keeping with the union.
        checkpoint: true,
        checkpoint_database: true,
        // The MySQL admin-table verb family — a pure addition, in keeping with the union.
        table_maintenance: true,
    };
}

impl AccessControlSyntax {
    /// The `LENIENT` preset for access control syntax.
    pub const LENIENT: Self = Self {
        alter_role_rename: true,
        access_control: true,
        // A pure-acceptance superset admits the schema-scoped grant objects and the
        // `{GRANT|ADMIN} OPTION FOR` prefix.
        access_control_extended_objects: true,
        // The permissive superset admits the MySQL account-management DDL family.
        user_role_management: true,
        // Off deliberately: the MySQL account-based grant grammar is a *route* that structurally
        // conflicts with (does not extend) the PostgreSQL-extended grant grammar above, so the
        // superset cannot enable both — the registered
        // `GrammarConflict::AccountGrantsVersusExtendedObjects`. Lenient keeps the richer
        // PostgreSQL forms (schema objects,
        // `GRANTED BY`, `CASCADE`, routine signatures) that a MySQL route would forfeit.
        access_control_account_grants: false,
    };
}

impl TypeNameSyntax {
    /// The `LENIENT` preset for type name syntax.
    pub const LENIENT: Self = Self {
        extended_scalar_type_names: true,
        enum_type: true,
        set_type: true,
        numeric_modifiers: true,
        integer_display_width: true,
        composite_types: true,
        // Accept a length-less `VARCHAR` and the zoned temporal types — the permissive
        // union never adds the MySQL length requirement or zoned-type restriction.
        varchar_requires_length: false,
        zoned_temporal_types: true,
        // The permissive superset admits MySQL's `CHARACTER SET`/`ASCII`/`UNICODE`/`BYTE`/
        // `BINARY` char-type annotation and DuckDB's empty `DECIMAL()`/`DEC()`/`NUMERIC()`.
        empty_type_parens: true,
        character_set_annotation: true,
        // The permissive superset admits PostgreSQL's signed `numeric` modifier.
        signed_type_modifier: true,
        // The ClickHouse preset and the permissive union both carry ClickHouse's `Nullable(T)`
        // combinator (no differential oracle), the `composite_types` / `format_clause` precedent.
        nullable_type: true,
        // Same for the sibling `LowCardinality(T)` combinator — ClickHouse/Lenient, no oracle.
        low_cardinality_type: true,
        // Same for `FixedString(N)`, the fixed-length byte-string constructor — ClickHouse/Lenient,
        // no oracle.
        fixed_string_type: true,
        // Same for `DateTime64(P[, 'tz'])`, the sub-second timestamp constructor —
        // ClickHouse/Lenient, no oracle.
        datetime64_type: true,
        // Same for `Nested(name Type, ...)`, the named-field repeated-group composite —
        // ClickHouse/Lenient, no oracle.
        nested_type: true,
        // Same for the `Int8`…`Int256`/`UInt*` fixed-bit-width integer names — ClickHouse/Lenient,
        // no oracle.
        bit_width_integer_names: true,
        // The permissive superset admits SQLite's liberal multi-word / two-argument affinity
        // type names (`LONG INTEGER`, `VARCHAR(123,456)`).
        liberal_type_names: true,
        string_type_modifiers: false,
        angle_bracket_types: true,
    };
}

impl FeatureSet {
    /// The optional permissive **tooling** union — see the module-level docs for the
    /// full inclusion list and every conflict-resolution rule.
    ///
    /// This is the "parse anything" mode behind the `lenient` cargo feature, and the
    /// honest counterpart to the ban on a "generic" union: it is precise, not a
    /// vibe. It is a `const` like every other preset, so the `Lenient` dialect's
    /// `features()` hands back a `'static` borrow and the parser's field reads
    /// const-fold under `Parser<Lenient>` — zero per-parse cost, same code path as
    /// [`ANSI`](FeatureSet::ANSI).
    pub const LENIENT: Self = Self {
        // Preserve exact text; impose no dialect's fold (conflict-resolution rule 6).
        identifier_casing: Casing::Preserve,
        // The multi-style union — the one real cost of "parse anything".
        identifier_quotes: LENIENT_IDENTIFIER_QUOTES,
        // ANSI/PostgreSQL default; semantics only (conflict-resolution rule 7).
        default_null_ordering: NullOrdering::NullsLast,
        // ANSI position-aware reserved model (conflict-resolution rule 5): keep the
        // grammar-disambiguating reserved words; everything marginal is freed or quotable.
        reserved_column_name: RESERVED_COLUMN_NAME,
        reserved_function_name: RESERVED_FUNCTION_NAME,
        reserved_type_name: RESERVED_TYPE_NAME,
        reserved_bare_alias: RESERVED_BARE_ALIAS,
        // `LENIENT` accepts every additive form: all keywords as ColLabels, and the
        // three-part catalog-qualified relation name.
        reserved_as_label: KeywordSet::EMPTY,
        catalog_qualified_names: true,
        // Backtick/bracket quoting, `#`, `&&`, and `$` all dispatch from the standard
        // byte classes gated by the knobs below — no bespoke byte-class table is needed,
        // and keeping `#` out of the identifier-start class keeps rule (`#`) consistent.
        byte_classes: STANDARD_BYTE_CLASSES,
        // The permissive union follows PostgreSQL: range/pattern/membership predicates bind
        // one tier above comparison, so `a = b BETWEEN c AND d` groups `a = (b BETWEEN c AND
        // d)` (see [`BindingPowerTable::range_predicate_override`]).
        binding_powers: BindingPowerTable {
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
                assoc: Assoc::NonAssoc,
            },
            range_predicate_override: Some(RANGE_PREDICATE_ABOVE_COMPARISON),
            // The `IS`-family predicates rank one tier below comparison (PostgreSQL/DuckDB
            // `%nonassoc IS`), so `a <> b IS NULL` groups `(a <> b) IS NULL`. The comparison-tier
            // bare `IS`/`<=>` null-safe (in)equality (SQLite/MySQL) is spelling-distinguished
            // and unaffected.
            is_predicate_override: Some(IS_PREDICATE_BELOW_COMPARISON),
            double_equals: BindingPower {
                left: 40,
                right: 41,
                assoc: Assoc::NonAssoc,
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
            prefix_bitwise_not: 46,
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
        },
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        string_literals: StringLiteralSyntax::LENIENT,
        numeric_literals: NumericLiteralSyntax::LENIENT,
        parameters: ParameterSyntax::LENIENT,
        session_variables: SessionVariableSyntax::LENIENT,
        identifier_syntax: IdentifierSyntax::LENIENT,
        table_expressions: TableExpressionSyntax::LENIENT,
        join_syntax: JoinSyntax::LENIENT,
        table_factor_syntax: TableFactorSyntax::LENIENT,
        expression_syntax: ExpressionSyntax::LENIENT,
        operator_syntax: OperatorSyntax::LENIENT,
        call_syntax: CallSyntax::LENIENT,
        string_func_forms: StringFuncForms::LENIENT,
        aggregate_call_syntax: AggregateCallSyntax::LENIENT,
        predicate_syntax: PredicateSyntax::LENIENT,
        // `||` is concatenation (conflict-resolution rule 4).
        pipe_operator: PipeOperator::StringConcat,
        // `&&` as AND — a pure addition over ANSI's "unsupported".
        double_ampersand: DoubleAmpersand::LogicalAnd,
        // Recognize MySQL's `DIV`/`MOD`/`XOR`/`RLIKE`/`REGEXP` infix operators — additive
        // in operator position; they remain free identifiers elsewhere under rule 5.
        keyword_operators: KeywordOperators::MySql,
        // Bitwise XOR is `^` here (conflict-resolution rule 8): `^` is claimed as the
        // MySQL-style XOR operator (so `^` is XOR, not exponentiation), while `#` stays a line
        // comment (`line_comment_hash` on in `CommentSyntax::LENIENT`), so the PostgreSQL `#`
        // spelling is the sacrifice — the two claim the same `#` trigger
        // (`LexicalConflict::HashXorOperatorVersusHashComment`). Precedence follows STANDARD
        // (looser than additive), not MySQL's tight `^` rank; a parse-anything union documents
        // such precedence sacrifices, and both spellings still parse under the dialect that
        // owns each.
        caret_operator: CaretOperator::BitwiseXor,
        // `#` stays a line comment (above), so it is not the XOR operator here.
        hash_bitwise_xor: false,
        comment_syntax: CommentSyntax::LENIENT,
        mutation_syntax: MutationSyntax::LENIENT,
        statement_ddl_gates: StatementDdlGates::LENIENT,
        create_table_clause_syntax: CreateTableClauseSyntax::LENIENT,
        column_definition_syntax: ColumnDefinitionSyntax::LENIENT,
        constraint_syntax: ConstraintSyntax::LENIENT,
        index_alter_syntax: IndexAlterSyntax::LENIENT,
        existence_guards: ExistenceGuards::LENIENT,
        select_syntax: SelectSyntax::LENIENT,
        query_tail_syntax: QueryTailSyntax::LENIENT,
        grouping_syntax: GroupingSyntax::LENIENT,
        utility_syntax: UtilitySyntax::LENIENT,
        show_syntax: ShowSyntax::LENIENT,
        maintenance_syntax: MaintenanceSyntax::LENIENT,
        access_control_syntax: AccessControlSyntax::LENIENT,
        type_name_syntax: TypeNameSyntax::LENIENT,
        // Render the portable ANSI canonical spellings: `LENIENT` is a parse-anything
        // tooling union, not a dialect identity, so it has no PostgreSQL-specific
        // output spelling to emit.
        target_spelling: TargetSpelling::Ansi,
    };
}

/// The permissive tooling union; prefer [`FeatureSet::LENIENT`] for struct update.
pub const LENIENT: FeatureSet = FeatureSet::LENIENT;

// Compile-time proof that the union resolves every contested tokenizer trigger to a
// single claimant: if a future edit reintroduces a conflict (say, turns
// `double_quoted_strings` back on while `"` still quotes identifiers), the build fails
// here rather than silently mis-lexing.
const _: () = assert!(FeatureSet::LENIENT.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this permissive union: no
// refinement flag rides an unset base, and every multi-claimant head it unions resolves by
// a documented lookahead/dispatch split rather than an unresolved grammar conflict.
const _: () = assert!(FeatureSet::LENIENT.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::LENIENT.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::super::{
        Casing, CommentSyntax, ExpressionSyntax, FeatureDelta, FeatureSet, IdentifierQuote,
        LexicalConflict, NumericLiteralSyntax, OperatorSyntax, ParameterSyntax, PipeOperator,
        SessionVariableSyntax, StringLiteralSyntax, TableExpressionSyntax,
    };

    #[test]
    fn lenient_resolves_every_contested_trigger_to_one_claimant() {
        // The executable form of the documented conflict-resolution rules: each shared
        // trigger has exactly one claimant, so the union is self-consistent.
        assert_eq!(FeatureSet::LENIENT.lexical_conflict(), None);
        assert!(FeatureSet::LENIENT.is_lexically_consistent());
    }

    #[test]
    fn lenient_quotes_three_identifier_styles() {
        // The multi-quote union: `"`, backtick, and `[`-bracket all open identifiers.
        let opens: Vec<char> = FeatureSet::LENIENT
            .identifier_quotes
            .iter()
            .map(|quote| quote.open())
            .collect();
        assert_eq!(opens, ['"', '`', '[']);
        // The bracket style is the only asymmetric one (open `[`, close `]`).
        assert_eq!(
            FeatureSet::LENIENT.identifier_quotes[2],
            IdentifierQuote::Asymmetric {
                open: '[',
                close: ']'
            },
        );
    }

    #[test]
    fn conflict_resolution_fields_match_the_documented_rules() {
        let lenient = FeatureSet::LENIENT;
        // Rule 1: `"` is an identifier, so it is not a string.
        assert!(!lenient.string_literals.double_quoted_strings);
        // Rule 2: `[` is an identifier, so the `[`-punctuation forms are off.
        assert!(!lenient.expression_syntax.subscript);
        assert!(!lenient.expression_syntax.array_constructor);
        // Rule 3: `$`+digit is a parameter, not money.
        assert!(!lenient.numeric_literals.money_literals);
        assert!(lenient.parameters.positional_dollar);
        // Rule 4: `||` is concatenation.
        assert_eq!(lenient.pipe_operator, PipeOperator::StringConcat);
        // Rule 6: identity preserves exact text.
        assert_eq!(lenient.identifier_casing, Casing::Preserve);
    }

    #[test]
    fn lenient_enables_copy_utility_statement() {
        // COPY is an additive utility statement (no contested trigger), so the permissive
        // union turns it on — the ANSI baseline gates it off. Bind to locals so the const
        // field reads are not flagged by clippy's `assertions_on_constants`.
        let lenient = FeatureSet::LENIENT.utility_syntax;
        let ansi = FeatureSet::ANSI.utility_syntax;
        assert!(lenient.copy);
        assert!(!ansi.copy);
        assert_ne!(lenient, ansi);
    }

    #[test]
    fn lexical_conflict_detects_each_contested_trigger() {
        // Rule 1 violated: `"` claimed by both a string and the identifier quote.
        let double_quote =
            FeatureSet::LENIENT.with(FeatureDelta::EMPTY.string_literals(StringLiteralSyntax {
                double_quoted_strings: true,
                ..StringLiteralSyntax::LENIENT
            }));
        assert_eq!(
            double_quote.lexical_conflict(),
            Some(LexicalConflict::DoubleQuoteStringVersusIdentifier),
        );

        // Rule 2 violated: `[` claimed by both the identifier quote and subscript syntax.
        let bracket =
            FeatureSet::LENIENT.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
                subscript: true,
                ..ExpressionSyntax::LENIENT
            }));
        assert_eq!(
            bracket.lexical_conflict(),
            Some(LexicalConflict::BracketIdentifierVersusArraySyntax),
        );

        // Rule 2's third claimant: the DuckDB `[…]` list literal contends for the same
        // `[` trigger, independently of subscript/array_constructor (both stay off).
        let bracket_list =
            FeatureSet::LENIENT.with(FeatureDelta::EMPTY.expression_syntax(ExpressionSyntax {
                collection_literals: true,
                ..ExpressionSyntax::LENIENT
            }));
        assert_eq!(
            bracket_list.lexical_conflict(),
            Some(LexicalConflict::BracketIdentifierVersusArraySyntax),
        );

        // Rule 2's fourth claimant: the Redshift/Snowflake table-position PartiQL path
        // (`FROM src[0].a`) also enters on `[`, so it contends with the bracket identifier
        // quote independently of the expression-position `[` grammars (all stay off).
        let bracket_table_path = FeatureSet::LENIENT.with(FeatureDelta::EMPTY.table_expressions(
            TableExpressionSyntax {
                table_json_path: true,
                ..TableExpressionSyntax::LENIENT
            },
        ));
        assert_eq!(
            bracket_table_path.lexical_conflict(),
            Some(LexicalConflict::BracketIdentifierVersusArraySyntax),
        );

        // Rule 3 violated: `$`+digit claimed by both money and a positional parameter.
        let money =
            FeatureSet::LENIENT.with(FeatureDelta::EMPTY.numeric_literals(NumericLiteralSyntax {
                money_literals: true,
                ..NumericLiteralSyntax::LENIENT
            }));
        assert_eq!(
            money.lexical_conflict(),
            Some(LexicalConflict::MoneyVersusPositionalDollar),
        );

        // The strict ANSI baseline plus only a positional parameter is *not* a conflict
        // (money stays off), confirming the check is the pair, not either flag alone.
        let pg_param = FeatureSet::ANSI.with(FeatureDelta::EMPTY.parameters(ParameterSyntax {
            positional_dollar: true,
            ..ParameterSyntax::ANSI
        }));
        assert_eq!(pg_param.lexical_conflict(), None);

        // `@name` claimed by both a named-at parameter and a user-variable read.
        // `LENIENT` keeps `named_at` on, so turning `user_variables` on contends.
        let at_name = FeatureSet::LENIENT.with(FeatureDelta::EMPTY.session_variables(
            SessionVariableSyntax {
                user_variables: true,
                ..SessionVariableSyntax::LENIENT
            },
        ));
        assert_eq!(
            at_name.lexical_conflict(),
            Some(LexicalConflict::AtNameParameterVersusUserVariable),
        );

        // `:name` claimed by both a colon parameter and the `a[x:y]` bare-identifier
        // slice bound. PostgreSQL has `subscript` on (and does not quote with `[`), so
        // turning `named_colon` on there contends — a pairing only a custom delta forms.
        let colon_slice =
            FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.parameters(ParameterSyntax {
                named_colon: true,
                ..ParameterSyntax::POSTGRES
            }));
        assert_eq!(
            colon_slice.lexical_conflict(),
            Some(LexicalConflict::ColonParameterVersusSliceBound),
        );

        // The `:`+identifier trigger's third claimant: a collection literal's
        // `key: value` separator before a bare-identifier value (`{a: b}`) contends
        // with the colon parameter on its own — `subscript` stays off here, so this
        // detects the collection grammar, not the slice bound.
        let colon_collection = FeatureSet::ANSI.with(
            FeatureDelta::EMPTY
                .parameters(ParameterSyntax {
                    named_colon: true,
                    ..ParameterSyntax::ANSI
                })
                .expression_syntax(ExpressionSyntax {
                    collection_literals: true,
                    ..ExpressionSyntax::ANSI
                }),
        );
        assert_eq!(
            colon_collection.lexical_conflict(),
            Some(LexicalConflict::ColonParameterVersusSliceBound),
        );

        // `<@` claimed by both PostgreSQL's containment operator and an abutting `@name`.
        // PostgreSQL has `containment_operators` on, so turning the `@name` parameter on
        // contends — the `a<@x` (meaning `a < @x`) reading is shadowed.
        let containment_at =
            FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.parameters(ParameterSyntax {
                named_at: true,
                ..ParameterSyntax::POSTGRES
            }));
        assert_eq!(
            containment_at.lexical_conflict(),
            Some(LexicalConflict::ContainmentOperatorVersusAtName),
        );

        // The bare `@` operator (the general operator surface) claimed by both
        // `custom_operators` and an abutting `@name` parameter. PostgreSQL has
        // `custom_operators` on; turning the `<@` containment off (so the earlier
        // `ContainmentOperatorVersusAtName` does not claim the `@` first) and `@name` on
        // leaves the bare-`@` operator contending with the sigil.
        let custom_at = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY
                .operator_syntax(OperatorSyntax {
                    containment_operators: false,
                    ..OperatorSyntax::POSTGRES
                })
                .parameters(ParameterSyntax {
                    named_at: true,
                    ..ParameterSyntax::POSTGRES
                }),
        );
        assert_eq!(
            custom_at.lexical_conflict(),
            Some(LexicalConflict::CustomOperatorVersusAtName),
        );

        // The `@@` operator (the general operator surface, with the `jsonb` family off so
        // `@@` is not that match operator) claimed by both `custom_operators` and MySQL's
        // `@@name` system variable. Turning the `jsonb` family off (so the earlier
        // `JsonbSearchOperatorVersusSystemVariable` does not claim `@@` first) and the system
        // variable on leaves the `@@` operator contending with the sigil.
        let custom_sysvar = FeatureSet::POSTGRES.with(
            FeatureDelta::EMPTY
                .operator_syntax(OperatorSyntax {
                    jsonb_operators: false,
                    ..OperatorSyntax::POSTGRES
                })
                .session_variables(SessionVariableSyntax {
                    system_variables: true,
                    ..FeatureSet::POSTGRES.session_variables
                }),
        );
        assert_eq!(
            custom_sysvar.lexical_conflict(),
            Some(LexicalConflict::CustomOperatorVersusSystemVariable),
        );

        // The disjoint `@@` system-variable form (LENIENT's default) never contends
        // with the `@name` parameter — confirming only the same-trigger pair conflicts.
        assert_eq!(FeatureSet::LENIENT.lexical_conflict(), None);

        // `#` claimed by both the PostgreSQL `#` XOR operator (`hash_bitwise_xor: true`) and a
        // `#` line comment: PostgreSQL sets `hash_bitwise_xor: true` with `line_comment_hash`
        // off, so turning the comment on contends — the comment shadows the operator in
        // `skip_trivia`.
        let hash_xor_and_comment =
            FeatureSet::POSTGRES.with(FeatureDelta::EMPTY.comment_syntax(CommentSyntax {
                line_comment_hash: true,
                ..CommentSyntax::ANSI
            }));
        assert_eq!(
            hash_xor_and_comment.lexical_conflict(),
            Some(LexicalConflict::HashXorOperatorVersusHashComment),
        );

        // The mirror: LENIENT keeps `#` a comment and spells XOR `^` (`CaretOperator::BitwiseXor`),
        // so its `#` trigger is uncontended — flipping `#` to the XOR operator is the conflict.
        let lenient_hash_xor = FeatureSet::LENIENT.with(FeatureDelta::EMPTY.hash_bitwise_xor(true));
        assert_eq!(
            lenient_hash_xor.lexical_conflict(),
            Some(LexicalConflict::HashXorOperatorVersusHashComment),
        );
    }
}
