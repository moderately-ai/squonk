// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! `cargo xtask feature-matrix` — the feature-combination build gate.
//!
//! The published crates' dialect cargo features are all default-OFF (`default =
//! []`; `postgres`/`mysql`/`sqlite`/`duckdb`/`lenient`/`full`/`serde`), and the two
//! standard gates build exactly two configurations (default workspace + the
//! conformance `oracle-engines` build). A break confined to ONE non-default feature
//! — a `sqlite.rs` preset initializer, a `duckdb`-gated module, a `serde` impl — is
//! invisible to every current gate. This gate closes that hole: it `cargo check`s
//! every feature combination a downstream consumer can actually select, then runs
//! the test suite once at the `full` endpoint.
//!
//! **Shape — why a standalone subcommand, not one `PREFLIGHT_STEPS` entry.** The
//! matrix is a *sequence* of cargo runs, each with its own per-combination
//! remediation (the failing feature set + the exact command to reproduce it). That
//! is the same "one `const` array is the single source of truth" shape as
//! [`crate::preflight`]'s `PREFLIGHT_STEPS`, but pitched at its own level:
//! [`FEATURE_MATRIX`] drives the run order, the table rows, AND each reproduce
//! command from one place — the `args` of an entry *are* the command you rerun. A
//! single heterogeneous `PreflightStep` carries exactly one `remediation` line and
//! one `cargo` arg vector, so it cannot express a per-combination remediation over
//! nine invocations; cramming the matrix into one step would break the very
//! single-array invariant it is meant to mirror. So the matrix owns its own array
//! and reuses preflight's log-streaming + table helpers ([`crate::preflight::run_cargo`],
//! [`crate::preflight::print_row_tail`], [`crate::preflight::tail_lines`]).
//!
//! Run this release-oriented gate explicitly with `cargo xtask feature-matrix`.
//!
//! **Run-all, not fail-fast** (the deliberate difference from preflight's core
//! stack): the core stack is cheap-first and stops at the first failure because a
//! broken `fmt` makes the later steps moot. The matrix combinations are instead
//! *independent* compile checks — a `sqlite` break and a `serde` break are
//! unrelated — so running every combination and reporting all failures in one pass
//! beats discovering them one re-run at a time. Only the *check* combinations run;
//! tests run once, at the `full` endpoint.
//!
//! **Opt-in on purpose** (minutes of builds): run this command when you touch
//! feature-gated surface — a dialect preset, a `#[cfg(feature = ...)]` module, the
//! Cargo feature wiring, or serde-gated code — not on every pass.
//!
//! **Dependency-free** (ADR-0017): a plain loop over `cargo`, no `cargo-hack`.
//!
//! **CI relation.** The ordinary CI stack remains fast and deterministic. Run this
//! separately for feature-wiring changes and release candidates.

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::time::Instant;

use crate::preflight::{NAME_WIDTH, StepOutcome, print_row_tail, run_cargo, tail_lines};

/// One feature combination the gate builds. `args` is both the `cargo` argument
/// vector AND the reproduce command a reader reruns (prefix `cargo `), so the run,
/// the table, and the per-combination remediation all derive from this one field —
/// they cannot drift.
struct FeatureCombo {
    /// Stable label printed in the table (the feature combination under test).
    name: &'static str,
    /// The `cargo` arguments. Verbatim on purpose: the entry *is* the command, so
    /// the printed reproduce line is copy-pasteable with no reconstruction.
    args: &'static [&'static str],
}

