// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! SQLite accept/reject **parity gate** over the vendored corpora + the authored
//! SQLite feature probes (`sqlite-oracle-at-scale`).
//!
//! This module routes every swept statement through the in-process
//! [`SqliteOracle`](crate::m2::SqliteOracle) (bundled `rusqlite`, `prepare`-only) under the
//! fitted [`Sqlite`] preset, pins per-corpus bucket counts, and gates the "we accept, SQLite
//! rejects" direction through an allowlist — the PostgreSQL ledger pattern
//! ([`pg::PG_DIVERGENCE_ALLOWLIST`](crate::pg::PG_DIVERGENCE_ALLOWLIST) /
//! `corpus_pg_verdicts`). The **setup driver at scale** unblinds the schema-shadowed
//! statements (mirrors `corpus_duckdb_verdicts`).
//!
//! # The hole the bare sweep left, and how the setup driver + reject-reason split close it
//!
//! [`SqliteOracle`] is [`PrepareBind`](crate::oracle::OracleSemantics::PrepareBind):
//! `prepare` resolves names, so `SELECT a FROM t1` against a bare database rejects "no
//! such table" — a **binding** reject our parse-only parser never sees. Over the
//! multi-dialect corpora that produce a large "we accept, SQLite bare-rejects" bucket
//! that is pure name-resolution noise, not real
//! over-acceptance. Two mechanisms separate the signal from the noise:
//!
//! - **The reject reason splits binding from syntax.** A *syntax* reject is
//!   schema-independent (SQLite reports it before name resolution — `near "X": syntax
//!   error`), so a statement we accept and SQLite syntax-rejects is a real
//!   validator-correctness divergence no schema can mask or cause. A *binding* reject
//!   (`no such table/column`, `already exists`) is name resolution — a counted residual,
//!   never ledgered. This split alone makes the over-acceptance gate correct over every
//!   corpus, provisioned or not.
//! - **The per-corpus setup driver unblinds the binding residual.** The self-contained
//!   `sqllogictest` corpus carries its own `CREATE TABLE`/`CREATE INDEX` DDL inline (it
//!   is SQLite-derived), so a *positional replay* provisions the schema each statement
//!   sees — the DDL is `execute`d onto an evolving connection (the setup driver; the
//!   *only* thing executed — corpus statements are only `prepare`d, the m2 never-execute
//!   guarantee), and a `SELECT a FROM t1` binding-reject over the bare DB flips to a
//!   schema-accept. Because the corpus redefines tables across concatenated test scripts
//!   (three `t1` shapes, no `DROP`s), the replay drops-and-recreates on redefinition so
//!   each query is compared against the schema epoch it belongs to.
//!
//! SQLite's accept verdict is taken as `bare-accepts OR schema-accepts`, exactly as the
//! DuckDB gate: bare-accept covers the schema-independent statements and neutralizes the
//! `CREATE TABLE t1`-under-test self-collision (a redefinition rejects "already exists"
//! against its own provisioned epoch but a fresh bare DB accepts it — the honest
//! verdict); schema-accept covers the statements the replay unblinds.
//!
//! # What the gate asserts, and where it stays green-by-counts
//!
//! The two directions have **different bars on purpose**:
//!
//! - **Over-acceptance is the ledgered correctness gate.** *We accept ∧ SQLite
//!   syntax-rejects* is a real validator-correctness divergence (schema-independent, so
//!   no schema masks or causes it). Every per-corpus over-acceptance pin is **0**: the fitted
//!   `Sqlite` preset tightens each family the broad shared multi-dialect grammar would
//!   otherwise over-accept — the FeatureSet-expressible statement / expression / DDL-clause
//!   families (typed & INTERVAL literals, `WITHIN GROUP`, quantified `ANY`/`ALL`, table-alias
//!   column lists, SET, GRANT/REVOKE, CREATE/DROP SCHEMA/DATABASE/FUNCTION/MATERIALIZED VIEW,
//!   OR REPLACE, DELETE USING, `GENERATED … IDENTITY`, WITH storage params, ON COMMIT, the
//!   extended ALTER surface, `EXTRACT`, bare `OFFSET`) via
//!   `sqlite-preset-over-acceptance-tightening`, and the position-aware query-structure +
//!   name-grammar families that share high-traffic parser paths (the FROM derived-table climb,
//!   the shared name/label parser, the nested-join grammar) via
//!   `sqlite-preset-over-acceptance-query-and-name-grammar`: parenthesized set-op / CTAS /
//!   INSERT / CTE-body operands (`parenthesized_query_operands` with a threaded FROM/scalar
//!   grouping context that keeps SQLite's `FROM ((SELECT 1))` while rejecting the bare operand),
//!   multi-part table/index names (`catalog_qualified_names`), reserved-word ColLabels
//!   (`reserved_as_label`), and stacked join qualifiers (`stacked_join_qualifiers`). They
//!   are ledgered two ways: singular cases go in the
//!   exact-SQL [`SQLITE_DIVERGENCE_ALLOWLIST`] (the PG-ledger clone, staleness enforced),
//!   and the multi-dialect *bulk* is accounted at corpus granularity via each corpus's
//!   pinned over-acceptance count, owned by [`SQLITE_OVER_ACCEPTANCE_TICKET`] — because
//!   exact-SQL-listing the bulk is impractical (the sqlglot-complex over-acceptances are
//!   100-line TPC-DS queries), the DuckDB family-count discipline for a large divergence
//!   set. Either way a NEW over-acceptance drifts a pin and fails; a family tightened in
//!   the parser drifts it the other way — nothing stays silently allowlisted.
//! - **Coverage gaps stay a green-by-counts inventory.** *SQLite accepts ∧ we reject* is
//!   the expected residual for the still-growing dialect — the bitwise-operator and
//!   `?NNN`-parameter families the child tickets close (the `PRAGMA`/`ATTACH`/`DETACH`
//!   family closed with `sqlite-pragma-attach-statements`, and the `INSERT OR`/`UPDATE OR`
//!   conflict-action seam with `sqlite-insert-or-action`).
//!   Per-corpus counts are pinned (anti-vanishing + honest re-baseline: a closed
//!   gap or a fresh regression drifts a pin and fails loudly), and the full inventory is
//!   printed on every run so the child tickets can re-derive it from the log.
//! - **The binding residual is counted, not silenced (the STOP fallback).** The `sqlglot`
//!   corpora reference thousands of arbitrary multi-dialect identifiers with no vendored
//!   schema; synthesizing one is intractable, so per the ticket's STOP rule those
//!   binding rejects are counted and pinned (never a synthesized "clean" schema that
//!   would fake unblinding). Only `sqllogictest` — self-contained — is replayed.

use crate::m2::SqliteOracle;
use crate::oracle::AcceptRejectOracle;
use crate::verdict_harness::{
    Cell, DivergenceEntry, GapClass, Probe, Quadrant, RejectReason, Verdict,
    assert_entries_are_ticketed, assert_entries_still_diverge, check_probe_group,
    sqlglot_complex_statements, sqlglot_identity_lines, sqllogictest_lines, ticket_exists,
};
use rusqlite::Connection as SqliteConnection;
use squonk::dialect::Sqlite;
use squonk::parse_with;
use std::collections::BTreeMap;

const SQLITE_FEATURES: &str = include_str!("../corpus/sqlite/features.sql");

/// Schema the setup driver provisions before the mutation probes are compared, so
/// their coverage-gap signal is name-resolution-clean (the m2 setup-driver pattern).
const SETUP_SQL: &str =
    "CREATE TABLE t(a INTEGER PRIMARY KEY, b TEXT, c INTEGER); CREATE TABLE u(x INTEGER, y TEXT)";

/// The SQLite mutation / index / trigger families that need a schema to exercise.
/// Every entry SQLite accepts once [`SETUP_SQL`] is provisioned (verified at authoring
/// time); the sweep records which our fitted `Sqlite` surface rejects.
const SETUP_DRIVEN_PROBES: &[Probe] = &[
    // INSERT OR <action> / UPDATE OR <action>: a conflict-action on the verb, distinct
    // from the ON CONFLICT upsert — the spike's `Insert.or_action` / `Update.or_action`
    // field, now parsed by the fitted preset (`mutation_syntax.or_conflict_action`), so
    // these are controls closed by `sqlite-insert-or-action`.
    Probe {
        sql: "INSERT OR REPLACE INTO t(a, b) VALUES (1, 'x')",
        class: GapClass::Control,
    },
    Probe {
        sql: "INSERT OR IGNORE INTO t(a, b) VALUES (1, 'x')",
        class: GapClass::Control,
    },
    Probe {
        sql: "INSERT OR ABORT INTO t(a, b) VALUES (1, 'x')",
        class: GapClass::Control,
    },
    Probe {
        sql: "INSERT OR FAIL INTO t(a, b) VALUES (1, 'x')",
        class: GapClass::Control,
    },
    Probe {
        sql: "INSERT OR ROLLBACK INTO t(a, b) VALUES (1, 'x')",
        class: GapClass::Control,
    },
    Probe {
        sql: "UPDATE OR REPLACE t SET a = 2 WHERE a = 1",
        class: GapClass::Control,
    },
    // REPLACE-as-statement (SQLite spells the MySQL surface too): a control the fitted
    // preset's `mutation_syntax.replace_into` parses.
    Probe {
        sql: "REPLACE INTO t(a, b) VALUES (1, 'x')",
        class: GapClass::Control,
    },
    // ON CONFLICT upsert + RETURNING: controls the fitted preset's
    // `on_conflict`/`returning` flags parse.
    Probe {
        sql: "INSERT INTO t(a, b) VALUES (1, 'x') ON CONFLICT(a) DO UPDATE SET b = 'y'",
        class: GapClass::Control,
    },
    Probe {
        sql: "INSERT INTO t(a, b) VALUES (1, 'x') ON CONFLICT DO NOTHING",
        class: GapClass::Control,
    },
    Probe {
        sql: "INSERT INTO t(a, b) VALUES (1, 'x') RETURNING a",
        class: GapClass::Control,
    },
    Probe {
        sql: "UPDATE t SET b = 'y' WHERE a = 1 RETURNING b",
        class: GapClass::Control,
    },
    Probe {
        sql: "DELETE FROM t WHERE a = 1 RETURNING a",
        class: GapClass::Control,
    },
    // CREATE INDEX family: plain / unique / expression indexes the shared surface
    // parses, and the partial-index `WHERE` tail is the fitted preset's
    // `partial_index` flag — all controls under SQLite.
    Probe {
        sql: "CREATE INDEX idx_t_a ON t(a)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE UNIQUE INDEX idx_t_b ON t(b)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE INDEX idx_t_expr ON t(a + c)",
        class: GapClass::Control,
    },
    Probe {
        sql: "CREATE INDEX idx_t_partial ON t(a) WHERE a > 0",
        class: GapClass::Control,
    },
    // CREATE TRIGGER: the compound-statement family (BEGIN ... END), now parsed by the
    // fitted preset (`schema_change_syntax.create_trigger`, closed by
    // `sqlite-utility-and-trigger-statements`) — a control.
    Probe {
        sql: "CREATE TRIGGER trg AFTER INSERT ON t BEGIN UPDATE t SET c = c + 1; END",
        class: GapClass::Control,
    },
    // Bitwise operators: the fitted preset now parses the shared `| & ~ << >>` family
    // (`bitwise-operators-cross-dialect-gap`, `OperatorSyntax::bitwise_operators`), so
    // this is a control. Kept schema-dependent so the sweep exercises it over real columns.
    Probe {
        sql: "SELECT a & c FROM t",
        class: GapClass::Control,
    },
];

/// The SQLite feature-probe statements: one per non-blank, non-`--` line.
fn sqlite_feature_statements() -> Vec<&'static str> {
    SQLITE_FEATURES
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("--"))
        .collect()
}

/// Pinned count of the authored feature-probe corpus (updated when a family is added).
/// 54 -> 57: `integer-display-width-mysql-sqlite` adds three built-in-integer display
/// width probes (`INT(11)`, `INT(11) NOT NULL DEFAULT -1`, `BIGINT(20)`) — each
/// engine-measured-accepted by rusqlite and now parsed by the fitted preset, so they
/// land as agree-accept (not gaps).
const SQLITE_FEATURE_PINNED: usize = 57;

/// How a corpus is provisioned for the schema-shadowed direction.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Provisioning {
    /// No vendored schema — arbitrary multi-dialect identifiers, so binding rejects are
    /// counted-and-pinned residual (the ticket's STOP fallback), never a fake schema.
    None,
    /// Self-contained: the corpus carries its own `CREATE` DDL inline, replayed
    /// positionally so each statement sees its schema epoch (see [`corpus_verdicts`]).
    SelfContainedReplay,
}

