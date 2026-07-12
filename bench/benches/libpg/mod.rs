// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Shared scaffolding for the apples-to-apples COMPUTE comparison against
//! `libpg_query` — the real PostgreSQL parser, in-process — via its `pg_query`
//! Rust bindings (ADR-0015: `pg_query` is already the differential oracle, so the
//! C library is built + cached; this adds no network and no second corpus).
//!
//! It deliberately reuses the `upstream` module's fairness scaffolding so the two
//! comparisons agree on exactly one corpus and one subset-selection rule: the
//! candidate set is `upstream::corpus()` (the PostgreSQL-regression supported
//! subset) plus `upstream::complex_datasets()` (TPC-H/DS + optimizer fixtures), the
//! SAME bytes the conformance oracle drives. The two sibling benches consume it:
//!
//! - `libpg_compare.rs` — codspeed-criterion wall-clock + this context report.
//! - `libpg_instr.rs`   — gungraun/callgrind instruction counts (Linux CI). Under
//!   callgrind the C parser is instrumented too, so the Ir count is a genuine
//!   ours-vs-the-reference-C-parser compute number.
//!
//! WHY COMPUTE ONLY (the load-bearing measurement-honesty boundary): libpg_query
//! allocates its parse tree in C (its own `palloc` memory contexts), which `dhat`
//! — a Rust GLOBAL-allocator hook — cannot observe. A dhat heap ratio against it
//! would silently measure ONLY our Rust side: a false comparison. So there is no
//! libpg heap bench and no libpg entry in `upstream-baseline.json`; the
//! Rust-vs-Rust dhat memory comparison stays in `upstream_heap.rs` (vs
//! `sqlparser-rs`), where both sides allocate through the same hook. See
//! `report_header` for the caveat text every run prints.
//!
//! Only the libpg benches mount this module (and `upstream`), so `pg_query` — and
//! the heavy C library it links — is NOT pulled into the existing
//! `upstream_*`/`upstream_gate` binaries; the ratio gate's dependency set is
//! unchanged.
#![allow(dead_code)]

use crate::upstream::{
    Excluded, ExclusionReason, Pair, complex_datasets, corpus, ours_parses, parse_ours,
};
use squonk_bench::BenchCase;
use std::fmt::Write as _;

/// The `pg_query` crate version this comparison is pinned against. Mirrors
/// `bench/Cargo.toml`'s `pg_query = "6.1"`, itself the version `conformance` pins
/// (the differential oracle), so the bench measures the same parser the
/// conformance suite asserts against. Keep the two in lockstep on bumps.
pub const PG_QUERY_VERSION: &str = "6.1";

/// The PostgreSQL major whose grammar libpg_query bundles (pg_query 6.x tracks
/// PostgreSQL 17). Printed so a raw bench log names the reference it measured.
pub const LIBPG_QUERY_PG_MAJOR: &str = "17";

/// Our parser runs under the Postgres dialect for this comparison: libpg_query IS
/// the PostgreSQL parser, so Postgres ↔ libpg_query is the only meaningful pairing
/// (there is no "generic" libpg_query to map `Ansi` onto).
pub const OURS_PAIR: Pair = Pair::PostgresPostgres;

// ---------------------------------------------------------------------------
// Cross-check fixtures — shared by `alloc_probe.rs` and `meta_probe.rs` (both
// already `mod libpg;`), whose samply allocation share is on record in
// `perf-ceiling-vs-floor.md` (allocation ~24% on `nested_expr`, ~26% on
// `star_join`), so both probes size their measurement against the same two cases.
// Also mirrored (independently) in `examples/perf_testbed.rs`, which is not a
// `mod libpg;` consumer — keep that copy in sync by hand.
// ---------------------------------------------------------------------------

