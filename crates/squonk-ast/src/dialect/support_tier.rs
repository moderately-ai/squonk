// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The release-contract support tier for a shipped dialect preset or optional
//! product surface, and the named source of truth that backs each stable claim.
//!
//! This is a distinct axis from the per-feature [`Maturity`](super::Maturity).
//! `Maturity` answers "is this one `FeatureSet` knob's API stable?"; every knob is
//! `Stable` because the knob *vocabulary* is frozen. [`SupportTier`] answers the
//! separate, coarser question the stable release actually turns on: "how strong is
//! the parity evidence for this whole preset (or product surface), and may we
//! advertise it as production-ready?" A preset can be built entirely from `Stable`
//! knobs yet only be `Experimental` because no engine oracle has ever checked it.
//!
//! The value types live here (a pure, reusable metadata axis beside `Maturity`);
//! the per-preset and per-surface *assignment* — which references `BuiltinDialect`
//! and the shipped product features — lives in the `squonk` crate, which owns
//! that identity. The single invariant this module encodes is
//! [`SupportEvidence::is_authoritative`]: a [`SupportTier::Stable`] claim must cite
//! an authoritative source, enforced by the release-tier gate in the conformance
//! crate.

/// How strong the parity evidence is for a shipped dialect preset or product
/// surface, and therefore what the stable release may promise about it.
///
/// Ordered weakest-to-strongest so `<`/`>=` compare promise levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(feature = "serde-serialize", serde(rename_all = "snake_case"))]
pub enum SupportTier {
    /// Documentation-derived only: modelled from vendor docs or the dialect-reference
    /// library with no differential oracle wired. Behaviour may diverge from the real
    /// engine and can change without notice. Not a production parity claim.
    Experimental,
    /// Usable and API-stable, but not a full engine-parity guarantee: either a
    /// differential/comparison oracle is wired at partial coverage (or is not yet in
    /// the default gate), or the preset is constructed to not match any single engine
    /// by design (the permissive union, the formatter, the not-yet-distributed
    /// bindings).
    Preview,
    /// Production-ready: backed by an authoritative source of truth
    /// ([`SupportEvidence::is_authoritative`]) and held to it by an enforced gate.
    Stable,
}

impl SupportTier {
    /// Stable machine-readable id (the serialized spelling), for tables and bindings.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Experimental => "experimental",
            Self::Preview => "preview",
            Self::Stable => "stable",
        }
    }
}

/// The named source of truth backing a preset's or surface's parity claim.
///
/// Only the [authoritative](Self::is_authoritative) variants may back a
/// [`SupportTier::Stable`] claim. The others document genuinely weaker evidence and
/// cap a preset at [`Preview`](SupportTier::Preview) or
/// [`Experimental`](SupportTier::Experimental) — they exist so a non-stable tier
/// still carries an honest, machine-readable reason rather than an empty gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde-serialize", derive(serde::Serialize))]
#[cfg_attr(
    feature = "serde-serialize",
    serde(tag = "kind", rename_all = "snake_case")
)]
pub enum SupportEvidence {
    /// Differential parity against a real engine or its reference parser — the
    /// engine's own accept/reject (and, where available, bind) verdict is the oracle.
    /// The only dialect-parity evidence that licenses `Stable`.
    EngineDifferential {
        /// The oracle engine or reference parser (e.g. `"mysql"`, `"libpg_query"`).
        engine: &'static str,
        /// The pinned engine/library version the parity is measured against.
        version: &'static str,
        /// How the verdict is obtained (e.g. `"live server prepare + parse"`,
        /// `"in-process EXPLAIN AST (partial modelled surface)"`).
        method: &'static str,
    },
    /// Validated against the SQL-standard text itself plus the parser's enforced
    /// structural round-trip property and curated accept corpus — the authoritative
    /// backing for the standard baseline, which has no single vendor engine.
    StandardReference {
        /// What the reference is (the standard edition plus the enforcing properties).
        note: &'static str,
    },
    /// A versioned, frozen, drift-gated contract artifact — the authoritative backing
    /// for a product surface (e.g. the serialized wire schema), which is not a dialect
    /// and has no engine oracle.
    ContractGate {
        /// The frozen artifact path (e.g. `"release/schema/wire-schema.v1.json"`).
        artifact: &'static str,
        /// How the contract is held (e.g. the drift gate and frozen baseline).
        note: &'static str,
    },
    /// Cross-checked against a non-authoritative multi-dialect parser (e.g. sqlglot):
    /// a modelled surface, never engine truth — its own gaps become false divergences.
    /// Weaker than an engine; caps below `Stable`.
    Comparison {
        /// The comparison parser (e.g. `"sqlglot"`).
        tool: &'static str,
        /// Scope and non-authority caveat.
        note: &'static str,
    },
    /// Derived from vendor documentation / the dialect-reference manifest only, with no
    /// differential oracle (typically because engine acquisition is blocked).
    DocumentationDerived {
        /// The documentation basis and the blocker, when one is tracked.
        note: &'static str,
    },
    /// A preset constructed to not match any single engine by design — the permissive
    /// parse-anything union, or a product surface whose contract is not engine parity.
    Constructed {
        /// Why the surface is constructed and what its real contract is.
        note: &'static str,
    },
}

impl SupportEvidence {
    /// Whether this source authoritatively backs a [`SupportTier::Stable`] claim.
    ///
    /// The release-tier gate asserts `tier == Stable` implies this is `true`, so a
    /// documentation-derived, comparison-only, or constructed preset can never be
    /// advertised as stable. It is deliberately one-directional: authoritative
    /// evidence does *not* force `Stable` (a real engine oracle wired at only partial
    /// coverage stays `Preview` — e.g. ClickHouse).
    pub const fn is_authoritative(self) -> bool {
        matches!(
            self,
            Self::EngineDifferential { .. }
                | Self::StandardReference { .. }
                | Self::ContractGate { .. }
        )
    }
}
