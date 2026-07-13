// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Whole-tree span invariants: every span-bearing node parsed from source carries a
//! real, in-bounds, UTF-8-safe byte span, contained by its nearest span-bearing
//! ancestor — and never [`Span::SYNTHETIC`] (ADR-0002, ADR-0013).
//!
//! `make_meta` is the parser's single metadata chokepoint (see the engine): every
//! node's `Meta` is minted there from a real source span — even field-less marker
//! syntax (`*`, `CROSS JOIN`, `NATURAL`, `UNBOUNDED PRECEDING`, `NOT NULL`, …)
//! captures the span of the keyword(s) it stands for, and a parent's span is always
//! at least the union of its children's (`Span::union`). So every invariant below
//! holds *by construction*; this module is the regression guard that proves it,
//! walking representative parsed trees with the generated [`Visit`] traversal and
//! asserting, per span-bearing node:
//!
//! - it is not [`Span::SYNTHETIC`] (`tests::finder_flags_a_synthetic_span`);
//! - `start <= end <= source.len()`, in bounds for the text it was parsed from
//!   (`tests::finder_flags_an_out_of_bounds_span`);
//! - both `start` and `end` fall on a UTF-8 char boundary
//!   (`tests::finder_flags_a_non_char_boundary_span`);
//! - it is contained by the nearest still-open span-bearing ancestor — i.e.
//!   `ancestor.start() <= start` and `end <= ancestor.end()`
//!   (`tests::finder_flags_a_too_narrow_parent_span`).
//!
//! [`Span::SYNTHETIC`] is reserved for two non-parsed situations, both tested here,
//! and both exempt from the bounds/UTF-8/containment checks above: a synthesized
//! node carries no source location to check against, so [`SyntheticSpanFinder`]
//! only records it in [`SyntheticSpanFinder::synthetic`] and does not push it as an
//! ancestor (a descendant that is itself real still checks against the nearest
//! *real* ancestor further up, skipping over the synthetic one):
//!
//! - **Rewrite-only nodes.** A node a later pass *synthesizes* has no backing
//!   source text, so it uses the sentinel. M1 ships no such rewrite, so no parsed
//!   tree contains one; `tests::finder_flags_a_synthetic_span` proves the guard
//!   catches one if it ever leaks in.
//! - **Empty [`ObjectName`].** `ObjectName::span()` folds its parts from the
//!   `SYNTHETIC` identity, so a *zero-part* name is synthetic. A parsed name always
//!   has at least one part and so never is — see
//!   `tests::empty_object_name_is_synthetic_but_a_parsed_name_is_not`.
//!
//! An *absent-source* marker is distinct from a synthetic one: a bare `t JOIN u`
//! records a [`JoinConstraint::None`] for the constraint it does not have, and that
//! node carries a real *zero-width* span at the join point, not the sentinel — see
//! `tests::bare_join_constraint_none_carries_a_real_empty_span`. Zero-width is a
//! true source position (`is_empty`); synthetic is the absence of one. A real
//! zero-width span still passes containment: an empty range flush with either edge
//! of its ancestor's range is contained by definition (`start <= end` and
//! `end <= end` both hold at the boundary).
//!
//! [`assert_parsed_span_invariants`] exposes the same walk to a caller that already
//! holds a `Parsed` value, so a harness outside this module's own corpus tests can
//! run it without re-parsing — currently the `roundtrip` fuzz body
//! (`fuzz::roundtrip_statement`), which checks it against every reparsed generated
//! statement, not just the fixed/vendored corpora here.

use squonk::Parsed;
use squonk_ast::generated::visit::{self, Visit};
use squonk_ast::*;

/// Records the kind of every parsed node whose span is synthetic, every span
/// bounds/UTF-8/containment violation found, and a count of all span-bearing nodes
/// visited (a coverage floor for the corpus).
///
/// Each override is uniform: [`Spanned::span`] resolves the node's span — for a
/// struct from its `meta`, for an enum through its variant — so a single check
/// covers both shapes without matching variants, and `Statement`'s
/// `#[non_exhaustive]` needs no special case. The override then delegates to the
/// matching generated `walk_*` to descend, so a node nested anywhere is still
/// reached and checked.
#[derive(Default)]
struct SyntheticSpanFinder<'src> {
    synthetic: Vec<&'static str>,
    visited: usize,
    /// The exact text the walked tree was parsed from, for the bounds and UTF-8
    /// char-boundary checks. Empty (the `Default`) for hand-built trees exercising
    /// only the synthetic-span check, which carry no real span to check.
    source: &'src str,
    /// `source`'s byte length, precomputed once so every node check is a plain
    /// comparison.
    source_len: u32,
    /// The `(method name, span)` of every real (non-synthetic) span-bearing
    /// ancestor currently open on the walk, innermost last. A synthetic node's span
    /// is never pushed here: it carries no source location, so it cannot
    /// meaningfully bound a real descendant's containment (see the module doc
    /// comment's synthetic-exemption note).
    ancestors: Vec<(&'static str, Span)>,
    /// Every bounds/UTF-8/containment violation found, already formatted for a
    /// panic message.
    violations: Vec<String>,
}

