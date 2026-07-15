// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The always-compiled ANSI/standard dialect preset.
//!
//! Other presets derive from this data without making the shared
//! [`FeatureSet`]/[`KeywordSet`]/`ByteClasses` machinery dialect-specific.

use super::keyword::{
    POSTGRES_AS_LABEL_KEYWORDS, POSTGRES_COL_NAME_KEYWORDS, POSTGRES_RESERVED_KEYWORDS,
    POSTGRES_TYPE_FUNC_NAME_KEYWORDS,
};
use super::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, ConstraintSyntax, CreateTableClauseSyntax,
    DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FeatureSet, GroupingSyntax,
    IdentifierQuote, IdentifierSyntax, IndexAlterSyntax, JoinSyntax, Keyword, KeywordOperators,
    KeywordSet, MaintenanceSyntax, MutationSyntax, NullOrdering, NumericLiteralSyntax,
    OperatorSyntax, ParameterSyntax, PipeOperator, PredicateSyntax, QueryTailSyntax,
    STANDARD_BYTE_CLASSES, SelectSyntax, SessionVariableSyntax, ShowSyntax, StatementDdlGates,
    StringFuncForms, StringLiteralSyntax, TableExpressionSyntax, TableFactorSyntax, TargetSpelling,
    TypeNameSyntax, UtilitySyntax,
};
use crate::precedence::{STANDARD_BINDING_POWERS, STANDARD_SET_OPERATION_BINDING_POWERS};

/// Standard `"`-only identifier quoting shared by the ANSI and PostgreSQL presets.
pub const STANDARD_IDENTIFIER_QUOTES: &[IdentifierQuote] = &[IdentifierQuote::Symmetric('"')];

// --- Per-position reject sets (prod-keyword-position-reserved-sets) -----------
//
// PostgreSQL reserves keywords per grammatical position via the four `kwlist.h`
// classes plus the `BARE_LABEL`/`AS_LABEL` axis; these compose the generated
// category bitsets into the reject set each parser gate consults. They are sourced
// from the PostgreSQL category model — the practical, libpg-query-validated one —
// for *both* the ANSI/generic and PostgreSQL presets: the strict SQL:2016 reserved
// list reserves `COUNT`/`COALESCE`/… as identifiers, which would reject the common
// `count(*)` / `coalesce(a, b)` SQL every working dialect accepts, so the generic
// dialect adopts PostgreSQL's position-aware categories (the differential oracle
// confirms the agreement). A dialect wanting strict ANSI reservation overrides
// these fields via a `FeatureDelta`. They live in the always-compiled ANSI module
// because PostgreSQL reuses them verbatim, so they are baseline data, not a
// PostgreSQL-gated cost.
//
// Source of truth (the `dialect-ref` citation convention — see
// docs/dialect-references/manifest.toml): `postgres/kwlist` @
// REL_18_BETA1-3053-g4b0bf0788b0, the four `kwlist.h` categories vendored at
// docs/dialect-references/corpora/postgres/kwlist.h (pin matches
// conformance/corpus/postgres). Cite the manifest id + pin, not a bare URL that
// rots; a version bump there is a deliberate commit that re-cites here.

/// `ColId` reject set (column/table name, FROM/correlation alias, qualifier): a
/// keyword usable as a bare ColId is `unreserved ∪ col_name`, so this rejects
/// `type_func_name ∪ reserved` (e.g. `JOIN`/`LEFT` cannot be a table alias).
pub const RESERVED_COLUMN_NAME: KeywordSet =
    POSTGRES_TYPE_FUNC_NAME_KEYWORDS.union(POSTGRES_RESERVED_KEYWORDS);

/// Keywords PostgreSQL never admits as a generic `func_application` name, on top
/// of the reserved set.
///
/// PostgreSQL's generic call name is `type_function_name` (`unreserved ∪
/// type_func_name`); the `col_name` keywords are admitted as functions *only*
/// through dedicated productions. Those productions split two ways:
///
/// - Some take an ordinary argument list (`coalesce(a, b)`, `greatest`, `least`,
///   `xmlconcat`, `grouping`, `json`, `substring`, `overlay`, `trim`,
///   `normalize`, the `json_object`/`json_array`/`json_scalar`/`json_serialize`
///   builders, `xmlforest`), so parsing them as ordinary calls happens to agree
///   with PostgreSQL — we keep admitting those.
/// - The rest have non-generic argument syntax (`position(x IN y)`,
///   `xmlelement(...)`, the `json_query`/`json_value`/… builders, `treat(x AS
///   ty)`), are the bare type spellings used only in a type position (`int`,
///   `bit`, `numeric`, `interval`, …), or are keywords with no call form at all
///   (`values`, `between`, `setof`, `inout`/`out`, `merge_action`, `operator`,
///   `none`). PostgreSQL rejects `kw(1)` for every one of these; admitting them
///   as ordinary calls is exactly the divergence the keyword-position oracle
///   pinned, so they are reserved here. `nullif` is *not* listed: its dedicated
///   `NULLIF(a, b)` production is enforced by a parser arity check instead, so
///   the valid two-argument form keeps parsing.
pub const POSTGRES_NON_GENERIC_FUNCTION_KEYWORDS: KeywordSet = KeywordSet::from_keywords(&[
    Keyword::Between,
    Keyword::Bigint,
    Keyword::Bit,
    Keyword::Boolean,
    Keyword::Char,
    Keyword::Character,
    Keyword::Dec,
    Keyword::Decimal,
    Keyword::Float,
    Keyword::Inout,
    Keyword::Int,
    Keyword::Integer,
    Keyword::Interval,
    Keyword::JsonExists,
    Keyword::JsonObjectagg,
    Keyword::JsonQuery,
    Keyword::JsonTable,
    Keyword::JsonValue,
    Keyword::MergeAction,
    Keyword::National,
    Keyword::Nchar,
    Keyword::None,
    Keyword::Numeric,
    Keyword::Operator,
    Keyword::Out,
    Keyword::Position,
    Keyword::Precision,
    Keyword::Real,
    Keyword::Setof,
    Keyword::Smallint,
    Keyword::Time,
    Keyword::Timestamp,
    Keyword::Treat,
    Keyword::Values,
    Keyword::Varchar,
    Keyword::Xmlattributes,
    Keyword::Xmlelement,
    Keyword::Xmlexists,
    Keyword::Xmlnamespaces,
    Keyword::Xmlparse,
    Keyword::Xmlpi,
    Keyword::Xmlroot,
    Keyword::Xmlserialize,
    Keyword::Xmltable,
]);

