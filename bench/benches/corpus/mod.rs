// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Corpus-scale parser measurement: aggregate cost of parsing realistic batches
//! of statements drawn from the vendored conformance corpora, so a *scale*
//! regression (a per-node allocation creeping up across hundreds of statements)
//! trips a local gate, not only the hand-picked micro fixtures in
//! `squonk_bench`.
//!
//! Three sibling targets share this one module so they agree on exactly one
//! corpus set, one deterministic subset-selection rule, and one measurement path:
//!
//! - `corpus_heap.rs`         — dhat: aggregate transient + retained + peak heap
//!   over the parseable subset (the real scale signal, runs on macOS + Linux).
//! - `tests/corpus_allocations.rs` — pins those aggregates (and the subset size)
//!   so a corpus-scale regression fails `cargo nextest run` locally.
//! - `corpus_instr.rs`        — gungraun/callgrind aggregate `Ir` over the batch
//!   (Linux/Valgrind CI signal).
//!
//! Each target consumes a different slice of this module, so unused-per-target
//! helpers are expected; the module-level `allow(dead_code)` keeps `-D warnings`
//! green without scattering attributes (same convention as `upstream/mod.rs`).
#![allow(dead_code)]

use squonk::dialect::{Ansi, Postgres};
use squonk::{StockParsed, parse_with};
use std::hint::black_box;

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------
//
// The three already-vendored, SPDX/provenance-clean conformance corpora. They are
// `include_str!`'d straight from the conformance crate's `corpus/` tree (the same
// files `conformance/src/corpus_*.rs` and `pg.rs` include), so the bench measures
// the EXACT bytes conformance already pins — no second copy to drift. The path is
// relative to THIS file (`bench/benches/corpus/mod.rs` -> repo root -> conformance),
// and `include_str!` resolves from a file's own directory even when the module is
// `#[path]`-mounted from `tests/`, so the heap bench, the alloc gate, and the instr
// bench all read the same fixtures.
//
// Touching only these read-only includes keeps this work inside `bench/**`: it does
// not modify the conformance crate (which is a separately-owned, concurrently-edited
// work-stream), so there is no shared-file write contention.

/// sqlglot transpiler-identity corpus: 955 single-line statements/expressions,
/// byte-for-byte upstream. Most lines exceed our surface — that is the point; the
/// subset selector keeps only what parses.
const SQLGLOT_IDENTITY: &str = include_str!("../../../conformance/corpus/sqlglot/identity.sql");

/// sqllogictest extracted corpus: one statement per line, no blanks/comments.
const SQLLOGICTEST_STATEMENTS: &str =
    include_str!("../../../conformance/corpus/sqllogictest/statements.sql");

/// PostgreSQL-regression supported subset: a `--`-comment SPDX header followed by
/// `;`-terminated, possibly multi-line statements (a different shape from the two
/// line-per-statement corpora — see [`Shape`]).
const PG_REGRESS_SUPPORTED: &str =
    include_str!("../../../conformance/corpus/postgres/regress-supported.sql");

/// How a corpus file delimits its statements.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    /// One statement per line, no blank/comment lines (sqlglot, sqllogictest).
    LinePerStatement,
    /// `;`-terminated statements after a leading `--`/blank comment header
    /// (PostgreSQL regress). No semicolons appear inside string literals in this
    /// curated file, so a plain `;` split is exact; the subset-size pin in
    /// `tests/corpus_allocations.rs` catches the fixture ever drifting from that.
    SemicolonDelimited,
}

/// One vendored corpus file plus how to cut it into statements.
struct CorpusFile {
    name: &'static str,
    text: &'static str,
    shape: Shape,
}

/// The corpora, in a fixed order. Iteration order here is the order candidates and
/// the measured subset are built in, so every derived number is deterministic and
/// git-diffable (ADR-0016/0017).
const CORPORA: &[CorpusFile] = &[
    CorpusFile {
        name: "sqlglot_identity",
        text: SQLGLOT_IDENTITY,
        shape: Shape::LinePerStatement,
    },
    CorpusFile {
        name: "sqllogictest_statements",
        text: SQLLOGICTEST_STATEMENTS,
        shape: Shape::LinePerStatement,
    },
    CorpusFile {
        name: "postgres_regress_supported",
        text: PG_REGRESS_SUPPORTED,
        shape: Shape::SemicolonDelimited,
    },
];

