// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! The fixed local/CI verification stack.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::check_all;

enum StepAction {
    Tidy,
    Cargo {
        args: &'static [&'static str],
        doc_zero_warning: bool,
    },
}

struct PreflightStep {
    name: &'static str,
    action: StepAction,
}

const PREFLIGHT_STEPS: &[PreflightStep] = &[
    cargo_step("fmt", &["fmt", "--all", "--", "--check"]),
    PreflightStep {
        name: "tidy",
        action: StepAction::Tidy,
    },
    cargo_step(
        "clippy-default",
        &["clippy", "--all-targets", "--", "-D", "warnings"],
    ),
    cargo_step(
        "clippy-bindings",
        &[
            "clippy",
            "-p",
            "squonk-python",
            "-p",
            "squonk-node",
            "-p",
            "squonk-wasm",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ),
    cargo_step(
        "clippy-conformance",
        &[
            "clippy",
            "-p",
            "squonk-conformance",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ),
    cargo_step("nextest-default", &["nextest", "run"]),
    cargo_step(
        "nextest-bindings",
        &["nextest", "run", "-p", "squonk-node", "-p", "squonk-wasm"],
    ),
    cargo_step(
        "nextest-schema",
        &["nextest", "run", "-p", "squonk", "--features", "serde"],
    ),
    cargo_step(
        "nextest-conformance",
        &["nextest", "run", "-p", "squonk-conformance"],
    ),
    doc_step("doc-default", &["doc", "--no-deps"]),
    doc_step(
        "doc-conformance",
        &["doc", "-p", "squonk-conformance", "--no-deps"],
    ),
    doc_step(
        "doc-all-features",
        &[
            "doc",
            "--all-features",
            "--no-deps",
            "--document-private-items",
        ],
    ),
];

const fn cargo_step(name: &'static str, args: &'static [&'static str]) -> PreflightStep {
    PreflightStep {
        name,
        action: StepAction::Cargo {
            args,
            doc_zero_warning: false,
        },
    }
}

const fn doc_step(name: &'static str, args: &'static [&'static str]) -> PreflightStep {
    PreflightStep {
        name,
        action: StepAction::Cargo {
            args,
            doc_zero_warning: true,
        },
    }
}

pub(crate) const NAME_WIDTH: usize = 24;

/// Run the same deterministic non-oracle stack locally and in CI.
pub fn run_preflight(root: &Path, args: &[String]) -> Result<i32, String> {
    if !args.is_empty() {
        return Err(
            "preflight takes no arguments; run oracle and feature-matrix checks separately".into(),
        );
    }

    let log_dir = root.join("target/preflight");
    fs::create_dir_all(&log_dir).map_err(|err| format!("create {}: {err}", log_dir.display()))?;
    println!(
        "preflight: fixed non-oracle stack ({} steps)",
        PREFLIGHT_STEPS.len()
    );

    let started = Instant::now();
    for step in PREFLIGHT_STEPS {
        let step_started = Instant::now();
        let outcome = match &step.action {
            StepAction::Tidy => run_tidy(&log_dir),
            StepAction::Cargo {
                args,
                doc_zero_warning,
            } => run_cargo(
                root,
                args,
                *doc_zero_warning,
                &log_dir.join(format!("{}.log", step.name)),
            ),
        };
        let elapsed = format!("{:.1}s", step_started.elapsed().as_secs_f64());
        match outcome {
            StepOutcome::Pass => print_row(step.name, "PASS", &elapsed, ""),
            StepOutcome::Skip { note } => print_row(step.name, "SKIP", &elapsed, &note),
            StepOutcome::Fail { log_path, note } => {
                print_row(step.name, "FAIL", &elapsed, &note);
                println!("preflight: FAILED at `{}`", step.name);
                println!("  log: {}", log_path.display());
                let tail = tail_lines(&log_path, 40);
                if !tail.is_empty() {
                    println!("\n{tail}");
                }
                return Ok(1);
            }
        }
    }
    println!(
        "preflight: {} passed ({:.1}s)",
        PREFLIGHT_STEPS.len(),
        started.elapsed().as_secs_f64()
    );
    Ok(0)
}

fn run_tidy(log_dir: &Path) -> StepOutcome {
    match check_all(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()) {
        Ok(()) => StepOutcome::Pass,
        Err(errors) => {
            let log_path = log_dir.join("tidy.log");
            let _ = fs::write(&log_path, format!("{}\n", errors.join("\n")));
            StepOutcome::Fail {
                log_path,
                note: String::new(),
            }
        }
    }
}

pub(crate) enum StepOutcome {
    Pass,
    Fail {
        log_path: PathBuf,
        note: String,
    },
    #[allow(dead_code)]
    Skip {
        note: String,
    },
}

pub(crate) fn run_cargo(
    root: &Path,
    args: &[&str],
    doc_zero_warning: bool,
    log_path: &Path,
) -> StepOutcome {
    let Some(parent) = log_path.parent() else {
        return StepOutcome::Fail {
            log_path: log_path.to_path_buf(),
            note: "invalid log path".into(),
        };
    };
    if let Err(err) = fs::create_dir_all(parent) {
        return StepOutcome::Fail {
            log_path: log_path.to_path_buf(),
            note: err.to_string(),
        };
    }
    let out = match File::create(log_path) {
        Ok(file) => file,
        Err(err) => {
            return StepOutcome::Fail {
                log_path: log_path.to_path_buf(),
                note: err.to_string(),
            };
        }
    };
    let err = match out.try_clone() {
        Ok(file) => file,
        Err(err) => {
            return StepOutcome::Fail {
                log_path: log_path.to_path_buf(),
                note: err.to_string(),
            };
        }
    };
    match Command::new("cargo")
        .args(args)
        .current_dir(root)
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err))
        .status()
    {
        Ok(status) if status.success() => {
            if doc_zero_warning && log_has_warning(log_path) {
                StepOutcome::Fail {
                    log_path: log_path.to_path_buf(),
                    note: "documentation emitted warnings".into(),
                }
            } else {
                StepOutcome::Pass
            }
        }
        Ok(_) => StepOutcome::Fail {
            log_path: log_path.to_path_buf(),
            note: String::new(),
        },
        Err(err) => StepOutcome::Fail {
            log_path: log_path.to_path_buf(),
            note: err.to_string(),
        },
    }
}

fn log_has_warning(path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|text| text.lines().any(|line| line.contains("warning:")))
        .unwrap_or(true)
}

pub(crate) fn tail_lines(path: &Path, count: usize) -> String {
    fs::read_to_string(path)
        .map(|text| {
            let lines: Vec<_> = text.lines().collect();
            lines[lines.len().saturating_sub(count)..].join("\n")
        })
        .unwrap_or_default()
}

fn print_row(name: &str, status: &str, time: &str, note: &str) {
    print!("  {name:<NAME_WIDTH$} ");
    print_row_tail(status, time, note);
}

pub(crate) fn print_row_tail(status: &str, time: &str, note: &str) {
    print!("{status:<5} {time:>8}");
    if note.is_empty() {
        println!();
    } else {
        println!("  {note}");
    }
    let _ = io::stdout().flush();
}