/// Function-name reject set: `reserved ∪` the `col_name`/special keywords that
/// have no generic call form (see `POSTGRES_NON_GENERIC_FUNCTION_KEYWORDS`).
/// PostgreSQL's generic `func_application` admits only `type_function_name`; the
/// `col_name` functions with an ordinary argument list (`coalesce`, …) still parse
/// because they are *not* in the reject set, while the bare type spellings and
/// non-generic-syntax builders are rejected here to match PostgreSQL.
pub const RESERVED_FUNCTION_NAME: KeywordSet =
    POSTGRES_RESERVED_KEYWORDS.union(POSTGRES_NON_GENERIC_FUNCTION_KEYWORDS);

/// Fully reserved words rejected by PostgreSQL's `var_value` production. This is
/// deliberately narrower than [`RESERVED_FUNCTION_NAME`], which also contains keywords
/// whose dedicated function syntax prevents them from being generic call names.
pub const RESERVED_SET_VALUE_WORDS: KeywordSet = POSTGRES_RESERVED_KEYWORDS;

/// Type-name reject set (`type_function_name`): a keyword usable as a type name is
/// `unreserved ∪ type_func_name`, so this rejects `col_name ∪ reserved` — matching
/// PostgreSQL, which rejects `CAST(x AS coalesce)` (`coalesce` is `col_name`).
/// Built-in type spellings are matched contextually before any gate, so this only
/// governs user-defined type names.
pub const RESERVED_TYPE_NAME: KeywordSet =
    POSTGRES_COL_NAME_KEYWORDS.union(POSTGRES_RESERVED_KEYWORDS);

/// Bare-label (`BareColLabel`) reject set: the `AS_LABEL` keywords, which cannot be
/// a column alias without `AS`. This is the axis behind `SELECT a over` /
/// `SELECT a filter` rejecting while `SELECT a select` accepts.
pub const RESERVED_BARE_ALIAS: KeywordSet = POSTGRES_AS_LABEL_KEYWORDS;

impl CommentSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        line_comment_hash: false,
        // `\n`-only line-comment termination is the strict SQL-standard baseline and the
        // pre-existing behaviour of every preset; PostgreSQL/DuckDB widen it to `\r` too
        // (`CommentSyntax::POSTGRES`), SQLite/MySQL keep it off.
        line_comment_ends_at_carriage_return: false,
        nested_block_comments: true,
        versioned_comments: None,
        // The strict baseline: an unterminated `/* …` running to EOF is a hard error
        // everywhere but SQLite (`CommentSyntax::SQLITE`).
        unterminated_block_comment_at_eof: false,
    };
}

impl StringLiteralSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        escape_strings: false,
        dollar_quoted_strings: false,
        national_strings: false,
        double_quoted_strings: false,
        backslash_escapes: false,
        unicode_strings: false,
        bit_string_literals: false,
        blob_literals: false,
        charset_introducers: false,
        // The standard requires a newline in the separator between adjacent literals.
        same_line_adjacent_concat: false,
    };
}

impl NumericLiteralSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        hex_integers: false,
        octal_integers: false,
        binary_integers: false,
        underscore_separators: false,
        radix_leading_underscore: false,
        money_literals: false,
        reject_trailing_junk: false,
    };
}

impl ParameterSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        positional_dollar: false,
        positional_dollar_large: false,
        anonymous_question: false,
        named_colon: false,
        named_at: false,
        named_dollar: false,
        numbered_question: false,
    };
}

impl SessionVariableSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        user_variables: false,
        system_variables: false,
        variable_assignment: false,
    };
}

impl IdentifierSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        non_ascii: super::NonAsciiIdentifierSyntax::UnicodeAlphanumeric,
        dollar_in_identifiers: false,
        string_literal_identifiers: false,
        empty_quoted_identifiers: false,
    };
}

impl TableExpressionSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        only: false,
        table_sample: false,
        parenthesized_joins: true,
        table_alias_column_lists: true,
        join_using_alias: false,
        // MySQL-only table-factor tails.
        index_hints: false,
        // MSSQL-only `WITH (...)` table hints.
        table_hints: false,
        partition_selection: false,
        base_table_alias_column_lists: true,
        // DuckDB-only string-literal table alias (`FROM t AS 't'('k')`).
        string_literal_aliases: false,
        aliased_parenthesized_join: true,
        // The standard bare table alias is a `ColId`, same as the table name (SQLite is the
        // outlier whose bare alias is the narrower `ids` class that reserves JOIN keywords).
        bare_table_alias_is_bare_label: false,
        // Version / time-travel modifiers are BigQuery/MSSQL/Databricks extensions.
        table_version: false,
        // PartiQL / SUPER table-position JSON paths are Redshift/Snowflake extensions.
        table_json_path: false,
        // SQLite's `INDEXED BY` / `NOT INDEXED` index directive is a SQLite extension.
        indexed_by: false,
    };
}

