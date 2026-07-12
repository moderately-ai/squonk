// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The dialect coverage matrix and the M2 differential-oracle matrix: [`DIALECT_PRESETS`],
//! [`feature_enablement`], the deterministic renderers, [`M2_ORACLE_ROWS`], plus the ISO-9075
//! feature-taxonomy hygiene tests.

use super::cases::*;
use super::harness::*;
use super::*;

// --- dialect milestone coverage matrix (prod-coverage-dialect-matrix) ----------
//
// The cases above prove behaviour per feature; this projects them into a
// deterministic, git-diffable matrix that names, for every feature: its
// `Maturity` (sourced from the dialect-data `FeatureMetadata`), how each shipping
// dialect preset treats it (the ANSI baseline vs. where PostgreSQL diverges), and
// the representative positive and negative behaviour cases that cover it. The
// rendered table (pinned by `coverage_matrix_snapshot`) is the planning artifact
// for dispatch and the M2 (SQLite/DuckDB) milestone: adding a preset to
// `DIALECT_PRESETS` adds a column and immediately shows which features that
// dialect must cover. `every_feature_has_positive_and_negative_coverage` stays the
// hard gate — this layer reports the matrix, it does not relax the requirement.

/// Shipping dialect presets the matrix reports as columns, in display order. ANSI
/// is the baseline every other preset is compared against. M2 extends this list
/// with the SQLite/DuckDB presets; growing the matrix needs no other change.
const DIALECT_PRESETS: &[(&str, FeatureSet)] = &[
    ("ansi", FeatureSet::ANSI),
    ("postgres", FeatureSet::POSTGRES),
    ("mysql", FeatureSet::MYSQL),
    ("sqlite", FeatureSet::SQLITE),
    ("duckdb", FeatureSet::DUCKDB),
];

/// How a dialect preset treats one feature, relative to the ANSI baseline. Computed
/// from the presets' `FeatureSet`s (never hand-declared), so a matrix cell cannot
/// drift from the dialect data it describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Enablement {
    /// The preset keeps the ANSI baseline for this feature's knob.
    Baseline,
    /// The preset's knob diverges from ANSI for this feature.
    Diverges,
}

impl Enablement {
    fn id(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Diverges => "diverges",
        }
    }
}

