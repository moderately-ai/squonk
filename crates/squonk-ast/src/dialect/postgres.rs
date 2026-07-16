// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The PostgreSQL dialect preset.
//!
//! The module is self-contained for feature gating: a build without the `postgres`
//! cargo feature compiles none of this preset's data.

use super::{
    AccessControlSyntax, AggregateCallSyntax, CallSyntax, CaretOperator, Casing,
    ColumnDefinitionSyntax, CommentSyntax, ConstraintSyntax, CreateTableClauseSyntax,
    DoubleAmpersand, ExistenceGuards, ExpressionSyntax, FeatureSet, GroupingSyntax,
    IdentifierSyntax, IndexAlterSyntax, JoinSyntax, KeywordOperators, KeywordSet,
    MaintenanceSyntax, MutationSyntax, NullOrdering, NumericLiteralSyntax, OperatorSyntax,
    POSTGRES_BYTE_CLASSES, ParameterSyntax, PipeOperator, PredicateSyntax, QueryTailSyntax,
    RESERVED_BARE_ALIAS, RESERVED_COLUMN_NAME, RESERVED_FUNCTION_NAME, RESERVED_SET_VALUE_WORDS,
    RESERVED_TYPE_NAME, STANDARD_IDENTIFIER_QUOTES, SelectSyntax, SessionVariableSyntax,
    ShowSyntax, StatementDdlGates, StringFuncForms, StringLiteralSyntax, TableExpressionSyntax,
    TableFactorSyntax, TargetSpelling, TypeNameSyntax, TransactionSyntax, UtilitySyntax,
};
use crate::precedence::{
    Assoc, BindingPower, BindingPowerTable, IS_PREDICATE_BELOW_COMPARISON,
    RANGE_PREDICATE_ABOVE_COMPARISON, STANDARD_SET_OPERATION_BINDING_POWERS,
};

impl CommentSyntax {
    /// The `POSTGRES` preset for comment syntax: the ANSI baseline with line comments
    /// additionally terminated by a carriage return.
    ///
    /// PostgreSQL's flex scanner defines a `--` line comment as `("--"{non_newline}*)`
    /// with `non_newline [^\n\r]`, so the comment ends at the first `\n` *or* `\r`
    /// (engine-verified against libpg_query: `SELECT 1 -- c\rFROM` reads `FROM` as a live
    /// token and rejects, where the `\n`-only reading accepts). DuckDB shares this
    /// PostgreSQL-derived scanner, so it adopts this preset too — the only two shipped
    /// dialects whose line comments end at a bare `\r` (SQLite/MySQL keep the `\n`-only
    /// `CommentSyntax::ANSI`/`::MYSQL` reading). Everything else matches `ANSI`.
    pub const POSTGRES: Self = Self {
        line_comment_ends_at_carriage_return: true,
        line_comment_hash: false,
        nested_block_comments: true,
        versioned_comments: None,
        unterminated_block_comment_at_eof: false,
    };
}

impl StringLiteralSyntax {
    /// The `POSTGRES` preset for string literal syntax.
    pub const POSTGRES: Self = Self {
        escape_strings: true,
        dollar_quoted_strings: true,
        // PostgreSQL has NO `N'…'` national-string constant: its scanner rewrites a
        // quote-adjacent `[nN]'` to the identifier `nchar` plus a separate string, so
        // `N'x'` is the typed literal `nchar 'x'` and `DO LANGUAGE N'p'` reads `nchar` as
        // the language name (engine-probed against pg_query 6.1.1). Arming a national-string
        // token here mislexed both — it read `N'x'` as one string constant (a different
        // shape from PG's typed cast) and rejected `DO LANGUAGE N'p'` outright. With the flag
        // off, `N` lexes as an ordinary word, so `N'x'` folds into the generalized typed
        // literal `N '…'` (an `Expr::Cast`) — matching PG's accept/reject in every position.
        // (We do not replicate PG's source-discarding `N`→`nchar` substitution: it would
        // break span round-trip, and the residual is a type-name-spelling shape nicety, not
        // an accept/reject divergence.)
        national_strings: false,
        double_quoted_strings: false,
        backslash_escapes: false,
        unicode_strings: true,
        bit_string_literals: true,
        // PostgreSQL's `X'…'` is a deferred *bit-string* (odd-length hex allowed), not
        // the eager even-byte blob — so this stays off and `bit_string_literals` owns the
        // `x`/`X` marker.
        blob_literals: false,
        charset_introducers: false,
        // PostgreSQL requires a newline in the separator between adjacent literals.
        same_line_adjacent_concat: false,
    };
}

impl NumericLiteralSyntax {
    /// The `POSTGRES` preset for numeric literal syntax.
    pub const POSTGRES: Self = Self {
        hex_integers: true,
        octal_integers: true,
        binary_integers: true,
        underscore_separators: true,
        // PG's `0[xX](_?{hexdigit})+` admits a `_` ahead of the first radix digit
        // (`0x_1F`, oracle-probed); SQLite does not, so this is its own axis.
        radix_leading_underscore: true,
        // `$` is a parameter or dollar-quote opener here, not a T-SQL money sigil.
        money_literals: false,
        // PG's scanner treats a numeric literal as a maximal-munch lexeme and errors on
        // trailing junk (`123abc`, `0x`, `100_`), which only distinguishes the malformed
        // forms from valid radix forms once those forms are recognised.
        reject_trailing_junk: true,
    };
}

impl ParameterSyntax {
    /// The `POSTGRES` preset for parameter syntax.
    pub const POSTGRES: Self = Self {
        positional_dollar: true,
        positional_dollar_large: true,
        anonymous_question: false,
        named_colon: false,
        named_at: false,
        // SQLite's `$name`; PostgreSQL spells `$` as positional/`$tag$`, never a
        // dollar-named parameter.
        named_dollar: false,
        numbered_question: false,
    };
}

impl IdentifierSyntax {
    /// The `POSTGRES` preset for identifier syntax.
    pub const POSTGRES: Self = Self {
        // PostgreSQL's scanner admits every high-bit character in an unquoted identifier.
        non_ascii: super::NonAsciiIdentifierSyntax::Any,
        // `$` is an identifier-continue byte, not an identifier-start byte, so `a$b` is
        // one word while a leading `$1` still dispatches to the parameter scanner.
        dollar_in_identifiers: true,
        // PostgreSQL syntax-rejects a string literal in a name position — `DELETE FROM 't'`
        // is a parse error — so the SQLite string-identifier misfeature stays off.
        string_literal_identifiers: false,
        // DuckDB-only `FROM 't'` single-part Sconst table name.
        string_literal_table_names: false,
        empty_quoted_identifiers: false,
    };
}