impl JoinSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        // Standard SQL right-nests stacked join qualifiers (`a JOIN b JOIN c ON p ON q`).
        stacked_join_qualifiers: true,
        full_outer_join: true,
        // SQLite-only `NATURAL CROSS JOIN` (PostgreSQL/DuckDB parse-reject it).
        natural_cross_join: false,
        straight_join: false,
        // DuckDB-only nonstandard joins.
        asof_join: false,
        positional_join: false,
        semi_anti_join: false,
        sided_semi_anti_join: false,
        apply_join: false,
        // The SQL:2023 recursive-query SEARCH/CYCLE clauses stay off in this conservative
        // ANSI-ish baseline (like `unnest`); PostgreSQL/Lenient enable them.
        recursive_search_cycle: false,
        // DuckDB-only parse restriction on a `UNION`-bodied recursive CTE's ORDER BY/LIMIT.
        recursive_union_rejects_order_limit: false,
        // DuckDB-only `USING KEY` recursive-CTE key clause; off in this baseline.
        recursive_using_key: false,
    };
}

impl TableFactorSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        lateral: false,
        table_functions: false,
        rows_from: false,
        // UNNEST is SQL-standard, but this ANSI-ish baseline keeps the table-function
        // surface off (like `table_functions`), so `FROM unnest(…)` is a clean reject.
        unnest: false,
        unnest_with_offset: false,
        table_function_ordinality: false,
        // The standard admits a special value function as a `FROM` source and an alias on a
        // parenthesized join (MySQL is the outlier that rejects both).
        special_function_table_source: true,
        // DuckDB-only PIVOT/UNPIVOT operators.
        pivot: false,
        unpivot: false,
        // DuckDB-only DESCRIBE/SHOW/SUMMARIZE table source.
        show_ref: false,
        // DuckDB-only bare `FROM VALUES (…) AS t` row-list table factor.
        from_values: false,
        // JSON_TABLE / XMLTABLE table factors are PostgreSQL/Lenient-only; off here.
        json_table: false,
        xml_table: false,
        // `TABLE(<expr>)` is a Lenient-only factor (no oracle-backed preset ships it);
        // off in this conservative baseline.
        table_expr_factor: false,
        // The standard PIVOT's extended value sources / `DEFAULT ON NULL` are a
        // BigQuery/Snowflake/Lenient form; off in this conservative baseline.
        pivot_value_sources: false,
        // MATCH_RECOGNIZE is a Snowflake/Oracle form; off in this conservative baseline.
        match_recognize: false,
        // OPENJSON is a SQL Server form; off in this conservative baseline.
        open_json: false,
    };
}

impl MutationSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        insert_ignore: false,
        insert_overwrite: false,
        returning: false,
        on_conflict: false,
        on_duplicate_key_update: false,
        multi_column_assignment: false,
        update_tuple_value_row_arity: false,
        where_current_of: false,
        merge: true,
        // The MySQL `REPLACE` statement and `INSERT ... SET` source are dialect
        // extensions, not standard surface, so the ANSI baseline rejects both.
        replace_into: false,
        insert_set: false,
        // The MySQL `UPDATE`/`DELETE ... ORDER BY ... LIMIT` tails are dialect
        // extensions; standard SQL row-limits neither statement.
        update_delete_tails: false,
        joined_update_delete: false,
        // The SQLite `INSERT OR`/`UPDATE OR <action>` conflict prefix is a SQLite
        // extension; standard SQL has no such verb-level conflict resolution.
        or_conflict_action: false,
        insert_column_matching: false,
        delete_using: true,
        update_from: true,
        // The standard admits an alias on a `DELETE … USING` target and a leading `WITH`
        // before `INSERT` (MySQL rejects both).
        delete_using_target_alias: true,
        cte_before_insert: true,
        // SQL:2016's `<merge statement>` takes no `<with clause>` (unlike `INSERT`,
        // whose source query carries one), so a leading `WITH` before `MERGE` is a
        // PostgreSQL/DuckDB extension the ANSI baseline rejects.
        cte_before_merge: false,
        // The standard's `<with clause>` bodies are query expressions only; the
        // data-modifying CTE is a PostgreSQL extension.
        data_modifying_ctes: false,
        // SQL:2016's `<merge when clause>` is only `MATCHED`/`NOT MATCHED` and its
        // `<merge insert specification>` has no `DEFAULT VALUES` alternative, so both
        // are PostgreSQL/DuckDB extensions the ANSI baseline rejects.
        merge_when_not_matched_by: false,
        merge_insert_default_values: false,
        // The `<override clause>` on a merge insert *is* SQL:2016 standard surface, so
        // the ANSI baseline accepts it (DuckDB is the outlier that rejects it).
        merge_insert_overriding: true,
        merge_insert_multirow: false,
        merge_update_set_star: false,
        merge_insert_star_by_name: false,
        merge_error_action: false,
        update_set_qualified_column: true,
    };
}

