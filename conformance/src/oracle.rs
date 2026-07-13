// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Pluggable external-engine oracle seam (`prod-conformance-oracle-adapter`).
//!
//! The differential method (ADR-0015) checks our parser against a real engine's
//! accept/reject verdict. [`AcceptRejectOracle`] is the seam that lets each dialect
//! milestone supply its own ground-truth engine without re-wiring the harness:
//!
//! | Milestone | Oracle | [`OracleSemantics`] |
//! |---|---|---|
//! | M1 | libpg_query ([`pg::PgQueryOracle`](crate::pg::PgQueryOracle)) | [`ParseOnly`](OracleSemantics::ParseOnly) |
//! | M2 | `rusqlite` / `duckdb` `prepare()` | [`PrepareBind`](OracleSemantics::PrepareBind) |
//! | M3 | external MySQL server `PREPARE` via the `mysql` crate (`m3::MySqlOracle`) | [`PrepareBind`](OracleSemantics::PrepareBind) |
//! | M4 | SQL Server ScriptDOM / `SET PARSEONLY` | [`ParseOnly`](OracleSemantics::ParseOnly) |
//!
//! # The acceptance-semantics contract — read before implementing a new oracle
//!
//! "Accept" is **not** uniform across engines, and getting this wrong makes a new
//! oracle report divergences that are not parser bugs:
//!
//! - [`OracleSemantics::ParseOnly`] — syntax only; an unresolved object name still
//!   accepts (libpg_query, SQL Server `SET PARSEONLY`/ScriptDOM). Our parser is also
//!   parse-only, so a `ParseOnly` oracle can run over the **full** generated/corpus
//!   surface.
//! - [`OracleSemantics::PrepareBind`] — parse **and** name resolution against the
//!   session schema; an unknown table/column rejects (`rusqlite`/`duckdb`
//!   `prepare()`, server `PREPARE`). Our parser does not bind, so comparing a
//!   `PrepareBind` oracle over schema-dependent SQL (`SELECT * FROM t`) yields a
//!   **false** divergence — we accept, the engine rejects "no such table". A
//!   `PrepareBind` oracle MUST therefore either (a) run only over schema-independent
//!   statements (`SELECT 1`, `VALUES (1)`, …), or (b) provision the referenced schema
//!   first (the "setup driver", ZetaSQL's model). [`accept_reject_divergence`] does
//!   **not** enforce this — it is the oracle/harness author's responsibility, flagged
//!   by [`AcceptRejectOracle::semantics`].
//!
//! No oracle executes statements, mutates state, or performs a network round trip
//! inside the default `cargo test` path; container/server oracles stay opt-in
//! (ADR-0017), and an unreachable engine surfaces as [`OracleUnavailable`] (a skip),
//! never as a divergence.
//!
//! # Structural parity — the second oracle kind
//!
//! Accept/reject is the universal seam — every engine can answer it. Structural parity
//! needs an engine-specific parse-tree dump mapped to the neutral
//! [`QueryShape`](crate::shape) ([`pg`](crate::pg) maps the PostgreSQL protobuf). It was
//! left un-abstracted until a **second** structural source existed; DuckDB's
//! `json_serialize_sql` (`duckdb_structural`) is that source,
//! so [`StructuralOracle`] now abstracts `engine-tree -> `[`StatementShape`](crate::shape),
//! implemented by both [`PgStructuralOracle`](crate::pg::PgStructuralOracle) and
//! `DuckDbStructuralOracle` and driven
//! through one [`structural_comparison`] — the structural analogue of
//! [`accept_reject_divergence`]. SQL Server (ScriptDOM) could join later.

use squonk::ast::NoExt;
use squonk::{Dialect, parse_with};

use crate::shape::{StatementShape, squonk_shape_result};

/// An engine's verdict on whether it accepts a statement, computed without
/// executing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OracleVerdict {
    /// The engine accepts the statement (parse, or parse + bind, succeeded).
    Accept,
    /// The engine rejects the statement.
    Reject,
}

impl OracleVerdict {
    /// Lift a plain `accepts` boolean into a verdict.
    pub fn from_accepts(accepts: bool) -> Self {
        if accepts { Self::Accept } else { Self::Reject }
    }

    /// Whether this verdict is [`Accept`](Self::Accept).
    pub fn accepts(self) -> bool {
        matches!(self, Self::Accept)
    }
}

/// What "accept" means for an oracle — see the module-level contract. This governs
/// which corpus is comparable against the oracle, so it is part of the interface a
/// new oracle must declare honestly, not a hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OracleSemantics {
    /// Syntax only; unresolved names still accept. Comparable over the full surface.
    ParseOnly,
    /// Parse + name resolution; unknown objects reject. Comparable only over
    /// schema-independent SQL, or after the schema is provisioned.
    PrepareBind,
}

