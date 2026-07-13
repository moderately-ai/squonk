// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CONFIG_PATH: &str = "release/semver-baseline.toml";

struct Config {
    baseline_tag: String,
    package_roots: Vec<PathBuf>,
}

pub fn run(root: &Path) -> Result<i32, String> {
    let major = workspace_major(root)?;
    let expected_baseline = if major == 0 {
        "v1.0.0".to_owned()
    } else {
        format!("v{major}.0.0")
    };
    let config = load_config(root, &expected_baseline)?;
    if major == 0 {
        println!(
            "semver: pre-stable workspace; baseline {} is configured for activation at 1.0.0",
            config.baseline_tag
        );
        return Ok(0);
    }

    require_baseline_tag(root, &config.baseline_tag)?;
    require_semver_checks(root)?;

    for package_root in config.package_roots {
        let package = package_name(&root.join(&package_root).join("Cargo.toml"))?;
        println!(
            "semver: checking {package} against {} with all features",
            config.baseline_tag
        );
        let status = Command::new("cargo")
            .current_dir(root)
            .args([
                "semver-checks",
                "--package",
                &package,
                "--baseline-rev",
                &config.baseline_tag,
                "--all-features",
            ])
            .status()
            .map_err(|err| format!("failed to run cargo-semver-checks for {package}: {err}"))?;
        if !status.success() {
            return Ok(status.code().unwrap_or(1));
        }
    }
    Ok(0)
}

fn load_config(root: &Path, expected_baseline: &str) -> Result<Config, String> {
    let path = root.join(CONFIG_PATH);
    let text = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let schema = scalar(&text, "schema")?;
    if schema != "1" {
        return Err(format!(
            "{CONFIG_PATH}: unsupported schema `{schema}` (expected 1)"
        ));
    }
    let baseline_tag = quoted_scalar(&text, "baseline_tag")?;
    if baseline_tag != expected_baseline {
        return Err(format!(
            "{CONFIG_PATH}: workspace major requires API baseline `{expected_baseline}`, found `{baseline_tag}`"
        ));
    }
    let package_roots = quoted_array(&text, "packages")?
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if package_roots.is_empty() {
        return Err(format!("{CONFIG_PATH}: `packages` must not be empty"));
    }
    for package_root in &package_roots {
        let manifest = root.join(package_root).join("Cargo.toml");
        if !manifest.is_file() {
            return Err(format!(
                "{CONFIG_PATH}: package manifest does not exist: {}",
                manifest.display()
            ));
        }
        let text = fs::read_to_string(&manifest)
            .map_err(|err| format!("{}: {err}", manifest.display()))?;
        if text.lines().any(|line| {
            let compact = line.trim().replace(' ', "");
            compact == "publish=false"
        }) {
            return Err(format!(
                "{CONFIG_PATH}: {} is not a published package",
                package_root.display()
            ));
        }
    }
    let configured: BTreeSet<PathBuf> = package_roots.iter().cloned().collect();
    let published = published_package_roots(root)?;
    if configured != published {
        return Err(format!(
            "{CONFIG_PATH}: `packages` must exactly cover published workspace members; configured: {}, published: {}",
            display_paths(&configured),
            display_paths(&published)
        ));
    }
    Ok(Config {
        baseline_tag,
        package_roots,
    })
}

fn published_package_roots(root: &Path) -> Result<BTreeSet<PathBuf>, String> {
    let path = root.join("Cargo.toml");
    let text = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let members = workspace_members(&text)?;
    let mut published = BTreeSet::new();
    for member in members {
        let manifest = root.join(member).join("Cargo.toml");
        let text = fs::read_to_string(&manifest)
            .map_err(|err| format!("{}: {err}", manifest.display()))?;
        if !manifest_is_unpublished(&text) {
            published.insert(PathBuf::from(member));
        }
    }
    Ok(published)
}

fn workspace_members(manifest: &str) -> Result<Vec<&str>, String> {
    let workspace = manifest
        .split_once("[workspace]")
        .map(|(_, tail)| tail)
        .ok_or_else(|| "Cargo.toml: missing `[workspace]`".to_owned())?;
    let section = workspace.split("\n[").next().unwrap_or(workspace);
    let members = section
        .split_once("members")
        .map(|(_, tail)| tail)
        .and_then(|tail| tail.split_once('[').map(|(_, tail)| tail))
        .and_then(|tail| tail.split_once(']').map(|(value, _)| value))
        .ok_or_else(|| "Cargo.toml: missing `[workspace] members`".to_owned())?;
    members
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .ok_or_else(|| format!("Cargo.toml: workspace member must be quoted: `{value}`"))
        })
        .collect()
}

fn manifest_is_unpublished(manifest: &str) -> bool {
    manifest.lines().any(|line| {
        let compact = line.trim().replace(' ', "");
        compact == "publish=false"
    })
}

fn display_paths(paths: &BTreeSet<PathBuf>) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn workspace_major(root: &Path) -> Result<u64, String> {
    let path = root.join("Cargo.toml");
    let text = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut in_workspace_package = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_workspace_package = line == "[workspace.package]";
            continue;
        }
        if in_workspace_package && line.starts_with("version") {
            let version = quoted_value(line, "version")?;
            let major = version
                .split('.')
                .next()
                .and_then(|value| value.parse().ok())
                .ok_or_else(|| format!("Cargo.toml: invalid workspace version `{version}`"))?;
            return Ok(major);
        }
    }
    Err("Cargo.toml: missing `[workspace.package] version`".to_owned())
}