/// Drop a leading run of blank lines and `--` line-comments from a `;`-delimited
/// chunk, returning the trimmed remainder (empty if the chunk is only
/// whitespace/comments, e.g. the SPDX header's chunk before the first statement,
/// or the empty tail after the final `;`). Sub-slicing preserves the input's
/// lifetime, so candidates stay borrows of the `'static` corpus text — the
/// candidate list itself allocates nothing the parse measurement could pick up.
fn strip_leading_comment_lines(chunk: &str) -> &str {
    let mut rest = chunk.trim();
    while rest.starts_with("--") {
        let cut = rest.find('\n').map_or(rest.len(), |i| i + 1);
        rest = rest[cut..].trim_start();
    }
    rest.trim_end()
}

/// Every candidate statement of one corpus file, in source order. A candidate is
/// just a SQL slice; whether it is *measured* is decided later by [`subset`].
fn file_candidates(file: &CorpusFile) -> Vec<&'static str> {
    match file.shape {
        Shape::LinePerStatement => file
            .text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect(),
        Shape::SemicolonDelimited => file
            .text
            .split(';')
            .map(strip_leading_comment_lines)
            .filter(|sql| !sql.is_empty())
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Dialect presets
// ---------------------------------------------------------------------------
//
// Measured under both shipped presets, mirroring the dual-dialect coverage the
// micro benches (`ast.rs`, `perf.rs`) already track: Postgres accepts a (super)set
// of what Ansi does, so the two aggregates move together and a regression shows up
// in both.

/// Which of our shipped presets a batch is parsed under.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    Ansi,
    Postgres,
}

pub const PRESETS: [Preset; 2] = [Preset::Ansi, Preset::Postgres];

impl Preset {
    pub fn label(self) -> &'static str {
        match self {
            Preset::Ansi => "Ansi",
            Preset::Postgres => "Postgres",
        }
    }
}

/// `true` iff our parser accepts `sql` under `preset` — the subset-selection
/// predicate ("parse each candidate; keep those our parser accepts").
pub fn parses(preset: Preset, sql: &str) -> bool {
    match preset {
        Preset::Ansi => parse_with(sql, squonk::ParseConfig::new(Ansi)).is_ok(),
        Preset::Postgres => parse_with(sql, squonk::ParseConfig::new(Postgres)).is_ok(),
    }
}

/// Parse to our owned AST root, kept for the retained-heap measurement. Only ever
/// called on statements [`subset`] already proved parse, so the `expect` cannot
/// fire on the selected subset.
pub fn parse_owned(preset: Preset, sql: &str) -> StockParsed {
    match preset {
        Preset::Ansi => {
            parse_with(sql, squonk::ParseConfig::new(Ansi)).expect("subset statement parses (Ansi)")
        }
        Preset::Postgres => parse_with(sql, squonk::ParseConfig::new(Postgres))
            .expect("subset statement parses (Postgres)"),
    }
}

// ---------------------------------------------------------------------------
// Subset selection
// ---------------------------------------------------------------------------

/// Per-corpus candidate vs measured counts, for the coverage report and the
/// anti-vanishing size pin.
#[derive(Clone, Copy, Debug)]
pub struct CorpusCoverage {
    pub corpus: &'static str,
    pub candidates: usize,
    pub included: usize,
}

/// The deterministically-selected measured subset for one preset: the statements
/// our parser accepts, in corpus-then-source order, plus the per-corpus coverage.
#[derive(Clone, Debug)]
pub struct Subset {
    pub preset: Preset,
    pub included: Vec<&'static str>,
    pub total_candidates: usize,
    pub coverage: Vec<CorpusCoverage>,
}

impl Subset {
    /// Fraction of all candidates our parser accepts under this preset.
    pub fn coverage_pct(&self) -> f64 {
        if self.total_candidates == 0 {
            0.0
        } else {
            100.0 * self.included.len() as f64 / self.total_candidates as f64
        }
    }
}

/// Compute the measured subset for `preset`: every candidate our parser accepts,
/// kept in deterministic corpus-then-source order so the aggregate numbers are
/// reproducible and git-diffable.
pub fn subset(preset: Preset) -> Subset {
    let mut included = Vec::new();
    let mut coverage = Vec::new();
    let mut total_candidates = 0;
    for file in CORPORA {
        let candidates = file_candidates(file);
        let candidate_count = candidates.len();
        total_candidates += candidate_count;
        let before = included.len();
        for sql in candidates {
            if parses(preset, sql) {
                included.push(sql);
            }
        }
        coverage.push(CorpusCoverage {
            corpus: file.name,
            candidates: candidate_count,
            included: included.len() - before,
        });
    }
    Subset {
        preset,
        included,
        total_candidates,
        coverage,
    }
}