/// A vendored/authored corpus swept against the oracle.
struct Corpus {
    label: &'static str,
    statements: Vec<&'static str>,
    /// Total swept statements (anti-vanishing).
    pinned_total: usize,
    /// Coverage gaps (SQLite accepts, we reject) under the setup driver — the clean
    /// inventory this programme drives to zero. Drift here is the meaningful signal.
    pinned_gaps: usize,
    /// Syntax over-acceptances (we accept, SQLite syntax-rejects) *not* individually
    /// listed in [`SQLITE_DIVERGENCE_ALLOWLIST`] — the multi-dialect backlog owned by
    /// [`SQLITE_OVER_ACCEPTANCE_TICKET`]. Pinned so no NEW over-acceptance can appear
    /// silently, and a family closed in the parser drifts a pin (ledger staleness at
    /// corpus granularity — closing them one-by-one by exact SQL is impractical: the
    /// sqlglot-complex over-acceptances are 100-line TPC-DS queries).
    pinned_over_accept: usize,
    provisioning: Provisioning,
}

fn corpora() -> Vec<Corpus> {
    vec![
        Corpus {
            label: "sqlglot",
            statements: sqlglot_identity_lines(),
            pinned_total: 955,
            // `sqlite-liberal-type-names` closed the last gap here: SQLite accepts `a INT
            // AUTO_INCREMENT` as a multi-word affinity type name (`INT` typed head + the
            // trailing `AUTO_INCREMENT` word), now modelled by the `liberal_type_names` gate's
            // `DataType::Liberal` fallback.
            pinned_gaps: 0,
            pinned_over_accept: 0,
            provisioning: Provisioning::None,
        },
        Corpus {
            label: "sqllogictest",
            statements: sqllogictest_lines(),
            pinned_total: 373,
            pinned_gaps: SQLLOGICTEST_GAP_PIN,
            pinned_over_accept: 0,
            provisioning: Provisioning::SelfContainedReplay,
        },
        Corpus {
            label: "sqlglot-complex",
            statements: sqlglot_complex_statements(),
            pinned_total: 238,
            pinned_gaps: 0,
            pinned_over_accept: 0,
            provisioning: Provisioning::None,
        },
        Corpus {
            label: "sqlite-features",
            statements: sqlite_feature_statements(),
            pinned_total: SQLITE_FEATURE_PINNED,
            // No remaining gaps: the `?NNN` numbered-parameter probes — the last sqlite-features
            // coverage gaps — landed with `sqlite-lexer-under-acceptance-bundle`.
            pinned_gaps: 0,
            pinned_over_accept: 0,
            provisioning: Provisioning::None,
        },
    ]
}

/// Minimum distinct statements the assessment sweeps (umbrella ticket target).
const MIN_SWEPT: usize = 1_000;

// --- Over-acceptance ledger (the shared PG-ledger pattern, exact SQL) ------------------

/// Current SQLite over-acceptances allowed by the gate (exact SQL, staleness enforced) —
/// each a [`DivergenceEntry`](crate::verdict_harness::DivergenceEntry): a statement our
/// fitted `Sqlite` surface accepts that SQLite *syntax*-rejects, a real
/// validator-correctness divergence we knowingly tolerate (fixing it is a parser-crate
/// tightening, outside this conformance ticket). The escape hatch for individually-triaged
/// singular cases; the multi-dialect *bulk* (133 at baseline) is instead accounted at
/// corpus granularity via each corpus's [`Corpus::pinned_over_accept`], owned by
/// [`SQLITE_OVER_ACCEPTANCE_TICKET`], because exact-SQL-listing 100-line TPC-DS queries is
/// impractical (the DuckDB family-count discipline for a large divergence set).
pub const SQLITE_DIVERGENCE_ALLOWLIST: &[DivergenceEntry] = SQLITE_DIVERGENCE_ALLOWLIST_ENTRIES;

/// The tracking ticket that owns the multi-dialect syntax over-acceptance backlog the
/// at-scale gate baselined (the fitted `Sqlite` preset admits syntax SQLite rejects;
/// tightening it is parser-crate work, out of this conformance ticket's scope). Every
/// corpus's [`Corpus::pinned_over_accept`] count is "allowlisted" by this ticket — a
/// closed family drifts a pin and forces a reviewed re-baseline, so nothing stays
/// silently allowlisted.
const SQLITE_OVER_ACCEPTANCE_TICKET: &str = "sqlite-preset-over-acceptance-tightening";

// --- Reject-reason classification (splits binding from syntax) ------------------------

/// Map a SQLite reject message onto the shared [`RejectReason`] trichotomy: a syntax
/// reject is schema-independent (SQLite reports it before name resolution), so read off
/// the *bare* probe it settles the syntax/binding split even when SQLite rejects with the
/// schema too. The message strings are the SQLite-specific part.
fn classify_sqlite_reject(err: &str) -> RejectReason {
    let e = err.to_ascii_lowercase();
    if e.contains("syntax error")
        || e.contains("unrecognized token")
        || e.contains("incomplete input")
    {
        RejectReason::Syntax
    } else if e.contains("no such table")
        || e.contains("no such column")
        || e.contains("no such function")
        || e.contains("no such collation")
        || e.contains("no such module")
        || e.contains("no such index")
        || e.contains("no such view")
        || e.contains("no such trigger")
        || e.contains("ambiguous column name")
        || e.contains("has no column named")
        || e.contains("already exists")
        || e.contains("not found")
        || e.contains("unknown function")
    {
        RejectReason::Binding
    } else {
        RejectReason::Other
    }
}

/// The bare reject reason for `sql` (only meaningful when the bare probe rejects it).
fn bare_reject_reason(probe: &SqliteConnection, sql: &str) -> RejectReason {
    match probe.prepare(sql) {
        Ok(_) => RejectReason::Other,
        Err(err) => classify_sqlite_reject(&err.to_string()),
    }
}

// --- Positional-replay provisioning (self-contained corpora) --------------------------

/// Whether `sql` is a schema-advancing `CREATE` the replay executes onto the live
/// connection (the setup driver). Only `CREATE {TABLE|INDEX|VIEW|TRIGGER}` mutate the
/// schema an accept/reject `prepare` binds against; `INSERT`/`SELECT` never do.
fn is_provisioning_ddl(sql: &str) -> bool {
    ddl_kind_and_name(sql).is_some()
}

/// Extract the object kind + name from a `CREATE …` statement, so a redefinition can be
/// `DROP`ped before it is recreated (the corpus redefines tables with no `DROP`s). Bare
/// identifiers only — the self-contained corpus uses no quoting/schema-qualification;
/// anything unrecognized returns `None` and is simply not treated as provisioning DDL
/// (a best-effort miss only shrinks unblinding, never fires a false gate).
fn ddl_kind_and_name(sql: &str) -> Option<(&'static str, String)> {
    let tokens: Vec<&str> = sql
        .split(|c: char| c.is_whitespace() || c == '(')
        .filter(|t| !t.is_empty())
        .collect();
    let upper: Vec<String> = tokens.iter().map(|t| t.to_ascii_uppercase()).collect();
    if upper.first().map(String::as_str) != Some("CREATE") {
        return None;
    }
    let kind_idx = upper
        .iter()
        .position(|t| matches!(t.as_str(), "TABLE" | "INDEX" | "VIEW" | "TRIGGER"))?;
    let kind = match upper[kind_idx].as_str() {
        "TABLE" => "TABLE",
        "INDEX" => "INDEX",
        "VIEW" => "VIEW",
        "TRIGGER" => "TRIGGER",
        _ => return None,
    };
    let mut name_idx = kind_idx + 1;
    while name_idx < upper.len() && matches!(upper[name_idx].as_str(), "IF" | "NOT" | "EXISTS") {
        name_idx += 1;
    }
    tokens.get(name_idx).map(|name| (kind, (*name).to_string()))
}

/// Advance the live schema by one `CREATE`. Drops any prior object of the same
/// name/kind first (the corpus redefines tables across concatenated scripts with no
/// `DROP`s), so each subsequent query binds against its schema epoch. Best-effort: a
/// failed provision only leaves a statement in the counted binding residual.
fn provision_advance(live: &SqliteConnection, sql: &str) {
    if let Some((kind, name)) = ddl_kind_and_name(sql) {
        let _ = live.execute_batch(&format!("DROP {kind} IF EXISTS {name}"));
    }
    let _ = live.execute_batch(sql);
}

// --- One statement's verdict against the oracle ---------------------------------------

/// Every statement's verdict for one corpus, provisioned per its strategy — the
/// per-statement `(sql, `[`Verdict`]`)` the shared quadrant tallies. The bare/schema
/// accept split and its derivations live in
/// [`Verdict`](crate::verdict_harness::Verdict).
fn corpus_verdicts(
    corpus: &Corpus,
    bare: &SqliteOracle,
    bare_probe: &SqliteConnection,
) -> Vec<(&'static str, Verdict)> {
    let one = |sql: &'static str, bare_accepts: bool, schema_accepts: bool| {
        (
            sql,
            Verdict {
                ours: parse_with(sql, Sqlite).is_ok(),
                bare_accepts,
                schema_accepts,
                bare_reason: if bare_accepts {
                    RejectReason::Other
                } else {
                    bare_reject_reason(bare_probe, sql)
                },
            },
        )
    };
    match corpus.provisioning {
        Provisioning::None => corpus
            .statements
            .iter()
            .map(|&sql| {
                let bare_accepts = bare.verdict(sql).map(|v| v.accepts()).unwrap_or(false);
                // No schema: the schema verdict is the bare verdict, so binding rejects
                // stay in the counted residual (never unblinded, never a fake schema).
                one(sql, bare_accepts, bare_accepts)
            })
            .collect(),
        Provisioning::SelfContainedReplay => {
            let live = SqliteConnection::open_in_memory().expect("open replay connection");
            corpus
                .statements
                .iter()
                .map(|&sql| {
                    let bare_accepts = bare.verdict(sql).map(|v| v.accepts()).unwrap_or(false);
                    // Verdict against the schema in effect at this position, then advance.
                    let schema_accepts = live.prepare(sql).is_ok();
                    let verdict = one(sql, bare_accepts, schema_accepts);
                    if is_provisioning_ddl(sql) {
                        provision_advance(&live, sql);
                    }
                    verdict
                })
                .collect()
        }
    }
}

