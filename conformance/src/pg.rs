// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! PostgreSQL differential oracle for the M1 parser surface.
//!
//! `pg_query` is the real PostgreSQL parser in-process (ADR-0015). This module
//! starts with the objective accept/reject oracle, then maps a PostgreSQL protobuf
//! subset into a neutral structural shape so we can compare M1 constructs without
//! requiring a second `squonk` interner. The shape covers the query surface
//! (`SELECT`/`VALUES`, set operations, the expression grammar), the DDL/DML families
//! (`CREATE TABLE`/`SCHEMA`/`VIEW`/`MATERIALIZED VIEW`/`INDEX`, `ALTER TABLE`, the
//! `DROP` family, `INSERT`/`UPDATE`/`DELETE` with their PostgreSQL extensions), and
//! the transaction-control (`BEGIN`/`COMMIT`/`ROLLBACK`/`SAVEPOINT`/`SET TRANSACTION`),
//! session (generic `SET`/`RESET`/`SHOW`), access-control (`GRANT`/`REVOKE`), and
//! `EXPLAIN` families (`pg-structural-oracle-for-dcl-tcl-utility`). Two utility
//! surfaces stay out by decision — `COPY` and the special `SET` subforms — surfacing
//! as explicit "not implemented" divergences rather than silent parity; see
//! [`StatementShape`].

use crate::oracle::{
    AcceptRejectOracle, OracleSemantics, OracleUnavailable, OracleVerdict, StructuralOracle,
    StructuralShape,
};
// The engine-neutral `*Shape` vocabulary and the `squonk`-side mapper live in
// `crate::shape`; the two straddling parity helpers below
// ([`assert_structural_parity`] / [`pg_structural_divergence`]) use
// `squonk_shape_result`, so the module root keeps the whole glob. The PostgreSQL
// protobuf mapper that builds every shape variant now lives in [`protobuf_shape`].
use crate::render_statements;
use crate::shape::*;
use squonk::dialect::Postgres;
use squonk::parse_with;
use squonk_ast::render::RenderMode;

/// The PostgreSQL protobuf -> neutral shape mapper: [`pg_shape`] and its ~148 per-node
/// helpers, split out of this file under the file+dir idiom.
mod protobuf_shape;

// Re-exported so `crate::pg::pg_shape` stays the stable path the `goldens` and
// `duckdb_structural` consumers already use, and so the parity helpers here can call it.
pub use protobuf_shape::pg_shape;

#[cfg(test)]
mod tests;

/// PostgreSQL differential divergence class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PgDivergenceKind {
    AcceptReject,
    Structural,
}

/// A triaged PostgreSQL differential divergence.
///
/// Entries are the accept/reject divergences the vendored-corpus verdict differential
/// (`corpus_pg_verdicts`,
/// run-pg-accept-reject-over-vendored-corpora) surfaced and triaged. Keeping the type
/// and helper in place makes each divergence an explicit, named, ticketed entry rather
/// than a silent weakening of the parity assertion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PgDivergenceAllowlistEntry {
    pub kind: PgDivergenceKind,
    pub sql: &'static str,
    pub ticket: &'static str,
    pub reason: &'static str,
}

/// Current PostgreSQL divergences allowed by the M1 oracle.
///
/// Every entry must carry a non-empty provenance label; the tests
/// also assert entries still diverge so fixed gaps do not stay silently
/// allowlisted. The set below is the triaged output of the vendored-corpus verdict
/// differential (`corpus_pg_verdicts`): the sole remaining entry is the deferred
/// `-|-` range-adjacency operator, a tracked coverage gap.
/// Over-acceptances (we accept SQL PostgreSQL rejects) are deliberately not
/// allowlisted — each is tightened to PG parity at the parse path, so the verdict
/// differential enforces the rejection forever rather than parking it here.
pub const PG_DIVERGENCE_ALLOWLIST: &[PgDivergenceAllowlistEntry] = &[
    // --- Known, tracked coverage gaps ---
    // COMMENT ON (the four object kinds above) now parses under the PostgreSQL preset
    // (close-p0-datafusion-parity-coverage-gaps), so its allowlist entries were removed;
    // the staleness test enforces that deletion.
    // The PG `-|-` range-adjacency operator needs no entry: it parses under the general
    // symbolic-operator surface (pg-operator-surface-regex-geometric-network:
    // `OperatorSyntax::custom_operators` — `-|-` is one `Op`-class token folding onto
    // `Expr::NamedOperator`), so both parsers accept it and there is nothing to allow. The
    // staleness test rejects any entry that would claim a divergence here.
    // The structural test-infra gap for the parenthesized WITH-query INSERT source
    // (set-shape-drops-nested-query-clauses) is fixed: `set_shape`/`query_shape` now
    // keep a set operand's own with/order_by/limit and collapse pure grouping, so the
    // structural-parity suite enforces it directly instead of allowlisting it.
];

