<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# Security Policy

## Reporting a vulnerability

**Do not open a public issue for a security vulnerability.** Public disclosure before a fix is available puts every user at risk.

Report privately through either channel:

1. **GitHub private security advisories (preferred).** On this repository, open [Security → Advisories → Report a vulnerability](https://github.com/moderately-ai/squonk/security/advisories/new). This keeps the report, the discussion, and any coordinated-disclosure timeline in one private thread, and lets us credit you and issue an advisory when the fix ships.
2. **Email.** Write to **security@moderately.ai**. This is a role address, not a personal one, so a report survives any single person being unavailable. If you want to encrypt, say so in a first low-detail message and we will exchange a key.

Please include, as far as you can: the affected version(s) and surface (Rust crate, Python wheel, or npm/WASM package), a minimal reproduction (ideally the SQL input and the dialect/features used), the impact you observed, and any suggested remediation.

## What to expect

We are a small team; these are our good-faith targets, not a contractual SLA:

- **Acknowledgement** within 3 business days that we received the report.
- **Initial assessment** (severity, affected versions, whether we can reproduce) within 10 business days.
- **Coordinated disclosure.** We aim to ship a fix and publish an advisory within 90 days of the initial report, and will keep you updated on progress. We prefer to disclose only after a fixed release is available; if you have a disclosure deadline, tell us up front and we will coordinate.
- **Credit.** With your permission, we credit reporters in the advisory and the [CHANGELOG](CHANGELOG.md) `Security` entry.

## Scope

In scope: the parser and AST behaviour of the published `squonk`/`squonk-ast` crates, the `squonk` Python wheel, and the `squonk` npm/WASM package — for example memory-safety issues, panics or unbounded resource use reachable from parsing untrusted SQL, or a mismatch that causes a consumer to mis-trust parsed structure. The parser is pure safe Rust (`#![forbid(unsafe_code)]` in the first-party crates; the sole `unsafe` is encapsulated in the `thin-vec` leaf dependency), so classic memory-corruption classes are out of the language's reach, but denial-of-service via pathological input is a valid report.

Out of scope: vulnerabilities in downstream applications that merely embed the parser, issues in dependencies that are already publicly tracked upstream, and anything requiring a malicious build environment or modified source tree.

## Fuzzing & differential testing

Untrusted-input robustness is tested continuously. Fuzz targets run one shared body under two engines — stable `cargo test` Bolero checks and nightly libFuzzer binaries — covering parser-never-panics, render round-trips, and recovery-mode invariants; a differential layer compares accept/reject and structure against the real PostgreSQL, SQLite, and DuckDB parsers, so an over-acceptance that a hand-written test would miss surfaces as a crash. The target inventory and the crash → committed-replay workflow are in [`conformance/fuzz/README.md`](conformance/fuzz/README.md). OSS-Fuzz enrolment is a recorded deferral with an explicit revisit trigger — see [ADR-0019](docs/adr/0019-oss-fuzz-readiness.md).

## Supported versions

Security fixes land on the latest `1.x` release and are shipped as a new patch version (crates.io, PyPI, and npm publishes are immutable — the fix is a new version, never an overwrite; see the release runbooks).

| Version | Supported |
| --- | --- |
| `1.x` (latest) | Yes — active security support |
| `1.x` (older patch/minor) | Upgrade to the latest `1.x`; no backports |
| `< 1.0` (pre-release) | No |

When a `2.x` line exists, this table will be updated to state the support window for `1.x`.
