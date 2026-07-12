// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared spine for the corpus accept/reject verdict sweeps
//! (`conformance-verdict-harness-consolidation`).
//!
//! The four sweeps — [`corpus_pg_verdicts`](crate::corpus_pg_verdicts),
//! [`corpus_mysql_verdicts`](crate::corpus_mysql_verdicts),
//! [`corpus_sqlite_verdicts`](crate::corpus_sqlite_verdicts), and
//! [`corpus_duckdb_verdicts`](crate::corpus_duckdb_verdicts) — grew by cloning one
//! another, so they carry the same pieces by construction, not coincidence. This
//! module owns those pieces; each sweep composes them with its own engine specifics.
//!
//! What is genuinely shared (lives here):
//! - The vendored multi-dialect corpus loaders (pg + mysql + sqlite draw the same
//!   sqlglot / sqllogictest / sqlglot-complex fixtures).
//! - [`GapClass`] + [`Probe`] + [`check_probe_group`]: the authored-probe class
//!   agreement check (mysql + sqlite).
//! - [`RejectReason`] + [`Verdict`] + [`Quadrant`]: the bare-OR-schema accept
//!   semantics, the `comparable`/`newly_comparable` derivations, and the quadrant
//!   tally the three `PrepareBind` parity gates (sqlite + duckdb + mysql) share
//!   (`mysql-oracle-at-scale` moved mysql onto them from its earlier bespoke `Bucket`
//!   tally). PG stays out — its oracle is `ParseOnly` (see below).
//! - [`DivergenceEntry`] + the ledger staleness assertions (sqlite + duckdb + mysql).
//!
//! What stays per-engine (does NOT belong here — the STOP condition's fault lines):
//! - The reject-message **classifier** strings (`classify_*`): each engine words its
//!   syntax/binding rejects differently.
//! - The **provisioning** strategy: none / positional-replay (sqlite) / per-file DDL
//!   groups (duckdb) / external server (mysql). Each is a genuinely different driver.
//! - The **gate policy** and **routing**: sqlite pins over-acceptance per corpus,
//!   duckdb routes coverage gaps to signature families with a hard zero-untriaged
//!   gate. Different shapes, wired in each sweep's own loop off the shared [`Cell`].
//! - The **printed block** format: the same skeleton, but each engine annotates its
//!   lines differently (header, per-engine suffixes). Unifying it would cost a
//!   fistful of fragile format parameters for a handful of shared lines, so each
//!   sweep prints inline off the shared [`Quadrant`] counters instead.
//! - The **pin values** each sweep baselines: per-corpus counts (sqlite/mysql/pg), a
//!   per-family map (duckdb), and the quadrant residual tuple — different shapes and
//!   arities, gated differently, so no shared "pins struct" fits all four.
//!
//! [`corpus_pg_verdicts`](crate::corpus_pg_verdicts) is the odd one out and stays fully
//! separate: its `pg_query` oracle is `ParseOnly`, so it has no bare/schema split, no
//! [`RejectReason`], and no [`Quadrant`] — it computes a two-valued `Direction` from
//! `postgres_accepts` alone and gates a *kind-tagged, both-direction* allowlist (an
//! allowlisted entry can be a coverage gap, e.g. the deferred `-|-` operator), beside a
//! separate fingerprint-mediated structural lane. None of that is a quadrant sweep.
//!
//! # Why one config-driven driver does not replace the four sweeps (measured NO-GO)
//!
//! `conformance-eval-sweep-as-config` re-litigated the consolidation's STOP boundary with a
//! changed constraint — pluggable routing via per-engine strategy *hooks/closures*, not the
//! declined format parameters — and measured a **NO-GO**. The consolidation already extracted
//! the one genuinely shared primitive ([`Quadrant::record`] returning [`Cell`]); a hook-based
//! driver adds only a generic loop around it, and the loop was never the bulk. Routing a
//! single engine (sqlite) through a best-effort hook driver measured net **+54 LOC** (the
//! ~15-line inner loop it absorbs is smaller than the closure-config that replaces it, before
//! the driver's own ~25 lines) — verdict production, the gates, the printed block, and the
//! pins all stay per-engine regardless. The driver needs five config fields, four of them
//! whole-body closures capturing mutable routing state — as numerous as, and more fragile
//! than, the format parameters it was meant to avoid: harder to read, and forced to span the
//! two independent `oracle-engines` / `oracle-mysql` features the separated sweeps keep apart.
//! Two hard cases make it strictly worse, not just larger. PG is not a quadrant sweep at all
//! (forcing it fabricates a degenerate [`Verdict`] and still leaves its structural lane and
//! both-direction allowlist outside the driver). And mysql's over-accept-*other* sampling
//! records the wire error CODE captured during verdict production, which a uniform
//! `(unit, sql)` router cannot see without a forbidden second wire query (it would reopen the
//! oracle-death window) or an engine-specific raw-detail field bolted onto the shared
//! [`Verdict`]. So the fault lines above stay: the hooks are the "fistful of fragile
//! parameters" relocated, not removed.
//!
//! Everything below is `#[cfg(test)]` support: several items are used only under
//! `oracle-engines` / `oracle-mysql`, so they carry `#[allow(dead_code)]` for the
//! default-feature build where their consumer sweep is not compiled.

