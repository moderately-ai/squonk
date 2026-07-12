// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Size-isolation probe: references ONLY the generated lookup, so the stripped
//! release binary's size reflects the generated bucket tables (the phf map is
//! dead-code-eliminated). Its size minus `keyword_size_phf`'s isolates the
//! generated-vs-phf footprint, with the shared std baseline cancelling in the diff.
//!
//! Build (both, stripped via the release profile's `strip = true`):
//! `cargo build -p squonk-bench --features phf-compare --release \
//!     --example keyword_size_generated --example keyword_size_phf`

#[path = "../benches/keyword_lookup_ref/mod.rs"]
mod keyword_lookup_ref;

use keyword_lookup_ref::{SIZE_PROBE, lookup_keyword};
use std::hint::black_box;

fn main() {
    let mut hits = 0usize;
    for word in SIZE_PROBE {
        if lookup_keyword(black_box(word)).is_some() {
            hits += 1;
        }
    }
    println!("{hits}");
}
