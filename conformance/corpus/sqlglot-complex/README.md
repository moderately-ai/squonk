<!-- SPDX-License-Identifier: MIT -->

# sqlglot Complex-Query Corpus

This directory vendors a subset of sqlglot's optimizer test fixtures as a corpus of *realistic, complex* SQL — the counterpart to the single-line `corpus/sqlglot/` identity corpus. Source material comes from the upstream sqlglot repository at https://github.com/tobymao/sqlglot, at commit `fd6d4d61c25e7918118fc22c5579098a86a58e10`, under `tests/fixtures/optimizer/`, and is covered by the MIT license (see `LICENSE`, copied verbatim from that repository).

The corpus is the TPC-H and TPC-DS benchmark suites plus four CTE/subquery optimizer fixtures — deeply nested joins, correlated subqueries, CTEs, INTERVAL/date arithmetic — chosen to exercise the parser on the kind of queries a real analytic workload emits, not the micro-statements the per-statement benches use.

## Files

| File | Source under `tests/fixtures/optimizer/` | Statements |
| --- | --- | --- |
| `tpc-h.sql` | `tpc-h/tpc-h.sql` | 22 |
| `tpc-ds.sql` | `tpc-ds/tpc-ds.sql` | 99 |
| `merge_subqueries.sql` | `merge_subqueries.sql` | 67 |
| `unnest_subqueries.sql` | `unnest_subqueries.sql` | 38 |
| `pushdown_cte_alias_columns.sql` | `pushdown_cte_alias_columns.sql` | 6 |
| `eliminate_ctes.sql` | `eliminate_ctes.sql` | 6 |

Total: **238** input statements.

## Extraction

Each upstream fixture is a sequence of `input; expected-output;` pairs: the first statement is the query under test, the second is sqlglot's optimized rewrite of it. We vendor only the **input** queries. The extraction, verified to yield 22 / 99 on TPC-H / TPC-DS, is:

1. Strip whole-line comments (`--` and `#`).
2. Split the remainder on the statement terminator.
3. Keep the even-indexed (0, 2, 4, …) statements — the inputs. The odd-indexed entries are sqlglot's optimized output and are not vendored.

Each vendored file carries an inline `-- SPDX` banner header (so the `cargo xtask license` gate sees a permissive marker on every file, ADR-0015) followed by the input statements, each terminated by `;` and blank-line separated. No semicolon appears inside any statement, so the loaders recover the statements with a plain split on the terminator; the per-dataset count pins (below) catch any drift. The banner comment lines deliberately contain no semicolon, since a `;` there would be mis-read as a statement boundary.

## Supported subset

As with the identity corpus, the split into supported / unsupported is decided by *running* the parser, never by hand — but the oracle here is **parse acceptance**, not the identity corpus's full canonical + parenthesized round-trip. Parse acceptance is exactly the predicate the upstream-comparison bench uses to choose the both-accept subset it times (`bench/benches/upstream/mod.rs`), so the coverage the conformance test pins (`conformance/src/corpus_complex.rs`) and the subset the bench measures are the same set. (Round-trip stability for these large multi-dialect queries is a separate concern.)

The conformance test classifies every statement as Ansi-accepted, Postgres-only, or unparsed, pins each dataset's candidate count (anti-vanishing) and its per-preset acceptance counts (coverage drift surfaces as a reviewable diff), and reports the breakdown on every run.

## Coverage

At the vendored commit, parse-acceptance coverage is:

| Dataset | Candidates | Ansi | Postgres |
| --- | --- | --- | --- |
| `tpc-h` | 22 | 16 | 16 |
| `tpc-ds` | 99 | 97 | 97 |
| `merge_subqueries` | 67 | 66 | 66 |
| `unnest_subqueries` | 38 | 34 | 36 |
| `pushdown_cte_alias_columns` | 6 | 4 | 4 |
| `eliminate_ctes` | 6 | 5 | 5 |
| **TOTAL** | **238** | **222 (93.3%)** | **224 (94.1%)** |

This runs well ahead of the originating spike's snapshot (TPC-H 7/22, TPC-DS 81/99): typed `DATE`/`TIME`/`TIMESTAMP`/`INTERVAL` literals landed afterwards (`prod-literal-date-time-interval` — "TPC-H Q1 now parses"), along with `CASE`, window functions, subquery predicates and CTE/`VALUES` support. Every Ansi-accepted statement here additionally round-trips structurally under the Ansi oracle, so these are genuine full parses, not lenient accepts. The remaining unparsed statements use features still outside the surface (e.g. `LEFT ANTI JOIN`, `VALUES … AS t(col)` column aliases, an aggregate over a bare subquery).

## How it is used

- `conformance/src/corpus_complex.rs` — the conformance loader: cuts each file into statements, pins the counts above, classifies and reports coverage.
- `bench/benches/upstream/mod.rs` (+ `bench/examples/compare_upstream.rs`) — the upstream comparison: parses the both-accept subset (statements both our parser and `sqlparser-rs` accept) with both parsers, reports per-dataset coverage for each, and pins the ours/theirs heap ratios in `bench/upstream-baseline.json` behind the ratio gate (ADR-0016). Over the both-accept subset we are roughly 10–20× lighter on the heap (transient ≈ 0.05, retained ≈ 0.10, peak ≈ 0.06 ours/theirs) — the byte-span + interner + `NodeId` trade-off (ADR-0002/0005/0011), not a super-linear cost.

## Regenerating

Re-extract the input statements from the pinned commit per the recipe above (see `PROVENANCE.toml`'s `regenerate` field). After re-vendoring, refresh the conformance pins and the bench baseline:

    cargo nextest run -p squonk-conformance corpus_complex
    cargo run -p squonk-bench --release --example compare_upstream --features compare-heap -- --update-baseline

and review the resulting diff.