impl StatementDdlGates {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        colocation_groups: false,
        materialized_view_to: false,
        // `CREATE TRIGGER`'s only modelled body form is SQLite's, so the standard
        // baseline does not dispatch it.
        create_trigger: false,
        // The macro DDL is DuckDB-only; the standard baseline does not dispatch it.
        create_macro: false,
        create_secret: false,
        // The user-defined-type DDL (`CREATE TYPE`/`DROP TYPE`) is a DuckDB extension here.
        create_type: false,
        // Virtual tables are a SQLite-only concept; the standard baseline does not dispatch
        // `CREATE VIRTUAL TABLE`.
        create_virtual_table: false,
        // T176 sequence generators are an *optional* standard feature modelled via the
        // PostgreSQL/DuckDB presets; the bare-standard baseline does not dispatch `SEQUENCE`.
        create_sequence: false,
        create_sequence_cache: false,
        extension_ddl: false,
        transform_ddl: false,
        alter_system: false,
        // MySQL's tablespace / logfile-group storage DDL has no ANSI equivalent (MySQL turns
        // both on; MySQL derives from this preset).
        tablespace_ddl: false,
        logfile_group_ddl: false,
        schemas: true,
        // ANSI accepts the `CREATE SCHEMA` head but not the embedded-element form here:
        // the standard embedding is validated only against the PostgreSQL oracle, so it
        // is gated to PostgreSQL/Lenient rather than widening the reference dialect's
        // accept surface without a differential; a trailing `CREATE`/`GRANT` stays a
        // separate top-level statement under ANSI.
        schema_elements: false,
        databases: true,
        // ANSI has no MySQL `DROP DATABASE`/`DROP SCHEMA` single-name synonym drop.
        drop_database: false,
        materialized_views: true,
        temporary_views: true,
        routines: true,
        or_replace: true,
        // `CREATE RECURSIVE VIEW` is gated to DuckDB/Lenient; the standard baseline
        // leaves `RECURSIVE` unconsumed before the expected `VIEW`.
        recursive_views: false,
        // The standard baseline has no MySQL-style compound-statement routine body.
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
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        table_options: false,
        // The SQLite trailing `WITHOUT ROWID` table option is a dialect extension, not
        // standard surface, so the baseline rejects it.
        without_rowid_table_option: false,
        // The SQLite trailing `STRICT` table option is a dialect extension, not standard
        // surface, so the baseline rejects it.
        strict_table_option: false,
        // `OR REPLACE TABLE` and `CREATE SECRET` are DuckDB-only extensions.
        create_or_replace_table: false,
        storage_parameters: true,
        on_commit: true,
        create_table_as_with_data: true,
        create_table_as_execute: false,
        // Declarative partitioning is a PostgreSQL extension, not standard SQL.
        declarative_partitioning: false,
        // Table inheritance and the LIKE source-table element are PostgreSQL extensions;
        // the statement-level `CREATE TABLE t LIKE src` is a MySQL extension.
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
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        // The keywordless generated-column `AS (…)` shorthand and the SQLite `CREATE
        // TABLE` decorations are dialect extensions, not standard surface.
        generated_column_shorthand: false,
        // The SQLite column-level `ON CONFLICT <resolution>` clause is a dialect
        // extension, not standard surface, so the baseline rejects it.
        column_conflict_resolution_clause: false,
        // A typeless column is a SQLite extension; the standard baseline requires a type,
        // so a column with no type is a clean parse error.
        typeless_column_definitions: false,
        // DuckDB's type-optional-for-generated-columns narrowing is a dialect extension; the
        // standard baseline requires a type on every column, generated or not.
        typeless_generated_columns: false,
        // The SQLite joined `AUTOINCREMENT` attribute is a dialect extension, not standard
        // surface, so the baseline rejects it.
        joined_autoincrement_attribute: false,
        // An `ASC`/`DESC` order on an inline `PRIMARY KEY` is a SQLite extension; the standard
        // baseline leaves the trailing keyword unconsumed and rejects it.
        inline_primary_key_ordering: false,
        // A `CONSTRAINT <name>` prefix on a column `COLLATE` is a SQLite extension; the standard
        // baseline has no column COLLATE at all, so the named wrapper is off.
        named_column_collate_constraint: false,
        identity_columns: true,
        compact_identity_columns: false,
        // The standard accepts a bare (unparenthesized) expression default and a
        // `CONSTRAINT <name>` prefix on any inline column constraint (MySQL restricts both).
        default_expression_requires_parens: false,
        column_default_requires_b_expr: false,
        // Column COLLATE, UNLOGGED, column STORAGE/COMPRESSION, the table USING access method,
        // legacy WITHOUT OIDS, and typed `OF <type>` tables are all dialect extensions absent
        // from standard SQL.
        column_collation: false,
        column_storage: false,
    };
}

impl ConstraintSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        deferrable_constraints: true,
        named_inline_non_check_constraints: true,
        // The standard requires a constraint element after `CONSTRAINT <name>` (SQLite only
        // makes it optional).
        bare_constraint_name: false,
        exclusion_constraints: false,
        constraint_no_inherit_not_valid: false,
        index_constraint_parameters: false,
        constraint_column_collate_order: false,
        referential_action_cascade_set: true,
        check_constraint_subqueries: true,
    };
}

impl IndexAlterSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        rename_constraint: false,
        alter_table_set_options: false,
        drop_primary_key: false,
        alter_column_add_identity: false,
        index_storage_parameters: false,
        drop_behavior: true,
        // ANSI has no MySQL `DROP INDEX … ON <table>` form.
        index_drop_on_table: false,
        index_concurrently: false,
        index_using_method: false,
        partial_index: false,
        index_if_not_exists: true,
        index_nulls_order: true,
        alter_table_extended: true,
        alter_nested_column_paths: false,
        alter_existence_guards: true,
        alter_column_set_data_type: true,
        routine_arg_types: true,
        routine_arg_defaults: true,
        routine_arg_modes: true,
        // The SQL-standard `<language name>` is a bare identifier, not a string constant.
        routine_language_string: false,
        alter_table_multiple_actions: true,
    };
}

impl ExistenceGuards {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        if_exists: false,
        view_if_not_exists: false,
        create_database_if_not_exists: false,
    };
}

