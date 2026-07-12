// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared, fairness-load-bearing scaffolding for the apples-to-apples comparison
//! against upstream `sqlparser` (apache/datafusion-sqlparser-rs) — the
//! 184-method-god-trait design ADR-0011 supersedes. Three sibling consumers use it so
//! they agree on exactly one corpus, one subset-selection rule, one set of parse
//! adapters, and one caveat text:
//!
//! - `examples/compare_upstream.rs` — the wall-clock (mimalloc) and dhat-heap
//!   comparison, mode selected by the `compare-heap` feature; the heap mode writes the
//!   byte-stable `upstream-baseline.json` this module's ratio gate reads back.
//! - `upstream_instr.rs`            — gungraun/callgrind instruction counts (Linux CI),
//!   kept a `[[bench]]` because gungraun's `main!` macro is the harness.
//! - `tests/upstream_gate.rs`       — the ratio gate: re-measures via this module and
//!   compares against the committed baseline.
//!
//! Each consumer uses a different subset of this module, so unused-per-consumer helpers
//! are expected; the module-level `allow(dead_code)` keeps `-D warnings` green without
//! scattering attributes.
#![allow(dead_code)]

use sqlparser::dialect::{GenericDialect, PostgreSqlDialect};
use sqlparser::parser::Parser as UpstreamParser;
use squonk::dialect::{Ansi, Postgres};
use squonk::{StockParsed, parse_with};
use squonk_bench::BenchCase;
use std::fmt::Write as _;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The exact upstream revision these numbers are taken against. Pinned in
/// `bench/Cargo.toml` as `sqlparser = "=0.62.0"`; mirrored here so every harness
/// prints the version it actually measured. Keep the two in lockstep on bumps.
pub const UPSTREAM_VERSION: &str = "0.62.0";

// ---------------------------------------------------------------------------
// Corpus
// ---------------------------------------------------------------------------
//
// The comparison candidate set is the PostgreSQL-regression supported subset,
// `include_str!`'d straight from the conformance corpus tree — the SAME bytes
// `conformance/src/pg.rs` drives its accept/reject + structural-parity oracle over
// (`assert_structural_parity(PG_REGRESS_SUPPORTED_SQL)`). Reading the file directly is a
// *blessed include path* — the seam the rest of this bench already uses (the complex
// corpus below and `benches/corpus/mod.rs`) — so the comparison widens automatically as
// the conformance corpus grows, instead of drifting from a hand-maintained inline copy.
// The boundary is the file path, stable now that the conformance corpus layout has
// settled; conformance exposes no public (non-`cfg(test)`) corpus accessor to depend on,
// so the include is the stable seam and adds no crate dependency.
//
// The set is deliberately *broader* than what both parsers accept today; the subset
// selector below measures only the intersection and logs the rest, so the excluded list
// visibly shrinks as our surface grows.

/// The PostgreSQL-regression supported subset. The path is relative to THIS file and
/// `include_str!` resolves from the file's own directory even when the module is
/// `#[path]`-mounted from `tests/` (matching the complex-corpus includes below), so the
/// heap bench and the ratio gate read identical bytes.
const PG_REGRESS_SUPPORTED: &str =
    include_str!("../../../conformance/corpus/postgres/regress-supported.sql");

// Named single statements the gungraun instruction bench (`upstream_instr.rs`) pins as
// its fixed fixtures: that bench feeds statements to a compile-time
// `#[benches::stmts(...)]` macro, which cannot consume the runtime-built `corpus()`
// below, so its representative set stays a handful of named `const`s. Four alias the
// shared `squonk_bench` constants (so the instruction tracker overlaps the fixtures
// `perf.rs` / `ast.rs` already track); four are PostgreSQL-regression forms kept verbatim
// from the corpus. They are NOT the comparison candidate set — `corpus()` is. Only
// `upstream_instr` reads them and it is Linux-gated, so they are unused on other targets;
// the module-level `allow(dead_code)` keeps that from tripping `-D warnings` (an
// `allow(dead_code)` covers a `const`, but not the `unused_imports` a `pub use` would
// raise — hence aliases, not re-exports).
pub const SIMPLE_SELECT: &str = squonk_bench::SIMPLE_SELECT;
pub const SET_SELECT: &str = squonk_bench::SET_SELECT;
pub const DEEP_NESTED_SELECT: &str = squonk_bench::DEEP_NESTED_SELECT;
pub const MULTI_STATEMENT_SELECTS: &str = squonk_bench::MULTI_STATEMENT_SELECTS;

pub const PG_CTE: &str = "WITH q1(x,y) AS (SELECT 1,2) SELECT * FROM q1, q1 AS q2";
pub const PG_UNION_ALL: &str = "SELECT 1 AS two UNION ALL SELECT 2";
pub const PG_CROSS_JOIN: &str = "select * from j1_tbl cross join j2_tbl";
pub const PG_JOIN_ON: &str = "select * from j1_tbl join j2_tbl on (j1_tbl.i <= j2_tbl.k)";

/// Cut the PostgreSQL-regress corpus into its statements, in source order. The file is
/// a leading `--`/blank SPDX-header block followed by `;`-terminated statements. The
/// header is dropped WHOLESALE before the `;`-split — not per-`;`-chunk — because the
/// header prose itself contains a semicolon ("… identifiers exactly; unquoted …"), which
/// a naive split would glue onto the first statement. After the header no `--` comment
/// and no in-string `;` appears in this curated file, so a plain `;` split of the
/// remainder is exact; the `corpus_size` pin in the baseline catches the fixture ever
/// drifting from that. Statements stay borrows of the `'static` include, so the candidate
/// list allocates nothing the parse measurement could pick up.
fn pg_regress_statements() -> impl Iterator<Item = &'static str> {
    // Byte offset of the first non-header line (the header is the contiguous leading run
    // of blank / `--`-comment lines). `split_inclusive` keeps each line's terminator, so
    // the running sum is an exact byte offset into the `'static` text.
    let mut offset = 0;
    for line in PG_REGRESS_SUPPORTED.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if !trimmed.is_empty() && !trimmed.starts_with("--") {
            break;
        }
        offset += line.len();
    }
    PG_REGRESS_SUPPORTED[offset..]
        .split(';')
        .map(str::trim)
        .filter(|stmt| !stmt.is_empty())
}

