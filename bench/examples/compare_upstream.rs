// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Dedicated ours-vs-upstream (`sqlparser`) comparison binary — the relocation of the
//! former `upstream_compare` (wall-clock) and `upstream_heap` (dhat) benches into one
//! standardized CLI (`bench-comparison-dedicated-binaries`).
//!
//! The measurement is unchanged: the corpus (`corpus()` + `complex_datasets()`), the
//! both-accept subset rule, and the metrics all live in `benches/upstream/mod.rs`, which
//! this example mounts and calls exactly as the two benches did — so the numbers agree
//! with the old benches within noise (a relocation, not a re-measurement).
//!
//! Two modes, selected at COMPILE time because a process has one `#[global_allocator]`:
//!
//! - default (mimalloc): the WALL-CLOCK comparison — per-pair aggregate `ours / theirs`
//!   ns over the both-accept curated subset and the complex corpus, both sides under the
//!   realistic fast allocator (the fairness invariant). Trend-only, never gated.
//! - `--features compare-heap` (dhat::Alloc): the deterministic HEAP comparison — the
//!   transient/retained/peak `ours / theirs` tables — and it (re)writes the byte-stable
//!   `upstream-baseline.json` the ratio gate (`tests/upstream_gate.rs`) reads.
//!
//! Run:
//!   cargo run -p squonk-bench --release --example compare_upstream               # wall-clock
//!   cargo run -p squonk-bench --release --example compare_upstream \
//!     --features compare-heap -- --update-baseline                                   # heap + baseline

#[cfg(not(feature = "compare-heap"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "compare-heap")]
#[global_allocator]
static GLOBAL: dhat::Alloc = dhat::Alloc;

#[path = "../benches/upstream/mod.rs"]
mod upstream;

use squonk_bench::compare::CompareArgs;

// ---------------------------------------------------------------------------
// Wall-clock mode (mimalloc) — the relocated `upstream_compare` measurement
// ---------------------------------------------------------------------------

#[cfg(not(feature = "compare-heap"))]
mod wall {
    use crate::upstream::{
        PAIRS, Pair, complex_both_accept, parse_ours, parse_theirs, report_header, subset,
    };
    use squonk_bench::compare::{CompareArgs, json_escape, ratio, time_op, write_json_report};

    fn parse_batch_ours(pair: Pair, batch: &[&str]) -> u64 {
        batch.iter().map(|sql| parse_ours(pair, sql)).sum::<usize>() as u64
    }

    fn parse_batch_theirs(pair: Pair, batch: &[&str]) -> u64 {
        batch
            .iter()
            .map(|sql| parse_theirs(pair, sql))
            .sum::<usize>() as u64
    }

    /// One measured `ours`/`theirs` batch: total ns for the whole batch on each side and
    /// the ns/parse-normalized ratio (identical to a per-parse ratio for equal counts).
    struct Timed {
        measured: usize,
        ours_ns: f64,
        theirs_ns: f64,
    }

    impl Timed {
        fn ratio(&self) -> f64 {
            ratio(self.ours_ns, self.theirs_ns)
        }
    }

    fn time_batch(args: &CompareArgs, pair: Pair, batch: &[&str]) -> Timed {
        let ours = time_op(args.warmup, args.iters, || parse_batch_ours(pair, batch));
        let theirs = time_op(args.warmup, args.iters, || parse_batch_theirs(pair, batch));
        Timed {
            measured: batch.len(),
            ours_ns: ours,
            theirs_ns: theirs,
        }
    }

    /// Which corpora to time, from `--corpus` (default: both).
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

    struct PairResult {
        pair: Pair,
        curated: Option<Timed>,
        complex_datasets: Vec<(&'static str, Timed)>,
        complex_total: Option<Timed>,
    }

