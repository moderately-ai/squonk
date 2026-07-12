<!-- SPDX-License-Identifier: MIT -->
# Benchmarks

The measurement crate behind the project's headline performance claims — `~2.8–3.2×
faster` parses and a `~15–19×` lighter AST than `datafusion-sqlparser-rs`, and the
`libpg_query` instruction-tax numbers. It is `publish = false`: a private harness, never
a shipped crate. The synthesized numbers and their caveats live in [`docs/performance.md`](../docs/performance.md);
the raw method and every not-adopted-dependency measurement live in [`notes/`](notes/),
indexed by [`notes/competitive-perf-index.md`](notes/competitive-perf-index.md) (every
parser measured faster than us on any axis, with mechanism and pursuable lever).

## Two signals, one honest stance

There are two kinds of measurement here and they are gated differently on purpose:

- **Instruction counts (the CI gate).** The `*_instr` benches and `perf` use
  [gungraun]/callgrind, which counts retired instructions (`Ir`) — deterministic and
  machine-independent, so a soft regression fails CI ([ADR-0016](../docs/adr/0016-perf-gating.md)).
  These are the hard signal. **Valgrind is Linux-only**, so the whole family is
  `cfg`-gated with a non-Linux skip `main`: on macOS they build and no-op.
- **Wall-clock (tracked, never gated).** The `ast` micro-benchmarks and the
  `compare_*` examples measure wall-clock time, which flaps on a shared machine and so
  **never fails a build** — it is tracked for drift (`ast` via CodSpeed's historical
  diffing) but is not a gate. This follows Criterion's own guidance that wall-clock
  benchmarks are too noisy to gate a CI pipeline on
  ([Criterion FAQ](https://bheisler.github.io/criterion.rs/book/faq.html#how-should-i-run-criterionrs-benchmarks-in-a-ci-pipeline)).
  Run them locally, read them as trends, and trust the `Ir` gate for regressions.

**Fairness invariant — mimalloc on both sides.** Every ours-vs-external *wall-clock*
comparison builds both parsers under [`mimalloc`](https://crates.io/crates/mimalloc), the
realistic fast allocator a perf-sensitive consumer actually ships (ADR-0017 keeps it out
of the *production* crates — it is a bench-only dev-dependency). A ratio measured with our
per-node allocation under the system allocator and theirs under a fast one would be a
handicap, not a comparison; mimalloc-both-sides is what makes the ratio compare equivalent
work. The *heap* comparisons instead swap in `dhat::Alloc` (deterministic byte accounting)
and so cannot share a binary with the wall-clock allocator — hence the compile-time
`compare-heap` split below.

## Families and how to reproduce each

Run everything from the repo root. The `compare_*` binaries are **examples**, not benches,
because their "theirs" side lives in `[dev-dependencies]` (which `[[bin]]` targets cannot
see); the ours-only regression lanes are `[[bench]]` targets.

| Family | What it measures | Reproduce |
|---|---|---|
| Wall-clock micro (ours) | tokenizer / parser / renderer time on fixed single statements, CodSpeed-tracked | `cargo bench -p squonk-bench --bench ast` |
| Instruction gate (ours) | `Ir` on the same three paths — the hard CI regression gate | `cargo bench -p squonk-bench --bench perf` *(Linux + valgrind)* |
| vs `sqlparser` — wall-clock | ours/theirs ns over the both-accept curated + complex corpus | `cargo run -p squonk-bench --release --example compare_upstream` |
| vs `sqlparser` — heap | transient/retained/peak heap; rewrites `upstream-baseline.json` (ratio gate) | `cargo run -p squonk-bench --release --example compare_upstream --features compare-heap -- --update-baseline` |
| vs `sqlparser` — `Ir` | gungraun instruction ratio | `cargo bench -p squonk-bench --bench upstream_instr` *(Linux + valgrind)* |
| vs `libpg_query` — compute | ours/theirs ns, in-process PostgreSQL C parser (compute only; heap not comparable across the C boundary) | `cargo run -p squonk-bench --release --example compare_libpg` |
| vs `libpg_query` — `Ir` | gungraun instruction ratio, ours vs the C parser | `cargo bench -p squonk-bench --bench libpg_instr` *(Linux + valgrind)* |
| Adversarial — heap | ours-vs-`sqlparser` heap scaling on pathological width/depth | `cargo run -p squonk-bench --release --example compare_adversarial` |
| Adversarial — `Ir` | ours-only worst-case-width instruction gate | `cargo bench -p squonk-bench --bench adversarial_instr` *(Linux + valgrind)* |
| Corpus scale | aggregate heap / `Ir` over the vendored conformance corpora | `cargo bench -p squonk-bench --bench corpus_heap` · `--bench corpus_instr` *(Linux + valgrind)* |
| Allocation isolation | how much of the libpg gap is malloc/free (`alloc_probe`) and per-node `Meta` cost (`meta_probe`) | `cargo bench -p squonk-bench --bench alloc_probe` · `--bench meta_probe` |
| Tokenizer vs `logos` | hand cursor vs the ADR-0005 `logos` reference (wall-clock \| heap) | `cargo run -p squonk-bench --release --example compare_tokenizer_logos` |
| Interner vs `lasso`/`string-interner` | our in-house interner vs the rejected crates (ADR-0003) | `cargo run -p squonk-bench --release --example compare_interner --features interner-compare` |
| Keyword lookup vs `phf` | dep-free generated lookup vs a perfect hash (ADR-0004) | `cargo run -p squonk-bench --release --example compare_keyword_lookup --features phf-compare` |

The `logos`/`lasso`/`string-interner`/`phf` crates are **measured, never adopted**
(ADR-0017): each sits behind a `required-features` flag so the default bench build — and
the `cargo nextest` / `clippy` gates — pull in none of them. Add `--features compare-heap`
to any wall-clock `compare_*` binary that carries both modes (`compare_upstream`,
`compare_tokenizer_logos`, `compare_interner`) to switch it to the dhat heap comparison.

### Cross-language positioning

[`cross-language/`](cross-language/) holds the sqlglot (Python), Apache Calcite / JSQLParser
(Java), and sql-formatter (Node) throughput harnesses. These run in a real environment with
the peer toolchains installed, **not** in the sandboxed build — they compare across runtimes
(JVM JIT, CPython), so they are honest *positioning* throughput, not per-parse algorithm
ratios. The Node harness has an npm entry point (`cd cross-language && npm run
throughput:postgres`); the workup and every runtime caveat are in
[`notes/cross-language-comparison.md`](notes/cross-language-comparison.md) and
[`notes/js-sql-formatter-comparison.md`](notes/js-sql-formatter-comparison.md).

## No oracle, no external engine required

Nothing in this crate needs a database oracle. The bench integration tests
([`tests/`](tests/) — the deterministic ratio/scaling gates that back the benches) green
under a plain `cargo nextest run`, and `cargo xtask preflight` is exactly what
CI runs. The one cross-boundary dependency is `libpg_query` for the `compare_libpg` /
`libpg_instr` family (a bundled C build via the `pg_query` dev-dependency, always present).
This is the same no-oracle contributor path documented for the conformance suite in
[`../conformance/README.md`](../conformance/README.md#running-without-an-oracle) and
[`CONTRIBUTING.md` § Oracle environments](../CONTRIBUTING.md#oracle-environments).

[gungraun]: https://crates.io/crates/gungraun