/// Whether `preset` diverges from the ANSI baseline in the single `FeatureSet` knob
/// owned by `feature`. Exhaustive over `Feature` (mirroring
/// `feature_set_fields_are_explicitly_enumerated`), so a new feature must add an arm
/// here too — the dialect axis can never silently miss a knob.
fn feature_enablement(feature: Feature, preset: &FeatureSet) -> Enablement {
    let ansi = &FeatureSet::ANSI;
    let diverges = match feature {
        Feature::IdentifierCasing => preset.identifier_casing != ansi.identifier_casing,
        Feature::IdentifierQuote => preset.identifier_quotes != ansi.identifier_quotes,
        Feature::DefaultNullOrdering => preset.default_null_ordering != ansi.default_null_ordering,
        Feature::ReservedColumnName => preset.reserved_column_name != ansi.reserved_column_name,
        Feature::ReservedFunctionName => {
            preset.reserved_function_name != ansi.reserved_function_name
        }
        Feature::ReservedTypeName => preset.reserved_type_name != ansi.reserved_type_name,
        Feature::ReservedBareAlias => preset.reserved_bare_alias != ansi.reserved_bare_alias,
        Feature::ReservedAsLabel => preset.reserved_as_label != ansi.reserved_as_label,
        Feature::CatalogQualifiedNames => {
            preset.catalog_qualified_names != ansi.catalog_qualified_names
        }
        Feature::ByteClasses => preset.byte_classes != ansi.byte_classes,
        Feature::BindingPowers => preset.binding_powers != ansi.binding_powers,
        Feature::SetOperationPowers => preset.set_operation_powers != ansi.set_operation_powers,
        Feature::StringLiterals => preset.string_literals != ansi.string_literals,
        Feature::NumericLiterals => preset.numeric_literals != ansi.numeric_literals,
        Feature::Parameters => preset.parameters != ansi.parameters,
        Feature::SessionVariables => preset.session_variables != ansi.session_variables,
        Feature::IdentifierSyntax => preset.identifier_syntax != ansi.identifier_syntax,
        Feature::TableExpressions => preset.table_expressions != ansi.table_expressions,
        Feature::JoinSyntax => preset.join_syntax != ansi.join_syntax,
        Feature::TableFactorSyntax => preset.table_factor_syntax != ansi.table_factor_syntax,
        Feature::ExpressionSyntax => preset.expression_syntax != ansi.expression_syntax,
        Feature::OperatorSyntax => preset.operator_syntax != ansi.operator_syntax,
        Feature::CallSyntax => preset.call_syntax != ansi.call_syntax,
        Feature::StringFuncForms => preset.string_func_forms != ansi.string_func_forms,
        Feature::AggregateCallSyntax => preset.aggregate_call_syntax != ansi.aggregate_call_syntax,
        Feature::PredicateSyntax => preset.predicate_syntax != ansi.predicate_syntax,
        Feature::PipeOperator => preset.pipe_operator != ansi.pipe_operator,
        Feature::DoubleAmpersand => preset.double_ampersand != ansi.double_ampersand,
        Feature::KeywordOperators => preset.keyword_operators != ansi.keyword_operators,
        Feature::CaretOperator => preset.caret_operator != ansi.caret_operator,
        Feature::HashBitwiseXor => preset.hash_bitwise_xor != ansi.hash_bitwise_xor,
        Feature::CommentSyntax => preset.comment_syntax != ansi.comment_syntax,
        Feature::MutationSyntax => preset.mutation_syntax != ansi.mutation_syntax,
        Feature::StatementDdlGates => preset.statement_ddl_gates != ansi.statement_ddl_gates,
        Feature::CreateTableClauseSyntax => {
            preset.create_table_clause_syntax != ansi.create_table_clause_syntax
        }
        Feature::ColumnDefinitionSyntax => {
            preset.column_definition_syntax != ansi.column_definition_syntax
        }
        Feature::ConstraintSyntax => preset.constraint_syntax != ansi.constraint_syntax,
        Feature::IndexAlterSyntax => preset.index_alter_syntax != ansi.index_alter_syntax,
        Feature::ExistenceGuards => preset.existence_guards != ansi.existence_guards,
        Feature::SelectSyntax => preset.select_syntax != ansi.select_syntax,
        Feature::QueryTailSyntax => preset.query_tail_syntax != ansi.query_tail_syntax,
        Feature::GroupingSyntax => preset.grouping_syntax != ansi.grouping_syntax,
        Feature::UtilitySyntax => preset.utility_syntax != ansi.utility_syntax,
        Feature::ShowSyntax => preset.show_syntax != ansi.show_syntax,
        Feature::MaintenanceSyntax => preset.maintenance_syntax != ansi.maintenance_syntax,
        Feature::AccessControlSyntax => preset.access_control_syntax != ansi.access_control_syntax,
        Feature::TypeNameSyntax => preset.type_name_syntax != ansi.type_name_syntax,
        Feature::TargetSpelling => preset.target_spelling != ansi.target_spelling,
    };
    if diverges {
        Enablement::Diverges
    } else {
        Enablement::Baseline
    }
}

/// Stable lowercase id for a `Maturity`, for the matrix cell. All M1 features are
/// `Stable`; the other arms are live once a Preview/Experimental knob lands.
fn maturity_id(maturity: Maturity) -> &'static str {
    match maturity {
        Maturity::Experimental => "experimental",
        Maturity::Preview => "preview",
        Maturity::Stable => "stable",
        Maturity::Deprecated => "deprecated",
    }
}

/// The name of the first behaviour case of `polarity` covering `feature`, in
/// `COVERAGE_CASES` order. `every_feature_has_positive_and_negative_coverage`
/// guarantees one exists for every stable feature; the `(none)` fallback keeps the
/// matrix renderable and is asserted absent by `coverage_matrix_is_exhaustive`.
fn representative_case(feature: Feature, polarity: Polarity) -> &'static str {
    COVERAGE_CASES
        .iter()
        .find(|case| {
            case.coverage.is_objective_behavior()
                && case.feature == feature
                && case.polarity == polarity
        })
        .map_or("(none)", |case| case.name)
}

