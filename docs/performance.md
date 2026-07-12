# Performance

Full performance numbers, methodology, fairness caveats, and allocator-tuning guidance. The [workspace `README.md`](../README.md) carries only the headline figures; everything below is the detail behind them. Reproducible benchmark programs live under [`bench/`](../bench/README.md).

## Against the prior art

Against the prior art it learns from (`apache/datafusion-sqlparser-rs`), `squonk` parses **~2.8–3.2× faster** (wall-clock, both parsers under a fast allocator) and produces an AST **~15–19× lighter** in transient and peak heap — measured across the vendored single-statement and TPC-H/TPC-DS corpora, and holding a **linear** cost curve on adversarial inputs (deep nesting, wide `IN`/`VALUES`/join lists) that degrade allocation-heavy parsers.

Against `libpg_query` (PostgreSQL's own C parser, run in-process) it is **~16× faster** than the full parse-and-serialize path a Rust consumer actually uses, while paying a deliberate **~1.8×** instruction tax over libpg's *throwaway* raw parse — the cost of retaining a full owned AST with byte spans, interned symbols, and stable node ids that a discarded C tree never builds. The `upstream_*` and `libpg_*` benches reproduce the comparison.

## Allocator tuning

For high-throughput or allocation-heavy parsing, the cheapest available win is the global allocator the *final binary* links — a choice only the consuming application can make, because a library cannot set the global allocator. The published crates allocate per AST node, and swapping in a fast general-purpose allocator (for example [`mimalloc`](https://crates.io/crates/mimalloc) or `jemalloc`) recovers **~15-19% of parse time on alloc-heavy SQL** in our measurements — no `unsafe`, no API change:

```rust
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
```

This is a recommendation, not a default: `squonk` deliberately takes no allocator dependency (ADR-0017 — a parser should not dictate its consumer's allocator). The figure is workload-dependent and honestly bounded — it was measured by `bench/benches/alloc_probe.rs` under `--profile profiling` (release codegen) on an Apple M4 Max, is effectively negligible on tiny statements or any workload that is not allocation-bound, and is already in hand for the many services that run such an allocator anyway.
