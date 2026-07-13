// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Coverage harness primitives (ADR-0015): the [`Probe`]/[`Coverage`] model, [`CoverageCase`],
//! the parse/tokenize/render confirmation [`confirm_case`], the shared [`sole_projection_expr`] /
//! [`query_body`] extractors, [`accepts_under`], and [`AdHocDialect`] — the runtime flip-harness that
//! parses under a `FeatureSet` computed at runtime, and the prior art for the `declare_dialect!` research.

use super::cases::*;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Polarity {
    Positive,
    Negative,
}

/// One harness-run observation backing a behaviour coverage case. The coverage harness
/// itself runs the parse/tokenize/render and checks the recorded outcome, so a case
/// built from probes carries objective behaviour *by construction* — its kind is derived
/// from the run, never a hand-set tag. This is what makes the objectivity gate
/// tag-independent (see [`has_objective_behavior`]).
#[derive(Clone, Copy)]
pub(crate) enum Probe {
    /// `parse_with(sql, squonk::ParseConfig::new(AdHocDialect(features)))` must accept.
    ParseAccepts {
        sql: &'static str,
        features: &'static FeatureSet,
    },
    /// `parse_with(sql, squonk::ParseConfig::new(AdHocDialect(features)))` must reject.
    ParseRejects {
        sql: &'static str,
        features: &'static FeatureSet,
    },
    /// `parse_with(sql, squonk::ParseConfig::new(AdHocDialect(features)))` must accept and its tree match `shape`.
    /// The structural half of a divergence where both feature settings parse but the
    /// parse *shape* differs (e.g. binding-power or set-operation reassociation).
    ParseShape {
        sql: &'static str,
        features: &'static FeatureSet,
        shape: fn(&Parsed) -> bool,
    },
    /// `tokenize_with(sql, features)` must succeed and its tokens match `tokens`.
    TokenShape {
        sql: &'static str,
        features: &'static FeatureSet,
        tokens: fn(&[Token]) -> bool,
    },
    /// `tokenize_with(sql, features)` must fail.
    TokenRejects {
        sql: &'static str,
        features: &'static FeatureSet,
    },
    /// Rendering `sql` to `target`'s preferred spellings must satisfy `text` — the
    /// `RenderSpelling::TargetDialect` path the `target_spelling` field drives.
    RenderText {
        sql: &'static str,
        target: &'static FeatureSet,
        text: fn(&str) -> bool,
    },
}

impl Probe {
    /// Run this probe through the harness and report whether its recorded outcome held.
    /// This is the function that makes a behaviour case's kind *harness-derived*: a probe
    /// whose SQL does not actually produce the recorded parse/tokenize/render outcome
    /// returns `false`, which fails [`confirm_case`] (and thus the executable gate).
    fn holds(self) -> bool {
        match self {
            Probe::ParseAccepts { sql, features } => {
                parse_with(sql, squonk::ParseConfig::new(AdHocDialect(features))).is_ok()
            }
            Probe::ParseRejects { sql, features } => {
                parse_with(sql, squonk::ParseConfig::new(AdHocDialect(features))).is_err()
            }
            Probe::ParseShape {
                sql,
                features,
                shape,
            } => {
                matches!(parse_with(sql, squonk::ParseConfig::new(AdHocDialect(features))), Ok(parsed) if shape(&parsed))
            }
            Probe::TokenShape {
                sql,
                features,
                tokens,
            } => matches!(tokenize_with(sql, features), Ok(stream) if tokens(&stream)),
            Probe::TokenRejects { sql, features } => tokenize_with(sql, features).is_err(),
            Probe::RenderText { sql, target, text } => text(&render_to_target(sql, target)),
        }
    }
}