impl<'src> SyntheticSpanFinder<'src> {
    /// Build a finder that checks every real span it visits against `source`'s
    /// byte bounds and UTF-8 char boundaries. `source` must be the exact text the
    /// tree under test was parsed from — bounds and containment are meaningless
    /// (and, for hand-built trees exercising only the synthetic-span check, unused)
    /// against any other string.
    fn new(source: &'src str) -> Self {
        let source_len =
            u32::try_from(source.len()).expect("source byte length exceeds u32 span range");
        Self {
            source,
            source_len,
            ..Self::default()
        }
    }

    /// Check one real (non-synthetic) span-bearing node's span against the
    /// source's byte bounds, its UTF-8 char boundaries, and the nearest still-open
    /// real span-bearing ancestor (the top of [`Self::ancestors`]), recording a
    /// formatted violation for each broken invariant. The three checks are
    /// independent — an out-of-bounds span can still (coincidentally) satisfy
    /// containment against an equally-wrong ancestor — so none short-circuits
    /// another, except that a span already out of bounds skips the UTF-8 check
    /// (which would be equally uninformative against bytes outside the source).
    fn check_span(&mut self, method: &'static str, span: Span) {
        let start = span.start();
        let end = span.end();

        if start > end || end > self.source_len {
            self.violations.push(format!(
                "{method}: span {start}..{end} exceeds the {}-byte source",
                self.source_len,
            ));
        } else if !self.source.is_char_boundary(start as usize)
            || !self.source.is_char_boundary(end as usize)
        {
            self.violations.push(format!(
                "{method}: span {start}..{end} does not fall on a UTF-8 char boundary",
            ));
        }

        if let Some(&(parent_method, parent)) = self.ancestors.last() {
            if start < parent.start() || end > parent.end() {
                self.violations.push(format!(
                    "{method}: span {start}..{end} escapes {parent_method}'s span \
                     {}..{} (its nearest span-bearing ancestor)",
                    parent.start(),
                    parent.end(),
                ));
            }
        }
    }
}

/// Generate one `visit_*` override per span-bearing node type: record the node if
/// its span is synthetic; otherwise check its bounds/UTF-8/containment and push it
/// as the open ancestor for its children. Either way, delegate to the matching
/// generated `walk_*` to descend, so a node nested anywhere is still reached and
/// checked. The list mirrors the generated [`Spanned`] impls; a brand-new node
/// *type* needs its line added here (the same manual step the sibling `NodeId`
/// guard documents).
macro_rules! check_spans {
    ($lt:lifetime, $(($method:ident, $walk:ident, $ty:ty)),+ $(,)?) => {
        $(
            fn $method(&mut self, node: &$lt $ty) {
                self.visited += 1;
                let span = node.span();
                if span.is_synthetic() {
                    self.synthetic.push(stringify!($method));
                    visit::$walk(self, node);
                } else {
                    self.check_span(stringify!($method), span);
                    self.ancestors.push((stringify!($method), span));
                    visit::$walk(self, node);
                    self.ancestors.pop();
                }
            }
        )+
    };
}

