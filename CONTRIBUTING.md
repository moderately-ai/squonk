<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# Contributing

Please open an issue before starting a substantial change. Pull requests should be focused,
include tests for observable behavior, and explain any compatibility implications.

## Development

Install the Rust toolchain from `rust-toolchain.toml` and `cargo-nextest`, then run focused
checks while developing:

```sh
cargo check -p squonk
cargo clippy -p squonk --all-targets -- -D warnings
cargo nextest run -p squonk <filter>
```

Before opening a pull request, run:

```sh
cargo fmt --all
cargo xtask preflight
```

CI runs the same fixed, non-oracle preflight stack. Feature wiring changes also require
`cargo xtask feature-matrix`.

## Generated code

Never edit `crates/squonk-ast/src/generated/` directly. Change the AST or sourcegen inputs,
then run:

```sh
cargo run -p squonk-sourcegen
```

When an AST family changes, audit its hand-written `Render` implementations before updating
that family's render-shape fingerprint.

## Dialects and conformance

Dialect behavior belongs in behavior-named feature data, not dialect-identity branches.
Engine behavior must be established against the real engine and promoted into permanent
tests or corpus fixtures. A green oracle test is evidence only when its output confirms the
engine ran rather than skipped.

Known divergence entries must include a concise rationale and a replay test proving the
divergence still exists. Remove an entry when the engines agree; never adjust it merely to
silence a changed result.

See [development setup](docs/development.md), [architecture](docs/architecture.md), and the
[architecture decisions](docs/adr/) for more detail.
