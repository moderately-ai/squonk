// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Serialized binding-schema contract gate (`serde` feature, `docs/schema-contract.md`).
//!
//! The Python and WASM bindings emit serde JSON for the AST plus the envelope
//! types in `squonk::bindings` (`ParseDocument`, `RecoveredDocument`,
//! `TokenizeDocument`, `ParseDiagnostic`, `DialectInfo`, `ResolverMetadata`, …).
//! The Rust semver gate (`cargo xtask semver`) cannot see a renamed serde field, a
//! changed enum representation, or an `skip_serializing_if` omission change, so
//! this test freezes the wire shape against a checked-in snapshot and fails loudly
//! on any unreviewed change — the wire analogue of the sourcegen
//! `generated_files_are_up_to_date` drift gate.
//!
//! Two artifacts under `release/schema/`, beside `release/semver-baseline.toml`:
//! - `wire-schema.v{VERSION}.json` — a canonical serialization of every JSON root.
//!   Regenerated on any *reviewed* shape change (additive-optional or, with a
//!   version bump, breaking). Byte-drift-gated by [`wire_schema_snapshot_is_current`].
//! - `compat/parsed.baseline.json` — a FROZEN `Parsed` document authored against
//!   the first schema version. [`frozen_baseline_still_deserializes`] proves the
//!   current code still reads it, so an additive change stays compatible while a
//!   breaking one fails here and forces a version bump.
//!
//! Regenerate the shape snapshot with `UPDATE_SCHEMA_SNAPSHOT=1 cargo nextest run
//! -p squonk --features serde wire_schema`. The frozen compat baseline is
//! written only when absent, so it never silently rebaselines.
#![cfg(feature = "serde")]

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use squonk::ast::dialect::{SupportEvidence, SupportTier};
use squonk::bindings::{
    BindingToken, BindingTokenKind, BindingTrivia, DialectInfo, KeywordSymbol, ParseDiagnostic,
    ParseDocument, RecoveredDocument, ResolverMetadata, SourceSpan, TokenizeDocument,
    WIRE_SCHEMA_VERSION,
};
use squonk::{BuiltinDialect, Parsed, parse, parse_recovering_builtin};

/// The checked-in shape snapshot for the current [`WIRE_SCHEMA_VERSION`].
fn snapshot_path() -> PathBuf {
    schema_dir().join(format!("wire-schema.v{WIRE_SCHEMA_VERSION}.json"))
}

/// The frozen `Parsed` document the compatibility test deserializes.
fn compat_baseline_path() -> PathBuf {
    schema_dir().join("compat").join("parsed.baseline.json")
}

/// `release/schema/`, resolved from this crate's manifest dir so the test is
/// cwd-independent (the workspace root is two levels up from `crates/squonk`).
fn schema_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../release/schema")
}

/// A deliberately minimal resolver: the wire *shape* of `ResolverMetadata`, not the
/// live ~700-entry keyword table (which is data that churns on every keyword
/// addition, not schema). Two entries pin the `keyword_symbols` element shape.
fn minimal_resolver() -> ResolverMetadata {
    ResolverMetadata {
        dynamic_base: 3,
        keyword_symbols: vec![
            KeywordSymbol {
                symbol: 1,
                text: "select",
            },
            KeywordSymbol {
                symbol: 2,
                text: "from",
            },
        ],
    }
}

/// A representative parse exercising statements, projections, a binary expression,
/// an `IN` list, and an `ORDER BY` — the concrete AST wire example and the
/// round-trip anchor.
fn ast_sample() -> Parsed {
    parse("SELECT a + 1 AS n, b FROM t WHERE a IN (1, 2) ORDER BY b").expect("sample parses")
}

