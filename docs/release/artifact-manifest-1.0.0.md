<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Moderately AI Inc. -->

# Release artifact manifest — `squonk` 1.0.0

The exact file inventory of every artifact intended for publication at 1.0.0, so the maintainer reviews precisely what ships before any registry upload. Regenerate it from the final release candidate and record the host and tool versions used.

This is a review document, not a gate. It records the shipped set; the enforced gates are the `include` allowlist in each `Cargo.toml`, the maturin build config, the npm `files` list, and `crates/squonk-wasm/size-budget.json`. Reproduce with:

```sh
cargo package --list -p squonk-ast
cargo package --list -p squonk
cd crates/squonk-python && maturin build --release --out ../../dist && maturin sdist --out ../../dist
unzip -l dist/squonk-1.0.0-*.whl ; tar tzf dist/squonk-1.0.0.tar.gz
cd crates/squonk-wasm && npm run build:wasm && npm run build:ts && npm pack --dry-run
```

---

## 1. crates.io — `squonk-ast` (published first)

Packaged **65 files, 5.0 MiB (963.8 KiB compressed)** — `cargo publish --dry-run` verified against a locally-staged dependency. A deliberate `include` allowlist: library sources, integration tests, README, crate-local LICENSE, and the cargo-injected metadata (`.cargo_vcs_info.json`, `Cargo.toml.orig`, `Cargo.lock`). No corpora or build artifacts.

```
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
LICENSE
README.md
src/ast/dcl.rs
src/ast/ddl.rs
src/ast/dml.rs
src/ast/expr.rs
src/ast/ext.rs
src/ast/ident.rs
src/ast/literal.rs
src/ast/match_recognize.rs
src/ast/mod.rs
src/ast/pipe_ops.rs
src/ast/pivot.rs
src/ast/query.rs
src/ast/replication.rs
src/ast/stmt.rs
src/ast/stored_program.rs
src/ast/tcl.rs
src/ast/tests.rs
src/ast/ty.rs
src/ast/util.rs
src/ast/window.rs
src/dialect/ansi.rs
src/dialect/bigquery.rs
src/dialect/clickhouse.rs
src/dialect/conflict.rs
src/dialect/databricks.rs
src/dialect/duckdb.rs
src/dialect/feature_set_generated.rs
src/dialect/head_contention.rs
src/dialect/hive.rs
src/dialect/keyword/generated.rs
src/dialect/keyword.rs
src/dialect/lenient.rs
src/dialect/lex_class.rs
src/dialect/mod.rs
src/dialect/mssql.rs
src/dialect/mysql.rs
src/dialect/postgres.rs
src/dialect/redshift.rs
src/dialect/snowflake.rs
src/dialect/sqlite.rs
src/dialect/standard_catalog.rs
src/dialect/support_tier.rs
src/generated/README.md
src/generated/mod.rs
src/generated/node_id_walk.rs
src/generated/render_skeleton.rs
src/generated/size_asserts.rs
src/generated/spanned.rs
src/generated/visit.rs
src/lib.rs
src/precedence/mod.rs
src/render/dyn_ext.rs
src/render/mod.rs
src/render/nodes.rs
src/render/tests.rs
src/serde_depth.rs
src/vocab/mod.rs
tests/main.rs
tests/serde_nodes.rs
```

## 2. crates.io — `squonk` (published second, depends on `squonk-ast`)

Packaged **92 files, 4.2 MiB (929.1 KiB compressed)**. Adds the three runnable `examples/`, the `format/snapshots/*.snap` fixtures the format tests read, and four integration tests. Same allowlist discipline — no corpora or build artifacts.

