// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Dedicated ours-vs-`sqlparser` adversarial-scaling HEAP comparison binary — the
//! relocation of the former `adversarial_heap` bench into the standardized comparison
//! CLI (`bench-comparison-dedicated-binaries`).
//!
//! The measurement is unchanged: the width generators, the doubling ladder, the Postgres
//! pair, and the dhat `sample` path all live in `benches/adversarial/mod.rs`, which this
//! example mounts and calls exactly as the bench did. HEAP only (dhat::Alloc, always) —
//! the deterministic per-width transient bytes/blocks for both parsers plus the per-family
//! linear-vs-super-linear verdict on OUR curve. The ours-only linear-scaling PIN stays a
//! `cargo nextest` gate in `tests/adversarial_scaling.rs`; the ours-only worst-case
//! `Ir` gate stays the Linux `adversarial_instr` gungraun bench.
//!
//! Run: cargo run -p squonk-bench --release --example compare_adversarial [-- --json out.json]

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

// `adversarial/mod.rs` imports `HeapSample`/`sample` from `crate::upstream` (it already
// links `sqlparser` directly, so mounting `upstream` alongside adds no dependency).
#[path = "../benches/adversarial/mod.rs"]
mod adversarial;
#[path = "../benches/upstream/mod.rs"]
mod upstream;

use adversarial::{FamilyScaling, measure_all, ours_bytes_series, top_width_bytes_ratio};
use squonk_bench::adversarial::{DEFAULT_SCALING_SLACK, superlinear_steps};
use squonk_bench::compare::{CompareArgs, json_escape, write_json_report};

fn print_header() {
    println!("# adversarial scaling: squonk vs sqlparser over deterministic width families");
    println!("#   mode     : HEAP (dhat::Alloc); deterministic transient bytes/blocks");
    println!("#   pair     : Postgres <-> PostgreSqlDialect (the widest both-accept superset)");
    println!("#   families : operator_chain, many_way_join, in_list, values_rows, cte_chain");
    println!("#   width    : doubling ladder; a LINEAR metric at most ~doubles per step (<2.5),");
    println!("#              a quadratic one ~quadruples (~4) — the verdict reads that growth");
    println!("#   transient: total bytes/blocks allocated to build the owned AST per parse");
    println!("#   ratio    : ours / theirs transient bytes (< 1 => we are lighter)");
    println!("#   numbers are deterministic; the failing gate is tests/adversarial_scaling.rs");
}

fn print_family(fs: &FamilyScaling) {
    println!("#");
    println!(
        "# [{}] ours accepts all widths: {}   theirs accepts all widths: {}",
        fs.name, fs.ours_accepts_all, fs.theirs_accepts_all
    );
    println!(
        "#   {:>6}  {:>14}  {:>14}  {:>14}  {:>14}",
        "width", "ours bytes", "ours blocks", "theirs bytes", "theirs blocks"
    );
    for (width, ours) in &fs.ours {
        let theirs = fs.theirs.iter().find(|(w, _)| w == width).map(|(_, h)| h);
        let (tb, tk) = theirs.map_or((0, 0), |h| (h.transient_bytes, h.transient_blocks));
        println!(
            "#   {width:>6}  {:>14}  {:>14}  {:>14}  {:>14}",
            ours.transient_bytes, ours.transient_blocks, tb, tk
        );
    }

    let bytes = ours_bytes_series(fs);
    let flagged = superlinear_steps(&bytes, DEFAULT_SCALING_SLACK);
    if flagged.is_empty() {
        println!("#   verdict: LINEAR (ours transient bytes; no step exceeds the slack)");
    } else {
        println!("#   verdict: SUPER-LINEAR (ours transient bytes):");
        for step in &flagged {
            println!(
                "#     {} -> {}: metric x{:.2} for width x{:.2} (allowed x{:.2})",
                step.from_width,
                step.to_width,
                step.metric_factor(),
                step.width_factor(),
                step.allowed_factor(DEFAULT_SCALING_SLACK),
            );
        }
    }
    if let Some((width, ratio)) = top_width_bytes_ratio(fs) {
        println!("#   ours/theirs transient bytes at width {width}: {ratio:.3}");
    }
}

fn json_report(families: &[FamilyScaling]) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"comparison\": \"adversarial\",");
    let _ = writeln!(out, "  \"mode\": \"heap\",");
    let _ = writeln!(out, "  \"pair\": \"Postgres <-> PostgreSqlDialect\",");
    let _ = writeln!(
        out,
        "  \"ratio\": \"ours / theirs transient bytes (< 1.0 = we are lighter)\","
    );
    let _ = writeln!(out, "  \"families\": [");
    for (i, fs) in families.iter().enumerate() {
        let linear = superlinear_steps(&ours_bytes_series(fs), DEFAULT_SCALING_SLACK).is_empty();
        let _ = writeln!(out, "    {{");
        let _ = writeln!(out, "      \"name\": \"{}\",", json_escape(fs.name));
        let _ = writeln!(out, "      \"ours_linear\": {linear},");
        let _ = writeln!(out, "      \"widths\": [");
        for (j, (width, ours)) in fs.ours.iter().enumerate() {
            let theirs = fs.theirs.iter().find(|(w, _)| w == width).map(|(_, h)| h);
            let (tb, tk) = theirs.map_or((0, 0), |h| (h.transient_bytes, h.transient_blocks));
            let comma = if j + 1 < fs.ours.len() { "," } else { "" };
            let _ = writeln!(
                out,
                "        {{ \"width\": {width}, \"ours_bytes\": {}, \"ours_blocks\": {}, \"theirs_bytes\": {tb}, \"theirs_blocks\": {tk} }}{comma}",
                ours.transient_bytes, ours.transient_blocks
            );
        }
        let _ = writeln!(out, "      ]");
        let comma = if i + 1 < families.len() { "," } else { "" };
        let _ = writeln!(out, "    }}{comma}");
    }
    let _ = writeln!(out, "  ]");
    let _ = writeln!(out, "}}");
    out
}

fn main() {
    // Heap counts are deterministic — warmup/iters are inapplicable (accepted + ignored).
    let args = CompareArgs::parse(0, 0);
    print_header();
    let families = measure_all();
    for fs in &families {
        print_family(fs);
    }
    if let Some(path) = &args.json {
        write_json_report(path, &json_report(&families));
    }
}
