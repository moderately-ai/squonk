// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

mod feature_matrix;
mod license_header;
mod preflight;
mod semver;
pub use feature_matrix::run as run_feature_matrix;
pub use license_header::{check_license_headers, write_license_headers};
pub use preflight::run_preflight;
pub use semver::run as run_semver;

pub type TidyResult = Result<(), Vec<String>>;

const DIALECT_BANS: &[&str] = &["dialect_of!", ".is::<", "TypeId"];
const CORPUS_DIR_NAMES: &[&str] = &["corpus", "corpora", "fixtures", "testdata"];
/// Corpus-root names whose subtrees hold *vendored third-party* material, which
/// ADR-0015 requires to carry an upstream-provenance record. The other corpus
/// roots (`fixtures`/`testdata`) hold first-party generated data (e.g. the
/// datadriven goldens) covered by this repository's own licence; they still get
/// the per-file SPDX scan but need no upstream provenance.
const VENDORED_CORPUS_ROOT_NAMES: &[&str] = &["corpus", "corpora"];
/// Per-group provenance manifest the gate enforces inside every vendored corpus
/// group. A flat `key = "value"` TOML subset (hand-parsed so xtask stays
/// dependency-free, ADR-0017), recording where the corpus came from and how to
/// regenerate it.
const PROVENANCE_FILE: &str = "PROVENANCE.toml";
/// Fields every vendored corpus group's `PROVENANCE.toml` must carry, non-empty:
/// the upstream origin, the pinned commit/tag/version, the elected SPDX licence,
/// and how to reproduce/refresh the vendored files (ADR-0015).
const REQUIRED_PROVENANCE_KEYS: &[&str] = &["source", "reference", "license", "regenerate"];
const ALLOWED_SPDX_LICENSES: &[&str] = &[
    "0BSD",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC0-1.0",
    "ISC",
    "MIT",
    "MIT-0",
    "PostgreSQL",
    "Unicode-3.0",
    "Unlicense",
    "Zlib",
];
const ALLOWED_SPDX_EXCEPTIONS: &[&str] = &["LLVM-exception"];

/// External (non-workspace) crates allowed in the *published* crates'
/// (`squonk`, `squonk-ast`) runtime — normal + build — dependency closure.
///
/// ADR-0017 keeps the published crates dependency-free except for sanctioned
/// leaf / zero-subtree containers. `thin-vec` is the lone runtime such leaf: a
/// one-word (8 B) child-sequence container (ADR-0007) with zero transitive
/// dependencies whose `unsafe` is encapsulated, so our crates stay `unsafe`-free.
/// Adding a name here is a deliberate ADR-0017 decision — weigh the *transitive
/// tree*, prefer a leaf over a subtree-dragging crate — never a drive-by;
/// [`check_published_deps`] fails until the name is added, forcing the choice to
/// the surface for review.
///
/// The `serde` cluster is the deliberate exception ADR-0017 explicitly permits: it
/// is reachable ONLY through the non-default `serde` feature, so the DEFAULT build
/// still pulls nothing but `thin-vec` (`cargo tree -p squonk-ast -e normal`
/// shows only `thin-vec`). Unlike `thin-vec` it is NOT a leaf — the `derive`
/// feature drags `serde_derive` and its `syn`/`quote`/`proc-macro2`/`unicode-ident`
/// tree — but every one of those is a BUILD-TIME proc-macro dependency that never
/// reaches the runtime artifact, and none is compiled unless a consumer opts in.
/// This flat allowlist cannot express "opt-in only", so the whole opt-in closure is
/// listed and self-contained (each entry's own lock deps are also listed); a
/// feature-aware gate that excused opt-in deps from the published surface entirely
/// is the more principled follow-up, left out here to keep the gate dependency-free.
const PUBLISHED_DEP_ALLOWLIST: &[&str] = &[
    "thin-vec",
    // Opt-in `serde` feature closure (build-time proc-macros; runtime-free).
    "serde",
    "serde_core",
    "serde_derive",
    "proc-macro2",
    "quote",
    "syn",
    "unicode-ident",
];

/// An upstream origin ADR-0015 forbids vendoring as a corpus, matched as a
/// case-insensitive substring of a provenance `source`/`reference` value or a
/// corpus group directory name. This is the "cannot be added silently" guard: it
/// fires on origin even when the corpus is (mis)labelled with a permissive
/// licence, so a GPL/BSL suite cannot slip in past the per-file SPDX scan.
struct DisallowedSource {
    needle: &'static str,
    reason: &'static str,
}

/// ADR-0015's named exclusions: the GPL MySQL/MariaDB test suites (run the engine
/// as an accept/reject oracle, do not vendor them) and CockroachDB's BSL testdata
/// (reuse the `datadriven` *format* only, never the copied test files).
const DISALLOWED_CORPUS_SOURCES: &[DisallowedSource] = &[
    DisallowedSource {
        needle: "mysql-test",
        reason: "the MySQL `mysql-test` suite is GPL; ADR-0015 says run the MySQL/MariaDB engine as an accept/reject oracle, do not vendor it",
    },
    DisallowedSource {
        needle: "mysql-server",
        reason: "the MySQL server tree is GPL; ADR-0015 says run the engine as an oracle, do not vendor its test data",
    },
    DisallowedSource {
        needle: "mariadb-server",
        reason: "the MariaDB server tree is GPL; ADR-0015 says run the engine as an oracle, do not vendor its test data",
    },
    DisallowedSource {
        needle: "cockroach",
        reason: "CockroachDB testdata is BSL; ADR-0015 permits reusing the datadriven *format* only, never copied CockroachDB test data",
    },
];

/// Copyleft / source-available licence families ADR-0015 disallows for vendored
/// corpora, matched as a case-insensitive substring of any SPDX token. `GPL`
/// already subsumes `AGPL`/`LGPL`; the redundant entries keep this list
/// self-documenting. `BUSL`/`BSL` is CockroachDB's Business Source Licence.
const DISALLOWED_LICENSE_FAMILIES: &[&str] = &[
    "GPL", "AGPL", "LGPL", "BUSL", "BSL", "SSPL", "CC-BY-SA", "CC-BY-NC", "CC-BY-ND",
];
const DISALLOWED_LICENSE_REASON: &str = "ADR-0015 disallows copyleft/source-available corpora (GPL/AGPL/LGPL/BUSL/SSPL/CC-BY-*): run the engine as an oracle, or reuse only the format, never vendor the data";

/// A single tidy gate: the function `cargo xtask <name>` invokes.
type Check = fn(&Path) -> TidyResult;

/// Every local tidy gate, keyed by its `cargo xtask <name>` subcommand name.
/// `check_all` folds over this, the CLI dispatch looks a name up in it, and
/// `usage()` derives its subcommand list from it — one array entry registers a
/// gate everywhere it's needed, instead of three call sites kept in sync by hand.
pub const CHECKS: &[(&str, Check)] = &[
    ("license", check_corpus_licenses),
    ("license-headers", check_license_headers),
    ("dialect", check_dialect_dispatch_ban),
    ("deps", check_published_deps),
    ("extension-seam", check_extension_seam),
    ("precedence", check_precedence),
    ("dialect-generic", check_dialect_generic),
];

/// Run every local tidy gate.
pub fn check_all(root: impl AsRef<Path>) -> TidyResult {
    let root = root.as_ref();
    let mut errors = Vec::new();
    for &(_, check) in CHECKS {
        collect(&mut errors, check(root));
    }
    finish(errors)
}

/// Ensure every vendored corpus subtree is licence-clean *and* provenance-backed.
///
/// Two layers, both mandated by ADR-0015 ("vendor only permissive corpora … a
/// REUSE/SPDX check guards the vendor subtree") and kept local-runnable per
/// ADR-0017:
///
/// 1. **Per-file SPDX.** Every file under a corpus-like subtree
///    (`corpus`/`corpora`/`fixtures`/`testdata`) must carry a permissive
///    `SPDX-License-Identifier:` — inline or via a REUSE-style `<file>.license`
///    companion. A non-permissive marker fails; a GPL/BSL/SSPL/CC-BY-* family
///    marker fails with the specific ADR-0015 citation so it cannot land
///    silently.
///
/// 2. **Per-group provenance.** Every *vendored* corpus group (an immediate
///    subdirectory of a `corpus`/`corpora` root, plus the root itself when it
///    holds files directly) must carry a `PROVENANCE_FILE` recording the
///    upstream `source`, the pinned `reference` (commit/tag/version), the elected
///    `license`, and `regenerate` instructions. Missing metadata fails. The
///    `source`/`reference` and the group directory name are screened against
///    ADR-0015's disallowed origins (`DISALLOWED_CORPUS_SOURCES`) so a
///    disallowed corpus cannot be added silently even if mislabelled permissive.
///
/// First-party generated data under `fixtures`/`testdata` (e.g. the datadriven
/// goldens) has no upstream, so layer 2 applies only to the vendored roots; it is
/// still covered by layer 1 and this repository's own licence.
pub fn check_corpus_licenses(root: &Path) -> TidyResult {
    let mut errors = Vec::new();
    for corpus_root in find_corpus_roots(root) {
        check_corpus_file_spdx(root, &corpus_root, &mut errors);
        if is_vendored_corpus_root(&corpus_root) {
            for group in corpus_groups(&corpus_root) {
                check_group_provenance(root, &group, &mut errors);
            }
        }
    }
    finish(errors)
}

