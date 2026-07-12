# ADR-0016: Performance & allocation gating

- **Status:** Accepted (2026-06-26); amended 2026-07-01 (gungraun gate is callgrind-only — see Amendment)
- **Atoms:** A36

## Context

Speed is squonk' thesis, yet the prior art's CI had no benchmark job — a PR doubling allocations passed green. Wall-clock benchmarks flap on shared runners, so a flapping gate gets ignored. Allocation-heaviness was the core perf complaint, so allocation counts deserve a first-class guard.

## Decision

- **The gate is deterministic:** `gungraun` (the renamed `iai-callgrind`) — Valgrind **instruction counts** (`Ir`, 5% soft-limit, `fail_fast`), immune to CI wall-clock noise. Callgrind-only as of the 2026-07-01 amendment; the DHAT-heap half is carried by the `dhat::assert_eq!` allocation tests (next bullet) instead.
- **`dhat::assert_eq!` allocation-count tests** pin "parsing query X performs exactly N allocations" — the cheapest, most theme-aligned guard.
- **CodSpeed** (`codspeed-criterion-compat`, free for OSS, Memory instrument, merge protection — usable now the repo is public) as a hosted PR-comment complement. `criterion`/`divan` are local-exploration only (never a gate).
- **Bench coverage** broadens well beyond the prior art's `GenericDialect`-only handful: a tokenizer-only path, DDL/DML, deep-nesting, multiple dialects, and analytical corpora (DSB/TPC-DS, fetched/generated).

## Consequences

- Regressions trip the build deterministically; the allocation guard directly protects the per-token-`String`/double-parse wins.
- The gate is **local-runnable** (`cargo bench` / `cargo nextest run` for the dhat asserts) per ADR-0017; it lands in **Phase 0**, live against trivial benches *before* any engine code, so nothing regresses unnoticed from the first commit.

## Amendment (2026-07-01): the gungraun gate is callgrind-only

The `gungraun` gate originally layered a valgrind-DHAT `TotalBytes` soft-limit on top of the callgrind `Ir` gate (`bench/benches/gungraun_gate/mod.rs`). On the first Linux run (gungraun 0.19.2, valgrind 3.25.1) the DHAT half could not execute: gungraun's DHAT consumer cannot parse this valgrind's DHAT output. This is a **version incompatibility in gungraun's DHAT integration, not a valgrind or path bug** — verified by running valgrind's DHAT directly, which writes a valid profile at the exact path gungraun requests.

Rather than pin a fragile valgrind version to satisfy gungraun-DHAT, the gate is now **callgrind-only**. No memory-gating coverage is lost: the memory-regression goal is already served — more strictly — by the `dhat::assert_eq!` allocation tests (`bench/tests/allocations.rs`, `corpus_allocations.rs`, `adversarial_scaling.rs`), which pin EXACT byte/block counts via the portable dhat *crate* (not valgrind-DHAT). Those were already a first-class guard in this ADR; they now solely carry the heap gate.

Restore path: re-add the DHAT tool to `gate_config()` if a gungraun/valgrind pairing that parses cleanly is adopted.

## Interconnects

- code: `bench/benches/gungraun_gate/mod.rs` — the callgrind gate config; `bench/tests/allocations.rs`, `bench/tests/corpus_allocations.rs`, `bench/tests/adversarial_scaling.rs` — `dhat` allocation pins
- invariant: the gungraun gate is callgrind-only; allocation counts are pinned by `dhat::assert_eq!` tests.
- xtask: none — enforced by the bench gate config and the allocation-pin tests.

## References

Atom A36. `gungraun`/`iai-callgrind`, `dhat-rs`, CodSpeed.