impl SelectSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        distinct_on: false,
        select_into: false,
        // Standard SQL requires at least one select item, so a bare `SELECT` is rejected.
        empty_target_list: false,
        // `QUALIFY` is a DuckDB (Teradata-origin) extension, not standard SQL.
        qualify: false,
        // A string literal as a column alias is a MySQL extension; the standard requires
        // an identifier.
        alias_string_literals: false,
        bare_alias_string_literals: false,
        // `UNION [ALL] BY NAME` is a DuckDB name-matched set operation, not standard
        // SQL; `BY` after a set operator is a syntax error here.
        union_by_name: false,
        wildcard_modifiers: false,
        wildcard_replace: false,
        intersect_all: true,
        except_all: true,
        // The standard's qualified asterisk is a non-aliasable `<all fields reference>`; a
        // trailing alias after `t.*` rejects (the ANSI-derived presets inherit this).
        qualified_wildcard_alias: false,
        // FROM-first SELECT (`FROM t SELECT x`, bare `FROM t`) is a DuckDB extension; a
        // leading `FROM` is never a statement start in standard SQL.
        from_first: false,
        // Standard SQL admits a parenthesized query as a compound operand
        // (`(SELECT …) UNION (SELECT …)`, PostgreSQL `select_with_parens`).
        parenthesized_query_operands: true,
        // A ragged VALUES constructor is a DuckDB parse-time reject; the ANSI baseline
        // leaves the arity check to bind, so it accepts one at parse (no oracle forces the
        // strict-baseline reject, and keeping it off makes this a clean DuckDB-only delta).
        values_rows_require_equal_arity: false,
        // The standard query-position VALUES constructor is bare-parenthesized rows.
        values_row_constructor: true,
        // Standard SQL's `AS` projection alias is a `ColLabel` admitting reserved words;
        // only MySQL rejects them there.
        as_alias_rejects_reserved: false,
        // A trailing comma in a list is a DuckDB tolerance; standard SQL rejects the
        // dangling comma.
        trailing_comma: false,
        // DuckDB's prefix colon alias (`SELECT j : 42`, `FROM b : a`) is not standard SQL;
        // a `:` at a select-item / table-factor head is a parse error here.
        prefix_colon_alias: false,
        // Hive/Spark `LATERAL VIEW` is not standard SQL; a post-FROM `LATERAL` is a
        // parse error here.
        lateral_view_clause: false,
        // The Oracle-style `START WITH`/`CONNECT BY` hierarchical query clause is not
        // standard SQL; a post-WHERE `CONNECT BY`/`START WITH` is a parse error here.
        connect_by_clause: false,
    };
}

impl QueryTailSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        fetch_first: true,
        limit_offset_comma: false,
        // A query-tail row-locking clause (`FOR UPDATE`/`FOR SHARE`) is a
        // PostgreSQL/MySQL extension, not a standard SQL query clause.
        locking_clauses: false,
        // The PostgreSQL-only strength refinements and stacked clauses are likewise
        // non-standard; with no base locking clause they never lead here.
        key_lock_strengths: false,
        stacked_locking_clauses: false,
        using_sample: false,
        leading_offset: true,
        limit_expressions: true,
        limit_percent: false,
        with_ties_requires_order_by: false,
        // BigQuery/ZetaSQL `|>` pipe syntax is not standard SQL; a `|>` after a query is a
        // parse error here (and the `|>` token never lexes with the gate off).
        pipe_syntax: false,
        // ClickHouse `LIMIT n BY …` is not standard SQL; a `BY` after `LIMIT` is a
        // parse error here.
        limit_by_clause: false,
        // ClickHouse `SETTINGS …` is not standard SQL; a trailing `SETTINGS` is a parse
        // error here.
        settings_clause: false,
        // ClickHouse `FORMAT …` is not standard SQL; a trailing `FORMAT` is a parse error
        // here.
        format_clause: false,
        // MSSQL `FOR XML`/`FOR JSON` is not standard SQL; a trailing `FOR XML`/`FOR JSON`
        // is a parse error here.
        for_xml_json_clause: false,
    };
}

impl GroupingSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        grouping_sets: true,
        // The trailing `WITH ROLLUP` is a MySQL-only spelling; standard SQL writes the
        // super-aggregate as the `ROLLUP (…)` grouping set above.
        with_rollup: false,
        // Standard SQL sorts only by `ASC`/`DESC`; the operator-driven `USING` sort is
        // a PostgreSQL extension.
        order_by_using: false,
        // `GROUP BY ALL` / `ORDER BY ALL` are DuckDB clause modes, not standard SQL;
        // `ALL` after either keyword pair is a parse error here (the word is reserved).
        group_by_all: false,
        group_by_set_quantifier: false,
        order_by_all: false,
    };
}

