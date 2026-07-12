// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Dedicated generated-vs-`phf` keyword-lookup comparison binary — the relocation of the
//! former `keyword_lookup` bench into the standardized comparison CLI
//! (`bench-comparison-dedicated-binaries`). Built only under `--features phf-compare`
//! (the `[[example]]` is `required-features`-gated), so a default build never compiles
//! `phf` (ADR-0004 / ADR-0017).
//!
//! The measurement is unchanged: the full ANSI/PostgreSQL inventory, the probe corpora,
//! and both lowerings live in `benches/keyword_lookup_ref/mod.rs`, which this example
//! calls exactly as the bench did. WALL-CLOCK only (both lookups are pure `fn(&str) ->
//! Option<Keyword>` over the same inventory, so there is no allocation to compare).
//!
//! Run: cargo run -p squonk-bench --release --features phf-compare --example compare_keyword_lookup

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[path = "../benches/keyword_lookup_ref/mod.rs"]
mod keyword_lookup_ref;

use keyword_lookup_ref::{Keyword, PROBES, lookup_keyword, lookup_keyword_phf, report_header};
use squonk_bench::compare::{CompareArgs, json_escape, ratio, time_op, write_json_report};
use std::hint::black_box;

/// Look every word up, forcing the result to be observed so neither the lookup nor the
/// loop is optimized away; returns the hit count (the black-box payload).
fn sweep(words: &[&str], lookup: fn(&str) -> Option<Keyword>) -> u64 {
    let mut hits = 0u64;
    for word in words {
        if black_box(lookup(black_box(word))).is_some() {
            hits += 1;
        }
    }
    black_box(hits)
}

struct Timed {
    generated_ns: f64,
    phf_ns: f64,
}

impl Timed {
    fn ratio(&self) -> f64 {
        ratio(self.generated_ns, self.phf_ns)
    }
}

fn main() {
    let args = CompareArgs::parse(2_000, 200_000);
    print!("{}", report_header());
    println!("# mode: WALL-CLOCK (mimalloc); ns per scenario sweep");
    println!(
        "#   warmup {} iters {}  (trend-only)",
        args.warmup, args.iters
    );

    // `all_keywords` — every spelling, the all-hit worst case for binary-search depth —
    // is built here because it is too large to spell out in the corpus.
    let all_keywords: Vec<&'static str> = Keyword::ALL.iter().map(|k| k.as_str()).collect();

    println!(
        "#   {:<18} {:>14} {:>14} {:>8}",
        "scenario", "generated ns", "phf ns", "ratio"
    );
    let mut rows = Vec::new();
    for (name, words) in std::iter::once(("all_keywords", all_keywords.as_slice()))
        .chain(PROBES.iter().map(|probe| (probe.name, probe.words)))
    {
        let t = Timed {
            generated_ns: time_op(args.warmup, args.iters, || sweep(words, lookup_keyword)),
            phf_ns: time_op(args.warmup, args.iters, || sweep(words, lookup_keyword_phf)),
        };
        println!(
            "#   {:<18} {:>14.1} {:>14.1} {:>8.3}",
            name,
            t.generated_ns,
            t.phf_ns,
            t.ratio()
        );
        rows.push((name, t));
    }

    if let Some(path) = &args.json {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "{{");
        let _ = writeln!(out, "  \"comparison\": \"keyword_lookup\",");
        let _ = writeln!(out, "  \"mode\": \"wall_clock\",");
        let _ = writeln!(out, "  \"unit\": \"ns_per_sweep\",");
        let _ = writeln!(out, "  \"warmup\": {},", args.warmup);
        let _ = writeln!(out, "  \"iters\": {},", args.iters);
        let _ = writeln!(
            out,
            "  \"ratio\": \"generated / phf (> 1.0 = the generated lookup is slower)\","
        );
        let _ = writeln!(out, "  \"scenarios\": [");
        for (i, (name, t)) in rows.iter().enumerate() {
            let comma = if i + 1 < rows.len() { "," } else { "" };
            let _ = writeln!(
                out,
                "    {{ \"name\": \"{}\", \"generated_ns\": {:.3}, \"phf_ns\": {:.3}, \"ratio\": {:.3} }}{comma}",
                json_escape(name),
                t.generated_ns,
                t.phf_ns,
                t.ratio()
            );
        }
        let _ = writeln!(out, "  ]");
        let _ = writeln!(out, "}}");
        write_json_report(path, &out);
    }
}