/// An oracle could not produce a verdict for an **infrastructure** reason (e.g. a
/// container is not running), as opposed to the engine rejecting the SQL. The
/// differential treats this as a skip, never as a divergence.
#[derive(Clone, Debug)]
pub struct OracleUnavailable(pub String);

/// A real-engine accept/reject ground-truth source for the differential.
///
/// Implementors are the per-milestone engines (see the module table). Keep the
/// verdict parse/prepare-only: never execute, mutate, or block on the network in the
/// default test path.
pub trait AcceptRejectOracle {
    /// Stable identifier used in divergence reports and allowlist entries.
    fn name(&self) -> &'static str;

    /// Acceptance semantics; governs which corpus is comparable (module contract).
    fn semantics(&self) -> OracleSemantics;

    /// The engine's verdict on `sql`, computed without executing it.
    ///
    /// `Err(`[`OracleUnavailable`]`)` is reserved for the engine being unreachable —
    /// an infrastructure failure the differential skips. A statement the engine
    /// *rejects* is `Ok(`[`OracleVerdict::Reject`]`)`, not an error.
    fn verdict(&self, sql: &str) -> Result<OracleVerdict, OracleUnavailable>;
}

/// The accept/reject divergence between `oracle` and our parser under `dialect`.
///
/// `Some(detail)` when the engine and `squonk` disagree on acceptance, else
/// `None`. Raw: no triage allowlist is applied (callers compose that explicitly), so
/// the fuzz loop can drive it over generated input.
///
/// An [`OracleUnavailable`] oracle yields `None` (skip): it is not comparable, and an
/// infrastructure failure must never read as a parser divergence. Availability
/// gating for opt-in container oracles is the caller's responsibility.
///
/// For a [`OracleSemantics::PrepareBind`] oracle, restrict `sql` to schema-independent
/// statements or provision the schema first (module contract) — this function does
/// not, and cannot, know the engine's schema.
pub fn accept_reject_divergence<D: Dialect, O: AcceptRejectOracle>(
    sql: &str,
    dialect: D,
    oracle: &O,
) -> Option<String> {
    let engine = match oracle.verdict(sql) {
        Ok(verdict) => verdict,
        Err(OracleUnavailable(_)) => return None,
    };
    let ours =
        OracleVerdict::from_accepts(parse_with(sql, squonk::ParseConfig::new(dialect)).is_ok());
    (engine != ours).then(|| {
        format!(
            "{}={}, squonk={}",
            oracle.name(),
            engine.accepts(),
            ours.accepts(),
        )
    })
}

// ---------------------------------------------------------------------------
// Structural parity seam (the second oracle kind — see the module note)
// ---------------------------------------------------------------------------

/// An engine's structural verdict for a statement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructuralShape {
    /// The engine mapped the statement into the neutral shape family.
    Mapped(Vec<StatementShape>),
    /// The statement is outside this engine's comparable structural subset — the engine
    /// rejects it (DuckDB serializes only `SELECT`), or it carries a construct with no
    /// neutral shape yet. A **skip** for the differential, never a divergence.
    OutsideSubset(String),
}

/// A real-engine **structural** ground-truth source: it maps a statement to the neutral
/// [`StatementShape`](crate::shape) family so the differential can compare tree *shape*, not
/// just accept/reject. The second-source abstraction this module deferred until DuckDB's
/// `json_serialize_sql` provided it (see the module note).
///
/// `Err(`[`OracleUnavailable`]`)` is reserved for the engine being unreachable (an
/// infrastructure skip); a statement the engine cannot serialize is
/// `Ok(`[`StructuralShape::OutsideSubset`]`)`, not an error.
pub trait StructuralOracle {
    /// Stable identifier used in divergence reports.
    fn name(&self) -> &'static str;

    /// The engine's structural shape for `sql`.
    fn shape(&self, sql: &str) -> Result<StructuralShape, OracleUnavailable>;
}

/// The outcome of comparing our parse against an engine's structural shape.
#[derive(Debug)]
pub enum Comparison {
    /// Our parser rejects `sql` — outside the both-accept subset.
    OursReject,
    /// Our parser accepts but the neutral model does not cover the statement kind.
    OursOutside(String),
    /// The engine is outside its comparable subset (rejects / no neutral shape).
    EngineOutside(String),
    /// The engine is unreachable (infrastructure skip).
    Unavailable(String),
    /// Both sides map to the same neutral shape.
    Match,
    /// Both sides mapped, but to different shapes.
    Divergence(String),
}

