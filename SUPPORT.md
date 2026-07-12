<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# Support

Thanks for using `squonk`. This document says where to take questions and what the project promises.

## Where to go

- **Bugs and feature requests** — open a [GitHub issue](https://github.com/moderately-ai/squonk/issues); the [issue forms](.github/ISSUE_TEMPLATE) collect exactly what we need. For a parser bug, the most useful report is a minimal SQL input, the dialect/features you used, what you got, and what you expected. Please search existing issues first.
- **New dialect or SQL syntax** — read the [dialect & syntax request policy](docs/dialect-requests.md) before filing: it states the tier a new dialect enters at, the engine-citation evidence bar, and the minimal request shape (SQL input + dialect + engine citation). Requests go through issues; GitHub Discussions is not enabled at launch.
- **Questions and general contact** — email **opensource@moderately.ai**.
- **Security vulnerabilities** — do **not** use issues or email-to-list; follow [SECURITY.md](SECURITY.md) (GitHub private advisories or security@moderately.ai).

All of the above is governed by our [Code of Conduct](CODE_OF_CONDUCT.md); report conduct concerns to opensource@moderately.ai.

We are not currently accepting outside code contributions (see [CONTRIBUTING.md](CONTRIBUTING.md)), but issue reports and questions are welcome and read.

## What is supported, and how strongly

Support is tiered and documented, not implied — check the tier before relying on a dialect or platform:

- **Dialect and product-surface tiers** — [docs/support-tiers.md](docs/support-tiers.md) states, per shipped dialect preset and per product surface (render, serde schema, wasm, python), whether its parity evidence is **stable** (authoritative engine/standard oracle behind an enforced gate), **preview** (partial or comparison oracle, or a constructed preset), or **experimental** (documentation-derived, no differential oracle; may diverge without notice). The five stable presets — `ansi`, `postgres`, `mysql`, `sqlite`, `duckdb` — are the strongest surface.
- **Platform and target tiers** — [docs/platform-support.md](docs/platform-support.md) states which operating systems and Rust targets are **Tier 1** (built and tested in CI every landing), **Tier 2** (compile-verified), or **Tier 3** (best-effort, no CI), across the Rust crates, the Python wheel, and the npm/WASM package. A registry or README never advertises a platform CI does not at least compile.
- **API stability** — [docs/stable-api.md](docs/stable-api.md) defines the SemVer contract that the `1.x` line upholds: what counts as the public API and what a version bump promises.

## Versions

Security and correctness fixes land on the latest `1.x` release; there are no backports to older patch/minor releases (upgrade forward). See [SECURITY.md](SECURITY.md) for the supported-versions table and [CHANGELOG.md](CHANGELOG.md) for what changed in each release.