```
.cargo_vcs_info.json
Cargo.lock
Cargo.toml
Cargo.toml.orig
LICENSE
README.md
examples/analyze_tables.rs
examples/rewrite_qualify.rs
examples/rewrite_redact.rs
src/bindings.rs
src/dialect/ansi.rs
src/dialect/bigquery.rs
src/dialect/builtin.rs
src/dialect/clickhouse.rs
src/dialect/databricks.rs
src/dialect/duckdb.rs
src/dialect/hive.rs
src/dialect/lenient.rs
src/dialect/mod.rs
src/dialect/mssql.rs
src/dialect/mysql.rs
src/dialect/postgres.rs
src/dialect/redshift.rs
src/dialect/snowflake.rs
src/dialect/sqlite.rs
src/dialect/support.rs
src/error.rs
src/format/comments.rs
src/format/coverage.rs
src/format/doc.rs
src/format/keyword_case.rs
src/format/render.rs
src/format/snapshots/squonk__format__coverage__inventory_comment_comma_side_reordering.snap
src/format/snapshots/squonk__format__coverage__inventory_comment_placement_supported.snap
src/format/snapshots/squonk__format__coverage__inventory_comment_relocation_fragment.snap
src/format/snapshots/squonk__format__coverage__inventory_comment_relocation_statement_boundary.snap
src/format/snapshots/squonk__format__coverage__inventory_fragment_fallback_no_deep_relayout.snap
src/format/snapshots/squonk__format__coverage__inventory_mainstream_structured_layout.snap
src/format/snapshots/squonk__format__coverage__inventory_spelling_uescape_limitation.snap
src/format/snapshots/squonk__format__coverage__inventory_subquery_relayout.snap
src/format/snapshots/squonk__format__coverage__inventory_width_and_style_knobs.snap
src/format/tests.rs
src/format.rs
src/interner/fast_hash.rs
src/interner/mod.rs
src/lib.rs
src/parser/body.rs
src/parser/clause_marks.rs
src/parser/dcl.rs
src/parser/ddl.rs
src/parser/dml.rs
src/parser/dyn_extension.rs
src/parser/engine.rs
src/parser/expr/call.rs
src/parser/expr/collections.rs
src/parser/expr/core.rs
src/parser/expr/keyword_forms.rs
src/parser/expr/literals.rs
src/parser/expr/mod.rs
src/parser/expr/primary.rs
src/parser/expr/sqljson.rs
src/parser/expr/string_funcs.rs
src/parser/expr/tests.rs
src/parser/expr/xml.rs
src/parser/extension_operators.rs
src/parser/from.rs
src/parser/match_recognize.rs
src/parser/mod.rs
src/parser/node_id.rs
src/parser/parsed.rs
src/parser/pivot.rs
src/parser/query.rs
src/parser/recovery.rs
src/parser/recursion.rs
src/parser/select.rs
src/parser/signal.rs
src/parser/streaming.rs
src/parser/tcl.rs
src/parser/ty.rs
src/parser/util.rs
src/parser/window.rs
src/render.rs
src/tokenizer/cursor.rs
src/tokenizer/error.rs
src/tokenizer/mod.rs
src/tokenizer/scan.rs
src/tokenizer/token.rs
src/tokenizer/trivia.rs
tests/cross_tree_safety.rs
tests/main.rs
tests/serde_parsed.rs
tests/wire_schema.rs
```

## 3. PyPI — `squonk` wheel (`cp39-abi3`)

Built artifact `squonk-1.0.0-cp39-abi3-macosx_11_0_arm64.whl` — **~4.0 MiB compressed / ~12.5 MiB unpacked (15 entries)**, dominated by the `_native.abi3.so` extension (~12.3 MiB, every built-in dialect). `twine check` **PASSED**. The published wheel matrix additionally covers manylinux2014 x86_64, macOS 10.12 x86_64, and win_amd64 — each built and native-smoked on its own CI runner (see `python-distribution.md`); only this host's arm64 wheel was built locally for the gate.

```
squonk/__init__.py
squonk/__init__.pyi
squonk/_ast.py
squonk/_ast.pyi
squonk/_ast_metadata.py
squonk/_exceptions.py
squonk/_exceptions.pyi
squonk/_native.abi3.so
squonk/_types.py
squonk/py.typed
squonk-1.0.0.dist-info/METADATA
squonk-1.0.0.dist-info/WHEEL
squonk-1.0.0.dist-info/licenses/LICENSE
squonk-1.0.0.dist-info/sboms/squonk-python.cyclonedx.json
squonk-1.0.0.dist-info/RECORD
```