/// Layer 1: every non-metadata file under `corpus_root` carries a permissive SPDX
/// marker (inline or `.license` companion).
fn check_corpus_file_spdx(root: &Path, corpus_root: &Path, errors: &mut Vec<String>) {
    let mut files = Vec::new();
    collect_files(corpus_root, &mut files, SkipMode::Repo);
    for file in files {
        if is_license_metadata(&file) || is_provenance_file(&file) {
            continue;
        }
        match spdx_for_file(&file) {
            Ok(Some(expr)) if spdx_expression_is_allowed(&expr) => {}
            Ok(Some(expr)) => errors.push(non_permissive_spdx_message(root, &file, &expr)),
            Ok(None) => errors.push(format!(
                "{}: missing SPDX-License-Identifier; add a permissive marker — either an inline \
                 `SPDX-License-Identifier: MIT` header in the file's own comment syntax \
                 (e.g. `-- SPDX-License-Identifier: MIT` for SQL), or a REUSE companion `{}` whose \
                 sole line is `SPDX-License-Identifier: MIT`. Permissive licences only (ADR-0015); \
                 allowed: {}",
                display_path(root, &file),
                companion_license_path(&file)
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or("<file>.license"),
                ALLOWED_SPDX_LICENSES.join(", "),
            )),
            Err(err) => errors.push(format!("{}: {err}", display_path(root, &file))),
        }
    }
}

/// Layer 2: a vendored corpus group must carry a complete, allowed, non-disallowed
/// [`PROVENANCE_FILE`].
fn check_group_provenance(root: &Path, group: &Path, errors: &mut Vec<String>) {
    // Screen the directory name itself first: a group literally named after a
    // banned origin (e.g. `mysql-test`) is rejected even before its provenance is
    // read, so it cannot be staged silently (ADR-0015).
    if let Some(name) = group.file_name().and_then(OsStr::to_str) {
        if let Some(reason) = disallowed_source_hit(name) {
            errors.push(format!(
                "{}: corpus group directory names a disallowed source — {reason}",
                display_path(root, group),
            ));
        }
    }

    let provenance = group.join(PROVENANCE_FILE);
    if !provenance.is_file() {
        errors.push(format!(
            "{}: missing {PROVENANCE_FILE}; a vendored corpus group needs one here — a flat TOML naming \
             `source = \"<upstream URL>\"`, `reference = \"<pinned commit/tag/version>\"`, \
             `license = \"<permissive SPDX, e.g. MIT>\"`, and `regenerate = \"<how to refresh it>\"` \
             (all of: {}) per ADR-0015",
            display_path(root, group),
            REQUIRED_PROVENANCE_KEYS.join(", "),
        ));
        return;
    }

    let fields = match read_provenance(&provenance) {
        Ok(fields) => fields,
        Err(err) => {
            errors.push(format!("{}: {err}", display_path(root, &provenance)));
            return;
        }
    };

    for key in REQUIRED_PROVENANCE_KEYS {
        if fields.get(*key).is_none_or(|value| value.is_empty()) {
            errors.push(format!(
                "{}: missing or empty `{key}` provenance field (required by ADR-0015)",
                display_path(root, &provenance),
            ));
        }
    }

    for key in ["source", "reference"] {
        if let Some(value) = fields.get(key) {
            if let Some(reason) = disallowed_source_hit(value) {
                errors.push(format!(
                    "{}: `{key}` names a disallowed corpus source — {reason}",
                    display_path(root, &provenance),
                ));
            }
        }
    }

    if let Some(license) = fields.get("license").filter(|value| !value.is_empty()) {
        if disallowed_license_family(license).is_some() {
            errors.push(format!(
                "{}: `license = {license}` is disallowed for vendored corpora — {DISALLOWED_LICENSE_REASON}",
                display_path(root, &provenance),
            ));
        } else if !spdx_expression_is_allowed(license) {
            errors.push(format!(
                "{}: `license = {license}` is not in the permissive corpus allowlist (ADR-0015); use one of: {}",
                display_path(root, &provenance),
                ALLOWED_SPDX_LICENSES.join(", "),
            ));
        }
    }
}

/// Ban TypeId/Any-style dialect dispatch in the parser crate.
pub fn check_dialect_dispatch_ban(root: &Path) -> TidyResult {
    let parser_src = root.join("crates/squonk/src");
    if !parser_src.is_dir() {
        return Err(vec![format!(
            "{}: parser source directory does not exist",
            display_path(root, &parser_src),
        )]);
    }

    let mut files = Vec::new();
    collect_files(&parser_src, &mut files, SkipMode::None);

    let mut errors = Vec::new();
    for file in files {
        if file.extension().and_then(OsStr::to_str) != Some("rs") {
            continue;
        }
        let text = match fs::read_to_string(&file) {
            Ok(text) => text,
            Err(err) => {
                errors.push(format!("{}: {err}", display_path(root, &file)));
                continue;
            }
        };
        for (line_index, line) in text.lines().enumerate() {
            for banned in DIALECT_BANS {
                if line.contains(banned) {
                    errors.push(format!(
                        "{}:{}: banned dialect-dispatch pattern `{banned}` — a dialect is DATA (ADR-0011): \
                         branch on a `self.features.<field>` FeatureSet flag, never on the concrete \
                         dialect type via `dialect_of!`/`.is::<>`/`TypeId`; add the deciding flag to \
                         `FeatureSet` if one does not exist yet",
                        display_path(root, &file),
                        line_index + 1,
                    ));
                }
            }
        }
    }
    finish(errors)
}