/// Deeply nested arithmetic + boolean — Pratt/expression-heavy (samply alloc ~24%).
pub const NESTED_EXPR: &str = "SELECT ((((a + b) * (c - d)) / ((e + f) * (g - h))) + (((i * j) - (k / l)) + ((m + n) * (o - p)))) AS x FROM t WHERE (a > b AND c < d) OR (e = f AND g <> h) OR (i >= j AND k <= l) OR (m > n AND o < p)";
/// TPC-DS-shaped star join — identifier/keyword-heavy (samply alloc ~26%).
pub const STAR_JOIN: &str = "SELECT i_item_id, AVG(ss_quantity) AS agg1, AVG(ss_list_price) AS agg2, AVG(ss_coupon_amt) AS agg3 FROM store_sales, customer_demographics, date_dim, item, promotion WHERE ss_sold_date_sk = d_date_sk AND ss_item_sk = i_item_sk AND ss_cdemo_sk = cd_demo_sk AND ss_promo_sk = p_promo_sk AND cd_gender = 'M' AND cd_marital_status = 'S' AND cd_education_status = 'College' AND d_year = 2000 AND (p_channel_email = 'N' OR p_channel_event = 'N') GROUP BY i_item_id ORDER BY i_item_id";

/// Whether this binary was built with optimizations (the only honest profile to
/// measure) — printed by both `alloc_probe` and `meta_probe`'s report headers.
pub fn profile_note() -> &'static str {
    if cfg!(debug_assertions) {
        "DEBUG (UNOPTIMIZED — do NOT trust these numbers; rebuild with --profile profiling)"
    } else {
        "optimized (release/profiling)"
    }
}

// ---------------------------------------------------------------------------
// Parse adapters
// ---------------------------------------------------------------------------

/// `true` iff our Postgres parser accepts `sql`.
pub fn ours_parses_pg(sql: &str) -> bool {
    ours_parses(OURS_PAIR, sql)
}

/// `true` iff libpg_query (the real PostgreSQL parser) accepts `sql`.
pub fn libpg_parses(sql: &str) -> bool {
    pg_query::parse(sql).is_ok()
}

/// Parse `sql` to our owned AST and return the statement count — the cheap,
/// allocation-free black-box payload for the compute benches (mirrors
/// `upstream::parse_ours`).
pub fn parse_ours_pg(sql: &str) -> usize {
    parse_ours(OURS_PAIR, sql)
}

/// Parse `sql` through libpg_query the way a Rust caller actually does, and return
/// its statement count. This is the `theirs_full` series.
///
/// NOTE — what `pg_query::parse` costs: the full cost of obtaining a Rust-usable
/// parse tree from libpg_query — the C parse, libpg_query's protobuf SERIALIZATION
/// in C, prost DESERIALIZATION in Rust, AND `pg_query`'s table/function/CTE metadata
/// walk over the decoded tree (`ParseResult::new`). That is strictly MORE than a
/// bare parse. It is the only public entry that yields a tree, and is exactly what
/// the conformance oracle calls, so it is the realistic in-process integration cost.
/// Reading `stmts.len()` forces the whole protobuf tree to materialize, so nothing
/// is optimized away. Because it does extra non-parse work, [`parse_libpg_parse_only`]
/// is measured alongside it to BRACKET libpg_query's true cost and rule out that
/// extra work as a red herring (see `report_header`).
pub fn parse_libpg(sql: &str) -> usize {
    pg_query::parse(sql)
        .expect("subset statement parses (libpg_query)")
        .protobuf
        .stmts
        .len()
}

/// Run libpg_query's grammar over `sql` with the parse tree DISCARDED, and return
/// the statement count. This is the `theirs_parse_only` series — a LOWER BOUND on
/// libpg_query's compute, used to rule out the protobuf/metadata red herring.
///
/// `pg_query::split_with_parser` calls libpg_query's full `raw_parser` (it rejects
/// malformed SQL, so it is a complete grammar parse, not a tokenizer pass) and reads
/// back only each statement's `(location, len)`, building NO protobuf, doing NO
/// prost decode, and running NO metadata walk. So it isolates the C parse itself.
/// It is biased AGAINST us (we materialize a full owned AST with spans + interning;
/// this side materializes nothing), so if ours still beats it, the "we only look
/// fast because `pg_query::parse` does extra work" red herring is decisively ruled
/// out. The returned `Vec` is dropped immediately; `.len()` is the black-box payload.
pub fn parse_libpg_parse_only(sql: &str) -> usize {
    pg_query::split_with_parser(sql)
        .expect("subset statement parses (libpg_query split_with_parser)")
        .len()
}