impl TableExpressionSyntax {
    /// The `POSTGRES` preset for table expression syntax.
    pub const POSTGRES: Self = Self {
        only: true,
        table_sample: true,
        parenthesized_joins: true,
        table_alias_column_lists: true,
        join_using_alias: true,
        // Index hints and `PARTITION (…)` selection are MySQL-only table-factor tails.
        index_hints: false,
        // MSSQL-only `WITH (...)` table hints; under PostgreSQL `WITH` stays CTE-only.
        table_hints: false,
        partition_selection: false,
        base_table_alias_column_lists: true,
        // DuckDB-only string-literal table alias (`FROM t AS 't'('k')`).
        string_literal_aliases: false,
        aliased_parenthesized_join: true,
        // PostgreSQL's bare table alias is a `ColId` (`alias_clause: [AS] ColId …`), not the
        // SQLite `ids` class, so the JOIN keywords stay reserved as a ColId either way.
        bare_table_alias_is_bare_label: false,
        // No table version / time-travel modifier under PostgreSQL.
        table_version: false,
        // No PartiQL / SUPER table-position JSON path under PostgreSQL.
        table_json_path: false,
        // No SQLite `INDEXED BY` / `NOT INDEXED` index directive under PostgreSQL.
        indexed_by: false,
        prefix_colon_alias: false,
    };
}

impl JoinSyntax {
    /// The `POSTGRES` preset for join syntax.
    pub const POSTGRES: Self = Self {
        stacked_join_qualifiers: true,
        full_outer_join: true,
        // PostgreSQL parse-rejects `NATURAL CROSS JOIN` (engine-probed on 16).
        natural_cross_join: false,
        // PostgreSQL has no `STRAIGHT_JOIN`; the word is a non-reserved identifier there.
        straight_join: false,
        // The DuckDB nonstandard joins; `asof`/`positional` are plain identifiers here.
        asof_join: false,
        positional_join: false,
        // `SEMI`/`ANTI` are plain identifiers here too.
        semi_anti_join: false,
        sided_semi_anti_join: false,
        apply_join: false,
        // PostgreSQL/SQL:2023 recursive-query SEARCH/CYCLE clauses on a CTE.
        recursive_search_cycle: true,
        // PostgreSQL parse-accepts ORDER BY/LIMIT/OFFSET on a recursive CTE's UNION body
        // (engine-probed on pg_query 17); the recursion restriction is a binder-layer check.
        recursive_union_rejects_order_limit: false,
        // `USING KEY` is DuckDB's keyed-recursion clause; PostgreSQL has no such spelling.
        recursive_using_key: false,
    };
}

impl TableFactorSyntax {
    /// The `POSTGRES` preset for table factor syntax.
    pub const POSTGRES: Self = Self {
        lateral: true,
        table_functions: true,
        rows_from: true,
        // `FROM unnest(array[…]) [WITH ORDINALITY] [AS u(…)]` — engine-probed accept.
        // `WITH OFFSET` is BigQuery-only (PostgreSQL parse-rejects it), so off here.
        unnest: true,
        unnest_with_offset: false,
        table_function_ordinality: true,
        // PostgreSQL admits `FROM current_date` and an alias on a parenthesized join.
        special_function_table_source: true,
        // PIVOT/UNPIVOT are DuckDB-only operators.
        pivot: false,
        unpivot: false,
        // DuckDB-only DESCRIBE/SHOW/SUMMARIZE table source.
        show_ref: false,
        // DuckDB-only bare `FROM VALUES (…) AS t` row-list table factor.
        from_values: false,
        // SQL/JSON JSON_TABLE and SQL/XML XMLTABLE table factors — engine-verified.
        json_table: true,
        xml_table: true,
        // `TABLE(<expr>)` is a Snowflake/Oracle form PostgreSQL parse-rejects
        // (engine-probed); off here.
        table_expr_factor: false,
        // The standard PIVOT is a Snowflake/BigQuery/Oracle form PostgreSQL has no
        // grammar for; off here (and inherited off by the DuckDB preset).
        pivot_value_sources: false,
        // MATCH_RECOGNIZE is a Snowflake/Oracle form PostgreSQL has no grammar for; off
        // here (and inherited off by the DuckDB preset).
        match_recognize: false,
        // OPENJSON is a SQL Server form PostgreSQL has no grammar for; off here (and
        // inherited off by the DuckDB preset).
        open_json: false,
    };
}

impl MutationSyntax {
    /// The `POSTGRES` preset for mutation syntax.
    pub const POSTGRES: Self = Self {
        insert_ignore: false,
        insert_overwrite: false,
        returning: true,
        on_conflict: true,
        on_duplicate_key_update: false,
        multi_column_assignment: true,
        update_tuple_value_row_arity: false,
        where_current_of: true,
        // PostgreSQL 15+ implements the standard `MERGE` statement.
        merge: true,
        // The MySQL `REPLACE` statement and `INSERT ... SET` source are not PostgreSQL.
        replace_into: false,
        insert_set: false,
        // PostgreSQL row-limits neither `UPDATE` nor `DELETE`, so the MySQL
        // `ORDER BY ... LIMIT` tails are rejected.
        update_delete_tails: false,
        joined_update_delete: false,
        // PostgreSQL spells conflict resolution `ON CONFLICT`, not the SQLite verb-level
        // `INSERT OR`/`UPDATE OR <action>` prefix, so it is rejected.
        or_conflict_action: false,
        insert_column_matching: false,
        delete_using: true,
        update_from: true,
        // PostgreSQL admits an alias on a `DELETE … USING` target and a leading `WITH`
        // before `INSERT` and (15+) before `MERGE`.
        delete_using_target_alias: true,
        cte_before_insert: true,
        cte_before_merge: true,
        // Data-modifying CTEs (`WITH t AS (DELETE … RETURNING *) SELECT …`) — the
        // PostgreSQL `PreparableStmt` CTE-body extension, admitted at every `WITH`
        // site during raw parsing (probed on pg_query 17).
        data_modifying_ctes: true,
        // MERGE residual grammar (pg-merge-residual-roots): `WHEN NOT MATCHED BY
        // SOURCE/TARGET` (PG 17), `INSERT DEFAULT VALUES`, and the `OVERRIDING` merge
        // insert override — all accepted (probed on pg_query 17).
        merge_when_not_matched_by: true,
        merge_insert_default_values: true,
        merge_insert_overriding: true,
        merge_insert_multirow: false,
        merge_update_set_star: false,
        merge_insert_star_by_name: false,
        merge_error_action: false,
        update_set_qualified_column: true,
    };
}