/// The at-scale parity gate: over-acceptance is ledgered (zero unallowlisted), coverage
/// gaps stay a green-by-counts inventory, and the binding residual is counted + pinned.
#[test]
fn sqlite_corpus_parity_gated_behind_setup_driver() {
    let bare = SqliteOracle::new().expect("open in-memory sqlite");
    let bare_probe = SqliteConnection::open_in_memory().expect("bare probe connection");
    let corpora = corpora();

    // Global tallies. The quadrant bookkeeping is shared (`quad`); the routing state
    // below — per-corpus maps, over-accept samples, inventories — is sqlite's own gate
    // policy over the two cells it gates.
    let mut total = 0usize;
    let mut quad = Quadrant::default();
    let mut allowlisted_over_accept = 0usize; // in the exact-SQL SQLITE_DIVERGENCE_ALLOWLIST
    let mut over_accept_other_samples: Vec<(&str, String)> = Vec::new();
    let mut per_corpus_gap: BTreeMap<&str, usize> = BTreeMap::new();
    let mut per_corpus_over_accept: BTreeMap<&str, usize> = BTreeMap::new();
    let mut inventory: Vec<(&str, &str)> = Vec::new(); // (corpus, gap sql)
    let mut over_accept_list: Vec<(&str, &str)> = Vec::new(); // (corpus, over-accepted sql)

    for corpus in &corpora {
        assert_eq!(
            corpus.statements.len(),
            corpus.pinned_total,
            "{} corpus statement count changed; if intentional, update the pin",
            corpus.label,
        );
        total += corpus.statements.len();

        for (sql, v) in corpus_verdicts(corpus, &bare, &bare_probe) {
            // `quad.record` does the shared quadrant bookkeeping; sqlite routes only the
            // two cells it gates (coverage gap -> per-corpus inventory; syntax
            // over-acceptance -> exact-SQL allowlist or the corpus-pinned bulk) plus its
            // OTHER samples.
            match quad.record(&v) {
                Cell::CoverageGap => {
                    *per_corpus_gap.entry(corpus.label).or_default() += 1;
                    inventory.push((corpus.label, sql));
                }
                Cell::OverAcceptSyntax => {
                    // Individually triaged (exact SQL) vs the corpus-count-pinned bulk.
                    if SQLITE_DIVERGENCE_ALLOWLIST.iter().any(|e| e.sql == sql) {
                        allowlisted_over_accept += 1;
                    } else {
                        *per_corpus_over_accept.entry(corpus.label).or_default() += 1;
                        over_accept_list.push((corpus.label, sql));
                    }
                }
                Cell::OverAcceptOther if over_accept_other_samples.len() < 20 => {
                    let detail = bare_probe
                        .prepare(sql)
                        .err()
                        .map(|e| e.to_string())
                        .unwrap_or_default();
                    over_accept_other_samples.push((sql, detail));
                }
                _ => {}
            }
        }
    }

    let Quadrant {
        agree_accept,
        agree_reject_syntax,
        agree_reject_binding,
        coverage_gap,
        over_accept_syntax,
        over_accept_binding,
        over_accept_other,
        newly_comparable,
        comparable,
    } = quad;
    let residual = total - comparable;

    eprintln!(
        "\n=== SQLite parity gate (fitted Sqlite vs SqliteOracle, per-corpus setup driver) ==="
    );
    eprintln!("  total statements            {total}");
    eprintln!("  agree accept (A/A)          {agree_accept}");
    eprintln!("  agree reject syntax (R/R)   {agree_reject_syntax}   <- mutual syntax reject");
    eprintln!(
        "  agree reject binding (R/R)  {agree_reject_binding}   <- masked residual (binding)"
    );
    eprintln!(
        "  COVERAGE GAP (R/A)          {coverage_gap}   <- SQLite syntax we reject (inventory)"
    );
    eprintln!(
        "  over-accept SYNTAX (A/R)    {over_accept_syntax}   <- REAL over-acceptance ({allowlisted_over_accept} exact-SQL allowlisted, rest corpus-pinned)"
    );
    eprintln!("  over-accept binding (A/R)   {over_accept_binding}   <- residual (schema miss)");
    eprintln!(
        "  over-accept other  (A/R)    {over_accept_other}   <- semantic reject (not syntax; not ledgered)"
    );
    eprintln!("  comparable                  {comparable} / {total}  (residual {residual})");
    eprintln!("  newly comparable vs bare    {newly_comparable}   <- setup driver unblinded these");
    eprintln!("\n  coverage gaps / over-acceptances by corpus:");
    for corpus in &corpora {
        let gap = per_corpus_gap.get(corpus.label).copied().unwrap_or(0);
        let over = per_corpus_over_accept
            .get(corpus.label)
            .copied()
            .unwrap_or(0);
        eprintln!(
            "    {:<16} gap {gap:>4} (pin {})   over-accept {over:>4} (pin {})",
            corpus.label, corpus.pinned_gaps, corpus.pinned_over_accept,
        );
    }

    if !over_accept_other_samples.is_empty() {
        eprintln!("\n  over-accept OTHER samples (verify none are hidden syntax rejects):");
        for (sql, detail) in &over_accept_other_samples {
            eprintln!("    {sql:?}  -- {detail}");
        }
    }

    // Full over-acceptance list — the backlog SQLITE_OVER_ACCEPTANCE_TICKET owns, printed
    // so it is re-derivable from the test log (never a loose note).
    eprintln!(
        "\n  over-acceptance backlog ({}) [{SQLITE_OVER_ACCEPTANCE_TICKET}]:",
        over_accept_list.len()
    );
    for (corpus, sql) in &over_accept_list {
        eprintln!("    A/R-syntax [{corpus}] {sql:?}");
    }

    // Full coverage-gap inventory — the printed block the child tickets classify.
    eprintln!("\n  coverage-gap inventory ({}):", inventory.len());
    for (corpus, sql) in &inventory {
        eprintln!("    GAP [{corpus}] {sql:?}");
    }

    // --- GATE 1: over-acceptance is fully accounted (zero UNaccounted) ---
    // Every "we accept ∧ SQLite syntax-rejects" is either an exact-SQL allowlist entry
    // (staleness-checked in its own test) or counted against its corpus's pin, owned by
    // the tracking ticket. A NEW over-acceptance drifts a corpus pin; a family tightened
    // in the parser drifts it the other way — neither can pass silently.
    assert!(
        ticket_exists(SQLITE_OVER_ACCEPTANCE_TICKET),
        "over-acceptance backlog ticket {SQLITE_OVER_ACCEPTANCE_TICKET} must exist",
    );
    for corpus in &corpora {
        let got = per_corpus_over_accept
            .get(corpus.label)
            .copied()
            .unwrap_or(0);
        assert_eq!(
            got, corpus.pinned_over_accept,
            "{} over-acceptance count drifted (we accept, SQLite syntax-rejects): a new \
             over-acceptance appeared or the fitted preset tightened one off — triage against \
             {SQLITE_OVER_ACCEPTANCE_TICKET} and re-baseline the pin (or add an exact-SQL \
             SQLITE_DIVERGENCE_ALLOWLIST entry for a singular case)",
            corpus.label,
        );
    }

    // --- GATE 2: coverage gaps stay a green-by-counts inventory (per corpus) ---
    for corpus in &corpora {
        let got = per_corpus_gap.get(corpus.label).copied().unwrap_or(0);
        assert_eq!(
            got, corpus.pinned_gaps,
            "{} coverage-gap count drifted; a SQLite family closed under the fitted preset or a \
             fixture changed — re-baseline the pin and update the child-ticket inventory",
            corpus.label,
        );
    }

    // --- GATE 3: setup-driver efficacy + counted residual (anti-drift) ---
    // The self-contained replay unblinds the sqllogictest binding residual; the sqlglot
    // corpora keep theirs counted (no vendored schema). These pins drift if the schema,
    // corpus, engine, or fitted preset changes, forcing a reviewed re-baseline.
    assert_eq!(
        (
            newly_comparable,
            over_accept_binding,
            over_accept_other,
            agree_reject_binding,
            residual,
        ),
        (
            NEWLY_COMPARABLE_PIN,
            OVER_ACCEPT_BINDING_PIN,
            OVER_ACCEPT_OTHER_PIN,
            AGREE_REJECT_BINDING_PIN,
            RESIDUAL_PIN,
        ),
        "setup-driver quadrant counts drifted (newly_comparable, over_accept_binding, \
         over_accept_other, agree_reject_binding, residual); re-baseline the pins",
    );

    assert!(
        total >= MIN_SWEPT,
        "sweeps {total} statements, below the {MIN_SWEPT} target",
    );
}

/// Cloned from `pg::PG_DIVERGENCE_ALLOWLIST`: every allowlisted over-acceptance must name
/// a real ticket and still actually diverge (we accept ∧ SQLite syntax-rejects), so a
/// fixed over-acceptance cannot stay silently allowlisted.
#[test]
fn sqlite_divergence_allowlist_entries_name_tickets_and_still_diverge() {
    let bare_probe = SqliteConnection::open_in_memory().expect("bare probe connection");
    assert_entries_are_ticketed(SQLITE_DIVERGENCE_ALLOWLIST);
    // Still diverges: we accept, SQLite syntax-rejects.
    assert_entries_still_diverge(SQLITE_DIVERGENCE_ALLOWLIST, |entry| {
        let ours = parse_with(entry.sql, Sqlite).is_ok();
        let sqlite = bare_probe.prepare(entry.sql).is_ok();
        let reason = bare_reject_reason(&bare_probe, entry.sql);
        ours && !sqlite && reason == RejectReason::Syntax
    });
}

/// Both-direction engine verification of the ragged-VALUES arity gate under the fitted
/// `Sqlite` preset (`sqlite-ragged-values-arity`): with
/// `SelectSyntax::values_rows_require_equal_arity` on, our SQLite parser rejects a ragged `VALUES`
/// constructor at parse, matching real SQLite (bundled `rusqlite`), which rejects it with
/// "all VALUES must have the same number of terms"; an equal-arity constructor still parses
/// on both sides. The corpora carry no ragged constructor, so this pins the behaviour the
/// preset flag drives directly against the engine.
#[test]
fn sqlite_ragged_values_arity_matches_the_engine_both_directions() {
    let bare = SqliteConnection::open_in_memory().expect("bare probe connection");

    const RAGGED: &str = "VALUES (1, 2), (3)";
    const EQUAL: &str = "VALUES (1, 2), (3, 4)";

    // Ragged rows: both the fitted preset and real SQLite reject at parse.
    assert!(
        parse_with(RAGGED, Sqlite).is_err(),
        "fitted Sqlite preset must parse-reject the ragged VALUES: {RAGGED:?}",
    );
    assert!(
        bare.prepare(RAGGED).is_err(),
        "real SQLite must reject the ragged VALUES: {RAGGED:?}",
    );

    // Equal-arity rows: both accept.
    assert!(
        parse_with(EQUAL, Sqlite).is_ok(),
        "fitted Sqlite preset must accept the equal-arity VALUES: {EQUAL:?}",
    );
    assert!(
        bare.prepare(EQUAL).is_ok(),
        "real SQLite must accept the equal-arity VALUES: {EQUAL:?}",
    );
}

/// Whether SQLite admits `sql` SYNTACTICALLY: it accepts, or it rejects only for name
/// resolution (a schema miss), never for a parse error. The syntax-vs-binding split the
/// position-aware model is measured against.
fn sqlite_admits_syntactically(conn: &SqliteConnection, sql: &str) -> bool {
    match conn.prepare(sql) {
        Ok(_) => true,
        Err(err) => {
            let e = err.to_string().to_ascii_lowercase();
            !(e.contains("syntax error")
                || e.contains("unrecognized token")
                || e.contains("incomplete input"))
        }
    }
}

