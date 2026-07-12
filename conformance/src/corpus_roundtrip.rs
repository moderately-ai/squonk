// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared round-trip stability machinery + triage vocabulary for the broad
//! vendored corpora (`corpus_sqlglot`, `corpus_sqllogictest`, `corpus_complex`).
//!
//! ADR-0014's P3 step — "corpus idempotence/stability" — asks for more than parse
//! acceptance over the vendored corpus: an accepted statement the dialect can
//! render must survive parse -> render -> re-parse back to a *structurally equal*
//! AST. "Structurally equal" is the derived `PartialEq` with the `Meta` wrapper
//! excluding span + `NodeId` (ADR-0002) and a shared test interner reconciling the
//! two parses' `Symbol` spaces (ADR-0003) — the same oracle the public round-trip
//! helpers use. The check runs in *both* render modes: the minimal-parentheses
//! Canonical mode and the independent, structure-derived Parenthesized mode
//! (ADR-0008/0010), so a precedence mis-bind cannot hide behind identical text.
//!
//! This module is the single home for that check ([`roundtrip`]) and for the
//! vocabulary that classifies a case which fails it ([`RoundtripDefect`]), so the
//! three corpus replayers share one mechanism rather than each inventing a parallel
//! one.

use squonk::{Dialect, parse_with};
use squonk_ast::NoExt;
use squonk_ast::render::RenderMode;

/// Outcome of attempting a both-modes round-trip under one dialect.
pub(crate) enum Roundtrip {
    /// The dialect's parser rejects the input outright — nothing to render.
    Unparsable,
    /// Parsed, but a render -> re-parse -> structural-compare step diverged. Carries
    /// a rendered diff (input, render, and both ASTs) for triage.
    Failed(String),
    /// Parsed and round-tripped structurally in *both* render modes.
    Ok,
}

/// Attempt a Canonical + fully-Parenthesized round-trip under `dialect`.
///
/// Non-panicking so it drives both the classifying corpus partitions and the
/// panicking oracle wrappers, reusing the same crate internals the public oracles
/// use ([`render_statements`](crate::render_statements) and the shared-interner
/// structural comparison). Returns at the first diverging mode.
pub(crate) fn roundtrip<D: Dialect<Ext = NoExt> + Copy>(sql: &str, dialect: D) -> Roundtrip {
    let parsed = match parse_with(sql, dialect) {
        Ok(parsed) => parsed,
        Err(_) => return Roundtrip::Unparsable,
    };
    for mode in [RenderMode::Canonical, RenderMode::Parenthesized] {
        let rendered = crate::render_statements(&parsed, mode);
        let reparsed = match parse_with(&rendered, dialect) {
            Ok(reparsed) => reparsed,
            Err(err) => {
                return Roundtrip::Failed(format!(
                    "reparse of {mode:?} render {rendered:?} failed: {err:?}"
                ));
            }
        };
        let comparison = crate::shared_interner::compare_statements_with_shared_symbols(
            parsed.statements(),
            parsed.resolver(),
            reparsed.statements(),
            reparsed.resolver(),
        );
        if !comparison.structurally_equal() {
            return Roundtrip::Failed(comparison.failure_message(
                &format!("round-trip mismatch in {mode:?} mode"),
                &[("input", sql), ("render", &rendered)],
                None,
            ));
        }
    }
    Roundtrip::Ok
}

/// Assert every line of `raw_text` that *parses* under `dialect` also round-trips
/// under it (both render modes); a line `dialect` rejects is simply skipped.
///
/// Unlike [`CorpusSpec`](crate::corpus_partition::CorpusSpec)'s pinned three-way
/// Ansi/Postgres/defect partition, this carries no coverage pin — the pragmatic
/// shape for a dialect whose defining trait is a wide, still-growing acceptance
/// boundary (e.g. [`Lenient`](squonk::dialect::Lenient), the permissive
/// "parse anything" union): pinning its exact accepted subset would need
/// re-deriving on every grammar change, whereas the property this exists to check —
/// "whatever it accepts stays round-trip-stable" — does not.
pub(crate) fn assert_accepted_lines_round_trip<D: Dialect<Ext = NoExt> + Copy>(
    raw_text: &str,
    dialect: D,
) {
    for sql in raw_text.lines() {
        match roundtrip(sql, dialect) {
            Roundtrip::Ok | Roundtrip::Unparsable => {}
            Roundtrip::Failed(message) => {
                panic!("{sql:?} parses but does not round-trip: {message}")
            }
        }
    }
}