/// The full candidate corpus, before subset selection: every statement of the
/// PostgreSQL-regression supported subset, positionally named. The names surface only in
/// the coverage/exclusion log, so they are synthesized here rather than carried in the
/// corpus file. Built once and cached: `include_str!` + split is a runtime step (unlike
/// the former `const` array), so the per-statement name strings are leaked exactly once
/// per process — a bench harness, not a long-running service, and the leak is outside
/// every `sample` window.
///
/// `pub` so the sibling libpg_query comparison (`benches/libpg/mod.rs`) measures the
/// IDENTICAL candidate set — same bytes, same statement cut — rather than re-deriving
/// it and risking drift.
pub fn corpus() -> &'static [BenchCase] {
    static CORPUS: OnceLock<Vec<BenchCase>> = OnceLock::new();
    CORPUS.get_or_init(|| {
        pg_regress_statements()
            .enumerate()
            .map(|(i, sql)| BenchCase {
                name: Box::leak(format!("pg_regress_{:02}", i + 1).into_boxed_str()),
                sql,
            })
            .collect()
    })
}

// ---------------------------------------------------------------------------
// Complex corpus (TPC-H / TPC-DS + CTE/subquery optimizer fixtures)
// ---------------------------------------------------------------------------
//
// The realistic-complex counterpart to the curated `corpus()` candidate set above: the input
// queries of sqlglot's TPC-H / TPC-DS suites plus four CTE/subquery optimizer
// fixtures, vendored (SPDX + provenance) under `conformance/corpus/sqlglot-complex/`.
// `include_str!`'d straight from that tree — the SAME bytes the conformance loader
// (`conformance/src/corpus_complex.rs`) classifies and pins — so the bench measures
// exactly what conformance reports, with no second copy to drift. The path is
// relative to THIS file (`bench/benches/upstream/mod.rs` -> repo root -> conformance),
// and `include_str!` resolves from a file's own directory even when the module is
// `#[path]`-mounted from `tests/`, so the heap bench and the ratio gate read the
// identical fixtures.

const COMPLEX_TPC_H: &str = include_str!("../../../conformance/corpus/sqlglot-complex/tpc-h.sql");
const COMPLEX_TPC_DS: &str = include_str!("../../../conformance/corpus/sqlglot-complex/tpc-ds.sql");
const COMPLEX_MERGE_SUBQUERIES: &str =
    include_str!("../../../conformance/corpus/sqlglot-complex/merge_subqueries.sql");
const COMPLEX_UNNEST_SUBQUERIES: &str =
    include_str!("../../../conformance/corpus/sqlglot-complex/unnest_subqueries.sql");
const COMPLEX_PUSHDOWN_CTE: &str =
    include_str!("../../../conformance/corpus/sqlglot-complex/pushdown_cte_alias_columns.sql");
const COMPLEX_ELIMINATE_CTES: &str =
    include_str!("../../../conformance/corpus/sqlglot-complex/eliminate_ctes.sql");

/// Provenance echoed into the baseline snapshot so the pinned complex numbers name
/// the upstream commit they were taken against. Kept in lockstep with
/// `conformance/corpus/sqlglot-complex/PROVENANCE.toml`.
pub const COMPLEX_PROVENANCE: &str =
    "sqlglot tests/fixtures/optimizer @ fd6d4d61c25e7918118fc22c5579098a86a58e10";

/// One vendored complex dataset: its name and its statements. The statements are
/// slices of the `'static` `include_str!` text, so the candidate list itself
/// allocates nothing the parse measurement could pick up.
pub struct Dataset {
    pub name: &'static str,
    pub cases: Vec<&'static str>,
}

/// Drop a leading run of blank lines and `--` line-comments from a `;`-split chunk
/// (the SPDX/banner header precedes the first statement). Kept identical to the
/// conformance loader (`corpus_complex.rs`) and `corpus/mod.rs` so all three cut the
/// vendored files into the same statements; the per-dataset count pins below catch
/// any drift between them.
fn strip_leading_comment_lines(chunk: &str) -> &str {
    let mut rest = chunk.trim();
    while rest.starts_with("--") {
        let cut = rest.find('\n').map_or(rest.len(), |i| i + 1);
        rest = rest[cut..].trim_start();
    }
    rest.trim_end()
}

/// Cut one vendored dataset file into its statements (no semicolon appears inside a
/// statement — enforced at vendoring time and pinned by the candidate counts — so a
/// plain `;` split is exact).
fn dataset(name: &'static str, text: &'static str) -> Dataset {
    let cases = text
        .split(';')
        .map(strip_leading_comment_lines)
        .filter(|sql| !sql.is_empty())
        .collect();
    Dataset { name, cases }
}

/// The complex datasets, in a fixed order so every derived number is deterministic
/// and git-diffable (ADR-0016/0017).
pub fn complex_datasets() -> Vec<Dataset> {
    vec![
        dataset("tpc-h", COMPLEX_TPC_H),
        dataset("tpc-ds", COMPLEX_TPC_DS),
        dataset("merge_subqueries", COMPLEX_MERGE_SUBQUERIES),
        dataset("unnest_subqueries", COMPLEX_UNNEST_SUBQUERIES),
        dataset("pushdown_cte_alias_columns", COMPLEX_PUSHDOWN_CTE),
        dataset("eliminate_ctes", COMPLEX_ELIMINATE_CTES),
    ]
}

// ---------------------------------------------------------------------------
// Dialect mapping (approximate — see caveats)
// ---------------------------------------------------------------------------