// ---------------------------------------------------------------------------
// `theirs_tree_build` — the FAIR "build a usable owned Node tree" series (Part 1)
// ---------------------------------------------------------------------------
//
// Neither series above is the apples-to-apples fight for OUR value proposition.
// `theirs_full` ADDS protobuf serialization + a metadata walk (overstates libpg_query),
// while `theirs_parse_only` DISCARDS the tree (understates — it retains nothing, we
// retain a full owned AST). The honest middle is libpg_query BUILDING its `List*`/`Node*`
// parse tree (the real PostgreSQL palloc tree) and then bulk-freeing it, with NO protobuf
// — the cost of producing a usable tree, the thing we also do. This series is therefore
// bounded BELOW by `theirs_parse_only` and ABOVE by `theirs_full`.
//
// `pg_query`'s SAFE public API does not expose that point: `parse` always serializes to
// protobuf and `split_with_parser` always discards. libpg_query's C layer does, as the
// internal `pg_query_raw_parse` (declared in `src/pg_query_internal.h`, NOT the public
// `pg_query.h`, so `bindgen` generates no binding) — the exact call `pg_query_parse_protobuf`
// itself makes before serializing. We bind it directly with a few-line FFI shim; the
// lifecycle below is copied verbatim from `pg_query_parse_protobuf_opts`
// (`src/pg_query_parse.c`) minus the `pg_query_nodes_to_protobuf` step, so it measures
// exactly that function's tree-build cost with serialization removed.
//
// This is bench-only `unsafe`, confined here (workspace policy permits reviewed `unsafe`
// outside the AST crate); nothing ships. Because every measured statement is from the
// both-accept subset, libpg_query accepts it, so `raw_parser` never raises (no `longjmp`
// crosses the FFI boundary) and the returned `error` is always null.
#[allow(unsafe_code)]
mod ffi {
    use std::os::raw::{c_char, c_int, c_void};

    /// `PgQueryInternalParsetreeAndError` (libpg_query `src/pg_query_internal.h`): the
    /// raw `List*` parse tree, a `strdup`'d stderr buffer, and an optional error.
    #[repr(C)]
    pub struct ParsetreeAndError {
        pub tree: *mut PgList,
        pub stderr_buffer: *mut c_char,
        pub error: *mut c_void, // PgQueryError* — null on accept
    }

    /// The first two fields of PostgreSQL's `List` (`src/include/nodes/pg_list.h`):
    /// `NodeTag type; int length;`. Only `length` (the `RawStmt` count) is read; the
    /// remaining fields are deliberately elided — the pointer is only dereferenced, never
    /// moved or copied, so reading offset 4 of a live `List` is in-bounds and correct.
    #[repr(C)]
    pub struct PgList {
        pub node_tag: c_int,
        pub length: c_int,
    }

    unsafe extern "C" {
        /// Create + switch into a fresh palloc memory context (libpg_query `src/pg_query.c`).
        pub fn pg_query_enter_memory_context() -> *mut c_void;
        /// Switch back to the top context and bulk-free `ctx` (the whole parse tree).
        pub fn pg_query_exit_memory_context(ctx: *mut c_void);
        /// Run `raw_parser` over `input`, returning the palloc'd `List*` tree — no
        /// serialization. `parser_options = 0` is `PG_QUERY_PARSE_DEFAULT`.
        pub fn pg_query_raw_parse(input: *const c_char, parser_options: c_int)
        -> ParsetreeAndError;
        /// libc `free`, for the `strdup`'d `stderr_buffer` (malloc'd; outlives the context).
        pub fn free(ptr: *mut c_void);
    }
}

