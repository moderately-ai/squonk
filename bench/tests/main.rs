// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[path = "../benches/upstream/mod.rs"]
mod upstream;

mod adversarial_recursion;
mod adversarial_scaling;
mod allocations;
mod corpus_allocations;
#[cfg(feature = "interner-compare")]
mod interner_reference;
mod iterative_pratt;
#[cfg(feature = "phf-compare")]
mod keyword_phf;
mod logos_reference;
mod ratio_gate_logic;
mod render_allocations;
mod upstream_gate;
