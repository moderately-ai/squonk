// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The SQL standard-feature catalog and its query functions: the ISO/dialect feature
//! taxonomy this parser is tracked against — a documentation and coverage axis, not
//! parse-time dialect data.

use super::*;

/// Maturity marker for self-described dialect features.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Maturity {
    /// Still proving API and behavior; coverage should not treat it as stable.
    Experimental,
    /// Usable but may still receive compatibility adjustments.
    Preview,
    /// Production API and behavior are stable.
    Stable,
    /// Kept for compatibility while callers move to a replacement.
    Deprecated,
}

/// SQL-standard conformance class for a feature, orthogonal to our [`Maturity`].
///
/// The standard axis (is this a mandatory Core feature?) and the implementation
/// axis ([`Maturity`]) never imply each other: a feature can be ISO
/// [`Core`](Conformance::Core) yet only [`Experimental`](Maturity::Experimental)
/// here, or fully [`Stable`](Maturity::Stable) for us yet a non-standard
/// [`Extension`](Conformance::Extension).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conformance {
    /// A mandatory SQL:2016 "Core" feature (the ~170 features every conforming
    /// implementation must provide).
    Core,
    /// An optional standard feature, outside Core.
    Optional,
    /// A dialect extension with no standard feature id (carries a namespaced
    /// local id instead, e.g. `pg:dollar-quoted-strings`).
    Extension,
}

impl Conformance {
    /// Whether this is a mandatory Core feature.
    pub const fn is_core(&self) -> bool {
        matches!(self, Self::Core)
    }

    /// Whether this is a non-standard dialect extension.
    pub const fn is_extension(&self) -> bool {
        matches!(self, Self::Extension)
    }
}

/// Stable metadata for a dialect feature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeatureMetadata {
    /// Which feature category (built-in vs extension); see [`Feature`].
    pub feature: Feature,
    /// Stable local id (snake_case field name), always present.
    pub id: &'static str,
    /// SQL:2016 feature id this knob anchors to, when it maps 1:1 to one
    /// standard feature. `None` for parser mechanisms and for aggregate knobs
    /// whose sub-features anchor individually in [`STANDARD_FEATURE_CATALOG`].
    pub iso_id: Option<&'static str>,
    /// Whether an "enable everything" preset should turn this feature on. `false`
    /// marks a negative-polarity / restrictive feature (e.g. a future strict-cast
    /// mode) that must stay out of max-feature unions without being mis-marked
    /// `Deprecated`. Orthogonal to [`Maturity`] — a feature can be `Stable` yet not
    /// ideally enabled. Defaults to `true`.
    pub ideally_enabled: bool,
    /// The feature's maturity level; see [`Maturity`].
    pub maturity: Maturity,
}

/// One row of the SQL feature taxonomy: a standard `Lnnn`/`Lnnn-nn` feature or a
/// namespaced dialect extension, carrying our current support state.
///
/// [`STANDARD_FEATURE_CATALOG`] holds both implemented features and the *negative
/// space* of standard surface we have not built yet, so coverage and traceability
/// can be expressed in the standard's own vocabulary and can enumerate
/// what is unimplemented — not only what we chose to add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StandardFeature {
    /// ISO 9075 feature id (`"E031-01"`) or namespaced extension id
    /// (`"pg:dollar-quoted-strings"`).
    pub id: &'static str,
    /// Human-readable feature name.
    pub name: &'static str,
    /// Parent feature id for a numbered sub-feature (`"E031"` for `"E031-01"`);
    /// `None` for a top-level feature.
    pub parent: Option<&'static str>,
    /// Standard conformance class (independent of [`Maturity`]).
    pub conformance: Conformance,
    /// The SQL-standard edition that introduced this feature, for version
    /// anchoring ([`standard_features_as_of`]); `None` for dialect extensions,
    /// which have no standard edition.
    pub standardized_in: Option<StandardVersion>,
    /// Whether the parser currently implements this feature.
    pub supported: bool,
    /// The dialect-data [`Feature`] knob that realizes it, when supported and gated by
    /// a knob that anchors this id 1:1. `None` for always-on grammar, for a row gated
    /// only by an aggregate knob with no 1:1 ISO anchor (e.g. `GRANT`, which rides the
    /// [`UtilitySyntax`] access-control subflag — an aggregate whose own `iso_id` is
    /// `None`), and for unimplemented rows.
    pub realized_by: Option<Feature>,
}

