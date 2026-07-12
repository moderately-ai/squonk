// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The license-header block prepended to every generated file.
//!
//! Generated files carry their SPDX + copyright header from THEIR generator, not the
//! `cargo xtask license-headers` stamper (which treats `@generated` files as
//! check-only, so it never fights the zero-drift gate). Both read the SAME policy —
//! `license-header.toml` at the workspace root — and MUST emit byte-identical header
//! lines, or the stamper's `--check` gate flags the freshly generated file. This
//! module produces that block; keep its format in lockstep with
//! `xtask/src/license_header.rs`.

use std::fs;

use crate::workspace_root;

/// The single source of truth for the header policy, at the workspace root.
const CONFIG_FILE: &str = "license-header.toml";

/// Comment syntax for the block: `//` for the Rust / TS / JS generated files, `#` for
/// the Python metadata file.
pub(crate) enum Comment {
    Slash,
    Hash,
}

impl Comment {
    fn prefix(&self) -> &'static str {
        match self {
            Comment::Slash => "// ",
            Comment::Hash => "# ",
        }
    }
}

/// The header block for `style`: two comment lines (SPDX id + copyright) then one
/// blank line, so the existing `@generated` preamble that follows reads as its own
/// paragraph. Ends with `\n\n`.
pub(crate) fn block(style: Comment) -> String {
    let config = load_config();
    let prefix = style.prefix();
    format!(
        "{prefix}SPDX-License-Identifier: {spdx}\n\
         {prefix}Copyright (c) {year} {holder}\n\n",
        spdx = config.spdx,
        year = config.year,
        holder = config.holder,
    )
}

struct LicenseConfig {
    spdx: String,
    holder: String,
    year: String,
}

/// Read the header policy. Panics — like every other error path in this dev-only
/// tool — naming the missing field, so a misconfiguration fails loudly rather than
/// silently emitting a malformed header.
fn load_config() -> LicenseConfig {
    let path = workspace_root().join(CONFIG_FILE);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    let mut spdx = None;
    let mut holder = None;
    let mut year = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim().to_owned();
        match key.trim() {
            "spdx" => spdx = Some(value),
            "holder" => holder = Some(value),
            "year" => year = Some(value),
            _ => {}
        }
    }
    let require = |field: Option<String>, name: &str| {
        field
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| panic!("{CONFIG_FILE}: missing or empty `{name}` field"))
    };
    LicenseConfig {
        spdx: require(spdx, "spdx"),
        holder: require(holder, "holder"),
        year: require(year, "year"),
    }
}