impl<'ast, 'src> Visit<'ast, NoExt> for SyntheticSpanFinder<'src> {
    check_spans!('ast,
        (visit_access_control_statement, walk_access_control_statement, AccessControlStatement),
        (visit_alter_column_action, walk_alter_column_action, AlterColumnAction<NoExt>),
        (visit_alter_table, walk_alter_table, AlterTable<NoExt>),
        (visit_alter_table_action, walk_alter_table_action, AlterTableAction<NoExt>),
        (visit_array_expr, walk_array_expr, ArrayExpr<NoExt>),
        (visit_at_time_zone_expr, walk_at_time_zone_expr, AtTimeZoneExpr<NoExt>),
        (visit_case_expr, walk_case_expr, CaseExpr<NoExt>),
        (visit_collate_expr, walk_collate_expr, CollateExpr<NoExt>),
        (visit_column_constraint, walk_column_constraint, ColumnConstraint<NoExt>),
        (visit_column_def, walk_column_def, ColumnDef<NoExt>),
        (visit_column_option, walk_column_option, ColumnOption<NoExt>),
        (visit_config_parameter, walk_config_parameter, ConfigParameter),
        (visit_conflict_action, walk_conflict_action, ConflictAction<NoExt>),
        (visit_conflict_target, walk_conflict_target, ConflictTarget<NoExt>),
        (visit_constraints_target, walk_constraints_target, ConstraintsTarget),
        (visit_create_index, walk_create_index, CreateIndex<NoExt>),
        (visit_create_schema, walk_create_schema, CreateSchema),
        (visit_create_table, walk_create_table, CreateTable<NoExt>),
        (visit_create_table_body, walk_create_table_body, CreateTableBody<NoExt>),
        (visit_create_table_option, walk_create_table_option, CreateTableOption<NoExt>),
        (visit_create_table_option_kind, walk_create_table_option_kind, CreateTableOptionKind<NoExt>),
        (visit_create_view, walk_create_view, CreateView<NoExt>),
        (visit_cte, walk_cte, Cte<NoExt>),
        (visit_data_type, walk_data_type, DataType),
        (visit_default_value, walk_default_value, DefaultValue),
        (visit_delete, walk_delete, Delete<NoExt>),
        (visit_dml_selection, walk_dml_selection, DmlSelection<NoExt>),
        (visit_dml_target, walk_dml_target, DmlTarget),
        (visit_drop_statement, walk_drop_statement, DropStatement),
        (visit_expr, walk_expr, Expr<NoExt>),
        (visit_extract_expr, walk_extract_expr, ExtractExpr<NoExt>),
        (visit_field_selection_expr, walk_field_selection_expr, FieldSelectionExpr<NoExt>),
        (visit_foreign_key_ref, walk_foreign_key_ref, ForeignKeyRef),
        (visit_function_call, walk_function_call, FunctionCall<NoExt>),
        (visit_generated_column, walk_generated_column, GeneratedColumn<NoExt>),
        (visit_grant_object, walk_grant_object, GrantObject),
        (visit_grantee, walk_grantee, Grantee),
        (visit_ident, walk_ident, Ident),
        (visit_identity_column, walk_identity_column, IdentityColumn<NoExt>),
        (visit_identity_option, walk_identity_option, IdentityOption<NoExt>),
        (visit_index_column, walk_index_column, IndexColumn<NoExt>),
        (visit_insert, walk_insert, Insert<NoExt>),
        (visit_insert_source, walk_insert_source, InsertSource<NoExt>),
        (visit_insert_target, walk_insert_target, InsertTarget),
        (visit_insert_value, walk_insert_value, InsertValue<NoExt>),
        (visit_insert_values, walk_insert_values, InsertValues<NoExt>),
        (visit_join, walk_join, Join<NoExt>),
        (visit_join_constraint, walk_join_constraint, JoinConstraint<NoExt>),
        (visit_join_operator, walk_join_operator, JoinOperator<NoExt>),
        (visit_limit, walk_limit, Limit<NoExt>),
        (visit_literal, walk_literal, Literal),
        (visit_named_window, walk_named_window, NamedWindow<NoExt>),
        (visit_object_name, walk_object_name, ObjectName),
        (visit_on_conflict, walk_on_conflict, OnConflict<NoExt>),
        (visit_order_by_expr, walk_order_by_expr, OrderByExpr<NoExt>),
        (visit_privilege, walk_privilege, Privilege),
        (visit_privileges, walk_privileges, Privileges),
        (visit_query, walk_query, Query<NoExt>),
        (visit_referential_action, walk_referential_action, ReferentialAction),
        (visit_returning, walk_returning, Returning<NoExt>),
        (visit_role_spec, walk_role_spec, RoleSpec),
        (visit_routine_signature, walk_routine_signature, RoutineSignature),
        (visit_row_expr, walk_row_expr, RowExpr<NoExt>),
        (visit_rows_from_item, walk_rows_from_item, RowsFromItem<NoExt>),
        (visit_select, walk_select, Select<NoExt>),
        (visit_select_distinct, walk_select_distinct, SelectDistinct<NoExt>),
        (visit_select_item, walk_select_item, SelectItem<NoExt>),
        (visit_session_statement, walk_session_statement, SessionStatement),
        (visit_set_expr, walk_set_expr, SetExpr<NoExt>),
        (visit_set_names_value, walk_set_names_value, SetNamesValue),
        (visit_set_parameter_value, walk_set_parameter_value, SetParameterValue),
        (visit_set_value, walk_set_value, SetValue),
        (visit_special_set_value, walk_special_set_value, SpecialSetValue),
        (visit_statement, walk_statement, Statement<NoExt>),
        (visit_subscript_expr, walk_subscript_expr, SubscriptExpr<NoExt>),
        (visit_table_alias, walk_table_alias, TableAlias),
        (visit_table_constraint, walk_table_constraint, TableConstraint<NoExt>),
        (visit_table_constraint_def, walk_table_constraint_def, TableConstraintDef<NoExt>),
        (visit_table_element, walk_table_element, TableElement<NoExt>),
        (visit_table_factor, walk_table_factor, TableFactor<NoExt>),
        (visit_table_function_column, walk_table_function_column, TableFunctionColumn),
        (visit_table_sample, walk_table_sample, TableSample<NoExt>),
        (visit_table_storage_parameter, walk_table_storage_parameter, TableStorageParameter<NoExt>),
        (visit_table_with_joins, walk_table_with_joins, TableWithJoins<NoExt>),
        (visit_transaction_mode, walk_transaction_mode, TransactionMode),
        (visit_transaction_statement, walk_transaction_statement, TransactionStatement),
        (visit_update, walk_update, Update<NoExt>),
        (visit_update_assignment, walk_update_assignment, UpdateAssignment<NoExt>),
        (visit_update_tuple_source, walk_update_tuple_source, UpdateTupleSource<NoExt>),
        (visit_update_value, walk_update_value, UpdateValue<NoExt>),
        (visit_upsert, walk_upsert, Upsert<NoExt>),
        (visit_values, walk_values, Values<NoExt>),
        (visit_values_item, walk_values_item, ValuesItem<NoExt>),
        (visit_when_clause, walk_when_clause, WhenClause<NoExt>),
        (visit_window_definition, walk_window_definition, WindowDefinition<NoExt>),
        (visit_window_frame, walk_window_frame, WindowFrame<NoExt>),
        (visit_window_frame_bound, walk_window_frame_bound, WindowFrameBound<NoExt>),
        (visit_window_spec, walk_window_spec, WindowSpec<NoExt>),
        (visit_with, walk_with, With<NoExt>),
    );
}

