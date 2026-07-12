// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! BigQuery oracle-parity lane: acquisition decision + comparison-only cross-check.
//!
//! # Source-of-truth decision (oracle-parity-bigquery)
//!
//! | Candidate | Licence | Role |
//! |-----------|---------|------|
//! | **google/zetasql** | Apache-2.0 | **Preferred real oracle.** GoogleSQL reference parser/analyzer. Bring up as an **external-process** harness only (never link C++ into this crate). Blocked until a provisioned binary exists (`ZETASQL_ORACLE_BIN` / builder install). When present, BigQuery joins tier-1 style accept/reject differentials. |
//! | ZetaSQL parser tests | Apache-2.0 | Future vendorable accept corpus with provenance (same licence). |
//! | **sqlglot `read="bigquery"`** | MIT | **Final-ditch ParseOnly comparison** wired below via external Python. **Not authority** — multi-dialect parser gaps become false divergences; do not treat sqlglot accept/reject as GoogleSQL truth. |
//! | sqlparser-rs BigQuery | Apache-2.0 | Not wired; same multi-dialect caveat. |
//!
//! Licensing is absolute: Apache-2.0/MIT only; no GPL. sqlglot stays out-of-process
//! (`pip install sqlglot` is optional local/CI, not a Cargo dep).
//!
//! # What this lane enforces today
//!
//! 1. **Modelled-surface seeds** ([`BIGQUERY_FEATURE_SEEDS`](crate::properties::BIGQUERY_FEATURE_SEEDS))
//!    parse under our [`BigQuery`](squonk::dialect::BigQuery) preset — the surface
//!    the preset actually enables (see `crates/squonk-ast/src/dialect/bigquery.rs`).
//! 2. **Flag-aware probes** self-select: every probe applies to BigQuery; ANSI enables
//!    strictly fewer (unit tests in `dialect_features`).
//! 3. **sqlglot comparison** (when Python+sqlglot are available): accept/reject agreement
//!    **only on the modelled-surface corpus**. Divergences outside that surface are not
//!    coverage gaps — e.g. `UNNEST([1,2])` is real GoogleSQL but our preset keeps
//!    `array_constructor` / `collection_literals` **off** by design (conservative, no
//!    ZetaSQL oracle yet). That reject is intentional; do not "fix" the parser to match
//!    sqlglot without an engine-backed ticket.
//!
//! # What this lane does *not* claim
//!
//! - Full GoogleSQL parity.
//! - Binder/semantic correctness (sqlglot is ParseOnly; ZetaSQL analyzer not wired).
//! - That sqlglot is the source of truth.
//!
//! # Feature gate
//!
//! Behind `oracle-bigquery` so default builds need no Python/sqlglot.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::oracle::{AcceptRejectOracle, OracleSemantics, OracleUnavailable, OracleVerdict};

/// Env var naming the Python interpreter (default: `python3` on `PATH`).
pub const BIGQUERY_ORACLE_PYTHON_ENV: &str = "BIGQUERY_ORACLE_PYTHON";

/// Default interpreter when the env var is unset.
pub const DEFAULT_BIGQUERY_ORACLE_PYTHON: &str = "python3";

/// External-process BigQuery ParseOnly **comparison** oracle (`sqlglot` `read='bigquery'`).
///
/// Final-ditch until ZetaSQL is provisioned — see module docs. Never treat as engine truth.
pub struct BigQuerySqlglotOracle {
    python: String,
}

impl BigQuerySqlglotOracle {
    /// Probe liveness: `import sqlglot` + parse `SELECT 1`.
    pub fn new() -> Result<Self, OracleUnavailable> {
        let python = std::env::var(BIGQUERY_ORACLE_PYTHON_ENV)
            .unwrap_or_else(|_| DEFAULT_BIGQUERY_ORACLE_PYTHON.to_owned());
        Self::with_python(python)
    }