/// Build libpg_query's raw `List*`/`Node*` parse tree WITHOUT protobuf serialization,
/// then bulk-free it — the `theirs_tree_build` series, the fair "cost to produce a usable
/// owned tree" analogue of our owned-AST parse. Returns the statement count (the `RawStmt`
/// list length), the cheap black-box payload, matching the other series' semantics.
#[allow(unsafe_code)]
pub fn parse_libpg_tree_build(sql: &str) -> usize {
    let input = std::ffi::CString::new(sql).expect("subset SQL has no interior NUL byte");
    // SAFETY: the call sequence is copied verbatim from `pg_query_parse_protobuf_opts`
    // (libpg_query `src/pg_query_parse.c`), minus the `pg_query_nodes_to_protobuf` step:
    // enter a fresh memory context, raw-parse into it, read the result, then delete the
    // context (bulk-freeing the tree). On the both-accept subset `raw_parser` always
    // succeeds, so `error` is null and no `longjmp` escapes. `stderr_buffer` is a malloc'd
    // `strdup` that outlives the context delete, so it is freed explicitly. `tree` lives in
    // `ctx`; its `length` is read BEFORE the context is deleted.
    unsafe {
        let ctx = ffi::pg_query_enter_memory_context();
        let result = ffi::pg_query_raw_parse(input.as_ptr(), 0);
        debug_assert!(
            result.error.is_null(),
            "both-accept subset: libpg_query must accept every measured statement",
        );
        let n_stmts = if result.tree.is_null() {
            0
        } else {
            (*result.tree).length as usize
        };
        if !result.stderr_buffer.is_null() {
            ffi::free(result.stderr_buffer.cast());
        }
        ffi::pg_query_exit_memory_context(ctx);
        n_stmts
    }
}

// ---------------------------------------------------------------------------
// Subset selection (the fairness gate — only the both-accept intersection runs)
// ---------------------------------------------------------------------------

/// The measured intersection (statements BOTH our Postgres parser and libpg_query
/// accept) plus the logged exclusions, attributed to the side that rejected. Reuses
/// `upstream::{Excluded, ExclusionReason}` so the exclusion vocabulary is shared;
/// `describe_exclusion` re-labels them for the libpg pairing.
#[derive(Clone, Debug)]
pub struct LibpgSubset {
    pub included: Vec<BenchCase>,
    pub excluded: Vec<Excluded>,
}

impl LibpgSubset {
    /// Fraction of the candidate corpus both parsers accept, as a percentage.
    pub fn coverage_pct(&self) -> f64 {
        100.0 * self.included.len() as f64 / corpus().len() as f64
    }
}

