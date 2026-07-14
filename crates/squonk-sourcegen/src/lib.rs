// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Code generator for the `squonk` AST — emits checked-in traversals from
//! the hand-written, annotated node types (ADR-0013).
//!
//! This dev-only tool (never published) reads the node types as text and emits
//! formatted `.rs` into `crates/squonk-ast/src/generated/`. Drift tests
//! regenerate in memory and fail `cargo test` if checked-in output is stale, so
//! adding a field or variant to a node without regenerating cannot pass tests.

mod descent;
mod dialect_presets;
mod feature_set;
mod keywords;
mod license_header;
mod mod_index;
mod node_id_walk;
mod python;
mod render_skeleton;
mod schema;
mod size_asserts;
mod spanned;
mod typescript;
mod visit;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use schema::Schema;

/// One checked-in generated file and its freshly rendered contents.
pub struct GeneratedFile {
    pub path: PathBuf,
    pub contents: String,
}

/// Render the source-backed `Keyword` inventory, lookup, and per-position
/// reservation bitsets from the checked-in objective keyword data
/// (`keyword_data/*.csv`, ADR-0004) — independent of the AST `Schema`.
///
/// Wired into [`generate_all`]: this is the live `Keyword` enum, lookup, and the
/// per-category bitsets the dialect per-position gates compose
/// (prod-keyword-position-reserved-sets). The drift gate keeps the checked-in
/// `dialect/keyword/generated.rs` in lockstep with the source data.
pub fn generate_keywords() -> String {
    format_generated(keywords::render())
}

/// Render the source-backed `Keyword` inventory and lookup WITHOUT the `rustfmt`
/// pass [`generate_keywords`] applies — the emitted lookup code is otherwise
/// identical.
///
/// The compiler is indifferent to formatting, so the bench `build.rs` that
/// `include!`s the generated lookup to measure it against `phf` (ADR-0004's
/// rejected-dep comparison) uses this to keep a build script from shelling out to
/// an external `rustfmt`. Not part of [`generate_all`]; see [`generate_keywords`].
pub fn generate_keywords_unformatted() -> String {
    keywords::render()
}

/// The keyword inventory as `(spelling, variant)` pairs — the single source both
/// the generated lookup and the bench-only `phf` map (ADR-0004) are built from, so
/// the comparison runs over identical data.
pub fn keyword_inventory() -> Vec<(String, String)> {
    keywords::inventory_pairs()
}

/// Render every checked-in generated file in memory.
pub fn generate_all() -> Vec<GeneratedFile> {
    dialect_presets::assert_explicit();
    let schema = Schema::load();
    vec![
        GeneratedFile {
            path: generated_mod_index_path(),
            contents: format_generated(mod_index::render()),
        },
        GeneratedFile {
            path: generated_spanned_path(),
            contents: format_generated(spanned::render(&schema)),
        },
        GeneratedFile {
            path: generated_visit_path(),
            contents: format_generated(visit::render(&schema)),
        },
        GeneratedFile {
            path: generated_node_id_walk_path(),
            contents: format_generated(node_id_walk::render(&schema)),
        },
        GeneratedFile {
            path: generated_render_skeleton_path(),
            contents: format_generated(render_skeleton::render(&schema)),
        },
        GeneratedFile {
            path: generated_size_asserts_path(),
            contents: format_generated(size_asserts::render(&schema)),
        },
        // Independent of the AST [`Schema`]: sourced from the `FeatureSet` struct
        // text plus its annotation table (ADR-0011 dialect data, ADR-0013 gate).
        GeneratedFile {
            path: generated_feature_set_path(),
            contents: format_generated(feature_set::render()),
        },
        // Independent of the AST [`Schema`]: sourced from the objective keyword
        // inventories (`keyword_data/*.csv`, ADR-0004). The live `Keyword` enum,
        // lookup, and per-position reservation bitsets.
        GeneratedFile {
            path: generated_keyword_path(),
            contents: format_generated(keywords::render()),
        },
        // TypeScript binding declarations: sourced from the same AST schema as the Rust
        // generated visitors, but emitted as `.d.ts` rather than Rust.
        GeneratedFile {
            path: generated_wasm_typescript_ast_path(),
            contents: typescript::render(&schema),
        },
        // Runtime binding metadata: the JSON AST erases some Rust struct names, so the
        // Python and JS facades need schema-sourced field types instead of guessing from
        // shape (for example, `ObjectName` versus a plain `Ident[]`).
        GeneratedFile {
            path: generated_wasm_ast_metadata_path(),
            contents: typescript::render_metadata_js(&schema),
        },
        GeneratedFile {
            path: generated_python_ast_metadata_path(),
            contents: typescript::render_metadata_py(&schema),
        },
        GeneratedFile {
            path: generated_python_ast_types_path(),
            contents: python::render(&schema),
        },
    ]
}