impl StatementDdlGates {
    /// The `POSTGRES` preset for statement ddl gates.
    pub const POSTGRES: Self = Self {
        colocation_groups: false,
        materialized_view_to: false,
        // PostgreSQL's `CREATE TRIGGER` uses an `EXECUTE FUNCTION` body, not the
        // modelled SQLite `BEGIN … END` form, so it is not dispatched here.
        create_trigger: false,
        // The macro DDL is DuckDB-specific; PostgreSQL's `CREATE FUNCTION` is the
        // string-body routine gated by `routines`, not a live-body macro.
        create_macro: false,
        create_secret: false,
        create_type: false,
        // Virtual tables are SQLite-only; PostgreSQL rejects `CREATE VIRTUAL TABLE`.
        create_virtual_table: false,
        // PostgreSQL has the SQL:2003 T176 sequence generator.
        create_sequence: true,
        create_sequence_cache: true,
        extension_ddl: true,
        transform_ddl: true,
        // PostgreSQL accepts `ALTER SYSTEM` server-configuration DDL.
        alter_system: true,
        // MySQL's tablespace / logfile-group storage DDL is not a PostgreSQL statement
        // (PostgreSQL's `CREATE TABLESPACE` is a different, location-based grammar, not modelled
        // here).
        tablespace_ddl: false,
        logfile_group_ddl: false,
        schemas: true,
        // PostgreSQL accepts the SQL-standard embedded schema-element form
        // (`CREATE SCHEMA s CREATE TABLE t ...`).
        schema_elements: true,
        databases: true,
        // PostgreSQL's `DROP DATABASE` is its own single-name form (unmodelled here) and its
        // `DROP SCHEMA` is the shared name-list drop, not the MySQL DATABASE/SCHEMA synonym.
        drop_database: false,
        materialized_views: true,
        temporary_views: true,
        routines: true,
        or_replace: true,
        // `CREATE RECURSIVE VIEW` stays off pending a PostgreSQL differential.
        recursive_views: false,
        // PostgreSQL routine bodies are opaque `$$…$$`/string definitions, not the
        // MySQL SQL/PSM compound statement.
        compound_statements: false,
        // PostgreSQL has these `ALTER` forms, but they stay gated to the measured dialect
        // (DuckDB) per the no-shadowing doctrine until a PostgreSQL differential lands.
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
    /// The `POSTGRES` preset for create table clause syntax.
    pub const POSTGRES: Self = Self {
        table_options: false,
        // PostgreSQL has no SQLite trailing `WITHOUT ROWID` table option; the trailing
        // `WITHOUT ROWID` is rejected as leftover input.
        without_rowid_table_option: false,
        // PostgreSQL has no SQLite trailing `STRICT` table option; the trailing `STRICT` is
        // rejected as leftover input.
        strict_table_option: false,
        // `OR REPLACE TABLE` and `CREATE SECRET` are DuckDB-specific.
        create_or_replace_table: false,
        storage_parameters: true,
        on_commit: true,
        create_table_as_with_data: true,
        create_table_as_execute: true,
        // PostgreSQL owns declarative partitioning (PARTITION BY / PARTITION OF / ATTACH …).
        declarative_partitioning: true,
        // PostgreSQL owns the legacy `INHERITS (parents)` clause and the `(LIKE src …)` element.
        table_inheritance: true,
        like_source_table: true,
        // No statement-level `CREATE TABLE t LIKE src`: PostgreSQL rejects the bare form at raw
        // parse and reads `(LIKE src …)` only as the element above (MySQL's distinct surface).
        statement_level_table_like: false,
        unlogged_tables: true,
        table_access_method: true,
        without_oids: true,
        typed_tables: true,
    };
}

impl ColumnDefinitionSyntax {
    /// The `POSTGRES` preset for column definition syntax.
    pub const POSTGRES: Self = Self {
        // PostgreSQL's generated columns require the `GENERATED ALWAYS` keywords, and it
        // has none of the SQLite `CREATE TABLE` decorations, so both are rejected.
        generated_column_shorthand: false,
        // PostgreSQL has no SQLite column-level `ON CONFLICT <resolution>` clause; the
        // trailing `ON` after a `UNIQUE`/`PRIMARY KEY`/… constraint is rejected.
        column_conflict_resolution_clause: false,
        // PostgreSQL requires a data type on every column; a typeless column is a SQLite
        // extension it rejects.
        typeless_column_definitions: false,
        // PostgreSQL requires a data type even on a generated column (`GENERATED ALWAYS AS
        // (expr) STORED` still names a type).
        typeless_generated_columns: false,
        // PostgreSQL has no joined `AUTOINCREMENT` attribute (its auto-increment is `serial`
        // types / `GENERATED … AS IDENTITY`); the trailing keyword is rejected.
        joined_autoincrement_attribute: false,
        // PostgreSQL's inline `PRIMARY KEY` takes no `ASC`/`DESC` order qualifier (ordering is a
        // per-index-column property, not an inline column constraint); the trailing keyword is
        // rejected.
        inline_primary_key_ordering: false,
        // PostgreSQL accepts a bare column `COLLATE` but rejects a `CONSTRAINT <name>` prefix on it
        // (there `COLLATE any_name` is a constraint alternative parallel to the nameable one);
        // engine-measured reject.
        named_column_collate_constraint: false,
        identity_columns: true,
        compact_identity_columns: false,
        // PostgreSQL accepts a bare expression default and a `CONSTRAINT <name>` on any
        // inline column constraint.
        default_expression_requires_parens: false,
        // PostgreSQL parses a column-constraint `DEFAULT` as the restricted `b_expr`.
        column_default_requires_b_expr: true,
        // The CREATE TABLE residue surfaces are all PostgreSQL-native: per-column COLLATE,
        // UNLOGGED persistence, column STORAGE/COMPRESSION, the USING access method, legacy
        // WITHOUT OIDS, and typed `OF <type>` tables.
        column_collation: true,
        column_storage: true,
    };
}

impl ConstraintSyntax {
    /// The `POSTGRES` preset for constraint syntax.
    pub const POSTGRES: Self = Self {
        deferrable_constraints: true,
        named_inline_non_check_constraints: true,
        // PostgreSQL requires a constraint element after `CONSTRAINT <name>` — a bodyless
        // `CONSTRAINT <name>` is engine-measured rejected.
        bare_constraint_name: false,
        exclusion_constraints: true,
        constraint_no_inherit_not_valid: true,
        index_constraint_parameters: true,
        // PostgreSQL's PRIMARY KEY / UNIQUE table constraint takes a bare column-name list;
        // COLLATE / opclass / ASC / DESC live in its CREATE INDEX grammar, not here
        // (engine-measured: `PRIMARY KEY (a COLLATE "C")` / `UNIQUE (a DESC)` are syntax errors).
        constraint_column_collate_order: false,
        referential_action_cascade_set: true,
        check_constraint_subqueries: true,
    };
}

impl IndexAlterSyntax {
    /// The `POSTGRES` preset for index alter syntax.
    pub const POSTGRES: Self = Self {
        rename_constraint: true,
        alter_table_set_options: true,
        drop_primary_key: false,
        alter_column_add_identity: false,
        index_storage_parameters: true,
        drop_behavior: true,
        // PostgreSQL's `DROP INDEX` is the shared name-list drop, not the MySQL `ON <table>` form.
        index_drop_on_table: false,
        index_concurrently: true,
        index_using_method: true,
        partial_index: true,
        index_if_not_exists: true,
        index_nulls_order: true,
        alter_table_extended: true,
        alter_nested_column_paths: false,
        alter_existence_guards: true,
        alter_column_set_data_type: true,
        routine_arg_types: true,
        routine_arg_defaults: true,
        routine_arg_modes: true,
        // PostgreSQL's `LANGUAGE` operand is `NonReservedWord_or_Sconst`: `LANGUAGE 'sql'`,
        // `E'sql'`, and `$$sql$$` are accepted alongside the bare word (pg_query-measured).
        routine_language_string: true,
        alter_table_multiple_actions: true,
    };
}

impl ExistenceGuards {
    /// The `POSTGRES` preset for existence guards.
    pub const POSTGRES: Self = Self {
        if_exists: true,
        view_if_not_exists: false,
        create_database_if_not_exists: false,
    };
}

impl SelectSyntax {
    /// The `POSTGRES` preset for select syntax.
    pub const POSTGRES: Self = Self {
        distinct_on: true,
        select_into: true,
        // libpg_query's raw grammar accepts an empty target list (`SELECT`,
        // `SELECT FROM t`); parse-level parity accepts it under the PostgreSQL preset.
        empty_target_list: true,
        // PostgreSQL has no `QUALIFY` clause (a DuckDB extension); `qualify` stays a
        // free identifier there.
        qualify: false,
        // PostgreSQL requires an identifier alias; a string literal there is a syntax error.
        alias_string_literals: false,
        bare_alias_string_literals: false,
        // `UNION [ALL] BY NAME` is a DuckDB extension; PostgreSQL has no name-matched
        // set operation, so `BY` after a set operator is a syntax error there.
        union_by_name: false,
        wildcard_modifiers: false,
        wildcard_replace: false,
        intersect_all: true,
        except_all: true,
        // PostgreSQL parses `t.*` as an ordinary columnref, so it takes the standard
        // `[AS] alias` projection alias (`SELECT t.* x` / `SELECT t.* AS x`); libpg_query-measured.
        qualified_wildcard_alias: true,
        // FROM-first SELECT is a DuckDB extension; PostgreSQL rejects a statement-position
        // `FROM` (`FROM t SELECT x` is a syntax error there — a required over-accept guard).
        from_first: false,
        explicit_table: true,
        parenthesized_query_operands: true,
        // libpg_query's raw grammar accepts a ragged VALUES constructor (`VALUES (1,2),(3)`);
        // PostgreSQL defers the equal-length check to parse-analysis, past parse-level
        // parity, so the preset accepts it at parse (measured: `pg_query::parse` accepts).
        values_rows_require_equal_arity: false,
        // PostgreSQL's query-position VALUES constructor uses bare-parenthesized rows.
        values_row_constructor: true,
        // PostgreSQL's projection `AS` alias is a `ColLabel` admitting reserved words.
        as_alias_rejects_reserved: false,
        // A trailing comma in a list is a DuckDB tolerance; PostgreSQL rejects it.
        trailing_comma: false,
        // The prefix colon alias is a DuckDB extension (DuckDB overrides this to `true`);
        // a `:` at a select-item / table-factor head is a parse error in PostgreSQL.
        prefix_colon_alias: false,
        // Hive/Spark `LATERAL VIEW` is not PostgreSQL (DuckDB inherits this value);
        // PostgreSQL's LATERAL is the derived-table factor, and a post-FROM `LATERAL`
        // is a parse error.
        lateral_view_clause: false,
        // The Oracle-style `START WITH`/`CONNECT BY` hierarchical query clause is not
        // PostgreSQL (DuckDB inherits this value); PostgreSQL uses recursive CTEs, so a
        // post-WHERE `CONNECT BY`/`START WITH` is a parse error.
        connect_by_clause: false,
    };
}

impl QueryTailSyntax {
    /// The `POSTGRES` preset for query tail syntax.
    pub const POSTGRES: Self = Self {
        fetch_first: true,
        limit_offset_comma: false,
        // PostgreSQL's `FOR UPDATE/SHARE [OF …] [NOWAIT|SKIP LOCKED]` locking clause.
        locking_clauses: true,
        // PostgreSQL's `FOR NO KEY UPDATE` / `FOR KEY SHARE` strengths and its stacked
        // clauses (`FOR UPDATE OF a FOR SHARE OF b`) — engine-verified accepts
        // (pg-locking-clause-strengths-and-stacking); multi-table `OF` is the shared
        // core the `locking_clauses` gate already reaches.
        key_lock_strengths: true,
        stacked_locking_clauses: true,
        using_sample: false,
        leading_offset: true,
        limit_expressions: true,
        limit_percent: false,
        // PostgreSQL's `gram.y` `WITH TIES` guards (requires `ORDER BY`, excludes
        // `SKIP LOCKED`), raised during raw parsing.
        with_ties_requires_order_by: true,
        // BigQuery/ZetaSQL `|>` pipe syntax is not PostgreSQL; off here (DuckDB inherits
        // this value). A `|>` after a query is a parse error, and the token never lexes.
        pipe_syntax: false,
        // ClickHouse `LIMIT n BY …` is not PostgreSQL (DuckDB inherits this value); a
        // `BY` after `LIMIT` is a parse error.
        limit_by_clause: false,
        // ClickHouse `SETTINGS …` is not PostgreSQL (DuckDB inherits this value); a
        // trailing `SETTINGS` is a parse error.
        settings_clause: false,
        // ClickHouse `FORMAT …` is not PostgreSQL (DuckDB inherits this value); a trailing
        // `FORMAT` is a parse error.
        format_clause: false,
        // MSSQL `FOR XML`/`FOR JSON` is not PostgreSQL (DuckDB inherits this value); a
        // trailing `FOR XML`/`FOR JSON` is a parse error.
        for_xml_json_clause: false,
    };
}

impl GroupingSyntax {
    /// The `POSTGRES` preset for grouping syntax.
    pub const POSTGRES: Self = Self {
        grouping_sets: true,
        // The trailing `WITH ROLLUP` is a MySQL-only spelling; PostgreSQL writes the
        // super-aggregate as the `ROLLUP (…)` grouping set above.
        with_rollup: false,
        // PostgreSQL's operator-driven `ORDER BY <expr> USING <operator>` sort form.
        order_by_using: true,
        // `GROUP BY ALL` / `ORDER BY ALL` are DuckDB clause modes; PostgreSQL reserves
        // `ALL`, so either spelling is a syntax error there.
        group_by_all: false,
        // But PostgreSQL DOES admit `GROUP BY {DISTINCT | ALL} <items>` — the SQL:2016
        // grouping-set quantifier, a modifier on a non-empty item list (not a mode).
        group_by_set_quantifier: true,
        order_by_all: false,
    };
}

impl UtilitySyntax {
    /// The `POSTGRES` preset for utility syntax.
    pub const POSTGRES: Self = Self {
        copy: true,
        // PostgreSQL's `COPY` is the `{FROM | TO}` transfer (the `copy` gate above);
        // Snowflake's `COPY INTO` load/unload is a different statement PostgreSQL has no
        // form of, so this stays off (a `COPY INTO` surfaces as an unknown statement).
        copy_into: false,
        stage_references: false,
        comment_on: true,
        comment_if_exists: false,
        pragma: false,
        attach: false,
        // `KILL` and the MySQL `DESCRIBE`/`DESC` synonyms + table-metadata overload are
        // MySQL-only, so off here — `EXPLAIN` keeps its plain query-plan grammar.
        kill: false,
        // MySQL's `HANDLER` cursor family is not a PostgreSQL statement.
        handler_statements: false,
        // MySQL's `INSTALL`/`UNINSTALL` `PLUGIN`/`COMPONENT` family is not a PostgreSQL statement.
        plugin_component_statements: false,
        // MySQL's server-administration families are not PostgreSQL statements.
        shutdown: false,
        restart: false,
        clone: false,
        import_table: false,
        help_statement: false,
        binlog: false,
        // MySQL's `CACHE INDEX` / `LOAD INDEX INTO CACHE` key-cache pair is MyISAM-specific,
        // not a PostgreSQL statement.
        key_cache_statements: false,
        // DuckDB (PostgreSQL-derived) is the one dialect with a `USE` statement; the
        // PostgreSQL base itself has none, so its leading `USE` stays undispatched.
        use_statement: false,
        // Moot: `use_statement` is off, so the name-arity refinement is unreachable.
        use_qualified_name: false,
        // Moot: `use_statement` is off, so the string-name refinement is unreachable.
        use_string_literal_name: false,
        // PostgreSQL has `PREPARE`/`EXECUTE`/`DEALLOCATE` too (`CALL` is a distinct
        // statement PostgreSQL also has, but tracked by its own `call` flag below, off
        // here — a separate, unfitted grammar ticket).
        prepared_statements: true,
        // PostgreSQL's own `PREPARE name ( <type> [, ...] ) AS ...` parenthesized
        // parameter-type list — a widening of the `prepared_statements` name position,
        // full type names including parameterized (`numeric(10,2)`) and arrayed
        // (`int[]`) forms, at least one, an empty `()` rejected (`planner-parity-
        // prepare-typed-parameters`, pg_query 6.1.1-verified).
        prepare_typed_parameters: true,
        // MySQL's `PREPARE ... FROM {'text' | @var}` lifecycle is a different grammar on
        // the same keywords; PostgreSQL keeps its typed-`AS` `prepared_statements` above
        // (`PREPARE p FROM 'x'` is a pg_query syntax error), so this stays off.
        prepared_statements_from: false,
        call: false,
        // `call` is off here (a separate, unfitted grammar ticket), so its MySQL bare-name
        // widening is moot and off too.
        call_bare_name: false,
        load_extension: true,
        load_bare_name: false,
        load_data: false,
        reset_scope: false,
        detach_if_exists: false,
        // PostgreSQL's `DO [LANGUAGE <lang>] '<body>'` anonymous code block (pg_query
        // PG-17 accepts). No other shipped fitted preset has it.
        do_statement: true,
        // MySQL's `DO <expr-list>` is a different behaviour on the `DO` keyword; PostgreSQL
        // keeps its anonymous-code-block reading, so this stays off.
        do_expression_list: false,
        // PostgreSQL's own `LOCK [TABLE] <rel>, … [IN <mode> MODE]` is a *different
        // behaviour* on the `LOCK` keyword (a statement-level mode list, not per-table lock
        // kinds) and is not yet modelled — when it is, it takes its own gate; MySQL's
        // reading stays off here. The backup-lock pair is MySQL-only.
        lock_tables: false,
        lock_instance: false,
        // PostgreSQL's own `BEGIN` modifier vocabulary is `ISOLATION LEVEL …`/`READ
        // ONLY|WRITE`/`[NOT] DEFERRABLE` (the existing `TransactionMode` list), not
        // SQLite's `DEFERRED`/`IMMEDIATE`/`EXCLUSIVE` keywords (pg_query PG-17 rejects all
        // three), so this stays off.
        // MySQL's `XA` distributed-transaction family is MySQL-only; PostgreSQL has no `XA`
        // statement, so the leading `XA` keyword is not dispatched.
        // The standalone `RENAME TABLE`/`RENAME USER` statements are MySQL-only; PostgreSQL
        // renames objects via `ALTER … RENAME TO`, so the leading `RENAME` is not dispatched.
        rename_statement: false,
        signal_diagnostics: false,
        // `EXPORT`/`IMPORT DATABASE` are DuckDB-specific; PostgreSQL has no such statements,
        // so the leading keywords stay undispatched. DuckDB turns the pair on over its
        // PostgreSQL base.
        export_import_database: false,
        // `UPDATE EXTENSIONS` is DuckDB extension management; PostgreSQL has no such statement,
        // so the `EXTENSIONS` lookahead is never taken and every `UPDATE` reaches the DML
        // parser. DuckDB turns it on over its PostgreSQL base.
        update_extensions: false,
        // MySQL's `FLUSH` / `PURGE BINARY LOGS` server-administration statements — PostgreSQL
        // (and DuckDB over its base) has neither, so both leading-keyword gates stay off.
        flush: false,
        purge_binary_logs: false,
        replication_statements: false,
};
}
impl TransactionSyntax {
    /// Transaction-control surface for the `POSTGRES` preset (split from UtilitySyntax).
    pub const POSTGRES: Self = Self {
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
        abort_transaction_alias: true,
        end_transaction_alias: true,
        transaction_release: false,
        transaction_chain: true,
        release_savepoint_keyword_optional: true,
        begin_transaction_mode: false,
        xa_transactions: false,
    };
}


impl ShowSyntax {
    /// The `POSTGRES` preset for show syntax.
    pub const POSTGRES: Self = Self {
        describe: false,
        // DuckDB (PostgreSQL-derived) turns this on; PostgreSQL has no `DESCRIBE`/`SUMMARIZE`.
        describe_summarize: false,
        session_statements: true,
        // `var_value` accepts `NonReservedWord_or_Sconst`, numeric values, and the
        // explicitly named boolean keywords `TRUE`, `FALSE`, and `ON`.
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
    /// The `POSTGRES` preset for maintenance syntax.
    pub const POSTGRES: Self = Self {
        // PostgreSQL has its own (differently-shaped) `VACUUM`/`REINDEX`/`ANALYZE`
        // maintenance statements, but only SQLite's forms are modelled, so they stay
        // off here and a leading `VACUUM`/`REINDEX`/`ANALYZE` surfaces as unknown.
        vacuum: false,
        vacuum_analyze: false,
        reindex: false,
        analyze: false,
        analyze_columns: false,
        // PostgreSQL has the bare `CHECKPOINT` and the `LOAD '<library>'` statement (both
        // pg_query PG-17 accepts). The DuckDB operand/argument/scope extensions are not
        // PostgreSQL: `FORCE CHECKPOINT`/`CHECKPOINT db`, a bare-identifier `LOAD`, a
        // `RESET` scope prefix, and `DETACH … IF EXISTS` are all pg_query parser errors.
        checkpoint: true,
        checkpoint_database: false,
        // The MySQL admin-table verbs are MySQL-only; PostgreSQL's `ANALYZE` is its own
        // (unmodelled) form and there is no `CHECK/CHECKSUM/OPTIMIZE/REPAIR TABLE`.
        table_maintenance: false,
    };
}

impl AccessControlSyntax {
    /// The `POSTGRES` preset for access control syntax.
    pub const POSTGRES: Self = Self {
        alter_role_rename: true,
        access_control: true,
        // PostgreSQL admits the schema-scoped grant objects (`ON SCHEMA`, `ON ALL … IN
        // SCHEMA`) and the `{GRANT|ADMIN} OPTION FOR` `REVOKE` prefix.
        access_control_extended_objects: true,
        // PostgreSQL's `CREATE ROLE`/`CREATE USER` is its own (unmodelled) grammar, not the
        // MySQL account-management family, so this MySQL-shaped surface is off.
        user_role_management: false,
        // PostgreSQL uses the typed-object/role-spec grant grammar, not MySQL's account-based route.
        access_control_account_grants: false,
    };
}

impl TypeNameSyntax {
    /// The `POSTGRES` preset for type name syntax.
    pub const POSTGRES: Self = Self {
        signed_type_modifier: true,
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
        nullable_type: false,
        low_cardinality_type: false,
        fixed_string_type: false,
        datetime64_type: false,
        nested_type: false,
        bit_width_integer_names: false,
        liberal_type_names: false,
        string_type_modifiers: false,
        angle_bracket_types: false,
    };
}

impl ExpressionSyntax {
    /// The `POSTGRES` preset for expression syntax.
    pub const POSTGRES: Self = Self {
        typecast_operator: true,
        subscript: true,
        // PostgreSQL slices are two-bound only; the three-bound `[lower:upper:step]` is
        // DuckDB's, so a `base[a:b:c]` here is a clean parse error at the second `:`.
        slice_step: false,
        collate: true,
        at_time_zone: true,
        semi_structured_access: false,
        array_constructor: true,
        // PostgreSQL's multidimensional array literals — the bare-bracket sub-row inside
        // `ARRAY[...]` (`ARRAY[[1,2],[3,4]]`).
        multidim_array_literals: true,
        // The `[…]`/`{…}`/`MAP` collection literals are DuckDB's; PostgreSQL spells
        // arrays with the `ARRAY` keyword and rows with `ROW(...)`.
        collection_literals: false,
        row_constructor: true,
        // BigQuery's `STRUCT(...)` value constructor is a dialect extension; PostgreSQL
        // keeps `struct(...)` an ordinary function call.
        struct_constructor: false,
        field_selection: true,
        field_wildcard: true,
        // PostgreSQL admits temporal and generalized typed literals.
        typed_string_literals: true,
        // The PostgreSQL prefix-typed interval literal (`INTERVAL '1' HOUR TO SECOND`,
        // and the unit-less `INTERVAL '1'` whose fields default from the string). DuckDB
        // and Lenient inherit `true`; MySQL overrides to `false` (no interval literal).
        typed_interval_literal: true,
        // The relaxed interval spellings are DuckDB's; PostgreSQL admits only the
        // standard quoted `INTERVAL '1' DAY`. DuckDB overrides this to `true`.
        relaxed_interval_syntax: false,
        mysql_interval_operator: false,
        // The `#n` positional column is DuckDB's; PostgreSQL spells `#` bitwise-XOR
        // (`hash_bitwise_xor: true`), so it stays off here. DuckDB overrides it to `true`.
        positional_column: false,
        lambda_keyword: false,
    };
}

impl OperatorSyntax {
    /// The `POSTGRES` preset for operator syntax.
    pub const POSTGRES: Self = Self {
        operator_construct: true,
        containment_operators: true,
        json_arrow_operators: true,
        // The `jsonb` existence/path/search operators `?`/`?|`/`?&`/`@?`/`@@`/`#>`/`#>>`/`#-`
        // (planner-parity-pg-json-op-*). PostgreSQL enables the whole family; it has no `?`
        // parameter, so the `?`-led members do not contend with a placeholder here.
        jsonb_operators: true,
        // SQLite-only equality surface; PostgreSQL has neither `==` nor general `IS`.
        double_equals: false,
        // DuckDB-only `//` spelling; PostgreSQL spells only `/`.
        integer_divide_slash: false,
        starts_with_operator: false,
        is_general_equality: false,
        // Truth-value tests are standard SQL (F571); libpg_query accepts all six forms.
        truth_value_tests: true,
        // `<=>` is MySQL-only.
        null_safe_equals: false,
        // The single-arrow lambda is DuckDB-only: PostgreSQL's `->` is always the
        // JSON accessor, so `x -> x + 1` stays a `JsonGet` binary op here.
        lambda_expressions: false,
        // PostgreSQL accepts the bitwise `| & ~ << >>` operators (engine-measured via
        // libpg_query). Bitwise XOR is its `#` spelling, carried by `hash_bitwise_xor` below.
        bitwise_operators: true,
        quantified_comparisons: true,
        quantified_comparison_lists: true,
        // PostgreSQL quantifies any operator except the boolean keywords `AND`/`OR`
        // (engine-probed via libpg_query: `3 * ANY('{1,2,3}')` parses).
        quantified_arbitrary_operator: true,
        // PostgreSQL's general symbolic-operator surface — regex `~`/`!~`/`~*`/`!~*`,
        // geometric/network/text-search ops, negator spellings, and prefix/user-defined
        // operators, all lexed by the maximal-munch `Op`-class rule (libpg_query-measured).
        custom_operators: true,
        null_test_postfix: true,
        // PostgreSQL removed postfix operators in version 14 (`SELECT 10!` now errors), so the
        // postfix reduction stays off — the general operator surface is infix/prefix only here.
        postfix_operators: false,
    };
}

impl CallSyntax {
    /// The `POSTGRES` preset for call syntax.
    pub const POSTGRES: Self = Self {
        named_argument: true,
        // The UTC_* niladic functions are MySQL-only.
        utc_special_functions: false,
        columns_expression: false,
        extract_from_syntax: true,
        try_cast: false,
        // PostgreSQL `CAST` admits any type name as its target.
        restricted_cast_targets: false,
        // PostgreSQL admits a single-quoted string as the `EXTRACT(<field> FROM x)` field
        // (`Sconst` in gram.y `extract_arg`), engine-verified against pg_query alongside its
        // reject boundary (a non-string non-identifier field). The remaining DuckDB-specific
        // call tails below stay off.
        extract_string_field: true,
        method_chaining: false,
        // PostgreSQL's SQL/JSON constructors `JSON()`/`JSON_SCALAR()`/`JSON_SERIALIZE()`
        // require their context-item argument (dedicated `gram.y` productions).
        sqljson_constructors_require_argument: true,
        // The SQL:2016 SQL/JSON expression functions (JSON_VALUE/JSON_QUERY/JSON_EXISTS,
        // the JSON_OBJECT/JSON_ARRAY constructors + aggregates, JSON()/JSON_SCALAR()/
        // JSON_SERIALIZE(), and the IS JSON predicate) are PostgreSQL grammar.
        sqljson_expression_functions: true,
        // The SQL:2006 SQL/XML expression functions (xmlelement/xmlforest/xmlconcat/
        // xmlparse/xmlpi/xmlroot/xmlserialize/xmlexists) and the IS DOCUMENT predicate are
        // PostgreSQL grammar (func_expr_common_subexpr).
        xml_expression_functions: true,
        variadic_argument: true,
        // PostgreSQL's `merge_action()` MERGE-RETURNING support function (dedicated
        // `MERGE_ACTION '(' ')'` production, raw-parse-accepted anywhere).
        merge_action_function: true,
        convert_function: false,
    };
}

impl StringFuncForms {
    /// The `POSTGRES` preset for string func forms.
    pub const POSTGRES: Self = Self {
        // The standard-SQL string special forms (the planner-parity-expr-substring/
        // position/overlay/trim bundle), engine-verified against pg_query (PG 17):
        // the full substring surface including the FOR-leading orders and the
        // SIMILAR/ESCAPE regex form, symmetric b_expr POSITION operands, OVERLAY
        // with its plain-call fallback (`overlay('a')` parse-accepts — arity is a
        // catalog concern, so the plain-call arity floor stays off, and `substr`
        // stays an ordinary catalog function with no keyword form), and the loose
        // trim_list tails.
        substring_from_for: true,
        substring_leading_for: true,
        substring_similar: true,
        substring_plain_call_requires_2_or_3_args: false,
        substr_from_for: false,
        position_in: true,
        position_asymmetric_operands: false,
        overlay_placing: true,
        overlay_requires_placing: false,
        trim_from: true,
        trim_list_syntax: true,
        // PostgreSQL's `COLLATION FOR (<expr>)` common-subexpr (dedicated
        // `COLLATION FOR '(' a_expr ')'` production).
        collation_for_expression: true,
        // PostgreSQL's `ceil`/`ceiling` are plain functions — no `TO <field>` grammar
        // (`pg_query`-verified: `CEIL(x TO DAY)` is a syntax error at `TO`).
        ceil_to_field: false,
        // PostgreSQL's `floor` is a plain function — no `TO <field>` grammar
        // (`pg_query`-verified: `FLOOR(x TO DAY)` is a syntax error at `TO`).
        floor_to_field: false,
        match_against: false,
    };
}

impl AggregateCallSyntax {
    /// The `POSTGRES` preset for aggregate call syntax.
    pub const POSTGRES: Self = Self {
        // The `GROUP_CONCAT(... SEPARATOR …)` delimiter is MySQL's; PostgreSQL spells the
        // aggregate delimiter as an ordinary `string_agg(x, ',')` argument.
        group_concat_separator: false,
        within_group: true,
        aggregate_filter: true,
        // PostgreSQL requires `FILTER (WHERE …)` (engine-probed on PG-17); the keyword-less
        // DuckDB body is not accepted.
        filter_optional_where: false,
        // PostgreSQL admits an aggregate's argument forms regardless of a space before the
        // `(` — the significant-space rule is MySQL's `IGNORE_SPACE`-off tokenizer only.
        aggregate_args_require_adjacent_paren: false,
        null_treatment: false,
        // MySQL-only built-in aggregate/window arity restrictions; PostgreSQL admits an
        // empty aggregate call and `OVER` on any function.
        aggregate_calls_reject_empty_arguments: false,
        over_requires_windowable_function: false,
        window_function_tail: false,
        standalone_argument_order_by: false,
    };
}

impl PredicateSyntax {
    /// The `POSTGRES` preset for predicate syntax.
    pub const POSTGRES: Self = Self {
        is_distinct_from: true,
        like: true,
        ilike: true,
        similar_to: true,
        // The SQL-standard `(s1, e1) OVERLAPS (s2, e2)` period predicate.
        overlaps_period_predicate: true,
        // PostgreSQL requires the parentheses; `x IN y` is a syntax error there.
        unparenthesized_in_list: false,
        // `'foo' LIKE ANY (ARRAY['%a'])` / `ILIKE ALL (…)` — PostgreSQL's pattern-match
        // ScalarArrayOpExpr (engine-probed via libpg_query; `SIMILAR TO ANY` rejects).
        pattern_match_quantifier: true,
        between_symmetric: true,
        is_normalized: true,
        // PostgreSQL requires a non-empty `IN` list; `x IN ()` is a syntax error there.
        empty_in_list: false,
        // PostgreSQL accepts the one-word `ISNULL`/`NOTNULL` postfix (see
        // `OperatorSyntax::null_test_postfix`) but rejects the two-word `<expr> NOT NULL`
        // (engine-measured via libpg_query): `SELECT 1 WHERE 1 NOT NULL` is a syntax error.
        null_test_two_word_postfix: false,
    };
}

impl FeatureSet {
    /// PostgreSQL as dialect data.
    pub const POSTGRES: Self = Self {
        identifier_casing: Casing::Lower,
        identifier_quotes: STANDARD_IDENTIFIER_QUOTES,
        default_null_ordering: NullOrdering::NullsLast,
        reserved_column_name: RESERVED_COLUMN_NAME,
        reserved_function_name: RESERVED_FUNCTION_NAME,
        reserved_type_name: RESERVED_TYPE_NAME,
        reserved_bare_alias: RESERVED_BARE_ALIAS,
        reserved_as_label: KeywordSet::EMPTY,
        catalog_qualified_names: true,
        // The shared M1 table plus the vertical tab (`0x0b`) as *ordinary* whitespace:
        // PostgreSQL's flex `space` set is `[ \t\n\r\f\v]`, and the vertical tab is the one
        // member Rust's `is_ascii_whitespace` (hence `STANDARD_BYTE_CLASSES`) omits. SQLite
        // and DuckDB fold `0x0b` only position-dependently (their own tables — SQLite as a
        // run continuation, DuckDB as statement-trim), so full whitespace-class membership
        // rides only this table, not the shared one (see [`POSTGRES_BYTE_CLASSES`]). `$` is
        // intentionally absent as an identifier byte:
        // PostgreSQL admits `$` as an identifier-*continue* byte through
        // `identifier_syntax.dollar_in_identifiers` (see [`IdentifierSyntax::POSTGRES`])
        // instead, which keeps it out of the identifier-*start* class so a leading `$` stays
        // a positional `$1` parameter.
        byte_classes: POSTGRES_BYTE_CLASSES,
        // PostgreSQL ranks `[NOT] BETWEEN`/`IN`/`LIKE`/`ILIKE`/`SIMILAR TO` one tier ABOVE
        // the comparison operators (gram.y `%nonassoc BETWEEN IN_P LIKE …`), so
        // `a = b BETWEEN c AND d` groups `a = (b BETWEEN c AND d)` — everything else stays
        // the standard table.
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
            // The `IS`-family predicates (`IS NULL`, `IS DISTINCT FROM`, `IS TRUE`, …) rank one
            // tier below comparison, so `a <> b IS NULL` groups `(a <> b) IS NULL`
            // (`%nonassoc IS ISNULL NOTNULL`, engine-measured on PostgreSQL 16).
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
        string_literals: StringLiteralSyntax::POSTGRES,
        numeric_literals: NumericLiteralSyntax::POSTGRES,
        parameters: ParameterSyntax::POSTGRES,
        // PostgreSQL spells `@` as a text-search / custom operator, never a session
        // variable, so the `@`/`@@` variable forms stay off (the ANSI baseline).
        session_variables: SessionVariableSyntax::ANSI,
        identifier_syntax: IdentifierSyntax::POSTGRES,
        table_expressions: TableExpressionSyntax::POSTGRES,
        join_syntax: JoinSyntax::POSTGRES,
        table_factor_syntax: TableFactorSyntax::POSTGRES,
        expression_syntax: ExpressionSyntax::POSTGRES,
        operator_syntax: OperatorSyntax::POSTGRES,
        call_syntax: CallSyntax::POSTGRES,
        string_func_forms: StringFuncForms::POSTGRES,
        aggregate_call_syntax: AggregateCallSyntax::POSTGRES,
        predicate_syntax: PredicateSyntax::POSTGRES,
        pipe_operator: PipeOperator::StringConcat,
        double_ampersand: DoubleAmpersand::Unsupported,
        keyword_operators: KeywordOperators::Unsupported,
        // PostgreSQL's `^` is exponentiation, its own precedence row (tighter than `*`).
        caret_operator: CaretOperator::Exponent,
        // PostgreSQL spells bitwise XOR `#` (engine-measured: `2 # 3` parses, `#` binds at
        // the "any other operator" rank), not `^`.
        hash_bitwise_xor: true,
        // A `--`/`#` line comment ends at `\r` as well as `\n` here (flex `non_newline`
        // is `[^\n\r]`) — the one comment-shape difference from the ANSI baseline.
        comment_syntax: CommentSyntax::POSTGRES,
        mutation_syntax: MutationSyntax::POSTGRES,
        statement_ddl_gates: StatementDdlGates::POSTGRES,
        create_table_clause_syntax: CreateTableClauseSyntax::POSTGRES,
        column_definition_syntax: ColumnDefinitionSyntax::POSTGRES,
        constraint_syntax: ConstraintSyntax::POSTGRES,
        index_alter_syntax: IndexAlterSyntax::POSTGRES,
        existence_guards: ExistenceGuards::POSTGRES,
        select_syntax: SelectSyntax::POSTGRES,
        query_tail_syntax: QueryTailSyntax::POSTGRES,
        grouping_syntax: GroupingSyntax::POSTGRES,
        utility_syntax: UtilitySyntax::POSTGRES,
        transaction_syntax: TransactionSyntax::POSTGRES,
        show_syntax: ShowSyntax::POSTGRES,
        maintenance_syntax: MaintenanceSyntax::POSTGRES,
        access_control_syntax: AccessControlSyntax::POSTGRES,
        type_name_syntax: TypeNameSyntax::POSTGRES,
        // Render PostgreSQL's canonical type spellings (`NUMERIC`, `TIMESTAMPTZ`, …) —
        // the one preset whose target spelling diverges from the ANSI baseline.
        target_spelling: TargetSpelling::Postgres,
    };
}

/// Prefer [`FeatureSet::POSTGRES`] for struct update.
pub const POSTGRES: FeatureSet = FeatureSet::POSTGRES;

// Compile-time proof the PostgreSQL preset claims no shared tokenizer trigger twice —
// notably that `subscript` and `containment_operators` (both on here) never meet a
// contending `:name`/`@name` form. The ratchet fails the build if a future edit adds
// one, rather than silently shadowing a meaning (uniform with `LENIENT`'s assert).
const _: () = assert!(FeatureSet::POSTGRES.is_lexically_consistent());
// The two sibling self-consistency registries are ratcheted the same way, so the
// parse-entry `debug_assert!` folds all three to dead code for this preset: every
// refinement flag (the locking strengths, the MERGE and extended-`ALTER TABLE` actions,
// `prepare_typed_parameters`) rides its enabled base, and no two features contend for one
// parser-position head.
const _: () = assert!(FeatureSet::POSTGRES.has_satisfied_feature_dependencies());
const _: () = assert!(FeatureSet::POSTGRES.has_no_grammar_conflict());