/// One grammatical identifier position in the SQLite matrix: a label and a template that
/// substitutes a keyword spelling into the probe SQL.
type PositionProbe = (&'static str, fn(&str) -> String);

/// The POSITION-AWARE reserved model for SQLite (`sqlite-position-aware-reserved` +
/// `sqlite-reserved-as-label-set-residual-divergences`), proved cell-by-cell against the
/// in-process rusqlite 3.53.2 oracle. SQLite's grammar admits the seven `JOIN_KW` keywords
/// as a NAME (`nm ::= JOIN_KW` — table/column name, function name, `AS` label) but not as a
/// bare alias or a type name (`ids ::= ID|STRING`), and it reserves `ISNULL`/`NOTNULL`/
/// `RETURNING`/`NOTHING` in every name position — so the five reserved sets genuinely
/// diverge by position (they are NOT one shared set). For every {keyword × position} cell,
/// our fitted `Sqlite` parser's accept/reject must match whether SQLite admits it
/// syntactically, except two documented residuals ledgered below. This is the measured
/// oracle for the model — a reserved-set that admitted too much or too little fails here.
#[test]
fn sqlite_position_aware_reserved_matches_the_engine() {
    let conn = SqliteConnection::open_in_memory().expect("probe connection");
    conn.execute_batch(
        "CREATE TABLE t9(a INTEGER, b INTEGER); CREATE TABLE t8(a INTEGER, c INTEGER);",
    )
    .expect("provision the position-probe schema");

    // Each grammatical identifier position, as a template over a keyword. The join-operator
    // position is a different-tree probe (join vs alias), not an accept/reject cell — it is
    // covered by the explicit JOIN guard below.
    let positions: &[PositionProbe] = &[
        ("ddl-table-name", |k| format!("CREATE TABLE {k}(x INTEGER)")),
        ("from-table-ref", |k| format!("SELECT * FROM {k}")),
        ("insert-table-ref", |k| {
            format!("INSERT INTO {k} DEFAULT VALUES")
        }),
        ("ddl-column-name", |k| {
            format!("CREATE TABLE tp({k} INTEGER)")
        }),
        ("column-ref", |k| format!("SELECT {k} FROM t9")),
        ("as-label", |k| format!("SELECT 1 AS {k}")),
        ("bare-label", |k| format!("SELECT 1 {k}")),
        ("as-table-alias", |k| format!("SELECT * FROM t9 AS {k}")),
        ("bare-table-alias", |k| format!("SELECT * FROM t9 {k}")),
        ("function-name", |k| format!("SELECT {k}(1)")),
        ("cast-type-name", |k| format!("SELECT CAST(1 AS {k})")),
    ];

    // The words the position-aware model governs, plus controls at both extremes: `select`
    // (reserved in every position) and `desc` (a `%fallback ID`, free in every position).
    let keywords = [
        "cross",
        "inner",
        "left",
        "natural",
        "outer",
        "right",
        "full",
        "isnull",
        "notnull",
        "returning",
        "nothing",
        "select",
        "desc",
    ];

    // The two ledgered residuals: `ISNULL`/`NOTNULL` are admitted as a bare alias to match
    // `SELECT 1 isnull` (SQLite reads it as the postfix null-test operator, which we do not
    // model — reserving them would over-reject that common form). That admission leaks into
    // the obscure bare-TABLE-alias position `FROM t9 isnull`, where SQLite syntax-rejects
    // and we accept. Two cells, unmeasured by any corpus; closing them needs the
    // ISNULL/NOTNULL postfix operator (a separate grammar surface), so they are ledgered
    // here, not closed. [sqlite-reserved-as-label-set-residual-divergences]
    fn is_ledgered(kw: &str, label: &str) -> bool {
        matches!(
            (kw, label),
            ("isnull", "bare-table-alias") | ("notnull", "bare-table-alias")
        )
    }

    let mut untriaged = Vec::new();
    for kw in keywords {
        for (label, tmpl) in positions {
            let sql = tmpl(kw);
            let oracle = sqlite_admits_syntactically(&conn, &sql);
            let ours = parse_with(&sql, Sqlite).is_ok();
            if oracle != ours && !is_ledgered(kw, label) {
                untriaged.push(format!(
                    "[{label}] {sql:?}: sqlite_admits={oracle} ours_accepts={ours}"
                ));
            }
        }
    }
    assert!(
        untriaged.is_empty(),
        "SQLite position-matrix divergence(s) not in the ledger — fix the reserved set for the \
         position, or ledger the cell with evidence:\n{}",
        untriaged.join("\n"),
    );

    // The ledgered residuals must STILL diverge — a fixed one is swept, never re-pinned.
    for sql in ["SELECT * FROM t9 isnull", "SELECT * FROM t9 notnull"] {
        assert!(
            !sqlite_admits_syntactically(&conn, sql) && parse_with(sql, Sqlite).is_ok(),
            "ledgered residual {sql:?} no longer diverges (SQLite still syntax-rejects it, we \
             still accept it as a bare table alias) — if the ISNULL/NOTNULL postfix operator \
             landed, sweep this ledger entry",
        );
    }

    // The tickets' hard guard: a JOIN keyword in join position parses as the JOIN, never as
    // the preceding factor's alias — both readings of the ambiguous minimal pairs.
    for join in [
        "SELECT * FROM t9 cross JOIN t8",
        "SELECT * FROM t9 left JOIN t8 ON t9.a=t8.a",
        "SELECT * FROM t9 inner JOIN t8 ON t9.a=t8.a",
        "SELECT * FROM t9 right JOIN t8 ON t9.a=t8.a",
        "SELECT * FROM t9 full JOIN t8 ON t9.a=t8.a",
        "SELECT * FROM t9 natural JOIN t8",
    ] {
        assert!(
            parse_with(join, Sqlite).is_ok() && sqlite_admits_syntactically(&conn, join),
            "a JOIN keyword must still parse as the join (both us and SQLite): {join:?}",
        );
    }
}

/// SQLite ships neither the SQL:2023 recursive-query `SEARCH` clause nor the `CYCLE` clause
/// at any version — contrary to the `sqlite-cycle-without-search` ticket premise (that
/// 3.42+ added `CYCLE` without `SEARCH`). Probed against the bundled rusqlite 3.53.2: every
/// `CYCLE … SET … [TO … DEFAULT …] USING …` form — short, mark-carrying, on RECURSIVE and
/// on plain `WITH` — is rejected at the `CYCLE` keyword with `near "CYCLE": syntax error`,
/// exactly as `SEARCH` is, while the same recursive CTE without the trailing clause is
/// accepted. Both directions of the parity are asserted here so the fitted `Sqlite`
/// preset's `recursive_search_cycle: false` gate stays engine-honest: SQLite *syntax*-rejects
/// each form (so enabling the flag for SQLite would be a real over-acceptance, not a
/// name-resolution residual), and our parser rejects it too. This is the guard against a
/// future re-enable — no dialect we model admits `CYCLE` without `SEARCH`, so the paired
/// flag is deliberately not split.
#[test]
fn sqlite_rejects_search_and_cycle_clauses_like_the_engine() {
    let conn = SqliteConnection::open_in_memory().expect("probe connection");

    // Sanity: the same recursive CTE without a trailing clause is accepted by both, so a
    // rejection below is the SEARCH/CYCLE keyword, not a malformed base query.
    for base in [
        "WITH RECURSIVE t(n) AS (SELECT 1 UNION SELECT n+1 FROM t WHERE n<5) SELECT * FROM t",
        "WITH t(n) AS (SELECT 1) SELECT * FROM t",
    ] {
        assert!(
            sqlite_admits_syntactically(&conn, base) && parse_with(base, Sqlite).is_ok(),
            "the clause-free recursive CTE must parse for both us and SQLite: {base:?}",
        );
    }

    for sql in [
        // SEARCH — both orders.
        "WITH RECURSIVE t(n) AS (SELECT 1 UNION SELECT n+1 FROM t WHERE n<5) SEARCH DEPTH FIRST BY n SET seq SELECT * FROM t",
        "WITH RECURSIVE t(n) AS (SELECT 1 UNION SELECT n+1 FROM t WHERE n<5) SEARCH BREADTH FIRST BY n SET seq SELECT * FROM t",
        // CYCLE — short form, the TO/DEFAULT mark, and on a plain (non-RECURSIVE) WITH.
        "WITH RECURSIVE t(n) AS (SELECT 1 UNION SELECT n+1 FROM t WHERE n<5) CYCLE n SET c USING p SELECT * FROM t",
        "WITH RECURSIVE t(n) AS (SELECT 1 UNION SELECT n+1 FROM t WHERE n<5) CYCLE n SET c TO 1 DEFAULT 0 USING p SELECT * FROM t",
        "WITH t(n) AS (SELECT 1) CYCLE n SET c USING p SELECT * FROM t",
        // SEARCH before CYCLE (the PostgreSQL-legal pairing) — SQLite rejects at SEARCH.
        "WITH RECURSIVE t(n) AS (SELECT 1 UNION SELECT n+1 FROM t WHERE n<5) SEARCH DEPTH FIRST BY n SET seq CYCLE n SET c USING p SELECT * FROM t",
    ] {
        assert!(
            !sqlite_admits_syntactically(&conn, sql),
            "SQLite must SYNTAX-reject the SEARCH/CYCLE clause (premise says it ships CYCLE — \
             the engine disagrees): {sql:?}",
        );
        assert!(
            parse_with(sql, Sqlite).is_err(),
            "our fitted Sqlite preset must reject the SEARCH/CYCLE clause (gate off): {sql:?}",
        );
    }
}

/// The setup-driver probes: each mutation/index/trigger family is compared behind a
/// provisioned schema, so the signal is name-resolution-clean. Every probe SQLite
/// accepts; the recorded [`GapClass`] must agree with our parser's verdict — a
/// [`GapClass::Control`] is a family we already parse, every other class is a real
/// gap we reject — so the classification cannot silently rot when the parser grows.
#[test]
fn setup_driven_sqlite_probes_match_recorded_class() {
    let provisioned = SqliteOracle::with_schema(SETUP_SQL).expect("provision sweep schema");

    eprintln!("[setup-driven mutation probes] (provisioned schema):");
    let gaps = check_probe_group(
        "sqlite",
        SETUP_DRIVEN_PROBES,
        |sql| {
            provisioned
                .verdict(sql)
                .map(|v| v.accepts())
                .unwrap_or(false)
        },
        |sql| parse_with(sql, Sqlite).is_ok(),
    );
    eprintln!("  setup-driven coverage gaps: {gaps}");
    assert_eq!(
        gaps, SETUP_DRIVEN_GAP_PIN,
        "setup-driven gap count drifted; re-baseline"
    );
}

/// Pinned setup-driven coverage-gap count (probes we reject that SQLite accepts): zero —
/// the setup-driven families are all controls now. The bitwise-operator family
/// (`bitwise-operators-cross-dialect-gap`) was the last remaining gap and is now parsed
/// (`OperatorSyntax::bitwise_operators`), following `CREATE TRIGGER`
/// (`sqlite-utility-and-trigger-statements`), the `INSERT OR`/`UPDATE OR` conflict-action
/// seam (`sqlite-insert-or-action`), and the `REPLACE INTO` / `ON CONFLICT` / `RETURNING`
/// / partial-index controls the fitted preset parses.
const SETUP_DRIVEN_GAP_PIN: usize = 0;

// --- Baselined pins (from the gate's printed run) -------------------------------------
//
// Baselined against SQLite 3.x (bundled `rusqlite`) + the vendored corpora under the
// fitted `Sqlite` preset with the per-corpus setup driver. These pins record the
// self-contained replay's newly comparable set and the remaining binding/other residuals
// for corpora with no vendored schema. They are measurement baselines: a drift fails
// loudly so the inventory is re-read and re-baselined.
const SQLLOGICTEST_GAP_PIN: usize = 0;
const NEWLY_COMPARABLE_PIN: usize = 303;
// `sqlite-table-valued-pragma-functions` turned `table_functions` on, so a generic
// function-in-FROM the multi-dialect corpora carry (not a registered SQLite table
// function) now parse-accepts here and binding-rejects on the engine: two statements
// moved from `agree_reject_binding` (28 -> 26) to `over_accept_binding` (504 -> 506).
// Binding rejects are name resolution, never ledgered — the syntax over-acceptance gate
// stays green.
//
// `sqlite-liberal-type-names` then moved two more sqllogictest statements the same way
// (`agree_reject_binding` 26 -> 24, `over_accept_binding` 506 -> 508): a liberal multi-word
// affinity type name now parse-accepts here and the statement binding-rejects on the engine
// (unbound names), never a syntax over-acceptance — the gate stays green.
const OVER_ACCEPT_BINDING_PIN: usize = 508;
const OVER_ACCEPT_OTHER_PIN: usize = 14;
const AGREE_REJECT_BINDING_PIN: usize = 24;
const RESIDUAL_PIN: usize = 546;

/// Individually-triaged singular over-acceptances (the PG-ledger clone). The
/// multi-dialect over-acceptance bulk (133 at baseline) is accounted at corpus
/// granularity via [`Corpus::pinned_over_accept`] under [`SQLITE_OVER_ACCEPTANCE_TICKET`]
/// — exact-SQL-listing 100-line TPC-DS queries is impractical; the machinery here holds
/// the singular, individually-tracked cases.
const SQLITE_DIVERGENCE_ALLOWLIST_ENTRIES: &[DivergenceEntry] = &[
    // Surfaced by `integer-display-width-mysql-sqlite`: enabling the `INT(11)` display
    // width let this statement parse past its type, exposing a *pre-existing* DEFAULT
    // over-acceptance — SQLite requires `DEFAULT (expr)` for a call and syntax-rejects
    // the bare `DEFAULT UUID()`, which our column-def DEFAULT grammar accepts. The width
    // itself agrees on both sides (`INT(11)` prepares; the same line without it,
    // `INT DEFAULT UUID()`, rejects identically), so the divergence is the DEFAULT
    // clause, owned by the over-acceptance-tightening backlog — not this ticket.
    DivergenceEntry {
        sql: "CREATE TABLE z (a INT(11) DEFAULT UUID())",
        ticket: SQLITE_OVER_ACCEPTANCE_TICKET,
        reason: "SQLite requires DEFAULT (expr) for a function call; the bare DEFAULT \
                 UUID() is a syntax reject there but accepted by our DEFAULT grammar. \
                 Unblocked (not caused) by the INT(11) display width.",
    },
];

// =====================================================================================
// SQLite TCL test-suite spec-audit corpus (spec-audit-sqlite-suite-corpus)
// =====================================================================================
//
// A SECOND, independent SQLite corpus, vendored under `corpus/sqlite-testsuite/` and
// pinned to the EXACT SQLite version our in-process rusqlite oracle links (bundled
// libsqlite3-sys 0.38.1 -> SQLite 3.53.2, SOURCE_ID d6e03d8c…). Where the self-authored
// `corpus/sqlite/features.sql` group above is a hand-curated slice biased toward the
// SQLite-idiomatic families the phase-0 sweep surfaced (and is FeatureSet-closed), this
// one is a *broad*, conservatively-extracted slice of the public-domain TCL regression
// suite (`test/*.test`) — the executable spec that curation skips — MEASURED to surface
// the true residual grammar-gap inventory (the review proved CREATE VIRTUAL TABLE is
// invisible to the "0 gaps" feature-probe state). It is a pure measurement surface: its
// sweep PINS the quadrant counts and PRINTS the family/over-accept inventory, files no
// tickets and gates nothing to zero (the ranked inventory drives separate fix tickets).
// See `corpus/sqlite-testsuite/README.md` + `extract_tcl.py`.
//
// Three artifacts, one `extract_tcl.py` run (per-file caps 12 accepts / 8 rejects):
// - `statements.sql`  — flat accepts (`execsql` + `do_execsql_test` bodies), one per line.
// - `rejects.sql`     — flat rejects (`catchsql` + `do_catchsql_test` bodies) — the
//   over-acceptance differential's food. NB many are RUNTIME rejects, not parse rejects;
//   the reject classifier (`classify_sqlite_reject`) sorts syntax from binding/other.
// - `statements_with_schema.sql` — the same queries AND rejects regrouped under their
//   source `.test` file with that file's pure `CREATE TABLE` setup DDL, driving the same
//   per-file setup driver as the DuckDB core tranche (`# file:`/`# setup`/`# query`/`# reject`).
const TESTSUITE_STATEMENTS: &str = include_str!("../corpus/sqlite-testsuite/statements.sql");
const TESTSUITE_REJECTS: &str = include_str!("../corpus/sqlite-testsuite/rejects.sql");
const TESTSUITE_WITH_SCHEMA: &str =
    include_str!("../corpus/sqlite-testsuite/statements_with_schema.sql");

/// Anti-vanishing count pins, measured off the SQLite 3.53.2 extraction. A vanished or
/// drifted line trips these (the extraction is reproducible via `extract_tcl.py`).
const TESTSUITE_STATEMENTS_PINNED: usize = 2403;
const TESTSUITE_REJECTS_PINNED: usize = 555;
const TESTSUITE_GROUPED_FILES_PINNED: usize = 230;
const TESTSUITE_GROUPED_SETUP_DDL_PINNED: usize = 1196;

/// A source-file group in the testsuite corpus: the file's pure `CREATE TABLE` setup DDL
/// and the accept statements-under-test + reject bodies drawn from it. Mirrors the DuckDB
/// `CoreSchemaGroup` so one sweep measures both coverage gaps (over `queries`) and
/// over-acceptances (over `rejects`) against the same per-file provisioned schema.
struct TestsuiteGroup {
    file: &'static str,
    setup: Vec<&'static str>,
    queries: Vec<&'static str>,
    rejects: Vec<&'static str>,
}

/// Parse [`TESTSUITE_WITH_SCHEMA`] into per-file groups. Line-oriented `# file:` /
/// `# setup` / `# query` / `# reject` markers; no extracted statement begins with `#`,
/// so the markers are unambiguous.
fn testsuite_groups() -> Vec<TestsuiteGroup> {
    #[derive(Clone, Copy)]
    enum Section {
        None,
        Setup,
        Query,
        Reject,
    }
    let mut groups: Vec<TestsuiteGroup> = Vec::new();
    let mut section = Section::None;
    for line in TESTSUITE_WITH_SCHEMA.lines() {
        if let Some(file) = line.strip_prefix("# file:") {
            groups.push(TestsuiteGroup {
                file: file.trim(),
                setup: Vec::new(),
                queries: Vec::new(),
                rejects: Vec::new(),
            });
            section = Section::None;
        } else if line == "# setup" {
            section = Section::Setup;
        } else if line == "# query" {
            section = Section::Query;
        } else if line == "# reject" {
            section = Section::Reject;
        } else if !line.trim().is_empty() {
            let group = groups
                .last_mut()
                .expect("a statement line precedes its `# file:` header");
            match section {
                Section::Setup => group.setup.push(line),
                Section::Query => group.queries.push(line),
                Section::Reject => group.rejects.push(line),
                Section::None => panic!("statement outside a section: {line:?}"),
            }
        }
    }
    groups
}

/// Compose a group's setup DDL into one `execute_batch` script. Every setup line is a
/// pure single-statement `CREATE TABLE` (the extractor enforces it), so `;`-joining is
/// unambiguous.
fn testsuite_setup_script(setup: &[&str]) -> String {
    setup
        .iter()
        .map(|s| format!("{s};"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether `sql` carries a `?NNN` numbered bind parameter (`?` immediately followed by an
/// ASCII digit) — the one schema-independent SQLite family still open in the parser.
fn has_numbered_param(sql: &str) -> bool {
    let b = sql.as_bytes();
    b.windows(2).any(|w| w[0] == b'?' && w[1].is_ascii_digit())
}

/// Whether an (uppercased) statement names a table with a bare join/query keyword SQLite
/// treats as an ordinary identifier in a name position (the func8 `CREATE TABLE
/// cross(...)` / `INSERT INTO left VALUES(...)` cluster). Matched only in the two name
/// positions the corpus surfaces, so a genuine `CROSS JOIN` never tags.
fn is_keyword_named_table(upper: &str) -> bool {
    const KEYWORD_NAMES: &[&str] = &[
        "CROSS", "FULL", "INNER", "LEFT", "NATURAL", "OUTER", "RIGHT",
    ];
    KEYWORD_NAMES.iter().any(|kw| {
        upper.contains(&format!("CREATE TABLE {kw}("))
            || upper.contains(&format!("INSERT INTO {kw} "))
    })
}

/// The SQLite signature families, detected on the shapes the testsuite surfaces. Used to
/// cross-tabulate coverage gaps so each grammar-family child ticket gets a real count. A
/// statement can hit several families. The set is measurement-derived: it names the
/// families the inventory actually surfaces (see README.md), not a speculative taxonomy.
fn sqlite_signature_families(sql: &str) -> Vec<&'static str> {
    let upper = sql.to_ascii_uppercase();
    let lead = upper.trim_start();
    let mut fams = Vec::new();
    let mut push = |cond: bool, name: &'static str| {
        if cond {
            fams.push(name);
        }
    };
    // `x'..'` blob literals (uppercased so `x'` -> `X'`) — the single largest family.
    push(upper.contains("X'"), "blob-literal");
    push(
        upper.contains("CREATE VIRTUAL TABLE"),
        "create-virtual-table",
    );
    // Window functions + frame syntax (`OVER`, named `WINDOW` clause, `RANGE`/`ROWS`
    // frame bounds, `NULLS FIRST/LAST` ordering) — the second-largest family.
    push(
        upper.contains(" OVER ")
            || upper.contains(" OVER(")
            || upper.contains(") OVER")
            || upper.contains(" WINDOW ")
            || upper.contains("PRECEDING")
            || upper.contains("FOLLOWING")
            || upper.contains("NULLS FIRST")
            || upper.contains("NULLS LAST"),
        "window-frame",
    );
    // A non-reserved keyword used as a table name (SQLite permits it; func8's
    // `CREATE TABLE cross(...)` / `INSERT INTO left VALUES(...)` cluster).
    push(is_keyword_named_table(&upper), "keyword-as-name");
    // Table-valued PRAGMA function in a FROM source (`pragma_table_info('t')`).
    push(upper.contains("PRAGMA_"), "pragma-table-function");
    push(has_numbered_param(sql), "numbered-param");
    push(
        upper.contains("INDEXED BY") || upper.contains("NOT INDEXED"),
        "indexed-by",
    );
    // A NATURAL join stacked with a CROSS/INNER/LEFT/… qualifier.
    push(
        [
            "NATURAL CROSS",
            "NATURAL INNER",
            "NATURAL LEFT",
            "NATURAL RIGHT",
            "NATURAL FULL",
            "NATURAL OUTER",
        ]
        .iter()
        .any(|q| upper.contains(q)),
        "stacked-natural-join",
    );
    push(upper.contains("ON CONFLICT"), "upsert-on-conflict");
    push(upper.contains("RETURNING"), "returning");
    push(
        upper.contains(" MATCH ") || upper.contains(" GLOB ") || upper.contains(" REGEXP "),
        "match-glob-regexp",
    );
    push(lead.starts_with("PRAGMA"), "pragma");
    push(lead.starts_with("ALTER TABLE"), "alter-table");
    push(lead.starts_with("REINDEX"), "reindex");
    push(
        lead.starts_with("VACUUM") && upper.contains("INTO"),
        "vacuum-into",
    );
    push(
        lead.starts_with("ATTACH") || lead.starts_with("DETACH"),
        "attach-detach",
    );
    // Transaction control with a mode modifier (`BEGIN IMMEDIATE`/`DEFERRED`/`EXCLUSIVE`).
    push(
        lead.starts_with("BEGIN IMMEDIATE")
            || lead.starts_with("BEGIN DEFERRED")
            || lead.starts_with("BEGIN EXCLUSIVE"),
        "txn-modifier",
    );
    push(
        upper.contains("GENERATED ALWAYS") || upper.contains("GENERATED"),
        "generated-column",
    );
    push(upper.contains("WITHOUT ROWID"), "without-rowid");
    push(
        upper.contains(") STRICT") || upper.ends_with("STRICT"),
        "strict-table",
    );
    push(upper.contains("COLLATE"), "collate");
    push(upper.contains("RAISE("), "raise-trigger");
    push(upper.contains("MATERIALIZED"), "cte-materialized");
    push(
        upper.contains(" FILTER(") || upper.contains(" FILTER ("),
        "agg-filter",
    );
    push(
        upper.contains("CREATE TEMP TRIGGER") || upper.contains("CREATE TEMPORARY TRIGGER"),
        "temp-trigger",
    );
    fams
}

/// Always-on (no oracle) anti-vanishing + P1 panic/hang guard over the testsuite corpus:
/// count pins guard the fixtures + their grouped coherence, and every accept-corpus and
/// reject-corpus line runs through our parser cleanly (a verdict, never a panic/hang —
/// the P1 class this audit hunts). Runs under the `oracle-engines` gate this whole module
/// carries, i.e. in the preflight the oracle sweep also runs in.
#[test]
fn sqlite_testsuite_is_pinned_and_parses_without_panicking() {
    let stmts: Vec<&str> = TESTSUITE_STATEMENTS
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    let rejects: Vec<&str> = TESTSUITE_REJECTS
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    assert_eq!(
        stmts.len(),
        TESTSUITE_STATEMENTS_PINNED,
        "sqlite-testsuite/statements.sql count changed; re-pin TESTSUITE_STATEMENTS_PINNED",
    );
    assert_eq!(
        rejects.len(),
        TESTSUITE_REJECTS_PINNED,
        "sqlite-testsuite/rejects.sql count changed; re-pin TESTSUITE_REJECTS_PINNED",
    );

    let groups = testsuite_groups();
    assert_eq!(
        groups.len(),
        TESTSUITE_GROUPED_FILES_PINNED,
        "sqlite-testsuite grouped file-group count changed; re-pin TESTSUITE_GROUPED_FILES_PINNED",
    );
    let ddl: usize = groups.iter().map(|g| g.setup.len()).sum();
    assert_eq!(
        ddl, TESTSUITE_GROUPED_SETUP_DDL_PINNED,
        "sqlite-testsuite grouped setup-DDL count changed; re-pin TESTSUITE_GROUPED_SETUP_DDL_PINNED",
    );

    let grouped_q: std::collections::BTreeSet<&str> = groups
        .iter()
        .flat_map(|g| g.queries.iter().copied())
        .collect();
    let flat_q: std::collections::BTreeSet<&str> = stmts.iter().copied().collect();
    assert_eq!(
        flat_q, grouped_q,
        "grouped `# query` set diverged from statements.sql — regenerate both from extract_tcl.py",
    );
    let grouped_r: std::collections::BTreeSet<&str> = groups
        .iter()
        .flat_map(|g| g.rejects.iter().copied())
        .collect();
    let flat_r: std::collections::BTreeSet<&str> = rejects.iter().copied().collect();
    assert_eq!(
        flat_r, grouped_r,
        "grouped `# reject` set diverged from rejects.sql — regenerate both from extract_tcl.py",
    );
    assert!(
        groups.iter().all(|g| g.file.starts_with("test/")),
        "every testsuite group must name its upstream `test/*.test` source file",
    );

    // Every line parses to a verdict (never a panic/hang). Track the accept/reject split
    // per surface so a regression is legible without the oracle.
    let (mut acc_a, mut acc_r) = (0usize, 0usize);
    for &sql in &flat_q {
        if parse_with(sql, Sqlite).is_ok() {
            acc_a += 1;
        } else {
            acc_r += 1;
        }
    }
    let (mut rej_a, mut rej_r) = (0usize, 0usize);
    for &sql in &flat_r {
        if parse_with(sql, Sqlite).is_ok() {
            rej_a += 1;
        } else {
            rej_r += 1;
        }
    }
    eprintln!(
        "squonk Sqlite over the testsuite tranche: accepts {acc_a}/{} parse-accept, \
         rejects {rej_a}/{} parse-accept",
        acc_a + acc_r,
        rej_a + rej_r,
    );
    assert!(
        acc_a > 0 && rej_r > 0,
        "parser produced a degenerate split over the testsuite tranche",
    );
}

/// Measured baseline pins for the testsuite spec-audit inventory under SQLite 3.53.2 +
/// the fitted `Sqlite` preset, with the per-file setup driver. Each is the seven-tuple
/// `(agree_accept, coverage_gap, over_accept_syntax, over_accept_binding,
/// over_accept_other, agree_reject_syntax, agree_reject_binding)`. Measurement baseline,
/// not a zero gate: a drift fails loudly so the inventory is re-read and re-baselined.
///
/// over_accept_syntax is pinned 0 on both surfaces: the SQLite-specific accept forms this
/// corpus exercises (postfix `COLLATE`; table-valued pragma functions; empty `IN ()`;
/// string-literal `AS` aliases and relation targets; `INDEXED BY`/`NOT INDEXED`; liberal
/// multi-word type names; the two-word `NOT NULL` postfix; `COLLATE`/`ASC`/`DESC` in
/// indexed-column lists; name-only `DROP TRIGGER`) parse-accept and round-trip faithfully, so
/// none is a syntax over-acceptance. Where such a form names an undefined collation, a
/// non-existent index, or an unprovisioned reference, SQLite *binding*-rejects it, so it counts
/// in over_accept_binding — the documented never-ledgered noise class, never the syntax floor.
const TESTSUITE_ACCEPT_QUADRANT: (usize, usize, usize, usize, usize, usize, usize) =
    (1519, 1, 0, 817, 18, 24, 24);
const TESTSUITE_REJECT_QUADRANT: (usize, usize, usize, usize, usize, usize, usize) =
    // over_accept_syntax is pinned 0 on the reject surface. A syntactically-valid reject-corpus
    // body SQLite rejects only at binding/runtime (a `catchsql` `INDEXED BY` naming no such
    // index, a `NOT NULL` trigger over an unprovisioned reference, a bare `DROP TRIGGER` over no
    // such trigger, …) parse-accepts, matching SQLite's own parser, and counts in
    // over_accept_binding — the never-ledgered binding-noise class, never the syntax floor.
    (167, 5, 0, 283, 47, 26, 27);
const TESTSUITE_PROVISIONING_FAILED_PIN: usize = 19;

/// The spec-audit inventory: routes the broad testsuite tranche through the in-process
/// `SqliteOracle` behind a per-file setup driver, tallies both accept and reject surfaces
/// into the shared [`Quadrant`], cross-tabulates coverage gaps by SQLite signature family,
/// and PRINTS the ranked inventory + minimal over-acceptance list. Pure measurement: the
/// quadrant tuples are pinned (anti-drift), nothing is gated to zero.
#[test]
fn sqlite_testsuite_spec_audit_inventory() {
    let bare = SqliteOracle::new().expect("open in-memory sqlite");
    let bare_probe = SqliteConnection::open_in_memory().expect("bare probe connection");
    let groups = testsuite_groups();

    let mut q_accept = Quadrant::default();
    let mut q_reject = Quadrant::default();
    let mut provisioning_failed = 0usize;
    let mut gaps_by_family: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut unclassified_gaps = 0usize;
    let mut coverage_gaps: Vec<(&str, &str)> = Vec::new();
    let mut accept_over_syntax: Vec<(&str, &str)> = Vec::new();
    let mut reject_over_syntax: Vec<(&str, &str)> = Vec::new();

    for group in &groups {
        let schema = if group.setup.is_empty() {
            None
        } else {
            match SqliteOracle::with_schema(&testsuite_setup_script(&group.setup)) {
                Ok(oracle) => Some(oracle),
                Err(_) => {
                    provisioning_failed += 1;
                    None
                }
            }
        };
        let verdict_of = |sql: &str| -> Verdict {
            let ours = parse_with(sql, Sqlite).is_ok();
            let bare_accepts = bare.verdict(sql).map(|v| v.accepts()).unwrap_or(false);
            let schema_accepts = match &schema {
                Some(oracle) => oracle.verdict(sql).map(|v| v.accepts()).unwrap_or(false),
                None => bare_accepts,
            };
            let sqlite_accepts = bare_accepts || schema_accepts;
            let bare_reason = if sqlite_accepts {
                RejectReason::Other
            } else {
                bare_reject_reason(&bare_probe, sql)
            };
            Verdict {
                ours,
                bare_accepts,
                schema_accepts,
                bare_reason,
            }
        };

        for &sql in &group.queries {
            let v = verdict_of(sql);
            match q_accept.record(&v) {
                Cell::CoverageGap => {
                    coverage_gaps.push((group.file, sql));
                    let fams = sqlite_signature_families(sql);
                    if fams.is_empty() {
                        unclassified_gaps += 1;
                    }
                    for fam in fams {
                        *gaps_by_family.entry(fam).or_default() += 1;
                    }
                }
                Cell::OverAcceptSyntax => accept_over_syntax.push((group.file, sql)),
                _ => {}
            }
        }
        for &sql in &group.rejects {
            let v = verdict_of(sql);
            if let Cell::OverAcceptSyntax = q_reject.record(&v) {
                reject_over_syntax.push((group.file, sql));
            }
        }
    }

    let a = &q_accept;
    let r = &q_reject;
    eprintln!(
        "\n=== SQLite testsuite spec-audit inventory (Sqlite vs SqliteOracle, 3.53.2, \
         per-file setup driver) ==="
    );
    eprintln!(
        "  source-file groups          {} ({provisioning_failed} provisioning-failed)",
        groups.len(),
    );
    eprintln!("\n  ACCEPT surface (`execsql` + `do_execsql_test` bodies):");
    eprintln!("    agree accept (A/A)        {}", a.agree_accept);
    eprintln!(
        "    COVERAGE GAP (R/A)        {}   <- SQLite accepts, we reject",
        a.coverage_gap,
    );
    eprintln!(
        "    over-accept SYNTAX (A/R)  {}   <- we accept, SQLite syntax-rejects",
        a.over_accept_syntax,
    );
    eprintln!("    over-accept binding (A/R) {}", a.over_accept_binding);
    eprintln!("    over-accept other  (A/R)  {}", a.over_accept_other);
    eprintln!("    agree reject syntax (R/R) {}", a.agree_reject_syntax);
    eprintln!("    agree reject binding(R/R) {}", a.agree_reject_binding);
    eprintln!(
        "\n  REJECT surface (`catchsql` + `do_catchsql_test` bodies — the over-accept differential):"
    );
    eprintln!(
        "    over-accept SYNTAX (A/R)  {}   <- we accept, SQLite syntax-rejects (REAL over-accept)",
        r.over_accept_syntax,
    );
    eprintln!("    over-accept binding (A/R) {}", r.over_accept_binding);
    eprintln!(
        "    over-accept other  (A/R)  {}   <- we accept, SQLite rejects (runtime/semantic)",
        r.over_accept_other,
    );
    eprintln!(
        "    agree reject syntax (R/R) {}   <- both reject at parse",
        r.agree_reject_syntax,
    );
    eprintln!("    agree reject binding(R/R) {}", r.agree_reject_binding);
    eprintln!(
        "    agree accept (A/A)        {}   <- SQLite prepares it (error is runtime-only)",
        r.agree_accept,
    );

    eprintln!("\n  coverage gaps by signature family (accept surface):");
    for (fam, count) in &gaps_by_family {
        eprintln!("    {fam:22} {count:>4}");
    }
    eprintln!("    {:22} {unclassified_gaps:>4}", "(unclassified)");

    eprintln!(
        "\n  --- coverage gaps ({}) [SQLite accepts, we reject] ---",
        coverage_gaps.len()
    );
    for (file, sql) in &coverage_gaps {
        eprintln!("    R/A [{file}] {sql:?}");
    }
    eprintln!(
        "\n  --- accept-surface over-acceptances ({}) [we accept, SQLite syntax-rejects] ---",
        accept_over_syntax.len()
    );
    for (file, sql) in &accept_over_syntax {
        eprintln!("    A/R [{file}] {sql:?}");
    }
    eprintln!(
        "\n  --- reject-surface over-acceptances ({}) [we accept, SQLite syntax-rejects] ---",
        reject_over_syntax.len()
    );
    for (file, sql) in &reject_over_syntax {
        eprintln!("    A/R [{file}] {sql:?}");
    }

    let got_accept = (
        a.agree_accept,
        a.coverage_gap,
        a.over_accept_syntax,
        a.over_accept_binding,
        a.over_accept_other,
        a.agree_reject_syntax,
        a.agree_reject_binding,
    );
    let got_reject = (
        r.agree_accept,
        r.coverage_gap,
        r.over_accept_syntax,
        r.over_accept_binding,
        r.over_accept_other,
        r.agree_reject_syntax,
        r.agree_reject_binding,
    );
    eprintln!("\n  MEASURED accept quadrant tuple: {got_accept:?}");
    eprintln!("  MEASURED reject quadrant tuple: {got_reject:?}");
    eprintln!("  MEASURED provisioning-failed:   {provisioning_failed}");

    assert_eq!(
        got_accept, TESTSUITE_ACCEPT_QUADRANT,
        "testsuite ACCEPT quadrant drifted; re-baseline TESTSUITE_ACCEPT_QUADRANT",
    );
    assert_eq!(
        got_reject, TESTSUITE_REJECT_QUADRANT,
        "testsuite REJECT quadrant drifted; re-baseline TESTSUITE_REJECT_QUADRANT",
    );
    assert_eq!(
        provisioning_failed, TESTSUITE_PROVISIONING_FAILED_PIN,
        "testsuite provisioning-failed count drifted; re-baseline TESTSUITE_PROVISIONING_FAILED_PIN",
    );
}

// =====================================================================================
//
// The grammar denominator: the canonical top-level command families of SQLite's `cmd`
// production, extracted from the pinned public-domain `src/parse.y` (the same
// 3.53.2 source tree the TCL corpus and the bundled `rusqlite` oracle come from) by
// `corpus/sqlite-testsuite/extract_cmd_productions.py`. Independent negative space: this
// names a command even when no vendored statement happens to exercise it — the SQLite
// analogue of pg-regress `stmt-productions.txt` (`corpus_pg_verdicts`). `EXPLAIN` /
// `EXPLAIN QUERY PLAN` are excluded on purpose: the grammar makes them an `ecmd`-level
// prefix wrapping any `cmd`, not a `cmd` alternative (see the extractor's docstring).
const SQLITE_COMMANDS: &str = include_str!("../corpus/sqlite-testsuite/commands.txt");

/// The canonical top-level command inventory (parse.y `cmd` alternatives, resolved to
/// command names). A production stays visible here with no corpus statement behind it.
fn sqlite_commands() -> std::collections::BTreeSet<&'static str> {
    SQLITE_COMMANDS.lines().filter(|l| !l.is_empty()).collect()
}

/// The depth-0 word tokens of already-uppercased `upper`: maximal ASCII
/// alnum/underscore runs that sit at paren-depth 0, outside string/identifier quotes
/// (`'…'`, `"…"`, `` `…` ``, `[…]`) and `--` / `/* */` comments. Reads a statement's
/// leading command keyword past parenthesized subqueries (a CTE body's inner `SELECT` is
/// at depth > 0, so it never masks the fronted command).
fn depth0_keywords(upper: &str) -> Vec<&str> {
    let b = upper.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
            }
            q @ (b'\'' | b'"' | b'`') => {
                i += 1;
                while i < b.len() {
                    if b[i] == q {
                        // A doubled quote is an escaped quote, not a terminator.
                        if i + 1 < b.len() && b[i + 1] == q {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
            }
            b'[' => {
                i += 1;
                while i < b.len() && b[i] != b']' {
                    i += 1;
                }
                i += usize::from(i < b.len());
            }
            b'-' if i + 1 < b.len() && b[i + 1] == b'-' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(b.len());
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                if depth == 0 {
                    out.push(&upper[start..i]);
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// Map a DML head keyword to its command family (the `with`-prefixed alternatives and
/// the `VALUES` form of `select`).
fn dml_head(word: &str) -> Option<&'static str> {
    match word {
        "SELECT" | "VALUES" => Some("SELECT"),
        "INSERT" | "REPLACE" => Some("INSERT"),
        "UPDATE" => Some("UPDATE"),
        "DELETE" => Some("DELETE"),
        _ => None,
    }
}

/// Resolve a `CREATE …` head to its command family, skipping the `TEMP`/`TEMPORARY`
/// (table/view/trigger) and `UNIQUE` (index) modifiers to the object keyword.
fn create_command(rest: &[&str]) -> Option<&'static str> {
    let obj = rest
        .iter()
        .copied()
        .find(|w| !matches!(*w, "TEMP" | "TEMPORARY" | "UNIQUE"))?;
    match obj {
        "TABLE" => Some("CREATE TABLE"),
        "VIEW" => Some("CREATE VIEW"),
        "INDEX" => Some("CREATE INDEX"),
        "TRIGGER" => Some("CREATE TRIGGER"),
        "VIRTUAL" => Some("CREATE VIRTUAL TABLE"),
        _ => None,
    }
}

/// The canonical top-level command family of `sql`, by leading keyword(s) — the SQLite
/// analogue of mapping a raw parse node back to its `cmd` alternative (`corpus_pg_verdicts`
/// does this from the pg_query node kind; SQLite's `prepare` oracle exposes no node, and
/// the `cmd` grammar is keyword-dispatched, so the leading keywords are the faithful key).
/// A leading `WITH [RECURSIVE] <cte-list>` prefix is skipped to the DML head it fronts,
/// and an `EXPLAIN [QUERY PLAN]` prefix is stripped to the wrapped command. `None` means
/// the leading keywords name no top-level command (e.g. a bare expression body).
fn sqlite_command(sql: &str) -> Option<&'static str> {
    let upper = sql.to_ascii_uppercase();
    let mut toks = depth0_keywords(&upper);
    if toks.first() == Some(&"EXPLAIN") {
        toks.remove(0);
        if toks.first() == Some(&"QUERY") {
            toks.remove(0);
            if toks.first() == Some(&"PLAN") {
                toks.remove(0);
            }
        }
    }
    match toks.as_slice() {
        ["WITH", rest @ ..] => {
            let rest = match rest {
                ["RECURSIVE", tail @ ..] => tail,
                tail => tail,
            };
            rest.iter().copied().find_map(dml_head)
        }
        ["SELECT", ..] | ["VALUES", ..] => Some("SELECT"),
        ["INSERT", ..] | ["REPLACE", ..] => Some("INSERT"),
        ["UPDATE", ..] => Some("UPDATE"),
        ["DELETE", ..] => Some("DELETE"),
        ["CREATE", rest @ ..] => create_command(rest),
        ["DROP", obj, ..] => match *obj {
            "TABLE" => Some("DROP TABLE"),
            "VIEW" => Some("DROP VIEW"),
            "INDEX" => Some("DROP INDEX"),
            "TRIGGER" => Some("DROP TRIGGER"),
            _ => None,
        },
        ["ALTER", "TABLE", ..] => Some("ALTER TABLE"),
        ["PRAGMA", ..] => Some("PRAGMA"),
        ["BEGIN", ..] => Some("BEGIN"),
        ["COMMIT", ..] | ["END", ..] => Some("COMMIT"),
        ["ROLLBACK", ..] => Some("ROLLBACK"),
        ["SAVEPOINT", ..] => Some("SAVEPOINT"),
        ["RELEASE", ..] => Some("RELEASE"),
        ["VACUUM", ..] => Some("VACUUM"),
        ["ATTACH", ..] => Some("ATTACH"),
        ["DETACH", ..] => Some("DETACH"),
        ["REINDEX", ..] => Some("REINDEX"),
        ["ANALYZE", ..] => Some("ANALYZE"),
        _ => None,
    }
}

/// SQLite `cmd` command families not reached by any oracle-accepted statement in the
/// vendored TCL corpus. A measured pin, not a support claim: an exercised command still
/// has many unexercised sub-productions, and an unexercised command is negative space the
/// authored probes below close. Both remaining members are savepoint-family utility
/// statements the conservative TCL extractor did not surface (no bare `SAVEPOINT`/`RELEASE`
/// block); the fitted `Sqlite` preset already parses both (`parser/tcl.rs`).
const SQLITE_TESTSUITE_UNEXERCISED_COMMANDS: &[&str] = &["RELEASE", "SAVEPOINT"];

/// Map every SQLite-accepted TCL-corpus statement (accept + reject surfaces, behind the
/// per-file setup driver) back to its top-level `cmd` command family, and pin the exact
/// unexercised set. This measures whether the executable spec reaches each top-level
/// command production; it does not claim coverage of the sub-productions inside a reached
/// command. Distinct from `sqlite_testsuite_spec_audit_inventory`, which measures the
/// squonk-vs-oracle accept/reject quadrants — here the denominator is the grammar.
#[test]
fn sqlite_testsuite_command_production_coverage_is_measured() {
    use std::collections::BTreeSet;

    let commands = sqlite_commands();
    assert_eq!(
        commands.len(),
        25,
        "SQLite top-level cmd command count drifted; regenerate commands.txt",
    );

    let bare = SqliteOracle::new().expect("open in-memory sqlite");
    let groups = testsuite_groups();
    let mut exercised = BTreeSet::new();
    let mut unmapped = BTreeSet::new();
    for group in &groups {
        let schema = if group.setup.is_empty() {
            None
        } else {
            SqliteOracle::with_schema(&testsuite_setup_script(&group.setup)).ok()
        };
        let oracle_accepts = |sql: &str| -> bool {
            let bare_ok = bare.verdict(sql).map(|v| v.accepts()).unwrap_or(false);
            let schema_ok = match &schema {
                Some(oracle) => oracle.verdict(sql).map(|v| v.accepts()).unwrap_or(false),
                None => bare_ok,
            };
            bare_ok || schema_ok
        };
        for &sql in group.queries.iter().chain(group.rejects.iter()) {
            if !oracle_accepts(sql) {
                continue;
            }
            match sqlite_command(sql) {
                Some(command) => {
                    assert!(
                        commands.contains(command),
                        "mapped command {command:?} is absent from commands.txt: {sql:?}",
                    );
                    exercised.insert(command);
                }
                None => {
                    unmapped.insert(sql.to_owned());
                }
            }
        }
    }

    let unexercised: Vec<_> = commands.difference(&exercised).copied().collect();
    eprintln!(
        "\nSQLite 3.53.2 top-level cmd command coverage from the TCL testsuite: {}/{} ({:.1}%)",
        exercised.len(),
        commands.len(),
        100.0 * exercised.len() as f64 / commands.len() as f64,
    );
    eprintln!("  exercised:   {exercised:?}");
    eprintln!("  UNEXERCISED: {unexercised:?}");
    eprintln!("  UNMAPPED oracle-accepted statements: {unmapped:?}");
    assert!(
        unmapped.is_empty(),
        "oracle-accepted corpus statements reached no top-level command mapping",
    );

    assert_eq!(
        unexercised, SQLITE_TESTSUITE_UNEXERCISED_COMMANDS,
        "SQLite testsuite command coverage drifted; review both exercised and unexercised sets \
         before re-baselining SQLITE_TESTSUITE_UNEXERCISED_COMMANDS",
    );
    let pinned_exercised = commands.len() - SQLITE_TESTSUITE_UNEXERCISED_COMMANDS.len();
    assert_eq!(
        exercised.len(),
        pinned_exercised,
        "exercised and unexercised command counts must partition the grammar inventory",
    );
}

/// Authored oracle probes for the command families absent from the vendored TCL corpus
/// (`SQLITE_TESTSUITE_UNEXERCISED_COMMANDS`). Each `(command, sql, squonk_support)`:
/// the in-process `SqliteOracle` accepts `sql`, it maps to `command`, and squonk
/// acceptance is pinned INDEPENDENTLY — engine-production reach is not a squonk support
/// claim (the `corpus_pg_verdicts` probe pattern). Both probes here parse under the fitted
/// `Sqlite` preset (`parser/tcl.rs`); the negative space they close is corpus absence, not
/// a parser gap. Self-authored one-liners (public-domain-trivial, CC0-1.0 like the sibling
/// `corpus/sqlite` group).
const SQLITE_UNEXERCISED_COMMAND_PROBES: &[(&str, &str, bool)] = &[
    ("RELEASE", "RELEASE sp1", true),
    ("SAVEPOINT", "SAVEPOINT sp1", true),
];

/// Close the TCL corpus's command negative space with an authored oracle probe per
/// unexercised family, pinning squonk acceptance separately (mirrors
/// `pg_unexercised_statement_productions_have_permanent_oracle_probes`). Every probe must
/// reach its expected command and be oracle-accepted; the probe set must EXACTLY partition
/// `SQLITE_TESTSUITE_UNEXERCISED_COMMANDS`. SQLite has no grammar-present-but-engine-
/// unimplemented `cmd` (unlike PostgreSQL's `CreateAssertionStmt`), so the probes alone
/// account for the whole gap.
#[test]
fn sqlite_unexercised_command_productions_have_permanent_oracle_probes() {
    use std::collections::BTreeSet;

    let commands = sqlite_commands();
    let bare = SqliteOracle::new().expect("open in-memory sqlite");
    let unexercised: BTreeSet<_> = SQLITE_TESTSUITE_UNEXERCISED_COMMANDS
        .iter()
        .copied()
        .collect();
    let mut probed = BTreeSet::new();

    for &(expected_command, sql, expected_squonk_support) in SQLITE_UNEXERCISED_COMMAND_PROBES {
        assert!(
            bare.verdict(sql).map(|v| v.accepts()).unwrap_or(false),
            "SqliteOracle rejected the {expected_command} probe {sql:?}",
        );
        assert_eq!(
            sqlite_command(sql),
            Some(expected_command),
            "probe reached the wrong top-level command: {sql:?}",
        );
        assert_eq!(
            parse_with(sql, Sqlite).is_ok(),
            expected_squonk_support,
            "squonk support changed for {expected_command}; review the coverage boundary",
        );
        assert!(
            commands.contains(expected_command),
            "probe names an unknown command"
        );
        assert!(probed.insert(expected_command), "duplicate command probe");
    }

    assert_eq!(
        probed, unexercised,
        "authored probes must exactly partition the unexercised command set",
    );
    let combined = commands.len() - unexercised.len() + probed.len();
    eprintln!(
        "SQLite 3.53.2 top-level cmd command coverage from testsuite + authored probes: \
         {combined}/{} ({:.1}%)",
        commands.len(),
        100.0 * combined as f64 / commands.len() as f64,
    );
    assert_eq!(combined, 25, "combined command coverage pin drifted");
}

/// The command the spec-coverage programme flagged as its proof-by-example gap. At this
/// pin it is negative space in neither axis: the vendored accept corpus exercises it (5
/// `CREATE VIRTUAL TABLE` statements the `SqliteOracle` accepts) AND the fitted `Sqlite`
/// preset parses it (`StatementDdlGates::create_virtual_table`). This canonical probe pins
/// BOTH facts so a regression on either surface fails loudly, keeping engine reach and
/// squonk acceptance as separately-asserted evidence.
const SQLITE_CREATE_VIRTUAL_TABLE_PROBE: &str = "CREATE VIRTUAL TABLE vt USING fts5(a, b)";

#[test]
fn sqlite_create_virtual_table_is_corpus_covered_and_parser_supported() {
    let bare = SqliteOracle::new().expect("open in-memory sqlite");
    assert_eq!(
        sqlite_command(SQLITE_CREATE_VIRTUAL_TABLE_PROBE),
        Some("CREATE VIRTUAL TABLE"),
        "canonical CREATE VIRTUAL TABLE probe misclassified",
    );
    // Engine-production reach: the prepare-only oracle accepts CREATE VIRTUAL TABLE.
    assert!(
        bare.verdict(SQLITE_CREATE_VIRTUAL_TABLE_PROBE)
            .map(|v| v.accepts())
            .unwrap_or(false),
        "SqliteOracle must accept CREATE VIRTUAL TABLE",
    );
    // squonk acceptance (the distinct axis): the fitted preset parses it.
    assert!(
        parse_with(SQLITE_CREATE_VIRTUAL_TABLE_PROBE, Sqlite).is_ok(),
        "fitted Sqlite preset must parse CREATE VIRTUAL TABLE",
    );
    // And the vendored TCL accept corpus really carries it (not just the authored probe).
    let corpus_covers = TESTSUITE_STATEMENTS
        .lines()
        .any(|line| sqlite_command(line) == Some("CREATE VIRTUAL TABLE"));
    assert!(
        corpus_covers,
        "the vendored accept corpus must exercise CREATE VIRTUAL TABLE",
    );
}

// =====================================================================================
// Flag-aware generative differential (oracle-parity-sqlite)
// =====================================================================================
//
// The corpus sweeps above replay *authored* and *vendored* SQL. This lane instead
// GENERATES novel SQL that exercises SQLite's flag-gated misfeature surfaces (liberal
// types, string-literal identifiers, INDEXED BY, constraint COLLATE/DESC, the NOT NULL/
// ISNULL postfix null tests, bare IS, typeless columns, …) and differentials each
// generated statement against the bundled `rusqlite` engine — the user directive that
// corpus replay alone is not parity. The generator itself
// (`crate::properties::dialect_features`) is dialect-agnostic and reusable: a probe's
// `applies(&FeatureSet)` predicate self-selects per preset, so dialect #2 supplies its
// own probe table and drives this same classifier + ledger + replay gate.
//
// The classifier is the SHARED `Verdict`/`Quadrant`/`RejectReason` machinery the corpus
// sweeps use (`verdict_harness`): a generative divergence is routed to a `Cell` exactly
// as a corpus statement is. The reject-reason split is load-bearing here too — a
// generated statement referencing an object the schema lacks binding-rejects on the
// engine (a false divergence our parse-only parser never sees), so only a SYNTAX reject
// counts as a real over-acceptance.

use crate::properties::dialect_features::{
    FEATURE_SCHEMA_SETUP_SQL, SQLITE_FEATURE_PROBES, SQLITE_MISFEATURE_SEEDS, arb_feature_statement,
};

/// The generative lane's over-acceptance ledger, separate from the corpus-replay
/// [`SQLITE_DIVERGENCE_ALLOWLIST`] so the two lanes' findings stay attributable. Empty:
/// the flag-aware generator surfaces no "we accept ∧ SQLite syntax-rejects" divergence
/// over the fitted `Sqlite` preset today. A fuzz/proptest-found, minimized generative
/// over-acceptance lands here (exact SQL + ticket) with the same staleness contract as
/// every other ledger (`assert_entries_still_diverge`).
const SQLITE_GENERATIVE_DIVERGENCE_ALLOWLIST: &[DivergenceEntry] = &[];

/// The bundled SQLite version the preset is fitted + measured against.
///
/// # Version-pin / bump protocol (the ticket's engine-version story)
///
/// The oracle links **bundled** SQLite via `rusqlite = { features = ["bundled"] }`
/// (`conformance/Cargo.toml`), so the engine version is deterministic per lockfile — no
/// system library, no environment. The build is pinned to **SQLite 3.53.2** (bundled
/// `libsqlite3-sys 0.38.1`), the version `corpus/sqlite-testsuite/PROVENANCE.toml` is
/// extracted against and the `TESTSUITE_*` quadrant pins are baselined on. The fitted
/// `Sqlite` preset's misfeature surfaces were additionally engine-probed against 3.43.2,
/// so [`SQLITE_ENGINE_VERSION_FLOOR`] is the 3.43.x floor: below it, a measured accept
/// boundary cannot be trusted to match the engine the preset was fitted against.
///
/// A bump moves like a toolchain bump (ADR-0015): a new bundled SQLite can move
/// accept/reject quadrants, so bumping `rusqlite` requires re-running the oracle sweeps
/// (`cargo nextest run -p squonk-conformance --features oracle-engines`), re-reading
/// this module's and the testsuite corpus's printed quadrants, re-baselining every
/// drifted pin from the *fresh measurement* (never by arithmetic — the measured-pin policy/// -value arithmetic), and, if the TCL corpus is re-extracted, following the `regenerate`
/// recipe in `corpus/sqlite-testsuite/PROVENANCE.toml` with the matching source tarball.
const SQLITE_BUILD_PINNED_VERSION: &str = "3.53.2";

/// The lowest SQLite version the fitted preset's misfeature accept boundaries were
/// engine-probed against (3.43.2 → `3_043_002`); the runtime engine must be at or above
/// it or the generative differential's oracle verdicts are off-baseline.
const SQLITE_ENGINE_VERSION_FLOOR: i32 = 3_043_000;

/// The runtime bundled SQLite version is documented and at or above the probed floor.
/// A `rusqlite` bump that drops the engine below the floor (or moves off the pinned
/// build version) surfaces here so the bump protocol above is followed, not silently
/// absorbed.
#[test]
fn sqlite_bundled_version_is_pinned_and_documented() {
    let runtime = rusqlite::version();
    let number = rusqlite::version_number();
    eprintln!(
        "bundled SQLite: runtime={runtime} number={number} (build-pinned {SQLITE_BUILD_PINNED_VERSION}, floor {SQLITE_ENGINE_VERSION_FLOOR})",
    );
    assert!(
        number >= SQLITE_ENGINE_VERSION_FLOOR,
        "bundled SQLite {runtime} (#{number}) is below the engine-probed floor \
         {SQLITE_ENGINE_VERSION_FLOOR}; a `rusqlite` bump moved the oracle off-baseline — \
         follow the bump protocol on SQLITE_BUILD_PINNED_VERSION (re-run the oracle sweeps \
         and re-baseline the drifted pins from the fresh measurement)",
    );
    assert!(
        runtime.starts_with("3."),
        "bundled SQLite major version changed from 3.x ({runtime}); treat as a toolchain bump",
    );
}

/// One generated statement's [`Verdict`] against the bundled engine, behind the
/// feature-schema setup driver — the shared classifier the corpus sweeps use, applied to
/// generated (not vendored) SQL. `bare_accepts` covers the schema-independent and
/// CREATE-under-test forms; `schema_accepts` covers the fragments the setup driver
/// unblinds (`SELECT … FROM t INDEXED BY ix`); `bare_reason` splits a real syntax
/// over-acceptance from a name-resolution (binding) residual the schema simply misses.
fn generative_verdict(
    sql: &str,
    bare: &SqliteOracle,
    bare_probe: &SqliteConnection,
    provisioned: &SqliteOracle,
) -> Verdict {
    let ours = parse_with(sql, Sqlite).is_ok();
    let bare_accepts = bare.verdict(sql).map(|v| v.accepts()).unwrap_or(false);
    let schema_accepts = provisioned
        .verdict(sql)
        .map(|v| v.accepts())
        .unwrap_or(false);
    let sqlite_accepts = bare_accepts || schema_accepts;
    let bare_reason = if sqlite_accepts {
        RejectReason::Other
    } else {
        bare_reject_reason(bare_probe, sql)
    };
    Verdict {
        ours,
        bare_accepts,
        schema_accepts,
        bare_reason,
    }
}

/// Classify a generated statement and, if it is an UN-ledgered generative divergence,
/// return the failure detail. A syntax over-acceptance (we accept ∧ SQLite
/// syntax-rejects) and a coverage gap (we reject ∧ SQLite accepts) are both real
/// divergences the lane surfaces; binding/other residuals are skipped (the setup driver
/// simply did not provision the referenced object — never a parser fault).
fn generative_divergence(
    sql: &str,
    bare: &SqliteOracle,
    bare_probe: &SqliteConnection,
    provisioned: &SqliteOracle,
) -> Option<String> {
    let verdict = generative_verdict(sql, bare, bare_probe, provisioned);
    let mut quad = Quadrant::default();
    match quad.record(&verdict) {
        Cell::OverAcceptSyntax => {
            if SQLITE_GENERATIVE_DIVERGENCE_ALLOWLIST
                .iter()
                .any(|e| e.sql == sql)
            {
                None
            } else {
                Some(format!(
                    "OVER-ACCEPT (we accept, SQLite syntax-rejects): {sql:?} — a real \
                     validator-correctness divergence the flag-aware generator caught; triage \
                     it (tighten the `Sqlite` preset, or ledger the exact SQL in \
                     SQLITE_GENERATIVE_DIVERGENCE_ALLOWLIST with a ticket)"
                ))
            }
        }
        Cell::CoverageGap => Some(format!(
            "COVERAGE GAP (SQLite accepts, we reject): {sql:?} — the flag-aware generator emitted \
             a gated form the fitted `Sqlite` preset rejects; fix the probe if malformed, else \
             file a coverage-gap ticket"
        )),
        _ => None,
    }
}

/// The DETERMINISTIC replay gate over the committed misfeature seeds — the generative
/// lane's counterpart to `fuzz::pg_differential_raw_bytes_replays_committed_inputs`
/// (committed-inputs, never a flaky always-random gate). Every seed is differentialled
/// against the bundled engine behind the setup driver; a preset tightening or a fresh
/// divergence trips it with the exact SQL. This is the durable proof + regression guard.
#[test]
fn sqlite_feature_generative_differential_replays_committed_seeds() {
    let bare = SqliteOracle::new().expect("open in-memory sqlite");
    let bare_probe = SqliteConnection::open_in_memory().expect("bare probe connection");
    let provisioned =
        SqliteOracle::with_schema(FEATURE_SCHEMA_SETUP_SQL).expect("provision feature schema");

    let divergences: Vec<String> = SQLITE_MISFEATURE_SEEDS
        .iter()
        .filter_map(|&sql| generative_divergence(sql, &bare, &bare_probe, &provisioned))
        .collect();

    assert!(
        divergences.is_empty(),
        "flag-aware generative differential found {} un-ledgered divergence(s) over the \
         committed misfeature seeds:\n{}",
        divergences.len(),
        divergences.join("\n"),
    );
}

/// The DETERMINISTIC-SEED random exploration: draws flag-aware statements the SQLite
/// preset enables (self-selected off `FeatureSet::SQLITE` by each probe's `applies`),
/// and differentials each against the bundled engine. Seeded with a fixed RNG
/// ([`TestRng::deterministic`]) so it is reproducible — the same statements every run,
/// never a flaky gate — while still exercising combinations the committed seeds do not
/// enumerate. Any un-ledgered over-acceptance or coverage gap fails with the minimized
/// SQL, promoting it to a committed seed (or a ledger entry) alongside its triage.
#[test]
fn sqlite_feature_generative_differential_explores_flag_aware_surface() {
    use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
    use squonk::Dialect;

    let bare = SqliteOracle::new().expect("open in-memory sqlite");
    let bare_probe = SqliteConnection::open_in_memory().expect("bare probe connection");
    let provisioned =
        SqliteOracle::with_schema(FEATURE_SCHEMA_SETUP_SQL).expect("provision feature schema");

    let strategy = arb_feature_statement(Sqlite.features(), SQLITE_FEATURE_PROBES);
    let mut runner = TestRunner::new_with_rng(
        Config {
            cases: 1024,
            // The generated inputs are self-contained SQL strings; failures carry the SQL
            // directly, so persistence-file shrinking adds nothing here.
            failure_persistence: None,
            ..Config::default()
        },
        TestRng::deterministic_rng(RngAlgorithm::ChaCha),
    );

    runner
        .run(&strategy, |(family, sql)| {
            if let Some(detail) = generative_divergence(&sql, &bare, &bare_probe, &provisioned) {
                return Err(proptest::test_runner::TestCaseError::fail(format!(
                    "[{family}] {detail}"
                )));
            }
            Ok(())
        })
        .expect("flag-aware generative differential surfaced an un-ledgered SQLite divergence");
}

/// The generative ledger's staleness contract (the PG/corpus-ledger clone): every entry
/// names a real ticket and still diverges (we accept ∧ SQLite syntax-rejects), so a
/// fixed generative over-acceptance cannot stay silently allowlisted. Vacuous while the
/// ledger is empty, but keeps the machinery in place for the first real finding.
#[test]
fn sqlite_generative_divergence_allowlist_entries_name_tickets_and_still_diverge() {
    let bare_probe = SqliteConnection::open_in_memory().expect("bare probe connection");
    assert_entries_are_ticketed(SQLITE_GENERATIVE_DIVERGENCE_ALLOWLIST);
    assert_entries_still_diverge(SQLITE_GENERATIVE_DIVERGENCE_ALLOWLIST, |entry| {
        let ours = parse_with(entry.sql, Sqlite).is_ok();
        let sqlite = bare_probe.prepare(entry.sql).is_ok();
        let reason = bare_reject_reason(&bare_probe, entry.sql);
        ours && !sqlite && reason == RejectReason::Syntax
    });
}