/// Format generated Rust with the same rustfmt used by `cargo fmt`.
fn format_generated(source: String) -> String {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rustfmt; install the rustfmt component to run sourcegen");

    {
        use std::io::Write as _;
        child
            .stdin
            .as_mut()
            .expect("rustfmt stdin is piped")
            .write_all(source.as_bytes())
            .expect("write generated source to rustfmt");
    }

    let output = child.wait_with_output().expect("wait for rustfmt");
    if !output.status.success() {
        panic!(
            "rustfmt failed while formatting generated source:\n{}",
            String::from_utf8_lossy(&output.stderr),
        );
    }
    String::from_utf8(output.stdout).expect("rustfmt output is UTF-8")
}

/// Env override for the resolved workspace root, in the repo's `SQUONK_*`
/// override family (cf. `SQUONK_CORPUS_ROOT`). When set it wins over the cwd
/// walk-up — the escape hatch for invoking from outside the tree.
const WORKSPACE_ROOT_ENV: &str = "SQUONK_WORKSPACE_ROOT";

/// The workspace root, resolved at RUNTIME from the current directory — never from
/// `CARGO_MANIFEST_DIR`, which bakes the *building* worktree's path into the
/// binary. Under a shared `target/` a stale cached binary would then write
/// generated files into the tree it was built in rather than the invoking one
/// (cross-worktree write contamination — ADR-0013/0017); runtime resolution binds
/// every output path to the invoking worktree instead.
///
/// Resolution order: the `SQUONK_WORKSPACE_ROOT` override, else the nearest
/// ancestor of the current directory whose `Cargo.toml` declares `[workspace]`.
/// Panics — naming the directory searched from — when neither yields a root, so a
/// misinvocation fails loudly rather than silently targeting the wrong tree.
pub fn workspace_root() -> PathBuf {
    try_workspace_root().unwrap_or_else(|err| panic!("sourcegen: {err}"))
}

/// The fallible form of [`workspace_root`], same resolution order. The writer
/// binary pre-flights this to exit cleanly on a misinvocation (mirroring xtask's
/// main), while the in-crate path helpers use the panicking form like every other
/// error in this dev tool.
pub fn try_workspace_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|err| format!("read current directory: {err}"))?;
    let override_root = std::env::var_os(WORKSPACE_ROOT_ENV).map(PathBuf::from);
    resolve_workspace_root(override_root, &cwd)
}

/// The pure resolver behind [`workspace_root`], split out so a unit test can pin
/// the derivation against a synthetic tree without mutating process-global cwd or
/// environment. The `override_root` (from `SQUONK_WORKSPACE_ROOT`) wins; else
/// walk `cwd`'s ancestors for the workspace manifest.
fn resolve_workspace_root(override_root: Option<PathBuf>, cwd: &Path) -> Result<PathBuf, String> {
    if let Some(root) = override_root {
        return Ok(root);
    }
    find_workspace_root_from(cwd)
}

