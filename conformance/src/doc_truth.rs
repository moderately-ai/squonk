// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Release-facing documentation truth gate.
//!
//! Manual truth sweeps of the human-facing docs (`README.md`, `docs/architecture.md`,
//! the crate-root rustdoc) went stale repeatedly: the dialect list dropped presets,
//! the dependency claim said "zero dependencies" while `thin-vec` shipped. This module
//! is the derived check that retires those sweeps: it parses the docs on disk and
//! compares them against the live code — `BuiltinDialect::ALL` for the dialect roster,
//! the published crates' own `Cargo.toml` for the runtime-dependency claim — so a doc
//! that drifts from the code fails the build. It is the doc-parsing sibling of
//! [`crate::support_tiers`] (which instead *generates* `docs/support-tiers.md` from the
//! same tier table); together they cover the claim classes the truth sweep tracked.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use squonk::BuiltinDialect;

    /// Workspace root, resolved from this crate's manifest dir (`conformance/` is one
    /// level below the root — the same anchor [`crate::support_tiers`] uses).
    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    fn read(rel: &str) -> String {
        let path = workspace_root().join(rel);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
    }

    /// The external, non-optional runtime dependencies declared by the two *published*
    /// crates, parsed straight from their `Cargo.toml`. First-party `squonk*` crates
    /// and `optional = true` (feature-gated) dependencies are excluded — the goal is the
    /// set a default `cargo build` of a downstream consumer actually pulls, which is what
    /// the docs' "dependency" claim describes.
    fn published_runtime_deps() -> BTreeSet<String> {
        let mut deps = BTreeSet::new();
        for manifest in ["crates/squonk-ast/Cargo.toml", "crates/squonk/Cargo.toml"] {
            let text = read(manifest);
            let mut in_deps = false;
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    // Only the plain `[dependencies]` table is a runtime edge;
                    // `[dev-dependencies]`, `[build-dependencies]`, `[features]` are not.
                    in_deps = trimmed == "[dependencies]";
                    continue;
                }
                if !in_deps || trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let Some((name, spec)) = trimmed.split_once('=') else {
                    continue;
                };
                let name = name.trim();
                // `optional = true` inside the inline table marks a feature-gated edge,
                // not a default one; first-party crates are not an external dependency.
                if spec.contains("optional = true") || name.starts_with("squonk") {
                    continue;
                }
                deps.insert(name.to_string());
            }
        }
        deps
    }

    /// Every dialect in the release-facing inventory docs is the live `BuiltinDialect::ALL`.
    ///
    /// The conformance crate compiles `squonk` with `full`, so `ALL` is the complete
    /// shipped roster here. Each preset's canonical `name()` (a lowercase id like
    /// `postgres`/`duckdb`) is a case-insensitive substring of every spelling the prose
    /// uses (`Postgres`, `PostgreSQL`, `DuckDb`, `DuckDB`), so a plain case-insensitive
    /// containment check catches the exact regression this gate exists for: a preset
    /// silently dropped from the enumeration (BigQuery/Hive/Redshift were, once).
    #[test]
    fn release_docs_enumerate_every_builtin_dialect() {
        // The two release-facing places that enumerate the dialect roster by hand.
        for doc in ["docs/architecture.md", "crates/squonk/src/lib.rs"] {
            let haystack = read(doc).to_ascii_lowercase();
            for dialect in BuiltinDialect::ALL {
                let needle = dialect.name().to_ascii_lowercase();
                assert!(
                    haystack.contains(&needle),
                    "{doc} does not mention the `{}` dialect, but it is in BuiltinDialect::ALL; \
                     update the enumeration to match the shipped roster",
                    dialect.name(),
                );
            }
        }
    }

    /// The published crates' runtime-dependency claim in the docs matches their manifests.
    ///
    /// Two halves, both derived from source: the live dependency set is exactly the
    /// `thin-vec` micro-leaf (guarding the code side against a new default dependency
    /// landing undocumented), and the docs that make the dependency claim both name it
    /// and carry no false absolute "no dependencies" phrasing (guarding the doc side
    /// against the "zero dependencies" regression that shipped while `thin-vec` did).
    #[test]
    fn published_dependency_claims_are_current() {
        let deps = published_runtime_deps();
        let expected: BTreeSet<String> = ["thin-vec".to_string()].into_iter().collect();
        assert_eq!(
            deps, expected,
            "published crates' non-optional runtime dependencies changed to {deps:?}; \
             update the dependency claims in README.md / docs/architecture.md (and this gate) \
             to match, then re-check the ADR-0017 `deps` allowlist",
        );

        // Absolute "no dependencies" claims are false the moment any dep exists (as
        // `thin-vec` always has); ADR-0017's honest framing is "only thin-vec". Forbid
        // the exact stale phrasings and require the dependency be named instead.
        const FORBIDDEN: &[&str] = &["zero dependencies", "has no dependencies"];
        for doc in ["README.md", "docs/architecture.md"] {
            let lower = read(doc).to_ascii_lowercase();
            for phrase in FORBIDDEN {
                assert!(
                    !lower.contains(phrase),
                    "{doc} contains the stale absolute claim {phrase:?}; the published crates \
                     depend on {deps:?}, so state that (\"a single micro-dependency, thin-vec\") \
                     rather than claiming none",
                );
            }
            for dep in &deps {
                assert!(
                    lower.contains(&dep.to_ascii_lowercase()),
                    "{doc} makes a dependency claim but never names the `{dep}` runtime \
                     dependency the published crates actually pull in",
                );
            }
        }
    }
}