    pub fn run(args: &CompareArgs) {
        print!("{}", report_header());
        println!("# mode: WALL-CLOCK (mimalloc both sides); ns per whole-batch parse pass");
        println!(
            "#   warmup {} iters {}  (trend-only — the deterministic ratios are heap/instr)",
            args.warmup, args.iters
        );
        let (do_curated, do_complex) = wants(args.corpus.as_deref());

        let mut results = Vec::new();
        for pair in PAIRS {
            let curated = do_curated.then(|| {
                let s = subset(pair);
                let batch: Vec<&str> = s.included.iter().map(|c| c.sql).collect();
                time_batch(args, pair, &batch)
            });

            let mut complex_datasets = Vec::new();
            let mut complex_total = None;
            if do_complex {
                let mut all: Vec<&str> = Vec::new();
                for (name, batch) in complex_both_accept(pair) {
                    if batch.is_empty() {
                        continue;
                    }
                    complex_datasets.push((name, time_batch(args, pair, &batch)));
                    all.extend(batch);
                }
                if !all.is_empty() {
                    complex_total = Some(time_batch(args, pair, &all));
                }
            }

            results.push(PairResult {
                pair,
                curated,
                complex_datasets,
                complex_total,
            });
        }

        print_summary(&results);
        if let Some(path) = &args.json {
            write_json_report(path, &json_report(args, &results));
        }
    }

    fn print_row(label: &str, t: &Timed) {
        println!(
            "#   {label:<22} measured {:>3}  ours {:>12.1} ns  theirs {:>12.1} ns  ratio {:>7.3}",
            t.measured,
            t.ours_ns,
            t.theirs_ns,
            t.ratio()
        );
    }

    fn print_summary(results: &[PairResult]) {
        for r in results {
            println!("#");
            println!("# [{}]", r.pair.label());
            if let Some(t) = &r.curated {
                print_row("curated (aggregate)", t);
            }
            for (name, t) in &r.complex_datasets {
                print_row(&format!("complex/{name}"), t);
            }
            if let Some(t) = &r.complex_total {
                print_row("complex (aggregate)", t);
            }
        }
    }

    fn json_timed(out: &mut String, indent: &str, key: &str, t: &Timed, comma: bool) {
        use std::fmt::Write as _;
        let _ = writeln!(
            out,
            "{indent}\"{key}\": {{ \"measured\": {}, \"ours_ns\": {:.3}, \"theirs_ns\": {:.3}, \"ratio\": {:.3} }}{}",
            t.measured,
            t.ours_ns,
            t.theirs_ns,
            t.ratio(),
            if comma { "," } else { "" }
        );
    }

    fn json_report(args: &CompareArgs, results: &[PairResult]) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "{{");
        let _ = writeln!(out, "  \"comparison\": \"upstream\",");
        let _ = writeln!(out, "  \"mode\": \"wall_clock\",");
        let _ = writeln!(
            out,
            "  \"upstream_version\": \"{}\",",
            crate::upstream::UPSTREAM_VERSION
        );
        let _ = writeln!(out, "  \"unit\": \"ns_per_batch_pass\",");
        let _ = writeln!(out, "  \"warmup\": {},", args.warmup);
        let _ = writeln!(out, "  \"iters\": {},", args.iters);
        let _ = writeln!(
            out,
            "  \"ratio\": \"ours / theirs (< 1.0 = we are faster)\","
        );
        let _ = writeln!(out, "  \"pairs\": [");
        for (i, r) in results.iter().enumerate() {
            let _ = writeln!(out, "    {{");
            let _ = writeln!(out, "      \"pair\": \"{}\",", json_escape(r.pair.label()));
            if let Some(t) = &r.curated {
                json_timed(&mut out, "      ", "curated", t, true);
            }
            let _ = writeln!(out, "      \"complex_datasets\": [");
            for (j, (name, t)) in r.complex_datasets.iter().enumerate() {
                let comma = if j + 1 < r.complex_datasets.len() {
                    ","
                } else {
                    ""
                };
                let _ = writeln!(
                    out,
                    "        {{ \"name\": \"{}\", \"measured\": {}, \"ours_ns\": {:.3}, \"theirs_ns\": {:.3}, \"ratio\": {:.3} }}{comma}",
                    json_escape(name),
                    t.measured,
                    t.ours_ns,
                    t.theirs_ns,
                    t.ratio()
                );
            }
            let _ = writeln!(out, "      ],");
            match &r.complex_total {
                Some(t) => json_timed(&mut out, "      ", "complex_total", t, false),
                None => {
                    let _ = writeln!(out, "      \"complex_total\": null");
                }
            }
            let comma = if i + 1 < results.len() { "," } else { "" };
            let _ = writeln!(out, "    }}{comma}");
        }
        let _ = writeln!(out, "  ]");
        let _ = writeln!(out, "}}");
        out
    }
}