/// The feature combinations the gate covers, in run order. Adding an entry here
/// registers it everywhere: the run order, the pass/FAIL table, and the reproduce
/// command all fold over this one array (the `PREFLIGHT_STEPS` discipline, applied
/// to feature combinations).
///
/// Coverage rationale — every configuration a downstream consumer can select on the
/// two published crates, which the standard default+oracle gates never build:
/// `--no-default-features` (the leanest ANSI-only build), each dialect feature solo
/// (so a break in one preset/module can't hide behind another feature pulling it
/// in), the `full` parity build, and the non-default `serde` axis. `duckdb` solo
/// legitimately compiles `postgres` too (`duckdb` implies it — the preset is
/// PostgreSQL-derived); that is the real dependency, not a gap. Checks pass both
/// `-p squonk -p squonk-ast` so a `squonk-ast`-only break (e.g. a
/// serde derive) is caught even though `squonk` forwards the same feature.
/// `--no-default-features` on every check makes "solo" explicit and future-proofs
/// the intent against a non-empty `default` ever being added. The two
/// `serde-{serialize,deserialize}-all-targets` cells additionally compile the crates'
/// TEST targets under one serde half at a time — the only cells that do — to close the
/// wider-gate-than-referent cfg-hole class structurally (see their inline comment).
const FEATURE_MATRIX: &[FeatureCombo] = &[
    FeatureCombo {
        name: "no-default-features",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
        ],
    },
    FeatureCombo {
        name: "postgres",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "postgres",
        ],
    },
    FeatureCombo {
        name: "mysql",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "mysql",
        ],
    },
    FeatureCombo {
        name: "sqlite",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "sqlite",
        ],
    },
    FeatureCombo {
        name: "duckdb",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "duckdb",
        ],
    },
    FeatureCombo {
        name: "bigquery",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "bigquery",
        ],
    },
    FeatureCombo {
        name: "hive",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "hive",
        ],
    },
    FeatureCombo {
        name: "clickhouse",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "clickhouse",
        ],
    },
    FeatureCombo {
        name: "databricks",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "databricks",
        ],
    },
    FeatureCombo {
        name: "mssql",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "mssql",
        ],
    },
    FeatureCombo {
        name: "snowflake",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "snowflake",
        ],
    },
    FeatureCombo {
        name: "redshift",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "redshift",
        ],
    },
    FeatureCombo {
        name: "lenient",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "lenient",
        ],
    },
    FeatureCombo {
        name: "full",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "full",
        ],
    },
    FeatureCombo {
        name: "serde",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "serde",
        ],
    },
    // The two partial-serde axes, each compiling TEST targets (`--all-targets`), not
    // just the lib. This is the only place the matrix compiles test code under a
    // partial feature set, and it exists to close one structural hole: a `#[cfg(test)]`
    // module whose gate is WIDER than the gates of the items it references (e.g. a test
    // gated `any(serialize, deserialize)` that names a `deserialize`-only item). Such a
    // module is invisible to every lib-only check and to the `serde`-umbrella lanes
    // (which activate both halves), and compiles only under one-half-without-the-other —
    // the exact shape that turned the platform macOS lane red (serde-test-cfg-hole-parsed-rs).
    // `check --all-targets` type-checks the test targets (catching the E0422/E0599 the
    // hole produces) without running them; the umbrella-gated serde integration tests are
    // absent here (their `#![cfg(feature = "serde")]` needs both halves), so the marginal
    // cost is just the two crates' unit-test compiles. Both published crates are built so
    // an AST-side asymmetry is caught symmetrically.
    FeatureCombo {
        name: "serde-serialize-all-targets",
        args: &[
            "check",
            "--all-targets",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "serde-serialize",
        ],
    },
    FeatureCombo {
        name: "serde-deserialize-all-targets",
        args: &[
            "check",
            "--all-targets",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "serde-deserialize",
        ],
    },
    // The pretty-printing formatter axis. The feature lives on `squonk` only (the
    // AST crate has no formatter), but both published crates are selected to keep the
    // matrix's "check builds both crates" invariant — cargo applies the feature to the
    // package that defines it. Solo over the ANSI-only base proves the `format` module
    // compiles with no dialect pulled in, guarding the serialize-only default.
    FeatureCombo {
        name: "document-render",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "document-render",
        ],
    },
    // The formatter composed with the full dialect surface — the interaction a
    // downstream consumer that formats every dialect selects.
    FeatureCombo {
        name: "document-render-full",
        args: &[
            "check",
            "-p",
            "squonk",
            "-p",
            "squonk-ast",
            "--no-default-features",
            "--features",
            "full,document-render",
        ],
    },
    // The one test endpoint: check-only for the matrix, tests only at `full`. Adds
    // `document-render` so the formatter's own tests (the `format` module) run in a
    // gate — the standard default/oracle lanes build without the feature.
    FeatureCombo {
        name: "full-tests",
        args: &[
            "nextest",
            "run",
            "-p",
            "squonk",
            "--features",
            "full,document-render",
        ],
    },
];