/// Assert every span-bearing node in `parsed`'s statements holds the whole-tree
/// span invariants this module's corpus tests check over the fixed/vendored
/// corpora: no synthetic span, `start <= end <= source.len()`, both offsets on a
/// UTF-8 char boundary, and containment within the nearest span-bearing ancestor.
/// Exposed for reuse by a caller that already holds a `Parsed` value — currently
/// the `roundtrip` fuzz body (`fuzz::roundtrip_statement`), which must not re-parse
/// text it just parsed to reach this check.
///
/// # Panics
///
/// Panics naming the offending node kind(s)/span(s) if any invariant is broken.
pub(crate) fn assert_parsed_span_invariants(parsed: &Parsed) {
    let mut finder = SyntheticSpanFinder::new(parsed.source());
    for statement in parsed.statements() {
        finder.visit_statement(statement);
    }
    assert!(
        finder.synthetic.is_empty(),
        "parsed nodes carried synthetic spans: {:?}",
        finder.synthetic,
    );
    assert!(
        finder.violations.is_empty(),
        "span bounds/containment violations: {:#?}",
        finder.violations,
    );
}

#[cfg(test)]
mod tests {
    use squonk::dialect::{Ansi, Postgres};
    use squonk::{Dialect, parse_with};
    use thin_vec::thin_vec;

    use super::*;