/// Assemble the canonical serialization of every JSON root that crosses the Python
/// and WASM boundaries. Each value pins field names, nesting, serde enum
/// representation (external vs the internally-tagged `BindingTokenKind`), and
/// `skip_serializing_if` omission behaviour — the shape a wire consumer depends on.
fn canonical_snapshot() -> String {
    // The AST root (`Parsed`) and the two documents that flatten it. The wrappers
    // use minimal instances so the envelope shape is not buried under a full parse.
    let ast = ast_sample();
    let wrapped = parse("SELECT 1 FROM t").expect("wrapper sample parses");
    let recovered = parse_recovering_builtin("SELECT 1; SELECT FROM t", BuiltinDialect::Ansi)
        .expect("recovering parse produces a partial tree");

    let parse_document = ParseDocument {
        dialect: "ansi",
        parsed: &wrapped,
        trivia: Some(vec![BindingTrivia {
            kind: "Whitespace",
            span: SourceSpan { start: 8, end: 9 },
            text: " ".to_owned(),
        }]),
        resolver: minimal_resolver(),
    };

    let recovered_document = RecoveredDocument {
        parsed: ParseDocument {
            dialect: "ansi",
            parsed: recovered.parsed(),
            trivia: None,
            resolver: minimal_resolver(),
        },
        errors: recovered
            .errors()
            .iter()
            .map(ParseDiagnostic::from)
            .collect(),
    };

    // A standalone diagnostic (the universal error shape thrown by the fail-fast
    // APIs) built through the real `From<&ParseError>` conversion.
    let diagnostic_error = parse("SELECT FROM t").expect_err("invalid SQL yields a diagnostic");
    let parse_diagnostic = ParseDiagnostic::from(&diagnostic_error);

    // Tokenizer output, hand-built to pin every representation-critical token kind:
    // the internally-tagged `kind` discriminator, a unit variant, and the two
    // struct variants carrying an operator / punctuation name.
    let tokenize_document = TokenizeDocument {
        source: "SELECT 1 + 2".to_owned(),
        dialect: "ansi",
        tokens: vec![
            BindingToken {
                kind: BindingTokenKind::Keyword { keyword: "select" },
                span: SourceSpan { start: 0, end: 6 },
                text: "SELECT".to_owned(),
            },
            BindingToken {
                kind: BindingTokenKind::Number,
                span: SourceSpan { start: 7, end: 8 },
                text: "1".to_owned(),
            },
            BindingToken {
                kind: BindingTokenKind::Operator { operator: "Plus" },
                span: SourceSpan { start: 9, end: 10 },
                text: "+".to_owned(),
            },
            BindingToken {
                kind: BindingTokenKind::Number,
                span: SourceSpan { start: 11, end: 12 },
                text: "2".to_owned(),
            },
        ],
        trivia: Some(vec![BindingTrivia {
            kind: "Whitespace",
            span: SourceSpan { start: 6, end: 7 },
            text: " ".to_owned(),
        }]),
    };

    // Dialect metadata — a representative pair, feature-independent on purpose so
    // the shape gate does not depend on which dialect presets are compiled.
    // Two entries pin two `SupportEvidence` variants (the internally-tagged `kind`
    // shape) alongside the `SupportTier` spelling, feature-independently.
    let dialect_info = vec![
        DialectInfo {
            name: "ansi",
            aliases: &["ansi", "sql"],
            tier: SupportTier::Stable,
            evidence: SupportEvidence::StandardReference {
                note: "ISO/IEC 9075 baseline",
            },
        },
        DialectInfo {
            name: "postgres",
            aliases: &["postgres", "postgresql", "pg"],
            tier: SupportTier::Stable,
            evidence: SupportEvidence::EngineDifferential {
                engine: "libpg_query",
                version: "pg_query 6.1.1",
                method: "raw-parse-tree differential",
            },
        },
    ];

    // The token-kind enum on its own, spanning unit and struct variants, to pin the
    // internally-tagged (`tag = "kind"`) representation independent of a live parse.
    let binding_token_kinds = vec![
        BindingTokenKind::Word,
        BindingTokenKind::Keyword { keyword: "select" },
        BindingTokenKind::Operator { operator: "Plus" },
        BindingTokenKind::Punctuation {
            punctuation: "Comma",
        },
    ];

    let mut roots = Map::new();
    roots.insert("parsed".to_owned(), to_value("parsed", &ast));
    roots.insert(
        "parse_document".to_owned(),
        to_value("parse_document", &parse_document),
    );
    roots.insert(
        "recovered_document".to_owned(),
        to_value("recovered_document", &recovered_document),
    );
    roots.insert(
        "parse_diagnostic".to_owned(),
        to_value("parse_diagnostic", &parse_diagnostic),
    );
    roots.insert(
        "tokenize_document".to_owned(),
        to_value("tokenize_document", &tokenize_document),
    );
    roots.insert(
        "dialect_info".to_owned(),
        to_value("dialect_info", &dialect_info),
    );
    roots.insert(
        "resolver_metadata".to_owned(),
        to_value("resolver_metadata", &minimal_resolver()),
    );
    roots.insert(
        "binding_token_kinds".to_owned(),
        to_value("binding_token_kinds", &binding_token_kinds),
    );

    let document = json!({
        "schema_version": WIRE_SCHEMA_VERSION,
        "roots": Value::Object(roots),
    });
    let mut text = serde_json::to_string_pretty(&document).expect("snapshot serializes");
    text.push('\n');
    text
}

fn to_value<T: serde::Serialize>(name: &str, value: &T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|err| panic!("serialize `{name}`: {err}"))
}