/// Walk `start` and its ancestors, returning the first directory whose `Cargo.toml`
/// declares a `[workspace]` table. Member manifests (no `[workspace]`) are walked
/// through; the root is the first that matches.
fn find_workspace_root_from(start: &Path) -> Result<PathBuf, String> {
    for dir in start.ancestors() {
        if let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) {
            if manifest_declares_workspace(&text) {
                return Ok(dir.to_path_buf());
            }
        }
    }
    Err(format!(
        "no workspace root (a Cargo.toml declaring [workspace]) found walking up from {}; \
         set {WORKSPACE_ROOT_ENV} to override",
        start.display(),
    ))
}

/// True when a `Cargo.toml`'s text carries a `[workspace]` table header. A
/// line-oriented match keeps sourcegen TOML-parser-free (ADR-0017): the trimmed
/// line must equal `[workspace]` exactly, so `[workspace.dependencies]`, a
/// commented `# [workspace]`, or a string containing the text never false-match.
fn manifest_declares_workspace(manifest: &str) -> bool {
    manifest.lines().any(|line| line.trim() == "[workspace]")
}

/// The directory holding the hand-written AST node source files.
pub fn ast_src_dir() -> PathBuf {
    workspace_root().join("crates/squonk-ast/src/ast")
}

/// The hand-written dialect source whose `FeatureSet` struct anchors the generated
/// delta/registry (read as text, never compiled — ADR-0013).
pub fn dialect_src_path() -> PathBuf {
    workspace_root().join("crates/squonk-ast/src/dialect/mod.rs")
}

/// The checked-in generated module index this tool (re)writes.
pub fn generated_mod_index_path() -> PathBuf {
    workspace_root().join("crates/squonk-ast/src/generated/mod.rs")
}

/// The checked-in `Spanned` impls this tool (re)writes.
pub fn generated_spanned_path() -> PathBuf {
    workspace_root().join("crates/squonk-ast/src/generated/spanned.rs")
}

/// The checked-in `Visit` / `VisitMut` output this tool (re)writes.
pub fn generated_visit_path() -> PathBuf {
    workspace_root().join("crates/squonk-ast/src/generated/visit.rs")
}

/// The checked-in node-id walk (`NodeIdWalk`) this tool (re)writes.
pub fn generated_node_id_walk_path() -> PathBuf {
    workspace_root().join("crates/squonk-ast/src/generated/node_id_walk.rs")
}

/// The checked-in render skeleton output this tool (re)writes.
pub fn generated_render_skeleton_path() -> PathBuf {
    workspace_root().join("crates/squonk-ast/src/generated/render_skeleton.rs")
}

/// The checked-in enum size assertions this tool (re)writes.
pub fn generated_size_asserts_path() -> PathBuf {
    workspace_root().join("crates/squonk-ast/src/generated/size_asserts.rs")
}

/// The checked-in `FeatureSet` delta/registry this tool (re)writes. It
/// lives under `dialect/` (not `generated/`) because it is derived from the
/// dialect `FeatureSet` struct, not the AST node schema the other generated walks
/// come from — and so stays in the `ast-dialect-data` scope alongside its source.
pub fn generated_feature_set_path() -> PathBuf {
    workspace_root().join("crates/squonk-ast/src/dialect/feature_set_generated.rs")
}

/// The checked-in generated `Keyword` inventory, lookup, and per-position
/// reservation bitsets this tool (re)writes. It lives under `dialect/keyword/`
/// (not `generated/`) because it is derived from the objective keyword data, not
/// the AST node schema — and so stays in the `ast-dialect-data` scope alongside
/// the hand-written `dialect/keyword.rs` that composes its bitsets.
pub fn generated_keyword_path() -> PathBuf {
    workspace_root().join("crates/squonk-ast/src/dialect/keyword/generated.rs")
}

/// The checked-in TypeScript AST declarations for the WASM binding wrapper.
pub fn generated_wasm_typescript_ast_path() -> PathBuf {
    workspace_root().join("crates/squonk-wasm/js/ast.generated.d.ts")
}

