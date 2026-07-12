// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Config-driven license-header stamping and verification.
//!
//! `cargo xtask license-headers --check | --write` keeps a minimal SPDX + copyright
//! header on every first-party source file. The policy — SPDX identifier, holder,
//! year — lives in one place, [`CONFIG_FILE`] at the workspace root, so finalising
//! or swapping the licence is one config edit plus one mechanical run, never a
//! hand-edit sweep. `squonk-sourcegen` reads the SAME config so generated files
//! carry the identical header from their generator template (this module treats
//! `@generated` files as check-only; it never rewrites them, or it would fight the
//! zero-drift gate).
//!
//! Header form (deliberately minimal — the full text lives in `LICENSE`, blocks rot
//! and bloat diffs):
//!
//! ```text
//! // SPDX-License-Identifier: MIT
//! // Copyright (c) 2026 Moderately AI Inc.
//! ```
//!
//! Ordering for Rust: the header sits at the very top (after a `#!` shebang, if any),
//! ABOVE `//!` module docs and `#![...]` inner attributes. A leading `//` line-comment
//! before `//!` is the ubiquitous Rust convention and leaves rustdoc's module docs and
//! clippy's inner attributes intact.
//!
//! Exclusions are load-bearing (ADR-0015): vendored corpora are NEVER stamped — the
//! walker skips every `corpus`/`corpora` subtree, and [`is_within_vendored_corpus`]
//! refuses a vendored path even if one is targeted directly, so the stamper can never
//! relicense a vendored file. Their provenance is verified by the separate
//! `check_corpus_licenses` gate.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{TidyResult, display_path, is_provenance_file, is_rust_module_dir, sorted_dir_entries};

/// The single source of truth for the header policy, at the workspace root. A flat
/// `key = "value"` TOML subset under a `[license-header]` table, hand-parsed so xtask
/// stays dependency-free (ADR-0017), exactly like the corpus `PROVENANCE.toml` gate.
const CONFIG_FILE: &str = "license-header.toml";

/// Build-output / dependency directories a source walk must never descend into. The
/// repo-wide `.git`/`.claude`/`target` skips plus the JS/wasm toolchain outputs
/// (`node_modules`, wasm-bindgen `pkg`, bundler `dist`) that are untracked build
/// artifacts, not first-party source. wasm-pack's per-preset outputs are `pkg-<preset>`
/// directories (gitignored, present only in workspaces that ran a wasm build), so a
/// `pkg-` prefix skips them alongside the bare `pkg`.
const SKIP_DIR_NAMES: &[&str] = &[".git", ".claude", "target", "node_modules", "pkg", "dist"];

/// Directory-name prefixes treated like [`SKIP_DIR_NAMES`] members.
const SKIP_DIR_PREFIXES: &[&str] = &["pkg-"];

/// Corpus-root directory names whose subtrees hold vendored third-party material the
/// stamper must never touch (mirrors `crate::VENDORED_CORPUS_ROOT_NAMES`; kept local so
/// the two gates can diverge if their policies ever do).
const VENDORED_CORPUS_ROOT_NAMES: &[&str] = &["corpus", "corpora"];

/// The resolved header policy.
struct LicenseConfig {
    spdx: String,
    holder: String,
    year: String,
}

impl LicenseConfig {
    /// The two header content lines (without any comment prefix), in order.
    fn content_lines(&self) -> [String; 2] {
        [
            format!("SPDX-License-Identifier: {}", self.spdx),
            format!("Copyright (c) {} {}", self.year, self.holder),
        ]
    }
}

/// The comment syntax a stampable file uses.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Comment {
    /// `//` — Rust, JS/MJS/TS, Java.
    Slash,
    /// `#` — Python, TOML, YAML.
    Hash,
}

impl Comment {
    fn prefix(self) -> &'static str {
        match self {
            Comment::Slash => "// ",
            Comment::Hash => "# ",
        }
    }

    /// The bare comment token (no trailing space), for recognising an existing header
    /// line regardless of its interior spacing.
    fn token(self) -> &'static str {
        match self {
            Comment::Slash => "//",
            Comment::Hash => "#",
        }
    }
}