/// Render the dialect milestone coverage matrix as a deterministic, sorted table.
/// Rows follow `Feature::ALL` (the documented stable coverage-matrix order), columns
/// are fixed, and nothing time- or environment-dependent enters the output, so the
/// snapshot is a clean git diff.
fn render_coverage_matrix() -> String {
    let mut header = vec!["feature".to_string(), "maturity".to_string()];
    for (name, _) in DIALECT_PRESETS {
        header.push((*name).to_string());
    }
    header.push("positive case".to_string());
    header.push("negative case".to_string());

    let mut rows = vec![header];
    for feature in Feature::ALL {
        let mut row = vec![
            feature.id().to_string(),
            maturity_id(feature.maturity()).to_string(),
        ];
        for (_, preset) in DIALECT_PRESETS {
            row.push(feature_enablement(feature, preset).id().to_string());
        }
        row.push(representative_case(feature, Polarity::Positive).to_string());
        row.push(representative_case(feature, Polarity::Negative).to_string());
        rows.push(row);
    }

    render_padded_table(&rows)
}

/// Left-pad `rows` (row 0 is the header) into a ` | `-separated table with a `-+-`
/// rule under the header — the shared layout for the dialect and M2 oracle matrices.
fn render_padded_table(rows: &[Vec<String>]) -> String {
    let columns = rows[0].len();
    let widths: Vec<usize> = (0..columns)
        .map(|col| rows.iter().map(|row| row[col].len()).max().unwrap_or(0))
        .collect();

    let mut out = String::new();
    for (index, row) in rows.iter().enumerate() {
        let line = row
            .iter()
            .enumerate()
            .map(|(col, cell)| format!("{cell:<width$}", width = widths[col]))
            .collect::<Vec<_>>()
            .join(" | ");
        out.push_str(line.trim_end());
        out.push('\n');
        if index == 0 {
            let rule = widths
                .iter()
                .map(|width| "-".repeat(*width))
                .collect::<Vec<_>>()
                .join("-+-");
            out.push_str(&rule);
            out.push('\n');
        }
    }
    out
}

// --- M2 differential-oracle milestone (prod-dialect-m2-sqlite-duckdb-oracles) -----
//
// The dialect matrix above is a feature x preset projection, self-attested from each
// preset's `FeatureSet`. M2 adds two real-engine accept/reject oracles (SQLite, DuckDB)
// that check those verdicts against the engine itself, surfaced here as their own small
// metadata table instead of new matrix columns. Each pairs an engine's `prepare()`-bind
// verdict with its fitted shipped `squonk` dialect: SQLite with `Sqlite`
// (`sqlite-featureset-preset`), DuckDB with `DuckDb` (`duckdb-featureset-preset`, which
// also added the `duckdb` matrix column above, replacing the earlier `postgres`
// stopgap). Kept as static metadata so this default-build snapshot needs no system
// engine; `m2::tests` (feature `oracle-engines`) welds these rows to the live oracle
// impls via `oracle_rows_match_coverage_matrix`.
//
// SQLite verdict source — oracle-verified at scale (umbrella definition-of-100% (d),
// `sqlite-oracle-at-scale`). The SQLite column is no longer self-attested: every
// statement of the vendored corpora (1,619) is routed through the in-process
// `SqliteOracle` (`rusqlite`, prepare-only) under the fitted `Sqlite` preset by the
// allowlist-gated `corpus_sqlite_verdicts` gate (the PG-ledger clone), both directions.
// Structural-oracle bound (assessed up front so children don't chase the impossible):
// SQLite has no public parse-tree dump (EXPLAIN emits VDBE bytecode, not an AST), so
// accuracy rests on prepare-parity at scale + round-trip identity + `sqlite3_column_name`
// projection probes; PG-class tree parity is NOT achievable and is not claimed. Caveat:
// `MATCH`/`REGEXP` are grammar-only — their backing functions are unregistered in the
// bundled engine, so a bare `prepare` rejects them (a function-resolution artifact, not
// grammar); `GLOB` (a built-in) is the operator family's oracle-verifiable representative,
// while MATCH/REGEXP are guarded by round-trip rather than accept/reject.