/// Compute the both-accept subset of the curated corpus, logging every excluded
/// candidate with the side that rejected it.
pub fn libpg_subset() -> LibpgSubset {
    let mut included = Vec::new();
    let mut excluded = Vec::new();
    for &case in corpus() {
        match (ours_parses_pg(case.sql), libpg_parses(case.sql)) {
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
    LibpgSubset { included, excluded }
}

/// The complex corpus's both-accept subset, grouped per dataset, for the wall-clock
/// bench to time each dataset as one batch (mirrors `upstream::complex_both_accept`,
/// with libpg_query as the "theirs" side).
pub fn libpg_complex_both_accept() -> Vec<(&'static str, Vec<&'static str>)> {
    complex_datasets()
        .into_iter()
        .map(|ds| {
            let kept = ds
                .cases
                .into_iter()
                .filter(|sql| ours_parses_pg(sql) && libpg_parses(sql))
                .collect();
            (ds.name, kept)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Self-describing report blocks
// ---------------------------------------------------------------------------

/// Re-label an `upstream::ExclusionReason` for the libpg pairing, naming
/// libpg_query explicitly (the shared `ExclusionReason::describe` says "upstream",
/// which here would read as `sqlparser-rs` rather than the C parser).
fn describe_exclusion(reason: ExclusionReason) -> &'static str {
    match reason {
        ExclusionReason::OursRejects => "ours rejects (libpg_query accepts)",
        ExclusionReason::TheirsRejects => "libpg_query rejects (ours accepts)",
        ExclusionReason::BothReject => "both reject",
    }
}

/// The fixed context + fairness caveats every libpg run prints, so a raw bench log
/// is interpretable on its own. The caveats are stated, never silent: they record
/// exactly what is and is not comparable.
pub fn report_header() -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# libpg_query comparison: squonk vs the real PostgreSQL parser (in-process)"
    );
    let _ = writeln!(out, "#   ours dialect           : Postgres");
    let _ = writeln!(
        out,
        "#   theirs                 : libpg_query (PostgreSQL {LIBPG_QUERY_PG_MAJOR} grammar) via pg_query {PG_QUERY_VERSION}"
    );
    let _ = writeln!(
        out,
        "#   ratio = ours / theirs  (< 1.0 ⇒ we are faster than the reference C parser)"
    );
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "# COMPUTE is apples-to-apples: same process, same corpus, same both-accept subset."
    );
    let _ = writeln!(out, "# fairness caveats (read the ratios through these):");
    let _ = writeln!(
        out,
        "#   1. libpg_query's cost is BRACKETED by three series, so the protobuf/metadata"
    );
    let _ = writeln!(
        out,
        "#      overhead cannot masquerade as a parser-speed win (red-herring control),"
    );
    let _ = writeln!(
        out,
        "#      and theirs_tree_build is the FAIR owned-tree fight (we both build a tree):"
    );
    let _ = writeln!(
        out,
        "#        - theirs_full       = `pg_query::parse`: the realistic Rust integration"
    );
    let _ = writeln!(
        out,
        "#          cost — C parse + protobuf serialize (C) + prost decode (Rust) +"
    );
    let _ = writeln!(
        out,
        "#          pg_query's table/function/CTE metadata walk. The UPPER bound."
    );
    let _ = writeln!(
        out,
        "#        - theirs_parse_only = `pg_query::split_with_parser`: libpg_query's full"
    );
    let _ = writeln!(
        out,
        "#          grammar parse with the tree DISCARDED — no protobuf, no decode, no"
    );
    let _ = writeln!(
        out,
        "#          metadata walk. The LOWER bound on libpg_query's compute, and biased"
    );
    let _ = writeln!(
        out,
        "#          AGAINST us (it materializes no tree; we build a full owned AST). If"
    );
    let _ = writeln!(
        out,
        "#          ours beats even this, the result is not a protobuf/metadata artifact."
    );
    let _ = writeln!(
        out,
        "#        - theirs_tree_build = `pg_query_raw_parse` (internal): libpg_query BUILDS"
    );
    let _ = writeln!(
        out,
        "#          its palloc `List*`/`Node*` tree then bulk-frees it — NO protobuf, NO"
    );
    let _ = writeln!(
        out,
        "#          discard. The honest middle: the cost of producing a USABLE tree, the"
    );
    let _ = writeln!(
        out,
        "#          owned-tree fight we actually pick (lower-bounded by parse_only, upper"
    );
    let _ = writeln!(
        out,
        "#          by full). Our ratio vs this is the fair owned-AST standing."
    );
    let _ = writeln!(
        out,
        "#   2. MEMORY is DELIBERATELY OMITTED. libpg_query allocates its tree in C"
    );
    let _ = writeln!(
        out,
        "#      (its own palloc memory contexts), which `dhat` — a Rust global-allocator"
    );
    let _ = writeln!(
        out,
        "#      hook — cannot observe. A dhat heap ratio here would measure only OUR"
    );
    let _ = writeln!(
        out,
        "#      side, a false comparison, so there is no libpg heap bench and no libpg"
    );
    let _ = writeln!(
        out,
        "#      baseline entry. The Rust-vs-Rust dhat memory comparison lives in"
    );
    let _ = writeln!(out, "#      upstream_heap.rs (vs sqlparser-rs).");
    let _ = writeln!(
        out,
        "#   3. Our AST carries byte spans, interned Symbols, NodeId and Meta; the"
    );
    let _ = writeln!(
        out,
        "#      libpg_query protobuf tree does not. The ratios include that design cost."
    );
    let _ = writeln!(
        out,
        "#   4. Only the subset BOTH parsers accept is measured; coverage logged below."
    );
    out
}

/// The coverage line plus the attributed exclusion list for the curated subset.
pub fn report_subset(s: &LibpgSubset) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "#");
    let _ = writeln!(
        out,
        "# [Postgres vs libpg_query] coverage {:.1}% ({}/{} statements measured)",
        s.coverage_pct(),
        s.included.len(),
        corpus().len(),
    );
    if s.excluded.is_empty() {
        let _ = writeln!(out, "#   excluded: none");
    } else {
        let _ = writeln!(out, "#   excluded:");
        for ex in &s.excluded {
            let _ = writeln!(
                out,
                "#     - {:<28} {}",
                ex.case.name,
                describe_exclusion(ex.reason),
            );
        }
    }
    out
}
