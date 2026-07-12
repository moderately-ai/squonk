// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! ClickHouse accept/reject differential oracle — external `clickhouse local`
//! process, ParseOnly via `EXPLAIN AST`.
//!
//! # Why external process
//!
//! ClickHouse is Apache-2.0 (no GPL boundary like MySQL), but the cheapest correct
//! shape is still an external binary: no linked crate, no Arrow/network graph, and
//! the same "Thomas provisions the binary" pattern as other external oracles.
//! `clickhouse local` is a full engine in one binary — no daemon.
//!
//! # Verdict contract (ClickHouse 25.5 measured)
//!
//! - **exit 0** after `EXPLAIN AST <sql>` → [`OracleVerdict::Accept`]
//! - **non-zero exit with a `DB::Exception` / `Code: N` diagnostic on stderr** →
//!   [`OracleVerdict::Reject`] (usually `Code: 62` `SYNTAX_ERROR`, not always)
//! - **spawn failed / binary missing / non-zero without diagnostic** →
//!   [`OracleUnavailable`] (infrastructure skip)
//!
//! # ParseOnly, with one measured semantic leak — the SETTINGS clause
//!
//! [`OracleSemantics::ParseOnly`]: `EXPLAIN AST` is syntax-only over the surface that
//! matters here — an unresolved table/column, an unknown function, an unknown *type*, and
//! an unknown `FORMAT` name all still accept (measured on 25.5). Our parser is likewise
//! parse-only, so the differential runs over the full generated/curated surface with **no**
//! schema-provisioning setup driver (unlike the m3 MySQL `PrepareBind` oracle).
//!
//! The one exception: the `SETTINGS name = value` clause is validated against ClickHouse's
//! settings registry even under `EXPLAIN AST` — an unknown setting name is `Code: 115`
//! (`UNKNOWN_SETTING`) and a value of the wrong type is `Code: 27`, both coded rejects. So
//! a `SETTINGS` fragment over an arbitrary identifier would *falsely* diverge (we accept any
//! `ident = expr`; the engine rejects the unknown name). The generative SETTINGS probe and
//! the curated corpus therefore keep to registry-real, integer-valued settings — the
//! ClickHouse analogue of provisioning a `PrepareBind` oracle's schema. This is the honest
//! boundary of the ParseOnly claim, recorded so a future SETTINGS probe cannot reintroduce
//! the false divergence.
//!
//! # Feature gate
//!
//! Behind `oracle-clickhouse` so the default (non-oracle) build stays free of a ClickHouse
//! PATH requirement. It joins the combined preflight/nightly oracle lane
//! (`--features oracle-engines,oracle-mysql,oracle-clickhouse`) — the standing guard the
//! tier-promotion programme wired — and skips cleanly wherever the binary is absent.

use std::process::Command;

use crate::oracle::{AcceptRejectOracle, OracleSemantics, OracleUnavailable, OracleVerdict};

/// Env var naming the `clickhouse` binary (default: `clickhouse` on `PATH`).
pub const CLICKHOUSE_ORACLE_BIN_ENV: &str = "CLICKHOUSE_ORACLE_BIN";

/// Default binary name when the env var is unset.
pub const DEFAULT_CLICKHOUSE_ORACLE_BIN: &str = "clickhouse";

/// External-process ClickHouse ParseOnly oracle (`EXPLAIN AST`).
pub struct ClickHouseOracle {
    bin: String,
}

impl ClickHouseOracle {
    /// Probe liveness with `SELECT 1`; skip if the binary is missing or broken.
    pub fn new() -> Result<Self, OracleUnavailable> {
        let bin = std::env::var(CLICKHOUSE_ORACLE_BIN_ENV)
            .unwrap_or_else(|_| DEFAULT_CLICKHOUSE_ORACLE_BIN.to_owned());
        Self::with_binary(bin)
    }