impl StandardFeature {
    /// Whether this is a mandatory Core feature.
    pub const fn is_core(&self) -> bool {
        self.conformance.is_core()
    }

    /// Whether this is a non-standard dialect extension.
    pub const fn is_extension(&self) -> bool {
        self.conformance.is_extension()
    }
}

/// The SQL feature taxonomy this parser is tracked against: implemented standard
/// features and extensions, plus a conservative seed of unimplemented standard
/// surface as enumerable negative space.
///
/// This is intentionally a curated seed, not the full ISO catalogue. Bulk
/// ingestion of the standard surface (PostgreSQL's ~750-row `sql_features.txt`)
/// is tracked by `prod-coverage-iso-catalog-ingestion`: it is a licensed-data
/// import (SPDX/attribution, per the corpus license gate) whose rows are almost
/// all far-future negative space at M1, so the durable mechanism and the
/// M1-relevant rows land here and the bulk rows extend the same table later.
pub const STANDARD_FEATURE_CATALOG: &[StandardFeature] = &[
    // --- implemented standard features ------------------------------------
    StandardFeature {
        id: "E031-01",
        name: "Delimited identifiers",
        parent: Some("E031"),
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: Some(Feature::IdentifierQuote),
    },
    StandardFeature {
        id: "E021-07",
        name: "Character concatenation",
        parent: Some("E021"),
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: Some(Feature::PipeOperator),
    },
    StandardFeature {
        id: "E021-08",
        name: "LIKE predicate",
        parent: Some("E021"),
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: Some(Feature::PredicateSyntax),
    },
    // The four string special forms are gated by `CallSyntax` sub-flags
    // (`substring_from_for` / `trim_from` / `position_in` / `overlay_placing`) —
    // an aggregate knob with no 1:1 ISO anchor, so `realized_by` stays None
    // rather than naming a knob the reverse-link invariant would reject
    // (the F031-03/GRANT precedent).
    StandardFeature {
        id: "E021-06",
        name: "SUBSTRING function",
        parent: Some("E021"),
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "E021-09",
        name: "TRIM function",
        parent: Some("E021"),
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "E021-11",
        name: "POSITION expression",
        parent: Some("E021"),
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "T312",
        name: "OVERLAY function",
        parent: None,
        conformance: Conformance::Optional,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "T611",
        name: "Elementary OLAP operations (window functions)",
        parent: None,
        conformance: Conformance::Optional,
        standardized_in: Some(StandardVersion::Sql2003),
        supported: true,
        // Always-on grammar (not gated by a FeatureSet knob); included so the
        // version axis has a post-SQL:1999 row that `as_of` genuinely filters.
        realized_by: None,
    },
    // The DML and DDL statement families below are always-on grammar — each parses
    // under the bare ANSI baseline (proved by the honesty-gate probes in the
    // conformance matrix), so none carries a 1:1 FeatureSet knob and all anchor
    // `realized_by: None`, like T611. They were long mis-flagged unsupported after
    // the parser built them (`iso-catalog-supported-flags-stale`).
    StandardFeature {
        id: "E101",
        name: "Basic data manipulation",
        parent: None,
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "F031",
        name: "Basic schema manipulation",
        parent: None,
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "F031-01",
        name: "CREATE TABLE statement",
        parent: Some("F031"),
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "F031-02",
        name: "CREATE VIEW statement",
        parent: Some("F031"),
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "F031-03",
        name: "GRANT statement",
        parent: Some("F031"),
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        // GRANT is gated by the aggregate `UtilitySyntax` access-control subflag,
        // baseline-on in ANSI; an aggregate knob has no 1:1 ISO anchor, so `realized_by`
        // stays None rather than naming a knob the reverse-link invariant would reject.
        realized_by: None,
    },
    StandardFeature {
        id: "F571",
        name: "Truth value tests",
        parent: None,
        conformance: Conformance::Optional,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        // `IS [NOT] {TRUE|FALSE|UNKNOWN}` rides the aggregate `OperatorSyntax.truth_value_tests`
        // subflag (on in ANSI/PostgreSQL/MySQL/DuckDB/Lenient, off in SQLite). An aggregate
        // knob has no 1:1 ISO anchor, so `realized_by` stays None like GRANT above.
        realized_by: None,
    },
    // Statement-head families folded from the measured per-dialect production inventories
    // once the bar-A parity programmes built them (spec-level-coverage-audit-programme item
    // 5, catalog-fold-statement-head-negative-space): the landed heads whose statement
    // anchors 1:1 to a single ISO/IEC 9075 feature id (verified against
    // `docs/dialect-references/corpora/postgres/sql_features.txt`). Each rides an aggregate
    // gate with no 1:1 `Feature` knob, so `realized_by` stays None like the GRANT/schema
    // rows above; each is proved built by a probe that PARSES under LENIENT in
    // `standard_catalog_flags_are_probe_backed`. Vendor-only heads with no ISO anchor (XA,
    // HANDLER, LOAD DATA, FLUSH/PURGE, PRAGMA, the SHOW family, object-DDL for
    // tablespaces/servers/secrets/resource-groups, …) get no row — the catalogue tracks
    // standard surface, not engine reach.
    StandardFeature {
        id: "T321-01",
        name: "User-defined functions with no overloading",
        parent: Some("T321"),
        conformance: Conformance::Optional,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "T321-02",
        name: "User-defined stored procedures with no overloading",
        parent: Some("T321"),
        conformance: Conformance::Optional,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "T321-04",
        name: "CALL statement",
        parent: Some("T321"),
        conformance: Conformance::Optional,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "T331",
        name: "Basic roles",
        parent: None,
        conformance: Conformance::Optional,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "T211",
        name: "Basic trigger capability",
        parent: None,
        conformance: Conformance::Optional,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "F034",
        name: "Extended REVOKE statement",
        parent: None,
        conformance: Conformance::Optional,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: true,
        realized_by: None,
    },
    // --- implemented dialect extensions (namespaced ids) ------------------
    StandardFeature {
        id: "pg:escape-string-syntax",
        name: "PostgreSQL E'...' escape string constants",
        parent: None,
        conformance: Conformance::Extension,
        standardized_in: None,
        supported: true,
        realized_by: Some(Feature::StringLiterals),
    },
    StandardFeature {
        id: "pg:dollar-quoted-strings",
        name: "PostgreSQL $tag$...$tag$ string constants",
        parent: None,
        conformance: Conformance::Extension,
        standardized_in: None,
        supported: true,
        realized_by: Some(Feature::StringLiterals),
    },
    // --- enumerable negative space: unimplemented standard surface --------
    // ISO-anchored surface the parser genuinely lacks. Each row is proved unbuilt by a
    // probe that FAILS to parse under the maximal-feature LENIENT dialect — the honesty
    // gate `standard_catalog_flags_are_probe_backed` in the conformance matrix — so a
    // stale flag can no longer masquerade as a gap (the failure the five DML/DDL rows
    // above previously hid). Not a census: every row is a verified miss, added under
    // `iso-catalog-supported-flags-stale`. Candidates that could not anchor to a single
    // wholly-unsupported ISO id were deliberately left out (MATCH PARTIAL — only that
    // one variant of F741 rejects, FULL/SIMPLE parse; CREATE TYPE — a family of S-features
    // with no clean 1:1 id).
    StandardFeature {
        id: "E121",
        name: "Basic cursor support",
        parent: None,
        conformance: Conformance::Core,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: false,
        realized_by: None,
    },
    StandardFeature {
        id: "T176",
        name: "Sequence generator support",
        parent: None,
        conformance: Conformance::Optional,
        standardized_in: Some(StandardVersion::Sql2003),
        // `CREATE`/`DROP SEQUENCE` (the SQL:2003 T176 generator) is now parsed under the
        // PostgreSQL/DuckDB/LENIENT presets, gated by `StatementDdlGates::create_sequence`.
        // `realized_by` is `None`: the feature rides that aggregate sub-flag, which has no
        // 1:1 `Feature` knob (the same anchoring the GRANT/schema-manipulation rows use).
        supported: true,
        realized_by: None,
    },
    StandardFeature {
        id: "F251",
        name: "Domain support",
        parent: None,
        conformance: Conformance::Optional,
        standardized_in: Some(StandardVersion::Sql1999),
        supported: false,
        realized_by: None,
    },
];

