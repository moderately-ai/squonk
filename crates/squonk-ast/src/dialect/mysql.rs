// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The MySQL dialect preset and its reserved-keyword sets.
//!
//! The module is self-contained for feature gating: a build without the `mysql`
//! cargo feature compiles none of this preset's data and never depends on a gated
//! sibling preset.

use super::keyword::{
    MYSQL_FUNCTION_ONLY_KEYWORDS, MYSQL_RESERVED_KEYWORDS, MYSQL_TYPE_FUNC_NAME_KEYWORDS,
};
use super::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, ConstraintSyntax, CreateTableClauseSyntax,
    DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FeatureSet, GroupingSyntax,
    IdentifierQuote, IdentifierSyntax, IndexAlterSyntax, JoinSyntax, Keyword, KeywordOperators,
    KeywordSet, MYSQL_BYTE_CLASSES, MaintenanceSyntax, MutationSyntax, NullOrdering,
    NumericLiteralSyntax, OperatorSyntax, ParameterSyntax, PipeOperator, PredicateSyntax,
    QueryTailSyntax, SelectSyntax, SessionVariableSyntax, ShowSyntax, StatementDdlGates,
    StringFuncForms, StringLiteralSyntax, TableExpressionSyntax, TableFactorSyntax, TargetSpelling,
    TypeNameSyntax, UtilitySyntax,
};
use crate::precedence::{
    Assoc, BindingPower, BindingPowerTable, STANDARD_SET_OPERATION_BINDING_POWERS,
};

/// MySQL backtick-only identifier quoting. MySQL spells a quoted identifier
/// `` `a` ``; `"a"` is a string under its default (`ANSI_QUOTES`-off) mode, so `"`
/// is deliberately absent here (see [`StringLiteralSyntax::MYSQL`]).
pub const MYSQL_IDENTIFIER_QUOTES: &[IdentifierQuote] = &[IdentifierQuote::Symmetric('`')];

// --- MySQL per-position reject sets (mysql-reserved-word-set) -----------------
//
// MySQL's reserved-word set (MySQL 8.0 manual, transcribed into
// `mysql_keywords.csv`) differs from the shared ANSI/PostgreSQL one in both
// directions: it reserves words PostgreSQL leaves free (`RLIKE`, `DIV`, `XOR`,
// `STRAIGHT_JOIN`, `ZEROFILL`, …) and leaves free words PostgreSQL reserves
// (`OFFSET`, `SYMMETRIC`, …). The shared inventory now carries every MySQL reserved
// word, and these sets reserve them *only* for the MySQL dialect; under
// ANSI/PostgreSQL the same words stay non-reserved (the `token_admissible` gate
// reads the active dialect's set, so a word is an identifier wherever its set
// omits it). MySQL has no PostgreSQL-style four-way class table — it has one
// reserved set plus a grammar carve-out admitting built-in functions as call names
// — so these compose from two generated bitsets the way the PostgreSQL gates
// compose from four: every reserved word is also a `type_func_name` member for the
// non-function positions, while the function position rejects only the fully
// reserved set.

/// MySQL `ColId` reject set (column/table name, correlation alias, qualifier):
/// `type_func_name ∪ reserved`, so `LEFT`/`RLIKE` cannot be a bare column name.
pub const MYSQL_RESERVED_COLUMN_NAME: KeywordSet =
    MYSQL_TYPE_FUNC_NAME_KEYWORDS.union(MYSQL_RESERVED_KEYWORDS);

/// MySQL's 11 dedicated *window* function names as a keyword bitset: `ROW_NUMBER`,
/// `RANK`, `DENSE_RANK`, `PERCENT_RANK`, `CUME_DIST`, `NTILE`, `LEAD`, `LAG`,
/// `FIRST_VALUE`, `LAST_VALUE`, `NTH_VALUE`. These are fully reserved words in MySQL 8.0,
/// yet MySQL admits each as a *call head* through its dedicated window-function grammar
/// (`ROW_NUMBER() OVER (…)` is valid, engine-verified on mysql:8), so they must be carved
/// out of [`MYSQL_RESERVED_FUNCTION_NAME`] below — the one function-call position where a
/// reserved window name is admissible. Every other position (column, type, bare/`AS`
/// alias) keeps rejecting them via [`MYSQL_RESERVED_KEYWORDS`], matching MySQL
/// (`SELECT ROW_NUMBER` bare / `AS row_number` are `ER_PARSE_ERROR`). The string-keyed
/// twin `MYSQL_WINDOW_FUNCTIONS` in the parser crate carries the same 11 names and drives
/// the OVER-required, fixed-arity window-function grammar once the head is admitted.
pub const MYSQL_WINDOW_FUNCTION_KEYWORDS: KeywordSet = KeywordSet::from_keywords(&[
    Keyword::RowNumber,
    Keyword::Rank,
    Keyword::DenseRank,
    Keyword::PercentRank,
    Keyword::CumeDist,
    Keyword::Ntile,
    Keyword::Lead,
    Keyword::Lag,
    Keyword::FirstValue,
    Keyword::LastValue,
    Keyword::NthValue,
]);

/// MySQL function-name reject set: the fully-reserved words *minus* the dedicated
/// window-function names ([`MYSQL_WINDOW_FUNCTION_KEYWORDS`]), *plus* the `function_only`
/// class ([`MYSQL_FUNCTION_ONLY_KEYWORDS`]). The `type_func_name` built-ins (`LEFT`, `IF`,
/// `MOD`, …) are admitted here because MySQL parses `kw(...)` as a call, matching how
/// PostgreSQL admits its `type_func_name` class as function names; the window-function
/// names are admitted for the same reason — MySQL's dedicated window grammar accepts
/// `ROW_NUMBER(…) OVER (…)` — even though they are otherwise fully reserved. The parser
/// then enforces the window grammar's own restrictions (mandatory `OVER`, fixed argument
/// arity) on the admitted head. The `function_only` class (only `array`) is the inverse:
/// a plain identifier in every non-function position, so it is added *only* here — MySQL
/// admits `SELECT 1 AS array` / `SELECT 1 array` but syntax-rejects `array(...)`
/// (engine-verified 1064 on 8.4.10, mysql-reserved-word-set-8-4-over-rejections).
pub const MYSQL_RESERVED_FUNCTION_NAME: KeywordSet = MYSQL_RESERVED_KEYWORDS
    .difference(MYSQL_WINDOW_FUNCTION_KEYWORDS)
    .union(MYSQL_FUNCTION_ONLY_KEYWORDS);

/// MySQL (user-defined) type-name reject set: `type_func_name ∪ reserved`. Built-in
/// type spellings (`INT`, `VARCHAR`, the MySQL `TINYINT`/`UNSIGNED`/… surface) are
/// matched contextually before this gate, so it only governs user-named types,
/// none of which may be a reserved word.
pub const MYSQL_RESERVED_TYPE_NAME: KeywordSet =
    MYSQL_TYPE_FUNC_NAME_KEYWORDS.union(MYSQL_RESERVED_KEYWORDS);

/// MySQL bare-alias reject set: `type_func_name ∪ reserved`. Unlike PostgreSQL —
/// whose `BARE_LABEL`/`AS_LABEL` split lets a reserved word like `SELECT` be a bare
/// alias — MySQL rejects every reserved word as a bare (`AS`-less) alias.
pub const MYSQL_RESERVED_BARE_ALIAS: KeywordSet =
    MYSQL_TYPE_FUNC_NAME_KEYWORDS.union(MYSQL_RESERVED_KEYWORDS);

impl CommentSyntax {
    /// The `MYSQL_VERSION_ID` the fitted preset models for versioned-comment
    /// gating: the ceiling of the MySQL 8.4 LTS series the `mysql:8` oracle image
    /// tracks. Real-world `/*!NNNNN … */` markers name the *released* version a
    /// feature appeared in, so every id the 8.4 line can reach is included
    /// regardless of which 8.4.x patch the oracle runs, while 8.5+/9.x ids are
    /// skipped exactly as the live server skips them (engine-verified:
    /// `/*!80500 … */` and `/*!90000 … */` are discarded on 8.4.10). Pinning the
    /// oracle's exact patch id instead would rot on every image bump; ids in the
    /// unreleased tail of the window (above the running patch, at most `..=80499`)
    /// are the accepted approximation.
    pub const MYSQL_8_VERSION_BOUND: u32 = 80499;

    /// The `MYSQL` preset for comment syntax.
    pub const MYSQL: Self = Self {
        line_comment_hash: true,
        // MySQL ends a `--`/`#` line comment at `\n` only — a `\r` is ordinary comment
        // content (engine-verified against mysql:8: `SELECT 1 -- c\rFROM` is one comment
        // to end-of-line and prepares as `SELECT 1`).
        line_comment_ends_at_carriage_return: false,
        nested_block_comments: false,
        versioned_comments: Some(Self::MYSQL_8_VERSION_BOUND),
        // MySQL rejects an unterminated `/* …` at EOF (engine-verified), unlike SQLite.
        unterminated_block_comment_at_eof: false,
    };
}

impl StringLiteralSyntax {
    /// The `MYSQL` preset for string literal syntax.
    pub const MYSQL: Self = Self {
        escape_strings: false,
        dollar_quoted_strings: false,
        national_strings: true,
        double_quoted_strings: true,
        backslash_escapes: true,
        unicode_strings: false,
        bit_string_literals: true,
        // MySQL's `x'…'`/`X'…'` hexadecimal literal requires an even count of hex digits
        // and syntax-rejects an odd/non-hex body (probed on the live m3 oracle:
        // `ER_PARSE_ERROR` 1064 for `x'ABC'`/`x'XY'`/`x'0'`; `x''` accepts), unlike the
        // deferred bit-string above. With both flags on, the eager blob arm owns the
        // `x`/`X` marker by scan precedence while `B'…'`/`b'…'` stays the deferred binary
        // bit-string — exactly MySQL's split (a `b'…'` body takes any digit count).
        blob_literals: true,
        charset_introducers: true,
        // MySQL concatenates adjacent string literals with any whitespace separator,
        // newline or not (`'a' 'b'` → `'ab'`).
        same_line_adjacent_concat: true,
    };
}