/// Guard ADR-0017 dependency minimalism: the *published* crates' runtime
/// dependency surface must stay within `PUBLISHED_DEP_ALLOWLIST`.
///
/// cargo-deny owns the whole-graph supply-chain checks (advisories, licences,
/// sources, duplicate versions) but cannot scope an allowlist to a single crate's
/// normal-edge subtree, so the published-surface allowlist lives here instead —
/// local-runnable per ADR-0017, mirroring `cargo xtask license`. Two layers keep
/// the guarantee closed over the dependency relation without a full graph engine:
///
/// 1. **Direct (manifests).** Every workspace member that is *published* (no
///    `publish = false`) may declare, in `[dependencies]` / `[build-dependencies]`
///    (`[dev-dependencies]` ship to nobody and are ignored), only another
///    published workspace crate or a name on `PUBLISHED_DEP_ALLOWLIST`. A stray
///    `regex = "1"` on `squonk` fails here.
///
/// 2. **Closed-set (`Cargo.lock`).** Every allowlisted external crate must itself
///    pull only allowlisted crates — its lock `dependencies` ⊆ the allowlist — so
///    a sanctioned leaf that sprouts a subtree upstream (e.g. a future `thin-vec`
///    that grows a dependency) fails too. Layers 1 and 2 together prove the
///    published runtime surface ⊆ `PUBLISHED_DEP_ALLOWLIST`.
pub fn check_published_deps(root: &Path) -> TidyResult {
    let mut errors = Vec::new();

    let root_manifest = root.join("Cargo.toml");
    let root_text = match fs::read_to_string(&root_manifest) {
        Ok(text) => text,
        Err(err) => {
            return Err(vec![format!(
                "{}: {err}",
                display_path(root, &root_manifest)
            )]);
        }
    };

    // Resolve the published workspace members and their declared runtime deps.
    let mut published_names = std::collections::BTreeSet::new();
    let mut published: Vec<(String, PathBuf, Vec<String>)> = Vec::new();
    for member in parse_workspace_members(&root_text) {
        let manifest_path = root.join(&member).join("Cargo.toml");
        let text = match fs::read_to_string(&manifest_path) {
            Ok(text) => text,
            Err(err) => {
                errors.push(format!("{}: {err}", display_path(root, &manifest_path)));
                continue;
            }
        };
        if !manifest_is_published(&text) {
            continue;
        }
        let name = manifest_package_name(&text)
            .unwrap_or_else(|| member.rsplit('/').next().unwrap_or(&member).to_owned());
        published_names.insert(name.clone());
        published.push((name, manifest_path, parse_runtime_dep_names(&text)));
    }

    // Layer 1: direct runtime dependencies of each published crate.
    for (name, manifest_path, deps) in &published {
        for dep in deps {
            if published_names.contains(dep) || PUBLISHED_DEP_ALLOWLIST.contains(&dep.as_str()) {
                continue;
            }
            errors.push(format!(
                "{}: published crate `{name}` declares runtime dependency `{dep}`, \
                 which is neither a published workspace crate nor on the ADR-0017 \
                 published-dependency allowlist ({}); add it to PUBLISHED_DEP_ALLOWLIST \
                 in xtask only after weighing its transitive tree, or move it to \
                 [dev-dependencies]",
                display_path(root, manifest_path),
                PUBLISHED_DEP_ALLOWLIST.join(", "),
            ));
        }
    }

    // Layer 2: every allowlisted external crate must pull only allowlisted crates.
    let lock_path = root.join("Cargo.lock");
    if lock_path.is_file() {
        match fs::read_to_string(&lock_path) {
            Ok(lock_text) => {
                let locked = parse_lock_dependencies(&lock_text);
                for leaf in PUBLISHED_DEP_ALLOWLIST {
                    let Some(deps) = locked.get(*leaf) else {
                        continue;
                    };
                    for dep in deps {
                        if !PUBLISHED_DEP_ALLOWLIST.contains(&dep.as_str()) {
                            errors.push(format!(
                                "Cargo.lock: allowlisted published dependency `{leaf}` now pulls \
                                 `{dep}`, expanding the published transitive surface beyond the \
                                 ADR-0017 allowlist; re-evaluate `{leaf}` — its leaf / \
                                 zero-subtree justification no longer holds",
                            ));
                        }
                    }
                }
            }
            Err(err) => errors.push(format!("{}: {err}", display_path(root, &lock_path))),
        }
    }

    finish(errors)
}

/// The `[workspace] members = [...]` paths from the root manifest. Hand-rolled to
/// keep xtask dependency-free (ADR-0017); the members array is a flat list of
/// quoted relative paths.
fn parse_workspace_members(manifest: &str) -> Vec<String> {
    let Some(start) = manifest.find("members") else {
        return Vec::new();
    };
    let after = &manifest[start..];
    let Some(open) = after.find('[') else {
        return Vec::new();
    };
    let tail = &after[open..];
    let Some(close) = tail.find(']') else {
        return Vec::new();
    };
    quoted_strings(&tail[..=close])
}

/// The `[package] name = "…"` from a member manifest.
fn manifest_package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_package = line == "[package]";
            continue;
        }
        if in_package {
            if let Some(rest) = line.strip_prefix("name") {
                if let Some(value) = rest.trim_start().strip_prefix('=') {
                    return Some(value.trim().trim_matches('"').to_owned());
                }
            }
        }
    }
    None
}

/// Whether a member manifest is published — i.e. it does not opt out with
/// `publish = false`. The published crates are exactly those whose dependency
/// surface this gate constrains.
fn manifest_is_published(manifest: &str) -> bool {
    !manifest
        .lines()
        .any(|line| line.trim().replace(' ', "") == "publish=false")
}

/// Crate names declared in the *runtime* dependency tables (`[dependencies]`,
/// `[build-dependencies]`, and their `[target.'…'.…]` variants) of `manifest`.
/// `[dev-dependencies]` are deliberately skipped — they never reach a published
/// crate's downstream users (ADR-0017). A dependency name is the key before `=`,
/// with any dotted-key suffix (`foo.workspace = true`) stripped.
fn parse_runtime_dep_names(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_runtime_section = false;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_runtime_section = is_runtime_dep_section(&line[1..line.len() - 1]);
            continue;
        }
        if !in_runtime_section || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        let name = key.trim().split('.').next().unwrap_or("").trim();
        if !name.is_empty() {
            names.push(name.to_owned());
        }
    }
    names
}

/// Whether a TOML section header (the text inside the brackets) names a *runtime*
/// dependency table — plain or `target.'cfg(…)'.…` — i.e. its final component is
/// `dependencies` or `build-dependencies`, but not `dev-dependencies`.
fn is_runtime_dep_section(header: &str) -> bool {
    let last = header.trim().rsplit('.').next().unwrap_or("").trim();
    matches!(last, "dependencies" | "build-dependencies")
}

/// Map every `[[package]]` in a `Cargo.lock` to the crate names in its
/// `dependencies` array. The lock records normal + build edges; transitive
/// dev-dependencies of non-workspace crates are absent, which is exactly the
/// runtime surface this gate cares about. Each entry is `"name"` or
/// `"name version …"`; only the leading name token is kept.
fn parse_lock_dependencies(lock: &str) -> BTreeMap<String, Vec<String>> {
    let mut map = BTreeMap::new();
    let mut name: Option<String> = None;
    let mut deps: Vec<String> = Vec::new();
    let mut in_deps = false;

    for raw in lock.lines() {
        let line = raw.trim();
        if line == "[[package]]" {
            if let Some(previous) = name.take() {
                map.insert(previous, std::mem::take(&mut deps));
            }
            in_deps = false;
        } else if let Some(rest) = line.strip_prefix("name = ") {
            name = Some(rest.trim().trim_matches('"').to_owned());
            in_deps = false;
        } else if let Some(rest) = line.strip_prefix("dependencies = [") {
            push_lock_dep_names(rest, &mut deps);
            in_deps = !rest.contains(']');
        } else if in_deps {
            if line.starts_with(']') {
                in_deps = false;
            } else {
                push_lock_dep_names(line, &mut deps);
            }
        }
    }
    if let Some(previous) = name.take() {
        map.insert(previous, deps);
    }
    map
}

/// Append the leading name token of each quoted lock dependency entry in `line`.
fn push_lock_dep_names(line: &str, deps: &mut Vec<String>) {
    for entry in quoted_strings(line) {
        if let Some(crate_name) = entry.split_whitespace().next() {
            deps.push(crate_name.to_owned());
        }
    }
}

/// The double-quoted spans in `text`, in order. Shared by the workspace-member and
/// `Cargo.lock` parsers, both of which read a small, well-formed TOML subset where
/// a scan for `"…"` is enough (hand-rolled to keep xtask dependency-free,
/// ADR-0017).
fn quoted_strings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else {
            break;
        };
        out.push(after[..end].to_owned());
        rest = &after[end + 1..];
    }
    out
}

/// ADR-0009 anchors: the extension seam's trait bound, its uninhabited stock type,
/// and the openable-enum set.
const EXTENSION_FILE: &str = "crates/squonk-ast/src/ast/ext.rs";
const EXTENSION_TRAIT_BOUND: &str = "pub trait Extension: Clone + Debug + Eq + Hash + Spanned {}";
const EXTENSION_BLANKET_IMPL: &str =
    "impl<T: Clone + Debug + Eq + Hash + Spanned> Extension for T {}";
const EXTENSION_NOEXT: &str = "pub enum NoExt {}";
const EXTENSION_SEAM_DIR: &str = "crates/squonk-ast/src/ast";
const EXTENSION_SEAM_FIELD: &str = "ext: X,";
const EXTENSION_SEAM_COUNT: usize = 6;