/// The libpg_query (real PostgreSQL-17 parser) accept/reject oracle.
///
/// Parse-only ([`OracleSemantics::ParseOnly`]): libpg_query accepts statements with
/// unresolved object names, exactly like our parser, so it is comparable over the
/// full corpus and generated surface. In-process, so it never reports
/// [`OracleUnavailable`]. This is the M1
/// implementation of the pluggable [`AcceptRejectOracle`] seam that later dialect
/// milestones (M2 `rusqlite`/`duckdb`, M3 MySQL, M4 SQL Server) extend.
#[derive(Clone, Copy, Debug, Default)]
pub struct PgQueryOracle;

impl AcceptRejectOracle for PgQueryOracle {
    fn name(&self) -> &'static str {
        "pg_query"
    }

    fn semantics(&self) -> OracleSemantics {
        OracleSemantics::ParseOnly
    }

    fn verdict(&self, sql: &str) -> Result<OracleVerdict, crate::oracle::OracleUnavailable> {
        Ok(OracleVerdict::from_accepts(pg_query::parse(sql).is_ok()))
    }
}

/// Whether the real PostgreSQL parser accepts `sql`.
pub fn postgres_accepts(sql: &str) -> bool {
    // The in-process oracle never reports unavailable, so the verdict is always `Ok`.
    matches!(PgQueryOracle.verdict(sql), Ok(OracleVerdict::Accept))
}

/// Whether `squonk` accepts `sql` under the PostgreSQL feature preset.
pub fn squonk_accepts(sql: &str) -> bool {
    parse_with(sql, squonk::ParseConfig::new(Postgres)).is_ok()
}

/// The accept/reject divergence for `sql` — `Some(detail)` when `pg_query` and
/// `squonk` disagree on whether it parses, else `None`.
///
/// Raw: the allowlist is *not* applied here (callers compose it via
/// [`pg_divergence_allowlisted`]), so the fuzz loop can drive it over generated
/// input and triage findings explicitly.
///
/// Segmentation-aware, not merely boolean: when BOTH parsers accept, the top-level
/// statement counts must also agree, or the input is reported as a divergence. Boolean
/// agreement can mask segmentation disagreement — the statement-splitter over-acceptance
/// (pg-do-statement-separator-divergence) hid for exactly this reason: `pg_query` accepted
/// a multi-clause `CREATE FUNCTION` as ONE statement while our splitter mis-split it into
/// a statement plus trailing tokens that happened to parse, and `accept == accept` read as
/// agreement. Comparing counts makes the raw-byte fuzz loop hunt that whole class
/// continuously instead of relying on a boolean coincidence to break.
pub fn pg_accept_reject_divergence(sql: &str) -> Option<String> {
    let ours = parse_with(sql, squonk::ParseConfig::new(Postgres)).ok();
    let pg = pg_query::parse(sql).ok();
    match (&ours, &pg) {
        (Some(ours), Some(pg)) => {
            let our_count = ours.statements().len();
            let pg_count = pg.protobuf.stmts.len();
            (our_count != pg_count).then(|| {
                format!(
                    "segmentation: both accept, pg_query={pg_count} statement(s), \
                     squonk={our_count}"
                )
            })
        }
        (None, None) => None,
        (ours, pg) => Some(format!(
            "pg_query={}, squonk={}",
            pg.is_some(),
            ours.is_some(),
        )),
    }
}

