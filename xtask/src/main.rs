// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

use std::env;
use std::process;

fn main() {
    let root = xtask::find_workspace_root().unwrap_or_else(|err| {
        eprintln!("error: {err}");
        process::exit(2);
    });

    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "tidy".to_owned());
    let rest: Vec<String> = args.collect();

    // `preflight` is the fixed non-oracle stack shared by contributors and CI.
    if command == "preflight" {
        match xtask::run_preflight(&root, &rest) {
            Ok(code) => process::exit(code),
            Err(message) => {
                eprintln!("error: {message}");
                usage();
                process::exit(2);
            }
        }
    }
    // `license-headers` mutates on `--write` and verifies on `--check` (or bare, also
    // reachable via the `CHECKS` registry so the `tidy` gate runs it). Dispatched here
    // so the mode flags don't trip the no-argument guard below.
    if command == "license-headers" {
        if rest == ["--write"] {
            match xtask::write_license_headers(&root) {
                Ok(summary) => {
                    println!("xtask license-headers --write: {summary}");
                    return;
                }
                Err(errors) => fail("xtask license-headers --write", errors),
            }
        }
        if rest.is_empty() || rest == ["--check"] {
            match xtask::check_license_headers(&root) {
                Ok(()) => {
                    println!("xtask license-headers --check: ok");
                    return;
                }
                Err(errors) => fail("xtask license-headers --check", errors),
            }
        }
        usage();
        process::exit(2);
    }
    // `feature-matrix` is a runner (it shells out to cargo per feature combination)
    // that returns the process exit code, like `preflight`. It takes no arguments.
    if command == "feature-matrix" {
        if !rest.is_empty() {
            usage();
            process::exit(2);
        }
        process::exit(xtask::run_feature_matrix(&root));
    }
    // `knob-org` is a freeze gate for dialect FeatureSet organization (bijection,
    // orphans, synopsis honesty, naming). Registered in CHECKS so `tidy` runs it too.
    if command == "knob-org" {
        if !rest.is_empty() {
            usage();
            process::exit(2);
        }
        match xtask::check_knob_org(&root) {
            Ok(()) => {
                println!("xtask knob-org: ok");
                return;
            }
            Err(errors) => fail("xtask knob-org", errors),
        }
    }
    // `semver` validates the checked baseline in 0.x and shells out to
    // cargo-semver-checks after 1.0, returning that tool's status.
    if command == "semver" {
        if !rest.is_empty() {
            usage();
            process::exit(2);
        }
        match xtask::run_semver(&root) {
            Ok(code) => process::exit(code),
            Err(message) => {
                eprintln!("error: {message}");
                process::exit(2);
            }
        }
    }
    if !rest.is_empty() {
        usage();
        process::exit(2);
    }

    let result = match command.as_str() {
        "tidy" => xtask::check_all(&root),
        "-h" | "--help" | "help" => {
            usage();
            return;
        }
        _ => match xtask::CHECKS
            .iter()
            .find_map(|&(name, check)| (name == command).then_some(check))
        {
            Some(check) => check(&root),
            None => {
                usage();
                process::exit(2);
            }
        },
    };

    match result {
        Ok(()) => println!("xtask {command}: ok"),
        Err(errors) => fail(&format!("xtask {command}"), errors),
    }
}

/// Print `{label}: failed` followed by each error on its own line, then exit(1).
/// Shared failure formatting for all in-process checks.
fn fail(label: &str, errors: Vec<String>) -> ! {
    eprintln!("{label}: failed");
    for error in errors {
        eprintln!("  - {error}");
    }
    process::exit(1);
}

/// The `[tidy|license|...]` list is derived from `CHECKS` (plus the `tidy`
/// aggregate, which is not itself a registered gate) so it can't drift from the
/// dispatch above.
fn usage() {
    let names: Vec<&str> = std::iter::once("tidy")
        .chain(xtask::CHECKS.iter().map(|&(name, _)| name))
        .collect();
    eprintln!("usage: cargo xtask [{}]", names.join("|"));
    eprintln!(
        "       cargo xtask license-headers [--check|--write]   # verify (default) or stamp the SPDX + copyright header on every first-party source file"
    );
    eprintln!(
        "       cargo xtask preflight   # fixed non-oracle local/CI stack (fmt→tidy→clippy→nextest→doc)"
    );
    eprintln!(
        "       cargo xtask preflight --only <step,...>   # run named steps in canonical order"
    );
    eprintln!(
        "       cargo xtask feature-matrix   # feature-combination build gate: each dialect feature solo + no-default + full + serde (check), tests at full"
    );
    eprintln!(
        "       cargo xtask semver   # validate the current-major baseline; compare both published crates against v<major>.0.0 with all features"
    );
}