    /// Point at an explicit binary path (tests; no env mutation).
    pub fn with_binary(bin: impl Into<String>) -> Result<Self, OracleUnavailable> {
        let bin = bin.into();
        let out = Command::new(&bin)
            .args(["local", "--query", "SELECT 1"])
            .output()
            .map_err(|err| {
                OracleUnavailable(format!(
                    "clickhouse binary {bin:?} could not be spawned: {err}"
                ))
            })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(OracleUnavailable(format!(
                "clickhouse binary {bin:?} rejected the `SELECT 1` liveness probe: {stderr}"
            )));
        }
        Ok(Self { bin })
    }

    /// The `clickhouse local --version` string, for the "oracle actually ran" nightly
    /// evidence the curated-corpus parity test emits (`oracle-nightly.yml` greps it). The
    /// binary was liveness-checked at construction, so a probe failure here is unexpected;
    /// it degrades to an empty string rather than panicking the evidence line.
    pub fn version(&self) -> String {
        Command::new(&self.bin)
            .arg("--version")
            .output()
            .ok()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
            .unwrap_or_default()
    }

    fn explain_ast(&self, sql: &str) -> Result<OracleVerdict, OracleUnavailable> {
        // Single-statement only — multi-statement EXPLAIN is not the differential unit.
        let query = format!("EXPLAIN AST {sql}");
        let out = Command::new(&self.bin)
            .args(["local", "--query", &query])
            .output()
            .map_err(|err| {
                OracleUnavailable(format!(
                    "clickhouse spawn failed mid-sweep ({:?}): {err}",
                    self.bin
                ))
            })?;
        if out.status.success() {
            return Ok(OracleVerdict::Accept);
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        // A coded ClickHouse exception is a real reject; anything else is infrastructure.
        if stderr.contains("DB::Exception") || stderr.contains("Code:") {
            return Ok(OracleVerdict::Reject);
        }
        Err(OracleUnavailable(format!(
            "clickhouse non-zero exit without diagnostic (status {:?}): {stderr}",
            out.status.code()
        )))
    }
}

impl AcceptRejectOracle for ClickHouseOracle {
    fn name(&self) -> &'static str {
        "clickhouse"
    }

    fn semantics(&self) -> OracleSemantics {
        OracleSemantics::ParseOnly
    }

    fn verdict(&self, sql: &str) -> Result<OracleVerdict, OracleUnavailable> {
        self.explain_ast(sql)
    }
}

/// Curated accept corpus (must parse under our ClickHouse preset and the engine).
pub const ACCEPT_CORPUS: &[&str] = &[
    "SELECT 1",
    "SELECT a FROM t",
    "SELECT * FROM t WHERE a = 1",
    "SELECT a, b FROM t ORDER BY a",
    "SELECT count() FROM t GROUP BY a",
    "INSERT INTO t VALUES (1)",
    "CREATE TABLE t (a Int32)",
];

