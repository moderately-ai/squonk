// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Dedicated ours-vs-`libpg_query` COMPUTE comparison binary — the relocation of the
//! former `libpg_compare` bench into the standardized comparison CLI
//! (`bench-comparison-dedicated-binaries`).
//!
//! The measurement is unchanged: the corpus, the both-accept subset rule, and the four
//! series (ours + libpg's `theirs_full` / `theirs_tree_build` / `theirs_parse_only`
//! bracket) all live in `benches/libpg/mod.rs`, which this example mounts and calls
//! exactly as the bench did. WALL-CLOCK only, under mimalloc on our side (libpg allocates
//! its tree in C, untouched) — memory is deliberately omitted (C palloc is invisible to
//! dhat; see `benches/libpg/mod.rs`), so there is no heap mode. The deterministic
//! instruction comparison stays the Linux-only `libpg_instr` gungraun bench.
//!
//! Run: cargo run -p squonk-bench --release --example compare_libpg [-- --corpus complex]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// `upstream` supplies the corpus + ours-side adapters `libpg` reuses (`crate::upstream`);
// its `sqlparser`/theirs paths go unused here (that module's own `allow(dead_code)`).
#[path = "../benches/libpg/mod.rs"]
mod libpg;
#[path = "../benches/upstream/mod.rs"]
mod upstream;

use libpg::{
    libpg_complex_both_accept, libpg_subset, parse_libpg, parse_libpg_parse_only,
    parse_libpg_tree_build, parse_ours_pg, report_header, report_subset,
};
use squonk_bench::compare::{CompareArgs, json_escape, ratio, time_op, write_json_report};

fn sum_ns(args: &CompareArgs, batch: &[&str], f: fn(&str) -> usize) -> f64 {
    time_op(args.warmup, args.iters, || {
        batch.iter().map(|sql| f(sql)).sum::<usize>() as u64
    })
}

/// One measured batch: ns for our side and each libpg series, plus the fair owned-tree
/// ratio (ours / theirs_tree_build) and the bracketing ratios.
struct Timed {
    measured: usize,
    ours_ns: f64,
    theirs_full_ns: f64,
    theirs_tree_build_ns: f64,
    theirs_parse_only_ns: f64,
}

impl Timed {
    fn measure(args: &CompareArgs, batch: &[&str]) -> Self {
        Timed {
            measured: batch.len(),
            ours_ns: sum_ns(args, batch, parse_ours_pg),
            theirs_full_ns: sum_ns(args, batch, parse_libpg),
            theirs_tree_build_ns: sum_ns(args, batch, parse_libpg_tree_build),
            theirs_parse_only_ns: sum_ns(args, batch, parse_libpg_parse_only),
        }
    }

    fn fair_ratio(&self) -> f64 {
        ratio(self.ours_ns, self.theirs_tree_build_ns)
    }
}

fn wants(corpus: Option<&str>) -> (bool, bool) {
    match corpus {
        None | Some("all") => (true, true),
        Some("curated") => (true, false),
        Some("complex") => (false, true),
        Some(other) => {
            eprintln!("--corpus must be one of: curated | complex | all (got `{other}`)");
            std::process::exit(2);
        }
    }
}

fn print_row(label: &str, t: &Timed) {
    println!(
        "#   {label:<24} measured {:>3}  ours {:>11.1}  tree_build {:>11.1}  parse_only {:>11.1}  full {:>11.1}",
        t.measured, t.ours_ns, t.theirs_tree_build_ns, t.theirs_parse_only_ns, t.theirs_full_ns
    );
    println!(
        "#   {:<24} ours/tree_build (FAIR) {:>7.3}   ours/parse_only {:>7.3}   ours/full {:>7.3}",
        "",
        t.fair_ratio(),
        ratio(t.ours_ns, t.theirs_parse_only_ns),
        ratio(t.ours_ns, t.theirs_full_ns),
    );
}

fn json_timed(out: &mut String, indent: &str, key: &str, t: &Timed, comma: bool) {
    use std::fmt::Write as _;
    let _ = writeln!(
        out,
        "{indent}\"{key}\": {{ \"measured\": {}, \"ours_ns\": {:.3}, \"theirs_tree_build_ns\": {:.3}, \"theirs_parse_only_ns\": {:.3}, \"theirs_full_ns\": {:.3}, \"ours_over_tree_build\": {:.3} }}{}",
        t.measured,
        t.ours_ns,
        t.theirs_tree_build_ns,
        t.theirs_parse_only_ns,
        t.theirs_full_ns,
        t.fair_ratio(),
        if comma { "," } else { "" }
    );
}

fn main() {
    let args = CompareArgs::parse(200, 2_000);
    print!("{}", report_header());
    println!("# mode: WALL-CLOCK (ours under mimalloc; libpg in C palloc); ns per batch pass");
    println!(
        "#   warmup {} iters {}  (trend-only)",
        args.warmup, args.iters
    );
    let (do_curated, do_complex) = wants(args.corpus.as_deref());

    let s = libpg_subset();
    let curated = do_curated.then(|| {
        print!("{}", report_subset(&s));
        let batch: Vec<&str> = s.included.iter().map(|c| c.sql).collect();
        Timed::measure(&args, &batch)
    });

    let mut complex_datasets = Vec::new();
    let mut complex_total = None;
    if do_complex {
        let mut all: Vec<&str> = Vec::new();
        for (name, batch) in libpg_complex_both_accept() {
            if batch.is_empty() {
                continue;
            }
            complex_datasets.push((name, Timed::measure(&args, &batch)));
            all.extend(batch);
        }
        if !all.is_empty() {
            complex_total = Some(Timed::measure(&args, &all));
        }
    }

    if let Some(t) = &curated {
        println!("#");
        print_row("curated (aggregate)", t);
    }
    for (name, t) in &complex_datasets {
        print_row(&format!("complex/{name}"), t);
    }
    if let Some(t) = &complex_total {
        print_row("complex (aggregate)", t);
    }

    if let Some(path) = &args.json {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "{{");
        let _ = writeln!(out, "  \"comparison\": \"libpg\",");
        let _ = writeln!(out, "  \"mode\": \"wall_clock\",");
        let _ = writeln!(out, "  \"unit\": \"ns_per_batch_pass\",");
        let _ = writeln!(out, "  \"warmup\": {},", args.warmup);
        let _ = writeln!(out, "  \"iters\": {},", args.iters);
        let _ = writeln!(
            out,
            "  \"ratio\": \"ours / theirs_tree_build is the FAIR owned-tree ratio (< 1.0 = we are faster)\","
        );
        if let Some(t) = &curated {
            json_timed(&mut out, "  ", "curated", t, true);
        }
        let _ = writeln!(out, "  \"complex_datasets\": [");
        for (j, (name, t)) in complex_datasets.iter().enumerate() {
            let comma = if j + 1 < complex_datasets.len() {
                ","
            } else {
                ""
            };
            let _ = writeln!(
                out,
                "    {{ \"name\": \"{}\", \"measured\": {}, \"ours_ns\": {:.3}, \"theirs_tree_build_ns\": {:.3}, \"ours_over_tree_build\": {:.3} }}{comma}",
                json_escape(name),
                t.measured,
                t.ours_ns,
                t.theirs_tree_build_ns,
                t.fair_ratio()
            );
        }
        let _ = writeln!(out, "  ],");
        match &complex_total {
            Some(t) => json_timed(&mut out, "  ", "complex_total", t, false),
            None => {
                let _ = writeln!(out, "  \"complex_total\": null");
            }
        }
        let _ = writeln!(out, "}}");
        write_json_report(path, &out);
    }
}
