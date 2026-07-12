// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Dedicated in-house-interner vs `lasso` vs `string-interner` comparison binary — the
//! relocation of the former `interner_compare` (wall-clock) and `interner_heap` (dhat)
//! benches into one standardized comparison CLI
//! (`bench-comparison-dedicated-binaries`). Built only under `--features interner-compare`
//! (the `[[example]]` is `required-features`-gated), so a default build compiles neither
//! rejected crate (ADR-0017).
//!
//! The measurement is unchanged: the identifier corpus, the three drivers, and the three
//! axes (intern / freeze / lookup) all live in `benches/interner_ref/mod.rs`, which this
//! example calls exactly as the benches did — read its module docs for the fairness
//! caveats (default hashers, the keyword probe, storage models, no string-interner freeze).
//!
//! Two modes, selected at COMPILE time (one `#[global_allocator]` per process):
//!   default (mimalloc)       — WALL-CLOCK: intern / freeze / lookup ns for each interner.
//!   --features compare-heap  — HEAP (dhat): transient + retained(interner/frozen) + peak.
//!
//! Run:
//!   cargo run -p squonk-bench --release --features interner-compare --example compare_interner
//!   cargo run -p squonk-bench --release --features interner-compare,compare-heap --example compare_interner

#[cfg(not(feature = "compare-heap"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "compare-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[path = "../benches/interner_ref/mod.rs"]
mod interner_ref;

use squonk_bench::compare::CompareArgs;

#[cfg(not(feature = "compare-heap"))]
mod wall {
    use crate::interner_ref::{self, lasso_ref, ours, report_header, string_interner_ref};
    use squonk_bench::compare::{
        CompareArgs, json_escape, time_build, time_op, time_transform, write_json_report,
    };

    /// The three per-interner axis timings; `None` where an interner lacks the axis
    /// (string-interner has no frozen form, so no freeze row).
    struct Axes {
        name: &'static str,
        intern_ns: f64,
        freeze_ns: Option<f64>,
        lookup_ns: f64,
    }

    pub fn run(args: &CompareArgs) {
        let words = interner_ref::identifier_corpus();
        print!("{}", report_header(&words));
        println!("# mode: WALL-CLOCK (mimalloc, every side); ns per axis operation");
        println!(
            "#   warmup {} iters {}  (trend-only)",
            args.warmup, args.iters
        );
        let words = words.as_slice();
        let (w, i) = (args.warmup, args.iters);

        // Prebuild the lookup fixtures once (untimed), so the lookup arm times only
        // resolution, exactly like the criterion bench's out-of-loop setup.
        let (ours_i, ours_syms) = ours::populate_with_symbols(words);
        let ours_res = ours::freeze(ours_i);
        let (lasso_r, lasso_syms) = lasso_ref::populate_with_symbols(words);
        let lasso_res = lasso_ref::freeze(lasso_r);
        let (si_i, si_syms) = string_interner_ref::populate_with_symbols(words);

        let axes = [
            Axes {
                name: "ours",
                intern_ns: time_build(w, i, || ours::populate(words)),
                freeze_ns: Some(time_transform(w, i, || ours::populate(words), ours::freeze)),
                lookup_ns: time_op(w, i, || ours::resolve_all(&ours_res, &ours_syms) as u64),
            },
            Axes {
                name: "lasso",
                intern_ns: time_build(w, i, || lasso_ref::populate(words)),
                freeze_ns: Some(time_transform(
                    w,
                    i,
                    || lasso_ref::populate(words),
                    lasso_ref::freeze,
                )),
                lookup_ns: time_op(w, i, || {
                    lasso_ref::resolve_all(&lasso_res, &lasso_syms) as u64
                }),
            },
            Axes {
                name: "string-interner",
                intern_ns: time_build(w, i, || string_interner_ref::populate(words)),
                freeze_ns: None,
                lookup_ns: time_op(w, i, || {
                    string_interner_ref::resolve_all(&si_i, &si_syms) as u64
                }),
            },
        ];

        println!("#");
        println!(
            "# {:<18} {:>14} {:>14} {:>14}",
            "interner", "intern ns", "freeze ns", "lookup ns"
        );
        for a in &axes {
            let freeze = a
                .freeze_ns
                .map_or_else(|| "     n/a".to_owned(), |f| format!("{f:>14.1}"));
            println!(
                "# {:<18} {:>14.1} {freeze} {:>14.1}",
                a.name, a.intern_ns, a.lookup_ns
            );
        }

        if let Some(path) = &args.json {
            use std::fmt::Write as _;
            let mut out = String::new();
            let _ = writeln!(out, "{{");
            let _ = writeln!(out, "  \"comparison\": \"interner\",");
            let _ = writeln!(out, "  \"mode\": \"wall_clock\",");
            let _ = writeln!(out, "  \"unit\": \"ns_per_op\",");
            let _ = writeln!(out, "  \"warmup\": {},", args.warmup);
            let _ = writeln!(out, "  \"iters\": {},", args.iters);
            let _ = writeln!(out, "  \"interners\": [");
            for (k, a) in axes.iter().enumerate() {
                let comma = if k + 1 < axes.len() { "," } else { "" };
                let freeze = a
                    .freeze_ns
                    .map_or_else(|| "null".to_owned(), |f| format!("{f:.3}"));
                let _ = writeln!(
                    out,
                    "    {{ \"name\": \"{}\", \"intern_ns\": {:.3}, \"freeze_ns\": {freeze}, \"lookup_ns\": {:.3} }}{comma}",
                    json_escape(a.name),
                    a.intern_ns,
                    a.lookup_ns
                );
            }
            let _ = writeln!(out, "  ]");
            let _ = writeln!(out, "}}");
            write_json_report(path, &out);
        }
    }
}