/// Curated reject corpus (must reject under our ClickHouse preset and the engine).
pub const REJECT_CORPUS: &[&str] = &["SELCT 1", "CREATE TABEL t (a Int32)", "SELECT * FORM t"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::{
        AcceptRejectOracle, OracleUnavailable, OracleVerdict, accept_reject_divergence,
    };
    use crate::properties::{
        CLICKHOUSE_FEATURE_PROBES, CLICKHOUSE_FEATURE_SEEDS, arb_feature_statement,
    };
    use squonk::Dialect;
    use squonk::dialect::ClickHouse;
    use squonk::parse_with;

    macro_rules! oracle_or_skip {
        ($name:ident = $ctor:expr) => {
            let $name = match $ctor {
                Ok(o) => o,
                Err(OracleUnavailable(reason)) => {
                    eprintln!("skipping clickhouse oracle tests: {reason}");
                    return;
                }
            };
        };
    }

    #[test]
    fn liveness_and_basic_verdicts() {
        oracle_or_skip!(oracle = ClickHouseOracle::new());
        assert_eq!(oracle.name(), "clickhouse");
        assert_eq!(oracle.semantics(), OracleSemantics::ParseOnly);
        assert!(matches!(
            oracle.verdict("SELECT 1").unwrap(),
            OracleVerdict::Accept
        ));
        assert!(matches!(
            oracle.verdict("SELCT 1").unwrap(),
            OracleVerdict::Reject
        ));
    }

    #[test]
    fn absent_binary_is_unavailable() {
        let result = ClickHouseOracle::with_binary("clickhouse_binary_that_does_not_exist_zzq");
        assert!(result.is_err());
    }

    #[test]
    fn accept_corpus_parses_under_clickhouse_preset() {
        for sql in ACCEPT_CORPUS {
            assert!(
                parse_with(sql, ClickHouse).is_ok(),
                "ClickHouse preset should parse {sql:?}"
            );
        }
    }

    #[test]
    fn reject_corpus_rejects_under_clickhouse_preset() {
        for sql in REJECT_CORPUS {
            assert!(
                parse_with(sql, ClickHouse).is_err(),
                "ClickHouse preset should reject {sql:?}"
            );
        }
    }

    #[test]
    fn clickhouse_accept_reject_parity_over_curated_corpus() {
        oracle_or_skip!(oracle = ClickHouseOracle::new());
        // Machine-readable "oracle actually ran" evidence the nightly workflow greps for
        // (oracle-nightly.yml); emitted only on the ran path, so its absence trips the guard.
        // Mirrors the m2 (`oracle-ran: sqlite`/`duckdb`) and m3 (`oracle-ran: mysql`) markers.
        eprintln!("oracle-ran: clickhouse ({})", oracle.version());
        for sql in ACCEPT_CORPUS {
            assert!(
                accept_reject_divergence(sql, ClickHouse, &oracle).is_none(),
                "accept corpus diverges on {sql:?}"
            );
        }
        for sql in REJECT_CORPUS {
            assert!(
                accept_reject_divergence(sql, ClickHouse, &oracle).is_none(),
                "reject corpus diverges on {sql:?}"
            );
        }
    }

    #[test]
    fn corpus_is_single_statement() {
        for sql in ACCEPT_CORPUS
            .iter()
            .chain(REJECT_CORPUS)
            .chain(CLICKHOUSE_FEATURE_SEEDS)
        {
            assert!(
                !sql.contains(';'),
                "corpus must be single-statement: {sql:?}"
            );
        }
    }

    // =================================================================================
    // Flag-aware generative differential (clickhouse-tier-promotion-generative-nightly)
    // =================================================================================

    /// A triaged ClickHouse generative divergence knowingly tolerated: a fragment where
    /// `clickhouse local` and our [`ClickHouse`] preset disagree. Every entry must name an
    /// a non-empty provenance label; the ticketed-entries test asserts each still diverges,
    /// so a fixed gap cannot stay silently allowlisted (mirrors `m3::M3_DIVERGENCE_ALLOWLIST`).
    #[derive(Clone, Copy, Debug)]
    struct ClickHouseDivergenceAllowlistEntry {
        sql: &'static str,
        ticket: &'static str,
        reason: &'static str,
    }

    /// Current ClickHouse generative accept/reject divergences allowed by the oracle. Empty:
    /// the committed seeds are engine-verified both-accept and the fitted preset is
    /// deliberately conservative, so no divergence is currently tolerated. The machinery
    /// stays in place for the first real entry (ADR-0015: a fix forces removal, never a
    /// re-pin).
    const CLICKHOUSE_GENERATIVE_DIVERGENCE_ALLOWLIST: &[ClickHouseDivergenceAllowlistEntry] = &[];

    /// The accept/reject divergence between `clickhouse local` and our `ClickHouse` preset on
    /// `sql`, with the generative allowlist applied. `None` when the two agree, the entry is
    /// allowlisted, or the oracle hiccups mid-sweep (a per-fragment spawn failure is a skip,
    /// never a false divergence — the start-of-sweep liveness check already gated the binary).
    fn clickhouse_generative_divergence(oracle: &ClickHouseOracle, sql: &str) -> Option<String> {
        let ours = parse_with(sql, ClickHouse).is_ok();
        let theirs = match oracle.verdict(sql) {
            Ok(verdict) => verdict.accepts(),
            Err(OracleUnavailable(_)) => return None,
        };
        if ours == theirs {
            return None;
        }
        if CLICKHOUSE_GENERATIVE_DIVERGENCE_ALLOWLIST
            .iter()
            .any(|entry| entry.sql == sql)
        {
            return None;
        }
        Some(if ours && !theirs {
            format!("over-acceptance: we accept, clickhouse rejects: {sql:?}")
        } else {
            format!("coverage gap: clickhouse accepts, we reject: {sql:?}")
        })
    }

    #[test]
    fn clickhouse_feature_generative_differential_replays_committed_seeds() {
        oracle_or_skip!(oracle = ClickHouseOracle::new());
        let divergences: Vec<String> = CLICKHOUSE_FEATURE_SEEDS
            .iter()
            .filter_map(|&sql| clickhouse_generative_divergence(&oracle, sql))
            .collect();
        assert!(
            divergences.is_empty(),
            "ClickHouse generative differential found {} un-ledgered divergence(s):\n  {}",
            divergences.len(),
            divergences.join("\n  "),
        );
    }

    #[test]
    fn clickhouse_feature_generative_differential_explores_flag_aware_surface() {
        use proptest::strategy::{Strategy, ValueTree};
        use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

        oracle_or_skip!(oracle = ClickHouseOracle::new());
        // A modest case count on purpose: unlike the in-process (SQLite/DuckDB) and wire
        // (MySQL) oracles, each `verdict` spawns a fresh `clickhouse local` process, so the
        // sweep is process-bound. 128 fixed-RNG draws over the flag-selected probes cover the
        // families' combinatorics without turning every preflight into a spawn storm.
        let mut runner = TestRunner::new_with_rng(
            Config {
                cases: 128,
                ..Config::default()
            },
            TestRng::from_seed(RngAlgorithm::ChaCha, &[0xCA; 32]),
        );
        let strategy = arb_feature_statement(ClickHouse.features(), CLICKHOUSE_FEATURE_PROBES);
        for _ in 0..128 {
            let tree = strategy.new_tree(&mut runner).expect("strategy ok");
            let (_family, sql) = tree.current();
            if let Some(detail) = clickhouse_generative_divergence(&oracle, &sql) {
                panic!("ClickHouse generative differential: {detail}");
            }
        }
    }

    #[test]
    fn clickhouse_generative_allowlist_entries_name_tickets_and_still_diverge() {
        // Mirrors the m3 allowlist test: every allowlisted divergence must name a real ticket
        // and still actually diverge. Vacuous while empty, but keeps the machinery honest.
        oracle_or_skip!(oracle = ClickHouseOracle::new());
        for entry in CLICKHOUSE_GENERATIVE_DIVERGENCE_ALLOWLIST {
            assert!(
                !entry.ticket.trim().is_empty(),
                "allowlisted divergence needs a provenance label: {} ({})",
                entry.ticket,
                entry.reason,
            );
            assert!(
                clickhouse_generative_divergence(&oracle, entry.sql).is_some(),
                "allowlisted case no longer diverges — SWEEP this entry (a fix forces removal, \
                 never a re-pin): {:?}",
                entry.sql,
            );
        }
    }

    // =================================================================================
    // Statement-head coverage baseline (clickhouse-tier-promotion-generative-nightly)
    // =================================================================================
    //
    // The measured coverage baseline the promotion decision rests on: how much of
    // ClickHouse's top-level statement surface the deliberately-conservative, ANSI-derived
    // `ClickHouse` preset reaches. Two axes are tracked separately (the m3 family-inventory
    // pattern):
    // - Engine recognition: every head must parse under `clickhouse local` (EXPLAIN AST),
    //   verified by the oracle-gated cross-check below — so the denominator is real ClickHouse
    //   grammar, not invented SQL.
    // - squonk reach: `parse_with(probe, ClickHouse).is_ok()` per head, partitioned into
    //   the covered set and the measured, pinned uncovered residual (the honest promotion gap).
    //
    // This is a deliberately-bounded baseline document/pin, NOT an exhaustive grammar
    // inventory — the full bar-A ClickHouse surface is its own umbrella. It exists so the tier
    // decision cites a measured number, and so a coverage regression/gain drifts the pin.

    /// The bounded ClickHouse statement-head inventory: `(head, minimal probe)`. Each probe's
    /// top-level head is a distinct ClickHouse statement family, engine-recognized (the
    /// oracle cross-check asserts it). Minimal on purpose — one representative per head.
    const CLICKHOUSE_STATEMENT_HEADS: &[(&str, &str)] = &[
        ("select", "SELECT 1"),
        ("select-from", "SELECT a FROM t"),
        ("select-union", "SELECT 1 UNION ALL SELECT 2"),
        ("select-with-cte", "WITH x AS (SELECT 1) SELECT * FROM x"),
        ("insert-values", "INSERT INTO t VALUES (1)"),
        ("insert-select", "INSERT INTO t SELECT 1"),
        ("create-table", "CREATE TABLE t (a Int32)"),
        ("create-database", "CREATE DATABASE db"),
        ("create-view", "CREATE VIEW v AS SELECT 1"),
        (
            "create-materialized-view",
            "CREATE MATERIALIZED VIEW mv AS SELECT 1",
        ),
        ("alter-table", "ALTER TABLE t ADD COLUMN b Int32"),
        ("drop-table", "DROP TABLE t"),
        ("drop-database", "DROP DATABASE db"),
        ("rename-table", "RENAME TABLE a TO b"),
        ("truncate-table", "TRUNCATE TABLE t"),
        ("optimize-table", "OPTIMIZE TABLE t"),
        ("system", "SYSTEM RELOAD DICTIONARIES"),
        ("set", "SET max_threads = 8"),
        ("use", "USE db"),
        ("show-tables", "SHOW TABLES"),
        ("describe-table", "DESCRIBE TABLE t"),
        ("kill-query", "KILL QUERY WHERE 1"),
        ("check-table", "CHECK TABLE t"),
        ("attach-table", "ATTACH TABLE t"),
        ("detach-table", "DETACH TABLE t"),
        ("exists-table", "EXISTS TABLE t"),
    ];

    /// The measured count of statement heads the fitted `ClickHouse` preset parses. A pin: a
    /// coverage gain (a head the preset newly parses) or a regression drifts it, forcing a
    /// reviewed re-baseline. Re-measured from a fresh run, never adjusted by arithmetic.
    const CLICKHOUSE_COVERED_STATEMENT_HEADS: usize = 15;

    /// The statement heads the `ClickHouse` preset does NOT parse — the measured, pinned
    /// promotion residual (11 of 26; the preset reaches 15/26 = 57.7%). Engine-recognized
    /// (the cross-check proves all are) but not yet implemented for the ClickHouse preset;
    /// each is a candidate follow-up under the bar-A umbrella. In inventory order, re-measured
    /// from a fresh run (never adjusted by arithmetic).
    const CLICKHOUSE_UNCOVERED_STATEMENT_HEADS: &[&str] = &[
        "drop-database",
        "rename-table",
        "optimize-table",
        "system",
        "use",
        "describe-table",
        "kill-query",
        "check-table",
        "attach-table",
        "detach-table",
        "exists-table",
    ];

    #[test]
    fn clickhouse_statement_head_inventory_coverage_is_pinned() {
        use std::collections::BTreeSet;

        let names: BTreeSet<&str> = CLICKHOUSE_STATEMENT_HEADS
            .iter()
            .map(|(head, _)| *head)
            .collect();
        assert_eq!(
            names.len(),
            CLICKHOUSE_STATEMENT_HEADS.len(),
            "duplicate statement head in the inventory (each head is probed exactly once)",
        );

        let uncovered: Vec<&str> = CLICKHOUSE_STATEMENT_HEADS
            .iter()
            .filter(|(_, sql)| parse_with(sql, ClickHouse).is_err())
            .map(|(head, _)| *head)
            .collect();
        let covered = CLICKHOUSE_STATEMENT_HEADS.len() - uncovered.len();
        eprintln!(
            "squonk ClickHouse preset covers {covered}/{} statement heads ({:.1}%)",
            CLICKHOUSE_STATEMENT_HEADS.len(),
            100.0 * covered as f64 / CLICKHOUSE_STATEMENT_HEADS.len() as f64,
        );
        eprintln!("  UNCOVERED (measured, pinned): {uncovered:?}");
        assert_eq!(
            covered, CLICKHOUSE_COVERED_STATEMENT_HEADS,
            "ClickHouse statement-head coverage count drifted; review before re-baselining",
        );
        assert_eq!(
            uncovered, CLICKHOUSE_UNCOVERED_STATEMENT_HEADS,
            "ClickHouse statement-head coverage set drifted; review before re-baselining",
        );
    }

    #[test]
    fn clickhouse_statement_head_inventory_has_live_oracle_probes() {
        oracle_or_skip!(oracle = ClickHouseOracle::new());
        eprintln!("oracle-ran: clickhouse ({})", oracle.version());
        let mut syntax_failures: Vec<&str> = Vec::new();
        for (head, sql) in CLICKHOUSE_STATEMENT_HEADS {
            match oracle.verdict(sql) {
                Ok(OracleVerdict::Accept) => {}
                // The engine must recognize every authored head; a reject means the probe SQL
                // is not valid ClickHouse (an inventory bug), not a coverage fact. A spawn
                // hiccup is a clean skip.
                Ok(OracleVerdict::Reject) => syntax_failures.push(head),
                Err(OracleUnavailable(_)) => return,
            }
        }
        assert!(
            syntax_failures.is_empty(),
            "authored statement-head probe(s) rejected by clickhouse local — fix the inventory \
             SQL so every head is grammar-valid: {syntax_failures:?}",
        );
    }
}