/// A mapped (ours ↔ theirs) dialect pair. The mapping is the load-bearing
/// approximation: there is no exact correspondence between our dialects and
/// theirs, so every row of output names the pair it used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pair {
    /// `squonk::dialect::Ansi` vs `sqlparser::dialect::GenericDialect`.
    AnsiGeneric,
    /// `squonk::dialect::Postgres` vs `sqlparser::dialect::PostgreSqlDialect`.
    PostgresPostgres,
}

pub const PAIRS: [Pair; 2] = [Pair::AnsiGeneric, Pair::PostgresPostgres];

impl Pair {
    /// Human-readable mapping, e.g. `Ansi ↔ GenericDialect`.
    pub fn label(self) -> &'static str {
        match self {
            Pair::AnsiGeneric => "Ansi ↔ GenericDialect",
            Pair::PostgresPostgres => "Postgres ↔ PostgreSqlDialect",
        }
    }

    /// Benchmark-id / filename-safe slug.
    pub fn slug(self) -> &'static str {
        match self {
            Pair::AnsiGeneric => "ansi_generic",
            Pair::PostgresPostgres => "postgres_postgresql",
        }
    }
}

// ---------------------------------------------------------------------------
// Parse adapters
// ---------------------------------------------------------------------------
//
// Each side is driven through its *default entry point* only — ours
// `parse_with(sql, dialect)`, theirs `Parser::parse_sql(&dialect, sql)` — with no
// rendering and no extra passes. Neither side is crippled; the feature set is the
// shipped default (theirs therefore includes `recursive-protection`, see caveats).

/// `true` iff our parser accepts `sql` under `pair`'s dialect.
pub fn ours_parses(pair: Pair, sql: &str) -> bool {
    match pair {
        Pair::AnsiGeneric => parse_with(sql, Ansi).is_ok(),
        Pair::PostgresPostgres => parse_with(sql, Postgres).is_ok(),
    }
}

/// `true` iff upstream accepts `sql` under `pair`'s dialect.
pub fn theirs_parses(pair: Pair, sql: &str) -> bool {
    match pair {
        Pair::AnsiGeneric => UpstreamParser::parse_sql(&GenericDialect, sql).is_ok(),
        Pair::PostgresPostgres => UpstreamParser::parse_sql(&PostgreSqlDialect {}, sql).is_ok(),
    }
}

/// Parse to our owned AST and return the statement count (cheap, allocation-free
/// summary for the compute benches; mirrors `squonk_bench::parse_ansi_sql`).
pub fn parse_ours(pair: Pair, sql: &str) -> usize {
    parse_ours_owned(pair, sql).statements().len()
}

/// Parse to upstream's owned AST and return the statement count.
pub fn parse_theirs(pair: Pair, sql: &str) -> usize {
    parse_theirs_owned(pair, sql).len()
}

/// Parse to our owned AST root, kept for retained-heap measurement.
pub fn parse_ours_owned(pair: Pair, sql: &str) -> StockParsed {
    match pair {
        Pair::AnsiGeneric => parse_with(sql, Ansi).expect("subset statement parses (ours)"),
        Pair::PostgresPostgres => {
            parse_with(sql, Postgres).expect("subset statement parses (ours)")
        }
    }
}

/// Parse to upstream's owned AST (`Vec<Statement>`), kept for retained-heap
/// measurement.
pub fn parse_theirs_owned(pair: Pair, sql: &str) -> Vec<sqlparser::ast::Statement> {
    match pair {
        Pair::AnsiGeneric => UpstreamParser::parse_sql(&GenericDialect, sql)
            .expect("subset statement parses (theirs)"),
        Pair::PostgresPostgres => UpstreamParser::parse_sql(&PostgreSqlDialect {}, sql)
            .expect("subset statement parses (theirs)"),
    }
}

// ---------------------------------------------------------------------------
// Subset selection
// ---------------------------------------------------------------------------

/// Why a candidate statement is not measured under a given pair.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExclusionReason {
    /// Our parser rejects it; upstream accepts it (the set that shrinks as our
    /// surface grows).
    OursRejects,
    /// Upstream rejects it; our parser accepts it.
    TheirsRejects,
    /// Neither parser accepts it under this pair.
    BothReject,
}

impl ExclusionReason {
    pub fn describe(self) -> &'static str {
        match self {
            ExclusionReason::OursRejects => "ours rejects (upstream accepts)",
            ExclusionReason::TheirsRejects => "upstream rejects (ours accepts)",
            ExclusionReason::BothReject => "both reject",
        }
    }
}

/// A candidate excluded from the measured set, with the reason.
#[derive(Clone, Copy, Debug)]
pub struct Excluded {
    pub case: BenchCase,
    pub reason: ExclusionReason,
}

/// The measured intersection for one pair plus the logged exclusions.
#[derive(Clone, Debug)]
pub struct Subset {
    pub pair: Pair,
    pub included: Vec<BenchCase>,
    pub excluded: Vec<Excluded>,
}

impl Subset {
    /// Fraction of the candidate corpus both parsers accept, as a percentage.
    pub fn coverage_pct(&self) -> f64 {
        100.0 * self.included.len() as f64 / corpus().len() as f64
    }
}

/// Compute the measured subset for `pair`: the statements *both* parsers accept,
/// with everything else logged and attributed to the side that rejected it. This
/// is the fairness gate — only the intersection is ever measured.
pub fn subset(pair: Pair) -> Subset {
    let mut included = Vec::new();
    let mut excluded = Vec::new();
    for &case in corpus() {
        match (ours_parses(pair, case.sql), theirs_parses(pair, case.sql)) {
            (true, true) => included.push(case),
            (false, true) => excluded.push(Excluded {
                case,
                reason: ExclusionReason::OursRejects,
            }),
            (true, false) => excluded.push(Excluded {
                case,
                reason: ExclusionReason::TheirsRejects,
            }),
            (false, false) => excluded.push(Excluded {
                case,
                reason: ExclusionReason::BothReject,
            }),
        }
    }
    Subset {
        pair,
        included,
        excluded,
    }
}

