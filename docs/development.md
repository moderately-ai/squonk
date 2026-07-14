<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# Development

## Setup

Install Rust through `rustup`; `rust-toolchain.toml` selects the supported toolchain and
components. Install `cargo-nextest` for the test runner used locally and in CI.

Useful commands:

```sh
cargo check -p squonk
cargo nextest run -p squonk
cargo run -p squonk-sourcegen
cargo xtask preflight
cargo xtask feature-matrix
```

`cargo xtask preflight` is a fixed non-oracle stack: formatting, repository policy checks,
Clippy, nextest, and rustdoc across the core, binding, and conformance crates. Oracle suites
remain explicit because DuckDB and MySQL require engine-specific environments.
CI fans this same stack into hygiene, Clippy, nextest, and rustdoc lanes with
`cargo xtask preflight --only <step,...>`; the unfiltered local command remains the release gate.

Generated AST support code is checked in. Change its source inputs and regenerate it; never
edit `crates/squonk-ast/src/generated/` by hand.

## Oracle status

To disclose whether the curated parity engines ran or skipped:

```sh
cargo nextest run -p squonk-conformance --features oracle-engines,oracle-mysql \
  -E 'test(accept_reject_parity_over_curated_corpus)' --success-output final
```

An `oracle-ran:` line is evidence of execution. A `skipping … differential:` line means the
corresponding engine was unavailable. MySQL remains an external server; GPL client/server
code is never linked or vendored.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for contribution policy and
[docs/adr/](adr/) for architectural decisions.