/// The reproduce command for a combination: exactly what a reader reruns. Derived
/// from `args` so it cannot drift from what the gate actually invoked.
fn reproduce(combo: &FeatureCombo) -> String {
    format!("cargo {}", combo.args.join(" "))
}

/// Run `cargo xtask feature-matrix`: `cargo check` every feature combination plus
/// the `full` test endpoint, streaming each to its own log, and print a
/// pass/FAIL table. Returns the process exit code (0 = every combination green,
/// 1 = at least one failed). Run-all: every combination runs even after a failure,
/// and each failure's per-combination remediation is printed at the end.
///
/// Shared by the `cargo xtask feature-matrix` subcommand and `cargo xtask preflight
/// --matrix`, so the covered combinations are defined in exactly one place.
pub fn run(root: &Path) -> i32 {
    let log_dir = root.join("target").join("feature-matrix");
    if let Err(err) = fs::create_dir_all(&log_dir) {
        eprintln!("feature-matrix: cannot create {}: {err}", log_dir.display());
        return 1;
    }

    println!("cargo xtask feature-matrix — feature-combination build gate");
    println!("  (check-only per combination; tests once at the `full` endpoint)");
    println!();

    let overall = Instant::now();
    let mut passed = 0usize;
    // Each failure carries the combo (for its reproduce line) and its log path.
    let mut failures: Vec<(&'static FeatureCombo, std::path::PathBuf)> = Vec::new();

    for combo in FEATURE_MATRIX {
        // Print the name and flush so the row shows "in progress" while the cargo
        // subprocess streams to its log file (never to this terminal) — the same
        // live-row idiom as `preflight`.
        print!("  {:<width$} ", combo.name, width = NAME_WIDTH);
        let _ = io::stdout().flush();

        let started = Instant::now();
        let log_path = log_dir.join(format!("{}.log", combo.name));
        let outcome = run_cargo(root, combo.args, false, &log_path);
        let elapsed = format!("{:.1}s", started.elapsed().as_secs_f64());

        match outcome {
            StepOutcome::Pass => {
                print_row_tail("pass", &elapsed, "");
                passed += 1;
            }
            StepOutcome::Fail { log_path, note } => {
                let note = if note.is_empty() {
                    "does not compile"
                } else {
                    note.as_str()
                };
                print_row_tail("FAIL", &elapsed, note);
                failures.push((combo, log_path));
            }
            // The matrix issues only `StepAction::Cargo`-style runs, which never
            // skip; a skip would be a `run_cargo` contract change, so surface it
            // as a failure rather than silently counting it green.
            StepOutcome::Skip { note } => {
                print_row_tail("FAIL", &elapsed, &format!("unexpected skip: {note}"));
                failures.push((combo, log_dir.join(format!("{}.log", combo.name))));
            }
        }
    }

    let total = overall.elapsed().as_secs_f64();
    println!();

    if failures.is_empty() {
        println!("feature-matrix: {passed} passed ({total:.1}s)");
        return 0;
    }

    println!(
        "feature-matrix: {passed} passed, {} FAILED ({total:.1}s)",
        failures.len()
    );
    // Per-combination remediation: name the failing combination and the exact
    // command to reproduce it, one line each, for every failure in this pass.
    println!("  remediation — rerun the failing combination(s):");
    for (combo, log_path) in &failures {
        println!("    - `{}`: {}", combo.name, reproduce(combo));
        println!("      log: {}", log_path.display());
    }
    // A representative tail: the first failure's log, so the reader has a concrete
    // error on screen without dumping every failing build inline (the rest are on
    // disk at the paths listed above).
    let (first_combo, first_log) = &failures[0];
    let tail = tail_lines(first_log, 40);
    if !tail.is_empty() {
        println!();
        println!(
            "--- {} `{}` (last 40 lines) ---",
            first_log.display(),
            first_combo.name
        );
        println!("{tail}");
    }

    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_combination_has_a_nonempty_name_and_args() {
        for combo in FEATURE_MATRIX {
            assert!(!combo.name.trim().is_empty(), "a combination has no name");
            assert!(
                !combo.args.is_empty(),
                "combination `{}` has no cargo args",
                combo.name
            );
        }
    }

    #[test]
    fn combination_names_are_unique() {
        // The name keys each combination's log file (`<name>.log`); a duplicate
        // would make two combinations clobber one log.
        let mut seen = std::collections::BTreeSet::new();
        for combo in FEATURE_MATRIX {
            assert!(
                seen.insert(combo.name),
                "duplicate combination name `{}`",
                combo.name
            );
        }
    }

    #[test]
    fn matrix_covers_every_selectable_feature() {
        // Pin the coverage: `--no-default-features`, each dialect feature solo, the
        // `full` parity build, and the `serde` axis must each appear. A new dialect
        // feature that is not added to the matrix is exactly the invisible-break
        // class this gate exists to prevent, so this test fails until it is covered.
        let joined: Vec<String> = FEATURE_MATRIX
            .iter()
            .map(|combo| combo.args.join(" "))
            .collect();
        let mentions = |needle: &str| joined.iter().any(|args| args.contains(needle));

        assert!(
            joined
                .iter()
                .any(|args| args.contains("--no-default-features") && !args.contains("--features")),
            "the bare --no-default-features combination must be covered"
        );
        for feature in [
            "postgres",
            "mysql",
            "sqlite",
            "duckdb",
            "bigquery",
            "hive",
            "clickhouse",
            "databricks",
            "mssql",
            "snowflake",
            "redshift",
            "lenient",
            "full",
            "serde",
        ] {
            assert!(
                mentions(&format!("--features {feature}")),
                "feature `{feature}` is not covered by the matrix"
            );
        }
    }

    #[test]
    fn checks_target_both_published_crates() {
        // Every `check` combination builds both published crates so a
        // `squonk-ast`-only break is caught even when `squonk` forwards the
        // same feature. (The `nextest` endpoint targets `squonk` alone by design.)
        for combo in FEATURE_MATRIX {
            if combo.args.first() == Some(&"check") {
                let args = combo.args.join(" ");
                assert!(
                    args.contains("-p squonk") && args.contains("-p squonk-ast"),
                    "check combination `{}` must build both published crates",
                    combo.name
                );
            }
        }
    }

    #[test]
    fn partial_serde_cells_compile_all_targets() {
        // The structural guarantee this ticket adds: each serde half is compiled with
        // its TEST targets under exactly that half (never the umbrella), so a test
        // module whose cfg gate is wider than the items it references cannot rot
        // unseen. Losing `--all-targets` on either cell — or collapsing them into the
        // both-halves umbrella — silently reopens the hole, so pin both properties.
        for half in ["serde-serialize", "serde-deserialize"] {
            let cell = FEATURE_MATRIX
                .iter()
                .find(|combo| {
                    let args = combo.args.join(" ");
                    args.contains(&format!("--features {half}"))
                        && !args.contains(&format!("--features {half},"))
                })
                .unwrap_or_else(|| panic!("no cell activates `{half}` alone"));
            let args = cell.args.join(" ");
            assert!(
                args.contains("--all-targets"),
                "the `{half}` cell must compile test targets (`--all-targets`) to close the cfg-hole class"
            );
        }
    }

    #[test]
    fn tests_run_only_at_the_full_endpoint() {
        // Check-only for the matrix, tests only at the endpoints: exactly one
        // combination runs tests, and it is the `full` build.
        let test_combos: Vec<&FeatureCombo> = FEATURE_MATRIX
            .iter()
            .filter(|combo| combo.args.first() == Some(&"nextest"))
            .collect();
        assert_eq!(
            test_combos.len(),
            1,
            "the matrix must run tests at exactly one endpoint"
        );
        let args = test_combos[0].args.join(" ");
        assert!(
            args.contains("--features full"),
            "the test endpoint must be the `full` build"
        );
    }

    #[test]
    fn reproduce_is_the_verbatim_cargo_command() {
        // The reproduce line must be exactly what the gate invoked, so a reader can
        // copy-paste it. It is `cargo ` + the same args the run used.
        let combo = &FEATURE_MATRIX[0];
        assert_eq!(reproduce(combo), format!("cargo {}", combo.args.join(" ")));
        assert!(reproduce(combo).starts_with("cargo check "));
    }
}