/// One M2 differential-oracle milestone row.
pub(crate) struct M2OracleRow {
    /// Engine identifier, matching `m2::*Oracle`'s `AcceptRejectOracle::name`.
    pub(crate) engine: &'static str,
    /// `OracleSemantics` id — `prepare_bind` for both M2 engines.
    pub(crate) semantics: &'static str,
    /// The fitted shipped `squonk` dialect the oracle pairs with.
    pub(crate) squonk_dialect: &'static str,
    /// How schema-dependent SQL is made comparable (the setup driver).
    pub(crate) setup_driver: &'static str,
}

/// The M2 accept/reject oracles, in display order. Extended as an engine lands.
pub(crate) const M2_ORACLE_ROWS: &[M2OracleRow] = &[
    M2OracleRow {
        engine: "sqlite",
        semantics: "prepare_bind",
        squonk_dialect: "sqlite",
        setup_driver: "schema-independent + provisioned",
    },
    M2OracleRow {
        engine: "duckdb",
        semantics: "prepare_bind",
        squonk_dialect: "duckdb",
        setup_driver: "schema-independent + provisioned",
    },
];

/// Render the M2 oracle milestone table deterministically, reusing the dialect
/// matrix's [`render_padded_table`] layout.
fn render_m2_oracle_matrix() -> String {
    let mut rows = vec![vec![
        "engine".to_string(),
        "semantics".to_string(),
        "squonk dialect".to_string(),
        "setup driver".to_string(),
    ]];
    for row in M2_ORACLE_ROWS {
        rows.push(vec![
            row.engine.to_string(),
            row.semantics.to_string(),
            row.squonk_dialect.to_string(),
            row.setup_driver.to_string(),
        ]);
    }
    render_padded_table(&rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One representative statement per `STANDARD_FEATURE_CATALOG` row, keying the
    /// `standard_catalog_flags_are_probe_backed` honesty gate. A `supported` row's probe
    /// must PARSE and an unsupported row's probe must FAIL under the maximal-feature
    /// `FeatureSet::LENIENT` — the widest documented union, so a reject there is a reject
    /// everywhere. The gate holds this table and the catalogue in exact 1:1
    /// correspondence, so no row can land — and no flag can flip — without a probe that
    /// proves its state. `iso-catalog-supported-flags-stale`.
    const CATALOG_PROBES: &[(&str, &str)] = &[
        // supported standard features (must PARSE)
        ("E031-01", "SELECT \"c\" FROM t"),
        ("E021-07", "SELECT 'a' || 'b'"),
        ("E021-08", "SELECT 'a' LIKE 'b'"),
        ("E021-06", "SELECT SUBSTRING('abcdef' FROM 2 FOR 3)"),
        ("E021-09", "SELECT TRIM(BOTH 'x' FROM 'xxabc')"),
        ("E021-11", "SELECT POSITION('b' IN 'abc')"),
        ("T312", "SELECT OVERLAY('abc' PLACING 'X' FROM 2 FOR 1)"),
        ("T611", "SELECT count(*) OVER ()"),
        ("E101", "INSERT INTO t VALUES (1)"),
        ("F031", "DROP TABLE t"),
        ("F031-01", "CREATE TABLE t (a INT)"),
        ("F031-02", "CREATE VIEW v AS SELECT 1"),
        ("F031-03", "GRANT SELECT ON t TO alice"),
        // The `UNKNOWN` form is unique to the truth-value predicate: under LENIENT it parses
        // to `Expr::IsTruth`, so a supported F571 must PARSE here.
        ("F571", "SELECT a IS UNKNOWN"),
        // Statement-head families folded from the measured per-dialect production
        // inventories once the bar-A parity programmes landed them (programme item 5,
        // catalog-fold-statement-head-negative-space). Each anchors a single ISO id and
        // must PARSE under LENIENT. CREATE FUNCTION uses the PostgreSQL dollar-body form
        // (the bare MySQL `RETURN`-body form routes to the compound-body path, which needs
        // the MySQL preset); CREATE TRIGGER uses the SQLite `BEGIN … END` body (the MySQL
        // `FOR EACH ROW` stored form is claimed only under the MySQL preset, not LENIENT).
        (
            "T321-01",
            "CREATE FUNCTION f() RETURNS INT AS $$SELECT 1$$ LANGUAGE SQL",
        ),
        ("T321-02", "CREATE PROCEDURE p() BEGIN END"),
        ("T321-04", "CALL p()"),
        ("T331", "CREATE ROLE r"),
        (
            "T211",
            "CREATE TRIGGER trg AFTER INSERT ON t BEGIN SELECT 1; END",
        ),
        ("F034", "REVOKE GRANT OPTION FOR SELECT ON t FROM alice"),
        // supported dialect extensions (must PARSE)
        ("pg:escape-string-syntax", "SELECT E'\\t'"),
        ("pg:dollar-quoted-strings", "SELECT $q$hi$q$"),
        // T176 sequence generators are now built (CREATE/DROP SEQUENCE under
        // PostgreSQL/DuckDB/LENIENT); the probe must PARSE, matching `supported: true`.
        ("T176", "CREATE SEQUENCE s"),
        // enumerable negative space (must FAIL)
        ("E121", "DECLARE c CURSOR FOR SELECT 1"),
        ("F251", "CREATE DOMAIN d AS INTEGER"),
    ];

    #[test]
    fn feature_set_fields_are_explicitly_enumerated() {
        let FeatureSet {
            identifier_casing: _,
            identifier_quotes: _,
            default_null_ordering: _,
            reserved_column_name: _,
            reserved_function_name: _,
            reserved_type_name: _,
            reserved_bare_alias: _,
            reserved_as_label: _,
            catalog_qualified_names: _,
            byte_classes: _,
            binding_powers: _,
            set_operation_powers: _,
            string_literals: _,
            numeric_literals: _,
            parameters: _,
            session_variables: _,
            identifier_syntax: _,
            table_expressions: _,
            join_syntax: _,
            table_factor_syntax: _,
            expression_syntax: _,
            operator_syntax: _,
            call_syntax: _,
            string_func_forms: _,
            aggregate_call_syntax: _,
            predicate_syntax: _,
            pipe_operator: _,
            double_ampersand: _,
            keyword_operators: _,
            caret_operator: _,
            hash_bitwise_xor: _,
            comment_syntax: _,
            mutation_syntax: _,
            statement_ddl_gates: _,
            create_table_clause_syntax: _,
            column_definition_syntax: _,
            constraint_syntax: _,
            index_alter_syntax: _,
            existence_guards: _,
            select_syntax: _,
            query_tail_syntax: _,
            grouping_syntax: _,
            utility_syntax: _,
            show_syntax: _,
            maintenance_syntax: _,
            access_control_syntax: _,
            type_name_syntax: _,
            target_spelling: _,
        } = FeatureSet::ANSI;

        assert_eq!(FEATURES.len(), 48);
    }

    // --- ISO 9075 feature taxonomy (prod-coverage-iso-feature-taxonomy) ------
    //
    // The registry anchors coverage on the standard's vocabulary: implemented
    // features carry their ISO id, extensions a namespaced id, and the catalogue
    // enumerates unbuilt standard surface so traceability can report what is
    // missing, not only what exists. These cases keep the registry and this
    // conformance layer honest against each other.

    #[test]
    fn standard_catalog_flags_are_probe_backed() {
        // The honesty gate replacing the former `unbuilt_core > 0` assert, which stayed
        // green only *because* five DML/DDL rows were stale negative space: a weak assert
        // that had inverted into a staleness shield (iso-catalog-supported-flags-stale).
        // Every catalogue row carries a probe; a supported row's probe must PARSE and an
        // unsupported row's probe must FAIL under the maximal-feature LENIENT dialect, so
        // a `supported` flag can never drift from what the parser actually does.

        // 1:1 correspondence, both directions — a flip or a new row cannot skip its proof.
        for row in STANDARD_FEATURE_CATALOG {
            assert!(
                CATALOG_PROBES.iter().any(|(id, _)| *id == row.id),
                "catalogue row `{}` has no probe in CATALOG_PROBES (conformance matrix.rs); \
                 add a representative statement that {} under FeatureSet::LENIENT",
                row.id,
                if row.supported {
                    "PARSES"
                } else {
                    "FAILS to parse"
                },
            );
        }
        for (id, _) in CATALOG_PROBES {
            assert!(
                standard_feature(id).is_some(),
                "CATALOG_PROBES carries a probe for `{id}`, absent from \
                 STANDARD_FEATURE_CATALOG; remove the stale probe or add the catalogue row",
            );
        }

        // Each probe's verdict under LENIENT must match its row's `supported` flag.
        for (id, sql) in CATALOG_PROBES {
            let row = standard_feature(id).expect("probe id resolved above");
            let parses = accepts_under(sql, &FeatureSet::LENIENT);
            if row.supported {
                assert!(
                    parses,
                    "catalogue marks `{id}` supported, but its probe `{sql}` FAILS to parse \
                     under FeatureSet::LENIENT; either the feature is not actually built \
                     (set `supported: false` in STANDARD_FEATURE_CATALOG) or the probe is \
                     wrong (fix CATALOG_PROBES in matrix.rs)",
                );
            } else {
                assert!(
                    !parses,
                    "catalogue marks `{id}` unsupported, but its probe `{sql}` PARSES under \
                     FeatureSet::LENIENT; the surface now exists — set `supported: true` in \
                     STANDARD_FEATURE_CATALOG (dialect/mod.rs) and set `realized_by` to its \
                     1:1 knob or None",
                );
            }
        }

        // The catalogue must still name unbuilt Core surface — the property the weak
        // assert reached for, now guaranteed alongside the mechanical proof above.
        assert!(
            unsupported_standard_features().any(|feature| feature.is_core()),
            "the catalogue no longer enumerates any unimplemented Core feature as negative \
             space; add an ISO-anchored Core gap with a failing probe (e.g. the E121 cursor \
             family) rather than leaving the negative space empty",
        );
    }

    #[test]
    fn realized_catalog_features_reference_enumerated_knobs() {
        // Every supported, knob-gated catalogue row points at a `Feature` that is
        // itself enumerated for the coverage matrix — so the standard vocabulary
        // and the dialect-data registry can never drift apart.
        for row in STANDARD_FEATURE_CATALOG {
            if let Some(feature) = row.realized_by {
                assert!(
                    FEATURES.contains(&feature),
                    "`{}` is realized by an unenumerated feature knob",
                    row.id,
                );
            }
        }
    }

    #[test]
    fn pg_string_literal_extensions_have_catalog_rows() {
        // The two PostgreSQL string-literal sub-flags are catalogued as namespaced
        // extension rows realized by `StringLiterals`. Other string sub-flags
        // (national/double-quoted/backslash/unicode) are lexical forms that are not
        // PostgreSQL extensions, so they carry no `pg:` catalogue row — the
        // ToggleableFeature `catalog_id` is `None` and `labels_resolve_in_the_feature_registry`
        // already enforces that every catalogued label resolves.
        for id in ["pg:escape-string-syntax", "pg:dollar-quoted-strings"] {
            let row = standard_feature(id).unwrap_or_else(|| panic!("`{id}` catalogued"));
            assert!(row.is_extension(), "`{id}` should be an extension row");
            assert_eq!(row.realized_by, Some(Feature::StringLiterals));
        }
    }

    #[test]
    fn iso_anchored_coverage_features_are_standard_not_extension() {
        // A coverage feature that claims an ISO id resolves to a standard (non
        // namespaced) catalogue row — proving coverage is expressible in the
        // standard's vocabulary, per the ticket acceptance.
        for feature in FEATURES {
            if let Some(iso_id) = feature.iso_id() {
                let row = standard_feature(iso_id)
                    .unwrap_or_else(|| panic!("coverage feature anchors missing id `{iso_id}`"));
                assert_eq!(row.conformance, Conformance::Core);
                assert!(!iso_id.contains(':'), "`{iso_id}` should be a standard id");
            }
        }
    }

    // --- version anchors & ideally-enabled polarity (prod-dialect-feature-version-anchors)

    #[test]
    fn version_anchor_pins_the_standard_feature_set_as_of_an_edition() {
        // A consumer can ask "what standard surface is in scope as of SQL:2016"
        // without enumerating features. Every implemented standard feature we expose
        // must fall within that frozen set (none post-dates the anchor).
        let as_of_2016: Vec<_> = standard_features_as_of(StandardVersion::Sql2016)
            .map(|feature| feature.id)
            .collect();
        for row in STANDARD_FEATURE_CATALOG {
            if row.supported && row.standardized_in.is_some() {
                assert!(
                    as_of_2016.contains(&row.id),
                    "supported standard feature `{}` should be in the SQL:2016 anchor",
                    row.id,
                );
            }
        }
        // The anchor is a subset relationship over editions, not the whole catalogue:
        // extensions (no edition) are never pinned by a standard-version anchor.
        assert!(
            standard_features_as_of(StandardVersion::Sql2016)
                .all(|feature| !feature.is_extension()),
        );
    }

    #[test]
    fn max_preset_turns_on_every_feature_at_m1() {
        // Every M1 dialect-data knob is additive (ideally enabled), so the
        // "enable everything" preset is the full coverage matrix — there is no
        // negative-polarity feature silently dropped today.
        assert_eq!(max_feature_metadata().count(), FEATURES.len());
    }

    // --- dialect milestone coverage matrix (prod-coverage-dialect-matrix) -----

    #[test]
    fn coverage_matrix_snapshot() {
        // The deterministic, git-diffable planning artifact (ADR-0015 source-of-truth):
        // a dialect x feature matrix carrying maturity and the covering behaviour
        // cases. Regenerate with `cargo insta accept` after an intended dialect-data
        // or coverage change; an unexpected diff means the dialect surface moved.
        insta::assert_snapshot!("coverage_matrix", render_coverage_matrix());
    }

    #[test]
    fn m2_oracle_matrix_snapshot() {
        // The M2 differential-oracle milestone, visible in the dialect-matrix artifact
        // (prod-dialect-m2-sqlite-duckdb-oracles). Static metadata welded to the live
        // oracles by `m2::tests::oracle_rows_match_coverage_matrix` under the
        // `oracle-engines` feature.
        insta::assert_snapshot!("m2_oracle_matrix", render_m2_oracle_matrix());
    }

    #[test]
    fn coverage_matrix_is_exhaustive_and_sorted() {
        // The matrix must enumerate every feature exactly once, in the documented
        // stable order, and name a real positive and negative case for each — the
        // structural invariants behind the snapshot, enforced independently of it.
        let matrix = render_coverage_matrix();
        let data_lines: Vec<&str> = matrix.lines().skip(2).collect(); // header + rule
        assert_eq!(
            data_lines.len(),
            FEATURES.len(),
            "the matrix must have exactly one row per enumerated feature",
        );
        for (line, feature) in data_lines.iter().zip(Feature::ALL) {
            assert!(
                line.starts_with(feature.id()),
                "matrix rows must follow Feature::ALL order: {line:?} should start with `{}`",
                feature.id(),
            );
            assert!(
                !line.contains("(none)"),
                "every feature must name a positive and a negative behaviour case: {line:?}",
            );
        }
    }

    #[test]
    fn dialect_presets_are_objective_against_ansi() {
        // ANSI is the baseline, so it never diverges from itself; PostgreSQL must
        // diverge for at least the features it visibly extends. This proves the
        // matrix's dialect axis is computed from the FeatureSets, not decorative.
        for feature in Feature::ALL {
            assert_eq!(
                feature_enablement(feature, &FeatureSet::ANSI),
                Enablement::Baseline,
                "ANSI is the matrix baseline and cannot diverge from itself: `{}`",
                feature.id(),
            );
        }
        for feature in [
            Feature::IdentifierCasing,
            Feature::StringLiterals,
            Feature::TableExpressions,
        ] {
            assert_eq!(
                feature_enablement(feature, &FeatureSet::POSTGRES),
                Enablement::Diverges,
                "PostgreSQL diverges from the ANSI baseline for `{}`",
                feature.id(),
            );
        }
    }

    #[test]
    fn diverging_dialect_features_have_a_positive_case() {
        // Dispatch/M2 readiness: wherever a shipping preset extends the ANSI baseline
        // there is a positive behaviour case proving the dialect actually accepts the
        // construct — the "positive case for each dialect that enables it" direction.
        for (name, preset) in DIALECT_PRESETS {
            for feature in Feature::ALL {
                if feature_enablement(feature, preset) == Enablement::Diverges {
                    assert_ne!(
                        representative_case(feature, Polarity::Positive),
                        "(none)",
                        "`{name}` diverges for `{}` but has no positive behaviour case",
                        feature.id(),
                    );
                }
            }
        }
    }
}
