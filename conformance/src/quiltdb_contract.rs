// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! QuiltDB's frozen SQL corpus and first-party parser contract.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use squonk::ast::render::RenderMode;
use squonk::dialect::QuiltDb;
use squonk::{ParseConfig, parse_with};

const EXPECTED_COUNT: usize = 1_670;
const EXPECTED_INTEGRATION_COUNT: usize = 1_356;
const EXPECTED_PARSE_ERRORS: &[&str] = &[
    "aeb5bd01df61db18",
    "ee1603da6ad01b31",
    "4a63469c3e410099",
    "7e0998cd6ce9f9a8",
    "58e53bcf407f88d9",
    "79cbf01d13dee461",
    "9f14495e570f978f",
    "a9f9294bb0453447",
    "2cf848ac69b6140c",
    "96d5b07f58be62a9",
    "a40f55a375159c5c",
    "c491c9ea328a3896",
    "ed09e3382a6da2f5",
    "6388384e75a7f912",
    "71594a77e9be554c",
    "c97b3e77ed842379",
];

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus/quiltdb/slt-corpus.json")
}

fn reject_corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus/quiltdb/reject-corpus.json")
}

fn integration_corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus/quiltdb/integration-corpus.json")
}

fn integration_stable_id(sql: &str) -> String {
    let normalized = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in normalized.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn cases() -> Vec<(String, String, String)> {
    let bytes = fs::read(corpus_path()).expect("read vendored QuiltDB corpus");
    let root: Value = serde_json::from_slice(&bytes).expect("valid QuiltDB corpus JSON");
    let count = root["count"].as_u64().expect("numeric corpus count") as usize;
    let rows = root["cases"].as_array().expect("corpus cases array");
    assert_eq!(count, EXPECTED_COUNT, "QuiltDB corpus count changed");
    assert_eq!(rows.len(), EXPECTED_COUNT, "QuiltDB case array changed");

    let mut ids = BTreeSet::new();
    rows.iter()
        .map(|row| {
            let id = row["id"].as_str().expect("case id").to_owned();
            let sql = row["sql"].as_str().expect("case SQL").to_owned();
            let context = row["slt_directive"]
                .as_str()
                .expect("SLT context")
                .to_owned();
            assert!(ids.insert(id.clone()), "duplicate QuiltDB stable id {id}");
            assert!(!sql.trim().is_empty(), "empty SQL for QuiltDB case {id}");
            (id, sql, context)
        })
        .collect()
}

#[test]
fn quiltdb_frozen_parser_contract() {
    let expected_errors = EXPECTED_PARSE_ERRORS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(expected_errors.len(), EXPECTED_PARSE_ERRORS.len());
    for (id, sql, context) in cases() {
        let parsed = parse_with(&sql, ParseConfig::new(QuiltDb));
        if expected_errors.contains(id.as_str()) {
            assert!(
                parsed.is_err(),
                "{id} ({context}) must remain parse_err: {sql}"
            );
            continue;
        }
        let parsed = parsed.unwrap_or_else(|error| {
            panic!("{id} ({context}) must remain parse_ok: {sql}\n{error}")
        });
        for mode in [RenderMode::Canonical, RenderMode::Parenthesized] {
            let rendered = crate::render_statements(&parsed, mode);
            let reparsed =
                parse_with(&rendered, ParseConfig::new(QuiltDb)).unwrap_or_else(|error| {
                    panic!("{id} rendered under {mode:?} did not parse: {rendered}\n{error}")
                });
            let comparison = crate::shared_interner::compare_statements_with_shared_symbols(
                parsed.statements(),
                parsed.resolver(),
                reparsed.statements(),
                reparsed.resolver(),
            );
            assert!(
                comparison.structurally_equal(),
                "{id} changed shape under {mode:?}: {rendered}"
            );
        }
    }
}

#[test]
fn quiltdb_curated_reject_contract() {
    let bytes = fs::read(reject_corpus_path()).expect("read curated reject corpus");
    let root: Value = serde_json::from_slice(&bytes).expect("valid reject corpus JSON");
    let rows = root["cases"].as_array().expect("reject cases array");
    assert!(!rows.is_empty(), "the reject contract must not be empty");
    let mut ids = BTreeSet::new();
    for row in rows {
        let id = row["id"].as_str().expect("reject case id");
        let sql = row["sql"].as_str().expect("reject case SQL");
        assert!(ids.insert(id), "duplicate reject case id {id}");
        assert!(
            parse_with(sql, ParseConfig::new(QuiltDb)).is_err(),
            "{id} must remain outside the grammar: {sql}"
        );
    }
}

#[test]
fn quiltdb_integration_parser_contract() {
    let bytes = fs::read(integration_corpus_path()).expect("read vendored integration corpus");
    let root: Value = serde_json::from_slice(&bytes).expect("valid integration corpus JSON");
    let count = root["count"].as_u64().expect("numeric integration count") as usize;
    let rows = root["cases"].as_array().expect("integration cases array");
    assert_eq!(count, EXPECTED_INTEGRATION_COUNT);
    assert_eq!(rows.len(), EXPECTED_INTEGRATION_COUNT);

    let mut failures = Vec::new();
    let mut ids = BTreeSet::new();
    let mut parse_ok = 0usize;
    let mut parse_err = 0usize;
    for row in rows {
        let id = row["id"].as_str().expect("integration case id");
        let sql = row["sql"].as_str().expect("integration case SQL");
        let verdict = row["verdict"].as_str().expect("integration case verdict");
        assert!(ids.insert(id), "duplicate integration case id {id}");
        assert_eq!(id, integration_stable_id(sql), "unstable id for {sql}");
        assert!(
            !row["provenance"]
                .as_array()
                .expect("integration provenance array")
                .is_empty(),
            "missing provenance for {id}"
        );
        if verdict == "parse_err" {
            parse_err += 1;
            if parse_with(sql, ParseConfig::new(QuiltDb)).is_ok() {
                failures.push(format!("{id}: must remain parse_err: {sql}"));
            }
            continue;
        }
        assert_eq!(verdict, "parse_ok", "unknown verdict for {id}");
        parse_ok += 1;
        match parse_with(sql, ParseConfig::new(QuiltDb)) {
            Ok(parsed) => {
                for mode in [RenderMode::Canonical, RenderMode::Parenthesized] {
                    let rendered = crate::render_statements(&parsed, mode);
                    let Ok(reparsed) = parse_with(&rendered, ParseConfig::new(QuiltDb)) else {
                        failures.push(format!("{id}: {mode:?} render did not parse: {rendered}"));
                        continue;
                    };
                    let comparison = crate::shared_interner::compare_statements_with_shared_symbols(
                        parsed.statements(),
                        parsed.resolver(),
                        reparsed.statements(),
                        reparsed.resolver(),
                    );
                    if !comparison.structurally_equal() {
                        failures.push(format!("{id}: changed shape under {mode:?}: {rendered}"));
                    }
                }
            }
            Err(error) => failures.push(format!("{id}: {sql}\n{error}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} integration SQL cases failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
    assert_eq!((parse_ok, parse_err), (1_346, 10));
}
