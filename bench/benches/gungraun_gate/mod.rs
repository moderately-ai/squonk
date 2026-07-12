// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared gungraun hard-gate builder (ADR-0016): callgrind `Ir` soft-limited to 5%,
//! `fail_fast`. The three Linux instruction gates — `perf.rs`, `corpus_instr.rs`,
//! `adversarial_instr.rs` — apply this identically, so it is defined once here rather
//! than re-declared per file. Valgrind is Linux-only, so this module (and every
//! consumer's mount of it) is `#[cfg(target_os = "linux")]`-gated.
//!
//! Callgrind-only: the valgrind-dhat `TotalBytes` gate this once also carried was
//! dropped (ADR-0016) because gungraun 0.19.2's dhat consumer cannot read valgrind
//! 3.25.1's dhat output — valgrind writes a valid profile at the expected path, but
//! gungraun fails to parse it (a version incompat, not a valgrind or path bug). The
//! memory-regression goal it served is met — more strictly — by the dhat-*crate*
//! allocation gates (`tests/allocations.rs`, `tests/corpus_allocations.rs`,
//! `tests/adversarial_scaling.rs`), which pin EXACT byte/block counts and are portable
//! across valgrind versions. Restore path: `restore-gungraun-dhat-gate-when-compatible`.

#[cfg(target_os = "linux")]
use gungraun::{Callgrind, EventKind, LibraryBenchmarkConfig};

#[cfg(target_os = "linux")]
pub fn gate_config() -> LibraryBenchmarkConfig {
    let mut callgrind = Callgrind::default();
    callgrind
        .soft_limits([(EventKind::Ir, 5.0)])
        .fail_fast(true);

    let mut config = LibraryBenchmarkConfig::default();
    config.tool(callgrind);
    config
}