impl UtilitySyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
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
        transaction_mode_comma_required: false,
        transaction_modes_unique: false,
        abort_transaction_alias: false,
        end_transaction_alias: false,
        transaction_release: false,
        transaction_chain: true,
        release_savepoint_keyword_optional: true,
        copy: false,
        // `COPY INTO` is Snowflake-specific bulk load/unload — not standard SQL, so the
        // ANSI baseline leaves the surface off (MySQL and the other ANSI-derived presets
        // inherit it off).
        copy_into: false,
        stage_references: false,
        comment_on: false,
        comment_if_exists: false,
        pragma: false,
        attach: false,
        kill: false,
        // MySQL's `HANDLER` low-level cursor family: no ANSI equivalent, so off at the
        // baseline (MySQL turns it on; MySQL derives from this preset).
        handler_statements: false,
        // MySQL's `INSTALL`/`UNINSTALL` `PLUGIN`/`COMPONENT` family: no ANSI equivalent, so off
        // at the baseline (MySQL turns it on; MySQL derives from this preset).
        plugin_component_statements: false,
        // MySQL's server-administration families (SHUTDOWN/RESTART/CLONE/IMPORT TABLE/HELP/
        // BINLOG): no ANSI equivalent, so off at the baseline (MySQL turns each on).
        shutdown: false,
        restart: false,
        clone: false,
        import_table: false,
        help_statement: false,
        binlog: false,
        // MySQL's `CACHE INDEX` / `LOAD INDEX INTO CACHE` MyISAM key-cache pair: no ANSI
        // equivalent, so off at the baseline (MySQL turns it on; MySQL derives from here).
        key_cache_statements: false,
        use_statement: false,
        use_qualified_name: false,
        // DuckDB's `PREPARE`/`EXECUTE`/`DEALLOCATE` and `CALL` statements are not standard
        // SQL, so the ANSI baseline dispatches neither (and MySQL inherits both off).
        prepared_statements: false,
        // The PostgreSQL typed parameter list is a widening of `PREPARE`, which is itself
        // off here; the ANSI baseline leaves it off too (and MySQL inherits it off).
        prepare_typed_parameters: false,
        // MySQL's `PREPARE ... FROM` / `EXECUTE ... USING` / `{DEALLOCATE | DROP} PREPARE`
        // lifecycle: no ANSI equivalent, so off at the baseline (MySQL turns it on).
        prepared_statements_from: false,
        call: false,
        // The `CALL` statement itself is off at the baseline, so its MySQL bare-name widening
        // is off too (and every ANSI-derived preset inherits it off).
        call_bare_name: false,
        load_extension: false,
        load_bare_name: false,
        load_data: false,
        reset_scope: false,
        detach_if_exists: false,
        // `DO` is the PostgreSQL anonymous code block — not standard SQL, so the ANSI
        // baseline leaves the leading keyword undispatched.
        do_statement: false,
        // MySQL's `DO <expr-list>` evaluate-and-discard statement — MySQL-only, so the ANSI
        // baseline leaves it off (and every ANSI-derived preset inherits it off).
        do_expression_list: false,
        // MySQL's `LOCK/UNLOCK {TABLES|TABLE}` per-table locking and its
        // `LOCK INSTANCE FOR BACKUP`/`UNLOCK INSTANCE` backup-lock pair — MySQL-only, so
        // the ANSI baseline leaves both leading keywords undispatched.
        lock_tables: false,
        lock_instance: false,
        // SQLite's `BEGIN {DEFERRED|IMMEDIATE|EXCLUSIVE}` modifier is not standard SQL
        // (the standard/PostgreSQL `BEGIN` takes its own `TransactionMode` list instead),
        // so the ANSI baseline leaves the modifier keyword unrecognized and it falls
        // through to the existing `BEGIN`-body error.
        begin_transaction_mode: false,
        // MySQL's `XA` distributed-transaction family is MySQL-only, so the ANSI baseline
        // leaves the leading `XA` keyword undispatched (every ANSI-derived preset inherits
        // it off unless it opts back in).
        xa_transactions: false,
        // The standalone `RENAME TABLE`/`RENAME USER` statements are MySQL-only, so the
        // ANSI baseline does not dispatch the leading `RENAME` keyword.
        rename_statement: false,
        signal_diagnostics: false,
        // `EXPORT`/`IMPORT DATABASE` are DuckDB catalogue-dump statements, not standard
        // SQL, so the ANSI baseline leaves both leading keywords undispatched (MySQL,
        // Snowflake, and Databricks inherit the pair off).
        export_import_database: false,
        // `UPDATE EXTENSIONS` is DuckDB extension management, not standard SQL, so the ANSI
        // baseline never takes the `EXTENSIONS` lookahead and every `UPDATE` reaches the DML
        // parser (MySQL, Snowflake, and Databricks inherit it off).
        update_extensions: false,
        // MySQL's `FLUSH` / `PURGE BINARY LOGS` server-administration statements — leading
        // keyword gates off in the ANSI baseline (only MySQL/Lenient arm them).
        flush: false,
        purge_binary_logs: false,
        replication_statements: false,
    };
}

impl ShowSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        describe: false,
        describe_summarize: false,
        session_statements: true,
        set_value_reserved_words: RESERVED_SET_VALUE_WORDS,
        set_value_on_keyword: true,
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
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        vacuum: false,
        vacuum_analyze: false,
        reindex: false,
        analyze: false,
        analyze_columns: false,
        // `CHECKPOINT`/`LOAD` are PostgreSQL/DuckDB utility statements and the DuckDB
        // `RESET`-scope / `DETACH … IF EXISTS` extensions are DuckDB-only — none is
        // standard SQL, so the ANSI baseline dispatches/accepts none.
        checkpoint: false,
        checkpoint_database: false,
        // The MySQL admin-table verbs (`ANALYZE/CHECK/CHECKSUM/OPTIMIZE/REPAIR TABLE`) are
        // MySQL-only, so the ANSI baseline dispatches none.
        table_maintenance: false,
    };
}

impl AccessControlSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        alter_role_rename: false,
        access_control: true,
        // The standard admits the schema-scoped grant objects and the `OPTION FOR` prefix.
        access_control_extended_objects: true,
        // The standard has no MySQL account-management DDL (`CREATE USER`, `CREATE ROLE`, …).
        user_role_management: false,
        // The standard uses the typed-object/role-spec grant grammar, not MySQL's account-based one.
        access_control_account_grants: false,
    };
}

impl TypeNameSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        extended_scalar_type_names: false,
        enum_type: false,
        set_type: false,
        numeric_modifiers: false,
        integer_display_width: false,
        composite_types: false,
        // The standard accepts a length-less `VARCHAR` and the zoned temporal types
        // (`TIMESTAMPTZ`, `WITH TIME ZONE`); MySQL requires the length and has no zoned type.
        varchar_requires_length: false,
        zoned_temporal_types: true,
        // Empty `DECIMAL()` parens are a DuckDB spelling; the standard rejects them
        // (`pg_query` rejects `DECIMAL()`).
        empty_type_parens: false,
        // MySQL's `CHARACTER SET`/`ASCII`/`UNICODE`/`BYTE`/`BINARY` type annotation; the
        // standard/PostgreSQL reject it (`pg_query` syntax-errors `CHAR(5) CHARACTER SET x`).
        character_set_annotation: false,
        // A signed `numeric`/`decimal` precision/scale is a PostgreSQL raw-parse laxity; the
        // standard requires an unsigned modifier.
        signed_type_modifier: false,
        // ClickHouse's `Nullable(T)` combinator has no differential oracle; the standard
        // has no such type (its head resolves to a user-defined name here).
        nullable_type: false,
        // ClickHouse's `LowCardinality(T)` combinator likewise has no differential
        // oracle; the standard has no such type.
        low_cardinality_type: false,
        // ClickHouse's `FixedString(N)` constructor likewise has no differential oracle;
        // the standard has no such type.
        fixed_string_type: false,
        // ClickHouse's `DateTime64(P[, 'tz'])` constructor likewise has no differential
        // oracle; the standard has no such type.
        datetime64_type: false,
        // ClickHouse's `Nested(name Type, ...)` composite likewise has no differential
        // oracle; the standard has no such type.
        nested_type: false,
        // ClickHouse's `Int8`…`Int256`/`UInt*` fixed-bit-width integer names likewise have no
        // differential oracle; the standard reads them as user-defined type names.
        bit_width_integer_names: false,
        // SQLite's liberal multi-word / two-argument affinity type name; the standard has a
        // closed type vocabulary, so `LONG INTEGER` / `VARCHAR(123,456)` are parse errors
        // (`pg_query` rejects both).
        liberal_type_names: false,
        string_type_modifiers: false,
        angle_bracket_types: false,
    };
}