/// Compare our parse of `sql` (under `dialect`) against `oracle`'s structural shape — the
/// structural analogue of [`accept_reject_divergence`], generalized over the
/// [`StructuralOracle`] seam so both PostgreSQL and DuckDB drive **one** comparator vs
/// [`squonk_shape`](crate::shape::squonk_shape). Returns a richer outcome than a bare
/// divergence so a corpus sweep can tally the honest residual (compared vs skipped, why).
///
/// `dialect` is bound `Ext = NoExt`: the neutral mapping
/// ([`squonk_shape_result`](crate::shape)) is over the base AST, not an extension one.
pub fn structural_comparison<D: Dialect<Ext = NoExt>, O: StructuralOracle>(
    sql: &str,
    dialect: D,
    oracle: &O,
) -> Comparison {
    let ours = match parse_with(sql, squonk::ParseConfig::new(dialect)) {
        Ok(parsed) => parsed,
        Err(_) => return Comparison::OursReject,
    };
    let engine_shape = match oracle.shape(sql) {
        Err(OracleUnavailable(reason)) => return Comparison::Unavailable(reason),
        Ok(StructuralShape::OutsideSubset(reason)) => return Comparison::EngineOutside(reason),
        Ok(StructuralShape::Mapped(shape)) => shape,
    };
    // Our shape is fallible-then-panic (`squonk_shape` unwraps); use the `Result`
    // form and guard the residual `unreachable!` arms (non-Ansi type/operator kinds) so
    // an unexpected corpus statement is a clean skip, never a test-killing panic.
    let ours_shape =
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| squonk_shape_result(&ours)))
        {
            Ok(Ok(shape)) => shape,
            Ok(Err(reason)) => return Comparison::OursOutside(reason),
            Err(_) => return Comparison::OursOutside("squonk shape mapping panicked".into()),
        };

    if ours_shape == engine_shape {
        Comparison::Match
    } else {
        Comparison::Divergence(format!(
            "structural shape mismatch for {sql:?}\n  squonk = {ours_shape:?}\n  {} = {engine_shape:?}",
            oracle.name(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use squonk::dialect::Ansi;

    /// A second, trivial oracle: proves the trait is implementable by an independent
    /// type and that [`accept_reject_divergence`] composes generically — the exact
    /// path M2/M3/M4 will take. Accepts everything.
    struct AcceptEverything;

    impl AcceptRejectOracle for AcceptEverything {
        fn name(&self) -> &'static str {
            "accept_everything"
        }
        fn semantics(&self) -> OracleSemantics {
            OracleSemantics::ParseOnly
        }
        fn verdict(&self, _sql: &str) -> Result<OracleVerdict, OracleUnavailable> {
            Ok(OracleVerdict::Accept)
        }
    }

    /// A `PrepareBind` oracle that is always unreachable — exercises the skip path.
    struct OfflineEngine;

    impl AcceptRejectOracle for OfflineEngine {
        fn name(&self) -> &'static str {
            "offline_engine"
        }
        fn semantics(&self) -> OracleSemantics {
            OracleSemantics::PrepareBind
        }
        fn verdict(&self, _sql: &str) -> Result<OracleVerdict, OracleUnavailable> {
            Err(OracleUnavailable("engine offline".to_string()))
        }
    }

    #[test]
    fn verdict_lifts_and_lowers_accepts_booleans() {
        assert_eq!(OracleVerdict::from_accepts(true), OracleVerdict::Accept);
        assert_eq!(OracleVerdict::from_accepts(false), OracleVerdict::Reject);
        assert!(OracleVerdict::Accept.accepts());
        assert!(!OracleVerdict::Reject.accepts());
    }

    #[test]
    fn divergence_is_reported_when_engine_and_parser_disagree() {
        // Our parser rejects this (a bare number is not a statement); the always-accept
        // oracle accepts it -> the seam reports the divergence with both verdicts.
        let detail = accept_reject_divergence("123 456", Ansi, &AcceptEverything)
            .expect("disagreement is a divergence");
        assert_eq!(detail, "accept_everything=true, squonk=false");

        // Both accept a valid statement -> no divergence.
        assert!(accept_reject_divergence("SELECT 1", Ansi, &AcceptEverything).is_none());
    }

    #[test]
    fn unavailable_oracle_is_skipped_not_a_divergence() {
        // An infrastructure failure must never read as a parser divergence, whether
        // our parser would accept or reject the input.
        assert!(accept_reject_divergence("SELECT 1", Ansi, &OfflineEngine).is_none());
        assert!(accept_reject_divergence("123 456", Ansi, &OfflineEngine).is_none());
    }

    #[test]
    fn semantics_is_declared_per_oracle() {
        assert_eq!(AcceptEverything.semantics(), OracleSemantics::ParseOnly);
        assert_eq!(OfflineEngine.semantics(), OracleSemantics::PrepareBind);
    }
}