/// The PostgreSQL implementation of the [`StructuralOracle`] seam, wrapping [`pg_shape`].
/// In-process (`pg_query`), so it never reports
/// [`OracleUnavailable`]. Present so the seam has a
/// second implementation beyond DuckDB — proving it is not DuckDB-specific — while the
/// exhaustive M1 PostgreSQL structural suite stays in [`pg_structural_divergence`].
#[derive(Clone, Copy, Debug, Default)]
pub struct PgStructuralOracle;

impl StructuralOracle for PgStructuralOracle {
    fn name(&self) -> &'static str {
        "pg_query"
    }

    fn shape(&self, sql: &str) -> Result<StructuralShape, OracleUnavailable> {
        match pg_query::parse(sql) {
            Ok(parsed) => Ok(match pg_shape(&parsed.protobuf) {
                Ok(shape) => StructuralShape::Mapped(shape),
                Err(reason) => StructuralShape::OutsideSubset(reason),
            }),
            Err(err) => Ok(StructuralShape::OutsideSubset(format!(
                "pg_query rejected: {err:?}"
            ))),
        }
    }
}

/// The structural divergence for `sql` — `Some(detail)` when the parsers map it to
/// different neutral query shapes (or PostgreSQL maps a construct ours cannot),
/// else `None`. Either parser rejecting `sql` is reported as a divergence too.
///
/// Raw (no allowlist; see [`pg_divergence_allowlisted`]). Maps through
/// [`squonk_shape`], which only covers the PostgreSQL structural corpus, so it
/// must run only on that subset — the structured fuzz loop gates it with
/// [`fuzz`](crate::fuzz)'s comparable predicate and the corpus restricts it to
/// mapped SQL.
pub fn pg_structural_divergence(sql: &str) -> Option<String> {
    let ours = match parse_with(sql, squonk::ParseConfig::new(Postgres)) {
        Ok(parsed) => parsed,
        Err(err) => return Some(format!("squonk rejected: {err:?}")),
    };
    let pg = match pg_query::parse(sql) {
        Ok(parsed) => parsed,
        Err(err) => return Some(format!("pg_query rejected: {err:?}")),
    };

    let ours_shape = match squonk_shape_result(&ours) {
        Ok(shape) => shape,
        Err(err) => return Some(err),
    };
    let pg_shape = match pg_shape(&pg.protobuf) {
        Ok(shape) => shape,
        Err(err) => return Some(err),
    };

    (ours_shape != pg_shape).then(|| "mapped shapes differ".to_string())
}

// ---------------------------------------------------------------------------
// Oracle-mediated structural lane (conformance-mediated-structural-lane-pg)
// ---------------------------------------------------------------------------

/// The classification of one PostgreSQL both-accept statement under the
/// fingerprint-mediated structural lane ([`PgMediatedStructuralOracle`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PgMediatedVerdict {
    /// `sql` is outside the both-accept subset — our parser rejects it, or `pg_query`
    /// cannot fingerprint the original (which requires PostgreSQL to parse it) — so
    /// there is nothing to compare. A skip: never a match/mismatch/unparseable.
    Skip(String),
    /// Our canonical render fingerprints identically to the original: the parse-tree
    /// shape survived the parse -> render round trip, modulo the fingerprint's
    /// documented blindness (literal values, alias names, IN-list length).
    Match,
    /// The two fingerprints differ — a structural drift that implicates the PARSER (we
    /// built the wrong tree) OR the RENDERER (we canonicalized to a different shape).
    Mismatch {
        original_hex: String,
        render_hex: String,
    },
    /// `pg_query` rejected our canonical render: the renderer emitted SQL PostgreSQL
    /// cannot re-parse. Also a parser-OR-renderer drift, isolated for triage.
    RenderUnparseable(String),
}

