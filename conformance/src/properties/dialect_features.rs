// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Flag-aware generative surfaces for the per-dialect oracle-parity programme.
//!
//! # ANSI source of truth (oracle-parity-ansi)
//!
//! There is **no ANSI SQL engine** to prepare against. The baseline's ground truth is
//! therefore a *composed* oracle, not a single process:
//!
//! 1. **Generative structural round-trip** — `parse(render(x)) == x` over
//!    [`arb_statement`](super::arb_statement) under the [`Ansi`](squonk::dialect::Ansi)
//!    preset. This is the strongest ANSI-native check: the generator is deliberately
//!    ANSI-clean (dialect-gated surfaces quarantined), so both-accept here proves the
//!    render/parse path is faithful on the standard surface.
//! 2. **Corpus pins** — sqllogictest + generated-corpus lines under Ansi
//!    (`corpus_sqllogictest`, `corpus_generated`).
//! 3. **Nearest-engine reference (PostgreSQL)** — libpg_query as a *delta* oracle: when
//!    ANSI and PG disagree, the divergence is ledgered (allowlist / ticket), not silently
//!    treated as ANSI truth. PG is not the ANSI standard; it is the closest freely
//!    runnable ParseOnly engine.
//!
//! The SQLite misfeature probes below are the *dialect-parity* pattern (engine vs
//! preset). ANSI's counterpart is the expanded `arb_statement` surface + the
//! deterministic seed/exploration gates in `properties` tests, not a second engine.
//!
//! The generic [`arb_statement`](super::arb_statement) generator is deliberately
//! ANSI-clean: it round-trips through the `Ansi` reparse path, so every dialect-gated
//! surface is quarantined (see the `# … dialect-gated …` comments throughout
//! `generators.rs`). That makes it a good *round-trip* oracle but a poor *dialect
//! parity* oracle — it never emits the gated forms where a fitted preset and its
//! engine can actually disagree.
//!
//! This module adds the missing lane: small, self-contained generative fragments,
//! each keyed to the [`FeatureSet`] flag that gates it, so a fragment is emitted only
//! under a preset that enables its surface. The differential driver renders the
//! fragment, parses it under the dialect's preset, and compares the verdict against
//! the dialect's engine oracle (SQLite's bundled `rusqlite`, DuckDB's `libduckdb`, …).
//!
//! # The reusable pattern (why 12 more dialects copy this, not fork it)
//!
//! The unit of reuse is the [`FeatureProbe`]: a `(family, applies, arb)` triple. The
//! `applies: fn(&FeatureSet) -> bool` predicate reads the *exact* gate flag off any
//! preset, so one probe table self-selects per dialect — [`applicable_probes`] hands
//! back the subset a given `FeatureSet` enables, and [`arb_feature_statement`] unions
//! their strategies. A new dialect writes its own probe table (or reuses entries whose
//! `applies` already matches its flags) and drives the *same* differential machinery:
//! the `Verdict`/`Quadrant`/`RejectReason` classifier from `verdict_harness`, the
//! `DivergenceEntry` ledger, and the committed-seed replay gate. Nothing in the driver
//! is SQLite-specific; only the probe table and the engine oracle are.
//!
//! The probes emit **SQL text** over the generator's fixed schema names (table `t`,
//! columns `a`/`b`/`c`, index `ix` — see [`FEATURE_SCHEMA_SETUP_SQL`]) rather than
//! rendering an AST under a dialect *target*. The gated misfeature shapes here
//! (`DataType::Liberal` multi-word affinity names, the `INDEXED BY` typed field, the
//! string-literal identifier quote style) have no faithful Tier-1 render under a
//! non-ANSI target today (`TargetSpelling::Ansi` for SQLite), so text fragments are
//! the honest generative surface; the render-under-target lane is a sized follow-up
//! (see the ticket). The fragments are still genuinely generative: proptest composes
//! the column counts, type-word runs, collation names, and directive spellings into
//! novel combinations the hand-authored corpora do not enumerate.

use proptest::prelude::*;
use squonk::ast::dialect::FeatureSet;

/// The schema the generative differential provisions before comparing schema-dependent
/// fragments (the m2 setup-driver pattern): a table plus one index, matching the fixed
/// names the probes reference (`t`, columns `a`/`b`/`c`, index `ix`). A fragment that
/// *creates* `t` is instead compared bare (the engine's bare-accept covers the
/// CREATE-under-test self-collision, exactly as `Verdict::engine_accepts`).
pub const FEATURE_SCHEMA_SETUP_SQL: &str =
    "CREATE TABLE t (a INTEGER, b TEXT, c INTEGER); CREATE INDEX ix ON t (a)";

/// One flag-gated generative surface: the misfeature *family* label (for divergence
/// reports and the quadrant inventory), the `applies` predicate that reads the gate
/// flag off a [`FeatureSet`] (so the probe self-selects per dialect), and the proptest
/// `arb` strategy that renders the surface as SQL over the fixed schema names.
pub struct FeatureProbe {
    /// Stable misfeature-family label.
    pub family: &'static str,
    /// Whether a preset enables this gated surface. Reads the exact gate flag, so the
    /// same probe table is reusable across dialects — each `FeatureSet` selects its own
    /// applicable subset.
    pub applies: fn(&FeatureSet) -> bool,
    /// A strategy producing SQL that exercises the gated surface. Boxed so a
    /// heterogeneous probe table composes into one `Union`.
    pub arb: fn() -> BoxedStrategy<String>,
}

/// The probes a `FeatureSet` enables, in table order (stable for reproducible sweeps).
pub fn applicable_probes<'a>(
    features: &FeatureSet,
    probes: &'a [FeatureProbe],
) -> Vec<&'a FeatureProbe> {
    probes
        .iter()
        .filter(|probe| (probe.applies)(features))
        .collect()
}

/// A strategy over the SQL of every probe `features` enables, tagged with its family.
///
/// The union is uniform over the applicable probes; each probe's own strategy owns its
/// internal shape distribution. Panics if `features` enables no probe in `probes` (a
/// misconfiguration — the caller pairs a preset with a probe table that covers it).
pub fn arb_feature_statement(
    features: &FeatureSet,
    probes: &'static [FeatureProbe],
) -> impl Strategy<Value = (&'static str, String)> {
    let applicable = applicable_probes(features, probes);
    assert!(
        !applicable.is_empty(),
        "no FeatureProbe applies to the given FeatureSet; pair it with a covering probe table",
    );
    let strategies: Vec<BoxedStrategy<(&'static str, String)>> = applicable
        .into_iter()
        .map(|probe| {
            let family = probe.family;
            (probe.arb)().prop_map(move |sql| (family, sql)).boxed()
        })
        .collect();
    proptest::strategy::Union::new(strategies)
}

// ---------------------------------------------------------------------------
// Small shared fragment strategies (fixed schema names)
// ---------------------------------------------------------------------------

/// A fixed column name from the generator's schema (`a`/`b`/`c`).
fn arb_column() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("a"), Just("b"), Just("c")]
}

