// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Moderately AI Inc.

//! Conformance & round-trip oracle harness for `squonk`.
//!
//! This crate is the testing foundation for the parser (ADR-0014). It is kept out
//! of the published crates (`publish = false`) so test-only dependencies such as
//! `proptest` never reach downstream users.
//!
//! It provides:
//! 1. ANSI round-trip oracles over the public
//!    [`Ansi`] dialect.
//! 2. [`assert_roundtrips`] — the canonical round-trip oracle (parse, render
//!    `Canonical`, re-parse, compare ASTs structurally).
//! 3. [`assert_roundtrips_parenthesized`] — the same over the fully-parenthesized
//!    render mode, an *independent* precedence oracle (ADR-0008/0014).
//! 4. [`pg`] — PostgreSQL accept/reject differential testing plus an incremental
//!    protobuf-to-neutral-shape mapper for M1 SELECT constructs.
//! 5. [`fuzz`] — stable `cargo test` Bolero targets and replay hooks for parser
//!    panic discovery and generated-AST structural round-trips.
//! 6. [`goldens`] — regenerable datadriven goldens over engine verdicts, render
//!    modes, and targeted Debug-AST snapshots.
//! 7. [`oracle`] — the pluggable [`AcceptRejectOracle`](oracle::AcceptRejectOracle)
//!    differential seam ([`pg`] is the M1 PostgreSQL implementation; later dialect
//!    milestones supply their own engine without re-wiring the harness).
//! 8. [`token_stream`] — the tokenizer differential oracle (ADR-0005): a
//!    regenerable token-stream golden format plus span/UTF-8/coverage invariants
//!    checked independently of the parser.
//! 9. [`keyword_positions`] — the libpg_query differential over the
//!    {keyword} × {position} matrix (prod-keyword-position-reserved-sets): every
//!    cell's accept/reject verdict must agree with PostgreSQL except documented gaps.
//! 10. [`properties`] — `proptest` strategies and structural oracles over generated
//!     ASTs: legal-by-construction trees exercised through render -> parse ->
//!     structural-equal, independent of the fixed corpus above.
//!
//! ## Why the AST is the oracle, not the rendered string
//!
//! The prior art asserted the re-rendered SQL *text*, so any wrong-but-identically
//! -rendered tree shipped green (the DIV-class precedence bug, ADR-0014). A
//! downstream rewriter consumes the AST, so the AST is the unit of truth here: the oracle compares
//! *trees*, not strings.
//!
//! ## How structural equality works
//!
//! `Statement<NoExt>` derives `PartialEq`, and the [`Meta`](squonk_ast::Meta)
//! wrapper makes that derive *structural*: span and `NodeId` are excluded
//! (ADR-0002), and trivia lives out-of-band, so two trees compare equal exactly
//! when their semantic shape matches.
//!
//! Identifiers are interned to [`Symbol`](squonk_ast::Symbol)s, which are only
//! meaningful within the resolver that produced them. Conformance comparisons
//! therefore remap both trees through one shared test interner before using the
//! AST's ordinary derived `PartialEq`. Resolver-aware normalization remains a
//! diagnostic fallback, not the primary oracle.
//!
//! The generative property layer (`proptest` strategies over the AST) is
//! `m1-proptest`; M1 ships only the oracle plus a fixed corpus.
//!
//! ## Per-dialect module homes (the layout convention)
//!
//! Shared oracle machinery stays in shared-concern modules: the accept/reject and
//! structural *seams* ([`oracle`]), the neutral tree vocabulary ([`shape`]), the
//! verdict-sweep spine (`verdict_harness` — read its FAULT LINES doc for what stays
//! per-engine and why), and the cross-dialect matrix (`coverage`). Each *dialect* then
//! keeps one findable home per kind of piece — the oracle adapter, its curated corpora
//! and divergence allowlist, and its sweep (pins + ledger + reject classifier):
//!
//! | Dialect | Oracle adapter(s) | Curated corpora + allowlist | Sweep = pins + ledger + reject classifier |
//! |---|---|---|---|
//! | PostgreSQL (M1) | [`pg`]: `PgQueryOracle` (accept/reject) + `PgStructuralOracle` (premium neutral-shape mapper) + `PgMediatedStructuralOracle` (commodity fingerprint lane), with the `pg_shape` protobuf mapper in `pg/protobuf_shape.rs` | `pg::PG_DIVERGENCE_ALLOWLIST`; `PG_MEDIATED_DIVERGENCE_ALLOWLIST`; the `regress-guide.sql` fixtures | `corpus_pg_verdicts` — ParseOnly accept/reject + the mediated structural lane (both-accept pin); no reject classifier |
//! | SQLite (M2) | `m2::SqliteOracle` | `m2::SCHEMA_*` (shared with DuckDB), `SQLITE_GRAMMAR_GAPS`, `M2_DIVERGENCE_ALLOWLIST` (shared) | `corpus_sqlite_verdicts` — `classify_sqlite_reject`, `SQLITE_DIVERGENCE_ALLOWLIST`, `SQLITE_OVER_ACCEPTANCE_TICKET` |
//! | DuckDB (M2) | `m2::DuckDbOracle` (accept/reject) + `duckdb_structural::DuckDbStructuralOracle` (premium neutral-shape mapper) + `DuckDbMediatedStructuralOracle` (commodity `json_serialize_sql` tree-equality lane) | `m2::SCHEMA_*` (shared), `DUCKDB_GRAMMAR_GAPS`, `DUCKDB_MEDIATED_DIVERGENCE_ALLOWLIST` | `corpus_duckdb_verdicts` — `classify_reject`, `DUCKDB_DIVERGENCE_ALLOWLIST`, `GAP_FAMILIES`, + the mediated structural lane (both-accept pin) |
//! | MySQL (M3) | `m3::MySqlOracle` (+ the `classify_prep_error` oracle-death split, `WireVerdict`) | `m3::SCHEMA_*`, `MYSQL_SCHEMA_SETUP_SQL`, `M3_DIVERGENCE_ALLOWLIST` | `corpus_mysql_verdicts` — `classify_mysql_code`, `MYSQL_DIVERGENCE_ALLOWLIST`, `MYSQL_OVER_ACCEPTANCE_TICKET` |
//!
//! **Structural coverage has two tiers**, kept both: the hand-written **premium** mapper
//! (`PgStructuralOracle` -> the neutral [`shape`] vocabulary) encodes literal-value /
//! alias-name / IN-list-arity sensitivity, while the **commodity** fingerprint lane
//! (`PgMediatedStructuralOracle`, wired in `corpus_pg_verdicts`) round-trips a both-accept
//! statement through our renderer and self-compares `pg_query::fingerprint` in the
//! engine's own tree space — a small adapter, blind to exactly those three, so a mediated
//! green is not full structural coverage. Any engine with a tree/fingerprint channel can
//! add the commodity lane cheaply; the premium mapper is reserved for flagship engines.
//! DuckDB carries both tiers too: the premium `DuckDbStructuralOracle` (neutral-shape
//! mapper) plus the commodity `DuckDbMediatedStructuralOracle`, whose channel is
//! `json_serialize_sql` tree-equality (the two normalized parse trees compared) rather
//! than a fingerprint — blind to whatever DuckDB's serializer itself erases or folds.
//!
//! Two intentional non-uniformities the table encodes, kept on purpose:
//!
//! - **`m2` is the joint SQLite+DuckDB home**, not two files. The two in-process engines
//!   were co-designed as one M2 differential and share their curated `SCHEMA_*` corpora,
//!   the `M2_DIVERGENCE_ALLOWLIST`, and a joint test suite (`corpus_is_single_statement`
//!   and `oracle_rows_match_coverage_matrix` iterate both). Splitting it per-engine would
//!   hoist the shared corpora into a third module and fragment those joint tests — more
//!   pieces, not one clean home — so it stays joined. `m3` is the external-server MySQL
//!   home.
//! - **`m2`/`m3` are the ADR-0015 milestone names**, not dialect names, matching the
//!   milestone table in [`oracle`]. Deliberately kept: the milestone vocabulary is the
//!   domain language the ADR and the coverage matrix already speak.
//!
//! Feature gating is load-bearing and unchanged: `pg` + `corpus_pg_verdicts` are
//! unconditional (`pg_query` is always in-process); `m2`, `duckdb_structural`, and
//! `corpus_{sqlite,duckdb}_verdicts` are behind `oracle-engines`; `m3` and
//! `corpus_mysql_verdicts` are behind the independent `oracle-mysql`. An oracle adapter is
//! a `pub` library item (feature-gated, never `cfg(test)`) because the sweeps and the
//! coverage matrix consume it; each sweep is `cfg(test)`.
//!
//! ### Oracle environments and local ran/skipped status
//!
//! The `oracle-engines` build discovers `libduckdb` automatically through
//! `.cargo/config.toml` (no env vars); the `oracle-mysql` build connects to an external
//! `mysql:8` server at `MYSQL_ORACLE_URL` (`m3::MYSQL_ORACLE_URL_ENV`, default
//! `m3::DEFAULT_MYSQL_ORACLE_URL` = `mysql://root@127.0.0.1:3306`). Every oracle skips
//! cleanly when its engine is absent, so a green run proves parity only for the engines
//! that actually *ran*. Each
//! curated-corpus parity test makes that visible: `sqlite`/`duckdb`
//! (`m2::tests::{sqlite,duckdb}_accept_reject_parity_over_curated_corpus`) and `mysql`
//! (`m3::tests::mysql_accept_reject_parity_over_curated_corpus`) each print
//! `oracle-ran: <engine> (<version>)` on the ran path and `skipping <engine> differential:
//! …` on the skip path. `oracle-nightly.yml` greps those markers as the hard gate; the
//! same markers give a local ran-vs-skipped readout — run the three lanes with
//! `--features oracle-engines,oracle-mysql -E 'test(accept_reject_parity_over_curated_corpus)'
//! --success-output final` (the flag replays the markers, which nextest otherwise
//! suppresses for passing tests). The full environment setup, the runnable status command,
//! and the GPL-external vendoring boundary live in one home — the contributor oracle
//! environments (human-facing summary in `CONTRIBUTING.md` § Oracle environments) and
//! ADR-0015; this crate carries only the marker mechanism its tests emit.
//!
//! ### Where a new dialect's pieces go
//!
//! A new engine-backed dialect adds, in order: (1) an `AcceptRejectOracle` adapter — its
//! own module, or an existing milestone module like `m2` when the engine is co-designed
//! with one already there — carrying the curated `SCHEMA_INDEPENDENT_*` /
//! `SCHEMA_DEPENDENT_*` corpora and an initially-empty divergence allowlist; (2) a
//! `corpus_<dialect>_verdicts` sweep composing `verdict_harness`'s `Verdict` / `Quadrant`
//! / `RejectReason` with the engine's own `classify_*` reject classifier, per-corpus pins,
//! and ledger (the per-engine fault lines `verdict_harness` documents); (3) its `mod`
//! lines in this file under the correct feature gate; (4) a Cargo feature when the engine
//! links a library or dials a server; and, only if the engine can dump a parse tree, (5) a
//! `StructuralOracle` mapping it into the existing [`shape`] vocabulary (as
//! `duckdb_structural` does for DuckDB). No shared seam changes — that invariance is the
//! layout's whole point, and the reason a physical `dialects/<dialect>/` regroup was
//! evaluated and declined: it would not shrink this list.