/// The wire-shape drift gate: the checked-in snapshot must be byte-identical to a
/// fresh serialization. Mirrors the sourcegen `generated_files_are_up_to_date`
/// gate, but for the *serialized* surface the Rust semver gate cannot see.
#[test]
fn wire_schema_snapshot_is_current() {
    let generated = canonical_snapshot();
    let path = snapshot_path();

    if std::env::var_os("UPDATE_SCHEMA_SNAPSHOT").is_some() {
        std::fs::create_dir_all(path.parent().expect("snapshot has a parent"))
            .expect("create release/schema");
        std::fs::write(&path, &generated).expect("write shape snapshot");
        // The frozen compat baseline is written only when absent, so a routine
        // regen never rebaselines it (it must survive as the immutable v1 witness).
        let baseline = compat_baseline_path();
        if !baseline.exists() {
            std::fs::create_dir_all(baseline.parent().expect("baseline has a parent"))
                .expect("create release/schema/compat");
            let baseline_value =
                serde_json::to_value(ast_sample()).expect("baseline document serializes");
            let mut frozen =
                serde_json::to_string_pretty(&baseline_value).expect("baseline serializes");
            frozen.push('\n');
            std::fs::write(&baseline, frozen).expect("write frozen compat baseline");
        }
        return;
    }

    let committed = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "{}: {err}\nregenerate with `UPDATE_SCHEMA_SNAPSHOT=1 cargo nextest run -p squonk \
             --features serde wire_schema` (see docs/schema-contract.md)",
            path.display(),
        )
    });

    if committed != generated {
        let line = first_divergent_line(&committed, &generated);
        panic!(
            "the serialized binding wire shape changed (first divergence at line {line} of {}).\n\
             This is a WIRE CONTRACT change the Rust semver gate cannot see. Follow \
             docs/schema-contract.md:\n\
             - additive-optional change (new skip_serializing_if field / #[non_exhaustive] variant): \
             keep WIRE_SCHEMA_VERSION, regenerate the snapshot;\n\
             - breaking change (renamed/removed field, changed enum representation or omission \
             behaviour): bump WIRE_SCHEMA_VERSION in crates/squonk/src/bindings.rs and add a new \
             release/schema/wire-schema.v{{N}}.json, keeping this one frozen.\n\
             Regenerate with `UPDATE_SCHEMA_SNAPSHOT=1 cargo nextest run -p squonk --features \
             serde wire_schema`.",
            snapshot_path().display(),
        );
    }
}

/// The version constant and the snapshot cannot drift apart: the checked-in
/// document's `schema_version` must equal the Rust [`WIRE_SCHEMA_VERSION`] both
/// bindings expose.
#[test]
fn schema_version_matches_binding_constant() {
    // In regen mode the writer test owns the file; validation is skipped so the
    // two run concurrently under nextest without racing on the freshly written file.
    if std::env::var_os("UPDATE_SCHEMA_SNAPSHOT").is_some() {
        return;
    }
    let path = snapshot_path();
    let committed =
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    let document: Value = serde_json::from_str(&committed).expect("snapshot is valid JSON");
    assert_eq!(
        document["schema_version"].as_u64(),
        Some(u64::from(WIRE_SCHEMA_VERSION)),
        "release/schema/wire-schema.v{WIRE_SCHEMA_VERSION}.json records a different schema_version \
         than the WIRE_SCHEMA_VERSION constant; regenerate after a version bump",
    );
}

/// Backward-compatibility across the stable baseline: a `Parsed` document authored
/// against the first schema version must still deserialize under the current code
/// and render. An additive-optional change keeps this green (unknown-to-old fields
/// are simply absent); a breaking representation change fails here and forces a
/// version bump rather than silently breaking every stored document.
#[test]
fn frozen_baseline_still_deserializes() {
    // Skip during regen: the writer test may be creating the baseline concurrently.
    if std::env::var_os("UPDATE_SCHEMA_SNAPSHOT").is_some() {
        return;
    }
    let path = compat_baseline_path();
    let frozen = std::fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "{}: {err}\ngenerate the frozen baseline once with `UPDATE_SCHEMA_SNAPSHOT=1 cargo \
             nextest run -p squonk --features serde wire_schema`",
            path.display(),
        )
    });

    let restored: Parsed = serde_json::from_str(&frozen).unwrap_or_else(|err| {
        panic!(
            "the frozen v1 baseline no longer deserializes ({err}). This is a BREAKING wire change: \
             bump WIRE_SCHEMA_VERSION and follow docs/schema-contract.md instead of editing the \
             frozen baseline.",
        )
    });
    // A round-trip render proves the deserialized tree is structurally usable, not
    // merely syntactically accepted.
    assert!(
        !restored.to_sql().is_empty(),
        "the baseline document rendered empty SQL",
    );
}

/// One-based line of the first divergence, for a readable drift message (mirrors the
/// sourcegen drift gate's helper).
fn first_divergent_line(committed: &str, generated: &str) -> usize {
    committed
        .lines()
        .zip(generated.lines())
        .position(|(a, b)| a != b)
        .map_or_else(
            || committed.lines().count().min(generated.lines().count()) + 1,
            |index| index + 1,
        )
}
