<!-- SPDX-License-Identifier: MIT -->

# PostgreSQL reference artifacts

Vendored PostgreSQL reference material for the dialect docs/spec reference library ([[dialect-docs-reference-library]]). Manifest entries: `postgres/kwlist`, `postgres/sql_features` in [`../../manifest.toml`](../../manifest.toml). Provenance and pin: [`PROVENANCE.toml`](PROVENANCE.toml).

## Files

- `kwlist.h` — PostgreSQL's canonical keyword list: every keyword with its category (`UNRESERVED_KEYWORD` / `COL_NAME_KEYWORD` / `TYPE_FUNC_NAME_KEYWORD` / `RESERVED_KEYWORD`) and its `BARE_LABEL`/`AS_LABEL` status. This is the source-of-truth for the per-position reject sets our ANSI/PostgreSQL presets adopt (`RESERVED_COLUMN_NAME`, `RESERVED_FUNCTION_NAME`, …); see `crates/squonk-ast/src/dialect/ansi.rs`, which cites this vendored copy by manifest id + version.
- `sql_features.txt` — PostgreSQL's ISO/IEC 9075 (SQL standard) feature-conformance table: each feature id, name, and supported (YES/NO) verdict. Feeds the ISO feature-taxonomy / spec-level coverage work (the "ISO 9075 + PG sql_features.txt" prior art the coverage programme leans on).

## Licence

PostgreSQL licence (OSI-approved, BSD/MIT-like permissive), verbatim in [`LICENSE`](LICENSE). On the `cargo xtask license` permissive allowlist, so these files may live vendored in-repo.

## Refresh

The pin matches `conformance/corpus/postgres` so the reference library and the PG regression corpus stay on one PostgreSQL revision. See `PROVENANCE.toml` `regenerate` and the manifest's version/bump protocol before bumping.