use std::fmt::Write as _;

use squonk::dialect::Ansi;
use squonk::{Parsed, parse_with};
use squonk_ast::render::{RenderConfig, RenderCtx, RenderExt as _, RenderMode};

pub mod fuzz;
pub mod goldens;
pub mod keyword_positions;
// M2 SQLite + DuckDB accept/reject oracles (prod-dialect-m2-sqlite-duckdb-oracles).
// Opt-in behind `oracle-engines` so the default build links no system libduckdb and
// needs no env; the seam itself ([`oracle`]) and its M1 PostgreSQL implementation
// ([`pg`]) stay in every build.
// Thin system-libduckdb FFI (open/prepare/query) — replaces the duckdb-rs crate
// for the prepare-only + structural surface (duckdb-oracle-thin-prepare-binding).
#[cfg(feature = "oracle-engines")]
pub mod duckdb_ffi;
// Thin unsafe FFI over rusqlite's raw handle for the never-execute statement-count
// observation the SQLite raw-byte differential needs (rusqlite hides `pzTail`); the
// SQLite analogue of [`duckdb_ffi`]. Same `oracle-engines` gate.
#[cfg(feature = "oracle-engines")]
pub mod m2;
#[cfg(feature = "oracle-engines")]
pub mod sqlite_ffi;
// DuckDB SELECT-surface *structural* oracle via `json_serialize_sql`
// (duckdb-structural-oracle-select). The second structural source after PostgreSQL, so
// it introduces the `StructuralOracle` seam and maps DuckDB's JSON parse tree into the
// same neutral `QueryShape` family [`pg`] defines. Same `oracle-engines` gate as [`m2`]
// (it links the in-process `duckdb` engine).
#[cfg(feature = "oracle-engines")]
pub mod duckdb_structural;
// M3 MySQL accept/reject oracle (mysql-differential-oracle-no-daemon). Opt-in behind
// its own `oracle-mysql` feature (independent of `oracle-engines`) so it never links or
// vendors a GPL server — it speaks the wire protocol to an external `mysqld` (see [`m3`]).
#[cfg(feature = "oracle-mysql")]
pub mod m3;
// ClickHouse external-process ParseOnly oracle (oracle-parity-clickhouse).
#[cfg(feature = "oracle-clickhouse")]
pub mod clickhouse;
// BigQuery external-process ParseOnly cross-check (sqlglot; oracle-parity-bigquery).
#[cfg(feature = "oracle-bigquery")]
pub mod bigquery;
pub mod doc_truth;
pub mod oracle;
pub mod pg;
pub mod properties;
pub mod shape;
pub mod support_tiers;
// Whole-tree invariant coverage for the recovering parse path (ADR-0002/0005). `pub`
// and unconditional (not `cfg(test)`) like `fuzz` above: the standalone nightly
// libFuzzer crate (`conformance/fuzz`) drives its `recover_invariants` body, and that
// crate never sees `cfg(test)`.
pub mod recovery_invariants;
pub mod token_stream;

