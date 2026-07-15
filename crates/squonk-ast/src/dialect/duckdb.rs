// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The DuckDB dialect preset (PostgreSQL-derived).
//!
//! DuckDB is PostgreSQL-dialect-compatible by design, but every field is enumerated here.
//! Values shared with PostgreSQL are repeated deliberately so either preset changing forces
//! an explicit review of the other.
//!
//! Comments focus on syntax and oracle evidence: parse-time tightenings, keyword reservation
//! needed to disambiguate grammar, and measured precedence differences.

use super::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, ConstraintSyntax, CreateTableClauseSyntax,
    DUCKDB_BYTE_CLASSES, DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FeatureSet,
    GroupingSyntax, IdentifierSyntax, IndexAlterSyntax, JoinSyntax, Keyword, KeywordOperators,
    KeywordSet, MaintenanceSyntax, MutationSyntax, NullOrdering, NumericLiteralSyntax,
    OperatorSyntax, ParameterSyntax, PipeOperator, PredicateSyntax, QueryTailSyntax,
    RESERVED_BARE_ALIAS, RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME, RESERVED_SET_VALUE_WORDS,
    RESERVED_TYPE_NAME, STANDARD_IDENTIFIER_QUOTES, SelectSyntax, SessionVariableSyntax,
    ShowSyntax, StatementDdlGates, StringFuncForms, StringLiteralSyntax, TableExpressionSyntax,
    TableFactorSyntax, TargetSpelling, TypeNameSyntax, UtilitySyntax,
};
use crate::precedence::{
    Assoc, BindingPower, BindingPowerTable, IS_PREDICATE_BELOW_COMPARISON,
    STANDARD_SET_OPERATION_BINDING_POWERS,
};

/// `QUALIFY` (`duckdb_keywords()` class `reserved`, DuckDB 1.5.4), the fully-reserved
/// half of DuckDB's reservation delta over the shared PostgreSQL-derived model.
/// Unioned into all four per-position reject sets below because DuckDB's `reserved`
/// class rejects the word as a column/table name, function name, type name, *and*
/// bare alias (probed: `SELECT qualify FROM t`, `SELECT * FROM qualify`,
/// `SELECT qualify(1)`, `CAST(1 AS qualify)`, `SELECT 1 qualify`, and
/// `FROM t qualify` all syntax-error; `SELECT 1 AS qualify` labels). The same
/// hand-composition pattern the SQLite/MySQL presets use for their reservation deltas.
pub const DUCKDB_QUALIFY_RESERVATION: KeywordSet = KeywordSet::from_keywords(&[Keyword::Qualify]);

/// `PIVOT` and `UNPIVOT` (`duckdb_keywords()` class `reserved`, DuckDB 1.5.4), the
/// row/column rotation operators. Like `QUALIFY`'s `reserved` class (and unlike the
/// `type_function` join words), both are rejected in all four identifier positions —
/// column/table name, function name, type name, and bare alias — while `AS pivot` still
/// labels (probed: `SELECT pivot FROM t`, `SELECT * FROM pivot`, `SELECT pivot(1)`,
/// `CAST(1 AS pivot)`, `SELECT 1 pivot` all syntax-error; `SELECT 1 AS pivot` parses —
/// identically for `unpivot`). The bare-alias reservation is load-bearing for the
/// grammar: it is what lets `FROM t PIVOT (…)` read the operator instead of a table
/// alias named `pivot`. Unioned into all four reject sets below, exactly like
/// [`DUCKDB_QUALIFY_RESERVATION`].
pub const DUCKDB_PIVOT_RESERVATION: KeywordSet =
    KeywordSet::from_keywords(&[Keyword::Pivot, Keyword::Unpivot]);

/// The nonstandard-join keywords, `ASOF` and `POSITIONAL` (`duckdb_keywords()` class
/// `type_function`, like `CROSS`, DuckDB 1.5.4). Unlike `QUALIFY`'s `reserved` class,
/// this profile rejects the words only as a column/table name (`ColId`) and as a bare
/// alias, while function/type positions and `AS` labels still admit them (probed:
/// `SELECT asof FROM t`, `CREATE TABLE asof(…)`, `FROM t asof`, and `SELECT 1 asof`
/// all syntax-error; `SELECT asof(1)`, `CAST(1 AS asof)`, and `SELECT 1 AS asof`
/// parse — identically for `positional`). The `ColId` reservation is load-bearing for
/// the grammar: it is what lets `FROM l ASOF JOIN r …` read the join instead of a
/// table alias named `asof`.
pub const DUCKDB_NONSTANDARD_JOIN_RESERVATION: KeywordSet =
    KeywordSet::from_keywords(&[Keyword::Asof, Keyword::Positional]);

/// The semi-/anti-join keywords, `SEMI` and `ANTI` (`duckdb_keywords()` class
/// `type_function`, DuckDB 1.5.4). Their `type_function` category matches
/// `ASOF`/`POSITIONAL`, but the DuckDB grammar reserves them one position *further*:
/// they reject as a column/table name (`ColId`), a bare alias, *and a function name*,
/// while only the type position and `AS` labels admit them (probed: `SELECT semi FROM
/// t`, `CREATE TABLE semi(…)`, `FROM t semi`, `SELECT 1 semi`, and — unlike `asof(1)` —
/// `SELECT semi(1)` all syntax-error; `CAST(1 AS semi)` and `SELECT 1 AS semi` parse,
/// identically for `anti`). So this set unions into the `ColId`, function-name, and
/// bare-alias rejects but *not* the type-name one. The `ColId`/bare-alias reservation
/// is load-bearing for the grammar: it is what lets `FROM l SEMI JOIN r …` read the
/// join instead of a table alias named `semi`.
pub const DUCKDB_SEMI_ANTI_JOIN_RESERVATION: KeywordSet =
    KeywordSet::from_keywords(&[Keyword::Semi, Keyword::Anti]);

/// PostgreSQL-reserved words that DuckDB 1.5.4 classifies as `unreserved`.
/// Both words are valid as a column/table name, function name, type name, and
/// generic `SET` value. They remain rejected as bare projection aliases.
pub const DUCKDB_UNRESERVED_CARVEOUT: KeywordSet =
    KeywordSet::from_keywords(&[Keyword::Grant, Keyword::User]);

/// PostgreSQL special-value keywords that are ordinary identifiers in DuckDB's
/// keyword inventory. They remain available as built-in names, including call
/// spellings, but carry no identifier-position reservation.
pub const DUCKDB_ORDINARY_SPECIAL_VALUE_NAMES: KeywordSet = KeywordSet::from_keywords(&[
    Keyword::CurrentCatalog,
    Keyword::CurrentDate,
    Keyword::CurrentRole,
    Keyword::CurrentSchema,
    Keyword::CurrentTime,
    Keyword::CurrentTimestamp,
    Keyword::CurrentUser,
    Keyword::Localtime,
    Keyword::Localtimestamp,
    Keyword::SessionUser,
    Keyword::SystemUser,
]);

/// DuckDB's unreserved `GRANT` and `USER` words are not valid bare projection
/// aliases, although they are valid identifiers in the other positions above.
pub const DUCKDB_UNRESERVED_BARE_ALIAS_RESERVATION: KeywordSet =
    KeywordSet::from_keywords(&[Keyword::Grant, Keyword::User]);

/// DuckDB `ColId` reject set: the shared model plus `QUALIFY`, the `PIVOT`/`UNPIVOT`
/// operators, and the nonstandard-join / semi-anti-join keywords.
pub const DUCKDB_RESERVED_COLUMN_NAME: KeywordSet = RESERVED_COLUMN_NAME
    .difference(DUCKDB_UNRESERVED_CARVEOUT)
    .difference(DUCKDB_ORDINARY_SPECIAL_VALUE_NAMES)
    .union(DUCKDB_QUALIFY_RESERVATION)
    .union(DUCKDB_PIVOT_RESERVATION)
    .union(DUCKDB_NONSTANDARD_JOIN_RESERVATION)
    .union(DUCKDB_SEMI_ANTI_JOIN_RESERVATION);

/// DuckDB function-name reject set: the shared model plus `QUALIFY` (DuckDB reads
/// `SELECT qualify(1)` as an empty projection followed by the QUALIFY clause, never a
/// call), `PIVOT`/`UNPIVOT` (their `reserved` class likewise rejects `pivot(1)`), and
/// `SEMI`/`ANTI` (DuckDB's grammar rejects `semi(1)` despite the `type_function` class).
/// The `ASOF`/`POSITIONAL` words are *not* here: their `type_function` class admits
/// `asof(1)` / `positional(1)` as calls, matching the engine.
pub const DUCKDB_RESERVED_FUNCTION_NAME: KeywordSet = RESERVED_FUNCTION_NAME
    .difference(DUCKDB_UNRESERVED_CARVEOUT)
    .difference(DUCKDB_ORDINARY_SPECIAL_VALUE_NAMES)
    .union(DUCKDB_QUALIFY_RESERVATION)
    .union(DUCKDB_PIVOT_RESERVATION)
    .union(DUCKDB_SEMI_ANTI_JOIN_RESERVATION);

/// DuckDB type-name reject set: the shared model plus `QUALIFY` and `PIVOT`/`UNPIVOT`
/// (their `reserved` class rejects `CAST(1 AS pivot)`). The nonstandard-join and
/// semi-anti-join words are *not* here: `CAST(1 AS asof)` and `CAST(1 AS semi)` parse in
/// the engine (the type position admits any non-`reserved` word).
pub const DUCKDB_RESERVED_TYPE_NAME: KeywordSet = RESERVED_TYPE_NAME
    .difference(DUCKDB_UNRESERVED_CARVEOUT)
    .difference(DUCKDB_ORDINARY_SPECIAL_VALUE_NAMES)
    .union(DUCKDB_QUALIFY_RESERVATION)
    .union(DUCKDB_PIVOT_RESERVATION);

/// Fully reserved words rejected in a DuckDB generic `SET` value. The PostgreSQL-derived
/// base is extended by DuckDB's own `reserved` keyword additions.
pub const DUCKDB_RESERVED_SET_VALUE_WORDS: KeywordSet = RESERVED_SET_VALUE_WORDS
    .difference(DUCKDB_UNRESERVED_CARVEOUT)
    .difference(DUCKDB_ORDINARY_SPECIAL_VALUE_NAMES)
    .union(DUCKDB_QUALIFY_RESERVATION)
    .union(DUCKDB_PIVOT_RESERVATION);

/// DuckDB bare-label reject set: the shared model plus `QUALIFY`, so a projection or
/// FROM-relation bare alias cannot swallow the clause keyword (`SELECT a FROM t
/// QUALIFY …` reads the clause); `PIVOT`/`UNPIVOT`, so a source's alias cannot swallow a
/// trailing operator (`FROM t PIVOT (…)` reads the operator); and the nonstandard-join /
/// semi-anti-join keywords, whose `type_function` class the engine likewise rejects as a
/// bare projection label (`SELECT 1 asof` / `SELECT 1 semi` syntax-error while
/// `SELECT 1 AS asof` / `SELECT 1 AS semi` parse).
pub const DUCKDB_RESERVED_BARE_ALIAS: KeywordSet = RESERVED_BARE_ALIAS
    .difference(DUCKDB_ORDINARY_SPECIAL_VALUE_NAMES)
    .union(DUCKDB_UNRESERVED_BARE_ALIAS_RESERVATION)
    .union(DUCKDB_QUALIFY_RESERVATION)
    .union(DUCKDB_PIVOT_RESERVATION)
    .union(DUCKDB_NONSTANDARD_JOIN_RESERVATION)
    .union(DUCKDB_SEMI_ANTI_JOIN_RESERVATION);

impl NumericLiteralSyntax {
    /// The `DUCKDB` preset for numeric literal syntax.
    pub const DUCKDB: Self = Self {
        hex_integers: true,
        octal_integers: true,
        binary_integers: true,
        underscore_separators: true,
        // DuckDB lexes numerics loosely (`123abc` re-reads as `123` aliased), so the
        // leading-underscore radix opener stays off and `0x_1F` keeps its `0` + word split.
        radix_leading_underscore: false,
        money_literals: false,
        reject_trailing_junk: false,
    };
}

