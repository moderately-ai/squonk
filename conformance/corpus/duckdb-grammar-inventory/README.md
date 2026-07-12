<!-- SPDX-License-Identifier: MIT -->

# DuckDB statement-production inventory

The production DENOMINATOR for the DuckDB spec-coverage instrument
([[spec-coverage-duckdb-production-inventory]]). Where the sibling
`corpus/duckdb-testsuite` and `corpus/duckdb-testsuite-tranche2` groups are
*test-derived* measurement surfaces, this group is the *grammar-derived* negative
space: the sorted set of direct alternatives of DuckDB's top-level `stmt` grammar
production. A production stays visible here even when no vendored corpus statement
exercises it — the DuckDB analogue of the PostgreSQL `stmt-productions.txt` instrument.

## Source + pin

DuckDB is MIT (© 2018-2026 Stichting DuckDB Foundation). Pinned to the exact upstream
commit our vendored `libduckdb` oracle links:

- tag `v1.5.4`, commit `08e34c447bae34eaee3723cac61f2878b6bdf787`
  (`duckdb --version` reports `v1.5.4 (Variegata) 08e34c447b`).

DuckDB's parser is a fork of libpg_query. Unlike upstream PostgreSQL — whose top-level
`stmt:` alternatives are inline in `gram.y` — DuckDB factors the alternative list out
into `third_party/libpg_query/grammar/statements.list`, one production per line, and
`scripts/generate_grammar.py` materializes the bison rule from it verbatim:

```python
stmt_list = "stmt: " + "\n\t| ".join(statements) + "\n\t| /*EMPTY*/\n\t{ $$ = NULL; }\n"
```

So `statements.list` *is* the direct-alternative set of the top-level `stmt` production.
`extract_stmt_productions.py` sorts and de-dups it into `stmt-productions.txt` (43
productions). `LICENSE` is DuckDB's licence verbatim; the `.txt` carries an SPDX
`.license` companion; `PROVENANCE.toml` records the pin and the regeneration command.
MIT is on the `cargo xtask license` permissive allowlist (ADR-0015).

Note: the vendored `grammar/grammar_out.output` (a checked-in bison report) is *stale*
relative to `statements.list` — it predates the `AlterDatabaseStmt`, `MergeIntoStmt`, and
`UpdateExtensionsStmt` additions and still lists a since-removed ordering — so the
manifest, not the report, is the denominator source, cross-checked against the compiled
`src_backend_parser_gram.cpp` and the live oracle.

## What the instrument measures

`corpus_duckdb_verdicts.rs` maps both completed test-suite tranches against this
denominator through a grammar-faithful leading-keyword classifier
(`duckdb_stmt_production`), which mirrors the keyword-deterministic `stmt:` dispatch (each
alternative opens with a distinct keyword signature), since libduckdb's C API exposes no
raw parse tree. Three measurement surfaces, kept distinct and separately pinned:

- **Engine-production reach** — `duckdb_statement_production_coverage_is_measured` records
  which productions the `DuckDbOracle` (`PrepareBind`) accepts over each tranche's
  accept-surface. A production counts as reached only on a real oracle accept (bare or
  under the file's provisioned schema).
- **squonk acceptance** — tracked as its own set in the same sweep. Engine reach does
  not imply squonk parses the production; the two columns are pinned independently
  (mirroring the PostgreSQL instrument's `squonk_accepts` column).
- **Uncovered families** — the productions NEITHER tranche reaches.
  `duckdb_uncovered_statement_productions_have_permanent_oracle_probes` closes that
  negative space with one authored, minimally-provisioned probe per uncovered production,
  proving the engine reaches it directly (parse-only via `json_serialize_sql`, then
  `PrepareBind`). Productions that parse but the binder rejects even provisioned are pinned
  separately (e.g. `ALTER … SET SCHEMA` is binder-unimplemented — `Not implemented Error:
  T_AlterObjectSchemaStmt` — analogous to PostgreSQL's grammar-present,
  engine-unimplemented `CreateAssertionStmt`; `IMPORT DATABASE` needs a real exported
  directory the prepare-only harness cannot supply).

The live counts, exercised/unexercised sets, and probe partitions are pinned in
`corpus_duckdb_verdicts.rs`; this README describes provenance and method only. The pins
are a measurement baseline, not a zero gate: a corpus/engine change or a closed gap
re-baselines the relevant const after the fresh oracle output is reviewed, and the
orchestrator files follow-up children from the measured uncovered set.