/// The oracle-mediated structural lane for PostgreSQL — the **commodity default**
/// structural check (conformance-mediated-structural-lane-pg; the ratified GO from the
/// `conformance-eval-mediated-roundtrip-structural` spike).
///
/// For a both-accept statement `s`, it round-trips `s` through our parser and canonical
/// renderer and asks the real PostgreSQL parser whether the *shape* survived, by
/// comparing `pg_query::fingerprint(s).hex` against
/// `pg_query::fingerprint(render_statements(&parse_with(s, Postgres)?, squonk::ParseConfig::new(Canonical))).hex`.
/// Each engine self-compares engine-tree vs engine-tree in its OWN fingerprint space, so
/// there is no cross-engine neutral vocabulary to reconcile — a small adapter instead of
/// the hundreds-of-lines hand-written mapper.
///
/// # Commodity vs premium — why [`PgStructuralOracle`] STAYS
///
/// This lane is BLIND, by construction (the fingerprint normalizes them), to exactly
/// three things: literal VALUES, column ALIAS names, and IN-list LENGTH. A misparse
/// whose ONLY symptom is a wrong literal, alias, or list arity is invisible here — so a
/// mediated green is NOT full structural coverage. That sensitivity is exactly what the
/// hand-written [`PgStructuralOracle`] (the neutral-shape mapper) encodes, which is why
/// it remains the **premium** tier: this lane augments it, it does not replace it.
#[derive(Clone, Copy, Debug, Default)]
pub struct PgMediatedStructuralOracle;

impl PgMediatedStructuralOracle {
    /// Stable identifier used in divergence reports.
    pub fn name(&self) -> &'static str {
        "pg_query (fingerprint-mediated)"
    }

    /// Classify `sql` under the mediated lane (see [`PgMediatedVerdict`]). Self-contained:
    /// it recomputes the both-accept precondition, so a non-comparable statement is a
    /// [`Skip`](PgMediatedVerdict::Skip) rather than a panic.
    pub fn verdict(&self, sql: &str) -> PgMediatedVerdict {
        let parsed = match parse_with(sql, squonk::ParseConfig::new(Postgres)) {
            Ok(parsed) => parsed,
            Err(err) => return PgMediatedVerdict::Skip(format!("squonk rejected: {err:?}")),
        };
        // Fingerprinting the original parses it with PostgreSQL, so success here IS the
        // pg-side of the both-accept precondition; a failure is the pg-rejects skip.
        let original_hex = match pg_query::fingerprint(sql) {
            Ok(fp) => fp.hex,
            Err(err) => {
                return PgMediatedVerdict::Skip(format!(
                    "pg_query could not fingerprint the original: {err:?}"
                ));
            }
        };
        let rendered = render_statements(&parsed, RenderMode::Canonical);
        match pg_query::fingerprint(&rendered) {
            Ok(fp) if fp.hex == original_hex => PgMediatedVerdict::Match,
            Ok(fp) => PgMediatedVerdict::Mismatch {
                original_hex,
                render_hex: fp.hex,
            },
            Err(err) => PgMediatedVerdict::RenderUnparseable(format!(
                "pg_query rejected our canonical render {rendered:?}: {err:?}"
            )),
        }
    }
}

/// Whether a divergence of `kind` for `sql` is named in [`PG_DIVERGENCE_ALLOWLIST`].
pub fn pg_divergence_allowlisted(kind: PgDivergenceKind, sql: &str) -> bool {
    allowlisted(kind, sql).is_some()
}

fn allowlisted(kind: PgDivergenceKind, sql: &str) -> Option<&'static PgDivergenceAllowlistEntry> {
    PG_DIVERGENCE_ALLOWLIST
        .iter()
        .find(|entry| entry.kind == kind && entry.sql == sql)
}

/// Assert accept/reject parity for one SQL string, unless explicitly allowlisted.
///
/// # Panics
///
/// Panics when `pg_query` and `squonk` disagree and no allowlist entry names
/// the SQL string.
pub fn assert_accept_reject_parity(sql: &'static str) {
    if let Some(detail) = pg_accept_reject_divergence(sql) {
        assert!(
            pg_divergence_allowlisted(PgDivergenceKind::AcceptReject, sql),
            "untriaged PostgreSQL accept/reject divergence for {sql:?}: {detail}",
        );
    }
}