    /// Parse `sql` under `dialect`, walk every statement, and return the finder.
    fn find_synthetic_spans<D: Dialect<Ext = NoExt>>(
        sql: &str,
        dialect: D,
    ) -> SyntheticSpanFinder<'_> {
        let parsed = parse_with(sql, squonk::ParseConfig::new(dialect))
            .unwrap_or_else(|err| panic!("corpus SQL must parse: {sql:?}: {err:?}"));
        let mut finder = SyntheticSpanFinder::new(sql);
        for statement in parsed.statements() {
            finder.visit_statement(statement);
        }
        finder
    }

    /// Parse `sql` under `dialect` and assert every span-bearing node holds the
    /// whole-tree span invariants: no synthetic span, in-bounds, UTF-8-safe, and
    /// contained by its nearest span-bearing ancestor. Returns the count of
    /// span-bearing nodes the walk visited.
    fn assert_span_invariants<D: Dialect<Ext = NoExt>>(sql: &str, dialect: D) -> usize {
        let finder = find_synthetic_spans(sql, dialect);
        assert!(
            finder.synthetic.is_empty(),
            "{sql:?}: parsed nodes carried synthetic spans: {:?}",
            finder.synthetic,
        );
        assert!(
            finder.violations.is_empty(),
            "{sql:?}: span bounds/containment violations: {:#?}",
            finder.violations,
        );
        assert!(finder.visited > 0, "{sql:?}: traversal visited no nodes");
        finder.visited
    }

    /// ANSI corpus, deliberately heavy on the field-less marker syntax this guard
    /// exists for: the `*` wildcard; every join marker (`INNER`/`LEFT`/`RIGHT`/`FULL`,
    /// `CROSS`, `NATURAL`, bare `JOIN`, `USING`); window-frame bounds (`UNBOUNDED
    /// PRECEDING`, `CURRENT ROW`, `… FOLLOWING`); `DEFAULT` in `INSERT`/`UPDATE`/
    /// `VALUES`; the column-option, type-keyword, identity-option, role-spec,
    /// config-parameter, and transaction-mode markers.
    const ANSI_CORPUS: &[&str] = &[
        // Projection wildcard and qualified wildcard.
        "SELECT *, t.*, a AS x FROM s.t AS t",
        // Every join marker, including a bare `JOIN` (constraint `None`) and `USING`.
        "SELECT * FROM t1 JOIN t2 ON t1.a = t2.a",
        "SELECT * FROM t1 INNER JOIN t2 ON TRUE",
        "SELECT * FROM t1 LEFT OUTER JOIN t2 ON TRUE",
        "SELECT * FROM t1 RIGHT JOIN t2 ON TRUE",
        "SELECT * FROM t1 FULL JOIN t2 USING (id)",
        "SELECT * FROM t1 CROSS JOIN t2",
        "SELECT * FROM t1 NATURAL JOIN t2",
        "SELECT * FROM t1 NATURAL LEFT JOIN t2",
        "SELECT * FROM t1 JOIN t2",
        // Window frames exercise every field-less frame-bound marker.
        "SELECT avg(a) OVER (ORDER BY b ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM t",
        "SELECT sum(a) OVER (ORDER BY b RANGE BETWEEN 1 PRECEDING AND UNBOUNDED FOLLOWING) FROM t",
        "SELECT count(*) OVER w FROM t WINDOW w AS (PARTITION BY a ORDER BY b)",
        // SELECT quantifiers and a rich expression spread.
        "SELECT DISTINCT a, count(DISTINCT b), CASE a WHEN 1 THEN b ELSE c END, \
            CAST(a AS INTEGER), EXTRACT(year FROM a) \
         FROM t WHERE a IN (1, 2) AND b BETWEEN 1 AND 2 AND a IS NOT NULL \
         GROUP BY a HAVING count(*) > 1 ORDER BY a DESC LIMIT 10",
        "SELECT ALL a FROM t",
        // DDL: column options (NULL/NOT NULL/DEFAULT/PRIMARY KEY/UNIQUE/CHECK),
        // identity options (incl. CYCLE), generated columns, table constraints, types.
        "CREATE TABLE t (\
            id INT PRIMARY KEY, \
            name TEXT NOT NULL DEFAULT 'x', \
            flag BOOLEAN NULL, \
            code BIGINT UNIQUE, \
            n INT GENERATED ALWAYS AS (id + 1) STORED, \
            m BIGINT GENERATED ALWAYS AS IDENTITY (START WITH 10 INCREMENT BY 2 NO MINVALUE MAXVALUE 100 CACHE 5 NO CYCLE), \
            amount DECIMAL(10, 2), \
            ratio DOUBLE PRECISION, \
            label CHARACTER VARYING(20), \
            CONSTRAINT u UNIQUE (name), \
            CHECK (id > 0)\
        )",
        "CREATE TABLE t AS SELECT 1",
        "CREATE TEMPORARY TABLE t (id INT) WITH (fillfactor = 70) ON COMMIT DROP",
        // ALTER TABLE exercises the field-less SET/DROP NOT NULL and DROP DEFAULT markers.
        "ALTER TABLE t ALTER COLUMN c SET NOT NULL",
        "ALTER TABLE t ALTER COLUMN c DROP DEFAULT",
        "ALTER TABLE t ADD COLUMN c INT DEFAULT 0",
        "ALTER TABLE t DROP COLUMN c",
        "DROP TABLE t",
        // Other CREATE statements.
        "CREATE SCHEMA s",
        "CREATE VIEW v AS SELECT 1",
        "CREATE INDEX i ON t (a, b)",
        // DML: `DEFAULT` markers in every position plus DEFAULT VALUES.
        "INSERT INTO t (id, name) VALUES (1, DEFAULT), (2, 'b')",
        "INSERT INTO t DEFAULT VALUES",
        "UPDATE t AS target SET a = 1, b = DEFAULT FROM u WHERE target.id = u.id",
        "DELETE FROM t AS target USING u WHERE target.id = u.id",
        "VALUES (1, DEFAULT), (DEFAULT, 2)",
        // Set operation, CTE, derived table.
        "WITH c AS (SELECT 1) SELECT 1 UNION ALL SELECT 2",
        "SELECT * FROM (SELECT 1) AS s",
        // Transaction control markers and isolation/access/deferrable modes.
        "BEGIN; SAVEPOINT sp1; ROLLBACK TO SAVEPOINT sp1; RELEASE SAVEPOINT sp1; COMMIT",
        "START TRANSACTION ISOLATION LEVEL SERIALIZABLE, READ ONLY, NOT DEFERRABLE",
        "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
        // Session config markers: SET .. = .., SET .. TO DEFAULT, RESET ALL, SHOW, and
        // the special-cased subforms (sentinel values, constraints target/timing,
        // session characteristics modes).
        "SET search_path TO public, pg_catalog",
        "SET x TO DEFAULT",
        "SET TIME ZONE LOCAL",
        "SET LOCAL ROLE NONE",
        "SET SESSION AUTHORIZATION DEFAULT",
        "SET NAMES utf8 COLLATE utf8_bin",
        "SET CONSTRAINTS ALL DEFERRED",
        "SET CONSTRAINTS a, b IMMEDIATE",
        "SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY, DEFERRABLE",
        "RESET ALL",
        "SHOW search_path",
        // Access control: `ALL PRIVILEGES`, the `PUBLIC` role, and a `CURRENT_USER`
        // grantor exercise the field-less privilege/role markers.
        "GRANT ALL PRIVILEGES ON TABLE t TO alice WITH GRANT OPTION",
        "GRANT SELECT ON t TO PUBLIC GRANTED BY CURRENT_USER",
        "REVOKE SELECT ON t FROM alice",
    ];

    /// PostgreSQL-only constructs, so their markers are covered too: `DISTINCT ON`,
    /// `LATERAL`/table-function/`ROWS FROM` factors, `ONLY`, `TABLESAMPLE`, the
    /// `ON CONFLICT` / `RETURNING` clauses (both conflict targets and both actions),
    /// a multi-column `SET (a, b) = ...` with a `DEFAULT` tuple item, and the
    /// `WHERE CURRENT OF` selection marker.
    const POSTGRES_CORPUS: &[&str] = &[
        "SELECT DISTINCT ON (a, b) c FROM t",
        "SELECT * FROM generate_series(1, 3) WITH ORDINALITY AS g(x, ord)",
        "SELECT * FROM LATERAL ROWS FROM (generate_series(1, 2), json_to_record('{}') AS (a INTEGER)) AS r",
        "SELECT * FROM ONLY (t) AS x TABLESAMPLE BERNOULLI (10) REPEATABLE (42)",
        "SELECT * FROM t JOIN u USING (id) AS merged",
        "INSERT INTO t (id, n) VALUES (1, 2) ON CONFLICT (id) DO UPDATE SET n = excluded.n RETURNING *",
        "INSERT INTO t VALUES (1) ON CONFLICT ON CONSTRAINT t_pkey DO NOTHING",
        "UPDATE t SET (a, b) = (1, DEFAULT) WHERE id = 2 RETURNING a, id AS kept",
        "UPDATE t SET a = 1 WHERE CURRENT OF my_cursor",
        "DELETE FROM t RETURNING t.*, id AS removed",
    ];

    #[test]
    fn parsed_nodes_hold_span_invariants() {
        let mut visited = 0usize;
        for sql in ANSI_CORPUS {
            visited += assert_span_invariants(sql, Ansi);
        }
        for sql in POSTGRES_CORPUS {
            visited += assert_span_invariants(sql, Postgres);
        }
        // Guard against a silent coverage collapse: the corpus must exercise a large,
        // varied tree, not a handful of trivially-real spans.
        assert!(
            visited > 200,
            "corpus exercised only {visited} span-bearing nodes; expected many",
        );
    }

    // ---------------------------------------------------------------------------
    // Vendored corpora: the same walk over fixtures other conformance modules
    // already classify, reached cheaply by re-`include_str!`-ing their committed
    // "supported" caches rather than the raw multi-dialect fixtures (most of which
    // are deliberately still outside the parser's surface and would fail the
    // `parse_with(...).unwrap()` this walk requires). This mirrors
    // `corpus_pg_verdicts`'s and the bench corpus loader's own independent-consumer
    // split: those modules' `SPEC`/const data is private to them, so re-reading the
    // same vendored files here — rather than reaching into their internals — keeps
    // this subset self-contained (see `corpus_pg_verdicts`'s doc comment). Every
    // line/statement here is already known to parse, so no entry needs
    // re-classifying just to reach the walker.
    // ---------------------------------------------------------------------------

    const VENDORED_SQLGLOT_SUPPORTED: &str = include_str!("../corpus/sqlglot/supported.sql");
    const VENDORED_SQLLOGICTEST_SUPPORTED: &str =
        include_str!("../corpus/sqllogictest/supported.sql");
    const VENDORED_POSTGRES_REGRESS_SUPPORTED: &str =
        include_str!("../corpus/postgres/regress-supported.sql");

    /// Strip a generated `supported.sql` cache's leading SPDX/description banner,
    /// returning its one-statement-per-line body. Mirrors the filter
    /// `corpus_partition::CorpusSpec::committed_supported` applies to the same
    /// files (kept independent rather than shared, per the split noted above).
    fn vendored_statement_lines(text: &str) -> impl Iterator<Item = &str> {
        text.lines()
            .filter(|line| !line.trim_start().starts_with("--") && !line.trim().is_empty())
    }

    #[test]
    fn vendored_corpora_hold_span_invariants() {
        let mut visited = 0usize;
        for sql in vendored_statement_lines(VENDORED_SQLGLOT_SUPPORTED) {
            visited += assert_span_invariants(sql, Ansi);
        }
        for sql in vendored_statement_lines(VENDORED_SQLLOGICTEST_SUPPORTED) {
            visited += assert_span_invariants(sql, Ansi);
        }
        // Unlike the one-statement-per-line caches above, this fixture is
        // multi-line, semicolon-terminated statements behind one shared SPDX/
        // provenance comment header, so the whole banner-and-all text parses in a
        // single call — exactly how `pg::assert_structural_parity` already
        // consumes this same fixture.
        visited += assert_span_invariants(VENDORED_POSTGRES_REGRESS_SUPPORTED, Postgres);

        // Guard against a silent coverage collapse, as the fixed-corpus test does.
        assert!(
            visited > 3000,
            "vendored corpora exercised only {visited} span-bearing nodes; expected many",
        );
    }

    /// A `Meta` whose span is the rewrite sentinel, for hand-built nodes that stand in
    /// for a synthesized (non-parsed) node.
    fn synthetic_meta() -> Meta {
        Meta::new(Span::SYNTHETIC, NodeId::new(1).expect("non-zero node id"))
    }

    #[test]
    fn finder_flags_a_synthetic_span() {
        // Simulate a rewrite-synthesized node that leaked the sentinel: a wrapping
        // `Expr` and its inner `Literal` both carry `Span::SYNTHETIC`. The walk must
        // surface both — this is the teeth behind the corpus assertion.
        let expr = Expr::Literal {
            literal: Literal {
                kind: LiteralKind::Integer,
                meta: synthetic_meta(),
            },
            meta: synthetic_meta(),
        };

        let mut finder = SyntheticSpanFinder::default();
        finder.visit_expr(&expr);

        assert_eq!(
            finder.synthetic,
            vec!["visit_expr", "visit_literal"],
            "both the Expr and its nested Literal must be flagged",
        );
        assert!(
            finder.violations.is_empty(),
            "a synthetic span is exempt from bounds/containment, not a violation: {:?}",
            finder.violations,
        );
    }

    #[test]
    fn finder_flags_a_too_narrow_parent_span() {
        // Simulate a `make_meta`/`Span::union` regression: the wrapping `Expr` claims
        // a span narrower than its own child `Literal` — e.g. a parent that captured
        // only its leading keyword while a child spans further. The walk must surface
        // the containment violation; this is the teeth behind the corpus assertion.
        let source = "1234567890";
        let expr = Expr::Literal {
            literal: Literal {
                kind: LiteralKind::Integer,
                meta: Meta::new(Span::new(0, 10), NodeId::new(1).expect("non-zero node id")),
            },
            // The parent claims only "12" (0..2) even though its own child spans the
            // whole source — a legitimate parent always spans at least as much as
            // each child (`Span::union`'s contract).
            meta: Meta::new(Span::new(0, 2), NodeId::new(2).expect("non-zero node id")),
        };

        let mut finder = SyntheticSpanFinder::new(source);
        finder.visit_expr(&expr);

        assert!(
            finder.synthetic.is_empty(),
            "every span here is real, not synthetic: {:?}",
            finder.synthetic,
        );
        assert_eq!(
            finder.violations.len(),
            1,
            "the too-narrow parent must surface exactly one containment violation: {:?}",
            finder.violations,
        );
        assert!(
            finder.violations[0].contains("visit_literal"),
            "the violation must name the offending child node: {:?}",
            finder.violations[0],
        );
    }

    #[test]
    fn finder_flags_an_out_of_bounds_span() {
        // A 5-byte source but a span claiming to reach byte 10: both the wrapping
        // `Expr` and its child `Literal` share the same out-of-bounds span, so no
        // ancestor mismatch is in play — this isolates the `end <= source.len()`
        // check specifically.
        let source = "abcde";
        let expr = Expr::Literal {
            literal: Literal {
                kind: LiteralKind::Integer,
                meta: Meta::new(Span::new(0, 10), NodeId::new(1).expect("non-zero node id")),
            },
            meta: Meta::new(Span::new(0, 10), NodeId::new(2).expect("non-zero node id")),
        };

        let mut finder = SyntheticSpanFinder::new(source);
        finder.visit_expr(&expr);

        assert_eq!(
            finder.violations.len(),
            2,
            "both the Expr and its Literal claim an out-of-bounds span: {:?}",
            finder.violations,
        );
        assert!(
            finder.violations.iter().all(|v| v.contains("exceeds")),
            "{:?}",
            finder.violations,
        );
    }

    #[test]
    fn finder_flags_a_non_char_boundary_span() {
        // "é" is a 2-byte UTF-8 sequence at byte offset 0..2; a span ending at byte 1
        // splits it. Both the wrapping `Expr` and its child `Literal` share that
        // span, so no ancestor mismatch is in play — this isolates the char-boundary
        // check specifically.
        let source = "é";
        assert_eq!(source.len(), 2, "the test relies on a 2-byte encoding");
        let expr = Expr::Literal {
            literal: Literal {
                kind: LiteralKind::Integer,
                meta: Meta::new(Span::new(0, 1), NodeId::new(1).expect("non-zero node id")),
            },
            meta: Meta::new(Span::new(0, 1), NodeId::new(2).expect("non-zero node id")),
        };

        let mut finder = SyntheticSpanFinder::new(source);
        finder.visit_expr(&expr);

        assert_eq!(
            finder.violations.len(),
            2,
            "both the Expr and its Literal split the same UTF-8 character: {:?}",
            finder.violations,
        );
        assert!(
            finder
                .violations
                .iter()
                .all(|v| v.contains("UTF-8 char boundary")),
            "{:?}",
            finder.violations,
        );
    }

    #[test]
    fn bare_join_constraint_none_carries_a_real_empty_span() {
        // A bare `t JOIN u` has no `ON`/`USING`/`NATURAL`, so the parser records a
        // `JoinConstraint::None` for the constraint that is absent from the source. That
        // marker is an *absent-source* case, not a synthetic one: it carries a real
        // zero-width span at the join point, so it is `is_empty` (a true position) and
        // never `is_synthetic` (no position at all).
        let parsed = parse_with("SELECT * FROM t1 JOIN t2", squonk::ParseConfig::new(Ansi))
            .expect("bare JOIN parses");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let JoinOperator::Inner {
            constraint: constraint @ JoinConstraint::None { .. },
            ..
        } = &select.from[0].joins[0].operator
        else {
            panic!("expected a bare inner join with no constraint");
        };
        let span = constraint.span();
        assert!(
            !span.is_synthetic(),
            "an absent constraint is not synthetic"
        );
        assert!(span.is_empty(), "it is a real zero-width source position");
    }

    #[test]
    fn empty_object_name_is_synthetic_but_a_parsed_name_is_not() {
        // `ObjectName::span()` folds its parts from the `SYNTHETIC` identity, so a
        // zero-part name — only ever built by hand or a rewrite, never by the parser —
        // is synthetic.
        let empty = ObjectName(thin_vec![]);
        assert!(
            empty.span().is_synthetic(),
            "a part-less object name has no source extent",
        );

        // A parsed name always has at least one part, so its folded span is real.
        let parsed = parse_with("SELECT * FROM s.t", squonk::ParseConfig::new(Ansi))
            .expect("qualified name parses");
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let TableFactor::Table { name, .. } = &select.from[0].relation else {
            panic!("expected a plain table factor");
        };
        assert_eq!(name.0.len(), 2, "schema-qualified `s.t`");
        assert!(
            !name.span().is_synthetic(),
            "a parsed object name spans its parts",
        );
    }
}