// ---------------------------------------------------------------------------
// Self-describing report blocks
// ---------------------------------------------------------------------------

/// The fixed context + fairness caveats every harness prints, so a raw bench log
/// is interpretable on its own. The caveats are stated, never silent (ADR-0011 /
/// ADR-0002 / ADR-0005): the ratios surface a deliberate design cost, not a bug.
pub fn report_header() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# upstream comparison: squonk vs sqlparser");
    let _ = writeln!(
        out,
        "#   upstream pinned version : sqlparser {UPSTREAM_VERSION}"
    );
    let _ = writeln!(
        out,
        "#   upstream features       : default (std + recursive-protection)"
    );
    let _ = writeln!(out, "#   dialect mapping (approx):");
    for pair in PAIRS {
        let _ = writeln!(out, "#     - {}", pair.label());
    }
    let _ = writeln!(
        out,
        "#   ratio = ours / theirs  (> 1.0 ⇒ we are heavier/slower)"
    );
    let _ = writeln!(out, "#");
    let _ = writeln!(out, "# fairness caveats (read the ratios through these):");
    let _ = writeln!(
        out,
        "#   1. Our AST carries byte spans, interned Symbols, NodeId and Meta;"
    );
    let _ = writeln!(
        out,
        "#      theirs does not. The ratios surface that deliberate design cost."
    );
    let _ = writeln!(
        out,
        "#   2. RETAINED heap is not the same bytes on both sides: ours keeps the"
    );
    let _ = writeln!(
        out,
        "#      source Arc<str> + the interner alive for zero-copy spans"
    );
    let _ = writeln!(
        out,
        "#      (ADR-0002/0005); theirs inlines owned Strings and retains no"
    );
    let _ = writeln!(
        out,
        "#      source/interner. Read it as 'footprint of a live parse result'."
    );
    let _ = writeln!(
        out,
        "#   3. TRANSIENT heap counts every allocation made to build the result"
    );
    let _ = writeln!(
        out,
        "#      (ours: source Arc + interner growth + nodes; theirs: Strings +"
    );
    let _ = writeln!(out, "#      nodes + Vec).");
    let _ = writeln!(
        out,
        "#   4. The dialect mapping is approximate; each row names its pair."
    );
    let _ = writeln!(
        out,
        "#   5. Upstream's default `recursive-protection` adds a stack-overflow"
    );
    let _ = writeln!(
        out,
        "#      guard on the hot recursive paths that our parser has no equivalent"
    );
    let _ = writeln!(
        out,
        "#      for, so their compute carries a small constant overhead — this"
    );
    let _ = writeln!(
        out,
        "#      biases ratios slightly in our favour (a conservative yardstick)."
    );
    let _ = writeln!(
        out,
        "#   6. Only the subset BOTH parsers accept is measured; coverage logged."
    );
    out
}