/// The comment syntax for a file, by extension, or `None` when the format is not
/// stamped. Data (`csv`/`json`/`snap`/`lock`), prose (`md`), and markup (`html`/`css`)
/// are deliberately excluded — see the ticket's per-format decisions.
fn comment_style(path: &Path) -> Option<Comment> {
    match path.extension().and_then(OsStr::to_str)? {
        "rs" | "js" | "mjs" | "ts" | "java" => Some(Comment::Slash),
        "py" | "pyi" | "toml" | "yml" | "yaml" => Some(Comment::Hash),
        _ => None,
    }
}

/// `cargo xtask license-headers --check` (and the `tidy` gate): every stampable
/// first-party file carries the exact, current header. Generated files are included —
/// a stale generated header fails here too, pointing at `squonk-sourcegen` — but
/// vendored trees are excluded (their provenance is the corpus gate's job).
pub fn check_license_headers(root: &Path) -> TidyResult {
    let config = load_config(root).map_err(|err| vec![err])?;
    let mut errors = Vec::new();
    for file in stampable_files(root) {
        let Some(style) = comment_style(&file) else {
            continue;
        };
        let contents = match fs::read_to_string(&file) {
            Ok(contents) => contents,
            // A non-UTF-8 file with a stamped extension is not something we author;
            // skip it rather than fail the gate on it.
            Err(_) => continue,
        };
        if stamp(&contents, style, &config) != contents {
            let remediation = if is_generated(&contents, style) {
                "regenerate with `cargo run -p squonk-sourcegen`"
            } else {
                "run `cargo xtask license-headers --write`"
            };
            errors.push(format!(
                "{}: license header missing or stale; {remediation}",
                display_path(root, &file),
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// `cargo xtask license-headers --write`: stamp/refresh every first-party file's
/// header idempotently. Generated files are left to their generator (this only
/// verifies them, via [`check_license_headers`]); vendored trees are never touched.
/// Returns a one-line summary of how many files changed.
pub fn write_license_headers(root: &Path) -> Result<String, Vec<String>> {
    let config = load_config(root).map_err(|err| vec![err])?;
    let mut changed = 0usize;
    let mut scanned = 0usize;
    let mut errors = Vec::new();
    for file in stampable_files(root) {
        let Some(style) = comment_style(&file) else {
            continue;
        };
        let contents = match fs::read_to_string(&file) {
            Ok(contents) => contents,
            Err(_) => continue,
        };
        // Defence in depth against the "stamp-the-world relicensing" bug (ADR-0015):
        // the walker already skips vendored corpus subtrees, but refuse a vendored path
        // outright even if one reaches here, so the stamper can never relicense
        // vendored material.
        if is_within_vendored_corpus(root, &file) {
            continue;
        }
        scanned += 1;
        // Generator-owned files get their header from the generator template reading
        // the same config; rewriting them here would diverge from what sourcegen emits
        // and break the zero-drift gate.
        if is_generated(&contents, style) {
            continue;
        }
        let stamped = stamp(&contents, style, &config);
        if stamped != contents {
            if let Err(err) = fs::write(&file, &stamped) {
                errors.push(format!(
                    "{}: write failed: {err}",
                    display_path(root, &file)
                ));
            } else {
                changed += 1;
            }
        }
    }
    if errors.is_empty() {
        Ok(format!("{changed} file(s) stamped, {scanned} scanned"))
    } else {
        Err(errors)
    }
}

/// Read and validate the header policy from [`CONFIG_FILE`].
fn load_config(root: &Path) -> Result<LicenseConfig, String> {
    let path = root.join(CONFIG_FILE);
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("{}: read failed: {err}", display_path(root, &path)))?;
    let mut spdx = None;
    let mut holder = None;
    let mut year = None;
    for raw in text.lines() {
        let line = raw.trim();
        // Skip blanks, comments (including this file's own stamped header), and the
        // `[license-header]` table header.
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
    let missing = |name: &str| format!("{CONFIG_FILE}: missing or empty `{name}` field");
    Ok(LicenseConfig {
        spdx: spdx
            .filter(|v| !v.is_empty())
            .ok_or_else(|| missing("spdx"))?,
        holder: holder
            .filter(|v| !v.is_empty())
            .ok_or_else(|| missing("holder"))?,
        year: year
            .filter(|v| !v.is_empty())
            .ok_or_else(|| missing("year"))?,
    })
}

/// Whether `contents` is generator-owned — the first body line (after a shebang and
/// the SPDX/copyright header block) is an `@generated` marker. Such files are
/// check-only: their header flows from the generator, not this stamper. Matching only
/// the leading marker line, not any `@generated` mention, keeps the generators' OWN
/// hand-written source (which carries `@generated` inside its emitted-header string
/// literals and prose) firmly in the stamped set.
fn is_generated(contents: &str, style: Comment) -> bool {
    let (_, rest) = split_shebang(contents);
    let body = strip_leading_header(rest, style);
    next_line(body).is_some_and(|(line, _)| line.contains("@generated"))
}

/// Produce the canonical stamped form of `contents`: the current header at the top
/// (after any `#!` shebang), any prior recognised header replaced, exactly one blank
/// line before the body. Idempotent — stamping an already-stamped file is a no-op.
fn stamp(contents: &str, style: Comment, config: &LicenseConfig) -> String {
    let (shebang, rest) = split_shebang(contents);
    let body = strip_leading_header(rest, style);

    let prefix = style.prefix();
    let mut out = String::new();
    out.push_str(shebang);
    for line in config.content_lines() {
        out.push_str(prefix);
        out.push_str(&line);
        out.push('\n');
    }
    if !body.is_empty() {
        out.push('\n');
        out.push_str(body);
    }
    out
}

/// Split a leading `#!` shebang line (returned WITH its trailing newline) from the
/// rest. A Rust `#![...]` inner attribute is not a shebang, so it stays in the body.
fn split_shebang(contents: &str) -> (&str, &str) {
    if contents.starts_with("#!") && !contents.starts_with("#![") {
        let end = contents
            .find('\n')
            .map(|index| index + 1)
            .unwrap_or(contents.len());
        contents.split_at(end)
    } else {
        ("", contents)
    }
}

/// Drop a leading run of recognised header lines (our SPDX / copyright lines in this
/// file's comment syntax) plus the blank lines that follow, returning the remaining
/// body. Only the contiguous leading block is consumed, so unrelated `//` comments a
/// file happens to open with are preserved.
fn strip_leading_header(body: &str, style: Comment) -> &str {
    let mut rest = body;
    while let Some((line, tail)) = next_line(rest) {
        if is_header_line(line, style) {
            rest = tail;
        } else {
            break;
        }
    }
    // Collapse the blank separator lines left behind, so re-stamping stays stable.
    while let Some((line, tail)) = next_line(rest) {
        if line.trim().is_empty() && rest.contains('\n') {
            rest = tail;
        } else {
            break;
        }
    }
    rest
}

/// Split `text` into (first line without its terminator, remainder including the
/// following lines). Returns `None` when `text` is empty.
fn next_line(text: &str) -> Option<(&str, &str)> {
    if text.is_empty() {
        return None;
    }
    match text.find('\n') {
        Some(index) => Some((text[..index].trim_end_matches('\r'), &text[index + 1..])),
        None => Some((text, "")),
    }
}

/// Whether `line` is one of our header lines (an SPDX or copyright line in `style`'s
/// comment syntax), used to recognise — and thus replace, never duplicate — a header
/// this tool wrote before, and to absorb a bare pre-existing SPDX one-liner.
fn is_header_line(line: &str, style: Comment) -> bool {
    let Some(rest) = line.trim_start().strip_prefix(style.token()) else {
        return false;
    };
    let rest = rest.trim_start();
    rest.starts_with("SPDX-License-Identifier") || rest.starts_with("Copyright (c)")
}

/// Every first-party, stampable-format file under `root`, skipping build outputs and
/// vendored corpus subtrees.
///
/// The file list comes from `git ls-files`, not a filesystem walk: gitignored
/// environment artifacts (per-preset wasm `pkg-*` outputs, python `.venv`s, editor
/// caches) exist only in some workspaces and must never make `--check` verdicts differ
/// between a fresh worktree and a long-lived checkout. The directory-based skips remain
/// as filters over the tracked list (defence-in-depth for the vendored refusal, and so
/// a tracked-but-vendored path still cannot be stamped). Falls back to the filesystem
/// walk when `git` is unavailable.
fn stampable_files(root: &Path) -> Vec<PathBuf> {
    let mut files = match git_tracked_files(root) {
        Some(tracked) => tracked
            .into_iter()
            .filter(|path| {
                path.is_file()
                    && comment_style(path).is_some()
                    && !is_provenance_file(path)
                    && !path
                        .ancestors()
                        .take_while(|dir| dir.starts_with(root) && *dir != root)
                        .any(should_skip_dir)
            })
            .collect(),
        None => {
            let mut walked = Vec::new();
            collect_stampable(root, &mut walked);
            walked
        }
    };
    files.sort();
    files
}

/// The repo's tracked files per `git ls-files -z`, or `None` when git is unavailable.
fn git_tracked_files(root: &Path) -> Option<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    Some(
        stdout
            .split('\0')
            .filter(|rel| !rel.is_empty())
            .map(|rel| root.join(rel))
            .collect(),
    )
}

fn collect_stampable(dir: &Path, files: &mut Vec<PathBuf>) {
    for path in sorted_dir_entries(dir) {
        if path.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            collect_stampable(&path, files);
        } else if path.is_file() && comment_style(&path).is_some() && !is_provenance_file(&path) {
            files.push(path);
        }
    }
}

/// Whether the walk must not descend into `dir`: a build/dependency output, or a
/// vendored corpus subtree (which the stamper must never relicense).
fn should_skip_dir(dir: &Path) -> bool {
    let Some(name) = dir.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    if SKIP_DIR_NAMES.contains(&name) || SKIP_DIR_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return true;
    }
    is_vendored_corpus_dir(dir)
}

/// Whether `dir` is a vendored corpus root: a `corpus`/`corpora` directory that holds
/// vendored data (not a Rust module directory that merely shares the name, e.g.
/// `bench/benches/corpus/`).
fn is_vendored_corpus_dir(dir: &Path) -> bool {
    dir.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| VENDORED_CORPUS_ROOT_NAMES.contains(&name))
        && !is_rust_module_dir(dir)
}

/// Whether `path` lies within a vendored corpus subtree — the absolute exclusion the
/// stamper honours even when a vendored path is targeted directly (the "stamp-the-world
/// relicensing" failure mode ADR-0015 forbids).
fn is_within_vendored_corpus(root: &Path, path: &Path) -> bool {
    let mut current = path;
    while let Some(parent) = current.parent() {
        if (parent.starts_with(root) || parent == root) && is_vendored_corpus_dir(parent) {
            return true;
        }
        if parent == root {
            break;
        }
        current = parent;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> LicenseConfig {
        LicenseConfig {
            spdx: "MIT".to_owned(),
            holder: "Moderately AI Inc.".to_owned(),
            year: "2026".to_owned(),
        }
    }

    #[test]
    fn stamps_plain_rust_above_module_docs() {
        let input = "//! module docs\n\nuse std::fs;\n";
        let out = stamp(input, Comment::Slash, &config());
        assert_eq!(
            out,
            "// SPDX-License-Identifier: MIT\n\
             // Copyright (c) 2026 Moderately AI Inc.\n\
             \n\
             //! module docs\n\nuse std::fs;\n",
        );
    }

    #[test]
    fn stamping_is_idempotent() {
        let input = "//! module docs\n\nfn main() {}\n";
        let once = stamp(input, Comment::Slash, &config());
        let twice = stamp(&once, Comment::Slash, &config());
        assert_eq!(once, twice);
    }

    #[test]
    fn preserves_shebang_and_stamps_below_it() {
        let input = "#!/usr/bin/env node\nconsole.log(1);\n";
        let out = stamp(input, Comment::Slash, &config());
        assert!(out.starts_with("#!/usr/bin/env node\n// SPDX-License-Identifier: MIT\n"));
    }

    #[test]
    fn rust_inner_attribute_is_not_a_shebang() {
        let input = "#![no_main]\n//! fuzz target\n";
        let out = stamp(input, Comment::Slash, &config());
        assert!(out.starts_with("// SPDX-License-Identifier: MIT\n"));
        assert!(out.contains("\n\n#![no_main]\n"));
    }

    #[test]
    fn absorbs_bare_preexisting_spdx_line() {
        // The `.mjs` bench files already carry a lone SPDX one-liner followed by an
        // unrelated description comment: the SPDX line is replaced, the description
        // comment is preserved.
        let input = "// SPDX-License-Identifier: MIT\n// Shared corpus loader.\n";
        let out = stamp(input, Comment::Slash, &config());
        assert_eq!(
            out,
            "// SPDX-License-Identifier: MIT\n\
             // Copyright (c) 2026 Moderately AI Inc.\n\
             \n\
             // Shared corpus loader.\n",
        );
    }

    #[test]
    fn replaces_stale_spdx_id() {
        let input = "// SPDX-License-Identifier: Apache-2.0\n// Copyright (c) 2026 Moderately AI Inc.\n\nfn a() {}\n";
        let out = stamp(input, Comment::Slash, &config());
        assert!(out.starts_with("// SPDX-License-Identifier: MIT\n"));
        assert_eq!(out.matches("SPDX-License-Identifier").count(), 1);
    }

    #[test]
    fn stamps_hash_syntax_for_toml() {
        let input = "[package]\nname = \"x\"\n";
        let out = stamp(input, Comment::Hash, &config());
        assert_eq!(
            out,
            "# SPDX-License-Identifier: MIT\n\
             # Copyright (c) 2026 Moderately AI Inc.\n\
             \n\
             [package]\nname = \"x\"\n",
        );
    }

    #[test]
    fn generated_marker_is_recognised() {
        assert!(is_generated(
            "// SPDX-License-Identifier: MIT\n// Copyright (c) 2026 Moderately AI Inc.\n\n//! @generated by x\n",
            Comment::Slash,
        ));
        assert!(!is_generated("//! a hand-written module\n", Comment::Slash));
    }

    #[test]
    fn write_refuses_vendored_paths_but_stamps_first_party() {
        use std::env;
        use std::process;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("after epoch")
            .as_nanos();
        let root = env::temp_dir().join(format!("squonk-lh-vendored-{}-{nonce}", process::id()));
        fs::create_dir_all(root.join("conformance/corpus/postgres")).expect("mkdir corpus");
        fs::create_dir_all(root.join("crates/x/src")).expect("mkdir src");
        fs::write(
            root.join(CONFIG_FILE),
            "[license-header]\nspdx = \"MIT\"\nholder = \"Moderately AI Inc.\"\nyear = \"2026\"\n",
        )
        .expect("write config");
        let vendored = root.join("conformance/corpus/postgres/probe.sql");
        // A `.toml` inside the vendored tree is a stampable format — the stamper must
        // STILL refuse it because it lives under `corpus/`.
        let vendored_toml = root.join("conformance/corpus/postgres/PROVENANCE.toml");
        let first_party = root.join("crates/x/src/lib.rs");
        fs::write(&vendored, "SELECT 1;\n").expect("write vendored sql");
        fs::write(&vendored_toml, "source = \"upstream\"\n").expect("write vendored toml");
        fs::write(&first_party, "//! module\n").expect("write first-party");

        let summary = write_license_headers(&root).expect("write succeeds");

        assert_eq!(
            fs::read_to_string(&vendored).expect("read vendored"),
            "SELECT 1;\n",
            "vendored corpus file must be untouched",
        );
        assert_eq!(
            fs::read_to_string(&vendored_toml).expect("read vendored toml"),
            "source = \"upstream\"\n",
            "vendored toml must be untouched even though `.toml` is a stamped format",
        );
        assert!(
            fs::read_to_string(&first_party)
                .expect("read first-party")
                .starts_with("// SPDX-License-Identifier: MIT\n"),
            "first-party file must be stamped",
        );
        // The first-party `lib.rs` and the root `license-header.toml` config are both
        // stampable; the vendored `.sql`/`.toml` are not counted (walker skipped them).
        assert!(summary.contains("2 file(s) stamped"), "summary: {summary}");
        assert!(
            is_within_vendored_corpus(&root, &vendored),
            "vendored guard recognises the corpus path",
        );
        assert!(
            !is_within_vendored_corpus(&root, &first_party),
            "vendored guard leaves first-party paths stampable",
        );

        let _ = fs::remove_dir_all(&root);
    }
}