/// How a feature's coverage is established, and whether it satisfies the objectivity
/// gate. The variant *is* the kind — there is no separate hand-set tag to mislabel.
#[derive(Clone, Copy)]
pub(crate) enum Coverage {
    /// A non-harness metadata diagnostic (e.g. two [`FeatureSet`]s compared unequal).
    /// Useful as a diagnostic, but does NOT satisfy the objectivity gate: ADR-0015
    /// requires real behaviour, not a divergence assertion.
    Metadata(fn()),
    /// Behaviour observed by the harness itself running every [`Probe`] and confirming
    /// its recorded outcome (`coverage_cases_are_executable` via [`confirm_case`]). The
    /// kind is *derived* from the run — a case that does not actually exercise the
    /// harness with its recorded outcome fails confirmation — so this cannot be faked by
    /// a mislabelled metadata assert. Satisfies the gate.
    Behavior(&'static [Probe]),
    /// The documented semantic-default escape hatch, and the *only* residual trust
    /// surface the gate still accepts on a hand-set basis. `DefaultNullOrdering` (a
    /// downstream semantic default) and `IdentifierCasing` (observable only as
    /// folded-identifier text, not an accept/reject or structural *parse* differential)
    /// have no parse/tokenize/render observation the harness could derive a kind from, so
    /// these two — and, enforced by
    /// `semantic_escape_hatch_is_limited_to_the_two_documented_features`, only these two —
    /// assert their semantic default directly. Satisfies the gate.
    SemanticDefault(fn()),
}

impl Coverage {
    /// Whether this coverage counts as objective behaviour for the gate: a harness-run
    /// [`Coverage::Behavior`] (kind derived from the run) or the documented
    /// [`Coverage::SemanticDefault`] escape hatch. [`Coverage::Metadata`] does not.
    pub(crate) fn is_objective_behavior(&self) -> bool {
        matches!(self, Coverage::Behavior(_) | Coverage::SemanticDefault(_))
    }
}

pub(crate) struct CoverageCase {
    pub(crate) feature: Feature,
    pub(crate) polarity: Polarity,
    pub(crate) name: &'static str,
    pub(crate) coverage: Coverage,
}

pub(crate) fn sole_projection_expr(parsed: &squonk::Parsed) -> &Expr<NoExt> {
    let [Statement::Query { query, .. }] = parsed.statements() else {
        panic!("expected one query statement");
    };
    let SetExpr::Select { select, .. } = &query.body else {
        panic!("expected a SELECT body");
    };
    let [
        SelectItem::Expr {
            expr, alias: None, ..
        },
    ] = select.projection.as_slice()
    else {
        panic!("expected one unaliased projection expression");
    };
    expr
}

pub(crate) fn query_body(parsed: &squonk::Parsed) -> &SetExpr<NoExt> {
    let [Statement::Query { query, .. }] = parsed.statements() else {
        panic!("expected one query statement");
    };
    &query.body
}

/// Run one coverage case and confirm it. Metadata and semantic-default cases execute
/// their assert; a [`Coverage::Behavior`] case has every [`Probe`] run through the
/// harness and each recorded outcome confirmed. Returns `Err` for a behaviour case that
/// runs no probe, or whose probe did not reproduce its recorded harness outcome — the
/// harness-derived-kind check that makes a mislabelled behaviour case fail.
/// `behavior_kind_is_harness_derived` drives this same function over synthetic
/// mislabelled cases to prove the teeth.
pub(crate) fn confirm_case(case: &CoverageCase) -> Result<(), String> {
    match case.coverage {
        Coverage::Metadata(assert) | Coverage::SemanticDefault(assert) => {
            assert();
            Ok(())
        }
        Coverage::Behavior(probes) => {
            if probes.is_empty() {
                return Err(format!(
                    "behaviour case `{}` runs no harness probe, so its kind cannot be confirmed",
                    case.name,
                ));
            }
            for (index, probe) in probes.iter().enumerate() {
                if !probe.holds() {
                    return Err(format!(
                        "behaviour case `{}` probe #{index} did not reproduce its recorded \
                         harness outcome",
                        case.name,
                    ));
                }
            }
            Ok(())
        }
    }
}

/// A dialect that hands back a borrowed `FeatureSet`, so a case can parse under a
/// `FeatureSet` computed at runtime (toggling individual sub-flags) without a unit
/// struct per variant. The borrow may be `'static` or a local.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AdHocDialect<'a>(pub(crate) &'a FeatureSet);

impl Dialect for AdHocDialect<'_> {
    type Ext = NoExt;

    fn features(&self) -> &FeatureSet {
        self.0
    }
}

/// Whether the parser accepts `sql` under a runtime `FeatureSet`. The executable half
/// of every label — the differential and round-trip corpora resolve their
/// `required_features` labels against this same parser path.
pub(crate) fn accepts_under(sql: &str, features: &FeatureSet) -> bool {
    parse_with(sql, squonk::ParseConfig::new(AdHocDialect(features))).is_ok()
}