/// The per-pair coverage line plus the attributed exclusion list.
pub fn report_subset(s: &Subset) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "# [{}] coverage {:.1}% ({}/{} statements measured)",
        s.pair.label(),
        s.coverage_pct(),
        s.included.len(),
        corpus().len(),
    );
    if s.excluded.is_empty() {
        let _ = writeln!(out, "#   excluded: none");
    } else {
        let _ = writeln!(out, "#   excluded:");
        for ex in &s.excluded {
            let _ = writeln!(out, "#     - {:<28} {}", ex.case.name, ex.reason.describe());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Heap measurement (shared by the heap bench and the ratio gate)
// ---------------------------------------------------------------------------
//
// `upstream_heap.rs` calls these to print the table and (re)write the baseline
// snapshot; the gate (`tests/upstream_gate.rs`) calls the SAME functions to
// re-measure for the regression check, so there is exactly one measurement path,
// not two (ADR-0016: one perf-gate mechanism). Both consumers install
// `dhat::Alloc` as the global allocator, so `sample` reads real, deterministic
// counts; benches that include this module but never call `sample`
// (`upstream_compare`, `upstream_instr`) leave the code unused under the
// module-level `allow(dead_code)`.

/// One statement's heap profile, split into transient (cumulative) and retained
/// (live-while-held) so the two are never conflated. `Debug` is needed by
/// `adversarial::FamilyScaling` (which imports this type rather than redefining it
/// — see `adversarial/mod.rs`) for its own `#[derive(Debug)]`.
#[derive(Clone, Copy, Default, Debug)]
pub struct HeapSample {
    pub transient_bytes: u64,
    pub transient_blocks: u64,
    pub retained_bytes: u64,
    pub retained_blocks: u64,
    pub peak_bytes: u64,
}

/// Corpus-wide totals for one side of one pair.
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
/// AST is held, then drop it and read the cumulative (transient) stats.
///
/// Generic over the owned-AST type so the concrete value is held directly on the
/// stack — no boxing, so no measurement artifact is added to either side.
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

pub fn sample_ours(pair: Pair, sql: &str) -> HeapSample {
    sample(|| parse_ours_owned(pair, sql))
}

pub fn sample_theirs(pair: Pair, sql: &str) -> HeapSample {
    sample(|| parse_theirs_owned(pair, sql))
}

/// Measure every statement in `pair`'s subset, returning the per-side totals.
pub fn measure(pair: Pair) -> (Subset, Totals, Totals) {
    let s = subset(pair);
    let mut ours = Totals::default();
    let mut theirs = Totals::default();
    for case in &s.included {
        ours.add(sample_ours(pair, case.sql));
        theirs.add(sample_theirs(pair, case.sql));
    }
    (s, ours, theirs)
}

pub fn ratio(ours: u64, theirs: u64) -> f64 {
    if theirs == 0 {
        f64::NAN
    } else {
        ours as f64 / theirs as f64
    }
}

/// The five metric rows, in stable order, as `(label, ours, theirs)`. The order is
/// load-bearing: the JSON snapshot, the printed table, and the gate all iterate it,
/// and the gate zips current against baseline by position.
pub fn rows(ours: &Totals, theirs: &Totals) -> [(&'static str, u64, u64); 5] {
    [
        (
            "transient_bytes",
            ours.transient_bytes,
            theirs.transient_bytes,
        ),
        (
            "transient_blocks",
            ours.transient_blocks,
            theirs.transient_blocks,
        ),
        ("retained_bytes", ours.retained_bytes, theirs.retained_bytes),
        (
            "retained_blocks",
            ours.retained_blocks,
            theirs.retained_blocks,
        ),
        ("peak_bytes", ours.peak_bytes, theirs.peak_bytes),
    ]
}

// ---------------------------------------------------------------------------
// Complex-corpus coverage + measurement
// ---------------------------------------------------------------------------
//
// The complex corpus is measured per dataset for COVERAGE (how many of each
// dataset's candidates ours / theirs / both accept) and in aggregate for HEAP: the
// pinned heap ratios are summed over the union of every dataset's both-accept subset
// under one pair, exactly as `measure` sums over the curated set. Per-dataset
// coverage is reported and pinned so a query silently dropping out of either
// parser's surface trips the gate; the aggregate ratios ride the same `Totals` /
// `sample` / `rows` path as the curated comparison.

/// Per-dataset acceptance coverage under one pair: how many of the dataset's
/// candidates OUR parser accepts, how many UPSTREAM accepts, and how many BOTH
/// accept (the subset actually measured). `both_accept <= min(ours, theirs)`.
#[derive(Clone, Debug)]
pub struct DatasetCoverage {
    pub name: &'static str,
    pub candidates: usize,
    pub ours_accepts: usize,
    pub theirs_accepts: usize,
    pub both_accept: usize,
}

/// Measure the whole complex corpus under `pair`: per-dataset coverage plus the
/// aggregate heap totals over the union of every dataset's both-accept subset. The
/// coverage probes (`ours_parses` / `theirs_parses`) run OUTSIDE the `sample`
/// windows, so only the both-accept parses land in the returned `Totals` — the
/// extra probe allocations never pollute the measurement.
pub fn measure_complex(pair: Pair) -> (Vec<DatasetCoverage>, Totals, Totals) {
    let mut coverage = Vec::new();
    let mut ours = Totals::default();
    let mut theirs = Totals::default();
    for ds in complex_datasets() {
        let mut cov = DatasetCoverage {
            name: ds.name,
            candidates: ds.cases.len(),
            ours_accepts: 0,
            theirs_accepts: 0,
            both_accept: 0,
        };
        for &sql in &ds.cases {
            let o = ours_parses(pair, sql);
            let t = theirs_parses(pair, sql);
            cov.ours_accepts += usize::from(o);
            cov.theirs_accepts += usize::from(t);
            if o && t {
                cov.both_accept += 1;
                ours.add(sample_ours(pair, sql));
                theirs.add(sample_theirs(pair, sql));
            }
        }
        coverage.push(cov);
    }
    (coverage, ours, theirs)
}

/// Total both-accept statements measured across all datasets (the aggregate the
/// pinned heap ratios are taken over).
pub fn complex_measured(coverage: &[DatasetCoverage]) -> usize {
    coverage.iter().map(|c| c.both_accept).sum()
}

/// The complex corpus's both-accept subset under `pair`, grouped per dataset. Used
/// by the wall-clock bench to time each dataset's batch on both sides (the time
/// ratio is read off the adjacent ours/theirs rows, like the curated compare).
pub fn complex_both_accept(pair: Pair) -> Vec<(&'static str, Vec<&'static str>)> {
    complex_datasets()
        .into_iter()
        .map(|ds| {
            let kept = ds
                .cases
                .into_iter()
                .filter(|sql| ours_parses(pair, sql) && theirs_parses(pair, sql))
                .collect();
            (ds.name, kept)
        })
        .collect()
}

/// The per-dataset coverage block for one pair: each dataset's ours / theirs / both
/// acceptance over its candidate count, so a raw bench log shows how much of TPC-H
/// (22) / TPC-DS (99) / each fixture each parser reaches.
pub fn report_complex_coverage(pair: Pair, coverage: &[DatasetCoverage]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "# [{}] complex-corpus coverage (ours / theirs / both accept, per dataset):",
        pair.label()
    );
    for c in coverage {
        let _ = writeln!(
            out,
            "#   {:<28} {:>3} candidates  ours {:>3}  theirs {:>3}  both {:>3}",
            c.name, c.candidates, c.ours_accepts, c.theirs_accepts, c.both_accept,
        );
    }
    let candidates: usize = coverage.iter().map(|c| c.candidates).sum();
    let ours: usize = coverage.iter().map(|c| c.ours_accepts).sum();
    let theirs: usize = coverage.iter().map(|c| c.theirs_accepts).sum();
    let _ = writeln!(
        out,
        "#   {:<28} {:>3} candidates  ours {:>3}  theirs {:>3}  both {:>3}",
        "TOTAL",
        candidates,
        ours,
        theirs,
        complex_measured(coverage),
    );
    out
}

// ---------------------------------------------------------------------------
// Baseline snapshot (write + read)
// ---------------------------------------------------------------------------

/// Absolute path of the checked-in baseline snapshot.
pub fn baseline_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("upstream-baseline.json")
}

/// Emit the five `"metric": { ours, theirs, ratio }` lines at `indent`, the last
/// without a trailing comma. Shared by the curated and complex sections so both
/// render the metric block identically (the gate reads either back the same way).
fn write_metrics(out: &mut String, indent: &str, ours: &Totals, theirs: &Totals) {
    let metric_rows = rows(ours, theirs);
    for (j, (label, o, t)) in metric_rows.iter().enumerate() {
        let comma = if j + 1 < metric_rows.len() { "," } else { "" };
        let _ = writeln!(
            out,
            "{indent}\"{label}\": {{ \"ours\": {o}, \"theirs\": {t}, \"ratio\": {:.3} }}{comma}",
            ratio(*o, *t)
        );
    }
}

/// Hand-format a deterministic JSON snapshot (no timestamps) so a re-run on
/// unchanged code yields a byte-identical file — any `git diff` is a real change.
/// Serialization stays hand-rolled (not serde) so the exact byte layout, and thus
/// the diff, is fully controlled here; the gate reads the file back with
/// `serde_json`, which tolerates that fixed shape robustly.
///
/// `curated` is the `corpus()` comparison; `complex` is the TPC-H/DS +
/// optimizer-fixture corpus, one entry per pair (per-dataset coverage plus the
/// aggregate both-accept heap totals).
pub fn baseline_json(
    curated: &[(Subset, Totals, Totals)],
    complex: &[(Pair, Vec<DatasetCoverage>, Totals, Totals)],
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"upstream_version\": \"{UPSTREAM_VERSION}\",");
    let _ = writeln!(out, "  \"corpus_size\": {},", corpus().len());
    let _ = writeln!(
        out,
        "  \"ratio\": \"ours / theirs (> 1.0 = we are heavier)\","
    );
    let _ = writeln!(out, "  \"pairs\": [");
    for (i, (s, ours, theirs)) in curated.iter().enumerate() {
        let _ = writeln!(out, "    {{");
        let _ = writeln!(out, "      \"pair\": \"{}\",", s.pair.label());
        let _ = writeln!(out, "      \"coverage_pct\": {:.1},", s.coverage_pct());
        let _ = writeln!(out, "      \"measured\": {},", s.included.len());
        let _ = writeln!(out, "      \"metrics\": {{");
        write_metrics(&mut out, "        ", ours, theirs);
        let _ = writeln!(out, "      }}");
        let comma = if i + 1 < curated.len() { "," } else { "" };
        let _ = writeln!(out, "    }}{comma}");
    }
    let _ = writeln!(out, "  ],");
    let _ = writeln!(out, "  \"complex_corpus\": {{");
    let _ = writeln!(out, "    \"provenance\": \"{COMPLEX_PROVENANCE}\",");
    let _ = writeln!(out, "    \"pairs\": [");
    for (i, (pair, coverage, ours, theirs)) in complex.iter().enumerate() {
        let _ = writeln!(out, "      {{");
        let _ = writeln!(out, "        \"pair\": \"{}\",", pair.label());
        let _ = writeln!(out, "        \"measured\": {},", complex_measured(coverage));
        let _ = writeln!(out, "        \"datasets\": [");
        for (j, c) in coverage.iter().enumerate() {
            let comma = if j + 1 < coverage.len() { "," } else { "" };
            let _ = writeln!(
                out,
                "          {{ \"name\": \"{}\", \"candidates\": {}, \"ours_accepts\": {}, \"theirs_accepts\": {}, \"both_accept\": {} }}{comma}",
                c.name, c.candidates, c.ours_accepts, c.theirs_accepts, c.both_accept,
            );
        }
        let _ = writeln!(out, "        ],");
        let _ = writeln!(out, "        \"metrics\": {{");
        write_metrics(&mut out, "          ", ours, theirs);
        let _ = writeln!(out, "        }}");
        let comma = if i + 1 < complex.len() { "," } else { "" };
        let _ = writeln!(out, "      }}{comma}");
    }
    let _ = writeln!(out, "    ]");
    let _ = writeln!(out, "  }}");
    let _ = writeln!(out, "}}");
    out
}

