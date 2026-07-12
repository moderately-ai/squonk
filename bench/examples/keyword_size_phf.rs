// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Size-isolation probe: references ONLY the phf lookup, so the stripped release
//! binary's size reflects the phf map + perfect-hash code (the generated bucket
//! tables are dead-code-eliminated). Sibling of `keyword_size_generated`; see it
//! for the build command and how the size delta isolates the footprint.

#[path = "../benches/keyword_lookup_ref/mod.rs"]
mod keyword_lookup_ref;

use keyword_lookup_ref::{SIZE_PROBE, lookup_keyword_phf};
use std::hint::black_box;

fn main() {
    let mut hits = 0usize;
    for word in SIZE_PROBE {
        if lookup_keyword_phf(black_box(word)).is_some() {
            hits += 1;
        }
    }
    println!("{hits}");
}