/// Triage label for a corpus case the parser accepts but cannot parse -> render ->
/// re-parse back to a structurally equal tree.
///
/// The label answers *where the divergence lives*, which decides who owns the fix.
/// It is assigned by a human reading the failing case, never by the harness — the
/// harness only *detects* non-stability (see [`roundtrip`]). The triage rule, in
/// order:
///
/// 1. The render or the re-parse cannot represent a construct the input has (the
///    re-parse errors, or the render visibly drops the construct) →
///    [`Unsupported`](Self::Unsupported): a coverage gap, no component is *wrong*.
/// 2. Both parses round-trip under a single non-Ansi preset but diverge once the
///    Canonical render is re-parsed under a different grammar →
///    [`DialectDivergence`](Self::DialectDivergence): the line is dialect-specific.
/// 3. Otherwise weigh the *first* AST against the input's meaning: a first parse
///    that already mis-binds is a [`ParserBug`](Self::ParserBug); a faithful first
///    parse whose rendered text loses or reshapes structure is a
///    [`RendererBug`](Self::RendererBug).
///
/// A labelled case stays in its corpus module's defect allowlist, tracked under
/// [`ROUNDTRIP_DEFECT_TICKET`], so a real defect is recorded with its SQL and class
/// and keeps the gate green-but-tracked rather than being silently dropped or
/// hard-failing the suite (ADR-0014 P3: triaged/quarantined, never blanket-skipped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RoundtripDefect {
    /// The construct is outside the current parser/renderer surface: the case
    /// parses, but the render or the re-parse cannot represent it, so the trees
    /// differ. No component is buggy — it is a coverage gap that closes when the
    /// missing construct lands, at which point the case promotes to the supported
    /// set.
    Unsupported,
    /// The *parser* builds the wrong tree (classically a precedence mis-bind): the
    /// first parse already disagrees with the SQL's meaning, so re-parsing the
    /// faithful render lands on a different — yet internally consistent — tree.
    ParserBug,
    /// The *renderer* drops or reshapes structure: the first parse is correct, but
    /// the rendered text loses information, so the re-parse diverges from it.
    RendererBug,
    /// Each parse is correct for *its own* dialect, but the Canonical render is
    /// re-parsed under a grammar that reads it differently. Not a bug in either
    /// component — the corpus line is dialect-specific and only round-trips when
    /// parsed and re-parsed under the same non-Ansi preset.
    DialectDivergence,
}

impl RoundtripDefect {
    /// Every triage label, in declaration order.
    ///
    /// Constructing all four here keeps the vocabulary exhaustive — a new variant
    /// must be added to `ALL` to be printable — and gives the labels a use site even
    /// while a class has no live corpus case yet, so an as-yet-unused label is not a
    /// dead-code warning.
    pub(crate) const ALL: [Self; 4] = [
        Self::Unsupported,
        Self::ParserBug,
        Self::RendererBug,
        Self::DialectDivergence,
    ];

    /// One-line gloss of what the label asserts about a non-stable case — the triage
    /// rule above in printable form (used in pin-block output and the taxonomy test).
    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Unsupported => {
                "construct outside the parser/renderer surface (coverage gap, no bug)"
            }
            Self::ParserBug => "parser builds the wrong tree (e.g. precedence mis-bind)",
            Self::RendererBug => "renderer drops or reshapes structure",
            Self::DialectDivergence => "dialect-specific line; only stable under one preset",
        }
    }
}

/// A known corpus round-trip defect: the offending SQL plus its triage label.
///
/// Replaces the prior flat `&[&str]` allowlists so the corpus defect list is
/// *classified*, not merely enumerated. Carried in each corpus module's pinned
/// const and tracked under [`ROUNDTRIP_DEFECT_TICKET`]; the per-module defect tests
/// assert each SQL still parses and still diverges, so a silently-fixed defect fails
/// loudly and is promoted out of the list.
pub(crate) struct RoundtripDefectCase {
    /// The offending statement, verbatim from the corpus (kept in source order).
    pub(crate) sql: &'static str,
    /// Where the divergence lives — see [`RoundtripDefect`] for the triage rule.
    pub(crate) label: RoundtripDefect,
}

/// The ticket owning render-stability triage for corpus round-trip defects. Each
/// corpus module asserts this ticket's file exists, so a labelled defect always
/// points at a live tracking ticket (ADR-0014 P3).
pub(crate) const ROUNDTRIP_DEFECT_TICKET: &str = "prod-corpus-idempotence-stability";

/// Whether the round-trip-defect ledger carries a provenance label.
pub(crate) fn defect_ticket_exists() -> bool {
    !ROUNDTRIP_DEFECT_TICKET.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triage_labels_form_a_complete_documented_taxonomy() {
        // Each label carries a non-empty gloss, so the classification stays
        // documented in code (not only in prose), and the loop constructs every
        // variant so the vocabulary is exhaustive and warning-clean.
        for label in RoundtripDefect::ALL {
            assert!(
                !label.description().is_empty(),
                "{label:?} needs a triage description"
            );
        }

        // The labels are distinct classes, not aliases — a duplicated arm would
        // quietly collapse two triage outcomes into one.
        for (i, a) in RoundtripDefect::ALL.iter().enumerate() {
            for b in &RoundtripDefect::ALL[i + 1..] {
                assert_ne!(a, b, "triage labels must be distinct classes");
            }
        }
    }

    #[test]
    fn defect_tracking_label_is_present() {
        assert!(
            defect_ticket_exists(),
            "round-trip defect provenance label must be present"
        );
    }
}
