// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! `cargo xtask oracle-mysql` — run the external-server oracle suites with the
//! MySQL dependency provisioned automatically.
//!
//! The m3 differential oracle and the publication-corpus gate speak the wire
//! protocol to a real `mysqld` (`conformance/src/m3.rs`); CI provisions one as a
//! `mysql:8` service container (`oracle-nightly.yml`), while a local
//! `cargo nextest run --all-features` used to require the contributor to have a
//! server already listening — the publication gate hard-fails (by design: a
//! publication claim must never be silently unverified) and the m3 sweeps
//! silently skip. This lane closes that gap deterministically:
//!
//! 1. `MYSQL_ORACLE_URL` set → the caller owns the server; verify it is
//!    reachable and run the suite (never touch docker).
//! 2. Something already listens on the default `127.0.0.1:3306` → reuse it,
//!    lifecycle untouched (the CI-service shape).
//! 3. Otherwise → start the same `mysql:8` image CI uses via `docker run`,
//!    wait for readiness, run the suite, and ALWAYS remove the container it
//!    started — success, failure, or health timeout.
//!
//! The suite is `-p squonk-conformance --features oracle-engines,oracle-mysql`:
//! the in-process SQLite/DuckDB oracles need no provisioning, and combining the
//! features is what compiles the publication-corpus gate. Shells out to `docker`
//! and `cargo nextest` only, keeping xtask dependency-free (ADR-0017), exactly
//! like the `semver` lane shells out to `cargo-semver-checks`.

use std::net::{TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

/// The env var the m3 oracle reads (`conformance::m3::MYSQL_ORACLE_URL_ENV`).
const MYSQL_ORACLE_URL_ENV: &str = "MYSQL_ORACLE_URL";

/// Host/port of `conformance::m3::DEFAULT_MYSQL_ORACLE_URL`.
const DEFAULT_ADDR: &str = "127.0.0.1:3306";

/// The exact image the `oracle-nightly.yml` MySQL service provisions.
const IMAGE: &str = "mysql:8";

/// Fixed container name so a crashed prior run is visible and reclaimable
/// instead of leaking anonymous containers.
const CONTAINER: &str = "squonk-oracle-mysql";

/// How long to wait for a freshly started server to accept connections. The
/// mysql:8 entrypoint runs a temporary init server first and binds TCP only in
/// its final phase, so TCP readiness is the real signal; first-run init on a
/// cold volume takes tens of seconds.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(180);

pub fn run(root: &Path) -> Result<i32, String> {
    if let Some(url) = std::env::var_os(MYSQL_ORACLE_URL_ENV) {
        let url = url
            .into_string()
            .map_err(|_| format!("{MYSQL_ORACLE_URL_ENV} is not valid UTF-8"))?;
        let addr = host_port_of(&url)?;
        if !reachable(&addr) {
            return Err(format!(
                "{MYSQL_ORACLE_URL_ENV} is set to `{url}` but nothing accepts connections at \
                 {addr}; an explicit URL means the caller owns the server lifecycle, so start it \
                 (or unset the variable to let this lane provision a `{IMAGE}` container)"
            ));
        }
        println!("oracle-mysql: using the caller-managed server from {MYSQL_ORACLE_URL_ENV}");
        return run_suite(root);
    }

    if reachable(DEFAULT_ADDR) {
        println!("oracle-mysql: reusing the server already listening on {DEFAULT_ADDR}");
        return run_suite(root);
    }

    ensure_docker()?;
    start_container()?;
    println!("oracle-mysql: started `{IMAGE}` as `{CONTAINER}`, waiting for readiness");
    // From here every path must tear the container down — it is ours.
    let result = match wait_ready() {
        Ok(()) => run_suite(root),
        Err(err) => Err(err),
    };
    remove_container();
    result
}

/// Extract `host:port` from a `mysql://[user[:pass]@]host[:port][/db]` URL, so the
/// reachability probe checks the same endpoint the oracle will dial. Hand-rolled
/// (no url crate): take what follows the last `@` (or the `//`), stop at `/`,
/// default the port to 3306.
fn host_port_of(url: &str) -> Result<String, String> {
    let after_scheme = url.split_once("//").map_or(url, |(_, rest)| rest);
    let after_user = after_scheme
        .rsplit_once('@')
        .map_or(after_scheme, |(_, rest)| rest);
    let host_port = after_user.split(['/', '?']).next().unwrap_or(after_user);
    if host_port.is_empty() {
        return Err(format!("cannot extract host:port from `{url}`"));
    }
    Ok(if host_port.contains(':') {
        host_port.to_owned()
    } else {
        format!("{host_port}:3306")
    })
}

/// True when a TCP connection to `addr` succeeds within a short timeout.
fn reachable(addr: &str) -> bool {
    let Ok(mut candidates) = addr.to_socket_addrs() else {
        return false;
    };
    candidates
        .any(|candidate| TcpStream::connect_timeout(&candidate, Duration::from_millis(500)).is_ok())
}

fn ensure_docker() -> Result<(), String> {
    let status = Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|err| format!("failed to run docker: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "no server on {DEFAULT_ADDR} and the docker daemon is unavailable; start docker (or \
             any MySQL 8 server on {DEFAULT_ADDR}, or point {MYSQL_ORACLE_URL_ENV} at one) and \
             re-run"
        ))
    }
}

fn start_container() -> Result<(), String> {
    // Reclaim a stale container from a crashed prior run before starting fresh.
    remove_container();
    let output = Command::new("docker")
        .args([
            "run",
            "--detach",
            "--name",
            CONTAINER,
            "--env",
            "MYSQL_ALLOW_EMPTY_PASSWORD=yes",
            "--publish",
            "3306:3306",
            IMAGE,
        ])
        .output()
        .map_err(|err| format!("failed to run docker: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "docker run {IMAGE} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Wait until the server's final phase accepts TCP and answers `mysqladmin ping`.
fn wait_ready() -> Result<(), String> {
    let start = Instant::now();
    while start.elapsed() < HEALTH_TIMEOUT {
        // `-h 127.0.0.1` forces TCP inside the container: the init-phase temp
        // server only binds the unix socket, so a TCP ping proves the real one.
        if reachable(DEFAULT_ADDR) {
            let ping = Command::new("docker")
                .args([
                    "exec",
                    CONTAINER,
                    "mysqladmin",
                    "ping",
                    "--silent",
                    "-h",
                    "127.0.0.1",
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
            if ping.is_ok_and(|status| status.success()) {
                return Ok(());
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Err(format!(
        "`{CONTAINER}` did not become ready within {}s; inspect it with `docker logs {CONTAINER}`",
        HEALTH_TIMEOUT.as_secs()
    ))
}

/// Best-effort removal; failure to remove is reported but never masks the suite result.
fn remove_container() {
    let removed = Command::new("docker")
        .args(["rm", "--force", CONTAINER])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if !removed.is_ok_and(|status| status.success()) {
        eprintln!("oracle-mysql: warning: could not remove container `{CONTAINER}`");
    }
}

fn run_suite(root: &Path) -> Result<i32, String> {
    println!(
        "oracle-mysql: cargo nextest run -p squonk-conformance --features oracle-engines,oracle-mysql"
    );
    let status = Command::new("cargo")
        .current_dir(root)
        .args([
            "nextest",
            "run",
            "-p",
            "squonk-conformance",
            "--features",
            "oracle-engines,oracle-mysql",
        ])
        .status()
        .map_err(|err| format!("failed to run cargo nextest: {err}"))?;
    Ok(status.code().unwrap_or(1))
}