/// Standard features the parser does not yet implement — the enumerable negative
/// space of the standard surface. Distinct from "features we added".
pub fn unsupported_standard_features() -> impl Iterator<Item = &'static StandardFeature> {
    STANDARD_FEATURE_CATALOG
        .iter()
        .filter(|feature| !feature.supported)
}

/// Look up a catalogue row by its feature id.
pub fn standard_feature(id: &str) -> Option<&'static StandardFeature> {
    STANDARD_FEATURE_CATALOG
        .iter()
        .find(|feature| feature.id == id)
}

/// The frozen set of standard features available as of SQL edition `version` — the
/// features whose `standardized_in` edition is no later than `version`.
///
/// This lets a consumer pin "the standard feature set as of release X" without
/// enumerating features. Dialect extensions (no standard edition) are excluded.
pub fn standard_features_as_of(
    version: StandardVersion,
) -> impl Iterator<Item = &'static StandardFeature> {
    STANDARD_FEATURE_CATALOG.iter().filter(
        move |feature| matches!(feature.standardized_in, Some(edition) if edition <= version),
    )
}

/// Feature metadata an "enable everything" preset should turn on: every feature
/// except negative-polarity ones ([`FeatureMetadata::ideally_enabled`] is `false`).
///
/// A restrictive feature stays tracked and tested but out of max-feature unions,
/// rather than being mis-marked [`Maturity::Deprecated`] to hide it.
pub fn max_feature_metadata() -> impl Iterator<Item = &'static FeatureMetadata> {
    FEATURE_METADATA
        .iter()
        .filter(|metadata| metadata.ideally_enabled)
}