// ---------------------------------------------------------------------------
// Heap mode (dhat) — the relocated `upstream_heap` measurement + baseline writer
// ---------------------------------------------------------------------------

#[cfg(feature = "compare-heap")]
mod heap {
    use crate::upstream::{
        DatasetCoverage, PAIRS, Pair, Subset, Totals, baseline_json, baseline_path,
        complex_measured, measure, measure_complex, ratio, report_complex_coverage, report_header,
        report_subset, rows,
    };
    use squonk_bench::compare::{CompareArgs, write_json_report};

    fn print_table(title: &str, measured: usize, ours: &Totals, theirs: &Totals) {
        println!("#");
        println!("# {title} over {measured} measured statements");
        println!(
            "#   {:<18} {:>12} {:>12} {:>8}",
            "metric", "ours", "theirs", "ratio"
        );
        for (label, o, t) in rows(ours, theirs) {
            println!("#   {label:<18} {o:>12} {t:>12} {:>8.3}", ratio(o, t));
        }
    }

    fn print_curated(s: &Subset, ours: &Totals, theirs: &Totals) {
        print_table(
            &format!("[{}] curated heap", s.pair.label()),
            s.included.len(),
            ours,
            theirs,
        );
    }

    fn print_complex(pair: Pair, coverage: &[DatasetCoverage], ours: &Totals, theirs: &Totals) {
        print!("{}", report_complex_coverage(pair, coverage));
        print_table(
            &format!("[{}] complex-corpus heap", pair.label()),
            complex_measured(coverage),
            ours,
            theirs,
        );
    }

    pub fn run(args: &CompareArgs) {
        print!("{}", report_header());
        println!("# mode: HEAP (dhat::Alloc); deterministic transient/retained/peak counts");

        let mut curated = Vec::new();
        let mut complex = Vec::new();
        for pair in PAIRS {
            let (s, ours, theirs) = measure(pair);
            print!("{}", report_subset(&s));
            print_curated(&s, &ours, &theirs);
            curated.push((s, ours, theirs));
        }
        for pair in PAIRS {
            let (coverage, ours, theirs) = measure_complex(pair);
            print_complex(pair, &coverage, &ours, &theirs);
            complex.push((pair, coverage, ours, theirs));
        }

        let json = baseline_json(&curated, &complex);
        // The baseline snapshot is the byte-stable report the ratio gate reads. Rewrite
        // it on `--update-baseline` (or the historical `SQUONK_UPDATE_BASELINE`);
        // `--json PATH` writes the same document to an out-of-tree copy independently.
        let baseline = baseline_path();
        if args.update_baseline {
            write_json_report(&baseline, &json);
        } else {
            println!("#");
            println!(
                "# baseline snapshot at {} (run with --update-baseline to refresh)",
                baseline.display()
            );
        }
        if let Some(path) = &args.json {
            write_json_report(path, &json);
        }
    }
}

fn main() {
    // Heap counts are deterministic (warmup/iters unused there); the wall-clock defaults
    // are a modest trend budget — override with --warmup/--iters.
    let args = CompareArgs::parse(200, 2_000);
    #[cfg(not(feature = "compare-heap"))]
    wall::run(&args);
    #[cfg(feature = "compare-heap")]
    heap::run(&args);
}
