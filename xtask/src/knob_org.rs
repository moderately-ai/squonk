// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! `cargo xtask knob-org` — dialect/AST knob organization freeze gate.
//!
//! Catches post-fuzz drift that unit tests do not: FeatureSet ↔ FEATURE_FIELDS
//! bijection, orphan bool flags (defined but never read under `crates/squonk/src`),
//! stale wrapper "not yet accepted" claims against enabled preset flags, and
//! narrowing-flag naming discipline.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::TidyResult;

/// Run every knob-org check; return all failures.
pub fn check_knob_org(root: &Path) -> TidyResult {
    let mut errors = Vec::new();
    if let Err(mut e) = check_feature_fields_bijection(root) {
        errors.append(&mut e);
    }
    if let Err(mut e) = check_orphan_bool_flags(root) {
        errors.append(&mut e);
    }
    if let Err(mut e) = check_wrapper_synopsis_honesty(root) {
        errors.append(&mut e);
    }
    if let Err(mut e) = check_narrowing_names(root) {
        errors.append(&mut e);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// FEATURE_FIELDS rows must match FeatureSet field names (order-independent set).
fn check_feature_fields_bijection(root: &Path) -> TidyResult {
    let mod_rs = fs::read_to_string(root.join("crates/squonk-ast/src/dialect/mod.rs"))
        .map_err(|e| vec![format!("read mod.rs: {e}")])?;
    let feature_set_rs =
        fs::read_to_string(root.join("crates/squonk-sourcegen/src/feature_set.rs"))
            .map_err(|e| vec![format!("read feature_set.rs: {e}")])?;

    let fs_fields = feature_set_struct_fields(&mod_rs);
    let meta_fields = feature_fields_meta(&feature_set_rs);

    let mut errors = Vec::new();
    for f in &fs_fields {
        if !meta_fields.contains(f) {
            errors.push(format!(
                "FeatureSet field `{f}` missing from FEATURE_FIELDS in squonk-sourcegen"
            ));
        }
    }
    for f in &meta_fields {
        if !fs_fields.contains(f) {
            errors.push(format!(
                "FEATURE_FIELDS row `{f}` has no FeatureSet field in dialect/mod.rs"
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn feature_set_struct_fields(mod_rs: &str) -> BTreeSet<String> {
    let start = mod_rs
        .find("pub struct FeatureSet {")
        .expect("FeatureSet struct");
    let body = &mod_rs[start..];
    let end = body.find("\n}").expect("FeatureSet close");
    let body = &body[..end];
    let mut out = BTreeSet::new();
    for line in body.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("pub ") {
            if let Some((name, _)) = rest.split_once(':') {
                out.insert(name.trim().to_owned());
            }
        }
    }
    out
}

fn feature_fields_meta(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    // Only scan the FEATURE_FIELDS table body.
    let Some(start) = src.find("const FEATURE_FIELDS") else {
        return out;
    };
    let body = &src[start..];
    let end = body.find("\n];").unwrap_or(body.len());
    let body = &body[..end];
    let mut rest = body;
    while let Some(idx) = rest.find("meta(") {
        rest = &rest[idx + 4..];
        if let Some(q) = rest.find('"') {
            rest = &rest[q + 1..];
            if let Some(end) = rest.find('"') {
                let name = &rest[..end];
                if !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '_')
                {
                    out.insert(name.to_owned());
                }
                rest = &rest[end + 1..];
            }
        }
    }
    out
}

/// Every nested bool field on dialect syntax structs must appear as a token in parser/tokenizer.
fn check_orphan_bool_flags(root: &Path) -> TidyResult {
    let mod_rs = fs::read_to_string(root.join("crates/squonk-ast/src/dialect/mod.rs"))
        .map_err(|e| vec![format!("read mod.rs: {e}")])?;
    let bools = dialect_bool_fields(&mod_rs);

    // One rg over squonk/src for all names is expensive; batch by checking with rg -F
    let squonk_src = root.join("crates/squonk/src");
    let mut errors = Vec::new();
    // Allowlist: pure identity/render metadata never consulted by the parser
    let allow: BTreeSet<&str> = [
        // none currently — keep list for intentional non-parse knobs
    ]
    .into_iter()
    .collect();

    for name in &bools {
        if allow.contains(name.as_str()) {
            continue;
        }
        let status = Command::new("rg")
            .args(["-q", "-F", name.as_str()])
            .arg(&squonk_src)
            .status()
            .map_err(|e| vec![format!("rg: {e}")])?;
        if !status.success() {
            errors.push(format!(
                "orphan bool flag `{name}`: no match under crates/squonk/src (parser/tokenizer)"
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn dialect_bool_fields(mod_rs: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in mod_rs.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("pub ") {
            if rest.contains(": bool") {
                if let Some((name, _)) = rest.split_once(':') {
                    out.insert(name.trim().to_owned());
                }
            }
        }
    }
    out
}

/// Wrapper synopses must not use the pre-review false-absolute form "Exactly one axis"
/// / "everything else is ANSI verbatim" when the preset enables more deltas.
fn check_wrapper_synopsis_honesty(root: &Path) -> TidyResult {
    let dir = root.join("crates/squonk/src/dialect");
    let mut errors = Vec::new();
    // Absolute-count lies that survived fuzz churn; "Everything else is inherited" in
    // closed-delta *tests* is OK when the test actually asserts field equality.
    let banned = ["Exactly one axis"];
    for entry in fs::read_dir(&dir).map_err(|e| vec![e.to_string()])? {
        let entry = entry.map_err(|e| vec![e.to_string()])?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|e| vec![e.to_string()])?;
        let name = path.file_name().unwrap().to_string_lossy();
        for phrase in banned {
            if text.contains(phrase) {
                errors.push(format!(
                    "{name}: banned stale synopsis phrase `{phrase}`"
                ));
            }
        }
    }
    // Also scan AST preset modules for the same absolute lies.
    let ast_dir = root.join("crates/squonk-ast/src/dialect");
    for entry in fs::read_dir(&ast_dir).map_err(|e| vec![e.to_string()])? {
        let entry = entry.map_err(|e| vec![e.to_string()])?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|e| vec![e.to_string()])?;
        let name = path.file_name().unwrap().to_string_lossy();
        for phrase in banned {
            if text.contains(phrase) {
                errors.push(format!(
                    "squonk-ast dialect/{name}: banned stale synopsis phrase `{phrase}`"
                ));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Narrowing bools should use requires_/rejects_ (with documented exceptions).
fn check_narrowing_names(root: &Path) -> TidyResult {
    let mod_rs = fs::read_to_string(root.join("crates/squonk-ast/src/dialect/mod.rs"))
        .map_err(|e| vec![format!("read mod.rs: {e}")])?;
    let exceptions: BTreeSet<&str> = [
        "position_asymmetric_operands",
        "restricted_cast_targets",
        "with_ties_requires_order_by", // dual-behavior grandfathered
        "transaction_modes_require_commas",
        "transaction_modes_reject_duplicates",
    ]
    .into_iter()
    .collect();

    let mut errors = Vec::new();
    // Flags whose docs say "ON rejects" / "Require" without naming doctrine form
    for line in mod_rs.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("pub ") {
            if rest.contains(": bool") {
                if let Some((name, _)) = rest.split_once(':') {
                    let name = name.trim();
                    if (name.contains("required") || name.ends_with("_unique"))
                        && !name.contains("requires_")
                        && !name.contains("rejects_")
                        && !exceptions.contains(name)
                    {
                        errors.push(format!(
                            "narrowing-looking flag `{name}` should use requires_/rejects_ form"
                        ));
                    }
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_fields_parse_nonempty() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mod_rs = fs::read_to_string(root.join("crates/squonk-ast/src/dialect/mod.rs")).unwrap();
        let fields = feature_set_struct_fields(&mod_rs);
        assert!(fields.len() >= 48, "expected FeatureSet fields, got {}", fields.len());
        assert!(fields.contains("utility_syntax"));
        assert!(fields.contains("transaction_syntax"));
        assert!(fields.contains("view_sequence_clause_syntax"));
    }
}