/// The checked-in runtime metadata for the WASM/TypeScript AST wrapper.
pub fn generated_wasm_ast_metadata_path() -> PathBuf {
    workspace_root().join("crates/squonk-wasm/js/ast-metadata.generated.js")
}

/// The checked-in runtime metadata for the Python AST wrapper.
pub fn generated_python_ast_metadata_path() -> PathBuf {
    workspace_root().join("crates/squonk-python/python/squonk/_ast_metadata.py")
}

/// Complete checked-in Python declarations for the serialized AST.
pub fn generated_python_ast_types_path() -> PathBuf {
    workspace_root().join("crates/squonk-python/python/squonk/ast.py")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// The anti-drift gate (ADR-0013): the checked-in file must be byte-identical
    /// to fresh in-memory generation.
    #[test]
    fn generated_files_are_up_to_date() {
        for GeneratedFile { path, contents } in generate_all() {
            let actual = std::fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
            // Compare with newlines normalized to LF: `.gitattributes`
            // (`* text=auto eol=lf`) keeps the working tree LF, but a checkout
            // predating it — or a user `core.autocrlf` override — could hand
            // `read_to_string` CRLF for a file rustfmt emits with LF. That is a
            // checkout artifact, not real drift (docs/platform-support.md).
            let actual = normalize_newlines(&actual);
            if actual != contents {
                let line = first_divergent_line(&actual, &contents);
                panic!(
                    "{} is stale — regenerate with `cargo run -p squonk-sourcegen` \
                     and commit the rewritten file (never hand-edit generated output); \
                     first divergence at line {line}",
                    path.display(),
                );
            }
        }
    }

    /// The source-backed keyword inventory regenerates deterministically from the
    /// checked-in `keyword_data/*.csv` and carries the objective dialect classes
    /// (ADR-0004). Pins the reproducibility the live wiring will later build on.
    #[test]
    fn keyword_inventory_is_source_backed_and_reproducible() {
        let rendered = keywords::render();
        assert_eq!(
            rendered,
            keywords::render(),
            "keyword generation must be deterministic",
        );

        // The full inventory, lookup, and every per-dialect reserved bitset are emitted.
        for expected in [
            "pub enum Keyword {",
            "pub const ALL: [Self;",
            "pub fn lookup_keyword(word: &str) -> Option<Keyword>",
            "pub const POSTGRES_RESERVED_KEYWORDS: super::KeywordSet",
            "pub const MYSQL_RESERVED_KEYWORDS: super::KeywordSet",
            "pub const MYSQL_TYPE_FUNC_NAME_KEYWORDS: super::KeywordSet",
        ] {
            assert!(rendered.contains(expected), "missing {expected:?}");
        }

        // The inventory is the full ANSI+PostgreSQL union, far beyond the M1 subset.
        let variant_count = rendered.matches("Self::").count();
        assert!(
            variant_count > 700,
            "expected the full keyword union, got ~{variant_count} variants",
        );

        // Objective reservation is carried through from the source data: `SELECT` and
        // `FROM` are PostgreSQL-reserved, while `YEAR` — a keyword only via the
        // SQL:2016 inventory — is unreserved in PostgreSQL, so it is absent from the
        // reserved bitset.
        let postgres = section(&rendered, "POSTGRES_RESERVED_KEYWORDS");
        for structural in ["Keyword::Select,", "Keyword::From,"] {
            assert!(
                postgres.contains(structural),
                "PostgreSQL reserved missing {structural}"
            );
        }
        assert!(!postgres.contains("Keyword::Year,"));

        // MySQL's reserved set is the third objective inventory (mysql-reserved-word-set):
        // `RLIKE` is MySQL-reserved yet a plain identifier under PostgreSQL, the
        // cross-dialect divergence the per-dialect sets carry.
        let mysql = section(&rendered, "MYSQL_RESERVED_KEYWORDS");
        assert!(mysql.contains("Keyword::Rlike,"));
        assert!(!postgres.contains("Keyword::Rlike,"));
    }

    /// The body of a `from_keywords(&[ ... ])` reserved-set const, for membership checks.
    fn section<'a>(rendered: &'a str, name: &str) -> &'a str {
        let start = rendered
            .find(name)
            .unwrap_or_else(|| panic!("{name} is emitted"));
        let body = &rendered[start..];
        let end = body.find("]);").expect("reserved const is closed");
        &body[..end]
    }

    /// One-based line of the first divergence, for a readable failure message.
    fn first_divergent_line(actual: &str, expected: &str) -> usize {
        actual
            .lines()
            .zip(expected.lines())
            .position(|(a, b)| a != b)
            .map_or_else(
                || actual.lines().count().min(expected.lines().count()) + 1,
                |i| i + 1,
            )
    }

    /// Fold CRLF to LF so the drift comparison is a content check, not a line-ending
    /// check. `.gitattributes` (`* text=auto eol=lf`) is the primary guard against a
    /// Windows checkout rewriting the LF generated files to CRLF; this keeps the gate
    /// correct even if that is bypassed. A lone CR (not part of a CRLF pair) is left
    /// intact.
    fn normalize_newlines(text: &str) -> String {
        text.replace("\r\n", "\n")
    }

    /// Strip the Windows verbatim (`\\?\`) prefix that `canonicalize()` prepends, so
    /// `TempTree` path comparisons see the ordinary path form. On Unix `canonicalize()`
    /// never produces this prefix and the input is returned unchanged, so the helper is
    /// a no-op there while its unit test still exercises the Windows shapes on any host.
    /// `\\?\C:\x` -> `C:\x`; `\\?\UNC\server\share` -> `\\server\share`.
    fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
        let Some(s) = path.to_str() else { return path };
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
        path
    }

    /// A throwaway directory tree under the system temp dir, removed on drop.
    /// Mirrors xtask's test idiom so the runtime derivation can be pinned against a
    /// synthetic workspace without a tempdir dependency (ADR-0017).
    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time is after epoch")
                .as_nanos();
            let root = std::env::temp_dir()
                .join(format!("squonk-sourcegen-{name}-{}-{nonce}", process::id()));
            std::fs::create_dir_all(&root).expect("create temp tree");
            // Canonicalize so path comparisons are immune to the `/tmp` ->
            // `/private/tmp` (and similar) symlinks in the platform temp path. On
            // Windows canonicalize() adds a `\\?\` verbatim prefix; strip it so the
            // module's path assertions see the ordinary form on every host
            // (docs/platform-support.md, Windows section).
            let root = strip_verbatim_prefix(root.canonicalize().expect("canonicalize temp tree"));
            Self { root }
        }

        fn write(&self, relative: &str, text: &str) {
            let path = self.root.join(relative);
            std::fs::create_dir_all(path.parent().expect("file has parent"))
                .expect("create parents");
            std::fs::write(path, text).expect("write test file");
        }

        fn mkdir(&self, relative: &str) -> PathBuf {
            let path = self.root.join(relative);
            std::fs::create_dir_all(&path).expect("create dir");
            path
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// The runtime derivation walks up from a nested cwd, steps THROUGH member
    /// manifests (no `[workspace]`), and stops at the first `[workspace]` root — so
    /// a stale binary invoked anywhere inside a worktree resolves *that* worktree,
    /// not the one it was compiled in. This is the invocation-relative property the
    /// whole fix turns on.
    #[test]
    fn derivation_finds_the_nearest_workspace_root_walking_up() {
        let tree = TempTree::new("walkup");
        tree.write("Cargo.toml", "[workspace]\nmembers = [\"crates/x\"]\n");
        // A member manifest without `[workspace]` must be walked through, not
        // mistaken for the root.
        tree.write("crates/x/Cargo.toml", "[package]\nname = \"x\"\n");
        let nested = tree.mkdir("crates/x/src/deep");

        let found = find_workspace_root_from(&nested).expect("root is found");
        assert_eq!(
            found, tree.root,
            "walk-up must land on the [workspace] root"
        );

        // The pure resolver with no override takes the same walk-up path.
        assert_eq!(
            resolve_workspace_root(None, &nested).expect("resolves"),
            tree.root,
        );
    }

    /// The `SQUONK_WORKSPACE_ROOT` override wins over the cwd walk-up, letting
    /// the tool be pointed at a tree it is not invoked from within.
    #[test]
    fn override_wins_over_walk_up() {
        let tree = TempTree::new("override");
        tree.write("Cargo.toml", "[workspace]\n");
        let elsewhere = PathBuf::from("/some/explicit/root");

        // cwd (`tree.root`) would otherwise resolve to itself; the override supersedes it.
        assert_eq!(
            resolve_workspace_root(Some(elsewhere.clone()), &tree.root).expect("override honoured"),
            elsewhere,
        );
    }

    /// With no `[workspace]` anywhere up the chain, derivation errors — naming the
    /// directory searched from and the override env — instead of silently
    /// targeting the wrong tree.
    #[test]
    fn derivation_errors_when_no_workspace_root_exists() {
        let tree = TempTree::new("noroot");
        // Only a member-style manifest exists; nothing declares `[workspace]`, and
        // the temp tree's ancestors carry no Cargo.toml.
        tree.write("crates/x/Cargo.toml", "[package]\nname = \"x\"\n");
        let nested = tree.mkdir("crates/x/src");

        let err = find_workspace_root_from(&nested).expect_err("no root exists");
        assert!(
            err.contains(&nested.display().to_string()),
            "error must name the directory searched from, got: {err}",
        );
        assert!(
            err.contains(WORKSPACE_ROOT_ENV),
            "error must point at the override, got: {err}",
        );
    }

    /// The `[workspace]` line-match is exact: only a bare table header counts, so a
    /// `[workspace.dependencies]` subtable, a comment, or a quoted occurrence in a
    /// member manifest never masquerades as the workspace root.
    #[test]
    fn workspace_marker_matches_only_the_bare_table_header() {
        assert!(manifest_declares_workspace("[workspace]\nmembers = []\n"));
        assert!(manifest_declares_workspace(
            "resolver = \"2\"\n  [workspace]\n"
        ));
        assert!(!manifest_declares_workspace(
            "[workspace.dependencies]\nthin-vec = \"0.2\"\n"
        ));
        assert!(!manifest_declares_workspace("# [workspace]\n"));
        assert!(!manifest_declares_workspace(
            "[package]\nname = \"x = [workspace]\"\n"
        ));
    }

    /// `TempTree` strips the `\\?\` verbatim prefix Windows `canonicalize()` prepends,
    /// leaving the ordinary path form; a Unix path (no prefix) is returned unchanged.
    /// Simulating the Windows shapes here exercises the UNC awareness on every host,
    /// not only on a Windows runner (docs/platform-support.md, Windows section).
    #[test]
    fn strip_verbatim_prefix_normalizes_windows_canonical_paths() {
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\C:\Users\ci\Temp\squonk")),
            PathBuf::from(r"C:\Users\ci\Temp\squonk"),
        );
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from(r"\\?\UNC\server\share\gen")),
            PathBuf::from(r"\\server\share\gen"),
        );
        assert_eq!(
            strip_verbatim_prefix(PathBuf::from("/private/tmp/squonk")),
            PathBuf::from("/private/tmp/squonk"),
        );
    }

    /// The drift gate compares content, not line-ending style: a CRLF working-tree file
    /// (a Windows checkout artifact) folds to the LF form rustfmt emits, so only genuine
    /// content drift fails the gate. A lone CR is preserved
    /// (docs/platform-support.md, Windows section).
    #[test]
    fn normalize_newlines_folds_crlf_to_lf() {
        assert_eq!(normalize_newlines("a\r\nb\r\n"), "a\nb\n");
        assert_eq!(normalize_newlines("a\nb\n"), "a\nb\n");
        assert_eq!(normalize_newlines("a\rb"), "a\rb");
    }
}
