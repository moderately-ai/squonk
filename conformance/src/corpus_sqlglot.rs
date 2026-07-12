// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! sqlglot identity-corpus conformance — a broad round-trip target.
//!
//! The vendored fixture `corpus/sqlglot/identity.sql` is sqlglot's
//! transpiler-identity test data: 955 single-line statements/expressions that
//! sqlglot round-trips across many dialects. It is byte-for-byte upstream (see
//! `corpus/sqlglot/README.md` for provenance and license), so most lines exceed
//! the M1 ANSI surface — that is the point. This corpus is a *growing* coverage
//! target: statements move into the supported subset as the parser grows.
//!
//! ## Partition model
//!
//! The split is decided by *running* every line through the parser, never by
//! hand. Each line falls into exactly one class:
//!
//! - **Ansi-supported** — parses and round-trips under `Ansi` in both the
//!   canonical and fully-parenthesized oracles. These are materialized into the
//!   regenerable `corpus/sqlglot/supported.sql` and driven through the public
//!   [`assert_roundtrips`](crate::assert_roundtrips) /
//!   [`assert_roundtrips_parenthesized`](crate::assert_roundtrips_parenthesized)
//!   oracles.
//! - **Postgres-only** — needs the `Postgres` preset to parse and round-trip
//!   (the Ansi oracle cannot reach them). Pinned in `SPEC.postgres_only_supported`
//!   and validated under Postgres, so coverage credited here stays honest.
//! - **Round-trip defect** — parses but fails a round-trip oracle. Pinned in
//!   `SPEC.ansi_roundtrip_defects` and tracked under
//!   `ROUNDTRIP_DEFECT_TICKET`; this mirrors the PostgreSQL guide machinery,
//!   which keeps unsupported cases ticketed instead of silently dropped.
//! - **Unparsed** — outside the current surface; tracked implicitly because
//!   `identity.sql` keeps every line. There is no separate `guide.sql`: the
//!   PostgreSQL corpus needs a guide file because it vendors no full upstream
//!   file, whereas `identity.sql` *is* the full corpus, so a guide file would
//!   only duplicate the unsupported majority. The test instead machine-checks
//!   that every line is accounted for in exactly one class.
//!
//! ## Anti-vanishing guarantees
//!
//! `SPEC.pinned_total` pins the vendored statement count, so a line disappearing
//! from the fixture trips the gate. `supported.sql` is regenerable: running with
//! `REWRITE=1` rewrites it from the current classification, so coverage changes
//! surface as a reviewable diff rather than silently. The two small pinned
//! lists are const-compared, so a Postgres-only case promoting to Ansi, a defect
//! getting fixed, or a fresh regression all fail loudly.
//!
//! ## Lenient
//!
//! Outside the Ansi/Postgres partition above, every vendored line that parses under
//! [`Lenient`](squonk::dialect::Lenient) is separately asserted to round-trip
//! under it ([`assert_accepted_lines_round_trip`](crate::corpus_roundtrip::assert_accepted_lines_round_trip)).
//! Unpinned on purpose: Lenient's whole point is a wide, still-growing acceptance
//! boundary, so only "whatever it accepts stays round-trip-stable" is checked here,
//! not the boundary itself.

use crate::corpus_partition::CorpusSpec;

