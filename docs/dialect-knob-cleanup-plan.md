# Dialect / AST Knob Organization Cleanup

Locked decisions (2026-07-16) and full work checklist. All items equal priority;
order below is merge-safety only.

## Locked decisions

| ID | Decision |
|---|---|
| DP1 | **Real axis splits** — re-home mis-layered flags into MECE structs |
| DP2 | **Dual-require** primary flags on pipe / companion entry paths |
| DP3 | **`stage_references`**: tokenizer + **parser re-check** |
| DP4 | **Extract table-driven statement-head dispatch** out of `query.rs` |
| DP5 | **New ledger kind** for base-vs-feature heads (not fake multi-claimant) |
| DP6 | **Expand properties** to every stable FeatureSet bool |
| DP7 | **`with_ties_requires_order_by`**: keep one flag (investigation: only PG, always co-varies); expand docs for ORDER BY + SKIP LOCKED |
| DP8 | **`:=` / `#`**: document intentional multi-claim; no new lex flags |

## Execution phases (dependency, not priority)

1. **α** Infrastructure + doctrine notes  
2. **β** Ledger + packaging + small plumbing  
3. **γ** Preset honesty + closed-delta ratchets (all dialects)  
4. **δ** Consumption: dual-require, stage_references, string-literal helpers, dispatch extract  
5. **ε** Axis splits + renames (API churn)  
6. **ζ** Properties expansion + freeze gates  

## Progress (2026-07-16 session)

### Landed (tests green: squonk-ast 303, squonk 1578 under `full`)

| Item | Status |
|---|---|
| DP package locked | done |
| `stage_references` parser re-check | done |
| Dual-require pipe `TABLESAMPLE` / `PIVOT` / `UNPIVOT` | done (primary disjunction `pivot\|\|pivot_value_sources` preserved for BQ/SF) |
| QuiltDb in dialect module inventory | done |
| squonk-ast `lenient` Cargo feature comment | done |
| xtask `serde,document-render` matrix cell | done |
| `with_ties_requires_order_by` dual-behavior docs + naming exception | done |
| Redshift two-axis synopsis (casing + `table_json_path`) | done |
| QuiltDB shares `FeatureSet::POSTGRES.binding_powers` | done |
| Head-contention: DESCRIBE/SUMMARIZE, COPY/COPY INTO | done |
| Head-contention: `BASE_VS_FEATURE_STATEMENT_HEADS` | done |
| Preset/wrapper honesty: BQ, SF, MSSQL, MySQL, SQLite, Databricks, Hive, DuckDB | done (docs + some ratchets) |

### Landed (continued)

| Item | Status |
|---|---|
| **δ Statement dispatch extract** | `parser/statement_dispatch.rs` owns `parse_statement` / head gates |
| **C1.5–C1.6** | Renamed → `transaction_modes_require_commas` / `transaction_modes_reject_duplicates` |
| **C1.7** | FeatureSet field docs for `call_syntax` / `select_syntax` fixed |
| **C4.7** | `try_parse_string_literal_table_name` / `peek_string_literal_table_name` central helpers |
| Axis headers | Utility/Show/Maintenance document statement_dispatch ownership |

### Landed (continued, later session)

| Item | Status |
|---|---|
| **ε TransactionSyntax extract** | TCL flags moved off `UtilitySyntax` onto new top-level `transaction_syntax` (26 fields); sourcegen regenerated; conformance updated |
| Tests | squonk-ast 303, squonk 1578, squonk-conformance 334 green |

### Landed (commits on main)

| Commit theme | Status |
|---|---|
| Plan + packaging honesty + head ledger | committed |
| statement_dispatch + dual-require + stage_references | committed |
| TransactionSyntax extract | committed |
| prefix_colon_alias position split | committed |

### Still open (equal priority)

| Item | Notes |
|---|---|
| **ε remaining axis splits** | StatementDdlGates clause tails; CreateTableClause head vs column (`table_options` AUTO_INCREMENT) |
| **C0.*** | Live catalog script + drift gate |
| **ζ DP6** | Expand properties to every stable FeatureSet bool |
| **C2.17 closed-delta ratchets** | Uniform template for every preset |
| **C4.5–C4.6** | Cross-link `:=` / `#` multi-claim in field docs (tokenizer already notes) |
| **C7 freeze gates** | Orphan scan, synopsis scan, naming lint |

## Checklist

See adversarial synthesis; track completion in PRs. Source findings under
`/tmp/squonk-adversarial-review/` from the review session.

## Non-goals

- Oracle verdict changes unrelated to dual-require / plumbing  
- Performance work  
- Generated-file hand edits (regenerate via sourcegen)
