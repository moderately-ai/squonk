// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared CLI + timing helpers for the ours-vs-external comparison examples
//! (`examples/compare_upstream`, `compare_libpg`, `compare_adversarial`,
//! `compare_tokenizer_logos`, `compare_interner`, `compare_keyword_lookup`).
//!
//! Each comparison binary owns its measurement (the corpus, the both-accept subset
//! rule, the metrics) via the `benches/*/mod.rs` module it mounts; this module owns
//! only the parts that must be IDENTICAL across all of them: the flag surface
//! (`--warmup`, `--iters`, `--corpus`, `--json`, `--update-baseline`), the
//! warm-up-then-time loop shapes, and a byte-stable JSON string escaper. Keeping the
//! knobs here — rather than re-hand-rolled per binary — is what makes "one
//! standardized CLI" a single definition instead of six drifting ones.
//!
//! WHY compile-time allocator selection (not a runtime `--heap` flag). The wall-clock
//! comparisons run under `mimalloc` (the realistic fast allocator, both sides — the
//! fairness invariant) and the heap comparisons under `dhat::Alloc` (the allocation
//! hook). A process has exactly one `#[global_allocator]`, so a binary that measures
//! both cannot switch at runtime: the family binaries pick allocator + mode at compile
//! time via the `compare-heap` feature, exactly as `alloc_probe` selects its allocator.
//! This CLI is the same either way; only which timers a binary calls changes.

use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

/// The parsed common flags. Per-example defaults for `warmup`/`iters` are supplied by
/// the caller because the natural iteration budget differs by measurement shape (a
/// per-statement parse vs a whole-corpus batch vs a heap profile that ignores both).
#[derive(Clone, Debug)]
pub struct CompareArgs {
    /// Untimed warm-up iterations run before every timed measurement (lazy statics,
    /// interner tables, allocator arenas land here so they never inflate a timing).
    pub warmup: u64,
    /// Timed iterations per measured case. Ignored by the deterministic heap binaries.
    pub iters: u64,
    /// Optional corpus selector; the meaning is per-binary (e.g. `curated` / `complex`
    /// / `all` for the corpus-driven comparisons). `None` measures the binary's default.
    pub corpus: Option<String>,
    /// Optional path to write the machine-readable JSON report to (in addition to the
    /// human-readable summary always printed to stdout).
    pub json: Option<PathBuf>,
    /// Rewrite the checked-in baseline snapshot (`compare_upstream` heap mode only;
    /// the ratio gate reads that file). Also honoured via `SQUONK_UPDATE_BASELINE`.
    pub update_baseline: bool,
}

impl CompareArgs {
    /// Parse the process arguments with the given per-example defaults.
    pub fn parse(default_warmup: u64, default_iters: u64) -> Self {
        Self::from_args(std::env::args().skip(1), default_warmup, default_iters)
    }

    /// Parse an explicit argument sequence (the testable core of [`CompareArgs::parse`]).
    /// Accepts both `--flag value` and `--flag=value`. Unknown flags are a hard error,
    /// so a typo fails loudly rather than silently measuring the default.
    pub fn from_args(
        args: impl IntoIterator<Item = String>,
        default_warmup: u64,
        default_iters: u64,
    ) -> Self {
        let mut out = CompareArgs {
            warmup: default_warmup,
            iters: default_iters,
            corpus: None,
            json: None,
            update_baseline: std::env::var_os("SQUONK_UPDATE_BASELINE").is_some(),
        };
        let mut it = args.into_iter().peekable();
        while let Some(arg) = it.next() {
            let (flag, inline) = match arg.split_once('=') {
                Some((f, v)) => (f.to_owned(), Some(v.to_owned())),
                None => (arg, None),
            };
            let mut value = |name: &str| {
                inline
                    .clone()
                    .or_else(|| it.next())
                    .unwrap_or_else(|| fail(&format!("{name} requires a value")))
            };
            match flag.as_str() {
                "--warmup" => out.warmup = parse_u64(&value("--warmup"), "--warmup"),
                "--iters" => out.iters = parse_u64(&value("--iters"), "--iters"),
                "--corpus" => out.corpus = Some(value("--corpus")),
                "--json" => out.json = Some(PathBuf::from(value("--json"))),
                "--update-baseline" => out.update_baseline = true,
                "-h" | "--help" => {
                    print!("{USAGE}");
                    std::process::exit(0);
                }
                other => fail(&format!("unrecognized argument `{other}`\n\n{USAGE}")),
            }
        }
        out
    }
}

const USAGE: &str = "\
Standardized comparison-binary flags:
  --warmup <N>         untimed warm-up iterations before each timing (default per-binary)
  --iters <N>          timed iterations per case (default per-binary; heap binaries ignore)
  --corpus <NAME>      corpus selector (per-binary: e.g. curated | complex | all)
  --json <PATH>        also write the machine-readable JSON report to PATH
  --update-baseline    rewrite the checked-in baseline snapshot (compare_upstream heap mode)
  -h, --help           print this help
";