impl NumericLiteralSyntax {
    /// The `MYSQL` preset for numeric literal syntax.
    pub const MYSQL: Self = Self {
        hex_integers: true,
        octal_integers: false,
        binary_integers: true,
        underscore_separators: false,
        radix_leading_underscore: false,
        money_literals: false,
        // MySQL's trailing-junk rule is mixed: it rejects an integer glued to an
        // identifier (`123abc`, `1x`) but accepts a dot-float glued to one (`0.a`,
        // `0.0e1a`) by re-reading the suffix as an alias. A single boolean cannot model
        // that split, so this stays loose until the lexer can express it.
        reject_trailing_junk: false,
    };
}

impl ParameterSyntax {
    /// The `MYSQL` preset for parameter syntax.
    pub const MYSQL: Self = Self {
        positional_dollar: false,
        anonymous_question: true,
        named_colon: false,
        named_at: false,
        // SQLite's `$name`; MySQL has no dollar-named parameter.
        named_dollar: false,
        numbered_question: false,
    };
}

impl SessionVariableSyntax {
    /// The `MYSQL` preset for session variable syntax.
    pub const MYSQL: Self = Self {
        user_variables: true,
        system_variables: true,
        variable_assignment: true,
    };
}

impl IdentifierSyntax {
    /// The `MYSQL` preset for identifier syntax.
    pub const MYSQL: Self = Self {
        non_ascii: super::NonAsciiIdentifierSyntax::Any,
        dollar_in_identifiers: true,
        // MySQL syntax-rejects a string literal in a name position, so the SQLite
        // string-identifier misfeature stays off.
        string_literal_identifiers: false,
        empty_quoted_identifiers: false,
    };
}

impl TableExpressionSyntax {
    /// The `MYSQL` preset for table expression syntax.
    pub const MYSQL: Self = Self {
        only: false,
        table_sample: false,
        parenthesized_joins: true,
        // MySQL admits a FROM table-alias column list on a *derived* table / subquery
        // (`FROM (SELECT …) AS c(x)` parses on mysql:8, only bind-failing) but rejects one
        // on a *base* table (`FROM t AS y(a, b)` is a syntax error). This single knob is not
        // position-aware, so leaving it on keeps the valid derived-table form; the
        // base-table over-acceptance needs a base-vs-derived split and stays pinned for a
        // follow-up.
        table_alias_column_lists: true,
        join_using_alias: false,
        // MySQL index hints and explicit partition selection on a table factor.
        index_hints: true,
        // MSSQL-only `WITH (...)` table hints — off; MySQL has no such tail.
        table_hints: false,
        partition_selection: true,
        // A column-list alias is admitted on a *derived* table / subquery
        // (`table_alias_column_lists` above, on) but NOT on a *base* table: `FROM t AS
        // y(a, b)` is `ER_PARSE_ERROR` on mysql:8 (while `FROM (SELECT …) AS c(x)` parses),
        // so the base-table position is off — the base-vs-derived split.
        base_table_alias_column_lists: false,
        // DuckDB-only string-literal table alias. MySQL accepts a string *column*
        // alias but rejects a string *table* alias, so this stays off here.
        string_literal_aliases: false,
        // MySQL admits a parenthesized join but rejects an alias on it — `(a CROSS JOIN b)
        // AS x` is `ER_PARSE_ERROR` on mysql:8, while the bare group and a derived-table
        // `(SELECT …) AS x` both parse.
        aliased_parenthesized_join: false,
        // MySQL's bare table alias is a `ColId`, not the SQLite `ids` class; its JOIN
        // keywords are reserved as a `ColId` regardless (via the MySQL reserved sets).
        bare_table_alias_is_bare_label: false,
        // MySQL has no table version / time-travel modifier.
        table_version: false,
        // MySQL has no PartiQL / SUPER table-position JSON path.
        table_json_path: false,
        // MySQL has no SQLite `INDEXED BY` / `NOT INDEXED` index directive (it has its own
        // `index_hints`).
        indexed_by: false,
    };
}

impl JoinSyntax {
    /// The `MYSQL` preset for join syntax.
    pub const MYSQL: Self = Self {
        stacked_join_qualifiers: true,
        // MySQL has no `FULL [OUTER] JOIN` — only `LEFT`/`RIGHT` outer joins — so an
        // already-aliased factor followed by `FULL [OUTER] JOIN` is a syntax error
        // (engine-measured-rejected on mysql:8). `FULL` is non-reserved, so a bare
        // `a full JOIN b` still reads `full` as the alias, matching the engine.
        full_outer_join: false,
        // MySQL's `NATURAL` join grammar admits only `LEFT`/`RIGHT`, never `CROSS`.
        natural_cross_join: false,
        straight_join: true,
        // DuckDB-only nonstandard joins.
        asof_join: false,
        positional_join: false,
        semi_anti_join: false,
        sided_semi_anti_join: false,
        apply_join: false,
        // MySQL's recursive CTEs have no SEARCH/CYCLE clauses (a SQL:2023 PostgreSQL form).
        recursive_search_cycle: false,
        // MySQL parse-accepts the modifier; its recursive-part restriction is a resolver check.
        recursive_union_rejects_order_limit: false,
        // `USING KEY` is DuckDB's keyed-recursion clause; MySQL has no such spelling.
        recursive_using_key: false,
    };
}

impl TableFactorSyntax {
    /// The `MYSQL` preset for table factor syntax.
    pub const MYSQL: Self = Self {
        lateral: false,
        table_functions: false,
        rows_from: false,
        // MySQL has no `FROM UNNEST(…)` (it uses `JSON_TABLE`), so the keyword falls
        // through to the named-table path and rejects.
        unnest: false,
        unnest_with_offset: false,
        table_function_ordinality: false,
        // MySQL has no PostgreSQL `func_table` promotion: a bare `current_date`/
        // `current_timestamp` special value function in table position is `ER_PARSE_ERROR`
        // on mysql:8 (those words are reserved), so it falls through to the named-table path
        // where the reserved-word gate rejects it.
        special_function_table_source: false,
        // PIVOT/UNPIVOT are DuckDB-only operators.
        pivot: false,
        unpivot: false,
        // DuckDB-only DESCRIBE/SHOW/SUMMARIZE table source.
        show_ref: false,
        // DuckDB-only bare `FROM VALUES (…) AS t` row-list table factor.
        from_values: false,
        // MySQL has its own `JSON_TABLE` with a different grammar, and no `XMLTABLE`; this
        // PG-shaped surface stays off so it never fires. `JSON_TABLE(` falls to the ordinary
        // function/name path (a MySQL-parity follow-up owns the MySQL grammar).
        json_table: false,
        xml_table: false,
        // `TABLE(<expr>)` is a Snowflake/Oracle form; MySQL has no such factor.
        table_expr_factor: false,
        // The standard PIVOT is a Snowflake/BigQuery/Oracle form; MySQL has no PIVOT.
        pivot_value_sources: false,
        // MATCH_RECOGNIZE is a Snowflake/Oracle form; MySQL has no such factor.
        match_recognize: false,
        // OPENJSON is a SQL Server form; MySQL has no such factor.
        open_json: false,
    };
}