**Review note:** maturin 1.14 auto-emits a CycloneDX SBOM at `dist-info/sboms/squonk-python.cyclonedx.json`. This is a supply-chain-positive addition not described in the runbook's file-list prose (written against an earlier maturin); it is a legitimate metadata file, not a stray. LICENSE ships under `dist-info/licenses/` per PEP 639. No source tree, no `.map`, no stray files.

## 4. PyPI — `squonk` sdist (`squonk-1.0.0.tar.gz`)

Source distribution (~2.0 MiB), compiled from source on install. `twine check` **PASSED**. Contains the workspace manifests + `Cargo.lock`, the three first-party crates that build the extension (`squonk`, `squonk-ast`, `squonk-python`) with their sources/tests/examples, the top-level and per-crate LICENSE + README, `pyproject.toml`, and the pure-Python package under `python/squonk/`. Full listing (176 entries) reproduced by `tar tzf dist/squonk-1.0.0.tar.gz`; the crate-source portion mirrors §1–§2 above under `squonk-1.0.0/crates/`.

Top-level shape:

```
squonk-1.0.0/PKG-INFO
squonk-1.0.0/Cargo.lock
squonk-1.0.0/Cargo.toml
squonk-1.0.0/LICENSE
squonk-1.0.0/README.md
squonk-1.0.0/pyproject.toml
squonk-1.0.0/crates/squonk/…            (parser sources, examples, tests — as §2)
squonk-1.0.0/crates/squonk-ast/…        (AST sources, tests — as §1)
squonk-1.0.0/crates/squonk-python/…     (Cargo.toml, LICENSE, README, examples, python/tests, src/lib.rs)
squonk-1.0.0/python/squonk/…            (__init__, _ast, _ast_metadata, _exceptions, _types, *.pyi, py.typed)
```

## 5. npm package family (wasm bindings)

Fresh measurements from the exact npm registry tarballs published by the release build:

| Package | WASM raw | WASM gzip | Tarball | Unpacked | Entries |
| --- | ---: | ---: | ---: | ---: | ---: |
| `@squonk-sql/ansi` | 3,778,544 | 680,359 | 748,950 | 4,104,121 | 18 |
| `@squonk-sql/postgres` | 4,163,252 | 761,532 | 830,243 | 4,489,360 | 18 |
| `@squonk-sql/mysql` | 4,335,554 | 819,499 | 888,852 | 4,661,484 | 18 |
| `@squonk-sql/sqlite` | 3,970,167 | 714,612 | 782,584 | 4,296,155 | 18 |
| `@squonk-sql/duckdb` | 4,184,756 | 770,241 | 838,848 | 4,510,744 | 18 |
| `@squonk-sql/lenient` | 4,702,263 | 919,477 | 988,759 | 5,028,309 | 18 |
| `squonk` | 7,426,957 | 1,108,524 | 1,180,586 | 7,760,094 | 18 |

Each self-contained package contains one WASM artifact, matching Node and browser entrypoints, the shared runtime, generated AST declarations and metadata, manifest, README, and LICENSE. No TypeScript sources, maps, reports, build tools, or `node_modules` ship. Per-package raw/gzip/packed/unpacked ceilings have 10% measured headroom; entry count is pinned exactly at 18.

The six focused packages contain ANSI plus their named dialect. The `squonk` umbrella contains all 13 built-in dialects. Every package includes document rendering and formatting, so the former serialize-only / `*-full` matrix is gone.

**Publish guard:** the checked-in build manifest stays private. Staged manifests are also private until the protected publish job flips the exact verified copies, repeats each dry-run, and publishes with provenance.

---

## Cross-artifact facts

- **Version:** every artifact is `1.0.0` — workspace `[workspace.package] version`, the two `[workspace.dependencies]` pins, the wheel (dynamic, derived), and `package.json`.
- **Name:** Rust and Python use `squonk`; npm adds six `@squonk-sql/*` focused packages alongside the `squonk` umbrella. `squonk-ast` is the second crates.io crate; `squonk-wasm` stays internal.
- **License:** MIT everywhere; each artifact carries its own LICENSE copy (crate-local, `dist-info/licenses/`, npm root).
- **Not shipped:** `squonk-sourcegen`, `squonk-bench` (`publish = false`); test corpora and CI/build artifacts.
- **Pending before publish:** the human-gated `v1.0.0` tag and registry uploads.