/// The measured-subset SQL for `preset`, for the gungraun setup (which builds this
/// out of the measured window so subset selection never inflates the `Ir` count).
pub fn included_sql(preset: Preset) -> Vec<&'static str> {
    subset(preset).included
}

/// Parse every statement of an already-selected subset under `preset`, summing the
/// statement counts so the result is `black_box`-able without retaining the ASTs.
pub fn parse_subset(preset: Preset, subset: &[&str]) -> usize {
    subset
        .iter()
        .map(|sql| parse_owned(preset, sql).statements().len())
        .sum()
}

// ---------------------------------------------------------------------------
// Heap measurement (dhat)
// ---------------------------------------------------------------------------
//
// A self-contained mirror of `upstream/mod.rs`'s `sample` / `Totals` harness. It
// is copied rather than imported because that module is welded to the ours-vs-
// upstream `Pair` comparison (and pulls in the `sqlparser` yardstick); corpus scale
// measures only OUR parser, so reusing the shape without the comparison apparatus
// keeps these benches from linking the upstream crate. Both consumers
// (`corpus_heap.rs`, `tests/corpus_allocations.rs`) install `dhat::Alloc` as the
// global allocator, so `sample` reads real, deterministic counts.

/// One parse's heap profile: transient (cumulative, including freed temporaries)
/// kept separate from retained (live while the owned AST is held) so the two —
/// which mean different things, since our retained footprint deliberately keeps the
/// source `Arc<str>` + interner alive per ADR-0002/0005 — are never conflated.
#[derive(Clone, Copy, Default)]
pub struct HeapSample {
    pub transient_bytes: u64,
    pub transient_blocks: u64,
    pub retained_bytes: u64,
    pub retained_blocks: u64,
    pub peak_bytes: u64,
}

/// Corpus-wide aggregate over the measured subset.
#[derive(Clone, Copy, Default)]
pub struct Totals {
    pub transient_bytes: u64,
    pub transient_blocks: u64,
    pub retained_bytes: u64,
    pub retained_blocks: u64,
    pub peak_bytes: u64,
}

impl Totals {
    pub fn add(&mut self, s: HeapSample) {
        self.transient_bytes += s.transient_bytes;
        self.transient_blocks += s.transient_blocks;
        self.retained_bytes += s.retained_bytes;
        self.retained_blocks += s.retained_blocks;
        self.peak_bytes += s.peak_bytes;
    }
}

/// Profile a single parse: open a fresh testing profiler so the counts are exactly
/// this one statement's allocations, read the live (retained) stats while the owned
/// AST is held, then drop it and read the cumulative (transient) stats. Generic over
/// the owned-AST type so the value is held directly on the stack — no boxing, no
/// measurement artifact.
pub fn sample<T>(parse: impl FnOnce() -> T) -> HeapSample {
    let _profiler = dhat::Profiler::builder().testing().build();

    let ast = parse();
    black_box(&ast);
    let live = dhat::HeapStats::get();
    let retained_bytes = live.curr_bytes as u64;
    let retained_blocks = live.curr_blocks as u64;
    let peak_bytes = live.max_bytes as u64;
    drop(ast);

    let done = dhat::HeapStats::get();
    HeapSample {
        transient_bytes: done.total_bytes,
        transient_blocks: done.total_blocks,
        retained_bytes,
        retained_blocks,
        peak_bytes,
    }
}

/// Measure the whole parseable subset for `preset` as one batch: sum the
/// per-statement [`sample`]s into corpus-wide [`Totals`]. Per-statement isolation
/// (a fresh profiler each parse) keeps the harness's own bookkeeping — the subset
/// `Vec`, the loop — out of the measured numbers, exactly as `upstream/mod.rs`'s
/// `measure` does.
pub fn measure(preset: Preset) -> (Subset, Totals) {
    let s = subset(preset);
    let mut totals = Totals::default();
    for &sql in &s.included {
        totals.add(sample(|| parse_owned(preset, sql)));
    }
    (s, totals)
}

/// The five aggregate metric rows, in stable order, for the printed table and the
/// pinned gate. The order is load-bearing: the bench table and the alloc-pin test
/// both rely on it.
pub fn rows(totals: &Totals) -> [(&'static str, u64); 5] {
    [
        ("transient_bytes", totals.transient_bytes),
        ("transient_blocks", totals.transient_blocks),
        ("retained_bytes", totals.retained_bytes),
        ("retained_blocks", totals.retained_blocks),
        ("peak_bytes", totals.peak_bytes),
    ]
}