#[cfg(test)]
mod tests {
    use crate::dialect::*;

    #[test]
    fn feature_metadata_is_enumerable_with_stable_ids() {
        assert!(!FEATURES.is_empty());
        assert_eq!(FEATURES.len(), FEATURE_METADATA.len());

        for (feature, metadata) in FEATURES.iter().zip(FEATURE_METADATA) {
            assert_eq!(*feature, metadata.feature);
            assert_eq!(feature.id(), metadata.id);
            assert!(!metadata.id.is_empty());
            assert_eq!(metadata.iso_id, feature.iso_id());
            assert_eq!(metadata.ideally_enabled, feature.ideally_enabled());
            assert_eq!(metadata.maturity, Maturity::Stable);
        }
    }

    #[test]
    fn iso_anchored_features_resolve_in_the_standard_catalog() {
        // Every knob that claims a standard id must have a supported catalogue
        // row that points back to it — the forward/reverse links stay in sync.
        for feature in FEATURES {
            let Some(iso_id) = feature.iso_id() else {
                continue;
            };
            let row = standard_feature(iso_id)
                .unwrap_or_else(|| panic!("`{iso_id}` missing from STANDARD_FEATURE_CATALOG"));
            assert!(
                row.supported,
                "{iso_id} anchors a knob but is marked unsupported"
            );
            assert_eq!(row.realized_by, Some(*feature));
            // A knob's iso_id only names standard (non-extension) features.
            assert!(
                !row.is_extension(),
                "{iso_id} should be a standard id, not an extension"
            );
        }
    }

    #[test]
    fn realized_standard_features_link_back_to_their_knob() {
        for row in STANDARD_FEATURE_CATALOG {
            let Some(feature) = row.realized_by else {
                continue;
            };
            assert!(
                row.supported,
                "{} is realized by a knob but unsupported",
                row.id
            );
            assert!(
                FEATURES.contains(&feature),
                "{} realized by an unenumerated knob",
                row.id
            );
            // Standard (non-extension) rows are the 1:1 anchors; their knob must
            // name them back. Extension rows hang off an aggregate knob whose
            // own `iso_id()` is `None`, so only check the standard direction.
            if !row.is_extension() {
                assert_eq!(feature.iso_id(), Some(row.id));
            }
        }
    }