// ---------------------------------------------------------------------------
// Ratio regression gate (ADR-0016 / ADR-0017)
// ---------------------------------------------------------------------------
//
// Promotes the interim "regenerate + git diff" signal into a thresholded gate: the
// gate test re-measures with `measure` above, reads the committed
// `upstream-baseline.json`, and fails (or warns) if any ours/theirs ratio has grown
// past the baseline by more than `slack`. The dhat numbers are deterministic
// run-to-run, so this is a real local gate (ADR-0017: local-runnable, no CI
// coupling), not a flaky wall-clock check.
//
// WHY a slack at all, given determinism? `rust-toolchain.toml` tracks floating
// `stable`, so std's allocation patterns can shift between the environment that
// generated the baseline and a contributor's — the slack absorbs that toolchain
// drift, not run-to-run noise (there is none). Exact per-statement allocation
// counts are pinned separately and tightly in `tests/allocations.rs`
// (`dhat::assert_eq!`); this gate is the complementary corpus-wide "are we still
// far lighter than upstream" guard, so a few-percent slack here does not weaken
// those fine-grained pins.

/// Default head-room a ratio may grow before it counts as a regression (5%).
/// Override with `SQUONK_RATIO_GATE_SLACK` (a non-negative fraction, e.g.
/// `0.02` for 2%).
pub const DEFAULT_RATIO_SLACK: f64 = 0.05;

/// Env var → fractional slack override (see `DEFAULT_RATIO_SLACK`).
pub const RATIO_SLACK_ENV: &str = "SQUONK_RATIO_GATE_SLACK";

/// Env var → if set, the gate warns instead of failing.
pub const RATIO_WARN_ENV: &str = "SQUONK_RATIO_GATE_WARN";

/// The effective slack: `SQUONK_RATIO_GATE_SLACK` when set to a finite,
/// non-negative number, else `DEFAULT_RATIO_SLACK`.
pub fn ratio_slack() -> f64 {
    std::env::var(RATIO_SLACK_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.0)
        .unwrap_or(DEFAULT_RATIO_SLACK)
}

/// Whether the gate should warn instead of fail. The DEFAULT is fail, so a green
/// `cargo nextest run` actually enforces the gate. The CI policy — whether to run
/// this as a blocking gate and the fail-vs-warn default there — is deferred to
/// `prod-adr-perf-production-gate` so there is one perf-gate policy, not two.
pub fn gate_is_warn_only() -> bool {
    std::env::var_os(RATIO_WARN_ENV).is_some()
}