mod shared_interner;

// Whole-tree span-invariant walker (ADR-0002): crate-private like `shared_interner`
// above, and unconditional for the same reason — `fuzz::roundtrip_statement` calls
// its `assert_parsed_span_invariants` from a body the standalone nightly libFuzzer
// crate also compiles (`conformance/fuzz`), which never sees `cfg(test)`.
mod spans;

// Test-only: broad external corpora exercised only by `cargo test`/`nextest`,
// like `coverage` below. Keeping them `cfg(test)` avoids dead-code in the library.
#[cfg(test)]
mod corpus_complex;

// Generated (not vendored) corpus: seeded AST -> render -> replay. Test-only like
// the vendored `corpus_*` replayers; the generator reuses `properties` read-only.
#[cfg(test)]
mod corpus_generated;

// Shared "growing corpus" classify/rewrite/test harness `corpus_sqlglot` and
// `corpus_sqllogictest` below share (cleanup-conformance-corpus-growing-harness).
#[cfg(test)]
mod corpus_partition;

// Shared spine for the four `corpus_*_verdicts` sweeps below: verdict semantics,
// quadrant tally, the divergence ledger, the gap-class probe check, and the vendored
// corpus loaders (conformance-verdict-harness-consolidation).
#[cfg(test)]
mod verdict_harness;

// Routes every vendored corpus statement through the in-process `pg_query`
// accept/reject oracle (run-pg-accept-reject-over-vendored-corpora, ADR-0015): the
// verdict differential the round-trip oracles above cannot see.
#[cfg(test)]
mod corpus_pg_verdicts;

// SQLite phase-0 assessment sweep (sqlite-dialect-100-percent-programme): routes
// every vendored + authored SQLite-probe statement through the in-process `rusqlite`
// prepare oracle vs the fitted `Sqlite` preset (sqlite-featureset-preset), the true
// residual coverage-gap inventory. Needs the `oracle-engines` feature for
// `m2::SqliteOracle`, so it is gated on both.
#[cfg(all(test, feature = "oracle-engines"))]
mod corpus_sqlite_verdicts;

// Cross-dialect bitwise-operator precedence and PostgreSQL structural parity
// (bitwise-operators-cross-dialect-gap): the per-dialect grouping is the load-bearing
// risk, so it is pinned, round-tripped, and pg_query-verified here.
#[cfg(test)]
mod bitwise;

// DuckDB accept/reject sweep over the vendored signature-surface corpus (phase 0 of
// duckdb-dialect-100-percent-programme); oracle side behind `oracle-engines`.
#[cfg(test)]
mod corpus_duckdb_verdicts;

// MySQL phase-0 assessment sweep (mysql-dialect-100-percent-programme): the vendored
// corpora + authored MySQL probes through the external-server m3 oracle vs the FITTED
// MySql preset — the true residual inventory. Needs `oracle-mysql` for `m3`.
#[cfg(all(test, feature = "oracle-mysql"))]
mod corpus_mysql_verdicts;

// Shared render-stability check + triage vocabulary the vendored `corpus_*`
// replayers above and below build on (ADR-0014 P3, prod-corpus-idempotence-stability).
#[cfg(test)]
mod corpus_roundtrip;

#[cfg(test)]
mod corpus_sqlglot;

#[cfg(test)]
mod corpus_sqllogictest;

// Spelling-fidelity ratchet: token-diffs every accepted corpus statement against
// its canonical render, so a construct whose surface spelling the render collapses
// (a missing spelling tag, a structural normalization) is measured and triaged, not
// discovered by a formatter user (spike-formatter-spelling-fidelity-inventory).
#[cfg(test)]
mod spelling_fidelity;

#[cfg(test)]
mod coverage;

// ADR-0011 canonical-shape + surface-tag proofs (prod-dialect-canonical-tags-audit): for a
// construct dialects spell differently, one canonical AST shape + a compact surface tag,
// acceptance gated by `FeatureSet` data. Evicted from `coverage` — orthogonal to the
// ADR-0015 coverage gate `coverage` proves.
#[cfg(test)]
mod canonical_shapes;

#[cfg(test)]
mod redaction;

// The Lenient union-property lane (oracle-parity-lenient): every statement any shipped
// preset accepts must parse under Lenient, save the sanctioned exceptions. Test-only like
// the corpus replayers; drives the cheap accepted surface (per-dialect seeds + flag-aware
// generative probes) and consumes the head-contention ledger as its exception source.
#[cfg(test)]
mod union_lenient;