    /// Point at an explicit Python binary (tests; no env mutation).
    pub fn with_python(python: impl Into<String>) -> Result<Self, OracleUnavailable> {
        let python = python.into();
        let probe = r#"
import sqlglot
sqlglot.parse_one("SELECT 1", read="bigquery")
print("ok")
"#;
        let out = Command::new(&python)
            .args(["-c", probe])
            .output()
            .map_err(|err| {
                OracleUnavailable(format!(
                    "bigquery sqlglot oracle: python {python:?} could not be spawned: {err}"
                ))
            })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(OracleUnavailable(format!(
                "bigquery sqlglot oracle: python {python:?} failed sqlglot liveness \
                 (install MIT-licensed sqlglot for the comparison lane): {stderr}"
            )));
        }
        Ok(Self { python })
    }

    fn parse_one(&self, sql: &str) -> Result<OracleVerdict, OracleUnavailable> {
        // SQL on stdin — no shell quoting. Protocol: exit 0 accept, 2 reject, else unavailable.
        let helper = r#"
import sys
import sqlglot
from sqlglot.errors import ParseError, TokenError

sql = sys.stdin.read()
try:
    sqlglot.parse_one(sql, read="bigquery")
except (ParseError, TokenError):
    sys.exit(2)
except Exception as exc:
    sys.stderr.write(f"sqlglot unexpected: {exc}\n")
    sys.exit(3)
sys.exit(0)
"#;
        let mut child = Command::new(&self.python)
            .args(["-c", helper])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| {
                OracleUnavailable(format!(
                    "bigquery sqlglot oracle spawn failed ({:?}): {err}",
                    self.python
                ))
            })?;
        {
            let mut stdin = child.stdin.take().ok_or_else(|| {
                OracleUnavailable("bigquery sqlglot oracle child has no stdin".to_owned())
            })?;
            stdin.write_all(sql.as_bytes()).map_err(|err| {
                OracleUnavailable(format!("bigquery sqlglot oracle stdin write failed: {err}"))
            })?;
        }
        let out = child.wait_with_output().map_err(|err| {
            OracleUnavailable(format!("bigquery sqlglot oracle wait failed: {err}"))
        })?;
        match out.status.code() {
            Some(0) => Ok(OracleVerdict::Accept),
            Some(2) => Ok(OracleVerdict::Reject),
            code => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                Err(OracleUnavailable(format!(
                    "bigquery sqlglot oracle unexpected exit {code:?}: {stderr}"
                )))
            }
        }
    }
}

impl AcceptRejectOracle for BigQuerySqlglotOracle {
    fn name(&self) -> &'static str {
        // Name makes the comparison-only status obvious in divergence strings.
        "bigquery-sqlglot-comparison"
    }

    fn semantics(&self) -> OracleSemantics {
        OracleSemantics::ParseOnly
    }

    fn verdict(&self, sql: &str) -> Result<OracleVerdict, OracleUnavailable> {
        self.parse_one(sql)
    }
}

/// Modelled-surface accept corpus: forms our BigQuery preset enables **and** that
/// sqlglot BigQuery also accepts (measured). Not the full GoogleSQL surface.
///
/// Deliberately uses `UNNEST(arr)` (column ref), not `UNNEST([1,2])` — list/array
/// literals stay off under this preset (`array_constructor` / `collection_literals`
/// inherit ANSI `false`; see dialect module docs and the unit test below).
pub const MODELLED_SURFACE_AGREE_ACCEPT: &[&str] = &[
    "SELECT 1",
    "SELECT * FROM UNNEST(arr) AS x",
    "SELECT * FROM UNNEST(arr) WITH OFFSET",
    "SELECT STRUCT(1 AS a, 2 AS b)",
    "SELECT CAST(x AS ARRAY<INT64>)",
    "SELECT CAST(x AS STRUCT<a INT64>)",
    "SELECT * FROM t FOR SYSTEM_TIME AS OF TIMESTAMP '2020-01-01 00:00:00'",
    "SELECT \"hello\"",
    "SELECT `a` FROM t",
];

