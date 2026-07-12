// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Dedicated cursor-vs-`logos` comparison binary — the relocation of the former
//! `tokenizer_logos` (wall-clock) and `tokenizer_logos_heap` (dhat) benches into one
//! standardized comparison CLI (`bench-comparison-dedicated-binaries`).
//!
//! The measurement is unchanged: the regular-SQL corpus (`TOKENIZER_CASES`), the shared
//! `lookup_keyword`, and the same-capacity token buffer all live in
//! `benches/logos_ref/mod.rs` + `squonk_bench`, which this example calls exactly as
//! the benches did — on the REGULAR subset logos can express (the non-regular cases stay
//! owned by the cursor; see `report_header`).
//!
//! Two modes, selected at COMPILE time (one `#[global_allocator]` per process):
//!   default (mimalloc)        — WALL-CLOCK: cursor vs logos ns per case + aggregate.
//!   --features compare-heap   — HEAP (dhat): transient bytes/blocks + peak, cursor vs logos.
//!
//! Run:
//!   cargo run -p squonk-bench --release --example compare_tokenizer_logos
//!   cargo run -p squonk-bench --release --example compare_tokenizer_logos --features compare-heap

#[cfg(not(feature = "compare-heap"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "compare-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[path = "../benches/logos_ref/mod.rs"]
mod logos_ref;

use squonk_bench::compare::CompareArgs;

#[cfg(not(feature = "compare-heap"))]
mod wall {
    use crate::logos_ref::{report_header, tokenize_logos};
    use squonk_bench::compare::{CompareArgs, json_escape, ratio, time_op, write_json_report};
    use squonk_bench::{TOKENIZER_CASES, tokenize_sql};

    struct Timed {
        cursor_ns: f64,
        logos_ns: f64,
    }

    impl Timed {
        fn ratio(&self) -> f64 {
            ratio(self.cursor_ns, self.logos_ns)
        }
    }

    fn time_case(args: &CompareArgs, sql: &'static str) -> Timed {
        Timed {
            cursor_ns: time_op(args.warmup, args.iters, || tokenize_sql(sql) as u64),
            logos_ns: time_op(args.warmup, args.iters, || tokenize_logos(sql) as u64),
        }
    }