// Public-API DoS-safety: deeply nested input is rejected cleanly, never a stack
// overflow (ADR-0012). Black-box over the published entry points.
#[cfg(test)]
mod recursion;

/// The canonical round-trip oracle.
///
/// Parses `sql`, renders every statement in [`RenderMode::Canonical`] (the minimal
/// parentheses the binding-power table requires), re-parses the rendered text, and
/// asserts the re-parsed statements are structurally equal to the originals.
///
/// # Panics
///
/// Panics if `sql` (or its rendering) fails to parse, or if the round-trip changes
/// the tree — the message carries both renderings and both ASTs for a readable diff.
pub fn assert_roundtrips(sql: &str) {
    assert_roundtrips_in(sql, RenderMode::Canonical);
}

/// The parenthesized round-trip oracle — an *independent* precedence check.
///
/// Identical to [`assert_roundtrips`] but renders in [`RenderMode::Parenthesized`],
/// which wraps every binary/unary subexpression regardless of binding power. The
/// fully-parenthesized text makes grouping explicit, so a precedence mis-bind
/// re-parses to a different tree and fails here even when the canonical round-trip
/// would not (ADR-0008/0014). Run both oracles on every corpus string.
///
/// # Panics
///
/// As [`assert_roundtrips`].
pub fn assert_roundtrips_parenthesized(sql: &str) {
    assert_roundtrips_in(sql, RenderMode::Parenthesized);
}

/// Shared round-trip body for both oracles, parameterized by render mode.
fn assert_roundtrips_in(sql: &str, mode: RenderMode) {
    let parsed = parse_generic(sql);
    let rendered = render_statements(&parsed, mode);
    let reparsed = parse_generic(&rendered);

    let comparison = shared_interner::compare_statements_with_shared_symbols(
        parsed.statements(),
        parsed.resolver(),
        reparsed.statements(),
        reparsed.resolver(),
    );
    if !comparison.structurally_equal() {
        // Recompute the round-tripped rendering only on failure, for the diff.
        let reparsed_rendered = render_statements(&reparsed, mode);
        panic!(
            "{}",
            comparison.failure_message(
                &format!("round-trip mismatch in {mode:?} mode"),
                &[
                    ("input", sql),
                    ("first render", &rendered),
                    ("second render", &reparsed_rendered),
                ],
                None,
            ),
        );
    }
}

/// Parse `sql` with the [`Ansi`] dialect, panicking with context on failure.
///
/// A parse failure inside an oracle is itself a test failure (the corpus is meant
/// to be parseable), so this turns the `Result` into a descriptive panic.
fn parse_generic(sql: &str) -> Parsed {
    parse_with(sql, Ansi)
        .unwrap_or_else(|err| panic!("expected {sql:?} to parse under Ansi, but: {err:?}"))
}

