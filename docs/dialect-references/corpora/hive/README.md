<!-- SPDX-License-Identifier: MIT -->

# Apache Hive grammar

Vendored HiveQL ANTLR3 grammar for the dialect docs/spec reference library ([[dialect-docs-reference-library]]). Manifest entry: `hive/grammar` in [`../../manifest.toml`](../../manifest.toml). Provenance and pin: [`PROVENANCE.toml`](PROVENANCE.toml).

## Files

- `HiveParser.g` — the HiveQL ANTLR3 parser grammar.
- `HiveLexer.g` — the companion lexer grammar.

These feed the HiveQL parity lanes (`planner-parity-hive`, `oracle-parity-hive`) as the documentary grammar reference.

## Licence

Apache-2.0, verbatim in [`LICENSE`](LICENSE); each `.g` file also carries the ASF licence header inline. On the `cargo xtask license` permissive allowlist.

## Refresh

See `PROVENANCE.toml` `regenerate` and the manifest's version/bump protocol. The pin has no reachable Hive release tag, so it is pinned by commit only.