// ---------------------------------------------------------------------------
// Vendored multi-dialect corpus loaders (pg + mysql + sqlite)
// ---------------------------------------------------------------------------

const SQLGLOT_IDENTITY: &str = include_str!("../corpus/sqlglot/identity.sql");
const SQLLOGICTEST: &str = include_str!("../corpus/sqllogictest/statements.sql");

/// The six sqlglot-complex datasets, in the fixed source order the sibling
/// `corpus_complex` loader (and the bench corpus loader) pin.
const COMPLEX_FILES: &[&str] = &[
    include_str!("../corpus/sqlglot-complex/tpc-h.sql"),
    include_str!("../corpus/sqlglot-complex/tpc-ds.sql"),
    include_str!("../corpus/sqlglot-complex/merge_subqueries.sql"),
    include_str!("../corpus/sqlglot-complex/unnest_subqueries.sql"),
    include_str!("../corpus/sqlglot-complex/pushdown_cte_alias_columns.sql"),
    include_str!("../corpus/sqlglot-complex/eliminate_ctes.sql"),
];

/// The sqlglot identity corpus, one statement per line.
pub fn sqlglot_identity_lines() -> Vec<&'static str> {
    SQLGLOT_IDENTITY.lines().collect()
}

/// The sqllogictest corpus, one statement per line.
pub fn sqllogictest_lines() -> Vec<&'static str> {
    SQLLOGICTEST.lines().collect()
}

/// Drop a leading run of blank / `--` comment lines from a `;`-delimited chunk, then
/// trim — the SPDX banner precedes the first statement in the complex fixtures.
/// Mirrors `corpus_complex` (and the bench loader) so every consumer cuts the
/// vendored files into identical statements.
fn strip_leading_comment_lines(chunk: &str) -> &str {
    let mut rest = chunk.trim();
    while rest.starts_with("--") {
        let cut = rest.find('\n').map_or(rest.len(), |i| i + 1);
        rest = rest[cut..].trim_start();
    }
    rest.trim_end()
}

/// The sqlglot-complex statements: each dataset is `;`-split (no `;` appears inside a
/// statement — enforced at vendoring time and pinned by each consumer's count).
pub fn sqlglot_complex_statements() -> Vec<&'static str> {
    COMPLEX_FILES
        .iter()
        .flat_map(|&text| text.split(';'))
        .map(strip_leading_comment_lines)
        .filter(|sql| !sql.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// Authored-probe class agreement (mysql + sqlite)
// ---------------------------------------------------------------------------

/// How a coverage-gap probe maps onto the dialect modelling work — the phase-0
/// classification the child tickets act on, grounded in the structural-dialect spike.
///
/// The taxonomy stays declared in full while a programme flips families to
/// [`Control`](Self::Control) probe-by-probe, so a variant can be unconstructed
/// between rounds (and entirely unconstructed under a feature build that compiles
/// neither probe sweep) — hence the enum-level `#[allow(dead_code)]`.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapClass {
    /// A `FeatureSet` flag/enum delta plus a small parser backing closes it — no new
    /// AST node.
    FeatureSet,
    /// Folds into an existing shared shape as a new field/variant + a gate.
    ExtensionShape,
    /// Needs a brand-new canonical statement node or clause family.
    NewStatement,
    /// A parity control: both sides agree already (a family we parse), or the
    /// divergence is semantic and invisible to an accept/reject oracle. Recorded so
    /// the classification cannot rot as the parser grows, not a gap to close.
    Control,
}

/// One authored probe, tagged with the modelling class the child tickets act on.
/// Every probe is verified to prepare/accept on the live engine (asserted by
/// [`check_probe_group`]); [`GapClass::Control`] ⟺ the fitted preset already parses it.
pub struct Probe {
    pub sql: &'static str,
    pub class: GapClass,
}

