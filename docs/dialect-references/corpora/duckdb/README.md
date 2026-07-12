<!-- SPDX-License-Identifier: MIT -->

# DuckDB keyword-category lists

Vendored DuckDB keyword categories for the dialect docs/spec reference library ([[dialect-docs-reference-library]]). Manifest entry: `duckdb/keywords` in [`../../manifest.toml`](../../manifest.toml). Provenance and pin: [`PROVENANCE.toml`](PROVENANCE.toml).

## Files

DuckDB's own PEG-parser keyword categories (from `src/parser/peg/grammar/keywords/`, not the `third_party/libpg_query` subtree, so cleanly DuckDB MIT):

- `reserved_keyword.list` — fully reserved words.
- `unreserved_keyword.list` — unreserved words (usable as identifiers).
- `column_name_keyword.list` — the `col_name` category.
- `func_name_keyword.list` — the `type_func_name` category.

These diff against our DuckDB keyword inventory (the `kwlist.h` precedent generalized to a second engine).

## Licence

MIT, verbatim in [`LICENSE`](LICENSE). On the `cargo xtask license` permissive allowlist.

## Refresh

See `PROVENANCE.toml` `regenerate` and the manifest's version/bump protocol.