impl ExpressionSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        typecast_operator: false,
        subscript: false,
        // DuckDB's three-bound `[lower:upper:step]` slice is a dialect extension.
        slice_step: false,
        collate: false,
        at_time_zone: false,
        semi_structured_access: false,
        array_constructor: false,
        multidim_array_literals: false,
        // The DuckDB `[…]`/`{…}`/`MAP` collection literals are a dialect extension.
        collection_literals: false,
        row_constructor: false,
        // BigQuery's `STRUCT(...)` value constructor is a dialect extension.
        struct_constructor: false,
        field_selection: false,
        field_wildcard: false,
        typed_string_literals: true,
        // The ANSI prefix-typed interval literal (`INTERVAL '1' HOUR TO SECOND`).
        typed_interval_literal: true,
        // DuckDB's relaxed interval spellings are a dialect extension.
        relaxed_interval_syntax: false,
        mysql_interval_operator: false,
        // DuckDB's `#n` positional column reference is a dialect extension.
        positional_column: false,
        lambda_keyword: false,
    };
}

impl OperatorSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        operator_construct: false,
        containment_operators: false,
        json_arrow_operators: false,
        jsonb_operators: false,
        double_equals: false,
        // DuckDB-only `//` spelling.
        integer_divide_slash: false,
        starts_with_operator: false,
        is_general_equality: false,
        // Truth-value tests `IS [NOT] {TRUE|FALSE|UNKNOWN}` are standard SQL (F571).
        truth_value_tests: true,
        // `<=>` is MySQL-only.
        null_safe_equals: false,
        // The single-arrow lambda is DuckDB-only (and `->` does not even lex here).
        lambda_expressions: false,
        // Bitwise `| & ~ << >>` are a shared PostgreSQL/MySQL/SQLite/DuckDB extension, not
        // standard SQL, so the ANSI baseline rejects them.
        bitwise_operators: false,
        quantified_comparisons: true,
        quantified_comparison_lists: false,
        // The standard quantifier admits only the comparison operators; the any-operator
        // extension is PostgreSQL's. MySQL/SQLite inherit this `false`.
        quantified_arbitrary_operator: false,
        // The general PostgreSQL `Op`-class operator surface is a PostgreSQL extension, not
        // standard SQL, so the ANSI baseline rejects it (`^` exponentiation is likewise off,
        // via `caret_operator` on the preset below).
        custom_operators: false,
        null_test_postfix: false,
        // Postfix symbolic operators are a non-standard extension; the ANSI baseline rejects a
        // trailing operator.
        postfix_operators: false,
    };
}

impl CallSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        named_argument: false,
        utc_special_functions: false,
        columns_expression: false,
        extract_from_syntax: true,
        try_cast: false,
        // ANSI/standard `CAST` admits any type name as its target.
        restricted_cast_targets: false,
        // The DuckDB-specific call tails — a quoted `EXTRACT` field, dot-method chaining,
        // and in-parenthesis null-treatment — are dialect extensions, off in the baseline.
        extract_string_field: false,
        method_chaining: false,
        sqljson_constructors_require_argument: false,
        // The SQL/JSON expression functions are modelled against PostgreSQL's raw-parse
        // surface, not verified against the bare ISO grammar, so they stay off for ANSI.
        sqljson_expression_functions: false,
        // The SQL/XML expression functions are likewise modelled against PostgreSQL's
        // raw-parse surface, not the bare ISO grammar, so they stay off for ANSI.
        xml_expression_functions: false,
        variadic_argument: false,
        // `merge_action()` is a PostgreSQL-only support function.
        merge_action_function: false,
        convert_function: false,
    };
}

impl StringFuncForms {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        // The four standard string special forms are core SQL-92/SQL:1999 grammar
        // (E021-06/-09/-11, T312), so ANSI takes their *standard* shapes — like
        // `extract_from_syntax` above: SUBSTRING is FROM-first only (no FOR-leading
        // order), POSITION operands are the symmetric restricted level, OVERLAY is
        // the PLACING form only (the standard defines no plain `overlay` call), and
        // TRIM is the single-source `[side] [chars] FROM src` operand (no
        // PostgreSQL trim_list tails). The PostgreSQL-only SIMILAR/ESCAPE regex
        // substring stays off.
        substring_from_for: true,
        substring_leading_for: false,
        substring_similar: false,
        substring_plain_call_requires_2_or_3_args: false,
        substr_from_for: false,
        position_in: true,
        position_asymmetric_operands: false,
        overlay_placing: true,
        overlay_requires_placing: true,
        trim_from: true,
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
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        // The MySQL `GROUP_CONCAT(... SEPARATOR …)` delimiter and `UTC_*` niladic
        // functions are dialect extensions.
        group_concat_separator: false,
        within_group: true,
        aggregate_filter: true,
        // Standard SQL requires the `WHERE` keyword inside `FILTER (…)`; only DuckDB drops it.
        filter_optional_where: false,
        // Standard SQL admits an aggregate's argument forms regardless of a space before the
        // `(`; only MySQL's `IGNORE_SPACE`-off tokenizer makes the space significant.
        aggregate_args_require_adjacent_paren: false,
        null_treatment: false,
        // The MySQL built-in aggregate/window arity restrictions; ANSI admits an empty
        // call and `OVER` on any function.
        aggregate_calls_reject_empty_arguments: false,
        over_requires_windowable_function: false,
        window_function_tail: false,
        standalone_argument_order_by: false,
    };
}