/// One side of one baseline metric (`ours`/`theirs` allocation counts). The file
/// also stores a rounded `ratio`, which the gate ignores: it recomputes ratios
/// from these exact integers to avoid the 3-decimal rounding in the snapshot.
#[derive(serde::Deserialize)]
struct MetricBaseline {
    ours: u64,
    theirs: u64,
}

/// The five baseline metrics for one pair. Fixed-field (not a map) so the metric
/// set is checked at parse time; matches the `rows` order via `baseline_rows`.
#[derive(serde::Deserialize)]
struct MetricsBaseline {
    transient_bytes: MetricBaseline,
    transient_blocks: MetricBaseline,
    retained_bytes: MetricBaseline,
    retained_blocks: MetricBaseline,
    peak_bytes: MetricBaseline,
}

/// One mapped dialect pair's baseline. `coverage_pct` / `measured` in the file are
/// human context the gate does not need, so they are simply not deserialized.
#[derive(serde::Deserialize)]
struct PairBaseline {
    pair: String,
    metrics: MetricsBaseline,
}

/// One complex dataset's pinned acceptance coverage under one pair. Exact, not
/// slack-bounded: a query silently dropping out of (or into) either parser's
/// surface must fail the gate, so these are equality-checked.
#[derive(serde::Deserialize)]
struct DatasetCoverageBaseline {
    name: String,
    candidates: usize,
    ours_accepts: usize,
    theirs_accepts: usize,
    both_accept: usize,
}

/// One pair's complex-corpus baseline: the per-dataset coverage (anti-vanishing +
/// coverage pins) and the aggregate both-accept heap metrics (slack-bounded ratios).
#[derive(serde::Deserialize)]
struct ComplexPairBaseline {
    pair: String,
    datasets: Vec<DatasetCoverageBaseline>,
    metrics: MetricsBaseline,
}

/// The complex-corpus section of the baseline (`provenance` is human context the
/// gate does not deserialize).
#[derive(serde::Deserialize)]
struct ComplexCorpusBaseline {
    pairs: Vec<ComplexPairBaseline>,
}

/// The whole committed baseline document.
#[derive(serde::Deserialize)]
struct Baseline {
    upstream_version: String,
    corpus_size: usize,
    pairs: Vec<PairBaseline>,
    complex_corpus: ComplexCorpusBaseline,
}