/// Check one probe group against its oracle: every probe must be accepted by the
/// engine (a reject means schema/version drift, never a valid classification), and the
/// recorded [`GapClass`] must agree with the parser (`Control` ⟺ we accept), so the
/// classification cannot silently rot when the parser grows. Prints one line per
/// probe (the caller frames the group + owns the pinned gap count) and returns the
/// number of non-`Control` (coverage-gap) probes.
#[allow(dead_code)]
pub fn check_probe_group(
    engine: &str,
    probes: &[Probe],
    engine_accepts: impl Fn(&str) -> bool,
    ours_accepts: impl Fn(&str) -> bool,
) -> usize {
    let mut gaps = 0usize;
    for probe in probes {
        let accepts = engine_accepts(probe.sql);
        let ours = ours_accepts(probe.sql);
        eprintln!(
            "  {engine}={accepts:<5} ours={ours:<5} [{:?}] {:?}",
            probe.class, probe.sql
        );

        assert!(
            accepts,
            "probe no longer prepares under the {engine} oracle (schema/version drift?): {:?}",
            probe.sql,
        );
        let is_control = probe.class == GapClass::Control;
        assert_eq!(
            ours,
            is_control,
            "probe {:?} classified {:?} but the fitted preset {} it — re-classify (Control ⟺ already parsed)",
            probe.sql,
            probe.class,
            if ours { "accepts" } else { "rejects" },
        );
        if !is_control {
            gaps += 1;
        }
    }
    gaps
}

// ---------------------------------------------------------------------------
// Reject-reason trichotomy + one statement's verdict (sqlite + duckdb parity gates)
// ---------------------------------------------------------------------------

/// Why the engine rejected a statement, read from the *bare* probe so the split is
/// authoritative: a syntax reject is schema-independent (the engine reports it before
/// name resolution), so the bare reason settles the split even when the engine
/// rejects with the schema too. The per-engine `classify_*` fn maps message text onto
/// these — the strings are the genuine per-engine specific, not this trichotomy.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// Real parser disagreement — schema-independent, so the ledgered class.
    Syntax,
    /// Name resolution (unknown table/column, already exists) — a counted residual.
    Binding,
    /// Something else (a semantic reject after parsing) — counted, never ledgered.
    Other,
}

/// One statement's verdict against a `PrepareBind` oracle, behind a setup driver.
/// The construction (which provisioning fills `schema_accepts`, how `bare_reason` is
/// classified) is per-engine; the derivations below are shared by construction.
#[allow(dead_code)]
pub struct Verdict {
    /// Whether the fitted preset accepts the statement.
    pub ours: bool,
    /// Whether the engine accepts it over a bare (empty) database.
    pub bare_accepts: bool,
    /// Whether the engine accepts it once the setup driver provisions the schema.
    pub schema_accepts: bool,
    /// Bare reject reason — only meaningful when the engine rejects
    /// (`!engine_accepts`).
    pub bare_reason: RejectReason,
}

#[allow(dead_code)]
impl Verdict {
    /// The engine accepts if the bare DB *or* the provisioned schema accepts: bare
    /// covers the schema-independent statements and neutralizes the CREATE-under-test
    /// self-collision (a redefinition rejects "already exists" against its own epoch,
    /// but a fresh bare DB accepts it — the honest verdict); schema-accept covers the
    /// statements the setup driver unblinds.
    pub fn engine_accepts(&self) -> bool {
        self.bare_accepts || self.schema_accepts
    }

    /// Comparable = the engine's verdict is trustworthy: it accepts, or it rejects for
    /// a reason a schema cannot mask (syntax). A binding/other reject means the setup
    /// driver did not cover it — a counted residual (the STOP fallback).
    pub fn comparable(&self) -> bool {
        self.engine_accepts() || self.bare_reason == RejectReason::Syntax
    }

    /// Unblinded by the setup driver: bare-rejected but schema-accepts. A syntax
    /// reject never flips to accept, so this isolates exactly the binding-masked
    /// signal the schema reveals.
    pub fn newly_comparable(&self) -> bool {
        !self.bare_accepts && self.schema_accepts
    }
}

/// Which quadrant a verdict falls in, returned by [`Quadrant::record`] so each sweep
/// can route the two interesting cells (coverage gap, syntax over-acceptance) through
/// its own gate policy while the tally bookkeeping stays shared.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// We accept, the engine accepts (A/A).
    AgreeAccept,
    /// We reject, the engine accepts (R/A) — the real coverage-gap signal.
    CoverageGap,
    /// We accept, the engine syntax-rejects (A/R) — the REAL over-acceptance, ledgered.
    OverAcceptSyntax,
    /// We accept, the engine binding-rejects (A/R) — schema-miss residual.
    OverAcceptBinding,
    /// We accept, the engine rejects for another reason (A/R) — semantic, not ledgered.
    OverAcceptOther,
    /// Both reject, the engine for syntax (R/R) — mutual syntax reject.
    AgreeRejectSyntax,
    /// Both reject, the engine for binding/other (R/R) — masked residual.
    AgreeRejectBinding,
}