const SPEC: CorpusSpec = CorpusSpec {
    log_label: "sqlglot",
    raw_text: include_str!("../corpus/sqlglot/identity.sql"),
    pinned_total: 955,
    supported_sql: include_str!("../corpus/sqlglot/supported.sql"),
    supported_relpath: "corpus/sqlglot/supported.sql",
    banner: "\
-- SPDX-License-Identifier: MIT
--
-- sqlglot identity-corpus supported subset (GENERATED — do not edit by hand).
-- Regenerate: REWRITE=1 cargo nextest run -p squonk-conformance corpus_sqlglot
--
-- Each line parses and round-trips under the Ansi dialect in both the
-- canonical and fully-parenthesized oracles. Lines are verbatim copies of
-- corpus/sqlglot/identity.sql, kept in source order. See README.md.

",
    postgres_only_supported: &[
        "SELECT * FROM db.FOO()",
        // PostgreSQL `DISTINCT ON (...)` (prod-sql-select-distinct-qualifiers): gated by
        // select_syntax.distinct_on, so only the Postgres preset reaches these lines.
        "SELECT DISTINCT ON (x) x, y FROM z",
        "SELECT DISTINCT ON (x, y + 1) * FROM z",
        "SELECT DISTINCT ON (x.y) * FROM z",
        // PostgreSQL's `->` JSON-access operator (pg-at-family-containment-operators):
        // sqlglot writes these as lambda arrows, but syntactically PostgreSQL parses each
        // `x -> y` as the json-arrow operator, so the enclosing function calls parse only
        // under the Postgres preset (gated by operator_syntax.json_arrow_operators).
        "SELECT TRANSFORM(a, b -> b) AS x",
        "SELECT AGGREGATE(a, (a, b) -> a + b) AS x",
        "SELECT X((a, b) -> a + b, z -> z) AS x",
        "SELECT X(a -> a + (\"z\" - 1))",
        // PostgreSQL expression extensions (prod-sql-expr-postgres): the implicit row
        // constructor and array subscripting reach these lines under the Postgres preset.
        "SELECT a FROM test WHERE (a, b) IN (SELECT 1, 2)",
        // Grouping-set GROUP BY items (model-group-by-grouping-sets-rollup-cube) whose
        // members include a multi-column parenthesized set `(x, y)` — an implicit row
        // constructor gated off under Ansi (expression_syntax.row_constructor), so the
        // whole statement parses only under the Postgres preset even though
        // `grouping_sets` is on for both.
        "SELECT a FROM test GROUP BY GROUPING SETS (x, (x, y), (x, y, z), q)",
        "SELECT a FROM test GROUP BY GROUPING SETS ((x, y)), ROLLUP (b)",
        // `SELECT CASE … END['a']` is intentionally absent: a subscript on a bare CASE is
        // a PG-parity reject (tighten-pg-overacceptance-trio), so it is Unparsed. The two
        // CASE lines below subscript inside the operand (`x[0]`), which stays valid.
        "SELECT CASE TEST(1) + x[0] WHEN 1 THEN 1 ELSE 2 END",
        "SELECT CASE x[0] WHEN 1 THEN 1 ELSE 2 END",
        "SELECT a['1'], b[0], x.c[0], \"x\".d['1'] FROM x",
        "SELECT student, score FROM tests CROSS JOIN UNNEST(scores) AS t(score)",
        "SELECT student, score FROM tests CROSS JOIN UNNEST(scores) AS t(a, b)",
        "SELECT student, score FROM tests CROSS JOIN UNNEST(scores) WITH ORDINALITY AS t(a, b)",
        "SELECT student, score FROM tests CROSS JOIN UNNEST(x.scores) AS t(score)",
        // CREATE INDEX with a `USING <method>` access-method clause (prod-sql-ddl-schema-view-index):
        // the clause is gated off under Ansi, so this parses only under the Postgres preset.
        "CREATE INDEX pointloc ON points USING GIST(BOX(location, location))",
        // PostgreSQL DROP with IF EXISTS (prod-sql-ddl-alter-drop): the existence guard is
        // gated off under Ansi, so these parse and round-trip only under the Postgres preset.
        "DROP TABLE IF EXISTS a",
        "DROP TABLE IF EXISTS a.b",
        "DROP VIEW IF EXISTS a",
        "DROP VIEW IF EXISTS a.b",
        // `y[1]` array subscript needs the Postgres preset (prod-sql-expr-postgres).
        "INSERT INTO x VALUES (1, 'a', 2.0), (1, 'a', 3.0), (X(), y[1], z.x)",
        // PostgreSQL RETURNING (prod-sql-dml-returning-conflict): gated off under Ansi, so
        // these now parse and round-trip only under the Postgres preset.
        "INSERT INTO \"tests_user\" (\"username\", \"first_name\", \"last_name\") VALUES ('fiara', 'Fiara', 'Ironhide') RETURNING \"tests_user\".\"id\"",
        "SELECT 1 FROM PARQUET_SCAN('/x/y/*') AS y",
        "UPDATE products SET price = price * 1.10 WHERE price <= 99.99 RETURNING name, price AS new_price",
        // PostgreSQL `COMMENT ON` (close-p0-datafusion-parity-coverage-gaps): gated by
        // utility_syntax.comment_on, so these parse only under the Postgres preset. The
        // sibling `COMMENT ON TABLE ... IS N'National String'` line stays Unparsed: `N'...'`
        // is not a bare `Sconst` in PostgreSQL, so both engines reject it.
        "COMMENT ON COLUMN my_schema.my_table.my_column IS 'Employee ID number'",
        "COMMENT ON DATABASE my_database IS 'Development Database'",
        "COMMENT ON PROCEDURE my_proc(integer, integer) IS 'Runs a report'",
        "COMMENT ON TABLE my_schema.my_table IS 'Employee Information'",
        // PostgreSQL ALTER TABLE with IF [NOT] EXISTS (prod-sql-ddl-alter-drop): the
        // existence guard is gated off under Ansi.
        "ALTER TABLE integers ADD COLUMN IF NOT EXISTS k INT",
        "ALTER TABLE IF EXISTS integers ADD COLUMN k INT",
        "ALTER TABLE integers DROP COLUMN IF EXISTS k",
        "ALTER TABLE mydataset.mytable DROP COLUMN A, DROP COLUMN IF EXISTS B",
        "ALTER TABLE mydataset.mytable ADD COLUMN A TEXT, ADD COLUMN IF NOT EXISTS B INT",
        "SELECT * FROM UNNEST(x) WITH ORDINALITY UNION ALL SELECT * FROM UNNEST(y) WITH ORDINALITY",
        // More grouping sets whose parenthesized members are multi-column row
        // constructors / expression rows (`(x + y, z)`, `(a + 1, b * 1)`), gated off
        // under Ansi, so these reach the parseable surface only under Postgres.
        "SELECT a FROM test GROUP BY GROUPING SETS ((x + y, z))",
        "SELECT * FROM tbl GROUP BY GROUPING SETS ((a + 1, b * 1), c, CUBE (a, b), ROLLUP (c, d), (a + y, b * 1), ())",
        // The `ILIKE` pattern-match predicate (predicate_syntax.ilike) is gated off under
        // Ansi, so this case-insensitive match parses only under the Postgres preset.
        "SELECT 'Ac' ILIKE 'a%c' ESCAPE NULL",
        // The `-|-` range-adjacency operator lexes only under the general operator surface
        // (operator_syntax.custom_operators), gated off under Ansi, so it parses only under
        // the Postgres preset (pg-operator-surface-regex-geometric-network).
        "SELECT NUMRANGE(1.1, 2.2) -|- NUMRANGE(2.2, 3.3)",
    ],
    ansi_roundtrip_defects: &[],
};

#[cfg(test)]
mod tests {
    use squonk::dialect::Lenient;

    use super::SPEC;

    #[test]
    fn vendored_corpus_is_pinned_and_fully_partitioned() {
        SPEC.vendored_corpus_is_pinned_and_fully_partitioned();
    }

    #[test]
    fn ansi_supported_subset_round_trips_under_both_oracles() {
        SPEC.ansi_supported_subset_round_trips_under_both_oracles();
    }

    #[test]
    fn postgres_only_supported_round_trips_under_postgres() {
        SPEC.postgres_only_supported_round_trips_under_postgres();
    }

    #[test]
    fn ansi_roundtrip_defects_remain_ticketed_and_still_diverge() {
        SPEC.ansi_roundtrip_defects_remain_ticketed_and_still_diverge();
    }

    /// Lenient has no pinned coverage tier of its own (it is the permissive union,
    /// not a growing-surface target like Ansi/Postgres above): every vendored line
    /// it *does* accept must still round-trip under it.
    #[test]
    fn lenient_accepted_lines_round_trip_under_lenient() {
        crate::corpus_roundtrip::assert_accepted_lines_round_trip(SPEC.raw_text, Lenient);
    }
}