impl PredicateSyntax {
    /// The `ANSI` predefined value.
    pub const ANSI: Self = Self {
        is_distinct_from: true,
        like: true,
        ilike: false,
        similar_to: false,
        // The standard `OVERLAPS` period predicate stays off in this conservative base
        // (like `similar_to`/`ilike` above); PostgreSQL and Lenient enable it. MySQL and
        // SQLite inherit this `false` (both reject the predicate, engine-probed).
        overlaps_period_predicate: false,
        // The unparenthesized `IN <value>` operator is a DuckDB extension; the standard
        // requires the parentheses.
        unparenthesized_in_list: false,
        // The pattern-match quantifier `LIKE/ILIKE ANY|ALL (array)` is PostgreSQL's.
        // MySQL/SQLite inherit this `false`.
        pattern_match_quantifier: false,
        between_symmetric: false,
        is_normalized: false,
        // The standard `IN` predicate requires at least one list element; an empty `IN ()`
        // is a syntax error. SQLite overrides this to accept it.
        empty_in_list: false,
        // The two-word `<expr> NOT NULL` postfix null test is a SQLite/DuckDB extension; the
        // standard has only `IS NOT NULL`. SQLite and Lenient override this to accept it.
        null_test_two_word_postfix: false,
    };
}

impl FeatureSet {
    /// The generic/standard dialect data.
    pub const ANSI: Self = Self {
        identifier_casing: Casing::Upper,
        identifier_quotes: STANDARD_IDENTIFIER_QUOTES,
        default_null_ordering: NullOrdering::NullsLast,
        reserved_column_name: RESERVED_COLUMN_NAME,
        reserved_function_name: RESERVED_FUNCTION_NAME,
        reserved_type_name: RESERVED_TYPE_NAME,
        reserved_bare_alias: RESERVED_BARE_ALIAS,
        // Standard/PostgreSQL admit every keyword as a `ColLabel` (`SELECT a AS select`).
        reserved_as_label: KeywordSet::EMPTY,
        // Standard relation names are catalog-qualified (`catalog.schema.table`).
        catalog_qualified_names: true,
        byte_classes: STANDARD_BYTE_CLASSES,
        binding_powers: STANDARD_BINDING_POWERS,
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
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
        // `^` has no infix meaning and `#` is not the XOR operator: bitwise XOR (`#`/`^`)
        // and `^` exponentiation are PostgreSQL/MySQL operators, not standard SQL.
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
        select_syntax: SelectSyntax::ANSI,
        query_tail_syntax: QueryTailSyntax::ANSI,
        grouping_syntax: GroupingSyntax::ANSI,
        utility_syntax: UtilitySyntax::ANSI,
        show_syntax: ShowSyntax::ANSI,
        maintenance_syntax: MaintenanceSyntax::ANSI,
        access_control_syntax: AccessControlSyntax::ANSI,
        type_name_syntax: TypeNameSyntax::ANSI,
        // The generic baseline renders its own canonical type spellings (ADR-0011).
        target_spelling: TargetSpelling::Ansi,
    };
}

/// Prefer [`FeatureSet::ANSI`] for struct update.
pub const ANSI: FeatureSet = FeatureSet::ANSI;

// Compile-time proof the ANSI baseline claims no shared tokenizer trigger twice. The
// ratchet must sit where a preset grows: a future edit that adds a contending feature
// fails the build here rather than silently shadowing one meaning (the discipline
// `LENIENT` already carries, applied uniformly).
const _: () = assert!(FeatureSet::ANSI.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: no refinement
// flag rides an unset base, and no two features contend for one parser-position head.
const _: () = assert!(FeatureSet::ANSI.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::ANSI.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::Keyword;
    use super::*;

    #[test]
    fn per_position_sets_are_data_driven_and_position_specific() {
        // `JOIN` is type_func_name: a function/type name but not a bare ColId.
        assert!(RESERVED_COLUMN_NAME.contains(Keyword::Join));
        assert!(!RESERVED_FUNCTION_NAME.contains(Keyword::Join));
        assert!(!RESERVED_TYPE_NAME.contains(Keyword::Join));

        // `COALESCE` is col_name: a column name and (for us) a function, but not a
        // type name.
        assert!(!RESERVED_COLUMN_NAME.contains(Keyword::Coalesce));
        assert!(!RESERVED_FUNCTION_NAME.contains(Keyword::Coalesce));
        assert!(RESERVED_TYPE_NAME.contains(Keyword::Coalesce));

        // `SELECT` is reserved: rejected as every kind of name, yet still a bare
        // label (it is BARE_LABEL, not AS_LABEL).
        assert!(RESERVED_COLUMN_NAME.contains(Keyword::Select));
        assert!(RESERVED_FUNCTION_NAME.contains(Keyword::Select));
        assert!(RESERVED_TYPE_NAME.contains(Keyword::Select));
        assert!(!RESERVED_BARE_ALIAS.contains(Keyword::Select));

        // `OVER`/`FILTER` are unreserved (usable as any name) yet AS_LABEL, so they
        // are the bare-label divergence: not a bare alias, but everything else.
        for keyword in [Keyword::Over, Keyword::Filter] {
            assert!(!RESERVED_COLUMN_NAME.contains(keyword));
            assert!(!RESERVED_FUNCTION_NAME.contains(keyword));
            assert!(!RESERVED_TYPE_NAME.contains(keyword));
            assert!(RESERVED_BARE_ALIAS.contains(keyword));
        }
    }
}