/// The shared quadrant tally: the counter bookkeeping the two parity gates run
/// identically, so a gate keeps only its own routing state (per-corpus maps,
/// signature-family maps, untriaged lists) beside a `Quadrant`. `total` and
/// `residual` stay per-engine (the iteration unit differs — corpora vs file groups).
#[allow(dead_code)]
#[derive(Default)]
pub struct Quadrant {
    pub agree_accept: usize,
    pub agree_reject_syntax: usize,
    pub agree_reject_binding: usize,
    pub coverage_gap: usize,
    pub over_accept_syntax: usize,
    pub over_accept_binding: usize,
    pub over_accept_other: usize,
    pub newly_comparable: usize,
    pub comparable: usize,
}

#[allow(dead_code)]
impl Quadrant {
    /// Tally one verdict: bump `comparable`/`newly_comparable`, place it in its
    /// quadrant, bump that counter, and return the [`Cell`] so the caller routes the
    /// coverage-gap and over-acceptance cells through its own gate.
    pub fn record(&mut self, v: &Verdict) -> Cell {
        if v.newly_comparable() {
            self.newly_comparable += 1;
        }
        if v.comparable() {
            self.comparable += 1;
        }
        match (v.ours, v.engine_accepts()) {
            (true, true) => {
                self.agree_accept += 1;
                Cell::AgreeAccept
            }
            (false, true) => {
                self.coverage_gap += 1;
                Cell::CoverageGap
            }
            (true, false) => match v.bare_reason {
                RejectReason::Syntax => {
                    self.over_accept_syntax += 1;
                    Cell::OverAcceptSyntax
                }
                RejectReason::Binding => {
                    self.over_accept_binding += 1;
                    Cell::OverAcceptBinding
                }
                RejectReason::Other => {
                    self.over_accept_other += 1;
                    Cell::OverAcceptOther
                }
            },
            (false, false) => match v.bare_reason {
                RejectReason::Syntax => {
                    self.agree_reject_syntax += 1;
                    Cell::AgreeRejectSyntax
                }
                _ => {
                    self.agree_reject_binding += 1;
                    Cell::AgreeRejectBinding
                }
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Over-acceptance divergence ledger (sqlite + duckdb; the PG-ledger pattern)
// ---------------------------------------------------------------------------

/// A triaged over-acceptance: a statement the fitted preset accepts that the engine
/// *syntax*-rejects even with the schema provisioned — a real validator-correctness
/// divergence knowingly tolerated (fixing it is parser-crate work, outside the
/// conformance ticket). Mirrors `pg::PgDivergenceAllowlistEntry`: every entry names an
/// a non-empty provenance label, and the still-diverges assertion keeps a fixed
/// over-acceptance from staying silently allowlisted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DivergenceEntry {
    pub sql: &'static str,
    pub ticket: &'static str,
    pub reason: &'static str,
}

/// Whether a divergence entry carries a non-empty provenance label.
pub fn ticket_exists(ticket: &str) -> bool {
    !ticket.trim().is_empty()
}

/// Assert every ledger entry carries SQL, a reason, and a provenance label.
pub fn assert_entries_are_ticketed(entries: &[DivergenceEntry]) {
    for entry in entries {
        assert!(!entry.sql.trim().is_empty(), "allowlist SQL is required");
        assert!(
            !entry.reason.trim().is_empty(),
            "allowlist reason is required for {:?}",
            entry.sql,
        );
        assert!(
            ticket_exists(entry.ticket),
            "over-acceptance allowlist entry {:?} has no provenance label ({})",
            entry.sql,
            entry.ticket,
        );
    }
}

/// Assert every ledger entry still diverges, per the engine's own predicate (its parse
/// dialect, oracle, and reject classifier), so a fixed over-acceptance cannot stay
/// silently allowlisted.
#[allow(dead_code)]
pub fn assert_entries_still_diverge(
    entries: &[DivergenceEntry],
    still_diverges: impl Fn(&DivergenceEntry) -> bool,
) {
    for entry in entries {
        assert!(
            still_diverges(entry),
            "stale over-acceptance allowlist entry {:?}: the engine no longer syntax-rejects it, \
             so the over-acceptance is fixed — SWEEP this entry (delete it from its ledger), \
             never re-pin or edit it to keep it allowlisted (ADR-0015: a fix forces removal)",
            entry.sql,
        );
    }
}
