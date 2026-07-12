<!-- SPDX-License-Identifier: MIT -->
# libFuzzer targets

libFuzzer fuzz targets for `squonk`, driven by [cargo-fuzz]. They reuse the
*same* target bodies and generators as the stable `cargo test` Bolero checks in
`squonk-conformance` (`src/fuzz.rs`), so there is one harness with two engines
(ADR-0014/0015): Bolero under stable `cargo nextest`, libFuzzer here.

This is a **standalone, nightly-only** crate. It declares its own `[workspace]`, so
the top-level workspace — the published crates and `cargo test` — never builds it and
stays installable on stable (`rust-toolchain.toml`).

**OSS-Fuzz status: deferred — see [ADR-0019](../../docs/adr/0019-oss-fuzz-readiness.md).**

## Targets

Bodies live under `squonk_conformance::` at the module shown.

| Target | Body | What it asserts |
|---|---|---|
| `parse_no_panic` | `fuzz::parse_no_panic` | The parser never panics on arbitrary bytes, under all four built-in dialects (Ansi, PostgreSQL, MySQL, Lenient). Every successful parse is also rendered in all three `RenderMode`s (discarding the output) so a render panic reachable only from a parsed-not-generated shape is caught too. |
| `roundtrip` | `fuzz::roundtrip_arbitrary_input` | A generated AST renders and re-parses to the same structure. |
| `differential` | `fuzz::differential_arbitrary_input` | A generated statement agrees with the real PostgreSQL parser (`pg_query`) on accept/reject and structure. |
| `pg_differential_raw_bytes` | `fuzz::pg_differential_raw_bytes` | *Raw* bytes (not a generated AST) agree with `pg_query` on accept/reject. The over-acceptance hunter: it searches the full raw-input space for the validator-correctness class (accepting SQL PostgreSQL rejects, or vice versa) that the generated `differential` — legal trees only — cannot reach. |
| `sqlite_differential_raw_bytes` | `fuzz::sqlite_differential_raw_bytes` | *Raw* bytes agree with real SQLite on accept/reject **and statement count**. Parse-only, never-execute oracle over `sqlite3_prepare_v2` + `pzTail` (a name-resolution reject reads as a parse accept, so the comparison is syntactic). Needs `--features oracle-engines` (rusqlite); a no-op without it. |
| `duckdb_differential_raw_bytes` | `fuzz::duckdb_differential_raw_bytes` | *Raw* bytes agree with real DuckDB on accept/reject **and statement count**. Parse-only, never-execute oracle over `duckdb_extract_statements` (the parser, not the preparer — so DuckDB's "prepare executes all but the last statement" hazard never fires). Needs `--features oracle-engines` (system libduckdb); a no-op without it. |
| `recover_invariants` | `recovery_invariants::recover_invariants` | `parse_recovering` never panics and its recovered partial tree holds the whole-tree invariants (unique nonzero NodeIds, non-synthetic in-bounds spans, symbol resolvability) across the resync boundaries the resilient path introduces, under all four dialects. |

## Running (requires nightly)

```sh
cargo install cargo-fuzz          # once
cargo +nightly fuzz build          # build all targets with sanitizer coverage
cargo +nightly fuzz run roundtrip  # fuzz until a crash (Ctrl-C to stop)

# bounded runs, useful in CI / smoke checks:
cargo +nightly fuzz run parse_no_panic -- -runs=100000 -max_total_time=30

# the SQLite/DuckDB differentials need the in-process oracles (rusqlite + libduckdb):
cargo +nightly fuzz run sqlite_differential_raw_bytes --features oracle-engines -- -max_total_time=30
cargo +nightly fuzz run duckdb_differential_raw_bytes --features oracle-engines -- -max_total_time=30
```

The growing input `corpus/` and any crash `artifacts/` are generated and
git-ignored.

## Crash → replay workflow

A crash is reproduced and shrunk, then promoted into the stable suite so it is
re-checked on every `cargo nextest` run (the shared crash corpus):

```sh
cargo +nightly fuzz fmt  <target> artifacts/<target>/crash-…   # show the input
cargo +nightly fuzz tmin <target> artifacts/<target>/crash-…   # minimize it
```

Commit the minimized bytes to the matching replay constant so the stable
`*_replays_committed_inputs` tests replay it without nightly:

| Target | Replay constant | File |
|---|---|---|
| `parse_no_panic` | `PARSE_NO_PANIC_REPLAYS` | `conformance/src/fuzz.rs` |
| `roundtrip` | `ROUNDTRIP_REPLAYS` | `conformance/src/fuzz.rs` |
| `differential` | `DIFFERENTIAL_REPLAYS` | `conformance/src/fuzz.rs` |
| `pg_differential_raw_bytes` | `PG_DIFFERENTIAL_RAW_BYTES_REPLAYS` | `conformance/src/fuzz.rs` |
| `sqlite_differential_raw_bytes` | `SQLITE_DIFFERENTIAL_RAW_BYTES_REPLAYS` | `conformance/src/fuzz.rs` |
| `duckdb_differential_raw_bytes` | `DUCKDB_DIFFERENTIAL_RAW_BYTES_REPLAYS` | `conformance/src/fuzz.rs` |
| `recover_invariants` | `RECOVER_INVARIANTS_REPLAYS` | `conformance/src/recovery_invariants.rs` |

A `pg_differential_raw_bytes` crash is an accept/reject *divergence*, not a panic:
triage it into `PG_DIVERGENCE_ALLOWLIST` (naming a ticket) or fix the parser/mapping
before pinning the seed, so the committed replay stays divergence-free and green. A
`sqlite_/duckdb_differential_raw_bytes` crash is likewise an accept/reject or
segmentation divergence: triage into `M2_DIVERGENCE_ALLOWLIST` (`conformance/src/m2.rs`,
naming a ticket) or fix the parser; the `*_replays_committed_inputs` tests for these two
run only under `--features oracle-engines`. The
corpus-minimization automation is tracked by `prod-fuzz-crash-corpus-workflow`;
OSS-Fuzz packaging by `prod-fuzz-oss-fuzz-readiness`.