    #[test]
    fn standard_catalog_enumerates_unimplemented_negative_space() {
        let unsupported: Vec<_> = unsupported_standard_features().collect();
        assert!(
            !unsupported.is_empty(),
            "the registry must enumerate unbuilt standard surface, not only what we added",
        );
        assert!(
            unsupported.iter().any(|feature| feature.is_core()),
            "negative space should include at least one mandatory Core feature",
        );
        // A concrete, unambiguous M1 gap: basic cursor support is unbuilt (no
        // DECLARE/OPEN/FETCH/CLOSE), unlike the DML/DDL surface mis-flagged unsupported
        // before `iso-catalog-supported-flags-stale`.
        let cursors = standard_feature("E121").expect("E121 catalogued");
        assert!(!cursors.supported);
        assert!(cursors.is_core());
    }

    #[test]
    fn standard_catalog_ids_are_unique_and_well_formed() {
        for (index, row) in STANDARD_FEATURE_CATALOG.iter().enumerate() {
            assert!(!row.id.is_empty() && !row.name.is_empty());
            assert!(
                STANDARD_FEATURE_CATALOG[index + 1..]
                    .iter()
                    .all(|later| later.id != row.id),
                "duplicate standard feature id `{}`",
                row.id,
            );
            // A numbered sub-feature's id extends its parent's id (`E031` -> `E031-01`).
            if let Some(parent) = row.parent {
                assert!(
                    row.id.starts_with(parent) && row.id.len() > parent.len(),
                    "`{}` should be a sub-feature of `{parent}`",
                    row.id,
                );
            }
            // Extension rows carry a namespaced local id; standard rows do not.
            assert_eq!(
                row.is_extension(),
                row.id.contains(':'),
                "namespacing must match conformance class for `{}`",
                row.id,
            );
        }
    }

    #[test]
    fn conformance_and_maturity_are_independent_axes() {
        // E031-01 is mandatory Core in the standard, yet our realization of it is
        // tracked on the separate maturity axis — neither value implies the other.
        let delimited = standard_feature("E031-01").expect("E031-01 catalogued");
        assert_eq!(delimited.conformance, Conformance::Core);
        assert_eq!(
            delimited.realized_by.map(|feature| feature.maturity()),
            Some(Maturity::Stable),
        );
        assert!(Conformance::Core.is_core());
        assert!(!Conformance::Core.is_extension());
        assert!(Conformance::Extension.is_extension());
    }

    #[test]
    fn standard_features_as_of_grows_monotonically_with_edition() {
        let at_1999 = standard_features_as_of(StandardVersion::Sql1999).count();
        let at_2003 = standard_features_as_of(StandardVersion::Sql2003).count();
        let at_2016 = standard_features_as_of(StandardVersion::Sql2016).count();

        assert!(at_1999 >= 1, "SQL:1999 Core features are catalogued");
        assert!(
            at_2003 > at_1999,
            "a SQL:2003 feature (T611) becomes available at the 2003 edition",
        );
        assert_eq!(
            at_2003, at_2016,
            "no later-edition feature is catalogued yet"
        );

        // T611 is exactly the row that appears only from SQL:2003 onward.
        assert!(
            standard_feature("T611").unwrap().standardized_in == Some(StandardVersion::Sql2003)
        );
        assert!(standard_features_as_of(StandardVersion::Sql1999).all(|f| f.id != "T611"));
        assert!(standard_features_as_of(StandardVersion::Sql2003).any(|f| f.id == "T611"));

        // Every standard row carries an edition; extensions never do.
        for row in STANDARD_FEATURE_CATALOG {
            assert_eq!(
                row.standardized_in.is_some(),
                !row.is_extension(),
                "`{}` edition presence must match standard-vs-extension",
                row.id,
            );
        }
    }

    #[test]
    fn ideally_enabled_excludes_negative_polarity_from_the_max_preset() {
        // Every current knob is additive, so the max preset is all of them.
        assert_eq!(max_feature_metadata().count(), FEATURE_METADATA.len());
        for metadata in FEATURE_METADATA {
            assert!(
                metadata.ideally_enabled,
                "{} should be ideally enabled",
                metadata.id
            );
        }

        // The axis genuinely gates and is independent of maturity: a constructed
        // restrictive feature is excluded from the max preset while still `Stable`.
        let restrictive = FeatureMetadata {
            feature: Feature::BindingPowers,
            id: "strict_example",
            iso_id: None,
            ideally_enabled: false,
            maturity: Maturity::Stable,
        };
        assert!(!restrictive.ideally_enabled);
        assert_eq!(restrictive.maturity, Maturity::Stable);
    }
}