/// Pin the ADR-0009 extension seam: the `Extension` supertrait bound, its blanket
/// impl, the uninhabited `NoExt`, and the exactly-five openable enums.
///
/// The trait bound is asserted as a source line because ADR-0009's
/// prose had drifted from it (the ADR named `Render`/`Visit` bounds the code
/// applies at call sites instead) — this gate closes that drift. The blanket
/// `impl<T: …> Extension for T {}` is pinned for the same anti-drift reason: it is
/// the bound-alias-forever decision ADR-0009 records (behaviour never lands on
/// `Extension`; a new opt-in trait would), so dropping or narrowing it is a design
/// change to re-decide, not a silent refactor. The six `ext: X` field seams are the
/// whole openable surface (`Statement`, `Expr`, `TableFactor`, `ColumnOption`,
/// `TableConstraint`, `DataType`); a seventh added, or one dropped, without amending
/// ADR-0009 fails here.
pub fn check_extension_seam(root: &Path) -> TidyResult {
    let mut errors = Vec::new();

    let ext_path = root.join(EXTENSION_FILE);
    match fs::read_to_string(&ext_path) {
        Ok(text) => {
            if !contains_trimmed_line(&text, EXTENSION_TRAIT_BOUND) {
                errors.push(format!(
                    "{}: the `Extension` supertrait bound must stay exactly `{EXTENSION_TRAIT_BOUND}` (ADR-0009); Render/Visit are applied at their call sites, not on the trait",
                    display_path(root, &ext_path),
                ));
            }
            if !contains_trimmed_line(&text, EXTENSION_BLANKET_IMPL) {
                errors.push(format!(
                    "{}: the blanket `{EXTENSION_BLANKET_IMPL}` must stay exact — ADR-0009 records `Extension` as a bound-alias forever (behaviour lands on Render/Visit at call sites, never on the trait; new behaviour is a new opt-in trait), and the `DynExt` hatch depends on the zero-boilerplate property",
                    display_path(root, &ext_path),
                ));
            }
            if !contains_trimmed_line(&text, EXTENSION_NOEXT) {
                errors.push(format!(
                    "{}: the stock extension type must stay the uninhabited `{EXTENSION_NOEXT}` (ADR-0009), so `Other(NoExt)` is statically dead",
                    display_path(root, &ext_path),
                ));
            }
        }
        Err(err) => errors.push(format!("{}: {err}", display_path(root, &ext_path))),
    }

    let seam_dir = root.join(EXTENSION_SEAM_DIR);
    if seam_dir.is_dir() {
        let count = count_trimmed_line(&seam_dir, EXTENSION_SEAM_FIELD);
        if count != EXTENSION_SEAM_COUNT {
            errors.push(format!(
                "{}: found {count} `{EXTENSION_SEAM_FIELD}` extension seams, expected exactly {EXTENSION_SEAM_COUNT} (Statement, Expr, TableFactor, ColumnOption, TableConstraint, DataType) per ADR-0009",
                display_path(root, &seam_dir),
            ));
        }
    } else {
        errors.push(format!(
            "{}: AST source directory does not exist",
            display_path(root, &seam_dir),
        ));
    }

    finish(errors)
}

/// ADR-0008 anchors: the banned paren node and the single set-op precedence source.
const PRECEDENCE_AST_DIR: &str = "crates/squonk-ast/src";
const NESTED_EXPR_BAN: &str = "Expr::Nested";
const SET_EXPR_FILE: &str = "crates/squonk/src/parser/query.rs";
const SET_EXPR_FN: &str = "parse_set_expr";
const SET_OP_BINDING_POWER: &str = "set_operation_binding_power";

/// Pin ADR-0008's "one binding-power table" discipline.
///
/// Two facets: (a) the AST crate carries no `Expr::Nested` paren node — parentheses
/// are derived at render from the binding-power table, never stored (like the
/// `dialect_of!` ban, a banned source pattern); (b) the set-operation parser reads
/// its precedence from `set_operation_binding_power` (the one table), not a second
/// hand-rolled fold — the exact "precedence decided in a second place" regression
/// ADR-0008 exists to prevent.
pub fn check_precedence(root: &Path) -> TidyResult {
    let mut errors = Vec::new();

    let ast_src = root.join(PRECEDENCE_AST_DIR);
    if ast_src.is_dir() {
        let mut files = Vec::new();
        collect_files(&ast_src, &mut files, SkipMode::None);
        for file in files {
            if file.extension().and_then(OsStr::to_str) != Some("rs") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };
            for (line_index, line) in text.lines().enumerate() {
                if line.contains(NESTED_EXPR_BAN) {
                    errors.push(format!(
                        "{}:{}: banned `{NESTED_EXPR_BAN}` — parentheses are derived at render, not stored as an AST node (ADR-0008)",
                        display_path(root, &file),
                        line_index + 1,
                    ));
                }
            }
        }
    } else {
        errors.push(format!(
            "{}: AST source directory does not exist",
            display_path(root, &ast_src),
        ));
    }

    let query_path = root.join(SET_EXPR_FILE);
    match fs::read_to_string(&query_path) {
        Ok(text) => {
            if !(text.contains(SET_EXPR_FN) && text.contains(SET_OP_BINDING_POWER)) {
                errors.push(format!(
                    "{}: `{SET_EXPR_FN}` must read set-operator precedence from `{SET_OP_BINDING_POWER}` (the one binding-power table), not a hand-rolled fold (ADR-0008)",
                    display_path(root, &query_path),
                ));
            }
        }
        Err(err) => errors.push(format!("{}: {err}", display_path(root, &query_path))),
    }

    finish(errors)
}

/// ADR-0011 anchors: the "generic" = ANSI mapping, the ANSI preset, and the banned
/// separate Generic preset/variant.
const DIALECT_BUILTIN_FILE: &str = "crates/squonk/src/dialect/builtin.rs";
const GENERIC_ALIAS: &str = "eq_ignore_ascii_case(\"generic\")";
const ANSI_VARIANT: &str = "Self::Ansi";
const ANSI_PRESET_FILE: &str = "crates/squonk-ast/src/dialect/ansi.rs";
const ANSI_PRESET_DECL: &str = "pub const ANSI:";
const DIALECT_SRC_DIRS: &[&str] = &["crates/squonk/src/dialect", "crates/squonk-ast/src/dialect"];
const GENERIC_PRESET_BANS: &[&str] = &["FeatureSet::GENERIC", "BuiltinDialect::Generic"];

/// Pin ADR-0011's "generic = ANSI, never a vibe-union" rule.
///
/// (a) `BuiltinDialect::from_name` maps the runtime name `"generic"` to the ANSI
/// baseline variant; (b) the `FeatureSet::ANSI` preset it resolves to exists; and
/// (c) no separate `Generic` preset/variant is defined — `Lenient` is the only
/// explicit permissive union. Prevents reintroducing the permissive "generic"
/// catch-all the prior art shipped.
pub fn check_dialect_generic(root: &Path) -> TidyResult {
    let mut errors = Vec::new();

    let builtin_path = root.join(DIALECT_BUILTIN_FILE);
    match fs::read_to_string(&builtin_path) {
        Ok(text) => {
            if !generic_maps_to_ansi(&text) {
                errors.push(format!(
                    "{}: `from_name` must map the runtime name `\"generic\"` to `{ANSI_VARIANT}` (the strict SQL:2016 baseline), not a separate preset (ADR-0011)",
                    display_path(root, &builtin_path),
                ));
            }
        }
        Err(err) => errors.push(format!("{}: {err}", display_path(root, &builtin_path))),
    }

    let ansi_path = root.join(ANSI_PRESET_FILE);
    match fs::read_to_string(&ansi_path) {
        Ok(text) => {
            if !text.contains(ANSI_PRESET_DECL) {
                errors.push(format!(
                    "{}: the `{ANSI_PRESET_DECL} …` FeatureSet preset (the `\"generic\"` baseline) must exist (ADR-0011)",
                    display_path(root, &ansi_path),
                ));
            }
        }
        Err(err) => errors.push(format!("{}: {err}", display_path(root, &ansi_path))),
    }

    for dir in DIALECT_SRC_DIRS {
        let dir_path = root.join(dir);
        let mut files = Vec::new();
        collect_files(&dir_path, &mut files, SkipMode::None);
        for file in files {
            if file.extension().and_then(OsStr::to_str) != Some("rs") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };
            for (line_index, line) in text.lines().enumerate() {
                for banned in GENERIC_PRESET_BANS {
                    if line.contains(banned) {
                        errors.push(format!(
                            "{}:{}: banned `{banned}` — `\"generic\"` must resolve to the ANSI baseline, with no separate Generic preset/variant (ADR-0011)",
                            display_path(root, &file),
                            line_index + 1,
                        ));
                    }
                }
            }
        }
    }

    finish(errors)
}

/// Whether `from_name`'s `"generic"` alias branch returns the ANSI variant: scan
/// the few lines from the alias to its `return`, asserting `Self::Ansi` appears
/// before any other returned variant.
fn generic_maps_to_ansi(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().collect();
    let Some(pos) = lines.iter().position(|line| line.contains(GENERIC_ALIAS)) else {
        return false;
    };
    for line in lines[pos..].iter().take(4) {
        if line.contains(ANSI_VARIANT) {
            return true;
        }
        if line.contains("return") {
            return false;
        }
    }
    false
}