fn require_baseline_tag(root: &Path, tag: &str) -> Result<(), String> {
    let reference = format!("refs/tags/{tag}^{{commit}}");
    let status = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--verify", "--quiet", &reference])
        .status()
        .map_err(|err| format!("failed to inspect stable baseline tag `{tag}`: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "stable workspace requires baseline tag `{tag}`; create the reviewed first-stable tag before running the release gate"
        ))
    }
}

fn require_semver_checks(root: &Path) -> Result<(), String> {
    let status = Command::new("cargo")
        .current_dir(root)
        .args(["semver-checks", "--version"])
        .status()
        .map_err(|err| format!("failed to inspect cargo-semver-checks: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("cargo-semver-checks is required for stable API comparison; install it with `cargo install cargo-semver-checks --locked`".to_owned())
    }
}

fn package_name(path: &Path) -> Result<String, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut in_package = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_package = line == "[package]";
            continue;
        }
        if in_package && line.starts_with("name") {
            return quoted_value(line, "name");
        }
    }
    Err(format!("{}: missing `[package] name`", path.display()))
}

fn scalar<'a>(text: &'a str, key: &str) -> Result<&'a str, String> {
    let line = config_line(text, key)?;
    line.split_once('=')
        .map(|(_, value)| value.trim())
        .ok_or_else(|| format!("{CONFIG_PATH}: invalid `{key}` entry"))
}

fn quoted_scalar(text: &str, key: &str) -> Result<String, String> {
    quoted_value(config_line(text, key)?, key).map_err(|err| format!("{CONFIG_PATH}: {err}"))
}

fn quoted_array(text: &str, key: &str) -> Result<Vec<String>, String> {
    let value = scalar(text, key)?;
    let inner = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| format!("{CONFIG_PATH}: `{key}` must be an array"))?;
    inner
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| parse_quoted(value, key))
        .collect()
}

fn config_line<'a>(text: &'a str, key: &str) -> Result<&'a str, String> {
    text.lines()
        .map(str::trim)
        .find(|line| line.starts_with(key) && line[key.len()..].trim_start().starts_with('='))
        .ok_or_else(|| format!("{CONFIG_PATH}: missing `{key}`"))
}

fn quoted_value(line: &str, key: &str) -> Result<String, String> {
    let value = line
        .split_once('=')
        .map(|(_, value)| value.trim())
        .ok_or_else(|| format!("invalid `{key}` entry"))?;
    parse_quoted(value, key)
}

fn parse_quoted(value: &str, key: &str) -> Result<String, String> {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_owned)
        .ok_or_else(|| format!("`{key}` value must be quoted: `{value}`"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos();
            let root = env::temp_dir().join(format!("squonk-semver-{}-{nonce}", process::id()));
            fs::create_dir_all(root.join("crates/a")).expect("create tree");
            fs::write(
                root.join("Cargo.toml"),
                "[workspace]\nmembers = [\"crates/a\"]\n\n[workspace.package]\nversion = \"0.1.0\"\n",
            )
            .expect("write workspace");
            fs::write(
                root.join("crates/a/Cargo.toml"),
                "[package]\nname = \"a\"\nversion = \"0.1.0\"\n",
            )
            .expect("write package");
            fs::create_dir_all(root.join("release")).expect("create release dir");
            Self(root)
        }

        fn config(&self, baseline: &str) {
            fs::write(
                self.0.join(CONFIG_PATH),
                format!("schema = 1\nbaseline_tag = \"{baseline}\"\npackages = [\"crates/a\"]\n"),
            )
            .expect("write config");
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn pre_stable_configuration_is_valid_without_external_tool() {
        let temp = TempTree::new();
        temp.config("v1.0.0");

        assert_eq!(run(&temp.0).expect("valid pre-stable config"), 0);
    }

    #[test]
    fn baseline_is_pinned_to_first_stable_tag() {
        let temp = TempTree::new();
        temp.config("latest");

        let error = load_config(&temp.0, "v1.0.0")
            .err()
            .expect("wrong baseline fails");
        assert!(error.contains("requires API baseline `v1.0.0`"), "{error}");
    }

    #[test]
    fn unpublished_package_is_rejected() {
        let temp = TempTree::new();
        temp.config("v1.0.0");
        fs::write(
            temp.0.join("crates/a/Cargo.toml"),
            "[package]\nname = \"a\"\npublish = false\n",
        )
        .expect("rewrite package");

        let error = load_config(&temp.0, "v1.0.0")
            .err()
            .expect("unpublished package fails");
        assert!(error.contains("not a published package"), "{error}");
    }

    #[test]
    fn omitted_published_package_is_rejected() {
        let temp = TempTree::new();
        temp.config("v1.0.0");
        fs::create_dir_all(temp.0.join("crates/b")).expect("create second package");
        fs::write(
            temp.0.join("crates/b/Cargo.toml"),
            "[package]\nname = \"b\"\nversion = \"0.1.0\"\n",
        )
        .expect("write second package");
        fs::write(
            temp.0.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n\n[workspace.package]\nversion = \"0.1.0\"\n",
        )
        .expect("rewrite workspace");

        let error = load_config(&temp.0, "v1.0.0")
            .err()
            .expect("omitted published package fails");
        assert!(error.contains("must exactly cover"), "{error}");
    }
}