/// Baseline metric rows in the SAME stable order as `rows`, so the gate can zip
/// current against baseline by position.
fn baseline_rows(m: &MetricsBaseline) -> [(&'static str, u64, u64); 5] {
    [
        (
            "transient_bytes",
            m.transient_bytes.ours,
            m.transient_bytes.theirs,
        ),
        (
            "transient_blocks",
            m.transient_blocks.ours,
            m.transient_blocks.theirs,
        ),
        (
            "retained_bytes",
            m.retained_bytes.ours,
            m.retained_bytes.theirs,
        ),
        (
            "retained_blocks",
            m.retained_blocks.ours,
            m.retained_blocks.theirs,
        ),
        ("peak_bytes", m.peak_bytes.ours, m.peak_bytes.theirs),
    ]
}

/// One metric whose ours/theirs ratio grew past the allowed head-room.
#[derive(Clone, Debug)]
pub struct RatioRegression {
    pub pair: String,
    pub metric: &'static str,
    pub baseline_ratio: f64,
    pub current_ratio: f64,
    pub allowed_ratio: f64,
}

/// Pure decision rule: flag every metric whose current ratio exceeds
/// `baseline_ratio * (1 + slack)`. Split out from measurement/IO so the gate's
/// logic is unit-testable without `dhat` or the filesystem. `baseline` and
/// `current` must be in the same metric order (both come from the canonical
/// `rows` / `baseline_rows` order).
pub fn detect_regressions(
    pair: &str,
    baseline: &[(&'static str, u64, u64)],
    current: &[(&'static str, u64, u64)],
    slack: f64,
) -> Vec<RatioRegression> {
    let mut out = Vec::new();
    for (b, c) in baseline.iter().zip(current.iter()) {
        debug_assert_eq!(
            b.0, c.0,
            "metric order must match between baseline and current"
        );
        let baseline_ratio = ratio(b.1, b.2);
        let current_ratio = ratio(c.1, c.2);
        let allowed_ratio = baseline_ratio * (1.0 + slack);
        if current_ratio > allowed_ratio {
            out.push(RatioRegression {
                pair: pair.to_owned(),
                metric: c.0,
                baseline_ratio,
                current_ratio,
                allowed_ratio,
            });
        }
    }
    out
}

/// Re-measure every pair and compare each ratio to the committed baseline. Returns
/// the regressions (empty = clean), or `Err` if the baseline is unreadable or stale
/// (upstream-version / corpus-size / pair mismatch) — those are operator errors to
/// fix by regenerating, not regressions to tolerate.
pub fn measure_ratio_regressions(slack: f64) -> Result<Vec<RatioRegression>, String> {
    let path = baseline_path();
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("read baseline {}: {e}", path.display()))?;
    let baseline: Baseline = serde_json::from_str(&text)
        .map_err(|e| format!("parse baseline {}: {e}", path.display()))?;

    if baseline.upstream_version != UPSTREAM_VERSION {
        return Err(format!(
            "baseline upstream_version {:?} != measured {UPSTREAM_VERSION:?}; regenerate with \
             `cargo run -p squonk-bench --release --example compare_upstream \
             --features compare-heap -- --update-baseline`",
            baseline.upstream_version,
        ));
    }
    if baseline.corpus_size != corpus().len() {
        return Err(format!(
            "baseline corpus_size {} != current {}; regenerate the baseline",
            baseline.corpus_size,
            corpus().len(),
        ));
    }

    let mut regressions = Vec::new();
    for pair in PAIRS {
        let label = pair.label();
        let pb = baseline
            .pairs
            .iter()
            .find(|p| p.pair == label)
            .ok_or_else(|| format!("baseline missing pair `{label}`; regenerate the baseline"))?;
        let (_subset, ours, theirs) = measure(pair);
        let current = rows(&ours, &theirs);
        let base = baseline_rows(&pb.metrics);
        regressions.extend(detect_regressions(label, &base, &current, slack));

        // Complex corpus: same ratio gate over the aggregate both-accept subset. The
        // per-dataset coverage that subset is built from is checked exactly by
        // `verify_complex_coverage` (called alongside this in the gate test), so a
        // silently shrinking subset is caught there before it could mask a ratio.
        let cb = baseline
            .complex_corpus
            .pairs
            .iter()
            .find(|p| p.pair == label)
            .ok_or_else(|| {
                format!("baseline missing complex pair `{label}`; regenerate the baseline")
            })?;
        let (_coverage, c_ours, c_theirs) = measure_complex(pair);
        let c_current = rows(&c_ours, &c_theirs);
        let c_base = baseline_rows(&cb.metrics);
        regressions.extend(detect_regressions(
            &format!("complex {label}"),
            &c_base,
            &c_current,
            slack,
        ));
    }
    Ok(regressions)
}

/// Exact-check the complex corpus's per-dataset acceptance coverage against the
/// committed baseline. Returns `Err` describing every drift, or `Ok(())` when the
/// coverage is byte-for-byte what was pinned. Separate from the ratio gate because
/// coverage is deterministic and exact (no slack): a query dropping out of — or
/// newly entering — either parser's surface is always a reviewable change, never
/// "within tolerance".
pub fn verify_complex_coverage() -> Result<(), String> {
    let path = baseline_path();
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("read baseline {}: {e}", path.display()))?;
    let baseline: Baseline = serde_json::from_str(&text)
        .map_err(|e| format!("parse baseline {}: {e}", path.display()))?;

    let mut drift = Vec::new();
    for pair in PAIRS {
        let label = pair.label();
        let cb = baseline
            .complex_corpus
            .pairs
            .iter()
            .find(|p| p.pair == label)
            .ok_or_else(|| {
                format!("baseline missing complex pair `{label}`; regenerate the baseline")
            })?;
        let (coverage, _ours, _theirs) = measure_complex(pair);
        if coverage.len() != cb.datasets.len() {
            drift.push(format!(
                "[complex {label}] dataset count {} != baseline {}",
                coverage.len(),
                cb.datasets.len(),
            ));
            continue;
        }
        for (c, b) in coverage.iter().zip(&cb.datasets) {
            if c.name != b.name {
                drift.push(format!(
                    "[complex {label}] dataset order changed: {} != baseline {}",
                    c.name, b.name,
                ));
                continue;
            }
            if c.candidates != b.candidates
                || c.ours_accepts != b.ours_accepts
                || c.theirs_accepts != b.theirs_accepts
                || c.both_accept != b.both_accept
            {
                drift.push(format!(
                    "[complex {label}] {}: candidates {}/{}, ours {}/{}, theirs {}/{}, both {}/{} (current/baseline)",
                    c.name,
                    c.candidates, b.candidates,
                    c.ours_accepts, b.ours_accepts,
                    c.theirs_accepts, b.theirs_accepts,
                    c.both_accept, b.both_accept,
                ));
            }
        }
    }
    if drift.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "complex-corpus coverage drifted on {} dataset(s):\n  {}\nIf intentional (a parser \
             surface change or a re-vendor), refresh the baseline with \
             `cargo run -p squonk-bench --release --example compare_upstream \
             --features compare-heap -- --update-baseline` and review the diff.",
            drift.len(),
            drift.join("\n  "),
        ))
    }
}

/// Render the gate's panic/warn message for a non-empty regression list.
pub fn format_regressions(regressions: &[RatioRegression], slack: f64) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "upstream ours/theirs ratio regressed beyond {:.1}% slack on {} metric(s):",
        slack * 100.0,
        regressions.len(),
    );
    for r in regressions {
        let _ = writeln!(
            out,
            "  [{}] {}: baseline {:.4} -> current {:.4} (allowed <= {:.4}, +{:.1}% over baseline)",
            r.pair,
            r.metric,
            r.baseline_ratio,
            r.current_ratio,
            r.allowed_ratio,
            (r.current_ratio / r.baseline_ratio - 1.0) * 100.0,
        );
    }
    let _ = writeln!(
        out,
        "If this is an intentional change, refresh the baseline with \
         `cargo run -p squonk-bench --release --example compare_upstream \
         --features compare-heap -- --update-baseline` and review the diff. \
         Tune the threshold via {RATIO_SLACK_ENV}; downgrade to warn via {RATIO_WARN_ENV}.",
    );
    out
}

// ---------------------------------------------------------------------------
// Instruction-count comparison config (gungraun/callgrind; Linux-gated)
// ---------------------------------------------------------------------------
//
// Valgrind is Linux-only, so both the import and the builder are individually
// `#[cfg(target_os = "linux")]`-gated even though this module itself is also
// mounted un-gated (by `upstream_compare.rs` / `upstream_heap.rs`): on non-Linux
// `compare_config` must stay absent everywhere, exactly as it was before this was
// shared.

#[cfg(target_os = "linux")]
use gungraun::{Callgrind, LibraryBenchmarkConfig};

/// Callgrind `Ir` only, no soft limits: a TRACKER, not a gate — the ours/theirs
/// instruction ratio is tracked over gungraun's historical diffing, while our
/// absolute `Ir` is separately hard-gated by `perf.rs`'s `gate_config`. Shared by
/// `upstream_instr.rs` and `libpg_instr.rs` (both already `mod upstream;`).
#[cfg(target_os = "linux")]
pub fn compare_config() -> LibraryBenchmarkConfig {
    let mut config = LibraryBenchmarkConfig::default();
    config.tool(Callgrind::default());
    config
}