/// Whether any line of `text`, trimmed, equals `needle`.
fn contains_trimmed_line(text: &str, needle: &str) -> bool {
    text.lines().any(|line| line.trim() == needle)
}

/// Count lines whose trimmed content equals `needle` across the `.rs` files under
/// `dir` (recursively).
fn count_trimmed_line(dir: &Path, needle: &str) -> usize {
    let mut files = Vec::new();
    collect_files(dir, &mut files, SkipMode::None);
    files
        .iter()
        .filter(|file| file.extension().and_then(OsStr::to_str) == Some("rs"))
        .filter_map(|file| fs::read_to_string(file).ok())
        .map(|text| text.lines().filter(|line| line.trim() == needle).count())
        .sum()
}

/// Find the workspace root from the current working directory.
pub fn find_workspace_root() -> Result<PathBuf, String> {
    let mut dir = env::current_dir().map_err(|err| format!("read current directory: {err}"))?;
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("crates").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err(
                "could not find workspace root containing Cargo.toml and crates/".to_owned(),
            );
        }
    }
}

fn collect(errors: &mut Vec<String>, result: TidyResult) {
    if let Err(mut found) = result {
        errors.append(&mut found);
    }
}

fn finish(errors: Vec<String>) -> TidyResult {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn find_corpus_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    collect_corpus_roots(root, &mut roots);
    roots
}

fn collect_corpus_roots(dir: &Path, roots: &mut Vec<PathBuf>) {
    let entries = sorted_dir_entries(dir);
    for path in entries {
        if !path.is_dir() || should_skip_repo_dir(&path) || is_cargo_fuzz_dir(&path) {
            continue;
        }
        if path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| CORPUS_DIR_NAMES.contains(&name))
        {
            // A Rust module directory that merely shares a corpus-root name (e.g.
            // the `bench/benches/corpus/` benchmark module) is first-party source,
            // not vendored third-party data, so it is not a corpus to license-scan.
            // Skip it rather than demand SPDX + provenance on Rust source.
            if is_rust_module_dir(&path) {
                continue;
            }
            roots.push(path);
        } else {
            collect_corpus_roots(&path, roots);
        }
    }
}

/// Whether `path` is a cargo-fuzz crate directory.
///
/// A cargo-fuzz crate keeps its libFuzzer inputs in a generated, gitignored
/// `corpus/` (plus `artifacts/`) — fuzzer-minted bytes, not vendored
/// license-bearing corpora — so the SPDX corpus gate skips the whole crate. The
/// `cargo-fuzz = true` package metadata marker is what identifies it, so a plain
/// directory that happens to be named `fuzz` is not skipped.
fn is_cargo_fuzz_dir(path: &Path) -> bool {
    path.file_name().and_then(OsStr::to_str) == Some("fuzz")
        && fs::read_to_string(path.join("Cargo.toml"))
            .is_ok_and(|manifest| manifest.contains("cargo-fuzz = true"))
}

/// Whether `path` is a Rust *module* directory (one that owns a `mod.rs`).
///
/// Used to tell a vendored DATA corpus apart from a benchmark/test module that
/// merely shares a corpus-root name — e.g. `bench/benches/corpus/`, the
/// corpus-scale parser bench. A vendored corpus holds `.sql`/`.txt` data and never
/// a `mod.rs`; a module directory is first-party source covered by this repo's own
/// licence, scanned by the crate-wide source tooling, not the corpus SPDX/provenance
/// gate. So such a directory is skipped as a corpus root, the same way a cargo-fuzz
/// crate's generated input `corpus/` is.
fn is_rust_module_dir(path: &Path) -> bool {
    path.join("mod.rs").is_file()
}

/// Whether `path` is a vendored corpus root (one whose subtree ADR-0015 requires
/// to carry upstream provenance), as opposed to a first-party `fixtures`/`testdata`
/// root.
fn is_vendored_corpus_root(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| VENDORED_CORPUS_ROOT_NAMES.contains(&name))
}

/// The vendored corpus *groups* under `corpus_root`: each immediate subdirectory
/// that owns at least one corpus file, plus `corpus_root` itself when it holds
/// corpus files directly. Provenance is enforced per group, matching the
/// `corpus/<group>/…` layout.
fn corpus_groups(corpus_root: &Path) -> Vec<PathBuf> {
    let mut groups = Vec::new();
    let mut root_owns_files = false;
    for path in sorted_dir_entries(corpus_root) {
        if path.is_dir() {
            if should_skip_repo_dir(&path) || is_cargo_fuzz_dir(&path) {
                continue;
            }
            if dir_has_corpus_file(&path) {
                groups.push(path);
            }
        } else if path.is_file() && !is_license_metadata(&path) && !is_provenance_file(&path) {
            root_owns_files = true;
        }
    }
    if root_owns_files {
        groups.push(corpus_root.to_path_buf());
    }
    groups
}

/// Whether `dir` (recursively) owns any actual corpus file — one that is neither
/// licence metadata nor the provenance manifest.
fn dir_has_corpus_file(dir: &Path) -> bool {
    let mut files = Vec::new();
    collect_files(dir, &mut files, SkipMode::Repo);
    files
        .iter()
        .any(|file| !is_license_metadata(file) && !is_provenance_file(file))
}

fn is_provenance_file(path: &Path) -> bool {
    path.file_name().and_then(OsStr::to_str) == Some(PROVENANCE_FILE)
}

/// Parse a vendored corpus group's `PROVENANCE.toml`: a flat `key = "value"` TOML
/// subset (`#` comments and blank lines ignored). Hand-rolled so xtask stays
/// dependency-free (ADR-0017); a structured manifest beats grepping README prose,
/// keeping the gate deterministic.
fn read_provenance(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("read failed: {err}"))?;
    let mut fields = BTreeMap::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!(
                "{PROVENANCE_FILE}:{}: malformed line (expected `key = \"value\"`): {line}",
                index + 1,
            ));
        };
        let value = value.trim().trim_matches('"').trim().to_owned();
        fields.insert(key.trim().to_owned(), value);
    }
    Ok(fields)
}

/// Append ADR-0015's policy citation to a non-permissive SPDX rejection when the
/// licence is a disallowed copyleft/source-available family, so the failure names
/// *why* the corpus is barred rather than just "not on the allowlist".
fn non_permissive_spdx_message(root: &Path, file: &Path, expr: &str) -> String {
    let base = format!(
        "{}: SPDX license expression `{expr}` is not in the permissive corpus allowlist (ADR-0015); use one of: {}",
        display_path(root, file),
        ALLOWED_SPDX_LICENSES.join(", "),
    );
    match disallowed_license_family(expr) {
        Some(reason) => format!("{base} ({reason})"),
        None => base,
    }
}

/// The reason a value names an ADR-0015 disallowed corpus origin, if it does
/// (case-insensitive substring match).
fn disallowed_source_hit(value: &str) -> Option<&'static str> {
    let lower = value.to_ascii_lowercase();
    DISALLOWED_CORPUS_SOURCES
        .iter()
        .find(|source| lower.contains(source.needle))
        .map(|source| source.reason)
}

/// Whether an SPDX expression names a disallowed copyleft/source-available family
/// (any token, case-insensitive); returns the shared ADR-0015 policy reason.
fn disallowed_license_family(expression: &str) -> Option<&'static str> {
    let upper = expression.to_ascii_uppercase();
    DISALLOWED_LICENSE_FAMILIES
        .iter()
        .any(|family| upper.contains(family))
        .then_some(DISALLOWED_LICENSE_REASON)
}

#[derive(Clone, Copy)]
enum SkipMode {
    None,
    Repo,
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>, skip_mode: SkipMode) {
    for path in sorted_dir_entries(dir) {
        if path.is_dir() {
            if matches!(skip_mode, SkipMode::Repo) && should_skip_repo_dir(&path) {
                continue;
            }
            collect_files(&path, files, skip_mode);
        } else if path.is_file() {
            files.push(path);
        }
    }
}

fn sorted_dir_entries(dir: &Path) -> Vec<PathBuf> {
    let mut entries = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    entries.sort();
    entries
}

fn should_skip_repo_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(OsStr::to_str),
        Some(".git" | ".claude" | "target")
    )
}

fn spdx_for_file(path: &Path) -> Result<Option<String>, String> {
    if let Some(expr) = read_spdx_expression(path)? {
        return Ok(Some(expr));
    }
    let companion = companion_license_path(path);
    if companion.is_file() {
        return read_spdx_expression(&companion);
    }
    Ok(None)
}