/// Render every statement of `parsed` in `mode`, joined by `; ` into one string.
pub(crate) fn render_statements(parsed: &Parsed, mode: RenderMode) -> String {
    let config = RenderConfig {
        mode,
        ..RenderConfig::default()
    };
    let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
    let mut out = String::new();
    for (i, statement) in parsed.statements().iter().enumerate() {
        if i > 0 {
            out.push_str("; ");
        }
        // Writing to a `String` is infallible; only the node's own formatting could
        // error, and the M1 renderer never does.
        write!(out, "{}", statement.displayed(&ctx)).expect("rendering into a String cannot fail");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use squonk::dialect::Postgres;
    use squonk_ast::{
        AliasSpelling, BinaryOperator, Expr, Ident, Keyword, Literal, Meta, NodeId, ObjectName,
        Query, QuoteStyle, Select, SelectItem, SelectSpelling, SetExpr, Span, Statement, Symbol,
    };
    use thin_vec::{ThinVec, thin_vec};

    /// Currently-parseable SQL covering the fixed structural round-trip oracle.
    ///
    /// This corpus round-trips under the generic [`Ansi`] dialect (the feature *floor*),
    /// so — unlike the PostgreSQL differential corpus (`pg::tests`) — every entry is
    /// parseable with no gated extension enabled. The `required_features` labelling model
    /// (prod-coverage-labels-differential-corpus) therefore tags each entry with the empty
    /// set: a round-trip case applies to *every* dialect at or above the ANSI floor, so a
    /// partial dialect skips none of it. Per-entry labels are left implicit rather than
    /// written out as ~90 vacuous `[]`s; they gain teeth only once a *restrictive* (rather
    /// than additive) feature or a sub-ANSI dialect lands (M2+). The one feature-sensitive
    /// form here, `SELECT "x"`, is a structural divergence (quoted identifier vs string),
    /// already covered by `coverage::double_quoted_strings_flips_string_vs_quoted_ident`.
    const CORPUS: &[&str] = &[
        "SELECT 1",
        "SELECT a, b, *",
        "SELECT 1 + 2 * 3",
        "SELECT (1 + 2) * 3",
        "SELECT a = b || c",
        "SELECT (a < b) < c",
        "SELECT a < (b < c)",
        "SELECT NOT a",
        "SELECT - 1",
        "SELECT a AND b OR c",
        // Quoted identifiers: ANSI enables the standard `"..."` style, so these
        // exercise quote-style preservation, doubled-close unescape/re-escape, a
        // quoted alias, and a fully-quoted qualified name through the round trip.
        "SELECT \"x\"",
        "SELECT \"a\"\"b\"",
        "SELECT \"x\" AS \"y\"",
        "SELECT * FROM \"s\".\"t\"",
        "SELECT (SELECT 1)",
        "SELECT * FROM t WHERE EXISTS (SELECT 1)",
        "SELECT * FROM t WHERE a IN (SELECT b FROM u)",
        "SELECT * FROM t WHERE a NOT IN (SELECT b FROM u)",
        "SELECT * FROM t WHERE a = ANY (SELECT b FROM u)",
        "SELECT * FROM t WHERE a < ALL (SELECT b FROM u)",
        "SELECT * FROM t WHERE a = SOME (SELECT b FROM u)",
        // Set-operation subquery in derived-table position
        // (parse-parenthesized-set-operation-operand-in-derived-table-from-position):
        // the canonical 3-arm form round-trips, and — the regression — its
        // Parenthesized render `FROM ((SELECT …) UNION …) x` re-parses as the same
        // derived table rather than being mistaken for a parenthesized joined table.
        "SELECT * FROM (SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3) AS x",
        "SELECT * FROM ((SELECT 1) EXCEPT (SELECT 2)) AS x",
        "CREATE TABLE t (id INT PRIMARY KEY, name TEXT NOT NULL DEFAULT 'x')",
        "CREATE TABLE t (id BIGINT GENERATED ALWAYS AS IDENTITY (START WITH 10 INCREMENT BY 2 NO MINVALUE MAXVALUE 100 CACHE 5 NO CYCLE), n INT GENERATED ALWAYS AS (id + 1) STORED)",
        "CREATE TABLE t (id INT REFERENCES parent (id), CONSTRAINT pk PRIMARY KEY (id))",
        "CREATE TEMP TABLE IF NOT EXISTS t (id) ON COMMIT DROP AS SELECT 1 WITH NO DATA",
        // Schema, view, and index DDL. The `OR REPLACE`, `MATERIALIZED`, `TEMP`, and
        // `WITH CHECK OPTION` view spellings and the index sort modifiers are not gated,
        // so they round-trip under the generic dialect; the PostgreSQL-only index
        // clauses live in the PostgreSQL accept corpus instead.
        "CREATE SCHEMA s",
        "CREATE SCHEMA IF NOT EXISTS s AUTHORIZATION joe",
        "CREATE VIEW v AS SELECT 1",
        "CREATE OR REPLACE VIEW v (a, b) AS SELECT a, b FROM t WITH CASCADED CHECK OPTION",
        "CREATE TEMP VIEW v AS SELECT a FROM t WITH CHECK OPTION",
        "CREATE MATERIALIZED VIEW IF NOT EXISTS m AS SELECT a FROM t WITH NO DATA",
        "CREATE INDEX i ON t (a)",
        "CREATE UNIQUE INDEX i ON t (a, b)",
        "CREATE INDEX i ON t (lower(a), b DESC NULLS LAST)",
        "INSERT INTO t (id, name) VALUES (1, DEFAULT), (2, 'b')",
        "INSERT INTO t DEFAULT VALUES",
        "WITH src AS (SELECT 1) INSERT INTO t SELECT * FROM src",
        "INSERT INTO t AS target (id) OVERRIDING USER VALUE SELECT 1",
        "UPDATE t AS target SET a = 1, b = DEFAULT FROM u WHERE target.id = u.id",
        "WITH src AS (SELECT 1) UPDATE t target SET a = 1 WHERE EXISTS (SELECT 1)",
        "DELETE FROM t AS target USING u WHERE target.id = u.id",
        "WITH src AS (SELECT 1) DELETE FROM t target WHERE EXISTS (SELECT 1)",
        // Transaction control.
        "BEGIN",
        "START TRANSACTION",
        "START TRANSACTION ISOLATION LEVEL SERIALIZABLE, READ ONLY",
        "COMMIT",
        "ROLLBACK",
        "ROLLBACK TO SAVEPOINT sp1",
        "SAVEPOINT sp1",
        "RELEASE SAVEPOINT sp1",
        "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
        // The `[NOT] DEFERRABLE` mode on START / SET TRANSACTION.
        "START TRANSACTION READ ONLY, DEFERRABLE",
        "SET TRANSACTION NOT DEFERRABLE",
        // Session configuration: generic SET, a signed numeric value, and the
        // special-cased subforms (TIME ZONE / ROLE / SESSION AUTHORIZATION /
        // CONSTRAINTS / NAMES / SESSION CHARACTERISTICS).
        "SET search_path TO public, pg_catalog",
        "SET LOCAL statement_timeout TO 100",
        "SET x TO DEFAULT",
        "SET x TO -1",
        "SET TIME ZONE 'UTC'",
        "SET TIME ZONE LOCAL",
        "SET LOCAL TIME ZONE DEFAULT",
        "SET ROLE admin",
        "SET ROLE NONE",
        "SET SESSION AUTHORIZATION admin",
        "SET SESSION AUTHORIZATION DEFAULT",
        "SET CONSTRAINTS ALL DEFERRED",
        "SET CONSTRAINTS a, b IMMEDIATE",
        "SET NAMES utf8",
        "SET NAMES utf8 COLLATE utf8_bin",
        "SET NAMES DEFAULT",
        "SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL SERIALIZABLE, READ ONLY",
        "RESET ALL",
        "SHOW search_path",
        // Access control.
        "GRANT SELECT, INSERT ON t TO alice, bob",
        "GRANT ALL PRIVILEGES ON TABLE t TO alice WITH GRANT OPTION",
        "GRANT SELECT (a, b) ON t TO alice",
        "REVOKE GRANT OPTION FOR INSERT ON t FROM alice",
        // Access control: the wider privilege/object/grantee matrix, role membership,
        // and the `WITH ADMIN OPTION` / `GRANTED BY` trailers.
        "GRANT USAGE, EXECUTE ON SCHEMA s TO alice",
        "GRANT mypriv ON t TO alice",
        "GRANT USAGE ON SEQUENCE s TO bob, GROUP admins",
        "GRANT EXECUTE ON FUNCTION f(int, text), g TO alice",
        "GRANT SELECT ON ALL TABLES IN SCHEMA s TO PUBLIC",
        "GRANT USAGE ON FOREIGN DATA WRAPPER w TO alice",
        "GRANT SELECT ON t TO alice GRANTED BY bob",
        "GRANT admin, staff TO alice WITH ADMIN OPTION",
        "REVOKE admin FROM alice",
        "REVOKE ADMIN OPTION FOR admin FROM bob",
        "GRANT SELECT TO alice",
    ];

    #[test]
    fn corpus_round_trips_under_both_oracles() {
        for sql in CORPUS {
            assert_roundtrips(sql);
            assert_roundtrips_parenthesized(sql);
        }
    }

    #[test]
    fn transaction_and_dcl_statements_render_canonically() {
        // Keywords normalize to upper case, but the default source-fidelity render
        // (`PreserveSource`) replays the optional / noise word spellings the source
        // wrote — the `WORK`/`TRANSACTION` block words, the optional `SAVEPOINT`, the
        // `= `/`TO` SET separator, and `ALL [PRIVILEGES]` all round-trip via their
        // spelling tags (spelling-tags-keyword-operator-batch) — while identifiers keep
        // their source spelling.
        for (input, expected) in [
            ("begin", "BEGIN"),
            ("Start Transaction", "START TRANSACTION"),
            ("commit work", "COMMIT WORK"),
            ("rollback to sp1", "ROLLBACK TO sp1"),
            ("release sp1", "RELEASE sp1"),
            ("set search_path = public", "SET search_path = public"),
            // The `=` separator and a signed numeric value: `=` is replayed as written.
            ("set x = -1", "SET x = -1"),
            // Special SET subforms: keywords upper-case, sentinels canonicalize.
            ("set time zone local", "SET TIME ZONE LOCAL"),
            ("set role none", "SET ROLE NONE"),
            (
                "set constraints all deferred",
                "SET CONSTRAINTS ALL DEFERRED",
            ),
            // The `[NOT] DEFERRABLE` transaction mode.
            (
                "start transaction not deferrable",
                "START TRANSACTION NOT DEFERRABLE",
            ),
            ("grant all on t to alice", "GRANT ALL ON t TO alice"),
            // The `PRIVILEGES` noise word round-trips when written.
            (
                "grant all privileges on t to alice",
                "GRANT ALL PRIVILEGES ON t TO alice",
            ),
            (
                "revoke select on t from alice",
                "REVOKE SELECT ON t FROM alice",
            ),
            // Privilege/object keywords upper-case; the role name keeps its spelling.
            (
                "grant usage on sequence s to alice",
                "GRANT USAGE ON SEQUENCE s TO alice",
            ),
            // Role-membership grant: `WITH ADMIN OPTION` canonicalizes, role names do not.
            (
                "grant admin to bob with admin option",
                "GRANT admin TO bob WITH ADMIN OPTION",
            ),
        ] {
            let parsed = parse_generic(input);
            assert_eq!(
                render_statements(&parsed, RenderMode::Canonical),
                expected,
                "{input:?}",
            );
        }
    }

    #[test]
    fn multiple_statements_round_trip() {
        // Exercises the `; ` join and the shared interner across statements.
        assert_roundtrips("SELECT 1; SELECT a, b");
        assert_roundtrips_parenthesized("SELECT a + b * c; SELECT NOT a OR b");
    }

    #[test]
    fn parsed_literal_accessors_materialize_values_without_changing_rendering() {
        let sql = "SELECT 42, 1.5e3, 'it''s', TRUE, FALSE, NULL";
        let parsed = parse_generic(sql);

        assert_eq!(
            projection_literal(&parsed, 0).as_i64(parsed.source()),
            Ok(42)
        );
        assert_eq!(
            projection_literal(&parsed, 1)
                .as_decimal_text(parsed.source())
                .expect("decimal text materializes")
                .as_ref(),
            "1.5e3",
        );
        assert_eq!(
            projection_literal(&parsed, 2)
                .as_str(parsed.source())
                .expect("string text materializes")
                .as_ref(),
            "it's",
        );
        assert_eq!(projection_literal(&parsed, 3).as_bool(), Ok(true));
        assert_eq!(projection_literal(&parsed, 4).as_bool(), Ok(false));
        assert_eq!(projection_literal(&parsed, 5).as_null(), Ok(()));

        assert_roundtrips(sql);
        assert_roundtrips_parenthesized(sql);
    }

    #[test]
    fn postgres_string_literals_materialize_values_without_changing_rendering() {
        let sql = "SELECT E'line\\nquote\\'', $$a\\n'b$$, e'\\141\\x62\\u0063\\U00000064'";
        let parsed = parse_with(sql, Postgres).expect("PostgreSQL string literals parse");

        assert_eq!(
            projection_literal(&parsed, 0)
                .as_str(parsed.source())
                .expect("escape string materializes")
                .as_ref(),
            "line\nquote'",
        );
        assert_eq!(
            projection_literal(&parsed, 1)
                .as_str(parsed.source())
                .expect("dollar-quoted string materializes")
                .as_ref(),
            "a\\n'b",
        );
        assert_eq!(
            projection_literal(&parsed, 2)
                .as_str(parsed.source())
                .expect("numeric escapes materialize")
                .as_ref(),
            "abcd",
        );

        assert_eq!(render_statements(&parsed, RenderMode::Canonical), sql);
        assert_eq!(render_statements(&parsed, RenderMode::Parenthesized), sql);
    }

    #[test]
    fn fixed_corpus_comparison_uses_shared_symbols_not_raw_ids() {
        struct TinyResolver;

        impl squonk_ast::Resolver for TinyResolver {
            fn try_resolve(&self, sym: Symbol) -> Option<&str> {
                match sym.as_u32() {
                    1 => Some("a"),
                    2 => Some("b"),
                    _ => None,
                }
            }
        }

        let parsed = parse_generic("SELECT a, b");
        let independently_symbolized = select_two_expr_stmt(1, 2);

        assert_ne!(
            parsed.statements()[0],
            independently_symbolized,
            "the control comparison must fail before shared-symbol remapping",
        );

        let comparison = shared_interner::compare_statement_with_shared_symbols(
            &parsed.statements()[0],
            parsed.resolver(),
            &independently_symbolized,
            &TinyResolver,
        );
        assert!(
            comparison.structurally_equal(),
            "{}",
            comparison.failure_message(
                "fixed corpus shared-symbol comparison failed",
                &[("sql", "SELECT a, b")],
                None,
            ),
        );
    }

    #[test]
    fn shared_symbol_remap_preserves_identifier_case() {
        // `Nulls`/`NULLS` are unreserved keywords used as column identifiers, so the
        // interner keeps them verbatim. The shared-interner remap must preserve that
        // distinction; folding keyword case to the canonical slot would collapse two
        // different identifiers and mask a real divergence in the comparison oracle.
        let lower = parse_generic("SELECT Nulls");
        let upper = parse_generic("SELECT NULLS");

        let comparison = shared_interner::compare_statement_with_shared_symbols(
            &lower.statements()[0],
            lower.resolver(),
            &upper.statements()[0],
            upper.resolver(),
        );
        assert!(
            !comparison.structurally_equal(),
            "identifiers differing only in case (Asc vs ASC) must not be folded \
             together by the shared interner",
        );
    }

    #[test]
    fn function_calls_round_trip_structurally() {
        for sql in [
            "SELECT coalesce(a, b, c)",
            "SELECT now()",
            "SELECT count(*)",
            "SELECT count(DISTINCT a)",
            "SELECT f(a + 1, g(b))",
            "SELECT nullif(a, b), greatest(a, b, c), least(a, b)",
            "SELECT array_agg(a ORDER BY b DESC)",
            "SELECT count(*) FILTER (WHERE a)",
        ] {
            assert_roundtrips(sql);
            assert_roundtrips_parenthesized(sql);
        }
    }

    #[test]
    fn special_function_and_type_productions_round_trip_structurally() {
        for sql in [
            // SQL special value functions (PostgreSQL `SQLValueFunction`).
            "SELECT CURRENT_DATE",
            "SELECT CURRENT_TIMESTAMP",
            "SELECT CURRENT_TIME(3)",
            "SELECT LOCALTIMESTAMP(6)",
            "SELECT USER",
            "SELECT SESSION_USER",
            "SELECT CURRENT_CATALOG",
            // Special type productions.
            "SELECT CAST(a AS BIT)",
            "SELECT CAST(a AS BIT VARYING(3))",
            "SELECT CAST(a AS JSON)",
            "SELECT CAST(a AS NCHAR(5))",
            "SELECT CAST(a AS NATIONAL CHARACTER VARYING(5))",
        ] {
            assert_roundtrips(sql);
            assert_roundtrips_parenthesized(sql);
        }
    }

    #[test]
    fn case_expressions_round_trip_structurally() {
        for sql in [
            "SELECT CASE WHEN a THEN b WHEN c THEN d ELSE e END",
            "SELECT CASE a WHEN 1 THEN b ELSE c END",
            "SELECT CASE WHEN a > b THEN a ELSE b END",
        ] {
            assert_roundtrips(sql);
            assert_roundtrips_parenthesized(sql);
        }
    }

    #[test]
    fn extract_expressions_round_trip_structurally() {
        for sql in [
            "SELECT EXTRACT(year FROM a)",
            "SELECT EXTRACT(month FROM a + b)",
        ] {
            assert_roundtrips(sql);
            assert_roundtrips_parenthesized(sql);
        }
    }

    #[test]
    fn predicate_expressions_round_trip_structurally() {
        for sql in [
            "SELECT a FROM t WHERE a IS NULL",
            "SELECT a FROM t WHERE a IS NOT NULL",
            "SELECT a FROM t WHERE a BETWEEN 1 AND 2",
            "SELECT a FROM t WHERE a NOT BETWEEN 1 AND 2",
            "SELECT a FROM t WHERE a IN (1, 2, 3)",
            "SELECT a FROM t WHERE a NOT IN (1, 2)",
            "SELECT a FROM t WHERE a IN (SELECT b FROM u)",
        ] {
            assert_roundtrips(sql);
            assert_roundtrips_parenthesized(sql);
        }
    }

    #[test]
    fn window_functions_round_trip_structurally() {
        for sql in [
            "SELECT row_number() OVER ()",
            "SELECT rank() OVER (ORDER BY a)",
            "SELECT sum(a) OVER (PARTITION BY b)",
            "SELECT sum(a) OVER (PARTITION BY b, c ORDER BY d DESC)",
            "SELECT avg(a) OVER (ORDER BY b ROWS UNBOUNDED PRECEDING)",
            "SELECT avg(a) OVER (ORDER BY b ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)",
            "SELECT avg(a) OVER (ORDER BY b RANGE BETWEEN 1 PRECEDING AND 2 FOLLOWING)",
            "SELECT avg(a) OVER (GROUPS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING EXCLUDE TIES)",
            "SELECT count(*) OVER w FROM t WINDOW w AS (PARTITION BY a ORDER BY b)",
            "SELECT count(*) OVER (w ORDER BY b) FROM t WINDOW w AS (PARTITION BY a)",
            "SELECT count(*) FILTER (WHERE a > 0) OVER (PARTITION BY b) FROM t",
            "SELECT sum(b) OVER (PARTITION BY a) FROM t WINDOW w AS (ORDER BY a), v AS (PARTITION BY b)",
        ] {
            assert_roundtrips(sql);
            assert_roundtrips_parenthesized(sql);
        }
    }

    #[test]
    fn window_functions_render_canonically() {
        // Pin the exact canonical spelling (the round-trip oracle only proves the
        // tree survives, not how it is written).
        for (sql, expected) in [
            (
                "SELECT sum(a) OVER (PARTITION BY b ORDER BY c)",
                "SELECT sum(a) OVER (PARTITION BY b ORDER BY c)",
            ),
            (
                "SELECT avg(a) OVER (ORDER BY b ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)",
                "SELECT avg(a) OVER (ORDER BY b ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW)",
            ),
            (
                "SELECT count(*) OVER w FROM t WINDOW w AS (PARTITION BY a)",
                "SELECT count(*) OVER w FROM t WINDOW w AS (PARTITION BY a)",
            ),
        ] {
            let parsed = parse_generic(sql);
            assert_eq!(render_statements(&parsed, RenderMode::Canonical), expected);
        }
    }

    // --- hand-built AST builders (independent of the parser) -------------------

    /// Placeholder metadata for a hand-built node. Span and `NodeId` are excluded
    /// from structural equality (ADR-0002), so any non-zero value serves.
    fn meta() -> Meta {
        Meta::new(Span::SYNTHETIC, NodeId::new(1).expect("non-zero node id"))
    }

    /// An unqualified column reference for the `ordinal`th dynamic identifier
    /// symbol, matching the shape the parser builds (`ObjectName` of one
    /// unquoted `Ident`). Dynamic identifiers start after the fixed keyword range.
    fn col(ordinal: u32) -> Expr {
        let sym = Keyword::ALL.len() as u32 + ordinal;
        Expr::Column {
            name: ObjectName(thin_vec![Ident {
                sym: Symbol::new(sym).expect("non-zero symbol"),
                quote: QuoteStyle::None,
                meta: meta(),
            }]),
            meta: meta(),
        }
    }

    fn bin(left: Expr, op: BinaryOperator, right: Expr) -> Expr {
        Expr::BinaryOp {
            left: Box::new(left),
            op,
            right: Box::new(right),
            meta: meta(),
        }
    }

    /// Wrap a projection expression in the exact statement shape the M1 SELECT
    /// grammar produces, so a comparison isolates the expression under test.
    fn select_expr_stmt(expr: Expr) -> Statement {
        Statement::Query {
            query: Box::new(Query {
                with: None,
                body: SetExpr::Select {
                    select: Box::new(Select {
                        distinct: None,
                        straight_join: false,
                        projection: thin_vec![SelectItem::Expr {
                            expr,
                            alias: None,
                            alias_spelling: AliasSpelling::As,
                            meta: meta(),
                        }],
                        into: None,
                        from: ThinVec::new(),
                        lateral_views: ThinVec::new(),
                        connect_by: None,
                        selection: None,
                        group_by: ThinVec::new(),
                        group_by_quantifier: None,
                        group_by_all: None,
                        having: None,
                        windows: ThinVec::new(),
                        qualify: None,
                        sample: None,
                        spelling: SelectSpelling::Select,
                        meta: meta(),
                    }),
                    meta: meta(),
                },
                order_by: ThinVec::new(),
                order_by_all: None,
                limit_by: None,
                limit: None,
                settings: ThinVec::new(),
                format: None,
                locking: ThinVec::new(),
                pipe_operators: ThinVec::new(),
                for_clause: None,
                meta: meta(),
            }),
            meta: meta(),
        }
    }

    fn select_two_expr_stmt(first: u32, second: u32) -> Statement {
        Statement::Query {
            query: Box::new(Query {
                with: None,
                body: SetExpr::Select {
                    select: Box::new(Select {
                        distinct: None,
                        straight_join: false,
                        projection: thin_vec![
                            SelectItem::Expr {
                                expr: Expr::Column {
                                    name: object_name(first),
                                    meta: meta(),
                                },
                                alias: None,
                                alias_spelling: AliasSpelling::As,
                                meta: meta(),
                            },
                            SelectItem::Expr {
                                expr: Expr::Column {
                                    name: object_name(second),
                                    meta: meta(),
                                },
                                alias: None,
                                alias_spelling: AliasSpelling::As,
                                meta: meta(),
                            },
                        ],
                        into: None,
                        from: ThinVec::new(),
                        lateral_views: ThinVec::new(),
                        connect_by: None,
                        selection: None,
                        group_by: ThinVec::new(),
                        group_by_quantifier: None,
                        group_by_all: None,
                        having: None,
                        windows: ThinVec::new(),
                        qualify: None,
                        sample: None,
                        spelling: SelectSpelling::Select,
                        meta: meta(),
                    }),
                    meta: meta(),
                },
                order_by: ThinVec::new(),
                order_by_all: None,
                limit_by: None,
                limit: None,
                settings: ThinVec::new(),
                format: None,
                locking: ThinVec::new(),
                pipe_operators: ThinVec::new(),
                for_clause: None,
                meta: meta(),
            }),
            meta: meta(),
        }
    }

    fn object_name(sym: u32) -> ObjectName {
        ObjectName(thin_vec![Ident {
            sym: Symbol::new(sym).expect("non-zero symbol"),
            quote: QuoteStyle::None,
            meta: meta(),
        }])
    }

    fn projection_literal(parsed: &Parsed, index: usize) -> &Literal {
        let Statement::Query { query, .. } = &parsed.statements()[0] else {
            panic!("expected a query statement");
        };
        let SetExpr::Select { select, .. } = &query.body else {
            panic!("expected a SELECT body");
        };
        let Some(SelectItem::Expr {
            expr: Expr::Literal { literal, .. },
            alias: None,
            ..
        }) = select.projection.get(index)
        else {
            panic!("expected literal projection item {index}");
        };
        literal
    }

    #[test]
    fn parenthesized_oracle_has_teeth_against_a_mis_grouping() {
        // `a + b * c` binds as `a + (b * c)` (right-heavy). A precedence mis-bind
        // would instead yield `(a + b) * c` (left-heavy). We build that mis-grouped
        // tree by hand — independently of the parser — and show the structural
        // comparison both oracles rely on rejects it, so a real mis-bind could not
        // round-trip green. `x`, `y`, `z` intern in appearance order to the first
        // three dynamic symbols after the fixed keyword range (the documented
        // interning-order reliance), so the hand-built columns line up with the
        // parser's. They are not keywords in the full inventory, so they take
        // dynamic symbols rather than a fixed keyword slot.
        let parsed = parse_generic("SELECT x + y * z");

        let correct = select_expr_stmt(bin(
            col(1),
            BinaryOperator::Plus,
            bin(col(2), BinaryOperator::Multiply, col(3)),
        ));
        let mis_grouped = select_expr_stmt(bin(
            bin(col(1), BinaryOperator::Plus, col(2)),
            BinaryOperator::Multiply,
            col(3),
        ));

        // The hand-built CORRECT grouping equals the parser's tree exactly: the
        // statement wrapper is identical, so only the grouping is in question.
        assert_eq!(parsed.statements()[0], correct);
        // The mis-grouping is structurally distinct, so the oracle's `==` rejects
        // it — this is the test's teeth.
        assert_ne!(parsed.statements()[0], mis_grouped);

        // Full parenthesization makes the divergence explicit in the rendered text:
        // two trees that both *minimally* render to `a + b * c` become different.
        let config = RenderConfig {
            mode: RenderMode::Parenthesized,
            ..RenderConfig::default()
        };
        let ctx = RenderCtx::new(parsed.resolver(), parsed.source(), &config);
        assert_eq!(correct.displayed(&ctx).to_string(), "SELECT (x + (y * z))");
        assert_eq!(
            mis_grouped.displayed(&ctx).to_string(),
            "SELECT ((x + y) * z)"
        );
    }

    proptest! {
        /// Smoke test keeping `proptest` wired: the round-trip oracles hold over
        /// randomized operands of a fixed precedence-bearing shape. The real
        /// generative AST strategies (depth-bounded `prop_recursive`) are
        /// `m1-proptest`; this exercises both oracles under randomization meanwhile.
        #[test]
        fn precedence_shape_round_trips(a in any::<u32>(), b in any::<u32>(), c in any::<u32>()) {
            let sql = format!("SELECT {a} + {b} * {c}");
            assert_roundtrips(&sql);
            assert_roundtrips_parenthesized(&sql);
        }
    }
}