fn parse_u64(s: &str, flag: &str) -> u64 {
    s.parse()
        .unwrap_or_else(|_| fail(&format!("{flag} expects a non-negative integer, got `{s}`")))
}

fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(2)
}

// ---------------------------------------------------------------------------
// Timing loop shapes (explicit warm-up, so `--warmup` is honoured verbatim)
// ---------------------------------------------------------------------------

/// Warm up, then time `f` over `iters` calls; return nanoseconds per call. The sink is
/// accumulated + `black_box`'d so neither the call nor the loop is optimized away. This
/// is the plain per-operation timer (parse one statement, sweep one word list, …).
pub fn time_op(warmup: u64, iters: u64, mut f: impl FnMut() -> u64) -> f64 {
    for _ in 0..warmup {
        black_box(f());
    }
    let start = Instant::now();
    let mut sink = 0u64;
    for _ in 0..iters {
        sink = sink.wrapping_add(black_box(f()));
    }
    let elapsed = start.elapsed().as_nanos() as f64;
    black_box(sink);
    elapsed / iters as f64
}

/// Time a value-BUILDING operation with its drop EXCLUDED from the measurement: each
/// built value is stashed and freed in bulk after the clock stops. Mirrors criterion's
/// `iter_with_large_drop` — used where freeing (N boxes vs a few arena buffers) differs
/// by design and would otherwise pollute the build-cost comparison (the interner intern
/// arm). Returns nanoseconds per build.
pub fn time_build<T>(warmup: u64, iters: u64, mut build: impl FnMut() -> T) -> f64 {
    for _ in 0..warmup {
        black_box(build());
    }
    let mut held: Vec<T> = Vec::with_capacity(iters as usize);
    let start = Instant::now();
    for _ in 0..iters {
        held.push(build());
    }
    let elapsed = start.elapsed().as_nanos() as f64;
    black_box(&held);
    drop(held);
    elapsed / iters as f64
}

/// Time a `setup -> transform` operation with BOTH the per-iteration setup and the
/// output drop excluded from the measurement (only `transform` is timed). Mirrors
/// criterion's `iter_batched(setup, transform, LargeInput)` — used for the interner
/// freeze arm, where each freeze needs a fresh populated interner (untimed) and the
/// frozen resolver is dropped untimed. Returns nanoseconds per transform.
pub fn time_transform<S, T>(
    warmup: u64,
    iters: u64,
    mut setup: impl FnMut() -> S,
    mut transform: impl FnMut(S) -> T,
) -> f64 {
    for _ in 0..warmup {
        let s = setup();
        black_box(transform(black_box(s)));
    }
    // Build every input up front (untimed), so the timed region is pure `transform`.
    let inputs: Vec<S> = (0..iters).map(|_| setup()).collect();
    let mut held: Vec<T> = Vec::with_capacity(iters as usize);
    let start = Instant::now();
    for s in inputs {
        held.push(transform(s));
    }
    let elapsed = start.elapsed().as_nanos() as f64;
    black_box(&held);
    drop(held);
    elapsed / iters as f64
}

/// `ours / theirs`, guarding a zero denominator (an un-measured or empty series) with
/// `NaN` rather than a panic — the same convention the shared measurement modules use.
pub fn ratio(ours: f64, theirs: f64) -> f64 {
    if theirs == 0.0 {
        f64::NAN
    } else {
        ours / theirs
    }
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

/// Escape a string for embedding in the hand-rolled JSON reports (the corpus/case
/// names are ASCII SQL identifiers today, but the escaper is correct for the JSON
/// control set so a future name with a quote or backslash cannot corrupt the report).
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Write `json` to `path` (used by every binary's `--json` handling), reporting the
/// destination on stdout so a run log records where the machine-readable copy landed.
pub fn write_json_report(path: &std::path::Path, json: &str) {
    std::fs::write(path, json)
        .unwrap_or_else(|e| fail(&format!("write JSON report {}: {e}", path.display())));
    println!("# wrote JSON report: {}", path.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flags_in_both_forms_and_applies_defaults() {
        let a = CompareArgs::from_args(
            [
                "--warmup",
                "5",
                "--iters=9",
                "--corpus",
                "complex",
                "--json",
                "/tmp/r.json",
            ]
            .into_iter()
            .map(String::from),
            1,
            2,
        );
        assert_eq!(a.warmup, 5);
        assert_eq!(a.iters, 9);
        assert_eq!(a.corpus.as_deref(), Some("complex"));
        assert_eq!(a.json.as_deref(), Some(std::path::Path::new("/tmp/r.json")));

        let d = CompareArgs::from_args(std::iter::empty(), 7, 11);
        assert_eq!((d.warmup, d.iters), (7, 11));
        assert!(d.corpus.is_none() && d.json.is_none());
    }

    #[test]
    fn json_escape_covers_the_control_set() {
        assert_eq!(json_escape("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(json_escape("x\ny"), "x\\ny");
        assert_eq!(json_escape("\u{1}"), "\\u0001");
    }
}