#[cfg(feature = "compare-heap")]
mod heap {
    use crate::interner_ref::{self, lasso_ref, ours, report_header, string_interner_ref};
    use squonk_bench::compare::{CompareArgs, json_escape, write_json_report};
    use std::hint::black_box;

    #[derive(Clone, Copy, Default)]
    struct HeapProfile {
        transient_bytes: u64,
        transient_blocks: u64,
        retained_interner_bytes: u64,
        retained_interner_blocks: u64,
        retained_frozen_bytes: u64,
        retained_frozen_blocks: u64,
        peak_bytes: u64,
    }

    fn measure<I, R>(populate: impl FnOnce() -> I, freeze: impl FnOnce(I) -> R) -> HeapProfile {
        let _profiler = dhat::Profiler::builder().testing().build();

        let interner = populate();
        black_box(&interner);
        let live_interner = dhat::HeapStats::get();

        let resolver = freeze(interner);
        black_box(&resolver);
        let live_frozen = dhat::HeapStats::get();
        drop(resolver);

        let done = dhat::HeapStats::get();
        HeapProfile {
            transient_bytes: done.total_bytes,
            transient_blocks: done.total_blocks,
            retained_interner_bytes: live_interner.curr_bytes as u64,
            retained_interner_blocks: live_interner.curr_blocks as u64,
            retained_frozen_bytes: live_frozen.curr_bytes as u64,
            retained_frozen_blocks: live_frozen.curr_blocks as u64,
            peak_bytes: done.max_bytes as u64,
        }
    }

    fn rows(p: &HeapProfile) -> [(&'static str, u64); 7] {
        [
            ("transient_bytes", p.transient_bytes),
            ("transient_blocks", p.transient_blocks),
            ("retained_interner_bytes", p.retained_interner_bytes),
            ("retained_interner_blocks", p.retained_interner_blocks),
            ("retained_frozen_bytes", p.retained_frozen_bytes),
            ("retained_frozen_blocks", p.retained_frozen_blocks),
            ("peak_bytes", p.peak_bytes),
        ]
    }

    pub fn run(args: &CompareArgs) {
        let words = interner_ref::identifier_corpus();
        print!("{}", report_header(&words));
        println!("# mode: HEAP (dhat::Alloc); deterministic transient/retained/peak counts");
        let words = words.as_slice();

        let ours = measure(|| ours::populate(words), ours::freeze);
        let lasso = measure(|| lasso_ref::populate(words), lasso_ref::freeze);
        // string-interner has no frozen form: its "freeze" is the identity, so the two
        // retained rows come out equal, making the un-shed dedup map explicit.
        let string_interner = measure(|| string_interner_ref::populate(words), |i| i);

        println!("#");
        println!(
            "# {:<26} {:>14} {:>14} {:>16}",
            "metric", "ours", "lasso", "string-interner"
        );
        for ((label, o), (_, l), (_, s)) in rows(&ours)
            .into_iter()
            .zip(rows(&lasso))
            .zip(rows(&string_interner))
            .map(|((a, b), c)| (a, b, c))
        {
            println!("# {label:<26} {o:>14} {l:>14} {s:>16}");
        }

        if let Some(path) = &args.json {
            use std::fmt::Write as _;
            let mut out = String::new();
            let _ = writeln!(out, "{{");
            let _ = writeln!(out, "  \"comparison\": \"interner\",");
            let _ = writeln!(out, "  \"mode\": \"heap\",");
            let _ = writeln!(out, "  \"metrics\": [");
            let names = [
                ("ours", &ours),
                ("lasso", &lasso),
                ("string-interner", &string_interner),
            ];
            for (k, (name, p)) in names.iter().enumerate() {
                let comma = if k + 1 < names.len() { "," } else { "" };
                let mut fields = String::new();
                for (label, v) in rows(p) {
                    let _ = write!(fields, ", \"{label}\": {v}");
                }
                let _ = writeln!(
                    out,
                    "    {{ \"name\": \"{}\"{fields} }}{comma}",
                    json_escape(name)
                );
            }
            let _ = writeln!(out, "  ]");
            let _ = writeln!(out, "}}");
            write_json_report(path, &out);
        }
    }
}

fn main() {
    let args = CompareArgs::parse(50, 500);
    #[cfg(not(feature = "compare-heap"))]
    wall::run(&args);
    #[cfg(feature = "compare-heap")]
    heap::run(&args);
}