/// Garbage both our parser and sqlglot should reject.
pub const AGREE_REJECT: &[&str] = &["SELCT 1", "SELECT * FORM t"];

/// Real GoogleSQL that our preset **intentionally** rejects today (conservative gates).
/// Documented so a future ZetaSQL lane can promote these without rediscovery.
pub const INTENTIONAL_OUR_REJECTS: &[&str] = &[
    // Array/list literal constructor is off on the BigQuery preset (ANSI inherit).
    // GoogleSQL accepts `UNNEST([1, 2])`; enabling it is a separate surface ticket
    // (collection_literals / array_constructor), not a silent parse widen here.
    "SELECT * FROM UNNEST([1, 2]) AS x",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::{AcceptRejectOracle, OracleUnavailable, OracleVerdict};
    use crate::properties::{
        BIGQUERY_FEATURE_PROBES, BIGQUERY_FEATURE_SEEDS, arb_feature_statement,
    };
    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
    use squonk::Dialect;
    use squonk::dialect::{Ansi, BigQuery};
    use squonk::parse_with;

    macro_rules! oracle_or_skip {
        ($name:ident = $ctor:expr) => {
            let $name = match $ctor {
                Ok(o) => o,
                Err(OracleUnavailable(reason)) => {
                    eprintln!("skipping bigquery sqlglot comparison tests: {reason}");
                    return;
                }
            };
        };
    }

    #[test]
    fn liveness_and_basic_verdicts() {
        oracle_or_skip!(oracle = BigQuerySqlglotOracle::new());
        assert_eq!(oracle.name(), "bigquery-sqlglot-comparison");
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
    fn modelled_surface_seeds_parse_under_our_bigquery_preset() {
        // Ground truth for *our* surface: every committed generative seed must parse.
        // This does not claim GoogleSQL completeness.
        for sql in BIGQUERY_FEATURE_SEEDS {
            assert!(
                parse_with(sql, BigQuery).is_ok(),
                "modelled-surface seed must parse under BigQuery: {sql:?}"
            );
        }
        for sql in MODELLED_SURFACE_AGREE_ACCEPT {
            assert!(
                parse_with(sql, BigQuery).is_ok(),
                "agree-accept corpus must parse under BigQuery: {sql:?}"
            );
        }
    }

    #[test]
    fn intentional_over_rejects_stay_rejected_under_our_preset() {
        // Pin the conservative rejects so a silent widen (e.g. turning on list
        // literals to chase sqlglot) fails loudly and forces an explicit ticket.
        for sql in INTENTIONAL_OUR_REJECTS {
            assert!(
                parse_with(sql, BigQuery).is_err(),
                "expected intentional reject under conservative BigQuery preset: {sql:?}"
            );
        }
        // ANSI also rejects (no unnest / no list primary).
        for sql in INTENTIONAL_OUR_REJECTS {
            assert!(parse_with(sql, Ansi).is_err(), "ANSI must reject {sql:?}");
        }
    }

    #[test]
    fn sqlglot_comparison_agrees_on_modelled_surface_accepts() {
        oracle_or_skip!(oracle = BigQuerySqlglotOracle::new());
        for sql in MODELLED_SURFACE_AGREE_ACCEPT {
            let v = oracle.verdict(sql).expect("oracle available");
            assert!(
                matches!(v, OracleVerdict::Accept),
                "sqlglot comparison must accept modelled-surface form {sql:?} (got {v:?})"
            );
        }
    }

    #[test]
    fn sqlglot_comparison_agrees_on_garbage_rejects() {
        oracle_or_skip!(oracle = BigQuerySqlglotOracle::new());
        for sql in AGREE_REJECT {
            assert!(
                parse_with(sql, BigQuery).is_err(),
                "our preset must reject garbage {sql:?}"
            );
            let v = oracle.verdict(sql).expect("oracle available");
            assert!(
                matches!(v, OracleVerdict::Reject),
                "sqlglot comparison must reject garbage {sql:?} (got {v:?})"
            );
        }
    }

    #[test]
    fn sqlglot_accepts_intentional_our_rejects_proving_comparison_is_not_authority() {
        // If sqlglot ever starts rejecting these, the comparison residual disappears —
        // but today it accepts real GoogleSQL we deliberately do not model. This test
        // documents that gap so nobody "fixes" it by trusting sqlglot over the preset.
        oracle_or_skip!(oracle = BigQuerySqlglotOracle::new());
        for sql in INTENTIONAL_OUR_REJECTS {
            let v = oracle.verdict(sql).expect("oracle available");
            assert!(
                matches!(v, OracleVerdict::Accept),
                "expected sqlglot to accept GoogleSQL form we intentionally reject: {sql:?} (got {v:?})"
            );
            assert!(
                parse_with(sql, BigQuery).is_err(),
                "our preset must still reject {sql:?}"
            );
        }
    }

    fn modelled_surface_comparison_divergence(
        oracle: &BigQuerySqlglotOracle,
        sql: &str,
    ) -> Option<String> {
        let ours = parse_with(sql, BigQuery).is_ok();
        let theirs = match oracle.verdict(sql) {
            Ok(OracleVerdict::Accept) => true,
            Ok(OracleVerdict::Reject) => false,
            Err(_) => return None,
        };
        if ours == theirs {
            return None;
        }
        // Only modelled-surface SQL should reach here; label as comparison residual.
        Some(if ours {
            format!("comparison residual: we accept, sqlglot rejects (not ZetaSQL truth): {sql:?}")
        } else {
            format!(
                "comparison residual: sqlglot accepts, we reject (check intentional gates): {sql:?}"
            )
        })
    }

    #[test]
    fn generative_seeds_agree_with_sqlglot_comparison_on_modelled_surface() {
        oracle_or_skip!(oracle = BigQuerySqlglotOracle::new());
        let divergences: Vec<String> = BIGQUERY_FEATURE_SEEDS
            .iter()
            .filter_map(|&sql| modelled_surface_comparison_divergence(&oracle, sql))
            .collect();
        assert!(
            divergences.is_empty(),
            "modelled-surface seed vs sqlglot comparison diverged (investigate gate vs seed):\n  {}",
            divergences.join("\n  "),
        );
    }

    #[test]
    fn generative_exploration_stays_on_modelled_surface_and_agrees_with_sqlglot() {
        // Probes are fixed `Just(...)` strings over modelled flags — exploration
        // re-samples that closed set (not free SQL). Agreement is a cross-check only.
        oracle_or_skip!(oracle = BigQuerySqlglotOracle::new());
        let mut runner = TestRunner::new_with_rng(
            Config {
                cases: 128,
                ..Config::default()
            },
            TestRng::from_seed(RngAlgorithm::ChaCha, &[0xB0; 32]),
        );
        let strategy = arb_feature_statement(BigQuery.features(), BIGQUERY_FEATURE_PROBES);
        for _ in 0..128 {
            let tree = strategy.new_tree(&mut runner).expect("strategy ok");
            let (family, sql) = tree.current();
            assert!(
                parse_with(&sql, BigQuery).is_ok(),
                "probe family {family:?} emitted SQL our BigQuery preset rejects: {sql:?}"
            );
            if let Some(detail) = modelled_surface_comparison_divergence(&oracle, &sql) {
                panic!("generative comparison: {detail}");
            }
        }
    }

    #[test]
    fn angle_bracket_types_gate_ansi_reject() {
        let sql = "SELECT CAST(x AS ARRAY<INT64>)";
        assert!(parse_with(sql, BigQuery).is_ok());
        assert!(parse_with(sql, Ansi).is_err());
    }
}
