<!-- SPDX-License-Identifier: MIT -->

# Trino SQL grammar

Vendored Trino ANTLR4 grammar for the dialect docs/spec reference library ([[dialect-docs-reference-library]]). Manifest entry: `trino/grammar` in [`../../manifest.toml`](../../manifest.toml). Provenance and pin: [`PROVENANCE.toml`](PROVENANCE.toml).

## Files

- `SqlBase.g4` — the complete ANTLR4 grammar the Trino engine ships (`io.trino:trino-grammar`). The [[dialect-trino]] ticket names this file as the base for the `BuiltinDialect::Trino` preset and for grammar-guided generation in the oracle-parity lanes.

## Licence

Apache-2.0, verbatim in [`LICENSE`](LICENSE); the grammar file also carries the ASF licence header inline. On the `cargo xtask license` permissive allowlist.

## Refresh

See `PROVENANCE.toml` `regenerate` and the manifest's version/bump protocol. Athena is Trino-derived and Presto is a sibling (`prestodb/presto` ships an Apache-2.0 `SqlBase.g4` too) — see the manifest's `presto/grammar` entry for the not-yet-vendored sibling.