/// A SQLite collation name (the three built-ins; a planner reads any bare identifier).
fn arb_collation() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("nocase"), Just("binary"), Just("rtrim")]
}

/// An optional `ASC`/`DESC` sort qualifier suffix (empty, ` ASC`, or ` DESC`).
fn arb_sort_suffix() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just(""), Just(" ASC"), Just(" DESC")]
}

// ---------------------------------------------------------------------------
// The SQLite misfeature family (oracle-parity-sqlite)
// ---------------------------------------------------------------------------

/// The SQLite misfeature-family probes: the gated surfaces the ticket names (liberal
/// types, string-literal identifiers, `INDEXED BY`, constraint collate/`DESC`, the
/// `NOT NULL`/`ISNULL` postfix null tests, bare `IS`, typeless columns) plus the
/// adjacent one-flag surfaces (`==`, bitwise operators, empty `IN ()`, string alias,
/// the `LIMIT o, c` comma form, inline PK ordering/`AUTOINCREMENT`, `WITHOUT ROWID`,
/// `STRICT`, column conflict clause, named column `COLLATE`). Every `applies` reads the
/// SQLite gate flag, so the table is the reusable unit — a sibling dialect's table
/// keys the same predicates off its own preset.
pub static SQLITE_FEATURE_PROBES: &[FeatureProbe] = &[
    FeatureProbe {
        family: "typeless-columns",
        applies: |f| f.column_definition_syntax.typeless_column_definitions,
        arb: || {
            prop::collection::vec(arb_column(), 1..4)
                .prop_map(|cols| format!("CREATE TABLE t ({})", cols.join(", ")))
                .boxed()
        },
    },
    FeatureProbe {
        family: "liberal-type-names",
        applies: |f| f.type_name_syntax.liberal_type_names,
        arb: || {
            prop_oneof![
                Just("UNSIGNED BIG INT".to_string()),
                Just("LONG INTEGER".to_string()),
                Just("NATIONAL CHARACTER".to_string()),
                (1u32..500, 1u32..500).prop_map(|(m, n)| format!("VARCHAR({m},{n})")),
                (1u32..40, 1u32..40).prop_map(|(m, n)| format!("FLOATING POINT({m},{n})")),
            ]
            .prop_map(|ty| format!("CREATE TABLE t (a {ty})"))
            .boxed()
        },
    },
    FeatureProbe {
        family: "string-literal-identifiers",
        applies: |f| f.identifier_syntax.string_literal_identifiers,
        arb: || {
            prop_oneof![
                Just("DELETE FROM 't'".to_string()),
                arb_column().prop_map(|c| format!("CREATE TABLE t (a, b, PRIMARY KEY('{c}'))")),
                arb_column().prop_map(|c| format!("CREATE TABLE t (a, b, UNIQUE('{c}'))")),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "indexed-by",
        applies: |f| f.table_expressions.indexed_by,
        arb: || {
            prop_oneof![
                Just("SELECT * FROM t INDEXED BY ix".to_string()),
                Just("SELECT * FROM t NOT INDEXED".to_string()),
                Just("SELECT * FROM t AS e INDEXED BY ix".to_string()),
                Just("SELECT * FROM t AS e NOT INDEXED".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "constraint-collate-order",
        applies: |f| f.constraint_syntax.constraint_column_collate_order,
        arb: || {
            (arb_column(), arb_collation(), arb_sort_suffix())
                .prop_map(|(col, coll, sort)| {
                    format!("CREATE TABLE t (a, b, PRIMARY KEY({col} COLLATE {coll}{sort}))")
                })
                .boxed()
        },
    },
    FeatureProbe {
        family: "not-null-two-word-postfix",
        applies: |f| f.predicate_syntax.null_test_two_word_postfix,
        arb: || {
            prop_oneof![
                Just("SELECT 1 WHERE 1 NOT NULL".to_string()),
                Just("SELECT 1 WHERE 1 NOTNULL".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "null-test-postfix",
        applies: |f| f.operator_syntax.null_test_postfix,
        arb: || {
            prop_oneof![
                Just("SELECT 1 WHERE 1 ISNULL".to_string()),
                Just("SELECT 1 WHERE 1 NOTNULL".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "bare-is-general-equality",
        applies: |f| f.operator_syntax.is_general_equality,
        arb: || {
            prop_oneof![
                Just("SELECT 1 IS 1".to_string()),
                Just("SELECT 1 IS NOT 2".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "double-equals",
        applies: |f| f.operator_syntax.double_equals,
        arb: || Just("SELECT 1 == 1".to_string()).boxed(),
    },
    FeatureProbe {
        family: "inline-primary-key-ordering",
        applies: |f| f.column_definition_syntax.inline_primary_key_ordering,
        arb: || {
            arb_sort_suffix()
                .prop_filter("needs an explicit sort", |s| !s.is_empty())
                .prop_map(|sort| format!("CREATE TABLE t (a INTEGER PRIMARY KEY{sort})"))
                .boxed()
        },
    },
    FeatureProbe {
        family: "joined-autoincrement",
        applies: |f| f.column_definition_syntax.joined_autoincrement_attribute,
        arb: || Just("CREATE TABLE t (a INTEGER PRIMARY KEY AUTOINCREMENT)".to_string()).boxed(),
    },
    FeatureProbe {
        family: "column-conflict-clause",
        applies: |f| f.column_definition_syntax.column_conflict_resolution_clause,
        arb: || {
            prop_oneof![
                Just("REPLACE"),
                Just("IGNORE"),
                Just("ABORT"),
                Just("FAIL"),
                Just("ROLLBACK"),
            ]
            .prop_map(|action| format!("CREATE TABLE t (a INTEGER UNIQUE ON CONFLICT {action})"))
            .boxed()
        },
    },
    FeatureProbe {
        family: "named-column-collate",
        applies: |f| f.column_definition_syntax.named_column_collate_constraint,
        arb: || {
            arb_collation()
                .prop_map(|coll| format!("CREATE TABLE t (a TEXT CONSTRAINT c COLLATE {coll})"))
                .boxed()
        },
    },
    FeatureProbe {
        family: "without-rowid",
        applies: |f| f.create_table_clause_syntax.without_rowid_table_option,
        arb: || Just("CREATE TABLE t (a INTEGER PRIMARY KEY) WITHOUT ROWID".to_string()).boxed(),
    },
    FeatureProbe {
        family: "strict-table",
        applies: |f| f.create_table_clause_syntax.strict_table_option,
        arb: || Just("CREATE TABLE t (a INTEGER) STRICT".to_string()).boxed(),
    },
    FeatureProbe {
        family: "bitwise-operators",
        applies: |f| f.operator_syntax.bitwise_operators,
        arb: || {
            prop_oneof![
                Just("SELECT 1 | 2".to_string()),
                Just("SELECT 3 & 2".to_string()),
                Just("SELECT ~5".to_string()),
                Just("SELECT 1 << 4".to_string()),
                Just("SELECT 8 >> 1".to_string()),
                Just("SELECT 1 | 2 & 3 << 1 >> 1".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "empty-in-list",
        applies: |f| f.predicate_syntax.empty_in_list,
        arb: || {
            prop_oneof![
                Just("SELECT 1 WHERE 1 IN ()".to_string()),
                Just("SELECT 1 WHERE 1 NOT IN ()".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "alias-string-literal",
        applies: |f| f.select_syntax.alias_string_literals,
        arb: || Just("SELECT 1 AS 'x'".to_string()).boxed(),
    },
    FeatureProbe {
        family: "limit-offset-comma",
        applies: |f| f.query_tail_syntax.limit_offset_comma,
        arb: || {
            (0u32..100, 0u32..100)
                .prop_map(|(offset, count)| format!("SELECT 1 LIMIT {offset}, {count}"))
                .boxed()
        },
    },
];

/// Committed deterministic seeds for the SQLite generative differential replay gate —
/// one or more per misfeature family, each engine-verified to be accepted by both the
/// fitted `Sqlite` preset and bundled SQLite (no over-acceptance), mirroring
/// `fuzz::PG_DIFFERENTIAL_RAW_BYTES_REPLAYS`. The replay gate
/// (`corpus_sqlite_verdicts::sqlite_feature_generative_differential_replays_seeds`)
/// asserts no un-allowlisted over-acceptance over these, so a preset tightening or a
/// fresh divergence trips it deterministically (never a flaky always-random gate). A
/// fuzz/proptest-found, minimized divergence lands here alongside its triage.
pub const SQLITE_MISFEATURE_SEEDS: &[&str] = &[
    // typeless columns
    "CREATE TABLE t (a, b)",
    "CREATE TABLE t (a, b, c)",
    // liberal (multi-word affinity) type names
    "CREATE TABLE t (a UNSIGNED BIG INT)",
    "CREATE TABLE t (a LONG INTEGER)",
    "CREATE TABLE t (a VARCHAR(123,456))",
    // string-literal identifiers
    "DELETE FROM 't'",
    "CREATE TABLE t (a, b, PRIMARY KEY('a'))",
    // INDEXED BY / NOT INDEXED (needs the provisioned schema)
    "SELECT * FROM t INDEXED BY ix",
    "SELECT * FROM t NOT INDEXED",
    "SELECT * FROM t AS e INDEXED BY ix",
    // constraint indexed-column COLLATE + ASC/DESC
    "CREATE TABLE t (a, b, PRIMARY KEY(a COLLATE nocase DESC))",
    "CREATE TABLE t (a, b, UNIQUE(b COLLATE binary ASC))",
    // the NOT NULL / ISNULL / NOTNULL postfix null tests
    "SELECT 1 WHERE 1 NOT NULL",
    "SELECT 1 WHERE 1 ISNULL",
    "SELECT 1 WHERE 1 NOTNULL",
    // bare general-equality IS / IS NOT
    "SELECT 1 IS 1",
    "SELECT 1 IS NOT 2",
    // the == equality spelling
    "SELECT 1 == 1",
    // inline PRIMARY KEY ordering + AUTOINCREMENT + conflict clause + named collate
    "CREATE TABLE t (a INTEGER PRIMARY KEY DESC)",
    "CREATE TABLE t (a INTEGER PRIMARY KEY AUTOINCREMENT)",
    "CREATE TABLE t (a INTEGER UNIQUE ON CONFLICT REPLACE)",
    "CREATE TABLE t (a TEXT CONSTRAINT c COLLATE nocase)",
    // trailing table options
    "CREATE TABLE t (a INTEGER PRIMARY KEY) WITHOUT ROWID",
    "CREATE TABLE t (a INTEGER) STRICT",
    // bitwise operators, empty IN (), string alias, LIMIT o, c
    "SELECT 1 | 2 & 3 << 1 >> 1, ~5",
    "SELECT 1 WHERE 1 IN ()",
    "SELECT 1 WHERE 1 NOT IN ()",
    "SELECT 1 AS 'x'",
    "SELECT 1 LIMIT 2, 3",
];

// ---------------------------------------------------------------------------
// MySQL distinctive surfaces (oracle-parity-mysql)
// ---------------------------------------------------------------------------

/// Flag-aware probes for MySQL surfaces the ANSI-clean generator does not emit.
/// Fragments are text SQL over the fixed schema; the generative differential
/// drives them against the wire `mysql:8` oracle when available (skips cleanly).
pub static MYSQL_FEATURE_PROBES: &[FeatureProbe] = &[
    FeatureProbe {
        family: "backtick-identifiers",
        applies: |f| f.identifier_quotes.iter().any(|q| q.open() == '`'),
        arb: || Just("SELECT `a` FROM `t`".to_string()).boxed(),
    },
    FeatureProbe {
        family: "null-safe-equals",
        applies: |f| f.operator_syntax.null_safe_equals,
        arb: || Just("SELECT 1 <=> 1".to_string()).boxed(),
    },
    FeatureProbe {
        family: "limit-offset-comma",
        applies: |f| f.query_tail_syntax.limit_offset_comma,
        arb: || Just("SELECT a FROM t LIMIT 2, 3".to_string()).boxed(),
    },
    FeatureProbe {
        family: "straight-join",
        applies: |f| f.join_syntax.straight_join,
        arb: || Just("SELECT * FROM t STRAIGHT_JOIN t AS x ON t.a = x.a".to_string()).boxed(),
    },
    FeatureProbe {
        family: "on-duplicate-key-update",
        applies: |f| f.mutation_syntax.on_duplicate_key_update,
        arb: || {
            Just("INSERT INTO t (a) VALUES (1) ON DUPLICATE KEY UPDATE a = 2".to_string()).boxed()
        },
    },
    FeatureProbe {
        family: "replace-into",
        applies: |f| f.mutation_syntax.replace_into,
        arb: || Just("REPLACE INTO t (a) VALUES (1)".to_string()).boxed(),
    },
    FeatureProbe {
        family: "insert-set",
        applies: |f| f.mutation_syntax.insert_set,
        arb: || Just("INSERT INTO t SET a = 1".to_string()).boxed(),
    },
    FeatureProbe {
        family: "update-delete-tails",
        applies: |f| f.mutation_syntax.update_delete_tails,
        arb: || {
            prop_oneof![
                Just("UPDATE t SET a = 1 ORDER BY a LIMIT 1".to_string()),
                Just("DELETE FROM t ORDER BY a LIMIT 1".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "convert-cast",
        applies: |f| f.call_syntax.convert_function,
        arb: || Just("SELECT CONVERT(a, CHAR) FROM t".to_string()).boxed(),
    },
];

/// Deterministic MySQL generative seeds — one+ per probe family.
pub const MYSQL_FEATURE_SEEDS: &[&str] = &[
    "SELECT `a` FROM `t`",
    "SELECT 1 <=> 1",
    "SELECT a FROM t LIMIT 2, 3",
    "SELECT * FROM t STRAIGHT_JOIN t AS x ON t.a = x.a",
    "INSERT INTO t (a) VALUES (1) ON DUPLICATE KEY UPDATE a = 2",
    "REPLACE INTO t (a) VALUES (1)",
    "INSERT INTO t SET a = 1",
    "UPDATE t SET a = 1 ORDER BY a LIMIT 1",
    "DELETE FROM t ORDER BY a LIMIT 1",
    "SELECT CONVERT(a, CHAR) FROM t",
];

// ---------------------------------------------------------------------------
// PostgreSQL distinctive surfaces (oracle-parity-postgres)
// ---------------------------------------------------------------------------

/// Flag-aware probes for PostgreSQL surfaces the ANSI-clean `arb_statement` does not
/// emit. Each `applies` reads a PG-on gate; the text fragments are engine-verified
/// accept under libpg_query + the fitted `Postgres` preset (ParseOnly — no schema).
pub static POSTGRES_FEATURE_PROBES: &[FeatureProbe] = &[
    FeatureProbe {
        family: "double-colon-cast",
        applies: |f| f.expression_syntax.typecast_operator,
        arb: || {
            prop_oneof![
                Just("SELECT 1::integer".to_string()),
                Just("SELECT 'x'::text".to_string()),
                Just("SELECT a::text FROM t".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "dollar-quoted-strings",
        applies: |f| f.string_literals.dollar_quoted_strings,
        arb: || {
            prop_oneof![
                Just("SELECT $$hello$$".to_string()),
                Just("SELECT $tag$world$tag$".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "ilike",
        applies: |f| f.predicate_syntax.ilike,
        arb: || {
            prop_oneof![
                Just("SELECT 1 WHERE 'AbC' ILIKE 'a%'".to_string()),
                Just("SELECT 1 WHERE 'AbC' NOT ILIKE 'b%'".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "full-outer-join",
        applies: |f| f.join_syntax.full_outer_join,
        arb: || Just("SELECT * FROM t FULL OUTER JOIN t AS x ON t.a = x.a".to_string()).boxed(),
    },
    FeatureProbe {
        family: "distinct-on",
        applies: |f| f.select_syntax.distinct_on,
        arb: || Just("SELECT DISTINCT ON (a) a, b FROM t".to_string()).boxed(),
    },
    FeatureProbe {
        family: "only-table",
        applies: |f| f.table_expressions.only,
        arb: || Just("SELECT * FROM ONLY t".to_string()).boxed(),
    },
    FeatureProbe {
        family: "returning",
        applies: |f| f.mutation_syntax.returning,
        arb: || {
            prop_oneof![
                Just("INSERT INTO t (a) VALUES (1) RETURNING a".to_string()),
                Just("UPDATE t SET a = 1 RETURNING a".to_string()),
                Just("DELETE FROM t RETURNING a".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "on-conflict",
        applies: |f| f.mutation_syntax.on_conflict,
        arb: || {
            prop_oneof![
                Just("INSERT INTO t (a) VALUES (1) ON CONFLICT DO NOTHING".to_string()),
                Just(
                    "INSERT INTO t (a) VALUES (1) ON CONFLICT (a) DO UPDATE SET a = EXCLUDED.a"
                        .to_string()
                ),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "multi-column-assignment",
        applies: |f| f.mutation_syntax.multi_column_assignment,
        arb: || Just("UPDATE t SET (a, b) = (1, 2)".to_string()).boxed(),
    },
    FeatureProbe {
        family: "filter-where",
        applies: |f| f.aggregate_call_syntax.aggregate_filter,
        arb: || Just("SELECT count(*) FILTER (WHERE a > 0) FROM t".to_string()).boxed(),
    },
    FeatureProbe {
        family: "lateral",
        applies: |f| f.table_factor_syntax.lateral,
        arb: || Just("SELECT * FROM t, LATERAL (SELECT t.a AS x) AS s".to_string()).boxed(),
    },
    FeatureProbe {
        family: "array-constructor",
        applies: |f| f.expression_syntax.array_constructor,
        arb: || {
            prop_oneof![
                Just("SELECT ARRAY[1, 2, 3]".to_string()),
                Just("SELECT ARRAY(SELECT a FROM t)".to_string()),
            ]
            .boxed()
        },
    },
];

/// Deterministic PG generative seeds — one+ per [`POSTGRES_FEATURE_PROBES`] family.
pub const POSTGRES_FEATURE_SEEDS: &[&str] = &[
    "SELECT 1::integer",
    "SELECT 'x'::text",
    "SELECT $$hello$$",
    "SELECT $tag$world$tag$",
    "SELECT 1 WHERE 'AbC' ILIKE 'a%'",
    "SELECT * FROM t FULL OUTER JOIN t AS x ON t.a = x.a",
    "SELECT DISTINCT ON (a) a, b FROM t",
    "SELECT * FROM ONLY t",
    "INSERT INTO t (a) VALUES (1) RETURNING a",
    "UPDATE t SET a = 1 RETURNING a",
    "DELETE FROM t RETURNING a",
    "INSERT INTO t (a) VALUES (1) ON CONFLICT DO NOTHING",
    "UPDATE t SET (a, b) = (1, 2)",
    "SELECT count(*) FILTER (WHERE a > 0) FROM t",
    "SELECT * FROM t, LATERAL (SELECT t.a AS x) AS s",
    "SELECT ARRAY[1, 2, 3]",
    "SELECT ARRAY(SELECT a FROM t)",
];

/// DuckDB-distinctive generative probes (oracle-parity-duckdb).
///
/// Prefer gates that are on under DuckDB and off under ANSI so the self-selection
/// property holds. Seeds are engine-checked against libduckdb (1.5.4).
pub static DUCKDB_FEATURE_PROBES: &[FeatureProbe] = &[
    FeatureProbe {
        family: "double-colon-cast",
        applies: |f| f.expression_syntax.typecast_operator,
        arb: || {
            prop_oneof![
                Just("SELECT 1::INTEGER".to_string()),
                Just("SELECT 'x'::VARCHAR".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "list-literal",
        applies: |f| f.expression_syntax.collection_literals,
        arb: || {
            prop_oneof![
                Just("SELECT [1, 2, 3]".to_string()),
                Just("SELECT ['a', 'b']".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "struct-literal",
        applies: |f| f.expression_syntax.collection_literals,
        arb: || Just("SELECT {'x': 1, 'y': 2}".to_string()).boxed(),
    },
    FeatureProbe {
        family: "double-ampersand-overlaps",
        applies: |f| {
            use squonk::ast::dialect::DoubleAmpersand;
            matches!(f.double_ampersand, DoubleAmpersand::Overlaps)
        },
        arb: || Just("SELECT [1, 2] && [2, 3]".to_string()).boxed(),
    },
    FeatureProbe {
        family: "filter-where",
        applies: |f| f.aggregate_call_syntax.aggregate_filter,
        arb: || Just("SELECT count(*) FILTER (WHERE a > 0) FROM t".to_string()).boxed(),
    },
    FeatureProbe {
        family: "group-by-all",
        applies: |f| f.grouping_syntax.group_by_all,
        arb: || Just("SELECT a, count(*) FROM t GROUP BY ALL".to_string()).boxed(),
    },
    FeatureProbe {
        family: "returning",
        applies: |f| f.mutation_syntax.returning,
        arb: || {
            prop_oneof![
                Just("INSERT INTO t (a) VALUES (1) RETURNING a".to_string()),
                Just("DELETE FROM t RETURNING a".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "merge-update-set-star",
        applies: |f| f.mutation_syntax.merge_update_set_star,
        arb: || {
            Just(
                "MERGE INTO t USING t AS s ON t.a = s.a WHEN MATCHED THEN UPDATE SET *".to_string(),
            )
            .boxed()
        },
    },
    FeatureProbe {
        family: "merge-insert-star",
        applies: |f| f.mutation_syntax.merge_insert_star_by_name,
        arb: || {
            Just(
                "MERGE INTO t USING t AS s ON t.a = s.a WHEN NOT MATCHED THEN INSERT *".to_string(),
            )
            .boxed()
        },
    },
    FeatureProbe {
        family: "array-constructor",
        applies: |f| f.expression_syntax.array_constructor,
        arb: || Just("SELECT ARRAY[1, 2, 3]".to_string()).boxed(),
    },
    FeatureProbe {
        family: "on-conflict",
        applies: |f| f.mutation_syntax.on_conflict,
        arb: || Just("INSERT INTO t (a) VALUES (1) ON CONFLICT DO NOTHING".to_string()).boxed(),
    },
];

/// Deterministic DuckDB generative seeds — one+ per [`DUCKDB_FEATURE_PROBES`] family.
pub const DUCKDB_FEATURE_SEEDS: &[&str] = &[
    "SELECT 1::INTEGER",
    "SELECT 'x'::VARCHAR",
    "SELECT [1, 2, 3]",
    "SELECT {'x': 1, 'y': 2}",
    "SELECT [1, 2] && [2, 3]",
    "SELECT count(*) FILTER (WHERE a > 0) FROM t",
    "SELECT a, count(*) FROM t GROUP BY ALL",
    "INSERT INTO t (a) VALUES (1) RETURNING a",
    "DELETE FROM t RETURNING a",
    "MERGE INTO t USING t AS s ON t.a = s.a WHEN MATCHED THEN UPDATE SET *",
    "MERGE INTO t USING t AS s ON t.a = s.a WHEN NOT MATCHED THEN INSERT *",
    "SELECT ARRAY[1, 2, 3]",
    "INSERT INTO t (a) VALUES (1) ON CONFLICT DO NOTHING",
];

/// BigQuery-distinctive generative probes (oracle-parity-bigquery).
///
/// Each probe is keyed to a flag the BigQuery preset actually enables. Seeds use
/// **modelled-surface** spellings only — e.g. `UNNEST(arr)` not `UNNEST([1,2])`,
/// because `array_constructor` / `collection_literals` stay off on this preset
/// (conservative; no ZetaSQL oracle yet). Engine ground truth is deferred to
/// ZetaSQL; sqlglot is comparison-only (see `conformance/src/bigquery.rs`).
pub static BIGQUERY_FEATURE_PROBES: &[FeatureProbe] = &[
    FeatureProbe {
        family: "unnest",
        applies: |f| f.table_factor_syntax.unnest,
        arb: || {
            prop_oneof![
                Just("SELECT * FROM UNNEST(arr) AS x".to_string()),
                Just("SELECT * FROM UNNEST(a, b)".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "unnest-with-offset",
        applies: |f| f.table_factor_syntax.unnest_with_offset,
        arb: || Just("SELECT * FROM UNNEST(arr) WITH OFFSET".to_string()).boxed(),
    },
    FeatureProbe {
        family: "struct-constructor",
        applies: |f| f.expression_syntax.struct_constructor,
        arb: || {
            prop_oneof![
                Just("SELECT STRUCT(1, 2)".to_string()),
                Just("SELECT STRUCT(1 AS a, 2 AS b)".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "angle-bracket-array-type",
        applies: |f| f.type_name_syntax.angle_bracket_types,
        arb: || {
            prop_oneof![
                Just("SELECT CAST(x AS ARRAY<INT64>)".to_string()),
                Just("SELECT CAST(x AS ARRAY<STRING>)".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "angle-bracket-struct-type",
        applies: |f| f.type_name_syntax.angle_bracket_types,
        arb: || {
            prop_oneof![
                Just("SELECT CAST(x AS STRUCT<a INT64>)".to_string()),
                Just("SELECT CAST(x AS STRUCT<a INT64, b STRING>)".to_string()),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "table-version-system-time",
        applies: |f| f.table_expressions.table_version,
        arb: || {
            Just(
                "SELECT * FROM t FOR SYSTEM_TIME AS OF TIMESTAMP '2020-01-01 00:00:00'".to_string(),
            )
            .boxed()
        },
    },
    FeatureProbe {
        family: "double-quoted-strings",
        applies: |f| f.string_literals.double_quoted_strings,
        arb: || Just("SELECT \"hello\"".to_string()).boxed(),
    },
    FeatureProbe {
        family: "backtick-identifiers",
        // BigQuery's quote set is backtick-only (ANSI is double-quote).
        applies: |f| f.identifier_quotes.len() == 1 && f.identifier_quotes[0].open() == '`',
        arb: || Just("SELECT `a` FROM t".to_string()).boxed(),
    },
];

/// Deterministic BigQuery generative seeds — one+ per [`BIGQUERY_FEATURE_PROBES`] family.
pub const BIGQUERY_FEATURE_SEEDS: &[&str] = &[
    "SELECT * FROM UNNEST(arr) AS x",
    "SELECT * FROM UNNEST(arr) WITH OFFSET",
    "SELECT STRUCT(1, 2)",
    "SELECT STRUCT(1 AS a, 2 AS b)",
    "SELECT CAST(x AS ARRAY<INT64>)",
    "SELECT CAST(x AS STRUCT<a INT64>)",
    "SELECT * FROM t FOR SYSTEM_TIME AS OF TIMESTAMP '2020-01-01 00:00:00'",
    "SELECT \"hello\"",
    "SELECT `a` FROM t",
];

// ---------------------------------------------------------------------------
// ClickHouse distinctive surfaces (clickhouse-tier-promotion-generative-nightly)
// ---------------------------------------------------------------------------

/// A ClickHouse fixed-bit-width integer name — the twelve `Int*`/`UInt*` spellings the
/// `bit_width_integer_names` gate admits (see `parser::ty::try_parse_bit_width_integer_name`).
fn arb_ch_bit_width_int() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("Int8"),
        Just("Int16"),
        Just("Int32"),
        Just("Int64"),
        Just("Int128"),
        Just("Int256"),
        Just("UInt8"),
        Just("UInt16"),
        Just("UInt32"),
        Just("UInt64"),
        Just("UInt128"),
        Just("UInt256"),
    ]
}

/// An inner type for the `Nullable(T)` / `LowCardinality(T)` combinators — bodies both the
/// fitted `ClickHouse` preset and `clickhouse local` accept when wrapped.
fn arb_ch_inner_type() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("Int32".to_string()),
        Just("UInt64".to_string()),
        Just("String".to_string()),
        (1u32..=255).prop_map(|n| format!("FixedString({n})")),
    ]
}

/// A registry-real ClickHouse setting name. `EXPLAIN AST` validates the SETTINGS clause's
/// setting *names* (an unknown name is `Code: 115` `UNKNOWN_SETTING`) and *value types* (a
/// non-numeric value for an integer setting is `Code: 27`), unlike the rest of its
/// otherwise pure-ParseOnly surface — see `conformance::clickhouse`. So the SETTINGS probe
/// keeps to known integer-valued settings, the ClickHouse analogue of provisioning a
/// PrepareBind oracle's schema.
fn arb_ch_setting() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("max_threads"),
        Just("max_block_size"),
        Just("max_rows_to_read"),
        Just("max_memory_usage"),
        Just("max_execution_time"),
    ]
}

/// A ClickHouse output-format name (a bare identifier both sides accept; `EXPLAIN AST` does
/// **not** validate the format name — an unknown one still parses).
fn arb_ch_format() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("JSON"),
        Just("CSV"),
        Just("TabSeparated"),
        Just("Pretty"),
        Just("TSKV"),
        Just("Parquet"),
    ]
}

/// Flag-aware probes for the ClickHouse surface the ANSI-clean `arb_statement` never emits.
/// Each `applies` reads the exact gate the fitted [`ClickHouse`](squonk::dialect::ClickHouse)
/// preset turns on over ANSI — the three query tails, the six type constructors, and the
/// backtick identifier quote. Fragments are engine-verified both-accept against
/// `clickhouse local`'s `EXPLAIN AST` (ParseOnly) plus the preset; the differential driver
/// lives in `conformance::clickhouse`.
pub static CLICKHOUSE_FEATURE_PROBES: &[FeatureProbe] = &[
    FeatureProbe {
        family: "limit-by",
        applies: |f| f.query_tail_syntax.limit_by_clause,
        arb: || {
            prop_oneof![
                (0u32..100, arb_column())
                    .prop_map(|(n, c)| format!("SELECT a FROM t LIMIT {n} BY {c}")),
                (0u32..100, arb_column(), arb_column())
                    .prop_map(|(n, c1, c2)| format!("SELECT a FROM t LIMIT {n} BY {c1}, {c2}")),
                (0u32..100, 0u32..100, arb_column())
                    .prop_map(|(n, o, c)| format!("SELECT a FROM t LIMIT {n} OFFSET {o} BY {c}")),
            ]
            .boxed()
        },
    },
    FeatureProbe {
        family: "settings-clause",
        applies: |f| f.query_tail_syntax.settings_clause,
        arb: || {
            (arb_ch_setting(), 0u64..10_000)
                .prop_map(|(name, value)| format!("SELECT a FROM t SETTINGS {name} = {value}"))
                .boxed()
        },
    },
    FeatureProbe {
        family: "format-clause",
        applies: |f| f.query_tail_syntax.format_clause,
        arb: || {
            arb_ch_format()
                .prop_map(|fmt| format!("SELECT a FROM t FORMAT {fmt}"))
                .boxed()
        },
    },
    FeatureProbe {
        family: "nullable-type",
        applies: |f| f.type_name_syntax.nullable_type,
        arb: || {
            arb_ch_inner_type()
                .prop_map(|inner| format!("CREATE TABLE t (c Nullable({inner}))"))
                .boxed()
        },
    },
    FeatureProbe {
        family: "low-cardinality-type",
        applies: |f| f.type_name_syntax.low_cardinality_type,
        arb: || {
            arb_ch_inner_type()
                .prop_map(|inner| format!("CREATE TABLE t (c LowCardinality({inner}))"))
                .boxed()
        },
    },
    FeatureProbe {
        family: "fixed-string-type",
        applies: |f| f.type_name_syntax.fixed_string_type,
        arb: || {
            (1u32..=255)
                .prop_map(|n| format!("CREATE TABLE t (c FixedString({n}))"))
                .boxed()
        },
    },
    FeatureProbe {
        family: "datetime64-type",
        applies: |f| f.type_name_syntax.datetime64_type,
        arb: || {
            (0u32..=9)
                .prop_map(|p| format!("CREATE TABLE t (c DateTime64({p}))"))
                .boxed()
        },
    },
    FeatureProbe {
        family: "nested-type",
        applies: |f| f.type_name_syntax.nested_type,
        arb: || {
            prop::collection::vec(arb_ch_bit_width_int(), 1..4)
                .prop_map(|types| {
                    let body = types
                        .iter()
                        .enumerate()
                        .map(|(i, ty)| format!("f{i} {ty}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("CREATE TABLE t (c Nested({body}))")
                })
                .boxed()
        },
    },
    FeatureProbe {
        family: "bit-width-integer-names",
        applies: |f| f.type_name_syntax.bit_width_integer_names,
        arb: || {
            arb_ch_bit_width_int()
                .prop_map(|ty| format!("CREATE TABLE t (c {ty})"))
                .boxed()
        },
    },
    FeatureProbe {
        family: "backtick-identifiers",
        applies: |f| f.identifier_quotes.iter().any(|q| q.open() == '`'),
        arb: || Just("SELECT `a` FROM `t`".to_string()).boxed(),
    },
];

/// Deterministic ClickHouse generative seeds — one+ per [`CLICKHOUSE_FEATURE_PROBES`]
/// family, each engine-verified both-accept against `clickhouse local` 25.5 and the fitted
/// `ClickHouse` preset. The replay gate
/// (`clickhouse::clickhouse_feature_generative_differential_replays_committed_seeds`) trips
/// on any un-allowlisted divergence, so a preset tightening or a fresh over-acceptance is
/// deterministic, never a flaky always-random gate.
pub const CLICKHOUSE_FEATURE_SEEDS: &[&str] = &[
    // limit-by
    "SELECT a FROM t LIMIT 5 BY a",
    "SELECT a FROM t LIMIT 5 BY a, b",
    "SELECT a FROM t LIMIT 5 OFFSET 2 BY a",
    // settings-clause (registry-real names — EXPLAIN AST validates them)
    "SELECT a FROM t SETTINGS max_threads = 8",
    "SELECT a FROM t SETTINGS max_threads = 8, max_block_size = 100",
    // format-clause
    "SELECT a FROM t FORMAT JSON",
    "SELECT a FROM t FORMAT TabSeparated",
    // nullable-type
    "CREATE TABLE t (c Nullable(Int32))",
    "CREATE TABLE t (c Nullable(FixedString(8)))",
    // low-cardinality-type
    "CREATE TABLE t (c LowCardinality(String))",
    "CREATE TABLE t (c LowCardinality(FixedString(16)))",
    // fixed-string-type
    "CREATE TABLE t (c FixedString(16))",
    // datetime64-type
    "CREATE TABLE t (c DateTime64(3))",
    "CREATE TABLE t (c DateTime64(9))",
    // nested-type
    "CREATE TABLE t (c Nested(a Int32, b String))",
    // bit-width-integer-names
    "CREATE TABLE t (c Int256)",
    "CREATE TABLE t (c UInt64)",
    // backtick-identifiers
    "SELECT `a` FROM `t`",
];

/// Committed ANSI generative round-trip seeds (oracle-parity-ansi): each must
/// `parse(render(parse(sql)))` under [`Ansi`](squonk::dialect::Ansi). Covers the
/// families newly pressured into [`arb_statement`](super::arb_statement) — CASE, CAST,
/// IS NULL, LIKE, BETWEEN, IN list, EXISTS, scalar subquery, derived table, qualified
/// wildcard — plus baseline SELECT/DML anchors.
pub const ANSI_ROUNDTRIP_SEEDS: &[&str] = &[
    "SELECT 1",
    "SELECT a FROM t",
    "SELECT t.* FROM t",
    "SELECT CAST(a AS INTEGER) FROM t",
    "SELECT CASE WHEN a = 1 THEN 2 ELSE 3 END FROM t",
    "SELECT 1 WHERE a IS NULL",
    "SELECT 1 WHERE a IS NOT NULL",
    "SELECT 1 WHERE a LIKE '%x%'",
    "SELECT 1 WHERE a BETWEEN 1 AND 10",
    "SELECT 1 WHERE a IN (1, 2, 3)",
    "SELECT 1 WHERE EXISTS (SELECT 1 FROM t)",
    "SELECT (SELECT 1 FROM t) FROM t",
    "SELECT a FROM (SELECT a FROM t) AS x",
    "SELECT a FROM t INNER JOIN t AS x ON a = x.a",
    "INSERT INTO t VALUES (1, 2, 3)",
    "UPDATE t SET a = 1 WHERE b = 2",
    "DELETE FROM t WHERE a = 1",
    "CREATE TABLE t (a INTEGER PRIMARY KEY, b TEXT)",
];

#[cfg(test)]
mod tests {
    use super::*;
    use squonk::Dialect;
    use squonk::dialect::{Ansi, Sqlite};

    #[test]
    fn every_sqlite_probe_applies_to_the_sqlite_preset() {
        // The probe table is authored for SQLite, so every entry must be selected by the
        // fitted `Sqlite` preset — a flag renamed/removed on the preset side trips this.
        let features = Sqlite.features();
        for probe in SQLITE_FEATURE_PROBES {
            assert!(
                (probe.applies)(features),
                "SQLite probe {:?} does not apply to the Sqlite preset (gate flag drifted)",
                probe.family,
            );
        }
        assert_eq!(
            applicable_probes(features, SQLITE_FEATURE_PROBES).len(),
            SQLITE_FEATURE_PROBES.len(),
        );
    }

    #[test]
    fn ansi_selects_far_fewer_probes_than_sqlite() {
        // The reusable self-selection property: the same probe table yields a *different*
        // (much smaller) applicable set under ANSI, since ANSI leaves the misfeature gates
        // off. This is the mechanism a sibling dialect relies on.
        let ansi = applicable_probes(Ansi.features(), SQLITE_FEATURE_PROBES).len();
        let sqlite = applicable_probes(Sqlite.features(), SQLITE_FEATURE_PROBES).len();
        assert!(
            ansi < sqlite,
            "ANSI ({ansi}) must enable strictly fewer misfeature probes than SQLite ({sqlite})",
        );
    }

    #[test]
    fn seeds_parse_under_the_sqlite_preset() {
        // Every committed seed is accepted by the fitted `Sqlite` preset (the "we accept"
        // half of the both-accept contract; the engine half is checked in the oracle-gated
        // replay gate). A seed our parser rejects would make the differential a coverage
        // gap, not the over-acceptance proof it is meant to be.
        for sql in SQLITE_MISFEATURE_SEEDS {
            assert!(
                squonk::parse_with(sql, squonk::ParseConfig::new(Sqlite)).is_ok(),
                "committed misfeature seed does not parse under Sqlite: {sql:?}",
            );
        }
    }

    #[test]
    fn every_mysql_probe_applies_to_the_mysql_preset() {
        use squonk::dialect::MySql;
        let features = MySql.features();
        for probe in MYSQL_FEATURE_PROBES {
            assert!(
                (probe.applies)(features),
                "MySql probe {:?} does not apply to the MySql preset",
                probe.family,
            );
        }
    }

    #[test]
    fn ansi_selects_fewer_mysql_probes_than_mysql() {
        use squonk::dialect::{Ansi, MySql};
        let ansi = applicable_probes(Ansi.features(), MYSQL_FEATURE_PROBES).len();
        let mysql = applicable_probes(MySql.features(), MYSQL_FEATURE_PROBES).len();
        assert!(ansi < mysql, "ANSI ({ansi}) < MySQL ({mysql})");
    }

    #[test]
    fn mysql_feature_seeds_parse_under_mysql() {
        use squonk::dialect::MySql;
        for sql in MYSQL_FEATURE_SEEDS {
            assert!(
                squonk::parse_with(sql, squonk::ParseConfig::new(MySql)).is_ok(),
                "committed MySQL seed does not parse under MySql: {sql:?}",
            );
        }
    }

    #[test]
    fn every_postgres_probe_applies_to_the_postgres_preset() {
        use squonk::dialect::Postgres;
        let features = Postgres.features();
        for probe in POSTGRES_FEATURE_PROBES {
            assert!(
                (probe.applies)(features),
                "Postgres probe {:?} does not apply to the Postgres preset",
                probe.family,
            );
        }
        assert_eq!(
            applicable_probes(features, POSTGRES_FEATURE_PROBES).len(),
            POSTGRES_FEATURE_PROBES.len(),
        );
    }

    #[test]
    fn ansi_selects_fewer_postgres_probes_than_postgres() {
        use squonk::dialect::{Ansi, Postgres};
        let ansi = applicable_probes(Ansi.features(), POSTGRES_FEATURE_PROBES).len();
        let pg = applicable_probes(Postgres.features(), POSTGRES_FEATURE_PROBES).len();
        assert!(
            ansi < pg,
            "ANSI ({ansi}) must enable strictly fewer PG probes than Postgres ({pg})",
        );
    }

    #[test]
    fn postgres_feature_seeds_parse_under_postgres() {
        use squonk::dialect::Postgres;
        for sql in POSTGRES_FEATURE_SEEDS {
            assert!(
                squonk::parse_with(sql, squonk::ParseConfig::new(Postgres)).is_ok(),
                "committed PG seed does not parse under Postgres: {sql:?}",
            );
        }
    }

    #[test]
    fn ansi_roundtrip_seeds_parse_and_round_trip() {
        // The public ANSI oracle (`assert_roundtrips`) — parse → Canonical render →
        // reparse → structural equal. Seeds cover the families newly pressured into
        // `arb_statement` (CASE/CAST/IS NULL/LIKE/BETWEEN/IN/EXISTS/subquery/derived/`t.*`).
        for sql in ANSI_ROUNDTRIP_SEEDS {
            crate::assert_roundtrips(sql);
            crate::assert_roundtrips_parenthesized(sql);
        }
    }

    #[test]
    fn ansi_arb_statement_explores_without_unledgered_roundtrip_failures() {
        // Fixed-RNG exploration over the ANSI-clean generator — the no-engine analogue
        // of the SQLite flag-aware generative differential.
        use crate::properties::{GENERATED_RESOLVER, arb_statement, render_generated};
        use crate::shared_interner;
        use proptest::prelude::*;
        use proptest::strategy::ValueTree;
        use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
        use squonk::dialect::Ansi;
        use squonk::parse_with;
        use squonk_ast::render::RenderMode;

        let mut runner = TestRunner::new_with_rng(
            Config {
                cases: 256,
                ..Config::default()
            },
            TestRng::from_seed(RngAlgorithm::ChaCha, &[0xA1; 32]),
        );
        let strategy = arb_statement();
        for _ in 0..256 {
            let tree = strategy
                .new_tree(&mut runner)
                .expect("arb_statement is infallible to instantiate");
            let statement = tree.current();
            let rendered = render_generated(&statement, RenderMode::Canonical);
            let reparsed =
                parse_with(&rendered, squonk::ParseConfig::new(Ansi)).unwrap_or_else(|e| {
                    panic!("arb_statement render must reparse under Ansi: {rendered:?}: {e:?}")
                });
            let comparison = shared_interner::compare_statement_with_shared_symbols(
                &statement,
                &GENERATED_RESOLVER,
                &reparsed.statements()[0],
                reparsed.resolver(),
            );
            assert!(
                comparison.structurally_equal(),
                "ANSI generative structural round-trip mismatch: {rendered:?}",
            );
        }
    }

    #[test]
    fn every_duckdb_probe_applies_to_the_duckdb_preset() {
        use squonk::dialect::DuckDb;
        let features = DuckDb.features();
        for probe in DUCKDB_FEATURE_PROBES {
            assert!(
                (probe.applies)(features),
                "DuckDb probe {:?} does not apply to the DuckDb preset",
                probe.family,
            );
        }
        assert_eq!(
            applicable_probes(features, DUCKDB_FEATURE_PROBES).len(),
            DUCKDB_FEATURE_PROBES.len(),
        );
    }

    #[test]
    fn ansi_selects_fewer_duckdb_probes_than_duckdb() {
        use squonk::dialect::{Ansi, DuckDb};
        let ansi = applicable_probes(Ansi.features(), DUCKDB_FEATURE_PROBES).len();
        let duck = applicable_probes(DuckDb.features(), DUCKDB_FEATURE_PROBES).len();
        assert!(
            ansi < duck,
            "ANSI ({ansi}) must enable strictly fewer DuckDB probes than DuckDB ({duck})",
        );
    }

    #[test]
    fn duckdb_feature_seeds_parse_under_duckdb() {
        use squonk::dialect::DuckDb;
        for sql in DUCKDB_FEATURE_SEEDS {
            assert!(
                squonk::parse_with(sql, squonk::ParseConfig::new(DuckDb)).is_ok(),
                "committed DuckDB feature seed does not parse under DuckDb: {sql:?}",
            );
        }
    }

    #[test]
    fn every_bigquery_probe_applies_to_the_bigquery_preset() {
        use squonk::dialect::BigQuery;
        let features = BigQuery.features();
        for probe in BIGQUERY_FEATURE_PROBES {
            assert!(
                (probe.applies)(features),
                "BigQuery probe {:?} does not apply to the BigQuery preset",
                probe.family,
            );
        }
        assert_eq!(
            applicable_probes(features, BIGQUERY_FEATURE_PROBES).len(),
            BIGQUERY_FEATURE_PROBES.len(),
        );
    }

    #[test]
    fn ansi_selects_fewer_bigquery_probes_than_bigquery() {
        use squonk::dialect::{Ansi, BigQuery};
        let ansi = applicable_probes(Ansi.features(), BIGQUERY_FEATURE_PROBES).len();
        let bq = applicable_probes(BigQuery.features(), BIGQUERY_FEATURE_PROBES).len();
        assert!(
            ansi < bq,
            "ANSI ({ansi}) must enable strictly fewer BigQuery probes than BigQuery ({bq})",
        );
    }

    #[test]
    fn bigquery_feature_seeds_parse_under_bigquery() {
        use squonk::dialect::BigQuery;
        for sql in BIGQUERY_FEATURE_SEEDS {
            assert!(
                squonk::parse_with(sql, squonk::ParseConfig::new(BigQuery)).is_ok(),
                "committed BigQuery feature seed does not parse under BigQuery: {sql:?}",
            );
        }
    }

    #[test]
    fn every_clickhouse_probe_applies_to_the_clickhouse_preset() {
        use squonk::dialect::ClickHouse;
        let features = ClickHouse.features();
        for probe in CLICKHOUSE_FEATURE_PROBES {
            assert!(
                (probe.applies)(features),
                "ClickHouse probe {:?} does not apply to the ClickHouse preset (gate flag drifted)",
                probe.family,
            );
        }
        assert_eq!(
            applicable_probes(features, CLICKHOUSE_FEATURE_PROBES).len(),
            CLICKHOUSE_FEATURE_PROBES.len(),
        );
    }

    #[test]
    fn ansi_selects_fewer_clickhouse_probes_than_clickhouse() {
        use squonk::dialect::{Ansi, ClickHouse};
        let ansi = applicable_probes(Ansi.features(), CLICKHOUSE_FEATURE_PROBES).len();
        let ch = applicable_probes(ClickHouse.features(), CLICKHOUSE_FEATURE_PROBES).len();
        assert!(
            ansi < ch,
            "ANSI ({ansi}) must enable strictly fewer ClickHouse probes than ClickHouse ({ch})",
        );
    }

    #[test]
    fn clickhouse_feature_seeds_parse_under_clickhouse() {
        use squonk::dialect::ClickHouse;
        for sql in CLICKHOUSE_FEATURE_SEEDS {
            assert!(
                squonk::parse_with(sql, squonk::ParseConfig::new(ClickHouse)).is_ok(),
                "committed ClickHouse feature seed does not parse under ClickHouse: {sql:?}",
            );
        }
    }
}


#[cfg(test)]
mod organization_completeness {
    use squonk::ast::dialect::FEATURES;

    /// DP6 (axis level): every top-level FeatureSet field is registered as a Feature enum
    /// variant so maturity/coverage gates can see it. Sub-flag breadth is the ToggleableFeature
    /// + LabeledCase system (`every_gated_subflag_is_required_by_a_labeled_case`) plus the
    /// `knob-org` orphan scan for every bool field under crates/squonk/src.
    #[test]
    fn every_feature_set_axis_has_a_feature_enum_variant() {
        assert_eq!(
            FEATURES.len(),
            50,
            "Feature registry must stay aligned with FeatureSet fields (run squonk-sourcegen after axis splits)"
        );
        let ids: Vec<_> = FEATURES.iter().map(|f| f.id()).collect();
        assert!(ids.iter().any(|id| *id == "transaction_syntax"));
        assert!(ids.iter().any(|id| *id == "view_sequence_clause_syntax"));
        assert!(ids.iter().any(|id| *id == "utility_syntax"));
    }
}