impl PredicateSyntax {
    /// The `DUCKDB` preset for predicate syntax.
    pub const DUCKDB: Self = Self {
        unparenthesized_in_list: true,
        // DuckDB rejects the SQL-standard `OVERLAPS` period predicate (engine-probed
        // 1.5.4).
        overlaps_period_predicate: false,
        // The PostgreSQL `LIKE/ILIKE ANY|ALL (array)` pattern-match quantifier is not a
        // DuckDB construct.
        pattern_match_quantifier: false,
        between_symmetric: false,
        is_normalized: false,
        // DuckDB accepts the two-word `<expr> NOT NULL` postfix (engine-measured).
        null_test_two_word_postfix: true,
        is_distinct_from: true,
        like: true,
        ilike: true,
        similar_to: true,
        empty_in_list: false,
    };
}

/// The DuckDB binding-power table: the standard table with the `->` token re-ranked
/// below every expression operator.
///
/// DuckDB lexes `->` as its own `LAMBDA_ARROW` grammar token (its `->>` stays an
/// ordinary `Op`), ranked looser than everything — measured on 1.5.4 via
/// `json_serialize_sql`: `x -> x % 2 = 0`, `x -> x OR y`, and
/// `elem -> extract(…) BETWEEN 2000 AND 2022` each put the whole right side in the
/// arrow's right operand, `NOT x -> y` and `a = x -> y` each take the full left
/// expression as the arrow's left operand, and `x -> y -> z` groups left. `4`/`5`
/// sits below `or` (10) with left-associativity, reproducing exactly that. The rank
/// belongs to the *token*, not to the lambda reading: a non-parameter left operand
/// still folds as the JSON accessor, at this same DuckDB rank (one table
/// drives parser and renderer, per dialect).
pub const DUCKDB_BINDING_POWERS: BindingPowerTable = BindingPowerTable {
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
    range_predicate_override: None,
    // The `IS`-family predicates (`IS NULL`, `IS DISTINCT FROM`, `IS TRUE`, …) rank one tier
    // below comparison, so `a <> b IS NULL` groups `(a <> b) IS NULL` and `a IS DISTINCT FROM
    // b = c` groups `a IS DISTINCT FROM (b = c)` (measured on 1.5.4 via `json_serialize_sql`).
    is_predicate_override: Some(IS_PREDICATE_BELOW_COMPARISON),
    // DuckDB lexes `==` as a generic `%left Op`, not the `%nonassoc '='` comparison.
    double_equals: BindingPower {
        left: 45,
        right: 46,
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
        left: 4,
        right: 5,
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
};

impl SelectSyntax {
    /// The `DUCKDB` preset for select syntax.
    pub const DUCKDB: Self = Self {
        // DuckDB rejects the empty target list where PostgreSQL's raw grammar accepts it.
        empty_target_list: false,
        qualify: true,
        // DuckDB's `UNION [ALL] BY NAME` name-matched set operation (probed on 1.5.4):
        // an additive grammar delta over the PostgreSQL base, UNION-only (the engine
        // rejects `INTERSECT`/`EXCEPT BY NAME`). `duckdb-union-by-name`.
        union_by_name: true,
        // DuckDB's FROM-first SELECT (`FROM t SELECT x`, bare `FROM t`) — an additive
        // grammar delta above the PostgreSQL base, which rejects a statement-position
        // `FROM`. `FROM` is reserved under the shared model, so the leading-`FROM` primary
        // can never shadow an identifier read (`duckdb-from-first-select`).
        from_first: true,
        // DuckDB's `*`/`t.*` wildcard modifiers `EXCLUDE`/`REPLACE`/`RENAME` (probed on
        // 1.5.4) — an additive grammar delta over the PostgreSQL base, which has no
        // wildcard tail. `duckdb-select-star-modifiers`.
        wildcard_modifiers: true,
        // DuckDB aliases a qualified wildcard (`t.* AS x`, engine-probed 1.5.4) — the plain
        // alias axis PostgreSQL shares, distinct from the DuckDB-only modifier tail above.
        qualified_wildcard_alias: true,
        // DuckDB rejects a ragged VALUES constructor (rows of differing width) at *parse*
        // — `Parser Error: VALUES lists must all be the same length`, in every VALUES
        // position (standalone, derived, INSERT; measured on 1.5.4) — where PostgreSQL's
        // raw grammar accepts it and defers the check to bind. A shape-level tightening
        // *below* the PostgreSQL base (like `empty_target_list`), enforced by the parser
        // comparing the parsed rows' arities. `duckdb-from-clause-parse-overaccept`.
        values_rows_require_equal_arity: true,
        // DuckDB admits a single-quoted string literal as a projection alias
        // (`(a = b) AS '(a = b)'`; probed on 1.5.4) — the MySQL-precedent
        // `alias_string_literals` gate, reusing its projection-alias round-trip machinery.
        // `duckdb-operator-and-literal-gaps`.
        alias_string_literals: true,
        // DuckDB accepts ONLY the `AS 'x'` form — the *bare* `SELECT 1 'x'` rejects (probed on
        // 1.5.4), unlike SQLite/MySQL — so the bare axis stays off here.
        bare_alias_string_literals: false,
        // DuckDB tolerates a single trailing comma in its list positions — the SELECT /
        // VALUES / collection-literal / `IN` lists (engine-probed 1.5.4), but not function
        // arguments, `ORDER BY`, or a bare row constructor. `duckdb-trailing-comma`.
        trailing_comma: true,
        // DuckDB's prefix colon alias (`SELECT j : 42`, `FROM b : a`; probed on 1.5.4) — an
        // additive grammar delta over the PostgreSQL base, pure sugar for a trailing `AS`
        // alias that folds onto the existing alias field. Conflict-free here: DuckDB has no
        // top-level semi-structured `a:b` access, the one construct that would claim the
        // same `<ident> :` head. `duckdb-colon-alias`.
        prefix_colon_alias: true,
        distinct_on: true,
        select_into: true,
        wildcard_replace: false,
        intersect_all: true,
        except_all: true,
        parenthesized_query_operands: true,
        values_row_constructor: true,
        as_alias_rejects_reserved: false,
        lateral_view_clause: false,
        connect_by_clause: false,
    };
}

impl QueryTailSyntax {
    /// The `DUCKDB` preset for query tail syntax.
    pub const DUCKDB: Self = Self {
        // DuckDB has no `FOR UPDATE`/`FOR SHARE` row locking. The
        // strength/stacking refinements stay off too
        // rather than inherit PostgreSQL's `true` for a dialect with no locking clause.
        locking_clauses: false,
        key_lock_strengths: false,
        stacked_locking_clauses: false,
        // DuckDB's `USING SAMPLE <entry>` query-level sample clause (probed on 1.5.4) —
        // an additive grammar delta over the PostgreSQL base, which has no such clause.
        // `duckdb-expression-and-clause-tails`.
        using_sample: true,
        // DuckDB's percentage `LIMIT` (`LIMIT 40 PERCENT`, `LIMIT 35%`; probed on 1.5.4) —
        // an additive grammar delta over the PostgreSQL base, which has no percentage form.
        // The marker folds only onto a numeric-literal count at a clause boundary, so
        // ordinary modulo (`LIMIT 10 % 3`) and non-literal counts stay unaffected.
        // `duckdb-limit-percent`.
        limit_percent: true,
        // PostgreSQL's raw-parse `WITH TIES` guards are not modelled for DuckDB (conservative
        // — DuckDB's own `WITH TIES` validity is unprobed here); keep the PG-only behaviour.
        with_ties_requires_order_by: false,
        fetch_first: true,
        limit_offset_comma: false,
        leading_offset: true,
        limit_expressions: true,
        pipe_syntax: false,
        limit_by_clause: false,
        settings_clause: false,
        format_clause: false,
        for_xml_json_clause: false,
    };
}

impl GroupingSyntax {
    /// The `DUCKDB` preset for grouping syntax.
    pub const DUCKDB: Self = Self {
        // DuckDB's `GROUP BY ALL` / `ORDER BY ALL` clause modes (probed on 1.5.4) —
        // purely additive grammar deltas: `ALL` is reserved under the shared
        // PostgreSQL-derived model, so neither branch can shadow an identifier read.
        group_by_all: true,
        group_by_set_quantifier: false,
        order_by_all: true,
        grouping_sets: true,
        with_rollup: false,
        order_by_using: true,
    };
}

impl ExpressionSyntax {
    /// The `DUCKDB` preset for expression syntax.
    pub const DUCKDB: Self = Self {
        collection_literals: true,
        // The three-bound `[lower:upper:step]` slice with its `-` open-upper placeholder.
        slice_step: true,
        // The `#n` positional column reference — a DuckDB-only extension.
        positional_column: true,
        lambda_keyword: true,
        // DuckDB parses `(struct).field` but
        // has no `.*` value-expansion production — `(struct).*`, `ROW(t.*)`, `f(t.*)`,
        // `t.*::type` all parse-reject (engine-probed 1.5.4). Override the POSTGRES `true`.
        field_wildcard: false,
        // DuckDB reaches nested `ARRAY[[1,2],[3,4]]` through `collection_literals` (a
        // top-level `[…]` list is a value there, and levels may mix scalars and lists),
        // so the multidimensional array production stays off.
        multidim_array_literals: false,
        semi_structured_access: false,
        // The relaxed interval spellings (`INTERVAL 3 DAYS`, `INTERVAL (x) DAY`).
        relaxed_interval_syntax: true,
        typecast_operator: true,
        subscript: true,
        collate: true,
        at_time_zone: true,
        array_constructor: true,
        row_constructor: true,
        struct_constructor: false,
        field_selection: true,
        typed_string_literals: true,
        typed_interval_literal: true,
        mysql_interval_operator: false,
    };
}

impl OperatorSyntax {
    /// The `DUCKDB` preset for operator syntax.
    pub const DUCKDB: Self = Self {
        lambda_expressions: true,
        double_equals: true,
        integer_divide_slash: true,
        starts_with_operator: true,
        // Off, overriding the inherited PostgreSQL `true`: DuckDB spells `?` as the anonymous
        // placeholder (`anonymous_question`), which contends with the `?`-led `jsonb`
        // operators, and it has none of that PostgreSQL `jsonb` operator family. Forcing it
        // off keeps `FeatureSet::DUCKDB` lexically consistent (the `const` assert below).
        jsonb_operators: false,
        // On, inheriting PostgreSQL's `true`. `^`-as-exponentiation (`caret_operator:
        // Exponent`, on the preset below) turns on too: both were probed against DuckDB 1.5.4
        // (`duckdb-operator-surface-sweep`, `duckdb-pg-operator-spelling-under-acceptance`).
        //
        // `^` is exponentiation with the *same* precedence row this preset already carries
        // (the shared [`exponent`](crate::precedence::BindingPowerTable::exponent) rank, which
        // `DUCKDB_BINDING_POWERS` inherits unchanged): probed `2^3*2 = 16` (`^` tighter than
        // `*`), `2^3^2 = 64` (left-associative), `-2^2 = 4` (unary sign tighter than `^`) —
        // identical to the PostgreSQL fit. So `CaretOperator::Exponent` is the honest model.
        // (DuckDB also spells the same power as `**`, an unmodelled synonym with no corpus
        // member — tracked on the sweep ticket, not this flag.)
        //
        // `custom_operators` turns on: DuckDB inherits PostgreSQL's generalized maximal-munch
        // operator lexer and *parse*-accepts the same `Op`-class runs — `1 <<| 2`, `1 <-> 2`,
        // `p &&&&&@ q`, regex `~`/`!~`/`~*` — via `duckdb_extract_statements`, then
        // bind-rejects the ones with no backing function (`1 <<| 2` → Catalog error). A
        // parse-accept that binds-fail is still under-acceptance when we reject at *parse*: our
        // parser is parse-only and the DuckDB accept/reject oracle compares parse acceptance
        // (`m2::duckdb_raw_bytes_divergence` reads `extract_statement_count`), so folding an
        // unknown run onto [`Expr::NamedOperator`](crate::ast::Expr::NamedOperator) matches the
        // engine's parse verdict — it does not claim DuckDB is user-extensible. DuckDB's real
        // operators still fold onto their dedicated [`BinaryOperator`] keys ahead of the generic
        // surface (`&&`/`^@`/`//`/`==` in `known_operator_token`), so their shape is unchanged.
        //
        // DuckDB's `Op` charset is PostgreSQL's *minus* `#` and `?`, which it repurposes as the
        // positional-column sigil (`#1`) and the anonymous parameter placeholder — the lexer's
        // `is_operator_char` drops them under `positional_column` / `anonymous_question`, so a
        // run stops at either (`1 @#@ 2` is `@` then a stray `#` — reject on both DuckDB and
        // here; `1 @?@ 2` is `@` then a `?` placeholder). Engine-measured across the full
        // single/doubled/prefix/postfix/trailing-sign matrix on DuckDB 1.5.4
        // (`duckdb-pg-operator-spelling-under-acceptance`). Backtick is an `Op`-class byte here
        // (DuckDB does not quote identifiers with it), so `` `= `` lexes as an operator. DuckDB
        // *postfix* symbolic operators (`1 !`, `1 ~`, `1 @` — PostgreSQL removed postfix in 14)
        // are a distinct axis carried by `postfix_operators` below (the parser-side postfix
        // reduction), not by this tokenizer-plus-infix/prefix flag.
        custom_operators: true,
        // On, overriding the inherited PostgreSQL `false`: DuckDB keeps the generalized postfix
        // reading PostgreSQL removed in version 14 — `10!`, `1 ~`, `1 <->`, `1 &` all
        // parse-accept (then bind-reject the ones with no backing `__postfix` function).
        // Engine-measured on DuckDB 1.5.4 (`duckdb-postfix-operator-dimension`). See the flag
        // doc for the MECE split against `custom_operators` and the eligible-token set.
        postfix_operators: true,
        // Inherited from DuckDB's PostgreSQL-fork grammar, which keeps the `a_expr ISNULL` /
        // `a_expr NOTNULL` postfix synonyms (additive over PostgreSQL like the other shared
        // operator knobs).
        null_test_postfix: true,
        // The PostgreSQL any-operator quantifier (`3 * ANY(list)`) is not a DuckDB
        // construct.
        quantified_arbitrary_operator: false,
        operator_construct: true,
        containment_operators: true,
        json_arrow_operators: true,
        is_general_equality: false,
        truth_value_tests: true,
        null_safe_equals: false,
        bitwise_operators: true,
        quantified_comparisons: true,
        quantified_comparison_lists: true,
    };
}

impl CallSyntax {
    /// The `DUCKDB` preset for call syntax.
    pub const DUCKDB: Self = Self {
        columns_expression: true,
        try_cast: true,
        extract_string_field: true,
        method_chaining: true,
        variadic_argument: true,
        // The PostgreSQL SQL/JSON empty-constructor reject is not modelled for DuckDB
        // (conservative — DuckDB's `json()` surface is unprobed here); keep it a plain call.
        sqljson_constructors_require_argument: false,
        // DuckDB has no SQL:2016 SQL/JSON expression-function special forms (its JSON
        // support is ordinary functions like `json_extract`), so the keyword heads stay
        // plain call/name forms — override the inherited PostgreSQL `true`.
        sqljson_expression_functions: false,
        // DuckDB has no SQL/XML expression functions; override the inherited PostgreSQL
        // `true` so the `xml*` keyword heads stay plain call/name forms.
        xml_expression_functions: false,
        // DuckDB has no `merge_action()` support function; override PostgreSQL's `true` so
        // the reserved keyword head stays the "no call form" reject (conservative — DuckDB's
        // MERGE surface is unprobed here).
        merge_action_function: false,
        named_argument: true,
        utc_special_functions: false,
        extract_from_syntax: true,
        restricted_cast_targets: false,
        convert_function: false,
    };
}

impl StringFuncForms {
    /// The `DUCKDB` preset for string func forms.
    pub const DUCKDB: Self = Self {
        // DuckDB's PG-fork string special forms diverge from PostgreSQL in exactly two
        // knobs (both probed on 1.5.4): the SIMILAR/ESCAPE regex substring production
        // was dropped (parser error), and OVERLAY kept *only* the PLACING form —
        // `overlay('abc', 'X', 2, 1)` / `overlay('abc')` / `overlay()` are parser
        // errors where PostgreSQL parse-accepts them as plain calls. Everything else
        // (FROM/FOR + FOR-leading substring orders, b_expr POSITION operands, the
        // loose trim_list tails) inherits PostgreSQL's `true` verbatim, each probed
        // parse-accepting on the live engine.
        substring_similar: false,
        overlay_requires_placing: true,
        // DuckDB's `COLLATION FOR (<expr>)` surface is unprobed; override PostgreSQL's
        // `true` back to `false` (conservative — `COLLATION` stays an ordinary name head).
        collation_for_expression: false,
        substring_from_for: true,
        substring_leading_for: true,
        substring_plain_call_requires_2_or_3_args: false,
        substr_from_for: false,
        position_in: true,
        position_asymmetric_operands: false,
        overlay_placing: true,
        trim_from: true,
        trim_list_syntax: true,
        ceil_to_field: false,
        floor_to_field: false,
        match_against: false,
    };
}

impl AggregateCallSyntax {
    /// The `DUCKDB` preset for aggregate call syntax.
    pub const DUCKDB: Self = Self {
        null_treatment: true,
        standalone_argument_order_by: true,
        // DuckDB accepts `FILTER (<predicate>)` without the standard `WHERE` (probed on 1.5.4).
        filter_optional_where: true,
        group_concat_separator: false,
        within_group: true,
        aggregate_filter: true,
        aggregate_args_require_adjacent_paren: false,
        aggregate_calls_reject_empty_arguments: false,
        over_requires_windowable_function: false,
        window_function_tail: false,
    };
}

impl TypeNameSyntax {
    /// The `DUCKDB` preset for type name syntax.
    pub const DUCKDB: Self = Self {
        composite_types: true,
        enum_type: true,
        // Empty `DECIMAL()`/`DEC()`/`NUMERIC()` parens mean the default `(18,3)` — probed on
        // 1.5.4, byte-identical to a bare `DECIMAL` (`duckdb-empty-type-parens`).
        empty_type_parens: true,
        // DuckDB requires an unsigned `DECIMAL` modifier — a negative scale is rejected
        // (probed on 1.5.4), unlike PostgreSQL, so this PG-inherited flag is turned back off.
        signed_type_modifier: false,
        // DuckDB admits a string-literal type modifier on a user-defined type name —
        // `GEOMETRY('OGC:CRS84')` and the general `type_name('constant', ...)` form (probed
        // on 1.5.4). `duckdb-geometry-type-and-overlaps-operator`.
        string_type_modifiers: true,
        extended_scalar_type_names: false,
        set_type: false,
        numeric_modifiers: false,
        integer_display_width: false,
        varchar_requires_length: false,
        zoned_temporal_types: true,
        character_set_annotation: false,
        nullable_type: false,
        low_cardinality_type: false,
        fixed_string_type: false,
        datetime64_type: false,
        nested_type: false,
        bit_width_integer_names: false,
        liberal_type_names: false,
        angle_bracket_types: false,
    };
}

impl TableExpressionSyntax {
    /// The `DUCKDB` preset for table expression syntax.
    pub const DUCKDB: Self = Self {
        // DuckDB's string-literal table alias (`FROM integers AS 't'('k')` / `t('k')`;
        // probed on 1.5.4). `duckdb-string-literal-table-alias`.
        string_literal_aliases: true,
        only: true,
        table_sample: true,
        parenthesized_joins: true,
        table_alias_column_lists: true,
        join_using_alias: true,
        index_hints: false,
        table_hints: false,
        partition_selection: false,
        base_table_alias_column_lists: true,
        aliased_parenthesized_join: true,
        bare_table_alias_is_bare_label: false,
        table_version: false,
        table_json_path: false,
        indexed_by: false,
    };
}

impl JoinSyntax {
    /// The `DUCKDB` preset for join syntax.
    pub const DUCKDB: Self = Self {
        asof_join: true,
        positional_join: true,
        semi_anti_join: true,
        // Spark/Hive-only; DuckDB parse-rejects the sided `LEFT/RIGHT SEMI/ANTI JOIN`
        // spelling (engine-probed), accepting only its own side-less `SEMI`/`ANTI JOIN`.
        sided_semi_anti_join: false,
        // MSSQL-only; DuckDB parse-rejects `APPLY` in join position.
        apply_join: false,
        // DuckDB parse-rejects the SQL:2023 recursive-query SEARCH/CYCLE clauses
        // (`syntax error at or near "SEARCH"`, probed on 1.5.4), so it overrides the
        // PostgreSQL surface it otherwise inherits below — the `data_modifying_ctes` split.
        recursive_search_cycle: false,
        // DuckDB parse-rejects a top-level ORDER BY/LIMIT/OFFSET on a `UNION`-bodied
        // recursive CTE (`Parser Error: ORDER BY in a recursive query is not allowed`;
        // probed on 1.5.4), overriding the inherited PostgreSQL parse-accept.
        // `duckdb-recursive-cte-term-restrictions-over-accept`.
        recursive_union_rejects_order_limit: true,
        // DuckDB's keyed-recursion `USING KEY (cols)` clause between the CTE column list and
        // `AS` (stable since 1.3; probed accepting on 1.5.4), overriding the inherited
        // PostgreSQL off. `duckdb-with-using-key`.
        recursive_using_key: true,
        stacked_join_qualifiers: true,
        full_outer_join: true,
        natural_cross_join: false,
        straight_join: false,
    };
}

impl TableFactorSyntax {
    /// The `DUCKDB` preset for table factor syntax.
    pub const DUCKDB: Self = Self {
        pivot: true,
        unpivot: true,
        // DuckDB's `DESCRIBE`/`SHOW`/`SUMMARIZE` utility as a parenthesized `FROM`
        // table source (its `SHOW_REF` table_ref; probed on 1.5.4).
        // `duckdb-statement-in-query-position`.
        show_ref: true,
        // DuckDB's bare `FROM VALUES (…) AS t` row-list table factor (no parentheses;
        // probed on 1.5.4). `duckdb-from-values-table-factor`.
        from_values: true,
        // DuckDB parse-rejects `JSON_TABLE(… COLUMNS …)` (reads `json_table` as an ordinary
        // name) and `XMLTABLE(` (probed on 1.5.4), so both override the inherited PostgreSQL
        // surface off — the same `recursive_search_cycle` split above.
        json_table: false,
        xml_table: false,
        lateral: true,
        table_functions: true,
        rows_from: true,
        unnest: true,
        unnest_with_offset: false,
        table_function_ordinality: true,
        special_function_table_source: true,
        table_expr_factor: false,
        pivot_value_sources: false,
        match_recognize: false,
        open_json: false,
    };
}

impl UtilitySyntax {
    /// The `DUCKDB` preset for utility syntax.
    pub const DUCKDB: Self = Self {
        start_transaction: true,
        start_transaction_block_optional: true,
        transaction_work_keyword: true,
        begin_transaction_keyword: true,
        commit_transaction_keyword: true,
        rollback_transaction_keyword: true,
        transaction_name: false,
        begin_transaction_modes: true,
        transaction_savepoints: false,
        set_transaction: false,
        transaction_isolation_mode: false,
        transaction_access_mode: true,
        transaction_deferrable_mode: false,
        start_transaction_isolation_mode: false,
        start_transaction_deferrable_mode: false,
        start_transaction_consistent_snapshot: false,
        transaction_multiple_modes: false,
        transaction_mode_comma_required: false,
        transaction_modes_unique: false,
        abort_transaction_alias: true,
        end_transaction_alias: true,
        transaction_release: false,
        transaction_chain: false,
        release_savepoint_keyword_optional: true,
        pragma: true,
        use_statement: true,
        // DuckDB's `USE <catalog> . <schema>` admits the dotted two-part name (MySQL's
        // single-ident form is a subset); the deeper `USE a.b.c` is still parser-rejected.
        use_qualified_name: true,
        prepared_statements: true,
        // DuckDB structurally rejects the PostgreSQL `PREPARE name(<type>, …)` typed
        // parameter-type list
        // ("Prepared statement argument types are not supported, use CAST"; probed on
        // 1.5.4), so only the bare `PREPARE <name> AS <statement>` form is admitted.
        prepare_typed_parameters: false,
        call: true,
        // DuckDB has `ATTACH`/`DETACH` (its own catalog databases), so the shared gate is
        // on — closing the `DETACH <db>` coverage gap. It extends `DETACH DATABASE` with an
        // `IF EXISTS` guard (`detach_if_exists`), and adds the `[FORCE] CHECKPOINT [db]`
        // operands (`checkpoint_database`), the bare-identifier `LOAD tpch` argument
        // (`load_bare_name`), and the `RESET`-scope prefix (`reset_scope`) — all DuckDB
        // extensions alongside `CHECKPOINT` and extension loading.
        attach: true,
        load_bare_name: true,
        reset_scope: true,
        detach_if_exists: true,
        // Override the PostgreSQL base: DuckDB has no `DO` anonymous code block (`DO $$...$$`
        // is a parser error, probed on 1.5.4), so the leading `DO` keyword stays
        // undispatched and surfaces as an unknown statement.
        do_statement: false,
        // DuckDB's `EXPORT DATABASE`/`IMPORT DATABASE` catalogue round-trip — the pair is a
        // DuckDB extension gated as one unit.
        export_import_database: true,
        // DuckDB's `UPDATE EXTENSIONS [( <name>, ... )]` extension-refresh statement — a
        // DuckDB extension with its own statement head.
        update_extensions: true,
        // Every remaining utility head is explicitly pinned below.
        copy: true,
        copy_into: false,
        stage_references: false,
        comment_on: true,
        comment_if_exists: false,
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
        prepared_statements_from: false,
        call_bare_name: false,
        load_extension: true,
        load_data: false,
        do_expression_list: false,
        lock_tables: false,
        lock_instance: false,
        begin_transaction_mode: false,
        xa_transactions: false,
        rename_statement: false,
        signal_diagnostics: false,
        flush: false,
        purge_binary_logs: false,
        replication_statements: false,
    };
}

impl ShowSyntax {
    /// The `DUCKDB` preset for show syntax.
    pub const DUCKDB: Self = Self {
        // DuckDB's `{DESCRIBE | SUMMARIZE} <query> | <table>` introspection statement — the
        // `SHOW_REF` utility (probed on 1.5.4), which it desugars to `SELECT * FROM
        // (SHOW_REF …)` and shares with the parenthesized-`FROM` `show_ref` table factor.
        describe_summarize: true,
        // DuckDB's `SHOW [ALL] TABLES [FROM <schema>]` catalogue listing (engine-probed
        // 1.5.4), which it desugars to `SELECT * FROM (SHOW_REF …)`; modelled as the typed
        // statement, distinct from the generic session `SHOW <var>` inherited from PG.
        show_tables: true,
        describe: false,
        session_statements: true,
        set_value_reserved_words: DUCKDB_RESERVED_SET_VALUE_WORDS,
        set_value_on_keyword: false,
        set_value_null_keyword: true,
        show_columns: false,
        show_create_table: false,
        show_functions: false,
        show_routine_status: false,
        show_verbose: false,
        show_admin: false,
    };
}

impl MaintenanceSyntax {
    /// The `DUCKDB` preset for maintenance syntax.
    pub const DUCKDB: Self = Self {
        checkpoint_database: true,
        // DuckDB's `VACUUM [ANALYZE] [<table> [(<cols>)]]` and `ANALYZE [<table>
        // [(<cols>)]]` statistics/compaction statements (both `PGVacuumStmt` in libpg_query;
        // engine-probed 1.5.4). The leading `VACUUM` dispatches under `vacuum_analyze` (a
        // separate gate from SQLite's `INTO`-shaped `vacuum`, which stays off); the leading
        // `ANALYZE` under `analyze`, with the DuckDB column
        // list under `analyze_columns`. Only the `ANALYZE` vacuum option parses —
        // `FULL`/`FREEZE`/`VERBOSE`/`disable_page_skipping` throw in 1.5.4's transform.
        vacuum_analyze: true,
        analyze: true,
        analyze_columns: true,
        // The other maintenance statement heads stay off.
        vacuum: false,
        reindex: false,
        checkpoint: true,
        table_maintenance: false,
    };
}

impl AccessControlSyntax {
    /// The `DUCKDB` preset for access control syntax.
    pub const DUCKDB: Self = Self {
        // DuckDB recognizes the PostgreSQL-shaped GRANT/REVOKE and ALTER ROLE forms.
        alter_role_rename: true,
        access_control: true,
        access_control_extended_objects: true,
        user_role_management: false,
        access_control_account_grants: false,
    };
}

impl FeatureSet {
    /// DuckDB as PostgreSQL-derived dialect data.
    pub const DUCKDB: Self = Self {
        identifier_casing: Casing::Lower,
        identifier_quotes: STANDARD_IDENTIFIER_QUOTES,
        default_null_ordering: NullOrdering::NullsLast,
        reserved_column_name: DUCKDB_RESERVED_COLUMN_NAME,
        reserved_function_name: DUCKDB_RESERVED_FUNCTION_NAME,
        reserved_type_name: DUCKDB_RESERVED_TYPE_NAME,
        reserved_bare_alias: DUCKDB_RESERVED_BARE_ALIAS,
        reserved_as_label: KeywordSet::EMPTY,
        // DuckDB relation names are `catalog.schema.table` in the shared table path (its
        // own two-part narrowing is a separate, unstarted tightening).
        catalog_qualified_names: true,
        // The shared M1 table plus the vertical tab (`0x0b`) as statement-boundary trim
        // (`DUCKDB_BYTE_CLASSES`): `libduckdb`-measured, DuckDB folds a `\v` at each
        // `;`-segment's leading/trailing edge (`"\x0bSELECT 1"`, `"SELECT 1\x0b"` accept)
        // but rejects one interior to a statement's content (`"SELECT\x0b1"` rejects,
        // even beside a real space) — the tokenizer's `skip_trivia` boundary guard.
        byte_classes: DUCKDB_BYTE_CLASSES,
        binding_powers: DUCKDB_BINDING_POWERS,
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        string_literals: StringLiteralSyntax::POSTGRES,
        numeric_literals: NumericLiteralSyntax::DUCKDB,
        // PostgreSQL's `$1` positional parameters plus DuckDB's anonymous `?` placeholder
        // (`SELECT 'Test' LIMIT ?`). `?` adds a second parameter claimant but no lexical
        // conflict: DuckDB has no `?`-led operator (its JSON `?`/`?|`/`?&` existence
        // operators are unimplemented — `SELECT '{}'::JSON ? 'a'` syntax-errors on 1.5.4),
        // so `?` has a single claimant and the `is_lexically_consistent` ratchet below
        // still holds.
        parameters: ParameterSyntax {
            anonymous_question: true,
            positional_dollar: true,
            positional_dollar_large: true,
            named_colon: false,
            named_at: false,
            named_dollar: false,
            numbered_question: false,
        },
        session_variables: SessionVariableSyntax::ANSI,
        identifier_syntax: IdentifierSyntax::POSTGRES,
        table_expressions: TableExpressionSyntax::DUCKDB,
        join_syntax: JoinSyntax::DUCKDB,
        table_factor_syntax: TableFactorSyntax::DUCKDB,
        expression_syntax: ExpressionSyntax::DUCKDB,
        operator_syntax: OperatorSyntax::DUCKDB,
        call_syntax: CallSyntax::DUCKDB,
        string_func_forms: StringFuncForms::DUCKDB,
        aggregate_call_syntax: AggregateCallSyntax::DUCKDB,
        predicate_syntax: PredicateSyntax::DUCKDB,
        pipe_operator: PipeOperator::StringConcat,
        double_ampersand: DoubleAmpersand::Overlaps,
        // DuckDB's `GLOB` infix (desugars to `~~~` glob match; probed 1.5.4). MATCH/REGEXP
        // keyword forms are not DuckDB infix operators.
        keyword_operators: KeywordOperators::DuckDb,
        // The one bitwise divergence from PostgreSQL: DuckDB has no bitwise XOR operator —
        // it rejects PostgreSQL's `#` and reads `^` as *exponentiation* (both measured on
        // 1.5.4: `SELECT 5 # 3` syntax-errors, `SELECT 5 ^ 3` is `125`). It spells bitwise
        // XOR as the `xor(a, b)` function instead, which parses as an ordinary call. The
        // shared `| & ~ << >>` family stays on, inherited via `ExpressionSyntax::DUCKDB`.
        // `^`-as-exponentiation is the honest `caret_operator` reading (rationale in the
        // `OperatorSyntax::DUCKDB` block above); `#` is not the XOR operator.
        caret_operator: CaretOperator::Exponent,
        hash_bitwise_xor: false,
        // DuckDB shares PostgreSQL's flex-derived scanner, so a `--` line comment ends at
        // `\r` as well as `\n` (measured on 1.5.4) — `CommentSyntax::POSTGRES`, not the
        // `\n`-only ANSI baseline. Block-comment nesting (also on in `POSTGRES`) already
        // matched DuckDB, so this is the only comment-shape change from `ANSI`.
        comment_syntax: CommentSyntax::POSTGRES,
        mutation_syntax: MutationSyntax {
            // DuckDB parse-rejects a DML CTE body (`A CTE needs a SELECT`; INSERT,
            // UPDATE, and DELETE bodies all probed on 1.5.4). MERGE
            // (1.4+), MERGE … RETURNING, and the leading `WITH` before MERGE, all
            // probed accepted on 1.5.4 — is shared with PostgreSQL.
            data_modifying_ctes: false,
            // DuckDB rejects `OVERRIDING` *inside* MERGE (`syntax error at or near
            // "OVERRIDING"`, probed on 1.5.4) even though it accepts it on a top-level
            // INSERT. The `WHEN NOT MATCHED BY SOURCE/TARGET` arms and
            // `INSERT DEFAULT VALUES` are both accepted on 1.5.4.
            merge_insert_overriding: false,
            // DuckDB MERGE extensions (probed on 1.5.4): `UPDATE SET *`, `INSERT *` /
            // `INSERT BY NAME [*]`, and `THEN ERROR`.
            merge_update_set_star: true,
            merge_insert_star_by_name: true,
            merge_error_action: true,
            // DuckDB accepts SQLite-style `INSERT OR REPLACE` / `OR IGNORE` (probed 1.5.4).
            or_conflict_action: true,
            insert_column_matching: true,
            // DuckDB parser rejects explicit tuple-assignment value-row arity mismatches
            // (`UPDATE t SET (a, b, c) = (1, 2)`), unlike PostgreSQL parse-only.
            update_tuple_value_row_arity: true,
            // DuckDB rejects qualified SET targets (`UPDATE t SET t.i = 1` — probed 1.5.4).
            update_set_qualified_column: false,
            insert_ignore: false,
            insert_overwrite: false,
            returning: true,
            on_conflict: true,
            on_duplicate_key_update: false,
            multi_column_assignment: true,
            where_current_of: true,
            merge: true,
            replace_into: false,
            insert_set: false,
            update_delete_tails: false,
            joined_update_delete: false,
            delete_using: true,
            update_from: true,
            delete_using_target_alias: true,
            cte_before_insert: true,
            cte_before_merge: true,
            merge_when_not_matched_by: true,
            merge_insert_default_values: true,
            merge_insert_multirow: false,
        },
        // PostgreSQL's schema-change surface plus DuckDB's live-body macro DDL
        // (`CREATE MACRO`/`CREATE FUNCTION … AS <expr>|TABLE <query>`), `CREATE OR REPLACE
        // TABLE`, and the `CREATE [PERSISTENT] SECRET` secrets statement.
        statement_ddl_gates: StatementDdlGates {
            // DuckDB accepts the shared sequence option core but rejects PostgreSQL's
            // `CACHE` extension.
            create_sequence_cache: false,
            colocation_groups: false,
            materialized_view_to: false,
            create_trigger: false,
            create_macro: true,
            create_secret: true,
            create_type: true,
            create_virtual_table: false,
            create_sequence: true,
            // DuckDB has no `CREATE DATABASE` (it uses `ATTACH`); the shared gate is off so
            // `DATABASE` after `CREATE` falls through as an unknown statement (probed
            // 1.5.4: "syntax error at or near \"DATABASE\"").
            databases: false,
            // DuckDB rejects the SQL-standard embedded schema-element form.
            schema_elements: false,
            schemas: true,
            drop_database: false,
            materialized_views: true,
            temporary_views: true,
            routines: true,
            or_replace: true,
            // DuckDB accepts `CREATE [OR REPLACE] [TEMP] RECURSIVE VIEW v (cols) AS …`
            // (engine-measured on 1.5.4).
            recursive_views: true,
            compound_statements: false,
            // DuckDB manages extensions with `INSTALL`/`LOAD`, not the PostgreSQL
            // `CREATE`/`ALTER EXTENSION` catalogue DDL.
            extension_ddl: false,
            // DuckDB has no transform catalogue (`pg_transform` / `CREATE TRANSFORM` is
            // PostgreSQL-only), so it must clear the POSTGRES `true` rather than inherit it.
            transform_ddl: false,
            // DuckDB has no `ALTER SYSTEM` server-configuration DDL (it configures through
            // `SET`/`RESET`), so it must clear the POSTGRES `true` rather than inherit it.
            alter_system: false,
            // MySQL's tablespace / logfile-group storage DDL is not a DuckDB statement.
            tablespace_ddl: false,
            logfile_group_ddl: false,
            // DuckDB's `ALTER DATABASE … SET ALIAS TO`, `ALTER SEQUENCE …` option list, and
            // `ALTER {TABLE|VIEW|SEQUENCE} … SET SCHEMA` forms (engine-measured on 1.5.4).
            alter_database: true,
            // MySQL-only families have no DuckDB equivalent.
            alter_database_options: false,
            server_definition: false,
            alter_instance: false,
            spatial_reference_system: false,
            resource_group: false,
            alter_sequence: true,
            alter_object_set_schema: true,
            view_definition_options: false,
        },
        create_table_clause_syntax: CreateTableClauseSyntax {
            create_or_replace_table: true,
            // DuckDB has no PostgreSQL-style declarative partitioning (its `PARTITION_BY` is a
            // COPY/export option, not a `CREATE TABLE` clause).
            declarative_partitioning: false,
            // DuckDB has no table inheritance and (probed against libduckdb) rejects the
            // PostgreSQL `(LIKE src …)` source-table element.
            table_inheritance: false,
            like_source_table: false,
            table_access_method: false,
            without_oids: false,
            typed_tables: false,
            create_table_as_execute: false,
            table_options: false,
            without_rowid_table_option: false,
            strict_table_option: false,
            storage_parameters: true,
            on_commit: true,
            create_table_as_with_data: true,
            statement_level_table_like: false,
            unlogged_tables: true,
        },
        column_definition_syntax: ColumnDefinitionSyntax {
            // The PostgreSQL `b_expr` column-default restriction is not modelled for DuckDB
            // (conservative — DuckDB's default-expression grammar class is unprobed here); it
            // reads the default as a full `a_expr`, unchanged.
            column_default_requires_b_expr: false,
            // DuckDB (probed against libduckdb) accepts a per-column `COLLATE <name>` and the
            // `UNLOGGED` persistence keyword, but rejects the column STORAGE/COMPRESSION
            // attributes, the table USING
            // access method, WITHOUT OIDS, and typed `OF <type>` tables, so those four must
            // clear the POSTGRES `true` rather than inherit it.
            column_storage: false,
            // DuckDB accepts the keywordless generated-column shorthand `<col> <type> AS
            // (<expr>) [VIRTUAL|STORED]` written without `GENERATED ALWAYS` (libduckdb 1.5.4:
            // `y INT AS (x + 1)` and `… VIRTUAL` parse-accept; `STORED` parses but is a binder
            // reject, out of this layer). PostgreSQL requires the keywords, so this must set
            // rather than inherit the POSTGRES `false`.
            generated_column_shorthand: true,
            // DuckDB requires a data type on every column *except* a generated one: both the
            // `AS (<expr>)` shorthand and the keyworded `GENERATED …` form may drop the type
            // (`CREATE TABLE t (x INT, gen_x AS (x + 5))`), while a plain typeless column is a
            // parse error. A narrowing of the SQLite typeless rule, distinct from PostgreSQL's
            // type-required `false`, so it too must set rather than inherit.
            typeless_generated_columns: true,
            column_conflict_resolution_clause: false,
            typeless_column_definitions: false,
            joined_autoincrement_attribute: false,
            inline_primary_key_ordering: false,
            named_column_collate_constraint: false,
            identity_columns: true,
            compact_identity_columns: false,
            default_expression_requires_parens: false,
            column_collation: true,
        },
        constraint_syntax: ConstraintSyntax {
            // DuckDB (probed against libduckdb 1.5.4) rejects PostgreSQL's `EXCLUDE` exclusion
            // constraints, the `AS EXECUTE` CTAS form, and the `UNIQUE`/`PRIMARY KEY`
            // index-parameter decorations (`INCLUDE`/`NULLS NOT DISTINCT`/`USING INDEX
            // TABLESPACE`), so all three must clear the POSTGRES `true`. It *does* accept the
            // `NO INHERIT` / `NOT VALID` constraint markers.
            exclusion_constraints: false,
            index_constraint_parameters: false,
            // DuckDB admits `ON DELETE`/`ON UPDATE` only for `RESTRICT`/`NO ACTION` —
            // `CASCADE`/`SET NULL`/`SET DEFAULT` are Parser Errors (probed 1.5.4).
            referential_action_cascade_set: false,
            // DuckDB (and SQLite) parse-reject subqueries in `CHECK` (probed 1.5.4).
            check_constraint_subqueries: false,
            deferrable_constraints: true,
            named_inline_non_check_constraints: true,
            bare_constraint_name: false,
            constraint_no_inherit_not_valid: true,
            constraint_column_collate_order: false,
        },
        index_alter_syntax: IndexAlterSyntax {
            // DuckDB's extended ALTER surface is on (IF EXISTS, RENAME, ALTER COLUMN, …)
            // but multi-action lists are not ("Only one ALTER command per statement",
            // probed 1.5.4).
            alter_table_multiple_actions: false,
            alter_nested_column_paths: true,
            // DuckDB parses table-option assignment lists and standalone index storage
            // parameters; option names and values remain binder concerns.
            alter_table_set_options: true,
            index_storage_parameters: true,
            // Table-scoped `DROP INDEX` remains outside the grammar.
            rename_constraint: true,
            drop_primary_key: false,
            alter_column_add_identity: false,
            drop_behavior: true,
            index_drop_on_table: false,
            index_concurrently: true,
            index_using_method: true,
            partial_index: true,
            index_if_not_exists: true,
            index_nulls_order: true,
            alter_table_extended: true,
            alter_existence_guards: true,
            alter_column_set_data_type: true,
            routine_arg_types: true,
            routine_arg_defaults: true,
            routine_arg_modes: true,
            routine_language_string: true,
        },
        existence_guards: ExistenceGuards::POSTGRES,
        select_syntax: SelectSyntax::DUCKDB,
        query_tail_syntax: QueryTailSyntax::DUCKDB,
        grouping_syntax: GroupingSyntax::DUCKDB,
        utility_syntax: UtilitySyntax::DUCKDB,
        show_syntax: ShowSyntax::DUCKDB,
        maintenance_syntax: MaintenanceSyntax::DUCKDB,
        access_control_syntax: AccessControlSyntax::DUCKDB,
        type_name_syntax: TypeNameSyntax::DUCKDB,
        // No DuckDB-specific Tier-1 output spelling yet: DuckDB shares PostgreSQL's
        // canonical type names (`INTEGER`, `VARCHAR`, `DECIMAL`), so it renders through
        // the PostgreSQL spelling table (minting a `TargetSpelling::DuckDb` is render
        // work a later ticket owns).
        target_spelling: TargetSpelling::Postgres,
    };
}

/// Prefer [`FeatureSet::DUCKDB`] for struct update.
pub const DUCKDB: FeatureSet = FeatureSet::DUCKDB;

// Compile-time proof the DuckDB preset claims no shared tokenizer trigger twice. Beyond
// PostgreSQL's lexical surface it adds exactly one trigger — the anonymous `?` parameter
// (`anonymous_question`) — which has a single claimant: DuckDB implements no `?`-led
// operator (its JSON `?`/`?|`/`?&` existence operators are absent), so `?` cannot contend
// and needs no registered conflict. The rest is PostgreSQL's surface: the numeric-radix
// scan, empty-target grammar gate, and QUALIFY reservation add no lexical trigger,
// `collection_literals` reuses the `[` punctuation PostgreSQL's `subscript`/
// `array_constructor` already claim (no preset here quotes identifiers with `[`, so the
// registered `BracketIdentifierVersusArraySyntax` conflict cannot fire), and
// `lambda_expressions` is a grammar-position gate over the `->` token
// `json_arrow_operators` already lexes (one lexical claimant; the lambda/JSON split
// happens in the parser by LHS shape). Kept as its own ratchet so a future DuckDB delta
// that *does* add a trigger fails the build here.
const _: () = assert!(FeatureSet::DUCKDB.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: every
// refinement flag (`slice_step`, `checkpoint_database`, `analyze_columns`, the bare-name
// utility tails) rides its enabled base, and no two features contend for one
// parser-position head (`prepared_statements_from` stays off, so the typed-`AS` lifecycle
// is unrivalled).
const _: () = assert!(FeatureSet::DUCKDB.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::DUCKDB.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinaryOperator, EqualsSpelling};
    use crate::precedence::STANDARD_BINDING_POWERS;

    #[test]
    fn duckdb_is_postgres_plus_the_measured_deltas() {
        // The preset is PostgreSQL with a documented set of divergent axes (numeric
        // radix, SELECT surface, expression surface, table expressions, call surface —
        // `COLUMNS(…)` + `TRY_CAST` — type-name surface — the anonymous composite types —
        // the binding-power `->` re-rank, and the keyword reservations); asserting the
        // whole rest equals PostgreSQL keeps the "PG-derived, every delta documented"
        // claim honest against a future stray edit.
        let pg = FeatureSet::POSTGRES;
        let duck = FeatureSet::DUCKDB;
        assert_eq!(duck.numeric_literals, NumericLiteralSyntax::DUCKDB);
        assert_eq!(duck.select_syntax, SelectSyntax::DUCKDB);
        assert_eq!(duck.expression_syntax, ExpressionSyntax::DUCKDB);
        assert_eq!(duck.table_expressions, TableExpressionSyntax::DUCKDB);
        assert_eq!(duck.call_syntax, CallSyntax::DUCKDB);
        assert_eq!(duck.type_name_syntax, TypeNameSyntax::DUCKDB);
        assert_ne!(duck.numeric_literals, pg.numeric_literals);
        assert_ne!(duck.select_syntax, pg.select_syntax);
        assert_ne!(duck.expression_syntax, pg.expression_syntax);
        assert_ne!(duck.table_expressions, pg.table_expressions);
        assert_ne!(duck.call_syntax, pg.call_syntax);
        assert_ne!(duck.type_name_syntax, pg.type_name_syntax);
        assert_eq!(duck.binding_powers, DUCKDB_BINDING_POWERS);
        assert_ne!(duck.numeric_literals, pg.numeric_literals);
        assert_ne!(duck.select_syntax, pg.select_syntax);
        assert_ne!(duck.expression_syntax, pg.expression_syntax);
        assert_ne!(duck.binding_powers, pg.binding_powers);
        assert_eq!(duck.reserved_column_name, DUCKDB_RESERVED_COLUMN_NAME);
        assert_eq!(duck.reserved_function_name, DUCKDB_RESERVED_FUNCTION_NAME);
        assert_eq!(duck.reserved_type_name, DUCKDB_RESERVED_TYPE_NAME);
        assert_eq!(duck.reserved_bare_alias, DUCKDB_RESERVED_BARE_ALIAS);

        // The utility surface adds the DuckDB `PRAGMA`, `USE`, and `CALL` statements plus
        // the `ATTACH`/`DETACH` pair and the DuckDB `CHECKPOINT`/`LOAD`/`RESET`/`DETACH`
        // extensions (`duckdb-settings-and-session-statements`, `duckdb-prepare-execute-call`,
        // `duckdb-utility-checkpoint-detach-load-reset`) over the inherited PostgreSQL
        // `COPY`/`COMMENT ON`/session/`CHECKPOINT`/`LOAD`/prepared-statement-lifecycle base
        // — a purely additive delta, plus the two reverse deltas noted below.
        assert_eq!(duck.utility_syntax, UtilitySyntax::DUCKDB);
        assert_eq!(
            duck.utility_syntax,
            UtilitySyntax {
                pragma: true,
                use_statement: true,
                use_qualified_name: true,
                call: true,
                attach: true,
                load_bare_name: true,
                reset_scope: true,
                detach_if_exists: true,
                // DuckDB's `EXPORT DATABASE`/`IMPORT DATABASE` catalogue round-trip, off in
                // the PostgreSQL base.
                export_import_database: true,
                // DuckDB's `UPDATE EXTENSIONS` extension-refresh statement, off in the
                // PostgreSQL base.
                update_extensions: true,
                // The two flags DuckDB turns *off* relative to PostgreSQL: PostgreSQL's `DO`
                // anonymous code block (no DuckDB equivalent), and the `PREPARE` typed
                // parameter-type list (DuckDB structurally rejects it: "Prepared statement
                // argument types are not supported, use CAST" — probed on 1.5.4). The base
                // `prepared_statements` lifecycle itself (`PREPARE`/`EXECUTE`/`DEALLOCATE`) is
                // inherited unchanged from PostgreSQL, both on.
                do_statement: false,
                prepare_typed_parameters: false,
                // DuckDB 1.5.4 accepts bare `START`, `START WORK`, and a single access
                // mode, but has no savepoint, SET TRANSACTION, isolation/deferrable,
                // repeated-mode, or transaction-chain grammar.
                start_transaction_block_optional: true,
                transaction_savepoints: false,
                set_transaction: false,
                transaction_isolation_mode: false,
                transaction_deferrable_mode: false,
                start_transaction_isolation_mode: false,
                start_transaction_deferrable_mode: false,
                transaction_multiple_modes: false,
                abort_transaction_alias: true,
                end_transaction_alias: true,
                transaction_chain: false,
                ..pg.utility_syntax
            },
        );
        assert_eq!(
            duck.maintenance_syntax,
            MaintenanceSyntax {
                checkpoint_database: true,
                // DuckDB's `VACUUM [ANALYZE] [<table> [(<cols>)]]` / `ANALYZE [<table>
                // [(<cols>)]]` statistics/compaction statements, off in the PostgreSQL base
                // (whose own VACUUM/ANALYZE grammar is unmodelled).
                vacuum_analyze: true,
                analyze: true,
                analyze_columns: true,
                ..pg.maintenance_syntax
            },
        );
        assert_eq!(
            duck.show_syntax,
            ShowSyntax {
                // DuckDB's typed `SHOW [ALL] TABLES [FROM <schema>]` catalogue listing, off
                // in the PostgreSQL base.
                show_tables: true,
                // DuckDB's leading-keyword `{DESCRIBE | SUMMARIZE}` introspection statement,
                // off in the PostgreSQL base.
                describe_summarize: true,
                // DuckDB does not admit PostgreSQL's special `ON` SET value.
                set_value_on_keyword: false,
                set_value_null_keyword: true,
                set_value_reserved_words: DUCKDB_RESERVED_SET_VALUE_WORDS,
                ..pg.show_syntax
            },
        );
        assert!(duck.utility_syntax.pragma && !pg.utility_syntax.pragma);
        assert!(duck.utility_syntax.use_statement && !pg.utility_syntax.use_statement);
        assert!(duck.utility_syntax.prepared_statements && pg.utility_syntax.prepared_statements);
        assert!(
            !duck.utility_syntax.prepare_typed_parameters
                && pg.utility_syntax.prepare_typed_parameters
        );
        assert!(duck.utility_syntax.call && !pg.utility_syntax.call);
        assert!(duck.utility_syntax.attach && !pg.utility_syntax.attach);
        // `checkpoint`/`load_extension` are inherited from PostgreSQL (both on there); the
        // DuckDB operand/argument/scope/guard extensions are the divergence.
        assert!(duck.maintenance_syntax.checkpoint && pg.maintenance_syntax.checkpoint);
        assert!(duck.utility_syntax.load_extension && pg.utility_syntax.load_extension);
        assert!(
            duck.maintenance_syntax.checkpoint_database
                && !pg.maintenance_syntax.checkpoint_database
        );
        assert!(duck.utility_syntax.load_bare_name && !pg.utility_syntax.load_bare_name);
        assert!(duck.utility_syntax.reset_scope && !pg.utility_syntax.reset_scope);
        assert!(duck.utility_syntax.detach_if_exists && !pg.utility_syntax.detach_if_exists);
        assert!(duck.show_syntax.show_tables && !pg.show_syntax.show_tables);
        assert!(duck.show_syntax.describe_summarize && !pg.show_syntax.describe_summarize);
        // `UPDATE EXTENSIONS` is DuckDB-only over the PostgreSQL base.
        assert!(duck.utility_syntax.update_extensions && !pg.utility_syntax.update_extensions);
        // `do_statement` is the reverse divergence: on in PostgreSQL, off in DuckDB.
        assert!(!duck.utility_syntax.do_statement && pg.utility_syntax.do_statement);

        // Everything else is inherited verbatim from PostgreSQL.
        assert_eq!(duck.string_literals, pg.string_literals);
        // Parameters differ in exactly one knob: DuckDB lexes the anonymous `?`
        // placeholder, which PostgreSQL does not (PG uses `$1` only). Every other field is
        // inherited (forcing `anonymous_question` off recovers PG).
        assert!(duck.parameters.anonymous_question);
        assert!(!pg.parameters.anonymous_question);
        assert_eq!(
            ParameterSyntax {
                anonymous_question: false,
                ..duck.parameters
            },
            pg.parameters,
        );
        // The mutation surface differs in the listed DuckDB/PG deltas: PostgreSQL admits
        // data-modifying CTE bodies (which DuckDB parse-rejects, `A CTE needs a
        // SELECT`) and `OVERRIDING` inside a MERGE insert (which DuckDB parse-rejects,
        // `syntax error at or near "OVERRIDING"`) — both probed on 1.5.4 — while DuckDB
        // adds MERGE star/by-name/error actions, INSERT column matching / verb-level
        // conflict actions, parse-time tuple value-row arity checks, and rejects qualified
        // UPDATE SET targets. Everything
        // else — including MERGE, its RETURNING tail, the leading `WITH` before MERGE,
        // the `WHEN NOT MATCHED BY SOURCE/TARGET` arms, and `INSERT DEFAULT VALUES`
        // (all probed accepted on 1.5.4) — is inherited verbatim (forcing the two
        // knobs on recovers PG).
        assert!(!duck.mutation_syntax.data_modifying_ctes);
        assert!(pg.mutation_syntax.data_modifying_ctes);
        assert!(!duck.mutation_syntax.merge_insert_overriding);
        assert!(pg.mutation_syntax.merge_insert_overriding);
        assert!(duck.mutation_syntax.merge_update_set_star);
        assert!(duck.mutation_syntax.merge_insert_star_by_name);
        assert!(duck.mutation_syntax.merge_error_action);
        assert!(!pg.mutation_syntax.merge_update_set_star);
        assert!(duck.mutation_syntax.merge_when_not_matched_by);
        assert!(duck.mutation_syntax.merge_insert_default_values);
        assert!(!duck.mutation_syntax.update_set_qualified_column);
        assert!(pg.mutation_syntax.update_set_qualified_column);
        assert_eq!(
            MutationSyntax {
                data_modifying_ctes: true,
                merge_insert_overriding: true,
                merge_update_set_star: false,
                merge_insert_star_by_name: false,
                merge_error_action: false,
                insert_column_matching: false,
                or_conflict_action: false,
                update_tuple_value_row_arity: false,
                update_set_qualified_column: true,
                ..duck.mutation_syntax
            },
            pg.mutation_syntax,
        );
        // The schema-change surface differs in exactly four knobs: DuckDB enables the
        // live-body macro DDL (`create_macro`), `CREATE OR REPLACE TABLE`
        // (`create_or_replace_table`), the `CREATE [PERSISTENT] SECRET` statement
        // (`create_secret`), and the `CREATE`/`DROP TYPE` user-defined-type DDL
        // (`create_type`), all of which PostgreSQL lacks. Every other field is inherited
        // verbatim (forcing the four off recovers PG).
        assert!(duck.statement_ddl_gates.create_macro);
        assert!(duck.create_table_clause_syntax.create_or_replace_table);
        assert!(duck.statement_ddl_gates.create_secret);
        assert!(duck.statement_ddl_gates.create_type);
        assert!(!pg.statement_ddl_gates.create_macro);
        assert!(!pg.create_table_clause_syntax.create_or_replace_table);
        assert!(!pg.statement_ddl_gates.create_secret);
        assert!(!pg.statement_ddl_gates.create_type);
        // DuckDB matches the PostgreSQL schema-change surface except for the four
        // DuckDB-specific create forms above, the PostgreSQL-only `b_expr` column-default
        // restriction (DuckDB reads a full `a_expr` default), PostgreSQL-only declarative
        // partitioning, the two PostgreSQL-only legacy CREATE TABLE clauses (`INHERITS` and the
        // `(LIKE …)` element), and the four PostgreSQL-only residue clauses (column
        // STORAGE/COMPRESSION, the USING access method, WITHOUT OIDS, typed `OF <type>` tables);
        // DuckDB *does* share the column `COLLATE` and `UNLOGGED` surfaces. Forcing all twelve
        // divergent flags to PostgreSQL's values makes the rest equal.
        assert!(!duck.column_definition_syntax.column_default_requires_b_expr);
        assert!(pg.column_definition_syntax.column_default_requires_b_expr);
        assert!(!duck.create_table_clause_syntax.declarative_partitioning);
        assert!(pg.create_table_clause_syntax.declarative_partitioning);
        assert!(!duck.create_table_clause_syntax.table_inheritance);
        assert!(pg.create_table_clause_syntax.table_inheritance);
        assert!(!duck.create_table_clause_syntax.like_source_table);
        assert!(pg.create_table_clause_syntax.like_source_table);
        assert!(duck.column_definition_syntax.column_collation);
        assert!(duck.create_table_clause_syntax.unlogged_tables);
        assert!(!duck.column_definition_syntax.column_storage);
        assert!(pg.column_definition_syntax.column_storage);
        assert!(!duck.create_table_clause_syntax.table_access_method);
        assert!(pg.create_table_clause_syntax.table_access_method);
        assert!(!duck.create_table_clause_syntax.without_oids);
        assert!(pg.create_table_clause_syntax.without_oids);
        assert!(!duck.create_table_clause_syntax.typed_tables);
        assert!(pg.create_table_clause_syntax.typed_tables);
        // DuckDB also lacks PostgreSQL's SQL-standard embedded schema-element form
        // (`schema_elements`): DuckDB's `CREATE SCHEMA` takes no inline `CREATE TABLE`/…
        // children, so recovering PG forces that flag back on alongside the three
        // DuckDB-specific create forms.
        assert!(!duck.statement_ddl_gates.schema_elements);
        assert!(pg.statement_ddl_gates.schema_elements);
        assert!(!duck.statement_ddl_gates.databases);
        assert!(pg.statement_ddl_gates.databases);
        // DuckDB manages extensions with `INSTALL`/`LOAD`, not the PostgreSQL
        // `CREATE`/`ALTER EXTENSION` catalogue DDL, so recovering PG forces this on.
        assert!(!duck.statement_ddl_gates.extension_ddl);
        assert!(pg.statement_ddl_gates.extension_ddl);
        // DuckDB has no transform catalogue (`CREATE`/`DROP TRANSFORM` is PostgreSQL-only),
        // so recovering PG forces this on.
        assert!(!duck.statement_ddl_gates.transform_ddl);
        assert!(pg.statement_ddl_gates.transform_ddl);
        // DuckDB configures through `SET`/`RESET`, not `ALTER SYSTEM`, so recovering PG
        // forces this on.
        assert!(!duck.statement_ddl_gates.alter_system);
        assert!(pg.statement_ddl_gates.alter_system);
        assert_eq!(
            StatementDdlGates {
                create_macro: false,
                create_secret: false,
                create_type: false,
                schema_elements: true,
                // DuckDB adds `CREATE RECURSIVE VIEW`; PostgreSQL is gated off here.
                recursive_views: false,
                // DuckDB has no `CREATE DATABASE` (uses ATTACH); PostgreSQL admits it.
                databases: true,
                // DuckDB has no PostgreSQL-style extension catalogue DDL.
                extension_ddl: true,
                // DuckDB has no PostgreSQL transform catalogue (`DROP TRANSFORM`).
                transform_ddl: true,
                // DuckDB has no `ALTER SYSTEM` server-configuration DDL.
                alter_system: true,
                // PostgreSQL accepts the `CACHE` sequence option.
                create_sequence_cache: true,
                // MySQL's tablespace / logfile-group storage DDL is not a DuckDB statement.
                tablespace_ddl: false,
                logfile_group_ddl: false,
                // DuckDB adds `ALTER DATABASE … SET ALIAS TO`, the `ALTER SEQUENCE …` option
                // list, and `ALTER … SET SCHEMA`; PostgreSQL is gated off here (no-shadowing).
                alter_database: false,
                alter_sequence: false,
                alter_object_set_schema: false,
                ..duck.statement_ddl_gates
            },
            pg.statement_ddl_gates,
        );
        assert_eq!(
            CreateTableClauseSyntax {
                create_or_replace_table: false,
                declarative_partitioning: true,
                table_inheritance: true,
                like_source_table: true,
                table_access_method: true,
                without_oids: true,
                typed_tables: true,
                create_table_as_execute: true,
                ..duck.create_table_clause_syntax
            },
            pg.create_table_clause_syntax,
        );
        assert_eq!(
            ColumnDefinitionSyntax {
                column_default_requires_b_expr: true,
                column_storage: true,
                // DuckDB turns the keywordless generated-column shorthand and the
                // type-optional generated column on (both off in PostgreSQL).
                generated_column_shorthand: false,
                typeless_generated_columns: false,
                ..duck.column_definition_syntax
            },
            pg.column_definition_syntax,
        );
        assert!(!duck.constraint_syntax.referential_action_cascade_set);
        assert!(pg.constraint_syntax.referential_action_cascade_set);
        assert!(!duck.constraint_syntax.check_constraint_subqueries);
        assert!(pg.constraint_syntax.check_constraint_subqueries);
        assert_eq!(
            ConstraintSyntax {
                exclusion_constraints: true,
                index_constraint_parameters: true,
                referential_action_cascade_set: true,
                check_constraint_subqueries: true,
                ..duck.constraint_syntax
            },
            pg.constraint_syntax,
        );
        assert!(!duck.index_alter_syntax.alter_table_multiple_actions);
        assert!(pg.index_alter_syntax.alter_table_multiple_actions);
        assert!(duck.index_alter_syntax.alter_nested_column_paths);
        assert!(!pg.index_alter_syntax.alter_nested_column_paths);
        assert_eq!(
            IndexAlterSyntax {
                alter_table_multiple_actions: true,
                alter_nested_column_paths: false,
                ..duck.index_alter_syntax
            },
            pg.index_alter_syntax,
        );
        assert_eq!(duck.target_spelling, pg.target_spelling);
        assert_eq!(duck.identifier_casing, pg.identifier_casing);
    }

    #[test]
    fn duckdb_reranks_the_arrow_and_double_equals_tokens() {
        use crate::precedence::Side;

        // The lambda arrow binds below `OR` (the loosest binary operator), while
        // `->>`, containment, and everything else keep PostgreSQL's ranks — the
        // engine-measured split (`x -> x OR y` puts the OR in the body; `a ->> 'k' =
        // 5` compares the extraction).
        let duck = DUCKDB_BINDING_POWERS;
        let arrow = duck.binary(&BinaryOperator::JsonGet);
        assert!(arrow.left < duck.or.left, "`->` is looser than OR");
        assert_eq!(arrow.assoc, Assoc::Left, "`x -> y -> z` groups left");
        // `==` is the second reranked token: DuckDB lexes it as a generic `%left Op`, not
        // the `%nonassoc '='` comparison, so it sits at the `any_operator` rank (tighter
        // than the comparisons, looser than additive, left-associative). Its `=` sibling
        // (`Eq(Single)`) and every other operator keep PostgreSQL's ranks.
        let double_eq = duck.binary(&BinaryOperator::Eq(EqualsSpelling::Double));
        assert_eq!(
            double_eq, duck.any_operator,
            "`==` rides the generic-Op rank"
        );
        assert_eq!(double_eq.assoc, Assoc::Left, "`a == b == c` groups left");
        assert!(
            duck.comparison.left < double_eq.left,
            "`==` binds tighter than the comparisons"
        );
        assert!(
            double_eq.left < duck.additive.left,
            "`==` binds looser than additive"
        );
        for untouched in [
            BinaryOperator::Eq(EqualsSpelling::Single),
            BinaryOperator::JsonGetText,
            BinaryOperator::Contains,
            BinaryOperator::ContainedBy,
        ] {
            assert_eq!(
                duck.binary(&untouched),
                STANDARD_BINDING_POWERS.binary(&untouched),
            );
        }
        // The grouping consequence, at the table level: an `=` right operand of `->`
        // needs no parentheses (it binds tighter), so `x -> x % 2 = 0` keeps the
        // whole comparison in the body — where PostgreSQL's table demands the split.
        assert!(!crate::precedence::needs_parens_between(
            arrow,
            duck.comparison,
            Side::Right,
        ));
    }

    #[test]
    fn duckdb_reserves_qualify_in_every_identifier_position() {
        // DuckDB's `duckdb_keywords()` classes QUALIFY `reserved` (like HAVING): every
        // per-position reject set names it, each strictly widening its shared base —
        // and the shared base itself must NOT contain it, or the "only DuckDB reserves
        // it" story (and PostgreSQL/ANSI identifier behaviour) silently breaks.
        for (duck_set, shared) in [
            (DUCKDB_RESERVED_COLUMN_NAME, RESERVED_COLUMN_NAME),
            (DUCKDB_RESERVED_FUNCTION_NAME, RESERVED_FUNCTION_NAME),
            (DUCKDB_RESERVED_TYPE_NAME, RESERVED_TYPE_NAME),
            (DUCKDB_RESERVED_BARE_ALIAS, RESERVED_BARE_ALIAS),
        ] {
            assert!(duck_set.contains(Keyword::Qualify));
            assert!(!shared.contains(Keyword::Qualify));
        }
        // The type set carries exactly the QUALIFY and PIVOT/UNPIVOT deltas (all
        // `reserved` class) and nothing else — the join words stay out (`CAST(1 AS
        // asof)`/`CAST(1 AS semi)` both parse). The function set carries those *plus*
        // SEMI/ANTI, whose grammar rejects `semi(1)` even though `duckdb_keywords()`
        // classes them `type_function` (the `ASOF`/`POSITIONAL` join words stay out —
        // `asof(1)` parses).
        assert_eq!(
            DUCKDB_RESERVED_FUNCTION_NAME,
            RESERVED_FUNCTION_NAME
                .difference(DUCKDB_UNRESERVED_CARVEOUT)
                .difference(DUCKDB_ORDINARY_SPECIAL_VALUE_NAMES)
                .union(DUCKDB_QUALIFY_RESERVATION)
                .union(DUCKDB_PIVOT_RESERVATION)
                .union(DUCKDB_SEMI_ANTI_JOIN_RESERVATION)
        );
        assert_eq!(
            DUCKDB_RESERVED_TYPE_NAME,
            RESERVED_TYPE_NAME
                .difference(DUCKDB_UNRESERVED_CARVEOUT)
                .difference(DUCKDB_ORDINARY_SPECIAL_VALUE_NAMES)
                .union(DUCKDB_QUALIFY_RESERVATION)
                .union(DUCKDB_PIVOT_RESERVATION)
        );
    }

    #[test]
    fn duckdb_carves_its_unreserved_words_out_of_postgres_reservation() {
        for keyword in [Keyword::Grant, Keyword::User] {
            assert!(DUCKDB_UNRESERVED_CARVEOUT.contains(keyword));
            assert!(!DUCKDB_RESERVED_COLUMN_NAME.contains(keyword));
            assert!(!DUCKDB_RESERVED_FUNCTION_NAME.contains(keyword));
            assert!(!DUCKDB_RESERVED_TYPE_NAME.contains(keyword));
            assert!(!DUCKDB_RESERVED_SET_VALUE_WORDS.contains(keyword));
            assert!(DUCKDB_RESERVED_BARE_ALIAS.contains(keyword));
        }
    }

    #[test]
    fn duckdb_special_value_names_are_ordinary_identifiers() {
        for keyword in [
            Keyword::CurrentCatalog,
            Keyword::CurrentDate,
            Keyword::CurrentRole,
            Keyword::CurrentSchema,
            Keyword::CurrentTime,
            Keyword::CurrentTimestamp,
            Keyword::CurrentUser,
            Keyword::Localtime,
            Keyword::Localtimestamp,
            Keyword::SessionUser,
            Keyword::SystemUser,
        ] {
            assert!(DUCKDB_ORDINARY_SPECIAL_VALUE_NAMES.contains(keyword));
            assert!(!DUCKDB_RESERVED_COLUMN_NAME.contains(keyword));
            assert!(!DUCKDB_RESERVED_FUNCTION_NAME.contains(keyword));
            assert!(!DUCKDB_RESERVED_TYPE_NAME.contains(keyword));
            assert!(!DUCKDB_RESERVED_BARE_ALIAS.contains(keyword));
            assert!(!DUCKDB_RESERVED_SET_VALUE_WORDS.contains(keyword));
        }
    }

    #[test]
    fn duckdb_reserves_pivot_and_unpivot_in_every_identifier_position() {
        // DuckDB's `duckdb_keywords()` classes PIVOT/UNPIVOT `reserved` (like QUALIFY):
        // every per-position reject set names them (probed on 1.5.4), each strictly
        // widening its shared base — and the shared base must NOT contain them, or the
        // "only DuckDB reserves them" story (and PostgreSQL/ANSI identifier behaviour)
        // silently breaks.
        for kw in [Keyword::Pivot, Keyword::Unpivot] {
            for (duck_set, shared) in [
                (DUCKDB_RESERVED_COLUMN_NAME, RESERVED_COLUMN_NAME),
                (DUCKDB_RESERVED_FUNCTION_NAME, RESERVED_FUNCTION_NAME),
                (DUCKDB_RESERVED_TYPE_NAME, RESERVED_TYPE_NAME),
                (DUCKDB_RESERVED_BARE_ALIAS, RESERVED_BARE_ALIAS),
            ] {
                assert!(duck_set.contains(kw));
                assert!(!shared.contains(kw));
            }
        }
    }

    #[test]
    fn duckdb_reserves_the_join_words_as_colid_and_bare_alias_only() {
        // `asof`/`positional` are `duckdb_keywords()` class `type_function` (like
        // `CROSS`): rejected as a column/table name and bare alias, admitted as a
        // function name, type name, and `AS` label. The set composition mirrors that
        // probed profile exactly, and the shared bases must stay free of both words
        // (every other dialect keeps them plain identifiers).
        for kw in [Keyword::Asof, Keyword::Positional] {
            assert!(DUCKDB_RESERVED_COLUMN_NAME.contains(kw));
            assert!(DUCKDB_RESERVED_BARE_ALIAS.contains(kw));
            assert!(!DUCKDB_RESERVED_FUNCTION_NAME.contains(kw));
            assert!(!DUCKDB_RESERVED_TYPE_NAME.contains(kw));
            for shared in [
                RESERVED_COLUMN_NAME,
                RESERVED_FUNCTION_NAME,
                RESERVED_TYPE_NAME,
                RESERVED_BARE_ALIAS,
            ] {
                assert!(!shared.contains(kw));
            }
        }
        assert_eq!(
            DUCKDB_RESERVED_COLUMN_NAME,
            RESERVED_COLUMN_NAME
                .difference(DUCKDB_UNRESERVED_CARVEOUT)
                .difference(DUCKDB_ORDINARY_SPECIAL_VALUE_NAMES)
                .union(DUCKDB_QUALIFY_RESERVATION)
                .union(DUCKDB_PIVOT_RESERVATION)
                .union(DUCKDB_NONSTANDARD_JOIN_RESERVATION)
                .union(DUCKDB_SEMI_ANTI_JOIN_RESERVATION)
        );
        assert_eq!(
            DUCKDB_RESERVED_BARE_ALIAS,
            RESERVED_BARE_ALIAS
                .difference(DUCKDB_ORDINARY_SPECIAL_VALUE_NAMES)
                .union(DUCKDB_UNRESERVED_BARE_ALIAS_RESERVATION)
                .union(DUCKDB_QUALIFY_RESERVATION)
                .union(DUCKDB_PIVOT_RESERVATION)
                .union(DUCKDB_NONSTANDARD_JOIN_RESERVATION)
                .union(DUCKDB_SEMI_ANTI_JOIN_RESERVATION)
        );
    }

    #[test]
    fn duckdb_reserves_semi_and_anti_as_colid_bare_alias_and_function() {
        // `semi`/`anti` are `duckdb_keywords()` class `type_function` like the join
        // words, but the DuckDB grammar reserves them one position further: rejected as
        // a column/table name, a bare alias, *and a function name* (`semi(1)`
        // syntax-errors where `asof(1)` parses), while the type position and `AS` labels
        // still admit them (`CAST(1 AS semi)`/`SELECT 1 AS semi` parse). The set
        // composition mirrors that probed profile exactly, and the shared bases stay
        // free of both words.
        for kw in [Keyword::Semi, Keyword::Anti] {
            assert!(DUCKDB_RESERVED_COLUMN_NAME.contains(kw));
            assert!(DUCKDB_RESERVED_BARE_ALIAS.contains(kw));
            assert!(DUCKDB_RESERVED_FUNCTION_NAME.contains(kw));
            assert!(!DUCKDB_RESERVED_TYPE_NAME.contains(kw));
            for shared in [
                RESERVED_COLUMN_NAME,
                RESERVED_FUNCTION_NAME,
                RESERVED_TYPE_NAME,
                RESERVED_BARE_ALIAS,
            ] {
                assert!(!shared.contains(kw));
            }
        }
    }

    #[test]
    fn duckdb_expression_deltas_are_additive_over_postgres() {
        // The delta is exactly the collection-literal and `#n` positional-column flags
        // (ExpressionSyntax), the lambda flag (OperatorSyntax), and the COLUMNS(…) flag
        // (CallSyntax); the whole PostgreSQL surface (subscript, ARRAY[…], `::`, …) is kept.
        // The lambda gate additionally depends on the inherited JSON-arrow lexing (`->`
        // must tokenize for the lambda grammar position to ever fire), so pin that
        // inheritance here too. Bind to locals so the const field reads are not flagged by
        // clippy's `assertions_on_constants`.
        let (duck_expr, pg_expr) = (ExpressionSyntax::DUCKDB, ExpressionSyntax::POSTGRES);
        let (duck_op, pg_op) = (OperatorSyntax::DUCKDB, OperatorSyntax::POSTGRES);
        let (duck_call, pg_call) = (CallSyntax::DUCKDB, CallSyntax::POSTGRES);
        let (duck_sf, pg_sf) = (StringFuncForms::DUCKDB, StringFuncForms::POSTGRES);
        let (duck_ag, pg_ag) = (AggregateCallSyntax::DUCKDB, AggregateCallSyntax::POSTGRES);
        assert!(duck_expr.collection_literals);
        assert!(!pg_expr.collection_literals);
        assert!(duck_expr.positional_column);
        assert!(!pg_expr.positional_column);
        assert!(duck_expr.lambda_keyword);
        assert!(!pg_expr.lambda_keyword);
        assert!(duck_expr.relaxed_interval_syntax);
        assert!(!pg_expr.relaxed_interval_syntax);
        assert!(duck_op.lambda_expressions);
        assert!(!pg_op.lambda_expressions);
        assert!(duck_op.double_equals);
        assert!(!pg_op.double_equals);
        assert!(duck_op.integer_divide_slash);
        assert!(!pg_op.integer_divide_slash);
        assert!(duck_call.columns_expression);
        assert!(!pg_call.columns_expression);
        assert!(duck_call.try_cast);
        assert!(!pg_call.try_cast);
        assert!(
            duck_op.json_arrow_operators,
            "lambda `->` rides the JSON-arrow lexeme"
        );
        // `field_wildcard` is a *subtractive* delta: DuckDB parses `(struct).field`
        // (field_selection, inherited) but has no `.*` value-expansion production, so it
        // vacates PostgreSQL's `true` (engine-probed 1.5.4).
        assert!(pg_expr.field_wildcard);
        assert!(!duck_expr.field_wildcard);
        // `multidim_array_literals` is a *subtractive* delta too: DuckDB nests `ARRAY[[1,2]]`
        // through `collection_literals` (a top-level `[…]` list is a value there and levels
        // may mix), so it vacates PostgreSQL's `true` for the multidim `array_expr` production.
        assert!(pg_expr.multidim_array_literals);
        assert!(!duck_expr.multidim_array_literals);
        assert_eq!(
            duck_expr,
            ExpressionSyntax {
                collection_literals: true,
                slice_step: true,
                positional_column: true,
                lambda_keyword: true,
                relaxed_interval_syntax: true,
                field_wildcard: false,
                multidim_array_literals: false,
                ..pg_expr
            },
        );
        // The PostgreSQL `jsonb` operators are a *subtractive* delta: DuckDB spells `?` as the
        // anonymous placeholder, which claims the same trigger as the `jsonb` `?` operators
        // (`LexicalConflict::JsonbKeyExistsVersusAnonymousParameter`), so DuckDB vacates the
        // whole family to stay lexically consistent — unlike the additive deltas above.
        assert!(pg_op.jsonb_operators);
        assert!(!duck_op.jsonb_operators);
        // `caret_operator` (top-level FeatureSet dimension) is SHARED with PostgreSQL (probed
        // identical on DuckDB 1.5.4 — see the preset comment): DuckDB's `^` is power at the
        // same precedence row, so both presets read `CaretOperator::Exponent`.
        // `custom_operators` is SHARED with PostgreSQL: DuckDB inherits its generalized
        // maximal-munch operator lexer and parse-accepts the same `Op`-class runs (bind-rejecting
        // the ones with no backing function). The one lexical divergence — DuckDB drops `#`/`?`
        // from the `Op` charset (positional-column and parameter sigils) — is carried by the
        // shared `is_operator_char` gate, not a separate flag. See the preset comment and
        // `duckdb-pg-operator-spelling-under-acceptance` for the probe.
        assert!(pg_op.custom_operators);
        assert!(duck_op.custom_operators);
        assert_eq!(FeatureSet::DUCKDB.caret_operator, CaretOperator::Exponent);
        assert_eq!(
            FeatureSet::DUCKDB.caret_operator,
            FeatureSet::POSTGRES.caret_operator
        );
        // The any-operator quantifier (`3 * ANY(list)`) is a *subtractive* delta: it is a
        // PostgreSQL extension, not a DuckDB construct, so DuckDB vacates PostgreSQL's `true`.
        assert!(pg_op.quantified_arbitrary_operator);
        assert!(!duck_op.quantified_arbitrary_operator);
        // Postfix symbolic operators (`10!`) are an *additive* delta: DuckDB keeps the postfix
        // reading PostgreSQL removed in 14, so it arms PostgreSQL's `false`
        // (`duckdb-postfix-operator-dimension`).
        assert!(!pg_op.postfix_operators);
        assert!(duck_op.postfix_operators);
        assert_eq!(
            duck_op,
            OperatorSyntax {
                lambda_expressions: true,
                double_equals: true,
                integer_divide_slash: true,
                starts_with_operator: true,
                jsonb_operators: false,
                quantified_arbitrary_operator: false,
                postfix_operators: true,
                ..pg_op
            },
        );
        // The PG-only SQL/JSON empty-constructor reject is a subtractive delta: DuckDB
        // deliberately keeps `json()` a plain call (unprobed surface, documented at the
        // preset).
        assert!(pg_call.sqljson_constructors_require_argument);
        assert!(!duck_call.sqljson_constructors_require_argument);
        // The SQL/JSON expression functions are a subtractive delta too: DuckDB has no
        // SQL:2016 special forms (only ordinary JSON functions), so it vacates PostgreSQL's
        // `true` to keep the keyword heads plain call/name forms.
        assert!(pg_call.sqljson_expression_functions);
        assert!(!duck_call.sqljson_expression_functions);
        // The SQL/XML expression functions are a subtractive delta too: DuckDB has no
        // SQL/XML special forms, so it vacates PostgreSQL's `true` here as well.
        assert!(pg_call.xml_expression_functions);
        assert!(!duck_call.xml_expression_functions);
        // The string special forms diverge in exactly two probed knobs: the SIMILAR
        // regex substring is dropped from DuckDB's PG-fork grammar, and OVERLAY kept
        // only the PLACING production (no plain-call fallback).
        assert!(pg_sf.substring_similar);
        assert!(!duck_sf.substring_similar);
        assert!(!pg_sf.overlay_requires_placing);
        assert!(duck_sf.overlay_requires_placing);
        assert_eq!(
            duck_call,
            CallSyntax {
                columns_expression: true,
                try_cast: true,
                extract_string_field: true,
                method_chaining: true,
                sqljson_constructors_require_argument: false,
                sqljson_expression_functions: false,
                xml_expression_functions: false,
                merge_action_function: false,
                ..pg_call
            },
        );
        assert_eq!(
            duck_ag,
            AggregateCallSyntax {
                null_treatment: true,
                standalone_argument_order_by: true,
                filter_optional_where: true,
                ..pg_ag
            },
        );
        assert_eq!(
            duck_sf,
            StringFuncForms {
                substring_similar: false,
                overlay_requires_placing: true,
                collation_for_expression: false,
                ..pg_sf
            },
        );
    }

    #[test]
    fn duckdb_numeric_surface_relaxes_postgres_trailing_junk_reject() {
        // DuckDB and PostgreSQL now share the *same* radix/separator surface (both model
        // PG 14+ `0x`/`0o`/`0b` and `_` grouping) — the delta is the strictness knob:
        // PostgreSQL rejects trailing junk after a number, DuckDB (probed) lexes it
        // loosely and accepts. Bound to locals so the field reads are runtime asserts.
        let (duck, pg) = (NumericLiteralSyntax::DUCKDB, NumericLiteralSyntax::POSTGRES);
        assert!(duck.hex_integers && duck.octal_integers && duck.binary_integers);
        assert!(duck.underscore_separators);
        assert!(!duck.money_literals);
        // The radix/separator forms are identical; only the reject differs.
        assert_eq!(duck.hex_integers, pg.hex_integers);
        assert_eq!(duck.octal_integers, pg.octal_integers);
        assert_eq!(duck.binary_integers, pg.binary_integers);
        assert_eq!(duck.underscore_separators, pg.underscore_separators);
        assert!(pg.reject_trailing_junk && !duck.reject_trailing_junk);
    }

    #[test]
    fn duckdb_select_surface_is_postgres_modulo_the_documented_deltas() {
        // Subtractive deltas (`empty_target_list`, and the `locking_clauses` family —
        // `locking_clauses` / `key_lock_strengths` / `stacked_locking_clauses` — since
        // DuckDB has no row locking) and additive clauses (`qualify`, the `GROUP BY ALL` /
        // `ORDER BY ALL` modes, `UNION [ALL] BY NAME`, and the FROM-first SELECT order);
        // the rest of the SELECT surface is PostgreSQL's (DISTINCT ON, FETCH FIRST,
        // SELECT INTO, …).
        let (duck, pg) = (SelectSyntax::DUCKDB, SelectSyntax::POSTGRES);
        let (duck_g, pg_g) = (GroupingSyntax::DUCKDB, GroupingSyntax::POSTGRES);
        let (duck_q, pg_q) = (QueryTailSyntax::DUCKDB, QueryTailSyntax::POSTGRES);
        assert!(!duck.empty_target_list);
        assert!(pg.empty_target_list);
        assert!(duck.qualify);
        assert!(!pg.qualify);
        assert!(duck_g.group_by_all && duck_g.order_by_all);
        assert!(!pg_g.group_by_all && !pg_g.order_by_all);
        assert!(duck.from_first);
        assert!(!pg.from_first);
        assert!(!duck_q.locking_clauses);
        assert!(pg_q.locking_clauses);
        assert!(!duck_q.key_lock_strengths && !duck_q.stacked_locking_clauses);
        assert!(pg_q.key_lock_strengths && pg_q.stacked_locking_clauses);
        assert!(duck.union_by_name);
        assert!(!pg.union_by_name);
        assert!(duck.wildcard_modifiers);
        assert!(!pg.wildcard_modifiers);
        assert!(duck.values_rows_require_equal_arity);
        assert!(!pg.values_rows_require_equal_arity);
        assert!(duck_q.limit_percent);
        assert!(!pg_q.limit_percent);
        assert!(duck.alias_string_literals);
        assert!(!pg.alias_string_literals);
        assert_eq!(
            duck,
            SelectSyntax {
                empty_target_list: false,
                qualify: true,
                from_first: true,
                union_by_name: true,
                wildcard_modifiers: true,
                values_rows_require_equal_arity: true,
                alias_string_literals: true,
                bare_alias_string_literals: false,
                trailing_comma: true,
                // DuckDB's prefix colon alias — additive over the PostgreSQL base.
                prefix_colon_alias: true,
                ..pg
            },
        );
        assert_eq!(
            duck_g,
            GroupingSyntax {
                group_by_all: true,
                // PostgreSQL admits the `GROUP BY {DISTINCT | ALL} <items>` grouping-set
                // quantifier; DuckDB does not (its `ALL` is the standalone mode above) —
                // a subtractive delta.
                group_by_set_quantifier: false,
                order_by_all: true,
                ..pg_g
            },
        );
        assert_eq!(
            duck_q,
            QueryTailSyntax {
                locking_clauses: false,
                key_lock_strengths: false,
                stacked_locking_clauses: false,
                using_sample: true,
                limit_percent: true,
                // PG-only raw-parse `WITH TIES` guards — a subtractive delta (DuckDB's own
                // `WITH TIES` validity is unprobed, documented at the preset).
                with_ties_requires_order_by: false,
                ..pg_q
            },
        );
    }
}