/// Parse `sql` with both parsers and compare the mapped M1 query structure.
///
/// # Panics
///
/// Panics if either parser rejects `sql`, if the PostgreSQL protobuf contains an
/// unmapped top-level construct, or if the mapped structures differ.
pub fn assert_structural_parity(sql: &str) {
    let ours = parse_with(sql, squonk::ParseConfig::new(Postgres))
        .unwrap_or_else(|err| panic!("expected squonk to parse {sql:?}: {err:?}"));
    let pg = pg_query::parse(sql)
        .unwrap_or_else(|err| panic!("expected pg_query to parse {sql:?}: {err:?}"));

    let ours_shape = match squonk_shape_result(&ours) {
        Ok(shape) => shape,
        Err(err) => {
            if allowlisted(PgDivergenceKind::Structural, sql).is_some() {
                return;
            }
            panic!("untriaged PostgreSQL structural mapping divergence for {sql:?}: {err}");
        }
    };
    let pg_shape = match pg_shape(&pg.protobuf) {
        Ok(shape) => shape,
        Err(err) => {
            if allowlisted(PgDivergenceKind::Structural, sql).is_some() {
                return;
            }
            panic!("untriaged PostgreSQL structural mapping divergence for {sql:?}: {err}");
        }
    };

    if ours_shape != pg_shape {
        if allowlisted(PgDivergenceKind::Structural, sql).is_some() {
            return;
        }
        assert_eq!(
            ours_shape, pg_shape,
            "untriaged PostgreSQL structural shape mismatch for {sql:?}",
        );
    }
}

/// One parsed `regress-guide` fixture case: a PostgreSQL statement the real engine
/// accepts but our neutral mapping does not yet cover, tagged with the ticket tracking
/// the gap. Parsed from the PG-specific `regress-guide.sql` corpus by
/// [`pg_regress_guide_cases`] and asserted still-diverging by
/// `tests::pg_regress_guide_cases_remain_ticketed_gaps`. PG-specific fixture logic, so it
/// stays in the module root beside the oracle rather than moving into `pg/tests.rs`.
#[cfg(test)]
#[derive(Debug)]
struct PgGuideCase {
    id: String,
    source: String,
    ticket: String,
    sql: String,
}

/// Parse the `-- case:` / `-- source:` / `-- ticket:` block format of the PG-specific
/// `regress-guide.sql` corpus into [`PgGuideCase`]s.
#[cfg(test)]
fn pg_regress_guide_cases(input: &str) -> Vec<PgGuideCase> {
    let mut cases = Vec::new();
    let mut current = PgGuideCaseBuilder::default();

    for line in input.lines() {
        if let Some(id) = line.strip_prefix("-- case:") {
            current.finish_into(&mut cases);
            current.id = Some(id.trim().to_owned());
        } else if let Some(source) = line.strip_prefix("-- source:") {
            current.source = Some(source.trim().to_owned());
        } else if let Some(ticket) = line.strip_prefix("-- ticket:") {
            current.ticket = Some(ticket.trim().to_owned());
        } else if current.id.is_some() && !line.trim_start().starts_with("--") {
            current.sql.push_str(line);
            current.sql.push('\n');
        }
    }
    current.finish_into(&mut cases);
    cases
}

#[cfg(test)]
#[derive(Default)]
struct PgGuideCaseBuilder {
    id: Option<String>,
    source: Option<String>,
    ticket: Option<String>,
    sql: String,
}

#[cfg(test)]
impl PgGuideCaseBuilder {
    fn finish_into(&mut self, cases: &mut Vec<PgGuideCase>) {
        let Some(id) = self.id.take() else {
            return;
        };
        let source = self
            .source
            .take()
            .unwrap_or_else(|| panic!("guide case {id} is missing source"));
        let ticket = self
            .ticket
            .take()
            .unwrap_or_else(|| panic!("guide case {id} is missing ticket"));
        let sql = self.sql.trim().to_owned();
        assert!(!sql.is_empty(), "guide case {id} is missing SQL");
        self.sql.clear();
        cases.push(PgGuideCase {
            id,
            source,
            ticket,
            sql,
        });
    }
}