fn companion_license_path(path: &Path) -> PathBuf {
    let mut file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_owned();
    file_name.push_str(".license");
    path.with_file_name(file_name)
}

fn read_spdx_expression(path: &Path) -> Result<Option<String>, String> {
    let bytes = fs::read(path).map_err(|err| format!("read failed: {err}"))?;
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Ok(None);
    };
    Ok(extract_spdx_expression(text))
}

fn extract_spdx_expression(text: &str) -> Option<String> {
    const MARKER: &str = "SPDX-License-Identifier:";
    text.lines().find_map(|line| {
        let marker = line.find(MARKER)?;
        let expression = line[marker + MARKER.len()..]
            .split("*/")
            .next()
            .unwrap_or_default()
            .split("-->")
            .next()
            .unwrap_or_default()
            .trim();
        (!expression.is_empty()).then(|| expression.to_owned())
    })
}

fn spdx_expression_is_allowed(expression: &str) -> bool {
    let normalized = expression.replace(['(', ')'], " ");
    normalized.split_whitespace().all(|token| {
        matches!(token, "AND" | "OR" | "WITH")
            || ALLOWED_SPDX_LICENSES.contains(&token)
            || ALLOWED_SPDX_EXCEPTIONS.contains(&token)
    })
}

fn is_license_metadata(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    name == "REUSE.toml"
        || name.ends_with(".license")
        || name.starts_with("LICENSE")
        || name.starts_with("COPYING")
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time is after epoch")
                .as_nanos();
            let root =
                env::temp_dir().join(format!("squonk-xtask-{name}-{}-{nonce}", process::id()));
            fs::create_dir_all(&root).expect("create temp tree");
            Self { root }
        }

        fn write(&self, relative: &str, text: &str) {
            let path = self.root.join(relative);
            fs::create_dir_all(path.parent().expect("file has parent")).expect("create parents");
            fs::write(path, text).expect("write test file");
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    /// A complete, allowed provenance record for tests that exercise other gate
    /// facets. `\` line-continuations strip the indentation, yielding flat
    /// `key = "value"` lines.
    const VALID_PROVENANCE: &str = "source = \"https://github.com/example/repo\"\n\
        reference = \"0123456789abcdef0123456789abcdef01234567\"\n\
        license = \"MIT\"\n\
        regenerate = \"vendor from upstream; refresh with REWRITE=1\"\n";

    #[test]
    fn dialect_ban_accepts_clean_parser_source() {
        let temp = TempTree::new("clean-dialect");
        temp.write("crates/squonk/src/parser.rs", "pub fn parse() {}\n");

        check_dialect_dispatch_ban(&temp.root).expect("clean parser source passes");
    }

    #[test]
    fn dialect_ban_reports_planted_patterns() {
        let temp = TempTree::new("bad-dialect");
        temp.write(
            "crates/squonk/src/parser/bad.rs",
            "fn bad(x: &dyn std::any::Any) { dialect_of!(self, Postgres); \
             let _ = x.is::<u8>(); let _ = TypeId::of::<u8>(); }\n",
        );

        let errors = check_dialect_dispatch_ban(&temp.root).expect_err("ban catches pattern");
        assert!(
            errors.iter().any(|error| error.contains("dialect_of!")),
            "{errors:#?}"
        );
        assert!(
            errors.iter().any(|error| error.contains(".is::<")),
            "{errors:#?}"
        );
        assert!(
            errors.iter().any(|error| error.contains("TypeId")),
            "{errors:#?}"
        );
    }

    #[test]
    fn corpus_license_check_passes_when_no_corpus_exists() {
        let temp = TempTree::new("no-corpus");

        check_corpus_licenses(&temp.root).expect("no corpus is fine");
    }

    #[test]
    fn corpus_license_check_accepts_inline_spdx() {
        let temp = TempTree::new("inline-spdx");
        temp.write(
            "conformance/corpus/postgres/select.sql",
            "-- SPDX-License-Identifier: PostgreSQL\nSELECT 1;\n",
        );
        temp.write(
            "conformance/corpus/postgres/PROVENANCE.toml",
            VALID_PROVENANCE,
        );

        check_corpus_licenses(&temp.root).expect("inline SPDX passes");
    }

    #[test]
    fn corpus_license_check_accepts_companion_license_file() {
        let temp = TempTree::new("companion-spdx");
        temp.write("conformance/corpus/sqlite/select.sql", "SELECT 1;\n");
        temp.write(
            "conformance/corpus/sqlite/select.sql.license",
            "SPDX-License-Identifier: CC0-1.0\n",
        );
        temp.write(
            "conformance/corpus/sqlite/PROVENANCE.toml",
            VALID_PROVENANCE,
        );

        check_corpus_licenses(&temp.root).expect("companion SPDX passes");
    }

    #[test]
    fn corpus_license_check_reports_missing_spdx() {
        let temp = TempTree::new("missing-spdx");
        temp.write("conformance/corpus/bad.sql", "SELECT 1;\n");

        let errors = check_corpus_licenses(&temp.root).expect_err("missing SPDX fails");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("missing SPDX-License-Identifier")),
            "{errors:#?}"
        );
    }

    #[test]
    fn corpus_license_check_rejects_non_permissive_spdx() {
        let temp = TempTree::new("bad-spdx");
        temp.write(
            "conformance/corpus/gpl.sql",
            "-- SPDX-License-Identifier: GPL-2.0-only\nSELECT 1;\n",
        );

        let errors = check_corpus_licenses(&temp.root).expect_err("GPL SPDX fails");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("not in the permissive corpus allowlist")),
            "{errors:#?}"
        );
    }

    #[test]
    fn corpus_license_check_skips_cargo_fuzz_inputs() {
        let temp = TempTree::new("cargo-fuzz-corpus");
        // A cargo-fuzz crate keeps generated, license-free libFuzzer inputs under a
        // `corpus/` dir; the SPDX gate must skip them (they are not vendored corpora).
        temp.write(
            "conformance/fuzz/Cargo.toml",
            "[package]\nname = \"x-fuzz\"\n\n[package.metadata]\ncargo-fuzz = true\n",
        );
        temp.write("conformance/fuzz/corpus/parse/seed-0", "\x00\x01rawbytes");

        check_corpus_licenses(&temp.root).expect("cargo-fuzz corpus is not a vendored corpus");
    }

    #[test]
    fn corpus_license_check_skips_rust_module_named_corpus() {
        // A Rust bench/test *module* directory named `corpus` (it owns a `mod.rs`)
        // is first-party source, not a vendored data corpus, so the gate must not
        // demand an SPDX header or a PROVENANCE.toml on it. Regression for
        // `bench/benches/corpus/mod.rs`, which the corpus-root discovery otherwise
        // mis-classified as a vendored corpus.
        let temp = TempTree::new("rust-module-corpus");
        temp.write(
            "bench/benches/corpus/mod.rs",
            "//! Corpus-scale parser bench.\npub fn measure() {}\n",
        );

        check_corpus_licenses(&temp.root)
            .expect("a Rust module directory named corpus is not a vendored corpus");
    }

    #[test]
    fn corpus_license_check_still_flags_a_plain_fuzz_dir() {
        // A directory merely named `fuzz`, without the cargo-fuzz marker, is not
        // special — a vendored corpus under it must still carry SPDX.
        let temp = TempTree::new("plain-fuzz");
        temp.write("fuzz/corpus/case.sql", "SELECT 1;\n");

        let errors = check_corpus_licenses(&temp.root)
            .expect_err("a non-cargo-fuzz corpus still needs SPDX");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("missing SPDX-License-Identifier")),
            "{errors:#?}"
        );
    }

    #[test]
    fn corpus_license_check_requires_provenance_for_vendored_group() {
        // A licence-clean file is not enough: a vendored corpus group must also
        // record where it came from (ADR-0015).
        let temp = TempTree::new("missing-provenance");
        temp.write(
            "conformance/corpus/acme/data.sql",
            "-- SPDX-License-Identifier: MIT\nSELECT 1;\n",
        );

        let errors = check_corpus_licenses(&temp.root)
            .expect_err("a vendored corpus group without provenance fails");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("missing PROVENANCE.toml")),
            "{errors:#?}"
        );
    }

    #[test]
    fn corpus_license_check_accepts_complete_provenance() {
        let temp = TempTree::new("complete-provenance");
        temp.write(
            "conformance/corpus/acme/data.sql",
            "-- SPDX-License-Identifier: MIT\nSELECT 1;\n",
        );
        temp.write("conformance/corpus/acme/PROVENANCE.toml", VALID_PROVENANCE);

        check_corpus_licenses(&temp.root).expect("complete provenance passes");
    }

    #[test]
    fn corpus_provenance_requires_every_field() {
        let temp = TempTree::new("incomplete-provenance");
        temp.write(
            "conformance/corpus/acme/data.sql",
            "-- SPDX-License-Identifier: MIT\nSELECT 1;\n",
        );
        // Complete but for a missing `regenerate` field.
        temp.write(
            "conformance/corpus/acme/PROVENANCE.toml",
            "source = \"https://github.com/example/repo\"\n\
             reference = \"deadbeef\"\n\
             license = \"MIT\"\n",
        );

        let errors = check_corpus_licenses(&temp.root)
            .expect_err("provenance missing a required field fails");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("missing or empty `regenerate`")),
            "{errors:#?}"
        );
    }

    #[test]
    fn corpus_provenance_rejects_gpl_mysql_test_source() {
        // ADR-0015: run the MySQL engine as an oracle, never vendor the GPL
        // `mysql-test` suite — even when mislabelled with a permissive licence.
        let temp = TempTree::new("mysql-test-source");
        temp.write(
            "conformance/corpus/mysql/data.sql",
            "-- SPDX-License-Identifier: MIT\nSELECT 1;\n",
        );
        temp.write(
            "conformance/corpus/mysql/PROVENANCE.toml",
            "source = \"https://github.com/mysql/mysql-server/tree/trunk/mysql-test\"\n\
             reference = \"8.0\"\n\
             license = \"MIT\"\n\
             regenerate = \"n/a\"\n",
        );

        let errors =
            check_corpus_licenses(&temp.root).expect_err("a GPL mysql-test source is rejected");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("disallowed corpus source")
                    && error.contains("mysql-test")),
            "{errors:#?}"
        );
    }

    #[test]
    fn corpus_provenance_rejects_bsl_cockroach_source() {
        // ADR-0015: reuse the CockroachDB datadriven *format* only, never the BSL
        // test data.
        let temp = TempTree::new("cockroach-source");
        temp.write(
            "conformance/corpus/crdb/data.sql",
            "-- SPDX-License-Identifier: MIT\nSELECT 1;\n",
        );
        temp.write(
            "conformance/corpus/crdb/PROVENANCE.toml",
            "source = \"https://github.com/cockroachdb/cockroach/.../logictest/testdata\"\n\
             reference = \"v23.1\"\n\
             license = \"MIT\"\n\
             regenerate = \"n/a\"\n",
        );

        let errors =
            check_corpus_licenses(&temp.root).expect_err("a BSL CockroachDB source is rejected");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("disallowed corpus source") && error.contains("BSL")),
            "{errors:#?}"
        );
    }

    #[test]
    fn corpus_provenance_rejects_disallowed_license_family() {
        let temp = TempTree::new("provenance-gpl-license");
        temp.write(
            "conformance/corpus/acme/data.sql",
            "-- SPDX-License-Identifier: MIT\nSELECT 1;\n",
        );
        temp.write(
            "conformance/corpus/acme/PROVENANCE.toml",
            "source = \"https://github.com/example/repo\"\n\
             reference = \"deadbeef\"\n\
             license = \"GPL-3.0-only\"\n\
             regenerate = \"n/a\"\n",
        );

        let errors =
            check_corpus_licenses(&temp.root).expect_err("a GPL provenance licence is rejected");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("disallowed for vendored corpora")),
            "{errors:#?}"
        );
    }

    #[test]
    fn corpus_license_check_flags_gpl_family_with_policy_citation() {
        // A GPL file-level marker fails with the specific ADR-0015 reason, not
        // just the generic allowlist rejection, so it cannot be added silently.
        let temp = TempTree::new("gpl-policy-citation");
        temp.write(
            "conformance/corpus/acme/data.sql",
            "-- SPDX-License-Identifier: GPL-2.0-only\nSELECT 1;\n",
        );
        temp.write("conformance/corpus/acme/PROVENANCE.toml", VALID_PROVENANCE);

        let errors = check_corpus_licenses(&temp.root).expect_err("a GPL corpus file fails");
        assert!(
            errors.iter().any(
                |error| error.contains("not in the permissive corpus allowlist")
                    && error.contains("ADR-0015")
            ),
            "{errors:#?}"
        );
    }

    #[test]
    fn corpus_license_check_does_not_require_provenance_for_generated_testdata() {
        // First-party generated data under `testdata` (e.g. datadriven goldens) is
        // covered by the per-file SPDX scan and this repo's own licence; it has no
        // upstream, so the vendored-only provenance requirement must not fire here.
        let temp = TempTree::new("generated-testdata");
        temp.write(
            "conformance/testdata/goldens/case",
            "# SPDX-License-Identifier: MIT\nfoo\n",
        );

        check_corpus_licenses(&temp.root)
            .expect("generated testdata needs SPDX but no upstream provenance");
    }

    #[test]
    fn published_deps_accept_an_allowlisted_surface() {
        let temp = TempTree::new("deps-clean");
        temp.write(
            "Cargo.toml",
            "[workspace]\n\
             members = [\n\
             \"crates/ast\",\n\
             \"crates/parser\",\n\
             \"tools/dev\",\n\
             ]\n",
        );
        temp.write(
            "crates/ast/Cargo.toml",
            "[package]\nname = \"ast\"\n\n[dependencies]\nthin-vec = \"0.2\"\n",
        );
        // A published crate may depend on another published workspace crate.
        temp.write(
            "crates/parser/Cargo.toml",
            "[package]\nname = \"parser\"\n\n[dependencies]\n\
             ast = { path = \"../ast\" }\nthin-vec = \"0.2\"\n",
        );
        // publish = false: an internal tool may pull anything; the gate ignores it.
        temp.write(
            "tools/dev/Cargo.toml",
            "[package]\nname = \"dev\"\npublish = false\n\n[dependencies]\n\
             syn = \"2\"\nquote = \"1\"\n",
        );

        check_published_deps(&temp.root).expect("an allowlisted published surface passes");
    }

    #[test]
    fn published_deps_reject_an_unlisted_dependency() {
        let temp = TempTree::new("deps-stray");
        temp.write("Cargo.toml", "[workspace]\nmembers = [\"crates/parser\"]\n");
        temp.write(
            "crates/parser/Cargo.toml",
            "[package]\nname = \"parser\"\n\n[dependencies]\n\
             thin-vec = \"0.2\"\nregex = \"1\"\n",
        );

        let errors = check_published_deps(&temp.root).expect_err("a stray published dep fails");
        assert!(
            errors
                .iter()
                .any(|error| error.contains("regex")
                    && error.contains("published-dependency allowlist")),
            "{errors:#?}"
        );
    }

    #[test]
    fn published_deps_ignore_dev_dependencies() {
        let temp = TempTree::new("deps-dev");
        temp.write("Cargo.toml", "[workspace]\nmembers = [\"crates/parser\"]\n");
        // proptest is a dev-dependency: it never ships, so the gate must not flag it.
        temp.write(
            "crates/parser/Cargo.toml",
            "[package]\nname = \"parser\"\n\n[dependencies]\nthin-vec = \"0.2\"\n\n\
             [dev-dependencies]\nproptest = \"1\"\n",
        );

        check_published_deps(&temp.root).expect("dev-dependencies do not ship and are ignored");
    }

    #[test]
    fn published_deps_ignore_unpublished_members() {
        let temp = TempTree::new("deps-unpublished");
        temp.write(
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/parser\", \"bench\"]\n",
        );
        temp.write(
            "crates/parser/Cargo.toml",
            "[package]\nname = \"parser\"\n\n[dependencies]\nthin-vec = \"0.2\"\n",
        );
        // A publish = false bench crate may depend on a rejected dep like `phf`.
        temp.write(
            "bench/Cargo.toml",
            "[package]\nname = \"bench\"\npublish = false\n\n[dependencies]\nphf = \"0.11\"\n",
        );

        check_published_deps(&temp.root).expect("publish = false members are exempt");
    }

    #[test]
    fn published_deps_reject_a_leaf_that_grows_a_subtree() {
        let temp = TempTree::new("deps-leaf-subtree");
        temp.write("Cargo.toml", "[workspace]\nmembers = []\n");
        // A hypothetical future `thin-vec` that grew a dependency must trip Layer 2.
        // The grown dependency must be a synthetic name absent from the allowlist: a
        // name the allowlist includes (e.g. the opt-in `serde` build-time proc-macros)
        // would be accepted by Layer 2, which is the opposite of what this asserts.
        temp.write(
            "Cargo.lock",
            "version = 3\n\n\
             [[package]]\nname = \"thin-vec\"\nversion = \"0.2.18\"\n\
             dependencies = [\n \"some-unlisted-crate\",\n]\n\n\
             [[package]]\nname = \"some-unlisted-crate\"\nversion = \"2.0.0\"\n",
        );

        let errors = check_published_deps(&temp.root)
            .expect_err("an allowlisted leaf growing a subtree fails");
        assert!(
            errors.iter().any(|error| error.contains("thin-vec")
                && error.contains("some-unlisted-crate")
                && error.contains("transitive surface")),
            "{errors:#?}"
        );
    }

    #[test]
    fn published_deps_pass_for_the_real_workspace() {
        // The live ADR-0017 regression gate as a plain `#[test]`: the real published
        // surface must already be within the allowlist (it is `thin-vec`-only).
        let Ok(root) = find_workspace_root() else {
            return;
        };
        check_published_deps(&root).expect("the real published dependency surface is allowlisted");
    }

    #[test]
    fn parse_workspace_members_reads_a_multiline_array() {
        let manifest = "[workspace]\nresolver = \"2\"\nmembers = [\n\
            \"crates/squonk-ast\",\n\"crates/squonk\",\n\"bench\",\n]\n\n\
            [workspace.dependencies]\nthin-vec = \"0.2\"\n";
        assert_eq!(
            parse_workspace_members(manifest),
            vec![
                "crates/squonk-ast".to_owned(),
                "crates/squonk".to_owned(),
                "bench".to_owned(),
            ]
        );
    }

    #[test]
    fn parse_runtime_dep_names_skips_dev_dependencies() {
        let manifest = "[package]\nname = \"p\"\n\n\
            [dependencies]\nthin-vec = { workspace = true }\nast.workspace = true\n\n\
            [build-dependencies]\ngen = \"1\"\n\n\
            [dev-dependencies]\nproptest = \"1\"\n";
        assert_eq!(
            parse_runtime_dep_names(manifest),
            vec!["thin-vec".to_owned(), "ast".to_owned(), "gen".to_owned()]
        );
    }

    #[test]
    fn parse_lock_dependencies_strips_version_tokens() {
        let lock = "[[package]]\nname = \"a\"\nversion = \"1.0.0\"\n\
            dependencies = [\n \"b\",\n \"c 2.0.0\",\n]\n\n\
            [[package]]\nname = \"b\"\nversion = \"0.1.0\"\n";
        let map = parse_lock_dependencies(lock);
        assert_eq!(map.get("a"), Some(&vec!["b".to_owned(), "c".to_owned()]));
        assert_eq!(map.get("b"), Some(&vec![]));
    }

    #[test]
    fn extension_seam_holds_for_the_real_tree() {
        let Ok(root) = find_workspace_root() else {
            return;
        };
        check_extension_seam(&root).expect("the shipped extension seam matches ADR-0009");
    }

    #[test]
    fn extension_seam_flags_a_drifted_trait_bound() {
        let temp = TempTree::new("ext-bound-drift");
        // Bound narrowed to `Clone + Debug` — the exact ADR-0009 drift this gate catches.
        temp.write(
            "crates/squonk-ast/src/ast/ext.rs",
            "pub enum NoExt {}\npub trait Extension: Clone + Debug {}\n",
        );
        let errors = check_extension_seam(&temp.root).expect_err("a drifted bound fails");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("supertrait bound must stay exactly")),
            "{errors:#?}"
        );
    }

    #[test]
    fn extension_seam_flags_a_dropped_blanket_impl() {
        let temp = TempTree::new("ext-blanket-drop");
        // Trait bound and `NoExt` intact, but the ADR-0009 blanket impl removed — the
        // foreclosed-on-purpose bound-alias decision this gate exists to keep from drifting.
        temp.write(
            "crates/squonk-ast/src/ast/ext.rs",
            "pub enum NoExt {}\npub trait Extension: Clone + Debug + Eq + Hash + Spanned {}\n",
        );
        let errors = check_extension_seam(&temp.root).expect_err("a missing blanket impl fails");
        assert!(errors.iter().any(|e| e.contains("blanket")), "{errors:#?}");
    }

    #[test]
    fn extension_seam_flags_a_seventh_openable_enum() {
        let temp = TempTree::new("ext-seventh-seam");
        temp.write(
            "crates/squonk-ast/src/ast/ext.rs",
            "pub enum NoExt {}\npub trait Extension: Clone + Debug + Eq + Hash + Spanned {}\n",
        );
        // Seven `ext: X,` seams where ADR-0009 fixes the openable set at six.
        let mut body = String::new();
        for _ in 0..7 {
            body.push_str("    Other {\n        ext: X,\n    },\n");
        }
        temp.write("crates/squonk-ast/src/ast/nodes.rs", &body);
        let errors = check_extension_seam(&temp.root).expect_err("a seventh seam fails");
        assert!(
            errors.iter().any(|e| e.contains("expected exactly 6")),
            "{errors:#?}"
        );
    }

    #[test]
    fn precedence_holds_for_the_real_tree() {
        let Ok(root) = find_workspace_root() else {
            return;
        };
        check_precedence(&root).expect("the shipped precedence wiring matches ADR-0008");
    }

    #[test]
    fn precedence_bans_a_nested_expr_node() {
        let temp = TempTree::new("nested-expr");
        temp.write(
            "crates/squonk-ast/src/ast/expr.rs",
            "pub enum Expr {\n    Nested(Box<Expr>),\n}\nfn f() { let _ = Expr::Nested; }\n",
        );
        // The set-op source is present so only the ban error is asserted.
        temp.write(
            "crates/squonk/src/parser/query.rs",
            "fn parse_set_expr_bp() { let _ = set_operation_binding_power(); }\n",
        );
        let errors = check_precedence(&temp.root).expect_err("a stored paren node fails");
        assert!(
            errors.iter().any(|e| e.contains("Expr::Nested")),
            "{errors:#?}"
        );
    }

    #[test]
    fn precedence_requires_the_single_set_op_table() {
        let temp = TempTree::new("setop-fold");
        temp.write("crates/squonk-ast/src/ast/expr.rs", "pub enum Expr {}\n");
        // A `parse_set_expr` that folds without consulting the binding-power table.
        temp.write(
            "crates/squonk/src/parser/query.rs",
            "fn parse_set_expr() { loop { left_fold(); } }\n",
        );
        let errors = check_precedence(&temp.root).expect_err("a hand-rolled set-op fold fails");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("set_operation_binding_power")),
            "{errors:#?}"
        );
    }

    #[test]
    fn dialect_generic_holds_for_the_real_tree() {
        let Ok(root) = find_workspace_root() else {
            return;
        };
        check_dialect_generic(&root)
            .expect("the shipped `generic` alias maps to ANSI per ADR-0011");
    }

    #[test]
    fn dialect_generic_flags_a_misrouted_generic_alias() {
        let temp = TempTree::new("generic-misroute");
        temp.write(
            "crates/squonk/src/dialect/builtin.rs",
            "fn from_name(name: &str) -> Option<Self> {\n    if name.eq_ignore_ascii_case(\"generic\") {\n        return Some(Self::Lenient);\n    }\n    None\n}\n",
        );
        temp.write(
            "crates/squonk-ast/src/dialect/ansi.rs",
            "impl FeatureSet { pub const ANSI: Self = Self {}; }\n",
        );
        let errors = check_dialect_generic(&temp.root).expect_err("generic->Lenient fails");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("must map the runtime name")),
            "{errors:#?}"
        );
    }

    #[test]
    fn dialect_generic_bans_a_separate_generic_preset() {
        let temp = TempTree::new("generic-preset");
        temp.write(
            "crates/squonk/src/dialect/builtin.rs",
            "fn from_name(name: &str) -> Option<Self> {\n    if name.eq_ignore_ascii_case(\"generic\") {\n        return Some(Self::Ansi);\n    }\n    None\n}\n",
        );
        temp.write(
            "crates/squonk-ast/src/dialect/ansi.rs",
            "impl FeatureSet {\n    pub const ANSI: Self = Self {};\n    pub const GENERIC: Self = FeatureSet::GENERIC;\n}\n",
        );
        let errors =
            check_dialect_generic(&temp.root).expect_err("a separate Generic preset fails");
        assert!(
            errors.iter().any(|e| e.contains("FeatureSet::GENERIC")),
            "{errors:#?}"
        );
    }

    // ===== Commenting standard: change-history-narration gate =====
}