impl MutationSyntax {
    /// The `MYSQL` preset for mutation syntax.
    pub const MYSQL: Self = Self {
        insert_ignore: true,
        insert_overwrite: false,
        returning: false,
        on_conflict: false,
        on_duplicate_key_update: true,
        multi_column_assignment: false,
        update_tuple_value_row_arity: false,
        where_current_of: false,
        merge: false,
        replace_into: true,
        insert_set: true,
        // MySQL admits the single-table `UPDATE`/`DELETE ... ORDER BY ... LIMIT` tails.
        update_delete_tails: true,
        joined_update_delete: true,
        // The SQLite `INSERT OR <action>` prefix is not MySQL: MySQL's own conflict
        // shorthand is a bare post-verb `INSERT IGNORE` (no `OR`), a different surface
        // not modelled here, so the `OR`-prefixed form stays off.
        or_conflict_action: false,
        insert_column_matching: false,
        delete_using: true,
        // MySQL has no `UPDATE … FROM`: it lists the extra tables in the target
        // (`UPDATE t1, t2 SET …`), so `UPDATE t SET … FROM u` is `ER_PARSE_ERROR` on mysql:8.
        update_from: false,
        // MySQL's `DELETE FROM tbl … USING …` names bare delete targets (no alias); an
        // alias on the target is `ER_PARSE_ERROR` on mysql:8 (`DELETE FROM t AS e USING …`),
        // while a plain single-table `DELETE FROM t AS e WHERE …` is fine.
        delete_using_target_alias: false,
        // MySQL admits a leading `WITH` before SELECT/UPDATE/DELETE but not before INSERT
        // (`WITH … INSERT …` is `ER_PARSE_ERROR` on mysql:8; the CTE rides the
        // `INSERT … SELECT` source instead).
        cte_before_insert: false,
        // MySQL has no `MERGE` at all, so the leading-`WITH` gate is moot; off.
        cte_before_merge: false,
        // MySQL CTE bodies are subqueries only — a DML body is `ER_PARSE_ERROR` 1064
        // on mysql:8 (probed).
        data_modifying_ctes: false,
        // MySQL has no `MERGE` at all, so its residual-grammar gates are all moot; off.
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
    /// The `MYSQL` preset for statement ddl gates.
    pub const MYSQL: Self = Self {
        colocation_groups: false,
        materialized_view_to: false,
        // MySQL's `CREATE TRIGGER` body is not the modelled SQLite `BEGIN … END` form.
        create_trigger: false,
        // The macro DDL is DuckDB-specific; MySQL has no `CREATE MACRO`.
        create_macro: false,
        create_secret: false,
        create_type: false,
        // Virtual tables are SQLite-only; MySQL rejects `CREATE VIRTUAL TABLE`.
        create_virtual_table: false,
        // MySQL has no sequence generators (it uses AUTO_INCREMENT); `CREATE SEQUENCE` rejects.
        create_sequence: false,
        create_sequence_cache: false,
        extension_ddl: false,
        transform_ddl: false,
        alter_system: false,
        // MySQL's InnoDB/NDB tablespace and NDB logfile-group storage DDL. Live mysql:8.4.10:
        // every grammar-valid form is grammar-positive (ER_UNSUPPORTED_PS 1295 over the PREPARE
        // oracle — recognized but not preparable).
        tablespace_ddl: true,
        logfile_group_ddl: true,
        schemas: true,
        // MySQL's `CREATE SCHEMA` is a `CREATE DATABASE` synonym with no embedded
        // schema-element grammar (engine-rejected), so the embedding stays off.
        schema_elements: false,
        databases: true,
        // MySQL's `DROP {DATABASE | SCHEMA} [IF EXISTS] <name>` single-database drop —
        // DATABASE and SCHEMA are synonyms, exactly one unqualified name, no CASCADE.
        drop_database: true,
        // MySQL has no materialized views (`CREATE`/`DROP MATERIALIZED VIEW` are
        // engine-measured-rejected on mysql:8), so the keyword pair is left undispatched.
        materialized_views: false,
        // MySQL has temporary *tables* but no temporary *views* — `CREATE TEMPORARY VIEW`
        // is engine-measured-rejected on mysql:8.
        temporary_views: false,
        routines: true,
        or_replace: true,
        // `CREATE RECURSIVE VIEW` is a DuckDB form; MySQL leaves `RECURSIVE`
        // unconsumed before the expected `VIEW`.
        recursive_views: false,
        // MySQL routine/trigger/event bodies are SQL/PSM compound statements
        // (`BEGIN … END` with a `DECLARE` prefix and flow control), parsed by the
        // separate body dispatcher.
        compound_statements: true,
        // MySQL's `ALTER DATABASE` (charset/collation) is a distinct behaviour not yet
        // modelled; DuckDB's alias/sequence/relocation forms stay off here.
        alter_database: false,
        alter_database_options: true,
        server_definition: true,
        alter_instance: true,
        spatial_reference_system: true,
        resource_group: true,
        alter_sequence: false,
        alter_object_set_schema: false,
        // MySQL owns the view definition-option surface: the `ALGORITHM`/`DEFINER`/`SQL
        // SECURITY` prefix on `CREATE VIEW` and the whole `ALTER VIEW` redefinition.
        view_definition_options: true,
    };
}

impl CreateTableClauseSyntax {
    /// The `MYSQL` preset for create table clause syntax.
    pub const MYSQL: Self = Self {
        table_options: true,
        // MySQL has no SQLite trailing `WITHOUT ROWID` table option (rowid storage is an
        // InnoDB internal, not surface syntax).
        without_rowid_table_option: false,
        // MySQL has no SQLite trailing `STRICT` table option (its column-type enforcement is
        // the `STRICT_*` SQL modes, not table surface syntax).
        strict_table_option: false,
        // `OR REPLACE TABLE` and `CREATE SECRET` are DuckDB-specific.
        create_or_replace_table: false,
        // MySQL has no PostgreSQL-style `WITH (<param> = <value>)` storage-parameter
        // clause on `CREATE TABLE` (its table options are bare `<KEY> = <value>` pairs,
        // gated by `table_options`), so the parenthesized form is off
        // (engine-measured-rejected on mysql:8).
        storage_parameters: false,
        // MySQL has no `ON COMMIT {PRESERVE | DELETE} ROWS` temporary-table clause
        // (engine-measured-rejected on mysql:8).
        on_commit: false,
        create_table_as_with_data: false,
        create_table_as_execute: false,
        // MySQL's `PARTITION BY HASH(c) PARTITIONS n` is an unrelated surface; the PostgreSQL
        // declarative form is not accepted.
        declarative_partitioning: false,
        // MySQL has no table inheritance; its `CREATE TABLE t LIKE src` is the distinct
        // statement-level production gated by `statement_level_table_like`, not the PostgreSQL
        // `(LIKE …)` element gated by `like_source_table`.
        table_inheritance: false,
        like_source_table: false,
        statement_level_table_like: true,
        unlogged_tables: false,
        table_access_method: false,
        without_oids: false,
        typed_tables: false,
    };
}

impl ColumnDefinitionSyntax {
    /// The `MYSQL` preset for column definition syntax.
    pub const MYSQL: Self = Self {
        // MySQL spells the keywordless generated-column `AS (…)` shorthand, but has none
        // of the SQLite `CREATE TABLE` decorations (its own `AUTO_INCREMENT` rides
        // `table_options`, not the SQLite flag), so that stays off.
        generated_column_shorthand: true,
        // MySQL has no SQLite column-level `ON CONFLICT <resolution>` clause (its upsert
        // conflict handling is `INSERT … ON DUPLICATE KEY`, a separate surface).
        column_conflict_resolution_clause: false,
        // MySQL requires a data type on every column; the SQLite typeless column is not
        // part of its grammar.
        typeless_column_definitions: false,
        // MySQL requires a type on a generated column too (`x INT AS (…)`); DuckDB's
        // type-optional generated column is not part of its grammar.
        typeless_generated_columns: false,
        // MySQL spells auto-increment as the underscored `AUTO_INCREMENT` attribute (gated by
        // `table_options`), never SQLite's joined `AUTOINCREMENT`, so the joined spelling is off.
        joined_autoincrement_attribute: false,
        // MySQL's inline `PRIMARY KEY` takes no `ASC`/`DESC` order qualifier; the trailing
        // keyword is left unconsumed and rejected.
        inline_primary_key_ordering: false,
        // MySQL has no column `COLLATE` clause (its collation is a distinct attribute grammar), so
        // it never admits the SQLite `CONSTRAINT <name>` prefix on one.
        named_column_collate_constraint: false,
        // MySQL has no SQL-standard `GENERATED … AS IDENTITY` column — it spells
        // auto-numbering with the `AUTO_INCREMENT` attribute (which rides `table_options`),
        // so the `IDENTITY` clause is off (engine-measured-rejected on mysql:8).
        identity_columns: false,
        compact_identity_columns: false,
        // MySQL requires a functional column default to be parenthesized: `DEFAULT UUID()` /
        // `DEFAULT 1 + 2` are `ER_PARSE_ERROR` on mysql:8, while `DEFAULT (UUID())` and the
        // literal / `CURRENT_TIMESTAMP`/`NOW()` forms parse.
        default_expression_requires_parens: true,
        // MySQL admits a `CONSTRAINT <symbol>` name only on an inline `CHECK`; a named inline
        // `REFERENCES`/`UNIQUE`/`PRIMARY KEY`/`NOT NULL` is `ER_PARSE_ERROR` on mysql:8.
        column_default_requires_b_expr: false,
        // Column COLLATE (MySQL spells it via its own `CHARACTER SET … COLLATE …` attribute
        // grammar — a separate surface), UNLOGGED, column STORAGE/COMPRESSION, the table USING
        // access method, WITHOUT OIDS, and typed `OF <type>` tables are all PostgreSQL surfaces
        // MySQL does not spell here.
        column_collation: false,
        column_storage: false,
    };
}

impl ConstraintSyntax {
    /// The `MYSQL` preset for constraint syntax.
    pub const MYSQL: Self = Self {
        deferrable_constraints: false,
        named_inline_non_check_constraints: false,
        // MySQL is not measured to accept a bodyless `CONSTRAINT <name>`; unmodelled, off.
        bare_constraint_name: false,
        exclusion_constraints: false,
        constraint_no_inherit_not_valid: false,
        index_constraint_parameters: false,
        // MySQL's key_part admits ASC/DESC and length prefixes / functional (expr) parts but not
        // COLLATE — a differently-shaped surface with no corpus demand, scoped out rather than
        // modelled as this SQLite-shaped gate.
        constraint_column_collate_order: false,
        referential_action_cascade_set: true,
        check_constraint_subqueries: true,
    };
}

impl IndexAlterSyntax {
    /// The `MYSQL` preset for index alter syntax.
    pub const MYSQL: Self = Self {
        rename_constraint: false,
        alter_table_set_options: false,
        drop_primary_key: true,
        alter_column_add_identity: false,
        index_storage_parameters: false,
        drop_behavior: true,
        // MySQL's `DROP INDEX <name> ON <table> [ALGORITHM …] [LOCK …]` — mandatory ON,
        // online-DDL execution hints.
        index_drop_on_table: true,
        index_concurrently: false,
        index_using_method: false,
        partial_index: false,
        // MySQL rejects `CREATE INDEX IF NOT EXISTS`, index-key `NULLS FIRST`/`LAST`, and a
        // routine argument-type list (`DROP FUNCTION f(INT)`) — each engine-measured
        // `ER_PARSE_ERROR` on mysql:8.
        index_if_not_exists: false,
        index_nulls_order: false,
        alter_table_extended: true,
        // MySQL's extended `ALTER TABLE` (multi-action lists, `ADD`/`DROP CONSTRAINT`,
        // `ALTER COLUMN`) is on via `alter_table_extended`, but it has none of these:
        // `ALTER TABLE IF EXISTS`, `ADD COLUMN IF NOT EXISTS`, `DROP [COLUMN|CONSTRAINT] IF
        // EXISTS` — each `ER_PARSE_ERROR` on mysql:8; and its `ALTER COLUMN` admits only
        // `SET`/`DROP DEFAULT` (type changes go through `MODIFY`/`CHANGE`), so `SET DATA
        // TYPE`/`TYPE`/`SET NOT NULL`/`DROP NOT NULL` are `ER_PARSE_ERROR` too. MySQL also
        // has no deferrable constraints (`… DEFERRABLE`/`INITIALLY DEFERRED`) and no
        // `CREATE TABLE … AS SELECT … WITH [NO] DATA` populate clause — all `ER_PARSE_ERROR`
        // on mysql:8.
        alter_nested_column_paths: false,
        alter_existence_guards: false,
        alter_column_set_data_type: false,
        routine_arg_types: false,
        routine_arg_defaults: false,
        routine_arg_modes: false,
        // MySQL's routine `LANGUAGE` admits only the bare word `SQL`; the string spelling
        // `LANGUAGE 'SQL'` is engine-measured `ER_PARSE_ERROR` (1064) on mysql:8 for both
        // `CREATE FUNCTION` and `CREATE PROCEDURE`.
        routine_language_string: false,
        alter_table_multiple_actions: true,
    };
}

impl ExistenceGuards {
    /// The `MYSQL` preset for existence guards.
    pub const MYSQL: Self = Self {
        if_exists: true,
        view_if_not_exists: false,
        create_database_if_not_exists: true,
    };
}

impl ExpressionSyntax {
    /// The `MYSQL` preset for expression syntax.
    pub const MYSQL: Self = Self {
        typecast_operator: false,
        subscript: false,
        // DuckDB's three-bound `[lower:upper:step]` slice is a dialect extension.
        slice_step: false,
        collate: false,
        at_time_zone: false,
        semi_structured_access: false,
        array_constructor: false,
        multidim_array_literals: false,
        collection_literals: false,
        row_constructor: false,
        struct_constructor: false,
        field_selection: false,
        field_wildcard: false,
        typed_string_literals: true,
        // MySQL has the `DATE`/`TIME`/`TIMESTAMP` typed literals but no first-class
        // interval literal: every prefix-typed `INTERVAL '…'` form — standalone or in a
        // `+`/`-` operand, including the unit-less `INTERVAL '1'` and the ANSI
        // `HOUR TO SECOND`/`SECOND(p)` spellings — is `ER_PARSE_ERROR` on mysql:8.4.10
        // (engine-measured). The only valid MySQL interval is the operator-position
        // `INTERVAL <expr> <unit>` (`mysql_interval_operator` below); the literal path
        // stays off so its declined forms reject.
        typed_interval_literal: false,
        // DuckDB's relaxed interval spellings are a dialect extension.
        relaxed_interval_syntax: false,
        mysql_interval_operator: true,
        // DuckDB's `#n` positional column reference is a dialect extension; MySQL spells
        // `#` a line comment.
        positional_column: false,
        lambda_keyword: false,
    };
}

impl OperatorSyntax {
    /// The `MYSQL` preset for operator syntax.
    pub const MYSQL: Self = Self {
        operator_construct: false,
        containment_operators: false,
        json_arrow_operators: false,
        // MySQL has neither the PostgreSQL `jsonb` operators nor a `#`/`@`-operator surface,
        // and it spells `@@name`/`?` as the system-variable sigil / placeholder, so this
        // stays off (enabling it would contend for the `@@` and `?` triggers).
        jsonb_operators: false,
        double_equals: false,
        // MySQL spells integer division with the `DIV` keyword (via `keyword_operators`),
        // not DuckDB's `//` symbol.
        integer_divide_slash: false,
        starts_with_operator: false,
        is_general_equality: false,
        // Truth-value tests are standard SQL (F571); MySQL 8 accepts all six forms
        // (measured over the wire).
        truth_value_tests: true,
        // MySQL null-safe equality `<=>`.
        null_safe_equals: true,
        // The single-arrow lambda is DuckDB-only. (MySQL's own JSON `->` accessor
        // stays off too — `json_arrow_operators` above — pending its dialect child.)
        lambda_expressions: false,
        // MySQL accepts the bitwise `| & ~ << >>` operators (its own distinct precedence
        // ranks live in `MYSQL_BINDING_POWERS`). Bitwise XOR is its `^` spelling, carried
        // by `caret_operator` on the preset below.
        bitwise_operators: true,
        quantified_comparisons: true,
        quantified_comparison_lists: false,
        // MySQL admits only the comparison operators in the quantifier — no any-operator
        // extension.
        quantified_arbitrary_operator: false,
        // MySQL has no general `Op`-class operator surface (its `^` being bitwise XOR, not
        // exponentiation, is carried by `caret_operator` on the preset below).
        custom_operators: false,
        null_test_postfix: false,
        // MySQL has no postfix operator surface — a trailing symbolic operator rejects.
        postfix_operators: false,
    };
}

impl CallSyntax {
    /// The `MYSQL` preset for call syntax.
    pub const MYSQL: Self = Self {
        named_argument: false,
        utc_special_functions: true,
        columns_expression: false,
        extract_from_syntax: true,
        try_cast: false,
        // MySQL's `CAST`/`CONVERT` target is the narrow `cast_type` set (SIGNED/UNSIGNED,
        // CHAR/BINARY, DATE/DATETIME/TIME, DECIMAL/DOUBLE/FLOAT/REAL, JSON) — not the full
        // column-type vocabulary — so `CAST(x AS INT)`/`AS VARCHAR`/`AS TIMESTAMP`/… are
        // engine-measured parse errors on mysql:8.
        restricted_cast_targets: true,
        // DuckDB-specific call tails; off for MySQL.
        extract_string_field: false,
        method_chaining: false,
        // MySQL has no SQL/JSON `JSON()`/`JSON_SCALAR()`/`JSON_SERIALIZE()` constructor
        // keywords; those names take the ordinary function path.
        sqljson_constructors_require_argument: false,
        // MySQL's JSON functions (`JSON_VALUE`/`JSON_OBJECT`/…) have their OWN grammar,
        // distinct from the SQL:2016 special forms modelled here; keep them ordinary calls.
        sqljson_expression_functions: false,
        // MySQL has no SQL/XML expression functions (`ExtractValue`/`UpdateXML` are ordinary
        // functions); keep the `xml*` names ordinary calls.
        xml_expression_functions: false,
        variadic_argument: false,
        // `merge_action()` is a PostgreSQL-only support function.
        merge_action_function: false,
        convert_function: true,
    };
}

impl StringFuncForms {
    /// The `MYSQL` preset for string func forms.
    pub const MYSQL: Self = Self {
        // The standard string special forms, engine-measured on mysql:8.4: SUBSTRING
        // takes the FROM-first keyword form only (the FOR-leading orders and the
        // SIMILAR regex form are 1064) with a 2-3 plain-call arity floor
        // (`SUBSTRING('a')` is 1064 while a spaced `SUBSTRING ('a')` demotes to the
        // any-arity stored-function path); SUBSTR is a full keyword synonym;
        // POSITION takes MySQL's asymmetric `bit_expr IN expr` operands; OVERLAY
        // does not exist (`overlay(…)` stays an ordinary stored-function-shaped
        // call); TRIM is the restricted single-source form (the PostgreSQL
        // trim_list tails and `trim('a', 'b')` comma form are all 1064). The
        // keyword forms compose with `aggregate_args_require_adjacent_paren` above:
        // a spaced `TRIM (LEADING …)` / `SUBSTRING ('a' FROM 2)` demotes to the
        // generic path exactly as the engine does (both probed 1064 via that path).
        substring_from_for: true,
        substring_leading_for: false,
        substring_similar: false,
        substring_plain_call_requires_2_or_3_args: true,
        substr_from_for: true,
        position_in: true,
        position_asymmetric_operands: true,
        overlay_placing: false,
        overlay_requires_placing: false,
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
        // MySQL's full-text `MATCH (…) AGAINST (…)` special form (this preset's oracle
        // ran the full grammar on mysql:8.4.10). SQLite's infix `MATCH` operator is a
        // separate binding-power entry, unaffected by this prefix-position gate.
        match_against: true,
    };
}

impl AggregateCallSyntax {
    /// The `MYSQL` preset for aggregate call syntax.
    pub const MYSQL: Self = Self {
        group_concat_separator: true,
        within_group: false,
        aggregate_filter: false,
        // MySQL has no aggregate `FILTER` clause at all, so the body-widening is inert.
        filter_optional_where: false,
        // MySQL's default `IGNORE_SPACE`-off tokenizer rejects the aggregate-only argument
        // forms behind a spaced paren (`COUNT ( * )`, `MAX ( ALL 1 )` — engine-measured 1064),
        // while a spaced normal call `count (1)` still parses (binding, not syntax).
        aggregate_args_require_adjacent_paren: true,
        null_treatment: false,
        // MySQL's dedicated aggregate grammar requires an argument (or `COUNT(*)`), so
        // `COUNT()`/`SUM()`/… are `ER_PARSE_ERROR` on mysql:8, while `NOW()`/`UUID()` and
        // empty user-function calls are accepted.
        aggregate_calls_reject_empty_arguments: true,
        // MySQL admits `OVER` only on the aggregate ∪ window functions; `OVER` on a scalar
        // built-in or user function (`PERCENTILE_CONT(x, 0.5) OVER ()`) is `ER_PARSE_ERROR`
        // on mysql:8.
        over_requires_windowable_function: true,
        window_function_tail: true,
        standalone_argument_order_by: false,
    };
}

impl SelectSyntax {
    /// The `MYSQL` preset for select syntax.
    pub const MYSQL: Self = Self {
        distinct_on: false,
        // MySQL has no `SELECT … INTO <table>` create-table form.
        select_into: false,
        // MySQL requires at least one select item, so a bare `SELECT` is rejected.
        empty_target_list: false,
        // MySQL has no `QUALIFY` clause (a DuckDB extension).
        qualify: false,
        // MySQL accepts a string literal as a column alias (`SELECT 1 AS 'x'`).
        alias_string_literals: true,
        // MySQL also reads a bare (`AS`-less) string in projection-alias position as the
        // column name (`SELECT 1 'x'`; engine-measured on mysql:8.4.10). The overlap with
        // same-line adjacent-string concatenation (`SELECT 'a' 'b'` is the single value
        // `'ab'`, not `'a' AS 'b'`) is resolved by parse ordering, not a carve-out flag: a
        // string primary greedily folds every following unprefixed string continuation
        // (`same_line_adjacent_concat`) before the alias parser runs, so a trailing
        // string only reaches the bare-alias branch when the preceding expression was NOT a
        // string (`SELECT 1 'x'` → alias; `SELECT 'a' 'b'` → concat). Engine-measured.
        bare_alias_string_literals: true,
        // `UNION [ALL] BY NAME` is a DuckDB extension; MySQL has no name-matched set
        // operation, so `BY` after a set operator is a syntax error there.
        union_by_name: false,
        wildcard_modifiers: false,
        wildcard_replace: false,
        intersect_all: true,
        except_all: true,
        // MySQL's `table_wild` (`t.*`) is a non-aliasable select-item production; a trailing
        // alias rejects (measured Reject on mysql:8 with the table provisioned).
        qualified_wildcard_alias: false,
        // FROM-first SELECT is a DuckDB extension; MySQL rejects a statement-position
        // `FROM`.
        from_first: false,
        parenthesized_query_operands: true,
        // MySQL accepts a ragged VALUES constructor at parse and rejects it later; the
        // parse-time equal-arity check is a DuckDB-only tightening, so it is off here.
        values_rows_require_equal_arity: false,
        // MySQL's query-position VALUES constructor is `VALUES ROW(1), ROW(2)`; a bare
        // `(…)` row (`VALUES (1)`, `FROM (VALUES (1))`, `VALUES (1) UNION …`) is
        // engine-measured `ER_PARSE_ERROR` on mysql:8, so bare rows are rejected in query
        // position. The `INSERT … VALUES (…)` source list is a separate path, unaffected.
        values_row_constructor: false,
        // MySQL has no PostgreSQL `ColLabel` relaxation: a reserved word (`type_func_name ∪
        // reserved`, the `reserved_bare_alias` set) is rejected as an `AS` projection alias
        // exactly as it is rejected as a bare alias — `SELECT 1 AS range`/`AS left`/`AS
        // delete` are `ER_PARSE_ERROR` on mysql:8, while the non-reserved `SELECT 1 AS any`
        // parses. The dotted-name continuation (`t.range`) stays permissive via the empty
        // `reserved_as_label`; this gate scopes the reservation to the projection `AS` alias.
        as_alias_rejects_reserved: true,
        // A trailing comma in a list is a DuckDB tolerance; MySQL rejects it.
        trailing_comma: false,
        // The prefix colon alias is a DuckDB extension; a `:` at a select-item /
        // table-factor head is a parse error in MySQL.
        prefix_colon_alias: false,
        // Hive/Spark `LATERAL VIEW` is not MySQL; a post-FROM `LATERAL` is a parse
        // error there.
        lateral_view_clause: false,
        // The Oracle-style `START WITH`/`CONNECT BY` hierarchical query clause is not
        // MySQL; a post-WHERE `CONNECT BY`/`START WITH` is a parse error there.
        connect_by_clause: false,
    };
}

impl QueryTailSyntax {
    /// The `MYSQL` preset for query tail syntax.
    pub const MYSQL: Self = Self {
        // MySQL row-limits with `LIMIT`; it has no SQL:2008 `FETCH FIRST … ROWS`
        // spelling (engine-measured-rejected on mysql:8), so the clause is gated off and
        // a leading `FETCH` surfaces as a clean parse error.
        fetch_first: false,
        limit_offset_comma: true,
        // MySQL's `FOR UPDATE`/`FOR SHARE [OF …] [NOWAIT|SKIP LOCKED]` and legacy
        // `LOCK IN SHARE MODE` row-locking tails.
        locking_clauses: true,
        // MySQL's grammar has only `UPDATE`/`SHARE` and exactly one locking clause
        // (engine-verified, mysql-select-tails-locking-hints-partition), so the
        // PostgreSQL-only `NO KEY UPDATE`/`KEY SHARE` strengths and stacked clauses stay
        // off — `FOR NO KEY UPDATE` and a trailing second `FOR …` are parse errors here.
        key_lock_strengths: false,
        stacked_locking_clauses: false,
        using_sample: false,
        // MySQL row-limits with `LIMIT` only: a bare leading `OFFSET` with no preceding
        // `LIMIT` (`SELECT 1 OFFSET 1`, and every `OFFSET … [LIMIT …]`/`OFFSET … ROWS`
        // spelling) is `ER_PARSE_ERROR` on mysql:8, so leading offset is off (like SQLite)
        // and the `OFFSET` keyword surfaces as a clean parse error. The `OFFSET` that
        // *trails* a `LIMIT` (`LIMIT 10 OFFSET 5`) is unaffected — parsed by the `LIMIT`
        // branch, not this gate.
        leading_offset: false,
        // MySQL restricts a `LIMIT`/`OFFSET` count to an unsigned integer literal or a `?`
        // placeholder — `LIMIT 1 + 1` / `LIMIT (SELECT 1)` are engine-measured-rejected on
        // mysql:8 — so arbitrary limit expressions are off.
        limit_expressions: false,
        limit_percent: false,
        with_ties_requires_order_by: false,
        // BigQuery/ZetaSQL `|>` pipe syntax is not MySQL; off here. A `|>` after a query is
        // a parse error, and the token never lexes with the gate off.
        pipe_syntax: false,
        // ClickHouse `LIMIT n BY …` is not MySQL; a `BY` after `LIMIT` is a parse error.
        limit_by_clause: false,
        // ClickHouse `SETTINGS …` is not MySQL; a trailing `SETTINGS` is a parse error.
        settings_clause: false,
        // ClickHouse `FORMAT …` is not MySQL; a trailing `FORMAT` is a parse error.
        format_clause: false,
        // MSSQL `FOR XML`/`FOR JSON` is not MySQL; a trailing `FOR XML`/`FOR JSON` is a
        // parse error (a bare `FOR UPDATE`/`FOR SHARE` locking clause is unaffected).
        for_xml_json_clause: false,
    };
}

impl GroupingSyntax {
    /// The `MYSQL` preset for grouping syntax.
    pub const MYSQL: Self = Self {
        // MySQL has no standard grouping sets; its only grouping surface is the
        // distinct trailing `WITH ROLLUP`. With this off, `ROLLUP (a, b)` in GROUP BY
        // falls through to the expression grammar as an ordinary function call, which
        // is how MySQL resolves it (a stored-function reference).
        grouping_sets: false,
        // MySQL's `GROUP BY <keys> WITH ROLLUP` is its sole grouping-set surface.
        with_rollup: true,
        // MySQL sorts only by `ASC`/`DESC`; `USING <operator>` is PostgreSQL-only.
        order_by_using: false,
        // `GROUP BY ALL` / `ORDER BY ALL` are DuckDB clause modes; MySQL reserves
        // `ALL`, so either spelling is a syntax error there.
        group_by_all: false,
        group_by_set_quantifier: false,
        order_by_all: false,
    };
}

impl UtilitySyntax {
    /// The `MYSQL` preset for utility syntax.
    pub const MYSQL: Self = Self {
        kill: true,
        // MySQL's `HANDLER <t> {OPEN | READ … | CLOSE}` low-level cursor family. Live
        // mysql:8.4.10: all forms grammar-accept (ER_UNSUPPORTED_PS 1295, not preparable over
        // the wire; a bare-connection unqualified `OPEN` is ER_NO_DB_ERROR 1046).
        handler_statements: true,
        // MySQL's `INSTALL`/`UNINSTALL` `PLUGIN`/`COMPONENT` family. Live mysql:8.4.10: `INSTALL
        // PLUGIN … SONAME …` and `UNINSTALL PLUGIN …` prepare; the `COMPONENT` forms grammar-
        // accept as ER_UNSUPPORTED_PS 1295 (not preparable over the wire).
        plugin_component_statements: true,
        // MySQL's server-administration leading-keyword families. Live mysql:8.4.10 (PREPARE
        // oracle): `SHUTDOWN`/`RESTART`/`CLONE`/`IMPORT TABLE`/`HELP` grammar-accept as
        // ER_UNSUPPORTED_PS 1295 (not preparable over the wire); `BINLOG` is preparable, so it
        // PREPAREs a grammar-valid payload (decode/apply happen only at execution).
        shutdown: true,
        restart: true,
        clone: true,
        import_table: true,
        help_statement: true,
        binlog: true,
        // MySQL's `CACHE INDEX` / `LOAD INDEX INTO CACHE` MyISAM key-cache pair. Live
        // mysql:8.4.10: every shape grammar-accepts (PREPAREs); a table list with `PARTITION`,
        // a `PARTITION` after the key list, `IGNORE LEAVES` before the key list, and a trailing
        // `IN <cache>` on `LOAD INDEX` all ER_PARSE_ERROR.
        key_cache_statements: true,
        // MySQL's standalone `RENAME TABLE`/`RENAME USER` object-rename statements — a
        // leading-keyword gate like `kill`. MySQL-only (bar the Lenient superset).
        rename_statement: true,
        signal_diagnostics: true,
        // MySQL's `USE <schema>` catalogue switch. `use_qualified_name` stays off (inherited
        // from ANSI): MySQL's `USE ident` takes a single unqualified schema and
        // `ER_PARSE_ERROR`s any dotted name.
        use_statement: true,
        // MySQL's `DO <expr-list>` evaluate-and-discard statement — a distinct behaviour on
        // the `DO` keyword from PostgreSQL's anonymous code block (`do_statement`, off here).
        do_expression_list: true,
        // MySQL's `PREPARE ... FROM {'text' | @var}` / `EXECUTE ... USING @var` /
        // `{DEALLOCATE | DROP} PREPARE name` lifecycle — a distinct grammar on the same three
        // keywords from DuckDB's typed-`AS` `prepared_statements` (off here). Live mysql:8.4.10:
        // all forms grammar-accept (ER_UNSUPPORTED_PS 1295, not preparable over the wire).
        prepared_statements_from: true,
        // MySQL's `LOCK/UNLOCK {TABLES|TABLE}` per-table lock-kind statements — the MySQL
        // reading of the leading `LOCK` keyword (PostgreSQL's statement-level mode list is a
        // different, unimplemented behaviour with its own future gate). Engine-measured on
        // mysql:8.4.10: the lock kind is mandatory (`LOCK TABLES t1` is 1064) and the pre-8.0
        // `LOW_PRIORITY WRITE` modifier is gone (1064).
        lock_tables: true,
        // MySQL's `LOCK INSTANCE FOR BACKUP`/`UNLOCK INSTANCE` backup-lock pair
        // (both `ER_UNSUPPORTED_PS` under the PREPARE oracle — grammar-positive).
        lock_instance: true,
        // MySQL's `CALL sp_name opt_paren_expr_list` stored-procedure invocation. The
        // parenthesized argument list is *optional* — `CALL p`, `CALL p()`, and `CALL p(1, 2)`
        // all grammar-accept on mysql:8.4.10 (the bare and empty forms resolve to
        // ER_SP_DOES_NOT_EXIST 1305 for an absent routine, a grammar-positive binding reject),
        // so the bare-name widening (`call_bare_name`) rides the base `call` gate here.
        call: true,
        call_bare_name: true,
        // MySQL's `FLUSH [NO_WRITE_TO_BINLOG | LOCAL] <target>` and `PURGE BINARY LOGS {TO
        // '<log>' | BEFORE <expr>}` server-administration statements — leading-keyword gates
        // like `kill`. Live mysql:8.4.10: FLUSH prepares (accept), PURGE grammar-accepts
        // (ER_UNSUPPORTED_PS 1295, not preparable); the removed `HOSTS` target and `PURGE
        // MASTER LOGS` synonym both `ER_PARSE_ERROR`.
        flush: true,
        purge_binary_logs: true,
        replication_statements: true,
        // MySQL's `XA` distributed-transaction family (`XA START/END/PREPARE/COMMIT/ROLLBACK/
        // RECOVER`) — a leading-keyword gate like `kill`. Live mysql:8.4.10: every grammar-valid
        // form is `ER_UNSUPPORTED_PS` 1295 (recognized, not preparable over the wire).
        xa_transactions: true,
        // MySQL's `LOAD {DATA | XML} … INFILE … INTO TABLE …` bulk-import statement — the MySQL
        // reading of the leading `LOAD` keyword (the PostgreSQL/DuckDB `load_extension`
        // shared-library load is a different behaviour, off here; the two dispatch on the
        // `LOAD DATA`/`LOAD XML` two-word lookahead). Engine-measured on mysql:8.4.10: the clause
        // train is strictly order-sensitive (any out-of-order clause is 1064), `FIELDS`/`COLUMNS`
        // and `LINES`/`ROWS` spellings are interchangeable, and every clause parses under both
        // `DATA` and `XML` (the format restrictions are semantic, enforced post-parse).
        load_data: true,
        // Every remaining utility statement head is explicitly pinned below.
        start_transaction: true,
        start_transaction_block_optional: false,
        transaction_work_keyword: true,
        begin_transaction_keyword: false,
        commit_transaction_keyword: false,
        rollback_transaction_keyword: false,
        begin_transaction_modes: false,
        transaction_savepoints: true,
        set_transaction: true,
        transaction_isolation_mode: true,
        transaction_access_mode: true,
        transaction_deferrable_mode: false,
        start_transaction_isolation_mode: false,
        start_transaction_deferrable_mode: false,
        start_transaction_consistent_snapshot: true,
        transaction_multiple_modes: true,
        transaction_mode_comma_required: true,
        transaction_modes_unique: true,
        abort_transaction_alias: false,
        end_transaction_alias: false,
        transaction_release: true,
        transaction_chain: true,
        release_savepoint_keyword_optional: false,
        copy: false,
        copy_into: false,
        stage_references: false,
        comment_on: false,
        comment_if_exists: false,
        pragma: false,
        attach: false,
        use_qualified_name: false,
        prepared_statements: false,
        prepare_typed_parameters: false,
        load_extension: false,
        load_bare_name: false,
        reset_scope: false,
        detach_if_exists: false,
        do_statement: false,
        begin_transaction_mode: false,
        export_import_database: false,
        update_extensions: false,
    };
}

impl ShowSyntax {
    /// The `MYSQL` preset for show syntax.
    pub const MYSQL: Self = Self {
        describe: true,
        // MySQL's `SHOW [EXTENDED] [FULL] TABLES [{FROM|IN} db] [LIKE | WHERE]` — the typed
        // catalogue listing, distinct from the generic session `SHOW <var>` it also has.
        show_tables: true,
        // MySQL's `SHOW [EXTENDED] [FULL] {COLUMNS|FIELDS} {FROM|IN} tbl [{FROM|IN} db]
        // [LIKE | WHERE]` — MySQL-only (DuckDB has no such grammar), so its own gate.
        show_columns: true,
        // MySQL's `SHOW CREATE TABLE tbl` — the DDL that recreates the table. No
        // EXTENDED/FULL modifiers on this subform; MySQL-only (DuckDB has no such grammar),
        // so its own gate. Only TABLE is modelled; the other `SHOW CREATE …` object kinds
        // are deferred to sibling tickets.
        show_create_table: true,
        // MySQL's `SHOW {FUNCTION | PROCEDURE} STATUS [LIKE | WHERE]` stored-routine
        // catalogue listing — a *different* statement from the Spark/Databricks bare `SHOW
        // FUNCTIONS` (`show_functions`, off here), so its own gate. MySQL-only (engine-probed
        // accept on mysql:8; `SHOW FUNCTION STATUS FROM db` and bare `SHOW FUNCTIONS` both
        // `ER_PARSE_ERROR`), off in every other preset.
        show_routine_status: true,
        // MySQL's server-administration / catalogue-introspection `SHOW` family (~40
        // sub-commands: DATABASES, STATUS/VARIABLES, ENGINES, PLUGINS, CREATE VIEW/…,
        // INDEX, GRANTS, WARNINGS/ERRORS, …). One behaviour gate for the whole family —
        // sub-command is DATA on the `ShowTarget` axis, reached by one table-driven
        // dispatch. MySQL-only, off in every other preset (bar the Lenient superset).
        show_admin: true,
        describe_summarize: false,
        session_statements: true,
        show_functions: false,
        show_verbose: false,
    };
}

impl MaintenanceSyntax {
    /// The `MYSQL` preset for maintenance syntax.
    pub const MYSQL: Self = Self {
        // MySQL's admin-table verb family (`ANALYZE/CHECK/CHECKSUM/OPTIMIZE/REPAIR TABLE`).
        // One behaviour gate for the whole family — the verb is DATA on the
        // `TableMaintenanceKind` axis, reached by one table-driven dispatch. MySQL-only,
        // off in every other preset (bar the Lenient superset).
        table_maintenance: true,
        // The non-MySQL maintenance heads remain off.
        vacuum: false,
        vacuum_analyze: false,
        reindex: false,
        analyze: false,
        analyze_columns: false,
        checkpoint: false,
        checkpoint_database: false,
    };
}

impl AccessControlSyntax {
    /// The `MYSQL` preset for access control syntax.
    pub const MYSQL: Self = Self {
        alter_role_rename: false,
        // `show_functions` stays off (from `..ANSI`): MySQL has no bare `SHOW FUNCTIONS`
        // listing. Its `SHOW FUNCTION STATUS [LIKE | WHERE]` is a *different* routine
        // catalogue over `mysql.proc`, carried by the `show_routine_status` gate on
        // `ShowSyntax::MYSQL` (a distinct statement, not an overload of the Spark/Databricks
        // `SHOW FUNCTIONS` this gate governs).
        // MySQL grants/revokes, but not the schema-scoped objects (`ON SCHEMA`/`ON
        // DATABASE`, `ON ALL … IN SCHEMA`) or the `{GRANT|ADMIN} OPTION FOR` prefix — all
        // engine-measured `ER_PARSE_ERROR` on mysql:8 (`SCHEMA`/`DATABASE` are reserved and
        // cannot introduce a priv_level). Only the extended object/prefix surface is off.
        access_control_extended_objects: false,
        // MySQL owns the account-management DDL family this gate names.
        user_role_management: true,
        // MySQL routes GRANT/REVOKE through its account-based grammar (priv-level objects,
        // `user@host` grantees, PROXY grants, `AS … WITH ROLE`, `IF EXISTS`/`IGNORE UNKNOWN USER`).
        access_control_account_grants: true,
        // GRANT and REVOKE remain enabled through the account-oriented grammar.
        access_control: true,
    };
}

impl TypeNameSyntax {
    /// The `MYSQL` preset for type name syntax.
    pub const MYSQL: Self = Self {
        extended_scalar_type_names: true,
        enum_type: true,
        set_type: true,
        numeric_modifiers: true,
        integer_display_width: true,
        composite_types: false,
        // MySQL's `VARCHAR`/`VARBINARY` require an explicit length (`ER_PARSE_ERROR` on
        // mysql:8 without one), unlike the fixed-width `CHAR`/`BINARY` that default to 1.
        varchar_requires_length: true,
        // MySQL has no zoned temporal type: `TIMESTAMPTZ` / `TIMESTAMP WITH TIME ZONE` /
        // `TIMETZ` are `ER_PARSE_ERROR` on mysql:8 (its `TIMESTAMP` carries no zone
        // qualifier).
        zoned_temporal_types: false,
        // MySQL requires a precision inside `DECIMAL(...)`; the empty-paren `DECIMAL()`
        // form is a DuckDB spelling, off here.
        empty_type_parens: false,
        // MySQL's char-family type carries the `opt_charset_with_opt_binary` annotation —
        // `CHARACTER SET x` / `CHARSET x` / `ASCII` / `UNICODE` / `BYTE` / trailing `BINARY`
        // — in both cast-target and column-definition positions (engine-measured on
        // mysql:8.4: `CAST(x AS CHAR(5) CHARACTER SET utf8mb4)`, `CHAR ASCII`, `CHAR(5)
        // BINARY`).
        character_set_annotation: true,
        // MySQL requires an unsigned `DECIMAL` modifier — a negative scale is a syntax error.
        signed_type_modifier: false,
        // ClickHouse's `Nullable(T)` combinator is a no-oracle ClickHouse/Lenient addition;
        // MySQL has no such type.
        nullable_type: false,
        // Same for the sibling `LowCardinality(T)` combinator — ClickHouse/Lenient, no oracle;
        // MySQL has no such type.
        low_cardinality_type: false,
        // Same for `FixedString(N)` — ClickHouse/Lenient, no oracle; MySQL has no such type.
        fixed_string_type: false,
        // Same for `DateTime64(P[, 'tz'])` — ClickHouse/Lenient, no oracle; MySQL has no such type.
        datetime64_type: false,
        // Same for `Nested(name Type, ...)` — ClickHouse/Lenient, no oracle; MySQL has no such type.
        nested_type: false,
        // Same for the `Int8`…`Int256`/`UInt*` bit-width integer names — ClickHouse/Lenient, no
        // oracle; MySQL spells its widths `TINYINT`/`INT`/`BIGINT`, not `Int32`.
        bit_width_integer_names: false,
        // SQLite's liberal multi-word / two-argument affinity type names; MySQL has a closed
        // type vocabulary and rejects `LONG INTEGER` / `VARCHAR(123,456)` (`ER_PARSE_ERROR`).
        liberal_type_names: false,
        string_type_modifiers: false,
        angle_bracket_types: false,
    };
}

/// MySQL binding powers, explicitly enumerated with the engine-measured comparison
/// associativity and bitwise precedence rows.
///
/// **Comparison associativity.** The comparison row (`= <> < <= > >=`, plus
/// `RLIKE`/`REGEXP`, which fold onto it) is `Assoc::Left`, not `Assoc::NonAssoc`: real
/// MySQL parses a comparison chain left-associatively — `1 < 2 < 3` means `(1 < 2) < 3`,
/// the boolean 0/1 result feeding the outer one — where ANSI/PostgreSQL reject the chain.
///
/// **Bitwise ranks (grammar-derived).** MySQL is the dialect that splits the bitwise
/// family across *four* distinct precedences, unlike PostgreSQL/SQLite/DuckDB's single
/// shared rank. Per the MySQL 8.0 operator-precedence manual (tight→loose):
/// `~` (unary) > `^` (XOR) > `* /` > `+ -` > `<< >>` > `&` > `|` > comparison. So `^`
/// binds *tighter than* multiplicative, the shifts sit between additive and `&`, and
/// `|` < `&`. Derived from the manual, not live-probed: a live `mysql:8` oracle should
/// confirm these ranks when one is available. `~` takes the tight [`prefix_sign`](crate::precedence::BindingPowerTable)
/// rank (`80`), matching the manual's "unary minus / bit inversion" row.
pub const MYSQL_BINDING_POWERS: BindingPowerTable = BindingPowerTable {
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
    // `|` < `&` < `<< >>` < additive (`50`); `^` > multiplicative (`60`). Values chosen so
    // each pair's left rank orders correctly against its neighbours; all left-associative.
    bitwise_or: BindingPower {
        left: 42,
        right: 43,
        assoc: Assoc::Left,
    },
    bitwise_and: BindingPower {
        left: 44,
        right: 45,
        assoc: Assoc::Left,
    },
    bitwise_shift: BindingPower {
        left: 47,
        right: 48,
        assoc: Assoc::Left,
    },
    bitwise_xor: BindingPower {
        left: 65,
        right: 66,
        assoc: Assoc::Left,
    },
    // MySQL groups unary `~` with unary minus (the tight sign rank), not the loose
    // PostgreSQL/DuckDB placement.
    prefix_bitwise_not: 80,
    prefix_not: 30,
    prefix_sign: 80,
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
    /// MySQL as dialect data: every parser/tokenizer choice below is read through
    /// [`FeatureSet`] fields, not a dialect-identity branch.
    pub const MYSQL: Self = Self {
        // MySQL columns/aliases compare case-insensitively while preserving the
        // written text; `Lower` models that identity (fold lower, render exact).
        identifier_casing: Casing::Lower,
        // Backtick only: `"` is a *string* under MySQL's default `ANSI_QUOTES`-off
        // mode (see `StringLiteralSyntax::MYSQL`), so it must not also quote idents.
        identifier_quotes: MYSQL_IDENTIFIER_QUOTES,
        // MySQL sorts NULLs first under ascending order (NULL ranks lowest).
        default_null_ordering: NullOrdering::NullsFirst,
        // MySQL's reserved-word set differs from the shared ANSI/PostgreSQL one in
        // both directions (it reserves `RLIKE`/`DIV`/`XOR`/… and frees
        // `OFFSET`/`SYMMETRIC`/…), so it gets its own per-position sets from the MySQL
        // reserved list (mysql-reserved-word-set), pinned toward the 8.4 LTS behaviour the
        // oracle runs: the set-op/sampling keywords `INTERSECT`/`PARALLEL`/`QUALIFY`/
        // `TABLESAMPLE` were added off an m3 sweep, then the three residual 8.4 over-
        // rejections that sweep found were closed (mysql-reserved-word-set-8-4-over-
        // rejections): `array` moved to the `function_only` class (reserved as a call head
        // only), and the removed-in-8.4 `MASTER_BIND`/`MASTER_SSL_VERIFY_SERVER_CERT`
        // replication words were dropped from the list entirely (see the CSV header).
        reserved_column_name: MYSQL_RESERVED_COLUMN_NAME,
        reserved_function_name: MYSQL_RESERVED_FUNCTION_NAME,
        reserved_type_name: MYSQL_RESERVED_TYPE_NAME,
        reserved_bare_alias: MYSQL_RESERVED_BARE_ALIAS,
        // MySQL rejects a reserved word as an `AS`-introduced alias (`SELECT 1 AS range`)
        // but *admits* one in the dotted-name-continuation position (`t.select` /
        // `schema.case` parse — engine-measured on mysql:8.4, only bind-failing; a full
        // 889-keyword m3 sweep confirms every keyword is admitted syntactically after a
        // dot). The two positions are split: the AS-alias projection is tightened by
        // [`SelectSyntax::as_alias_rejects_reserved`] (above) rerouting it to the stricter
        // `reserved_bare_alias`, so `reserved_as_label` governs only the permissive
        // dotted-continuation and stays empty (a non-empty set would over-reject the valid
        // dotted form).
        reserved_as_label: KeywordSet::EMPTY,
        // MySQL relation names are `db.table` — two parts at most; it has no catalog
        // qualifier, so a three-part `a.b.c` in table/index/view position is the syntax
        // error MySQL reports (engine-measured-rejected on mysql:8). Column references reach
        // one part deeper through a separate grammar position and are unaffected.
        catalog_qualified_names: false,
        // The shared M1 table plus the vertical tab (`0x0b`) in the whitespace class:
        // MySQL's tokenizer folds the same flex `space` set `[ \t\n\r\f\v]` as PostgreSQL,
        // and the vertical tab is the one member Rust's `is_ascii_whitespace` (hence
        // `STANDARD_BYTE_CLASSES`) omits. Engine-verified on the live `mysql:8` oracle: a
        // lone `0x0b` prepares as an empty statement and `SELECT\x0b1` prepares as
        // `SELECT 1`, while SQLite/DuckDB fold `0x0b` only position-dependently (their own
        // tables), so full whitespace-class membership rides only this table
        // (see [`MYSQL_BYTE_CLASSES`]). `#` comments, backtick quotes, `&&`, `$`-in-ident,
        // and `?` placeholders all still dispatch from the standard byte classes gated by
        // the knobs below — the vertical tab is the sole byte-class divergence.
        byte_classes: MYSQL_BYTE_CLASSES,
        // MySQL's comparison family is left-associative, not `STANDARD`'s
        // `NonAssoc` (`SELECT a < b < c` is legal MySQL meaning `(a < b) < c`),
        // so it needs its own table (`MYSQL_BINDING_POWERS`) rather than reusing
        // the shared one (ADR-0008; mysql-comparison-operators-are-left-associative).
        binding_powers: MYSQL_BINDING_POWERS,
        set_operation_powers: STANDARD_SET_OPERATION_BINDING_POWERS,
        string_literals: StringLiteralSyntax::MYSQL,
        numeric_literals: NumericLiteralSyntax::MYSQL,
        parameters: ParameterSyntax::MYSQL,
        session_variables: SessionVariableSyntax::MYSQL,
        // `$` as an identifier-continue byte is the same policy as PostgreSQL, but
        // MySQL keeps its own copy so this module never depends on `postgres`.
        identifier_syntax: IdentifierSyntax::MYSQL,
        // MySQL's join grammar adds the `STRAIGHT_JOIN` hint over the ANSI surface, so
        // it needs its own `TableExpressionSyntax` preset (`straight_join: true`)
        // rather than reusing `TableExpressionSyntax::ANSI` directly.
        table_expressions: TableExpressionSyntax::MYSQL,
        join_syntax: JoinSyntax::MYSQL,
        table_factor_syntax: TableFactorSyntax::MYSQL,
        // MySQL's aggregate grammar adds the `GROUP_CONCAT(... SEPARATOR …)` delimiter
        // over the ANSI expression surface, so it needs its own `ExpressionSyntax` preset
        // rather than reusing `ExpressionSyntax::ANSI` directly.
        expression_syntax: ExpressionSyntax::MYSQL,
        operator_syntax: OperatorSyntax::MYSQL,
        call_syntax: CallSyntax::MYSQL,
        string_func_forms: StringFuncForms::MYSQL,
        aggregate_call_syntax: AggregateCallSyntax::MYSQL,
        // MySQL has the standard `LIKE` predicate but neither `ILIKE` nor `SIMILAR TO`.
        predicate_syntax: PredicateSyntax::ANSI,
        pipe_operator: PipeOperator::LogicalOr,
        double_ampersand: DoubleAmpersand::LogicalAnd,
        keyword_operators: KeywordOperators::MySql,
        // MySQL spells bitwise XOR `^` (distinct from the logical `XOR` keyword above);
        // grammar-derived precedence (tighter than `*`) lives in `MYSQL_BINDING_POWERS`.
        caret_operator: CaretOperator::BitwiseXor,
        // MySQL's `#` is a line comment, so the PostgreSQL `#` XOR spelling is rejected.
        hash_bitwise_xor: false,
        comment_syntax: CommentSyntax::MYSQL,
        mutation_syntax: MutationSyntax::MYSQL,
        statement_ddl_gates: StatementDdlGates::MYSQL,
        create_table_clause_syntax: CreateTableClauseSyntax::MYSQL,
        column_definition_syntax: ColumnDefinitionSyntax::MYSQL,
        constraint_syntax: ConstraintSyntax::MYSQL,
        index_alter_syntax: IndexAlterSyntax::MYSQL,
        existence_guards: ExistenceGuards::MYSQL,
        select_syntax: SelectSyntax::MYSQL,
        query_tail_syntax: QueryTailSyntax::MYSQL,
        grouping_syntax: GroupingSyntax::MYSQL,
        // MySQL has no `COPY` (its bulk load is `LOAD DATA`) and none of the SQLite utility
        // statements, but it does have `KILL` and the `DESCRIBE`/`DESC` EXPLAIN synonyms, so
        // it takes its own preset (those two on, the rest off) rather than the ANSI baseline.
        utility_syntax: UtilitySyntax::MYSQL,
        show_syntax: ShowSyntax::MYSQL,
        maintenance_syntax: MaintenanceSyntax::MYSQL,
        access_control_syntax: AccessControlSyntax::MYSQL,
        // The MySQL type-name vocabulary diverges from the shared standard set, so
        // it is recognized as its own gated data rather than reusing it.
        type_name_syntax: TypeNameSyntax::MYSQL,
        // No MySQL-specific Tier-1 output spelling yet: a target-dialect render of
        // MySQL falls back to the portable ANSI canonical spellings, exactly as it did
        // when the renderer only special-cased PostgreSQL.
        target_spelling: TargetSpelling::Ansi,
    };
}

/// Prefer [`FeatureSet::MYSQL`] for struct update.
pub const MYSQL: FeatureSet = FeatureSet::MYSQL;

// Compile-time proof the MySQL preset claims no shared tokenizer trigger twice —
// notably that `user_variables`/`system_variables` (on here) never meet a contending
// `named_at` or containment `<@`, and `double_quoted_strings` never meets a `"` quote.
// The ratchet fails the build if a future edit adds a conflict, rather than silently
// shadowing a meaning (uniform with `LENIENT`'s assert).
const _: () = assert!(FeatureSet::MYSQL.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: every
// refinement flag (`call_bare_name`, the account-grant grammar) rides its enabled base,
// and no two features contend for one parser-position head (`prepared_statements` and
// `do_statement` stay off, so the `FROM`/`USING` lifecycle and `DO` expression list are
// each unrivalled).
const _: () = assert!(FeatureSet::MYSQL.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::MYSQL.has_no_grammar_conflict());

#[cfg(test)]
mod tests {
    use super::super::{
        Keyword, RESERVED_BARE_ALIAS, RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME,
        RESERVED_TYPE_NAME,
    };
    use super::*;
    use crate::ast::{BinaryOperator, EqualsSpelling, RegexpSpelling};
    use crate::precedence::{STANDARD_BINDING_POWERS, Side};

    #[test]
    fn mysql_reserved_sets_diverge_from_the_shared_sets_in_both_directions() {
        // Forward divergence (mysql-reserved-word-set): MySQL 8.0 reserves words the
        // shared ANSI/PostgreSQL model leaves free. They enter the shared inventory
        // but are reserved *only* under MySQL — every position rejects them, and the
        // shared sets do not.
        for keyword in [
            Keyword::Rlike,
            Keyword::Div,
            Keyword::Xor,
            Keyword::Zerofill,
        ] {
            assert!(MYSQL_RESERVED_COLUMN_NAME.contains(keyword));
            assert!(MYSQL_RESERVED_FUNCTION_NAME.contains(keyword));
            assert!(MYSQL_RESERVED_TYPE_NAME.contains(keyword));
            assert!(MYSQL_RESERVED_BARE_ALIAS.contains(keyword));
            assert!(!RESERVED_COLUMN_NAME.contains(keyword));
            assert!(!RESERVED_FUNCTION_NAME.contains(keyword));
            assert!(!RESERVED_TYPE_NAME.contains(keyword));
            assert!(!RESERVED_BARE_ALIAS.contains(keyword));
        }

        // Reverse divergence: PostgreSQL reserves `OFFSET`/`SYMMETRIC`, MySQL does
        // not, so under MySQL they are free identifiers in every position.
        for keyword in [Keyword::Offset, Keyword::Symmetric] {
            assert!(RESERVED_COLUMN_NAME.contains(keyword));
            assert!(!MYSQL_RESERVED_COLUMN_NAME.contains(keyword));
            assert!(!MYSQL_RESERVED_FUNCTION_NAME.contains(keyword));
            assert!(!MYSQL_RESERVED_TYPE_NAME.contains(keyword));
            assert!(!MYSQL_RESERVED_BARE_ALIAS.contains(keyword));
        }

        // Position-specific within MySQL: a `type_func_name` built-in (`LEFT`) is
        // rejected as a column/type/bare-alias name but admitted as a function name,
        // so `SELECT left FROM t` fails while `LEFT(s, 3)` parses.
        assert!(MYSQL_RESERVED_COLUMN_NAME.contains(Keyword::Left));
        assert!(MYSQL_RESERVED_TYPE_NAME.contains(Keyword::Left));
        assert!(MYSQL_RESERVED_BARE_ALIAS.contains(Keyword::Left));
        assert!(!MYSQL_RESERVED_FUNCTION_NAME.contains(Keyword::Left));
    }

    #[test]
    fn mysql_window_function_names_are_admitted_only_as_call_heads() {
        // The 11 dedicated window-function names are fully reserved words (rejected as a
        // column/type/bare-alias name, matching `SELECT ROW_NUMBER` / `AS row_number` →
        // ER_PARSE_ERROR on mysql:8) but are carved out of the function-name reject so
        // MySQL's dedicated window grammar can admit `ROW_NUMBER() OVER (…)` as a call
        // head (mysql-reserved-window-function-names).
        for keyword in [
            Keyword::RowNumber,
            Keyword::Rank,
            Keyword::DenseRank,
            Keyword::PercentRank,
            Keyword::CumeDist,
            Keyword::Ntile,
            Keyword::Lead,
            Keyword::Lag,
            Keyword::FirstValue,
            Keyword::LastValue,
            Keyword::NthValue,
        ] {
            assert!(
                MYSQL_RESERVED_KEYWORDS.contains(keyword),
                "{keyword:?} is a fully reserved MySQL word",
            );
            assert!(
                !MYSQL_RESERVED_FUNCTION_NAME.contains(keyword),
                "{keyword:?} must be admissible as a call head",
            );
            assert!(
                MYSQL_RESERVED_COLUMN_NAME.contains(keyword),
                "{keyword:?} must stay reserved as a column name",
            );
            assert!(
                MYSQL_RESERVED_TYPE_NAME.contains(keyword),
                "{keyword:?} must stay reserved as a type name",
            );
            assert!(
                MYSQL_RESERVED_BARE_ALIAS.contains(keyword),
                "{keyword:?} must stay reserved as a bare/AS alias",
            );
        }
        // The carve-out removes *only* the 11 window names: another fully reserved word
        // (`SELECT`) is still rejected as a call head.
        assert!(MYSQL_RESERVED_FUNCTION_NAME.contains(Keyword::Select));
    }

    #[test]
    fn mysql_array_is_reserved_only_as_a_call_head() {
        // The inverse of the `type_func_name` carve-out (mysql-reserved-word-set-8-4-over-
        // rejections): MySQL 8.4 admits `array` as a plain identifier in every position
        // (`SELECT 1 AS array` / `SELECT 1 array` both prepare, engine-verified on 8.4.10)
        // but syntax-rejects `array(...)` as a call (1064). The `function_only` class adds it
        // to the function-name reject set alone, closing both the `ARRAY(...)` over-acceptance
        // and the `AS array` over-rejection at once.
        assert!(
            MYSQL_RESERVED_FUNCTION_NAME.contains(Keyword::Array),
            "array must be rejected as a call head",
        );
        assert!(
            !MYSQL_RESERVED_COLUMN_NAME.contains(Keyword::Array),
            "array must be admissible as a column name",
        );
        assert!(
            !MYSQL_RESERVED_TYPE_NAME.contains(Keyword::Array),
            "array must be admissible as a type name",
        );
        assert!(
            !MYSQL_RESERVED_BARE_ALIAS.contains(Keyword::Array),
            "array must be admissible as a bare/AS alias",
        );
    }

    #[test]
    fn mysql_binding_powers_differ_from_standard_in_comparison_assoc_and_bitwise_ranks() {
        use crate::ast::NotEqSpelling;
        // Two documented delta families over STANDARD: the comparison-row associativity
        // (`NonAssoc` -> `Left`, mysql-comparison-operators-are-left-associative) and the
        // four-way bitwise precedence split MySQL's grammar mandates. Mutating a copy of
        // STANDARD pins the exact shape rather than trusting the struct update by inspection.
        let mut expected = STANDARD_BINDING_POWERS;
        expected.comparison.assoc = Assoc::Left;
        // `==` rides the comparison row with `=` (the `double_equals` field tracks
        // `comparison`); MySQL does not even lex `==`, but the shape must still match.
        expected.double_equals.assoc = Assoc::Left;
        expected.bitwise_or = BindingPower {
            left: 42,
            right: 43,
            assoc: Assoc::Left,
        };
        expected.bitwise_and = BindingPower {
            left: 44,
            right: 45,
            assoc: Assoc::Left,
        };
        expected.bitwise_shift = BindingPower {
            left: 47,
            right: 48,
            assoc: Assoc::Left,
        };
        expected.bitwise_xor = BindingPower {
            left: 65,
            right: 66,
            assoc: Assoc::Left,
        };
        expected.prefix_bitwise_not = 80;
        assert_eq!(MYSQL_BINDING_POWERS, expected);

        // The load-bearing MySQL ordering (grammar-derived): `|` < `&` < `<<`/`>>` <
        // additive, and `^` (XOR) tighter than multiplicative — the split that makes
        // `1 | 2 & 3` group `1 | (2 & 3)` where PostgreSQL/SQLite group `(1 | 2) & 3`.
        use crate::ast::BitwiseXorSpelling;
        let or = MYSQL_BINDING_POWERS.binary(&BinaryOperator::BitwiseOr);
        let and = MYSQL_BINDING_POWERS.binary(&BinaryOperator::BitwiseAnd);
        let shift = MYSQL_BINDING_POWERS.binary(&BinaryOperator::BitwiseShiftLeft);
        let add = MYSQL_BINDING_POWERS.binary(&BinaryOperator::Plus);
        let mul = MYSQL_BINDING_POWERS.binary(&BinaryOperator::Multiply);
        let xor =
            MYSQL_BINDING_POWERS.binary(&BinaryOperator::BitwiseXor(BitwiseXorSpelling::Caret));
        assert!(or.left < and.left, "`|` looser than `&`");
        assert!(and.left < shift.left, "`&` looser than `<<`/`>>`");
        assert!(shift.left < add.left, "shift looser than additive");
        assert!(mul.left < xor.left, "`^` tighter than multiplicative");

        // Every comparison operator — and `RLIKE`/`REGEXP`, which folds onto the
        // same row (`crate::precedence::BindingPowerTable::binary`) — rides the
        // delta together: MySQL ranks the whole family at one precedence level
        // where "operators of equal precedence evaluate left to right" (MySQL
        // Reference Manual 12.3.1), which is exactly `Assoc::Left`.
        for op in [
            BinaryOperator::Eq(EqualsSpelling::Single),
            BinaryOperator::NotEq(NotEqSpelling::AngleBracket),
            BinaryOperator::Lt,
            BinaryOperator::LtEq,
            BinaryOperator::Gt,
            BinaryOperator::GtEq,
            BinaryOperator::Regexp(RegexpSpelling::Rlike),
            BinaryOperator::Regexp(RegexpSpelling::Regexp),
        ] {
            let bp = MYSQL_BINDING_POWERS.binary(&op);
            assert_eq!(bp.assoc, Assoc::Left, "{op:?} should be left-associative");
            assert_eq!(bp.left, 40, "{op:?} keeps the STANDARD left rank");
            assert_eq!(bp.right, 41, "{op:?} keeps the STANDARD right rank");
        }

        // `predicate()` (`IS [NOT] NULL` / `[NOT] BETWEEN` / `[NOT] IN`) is a
        // derived accessor onto `comparison`, so it rides the same delta without a
        // second field to keep in sync.
        assert_eq!(MYSQL_BINDING_POWERS.predicate().assoc, Assoc::Left);

        // Render-time parenthesization (the other half of the one binding-power
        // table, ADR-0008) picks the delta up automatically: a same-precedence
        // child on the left needs no parens under `Left` (`a < b < c` renders
        // bare), while the right still does (`a < (b < c)` keeps its parens
        // either way — that side was never where NonAssoc vs. Left differed).
        assert!(!MYSQL_BINDING_POWERS.needs_parens(
            &BinaryOperator::Lt,
            &BinaryOperator::Eq(EqualsSpelling::Single),
            Side::Left
        ));
        assert!(MYSQL_BINDING_POWERS.needs_parens(
            &BinaryOperator::Lt,
            &BinaryOperator::Eq(EqualsSpelling::Single),
            Side::Right
        ));
    }
}