    pub fn run(args: &CompareArgs) {
        print!("{}", report_header());
        println!("# mode: WALL-CLOCK (mimalloc both sides); ns per tokenize");
        println!(
            "#   warmup {} iters {}  (trend-only)",
            args.warmup, args.iters
        );
        println!(
            "#   {:<22} {:>12} {:>12} {:>8}",
            "case", "cursor ns", "logos ns", "ratio"
        );
        let mut rows = Vec::new();
        for case in TOKENIZER_CASES {
            let t = time_case(args, case.sql);
            println!(
                "#   {:<22} {:>12.1} {:>12.1} {:>8.3}",
                case.name,
                t.cursor_ns,
                t.logos_ns,
                t.ratio()
            );
            rows.push((case.name, t));
        }

        if let Some(path) = &args.json {
            use std::fmt::Write as _;
            let mut out = String::new();
            let _ = writeln!(out, "{{");
            let _ = writeln!(out, "  \"comparison\": \"tokenizer_logos\",");
            let _ = writeln!(out, "  \"mode\": \"wall_clock\",");
            let _ = writeln!(out, "  \"unit\": \"ns_per_tokenize\",");
            let _ = writeln!(out, "  \"warmup\": {},", args.warmup);
            let _ = writeln!(out, "  \"iters\": {},", args.iters);
            let _ = writeln!(
                out,
                "  \"ratio\": \"cursor / logos (> 1.0 = the cursor is slower)\","
            );
            let _ = writeln!(out, "  \"cases\": [");
            for (i, (name, t)) in rows.iter().enumerate() {
                let comma = if i + 1 < rows.len() { "," } else { "" };
                let _ = writeln!(
                    out,
                    "    {{ \"name\": \"{}\", \"cursor_ns\": {:.3}, \"logos_ns\": {:.3}, \"ratio\": {:.3} }}{comma}",
                    json_escape(name),
                    t.cursor_ns,
                    t.logos_ns,
                    t.ratio()
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
    use crate::logos_ref::{report_header, tokenize_logos};
    use squonk_bench::compare::{CompareArgs, json_escape, ratio, write_json_report};
    use squonk_bench::{TOKENIZER_CASES, tokenize_sql};
    use std::hint::black_box;

    #[derive(Clone, Copy, Default)]
    struct Heap {
        total_bytes: u64,
        total_blocks: u64,
        peak_bytes: u64,
    }

    impl Heap {
        fn add(&mut self, other: Heap) {
            self.total_bytes += other.total_bytes;
            self.total_blocks += other.total_blocks;
            self.peak_bytes += other.peak_bytes;
        }
    }

    /// Profile one lex in a fresh testing profiler so the counts are exactly this call's
    /// allocations; `black_box` keeps the token buffer from being optimized away.
    fn sample(lex: impl FnOnce() -> usize) -> Heap {
        let _profiler = dhat::Profiler::builder().testing().build();
        black_box(lex());
        let stats = dhat::HeapStats::get();
        Heap {
            total_bytes: stats.total_bytes,
            total_blocks: stats.total_blocks,
            peak_bytes: stats.max_bytes as u64,
        }
    }

    fn print_row(label: &str, cursor: u64, logos: u64) {
        println!(
            "#   {label:<16} {cursor:>12} {logos:>12} {:>8.3}",
            ratio(cursor as f64, logos as f64)
        );
    }

    fn print_case(name: &str, cursor: Heap, logos: Heap) {
        println!("#");
        println!("# [{name}]");
        println!(
            "#   {:<16} {:>12} {:>12} {:>8}",
            "metric", "cursor", "logos", "ratio"
        );
        print_row("total_bytes", cursor.total_bytes, logos.total_bytes);
        print_row("total_blocks", cursor.total_blocks, logos.total_blocks);
        print_row("peak_bytes", cursor.peak_bytes, logos.peak_bytes);
    }

    pub fn run(args: &CompareArgs) {
        print!("{}", report_header());
        println!("# mode: HEAP (dhat::Alloc); deterministic total bytes/blocks + peak");

        let mut cursor_total = Heap::default();
        let mut logos_total = Heap::default();
        let mut rows = Vec::new();
        for case in TOKENIZER_CASES {
            let cursor = sample(|| tokenize_sql(case.sql));
            let logos = sample(|| tokenize_logos(case.sql));
            print_case(case.name, cursor, logos);
            cursor_total.add(cursor);
            logos_total.add(logos);
            rows.push((case.name, cursor, logos));
        }
        print_case("TOTAL (regular corpus)", cursor_total, logos_total);

        if let Some(path) = &args.json {
            use std::fmt::Write as _;
            let mut out = String::new();
            let _ = writeln!(out, "{{");
            let _ = writeln!(out, "  \"comparison\": \"tokenizer_logos\",");
            let _ = writeln!(out, "  \"mode\": \"heap\",");
            let _ = writeln!(
                out,
                "  \"ratio\": \"cursor / logos (block counts are the apples-to-apples signal)\","
            );
            let _ = writeln!(out, "  \"cases\": [");
            let emit = |out: &mut String, name: &str, c: &Heap, l: &Heap, comma: &str| {
                let _ = writeln!(
                    out,
                    "    {{ \"name\": \"{}\", \"cursor_bytes\": {}, \"cursor_blocks\": {}, \"cursor_peak\": {}, \"logos_bytes\": {}, \"logos_blocks\": {}, \"logos_peak\": {} }}{comma}",
                    json_escape(name),
                    c.total_bytes,
                    c.total_blocks,
                    c.peak_bytes,
                    l.total_bytes,
                    l.total_blocks,
                    l.peak_bytes
                );
            };
            for (name, c, l) in &rows {
                emit(&mut out, name, c, l, ",");
            }
            emit(&mut out, "TOTAL", &cursor_total, &logos_total, "");
            let _ = writeln!(out, "  ]");
            let _ = writeln!(out, "}}");
            write_json_report(path, &out);
        }
    }
}

fn main() {
    let args = CompareArgs::parse(2_000, 200_000);
    #[cfg(not(feature = "compare-heap"))]
    wall::run(&args);
    #[cfg(feature = "compare-heap")]
    heap::run(&args);
}
